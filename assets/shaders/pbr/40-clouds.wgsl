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
// Footprint window over which constructed bodies hand back to the noise body,
// as log2(footprint in km).
//
// CURRENTLY NEUTRALISED (1.9 -> 2.0 reproduces the old hard cut at a 4 km
// footprint), and the reason is worth reading before widening it again.
//
// The intent was to fix the operator's "a ton of white dots... like snow
// flakes" seen from orbit: once a whole cloud is near or below one sample
// footprint it cannot be filtered, only twinkled, so the built body should
// hand back to the smooth noise body. That reasoning is sound and the window
// -2 to 0 (250 m to 1 km of footprint) is about right.
//
// What it exposed is that THE TWO BODY MODELS ARE NOT BRIGHTNESS-MATCHED. The
// noise body renders darker, so fading toward it with distance darkened the
// far half of every frame. Measured at cumulus-closeup-ultra: mean grey 191.1
// with the fade off, 157.4 with it on, and the same 157 with the shading term
// that was initially blamed removed entirely.
//
// So the fade cannot ship until the two bodies agree on brightness at the
// handover. That is the actual next task here - matching them, then reopening
// this window - not tuning these two numbers.
// ── THE FADE BAND WAS THE WARP (v0.1255, operator + rig bisect) ──
// The v2-to-noise representation handoff is DISTANCE-keyed (footprint
// lod), and inside its band the two bodies INTERFERE: the operator's
// crescent clouds with eaten centres ("the rosette causing a cloud to
// disappear in the center... nothing should be changing about how it
// looks but it still is"), and at the far end the noise body's
// sheet-like coverage collapsing into discrete v2 specks on approach
// ("immersion breaking to go from a white sheet to almost no cloud
// cover"). Rig-proven: leap-off left the crescents standing; fade-out
// dissolved them into solid masses at identical FPS. Pushed 1.9/2.0 ->
// 3.9/4.0 (a 4x farther handoff radius): the operator's whole flying
// band is pure constructed bodies, and the morph now happens where a
// cloud subtends ~a pixel, so the representation change is invisible
// by scale separation. NOT pushed to infinity: at true orbital
// footprints sub-footprint lobes would point-sample as speckle - the
// noise body (carve-hinge band-limited) remains the correct coarse
// representation.
const CLOUD_V2_FADE_LO: f32 = 3.9;
const CLOUD_V2_FADE_HI: f32 = 4.0;
// How dark a fully down-facing cloud surface goes, and how much the crevices
// between buds darken. Both act on the constructed path only.
const CLOUD_V2_SKY_FLOOR: f32 = 0.80;
const CLOUD_V2_SEAM_AO: f32 = 0.16;
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
//   Low    (cloud_layer_flat)       - the old cloud_weather path never reads
//          them at all. It used to hold the ONLY assignment, which was
//          unobservable: `var<private>` is per-invocation storage and
//          cloud_layer dispatches to exactly ONE path per invocation, so the
//          Low path's write could never reach the High path's read. The far
//          rung's profile branch (cloud_layer_flat_profile, v0.1290+) DOES
//          read them (slab height for the pooled bins) and calls
//          cloud_set_slab_bounds itself first.
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
// March iterations actually used by the last cloud_march_core call (dev
// instrument: the flower-nadir ring forensics render this; costs one MOV).
var<private> g_march_iters: f32 = 0.0;
// Depth of the first ACCEPTED sample below the last clear one, in metres
// (map_diag 6). The v0.1271 twin predicts a 0-600 m per-pixel hash on the
// shipped march (first accepted sample 311 +- 280 m deep at 1.5 km) and a
// flat <= 30 m field once the march is sample-anchored. It is the gate that
// cannot pass by setup: it must read RED on the old march first.
var<private> g_march_first_depth_m: f32 = 0.0;
// 1.0 when the current view sample sits behind more than one optical depth
// of cloud from the eye (bit 20 experiment: coarse sun ladder deep inside).
var<private> g_deep_sample: f32 = 0.0;
// Rosette-bisect channels (v0.1249 forensics): luminance of the DIRECT-sun
// and AMBIENT contributions of the last march, accumulated with the same
// transmittance weights as the radiance. The octa pass renders one of them
// (or first-hit t) into the map with the EMA bypassed when the map_diag
// showcase pin is set - whichever channel carries the anchor-centred petals
// is the biased term.
var<private> g_march_sun_acc: f32 = 0.0;
var<private> g_march_amb_acc: f32 = 0.0;
// Maximum march range in KM for the next cloud_march_core call (v0.1244,
// the per-pixel regime split). The NEAR screen path sets this to its
// ownership range: content entering the slab beyond it is the octa map's
// job, so the ray ABSTAINS (returns clear, first_t = 0) before stepping
// once - which is what turns the both-whales blend band into near-pays-
// only-for-near. The map path leaves it huge (unbounded).
var<private> g_march_max_km: f32 = 1.0e9;
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

// The mip the v2 surface DISPLACEMENT is sampled at, frozen once per ray
// alongside g_v2_foot_m. Displacement is SHAPE, so every evaluation that
// reaches a given point in the world must agree on it. See the taps in
// 41-cloud-bodies.wgsl for what went wrong when it did not.
var<private> g_v2_disp_lod: f32 = 0.0;

// Sun-PROFILE flag (v0.1252.2, the sandblast bisect + 3-agent audit). The
// speck-sun channel convicted the direct term (Laplacian 2.57-3.54 vs
// alpha 0.78), and the mechanism audit convicted its dominant source:
// mid-ladder sun taps multiply POINT samples of the built body's
// sub-MFP fields (interior turbulence +-42% at ~4 m content via the
// frozen g_v2_disp_lod, the fine displacement octave, Worley erosion)
// by 200-400 m segments - delta_tau 1.5-4 rms between adjacent pixels,
// 2-5x direct swings through the exp octaves. Real clouds launder
// structure below one light mean-free-path (~22 m at 45/km) via
// LATERAL multiple scattering, and every production renderer therefore
// feeds the sun march a LOWER-frequency density than the eye ray (HZD
// 2015 "cheap" light samples; Nubis3 profile-based light volume, first
// two taps per-pixel only). When this flag is 1, cv2_cloud_sdf
// substitutes each sub-MFP field's MEAN (0.5 for the FBMs - which
// zeroes the fine displacement and makes the interior factor exactly
// 1.0) - the mip-infinity limit, IDENTICAL for every tap, so the
// per-tap-lod eyeball-ring class (v0.1230) cannot return. Coarse
// relief (the lobe SDF, the 2 km displacement octave) stays: lobes
// still self-shadow. Detail erodes alpha, never deep sun transmittance.
var<private> g_sun_profile: f32 = 0.0;

// Signed distance in METRES from the last constructed-body evaluation to the
// lobe cluster surface. Large positive = nothing near. Stays at this sentinel
// on tiers that do not build bodies, which is how the march knows not to
// steer by it.
var<private> g_v2_sdf_m: f32 = 1.0e9;
// ── INCREMENT A globals (v0.1280, the in-cloud light) ──
// Sun optical depth over the first two ladder rungs only (87 m on-axis):
// the local burial cue.
var<private> g_sun_tau01: f32 = 0.0;
// Slant column optical depth handed to the ladder by the caller (A2): a
// buried sample takes this instead of rungs 2-11.
var<private> g_sun_tau_col: f32 = 0.0;
// Burial profile of the current view sample, 0 at the surface, 1 deep.
var<private> g_ms_prof: f32 = 0.0;
// Increment A on (dev pad bit 22).
var<private> g_ms_on: f32 = 0.0;
// map_diag 8: the burial profile accumulated with the same trans*a_i weights
// as the colour, so it reads as what the pixel SEES.
var<private> g_march_prof_acc: f32 = 0.0;
// Local column top (band fraction) from the crown estimator, so the column
// above a sample is the CLOUD's own top, not the regime band top 4 km up.
var<private> g_cloud_coltop: f32 = 1.0;
// Built path: the winning cloud top height and the sample height, metres.
var<private> g_v2_top_m: f32 = 0.0;
var<private> g_v2_up_m: f32 = 0.0;
// Increment C: the body's plane-parallel interior density at this sample
// (adiabatic profile without turb), for the built-path source column.
var<private> g_v2_int_dens: f32 = 0.0;

// Peak domain-warp displacement in metres for the last body evaluated. The
// warp bends the space the distance field is measured in, so it is exactly
// how far the true surface can sit from where the raw distance says.
var<private> g_v2_warp_m: f32 = 0.0;

// Vertical component of the constructed surface normal, and the smooth-union
// seam strength, from the last body evaluated. See the smin loop in
// 41-cloud-bodies.wgsl. Neutral defaults so the noise path is untouched.
var<private> g_v2_ny: f32 = 0.0;
var<private> g_v2_seam: f32 = 0.0;
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
// Fixed WORLD level of detail for shape-defining fields, as log2 of a
// footprint in km: -7.0 is about 8 m of world detail. Every evaluation that
// reaches a world point agrees on it regardless of camera distance, which is
// the invariant the g_v2_disp_lod comment already demands.
const CLOUD_V2_SHAPE_LOD_WORLD: f32 = -7.0;
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
// Lateral cone fraction of the sun ladder (v0.1252.2, sandblast bisect,
// the HZD-2015 light cone). Each pixel's sun ladder is an independent
// 1D transect, so even perfectly band-limited taps carry a rung-
// invariant floor of delta_tau = ext_km * pixel_pitch per rung at full
// per-pixel contrast - structure real clouds launder via lateral
// multiple scattering at the ~22 m MFP scale. Offsetting each far tap
// inside a cone (radius = K * distance along the light ray, golden-
// angle spiral, phase advanced per frame by the lod jitter) turns the
// line integral into an area integral the resolve's accumulation
// converges - transmittance arriving at a point HAS diffused laterally,
// so the area average is the physically right object, not a blur.
const CLOUD_SUN_CONE_K: f32 = 0.12;
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
// Height-varying warp amplitude, km (design 2c, dev pad bit 12).
const CLOUD_HV_WARP_KM: f32 = 0.5;
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
// REFITTED v0.1259 by carve_consistency_widths_are_fitted after the bake
// moved to per-mip HISTOGRAM MATCHING. The widths DROP because they are
// no longer compensating a distribution mismatch the renormalization
// left behind - matched distributions need no threshold correction, so
// what remains is only the genuine sub-footprint spread of the soft
// hinge. Re-run the harness and paste again after ANY bake change.
// Wide-edge experiment multiplier on the hinge width (dev pad bit 6).
const CLOUD_EDGE_WIDE_MUL: f32 = 20.0;
const CLOUD_CARVE_W0: f32 = 0.0050;
const CLOUD_CARVE_W1: f32 = 0.0050;
const CLOUD_CARVE_W2: f32 = 0.0050;
const CLOUD_CARVE_W3: f32 = 0.0050;
const CLOUD_CARVE_W4: f32 = 0.0050;
const CLOUD_CARVE_W5: f32 = 0.0050;
const CLOUD_CARVE_W6: f32 = 0.0050;
const CLOUD_CARVE_W7: f32 = 0.0050;
const CLOUD_CARVE_W8: f32 = 0.0050;

// ── SIGNED THRESHOLD OFFSET, SEPARATE FROM THE SOFTNESS (v0.1265) ──
// The carve hinge had ONE per-mip number doing two jobs: it is the
// hinge's half-width (a SOFTNESS, and a divisor, so necessarily
// positive) and it doubles as a threshold shift, since coverage is
// about P(body > thr - w). The coverage fit wants NEGATIVE shifts at
// mips 2-5 - the mip distributions sit slightly high, not low - and a
// divisor cannot go negative, so the fit was clamped at its floor and
// left 36% of the achievable correction on the table (total coverage
// error 0.643 legal vs 0.408 unconstrained).
//
// Splitting the roles frees it: W stays the positive softness, T is a
// SIGNED threshold offset. The operator's evidence for why this
// matters: the residual rosette is strongest in the SHADER (noise
// path) clouds and weakest in the VOXEL (constructed) ones - and the
// constructed body takes its coverage from an SDF with no mip
// dependence at all, while the noise body's coverage IS a thresholded
// mip. The remaining radial gradient is this drift.
// Fitted by coverage_width_fit; re-run and paste after any bake change.
const CLOUD_CARVE_T0: f32 = 0.0050;
const CLOUD_CARVE_T1: f32 = 0.0050;
const CLOUD_CARVE_T2: f32 = 0.0075;
const CLOUD_CARVE_T3: f32 = 0.0075;
const CLOUD_CARVE_T4: f32 = 0.0075;
const CLOUD_CARVE_T5: f32 = 0.0075;
const CLOUD_CARVE_T6: f32 = 0.0025;
const CLOUD_CARVE_T7: f32 = -0.0125;
const CLOUD_CARVE_T8: f32 = -0.0125;

// Dev pad bits 13-15 as an index (v0.1283): which density component the
// F10 bisect turns off. fract(w / 2^16) * 8 leaves bits 13-15 as the integer
// part (bits 0-12 sum to under 8192, so they cannot carry).
fn cloud_bisect_index() -> u32 {
    return u32(floor(fract(camera.light7_color.w * 0.0000152587890625) * 8.0));
}
// Dev pad bit 16 = the SUN-SHADOW CACHE (increment 1 of the performance
// arc, v0.1286). Bit 17 is still free (v0.1283; the carve-saturation remap
// that briefly used both was a null and was removed, see BUGS/PRIORITIES).
// fract(w / 2^17) >= 0.5 isolates bit 16: bits 0-15 sum to under 65536, so
// they cannot carry into it, and the pad is integer-valued so the test is
// exact in f32.
fn cloud_light_cache_on() -> bool {
    return fract(camera.light7_color.w * 0.00000762939453125) >= 0.5;
}

// ── THE SUN-SHADOW CACHE (increment 1, v0.1286) ──────────────────────────
//
// What it is: the sun optical depth a sample sees (`cloud_sun_tau`) used to
// be a 12-rung ladder of density evaluations PER VIEW SAMPLE, and at Ultra
// each rung rebuilds a constructed lobe cluster, so the ladder was 80-88% of
// all density work in every situation (PRIORITIES v0.1284). Sunlight at a
// point in a cloud does not depend on where the camera is, so rungs 2-11 of
// that ladder are now BAKED ONCE into a planet-fixed 3D lattice around the
// camera's ground point (two nested windows, fine and coarse) and read back
// with one trilinear tap. Each pixel keeps rungs 0 and 1 on its own axis
// (the 30 m + 57 m local self-shadow that the in-cloud light's A2 split
// already isolates as g_sun_tau01), so the entry rind still self-shadows
// exactly. Beyond both windows the per-pixel ladder runs as before.
//
// The lattice: a window is an axis-aligned box in a LOCAL planet-fixed
// frame at its anchor (a point on the planet at the camera's ground
// lat/lon): u = normalize(anchor), e = normalize(cross(Y, u)) with Y the
// planet spin axis in this planet-local space, n = cross(u, e). Lattice
// point (i, j, k) sits at
//   anchor + e * ((i + 0.5) * cell_h - half_w)
//          + n * ((j + 0.5) * cell_h - half_w)
//          + u * (k * cell_v + z0)
// with half_w = nx * cell_h / 2 and z0 = the slab base height above the
// anchor's own radius (g_cloud_rb - length(anchor)), so slice 0 sits on the
// cloud base and the slices climb through the band. Everything is in the
// march's own coordinate space (the drawn shell's object space, 1 unit =
// the drawn shell radius; g_cloud_upkm converts km into it).
//
// Rust (renderer::cloud_temporal::CloudLightCache) owns the anchors, the
// re-anchor hysteresis, the atlas texture and the bake pass; it hands the
// shader the anchor and the horizontal cell size of each window through two
// camera pads that nothing else reads:
//   light3_color = (fine_anchor.xyz, fine_cell_h)
//   light4_color = (coarse_anchor.xyz, coarse_cell_h)
// The cell counts, the vertical cell size and the atlas packing are the
// CLOUD_LC_* constants below, mirrored in cloud_temporal.rs and pinned by a
// unit test that reads this file (same discipline as cloud_reference.rs's
// wgsl_reference_constants_stay_in_sync: every constant is `const NAME: f32
// = value;` on one line so that parser finds it).
//
// The atlas: ONE R16F texture_2d, slices side by side along x, riding the
// existing group-3 binding 0 `albedo_texture` (no bind-group-layout change,
// exactly the way the octa map rode the albedo slot). Fine slices first
// (48 of 256x256), then the coarse slices (24 of 128x128):
//   fine:   x = k * 256 + i,                 y = j
//   coarse: x = CLOUD_LC_COARSE_X0 + k * 128 + i,  y = j   (rows 128..255 unused)
// The stored value is tau_far = the ladder's optical depth from rung 2
// onward, measured from the lattice point (the first tap lands 195 m
// sunward, exactly where rung 2 lands for a view sample), clamped to
// [0, CLOUD_LC_TAU_MAX].

// Fine window: 256 x 256 columns of 190 m (48.6 km square), 48 slices of
// 240 m (11.5 km tall).
const CLOUD_LC_FINE_NX: f32 = 256.0;
const CLOUD_LC_FINE_NZ: f32 = 48.0;
const CLOUD_LC_FINE_CELL_H_M: f32 = 190.0;
const CLOUD_LC_FINE_CELL_V_M: f32 = 240.0;
// Coarse window: 128 x 128 columns of 760 m (97 km square), 24 slices of
// 480 m (11.5 km tall). It only has to cover the fine window's re-anchor
// hysteresis and the mid distance; beyond it the ladder runs as before.
const CLOUD_LC_COARSE_NX: f32 = 128.0;
const CLOUD_LC_COARSE_NZ: f32 = 24.0;
const CLOUD_LC_COARSE_CELL_H_M: f32 = 760.0;
const CLOUD_LC_COARSE_CELL_V_M: f32 = 480.0;
// Atlas packing: the coarse slices start at x = 48 * 256; the whole atlas is
// 15360 x 256 texels.
const CLOUD_LC_COARSE_X0: f32 = 12288.0;
const CLOUD_LC_ATLAS_W: f32 = 15360.0;
const CLOUD_LC_ATLAS_H: f32 = 256.0;
// The stored optical depth is clamped here (f16 holds it exactly enough;
// exp(-64 * 0.125) is 3e-4 on the slowest scatter octave, i.e. black).
const CLOUD_LC_TAU_MAX: f32 = 64.0;
// The blend band at each window's edge, as a fraction of the half-width:
// fine -> coarse across the fine window's outer 20%, coarse -> the fallback
// across the coarse window's outer 20%, so no window edge prints a ring.
const CLOUD_LC_BLEND_FRAC: f32 = 0.20;
// What the coarse window blends INTO at its edge, and what runs beyond it.
// 0.0 = the per-pixel ladder (rungs 2-11 as before; the look outside the
// windows is exactly today's, and the coarse outer band pays the ladder to
// blend against). 1.0 = the analytic slant column g_sun_tau_col (cheaper,
// but a different quantity from the ladder beyond 48 km, so the far field's
// look changes). The contract text names both; the ladder is the safe first
// cut and the constant is the one-line switch to try the other.
const CLOUD_LC_FAR_ANALYTIC: f32 = 1.0; // v0.1288: the analytic column beyond the coarse window (rain 26 km 152 -> 122 ms, look identical)
// "Sun source" codes for map_diag channel 9 (the bisect instrument that
// shows where each window's edge falls): fine window 1.0, coarse window
// 0.5, "decided by rungs 0-1" 0.35 (the A2 buried early-exit or the opaque
// cap fired before any cache read; this happens INSIDE the windows too, so
// the ring gate must not read it as "outside"), the ladder / analytic
// fallback 0.15 (bit off, or beyond the coarse window).
const CLOUD_LC_SRC_FINE: f32 = 1.0;
const CLOUD_LC_SRC_COARSE: f32 = 0.5;
const CLOUD_LC_SRC_DECIDED: f32 = 0.35;
const CLOUD_LC_SRC_FALLBACK: f32 = 0.15;

// Per-sample "sun source" code (see CLOUD_LC_SRC_*), set by cloud_sun_tau,
// and its transmittance-weighted accumulation for map_diag channel 9, the
// same acc weights as the colour so it reads as what the pixel sees.
var<private> g_light_src: f32 = 0.15;
var<private> g_march_src_acc: f32 = 0.0;

// The local frame of a window: returns e (east), with n and u recoverable
// by the caller. Kept as one function so the bake (which places lattice
// points) and the read (which locates a sample in the lattice) can never
// disagree on the frame. At the poles cross(Y, u) vanishes; the X axis
// stands in there (Rust mirrors this rule; the re-anchor keeps the anchor
// off the exact pole in practice).
fn light_cache_east(u: vec3<f32>) -> vec3<f32> {
    var e = cross(vec3<f32>(0.0, 1.0, 0.0), u);
    if (dot(e, e) < 1.0e-8) {
        e = cross(vec3<f32>(1.0, 0.0, 0.0), u);
    }
    return normalize(e);
}

// The lattice point of texel (i, j) on slice k of the window anchored at
// `anchor` with horizontal cell `cell_h` (p-units) and vertical cell
// `cell_v` (p-units): the bake's inverse of light_cache_tap's coordinate
// mapping. `nx` is the window's column count (its width in cells).
fn light_cache_point(
    anchor: vec3<f32>, cell_h: f32, cell_v: f32, nx: f32,
    i: f32, j: f32, k: f32,
) -> vec3<f32> {
    let u = normalize(anchor);
    let e = light_cache_east(u);
    let n = cross(u, e);
    let half_w = nx * cell_h * 0.5;
    let z0 = g_cloud_rb - length(anchor);
    return anchor
        + e * ((i + 0.5) * cell_h - half_w)
        + n * ((j + 0.5) * cell_h - half_w)
        + u * (k * cell_v + z0);
}

// How close to a window's horizontal edge a sample sits: 0 inside the inner
// (1 - CLOUD_LC_BLEND_FRAC) of the half-width, rising to 1 at the edge and
// beyond. Chebyshev distance (max of |east|, |north|) because the window is
// a square; smoothstep so the blend is C1 at both ends of the band. The
// vertical axis is NOT part of this test: both windows span the whole slab
// and a sample above or below the lattice reads the nearest slice (see
// light_cache_tap), never a fade to the fallback, because a vertical fade
// would hand the lit cloud tops to the fallback quantity.
fn light_cache_edge_w(p: vec3<f32>, anchor: vec3<f32>, cell_h: f32, nx: f32) -> f32 {
    let u = normalize(anchor);
    let e = light_cache_east(u);
    let n = cross(u, e);
    let d = p - anchor;
    let half_w = nx * cell_h * 0.5;
    let m = max(abs(dot(d, e)), abs(dot(d, n))) / max(half_w, 1.0e-12);
    return smoothstep(1.0 - CLOUD_LC_BLEND_FRAC, 1.0, m);
}

// One manual-trilinear read of a window: two bilinear taps on the adjacent
// k slices, each clamped INSIDE its own slice (texel-centre coordinates in
// [0.5, n - 0.5], so the hardware bilinear filter can never reach into the
// neighbouring slice), then a lerp in k. The sampler is the group's own
// linear albedo_sampler; the texture is the atlas in the albedo slot.
fn light_cache_tap(
    p: vec3<f32>, anchor: vec3<f32>, cell_h: f32, cell_v: f32,
    nx: f32, nz: f32, x0: f32,
) -> f32 {
    let u = normalize(anchor);
    let e = light_cache_east(u);
    let n = cross(u, e);
    let d = p - anchor;
    let half_w = nx * cell_h * 0.5;
    let z0 = g_cloud_rb - length(anchor);
    // Continuous lattice coordinates: the inverse of light_cache_point, so
    // a sample exactly on lattice point (i, j, k) reads that texel.
    let fi = clamp((dot(d, e) + half_w) / cell_h - 0.5, 0.0, nx - 1.0);
    let fj = clamp((dot(d, n) + half_w) / cell_h - 0.5, 0.0, nx - 1.0);
    let fk = clamp((dot(d, u) - z0) / cell_v, 0.0, nz - 1.0);
    let k0 = floor(fk);
    let k1 = min(k0 + 1.0, nz - 1.0);
    let wk = fk - k0;
    let inv_atlas = vec2<f32>(1.0 / CLOUD_LC_ATLAS_W, 1.0 / CLOUD_LC_ATLAS_H);
    let uv0 = vec2<f32>(x0 + k0 * nx + fi + 0.5, fj + 0.5) * inv_atlas;
    let uv1 = vec2<f32>(x0 + k1 * nx + fi + 0.5, fj + 0.5) * inv_atlas;
    let v0 = textureSampleLevel(albedo_texture, albedo_sampler, uv0, 0.0).r;
    let v1 = textureSampleLevel(albedo_texture, albedo_sampler, uv1, 0.0).r;
    return mix(v0, v1, wk);
}

// The cache read for a view sample at p (the sample itself, NOT offset
// along the sun: the stored value already starts its ladder 87 m sunward
// of each lattice point, exactly as rung 2 does for a view sample).
// Returns (tau_far, w_far, src):
//   tau_far = the cached rungs 2-11 optical depth, fine inside the fine
//             window's inner 80%, blended to coarse across its outer 20%,
//             coarse out to the coarse window's inner 80%;
//   w_far   = how much of the FALLBACK the caller must blend in: 0 inside
//             the coarse inner 80%, rising to 1 at the coarse edge, and 1
//             (cache unusable) beyond it or when the pads are unset;
//   src     = the "Sun source" code for map_diag channel 9.
fn light_cache_tau(p: vec3<f32>) -> vec3<f32> {
    let fa = camera.light3_color.xyz;
    let f_cell_h = camera.light3_color.w;
    let ca = camera.light4_color.xyz;
    let c_cell_h = camera.light4_color.w;
    // Pads not written (cache armed in the shader but not yet planned by
    // Rust this frame): treat as outside both windows.
    if (f_cell_h <= 0.0 || c_cell_h <= 0.0) {
        return vec3<f32>(0.0, 1.0, CLOUD_LC_SRC_FALLBACK);
    }
    let wc = light_cache_edge_w(p, ca, c_cell_h, CLOUD_LC_COARSE_NX);
    if (wc >= 1.0) {
        return vec3<f32>(0.0, 1.0, CLOUD_LC_SRC_FALLBACK);
    }
    let upm = g_cloud_upkm * 0.001;
    var tau = light_cache_tap(
        p, ca, c_cell_h, CLOUD_LC_COARSE_CELL_V_M * upm,
        CLOUD_LC_COARSE_NX, CLOUD_LC_COARSE_NZ, CLOUD_LC_COARSE_X0);
    var src = CLOUD_LC_SRC_COARSE;
    let wf = light_cache_edge_w(p, fa, f_cell_h, CLOUD_LC_FINE_NX);
    if (wf < 1.0) {
        let tau_f = light_cache_tap(
            p, fa, f_cell_h, CLOUD_LC_FINE_CELL_V_M * upm,
            CLOUD_LC_FINE_NX, CLOUD_LC_FINE_NZ, 0.0);
        tau = mix(tau_f, tau, wf);
        src = mix(CLOUD_LC_SRC_FINE, CLOUD_LC_SRC_COARSE, wf);
    }
    return vec3<f32>(tau, wc, src);
}

fn cloud_carve_thr_off(lod: f32) -> f32 {
    var t: array<f32, 9> = array<f32, 9>(
        CLOUD_CARVE_T0, CLOUD_CARVE_T1, CLOUD_CARVE_T2, CLOUD_CARVE_T3,
        CLOUD_CARVE_T4, CLOUD_CARVE_T5, CLOUD_CARVE_T6, CLOUD_CARVE_T7,
        CLOUD_CARVE_T8,
    );
    let l = clamp(lod, 0.0, 8.0);
    let i = i32(floor(l));
    let f = l - floor(l);
    let i1 = min(i + 1, 8);
    return mix(t[i], t[i1], f);
}

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
// Floor on the carve normaliser (design item 2A, v0.1273, dev pad bit 10).
// max(CLOUD_BODY_TOP - thr, 1e-3) collapses to a 1e-3 STENCIL whenever
// thr + T > 0.79 (weather alpha below ~0.13): measured rise 25 m (p10 12 m),
// the only genuine cliff on the tiled-noise path. 0.05 body units is a
// ramp of about 450 m at the cell gradient. The hinge zero crossing
// P(body > thr + T - sw) is untouched, so the fitted CLOUD_CARVE_W/T
// tables and coverage_width_fit stay valid; only the slope above it is
// bounded. Look change confined to the low-coverage residual-breaks regime.
const CLOUD_CARVE_NORM_FLOOR: f32 = 0.05;
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
// Thin-deck experiment scale on the regime band height (dev pad bit 11).
// 0.3 takes the cumulus band from 5.2 km to 1.6 km, altocumulus 4.7 -> 1.4.
const CLOUD_THIN_DECK_SCALE: f32 = 0.3;
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
// Relaxed-Beer clamp weight on the direct octave sum (v0.1252.2, the
// sandblast bisect - Nubis 2017's max(exp(-tau), 0.7*exp(-0.25*tau))).
// When v0.1252 moved tau_vert to the carve envelope the sun-channel
// grain ROSE: the (accidentally noisy) diffusion floor had been
// propping up deep-shadow luminance, and with a lower floor the
// exp-octave grain became a larger share of a smaller direct term. The
// 4-octave ladder's 2nd/3rd octaves still leave dln(e)/dtau ~ -0.30 at
// tau 2-6; the clamp puts a floor with slope -0.25 under tau ~4-20, so
// a delta_tau ripple there produces at most 0.25x the contrast plain
// Beer would - the cheapest stand-in for lateral multiple scattering
// filling self-shadow crevices. Phase at quarter-g (the relaxed light
// is nearly diffused). Tune DOWN if deep-shadow faces read washed.
const CLOUD_SUN_RELAX: f32 = 0.7;
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

// ── THE LOW SHEET FROM THE PROFILE (perf increment 4, contract "The orbit
// and Low-sheet map") ──
// The global equirect map IS the 2D profile map: four pooled height bins of
// (cloud fraction fp, mean density Gp) and the pooled column Tp per texel,
// at the mip pair bracketing the fragment's own footprint. The sheet's
// opacity is the element law (A7) applied to each pooled bin in turn along
// this fragment's ray: T_q = (1 - fp_q (1 - exp(-sigma D_in L_elem)))^(seg /
// L_elem), alpha = 1 - T_0 T_1 T_2 T_3, so a 30 percent cumulus field reads
// 30 percent opaque from orbit instead of the old field's cue-ball white.
// Self-shadow comes from the column (Tp toward the sun minus Tp here)
// instead of a second field sample; the sphere-normal lighting, silver
// lining, ACES, limb fade and low-camera haze are the flat path's own.
// Called only with the knob on and the global valid; `dir` is the planet-
// local unit direction of the fragment, `inv_model` the world-to-local
// rotation, both already computed by the caller.
fn cloud_layer_flat_profile(
    world_position: vec3<f32>,
    center: vec3<f32>,
    shell_r: f32,
    dir: vec3<f32>,
    inv_model: mat4x4<f32>,
    cam_inside: bool,
    t: f32,
    seed: f32,
) -> vec4<f32> {
    // The slab, for the bin height and the element caps (the flat path
    // never set these before; the profile is expressed in slab bins).
    cloud_set_slab_bounds();
    let slab_km = (g_cloud_rt - g_cloud_rb) / max(g_cloud_upkm, 1.0e-9);
    let dz_pool_km = 3.0 * slab_km / f32(CLOUD_FR_NZ);
    // Footprint: the shell draws at SCREEN resolution, so the screen pixel
    // angle is the right one here (cloud_pix_ang_screen, pad
    // light5_cone_inner.z), times the fragment's distance from the camera.
    // shell_r is one drawn-shell unit in render units; g_cloud_upkm converts
    // drawn-shell units to km.
    let slant = length(world_position - camera.view_pos.xyz);
    let dist_km = slant / max(shell_r, 1.0e-6) / max(g_cloud_upkm, 1.0e-9);
    let foot_km = max(dist_km * cloud_pix_ang_screen(), 1.0e-4);
    let lodb_sheet = log2(foot_km);
    // The regime at the fragment's OWN direction (BUG-074 rule): its
    // extinction per km and the element sizes of its family, in km.
    let tc = cloud_type_coord(dir, t, seed);
    let reg = cloud_regime(tc);
    let sigma = reg.ext_km;
    let e_km = cloud_fr_elem_km(tc, reg, slab_km);
    // The global read (the optical-depth columns it also returns are in the
    // march's units and unused here: the sheet reads the raw pooled bins).
    g_pf_sigma_v = 0.0;
    let lat = asin(clamp(dir.y, -1.0, 1.0));
    let lon = atan2(-dir.z, dir.x);
    let g = cloud_profile_global(lon, lat, 0.5, lodb_sheet);
    if (!g.ok) {
        return vec4<f32>(0.0);
    }
    // The ray in the planet-local frame and its angle to the local vertical:
    // c_v = 1 looks straight down through the pooled bin, c_v -> 0 grazes it.
    let rd = normalize(world_position - camera.view_pos.xyz);
    let rd_local = normalize((inv_model * vec4<f32>(rd, 0.0)).xyz);
    let c_v = abs(dot(rd_local, dir));
    let seg_q = dz_pool_km / max(c_v, 0.05);
    let l_v_eff = max(e_km.y, dz_pool_km);
    // The element law per pooled bin, bins multiplied along the ray.
    var trans = 1.0;
    for (var q = 0; q < CLOUD_FR_GLOBAL_NZ; q = q + 1) {
        let fp_q = cloud_profile_chan(g.fp, q);
        let gp_q = cloud_profile_chan(g.Gp, q);
        let d_in = clamp(gp_q / max(fp_q, CLOUD_FR_F_EPS), 0.0, 1.0);
        trans = trans * cloud_fr_t_pf(1.0, fp_q, d_in, sigma, e_km.x, l_v_eff, c_v, seg_q);
    }
    let alpha = 1.0 - trans;
    if (alpha <= 0.002) {
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

    // Cheap self-shadow from the COLUMN: the pooled column Tp a short
    // great-circle step toward the sun against the column here. If the
    // column RISES toward the sun, this fragment sits on the shaded flank of
    // a cloud mass -> darken. Tp is in pooled-bin units (at most 4), hence
    // the 0.25. The sun step vanishes smoothly when the sun is overhead (the
    // tangent projection goes to zero; no normalize-of-zero NaN).
    let sun_local = normalize((inv_model * vec4<f32>(sun, 0.0)).xyz);
    let tang = sun_local - dir * dot(sun_local, dir);
    let sdir = normalize(dir + tang * CLOUD_SHADOW_STEP);
    let lat_s = asin(clamp(sdir.y, -1.0, 1.0));
    let lon_s = atan2(-sdir.z, sdir.x);
    let gs = cloud_profile_global(lon_s, lat_s, 0.5, lodb_sheet);
    let shade = 1.0
        - CLOUD_SHADOW_STRENGTH
            * clamp((gs.Cp.w - g.Cp.w) * 0.25 * CLOUD_SHADOW_SHARP, 0.0, 1.0);

    // Silver lining: HG forward lobe (the atmosphere's phase function,
    // reused) at THIN edges -- thick cores block the forward-scattered sun,
    // so weight by (1 - alpha). Gated by a twilight-wide day window so the
    // deep night limb never glows.
    let cos_vs = dot(rd, sun);
    let silver = CLOUD_SILVER_GAIN * atmo_mie_phase(cos_vs) * (1.0 - alpha)
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

    // Limb fade: near the disc edge the shell is seen almost edge-on and
    // stacks over the atmosphere's own limb brightening into a hard white
    // ring; ease the deck off as the view grazes the sphere.
    let mu = clamp(abs(dot(rd, n)), 0.0, 1.0);
    let limb = mix(0.55, 1.0, smoothstep(0.0, 0.35, mu));
    let low_haze = cloud_low_cam_haze(world_position, cam_inside, center, shell_r);
    // The element law already IS the density ramp: alpha is the real
    // transmittance loss, so dense columns approach opaque and thin cloud
    // stays translucent without the old field's re-shaping.
    return vec4<f32>(mapped, alpha * limb * low_haze * 0.97);
}

fn cloud_layer_flat(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    // Shell center + radius recovered from the object transform, same trick
    // as the atmosphere shell: unit icosphere placed via Vec3::splat(scale),
    // so column 0's length IS the shell radius and column 3 the center.
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);
    // No slab bounds here: the old cloud_weather path below paints ONE
    // field sample at the fragment, so it has no altitude bounds to set and
    // never reads g_cloud_rb/rt. (It used to write them - the dead write
    // described in the g_cloud_rb declaration comment, removed 2026-07-31.)
    // The far rung's profile branch (cloud_layer_flat_profile, gated below)
    // DOES read g_cloud_rb/rt for the slab height and calls
    // cloud_set_slab_bounds itself before its first read.

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

    // ── THE FAR RUNG'S LOW SHEET (perf increment 4) ── when the profile
    // knob is on and the global map has completed its first pass (flag bit
    // 1), the sheet is redrawn from the same planet-fixed profile the march
    // reads beyond level 5's window, so the orbit disc, the marble and the
    // Low tier all draw one map from one law. Knob 0, or before the first
    // pass completes: today's cloud_weather path below, so the sheet never
    // goes blank while the first pass bakes.
    // The last two terms are the guards the march path already has:
    // cloud_profile_tap refuses a material with no planet radius
    // (params2.z), and cloud_set_slab_bounds only sets the slab when the
    // material carries params.w (planet_r / drawn_r). Without them a shell
    // drawn from a non-planet material would reach cloud_profile_global
    // with planet_km = 0 (log2(0) in the mip pick, the legacy slab
    // constants for dist_km) and paint garbage instead of falling through.
    if (cloud_profile_knob() != CLOUD_FR_KNOB_OFF && cloud_profile_global_valid()
        && material.params2.z >= 0.5 && material.params.w > 0.001) {
        return cloud_layer_flat_profile(world_position, center, shell_r, dir, inv_model, cam_inside, t, seed);
    }

    // v0.1284: the SAME field the ground shadow and the volumetric tiers use
    // (blended live+procedural weather), not the older purely procedural
    // cloud_field - from orbit the two patterns differed and the ground own
    // darkening showed through the translucent sheet as hard-edged patches.
    let field = cloud_weather(dir, t, seed);
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
    let field_sun = cloud_weather(sdir, t, seed);
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
    // Dense cores approach opaque (a real deck from orbit hides the ground);
    // thin cloud keeps the CLOUD_MAX_ALPHA cap.
    let cap = mix(CLOUD_MAX_ALPHA, 0.97, t_core);
    return vec4<f32>(mapped, body * density * limb * low_haze * cap);
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
// The PRE-EROSION carve envelope at the last cloud_carve call (v0.1252).
// The shading site's vertical-column optical depth (tau_vert) is a
// COARSE physical quantity - a column average over the whole band above
// the sample - and estimating it from the post-erosion POINT density
// printed every fine erosion detail straight into the lighting: the
// bisect (speck-alpha 0.78 vs speck-sun 2.57 grain) proved alpha hides
// the fine structure (it saturates) while the diffusion floor, ambient
// attenuation and ground bounce - all tau_vert consumers - keep its
// full contrast per pixel. That was the operator's "sandblasted"
// stipple. The carve is the smooth interior envelope: the right
// estimator for a column quantity. v0.1252.2: this now stores the
// CELL-FREE envelope (carve_env) - the cumulus cell-split tap is 20.8 m
// voxels at ~mip 0 up close, pixel-scale grain the column estimate must
// not carry either.
var<private> g_cloud_carve: f32 = 0.0;
// ── THE FAR RUNG'S TWO PUBLISHES (perf increment 4, contract "NOISE part") ──
// g_cloud_frac = clamp((zc + 1) * 0.5, 0, 1): the compact hinge's OWN areal
// fraction, the share of the sample's footprint whose body lies above the
// coverage threshold under the uniform sub-footprint spread the hinge
// integrates (zc = -1 nothing, +1 everything). g_cloud_carve_pt = the
// PRE-EROSION carve (the value cloud_density_hi erodes from). The profile
// bake turns the pair into the noise body's per-cell cloud FRACTION f_n:
// frac = g_cloud_frac * clamp(dens / g_cloud_carve_pt, 0, 1), evaluated at
// the cell's own mip, where the hinge IS the cell mean. Zero cost: both
// values already exist inside cloud_carve. Reset at every carve entry so an
// early return (clear sky, outside the band) never leaks a stale fraction.
var<private> g_cloud_frac: f32 = 0.0;
var<private> g_cloud_carve_pt: f32 = 0.0;
// How much of this sample is the CONSTRUCTED body rather than the noise body.
// Published on the same side-channel as g_cloud_pouch because the shading site
// needs it and the CloudSample it lives on is not in scope there.
var<private> g_v2_w: f32 = 0.0;

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
    // Bit 18 (v0.1279 experiment): a SHARP base. 3% of the band is 150-330 m
    // of soft underside; the lifting condensation level is opaque within
    // tens of metres. 0.5% is 26-55 m.
    let base_frac = select(0.03, 0.005,
        fract(camera.light7_color.w * 0.0000019073486328125) >= 0.5);
    let a = mix(h_lo, h_hi, base_frac);
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
    // Far-rung publishes (increment 4): reset first, so the early returns
    // below leave "no cloud" behind, never a previous sample's fraction.
    g_cloud_frac = 0.0;
    g_cloud_carve_pt = 0.0;
    let r = length(p);
    let h = clamp((r - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
    // ── THE LEAN (v0.1275, design 1b; showcase cloud_shear, F10 slider) ──
    // The residual hunt named the rosette as correct perspective of
    // vertical-walled prisms: every coverage field has a horizontal
    // correlation length larger than the band height, so masses are
    // prisms whose walls run the full band and converge at the nadir.
    // This is the discriminating experiment and a rung of the real fix
    // (wind shear leans a column with height): the SAMPLE coordinate is
    // displaced toward local east (e_hat = up x dir) by light6_color.w
    // metres per metre of height above the base, so the drawn columns lean
    // toward local WEST with height. If the fan is the walls, its
    // convergence point moves off the nadir by about f*tan(atan(shear))
    // toward local west. Applied HERE, once, so
    // the view samples, the bisection taps, the priming tap and every
    // sun-ladder tap (which re-enters this function) lean identically.
    // 0 = off; the weather tap stays on the unleaned direction (its
    // octaves are 95-1274 km, the lean is metres).
    let lean_s = camera.light6_color.w;
    let e_hat = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), p / max(r, 1.0e-6)));
    let p_l = p + e_hat * (lean_s * max(r - g_cloud_rb, 0.0));
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
    // v0.1284: the column drifts as a whole at the BAND-MEAN wind. The
    // height-dependent rate that was here (mix(wind_lo, wind_hi, h)) made
    // base and top drift apart by (wind_hi - wind_lo) * t: about 35 m/s for
    // a cumulonimbus family, so 126 km after one hour over an 11 km band, a
    // tilt near 85 degrees that grew for the whole session and that the rig
    // at its 120 s clock pin could never see. Real shear tilts a cloud by a
    // bounded amount (the lean above); a field does not accumulate it.
    let omega_c = cloud_wind_omega(mix(reg.wind_lo, reg.wind_hi, 0.5));
    let ps0 = cloud_rot_y(p_l, t * omega_c);
    let ps = cloud_stretch_domain(ps0, normalize(p), reg.stretch);
    // ── HEIGHT-VARYING DOMAIN WARP (v0.1278, design 2c, dev pad bit 12) ──
    // The residual hunt named the rosette: every coverage field has a
    // horizontal correlation length (finest shape Worley cell 5.6 km) far
    // larger than the band height, so the shape tap below is effectively
    // 2D and every cloud is a vertical-walled prism; from inside the deck
    // the walls and the clear corridors between them converge at the
    // nadir as a flower (the operator v0.1277.1 storm capture: the
    // cumulonimbus band spans the whole slab, walls 11 km tall). Warping
    // the SHAPE coordinate with the cell tap - the same 3D texture at the
    // 8 km tile, 1.33 km Worley cells, which DOES vary with height - makes
    // a coarse-cell wall wander +-CLOUD_HV_WARP_KM per 1.3 km of height:
    // silhouettes become curves and no edge extrapolates to one point. A
    // stationary field marginal is invariant under a domain warp, so the
    // coverage window stays calibrated. Faded out as the footprint passes
    // 0.5-2 km so the far field keeps its statistics. One extra tap.
    let hv_warp = fract(camera.light7_color.w * 0.0001220703125) >= 0.5;
    var ps_s = ps;
    if (hv_warp) {
        let cw = textureSampleLevel(
            cloud_shape_tex, cloud_tile_sampler, ps * g_cell_freq,
            cloud_lod(lodb, CLOUD_LODC_CELL)).rgb;
        let hv_fade = 1.0 - smoothstep(-1.0, 1.0, lodb);
        // Amplitude from light5_color.x when set (showcase cloud_hv_km, F10
        // slider); the constant is the fallback. At the operator bm-12 camera
        // 0.5 km was a null: the sight lines through km-scale gaps need the
        // walls to wander by more than the gap width over the visible height.
        let hv_km = select(CLOUD_HV_WARP_KM, camera.light5_color.x, camera.light5_color.x > 0.0);
        let a_w = hv_km * g_cloud_upkm * hv_fade;
        ps_s = ps + (cw - vec3<f32>(0.5)) * 2.0 * a_w;
    }
    // ── INCREMENT B 2.1: THREE-OCTAVE DOMAIN WARP (v0.1281, dev pad bit 23) ──
    // Measure-preserving: the field marginal is invariant under a smooth
    // domain warp, so coverage holds without a refit. W_org bends the
    // 5.6-45 km facets into sinuous walls (a straight corridor pinches shut
    // every ~1.5 km), W_wall is the 100-500 m turbulence band, W_fine the
    // 10-100 m band the eye resolves from inside. The sun path keeps W_org
    // (it moves mass) and takes the fine octaves at their means, the same
    // discipline as the built path.
    let field_on = fract(camera.light7_color.w * 0.000000059604644775390625) >= 0.5;
    var ps_b = ps_s;
    if (field_on) {
        let w_org = textureSampleLevel(cloud_detail_tex, cloud_tile_sampler,
            ps / (24.0 * g_cloud_upkm), cloud_lod(lodb, -3.42)).rgb;
        ps_b = ps_b + (w_org - vec3<f32>(0.5)) * 2.0 * (0.8 * g_cloud_upkm);
        if (g_sun_profile < 0.5) {
            let w_wall = textureSampleLevel(cloud_detail_tex, cloud_tile_sampler,
                ps / (3.0 * g_cloud_upkm), cloud_lod(lodb, -6.42)).rgb;
            let w_fine = textureSampleLevel(cloud_detail_tex, cloud_tile_sampler,
                ps / (0.6 * g_cloud_upkm), cloud_lod(lodb, -8.74)).rgb;
            ps_b = ps_b + (w_wall - vec3<f32>(0.5)) * 2.0 * (0.12 * g_cloud_upkm)
                + (w_fine - vec3<f32>(0.5)) * 2.0 * (0.03 * g_cloud_upkm);
        }
    }
    let s = textureSampleLevel(
        cloud_shape_tex, cloud_tile_sampler, ps_b * g_shape_freq,
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
    g_v2_w = 0.0;
    // NOT reset here (v0.1276.2). The v0.1275 hygiene reset g_v2_sdf_m to
    // the no-SDF sentinel at every carve, so any sample where the body is
    // skipped - clear air, exactly where the stride matters - disabled the
    // clear-air stride and grazing rays took hundreds of base steps (the
    // nadir-anchor-40 vantage fell from 15.6 to 4-5 fps with the OLD march
    // too, so it was not the bisection). The ladder-tap leak the reset was
    // for is closed by saving and restoring g_v2_sdf_m around the ladder.
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
    // ── FOOTPRINT FADE (v0.1233) ──
    //
    // Operator, floating above the planet: "a ton of white dots appear
    // everywhere. Makes it look kind of like snow flakes."
    //
    // Constructed bodies stop paying for themselves once a whole cloud is
    // near or below ONE sample footprint. The power-law sizes made most
    // clouds a few hundred metres across, so from orbit each one lands on
    // about a pixel - and a sub-pixel bright object cannot be filtered, it
    // can only twinkle. That is the snow.
    //
    // The old gate was a hard cut at a 4 km footprint, chosen to bound COST,
    // not to bound aliasing, and 4 km is many times the size of the clouds it
    // was letting through. This fades the built body back to the smooth noise
    // body across 250 m to 1 km of footprint - which is a mip fade, the same
    // reasoning that says stop drawing detail once it is smaller than a
    // sample. Fading rather than cutting because a hard switch of body model
    // would pop.
    // Fade coordinate carries the per-pixel lod dither (v0.1242): lodb is
    // monotone in screen radius on a down-look, so the raw 0.1-lod handoff
    // band printed as a hard crosshair-centred CIRCLE where built bodies
    // swap to the noise body (one ring of the operator's melted flower,
    // proven by the flower-nadir vantage). Dithered, the handoff becomes a
    // sub-noise-floor speckle band. g_lod_jitter defaults to 0.0 on paths
    // that never set it (Medium direct), which keeps the old exact edge.
    let lodf = lodb + g_lod_jitter * 0.35;
    if (material.params.y >= 2.5 && g_v2_allowed && lodf < CLOUD_V2_FADE_HI) {
        let tc_v2 = cloud_type_coord(normalize(p), t, seed);
        // THIN-GENUS BLEND (increment 6, the promised-but-missing half):
        // grape clusters cannot be wisps, so cirrus/altocumulus keep the
        // noise body and the built body fades in across the boundary of
        // the convective range. Replaces the unconditional swap that
        // rendered thin high cloud as low grape clusters.
        let w_foot = 1.0
            - smoothstep(CLOUD_V2_FADE_LO, CLOUD_V2_FADE_HI, lodf);
        let w_built = smoothstep(0.20, 0.30, tc_v2) * w_foot;
        v2_w = w_built;
        g_v2_w = w_built;
        let built = cloud_v2_body(p_l, wa, tc_v2, lodb);
        // ── SHEET UNION (v0.1234) ──
        //
        // Operator: "the volume of cloud in the grid coordinate still doesn't
        // fill the full area so we can never have real sheets of clouds...
        // all the clouds are small and never merge with other grid sections."
        //
        // Structurally true: the constructed bodies are placed ONE PER GRID
        // CELL, so no coverage value can ever merge cells into a sheet. But
        // the continuous noise field CAN make sheets - it is why the planet
        // looks properly overcast from orbit (the far renderer uses it) and
        // then dissolves into per-cell speckle as the constructed bodies take
        // over on approach. The two models disagreed about the sky.
        //
        // The fix is what real skies do. Scattered fair-weather cumulus ARE
        // discrete objects; overcast is NOT more of them, it is a continuous
        // stratiform layer. So as coverage rises past ~0.55 the field itself
        // is unioned back in under the clusters, filling the space between
        // them until at ~0.85 the sky closes. Below that the union term is
        // zero and scattered skies are exactly as before.
        // Gated on the GLOBAL coverage, not the local weather alpha. The first
        // cut used wa, which is a PER-SAMPLE value that reaches 1.0 inside any
        // cloud at any coverage - so the union fired everywhere there was any
        // cloud at all, closed the whole sky at 0.55, and put the camera of the
        // scattered-cumulus test inside a dark solid deck. The sky-wide question
        // "is this an overcast day" belongs to the sky-wide coverage value.
        let sheet_w = smoothstep(0.60, 0.90, material.base_color.a);
        // Fade the UNION, never the sheet's own density (v0.1235). The first
        // form was max(built, body * sheet_w): scaling the field down at partial
        // coverage pushed it under the visibility threshold and punched holes -
        // the operator's "very holey swiss cheese" sheets. A sheet that exists
        // should exist at full density; what fades in is how much of it joins.
        let fused = mix(built, max(built, body), sheet_w);
        body = mix(body, fused, w_built);
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
    // THIN DECK experiment (v0.1275, dev pad bit 11): the band height scaled
    // by CLOUD_THIN_DECK_SCALE toward physical thickness. The residual hunt
    // named the rosette as perspective of vertical-walled prisms - coverage
    // fields with horizontal correlation (5.6 km finest) larger than the band
    // height (cumulus 5.2 km) - and a thin deck is the direct test: walls
    // 3x shorter should print a fan 3x weaker at the nadir.
    let thin_deck = fract(camera.light7_color.w * 0.000244140625) >= 0.5;
    let h_span = select(reg.h_hi - reg.h_lo, (reg.h_hi - reg.h_lo) * CLOUD_THIN_DECK_SCALE, thin_deck);
    let h_hi_thin = reg.h_lo + h_span;
    let h_hi_eff = min(h_hi_thin + tower * 0.8 * h_span, 1.0);
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
        + shape_w * CLOUD_BASE_DROP * reg.base_drop * v_band * v_band * (1.0 - lofi)
            * select(1.0, 0.0, cloud_bisect_index() == 5u);
    // Cumulus-scale cell split (phase 3, fidelity finding 4): the shape
    // volume's finest feature is ~11 km, and erosion can only nibble a
    // blob's edges - nothing could ever make a 1-2 km cloud. A second tap
    // of the SAME shape volume at a ~8 km tile raises the coverage
    // threshold between cells, splitting big masses into discrete cumuli.
    // Distance-faded like the puff band so orbit never pays or changes.
    var cell_g = 0.481;
    // v0.1252.2: the cell term is captured separately so the tau_vert
    // ENVELOPE below can exclude it (its 20.8 m voxels sample at ~mip 0
    // up close - genuinely pixel-scale - and through the hinge slope it
    // was the bulk of the ambient channel's residual grain). Density and
    // alpha keep the full carve.
    var cell_term = 0.0;
    if (cell_amt > 0.01) {
        // The cell split follows the warped walls (increment B 2.1).
        let c = textureSampleLevel(
            cloud_shape_tex, cloud_tile_sampler, select(ps, ps_b, field_on) * g_cell_freq,
            cloud_lod(lodb, CLOUD_LODC_CELL));
        // CENTERED at the bake's g-channel mean (increment 11): the split
        // is always on now (its distance fade is deleted), so it must
        // modulate coverage locally WITHOUT shifting the global mean -
        // (mean - c.g) raises the threshold in the gaps between cells and
        // lowers it slightly at the cores, zero-mean by construction.
        // 0.481 = the baked g-channel mean (bake_stats probe).
        cell_term = CLOUD_CELL_SPLIT * cell_amt * reg.fine * (0.481 - c.g);
        thr = thr + cell_term;
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
    let lod_shape = cloud_lod(lodb, CLOUD_LODC_SHAPE);
    // Bit 6 (v0.1271 experiment): WIDE EDGE - the density transition at a
    // silhouette becomes a ramp CLOUD_EDGE_WIDE_MUL times the fitted hinge
    // width, with the threshold shifted so the OUTER boundary stays where
    // it was (the ramp extends inward). Real cloud edges are radiatively
    // smoothed over ~250-400 m (Marshak 1995); a hinge of 0.005 noise units
    // is metres wide and every march step that straddles it is a coin flip.
    let wide_edge = fract(camera.light7_color.w * 0.0078125) >= 0.5;
    let sw0 = cloud_carve_width(lod_shape);
    // Runtime multiplier in light6_color.x (offset 304; zero-filled by the
    // celestial pass and read by no shader - light5_cone_inner.y/.z looked
    // free in a stale mod.rs comment but carry underwater extinction and
    // pix_ang). F10 slider / showcase cloud_edge_mul; the constant is the
    // fallback when the pad is 0.
    let edge_mul = select(CLOUD_EDGE_WIDE_MUL, camera.light6_color.x,
        camera.light6_color.x > 0.0);
    // ASYMMETRIC (v0.1271 round 2): the first cut widened the hinge on BOTH
    // sides with the threshold shifted to hold the outer boundary, which
    // meant full density needed body > thr + 2*sw and at x60 the interior
    // never saturated - the clouds thinned until the coverage alpha killed
    // them, and the metric drop was mostly missing edges. Real cloud edges
    // are a dense core with a wispy, low-density fringe OUTSIDE it, so the
    // ramp widens outward only: the inner side keeps the fitted sw0 and
    // saturates exactly where it did, the outer side stretches by edge_mul.
    // Round 3: CENTERED symmetric widening. Round 1 shifted the threshold to
    // hold the outer boundary and starved the interior; round 2 widened
    // only the outer side and changed nothing. A ramp centred on the
    // threshold keeps the mean coverage (P(body > thr)) to first order
    // while both the boundary and the saturation move by sw/2 - the
    // soft-remap shape production renderers use.
    let sw = select(sw0, sw0 * edge_mul, wide_edge);
    let thr_shift = 0.0;
    // The signed per-mip threshold offset (see CLOUD_CARVE_T0): coverage
    // is P(body > thr + T - sw), so T corrects the mip's distribution
    // shift in the direction the fit actually wants, while sw keeps its
    // own job as the hinge softness.
    let zc = (body - (thr + cloud_carve_thr_off(lod_shape) + thr_shift)) / sw;
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
    let norm_floor = select(1.0e-3, CLOUD_CARVE_NORM_FLOOR,
        fract(camera.light7_color.w * 0.00048828125) >= 0.5);
    let carve = clamp(
        hinge * sw / max(CLOUD_BODY_TOP - thr, norm_floor), 0.0, 1.0) * env;
    // The far rung's publishes (increment 4, see the declarations): the
    // hinge's own areal fraction and the pre-erosion carve.
    g_cloud_frac = clamp((zc + 1.0) * 0.5, 0.0, 1.0);
    g_cloud_carve_pt = carve;
    // v0.1252.2: a second, CELL-FREE hinge for the tau_vert envelope (see
    // g_cloud_carve's note). The cell split is a COVERAGE mechanism
    // (where cloud exists); the vertical column-depth estimate feeding
    // the two-stream diffusion floor is physically a smooth profile
    // quantity, and the cell tap's 20.8 m voxels put pixel-scale grain
    // into it. Pure ALU - no extra texture tap.
    let thr_env = thr - cell_term;
    let zc_e = (body - thr_env) / sw;
    var hinge_e: f32;
    if (zc_e <= -1.0) {
        hinge_e = 0.0;
    } else if (zc_e < 1.0) {
        let ue = zc_e + 1.0;
        hinge_e = 0.25 * ue * ue;
    } else {
        hinge_e = zc_e;
    }
    let carve_env = clamp(
        hinge_e * sw / max(CLOUD_BODY_TOP - thr_env, norm_floor), 0.0, 1.0) * env;
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
    // Neutralizer saturates at HALF the body crossfade (field-coherence
    // rebuild, 2026-08-31). w_built spans tc 0.20-0.30 while the arch
    // table starts building at tc 0.25, so a third of the fair-weather
    // population renders at v2_w in (0.5, 1) - where crown and pouch
    // are functions of a value that is MOSTLY a distance field, and
    // they partially re-printed the iso-distance rings the 2026-08-25
    // eyeball fix removed at v2_w = 1 (up to ~1.35x of ringed contrast,
    // per the lobe-lattice audit). The ring-carrying terms must die
    // FASTER than the body mixes: fully neutral by v2_w = 0.5, so no
    // cloud ever shows a majority-built body under fractal-path ring
    // shading. The crossfade itself is untouched.
    let ring_off = smoothstep(0.0, 0.5, v2_w);
    let crown = mix(
        clamp(u_band / clamp(u_crown, 1.0e-3, 1.0), 0.0, 1.0), 1.0, ring_off);
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
    let bd_wt = CLOUD_BASE_DROP * reg.base_drop * (1.0 - lofi)
        * select(1.0, 0.0, cloud_bisect_index() == 5u);
    // Same iso-distance defect as crown above: pouch is f(body), which
    // on the constructed path is a radial coordinate. Faded out with the
    // constructed weight (0 = no pouch darkening).
    let pouch = mix(clamp(
        sqrt(max(body - thr_base, 0.0) / max(bd_wt, 1.0e-3)), 0.0, 1.0),
        0.0, ring_off);
    g_cloud_bandtop = h_hi_eff;
    // Increment A: the column's OWN top, from the crown the carve already
    // solves for (u_crown = how far up this column reaches, band fractions).
    g_cloud_coltop = reg.h_lo + clamp(u_crown, 0.0, 1.0) * (h_hi_eff - reg.h_lo);
    g_cloud_pouch = pouch;
    g_cloud_carve = carve_env;
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
    // DEAD-TAP TRIM (v0.1285, perf day 0): the fray and detail taps only
    // erode base, base only feeds dens_fractal, and dens_fractal is mixed
    // with the built density by cs.v2 - at exactly 1.0 (a fully constructed
    // sample) their result is multiplied by zero. Skipping them there is
    // bit-exact: two of four taps saved on such eye samples and, on the
    // sun-profile path (density only, cavity unused), three of four.
    let built_only = cs.v2 >= 1.0;
    if (base <= 0.003) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    if (!built_only) {
    // COARSE fray (always on -> orbit wispiness): erode edges with the detail
    // volume's Worley FBM sampled at a LOW world frequency (~88 km features,
    // supra-pixel from orbit so no stipple), in the same stretched domain so
    // it streaks. Erode HARDER where the body is thin (the 1-base weight):
    // frayed filaments at the edges, solid cores -- erode-edges-keep-cores.
    let fr = textureSampleLevel(
        cloud_detail_tex, cloud_tile_sampler, cs.ps * g_fray_freq,
        cloud_lod(lodb, CLOUD_LODC_FRAY));
    let frfbm = fr.r * 0.625 + fr.g * 0.25 + fr.b * 0.125;
    // Bit 16 of the dev pad: fray erosion off (component bisect, v0.1279).
    let fray_on = select(1.0, 0.0, cloud_bisect_index() == 4u);
    let erode_c = frfbm * reg.fray * CLOUD_FRAY_ERODE * (0.35 + 0.65 * (1.0 - base)) * fray_on;
    base = clamp(cloud_remap(base, erode_c, 1.0, 0.0, 1.0), 0.0, 1.0);
    // FILAMENT streaking: the ridged-Perlin channel (detail alpha) frays flat
    // sheets into thin branching streaks. Weighted by the regime (cirrus high,
    // cumulus ~none) so only the high thin clouds get mares'-tail structure.
    let fmask = smoothstep(CLOUD_FIL_LO, CLOUD_FIL_HI, fr.a);
    base = base * mix(1.0, fmask, reg.filament);
    if (base <= 0.003) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
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
    if (detail_amt > 0.01 && !built_only) {
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
    if (puff_amt > 0.01 && base > 0.003 && !(built_only && g_sun_profile > 0.5)) {
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

// (cloud_density_light, the never-called light-march density, was deleted in
// v0.1285: the sun ladder uses cloud_density_hi with the profile flag.)

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
//
// SPLIT IN TWO (increment 1, v0.1286, the sun-shadow cache): this function
// walks rungs 0 and 1 itself (the 30 m + 57 m on-axis self-shadow every
// pixel keeps), then either reads rungs 2-11 from the planet-fixed cache
// (dev pad bit 16 on and the sample inside a window) or hands the SAME
// running state to cloud_sun_tau_far, which walks rungs 2-11 exactly as the
// single loop used to. The two halves are one arithmetic: the same dist /
// step_d recurrence, the same `p + sun_local * dist` tap positions, the
// same skips and the same break, so with the cache bit OFF the result is
// bit-identical to the pre-split ladder (the A/B twin). cloud_sun_tau_far
// is also what the bake pass evaluates at every lattice point.
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
    var w_split = 0.0;
    // Set when rungs 0-1 already decided the answer (the A2 buried case or
    // the opaque cap): no cache read, no far ladder.
    var done = false;
    for (var i = 0; i < 2; i = i + 1) {
        // Deep-sample coarse ladder (v0.1279 experiment, dev pad bit 20):
        // when the VIEW sample is already deep (g_deep_sample set by the
        // march from the eye transmittance), the first three rungs are
        // skipped so nearby lobe shadows do not print through the eye.
        // (cloud_sun_tau_far applies the same skip to rung 2.)
        if (g_deep_sample > 0.5 && i < 3
            && fract(camera.light7_color.w * 0.000000476837158203125) >= 0.5) {
            continue;
        }
        // Geometric ladder: the segment IS the step (see
        // CLOUD_LIGHT_NEAR_KM / RATIO above).
        dist = dist + step_d;
        let seg = step_d;
        step_d = step_d * CLOUD_LIGHT_RATIO;
        // First two taps stay ON-AXIS: the entry rind is the surface the
        // eye sees and must self-shadow exactly (the sun-profile split).
        let lp = p + sun_local * dist;
        // Slab skip (v0.1257): density outside the band is zero by
        // construction. See the fuller note in cloud_sun_tau_far.
        let lr_t = length(lp);
        if (lr_t < g_cloud_rb || lr_t > g_cloud_rt) {
            continue;
        }
        // Band limit = the tap's OWN segment length floored at the 260 m
        // radiative-smoothing scale; never the view footprint (v0.1264,
        // see cloud_sun_tau_far).
        let lod_t = max(
            log2(max(seg / g_cloud_upkm, 1.0e-4)),
            log2(0.26),
        );
        // The PROFILE body (v0.1252.4; sub-MFP fields at their means).
        g_sun_profile = 1.0;
        let dens = cloud_density_hi(
            lp, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt,
            lod_t).x;
        tau = tau + sigma * dens * seg;
        // ── INCREMENT A2: DEPTH-SPLIT LADDER (v0.1280) ──
        // The reference: at 45/km the transport mean free path is 148 m,
        // so 150 m inside the medium the radiance field is already near
        // isotropic and no lobe 400-1500 m away can cast a visible shadow.
        // A sample buried under rungs 0-1 (87 m on-axis) takes the slant
        // column for the rest instead of resolving its neighbours with
        // rungs 2-11 - which is exactly what printed the lobes around an
        // eye inside the cloud as radial petals.
        if (i == 1) {
            g_sun_tau01 = tau;
            if (g_ms_on > 0.5) {
                let w_deep = smoothstep(1.5, 4.0, tau);
                if (w_deep >= 0.999) {
                    tau = max(tau, g_sun_tau_col);
                    done = true;
                    break;
                }
                w_split = w_deep;
            }
        }
        // v0.911 (perf audit #3): once the sun path is this optically deep
        // every scatter octave is effectively zero - later taps cannot
        // change the pixel. Cap raised again with physical extinction
        // (exp(-40 * 0.20) is 3e-4 on the slowest octave).
        if (tau > 40.0) {
            done = true;
            break;
        }
    }
    // ── THE CACHE READ (increment 1) ── rungs 2-11 from the lattice.
    // The default source code: "decided" when rungs 0-1 already settled
    // the answer (no cache read, no far ladder), else "fallback", which
    // covers every path that runs the ladder (bit off, outside both
    // windows). The cache branch below overwrites it with the window code.
    // NOTE (dev pad bit 20, the v0.1279 deep-sample experiment): that toggle
    // skips rungs 0, 1, 2 per pixel, but with the cache on rung 2 arrives
    // from the atlas, which is baked with g_deep_sample = 0. The two toggles
    // do not compose as written; run bit 20 with the cache off.
    g_light_src = select(CLOUD_LC_SRC_FALLBACK, CLOUD_LC_SRC_DECIDED, done);
    if (!done) {
        var cached = false;
        if (cloud_light_cache_on()) {
            let lc = light_cache_tau(p);
            if (lc.y < 1.0) {
                // Inside the coarse window: the cached far rungs, blended
                // toward the fallback across the coarse outer band.
                var tau_c = tau + lc.x;
                if (lc.y > 0.0) {
                    var tau_fb = max(tau, g_sun_tau_col);
                    if (CLOUD_LC_FAR_ANALYTIC < 0.5) {
                        tau_fb = cloud_sun_tau_far(
                            p, sun_local, t, seed, weather_a, reg,
                            detail_amt, puff_amt, cell_amt, tau);
                    }
                    tau_c = mix(tau_c, tau_fb, lc.y);
                }
                tau = tau_c;
                g_light_src = mix(lc.z, CLOUD_LC_SRC_FALLBACK, lc.y);
                cached = true;
            }
        }
        if (!cached) {
            tau = cloud_sun_tau_far(
                p, sun_local, t, seed, weather_a, reg,
                detail_amt, puff_amt, cell_amt, tau);
        }
    }
    // Flag hygiene: cloud_sun_tau's single exit. A leak would make the
    // EYE see the profile body - visible as lost close-up texture.
    g_sun_profile = 0.0;
    if (w_split > 0.0) {
        tau = mix(tau, max(g_sun_tau01, g_sun_tau_col), w_split);
    }
    return tau;
}

// Rungs 2 to CLOUD_HI_LIGHT_SAMPLES - 1 of the sun ladder: the big-mass
// shadow, the part the sun-shadow cache stores. `p` is the point the ladder
// is measured FROM (the view sample, or a lattice point in the bake); the
// function re-walks the rung 0-1 recurrence without tapping so its rung 2
// lands 195 m sunward of p exactly as it always did (dist = 30 + 57 + 108 m,
// the same f32 sum in the same order), then taps rungs 2-11 on the profile
// body inside the sun cone. `tau_in` is the optical depth accumulated so
// far (rungs 0-1 for a view sample, 0 for the bake) so the opaque cap
// breaks at the same cumulative value the single loop did; the return is
// the cumulative total (tau_in + the far rungs).
//
// Everything in here is a pure function of position and sun direction
// (v0.1264: no view footprint enters; there is deliberately NO lodb
// parameter, each tap band-limits by its own segment), which is what makes
// it cacheable on a planet-fixed lattice in the first place. The one view-dependent
// input is the bit-20 experiment's g_deep_sample skip, which is 0 in the
// bake pass.
fn cloud_sun_tau_far(
    p: vec3<f32>,
    sun_local: vec3<f32>,
    t: f32,
    seed: f32,
    weather_a: f32,
    reg: CloudRegime,
    detail_amt: f32,
    puff_amt: f32,
    cell_amt: f32,
    tau_in: f32,
) -> f32 {
    let sigma = reg.ext_km / g_cloud_upkm;
    var tau = tau_in;
    var dist = 0.0;
    var step_d = g_light_near;
    // Cone basis perpendicular to the sun ray (v0.1252.2; see
    // CLOUD_SUN_CONE_K). Any stable pair works - the spiral covers the
    // disc.
    var cu = cross(sun_local, vec3<f32>(0.0, 1.0, 0.0));
    if (dot(cu, cu) < 1.0e-6) {
        cu = cross(sun_local, vec3<f32>(1.0, 0.0, 0.0));
    }
    cu = normalize(cu);
    let cv = cross(sun_local, cu);
    for (var i = 0; i < CLOUD_HI_LIGHT_SAMPLES; i = i + 1) {
        // Deep-sample coarse ladder (v0.1279 experiment, dev pad bit 20):
        // the same skip cloud_sun_tau applies to rungs 0-1, here for rung 2
        // (and, because the skip does not advance the recurrence, rungs 0
        // and 1 stay un-advanced under it too, as in the single loop).
        if (g_deep_sample > 0.5 && i < 3
            && fract(camera.light7_color.w * 0.000000476837158203125) >= 0.5) {
            continue;
        }
        // Geometric ladder: the segment IS the step (see
        // CLOUD_LIGHT_NEAR_KM / RATIO above).
        dist = dist + step_d;
        let seg = step_d;
        step_d = step_d * CLOUD_LIGHT_RATIO;
        // Rungs 0 and 1 belong to the caller (or, in the bake, to the
        // pixel that will read this lattice point): advance past them.
        if (i < 2) {
            continue;
        }
        var lp = p + sun_local * dist;
        // First two taps stay ON-AXIS: the entry rind is the surface the
        // eye sees and must self-shadow exactly (the sun-profile split).
        // Farther taps spiral inside the cone; the g_lod_jitter phase is
        // per-pixel + frame-advanced on the temporal path (0 on the
        // Medium direct path, where the fixed spiral still buys the
        // lateral average without needing an integrator, and 0 in the
        // bake, whose lattice is planet-fixed).
        let ang = 2.3999632 * f32(i) + g_lod_jitter * 6.2831853;
        lp = lp + (cu * cos(ang) + cv * sin(ang))
            * (dist * CLOUD_SUN_CONE_K);
        // Band-limit each tap by ITS OWN step length too (phase 5): the
        // far taps stride tens of km and should integrate the mean field
        // at that scale, not point-sample full-frequency noise. Never
        // finer than the view sample's footprint.
        // ── SLAB SKIP (v0.1257, the operator's sub-1-FPS report) ──
        // The sun ladder is geometric and reaches ~125 km by its last
        // rung, while the cloud band is ~12 km thick. Every tap beyond
        // the band was paying a FULL constructed-cluster evaluation
        // (a 3x3 cell search plus a 20-lobe build, the single most
        // expensive call in the renderer) to be told there is no cloud
        // in empty stratosphere. The view march never had this problem
        // because it clips its own segment to the slab; the sun ladder
        // has no such clip. Skipping - not breaking, because a low sun
        // leaves and re-enters the band along a shallow chord - costs
        // one length() and removes the majority of sun-tap work at
        // every altitude. Physically exact: density outside the band
        // is zero by construction, so nothing is approximated away.
        let lr_t = length(lp);
        if (lr_t < g_cloud_rb || lr_t > g_cloud_rt) {
            continue;
        }
        // ── SUN TAU MUST NOT DEPEND ON THE CAMERA (v0.1264) ──
        // This was max(lodb, log2(seg)) - the sun tap's mip FLOORED BY THE
        // VIEW FOOTPRINT. Sunlight arriving at a point in a cloud does not
        // care where the camera is, but lodb does: on a down-look the
        // footprint is monotone in the angle from the nadir, so the sun's
        // transmittance at a FIXED world point changed with the viewer's
        // screen angle - a radial lighting gradient centred on the view
        // axis, by construction.
        //
        // The operator photographed exactly this and it is what separated
        // the two rosettes: at 2.7 km under a thick deck their COVERAGE
        // channel is uniformly white (saturated, no pattern at all) while
        // DIRECT SUN and AMBIENT show an enormous radial flower. Coverage
        // saturates and hides its own drift; the lighting does not.
        //
        // The band limit a sun tap is entitled to is its OWN segment
        // length, floored by the radiative-smoothing scale (260 m - the
        // scale below which multiple scattering means the sun physically
        // cannot carry structure, the same constant the profile body
        // uses). Both are world-space quantities, so tau is now a pure
        // function of position and sun direction. It also caps cost the
        // way the lodb floor used to, without the view dependence.
        let lod_t = max(
            log2(max(seg / g_cloud_upkm, 1.0e-4)),
            log2(0.26),
        );
        // ALL taps on the REAL eroded density (increment 10, the dots'
        // deepest root): the old body-only far taps returned ~1 across the
        // whole carved envelope - a MASK, not a density - which at
        // physical extinction (45/km) reported tau in the HUNDREDS where
        // the converged reference reads 1-10. Bimodal tau (0 in gaps,
        // absurd in bodies) WAS the 18.9x per-texel energy coin flip. The
        // CPU twin measured the fix: -90% -> -1% ladder error at 12 taps.
        // ALL taps read the PROFILE body (v0.1252.4; sub-MFP fields at
        // their means - see g_sun_profile). The v0.1252.2 cut kept the
        // first two taps fully detailed for rind self-shadow, and the
        // operator's next capture showed the residual: WHITE sparkle on
        // shadowed faces - single pixels where a detailed near tap's
        // erosion gap let full sun through a dark face. The lobe-scale
        // SDF (which the profile keeps) still self-shadows; the sub-MFP
        // texture on the first 120 m of sun path rendered as per-pixel
        // noise, not as texture, at this sampling rate. Mean
        // substitution preserves mean tau (the increment-10 bimodal bug
        // cannot return).
        g_sun_profile = 1.0;
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
    // (g_sun_profile is reset by the caller's single exit: cloud_sun_tau,
    // or the bake fragment.)
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
        let ph_dir = mix(
            cloud_hg(cos_vs, CLOUD_HG_BACK * g_n),
            cloud_hg(cos_vs, CLOUD_HG_FWD * g_n),
            CLOUD_HG_FWD_WEIGHT,
        );
        // Increment A3: the octaves go isotropic with burial.
        let ph = mix(ph_dir, 1.0, g_ms_prof * g_ms_on);
        e = e + c_n * ph * exp(-tau * a_n);
        c_n = c_n * 0.5;
        a_n = a_n * 0.5;
        g_n = g_n * 0.5;
    }
    // Relaxed-Beer contrast floor (v0.1252.2; see CLOUD_SUN_RELAX).
    let ph_wide = mix(
        cloud_hg(cos_vs, CLOUD_HG_BACK * 0.25),
        cloud_hg(cos_vs, CLOUD_HG_FWD * 0.25),
        CLOUD_HG_FWD_WEIGHT,
    );
    e = max(e, CLOUD_SUN_RELAX * ph_wide * exp(-0.25 * tau));
    let t_diff = 1.0 / (1.0 + 0.75 * (1.0 - CLOUD_HG_FWD) * tau_diff);
    // Increment A3 replaces this transmittance-only floor with a real
    // in-scattered source (cloud_ms_source); off when A is on.
    return e + CLOUD_MS_DIFFUSE * t_diff * (1.0 + 0.13 * cos_vs) * (1.0 - g_ms_on);
}

// ── INCREMENT A3: THE IN-SCATTERED SOURCE (v0.1280) ──
// Eddington two-stream, conservative (omega0 = 1: a droplet is a near-perfect
// scatterer, so more extinction means MORE light returned, never less - the
// shipped floor fell with extinction because it was a transmittance),
// delta-scaled with the repo's own T form. tau_above / tau_below are the
// local column optical depths, x the fractional depth, c = dot(-rd, up)
// (+1 when the light travels up toward an eye above), mu_s the sun cosine.
// E_WHITE = 1/pi: a sunlit white Lambertian surface in the shader's relative
// scatter units. Numbers at tau_tot 27 (600 m at 45/km): top third looking
// down 0.17 (143/255 after ACES), looking up 0.29, mid-deck 0.10, the base
// from below 0.10. At x3 extinction it RISES 38%.
fn cloud_ms_source(tau_above: f32, tau_below: f32, c: f32, mu_s: f32, prof: f32) -> f32 {
    let tau_tot = tau_above + tau_below;
    let tt = 1.0 / (1.0 + 0.75 * (1.0 - CLOUD_HG_FWD) * tau_tot);
    let x = tau_above / max(tau_tot, 1.0e-3);
    let e_white = 0.3183;
    return e_white * max(mu_s, 0.0) * prof
        * max(0.5 * (1.0 + (1.0 - tt) * (1.0 - 2.0 * x)) - 0.75 * tt * c, 0.0);
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

// ── THE CLOUD FAR RUNG (perf arc increment 4): the planet-fixed PROFILE ──
//
// Interface contract: docs/design/cloud-far-rung.md (v2, 2026-09-05). This
// block is the A17 integration stub the orchestrator commits BEFORE the two
// implementers start in separate worktrees (Rust: the atlas, passes, rects,
// pads, groups, F10; WGSL: the real bake fragments and the transmittance /
// lighting wiring in cloud_march_core). Every constant, pad decode, slice
// layout formula and function signature here IS the shared interface: change
// one only through the contract, and keep the "both"-owned constants in the
// one-line `const NAME: <type> = <value>;` form the Rust sync test parses.
//
// What the profile is: per lattice cell and height bin, the cloud FRACTION f
// (areal fraction of the cell holding cloud anywhere in the bin's height
// range), the MEAN density G of the same field the march renders, and the
// COLUMN C above each bin, baked at the cell's own footprint into an RGBA8
// atlas over six nested TOROIDAL (clipmap) windows on an absolute equal-angle
// equirect lattice, plus one global equirect map with a real mip pyramid. The
// march reads it with 4-tap loads chosen by the same lodb that picks every
// noise mip, and the profile share is integrated in TRANSMITTANCE by a
// clumped-medium law (cloud_fr_t_pf, wired into cloud_march_core; the bake
// fragments live in 45-cloud-temporal.wgsl).
//
// The speckles it kills: the constructed bodies point-sampled at footprints
// larger than the clouds (873 km: 25-31 percent white texels as sugar grain).
//
// Pad: camera.light2_color = (ground_I_0, ground_J_0, knob, flags), all four
// exact integers in f32, written every frame by Rust, zeros when no atlas
// exists. Knob 0 = today's field, bit-identical (the A/B twin). Atlas: group 3
// binding 14 (tree_atlas_tex, read only by the vegetation branches today), no
// bind-group-layout change; the global reads use the group's albedo_sampler
// (Linear / Linear / mipmap Nearest), the window reads use textureLoad.
//
// Lattice (angular, radians of the planet-local unit direction): level L = 0..5,
// cell c_L = CELL0_KM * 2^L km of ARC (north-south everywhere; east-west
// c_L * cos(lat): deliberately NOT cos-scaled, so a window spans less ground
// east-west at high latitude and hands to the mipped global sooner - a blur,
// never a speckle). N_I(L) = floor(2 pi planet_km / c_L) cells around,
// N_J(L) = floor(pi planet_km / c_L) rows, both floors in f32 in BOTH
// languages so they are bit-identical. Cell (I, J) centre: lon = (I + 0.5) *
// cell_rad - pi, lat = (J + 0.5) * cell_rad - pi/2. Window L covers I_abs in
// [I0_L, I0_L + 512), J_abs in [J0_L, J0_L + 512) with I0_L = ground_I_L - 256;
// storage x = x0_s + pmod(I_abs, 512), y = y0_s + pmod(J_abs, 512) (toroidal:
// a cell's storage position never changes while it stays in the window).
//
// Texels: per level nine slices s = L * 9 + p at x0_s = (s mod 12) * 512,
// y0_s = (s / 12) * 512: pair slices p = 0..5 hold (f_k, G_k, f_k+1, G_k+1)
// for k = 2p; column slices p = 6, 7, 8 (q = p - 6) hold (C_4q .. C_4q+3) for
// q = 0, 1 and (C_8, C_9, C_10, T) for q = 2 (C_11 is identically zero, so its
// channel carries the whole column T). f, G linear in [0, 1]; columns
// sqrt-encoded: enc(C) = sqrt(clamp(C / COL_SCALE, 0, 1)). Global: 2048 x
// 1024 texels per slice at row 2560 (pair0 at x 0, pair1 at 2048, column at
// 4096), row 0 = north; mips 1..6 hold the 2x2 box average of the global
// region only (origin (0, 2560 >> m), size (6144 >> m, 1024 >> m)).

// ── Constants (the contract's table; the "both"-owned rows are parsed out of
// this file by the Rust sync test in exactly this one-line form) ──
// finest window cell, km of arc
const CLOUD_FR_CELL0_KM: f32 = 0.25;
// window levels, cells 0.25 to 8 km
const CLOUD_FR_LEVELS: i32 = 6;
// lodb of level 0 (= log2(CELL0_KM); the Rust test asserts it)
const CLOUD_FR_LOD0: f32 = -2.0;
// window cells across (and down)
const CLOUD_FR_NX: i32 = 512;
// slab height bins
const CLOUD_FR_NZ: i32 = 12;
// pair slices (f_k, G_k, f_k+1, G_k+1) per level
const CLOUD_FR_PAIRS: i32 = 6;
// column slices per level (the last carries T in .w)
const CLOUD_FR_CSLICES: i32 = 3;
// pair + column slices per level
const CLOUD_FR_SLICES_PER_LEVEL: i32 = 9;
// slices per atlas row (12 x 512 = 6144)
const CLOUD_FR_SLICE_COLS: i32 = 12;
// column encoding scale: enc(C) = sqrt(C / 12), dec(v) = v * v * 12
const CLOUD_FR_COL_SCALE: f32 = 12.0;
// atlas width, mip 0
const CLOUD_FR_ATLAS_W: i32 = 6144;
// atlas height, mip 0 (window band 2560 + global 1024)
const CLOUD_FR_ATLAS_H: i32 = 3584;
// global equirect map width (one slice)
const CLOUD_FR_GLOBAL_W: i32 = 2048;
// global equirect map height
const CLOUD_FR_GLOBAL_H: i32 = 1024;
// atlas row where the global region starts (5 * 2^9: mip-aligned)
const CLOUD_FR_GLOBAL_Y0: i32 = 2560;
// pooled height bins of the global (three slab bins each)
const CLOUD_FR_GLOBAL_NZ: i32 = 4;
// atlas mip count; mips 1..6 hold the global region only
const CLOUD_FR_GLOBAL_MIPS: i32 = 7;
// window edge hand-off band (Chebyshev distance, smoothstep)
const CLOUD_FR_BLEND_FRAC: f32 = 0.20;
// hand-off band start in dithered lodf (250 m footprint)
const CLOUD_FR_LOD_LO: f32 = -2.0;
// hand-off band end in dithered lodf (1 km footprint)
const CLOUD_FR_LOD_HI: f32 = 0.0;
// fraction floor for D_in and the in-cloud column
const CLOUD_FR_F_EPS: f32 = 0.02;
// horizontal element size, non-built families, km
const CLOUD_FR_ELEM_THIN_KM: f32 = 8.0;
// vertical element factor on the archetype aspect
const CLOUD_FR_ELEM_SQUASH: f32 = 0.65;
// cell-stratified ellipse test points per (bin, height), fine levels (4x4)
const CLOUD_FR_PTS: i32 = 16;
// heights per bin in the bake
const CLOUD_FR_ZSUB: i32 = 4;
// cv2 cells enumerated per texel, hard cap (stride subsampling beyond)
const CLOUD_FR_MAX_CV2: i32 = 512;
// reference bake points across the cell per bin (8x8)
const CLOUD_FR_REF_K: i32 = 8;
// reference bake heights per bin
const CLOUD_FR_REF_KZ: i32 = 2;
// calibration height rows per archetype
const CLOUD_FR_CALIB_ROWS: i32 = 32;
// canonical clouds averaged per archetype
const CLOUD_FR_CALIB_SEEDS: i32 = 8;
// calibration cross-section grid per side
const CLOUD_FR_CALIB_GRID: i32 = 64;
// calibration row span in cloud heights (0..1.5)
const CLOUD_FR_CALIB_YMAX: f32 = 1.5;
// final calibration table origin x, MIP-1 texel coords (32 x 4)
const CLOUD_FR_CALIB_X0: i32 = 1536;
// final calibration table origin y, MIP-1 texel coords
const CLOUD_FR_CALIB_Y0: i32 = 1024;
// per-seed staging origin x, MIP-2 texel coords (32 x 32)
const CLOUD_FR_CALIB_STAGE_X0: i32 = 768;
// per-seed staging origin y, MIP-2 texel coords
const CLOUD_FR_CALIB_STAGE_Y0: i32 = 512;
// Knob codes (pad light2_color.z).
// off: today's field, bit-identical (the A/B twin)
const CLOUD_FR_KNOB_OFF: i32 = 0;
// automatic level by lodb, blended across levels and edges
const CLOUD_FR_KNOB_ON: i32 = 1;
// level 0 at w = 1 on every sample (Rust keeps it active)
const CLOUD_FR_KNOB_FORCE0: i32 = 2;
// level 1 forced
const CLOUD_FR_KNOB_FORCE1: i32 = 3;
// level 2 forced
const CLOUD_FR_KNOB_FORCE2: i32 = 4;
// level 3 forced
const CLOUD_FR_KNOB_FORCE3: i32 = 5;
// level 4 forced
const CLOUD_FR_KNOB_FORCE4: i32 = 6;
// level 5 forced
const CLOUD_FR_KNOB_FORCE5: i32 = 7;
// automatic level, hard switch, no blend anywhere (the prove-red)
const CLOUD_FR_KNOB_HARD: i32 = 8;
// the reference bake (dev only, slow)
const CLOUD_FR_KNOB_REF: i32 = 9;

// ── Pad decode (light2_color = (ground_I_0, ground_J_0, knob, flags)) ──
// The knob code.
fn cloud_profile_knob() -> i32 {
    return i32(camera.light2_color.z);
}
// Flag bit b of the integer-valued pad: exact, because the pad is an integer
// and the scaled fract isolates one bit. Bit 0 = some window level valid,
// bit 1 = global valid (first full pass plus mips done), bits 2..7 = level
// L = b - 2 valid (its first full fill completed), bit 8 = calibration valid.
fn cloud_profile_flag(b: i32) -> bool {
    return fract(camera.light2_color.w * exp2(-f32(b + 1))) >= 0.5;
}
fn cloud_profile_level_valid(L: i32) -> bool {
    return cloud_profile_flag(2 + L);
}
fn cloud_profile_global_valid() -> bool {
    return cloud_profile_flag(1);
}

// Positive modulus (the toroidal storage rule): pmod(a, n) = ((a mod n) + n) mod n.
fn pmod(a: i32, n: i32) -> i32 {
    return ((a % n) + n) % n;
}

// ── The read side's results ──
// One window level: ok, f, G, the columns as optical depths, and the edge
// hand-off weight (0 inside the window, 1 at its rim).
struct ProfileLevel {
    ok: bool,
    f: f32,
    G: f32,
    tau_above: f32,
    tau_below: f32,
    w_edge: f32,
};
// The global map: the march's (f, G, tau_above, tau_below) at h, plus ALL
// four pooled bins (fp, Gp) and the decoded column (Cp_0, Cp_1, Cp_2, Tp) for
// the Low sheet, which reads the whole column at once (the contract's
// `h_or_all`: one call serves both callers).
struct ProfileGlobal {
    ok: bool,
    f: f32,
    G: f32,
    tau_above: f32,
    tau_below: f32,
    fp: vec4<f32>,
    Gp: vec4<f32>,
    Cp: vec4<f32>,
};
// The tap after the level walk: ok, f, G, the columns, and the (blended)
// level index 0..6 (6 = the global) for map_diag channel 11.
struct ProfileTap {
    ok: bool,
    f: f32,
    G: f32,
    tau_above: f32,
    tau_below: f32,
    level: f32,
};

// Extinction per drawn-shell unit of the CURRENT march (sigma_v), published
// by the hook in cloud_march_core so the reads can express the columns as
// optical depths while the contract's signatures carry only (dirp, h, lodb).
var<private> g_pf_sigma_v: f32 = 0.0;

// Column encoding (sqrt gives the low-tau end, where the light changes, 8-bit
// steps of 0.008 in tau instead of 2.0). Bilinear on encoded channels must
// decode each tap FIRST, then blend: the encoding is not linear.
fn cloud_profile_col_dec(v: vec4<f32>) -> vec4<f32> {
    return v * v * CLOUD_FR_COL_SCALE;
}
fn cloud_profile_col_enc(c: vec4<f32>) -> vec4<f32> {
    return sqrt(clamp(c / CLOUD_FR_COL_SCALE, vec4<f32>(0.0), vec4<f32>(1.0)));
}
// Channel c of a texel, 0..3 (no dynamic vector indexing: the HLSL backend
// is the one that decides, see check_hlsl_expressible).
fn cloud_profile_chan(v: vec4<f32>, c: i32) -> f32 {
    if (c <= 0) { return v.x; }
    if (c == 1) { return v.y; }
    if (c == 2) { return v.z; }
    return v.w;
}
// Storage origin of slice s = L * 9 + p: 12 slices per atlas row, five rows.
fn cloud_profile_slice_origin(s: i32) -> vec2<i32> {
    return vec2<i32>((s % CLOUD_FR_SLICE_COLS) * CLOUD_FR_NX, (s / CLOUD_FR_SLICE_COLS) * CLOUD_FR_NX);
}
// The four-tap bilinear read of one slice, RAW channels (pair slices).
// (xa, xb) / (ya, yb) are the two storage columns / rows inside the slice.
fn cloud_profile_load4(o: vec2<i32>, xa: i32, xb: i32, ya: i32, yb: i32, fu: f32, fv: f32) -> vec4<f32> {
    let t00 = textureLoad(tree_atlas_tex, vec2<i32>(o.x + xa, o.y + ya), 0);
    let t10 = textureLoad(tree_atlas_tex, vec2<i32>(o.x + xb, o.y + ya), 0);
    let t01 = textureLoad(tree_atlas_tex, vec2<i32>(o.x + xa, o.y + yb), 0);
    let t11 = textureLoad(tree_atlas_tex, vec2<i32>(o.x + xb, o.y + yb), 0);
    return mix(mix(t00, t10, fu), mix(t01, t11, fu), fv);
}
// The same for a COLUMN slice: every tap decoded before the blend.
fn cloud_profile_load4_col(o: vec2<i32>, xa: i32, xb: i32, ya: i32, yb: i32, fu: f32, fv: f32) -> vec4<f32> {
    let t00 = cloud_profile_col_dec(textureLoad(tree_atlas_tex, vec2<i32>(o.x + xa, o.y + ya), 0));
    let t10 = cloud_profile_col_dec(textureLoad(tree_atlas_tex, vec2<i32>(o.x + xb, o.y + ya), 0));
    let t01 = cloud_profile_col_dec(textureLoad(tree_atlas_tex, vec2<i32>(o.x + xa, o.y + yb), 0));
    let t11 = cloud_profile_col_dec(textureLoad(tree_atlas_tex, vec2<i32>(o.x + xb, o.y + yb), 0));
    return mix(mix(t00, t10, fu), mix(t01, t11, fu), fv);
}

// Window coordinates of a direction at level L: (du, dv) = the continuous
// cell coordinate relative to the GROUND cell, the date line folded, so the
// window is du, dv in [-256, 255). One function so the containment walk and
// the read can never disagree.
fn cloud_profile_window_uv(L: i32, lon: f32, lat: f32) -> vec2<f32> {
    let planet_km = material.params2.z;
    let c_km = CLOUD_FR_CELL0_KM * exp2(f32(L));
    let cell_rad = c_km / planet_km;
    let NI = floor(TAU * planet_km / c_km);
    let gI = floor(camera.light2_color.x / exp2(f32(L)));
    let gJ = floor(camera.light2_color.y / exp2(f32(L)));
    let u = (lon + PI) / cell_rad - 0.5;
    let v = (lat + 0.5 * PI) / cell_rad - 0.5;
    var du = u - gI;
    if (du >= 0.5 * NI) {
        du = du - NI;
    } else if (du < -0.5 * NI) {
        du = du + NI;
    }
    let dv = v - gJ;
    return vec2<f32>(du, dv);
}
// Does level L's window (valid, and containing the sample) cover this
// direction? Arithmetic only, no fetch.
fn cloud_profile_contains(L: i32, lon: f32, lat: f32) -> bool {
    if (!cloud_profile_level_valid(L)) {
        return false;
    }
    let d = cloud_profile_window_uv(L, lon, lat);
    let half = f32(CLOUD_FR_NX / 2);
    return d.x >= -half && d.x < half - 1.0 && d.y >= -half && d.y < half - 1.0;
}
// The arithmetic walk (A13): the first level >= L0 whose window contains the
// sample; CLOUD_FR_LEVELS (6) = "the global" when none up to 5 does.
fn cloud_profile_walk(L0: i32, lon: f32, lat: f32) -> i32 {
    for (var L = L0; L < CLOUD_FR_LEVELS; L = L + 1) {
        if (cloud_profile_contains(L, lon, lat)) {
            return L;
        }
    }
    return CLOUD_FR_LEVELS;
}

// The window read at one level (all of it textureLoad, level 0): one or two
// pair fetches and one or two column fetches, four loads each.
fn cloud_profile_level(L: i32, lon: f32, lat: f32, h: f32) -> ProfileLevel {
    var r = ProfileLevel(false, 0.0, 0.0, 0.0, 0.0, 1.0);
    let planet_km = material.params2.z;
    let c_km = CLOUD_FR_CELL0_KM * exp2(f32(L));
    let NJ = floor(PI * planet_km / c_km);
    let gI = floor(camera.light2_color.x / exp2(f32(L)));
    let gJ = floor(camera.light2_color.y / exp2(f32(L)));
    let d = cloud_profile_window_uv(L, lon, lat);
    let du = d.x;
    let dv = d.y;
    let half = f32(CLOUD_FR_NX / 2);
    if (!(cloud_profile_level_valid(L) && du >= -half && du < half - 1.0
        && dv >= -half && dv < half - 1.0)) {
        return r;
    }
    let i0 = floor(du);
    let fu = du - i0;
    let j0 = floor(dv);
    let fv = dv - j0;
    // Chebyshev edge weight: 0 inside, rising across the outer BLEND_FRAC of
    // the half-width to 1 at the rim, where the level hands off.
    let m = max(abs(du), abs(dv)) / (half - 1.0);
    r.w_edge = smoothstep(1.0 - CLOUD_FR_BLEND_FRAC, 1.0, m);
    // Storage coordinates of the four taps (J clamped to existing rows).
    let nj = i32(NJ);
    let ia = i32(gI) + i32(i0);
    let xa = pmod(ia, CLOUD_FR_NX);
    let xb = pmod(ia + 1, CLOUD_FR_NX);
    let ja = i32(gJ) + i32(j0);
    let ya = pmod(clamp(ja, 0, nj - 1), CLOUD_FR_NX);
    let yb = pmod(clamp(ja + 1, 0, nj - 1), CLOUD_FR_NX);
    // Bins: the pair (k0, k1) bracketing the sample for f and G, and the bin
    // kb holding it for the column split.
    let hz = h * f32(CLOUD_FR_NZ);
    let fk = clamp(hz - 0.5, 0.0, f32(CLOUD_FR_NZ - 1));
    let k0 = i32(floor(fk));
    let k1 = min(k0 + 1, CLOUD_FR_NZ - 1);
    let wk = fk - f32(k0);
    let kb = clamp(i32(floor(hz)), 0, CLOUD_FR_NZ - 1);
    // Fraction of bin kb ABOVE the sample.
    let frac_above = clamp(f32(kb + 1) - hz, 0.0, 1.0);
    let s_base = L * CLOUD_FR_SLICES_PER_LEVEL;
    // Pair slice(s) k0/2 (and k1/2 if different).
    let o0 = cloud_profile_slice_origin(s_base + k0 / 2);
    let p0 = cloud_profile_load4(o0, xa, xb, ya, yb, fu, fv);
    var p1 = p0;
    if (k1 / 2 != k0 / 2) {
        let o1 = cloud_profile_slice_origin(s_base + k1 / 2);
        p1 = cloud_profile_load4(o1, xa, xb, ya, yb, fu, fv);
    }
    let f0 = select(p0.x, p0.z, (k0 % 2) == 1);
    let G0 = select(p0.y, p0.w, (k0 % 2) == 1);
    let f1 = select(p1.x, p1.z, (k1 % 2) == 1);
    let G1 = select(p1.y, p1.w, (k1 % 2) == 1);
    r.f = mix(f0, f1, wk);
    r.G = mix(G0, G1, wk);
    // kb is always k0 or k1 (fk = hz - 0.5 brackets floor(hz)).
    let G_kb = select(G1, G0, kb == k0);
    // Column slice kb/4 -> C_kb (decoded); T from column slice 2 channel w
    // (a second column fetch unless kb/4 == 2). C_11 is identically zero.
    let q = kb / 4;
    let oc = cloud_profile_slice_origin(s_base + CLOUD_FR_PAIRS + q);
    let cq = cloud_profile_load4_col(oc, xa, xb, ya, yb, fu, fv);
    var T = cq.w;
    if (q != CLOUD_FR_CSLICES - 1) {
        let oT = cloud_profile_slice_origin(s_base + CLOUD_FR_PAIRS + CLOUD_FR_CSLICES - 1);
        T = cloud_profile_load4_col(oT, xa, xb, ya, yb, fu, fv).w;
    }
    let C_kb = select(cloud_profile_chan(cq, kb % 4), 0.0, kb == CLOUD_FR_NZ - 1);
    // One bin in drawn-shell units.
    let dz = (g_cloud_rt - g_cloud_rb) / f32(CLOUD_FR_NZ);
    let above = C_kb + G_kb * frac_above;
    r.tau_above = g_pf_sigma_v * dz * above;
    r.tau_below = g_pf_sigma_v * dz * max(T - above, 0.0);
    r.ok = true;
    return r;
}

// One integer mip of the global map: the two pair slices raw, the column
// slice decoded. Hardware bilinear inside the slice at that mip; every read
// is clamped to texel centres inside its own slice so the filter never
// crosses a slice edge and the sampler's u wrap is never reached.
struct ProfileGlobalRaw {
    pair0: vec4<f32>,
    pair1: vec4<f32>,
    col: vec4<f32>,
};
fn cloud_profile_global_fetch(m: i32, lon: f32, lat: f32) -> ProfileGlobalRaw {
    let sh = u32(m);
    let w_m = f32(CLOUD_FR_GLOBAL_W >> sh);
    let h_m = f32(CLOUD_FR_GLOBAL_H >> sh);
    let y0_m = f32(CLOUD_FR_GLOBAL_Y0 >> sh);
    let aw = f32(CLOUD_FR_ATLAS_W >> sh);
    let ah = f32(CLOUD_FR_ATLAS_H >> sh);
    let u_m = clamp((lon + PI) / TAU * w_m, 0.5, w_m - 0.5);
    let v_m = clamp((0.5 * PI - lat) / PI * h_m, 0.5, h_m - 0.5);
    let lvl = f32(m);
    var r: ProfileGlobalRaw;
    r.pair0 = textureSampleLevel(tree_atlas_tex, albedo_sampler,
        vec2<f32>((0.0 * w_m + u_m) / aw, (y0_m + v_m) / ah), lvl);
    r.pair1 = textureSampleLevel(tree_atlas_tex, albedo_sampler,
        vec2<f32>((1.0 * w_m + u_m) / aw, (y0_m + v_m) / ah), lvl);
    r.col = cloud_profile_col_dec(textureSampleLevel(tree_atlas_tex, albedo_sampler,
        vec2<f32>((2.0 * w_m + u_m) / aw, (y0_m + v_m) / ah), lvl));
    return r;
}

// The global read (shared with the Low sheet), A11: two integer-mip fetches
// bracketing lodb, lerped here because the group's sampler has mipmap_filter
// Nearest (columns decoded before the lerp).
fn cloud_profile_global(lon: f32, lat: f32, h: f32, lodb: f32) -> ProfileGlobal {
    var r = ProfileGlobal(false, 0.0, 0.0, 0.0, 0.0, vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    if (!cloud_profile_global_valid()) {
        return r;
    }
    let planet_km = material.params2.z;
    // The global cell, km of arc, from the planet radius (never a constant).
    let global_km = TAU * planet_km / f32(CLOUD_FR_GLOBAL_W);
    let mf = clamp(lodb - log2(global_km), 0.0, f32(CLOUD_FR_GLOBAL_MIPS - 1));
    let m0 = i32(floor(mf));
    let m1 = min(m0 + 1, CLOUD_FR_GLOBAL_MIPS - 1);
    let wm = mf - f32(m0);
    let a = cloud_profile_global_fetch(m0, lon, lat);
    var b = a;
    if (m1 != m0) {
        b = cloud_profile_global_fetch(m1, lon, lat);
    }
    let pair0 = mix(a.pair0, b.pair0, wm);
    let pair1 = mix(a.pair1, b.pair1, wm);
    let col = mix(a.col, b.col, wm);
    r.fp = vec4<f32>(pair0.x, pair0.z, pair1.x, pair1.z);
    r.Gp = vec4<f32>(pair0.y, pair0.w, pair1.y, pair1.w);
    r.Cp = col;
    // Pooled bins (three slab bins each): the same bracketing as the window.
    let hz = h * f32(CLOUD_FR_GLOBAL_NZ);
    let fk = clamp(hz - 0.5, 0.0, f32(CLOUD_FR_GLOBAL_NZ - 1));
    let k0 = i32(floor(fk));
    let k1 = min(k0 + 1, CLOUD_FR_GLOBAL_NZ - 1);
    let wk = fk - f32(k0);
    let kb = clamp(i32(floor(hz)), 0, CLOUD_FR_GLOBAL_NZ - 1);
    let frac_above = clamp(f32(kb + 1) - hz, 0.0, 1.0);
    r.f = mix(cloud_profile_chan(r.fp, k0), cloud_profile_chan(r.fp, k1), wk);
    r.G = mix(cloud_profile_chan(r.Gp, k0), cloud_profile_chan(r.Gp, k1), wk);
    let Gp_kb = cloud_profile_chan(r.Gp, kb);
    // Cp_3 is identically zero; channel w of the column slice carries Tp.
    let Cp_kb = select(cloud_profile_chan(col, kb), 0.0, kb == CLOUD_FR_GLOBAL_NZ - 1);
    let Tp = col.w;
    let dz_pool = 3.0 * (g_cloud_rt - g_cloud_rb) / f32(CLOUD_FR_NZ);
    let above = Cp_kb + Gp_kb * frac_above;
    r.tau_above = g_pf_sigma_v * dz_pool * above;
    r.tau_below = g_pf_sigma_v * dz_pool * max(Tp - above, 0.0);
    r.ok = true;
    return r;
}

// The tap with the level walk (A13): at most TWO window fetches (La, Lb)
// plus the global, hand-off weights summing to 1 by construction. Returns
// not-ok (w_pf = 0 for this sample) when nothing baked covers it.
fn cloud_profile_tap(dirp: vec3<f32>, h: f32, lodb: f32, knob: i32) -> ProfileTap {
    var r = ProfileTap(false, 0.0, 0.0, 0.0, 0.0, 0.0);
    if (material.params2.z < 0.5) {
        return r;
    }
    // The lattice is angular: the lines cloud_v2_body keys its cells on,
    // minus its cos-scaling of longitude.
    let lat = asin(clamp(dirp.y, -1.0, 1.0));
    let lon = atan2(-dirp.z, dirp.x);
    var lv = clamp(lodb - CLOUD_FR_LOD0, 0.0, f32(CLOUD_FR_LEVELS - 1));
    if (knob >= CLOUD_FR_KNOB_FORCE0 && knob <= CLOUD_FR_KNOB_FORCE5) {
        lv = f32(knob - CLOUD_FR_KNOB_FORCE0);      // forced: one level, no blend
    }
    var La = i32(floor(lv));
    var Lb = min(La + 1, CLOUD_FR_LEVELS - 1);
    var wl = lv - f32(La);
    if (knob == CLOUD_FR_KNOB_HARD) {
        wl = 0.0;                                     // HARD: floor level only
    }
    // The arithmetic walk (no fetch): the first level >= La whose window
    // contains the sample; likewise from Lb.
    La = cloud_profile_walk(La, lon, lat);
    Lb = cloud_profile_walk(Lb, lon, lat);
    if (La >= Lb) {
        Lb = La;                                      // both walks landed on one level
        wl = 0.0;
    }
    var ra = ProfileLevel(false, 0.0, 0.0, 0.0, 0.0, 1.0);
    var rb = ProfileLevel(false, 0.0, 0.0, 0.0, 0.0, 1.0);
    if (La < CLOUD_FR_LEVELS) {
        ra = cloud_profile_level(La, lon, lat, h);
    }
    if (Lb < CLOUD_FR_LEVELS && Lb != La) {
        rb = cloud_profile_level(Lb, lon, lat, h);
    }
    // Hand-off weights: La's edge band hands to rb when rb was fetched, else
    // to the global; rb's edge band hands to the global. HARD: ea = eb = 0
    // (the walk already guarantees the fetched level contains the sample).
    var ea = select(1.0, ra.w_edge, ra.ok);
    var eb = select(1.0, rb.w_edge, rb.ok);
    if (knob == CLOUD_FR_KNOB_HARD) {
        ea = 0.0;
        eb = 0.0;
    }
    var share_a = 1.0 - wl;
    var share_b = wl;
    var w_g = 0.0;
    if (La == CLOUD_FR_LEVELS) {
        share_a = 0.0;                                // both walks ended at the global
        share_b = 0.0;
        w_g = 1.0;
    }
    var w_ra = share_a * (1.0 - ea);
    let handed_a = share_a * ea;
    if (rb.ok) {
        share_b = share_b + handed_a;
    } else {
        w_g = w_g + handed_a;
    }
    var w_rb = share_b * (1.0 - eb);
    w_g = w_g + share_b * eb;
    var g = ProfileGlobal(false, 0.0, 0.0, 0.0, 0.0, vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    if (w_g > 0.0) {
        g = cloud_profile_global(lon, lat, h, lodb);
    }
    if (w_g > 0.0 && !g.ok) {
        // Global wanted but not baked yet: renormalize onto the windows, or
        // abstain when there are none.
        let wsum = w_ra + w_rb;
        if (wsum <= 0.0) {
            return r;
        }
        w_ra = w_ra / wsum;
        w_rb = w_rb / wsum;
        w_g = 0.0;
    }
    r.f = w_ra * ra.f + w_rb * rb.f + w_g * g.f;
    r.G = w_ra * ra.G + w_rb * rb.G + w_g * g.G;
    r.tau_above = w_ra * ra.tau_above + w_rb * rb.tau_above + w_g * g.tau_above;
    r.tau_below = w_ra * ra.tau_below + w_rb * rb.tau_below + w_g * g.tau_below;
    // Channel 11 = level / 6 (window edges visible).
    r.level = w_ra * f32(La) + w_rb * f32(Lb) + w_g * f32(CLOUD_FR_LEVELS);
    r.ok = true;
    return r;
}

// ── ELEMENT SIZES (contract "Element sizes"), in KILOMETRES ──
// The clumped-medium law (A7) treats the profile share as a field of cloud
// ELEMENTS: overlap is total inside one element (a ray through a cumulus
// sees its whole depth, not a mean) and random beyond it. It needs the
// element's horizontal and vertical size per family. Built families
// (cv2_arch_index(tc) >= 0): the archetype's own geometric-mean width and
// the squashed height (cv2_elem_table_m, 41-cloud-bodies.wgsl), the vertical
// size capped at the regime's band height. Thin families (cirrus, altocu,
// index < 0): CLOUD_FR_ELEM_THIN_KM across and the band height tall (a sheet
// is one element vertically). Returns (L_h, L_v) in km; callers convert to
// their own units (the march: km * g_cloud_upkm; the Low sheet: km as is).
fn cloud_fr_elem_km(tc: f32, reg: CloudRegime, slab_km: f32) -> vec2<f32> {
    let band_km = max((reg.h_hi - reg.h_lo) * slab_km, 1.0e-3);
    let arch_i = cv2_arch_index(tc);
    if (arch_i < 0) {
        return vec2<f32>(CLOUD_FR_ELEM_THIN_KM, band_km);
    }
    let e_km = cv2_elem_table_m(arch_i) * 0.001;
    return vec2<f32>(max(e_km.x, 1.0e-3), max(min(e_km.y, band_km), 1.0e-3));
}

// ── THE ELEMENT LAW (A7), one sample's profile-share transmittance ──
// tau_elem = sigma_v * D_in * L_elem is the optical depth of ONE element
// along the ray (D_in = G / f, the in-cloud density; L_elem the element's
// chord for this ray direction, 1 / (c_v / L_v + c_h / L_h)); a fraction f of
// the ground is covered, so one element-length of travel transmits
// 1 - w * f * (1 - exp(-tau_elem)); a segment of length seg is seg / L_elem
// element-lengths, and random overlap between elements makes the exponents
// ADD, so the result is a property of the medium and not of the step (one
// step of S and k steps of S / k give the same product by construction; the
// v1 form failed exactly that). Returns T_pf in [0, 1].
fn cloud_fr_t_pf(w_pf: f32, f: f32, D_in: f32, sigma_v: f32, l_h: f32, l_v: f32, c_v: f32, seg: f32) -> f32 {
    let c_h = sqrt(max(1.0 - c_v * c_v, 0.0));
    let l_elem = 1.0 / max(c_v / l_v + c_h / l_h, 1.0e-9);
    let tau_elem = sigma_v * D_in * l_elem;
    let per_elem = max(1.0 - w_pf * f * (1.0 - exp(-tau_elem)), 0.0);
    return pow(per_elem, max(seg / l_elem, 1.0e-6));
}

// map_diag channels 10 / 11 / 12: the profile share w_pf, the blended level
// / 6, and the profile fraction pf.f, each accumulated with the colour's own
// weights (trans * a_i) exactly as g_march_src_acc, so they read as what the
// pixel sees. Reset beside the other march accumulators in cloud_march_core.
var<private> g_march_pf_acc: f32 = 0.0;
var<private> g_march_lvl_acc: f32 = 0.0;
var<private> g_march_frac_acc: f32 = 0.0;

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
    g_march_sun_acc = 0.0;
    g_march_amb_acc = 0.0;
    g_march_src_acc = 0.0;
    // The far rung's channels (increment 4, A17): 10 / 11 / 12.
    g_march_pf_acc = 0.0;
    g_march_lvl_acc = 0.0;
    g_march_frac_acc = 0.0;
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
    // Per-pixel regime split (v0.1244): content beyond the caller's
    // ownership range belongs to the octa map. Abstain before stepping when
    // the slab ENTRY is already past it; otherwise clamp the far end so
    // this ray only pays for (and only claims, via g_march_first_t feeding
    // the composite's distance key) the content it owns. Local units:
    // 1 unit = 1 planet radius; km = t / g_cloud_upkm.
    // NOTE (v0.1282 panel audit): nothing writes g_march_max_km today (it
    // stays at its 1.0e9 default), so this clamp is inert; it is kept as the
    // hook the octa-map ownership split was designed around.
    let max_t = g_march_max_km * g_cloud_upkm;
    if (m0 > max_t) {
        return vec4<f32>(0.0);
    }
    m1 = min(m1, max_t);
    // ── THE HORIZON SEAM (v0.1233) ──
    //
    // Operator screenshots at 5.7 and 6.2 km show a hard horizontal line across
    // the frame, smoother cloud above it and sharper, grainier cloud below.
    //
    // It is the CLOUD BASE SHELL horizon. From 5.7 km it sits 2.20 degrees below
    // horizontal and from 6.2 km 2.31 degrees, which is exactly where the line
    // appears. Across it the marched segment jumps discontinuously, because a ray
    // just below the tangent is clipped where it enters the base shell while a ray
    // just above it runs on to the far side of the top shell: 245 km versus 619 km
    // at 5.7 km altitude, a 375 km step at the tangent.
    //
    // That matters because the step law is SEGMENT-RELATIVE (dt_seg = seg / 48)
    // and the sampling mip is derived from the step. A 2.53x jump in step is 1.34
    // mip levels of detail, appearing instantly along one line - coarser above
    // where the segment is long, finer below where it is short, which is the way
    // round the operator describes.
    //
    // The cure is to stop letting a WHOLE-RAY property set PER-SAMPLE detail.
    // This keeps the unclipped top-shell chord for the step budget, which is
    // continuous in d2 and therefore across the tangent, while the march itself
    // still stops at the clipped m1. Cost is unchanged or lower: the unclipped
    // chord is the longer of the two, so steps below the line get no finer than
    // they already were above it.
    let seg_step = m1 - m0;
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
    // Continuous midpoint (v0.1233). This picks the cloud REGIME - the family,
    // its opacity, extinction, tint and band heights - from the direction at the
    // segment midpoint. Using the CLIPPED segment made that direction jump across
    // the base-shell tangent, so the cloud FAMILY could change along one screen
    // row. That is a far coarser discontinuity than a mip step, and it is the
    // dominant half of the horizon seam.
    // ── THE ROSETTE (v0.1282, panel finding, reproduced offline at r 0.66) ──
    // seg_step is the UNCLIPPED top-shell chord (kept for the step budget,
    // continuous across the base-shell tangent). For a camera inside the
    // slab looking down, that chord runs THROUGH THE PLANET (~2R) and its
    // midpoint is ~90 degrees around the globe in the pixel own screen
    // azimuth: every near-nadir ray read the cloud FAMILY of the far
    // hemisphere, so the type noise (8-19 degree cells) printed as wedges
    // radiating from the nadir - band bases above the camera in some sectors
    // (see-through slivers), other families in others - the starburst at the
    // feet that survived every march, dither, shape and lighting toggle and
    // was absent only on Low, which has no regime. Fix: the lookup point is
    // the segment midpoint but never farther than two slab thicknesses down
    // the ray. Short segments (down-looks) are unchanged and local; at the
    // base-shell tangent both neighbours are far longer than the cap, so the
    // v0.1233 continuity holds (the worst jump is under the cap, ~0.1 deg
    // on the sphere, far inside one type cell).
    let reg_reach = 4.0 * (g_cloud_rt - g_cloud_rb);
    let mid_dir = normalize(ro + rd * (m0 + min(m1 - m0, reg_reach) * 0.5));
    // The type coordinate is kept (increment 4): the far rung's element
    // sizes are per family, so the ray's own tc names the archetype. The
    // regime itself is the same call it always was.
    let tc_ray = cloud_type_coord(mid_dir, t, seed);
    let reg = cloud_regime(tc_ray);
    // Freeze the v2 body's rind for this ray (see g_v2_foot_m): the ray's
    // own footprint at the segment midpoint, in metres. Every density
    // call in this invocation - view samples AND all eight sun-shadow
    // taps - now sees ONE body scale.
    // ── THE HORIZON SEAM, THE ACTUAL CHANNEL (v0.1233) ──
    //
    // This midpoint is where the seam came from, and it took a wrong guess
    // first: the segment-relative STEP law was blamed, but the step is identical
    // on both sides of the tangent for the first ~109 km of every ray. This is
    // the line that jumps for EVERY sample.
    //
    // seg is clipped at the base shell, so it steps 245 -> 619 km across the
    // base-shell tangent (2.201 degrees below horizontal at 5.7 km altitude,
    // 2.315 at 6.2 km - exactly where the operator photographed a hard line).
    // The midpoint therefore steps by the same 2.53x, and it is frozen into the
    // per-ray footprint that sets the rind AND the displacement mip - so the
    // surface DETAIL of every cloud on the ray changes by 1.34 mip levels along
    // one screen row. Coarser above where the segment is long, finer below where
    // it is short, which is the way round it was reported.
    //
    // seg_step is the UNCLIPPED top-shell chord, continuous in d2 and therefore
    // across the tangent. The march still stops at the clipped m1; only the
    // detail scale is taken from the continuous one.
    // ── THE CHORD MUST NOT SET THE DETAIL SCALE (v0.1267) ──
    // This froze the per-ray footprint at the segment MIDPOINT, and the
    // comment above admits what that costs: "the surface DETAIL of every
    // cloud on the ray changes by 1.34 mip levels along one screen row.
    // Coarser above where the segment is long, finer below where it is
    // short." That IS a radial gradient - and INSIDE the deck it is the
    // whole term, because m0 collapses to ~0 in every direction while
    // the chord runs from a few km straight down to hundreds of km near
    // the horizon. The operator: "It still exists very strongly while
    // inside the cloud layer... I don't understand why it keeps pinching
    // at the bottom." This is why.
    //
    // The chord tells you how much SLAB the ray crosses; it does not
    // tell you where the visible surface is, which is what the detail
    // scale should track. Capping its contribution at a cloud-scale
    // distance keeps the useful part (a nearby surface is resolved
    // finely) and drops the part that only encodes the viewing angle.
    // OUTSIDE the deck this changes almost nothing - m0 dominates by
    // orders of magnitude there, which is why the artifact was always
    // strongest inside. The per-RAY freeze itself is preserved: the eye
    // and all its sun taps still shade one surface (the v0.1234 rule).
    // ── THE CHORD IS GONE FROM THE DETAIL SCALE ENTIRELY (v0.1268) ──
    // v0.1267 CAPPED the chord contribution instead of removing it, which
    // bounded the angular sweep but did not end it: from nadir out to the
    // angle where the cap engages the footprint still grew smoothly with
    // viewing angle, and AT the cap it stopped - a bowl with a rim, both
    // centred on the view axis. Measured in the rig at the operator state
    // (3.4 km inside the deck, down-look): the COVERAGE channel is clean
    // and round while the DIRECT SUN channel carries the radial slivers.
    // That split is the proof of where the term acts - the view density
    // call already takes its mip per sample from lodb (an honest
    // camera-to-sample distance), so only the frozen value the eight sun
    // taps read still carried the chord.
    //
    // This is now just a seed for anything sampled before the first march
    // step; the loop overwrites it per sample from that sample own
    // footprint, right beside g_v2_disp_lod, so the sun taps shade the
    // surface the eye sees at THAT sample (the v0.1234 rule, kept) with
    // no angular term at all.
    // Bit 2 of the dev pad restores the old chord-frozen scale so ONE run
    // can capture both sides (the rig cannot A/B across builds honestly -
    // see cloud_clock_pin).
    // Dev pad bits, each tested as a BIT. Magnitude tests ("w < 1.5",
    // "w >= 3.5") break the moment a higher bit is added - that already
    // caught the shape-frame flag once this arc.
    let chord_foot = fract(camera.light7_color.w * 0.125) >= 0.5;
    let world_shape_lod = fract(camera.light7_color.w * 0.0625) >= 0.5;
    // Bit 5 (v0.1271 experiment): UNIFORM STEP - the march step depends on
    // distance from the camera only, never on the angle between the ray
    // and the local vertical. The step-count diagnostic prints a pinwheel
    // about nadir because dt_vert divides by the ray verticality and
    // dt_seg scales with the slab chord; with a cliff-like density edge the
    // estimator MEAN depends on step spacing, so the converged image
    // inherits that pinwheel and no look setting can remove it.
    let uniform_step = fract(camera.light7_color.w * 0.015625) >= 0.5;
    // Fixed step in METRES (light6_color.z, showcase cloud_step_m; 0 = off).
    // The distance-only law above is still nadir-anchored on a down-look
    // (distance to the deck varies with angle from nadir), so the honest
    // test of estimator bias is a step that is the same everywhere.
    let fixed_step_m = camera.light6_color.z;
    let fixed_step = uniform_step && fixed_step_m > 0.0;
    let fixed_dt = fixed_step_m * 0.001 * g_cloud_upkm;
    // ── THE SAMPLE-ANCHORED MARCH (v0.1272, dev pad bit 7) ──
    // The v0.1271 assessment (1-D twins of this loop) found the shipped
    // march biased to FIRST order in the step h: the march position t_cur
    // and the sample position tm were two different variables, and every
    // endpoint rule mixed them - the SDF stride was measured at tm and
    // spent from t_cur (overshoot up to a full step, first accepted sample
    // 311 +- 280 m deep, 39-76% of 400 m clouds missed at 1.5-6 km), the
    // coarse-entry backtrack rewound to the step start not the last clear
    // sample, the entry trapezoid ran against dens_last = 0, the first-step
    // rewind landed behind the eye, and the exit half-step was dropped.
    // Inside the deck h = seg/16 = 188 m / cos(theta), so each of those is a
    // nadir pinwheel in the MEAN, and the frozen per-pixel jitter makes each
    // a static coin flip: the rosette, the glittering edges and the dark
    // pepper inside bright bodies are one cause. With est on, the state IS
    // the last sample: the next sample is t_cur + dt, the stride is
    // measured and spent from the same point, entry is localised by a
    // 2-tap bisection instead of a rewind, the eye-inside-cloud case is
    // primed with one tap at m0, and the trapezoid integrates exactly
    // between consecutive samples. Twin: bias -0.43 -> -0.02, sd 0.25 ->
    // 0.087, identical across approach distances.
    let est = fract(camera.light7_color.w * 0.00390625) >= 0.5;
    // STEP ECONOMY (perf increment 2, v0.1288): strength 0..1 in
    // light7_color.y (showcase cloud_step_eco, F10 slider). Floors the
    // interior step at half the sample footprint (so far rays stop taking
    // 22 m steps at 300 km) and relaxes the step as the ray goes opaque.
    let eco = clamp(camera.light7_color.y, 0.0, 1.0);
    // Bit 8: band-limit the domain warp to its own tile (see
    // 41-cloud-bodies.wgsl) and refine at surfaces to rind/4.
    let warp_bl = fract(camera.light7_color.w * 0.001953125) >= 0.5;
    // ── ISOTROPIC NEAR STEP + BOUNDED FAR ANGLE TERM (v0.1274, bit 9) ──
    // Design items 1B + 1C. Inside 27 km the step carries NO term in the
    // angle to the local vertical except distance itself: dt = clamp(cone,
    // 30 m, slab_h * VERT_FRAC). The seg/16 near floor and the seg/48 chord
    // floor are gone on this path - the winking they were added for was
    // the stride lag the sample-anchored march removed, and the chord
    // floor was a 48-sample cost floor that guaranteed nothing (for an
    // in-deck down-look seg_step is the through-planet chord, so it never
    // bound). Beyond 27 km an angle term is unavoidable: an orbital nadir
    // ray needs <= 928 m radial steps while a limb ray with a 780 km slab
    // chord needs >= 4 km to fit the iteration cap, so the far ceiling is
    // 928 m / max(r_rate, 0.1) - a 10x range instead of the shipped 20x,
    // the reference family (sample count varies ~2x with angle) - blended
    // in over 27-54 km. Inside the deck t > 27 km exists only on rays
    // more than 81 deg from vertical, the horizon band, never a ring
    // about the nadir.
    let iso_step = fract(camera.light7_color.w * 0.0009765625) >= 0.5;
    g_v2_foot_m = select(
        m0 * pix_ang / max(g_cloud_upkm, 1.0e-9) * 1000.0,
        (m0 + seg_step * 0.5) * pix_ang / max(g_cloud_upkm, 1.0e-9) * 1000.0,
        chord_foot,
    );
    // Same freeze for the displacement mip, in the same units as `lodb`
    // (log2 of the footprint in km). The rind was frozen back in v0.1213 for
    // exactly this reason and the displacement was left behind.
    g_v2_disp_lod = log2(max(g_v2_foot_m * 0.001, 1.0e-4));
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
    let step_near = select(
        min(slab_h * CLOUD_STEP_BAND_FRAC,
            max(seg * (1.0 / 16.0), 30.0 * g_cloud_upkm * 0.001)),
        // uniform: a fixed 30 m floor; the cone term owns the near field
        30.0 * g_cloud_upkm * 0.001,
        uniform_step || iso_step);
    // Per-ray physical extinction (phase 3): per-family sigma converted to
    // drawn units. Replaces the global CLOUD_HI_SIGMA_KM.
    // Dev knob (v0.1279): extinction multiplier in light5_color.y (showcase
    // cloud_sigma_mul, F10 slider); 0 = off. The transparency test at bm-12.
    let sigma_mul = select(1.0, camera.light5_color.y, camera.light5_color.y > 0.0);
    let sigma_v = (reg.ext_km / g_cloud_upkm) * sigma_mul;
    // ── THE FAR RUNG (perf increment 4): per-ray constants ──
    // Knob 0 (the A/B twin): this one uniform read is the ONLY new code
    // that executes; every profile branch below tests it or the w_pf it
    // derives, so the march is bit-identical to v0.1288. Knob != 0: the
    // element sizes of this ray's family in drawn-shell units (the element
    // law needs them per sample), and one bin's height for the vertical
    // element floor.
    let knob = cloud_profile_knob();
    var fr_l_h = 1.0;
    var fr_l_v = 1.0;
    var fr_dz = 1.0;
    if (knob != CLOUD_FR_KNOB_OFF) {
        let e_km = cloud_fr_elem_km(tc_ray, reg, slab_h / max(g_cloud_upkm, 1.0e-9));
        fr_l_h = e_km.x * g_cloud_upkm;
        fr_l_v = e_km.y * g_cloud_upkm;
        fr_dz = slab_h / f32(CLOUD_FR_NZ);
    }
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
    // Distance to the constructed surface at the PREVIOUS sample, in metres.
    // Follows the same one-step lag as dens_prev: the step has to commit
    // before the new sample is known, and the jitter decorrelates the lag.
    var sdf_prev = 1.0e9;
    // Sample-anchored bookkeeping: the last sample position (the trapezoid
    // integrates from it) and the last CLEAR sample (the entry depth).
    var t_last = m0;
    var t_last_clear = m0;
    g_march_first_depth_m = 0.0;
    if (est && m0 <= 1.0e-6) {
        // The eye is inside the slab: one tap AT the eye primes the state so
        // the first step is already MFP-refined and no entry ever fires at
        // t = 0 - the behind-the-eye integration the operator sees as the
        // rosette being strongest inside clouds.
        let p0 = ro;
        let dirp0 = normalize(p0);
        let foot0 = max(step_near * 0.25, 1.0e-6);
        let lodb0 = log2(max(foot0 / g_cloud_upkm, 1.0e-4));
        g_v2_disp_lod = select(lodb0, CLOUD_V2_SHAPE_LOD_WORLD, world_shape_lod);
        let wlod0 = max(log2(max(foot0 / g_cloud_upkm / 27.8, 1.0)), 0.0);
        let wa0 = clamp(
            cloud_alpha_from_field(
                cloud_weather_adv(dirp0, t, seed, wind_ang, wlod0), coverage)
                + reg.cover_bias, 0.0, 1.0);
        dens_prev = cloud_density_hi(p0, t, seed, wa0, reg, 1.0, 1.0, 1.0, lodb0).x;
        sdf_prev = g_v2_sdf_m;
    }
    for (var i = 0; i < CLOUD_STEP_ITER_CAP; i = i + 1) {
        if (t_cur >= m1) {
            break;
        }
        g_march_iters = f32(i + 1);
        // Footprint-proportional step with a VERTICAL ceiling (see
        // CLOUD_STEP_VERT_FRAC), an interior MFP ceiling (increment 10),
        // and a segment-density floor, clamped to what remains of the
        // segment so the march reaches m1 exactly (no unsampled tail).
        let p_cur = ro + rd * t_cur;
        // Step economy (increment 2): half the sample footprint, the floor no
        // interior rule may step below; 0 with the knob off.
        let foot_floor = 0.5 * t_cur * pix_ang * eco;
        let r_rate = abs(dot(normalize(p_cur), rd));
        let dt_vert = max(
            step_near,
            slab_h * CLOUD_STEP_VERT_FRAC / select(max(r_rate, 0.05), 1.0, uniform_step),
        );
            let dt_seg = select(max(step_near, seg_step * CLOUD_STEP_SEG_FRAC), 1.0e9, uniform_step);
        var dt = min(
            min(
                max(step_near, t_cur * pix_ang * CLOUD_STEP_CONE_K),
                min(dt_vert, dt_seg),
            ),
            m1 - t_cur,
        );
        if (dens_prev > CLOUD_STEP_INTERIOR_GATE) {
            let dt_mfp = CLOUD_STEP_TAU_MAX / (sigma_v * dens_prev);
            dt = min(dt, max(max(dt_mfp, slab_h * 0.002), foot_floor));
        }
        // Skirt floor (est): dt_mfp = 0.75/(sigma*dens) is 167 m at dens 0.1
        // and 333 m at 0.05 - it refines cores and abandons exactly the
        // skirt the eye sees. Hold the step to 0.5% of the slab there.
        if (est && dens_prev > CLOUD_STEP_INTERIOR_GATE && dens_prev < 0.3) {
            dt = min(dt, max(slab_h * 0.005, foot_floor));
        }
        // ── SDF-GUIDED STEP (v0.1230): stride the gaps, refine at surfaces ──
        //
        // The march was hopping a fixed 495 m through clear air while hunting a
        // cloud edge 90 m thick. At every silhouette pixel that is a coin flip -
        // did the hop land inside the edge or step clean over it - and the answer
        // changes as the camera moves. That coin flip IS the grain the operator
        // has been calling TV static, and no denoiser can average away a signal
        // that is genuinely random per frame.
        //
        // The constructed body already computes a real distance to its surface
        // and threw it away. Now the step uses it in both directions: far from
        // any cloud, stride the whole safe distance in one hop (cheaper than the
        // fixed step); within reach of a surface, refine to a fraction of the
        // rind so the edge is measured rather than guessed.
        //
        // The safety margin is subtracted because the field is NOT a strict
        // distance bound once shaped: surface displacement can push the boundary
        // out by up to DISP + DISP2, the rind widens it further, and the domain
        // warp bends the space it is measured in. Stride less than the true
        // distance and the worst case is wasted work; stride more and clouds get
        // skipped, which is the bug being fixed.
        if (sdf_prev < 1.0e8) {
            // Erosion amplitude included (v0.1242): the one-sided carve only
            // deepens the surface, so the outer approach stays safe, but the
            // refine/backtrack now hunts a boundary that can recede by the
            // full carve depth - keep the margin conservative.
            let margin_m = CLOUD_V2_RIND_M + CLOUD_V2_DISP_M + CLOUD_V2_DISP2_M
                + g_v2_warp_m + CLOUD_V2_ERODE_M + CLOUD_V2_ERODE2_M * 0.4;
            // A lean is not an isometry: the SDF is measured in the leaned
            // domain, so the clear-air stride is bounded by 1/sqrt(1+s^2).
            let lean_k = inverseSqrt(1.0 + camera.light6_color.w * camera.light6_color.w);
            let safe_m = sdf_prev * lean_k - margin_m;
            let to_draw = 0.001 * g_cloud_upkm;
            if (safe_m > 0.0) {
                dt = max(dt, safe_m * to_draw);
            } else {
                // Inside or near a body: a quarter-rind step, but never below the
                // sample footprint (increment 2): a 22 m step at 300 km is waste.
                dt = min(dt, max(CLOUD_V2_RIND_M * select(0.5, 0.25, est || warp_bl) * to_draw, foot_floor));
            }
            dt = min(dt, m1 - t_cur);
        }
        // ── FINAL-STEP TAIL INTEGRATION (v0.1252.5) ──
        // The iteration cap used to END the march mid-segment, and WHERE
        // it ended is a function of slant angle - screen radius from the
        // aim point - so the truncation bias converged into a faint
        // cursor-locked radial pattern (the grazing iteration-cap tail).
        // The v0.1252.4 cure was WORSE than the disease: an every-step
        // budget stride `max(dt, remaining/iters_left)` forced
        // kilometre strides from the FIRST sample whenever the slab exit
        // was far (always, inside the deck), overriding the MFP/SDF
        // refinement - coarse near-steps are exactly the mechanism of
        // the v0.1241 melted flower, and the operator's night captures
        // showed it back in force. REVERTED. The correct O(1) form:
        // only the LAST budgeted step stretches to cover whatever tail
        // remains - one coarse sample (its footprint self-selects a
        // deep mip via `foot = max(.., dt * 0.25)`) instead of an
        // unsampled tail, and the near sampling is untouched.
        if (iso_step) {
            let t_km = t_cur / max(g_cloud_upkm, 1.0e-9);
            let far = smoothstep(27.0, 54.0, t_km);
            let ceil_near = slab_h * CLOUD_STEP_VERT_FRAC;
            let ceil_far = ceil_near / max(r_rate, 0.1);
            let dt_ceil = mix(ceil_near, ceil_far, far);
            let near_floor = 30.0 * g_cloud_upkm * 0.001;
            var dt_iso = clamp(t_cur * pix_ang * CLOUD_STEP_CONE_K, near_floor, dt_ceil);
            // Keep the interior MFP ceiling, the skirt floor and the SDF
            // refine/stride: they are geometry- and density-driven, not
            // angle-driven, and 1A depends on them.
            dt = min(min(dt, dt_iso), m1 - t_cur);
            if (sdf_prev < 1.0e8) {
                let margin_i = CLOUD_V2_RIND_M + CLOUD_V2_DISP_M + CLOUD_V2_DISP2_M
                    + g_v2_warp_m + CLOUD_V2_ERODE_M + CLOUD_V2_ERODE2_M * 0.4;
                let safe_i = sdf_prev - margin_i;
                if (safe_i > 0.0) {
                    dt = max(dt, min(safe_i * 0.001 * g_cloud_upkm, m1 - t_cur));
                }
            }
        }
        if (fixed_step) {
            dt = min(fixed_dt, m1 - t_cur);
        }
        // Deep relaxation (increment 2), after every other dt rule: below
        // transmittance 0.5 the step grows to 2x at full opacity; those
        // samples contribute the least and the exit at 0.005 comes sooner.
        dt = min(dt * (1.0 + eco * clamp((0.5 - trans) * 2.0, 0.0, 1.0)), m1 - t_cur);
        if (i == CLOUD_STEP_ITER_CAP - 1) {
            dt = m1 - t_cur;
        }
        // The jitter places the sample inside its own step - same
        // decorrelation role it had in the exp-spaced form.
        // est: the sample IS the march state. The ladder-phase jitter stays
        // on the first step only; every later sample sits exactly dt after
        // the previous one, so the trapezoid interval is exact.
        var tm = t_cur + dt * jitter;
        if (est) {
            tm = t_cur + dt * select(1.0, max(jitter, 1.0e-3), i == 0);
        }
        // LADDER-PHASE JITTER (v0.1242, the melted-flower fix). Sampling
        // inside the step is not enough: the step ENDPOINTS themselves were
        // one deterministic comb anchored at m0 and shared by every ray, so
        // the integer step COUNT is a staircase in screen radius - and on a
        // flat deck every +-1 tread prints as a visible ring centred at the
        // nadir (proven by the iteration-count diagnostic: its contours
        // matched the operator's flower rings exactly; the rings survived
        // both jitter-hash fixes because no per-sample jitter can move the
        // comb). Advancing the FIRST step by the jittered fraction phase-
        // shifts the entire comb per pixel/frame; the count boundary then
        // dithers across a full step and the resolve's accumulation
        // averages the treads away.
        if (est) {
            t_cur = tm;
        } else if (i == 0) {
            t_cur = t_cur + dt * max(jitter, 1.0e-3);
        } else {
            t_cur = t_cur + dt;
        }
        let seg_len = tm - t_last;
        t_last = tm;

        let p = ro + rd * tm;
        let dirp = normalize(p);
        // Footprint FIRST (hoisted, increment 11b) - the weather tap now
        // band-limits itself with the same footprint the volume taps use.
        // Weather-map texel = 27.8 km at mip 0.
        let foot = max(tm * pix_ang, dt * 0.25);
        let lodb = log2(max(foot / g_cloud_upkm, 1.0e-4));
        // ── THE FAR RUNG (perf increment 4): the profile tap ──
        // Knob 0 (no atlas, or the A/B twin): a branch not taken, w_pf = 0,
        // full = true - the march below is bit-identical to v0.1288. Knob !=
        // 0: the planet-fixed profile (fraction f, mean density G, the
        // columns above and below) is read through the lattice / walk
        // arithmetic above and takes the share w_pf of this sample:
        //   * w_pf < 1 - 1e-4 ("full"): the weather tap, cloud_density_hi,
        //     the sun ladder / cache, the entry bisection, the backtrack and
        //     the refinements run as today on the full-field share;
        //   * w_pf >= 1 - 1e-4: none of them run (dens = 0, dens_prev = 0,
        //     sdf_prev = the no-body sentinel so the step is the base law).
        //     That is where the orbit cost goes.
        // The transmittance takes the element law T_pf (A7) on the profile
        // share and the sun / burial columns and the relief terms mix by w_pf
        // (contract "March side (WGSL)").
        var pf = ProfileTap(false, 0.0, 0.0, 0.0, 0.0, 0.0);
        var w_pf = 0.0;
        if (knob != CLOUD_FR_KNOB_OFF) {
            // The same dithered coordinate the v2 fade uses (cloud_carve).
            let lodf = lodb + g_lod_jitter * 0.35;
            // Normalized slab height of this sample: the contract's `h`. A
            // later `let h` in this same scope holds the same value for the
            // lighting block, hence the suffix here.
            let h_pf = clamp((length(p) - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
            g_pf_sigma_v = sigma_v;
            pf = cloud_profile_tap(dirp, h_pf, lodb, knob);
            if (knob >= CLOUD_FR_KNOB_FORCE0 && knob <= CLOUD_FR_KNOB_FORCE5) {
                w_pf = 1.0;
            } else if (knob == CLOUD_FR_KNOB_HARD) {
                w_pf = select(0.0, 1.0, lodf >= -1.0);
            } else {
                w_pf = smoothstep(CLOUD_FR_LOD_LO, CLOUD_FR_LOD_HI, lodf);
            }
            w_pf = select(0.0, w_pf, pf.ok);
        }
        // Does the full field still own any of this sample?
        let full = w_pf < 1.0 - 1.0e-4;
        // The profile share's in-cloud columns (optical depths above and
        // below the sample INSIDE the cloud fraction, capped at the slab):
        // computed with the share so the lighting block can mix them in.
        var tau_above_in = 0.0;
        var tau_below_in = 0.0;
        var D_in_pf = 0.0;
        if (w_pf > 0.0) {
            let f_floor = max(pf.f, CLOUD_FR_F_EPS);
            let r_pf = length(p);
            tau_above_in = min(pf.tau_above / f_floor, sigma_v * max(g_cloud_rt - r_pf, 0.0));
            tau_below_in = min(pf.tau_below / f_floor, sigma_v * max(r_pf - g_cloud_rb, 0.0));
            D_in_pf = clamp(pf.G / f_floor, 0.0, 1.0);
        }
        // Surface-detail mip frozen PER SAMPLE, not per ray (v0.1234). The
        // per-ray freeze took the footprint at the SEGMENT MIDPOINT, and inside
        // the slab the unclipped chord runs ~600 km - so a cloud 500 m from the
        // camera was surfaced at the mip of a point 300 km away, which is why
        // the NEAREST clouds were the smooth melted ones while mid-distance
        // ones kept their detail (backwards from any honest mip ladder). The
        // freeze exists so the eight sun-shadow taps shade the SAME surface the
        // eye sees; setting it here, from this sample's own footprint, before
        // the view density call, preserves exactly that - the sun march that
        // follows reuses the value - while giving near clouds near detail.
        // ── SHAPE LOD: WORLD-ANCHORED OR CAMERA-ANCHORED (v0.1269 test) ──
        // The comment on g_v2_disp_lod states the invariant plainly:
        // "Displacement is SHAPE, so every evaluation that reaches a given
        // point in the world must agree on it." Assigning it from lodb
        // VIOLATES that - lodb is log2 of the footprint, and the footprint is
        // camera distance times the pixel angle. So a cloud is shaped
        // differently depending on how far away you are standing.
        //
        // That is a nadir-anchored artifact by geometry: looking at a shell,
        // lines of equal distance-to-camera project to circles centred on the
        // point straight below the camera. The fine displacement octave rides
        // CLOUD_V2_INT_LODC = -9.56, whose mip ramp spans roughly 1.7 km to
        // 425 km - precisely the near field - gaining a level per doubling of
        // distance. Clouds at the nadir are nearest, so they get the most
        // shape detail, and it thins with angle. Operator, flying it: "the
        // further the clouds get from my feet the more normal the clouds look.
        // The closer we get to my feet the worse the warping effect becomes."
        g_v2_disp_lod = select(lodb, CLOUD_V2_SHAPE_LOD_WORLD, world_shape_lod);
        // The sun taps read g_v2_foot_m for the body rind. Set it from THIS
        // sample footprint, exactly as disp_lod is, so the eye and its eight
        // sun taps share one body scale that depends on distance to the
        // surface and nothing else (v0.1268; was frozen from the ray chord,
        // which is a function of viewing angle - the rosette in the sun
        // channel).
        if (!chord_foot) {
            g_v2_foot_m = foot / max(g_cloud_upkm, 1.0e-9) * 1000.0;
        }
        let wlod = max(log2(max(foot / g_cloud_upkm / 27.8, 1.0)), 0.0);
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
        // ── NOISE-PATH COMPONENT BISECT (v0.1279, dev pad bits 13-15) ──
        // The bm-12 rosette is in the density field itself (present in
        // coverage alpha, at every volumetric tier, under every march and
        // resolve toggle). One term off at a time.
        // v0.1283: the component bisect is a 3-bit INDEX at bits 13-15
        // (0 none, 1 detail, 2 puff, 3 cell, 4 fray, 5 base drop), one term
        // off at a time; bits 16-17 are free.
        let bis = cloud_bisect_index();
        let detail_amt = select(1.0, 0.0, bis == 1u);
        let puff_amt = select(1.0, 0.0, bis == 2u);
        let cell_amt = select(1.0, 0.0, bis == 3u);
        // (foot/lodb hoisted above the weather tap - increment 11b.)
        // ── THE FULL-FIELD SHARE (increment 4) ── the weather tap, the
        // density call and the side-channel copies run only while the full
        // field owns any of this sample (`full`); at w_pf = 1 the sample is
        // clear air to the march (dens 0, no body, neutral side channels)
        // and the profile share carries it. Knob 0: `full` is always true and
        // every line below is the shipped one.
        var weather_a = 0.0;
        var dc = vec3<f32>(0.0);
        var dens = 0.0;
        // 12f: the view sample's side-channel, copied IMMEDIATELY after the
        // density call - the sun march below re-enters cloud_carve and
        // overwrites these globals. Neutral when the density call is skipped.
        var s_pouch = 0.0;
        var s_v2_w = 0.0;
        var s_v2_ny = 0.0;
        var s_v2_seam = 0.0;
        var s_btop = 1.0;
        var s_carve = 0.0;
        // Hygiene (v0.1275, design 1a): the sun ladder re-enters the body
        // and overwrites g_v2_warp_m with its 6-lobe value; the NEXT
        // iteration reads it in margin_m. Restore the eye value after.
        var s_warp = 0.0;
        var s_sdf = 1.0e9;
        if (full) {
            weather_a = clamp(
                cloud_alpha_from_field(
                    cloud_weather_adv(dirp, t, seed, wind_ang, wlod), coverage)
                    + reg.cover_bias, 0.0, 1.0);
            dc = cloud_density_hi(
                p, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt, lodb);
            dens = dc.x;
            // ── SYNTHETIC CHECKER (dev pad bit 21) ── a known world-space field.
            if (fract(camera.light7_color.w * 0.000000238418579101562) >= 0.5) {
                let alt_m = (length(p) - g_cloud_rb) / max(g_cloud_upkm, 1.0e-9) * 1000.0;
                let lat_r = asin(clamp(dirp.y, -1.0, 1.0));
                let lon_r = atan2(-dirp.z, dirp.x);
                let u = floor(lat_r * 6371.0 / 0.5);
                let vv = floor(lon_r * cos(lat_r) * 6371.0 / 0.5);
                let odd = fract((u + vv) * 0.5) > 0.25;
                let in_slab = alt_m > 0.0 && alt_m < 600.0;
                dens = select(0.0, 1.0, odd && in_slab);
            }
            s_pouch = g_cloud_pouch;
            s_v2_w = g_v2_w;
            s_v2_ny = g_v2_ny;
            s_v2_seam = g_v2_seam;
            s_btop = g_cloud_bandtop;
            s_carve = g_cloud_carve;
            s_warp = g_v2_warp_m;
            s_sdf = g_v2_sdf_m;
        }
        // COARSE-ENTRY BACKTRACK (increment 10, the +45%-dark diagnosis):
        // a law-sized step that lands in dense cloud would accumulate its
        // whole optical depth at ONE deep, dark sample - skipping the
        // bright sunlit rind that dominates what the eye sees (the
        // converged reference resolves that rind; the first cut of this
        // march read 45% darker than it). Nubis-style fix: reject the
        // coarse step, back up, and re-march the span at MFP resolution
        // (dens_prev primes the interior refinement above).
        // The guard is on the bisection STOP width, not on step_near
        // (v0.1276): step_near is capped at 4.5% of the slab (522 m), so at
        // 26 km the 500-700 m cone step never exceeded 2*step_near and the
        // bisection NEVER FIRED - the entry fell through to the trapezoid,
        // whose depth below a flat cloud top is the comb phase, printing
        // concentric contour bands about the nadir on a rain overcast.
        // (Both entry rules belong to the full-field share: `full` is true
        // at knob 0, and a profile-only sample has no field to enter.)
        if (full && est && dens > CLOUD_STEP_INTERIOR_GATE
            && dens_prev <= CLOUD_STEP_INTERIOR_GATE
            && seg_len > 4.0 * (30.0 * 0.001 * g_cloud_upkm))
        {
            // ENTRY LOCALISATION (est): two bisection taps on [last clear
            // sample, this sample] find the crossing to within seg_len/4,
            // then the march restarts FROM the crossing at the MFP step. No
            // rewind, so nothing is integrated behind the last clear sample
            // and no sunlit front is lost. Localisation error h/4 -> mean
            // bias <= h/24 (8-22 m, under the 22 m sunlit skin at 45/km).
            var lo = t_last_clear;
            var hi = tm;
            var dens_hi = dens;
            // Five bisections (v0.1276; was two). Two left the crossing within
            // seg_len/4 - 175-350 m at 26 km - and on a flat cloud top that
            // error, taken modulo the step comb, printed CONCENTRIC CONTOUR
            // BANDS about the nadir (the v0.1242 flower-ring class, exposed
            // by a rain overcast from altitude once the estimator noise was
            // gone). Five taps put the crossing within seg_len/32, under the
            // 22 m sunlit skin, and stop early once the bracket is 30 m.
            let stop_w = 30.0 * 0.001 * g_cloud_upkm;
            // Budget (v0.1276.2): five taps for the FIRST entry on the ray,
            // which sets the visible surface, two for entries behind it
            // (already attenuated, their comb error proportionally less
            // visible). Grazing rays through scattered cumulus cross many
            // entries; five taps on each cost 4x frame time at look-40.
            let n_bis = select(2, 5, first_t < 0.0);
            for (var b = 0; b < n_bis; b = b + 1) {
                if (hi - lo < stop_w) {
                    break;
                }
                let mid = 0.5 * (lo + hi);
                let pm = ro + rd * mid;
                let dm = cloud_density_hi(
                    pm, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt, lodb).x;
                if (dm > CLOUD_STEP_INTERIOR_GATE) {
                    hi = mid;
                    dens_hi = dm;
                } else {
                    lo = mid;
                }
            }
            t_cur = hi;
            t_last = hi;
            dens_prev = dens_hi;
            sdf_prev = g_v2_sdf_m;
            if (first_t < 0.0) {
                first_t = hi;
                g_march_first_depth_m = (hi - t_last_clear) / max(g_cloud_upkm, 1.0e-9) * 1000.0;
            }
            continue;
        }
        if (full && !est && dens > CLOUD_STEP_INTERIOR_GATE
            && dens_prev <= CLOUD_STEP_INTERIOR_GATE
            && sigma_v * dens * dt > CLOUD_STEP_TAU_MAX)
        {
            t_cur = t_cur - dt;
            dens_prev = dens;
            sdf_prev = g_v2_sdf_m;
            continue;
        }
        // Capture the PREVIOUS step's density before dens_prev is
        // advanced - the v0.1258 trapezoid read dens_prev AFTER this
        // assignment, so it averaged a value with itself and was a
        // no-op (which is why it measured a 6% move: noise). The
        // integral needs the far endpoint, so it has to be taken here.
        let dens_last = dens_prev;
        dens_prev = dens;
        // A profile-only sample leaves the no-body sentinel behind so the
        // SDF stride block is bypassed and the next step is the base law.
        sdf_prev = select(1.0e9, g_v2_sdf_m, full);
        if (dens <= CLOUD_STEP_INTERIOR_GATE) {
            t_last_clear = tm;
        }
        // The profile share's own cloud in this sample: the two skips below
        // continue only when BOTH shares are empty (0 at knob 0, so the
        // shipped tests are unchanged).
        let pf_f = w_pf * pf.f;
        // est credits the EXIT half-step: the skip moved below the
        // trapezoid and tests the whole interval, not this endpoint.
        if (!est && dens <= 0.001 && pf_f <= 0.001) {
            continue;
        }
        // ── TRAPEZOID OVER THE STEP (v0.1258, the operator's third
        // layer: "one layer lower that just renders as static on the
        // cloud surface") ──
        // The opacity of a step was a POINT sample of density times the
        // step length. At physical extinction (45/km) a 45 m step is
        // tau 2 - so whether one pixel's jittered sample lands just
        // inside or just outside the density ramp is the difference
        // between opaque and clear, and adjacent pixels disagree at
        // random. That coin flip IS the static on the surface, and it
        // is why the coverage-alpha channel measured GRAINIER than
        // either lighting channel (2.04 vs 0.36).
        // The step is an INTEGRAL, not a sample, and we already hold
        // both endpoints: the trapezoid is the exact integral of a
        // linear ramp, halves the estimator's variance, and costs
        // nothing. It also softens the first step into a cloud (where
        // dens_prev is the clear air outside), which is exactly the
        // binary opaque-or-clear edge the operator has been reporting.
        let dens_i = 0.5 * (dens + dens_last);
        if (est && dens_i <= 0.001 && pf_f <= 0.001) {
            continue;
        }
        // ── TRANSMITTANCE (increment 4, the element law A7) ──
        // The full-field share is Beer-Lambert on the trapezoid density over
        // the step, as shipped (knob 0: exactly the shipped expression). The
        // profile share multiplies in T_pf from cloud_fr_t_pf: a fraction f
        // of ground covered by elements of in-cloud density D_in, total
        // overlap inside one element, random beyond it, so the exponent
        // adds across any subdivision of the step and the result cannot
        // depend on the step law (the G0(e) prove-red). A 30 percent field of
        // slab-tall elements seen from the nadir transmits 0.7, where the
        // plain mean would give exp(-157): the white-veil class.
        let seg_used = select(dt, seg_len, est);
        var a_i = 1.0 - exp(-sigma_v * dens_i * seg_used);
        if (w_pf > 0.0) {
            let c_v = abs(dot(rd, dirp));
            let t_pf = cloud_fr_t_pf(w_pf, pf.f, D_in_pf, sigma_v, fr_l_h, max(fr_l_v, fr_dz), c_v, seg_used);
            a_i = 1.0 - exp(-sigma_v * dens_i * seg_used * (1.0 - w_pf)) * t_pf;
        }
        if (first_t < 0.0) {
            first_t = tm;
            g_march_first_depth_m = (tm - t_last_clear) / max(g_cloud_upkm, 1.0e-9) * 1000.0;
        }

        // Day/night from the sample's own sphere normal (soft terminator).
        let ndl = dot(dirp, sun_local);
        let day = smoothstep(-0.05, 0.3, ndl);
        // ── INCREMENT A1: BURIAL (v0.1280) ── world-space, camera-independent.
        // The column above and below this sample, from the cloud's OWN top
        // (g_cloud_coltop) and the band base, at the envelope density; on the
        // built path also the signed distance inside the body.
        let h_a1 = clamp((length(p) - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
        let slab_a1 = g_cloud_rt - g_cloud_rb;
        let col_above = max(g_cloud_coltop - h_a1, 0.0) * slab_a1;
        let col_below = max(h_a1 - reg.h_lo, 0.0) * slab_a1;
        // Increment C (v0.1282): inside a constructed body the column is
        // the body's OWN geometry at its interior density, not the noise
        // envelope's crown solve at ~0.5. Blended in by the saturation knob
        // times the built weight; sat 0 is the v0.1280 column.
        let sat_a1 = clamp(camera.light5_color.w, 0.0, 1.0) * s_v2_w
            * select(0.0, 1.0, g_v2_sdf_m < 0.0);
        let col_above_b = max(g_v2_top_m - g_v2_up_m, 0.0) * 0.001 * g_cloud_upkm;
        let col_below_b = max(g_v2_up_m, 0.0) * 0.001 * g_cloud_upkm;
        var tau_above = sigma_v * mix(s_carve * col_above,
            max(s_carve * col_above, g_v2_int_dens * col_above_b), sat_a1);
        var tau_below = sigma_v * mix(s_carve * col_below,
            max(s_carve * col_below, g_v2_int_dens * col_below_b), sat_a1);
        var tau_built = sigma_v * max(-g_v2_sdf_m, 0.0) * 0.001 * g_cloud_upkm;
        // ── THE PROFILE SHARE'S BURIAL (increment 4) ── the columns above
        // and below mix toward the profile's IN-CLOUD columns by w_pf (the
        // share renders f of thick cloud plus 1 - f of clear, so the burial
        // of the cloud part is the in-cloud column, never the plain mean
        // that lit a thin haze and read dark: the v0.1233 mismatch); the
        // body-depth term belongs to the full field only.
        if (w_pf > 0.0) {
            tau_above = mix(tau_above, tau_above_in, w_pf);
            tau_below = mix(tau_below, tau_below_in, w_pf);
            tau_built = tau_built * (1.0 - w_pf);
        }
        g_sun_tau_col = tau_above / max(ndl, 0.15);
        g_ms_on = select(0.0, 1.0, fract(camera.light7_color.w * 0.000000238418579101562 * 0.5) >= 0.5);

        // Light march toward the sun + Beer-powder edge darkening. The
        // first two taps see the same eroded density this view sample does
        // (clouds depth increment), so lobes self-shadow.
        // Increment 4: the ladder / cache runs on the full-field share only;
        // the profile share takes the analytic slant column of its in-cloud
        // depth above (tau_pf = tau_above_in / ndl, the same column the march
        // already uses beyond the sun windows), mixed by w_pf. At w_pf = 1
        // the ladder never ran, which is where the orbit cost goes.
        var tau = 0.0;
        if (full) {
            g_deep_sample = select(0.0, 1.0, trans < 0.5);
            tau = cloud_sun_tau(
                p, sun_local, t, seed, weather_a, reg, detail_amt, puff_amt, cell_amt,
                lodb);
            g_v2_warp_m = s_warp;
            g_v2_sdf_m = s_sdf;
        }
        if (w_pf > 0.0) {
            let tau_pf = tau_above_in / max(ndl, 0.15);
            tau = mix(tau, tau_pf, w_pf);
        }
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
        // Beer-powder DELETED for the constructed path (v0.1231). Cloud
        // droplets have a single-scatter albedo above 0.9999 - they scatter
        // essentially every photon they receive - so a cloud edge physically
        // CANNOT be darker than the sky behind it. Ours measured 0.71x, which
        // read as grey mush exactly where a real cloud is at its brightest and
        // most translucent. It is also double-counted: cloud_scatter_energy
        // already evaluates the dual-lobe phase per octave AND carries a
        // two-stream diffusion floor, which is the multiple scattering that
        // powder is an ad-hoc stand-in for. The noise path keeps it, its look
        // being calibrated around it.
        let powder = select(powder_raw, 1.0, material.params.y >= 2.5);
        let pw = mix(powder, 1.0, powder_gate);
        let h = clamp((length(p) - g_cloud_rb) / (g_cloud_rt - g_cloud_rb), 0.0, 1.0);
        // 12f: the VERTICAL column depth above this sample (plane-
        // parallel estimate from the local density and the column's own
        // band top) - what the diffusion floor and ambient shaper are
        // physically governed by. Driving them with the slant SUN path
        // double-counted obliquity (~1.5x too dark at mid sun) and read
        // relief kilometres sideways from where the eye sees it.
        let slab_h_d = g_cloud_rt - g_cloud_rb;
        // v0.1252: column depth from the smooth carve ENVELOPE, not the
        // post-erosion point density (see g_cloud_carve's note - this is
        // the sandblast-stipple fix; magnitude stays right because the
        // carve IS the interior density the erosion ratio renormalizes
        // against).
        // Increment A: the column above the sample is the cloud's own column
        // (tau_above), not the regime band top 4 km up - the 10x overstatement
        // that made every ambient term FALL with extinction.
        // Increment 4: the envelope form (in-cloud light off) mixes toward
        // the profile's in-cloud column by w_pf; the increment-A form reads
        // tau_above, which is already mixed.
        var tau_vert_env = sigma_v * s_carve * max(s_btop - h, 0.0) * slab_h_d;
        if (w_pf > 0.0) {
            tau_vert_env = mix(tau_vert_env, tau_above_in, w_pf);
        }
        let tau_vert = select(tau_vert_env, tau_above, g_ms_on > 0.5);
        // A1 burial profile: 0 at the surface, 1 one transport MFP in.
        // Burial is the column AROUND the sample (above and below at the
        // envelope density), or the depth inside a built body. NOT the sun
        // rung tau: in a thin-skirted top the first 87 m toward the sun are
        // nearly clear while the sample sits hundreds of metres inside the
        // medium, and the first cut gated the source off exactly there
        // (bm-12 masked mean 45 -> 51 instead of the predicted 135+).
        let tau_col = min(tau_above, tau_below);
        let tau_min = max(tau_col, tau_built * s_v2_w);
        let prof = smoothstep(1.0, 4.0, tau_min) * g_ms_on;
        g_ms_prof = prof;
        // A4: powder is a rind effect.
        let pw_a = mix(pw, 1.0, prof);
        // A3: the in-scattered source. c = +1 when the eye looks DOWN at
        // this sample (light travelling up), mu_s the sun cosine.
        let ms_gain = select(1.0, camera.light5_color.z, camera.light5_color.z > 0.0);
        let e_ms = cloud_ms_source(tau_above, tau_below, dot(-rd, dirp), ndl, prof) * ms_gain;
        let direct = cloud_scatter_energy(tau, cos_vs, tau_vert) * pw_a + e_ms;

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
        // Increment A4: blue is a 150-300 m surface phenomenon; deep interior
        // is achromatic and lit by the solar diffuse field (e_ms), so the sky
        // and bounce ambient fade with burial.
        let sky_hue = mix(mix(vec3<f32>(1.0), sky_aer / sky_peak, 0.55), vec3<f32>(1.0), prof);
        let amb_col = (sky_hue * amb_h
            + vec3<f32>(0.98, 0.94, 0.88)
                * (CLOUD_AMB_BOUNCE * (1.0 - h) * bounce_t)) * (1.0 - prof);

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
        var crown_shade = mix(crown_floor, 1.12, dc.z);
        // 12f pouch shading: the from-below twin of the crown term. A
        // column whose own base hangs low (pouch -> 1) has more cloud
        // directly above its base and a smaller sky-view solid angle from
        // below - real mamma sit 20-40% darker than the surrounding base.
        // Weighted toward the band bottom so tops are untouched.
        let vband = clamp(
            1.0 - (h - reg.h_lo) / max(s_btop - reg.h_lo, 1.0e-4), 0.0, 1.0);
        var pouch_shade = mix(1.0, 0.72, s_pouch * vband * vband);
        // ── RELIEF ON THE PROFILE SHARE (increment 4) ── the crown, pouch
        // and cavity terms are surface relief of the marched field; the
        // profile share has no surface, so they mix toward 1.0 by w_pf and
        // the share renders f of bright thick cloud plus 1 - f of clear (the
        // brightness-equal condition).
        var cav = (1.0 - CLOUD_PUFF_AO * dc.y);
        if (w_pf > 0.0) {
            crown_shade = mix(crown_shade, 1.0, w_pf);
            pouch_shade = mix(pouch_shade, 1.0, w_pf);
            cav = mix(cav, 1.0, w_pf);
        }
        var ao = cav * crown_shade * pouch_shade;
        // ── INTERIOR RELIEF FADE (v0.1279 experiment, dev pad bit 19) ──
        // Relief is a surface phenomenon. `trans` here is the transmittance
        // from the eye to THIS sample: 1 at the visible surface, ~0 one
        // optical depth in. Deep samples are lit by diffuse multiple
        // scattering and carry no lobe-relief shading; without this, an eye
        // INSIDE a body sees the lobes around it painted as dark petals that
        // converge at the nadir.
        var relief_w = select(1.0, trans,
            fract(camera.light7_color.w * 0.00000095367431640625) >= 0.5);
        // Increment A4: relief is gated by BURIAL (world-space), never by
        // eye transmittance - the bit-19 null showed the load-bearing
        // samples sit at trans ~1 by construction.
        relief_w = mix(relief_w, 1.0 - prof, g_ms_on);
        ao = mix(1.0, ao, relief_w);
        // ── SHADE ON THE REAL SURFACE: GROUNDWORK ONLY (v0.1233) ──
        //
        // The normal and seam ARE computed now (41-cloud-bodies.wgsl) and cost
        // almost nothing, but the attempt to shade with them is NOT shipped:
        // wiring them into ao turned every cloud into a dark silhouette and
        // three separate retunes - gentler floor, gentler seam, NaN guards on
        // both normalizes - each failed to bring the brightness back, which
        // says the fault is in HOW the term enters the lighting rather than in
        // its magnitude.
        //
        // The likely reason, for whoever picks this up: the normal is only
        // meaningful within a rind of the surface. Deep inside a body the
        // gradient direction is arbitrary, and those interior samples carry
        // most of the accumulated weight - so an ao built from it is being
        // applied hardest exactly where it means least. The next attempt should
        // weight the term by surface proximity (g_v2_sdf_m is right there) and
        // apply it to the AMBIENT only, never to direct, since a sky-view term
        // is by definition about the sky.
        //
        // Shipping the groundwork unused rather than shipping a regression.
        // Direct carries the SUN's colour; ambient carries the SKY's (the
        // two-tone split above). Ambient magnitude rides the sun's
        // luminance so total energy matches the old single-hue form.
        // ── RADIATIVE-SMOOTHING CLAMP on direct (field-coherence
        // rebuild, 2026-08-31) ── the cavity field (dc.y, puff noise)
        // is sub-smoothing-scale structure in the sun channel, and real
        // cloud-top radiance carries at most ~1.1-1.35x local contrast
        // below ~300 m (Marshak 1995; the lobe-lattice audit measured
        // ours at 5-11x cap-vs-crevice - the dot lattice). The BUILT
        // path compresses cavity's bite on DIRECT from the 1.43x full
        // swing to <= ~1.15x; the noise path keeps its calibrated
        // half-strength (its look was tuned around it, and its body is
        // not a distance field). Ambient keeps its 0.35 cavity: the
        // bisect convicted the sun channel, not ambient. Envelope-scale
        // terms (tau_vert, pouch, day) keep their FULL range - the
        // clamp is scale-gated, never global, or the deck goes
        // cardboard (the v0.1241 melted-blob rejection cuts both ways).
        let cav_dir_w = mix(0.5, 0.12, s_v2_w);
        let direct_lit = direct * mix(1.0, clamp(ao, 0.0, 1.0), cav_dir_w);
        // ── SMOOTH AMBIENT (v0.1252, the operator's "sandblasted" grain) ──
        // The cavity noise (dc.y, puff-frequency) used to hit the AMBIENT
        // at full strength while direct took half - backwards physically.
        // Ambient skylight arriving inside a cloud is the most heavily
        // multiple-scattered light there is: fine crevices are FILLED IN
        // (that fill is why real cumulus read soft and luminous), and
        // real ambient occlusion operates at LOBE scale, not noise scale.
        // Per-sample cavity noise multiplying the ambient painted frozen
        // salt-and-pepper over every converged surface - static no
        // temporal filter could remove, because it is in the signal.
        // Ambient keeps the coarse relief terms (crown, pouch) and 35%
        // of the cavity; direct keeps its full half-strength cavity (the
        // sunlit cauliflower texture is real).
        // ── AMBIENT MUST NOT CARRY MIP-DEPENDENT RELIEF (v0.1266) ──
        // crown_shade and pouch_shade are functions of `body`, which is a
        // MIPPED texture sample - so they inherit the view footprint, and
        // on a down-look the footprint is monotone in the angle from the
        // nadir. On the CONSTRUCTED path they are already neutralised
        // (ring_off, v0.1252.6); on the NOISE path they run at full
        // strength - which is precisely where the operator still sees the
        // last residue: "from high orbit... much more pronounced in the
        // ambient light setting", on the full sheet rather than the voxel
        // clouds.
        //
        // Physically the compression is right anyway, and this is the same
        // argument that damped the cavity above: ambient skylight inside a
        // cloud is the most heavily multiple-scattered light there is, so
        // it cannot carry sharp relief structure. Direct keeps crown and
        // pouch at full strength - the sunlit relief cue is real and is
        // not view-dependent in the same way, because the sun path no
        // longer reads the view footprint at all (v0.1264).
        //
        // NOT a global flattening: 0.35 keeps a third of the relief, so
        // undersides and crowns still read, and the ELIMINATION that led
        // here is recorded too - the carve MAGNITUDE was measured
        // mip-invariant to 1% (carve_magnitude_fit), so tau_vert is not
        // the carrier and needs no gain table.
        let crown_amb = mix(1.0, crown_shade, 0.35 * relief_w);
        let pouch_amb = mix(1.0, pouch_shade, 0.35 * relief_w);
        var cav_amb = (1.0 - CLOUD_PUFF_AO * dc.y * 0.35 * relief_w);
        if (w_pf > 0.0) {
            cav_amb = mix(cav_amb, 1.0, w_pf);
        }
        let ao_amb = cav_amb * crown_amb * pouch_amb;
        let sun_lum = dot(sun_energy, vec3<f32>(0.2126, 0.7152, 0.0722));

        let c_i = material.base_color.rgb
            * (sun_energy * (direct_lit * day)
                + amb_col * (sun_lum * ao_amb * day)
                + vec3<f32>(CLOUD_NIGHT_FLOOR));
        acc = acc + c_i * (trans * a_i);
        acc_w = acc_w + trans * a_i;
        // Rosette-bisect channels (v0.1249): same weights as acc.
        let lum_w = vec3<f32>(0.2126, 0.7152, 0.0722);
        g_march_sun_acc = g_march_sun_acc
            + dot(sun_energy * (direct_lit * day), lum_w) * (trans * a_i);
        g_march_amb_acc = g_march_amb_acc
            + dot(amb_col * (sun_lum * ao_amb * day), lum_w) * (trans * a_i);
        acc_d = acc_d + tm * (trans * a_i);
        g_march_prof_acc = g_march_prof_acc + prof * (trans * a_i);
        // map_diag 9 (increment 1): which sun source lit this sample.
        g_march_src_acc = g_march_src_acc + g_light_src * (trans * a_i);
        // map_diag 10 / 11 / 12 (increment 4, A17): the profile share, the
        // blended level / 6 and the profile fraction, the colour's weights.
        g_march_pf_acc += trans * a_i * w_pf;
        g_march_lvl_acc += trans * a_i * pf.level / 6.0;
        g_march_frac_acc += trans * a_i * pf.f;
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
    // Increment A4: the v0.909 opacity darkening is a transmittance-era
    // proxy for an interior that now has its own source; off when A is on.
    radiance = radiance * mix(1.0 - 0.32 * smoothstep(0.72, 0.98, body_total), 1.0, g_ms_on);

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

