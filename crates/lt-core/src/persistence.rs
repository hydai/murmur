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

/// Make an existing file readable and writable by its owner only. Files
/// written before atomic replacement was introduced may still be 0644;
/// missing files are ignored.
pub fn restrict_to_owner(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => {
                let mut permissions = metadata.permissions();
                if permissions.mode() & 0o077 != 0 {
                    permissions.set_mode(0o600);
                    std::fs::set_permissions(path, permissions)?;
                }
                Ok(())
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
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

    #[cfg(unix)]
    #[test]
    fn atomic_write_creates_owner_only_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        atomic_write(&path, "new").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode was {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn restrict_to_owner_tightens_existing_files_and_ignores_missing_ones() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, "[]").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        restrict_to_owner(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode was {mode:o}");
        restrict_to_owner(dir.path().join("missing.json")).unwrap();
    }
}
