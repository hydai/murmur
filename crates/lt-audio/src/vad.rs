use serde::{Deserialize, Serialize};

/// Audio level data for waveform visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioLevel {
    /// RMS (Root Mean Square) level, range 0.0 - 1.0
    pub rms: f32,
    /// Voice activity detected (true if RMS > threshold)
    pub voice_active: bool,
    /// Timestamp in milliseconds
    pub timestamp_ms: u64,
}

/// Simple RMS-based Voice Activity Detection
pub struct VadProcessor {
    threshold: f32,
}

impl VadProcessor {
    /// Create a new VAD processor with a given RMS threshold
    /// Typical threshold: 0.01 - 0.05 for normalized audio
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Calculate RMS and determine voice activity
    pub fn process(&self, samples: &[i16], timestamp_ms: u64) -> AudioLevel {
        let rms = Self::calculate_rms(samples);
        let voice_active = rms > self.threshold;

        AudioLevel {
            rms,
            voice_active,
            timestamp_ms,
        }
    }

    /// Calculate RMS (Root Mean Square) of audio samples
    /// Normalized to 0.0 - 1.0 range (assuming 16-bit samples)
    fn calculate_rms(samples: &[i16]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let sum_squares: f64 = samples
            .iter()
            .map(|&sample| {
                let normalized = sample as f64 / i16::MAX as f64;
                normalized * normalized
            })
            .sum();

        let mean_square = sum_squares / samples.len() as f64;
        mean_square.sqrt() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_calculation_silence() {
        let samples = vec![0i16; 1024];
        let rms = VadProcessor::calculate_rms(&samples);
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn test_rms_calculation_signal() {
        let samples = vec![1000i16; 1024];
        let rms = VadProcessor::calculate_rms(&samples);
        assert!(rms > 0.0);
        assert!(rms < 1.0);
    }

    #[test]
    fn test_vad_detection() {
        let vad = VadProcessor::new(0.01);

        // Silence
        let silence = vec![0i16; 1024];
        let level = vad.process(&silence, 0);
        assert!(!level.voice_active);
        assert_eq!(level.rms, 0.0);

        // Voice signal
        let voice = vec![5000i16; 1024];
        let level = vad.process(&voice, 100);
        assert!(level.voice_active);
        assert!(level.rms > 0.01);
    }
}
