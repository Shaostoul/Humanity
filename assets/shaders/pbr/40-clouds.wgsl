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
// cloud-clock time.
//
// v0.1021 (operator: "the main cloud structure are moving a different way
// than the details of the clouds"): the LIVE MODIS envelope pins mass
// placement to real geography (real weather cannot drift), but every
// texture domain drifted at the old 0.0015 rad/s - interiors slid through
// their own pinned silhouettes at ~580 km/min. Drifting the MODIS lookup
// instead would wheel real geography (clear Sahara, ITCZ) around the
// planet within the hour, so the fix is the drift rate itself: ~75x
// slower = ~127 m/s equatorial = jet-stream speed. Structure and detail
// now move coherently, and the residual silhouette mismatch accrues at
// ~7.6 km/min - a gentle, realistic crawl. Slow evolution still comes
// from the two sets' differential motion.
const CLOUD_DRIFT_ZONAL: f32 = 0.00002;
const CLOUD_DRIFT_CROSS: f32 = -0.000012;
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
// The slab, in PLANET-RADIUS multiples. Since the clouds DEPTH increment
// (docs/design/clouds-depth.md) the REAL bounds are per-planet physical
// altitudes (material.params2: Earth 0.4-12 km) - these constants are the
// FALLBACK for a material with no params2 data, kept at their historical
// values so a stale material degrades to the old look instead of breaking.
// The old values were tuned for a 4x terrain vertical exaggeration deleted
// in v0.883.2, which left every cloud family 10-50x above its real band.
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
// Extinction per KILOMETRE at density 1 for the Medium march (clouds depth
// increment: metric, like the rest of the ladder - the old 560 per
// drawn-shell unit was calibrated against the deleted 51 km slab, and on
// the physical 0.4-12 km slab it left every deck a see-through gauze;
// the orbital marble lost its clouds entirely). Phase 2 raised the
// look-preserving 0.44 conversion ~4x toward physical: a family band is
// 1-2 km thick now, and the eroded density field averages well under
// 0.2, so the under-deck vertical path needs real per-km extinction to
// read as cloud instead of haze (real stratus is 30-100/km; we stay far
// below that because the density field, pow shaping and erosion already
// discount the medium).
const CLOUD_SIGMA_KM: f32 = 1.75;
// Self-shadow tap for the Medium march: a 3D offset TOWARD the sun of HALF
// THE SLAB THICKNESS (computed from g_cloud_rb/rt at the tap site since the
// clouds depth increment - the old fixed 0.004 drawn units was ~half of the
// legacy 51 km slab, which overshoots the physical 12 km slab entirely);
// density rising toward the sun = this sample sits in a cloud mass's shadow.
// SHARP converts the (envelope-scaled, so smaller) density difference into a
// usable shading range.
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
// Fly-through fix (v0.1025): the drawn shell radius is DYNAMIC now - it
// rises above the slab top while the camera is inside the layer, so
// fragments cover the whole sky (a camera above the mid-slab shell used
// to lose the upper half of its view: the operator's "clouds kind of
// disappear" at cloud level). The slab bounds in drawn-shell units
// therefore come from the planet/drawn radius ratio the engine passes in
// the material's emissive slot (material.params.w = planet_r / drawn_r).
// Zero ratio (a stale material) falls back to the legacy constants.
//
// WHO ACTUALLY SETS THEM (corrected 2026-07-31; the comment here used to
// claim "set at the top of cloud_layer_volumetric", which was false and is
// how a dead write survived from v0.1025 to v0.1073):
//   High   (cloud_layer_volumetric) - CALLS cloud_set_slab_bounds() as its
//          first statement after the face discard. It is the only writer,
//          and the reader of everything below.
//   Medium (cloud_layer_march)      - does NOT call it. It still derives
//          local bounds from the CLOUD_*_SCALE constants and ignores
//          params.w. Deliberate: Medium has ZERO probe-rig coverage (no
//          vantage in tests/visual/vantages.json selects it, and
//          scripts/probe-sweep.js never sets a cloud quality, so every
//          sweep runs High per the src/config.rs "high" default), and
//          Medium consuming params.w for the first time is exactly what
//          caused BUG-049. It gets its own bounds only once it has its own
//          storm + in-slab vantage.
//   Low    (cloud_layer_flat)       - never reads them at all. It used to
//          hold the ONLY assignment, which was unobservable: `var<private>`
//          is per-invocation storage and cloud_layer dispatches to exactly
//          ONE path per invocation, so the Low path's write could never
//          reach the High path's read.
//
// WHAT THE DEAD WRITE COST: High fell back to the static CLOUD_RB/RT
// (0.996032 / 1.003968), but src/lib.rs raises the DRAWN shell to
// CLOUD_TOP_SCALE + 0.004 = 1.016 R whenever the camera is inside
// ~1.05 * CLOUD_TOP_SCALE (altitude below ~399 km), so the marched slab
// landed at 0.996032*1.016 .. 1.003968*1.016 = 1.01197..1.02003 R, i.e.
// 76.3-127.6 km altitude on Earth instead of the intended 25.5-76.5 km -
// one full slab thickness too high, above the visible atmosphere, at every
// camera altitude below ~400 km. Above ~400 km the ratio is
// 1/CLOUD_SHELL_SCALE and the constants coincidentally agree, which is
// exactly why the orbital blue marble looked right and every ground and
// low-flight view did not.
var<private> g_cloud_rb: f32 = CLOUD_RB;
var<private> g_cloud_rt: f32 = CLOUD_RT;
// First-hit distance of the most recent cloud_march_core call, WORLD
// units (0 = the march saw no cloud). The octa pass reads it for
// per-texel history reprojection (slice B): the march's own first hit is
// the exact parallax distance for the content this texel shows, where
// the analytic shell-sphere hit is only right for cloud AT the shell -
// worst inside the slab, which is exactly where the operator still saw
// ghosting after the analytic cut.
var<private> g_march_first_t: f32 = 0.0;
// The v2 constructed body's soft-rind width in METRES, frozen ONCE per
// ray by cloud_march_core (2026-08-25, the operator's "rings extending
// from their center... like eyeballs").
//
// THE BUG IT KILLS: cloud_v2_body derived its rind from whatever `lodb`
// its caller passed - and cloud_sun_tau calls the density function EIGHT
// times per shading evaluation with a different per-tap `lod_t` (the
// geometric shadow ladder: 30/57/108/206/391/743 m segments). Because
// the v2 body is a DISTANCE FIELD, the rind is a metric radius, so each
// tap shaded a concentrically SHRUNKEN copy of the same lobe, and each
// tap's optical-depth riser printed that copy's boundary as a hard
// bright-or-dark ring: six nested rings per lobe, offset sunward (the
// "pupil"), breathing as the coverage threshold drifted past. The noise
// body was immune because a mip level has no radius; a distance field
// has one. The view march's own step-driven footprint latch added a
// seventh ring at every silhouette.
//
// Freezing it per ray keeps the band-limiting intent (the rind still
// widens with distance, one value per pixel, continuous across the
// screen) while making a ring centred on a lobe impossible: nothing
// downstream can vary the body's scale within one shading evaluation.
var<private> g_v2_foot_m: f32 = 0.0;

// May this march build CONSTRUCTED cloud bodies (Ultra)?
//
// Defaults to FALSE and each screen-march entry opts IN, which is the safe
// direction: a site that forgets to set it loses the built look, which is
// visible and cheap. The other polarity cost the operator a session - see the
// gate in cloud_carve for what happens when the bodies run in the octa map.
var<private> g_v2_allowed: bool = false;
// Drawn-shell units per kilometre (clouds depth increment): converts the
// metre-expressed noise ladder + fade constants below into the march's
// coordinate space, so feature sizes are anchored to physical lengths
// instead of a drawn radius that flips with camera altitude. Default is
// the legacy Earth value (1 / (6371 km * 1.008)).
var<private> g_cloud_upkm: f32 = 0.00015572;
// Derived per-invocation ladder (set in cloud_set_slab_bounds; all in
// drawn-shell units): texture tile frequencies + fade distances + the
// light-march first step.
var<private> g_shape_freq: f32 = 24.0;
var<private> g_detail_freq: f32 = 60.0;
var<private> g_puff_freq: f32 = 140.0;
var<private> g_fray_freq: f32 = 9.0;
var<private> g_detail_fade_near: f32 = 0.03;
var<private> g_detail_fade_far: f32 = 0.70;
var<private> g_puff_fade_near: f32 = 0.008;
var<private> g_puff_fade_far: f32 = 0.045;
var<private> g_light_near: f32 = 0.00014;
var<private> g_cell_freq: f32 = 800.0;
var<private> g_cell_fade_near: f32 = 0.0047;
var<private> g_cell_fade_far: f32 = 0.0093;
// Medium-march extinction per drawn-shell unit (the km sigma converted;
// default is the legacy Earth value so a caller that never sets bounds
// keeps the old look on the fallback slab). The HIGH path's extinction is
// per-family (CloudRegime.ext_km) since phase 3.
var<private> g_sigma_med: f32 = 560.0;
// Per-invocation slab bounds + metric ladder from the material. MUST be
// called before ANY g_cloud_* read. The physical slab (params2.x/y as
// planet-radius multiples, params2.z = planet radius km) comes from the
// planet def's cloud_base_km/cloud_top_km; a zero params2 (stale or
// non-planet material) falls back to the legacy constants.
fn cloud_set_slab_bounds() {
    let inv_drawn = material.params.w;
    if (inv_drawn > 0.001) {
        var rb_p = CLOUD_BASE_SCALE;
        var rt_p = CLOUD_TOP_SCALE;
        if (material.params2.y > 0.5) {
            rb_p = material.params2.x;
            rt_p = material.params2.y;
        }
        g_cloud_rb = rb_p * inv_drawn;
        g_cloud_rt = rt_p * inv_drawn;
        if (material.params2.z > 0.5) {
            // drawn_r_km = planet_r_km / inv_drawn, so units-per-km is
            // inv_drawn / planet_r_km.
            g_cloud_upkm = inv_drawn / material.params2.z;
        }
    }
    g_shape_freq = 1.0 / (CLOUD_SHAPE_TILE_KM * g_cloud_upkm);
    g_detail_freq = 1.0 / (CLOUD_DETAIL_TILE_KM * g_cloud_upkm);
    g_puff_freq = 1.0 / (CLOUD_PUFF_TILE_KM * g_cloud_upkm);
    g_fray_freq = 1.0 / (CLOUD_FRAY_TILE_KM * g_cloud_upkm);
    g_detail_fade_near = CLOUD_DETAIL_FADE_NEAR_KM * g_cloud_upkm;
    g_detail_fade_far = CLOUD_DETAIL_FADE_FAR_KM * g_cloud_upkm;
    g_puff_fade_near = CLOUD_PUFF_FADE_NEAR_KM * g_cloud_upkm;
    g_puff_fade_far = CLOUD_PUFF_FADE_FAR_KM * g_cloud_upkm;
    g_light_near = CLOUD_LIGHT_NEAR_KM * g_cloud_upkm;
    g_cell_freq = 1.0 / (CLOUD_CELL_TILE_KM * g_cloud_upkm);
    g_cell_fade_near = CLOUD_CELL_FADE_NEAR_KM * g_cloud_upkm;
    g_cell_fade_far = CLOUD_CELL_FADE_FAR_KM * g_cloud_upkm;
    // Extinction: per-km sigma over drawn-units-per-km = per drawn unit.
    g_sigma_med = CLOUD_SIGMA_KM / g_cloud_upkm;
}
// View-march samples through the slab. Exponentially spaced (dense near
// the entry point -- see CLOUD_HI_STEP_EXP) so the puffy foreground gets
// the detail budget and the far limb blurs gracefully.
const CLOUD_HI_SAMPLES: i32 = 48;
// ── Wave B step law (increment 9) ── near steps resolve the slab band
// (fraction of slab thickness; 1/16 of Earth's 11.6 km slab = ~725 m,
// matching the old exp-spacing's fine end), far steps grow with the ray
// cone (CONE_K pixel-widths per step: at 20 km distance with a 1.44 mrad
// pixel that is again ~700 m, so the two regimes meet smoothly). The cap
// is a guard, not the budget - the growing step terminates a limb-graze
// in roughly CLOUD_HI_SAMPLES iterations by itself.
const CLOUD_STEP_BAND_FRAC: f32 = 0.045;
const CLOUD_STEP_CONE_K: f32 = 24.0;
const CLOUD_STEP_ITER_CAP: i32 = 224;
// VERTICAL step ceiling: the slab's band structure (family envelopes,
// domed tops, base undulation) lives at slab scale no matter how wide the
// pixel footprint is, so the step's RADIAL component may never exceed
// this fraction of the slab. Without it the first cut of the step law
// strode 11.6 km - the whole slab in one step - on the temporal map's
// far nadir rays (250 km capture darkened 30%, speckle doubled, measured
// on the nearslab A/B pair). Grazing rays (radial speed ~ 0) stay
// footprint-ruled, which is correct - they cross bands slowly.
const CLOUD_STEP_VERT_FRAC: f32 = 0.08;
// SEGMENT-fraction ceiling: no ray may receive fewer samples across its
// marched extent than the old exp-law's budget gave it (~CLOUD_HI_SAMPLES).
// The footprint law's job in THIS increment is to kill the knee, the
// integer rung and the unsampled tail - NOT to cut limb sampling 5x: the
// first cut let map-path grazes stride 10+ km and the 250 km nearslab
// capture darkened 30% with doubled speckle (whether that darkening is
// actually TRUTH is increment 10's question, judged by the reference
// march - the sampling law must not smuggle it in). 1/48 = the old
// full-budget density as a per-ray floor.
const CLOUD_STEP_SEG_FRAC: f32 = 0.020833;
// INTERIOR mean-free-path refinement (increment 10): once the march is
// INSIDE cloud (previous sample's density above the gate), the step may
// not exceed TAU_MAX optical depths - cumulus (45/km) refines to ~22 m,
// stratus ~45 m, cirrus (1.2/km) stays on the coarse law. This is what
// turns the binary opaque/transparent texel coin-flip into a resolved
// density gradient. The phase-9 lesson stands: view refinement ALONE
// made speckle worse - it ships only together with the sun-ladder fix
// above and the field re-tune, judged against the converged reference.
const CLOUD_STEP_TAU_MAX: f32 = 0.75;
const CLOUD_STEP_INTERIOR_GATE: f32 = 0.02;
// Light-march taps toward the sun per lit view sample. Spacing widens with
// each tap (near taps catch self-shadowing detail, far taps the big mass).
const CLOUD_HI_LIGHT_SAMPLES: i32 = 12;
// GEOMETRIC light-march ladder (v0.1014, operator: "from above they mostly
// just look like a solid flat sheet"): the old arithmetic-quadratic ladder's
// FIRST tap was ~3.9 km (0.0006 shell units), so a sample near a dome crown
// saw tau = 0 in every direction - the whole top surface of the deck was
// lit dead flat regardless of its relief, which erased exactly the
// mound-and-valley shading that makes real cloud tops read 3D. Geometric
// spacing starts at ~0.9 km (dome-scale self-shadow) and multiplies by
// RATIO each tap, reaching ~125 km by tap 8 (big-mass shadows keep their
// range). Same 8-tap cost. NEAR is the first step, shell units.
// First light-march step in KILOMETRES (clouds depth increment: the ladder
// is metric now - see g_light_near). ~0.9 km keeps dome-crown relief
// self-shadowing on any planet.
// THE INTEGRATOR (increment 10): first tap 0.03 km / ratio 2.4 - the
// isolated control run measured this single change cutting speckle rms
// 3.5x, the most effective intervention ever measured on the dots. The
// old 0.9 km first tap put a sample point's nearest shadow probe 41
// optical depths deep in cumulus (45/km): the sun term was a coin flip
// between "first tap in cloud" (black) and "first tap in a gap" (blown),
// an 18.9x energy swing per texel. 0.03 km = 1.35 optical depths -
// resolved self-shadowing. Reach is now ~23.5 km (was 125): at physical
// extinctions any path that long is opaque (tau 40 early-out) except
// thin cirrus, whose shadows are faint anyway; the field re-tune below
// absorbs the energy shift, judged by the converged reference.
const CLOUD_LIGHT_NEAR_KM: f32 = 0.03;
// 1.9 x 12 taps (twin-calibrated): reach ~78 km, ladder-vs-fine-march
// error +0.9% on the isolation harness (2.4 x 8 measured the same mean
// but coarser coverage; the extra taps buy the long-shadow range back).
const CLOUD_LIGHT_RATIO: f32 = 1.9;
// (The old CLOUD_LIGHT_SIGMA_MULT view/shadow sigma split retired in
// phase 3: it existed because the view sigma was artificially low for
// alpha feathering; with a physical per-family medium both the view and
// the light march use CloudRegime.ext_km directly.)
// The High path's extinction is PER-FAMILY since phase 3: see
// CloudRegime.ext_km and the t_ext table (cumulus 45/km, cirrus 1.2/km,
// ...). The old single global sigma could not give cirrus and cumulus
// different water content, which is what made the orbital disc a binary
// white/clear stencil (fidelity finding 3).
// Peak alpha of the High deck. Above Medium's 0.72: photoreal cumulus
// cores genuinely block the ground; thin skirts stay translucent anyway.
const CLOUD_HI_MAX_ALPHA: f32 = 0.96;
// ── The noise ladder, in KILOMETRES (clouds depth increment) ──
// Every tile size and fade distance below is a physical length, converted
// to drawn-shell units per invocation (cloud_set_slab_bounds). The values
// are the exact km equivalents of the old per-drawn-unit constants on
// Earth's legacy 1.008 R shell, so the horizontal look is unchanged - what
// changed is that they no longer drift when the drawn radius flips with
// camera altitude, and they stay physical on any planet.
//
// SHAPE tile: ~268 km per tile -> base Worley cells (6/tile) ~45 km, the
// finest shape octave (24/tile) ~11 km - the "cloud mass" band.
const CLOUD_SHAPE_TILE_KM: f32 = 267.6;
// DETAIL tile: erosion features ~3..13 km (the distance fade removes them
// from orbit).
const CLOUD_DETAIL_TILE_KM: f32 = 107.0;
// How deeply the detail octaves erode the shape's edges (0 = off).
const CLOUD_DETAIL_ERODE: f32 = 0.38;
// Detail erosion distance fade (km of camera-to-sample distance): full
// cauliflower within NEAR, gone by FAR. Keeps the orbital marble smooth
// (the ~km detail is sub-pixel there and would alias into salt-and-pepper
// stipple) while the low fly-by keeps its billowy edges.
const CLOUD_DETAIL_FADE_NEAR_KM: f32 = 192.7;
const CLOUD_DETAIL_FADE_FAR_KM: f32 = 4495.0;
// ── PUFF band (v0.1011, clouds STRUCTURE arc: "real clouds are pillowy /
// cauliflower-like") ── a THIRD erosion band at ~4x the fine-detail
// frequency. The existing ladder bottomed out at ~2.2 km features, so
// everything smaller rendered smooth - but cauliflower lobes read at
// 100-700 m. This band carves ~0.5-1.8 km cavities into mass edges (which
// is what makes lobes) and only exists NEAR the camera: full within ~50 km
// (the deck overhead when standing on the surface), gone by ~290 km, so
// the orbital marble and the horizon-distance deck pay nothing.
// Shader-only tuning (texture-sampling path; not mirrorable).
const CLOUD_PUFF_TILE_KM: f32 = 45.9;
const CLOUD_PUFF_ERODE: f32 = 0.38;
const CLOUD_PUFF_FADE_NEAR_KM: f32 = 51.4;
const CLOUD_PUFF_FADE_FAR_KM: f32 = 289.0;
// ── Cumulus-cell split (phase 3, fidelity finding 4) ── a second tap of
// the SAME shape volume at a ~8 km tile whose Worley channel RAISES the
// coverage threshold between cells, splitting the shape volume's >= 11 km
// masses into discrete 1-2 km cumuli near the camera. Threshold-side (not
// erosion) because erosion can only nibble a blob's edges, never divide
// it. Distance-faded so orbit never changes.
const CLOUD_CELL_TILE_KM: f32 = 8.0;
const CLOUD_CELL_SPLIT: f32 = 0.15;
const CLOUD_CELL_FADE_NEAR_KM: f32 = 30.0;
const CLOUD_CELL_FADE_FAR_KM: f32 = 60.0;
// Crevice occlusion from the SAME puff noise (already sampled for the
// erosion, so this shading is free): surviving density next to a carved
// cavity darkens, which is what makes individual lobes read as 3D bumps
// even though the light march steps (~4 km) are far coarser than the
// lobes themselves. Fraction of ambient+scatter removed at full cavity.
const CLOUD_PUFF_AO: f32 = 0.60;
// ── Band-limited volume sampling (phase 5, the mip ladder) ──
// Both noise volumes carry full box-filtered mip chains (built CPU-side
// in renderer::cloud_noise::mip_chain). Every march sample computes its
// FOOTPRINT - the larger of the ray-cone width at that distance and the
// march step length - and each tap samples the mip whose voxels match
// that footprint, so distant clouds read pre-averaged noise instead of
// point-sampling full-frequency texels. This is the structural fix for
// distant shimmer: temporal averaging can only clean noise AFTER it
// aliases; band-limiting prevents the alias at the source.
//
// lodb is log2(footprint in km); each sample site subtracts its own
// log2 voxel size (tile_km / texture resolution): shape 267.6/384,
// cell tap 8/384, detail 107/256, puff 45.9/256, fray 713.6/256 km.
// (Volumes doubled to 384^3 / 256^3 in the brute-force wave - every
// offset dropped by exactly 1.0.)
const CLOUD_LODC_SHAPE: f32 = -0.521;
const CLOUD_LODC_CELL: f32 = -5.585;
const CLOUD_LODC_DETAIL: f32 = -1.259;
const CLOUD_LODC_PUFF: f32 = -2.480;
const CLOUD_LODC_FRAY: f32 = 1.479;
// Angular pixel size feeding the ray-cone width (Wave B, increment 9):
// the temporal map's Lambert texel is DERIVED from its own extent
// (4 / CLOUD_OCTA_SIZE radians; locked to the Rust size constant by
// wgsl_map_pixel_angle_matches_the_octa_size). The direct path reads the
// TRUE per-frame value 2*tan(fov/2)/viewport_rows from
// camera.light5_cone_inner.z (written by render_celestial_onto) - the old
// hardcoded 1 mrad guess under-read a 90-deg/1387-row frame by ~40%, and
// every footprint and mip pick with it. The fallback below only covers an
// older writer that never fills the pad.
// 12c: the map texel angle depends on the extent - the disc-center
// radial texel angle is sqrt(2 k) / (SIZE / 2). k = 2 (full sphere)
// reproduces the pre-12c constant 4/2048. Since the 4096-map brute
// force this is a FOOTPRINT constant deliberately pinned to the
// 2048-map angular size, NOT the stored map size: the 4096 texels
// spatially supersample the same band-limited field (quarter-cadence
// marching keeps per-frame cost flat), and marching finer footprints
// would instead multiply per-ray step counts for detail the field's
// mips cannot carry at range.
fn cloud_pix_ang_map() -> f32 {
    return sqrt(2.0 * cloud_map_k()) * (2.0 / 2048.0);
}
fn cloud_pix_ang_screen() -> f32 {
    let pa = camera.light5_cone_inner.z;
    return select(0.00144, pa, pa > 1.0e-5);
}

// STOCHASTIC mip rounding (2026-08-24, the operator's "I'm the center of
// a storm and all the clouds radiate away from me"): in the near regime
// the march footprint grows with distance FROM THE CAMERA, so the mip
// ladder crosses integer levels at fixed radii around the camera - and
// both the trilinear blend (two decorrelated fields averaged = a
// variance DIP between integers) and the carve-width table print those
// crossings as CONCENTRIC RINGS of alternating cloud texture. The cure
// is to never blend mips at all: each sample rounds to ONE integer mip,
// with the rounding threshold jittered per BLOCK/frame (set by the
// CALLING fragment entry, not the core; 0 elsewhere so the Medium
// direct path - which has no temporal accumulation to integrate the
// noise - keeps plain trilinear). BLOCK-coherent, not per-pixel,
// deliberately: per-pixel mip choices made adjacent pixels fetch
// different mip levels and the texture-cache thrash cost ~25% frame
// time across every vantage (measured 2026-08-24); 8x8 blocks sharing
// one choice keep the cache hot while boundaries dissolve spatially
// and temporally.
// Variance stays uniform (no blend dip), boundaries dissolve into noise
// the temporal accumulation integrates, and a single-mip fetch is
// cheaper than trilinear.
var<private> g_lod_jitter: f32 = 0.0;

// Mip level for one sample site: log2 footprint minus the site's log2
// voxel size, clamped to the 8-level chain (0..7).
fn cloud_lod(lodb: f32, site_c: f32) -> f32 {
    // 0..8: the 384-chain has nine levels; clamping at the old 7 would
    // saturate the deepest footprints one rung early and leave far-field
    // samples under-band-limited.
    // CONTINUOUS lod dither (2026-08-24 round 2, operator: "big squares"
    // of static while falling through the deck): integer stochastic
    // rounding put whole blocks on visibly different mip levels - the
    // 8x8 block-coherent variant traded the cache thrash of the
    // per-pixel variant for BLOCK ARTIFACTS lit up by any low sun. The
    // ring cure never needed integer mips: adding a continuous +-0.5
    // dither to the TRILINEAR lod smears the between-mip variance dip
    // across radius, so no fixed ring radius exists, while the sampled
    // value stays continuous in lod (no jumps, no squares) and the two
    // fetched mips stay the same pair as undithered (no cache cliff).
    return clamp(lodb - site_c + g_lod_jitter, 0.0, 8.0);
}

// ── Mip-width-aware SOFT carve (Wave B, increment 9) ──
// The dots forensics proved the carve is where band-limiting must happen:
// dens_n returns exactly 1.0 in every uneroded interior, so prefiltering
// the noise can never band-limit the OUTPUT - a mip-N sample pushed
// through the hard threshold still answers all-or-nothing for a footprint
// that really covers a MIX of cloud and gap. The fix follows the
// statistics: for a sample that stands for a whole footprint, the carve
// should return the footprint's EXPECTED carve - E[relu(x - thr)]/(1-thr)
// over the sub-footprint distribution, which for width sw is the smooth
// hinge 0.5*(z + sqrt(z^2 + 2/pi))*sw with z = (body-thr)/sw. As sw -> 0
// this reduces EXACTLY to the shipped hard ramp, so the near field is
// unchanged by construction.
//
// Per-level widths are FITTED, not derived: the shipped mip chain is
// variance-renormalized (cloud_noise::renormalize_level), so the residual
// sub-footprint spread at each level is an empirical property of the bake.
// The table below is fitted by clouds::carve_consistency_widths_are_fitted
// (threshold-of-mip-N vs area-average of threshold-of-mip-0 over real
// volume regions); the test FAILS if the bake drifts from these numbers.
// Re-fitted 2026-08-21 against the COMPACT hinge with a COVERAGE BOUND
// (coverage-vs-footprint increment). The unbounded mean-preserving fit
// produced widths to 0.125 and a planet-wide veil: Beer-Lambert is
// nonlinear, so mean-density-preservation at coarse mips is over-opaque
// by construction (exp(-mean_tau) vs the true clear/cloudy sub-column
// mixture - Jensen). The fit now bounds each level's areal coverage
// P(level > thr - w) to <= 1.5x the base field's true coverage, then
// fits the mean inside that bound (MAE <= 0.011, still 5x tighter than
// the old Gaussian-hinge gate). The far-field mass this leaves out is
// increment 15's statistical far field, NOT the carve's job. Level 0 is
// a single voxel - the hinge collapses to the exact hard ramp there.
// Re-fitted for the 384^3 bake (brute-force wave): nine levels; the
// fitted values are the previous chain's table shifted one rung deeper,
// exactly what a chain with one extra level ahead of it should produce.
// The 0.02 cap from the coverage adjudication still applies at the fit.
const CLOUD_CARVE_W0: f32 = 0.005;
const CLOUD_CARVE_W1: f32 = 0.005;
const CLOUD_CARVE_W2: f32 = 0.010;
const CLOUD_CARVE_W3: f32 = 0.010;
const CLOUD_CARVE_W4: f32 = 0.015;
const CLOUD_CARVE_W5: f32 = 0.015;
const CLOUD_CARVE_W6: f32 = 0.020;
const CLOUD_CARVE_W7: f32 = 0.020;
const CLOUD_CARVE_W8: f32 = 0.020;

fn cloud_carve_width(lod: f32) -> f32 {
    var w: array<f32, 9> = array<f32, 9>(
        CLOUD_CARVE_W0, CLOUD_CARVE_W1, CLOUD_CARVE_W2, CLOUD_CARVE_W3,
        CLOUD_CARVE_W4, CLOUD_CARVE_W5, CLOUD_CARVE_W6, CLOUD_CARVE_W7,
        CLOUD_CARVE_W8,
    );
    let l = clamp(lod, 0.0, 8.0);
    let i = i32(floor(l));
    let f = l - floor(l);
    let i1 = min(i + 1, 8);
    return mix(w[i], w[i1], f);
}
// Coverage carve thresholds (shader-only tuning; not mirrored -- the density
// function they live in samples textures and cannot be mirrored). The shape
// noise must clear a weather-driven threshold to become cloud: where the
// weather field is thin the threshold is CLOUD_COV_LO (almost nothing
// survives -> clear blue sky), where it peaks the threshold drops to
// CLOUD_COV_HI (dense cores). Tuned high/sparse on purpose so the deck reads
// as SCATTERED cumulus with real gaps, not a solid overcast blanket -- the
// first orbital field test (2026-07-11) rendered a near-total white sheet
// because the old `1 - weather_a` carve kept the shape almost everywhere.
const CLOUD_COV_LO: f32 = 0.854;
// The single-construction body tops out at ~p99 = 0.79 (bake statistic,
// increment 10b) - the old doubled construction saturated toward 1.0 and
// every erosion amplitude is calibrated against carve values that REACH 1
// in cores. Normalizing the carve against the real body top keeps that
// contract without retuning four erosion bands.
const CLOUD_BODY_TOP: f32 = 0.79;
// Coverage threshold for the CONSTRUCTED body. Near zero on purpose:
// the per-cell occupancy law in 41-cloud-bodies.wgsl already applies
// coverage, so this only has to keep clear air clear. Raising it
// re-introduces the ball pit (it deletes the small buds) and the
// stipple (it crushes the density rind to a stencil).
const CLOUD_V2_THR: f32 = 0.02;
const CLOUD_COV_HI: f32 = 0.347;
// Domed tops (v0.1013.x, operator field report: "the big bulky clouds still
// look like their edges are mostly cliffs" / "from above they mostly just
// look like a solid flat sheet"): the carve threshold RISES with height
// inside the regime band, so only the strongest shape noise survives near
// the band top. Each ~45 km shape cell becomes a dome - broad base, sloped
// shoulders, rounded crown at its own height - instead of a full-height
// wall under one shared flat ceiling. Quadratic in band fraction so bases
// stay broad and flat (condensation-level look) while tops taper. The
// light march shares cloud_carve, so inter-dome valleys catch real shadow
// from above. Shader-only tuning (texture-sampling path; not mirrorable).
const CLOUD_TOP_RISE: f32 = 0.45;
// Base-height field (clouds depth increment): the symmetric partner of
// CLOUD_TOP_RISE that was always missing. Without it every column's BASE
// sat on one shared iso-surface - a dead-level plane the light march could
// not shade, which is most of why the from-below deck read flat. The carve
// threshold now also rises toward the band BOTTOM, weighted by the shape
// volume's low-frequency support, so weakly supported columns get lifted,
// undulating bases while solid cores keep their low flat condensation
// deck. The light march shares cloud_carve, so base relief self-shadows.
const CLOUD_BASE_DROP: f32 = 0.35;
// (CLOUD_COV_SOFT retired in phase 3: the carve remaps against (1 - thr)
// Nubis-style so the top of the noise range always reaches density 1;
// edge softness now comes from the density normalization's skirt term.)
// Cloud-TYPE field frequency (tiles around the sphere): a very-low-freq
// noise picks stratus (0) vs cumulus (1) regions, ~2000 km weather cells.
const CLOUD_TYPE_FREQ: f32 = 3.0;
// Dual-lobe Henyey-Greenstein phase: strong forward lobe (silver linings,
// bright toward-sun rims) + mild back lobe (retro-reflection when the sun
// is behind the camera), blended by the forward weight.
// Forward g raised 0.55 -> 0.80 (phase 5 lighting, fidelity finding 5):
// cloud droplets scatter at g ~ 0.85 in the geometric-optics regime, and
// the relative-HG forward peak at g=0.55 was ~6x under-strength - the
// measured backlit rim was 1.35x the sky where a real one is 3-10x and
// clips. The multi-scatter octaves widen g per octave (see
// cloud_scatter_energy), which is what keeps deep samples from
// over-glowing at this stronger lobe. (cloud_phase feeds the aerial Mie
// term too - fog's own physical g is ~0.85, so the tighter halo there is
// a move TOWARD its own documented target.)
const CLOUD_HG_FWD: f32 = 0.80;
const CLOUD_HG_BACK: f32 = -0.15;
const CLOUD_HG_FWD_WEIGHT: f32 = 0.7;
// Two-stream diffusion floor (phase 5 lighting, fidelity finding 3):
// droplet single-scatter albedo is ~0.9999, so a thick cloud is BRIGHT -
// conservative-scattering transmittance decays ALGEBRAICALLY
// (1 / (1 + 0.75*(1-g)*tau)), never exponentially to black. This is the
// energy floor that keeps a deep overcast luminous grey instead of the
// measured ambient-only mud (direct+multiscatter was ~0 across an entire
// 0.95-coverage frame). Weight of that floor in scatter energy.
const CLOUD_MS_DIFFUSE: f32 = 0.14;
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
// Ground-bounce ambient at the slab base (clouds depth increment):
// surface-reflected sunlight lighting cloud undersides from below.
// Fraction of sun energy, faded to zero at the slab top.
const CLOUD_AMB_BOUNCE: f32 = 0.05;
// ── Wispiness + cloud-type regime constants (v0.828, Rust mirror: clouds) ──
// The "giant blotches" of the first volumetric pass came from the detail
// erosion FADING OUT with distance (CLOUD_DETAIL_FADE_*): from orbit only the
// smooth round Perlin-Worley body survived, so masses read as blobs. The fix
// is a SECOND, COARSER erosion band that never fades -- big enough (tens of
// km) to stay well above a pixel from orbit, so the marble keeps frayed,
// wispy edges. CLOUD_FRAY_TILE_KM sets the tile size: ~714 km per
// tile -> the detail volume's 8-cell Worley reads as ~88 km fray features
// (supra-pixel from 12,000 km, so no salt-and-pepper stipple).
const CLOUD_FRAY_TILE_KM: f32 = 713.6;
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
    // g_cloud_rb/rt (clouds depth increment): the Medium march sets them
    // from the material's physical slab, same as High; the defaults equal
    // the old static ratios for any caller that never sets them.
    let u = clamp((r - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
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
    let u = clamp((r - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
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
    // Medium rides the temporal map when it is armed (phase 8, the
    // quality-ladder inversion fix): only High used to get the map, so
    // Medium - the tier weak GPUs select - marched per-pixel at full
    // resolution every frame and MEASURED SLOWER than High at the same
    // vantage (84 vs 66 ms). With the flag set (params2.w +4) the
    // volumetric path's composite branch is ONE texture sample; without
    // it (orbit, or the map not armed) Medium keeps its own march.
    if (quality < 1.5 && material.params2.w < 3.5) {
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
    // No slab bounds here: the Low path paints ONE field sample at the
    // fragment, so it has no altitude bounds to set and never reads
    // g_cloud_rb/rt. (It used to write them - the dead write described in
    // the g_cloud_rb declaration comment, removed 2026-07-31.)

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
    // Physical slab bounds (clouds depth increment): Medium now marches
    // the SAME per-planet slab as High. It deliberately did not for a
    // long time (BUG-049 was Medium consuming params.w for the first
    // time), which the design doc allowed only until Medium had its own
    // probe-rig vantage - it has one now (silverdale-2km-medium).
    cloud_set_slab_bounds();

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
    let rb = g_cloud_rb;
    let rt = g_cloud_rt;
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
        let a_i = 1.0 - exp(-g_sigma_med * dens * dtm);
        // Macro lighting from the sample's own sphere normal (local frame
        // preserves dots), soft terminator as in increment 1.
        let n_i = normalize(p);
        let ndl = dot(n_i, sun_local);
        let day = smoothstep(-0.05, 0.3, ndl);
        let lit = clamp(ndl, 0.0, 1.0);
        // One-tap self-shadow: density gradient toward the sun in 3D, half
        // the slab thickness up-sun (see CLOUD_MARCH_SHADOW_SHARP note).
        let d_sun = cloud_density(
            p + sun_local * ((rt - rb) * 0.5), t, seed, coverage);
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
// Wind angular rate, rad/s, for a wind speed in m/s at this planet's
// radius (params2.z, km). Falls back to the legacy solid-body rate on a
// material with no planet radius (params2 zeroed).
fn cloud_wind_omega(mps: f32) -> f32 {
    if (material.params2.z > 0.5) {
        return mps / (material.params2.z * 1000.0);
    }
    return CLOUD_DRIFT_ZONAL;
}

fn cloud_weather(dir: vec3<f32>, t: f32, seed: f32) -> f32 {
    // Legacy drift for the Medium/Low paths, which have no regime in
    // scope; the High path passes the family's own base wind through
    // cloud_weather_adv (phase 7 motion).
    return cloud_weather_adv(dir, t, seed, t * CLOUD_DRIFT_ZONAL, 0.0);
}

fn cloud_weather_adv(dir: vec3<f32>, t: f32, seed: f32, drift_ang: f32, wlod: f32) -> f32 {
    let da0 = cloud_rot_y(dir, drift_ang);
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
    // Live weather (v0.874): sample the real MODIS cloud fraction in the
    // near-geography domain. v0.1032 wind-advection: the lookup direction
    // rotates by the accumulated zonal angle in light1_cone_inner.x (fed
    // per frame from the weather sim's wind), so cloud masses MOVE at
    // weather-dependent rates between map refreshes instead of sitting
    // pinned; a fresh map eases the angle back to zero (geography
    // re-wins - see clouds::advance_cloud_advect). Equirect UV matches
    // the planet albedo mapping above: east = -z, +Y = north.
    // albedo_sampler wraps u (antimeridian) and clamps v (poles).
    // textureSampleLevel because this runs inside the raymarch loop
    // (non-uniform control flow).
    let wdir = cloud_rot_y(dir, camera.light1_cone_inner.x);
    let w_lon = atan2(-wdir.z, wdir.x);
    let w_lat = asin(clamp(wdir.y, -1.0, 1.0));
    let w_uv = vec2<f32>(w_lon * 0.15915494 + 0.5, 0.5 - w_lat * 0.31830987);
    let w = textureSampleLevel(weather_map, albedo_sampler, w_uv, wlod).rg;
    let proc = smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, (macro_f + meso_f) / 1.04);
    // The MODIS DAILY fraction is nearly binary ("was cloudy at any point
    // today" saturates most of the globe to 100% -- rendering it 1:1 gave a
    // full whiteout). So the map is a placement MASK, not an opacity: inside
    // real cloudy zones the procedural meso/fine octaves carve the actual
    // broken deck (~instantaneous look); real clear zones (deserts) go clear.
    // G2 FRACTIONAL COVERAGE (increment 11b): the texel's value is a cloud
    // FRACTION and must RENDER as that areal fraction. The old
    // smoothstep(0.35, 0.9) envelope turned a texel saying "40% cloudy"
    // into keep/kill stipple. Now: the mip chain makes w.r the footprint's
    // true MEAN fraction; the meso octaves are the sub-texel placement
    // pattern, thresholded at the QUANTILE where P(meso > q) equals the
    // wanted fraction (cubic fitted on the real meso distribution, max err
    // 0.002 - g2_calibration in cloud_reference.rs); and the fraction is
    // pre-divided by F1 = 0.922, the measured end-to-end areal fraction at
    // full weather (erosion + lanes eat ~8%), so what SURVIVES the carve
    // matches the texel. A uniform-0.40 map renders ~0.40 areal cover.
    // MODIS DAILY-MASK CALIBRATION: the live layer is quasi-binary ("was
    // cloudy at any point today" saturates most of the globe), NOT a true
    // instantaneous fraction - rendering w.r 1:1 walks straight back into
    // the documented whiteout. A saturated texel maps to the ~55% areal
    // coverage the old envelope+meso law effectively rendered in cloudy
    // zones; partial texels respond LINEARLY below that (the fractional
    // honesty G2 wants), and the F1 = 0.922 divisor compensates what the
    // erosion + lanes eat after the carve.
    // 0.45 (was 0.55): first live-play verdict was 'super heavy' - one
    // notch lighter in saturated MODIS zones; partial texels stay linear.
    let cl = clamp(w.r * 0.45 / 0.922, 0.0, 1.0);
    let q_thr = 0.28542 + cl * (-0.30834 + cl * (0.39518 + cl * (-0.26396)));
    let live = smoothstep(q_thr - 0.015, q_thr + 0.015, meso_f);
    // Placement blend (params2.w): [0,1] = fraction of the live MODIS
    // placement to bypass toward the procedural field - the in-game
    // weather raises it so a Cloudy/Rain sky shows clouds even where the
    // real map is clear, and the dev cloud_cover pin sets 1 (full bypass;
    // w = 2+tc adds the cloud_type pin, see cloud_type_coord). Below
    // coverage ~1 a live-zeroed field cannot be resurrected by the
    // coverage knob alone, which is why the blend must move WITH the
    // coverage floor.
    let pin = cloud_pin_base();
    var bypass = clamp(pin, 0.0, 1.0);
    if (pin >= 1.5) {
        bypass = 1.0;
    }
    let live_w = w.g * (1.0 - bypass);
    return mix(proc, live, live_w);
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
    opacity: f32,    // shading heuristic weight (crown floor; NOT density)
    cover_bias: f32, // added to coverage (stratus fills to overcast)
    fray: f32,       // coarse edge-fray strength (frayed vs smooth)
    fine: f32,       // fine cauliflower strength (close-up billow)
    stretch: f32,    // domain anisotropy (cirrus streaks east-west)
    filament: f32,   // ridged-filament streaking (cirrus)
    tint: f32,       // luminance factor (overcast reads greyer)
    // PHYSICAL extinction per km at density 1 (clouds phase 3): the old
    // dimensionless opacity multiplied an already-discounted density and
    // left effective extinction ~1000x below physical - bases rendered
    // BRIGHTER than the sky instead of darker. Real values: cumulus
    // 20-100/km, stratus 15-30, cirrus 0.5-2.
    ext_km: f32,
    // How much the base-undulation threshold (CLOUD_BASE_DROP) applies:
    // undulating bases are a stratocumulus/nimbostratus (mamma) cue; a
    // cumulus base is the flat lifting-condensation level and must NOT
    // be lifted per column.
    base_drop: f32,
    // Per-family wind, m/s, at the band bottom (wind_lo) and top
    // (wind_hi) - phase 7 motion. The single CLOUD_DRIFT_ZONAL rotation
    // was 127 m/s at the equator for EVERY family at EVERY altitude,
    // 10-40x too fast for a low deck; real winds run stratus 3-7 m/s up
    // to cirrus 28-70, with shear between the band bottom and top. The
    // carve mixes these by the sample's own band height, so a towering
    // cloud's top genuinely outruns its base.
    wind_lo: f32,
    wind_hi: f32,
};

// The carved cloud body plus the values the fray/detail passes reuse.
struct CloudSample {
    carve: f32,      // coverage-carved, height-shaped body in [0,1] (pre-fray)
    ps: vec3<f32>,   // the drifted + stretched sample position (tap domain)
    h: f32,          // slab fraction at the sample
    crown: f32,      // 0 deep in the column .. 1 at the column's own crown
    // 12f underside relief (fidelity consult 2026-08-23):
    lwp: f32,        // column water-path multiplier ~[0.45, 1.55] - cloud
                     // FRACTION 1.0 never meant water path uniform; this is
                     // the low-frequency LWP field real decks mottle by
                     // (marine BL inhomogeneity nu = 2.5-3)
    v2: f32,         // 0..1 how much of this sample is the CONSTRUCTED body
    pouch: f32,      // 0..1 how low this column's own base hangs (the
                     // from-below twin of `crown`: mamma/pouch shading)
};
// Side-channel for the march (12f): the view sample's band top + pouch,
// written by cloud_carve, copied by the march IMMEDIATELY after its
// density call (cloud_sun_tau's own carve calls overwrite them later in
// the same invocation - copy first).
var<private> g_cloud_bandtop: f32 = 1.0;
var<private> g_cloud_pouch: f32 = 0.0;

// The type coordinate at a planet-fixed direction: two low-frequency octaves
// so regime patches are organic (not a few giant zones). In [0,1].
// The pin channel with the phase-4 temporal flag (+4) stripped: params2.w
// encodes [0,1] = live-MODIS bypass fraction, 1 = dev coverage pin,
// 2 + tc = coverage AND type pin, and +4.0 on top of any of those means
// "the temporal octa map is active" (a flag the pin decodes below must
// ignore).
fn cloud_pin_base() -> f32 {
    var w = material.params2.w;
    if (w >= 3.5) {
        w = w - 4.0;
    }
    return w;
}

fn cloud_type_coord(dir: vec3<f32>, t: f32, seed: f32) -> f32 {
    // Dev type pin (params2.w = 2 + tc, showcase cloud_type override):
    // a cloud-verification vantage needs a KNOWN family - the natural
    // type field can deal the capture site a faint cirrus/stratocu hand
    // and the underside gates would measure the family, not the shader.
    let pin = cloud_pin_base();
    if (pin >= 1.5) {
        return clamp(pin - 2.0, 0.0, 1.0);
    }
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
    // Height bands re-authored for the PHYSICAL slab (clouds depth
    // increment; Earth 0.4-12 km, fraction = (alt_km - 0.4) / 11.6):
    // cirrus 6-12 km, altocumulus 2-7, cumulus 0.5-6 (towering extends),
    // cumulonimbus 0.5-12, stratus 0.4-2, nimbostratus 0.4-5,
    // stratocumulus 0.5-2.5. The old fractions were tuned for the deleted
    // 25.5-76.5 km slab, where "stratus" floated at 10 km.
    //                 cirrus altocu cumulus cumulonimb stratus nimbostr stratocu
    var t_h_lo    = array<f32, 7>(0.48, 0.14, 0.01, 0.01, 0.00, 0.00, 0.01);
    var t_h_hi    = array<f32, 7>(1.00, 0.57, 0.48, 1.00, 0.14, 0.40, 0.18);
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
    // Physical extinction per km (clouds phase 3; see CloudRegime.ext_km).
    // Conservative ends of the real ranges - the density field, erosion
    // and skirt shaping still discount the medium.
    var t_ext     = array<f32, 7>(1.2, 8.0, 45.0, 60.0, 22.0, 30.0, 20.0);
    // Base-undulation weight (see CloudRegime.base_drop): full for the
    // broken low decks, near zero for flat-based convective families.
    var t_bdrop   = array<f32, 7>(0.0, 0.50, 0.10, 0.20, 0.30, 0.80, 1.00);
    // Per-family band-bottom / band-top winds, m/s (see CloudRegime
    // wind_lo/wind_hi): cirrus rides the jet, low decks amble.
    var t_wlo     = array<f32, 7>(28.0, 11.0, 5.0, 8.0, 3.0, 8.0, 6.0);
    var t_whi     = array<f32, 7>(60.0, 22.0, 10.0, 20.0, 7.0, 16.0, 11.0);
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
    var ext = 0.0;
    var bdrop = 0.0;
    var wlo = 0.0;
    var whi = 0.0;
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
        ext = ext + wi * t_ext[i];
        bdrop = bdrop + wi * t_bdrop[i];
        wlo = wlo + wi * t_wlo[i];
        whi = whi + wi * t_whi[i];
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
        ext * inv,
        bdrop * inv,
        wlo * inv,
        whi * inv,
    );
}

// Height envelope over the slab fraction h for a regime's [h_lo, h_hi] band:
// HARD rise off the base, plateau, smooth fall to the top. Mirrored + tested.
// Phase 3: the lower knee moved 30% -> 3% of the band. A cloud base is the
// lifting condensation level - a thermodynamic surface where density goes
// zero-to-full within tens of metres - and the old 30% knee smeared the
// bottom of a tower-extended cumulus band over 3.4 km, which is most of why
// the from-below deck floated at 4-8 km as a veil (fidelity finding 2).
fn cloud_height_band(h: f32, h_lo: f32, h_hi: f32) -> f32 {
    let a = mix(h_lo, h_hi, 0.03);
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
fn cloud_carve(
    p: vec3<f32>,
    t: f32,
    seed: f32,
    wa: f32,
    reg: CloudRegime,
    cell_amt: f32,
    lodb: f32,
) -> CloudSample {
    let r = length(p);
    let h = clamp((r - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
    // Early-out against the MAXIMALLY tower-extended band (the widest this
    // regime can occupy): the true tower amount needs the shape texture,
    // which we must not pay for in clear sky.
    let h_hi_max = min(reg.h_hi + 0.8 * (reg.h_hi - reg.h_lo), 1.0);
    if (cloud_height_band(h, reg.h_lo, h_hi_max) <= 0.002 || wa <= 0.003) {
        return CloudSample(0.0, p, h, 0.0, 1.0, 0.0, 0.0);
    }
    // Drift at the sample's own family wind for its band height (phase 7
    // motion): a stratus deck ambles at 3-7 m/s while cirrus rides the
    // jet at 28-60 - and because the rate mixes across the band, a
    // tower's top genuinely outruns its base (wind-shear skew). Replaces
    // the single solid-body CLOUD_DRIFT_ZONAL, which was 127 m/s at the
    // equator for every family at every altitude.
    let omega_c = cloud_wind_omega(mix(reg.wind_lo, reg.wind_hi, h));
    let ps0 = cloud_rot_y(p, t * omega_c);
    let ps = cloud_stretch_domain(ps0, normalize(p), reg.stretch);
    let s = textureSampleLevel(
        cloud_shape_tex, cloud_tile_sampler, ps * g_shape_freq,
        cloud_lod(lodb, CLOUD_LODC_SHAPE));
    let lofi = s.g * 0.625 + s.b * 0.25 + s.a * 0.125;
    // SINGLE construction (increment 10b): the bake's R channel IS the
    // finished Perlin-Worley body (cloud_noise::shape_voxel, polarity
    // corrected there). The remap that used to sit here was a SECOND
    // application of the construction on the already-built body - a
    // historic double-boost the old look had absorbed; after the bake's
    // polarity fix the two dilations fought each other and shredded the
    // field into dust (carve-map probe, 2026-08-21). The coverage window
    // below is re-derived against the single-construction distribution.
    var body = s.r;
    // How much of this sample is the CONSTRUCTED (v2) body rather than
    // the noise field. The constructed body is a DISTANCE FIELD, so the
    // downstream terms designed for a fractal are neutralised in
    // proportion to this weight (see each use below).
    var v2_w = 0.0;
    // ── CLOUDS V2 (Ultra tier, material.params.y >= 2.5) ── the body
    // CONSTRUCTED primitives instead of the noise field. This is the
    // Nubis wiring the survey found universal: noise ERODES a body, it
    // never IS the body. The erosion bands below are unchanged and now
    // bite into this constructed rind, which is the job they are good
    // at. See 41-cloud-bodies.wgsl for why round Worley cells could
    // never make a cumulus.
    // Footprint gate (2026-08-24, operator: Ultra "came to an absolute
    // crawl being unable to fly to the Earth" at 4 FPS): the constructed
    // bodies cost hundreds of hash evals per sample (the increment-14
    // costing) and are SUB-PIXEL at orbit footprints anyway - the coarse
    // mips both paid the full price and rendered nearly nothing (the
    // built bodies thin out at coarse lod). v2 bodies now exist only
    // where the footprint can actually resolve them (~a few km); coarse
    // samples keep the noise body, so Ultra at orbit looks and costs
    // exactly like High and the close-up range keeps the built look.
    // REGIME GATE (v0.1229), and the footprint gate is NOT enough on its own.
    //
    // Operator, on the first build where Ultra actually persisted: "I can't get
    // any closer to Earth than this. Hitting below 1 fps." Measured on the rig:
    // at 2000 km, High 86.9 ms vs Ultra 195.5 ms; at 12000 km and at 400 km the
    // two tiers were identical. A cost that appears only in a BAND is a gate
    // problem, not a shader-is-heavy problem.
    //
    // The band is the FAR octa map. That map is 4096x4096 = 16.7M texels, about
    // 76x more rays than the quarter-res screen march's ~222k. Its angular
    // resolution is 8.65e-4 rad, so the footprint it reports is distance-
    // dependent: 10.4 km at 12000 km altitude (over the 4 km threshold, gate
    // blocks, tiers match) but 1.73 km at 2000 km - under the threshold, so the
    // constructed bodies switched on across all 16.7M texels. At that altitude
    // the entire planet is 1526 px wide, so a 1.7 km cloud is far below one
    // pixel: the map paid the full price of building cauliflower and then
    // resolved none of it.
    //
    // The footprint alone can never express this, because the map and the screen
    // report footprints on completely different ray budgets. The regime has to
    // be asked directly.
    if (material.params.y >= 2.5 && g_v2_allowed && lodb < 2.0) {
        let tc_v2 = cloud_type_coord(normalize(p), t, seed);
        // THIN-GENUS BLEND (increment 6, the promised-but-missing half):
        // grape clusters cannot be wisps, so cirrus/altocumulus keep the
        // noise body and the built body fades in across the boundary of
        // the convective range. Replaces the unconditional swap that
        // rendered thin high cloud as low grape clusters.
        let w_built = smoothstep(0.20, 0.30, tc_v2);
        v2_w = w_built;
        let built = cloud_v2_body(p, wa, tc_v2, lodb);
        body = mix(body, built, w_built);
        if (body <= 0.001) {
            return CloudSample(0.0, ps, h, 0.0, 1.0, 0.0, 0.0);
        }
    }
    // Towering (v0.880), re-keyed in phase 3: v0.880 drove the tower from
    // COVERAGE, so pinning/raising coverage extended EVERY column to the
    // slab top - the cumulus band became 0.7-12 km and its 30%-of-band
    // lower knee floated the deck to 4-8 km (fidelity finding 2). The
    // tower now keys on the column's own low-frequency convective support
    // (lofi): strong cells tower, the rest of the field keeps the flat
    // family band, at every coverage. The light march shares this
    // function, so tower shadows stay consistent.
    let tower = smoothstep(0.62, 0.92, lofi);
    let h_hi_eff = min(reg.h_hi + tower * 0.8 * (reg.h_hi - reg.h_lo), 1.0);
    let env = cloud_height_band(h, reg.h_lo, h_hi_eff);
    if (env <= 0.002) {
        return CloudSample(0.0, ps, h, 0.0, 1.0, 0.0, 0.0);
    }
    // Domed tops (see CLOUD_TOP_RISE): the threshold climbs quadratically
    // with the fraction of the (tower-extended) band already below this
    // sample, so weak shape only exists near the base and each cell's crown
    // peaks where its own noise is strongest - rounded domes, not walls.
    let u_band = clamp(
        (h - reg.h_lo) / max(h_hi_eff - reg.h_lo, 1.0e-4), 0.0, 1.0);
    // NOTE (12f round 5, tried and REVERTED): driving thr toward 0 at
    // coverage 1.0 to close the last areal holes filled the band
    // VERTICALLY to the slab floor - the 422 m camera ended up inside
    // near-black scud. Coverage is an AREAL contract; the base must stay
    // at the condensation level. The residual few-percent thin breaks at
    // pinned 1.0 (body < COV_HI regions, sliding with the weather
    // advect) are accepted as physical; the cov100 gate caps them at 2%
    // instead of zero.
    // COVERAGE IS APPLIED ONCE (2026-08-25 - the operator's "ball pit"
    // and "faking transparency through TV static"). On the NOISE path
    // this threshold IS the coverage mechanism. On the CONSTRUCTED path
    // coverage was ALREADY applied by the per-cell occupancy law in
    // 41-cloud-bodies.wgsl (p_cell = wa * cell_area / cloud_area), so
    // thresholding again did nothing but shave every lobe inward:
    // 31-77 m of erosion depending on wa, out of a 90 m rind. Two
    // consequences the operator saw directly:
    //  - it DELETED THE BUDS. Lobe radii are Pareto-distributed from
    //    0.06*width up, so 40-57% of the 14 lobes fell below threshold,
    //    and on a small cumulus only the core lobe survived - one bald
    //    sphere. That is the ball pit, and the cauliflower that should
    //    have hidden it was exactly what got cut.
    //  - it CRUSHED the 90 m density rind to a 3-18 m skin (sub-metre
    //    for isolated low-coverage clouds - a hard stencil), leaving no
    //    gradient to be translucent with, so the erosion bands stippled
    //    the edge instead. That is the TV static.
    // Thresholded at ~0, the full rind becomes the density ramp: a real
    // ~69 m soft edge that Beer-Lambert integrates into genuine
    // transparency. The v0.1201 "thr -> 0 fills the slab vertically",
    // (scud) lesson does NOT transfer: that was the noise body, where
    // thr is the only vertical shaping. The constructed body carries
    // its own flat condensation base and height cap in the SDF.
    let thr_noise = mix(CLOUD_COV_LO, CLOUD_COV_HI, wa);
    let thr_base = mix(thr_noise, CLOUD_V2_THR, v2_w);
    // Base-height field (see CLOUD_BASE_DROP), regime-weighted in phase 3:
    // undulating bases are a stratocumulus/nimbostratus cue; a cumulus
    // base is the flat condensation level and keeps it near zero.
    let v_band = 1.0 - u_band;
    // The domed-top and base-undulation threshold terms shape the NOISE
    // body vertically; the constructed body already carries a modelled
    // crown and a flat condensation base in its SDF, so applying them
    // again only re-erodes it - and, being functions of the body value,
    // they draw further iso-distance rings on a distance field.
    let shape_w = 1.0 - v2_w;
    var thr = thr_base
        + shape_w * CLOUD_TOP_RISE * u_band * u_band
        + shape_w * CLOUD_BASE_DROP * reg.base_drop * v_band * v_band * (1.0 - lofi);
    // Cumulus-scale cell split (phase 3, fidelity finding 4): the shape
    // volume's finest feature is ~11 km, and erosion can only nibble a
    // blob's edges - nothing could ever make a 1-2 km cloud. A second tap
    // of the SAME shape volume at a ~8 km tile raises the coverage
    // threshold between cells, splitting big masses into discrete cumuli.
    // Distance-faded like the puff band so orbit never pays or changes.
    var cell_g = 0.481;
    if (cell_amt > 0.01) {
        let c = textureSampleLevel(
            cloud_shape_tex, cloud_tile_sampler, ps * g_cell_freq,
            cloud_lod(lodb, CLOUD_LODC_CELL));
        // CENTERED at the bake's g-channel mean (increment 11): the split
        // is always on now (its distance fade is deleted), so it must
        // modulate coverage locally WITHOUT shifting the global mean -
        // (mean - c.g) raises the threshold in the gaps between cells and
        // lowers it slightly at the cores, zero-mean by construction.
        // 0.481 = the baked g-channel mean (bake_stats probe).
        thr = thr + CLOUD_CELL_SPLIT * cell_amt * reg.fine * (0.481 - c.g);
        cell_g = c.g;
    }
    // Nubis-form carve (phase 3, fidelity finding 1): the old fixed
    // 0.28-wide onset window meant body -> 1 mapped to carve 1 only in a
    // ~1% tail, so "cores" sat at 0.28-0.37 density before erosion even
    // started - one of three stacked discounts that left effective
    // extinction ~1000x below physical. Remapping against (1 - thr) puts
    // the top of the noise range at carve 1.0 ALWAYS, like Nubis's
    // remap(shape, 1-coverage, 1, 0, 1).
    // SOFT hinge instead of the hard ramp (Wave B, increment 9): the same
    // (body - thr)/(1 - thr) law, but the relu is the expected relu over
    // the sample's footprint (width from the mip actually sampled). At
    // mip 0 the width is ~0 and this IS the old hard ramp; deep mips
    // return partial coverage instead of all-or-nothing, which is what
    // stops silhouettes reshaping as the mip blend moves with distance.
    let sw = cloud_carve_width(cloud_lod(lodb, CLOUD_LODC_SHAPE));
    let zc = (body - thr) / sw;
    // COMPACT-SUPPORT hinge (coverage-vs-footprint increment): the old
    // Gaussian-tail hinge 0.5*(z + sqrt(z^2 + 2/pi)) never returns zero,
    // and its ~1/|z| tail times an 11 km slab path integrated into a
    // planet-wide translucent veil at coarse mips (the 114 km white-out;
    // pin ladder proved the width table was the whole term - hard-carving
    // the mipped field matched the lod-0 truth). This hinge is E[relu]
    // over a UNIFORM sub-footprint spread of half-width sw: exactly zero
    // when the whole footprint sits below threshold, exactly the hard
    // ramp when it sits above, quadratic blend between - so a clear-sky
    // footprint is CLEAR at every mip.
    var hinge: f32;
    if (zc <= -1.0) {
        hinge = 0.0;
    } else if (zc < 1.0) {
        let u = zc + 1.0;
        hinge = 0.25 * u * u;
    } else {
        hinge = zc;
    }
    // Normalized against the REAL body top (CLOUD_BODY_TOP, the bake's
    // p99), not 1.0: the single-construction body never reaches 1, so a
    // (1 - thr) denominator capped cores at ~0.68 and typical carves at
    // 0.2-0.4 - and the four erosion bands, all calibrated against
    // carve-1 cores, ground those to ZERO. That was the under-deck
    // vanish (2026-08-23 stage forensics: pre-erosion carve max 0.23,
    // post-erosion 0.000 on every sky ray at pinned coverage 1.0; the
    // constant's own comment documents this contract but the code had
    // lost it).
    let carve = clamp(
        hinge * sw / max(CLOUD_BODY_TOP - thr, 1.0e-3), 0.0, 1.0) * env;
    // Crown proximity: the rise threshold means this column's own top sits
    // at u_crown = sqrt((body - thr_base) / CLOUD_TOP_RISE) band fractions
    // up; how close this sample is to that crown drives the valley-shade /
    // crown-light term in the march (real decks: mounds catch the sky,
    // the folds between them sit in their own shade - visible even with
    // the sun at zenith, where the tau march alone reads flat).
    let u_crown = sqrt(max(body - thr_base, 0.0) / CLOUD_TOP_RISE);
    // THE EYEBALL (2026-08-25): crown is a pure function of `body`, and
    // on the constructed path `body` IS distance-to-surface - so
    // crown_shade (mix(0.62, 1.12, crown) in the march) painted a 1.81x
    // bright arc on a contour concentric with every lobe. Measured
    // arc/interior on the operator capture: 1.90x, predicted to 5%. It
    // is a valid relief cue for the FRACTAL body, where `body` is not a
    // radial coordinate; on a distance field it is meaningless, so it
    // fades out with the constructed weight.
    let crown = mix(
        clamp(u_band / clamp(u_crown, 1.0e-3, 1.0), 0.0, 1.0), 1.0, v2_w);
    // ── 12f underside relief (fidelity consult 2026-08-23) ──
    // LWP field: cloud fraction 1.0 never meant water path uniform. Real
    // marine boundary-layer decks carry optical-thickness inhomogeneity
    // nu = mean/std of 2.5-3 (ISCCP), i.e. tau p5-p95 of ~10-33 at mean
    // 20 - which the shader's own two-stream floor turns into 2.1-2.5x
    // of base luminance. Two zero-cost sources already sampled here give
    // the field its real spatial scales: lofi (mesoscale 30-130 km) and
    // the cell tap (1-2 km Sc cells). MULTIPLIES density, never the
    // threshold - with the 0.45 floor the thinnest column still reaches
    // tau ~15 (alpha 1 - exp(-15)), so coverage stays unbroken by
    // arithmetic and the vanish class cannot return through this door.
    let lwp_f = clamp(
        (lofi - 0.30) * 2.0 + (cell_g - 0.481) * 1.6 + 0.35, 0.0, 1.0);
    let lwp = mix(0.45, 1.62, lwp_f);
    // Pouch: the from-below twin of `crown`. The base-drop term lifts
    // weakly supported columns' bases; solving body = thr_base +
    // BASE_DROP*wt*v^2 for v gives how far DOWN this column's own base
    // reaches (v_base >= 1: hangs at the very slab floor = a pouch).
    let bd_wt = CLOUD_BASE_DROP * reg.base_drop * (1.0 - lofi);
    // Same iso-distance defect as crown above: pouch is f(body), which
    // on the constructed path is a radial coordinate. Faded out with the
    // constructed weight (0 = no pouch darkening).
    let pouch = mix(clamp(
        sqrt(max(body - thr_base, 0.0) / max(bd_wt, 1.0e-3)), 0.0, 1.0), 0.0, v2_w);
    g_cloud_bandtop = h_hi_eff;
    g_cloud_pouch = pouch;
    return CloudSample(carve, ps, h, crown, lwp, pouch, v2_w);
}

// The increment-3 VIEW density: the carved body, then TWO erosion bands and a
// filament streaking pass, then the density-power thin-edge shaping. `weather_a`
// is the caller's coverage value (regime bias already folded in). `detail_amt`
// (0..1) fades ONLY the fine cauliflower band with camera distance -- the
// coarse fray band is always on, which is what gives the ORBITAL marble its
// wispy frayed edges (the fix for the "giant blotches": before, all erosion
// faded with distance and orbit saw only smooth round blobs).
// Returns (density, puff cavity 0..1, crown proximity 0..1). The cavity
// channel is the puff noise that carved this neighborhood - the march turns
// it into crevice occlusion so lobes shade individually (v0.1011). The
// crown channel is how close the sample sits to its own column's domed top
// (v0.1014) - the march turns it into valley shade / crown light.
fn cloud_density_hi(
    p: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
    detail_amt: f32,
    puff_amt: f32,
    cell_amt: f32,
    lodb: f32,
) -> vec3<f32> {
    let cs = cloud_carve(p, t, seed, weather_a, reg, cell_amt, lodb);
    var base = cs.carve;
    if (base <= 0.003) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    // COARSE fray (always on -> orbit wispiness): erode edges with the detail
    // volume's Worley FBM sampled at a LOW world frequency (~88 km features,
    // supra-pixel from orbit so no stipple), in the same stretched domain so
    // it streaks. Erode HARDER where the body is thin (the 1-base weight):
    // frayed filaments at the edges, solid cores -- erode-edges-keep-cores.
    let fr = textureSampleLevel(
        cloud_detail_tex, cloud_tile_sampler, cs.ps * g_fray_freq,
        cloud_lod(lodb, CLOUD_LODC_FRAY));
    let frfbm = fr.r * 0.625 + fr.g * 0.25 + fr.b * 0.125;
    let erode_c = frfbm * reg.fray * CLOUD_FRAY_ERODE * (0.35 + 0.65 * (1.0 - base));
    base = clamp(cloud_remap(base, erode_c, 1.0, 0.0, 1.0), 0.0, 1.0);
    // FILAMENT streaking: the ridged-Perlin channel (detail alpha) frays flat
    // sheets into thin branching streaks. Weighted by the regime (cirrus high,
    // cumulus ~none) so only the high thin clouds get mares'-tail structure.
    let fmask = smoothstep(CLOUD_FIL_LO, CLOUD_FIL_HI, fr.a);
    base = base * mix(1.0, fmask, reg.filament);
    if (base <= 0.003) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    // Both near-camera erosion bands sample the drifted-but-UNSTRETCHED
    // domain (v0.1013.x, completing the v0.1012 puff fix): cs.ps carries the
    // regime's east-west stretch (up to 3.4x), which at erosion frequencies
    // turns round cavities into long knife slashes with hard straight edges
    // - the operator's "straight hard lines ... worse the closer I get to
    // the cloud layer" (the fine band fades in within ~190 km, exactly the
    // reported onset). Cauliflower turbulence is isotropic; only the coarse
    // FRAY band and the filament mask keep the stretch (mares'-tail streaks
    // at 88 km scale are the intended look).
    let pu0 = cloud_rot_y(
        p, t * cloud_wind_omega(mix(reg.wind_lo, reg.wind_hi, cs.h)));
    // FINE cauliflower (near only): high-frequency Worley erosion, phase
    // flipping with height (wispy bases, billowy tops). Fades out with
    // distance so orbit stays smooth -- the standard Nubis distance trick.
    if (detail_amt > 0.01) {
        let d = textureSampleLevel(
            cloud_detail_tex, cloud_tile_sampler, pu0 * g_detail_freq,
            cloud_lod(lodb, CLOUD_LODC_DETAIL));
        let dfbm = d.r * 0.625 + d.g * 0.25 + d.b * 0.125;
        // Crown-weighted (v0.1014): erosion bites up to ~1.5x deeper near
        // the column's own domed top, so crowns break into individual
        // 3-13 km turrets (real cumulus castellation) instead of staying
        // one smooth slab surface; bases keep the calmer carve.
        // EDGE-weighted since the clouds depth increment: the fine band
        // was the ONLY erosion pass with no core protection - it removed
        // up to 0.54 of density FLAT, which on the physical thin slab
        // annihilated the entire from-below deck (the fray and puff bands
        // both carry a (1-base) edge weight for exactly this reason).
        // Cores now keep most of their mass; edges still shred.
        let dmod = mix(dfbm, 1.0 - dfbm, clamp(cs.h * 3.0, 0.0, 1.0))
            * CLOUD_DETAIL_ERODE * reg.fine * detail_amt
            * (0.60 + 0.90 * cs.crown)
            * (0.35 + 0.65 * (1.0 - base));
        base = clamp(cloud_remap(base, dmod, 1.0, 0.0, 1.0), 0.0, 1.0);
    }
    // PUFF band (v0.1011): the cauliflower-lobe scale the ladder was
    // missing (~0.5-1.8 km cavities). Edge-weighted like the coarse fray
    // (1-base) so cores stay solid while mass surfaces break into lobes;
    // same wispy-base / billowy-top height phase as the fine band. Only
    // near the camera (puff_amt fades by ~290 km).
    var cavity = 0.0;
    if (puff_amt > 0.01 && base > 0.003) {
        // Unstretched domain (v0.1012.x fix; pu0 hoisted above since the
        // fine band now shares it).
        let pu = textureSampleLevel(
            cloud_detail_tex, cloud_tile_sampler, pu0 * g_puff_freq,
            cloud_lod(lodb, CLOUD_LODC_PUFF));
        let pufbm = pu.r * 0.625 + pu.g * 0.25 + pu.b * 0.125;
        let phased = mix(pufbm, 1.0 - pufbm, clamp(cs.h * 3.0, 0.0, 1.0));
        let pmod = phased
            * CLOUD_PUFF_ERODE * reg.fine * puff_amt
            * (0.30 + 0.70 * (1.0 - base));
        base = clamp(cloud_remap(base, pmod, 1.0, 0.0, 1.0), 0.0, 1.0);
        // Cavity field for crevice occlusion: the same phased noise,
        // regime- and distance-weighted, independent of the edge weight
        // so even solid cores shade lobe-by-lobe.
        cavity = clamp(phased * reg.fine, 0.0, 1.0) * puff_amt;
    }
    // Thin-edge shaping: pow > 1 makes low densities translucent (see-through
    // Silhouette/density separation (phase 3, fidelity finding 1): erosion
    // decides WHERE cloud is; it must not also decide how much water the
    // surviving interior holds. Renormalizing the eroded field against its
    // own pre-erosion support puts every interior sample at density 1 (a
    // real cloud interior IS at full density) while eroded cavities and
    // skirts keep their falloff; the pow then shapes only the thin edge.
    // The skirt term re-feathers the outer silhouette over the carve
    // onset so mass borders still dissolve instead of stenciling.
    let dens_n = clamp(base / max(cs.carve, 1.0e-3), 0.0, 1.0);
    let skirt = smoothstep(0.0, 0.12, cs.carve);
    // 12f: the LWP field scales the WATER CONTENT of the surviving
    // interior (dens_n deliberately divides the carve's magnitude out -
    // erosion shape only - so without this the column depth was set by
    // geometric thickness alone: measured 1.3x spread, flat ceiling).
    // SOLIDITY-GATED (round 4): the tau-floor safety argument only holds
    // for solid interiors; marginal skirt columns are already thin, and
    // multiplying them by 0.45 pushed whole patches below visibility -
    // real sky holes at pinned coverage 1.0. Thin columns keep their
    // density; solid cores get the full mottle.
    let lwp_eff = mix(1.0, cs.lwp, smoothstep(0.10, 0.35, cs.carve));
    // ── THE CONSTRUCTED PATH TAKES ITS DENSITY STRAIGHT (2026-08-25) ──
    // The operator, on the fourth round of this: "I am waiting for the ball
    // pit look to go away... I do not know what to say or how to articulate
    // making this look remotely real."
    //
    // Every term in the line above is a function of `body` - dens_n (the
    // erosion ratio, which has an interior maximum), skirt (a smoothstep
    // over the outer 12% of the carve), lwp_eff (gated on the carve), and
    // the three erosion bands feeding `base` through their (1 - base) edge
    // weights. On the FRACTAL body those are organic, because `body` is a
    // fractal. On the CONSTRUCTED body `body` is distance-to-surface, so
    // every one of them is a circle concentric with its lobe, and they
    // STACK: a bright rim, a darker interior, further rings inside. That is
    // the eyeball, and patching them one at a time was whack-a-mole.
    //
    // So the constructed path stops using the chain at all. Its density IS
    // the displaced signed-distance ramp - the fractal detail now lives in
    // the SURFACE (the FBM displacement in 41-cloud-bodies.wgsl), which is
    // where a real cumulus keeps it, and all remaining brightness variation
    // has to come from the sun march, i.e. from geometry. That is the
    // principled split the analysis asked for: noise DISPLACES the surface
    // on the built path, and noise ERODES the density on the noise path.
    let dens_fractal = pow(dens_n, CLOUD_DENSITY_POW) * skirt * lwp_eff;
    let dens_built = cs.carve * cs.lwp;
    return vec3<f32>(
        mix(dens_fractal, dens_built, cs.v2), cavity, cs.crown);
}

// The LIGHT-march density: carved body only (no fray/detail taps -- edges err
// slightly thick, which reads as soft shadow and halves the texture cost).
// Phase 3: the interior is FULL density (physical extinction does the rest);
// only the outer skirt feathers.
fn cloud_density_light(
    p: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
    lodb: f32,
) -> f32 {
    let cs = cloud_carve(p, t, seed, weather_a, reg, 0.0, lodb);
    // 12f: same solidity-gated LWP scaling as the view density - shadows
    // must read the same water the eye does.
    let lwp_eff = mix(1.0, cs.lwp, smoothstep(0.10, 0.35, cs.carve));
    return smoothstep(0.0, 0.12, cs.carve) * lwp_eff;
}

// Optical depth toward the sun from a sample point: CLOUD_HI_LIGHT_SAMPLES
// taps with geometrically widening spacing (dense near the point for
// self-shadow detail, sparse toward the slab exit for the big-mass shadow).
//
// Clouds depth increment: the FIRST TWO taps sample the fully eroded
// density (fine + puff bands, same fade amounts as the view sample)
// instead of the smooth carved body. This is the change that makes lobes
// read as lobes: before it, fray/detail/puff carved the silhouette but
// cast ZERO self-shadow (from-above captures had 6-15x the structural
// detail energy of from-below on the same build - all shipped structure
// was top-surface). Two taps cover ~0.9-2.5 km, exactly the lobe scale;
// the remaining taps keep the cheap body-only density for the big-mass
// shadow, where erosion detail is sub-shadow anyway.
fn cloud_sun_tau(
    p: vec3<f32>,
    sun_local: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
    detail_amt: f32,
    puff_amt: f32,
    cell_amt: f32,
    lodb: f32,
) -> f32 {
    // Physical extinction (phase 3): per-family sigma in drawn units.
    // The old view/shadow sigma split (CLOUD_LIGHT_SIGMA_MULT) existed
    // because the view sigma was artificially low for alpha feathering;
    // with a physical medium both paths use the same extinction.
    let sigma = reg.ext_km / g_cloud_upkm;
    var tau = 0.0;
    var dist = 0.0;
    var step_d = g_light_near;
    for (var i = 0; i < CLOUD_HI_LIGHT_SAMPLES; i = i + 1) {
        // Geometric ladder: the segment IS the step, positions run
        // ~0.9 / 2.5 / 5.5 / 11 / 21 / 38 / 69 / 125 km (see
        // CLOUD_LIGHT_NEAR_KM / RATIO above).
        dist = dist + step_d;
        let seg = step_d;
        step_d = step_d * CLOUD_LIGHT_RATIO;
        let lp = p + sun_local * dist;
        // Band-limit each tap by ITS OWN step length too (phase 5): the
        // far taps stride tens of km and should integrate the mean field
        // at that scale, not point-sample full-frequency noise. Never
        // finer than the view sample's footprint.
        let lod_t = max(lodb, log2(max(seg / g_cloud_upkm, 1.0e-4)));
        // ALL taps on the REAL eroded density (increment 10, the dots'
        // deepest root): the old body-only far taps returned ~1 across the
        // whole carved envelope - a MASK, not a density - which at
        // physical extinction (45/km) reported tau in the HUNDREDS where
        // the converged reference reads 1-10. Bimodal tau (0 in gaps,
        // absurd in bodies) WAS the 18.9x per-texel energy coin flip. The
        // CPU twin measured the fix: -90% -> -1% ladder error at 12 taps.
        let dens = cloud_density_hi(
            lp, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt,
            lod_t).x;
        tau = tau + sigma * dens * seg;
        // v0.911 (perf audit #3): once the sun path is this optically deep
        // every scatter octave is effectively zero - later taps cannot
        // change the pixel. Cap raised again with physical extinction
        // (exp(-40 * 0.20) is 3e-4 on the slowest octave).
        if (tau > 40.0) {
            break;
        }
    }
    return tau;
}

// Sun in-scatter energy at optical depth tau (rewritten, phase 5
// lighting - fidelity finding 3). The old form was three Beer octaves
// with FIXED decay rates; at a low sun through a dense deck every
// octave collapsed below 1e-3 and the medium went BLACK with thickness,
// while a real cloud (droplet albedo ~0.9999) goes WHITE. Two changes:
//
// - The true Wrenninge/Schneider octave ladder: each octave HALVES the
//   extinction (a^n), HALVES the energy (c^n), and WIDENS the phase
//   toward isotropic by halving g (b^n) - evaluating the dual-lobe HG
//   per octave instead of scaling one precomputed phase value. The
//   widening is what lets the strong g=0.8 forward lobe coexist with
//   deep samples that must not over-glow.
// - A two-stream diffusion floor: conservative-scattering transmittance
//   through a plane-parallel cloud decays ALGEBRAICALLY,
//   1/(1 + 0.75*(1-g)*tau) (similarity theory, J. Atmos. Sci. 72(11)) -
//   the physical reason an overcast is luminous grey from below, never
//   black. Isotropic by the time it has diffused, so no phase factor.
// 12f tau split (fidelity consult finding 3): the direct octaves need
// the SUN path, but the diffusion floor is plane-parallel transmittance
// governed by the VERTICAL column above the sample - driving it with the
// slant sun path double-counted obliquity (~1.5x too dark at mid
// elevations) and sampled relief up to 2.2 km sideways from where the
// eye reads it. tau_diff = the vertical column estimate; the mild
// Eddington linear-in-cosine term restores the CIE-overcast solar
// gradient (1.15-1.30x across the sky - real overcast DOES show where
// the sun is; the octave ladder alone measured 1.01x).
fn cloud_scatter_energy(tau: f32, cos_vs: f32, tau_diff: f32) -> f32 {
    var e = 0.0;
    var c_n = 1.0;
    var a_n = 1.0;
    var g_n = 1.0;
    for (var n = 0; n < 4; n = n + 1) {
        let ph = mix(
            cloud_hg(cos_vs, CLOUD_HG_BACK * g_n),
            cloud_hg(cos_vs, CLOUD_HG_FWD * g_n),
            CLOUD_HG_FWD_WEIGHT,
        );
        e = e + c_n * ph * exp(-tau * a_n);
        c_n = c_n * 0.5;
        a_n = a_n * 0.5;
        g_n = g_n * 0.5;
    }
    let t_diff = 1.0 / (1.0 + 0.75 * (1.0 - CLOUD_HG_FWD) * tau_diff);
    return e + CLOUD_MS_DIFFUSE * t_diff * (1.0 + 0.13 * cos_vs);
}

// Direction<->uv mapping for the temporal cloud map (phase 4): a LAMBERT
// AZIMUTHAL EQUAL-AREA projection centred on the LOCAL UP at the camera.
// The history buffer is indexed by direction, so camera rotation needs no
// reprojection matrix at all, and translation against km-distant clouds
// moves a direction by well under a texel per frame - the EMA absorbs it.
// Lambert (and not octahedral, which shipped for one probe and smeared a
// wide diagonal band across the sky): the azimuthal map is CONTINUOUS
// over the whole sphere except the single antipodal point - straight
// DOWN, where no cloud is ever seen from under a deck - so bilinear
// sampling never crosses a fold. The basis derives from the planet
// centre, identically in the accumulate and composite paths; it turns
// slowly as the camera travels and the EMA absorbs that too.
// PLANET-FIXED map basis (Wave D slice 1, increment 12): the basis centre
// used to be the LIVE camera direction, so camera travel rotated the whole
// map's texel->direction mapping every frame - the history was looked up
// at UVs that meant different directions each frame, and the sky visibly
// SWAM between the map's converged state and the fresh marches (the
// operator's "clouds shift left and right between state 1 and state 2",
// the out-of-bounds-repeat feel). Now: the camera's direction is expressed
// in the PLANET's local frame (the shell's model basis - the vegetation/v2
// anchoring convention), SNAPPED to a ~0.03 rad grid, and only then taken
// back to world space. Within a snap cell (~190 km of ground travel) the
// basis is rigidly planet-locked - it turns with the planet's spin and
// ignores the camera entirely; crossing a cell boundary is a single
// discrete re-anchor the EMA absorbs once, instead of a continuous swim.
// The anchor arrives from the CPU with HYSTERESIS (Wave D fix 2): the
// first cut snapped the camera direction to a 0.03 rad grid STATELESSLY
// in-shader, and a camera hovering near a cell boundary flip-flopped the
// whole basis frame to frame - the operator's "weird left/right flicking
// that gets worse the longer we stay in the view". lib.rs now owns the
// anchor and re-anchors only past a drift threshold; it rides pads
// 496 (light2_cone_inner.x) + 556 (light5_cone_inner.w) as an octahedral
// pair. LOCKSTEP with the encode in renderer/mod.rs.
// Decode one octahedrally-encoded planet-local axis (the CPU encode in
// renderer/mod.rs) and take it to world space. Used for the CURRENT
// anchor (pads 496/556) and, on a resample frame, the OLD anchor
// (camera.light3.xy - the legacy point-light slot repurposed as 12c pads).
fn cloud_map_axis_world(ox: f32, oz: f32) -> vec3<f32> {
    let ay = 1.0 - abs(ox) - abs(oz);
    var a = vec3<f32>(ox, ay, oz);
    if (ay < 0.0) {
        a = vec3<f32>(
            (1.0 - abs(oz)) * sign(ox),
            ay,
            (1.0 - abs(ox)) * sign(oz),
        );
    }
    let a_l = normalize(a);
    return normalize((obj_normal_matrix() * vec4<f32>(a_l, 0.0)).xyz);
}

fn cloud_map_up(center: vec3<f32>) -> vec3<f32> {
    return cloud_map_axis_world(camera.light2_cone_inner.x, camera.light5_cone_inner.w);
}

// 12c extent: k = 1 - cos(theta_max) of the frozen map params, from the
// light3_cone_inner.x pad (offset 512). k = 2 (cmax = -1) is the full
// sphere - the pre-12c map exactly. Smaller k concentrates every texel
// inside theta_max of the anchor: from orbit the whole map covers just
// the planet disc (~23x the old areal resolution at 12000 km), and above
// the deck the anchor is NADIR, so the old k = 2 antipode singularity
// (the operator's "sharp warped point at my feet") is gone everywhere
// ABOVE the deck top. KNOWN LIMIT: inside the slab (regime 2, ~0.4-12 km
// on Earth) the map is still the full k = 2 sphere with the antipode at
// nadir - a camera inside the deck looking straight down still crosses
// the rim's distortion. Owned by slice B / a dual-disc mapping if it
// proves visible in play. The pad is written every celestial frame
// by the same block that writes the anchor pads; the clamp is only a
// safety net against a garbage read.
fn cloud_map_k() -> f32 {
    return clamp(1.0 - camera.light3_cone_inner.x, 1.0e-3, 2.0);
}

fn cloud_map_tangents(up: vec3<f32>) -> mat3x3<f32> {
    // Reference axis from the PLANET frame (its spin axis in world space),
    // not world Y: the tangent pair must turn with the planet exactly as
    // the up vector does, or the basis twists as the planet rotates.
    let axis = normalize(obj_normal_matrix()[1].xyz);
    var t1 = cross(up, axis);
    if (dot(t1, t1) < 1.0e-6) {
        t1 = cross(up, normalize(obj_normal_matrix()[0].xyz));
    }
    t1 = normalize(t1);
    let t2 = cross(up, t1);
    return mat3x3<f32>(t1, up, t2);
}

// Extent-Lambert encode against EXPLICIT params (12c): xy = uv, z = the
// RAW r^2 before clamping - z > 1 means the direction lies outside this
// mapping's extent (no map data there). The resample path in the octa
// pass calls this with the OLD params; the composite pass carries its
// own LOCKSTEP copy (cloud_composite.wgsl map_encode).
fn cloud_map_encode_at(d: vec3<f32>, up: vec3<f32>, k: f32) -> vec3<f32> {
    let b = cloud_map_tangents(up);
    // Local frame: y = altitude toward the anchor, xz = the azimuth plane.
    let l = vec3<f32>(dot(d, b[0]), dot(d, b[1]), dot(d, b[2]));
    let r2 = (1.0 - l.y) / max(k, 1.0e-6);
    let xz_len = max(length(l.xz), 1.0e-6);
    let p = (l.xz / xz_len) * sqrt(clamp(r2, 0.0, 1.0));
    return vec3<f32>(p * 0.5 + vec2<f32>(0.5), r2);
}

fn cloud_map_decode(uv: vec2<f32>, center: vec3<f32>) -> vec3<f32> {
    let b = cloud_map_tangents(cloud_map_up(center));
    let p = uv * 2.0 - vec2<f32>(1.0);
    let r2 = clamp(dot(p, p), 0.0, 1.0);
    let k = cloud_map_k();
    // Inverse of the extent encode: l.y = 1 - k r^2; |l.xz| follows from
    // unit length, and p already has magnitude r, so the xz factor is
    // sqrt(k (2 - k r^2)). k = 2 reduces to the classic Lambert decode.
    let y = 1.0 - k * r2;
    let s = sqrt(max(k * (2.0 - k * r2), 0.0));
    let l = vec3<f32>(p.x * s, y, p.y * s);
    return normalize(b[0] * l.x + b[1] * l.y + b[2] * l.z);
}

// Increment-3 raymarch (High quality): precomputed tiling 3D noise +
// weather map + per-sample light march. Same spherical-slab geometry, ray
// setup, probe gate, and compositing posture as the increment-2 march; the
// interior is the standard photoreal recipe -- exponential view sampling,
// Beer-Lambert light march with Beer-powder, dual-lobe HG phase, height-
// proportional ambient.
//
// Phase 4 split: the wrapper below owns the shell-fragment concerns
// (discard rule, limb fade, the temporal-composite branch); the MARCH
// lives in cloud_march_core so the temporal octa pass can drive it from a
// direction instead of a fragment.
fn cloud_layer_volumetric(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);

    // Exactly ONE shell layer (same rule as every other cloud path).
    let ro_w = (camera.view_pos.xyz - center) / shell_r;
    let cam_inside = dot(ro_w, ro_w) < 1.0;
    if (front_facing == cam_inside) {
        discard;
    }
    // Dynamic slab bounds BEFORE the slab intersection below and before any
    // cloud_carve / height read. This is the ONLY writer of g_cloud_rb/rt:
    // the assignment used to sit in cloud_layer_flat, where nothing could
    // ever observe it, so this path marched one whole slab thickness too
    // high (76-128 km instead of 25.5-76.5 km) below ~400 km altitude.
    cloud_set_slab_bounds();

    let rd_w = normalize(world_position - camera.view_pos.xyz);
    // Limb fade: ease the deck off where the view grazes the sphere so
    // ORBIT never stacks the shell into a hard white ring. Gated to the
    // outside camera (fidelity audit finding 10): from UNDER the deck a
    // grazing ray IS the horizon deck, and the real thing THICKENS
    // toward the horizon (path length grows as 1/sin(elevation)) - the
    // march supplies that for free; the fade was thinning exactly the
    // band it should have left alone.
    let n_frag = normalize(world_position - center);
    let mu = clamp(abs(dot(rd_w, n_frag)), 0.0, 1.0);
    // Wave A (increment 5): the fade eases in CONTINUOUSLY with camera
    // altitude instead of flipping on the cam_inside boolean - that flip
    // was a whole-sky change at the shell radius (~16 km), ladder-red.
    // At the shell the fade is off (the horizon deck thickens, as
    // physical); by 1.35 shell radii (~2200 km altitude) it is the full
    // orbital ring guard. Between them it blends.
    let limb_w = smoothstep(1.0, 1.35, length(ro_w));
    let limb = mix(1.0, mix(0.55, 1.0, smoothstep(0.0, 0.35, mu)), limb_w);

    // TEMPORAL COMPOSITE (phase 4, pin flag +4 in params2.w): the octa
    // pass has already marched and accumulated this direction - sample
    // the map instead of marching again. This is where the boiling
    // static dies: the map is an exponential average of many jittered
    // marches, i.e. the supersampling the single-frame march never had.
    if (material.params2.w >= 3.5) {
        // Wave D slice 1b: while the temporal map is armed, the FULLSCREEN
        // depth-aware composite pass (cloud_composite.wgsl) is the ONLY
        // compositor - it occludes per pixel against the real scene depth,
        // which is what lets a deck BELOW the camera survive (this shell
        // fragment path could not: downward rays' fragments lie beyond the
        // planet and the hardware depth test killed them - the vanishing
        // deck). This fragment's only remaining job when armed is to get
        // out of the way.
        discard;
    }

    let inv_model = transpose(obj_normal_matrix());
    let dirf = normalize((inv_model * vec4<f32>(world_position, 1.0)).xyz);
    // Stratified per-ray jitter, FROZEN on the direct path (phase 4): an
    // animated jitter with no history accumulation reads as boiling TV
    // static. The octa pass animates its own jitter as the accumulation
    // sequence, where it belongs.
    let jitter = fract(
        hash21(dirf.xy * 49152.0 + vec2<f32>(dirf.z * 12288.0, 17.0)),
    );
    // Screen march: constructed bodies are welcome here (see g_v2_allowed).
    g_v2_allowed = true;
    let s = cloud_march_core(rd_w, center, shell_r, jitter, cloud_pix_ang_screen());
    return vec4<f32>(s.rgb, s.a * limb);
}

// The marched slab integral: everything from the ray/slab intersection
// through lighting, aerial perspective, and ACES, WITHOUT the
// fragment-specific discard/limb concerns. Callable from the inline
// fragment path and the temporal octa pass.
fn cloud_march_core(
    rd_w: vec3<f32>,
    center: vec3<f32>,
    shell_r: f32,
    jitter: f32,
    pix_ang: f32,
) -> vec4<f32> {
    g_march_first_t = 0.0;
    // (g_lod_jitter is set by the CALLING fragment entry, block-coherent
    // - see the note at its declaration. The Medium direct path never
    // sets it and keeps plain trilinear.)
    let inv_model = transpose(obj_normal_matrix());
    let ro = (inv_model * vec4<f32>(camera.view_pos.xyz, 1.0)).xyz;
    let rd = normalize((inv_model * vec4<f32>(rd_w, 0.0)).xyz);

    let t = camera.sun_color.w;
    let seed = material.params.x;
    let coverage = material.base_color.a;

    // Slab interval along the ray (identical geometry to the Medium march).
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= g_cloud_rt * g_cloud_rt) {
        return vec4<f32>(0.0);
    }
    let thc_t = sqrt(g_cloud_rt * g_cloud_rt - d2);
    var m0 = max(tca - thc_t, 0.0);
    var m1 = tca + thc_t;
    if (m1 <= 0.0) {
        return vec4<f32>(0.0);
    }
    if (d2 < g_cloud_rb * g_cloud_rb) {
        let thc_b = sqrt(g_cloud_rb * g_cloud_rb - d2);
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
    // Ground occlusion (phase 5, perf finding): a camera UNDER the slab
    // looking down used to march the ANTIPODAL slab through the planet -
    // the inner-void branch above sets m0 to the inner sphere's EXIT,
    // which for a downward ray is beyond the far side of the Earth. ~61%
    // of the octa map's rays paid a full march for cloud that ground
    // always hides. If the ray strikes the planet surface (radius 1.0 in
    // planet units = inv_drawn in shell units) before the slab segment
    // starts, nothing marched past it can be seen.
    let r_surf = material.params.w;
    if (r_surf > 0.001 && d2 < r_surf * r_surf) {
        let t_surf = tca - sqrt(r_surf * r_surf - d2);
        if (t_surf > 0.0 && t_surf < m0) {
            return vec4<f32>(0.0);
        }
    }

    // Cloud regime for this ray (sampled mid-segment; type cells are ~2000 km,
    // so per-sample evaluation would buy nothing). Computed BEFORE the gate so
    // its coverage bias -- which lets a stratus air mass fill to overcast even
    // where the raw weather is thin -- is included in the clear-sky test.
    let seg = m1 - m0;
    let mid_dir = normalize(ro + rd * (m0 + seg * 0.5));
    let reg = cloud_regime(cloud_type_coord(mid_dir, t, seed));
    // Freeze the v2 body's rind for this ray (see g_v2_foot_m): the ray's
    // own footprint at the segment midpoint, in metres. Every density
    // call in this invocation - view samples AND all eight sun-shadow
    // taps - now sees ONE body scale.
    g_v2_foot_m = (m0 + seg * 0.5) * pix_ang / max(g_cloud_upkm, 1.0e-9) * 1000.0;
    // Placement moves at the family's BASE wind (phase 7 motion, the
    // v0.1021 coherence rule: silhouettes must not slide through
    // interiors - the carve's own drift mixes up from this same value).
    let wind_ang = t * cloud_wind_omega(reg.wind_lo);

    // Clear-sky gate: 3 weather probes (regime coverage bias folded in) before
    // paying for the march.
    let probe = max(
        max(
            clamp(cloud_alpha_from_field(
                cloud_weather_adv(normalize(ro + rd * m0), t, seed, wind_ang, 0.0),
                coverage) + reg.cover_bias, 0.0, 1.0),
            clamp(cloud_alpha_from_field(
                cloud_weather_adv(mid_dir, t, seed, wind_ang, 0.0), coverage)
                + reg.cover_bias, 0.0, 1.0),
        ),
        clamp(cloud_alpha_from_field(
            cloud_weather_adv(normalize(ro + rd * m1), t, seed, wind_ang, 0.0),
            coverage) + reg.cover_bias, 0.0, 1.0),
    );
    if (probe <= 0.002) {
        return vec4<f32>(0.0);
    }

    let sun = normalize(camera.sun_direction.xyz);
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let sun_energy = camera.sun_color.rgb * camera.sun_direction.w;

    // Powder gate is per-RAY (cos view-sun is constant along it). The
    // phase itself is evaluated per octave inside cloud_scatter_energy
    // since the phase-5 lighting rework (g widens per octave).
    let cos_vs = dot(rd_w, sun);
    // Beer-powder shows on the sun-facing side of masses, i.e. when the
    // sun is roughly BEHIND the camera; looking toward the sun the forward
    // lobe (silver lining) must win, so the powder eases off there.
    let powder_gate = smoothstep(0.3, 0.9, cos_vs);

    // (Jitter is the caller's: frozen hash on the direct fragment path,
    // the animated accumulation sequence on the temporal octa pass.)

    // ONE SAMPLING-RATE LAW (Wave B, increment 9): fixed-length steps whose
    // length follows the FOOTPRINT - near the camera the step resolves the
    // slab's vertical band (slab_h * CLOUD_STEP_BAND_FRAC), far out it
    // grows with the ray cone (tm * pix_ang * CLOUD_STEP_CONE_K), so every
    // sample integrates the field at the scale a pixel can actually see.
    // Replaces the exp-spaced n_samp_f heuristic, which carried three
    // defects the council catalogued: the 0.34 clamp KNEE (sample density
    // jumped discontinuously with segment length), the integer RUNG
    // (i32(n_samp_f) quantized budgets frame to frame as the segment
    // drifted), and the 2.5%-SHORT march (u^1.6 spacing never quite
    // reached u = 1, leaving the segment tail unsampled). The iteration
    // cap only guards degenerate rays; the step law itself terminates
    // grazing paths in ~CLOUD_HI_SAMPLES steps.
    let slab_h = g_cloud_rt - g_cloud_rb;
    // STEP FLOOR, SEGMENT-RELATIVE (2026-08-25, the operator: "balls would
    // just disappear in game never to return despite being incredibly dense
    // looking clouds").
    //
    // This floor was a flat fraction of the SLAB (11.6 km x 0.045 = 522 m).
    // A fair-weather cumulus is 100-500 m across, so its whole segment fitted
    // inside ONE step: the march either happened to sample it or stepped
    // clean over it, and which of those occurred changed as the camera moved.
    // That is a cloud winking out of existence, and it is the same
    // undersampling that stipples the skirts (the ramp is ~69 m wide and the
    // step was 522 m).
    //
    // The floor now scales with the SEGMENT this ray actually has to cross,
    // targeting ~16 samples through it, clamped to the old value above and a
    // 30 m physical floor below. Long segments (deep slab crossings, grazing
    // limb rays) keep their old step and their old cost; only the short
    // segments - small isolated clouds and silhouette skirts, exactly where
    // the operator sees the problem - get refined.
    let step_near = min(slab_h * CLOUD_STEP_BAND_FRAC,
        max(seg * (1.0 / 16.0), 30.0 * g_cloud_upkm * 0.001));
    // Per-ray physical extinction (phase 3): per-family sigma converted to
    // drawn units. Replaces the global CLOUD_HI_SIGMA_KM.
    let sigma_v = reg.ext_km / g_cloud_upkm;
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    var acc_w = 0.0;
    // Transmittance-weighted mean marched distance (phase 3, fidelity
    // finding 5): feeds the engine's own aerial perspective after the
    // loop, so a far cumulus hazes like the terrain beside it.
    var acc_d = 0.0;
    // First-hit distance (phase 5 lighting, fidelity finding 11): the
    // VISIBLE cloud surface, where aerial perspective belongs - the mean
    // marched distance sits inside the mass, behind what the eye sees,
    // so haze was over-applied relative to the surface.
    var first_t = -1.0;
    var t_cur = m0;
    // Previous sample's density - drives the interior MFP refinement
    // (exponential-tracking style: the step commits before this sample's
    // density is known, so it follows the last one; the jitter
    // decorrelates the one-step lag at boundaries).
    var dens_prev = 0.0;
    for (var i = 0; i < CLOUD_STEP_ITER_CAP; i = i + 1) {
        if (t_cur >= m1) {
            break;
        }
        // Footprint-proportional step with a VERTICAL ceiling (see
        // CLOUD_STEP_VERT_FRAC), an interior MFP ceiling (increment 10),
        // and a segment-density floor, clamped to what remains of the
        // segment so the march reaches m1 exactly (no unsampled tail).
        let p_cur = ro + rd * t_cur;
        let r_rate = abs(dot(normalize(p_cur), rd));
        let dt_vert = max(
            step_near,
            slab_h * CLOUD_STEP_VERT_FRAC / max(r_rate, 0.05),
        );
        let dt_seg = max(step_near, seg * CLOUD_STEP_SEG_FRAC);
        var dt = min(
            min(
                max(step_near, t_cur * pix_ang * CLOUD_STEP_CONE_K),
                min(dt_vert, dt_seg),
            ),
            m1 - t_cur,
        );
        if (dens_prev > CLOUD_STEP_INTERIOR_GATE) {
            let dt_mfp = CLOUD_STEP_TAU_MAX / (sigma_v * dens_prev);
            dt = min(dt, max(dt_mfp, slab_h * 0.002));
        }
        // The jitter places the sample inside its own step - same
        // decorrelation role it had in the exp-spaced form.
        let tm = t_cur + dt * jitter;
        t_cur = t_cur + dt;

        let p = ro + rd * tm;
        let dirp = normalize(p);
        // Footprint FIRST (hoisted, increment 11b) - the weather tap now
        // band-limits itself with the same footprint the volume taps use.
        // Weather-map texel = 27.8 km at mip 0.
        let foot = max(tm * pix_ang, dt * 0.25);
        let lodb = log2(max(foot / g_cloud_upkm, 1.0e-4));
        let wlod = max(log2(max(foot / g_cloud_upkm / 27.8, 1.0)), 0.0);
        let weather_a = clamp(
            cloud_alpha_from_field(
                cloud_weather_adv(dirp, t, seed, wind_ang, wlod), coverage)
                + reg.cover_bias, 0.0, 1.0);
        // DISTANCE FADES DELETED (increment 11, far-field truth): the
        // detail/puff/cell amounts used to fade out at 193/51/30 km, which
        // made the FIELD ITSELF a function of camera distance - the
        // measured consequences were the concentric texture rings sweeping
        // beneath a descending camera (mid-alt-45km vantage) and the HOLE
        // IN THE SKY (a clearing following the camera everywhere below
        // ~60 km: at 5 km nadir the near deck simply vanished - the
        // operator's "I get underneath and the entire cloud cover
        // changes", silhouette-ladder baseline IoU 0.009). Band-limiting
        // is the MIP LADDER's job now: every tap already picks the mip
        // matching its footprint (Wave B), and the soft carve keeps
        // threshold statistics honest across mips, so far samples read the
        // band's mean erosion instead of nothing at all.
        let detail_amt = 1.0;
        let puff_amt = 1.0;
        let cell_amt = 1.0;
        // (foot/lodb hoisted above the weather tap - increment 11b.)
        let dc = cloud_density_hi(
            p, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt, lodb);
        let dens = dc.x;
        // 12f: copy the view sample's side-channel NOW - the sun march
        // below re-enters cloud_carve and overwrites these globals.
        let s_pouch = g_cloud_pouch;
        let s_btop = g_cloud_bandtop;
        // COARSE-ENTRY BACKTRACK (increment 10, the +45%-dark diagnosis):
        // a law-sized step that lands in dense cloud would accumulate its
        // whole optical depth at ONE deep, dark sample - skipping the
        // bright sunlit rind that dominates what the eye sees (the
        // converged reference resolves that rind; the first cut of this
        // march read 45% darker than it). Nubis-style fix: reject the
        // coarse step, back up, and re-march the span at MFP resolution
        // (dens_prev primes the interior refinement above).
        if (dens > CLOUD_STEP_INTERIOR_GATE
            && dens_prev <= CLOUD_STEP_INTERIOR_GATE
            && sigma_v * dens * dt > CLOUD_STEP_TAU_MAX)
        {
            t_cur = t_cur - dt;
            dens_prev = dens;
            continue;
        }
        dens_prev = dens;
        if (dens <= 0.001) {
            continue;
        }
        let a_i = 1.0 - exp(-sigma_v * dens * dt);
        if (first_t < 0.0) {
            first_t = tm;
        }

        // Day/night from the sample's own sphere normal (soft terminator).
        let ndl = dot(dirp, sun_local);
        let day = smoothstep(-0.05, 0.3, ndl);

        // Light march toward the sun + Beer-powder edge darkening. The
        // first two taps see the same eroded density this view sample does
        // (clouds depth increment), so lobes self-shadow.
        let tau = cloud_sun_tau(
            p, sun_local, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt,
            lodb);
        // BEER-POWDER, capped on the CONSTRUCTED path (2026-08-25).
        // At a thin edge tau -> 0 so this returns 1 - 0.92 = 0.08: a
        // 12.5x darkening. Measured on the operator capture, that put
        // the cloud rim at 0.71x the luminance of the SKY BEHIND IT -
        // physically impossible for a conservative scatterer (droplet
        // single-scatter albedo > 0.9999). It is also double-counted:
        // cloud_scatter_energy already evaluates the dual-lobe HG phase
        // per octave AND carries a two-stream diffusion floor, which is
        // the multiple-scattering behaviour powder is an ad-hoc stand-in
        // for. The noise path keeps it (its look is calibrated around
        // it); the constructed path floors it so an edge can never go
        // darker than the sky it is seen against.
        let powder_raw = 1.0 - CLOUD_POWDER_STRENGTH * exp(-2.0 * tau);
        let powder = select(powder_raw, max(powder_raw, 0.75),
            material.params.y >= 2.5);
        let pw = mix(powder, 1.0, powder_gate);
        let h = clamp((length(p) - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
        // 12f: the VERTICAL column depth above this sample (plane-
        // parallel estimate from the local density and the column's own
        // band top) - what the diffusion floor and ambient shaper are
        // physically governed by. Driving them with the slant SUN path
        // double-counted obliquity (~1.5x too dark at mid sun) and read
        // relief kilometres sideways from where the eye sees it.
        let slab_h_d = g_cloud_rt - g_cloud_rb;
        let tau_vert = sigma_v * dens * max(s_btop - h, 0.0) * slab_h_d;
        let direct = cloud_scatter_energy(tau, cos_vs, tau_vert) * pw;

        // Ambient skylight (clouds depth increment): height across the slab
        // picks the base value (tops see the sky dome), then the VERTICAL
        // column depth attenuates it (12f - the old exp(-tau_sun * 0.12)
        // was shaped for tau 0-10 and moved 3% across a real overcast's
        // tau 33-49: numerically dead). Plus a GROUND BOUNCE term: real
        // cloud bases over land/ocean are lit from below by surface-
        // reflected sunlight - the cue that keeps undersides readable
        // instead of uniformly mud-grey.
        // TWO-TONE ambient (phase 5 lighting, fidelity finding 4): the
        // strongest photographic cloud cue is CHROMATIC - sunlit faces
        // warm, shadowed faces and bases BLUE (lit by the sky dome),
        // undersides warm-grey from ground bounce. The old scalar amb
        // multiplied the sun's own colour, so shadow and light differed
        // only in brightness. The sky term now takes its HUE from the
        // aerial sky colour (light2_cone_inner.yzw - the same
        // transmittance-tinted, weather-tinted, day-faded sky the haze
        // uses, so dusk ambient goes orange and night goes dark for
        // free); the ground bounce keeps a fixed warm hue. Magnitudes
        // are hue-normalized so overall energy matches the old scalar.
        let amb_h = mix(CLOUD_AMB_BASE, CLOUD_AMB_TOP, h)
            * (0.25 + 0.75 / (1.0 + 0.10 * tau_vert));
        let sky_aer = vec3<f32>(
            camera.light2_cone_inner.y,
            camera.light2_cone_inner.z,
            camera.light2_cone_inner.w,
        );
        let sky_peak = max(max(sky_aer.x, sky_aer.y), max(sky_aer.z, 1.0e-4));
        // 12f ground bounce: bounce is ground albedo times the DOWNWELLING
        // irradiance at the surface, which IS the cloud's own diffuse
        // transmittance - a fixed warm 0.05 was 57-63% of the base
        // radiance under a real overcast and inverted the chroma sign
        // (measured: darkest decile R/B 1.377 vs brightest 1.187 - dark
        // went WARMER; real thick cloud goes BLUER because only diffuse
        // skylight remains). Scaling by the column transmittance restores
        // both the sign and the luminance range.
        let bounce_t = clamp(1.8 / (1.0 + 0.15 * tau_vert), 0.0, 1.0);
        // Whitened sky hue: multiple scattering inside the medium
        // desaturates the ambient - full-strength sky blue made thin
        // columns read as open sky (round-2 gate4: 33k false-sky
        // pixels). 0.55 keeps the blue TENDENCY (thick = bluer than
        // warm) without cloud impersonating sky.
        let sky_hue = mix(vec3<f32>(1.0), sky_aer / sky_peak, 0.55);
        let amb_col = sky_hue * amb_h
            + vec3<f32>(0.98, 0.94, 0.88)
                * (CLOUD_AMB_BOUNCE * (1.0 - h) * bounce_t);

        // Crevice occlusion (v0.1011): the puff cavity field darkens the
        // sample - lobes shade individually even though the light march
        // is far coarser than the lobe scale. Direct takes half the
        // occlusion (crevices still catch some sun), ambient the full.
        // Crown shading (v0.1014): samples near their own column's domed
        // crown catch extra sky, samples deep in the fold between domes
        // sit in valley shade - the from-above relief cue that survives
        // even a zenith sun.
        // Regime-aware floor (v0.1021, watch item "cirrus possibly
        // grayer"): thin families' samples sit LOW in their bands (small
        // crown fraction), so a fixed 0.70 floor grayed cirrus veils that
        // should stay bright. Faint regimes get a gentler valley shade.
        let crown_floor = mix(0.88, 0.62, reg.opacity);
        let crown_shade = mix(crown_floor, 1.12, dc.z);
        // 12f pouch shading: the from-below twin of the crown term. A
        // column whose own base hangs low (pouch -> 1) has more cloud
        // directly above its base and a smaller sky-view solid angle from
        // below - real mamma sit 20-40% darker than the surrounding base.
        // Weighted toward the band bottom so tops are untouched.
        let vband = clamp(
            1.0 - (h - reg.h_lo) / max(s_btop - reg.h_lo, 1.0e-4), 0.0, 1.0);
        let pouch_shade = mix(1.0, 0.72, s_pouch * vband * vband);
        let ao = (1.0 - CLOUD_PUFF_AO * dc.y) * crown_shade * pouch_shade;
        // Direct carries the SUN's colour; ambient carries the SKY's (the
        // two-tone split above). Ambient magnitude rides the sun's
        // luminance so total energy matches the old single-hue form.
        let direct_lit = direct * mix(1.0, clamp(ao, 0.0, 1.0), 0.5);
        let sun_lum = dot(sun_energy, vec3<f32>(0.2126, 0.7152, 0.0722));

        let c_i = material.base_color.rgb
            * (sun_energy * (direct_lit * day)
                + amb_col * (sun_lum * ao * day)
                + vec3<f32>(CLOUD_NIGHT_FLOOR));
        acc = acc + c_i * (trans * a_i);
        acc_w = acc_w + trans * a_i;
        acc_d = acc_d + tm * (trans * a_i);
        trans = trans * (1.0 - a_i);
        // 0.005, not 0.02 (increment 10): with resolved density gradients
        // the last 1.5% of transmittance carries visible skirt light.
        if (trans <= 0.005) {
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

    // Aerial perspective at the cloud's OWN depth (phase 3, fidelity
    // finding 5): the engine's shared aerial integral, evaluated at the
    // transmittance-weighted mean marched distance, so a far cumulus hazes
    // exactly like the terrain beside it and near-horizon masses dissolve
    // into haze COLOUR instead of being alpha-deleted. Replaces
    // cloud_low_cam_haze on this path - that ratio hack was calibrated
    // when the deck WAS the drawn shell, and on the physical slab it
    // erased real clouds below ~5 degrees elevation.
    // Phase 5 lighting (fidelity finding 11): aerial at the FIRST-HIT
    // distance - the visible surface - not the transmittance-weighted
    // mean, which sits inside the mass and over-hazed it. And the haze's
    // own opacity RAISES the fragment alpha below, so a distant cumulus
    // fades toward the haze colour while KEEPING its silhouette instead
    // of dissolving to transparent.
    let mean_t = acc_d / max(acc_w, 1.0e-4);
    var srf_t = mean_t;
    if (first_t > 0.0) {
        srf_t = first_t;
    }
    // Export for the octa pass's history reprojection (see the private
    // declaration): the visible cloud surface distance, world units.
    g_march_first_t = srf_t * shell_r;
    let srf_world = camera.view_pos.xyz + rd_w * (srf_t * shell_r);
    radiance = aerial_apply(radiance, srf_world);
    let t_aer = aerial_transmittance(srf_world);

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

    // (Limb fade is the fragment wrapper's concern - the octa map must
    // store the un-limbed march so the composite can apply the CURRENT
    // fragment's grazing angle.)
    let a_body = body_total * CLOUD_HI_MAX_ALPHA;
    return vec4<f32>(mapped, clamp(1.0 - (1.0 - a_body) * t_aer, 0.0, 1.0));
}

