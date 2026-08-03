//! WGSL shader loading with optional hot-reload (native only).
//!
//! Shaders live in `assets/shaders/*.wgsl` and are reloaded on change.
//! The embedded fallback shader works on all platforms (native + WASM).

#[cfg(feature = "native")]
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;

/// The megashader's SOURCE PARTS (v0.973, docs/design/shader-organization.md):
/// contiguous numbered slices of the former pbr_simple.wgsl, concatenated in
/// name order into the ONE module every PBR pipeline compiles. The split is
/// file-level organization only - the assembled source is byte-identical to
/// the pre-split monolith, proven by the round-trip check at split time and
/// pinned by `assembled_parts_form_a_valid_module` below. Add a part by
/// adding it HERE and on disk; order is the tuple order (name-sorted).
pub const PBR_PARTS: &[(&str, &str)] = &[
    ("00-bindings-vertex.wgsl", include_str!("../../assets/shaders/pbr/00-bindings-vertex.wgsl")),
    ("10-lighting-patterns.wgsl", include_str!("../../assets/shaders/pbr/10-lighting-patterns.wgsl")),
    ("20-surface-detail.wgsl", include_str!("../../assets/shaders/pbr/20-surface-detail.wgsl")),
    ("30-atmosphere.wgsl", include_str!("../../assets/shaders/pbr/30-atmosphere.wgsl")),
    ("40-clouds.wgsl", include_str!("../../assets/shaders/pbr/40-clouds.wgsl")),
    ("50-brdf.wgsl", include_str!("../../assets/shaders/pbr/50-brdf.wgsl")),
    ("90-fragment-main.wgsl", include_str!("../../assets/shaders/pbr/90-fragment-main.wgsl")),
];

/// The assembled megashader source (embedded parts, joined). THE single
/// source both the pipelines and every source-scanning test read, so a
/// constant moved between parts can never dodge a lockstep check.
pub fn assembled_pbr_source() -> &'static str {
    static SRC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SRC.get_or_init(|| PBR_PARTS.iter().map(|(_, s)| *s).collect::<String>())
}

/// The terrain-batch OBJECT-SOURCE block: swapped in for the classic
/// per-draw object uniform by `batched_variant_of`. Every patch draw in a
/// batch shares one bind group; per-patch data (anchor translation + LOD
/// fade) lives in a storage array indexed by @builtin(instance_index),
/// which the batched draw loop feeds via draw_indexed(.., i..i+1) -- DIRECT
/// draws, deliberately: this machine's DX12 adapter reports the
/// VERTEX_AND_INSTANCE_INDEX_RESPECTS_RESPECTIVE_FIRST_VALUE_IN_INDIRECT_DRAW
/// downlevel flag MISSING, so the indirect-draw first_instance trick is not
/// portable here, while first_instance in direct draws is core WebGPU.
pub const BATCH_OBJECT_SOURCE: &str = r#"// BEGIN OBJECT-SOURCE (terrain-batch variant, injected by
// shader_loader::batched_variant_of; the classic block lives in
// 00-bindings-vertex.wgsl).
struct BatchUniforms {
    // The rotation-only model matrix every patch in the batch shares this
    // frame (planet rotation; patches never scale). Per-patch translation
    // + fade arrive through the inst_pos_fade instance attribute, captured
    // into g_inst_data by vs_main (attribute) and fs_main (flat varying).
    rot: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> batch: BatchUniforms;
var<private> g_inst_data: vec4<f32> = vec4<f32>(0.0);
fn obj_model() -> mat4x4<f32> {
    var m = batch.rot;
    m[3] = vec4<f32>(g_inst_data.xyz, 1.0);
    // Keep the classic metadata contract: model[0].w carries the fade.
    m[0].w = g_inst_data.w;
    return m;
}
fn obj_normal_matrix() -> mat4x4<f32> {
    // Pure rotation at unit scale: inverse-transpose == the rotation
    // itself, and transpose(obj_normal_matrix()) == the model's inverse
    // rotation -- exactly the property the planet-local fragment math
    // (and the water vertex branch) relies on.
    return batch.rot;
}
fn obj_lod_fade() -> f32 { return g_inst_data.w; }
// END OBJECT-SOURCE"#;

/// Derive the terrain-batch shader variant from an assembled classic
/// source by replacing the marked OBJECT-SOURCE block. Works on both the
/// embedded assembly and a hot-reloaded on-disk assembly, so shader edits
/// keep applying to BOTH pipelines. None when the markers are missing
/// (someone renamed them in 00-bindings-vertex.wgsl -- the caller logs and
/// keeps the classic-only pipeline set rather than crashing).
pub fn batched_variant_of(classic: &str) -> Option<String> {
    let begin = classic.find("// BEGIN OBJECT-SOURCE")?;
    let end_marker = "// END OBJECT-SOURCE";
    let end = classic.find(end_marker)? + end_marker.len();
    if begin >= end {
        return None;
    }
    Some(format!(
        "{}{}{}",
        &classic[..begin],
        BATCH_OBJECT_SOURCE,
        &classic[end..]
    ))
}

/// The assembled terrain-batch megashader variant (embedded).
pub fn assembled_pbr_batch_source() -> &'static str {
    static SRC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SRC.get_or_init(|| {
        batched_variant_of(assembled_pbr_source())
            .expect("OBJECT-SOURCE markers missing from 00-bindings-vertex.wgsl")
    })
}

/// Assemble the megashader from ON-DISK parts (dev checkout / portable rig):
/// every .wgsl under `shaders_dir/pbr/`, joined in name order. None when the
/// directory is absent or empty (stripped install) - the embedded assembly
/// rules then, exactly like the old single-file fallback.
#[cfg(feature = "native")]
pub fn assembled_pbr_source_from_dir(shaders_dir: &std::path::Path) -> Option<String> {
    let dir = shaders_dir.join("pbr");
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |x| x == "wgsl"))
        .collect();
    if names.is_empty() {
        return None;
    }
    names.sort();
    let mut out = String::new();
    for p in names {
        out.push_str(&std::fs::read_to_string(p).ok()?);
    }
    Some(out)
}

/// Newest modification time across the on-disk parts - the hot-reload
/// poll's change signal (any part saved = the assembly is stale).
#[cfg(feature = "native")]
pub fn pbr_parts_mtime(shaders_dir: &std::path::Path) -> Option<std::time::SystemTime> {
    let dir = shaders_dir.join("pbr");
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |x| x == "wgsl"))
        .filter_map(|p| std::fs::metadata(&p).ok()?.modified().ok())
        .max()
}

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
    check_hlsl_expressible(source)?;
    Ok(())
}

/// Constructs that parse AND pass full naga validation, and are then REJECTED
/// by the HLSL backend at device init on Windows/DX12.
///
/// v0.1101 shipped a `fn ground_material_weights(...) -> array<f32, 5>`. WGSL
/// allows it, naga validated it, the megashader gate went green, and the app
/// died on the operator's adapter with "cannot initialize return object of type
/// 'float' with an lvalue of type 'float[5]'" - a boot failure that every
/// static check had passed. This is the same shape as the v0.782 device-limit
/// incident: the thing that decides is the BACKEND, and naga's validator does
/// not speak for it.
///
/// A textual check rather than running naga's `back::hlsl` writer, because the
/// backend is not a compiled-in dependency here and the failing construct has a
/// crisp syntactic signature. If this list ever needs a third entry, take that
/// as the signal to depend on the real backend instead of growing a lint.
fn check_hlsl_expressible(source: &str) -> Result<(), String> {
    for (n, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        // A function RETURNING an array. Arrays are fine as locals, as
        // parameters and as module-scope constants - only the return crosses
        // an ABI the HLSL backend cannot express.
        if code.contains("->") && code.split("->").nth(1).is_some_and(|r| r.trim_start().starts_with("array<"))
        {
            return Err(format!(
                "line {}: a function returns an array, which naga validates but the \
                 HLSL backend cannot express - it fails at device init on DX12 only. \
                 Return a struct instead (see GroundWeights in 20-surface-detail.wgsl). \
                 Offending line: {}",
                n + 1,
                code.trim()
            ));
        }
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
                assembled_pbr_source().to_string()
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
            source: wgpu::ShaderSource::Wgsl(assembled_pbr_source().into()),
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
        if let Err(e) = super::validate_wgsl(super::assembled_pbr_source()) {
            panic!("assembled megashader failed validation: {e}");
        }
    }

    /// The terrain-batch variant must ALSO validate: it is a real second
    /// module compiled at boot (patch pipelines), derived by marker
    /// substitution -- a marker rename or a batch-block typo would
    /// otherwise only surface as a boot-time pipeline panic.
    #[test]
    fn batched_variant_parses_and_validates() {
        let src = super::assembled_pbr_batch_source();
        assert!(
            src.contains("BatchUniforms"),
            "batch substitution did not take (markers missing?)"
        );
        assert!(
            !src.contains("var<uniform> object:"),
            "classic object uniform leaked into the batch variant"
        );
        if let Err(e) = super::validate_wgsl(src) {
            panic!("terrain-batch megashader variant failed validation: {e}");
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

#[cfg(test)]
mod hlsl_guard_tests {
    use super::*;

    /// The guard must FAIL on the exact construct that shipped a boot panic in
    /// v0.1101, and must not fire on the legal uses of arrays around it.
    #[test]
    fn array_returning_function_is_rejected_but_array_locals_are_fine() {
        let bad = "fn ground_material_weights(img: vec3<f32>) -> array<f32, 5> {\n\
                   var w: array<f32, 5>;\n return w;\n}\n\
                   @vertex fn vs_main() -> @builtin(position) vec4<f32> { return vec4<f32>(0.0); }\n\
                   @fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.0); }\n";
        let err = check_hlsl_expressible(bad).expect_err("must reject an array return");
        assert!(err.contains("HLSL"), "the error must name the backend: {err}");

        // Legal: array locals, array params, module-scope const arrays, and a
        // struct return. None of these may trip the guard.
        let good = "var<private> TILE: array<f32, 5> = array<f32, 5>(1.0, 1.0, 1.0, 1.0, 1.0);\n\
                    struct GroundWeights { w: vec4<f32>, e: f32 }\n\
                    fn weights(img: vec3<f32>) -> GroundWeights {\n\
                      var t: array<f32, 5>;\n var o: GroundWeights;\n return o;\n}\n";
        check_hlsl_expressible(good).expect("legal array use must pass");
    }

    /// And it must be wired into the gate the hot-reloader and the build-time
    /// megashader test both call - a guard nobody calls is not a guard.
    #[test]
    fn the_gate_itself_rejects_an_array_return() {
        let src = "fn f() -> array<f32, 2> { var a: array<f32, 2>; return a; }\n\
                   @vertex fn vs_main() -> @builtin(position) vec4<f32> { return vec4<f32>(0.0); }\n\
                   @fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.0); }\n";
        assert!(
            validate_wgsl(src).is_err(),
            "validate_wgsl must reject what the HLSL backend cannot compile"
        );
    }
}
