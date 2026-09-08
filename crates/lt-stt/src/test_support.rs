//! Test-only helpers shared across this crate's test modules.

use std::sync::{Arc, Mutex, OnceLock};

struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every test in this binary shares one capturing subscriber, so a test that
/// asserts on log content must look for its own unique markers.
pub(crate) fn captured_logs() -> Arc<Mutex<Vec<u8>>> {
    static LOGS: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || SharedWriter(writer.clone()))
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("no other global subscriber in the test binary");
        buffer
    })
    .clone()
}

pub(crate) fn logs_text(logs: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&logs.lock().unwrap()).into_owned()
}
