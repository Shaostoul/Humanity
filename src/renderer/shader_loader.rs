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
///
/// SHADER PERMUTATIONS (increment P1 of the frame-cost arc, 2026-09-18):
/// one module, but NOT one program. `05-overrides.wgsl` declares WGSL
/// `override` switches (`HAS_ATMOSPHERE_BRANCH`, `HAS_CLOUD_BRANCH`,
/// `HAS_OCEAN_BRANCH`, all defaulting to true) that guard the three
/// heavyweight shell dispatches, and each pipeline in
/// `pipeline.rs::PSO_REGISTRY` is compiled with the switches its material
/// class needs. Naga substitutes the values before the backend sees the
/// source, so a switched-off branch is dead code DXC folds away, together
/// with every function and `var<private>` table only it reached.
///
/// CLASS ENTRIES (increment P3, 2026-09-18): one module, SIX colour entry
/// points. `80-fragment-shared.wgsl` holds what every material shares (the
/// prologue that takes the derivatives and reads the material, the PBR
/// tail that lights and composes), and `90-fragment-main.wgsl` has one
/// `@fragment` entry per material class (`fs_surface`, `fs_terrain`,
/// `fs_vegetation`, `fs_water`, `fs_shell`, `fs_cloud`; the classes and
/// their types are `pipeline.rs::ShaderClass`) plus the union `fs_shadow`.
/// A pipeline compiles only the entry of the class it draws, so its DXIL
/// never holds another class's blocks, and the three switches guard their
/// dispatches inside their own entries. `validate_wgsl` refuses a module
/// missing any of the seven fragment entries (or `vs_main`), the way it
/// refuses one missing a switch.
///
/// The rule for anyone adding a material: put its block in the entry of
/// its class (or give it a class, an entry and registry rows if it is a
/// new family), never in the shared parts, and if it drags a big
/// per-invocation frame into the module (a `var<private>` array, a deep
/// march), give it a switch as well and turn it off in every PSO that can
/// never draw that material, because the backend charges that frame to
/// EVERY fragment of every pipeline that can reach it, not to the fragments
/// that take the branch. Measured on the terrain pass: see the P1 outcome in
/// docs/design/frame-cost-arc.md section 1(c), and the P3 outcome for what
/// the entry split itself moved.
pub const PBR_PARTS: &[(&str, &str)] = &[
    ("00-bindings-vertex.wgsl", include_str!("../../assets/shaders/pbr/00-bindings-vertex.wgsl")),
    ("05-overrides.wgsl", include_str!("../../assets/shaders/pbr/05-overrides.wgsl")),
    ("10-lighting-patterns.wgsl", include_str!("../../assets/shaders/pbr/10-lighting-patterns.wgsl")),
    ("20-surface-detail.wgsl", include_str!("../../assets/shaders/pbr/20-surface-detail.wgsl")),
    ("30-atmosphere.wgsl", include_str!("../../assets/shaders/pbr/30-atmosphere.wgsl")),
    ("40-clouds.wgsl", include_str!("../../assets/shaders/pbr/40-clouds.wgsl")),
    (
        "41-cloud-bodies.wgsl",
        include_str!("../../assets/shaders/pbr/41-cloud-bodies.wgsl"),
    ),
    ("45-cloud-temporal.wgsl", include_str!("../../assets/shaders/pbr/45-cloud-temporal.wgsl")),
    ("50-brdf.wgsl", include_str!("../../assets/shaders/pbr/50-brdf.wgsl")),
    ("80-fragment-shared.wgsl", include_str!("../../assets/shaders/pbr/80-fragment-shared.wgsl")),
    ("90-fragment-main.wgsl", include_str!("../../assets/shaders/pbr/90-fragment-main.wgsl")),
];

/// Every entry point the megashader must declare: the vertex entry, the
/// six class colour entries and the union shadow twin. `validate_wgsl`
/// refuses a module lacking any of them, so a class PSO can never be
/// created against a module that dropped its entry (wgpu would refuse the
/// pipeline at boot with "Unable to find entry point"; this names it
/// earlier and on the hot-reload path, where a boot panic is not the
/// failure mode).
pub fn required_entry_points() -> Vec<&'static str> {
    let mut out = vec!["vs_main"];
    out.extend(super::pipeline::ShaderClass::ALL.iter().map(|c| c.fragment_entry()));
    out.push(super::pipeline::SHADOW_FRAGMENT_ENTRY);
    out
}

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
    // into g_inst_data by vs_main (attribute) and frag_prologue (flat varying,
    // for every class entry).
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

/// The megashader source to BUILD PIPELINES FROM AT BOOT.
///
/// Normally the embedded assembly, which is baked in at compile time. With
/// `HUMANITY_SHADERS_FROM_DISK=1` it reads the on-disk parts instead.
///
/// WHY THIS EXISTS (2026-08-28). Hot reload only fires when a part's mtime
/// changes WHILE THE APP IS RUNNING. The probe rig boots, captures and exits,
/// so a rig sweep always rendered the COMPILED-IN shader no matter what was on
/// disk - and a whole day of cloud measurements was made against an unchanged
/// shader before a control (force the cloud body to 1.0 everywhere; the sky did
/// not go white) caught it. Every "no change" result from that day was an
/// artifact of the edit never reaching the GPU.
///
/// With the flag, a shader iteration is one sweep (about a minute) instead of a
/// four-minute rebuild plus a sweep. That matters: the cloud work needs dozens
/// of iterations, and a slow loop is why so many were batched and confounded.
///
/// Off by default so a shipped build can never depend on loose files.
#[cfg(feature = "native")]
pub fn boot_pbr_source() -> std::borrow::Cow<'static, str> {
    if std::env::var("HUMANITY_SHADERS_FROM_DISK").ok().as_deref() == Some("1") {
        if let Some(dir) = find_shaders_dir() {
            if let Some(src) = assembled_pbr_source_from_dir(&dir) {
                // The same gate the hot reload passes through. A parse or
                // validation error would die loudly inside wgpu's module
                // creation anyway; what this catches that wgpu cannot is a
                // tree from BEFORE the permutation (no switch declared, no
                // guard using one: a stale checkout or mirror), which
                // compiles and boots and silently hands the terrain pass the
                // cloud march again (see check_permutation_switches). A rig
                // exists to measure what is on disk, so a disk tree that
                // fails the gate is a boot failure with the cause in the
                // log, not a quiet fallback the rig would then measure as if
                // it were the edit.
                if let Err(e) = validate_wgsl(&src) {
                    panic!(
                        "[Shaders] HUMANITY_SHADERS_FROM_DISK=1: the on-disk megashader under \
                         {:?} was REJECTED and this boot refuses to measure something else \
                         instead: {e}",
                        dir.join("pbr")
                    );
                }
                log::info!(
                    "[Shaders] BOOT FROM DISK ({} bytes) under {:?}",
                    src.len(),
                    dir.join("pbr")
                );
                return std::borrow::Cow::Owned(src);
            }
        }
        log::warn!(
            "[Shaders] HUMANITY_SHADERS_FROM_DISK=1 but no on-disk parts found; \
             using the embedded assembly"
        );
    }
    std::borrow::Cow::Borrowed(assembled_pbr_source())
}

#[cfg(not(feature = "native"))]
pub fn boot_pbr_source() -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed(assembled_pbr_source())
}

/// The terrain-batch variant of whatever [`boot_pbr_source`] returned, so both
/// pipelines come from the SAME source and cannot disagree.
pub fn boot_pbr_batch_source() -> String {
    let base = boot_pbr_source();
    batched_variant_of(&base).unwrap_or_else(|| assembled_pbr_batch_source().to_string())
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
/// megashader hot-reload): parse, validate, and pin the entry points (the
/// vertex entry, the six class entries and fs_shadow since P3). Used as
/// the gate before a hot-reloaded shader is allowed anywhere near pipeline
/// creation - a mid-edit save must produce a log line, never a crash. Same
/// checks the `embedded_pbr_shader_parses_and_validates` test enforces at
/// build time.
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
    let missing: Vec<&str> = required_entry_points()
        .into_iter()
        .filter(|name| !entries.contains(name))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "entry points missing (an attribute may have orphaned onto a const): {missing:?} \
             not among {entries:?}"
        ));
    }
    check_permutation_switches(&module)?;
    check_hlsl_expressible(source)?;
    Ok(())
}

/// The permutation switches (P1, and the P2 class split) must be DECLARED
/// by the module, or the pipeline constants that switch the general and
/// terrain PSOs' shell branches off (and the cloud march off the shell
/// PSOs) bind to nothing.
///
/// Why this is a refusal and not a warning: naga's override substitution
/// returns a module UNCHANGED when it declares no overrides, and wgpu 24
/// raises no error for a constant key the module does not declare. So a
/// megashader from BEFORE the permutation, with no switch declared and no
/// guard using one (a `HUMANITY_SHADERS_FROM_DISK` boot or a hot reload
/// against a stale `assets/shaders/pbr/`, a `--shipped-assets` rig whose
/// mirror predates P1), compiles, boots, renders correctly and silently
/// hands every terrain fragment the whole cloud march again: the 45 ms
/// floor P1 removed, back with no log line anywhere. Nothing downstream can
/// tell; this gate can. (Deleting 05-overrides.wgsl ALONE is not that case:
/// the guards then reference undefined identifiers and naga refuses the
/// source as a parse error before this runs.)
///
/// Checked on the PARSED module rather than the text, so the declaration's
/// spacing is free but its meaning is pinned: each switch must exist, be a
/// `bool`, and default to `true` (every pipeline that does not name a switch
/// relies on that default to keep its branch).
fn check_permutation_switches(module: &wgpu::naga::Module) -> Result<(), String> {
    use wgpu::naga::{Expression, Literal, Scalar, ScalarKind, TypeInner};
    for name in super::pipeline::ALL_BRANCH_SWITCHES {
        let Some((_, switch)) = module
            .overrides
            .iter()
            .find(|(_, o)| o.name.as_deref() == Some(*name))
        else {
            return Err(format!(
                "permutation switch missing: the module declares no `override {name}: bool = true;` \
                 (assets/shaders/pbr/05-overrides.wgsl). Without it the pipeline constant that \
                 switches this branch off binds to nothing and every terrain fragment silently \
                 pays for the branch again; refusing the module"
            ));
        };
        let is_bool = matches!(
            module.types[switch.ty].inner,
            TypeInner::Scalar(Scalar { kind: ScalarKind::Bool, .. })
        );
        if !is_bool {
            return Err(format!(
                "permutation switch {name} must be declared `bool` (pipeline.rs maps 0.0 onto a \
                 bool override as `value != 0.0`); refusing the module"
            ));
        }
        let defaults_true = switch
            .init
            .is_some_and(|h| matches!(module.global_expressions[h], Expression::Literal(Literal::Bool(true))));
        if !defaults_true {
            return Err(format!(
                "permutation switch {name} must default to `= true`: every pipeline that does not \
                 name it relies on that default to keep its branch compiled in; refusing the module"
            ));
        }
    }
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
        let src = boot_pbr_source();
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pbr_simple (boot)"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
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
        let err = super::validate_wgsl("fn nope( {").expect_err("a parse error must be refused");
        assert!(err.starts_with("parse error"), "refused for the wrong reason: {err}");
        // Valid WGSL with the permutation switches declared but no entry
        // points at all. The switches are included so the refusal is for
        // the reason this test is about, not for the gate added later (see
        // the test below for that one).
        let src = format!("{}fn helper() -> f32 {{ return 1.0; }}", super::test_support::switch_declarations());
        let err = super::validate_wgsl(&src).expect_err("an entryless module must be refused");
        assert!(err.starts_with("entry points missing"), "refused for the wrong reason: {err}");
        // ONE class entry missing (P3): every class PSO compiles its own
        // entry, so a module that dropped one is refused and the refusal
        // names it, on the hot-reload path where a boot panic cannot help.
        let all = format!(
            "{}{}",
            super::test_support::switch_declarations(),
            super::test_support::entry_stubs()
        );
        super::validate_wgsl(&all).expect("every required entry present passes");
        let stub = super::test_support::entry_stub("fs_vegetation");
        assert!(all.contains(&stub), "the vegetation stub is in the full set");
        let one_gone = all.replace(&stub, "");
        let err = super::validate_wgsl(&one_gone).expect_err("a missing class entry must be refused");
        assert!(
            err.starts_with("entry points missing") && err.contains("fs_vegetation"),
            "the refusal must name the missing entry: {err}"
        );
    }

    /// Parse + full naga validation ONLY, with no gate of ours: what wgpu's
    /// own module creation would accept. Used to show that the sources the
    /// switch gate refuses are sources nothing else would have refused.
    fn naga_alone_accepts(source: &str) -> bool {
        let Ok(module) = wgpu::naga::front::wgsl::parse_str(source) else {
            return false;
        };
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .is_ok()
    }

    /// A megashader as it looked BEFORE the permutation for `name`: no
    /// `override` declaration and no `HAS_X && ` in its guard. That is the
    /// shape of a stale tree (a `--shipped-assets` mirror or an on-disk
    /// checkout that predates P1), and it is the DANGEROUS shape: it parses
    /// and validates, naga substitutes nothing into a module that declares
    /// no overrides, wgpu 24 raises no unknown-key error, and the terrain
    /// PSOs' constants bind to nothing. (Stripping the declaration alone
    /// leaves the guard referencing an undefined identifier, which naga
    /// refuses as a parse error on its own; that half-pruned shape needs no
    /// gate of ours.)
    fn without_switch(src: &str, name: &str) -> String {
        let decl = format!("override {name}: bool = true;");
        let guard_use = format!("{name} && ");
        assert!(src.contains(&decl), "the source declares {name}");
        assert_eq!(src.matches(&guard_use).count(), 1, "the source guards ONE dispatch with {name}");
        src.replace(&decl, "").replace(&guard_use, "")
    }

    /// The gate REFUSES a megashader that lacks a permutation switch, naming
    /// the switch, and the sources it refuses are ones naga alone ACCEPTS:
    /// without this refusal they compile, boot and render correctly while
    /// every terrain fragment silently pays for the cloud march again, and
    /// nothing downstream can tell. Three shapes of the same defect: one
    /// switch gone (declaration and guard, the pre-permutation shape for
    /// that branch), all three gone (a tree from before P1), and a switch
    /// declared but defaulting to `false`.
    #[test]
    fn validate_wgsl_refuses_a_megashader_missing_a_permutation_switch() {
        use super::super::pipeline::{ALL_BRANCH_SWITCHES, BRANCH_CLOUD};
        let intact = super::assembled_pbr_source();
        super::validate_wgsl(intact).expect("the embedded megashader passes the gate");

        // One switch gone: naga alone accepts it; the gate refuses it and
        // names the switch.
        let one_gone = without_switch(intact, BRANCH_CLOUD);
        assert!(
            naga_alone_accepts(&one_gone),
            "the pre-permutation shape must be a source naga accepts, or this test is not \
             testing the silent case"
        );
        let err = super::validate_wgsl(&one_gone)
            .expect_err("a megashader without HAS_CLOUD_BRANCH must be refused");
        assert!(
            err.contains(BRANCH_CLOUD) && err.contains("permutation switch missing"),
            "the refusal must name the missing switch: {err}"
        );

        // All three gone (the stale-tree case): naga accepts, the gate refuses.
        let mut all_gone = intact.to_string();
        for name in ALL_BRANCH_SWITCHES {
            all_gone = without_switch(&all_gone, name);
        }
        for name in ALL_BRANCH_SWITCHES {
            // (The names survive in comments, which is fine; only the
            // declaration and the guard use matter to naga.)
            assert!(
                !all_gone.contains(&format!("override {name}")) && !all_gone.contains(&format!("{name} && ")),
                "{name} is neither declared nor used any more"
            );
        }
        assert!(naga_alone_accepts(&all_gone), "a pre-P1 megashader is valid WGSL");
        let err = super::validate_wgsl(&all_gone).expect_err("a switchless megashader must be refused");
        assert!(err.contains("permutation switch missing"), "wrong reason: {err}");

        // Declared, but defaulting to false: every pipeline that does not
        // name the switch would lose the branch. Naga accepts, the gate
        // refuses naming the switch and the required default.
        let decl = format!("override {BRANCH_CLOUD}: bool = true;");
        let flipped = intact.replace(&decl, &format!("override {BRANCH_CLOUD}: bool = false;"));
        assert!(naga_alone_accepts(&flipped), "a false default is valid WGSL");
        let err = super::validate_wgsl(&flipped)
            .expect_err("a switch defaulting to false must be refused");
        assert!(
            err.contains(BRANCH_CLOUD) && err.contains("= true"),
            "the refusal must name the switch and the required default: {err}"
        );

        // For completeness: the half-pruned shape (declaration gone, guard
        // kept) is refused too, by naga's parser, before our gate runs.
        let half = intact.replace(&decl, "");
        let err = super::validate_wgsl(&half).expect_err("an undefined identifier is refused");
        assert!(err.starts_with("parse error"), "refused by the parser: {err}");
    }
}

/// Shared by the two test modules below: the permutation switch
/// declarations a minimal test source needs to get PAST the switch gate, so
/// each test's refusal is for the reason that test is about.
#[cfg(test)]
mod test_support {
    pub fn switch_declarations() -> String {
        super::super::pipeline::ALL_BRANCH_SWITCHES
            .iter()
            .map(|name| format!("override {name}: bool = true;\n"))
            .collect()
    }

    /// A minimal declaration of one required entry point: the vertex entry
    /// returns a position, a colour entry returns a colour, the shadow twin
    /// returns nothing (it only ever discards).
    pub fn entry_stub(name: &str) -> String {
        match name {
            "vs_main" => format!("@vertex fn {name}() -> @builtin(position) vec4<f32> {{ return vec4<f32>(0.0); }}\n"),
            n if n == super::super::pipeline::SHADOW_FRAGMENT_ENTRY => format!("@fragment fn {name}() {{ }}\n"),
            _ => format!("@fragment fn {name}() -> @location(0) vec4<f32> {{ return vec4<f32>(0.0); }}\n"),
        }
    }

    /// Every entry point `validate_wgsl` requires, as stubs, so a minimal
    /// test source gets PAST the entry gate and its refusal is for the
    /// reason the test is about.
    pub fn entry_stubs() -> String {
        super::required_entry_points().into_iter().map(entry_stub).collect()
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
        // With the permutation switches declared and every required entry
        // stubbed, so the refusal below is the HLSL guard's and not the
        // switch or entry gate's.
        let src = format!(
            "{}fn f() -> array<f32, 2> {{ var a: array<f32, 2>; return a; }}\n{}",
            super::test_support::switch_declarations(),
            super::test_support::entry_stubs()
        );
        let err = validate_wgsl(&src)
            .expect_err("validate_wgsl must reject what the HLSL backend cannot compile");
        assert!(err.contains("HLSL"), "refused for the wrong reason: {err}");
    }
}
