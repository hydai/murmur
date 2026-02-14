use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

const DEFAULT_MAX_ENTRIES: usize = 500;

/// A single transcription history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique identifier (timestamp_ms as string)
    pub id: String,
    /// LLM-processed final text
    pub final_text: String,
    /// Raw transcription before LLM processing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_text: Option<String>,
    /// When the transcription completed (epoch ms)
    pub timestamp_ms: u64,
    /// LLM processing duration in ms
    pub processing_time_ms: u64,
    /// Voice command used, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_name: Option<String>,
}

/// Persistent transcription history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionHistory {
    pub entries: Vec<HistoryEntry>,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_max_entries() -> usize {
    DEFAULT_MAX_ENTRIES
}

impl TranscriptionHistory {
    /// Create a new empty history
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Load history from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let history: TranscriptionHistory = serde_json::from_str(&content)?;
        Ok(history)
    }

    /// Save history to a JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add an entry (prepend so newest is first), pruning if over capacity
    pub fn add_entry(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Delete an entry by id
    pub fn delete_entry(&mut self, id: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Search entries by text content (case-insensitive partial match)
    pub fn search_entries(&self, query: &str) -> Vec<HistoryEntry> {
        if query.is_empty() {
            return self.entries.clone();
        }

        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.final_text.to_lowercase().contains(&query_lower)
                    || e.raw_text
                        .as_ref()
                        .is_some_and(|r| r.to_lowercase().contains(&query_lower))
                    || e.command_name
                        .as_ref()
                        .is_some_and(|c| c.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }
}

impl Default for TranscriptionHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, text: &str) -> HistoryEntry {
        HistoryEntry {
            id: id.to_string(),
            final_text: text.to_string(),
            raw_text: None,
            timestamp_ms: id.parse().unwrap_or(0),
            processing_time_ms: 100,
            command_name: None,
        }
    }

    #[test]
    fn test_add_entry_prepends() {
        let mut history = TranscriptionHistory::new();
        history.add_entry(make_entry("1000", "first"));
        history.add_entry(make_entry("2000", "second"));

        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0].id, "2000");
        assert_eq!(history.entries[1].id, "1000");
    }

    #[test]
    fn test_add_entry_prunes_over_capacity() {
        let mut history = TranscriptionHistory::new();
        history.max_entries = 3;

        for i in 0..5 {
            history.add_entry(make_entry(&i.to_string(), &format!("entry {i}")));
        }

        assert_eq!(history.entries.len(), 3);
        // Newest entries are kept
        assert_eq!(history.entries[0].id, "4");
        assert_eq!(history.entries[1].id, "3");
        assert_eq!(history.entries[2].id, "2");
    }

    #[test]
    fn test_delete_entry() {
        let mut history = TranscriptionHistory::new();
        history.add_entry(make_entry("1000", "hello"));
        history.add_entry(make_entry("2000", "world"));

        assert!(history.delete_entry("1000"));
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].id, "2000");
    }

    #[test]
    fn test_delete_nonexistent_entry() {
        let mut history = TranscriptionHistory::new();
        assert!(!history.delete_entry("9999"));
    }

    #[test]
    fn test_clear() {
        let mut history = TranscriptionHistory::new();
        history.add_entry(make_entry("1", "a"));
        history.add_entry(make_entry("2", "b"));
        history.clear();
        assert!(history.entries.is_empty());
    }

    #[test]
    fn test_search_entries_empty_query() {
        let mut history = TranscriptionHistory::new();
        history.add_entry(make_entry("1", "hello"));
        history.add_entry(make_entry("2", "world"));

        let results = history.search_entries("");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_entries_by_final_text() {
        let mut history = TranscriptionHistory::new();
        history.add_entry(make_entry("1", "Hello world"));
        history.add_entry(make_entry("2", "Goodbye moon"));

        let results = history.search_entries("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].final_text, "Hello world");
    }

    #[test]
    fn test_search_entries_by_raw_text() {
        let mut history = TranscriptionHistory::new();
        let mut entry = make_entry("1", "processed text");
        entry.raw_text = Some("original raw input".to_string());
        history.add_entry(entry);

        let results = history.search_entries("original");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_entries_by_command_name() {
        let mut history = TranscriptionHistory::new();
        let mut entry = make_entry("1", "some text");
        entry.command_name = Some("dictate".to_string());
        history.add_entry(entry);

        let results = history.search_entries("dictate");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_roundtrip_file() {
        let dir = std::env::temp_dir().join("murmur_test_history");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("history.json");

        let mut history = TranscriptionHistory::new();
        history.add_entry(make_entry("1000", "saved text"));
        history.save_to_file(&path).unwrap();

        let loaded = TranscriptionHistory::load_from_file(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].final_text, "saved text");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
