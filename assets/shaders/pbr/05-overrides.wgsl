// ── PIPELINE PERMUTATION SWITCHES (increment P1 of the frame-cost arc,
//    docs/design/frame-cost-arc.md section 1, 2026-09-18) ──
//
// One megashader source, several compiled programs. These are WGSL
// pipeline-overridable constants: every pipeline that compiles this module
// may set them through wgpu::PipelineCompilationOptions::constants, and naga
// substitutes the value BEFORE the backend sees the source. A switch left at
// its default keeps the branch; a switch set to false turns the guarded
// dispatch in fs_main (90-fragment-main.wgsl) into `if (false && ...)`,
// which DXC folds away together with every function and every
// `var<private>` table only that branch reached.
//
// Why this exists, in one paragraph. fs_main dispatches on material type,
// and three of its early-return branches are heavyweight whole programs: the
// atmosphere integrator (type 14, 30-atmosphere.wgsl), the volumetric cloud
// march (type 15, 40-clouds.wgsl + 41-cloud-bodies.wgsl) and the ocean shell
// (type 16, the `ocean_shell` function below in 90-fragment-main.wgsl). The
// cloud march declares per-invocation private storage (`g_bc_lc`, 180 x
// vec4, zero-initialised and dynamically indexed) and the backend charges
// that frame to EVERY fragment of every pipeline whose entry point can reach
// it, whether or not the fragment ever takes the branch. Measured on the
// terrain pass with the branches made unreachable (phase A of P1, same boot,
// same frozen clock): moon-surface-200m gpu.celestial 44.11 -> 7.43 ms,
// sahara-noon-ground 31.38 -> 5.43 ms with the cloud branch alone removed,
// and the Sahara's atmosphere shell in the transparent pass 10.44 -> 0.19 ms,
// all with the image unchanged inside the rig's frame-to-frame noise.
//
// Which pipeline sets what lives in ONE place, `PSO_DEAD_BRANCHES` in
// src/renderer/pipeline.rs, and a test there pins these declarations, the
// guards in fs_main and that table against each other. Add a switch here
// when a new fs_main branch drags a large per-invocation frame into the
// module; name it HAS_<THING>_BRANCH, default it to true, guard the dispatch
// with it as the FIRST operand of the condition, and register it there.
//
// Keys: wgpu carries every override value as an f64 and naga maps it onto a
// bool as `value != 0.0`, so "off" is 0.0 in the Rust map.
override HAS_ATMOSPHERE_BRANCH: bool = true;
override HAS_CLOUD_BRANCH: bool = true;
override HAS_OCEAN_BRANCH: bool = true;

