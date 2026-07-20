# Performance Findings, 2026-07-20 (merged from 4 investigation reports)

Synthesis of four read-only investigations: R1 (CPU frame), R2 (GPU draw), R3 (shader
cost), R4 (streaming). Duplicates merged; each item ranked by expected FPS gain times
implementation safety. Sizes: SMALL (under an hour, low risk), MEDIUM (an evening),
LARGE (an arc).

HOT FILE flags: `src/lib.rs`, `src/renderer/mod.rs`, and the megashader
`assets/shaders/pbr_simple.wgsl` are the funnel files. Anything touching them needs
careful SOLO implementation (no parallel agents on the same file) and a boot-verified
release exe (see Measurement notes). Items marked ISOLATED are safe to hand off or
parallelize.

Target scenarios from the reports: 17-30 FPS on planet surfaces, 10-16 FPS inside a
cloud deck, dips at a machine-heavy homestead, hitches during descent/approach.

## Top 5 quick wins

All SMALL. Suggested order of attack: 1 then 2 land in minutes and pay on every
surface frame; 3-5 are shader-side and can ship as one careful megashader pass plus
one isolated godrays edit.

1. **Skip the duplicate object-uniform upload when sun shadows are on.** (R1#2 = R2#4
   = R4#6) `src/renderer/mod.rs:2072` uploads uniforms for the shadow pass with a
   comment saying it covers both passes, then `src/renderer/mod.rs:2175` uploads the
   identical list again for the celestial pass. At 2048-6144 patches that is ~0.5-1.6 MB
   staged and written twice, with a `Mat4::inverse().transpose()` per object each time
   (`src/renderer/mod.rs:1475-1490`). Fix: guard the second call with `if !shadow_on`.
   One line. Expected: ~1-2 ms back every frame on planet surfaces. SMALL. HOT FILE
   (renderer/mod.rs), but the change is trivial and self-contained.

2. **Tighten the shadow caster cull from 65 km to the shadow extent.** (R2#2a, R4#2
   partial) The shadow ortho box is only 1500 m wide (`src/renderer/mod.rs:2045`) but
   the caster cull at `src/renderer/mod.rs:2112` accepts patches out to 65 km, so
   thousands of patches are re-rasterized into the 4096x4096 map for zero contribution.
   Fix: cull at extent plus patch bounding radius (~3-4 km). One constant. Expected:
   drops shadow draws from thousands to tens; reports estimate +20-40% surface FPS.
   SMALL. HOT FILE (renderer/mod.rs).

3. **Cloud light-march early-outs.** (R3#1) `cloud_sun_tau`
   (`assets/shaders/pbr_simple.wgsl:2109-2130`, called at :2282) does 8 3D volume taps
   per lit view sample, up to 384 taps per pixel in a deck. Three guarded exits, all
   visually neutral per R3: (a) skip entirely when `day <= 0.005` (night side already
   multiplies the result to ~0 at :2279); (b) break the loop once accumulated tau
   saturates (~10); (c) drop to 4 taps once view transmittance is below 0.3. Expected:
   +30-50% in the 10-16 FPS full-deck scene. SMALL. MEGASHADER. Note: shader constants
   are mirrored and asserted in `src/renderer/clouds.rs:779-813`; keep both sides in
   sync if any constant changes.

4. **God rays: compute the glow gate before the march.** (R3#5) `godrays.wgsl:52-70`
   marches 40 depth taps per pixel full-screen, then multiplies by
   `glow = exp(-dist * 2.6)` at :75. Beyond ~1.5 screen radii from the sun the result
   is under 1%. Fix: compute `glow` first and return 0 when `glow * intensity < 0.003`,
   skipping the march on 60-80% of pixels. Expected: ~1-2 ms whenever the sun is on
   screen. SMALL. ISOLATED (`assets/shaders/godrays.wgsl` only).

5. **Scale cloud view samples with segment length.** (R3#2) The march at
   `assets/shaders/pbr_simple.wgsl:2252` always takes `CLOUD_HI_SAMPLES = 48`
   (:1274) even when looking straight up through a thin slab; grazing limb rays are the
   only case needing all 48. Fix: scale the count with the ray segment `seg` (:2201)
   relative to slab thickness, clamped 16..48. Expected: +20-40% in the under-deck and
   low-flight case, no visual change from the ground. SMALL. MEGASHADER (same
   clouds.rs mirror note as item 3).

## Medium

Additional SMALL items first (not in the top 5 only because their per-item gain is
lower; all are near-free):

- **Time-budget the synchronous terrain build loop (interim fix).** (R1#1, R4#1
  interim) `src/lib.rs:15524-15570` builds up to `terrain_builds_per_frame` (default
  64, `src/config.rs:642`, `src/gui/mod.rs:6661`) patch meshes inline on the frame
  thread; each `build_patch_mesh` (`src/terrain/planet_chunks.rs:1219`) is 0.3-1 ms,
  so a descent frame can spend 20-60 ms building. Interim fix: replace the count
  budget with a time budget (stop after 2-3 ms spent this frame), or drop the default
  to ~8. SMALL. HOT FILE (lib.rs). The full fix is the worker pool under Large arcs.
- **Fix cache byte accounting to use real mesh size.** (R4#4) `src/lib.rs:15562-15568`
  inserts every patch at the constant `PATCH_MESH_BYTES` (38 KB,
  `src/terrain/planet_chunks.rs:163`), but vegetation-baked patches are ~1.8x larger,
  so the 1.5 GB cap can be 2-3 GB of real VRAM. Fix: compute bytes from
  `vertices.len() * 32 + indices.len() * 4` at insert. Two lines. SMALL. HOT FILE
  (lib.rs) but trivial.
- **Guard `land_detail_factor` octaves on zero fade.** (R3#7)
  `assets/shaders/pbr_simple.wgsl:758-771` runs 15 value-noise evaluations per land
  pixel even from orbit where every fade is 0. Add `if fade > 0.001` per octave, the
  same pattern `wave_octave` already uses at :691. ~0.3-0.5 ms in orbital views.
  SMALL. MEGASHADER.
- **Throttle machine walk-up card stat formatting to ~4 Hz.** (R1#4)
  `src/lib.rs:17925-18042` re-formats tank/battery/container stats for every machine
  every frame whenever any label exists. Values move slowly; throttle or patch only
  nearby labels. Gain scales with exactly the machine-heavy homestead dips. SMALL.
  HOT FILE (lib.rs).
- **Point-light selection without cloning.** (R1#8) `src/lib.rs:20584-20612` clones
  `room_lights` and `door_locks` wholesale and sorts all lights by distance every
  frame. Sort (index, dist) pairs instead; recompute on camera-moved threshold or
  light change. SMALL. HOT FILE (lib.rs).
- **Cache movement-speed modifiers.** (R1#9) `src/lib.rs:10456-10493` rebuilds
  effect-id and gear-id String vecs and folds two registries per frame. Cache the
  multiplier, recompute on effect or outfit change. SMALL. HOT FILE (lib.rs).
- **Reuse the object-uniform staging Vec and shortcut rigid normal matrices.** (R1#7,
  R2#9) `src/renderer/mod.rs:1477` allocates a fresh staging Vec per pass call (4-5
  calls per frame, grows past its 256 KB capacity at high patch counts), and does a
  full 4x4 inverse-transpose even for rigid transforms where the normal matrix is
  just the rotation. Keep one persistent Vec on the Renderer; branch on unit scale.
  SMALL. HOT FILE (renderer/mod.rs).
- **Cache godrays/SSAO bind groups.** (R2#8) `src/renderer/godrays.rs:183-196` and
  `src/renderer/ssao.rs:138-151` rebuild bind groups every frame; the depth view only
  changes on resize. Cache and invalidate on resize. SMALL. ISOLATED.
- **Departure eviction actually reclaims, and eviction stops being O(cache) per
  victim.** (R4#3) `collect_evictions` (`src/terrain/planet_chunks.rs:1884-1905`)
  refuses victims used in the last 120 frames, and the departure shrink
  (`src/lib.rs:15802-15833`) advances the frame by 1 before calling it, so the shrink
  to 64 MB no-ops and a departed planet parks hundreds of MB of VRAM. Each eviction is
  also a full cache scan (~2.6 M iterations per frame when riding the cap). Fix: add
  `ignore_recency: bool` for the departure path; collect eligible entries once, sort,
  evict until under cap. SMALL-MEDIUM. Mostly ISOLATED (planet_chunks.rs) plus a
  small lib.rs call-site change.
- **Throttle per-frame filesystem polls and String-keyed DataStore writes.** (R1#10)
  Camera state re-inserted under String keys three times per frame
  (`src/lib.rs:11499-11509`); up to 4 `debug/*_request.json` stat calls per frame
  (`src/lib.rs:2347, 2371, 18347, 18363`); `auto_seed_showcase` re-reads
  `data/world/showcase.ron` from disk every frame while the garden is empty
  (`src/lib.rs:1845-1853`). Poll request files every ~15 frames; cache the showcase
  config; reuse resident entries for the camera trio. SMALL. HOT FILE (lib.rs).

MEDIUM proper:

- **Shadow map cadence.** (R2#2b) The sun barely moves and the view-proj is already
  texel-snapped (`src/renderer/mod.rs:2053-2056`). Re-render the shadow map only when
  the snapped light matrix changed (sun moved > ~0.05 deg or camera crossed a snap
  cell), or every 2-4 frames, reusing the cached map otherwise. Combined with quick
  win 2 this makes the shadow pass nearly free. MEDIUM. HOT FILE (renderer/mod.rs).
- **Half-resolution cloud shell with depth-aware upsample.** (R2#1) Inside a deck the
  sky shell covers the whole 1440p frame with up to 48x8 noise samples per pixel.
  Render the cloud shell to a half or quarter-res offscreen target and composite;
  puffs and shafts are low-frequency. Expected 2.5-4x in-deck FPS, multiplicative
  with quick wins 3 and 5. MEDIUM. HOT FILES (renderer/mod.rs wiring plus megashader
  or a split shader entry).
- **Fold god rays and SSAO into one shared half-res depth-effects pass.** (R2#6,
  R2#7, R3#10) Both currently run full-res onto the swapchain
  (`src/renderer/godrays.rs:198-219`, `src/renderer/ssao.rs:115-174`). One half-res
  target, one composite, two fewer submits. ~4x cheaper for both, roughly 1.5-2.5 ms
  combined. MEDIUM. Mostly ISOLATED (godrays.rs, ssao.rs) plus renderer/mod.rs wiring.
- **Gate the ECS-to-GUI bridge rebuilds on page visibility or change counters.**
  (R1#3) `src/lib.rs:17187-17517` rebuilds inventory slots, the full sorted skill
  sheet, quests, crops, asteroids, drones, vehicles, and factory status with fresh
  String allocations every frame, unconditionally; 1000+ heap allocations per frame
  at a big homestead. The correct pattern already exists at `src/lib.rs:12233`
  (refresh only while the consuming page is open). MEDIUM. HOT FILE (lib.rs), but
  each bridge is independently gateable, so it can land incrementally.
- **Rebuild `placed_machine_types` and `home_stock` on dirty flags, not per frame.**
  (R1#5) `src/lib.rs:16788-16797` and `src/lib.rs:12665-12674` rebuild and re-insert
  full collections (including a whole-map clone for `home_stock`) every frame for
  data that changes only on edits. Keep the Mutex resident and mutate contents.
  SMALL-MEDIUM. HOT FILE (lib.rs).
- **Reuse the LOD selection when the camera is static; cache PatchBounds.** (R1#6,
  R4#8) `select_patches_sticky` runs in full twice per frame (terrain + water shell,
  `src/lib.rs:15493-15500`, :15703-15710), visiting 25-35k nodes and recomputing
  camera-independent `PatchBounds` (`src/terrain/planet_chunks.rs:685-713`) per
  visit, then rebuilds the `last_drawn` HashSet from scratch. Skip selection entirely
  under a camera-pose epsilon with no outstanding builds; cache bounds per PatchId;
  double-buffer the drawn sets. Several ms per frame at high budget; parked-camera
  cost drops to near zero. MEDIUM. HOT FILE (lib.rs) plus planet_chunks.rs.
- **Interpolate `cloud_weather` along the ray instead of per sample.** (R3#3)
  Re-evaluated inside the march (`assets/shaders/pbr_simple.wgsl:2262-2264`) though
  its finest octave is ~600 km; the three entry/mid/exit probes already exist
  (:2207-2219). Lerp them. +5-10% in deck scenes, visually neutral. MEDIUM (needs
  care not to break the probe gate). MEGASHADER.
- **Triplanar dominant-plane and dominant-material shortcuts.** (R3#4)
  `assets/shaders/pbr_simple.wgsl:2896-2924`: up to 15 array taps per terrain pixel.
  When the max plane weight exceeds ~0.8 take 1 tap per material; when the max
  material weight exceeds ~0.85 sample only that material (keep 2 at borders).
  Typical cut 15 to 4-6 taps, +10-20% close-range terrain. MEDIUM. MEGASHADER.
- **Bake the cloud ground-shadow coverage field to a small texture.** (R3#6)
  `assets/shaders/pbr_simple.wgsl:2944-2954` runs 5 procedural noise octaves per
  terrain fragment for a field that depends only on direction, time, and seed. Bake
  to a small equirect texture on a weather-clock cadence, replace with 1 tap.
  ~0.5 ms on full-screen terrain. MEDIUM. MEGASHADER plus a small renderer-side bake.
- **Cheap seabed path under a near-opaque ocean shell.** (R3#9) Terrain water pixels
  (`assets/shaders/pbr_simple.wgsl:2997-3011`) run the full 6-octave wave stack, then
  the shell (:2389, alpha >= 0.93 per :2510) shades the same pixels again; the
  terrain contribution is under 7%. Flag via a camera pad to keep only the cheap
  glint path when the shell is active; coasts keep bathymetry. ~0.5-1 ms over open
  ocean. MEDIUM. MEGASHADER.
- **Drop the fine ocean domain-warp octave for short wavelengths.** (R3#8)
  `wave_octave` (`assets/shaders/pbr_simple.wgsl:716-736`) pays 2 warp noise calls
  per octave; the fine warp is sub-pixel for the 3 shortest wavelengths. ~0.5-1 ms on
  full-ocean views. SMALL-MEDIUM (visual check needed). MEGASHADER.
- **Stop streaming and drawing the full planet while inside the home ship.** (R4#2)
  `chunked_on` (`src/lib.rs:15371-15376`) is permanently true at the ~400 km home
  orbit, so selection, builds, prefetch, up to 6144 patch draws, and the shadow pass
  all run every frame while the interior pass paints over them
  (`src/lib.rs:20642, 20660-20683`). Add a cheap camera-enclosed flag from
  home_structure; while enclosed drop `max_leaves` to a few hundred, skip prefetch,
  pause builds, keep the last selection. Removes thousands of draws in the single
  most common play position. MEDIUM. HOT FILE (lib.rs).
- **Vegetation interim: bake trees only at depth 15-16 and grass only at 18.** (R4#5
  interim) Deep patches (depth 17-20) currently re-scan ~700 candidates to keep ~0
  and re-bake the same tree into up to 6 cached meshes
  (`src/terrain/planet_chunks.rs:1429-1625`). Depth-limiting the bake cuts build cost
  at walking depths and shrinks cache bytes. MEDIUM. ISOLATED (planet_chunks.rs).
  Full fix under Large arcs.

## Large arcs

- **Worker-pool terrain mesh building.** (R1#1, R4#1) Move `build_patch_mesh` (pure
  CPU by design) to a small thread pool; the frame thread only uploads finished
  vertex data, capped per frame. Also hoist the per-tap tile HashMap ref across the
  16 taps of one bicubic sample (`src/terrain/terrain_tiles.rs:237-262`). This is the
  best single explanation for surface FPS and descent hitches; the time-budget quick
  fix above buys breathing room until this lands. LARGE. HOT FILE (lib.rs build loop)
  plus planet_chunks.rs.
- **Terrain draw submission batching.** (R1 cross-cutting, R2#3, R4#7) 1500-6144
  separate micro-draws per frame, twice with shadows on, each with dynamic-offset
  bind plus VB/IB rebind (`src/renderer/mod.rs:2179-2207`, :2108-2132). Staged plan:
  (a) drop per-patch index buffers, they are pure identity 0..n
  (`src/terrain/planet_chunks.rs:1365, 1388, 1455, 1661`), use `draw(0..n)`, saving
  ~11% patch memory and ~12k binds per frame (SMALL-MEDIUM on its own); (b) merge
  built same-depth sibling quads, up to 4x fewer draws; (c) shared slab
  vertex/index buffer plus `multi_draw_indexed_indirect`
  (`wgpu::Features::MULTI_DRAW_INDIRECT`, native Vulkan/DX12). Expected +15-30% at
  high patch counts and makes the shadow pass nearly free. LARGE. HOT FILES.
- **Single-encoder frame, one submit.** (R2#5) ~12 `queue.submit` calls per frame,
  forced by every pass rewriting the one shared camera buffer
  (`src/renderer/mod.rs:1992-2036` and pass sites listed in R2). Give each pass its
  own 256-byte dynamic-offset slot in one camera buffer and disjoint object-buffer
  ranges, record all passes into one encoder. ~0.5-1.5 ms CPU plus GPU idle bubbles.
  LARGE. HOT FILES (renderer/mod.rs, lib.rs).
- **Per-cell vegetation overlay meshes.** (R4#5 full) Bake vegetation once per cell
  into standalone overlay meshes keyed by cell id, drawn alongside whatever terrain
  depth is resident; terrain builds skip vegetation entirely. Kills duplicate
  scan/bake cost, shrinks cache bytes, stops LOD splits re-uploading identical
  forests. LARGE. Mostly ISOLATED (planet_chunks.rs) plus draw-list wiring in lib.rs.

## Measurement notes

- **Boot-verify every renderer/pipeline/shader change** (incident v0.782-784): tests
  and naga validation do not catch device-limit or startup failures. Bar: release
  build, launch `target/release/HumanityOS.exe`, wait ~10 s, grep
  `%APPDATA%/HumanityOS/logs/run.log` for PANIC, then kill.
- **Benchmark scenarios** (fixed camera via F6 bookmarks + `debug/camera_request.json`,
  capture via `debug/screenshot_request.json`): (a) planet surface walk at noon,
  shadows on; (b) inside a full High cloud deck; (c) homestead interior with machines
  running; (d) descent from orbit (hitch/1% lows, not average FPS); (e) parked camera
  on surface (should approach zero streaming cost after the selection-reuse fix).
  Use the portable perf probe and the draws diag (draws=6144 confirms the interior
  case) for before/after on every item.
- **Measure one change at a time** on the same bookmark; several items overlap (quick
  wins 1-2 and the shadow cadence all shrink the same pass), so stacked estimates do
  not add linearly.
- **Shader constants are mirrored**: cloud march constants live in both
  `assets/shaders/pbr_simple.wgsl` and `src/renderer/clouds.rs:779-813` (asserted
  ranges). Change both sides together or the debug assert fires.
- **Do not re-add early-outs that already exist** (verified working, R3): cloud
  clear-sky 3-probe gate (pbr_simple.wgsl:2207-2220), march break at trans <= 0.02
  (:2297), light-march density skip (:2272), carve rejections (:2019, :2051, :2069),
  shadow-map bounds rejection (:146), wave-octave fade early-out (:692), Rust-side
  godray night skip.
- **Non-findings, do not chase** (R2#10): depth/shadow formats, no MSAA, bloom not in
  the frame loop, MAX_OBJECTS headroom, and the bound_material elision are all fine.
  Cosmetic only: `let mut bound_material` declared twice back-to-back at
  `src/renderer/mod.rs:1791-1792`, :1861-1862, :2177-2178 (harmless shadowing).
- **Landmine noted in passing** (R2#9 side note): `render_instanced`
  (`src/renderer/mod.rs:2473-2486`) writes the transform buffer inside an open pass
  per instance, so all instances would render with the last transform. Unused in the
  live frame path; fix before anyone reaches for it.
