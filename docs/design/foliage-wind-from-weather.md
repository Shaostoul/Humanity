# Foliage wind driven by the real weather

Status: SHADER HALF IMPLEMENTED (2026-07-31, `assets/shaders/pbr/00-bindings-vertex.wgsl`
lines 577-690), PUBLISHER HALF PENDING as a wiring request. Independent of
`docs/design/vegetation-cards-every-species.md`: different files, no conflict,
the two can run in parallel.

## What was wrong

Foliage was the last consumer in the engine still faking the wind. Cloud
advection consumes the live field (v0.1032), weather-event Front gusts export
onto it (v0.1035), and the ocean spectrum is driven by it (v0.1050), while
plants ran on three hardcoded constants.

Three separate defects, all in the type-20 vertex branch:

1. **The wind vector was a constant in OBJECT space.** Line 594 was
   `normalize(vec3(0.86, 0.0, 0.32))`, and the comment at line 570 says object
   space explicitly. Instance yaw is a full random turn
   (`src/terrain/planet_chunks.rs:1883`, `az = (r2 % 6283) / 1000.0`), so every
   tree's wind direction was its own random azimuth and a stand FANNED OUT
   instead of leaning together. This is the most unphysical part and no amount
   of amplitude tuning fixes it.
2. **No static lean at any speed.** The displacement was
   `sin(t * 0.9 + phase) * 0.035 * h * gust`, zero-mean. Real drag on a crown
   goes as v^2, so a stand holds a mean downwind deflection and oscillates ABOUT
   it; at Beaufort 8 (17-20 m/s, inside the storm range) the textbook
   description is literally "whole trees in motion". `WeatherState::wind_speed`
   exists at `src/systems/weather.rs:35` and reaches 10-25 m/s on Storm and
   Hurricane rolls (`weather.rs:284` and `:305`). Nothing in the foliage path
   read it.
3. **The gust was a standing wave.** `0.65 + 0.35 * sin(t * 0.23 + phase * 0.5)`
   with `phase` derived from position and no space-time coupling: it pulsed in
   place instead of advecting downwind, so there was no gust front crossing the
   canopy.

Evidence that a still frame is enough here: mean deflection is a static
quantity. `.probe-rig/sweeps/20260731-165225/ground-storm-inslab.png` (12-25 m/s
storm) is indistinguishable in tree pose from
`.probe-rig/sweeps/20260731-165006/fuji-forest-ground.png` (calm) at the same
coordinates. Every trunk plumb, every crown centred over its own base.

## The uniform slot, and why it is legitimately free

`light7_cone_inner`, byte offset 576 in the camera uniform buffer (light5 is
544, light6 is 560).

The `light0..7_*` uniform fields are LEGACY. The struct comment at
`assets/shaders/pbr/00-bindings-vertex.wgsl:75-80` says so: real scene lights
moved to the uncapped `scene_lights` storage buffer in v0.782, and nothing reads
`lightN_cone_inner` as a light any more. That is exactly why light4 already
carries the FFT ocean anchor and the weld K, light5 the sea crest and the
underwater extinction, and light6 the sea sphere.

One correction to the survey that proposed this slot: it is NOT strictly
unwritten. `Camera::uniforms_with_lights` (`src/renderer/camera.rs:472-478`)
still fills `light_cone_inner[i] = [cos_inner, 0, 0, 0]` for up to 8 room
lights, so with 8 lights present the stamp puts a cosine in `light7.x`. This
does not block the repurposing (nothing reads it), but it does dictate WHERE the
poke goes, and getting that wrong produces a bug that only appears indoors or in
one pass.

No bind-group-layout change, no new binding, no new pipeline. That is
deliberate: it avoids the v0.1029-v0.1038 incident class entirely.

## WIRING REQUEST (the publisher half, not yet applied)

Two edits, both outside the vegetation domain's owned paths.

### A. `src/lib.rs`, the weather-to-renderer block

Anchor: the existing sea-state and cloud-advection publishers, around
`src/lib.rs:13090-13110`, where `w` (the live `WeatherState`) and
`state.renderer` are both in scope.

Insert alongside them:

```rust
// Foliage wind (v0.1079): the type-20 vertex branch reads
// light7_cone_inner as (wind_dir_world.xyz, wind_speed_m_s).
// WeatherState::wind_direction is a LOCAL tangent-plane compass
// vector (weather.rs:325 builds it as (cos a, 0, sin a)), so it must
// be lifted into world space through the player's own east/north
// basis or the shader receives a mostly-VERTICAL vector at any
// latitude away from the equator and the lean collapses. Basis
// construction mirrors the card emitter exactly
// (terrain/planet_chunks.rs:1884-1885).
let up_l = <player radial up, unit>;
let east = glam::Vec3::Y.cross(up_l).normalize_or_zero();
let north = up_l.cross(east).normalize_or_zero();
let wdir = (east * w.wind_direction.x + north * w.wind_direction.z)
    .normalize_or_zero();
state.renderer.foliage_wind = [wdir.x, wdir.y, wdir.z, w.wind_speed];
```

`foliage_wind: [f32; 4]` is a new public field on `Renderer`, defaulting to
`[0.0; 4]` so an unpublished frame takes the shader's fallback.

### B. `src/renderer/mod.rs`, the CELESTIAL-pass uniform stamp

Anchor: the sea-sphere poke, currently

```rust
        // Sea sphere in light6_cone_inner.xyzw (offset 560), v0.1061.
        self.queue
            .write_buffer(&self.camera_buffer, 560, bytemuck::cast_slice(&self.sea_sphere));
```

Insert immediately after:

```rust
        // Foliage wind in light7_cone_inner.xyzw (offset 576), v0.1079:
        // xyz = world wind direction (unit), w = speed m/s. The type-20
        // vertex branch rotates it into object space and leans the stand.
        self.queue
            .write_buffer(&self.camera_buffer, 576, bytemuck::cast_slice(&self.foliage_wind));
```

**It must go in THIS block and not in the main-pass stamp.** Planet-surface
trees are pushed as `celestial_objects` (`src/lib.rs:9628`), so they are drawn
in the celestial pass, and that pass stamps `celestial_uniforms()` over the
whole buffer first. That is the identical trap the fill-light comment at
`renderer/mod.rs:2786-2795` documents: a value set once per frame elsewhere is
silently discarded here. It is also what makes the `uniforms_with_lights` write
harmless, because the poke lands after the stamp.

If the same wind is later wanted for crops drawn in the main pass, the poke has
to be repeated against that stamp too.

## The shader half (already applied)

`assets/shaders/pbr/00-bindings-vertex.wgsl`, the type-20 branch.

- **Reads** `camera.light7_cone_inner` as (dir_world.xyz, speed).
- **Falls back** to a 4 m/s breeze along the old constant direction when the
  slot is zero or the speed is non-positive. This is not defensive padding, it
  is what keeps the un-wired intermediate state from being a REGRESSION: a zero
  wind speed would freeze every plant on the planet, which is strictly worse
  than what shipped, and the shader lands before the publisher.
- **Rotates world to object** with `transpose(obj_normal_matrix())`, the same
  model-inverse identity the water branch uses two blocks below and the leaf
  detail pass uses at `90-fragment-main.wgsl:1179`. Because the displacement is
  applied BEFORE `obj_model()`, undoing the instance yaw here is exactly what
  makes every tree lean the same compass direction.
- **Zeroes the object-Y component** and renormalizes. Object +Y is the trunk
  axis by construction, so the lean is purely tangential: trees bend across the
  ground, never into or out of it, at any latitude. Degenerate case (wind along
  the trunk axis) picks an arbitrary tangent rather than producing NaN.
- **Static lean** `lean_m = h * hn * min(6.0e-4 * v^2, 0.30)` with
  `hn = clamp(h / 12.0, 0.1, 1.0)`. `hn` is a cantilever profile normalised
  against a 12 m reference tree, which is what stops a 22 m fir folding flat
  while an 8 m cherry barely moves; the 0.30 cap stops a 25 m/s hurricane roll
  laying the stand down. Worked numbers: an 8 m sakura tip displaces about
  0.05 m at 4 m/s (visually upright) and about 1.0 m at 18 m/s, inside the 1-3 m
  a real 8 m crown holds at Beaufort 8.
- **Sway about the lean** with a wind-scaled amplitude plus a small calm-air
  term, so a still day still breathes rather than freezing, and total
  displacement stays downwind at storm strength instead of swinging back past
  vertical.
- **Travelling gust.** `travel = dot(o.xz, K) - |K| * v * t`, so crests advect
  downwind at the wind speed. K is snapped to integer harmonics of TAU/64 by
  rounding the horizontal wind onto a x2 lattice.

  **This snapping is not optional.** The per-plant phase at lines 589-591 is
  built from integer harmonics of TAU/64 precisely because the model
  translation is RENDER space and re-snaps on every floating-origin rebase; an
  arbitrary `dot(o, wind_dir)` is not 64 m-periodic and every rebase would pop
  the whole forest. Same discipline as the ocean chop trains (CLAUDE.md,
  f32-at-planet-scale). The snapped lattice gives a 32-64 m gust wavelength,
  which is in the right band for a canopy wave and far finer in angle than
  anyone can read off a treetop.
- **Leaf flutter** now scales with speed through `fv = clamp(v / 6.0, 0.35, 3.0)`.
  Leaves are the first thing to show a rising wind and the last to stop.

Cost: 4 floats per frame into an existing uniform slot, plus roughly a dozen ALU
and one 3x3 rotate in a vertex branch that already ran. The wind branch only
runs for material type 20, which is procedural plants, trees and crops, not the
terrain cards (type 12) and not the photoscans. At `fuji-forest-ground` that is
about 194 procedural trees at ~11k vertices, so ~2.1M vertices per pass, ~4.2M
with the shadow pass. Well under 0.05 ms on this class of GPU, and below the
run-to-run spread of any A/B the rig can resolve. This is the highest
realism-per-unit-work item on the vegetation list.

## Verification status

- `cargo test --features native --lib shader_loader`: 3 passed. That is full
  naga parse plus validation of the assembled megashader AND of the
  terrain-batch variant (`batched_variant_parses_and_validates`), which is the
  variant that swaps the object-source block, so `obj_normal_matrix()` is
  confirmed to resolve in both modules.
- `cargo check --features relay --no-default-features`: clean.
- **NOT run: the probe rig.** No before/after capture exists for this change.
  The structural failure classes (device limits, bind-group mismatch) are ruled
  out by construction because nothing here adds a binding, a pipeline or a
  layout entry, and compile failure is ruled out by the naga tests, but whether
  the lean READS right at 12-25 m/s is unmeasured. Run
  `node scripts/probe-sweep.js --only ground-storm-inslab` and `--only
  fuji-forest-ground` against a release build before calling it done, and judge
  it on the two regression lines added to `tests/visual/vantages.json`.

## Follow-up, deliberately not part of this task

Cards do not sway. The wind branch is type-20 only, so a tree sways until you
back away past the card-hide radius (154 m at the rig vantage) and then freezes
as it becomes a static card.

Once every species is a sprite card
(`docs/design/vegetation-cards-every-species.md`), the same MEAN LEAN can be
applied to cards using the `v01` already encoded in the sprite-card colour
channel (`planet_chunks.rs:1721`, decoded at `90-fragment-main.wgsl:813`): shear
the card's upper corners downwind in proportion to `v01^2` and the identical
`lean_frac` formula, and the lean becomes continuous across the model-to-card
handoff. That is what closes the last visible seam in the wind, and it is a
separate increment that depends on this one AND on the cards one.
