# Water FFT: the Tessendorf ocean

Status: DESIGN for operator review, 2026-07-28. Hyper-realism rock #1
(PRIORITIES roadmap; companion: weather-water-roadmap.md). Replaces the
six hand-tuned wave trains with a statistical sea computed from a real
oceanographic spectrum - the technique behind Sea of Thieves, AC, and
most modern game oceans.

## Why FFT beats more hand trains

The 6-train sum gives six wavelengths. A 128x128 FFT gives ~16,000
simultaneously, with the ENERGY DISTRIBUTION real oceans measure
(short chop rides long swell in the statistically correct ratio), and
its byproducts are exactly the maps we currently fake by hand:
displacement (geometry), slope (normals), and the Jacobian (foam where
waves pinch - whitecaps emerge from the math instead of a threshold).

## Spectrum: JONSWAP, wind-driven

- JONSWAP over Phillips: fetch-limited peaks, less low-frequency mush,
  one extra parameter (peak enhancement gamma ~3.3).
- Parameterized by wind speed U10 + direction from the WEATHER SIM (the
  HUD wind), with directional cos^k spreading about the wind vector.
  This is the operator's "clouds move at varying rates depending on the
  weather" principle applied to the sea: calm mornings glass out, storm
  wind builds real steep seas, and the transition is physical.
- Regional variation: the spectrum/FFT field is GLOBAL per cascade;
  the existing sea_state machinery (live MODIS storm cells) keeps doing
  what it does today - modulating local AMPLITUDE - so a storm cell
  still darkens and roughens its own patch of ocean.
- Wind changes REGENERATE h0(k) when the delta is material (> ~1.5 m/s
  or > ~20 deg), crossfaded over ~5 s so the sea never snaps.

## Tiling and the f32 discipline

Cascade tiles must respect the camera-anchored 64 m-modulus domain
(CLAUDE.md f32-at-scale gotcha - the anchored-chop precedent).

CORRECTION (v0.1029, found during increment 1): the tile must DIVIDE the
64 m anchor modulus, not be a multiple of it. ground_anchor snaps in
exact 64 m steps, so a 256 m tile would shift by a QUARTER tile per snap
(a visible sea jump); a 64 m tile shifts by exactly one whole period
(invisible). So:

- Cascade A (shipped, increment 1): 64 m tile, 128x128 -> 0.5 m texels.
  Replaces the three anchored chop trains; the three long swells
  (> 64 m wavelength) stay analytic in both modes.
- Cascade B (chop, increment 3): 32 m or 16 m tile (divide 64), finer
  texels for sub-half-metre ripple.
- A LONG-swell cascade (256 m) needs its own mod-256 anchor uniform -
  one extra vec3 pad, increment 3 work.
- Shader samples displacement at (anchw + dvw) / tile via triplanar
  projection (a single 2D plane degenerates at the equator; three
  axis planes blended by radial^2 cover every latitude, matching the
  trains' three axis-aligned directions). Anchor snaps shift UVs by
  whole tiles. No planet-radius f32 dots anywhere in the path.

## Drawn == sampled, stronger than ever

The FFT runs on the CPU (worker thread) in increment 1; the buoyancy
twin BILINEARLY SAMPLES THE SAME ARRAY the GPU texture is uploaded
from. Physics and pixels cannot disagree because they are the same
numbers - stronger than today's re-derived cos-sum twin. Determinism:
h0(k) seeds from terrain_seed, so every client computes the identical
sea (multiplayer-honest without sync traffic).

## Performance envelope

128x128 complex IFFT x 2 (height + one gradient pair packed) is well
under a millisecond on a worker thread; the upload is 128 KB/frame.
GPU compute (increment 4) removes even that and unlocks 512x512. The
existing light_tiles.rs compute pass is the wgpu compute precedent.

## Migration path (no rug-pulls)

- Settings > Graphics > Water simulation: "Wave trains" | "FFT ocean"
  (the cloud-quality-tier precedent). Trains remain the fallback tier
  and the low-end path; their lockstep tests stay untouched.
- The FFT path gets its own drawn==sampled test (trivially strong: the
  test samples the same buffer the upload reads).
- The type-16 shader keeps its shading/foam/shore machinery; increment
  1 only swaps WHERE vertex displacement comes from.

## Increments (each shippable)

1. CPU 128^2 JONSWAP -> vertical displacement texture; VS samples it in
   the anchored domain; buoyancy samples the same array; behind the
   settings toggle, trains stay default. (One session.)
2. Choppy (horizontal) displacement + slope maps from the same spectrum
   -> replaces the analytic shading gradient; Jacobian foam mask ->
   replaces the threshold whitecaps. FFT becomes the default tier.
3. Second cascade (32 m chop) + retire the near-chop trains.
4. GPU compute FFT (512^2, frees the worker).
5. Depth buffer for water: screen-space reflections + refraction (the
   Subnautica look; also enables real underwater god rays later).
6. Shore-wave simulation (depth-driven breaking, foam advection) - the
   weather-water roadmap's beach payoff.

## Operator fork points (the taste calls)

1. Sea character default: JONSWAP fetch ~200 km reads "open Pacific";
   shorter fetch reads "coastal chop". Tunable live; pick a default.
2. How calm is calm: true glass at 0 wind, or keep a residual 0.5 m swell
   so the ocean never looks frozen? (Recommend: tiny residual swell.)
3. Storm ceiling: JONSWAP at 25 m/s wind is genuinely violent (8 m+
   significant height). Cap for playability or let hurricanes be
   hurricanes? (Recommend: let events own the extremes.)
