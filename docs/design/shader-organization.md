# Shader organization: why one megashader, and the plan to split its SOURCE

Status: SHIPPED v0.973.0 (2026-07-26). Implemented as CONTIGUOUS slices of
the original file (byte-identical concatenation, zero semantic risk) rather
than the thematic regrouping sketched below; regrouping can happen gradually
WITHIN the split structure now that each part is its own file. Part names:
00-bindings-vertex, 05-overrides (the pipeline permutation switches, added
by increment P1 of the frame-cost arc, 2026-09-18: `override` constants that
guard the atmosphere, cloud and ocean dispatches, compiled off for every
pipeline whose class cannot draw them through `pipeline.rs::PSO_REGISTRY`;
the rule is in the `PBR_PARTS` doc comment in
`src/renderer/shader_loader.rs` and the numbers in
`docs/design/frame-cost-arc.md`), 10-lighting-patterns, 20-surface-detail,
30-atmosphere, 40-clouds, 41-cloud-bodies, 45-cloud-temporal, 50-brdf,
80-fragment-shared (increment P3, 2026-09-18: the prologue and the PBR tail
every material class shares, as two functions), 90-fragment-main (the six
per-class colour entries and the union `fs_shadow`). Verified per the v0.782
bar: full battery + boot + 4-vantage probe sweep (renders identical) +
hot-reload exercise (part save reassembles in 1.3 s). Operator question: "Is
there any particular reason you didn't break each shader down to its own
file? Wouldn't a bunch of individual shader files be better than a single
monolithic file?"

## One source, six entries, thirteen programs: the pipeline-per-class table (P1 + P2 + P3, 2026-09-18)

The megashader is one MODULE but not one PROGRAM. Three of the material
dispatch's early-return branches are whole programs on their own (the
atmosphere integrator, the volumetric cloud march, the ocean shell), and the
backend charges a branch's per-invocation frame to every fragment of every
pipeline that can reach it, whether or not the fragment takes it (the
finding in `docs/design/frame-cost-arc.md` section 0, proven by P1). So the
material type space is cut into CLASSES, each class has its OWN `@fragment`
entry in `90-fragment-main.wgsl` (P3) sharing the prologue and the lighting
tail in `80-fragment-shared.wgsl`, each pipeline compiles only the entry of
the class it draws with the WGSL `override` switches (`05-overrides.wgsl`)
that class keeps, and the draw loops pick the pipeline by the class of the
material in hand (`src/renderer/pipeline.rs`: `shader_class`,
`Pipeline::opaque_for`, `transparent_for`, `overlay_for`; the registry is
`PSO_REGISTRY`, one row per pipeline naming its class, its entry and its
dead switches).

| class | entry | material types | pipelines (label: state) | switch kept ON |
|---|---|---|---|---|
| Surface | `fs_surface` | 0..11 the procedural surfaces, 17 the sun's radial glow, 18 gas giant bands, 19 textured meshes, 24 screens, and the default look for any type no entry claims | PBR-lite Surface Render (opaque, back-face cull, depth write): walls, floors, props, machines, photoscanned trees, non-terrain bodies. PBR-lite Surface Transparent (alpha blend, no cull, depth test, no write): glass and windows, holograms, particles, the sun's blended core and halo. PBR-lite Surface Overlay (alpha blend, no cull, depth WRITE): editor gizmos | none |
| Terrain | `fs_terrain` | 12 the planet surface: terrain patches, the sprite tree cards baked into them, the far canopy sheet, orbital water | PBR-lite Terrain Render (opaque, classic per-object source): uniform-sphere planet bodies. Patch Batch Render (opaque, batch object source): the chunked patches and their baked cards. Patch Batch Shadow (`fs_shadow`, standard z): the same patches into the sun map | none |
| Vegetation | `fs_vegetation` | 20 procedural plant, 21 cluster card, 22 baked bark, 23 grass strand | PBR-lite Vegetation Render (opaque): near-tree parts, garden plants, the instanced grass sward | none |
| Water | `fs_water` | 16 the ocean shell and its backstop | PBR-lite Water Transparent (as Surface Transparent): the sea from orbit and whenever `water_depth_write` is off. PBR-lite Water Overlay (as Surface Overlay): the sea while `water_depth_write` holds (v0.1060) | `HAS_OCEAN_BRANCH` |
| Shell | `fs_shell` | 13 the Fresnel atmosphere fallback, 14 the scattering atmosphere | PBR-lite Shell Transparent (as Surface Transparent). No overlay PSO: nothing routes an atmosphere through an overlay list | `HAS_ATMOSPHERE_BRANCH` |
| Cloud | `fs_cloud` | 15 the cloud shell, and nothing else | PBR-lite Cloud Transparent (as Surface Transparent): the ONE pipeline whose fragments pay for the march's per-invocation tables | `HAS_CLOUD_BRANCH` |
| (every caster) | `fs_shadow` or none | all | Sun Shadow (depth only, no fragment): opaque casters, water included (its vertex displacement is untouched: vs_main reads no switch). Sun Shadow Alpha (`fs_shadow` cutout, no colour target): cutout casters (12 sprite cards, 19, 21) | none (every switch off, a stated no-op) |

The opaque draw loops walk their list once per class it carries, in
`OPAQUE_CLASS_ORDER` (surface, vegetation, terrain: the cheap fragments
first so the expensive terrain fragment behind them is z-rejected), each
walk on its own PSO, so a list that interleaves classes at fine grain (a
bark part next to a photoscan stem, a planter next to its plant) binds each
pipeline once rather than switching per object; within a class the list
order is kept. The transparent lists switch on class change, grouped by the
planet layer band (`renderer/celestial_order.rs`, not by class, so the
authored dome / deck / sea compositing order is untouched). The six
`fs_cloud_*` fullscreen pipelines compile the same module through entries
that are no class entry and reach no material dispatch, and are exempt from
the registry.

`fs_shadow` stays ONE union twin of the class entries' cutouts: it carries
no heavyweight storage (no shell is reachable from it), the shadow pass has
no class routing to gain from a split, and the cutout mirror test names the
class entry each cutout lives in.

Rules that keep the table true (`src/renderer/pipeline.rs`, all read from
the ONE assembled source): `shader_class_is_pinned_to_the_class_entries`
(every type inside a guarded band lands in the class that keeps its switch;
every shipped type 0..24 lands where the hand table says AND its class's
entry is the one entry whose body tests for it; the band edges are
half-open at .5); `each_shell_dispatch_is_guarded_by_its_switch_inside_its_class_entry`
(the guard shape, the guard inside the right entry, the guarded function
called from exactly one place); `each_class_entry_reads_exactly_its_live_switches`
(the registry's live sets are the switches the entries read, and the shared
prologue and tail read none);
`the_fragment_entries_are_the_class_entries_the_shadow_twin_and_the_exempt_passes`
(no fs_main lingering, no colour entry without a class);
`the_registry_covers_exactly_the_thirteen_megashader_psos`; and every
builder must take both its constants and its fragment entry from the
registry by label (`every_megashader_pso_builder_asks_the_registry`).
`validate_wgsl` refuses a module missing any class entry or any switch. A
new material goes in its class's entry; a new family gets a class, an entry,
registry rows and a classifier band; a new heavyweight branch gets its own
switch as well and is folded off every pipeline that cannot draw it.

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
