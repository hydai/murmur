use lt_core::error::{MurmurError, Result};
use lt_core::stt::AudioChunk;
use std::io::Cursor;
use tracing::debug;

/// Audio chunker for REST APIs
/// Accumulates audio samples and encodes them as WAV when flushed
pub struct AudioChunker {
    /// Accumulated audio samples
    buffer: Vec<i16>,
    /// Sample rate (Hz)
    sample_rate: u32,
    /// Chunk duration in milliseconds
    chunk_duration_ms: u64,
    /// Last flush timestamp
    last_flush_ms: u64,
}

impl AudioChunker {
    /// Create a new audio chunker
    ///
    /// # Arguments
    /// * `chunk_duration_ms` - Duration in milliseconds before auto-flush (e.g., 3000-5000ms)
    pub fn new(chunk_duration_ms: u64) -> Self {
        Self {
            buffer: Vec::new(),
            sample_rate: 16000, // 16kHz as per spec
            chunk_duration_ms,
            last_flush_ms: 0,
        }
    }

    /// Add an audio chunk to the buffer
    pub fn add_chunk(&mut self, chunk: &AudioChunk) {
        self.buffer.extend_from_slice(&chunk.data);

        // Update last flush timestamp if this is the first chunk
        if self.last_flush_ms == 0 {
            self.last_flush_ms = chunk.timestamp_ms;
        }

        debug!(
            "Added {} samples to buffer (total: {} samples)",
            chunk.data.len(),
            self.buffer.len()
        );
    }

    /// Check if the buffer should be flushed based on duration
    pub fn should_flush(&self, current_timestamp_ms: u64) -> bool {
        if self.buffer.is_empty() {
            return false;
        }

        let elapsed_ms = current_timestamp_ms.saturating_sub(self.last_flush_ms);
        elapsed_ms >= self.chunk_duration_ms
    }

    /// Flush the buffer and encode as WAV
    /// Returns the WAV bytes ready for upload
    pub fn flush(&mut self) -> Result<Vec<u8>> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }

        debug!("Flushing {} samples as WAV", self.buffer.len());

        // Encode as WAV using hound
        let wav_bytes = self.encode_wav(&self.buffer)?;

        // Clear the buffer
        self.buffer.clear();
        self.last_flush_ms = 0;

        Ok(wav_bytes)
    }

    /// Get the current buffer size in samples
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Encode PCM samples as WAV bytes
    fn encode_wav(&self, samples: &[i16]) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());

        {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: self.sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };

            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .map_err(|e| MurmurError::Stt(format!("Failed to create WAV writer: {}", e)))?;

            for &sample in samples {
                writer
                    .write_sample(sample)
                    .map_err(|e| MurmurError::Stt(format!("Failed to write WAV sample: {}", e)))?;
            }

            writer
                .finalize()
                .map_err(|e| MurmurError::Stt(format!("Failed to finalize WAV: {}", e)))?;
        }

        Ok(cursor.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunker_creation() {
        let chunker = AudioChunker::new(3000);
        assert_eq!(chunker.buffer_size(), 0);
        assert_eq!(chunker.chunk_duration_ms, 3000);
    }

    #[test]
    fn test_add_chunk() {
        let mut chunker = AudioChunker::new(3000);

        let chunk = AudioChunk {
            data: vec![1, 2, 3, 4, 5],
            timestamp_ms: 100,
        };

        chunker.add_chunk(&chunk);
        assert_eq!(chunker.buffer_size(), 5);
    }

    #[test]
    fn test_should_flush() {
        let mut chunker = AudioChunker::new(3000);

        // Empty buffer should not flush
        assert!(!chunker.should_flush(5000));

        // Add chunk at timestamp 1000
        let chunk = AudioChunk {
            data: vec![1, 2, 3],
            timestamp_ms: 1000,
        };
        chunker.add_chunk(&chunk);

        // At timestamp 3000 (2000ms elapsed), should not flush yet
        assert!(!chunker.should_flush(3000));

        // At timestamp 4000 (3000ms elapsed), should flush
        assert!(chunker.should_flush(4000));

        // At timestamp 5000 (4000ms elapsed), should flush
        assert!(chunker.should_flush(5000));
    }

    #[test]
    fn test_flush_wav_encoding() {
        let mut chunker = AudioChunker::new(3000);

        // Add some sample data
        let chunk = AudioChunk {
            data: vec![100, 200, -100, -200, 0],
            timestamp_ms: 1000,
        };
        chunker.add_chunk(&chunk);

        // Flush and get WAV bytes
        let wav_bytes = chunker.flush().expect("Failed to flush");

        // Should have WAV header + data
        assert!(wav_bytes.len() > 44); // WAV header is 44 bytes

        // Check RIFF header
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");

        // Buffer should be cleared after flush
        assert_eq!(chunker.buffer_size(), 0);
    }

    #[test]
    fn test_flush_empty_buffer() {
        let mut chunker = AudioChunker::new(3000);

        let wav_bytes = chunker.flush().expect("Failed to flush");
        assert!(wav_bytes.is_empty());
    }
}
