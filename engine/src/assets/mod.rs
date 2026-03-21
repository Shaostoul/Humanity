//! Asset manager — loads and caches game assets with hot-reload support.
//!
//! Supported formats: GLB, KTX2, OGG, WGSL, RON, CSV, TOML.

#[cfg(feature = "native")]
pub mod watcher;
pub mod loader;

/// Central asset manager with caching and hot-reload.
pub struct AssetManager {
    // TODO: asset cache, file watcher
}

impl AssetManager {
    pub fn new() -> Self {
        Self {}
    }
}
