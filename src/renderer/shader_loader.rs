//! WGSL shader loading with optional hot-reload (native only).
//!
//! Shaders live in `assets/shaders/*.wgsl` and are reloaded on change.
//! The embedded fallback shader works on all platforms (native + WASM).

#[cfg(feature = "native")]
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;

/// Embedded fallback shader used when the on-disk shader can't be loaded.
const FALLBACK_SHADER: &str = include_str!("../../assets/shaders/pbr_simple.wgsl");

/// Loads and caches WGSL shader modules, recompiling on file change (native only).
pub struct ShaderLoader {
    shaders: HashMap<PathBuf, wgpu::ShaderModule>,
    #[cfg(feature = "native")]
    _watcher: Option<RecommendedWatcher>,
    #[cfg(feature = "native")]
    change_rx: std::sync::mpsc::Receiver<PathBuf>,
}

/// Full naga validation of WGSL source WITHOUT touching the GPU (v0.924
/// megashader hot-reload): parse, validate, and pin the two entry points.
/// Used as the gate before a hot-reloaded shader is allowed anywhere near
/// pipeline creation - a mid-edit save must produce a log line, never a
/// crash. Same checks the `embedded_pbr_shader_parses_and_validates` test
/// enforces at build time.
pub fn validate_wgsl(source: &str) -> Result<(), String> {
    let module = wgpu::naga::front::wgsl::parse_str(source)
        .map_err(|e| format!("parse error: {e}"))?;
    let mut validator = wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .map_err(|e| format!("validation error: {e:?}"))?;
    let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
    if !entries.contains(&"vs_main") || !entries.contains(&"fs_main") {
        return Err(format!(
            "entry points missing (an attribute may have orphaned onto a const): {entries:?}"
        ));
    }
    Ok(())
}

/// Locate assets/shaders/ beside the exe or up the parent chain (the same
/// walk ground_textures uses for its asset dir), falling back to the CWD.
/// None = stripped install with no shader sources: hot-reload simply stays
/// off and the embedded shader rules, exactly as before v0.924.
#[cfg(feature = "native")]
pub fn find_shaders_dir() -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.to_path_buf());
            let mut dir = exe_dir.to_path_buf();
            for _ in 0..6 {
                match dir.parent() {
                    Some(p) => {
                        candidates.push(p.to_path_buf());
                        dir = p.to_path_buf();
                    }
                    None => break,
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    candidates
        .into_iter()
        .map(|c| c.join("assets").join("shaders"))
        .find(|p| p.is_dir())
}

impl ShaderLoader {
    pub fn new() -> Self {
        #[cfg(feature = "native")]
        let (_tx, rx) = std::sync::mpsc::channel();

        Self {
            shaders: HashMap::new(),
            #[cfg(feature = "native")]
            _watcher: None,
            #[cfg(feature = "native")]
            change_rx: rx,
        }
    }

    /// Load a .wgsl shader from disk. Falls back to embedded shader on error.
    /// Only available on native (WASM loads shaders via include_str or fetch).
    #[cfg(feature = "native")]
    pub fn load(
        &mut self,
        device: &wgpu::Device,
        path: &std::path::Path,
    ) -> &wgpu::ShaderModule {
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
    /// Works on all platforms.
    pub fn load_embedded_pbr(&self, device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr_simple (embedded)"),
            source: wgpu::ShaderSource::Wgsl(FALLBACK_SHADER.into()),
        })
    }

    /// Start watching a directory for .wgsl file changes (native only).
    /// Changed paths are queued and can be polled with `poll_changes()`.
    #[cfg(feature = "native")]
    pub fn watch(&mut self, dir: &std::path::Path) {
        let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
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

    /// Poll for changed shader files. Returns paths that need recompilation (native only).
    #[cfg(feature = "native")]
    pub fn poll_changes(&mut self) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        while let Ok(path) = self.change_rx.try_recv() {
            changed.push(path);
        }
        changed
    }

    /// Recompile a shader from disk, replacing the cached module (native only).
    /// Returns true if recompilation succeeded.
    #[cfg(feature = "native")]
    pub fn recompile(&mut self, device: &wgpu::Device, path: &std::path::Path) -> bool {
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

#[cfg(test)]
mod tests {
    /// Parse + validate the embedded PBR shader headlessly (naga front-end,
    /// no GPU). Without this, a WGSL syntax/type error only surfaces at the
    /// first app launch, taking every material down with it. Added v0.763
    /// alongside the planet-surface material types (12/13).
    #[test]
    fn embedded_pbr_shader_parses_and_validates() {
        // Entry points must EXIST by name (v0.876 lesson): naga silently
        // accepts an @vertex/@fragment attribute orphaned onto a const by an
        // insertion between the attribute and its fn -- the module then
        // validates fine but has no entry point, and every pipeline dies at
        // FIRST BOOT with "Unable to find entry point". validate_wgsl (the
        // hot-reload gate, v0.924) carries all three checks now - this test
        // pins the EMBEDDED shader through the same gate.
        if let Err(e) = super::validate_wgsl(super::FALLBACK_SHADER) {
            panic!("pbr_simple.wgsl failed validation: {e}");
        }
    }

    #[test]
    fn validate_wgsl_rejects_broken_and_entryless_sources() {
        // Parse error.
        assert!(super::validate_wgsl("fn nope( {").is_err());
        // Valid WGSL but no vs_main/fs_main entry points.
        assert!(super::validate_wgsl("fn helper() -> f32 { return 1.0; }").is_err());
    }
}
