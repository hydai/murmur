use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};

/// Keeps settings accessible after a startup configuration error without
/// silently selecting another provider. Recording rebuilds the chosen provider.
pub struct UnavailableProcessor(pub String);

#[async_trait]
impl LlmProcessor for UnavailableProcessor {
    async fn process(&self, _task: ProcessingTask) -> Result<ProcessingOutput> {
        Err(MurmurError::Llm(self.0.clone()))
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_startup_error_remains_unavailable_instead_of_using_another_provider() {
        let processor = UnavailableProcessor("provider needs configuration".into());
        assert!(!processor.health_check().await.unwrap());
        let error = processor
            .process(ProcessingTask::Shorten {
                text: "test".into(),
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("provider needs configuration"));
    }
}
