//! Hot-reload coordinator — watches data files and reports which changed.
//!
//! On native: polls FileWatcher each frame for changed files and hands them to the engine, which
//! invalidates the AssetManager's cache entries and runs the rebuild each file asks for
//! (engine/hot_reload_gates.rs; systems reload on next access). On WASM: hot-reload is not
//! supported (data fetched from server). Since the crate split's rung 0 (2026-10-31) this module
//! is the core's and reaches nothing above it: the gates that rebuild the engine's state moved to
//! the engine (part B; the watcher had come down from the renderer in part A).

pub mod data_store;
/// The data folder's file watcher (notify): here since the crate split's rung 0, beside the
/// coordinator that polls it (it was `assets::watcher`).
#[cfg(feature = "native")]
pub mod watcher;

#[cfg(feature = "native")]
use watcher::FileWatcher;

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

    /// Poll for file changes. Call once per frame. Returns the changed file paths, relative to
    /// the data directory, for the caller to invalidate in the asset manager and to log
    /// (engine/hot_reload_gates.rs `poll_and_apply`). It used to take the asset manager and invalidate them itself; since the
    /// crate split's rung 0 it only reports, so this coordinator does not name the renderer's
    /// `AssetManager`.
    #[cfg(feature = "native")]
    pub fn poll(&self) -> Vec<String> {
        match self.watcher {
            Some(ref watcher) => watcher.poll_changes(),
            None => Vec::new(),
        }
    }

    /// No-op poll for WASM.
    #[cfg(feature = "wasm")]
    pub fn poll(&self) -> Vec<String> {
        Vec::new()
    }
}
