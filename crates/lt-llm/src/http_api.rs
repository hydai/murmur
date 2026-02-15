use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use reqwest::Client;
use std::time::{Duration, Instant};

use crate::prompts::PromptManager;

/// API format determines how requests and responses are serialized
#[derive(Debug, Clone)]
pub enum ApiFormat {
    /// OpenAI Chat Completions API (also used for custom endpoints)
    OpenAi,
    /// Anthropic Messages API
    Claude,
    /// Google Gemini REST API
    GeminiApi,
}

/// Default models per provider
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-4o-mini";
pub const CLAUDE_DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
pub const GEMINI_API_DEFAULT_MODEL: &str = "gemini-2.0-flash";

/// HTTP-based LLM processor supporting multiple API formats
pub struct HttpLlmProcessor {
    client: Client,
    api_format: ApiFormat,
    base_url: String,
    api_key: String,
    model: String,
    prompt_manager: PromptManager,
    timeout_secs: u64,
}

impl HttpLlmProcessor {
    /// Create an OpenAI API processor
    pub fn openai(api_key: String, model: Option<String>) -> Self {
        let model = model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| OPENAI_DEFAULT_MODEL.to_string());
        Self {
            client: Client::new(),
            api_format: ApiFormat::OpenAi,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key,
            model,
            prompt_manager: PromptManager::new(),
            timeout_secs: 30,
        }
    }

    /// Create a Claude API processor
    pub fn claude(api_key: String, model: Option<String>) -> Self {
        let model = model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| CLAUDE_DEFAULT_MODEL.to_string());
        Self {
            client: Client::new(),
            api_format: ApiFormat::Claude,
            base_url: "https://api.anthropic.com".to_string(),
            api_key,
            model,
            prompt_manager: PromptManager::new(),
            timeout_secs: 30,
        }
    }

    /// Create a Gemini REST API processor
    pub fn gemini_api(api_key: String, model: Option<String>) -> Self {
        let model = model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| GEMINI_API_DEFAULT_MODEL.to_string());
        Self {
            client: Client::new(),
            api_format: ApiFormat::GeminiApi,
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            api_key,
            model,
            prompt_manager: PromptManager::new(),
            timeout_secs: 30,
        }
    }

    /// Create a custom OpenAI-compatible endpoint processor
    pub fn custom(base_url: String, api_key: String, model: Option<String>) -> Self {
        let model = model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| OPENAI_DEFAULT_MODEL.to_string());
        Self {
            client: Client::new(),
            api_format: ApiFormat::OpenAi,
            base_url,
            api_key,
            model,
            prompt_manager: PromptManager::new(),
            timeout_secs: 30,
        }
    }

    /// Build the HTTP request for the given prompt
    fn build_request(&self, prompt: &str) -> Result<reqwest::RequestBuilder> {
        match &self.api_format {
            ApiFormat::OpenAi => {
                let url = format!("{}/chat/completions", self.base_url);
                let body = serde_json::json!({
                    "model": self.model,
                    "messages": [
                        { "role": "system", "content": "You are a helpful text processing assistant. Follow the instructions precisely and return only the processed text." },
                        { "role": "user", "content": prompt }
                    ]
                });
                Ok(self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .json(&body))
            }
            ApiFormat::Claude => {
                let url = format!("{}/v1/messages", self.base_url);
                let body = serde_json::json!({
                    "model": self.model,
                    "max_tokens": 4096,
                    "system": "You are a helpful text processing assistant. Follow the instructions precisely and return only the processed text.",
                    "messages": [
                        { "role": "user", "content": prompt }
                    ]
                });
                Ok(self
                    .client
                    .post(&url)
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body))
            }
            ApiFormat::GeminiApi => {
                let url = format!(
                    "{}/v1beta/models/{}:generateContent?key={}",
                    self.base_url, self.model, self.api_key
                );
                let body = serde_json::json!({
                    "contents": [
                        {
                            "parts": [{ "text": prompt }]
                        }
                    ]
                });
                Ok(self.client.post(&url).json(&body))
            }
        }
    }

    /// Extract the response text from the API-specific JSON
    fn extract_response(&self, json: &serde_json::Value) -> Result<String> {
        let text = match &self.api_format {
            ApiFormat::OpenAi => json
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str()),
            ApiFormat::Claude => json
                .get("content")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str()),
            ApiFormat::GeminiApi => json
                .get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str()),
        };

        text.map(|s| s.to_string()).ok_or_else(|| {
            MurmurError::Llm(format!(
                "Failed to extract text from API response: {}",
                serde_json::to_string_pretty(json).unwrap_or_default()
            ))
        })
    }

    /// Map HTTP status codes to user-friendly error messages
    fn map_http_error(&self, status: reqwest::StatusCode, body: &str) -> MurmurError {
        match status.as_u16() {
            401 => MurmurError::Llm("Authentication failed. Check your API key.".to_string()),
            429 => MurmurError::Llm("Rate limited. Please wait and try again.".to_string()),
            500..=599 => MurmurError::Llm("Server error. Try again later.".to_string()),
            _ => MurmurError::Llm(format!(
                "API request failed (HTTP {}): {}",
                status,
                body.chars().take(200).collect::<String>()
            )),
        }
    }
}

#[async_trait]
impl LlmProcessor for HttpLlmProcessor {
    async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
        let start_time = Instant::now();

        let prompt = self.prompt_manager.build_prompt(&task);

        tracing::debug!(
            "Sending HTTP API request ({:?}, model: {}, prompt length: {} chars)",
            self.api_format,
            self.model,
            prompt.len()
        );

        let request = self.build_request(&prompt)?;

        let response = request
            .timeout(Duration::from_secs(self.timeout_secs))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    MurmurError::Llm(format!("Request timed out ({}s).", self.timeout_secs))
                } else if e.is_connect() {
                    MurmurError::Llm(format!(
                        "Failed to connect to {}. Check your network connection.",
                        self.base_url
                    ))
                } else {
                    MurmurError::Llm(format!("HTTP request failed: {}", e))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::error!("API error (HTTP {}): {}", status, body);
            return Err(self.map_http_error(status, &body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| MurmurError::Llm(format!("Failed to parse API response: {}", e)))?;

        let processed_text = self.extract_response(&json)?;
        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            "HTTP LLM processing completed in {}ms (output: {} chars)",
            processing_time_ms,
            processed_text.len()
        );

        Ok(ProcessingOutput {
            text: processed_text,
            processing_time_ms,
            metadata: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        // Return true if API key is non-empty (no live API call to avoid cost)
        Ok(!self.api_key.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_constructor() {
        let processor = HttpLlmProcessor::openai("test-key".to_string(), None);
        assert_eq!(processor.model, OPENAI_DEFAULT_MODEL);
        assert_eq!(processor.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_claude_constructor() {
        let processor = HttpLlmProcessor::claude("test-key".to_string(), None);
        assert_eq!(processor.model, CLAUDE_DEFAULT_MODEL);
        assert_eq!(processor.base_url, "https://api.anthropic.com");
    }

    #[test]
    fn test_gemini_api_constructor() {
        let processor = HttpLlmProcessor::gemini_api("test-key".to_string(), None);
        assert_eq!(processor.model, GEMINI_API_DEFAULT_MODEL);
    }

    #[test]
    fn test_custom_constructor() {
        let processor = HttpLlmProcessor::custom(
            "http://localhost:11434/v1".to_string(),
            "".to_string(),
            Some("llama3".to_string()),
        );
        assert_eq!(processor.model, "llama3");
        assert_eq!(processor.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn test_model_override() {
        let processor = HttpLlmProcessor::openai("key".to_string(), Some("gpt-4o".to_string()));
        assert_eq!(processor.model, "gpt-4o");
    }

    #[test]
    fn test_empty_model_uses_default() {
        let processor = HttpLlmProcessor::openai("key".to_string(), Some("".to_string()));
        assert_eq!(processor.model, OPENAI_DEFAULT_MODEL);
    }

    #[tokio::test]
    async fn test_health_check_with_key() {
        let processor = HttpLlmProcessor::openai("sk-test".to_string(), None);
        assert!(processor.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check_without_key() {
        let processor = HttpLlmProcessor::openai("".to_string(), None);
        assert!(!processor.health_check().await.unwrap());
    }

    #[test]
    fn test_extract_openai_response() {
        let processor = HttpLlmProcessor::openai("key".to_string(), None);
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "Hello world"
                }
            }]
        });
        assert_eq!(processor.extract_response(&json).unwrap(), "Hello world");
    }

    #[test]
    fn test_extract_claude_response() {
        let processor = HttpLlmProcessor::claude("key".to_string(), None);
        let json = serde_json::json!({
            "content": [{
                "type": "text",
                "text": "Hello world"
            }]
        });
        assert_eq!(processor.extract_response(&json).unwrap(), "Hello world");
    }

    #[test]
    fn test_extract_gemini_response() {
        let processor = HttpLlmProcessor::gemini_api("key".to_string(), None);
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Hello world"
                    }]
                }
            }]
        });
        assert_eq!(processor.extract_response(&json).unwrap(), "Hello world");
    }
}
