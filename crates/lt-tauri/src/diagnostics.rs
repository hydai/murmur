use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const MAX_DIAGNOSTIC_LOG_ENTRIES: usize = 200;

static DIAGNOSTIC_LOG_STORE: OnceLock<Arc<DiagnosticLogStore>> = OnceLock::new();

#[derive(Clone, Debug, serde::Serialize)]
pub struct DiagnosticLogEntry {
    pub timestamp_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl DiagnosticLogEntry {
    pub fn new(level: String, target: String, message: String) -> Self {
        Self {
            timestamp_ms: now_ms(),
            level,
            target,
            message: redact_sensitive(&message),
        }
    }
}

pub struct DiagnosticLogStore {
    capacity: usize,
    entries: Mutex<VecDeque<DiagnosticLogEntry>>,
}

impl DiagnosticLogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn push(&self, entry: DiagnosticLogEntry) {
        if self.capacity == 0 {
            return;
        }

        let mut entries = self.entries_guard();
        while entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    pub fn entries(&self) -> Vec<DiagnosticLogEntry> {
        self.entries_guard().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.entries_guard().clear();
    }

    fn entries_guard(&self) -> MutexGuard<'_, VecDeque<DiagnosticLogEntry>> {
        match self.entries.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Clone)]
pub struct DiagnosticLogLayer {
    store: Arc<DiagnosticLogStore>,
}

impl DiagnosticLogLayer {
    pub fn new(store: Arc<DiagnosticLogStore>) -> Self {
        Self { store }
    }
}

impl<S> Layer<S> for DiagnosticLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if !matches!(*metadata.level(), Level::ERROR | Level::WARN) {
            return;
        }

        let mut visitor = DiagnosticFieldVisitor::default();
        event.record(&mut visitor);

        let message = visitor.message();
        if message.trim().is_empty() {
            return;
        }

        self.store.push(DiagnosticLogEntry::new(
            metadata.level().to_string().to_lowercase(),
            metadata.target().to_string(),
            message,
        ));
    }
}

#[derive(Default)]
struct DiagnosticFieldVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl DiagnosticFieldVisitor {
    fn message(self) -> String {
        match (self.message, self.fields.is_empty()) {
            (Some(message), true) => message,
            (Some(message), false) => format!("{} ({})", message, self.fields.join(", ")),
            (None, _) => self.fields.join(", "),
        }
    }

    fn record_field(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = Some(value);
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }
}

impl Visit for DiagnosticFieldVisitor {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_field(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_field(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_field(field, value.to_string());
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_field(field, value.to_string());
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_field(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_field(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field, value.to_string());
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.record_field(field, format!("{value:?}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_field(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let value = trim_debug_string(&format!("{value:?}")).to_string();
        self.record_field(field, value);
    }
}

pub fn shared_diagnostic_log_store() -> Arc<DiagnosticLogStore> {
    DIAGNOSTIC_LOG_STORE
        .get_or_init(|| Arc::new(DiagnosticLogStore::new(MAX_DIAGNOSTIC_LOG_ENTRIES)))
        .clone()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn trim_debug_string(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value)
}

fn redact_sensitive(message: &str) -> String {
    let mut redacted = String::with_capacity(message.len());
    let mut rest = message;

    while let Some(index) = rest.find("Bearer ") {
        redacted.push_str(&rest[..index + "Bearer ".len()]);
        let token_start = index + "Bearer ".len();
        let token = &rest[token_start..];
        let token_len = token.find(char::is_whitespace).unwrap_or(token.len());
        redacted.push_str("[REDACTED]");
        rest = &token[token_len..];
    }

    redacted.push_str(rest);
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn diagnostic_log_store_keeps_recent_entries_and_clears() {
        let store = DiagnosticLogStore::new(2);

        store.push(DiagnosticLogEntry::new(
            "warn".to_string(),
            "lt_stt::custom".to_string(),
            "first".to_string(),
        ));
        store.push(DiagnosticLogEntry::new(
            "error".to_string(),
            "lt_pipeline".to_string(),
            "second".to_string(),
        ));
        store.push(DiagnosticLogEntry::new(
            "warn".to_string(),
            "lt_tauri".to_string(),
            "third".to_string(),
        ));

        let entries = store.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "second");
        assert_eq!(entries[1].message, "third");

        store.clear();
        assert!(store.entries().is_empty());
    }

    #[test]
    fn diagnostic_log_store_recovers_from_poisoned_lock() {
        let store = DiagnosticLogStore::new(2);

        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = store.entries.lock().expect("lock before poison");
            panic!("poison diagnostic log lock");
        }));

        store.push(DiagnosticLogEntry::new(
            "error".to_string(),
            "lt_tauri".to_string(),
            "after poison".to_string(),
        ));

        let entries = store.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "after poison");

        store.clear();
        assert!(store.entries().is_empty());
    }

    #[test]
    fn diagnostic_log_layer_records_typed_fields() {
        let store = Arc::new(DiagnosticLogStore::new(5));
        let subscriber =
            tracing_subscriber::registry().with(DiagnosticLogLayer::new(store.clone()));

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(
                target: "lt_stt::custom",
                error = "timeout",
                attempts = 2_u64,
                retry = true,
                "Custom STT request failed"
            );
        });

        let entries = store.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].target, "lt_stt::custom");
        assert_eq!(entries[0].level, "warn");
        assert!(entries[0].message.contains("Custom STT request failed"));
        assert!(entries[0].message.contains("error=timeout"));
        assert!(entries[0].message.contains("attempts=2"));
        assert!(entries[0].message.contains("retry=true"));
    }
}
