//! Near-field grass STRANDS: the sward layer.
//!
//! Extracted verbatim from `terrain::planet_chunks` (v0.1092) - the strand
//! increment landed inside the chunked-LOD module because that is where the
//! patch tree it samples lives, and it took that file 1,078 lines past its
//! ratchet budget. Nothing here changed in the move: the constants, the
//! drawn-surface sampler, the harvest, the shared mesh and every grass test
//! are byte-identical to what shipped in v0.1091.
//!
//! What lives here:
//!   * the `GRASS_*` constants (field scale, stream cell, density ramp,
//!     tiller shape, harvest margins, gate lattice);
//!   * the ground sampler [`DrawnPatchSurface`] - it interpolates the DRAWN
//!     patch triangle rather than re-sampling the elevation field, which is
//!     what puts a blade base on the ground you can actually see. It MOVED
//!     OUT in v0.1097, exactly as the note here asked: near-field trees
//!     needed the same thing, so it lives in `terrain::drawn_surface` and is
//!     re-exported here for the call sites that say `grass::`;
//!   * the three pure field functions (`grass_density_at`,
//!     `grass_clump_gain`, `grass_height_field`) plus `grass_senescence`
//!     and the live-emergence pair;
//!   * [`near_grass_instances`], the planet-fixed harvest, and
//!     [`grass_tiller_mesh`], the ONE shared mesh every instance draws.
//!
//! Every existing `planet_chunks::` path still resolves: that module glob
//! re-exports this one, so `lib.rs`, `engine::state` and `renderer` needed
//! no edits.

use glam::DVec3;

use super::planet::PlanetDef;
use super::planet_albedo::PlanetAlbedo;
// NOTE the absence of `veg_density`: since v0.1103 nothing in this file may
// read the quality slider. Coverage is derived from `GRASS_TARGET_LAI`; the
// slider is interpreted exactly once, in `grass_mesh::grass_detail_for`. The
// CI twin `near_grass_density_matches_a_real_sward` scans this file for a
// re-introduced read, because the test process runs at the atomic's 1.0
// default and could never measure one.
use super::planet_chunks::{
    drawn_elevation_normalized, veg_biome_weight, ElevationSource, TREELINE_M, VEG_WEIGHT_MIN,
};
use super::planet_surface::surface_color;

/// The drawn-ground sampler, PROMOTED to `terrain::drawn_surface` (v0.1097)
/// now that trees stand on it too. Re-exported here (and so, through
/// `planet_chunks`'s glob, everywhere it was ever reached from) because it
/// was grass's own type for two releases and every call site says `grass::`.
pub use super::drawn_surface::DrawnPatchSurface;

/// The shared tiller MESH and the detail ladder that sizes it (v0.1103), a
/// `#[path]` CHILD module rather than a sibling so every existing
/// `planet_chunks::grass_tiller_mesh` / `GRASS_BLADES_*` path still resolves
/// through this re-export and `terrain/mod.rs` needed no edit.
#[path = "grass_mesh.rs"]
mod grass_mesh;
pub use grass_mesh::*;

// ── Near-field grass STRANDS (v0.1090) ────────────────────────────────────
// The baked grass card is deleted, not deprecated. What it was: two crossed
// 0.5 m x 0.25-0.50 m opaque quads per tuft on a 33 m planet cell at 160
// tufts/cell, i.e. 0.087 tufts/m^2 - one tuft per 11.5 m^2, an effective LAI
// of 0.033 against a measured turfgrass canopy of 1.9-6.0. Two independent
// measurements agreed the ground was a painted plane: 99.8% of 16x16 px
// blocks in the near-field had internal luma SD under 3/255, and the emitter
// never fired at all at the Fuji vantage because its depth gate was never
// reached. A layer 60-180x short of its target is not tunable; it is
// replaced.
//
// The replacement is CAMERA-RELATIVE and INSTANCED: `near_grass_instances`
// walks a planet-fixed lat/lon cell grid around the camera (the same
// discipline `near_tree_instances` uses, so a tiller stands in the same spot
// at every distance and never reshuffles), and the renderer draws ONE
// instanced draw of the shared tiller mesh below against a per-instance
// buffer. Density then ramps with DISTANCE instead of stepping with patch
// depth, which is what removes the moving lit ring the depth gate produced
// (v0.999 operator report: "a line of light perpendicular to me like 10
// meters away").

/// The reference scale of the strand FIELDS (clumping, height, dryness), in
/// radians of arc: ~8 m at Earth's radius. Everything spatial about how the
/// sward LOOKS is expressed as a fraction of this, and nothing about how it
/// is streamed - so the stream cell below can be retuned for cost without
/// moving a single clump.
pub const GRASS_FIELD_RAD: f64 = 1.2557e-6;
/// Stream cell size in radians of arc: ~3 m at Earth's radius.
///
/// SIZED BY THE RAMP, not by the field. A cell's whole item stream is
/// budgeted from the density at its NEAREST corner, so on the steep leg of
/// the ramp a big cell evaluates far more candidates than it can accept: at
/// ~8 m cells the harvest drew 285,000 candidates to accept 46,000 (6.2x),
/// and 232,000 of those were in ramp-straddling cells. At ~3 m the cell's
/// nearest corner is close to its whole extent, and that overhead shrinks.
///
/// Smaller still stops paying: per-cell setup (sort key, stream seed) is
/// fixed cost, and the cell count grows with the square. The live cost is
/// printed by `near_grass_density_matches_a_real_sward`; measure there
/// rather than reasoning about it.
pub const GRASS_CELL_RAD: f64 = 4.7087e-7;
/// LEAF AREA INDEX of the near sward: one-sided leaf area per unit ground,
/// counting BOTH populations. THE AUTHORED QUANTITY of this whole layer, and
/// the thing the quality slider is no longer allowed to touch (v0.1103).
///
/// WHY THIS REPLACED A TILLER COUNT. Coverage and detail used to be one
/// number: `veg_density` multiplied the tiller stream, so a player who turned
/// quality down turned the ground BALD. Measured at the operator's live 0.1039
/// setting: LAI 0.58, against 3.33 at the 0.6 default - the sward read as
/// isolated dark spikes scattered on a lawn, and a large part of what was
/// reported as grass defects (the "rings", the "grass only appears a few feet
/// in front of me") was that. Leaf area is what the eye reads as "is this
/// ground covered", so it is authored here and the tiller count is DERIVED
/// from it; the slider moves blade detail instead (`grass_detail_for`), with
/// blade width rising as blade count falls so projected leaf area is
/// conserved. Standard practice: Jahrmann & Wimmer, "Responsive Real-Time
/// Grass Rendering for General 3D Scenes" (I3D 2017).
///
/// 3.11 is inside the measured turfgrass range (LAI 1.9-6.0, PLOS One
/// 10.1371/journal.pone.0188080) and is the value that reproduces the SHIPPED
/// 0.6 default exactly: it derives 27.0 tillers/m^2, which is what
/// `GRASS_PEAK_PER_M2 45.0 * 0.6` used to give. Grass is erectophile
/// (extinction ~0.3-0.5), so it needs MORE leaf area than a horizontal-leaf
/// canopy for the same light interception.
pub const GRASS_TARGET_LAI: f32 = 3.11;
/// MEAN OF height^2 over the emitted tussock population, m^2 - the factor that
/// turns a unit-height mesh's leaf area into a real instance's. MEASURED off
/// the emitter (the height field x the per-tiller jitter, clamped to
/// `GRASS_HEIGHT_MIN_M..MAX`), not derived, because the clamp makes it not the
/// square of the mean. If the height range or field changes, the CI twin
/// `near_grass_density_matches_a_real_sward` fires and this is re-measured.
pub const GRASS_MEAN_H2_M2: f32 = 0.130;
/// The FILLER class's share of the canopy's leaf area, as a fraction of the
/// tussock class's: `GRASS_FILLER_FRACTION` x 0.615 (the mean of the clump
/// field's complement) x 0.281 (the mean of the filler height scale SQUARED).
/// Folded into the derivation below so `GRASS_TARGET_LAI` means the TOTAL
/// canopy, which is what the eye and the CI twin both measure.
pub const GRASS_FILLER_LAI_SHARE: f32 = 0.0365;
/// Density at `GRASS_MID_M` as a FRACTION of the peak. The ramp's SHAPE, kept
/// separate from its magnitude so `grass_appear_distance` stays a pure const
/// inverse that no derived density has to be threaded through.
pub const GRASS_MID_FRACTION: f32 = 14.0 / 45.0;

/// Peak tiller density (tillers per m^2 of ground) inside `GRASS_NEAR_M`.
/// DERIVED from `GRASS_TARGET_LAI`, not authored, and NOT scaled by
/// `veg_density` - that is the whole v0.1103 change. Works out to 27.0/m^2,
/// which is 243 drawn blades/m^2 at the default rung.
///
/// WHY THIS IS NOT A REAL SHOOT COUNT, stated plainly so nobody "corrects" it
/// later: a real turf carries 3,000-30,000 SHOOTS/m^2, which no renderer
/// draws as geometry. Each drawn blade is a BUNDLE (see
/// `GRASS_LEAF_AREA_UNIT`). The quantity that has to be right is the one the
/// eye reads, which is leaf AREA per ground area; the CI twin
/// `near_grass_density_matches_a_real_sward` measures LAI off the real
/// emitted geometry at EVERY slider position rather than trusting this
/// comment.
#[inline]
pub fn grass_peak_per_m2() -> f32 {
    // `grass_density` is the COVERAGE knob (v0.1106) and is the one thing in
    // this file allowed to scale the target. It is not the quality slider that
    // v0.1105 removed: quality now lives in `grass_detail` and changes blades
    // per tiller with width compensating, so it cannot move leaf area at all.
    // The distinction is the whole point - the operator wanted thick grass and
    // thin forest, and one slider could not express that.
    GRASS_TARGET_LAI * crate::terrain::planet_chunks::grass_density()
        / (GRASS_LEAF_AREA_UNIT * GRASS_MEAN_H2_M2 * (1.0 + GRASS_FILLER_LAI_SHARE))
}
/// Density at `GRASS_MID_M`, tillers per m^2.
#[inline]
pub fn grass_mid_per_m2() -> f32 {
    grass_peak_per_m2() * GRASS_MID_FRACTION
}
/// Full density out to here (metres of surface distance from the camera).
pub const GRASS_NEAR_M: f32 = 6.0;
/// Mid ring: the peak ramps linearly to `GRASS_MID_FRACTION` of itself here.
pub const GRASS_MID_M: f32 = 12.0;
/// Density reaches ZERO here. Nothing takes over past it - the ground
/// texture carries the far field - so the ramp has to reach zero smoothly or
/// it draws a ring. The old baked cards are NOT kept as a 15-45 m stage:
/// they were the geometry this increment deletes, and keeping them would
/// have meant maintaining both a strand layer and a card layer plus a
/// hide-radius handshake between them.
///
/// THIS IS NOT WHAT BOUNDED THE VISIBLE FIELD (v0.1103, operator: "grass
/// appears in front of me by only a few feet"). Nothing gates grass on a short
/// radius; 22 m is the ramp's end and there are only two other limits, both
/// slack - the harvest's `max_n` (70,000 in lib.rs, against a 31,000-instance
/// superset) and the per-frame re-gate, which is the ramp again. What actually
/// ended the field early was DENSITY: at the operator's 0.1039 the ramp gave
/// 4.7 tillers/m^2 at the camera, 1.5 at 12 m and 0.5 at 18 m, so past ~10 m
/// there was nothing left to see. Deriving coverage from LAI puts 27, 8.4 and
/// 3.4 there instead, a 5.8x thicker far field, and the layer should now read
/// out to its full 22 m without this constant moving.
///
/// PUSHING IT FURTHER, costed so the decision is informed rather than a guess:
/// integrating the ramp over the disc, 22 -> 35 m takes the drawn set from
/// 12,755 to 20,645 tillers (+62%) and roughly doubles the cell walk
/// ((35+6)^2 / (22+6)^2 = 2.1x). The principled way to buy that is the
/// technique the quality ladder already implements - bucket the instance list
/// by distance and draw each bucket with its own rung, so a tiller at 30 m
/// costs 30 triangles instead of 90 (Jahrmann & Wimmer, I3D 2017). That needs
/// a second mesh slot and a second instanced draw in the renderer; nothing in
/// this file has to change for it.
pub const GRASS_FAR_M: f32 = 22.0;

/// LIVE grass draw distance, f32 bits in an atomic (the harvest runs on worker
/// threads - same pattern as `GRASS_DENSITY_BITS`). `GRASS_FAR_M` above stays
/// the DEFAULT and the shipped picture; this is what the ramp and the harvest
/// actually read, written each frame from the Settings control.
///
/// The operator asked for this directly: "I would like to see how the game
/// performs when I extend the grass to render further away... then I wouldn't
/// have to ask you to increase the ceiling."
pub static GRASS_FAR_BITS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x41B0_0000); // 22.0f32

/// The ramp's zero point. Floored just past `GRASS_MID_M` so the last leg can
/// never invert, and ceilinged at the same 250 m the Settings control offers
/// (duplicated as a literal because `config` is native-gated and this file is
/// not).
#[inline]
pub fn grass_far_m() -> f32 {
    f32::from_bits(GRASS_FAR_BITS.load(std::sync::atomic::Ordering::Relaxed))
        .clamp(GRASS_MID_M + 1.0, 250.0)
}
/// Ceiling on `grass_clump_gain`. Two things depend on it: the per-cell item
/// budget (a cell sitting entirely inside a clump must have enough items in
/// its stream to fill it) and the acceptance probability (which divides by
/// this, so it can never exceed 1).
pub const GRASS_CLUMP_GAIN_MAX: f32 = 2.6;
// The tiller MESH constants (`GRASS_BLADES_*`, `GRASS_ROOT_SPREAD_*`, the
// blade width profile and `GRASS_LEAF_AREA_UNIT`) live in the `grass_mesh`
// child module above and are re-exported from here, so every path that ever
// said `grass::GRASS_BLADES_PER_TILLER` still resolves.

/// FILLER STUBBLE density as a FRACTION of the tussock peak, so the two
/// populations keep their shipped ratio now that the peak is derived rather
/// than authored (9.5 / 45.0 - the two constants this replaced). The realised
/// mean is about 61% of the product, because the class rides the COMPLEMENT of
/// `grass_clump_gain` (see `grass_filler_gain`) and that complement averages
/// 0.61 over the clump field.
///
/// The second population exists because a clumped field is bare between its
/// clumps: at a clump gain of 0.2 the tussock density is a fifth of nominal,
/// and the eye reads the ground there as dirt with bouquets standing on it.
/// Real swards carry a low, sparse stubble of individual shoots in exactly
/// those gaps - grazing, trampling and fresh tillering all leave short
/// single shoots where the clumps thinned.
///
/// SIZED BY THE TRIANGLE BUDGET, not by botany: a filler instance draws the
/// same shared mesh as a tussock (one mesh, one draw - see
/// `grass_tiller_mesh`), so it costs a full tiller's triangles however small
/// it is drawn. 0.2111 of the peak puts the realised addition near 13% of the
/// tussock population, which `near_grass_density_matches_a_real_sward` prints
/// as a triangle count. A genuinely 1-2 blade filler mesh would cost a ninth
/// of that and is a renderer-side follow-up (a second mesh + a second
/// instanced draw); nothing here has to change when it lands, only the mesh
/// the filler class is drawn with.
pub const GRASS_FILLER_FRACTION: f32 = 9.5 / 45.0;
/// Filler instances per m^2 where the clump field is emptiest, before the
/// distance ramp.
#[inline]
pub fn grass_filler_per_m2() -> f32 {
    grass_peak_per_m2() * GRASS_FILLER_FRACTION
}
/// Height of a filler instance as a fraction of the tussock height the same
/// spot would grow. 0.40-0.65 of a 0.24-0.52 m sward is 10-34 cm, i.e.
/// distinctly a short shoot between the tussocks rather than another tussock.
pub const GRASS_FILLER_HEIGHT_LO: f32 = 0.40;
pub const GRASS_FILLER_HEIGHT_HI: f32 = 0.65;
/// Stream salt for the filler class. A DIFFERENT salt from the tussock
/// stream's, not a different offset into it: the two populations must be
/// statistically independent, and sharing one stream would make the sparse
/// class a nested subset of the dense one (see the class loop in
/// `near_grass_instances`). Keeping the tussock salt untouched also means
/// this increment moves no existing tiller by a millimetre.
const GRASS_FILLER_SALT: u64 = 0x5EED_1A11_E75B_10DE;
/// Height range of a tiller in metres (the mesh is built at unit height and
/// scaled per instance). A 30 cm sward is the operator-facing target; the
/// spread is spatially correlated (see `grass_height_field`), so stands agree
/// locally instead of every neighbour being an independent draw.
pub const GRASS_HEIGHT_MIN_M: f32 = 0.24;
pub const GRASS_HEIGHT_MAX_M: f32 = 0.52;
/// Downward bias applied to every tiller base, in metres. MEASURED, not
/// guessed - `grass_bases_sit_on_the_drawn_surface` ray-casts real strands
/// against the real built patch mesh at two sites and two depths.
///
/// It is SMALL because a tiller base is now placed ON THE DRAWN TRIANGLE
/// (`DrawnPatchSurface`, below), not on a direct sample of the elevation
/// field. Interpolating the resident patch mesh's own face is exact at any
/// depth and any slope by construction, so the residual is f64 rounding plus
/// the tiller's own sub-millimetre offset from the plane. The bias exists
/// only to make sure a crown is never seen hovering by a pixel; it is far
/// under the 24 cm shortest blade, so nothing disappears into the ground.
///
/// HISTORY, so nobody re-enlarges it: while the base came from a DIRECT
/// `drawn_elevation_normalized` sample, Fuji at depth 17 measured p95 +1.06 m
/// with 23.8% of tillers fully buried, because the mesh samples the f32
/// base-heightmap staircase at its own 0.84 m lattice and LERPS between those
/// samples while a direct sample lands on whatever ~1.4 m tread it hits.
/// A constant offset cannot correct that; interpolating the drawn face does.
pub const GRASS_GROUND_BIAS_M: f64 = 0.03;
/// How far the camera travels while a tiller grows from nothing to its full
/// height, in metres. See `grass_live_emerge`: this is the ONE number that
/// decides whether a blade appears or emerges. At a 5 m/s sprint and 30 fps
/// the camera covers 0.17 m in a frame, so 2 m of grow-in caps the tallest
/// thing that can appear between two frames at ~8% of a blade.
pub const GRASS_EMERGE_LEN_M: f32 = 2.0;
/// How far the CPU harvest over-reaches, in metres. The harvest is a
/// SUPERSET: it accepts every tiller that any camera position within this
/// radius of the harvest centre could want, which is what lets the harvest
/// re-anchor without the visible population changing at all (the drawn set is
/// re-derived from the LIVE camera every frame - `grass_live_emerge`).
///
/// Sized against the re-harvest trigger, not against comfort: lib.rs
/// re-harvests after `GRASS_REHARVEST_M` of camera movement, and the worker
/// takes tens of milliseconds, so the margin has to cover the trigger plus
/// the latency of a fast walk. Bigger costs harvest time (the superset grows
/// with the square of the peak-density radius); smaller thins the leading
/// edge while a harvest is in flight.
pub const GRASS_HARVEST_MARGIN_M: f64 = 6.0;
/// Camera movement that triggers a re-harvest, metres. Strictly less than
/// `GRASS_HARVEST_MARGIN_M` so the in-flight superset always still covers.
pub const GRASS_REHARVEST_M: f64 = 4.0;
/// Ground-GATE lattice spacing as a fraction of `GRASS_FIELD_RAD`: the
/// elevation band and surface colour that decide WHETHER a tiller exists are
/// evaluated on a PLANET-FIXED lattice at ~1 m and bilinearly interpolated,
/// because those are yes/no questions about ground that varies over hundreds
/// of metres. The tiller's standing POSITION is not: it comes off the drawn
/// patch face, per surviving tiller (see the comment at that call).
///
/// Keyed on the FIELD scale, not the stream cell: this is a cost/accuracy
/// tradeoff about the GROUND, and it must not move when the stream cell is
/// retuned - a finer gate lattice is thousands more elevation samples.
///
/// Planet-fixed matters even for the gates: a camera-relative lattice shifts
/// under the field every time the harvest recentres, so tillers would wink in
/// and out along the biome boundary as you walked.
pub const GRASS_LATTICE_DIV: i64 = 8;

// ══ Near-field grass strands (v0.1090) ═══════════════════════════════════
//
// The whole layer is three pure functions plus a harvest:
//
//   grass_density_at   distance -> tillers per m^2 (the ramp that replaces
//                      the old patch-depth gate)
//   grass_clump_gain   position -> local density multiplier (mean 1) so the
//                      field is CLUSTERED, not the exact Poisson process the
//                      bake produced
//   grass_height_field position -> local height multiplier, coarse, so a
//                      stand agrees with itself instead of every neighbour
//                      being an independent draw
//   near_grass_instances  the harvest: planet-fixed cells, fixed-6-randoms
//                      stream, elevation + biome gates, one Vec of instances
//
// plus `grass_tiller_mesh`, the ONE shared mesh every instance draws.

/// One grass tiller from the planet-fixed strand stream. The twin of
/// `NearTree`: `dir` is the planet-local unit direction of the crown, `r_m`
/// the drawn ground radius there (already carrying `GRASS_GROUND_BIAS_M`).
///
/// The renderer's per-instance record is built from this - see the WIRING
/// note on `near_grass_instances`. Everything here is per-instance ONLY;
/// anything shared by every tiller belongs in `grass_tiller_mesh` instead.
#[derive(Clone, Copy, Debug)]
pub struct NearGrass {
    pub dir: DVec3,
    pub r_m: f64,
    /// Rotation about the local radial up, radians.
    pub yaw: f32,
    /// Metres, crown to tip. The shared mesh is built at UNIT height, so this
    /// is also the instance's uniform scale.
    pub height_m: f32,
    /// Albedo, tinted from `surface_color` at this exact spot (finding 3: the
    /// old tuft was a planet-wide constant straw green against real imagery).
    pub color: [f32; 3],
    /// 0..TAU. Feeds the wind branch's per-plant phase so a stand does not
    /// sway in lockstep; derived from the planet-fixed stream, so it is
    /// stable across harvests.
    pub phase: f32,
    /// This tiller's place in its cell's acceptance order, expressed as the
    /// NORMALIZED DENSITY at which it starts to exist: it is drawn exactly
    /// while `grass_ramp_at(live distance) > thr`,
    /// i.e. inside `grass_appear_distance(thr)` of the camera.
    ///
    /// This is the whole reason the layer has no population ring. The CPU
    /// harvest is a camera-relative SUPERSET, re-run only every
    /// `GRASS_REHARVEST_M`; the visible set is re-derived from the LIVE
    /// camera distance every frame through `grass_live_emerge`, so the
    /// density ramp is anchored on where you actually are, never on where
    /// the last harvest happened. Recentring the harvest changes nothing
    /// on screen.
    pub thr: f32,
    /// Which of the two populations this instance belongs to: `false` is a
    /// TUSSOCK (the clumped main sward), `true` is FILLER STUBBLE - a short
    /// shoot from the sparse second population that rides the COMPLEMENT of
    /// the clump field, so it is thickest exactly where the tussocks thin
    /// out (`grass_filler_gain`).
    ///
    /// Today the two classes differ only in where they stand and how tall
    /// they are drawn: both draw the ONE shared tiller mesh, because the
    /// renderer has one grass mesh and one instanced draw. The flag is
    /// carried per instance so that a cheaper 1-2 blade filler mesh can be
    /// bound to this class from the renderer side without the harvest
    /// changing at all - the split is decided here, where the clump field
    /// is in hand.
    pub filler: bool,
}

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

/// Ground sample shared by every tiller near a lattice node: what the GATES
/// need. Real metres above sea level (the beach + treeline band) and the
/// surface colour (the biome gate, and the tint the blades take). NOT the
/// standing position - that is sampled per tiller.
#[derive(Clone, Copy)]
struct GrassGroundNode {
    elev_m: f32,
    color: [f32; 3],
}

/// Enumerate grass instances within `far_m` surface metres of `center_dir`.
///
/// TWO POPULATIONS come back in one Vec, tagged by `NearGrass::filler`:
/// TUSSOCKS (the clumped main sward, riding `grass_clump_gain`) and FILLER
/// STUBBLE (short shoots riding `grass_filler_gain`, the complement of the
/// same field, so they are thickest exactly where the tussocks thin out).
/// The second class exists because a clumped field is bare BETWEEN its
/// clumps: at a clump gain of 0.2 the tussock density is a fifth of nominal
/// and the eye reads that ground as dirt with bouquets standing on it.
/// Each class has its own per-cell stream; see the class loop below for why
/// that is required rather than tidy.
///
/// The twin of `near_tree_instances`, on a much finer planet-fixed cell
/// (~8 m against the tree grid's ~220 m) and with the SAME discipline in the
/// places that matter:
///
///   * positions come from a per-cell xorshift stream seeded from the CELL
///     COORDINATES, never from the camera, so a tiller stands in exactly the
///     same spot no matter where you harvest from;
///   * every item draws a FIXED SIX randoms before any gate, so an item's
///     look is a function of its index alone and nothing downstream can
///     reshuffle the field;
///   * every field that varies spatially (clumping, height, senescence) is a
///     pure function of POSITION, gated on an extra hash, never on an extra
///     `next()` call.
///
/// Differences from the tree harvest, each deliberate:
///
///   * DENSITY RAMPS WITH DISTANCE (`grass_density_at`) instead of being
///     constant. Acceptance is `item_index < count * p`, which is a uniform
///     random subset because an item's position is independent of its index,
///     and it is MONOTONE in p - approaching only ever adds tillers, never
///     swaps them - so the walk can stop the moment the index passes the
///     cell's most generous possible threshold. That is what keeps the
///     harvest affordable: only accepted-ish items pay for trigonometry.
///   * GATES RIDE A LATTICE, POSITION DOES NOT. The elevation band and the
///     biome colour are yes/no questions about ground that varies over
///     hundreds of metres, so they come off a planet-fixed ~1 m lattice. The
///     standing position does not: it comes off the DRAWN patch triangle.
///   * THE BASE SITS ON THE MESH, not on the elevation field. A tiller is
///     30 cm tall and stands where you are standing, so a metre of
///     disagreement with the visible ground is the whole layer failing.
///     `DrawnPatchSurface` interpolates the resident patch's own face, which
///     is exact at any depth and any slope - see the long note there for
///     what a direct field sample gets wrong and by how much.
///
/// THE HARVEST IS A SUPERSET, and that is the design, not a slop budget.
/// Acceptance is evaluated at `(distance - margin_m)`, so every tiller ANY
/// camera position within `margin_m` of `center_dir` could want is in the
/// returned set. The caller then decides what is actually drawn, every
/// frame, from the LIVE camera distance via `grass_live_emerge`. That split
/// is what makes re-harvesting invisible: the density ramp is anchored on
/// the real camera at all times, so moving the harvest centre cannot pop,
/// thin or ring the field the way a plain hysteresis would (a tiller 6 m
/// from a new pose was 14 m from the old one, and `grass_density_at` differs
/// by 4x across that).
///
/// `depth` is the patch tree depth of the ground actually being DRAWN under
/// the camera - `lib.rs` reads it from the drawn leaf set. Getting it wrong
/// by a level costs sub-decimetre accuracy; leaving it at a fixed guess
/// costs metres on a coarse mesh.
///
/// COST, and the known follow-up: this runs INLINE on the frame thread,
/// once per `GRASS_REHARVEST_M` of camera movement. MEASURED in release at
/// the old `veg_density` 1.0, 47,000 tillers: 27 ms at Fuji depth 17, 31 ms
/// at Amazon depth 20 (roughly 16 and 18 at the shipped 0.6 density).
///
/// SINCE v0.1103 THAT COST IS THE SAME AT EVERY SLIDER POSITION, because the
/// tiller count is now derived from `GRASS_TARGET_LAI` instead of scaled by
/// the slider, and the derived peak is exactly what 0.6 used to give. So the
/// default player pays what they always paid (~16 ms), max quality got
/// CHEAPER (it was 27 ms), and the floor of the slider got dearer (it was
/// ~2.8 ms at the operator's 0.1039, on a canopy of LAI 0.58 - i.e. it was
/// cheap because there was almost no grass, which is the defect). Holding
/// leaf area constant is what holds this number constant; the slider now buys
/// its frame time back in the DRAW, not in the harvest.
///
/// That is one doubled frame every few seconds of walking - real, and the honest
/// next increment is to TIME-SLICE it: the cell walk is sorted nearest-first
/// and every cell is independent, so it can be resumed across frames with
/// the previous superset still live (which the margin already guarantees is
/// valid). A worker thread is the wrong shape here - the harvest must see
/// the same streamed tile tier the patch mesh saw, and `TerrainTiles` is
/// owned and mutated by the frame thread.
pub fn near_grass_instances(
    def: &PlanetDef,
    source: &ElevationSource,
    albedo: Option<&PlanetAlbedo>,
    center_dir: DVec3,
    far_m: f64,
    margin_m: f64,
    depth: u8,
    max_n: usize,
) -> Vec<NearGrass> {
    let mut out: Vec<NearGrass> = Vec::new();
    let center = center_dir.normalize();
    let lat_c = center.y.clamp(-1.0, 1.0).asin();
    // No grass on the caps (mirrors the tree harvest's polar gate; lon cells
    // degenerate there and the biome gate would reject the ice anyway).
    if lat_c.abs() > 1.5 {
        return out;
    }
    let lon_c = (-center.z).atan2(center.x);
    let margin_m = margin_m.clamp(0.0, grass_far_m() as f64);
    // The superset reaches `margin_m` further than the ramp does: a camera
    // that far from the harvest centre still sees the last of the ramp.
    let far_m = far_m.clamp(1.0, grass_far_m() as f64) + margin_m;
    let ang = far_m / def.radius.max(1.0);
    let cos_ang = ang.cos();
    let sea = def.sea_level.clamp(0.0, 1.0);
    let range_m = match source {
        ElevationSource::Heightmap { hm, .. } => hm.max_meters() - hm.min_meters(),
        ElevationSource::Noise(_) => 1.0,
    };
    let mut ground = DrawnPatchSurface::new(def, source, depth);
    // Pin the shared prefix of the patch walk to the whole harvest disc
    // (plus a cell of slop). Without this, every one of ~47,000 base
    // placements re-walks 15-20 tree levels that are identical for all of
    // them; with it, only the last few levels differ. MEASURED: 22.7 ms of
    // the harvest before, 6-8 ms after.
    let cell = GRASS_CELL_RAD;
    ground.set_region(center, ang + cell);
    let salt: u64 = 0x9A55_77EE_0F5A_11D5;
    // COVERAGE IS NOT A QUALITY SETTING (v0.1103): both peaks are derived from
    // `GRASS_TARGET_LAI` and `veg_density` is not read anywhere in this
    // function. Turning quality down costs blade DETAIL (`grass_detail_for`),
    // never ground cover.
    let peak_tussock = grass_peak_per_m2();
    let peak_filler = grass_filler_per_m2();
    // Metres per radian of latitude; longitude is this times cos(lat).
    let m_per_rad = def.radius.max(1.0);
    let coslat = lat_c.cos().max(0.05);

    // ── Planet-fixed ground lattice ──
    // Node (i, j) sits at lat = i * lcell, lon = j * lcell with lcell =
    // cell / GRASS_LATTICE_DIV (~1 m). Indices are ABSOLUTE, so the same
    // node carries the same value from any camera position and the field
    // cannot jiggle vertically when the harvest recentres.
    let lcell = GRASS_FIELD_RAD / GRASS_LATTICE_DIV as f64;
    let lat_span = ang / lcell;
    let lon_span = ang / (lcell * coslat);
    let nylo = (lat_c / lcell).floor() as i64 - lat_span.ceil() as i64 - 1;
    let nyhi = (lat_c / lcell).floor() as i64 + lat_span.ceil() as i64 + 2;
    let nxlo = (lon_c / lcell).floor() as i64 - lon_span.ceil() as i64 - 1;
    let nxhi = (lon_c / lcell).floor() as i64 + lon_span.ceil() as i64 + 2;
    let nw = (nxhi - nxlo + 1).max(1) as usize;
    let nh = (nyhi - nylo + 1).max(1) as usize;
    let mut nodes: Vec<GrassGroundNode> = Vec::with_capacity(nw * nh);
    for iy in nylo..=nyhi {
        let nlat = iy as f64 * lcell;
        let (clat, slat) = (nlat.cos(), nlat.sin());
        for ix in nxlo..=nxhi {
            let nlon = ix as f64 * lcell;
            let d = DVec3::new(clat * nlon.cos(), slat, -clat * nlon.sin());
            let e = match source {
                ElevationSource::Heightmap { hm, detail, tiles, .. } => {
                    // The DRAWN elevation - the same function the player's
                    // eye-height ground clamp uses - so a blade base and a
                    // boot sole are coplanar by construction.
                    drawn_elevation_normalized(hm, def, detail, *tiles, d)
                }
                ElevationSource::Noise(sm) => sm.elevation_at(d.as_vec3()),
            };
            nodes.push(GrassGroundNode {
                elev_m: (e - sea) * range_m,
                color: surface_color(def, albedo, d.as_vec3(), e),
            });
        }
    }
    // Bilinear fetch on the lattice. Out-of-range clamps rather than
    // returning None: the lattice always covers the harvest disc plus a
    // node of margin, so a clamp only ever happens on the rim.
    let node_at = |lat: f64, lon: f64| -> (f32, [f32; 3]) {
        let y = lat / lcell;
        let x = lon / lcell;
        let iy = (y.floor() as i64).clamp(nylo, nyhi - 1);
        let ix = (x.floor() as i64).clamp(nxlo, nxhi - 1);
        let fy = (y - iy as f64) as f32;
        let fx = (x - ix as f64) as f32;
        let idx = |gy: i64, gx: i64| -> usize {
            ((gy - nylo) as usize) * nw + ((gx - nxlo) as usize)
        };
        let (n00, n01) = (nodes[idx(iy, ix)], nodes[idx(iy, ix + 1)]);
        let (n10, n11) = (nodes[idx(iy + 1, ix)], nodes[idx(iy + 1, ix + 1)]);
        let bl = |a: f32, b: f32, c: f32, d: f32| {
            let t = a + (b - a) * fx;
            let u = c + (d - c) * fx;
            t + (u - t) * fy
        };
        let elev_m = bl(n00.elev_m, n01.elev_m, n10.elev_m, n11.elev_m);
        let color = [
            bl(n00.color[0], n01.color[0], n10.color[0], n11.color[0]),
            bl(n00.color[1], n01.color[1], n10.color[1], n11.color[1]),
            bl(n00.color[2], n01.color[2], n10.color[2], n11.color[2]),
        ];
        (elev_m, color)
    };

    // ── Cell walk ──
    // Nearest-first, same reason the tree harvest walks that way: `max_n` is
    // a hard cap, and a row-major walk spends it on the disc's south-west
    // corner, leaving the ground under the camera bare.
    let cspan_lat = ang / cell;
    let cspan_lon = ang / (cell * coslat);
    let cylo = (lat_c / cell).floor() as i64 - cspan_lat.ceil() as i64 - 1;
    let cyhi = (lat_c / cell).floor() as i64 + cspan_lat.ceil() as i64 + 1;
    let cxlo = (lon_c / cell).floor() as i64 - cspan_lon.ceil() as i64 - 1;
    let cxhi = (lon_c / cell).floor() as i64 + cspan_lon.ceil() as i64 + 1;
    let (cy, cx) = (lat_c / cell, lon_c / cell);
    let mut cells: Vec<(i64, i64)> =
        Vec::with_capacity((((cyhi - cylo + 1) * (cxhi - cxlo + 1)).max(0)) as usize);
    for iy in cylo..=cyhi {
        for ix in cxlo..=cxhi {
            cells.push((iy, ix));
        }
    }
    cells.sort_by(|a, b| {
        let d = |c: &(i64, i64)| {
            let dy = c.0 as f64 + 0.5 - cy;
            let dx = (c.1 as f64 + 0.5 - cx) * coslat;
            dy * dy + dx * dx
        };
        d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    // Cell area in m^2 at this latitude (lon cells narrow by cos(lat), which
    // is exactly what keeps density constant PER AREA rather than per cell).
    let cell_m = cell * m_per_rad;

    for (iy, ix) in cells {
        if out.len() >= max_n {
            break;
        }
        let cell_lat = (iy as f64 + 0.5) * cell;
        let cell_lon = (ix as f64 + 0.5) * cell;
        let cell_coslat = cell_lat.cos().max(0.0);
        // Cell-level cull: how close can this cell get to the camera? If even
        // its nearest corner is past the ramp's end, no item in it can be
        // accepted, so skip it without touching its stream (legal because
        // each cell's stream is seeded from its own coordinates - skipping a
        // cell cannot desynchronise any other).
        let dy = (cell_lat - lat_c).abs() - cell * 0.5;
        let dx = ((cell_lon - lon_c).abs() - cell * 0.5) * coslat;
        let near_rad = (dy.max(0.0).powi(2) + dx.max(0.0).powi(2)).sqrt();
        let near_m = near_rad * m_per_rad;
        if near_m >= far_m {
            continue;
        }
        let area_m2 = cell_m * cell_m * cell_coslat;
        // Acceptance is index < count * p with p = want * gain / GAIN_MAX; p
        // can never exceed `want` at the cell's nearest point, so the stream
        // can stop there instead of running the full cell. The superset
        // margin (see the fn doc) enters here as a distance discount, which
        // is what makes the bound valid for any camera within it.
        let p_ceiling = grass_ramp_at((near_m - margin_m).max(0.0) as f32).min(1.0);
        // ── TWO POPULATIONS PER CELL (v0.1093) ──
        // Class 0 = TUSSOCKS, the clumped main sward. Class 1 = FILLER
        // STUBBLE, short shoots riding the COMPLEMENT of the clump field so
        // the ground BETWEEN the tussocks is stubbled instead of bare.
        //
        // Each class runs its OWN xorshift stream from its own salt rather
        // than sharing one index space. That is not a style choice: the
        // acceptance rule is `index < count * p`, so two classes drawing from
        // one stream would NEST (the lower-threshold class's items are a
        // subset of the other's) and the stubble would land inside the
        // tussocks it is meant to fill between. Separate streams also keep
        // the tussock field BIT-IDENTICAL to a single-class harvest - the
        // planet-fixed and recentring invariants below are unchanged by this
        // increment, because class 0's stream is untouched.
        //
        // One loop body for both, so the gates, the ground sample, the colour
        // path and the superset margin cannot drift apart between them.
        for class in 0..2u32 {
            if out.len() >= max_n {
                break;
            }
            let filler = class == 1;
            // Items per cell at the THICKEST that class can be: cos(lat)-thinned
            // so the per-area density is constant, and scaled by the Settings
            // vegetation slider. For tussocks the GRASS_CLUMP_GAIN_MAX headroom
            // is what lets a clump genuinely exceed the nominal density instead
            // of saturating at it (a gain that can only ever thin would drag the
            // mean below grass_peak_per_m2 and make that number a lie); for
            // the filler class the gain is already normalized to a 1.0 ceiling.
            let (peak, gain_max) = if filler {
                (peak_filler, 1.0f32)
            } else {
                (peak_tussock, GRASS_CLUMP_GAIN_MAX)
            };
            let count = ((peak * gain_max) as f64 * area_m2).round() as u32;
            if count == 0 {
                continue;
            }
            let take = ((count as f32) * p_ceiling).ceil() as u32;
            if take == 0 {
                continue;
            }
            let mut s = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                ^ if filler { GRASS_FILLER_SALT } else { salt };
            if s == 0 {
                s = 0x94D0_49BB_1331_11EB;
            }
            let mut next = move || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            };
            for item in 0..take {
                // SIX randoms, always, before any gate - the stream discipline
                // the whole planet-fixed scheme rests on. r0/r1 place, r2 turns,
                // r3 sizes, r4 tints, r5 phases.
                let r0 = next();
                let r1 = next();
                let r2 = next();
                let r3 = next();
                let r4 = next();
                let r5 = next();
                if out.len() >= max_n {
                    break;
                }
                // 24 bits of position per axis, from the TOP of the word, not
                // the tree stream's `% 4096`. Two reasons, both measured: 4096
                // steps across an 8 m cell is 2 mm, and with ~7,500 items in a
                // cell the birthday collision rate is ~1.7 duplicate positions
                // PER CELL - two tillers with different looks standing in
                // exactly the same spot, z-fighting. (It is harmless on the
                // tree grid, where 480 items share a 220 m cell.) And
                // xorshift's low bits are its weakest; the high 24 are not.
                let lat = (iy as f64 + (r0 >> 40) as f64 / 16_777_216.0) * cell;
                let lon = (ix as f64 + (r1 >> 40) as f64 / 16_777_216.0) * cell;
                // Surface distance from the camera, small-angle (the whole disc
                // is under 30 m on a 6,371 km sphere, so the chord IS the arc).
                let ddy = lat - lat_c;
                let ddx = (lon - lon_c) * coslat;
                let d_m = ((ddy * ddy + ddx * ddx).sqrt() * m_per_rad) as f32;
                if d_m as f64 >= far_m {
                    continue;
                }
                // Density gate, clumping folded in. Both are position-keyed, so
                // no random was consumed to get here. The filler class rides
                // the COMPLEMENT of the clump field, so where this returns
                // zero for one class it is at its largest for the other and
                // the ground is never left to neither.
                let gain = if filler {
                    grass_filler_gain(lat, lon)
                } else {
                    grass_clump_gain(lat, lon)
                };
                if gain <= 0.0 {
                    continue; // bare scrape (tussocks) / solid clump (filler)
                }
                // The instance's THRESHOLD: the normalized density at which it
                // starts to exist, from its index in the cell's stream. Invert
                // the acceptance rule `item < count * (density/PEAK) * gain/GMAX`
                // to get a per-instance constant that carries no camera distance
                // at all, so the draw-time gate can re-evaluate the ramp live.
                //
                // WHY THIS IS NOT A LADDER, and why an attempt to stratify it
                // was measured and then REVERTED (v0.1103). `item / count` on
                // its own would be one: `grass_appear_distance` is monotone in
                // the threshold, so evenly-spaced thresholds are evenly-spaced
                // emergence RADII, and whole cohorts of blades would grow in
                // together at fixed distances from the camera - concentric arcs
                // of half-height grass, a plausible reading of the operator's
                // "they still look like rings". The `gain_max / gain` factor is
                // what saves it: gain is a per-ITEM function of position on a
                // field whose fine octave is 0.72 m, well inside this ~3 m cell,
                // so two items on the same index rung land at different radii.
                // MEASURED, not argued: `grass_appear_distances_are_not_a_ladder`
                // scores the spread of within-cell threshold gaps at ~0.98 as
                // shipped and 0.00 with the gain factor removed. A stratified
                // `(item + hash01)` offset was written and measured to move that
                // statistic by nothing, so it was dropped rather than shipped -
                // it would have moved every tiller in the world for no gain.
                let thr = (item as f32 / count as f32) * (gain_max / gain);
                // SUPERSET acceptance: discount the distance by the margin, so
                // any camera within it still finds this tiller in the set.
                let p_here = grass_ramp_at(((d_m as f64 - margin_m).max(0.0)) as f32);
                if thr >= p_here {
                    continue;
                }
                let cl = lat.cos();
                let dir = DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
                if dir.dot(center) < cos_ang {
                    continue;
                }
                let (elev_m, sc) = node_at(lat, lon);
                // Same gates as the trees: above the storm-surge line, below the
                // treeline placeholder, and where the imagery reads vegetated.
                // GATES ride the lattice (they are yes/no questions about ground
                // that varies over hundreds of metres); the POSITION does not.
                if elev_m < 6.0 || elev_m > TREELINE_M {
                    continue;
                }
                // BIOME AS A DENSITY WEIGHT (v0.1108), not a verdict. Folded in
                // exactly the way the clump gain is: it DIVIDES the instance's
                // threshold, so half-weight ground keeps half its tillers and
                // they emerge at half the distance. The pre-reject above still
                // used the unweighted `thr`, which is a valid conservative
                // bound because the weight can only ever raise it - that is
                // what lets the cheap distance test run before `node_at`.
                let vw = veg_biome_weight(sc);
                if vw < VEG_WEIGHT_MIN {
                    continue;
                }
                let thr = thr / vw;
                if thr >= p_here {
                    continue;
                }
                // STANDING POSITION: the DRAWN patch face, per surviving tiller.
                // Not a direct elevation sample - see the DrawnPatchSurface note.
                // Only survivors pay, and the vertex memo means neighbours in the
                // same lattice cell share their three corner samples.
                let r = ground.radius_at(dir) - GRASS_GROUND_BIAS_M;
                let hf = grass_height_field(lat, lon);
                let jitter = 0.82 + (r3 % 1000) as f32 / 1000.0 * 0.36;
                let mut height_m = ((GRASS_HEIGHT_MIN_M + GRASS_HEIGHT_MAX_M) * 0.5 * hf * jitter)
                    .clamp(GRASS_HEIGHT_MIN_M, GRASS_HEIGHT_MAX_M);
                if filler {
                    // Scaled OFF the tussock height this spot would grow, not
                    // drawn independently: a short shoot in a tall stand is
                    // still taller than a short shoot in a cropped one, and
                    // driving it from the same height field keeps the two
                    // populations agreeing about how tall the sward is here.
                    // Different bits of r3 than the tussock jitter above, so
                    // the two are independent without a seventh random (the
                    // fixed-six stream discipline).
                    let f = GRASS_FILLER_HEIGHT_LO
                        + ((r3 >> 24) % 1000) as f32 / 1000.0
                            * (GRASS_FILLER_HEIGHT_HI - GRASS_FILLER_HEIGHT_LO);
                    height_m *= f;
                }
                // Colour: the ground it grows in, not a planet-wide constant.
                // Lifted a little (a near-vertical leaf catches more sky than the
                // horizontal ground beside it, and the sward the imagery averages
                // is darker than the blades that make it), jittered per tiller
                // the way moisture and species mix vary between clumps, and
                // pulled toward straw for the senescent fraction.
                let jl = 0.88 + (r4 % 1000) as f32 / 1000.0 * 0.30;
                let sen = grass_senescence(lat, lon, r4 >> 20);
                let live = [
                    (sc[0] * 1.18 * jl).clamp(0.0, 1.0),
                    (sc[1] * 1.30 * jl).clamp(0.0, 1.0),
                    (sc[2] * 1.05 * jl).clamp(0.0, 1.0),
                ];
                // Straw is derived FROM the live colour, not a fixed bright hay
                // yellow: dead tissue in a meadow is the same brightness as the
                // live tissue beside it, just yellower and less saturated. A
                // constant straw would reintroduce the exact defect this tint
                // exists to fix - grass brighter than the ground it grows in.
                let l = live[0] * 0.30 + live[1] * 0.59 + live[2] * 0.11;
                let straw = [l * 1.35, l * 1.15, l * 0.40];
                let color = [
                    live[0] + (straw[0] - live[0]) * sen,
                    live[1] + (straw[1] - live[1]) * sen,
                    live[2] + (straw[2] - live[2]) * sen,
                ];
                out.push(NearGrass {
                    dir,
                    r_m: r,
                    yaw: (r2 % 6283) as f32 / 1000.0,
                    height_m,
                    color,
                    phase: (r5 % 6283) as f32 / 1000.0,
                    thr,
                    filler,
                });
            }
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::planet_chunks::{
        build_patch_mesh, patch_corners, patch_edge_arc_m, tests::earth_like, DetailNoise, PatchId,
        PatchMesh, PATCH_TESS,
    };
    // Both were module-level imports until DrawnPatchSurface moved out
    // (v0.1097); only the tests still need them here.
    use super::super::planet_surface::displaced_radius_f64;

    // ══ Grass strands (v0.1090) ═══════════════════════════════════════════

    /// Load the shipped Earth grids + a def with the real sea level, the same
    /// way `near_tree_harvest_finds_trees_in_real_forests` does. Every grass
    /// test runs against REAL data: the whole point of the layer is that it
    /// grows where the imagery says grass grows.
    fn real_earth() -> (
        crate::terrain::planet_heightmap::PlanetHeightmap,
        crate::terrain::planet_albedo::PlanetAlbedo,
        PlanetDef,
    ) {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = crate::terrain::planet_heightmap::PlanetHeightmap::load(
            &base.join("earth_heightmap.bin"),
        )
        .expect("earth heightmap loads");
        let albedo =
            crate::terrain::planet_albedo::PlanetAlbedo::load(&base.join("earth_albedo.bin"))
                .expect("earth albedo loads");
        let mut def = earth_like();
        def.sea_level = hm.sea_level_normalized();
        (hm, albedo, def)
    }

    fn dir_of(lat_deg: f64, lon_deg: f64) -> DVec3 {
        let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
        let cl = lat.cos();
        DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin())
    }

    const BIOME_EDGE_LAT: f64 = -1.802;
    const BIOME_EDGE_LON: f64 = 32.915;

    /// THE FUJI STABILITY CHECK the brief asked for: 8 parked poses spread
    /// over ~50 m must not disagree about how much grass is underfoot.
    ///
    /// REPORTED HONESTLY: this one passes BEFORE the v0.1108 weight as well as
    /// after, and that is itself the finding. The brief expected it to fail on
    /// the strength of rig logs showing 0 tillers at one parked Fuji pose and
    /// 5,930 at the next, against 31,635-32,958 elsewhere. Measured here, the
    /// Fuji vantage is nowhere near the biome threshold: over a 1,150 m walk
    /// the r/g ratio runs 0.4196..0.4311 against a 1.25 cut, g/b runs
    /// 6.49..6.43 against 1.04, ground elevation runs 1,140..1,234 m against a
    /// 1,700 m treeline, and the harvest returns 30,062..33,792 tillers - a
    /// 1.12x spread, not 3x and certainly not infinity. So the biome gate did
    /// NOT cause that rig variance and something else did; the vantage's own
    /// file is the lead, documenting a 75 s settle ("at 25 s this vantage
    /// captures a half-built forest") and player rest altitudes of
    /// 1240/1364/1376/1424 m across four runs of a byte-identical config.
    ///
    /// It stays as a REGRESSION gate: the weight must not destabilise the core
    /// of a forest, only its edges.
    #[test]
    fn grass_density_is_stable_across_poses_at_one_vantage() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // 8 poses over ~50 m: a 4x2 grid at 16 m spacing about the vantage.
        let mut counts = Vec::new();
        for i in 0..8 {
            let dn = (i % 4) as f64 * 16.0;
            let de = (i / 4) as f64 * 16.0;
            let lat = 35.3 + dn / 111_320.0;
            let lon = 138.8 + de / (111_320.0 * lat.to_radians().cos());
            let c = dir_of(lat, lon);
            let g = near_grass_instances(
                &def,
                &src,
                Some(&albedo),
                c,
                GRASS_FAR_M as f64,
                GRASS_HARVEST_MARGIN_M,
                17,
                80_000,
            );
            counts.push(g.len());
        }
        let lo = *counts.iter().min().unwrap();
        let hi = *counts.iter().max().unwrap();
        println!("[grass pose spread] fuji, 8 poses over ~50 m: {counts:?} (lo {lo}, hi {hi})");
        assert!(
            lo > 0 && (hi as f64) < lo as f64 * 3.0,
            "the sward under one vantage disagrees by {:.1}x across 50 m of parking: {counts:?}",
            hi as f64 / lo.max(1) as f64
        );
    }

    /// THE CHECK THAT CAN FAIL, and the one this increment exists for: at a
    /// real biome boundary the grass density must be a GRADIENT, not a cliff.
    ///
    /// FAILS ON PRE-v0.1108 CODE. `veg_biome_ok` was a hard threshold on a
    /// 9.78 km-per-texel colour field, so an entire ~30 m harvest disc gets a
    /// single verdict and the sward goes from a full sward to literally zero
    /// between two poses 60 m apart. Measured on the old code at the transect
    /// below the walk reads 32,061 -> 0 tillers in one step (an infinite
    /// ratio); with the weight it steps down through the intermediate values a
    /// real ecotone has.
    ///
    /// The transect is a MEASURED crossing, not a guessed one: the sweep in
    /// the commit notes found 13,966 adjacent land texel pairs that straddle
    /// the old cut with healthy green/blue on both sides, and this is one of
    /// them, chosen because its elevation sits clear of both the 6 m beach
    /// floor and the 1,700 m treeline so no other gate can confound it.
    #[test]
    fn vegetation_edges_are_gradients_not_cliffs() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let range_m = hm.max_meters() - hm.min_meters();
        let sea = def.sea_level.clamp(0.0, 1.0);
        // Walk east across the contour in 24 steps of ~380 m.
        let (lat, lon0) = (BIOME_EDGE_LAT, BIOME_EDGE_LON);
        let mut counts = Vec::new();
        for i in 0..24 {
            let lon = lon0 + (i as f64 * 380.0) / (111_320.0 * lat.to_radians().cos());
            let c = dir_of(lat, lon);
            let e = drawn_elevation_normalized(&hm, &def, &detail, None, c);
            let elev_m = (e - sea) * range_m;
            assert!(
                (6.0..TREELINE_M).contains(&elev_m),
                "transect step {i} left the elevation band at {elev_m:.0} m - pick another \
                 crossing, this test must isolate the BIOME gate"
            );
            let g = near_grass_instances(
                &def,
                &src,
                Some(&albedo),
                c,
                GRASS_FAR_M as f64,
                GRASS_HARVEST_MARGIN_M,
                17,
                80_000,
            );
            counts.push(g.len());
        }
        println!("[grass ecotone] {lat:.3} lon {lon0:.3} east over 8.7 km: {counts:?}");
        // The transect must actually SPAN the edge, or it proves nothing.
        let hi = *counts.iter().max().unwrap();
        let lo = *counts.iter().min().unwrap();
        assert!(
            hi > 5_000 && lo * 4 < hi,
            "this transect does not cross a biome edge at all ({lo}..{hi}) - it cannot \
             detect a cliff, so re-derive it before trusting a pass"
        );
        // THE CLAIM: crossing the edge takes more than one step. Count how many
        // poses land in the interior of the range - a cliff has none.
        let graded_steps = counts
            .iter()
            .filter(|&&n| n as f64 > hi as f64 * 0.10 && (n as f64) < hi as f64 * 0.90)
            .count();
        assert!(
            graded_steps >= 3,
            "the sward falls off a cliff: only {graded_steps} of 24 poses sit between 10% and \
             90% of peak density ({counts:?}). A biome edge is an ecotone kilometres wide, and \
             the imagery texel that decides it is 9.78 km across - a hard threshold on that \
             field switches the whole world off along a line, which is the operator's \
             'weird chunks that wouldn't spawn trees'"
        );
    }

    /// The patch of `depth` whose spherical triangle contains `dir`: descend
    /// from the root face, taking whichever child covers the direction.
    /// Surface distance from a harvest/camera centre to a tiller, in metres.
    /// The SAME small-angle chord measure `near_grass_instances` accepts on
    /// and `lib.rs` re-gates on every frame, so a test that uses anything
    /// else is measuring a different field than the one that draws.
    fn surf_d(def: &PlanetDef, center: DVec3, dir: DVec3) -> f32 {
        ((dir - center.normalize()).length() * def.radius) as f32
    }

    fn patch_containing(dir: DVec3, depth: u8) -> PatchId {
        let inside = |id: &PatchId, d: DVec3| -> bool {
            let c = patch_corners(id);
            let cn = [c[0].normalize(), c[1].normalize(), c[2].normalize()];
            let en = [cn[0].cross(cn[1]), cn[1].cross(cn[2]), cn[2].cross(cn[0])];
            let es = [en[0].dot(cn[2]), en[1].dot(cn[0]), en[2].dot(cn[1])];
            (0..3).all(|i| en[i].dot(d) * es[i] >= 0.0)
        };
        let d = dir.normalize();
        let mut id = (0..20u8)
            .map(PatchId::root)
            .find(|r| inside(r, d))
            .expect("some root face contains the direction");
        while id.depth < depth {
            let mut next = None;
            for c in 0..4u32 {
                let ch = id.child(c);
                if inside(&ch, d) {
                    next = Some(ch);
                    break;
                }
            }
            id = next.expect("some child contains the direction");
        }
        id
    }

    /// Where a ray from the planet centre along `dir` crosses the built patch
    /// GROUND, in metres of radius. Moller-Trumbore; the mesh is small enough
    /// (a few thousand faces) that brute force is the right amount of
    /// cleverness for a test.
    ///
    /// GRID FACES ONLY - the first `PATCH_TESS^2` triangles, before the
    /// vegetation cards and the skirt. The skirt is a near-RADIAL apron, so a
    /// ray from the planet centre runs almost parallel to it and grazes it at
    /// an arbitrary distance; including it produced 0.6-1.5 m phantom
    /// "floating" readings on strands near the patch border.
    fn drawn_radius_along(pm: &PatchMesh, dir: DVec3) -> Option<f64> {
        let d = dir.normalize();
        let v = |i: u32| -> DVec3 {
            let p = pm.mesh.vertices[i as usize].position;
            pm.anchor + DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
        };
        let grid_idx = (PATCH_TESS * PATCH_TESS * 3) as usize;
        let mut best: Option<f64> = None;
        for tri in pm.mesh.indices[..grid_idx].chunks_exact(3) {
            let (a, b, c) = (v(tri[0]), v(tri[1]), v(tri[2]));
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-12 {
                continue;
            }
            let inv = 1.0 / det;
            let s = -a;
            let u = inv * s.dot(h);
            if !(-1e-9..=1.0 + 1e-9).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let w = inv * d.dot(q);
            if w < -1e-9 || u + w > 1.0 + 1e-9 {
                continue;
            }
            let t = inv * e2.dot(q);
            if t > 0.0 && best.map_or(true, |bt| t > bt) {
                best = Some(t);
            }
        }
        best
    }

    /// THE GROUND-CONTACT GATE. A tiller is 24-52 cm tall and stands exactly
    /// where the player stands, so "close enough" is a few centimetres, not a
    /// few metres. Measured against the REAL drawn patch mesh - a ray from the
    /// planet centre through each strand, intersected with the patch's grid
    /// faces - never against the elevation field the strand came from.
    ///
    /// WHERE THIS GATE IS SET, and why it moved (v0.1091):
    ///
    ///  1. IT GATES AT FUJI, AT THE DEPTH THE SELECTOR ACTUALLY REACHES. The
    ///     first version of this test gated only at AMAZON at depth 20 - flat
    ///     ground on the finest possible mesh, i.e. the one combination that
    ///     could not fail. It printed the interesting rows and asserted on
    ///     none of them: Fuji at depth 17 measured p95 +1.060 m with 23.8% of
    ///     tillers buried whole, and nothing turned red. Fuji IS the vantage
    ///     the grass regression exists for, and 17 IS where its LOD stops.
    ///
    ///  2. THE FIX WAS NOT A BIAS, IT WAS THE REFERENCE SURFACE. A direct
    ///     elevation sample is not the ground you can see: the mesh samples
    ///     the f32 base-heightmap staircase at ITS lattice (0.84 m at depth
    ///     17) and interpolates linearly between those samples, while a
    ///     direct sample lands on whatever ~1.4 m tread it happens to hit.
    ///     `DrawnPatchSurface` interpolates the drawn face instead, which is
    ///     exact at any depth and any slope, so the residual here is f64
    ///     rounding plus `GRASS_GROUND_BIAS_M`.
    ///
    ///  3. BURIED IS MEASURED AT HALF A BLADE, not a whole one. A tiller sunk
    ///     by its full height is invisible; one sunk by half is a visibly
    ///     stunted patch, and a field of them reads as bald ground.
    #[test]
    fn grass_bases_sit_on_the_drawn_surface() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // (name, lat, lon, depth). The depth is the one the LOD selector
        // really reaches at that site, measured in the running game:
        // Fuji's 1,224 m of elevation caps it at 17 (that is terrain ticket
        // T1, filed separately); the Amazon floodplain reaches 20.
        let sites = [("fuji", 35.3_f64, 138.8_f64, 17u8), ("amazon", -3.0, -60.0, 20)];
        for (name, lat, lon, depth) in sites {
            let center = dir_of(lat, lon);
            let g = near_grass_instances(&def, &src, Some(&albedo), center, 12.0, 0.0, depth, 4000);
            assert!(
                g.len() >= 50,
                "{name}: only {} strands - the harvest is not producing a sward",
                g.len()
            );
            let id = patch_containing(center, depth);
            let pm = build_patch_mesh(&def, &src, Some(&albedo), &id);
            let mut errs: Vec<f64> = Vec::new();
            let (mut sunk, mut hit) = (0, 0);
            for t in g.iter().take(400) {
                if let Some(r) = drawn_radius_along(&pm, t.dir) {
                    let off = t.r_m - r; // + = floating, - = buried
                    errs.push(off);
                    hit += 1;
                    if off < -(t.height_m as f64) * 0.5 {
                        sunk += 1;
                    }
                }
            }
            assert!(
                errs.len() >= 50,
                "{name}: only {} of {} strands hit the depth-{depth} patch",
                errs.len(),
                g.len()
            );
            errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mean = errs.iter().sum::<f64>() / errs.len() as f64;
            let med = errs[errs.len() / 2];
            let p95 = errs[errs.len() * 95 / 100];
            let sunk_frac = sunk as f64 / hit as f64;
            println!(
                "[grass ground] {name} depth {depth} ({:.2} m triangles): n={} \
                 mean {:+.3}  median {:+.3}  p95 {:+.3}  max {:+.3}  min {:+.3}  \
                 buried-past-half {:.1}%",
                patch_edge_arc_m(depth, def.radius) / PATCH_TESS as f64,
                errs.len(),
                mean,
                med,
                p95,
                errs.last().unwrap(),
                errs[0],
                sunk_frac * 100.0
            );
            // A blade is 24-52 cm tall: floating by more than a third of the
            // shortest one is a visibly hovering mat. FLOATING is the failure
            // that reads as a bug, so it gets the hard percentile bound.
            assert!(
                p95 < 0.30,
                "{name} (depth {depth}): 5% of strands float more than {p95:.3} m above the \
                 drawn ground (median {med:+.3}) - the base is no longer being taken from \
                 the drawn patch face, or GRASS_GROUND_BIAS_M ({GRASS_GROUND_BIAS_M}) drifted"
            );
            assert!(
                med.abs() < 0.15,
                "{name} (depth {depth}): the whole sward sits {med:+.3} m off the ground - \
                 that is a systematic offset, not sampling noise"
            );
            assert!(
                sunk_frac <= 0.0,
                "{name} (depth {depth}): {:.1}% of strands are buried past half their height \
                 - a stunted patch reads as bald ground",
                sunk_frac * 100.0
            );
        }
    }

    /// The reference surface is the WHOLE fix, so prove the two references
    /// really do disagree by metres on a coarse mesh. Without this, someone
    /// "simplifying" `DrawnPatchSurface` back to a direct elevation sample
    /// would only see the gate above go red with no explanation of why.
    #[test]
    fn drawn_patch_surface_beats_a_direct_elevation_sample_on_a_coarse_mesh() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let center = dir_of(35.3, 138.8); // Fuji's flank
        let depth = 17u8;
        let id = patch_containing(center, depth);
        let pm = build_patch_mesh(&def, &src, Some(&albedo), &id);
        let mut surf = DrawnPatchSurface::new(&def, &src, depth);
        // A 12 m transect at 20 cm steps, i.e. blade spacing.
        let mut worst_direct = 0.0f64;
        let mut worst_interp = 0.0f64;
        let mut n = 0;
        let east = DVec3::new(-center.z, 0.0, center.x).normalize();
        for i in 0..60 {
            let dir = (center + east * (i as f64 * 0.2 / def.radius)).normalize();
            let Some(mesh_r) = drawn_radius_along(&pm, dir) else { continue };
            let e = drawn_elevation_normalized(&hm, &def, &detail, None, dir);
            let direct_r = def.radius * displaced_radius_f64(&def, e as f64);
            worst_direct = worst_direct.max((direct_r - mesh_r).abs());
            worst_interp = worst_interp.max((surf.radius_at(dir) - mesh_r).abs());
            n += 1;
        }
        assert!(n >= 30, "only {n} transect samples hit the patch");
        println!(
            "[grass ground ref] fuji depth {depth}: worst |direct - mesh| {worst_direct:.3} m, \
             worst |interpolated - mesh| {worst_interp:.4} m over {n} samples \
             ({} lattice elevation samples for {n} queries)",
            surf.samples
        );
        assert!(
            worst_interp < 0.01,
            "the interpolated surface is {worst_interp:.4} m off the mesh it is supposed to BE - \
             the lattice walk or the sub-triangle pick has drifted from build_patch_mesh"
        );
        assert!(
            worst_direct > 0.5,
            "a direct elevation sample is only {worst_direct:.3} m off the drawn mesh here, so \
             the coarse-mesh staircase this machinery exists for is gone - re-derive the note \
             on DrawnPatchSurface before deleting anything"
        );
    }

    /// CI TWIN for the "there is no grass where you stand" finding: the layer
    /// must deliver a real sward's leaf area, not a token scatter. Measured
    /// off the emitter AND the real shared mesh - no hand arithmetic, because
    /// the layer the bake shipped was 60-180x short and its own comments
    /// claimed otherwise.
    ///
    /// Reference: measured turfgrass canopies run LAI 1.9-6.0 and pass only
    /// 37% down to 0.2% of PAR to the soil (PLOS One 10.1371/journal.pone.0188080).
    /// Grass is erectophile (extinction coefficient ~0.3-0.5), so it needs
    /// MORE leaf area than a horizontal-leaf canopy for the same interception.
    #[test]
    fn near_grass_density_matches_a_real_sward() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // Cost is quoted at the SHIPPED DEFAULT rung, not at whatever the test
        // process's atomic holds, so the printed triangle count is the one the
        // default player pays.
        let (_, stats) = grass_tiller_mesh_at(grass_detail_for(0.6));
        // FULL-COST harvest, exactly as lib.rs runs it: the whole ramp plus
        // the superset margin, at the depth each site's LOD really reaches.
        // Printed, not asserted - debug and release differ by more than an
        // order of magnitude - but this is the number the caller budgets
        // against, and it runs INLINE on the frame thread at most once per
        // GRASS_REHARVEST_M of walking. If it ever grows past a frame's
        // worth, move it to a worker (which then needs its own view of the
        // streamed tile tier - see the note on near_grass_instances).
        for (name, lat, lon, depth) in [
            ("fuji", 35.3_f64, 138.8_f64, 17u8),
            ("amazon", -3.0, -60.0, 20),
        ] {
            let c = dir_of(lat, lon);
            let t0 = std::time::Instant::now();
            let g = near_grass_instances(
                &def,
                &src,
                Some(&albedo),
                c,
                GRASS_FAR_M as f64,
                GRASS_HARVEST_MARGIN_M,
                depth,
                80_000,
            );
            let vis: Vec<&NearGrass> = g
                .iter()
                .filter(|t| grass_live_emerge(t.thr, surf_d(&def, c, t.dir)) > 0.0)
                .collect();
            let drawn = vis.len();
            let fillers = vis.iter().filter(|t| t.filler).count();
            // TRIANGLE BUDGET (v0.1093). The filler class draws the SAME
            // shared mesh as a tussock - one mesh, one draw - so it costs a
            // full tiller's triangles however small it is drawn, and its
            // share of the drawn set IS its share of the added cost. The
            // v0.1092 baseline at these two sites was 1,991,070 and
            // 1,884,240 triangles; the ~13% the filler class adds is the
            // whole delta, because the tussock mesh's triangle count did not
            // change (still GRASS_BLADES_PER_TILLER * 10).
            println!(
                "[grass cost] {name} depth {depth}: {} instances harvested (superset), \
                 {drawn} drawn ({} tussocks + {fillers} filler, {:.0}% filler), \
                 {} triangles drawn, {:.1} ms (this build profile)",
                g.len(),
                drawn - fillers,
                fillers as f64 / drawn.max(1) as f64 * 100.0,
                drawn * stats.triangles,
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }
        // ── THE WHOLE SLIDER, not one position (v0.1103) ──
        //
        // The gate this replaces asserted the band at EXACTLY ONE setting: it
        // measured LAI at whatever `veg_density` the test process happened to
        // hold, then scaled by `0.6 / dens` on the (then correct) assumption
        // that density was exactly linear in the slider. That is precisely why
        // the defect shipped - it validated the default and said nothing about
        // the 0.1039 the operator was actually running, where the same
        // arithmetic gives LAI 0.58 and the ground reads as spikes on a lawn.
        //
        // Coverage no longer depends on the slider at all, so the harvest runs
        // ONCE per site and the loop re-measures leaf area against the mesh
        // each rung would build. Both halves of the decoupling are under test:
        // the realised-density check catches a harvest that reads the slider
        // again, and the LAI band catches a rung whose mesh stops conserving
        // leaf area.
        for (name, lat, lon) in [
            ("fuji", 35.3_f64, 138.8_f64),
            ("amazon", -3.0, -60.0),
            ("black_forest", 48.2, 8.2),
        ] {
            let c = dir_of(lat, lon);
            let g = near_grass_instances(
                &def,
                &src,
                Some(&albedo),
                c,
                GRASS_NEAR_M as f64,
                0.0,
                20,
                40_000,
            );
            // Everything inside GRASS_NEAR_M is at peak density, so the disc
            // area is the denominator.
            let area = std::f64::consts::PI * (GRASS_NEAR_M as f64).powi(2);
            let n = g.len() as f64;
            let nf = g.iter().filter(|t| t.filler).count() as f64;
            let tillers_m2 = n / area;
            // The height^2 sum is the site's whole contribution to LAI that
            // does NOT depend on the mesh: pull it out once, then each rung is
            // one multiply. The height is the LIVE one, exactly what the
            // renderer scales by.
            let h2: f64 = g
                .iter()
                .map(|t| {
                    let h =
                        (t.height_m * grass_live_emerge(t.thr, surf_d(&def, c, t.dir))) as f64;
                    h * h
                })
                .sum();
            // COVERAGE IS THE DERIVED QUANTITY and must match what
            // `grass_peak_per_m2` claims, within the clump field's site-to-site
            // spread (its mean is 1.0 by construction, but one 6 m disc can sit
            // on thick or thin ground).
            let want_t = (grass_peak_per_m2() + grass_filler_per_m2() * 0.615) as f64;
            assert!(
                tillers_m2 > want_t * 0.75 && tillers_m2 < want_t * 1.30,
                "{name}: {tillers_m2:.1} instances/m2 against the {want_t:.1} the target LAI \
                 derives - coverage has drifted off the authored number"
            );
            let mut lo = f64::MAX;
            let mut hi = 0.0f64;
            let mut q = GRASS_QUALITY_MIN;
            while q <= 1.0001 {
                let (_, ms) = grass_tiller_mesh_at(grass_detail_for(q));
                // LAI: one-sided leaf area per unit ground. The mesh is built
                // at unit height, so an instance carries area * height^2.
                let lai = ms.one_sided_area_unit as f64 * h2 / area;
                let blades_m2 = tillers_m2 * ms.blades as f64;
                println!(
                    "[grass sward] {name} @ quality {q:.2}: {n} instances ({:.0} tussocks + \
                     {nf} filler), {tillers_m2:.1}/m2, {} blades x {} segs, {blades_m2:.0} \
                     blades/m2, {:.0} tris/m2, LAI {lai:.2}",
                    n - nf,
                    ms.blades,
                    ms.segments,
                    tillers_m2 * ms.triangles as f64
                );
                // A SANITY FLOOR, not the coverage gate. Blade COUNT is
                // allowed to fall with quality now - that is the exchange, and
                // the width rises to match - so the quantity that must hold is
                // the LAI below. This still catches the class of defect the
                // deleted bake was (0.5 blades/m^2, 200x under this) without
                // re-asserting the coupling this increment removed.
                assert!(
                    blades_m2 >= 100.0,
                    "{name} @ quality {q:.2}: {blades_m2:.0} blades/m2 - a sward is a mat, not \
                     a scatter (the deleted bake managed 0.5)"
                );
                assert!(
                    (1.5..=5.0).contains(&lai),
                    "{name} @ quality {q:.2}: LAI {lai:.2}, wanted 1.5..5.0 against the \
                     measured turf range 1.9-6.0. A QUALITY setting is changing how much \
                     canopy there is again - it may only change how finely it is built"
                );
                lo = lo.min(lai);
                hi = hi.max(lai);
                q += 0.05;
            }
            // And the SPREAD, which is the actual claim: the authored canopy
            // is invariant under the slider, not merely in band at each end.
            assert!(
                hi < lo * 1.05,
                "{name}: LAI runs {lo:.2}..{hi:.2} across the slider - that is a {:.0}% swing \
                 in ground cover from a QUALITY setting",
                (hi / lo - 1.0) * 100.0
            );
        }
        // THE STRUCTURAL HALF, because the loop above cannot see it: the
        // harvest must not read the quality slider at all. Every number it uses
        // is derived from GRASS_TARGET_LAI, and the test process runs at the
        // atomic's 1.0 default, so a re-introduced slider multiply would be
        // invisible to any measurement made here.
        let src_txt = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terrain/grass.rs"),
        )
        .expect("read grass.rs");
        // The needle is assembled at runtime so this scanner's own source line
        // is not an offender - the first draft of it flagged itself.
        //
        // v0.1106: the forbidden name is now the QUALITY knob specifically.
        // `grass_density()` IS allowed here and is read by `grass_peak_per_m2`
        // on purpose - it is the coverage control, not a quality setting. What
        // must never come back is coverage following a knob whose job is to buy
        // frames, which is what `grass_detail` is for.
        let needle = format!("{}{}", "grass_", "detail(");
        let offenders: Vec<usize> = src_txt
            .lines()
            .enumerate()
            .filter(|(_, l)| {
                let t = l.trim_start();
                !t.starts_with("//") && t.contains(&needle)
            })
            .map(|(i, _)| i + 1)
            .collect();
        assert!(
            offenders.is_empty(),
            "the quality slider is read in the coverage path at grass.rs line(s) {offenders:?} \
             - coverage is derived from GRASS_TARGET_LAI, and the slider belongs in \
             grass_detail_for and nowhere else"
        );
    }

    /// CI TWIN for "grass is scattered as an exact Poisson process": the bake
    /// drew two independent uniforms per tuft, whose quadrat counts have a
    /// variance-to-mean ratio of exactly 1.0 by construction, so even at the
    /// right density it would have read as uniform noise rather than as a
    /// meadow. Real grass is patchy at 0.5-5 m and carries bare scrapes.
    #[test]
    fn grass_scatter_is_clustered_not_poisson() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // Pool 1 m^2 quadrats from several discs 300 m apart, all inside the
        // flat part of the density ramp so the ramp itself contributes no
        // variance. One disc alone gives ~95 quadrats, which is too few to
        // separate clustering from sampling noise.
        let mut counts: Vec<u32> = Vec::new();
        let mut tussock_counts: Vec<u32> = Vec::new();
        let mut filler_counts: Vec<u32> = Vec::new();
        let mut sites = 0;
        for i in 0..3 {
            for j in 0..3 {
                let c = dir_of(-3.0 + i as f64 * 0.003, -60.0 + j as f64 * 0.003);
                let g = near_grass_instances(
                    &def,
                    &src,
                    Some(&albedo),
                    c,
                    GRASS_NEAR_M as f64,
                    0.0,
                    20,
                    40_000,
                );
                if g.len() < 200 {
                    continue; // not a vegetated site; the gates rejected it
                }
                sites += 1;
                let up = c.normalize();
                let east = DVec3::Y.cross(up).normalize();
                let north = up.cross(east).normalize();
                // 11x11 metre-square quadrats centred on the pose, clipped to
                // a 5.5 m radius so every quadrat is fully inside the disc.
                let mut grid = [[0u32; 11]; 11];
                let mut tgrid = [[0u32; 11]; 11];
                let mut fgrid = [[0u32; 11]; 11];
                for t in &g {
                    let rel = (t.dir - up) * def.radius;
                    let (e, n) = (rel.dot(east), rel.dot(north));
                    if e.abs() >= 5.5 || n.abs() >= 5.5 {
                        continue;
                    }
                    let (qy, qx) = ((n + 5.5) as usize, (e + 5.5) as usize);
                    grid[qy][qx] += 1;
                    if t.filler {
                        fgrid[qy][qx] += 1;
                    } else {
                        tgrid[qy][qx] += 1;
                    }
                }
                for (gy, row) in grid.iter().enumerate() {
                    for (gx, v) in row.iter().enumerate() {
                        let (dy, dx) = (gy as f64 - 5.0, gx as f64 - 5.0);
                        if dy * dy + dx * dx <= 25.0 {
                            counts.push(*v);
                            tussock_counts.push(tgrid[gy][gx]);
                            filler_counts.push(fgrid[gy][gx]);
                        }
                    }
                }
            }
        }
        assert!(sites >= 4, "only {sites} vegetated sites sampled - test is not measuring");
        let n = counts.len() as f64;
        let stat = |c: &[u32]| -> (f64, f64, f64) {
            let m = c.iter().map(|v| *v as f64).sum::<f64>() / n;
            let v = c.iter().map(|x| (*x as f64 - m).powi(2)).sum::<f64>() / (n - 1.0);
            (m, v, v / m.max(1e-6))
        };
        let (mean, var, vmr) = stat(&counts);
        let (tmean, _tvar, tvmr) = stat(&tussock_counts);
        let (fmean, _fvar, _fvmr) = stat(&filler_counts);
        let empty = counts.iter().filter(|c| **c == 0).count() as f64 / n;
        let tempty = tussock_counts.iter().filter(|c| **c == 0).count() as f64 / n;
        println!(
            "[grass clumping] {} quadrats over {sites} sites: mean {mean:.1}/m2 \
             ({tmean:.1} tussock + {fmean:.1} filler), var {var:.1}, variance-to-mean \
             {vmr:.2} (tussocks alone {tvmr:.2}), {:.1}% empty ({:.1}% with no tussock)",
            counts.len(),
            empty * 100.0,
            tempty * 100.0
        );
        // The gate is on the COMBINED field, because that is what the eye
        // sees. It survives the filler class comfortably (v0.1092 measured
        // 15.85 with tussocks alone; the filler class rides the complement of
        // the same field, so it flattens the field slightly rather than
        // erasing its structure) - if this ever approaches 2.0, the two
        // populations have cancelled each other into a uniform mat, which is
        // the static-noise look the clumping exists to avoid.
        assert!(
            vmr >= 2.0,
            "variance-to-mean {vmr:.2} - an exact Poisson process scores 1.0, which is what \
             the deleted bake produced. The field must have visibly thicker and thinner \
             patches."
        );
        // Sanity on the other side: the clump gain is meant to have mean 1, so
        // the realised TUSSOCK density must still be the one grass_peak_per_m2
        // claims. Measured on the tussock class alone - the filler class is a
        // second population with its own constant and would otherwise inflate
        // this into a false pass.
        let want = grass_peak_per_m2() as f64;
        assert!(
            tmean > want * 0.75 && tmean < want * 1.25,
            "mean {tmean:.1} tussocks/m2 against the nominal {want:.1} - grass_clump_gain's \
             mean has drifted off 1.0, so grass_peak_per_m2 no longer means what it says"
        );
        // And the filler class must land near ITS constant: the complement of
        // a mean-1.0 clump field averages (GAIN_MAX - 1)/GAIN_MAX = 0.615, so
        // the realised stubble density is 61.5% of grass_filler_per_m2.
        let want_f = grass_filler_per_m2() as f64
            * ((GRASS_CLUMP_GAIN_MAX - 1.0) / GRASS_CLUMP_GAIN_MAX) as f64;
        assert!(
            fmean > want_f * 0.7 && fmean < want_f * 1.3,
            "mean {fmean:.1} filler/m2 against the expected {want_f:.1} - grass_filler_gain \
             is no longer the complement of the clump field"
        );
    }

    /// THE BARE-GROUND GATE (v0.1093). A clumped field is bare BETWEEN its
    /// clumps - that is what clumping means - and at a clump gain of 0.2 the
    /// tussock density is a fifth of nominal, which the eye reads as dirt with
    /// bouquets standing on it. The filler class exists to stubble exactly
    /// that ground, so this measures the three things that have to be true of
    /// it: it lands where the tussocks are NOT, it is short, and it actually
    /// closes bare ground.
    ///
    /// COVERAGE IS MEASURED, not assumed: every instance is stamped as a disc
    /// of `GrassTillerStats::footprint_unit * height_m` (the widest vertex of
    /// the real mesh, scaled by the instance's real height) onto a 2 cm grid,
    /// once with tussocks alone and once with both classes. That is a model of
    /// a blade canopy, not a render, but it is a model built from the shipped
    /// geometry, and the DIFFERENCE between the two runs is what the class is
    /// for.
    #[test]
    fn grass_filler_stubble_lands_where_the_tussocks_thin() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let (_, stats) = grass_tiller_mesh();
        let c = dir_of(-3.0, -60.0);
        let g = near_grass_instances(
            &def,
            &src,
            Some(&albedo),
            c,
            GRASS_NEAR_M as f64,
            0.0,
            20,
            60_000,
        );
        assert!(g.len() > 2_000, "only {} instances - not a sward to measure", g.len());
        let (fill, tuss): (Vec<&NearGrass>, Vec<&NearGrass>) = g.iter().partition(|t| t.filler);
        assert!(!fill.is_empty(), "the filler class emitted nothing at all");
        let frac = fill.len() as f64 / g.len() as f64;

        // ── 1. IT LANDS IN THE GAPS ──
        // The clump gain at each instance's own position: tussocks are drawn
        // in proportion to it, filler in proportion to its complement, so the
        // two means must separate clearly.
        let gain_at = |t: &NearGrass| {
            let d = t.dir.normalize();
            grass_clump_gain(d.y.clamp(-1.0, 1.0).asin(), (-d.z).atan2(d.x)) as f64
        };
        let mean_of = |v: &[&NearGrass], f: &dyn Fn(&NearGrass) -> f64| -> f64 {
            v.iter().map(|t| f(t)).sum::<f64>() / v.len().max(1) as f64
        };
        let gt = mean_of(&tuss, &gain_at);
        let gf = mean_of(&fill, &gain_at);
        // ── 2. IT IS SHORT ──
        let ht = mean_of(&tuss, &|t| t.height_m as f64);
        let hf = mean_of(&fill, &|t| t.height_m as f64);

        // ── 3. IT CLOSES BARE GROUND ──
        // 8 m x 8 m of the disc, on a 2 cm grid, stamped with each instance's
        // real footprint.
        let up = c.normalize();
        let east = DVec3::Y.cross(up).normalize();
        let north = up.cross(east).normalize();
        const HALF_M: f64 = 4.0;
        const CELL_M: f64 = 0.02;
        const N: usize = (2.0 * HALF_M / CELL_M) as usize; // 400
        let mut cover_t = vec![false; N * N];
        let mut cover_all = vec![false; N * N];
        for t in &g {
            let rel = (t.dir - up) * def.radius;
            let (e, n) = (rel.dot(east), rel.dot(north));
            let r = (stats.footprint_unit * t.height_m) as f64;
            let (lo_e, hi_e) = (e - r, e + r);
            let (lo_n, hi_n) = (n - r, n + r);
            if hi_e < -HALF_M || lo_e > HALF_M || hi_n < -HALF_M || lo_n > HALF_M {
                continue;
            }
            let cell =
                |v: f64| (((v + HALF_M) / CELL_M).floor() as isize).clamp(0, N as isize - 1);
            for gy in cell(lo_n)..=cell(hi_n) {
                let cy = -HALF_M + (gy as f64 + 0.5) * CELL_M;
                for gx in cell(lo_e)..=cell(hi_e) {
                    let cx = -HALF_M + (gx as f64 + 0.5) * CELL_M;
                    if (cx - e).powi(2) + (cy - n).powi(2) > r * r {
                        continue;
                    }
                    let i = gy as usize * N + gx as usize;
                    cover_all[i] = true;
                    if !t.filler {
                        cover_t[i] = true;
                    }
                }
            }
        }
        let bare_t = cover_t.iter().filter(|c| !**c).count() as f64 / (N * N) as f64;
        let bare_all = cover_all.iter().filter(|c| !**c).count() as f64 / (N * N) as f64;
        println!(
            "[grass filler] {} instances = {} tussocks + {} filler ({:.1}%); mean clump gain \
             under tussocks {gt:.2} vs under filler {gf:.2}; mean height {ht:.2} m vs \
             {hf:.2} m; bare ground over 64 m2 {:.1}% -> {:.1}% (footprint {:.2} m at a \
             {ht:.2} m tiller)",
            g.len(),
            tuss.len(),
            fill.len(),
            frac * 100.0,
            bare_t * 100.0,
            bare_all * 100.0,
            stats.footprint_unit as f64 * ht
        );
        assert!(
            gf < gt * 0.8,
            "filler stubble sits at a mean clump gain of {gf:.2} against the tussocks' \
             {gt:.2} - it is being scattered uniformly instead of into the gaps, so it \
             thickens the clumps it was supposed to fill between"
        );
        assert!(
            hf < ht * 0.75 && hf > ht * 0.25,
            "filler stubble averages {hf:.2} m against the tussocks' {ht:.2} m - it has to \
             read as a short shoot between the tussocks, not as another tussock and not as \
             an invisible sliver"
        );
        assert!(
            bare_all < bare_t * 0.90,
            "bare ground only fell from {:.1}% to {:.1}% - the filler class is not closing \
             the gaps it costs triangles to draw",
            bare_t * 100.0,
            bare_all * 100.0
        );
        // COST CEILING, the other half of the bargain: the filler class draws
        // the same shared mesh, so its share of the instance count IS its
        // share of the added triangles.
        //
        // The POOLED expectation is 11.5% (`grass_scatter_is_clustered_not_
        // poisson` measures 45.6 tussock + 5.8 filler per m^2 over nine
        // sites), and the drawn sets at Fuji and the Amazon measure 11-12% in
        // `near_grass_density_matches_a_real_sward`. THIS site reads higher
        // (14.5%) and is meant to: its ground averages a clump gain of 0.85,
        // and thin ground is exactly where the stubble belongs. So the gate
        // here is a budget ceiling with room for a thin site, not the
        // expectation - the expectation is pinned against grass_filler_per_m2
        // in the clustering test.
        assert!(
            frac < 0.20,
            "filler stubble is {:.0}% of the instances, i.e. {:.0}% more triangles than the \
             tussocks alone would cost",
            frac * 100.0,
            frac / (1.0 - frac) * 100.0
        );
    }

    /// CI TWIN for "the tuft does not match the ground it grows in": the
    /// deleted card hardcoded `col = [0.24, 0.34 + jitter, 0.10]`, i.e. R and
    /// B were LITERALLY CONSTANT for every tuft on the planet while the
    /// ground under it came from Blue Marble imagery - measured as a 2.59x
    /// brightness break and a large hue break at the Fuji vantage.
    #[test]
    fn grass_colour_tracks_the_surface_it_stands_on() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let mut yellowed = 0usize;
        let mut tillers = 0usize;
        let mut sen_pos: Vec<(f64, f64)> = Vec::new();
        for (name, lat, lon) in [
            ("fuji", 35.3_f64, 138.8_f64),
            ("amazon", -3.0, -60.0),
            ("black_forest", 48.2, 8.2),
            ("siberian_taiga", 58.0, 98.0),
        ] {
            let c = dir_of(lat, lon);
            let g = near_grass_instances(&def, &src, Some(&albedo), c, 8.0, 0.0, 20, 4000);
            assert!(!g.is_empty(), "{name}: no grass to check");
            let ground = surface_color(
                &def,
                Some(&albedo),
                c.as_vec3(),
                drawn_elevation_normalized(&hm, &def, &detail, None, c),
            );
            // BRIGHTNESS must track the ground. The measured defect at Fuji
            // was a 2.59x luma break between tuft and sward; the tint lifts a
            // little (a near-vertical leaf catches more sky than the
            // horizontal ground beside it) and jitters per clump, so this is
            // a band, but it is a band around the GROUND, which a planet-wide
            // constant can never be.
            let luma = |c: [f32; 3]| c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
            let gl = luma(ground).max(0.005);
            let mut ratios: Vec<f32> = g.iter().map(|t| luma(t.color) / gl).collect();
            ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med_ratio = ratios[ratios.len() / 2];
            let max_ratio = *ratios.last().unwrap();
            // YELLOWING: senescent tissue raises R/G well above the live
            // tint's. A real meadow always carries 5-20% dead tissue and an
            // entirely lush field is as wrong as an entirely straw one.
            let live_rg = ground[0].max(1e-4) / ground[1].max(1e-4) * (1.18 / 1.30);
            let sen = g
                .iter()
                .filter(|t| t.color[0] / t.color[1].max(1e-4) > live_rg * 1.30)
                .count();
            let sen_frac = sen as f32 / g.len() as f32;
            println!(
                "[grass colour] {name}: ground {ground:?} luma {gl:.3}; grass/ground luma \
                 median {med_ratio:.2}x max {max_ratio:.2}x; yellowed fraction {:.1}%",
                sen_frac * 100.0
            );
            assert!(
                med_ratio > 0.8 && med_ratio < 1.6,
                "{name}: grass reads {med_ratio:.2}x the ground it grows in - the \
                 measured defect at Fuji was 2.59x, from a hardcoded planet-wide colour"
            );
            assert!(
                max_ratio < 2.2,
                "{name}: the brightest tiller reads {max_ratio:.2}x the ground"
            );
            yellowed += sen;
            tillers += g.len();
            for t in g.iter().step_by(7) {
                let d = t.dir.normalize();
                sen_pos.push((d.y.clamp(-1.0, 1.0).asin(), (-d.z).atan2(d.x)));
            }
        }
        // Pooled across sites: the dryness field is correlated over metres, so
        // ONE 8 m disc can legitimately sit entirely in a lush hollow. What
        // must not happen is a planet with no dead tissue anywhere.
        //
        // The number a real meadow is quoted by (5-20% dead tissue) is a
        // TISSUE fraction, not a plant count, so the gate is the mean
        // senescence across tillers; the count of visibly-yellowed tillers is
        // printed alongside for context and is naturally larger, because a
        // tiller at 20% senescence reads as tinged rather than dead.
        //
        // AVERAGE OVER THE REAL JITTER DISTRIBUTION, not over its two
        // endpoints. `grass_senescence` clamps at 0, so it is not linear in
        // the jitter and the endpoint mean systematically OVERSTATES the
        // expectation - by 2.4x in a lush hollow, where the whole low half of
        // the jitter range is clamped to zero and the endpoint average keeps
        // crediting half of it. The v0.1090 test published 18.8% that way
        // when the true field mean was near 12%. The jitter is
        // `(r % 1000) / 1000` and enters nowhere else, so sampling all 1000
        // values IS the exact expectation, not an approximation of it.
        let mut sen_sum = 0.0f64;
        let mut sen_n = 0usize;
        for (lat, lon) in &sen_pos {
            for r in 0..1000u64 {
                sen_sum += grass_senescence(*lat, *lon, r) as f64;
                sen_n += 1;
            }
        }
        let mean_sen = (sen_sum / sen_n as f64) as f32;
        let pooled = yellowed as f32 / tillers as f32;
        println!(
            "[grass colour] pooled: mean senescent tissue {:.1}%, visibly yellowed \
             tillers {:.1}%",
            mean_sen * 100.0,
            pooled * 100.0
        );
        assert!(
            (0.03..=0.25).contains(&mean_sen),
            "mean senescent tissue {:.1}% - a real meadow always carries 5-20% dead or \
             dying tissue, and its absence is a strong 'this is plastic' cue while an \
             excess turns a summer meadow into hay",
            mean_sen * 100.0
        );
    }

    /// The drawn ground must be SMOOTH at the scale a 30 cm blade stands on.
    /// It is, on gentle terrain - and it is NOT on a steep flank, which this
    /// test measures and prints because it is the single biggest limit on how
    /// well anything can be planted on the ground.
    ///
    /// THE DEFECT, found while measuring grass base placement and filed as a
    /// separate ticket because it is not grass's to fix:
    /// `PlanetHeightmap::sample_meters_smooth` takes an **f32** `Vec3` and
    /// `dir_to_latlon_deg` hands back **f32 degrees**. One ulp of f32 at
    /// lon 138.8 is 1.53e-5 deg, which is 1.39 m on the ground at lat 35.3,
    /// so the base elevation is a STAIRCASE with ~1.4 m treads. On Fuji's
    /// flank (base gradient ~0.33 m/m, x4.0 vertical exaggeration) one riser
    /// of that staircase is **1.708 m of drawn radius**, measured below over
    /// a 4 m transect at 5 cm steps. Amazon, on flat ground, reads 0.008 m
    /// over the same transect - so the artifact scales with slope and is
    /// invisible everywhere the terrain is gentle, which is why it has not
    /// been caught before.
    ///
    /// It is not a grass bug and it is not a vegetation bug: the player's own
    /// ground clamp (`drawn_elevation_normalized`, the function measured
    /// here) and every patch vertex go through the same sampler. It bounds
    /// how closely ANY placed object can sit on the ground on a slope.
    #[test]
    fn ground_sampler_is_smooth_at_blade_scale_on_gentle_terrain() {
        let (hm, _albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let mut worst_by_site = Vec::new();
        for (name, la, lo) in [("fuji", 35.3, 138.8), ("amazon", -3.0, -60.0)] {
            let c = dir_of(la, lo);
            let up = c.normalize();
            let east = DVec3::Y.cross(up).normalize();
            let mut prev = 0.0f64;
            let mut worst: f64 = 0.0;
            print!("{name}: ");
            for i in 0..80 {
                let off = (i as f64 - 40.0) * 0.05; // metres east
                let d = (up + east * (off / def.radius)).normalize();
                let e = drawn_elevation_normalized(&hm, &def, &detail, None, d);
                let r = def.radius * displaced_radius_f64(&def, e as f64);
                if i > 0 {
                    let s = r - prev;
                    if s.abs() > worst.abs() {
                        worst = s;
                    }
                    print!("{:+.3} ", s);
                }
                prev = r;
            }
            println!("| worst 5 cm step {worst:+.3} m");
            worst_by_site.push((name, worst));
        }
        // Only the gentle site is a GATE - the steep one is the known defect
        // above, and asserting on it would just pin a bug. If Amazon ever
        // develops metre-scale steps, something has gone wrong in the sampler
        // that is nothing to do with f32 latitude.
        let (_, amazon) = worst_by_site[1];
        assert!(
            amazon.abs() < 0.05,
            "flat ground steps by {amazon:+.3} m in 5 cm - the drawn surface is no longer \
             smooth at the scale things stand on it"
        );
    }

    /// The layer is CAMERA-RELATIVE but the field is PLANET-FIXED: harvesting
    /// from two nearby poses must return the same tillers in the same spots
    /// for the ground both discs cover, or the sward crawls as you walk. This
    /// is the invariant the fixed-six-randoms stream exists to protect.
    #[test]
    fn grass_is_planet_fixed_not_camera_fixed() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let a_dir = dir_of(-3.0, -60.0);
        // ~4 m east.
        let b_dir = dir_of(-3.0, -60.0 + (4.0 / def.radius).to_degrees());
        let a = near_grass_instances(&def, &src, Some(&albedo), a_dir, 6.0, 0.0, 20, 40_000);
        let b = near_grass_instances(&def, &src, Some(&albedo), b_dir, 6.0, 0.0, 20, 40_000);
        assert!(a.len() > 200 && b.len() > 200, "not enough grass to compare");
        // 1e11 on a unit direction = 64 micrometres of key resolution. 1e9
        // (6.4 mm) was tried first and produced ~5 false collisions among
        // 5,000 tillers in a 113 m^2 disc, which read as "the same tiller
        // changed look" when it was two different tillers sharing a key.
        let key = |t: &NearGrass| -> (i64, i64, i64) {
            let p = t.dir * 1.0e11;
            (p.x.round() as i64, p.y.round() as i64, p.z.round() as i64)
        };
        let amap: std::collections::HashMap<_, _> = a.iter().map(|t| (key(t), *t)).collect();
        let mut shared = 0;
        for t in &b {
            if let Some(o) = amap.get(&key(t)) {
                shared += 1;
                assert!(
                    (o.height_m - t.height_m).abs() < 1.0e-6
                        && (o.yaw - t.yaw).abs() < 1.0e-6
                        && (o.color[1] - t.color[1]).abs() < 1.0e-6,
                    "the same tiller changed look between harvests - the stream is not \
                     position-keyed"
                );
            }
        }
        // The two discs overlap heavily at 4 m of separation on a 6 m radius.
        assert!(
            shared as f64 > b.len() as f64 * 0.30,
            "only {shared} of {} tillers survived a 4 m step - the field is being \
             re-rolled, which is the LOD-reshuffle bug v0.897 fixed for trees",
            b.len()
        );
    }

    /// The density ramp must reach zero smoothly at GRASS_FAR_M. A layer that
    /// stops at a hard edge draws the moving lit ring the operator reported
    /// at v0.999 ("a line of light perpendicular to me like 10 meters away"),
    /// which is the whole reason the depth gate had to go.
    #[test]
    fn grass_density_ramp_has_no_edge() {
        assert_eq!(grass_density_at(0.0), grass_peak_per_m2());
        assert_eq!(grass_density_at(GRASS_NEAR_M), grass_peak_per_m2());
        assert!((grass_density_at(GRASS_MID_M) - grass_mid_per_m2()).abs() < 1e-3);
        assert_eq!(grass_density_at(GRASS_FAR_M), 0.0);
        assert_eq!(grass_density_at(GRASS_FAR_M + 5.0), 0.0);
        // No step anywhere: the biggest jump between 10 cm samples must stay
        // small against the peak.
        let mut worst = 0.0f32;
        let mut prev = grass_density_at(0.0);
        let mut d = 0.1f32;
        while d <= GRASS_FAR_M + 2.0 {
            let v = grass_density_at(d);
            worst = worst.max((prev - v).abs());
            prev = v;
            d += 0.1;
        }
        assert!(
            worst < grass_peak_per_m2() * 0.02,
            "the ramp steps by {worst:.2} tillers/m2 in 10 cm - that is an edge"
        );
    }

    /// THE LADDER GATE (v0.1103), written while chasing the operator's "they
    /// still look like rings" report and kept as a regression guard after it
    /// REFUTED the hypothesis it was written for.
    ///
    /// The worry: a tiller's threshold is `item / count * GAIN_MAX / gain`, and
    /// `grass_appear_distance` is monotone in it, so an evenly-spaced set of
    /// thresholds is an evenly-spaced set of emergence RADII. Cohorts of blades
    /// would then grow in together at fixed distances from the camera and the
    /// field would band into concentric arcs of half-height grass - exactly
    /// what an arc artifact looks like.
    ///
    /// The measurement says it does not happen, and says WHY: `gain` is a
    /// per-ITEM function of position on a field whose fine octave is 0.72 m,
    /// well inside the ~3 m stream cell, so items sharing an index rung do NOT
    /// share a radius. Scored as the coefficient of variation of the sorted
    /// within-cell threshold gaps: 0.98 as shipped, and 0.00 (a perfect ladder)
    /// when the gain factor is dropped from the threshold, which is what this
    /// gate now protects. Thresholds, not distances, because the ramp's own 4x
    /// slope change between its two legs puts irregularity into the distances
    /// that has nothing to do with the ladder (a forced-ladder probe still
    /// scored 0.62 that way, so the distance version could not tell the two
    /// apart).
    #[test]
    fn grass_appear_distances_are_not_a_ladder() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let c = dir_of(48.2, 8.2);
        let g = near_grass_instances(&def, &src, Some(&albedo), c, GRASS_FAR_M as f64, 0.0, 20, 60_000);
        assert!(g.len() > 5_000, "only {} instances - not a field to measure", g.len());
        // Group by STREAM CELL: the ladder is a per-cell artifact (count and
        // gain are shared within one), and pooling cells with different gains
        // would smear it and let the defect pass.
        use std::collections::HashMap;
        let mut cells: HashMap<(i64, i64), Vec<f32>> = HashMap::new();
        for t in &g {
            if t.filler {
                continue; // one class at a time: they have different counts
            }
            let d = t.dir.normalize();
            let (lat, lon) = (d.y.clamp(-1.0, 1.0).asin(), (-d.z).atan2(d.x));
            let key = (
                (lat / GRASS_CELL_RAD).floor() as i64,
                (lon / GRASS_CELL_RAD).floor() as i64,
            );
            cells.entry(key).or_default().push(t.thr);
        }
        // THE RUNG WIDTH this cell's stream would have. A ladder's smallest
        // threshold gap is exactly one rung (two adjacent indices); a field
        // whose thresholds are smeared by a per-item multiplier puts many
        // instances INSIDE a rung, so its smallest gap is a small fraction of
        // one. That ratio is the statistic, and it is scale-free.
        let cell_m = GRASS_CELL_RAD * def.radius;
        let area_m2 = cell_m * cell_m * 48.2_f64.to_radians().cos();
        let count = (grass_peak_per_m2() * GRASS_CLUMP_GAIN_MAX) as f64 * area_m2;
        let rung = 1.0 / count;
        let mut ratios: Vec<f64> = Vec::new();
        for v in cells.values_mut() {
            if v.len() < 24 {
                continue;
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let min_gap = v
                .windows(2)
                .map(|w| (w[1] - w[0]) as f64)
                .fold(f64::MAX, f64::min);
            ratios.push(min_gap / rung);
        }
        assert!(ratios.len() > 40, "only {} cells were populous enough to measure", ratios.len());
        ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = ratios[ratios.len() / 2];
        println!(
            "[grass ladder] {} cells, rung {rung:.5} of normalized density: smallest \
             within-cell threshold gap as a fraction of one rung - median {median:.2}, p10 \
             {:.2}, p90 {:.2}",
            ratios.len(),
            ratios[ratios.len() / 10],
            ratios[ratios.len() * 9 / 10]
        );
        // A perfect ladder scores 1.00 by construction (adjacent indices, no
        // smearing) and MEASURED 1.00 when the gain factor is dropped from the
        // threshold. The shipped field measures 0.03. 0.5 is a wide margin
        // either way.
        assert!(
            median < 0.5,
            "the smallest within-cell threshold gap is {median:.2} of a full index rung - the \
             thresholds are a regular ladder, so cohorts of blades emerge together at fixed \
             radii from the camera and the field bands into concentric arcs"
        );
    }

    /// THE HARVEST DESIGN GATE (v0.1091), and the CI twin for NO GRASS RING.
    ///
    /// The density ramp is a function of distance FROM THE CAMERA. If the
    /// drawn set were whatever the last harvest returned, then every time the
    /// harvest re-centred, the ramp would re-anchor: a tiller 6 m from the
    /// new pose was 14 m from the old one, where `grass_density_at` is 4x
    /// smaller, so a whole annulus of blades would appear at once. That is a
    /// breathing population ring, exactly what the vantage's regression
    /// forbids, and no amount of hysteresis tuning removes it - a bigger
    /// hysteresis makes the ring bigger, a smaller one makes it more often.
    ///
    /// The design instead splits the two: the harvest returns a SUPERSET for
    /// a whole ball of camera positions, and the drawn set is re-derived from
    /// the LIVE camera every frame. This test pins both halves of that:
    ///
    ///   1. COVERAGE - a superset harvested at C really does contain
    ///      everything a camera anywhere within GRASS_HARVEST_MARGIN_M of C
    ///      draws. If it does not, the leading edge thins as you walk.
    ///   2. INVARIANCE - re-harvesting at a different centre changes NOTHING
    ///      about what is drawn from a given camera pose. That is the ring
    ///      gate stated exactly: same pose, different harvest, same sward.
    #[test]
    fn grass_harvest_recentring_cannot_change_what_is_drawn() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let harvest = |c: DVec3, margin: f64| {
            near_grass_instances(
                &def,
                &src,
                Some(&albedo),
                c,
                GRASS_FAR_M as f64,
                margin,
                20,
                80_000,
            )
        };
        // A "drawn set" keyed by planet-fixed position, with the live height
        // each tiller stands at from a given camera pose. 1e11 on a unit
        // direction is 64 micrometres of key resolution (see the note in
        // grass_is_planet_fixed_not_camera_fixed).
        let drawn_at = |set: &[NearGrass], cam: DVec3| {
            let mut m: std::collections::HashMap<(i64, i64, i64), f32> =
                std::collections::HashMap::new();
            for t in set {
                let e = grass_live_emerge(t.thr, surf_d(&def, cam, t.dir));
                if e <= 0.0 {
                    continue;
                }
                let p = t.dir * 1.0e11;
                m.insert(
                    (p.x.round() as i64, p.y.round() as i64, p.z.round() as i64),
                    t.height_m * e,
                );
            }
            m
        };
        let c0 = dir_of(-3.0, -60.0);
        let sup = harvest(c0, GRASS_HARVEST_MARGIN_M);
        assert!(sup.len() > 2_000, "only {} tillers harvested", sup.len());

        // ── 1. COVERAGE ──
        // Walk to the edge of the margin ball in four directions and check the
        // superset still contains everything a local harvest would draw.
        let east = DVec3::Y.cross(c0).normalize();
        let north = c0.cross(east).normalize();
        let mut worst_missing = 0usize;
        let mut worst_of = 0usize;
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (-0.7, -0.7)] {
            let step = (east * dx + north * dy) * (GRASS_HARVEST_MARGIN_M / def.radius);
            let cam = (c0 + step).normalize();
            let local = harvest(cam, 0.0);
            let want = drawn_at(&local, cam);
            let have = drawn_at(&sup, cam);
            let missing = want.keys().filter(|k| !have.contains_key(*k)).count();
            if missing > worst_missing {
                worst_missing = missing;
                worst_of = want.len();
            }
        }
        assert_eq!(
            worst_missing, 0,
            "the superset is missing {worst_missing} of {worst_of} tillers a camera at the \
             edge of GRASS_HARVEST_MARGIN_M ({GRASS_HARVEST_MARGIN_M} m) draws - the leading \
             edge of the sward thins while a harvest is in flight"
        );

        // ── 2. INVARIANCE (the ring gate) ──
        // Same camera pose, two harvests centred metres apart. The drawn set
        // must be IDENTICAL - same tillers, same heights, to the bit.
        let cam = (c0 + east * (3.0 / def.radius)).normalize();
        let a = drawn_at(&sup, cam);
        let b = drawn_at(&harvest(cam, GRASS_HARVEST_MARGIN_M), cam);
        assert!(a.len() > 1_000, "only {} tillers drawn", a.len());
        assert_eq!(
            a.len(),
            b.len(),
            "re-harvesting at a different centre changed the drawn population from {} to {} \
             at the SAME camera pose - the density ramp has re-anchored on the harvest \
             instead of on the camera, which is a population ring",
            a.len(),
            b.len()
        );
        for (k, ha) in &a {
            let hb = b.get(k).copied().unwrap_or(-1.0);
            assert!(
                (ha - hb).abs() < 1.0e-6,
                "a tiller stands {ha:.4} m tall from one harvest and {hb:.4} m from another \
                 at the same camera pose"
            );
        }

        // ── 3. NO STEP AS YOU WALK ──
        // Nothing may appear at a visible height IN ONE FRAME. The step is
        // 0.17 m, which is what a 5 m/s sprint covers at 30 fps - the fastest
        // the camera legitimately moves on foot.
        const FRAME_STEP_M: f64 = 0.17;
        let mut prev = drawn_at(&sup, c0);
        let mut worst_pop_in = 0.0f32;
        let mut worst_step = 0.0f32;
        for i in 1..=30 {
            let cam = (c0 + east * (i as f64 * FRAME_STEP_M / def.radius)).normalize();
            let now = drawn_at(&sup, cam);
            for (k, h) in &now {
                match prev.get(k) {
                    None => worst_pop_in = worst_pop_in.max(*h),
                    Some(p) => worst_step = worst_step.max((h - p).abs()),
                }
            }
            prev = now;
        }
        println!(
            "[grass ring] walking 5 m in {FRAME_STEP_M} m frames: tallest blade to appear \
             from nothing {worst_pop_in:.4} m, largest height change of a standing blade \
             {worst_step:.4} m (shortest blade is {GRASS_HEIGHT_MIN_M} m, grow-in runs over \
             {GRASS_EMERGE_LEN_M} m of approach)"
        );
        assert!(
            worst_pop_in < GRASS_HEIGHT_MIN_M * 0.25,
            "a {worst_pop_in:.3} m blade appeared from nothing in one {FRAME_STEP_M} m frame - \
             GRASS_EMERGE_LEN_M is too short and blades are popping in at height"
        );
    }
}
