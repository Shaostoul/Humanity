//! THE PURE FIELDS of the sward: distance to density, and position to
//! clumping, height and dryness.
//!
//! Extracted VERBATIM from `terrain::grass` (2026-09-19) under the file-size
//! ratchet, which grass.rs had outgrown at 2,705 lines against a 2,645 budget.
//!
//! WHY THIS IS THE CLUSTER: grass.rs already says, in its own words, that the
//! layer is "three pure functions plus a harvest". This file is those pure
//! functions. Every one of them takes numbers and returns a number - no state,
//! no RNG draw, no world, no allocation - which is why they can be reasoned
//! about on their own, and why lifting them out cannot change what the harvest
//! does.
//!
//! The two halves:
//!   * THE DISTANCE RAMP - `grass_density_at`, its normalized twin
//!     `grass_ramp_at`, the closed-form inverse `grass_appear_distance`, and
//!     `grass_live_emerge`, the fade that hides the pop the inverse would
//!     otherwise make visible.
//!   * THE POSITION FIELDS - a planet-fixed integer lattice (`lattice_hash01`,
//!     `lattice_noise01`) and the four fields read off it: `grass_clump_gain`
//!     (where the sward bunches), `grass_filler_gain` (its complement, where
//!     the stubble class fills in), `grass_height_field` (so a stand agrees
//!     with itself) and `grass_senescence` (how yellowed it reads).
//!
//! THE ONE RULE THAT BINDS THEM, and the reason they are all pure: a field is
//! keyed on POSITION, never on the scatter stream. The harvest draws exactly
//! six randoms per item and nothing may be inserted into that stream, so
//! anything that varies across the ground has to be a function of where the
//! ground is. That is said again on `lattice_hash01` below, where it bites.
//!
//! Declared as a `#[path]` CHILD of `grass`, not a sibling, for the reason
//! `grass_mesh` gives: one `use super::*` brings in the `GRASS_*` constants
//! these read, and the parent's `pub use` keeps every existing
//! `grass::grass_clump_gain` / `planet_chunks::grass_density_at` path
//! resolving, so `terrain/mod.rs` and every call site needed no edit at all.

use super::*;

/// The surface distance at which a tiller of this threshold starts to exist:
/// the inverse of the density ramp. Beyond it the local density is below the
/// tiller's own threshold and it is not drawn; inside it, it is.
///
/// Closed form because `grass_density_at` is piecewise linear, and exact
/// rather than a search because it is evaluated per tiller per frame.
pub fn grass_appear_distance(thr: f32) -> f32 {
    if thr >= 1.0 {
        return 0.0;
    }
    if thr <= 0.0 {
        return grass_far_m();
    }
    let m = GRASS_MID_FRACTION;
    if thr >= m {
        // On the peak-to-mid leg: density/PEAK falls 1 -> m over NEAR..MID.
        let t = (thr - 1.0) / (m - 1.0);
        GRASS_NEAR_M + (GRASS_MID_M - GRASS_NEAR_M) * t
    } else {
        // On the mid-to-zero leg: density/PEAK falls m -> 0 over MID..FAR.
        let t = 1.0 - thr / m;
        GRASS_MID_M + (grass_far_m() - GRASS_MID_M) * t
    }
}

/// How tall this tiller stands right now, as a fraction of `height_m`, for a
/// camera `d_m` surface-metres away. Zero means "not drawn at all".
///
/// The density ramp and the grow-in are ONE function evaluated at draw time:
/// a tiller grows from nothing to full height over the last
/// `GRASS_EMERGE_LEN_M` metres of the camera's approach, wherever on the ramp
/// its own threshold happens to sit. Expressing the band in METRES OF CAMERA
/// TRAVEL rather than in units of density is what makes the grow-in rate
/// uniform: the ramp's slope varies 4x between its two legs, so a fixed
/// density band pops blades in on the steep leg (measured: a 0.22 m blade
/// appearing from nothing in a single 25 cm step) while over-fading them on
/// the shallow one.
#[inline]
pub fn grass_live_emerge(thr: f32, d_m: f32) -> f32 {
    ((grass_appear_distance(thr) - d_m) / GRASS_EMERGE_LEN_M).clamp(0.0, 1.0)
}

/// The distance ramp's SHAPE, normalized to 1.0 at the peak: full inside
/// `GRASS_NEAR_M`, down to `GRASS_MID_FRACTION` at `GRASS_MID_M`, then to
/// zero at `GRASS_FAR_M`.
///
/// Normalized rather than absolute because it is the only form the harvest
/// and the per-frame draw gate actually want (both divide by the peak), and
/// because it keeps the ramp a pure const function now that the peak is
/// derived from a target LAI.
///
/// Why a ramp at all: this is what makes the layer ringless AT ITS EDGE. The
/// bake gated on patch DEPTH, so the field ended wherever the LOD selector
/// happened to stop refining - a hard edge that moved with the camera and lit
/// up at grazing sun (the v0.999 report). A density that reaches zero smoothly
/// has no edge to see.
///
/// KNOWN RESIDUAL, measured not guessed (v0.1103): the ramp is C0 but not C1.
/// Its slope jumps at all three anchors - 0 to -3.10 tillers/m^2/m at
/// `GRASS_NEAR_M`, -3.10 to -0.84 at `GRASS_MID_M`, and -0.84 to 0 at
/// `GRASS_FAR_M` - and a first-derivative discontinuity in a density field is
/// a Mach band: three faint concentric rings at 6 m, 12 m and 22 m that move
/// with the player. That is the strongest remaining candidate for the
/// operator's "they still look like rings" once coverage is fixed. It is NOT
/// fixed here because every C1 profile through the same three anchors costs
/// instances: a squared-cosine bump (`(1-u^2)^2` over NEAR..FAR) draws 18,979
/// tillers against this ramp's 12,755 (+49%), and a parabola pair that is C1
/// at the knee forces either `GRASS_MID_M` to 17.0 m or `GRASS_MID_FRACTION`
/// to 0.625 (+25%). The cheap fix is a parabolic FILLET of half-width ~1.5 m
/// at each knee: mass-neutral to second order, still invertible in closed form
/// (one sqrt), and it is what the next increment here should do.
#[inline]
pub fn grass_ramp_at(d_m: f32) -> f32 {
    if d_m <= GRASS_NEAR_M {
        1.0
    } else if d_m < GRASS_MID_M {
        let t = (d_m - GRASS_NEAR_M) / (GRASS_MID_M - GRASS_NEAR_M);
        1.0 + (GRASS_MID_FRACTION - 1.0) * t
    } else if d_m < grass_far_m() {
        let t = (d_m - GRASS_MID_M) / (grass_far_m() - GRASS_MID_M);
        GRASS_MID_FRACTION * (1.0 - t)
    } else {
        0.0
    }
}

/// Tillers per m^2 of ground at `d_m` surface metres from the camera. NOT
/// scaled by `veg_density` since v0.1103 - see `GRASS_TARGET_LAI`.
#[inline]
pub fn grass_density_at(d_m: f32) -> f32 {
    grass_peak_per_m2() * grass_ramp_at(d_m)
}

/// Hash a planet-fixed integer lattice node to 0..1. Pure, no state, no RNG
/// draw - which is the point: the scatter stream draws exactly 6 randoms per
/// item and nothing may be inserted into it (see the stream comment in
/// `near_grass_instances`), so every field below is keyed on POSITION.
#[inline]
fn lattice_hash01(i: i64, j: i64, salt: u64) -> f32 {
    let mut h = (i as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (j as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt;
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    ((h >> 40) as f32) / 16_777_216.0
}

/// Smooth value noise on the planet-fixed lat/lon lattice at `cell` radians,
/// 0..1. Smootherstep between the four corners so the field has no lattice
/// creases (a plain lerp shows the grid as diamond-shaped density steps).
fn lattice_noise01(lat: f64, lon: f64, cell: f64, salt: u64) -> f32 {
    let y = lat / cell;
    let x = lon / cell;
    let (iy, ix) = (y.floor(), x.floor());
    let (fy, fx) = ((y - iy) as f32, (x - ix) as f32);
    let s = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (wy, wx) = (s(fy), s(fx));
    let (iy, ix) = (iy as i64, ix as i64);
    let c00 = lattice_hash01(iy, ix, salt);
    let c01 = lattice_hash01(iy, ix + 1, salt);
    let c10 = lattice_hash01(iy + 1, ix, salt);
    let c11 = lattice_hash01(iy + 1, ix + 1, salt);
    let a = c00 + (c01 - c00) * wx;
    let b = c10 + (c11 - c10) * wx;
    a + (b - a) * wy
}

/// Local density multiplier, mean ~1 over any large area. Turns the exact
/// homogeneous Poisson process the bake produced (variance-to-mean ratio
/// 1.0 by construction) into a clustered one, which is what makes a
/// correctly-dense field read as a MEADOW rather than as static.
///
/// Real grass is patchy at 0.5-5 m from soil moisture, litter, trampling and
/// clonal spread, and it carries genuine bare scrapes. Two octaves give the
/// clumps (~3.4 m) and the fine break-up (~1.4 m); the low tail is squashed
/// to zero so scrapes exist at all.
///
/// Cost note: this is evaluated per CANDIDATE, before the trigonometry, so
/// it must stay two hashes deep. It is deliberately keyed on lat/lon rather
/// than on a 3D direction because lat/lon are already in hand at that point
/// (they come straight out of the stream, no transcendentals needed).
pub fn grass_clump_gain(lat: f64, lon: f64) -> f32 {
    let coarse = lattice_noise01(lat, lon, GRASS_FIELD_RAD * 0.22, 0x51E5_C0FF_EEA1_1CE5);
    let fine = lattice_noise01(lat, lon, GRASS_FIELD_RAD * 0.09, 0xD1CE_5EED_0BAD_F00D);
    let c = coarse * 0.75 + fine * 0.25;
    // LINEAR about the field's own mean of 0.5, so the multiplier's mean is
    // 1.0 by construction and the peak density stays the number the constant
    // says it is. The low clip creates real bare scrapes (c below 0.25);
    // GRASS_CLUMP_GAIN_MAX caps the thick end so the per-cell item budget can
    // be sized once. `grass_scatter_is_clustered_not_poisson` measures the
    // realised mean AND the variance-to-mean ratio off the emitter rather
    // than trusting this arithmetic.
    (1.0 + 4.0 * (c - 0.5)).clamp(0.0, GRASS_CLUMP_GAIN_MAX)
}

/// Local density multiplier for the FILLER STUBBLE class: the complement of
/// `grass_clump_gain`, normalized to 0..1.
///
/// Deliberately the exact complement rather than a field of its own. The
/// stubble exists to answer ONE question - how bare is the ground between the
/// tussocks here - and the clump field already answers it, so a second noise
/// field would only let the two drift out of register and leave gaps that no
/// population fills. At the clump field's mean (gain 1.0) this returns 0.61,
/// which is why the realised filler density is ~61% of `grass_filler_per_m2`;
/// in a bare scrape (gain 0) it returns 1.0 and the stubble is at its
/// thickest; inside the fattest clump (gain `GRASS_CLUMP_GAIN_MAX`) it
/// returns 0 and the class disappears, so a tussock stays a tussock.
#[inline]
pub fn grass_filler_gain(lat: f64, lon: f64) -> f32 {
    (GRASS_CLUMP_GAIN_MAX - grass_clump_gain(lat, lon)) / GRASS_CLUMP_GAIN_MAX
}

/// Local height multiplier (~0.75..1.25) on a COARSER field than the clumping
/// one, so tall stands sit in hollows and along drainage while ridges read
/// short - a real sward's height is strongly correlated over tens of metres,
/// where the bake drew every tuft's height as an independent uniform.
pub fn grass_height_field(lat: f64, lon: f64) -> f32 {
    let n = lattice_noise01(lat, lon, GRASS_FIELD_RAD * 1.2, 0x600D_5EED_1234_5678);
    0.85 + n * 0.30
}

/// Fraction of a tiller's tissue that has gone senescent (dry/yellow). A real
/// meadow always carries 5-20% dead tissue and it rides the same dryness the
/// height field describes, so ridges read dry and hollows green.
pub fn grass_senescence(lat: f64, lon: f64, r: u64) -> f32 {
    // -0.25 (lush hollow) .. +0.25 (dry ridge).
    let dry = 1.0 - grass_height_field(lat, lon);
    let jitter = (r % 1000) as f32 / 1000.0;
    // The PER-TILLER term has to dominate. The dryness field is correlated
    // over ~24 m, so driving senescence from it alone makes every tiller
    // within one stand agree - measured as 0% yellowed across a whole 8 m
    // disc at Fuji, i.e. a perfectly lush field, which no real meadow is.
    // Individual leaves die on their own schedule; the field only shifts the
    // odds.
    (dry * 0.8 + jitter * 0.55 - 0.20).clamp(0.0, 0.45)
}
