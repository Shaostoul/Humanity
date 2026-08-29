# ABYSSAL (Token-Gremlin/natural-disasters): adoption notes

**What it is:** an MIT-licensed, ~400 KB Three.js/WebGL2 procedural ocean +
extreme-weather simulator (https://github.com/Token-Gremlin/natural-disasters,
live demo at token-gremlin.github.io/natural-disasters). Built directly on the
canonical published techniques: Tessendorf FFT ocean, Hillaire 2020 atmosphere,
Horizon Zero Dawn volumetric clouds, Karis TAA, Frostbite EV100. Exceptionally
well-commented; the comments explain WHY, with failure modes named.

**License:** MIT. Formulas, constants, and code are all freely portable with
optional attribution.

**Assessment date:** 2026-08-28. Full deep-read (9 readers, both codebases) in
the session journal; this file keeps the durable conclusions.

## Verdict in one paragraph

Our base architecture is already at or beyond theirs where we have invested:
real sphere instead of their faked parabolic curvature, Hillaire transmittance +
multi-scatter Psi + sky-view LUTs, Nubis-class constructed cloud bodies with 7
families and live NASA weather, a wind-driven JONSWAP CPU-FFT ocean with
Jacobian foam calibrated to the same Monahan whitecap formula they use, and the
drawn==sampled buoyancy twin (their "keep GLSL in step with Director.eventHeight"
comment is literally our lockstep discipline, independently converged). What
ABYSSAL has that we lack is the ENTIRE disaster/event layer on water, ocean
spray/rain quality, and several worked shading refinements our own gap list
already names. Because their disaster fields are closed-form analytic
height-field modifiers dual-evaluated CPU+GPU, they drop into our architecture
with near-zero impedance.

## Tier 1: fills holes we have nothing for (highest value)

1. **Analytic disaster height fields** (`src/ocean/OceanSampleGLSL.js`,
   `src/weather/Director.js`). Tsunami = sech^2 soliton with the leading
   coordinate compressed `x*(1+1.35*steep)` (asymmetric shoaling face) plus a
   drawdown trough AHEAD of the crest (the receding-sea precursor), crest-lip
   foam, and a `profile^2`-concentrated forward overhang push. Rogue wave =
   3-mode Gerstner group under a Gaussian envelope, crest-sharpened
   `sign(g)*|g|^0.72`, wavelength 420 m paired with 26 m/s (deep-water
   dispersion consistent). Whirlpool/maelstrom = Rankine vortex depression, with
   swirl done by ROTATING THE WAVE-TEXTURE LOOKUP COORDS by a time-growing
   Rankine angle (existing wave detail spirals in, zero extra simulation).
   Hurricane = Gaussian eyewall swell ring at r = 1.25*eye minus a glassy-eye
   calm term that also suppresses foam/chop. All state fits in a handful of
   vec4 uniforms (4 vortices, 2 solitons, 1 rogue, 1 hurricane). Each field is
   evaluated identically in shader and on CPU so the camera/buoyancy rides the
   drawn surface. This is exactly the house twin pattern; it would finally give
   `src/systems/disasters.rs` (currently written but NOT registered, zero
   visuals) real water-coupled events. Foam-from-shear saturates at
   `min(k*strength, 0.62)`: already-broken water cannot get whiter.

2. **Branching lightning** (`src/weather/Lightning.js`). CPU recursive midpoint
   displacement (depth <= 5, min segment 40 m, jitter decay 0.55/level), forks
   with p=0.42 below depth 3, in-cloud spider-crawl branches, and 1-4 return
   strokes with amplitude 0.62^i and flicker envelope `(1-u)^1.7*(0.75+0.25
   sin(61u))`: that stutter is what reads as real lightning. Rendered as
   instanced camera-facing ribbon quads (core + glow layers), distance-widened
   `1 + 0.0016*d` so far bolts never sub-pixel. The two strongest bolts are
   exported as point lights so ocean/cloud/sky flash coherently. We have ZERO
   lightning anywhere (data/weather/events.ron thunderstorm event has no
   visual); this is a compact, fully portable generator.

3. **Waterspout / tornado condensation funnel** (`src/weather/Waterspout.js`).
   Raymarched analytic funnel (56 steps in a proxy box bounded by one analytic
   cylinder quadratic): wall = condensation at radius of max wind with hollow
   core, spray cascade at the waterline, detail noise sampled in the ROTATING
   vortex frame with angular speed ~ 1/radius (angular-momentum conservation
   makes the neck spin fastest), dual-HG forward phase so a spout against the
   sun becomes a bright pillar, rope-out decay longer than spin-up. Our tornado
   weather event currently places a core and shows a HUD warning only. This +
   the Rankine ocean vortex = a complete visible tornado/waterspout.

4. **Crest spray particles** (`src/weather/Precipitation.js` Spray). GPU
   particle state (pos+age, vel+seed), dead texels try 3 spawn candidates per
   frame, accepting where the ocean's turbulence/breaking measure at that point
   exceeds a roll: spray is physically anchored to breaking crests. Up-kick
   `3.5 + breaking*9.0` m/s, wind drag `mix(0.35, 2.6, seed)` (small droplets
   couple faster), forward-scatter phase `0.55 + 1.9*mu^6` (glows backlit). We
   already have a GPU particle sim (`src/renderer/particles_gpu.rs`) and an FFT
   foam field to spawn from; ocean spray is entirely missing today.

## Tier 2: upgrades to systems we already have

5. **Dual-criterion foam** (`src/ocean/OceanFFT.js` ASSEMBLE_FRAG). Our FFT foam
   is Jacobian-only. Their second criterion catches SPILLING breakers the
   Jacobian misses (Jacobian only fires past self-intersection, which a
   physically scaled chop never reaches, "a gale looks glassy"): steepness
   past the Stokes H/L = 1/7 limit, weighted by a leeward-face factor
   `0.55 + 0.45*dot(grad_dir, wind)` and gated by being high in the band
   (`Dy * 16*PI/lengthScale`). Foam integrates an entrainment RATE with decay
   (equilibrium = rate*duty/decay), never snaps to 1 ("storm sea becomes a
   snowfield"). Also: a separate BUBBLES channel (raft lingers tens of seconds
   after the whitecap; decay 0.12/s vs foam 0.35/s) and a spray-seed channel.
   Relevant beside `src/terrain/ocean_fft.rs` foam pipeline (L350-461).

6. **Backlit crest subsurface scattering** (`src/ocean/OceanMesh.js`). Our gap
   list names missing SSS explicitly. Their worked formulation: `backlit =
   heightNorm * thinness * dot(L,-V)^4 * (0.5 - 0.5*dot(L,N))^3`, scatter =
   bodyR * sun * backlit * 3.4, with the hard-won lesson that event/disaster
   height must be EXCLUDED from the thinness proxy (`1/(1 + eventY*0.075)`),
   or a tsunami face becomes "a slab of jade". Backface fragments flip N and
   switch to an ABSORPTION-driven transmission model (scatter-albedo-driven
   interiors read as black holes).

7. **Energy-consistent roughness LOD** (`OceanMesh.js` fragment). Cox-Munk
   total mss `0.003 + 0.00512*U`, apportioned per cascade (0.06/0.30/0.64);
   whatever slope variance the chosen mip filtered out converts to GGX alpha
   `sqrt(2*mssUnres)` on the geometric-mean footprint. Sub-pixel waves BECOME
   specular roughness, so the horizon neither speckles nor goes mirror-flat.
   We already use Cox-Munk in subtraction form (`20-surface-detail.wgsl`
   L1407-1467); theirs is the fuller mip-integrated treatment. Bonus: widen
   the sun GGX lobe by half the solar angular radius (aP = a + 0.00465/2,
   renormalized) for a stable glint.

8. **Closed-form rain + squall curtains** (`Precipitation.js` Rain). 100%
   stateless vertex-shader rain (position = f(hash(index), t) with modulo
   wrap): 180k drops at ultra with zero CPU and zero sim pass, vs our 1600
   particles. Two tricks worth taking even without the rewrite: (a)
   energy-conserving sub-pixel streaks: quad width floored at ~1.15 px and
   opacity multiplied by trueWidth/renderedWidth, so distant rain integrates
   to a grey veil instead of confetti; (b) a single drifting 3-octave fbm
   "curtain" sheet gates rain density in world space: almost free, carries
   most of the storm read. Also rain impact rings on the water (hashed cell
   grid, expanding ring). Our rain block: `src/lib.rs` L3645-3872,
   `data/particles.ron`.

9. **Cloud reflections in water via a cheap env probe** (`Clouds.js` env pass).
   An 18-step equirect cloud march refreshed every 8th frame at low res, with
   below-horizon directions folded upward and water-tinted. Our water mirror is
   the sky-view LUT only (no clouds). This is the cheapest known way to get the
   deck into the sea.

10. **Wind-frame foam erosion (Langmuir windrows)** (`OceanMesh.js`). The foam
    noise lookup is rotated into the wind frame and stretched (0.22, 1.0):
    long downwind streaks, narrow across; the noise MULTIPLIES the sim foam so
    empty windrows stay water. Whitecap onset threshold slides with Monahan
    coverage `mix(0.62, 0.26, W/0.16)`. Beside our lacework tap in
    `90-fragment-main.wgsl` L660-710.

11. **Beaufort preset catalog** (`Weather.js` + `Director.js` acts). A
    validated per-Beaufort table of wind/swell Hs+period/cloud deck/turbidity/
    water scatter+absorb RGB per mood (clear day, trade wind, golden hour,
    overcast, squall, violent storm, night storm), plus the per-key
    exponential-damping target integrator. Our weather transitions are fixed
    30 s lerps; their per-quantity rates (wind 0.28, sun azimuth 0.06 etc.)
    read better. Their open-ocean water spectra with the physics note (open
    ocean peaks hard in blue; raising green toward blue is what coasts do,
    "turquoise enamel" failure) are worth copying into our sea-color grading.

12. **GPU FFT reference for our increment 4** (`OceanFFT.js`). Our roadmap
    already plans moving the 128^2 CPU FFT to GPU compute. Their layout is the
    reference: 8 real fields packed as 4 complex Hermitian signals so ONE IFFT
    inverts displacement + all derivatives; non-harmonic cascade tiles
    (4099 / 389 / 41.3 m) so repeats never beat; band cutoffs handed off while
    the shortest wave still spans ~6 texels; displacement/derivative/turbulence
    output triplet. In wgpu use real compute instead of their fragment
    butterfly ping-pong. Their finite-depth dispersion + TMA shallow correction
    is also the hook for depth-aware waves at our real coasts (we have baked
    per-vertex seafloor depth already).

## Tier 3: worth knowing, not worth porting

- **Projected-grid ocean mesh**: elegant for a flat demo sea, wrong for our
  multi-planet quadtree shell. But two of its lessons transfer: (a) add the
  analytic event height directly in our ocean vertex shader and densify
  tessellation near active events (their bisection exists only because a flat
  reference plane starves a 40 m wall of vertex rows); (b) the Citardauq
  quadratic form for near-parallel ray-sphere intersections (a ~ 1e-8
  catastrophic cancellation tears meshes; we solve on a real sphere in f64,
  same defect class as our f32-at-planet-scale rule).
- **Their cloud system overall**: ours is architecturally richer (constructed
  bodies, 7 families, live weather, octahedral temporal). Cherry-picks that map
  onto the ACTIVE cloud plan: their two-octave curl-warped erosion with bite
  curves `mix(0.78, 0.14, ...)` that spare the core and eat the silhouette,
  wispy-base/billowy-top detail mix, sub-voxel domain warp `(detail*11-0.5)*
  0.011` to kill voxel-lattice bricks, and the coverage threshold gamma
  `pow(cov, 0.67)`. Their powder term keys off LOCAL density (`1-exp(-14*
  dens)`, mixed 0.6) to solidify rims without darkening edges below the sky:
  an alternative to Inc 5's planned outright Beer-powder deletion, if deletion
  alone reads wrong.
- **Their screen-space cloud reprojection**: reprojects each pixel along its
  own stored first-hit DEPTH (fallback shell-mid), TAA-clamps against the 3x3
  of fresh samples, and hides the 4x4 amortization crosshatch with an exact
  4-texel box (4 bilinear taps snapped to a texel corner) faded in ONLY under
  measured camera motion (still frames get sharp Catmull-Rom). Different
  architecture from our octahedral map, but the per-pixel-depth reprojection
  is the principled fix for the starburst class (parallax error against
  metres-away content) if the v0.1236 motion-floor turns out insufficient.
- **Atmosphere deltas**: they carry ozone absorption (0.650, 1.881, 0.085 /MM,
  tent profile at 25 km), a separate Mie scale height (1.2 km vs Rayleigh
  8 km), and a 32-slice aerial-perspective froxel LUT. All three are on our
  own honest-gaps list in `src/renderer/atmosphere.rs`.
- **Post chain (TAA, EV100 auto-exposure, AgX, CoD bloom, thin-lens DOF,
  adaptive quality controller)**: a clean reference implementation if/when we
  do a post-pipeline arc; their frame-time controller (median-based PANIC,
  resolution first, tier second) is a good pattern for our perf floors.
- **What we do better, for confidence**: real sphere + orbit-capable
  altitudes (their view height is pinned near sea level), real multi-planet
  data-driven bodies, real NASA weather, constructed cloud morphology, f64
  discipline, connected-ocean mask + bathymetry, and gameplay coupling
  (vitals, farming, hydrology schemas) they have none of.

## Porting cautions (from the deep-read)

- Everything of theirs runs in f32 world coordinates near the origin. Every
  ported field must be evaluated camera-anchored per our f32-at-planet-scale
  rule (disaster centers as DVec3, downcast the LOCAL offset), and their
  unbounded `time * wind` weather-map advection products need modulo wrapping
  (same fix family as our 64 m wave anchor).
- Their sech clamp of +/-12 must be preserved bit-identically in any CPU/GPU
  twin (it is part of the profile).
- CPU/GPU disaster twinning: keep one authoritative Rust implementation and
  lockstep-test the WGSL against it, exactly like `ocean_waves.rs` L192-252.
- The rogue wave CPU twin deliberately tracks the ENVELOPE only (0.8*amp),
  never the oscillating carrier, so the riding camera does not judder.
- Their uniform-loop-bound workarounds exist only for ANGLE's HLSL unroller;
  WGSL constants are fine.
