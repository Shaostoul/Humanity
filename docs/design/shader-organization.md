# Shader organization: why one megashader, and the plan to split its SOURCE

Status: SHIPPED v0.973.0 (2026-07-26). Implemented as CONTIGUOUS slices of
the original file (byte-identical concatenation, zero semantic risk) rather
than the thematic regrouping sketched below; regrouping can happen gradually
WITHIN the split structure now that each part is its own file. Part names:
00-bindings-vertex, 05-overrides (the pipeline permutation switches, added
by increment P1 of the frame-cost arc, 2026-09-18: `override` constants that
guard fs_main's atmosphere, cloud and ocean dispatches, compiled off for the
terrain PSOs through `pipeline.rs::PSO_DEAD_BRANCHES`; the rule is in the
`PBR_PARTS` doc comment in `src/renderer/shader_loader.rs` and the numbers
in `docs/design/frame-cost-arc.md`), 10-lighting-patterns, 20-surface-detail,
30-atmosphere, 40-clouds, 41-cloud-bodies, 45-cloud-temporal, 50-brdf,
90-fragment-main. Verified per the v0.782 bar: full
battery + boot + 4-vantage probe sweep (renders identical) + hot-reload
exercise (part save reassembles in 1.3 s). Operator question: "Is there any particular
reason you didn't break each shader down to its own file? Wouldn't a bunch of
individual shader files be better than a single monolithic file?"

## One source, ten programs: the pipeline-per-class table (P1 + P2, 2026-09-18)

The megashader is one MODULE but not one PROGRAM. Three of fs_main's
early-return branches are whole programs on their own (the atmosphere
integrator, the volumetric cloud march, the ocean shell), and the backend
charges a branch's per-invocation frame to every fragment of every pipeline
that can reach it, whether or not the fragment takes it (the finding in
`docs/design/frame-cost-arc.md` section 0, proven by P1). So each pipeline is
compiled with WGSL `override` switches (`05-overrides.wgsl`) set for the
MATERIAL CLASS it draws, and the draw loops pick the pipeline by the class
of the material in hand (`src/renderer/pipeline.rs`: `shader_class`,
`Pipeline::transparent_for`, `Pipeline::overlay_for`; the registry is
`PSO_DEAD_BRANCHES`, one row per pipeline).

| pipeline (label) | class | fixed-function state | switches kept ON | material types that ride it |
|---|---|---|---|---|
| PBR-lite Render Pipeline | General | opaque, back-face cull, depth write | none | every opaque material: 0..11 procedural surfaces, 17 and 18 gas giants, 19 textured meshes, 20 near-tree foliage, 21 cluster cards, 22 bark, 23 grass, 24 screens; planet bodies; never a shell (debug-asserted at every opaque draw site) |
| PBR-lite Transparent Pipeline | General | alpha blend, no cull, depth test, no write | none | general transparents: glass and windows, holograms, particles, the sun's blended core and halo (17) |
| PBR-lite Overlay Pipeline | General | alpha blend, no cull, depth WRITE | none | editor gizmos |
| PBR-lite Shell Transparent Pipeline | Shell | as Transparent | `HAS_ATMOSPHERE_BRANCH`, `HAS_OCEAN_BRANCH` | 14 atmosphere shell, 16 water shell and its backstop |
| PBR-lite Shell Overlay Pipeline | Shell | as Overlay | `HAS_ATMOSPHERE_BRANCH`, `HAS_OCEAN_BRANCH` | 16 water shell while `water_depth_write` holds (v0.1060) |
| PBR-lite Cloud Transparent Pipeline | Cloud | as Transparent | `HAS_CLOUD_BRANCH` | 15 cloud shell, and nothing else |
| Sun Shadow Pipeline | General (no-op) | depth only, no fragment | none | opaque shadow casters, water included (its vertex displacement is untouched: fs_shadow never reaches a shell and vs_main reads no switch) |
| Sun Shadow Alpha Pipeline | General (no-op) | `fs_shadow` cutout, no colour target | none | cutout casters (12 sprite cards, 19, 21) |
| Patch Batch Render Pipeline | terrain (all off) | as Render, batch object source | none | 12 terrain patches and their baked cards |
| Patch Batch Shadow Pipeline | terrain (all off) | `fs_shadow`, standard z | none | the same patches into the sun map |

Type 13, the legacy Fresnel atmosphere fallback, is not a guarded dispatch
(it runs through the `< 13.5` chain), so it is General and draws through the
general transparent pipeline. The six `fs_cloud_*` fullscreen pipelines
compile the same module through entries that never enter fs_main and are
exempt from the registry.

Rules that keep the table true: `shader_class` is pinned to the shader's
three guarded bands by `shader_class_is_pinned_to_the_guarded_dispatch_bands`
(every type in a band must land in a class that keeps that band's switch,
every other type in 0 to 24.5 must be General, and the cloud switch is live
in exactly one class); `the_registry_covers_exactly_the_ten_megashader_psos`
pins each row's dead set to its class; every builder must ask the registry
by label (`every_megashader_pso_builder_asks_the_registry`). A new
heavyweight branch in fs_main gets its own switch, its own class row here,
and is folded off every pipeline that cannot draw it.

## The honest answer

There are two different things called "a shader" here, and they have
different rules:

1. **What the GPU compiles.** wgpu compiles ONE WGSL module per pipeline.
   Everything a pipeline's vertex + fragment stages call - every function,
   constant, and binding declaration - must be in that one module. WGSL has
   no include statement and naga (the compiler) has no preprocessor. So at
   the GPU level there is always exactly one big text blob per pipeline; the
   only question is how that blob is assembled.

2. **What humans edit.** Today the blob IS the source file:
   `assets/shaders/pbr_simple.wgsl` (~3,500 lines) carries the PBR core plus
   the atmosphere, clouds (three quality tiers), ocean, sky-view hybrid,
   water shading, vegetation cards, ground textures, and shadows, because
   they all share bindings, helpers, and the material-type dispatch in
   `fs_main`.

The monolith was the zero-tooling choice: no build step, no concatenation
order to get wrong, one file to hot-reload (the runtime watches it and
rebuilds pipelines in ~3 s), one file for the loader's embedded fallback,
and one file for the source-scanning tests (the ocean CPU-twin lockstep test
parses it by path, and the lint suite scans it).

Where the operator's instinct is right: 3,500 lines is past the point where
one file serves human navigation, focused diffs, or parallel agents editing
disjoint shader domains without three-way-merge hazards (the repo's own
throughput notes call this file a merge funnel). Standalone passes are
already separate files (`sky_view_lut.wgsl`, `particles.wgsl`,
`billboard_bake` inline) - the megashader is only big because its domains
genuinely share one pipeline.

## Accepted design: split the SOURCE, keep the MODULE

`assets/shaders/pbr/` gains numbered parts, concatenated in name order at
load time into the same single module the pipelines compile today:

- `00-bindings.wgsl` - groups 0-3 declarations, shared consts, vertex IO
- `10-lighting-core.wgsl` - PBR/GGX, shadows, light loop + tile lists
- `20-atmosphere.wgsl` - scattering, LUT hybrid, aerial perspective
- `30-clouds.wgsl` - the three cloud variants + noise helpers
- `40-water.wgsl` - ocean waves, water shading, sea ice
- `50-surface.wgsl` - planet surface (type 12), vegetation cards, ground
  textures
- `90-main.wgsl` - vs_main + fs_main dispatch (the if-chain stays whole;
  function bodies cannot straddle files but functions separate cleanly)

Loader changes (all three read sites must move together):
1. Runtime load: read the directory in name order, join, compile (same
   `create_shader_module` call).
2. Hot-reload: watch the directory, not one file; any part changing rebuilds
   the same pipeline set.
3. Embedded fallback: `include_str!` each part, join at compile time.

Test changes: the ocean lockstep test and the shader lints read the
concatenation (a tiny shared helper in `src/renderer/` returns the joined
source both for the compiler and for tests) - they must never go
path-by-path, or a constant moved between parts would dodge the lockstep.

## Why this is deferred rather than done tonight

The v0.782-784 incident (three consecutive unbootable releases from a
device-limit rejection no test caught) sets the verification bar for
megashader surgery: full release build, boot the exe, probe-sweep the
vantage suite, and hot-reload exercise - per split stage, not once at the
end. That is a focused daytime session, not the tail of a 14-release night.
The split is mechanical (move functions, never edit them), so the risk is
purely procedural - which is exactly why it gets fresh-session discipline.
