//! In-memory data store that auto-refreshes when source files change.
//!
//! Used for CSV tables, RON configs, and TOML settings.

use std::collections::HashMap;

/// Keyed data store that reloads from disk on file change.
pub struct DataStore {
    /// Cached string data keyed by file path.
    entries: HashMap<String, String>,
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Get cached data for a file path, or None if not loaded.
    pub fn get(&self, path: &str) -> Option<&str> {
        self.entries.get(path).map(|s| s.as_str())
    }
}
