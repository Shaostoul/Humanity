//! In-memory data store — type-erased cache for game data loaded from files.
//!
//! The DataStore wraps AssetManager and provides a simpler interface for
//! game systems that just need to look up loaded data by key.

use std::collections::HashMap;
use std::any::Any;

/// Type-erased data store for hot-reloadable game data.
/// Game systems store parsed registries here (item lists, crop defs, recipes, etc.).
pub struct DataStore {
    entries: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Store a typed value by key.
    pub fn insert<T: Send + Sync + 'static>(&mut self, key: &str, value: T) {
        self.entries.insert(key.to_string(), Box::new(value));
    }

    /// Retrieve a typed reference by key.
    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.entries.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    /// Remove an entry (used on hot-reload to force re-load).
    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }
}
