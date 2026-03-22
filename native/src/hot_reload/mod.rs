//! Hot-reload coordinator — watches data files and invalidates caches.
//!
//! On native: polls FileWatcher each frame for changed files,
//! invalidates AssetManager cache entries, systems reload on next access.
//! On WASM: hot-reload is not supported (data fetched from server).

pub mod data_store;

#[cfg(feature = "native")]
use crate::assets::watcher::FileWatcher;
use crate::assets::AssetManager;

/// Coordinates hot-reload between file watcher and asset manager.
pub struct HotReloadCoordinator {
    #[cfg(feature = "native")]
    watcher: Option<FileWatcher>,
}

impl HotReloadCoordinator {
    /// Create a new coordinator. On native, starts watching the data directory.
    #[cfg(feature = "native")]
    pub fn new(data_dir: &std::path::Path) -> Self {
        let watcher = match FileWatcher::new(data_dir.to_path_buf()) {
            Ok(w) => Some(w),
            Err(e) => {
                log::warn!("Hot-reload disabled: {e}");
                None
            }
        };
        Self { watcher }
    }

    /// Create a no-op coordinator (WASM).
    #[cfg(feature = "wasm")]
    pub fn new() -> Self {
        Self {}
    }

    /// Poll for file changes and invalidate affected cache entries.
    /// Call once per frame. Returns the list of changed file paths (for logging).
    #[cfg(feature = "native")]
    pub fn poll(&self, asset_manager: &mut AssetManager) -> Vec<String> {
        let mut changed = Vec::new();
        if let Some(ref watcher) = self.watcher {
            for path in watcher.poll_changes() {
                asset_manager.invalidate(&path);
                changed.push(path);
            }
        }
        changed
    }

    /// No-op poll for WASM.
    #[cfg(feature = "wasm")]
    pub fn poll(&self, _asset_manager: &mut AssetManager) -> Vec<String> {
        Vec::new()
    }
}
