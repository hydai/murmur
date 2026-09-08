use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// FFI declarations — must match crates/lt-stt-apple/Sources/.../speech_bridge.h
// ---------------------------------------------------------------------------

type SpeechTranscriptionCallback = unsafe extern "C" fn(
    ctx: *mut std::ffi::c_void,
    text: *const std::ffi::c_char,
    timestamp_ms: u64,
    is_final: bool,
);

type SpeechErrorCallback =
    unsafe extern "C" fn(ctx: *mut std::ffi::c_void, message: *const std::ffi::c_char);

type SpeechModelProgressCallback =
    unsafe extern "C" fn(ctx: *mut std::ffi::c_void, progress: f64, finished: bool);

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechModelStatus {
    Installed = 0,
    NotInstalled = 1,
    Downloading = 2,
    Unavailable = 3,
}

impl From<i32> for SpeechModelStatus {
    fn from(v: i32) -> Self {
        match v {
            0 => SpeechModelStatus::Installed,
            1 => SpeechModelStatus::NotInstalled,
            2 => SpeechModelStatus::Downloading,
            3 => SpeechModelStatus::Unavailable,
            _ => SpeechModelStatus::Unavailable,
        }
    }
}

extern "C" {
    fn speech_bridge_is_available() -> bool;
    fn speech_bridge_get_supported_locales() -> *mut std::ffi::c_char;
    fn speech_bridge_check_model_status(locale: *const std::ffi::c_char) -> i32;
    fn speech_bridge_download_model(
        locale: *const std::ffi::c_char,
        ctx: *mut std::ffi::c_void,
        callback: SpeechModelProgressCallback,
    );
    fn speech_bridge_create_session(
        locale: *const std::ffi::c_char,
        ctx: *mut std::ffi::c_void,
        on_transcription: SpeechTranscriptionCallback,
        on_error: SpeechErrorCallback,
    ) -> *mut std::ffi::c_void;
    fn speech_bridge_send_audio(
        session: *mut std::ffi::c_void,
        samples: *const i16,
        count: usize,
        timestamp_ms: u64,
    ) -> bool;
    fn speech_bridge_stop_session(session: *mut std::ffi::c_void);
    fn speech_bridge_destroy_session(session: *mut std::ffi::c_void);
    fn speech_bridge_free_string(ptr: *mut std::ffi::c_char);
}

// ---------------------------------------------------------------------------
// Safe wrappers for static FFI functions
// ---------------------------------------------------------------------------

/// Check if Apple SpeechTranscriber is available on this system.
pub fn is_available() -> bool {
    unsafe { speech_bridge_is_available() }
}

/// Get the list of supported locale identifiers (e.g. "en_US", "ja_JP").
pub fn get_supported_locales() -> Vec<String> {
    unsafe {
        let ptr = speech_bridge_get_supported_locales();
        if ptr.is_null() {
            return vec![];
        }
        let c_str = CStr::from_ptr(ptr);
        let json = c_str.to_string_lossy().to_string();
        speech_bridge_free_string(ptr);
        serde_json::from_str(&json).unwrap_or_default()
    }
}

/// Check the model installation status for a locale.
pub fn check_model_status(locale: &str) -> SpeechModelStatus {
    let c_locale = match CString::new(locale) {
        Ok(s) => s,
        Err(_) => return SpeechModelStatus::Unavailable,
    };
    let raw = unsafe { speech_bridge_check_model_status(c_locale.as_ptr()) };
    SpeechModelStatus::from(raw)
}

// ---------------------------------------------------------------------------
// Callback context — lives on the heap for the session's lifetime
// ---------------------------------------------------------------------------

struct CallbackContext {
    event_tx: mpsc::Sender<TranscriptionEvent>,
}

/// Trampoline: called from Swift when a transcription result arrives.
unsafe extern "C" fn on_transcription(
    ctx: *mut std::ffi::c_void,
    text: *const std::ffi::c_char,
    timestamp_ms: u64,
    is_final: bool,
) {
    if ctx.is_null() || text.is_null() {
        return;
    }
    let cb = unsafe { &*(ctx as *const CallbackContext) };
    let text_str = unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .to_string();

    let event = if is_final {
        TranscriptionEvent::Committed {
            text: text_str,
            timestamp_ms,
        }
    } else {
        TranscriptionEvent::Partial {
            text: text_str,
            timestamp_ms,
        }
    };

    // Use blocking_send since we're called from Swift's thread (not tokio).
    if let Err(e) = cb.event_tx.blocking_send(event) {
        error!("Apple STT: failed to send transcription event: {}", e);
    }
}

/// Trampoline: called from Swift when an error occurs.
unsafe extern "C" fn on_error(ctx: *mut std::ffi::c_void, message: *const std::ffi::c_char) {
    if ctx.is_null() || message.is_null() {
        return;
    }
    let cb = unsafe { &*(ctx as *const CallbackContext) };
    let msg = unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .to_string();

    error!("Apple STT error: {}", msg);

    let event = TranscriptionEvent::Error { message: msg };
    if let Err(e) = cb.event_tx.blocking_send(event) {
        error!("Apple STT: failed to send error event: {}", e);
    }
}

// ---------------------------------------------------------------------------
// Model download callback
// ---------------------------------------------------------------------------

struct DownloadContext {
    progress_tx: mpsc::Sender<(f64, bool)>,
}

unsafe extern "C" fn on_model_progress(ctx: *mut std::ffi::c_void, progress: f64, finished: bool) {
    if ctx.is_null() {
        return;
    }
    let dl_ctx = unsafe { &*(ctx as *const DownloadContext) };
    // Reserve one slot for the terminal event. Progress may be coalesced, but
    // completion must be delivered even if the UI has not drained the queue.
    // This callback can also be invoked synchronously by an unavailable bridge,
    // so it must never use blocking_send on a Tokio runtime thread.
    if finished || dl_ctx.progress_tx.capacity() > 1 {
        let _ = dl_ctx.progress_tx.try_send((progress, finished));
    }
    if finished {
        // Swift guarantees exactly one terminal callback, after cancelling and
        // joining its progress task. No callback can use ctx after this point.
        drop(unsafe { Box::from_raw(ctx as *mut DownloadContext) });
    }
}

/// Download the speech model for a locale. Returns a channel that reports
/// (progress: 0.0-1.0, finished: bool).
pub fn download_model(locale: &str) -> mpsc::Receiver<(f64, bool)> {
    let (tx, rx) = mpsc::channel(32);
    let c_locale = CString::new(locale).unwrap_or_default();

    let dl_ctx = Box::new(DownloadContext { progress_tx: tx });
    let ctx_ptr = Box::into_raw(dl_ctx) as *mut std::ffi::c_void;

    unsafe {
        speech_bridge_download_model(c_locale.as_ptr(), ctx_ptr, on_model_progress);
    }

    rx
}

// ---------------------------------------------------------------------------
// AppleSttProvider — implements SttProvider trait
// ---------------------------------------------------------------------------

/// Take a lock without letting one panic disable the provider for good.
///
/// A poisoned mutex means some other call panicked while holding it, not that
/// the pointer or channel behind it is unusable. Unwrapping here would turn a
/// single failure into a permanent one — every later `start_session` would
/// panic too — and in `Drop` it would abort the process outright when the lock
/// was poisoned during an unwind. `lt-tauri`'s diagnostics store takes the same
/// view of its own log buffer.
fn guard<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Apple on-device speech-to-text provider using SpeechTranscriber (macOS 26+).
pub struct AppleSttProvider {
    locale: String,
    session: Mutex<*mut std::ffi::c_void>,
    // The callback context must outlive the session.
    callback_ctx: Mutex<Option<*mut CallbackContext>>,
    event_tx: Mutex<Option<mpsc::Sender<TranscriptionEvent>>>,
    event_rx: Mutex<Option<mpsc::Receiver<TranscriptionEvent>>>,
}

// Safety: The raw pointer in `session` is accessed through a Mutex.
// The pointer is only used by FFI calls that are themselves thread-safe.
unsafe impl Send for AppleSttProvider {}
unsafe impl Sync for AppleSttProvider {}

impl AppleSttProvider {
    /// Create a new Apple STT provider for the given locale.
    /// Use "auto" to detect the system locale at runtime.
    pub fn new(locale: String) -> Self {
        Self {
            locale,
            session: Mutex::new(ptr::null_mut()),
            callback_ctx: Mutex::new(None),
            event_tx: Mutex::new(None),
            event_rx: Mutex::new(None),
        }
    }

    /// Resolve "auto" to the system's primary language, or validate a specific locale.
    fn resolve_locale(&self) -> String {
        if self.locale == "auto" {
            // Get system language and try to match against supported locales.
            let system_locale = sys_locale::get_locale().unwrap_or_else(|| "en_US".to_string());
            // Normalize: sys_locale may return "en-US", SpeechTranscriber wants "en_US".
            let normalized = system_locale.replace('-', "_");
            debug!("Auto-detected system locale: {}", normalized);
            normalized
        } else {
            self.locale.clone()
        }
    }
}

#[async_trait]
impl SttProvider for AppleSttProvider {
    async fn start_session(&mut self) -> Result<()> {
        let locale = self.resolve_locale();
        info!("Starting Apple STT session with locale: {}", locale);

        let c_locale = CString::new(locale.as_str())
            .map_err(|e| MurmurError::Stt(format!("Invalid locale string: {}", e)))?;

        // Create event channels.
        let (event_tx, event_rx) = mpsc::channel::<TranscriptionEvent>(64);
        *guard(&self.event_tx) = Some(event_tx.clone());
        *guard(&self.event_rx) = Some(event_rx);

        // Allocate callback context on the heap.
        let ctx = Box::new(CallbackContext { event_tx });
        let ctx_ptr = Box::into_raw(ctx);
        *guard(&self.callback_ctx) = Some(ctx_ptr);

        // Create the Swift session.
        let session_ptr = unsafe {
            speech_bridge_create_session(
                c_locale.as_ptr(),
                ctx_ptr as *mut std::ffi::c_void,
                on_transcription,
                on_error,
            )
        };

        if session_ptr.is_null() {
            // Clean up the callback context since session creation failed.
            unsafe {
                let _ = Box::from_raw(ctx_ptr);
            }
            *guard(&self.callback_ctx) = None;
            return Err(MurmurError::Stt(
                "Failed to create Apple STT session. Is macOS 26+ and the speech model installed?"
                    .to_string(),
            ));
        }

        *guard(&self.session) = session_ptr;
        info!("Apple STT session started");
        Ok(())
    }

    async fn send_audio(&mut self, chunk: AudioChunk) -> Result<()> {
        let session = *guard(&self.session);
        if session.is_null() {
            return Err(MurmurError::Stt("Session not started".to_string()));
        }

        let ok = unsafe {
            speech_bridge_send_audio(
                session,
                chunk.data.as_ptr(),
                chunk.data.len(),
                chunk.timestamp_ms,
            )
        };

        if !ok {
            warn!("Apple STT: failed to send audio chunk");
        }
        Ok(())
    }

    async fn stop_session(&mut self) -> Result<()> {
        info!("Stopping Apple STT session");

        let session = {
            let mut session = guard(&self.session);
            let s = *session;
            *session = ptr::null_mut();
            s
        };

        let callback_ctx = guard(&self.callback_ctx).take().map(|p| p as usize);
        let session = session as usize;
        // Swift finalization waits for callbacks. Keep Tokio workers available
        // to consume those callbacks, and move ownership into the blocking job
        // so cancellation of this future cannot free an in-use context.
        tokio::task::spawn_blocking(move || destroy_session(session, callback_ctx))
            .await
            .map_err(|e| MurmurError::Stt(format!("Apple STT shutdown failed: {e}")))?;
        guard(&self.event_tx).take();

        info!("Apple STT session stopped");
        Ok(())
    }

    async fn subscribe_events(&self) -> mpsc::Receiver<TranscriptionEvent> {
        guard(&self.event_rx)
            .take()
            .expect("subscribe_events called multiple times or before start_session")
    }
}

fn destroy_session(session: usize, callback_ctx: Option<usize>) {
    if session != 0 {
        unsafe {
            speech_bridge_stop_session(session as *mut std::ffi::c_void);
            speech_bridge_destroy_session(session as *mut std::ffi::c_void);
        }
    }
    if let Some(ctx) = callback_ctx {
        drop(unsafe { Box::from_raw(ctx as *mut CallbackContext) });
    }
}

impl Drop for AppleSttProvider {
    fn drop(&mut self) {
        let session = *guard(&self.session) as usize;
        let ctx = guard(&self.callback_ctx).take().map(|p| p as usize);
        if session != 0 || ctx.is_some() {
            // Drop may run on a Tokio worker or after its runtime has stopped.
            // A dedicated thread owns the FFI resources until callbacks finish.
            std::thread::spawn(move || destroy_session(session, ctx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_does_not_disable_the_provider() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let provider = AppleSttProvider::new("en_US".to_string());

        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _held = provider.session.lock().expect("lock before poison");
            panic!("poison the session lock");
        }));
        assert!(provider.session.is_poisoned());

        // Reaching the pointer again must still work: one panic elsewhere is
        // not a reason to refuse every later session.
        assert!(guard(&provider.session).is_null());

        // A real address, not a fabricated one, and cleared again before drop
        // so nothing hands it to the bridge.
        let mut sentinel = 0u8;
        let marker = std::ptr::from_mut(&mut sentinel) as *mut std::ffi::c_void;
        *guard(&provider.session) = marker;
        assert_eq!(*guard(&provider.session), marker);
        *guard(&provider.session) = ptr::null_mut();
    }

    #[test]
    fn dropping_a_provider_with_poisoned_locks_does_not_abort() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let provider = AppleSttProvider::new("en_US".to_string());
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _held = provider.callback_ctx.lock().expect("lock before poison");
            panic!("poison the callback context lock");
        }));
        assert!(provider.callback_ctx.is_poisoned());

        // Drop runs the FFI teardown; panicking there during an unwind would
        // abort the process rather than surface an error.
        drop(provider);
    }

    #[tokio::test]
    async fn download_completion_reclaims_context_and_preserves_terminal_event() {
        let (tx, mut rx) = mpsc::channel(4);
        let ctx =
            Box::into_raw(Box::new(DownloadContext { progress_tx: tx })) as *mut std::ffi::c_void;
        for _ in 0..100 {
            unsafe { on_model_progress(ctx, 0.5, false) };
        }
        unsafe { on_model_progress(ctx, 1.0, true) };
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        assert_eq!(events.len(), 4);
        assert_eq!(events.last(), Some(&(1.0, true)));
    }

    #[test]
    fn download_completion_is_safe_when_ui_has_gone_away() {
        let (tx, rx) = mpsc::channel(4);
        drop(rx);
        let ctx =
            Box::into_raw(Box::new(DownloadContext { progress_tx: tx })) as *mut std::ffi::c_void;
        unsafe { on_model_progress(ctx, 0.0, true) };
    }
}
