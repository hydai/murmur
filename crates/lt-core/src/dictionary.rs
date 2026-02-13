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

    /// Update an entry by term
    pub fn update_entry(&mut self, old_term: &str, new_entry: DictionaryEntry) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.term == old_term) {
            self.entries[pos] = new_entry;
            true
        } else {
            false
        }
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

    /// Search entries by query (case-insensitive partial match)
    pub fn search_entries(&self, query: &str) -> Vec<DictionaryEntry> {
        if query.is_empty() {
            return self.entries.clone();
        }

        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.term.to_lowercase().contains(&query_lower)
                    || e.aliases.iter().any(|a| a.to_lowercase().contains(&query_lower))
                    || e.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_entry() {
        let mut dict = PersonalDictionary::new();
        assert_eq!(dict.entries.len(), 0);

        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec!["local type".to_string()],
            description: Some("A privacy-first voice typing app".to_string()),
        });

        assert_eq!(dict.entries.len(), 1);
        assert_eq!(dict.entries[0].term, "Localtype");
    }

    #[test]
    fn test_update_entry() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec!["local type".to_string()],
            description: None,
        });

        let updated = dict.update_entry("Localtype", DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec!["local type".to_string(), "local-type".to_string()],
            description: Some("Updated description".to_string()),
        });

        assert!(updated);
        assert_eq!(dict.entries[0].aliases.len(), 2);
        assert_eq!(dict.entries[0].description, Some("Updated description".to_string()));
    }

    #[test]
    fn test_update_nonexistent_entry() {
        let mut dict = PersonalDictionary::new();
        let updated = dict.update_entry("NonExistent", DictionaryEntry {
            term: "Test".to_string(),
            aliases: vec![],
            description: None,
        });

        assert!(!updated);
    }

    #[test]
    fn test_remove_entry() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec![],
            description: None,
        });

        assert_eq!(dict.entries.len(), 1);
        assert!(dict.remove_entry("Localtype"));
        assert_eq!(dict.entries.len(), 0);
    }

    #[test]
    fn test_remove_nonexistent_entry() {
        let mut dict = PersonalDictionary::new();
        assert!(!dict.remove_entry("NonExistent"));
    }

    #[test]
    fn test_search_entries_empty_query() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec![],
            description: None,
        });
        dict.add_entry(DictionaryEntry {
            term: "BYOK".to_string(),
            aliases: vec![],
            description: None,
        });

        let results = dict.search_entries("");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_entries_by_term() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec!["local type".to_string()],
            description: None,
        });
        dict.add_entry(DictionaryEntry {
            term: "BYOK".to_string(),
            aliases: vec!["bee yok".to_string()],
            description: None,
        });

        let results = dict.search_entries("local");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].term, "Localtype");
    }

    #[test]
    fn test_search_entries_by_alias() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec!["local type".to_string()],
            description: None,
        });
        dict.add_entry(DictionaryEntry {
            term: "BYOK".to_string(),
            aliases: vec!["bee yok".to_string()],
            description: None,
        });

        let results = dict.search_entries("bee");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].term, "BYOK");
    }

    #[test]
    fn test_search_entries_case_insensitive() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec!["local type".to_string()],
            description: None,
        });

        let results = dict.search_entries("LOCAL");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].term, "Localtype");
    }

    #[test]
    fn test_search_entries_by_description() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec![],
            description: Some("privacy-first voice typing".to_string()),
        });
        dict.add_entry(DictionaryEntry {
            term: "BYOK".to_string(),
            aliases: vec![],
            description: Some("Bring Your Own Key".to_string()),
        });

        let results = dict.search_entries("privacy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].term, "Localtype");
    }

    #[test]
    fn test_get_terms() {
        let mut dict = PersonalDictionary::new();
        dict.add_entry(DictionaryEntry {
            term: "Localtype".to_string(),
            aliases: vec![],
            description: None,
        });
        dict.add_entry(DictionaryEntry {
            term: "BYOK".to_string(),
            aliases: vec![],
            description: None,
        });

        let terms = dict.get_terms();
        assert_eq!(terms.len(), 2);
        assert!(terms.contains(&"Localtype".to_string()));
        assert!(terms.contains(&"BYOK".to_string()));
    }
}
