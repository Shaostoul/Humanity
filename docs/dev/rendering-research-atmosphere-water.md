# Rendering research: atmospheres and water (2026-07-20)

> Operator request: "Please take your time to research how other games create
> real looking atmospheres and water effects." This doc surveys the proven
> techniques (papers + GDC/SIGGRAPH talks, cited at the bottom), measures each
> against what `assets/shaders/pbr_simple.wgsl` does today, and ends with a
> ranked increment roadmap. Companion docs: `docs/dev/adding-shaders.md` (the
> megashader), `docs/design/ocean.md` (the ocean stages plan).

## Where we are today (baseline)

**Atmosphere (material type 14, `atmosphere_scattering`)**: an O'Neil-class
single-scattering march (12 samples) over an oversized shell sphere, with an
analytic Chapman-function optical depth toward the sun (`atmo_od_to_space`,
`atmo_chapman`, `atmo_erfcx`), Rayleigh + Henyey-Greenstein Mie phase, ACES
tonemap per branch, alpha-blended over the `stars.wgsl` skybox. Known warts,
all operator-reported and all patched with heuristics rather than solved:

- **Stars bleeding through the day sky.** Physically stars vanish because sky
  radiance out-shines them, but an alpha-blended shell can only express
  "coverage." The v0.912 fix (`alpha_occ = max(alpha, sky_lum * 3.2)`) fakes
  radiance-domination through alpha. It works but it is a tuned patch, and it
  couples star visibility to the tonemapped sky brightness.
- **Washed-out haze dome.** `ATMO_EXPOSURE = 4.0` was needed to make the limb
  and sky bright enough, then `ATMO_EXPOSURE_NEAR = 1.4` and
  `ATMO_NEAR_HAZE = 0.45` were needed to un-wash the surface that the 4x boost
  flooded. The root cause is missing energy: single scattering alone is too
  dark (real skies get 30-60 percent of their brightness from multiple
  scattering, more at twilight), so we over-expose to compensate and then
  claw it back with two more knobs.
- **Sun disc dimming wrong.** The sun disc (`sun_surface.wgsl` + the type 17
  corona) is dimmed by whatever alpha the haze shell happens to have over it,
  not by the actual transmittance along the view ray to the sun. Correct
  behavior: the disc's radiance is multiplied by per-channel transmittance,
  which is what makes a setting sun dim AND redden (blue extinguishes first).
- **No aerial perspective on terrain.** Distant mountains and coastline render
  at full contrast; only the atmosphere shell behind them tints. This is the
  single strongest missing realism cue (see the aerial perspective section).

**Ocean (material type 16, `ocean_shell`)**: six cosine height trains
(OCEAN_W1..W6, 2000 m down to 6 m wavelength) displace shell vertices in
`vs_main` near the camera; `water_wave_gradient` perturbs normals with six
slope octaves plus two camera-anchored micro ripple octaves;
`water_shade` does Schlick Fresnel against an analytic two-color sky ramp,
sun-only Lambert body, Blinn sparkle + anchor glint; whitecap foam comes from
slope steepness gated by sea state; alpha is a near-constant 0.93 + Fresnel.
The land interface is a raw geometric intersection between the displaced
shell and the terrain quadtree: no depth awareness, no shore fade, no foam
line, waves the same size in 1 m of water as in 4000 m. That is why "the
water to land interface is still behaving very weird."

The good news: the analytic machinery we already have (Chapman optical depth,
the drawn == sampled golden rule, the quadtree patch LOD, per-planet RON
params) is exactly the foundation the production techniques below want.

---

## 1. Atmosphere techniques

### 1.1 Hillaire 2020, "A Scalable and Production Ready Sky and Atmosphere Rendering Technique" (EGSR 2020, shipped in UE4/5 and Frostbite)

This is the modern default, and the one to aim at. The insight: you do not
need Bruneton's big 4D table. Split the problem into four SMALL LUTs that are
all recomputed every frame (so time of day, planet, and density are fully
dynamic), then the per-pixel work becomes a couple of texture fetches.

The four LUTs, with the paper's shipped sizes and GTX 1080 timings:

| LUT | Size | March steps | Cost | What it stores |
|---|---|---|---|---|
| Transmittance | 256 x 64 | 40 | 0.01 ms | Per-channel transmittance from a (height, sun-zenith-angle) point to space |
| Multiple scattering | 32 x 32 | 20 | 0.07 ms | The infinite-bounce energy factor, parameterized by (height, sun angle) |
| Sky-view | 200 x 100 | 30 | 0.05 ms | The full distant-sky color for the CURRENT camera position, lat-long with a non-linear latitude mapping that packs texels at the horizon: `v = 0.5 + 0.5 * sign(l) * sqrt(|l| / (pi/2))` |
| Aerial perspective | 32 x 32 x 32 froxels | 30 | 0.04 ms | In-scattered light (rgb) + transmittance (a) at 32 depth slices through the camera frustum |

Whole daytime sky at 720p: 0.14 ms, 0.31 ms with post. The same technique
runs on an iPhone 6s at ~1 ms with smaller LUTs, which proves the degrade
story our RTX-class-but-must-degrade target needs.

Key mechanics:

- **Multiple scattering as a geometric series.** Assume scattering orders >= 2
  are isotropic and locally uniform; then total energy = second order times
  `1 / (1 - f_ms)`. One 32 x 32 LUT captures it. This is the missing energy
  that lets you delete brightness knobs: sky brightness becomes right on its
  own, twilight stops dying instantly, the zenith stops needing over-exposure.
- **The sky is rendered as RADIANCE, not coverage.** The sky-view LUT is
  sampled as the background color wherever scene depth is far. Stars are
  either drawn first and the sky added over them, or masked by comparing star
  radiance against sky radiance. Daytime star occlusion becomes automatic
  physics instead of our `sky_lum * 3.2` patch.
- **Sun disc composited separately.** The disc is NOT in the sky-view LUT
  (too low-res, non-linear mapping would smear it). It is drawn after,
  multiplied by a single transmittance LUT fetch toward the sun. That one
  fetch is the entire correct sunset-dimming-and-reddening behavior.
- **Aerial perspective is applied to opaque geometry as a post step** (or in
  the forward shader): sample the froxel volume at the fragment's screen uv +
  depth, `color = color * ap.a + ap.rgb`. Done.

**Implementation size in our engine.** No compute passes required: every LUT
can be a small fragment pass rendering to an offscreen texture, and we
already have that exact machinery in `bloom.wgsl`, `godrays.wgsl`,
`ssao.wgsl` pipelines. The 3D aerial-perspective texture can be a 32-slice
2D atlas if 3D render targets are annoying. Estimated shape:

- 1 new WGSL file with the four LUT-generation entry points (the scattering
  math already exists in `pbr_simple.wgsl` and its Rust mirror
  `src/renderer/atmosphere.rs`; the LUT passes are mostly re-plumbing it),
  ~500-800 lines.
- Rust: 4 small pipelines + textures + per-frame encode order (LUTs before
  the main pass), one bind-group addition so the megashader can sample
  sky-view + aerial-perspective + transmittance. Watch the wgpu default
  limit of 16 sampled textures per stage; group 3 currently uses 8, adding
  4 more leaves headroom but it must be counted.
- The existing type-14 shell stays as the FROM-SPACE path (it is approved
  from orbit); the LUT path takes over when the camera is inside or near the
  atmosphere. Hybrid switching avoids re-fighting the approved space look.

### 1.2 Bruneton 2008, "Precomputed Atmospheric Scattering"

The classic reference implementation (EGSR 2008, updated open-source GLSL).
Precomputes single + multiple scattering into a 4D table (height, view angle,
sun angle, view-sun azimuth), a few seconds of precompute, then sky + aerial
perspective in under 10 fetches per pixel. Quality is the reference bar and
the code is exhaustively documented. Downsides for us: the 4D parameterization
is the notorious part (horizon seam artifacts, tricky texel mapping),
precompute makes density/planet changes non-free (we hot-reload planet RONs),
and Hillaire is explicitly the production-ready successor built to remove the
4D table. Verdict: read it for understanding, implement Hillaire.

### 1.3 Analytic models: Preetham 1999, Hosek-Wilkie 2012

Fitted formulas for CLEAR-SKY ground-level radiance: give sun elevation +
turbidity, get sky color per direction, near zero cost. Hosek-Wilkie is the
better fit (9 coefficients vs 5, handles sunsets and turbidity better, adds
ground albedo; about 30 percent more math than Preetham). Both are
ground-only hemispheres: no space view, no altitude continuum, no aerial
perspective on terrain, no transmittance for a sun disc. For an
orbit-to-surface game they are a dead end, and our existing analytic march
already beats them from space. One borrowable piece: their zenith-horizon
gradients are a cheap upgrade reference for the hardcoded two-color sky ramp
inside `water_shade` (horizon/zenith constants at lines ~835-836), which
should eventually sample the real sky (sky-view LUT) instead.

---

## 2. Aerial perspective (the number-one missing realism cue)

What it is: distant terrain loses contrast and shifts toward the sky color,
because air between camera and mountain (a) extinguishes the mountain's light
(transmittance) and (b) adds its own in-scattered light. Every landscape
photo has it; our terrain currently renders Denali at 200 km with the same
contrast as a rock at 2 m.

How games do it, cheapest to best:

1. **Distance fog toward a fixed color** (classic `mix(color, fog_color, f)`
   with exponential f). Fails in a specific observable way (runevision's
   mountain notes): real mountains fade through DEEP BLUE at moderate
   distance to PALE at extreme distance; one fixed fog color cannot do both,
   and matching the pale horizon makes near ridges too pale while matching
   blue makes the far ones darker than the sky, which never happens.
2. **Height fog with two colors** (UE's exponential height fog: one color
   toward the sun hemisphere, one away). Better, still a fit.
3. **Physical single-scatter along the view path** (what Hillaire's froxel
   volume stores, and what Bruneton computes from the table): per fragment,
   transmittance T and in-scatter S over the camera-to-fragment segment, then
   `color * T + S`. This produces the blue-then-pale progression, the
   sun-side warm haze, and altitude dependence automatically, because it is
   the same math as the sky itself.

**Our shortcut: we can have option 3 without any LUT.** The megashader's
terrain branch (type 12) already knows the planet center, radius, and the
atmosphere params live one material away; `atmo_od_to_space` +
`atmo_chapman` already give analytic optical depths. A 4-6 sample march
(or even 2 samples plus the analytic depth for the sun legs) over the
camera-fragment segment inside the terrain branch is a contained change to
`pbr_simple.wgsl` plus passing the atmosphere params into the terrain
material (params/UV packing or one small uniform). This is increment 2 in
the roadmap and probably the single biggest visual jump available for the
effort. The Hillaire froxel volume later replaces the per-fragment march
with one texture fetch (an optimization, not a look change).

Tuning caution from our own history: v0.826's ATMO_NEAR_HAZE exists because
in-scatter over long grazing paths piles up fast. Aerial perspective on
terrain wants the SAME treatment from day one: compute it physically, then
give the operator one strength slider, because the physically-full effect
reads hazier than games have trained eyes to expect.

---

## 3. Water techniques

### 3.1 Tessendorf FFT ocean (the industry baseline since 2001)

Sum tens of thousands of waves whose amplitudes are drawn from an
oceanographic spectrum, evaluated with an inverse FFT into tiling
displacement + normal maps. Used by Sea of Thieves, Atlas, AC Black Flag,
God of War, basically every serious game ocean.

- **Spectra**: Phillips (the paper's original, simple), JONSWAP (adds fetch,
  waves never "fully developed"), TMA (JONSWAP times a depth attenuation
  term, correct in shallow water). Modern implementations default JONSWAP or
  TMA with wind speed, fetch, and depth as the sea-state knobs, which is a
  physical upgrade of our `sea_state` scalar.
- **Cascades**: one FFT tile cannot cover 2000 m swells and 20 cm chop, so
  ships use 3-4 cascades (e.g. 256^2 textures at tile sizes related by
  irrational ratios to hide tiling), summed in the vertex/fragment shader.
  GodotOceanWaves (an excellent readable open implementation) uses TMA +
  cascades + per-cascade update-rate load balancing.
- **Cost**: a 256^2 FFT is cheap on anything modern; a full 4-cascade chain
  (spectrum update + 2D IFFT + Jacobian/foam pass) ran at 60 fps on a GTX
  1050 Ti, and Triton shipped 256^2 (65,536 waves) at hundreds of fps on
  2013 hardware. On our RTX target this is background noise; the real cost
  is engineering: it wants compute passes (ping-pong Stockham FFT), which
  wgpu fully supports but our engine has not used yet. It would be our
  first compute infrastructure.
- **Foam**: the Jacobian of the horizontal displacement goes negative where
  crests fold over themselves; that is the geometric foam mask, accumulated
  in a buffer with grow/decay rates so foam lingers and dissipates.
- **Choppiness**: FFT oceans displace HORIZONTALLY too (the minus-gradient
  trick), sharpening crests. Our cosine trains displace only radially, which
  is one reason our sea reads soft; plain Gerstner (GPU Gems ch. 1, Finch)
  adds the same horizontal term analytically and we could adopt it without
  FFT.

**The drawn == sampled golden rule vs FFT.** `docs/design/ocean.md` requires
physics to sample exactly what the shader draws. Three workable resolutions:
(a) keep the analytic trains as the physics field and let FFT cascades only
carry wavelengths below the physics-relevance threshold (say < 10 m, low
amplitude), so buoyancy error stays centimeters; (b) run a low-res CPU
inverse FFT (32^2) of the same spectrum for physics; (c) async GPU readback
of the displacement maps with a frame of latency. (a) is the least machinery
and keeps the guard test meaningful.

### 3.2 Sea of Thieves (SIGGRAPH 2018 talk, exact recipe)

The best-documented stylized-real ocean, and almost everything transfers:

- FFT base per Tessendorf.
- **Water color = blend(deep color, subsurface color)** driven by view
  angle, sun direction, and a WAVE PEAK MASK derived from the FFT
  choppiness offsets: peaks are thinner, light travels a shorter path, so
  peaks go green-turquoise while troughs stay deep blue. This single trick
  is a huge part of their look. Our analog without FFT: use the analytic
  wave height (we already compute it) as the peak mask driving a
  deep-to-subsurface blend in `ocean_shell`, replacing the current
  noise-only `sea_var` hue variation.
- **Foam**: at wave peaks (Jacobian method) PLUS around intersecting objects
  via depth-buffer comparisons in a camera-centered window, with the foam
  buffer progressively blurred with feedback so it disperses softly, then
  blended with artist textures. Storm/calm states modulate generation.
- **Area specular** via Karis closest-point-on-sphere so a low sun makes a
  long stretched glitter road (our `anchor` lobe approximates this; the
  area-light form is the upgrade).
- **Snell's window** when the camera is underwater.

### 3.3 Atlas (GDC 2019) and AC Black Flag (GDC 2013)

- **Atlas** ("Wakes, Explosions and Lighting: Interactive Water Simulation
  in 'Atlas'", WildCard + NVIDIA): FFT ocean plus LOCAL interactive
  disturbances (ship wakes, explosions) as wave particles superposed on the
  spectrum field, synchronized across multiplayer servers. This is exactly
  the shape of our Stage 4 "displacement events" plan in
  `docs/design/ocean.md` (analytic radial wave packets on top of the base
  field), and their ocean BSDF is a common reference for sun/sky/scatter
  combination.
- **Black Flag**: systemic Beaufort-scale sea states (design-controllable 0-12
  storm ladder; our sea_state 0..1 is the same idea and could adopt the
  Beaufort table as its data-driven backbone), a third small-wave layer for
  close detail, and the depth-mask trick: an invisible "lid" mesh over each
  ship interior rendered into the depth buffer only, so the ocean plane
  never draws inside the hull. We will need exactly this the day ships sail
  (Stage 2), and it is cheap: one depth-only draw per boat.

### 3.4 Shoreline: depth-based shallow blending + foam line (the fix for "the water to land interface is still behaving very weird")

The standard technique, used by essentially every game with coastlines:

1. **Make scene depth readable in the water pass.** After the opaque pass,
   copy the depth buffer to a sampleable texture (wgpu cannot sample the
   depth attachment being written). Bind it in group 3 (next free binding,
   11). This is the one piece of new plumbing, and it is also the enabler
   for Sea-of-Thieves object foam and Snell effects later.
2. **Water column thickness per pixel**: linearize scene depth vs water
   fragment depth; the difference is how much water the eye ray crosses
   before hitting seabed.
3. **Use the thickness three ways**:
   - **Absorption tint**: `exp(-k * thickness)` per channel blends
     seabed color through turquoise shallow to deep blue. Physical, one
     constant vector k. (Cheaper linear lerp also ships fine, per the
     Cyanilux breakdown.)
   - **Alpha ramp to zero at zero thickness**: the waterline stops being a
     hard polygon intersection and becomes a soft wet edge. This alone kills
     most of the "weird interface." Replaces our constant 0.93 alpha near
     coasts (and subsumes the v0.887/v0.902 glow-vs-paint tuning war).
   - **Foam band**: `smoothstep` on small thickness gives a shore-hugging
     band; distort it with noise and scroll it (repeating fract/cosine
     pattern moving shoreward with a synchronized swash return) and you get
     animated breaking wavefronts at the beach for a few shader lines.
4. **Depth-attenuate the waves themselves**: scale wave amplitude by a
   function of local water depth (TMA's Kitaigorodskii factor is the
   physical version; a smoothstep on depth works). Swells must die before
   the sand; right now our 2000 m train displaces the shell straight through
   beach terrain, which is a big component of the weirdness. The CPU twin
   (`terrain/ocean_waves.rs`) must apply the same factor (golden rule), so
   the shader and Rust change ship together with the guard test updated.

**Screen-space vs geometric foam**, summarized: geometric foam comes from
the wave field itself (Jacobian folds, crest steepness like our v0.909
whitecaps) and lives in world space, LOD-stable; screen-space foam comes
from depth comparisons (shorelines, hulls, rocks) and catches every
intersection for free but needs the depth copy and a feedback-blur buffer if
you want it to disperse softly. Ships use both: geometric for open-sea
whitecaps, depth-based for every water-meets-thing line.

### 3.5 Ocean LOD strategies

- **Projected grid** (Johanson 2004): a screen-space uniform grid re-projected
  onto the sea plane each frame; automatic detail falloff, no LOD levels, but
  it swims at grazing angles and fights spherical planets. Not a fit for us.
- **Geometry clipmaps / radial grids**: nested rings following the camera
  (GodotOceanWaves, Crest). Uniform screen density, very few draw calls,
  needs its own mesh system.
- **Quadtree patches (ours)**: what our terrain and ocean shell already use,
  and what large-world ships use for streaming reasons. Fine to keep, the
  research consensus is that the mesh scheme matters far less than what
  displaces it. The one thing worth adding is denser near-camera
  tessellation when we push real displacement detail (the v0.912 "geometric
  near chop" already leans this way).

---

## 4. Recommended roadmap (smallest first)

Each increment stands alone, ships alone, and none blocks the ones after it.
Sizes are relative (S under a day, M a day or two, L a multi-day arc).

1. **Sun disc transmittance (S).** Multiply the sun disc + corona radiance by
   per-channel transmittance toward the sun, computed analytically from the
   existing `atmo_od_to_space` machinery (Rust mirror in
   `renderer/atmosphere.rs` for the CPU-side sun tint so lighting agrees).
   Payoff: the sun dims AND reddens correctly through the atmosphere, sunset
   sun stops being a washed white disc; also tints sunlight on terrain/water
   at low sun for free. This is Hillaire's own composite-the-disc-separately
   step, minus any LUT.
2. **Aerial perspective on terrain (M).** In the megashader terrain branch
   (type 12), march 4-6 samples camera-to-fragment with the existing Chapman
   analytic sun legs; output `surface * T + S`. One operator-facing strength
   slider. Payoff: distant ridges and coasts fade toward sky color, the
   single strongest landscape realism cue we lack, and it uses math already
   proven in `atmosphere_scattering`.
3. **Shoreline depth fade + foam line (M).** Depth copy after the opaque
   pass, bind at group 3 binding 11, then in `ocean_shell`: thickness-based
   absorption tint, alpha ramp to zero at the waterline, animated foam band,
   and depth-attenuated wave amplitude (shader + `terrain/ocean_waves.rs`
   twin + guard test together). Payoff: directly fixes "the water to land
   interface is still behaving very weird"; beaches get wet edges, shallow
   turquoise, and breaking-wave foam lines.
4. **Multiple scattering energy + exposure cleanup (M). DONE v0.918.0.**
   Shipped as a three-tier exposure instead of a global walk-down: ground
   SKY rays get `ATMO_EXPOSURE_DOME` 1.7 (ramping back to the full 4.0 by
   the shell top, so the 400 km limb + 12,000 km marble are bit-identical),
   grazing surface rays blend toward the sky tier (killing the white veil
   on grazing-angle water), and an analytic isotropic multiple-scatter term
   (`ATMO_MS_ISO` 0.07, gated to exactly the region the dome dimmed) rides
   the same per-channel path integral. The NEAR/HAZE compensators were KEPT
   deliberately - they compensate close-range surface haze, not the dome,
   and each retirement risks an approved look; revisit under item 5 when
   the sky becomes a sampled radiance source. Twilight star-occlusion gain
   3.2 -> 4.5 offsets the dimmer dome. Bonus fix (BUG-047): shell meshes no
   longer ride `planet_max_subdiv` down into the planet.
5. **Sky-view LUT + sky-as-radiance (M/L).** 200 x 100 fragment-pass LUT with
   the horizon-packed mapping, recomputed per frame for the near planet;
   the in-atmosphere sky samples it (space views keep the approved type-14
   shell). Draw sky as radiance over the starfield so daytime star occlusion
   is physics, replacing the `sky_lum * 3.2` alpha patch. Payoff: smoother
   cheaper sky, correct stars, and one consistent sky source that
   `water_shade`'s hardcoded two-color reflection ramp can start sampling.
6. **Wave peak subsurface color + Gerstner choppiness (M).** Sea of Thieves'
   deep-vs-subsurface blend driven by our analytic wave height as the peak
   mask, plus horizontal Gerstner displacement so crests sharpen (same train
   constants, one extra term, CPU twin updated). Payoff: the sea stops being
   one blue; peaks glow turquoise against deep troughs, crests look pushed
   instead of inflated.
7. **Aerial perspective froxel volume (M, optional optimization).** Replace
   increment 2's per-fragment march with Hillaire's 32^3 volume sampled by
   uv + depth, once more things (clouds, water, particles) want the same
   fade. Payoff: perf headroom and consistency, not a look change.
8. **FFT ocean cascades (L).** Our first compute passes: 2-3 cascades at
   256^2, TMA spectrum with wind/fetch as the sea-state backbone, Jacobian
   foam accumulation, cascades restricted to sub-physics wavelengths so the
   analytic trains remain the drawn == sampled physics truth. Payoff: the
   end-game open-sea look (Sea of Thieves / Atlas class detail), real
   spectrum-driven storms, foam that appears where waves actually fold.

Suggested order is as numbered; 1-3 are independent of each other and could
land in any order or in parallel worktrees (disjoint code: sun disc vs
terrain branch vs ocean branch).

---

## Sources

- Hillaire, "A Scalable and Production Ready Sky and Atmosphere Rendering
  Technique", EGSR 2020. Paper: https://sebh.github.io/publications/egsr2020.pdf
  (also https://onlinelibrary.wiley.com/doi/abs/10.1111/cgf.14050)
- WebGPU port of Hillaire's model (LUT setup reference directly in WGSL):
  https://github.com/JolifantoBambla/webgpu-sky-atmosphere
- Bruneton, "Precomputed Atmospheric Scattering", EGSR 2008 + maintained
  implementation: https://ebruneton.github.io/precomputed_atmospheric_scattering/
- Hosek and Wilkie, "An Analytic Model for Full Spectral Sky-Dome Radiance",
  SIGGRAPH 2012: https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint_lowres.pdf
- Willmott, sun/sky model survey code (Preetham vs Hosek notes):
  https://github.com/andrewwillmott/sun-sky
- Runevision, "Notes on atmospheric perspective and distant mountains":
  https://blog.runevision.com/2025/06/notes-on-atmospheric-perspective-and.html
- Maxime Heckel, "On Rendering the Sky, Sunsets, and Planets" (readable
  Hillaire walkthrough with gotchas):
  https://blog.maximeheckel.com/posts/on-rendering-the-sky-sunsets-and-planets/
- Tessendorf, "Simulating Ocean Water", SIGGRAPH course 2001 (the FFT ocean).
- Ang, Catling, Ciardi, Kozin, "The Technical Art of Sea of Thieves",
  SIGGRAPH 2018 Talks: https://dl.acm.org/doi/10.1145/3214745.3214820
  (PDF: https://history.siggraph.org/wp-content/uploads/2022/09/2018-Talks-Ang_The-Technical-Art-of-Sea-of-Thieves.pdf)
- Mihelich and Tcheblokov, "Wakes, Explosions and Lighting: Interactive Water
  Simulation in 'Atlas'", GDC 2019: https://gdcvault.com/play/1025819/
  (video: https://www.youtube.com/watch?v=Dqld965-Vv0)
- fxguide on AC4 Black Flag ocean tech (Beaufort systemic seas):
  https://www.fxguide.com/fxfeatured/5-things-you-need-to-know-about-the-tech-of-assassins-creed-iv-black-flag/
- Simon Trumpler, "Black Flag - Waterplane" (the ship depth-mask lid):
  https://simonschreibt.de/gat/black-flag-waterplane/
- Cyanilux, "Shoreline Shader Breakdown" (depth fade + animated foam line):
  https://www.cyanilux.com/tutorials/shoreline-shader-breakdown/
- 2Retr0, GodotOceanWaves (TMA + cascades + Jacobian foam, readable source):
  https://github.com/2Retr0/GodotOceanWaves
- Ryan, "Ocean Rendering, Part 1 - Simulation" (spectra + cascade practice):
  https://rtryan98.github.io/2025/10/04/ocean-rendering-part-1.html
- Johanson, "Real-time water rendering - the projected grid concept":
  http://habib.wikidot.com/projected-grid-ocean-shader-full-html-version
- Finch, "Effective Water Simulation from Physical Models", GPU Gems ch. 1
  (Gerstner waves in games): https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models
