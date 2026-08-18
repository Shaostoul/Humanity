# Cloud Depth: why the deck reads flat, and the fenced fix

> Operator report (2026-08-17, from 1.5-6 km over Silverdale): "can we
> increase the resolution for the cloud layer so they have more visual
> depth?" Fidelity-expert investigation, same day, measured on the probe
> rig at the operator's settings. VERDICT UP FRONT: it is NOT a texture
> resolution problem. Do not grow the noise volumes.
>
> **STATUS: PHASE 1 SHIPPED (v0.1156); PHASE 2 = the underside.** All six
> structural steps below are in: physical per-planet slab
> (`cloud_base_km`/`cloud_top_km` in earth.ron, carried to the shader in
> the new `material.params2` vector), the metric noise ladder
> (`*_TILE_KM`/`*_KM` constants via `g_cloud_upkm`), metric extinction
> (`CLOUD_*_SIGMA_KM` - the old per-drawn-unit sigmas left the thin slab
> invisible from orbit), CLOUD_BASE_DROP, eroded-density light-march taps,
> depth-aware ambient + ground bounce + third-octave sigma 0.20 + tau cap
> 16, family bands re-authored in km, Medium on the physical slab, and the
> vantages + dev pins (showcase `cloud_quality`, `cloud_cover` - the pin
> also sets params2.w = 1 so cloud_weather ignores live MODIS placement).
> The ORBITAL look is verified good with live MODIS.
>
> **PHASE 2 (next, the actual operator-report surface): from-below
> presence.** Probe-bisected on the rig (sweeps 20260818-0227..0241): the
> pre-erosion carve is strong from below (diagnostic green), but the
> density that survives erosion stays near zero at under-deck distances -
> the carve thresholds, erosion strengths, and sample budget were all
> calibrated against the 51 km slab. First fixes already in (fine-band
> edge protection - it was the only band eroding CORES at full strength -
> and a 2x slant-ray sample budget); the remaining calibration must hit
> the acceptance gates below ON the silverdale-flight-2km capture.
>
> **Harness rules learned the hard way (2026-08-18):** the exe compiles
> shaders from EMBEDDED include_str! sources - a shader-only edit needs
> `--reload-shaders` on the probe sweep (or a rebuild) to reach the GPU;
> naga REJECTS a bare `return` diagnostic above live code
> (InstructionsAfterReturn - wrap it in an `if`); and the rig's assets/
> junction can silently degrade to a stale real directory (self-heal now
> in probe-sweep.js ensureJunction). Two sessions of cloud probes were
> invalidated by exactly these three traps.
> Gate measurements: `node scripts/measure-cloud-depth.mjs <capture.png>`.

## What ships today (do not rebuild)

`cloud_layer_volumetric` (assets/shaders/pbr/40-clouds.wgsl:1277) is a
genuine Nubis-recipe raymarcher: 192^3 Perlin-Worley SHAPE + 128^3
DETAIL volumes (src/renderer/cloud_noise.rs:33,:35 - both ABOVE Nubis's
shipped sizes), exponential view march (48 max), 8-tap light march,
dual-lobe HG phase, Beer-powder, 3-octave multi-scatter, 7 families,
live MODIS placement, domed tops / crown shading / puff lobes.

## The three findings (measured, file:line cited)

**1. The deck is 10-50x too high, for a deleted reason.**
CLOUD_BASE_SCALE 1.004 / TOP 1.012 (40-clouds.wgsl:124, mirrored
src/renderer/clouds.rs:78) put the slab at 25.5-76.5 km. The
justification (clouds.rs:60-69) cites a 4x terrain vertical
exaggeration that data/planets/earth.ron:83 DELETED at v0.883.2
(surface_relief 0.011 -> 0.003123; true peak displacement 7.4 km).
Every family renders 10-50x above its real altitude band, so all seven
read as one distant grey ceiling; the finest modelled feature (1.44 km
puff cell) subtends fractions of a degree; view-march steps reach
10 km against that 1.44 km feature (3-7x undersampled); aerial haze
washes everything at 20-250 km slant range.

**2. The underside is a level plane the light march cannot shade.**
Measured pre-ACES radiance range across the from-below deck: 2.2-2.8x,
where two-stream physics gives 7.6x from thickness alone (tau 22 vs
225). Causes: cloud_carve (:1097) has CLOUD_TOP_RISE but NO symmetric
base term (every column's base sits on one iso-surface, ramped over
14.5 km of slab); cloud_density_light (:1211) samples the UN-ERODED
body only, so fray/detail/puff carve the silhouette but cast zero
self-shadow (proof: from-above captures have 6-15x the structural
detail energy of from-below on the same build - all shipped structure
is top-surface); the third scatter octave exp(-tau*0.06) with tau
capped at 10 (:1253) is effectively a constant 0.14 luminous floor,
and ambient (:1437) is slab-fraction, not cloud-depth, with no
ground-bounce term.

**3. "Resolution" is world-space frequency + sampling, not texels.**
The frequency ladder is expressed in drawn-shell units whose radius
itself flips 1.016 <-> 1.008 R with camera altitude (src/lib.rs:11327),
so nothing is anchored to a physical length. Light-march first tap =
906 m (CLOUD_LIGHT_NEAR :228): no shading detail finer than that can
exist, ever.

## The increment (one increment, this order - 1 alone exposes the flat
underside up close; 2 alone buys nothing at 20 km)

1. `cloud_base_km` / `cloud_top_km` in data/planets/earth.ron
   (Infinite-of-X, per-planet); slab bounds from the material, family
   table re-authored in km. Files: earth.ron, clouds.rs (constants +
   mirrors + tests :751/:794), 40-clouds.wgsl :123-125/:989-990,
   lib.rs:11327-11351.
2. Frequency ladder + fade distances re-expressed in METRES
   (40-clouds.wgsl :254,:258,:266,:277,:279,:347).
3. Base-height field in cloud_carve (CLOUD_BASE_DROP symmetric to
   CLOUD_TOP_RISE, driven by the SHAPE volume's G channel).
4. cloud_density_light samples fine + puff bands for its FIRST 2-3 taps
   only (the single change that makes lobes read as lobes).
5. Depth-based ambient with a ground-bounce term; third octave sigma
   -> ~0.25; raise the tau cap.
6. New vantage silverdale-flight-2km (lat 47.645, lon -122.6925,
   2.0 km, look 115, rain, time 20.2) + a Medium-tier in-slab vantage
   (Medium is the BUG-049 surface and has ZERO rig coverage today; the
   Medium path deliberately keeps static constants, 40-clouds.wgsl
   :174-181 - moving the constants changes Medium silently).

## Acceptance gates (measure, do not eyeball)

- Parallax: two captures 500 m apart at 2 km altitude must shift a
  cloud base >> 1.2 deg (23 km backdrop) - target ~27 deg (1 km base).
- Cloud-only mask, recovered pre-ACES radiance p95/p05 >= 6.0
  (measured v0.1152: 2.19-2.81).
- Underside high-pass detail energy >= 0.05 at sigma 11 px and >= 0.08
  at sigma 32 px (measured v0.1152 from below: 0.0093 / 0.025; from
  above on the same build: 0.143 / 0.206).
- NO cloud slab above 12 km on Earth; NO BUG-049 rings at any altitude.

## Do NOT

- Do not grow cloud_noise.rs SHAPE/DETAIL sizes (already above Nubis).
- Do not touch the per-fragment ACES (:1486) as a first move: the
  shader only PRODUCES 2-3x range; fix the range first, then price a
  scene-wide HDR tonemap as its own operator-approved pipeline change.
- Do not re-close the light-march blindness by widening CLOUD_PUFF_AO
  (:286): it is a noise multiplier, not occlusion.

## Operator taste call, flagged not decided

`weather: "cloudy"` can render a cloudless sky when live MODIS has no
cloud over the area (honest real weather vs the label disagreeing).
Should "Cloudy" force local coverage? Design decision for the operator
when this increment is fenced.

## Sources

Nubis^3 (Guerrilla, SIGGRAPH 2023 Advances); Horizon Forbidden West
Burning Shores clouds (PlayStation Blog 2023); Schneider, Nubis
Evolved; two-stream similarity transmittance for plane-parallel
clouds (J. Atmos. Sci. 72(11)).
