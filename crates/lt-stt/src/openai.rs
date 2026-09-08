use crate::http::{delegate_http_provider, HttpSttProvider};

/// Whisper REST provider sharing bounded buffering, timeouts, and shutdown.
pub struct OpenAIProvider {
    inner: HttpSttProvider,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            inner: HttpSttProvider::new(
                "https://api.openai.com/v1".into(),
                Some(api_key),
                Some("whisper-1".into()),
                None,
            )
            .with_chunk_duration(4000),
        }
    }
}

delegate_http_provider!(OpenAIProvider);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_uses_expected_model() {
        assert_eq!(OpenAIProvider::new("test".into()).inner.model, "whisper-1");
    }
}
