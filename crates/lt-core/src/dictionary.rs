use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

/// Dictionary entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    /// The correct term
    pub term: String,
    /// Aliases or common misspellings
    pub aliases: Vec<String>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Personal dictionary for custom terms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalDictionary {
    pub entries: Vec<DictionaryEntry>,
}

impl PersonalDictionary {
    /// Create a new empty dictionary
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Load dictionary from JSON file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let dict: PersonalDictionary = serde_json::from_str(&content)?;
        Ok(dict)
    }

    /// Save dictionary to JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add an entry
    pub fn add_entry(&mut self, entry: DictionaryEntry) {
        self.entries.push(entry);
    }

    /// Remove an entry by term
    pub fn remove_entry(&mut self, term: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.term == term) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get all terms
    pub fn get_terms(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.term.clone()).collect()
    }
}

impl Default for PersonalDictionary {
    fn default() -> Self {
        Self::new()
    }
}
