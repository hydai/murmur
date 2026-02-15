use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::Instant;

use crate::prompts::PromptManager;

// FFI declarations matching llm_bridge.h
extern "C" {
    fn llm_bridge_is_available() -> bool;
    fn llm_bridge_process(
        instructions: *const c_char,
        prompt: *const c_char,
        ctx: *mut std::ffi::c_void,
        on_complete: extern "C" fn(*mut std::ffi::c_void, *const c_char),
        on_error: extern "C" fn(*mut std::ffi::c_void, *const c_char),
    );
}

/// Callback context for receiving LLM results via FFI.
/// Heap-allocated, passed as opaque pointer, reclaimed after callback fires.
struct LlmCallbackContext {
    result_tx: tokio::sync::oneshot::Sender<Result<String>>,
}

/// Completion callback trampoline — sends Ok(text) through the oneshot channel.
extern "C" fn on_complete(ctx: *mut std::ffi::c_void, text: *const c_char) {
    if ctx.is_null() {
        return;
    }
    let context = unsafe { Box::from_raw(ctx as *mut LlmCallbackContext) };
    let result = if text.is_null() {
        Ok(String::new())
    } else {
        let c_str = unsafe { CStr::from_ptr(text) };
        Ok(c_str.to_string_lossy().into_owned())
    };
    let _ = context.result_tx.send(result);
}

/// Error callback trampoline — sends Err through the oneshot channel.
extern "C" fn on_error(ctx: *mut std::ffi::c_void, message: *const c_char) {
    if ctx.is_null() {
        return;
    }
    let context = unsafe { Box::from_raw(ctx as *mut LlmCallbackContext) };
    let msg = if message.is_null() {
        "Unknown Apple LLM error".to_string()
    } else {
        let c_str = unsafe { CStr::from_ptr(message) };
        c_str.to_string_lossy().into_owned()
    };
    let _ = context.result_tx.send(Err(MurmurError::Llm(msg)));
}

/// Apple Foundation Models LLM processor — on-device, privacy-first.
pub struct AppleLlmProcessor {
    prompt_manager: PromptManager,
}

pub const DEFAULT_MODEL: &str = "(system default)";

impl AppleLlmProcessor {
    pub fn new() -> Self {
        Self {
            prompt_manager: PromptManager::new(),
        }
    }

    /// Create processor (model parameter ignored — Apple uses the system model).
    pub fn with_model(_model: Option<String>) -> Self {
        Self::new()
    }

    /// Check if Apple Foundation Models is available (static, no instance needed).
    pub fn is_available() -> bool {
        unsafe { llm_bridge_is_available() }
    }
}

impl Default for AppleLlmProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProcessor for AppleLlmProcessor {
    async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
        let start_time = Instant::now();

        let prompt = self.prompt_manager.build_prompt(&task);

        tracing::debug!(
            "Apple LLM processing prompt (length: {} chars)",
            prompt.len()
        );

        let instructions = CString::new(
            "You are a text processing assistant. Return only the processed text, no explanations.",
        )
        .map_err(|e| MurmurError::Llm(format!("Invalid instructions string: {}", e)))?;

        let c_prompt = CString::new(prompt)
            .map_err(|e| MurmurError::Llm(format!("Invalid prompt string: {}", e)))?;

        // Create a oneshot channel for receiving the result from the callback.
        let (tx, rx) = tokio::sync::oneshot::channel();

        let context = Box::new(LlmCallbackContext { result_tx: tx });
        let ctx_ptr = Box::into_raw(context) as *mut std::ffi::c_void;

        // Call the Swift FFI bridge. This blocks until the LLM responds,
        // but we're already on a Tokio task so that's fine.
        unsafe {
            llm_bridge_process(
                instructions.as_ptr(),
                c_prompt.as_ptr(),
                ctx_ptr,
                on_complete,
                on_error,
            );
        }

        // The callback has already fired (llm_bridge_process is synchronous),
        // so the channel should have a value immediately.
        let result = rx.await.map_err(|_| {
            MurmurError::Llm("Apple LLM callback channel closed unexpectedly".to_string())
        })??;

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            "Apple LLM processing completed in {}ms (output length: {} chars)",
            processing_time_ms,
            result.len()
        );

        Ok(ProcessingOutput {
            text: result,
            processing_time_ms,
            metadata: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let available = Self::is_available();
        if available {
            tracing::info!("Apple Foundation Models is available");
        } else {
            tracing::warn!("Apple Foundation Models is not available on this system");
        }
        Ok(available)
    }
}
