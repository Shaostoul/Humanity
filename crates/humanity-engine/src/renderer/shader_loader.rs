//! WGSL shader hot-reload via notify file watcher.
//!
//! Shaders live in `assets/shaders/*.wgsl` and are reloaded on change.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Embedded fallback shader used when the on-disk shader can't be loaded.
const FALLBACK_SHADER: &str = include_str!("../../../../assets/shaders/pbr_simple.wgsl");

/// Loads and caches WGSL shader modules, recompiling on file change.
pub struct ShaderLoader {
    shaders: HashMap<PathBuf, wgpu::ShaderModule>,
    _watcher: Option<RecommendedWatcher>,
    change_rx: mpsc::Receiver<PathBuf>,
}

impl ShaderLoader {
    pub fn new() -> Self {
        let (_tx, rx) = mpsc::channel();
        Self {
            shaders: HashMap::new(),
            _watcher: None,
            change_rx: rx,
        }
    }

    /// Load a .wgsl shader from disk. Falls back to embedded shader on error.
    pub fn load(&mut self, device: &wgpu::Device, path: &Path) -> &wgpu::ShaderModule {
        let canonical = path.to_path_buf();
        if !self.shaders.contains_key(&canonical) {
            let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
                log::warn!("Failed to load shader {:?}: {}, using fallback", path, e);
                FALLBACK_SHADER.to_string()
            });
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(path.to_str().unwrap_or("shader")),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            self.shaders.insert(canonical.clone(), module);
        }
        self.shaders.get(&canonical).unwrap()
    }

    /// Load the embedded PBR-lite shader directly (no disk path needed).
    pub fn load_embedded_pbr(&self, device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr_simple (embedded)"),
            source: wgpu::ShaderSource::Wgsl(FALLBACK_SHADER.into()),
        })
    }

    /// Start watching a directory for .wgsl file changes.
    /// Changed paths are queued and can be polled with `poll_changes()`.
    pub fn watch(&mut self, dir: &Path) {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        self.change_rx = rx;

        let sender = tx;
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if path.extension().map_or(false, |ext| ext == "wgsl") {
                        let _ = sender.send(path);
                    }
                }
            }
        })
        .expect("Failed to create file watcher");

        if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
            log::warn!("Failed to watch shader directory {:?}: {}", dir, e);
        }
        self._watcher = Some(watcher);
    }

    /// Poll for changed shader files. Returns paths that need recompilation.
    pub fn poll_changes(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        while let Ok(path) = self.change_rx.try_recv() {
            changed.push(path);
        }
        changed
    }

    /// Recompile a shader from disk, replacing the cached module.
    /// Returns true if recompilation succeeded.
    pub fn recompile(&mut self, device: &wgpu::Device, path: &Path) -> bool {
        match std::fs::read_to_string(path) {
            Ok(source) => {
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(path.to_str().unwrap_or("shader")),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
                self.shaders.insert(path.to_path_buf(), module);
                log::info!("Recompiled shader: {:?}", path);
                true
            }
            Err(e) => {
                log::error!("Failed to reload shader {:?}: {}", path, e);
                false
            }
        }
    }
}
