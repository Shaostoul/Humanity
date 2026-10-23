# Renderer, shaders and planet-scale numbers

**Who reads this:** anyone changing the renderer, a shader, a pipeline or bind group, or math that
carries planet-scale positions. Before fencing a visual increment, also read CLAUDE.md's
"Realistic-first" rule; before booting anything, `docs/ai/game-boots.md`.

Moved from `CLAUDE.md` on 2026-10-23 with the wording unchanged; CLAUDE.md keeps a one-line
version of each rule.

## The PBR shader files

The PBR shader is a set of numbered files under `assets/shaders/pbr/`, not one megashader file;
the list that matters, and its order, is `src/renderer/shader_loader.rs` (`pbr_simple.wgsl` has
not existed for a long time). `90-fragment-main.wgsl` is the shared shader tail: more than two or
three agents editing the SAME shader branch create genuine 3-way-merge correctness hazards, not
just merge chores (`docs/ai/orchestrator.md`, section 4).

## The megashader's class entries

- **The PBR megashader is six class entries, one per PSO, and no `fs_main`: a heavyweight branch costs every pixel of every pipeline that compiles it, even untaken** (H1: the cloud march cost terrain 44.5 ms with clouds off; `docs/design/frame-cost-arc.md`, P1 to P3). A new heavyweight branch gets its own `override` switch, lives in ONE class entry (`pipeline.rs` `ShaderClass`, `PSO_REGISTRY`), is off in every pipeline that cannot draw it, and its switch is the FIRST operand of the guard (`if (HAS_X && type_test)`). Every draw loop picks its PSO by the object's class (`Pipeline::opaque_for` / `transparent_for` / `overlay_for`); a material on an entry without its branch draws the default panel grid, with no error in release. Group the celestial transparent list by planet LAYER BAND (`renderer/celestial_order.rs`), never by class (a class key sank the fallback dome under the sea). Keep `permutation_tests` (`pipeline.rs`) and the switch check in `shader_loader::validate_wgsl`: naga silently ignores an unknown override key, so a rename or a stale assets mirror re-enables the branch. Measure with the rig (same-boot A/B through the shader hot reload), not by reading.

## Verify by booting and entering the world

- **Verify every renderer, pipeline, bind-group or shader change by BOOTING the release exe AND ENTERING THE WORLD: `just verify-runtime`** (~3 min, needs the GPU, so never part of `just verify`; it boots the release binary in the portable rig, enters the world by autopilot and fails on any `PANIC`). Static checks miss both failures it exists for: wgpu device-limit rejections (v0.782 to v0.784 shipped unbootable, "a flicker but the game never comes up"; limits are now `wgpu::Limits::default()`) and world-entry panics (a missed lazy bind group, ten releases, "insta crashes when I press esc"). After changing a bind group layout, every `create_bind_group` site built from it must carry the same bindings: `tests/bind_group_entry_lint.rs` (in `just lints` and CI, since 2026-10-19) checks every site against its layout and fails closed on a site it cannot resolve. A quick manual boot, after `cargo build --features native --release`, is `just launch-bg` (freshness gate, no focus, no hand-off to a newer archive; a hand-typed `target/release/HumanityOS.exe` skips all three), then grep `%APPDATA%/HumanityOS/logs/run.log` for `PANIC` and `taskkill //PID <pid>`; world entry goes through the rig because the in-place exe refuses autopilot when a real identity exists. When the operator reports a boot failure, read `run.log`/`crash.log` FIRST: the panic hook always writes the cause there.

## f32 at planet scale

- **f32 at planet scale is a defect class.** On the CPU, keep planet positions AND unit directions in f64 (`DVec3`) all the way to a terrain or data sampler, and downcast only at a coarse-data boundary (base heightmap cells are 460+ m, so `.as_vec3()` is fine THERE and nowhere earlier): an f32 unit vector quantizes ground position at ~0.4-0.8 m, an f32 global-grid coordinate at ~3.6 m. Check the whole chain: a CALLER that narrows to f32 undoes an f64-upgraded function (the buoyancy float). On the GPU (f32 by hardware), keep magnitudes small near the camera (floating origin, anchor-relative patch verts, drawn-shell units); a shader phase or UV from a planet-radius f32 dot (`dot(dir * r, d)`) carries ~0.5 m quantization that moves with the camera, so evaluate it in the camera-anchored 64 m-modulus domain (`anchw + dvw`) with wavelengths that DIVIDE 64 and axis-aligned directions, so anchor re-snaps shift phase by whole numbers and the CPU f64 twin matches mod 1 (ocean chop W4/W5/W6 and its lockstep test are the worked example).
