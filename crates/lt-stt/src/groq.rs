use crate::http::{delegate_http_provider, HttpSttProvider};

/// Whisper REST provider sharing bounded buffering, timeouts, and shutdown.
pub struct GroqProvider {
    inner: HttpSttProvider,
}

impl GroqProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            inner: HttpSttProvider::new(
                "https://api.groq.com/openai/v1".into(),
                Some(api_key),
                Some("whisper-large-v3-turbo".into()),
                None,
            )
            .with_chunk_duration(3000),
        }
    }
}

delegate_http_provider!(GroqProvider);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_uses_expected_model() {
        assert_eq!(
            GroqProvider::new("test".into()).inner.model,
            "whisper-large-v3-turbo"
        );
    }
}
