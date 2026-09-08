use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use lt_core::{AppConfig, HistoryEntry, MurmurError, PersonalDictionary, TranscriptionHistory};
use tokio::sync::Mutex;

pub(crate) trait FileData: Default + Send + 'static {
    fn load(path: &Path) -> lt_core::error::Result<Self>;
    fn save(&self, path: &Path) -> lt_core::error::Result<()>;
}

macro_rules! file_data {
    ($kind:ty) => {
        impl FileData for $kind {
            fn load(path: &Path) -> lt_core::error::Result<Self> {
                Self::load_from_file(path)
            }
            fn save(&self, path: &Path) -> lt_core::error::Result<()> {
                self.save_to_file(path)
            }
        }
    };
}
file_data!(AppConfig);
file_data!(PersonalDictionary);
file_data!(TranscriptionHistory);

/// One transaction queue per document, shared by commands and background tasks.
/// File I/O runs off the async workers. The guard moves into the blocking job so
/// cancelling the caller cannot unlock a write that is still in progress.
#[derive(Clone)]
pub(crate) struct FileStore<T> {
    path: PathBuf,
    gate: Arc<Mutex<()>>,
    kind: PhantomData<fn() -> T>,
}

impl<T: FileData> FileStore<T> {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            gate: Arc::new(Mutex::new(())),
            kind: PhantomData,
        }
    }

    fn load(path: &Path) -> Result<T, String> {
        match T::load(path) {
            Ok(value) => Ok(value),
            Err(MurmurError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(T::default())
            }
            Err(error) => Err(format!("Failed to load {}: {error}", path.display())),
        }
    }

    pub async fn read(&self) -> Result<T, String> {
        let guard = self.gate.clone().lock_owned().await;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            Self::load(&path)
        })
        .await
        .map_err(|error| format!("Storage task failed: {error}"))?
    }

    pub async fn update<F>(&self, update: F) -> Result<T, String>
    where
        F: FnOnce(&mut T) -> Result<(), String> + Send + 'static,
    {
        self.update_then(update, |_| {}).await
    }

    /// Like `update`, but runs `committed` only after the new contents are on
    /// disk, still inside the transaction.
    pub async fn update_then<F, G>(&self, update: F, committed: G) -> Result<T, String>
    where
        F: FnOnce(&mut T) -> Result<(), String> + Send + 'static,
        G: FnOnce(&T) + Send + 'static,
    {
        let guard = self.gate.clone().lock_owned().await;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let mut value = Self::load(&path)?;
            update(&mut value)?;
            value
                .save(&path)
                .map_err(|error| format!("Failed to save {}: {error}", path.display()))?;
            committed(&value);
            Ok(value)
        })
        .await
        .map_err(|error| format!("Storage task failed: {error}"))?
    }
}

/// History transactions plus a clear generation. Appends reach the store
/// through a queue, so an append tagged with a generation older than the
/// store's current one is dropped instead of resurrecting entries the user
/// cleared after the recording finished.
#[derive(Clone)]
pub(crate) struct HistoryStore {
    file: FileStore<TranscriptionHistory>,
    generation: Arc<AtomicU64>,
    enabled: Arc<AtomicBool>,
}

impl HistoryStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            file: FileStore::new(path),
            generation: Arc::new(AtomicU64::new(0)),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Mirrors `AppConfig::save_history`; appends are dropped while disabled.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub async fn read(&self) -> Result<TranscriptionHistory, String> {
        self.file.read().await
    }

    pub async fn update<F>(&self, update: F) -> Result<TranscriptionHistory, String>
    where
        F: FnOnce(&mut TranscriptionHistory) -> Result<(), String> + Send + 'static,
    {
        self.file.update(update).await
    }

    /// Take this when the recording finishes and pass it to `append`.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Appends unless history is disabled or was cleared after `generation`
    /// was taken.
    pub async fn append(&self, entry: HistoryEntry, generation: u64) -> Result<(), String> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Ok(());
        }
        let current = self.generation.clone();
        self.file
            .update(move |history| {
                if current.load(Ordering::SeqCst) == generation {
                    history.add_entry(entry);
                } else {
                    tracing::info!(
                        "Dropping a history entry produced before the history was cleared"
                    );
                }
                Ok(())
            })
            .await
            .map(|_| ())
    }

    /// The generation advances only once the cleared history is on disk, so a
    /// failed clear keeps queued appends from the previous generation.
    pub async fn clear(&self) -> Result<(), String> {
        let current = self.generation.clone();
        self.file
            .update_then(
                |history| {
                    history.clear();
                    Ok(())
                },
                move |_| {
                    current.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await
            .map(|_| ())
    }
}

#[derive(Clone)]
pub(crate) struct AppStore {
    pub config: FileStore<AppConfig>,
    pub dictionary: FileStore<PersonalDictionary>,
    pub history: HistoryStore,
}

impl AppStore {
    pub fn new(directory: PathBuf) -> Self {
        let config = directory.join("config.toml");
        let dictionary = directory.join("dictionary.json");
        let history = directory.join("history.json");
        // Atomic replacement writes owner-only files; documents written by
        // earlier releases may still be world-readable.
        for path in [&config, &dictionary, &history] {
            if let Err(error) = lt_core::persistence::restrict_to_owner(path) {
                tracing::warn!("Failed to restrict {}: {error}", path.display());
            }
        }
        Self {
            config: FileStore::new(config),
            dictionary: FileStore::new(dictionary),
            history: HistoryStore::new(history),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_config_updates_preserve_all_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        let mut tasks = Vec::new();
        for i in 0..20 {
            let config = store.config.clone();
            tasks.push(tokio::spawn(async move {
                config
                    .update(move |config| {
                        config
                            .api_keys
                            .insert(format!("provider-{i}"), "test".into());
                        Ok(())
                    })
                    .await
                    .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(store.config.read().await.unwrap().api_keys.len(), 20);
    }

    #[tokio::test]
    async fn corrupt_history_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, "broken json").unwrap();
        let store = AppStore::new(dir.path().to_owned());
        let result = store
            .history
            .update(|history| {
                history.clear();
                Ok(())
            })
            .await;
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "broken json");
    }

    #[tokio::test]
    async fn failed_mutation_preserves_previous_config() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        store
            .config
            .update(|config| {
                config.hotkey = "Ctrl+A".into();
                Ok(())
            })
            .await
            .unwrap();
        assert!(store
            .config
            .update(|config| {
                config.hotkey = "Ctrl+B".into();
                Err("registration failed".into())
            })
            .await
            .is_err());
        assert_eq!(store.config.read().await.unwrap().hotkey, "Ctrl+A");
    }

    #[tokio::test]
    async fn cancelling_caller_does_not_release_an_active_write() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first_store = store.config.clone();
        let first = tokio::spawn(async move {
            first_store
                .update(move |config| {
                    let _ = started_tx.send(());
                    release_rx.recv().unwrap();
                    config.api_keys.insert("first".into(), "test".into());
                    Ok(())
                })
                .await
        });
        started_rx.await.unwrap();
        first.abort();
        let _ = first.await;

        let second_store = store.config.clone();
        let mut second = tokio::spawn(async move {
            second_store
                .update(|config| {
                    config.api_keys.insert("second".into(), "test".into());
                    Ok(())
                })
                .await
        });
        let was_blocked = tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
            .await
            .is_err();
        // Release before asserting so an assertion failure cannot strand a blocking job.
        release_tx.send(()).unwrap();
        assert!(was_blocked);
        second.await.unwrap().unwrap();
        assert_eq!(store.config.read().await.unwrap().api_keys.len(), 2);
    }

    #[tokio::test]
    async fn append_queued_before_a_clear_does_not_resurrect() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let blocker = store.history.clone();
        let blocking = tokio::spawn(async move {
            blocker
                .update(move |_| {
                    let _ = started_tx.send(());
                    release_rx.recv().unwrap();
                    Ok(())
                })
                .await
        });
        started_rx.await.unwrap();

        // The recording finished while the store was busy; the user cleared the
        // history before the queued append could acquire the store.
        let generation = store.history.generation();
        let clearing = {
            let history = store.history.clone();
            tokio::spawn(async move { history.clear().await })
        };
        tokio::task::yield_now().await;
        let appending = {
            let history = store.history.clone();
            tokio::spawn(async move {
                history
                    .append(
                        lt_core::HistoryEntry::new("late".into(), None, 0, None),
                        generation,
                    )
                    .await
            })
        };

        release_tx.send(()).unwrap();
        blocking.await.unwrap().unwrap();
        clearing.await.unwrap().unwrap();
        appending.await.unwrap().unwrap();
        assert!(store.history.read().await.unwrap().entries.is_empty());
    }

    #[tokio::test]
    async fn append_after_a_clear_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        store.history.clear().await.unwrap();
        let generation = store.history.generation();
        store
            .history
            .append(
                lt_core::HistoryEntry::new("fresh".into(), None, 0, None),
                generation,
            )
            .await
            .unwrap();
        assert_eq!(store.history.read().await.unwrap().entries.len(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_clear_keeps_the_generation_and_queued_appends() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        store
            .history
            .append(
                lt_core::HistoryEntry::new("kept".into(), None, 0, None),
                store.history.generation(),
            )
            .await
            .unwrap();
        let generation = store.history.generation();

        // A read-only directory makes the atomic replacement fail after the
        // in-memory clear succeeded.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let cleared = store.history.clear().await;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(cleared.is_err());

        store
            .history
            .append(
                lt_core::HistoryEntry::new("late".into(), None, 0, None),
                generation,
            )
            .await
            .unwrap();
        let texts: Vec<String> = store
            .history
            .read()
            .await
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.final_text.clone())
            .collect();
        assert_eq!(texts, ["late", "kept"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn opening_the_store_restricts_legacy_files_to_the_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        for name in ["config.toml", "history.json", "dictionary.json"] {
            let path = dir.path().join(name);
            std::fs::write(&path, "").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let _store = AppStore::new(dir.path().to_owned());
        for name in ["config.toml", "history.json", "dictionary.json"] {
            let mode = std::fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{name} mode was {mode:o}");
        }
    }

    #[tokio::test]
    async fn appends_are_skipped_while_history_is_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path().to_owned());
        store.history.set_enabled(false);
        store
            .history
            .append(
                lt_core::HistoryEntry::new("private".into(), None, 0, None),
                store.history.generation(),
            )
            .await
            .unwrap();
        assert!(store.history.read().await.unwrap().entries.is_empty());

        store.history.set_enabled(true);
        store
            .history
            .append(
                lt_core::HistoryEntry::new("kept".into(), None, 0, None),
                store.history.generation(),
            )
            .await
            .unwrap();
        assert_eq!(store.history.read().await.unwrap().entries.len(), 1);
    }
}
