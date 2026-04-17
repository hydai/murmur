use crate::prompts::{PromptName, PromptSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Disk-backed storage for user prompt overrides. Overrides live at
/// `{config_dir}/prompts/{name}.md`. A missing file (or directory) means
/// "no override" — callers fall back to the compile-time default.
pub struct PromptStore;

impl PromptStore {
    pub const SUBDIR: &'static str = "prompts";

    pub fn dir(config_dir: &Path) -> PathBuf {
        config_dir.join(Self::SUBDIR)
    }

    pub fn file_for(config_dir: &Path, name: PromptName) -> PathBuf {
        Self::dir(config_dir).join(format!("{}.md", name.as_str()))
    }

    /// Load every override present on disk into a `PromptSet`.
    /// A missing directory is treated as "no overrides" and returns a default set.
    /// Individual missing files are skipped. Other I/O errors bubble up.
    pub fn load_all(config_dir: &Path) -> io::Result<PromptSet> {
        let mut set = PromptSet::default();
        let dir = Self::dir(config_dir);
        if !dir.exists() {
            return Ok(set);
        }
        for name in PromptName::ALL {
            let path = Self::file_for(config_dir, name);
            match fs::read_to_string(&path) {
                Ok(content) => set.set_override(name, content),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(set)
    }

    pub fn save(config_dir: &Path, name: PromptName, content: &str) -> io::Result<()> {
        let dir = Self::dir(config_dir);
        fs::create_dir_all(&dir)?;
        fs::write(Self::file_for(config_dir, name), content)
    }

    pub fn reset(config_dir: &Path, name: PromptName) -> io::Result<()> {
        match fs::remove_file(Self::file_for(config_dir, name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_all_missing_dir_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let set = PromptStore::load_all(tmp.path()).unwrap();
        for name in PromptName::ALL {
            assert!(!set.has_override(name));
            assert_eq!(set.get(name), name.default_template());
        }
    }

    #[test]
    fn save_then_load_round_trip_is_byte_exact() {
        let tmp = TempDir::new().unwrap();
        let content = "# Custom\n\nLine with {placeholder} here.\n";
        PromptStore::save(tmp.path(), PromptName::Shorten, content).unwrap();

        let set = PromptStore::load_all(tmp.path()).unwrap();
        assert!(set.has_override(PromptName::Shorten));
        assert_eq!(set.get(PromptName::Shorten), content);
    }

    #[test]
    fn reset_deletes_override_file() {
        let tmp = TempDir::new().unwrap();
        PromptStore::save(tmp.path(), PromptName::Translate, "OVERRIDE").unwrap();
        assert!(PromptStore::file_for(tmp.path(), PromptName::Translate).exists());

        PromptStore::reset(tmp.path(), PromptName::Translate).unwrap();
        assert!(!PromptStore::file_for(tmp.path(), PromptName::Translate).exists());

        let set = PromptStore::load_all(tmp.path()).unwrap();
        assert!(!set.has_override(PromptName::Translate));
    }

    #[test]
    fn reset_missing_file_is_noop() {
        let tmp = TempDir::new().unwrap();
        PromptStore::reset(tmp.path(), PromptName::PostProcess).unwrap();
    }

    #[test]
    fn overrides_are_independent_per_name() {
        let tmp = TempDir::new().unwrap();
        PromptStore::save(tmp.path(), PromptName::Shorten, "ONE").unwrap();
        PromptStore::save(tmp.path(), PromptName::Translate, "TWO").unwrap();

        let set = PromptStore::load_all(tmp.path()).unwrap();
        assert_eq!(set.get(PromptName::Shorten), "ONE");
        assert_eq!(set.get(PromptName::Translate), "TWO");
        assert!(!set.has_override(PromptName::ChangeTone));
        assert_eq!(
            set.get(PromptName::ChangeTone),
            PromptName::ChangeTone.default_template()
        );
    }
}
