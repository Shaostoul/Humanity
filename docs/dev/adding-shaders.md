# Adding Shaders and Materials

The rendering reality: almost everything visible in the 3D world goes through
ONE shader, `assets/shaders/pbr_simple.wgsl` (~3100 lines), the "megashader".
This doc explains its material-type dispatch, how to add a new material, the
naga/FXC gotchas that have actually bitten, and the verify bar.

## The megashader model

- Bind groups: **0** camera + the uncapped storage-buffer light list, **1**
  object (model + normal matrix, dynamic offset), **2** material uniforms,
  **3** textures + shadow map (full layout below).
- `MaterialUniforms` is `base_color: vec4` + `params: vec4` where
  `params = (metallic, roughness, material_type, emissive_strength)`.
- The fragment entry `fs_main` reads `material.params.z` and branches into one
  of the material types. Types 13.5..16.5 short-circuit the PBR path entirely
  (they are participating media, not surfaces); most others tweak
  albedo/metallic/roughness/emissive and fall through to the shared
  Cook-Torrance GGX lighting path (sun + fill + point/spot lights + shadow
  map + eclipse/terminator logic).
- **Note the param overloads.** Some types repurpose param slots:
  type 12 uses `params.w` as the "albedo texture present" flag (and zeroes
  emissive); type 15 uses `params.y` as the cloud-quality switch; type 18 uses
  `params.w` as the giant-palette selector. When adding a type, check which
  slots the shared tail still interprets as metallic/roughness/emissive.

Editing note: the pipeline compiles pbr_simple.wgsl via
`include_str!` (`src/renderer/shader_loader.rs` `FALLBACK_SHADER`, used by
`load_embedded_pbr`). **Shader edits therefore require a `cargo build`, there
is no live hot reload of this file** (the ShaderLoader's disk/watch path
exists but nothing calls it today). Also be aware `assets/shaders/README.md`
describes an older binding convention (group 2 textures, group 3 material)
that pbr_simple does NOT follow; trust this doc + `src/renderer/pipeline.rs`.

## Material type table (from `pbr_simple.wgsl` `fs_main`)

| Type | What it is |
|------|------------|
| 0 | Default panel grid (walls, floors) |
| 1 | Brushed metal |
| 2 | Concrete |
| 3 | Wood |
| 4 | Glass (Fresnel-boosted reflectivity, drawn on the transparent pipeline) |
| 5 | Ice (blue-white, wrap lighting, crystalline noise) |
| 6 | Water surface (animated wave normals, foam at shallow angles) |
| 7 | Leather (Voronoi pore pattern) |
| 8 | Crystal (faceted noise, prismatic view-angle color, high metallic) |
| 9 | Rust/corroded (FBM orange-brown patches over base metal) |
| 10 | Moss/growth (green patches on upward/north-facing world-space surfaces) |
| 11 | Lava (black rock, glowing emissive cracks) |
| 12 | Planet surface (v0.763): per-face color packed into UV; `params.w > 0.5` = sample the group-3 equirect albedo texture instead |
| 13 | Atmosphere shell (fresnel limb tint, the simple fallback) |
| 14 | Analytic atmosphere scattering (v0.807), short-circuits PBR |
| 15 | Procedural cloud layer (v0.874+): `params.y` picks Low/Medium/High quality path, all three stay compiled so no tier rots |
| 16 | Planetary ocean shell (v0.876 Gerstner real-water), short-circuits PBR; also displaces vertices in `vs_main` |
| 17 | Radial glow (v0.886, the sun's corona halo on an oversized sphere) |
| 18 | Gas giant bands (v0.905): latitude-ramp palettes warped by noise; `params.w` = 0 jupiter, 1 saturn, 2 uranus, 3 neptune |
| 19 | Textured mesh (v0.909, photoscanned plants): group-3 albedo texture times base_color, alpha cutout at a < 0.35, then normal PBR |

A type number >= 19.5 matches NO branch: it renders as a plain untextured PBR
surface with the material's base_color/metallic/roughness (a stale comment in
the shader claims fall-through goes to the panel grid; it does not, the
type-0 branch is `material_type < 0.5`).

## Group-3 binding layout (bindings 0-10)

Every pipeline sharing this shader binds SOMETHING at each slot; draws that do
not use a texture get a 1x1 fallback, so the group is harmless to all other
material types. Layout (declarations near the top of the shader; Rust side in
`src/renderer/pipeline.rs` `texture_bind_group_layout`):

| Binding | Var | Purpose |
|---------|-----|---------|
| 0 | `albedo_texture` (texture_2d) | Planet equirect imagery (type 12) / mesh texture (type 19) |
| 1 | `albedo_sampler` | Clamping sampler for binding 0 |
| 2 | `cloud_shape_tex` (texture_3d) | 128^3 Perlin-Worley cloud shape volume (type 15 High) |
| 3 | `cloud_detail_tex` (texture_3d) | 64^3 inverted-Worley detail volume |
| 4 | `cloud_tile_sampler` | Repeating sampler for the tiling volumes |
| 5 | `weather_map` (texture_2d) | Live NASA GIBS cloud-fraction equirect (RG8: fraction + validity) |
| 6 | `shadow_map` (texture_depth_2d) | 4096^2 sun shadow map (v0.899) |
| 7 | `shadow_samp` (sampler_comparison) | PCF comparison sampler |
| 8 | `shadow_u` (uniform ShadowUniforms) | light_vp + enable/strength/texel-size |
| 9 | `ground_tex` (texture_2d_array) | Ground PBR array: layers 0-3 color (grass/dirt/rock/sand), 4-7 normal maps (v0.907) |
| 10 | `ground_samp` | Wrapping, 4x anisotropic sampler for the ground array |

If you add a binding, you must update BOTH the WGSL declarations AND
`texture_bind_group_layout` in `pipeline.rs`, plus every
`create_bind_group` site that fills the group (grep `texture_bind_group` in
`src/renderer/mod.rs`), and provide the neutral 1x1 fallback resource so
builds without the asset still boot.

## Adding a new material type

1. Pick the next free number (20 as of this writing). Grep the shader for
   `material_type >=` to see the dispatch pattern: ranges are half-open
   floating bands (`>= 18.5 && < 19.5` is type 19).
2. Add the branch in `fs_main` (or a short-circuit before the PBR tail if
   your material is not a lit surface). Set `albedo` / `metallic` /
   `roughness` / `proc_emissive` / `emissive_strength` and fall through, or
   `return` a final color.
3. Create the material from Rust:
   `renderer.add_material_full(base_color, metallic, roughness, 20.0,
   emissive)` (`src/renderer/mod.rs`; `add_material_typed` if you do not need
   emissive, `add_textured_material` if it consumes a texture). Real example,
   the four gas giants in `src/lib.rs`:
   `add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 0.0)`.
4. If it needs per-draw data beyond the two vec4s, that is a bind-group
   change, think hard first (an unused params slot or packing into UV, the
   type-12 trick, is much cheaper).
5. Follow the two-place rule: if the effect has a Rust-side mirror (the cloud
   regime tables have one in `renderer/clouds.rs`), keep the constants
   numerically identical and locked by a test.

## naga / FXC gotchas (each has shipped a broken build; do not rediscover them)

- **Cannot pass `array<f32, N>` across function boundaries.** naga's HLSL
  backend generates code FXC rejects (X3017 "cannot convert from 'float[7]'
  to 'float'"), and it fails at PIPELINE CREATION at app boot, not at build
  time. Local arrays indexed inside one function are fine; accumulate with
  scalar locals in a single loop instead of returning arrays (see the comment
  above `cloud_regime` in the shader, the v0.893 incident).
- **`textureSampleLevel`, not `textureSample`, inside non-uniform control
  flow.** Implicit-derivative sampling is only legal in uniform control flow;
  anything inside a raymarch loop, an `if material_type` branch's loop, or
  after a `discard` path must sample with an explicit LOD. The convention in
  this codebase: single-mip textures (weather map, mesh textures) always use
  `textureSampleLevel(..., 0.0)`.
- **An insertion between an `@vertex`/`@fragment` attribute and its `fn`
  silently orphans the attribute** (e.g. adding a `const` there). naga
  validates the module with ZERO entry points and every pipeline dies at
  first boot. The `shader_loader` test pins entry points by name to catch
  this (2026-07-18 lesson).
- **Uniform struct padding:** uniforms are std140-like; keep vec4 packing
  (the codebase packs scalars into `.x/.y/.z/.w` of vec4s rather than adding
  loose f32 fields, see `ShadowUniforms.params`). Never reorder existing
  fields of `CameraUniforms`; the legacy light0..7 fields are dead but kept
  so no byte offset shifts.
- **Device limits are not validated by tests.** The v0.782-784 incident:
  a storage buffer passed naga validation and all tests, but the device was
  requested with downlevel limits (fragment storage buffers = 0) and the app
  died before the first frame, for three releases. Limits now come from
  `wgpu::Limits::default()` (`renderer/mod.rs` `request_device`); if your
  feature needs more than defaults, gate it at runtime.

## The verify bar (non-negotiable, from the v0.782 incident)

`cargo test` + naga validation do NOT catch pipeline-creation failures,
device-limit rejections, or FXC backend errors. For ANY
renderer/pipeline/bind-group/shader change:

1. `cargo build --features native --release`
2. Launch `target/release/HumanityOS.exe`, wait ~10 s.
3. Grep `%APPDATA%/HumanityOS/logs/run.log` for `PANIC` (expect zero) and for
   wgpu validation errors.
4. Visual check via the screenshot protocol: drop
   `debug/screenshot_request.json` in the repo root, read
   `debug/screenshot_N.png` (see
   [performance-profiling.md](performance-profiling.md)).
5. `taskkill //PID <pid>` when done.
6. Also `cargo check --features relay --no-default-features` (the relay build
   must never regress from a native-side change).

## Other shaders

Separate pipelines with their own WGSL live next to pbr_simple:
`bloom.wgsl`, `godrays.wgsl`, `ssao.wgsl`, `particle.wgsl`, `stars.wgsl`,
`ghost_preview.wgsl`, and the per-body celestial shaders (`sun_surface.wgsl`,
`earth.wgsl`, ...). Same verify bar applies. The procedural floor/wall
material WGSLs (`plank_wood.wgsl`, `granite_tile.wgsl`, ...) are the
construction-material family. When in doubt whether to extend pbr_simple or
add a pipeline: a new SURFACE look is a material type; a new PASS (post
effect, different vertex format) is a new pipeline in
`src/renderer/`.

## Hot-reload (v0.924): edit the megashader LIVE, no rebuild

Saving `assets/shaders/pbr_simple.wgsl` while HumanityOS is running
revalidates the source with naga and rebuilds the four PSOs in place -
the running world (position, saves, streamed terrain) is untouched. This
replaces the 3+ minute rebuild-and-reboot loop for shader iteration.

- Detection is a once-per-second MTIME poll (`Renderer::poll_shader_reload`),
  NOT a filesystem watcher: the notify backend delivered zero events
  through the portable rig's NTFS junction (probe-proven on both the
  junction path and the canonicalized real path). One metadata read per
  second is free and works through every alias and editor write strategy.
- A broken mid-edit save is REJECTED with a `[HotReload] ... REJECTED` log
  line and the old pipelines stay - the app never crashes on a bad save.
  Fix the file and save again.
- Rebuild cost is the PSO compile: ~5 s with DXC (`dxcompiler.dll` +
  `dxil.dll` beside the exe), ~30 s on the FXC fallback. The frame thread
  blocks for that time (acceptable for a dev loop; async swap is a noted
  follow-up). Watch for `[HotReload] ... recompiled + 4 PSOs rebuilt`.
- Scope: `pbr_simple.wgsl` only (the shader that changes daily). The small
  single-purpose shaders still need a rebuild.
- The probe rig gets hot-reload automatically (its `assets/` junction
  resolves to the repo checkout). Keep the DXC DLLs in the rig folder or
  reloads take the slow path.
