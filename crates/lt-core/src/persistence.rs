use std::io::{self, Write};
use std::path::Path;

/// Replace a file only after all of its new contents have been written.
/// The temporary file lives beside the destination so the rename is atomic.
/// Readers see either complete version, and a failed write preserves the old one.
pub fn atomic_write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(contents.as_ref())?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn concurrent_readers_only_observe_complete_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.json");
        let first = "a".repeat(64 * 1024);
        let second = "b".repeat(128 * 1024);
        atomic_write(&path, &first).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        std::thread::scope(|scope| {
            let writer_done = done.clone();
            let path = &path;
            let first = &first;
            let second = &second;
            scope.spawn(move || {
                for _ in 0..20 {
                    atomic_write(path, second).unwrap();
                    atomic_write(path, first).unwrap();
                }
                writer_done.store(true, Ordering::Release);
            });
            while !done.load(Ordering::Acquire) {
                let contents = std::fs::read_to_string(path).unwrap();
                assert!(contents == *first || contents == *second);
            }
        });
    }

    #[test]
    fn failed_replace_cleans_up_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("existing-directory");
        std::fs::create_dir(&destination).unwrap();
        assert!(atomic_write(&destination, "new contents").is_err());
        assert!(destination.is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
