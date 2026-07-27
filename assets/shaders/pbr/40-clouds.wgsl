// ── Procedural cloud layer (material type 15, clouds increments 2 + 3) ──
//
// Increment 3 (the photo-real upgrade) adds `cloud_layer_volumetric`: two
// precomputed tiling 3D noise volumes (group 3 bindings 2..4, generated at
// startup by renderer::cloud_noise) carve real cauliflower cloud bodies out
// of the increment-1 weather field, lit by a per-sample Beer-Lambert light
// march with Beer-powder edges and a dual-lobe HG phase. The quality ladder
// (material.params.y: 0 Low / 1 Medium / 2 High) is dispatched in
// `cloud_layer` below; the older paths are kept verbatim as the lower tiers.
//
// An animated cloud DECK on a SECOND translucent shell just above the planet
// surface and BELOW the scattering atmosphere shell. lib.rs pushes the cloud
// shell into the transparent celestial list BEFORE the atmosphere shell, and
// that list draws in order with no depth writes, so the air blends OVER the
// clouds -- physically correct: the atmosphere scatters in front of the deck.
//
// Increment 2 (v0.815): the deck is now RAYMARCHED through a thin spherical
// slab (CLOUD_BASE_SCALE..CLOUD_TOP_SCALE planet radii; the drawn shell at
// CLOUD_SHELL_SCALE sits mid-slab and only supplies the fragments/rays).
// Exactly the reuse contract designed into increment 1:
//   density(p_local) = cloud_alpha_from_field(
//       cloud_field(normalize(p_local), t, seed), coverage)
//       * cloud_altitude_envelope(length(p_local))
// cloud_field/cloud_alpha_from_field are UNCHANGED; the altitude envelope and
// the march loop are the only new math. Front-to-back alpha accumulation with
// an early-out at opacity saturation, per-sample macro N-dot-L lighting, a
// one-tap sun-direction density gradient for volumetric self-shadow, and a
// base-to-top height gradient (bases darker, tops brighter). The silver
// lining and the ACES tail keep increment 1's posture. The increment-1
// single-sample path is kept verbatim as `cloud_layer_flat`, selected by
// setting CLOUD_MARCH_SAMPLES to 0 (the quality switch; this file is
// hot-reloaded from disk, so the fallback is one edit away on weak GPUs).
//
// Material packing (producer: lib.rs planet_cloud_materials; Rust mirror +
// unit tests: src/renderer/clouds.rs -- keep every CLOUD_* constant in sync,
// the wgsl_cloud_constants_stay_in_sync test enforces it by parsing this
// file):
//   base_color.rgb  cloud tint (white today; a future per-planet cloud_color
//                   field can ride here with zero shader changes)
//   base_color.a    coverage 0..1 (planet RON cloud_coverage)
//   params.x        per-planet noise seed (terrain_seed % 1024) so two
//                   cloudy worlds never show the same pattern
//   params.y        cloud quality (clouds increment 3): 0 = Low (the
//                   increment-1 painted deck), 1 = Medium (the 10-sample
//                   field march), 2 = High (the volumetric 3D-noise system
//                   below). Settings > Graphics > Cloud quality.
//   params.z        15.0 (this shader type)
//
// TIME rides in camera.sun_color.w -- that slot was a documented-unused pad,
// so animating the clouds needed no uniform layout change (the same
// no-layout-churn rule as the type-14 material packing; the v0.782
// device-limit incident is why layout churn is avoided). Written by
// render_celestial_onto each frame; app-start-relative seconds, so f32
// precision stays comfortable for days of uptime at these drift rates.

// Peak opacity of the thickest cloud core. Deliberately below 1.0 so the
// planet surface stays readable through even the densest deck. Lowered
// 0.85 -> 0.72 after the first orbital field test (2026-07-11): at 0.85 the
// decks fused into a featureless white cue ball.
const CLOUD_MAX_ALPHA: f32 = 0.72;
// Empirical contrast window of the raw octave sum over the sphere (p03/p96
// of 20,000 spiral samples, measured in renderer::clouds's tuning probe):
// the triplanar blend + octave averaging concentrate the sum tightly around
// ~0.49, so WITHOUT this stretch a mid coverage value catches only the
// distribution's thin upper tail and Earth reads nearly cloudless (caught
// by the coverage_maps_monotonically test on first tuning). smoothstep
// through this window spreads the sum to a roughly UNIFORM 0..1
// "cloudiness", which is what lets the coverage knob track actual sky
// fraction via the simple threshold below.
const CLOUD_FIELD_LO: f32 = 0.32;
const CLOUD_FIELD_HI: f32 = 0.65;
// Softness of the cloud edge: alpha ramps over this field range above the
// threshold, giving wispy borders instead of cookie-cutter blobs. Widened
// 0.18 -> 0.30 with the detail octaves (2026-07-11): the wider ramp lets
// the high-frequency octaves erode the borders into filigree instead of
// stamping hard blob outlines.
const CLOUD_EDGE: f32 = 0.30;
// Zonal anisotropy of the cloud field: the sampling direction's y (the spin
// axis) is scaled UP by this factor before the noise lookup, so the noise
// varies faster with latitude than longitude and features stretch east-west
// like real storm bands and jet-stream streaks. 1.0 = isotropic blobs.
const CLOUD_BAND_STRETCH: f32 = 1.75;
// The "weather" of increment 1: two octave SETS drift as rigid rotations at
// different speeds around different axes, so their SUM genuinely evolves
// (morphs) rather than sliding as one frozen texture. Radians per second of
// cloud-clock time; zonal ~0.0015 rad/s is a visible-but-calm crawl (a
// pattern crosses a planet disc in tens of minutes). Increment 2 can promote
// these to per-planet data.
const CLOUD_DRIFT_ZONAL: f32 = 0.0015;
const CLOUD_DRIFT_CROSS: f32 = -0.0009;
// Self-shadow lookup: great-circle step (radians) toward the sun, and how
// hard a density rise over that step darkens this fragment. SHARP amplifies
// the (already contrast-stretched) field differences into a usable shading
// range without saturating everywhere.
const CLOUD_SHADOW_STEP: f32 = 0.05;
const CLOUD_SHADOW_STRENGTH: f32 = 0.65;
const CLOUD_SHADOW_SHARP: f32 = 2.5;
// Silver lining: forward-scatter glow at THIN cloud edges when looking
// toward the sun (Henyey-Greenstein lobe, reusing the atmosphere's phase
// function -- no third scattering model).
const CLOUD_SILVER_GAIN: f32 = 0.3;
// Ambient skylight floor on the day side (shadowed cloud flanks stay
// visibly white, not gray mush) and the night-side floor (near-black but
// not absolute zero, matching the surface shader's ambient posture).
const CLOUD_AMBIENT: f32 = 0.08;
const CLOUD_NIGHT_FLOOR: f32 = 0.006;
// ── Increment-2 raymarch constants (Rust mirror: renderer::clouds) ──
// The slab, in PLANET-RADIUS multiples: the drawn shell (fragments/rays only)
// sits mid-slab at CLOUD_SHELL_SCALE; density lives between BASE and TOP.
// For Earth: base ~25.5 km, drawn shell ~51 km, top ~76.5 km. Terrain peaks
// (up to ~1.0041 R) may poke ~100 m into the base -- mountains in cloud,
// physically charming and harmless (the envelope is ~0 there).
const CLOUD_SHELL_SCALE: f32 = 1.008;
const CLOUD_BASE_SCALE: f32 = 1.004;
const CLOUD_TOP_SCALE: f32 = 1.012;
// Medium-tier march samples along the view segment through the slab.
// 8-12 is the designed band. (Since increment 3 the LOW/MEDIUM/HIGH switch
// is material.params.y -- see cloud_layer -- so this no longer doubles as
// the quality toggle.) Measured on the RTX 4070 at 2560x1373 (2026-07-11,
// same-regime march-vs-flat pairs): +1.5 ms at the 12,000 km orbit view,
// +0.2 ms at 400 km with the deck filling the frame -- the clear-sky probe
// gate and the saturation early-out keep the worst case cheap, and the
// ~90 FPS orbit baseline holds with the march on.
const CLOUD_MARCH_SAMPLES: i32 = 10;
// Extinction per drawn-shell unit at density 1. Calibrated so a full-density
// radial pass through the slab (envelope integral ~0.6 * thickness) reaches
// ~93% opacity -- matching increment 1's thick-core look after the
// CLOUD_MAX_ALPHA cap: 1 - exp(-560 * 0.6 * 0.00794) ~ 0.93.
const CLOUD_SIGMA_T: f32 = 560.0;
// Self-shadow tap for the march: a 3D offset TOWARD the sun (drawn-shell
// units, ~half the slab thickness) replaces increment 1's great-circle step;
// density rising toward the sun = this sample sits in a cloud mass's shadow.
// SHARP converts the (envelope-scaled, so smaller) density difference into a
// usable shading range.
const CLOUD_MARCH_SHADOW_STEP: f32 = 0.004;
const CLOUD_MARCH_SHADOW_SHARP: f32 = 4.0;
// Height gradient: cloud BASES receive less sky/sun light than tops. The
// classic volumetric cue -- flat bottoms read dark, sunlit tops read bright.
const CLOUD_BASE_DARKEN: f32 = 0.75;
// ── Increment-3 volumetric constants (Rust mirror: renderer::clouds) ──
// The High-quality path: precomputed tiling 3D noise (group 3, bindings
// 2..4) + weather map + per-sample light march. Standard real-time cloud
// recipe (Nubis / Horizon style) adapted to the spherical slab.
//
// Slab bounds in DRAWN-SHELL units, derived from the scales above.
const CLOUD_RB: f32 = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
const CLOUD_RT: f32 = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
// View-march samples through the slab. Exponentially spaced (dense near
// the entry point -- see CLOUD_HI_STEP_EXP) so the puffy foreground gets
// the detail budget and the far limb blurs gracefully.
const CLOUD_HI_SAMPLES: i32 = 48;
// Exponent of the sample-position curve: t = m0 + seg * u^EXP. 1 = uniform.
const CLOUD_HI_STEP_EXP: f32 = 1.6;
// Light-march taps toward the sun per lit view sample. Spacing widens with
// each tap (near taps catch self-shadowing detail, far taps the big mass).
const CLOUD_HI_LIGHT_SAMPLES: i32 = 8;
// Base light-march step, drawn-shell units (slab thickness is ~0.0079).
// Halved 0.0012 -> 0.0006 (2026-07-27 tau-heat-map probe): the first tap
// used to jump ~15% of the slab, overshooting thin stratus bands entirely
// (every tap above the band -> tau 0 -> flat white lighting).
const CLOUD_LIGHT_STEP: f32 = 0.0006;
// Light-march extinction multiplier over the view sigma (2026-07-27, the
// flat-lighting root cause): CLOUD_HI_SIGMA_T is calibrated for VIEW
// opacity (kept low so deck edges feather instead of reading as carved
// stencils), but reusing it for the SUN path made every shadow shallower
// than e^-0.5 -- the tau heat-map probe showed tau ~0.1 across a solid
// overcast deck at noon, i.e. structurally flat lighting. Real cloud media
// are optically far thicker than the view calibration pretends; a separate
// stronger shadow extinction is the standard production split (view sigma
// for alpha, boosted sigma for the light march), with the multi-scatter
// octaves in cloud_scatter_energy keeping deep shadows luminous, not black.
const CLOUD_LIGHT_SIGMA_MULT: f32 = 6.0;
// Extinction per drawn-shell unit at density 1 for the High path. Tuned so
// dense cores saturate but thin edges stay translucent -- too high (the
// first 1400 value) turned every density onset into a hard opaque cliff, so
// clouds read as carved stencils from orbit; this softer value feathers the
// edges while the CLOUD_HI_MAX_ALPHA cap still lets cores block the ground.
const CLOUD_HI_SIGMA_T: f32 = 850.0;
// Peak alpha of the High deck. Above Medium's 0.72: photoreal cumulus
// cores genuinely block the ground; thin skirts stay translucent anyway.
const CLOUD_HI_MAX_ALPHA: f32 = 0.96;
// SHAPE texture tiles per drawn-shell unit. Earth: one tile = 6422/24 =
// ~268 km, so the base Worley cells (6 per tile) are ~45 km features and
// the finest shape octave (24 per tile) ~11 km -- the 30..80 km "cloud
// mass" band the design calls for.
const CLOUD_SHAPE_FREQ: f32 = 24.0;
// DETAIL texture tiles per drawn-shell unit. Lowered 90 -> 60 so the erosion
// features are ~3..13 km (larger, less prone to sub-pixel aliasing) -- the
// distance fade below removes what remains from orbit.
const CLOUD_DETAIL_FREQ: f32 = 60.0;
// How deeply the detail octaves erode the shape's edges (0 = off).
const CLOUD_DETAIL_ERODE: f32 = 0.38;
// Detail erosion distance fade (drawn-shell units of camera-to-sample
// distance): full cauliflower within NEAR, gone by FAR. Keeps the orbital
// marble smooth (the ~km detail is sub-pixel there and would alias into
// salt-and-pepper stipple) while the low fly-by keeps its billowy edges.
// NEAR ~0.03 R = ~190 km; FAR ~0.35 R = ~2200 km.
const CLOUD_DETAIL_FADE_NEAR: f32 = 0.03;
const CLOUD_DETAIL_FADE_FAR: f32 = 0.70;
// Coverage carve thresholds (shader-only tuning; not mirrored -- the density
// function they live in samples textures and cannot be mirrored). The shape
// noise must clear a weather-driven threshold to become cloud: where the
// weather field is thin the threshold is CLOUD_COV_LO (almost nothing
// survives -> clear blue sky), where it peaks the threshold drops to
// CLOUD_COV_HI (dense cores). Tuned high/sparse on purpose so the deck reads
// as SCATTERED cumulus with real gaps, not a solid overcast blanket -- the
// first orbital field test (2026-07-11) rendered a near-total white sheet
// because the old `1 - weather_a` carve kept the shape almost everywhere.
const CLOUD_COV_LO: f32 = 0.92;
const CLOUD_COV_HI: f32 = 0.52;
// Width of the soft density onset above the coverage threshold (in shape-noise
// units). Wider = more feathered mass edges; too wide washes coverage out.
const CLOUD_COV_SOFT: f32 = 0.20;
// Cloud-TYPE field frequency (tiles around the sphere): a very-low-freq
// noise picks stratus (0) vs cumulus (1) regions, ~2000 km weather cells.
const CLOUD_TYPE_FREQ: f32 = 3.0;
// Dual-lobe Henyey-Greenstein phase: strong forward lobe (silver linings,
// bright toward-sun rims) + mild back lobe (retro-reflection when the sun
// is behind the camera), blended by the forward weight.
const CLOUD_HG_FWD: f32 = 0.55;
const CLOUD_HG_BACK: f32 = -0.15;
const CLOUD_HG_FWD_WEIGHT: f32 = 0.7;
// Beer-powder strength: thin media darken (little in-scattering) -- the
// classic dark-translucent-edge cue. Raised 0.75 -> 0.92 to kill a bright
// RIM the orbital marble showed: thin cloud skirts over dark ocean were
// out-scattering brighter than the cores, outlining every gap in white. A
// strong powder term darkens those thin skirts so masses read solid, with
// the bright silver lining preserved only where the sun is behind them (the
// powder_gate eases powder off toward the sun). 0 = off.
const CLOUD_POWDER_STRENGTH: f32 = 0.92;
// Ambient skylight across the slab: bases sit in their own shadow and see
// mostly ground; tops see the whole sky dome. Fraction of sun energy. Kept
// low so shadowed flanks and bases read as visibly darker grey (the tonal
// range that makes puffs look 3D) instead of a flat bright white sheet.
const CLOUD_AMB_BASE: f32 = 0.03;
const CLOUD_AMB_TOP: f32 = 0.14;
// ── Wispiness + cloud-type regime constants (v0.828, Rust mirror: clouds) ──
// The "giant blotches" of the first volumetric pass came from the detail
// erosion FADING OUT with distance (CLOUD_DETAIL_FADE_*): from orbit only the
// smooth round Perlin-Worley body survived, so masses read as blobs. The fix
// is a SECOND, COARSER erosion band that never fades -- big enough (tens of
// km) to stay well above a pixel from orbit, so the marble keeps frayed,
// wispy edges. CLOUD_FRAY_FREQ tiles per drawn-shell unit: Earth ~708 km per
// tile -> the detail volume's 8-cell Worley reads as ~88 km fray features
// (supra-pixel from 12,000 km, so no salt-and-pepper stipple).
const CLOUD_FRAY_FREQ: f32 = 9.0;
// Global strength of the coarse fray (the per-regime FRAY weight scales it).
const CLOUD_FRAY_ERODE: f32 = 0.5;
// Density-response shaping exponent applied to the carved cloud body before
// extinction. > 1 pushes LOW densities down hard while leaving cores intact:
// thin skirts turn translucent and see-through (the operator's "way more
// wispy"), dense cores still saturate. The classic erode-edges-keep-cores
// curve, applied in density space so it composes with Beer-Lambert.
const CLOUD_DENSITY_POW: f32 = 1.7;
// Secondary cloud-type octave: blended with CLOUD_TYPE_FREQ so the regime
// map has organic sub-structure (more than a few giant bands) and every
// cloud type shows somewhere across the disc.
const CLOUD_TYPE_FREQ2: f32 = 7.0;
// Filament mask window: the ridged-Perlin filament channel (DETAIL alpha) is
// smoothstepped through this range to a streak mask. Cirrus multiplies its
// body by this, fraying flat sheets into thin, branching streaks.
const CLOUD_FIL_LO: f32 = 0.30;
const CLOUD_FIL_HI: f32 = 0.74;

// Rigid rotation around the local Y axis (the planet's spin axis in the
// icosphere's local frame): zonal drift, like real weather bands.
fn cloud_rot_y(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * v.x + s * v.z, v.y, c * v.z - s * v.x);
}

// Rigid rotation around the local X axis: the cross-drift for octave set B,
// deliberately a DIFFERENT axis so the two sets shear against each other.
fn cloud_rot_x(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(v.x, c * v.y - s * v.z, c * v.z + s * v.y);
}

// Seamless noise on the sphere: TRIPLANAR blend of the existing 2D value
// noise (reusing hash21/value_noise above -- not a third noise
// implementation). For a UNIT direction the squared components already sum
// to 1, so dir*dir are the blend weights for free. Each plane gets a
// different seed offset so the three projections never mirror each other at
// the +/- axis crossings.
fn hash13(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.zyx + 31.32);
    return fract((q.x + q.y) * q.z);
}

/// TRUE 3D value noise with a quintic fade. Replaces the old triplanar
/// 2D-projection blend (v0.873): three projections mixed on a sphere always
/// crease along the diagonal great circles no matter how sharp the blend
/// weights - the operator's "weird straight lines" through the cloud deck.
/// A genuine 3D lattice has no projections, hence no seams, and 8 corner
/// hashes cost less than the triplanar's 12. The quintic fade (vs the 2D
/// helper's smoothstep) also removes the lattice's derivative creases that
/// the coverage contrast-stretch used to amplify into visible cell edges.
fn cloud_noise(dir: vec3<f32>, freq: f32, seed: f32) -> f32 {
    let p = dir * freq + vec3<f32>(seed, seed * 0.617, seed * 0.317);
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0); // quintic fade

    let c000 = hash13(i);
    let c100 = hash13(i + vec3<f32>(1.0, 0.0, 0.0));
    let c010 = hash13(i + vec3<f32>(0.0, 1.0, 0.0));
    let c110 = hash13(i + vec3<f32>(1.0, 1.0, 0.0));
    let c001 = hash13(i + vec3<f32>(0.0, 0.0, 1.0));
    let c101 = hash13(i + vec3<f32>(1.0, 0.0, 1.0));
    let c011 = hash13(i + vec3<f32>(0.0, 1.0, 1.0));
    let c111 = hash13(i + vec3<f32>(1.0, 1.0, 1.0));

    let x00 = mix(c000, c100, u.x);
    let x10 = mix(c010, c110, u.x);
    let x01 = mix(c001, c101, u.x);
    let x11 = mix(c011, c111, u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// The cloud density field: 4 octaves in two independently drifting sets.
// Set A (3 octaves, zonal drift) carries the main cloud masses; set B (one
// mid-frequency octave on a different axis and speed) makes the sum evolve
// over time instead of rotating rigidly. Pure function of (planet-fixed
// direction, time, seed) -- exactly the sampling contract the increment-2
// raymarcher needs. The raw amplitude-normalized sum is contrast-stretched
// through its empirical window (see CLOUD_FIELD_LO/HI) so the output is a
// roughly uniform 0..1 "cloudiness".
fn cloud_field(dir: vec3<f32>, t: f32, seed: f32) -> f32 {
    let da0 = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    // Band stretch (see CLOUD_BAND_STRETCH): re-normalized so the triplanar
    // weights in cloud_noise still see a unit direction.
    let da = normalize(vec3<f32>(da0.x, da0.y * CLOUD_BAND_STRETCH, da0.z));
    let db = cloud_rot_x(dir, t * CLOUD_DRIFT_CROSS);
    // v0.999.x cell-size retune (operator: "the big cloud patches are so
    // big that they can cover whole continents"): the octave ladder rises
    // from 5 cycles/planet (~2,500 km features) to 9 (~1,400 km synoptic
    // systems) with matching upper octaves, so the deck breaks into
    // mesoscale structure instead of continent slabs. BOTH stream sites
    // (this + renderer::clouds mirror) must stay identical.
    var f = 0.5 * cloud_noise(da, 9.0, seed);
    f = f + 0.25 * cloud_noise(da, 19.0, seed + 19.0);
    f = f + 0.125 * cloud_noise(da, 41.0, seed + 47.0);
    f = f + 0.0625 * cloud_noise(da, 83.0, seed + 83.0);
    f = f + 0.35 * cloud_noise(db, 13.0, seed + 101.0);
    return smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, f / 1.2875);
}

// Coverage (0..1, from the planet RON) -> cloud body opacity. Because
// cloud_field is ~uniform after its contrast stretch, the fraction of sky
// above a threshold thr is ~(1 - thr), so thr = 1 - coverage makes the knob
// track real sky fraction; the -CLOUD_EDGE endpoint lets coverage 1.0 reach
// FULL opacity everywhere (thr + edge <= 0) instead of leaving thin holes.
// smoothstep softens the edge. Monotonic in both arguments (unit-tested in
// renderer::clouds).
fn cloud_alpha_from_field(field: f32, coverage: f32) -> f32 {
    let thr = mix(1.0, -CLOUD_EDGE, clamp(coverage, 0.0, 1.0));
    // Dense-cloud edge sharpening (v0.887, operator's silver-lining photo:
    // "the thicker/heavier the cloud... more defined edges"): the alpha
    // ramp narrows as the field strengthens past the threshold, so heavy
    // masses get crisp borders while thin haze keeps the soft ramp.
    let base = smoothstep(thr, thr + CLOUD_EDGE, field);
    let dense = smoothstep(thr, thr + CLOUD_EDGE * 0.35, field);
    return mix(base, dense, base * base);
}

// Altitude envelope (increment 2): shapes density across the slab. r is in
// DRAWN-SHELL units (drawn shell = 1.0, so the slab spans BASE/SHELL ..
// TOP/SHELL). Smooth rise from the base, a full-density plateau through the
// middle (the drawn-shell radius u = 0.5 sits inside it, so the increment-1
// fragment altitude evaluates at envelope 1), fade to zero at the top.
// Pure in r; mirrored + unit-tested in renderer::clouds.
fn cloud_altitude_envelope(r: f32) -> f32 {
    let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
    let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
    let u = clamp((r - base) / (top - base), 0.0, 1.0);
    return smoothstep(0.0, 0.4, u) * (1.0 - smoothstep(0.6, 1.0, u));
}

// The increment-2 sampling contract from the increment-1 design note --
// horizontal coverage field times the altitude envelope -- with one response
// shaping: the horizontal alpha is SQUARED. Beer-Lambert accumulation is
// concave (1 - exp(-tau) inflates mid densities toward opaque), so feeding
// it the raw ~uniform alpha fused the whole deck into a pale shroud on the
// first orbital capture (2026-07-11) -- the same cue-ball failure increment
// 1 hit and solved with its core-vs-skirt density ramp. Squaring restores
// that response through the march: 1 - exp(-2.67 a^2) tracks increment 1's
// a * (0.4 + 0.6 a) skirt curve within a few percent across the range,
// keeping skirts translucent while cores still saturate. p is a point in
// the mesh's LOCAL frame (planet-fixed, drawn shell = radius 1).
fn cloud_density(p: vec3<f32>, t: f32, seed: f32, coverage: f32) -> f32 {
    let r = length(p);
    if (cloud_altitude_envelope(r) <= 0.0) {
        return 0.0; // outside the slab entirely - skip the field lookup
    }
    let a_h = cloud_alpha_from_field(cloud_field(normalize(p), t, seed), coverage);
    if (a_h <= 0.001) {
        return 0.0;
    }
    // Height squash (v0.999.x, operator: "the cloud edges ... are kind of
    // like sheer cliffs instead of gradual like real clouds with varying
    // height"): the deck TOP now scales with the horizontal density, so
    // thin skirts are LOW and cores tower - edges slope down into wisps
    // instead of ending as full-height walls, and the deck's roof gains
    // real height variation. Mirrored in renderer::clouds.
    let base = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
    let top = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
    let u = clamp((r - base) / (top - base), 0.0, 1.0);
    let squash = 0.30 + 0.70 * a_h;
    let uq = u / squash;
    if (uq > 1.0) {
        return 0.0;
    }
    let env = smoothstep(0.0, 0.4, uq) * (1.0 - smoothstep(0.6, 1.0, uq));
    return a_h * a_h * env;
}

// Quality dispatcher (clouds increment 3): the RUNTIME switch rides in
// material.params.y (Settings > Graphics > Cloud quality; producer lib.rs).
// 0 = Low (increment-1 painted deck), 1 = Medium (increment-2 field march),
// 2 = High (the volumetric 3D-noise system, the default). All three paths
// stay compiled and naga-validated, so no tier can rot.
fn cloud_layer(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    let quality = material.params.y;
    if (quality < 0.5) {
        return cloud_layer_flat(world_position, front_facing);
    }
    if (quality < 1.5) {
        return cloud_layer_march(world_position, front_facing);
    }
    return cloud_layer_volumetric(world_position, front_facing);
}

// Increment-1 fallback: one field sample at the fragment, painted-on deck.
// Aerial-perspective fade for a camera under the deck (v0.958): a deck
// fragment at the visual horizon sits behind ~160 km of air (slant =
// sqrt(2 R h) for a 2 km deck) - real clouds dissolve into the horizon
// haze there, but ours arrived as stark white/black SHEET BANDS riding
// the horizon line at grazing angles (the ocean-vantage slab artifact;
// clouds-off A/B proved the deck was the source). Slant fade 30..80 km,
// active ONLY when the camera is inside the deck shell - from orbit the
// far deck is the blue marble's face and must not fade. Shared by all
// three quality variants so Low/Medium/High agree.
fn cloud_low_cam_haze(
    world_position: vec3<f32>,
    cam_inside: bool,
    center: vec3<f32>,
    shell_r: f32,
) -> f32 {
    if (!cam_inside) {
        return 1.0;
    }
    // v0.974 rewrite: the v0.958 form faded on ABSOLUTE slant (30..80 km),
    // tuned for a 2 km deck -- but the drawn shell sits at 51 km altitude
    // (CLOUD_SHELL_SCALE 1.008), so from the ground even the ZENITH fragment
    // was 40% faded and everything below ~50 degrees elevation vanished:
    // cloud shadows swept the ground under a visually clear sky. Fade on the
    // GRAZING RATIO instead: slant divided by the camera's radial gap to the
    // shell is ~1/sin(elevation) for any shell height, so "grazing" is pure
    // geometry. Full deck above ~10 degrees elevation (ratio 6), dissolved
    // below ~4 degrees (ratio 14) -- the ocean-vantage horizon slabs sat at
    // ratio 15+ and stay dead, while the sky dome gets its clouds back.
    let slant = length(world_position - camera.view_pos.xyz);
    let gap = max(shell_r - length(camera.view_pos.xyz - center), 1.0);
    return 1.0 - smoothstep(6.0, 14.0, slant / gap);
}

fn cloud_layer_flat(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    // Shell center + radius recovered from the object transform, same trick
    // as the atmosphere shell: unit icosphere placed via Vec3::splat(scale),
    // so column 0's length IS the shell radius and column 3 the center.
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);

    // Exactly ONE shell layer (same rule as the atmosphere): the transparent
    // pipeline draws both faces (cull off, shared with glass). Keep front
    // faces when the camera is outside the shell, back faces when inside --
    // the inside view is the increment-2 under-the-deck flight case, which
    // this rule already handles correctly.
    let ro = (camera.view_pos.xyz - center) / shell_r;
    let cam_inside = dot(ro, ro) < 1.0;
    if (front_facing == cam_inside) {
        discard;
    }

    // PLANET-FIXED sample direction: rotate the world direction back into
    // the mesh's local frame so the pattern rides the planet's spin and the
    // drift constants are true weather motion relative to the ground.
    // transpose(normal_matrix) IS model.inverse() exactly (normal_matrix is
    // inverse-transpose), so no matrix inversion is needed in the shader.
    let inv_model = transpose(obj_normal_matrix());
    let dir = normalize((inv_model * vec4<f32>(world_position, 1.0)).xyz);

    let t = camera.sun_color.w; // the cloud clock (see header comment)
    let seed = material.params.x;
    let coverage = material.base_color.a;

    let field = cloud_field(dir, t, seed);
    let body = cloud_alpha_from_field(field, coverage);
    if (body <= 0.002) {
        // Clear sky at this fragment: fully transparent, skip the lighting.
        return vec4<f32>(0.0);
    }

    // Macro lighting from the SPHERE normal: the deck is a thin wrap, so the
    // planet's own day/night curvature dominates. Computed from geometry
    // (position - center), not the interpolated mesh normal, so the level-3
    // icosphere facets never show in the shading.
    let n = normalize(world_position - center);
    let sun = normalize(camera.sun_direction.xyz);
    let ndl = dot(n, sun);
    // Soft terminator; the night side fades to near-black (clouds are lit by
    // the sun alone -- moonlight/city glow are future increments).
    let day = smoothstep(-0.05, 0.3, ndl);

    // Cheap self-shadow: re-sample the field a short great-circle step
    // TOWARD the sun (sun projected into the tangent plane at dir; the
    // projection goes to zero when the sun is overhead, so the step -- and
    // the shadow -- smoothly vanish there, with no normalize-of-zero NaN).
    // If density RISES toward the sun, this fragment sits on the shaded
    // flank of a cloud mass -> darken. Fake but effective from orbit: flat
    // coverage blobs pick up an internal sun-facing gradient and read puffy.
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let tang = sun_local - dir * dot(sun_local, dir);
    let sdir = normalize(dir + tang * CLOUD_SHADOW_STEP);
    let field_sun = cloud_field(sdir, t, seed);
    let shade = 1.0
        - CLOUD_SHADOW_STRENGTH
            * clamp((field_sun - field) * CLOUD_SHADOW_SHARP, 0.0, 1.0);

    // Silver lining: HG forward lobe (the atmosphere's phase function,
    // reused) at THIN edges -- thick cores block the forward-scattered sun,
    // so weight by (1 - body). Gated by a twilight-wide day window so the
    // deep night limb never glows.
    let rd = normalize(world_position - camera.view_pos.xyz);
    let cos_vs = dot(rd, sun);
    let silver = CLOUD_SILVER_GAIN * atmo_mie_phase(cos_vs) * (1.0 - body)
        * smoothstep(-0.15, 0.1, ndl);

    // Sun energy matches the celestial pass's directional light so the deck
    // sits in the same exposure regime as the surface below it.
    let sun_energy = camera.sun_color.rgb * camera.sun_direction.w;
    let lit = clamp(ndl, 0.0, 1.0);
    var radiance = material.base_color.rgb
        * (sun_energy * (CLOUD_AMBIENT + lit * shade) * day
            + vec3<f32>(CLOUD_NIGHT_FLOOR));
    radiance = radiance + sun_energy * silver;

    // Same ACES curve as the rest of the pipeline: all math above is linear,
    // the render target view is sRGB, blending happens in linear space per
    // the WebGPU spec (the v0.802/v0.803 lesson: encode once, never twice).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Density ramp (2026-07-11 orbital field test): `body` saturates within
    // CLOUD_EDGE of the threshold, which painted every deck the same solid
    // white ("cue ball"). Re-shape by the field's headroom above the
    // threshold so cloud SKIRTS stay translucent and only the dense cores
    // approach max alpha -- the surface reads through most of the deck.
    let thr = mix(1.0, -CLOUD_EDGE, clamp(coverage, 0.0, 1.0));
    let t_core = clamp((field - thr) / max(1.0 - thr, CLOUD_EDGE), 0.0, 1.0);
    let density = 0.40 + 0.60 * t_core * t_core * (3.0 - 2.0 * t_core);
    // Limb fade: near the disc edge the shell is seen almost edge-on and
    // stacks over the atmosphere's own limb brightening into a hard white
    // ring; ease the deck off as the view grazes the sphere.
    let mu = clamp(abs(dot(rd, n)), 0.0, 1.0);
    let limb = mix(0.55, 1.0, smoothstep(0.0, 0.35, mu));
    let low_haze = cloud_low_cam_haze(world_position, cam_inside, center, shell_r);
    return vec4<f32>(mapped, body * density * limb * low_haze * CLOUD_MAX_ALPHA);
}

// Increment-2 raymarch: real thickness, parallax, and volumetric
// self-shadow. Everything happens in the mesh's LOCAL frame (planet-fixed,
// drawn shell = radius 1): the model transform is rotation + uniform scale,
// so directions transfer with one normalize and dot products are preserved.
fn cloud_layer_march(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);

    // Exactly ONE shell layer, same rule as the flat path and the
    // atmosphere: front faces when the camera is outside the drawn shell,
    // back faces when inside (the under-the-deck flight case).
    let ro_w = (camera.view_pos.xyz - center) / shell_r;
    let cam_inside = dot(ro_w, ro_w) < 1.0;
    if (front_facing == cam_inside) {
        discard;
    }

    // transpose(normal_matrix) IS model.inverse() exactly (see the flat
    // path); it maps world points into the unit-icosphere local frame.
    let inv_model = transpose(obj_normal_matrix());
    let ro = (inv_model * vec4<f32>(camera.view_pos.xyz, 1.0)).xyz;
    let rd_w = normalize(world_position - camera.view_pos.xyz);
    let rd = normalize((inv_model * vec4<f32>(rd_w, 0.0)).xyz);
    let dirf = normalize((inv_model * vec4<f32>(world_position, 1.0)).xyz);

    let t = camera.sun_color.w; // the cloud clock (see header comment)
    let seed = material.params.x;
    let coverage = material.base_color.a;

    // Slab interval along the ray: inside the TOP sphere, outside the BASE
    // sphere, in front of the camera. Only the FIRST such interval is
    // marched: a ray that dives below the base either hits the planet (the
    // far-side re-entry is depth-occluded) or grazes the limb where the near
    // interval alone already saturates opacity.
    let rb = CLOUD_BASE_SCALE / CLOUD_SHELL_SCALE;
    let rt = CLOUD_TOP_SCALE / CLOUD_SHELL_SCALE;
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= rt * rt) {
        return vec4<f32>(0.0); // grazing numeric miss of the whole slab
    }
    let thc_t = sqrt(rt * rt - d2);
    var m0 = max(tca - thc_t, 0.0);
    var m1 = tca + thc_t;
    if (m1 <= 0.0) {
        return vec4<f32>(0.0); // slab entirely behind the camera
    }
    if (d2 < rb * rb) {
        let thc_b = sqrt(rb * rb - d2);
        let b0 = tca - thc_b;
        let b1 = tca + thc_b;
        if (b0 > m0) {
            m1 = min(m1, b0); // clipped where the ray dives below the base
        } else if (b1 > m0) {
            m0 = b1; // started under the deck: begin at the base exit above
        }
    }
    if (m1 <= m0) {
        return vec4<f32>(0.0);
    }

    // Clear-sky gate: probe the horizontal field at the segment's start,
    // middle, and end before paying for the full march. Most pixels over a
    // partly-cloudy planet are clear; 3 field evaluations instead of ~20
    // keeps them cheap. (A cloud strictly between probes on a long grazing
    // segment can slip through -- only skirt-thin alpha is at stake.)
    let seg = m1 - m0;
    let probe = max(
        max(
            cloud_alpha_from_field(
                cloud_field(normalize(ro + rd * m0), t, seed), coverage),
            cloud_alpha_from_field(
                cloud_field(normalize(ro + rd * (m0 + seg * 0.5)), t, seed), coverage),
        ),
        cloud_alpha_from_field(
            cloud_field(normalize(ro + rd * m1), t, seed), coverage),
    );
    if (probe <= 0.002) {
        return vec4<f32>(0.0);
    }

    let sun = normalize(camera.sun_direction.xyz);
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let sun_energy = camera.sun_color.rgb * camera.sun_direction.w;

    // Stratified jitter from the planet-fixed fragment direction: one sample
    // offset shared by the whole ray de-bands the thin slab at grazing
    // angles without screen-space shimmer (the pattern rides the planet).
    let jitter = hash21(dirf.xy * 4096.0 + vec2<f32>(dirf.z * 1024.0, 17.0));

    // Front-to-back accumulation with early-out at opacity saturation.
    let dtm = seg / f32(CLOUD_MARCH_SAMPLES);
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    var acc_w = 0.0;
    for (var i = 0; i < CLOUD_MARCH_SAMPLES; i = i + 1) {
        let tm = m0 + (f32(i) + jitter) * dtm;
        let p = ro + rd * tm;
        let dens = cloud_density(p, t, seed, coverage);
        if (dens <= 0.0005) {
            continue; // empty sample: skip the lighting taps
        }
        let a_i = 1.0 - exp(-CLOUD_SIGMA_T * dens * dtm);
        // Macro lighting from the sample's own sphere normal (local frame
        // preserves dots), soft terminator as in increment 1.
        let n_i = normalize(p);
        let ndl = dot(n_i, sun_local);
        let day = smoothstep(-0.05, 0.3, ndl);
        let lit = clamp(ndl, 0.0, 1.0);
        // One-tap self-shadow: density gradient toward the sun in 3D.
        let d_sun = cloud_density(
            p + sun_local * CLOUD_MARCH_SHADOW_STEP, t, seed, coverage);
        let shade = 1.0
            - CLOUD_SHADOW_STRENGTH
                * clamp((d_sun - dens) * CLOUD_MARCH_SHADOW_SHARP, 0.0, 1.0);
        // Height gradient: bases darker, tops brighter.
        let u_h = clamp((length(p) - rb) / (rt - rb), 0.0, 1.0);
        let grad = mix(CLOUD_BASE_DARKEN, 1.0, u_h);
        let c_i = material.base_color.rgb
            * (sun_energy * (CLOUD_AMBIENT + lit * shade * grad) * day
                + vec3<f32>(CLOUD_NIGHT_FLOOR));
        acc = acc + c_i * (trans * a_i);
        acc_w = acc_w + trans * a_i;
        trans = trans * (1.0 - a_i);
        if (trans <= 0.02) {
            break; // opacity saturated: the rest of the slab is invisible
        }
    }
    let body_total = 1.0 - trans;
    if (body_total <= 0.003) {
        return vec4<f32>(0.0);
    }
    // Transmittance-weighted mean color of the marched samples.
    var radiance = acc / max(acc_w, 1.0e-4);

    // Silver lining: same HG forward lobe + thin-edge weighting + twilight
    // gate as increment 1, driven by the marched total instead of the single
    // sample.
    let n_frag = normalize(world_position - center);
    let cos_vs = dot(rd_w, sun);
    let silver = CLOUD_SILVER_GAIN * atmo_mie_phase(cos_vs) * (1.0 - body_total)
        * smoothstep(-0.15, 0.1, dot(n_frag, sun));
    radiance = radiance + sun_energy * silver;

    // Same ACES curve as the rest of the pipeline (linear in, sRGB target).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Limb fade, as in increment 1: the deck stacks over the atmosphere's
    // own limb brightening at grazing view angles; ease it off there.
    let mu = clamp(abs(dot(rd_w, n_frag)), 0.0, 1.0);
    let limb = mix(0.55, 1.0, smoothstep(0.0, 0.35, mu));
    let low_haze = cloud_low_cam_haze(world_position, cam_inside, center, shell_r);
    return vec4<f32>(mapped, body_total * limb * low_haze * CLOUD_MAX_ALPHA);
}

// ── Increment-3 volumetric helpers (Rust mirrors: renderer::clouds) ──

// The remap every cloud paper calls Remap: rescale v from [l0,h0] to
// [l1,h1]. No clamping -- callers clamp.
fn cloud_remap(v: f32, l0: f32, h0: f32, l1: f32, h1: f32) -> f32 {
    return l1 + (v - l0) / (h0 - l0) * (h1 - l1);
}

// Henyey-Greenstein lobe, RELATIVE normalization (1.0 everywhere at g = 0,
// so multiplying by it never globally dims -- the 1/4pi absolute constant
// is folded into the sun-energy calibration).
fn cloud_hg(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / pow(max(1.0 + g2 - 2.0 * g * cos_t, 1.0e-4), 1.5);
}

// Dual-lobe phase: forward silver-lining lobe + mild back lobe.
fn cloud_phase(cos_t: f32) -> f32 {
    return mix(
        cloud_hg(cos_t, CLOUD_HG_BACK),
        cloud_hg(cos_t, CLOUD_HG_FWD),
        CLOUD_HG_FWD_WEIGHT,
    );
}

// Weather map: increment 1's cloud_field minus its two finest octaves --
// at High quality the 3D volumes own every feature below ~50 km, so the
// weather only PLACES the big masses (and keeps increment 1's drift and
// band-stretch posture so coverage semantics and motion carry over).
// Amplitude renormalized (0.5 + 0.25 + 0.35 = 1.10) through the same
// empirical contrast window.
fn cloud_weather(dir: vec3<f32>, t: f32, seed: f32) -> f32 {
    let da0 = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    let da = normalize(vec3<f32>(da0.x, da0.y * CLOUD_BAND_STRETCH, da0.z));
    let db = cloud_rot_x(dir, t * CLOUD_DRIFT_CROSS);
    // Five octaves from synoptic (~2500 km systems) down to broken fields
    // (~100 km): the old 3-octave field stopped at globe scale, so coverage
    // read as single continuous splotches spanning hemispheres (operator
    // 2026-07-17). The added meso/regional octaves carve every large mass
    // into fronts, bands, and broken decks like real satellite imagery.
    // Split macro (placement, 0.64 amplitude) from meso/fine (texture, 0.40)
    // so the live weather map can OWN placement where it has real data.
    let macro_f = 0.40 * cloud_noise(da, 5.0, seed)
        + 0.24 * cloud_noise(da, 13.0, seed + 19.0);
    let meso_f = 0.20 * cloud_noise(db, 7.0, seed + 101.0)
        + 0.12 * cloud_noise(da, 31.0, seed + 233.0)
        + 0.08 * cloud_noise(db, 67.0, seed + 409.0);
    // Live weather (v0.874): sample the real MODIS cloud fraction with the
    // UNDRIFTED planet-local direction (real weather pins to geography; only
    // the procedural texture octaves drift). Equirect UV matches the planet
    // albedo mapping above: east = -z, +Y = north. albedo_sampler wraps u
    // (antimeridian) and clamps v (poles). textureSampleLevel because this
    // runs inside the raymarch loop (non-uniform control flow).
    let w_lon = atan2(-dir.z, dir.x);
    let w_lat = asin(clamp(dir.y, -1.0, 1.0));
    let w_uv = vec2<f32>(w_lon * 0.15915494 + 0.5, 0.5 - w_lat * 0.31830987);
    let w = textureSampleLevel(weather_map, albedo_sampler, w_uv, 0.0).rg;
    let proc = smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, (macro_f + meso_f) / 1.04);
    // The MODIS DAILY fraction is nearly binary ("was cloudy at any point
    // today" saturates most of the globe to 100% -- rendering it 1:1 gave a
    // full whiteout). So the map is a placement MASK, not an opacity: inside
    // real cloudy zones the procedural meso/fine octaves carve the actual
    // broken deck (~instantaneous look); real clear zones (deserts) go clear.
    let envelope = smoothstep(0.35, 0.9, w.r);
    let live = envelope * smoothstep(0.15, 0.7, meso_f * 2.5);
    return mix(proc, live, w.g);
}

// ── Cloud-TYPE regimes (v0.828: the four real-Earth cloud families) ──
//
// Real Blue-Marble skies show several cloud types at once: thin cirrus streaks
// high up, puffy cumulus clusters mid-level, flat overcast stratus decks low
// down, and broken stratocumulus in between. We drive all four from ONE
// low-frequency "type coordinate" over the sphere (like real air masses), then
// derive every per-regime property (height band, opacity, coverage bias,
// erosion, streakiness, tint) as a smooth weighted blend -- so the disc shows
// every type simultaneously with no hard boundaries. Order everywhere is:
//   x = CIRRUS  y = CUMULUS  z = STRATUS/overcast  w = STRATOCUMULUS/broken

// The blended per-regime parameters for one ray.
struct CloudRegime {
    h_lo: f32,       // slab-fraction bottom of this regime's height band
    h_hi: f32,       // slab-fraction top of the band
    opacity: f32,    // density scale (cirrus faint, cumulus solid)
    cover_bias: f32, // added to coverage (stratus fills to overcast)
    fray: f32,       // coarse edge-fray strength (frayed vs smooth)
    fine: f32,       // fine cauliflower strength (close-up billow)
    stretch: f32,    // domain anisotropy (cirrus streaks east-west)
    filament: f32,   // ridged-filament streaking (cirrus)
    tint: f32,       // luminance factor (overcast reads greyer)
};

// The carved cloud body plus the values the fray/detail passes reuse.
struct CloudSample {
    carve: f32,      // coverage-carved, height-shaped body in [0,1] (pre-fray)
    ps: vec3<f32>,   // the drifted + stretched sample position (tap domain)
    h: f32,          // slab fraction at the sample
};

// The type coordinate at a planet-fixed direction: two low-frequency octaves
// so regime patches are organic (not a few giant zones). In [0,1].
fn cloud_type_coord(dir: vec3<f32>, t: f32, seed: f32) -> f32 {
    let d = cloud_rot_y(dir, t * CLOUD_DRIFT_ZONAL);
    let a = cloud_noise(d, CLOUD_TYPE_FREQ, seed + 211.0);
    let b = cloud_noise(d, CLOUD_TYPE_FREQ2, seed + 331.0);
    return clamp(0.62 * a + 0.38 * b, 0.0, 1.0);
}

// Regime weights: overlapping smootherstep tents around four centers spread
// across [0,1], normalized so they sum to 1 -- a smooth partition of unity, so
// the blend is seamless everywhere. Mirrored + unit-tested in renderer::clouds.
// v0.893: 7 families (was 4). Interleaved so the original four keep their
// type-coordinate anchors; the newcomers fill the gaps: altocumulus (patchy
// mid-deck), cumulonimbus (storm towers to the slab top), nimbostratus
// (dark rain overcast). Evaluated ONCE per ray, so 7-wide costs nothing.
//
// Blend the per-regime parameter tables by overlapping smoothstep tents
// around 7 centers, normalized to a partition of unity. The tables ARE the
// design of each cloud family -- keep them numerically identical with the
// Rust mirror (renderer::clouds::cloud_regime); the regime tests lock the
// blended output. IMPLEMENTATION NOTE: everything is accumulated in ONE
// loop with scalar accumulators because naga's HLSL backend cannot pass
// array<f32, N> values across function boundaries (FXC X3017 "cannot
// convert from 'float[7]' to 'float'" -- crashed the v0.893 first cut at
// pipeline creation). Local arrays indexed in-function are fine.
fn cloud_regime(tc: f32) -> CloudRegime {
    var centers = array<f32, 7>(0.0, 0.17, 0.33, 0.5, 0.67, 0.83, 1.0);
    //                 cirrus altocu cumulus cumulonimb stratus nimbostr stratocu
    var t_h_lo    = array<f32, 7>(0.68, 0.42, 0.05, 0.02, 0.00, 0.00, 0.05);
    var t_h_hi    = array<f32, 7>(1.00, 0.62, 0.72, 1.00, 0.20, 0.45, 0.40);
    var t_opacity = array<f32, 7>(0.34, 0.55, 1.00, 1.00, 0.80, 0.95, 0.62);
    var t_cover   = array<f32, 7>(0.06, 0.02, -0.03, 0.00, 0.34, 0.42, 0.03);
    var t_fray    = array<f32, 7>(1.00, 0.85, 0.55, 0.35, 0.18, 0.10, 0.80);
    var t_fine    = array<f32, 7>(0.35, 0.90, 0.95, 0.90, 0.30, 0.25, 0.80);
    var t_stretch = array<f32, 7>(3.40, 1.60, 1.15, 1.05, 1.50, 1.40, 1.70);
    var t_fil     = array<f32, 7>(0.90, 0.25, 0.10, 0.05, 0.04, 0.02, 0.30);
    // v0.909 (operator: "cumulonimbus is a lot darker due to the higher
    // denser water content... thinner clouds bright white, the dark ones
    // fairly dark grey"): tint spread widened hard - storm/rain families
    // now read properly dark while cirrus/cumulus stay brilliant.
    var t_tint    = array<f32, 7>(1.00, 0.96, 0.98, 0.55, 0.74, 0.42, 0.85);
    let hw = 0.22;
    var s = 0.0;
    var h_lo = 0.0;
    var h_hi = 0.0;
    var opacity = 0.0;
    var cover = 0.0;
    var fray = 0.0;
    var fine = 0.0;
    var stretch = 0.0;
    var fil = 0.0;
    var tint = 0.0;
    for (var i = 0u; i < 7u; i = i + 1u) {
        var wi = clamp(1.0 - abs(tc - centers[i]) / hw, 0.0, 1.0);
        wi = wi * wi * (3.0 - 2.0 * wi); // smoothstep each tent
        s = s + wi;
        h_lo = h_lo + wi * t_h_lo[i];
        h_hi = h_hi + wi * t_h_hi[i];
        opacity = opacity + wi * t_opacity[i];
        cover = cover + wi * t_cover[i];
        fray = fray + wi * t_fray[i];
        fine = fine + wi * t_fine[i];
        stretch = stretch + wi * t_stretch[i];
        fil = fil + wi * t_fil[i];
        tint = tint + wi * t_tint[i];
    }
    let inv = 1.0 / max(s, 1.0e-4);
    return CloudRegime(
        h_lo * inv,
        h_hi * inv,
        opacity * inv,
        cover * inv,
        fray * inv,
        fine * inv,
        stretch * inv,
        fil * inv,
        tint * inv,
    );
}

// Height envelope over the slab fraction h for a regime's [h_lo, h_hi] band:
// smooth rise off the base, plateau, smooth fall to the top. Mirrored + tested.
fn cloud_height_band(h: f32, h_lo: f32, h_hi: f32) -> f32 {
    let a = mix(h_lo, h_hi, 0.30);
    let b = mix(h_lo, h_hi, 0.62);
    return smoothstep(h_lo, a, h) * (1.0 - smoothstep(b, h_hi, h));
}

// Anisotropic domain stretch: slow the sample coordinate along the ZONAL
// tangent (east-west, perpendicular to the spin axis Y) by `stretch`, so noise
// features elongate into east-west streaks -- cirrus mares'-tails and jet
// banding. At the poles the tangent vanishes and the stretch smoothly no-ops.
// Pure; mirrored + unit-tested.
fn cloud_stretch_domain(p: vec3<f32>, dir: vec3<f32>, stretch: f32) -> vec3<f32> {
    var tang = cross(vec3<f32>(0.0, 1.0, 0.0), dir);
    let tl = length(tang);
    if (tl < 1.0e-4) {
        return p;
    }
    tang = tang / tl;
    // Reduce p's projection on the tangent so features vary slower there.
    return p - tang * dot(p, tang) * (1.0 - 1.0 / stretch);
}

// The coverage-carved, height-shaped cloud BODY (pre-fray) plus the stretched
// tap domain the fray/detail passes reuse. Shared by the view march and the
// (cheaper) light march so shadows and shading agree on where cloud is.
fn cloud_carve(p: vec3<f32>, t: f32, seed: f32, wa: f32, reg: CloudRegime) -> CloudSample {
    let r = length(p);
    let h = clamp((r - CLOUD_RB) / (CLOUD_RT - CLOUD_RB), 0.0, 1.0);
    // Towering (v0.880, operator: "real clouds have a variety of heights").
    // Dense columns BUILD VERTICALLY: the effective band top rises with the
    // local coverage, scaled by the regime's own band thickness - so solid
    // cumulus masses tower toward the slab top while thin stratus decks and
    // sparse fields stay flat. The light march shares this function, so
    // tower shadows stay consistent.
    let tower = smoothstep(0.55, 1.0, wa);
    let h_hi_eff = min(reg.h_hi + tower * 0.8 * (reg.h_hi - reg.h_lo), 1.0);
    let env = cloud_height_band(h, reg.h_lo, h_hi_eff);
    if (env <= 0.002 || wa <= 0.003) {
        return CloudSample(0.0, p, h);
    }
    // Drift the sample like weather set A, then stretch for streaks.
    let ps0 = cloud_rot_y(p, t * CLOUD_DRIFT_ZONAL);
    let ps = cloud_stretch_domain(ps0, normalize(p), reg.stretch);
    let s = textureSampleLevel(
        cloud_shape_tex, cloud_tile_sampler, ps * CLOUD_SHAPE_FREQ, 0.0);
    let lofi = s.g * 0.625 + s.b * 0.25 + s.a * 0.125;
    let body = clamp(cloud_remap(s.r, lofi - 1.0, 1.0, 0.0, 1.0), 0.0, 1.0);
    let thr = mix(CLOUD_COV_LO, CLOUD_COV_HI, wa);
    let carve = clamp((body - thr) / CLOUD_COV_SOFT, 0.0, 1.0) * env;
    return CloudSample(carve, ps, h);
}

// The increment-3 VIEW density: the carved body, then TWO erosion bands and a
// filament streaking pass, then the density-power thin-edge shaping. `weather_a`
// is the caller's coverage value (regime bias already folded in). `detail_amt`
// (0..1) fades ONLY the fine cauliflower band with camera distance -- the
// coarse fray band is always on, which is what gives the ORBITAL marble its
// wispy frayed edges (the fix for the "giant blotches": before, all erosion
// faded with distance and orbit saw only smooth round blobs).
fn cloud_density_hi(
    p: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
    detail_amt: f32,
) -> f32 {
    let cs = cloud_carve(p, t, seed, weather_a, reg);
    var base = cs.carve;
    if (base <= 0.003) {
        return 0.0;
    }
    // COARSE fray (always on -> orbit wispiness): erode edges with the detail
    // volume's Worley FBM sampled at a LOW world frequency (~88 km features,
    // supra-pixel from orbit so no stipple), in the same stretched domain so
    // it streaks. Erode HARDER where the body is thin (the 1-base weight):
    // frayed filaments at the edges, solid cores -- erode-edges-keep-cores.
    let fr = textureSampleLevel(
        cloud_detail_tex, cloud_tile_sampler, cs.ps * CLOUD_FRAY_FREQ, 0.0);
    let frfbm = fr.r * 0.625 + fr.g * 0.25 + fr.b * 0.125;
    let erode_c = frfbm * reg.fray * CLOUD_FRAY_ERODE * (0.35 + 0.65 * (1.0 - base));
    base = clamp(cloud_remap(base, erode_c, 1.0, 0.0, 1.0), 0.0, 1.0);
    // FILAMENT streaking: the ridged-Perlin channel (detail alpha) frays flat
    // sheets into thin branching streaks. Weighted by the regime (cirrus high,
    // cumulus ~none) so only the high thin clouds get mares'-tail structure.
    let fmask = smoothstep(CLOUD_FIL_LO, CLOUD_FIL_HI, fr.a);
    base = base * mix(1.0, fmask, reg.filament);
    if (base <= 0.003) {
        return 0.0;
    }
    // FINE cauliflower (near only): high-frequency Worley erosion, phase
    // flipping with height (wispy bases, billowy tops). Fades out with
    // distance so orbit stays smooth -- the standard Nubis distance trick.
    if (detail_amt > 0.01) {
        let d = textureSampleLevel(
            cloud_detail_tex, cloud_tile_sampler, cs.ps * CLOUD_DETAIL_FREQ, 0.0);
        let dfbm = d.r * 0.625 + d.g * 0.25 + d.b * 0.125;
        let dmod = mix(dfbm, 1.0 - dfbm, clamp(cs.h * 3.0, 0.0, 1.0))
            * CLOUD_DETAIL_ERODE * reg.fine * detail_amt;
        base = clamp(cloud_remap(base, dmod, 1.0, 0.0, 1.0), 0.0, 1.0);
    }
    // Thin-edge shaping: pow > 1 makes low densities translucent (see-through
    // skirts) while cores stay opaque, then the regime opacity scales the whole
    // (cirrus faint, cumulus solid).
    return pow(base, CLOUD_DENSITY_POW) * reg.opacity;
}

// The LIGHT-march density: carved body only (no fray/detail taps -- edges err
// slightly thick, which reads as soft shadow and halves the texture cost),
// with the same pow + opacity shaping so shadow depth matches the view body.
fn cloud_density_light(
    p: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
) -> f32 {
    let cs = cloud_carve(p, t, seed, weather_a, reg);
    if (cs.carve <= 0.003) {
        return 0.0;
    }
    return pow(cs.carve, CLOUD_DENSITY_POW) * reg.opacity;
}

// Optical depth toward the sun from a sample point: CLOUD_HI_LIGHT_SAMPLES
// taps with quadratically widening spacing (dense near the point for
// self-shadow detail, sparse toward the slab exit for the big-mass shadow).
fn cloud_sun_tau(
    p: vec3<f32>,
    sun_local: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
) -> f32 {
    var tau = 0.0;
    var prev_d = 0.0;
    for (var i = 0; i < CLOUD_HI_LIGHT_SAMPLES; i = i + 1) {
        let fi = f32(i);
        let dist = CLOUD_LIGHT_STEP * (fi + 1.0)
            + CLOUD_LIGHT_STEP * 0.35 * fi * fi;
        let seg = dist - prev_d;
        prev_d = dist;
        let lp = p + sun_local * dist;
        let dens = cloud_density_light(lp, t, seed, weather_a, reg);
        tau = tau + CLOUD_HI_SIGMA_T * CLOUD_LIGHT_SIGMA_MULT * dens * seg;
        // v0.911 (perf audit #3): once the sun path is this optically deep
        // every scatter octave is effectively zero - later taps cannot
        // change the pixel. Saves up to half the light taps inside dense
        // decks (the 10-16 FPS worst case), bit-identical output.
        if (tau > 10.0) {
            break;
        }
    }
    return tau;
}

// Sun in-scatter energy at optical depth tau: a 3-octave multiple-
// scattering approximation (Wrenninge-style -- each octave attenuates
// sigma and widens the phase toward isotropic), so deep cores fade to a
// diffuse glow instead of going black the way single-scatter Beer does.
fn cloud_scatter_energy(tau: f32, phase: f32) -> f32 {
    var e = phase * exp(-tau);
    e = e + 0.45 * mix(1.0, phase, 0.5) * exp(-tau * 0.25);
    e = e + 0.18 * exp(-tau * 0.06);
    return e;
}

// Increment-3 raymarch (High quality): precomputed tiling 3D noise +
// weather map + per-sample light march. Same spherical-slab geometry, ray
// setup, probe gate, and compositing posture as the increment-2 march; the
// interior is the standard photoreal recipe -- exponential view sampling,
// Beer-Lambert light march with Beer-powder, dual-lobe HG phase, height-
// proportional ambient.
fn cloud_layer_volumetric(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);

    // Exactly ONE shell layer (same rule as every other cloud path).
    let ro_w = (camera.view_pos.xyz - center) / shell_r;
    let cam_inside = dot(ro_w, ro_w) < 1.0;
    if (front_facing == cam_inside) {
        discard;
    }

    let inv_model = transpose(obj_normal_matrix());
    let ro = (inv_model * vec4<f32>(camera.view_pos.xyz, 1.0)).xyz;
    let rd_w = normalize(world_position - camera.view_pos.xyz);
    let rd = normalize((inv_model * vec4<f32>(rd_w, 0.0)).xyz);
    let dirf = normalize((inv_model * vec4<f32>(world_position, 1.0)).xyz);

    let t = camera.sun_color.w;
    let seed = material.params.x;
    let coverage = material.base_color.a;

    // Slab interval along the ray (identical geometry to the Medium march).
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= CLOUD_RT * CLOUD_RT) {
        return vec4<f32>(0.0);
    }
    let thc_t = sqrt(CLOUD_RT * CLOUD_RT - d2);
    var m0 = max(tca - thc_t, 0.0);
    var m1 = tca + thc_t;
    if (m1 <= 0.0) {
        return vec4<f32>(0.0);
    }
    if (d2 < CLOUD_RB * CLOUD_RB) {
        let thc_b = sqrt(CLOUD_RB * CLOUD_RB - d2);
        let b0 = tca - thc_b;
        let b1 = tca + thc_b;
        if (b0 > m0) {
            m1 = min(m1, b0);
        } else if (b1 > m0) {
            m0 = b1;
        }
    }
    if (m1 <= m0) {
        return vec4<f32>(0.0);
    }

    // Cloud regime for this ray (sampled mid-segment; type cells are ~2000 km,
    // so per-sample evaluation would buy nothing). Computed BEFORE the gate so
    // its coverage bias -- which lets a stratus air mass fill to overcast even
    // where the raw weather is thin -- is included in the clear-sky test.
    let seg = m1 - m0;
    let mid_dir = normalize(ro + rd * (m0 + seg * 0.5));
    let reg = cloud_regime(cloud_type_coord(mid_dir, t, seed));

    // Clear-sky gate: 3 weather probes (regime coverage bias folded in) before
    // paying for the march.
    let probe = max(
        max(
            clamp(cloud_alpha_from_field(
                cloud_weather(normalize(ro + rd * m0), t, seed), coverage)
                + reg.cover_bias, 0.0, 1.0),
            clamp(cloud_alpha_from_field(
                cloud_weather(mid_dir, t, seed), coverage)
                + reg.cover_bias, 0.0, 1.0),
        ),
        clamp(cloud_alpha_from_field(
            cloud_weather(normalize(ro + rd * m1), t, seed), coverage)
            + reg.cover_bias, 0.0, 1.0),
    );
    if (probe <= 0.002) {
        return vec4<f32>(0.0);
    }

    let sun = normalize(camera.sun_direction.xyz);
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let sun_energy = camera.sun_color.rgb * camera.sun_direction.w;

    // Phase + powder gate are per-RAY (cos view-sun is constant along it).
    let cos_vs = dot(rd_w, sun);
    let phase = cloud_phase(cos_vs);
    // Beer-powder shows on the sun-facing side of masses, i.e. when the
    // sun is roughly BEHIND the camera; looking toward the sun the forward
    // lobe (silver lining) must win, so the powder eases off there.
    let powder_gate = smoothstep(0.3, 0.9, cos_vs);

    // Stratified per-ray jitter, ANIMATED (v0.872): the old planet-fixed hash
    // dithered banding into a frozen stipple pattern. Adding a golden-ratio
    // step per cloud-clock tick keeps the dither moving so the eye averages
    // it out (the precursor to real temporal accumulation).
    let jitter = fract(
        hash21(dirf.xy * 4096.0 + vec2<f32>(dirf.z * 1024.0, 17.0))
            + fract(camera.sun_color.w * 7.0) * 0.618034,
    );

    // Exponentially spaced front-to-back march: t = m0 + seg * u^EXP puts
    // over half the samples in the nearest third of the segment -- the
    // foreground puffs get the budget, the far limb averages out.
    // v0.911 (perf audit #5): the sample COUNT scales with how much slab
    // the ray actually crosses - a straight-up path through the thin deck
    // (seg ~ one slab thickness) needs ~a third of the budget a grazing
    // limb path does. Same step distribution, fewer steps on short rays;
    // the under-deck flight worst case gets its samples back.
    let slab_h = CLOUD_RT - CLOUD_RB;
    let n_samp_f = clamp(seg / (slab_h * 6.0), 0.34, 1.0) * f32(CLOUD_HI_SAMPLES);
    let n_samp = max(i32(n_samp_f), 8);
    var s_prev = 0.0;
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    var acc_w = 0.0;
    for (var i = 0; i < n_samp; i = i + 1) {
        let fi = f32(i);
        let s_next = pow((fi + 1.0) / n_samp_f, CLOUD_HI_STEP_EXP);
        let dt = (s_next - s_prev) * seg;
        let sm = pow((fi + jitter) / n_samp_f, CLOUD_HI_STEP_EXP);
        let tm = m0 + sm * seg;
        s_prev = s_next;

        let p = ro + rd * tm;
        let dirp = normalize(p);
        let weather_a = clamp(
            cloud_alpha_from_field(cloud_weather(dirp, t, seed), coverage)
                + reg.cover_bias, 0.0, 1.0);
        // Distance fade for the FINE cauliflower band only: tm is the sample's
        // distance from the camera (drawn-shell units). Far/orbit samples get
        // detail_amt ~0 (no sub-pixel stipple); close fly-by samples get full
        // cauliflower. The COARSE fray band inside cloud_density_hi is always
        // on, so orbit keeps its wispy frayed edges.
        let detail_amt = 1.0 - smoothstep(CLOUD_DETAIL_FADE_NEAR, CLOUD_DETAIL_FADE_FAR, tm);
        let dens = cloud_density_hi(p, t, seed, weather_a, reg, detail_amt);
        if (dens <= 0.001) {
            continue;
        }
        let a_i = 1.0 - exp(-CLOUD_HI_SIGMA_T * dens * dt);

        // Day/night from the sample's own sphere normal (soft terminator).
        let ndl = dot(dirp, sun_local);
        let day = smoothstep(-0.05, 0.3, ndl);

        // Light march toward the sun + Beer-powder edge darkening.
        let tau = cloud_sun_tau(p, sun_local, t, seed, weather_a, reg);
        let powder = 1.0 - CLOUD_POWDER_STRENGTH * exp(-2.0 * tau);
        let pw = mix(powder, 1.0, powder_gate);
        let direct = cloud_scatter_energy(tau, phase) * pw;

        // Ambient skylight proportional to height in the slab: tops see the
        // sky dome, bases see mostly their own shadow.
        let h = clamp((length(p) - CLOUD_RB) / (CLOUD_RT - CLOUD_RB), 0.0, 1.0);
        let amb = mix(CLOUD_AMB_BASE, CLOUD_AMB_TOP, h);

        let c_i = material.base_color.rgb
            * (sun_energy * (direct + amb) * day + vec3<f32>(CLOUD_NIGHT_FLOOR));
        acc = acc + c_i * (trans * a_i);
        acc_w = acc_w + trans * a_i;
        trans = trans * (1.0 - a_i);
        if (trans <= 0.02) {
            break;
        }
    }
    let body_total = 1.0 - trans;
    if (body_total <= 0.003) {
        return vec4<f32>(0.0);
    }
    var radiance = acc / max(acc_w, 1.0e-4);
    // Regime tint: overcast stratus reads greyer (dimmer white); cirrus and
    // cumulus stay bright. A luminance factor, so it never shifts hue.
    radiance = radiance * reg.tint;
    // Column-density darkening (v0.909): an optically THICK column holds
    // far more water, and its interior light dies - so near-opaque masses
    // dim toward heavy grey while translucent wisps keep full brightness.
    // Rides body_total (the ray's own opacity), so one deck can show a
    // bright thin skirt around a dark dense core.
    radiance = radiance * (1.0 - 0.32 * smoothstep(0.72, 0.98, body_total));

    // Same ACES curve as the rest of the pipeline (linear in, sRGB target).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Limb fade, as in the other paths: ease the deck off where the view
    // grazes the sphere so it never stacks into a hard white ring.
    let n_frag = normalize(world_position - center);
    let mu = clamp(abs(dot(rd_w, n_frag)), 0.0, 1.0);
    let limb = mix(0.55, 1.0, smoothstep(0.0, 0.35, mu));
    let low_haze = cloud_low_cam_haze(world_position, cam_inside, center, shell_r);
    return vec4<f32>(mapped, body_total * limb * low_haze * CLOUD_HI_MAX_ALPHA);
}

