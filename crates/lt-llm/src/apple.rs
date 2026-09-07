use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::{Duration, Instant};

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
    ) -> *mut std::ffi::c_void;
    fn llm_bridge_cancel(handle: *mut std::ffi::c_void);
}

type Callback = extern "C" fn(*mut std::ffi::c_void, *const c_char);
type CancelRequest = unsafe extern "C" fn(*mut std::ffi::c_void);

struct AppleRequest {
    handle: *mut std::ffi::c_void,
    cancel: CancelRequest,
}

// The opaque Swift handle is exclusively owned here. Cancellation can be
// called from any thread, and the model task owns its inputs/context separately.
unsafe impl Send for AppleRequest {}

impl Drop for AppleRequest {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.cancel)(self.handle) };
        }
    }
}

async fn run_request(
    instructions: CString,
    prompt: CString,
    start: impl FnOnce(
        *const c_char,
        *const c_char,
        *mut std::ffi::c_void,
        Callback,
        Callback,
    ) -> *mut std::ffi::c_void,
    cancel: CancelRequest,
) -> Result<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let context = Box::new(LlmCallbackContext { result_tx: tx });
    // The bridge copies the C strings before returning. The callback owns and
    // releases this context, even if timeout/cancellation drops the receiver.
    let request = AppleRequest {
        handle: start(
            instructions.as_ptr(),
            prompt.as_ptr(),
            Box::into_raw(context).cast(),
            on_complete,
            on_error,
        ),
        cancel,
    };
    let result = tokio::time::timeout(Duration::from_secs(30), rx)
        .await
        .map_err(|_| MurmurError::Llm("Apple LLM timed out (30s).".to_string()))?
        .map_err(|_| {
            MurmurError::Llm("Apple LLM callback channel closed unexpectedly".to_string())
        })?;
    drop(request);
    result
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

    /// Create processor with a shared PromptManager (model parameter ignored).
    pub fn with_model_and_prompts(_model: Option<String>, prompts: PromptManager) -> Self {
        Self {
            prompt_manager: prompts,
        }
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

        let prompt = self.prompt_manager.build_prompt(&task).await;

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

        let result = run_request(
            instructions,
            c_prompt,
            |instructions, prompt, ctx, complete, error| unsafe {
                llm_bridge_process(instructions, prompt, ctx, complete, error)
            },
            llm_bridge_cancel,
        )
        .await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    unsafe extern "C" fn cancel_fake_request(handle: *mut std::ffi::c_void) {
        let canceled = unsafe { Box::from_raw(handle.cast::<Arc<AtomicBool>>()) };
        canceled.store(true, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn delayed_callback_does_not_block_runtime_and_releases_handle() {
        let canceled = Arc::new(AtomicBool::new(false));
        let handle_state = canceled.clone();
        let request = run_request(
            CString::new("instructions").unwrap(),
            CString::new("copied prompt").unwrap(),
            move |_, prompt, context, complete, _| {
                // Like Swift, the fake bridge copies inputs before returning.
                let prompt = unsafe { CStr::from_ptr(prompt) }.to_owned();
                let context = context as usize;
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    complete(context as *mut _, prompt.as_ptr());
                });
                Box::into_raw(Box::new(handle_state)).cast()
            },
            cancel_fake_request,
        );
        let (result, ()) = tokio::join!(request, async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            assert!(!canceled.load(Ordering::SeqCst));
        });
        assert_eq!(result.unwrap(), "copied prompt");
        assert!(canceled.load(Ordering::SeqCst));
        assert_eq!(Arc::strong_count(&canceled), 1);
    }

    #[tokio::test]
    async fn abort_cancels_request_and_late_callback_remains_valid() {
        let canceled = Arc::new(AtomicBool::new(false));
        let handle_state = canceled.clone();
        let worker_state = canceled.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(run_request(
            CString::new("instructions").unwrap(),
            CString::new("prompt").unwrap(),
            move |_, _, context, _, error| {
                let context = context as usize;
                tokio::spawn(async move {
                    while !worker_state.load(Ordering::SeqCst) {
                        tokio::task::yield_now().await;
                    }
                    // The receiver is gone, but the callback still exclusively
                    // owns the context and must reclaim it exactly once.
                    let message = CString::new("canceled").unwrap();
                    error(context as *mut _, message.as_ptr());
                    let _ = finished_tx.send(());
                });
                let _ = started_tx.send(());
                Box::into_raw(Box::new(handle_state)).cast()
            },
            cancel_fake_request,
        ));
        started_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(1), finished_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(canceled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn synchronous_error_callback_needs_no_request_handle() {
        let result = run_request(
            CString::new("instructions").unwrap(),
            CString::new("prompt").unwrap(),
            |_, _, context, _, error| {
                let message = CString::new("unavailable").unwrap();
                error(context, message.as_ptr());
                std::ptr::null_mut()
            },
            cancel_fake_request,
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("unavailable"));
    }
}
