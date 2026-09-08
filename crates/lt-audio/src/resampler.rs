use crate::error::{AudioError, Result};
use std::collections::VecDeque;
use std::f64::consts::PI;

const FILTER_PHASES: usize = 256;
const FILTER_ZERO_CROSSINGS: usize = 32;
const MAX_FILTER_RADIUS: usize = 4096;

/// Streaming mono resampler with a Blackman-windowed sinc low-pass filter.
/// Integer phase accumulation preserves the rate across arbitrary input chunks.
/// Filtering retains approximately 2 ms of lookahead at 16 kHz; call `flush`
/// at the end of a recording to emit that tail without extending its duration.
pub struct AudioResampler {
    input_sample_rate: u32,
    output_sample_rate: u32,
    channels: usize,
    partial_frame_sum: i64,
    partial_frame_channels: usize,
    filter_radius: usize,
    kernels: Vec<Vec<f64>>,
    input: VecDeque<i16>,
    buffer_start: u64,
    input_frames: u64,
    output_frames: u64,
    next_input_frame: u64,
    phase: u64,
    finished: bool,
}

impl AudioResampler {
    pub fn new(input_sample_rate: u32, output_sample_rate: u32, channels: usize) -> Result<Self> {
        if channels == 0 || input_sample_rate == 0 || output_sample_rate == 0 {
            return Err(AudioError::UnsupportedFormat(
                "Sample rates and number of channels must be > 0".to_string(),
            ));
        }
        let rate_ratio = (output_sample_rate as f64 / input_sample_rate as f64).min(1.0);
        let filter_radius = (FILTER_ZERO_CROSSINGS as f64 / rate_ratio).ceil() as usize;
        if filter_radius > MAX_FILTER_RADIUS {
            return Err(AudioError::UnsupportedFormat(
                "Resampling ratio requires an unsupported filter length".to_string(),
            ));
        }
        let kernels = if input_sample_rate == output_sample_rate {
            Vec::new()
        } else {
            // Leave a transition band below the lower Nyquist frequency.
            // Windowed-sinc FIR design: https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.firwin.html
            Self::build_kernels(filter_radius, 0.45 * rate_ratio)
        };
        Ok(Self {
            input_sample_rate,
            output_sample_rate,
            channels,
            partial_frame_sum: 0,
            partial_frame_channels: 0,
            filter_radius,
            kernels,
            input: VecDeque::new(),
            buffer_start: 0,
            input_frames: 0,
            output_frames: 0,
            next_input_frame: 0,
            phase: 0,
            finished: false,
        })
    }

    /// Accept interleaved i16 input. Incomplete channel frames and filter
    /// history are retained until subsequent calls supply the missing samples.
    pub fn resample(&mut self, input: &[i16]) -> Result<Vec<i16>> {
        if self.finished {
            return Err(AudioError::UnsupportedFormat(
                "Cannot resample after flush; create a new resampler".to_string(),
            ));
        }
        let mono = self.mix_frames(input);
        if self.input_sample_rate == self.output_sample_rate {
            return Ok(mono);
        }
        self.input_frames += mono.len() as u64;
        self.input.extend(mono);
        Ok(self.emit_available(false))
    }

    /// Finalize this stream. Zero padding supplies FIR lookahead only: total
    /// output length is ceil(input_frames * output_rate / input_rate).
    /// Repeated flushes return no additional samples.
    pub fn flush(&mut self) -> Result<Vec<i16>> {
        if self.partial_frame_channels != 0 {
            return Err(AudioError::UnsupportedFormat(
                "Audio ended with an incomplete interleaved channel frame".to_string(),
            ));
        }
        if self.finished {
            return Ok(Vec::new());
        }
        self.finished = true;
        if self.input_sample_rate == self.output_sample_rate {
            return Ok(Vec::new());
        }
        let tail = self.emit_available(true);
        self.input.clear();
        Ok(tail)
    }

    fn emit_available(&mut self, flushing: bool) -> Vec<i16> {
        let total_output = (self.input_frames as u128 * self.output_sample_rate as u128)
            .div_ceil(self.input_sample_rate as u128) as u64;
        let mut output = Vec::new();
        while self.output_frames < total_output {
            if !flushing && self.next_input_frame + self.filter_radius as u64 >= self.input_frames {
                break;
            }
            let phase_index =
                (self.phase * FILTER_PHASES as u64 / self.output_sample_rate as u64) as usize;
            let kernel = &self.kernels[phase_index];
            let first_frame = self.next_input_frame as i64 - self.filter_radius as i64;
            let mut sample = 0.0;
            for (offset, coefficient) in kernel.iter().enumerate() {
                let frame = first_frame + offset as i64;
                if frame >= self.buffer_start as i64 && frame < self.input_frames as i64 {
                    let value = self.input[(frame as u64 - self.buffer_start) as usize];
                    sample += value as f64 * coefficient;
                }
            }
            output.push(sample.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
            self.output_frames += 1;
            self.phase += self.input_sample_rate as u64;
            self.next_input_frame += self.phase / self.output_sample_rate as u64;
            self.phase %= self.output_sample_rate as u64;
        }
        // Retain only the history needed by the next convolution, independent
        // of input chunk boundaries and the total recording duration.
        let keep_from = self
            .next_input_frame
            .saturating_sub(self.filter_radius as u64);
        let discard = keep_from
            .saturating_sub(self.buffer_start)
            .min(self.input.len() as u64) as usize;
        self.input.drain(..discard);
        self.buffer_start += discard as u64;
        output
    }

    fn build_kernels(radius: usize, cutoff: f64) -> Vec<Vec<f64>> {
        (0..FILTER_PHASES)
            .map(|phase| {
                let fraction = phase as f64 / FILTER_PHASES as f64;
                let mut kernel: Vec<f64> = (0..=radius * 2)
                    .map(|tap| {
                        let distance = tap as f64 - radius as f64 - fraction;
                        if distance.abs() > radius as f64 {
                            return 0.0;
                        }
                        let angle = 2.0 * PI * cutoff * distance;
                        let sinc = if angle.abs() < 1e-12 {
                            1.0
                        } else {
                            angle.sin() / angle
                        };
                        let window_angle = PI * distance / radius as f64;
                        let window =
                            0.42 + 0.5 * window_angle.cos() + 0.08 * (2.0 * window_angle).cos();
                        2.0 * cutoff * sinc * window
                    })
                    .collect();
                let sum: f64 = kernel.iter().sum();
                for coefficient in &mut kernel {
                    *coefficient /= sum;
                }
                kernel
            })
            .collect()
    }

    fn mix_frames(&mut self, input: &[i16]) -> Vec<i16> {
        let mut mono = Vec::with_capacity(input.len() / self.channels);
        for &sample in input {
            self.partial_frame_sum += sample as i64;
            self.partial_frame_channels += 1;
            if self.partial_frame_channels == self.channels {
                mono.push((self.partial_frame_sum / self.channels as i64) as i16);
                self.partial_frame_sum = 0;
                self.partial_frame_channels = 0;
            }
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

    fn resample_complete(input: &[i16], input_rate: u32, output_rate: u32) -> Vec<i16> {
        let mut resampler = AudioResampler::new(input_rate, output_rate, 1).unwrap();
        let mut output = resampler.resample(input).unwrap();
        output.extend(resampler.flush().unwrap());
        output
    }

    #[test]
    fn chunk_boundaries_do_not_change_samples_or_duration() {
        for (input_rate, output_rate) in [
            (48000, 16000),
            (44100, 16000),
            (96000, 16000),
            (16000, 48000),
        ] {
            let input: Vec<i16> = (0..input_rate)
                .map(|i| ((i * 97 % 24001) as i32 - 12000) as i16)
                .collect();
            let expected = resample_complete(&input, input_rate, output_rate);
            for chunk_size in [1, 511, 512, 1024] {
                let mut resampler = AudioResampler::new(input_rate, output_rate, 1).unwrap();
                let mut actual = Vec::new();
                for chunk in input.chunks(chunk_size) {
                    actual.extend(resampler.resample(chunk).unwrap());
                }
                actual.extend(resampler.flush().unwrap());
                assert_eq!(
                    actual.len(),
                    output_rate as usize,
                    "duration changed at {input_rate}->{output_rate}, chunks={chunk_size}"
                );
                assert_eq!(
                    actual, expected,
                    "samples changed at {input_rate}->{output_rate}, chunks={chunk_size}"
                );
                assert!(resampler.input.is_empty());
            }
        }
    }

    #[test]
    fn downsampling_preserves_speech_band_and_rejects_aliases() {
        for input_rate in [44100, 48000, 96000] {
            for frequency in [1000.0, 6000.0, 8500.0, 10000.0, 15000.0] {
                let input: Vec<i16> = (0..input_rate / 5)
                    .map(|i| {
                        (10000.0 * (2.0 * PI * frequency * i as f64 / input_rate as f64).sin())
                            .round() as i16
                    })
                    .collect();
                let output = resample_complete(&input, input_rate, 16000);
                let interior = &output[64..output.len() - 64];
                let rms = (interior.iter().map(|&x| (x as f64).powi(2)).sum::<f64>()
                    / interior.len() as f64)
                    .sqrt();
                if frequency <= 6000.0 {
                    assert!(
                        (6900.0..7250.0).contains(&rms),
                        "passband loss at {input_rate} Hz / {frequency} Hz: {rms}"
                    );
                } else {
                    assert!(
                        rms < 10.0,
                        "aliased tone at {input_rate} Hz / {frequency} Hz: {rms}"
                    );
                }
            }
        }
    }

    #[test]
    fn flush_preserves_short_recording_and_is_idempotent() {
        let mut resampler = AudioResampler::new(48000, 16000, 1).unwrap();
        let mut input = vec![0; 16];
        input[15] = 12000;
        assert!(resampler.resample(&input).unwrap().is_empty());
        let tail = resampler.flush().unwrap();
        assert_eq!(tail.len(), 6);
        assert!(tail.iter().any(|&sample| sample != 0));
        assert!(resampler.flush().unwrap().is_empty());
        assert!(resampler.resample(&input).is_err());
    }

    #[test]
    fn split_stereo_frames_are_retained() {
        let mut resampler = AudioResampler::new(16000, 16000, 2).unwrap();
        assert!(resampler.resample(&[100]).unwrap().is_empty());
        assert_eq!(resampler.resample(&[200, 300]).unwrap(), [150]);
        assert_eq!(resampler.resample(&[400]).unwrap(), [350]);
        assert!(resampler.flush().unwrap().is_empty());
    }

    #[test]
    fn invalid_rates_and_incomplete_final_frame_are_rejected() {
        assert!(AudioResampler::new(0, 16000, 1).is_err());
        assert!(AudioResampler::new(48000, 0, 1).is_err());
        assert!(AudioResampler::new(48000, 16000, 0).is_err());
        assert!(AudioResampler::new(u32::MAX, 1, 1).is_err());
        let mut resampler = AudioResampler::new(48000, 16000, 2).unwrap();
        resampler.resample(&[100]).unwrap();
        assert!(resampler.flush().is_err());
    }

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
        let mut resampler = AudioResampler::new(16000, 16000, 2).unwrap();
        let input = vec![100i16, 200i16, 300i16, 400i16]; // 2 stereo frames
        let mono = resampler.mix_frames(&input);

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
        let mut resampler = AudioResampler::new(16000, 16000, 2).unwrap();

        // Empty input
        let empty: Vec<i16> = vec![];
        let mono = resampler.mix_frames(&empty);
        assert_eq!(mono.len(), 0);

        // Odd number of samples: keep the incomplete frame for the next call
        let odd_input = vec![100i16, 200i16, 300i16];
        let mono = resampler.mix_frames(&odd_input);
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
