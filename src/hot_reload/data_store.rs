//! In-memory data store — type-erased cache for game data loaded from files.
//!
//! The DataStore is a plain map from a key to any loaded value: game systems
//! look up their parsed data by key (it does not reach the asset manager).

use std::collections::HashMap;
use std::any::Any;

/// Type-erased data store for hot-reloadable game data.
/// Game systems store parsed registries here (item lists, crop defs, recipes, etc.).
pub struct DataStore {
    entries: HashMap<String, Box<dyn Any + Send + Sync>>,
    /// Each key's change stamp (2026-10-31, docs/design/sim-frame-cost.md, "The Garden views by
    /// area"): the store's running count of inserts and removes at the last one of that key. A
    /// reader that keeps what it worked out from an entry (the Garden page's rows) keeps it only
    /// while the stamp is the one it read: a hot reload, a machine edit that publishes the grow
    /// plots again, anything that INSERTS the key moves it. A value held in a `Mutex` and changed
    /// in place does not move it; such a reader reads that value itself.
    stamps: HashMap<String, u64>,
    changes: u64,
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            stamps: HashMap::new(),
            changes: 0,
        }
    }

    /// Store a typed value by key.
    pub fn insert<T: Send + Sync + 'static>(&mut self, key: &str, value: T) {
        self.entries.insert(key.to_string(), Box::new(value));
        self.stamp_change(key);
    }

    /// Retrieve a typed reference by key.
    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.entries.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    /// Remove an entry (used on hot-reload to force re-load).
    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
        self.stamp_change(key);
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// The key's change stamp: moves every time it is inserted or removed, 0 for a key never
    /// set. Two equal stamps mean the same entry, never re-inserted in between.
    pub fn stamp(&self, key: &str) -> u64 {
        self.stamps.get(key).copied().unwrap_or(0)
    }

    fn stamp_change(&mut self, key: &str) {
        self.changes += 1;
        match self.stamps.get_mut(key) {
            Some(s) => *s = self.changes,
            None => {
                self.stamps.insert(key.to_string(), self.changes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DataStore;

    /// A stamp moves on every insert and remove of its own key and on nothing else: the Garden
    /// page keeps its rows while the stamps of the data it read are the ones it read. Seen red
    /// 2026-10-31 with the stamp never moving: "the same value inserted again still moves it".
    #[test]
    fn a_stamp_moves_with_its_own_key_only() {
        let mut d = DataStore::new();
        assert_eq!(d.stamp("a"), 0, "never set");
        d.insert("a", 1_u32);
        let a1 = d.stamp("a");
        d.insert("b", 2_u32);
        assert_eq!(d.stamp("a"), a1, "another key's insert leaves it");
        d.insert("a", 1_u32);
        let a2 = d.stamp("a");
        assert_ne!(a2, a1, "the same value inserted again still moves it");
        d.remove("a");
        assert_ne!(d.stamp("a"), a2, "a remove moves it");
        let b = d.stamp("b");
        let _ = d.get::<u32>("b");
        assert_eq!(d.stamp("b"), b, "a read moves nothing");
    }
}
