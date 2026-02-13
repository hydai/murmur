use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Mutex;
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

type SpeechErrorCallback = unsafe extern "C" fn(
    ctx: *mut std::ffi::c_void,
    message: *const std::ffi::c_char,
);

type SpeechModelProgressCallback = unsafe extern "C" fn(
    ctx: *mut std::ffi::c_void,
    progress: f64,
    finished: bool,
);

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

unsafe extern "C" fn on_model_progress(
    ctx: *mut std::ffi::c_void,
    progress: f64,
    finished: bool,
) {
    if ctx.is_null() {
        return;
    }
    let dl_ctx = unsafe { &*(ctx as *const DownloadContext) };
    let _ = dl_ctx.progress_tx.blocking_send((progress, finished));

    // If finished, the context will be cleaned up by the caller.
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

    // The context will leak if the Swift side finishes and we don't reclaim it.
    // Spawn a task that waits for the "finished" signal, then cleans up.
    let ctx_raw = ctx_ptr as usize; // safe to send across threads
    tokio::spawn(async move {
        // Wait a reasonable time for download to finish.
        tokio::time::sleep(tokio::time::Duration::from_secs(600)).await;
        // Safety: reclaim the Box to avoid leak if Swift never sent "finished".
        unsafe {
            let _ = Box::from_raw(ctx_raw as *mut DownloadContext);
        }
    });

    rx
}

// ---------------------------------------------------------------------------
// AppleSttProvider — implements SttProvider trait
// ---------------------------------------------------------------------------

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
        *self.event_tx.lock().unwrap() = Some(event_tx.clone());
        *self.event_rx.lock().unwrap() = Some(event_rx);

        // Allocate callback context on the heap.
        let ctx = Box::new(CallbackContext { event_tx });
        let ctx_ptr = Box::into_raw(ctx);
        *self.callback_ctx.lock().unwrap() = Some(ctx_ptr);

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
            *self.callback_ctx.lock().unwrap() = None;
            return Err(MurmurError::Stt(
                "Failed to create Apple STT session. Is macOS 26+ and the speech model installed?"
                    .to_string(),
            ));
        }

        *self.session.lock().unwrap() = session_ptr;
        info!("Apple STT session started");
        Ok(())
    }

    async fn send_audio(&mut self, chunk: AudioChunk) -> Result<()> {
        let session = *self.session.lock().unwrap();
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
            let mut guard = self.session.lock().unwrap();
            let s = *guard;
            *guard = ptr::null_mut();
            s
        };

        if !session.is_null() {
            unsafe {
                speech_bridge_stop_session(session);
                speech_bridge_destroy_session(session);
            }
        }

        // Reclaim the callback context.
        if let Some(ctx_ptr) = self.callback_ctx.lock().unwrap().take() {
            unsafe {
                let _ = Box::from_raw(ctx_ptr);
            }
        }

        info!("Apple STT session stopped");
        Ok(())
    }

    async fn subscribe_events(&self) -> mpsc::Receiver<TranscriptionEvent> {
        self.event_rx
            .lock()
            .unwrap()
            .take()
            .expect("subscribe_events called multiple times or before start_session")
    }
}

impl Drop for AppleSttProvider {
    fn drop(&mut self) {
        // Ensure cleanup if the provider is dropped without stopping.
        let session = *self.session.lock().unwrap();
        if !session.is_null() {
            warn!("AppleSttProvider dropped without stop_session — cleaning up");
            unsafe {
                speech_bridge_stop_session(session);
                speech_bridge_destroy_session(session);
            }
        }

        if let Some(ctx_ptr) = self.callback_ctx.lock().unwrap().take() {
            unsafe {
                let _ = Box::from_raw(ctx_ptr);
            }
        }
    }
}
