use crate::error::{AudioError, Result};

/// Audio resampler for converting to 16kHz mono
/// Uses simple linear interpolation for now (can be upgraded to rubato later)
pub struct AudioResampler {
    input_sample_rate: u32,
    output_sample_rate: u32,
    channels: usize,
}

impl AudioResampler {
    /// Create a new resampler
    ///
    /// # Arguments
    /// * `input_sample_rate` - Input sample rate in Hz
    /// * `output_sample_rate` - Output sample rate in Hz (typically 16000)
    /// * `channels` - Number of input channels (will be converted to mono)
    pub fn new(
        input_sample_rate: u32,
        output_sample_rate: u32,
        channels: usize,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(AudioError::UnsupportedFormat(
                "Number of channels must be > 0".to_string(),
            ));
        }

        Ok(Self {
            input_sample_rate,
            output_sample_rate,
            channels,
        })
    }

    /// Resample i16 samples to 16kHz mono using linear interpolation
    ///
    /// # Arguments
    /// * `input` - Input samples (interleaved if multi-channel)
    ///
    /// # Returns
    /// Resampled mono i16 samples at target sample rate
    pub fn resample(&mut self, input: &[i16]) -> Result<Vec<i16>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Deinterleave and convert to mono
        let mono_input = self.to_mono(input);

        // Resample if needed
        if self.input_sample_rate == self.output_sample_rate {
            return Ok(mono_input);
        }

        let ratio =
            self.output_sample_rate as f64 / self.input_sample_rate as f64;
        let output_len = (mono_input.len() as f64 * ratio).ceil() as usize;

        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let input_pos = i as f64 / ratio;
            let index = input_pos.floor() as usize;
            let frac = input_pos - index as f64;

            let sample = if index + 1 < mono_input.len() {
                // Linear interpolation
                let s0 = mono_input[index] as f64;
                let s1 = mono_input[index + 1] as f64;
                let interpolated = s0 + (s1 - s0) * frac;
                interpolated.round() as i16
            } else if index < mono_input.len() {
                mono_input[index]
            } else {
                0
            };

            output.push(sample);
        }

        Ok(output)
    }

    /// Convert interleaved multi-channel audio to mono
    fn to_mono(&self, input: &[i16]) -> Vec<i16> {
        if self.channels == 1 {
            return input.to_vec();
        }

        let samples_per_channel = input.len() / self.channels;
        let mut mono = Vec::with_capacity(samples_per_channel);

        for frame_idx in 0..samples_per_channel {
            let mut sum: i32 = 0;
            for ch in 0..self.channels {
                let idx = frame_idx * self.channels + ch;
                if idx < input.len() {
                    sum += input[idx] as i32;
                }
            }
            let avg = (sum / self.channels as i32) as i16;
            mono.push(avg);
        }

        mono
    }

    pub fn input_sample_rate(&self) -> u32 {
        self.input_sample_rate
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.output_sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = AudioResampler::new(48000, 16000, 1);
        assert!(resampler.is_ok());
    }

    #[test]
    fn test_resample_mono() {
        let mut resampler = AudioResampler::new(48000, 16000, 1).unwrap();
        let input = vec![1000i16; 4800]; // 100ms at 48kHz
        let output = resampler.resample(&input).unwrap();

        // Should be roughly 1/3 the size (48kHz -> 16kHz)
        assert!(output.len() > 1500 && output.len() < 1700);
    }

    #[test]
    fn test_resample_stereo_to_mono() {
        let mut resampler = AudioResampler::new(48000, 16000, 2).unwrap();
        let input = vec![1000i16; 9600]; // 100ms stereo at 48kHz
        let output = resampler.resample(&input).unwrap();

        // Output should be mono and roughly 1/3 the size per channel
        assert!(output.len() > 1500 && output.len() < 1700);
    }

    #[test]
    fn test_no_resample_needed() {
        let mut resampler = AudioResampler::new(16000, 16000, 1).unwrap();
        let input = vec![1000i16; 1600]; // 100ms at 16kHz
        let output = resampler.resample(&input).unwrap();

        // Should be same size (no resampling)
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn test_to_mono_stereo() {
        let resampler = AudioResampler::new(16000, 16000, 2).unwrap();
        let input = vec![100i16, 200i16, 300i16, 400i16]; // 2 stereo frames
        let mono = resampler.to_mono(&input);

        assert_eq!(mono.len(), 2);
        assert_eq!(mono[0], 150); // avg of 100 and 200
        assert_eq!(mono[1], 350); // avg of 300 and 400
    }

    #[test]
    fn test_empty_input() {
        let mut resampler = AudioResampler::new(48000, 16000, 1).unwrap();
        let input = vec![];
        let output = resampler.resample(&input).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_single_sample() {
        let mut resampler = AudioResampler::new(16000, 16000, 1).unwrap();
        let input = vec![100i16];
        let output = resampler.resample(&input).unwrap();
        assert_eq!(output.len(), 1);
    }

    #[test]
    fn test_to_mono_edge_cases() {
        let resampler = AudioResampler::new(16000, 16000, 2).unwrap();

        // Empty input
        let empty: Vec<i16> = vec![];
        let mono = resampler.to_mono(&empty);
        assert_eq!(mono.len(), 0);

        // Odd number of samples (incomplete frame) - should handle gracefully
        let odd_input = vec![100i16, 200i16, 300i16];
        let mono = resampler.to_mono(&odd_input);
        assert_eq!(mono.len(), 1); // Only complete frames
    }

    #[test]
    fn test_extreme_values() {
        let mut resampler = AudioResampler::new(16000, 16000, 1).unwrap();

        // Test with max and min i16 values
        let input = vec![i16::MAX, i16::MIN, 0i16];
        let output = resampler.resample(&input).unwrap();
        assert_eq!(output.len(), 3);
    }

    #[test]
    fn test_high_sample_rate_conversion() {
        // Test 96kHz to 16kHz (6:1 ratio)
        let mut resampler = AudioResampler::new(96000, 16000, 1).unwrap();
        let input = vec![1000i16; 9600]; // 100ms at 96kHz
        let output = resampler.resample(&input).unwrap();

        // Should be roughly 1/6 the size
        assert!(output.len() > 1500 && output.len() < 1700);
    }
}
