//! Asset manager — loads and caches game data with hot-reload support.
//!
//! Supported data formats: CSV, TOML, RON, JSON.
//! Asset formats: GLB (meshes), PNG/KTX2 (textures), OGG/WAV (audio), WGSL (shaders).
//!
//! The data directory lives next to the exe (like Space Engineers' Content/ folder).
//! On native: reads from disk, watches for changes via notify.
//! On WASM: data fetched via HTTP from the server.

#[cfg(feature = "native")]
pub mod watcher;
pub mod loader;

use std::collections::HashMap;
use std::any::Any;
use std::path::PathBuf;
use serde::de::DeserializeOwned;

/// Central asset manager: loads data files, caches parsed results, supports hot-reload.
pub struct AssetManager {
    /// Root data directory (e.g., `HumanityOS/content/data/`).
    data_dir: PathBuf,
    /// Cached parsed data, keyed by relative path from data_dir.
    cache: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl AssetManager {
    /// Create a new asset manager rooted at the given data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        log::info!("AssetManager: data directory = {}", data_dir.display());
        Self {
            data_dir,
            cache: HashMap::new(),
        }
    }

    /// Full path to a data file.
    pub fn data_path(&self, relative: &str) -> PathBuf {
        self.data_dir.join(relative)
    }

    /// The root data directory.
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Load and parse a CSV file into a Vec<T>. Results are cached by path.
    /// Skips comment lines (starting with #).
    #[cfg(feature = "native")]
    pub fn load_csv<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&Vec<T>, String> {
        if !self.cache.contains_key(relative_path) {
            let path = self.data_dir.join(relative_path);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let records: Vec<T> = loader::parse_csv(&bytes)?;
            log::info!("Loaded {} records from {}", records.len(), relative_path);
            self.cache.insert(relative_path.to_string(), Box::new(records));
        }
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<Vec<T>>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load and parse a TOML file into T. Results are cached by path.
    #[cfg(feature = "native")]
    pub fn load_toml<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if !self.cache.contains_key(relative_path) {
            let path = self.data_dir.join(relative_path);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let value: T = loader::parse_toml(&bytes)?;
            log::info!("Loaded TOML: {}", relative_path);
            self.cache.insert(relative_path.to_string(), Box::new(value));
        }
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load and parse a RON file into T. Results are cached by path.
    #[cfg(feature = "native")]
    pub fn load_ron<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if !self.cache.contains_key(relative_path) {
            let path = self.data_dir.join(relative_path);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let value: T = loader::parse_ron(&bytes)?;
            log::info!("Loaded RON: {}", relative_path);
            self.cache.insert(relative_path.to_string(), Box::new(value));
        }
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Invalidate a cached entry (called by hot-reload on file change).
    pub fn invalidate(&mut self, relative_path: &str) {
        if self.cache.remove(relative_path).is_some() {
            log::info!("Cache invalidated: {}", relative_path);
        }
    }

    /// Store pre-parsed data (used by WASM where data arrives via fetch).
    pub fn store<T: Send + Sync + 'static>(&mut self, key: &str, value: T) {
        self.cache.insert(key.to_string(), Box::new(value));
    }

    /// Retrieve cached data by key.
    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.cache.get(key).and_then(|v| v.downcast_ref::<T>())
    }
}
