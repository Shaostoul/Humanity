//! WGSL shader hot-reload via notify file watcher.
//!
//! Shaders live in `assets/shaders/*.wgsl` and are reloaded on change.

/// Loads and caches WGSL shader modules, recompiling on file change.
pub struct ShaderLoader {
    // TODO: shader cache, file watcher handle
}

impl ShaderLoader {
    pub fn new() -> Self {
        Self {}
    }
}
