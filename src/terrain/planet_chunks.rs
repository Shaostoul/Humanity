//! Chunked planetary LOD: a quadtree of surface patch meshes whose detail
//! follows the camera (2026-07-11, the FTL close-approach increment).
//!
//! WHY: the uniform whole-sphere icosphere path (terrain::planet_surface)
//! subdivides EVERYWHERE at once, so its cost is 20 * 4^level faces no
//! matter where the camera looks. Level 9 (5.2M faces) is its practical
//! ceiling; triangle edges there are still ~13 km. Reaching the operator's
//! 1 m target uniformly would need level 23 = 1.4e15 faces. The classic
//! answer is chunked LOD: split the sphere into a tree of small patch
//! meshes and refine ONLY where the camera is close, so detail is O(what
//! you can see), not O(the planet).
//!
//! ── The math (documented per the design brief) ──
//! Tree roots are the 20 icosahedron faces. Adjacent icosahedron vertices
//! (circumradius 1) have dot = 1/sqrt(5), so one root edge spans
//! acos(1/sqrt(5)) = 1.1071487 rad of arc. For Earth (R = 6,371 km) that is
//! an arc of R * 1.1071487 = 7,054 km. Each split halves the angle:
//!   patch edge arc at depth D  = 7,054 km / 2^D
//!   triangle (vertex) spacing  = patch edge / PATCH_TESS (16)
//!   depth 11: patch 3,444 m -> triangles ~215 m
//!   depth 12: patch 1,722 m -> triangles ~108 m
//!   depth 13: patch   861 m -> triangles  ~54 m   <- MAX_PATCH_DEPTH
//! Depth 13 lands triangle edges in the original 50-100 m target band.
//! The ~1 m follow-up SHIPPED in v0.875: with the streamed tile tier
//! installed the cap is TILE_MAX_PATCH_DEPTH = 20 (7,054 km / 2^20 / 16 =
//! 0.42 m triangles) plus seven extra fine-noise octaves (62 m .. 1 m
//! wavelengths) as the micro-detail synthesis; the per-patch f64-anchor
//! scheme below carries the precision (offsets from the anchor are a few
//! meters at depth 20, ulp sub-micrometer).
//!
//! ── Precision discipline (mirrors dev travel) ──
//! An f32 vertex relative to the PLANET CENTER has an ulp of ~0.5 m at
//! Earth-radius magnitudes: sub-meter geometry would visibly jitter. So a
//! patch NEVER stores planet-relative f32 vertices. Each patch has an f64
//! anchor (its center direction * sphere radius, in the planet's unrotated
//! local frame); vertices are f32 offsets FROM that anchor (a few km at
//! most, so ulp is sub-mm). At draw time the translation is composed in f64
//! (planet_render_pos_f64 + rotation_f64 * anchor_f64) and narrowed to f32
//! only at the very end, exactly like ship_world_pos handling in lib.rs.
//!
//! ── Cracks: skirts ──
//! Neighboring patches at DIFFERENT depths sample elevation at different
//! densities, so their shared border disagrees (a crack). Each patch drops
//! a short vertical apron (skirt) from its border, depth-scaled, which
//! visually seals the gap from any exterior viewpoint. Skirts are the
//! simplest robust choice: proper T-junction stitching needs neighbor
//! bookkeeping across 20 root faces and re-meshing on every LOD change; it
//! is the documented follow-up if skirts ever show. Same-depth neighbors
//! share bit-identical border sample DIRECTIONS (commutative f64 midpoint
//! math), so their only mismatch is per-patch f32 anchor rounding (sub-cm),
//! also hidden by the skirt.
//!
//! ── Culling ──
//! Two gates, applied during tree descent (so culled regions never generate
//! geometry) and implicitly at draw (only selected patches are drawn):
//! - HORIZON: a patch whose entire bounding cone lies beyond the planet's
//!   horizon from the camera is skipped. The far side costs zero.
//! - FRUSTUM: patch bounding spheres are tested against the camera frustum
//!   (planes handed in already transformed into the planet-local frame).
//!
//! ── Streaming ──
//! Patch builds are CPU work (heightmap sampling + noise + color); they are
//! budgeted per frame and prioritized by screen-space error. The selection
//! uses RESTRICTED DESCENT: a node only splits when every visible child
//! mesh is already resident, otherwise it requests the missing children and
//! draws itself this frame. The tree therefore refines progressively with
//! zero holes. An LRU byte-capped cache evicts patches the camera left
//! behind (roots are pinned so a whole-planet fallback always exists).
//!
//! Everything in this module is pure math (no GPU) and fully unit-tested
//! headless; the GPU hop reuses renderer::mesh::Mesh::from_planet_surface
//! on the SurfaceMeshData this module emits (positions are METERS relative
//! to the patch anchor rather than unit-sphere, which that constructor does
//! not care about).

use glam::{DMat4, DQuat, DVec3, DVec4};
use noise::{NoiseFn, Perlin};
use std::collections::{BinaryHeap, HashMap};

use super::planet::PlanetDef;
use super::planet_albedo::PlanetAlbedo;
use super::planet_heightmap::PlanetHeightmap;
use super::planet_surface::{
    displaced_radius_f64, displaced_radius_f64_true, slope_shade, surface_color, SurfaceMeshData,
    SurfaceSampler, SurfaceVertexData,
};

/// Tessellation of one patch edge: 16 segments -> a triangular grid of
/// (16+1)(16+2)/2 = 153 unique sample points and 16^2 = 256 grid triangles.
/// Chosen so a patch is one cheap build unit (~153 elevation samples) while
/// still being a real mesh (not a single triangle) -- the tree stores
/// patches, not triangles, so tree depth stays shallow (depth 13, not 17).
pub const PATCH_TESS: u32 = 16;

/// Depth cap for this increment. See the module-header math: depth 13 puts
/// triangle edges at ~54 m on Earth (7,054 km / 2^13 / 16), inside the
/// 50-100 m target. The ~1 m follow-up raises this to ~19 and adds
/// micro-detail synthesis.
pub const MAX_PATCH_DEPTH: u8 = 13;

/// Patch depth at which the streamed high-detail tiles (terrain_tiles,
/// ~460 m cells from ETOPO 2022) take over elevation sampling from the base
/// grid when resident: depth 9 triangles (~1.2 km) start resolving what the
/// 460 m data carries, and the bicubic stencil (4 cells, ~1.8 km) stays
/// smooth. The depth-8/9 LOD boundary step is absorbed by skirts, exactly
/// like the fine-octave depth gates below.
pub const TILE_MIN_DEPTH: u8 = 9;

/// Depth cap when the tile tier is installed. Raised 16 -> 20 for the 1 m
/// ladder (v0.875, operator's max-settings directive): depth 20 patches are
/// ~6.7 m wide with ~0.42 m triangles, engaged only within ~30 m of the
/// ground (screen-space split), expressing the full extended fine-octave
/// ladder (gates 14..20 below). Base-only stays at MAX_PATCH_DEPTH (deeper
/// triangles over 5.5 km cells buy nothing). PatchId.path is u64 (2 bits
/// per level), so the tree could go to 32; the cap is a QUALITY choice.
pub const TILE_MAX_PATCH_DEPTH: u8 = 20;

/// Central angle of one root icosahedron edge: acos(1/sqrt(5)).
/// Adjacent icosahedron vertices at circumradius 1 have dot = 1/sqrt(5)
/// (e.g. (-1,t,0) and (1,t,0) normalized give (t*t-1)/(t*t+1) = 1/sqrt(5)).
pub const ROOT_EDGE_ANGLE_RAD: f64 = 1.1071487177940904;

/// Split threshold on PROJECTED TRIANGLE EDGE size, in pixels: a patch
/// splits while its vertex spacing subtends more than this many pixels.
/// The spirit of planet::lod_level_for_pixels (a size-doubling ladder)
/// applied per-patch: just before a split triangles are ~12 px, right
/// after they are ~6 px, so leaves render 6-12 px triangles until the
/// depth cap flattens further refinement.
pub const CHUNK_SPLIT_PX: f32 = 12.0;

/// Max patches drawn per planet per frame. The celestial pass shares one
/// 1024-slot object-uniform buffer (renderer MAX_OBJECTS) with every sky
/// body + atmosphere shell, so patches get most-but-not-all of it. The
/// selection's priority heap refines biggest-screen-error-first, so when
/// this budget saturates it is the FINEST (least visible) splits that are
/// skipped, degrading gracefully.
// NOTE (audit 2026-07-30): the comment above justified this cap by "one 1024-slot
// object-uniform buffer (renderer MAX_OBJECTS)". MAX_OBJECTS is 16384
// (src/renderer/mod.rs), 16x larger, so the buffer is NOT what bounds this. The 640
// is a draw-submission budget, not a buffer limit. Do not raise it on the assumption
// that the buffer is the constraint without measuring the draw cost first.
pub const MAX_CHUNK_LEAVES: usize = 640;

/// Patch mesh builds per frame across all planets. Each build is ~153
/// heightmap samples + 3 noise octaves + 352 triangles of assembly
/// (sub-millisecond). Raised 6 -> 24 (v0.867): landing dropped the player
/// onto ground that was still refining beneath them for several seconds
/// (float-then-snap, operator "weird issues" report); 24 refines a
/// from-scratch close approach (~500 patches) in under a second while the
/// worst-case frame cost stays a few ms during descent only.
pub const PATCH_BUILDS_PER_FRAME: usize = 24;

/// Build requests returned per selection; anything beyond the per-frame
/// build budget would be discarded anyway (requests are re-derived fresh
/// every frame, so there is no persistent queue to grow stale).
pub const MAX_BUILD_REQUESTS: usize = 96;

/// LRU cache byte cap for resident patch meshes (GPU estimate). 256 MB was
/// sized in the 640-leaf era (~7,000 bare 38 KB patches). By v0.898 the
/// budget reaches 6144 leaves, prefetch banks children ahead of need, and
/// VEGETATION multiplies per-patch bytes - the needed set alone outgrew the
/// cap, so every build evicted a still-needed patch: the parked-camera
/// build->evict->rebuild churn the operator reported as terrain flicker
/// that got WORSE at higher settings (probe: draws swinging 3572->1561->
/// 4577 per second, requests pinned at the cap, cache pinned at 7,061).
/// 1.5 GB holds the max working set with real headroom; the LRU only
/// grows to what the camera actually needs, so low settings stay small.
// 1536 -> 1400 MiB (v0.1084): the cache ceiling was LARGER than the whole
// patch arena (1470 MiB), and real_bytes counts the same currency, so LRU
// eviction (graceful) could mathematically never fire before arena overflow
// (not graceful) at any split. The ceiling must sit BELOW the arena total so
// the cache is the limiter. Indexed cards cut per-patch bytes ~2/3, so this
// still holds far more patches than the old ceiling ever did.
pub const PATCH_CACHE_MAX_BYTES: usize = 1400 * 1024 * 1024;

/// Cache floor applied ONCE when a planet leaves chunked mode (the camera
/// flew away): shrink to this so a departed planet parks ~64 MB of warm
/// patches (fast re-approach) instead of the full 256 MB. Roots stay
/// pinned regardless, so re-activation never starts from zero.
pub const PATCH_CACHE_WARM_BYTES: usize = 64 * 1024 * 1024;

/// GPU byte estimate for one built patch (see PATCH_CACHE_MAX_BYTES).
/// Exact for WATER-shell patches (still 3 unique verts per face); an
/// upper bound for terrain patches since the shared-vertex layout
/// (draw-batching increment 3) - terrain cache inserts use REAL bytes.
pub const PATCH_MESH_BYTES: usize = 1056 * 32 + 1056 * 4;

/// Skirt depth = patch edge arc * this fraction, clamped to the min/max
/// below. 15% of the edge comfortably covers the elevation disagreement a
/// coarser neighbor can show across one of its triangles (real terrain
/// slopes, even 4x-exaggerated, stay well under this).
// ── Procedural vegetation (v0.888, operator: "take a shot at spawning
// grass and trees... simple placeholders (possibly procedural) are okay") ──
// Baked INTO the patch mesh at build time: deterministic per-patch scatter
// (seeded from the PatchId, so the same patch always grows the same trees),
// land-only, gated by elevation band and slope. LOD is free: vegetation
// appears exactly when its patch's depth builds and vanishes with it.
/// Patch depth at which TREES appear (215 m patches - ~8 px trees at the
/// distance that depth becomes resident).
pub const TREE_MIN_DEPTH: u8 = 15;
// GRASS_MIN_DEPTH is GONE (v0.1090, grass-strands increment). Grass is no
// longer baked into the patch mesh at all - see `near_grass_instances` below.
// The bake gated grass on patch depth >= 18, which meant grass only existed
// where the LOD selector had refined that far, and the selector measures
// distance to the DATUM SPHERE rather than to the ground (see the
// `patch_bounds` / `screen_error_px` note on GRASS_FAR_M): at Fuji (ground
// 1224 m) the deepest patch built was 15-16 in every measured run, so
// hist[18] was 0 and there was no grass anywhere near the camera. Depth is
// the wrong gate for a camera-relative layer; distance is the right one.
/// Vegetation cell grid (v0.897): plant positions come from a PLANET-FIXED
/// lat/lon hash grid, not from each patch's own rng - so the same plants
/// stand in the same spots at every LOD depth. (They used to reshuffle on
/// every split: with 40 trees per patch the whole forest visibly rolled on
/// each LOD swap - a big share of the residual "terrain flicker" the
/// operator reported, and why grass seemed to vanish near the player.)
/// Cell size is radians of arc on the unit sphere.
pub const TREE_CELL_RAD: f64 = 3.45e-5; // ~220 m at the equator
/// Expected trees per tree cell at the equator; scaled by cos(lat) so
/// density stays constant per square km. History: 100 (~2,000/km^2,
/// v0.913 "can we see about making the forest dense"); 400 (~8,000/km^2,
/// v0.963 - operator: "try to make the forests dense. I want to see what
/// we can get away with"). Real temperate forest runs 20k-80k/km^2; the
/// sprite cards (2 quads/tree since v0.961) are what buys the headroom.
pub const TREES_PER_CELL: u32 = 800;

/// Vegetation density multiplier (v0.1084, operator: fewer trees, free the
/// GPU). Settings > Graphics "Vegetation: forest density" writes this each
/// frame from lib.rs (f32 bits in an atomic - patch builds happen on worker
/// threads). 1.0 = the historical full density; the setting defaults to 0.6.
pub static VEG_DENSITY_BITS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x3F80_0000); // 1.0f32

#[inline]
pub fn veg_density() -> f32 {
    f32::from_bits(VEG_DENSITY_BITS.load(std::sync::atomic::Ordering::Relaxed))
        .clamp(0.1, 1.0)
}
/// Real-meter elevation ceiling for trees (a global treeline placeholder).
pub const TREELINE_M: f32 = 1700.0;

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
/// Peak tiller density (tillers per m^2 of ground) inside `GRASS_NEAR_M`,
/// before `veg_density()` scales it. 45 x the shipped 0.6 density setting =
/// 27 tillers/m^2 = 189 drawn blades/m^2.
///
/// WHY THIS IS NOT A REAL SHOOT COUNT, stated plainly so nobody "corrects" it
/// later: a real turf carries 3,000-30,000 SHOOTS/m^2, which no renderer
/// draws as geometry. Each blade here is a BUNDLE - it stands for roughly
/// 5-8 real blades and carries their combined projected area, which is why
/// its base is centimetres wide rather than the 3-5 mm of a single ryegrass
/// blade. The quantity that has to be right is the one the eye reads, which
/// is leaf AREA per ground area (LAI), not shoot count; the CI twin
/// `near_grass_density_matches_a_real_sward` measures LAI off the real
/// emitted geometry rather than trusting this comment.
pub const GRASS_PEAK_PER_M2: f32 = 45.0;
/// Density at `GRASS_MID_M`, before `veg_density()`.
pub const GRASS_MID_PER_M2: f32 = 14.0;
/// Full density out to here (metres of surface distance from the camera).
pub const GRASS_NEAR_M: f32 = 6.0;
/// Mid ring: `GRASS_PEAK_PER_M2` ramps linearly to `GRASS_MID_PER_M2` here.
pub const GRASS_MID_M: f32 = 12.0;
/// Density reaches ZERO here. Nothing takes over past it - the ground
/// texture carries the far field - so the ramp has to reach zero smoothly or
/// it draws a ring. The old baked cards are NOT kept as a 15-45 m stage:
/// they were the geometry this increment deletes, and keeping them would
/// have meant maintaining both a strand layer and a card layer plus a
/// hide-radius handshake between them.
pub const GRASS_FAR_M: f32 = 22.0;
/// Ceiling on `grass_clump_gain`. Two things depend on it: the per-cell item
/// budget (a cell sitting entirely inside a clump must have enough items in
/// its stream to fill it) and the acceptance probability (which divides by
/// this, so it can never exceed 1).
pub const GRASS_CLUMP_GAIN_MAX: f32 = 2.6;
/// Blades on one tiller. 5-9 is the real range for a perennial-ryegrass
/// tiller; 7 is the middle and it is a SHARED mesh, so the variety between
/// tillers comes from yaw, height, lean and colour instead of blade count.
pub const GRASS_BLADES_PER_TILLER: usize = 9;
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

pub const SKIRT_EDGE_FRACTION: f64 = 0.15;
/// Never shallower than 20 m (hides f32 rounding + same-depth seams).
pub const SKIRT_MIN_M: f64 = 20.0;
/// Never deeper than 80 km (a coarse-patch skirt does not need to exceed
/// the full exaggerated relief span).
pub const SKIRT_MAX_M: f64 = 80_000.0;

/// The uniform-path LOD ladder level at which chunked mode engages: level 8
/// is where the old ladder starts building its heavy close-approach meshes
/// (a >1280 px disc at the default 10 px threshold, i.e. the planet fills
/// the screen). Below this the existing uniform icosphere path draws
/// exactly as today: it is correct and cheap at distance.
pub const CHUNK_ACTIVATION_LADDER_LEVEL: u32 = 8;

// ── Detail noise (design constraint 7; close-range extension v0.818) ──
// Earth's heightmap cells are 0.05 deg (~5.5 km at the equator); below that
// the sampler is geometrically flat, so sub-5-km triangles would
// buy nothing. Seeded Perlin octaves add believable relief below the data
// floor. The noise is masked to LAND (fading in over the first 50 m above
// sea level) so oceans and coastlines stay exactly where the data puts them
// (ocean waves are a shader concern, not geometry). Seeded from terrain_seed
// ONLY and sampled by position: two patches sharing a border direction get
// bit-identical values, which per-patch seeding would break (so the brief's
// "seeded from terrain_seed + patch coords" is realized as seed-from-
// terrain_seed + deterministic patch-coordinate SAMPLING, not per-patch
// seeds). Amplitudes are REAL meters, then receive the same surface_relief
// vertical exaggeration as the data itself (Earth ~4x), so they read in
// proportion.
//
// The ladder has two tiers:
//
// BASE (always applied, at every patch depth): wavelengths ~8/4/2 km
// (frequencies 800/1600/3200 on the unit sphere: 6,371 km / 800 = 8 km),
// amplitudes tapering 17/8.5/4.5 m. These fill the gap just below the ~11 km
// data floor and are what a whole-continent or regional view shows.
//
// FINE (depth-GATED, v0.818; extended to ~1 m in v0.875): more octaves
// continuing the geometric ladder, so at every altitude band the mesh
// carries form at the scale it can express. wavelength_m ~= radius_m / freq:
//   freq    6400 -> ~1.0 km  gate depth 10   (triangle edge ~430 m)
//   freq   12800 -> ~500 m   gate depth 11   (triangle edge ~215 m)
//   freq   25600 -> ~250 m   gate depth 12   (triangle edge ~108 m)
//   freq   51200 -> ~125 m   gate depth 13   (triangle edge  ~54 m)
//   freq  102400 -> ~62 m    gate depth 14   (triangle edge  ~27 m)
//   freq  204800 -> ~31 m    gate depth 15   (triangle edge  ~13 m)
//   freq  409600 -> ~16 m    gate depth 16   (triangle edge ~6.7 m)
//   freq  819200 -> ~7.8 m   gate depth 17   (triangle edge ~3.4 m)
//   freq 1638400 -> ~3.9 m   gate depth 18   (triangle edge ~1.7 m)
//   freq 3276800 -> ~1.9 m   gate depth 19   (triangle edge ~0.8 m)
//   freq 6553600 -> ~1.0 m   gate depth 20   (triangle edge ~0.4 m = cap)
// Amplitudes taper ~x0.55 per octave (4.5 -> 2.3 -> ... -> 0.007 m): the
// first four fine octaves add ~4.3 m of REAL elevation; the seven 1 m-ladder
// octaves add only ~0.4 m more (rock-scale wrinkle, ~1.6 m after Earth's
// exaggeration) -- micro-relief, never new landforms. The taper flattens
// slightly at the tail (x0.55, not x0.5) because natural terrain roughness
// does not vanish at rock scale; pure halving faded to invisibility.
//
// WHY the depth gate: a high-frequency octave sampled by triangles too coarse
// to resolve it (fewer than ~2 samples per wavelength) turns into aliasing
// noise, not detail. Each fine octave therefore only contributes once the
// patch has refined to a depth whose triangle edge is <= half its wavelength
// (Nyquist). The gate is a pure function of patch depth and, because BOTH
// wavelength (radius/freq) and triangle edge (radius*angle/2^depth/16) scale
// with planet radius, the gate DEPTH is radius-independent: every fine octave
// activates at exactly ~2.31 samples/wavelength (see the depth_gate test).
// Two load-bearing consequences:
//   1. Far / coarse patches (depth < 10) get ZERO fine contribution, so their
//      geometry is byte-identical to the base-only ladder shipped before this
//      change -- the whole-Earth and mid-approach views are a regression gate.
//   2. As the camera descends, each finer octave is a strict ADD on top of the
//      coarser ones (which are already present), so the large forms stay put
//      and only smaller wrinkles appear -- detail grows in, it does not swim.
pub const DETAIL_FREQS: [f64; 3] = [800.0, 1600.0, 3200.0];
pub const DETAIL_AMPS_M: [f32; 3] = [17.0, 8.5, 4.5];
/// Fine (depth-gated) octave frequencies: continue the base ladder halving
/// down to ~1 m wavelength (the v0.875 1 m-ladder extension).
pub const DETAIL_FINE_FREQS: [f64; 11] = [
    6400.0, 12800.0, 25600.0, 51200.0, 102400.0, 204800.0, 409600.0, 819200.0, 1638400.0,
    3276800.0, 6553600.0,
];
/// Fine octave amplitudes in REAL meters (before vertical exaggeration),
/// tapering ~x0.55 per octave (see the ladder comment above).
pub const DETAIL_FINE_AMPS_M: [f32; 11] =
    [2.3, 1.1, 0.6, 0.3, 0.17, 0.10, 0.06, 0.035, 0.02, 0.012, 0.007];
/// Minimum patch depth at which each fine octave switches on: the first depth
/// whose triangle edge is <= half the octave's wavelength (Nyquist). Derived
/// once and radius-independent (see the module comment + the depth_gate test).
pub const DETAIL_FINE_MIN_DEPTH: [u8; 11] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
/// Land-mask fade band: detail reaches full strength this many meters
/// above sea level (0 at the waterline, so shorelines are unmodified).
pub const DETAIL_LAND_FADE_M: f32 = 50.0;

/// Seeded sub-heightmap detail noise. Same seed -> identical values
/// forever (determinism tests + multiplayer re-derivation rely on it).
pub struct DetailNoise {
    oct: [Perlin; 3],
    fine: [Perlin; 11],
}

impl DetailNoise {
    pub fn new(terrain_seed: u64) -> Self {
        let s = terrain_seed as u32;
        // Offsets 101..107 keep these octaves decorrelated from the
        // SurfaceSampler's continental/mountain/detail Perlins (offsets
        // 0/1/2 from the same seed) and from each other.
        Self {
            oct: [
                Perlin::new(s.wrapping_add(101)),
                Perlin::new(s.wrapping_add(102)),
                Perlin::new(s.wrapping_add(103)),
            ],
            // Offsets 104..114: one per fine octave, decorrelated from each
            // other and from the base tier above.
            fine: std::array::from_fn(|i| Perlin::new(s.wrapping_add(104 + i as u32))),
        }
    }

    /// Raw (unmasked) detail elevation in meters at a unit-sphere direction,
    /// for a patch at the given tree `depth`. The BASE octaves always apply;
    /// each FINE octave is added only once `depth` reaches its Nyquist gate
    /// (DETAIL_FINE_MIN_DEPTH), so coarse patches stay byte-identical to the
    /// base-only ladder and no octave is ever sampled by triangles too coarse
    /// to resolve it. Sampled in 3D like SurfaceSampler so there is no polar
    /// pinching.
    pub fn sample_m(&self, dir: DVec3, depth: u8) -> f32 {
        let mut sum = 0.0_f64;
        for (i, p) in self.oct.iter().enumerate() {
            let f = DETAIL_FREQS[i];
            sum += DETAIL_AMPS_M[i] as f64 * p.get([dir.x * f, dir.y * f, dir.z * f]);
        }
        for (i, p) in self.fine.iter().enumerate() {
            if depth >= DETAIL_FINE_MIN_DEPTH[i] {
                let f = DETAIL_FINE_FREQS[i];
                sum += DETAIL_FINE_AMPS_M[i] as f64 * p.get([dir.x * f, dir.y * f, dir.z * f]);
            }
        }
        sum as f32
    }

    /// Detail for TILE-backed samples: the base octaves (8/4/2 km) and the
    /// ~1 km fine octave duplicate structure the 460 m tile data already
    /// carries, so only the sub-500 m octaves apply on top of tiles -
    /// procedural wrinkle strictly BELOW the data floor, never fighting it.
    pub fn sample_m_tile_gated(&self, dir: DVec3, depth: u8) -> f32 {
        let mut sum = 0.0_f64;
        for (i, p) in self.fine.iter().enumerate().skip(1) {
            if depth >= DETAIL_FINE_MIN_DEPTH[i] {
                let f = DETAIL_FINE_FREQS[i];
                sum += DETAIL_FINE_AMPS_M[i] as f64 * p.get([dir.x * f, dir.y * f, dir.z * f]);
            }
        }
        sum as f32
    }
}

/// Elevation normalized 0..1 with the streamed-tile override: at depth >=
/// TILE_MIN_DEPTH, when the tile tier covers this point (every bicubic tap
/// resident), sample the 460 m tile data; otherwise the base grid. Returns
/// (normalized elevation, sampled_from_tile) so callers pick the matching
/// detail-noise gate. THE one elevation entry point for tile-aware callers
/// (mesh builder + the ground clamp) - drawn == sampled stays inviolate.
/// Vegetation biome gate, shared VERBATIM by the card bake and the
/// near-model harvest (the model must hide its card, so the two streams may
/// never disagree). Vegetation = green not badly beaten by red, and green
/// clearly above blue (rejects water, ice, snow). Raw Blue Marble linear
/// vegetation is often BROWN-green - Tasmania forest r/g 1.09, Kansas
/// prairie 1.01, the operator's bare hills 1.08 - and the old strict
/// green-dominance test (green > red * 1.04) rejected all of those, so
/// distant texels grew cards while the ground underfoot grew NOTHING
/// (field report 2026-07-25: "trees still won't render near me"). Barren
/// stays barren with a wide margin: Gobi r/g 1.44, Tibet 1.47, Spain
/// meseta 1.55, Sahara 1.69, Outback 3.12 (see biome_gate_separates_
/// vegetated_from_barren, measured over the shipped albedo).
pub fn veg_biome_ok(sc: [f32; 3]) -> bool {
    sc[0] < sc[1] * 1.25 && sc[1] > sc[2] * 1.04
}

/// Pick the tree species for a spawn cell from `data/vegetation/trees.ron`
/// (v0.1066), replacing the hardcoded `species_fir` bit that gave the entire
/// planet exactly two species.
///
/// BOTH stream sites call this - the card bake and the near-model mirror - and
/// they pass the identical `dir`, `elev_m` and random word, because if they
/// disagree a tree changes species as you walk toward it. That is the invariant
/// the old `species_fir` comment guarded by asking two copies of an expression
/// to stay in sync; it is structural now (one function, one call each) and the
/// purity it depends on is pinned by `tree_mesh::tests::pick_is_deterministic`.
///
/// Falls back to species 0 rather than returning None when no gate passes, so a
/// forest can never silently vanish on a planet whose latitudes or elevations
/// fall outside every authored range.
pub fn pick_tree_species(def: &PlanetDef, dir: DVec3, elev_m: f32, r5: u64) -> usize {
    let reg = crate::renderer::tree_mesh::registry();
    if reg.is_empty() {
        return 0;
    }
    let lat_deg = dir.y.clamp(-1.0, 1.0).asin().to_degrees() as f32;
    reg.pick([dir.x, dir.y, dir.z], lat_deg, elev_m, def.radius, (r5 >> 9) as u32)
        .unwrap_or(0)
}

/// Framing of one sprite tree card (v0.1083, brief item 3b), factored out of
/// `emit_sprite_card` so the contract is unit-testable.
///
/// `fp` is the atlas tile's baked footprint and `h` the height THIS instance
/// should stand. Returns `(side_m, drop_m)`: the square card's side, and how
/// far BELOW the tree's base the card's bottom edge hangs.
///
/// The contract, pinned by `sprite_card_frame_puts_the_tree_on_the_ground`:
/// `v01 == fp.base_offset` lands exactly on the ground, and
/// `v01 == fp.base_offset + h_nominal/frame` lands exactly at `h`. The card is
/// therefore usually a little TALLER than the tree (the baked frame is square
/// on max(width, height) with a 5% margin), which is precisely what the old
/// `let w = h;` got wrong for any crown wider than the tree is tall.
///
/// Written as a RATIO rather than reusing the caller's per-instance jitter: a
/// model-backed species' baked AABB height is the glTF's, not the registry's,
/// and only the ratio form is right for both kinds.
pub(crate) fn sprite_card_frame(
    fp: crate::renderer::tree_mesh::CardFootprint,
    h: f32,
) -> (f32, f32) {
    let s = fp.frame_m * (h / fp.h_nominal_m.max(0.01));
    (s, fp.base_offset * s)
}

pub fn tile_or_base(
    hm: &PlanetHeightmap,
    tiles: Option<&super::terrain_tiles::TerrainTiles>,
    dir: DVec3,
    depth: u8,
) -> (f32, bool) {
    if depth >= TILE_MIN_DEPTH {
        if let Some(t) = tiles {
            // f64 all the way to the grid coordinate (v0.1010): the old
            // .as_vec3() downcast quantized sample positions at ~3.6 m,
            // drawing staircase ripples on smooth slopes (bm-7 report).
            let (lat, lon) = super::planet_heightmap::dir_to_latlon_deg_f64(dir);
            if let Some(m) = t.sample_meters_smooth(lat, lon) {
                let range = hm.max_meters() - hm.min_meters();
                if range > 0.0 {
                    return (((m - hm.min_meters()) / range).clamp(0.0, 1.0), true);
                }
            }
        }
    }
    (hm.normalized_at(dir.as_vec3()), false)
}

/// The DRAWN normalized elevation (base heightmap + land-masked sub-grid
/// detail) at a unit direction, at the FINEST detail depth. This is the single
/// source of truth that the eye-height ground clamp (lib.rs `ground_radius_m`)
/// shares with the mesh builder above, so the player stands ON the drawn ground
/// rather than sinking into the ~4x-exaggerated detail relief and seeing through
/// it (2026-07-12). Uses the finest depth so the clamp matches the HIGHEST LOD
/// -- the eye is then never below even a coarser (not-yet-streamed) patch mesh.
/// Mirrors the elevation formula in `build_patch_mesh` (base + masked detail).
pub fn drawn_elevation_normalized(
    hm: &PlanetHeightmap,
    def: &PlanetDef,
    detail: &DetailNoise,
    tiles: Option<&super::terrain_tiles::TerrainTiles>,
    // f64 (v0.1012): f32 unit dirs quantize ground sampling at ~0.4-0.8 m.
    dir: glam::DVec3,
) -> f32 {
    let (base, from_tile) = tile_or_base(hm, tiles, dir, FINEST_DETAIL_DEPTH);
    let range_m = hm.max_meters() - hm.min_meters();
    if range_m <= 0.0 {
        return base.clamp(0.0, 1.0);
    }
    let sea = def.sea_level.clamp(0.0, 1.0);
    let above_sea_m = (base - sea) * range_m;
    let mask = smoothstep01(above_sea_m / DETAIL_LAND_FADE_M);
    let e = if mask > 0.0 {
        let dm = if from_tile {
            detail.sample_m_tile_gated(dir, FINEST_DETAIL_DEPTH)
        } else {
            detail.sample_m(dir, FINEST_DETAIL_DEPTH)
        };
        base + (dm * mask) / range_m
    } else {
        base
    };
    e.clamp(0.0, 1.0)
}

/// Depth high enough that `DetailNoise::sample_m` enables EVERY fine octave
/// (all `DETAIL_FINE_MIN_DEPTH` gates), so the clamp sees the finest drawn ground.
const FINEST_DETAIL_DEPTH: u8 = 24;

// ── The DRAWN surface, exactly (v0.1091) ──────────────────────────────────
//
// `drawn_elevation_normalized` above answers "what does the elevation FIELD
// say here". That is NOT what you can see. What you see is a mesh: the
// resident patch samples that field at its own lattice (PATCH_TESS steps
// along each patch edge) and the rasteriser LINEARLY INTERPOLATES between
// those samples. The two answers differ by up to a whole quantization tread
// of the base sampler - measured at Fuji, 1.06 m at the 95th percentile,
// with 23.8% of grass tillers placed entirely underground - because the base
// heightmap sampler is an f32 staircase with ~1.4 m treads on a steep flank
// and the mesh's lattice is 0.84 m at depth 17.
//
// So anything that has to SIT ON the visible ground (a blade base, and in
// time the player's boots) must interpolate the drawn face, not re-sample
// the field. `DrawnPatchSurface` does exactly that, and it is exact at any
// depth and any slope by construction: same patch, same lattice, same
// elevation formula, same linear interpolation.
//
// The two costs that make it practical:
//   * LOCATING the patch is pure barycentric arithmetic. Radially projecting
//     the planar medial subdivision of a triangle gives exactly the spherical
//     child triangles the patch tree uses, so "which child contains this
//     direction" is a compare on three floats and the child's barycentrics
//     are an affine map of the parent's - no point-in-triangle tests, no
//     normalize per level.
//   * SAMPLING is memoized per lattice VERTEX. Neighbouring queries share
//     corners, so on a coarse patch (0.84 m lattice) a few thousand tillers
//     cost a few hundred elevation samples.

/// Lattice-vertex memo: a DIRECT-MAPPED cache, not a HashMap.
///
/// The access pattern is tiny and extremely local - a harvest touches a few
/// thousand distinct lattice vertices and asks for each of them a couple of
/// dozen times, in spatial order - so probing, tombstones and growth are all
/// dead weight. A collision simply overwrites and the loser is recomputed
/// later, which is correct because every entry is a pure function of its key.
///
/// The DIRECTION is cached beside the radius. It is a 3-scale, 2-add and a
/// square root, which turned out to cost as much as the lookup guarding it.
struct VertexMemo {
    slots: Vec<(u64, DVec3, f64)>,
    mask: usize,
}

impl VertexMemo {
    fn new() -> Self {
        // 65,536 slots = 3 MB. A 30 m harvest disc touches ~4,000 distinct
        // lattice vertices at depth 17 and ~50,000 at depth 20, so the table
        // is sized for the deep case and effectively collision-free.
        let n = 1 << 16;
        Self { slots: vec![(0, DVec3::ZERO, 0.0); n], mask: n - 1 }
    }
    #[inline]
    fn slot(&self, key: u64) -> usize {
        let mut h = key;
        h ^= h >> 33;
        h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        h ^= h >> 29;
        (h as usize) & self.mask
    }
}

/// The radius of the DRAWN patch surface along a direction, at a given patch
/// tree depth. Reproduces `build_patch_mesh`'s geometry exactly: the same
/// lattice directions, the same depth-gated elevation, and the same flat
/// triangle between them.
///
/// Stateful because it memoizes; create one per harvest and reuse it.
pub struct DrawnPatchSurface<'a, 'b> {
    def: &'a PlanetDef,
    source: &'a ElevationSource<'b>,
    depth: u8,
    bathymetric: bool,
    /// Root icosahedron face of the last query (the fast path: a whole
    /// harvest disc is thousands of times smaller than a root face).
    face: u8,
    /// Child index taken at each level of the last query, and the patch
    /// corners AFTER that step. `corners[0]` is the root face itself, so
    /// `corners[l + 1]` is the patch at depth `l + 1`.
    path: Vec<u8>,
    corners: Vec<[DVec3; 3]>,
    /// Per level, the lengths |c0+c1|/2, |c1+c2|/2, |c2+c0|/2 of that
    /// level's UNNORMALIZED edge midpoints. The descent needs them to
    /// re-express a point's barycentrics in the child's basis (see the note
    /// in `radius_at`); they are a property of the level's corners, so they
    /// are cached beside them and recomputed only where the path diverges.
    mids: Vec<[f64; 3]>,
    /// Running patch-identity key per level, so the vertex memo's key does
    /// not have to fold the whole path on every query.
    pkeys: Vec<u64>,
    /// Deepest level whose patch is known to contain EVERY query (set by
    /// `set_region`). Levels below it are re-walked per query; levels above
    /// are fixed for the whole harvest, so a depth-20 lookup can start at
    /// level 13 and walk 7 levels instead of 20.
    base_level: usize,
    memo: VertexMemo,
    /// Count of elevation samples actually taken (diagnostics + tests).
    pub samples: usize,
}

impl<'a, 'b> DrawnPatchSurface<'a, 'b> {
    pub fn new(def: &'a PlanetDef, source: &'a ElevationSource<'b>, depth: u8) -> Self {
        let bathymetric = matches!(source, ElevationSource::Heightmap { ocean: Some(_), .. });
        Self {
            def,
            source,
            depth,
            bathymetric,
            face: u8::MAX,
            path: Vec::with_capacity(depth as usize),
            corners: Vec::with_capacity(depth as usize + 1),
            mids: Vec::with_capacity(depth as usize + 1),
            pkeys: Vec::with_capacity(depth as usize + 1),
            base_level: 0,
            memo: VertexMemo::new(),
            samples: 0,
        }
    }

    /// Pin the descent to the deepest patch that wholly contains a spherical
    /// cap, so every later query walks only the levels BELOW it.
    ///
    /// A harvest disc is ~30 m and a root face is ~7,000 km, so 12-14 of a
    /// depth-20 walk's levels are the same for every tiller in the harvest.
    /// Walking them 47,000 times is most of the cost of the walk. This runs
    /// the shared prefix ONCE.
    ///
    /// Safe by construction: if a later query somehow falls outside the
    /// pinned patch (a caller that changed centre without re-pinning), the
    /// containment test in `radius_at` fails and it re-walks from the root.
    pub fn set_region(&mut self, center: DVec3, ang_rad: f64) {
        self.face = u8::MAX;
        self.path.clear();
        self.corners.clear();
        self.mids.clear();
        self.pkeys.clear();
        self.base_level = 0;
        let c = center.normalize();
        let Some((f, root)) = root_face_corners()
            .iter()
            .enumerate()
            .find(|(_, rc)| Self::bary(rc, c).is_some())
        else {
            return;
        };
        let root = *root;
        if !Self::cap_inside(&root, c, ang_rad) {
            return;
        }
        self.face = f as u8;
        self.corners.push(root);
        self.mids.push(Self::mid_lens(&root));
        self.pkeys.push(f as u64);
        for _ in 0..self.depth as usize {
            let cur = self.corners[self.base_level];
            let kids = child_corners(&cur);
            let Some((ci, kid)) = kids
                .iter()
                .enumerate()
                .find(|(_, k)| Self::cap_inside(k, c, ang_rad))
            else {
                break;
            };
            let kid = *kid;
            self.corners.push(kid);
            self.mids.push(Self::mid_lens(&kid));
            self.pkeys.push(
                self.pkeys[self.base_level]
                    .wrapping_mul(4)
                    .wrapping_add(ci as u64 + 1),
            );
            self.path.push(ci as u8);
            self.base_level += 1;
        }
    }

    /// Is the spherical cap (`center`, `ang_rad`) wholly inside the spherical
    /// triangle `c`? Each edge is a great circle whose inward unit normal is
    /// the normalized cross product of its endpoints; the cap clears the edge
    /// when the centre is at least `sin(ang)` inside it.
    fn cap_inside(c: &[DVec3; 3], center: DVec3, ang_rad: f64) -> bool {
        let s = ang_rad.sin();
        for i in 0..3 {
            let n = c[i].cross(c[(i + 1) % 3]);
            let nl = n.length();
            if nl < 1e-18 {
                return false;
            }
            // Orient inward using the opposite corner.
            let n = if n.dot(c[(i + 2) % 3]) < 0.0 { -n / nl } else { n / nl };
            if n.dot(center) < s {
                return false;
            }
        }
        true
    }

    /// |c0+c1|/2, |c1+c2|/2, |c2+c0|/2 - the lengths the edge midpoints have
    /// BEFORE `midpoint()` normalizes them.
    #[inline]
    fn mid_lens(c: &[DVec3; 3]) -> [f64; 3] {
        [
            (c[0] + c[1]).length() * 0.5,
            (c[1] + c[2]).length() * 0.5,
            (c[2] + c[0]).length() * 0.5,
        ]
    }

    /// Barycentric weights of `dir` in the spherical triangle `c`, normalized
    /// to sum 1, or None when `dir` is not inside it.
    ///
    /// THE SIGN OF THE SUM IS PART OF THE TEST, not a normalisation detail.
    /// The root faces are CCW from outside, so the scalar triple product is
    /// positive and a contained direction has three positive raw weights. The
    /// ANTIPODAL face has three NEGATIVE raw weights - and dividing three
    /// negatives by their negative sum yields three positives, so a test that
    /// only checks the normalised signs accepts the face on the far side of
    /// the planet. That is not theoretical: it picked the wrong root face at
    /// Amazon (but not at Fuji, which is why it looked like a depth-20 bug)
    /// and returned a radius of exactly -R, the far-side plane crossing.
    #[inline]
    fn bary(c: &[DVec3; 3], dir: DVec3) -> Option<[f64; 3]> {
        let b0 = dir.dot(c[1].cross(c[2]));
        let b1 = dir.dot(c[2].cross(c[0]));
        let b2 = dir.dot(c[0].cross(c[1]));
        if b0 < 0.0 || b1 < 0.0 || b2 < 0.0 {
            return None;
        }
        let s = b0 + b1 + b2;
        if s < 1e-300 {
            return None;
        }
        Some([b0 / s, b1 / s, b2 / s])
    }

    /// The elevation formula `build_patch_mesh` uses, at THIS depth.
    fn mesh_elevation(&self, dir: DVec3) -> f32 {
        match self.source {
            ElevationSource::Heightmap { hm, detail, tiles, .. } => {
                let (base, from_tile) = tile_or_base(hm, *tiles, dir, self.depth);
                let range_m = hm.max_meters() - hm.min_meters();
                if range_m <= 0.0 {
                    return base.clamp(0.0, 1.0);
                }
                let sea = self.def.sea_level.clamp(0.0, 1.0);
                let above_sea_m = (base - sea) * range_m;
                let mask = smoothstep01(above_sea_m / DETAIL_LAND_FADE_M);
                let e = if mask > 0.0 {
                    let dm = if from_tile {
                        detail.sample_m_tile_gated(dir, self.depth)
                    } else {
                        detail.sample_m(dir, self.depth)
                    };
                    base + (dm * mask) / range_m
                } else {
                    base
                };
                e.clamp(0.0, 1.0)
            }
            ElevationSource::Noise(s) => s.elevation_at(dir.as_vec3()),
        }
    }

    /// Planet-local radius in metres of the mesh vertex at lattice (r, c) of
    /// the patch whose corners are `c3` and whose identity key is `pkey`.
    fn vertex(&mut self, pkey: u64, c3: &[DVec3; 3], r: u32, cc: u32) -> (DVec3, f64) {
        let n = PATCH_TESS;
        let w0 = (n - r) as f64;
        let w1 = (r - cc) as f64;
        let w2 = cc as f64;
        // The key is unique per (patch, lattice node), and non-zero so slot
        // 0 can never be mistaken for a filled entry.
        let key = pkey
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ ((r as u64) << 8)
            ^ (cc as u64)
            ^ 0x2545_F491_4F6C_DD1D;
        let si = self.memo.slot(key);
        {
            let e = &self.memo.slots[si];
            if e.0 == key {
                return (e.1, e.2);
            }
        }
        // Bit-identical to build_patch_mesh vertex direction: same integer
        // weights, same product-and-sum order, same normalize.
        let dir = (c3[0] * w0 + c3[1] * w1 + c3[2] * w2).normalize();
        let e = self.mesh_elevation(dir);
        let r_m = self.def.radius
            * if self.bathymetric {
                displaced_radius_f64_true(self.def, e as f64)
            } else {
                displaced_radius_f64(self.def, e as f64)
            };
        self.memo.slots[si] = (key, dir, r_m);
        self.samples += 1;
        (dir, r_m)
    }

    /// Radius (planet-local metres from the planet centre) at which a ray
    /// along `dir` leaves the drawn patch surface.
    pub fn radius_at(&mut self, dir: DVec3) -> f64 {
        let d = dir.normalize();
        // ── Entry level ──
        // Normally the pinned region (set_region) - the deepest patch that
        // contains every query in this harvest, so the walk starts there.
        // Falls back to a full root walk when nothing is pinned or when a
        // query lands outside the pinned patch.
        let mut b = [0.0f64; 3];
        let mut start = self.base_level;
        let mut found = false;
        if start > 0 && start < self.corners.len() {
            if let Some(t) = Self::bary(&self.corners[start], d) {
                b = t;
                found = true;
            }
        }
        if !found {
            start = 0;
            if self.face != u8::MAX {
                let c = root_face_corners()[self.face as usize];
                if let Some(t) = Self::bary(&c, d) {
                    b = t;
                    found = true;
                }
            }
            if !found {
                for (f, c) in root_face_corners().iter().enumerate() {
                    if let Some(t) = Self::bary(c, d) {
                        b = t;
                        self.face = f as u8;
                        self.path.clear();
                        self.corners.clear();
                        self.mids.clear();
                        self.pkeys.clear();
                        self.base_level = 0;
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Numerically on an edge of every face: fall back to the
                    // field sample rather than returning nonsense.
                    return self.def.radius
                        * displaced_radius_f64(self.def, self.mesh_elevation(d) as f64);
                }
            }
            if self.corners.is_empty() {
                let c = root_face_corners()[self.face as usize];
                self.corners.push(c);
                self.mids.push(Self::mid_lens(&c));
                self.pkeys.push(self.face as u64);
            }
        }
        // ── Descend ──
        //
        // Radially projecting the PLANAR medial subdivision of a triangle
        // gives exactly the spherical children `patch_corners` builds: an
        // edge midpoint normalizes onto the same ray as the planar midpoint,
        // and a great-circle edge is the projection of the straight one. So
        // "which child contains this direction" is `b[i] >= 0.5` on the
        // parent's barycentrics - three compares, no point-in-triangle test.
        //
        // RE-EXPRESSING the barycentrics in the child's basis is where this
        // gets subtle, and getting it wrong is silent. Write P in the child's
        // UNNORMALIZED basis first (for child 0, that is c0 and the raw
        // midpoints (c0+c1)/2 and (c2+c0)/2):
        //
        //   P = (b0-b1-b2) c0  +  2*b1 * (c0+c1)/2  +  2*b2 * (c2+c0)/2
        //
        // then convert each raw midpoint to the UNIT corner the patch tree
        // actually uses, which scales that coefficient by the midpoint's
        // length. Skipping that scaling looks right (the coefficients still
        // sum to 1 and the child selection still works) and quietly drifts the
        // lattice position: measured 138 m of error at Fuji depth 17, because
        // a root-face midpoint is only 0.851 long and the distortion compounds
        // down the first few levels.
        let depth = self.depth as usize;
        for level in start..depth {
            let m = self.mids[level];
            let (child, nb): (u8, [f64; 3]) = if b[0] >= 0.5 {
                (0, [b[0] - b[1] - b[2], 2.0 * b[1] * m[0], 2.0 * b[2] * m[2]])
            } else if b[1] >= 0.5 {
                (1, [b[1] - b[2] - b[0], 2.0 * b[2] * m[1], 2.0 * b[0] * m[0]])
            } else if b[2] >= 0.5 {
                (2, [b[2] - b[0] - b[1], 2.0 * b[0] * m[2], 2.0 * b[1] * m[1]])
            } else {
                (
                    3,
                    [
                        (1.0 - 2.0 * b[2]) * m[0],
                        (1.0 - 2.0 * b[0]) * m[1],
                        (1.0 - 2.0 * b[1]) * m[2],
                    ],
                )
            };
            let s = nb[0] + nb[1] + nb[2];
            b = if s.abs() < 1e-300 {
                [1.0, 0.0, 0.0]
            } else {
                [nb[0] / s, nb[1] / s, nb[2] / s]
            };
            // Extend / repair the cached corner chain. Levels whose child is
            // unchanged keep their corners, midpoint lengths and patch key,
            // which is why a harvest disc metres wide only ever pays for its
            // last few levels: the prefix is shared by every query in it.
            let want = level + 1;
            let stale = self.path.len() <= level
                || self.path[level] != child
                || self.corners.len() <= want;
            if stale {
                self.path.truncate(level);
                self.corners.truncate(want);
                self.mids.truncate(want);
                self.pkeys.truncate(want);
                let kid = child_corners(&self.corners[level])[child as usize];
                self.corners.push(kid);
                self.mids.push(Self::mid_lens(&kid));
                self.pkeys
                    .push(self.pkeys[level].wrapping_mul(4).wrapping_add(child as u64 + 1));
                self.path.push(child);
            }
        }
        self.path.truncate(depth);
        self.corners.truncate(depth + 1);
        self.mids.truncate(depth + 1);
        self.pkeys.truncate(depth + 1);
        let c3 = self.corners[depth];
        let pkey = self.pkeys[depth];
        // ── Lattice cell ──
        // Vertex (r, c) carries integer weights (n-r, r-c, c), so
        // r = n*(1-b0) and c = n*b2.
        let n = PATCH_TESS;
        let nf = n as f64;
        let v = ((1.0 - b[0]) * nf).clamp(0.0, nf);
        let u = (b[2] * nf).clamp(0.0, v);
        let ri = (v.floor() as u32).min(n - 1);
        let ci = (u.floor() as u32).min(ri);
        let fv = v - ri as f64;
        let fu = u - ci as f64;
        // Upward cell (fu <= fv) or downward cell - the same two triangles
        // build_patch_mesh emits per lattice square.
        let tri = if fu <= fv {
            [(ri, ci), (ri + 1, ci), (ri + 1, ci + 1)]
        } else {
            [(ri, ci), (ri + 1, ci + 1), (ri, ci + 1)]
        };
        let (d0, r0) = self.vertex(pkey, &c3, tri[0].0, tri[0].1);
        let (d1, r1) = self.vertex(pkey, &c3, tri[1].0, tri[1].1);
        let (d2, r2) = self.vertex(pkey, &c3, tri[2].0, tri[2].1);
        let (p0, p1, p2) = (d0 * r0, d1 * r1, d2 * r2);
        // Where the ray leaves the drawn FACE. Plane intersection rather than
        // a weighted mean of the three radii: the face is what rasterises.
        let nrm = (p1 - p0).cross(p2 - p0);
        let den = d.dot(nrm);
        if den.abs() < 1e-9 {
            return (r0 + r1 + r2) / 3.0;
        }
        p0.dot(nrm) / den
    }
}

// ── Patch identity + geometry derivation ──

/// One node of the per-planet patch tree. `path` packs 2 bits per level
/// (child index 0-3), level 0 in the lowest bits. u64 since the 1 m ladder
/// (v0.875): u32 capped the tree at depth 16 (~6.7 m triangles); 64 bits
/// carry depth 32, far past the depth-20 (~0.4 m) cap actually used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PatchId {
    pub face: u8,
    pub depth: u8,
    pub path: u64,
}

impl PatchId {
    pub fn root(face: u8) -> Self {
        Self { face, depth: 0, path: 0 }
    }

    /// Child i (0-2 = corner triangles keeping corner i, 3 = center).
    pub fn child(&self, i: u32) -> Self {
        Self {
            face: self.face,
            depth: self.depth + 1,
            path: self.path | (((i & 3) as u64) << (2 * self.depth as u32)),
        }
    }

    /// Direct parent (None for roots).
    pub fn parent(&self) -> Option<Self> {
        if self.depth == 0 {
            return None;
        }
        let d = self.depth - 1;
        Some(Self {
            face: self.face,
            depth: d,
            path: self.path & ((1u64 << (2 * d as u32)) - 1),
        })
    }

    /// True if `self` is a strict ancestor of `other` (same root face,
    /// shallower, and `other`'s path starts with `self`'s path).
    pub fn is_ancestor_of(&self, other: &PatchId) -> bool {
        if self.face != other.face || self.depth >= other.depth {
            return false;
        }
        let mask = if self.depth == 0 {
            0
        } else {
            (1u64 << (2 * self.depth as u32)) - 1
        };
        (other.path & mask) == self.path
    }
}

/// The 20 root faces' corner directions in f64 (same vertex table + face
/// winding as terrain::icosphere::Icosphere::new, so patch triangles keep
/// the CCW-from-outside winding the backface-culling pipeline expects).
fn root_face_corners() -> &'static [[DVec3; 3]; 20] {
    use std::sync::OnceLock;
    static CORNERS: OnceLock<[[DVec3; 3]; 20]> = OnceLock::new();
    CORNERS.get_or_init(|| {
        let t = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let raw = [
            DVec3::new(-1.0, t, 0.0),
            DVec3::new(1.0, t, 0.0),
            DVec3::new(-1.0, -t, 0.0),
            DVec3::new(1.0, -t, 0.0),
            DVec3::new(0.0, -1.0, t),
            DVec3::new(0.0, 1.0, t),
            DVec3::new(0.0, -1.0, -t),
            DVec3::new(0.0, 1.0, -t),
            DVec3::new(t, 0.0, -1.0),
            DVec3::new(t, 0.0, 1.0),
            DVec3::new(-t, 0.0, -1.0),
            DVec3::new(-t, 0.0, 1.0),
        ];
        let v: Vec<DVec3> = raw.iter().map(|p| p.normalize()).collect();
        let f = |a: usize, b: usize, c: usize| [v[a], v[b], v[c]];
        [
            f(0, 11, 5),
            f(0, 5, 1),
            f(0, 1, 7),
            f(0, 7, 10),
            f(0, 10, 11),
            f(1, 5, 9),
            f(5, 11, 4),
            f(11, 10, 2),
            f(10, 7, 6),
            f(7, 1, 8),
            f(3, 9, 4),
            f(3, 4, 2),
            f(3, 2, 6),
            f(3, 6, 8),
            f(3, 8, 9),
            f(4, 9, 5),
            f(2, 4, 11),
            f(6, 2, 10),
            f(8, 6, 7),
            f(9, 8, 1),
        ]
    })
}

/// Spherical edge midpoint. `(a + b) * 0.5` then normalize: addition is
/// COMMUTATIVE in IEEE f64, so both neighbors of an edge derive the exact
/// same bits regardless of corner order -- this is what makes same-depth
/// patch borders seamless without any stitching.
#[inline]
fn midpoint(a: DVec3, b: DVec3) -> DVec3 {
    ((a + b) * 0.5).normalize()
}

/// Corner unit directions of a patch, derived by walking its path from the
/// root face. Child layout matches Icosphere::subdivide exactly:
/// child 0 = (v0, m01, m20), 1 = (v1, m12, m01), 2 = (v2, m20, m12),
/// 3 = (m01, m12, m20) -- every child keeps the parent's CCW orientation.
pub fn patch_corners(id: &PatchId) -> [DVec3; 3] {
    let mut c = root_face_corners()[id.face as usize];
    for level in 0..id.depth as u32 {
        let child = (id.path >> (2 * level)) & 3;
        let m01 = midpoint(c[0], c[1]);
        let m12 = midpoint(c[1], c[2]);
        let m20 = midpoint(c[2], c[0]);
        c = match child {
            0 => [c[0], m01, m20],
            1 => [c[1], m12, m01],
            2 => [c[2], m20, m12],
            _ => [m01, m12, m20],
        };
    }
    c
}

/// Corner sets for all 4 children given the parent's corners (avoids
/// re-walking the path from the root during tree descent).
pub fn child_corners(c: &[DVec3; 3]) -> [[DVec3; 3]; 4] {
    let m01 = midpoint(c[0], c[1]);
    let m12 = midpoint(c[1], c[2]);
    let m20 = midpoint(c[2], c[0]);
    [
        [c[0], m01, m20],
        [c[1], m12, m01],
        [c[2], m20, m12],
        [m01, m12, m20],
    ]
}

/// Patch edge arc length in meters at a given depth (the module-header
/// formula: root edge angle halves per split).
pub fn patch_edge_arc_m(depth: u8, radius_m: f64) -> f64 {
    radius_m * ROOT_EDGE_ANGLE_RAD / (1u64 << depth as u64) as f64
}

/// Triangle (vertex) spacing in meters at a given depth.
pub fn vertex_spacing_m(depth: u8, radius_m: f64) -> f64 {
    patch_edge_arc_m(depth, radius_m) / PATCH_TESS as f64
}

// ── Culling primitives ──

/// Six frustum planes as (normal, d): a point p is INSIDE the half-space
/// when dot(n, p) + d >= 0. Extracted Gribb-Hartmann style from a
/// view-projection matrix (works for the reverse-Z celestial projection
/// too: reversed near/far just swaps which extracted plane is which, and
/// we keep all six).
#[derive(Debug, Clone)]
pub struct FrustumPlanes {
    pub planes: [DVec4; 6],
}

impl FrustumPlanes {
    /// Extract from a view-projection matrix (wgpu clip conventions:
    /// -w<=x<=w, -w<=y<=w, 0<=z<=w). Planes are normalized so `d` is a
    /// real distance and bounding-sphere tests are exact.
    pub fn from_view_proj(vp: &DMat4) -> Self {
        let r0 = vp.row(0);
        let r1 = vp.row(1);
        let r2 = vp.row(2);
        let r3 = vp.row(3);
        let raw = [
            r3 + r0, // left:   x >= -w
            r3 - r0, // right:  x <=  w
            r3 + r1, // bottom: y >= -w
            r3 - r1, // top:    y <=  w
            r2,      // z >= 0 (reverse-Z: this is the FAR plane at 1e13)
            r3 - r2, // z <= w (reverse-Z: this is the NEAR plane)
        ];
        let planes = raw.map(|p| {
            let n = DVec3::new(p.x, p.y, p.z);
            let len = n.length().max(1e-30);
            DVec4::new(p.x / len, p.y / len, p.z / len, p.w / len)
        });
        Self { planes }
    }

    /// Re-express the planes in a LOCAL frame related to the render frame
    /// by x_render = translation + rotation * x_local (the planet's model
    /// transform). For plane n.x + d >= 0: substituting gives local normal
    /// rotation^-1 * n and local d of d + dot(n, translation).
    pub fn into_local(&self, rotation: DQuat, translation: DVec3) -> Self {
        let inv = rotation.inverse();
        let planes = self.planes.map(|p| {
            let n = DVec3::new(p.x, p.y, p.z);
            let nl = inv * n;
            DVec4::new(nl.x, nl.y, nl.z, p.w + n.dot(translation))
        });
        Self { planes }
    }

    /// Conservative bounding-sphere test: false only when the sphere is
    /// fully outside at least one plane.
    pub fn sphere_visible(&self, center: DVec3, radius: f64) -> bool {
        for p in &self.planes {
            let n = DVec3::new(p.x, p.y, p.z);
            if n.dot(center) + p.w < -radius {
                return false;
            }
        }
        true
    }
}

/// Per-patch conservative bounds used by both culls and the split metric.
pub struct PatchBounds {
    /// Unit direction of the patch center from the planet center.
    pub center_dir: DVec3,
    /// Max angle (radians) from center_dir to any point of the patch.
    /// For a small geodesic triangle the angular max over the region is
    /// attained at a corner (distance-to-point is geodesically convex),
    /// but edge midpoints are included anyway for slop.
    pub ang_radius: f64,
    /// Bounding sphere center in planet-local meters.
    pub bound_center: DVec3,
    /// Bounding sphere radius in meters (covers the patch's radial band).
    pub bound_radius: f64,
    /// The band max this bound was built with (horizon lift uses it: a
    /// tall patch peeks over the horizon sooner).
    pub max_r_m: f64,
}

/// Radial band a stretch of terrain occupies, in meters from the planet
/// center. Two flavors flow through selection:
/// - CONSERVATIVE (ChunkParams::band): the whole planet's possible range,
///   from displaced_radius_f64 at elevation 0.0 / 1.0. Always safe, but
///   fat: Earth's 4x-exaggerated relief spans ~26 km, which would make a
///   near-surface bounding sphere so thick that frustum culling barely
///   bites (a patch 5 km under the camera would still "poke into" view).
/// - MEASURED (PatchMesh::band, stored in PatchEntry): the actual min/max
///   radii of a BUILT patch's vertices (skirt included). Tight, so built
///   patches near the camera cull sharply. Unbuilt patches fall back to
///   the conservative band, which can only over-include (safe).
#[derive(Debug, Clone, Copy)]
pub struct RadialBand {
    pub min_r_m: f64,
    pub max_r_m: f64,
}

pub fn patch_bounds(corners: &[DVec3; 3], radius_m: f64, band: &RadialBand) -> PatchBounds {
    let center_dir = (corners[0] + corners[1] + corners[2]).normalize();
    let mids = [
        midpoint(corners[0], corners[1]),
        midpoint(corners[1], corners[2]),
        midpoint(corners[2], corners[0]),
    ];
    let mut ang: f64 = 0.0;
    for d in corners.iter().chain(mids.iter()) {
        ang = ang.max(center_dir.dot(*d).clamp(-1.0, 1.0).acos());
    }
    // Tiny slack for the f64 trig round-trip.
    let ang_radius = ang + 1e-9;

    let bound_center = center_dir * radius_m;
    let mut r2: f64 = 0.0;
    for d in corners.iter().chain(mids.iter()).chain([center_dir].iter()) {
        for radial in [band.min_r_m, band.max_r_m] {
            r2 = r2.max((*d * radial - bound_center).length_squared());
        }
    }
    PatchBounds {
        center_dir,
        ang_radius,
        bound_center,
        bound_radius: r2.sqrt() + 1.0, // +1 m slop
        max_r_m: band.max_r_m,
    }
}

/// True when the whole patch is beyond the planet's horizon as seen from
/// the camera (planet-local frame). Standard cone test: from a camera at
/// distance d, surface at occluder radius r_occ is visible out to angular
/// separation acos(r_occ/d) (the tangent ring), and terrain raised to the
/// patch's own max radius peeks over the horizon a further
/// acos(r_occ/max_r). A patch whose NEAREST point (center angle minus
/// angular radius) is beyond both is provably hidden. `occluder_r_m` must
/// be the PLANET-WIDE minimum surface radius (the guaranteed-solid sphere
/// doing the occluding), not the patch's own.
pub fn horizon_culled(bounds: &PatchBounds, cam_local_m: DVec3, occluder_r_m: f64) -> bool {
    let d = cam_local_m.length();
    // At or below the lowest surface the tangent math degenerates; never
    // cull (the camera cannot legitimately be there, but never blank the
    // planet if it is).
    if d <= occluder_r_m * 1.000001 {
        return false;
    }
    let cam_dir = cam_local_m / d;
    let horizon = (occluder_r_m / d).clamp(-1.0, 1.0).acos();
    let lift = (occluder_r_m / bounds.max_r_m.max(occluder_r_m)).clamp(-1.0, 1.0).acos();
    let patch_angle = bounds.center_dir.dot(cam_dir).clamp(-1.0, 1.0).acos();
    patch_angle - bounds.ang_radius > horizon + lift
}

// ── Selection (the per-frame LOD decision) ──

/// Everything the selector needs, precomputed by the caller.
#[derive(Debug, Clone)]
pub struct ChunkParams {
    pub radius_m: f64,
    pub band: RadialBand,
    pub max_depth: u8,
    /// Split while projected vertex spacing exceeds this many pixels.
    pub split_px: f32,
    /// viewport_height_px / vertical_fov_radians: converts an angular size
    /// (small-angle) to on-screen pixels.
    pub px_per_rad: f32,
    pub max_leaves: usize,
    pub max_build_requests: usize,
    /// Occluder radius for the horizon cull, when it should NOT be
    /// `band.min_r_m` (v0.1049). `band.min_r_m` is the guaranteed-solid
    /// sphere for TERRAIN, but the water band subtracts SKIRT_MAX_M (80 km)
    /// even though the water shell emits no skirts, which puts the occluder
    /// 80 km below the sea floor and lets water patches survive out to
    /// ~2020 km of arc at any altitude. Roughly 40% of the water leaf budget
    /// was therefore spent refining ocean BEYOND the horizon, where it can
    /// never be seen - and that starvation is what forced the visible far
    /// field onto a coarse cut in the first place. None = use band.min_r_m
    /// (terrain, unchanged).
    pub occluder_r_m: Option<f64>,
}

/// Selection outcome for one planet this frame. Clone exists for the
/// parked-selection skip (v0.928): a parked camera in surface mode reuses
/// the last full selection instead of re-walking ~30k nodes per frame.
#[derive(Clone)]
pub struct Selection {
    /// Final draw list: complete, non-overlapping cover of the visible
    /// surface (unbuilt leaves are substituted by their nearest built
    /// ancestor, and built descendants of a drawn ancestor are dropped so
    /// nothing z-fights).
    pub draws: Vec<PatchId>,
    /// Missing patches worth building, priority (screen error) descending,
    /// capped at max_build_requests. Re-derived fresh each frame.
    pub build_requests: Vec<PatchId>,
    /// False while some visible region has NO built patch at any depth
    /// (only the first frames after activation, before the 20 pinned roots
    /// finish building). The caller draws the uniform sphere instead then.
    pub fully_covered: bool,
    /// Built patches the walk DEPENDED ON without drawing them: split parents
    /// and provably-invisible drops. The LRU must stamp these too (v0.1077,
    /// operator standstill flicker): a required-but-undrawn patch is invisible
    /// to both eviction guards, so at a capped cache it ages out on the
    /// 120-frame LRU line, its whole subtree collapses to one giant leaf, it
    /// rebuilds, and the cycle repeats every ~6 s. Standing still made it
    /// WORSE because a frozen draw set leaves only these as eviction victims.
    pub required: Vec<PatchId>,
    pub stats: SelectStats,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SelectStats {
    pub visited: usize,
    pub horizon_culled: usize,
    pub frustum_culled: usize,
    pub leaves: usize,
    pub budget_saturated: bool,
    /// Diagnostics: the LARGEST screen error refused by the leaf budget
    /// this selection, and the depth it happened at (0 = none refused).
    pub max_refused_err: f32,
    pub max_refused_depth: u8,
    /// Diagnostics: branch taken by HOT (>1000 px) want-split nodes.
    pub hot_vis_empty: usize,
    pub hot_budget: usize,
    pub hot_missing: usize,
    pub hot_split: usize,
    /// The largest screen error among FINAL LEAVES and its depth.
    pub max_leaf_err: f32,
    pub max_leaf_depth: u8,
}

/// Max-heap node ordered by screen-space error, so the worst error always
/// refines (and requests builds) first; ties break on the id for
/// determinism.
struct HeapNode {
    err_px: f32,
    id: PatchId,
    corners: [DVec3; 3],
    bounds: PatchBounds,
    /// The radial band this node was evaluated with (measured when built,
    /// inherited-from-parent otherwise). Children of an UNBUILT node
    /// inherit it, padded, so their assumed elevation tracks the LOCAL
    /// terrain instead of the planet-wide conservative band (v0.887: on
    /// Rainier the conservative mid-radius sits ~5 km below the summit
    /// camera, so every unbuilt child read as kilometers away and the
    /// descent stalled at depth 14 - coasts split fine, mountains never).
    band: RadialBand,
}

impl PartialEq for HeapNode {
    fn eq(&self, other: &Self) -> bool {
        self.err_px.total_cmp(&other.err_px) == std::cmp::Ordering::Equal && self.id == other.id
    }
}
impl Eq for HeapNode {}
impl PartialOrd for HeapNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.err_px
            .total_cmp(&other.err_px)
            .then_with(|| self.id.cmp(&other.id))
    }
}

/// Projected size of this patch's triangles in pixels: vertex spacing over
/// distance to the patch's nearest bounding point, through px_per_rad.
fn screen_error_px(
    depth: u8,
    bounds: &PatchBounds,
    cam_local_m: DVec3,
    params: &ChunkParams,
) -> f32 {
    let spacing = vertex_spacing_m(depth, params.radius_m);
    // Tangentially-honest distance (v0.887, the smooth-foreground lock-in
    // found parked on Rainier): UNBUILT patches carry the conservative
    // planet-wide radial band, so their bounding spheres reached the camera
    // from up to +-full-relief away - every patch within ~20 km claimed
    // dist = 1 m through the floor, tied at maximum error, and starved the
    // patches actually underfoot. Worse, the v0.883 budget-first rule then
    // refused the build requests that would have replaced those fat bands
    // with tight measured ones: permanent low-detail lock-in. Clamping the
    // effective radius to the patch's own edge length keeps near patches
    // near (their edge dwarfs their band) while distant unbuilt patches
    // read at their true range and stop stealing the budget.
    let edge = patch_edge_arc_m(depth, params.radius_m);
    let eff_r = bounds.bound_radius.min(edge);
    let dist = ((cam_local_m - bounds.bound_center).length() - eff_r).max(1.0);
    ((spacing / dist) * params.px_per_rad as f64) as f32
}

/// Select the patch set to draw this frame. Pure: camera + frustum +
/// built-ness in, draw list + build wishlist out. `frustum` is optional so
/// headless tests can exercise horizon culling and LOD in isolation.
///
/// `is_built` returns the patch's MEASURED radial band when its mesh is
/// resident (None = not built). Built patches are culled with their tight
/// real bounds; unbuilt ones with the planet-wide conservative band (which
/// only over-includes, so streaming never skips something visible).
pub fn select_patches(
    cam_local_m: DVec3,
    frustum: Option<&FrustumPlanes>,
    is_built: &dyn Fn(&PatchId) -> Option<RadialBand>,
    params: &ChunkParams,
) -> Selection {
    select_patches_sticky(cam_local_m, frustum, is_built, params, None)
}

/// select_patches with SELECTION STICKINESS (v0.898): `last_drawn` is the
/// leaf set that was actually drawn last frame. Two stabilizers hang off it,
/// killing the two oscillators behind the operator's "rapidly builds and
/// resets planet chunks of varying size, worse at higher settings":
/// 1. Split/merge hysteresis keys on WAS-DRAWN-SPLIT instead of
///    children-resident (residency stopped meaning "we chose to split"
///    the moment prefetch started building children everywhere).
/// 2. A committed-split budget tier: splits that were drawn last frame may
///    finish 5% past the leaf budget, so the budget wall cannot sweep
///    across the planet re-deciding a different refusal set every frame.
pub fn select_patches_sticky(
    cam_local_m: DVec3,
    frustum: Option<&FrustumPlanes>,
    is_built: &dyn Fn(&PatchId) -> Option<RadialBand>,
    params: &ChunkParams,
    last_drawn: Option<&std::collections::HashSet<PatchId>>,
) -> Selection {
    let mut stats = SelectStats::default();
    let mut heap: BinaryHeap<HeapNode> = BinaryHeap::new();
    // (id, err) of leaves emitted this frame, before fallback substitution.
    let mut leaves: Vec<(PatchId, f32)> = Vec::new();
    let mut requests: Vec<(PatchId, f32)> = Vec::new();
    let mut required: Vec<PatchId> = Vec::new();
    let mut prefetches: usize = 0;

    // Visibility check shared by roots and children. Returns None when
    // culled (and counts why).
    let mut visible = |corners: &[DVec3; 3],
                       band: &RadialBand,
                       stats: &mut SelectStats|
     -> Option<PatchBounds> {
        let b = patch_bounds(corners, params.radius_m, band);
        let occluder_r = params.occluder_r_m.unwrap_or(params.band.min_r_m);
        if horizon_culled(&b, cam_local_m, occluder_r) {
            stats.horizon_culled += 1;
            return None;
        }
        if let Some(f) = frustum {
            if !f.sphere_visible(b.bound_center, b.bound_radius) {
                stats.frustum_culled += 1;
                return None;
            }
        }
        Some(b)
    };

    for face in 0..20u8 {
        let id = PatchId::root(face);
        let corners = patch_corners(&id);
        stats.visited += 1;
        let band = is_built(&id).unwrap_or(params.band);
        if let Some(bounds) = visible(&corners, &band, &mut stats) {
            let err_px = screen_error_px(0, &bounds, cam_local_m, params);
            heap.push(HeapNode { err_px, id, corners, bounds, band });
        }
    }

    while let Some(node) = heap.pop() {
        // Split/merge HYSTERESIS (v0.882; re-keyed v0.898): a hard threshold
        // made boundary patches flip parent<->child every frame. The memory
        // used to be children-residency, but the v0.889 prefetch builds
        // children for EVERY near-threshold node, which dropped those nodes'
        // thresholds to 0.7x and turned the whole prefetch band into a
        // dense flip zone (higher budget = more prefetch = more flicker).
        // Now the memory is WAS-DRAWN-SPLIT: only a node whose children were
        // actually on screen last frame keeps the low keep-split threshold.
        let was_split = last_drawn
            .map(|s| (0..4u32).any(|i| s.contains(&node.id.child(i))))
            .unwrap_or(false);
        // v0.913 (operator: "a lot of terrain LOD flickering even when I'm
        // barely moving or even not at all"): a parked probe showed an
        // ENDLESS split trickle (draws +20-70 per diag tick for minutes) -
        // the planet's own spin drifts every node's screen error, and with
        // the split threshold exactly AT split_px the fringe ring crosses
        // it perpetually, popping patches one by one. A fresh split now
        // needs 1.15x the threshold (was-split keeps the low 0.7x hold),
        // giving spin drift a dead zone to wander in on both sides.
        let split_thr = if was_split {
            params.split_px * 0.7
        } else {
            params.split_px * 1.15
        };
        let want_split = node.err_px > split_thr && node.id.depth < params.max_depth;
        if want_split {
            // Derive + visibility-check the 4 children. Culled children are
            // simply not needed (that region is invisible); the far side of
            // the planet and everything off-screen costs zero geometry.
            let kids_c = child_corners(&node.corners);
            let mut vis: Vec<HeapNode> = Vec::with_capacity(4);
            let mut missing: Vec<(PatchId, f32)> = Vec::new();
            for (i, kc) in kids_c.iter().enumerate() {
                stats.visited += 1;
                let kid = node.id.child(i as u32);
                let built = is_built(&kid);
                // Unbuilt children inherit the PARENT's band (padded ~60 m
                // for deeper detail octaves + skirts): the parent's geometry
                // already brackets the local elevation, so the child's
                // assumed center sits AT the terrain instead of at the
                // planet-wide band's mid-radius kilometers below a summit.
                let band = built.unwrap_or(RadialBand {
                    min_r_m: node.band.min_r_m - 60.0,
                    max_r_m: node.band.max_r_m + 60.0,
                });
                if let Some(kb) = visible(kc, &band, &mut stats) {
                    if built.is_none() {
                        missing.push((kid, node.err_px));
                    }
                    let err_px = screen_error_px(kid.depth, &kb, cam_local_m, params);
                    vis.push(HeapNode { err_px, id: kid, corners: *kc, bounds: kb, band });
                }
            }
            let hot = node.err_px > 1000.0;
            if vis.is_empty() {
                if hot { stats.hot_vis_empty += 1; }
                // This node is BUILT (only built nodes reach the heap) and the
                // selector depended on its residency to reach this decision,
                // but it is never drawn: keep the LRU aware of it (v0.1077).
                required.push(node.id);
                // The 4 children exactly cover the parent and their bounds
                // are conservative SUPERSETS of their regions, so if every
                // child is culled the parent region is provably invisible:
                // drop it entirely (this is what makes "look straight away
                // from the planet" cost zero patches).
                continue;
            }
            // Leaf budget BEFORE build requests (v0.883, operator: "I'm not
            // even moving and the terrain is rapidly switching LODs"). The
            // old order requested missing children first and applied the
            // budget after, so a saturated tree kept COMMISSIONING builds it
            // could never draw: the cache grew to the eviction cap, evicted
            // idle children, which flipped split-hysteresis thresholds and
            // re-shuffled the budget tail every frame - a perpetual
            // build->evict->rebuild wave rolling around the visible set even
            // with the camera parked. Refusing the split BEFORE requesting
            // makes a stationary view converge to a fixed point: the tree
            // refines to the budget, requests stop, evictions stop, and the
            // drawn set becomes frame-to-frame identical.
            let projected_total = leaves.len() + heap.len() + vis.len();
            // Committed-split tier (v0.898): a split that was DRAWN last
            // frame may finish up to 5% past the budget. Without it, the
            // heap-order budget wall lands on a slightly different node set
            // every frame (errors drift with spin/camera), and every node
            // the wall crosses swaps parent<->children - the "chunks of
            // varying size rapidly resetting" the operator reported.
            let budget_cap = if was_split {
                params.max_leaves + params.max_leaves / 20
            } else {
                params.max_leaves
            };
            if projected_total > budget_cap {
                stats.budget_saturated = true;
                if hot { stats.hot_budget += 1; }
                if node.err_px > stats.max_refused_err {
                    stats.max_refused_err = node.err_px;
                    stats.max_refused_depth = node.id.depth;
                }
                leaves.push((node.id, node.err_px));
                continue;
            }
            if !missing.is_empty() {
                if hot { stats.hot_missing += 1; }
                // RESTRICTED DESCENT: cannot split until every visible
                // child mesh exists. Draw self this frame; the requests
                // stream the children in over the next frames.
                for r in missing {
                    requests.push(r);
                }
                leaves.push((node.id, node.err_px));
                continue;
            }
            if hot { stats.hot_split += 1; }
            // Split parents are protected by the drawn-leaf ancestor chains
            // only while a descendant actually draws; a subtree that fully
            // drops leaves its parent unprotected. Required covers both.
            required.push(node.id);
            for k in vis {
                heap.push(k);
            }
        } else {
            // PREFETCH (v0.889): nodes approaching the split threshold get
            // their children built EARLY, so camera motion crosses the
            // threshold into already-resident meshes (no parent-hold pop).
            if node.err_px > params.split_px * 0.55
                && node.id.depth < params.max_depth
                && !stats.budget_saturated
                && prefetches < MAX_PREFETCH_REQUESTS
            {
                for i in 0..4u32 {
                    let kid = node.id.child(i);
                    if is_built(&kid).is_none() {
                        requests.push((kid, node.err_px * 0.5));
                        prefetches += 1;
                    }
                }
            }
            leaves.push((node.id, node.err_px));
        }
    }
    for (lid, lerr) in &leaves {
        if *lerr > stats.max_leaf_err {
            stats.max_leaf_err = *lerr;
            stats.max_leaf_depth = lid.depth;
        }
    }
    stats.leaves = leaves.len();

    // ── Fallback substitution ──
    // A leaf that is not built yet cannot be drawn; walk up to the nearest
    // BUILT ancestor and draw that instead (once). Any built leaf that
    // would be covered by a drawn ancestor is dropped so surfaces never
    // overlap/z-fight. If some leaf has no built ancestor at all the cover
    // has a hole: report fully_covered = false so the caller can keep the
    // uniform sphere up during the first activation frames.
    //
    // NOTE: under the CURRENT restricted-descent rule the only leaves that
    // can be unbuilt are the 20 roots (children are pushed onto the heap
    // only when already built), so the ancestor walk finds nothing and this
    // reduces to "unbuilt root -> fully_covered = false + hole-priority
    // build request". The substitution machinery is kept (and tested)
    // deliberately: it makes the cover correct under ANY future descent
    // rule (e.g. optimistic descent), not just today's.
    let mut fully_covered = true;
    let mut ancestors: Vec<PatchId> = Vec::new();
    let mut draws: Vec<PatchId> = Vec::new();
    for (id, err) in &leaves {
        if is_built(id).is_some() {
            draws.push(*id);
            continue;
        }
        let mut cur = id.parent();
        let mut found = None;
        while let Some(p) = cur {
            if is_built(&p).is_some() {
                found = Some(p);
                break;
            }
            cur = p.parent();
        }
        match found {
            Some(a) => {
                requests.push((*id, *err));
                if !ancestors.contains(&a) {
                    ancestors.push(a);
                }
            }
            None => {
                // A visible region with NO built cover at any depth: this
                // is a hole (only the first activation frames). Build these
                // before everything else so coverage completes fastest.
                requests.push((*id, f32::INFINITY));
                fully_covered = false;
            }
        }
    }
    if !ancestors.is_empty() {
        // Nested ancestors: keep only the shallowest of any chain (a deeper
        // one would be covered by it).
        ancestors.sort(); // (face, depth, path) order puts shallower first per face
        let mut kept: Vec<PatchId> = Vec::new();
        for a in ancestors {
            if !kept.iter().any(|k| k.is_ancestor_of(&a) || *k == a) {
                kept.push(a);
            }
        }
        draws.retain(|d| !kept.iter().any(|k| k.is_ancestor_of(d)));
        draws.extend(kept);
    }

    // Priority-order the build wishlist (worst screen error first), dedupe,
    // cap. Re-derived fresh each frame so nothing goes stale.
    requests.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut build_requests: Vec<PatchId> = Vec::new();
    for (id, _) in requests {
        if build_requests.len() >= params.max_build_requests {
            break;
        }
        if !build_requests.contains(&id) {
            build_requests.push(id);
        }
    }

    Selection { draws, build_requests, fully_covered, stats, required }
}

// ── Patch mesh generation ──

/// Where a patch's vertex elevations come from.
pub enum ElevationSource<'a> {
    /// Real elevation grid + sub-grid detail noise (Earth). This is the
    /// only source the engine wires up this increment; chunking noise-only
    /// planets is the documented extension point (they would pass Noise).
    Heightmap {
        hm: &'a PlanetHeightmap,
        detail: &'a DetailNoise,
        /// Streamed high-detail tile tier (Earth); None = base grid only.
        tiles: Option<&'a super::terrain_tiles::TerrainTiles>,
        /// Connected-ocean mask (Earth, v0.876 real-water Stage 1). When
        /// present the patch renders TRUE BATHYMETRY: below-sea cells are
        /// real depressions (seafloor, dry basins) instead of being clamped
        /// to a smooth sea sphere, and no face carries the water flag --
        /// the translucent ocean shell (material type 16) draws the water.
        ocean: Option<&'a super::ocean_mask::OceanMask>,
    },
    /// Seeded fractal noise, same field the uniform sphere path uses.
    Noise(&'a SurfaceSampler),
}

/// A built patch: mesh data (positions in METERS relative to `anchor`),
/// the f64 anchor itself (planet-local unrotated frame, meters), and the
/// MEASURED radial band of the actual geometry (skirt included) so future
/// selections can cull this patch with tight real bounds instead of the
/// planet-wide conservative band. The GPU hop is renderer
/// Mesh::from_planet_surface, unchanged.
pub struct PatchMesh {
    pub mesh: SurfaceMeshData,
    pub anchor: DVec3,
    pub band: RadialBand,
}

#[inline]
fn smoothstep01(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Triangular grid vertex index for row r (0..=N from corner0), column c
/// (0..=r from the corner1 side toward corner2).
#[inline]
fn grid_idx(r: u32, c: u32) -> usize {
    (r * (r + 1) / 2 + c) as usize
}

/// Border vertex indices in CCW order (corner0 -> corner1 -> corner2 ->
/// back), each vertex once: 3 * N entries. Used to hang the skirt.
fn boundary_indices(n: u32) -> Vec<usize> {
    let mut out = Vec::with_capacity((3 * n) as usize);
    // Edge corner0 -> corner1: column 0, rows 0..n.
    for r in 0..n {
        out.push(grid_idx(r, 0));
    }
    // Edge corner1 -> corner2: bottom row, columns 0..n.
    for c in 0..n {
        out.push(grid_idx(n, c));
    }
    // Edge corner2 -> corner0: the diagonal c == r, rows n..1.
    for r in (1..=n).rev() {
        out.push(grid_idx(r, r));
    }
    out
}

/// One grid triangle's worth of input to `emit_shared_grid_faces`: corner
/// GRID indices (triangular-grid order, winding preserved as given) plus
/// the per-FACE data the provoking vertex must transport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SharedGridFace {
    pub a: usize,
    pub b: usize,
    pub c: usize,
    pub color: [f32; 3],
    pub water: bool,
}

/// Draw-batching increment 3 (v0.1015): SHARED grid vertices via
/// provoking-vertex per-face data.
///
/// Every grid triangle used to emit 3 unique vertices (768/patch) purely
/// so its packed per-face color could ride identical UVs on all corners -
/// sharing would have interpolated (and corrupted) the packed float. Since
/// v0.1013.1 the fragment shader reads the pack through an
/// @interpolate(flat) channel, which takes the value from each triangle's
/// FIRST (provoking) vertex only. So vertices can now be shared: this
/// function emits each unique (grid point, water-flavor) once and arranges
/// the index buffer so every face's first index points at a vertex
/// carrying THAT face's pack:
///
/// - A face tries its three winding-preserving rotations (a,b,c) ->
///   (b,c,a) -> (c,a,b) and picks one whose first vertex is unclaimed (or
///   already claims an identical pack - equal-color faces share).
/// - When all three corners are claimed with different packs, ONE corner
///   is duplicated to carry this face's pack (~103 duplicates worst case
///   on the 16-tess grid: 256 faces vs 153 points; ~280 vertices total vs
///   768 - 2.7x fewer VS invocations plus real post-transform cache reuse).
/// - WATER-flavor: land faces light with the smoothed per-vertex normals,
///   water faces with spherical ones, so a coastline grid point serves the
///   two face kinds through separate flavored copies (position identical).
///
/// Non-provoking vertices keep the color of the first face that touched
/// them: their UV is never flat-read, and grid faces ignore interpolated
/// in.uv entirely (only cards and the water shell consume it, both emitted
/// elsewhere and still unshared). `make_vertex(grid_idx, water)` supplies
/// position + flavor normal; color/water are overwritten on claim.
pub(crate) fn emit_shared_grid_faces(
    faces: &[SharedGridFace],
    mut make_vertex: impl FnMut(usize, bool) -> SurfaceVertexData,
    vertices: &mut Vec<SurfaceVertexData>,
    indices: &mut Vec<u32>,
) {
    let n_slots = faces
        .iter()
        .map(|f| f.a.max(f.b).max(f.c) + 1)
        .max()
        .unwrap_or(0);
    // Per (grid point, flavor): the emitted vertex index and, once some
    // face's provoking corner lands here, the color it claims.
    struct Slot {
        vi: u32,
        claimed: Option<[f32; 3]>,
    }
    let mut land: Vec<Option<Slot>> = (0..n_slots).map(|_| None).collect();
    let mut water: Vec<Option<Slot>> = (0..n_slots).map(|_| None).collect();

    for f in faces {
        let flavor = f.water;
        // Materialize this face's three corner vertices in its flavor.
        let mut vis = [0u32; 3];
        for (k, gi) in [f.a, f.b, f.c].into_iter().enumerate() {
            let slots = if flavor { &mut water } else { &mut land };
            let slot = &mut slots[gi];
            if slot.is_none() {
                let mut v = make_vertex(gi, flavor);
                v.color = f.color;
                v.water = flavor;
                *slot = Some(Slot { vi: vertices.len() as u32, claimed: None });
                vertices.push(v);
            }
            vis[k] = slot.as_ref().unwrap().vi;
        }
        // Winding-preserving rotation whose provoking slot is free or
        // already carries this exact pack.
        let grid = [f.a, f.b, f.c];
        let mut chosen: Option<usize> = None;
        for rot in 0..3 {
            let slots = if flavor { &mut water } else { &mut land };
            let slot = slots[grid[rot]].as_mut().unwrap();
            match slot.claimed {
                None => {
                    slot.claimed = Some(f.color);
                    vertices[slot.vi as usize].color = f.color;
                    chosen = Some(rot);
                    break;
                }
                Some(c) if c == f.color => {
                    chosen = Some(rot);
                    break;
                }
                Some(_) => {}
            }
        }
        match chosen {
            Some(rot) => {
                indices.push(vis[rot]);
                indices.push(vis[(rot + 1) % 3]);
                indices.push(vis[(rot + 2) % 3]);
            }
            None => {
                // All three corners already provoke other packs: duplicate
                // corner A to carry this face's pack.
                let mut v = make_vertex(f.a, flavor);
                v.color = f.color;
                v.water = flavor;
                let dup = vertices.len() as u32;
                vertices.push(v);
                indices.push(dup);
                indices.push(vis[1]);
                indices.push(vis[2]);
            }
        }
    }
}

/// Build one patch's flat-shaded mesh.
///
/// Precision (design constraint 1): every position is computed in f64
/// (unit direction * displaced radius, meters), the patch anchor (center
/// direction * sphere radius) is subtracted in f64, and only the RESULTING
/// small offset is narrowed to f32. At MAX_PATCH_DEPTH the offsets are at
/// most a few tens of km (relief dominates patch size), keeping f32 error
/// in the millimeter range; a test locks sub-meter behavior.
///
/// `albedo`: the planet's real-color grid when it ships one (Earth); face
/// colors then come from imagery via `planet_surface::surface_color`, same
/// as the uniform-sphere path, so the LOD handoff never changes hue.
pub fn build_patch_mesh(
    def: &PlanetDef,
    source: &ElevationSource,
    albedo: Option<&PlanetAlbedo>,
    id: &PatchId,
) -> PatchMesh {
    let n = PATCH_TESS;
    let corners = patch_corners(id);
    let radius_m = def.radius;
    let anchor = (corners[0] + corners[1] + corners[2]).normalize() * radius_m;
    let sea = def.sea_level.clamp(0.0, 1.0);

    // ── Unique grid samples ──
    // Directions via integer barycentric weights: both patches sharing an
    // edge compute the same products and two-term sums (the third weight is
    // zero on an edge), so border directions are bit-identical across
    // same-depth neighbors regardless of corner order (f64 +/* are
    // commutative). That plus the seed-only detail noise makes same-depth
    // borders crack-free by construction.
    let vert_count = ((n + 1) * (n + 2) / 2) as usize;
    let mut dirs: Vec<DVec3> = Vec::with_capacity(vert_count);
    let mut elevs: Vec<f32> = Vec::with_capacity(vert_count);
    for r in 0..=n {
        for c in 0..=r {
            let w0 = (n - r) as f64;
            let w1 = (r - c) as f64;
            let w2 = c as f64;
            let dir = (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize();
            let e = match source {
                ElevationSource::Heightmap { hm, detail, tiles, ocean: _ } => {
                    // Base: real elevation normalized 0..1 - from the
                    // streamed 460 m tile tier at deep LODs when resident,
                    // the shipped base grid otherwise (tile_or_base).
                    let (base, from_tile) = tile_or_base(hm, *tiles, dir, id.depth);
                    // Sub-grid detail (see the module-header rationale):
                    // land-masked so oceans + coastlines stay untouched,
                    // expressed in real meters then folded back into the
                    // normalized domain so it inherits the SAME vertical
                    // exaggeration (surface_relief) as the data. Tile-backed
                    // samples gate the octaves that duplicate tile data.
                    let range_m = hm.max_meters() - hm.min_meters();
                    let above_sea_m = (base - sea) * range_m;
                    let mask = smoothstep01(above_sea_m / DETAIL_LAND_FADE_M);
                    let e = if mask > 0.0 {
                        let dm = if from_tile {
                            detail.sample_m_tile_gated(dir, id.depth)
                        } else {
                            detail.sample_m(dir, id.depth)
                        };
                        base + (dm * mask) / range_m
                    } else {
                        base
                    };
                    e.clamp(0.0, 1.0)
                }
                ElevationSource::Noise(s) => s.elevation_at(dir.as_vec3()),
            };
            dirs.push(dir);
            elevs.push(e);
        }
    }
    // Displaced position in f64 planet-local meters, then the f32 offset
    // from the anchor (the narrowing happens HERE and nowhere earlier).
    // The min/max radii actually seen become the patch's measured band.
    let mut min_r = f64::MAX;
    let mut max_r = f64::MIN;
    // Bathymetric mode (v0.876): with a connected-ocean mask present the
    // sea-sphere clamp is dropped -- below-sea terrain is REAL geometry
    // (ocean floor, dry basins). Above-sea land is bit-identical either
    // way (the clamp only ever affected below-sea cells).
    let bathymetric = matches!(source, ElevationSource::Heightmap { ocean: Some(_), .. });
    let offsets: Vec<glam::Vec3> = dirs
        .iter()
        .zip(&elevs)
        .map(|(d, e)| {
            let r = radius_m
                * if bathymetric {
                    displaced_radius_f64_true(def, *e as f64)
                } else {
                    displaced_radius_f64(def, *e as f64)
                };
            min_r = min_r.min(r);
            max_r = max_r.max(r);
            ((*d * r) - anchor).as_vec3()
        })
        .collect();

    // ── Flat-shaded faces (mirrors planet_surface::build_surface_mesh:
    // underwater = smooth spherical normals on the undisplaced sphere,
    // land = flat geometric normal with an outward fallback + slope
    // shading; per-face color from surface_color so zero color logic is
    // duplicated). ──
    let grid_tris = (n * n) as usize;
    let skirt_tris = (3 * n * 2) as usize;
    let mut vertices: Vec<SurfaceVertexData> = Vec::with_capacity((grid_tris + skirt_tris) * 3);
    let mut indices: Vec<u32> = Vec::with_capacity((grid_tris + skirt_tris) * 3);

    // SMOOTH per-vertex normals (v0.884, operator: "this stepping effect...
    // make it smoother"): flat shading gave every face one normal, so each
    // 0.3 m heightmap-quantization quantum on near-flat plains rendered as
    // a visibly shaded ledge (the Minecraft-step look). Average adjacent
    // face normals per grid vertex; faces then interpolate normals across
    // their corners and the ledges melt into continuous slopes. Per-face
    // COLOR is unchanged (the packed-color transport needs identical
    // corners); only lighting smooths.
    let mut vnorm: Vec<glam::Vec3> = vec![glam::Vec3::ZERO; vert_count];
    {
        let mut acc = |ia: usize, ib: usize, ic: usize| {
            let (p0, p1, p2) = (offsets[ia], offsets[ib], offsets[ic]);
            let n = (p1 - p0).cross(p2 - p0);
            vnorm[ia] += n;
            vnorm[ib] += n;
            vnorm[ic] += n;
        };
        for r in 0..n {
            for c in 0..=r {
                acc(grid_idx(r, c), grid_idx(r + 1, c), grid_idx(r + 1, c + 1));
            }
            for c in 0..r {
                acc(grid_idx(r, c), grid_idx(r + 1, c + 1), grid_idx(r, c + 1));
            }
        }
        for (i, v) in vnorm.iter_mut().enumerate() {
            let out = dirs[i].as_vec3();
            let nn = v.normalize_or_zero();
            // Outward spherical fallback for degenerate or inward sums.
            *v = if nn.length_squared() < 1e-9 || nn.dot(out) < 0.0 { out } else { nn };
        }
    }

    // Per-FACE data (color + water), identical math to the old per-face
    // emission; the winding-preserving provoking-vertex layout is handled
    // by emit_shared_grid_faces (draw-batching increment 3 - see its doc).
    let face_data = |ia: usize, ib: usize, ic: usize| -> SharedGridFace {
        let mean_e = (elevs[ia] + elevs[ib] + elevs[ic]) / 3.0;
        let centroid_dir = ((dirs[ia] + dirs[ib] + dirs[ic]) / 3.0).normalize();
        // Real imagery when the def ships an albedo grid (Earth), the
        // elevation-band classifier otherwise -- shared with the uniform
        // sphere path so zero color logic is duplicated.
        let color = surface_color(def, albedo, centroid_dir.as_vec3(), mean_e);
        // v0.1049 (operator: "the sea floor looks like it has tiger
        // stripes"). In BATHYMETRIC mode these faces are the SEA FLOOR, not
        // the sea surface - the separate water shell draws the water. Flagging
        // them `water` sent the seabed down the type-12 ocean-SURFACE path,
        // which shades a pixel with the three analytic swell trains
        // (2000/850/360 m) plus a Fresnel sky mirror: the floor was literally
        // painted with the ocean's own waves, which is what the diagonal
        // banding was. It also forced RADIAL normals on those vertices, hiding
        // the real bathymetric relief, and the deep-ocean albedo floor left
        // nothing else to look at. The skirt builder in this same function has
        // carried the `!bathymetric` guard since the v0.876 ocean split
        // (whose commit message says "NO TERRAIN FACE CARRIES THE WATER FLAG");
        // the grid-face builder predates the split and never got it.
        let underwater = !bathymetric && def.has_water && mean_e < sea;
        if underwater {
            // water: true drives the shader's sun glint; smooth spherical
            // normals ride the water-flavor vertices.
            SharedGridFace { a: ia, b: ib, c: ic, color, water: true }
        } else {
            let (p0, p1, p2) = (offsets[ia], offsets[ib], offsets[ic]);
            let mut nrm = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            let out = centroid_dir.as_vec3();
            if nrm.length_squared() < 1e-9 || nrm.dot(out) < 0.0 {
                // Degenerate or inward-wound: outward spherical fallback,
                // never an inside-out face.
                nrm = out;
            }
            // Slope shading stays per-FACE (it rides the provoking pack);
            // LIGHTING normals are the smooth per-vertex averages (v0.884)
            // so quantization ledges melt.
            let shade = slope_shade(nrm, out);
            let color = [color[0] * shade, color[1] * shade, color[2] * shade];
            SharedGridFace { a: ia, b: ib, c: ic, color, water: false }
        }
    };

    // Grid triangles: between rows r and r+1 there are r+1 up-pointing and
    // r down-pointing triangles; both windings verified CCW-from-outside
    // (they match the parent corner orientation, which matches the
    // icosphere the backface-culling pipeline already draws correctly).
    // emit_shared_grid_faces only ever ROTATES an index triple, so the
    // orientation survives sharing.
    let mut grid_faces: Vec<SharedGridFace> = Vec::with_capacity(grid_tris);
    for r in 0..n {
        for c in 0..=r {
            grid_faces.push(face_data(
                grid_idx(r, c),
                grid_idx(r + 1, c),
                grid_idx(r + 1, c + 1),
            ));
        }
        for c in 0..r {
            grid_faces.push(face_data(
                grid_idx(r, c),
                grid_idx(r + 1, c + 1),
                grid_idx(r, c + 1),
            ));
        }
    }
    emit_shared_grid_faces(
        &grid_faces,
        |gi, water_flavor| SurfaceVertexData {
            position: offsets[gi].to_array(),
            normal: if water_flavor {
                dirs[gi].as_vec3().to_array()
            } else {
                vnorm[gi].to_array()
            },
            color: [0.0; 3], // overwritten by the emitter
            water: water_flavor,
            tree_card: false,
        },
        &mut vertices,
        &mut indices,
    );

    // ── Procedural vegetation (v0.888; planet-fixed cells v0.897) ──
    // Crossed-quad trunks + diamond canopies for TREES. Plant positions and
    // looks come from a PLANET-FIXED lat/lon cell grid hashed per cell, so
    // every patch depth regenerates the identical plants and LOD swaps never
    // reshuffle the forest.
    //
    // GRASS IS NOT HERE ANY MORE (v0.1090). It used to ride this same loop as
    // a second pass; it is now the camera-relative instanced strand layer
    // (`near_grass_instances`), so this block is trees only and the pass loop
    // that switched between them is gone.
    {
        let range_m = match source {
            ElevationSource::Heightmap { hm, .. } => hm.max_meters() - hm.min_meters(),
            ElevationSource::Noise(_) => 8000.0,
        };
        // Tree cards get the bit-17 mark (v0.912) so the shader can hide a
        // card when the real 3D model stands inside it. A Cell because the
        // emitter closure holds its capture across the whole scatter loop.
        // (Bit 18, the grass-card mark, has no writer any more - see the
        // WIRING note on `near_grass_instances` about removing it.)
        let marking_trees = std::cell::Cell::new(false);
        let mut emit_card = |base: glam::Vec3,
                             up: glam::Vec3,
                             side: glam::Vec3,
                             w: f32,
                             h0: f32,
                             h1: f32,
                             color: [f32; 3],
                             vertices: &mut Vec<SurfaceVertexData>,
                             indices: &mut Vec<u32>| {
            // One two-sided quad from h0 to h1 along up, w wide along side.
            let p00 = base + up * h0 - side * (w * 0.5);
            let p01 = base + up * h0 + side * (w * 0.5);
            let p10 = base + up * h1 - side * (w * 0.5);
            let p11 = base + up * h1 + side * (w * 0.5);
            // Light vegetation like the GROUND under it (v0.896): the card
            // plane normal is horizontal, so an overhead sun gave N.L ~ 0 and
            // every tree rendered as a black slab at noon (probe capture).
            // The radial up matches the terrain shading exactly.
            let nrm = up;
            // INDEXED (v0.1083): four triangles - two quads, the second pair
            // wound backwards so the card survives back-face culling - built
            // from only FOUR distinct corners. This used to push a fresh
            // vertex per corner per triangle, storing 12 where 4 do: 384 bytes
            // of vertices against 128. Cards were 86% of the 1.2 GB vertex
            // arena at a forest vantage (33.56M of 39.32M elements, 98.8%
            // used) and every config that overflowed the arena ran 93-98 ms
            // against 29-64 ms for one that did not.
            //
            // BIT-IDENTICAL by construction: same four corners, same triangle
            // order, same corner order within each triangle, same attributes.
            // Sharing corners is safe for the FLAT-interpolated packed colour
            // (00-bindings-vertex.wgsl:242) because all four corners of a
            // coloured card carry the same value.
            let base_i = vertices.len() as u32;
            for p in [p00, p01, p11, p10] {
                vertices.push(SurfaceVertexData {
                    position: p.to_array(),
                    normal: nrm.to_array(),
                    color,
                    water: false,
                    tree_card: marking_trees.get(),
                });
            }
            // p00=0, p01=1, p11=2, p10=3
            for i in [0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
                indices.push(base_i + i);
            }
        };
        // Sprite tree card (v0.961): one two-sided SQUARE quad (the baked
        // sprite frame is square, height-dominant) textured from the conifer
        // atlas. Transport: the color channel carries a sentinel the mesh
        // uploader converts to the shader's negative-uv encoding
        // (color[0] = -1, color[1] = -(1 + tile + u01*0.5), color[2] = v01;
        // see Mesh::from_planet_surface + the type-12 uv.x < -0.5 branch).
        // The tiny |uv.x| base keeps f32 interpolation of u01 sub-texel.
        // Normal stays the radial up - sprite cards light like the ground,
        // exactly as the colored cards did.
        //
        // FOOTPRINT (v0.1083, brief item 3b). The card is NOT `h` by `h`. The
        // baker frames a SQUARE on max(width, height) of the model's joint
        // AABB, so a tile holds the whole tree plus whatever margin the wider
        // dimension forced. Drawing that tile as an h-by-h quad was harmless
        // while only fir and pine took this path (both height-dominant, 5%
        // margin), and wrong the moment wide crowns arrived: acacia's crown is
        // wider than the tree is tall (tree_mesh.rs `umbrella`), which frames
        // at 1.365h and would render 27% too short with its trunk base 13% of
        // the card height OFF THE GROUND. That is a motion cue - every
        // wide-crowned tree would hop as you crossed the 120 m model handoff.
        //
        // So the card takes the tile's real framing: side = frame_m scaled by
        // this tree's height against the baked one, dropped so the tree's base
        // lands on the ground. `v01` now spans the FRAME rather than the tree,
        // which is what the baked pixels actually occupy.
        let fp_table = crate::renderer::tree_mesh::card_footprint_table();
        let mut emit_sprite_card = |base: glam::Vec3,
                                    up: glam::Vec3,
                                    side: glam::Vec3,
                                    h: f32,
                                    tile: u32,
                                    vertices: &mut Vec<SurfaceVertexData>,
                                    indices: &mut Vec<u32>| {
            let fp = fp_table[(tile as usize).min(fp_table.len() - 1)];
            let (s, drop_m) = sprite_card_frame(fp, h);
            let foot = base - up * drop_m;
            let corner = |u01: f32, v01: f32| -> (glam::Vec3, [f32; 3]) {
                let p = foot + up * (s * v01) + side * (s * (u01 - 0.5));
                let enc_x = -((1 + tile) as f32 + u01 * 0.5);
                (p, [-1.0, enc_x, v01])
            };
            // INDEXED, same four-corner sharing as emit_card above. Sprite
            // corners each carry a DIFFERENT u01 on the SMOOTH uv channel
            // (that is how the tile encoding rides), so the shared corner
            // keeps exactly the attributes it had per triangle - do not
            // "fix" them into agreeing.
            let corners = [
                corner(0.0, 0.0), // c00
                corner(1.0, 0.0), // c10
                corner(1.0, 1.0), // c11
                corner(0.0, 1.0), // c01
            ];
            let nrm = up.to_array();
            let base_i = vertices.len() as u32;
            for (p, col) in corners {
                vertices.push(SurfaceVertexData {
                    position: p.to_array(),
                    normal: nrm,
                    color: col,
                    water: false,
                    tree_card: true,
                });
            }
            // c00=0, c10=1, c11=2, c01=3: the original
            // [c00,c10,c11] [c00,c11,c01] [c00,c11,c10] [c00,c01,c11].
            for i in [0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2] {
                indices.push(base_i + i);
            }
        };
        let want_trees = id.depth >= TREE_MIN_DEPTH;
        // Spherical point-in-triangle: a direction is inside the patch when
        // it sits on the same side of each edge's great-circle plane as the
        // opposite corner.
        let cn = [
            corners[0].normalize(),
            corners[1].normalize(),
            corners[2].normalize(),
        ];
        let edge_n = [
            cn[0].cross(cn[1]),
            cn[1].cross(cn[2]),
            cn[2].cross(cn[0]),
        ];
        let edge_s = [
            edge_n[0].dot(cn[2]),
            edge_n[1].dot(cn[0]),
            edge_n[2].dot(cn[1]),
        ];
        let inside =
            |d: glam::DVec3| -> bool { (0..3).all(|i| edge_n[i].dot(d) * edge_s[i] >= 0.0) };
        // Patch bbox in lat/lon; unwrap longitudes into a continuous window
        // across the antimeridian.
        let mut lats = [0.0f64; 3];
        let mut lons = [0.0f64; 3];
        for i in 0..3 {
            lats[i] = cn[i].y.clamp(-1.0, 1.0).asin();
            lons[i] = (-cn[i].z).atan2(cn[i].x);
        }
        let lat_min = lats.iter().cloned().fold(f64::MAX, f64::min);
        let lat_max = lats.iter().cloned().fold(f64::MIN, f64::max);
        let raw_span = lons.iter().cloned().fold(f64::MIN, f64::max)
            - lons.iter().cloned().fold(f64::MAX, f64::min);
        if raw_span > std::f64::consts::PI {
            for l in lons.iter_mut() {
                if *l < 0.0 {
                    *l += std::f64::consts::TAU;
                }
            }
        }
        let lon_min = lons.iter().cloned().fold(f64::MAX, f64::min);
        let lon_max = lons.iter().cloned().fold(f64::MIN, f64::max);
        // No vegetation on the polar caps (lon cells degenerate there and
        // the biome gate would reject the ice anyway).
        let polar = lat_max.abs().max(lat_min.abs()) > 1.53;
        // ONE pass (v0.1090): trees. This used to be `for pass in 0..2` with
        // the second pass emitting grass tufts on their own cell grid.
        if want_trees && !polar {
            marking_trees.set(true);
            let cell = TREE_CELL_RAD;
            let per_cell = (((TREES_PER_CELL as f32) * veg_density()).round() as u32).max(1);
            let salt: u64 = 0x51F0_A11C;
            let ylo = ((lat_min / cell).floor() as i64) - 1;
            let yhi = ((lat_max / cell).floor() as i64) + 1;
            let xlo = ((lon_min / cell).floor() as i64) - 1;
            let xhi = ((lon_max / cell).floor() as i64) + 1;
            for iy in ylo..=yhi {
                let cell_lat = (iy as f64 + 0.5) * cell;
                // Constant per-AREA density: lon cells narrow toward the
                // poles by cos(lat), so thin the per-cell count to match.
                let count = ((per_cell as f64) * cell_lat.cos().max(0.0)).round() as u32;
                for ix in xlo..=xhi {
                    // Per-cell deterministic stream, independent of the
                    // evaluating patch. Every item draws a FIXED number of
                    // randoms (6) before any gate, so neighbouring patches
                    // that share this cell stay stream-aligned and agree on
                    // every plant's position and look.
                    let mut s = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ (iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                        ^ salt;
                    // A zero state would stick; xorshift needs a nonzero seed.
                    if s == 0 {
                        s = 0x94D0_49BB_1331_11EB;
                    }
                    let mut next = move || {
                        s ^= s << 13;
                        s ^= s >> 7;
                        s ^= s << 17;
                        s
                    };
                    for _ in 0..count {
                        let r0 = next();
                        let r1 = next();
                        let r2 = next();
                        let r3 = next();
                        let r4 = next();
                        let r5 = next();
                        let lat = (iy as f64 + (r0 % 4096) as f64 / 4096.0) * cell;
                        let lon = (ix as f64 + (r1 % 4096) as f64 / 4096.0) * cell;
                        let cl = lat.cos();
                        let dir =
                            glam::DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
                        if !inside(dir) {
                            continue;
                        }
                        // Elevation through the SAME sampler as the grid.
                        let (e, _tile) = match source {
                            ElevationSource::Heightmap { hm, tiles, .. } => {
                                tile_or_base(hm, *tiles, dir, id.depth)
                            }
                            ElevationSource::Noise(sm) => {
                                (sm.elevation_at(dir.as_vec3()), false)
                            }
                        };
                        let elev_m = (e - sea) * range_m;
                        // Land only, below the treeline, above the beach.
                        // Floor raised 3 -> 6 m (v0.1018, operator: "weird
                        // strip of trees on the ocean"): coastal shelves are
                        // so flat that the first legal contour formed a
                        // shoreline-hugging tree line on the tidal flat,
                        // fed by blue-blurred imagery passing the green
                        // gate. 6 m is the storm-surge line - real beaches
                        // and flats stay bare.
                        if elev_m < 6.0 || elev_m > TREELINE_M {
                            continue;
                        }
                        // Biome gate (v0.896, loosened v0.955): vegetation
                        // where the surface COLOR reads vegetated - the same
                        // imagery/ramp the ground renders with. Real Earth
                        // imagery is the planet-wide biome map for free.
                        let sc = surface_color(def, albedo, dir.as_vec3(), e);
                        if !veg_biome_ok(sc) {
                            continue;
                        }
                        let r = radius_m
                            * if bathymetric {
                                displaced_radius_f64_true(def, e as f64)
                            } else {
                                displaced_radius_f64(def, e as f64)
                            };
                        let base = ((dir * r) - anchor).as_vec3();
                        let up = dir.as_vec3();
                        let az = (r2 % 6283) as f32 / 1000.0;
                        let east = glam::Vec3::Y.cross(up).normalize_or_zero();
                        let north = up.cross(east).normalize_or_zero();
                        let side_a = (east * az.cos() + north * az.sin()).normalize_or_zero();
                        let side_b = up.cross(side_a).normalize_or_zero();
                        if side_a.length_squared() < 0.5 {
                            continue; // polar degenerate
                        }
                        // v0.1090: this used to be `if !is_tree { grass } else
                        // { tree }`. The grass arm emitted two crossed
                        // 0.5 m x 0.25-0.50 m opaque quads in a hardcoded
                        // straw green (R and B were literally constant for
                        // every tuft on the planet while the ground came from
                        // real imagery, a 2.59x brightness break); it is gone,
                        // replaced by `near_grass_instances`. The block braces
                        // stay so the tree body keeps its indentation and this
                        // diff stays readable.
                        {
                            // Tree: trunk cards (brown) + canopy cards (dark
                            // green), 7-13 m - a conifer silhouette at range.
                            // v0.913: wider size spread (operator: "varied size... they all
                            // seem uniform height"). 4-18 m, skewed toward
                            // younger trees; BOTH stream sites (bake + the
                            // near-model mirror) must stay identical.
                            // v0.1066: species comes from data/vegetation/trees.ron
                            // (was a hardcoded fir/pine bit). v0.914 (operator:
                            // "set all the trees to the max height of their tree
                            // species"): full-grown, with a natural spread.
                            // BOTH stream sites stay identical.
                            let sp_i = pick_tree_species(def, dir, elev_m, r5);
                            let reg = crate::renderer::tree_mesh::registry();
                            let sp = reg.get(sp_i);
                            let (sp_h, sp_jit) = match sp {
                                Some(t) => (t.height_m, t.height_jitter),
                                None => (22.0, 0.12),
                            };
                            let jitter =
                                1.0 - sp_jit + (r3 % 100) as f32 / 100.0 * (sp_jit * 2.0);
                            let h = sp_h * jitter;
                            // Sprite cards (v0.961, billboard bake increment
                            // 2; EVERY species since v0.1083): on imagery
                            // planets the card is ONE crossed pair of quads
                            // textured from the baked atlas - the same tile
                            // the near 3D model was baked from, so the LOD
                            // handoff keeps the exact silhouette. Procedural
                            // species used to be excluded here (they had no
                            // baked tile, so a pink sakura grove would have
                            // turned into fir silhouettes) and fell through to
                            // coloured rectangles; the baker now bakes them
                            // too, from the identical mesh generator.
                            // Variant picks match near_tree_instances exactly
                            // ((r5 >> 11) % 3).
                            let variant = ((r5 >> 11) % 3) as u32;
                            let tile = crate::renderer::tree_mesh::tile_of(sp_i, variant);
                            if let (true, Some(tile)) = (albedo.is_some(), tile) {
                                emit_sprite_card(base, up, side_a, h, tile, &mut vertices, &mut indices);
                                emit_sprite_card(base, up, side_b, h, tile, &mut vertices, &mut indices);
                            } else {
                                // No atlas at all (a NOISE planet has no
                                // imagery and no bake) or a species past the
                                // atlas ceiling: coloured trunk + canopy
                                // cards, tinted from the species row so a
                                // distant sakura grove still reads pink.
                                let (trunk, canopy) = match sp {
                                    Some(t) => {
                                        let leafy = if t.blossom_frac > 0.5 {
                                            t.blossom_color
                                        } else {
                                            t.leaf_color
                                        };
                                        (
                                            t.trunk_color,
                                            [
                                                leafy[0] + (r4 % 60) as f32 / 1000.0,
                                                leafy[1] + (r5 % 80) as f32 / 1000.0,
                                                leafy[2],
                                            ],
                                        )
                                    }
                                    None => (
                                        [0.30, 0.22, 0.13],
                                        [
                                            0.08 + (r4 % 60) as f32 / 1000.0,
                                            0.26 + (r5 % 80) as f32 / 1000.0,
                                            0.10,
                                        ],
                                    ),
                                };
                                emit_card(base, up, side_a, 0.5, 0.0, h * 0.35, trunk, &mut vertices, &mut indices);
                                emit_card(base, up, side_b, 0.5, 0.0, h * 0.35, trunk, &mut vertices, &mut indices);
                                emit_card(base, up, side_a, h * 0.55, h * 0.25, h, canopy, &mut vertices, &mut indices);
                                emit_card(base, up, side_b, h * 0.55, h * 0.25, h, canopy, &mut vertices, &mut indices);
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Skirt (design constraint 3) ──
    // A vertical apron hanging from the border toward the planet center,
    // sealing cracks against ANY coarser/finer neighbor. Depth scales with
    // patch size (bigger patches can disagree by more meters).
    let edge_m = patch_edge_arc_m(id.depth, radius_m);
    let skirt_depth = (edge_m * SKIRT_EDGE_FRACTION).clamp(SKIRT_MIN_M, SKIRT_MAX_M);
    let border = boundary_indices(n);
    let m = border.len();
    for s in 0..m {
        let ia = border[s];
        let ib = border[(s + 1) % m];
        let b0 = offsets[ia];
        let b1 = offsets[ib];
        // Drop straight toward the planet center (along the vertex's own
        // radial direction) so the apron is truly vertical.
        let s0 = b0 - dirs[ia].as_vec3() * skirt_depth as f32;
        let s1 = b1 - dirs[ib].as_vec3() * skirt_depth as f32;
        // One color + one smooth normal per segment (flat-shading transport
        // requires all 3 corners of a face to carry identical packed color).
        // Same surface_color source as the grid faces so the apron blends
        // in; no slope shading (the normal is radial, shade would be 1.0),
        // and the water flag follows the same below-sea rule.
        let mean_e = (elevs[ia] + elevs[ib]) / 2.0;
        let mid_dir = midpoint(dirs[ia], dirs[ib]);
        let color = surface_color(def, albedo, mid_dir.as_vec3(), mean_e);
        let skirt_water = !bathymetric && def.has_water && mean_e < sea;
        let nrm = mid_dir.as_vec3().to_array();
        // Winding: walking the border CCW (seen from outside), the wall
        // must face AWAY from the patch interior; (s0, s1, b1) + (s0, b1,
        // b0) give outward-facing CCW triangles (derivation in the increment
        // notes; a flipped skirt would be backface-culled exactly when it
        // is needed).
        for tri in [[s0, s1, b1], [s0, b1, b0]] {
            for p in tri {
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: p.to_array(),
                    normal: nrm,
                    color,
                    water: skirt_water,
                    tree_card: false,
                });
            }
        }
    }

    PatchMesh {
        mesh: SurfaceMeshData { vertices, indices },
        anchor,
        band: RadialBand {
            // The skirt hangs skirt_depth below the lowest grid vertex;
            // include it so culling never clips a visible apron. A meter
            // of slop each way absorbs f32 offset rounding.
            min_r_m: min_r - skirt_depth - 1.0,
            max_r_m: max_r + 1.0,
        },
    }
}

/// Prefetch cap per selection (v0.889): how many near-threshold children
/// may be requested ahead of need each frame. Small enough that the cache
/// cannot balloon to its eviction cap (the v0.883 churn), large enough
/// that steady motion always has the next ring of detail ready.
pub const MAX_PREFETCH_REQUESTS: usize = 12;

/// Water-shell patch cap. History: 14 (~38 m verts - only km swells were
/// meshable, the "flat 2D shape" report); 17 (v0.957, ~4.8 m - the 18-50 m
/// chop became real); 20 (v0.964, operator: "I imagine we wouldn't need
/// resolution for water any lower than 0.1m... maybe 0.5m") = ~0.6 m
/// vertices right at the eye, so even the 6 m ripple train is true
/// geometry at the waterline. Selection stays pixel-driven, so the deep
/// tiers exist only within tens of metres of the camera - the far ocean
/// costs exactly what it did.
pub const WATER_MAX_PATCH_DEPTH: u8 = 20;

/// Divisor that packs a water patch's vertex spacing into the vertex blue
/// channel (which `pack_color_to_uv` clamps to 0..1 and forwards as `uv.y`).
/// 65536 m covers the coarsest water leaf ever drawn (depth 3 = ~52 km cells
/// clamp to 1.0, and anything that coarse is gated off entirely anyway).
/// LOCKSTEP with `WATER_CELL_CODE_SCALE` in 00-bindings-vertex.wgsl.
pub const WATER_CELL_CODE_SCALE: f32 = 65536.0;

/// Water-shell leaf budget: six deeper tiers need more near-camera
/// leaves; MAX_OBJECTS is 16384 today, so 512 is still a small slice.
///
/// v0.1048: 512 -> 1024. With the water error floor added alongside (see
/// the wparams split_px in lib.rs) this is headroom for a high eye over open
/// ocean, where the visible sea area - and so the leaf demand - grows with
/// altitude; 512 covered a 3 m eye but not a 13 m one.
///
/// MEASURED, v0.1045: at 512 the ocean genuinely runs ~1.5 LOD levels
/// COARSER than the pixel-error target (the split heap hits the cap and
/// cuts every request above ~11-14 px of error instead of the 4.6 px the
/// selector asks for), so cross-LOD borders carry a bigger wave-height
/// mismatch than the selector intends. Raising this to 2048 was tried and
/// REVERTED: at a grazing dusk vantage it cost ~26 ms/frame (34 -> 60 ms)
/// and changed nothing visible, because the artifact it was meant to fix
/// (the operator's flat pale tiles) was the BACKSTOP's mismatched shading,
/// not coverage - see the type-16 backstop branch in 90-fragment-main.wgsl.
/// If the residual dusk seam ever needs attacking, make this a Settings
/// slider like terrain_patch_budget rather than raising the default.
pub const WATER_MAX_LEAVES: usize = 1024;

/// One near-field tree from the planet-fixed vegetation stream (v0.911):
/// the same cell hash the patch bake emits silhouette cards from,
/// re-enumerated at runtime so REAL 3D models can stand where the cards
/// are. dir is the planet-local unit direction, r_m the drawn ground
/// radius at the base; yaw/height mirror the card's own randoms.
pub struct NearTree {
    pub dir: DVec3,
    pub r_m: f64,
    pub yaw: f32,
    pub height_m: f32,
    /// 0 = fir, 1 = pine (stable per tree).
    pub species: u8,
    /// Shape variant 0-2 (the three photoscan variants per species).
    pub variant: u8,
}

/// Enumerate trees within `radius_m` surface metres of `center_dir` on the
/// planet-fixed tree grid: the SAME deterministic per-cell stream, gates
/// (treeline, beach, imagery-green biome), and ground sampling as
/// build_patch_mesh's vegetation pass, so every returned tree coincides
/// with a baked card (the model hides its card inside it). Capped at
/// `max_n` (cells walk outward from the center row-major; a generous cap
/// simply stops early).
pub fn near_tree_instances(
    def: &PlanetDef,
    source: &ElevationSource,
    albedo: Option<&PlanetAlbedo>,
    center_dir: DVec3,
    radius_m: f64,
    max_n: usize,
) -> Vec<NearTree> {
    let mut out = Vec::new();
    let center = center_dir.normalize();
    let lat_c = center.y.clamp(-1.0, 1.0).asin();
    // No trees on the caps (mirrors the bake's polar gate).
    if lat_c.abs() > 1.5 {
        return out;
    }
    // Gate diagnostics (operator field report 2026-07-25: every recompute
    // returned 0 while cards rendered): count WHY candidates die so the
    // run.log names the guilty gate instead of just "0 trees".
    let mut n_total = 0u32;
    let mut n_out = 0u32;
    let mut n_elev = 0u32;
    let mut n_green = 0u32;
    let mut elev_samples: Vec<f32> = Vec::new();
    let mut green_samples: Vec<[f32; 3]> = Vec::new();
    let lon_c = (-center.z).atan2(center.x);
    let ang = radius_m / def.radius.max(1.0);
    let cos_ang = ang.cos();
    let sea = def.sea_level.clamp(0.0, 1.0);
    let range_m = match source {
        ElevationSource::Heightmap { hm, .. } => hm.max_meters() - hm.min_meters(),
        ElevationSource::Noise(_) => 1.0,
    };
    let bathymetric = matches!(source, ElevationSource::Heightmap { ocean: Some(_), .. });
    let cell = TREE_CELL_RAD;
    let salt: u64 = 0x51F0_A11C;
    let lat_span = ang / cell;
    let lon_span = ang / (cell * lat_c.cos().max(0.05));
    let ylo = ((lat_c / cell).floor() as i64) - lat_span.ceil() as i64 - 1;
    let yhi = ((lat_c / cell).floor() as i64) + lat_span.ceil() as i64 + 1;
    let xlo = ((lon_c / cell).floor() as i64) - lon_span.ceil() as i64 - 1;
    let xhi = ((lon_c / cell).floor() as i64) + lon_span.ceil() as i64 + 1;
    // Walk cells NEAREST-FIRST (v0.969, operator: "I see their shadows, but
    // I can never get close"): the old row-major walk filled the max_n cap
    // from the disc's south-west corner, which was harmless at the old
    // density but at 8x (v0.963) the cap fills before the walk ever reaches
    // the camera's own cell - every drawn model sat in a southern stripe,
    // the card-hide radius engaged anyway, and the player stood on
    // shadowed-but-treeless ground (card shadows persist because the shadow
    // pass's "camera" is the sun, so the hide-discard never fires there).
    // Distance-sorted cells make the cap collect the trees AROUND the
    // camera; lon distance is cos(lat)-weighted to keep rings round.
    let cy = lat_c / cell;
    let cx = lon_c / cell;
    let coslat = lat_c.cos().max(0.05);
    let mut cells: Vec<(i64, i64)> = Vec::with_capacity(
        ((yhi - ylo + 1) * (xhi - xlo + 1)).max(0) as usize,
    );
    for iy in ylo..=yhi {
        for ix in xlo..=xhi {
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
    for (iy, ix) in cells {
        let cell_lat = (iy as f64 + 0.5) * cell;
        let count = ((TREES_PER_CELL as f64)
            * (veg_density() as f64)
            * cell_lat.cos().max(0.0))
        .round() as u32;
        {
            // Identical stream to the bake: 6 randoms per item BEFORE any
            // gate, so positions/looks agree exactly with the cards.
            let mut s = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                ^ salt;
            if s == 0 {
                s = 0x94D0_49BB_1331_11EB;
            }
            let mut next = move || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            };
            for _ in 0..count {
                let r0 = next();
                let r1 = next();
                let r2 = next();
                let r3 = next();
                let _r4 = next();
                let r5 = next();
                if out.len() >= max_n {
                    return out;
                }
                n_total += 1;
                let lat = (iy as f64 + (r0 % 4096) as f64 / 4096.0) * cell;
                let lon = (ix as f64 + (r1 % 4096) as f64 / 4096.0) * cell;
                let cl = lat.cos();
                let dir = DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
                if dir.dot(center) < cos_ang {
                    n_out += 1;
                    continue;
                }
                let (e, _tile) = match source {
                    ElevationSource::Heightmap { hm, tiles, .. } => {
                        // Depth 20 = the deepest bake tier, so the tile-
                        // gated sampler matches close patches.
                        tile_or_base(hm, *tiles, dir, 20)
                    }
                    ElevationSource::Noise(sm) => (sm.elevation_at(dir.as_vec3()), false),
                };
                let elev_m = (e - sea) * range_m;
                if elev_m < 6.0 || elev_m > TREELINE_M {
                    n_elev += 1;
                    if elev_samples.len() < 4 {
                        elev_samples.push(elev_m);
                    }
                    continue;
                }
                let sc = surface_color(def, albedo, dir.as_vec3(), e);
                if !veg_biome_ok(sc) {
                    n_green += 1;
                    if green_samples.len() < 4 {
                        green_samples.push(sc);
                    }
                    continue;
                }
                let r = def.radius
                    * if bathymetric {
                        displaced_radius_f64_true(def, e as f64)
                    } else {
                        displaced_radius_f64(def, e as f64)
                    };
                let yaw = (r2 % 6283) as f32 / 1000.0;
                // v0.913: wider size spread (operator: "varied size... they all
                            // seem uniform height"). 4-18 m, skewed toward
                            // younger trees; BOTH stream sites (bake + the
                            // near-model mirror) must stay identical.
                            // v0.1066: MUST match the bake site above exactly -
                            // same inputs, same helper - or a tree changes
                            // species as you walk toward it.
                            let sp_i = pick_tree_species(def, dir, elev_m, r5);
                            let (sp_h, sp_jit) = match crate::renderer::tree_mesh::registry().get(sp_i)
                            {
                                Some(t) => (t.height_m, t.height_jitter),
                                None => (22.0, 0.12),
                            };
                            let jitter =
                                1.0 - sp_jit + (r3 % 100) as f32 / 100.0 * (sp_jit * 2.0);
                            let h = sp_h * jitter;
                out.push(NearTree {
                    dir,
                    r_m: r,
                    yaw,
                    height_m: h,
                    species: sp_i as u8,
                    variant: ((r5 >> 11) % 3) as u8,
                });
            }
        }
    }
    // Gate autopsy in the log whenever the harvest comes back empty-handed
    // but candidates existed - names the guilty gate with sample values.
    if out.is_empty() && n_total > 0 {
        log::info!(
            "[NearTree] gates: {} candidates, {} outside radius, {} elev-gated \
             (samples {:?} m), {} green-gated (samples {:?})",
            n_total,
            n_out,
            n_elev,
            elev_samples,
            n_green,
            green_samples
        );
    }
    // Nearest-first so the caller's draw cap keeps the trees beside the
    // camera, not whichever cell enumerated first.
    out.sort_by(|a, b| {
        b.dir
            .dot(center)
            .partial_cmp(&a.dir.dot(center))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

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
    /// while `grass_density_at(live distance) / GRASS_PEAK_PER_M2 > thr`,
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
        return GRASS_FAR_M;
    }
    let m = GRASS_MID_PER_M2 / GRASS_PEAK_PER_M2;
    if thr >= m {
        // On the peak-to-mid leg: density/PEAK falls 1 -> m over NEAR..MID.
        let t = (thr - 1.0) / (m - 1.0);
        GRASS_NEAR_M + (GRASS_MID_M - GRASS_NEAR_M) * t
    } else {
        // On the mid-to-zero leg: density/PEAK falls m -> 0 over MID..FAR.
        let t = 1.0 - thr / m;
        GRASS_MID_M + (GRASS_FAR_M - GRASS_MID_M) * t
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

/// Tillers per m^2 of ground at `d_m` surface metres from the camera, BEFORE
/// `veg_density()`. Piecewise linear: full density inside `GRASS_NEAR_M`,
/// down to `GRASS_MID_PER_M2` at `GRASS_MID_M`, then to zero at
/// `GRASS_FAR_M`.
///
/// Why a ramp at all: this is what makes the layer ringless. The bake gated
/// on patch DEPTH, so the field ended wherever the LOD selector happened to
/// stop refining - a hard edge that moved with the camera and lit up at
/// grazing sun (the v0.999 report). A density that reaches zero smoothly has
/// no edge to see.
#[inline]
pub fn grass_density_at(d_m: f32) -> f32 {
    if d_m <= GRASS_NEAR_M {
        GRASS_PEAK_PER_M2
    } else if d_m < GRASS_MID_M {
        let t = (d_m - GRASS_NEAR_M) / (GRASS_MID_M - GRASS_NEAR_M);
        GRASS_PEAK_PER_M2 + (GRASS_MID_PER_M2 - GRASS_PEAK_PER_M2) * t
    } else if d_m < GRASS_FAR_M {
        let t = (d_m - GRASS_MID_M) / (GRASS_FAR_M - GRASS_MID_M);
        GRASS_MID_PER_M2 * (1.0 - t)
    } else {
        0.0
    }
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

/// Enumerate grass tillers within `far_m` surface metres of `center_dir`.
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
/// `veg_density` 1.0, 47,000 tillers: 27 ms at Fuji depth 17, 31 ms at
/// Amazon depth 20 (roughly 16 and 18 at the shipped 0.6 density). That is
/// one doubled frame every few seconds of walking - real, and the honest
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
    let margin_m = margin_m.clamp(0.0, GRASS_FAR_M as f64);
    // The superset reaches `margin_m` further than the ramp does: a camera
    // that far from the harvest centre still sees the last of the ramp.
    let far_m = far_m.clamp(1.0, GRASS_FAR_M as f64) + margin_m;
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
    let density_scale = veg_density();
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
        // Items per cell at the THICKEST a clump can be: cos(lat)-thinned so
        // the per-area density is constant, and scaled by the Settings
        // vegetation slider. The GRASS_CLUMP_GAIN_MAX headroom is what lets a
        // clump genuinely exceed the nominal density instead of saturating at
        // it (a gain that can only ever thin would drag the mean below
        // GRASS_PEAK_PER_M2 and make that constant a lie).
        let area_m2 = cell_m * cell_m * cell_coslat;
        let count = ((GRASS_PEAK_PER_M2 * density_scale * GRASS_CLUMP_GAIN_MAX) as f64
            * area_m2)
            .round() as u32;
        if count == 0 {
            continue;
        }
        // Acceptance is index < count * p with p = want * gain / GAIN_MAX; p
        // can never exceed `want` at the cell's nearest point, so the stream
        // can stop there instead of running the full cell. The superset
        // margin (see the fn doc) enters here as a distance discount, which
        // is what makes the bound valid for any camera within it.
        let p_ceiling =
            (grass_density_at((near_m - margin_m).max(0.0) as f32) / GRASS_PEAK_PER_M2).min(1.0);
        let take = ((count as f32) * p_ceiling).ceil() as u32;
        if take == 0 {
            continue;
        }
        let mut s = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ salt;
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
            // 24 bits of position per axis, from the TOP of the word, not the
            // tree stream's `% 4096`. Two reasons, both measured: 4096 steps
            // across an 8 m cell is 2 mm, and with ~7,500 items in a cell the
            // birthday collision rate is ~1.7 duplicate positions PER CELL -
            // two tillers with different looks standing in exactly the same
            // spot, z-fighting. (It is harmless on the tree grid, where 480
            // items share a 220 m cell.) And xorshift's low bits are its
            // weakest; the high 24 are not.
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
            // no random was consumed to get here.
            let gain = grass_clump_gain(lat, lon);
            if gain <= 0.0 {
                continue; // bare scrape
            }
            // The tiller's THRESHOLD: the normalized density at which it
            // starts to exist, from its index in the cell's stream. Invert
            // the acceptance rule `item < count * (density/PEAK) * gain/GMAX`
            // to get a per-tiller constant that carries no camera distance at
            // all, so the draw-time gate can re-evaluate the ramp live.
            let thr = (item as f32 / count as f32) * (GRASS_CLUMP_GAIN_MAX / gain);
            // SUPERSET acceptance: discount the distance by the margin, so
            // any camera within it still finds this tiller in the set.
            let p_here = grass_density_at(((d_m as f64 - margin_m).max(0.0)) as f32)
                / GRASS_PEAK_PER_M2;
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
            if !veg_biome_ok(sc) {
                continue;
            }
            // STANDING POSITION: the DRAWN patch face, per surviving tiller.
            // Not a direct elevation sample - see the DrawnPatchSurface note.
            // Only survivors pay, and the vertex memo means neighbours in the
            // same lattice cell share their three corner samples.
            let r = ground.radius_at(dir) - GRASS_GROUND_BIAS_M;
            let hf = grass_height_field(lat, lon);
            let jitter = 0.82 + (r3 % 1000) as f32 / 1000.0 * 0.36;
            let height_m = ((GRASS_HEIGHT_MIN_M + GRASS_HEIGHT_MAX_M) * 0.5 * hf * jitter)
                .clamp(GRASS_HEIGHT_MIN_M, GRASS_HEIGHT_MAX_M);
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
            });
        }
    }
    out
}

/// Metadata the caller needs about the shared tiller mesh, returned beside it
/// so nothing has to re-derive it from the vertex buffer.
#[derive(Clone, Copy, Debug)]
pub struct GrassTillerStats {
    /// ONE-SIDED leaf area of the whole tiller at unit height, m^2. The mesh
    /// is emitted double-sided (the opaque pipeline back-culls and a blade
    /// must be visible from behind), so this is half the raw triangle area.
    /// Scale by height^2 for a real instance.
    pub one_sided_area_unit: f32,
    pub blades: usize,
    pub triangles: usize,
}

/// The ONE mesh every grass instance draws: a fan of `GRASS_BLADES_PER_TILLER`
/// arching, tapered blades rising from a single crown, built at UNIT height in
/// a canonical Y-up frame so an instance's uniform scale IS its height in
/// metres.
///
/// Shape, from the real plant rather than from convenience:
///   * A blade emerges near-vertical and ARCHES over. A straight blade reads
///     as wheat stubble or a bristle brush; the arch is the entire reason a
///     sward reads as a soft mass. The midrib follows a quadratic Bezier from
///     the crown to a tip that has fallen outward and down.
///   * It TAPERS to a point. The old card had a ruler-straight top edge, which
///     is what made the tufts photograph as pieces of pale tape.
///   * Blades fan out around the crown at irregular azimuths and lean out by
///     different amounts, so the tiller has no viewing angle where it
///     collapses into a line.
///
/// Shading, all of it free because the vertices exist anyway:
///   * PER-CORNER NORMALS blended half-way between the blade's own facing and
///     the crown's radial up. That gives a real lit side and a shaded side
///     across one tiller (the deleted card pinned every normal to the radial
///     up, so every tuft on the planet lit identically), while structurally
///     preventing N.L from reaching zero - which is what the v0.896 radial pin
///     was protecting against. It is the same 0.5 spherification the cluster
///     cards use.
///   * The LEAF ORGAN BIT on every face, so the shipped `is_leaf` transmission
///     path lights a backlit blade as thin tissue. Green is the band leaf
///     tissue transmits best; a sward with the sun behind it glows.
///   * A VERTICAL SHADE RAMP, dark at the crown and full at the tips. This is
///     Beer-Lambert, not decoration: at LAI 2-4 with grass's low extinction
///     the base of a sward sits at 20-30% of the top-of-canopy irradiance,
///     and a uniformly lit blade is a large part of what reads as a sticker.
///
/// THE MESH CARRIES NO COLOUR, and must not be given one. It is drawn ONCE,
/// instanced, for every tiller in the field; the moment its packed UV holds a
/// tint it stops being shareable and becomes one mesh per tiller, which is
/// the thing this design exists to avoid. So the packed channel carries the
/// GREY RAMP (0.30 at the crown to 1.00 at the tip) and the per-instance
/// albedo in `NearGrass::color` multiplies it in the fragment stage. That
/// multiply is part of the new material type's wiring - see the WIRING note
/// on `near_grass_instances`.
pub fn grass_tiller_mesh(
) -> (crate::renderer::plant_mesh::PlantMeshBuilder, GrassTillerStats) {
    use crate::renderer::plant_mesh::{Organ, PlantMeshBuilder};
    use glam::Vec3;
    let mut b = PlantMeshBuilder::new();
    b.set_organ(Organ::Leaf);
    // Cross-sections along a blade at t = 0, 1/3, 2/3, 1(tip). Widths are
    // fractions of unit height; see the GRASS_PEAK_PER_M2 note on why a drawn
    // blade is a BUNDLE and therefore centimetres wide, not millimetres.
    const SEG_W: [f32; 3] = [0.209, 0.115, 0.041];
    let up = Vec3::Y;
    let blades = GRASS_BLADES_PER_TILLER;
    let mut one_sided = 0.0f32;
    for k in 0..blades {
        // Irregular fan: the golden angle spreads azimuths without ever
        // repeating, and a small per-blade wobble keeps two neighbours from
        // looking like a mirrored pair.
        let kf = k as f32;
        let az = kf * 2.399_963_2 + (kf * 1.7).sin() * 0.35;
        let side = Vec3::new(az.cos(), 0.0, az.sin());
        // Arch: tip height and outward reach vary per blade so a tiller has a
        // ragged silhouette instead of a parasol.
        let vary = ((kf * 3.1).sin() * 0.5 + 0.5) * 0.35;
        let tip_h = 0.72 + vary * 0.55; // 0.72..1.07 of unit height
        let reach = 0.22 + vary * 0.75; // outward fall of the tip
        // Quadratic Bezier control point: high and close in, so the blade
        // leaves the crown near-vertical and only bends over near the tip.
        let p0 = Vec3::ZERO;
        let p1 = up * (tip_h * 0.78);
        let p2 = up * tip_h + side * reach;
        let at = |t: f32| -> Vec3 {
            let u = 1.0 - t;
            p0 * (u * u) + p1 * (2.0 * u * t) + p2 * (t * t)
        };
        // Blade-plane frame: `side` is the fall direction, so the blade's
        // WIDTH runs across it.
        let across = up.cross(side).normalize();
        let ts = [0.0f32, 1.0 / 3.0, 2.0 / 3.0, 1.0];
        let mids: Vec<Vec3> = ts.iter().map(|t| at(*t)).collect();
        // Per-corner normal: the blade's own facing (perpendicular to the
        // blade plane) blended HALF-WAY toward the crown's radial up. Capped
        // at 0.5 so N.L can never reach 0 - the black-slab guard.
        let facing = across.cross(mids[3] - mids[0]).normalize_or_zero();
        let mut facing = if facing.length_squared() < 0.5 { side } else { facing };
        // The blade's UPPER surface faces the sky, so orient the facing
        // upward before blending. Without this the cross product's sign flips
        // with the fan azimuth and half the blades ship an inverted normal
        // (measured: n.y = -0.55 on the first pass), which at grazing sun is
        // the black-slab defect the v0.896 radial pin was fighting.
        if facing.y < 0.0 {
            facing = -facing;
        }
        let nrm = (facing * 0.5 + up * 0.5).normalize();
        // Vertical Beer-Lambert ramp, packed as a GREY (see the no-colour
        // note above). Flat-interpolated, so it is per-FACE - one shade per
        // segment, which is exactly the resolution a 3-segment blade carries.
        let shade = |t: f32| -> [f32; 3] {
            let k = 0.30 + 0.70 * t; // crown 30% of tip irradiance
            [k, k, k]
        };
        for s in 0..3 {
            let (m0, m1) = (mids[s], mids[s + 1]);
            let w0 = SEG_W[s] * 0.5;
            let w1 = if s == 2 { 0.0 } else { SEG_W[s + 1] * 0.5 };
            let c = shade((ts[s] + ts[s + 1]) * 0.5);
            let a0 = m0 - across * w0;
            let a1 = m0 + across * w0;
            if s == 2 {
                // Pointed tip: one triangle, no top edge.
                b.tri_smooth(
                    [a0.to_array(), a1.to_array(), m1.to_array()],
                    [nrm.to_array(); 3],
                    c,
                );
                b.tri_smooth(
                    [a0.to_array(), m1.to_array(), a1.to_array()],
                    [(-nrm).to_array(); 3],
                    c,
                );
                one_sided += 0.5 * (a1 - a0).length() * (m1 - m0).length();
            } else {
                let b0 = m1 - across * w1;
                let b1 = m1 + across * w1;
                for (p, n) in [
                    ([a0, a1, b1], nrm),
                    ([a0, b1, b0], nrm),
                    ([a0, b1, a1], -nrm),
                    ([a0, b0, b1], -nrm),
                ] {
                    b.tri_smooth(
                        [p[0].to_array(), p[1].to_array(), p[2].to_array()],
                        [n.to_array(); 3],
                        c,
                    );
                }
                one_sided += 0.5 * ((a1 - a0).length() + (b1 - b0).length())
                    * (m1 - m0).length();
            }
        }
    }
    b.set_organ(Organ::Stem);
    let triangles = b.indices.len() / 3;
    (b, GrassTillerStats { one_sided_area_unit: one_sided, blades, triangles })
}

/// Conservative radial band for water-shell selection/culling: the sea
/// sphere plus the worst-case analytic wave height either way (the vertex
/// shader displaces within this envelope), plus skirt + slop.
pub fn water_band(radius_m: f64) -> RadialBand {
    // v0.1051: the FFT sea's crest now scales with wind (up to ~10 m), so this
    // CULLING bound must cover the worst case, not the trains' 3.1 m. It is a
    // conservative bound only - over-estimating costs a little less culling.
    let wave = crate::terrain::ocean_waves::MAX_SEA_HEIGHT_M as f64;
    RadialBand {
        min_r_m: radius_m - wave - SKIRT_MAX_M - 1.0,
        max_r_m: radius_m + wave + 1.0,
    }
}

/// Build one WATER-SHELL patch (v0.876 real-water Stage 1): the flat sea
/// sphere at exactly `def.radius`, only where the connected-ocean mask says
/// water. Returns None for all-land patches (the shell simply has no
/// geometry there -- the driver caches the miss so selection stops asking).
/// Faces are water-style (spherical normals); the type-16 material's vertex
/// stage displaces by the analytic wave height and its fragment stage draws
/// the Fresnel sky mirror + sun glitter, so the MESH stays the undisplaced
/// sphere -- the CPU physics twin (terrain::ocean_waves) adds the same
/// height analytically and drawn == sampled holds.
pub fn build_water_patch_mesh(
    def: &PlanetDef,
    ocean: &super::ocean_mask::OceanMask,
    hm: Option<&PlanetHeightmap>,
    id: &PatchId,
) -> Option<PatchMesh> {
    build_water_patch_mesh_at(def, ocean, hm, id, 0.0)
}

/// The two next-coarser-level parents of barycentric lattice vert (r, c),
/// or None for EVEN verts (they survive coarsening). Parents follow the
/// triangulation's three lattice axes (+row, +col, and the emit-order
/// (r,c)->(r+1,c+1) diagonal), so every odd vert lies on a real edge of
/// the coarse triangulation and its fully-morphed height equals that
/// edge's linear interpolation - the geomorph weld contract (v0.1041).
pub fn water_geomorph_parents(r: u32, c: u32) -> Option<((u32, u32), (u32, u32))> {
    match (r % 2, c % 2) {
        (1, 0) => Some(((r - 1, c), (r + 1, c))),
        (0, 1) => Some(((r, c - 1), (r, c + 1))),
        (1, 1) => Some(((r - 1, c - 1), (r + 1, c + 1))),
        _ => None,
    }
}

/// `lift_offset_m` lowers (negative) or raises the shell radius relative to
/// the standard SURFACE_LIFT sphere. The BACKSTOP shell (v0.1019, water arc:
/// "holes through the water along the seams") builds at
/// -(MAX_WAVE_HEIGHT + 0.5): a coarse, UNDISPLACED deep-water layer under
/// the wave shell, so any cross-depth T-junction tear in the displaced
/// surface reveals water-colored backstop instead of pale seafloor or sky -
/// the long swells (360-2000 m) sag up to ~1.2 m across coarse patch edges
/// and CANNOT be resolution-faded away (they are the visible sea).
pub fn build_water_patch_mesh_at(
    def: &PlanetDef,
    ocean: &super::ocean_mask::OceanMask,
    hm: Option<&PlanetHeightmap>,
    id: &PatchId,
    lift_offset_m: f64,
) -> Option<PatchMesh> {
    let n = PATCH_TESS;
    let corners = patch_corners(id);
    let radius_m =
        def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64 + lift_offset_m;
    let anchor = (corners[0] + corners[1] + corners[2]).normalize() * radius_m;

    // Same bit-identical border walk as the terrain builder (commutative
    // f64 midpoint math), so same-depth water neighbors share borders.
    let vert_count = ((n + 1) * (n + 2) / 2) as usize;
    let mut dirs: Vec<DVec3> = Vec::with_capacity(vert_count);
    let mut any_ocean = false;
    for r in 0..=n {
        for c in 0..=r {
            let w0 = (n - r) as f64;
            let w1 = (r - c) as f64;
            let w2 = c as f64;
            let dir = (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize();
            // DILATED coverage test (v0.1056): the mask's 5.56 km cells are
            // 12x coarser than the seabed actually drawn, so an undilated test
            // clipped the shell along mask-cell edges and left kilometre-wide
            // strips of drawn-underwater seabed bare. The per-vertex depth
            // feather trims the shell back to the real waterline, so being
            // generous here costs nothing but closes those strips.
            if ocean.is_ocean_near(dir.as_vec3()) {
                any_ocean = true;
            }
            dirs.push(dir);
        }
    }
    if !any_ocean {
        return None;
    }
    let offsets: Vec<glam::Vec3> = dirs
        .iter()
        .map(|d| ((*d * radius_m) - anchor).as_vec3())
        .collect();

    let grid_tris = (n * n) as usize;
    let skirt_tris = (3 * n * 2) as usize;
    let mut vertices: Vec<SurfaceVertexData> = Vec::with_capacity((grid_tris + skirt_tris) * 3);
    let mut indices: Vec<u32> = Vec::with_capacity((grid_tris + skirt_tris) * 3);
    // Per-vertex WATER DEPTH baked into the color transport (v0.917,
    // shoreline increment): the builder already knows the seafloor from
    // the heightmap, so the shader gets a smooth interpolated depth field
    // with zero runtime cost - no depth-texture pass needed. Encoding:
    // color r/g carry depth in decimetres as (hi, lo) bytes; the packed
    // UV then equals water_bit + depth_dm, and LINEAR interpolation of
    // that scalar across a triangle IS linear depth interpolation.
    // Without a heightmap every vertex reads 30 m (open-deep default).
    let depth_color = |dir: DVec3| -> [f32; 3] {
        // sample_meters is real elevation relative to sea level, so depth
        // below the surface is simply its negation.
        let depth_m = hm
            .map(|h| (-h.sample_meters(dir.as_vec3())).max(0.0))
            .unwrap_or(300.0);
        let dm = (depth_m * 10.0).clamp(0.0, 65535.0) as u32;
        [((dm >> 8) & 255) as f32 / 255.0, (dm & 255) as f32 / 255.0, 0.0]
    };
    // CELL SIZE baked into the free blue channel (v0.1049 - the far-field
    // facets). The wave fades are Nyquist gates written in DISTANCE: each
    // train dies at 60 * lambda because the vertex spacing there is assumed
    // to be ~dist/325 (screen-error LOD at split_px 4), which lands the
    // fade at cell ~ lambda/5.4. But the water shell's leaf budget SATURATES
    // (measured [WaterDiag] at 700 m: coarsest drawn leaf carries 38-60 px of
    // error against a 4 px target), so the real spacing out there is 10-15x
    // coarser than the fade assumes: the shader keeps displacing waves the
    // mesh cannot represent, and 16 verts spanning hundreds of metres draw a
    // wave field as big randomly-tilted facets. That is the operator's
    // "flat triangles... only the very furthest", and why ascending makes it
    // worse (a higher eye sees more sea, so the budget cuts coarser).
    // Measuring the spacing instead of assuming it makes the same gate
    // correct at any budget. color.b -> uv.y (pack_color_to_uv keeps it as a
    // plain float, and water always wrote 0 there).
    let cell_m = ((dirs[grid_idx(1, 0)] - dirs[grid_idx(0, 0)]).length() * radius_m) as f32;
    let cell_code = (cell_m / WATER_CELL_CODE_SCALE).min(1.0);
    let depth_colors: Vec<[f32; 3]> = dirs
        .iter()
        .map(|d| {
            let mut c = depth_color(*d);
            c[2] = cell_code;
            c
        })
        .collect();
    // Geomorph parent deltas (v0.1041, the WELD fix - operator: "let's
    // fix the welds on the water polygons"): every ODD-parity lattice
    // vert disappears at the next-coarser LOD, where the surviving edge
    // linearly interpolates its two PARENT verts. The half-offset to
    // those parents rides the NORMAL slot (the water FS derives its
    // normal from position, so the slot is free transport; even verts
    // carry zero). The vertex shader morphs displacement toward the
    // parents' mean as the camera recedes, reaching EXACTLY the coarser
    // neighbor's edge interpolation before that neighbor can exist -
    // spatially exact welds with no neighbor bookkeeping (CDLOD).
    // Parent directions follow the triangulation's three lattice axes
    // (+row, +col, and the (r,c)->(r+1,c+1) diagonal of emit order), so
    // every odd vert sits on a real coarse edge. Along shared borders
    // the parity and parents are intrinsic to the edge, so both sides
    // compute identical morphs (the border walk is bit-identical).
    let vpos = |r: u32, c: u32| -> DVec3 { dirs[grid_idx(r, c)] * radius_m };
    let mut deltas: Vec<glam::Vec3> = Vec::with_capacity(vert_count);
    for r in 0..=n {
        for c in 0..=r {
            deltas.push(match water_geomorph_parents(r, c) {
                Some(((r1, c1), (r2, c2))) => {
                    ((vpos(r1, c1) - vpos(r2, c2)) * 0.5).as_vec3()
                }
                None => glam::Vec3::ZERO,
            });
        }
    }
    let mut emit_face = |ia: usize, ib: usize, ic: usize,
                         vertices: &mut Vec<SurfaceVertexData>,
                         indices: &mut Vec<u32>| {
        for &i in &[ia, ib, ic] {
            indices.push(vertices.len() as u32);
            vertices.push(SurfaceVertexData {
                position: offsets[i].to_array(),
                normal: deltas[i].to_array(),
                color: depth_colors[i],
                water: true,
                tree_card: false,
            });
        }
    };
    for r in 0..n {
        for c in 0..=r {
            emit_face(
                grid_idx(r, c),
                grid_idx(r + 1, c),
                grid_idx(r + 1, c + 1),
                &mut vertices,
                &mut indices,
            );
        }
        for c in 0..r {
            emit_face(
                grid_idx(r, c),
                grid_idx(r + 1, c + 1),
                grid_idx(r, c + 1),
                &mut vertices,
                &mut indices,
            );
        }
    }

    // NO skirts on water (v0.878.2, operator: visible triangle seams across
    // the whole ocean). The shell draws in the TRANSPARENT pass (no depth
    // write), so a skirt wall behind the surface blend-stacks along every
    // patch border - each border became a darker seam line. Cracks are
    // covered differently here: the shader's vertex wave displacement fades
    // to ZERO with distance (see ocean_wave_height's fade), so far patches
    // of any two LODs lie on the exact same sphere (bit-matching borders),
    // and near-field neighbor depths sample the same smooth analytic field
    // densely enough that any residual T-junction gap is sub-wave-height
    // over moving water - invisible where a skirt line was glaring.

    Some(PatchMesh {
        mesh: SurfaceMeshData { vertices, indices },
        anchor,
        band: water_band(radius_m),
    })
}

// ── Per-planet runtime cache (engine side; holds renderer mesh handles as
// plain indices so this module stays GPU-free and testable) ──

pub struct PatchEntry {
    /// Index into Renderer::meshes. usize::MAX when the patch lives in the
    /// mega-buffer arena instead (slot below) -- the arena is the normal
    /// path since draw-batching increment 1; a classic mesh handle only
    /// appears when the arena was full at build time.
    pub mesh: usize,
    /// Arena ranges when this patch is batched (plain data - keeps this
    /// module GPU-free and testable; the engine owns the actual buffers).
    pub slot: Option<crate::renderer::patch_arena::PatchSlot>,
    /// GPU byte estimate for the LRU cap.
    pub bytes: usize,
    /// Patch anchor: planet-local unrotated frame, meters (f64). The draw
    /// site composes planet_render_pos + rotation * anchor in f64 and
    /// narrows at the end (the whole point of the anchor scheme).
    pub anchor: DVec3,
    /// Measured radial band of the built geometry (tight culling bounds).
    pub band: RadialBand,
    /// Frame stamp of last draw (LRU key).
    pub last_used: u64,
}

/// All chunked-LOD state for one planet.
pub struct ChunkState {
    pub cache: HashMap<PatchId, PatchEntry>,
    pub total_bytes: usize,
    pub detail: DetailNoise,
    /// Monotonic frame counter (advanced by the engine each frame this
    /// planet is chunk-active).
    pub frame: u64,
    /// Whether patches actually drew last frame (for transition logging).
    pub active_last_frame: bool,
    /// The leaf set DRAWN last frame (v0.898): the memory behind the
    /// split/merge hysteresis and the committed-split budget tier in
    /// select_patches_sticky. Keyed on what was actually on screen, not on
    /// residency - the v0.889 prefetch builds children everywhere, which
    /// silently turned the old residency-keyed hysteresis into a dense
    /// oscillation zone (the operator's "higher settings = worse flicker").
    pub last_drawn: std::collections::HashSet<PatchId>,
    /// Frame stamp of the last budget-saturation log (throttle).
    pub last_saturation_log: u64,
    /// LOD crossfades in flight (v0.920 geomorph fades): each split/merge
    /// dissolves over FADE_SECONDS instead of popping. Selection is
    /// untouched - this is pure presentation on top of the drawn-set diff.
    pub fades: Vec<FadePair>,
    /// Every patch id ever drawn this session (v0.995): re-entering the
    /// frustum after the camera looked away must POP, not fade up from
    /// nothing (the "ground vanishes as I look around" report). Bounded by
    /// the ids visited near a body; cleared with the cache on world swaps.
    pub ever_drawn: std::collections::HashSet<PatchId>,
    /// Parked-selection skip (v0.928): the last full selection + the local
    /// pose/params it was computed at. While the camera is parked in
    /// surface mode (planet-local pose static) and nothing invalidated it,
    /// the ~30k-node walk is skipped and this is reused.
    pub last_selection: Option<Selection>,
    pub last_sel_cam: DVec3,
    pub last_sel_fwd: DVec3,
    pub last_sel_split_px: f32,
    pub last_sel_budget: f32,
    /// Set by builds, evictions, tile arrivals - anything that changes what
    /// a fresh selection would decide.
    pub sel_dirty: bool,
}

/// One LOD crossfade in flight (v0.920): `rising` dissolves IN while
/// `falling` dissolves OUT with the complementary Bayer mask, sharing one
/// clock, so the two generations partition the screen per-pixel (no holes,
/// no double-write; see RenderObject::fade).
pub struct FadePair {
    pub rising: Vec<PatchId>,
    pub falling: Vec<PatchId>,
    /// 0..1, advanced by dt / FADE_SECONDS each frame; retired at 1.
    pub t: f32,
}

/// Crossfade duration. Short enough that fast dives never stack many
/// generations; long enough that a swap reads as a dissolve, not a pop.
pub const FADE_SECONDS: f32 = 0.30;
/// Overflow guard: past this many active pairs new swaps just pop (bounds
/// the extra falling-patch draw cost during a screaming descent).
pub const MAX_FADE_PAIRS: usize = 192;

/// Classify one frame's drawn-set diff into crossfade pairs (v0.920).
/// `appeared` / `vanished` are this frame's set differences against the
/// previous drawn set. A vanished PARENT whose children appeared = a split
/// (parent falls, children rise together). Vanished CHILDREN whose parent
/// appeared = a merge (parent rises, children fall together). Appeared
/// orphans (fresh stream-ins) rise from nothing as one batch; vanished
/// orphans (culled off-screen) pop instantly - fading something the frustum
/// already rejected would draw it for nothing.
pub fn classify_lod_swaps(
    appeared: &[PatchId],
    vanished: &[PatchId],
    seen_before: &dyn Fn(&PatchId) -> bool,
) -> Vec<FadePair> {
    use std::collections::HashSet;
    let vanished_set: HashSet<PatchId> = vanished.iter().cloned().collect();
    let appeared_set: HashSet<PatchId> = appeared.iter().cloned().collect();
    let mut used_appeared: HashSet<PatchId> = HashSet::new();
    let mut used_vanished: HashSet<PatchId> = HashSet::new();
    let mut pairs: Vec<FadePair> = Vec::new();
    // Splits: group appeared children under a vanished parent.
    let mut split_kids: HashMap<PatchId, Vec<PatchId>> = HashMap::new();
    for a in appeared {
        if let Some(p) = a.parent() {
            if vanished_set.contains(&p) {
                split_kids.entry(p).or_default().push(*a);
                used_appeared.insert(*a);
            }
        }
    }
    for (parent, kids) in split_kids {
        used_vanished.insert(parent);
        pairs.push(FadePair { rising: kids, falling: vec![parent], t: 0.0 });
    }
    // Merges: group vanished children under an appeared parent.
    let mut merge_kids: HashMap<PatchId, Vec<PatchId>> = HashMap::new();
    for v in vanished {
        if used_vanished.contains(v) {
            continue;
        }
        if let Some(p) = v.parent() {
            if appeared_set.contains(&p) && !used_appeared.contains(&p) {
                merge_kids.entry(p).or_default().push(*v);
            }
        }
    }
    for (parent, kids) in merge_kids {
        for k in &kids {
            used_vanished.insert(*k);
        }
        used_appeared.insert(parent);
        pairs.push(FadePair { rising: vec![parent], falling: kids, t: 0.0 });
    }
    // Orphan rises (fresh stream-ins): one shared-clock batch. v0.995
    // (operator: "as I look around the ground just vanishes"): a patch
    // RE-ENTERING the frustum after the camera looked away is NOT a fresh
    // stream-in - fading it up from nothing leaves a visible hole in ground
    // that was there a second ago. `seen_before` says whether an id has ever
    // been drawn this session; re-entries skip the rise and pop instantly,
    // exactly like culled-off-screen patches pop on the way OUT.
    let orphans: Vec<PatchId> = appeared
        .iter()
        .filter(|a| !used_appeared.contains(a) && !seen_before(a))
        .cloned()
        .collect();
    if !orphans.is_empty() {
        pairs.push(FadePair { rising: orphans, falling: Vec::new(), t: 0.0 });
    }
    pairs
}

impl ChunkState {
    pub fn new(terrain_seed: u64) -> Self {
        Self {
            cache: HashMap::new(),
            total_bytes: 0,
            detail: DetailNoise::new(terrain_seed),
            frame: 0,
            active_last_frame: false,
            last_drawn: std::collections::HashSet::new(),
            last_saturation_log: 0,
            fades: Vec::new(),
            ever_drawn: std::collections::HashSet::new(),
            last_selection: None,
            last_sel_cam: DVec3::ZERO,
            last_sel_fwd: DVec3::ZERO,
            last_sel_split_px: 0.0,
            last_sel_budget: 0.0,
            sel_dirty: false,
        }
    }

    /// Ingest one frame's drawn-set diff as new crossfade pairs (v0.920) and
    /// advance every active clock by `dt` seconds. Re-appearing patches are
    /// purged from falling lists (their area is covered by the normal draw
    /// again) and re-vanished patches from rising lists, so a hysteresis
    /// flip mid-fade can never double-mask an area.
    pub fn ingest_lod_swaps(&mut self, appeared: &[PatchId], vanished: &[PatchId], dt: f32) {
        for f in &mut self.fades {
            f.t += dt / FADE_SECONDS;
            if !appeared.is_empty() {
                f.falling.retain(|id| !appeared.contains(id));
            }
            if !vanished.is_empty() {
                f.rising.retain(|id| !vanished.contains(id));
            }
        }
        self.fades
            .retain(|f| f.t < 1.0 && (!f.rising.is_empty() || !f.falling.is_empty()));
        if (!appeared.is_empty() || !vanished.is_empty()) && self.fades.len() < MAX_FADE_PAIRS {
            let ever = &self.ever_drawn;
            self.fades
                .append(&mut classify_lod_swaps(appeared, vanished, &|id| ever.contains(id)));
        }
        // Record AFTER classification so a genuinely fresh patch still rises
        // this frame and only its future re-entries pop.
        for a in appeared {
            self.ever_drawn.insert(*a);
        }
    }

    /// Per-patch fade values for the draw pass: positive = rising (show
    /// where Bayer < t), negative = falling (show where Bayer >= t). Absent
    /// = drawn normally.
    pub fn fade_values(&self) -> HashMap<PatchId, f32> {
        let mut m = HashMap::new();
        for p in &self.fades {
            let t = p.t.clamp(0.0, 1.0);
            for id in &p.rising {
                m.insert(*id, t.max(1.0 / 32.0));
            }
            for id in &p.falling {
                m.insert(*id, -t.max(1.0 / 32.0));
            }
        }
        m
    }

    pub fn insert(&mut self, id: PatchId, mesh: usize, bytes: usize, anchor: DVec3, band: RadialBand) {
        self.insert_slotted(id, mesh, None, bytes, anchor, band);
    }

    /// Insert with optional arena ranges (draw-batching increment 1).
    /// `mesh` should be usize::MAX when `slot` is Some.
    pub fn insert_slotted(
        &mut self,
        id: PatchId,
        mesh: usize,
        slot: Option<crate::renderer::patch_arena::PatchSlot>,
        bytes: usize,
        anchor: DVec3,
        band: RadialBand,
    ) {
        if let Some(old) = self.cache.insert(
            id,
            PatchEntry { mesh, slot, bytes, anchor, band, last_used: self.frame },
        ) {
            // Should not happen (selection never requests a built patch),
            // but never leak the byte count if it does.
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
        }
        self.total_bytes += bytes;
    }

    /// Pop LRU entries until under the byte cap. Returns (id, mesh index,
    /// arena slot) triples so the engine can recycle renderer mesh slots
    /// AND return arena ranges to the free lists.
    /// Never evicts roots (depth 0: the permanent whole-planet fallback)
    /// or anything used this frame.
    pub fn collect_evictions(
        &mut self,
        byte_cap: usize,
    ) -> Vec<(PatchId, usize, Option<crate::renderer::patch_arena::PatchSlot>)> {
        let mut evicted = Vec::new();
        // Recency guard (v0.898): never evict anything used in the last ~2
        // seconds. When the working set genuinely exceeds the cap, evicting
        // the just-culled ring made every camera micro-turn a rebuild storm;
        // running temporarily over cap is strictly cheaper than thrash.
        let recent = self.frame.saturating_sub(120);
        // Ancestor-chain guard (v0.913, operator: "the ground I'm standing
        // on keeps shifting to lower LODs which causes a flicker"): only
        // DRAWN leaves refresh last_used, so the interior nodes of their
        // descent chains went recency-stale and were evicted at the cache
        // cap - restricted descent then stalled at a shallow ancestor for a
        // frame (probe caught a depth-6 leaf with a 6-million-pixel error
        // flashing at the camera). Everything on the path from a drawn leaf
        // up to its root is load-bearing; protect the whole chain.
        let mut protected: std::collections::HashSet<PatchId> = std::collections::HashSet::new();
        for id in &self.last_drawn {
            let mut cur = *id;
            loop {
                if !protected.insert(cur) {
                    break; // shared ancestor chain already walked
                }
                match cur.parent() {
                    Some(p) => cur = p,
                    None => break,
                }
            }
        }
        // Mid-crossfade guard (v0.920): a fading-out patch is still being
        // drawn for up to FADE_SECONDS after it left the drawn set (merge
        // children are NOT ancestors of any drawn leaf, so the chain guard
        // above does not cover them). Evicting one mid-dissolve would flash
        // a hole exactly where the eye is watching a transition.
        for pair in &self.fades {
            for id in pair.rising.iter().chain(pair.falling.iter()) {
                protected.insert(*id);
            }
        }
        // LINEAR eviction (v0.930, operator: "10+ second hang" leaving a
        // planet): the old loop re-scanned the whole cache to find each
        // victim - O(N) per eviction, O(N*M) for the deactivation shrink,
        // which at a 12k-patch budget meant ~10 SECONDS on one frame. One
        // pass + one sort, oldest-first, and a per-call cap so a huge
        // shrink spreads across frames instead of owning one.
        const MAX_EVICTIONS_PER_CALL: usize = 2048;
        let mut cands: Vec<(u64, PatchId, usize)> = self
            .cache
            .iter()
            .filter(|(id, e)| id.depth > 0 && e.last_used < recent && !protected.contains(id))
            .map(|(id, e)| (e.last_used, *id, e.bytes))
            .collect();
        cands.sort_unstable();
        let mut freed = 0usize;
        for (_, id, bytes) in cands {
            if self.total_bytes.saturating_sub(freed) <= byte_cap
                || evicted.len() >= MAX_EVICTIONS_PER_CALL
            {
                break;
            }
            if let Some(e) = self.cache.remove(&id) {
                freed += bytes;
                evicted.push((id, e.mesh, e.slot));
            }
        }
        self.total_bytes = self.total_bytes.saturating_sub(freed);
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── emit_shared_grid_faces (draw-batching increment 3) ──

    /// Grid-index-encoding vertex factory: position.x = grid index,
    /// position.y = flavor, so tests can map emitted vertices back.
    fn test_vertex(gi: usize, water: bool) -> SurfaceVertexData {
        SurfaceVertexData {
            position: [gi as f32, if water { 1.0 } else { 0.0 }, 0.0],
            normal: [0.0, 1.0, 0.0],
            color: [0.0; 3],
            water,
            tree_card: false,
        }
    }

    /// The full 16-tess triangular grid's face list with a color function.
    fn full_grid_faces(color_of: impl Fn(usize) -> [f32; 3]) -> Vec<SharedGridFace> {
        let n = PATCH_TESS;
        let mut faces = Vec::new();
        for r in 0..n {
            for c in 0..=r {
                faces.push((grid_idx(r, c), grid_idx(r + 1, c), grid_idx(r + 1, c + 1)));
            }
            for c in 0..r {
                faces.push((grid_idx(r, c), grid_idx(r + 1, c + 1), grid_idx(r, c + 1)));
            }
        }
        faces
            .into_iter()
            .enumerate()
            .map(|(i, (a, b, c))| SharedGridFace { a, b, c, color: color_of(i), water: false })
            .collect()
    }

    #[test]
    fn shared_grid_provoking_carries_each_faces_pack_and_preserves_winding() {
        // Worst case for sharing: every face a distinct color.
        let faces = full_grid_faces(|i| [i as f32, (i * 7) as f32, (i * 13) as f32]);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        emit_shared_grid_faces(&faces, test_vertex, &mut vertices, &mut indices);
        assert_eq!(indices.len(), faces.len() * 3);
        for (i, f) in faces.iter().enumerate() {
            let tri: Vec<usize> =
                indices[i * 3..i * 3 + 3].iter().map(|&v| v as usize).collect();
            // Provoking vertex carries THIS face's pack.
            assert_eq!(
                vertices[tri[0]].color, f.color,
                "face {i}: provoking vertex carries a foreign pack"
            );
            assert!(!vertices[tri[0]].water);
            // The emitted triple is a winding-preserving ROTATION of
            // (a,b,c), mapped through the grid encoding in position.x.
            let g: Vec<usize> =
                tri.iter().map(|&vi| vertices[vi].position[0] as usize).collect();
            let orig = [f.a, f.b, f.c];
            let ok = (0..3).any(|r| {
                g[0] == orig[r] && g[1] == orig[(r + 1) % 3] && g[2] == orig[(r + 2) % 3]
            });
            assert!(ok, "face {i}: {g:?} is not a rotation of {orig:?}");
        }
        // Sharing bound: 153 unique points + at most one duplicate per
        // face; the greedy 3-rotation claim keeps it far below the old
        // 768-vertex layout even with every color distinct.
        let points = ((PATCH_TESS + 1) * (PATCH_TESS + 2) / 2) as usize;
        assert!(vertices.len() >= faces.len().min(points));
        assert!(
            vertices.len() <= points + faces.len(),
            "vertex count {} above the hard bound",
            vertices.len()
        );
        assert!(
            vertices.len() < 480,
            "distinct-color sharing too weak: {} verts (old layout 768)",
            vertices.len()
        );
    }

    #[test]
    fn shared_grid_dedups_fully_when_colors_repeat() {
        // One color everywhere: every face can claim any corner, so the
        // vertex array must collapse to exactly the unique grid points.
        let faces = full_grid_faces(|_| [0.25, 0.5, 0.75]);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        emit_shared_grid_faces(&faces, test_vertex, &mut vertices, &mut indices);
        let points = ((PATCH_TESS + 1) * (PATCH_TESS + 2) / 2) as usize;
        assert_eq!(vertices.len(), points, "uniform color must dedup fully");
        assert!(vertices.iter().all(|v| v.color == [0.25, 0.5, 0.75]));
    }

    #[test]
    fn shared_grid_flavors_coastline_vertices_per_face_kind() {
        // Two faces sharing an edge, one water one land: the shared grid
        // points must exist in BOTH flavors, and every face must reference
        // only vertices of its own flavor (they carry different normals).
        let faces = [
            SharedGridFace { a: 0, b: 1, c: 2, color: [0.1, 0.2, 0.3], water: false },
            SharedGridFace { a: 0, b: 2, c: 1, color: [0.4, 0.5, 0.6], water: true },
        ];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        emit_shared_grid_faces(&faces, test_vertex, &mut vertices, &mut indices);
        for (i, f) in faces.iter().enumerate() {
            for k in 0..3 {
                let v = &vertices[indices[i * 3 + k] as usize];
                assert_eq!(v.water, f.water, "face {i} corner {k} wrong flavor");
                assert_eq!(v.position[1], if f.water { 1.0 } else { 0.0 });
            }
        }
        // 3 land + 3 water copies, no cross-flavor sharing.
        assert_eq!(vertices.len(), 6);
    }

    #[test]
    fn shared_grid_duplicates_when_all_corners_are_claimed() {
        // A fan of 4 faces around vertex 0 where every face also touches
        // vertices claimed early: forces at least one duplicate, which
        // must still carry the right pack + winding.
        let faces = [
            SharedGridFace { a: 0, b: 1, c: 2, color: [1.0, 0.0, 0.0], water: false },
            SharedGridFace { a: 1, b: 3, c: 2, color: [0.0, 1.0, 0.0], water: false },
            SharedGridFace { a: 2, b: 3, c: 0, color: [0.0, 0.0, 1.0], water: false },
            // All of 0,1,2,3 are now claimed: this face must duplicate.
            SharedGridFace { a: 0, b: 2, c: 1, color: [1.0, 1.0, 0.0], water: false },
        ];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        emit_shared_grid_faces(&faces, test_vertex, &mut vertices, &mut indices);
        assert_eq!(vertices.len(), 5, "expected exactly one duplicate");
        for (i, f) in faces.iter().enumerate() {
            let tri: Vec<usize> =
                indices[i * 3..i * 3 + 3].iter().map(|&v| v as usize).collect();
            assert_eq!(vertices[tri[0]].color, f.color, "face {i} pack wrong");
            let g: Vec<usize> =
                tri.iter().map(|&vi| vertices[vi].position[0] as usize).collect();
            let orig = [f.a, f.b, f.c];
            let ok = (0..3).any(|r| {
                g[0] == orig[r] && g[1] == orig[(r + 1) % 3] && g[2] == orig[(r + 2) % 3]
            });
            assert!(ok, "face {i}: {g:?} not a rotation of {orig:?}");
        }
    }

    /// Earth-like water world def with a heightmap-loader-style sea level.
    fn earth_like() -> PlanetDef {
        let mut def: PlanetDef = ron::from_str(
            r#"(
                name: "ChunkTest",
                radius: 6371000.0,
                gravity: 9.81,
                terrain_seed: 42,
                ore_seed: 1,
                has_water: true,
                sea_level: 0.6286,
                surface_relief: 0.011,
            )"#,
        )
        .expect("test def parses");
        def.polar_cap_latitude = 0.88;
        def
    }

    /// Synthetic heightmap through the public byte format: a lat/lon ramp
    /// with real mountains so displacement is nonuniform.
    fn synth_heightmap(width: u32, height: u32, min_m: f32, max_m: f32, f: impl Fn(u32, u32) -> f32) -> PlanetHeightmap {
        use crate::terrain::planet_heightmap::{quantize_meters, HEIGHTMAP_MAGIC};
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&min_m.to_le_bytes());
        bytes.extend_from_slice(&max_m.to_le_bytes());
        for y in 0..height {
            for x in 0..width {
                bytes.extend_from_slice(&quantize_meters(f(x, y), min_m, max_m).to_le_bytes());
            }
        }
        PlanetHeightmap::from_bytes(&bytes).expect("synthetic heightmap parses")
    }

    fn bumpy_earth() -> PlanetHeightmap {
        // -11000..6500 like the shipped Earth window; a smooth sinusoidal
        // continent field with everything from deep ocean to high peaks.
        synth_heightmap(64, 32, -11000.0, 6500.0, |x, y| {
            let fx = x as f32 / 64.0 * std::f32::consts::TAU;
            let fy = y as f32 / 32.0 * std::f32::consts::PI;
            -2000.0 + 6000.0 * (fx * 3.0).sin() * (fy * 2.0).sin()
        })
    }

    fn band_for(def: &PlanetDef) -> RadialBand {
        RadialBand {
            min_r_m: def.radius * displaced_radius_f64(def, 0.0),
            max_r_m: def.radius * displaced_radius_f64(def, 1.0),
        }
    }

    /// The measured band a built near-sea-level patch would report: what
    /// the all-built test closures hand back so culling runs with the
    /// tight bounds it has in the real steady state.
    fn tight_band(def: &PlanetDef) -> RadialBand {
        RadialBand {
            min_r_m: def.radius - 200.0,
            max_r_m: def.radius + 200.0,
        }
    }

    fn params_for(def: &PlanetDef) -> ChunkParams {
        ChunkParams {
            occluder_r_m: None,
            radius_m: def.radius,
            band: band_for(def),
            max_depth: MAX_PATCH_DEPTH,
            split_px: CHUNK_SPLIT_PX,
            // 1080 px viewport at 60 deg vertical fov.
            px_per_rad: 1080.0 / 60f32.to_radians(),
            max_leaves: MAX_CHUNK_LEAVES,
            max_build_requests: MAX_BUILD_REQUESTS,
        }
    }

    #[test]
    fn depth_cap_math_lands_in_target_band() {
        // The header math, verified against the ACTUAL derived geometry:
        // walk to a depth-13 patch and measure its corner-to-corner arc.
        let r = 6_371_000.0_f64;
        let edge = patch_edge_arc_m(MAX_PATCH_DEPTH, r);
        let spacing = vertex_spacing_m(MAX_PATCH_DEPTH, r);
        assert!((edge - 861.0).abs() < 1.0, "patch edge at cap: {edge}");
        assert!(
            (50.0..=100.0).contains(&spacing),
            "vertex spacing at cap must land 50-100 m, got {spacing}"
        );
        // Measured arc of a real depth-13 patch edge agrees with the
        // formula within the slight nonuniformity of spherical bisection.
        let mut id = PatchId::root(0);
        for _ in 0..MAX_PATCH_DEPTH {
            id = id.child(3); // center children stay mid-face
        }
        let c = patch_corners(&id);
        let measured = c[0].dot(c[1]).clamp(-1.0, 1.0).acos() * r;
        // Spherical midpoint bisection is NONUNIFORM: patches near a root
        // face's center run up to ~20% larger than the formula and corner
        // patches somewhat smaller, so actual triangle edges at the cap
        // spread roughly 45-65 m around the 54 m nominal. The formula is
        // what the split metric uses (uniformly), which is fine for LOD.
        assert!(
            (measured - edge).abs() / edge < 0.25,
            "measured {measured} vs formula {edge}"
        );
    }

    #[test]
    fn child_corners_partition_parent() {
        let id = PatchId::root(7);
        let c = patch_corners(&id);
        let kids = child_corners(&c);
        // Corner children keep their parent corner; the center child's
        // corners are exactly the three edge midpoints.
        assert_eq!(kids[0][0], c[0]);
        assert_eq!(kids[1][0], c[1]);
        assert_eq!(kids[2][0], c[2]);
        let m01 = midpoint(c[0], c[1]);
        let m12 = midpoint(c[1], c[2]);
        let m20 = midpoint(c[2], c[0]);
        assert_eq!(kids[3], [m01, m12, m20]);
        // And patch_corners agrees with child_corners derivation.
        for i in 0..4u32 {
            assert_eq!(patch_corners(&id.child(i)), kids[i as usize]);
        }
    }

    #[test]
    fn ancestor_relation_via_path_prefix() {
        let root = PatchId::root(4);
        let a = root.child(2);
        let b = a.child(1).child(3);
        assert!(root.is_ancestor_of(&a));
        assert!(root.is_ancestor_of(&b));
        assert!(a.is_ancestor_of(&b));
        assert!(!b.is_ancestor_of(&a));
        assert!(!a.is_ancestor_of(&a));
        // Different sibling subtree is NOT an ancestor.
        assert!(!root.child(0).is_ancestor_of(&b));
        // Parent round-trips.
        assert_eq!(b.parent().unwrap().parent().unwrap(), a);
    }

    #[test]
    fn horizon_cull_behind_planet_culled_limb_kept() {
        let def = earth_like();
        let band = band_for(&def);
        let r = def.radius;
        // Camera at 2R on +X.
        let cam = DVec3::new(2.0 * r, 0.0, 0.0);
        // A small deep patch centered near -X (the far side).
        let far_side = {
            // Find a root face whose center points most toward -X, then
            // descend center children to shrink it.
            let mut best = PatchId::root(0);
            let mut best_dot = f64::MAX;
            for f in 0..20u8 {
                let c = patch_corners(&PatchId::root(f));
                let dir = (c[0] + c[1] + c[2]).normalize();
                if dir.x < best_dot {
                    best_dot = dir.x;
                    best = PatchId::root(f);
                }
            }
            let mut id = best;
            for _ in 0..6 {
                id = id.child(3);
            }
            id
        };
        let fb = patch_bounds(&patch_corners(&far_side), r, &band);
        assert!(
            horizon_culled(&fb, cam, band.min_r_m),
            "far-side patch must be horizon-culled"
        );
        // The sub-camera patch is kept.
        let near_side = {
            let mut best = PatchId::root(0);
            let mut best_dot = f64::MIN;
            for f in 0..20u8 {
                let c = patch_corners(&PatchId::root(f));
                let dir = (c[0] + c[1] + c[2]).normalize();
                if dir.x > best_dot {
                    best_dot = dir.x;
                    best = PatchId::root(f);
                }
            }
            let mut id = best;
            for _ in 0..6 {
                id = id.child(3);
            }
            id
        };
        let nb = patch_bounds(&patch_corners(&near_side), r, &band);
        assert!(!horizon_culled(&nb, cam, band.min_r_m), "sub-camera patch kept");
        // A LIMB patch (~90 deg off-axis, i.e. right at the visible edge
        // from 2R where the horizon sits at 60 deg + lift): build one at
        // ~62 deg, inside the horizon -> kept.
        let deg62 = DVec3::new(62f64.to_radians().cos(), 62f64.to_radians().sin(), 0.0);
        let mut limb = PatchId::root(0);
        let mut best = f64::MIN;
        for f in 0..20u8 {
            let c = patch_corners(&PatchId::root(f));
            let dir = (c[0] + c[1] + c[2]).normalize();
            if dir.dot(deg62) > best {
                best = dir.dot(deg62);
                limb = PatchId::root(f);
            }
        }
        // Descend toward the 62-degree direction to shrink the patch there.
        let mut id = limb;
        for _ in 0..6 {
            let c = patch_corners(&id);
            let kids = child_corners(&c);
            let mut pick = 0u32;
            let mut pb = f64::MIN;
            for (i, kc) in kids.iter().enumerate() {
                let d = (kc[0] + kc[1] + kc[2]).normalize().dot(deg62);
                if d > pb {
                    pb = d;
                    pick = i as u32;
                }
            }
            id = id.child(pick);
        }
        let lb = patch_bounds(&patch_corners(&id), r, &band);
        assert!(!horizon_culled(&lb, cam, band.min_r_m), "limb patch inside horizon kept");
        // And the whole-selection view: with everything built, no drawn
        // patch is on the far side, and horizon culling did real work.
        let tight = tight_band(&def);
        let sel = select_patches(cam, None, &|_| Some(tight), &params_for(&def));
        assert!(sel.stats.horizon_culled > 0, "horizon cull must trigger");
        for d in &sel.draws {
            let b = patch_bounds(&patch_corners(d), r, &tight);
            assert!(
                !horizon_culled(&b, cam, band.min_r_m),
                "selection drew a horizon-culled patch {d:?}"
            );
        }
    }

    #[test]
    fn selection_refines_near_camera_and_respects_cap() {
        let def = earth_like();
        let params = params_for(&def);
        let tight = tight_band(&def);
        // 2 km above the surface, everything pre-built (with the tight
        // measured bands built patches report in steady state).
        let cam = DVec3::new(def.radius + 2_000.0, 0.0, 0.0);
        let sel = select_patches(cam, None, &|_| Some(tight), &params);
        assert!(sel.fully_covered);
        assert!(!sel.draws.is_empty());
        let max_d = sel.draws.iter().map(|d| d.depth).max().unwrap();
        let min_d = sel.draws.iter().map(|d| d.depth).min().unwrap();
        assert_eq!(max_d, MAX_PATCH_DEPTH, "sub-camera must reach the cap");
        assert!(min_d < MAX_PATCH_DEPTH, "limb must stay coarser than the cap");
        assert!(sel.draws.len() <= params.max_leaves);
        // Deep leaves must be NEAR the camera, shallow leaves far.
        let cam_dir = cam.normalize();
        for d in &sel.draws {
            let c = patch_corners(d);
            let dir = (c[0] + c[1] + c[2]).normalize();
            if d.depth == MAX_PATCH_DEPTH {
                assert!(
                    dir.dot(cam_dir) > 0.99,
                    "cap-depth patch far from sub-camera point"
                );
            }
        }
        // From very far away nothing needs splitting: coarse roots only.
        let far = select_patches(
            DVec3::new(def.radius * 1e6, 0.0, 0.0),
            None,
            &|_| Some(tight),
            &params,
        );
        assert!(far.draws.iter().all(|d| d.depth == 0), "distant camera stays at roots");
    }

    #[test]
    fn tile_tier_descends_to_the_1m_cap() {
        // The v0.875 1 m ladder: with the tile-tier depth cap (20), a camera
        // ~15 m above the surface must refine all the way down to depth-20
        // patches (~0.42 m triangles). This exercises PatchId.path as u64 --
        // depth 17+ paths need more than 32 bits, so this test FAILS if the
        // path field ever regresses to u32 (silent child-id collisions).
        let def = earth_like();
        let mut params = params_for(&def);
        params.max_depth = TILE_MAX_PATCH_DEPTH;
        let cam = DVec3::new(def.radius + 15.0, 0.0, 0.0);
        // Steady-state MEASURED bands: built patches report the real radial
        // extent of their own geometry, which for this flat synthetic world
        // is a few meters -- NOT the coarse tight_band(+-200 m) other tests
        // use. The distinction is load-bearing here: with a +-200 m band
        // every patch within 200 m of the camera hits screen_error_px's 1 m
        // distance floor, ties at max priority, and the leaf budget
        // saturates before the deepest chain finishes (found the hard way).
        let measured = RadialBand {
            min_r_m: def.radius - 2.0,
            max_r_m: def.radius + 2.0,
        };
        let sel = select_patches(cam, None, &|_| Some(measured), &params);
        assert!(sel.fully_covered);
        let max_d = sel.draws.iter().map(|d| d.depth).max().unwrap();
        assert_eq!(
            max_d, TILE_MAX_PATCH_DEPTH,
            "walking-height camera must reach the 1 m cap; stats={:?} leaves={}",
            sel.stats,
            sel.draws.len()
        );
        // Deep leaves hug the sub-camera point; the limb stays coarse.
        let cam_dir = cam.normalize();
        for d in &sel.draws {
            if d.depth >= 18 {
                let c = patch_corners(d);
                let dir = (c[0] + c[1] + c[2]).normalize();
                assert!(
                    dir.dot(cam_dir) > 0.999,
                    "deep patch far from the sub-camera point: {d:?}"
                );
            }
        }
    }

    fn synth_mask(all_ocean: bool) -> crate::terrain::ocean_mask::OceanMask {
        // 8x4 grid through the public byte format (HOSOCM1 + dims + bits).
        let (w, h) = (8u32, 4u32);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(crate::terrain::ocean_mask::OCEAN_MASK_MAGIC);
        bytes.extend_from_slice(&w.to_le_bytes());
        bytes.extend_from_slice(&h.to_le_bytes());
        let fill = if all_ocean { 0xFFu8 } else { 0x00u8 };
        bytes.extend(std::iter::repeat(fill).take(((w * h + 7) / 8) as usize));
        crate::terrain::ocean_mask::OceanMask::from_bytes(&bytes).expect("synthetic mask")
    }

    #[test]
    fn water_patch_covers_ocean_and_skips_land() {
        let def = earth_like();
        let id = PatchId::root(3).child(2).child(1);
        // All-ocean mask: a real mesh at the exact sea radius.
        let pm = build_water_patch_mesh(&def, &synth_mask(true), None, &id)
            .expect("ocean patch builds");
        assert!(!pm.mesh.vertices.is_empty());
        // Every grid vertex sits ON the LIFTED sea sphere (v0.882: the
        // surface floats SURFACE_LIFT_M above nominal sea level to stop
        // beach-line z-shimmer; skirt verts sit below).
        let sea_r = def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64;
        let mut on_sphere = 0usize;
        for v in &pm.mesh.vertices {
            let p = pm.anchor + DVec3::new(v.position[0] as f64, v.position[1] as f64, v.position[2] as f64);
            let r = p.length();
            assert!(
                r <= sea_r + 1.0,
                "water vertex above the lifted sea sphere: {r}"
            );
            if (r - sea_r).abs() < 0.5 {
                on_sphere += 1;
            }
            assert!(v.water, "every water-shell vertex carries the water flag");
        }
        assert!(on_sphere > 0, "no vertex on the sea sphere");
        // The declared band contains the analytic wave envelope.
        let wave = crate::terrain::ocean_waves::MAX_WAVE_HEIGHT_M as f64;
        assert!(pm.band.max_r_m >= def.radius + wave);
        assert!(pm.band.min_r_m <= def.radius - wave);
        // All-land mask: no geometry at all.
        assert!(
            build_water_patch_mesh(&def, &synth_mask(false), None, &id).is_none(),
            "all-land patch must not build water"
        );
    }

    /// v0.1043 THE SEAM ITSELF, measured: a fine patch's edge vertex sits
    /// ON the sea sphere, but the coarser neighbor draws that span as a
    /// straight triangle EDGE (a chord) whose midpoint lies inside the
    /// sphere. That difference IS the operator's dusk seam ("the vertices
    /// seem welded but I keep seeing gaps along the edges"). This test
    /// asserts (a) the crack equals the chord sagitta |delta|^2/(2r), and
    /// (b) the shader's morph - drop the vert radially by exactly that -
    /// closes it to under 1% of its size. The shader lockstep test
    /// (ocean_fft) proves the shader really contains this formula.
    #[test]
    fn geomorph_chord_sag_is_the_seam_and_the_morph_closes_it() {
        let def = earth_like();
        let radius = def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64;
        let n = PATCH_TESS;
        // Sample several depths: the crack grows with the SQUARE of tile
        // size, which is why the operator fingered "the slightly larger
        // tiles". Depth 6 is a coarse far patch; 14 is near-field.
        for depth_extra in [8u32, 10, 12, 14] {
            let mut id = PatchId::root(3);
            for _ in 0..depth_extra {
                id = id.child(1);
            }
            let corners = patch_corners(&id);
            let vpos = |r: u32, c: u32| -> DVec3 {
                let (w0, w1, w2) = ((n - r) as f64, (r - c) as f64, c as f64);
                (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize() * radius
            };
            let (mut worst_before, mut worst_after, mut worst_sag) = (0.0f64, 0.0f64, 0.0f64);
            for r in 0..=n {
                for c in 0..=r {
                    let Some(((r1, c1), (r2, c2))) = water_geomorph_parents(r, c) else {
                        continue;
                    };
                    let (p1, p2, v) = (vpos(r1, c1), vpos(r2, c2), vpos(r, c));
                    let delta = (p1 - p2) * 0.5;
                    let mid = (p1 + p2) * 0.5;
                    // The shader's chord-sag term, verbatim.
                    let sag = delta.length() * delta.length() / (2.0 * radius);
                    let morphed = v - v.normalize() * sag;
                    worst_before = worst_before.max((v - mid).length());
                    worst_after = worst_after.max((morphed - mid).length());
                    worst_sag = worst_sag.max(sag);
                }
            }
            // (a) The unmorphed crack IS the sagitta (within 2%).
            assert!(
                (worst_before - worst_sag).abs() < worst_sag * 0.02,
                "depth+{depth_extra}: crack {worst_before:.4} m should equal sagitta {worst_sag:.4} m"
            );
            // (b) The morph closes it to under 1% of the crack.
            assert!(
                worst_after < worst_sag * 0.01,
                "depth+{depth_extra}: morph left {worst_after:.6} m of a {worst_sag:.4} m crack"
            );
            println!(
                "depth+{depth_extra}: cell {:.1} m, crack {:.4} m -> {:.6} m after morph",
                (vpos(1, 0) - vpos(0, 0)).length(),
                worst_before,
                worst_after
            );
        }
    }

    /// v0.1041 geomorph weld: the parent map obeys the parity contract,
    /// and built water verts carry parent half-offsets in the normal slot
    /// (zero on even verts, ~one lattice step on odd verts).
    #[test]
    fn water_geomorph_deltas_follow_the_parity_contract() {
        // Parity map: even verts have no parents; odd verts' parents are
        // 2 lattice steps apart along the axis of oddness, all in-bounds.
        for r in 0..=PATCH_TESS {
            for c in 0..=r {
                match water_geomorph_parents(r, c) {
                    None => assert!(r % 2 == 0 && c % 2 == 0, "({r},{c}) parity"),
                    Some(((r1, c1), (r2, c2))) => {
                        assert!(r % 2 == 1 || c % 2 == 1, "({r},{c}) should be even");
                        // Parents are even-parity, straddle the vert, and
                        // stay inside the triangular lattice.
                        for (pr, pc) in [(r1, c1), (r2, c2)] {
                            assert!(pr % 2 == 0 && pc % 2 == 0, "parent parity");
                            assert!(pc <= pr && pr <= PATCH_TESS, "parent bounds ({pr},{pc})");
                        }
                        assert_eq!(r1 + r2, 2 * r, "row midpoint");
                        assert_eq!(c1 + c2, 2 * c, "col midpoint");
                    }
                }
            }
        }
        // Built mesh: deltas ride the normal slot; even verts zero, odd
        // verts about one lattice step long (the patch cell size).
        let def = earth_like();
        let id = PatchId::root(3).child(2).child(1);
        let pm = build_water_patch_mesh(&def, &synth_mask(true), None, &id)
            .expect("ocean patch builds");
        let (mut zeros, mut steps) = (0usize, 0usize);
        let mut step_len = 0.0f32;
        for v in &pm.mesh.vertices {
            let d = glam::Vec3::from_array(v.normal).length();
            if d < 1.0e-3 {
                zeros += 1;
            } else {
                steps += 1;
                step_len = step_len.max(d);
            }
        }
        assert!(zeros > 0, "no even verts");
        assert!(steps > 0, "no odd verts carrying deltas");
        // The lattice step at depth 5-ish is hundreds of metres; sanity
        // band rather than exact (spherical cells vary slightly).
        for v in &pm.mesh.vertices {
            let d = glam::Vec3::from_array(v.normal).length();
            assert!(
                d < 1.0e-3 || (d > step_len * 0.4 && d <= step_len * 1.001),
                "delta {d} outside the lattice-step band (max {step_len})"
            );
        }
    }

    #[test]
    fn vegetation_bakes_into_deep_land_patches_deterministically() {
        // v0.888: a tree-depth land patch gains extra card triangles beyond
        // the 352-tri grid+skirt baseline, twice-built output is identical
        // (deterministic scatter), and vegetation never sprouts below depth
        // TREE_MIN_DEPTH or underwater.
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        // Walk to a LAND spot at tree depth: probe candidate ids until one
        // has mid-band elevation in the tree window.
        let mut found = None;
        'outer: for f in 0..20u8 {
            let mut id = PatchId::root(f);
            for _ in 0..TREE_MIN_DEPTH {
                id = id.child(3);
            }
            let c = patch_corners(&id);
            let dir = ((c[0] + c[1] + c[2]) / 3.0).normalize();
            let e = hm.normalized_at(dir.as_vec3());
            let elev = (e - def.sea_level) * (hm.max_meters() - hm.min_meters());
            if elev > 50.0 && elev < 1500.0 {
                found = Some(id);
                break 'outer;
            }
        }
        let id = found.expect("some root chain lands on tree-band terrain");
        let a1 = build_patch_mesh(&def, &src, None, &id);
        let a2 = build_patch_mesh(&def, &src, None, &id);
        assert_eq!(a1.mesh.vertices.len(), a2.mesh.vertices.len(), "non-deterministic");
        let baseline = (PATCH_TESS * PATCH_TESS + 3 * PATCH_TESS * 2) as usize * 3;
        assert!(
            a1.mesh.vertices.len() > baseline,
            "no vegetation baked: {} <= {}",
            a1.mesh.vertices.len(),
            baseline
        );
        // Shallow patch: no vegetation.
        let shallow = PatchId::root(id.face).child(3).child(3);
        let s1 = build_patch_mesh(&def, &src, None, &shallow);
        assert!(s1.mesh.vertices.len() <= baseline, "vegetation sprouted at depth 2");
    }

    /// ITEM 5: cards are INDEXED. Four distinct corners carry four triangles
    /// (two quads, the second pair reverse-wound for two-sidedness), so a card
    /// costs 4 vertices + 12 indices where it used to cost 12 + 12. Cards were
    /// 86% of the 1.2 GB vertex arena at a forest vantage, and an overflowing
    /// arena is what pushes tree-bearing patches onto the classic per-draw
    /// path (measured: 93-98 ms churning against a full arena, 29-64 ms not).
    ///
    /// The card index BLOCK is contiguous between the grid and the skirt
    /// (build order is grid, vegetation, skirt), and its first twelve indices
    /// must be exactly the old triangle list re-expressed against four shared
    /// corners - same triangles, same winding, same order, or the image
    /// changed and this was not the free half of the increment.
    #[test]
    fn vegetation_cards_are_indexed_four_corners_not_twelve_vertices() {
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        let mut found = None;
        'outer: for f in 0..20u8 {
            let mut id = PatchId::root(f);
            for _ in 0..TREE_MIN_DEPTH {
                id = id.child(3);
            }
            let c = patch_corners(&id);
            let dir = ((c[0] + c[1] + c[2]) / 3.0).normalize();
            let e = hm.normalized_at(dir.as_vec3());
            let elev = (e - def.sea_level) * (hm.max_meters() - hm.min_meters());
            if elev > 50.0 && elev < 1500.0 {
                found = Some(id);
                break 'outer;
            }
        }
        let id = found.expect("some root chain lands on tree-band terrain");
        let pm = build_patch_mesh(&def, &src, None, &id);
        let n = PATCH_TESS as usize;
        let grid_idx = n * n * 3;
        let skirt_idx = 3 * n * 2 * 3;
        let card_idx = pm.mesh.indices.len() - grid_idx - skirt_idx;
        assert!(card_idx > 0, "no vegetation baked into this patch");
        assert_eq!(card_idx % 12, 0, "card index count {card_idx} is not 12 per card");
        // 4 vertices per 12 indices, counted off the card flags themselves.
        let card_verts = pm
            .mesh
            .vertices
            .iter()
            .filter(|v| v.tree_card)
            .count();
        assert_eq!(
            card_verts * 3,
            card_idx,
            "cards emit {card_verts} vertices for {card_idx} indices - the 4-corner sharing \
             regressed (the old unshared form was 1:1)"
        );
        // First card's triangle list, relative to its own base vertex.
        let base = pm.mesh.indices[grid_idx];
        let first: Vec<u32> = pm.mesh.indices[grid_idx..grid_idx + 12]
            .iter()
            .map(|i| i - base)
            .collect();
        assert_eq!(
            first,
            vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2],
            "card triangle order/winding changed - the image must be bit-identical from item 5"
        );
    }

    /// ITEM 3b: a card frames on the tile's BAKED footprint, so the tree it
    /// draws stands at its own height with its base on the ground - including
    /// the wide-crown case (acacia frames at 1.365h, which the old `let w = h;`
    /// rendered 27% too short with its base 13.4% of the card off the ground).
    #[test]
    fn sprite_card_frame_puts_the_tree_on_the_ground() {
        use crate::renderer::tree_mesh::CardFootprint;
        // (footprint, drawn height). Cases: a 22 m conifer (height-dominant,
        // only the 5% margin), a 9 m acacia with a 1.3:1 crown worked through
        // the baker's own arithmetic, and the pre-bake square default.
        let cases = [
            (CardFootprint { frame_m: 23.1, h_nominal_m: 22.0, base_offset: 0.0238 }, 19.5),
            (CardFootprint { frame_m: 12.285, h_nominal_m: 9.0, base_offset: 0.13370 }, 10.4),
            (CardFootprint::square(8.0), 8.0),
        ];
        for (fp, h) in cases {
            let (s, drop_m) = sprite_card_frame(fp, h);
            // v01 = base_offset is the tree's base: it must sit on the ground.
            let base_world = -drop_m + s * fp.base_offset;
            assert!(
                base_world.abs() < 1e-3,
                "base floats {base_world} m off the ground (frame {}, h {h})",
                fp.frame_m
            );
            // v01 = base_offset + h_nom/frame is the tree's top: exactly h.
            let top_world = -drop_m + s * (fp.base_offset + fp.h_nominal_m / fp.frame_m);
            assert!(
                (top_world - h).abs() < 1e-3,
                "tree top drawn at {top_world} m, wanted {h} m (frame {})",
                fp.frame_m
            );
            // And the card is never SMALLER than the tree it holds.
            assert!(s >= h - 1e-3, "card side {s} m cannot hold a {h} m tree");
        }
        // The regression this fixes, stated numerically: forcing side = h on a
        // wide crown draws it 27% short.
        let acacia = CardFootprint { frame_m: 12.285, h_nominal_m: 9.0, base_offset: 0.13370 };
        let drawn_if_square = acacia.h_nominal_m / acacia.frame_m;
        assert!(
            (drawn_if_square - 0.7326).abs() < 1e-3,
            "worked example drifted: {drawn_if_square}"
        );
    }

    #[test]
    fn patch_id_u64_path_integrity_past_depth_16() {
        // Walk one id to depth 24 taking child 3 then 1 alternately, checking
        // child/parent round-trips and that sibling ids stay DISTINCT at
        // every level. With the old u32 path, levels past 16 shifted bits
        // clean off the top: children collided with each other and with the
        // parent, which this loop catches immediately.
        let mut id = PatchId::root(7);
        for level in 0..24u32 {
            let pick = if level % 2 == 0 { 3 } else { 1 };
            let siblings: Vec<PatchId> = (0..4).map(|i| id.child(i)).collect();
            for a in 0..4 {
                for b in (a + 1)..4 {
                    assert_ne!(
                        siblings[a], siblings[b],
                        "sibling collision at depth {}",
                        level + 1
                    );
                }
            }
            let next = id.child(pick);
            assert_eq!(next.parent(), Some(id), "parent round-trip at depth {}", level + 1);
            assert!(id.is_ancestor_of(&next));
            id = next;
        }
        assert_eq!(id.depth, 24);
        // Corners must remain finite, distinct unit vectors even at depth 24
        // (patch_corners walks the full u64 path).
        let c = patch_corners(&id);
        for v in &c {
            assert!(v.is_finite());
            assert!((v.length() - 1.0).abs() < 1e-12);
        }
        assert_ne!(c[0], c[1]);
        assert_ne!(c[1], c[2]);
    }

    #[test]
    fn restricted_descent_requests_missing_children_draws_parent() {
        let def = earth_like();
        let params = params_for(&def);
        let cam = DVec3::new(def.radius + 5_000.0, 0.0, 0.0);
        let tight = tight_band(&def);
        // Only roots are built.
        let sel = select_patches(
            cam,
            None,
            &|id: &PatchId| (id.depth == 0).then_some(tight),
            &params,
        );
        assert!(sel.fully_covered, "roots cover everything visible");
        assert!(sel.draws.iter().all(|d| d.depth == 0), "draws stay at built roots");
        assert!(!sel.build_requests.is_empty(), "children get requested");
        assert!(sel.build_requests.iter().all(|r| r.depth == 1), "first wave is depth 1");
        assert!(sel.build_requests.len() <= params.max_build_requests);
        // Nothing built at all: not covered, and the uncovered leaves (the
        // roots themselves) head the build queue (hole-filling priority).
        let none = select_patches(cam, None, &|_| None, &params);
        assert!(!none.fully_covered);
        assert!(none.draws.is_empty());
        assert_eq!(none.build_requests[0].depth, 0, "holes build first");
    }

    /// The standstill-flicker contract (v0.1077, from the operator's field
    /// report + run.log forensics): the walk depends on built patches it never
    /// draws (split parents, provably-invisible drops). Those must be reported
    /// in `Selection::required` so the LRU stamps them, because evicting ONE
    /// collapses the subtree below it (restricted descent draws the stalled
    /// parent as a single giant leaf: the operator's log showed draws=12,877
    /// collapsing to draws=1 on a 6.1 s cycle while parked at 11 m altitude).
    /// This test documents both halves: required is non-empty at a ground
    /// camera, and losing one required node really does collapse the cover.
    #[test]
    fn required_patches_are_reported_and_losing_one_collapses_the_cover() {
        let def = earth_like();
        let params = params_for(&def);
        // Near-ground camera like the operator's parked session.
        let cam = DVec3::new(def.radius + 50.0, 0.0, 0.0);
        let tight = tight_band(&def);
        let all_built = |_: &PatchId| Some(tight);
        let sel = select_patches(cam, None, &all_built, &params);
        assert!(sel.fully_covered);
        assert!(
            !sel.required.is_empty(),
            "a ground camera must produce required-but-undrawn patches \
             (split parents at minimum); if this is ever empty the LRU \
             stamping in lib.rs protects nothing"
        );
        // None of the required nodes may also be drawn (they would be
        // double-stamped harmlessly, but the sets are disjoint by design).
        for r in &sel.required {
            assert!(
                !sel.draws.contains(r),
                "{r:?} is both drawn and required; the walk should report it once"
            );
        }
        // Losing a single required node must collapse the cover, which is
        // exactly why eviction of one caused the flicker. Pick a mid-depth
        // one (a root would trivially collapse; mid-depth shows the class).
        let victim = sel
            .required
            .iter()
            .find(|r| r.depth >= 2)
            .copied()
            .unwrap_or(sel.required[0]);
        // The victim must have DRAWN descendants for the collapse to be
        // observable (a leaf-budget-saturated count can mask it, so assert
        // the structure, not the count).
        let victim = sel
            .required
            .iter()
            .filter(|r| r.depth >= 2)
            .find(|r| sel.draws.iter().any(|d| r.is_ancestor_of(d)))
            .copied()
            .expect("some mid-depth required node has drawn descendants");
        let without = |id: &PatchId| (*id != victim).then_some(tight);
        let sel2 = select_patches(cam, None, &without, &params);
        let descendants_before =
            sel.draws.iter().filter(|d| victim.is_ancestor_of(d)).count();
        let descendants_after =
            sel2.draws.iter().filter(|d| victim.is_ancestor_of(d)).count();
        assert!(descendants_before > 0);
        assert_eq!(
            descendants_after, 0,
            "with required {victim:?} evicted, nothing below it can draw \
             (restricted descent stalls at the missing node)"
        );
        assert!(
            sel2.draws
                .iter()
                .any(|d| d.is_ancestor_of(&victim) || *d == victim.parent().unwrap()),
            "an ANCESTOR of the evicted node must take over as one giant \
             leaf: that leaf covering {descendants_before} former draws IS \
             the terrain-vanishing flicker the LRU stamp prevents"
        );
    }

    #[test]
    fn fallback_substitution_never_overlaps() {
        let def = earth_like();
        let params = params_for(&def);
        let cam = DVec3::new(def.radius + 50_000.0, 0.0, 0.0);
        let tight = tight_band(&def);
        // Everything built EXCEPT depth >= 6 (simulates eviction of fine
        // patches): leaves wanting depth >= 6 fall back to their depth-5
        // ancestors, and no drawn patch may be an ancestor of another.
        let sel = select_patches(
            cam,
            None,
            &|id: &PatchId| (id.depth < 6).then_some(tight),
            &params,
        );
        assert!(sel.fully_covered);
        assert!(!sel.draws.is_empty());
        assert!(sel.draws.iter().all(|d| d.depth < 6));
        for a in &sel.draws {
            for b in &sel.draws {
                assert!(
                    !a.is_ancestor_of(b),
                    "drawn {a:?} covers drawn {b:?}: z-fight"
                );
            }
        }
    }

    #[test]
    fn leaf_budget_saturates_gracefully() {
        let def = earth_like();
        let mut params = params_for(&def);
        params.max_leaves = 40;
        let tight = tight_band(&def);
        let cam = DVec3::new(def.radius + 2_000.0, 0.0, 0.0);
        let sel = select_patches(cam, None, &|_| Some(tight), &params);
        assert!(sel.draws.len() <= 40);
        assert!(sel.stats.budget_saturated, "tiny budget must saturate");
        assert!(sel.fully_covered);
    }

    #[test]
    fn frustum_extraction_and_culling() {
        // Camera at origin looking down -Z (glam look_at_rh convention),
        // 60 deg fov, 16:9, celestial-style reverse-Z far plane.
        let view = DMat4::look_at_rh(DVec3::ZERO, DVec3::new(0.0, 0.0, -1.0), DVec3::Y);
        let proj = DMat4::perspective_rh(60f64.to_radians(), 16.0 / 9.0, 1.0e13, 1.0);
        let f = FrustumPlanes::from_view_proj(&(proj * view));
        // In front: visible. Behind: culled. Far off to the side: culled.
        assert!(f.sphere_visible(DVec3::new(0.0, 0.0, -100.0), 1.0));
        assert!(!f.sphere_visible(DVec3::new(0.0, 0.0, 100.0), 1.0));
        assert!(!f.sphere_visible(DVec3::new(1000.0, 0.0, -100.0), 1.0));
        // A big sphere straddling a side plane stays visible.
        assert!(f.sphere_visible(DVec3::new(200.0, 0.0, -100.0), 500.0));

        // Local-frame transform: planet centered 1000 m down -Z, rotated.
        let rot = DQuat::from_rotation_y(1.0);
        let trans = DVec3::new(0.0, 0.0, -1000.0);
        let fl = f.into_local(rot, trans);
        // The planet-local origin maps to (0,0,-1000) in render frame:
        // visible. A local point that maps behind the camera: culled.
        assert!(fl.sphere_visible(DVec3::ZERO, 1.0));
        let behind_local = rot.inverse() * (DVec3::new(0.0, 0.0, 50.0) - trans);
        assert!(!fl.sphere_visible(behind_local, 1.0));

        // Whole-selection integration: camera above the surface looking
        // straight AWAY from the planet -> frustum culls everything (built
        // patches report tight measured bands, which is what makes this
        // sharp: with only the conservative 26 km relief band, spheres
        // near the camera would straddle the view planes forever).
        let def = earth_like();
        let params = params_for(&def);
        let tight = tight_band(&def);
        let cam_local = DVec3::new(def.radius + 5_000.0, 0.0, 0.0);
        // Render frame == local frame here (identity planet transform);
        // looking +X from above the +X pole faces away from the center.
        let view = DMat4::look_at_rh(cam_local, cam_local + DVec3::X, DVec3::Y);
        let proj = DMat4::perspective_rh(60f64.to_radians(), 16.0 / 9.0, 1.0e13, 1.0);
        let fr = FrustumPlanes::from_view_proj(&(proj * view));
        let sel = select_patches(cam_local, Some(&fr), &|_| Some(tight), &params);
        assert!(
            sel.draws.is_empty(),
            "looking away from the planet must draw zero patches, got {}",
            sel.draws.len()
        );
        assert!(sel.stats.frustum_culled > 0);
        // And looking DOWN at the surface keeps patches.
        let view = DMat4::look_at_rh(cam_local, DVec3::ZERO, DVec3::Y);
        let fr = FrustumPlanes::from_view_proj(&(proj * view));
        let sel = select_patches(cam_local, Some(&fr), &|_| Some(tight), &params);
        assert!(!sel.draws.is_empty(), "looking at the planet draws patches");
    }

    #[test]
    fn patch_mesh_counts_and_winding() {
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        let id = PatchId::root(0).child(3).child(1);
        let pm = build_patch_mesh(&def, &src, None, &id);
        let n = PATCH_TESS;
        let grid_tris = (n * n) as usize;
        let skirt_tris = (3 * n * 2) as usize;
        // Index count is layout-independent: 3 per face, grid + skirt.
        assert_eq!(pm.mesh.indices.len(), (grid_tris + skirt_tris) * 3);
        // Shared-vertex grid (draw-batching increment 3): the grid portion
        // must come in well under the old 3-unique-per-face 768, plus the
        // still-unshared skirt (288). The unique grid points alone are 153;
        // provoking-slot duplicates push it up, but far below 768.
        let grid_vert_count = pm.mesh.vertices.len() - skirt_tris * 3;
        assert!(
            grid_vert_count < 480,
            "grid vertex sharing regressed: {grid_vert_count} grid verts (old layout was 768)"
        );
        // Every GRID face must wind CCW from outside: its geometric normal
        // (recomputed from positions THROUGH THE INDICES) points away from
        // the planet center.
        let anchor = pm.anchor;
        for t in 0..grid_tris {
            let p = |k: usize| {
                glam::Vec3::from_array(
                    pm.mesh.vertices[pm.mesh.indices[t * 3 + k] as usize].position,
                )
            };
            let (a, b, c) = (p(0), p(1), p(2));
            let nrm = (b - a).cross(c - a);
            if nrm.length_squared() < 1e-12 {
                continue; // degenerate slivers get the fallback normal
            }
            let centroid_world = anchor + ((a + b + c) / 3.0).as_dvec3();
            let outward = centroid_world.normalize().as_vec3();
            assert!(
                nrm.dot(outward) > 0.0,
                "grid face {t} winds inward (would be backface-culled)"
            );
        }
    }

    #[test]
    fn skirt_hangs_below_the_border() {
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        let id = PatchId::root(2).child(0).child(0).child(0);
        let pm = build_patch_mesh(&def, &src, None, &id);
        let n = PATCH_TESS;
        // Skirt vertices are still emitted unshared, APPENDED after the
        // shared grid: take the tail (the grid portion is variable now).
        let skirt_len = (3 * n * 2) as usize * 3;
        let skirt_verts = &pm.mesh.vertices[pm.mesh.vertices.len() - skirt_len..];
        assert_eq!(skirt_verts.len(), skirt_len);
        let edge_m = patch_edge_arc_m(id.depth, def.radius);
        let expect_depth = (edge_m * SKIRT_EDGE_FRACTION).clamp(SKIRT_MIN_M, SKIRT_MAX_M);
        // Each skirt quad is (s0, s1, b1, then s0, b1, b0): vertices 0,1,3
        // of the 6 are the DROPPED copies; their world radius must sit
        // skirt-depth below their partners' (2,4,5 are on the border).
        let anchor = pm.anchor;
        let radius_of = |v: &SurfaceVertexData| {
            (anchor + glam::Vec3::from_array(v.position).as_dvec3()).length()
        };
        let mut checked = 0;
        for q in skirt_verts.chunks_exact(6) {
            let dropped = radius_of(&q[0]);
            let border = radius_of(&q[2]);
            let dz = border - dropped;
            assert!(
                (dz - expect_depth).abs() < expect_depth * 0.05 + 1.0,
                "skirt drop {dz} != expected {expect_depth}"
            );
            checked += 1;
        }
        assert_eq!(checked, (3 * n) as usize);
    }

    #[test]
    fn anchor_precision_submeter_at_depth_cap() {
        // Design constraint 1: reconstructing world positions as
        // f64 anchor + f32 offset must stay sub-meter (in practice sub-cm)
        // at the depth cap, where triangles are ~54 m.
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        let mut id = PatchId::root(9);
        for i in 0..MAX_PATCH_DEPTH {
            id = id.child((i % 4) as u32);
        }
        assert_eq!(id.depth, MAX_PATCH_DEPTH);
        let pm = build_patch_mesh(&def, &src, None, &id);
        // Reference: recompute the grid positions fully in f64.
        let n = PATCH_TESS;
        let corners = patch_corners(&id);
        let sea = def.sea_level;
        let range_m = hm.max_meters() - hm.min_meters();
        let mut worst = 0.0_f64;
        let mut vi = 0usize; // face-corner counter (through the indices)
        let mut fi = 0usize; // face counter, matching the emission order
        for r in 0..n {
            let row_faces: Vec<[ (u32, u32); 3 ]> = {
                let mut v = Vec::new();
                for c in 0..=r {
                    v.push([(r, c), (r + 1, c), (r + 1, c + 1)]);
                }
                for c in 0..r {
                    v.push([(r, c), (r + 1, c + 1), (r, c + 1)]);
                }
                v
            };
            for face in row_faces {
                // Exact f64 positions for this face's three corners.
                let exact: Vec<DVec3> = face
                    .iter()
                    .map(|&(rr, cc)| {
                        let w0 = (n - rr) as f64;
                        let w1 = (rr - cc) as f64;
                        let w2 = cc as f64;
                        let dir =
                            (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize();
                        // Same elevation pipeline as the builder.
                        let base = hm.normalized_at(dir.as_vec3());
                        let above = (base - sea) * range_m;
                        let mask = smoothstep01(above / DETAIL_LAND_FADE_M);
                        let e = if mask > 0.0 {
                            (base + detail.sample_m(dir, id.depth) * mask / range_m)
                                .clamp(0.0, 1.0)
                        } else {
                            base.clamp(0.0, 1.0)
                        };
                        dir * (def.radius * displaced_radius_f64(&def, e as f64))
                    })
                    .collect();
                // The shared-vertex layout may ROTATE the triple, so match
                // each reconstructed corner to its nearest exact corner.
                for k in 0..3 {
                    let idx = pm.mesh.indices[fi * 3 + k] as usize;
                    let recon = pm.anchor
                        + glam::Vec3::from_array(pm.mesh.vertices[idx].position).as_dvec3();
                    let err = exact
                        .iter()
                        .map(|e| (*e - recon).length())
                        .fold(f64::MAX, f64::min);
                    worst = worst.max(err);
                    vi += 1;
                }
                fi += 1;
            }
        }
        assert!(vi > 0);
        assert!(
            worst < 0.01,
            "anchor+f32 reconstruction error {worst} m (must be sub-meter; expected sub-cm)"
        );
    }

    #[test]
    fn determinism_same_patch_identical() {
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        let id = PatchId::root(5).child(2).child(1).child(3);
        let a = build_patch_mesh(&def, &src, None, &id);
        let b = build_patch_mesh(&def, &src, None, &id);
        assert_eq!(a.anchor, b.anchor);
        assert_eq!(a.mesh.vertices, b.mesh.vertices);
        assert_eq!(a.mesh.indices, b.mesh.indices);
        // The noise path is deterministic too.
        let sampler = SurfaceSampler::new(&def);
        let ns = ElevationSource::Noise(&sampler);
        let c = build_patch_mesh(&def, &ns, None, &id);
        let d = build_patch_mesh(&def, &ns, None, &id);
        assert_eq!(c.mesh.vertices, d.mesh.vertices);
        // And the two sources genuinely differ.
        assert_ne!(a.mesh.vertices, c.mesh.vertices);
    }

    #[test]
    fn same_depth_neighbor_borders_agree_submeter() {
        // Sibling patches share an edge; their independently generated
        // border vertices must land at the same world positions (exact in
        // f64; the only divergence is each patch's own f32 anchor rounding,
        // which must stay far under a centimeter).
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        let parent = PatchId::root(11).child(3).child(2);
        // Child 0 keeps corner0 with edge (m01, m20); child 3 (center) has
        // corners (m01, m12, m20): they share the edge m01-m20.
        let a = build_patch_mesh(&def, &src, None, &parent.child(0));
        let b = build_patch_mesh(&def, &src, None, &parent.child(3));
        let world = |pm: &PatchMesh| -> Vec<DVec3> {
            pm.mesh.indices[..(PATCH_TESS * PATCH_TESS) as usize * 3]
                .iter()
                .map(|&i| {
                    pm.anchor
                        + glam::Vec3::from_array(pm.mesh.vertices[i as usize].position)
                            .as_dvec3()
                })
                .collect()
        };
        let wa = world(&a);
        let wb = world(&b);
        // For each of A's vertices, find B's nearest: along the shared edge
        // the distance must be sub-cm. Count how many matched (the shared
        // edge has PATCH_TESS+1 unique sample points, each appearing in
        // multiple flat-shaded faces).
        let mut matched = 0;
        for pa in &wa {
            let nearest = wb
                .iter()
                .map(|pb| (*pa - *pb).length())
                .fold(f64::MAX, f64::min);
            if nearest < 0.01 {
                matched += 1;
            }
        }
        assert!(
            matched >= (PATCH_TESS + 1) as usize,
            "shared border vertices did not line up: only {matched} matches"
        );
    }

    #[test]
    fn detail_noise_masked_out_at_sea() {
        let def = earth_like();
        // All-ocean grid: detail must contribute NOTHING (a bumpy ocean
        // would break the flat-sea invariant the uniform path guarantees).
        let ocean = synth_heightmap(8, 4, -1000.0, 1000.0, |_, _| -500.0);
        let detail = DetailNoise::new(def.terrain_seed);
        let mut def_ocean = def.clone();
        def_ocean.sea_level = 0.5;
        let src = ElevationSource::Heightmap { hm: &ocean, detail: &detail, tiles: None, ocean: None };
        let pm = build_patch_mesh(&def_ocean, &src, None, &PatchId::root(0).child(3));
        let n = PATCH_TESS;
        for v in &pm.mesh.vertices[..pm.mesh.vertices.len() - (3 * n * 2) as usize * 3] {
            let r = (pm.anchor + glam::Vec3::from_array(v.position).as_dvec3()).length();
            assert!(
                (r - def_ocean.radius).abs() < 0.5,
                "ocean vertex off the sphere: {r}"
            );
        }
        // Sanity: the raw noise is not identically zero, and is
        // deterministic per direction (at any depth).
        let d = DVec3::new(0.3, 0.9, 0.1).normalize();
        assert_eq!(detail.sample_m(d, MAX_PATCH_DEPTH), detail.sample_m(d, MAX_PATCH_DEPTH));
        let mut any = false;
        for i in 0..32 {
            let t = i as f64 * 0.2;
            let dir = DVec3::new(t.cos(), 0.5, t.sin()).normalize();
            if detail.sample_m(dir, MAX_PATCH_DEPTH).abs() > 0.5 {
                any = true;
                break;
            }
        }
        assert!(any, "detail noise never produced signal");
    }

    #[test]
    fn fine_detail_depth_gate_holds() {
        // The close-range extension (v0.818): fine octaves must contribute
        // NOTHING below their Nyquist gate depth (so coarse/far patches stay
        // byte-identical to the base-only ladder) and switch on exactly at it.
        let detail = DetailNoise::new(42);
        // A spread of LAND-ish probe directions (the mask is applied
        // elsewhere; here we exercise the raw sampler).
        let probes: Vec<DVec3> = (0..24)
            .map(|i| {
                let t = i as f64 * 0.31;
                DVec3::new(t.cos(), 0.35 + 0.02 * i as f64, t.sin()).normalize()
            })
            .collect();

        // (1) REGRESSION GATE: every depth strictly below the first fine gate
        // returns the identical base-only value. sample_m(dir, 0) is base-only
        // by construction (0 < every gate), so this proves depths 0..9 are all
        // byte-identical to it -- i.e. unchanged from before this change.
        for d in &probes {
            let base = detail.sample_m(*d, 0);
            for depth in 0..DETAIL_FINE_MIN_DEPTH[0] {
                assert_eq!(
                    detail.sample_m(*d, depth),
                    base,
                    "fine octave leaked into a coarse patch (depth {depth})"
                );
            }
        }

        // (2) Each fine octave switches on exactly at its gate depth: the
        // value at the gate differs from the value one depth shallower for at
        // least one probe (a single Perlin sample can be ~0 by coincidence, so
        // require it across the probe set, not per-direction).
        for (i, &gate) in DETAIL_FINE_MIN_DEPTH.iter().enumerate() {
            let below = gate - 1;
            let changed = probes
                .iter()
                .any(|d| detail.sample_m(*d, gate) != detail.sample_m(*d, below));
            assert!(
                changed,
                "fine octave {i} (gate depth {gate}) produced no change when it engaged"
            );
        }

        // (3) The gate is a Nyquist threshold and, because both wavelength
        // (radius/freq) and triangle edge (radius*angle/2^depth/16) scale with
        // radius, it is RADIUS-INDEPENDENT: recompute samples-per-wavelength
        // at each declared gate and confirm it first crosses 2.0 exactly there.
        for (i, &freq) in DETAIL_FINE_FREQS.iter().enumerate() {
            let gate = DETAIL_FINE_MIN_DEPTH[i];
            let spw = |depth: u8| {
                // wavelength / triangle_edge, radius cancels:
                //   (radius/freq) / (radius*angle/2^depth/16)
                (2u64.pow(depth as u32) as f64 * PATCH_TESS as f64)
                    / (freq * ROOT_EDGE_ANGLE_RAD)
            };
            assert!(spw(gate) >= 2.0, "gate {gate} for freq {freq} is below Nyquist");
            assert!(
                spw(gate - 1) < 2.0,
                "freq {freq} could have gated one depth shallower ({})",
                gate - 1
            );
        }
    }

    #[test]
    fn fine_detail_deep_ocean_stays_smooth() {
        // The land mask gates the fine octaves too: an all-ocean patch built
        // at the MAX depth (where every fine octave is active) must still be a
        // smooth sphere -- ocean geometry stays flat at any LOD.
        let mut def = earth_like();
        def.sea_level = 0.5;
        let ocean = synth_heightmap(8, 4, -1000.0, 1000.0, |_, _| -500.0);
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &ocean, detail: &detail, tiles: None, ocean: None };
        let mut id = PatchId::root(0);
        for _ in 0..MAX_PATCH_DEPTH {
            id = id.child(3);
        }
        assert_eq!(id.depth, MAX_PATCH_DEPTH);
        let pm = build_patch_mesh(&def, &src, None, &id);
        let n = PATCH_TESS;
        for v in &pm.mesh.vertices[..pm.mesh.vertices.len() - (3 * n * 2) as usize * 3] {
            let r = (pm.anchor + glam::Vec3::from_array(v.position).as_dvec3()).length();
            assert!(
                (r - def.radius).abs() < 0.5,
                "deep ocean vertex off the sphere: {r}"
            );
        }
    }

    #[test]
    fn fine_detail_deep_neighbor_borders_agree_submeter() {
        // Border agreement must hold once the fine octaves are live: build two
        // sibling patches at depth 10 (fine octave 0 active) and confirm their
        // shared edge lines up. Both siblings share the same depth, so they
        // hit the same gate and sample the SAME position-seeded field -- the
        // seams stay crack-free exactly as at coarse depths.
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &detail, tiles: None, ocean: None };
        // Walk to a depth-9 parent so its children are depth 10.
        let mut parent = PatchId::root(11);
        for _ in 0..9 {
            parent = parent.child(3);
        }
        assert_eq!(parent.depth, 9);
        assert_eq!(parent.child(0).depth, DETAIL_FINE_MIN_DEPTH[0], "gate must be live");
        let a = build_patch_mesh(&def, &src, None, &parent.child(0));
        let b = build_patch_mesh(&def, &src, None, &parent.child(3));
        // Grid corner positions THROUGH the indices (the shared-vertex
        // layout dedups the vertex array; the face-corner multiset the
        // border comparison needs is index-defined and unchanged).
        let world = |pm: &PatchMesh| -> Vec<DVec3> {
            pm.mesh.indices[..(PATCH_TESS * PATCH_TESS) as usize * 3]
                .iter()
                .map(|&i| {
                    pm.anchor
                        + glam::Vec3::from_array(pm.mesh.vertices[i as usize].position)
                            .as_dvec3()
                })
                .collect()
        };
        let wa = world(&a);
        let wb = world(&b);
        let mut matched = 0;
        for pa in &wa {
            let nearest = wb.iter().map(|pb| (*pa - *pb).length()).fold(f64::MAX, f64::min);
            if nearest < 0.01 {
                matched += 1;
            }
        }
        assert!(
            matched >= (PATCH_TESS + 1) as usize,
            "deep shared border did not line up: only {matched} matches"
        );
    }

    #[test]
    fn fine_detail_adds_real_relief_at_depth_cap() {
        // Sanity that the extension actually does something: over LAND, the
        // depth-cap field (all fine octaves live) must carry MORE radial
        // variation than the base-only field (depth 0). The fine octaves
        // depend only on (direction, depth), so probe land directions across
        // the whole sphere -- no need to land a single patch region on land.
        let def = earth_like();
        let hm = bumpy_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let range_m = hm.max_meters() - hm.min_meters();
        let sea = def.sea_level;
        // Fibonacci-sphere sampling of directions (even coverage).
        let ga = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let count = 4000;
        let mut land_pts = 0;
        // Largest LOCAL displacement (in real radius-meters) the fine octaves
        // add to a land point at the depth cap vs. the base-only field. This
        // is the actual close-range relief the extension buys.
        let mut max_fine_real_m = 0.0_f64;
        for i in 0..count {
            let y = 1.0 - (i as f64 / (count - 1) as f64) * 2.0;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = ga * i as f64;
            let dir = DVec3::new(theta.cos() * r, y, theta.sin() * r).normalize();
            let base = hm.normalized_at(dir.as_vec3());
            let above = (base - sea) * range_m;
            let mask = smoothstep01(above / DETAIL_LAND_FADE_M);
            if mask <= 0.0 {
                continue;
            }
            land_pts += 1;
            let full =
                (base + detail.sample_m(dir, MAX_PATCH_DEPTH) * mask / range_m).clamp(0.0, 1.0);
            let only_base = (base + detail.sample_m(dir, 0) * mask / range_m).clamp(0.0, 1.0);
            let rf = def.radius * displaced_radius_f64(&def, full as f64);
            let rb = def.radius * displaced_radius_f64(&def, only_base as f64);
            max_fine_real_m = max_fine_real_m.max((rf - rb).abs());
        }
        assert!(land_pts > 100, "too few land probes: {land_pts}");
        // The fine tier sums to ~4.3 m pre-exaggeration; after Earth's ~4x
        // it reaches ~10 m+ where octaves align. Require at least a couple of
        // meters of real, resolvable close-range relief.
        assert!(
            max_fine_real_m > 2.0,
            "fine octaves added negligible relief at the cap: {max_fine_real_m} m"
        );
    }

    #[test]
    fn chunk_state_lru_eviction_pins_roots_and_current_frame() {
        let band = RadialBand { min_r_m: 1.0, max_r_m: 2.0 };
        let mut cs = ChunkState::new(1);
        let bytes = PATCH_MESH_BYTES;
        // 3 roots + 3 deep patches inserted across frames 1..6.
        for (i, depth_sel) in [(0u64, true), (1, false), (2, true), (3, false), (4, true), (5, false)]
            .iter()
            .enumerate()
        {
            cs.frame = depth_sel.0 + 1;
            let id = if depth_sel.1 {
                PatchId::root(i as u8)
            } else {
                PatchId::root(i as u8).child(1).child(2)
            };
            cs.insert(id, 100 + i, bytes, DVec3::X, band);
        }
        // Far past the ~2 s recency guard (v0.898), so frames 1..6 are
        // genuinely stale and evictable.
        cs.frame = 300;
        // Cap that forces evicting all but ~4 entries.
        let evicted = cs.collect_evictions(bytes * 4);
        assert!(!evicted.is_empty());
        // Roots never evict.
        for (id, _, _) in &evicted {
            assert!(id.depth > 0, "evicted a pinned root {id:?}");
        }
        assert!(cs.total_bytes <= bytes * 4);
        // LRU order: the oldest deep patch went first.
        assert_eq!(evicted[0].0, PatchId::root(1).child(1).child(2));
        // Entries used THIS frame are safe even over cap.
        let mut cs2 = ChunkState::new(1);
        cs2.frame = 3;
        cs2.insert(PatchId::root(0).child(1), 1, bytes, DVec3::X, band);
        cs2.insert(PatchId::root(0).child(2), 2, bytes, DVec3::X, band);
        let ev = cs2.collect_evictions(bytes); // over cap but all last_used == frame
        assert!(ev.is_empty(), "current-frame entries must not evict");
    }

    #[test]
    fn boundary_walk_is_closed_and_unique() {
        let n = PATCH_TESS;
        let b = boundary_indices(n);
        assert_eq!(b.len(), (3 * n) as usize);
        // All indices unique (each border vertex once).
        let mut seen = std::collections::HashSet::new();
        for i in &b {
            assert!(seen.insert(*i), "border index {i} repeated");
        }
        // Corners present: (0,0), (n,0), (n,n).
        assert!(b.contains(&grid_idx(0, 0)));
        assert!(b.contains(&grid_idx(n, 0)));
        assert!(b.contains(&grid_idx(n, n)));
    }

    #[test]
    fn lod_swap_classifier_pairs_splits_merges_and_orphans() {
        let parent = PatchId::root(3).child(1);
        let kids: Vec<PatchId> = (0..4).map(|i| parent.child(i)).collect();
        // Split: parent vanished, its 4 children appeared.
        let pairs = classify_lod_swaps(&kids, &[parent], &|_| false);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].rising.len(), 4);
        assert_eq!(pairs[0].falling, vec![parent]);
        // Merge: children vanished, parent appeared.
        let pairs = classify_lod_swaps(&[parent], &kids, &|_| false);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].rising, vec![parent]);
        assert_eq!(pairs[0].falling.len(), 4);
        // Orphan appear (fresh stream-in): rises alone, nothing falls.
        let stray = PatchId::root(7).child(0).child(2);
        let pairs = classify_lod_swaps(&[stray], &[], &|_| false);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].rising, vec![stray]);
        assert!(pairs[0].falling.is_empty());
        // Orphan vanish (culled off-screen): pops instantly, no pair.
        let pairs = classify_lod_swaps(&[], &[stray], &|_| false);
        assert!(pairs.is_empty());
        // Mixed frame: one split + one culled orphan = exactly one pair.
        let pairs = classify_lod_swaps(&kids, &[parent, stray], &|_| false);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn ingest_advances_retires_and_purges_reappearances() {
        let mut cs = ChunkState::new(42);
        let parent = PatchId::root(0).child(0);
        let kids: Vec<PatchId> = (0..4).map(|i| parent.child(i)).collect();
        cs.ingest_lod_swaps(&kids, &[parent], 0.0);
        assert_eq!(cs.fades.len(), 1);
        // Fade values: risers positive, faller negative, complementary clock.
        let m = cs.fade_values();
        assert!(m[&kids[0]] > 0.0);
        assert!(m[&parent] < 0.0);
        // A hysteresis flip mid-fade: the parent re-appears -> purged from
        // the falling list so it can never double-mask its own normal draw.
        cs.ingest_lod_swaps(&[parent], &kids, FADE_SECONDS * 0.5);
        for f in &cs.fades {
            assert!(
                !f.falling.contains(&parent),
                "re-appeared patch still falling"
            );
        }
        // Clock retirement: after FADE_SECONDS everything is gone.
        cs.ingest_lod_swaps(&[], &[], FADE_SECONDS * 1.1);
        assert!(cs.fades.is_empty(), "fades survived their clock");
    }

    #[test]
    fn evictions_never_take_a_mid_fade_patch() {
        let mut cs = ChunkState::new(42);
        let parent = PatchId::root(0).child(0);
        let kids: Vec<PatchId> = (0..4).map(|i| parent.child(i)).collect();
        // Cache the parent as a stale entry that would normally be evicted.
        cs.insert(parent, 11, 1_000_000, DVec3::X, RadialBand { min_r_m: 1.0, max_r_m: 2.0 });
        cs.frame = 10_000; // far past the recency guard
        cs.ingest_lod_swaps(&kids, &[parent], 0.0);
        let evicted = cs.collect_evictions(0); // cap 0 = evict everything legal
        assert!(
            evicted.iter().all(|(id, _, _)| *id != parent),
            "mid-fade parent was evicted"
        );
    }

    /// The vegetation biome gate must separate real vegetated biomes from
    /// real barren ones ON THE SHIPPED ALBEDO. The vegetated list includes
    /// the brown-green cases the old strict green-dominance test wrongly
    /// rejected (Tasmania forest r/g 1.09, Kansas prairie 1.01 - the
    /// 2026-07-25 "no trees near me" field report); the barren list pins
    /// deserts and high plateaus bare. If imagery or grading changes shift
    /// these ratios, this fails and the threshold gets re-measured.
    #[test]
    fn biome_gate_separates_vegetated_from_barren() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = crate::terrain::planet_heightmap::PlanetHeightmap::load(
            &base.join("earth_heightmap.bin"),
        )
        .unwrap();
        let albedo =
            crate::terrain::planet_albedo::PlanetAlbedo::load(&base.join("earth_albedo.bin"))
                .unwrap();
        let mut def = earth_like();
        def.sea_level = hm.sea_level_normalized();
        let sample = |lat_deg: f64, lon_deg: f64| -> [f32; 3] {
            let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
            let cl = lat.cos();
            let dir = glam::DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
            let e = hm.normalized_at(dir.as_vec3());
            surface_color(&def, Some(&albedo), dir.as_vec3(), e)
        };
        let vegetated = [
            ("amazon", -3.0, -60.0),
            ("black_forest", 48.2, 8.2),
            ("siberia", 58.0, 98.0),
            ("fuji", 35.3, 138.8),
            ("congo", -1.0, 22.0),
            ("appalachia", 37.5, -81.0),
            ("scotland", 57.0, -4.5),
            ("se_australia", -36.5, 146.5),
            ("tasmania", -42.0, 146.5),
            ("kansas", 38.5, -98.0),
        ];
        let barren = [
            ("sahara", 23.0, 13.0),
            ("outback", -25.0, 133.0),
            ("spain_meseta", 40.0, -3.0),
            ("gobi", 43.0, 105.0),
            ("tibet", 33.0, 90.0),
        ];
        for (name, lat, lon) in vegetated {
            let sc = sample(lat, lon);
            assert!(veg_biome_ok(sc), "{name} must pass the vegetation gate, got {sc:?}");
        }
        for (name, lat, lon) in barren {
            let sc = sample(lat, lon);
            assert!(!veg_biome_ok(sc), "{name} must stay barren, got {sc:?}");
        }
    }

    /// Operator field report 2026-07-25: "trees still won't render near me
    /// within like 50 meters" - the [NearTree] log showed EVERY harvest
    /// returning 0 trees (93 recomputes, alt 275 m down to 4 m) while the
    /// card stream clearly plants trees at the same spots. This test runs
    /// the REAL harvest over the SHIPPED Earth data at three famously
    /// forested places; any of them returning zero reproduces the bug.
    #[test]
    fn near_tree_harvest_finds_trees_in_real_forests() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = crate::terrain::planet_heightmap::PlanetHeightmap::load(
            &base.join("earth_heightmap.bin"),
        )
        .expect("earth heightmap loads");
        let albedo =
            crate::terrain::planet_albedo::PlanetAlbedo::load(&base.join("earth_albedo.bin"))
                .expect("earth albedo loads");
        let mut def = earth_like();
        // The REAL Earth sea level (the shipped grid's), not the synthetic one.
        def.sea_level = hm.sea_level_normalized();

        let spots = [
            ("amazon", -3.0_f64, -60.0_f64),
            ("black_forest", 48.2, 8.2),
            ("siberian_taiga", 58.0, 98.0),
            // Brown-green texels the old gate starved (field report):
            ("tasmania", -42.0, 146.5),
            ("fuji_descent", 35.29, 138.79),
        ];
        let detail = DetailNoise::new(7);
        for (name, lat_deg, lon_deg) in spots {
            let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
            let cl = lat.cos();
            let dir = glam::DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
            let src = ElevationSource::Heightmap {
                hm: &hm,
                detail: &detail,
                tiles: None,
                ocean: None,
            };
            let trees = near_tree_instances(&def, &src, Some(&albedo), dir, 360.0, 600);
            assert!(
                !trees.is_empty(),
                "harvest found ZERO trees at {name} ({lat_deg},{lon_deg}) - reproduces the operator's bare-ground report"
            );
            // v0.969 regression (operator: "I see their shadows, but I can
            // never get close"): at 8x density the row-major cell walk
            // filled the cap from the disc's corner, so the NEAREST
            // returned tree could be hundreds of metres away and whole
            // quadrants around the camera were empty. Nearest-first cells
            // must put a tree within ~60 m of a dense-forest center and
            // populate at least 3 of the 4 quadrants.
            let nearest_m = trees
                .iter()
                .map(|t| t.dir.dot(dir.normalize()).clamp(-1.0, 1.0).acos() * def.radius)
                .fold(f64::MAX, f64::min);
            assert!(
                nearest_m < 60.0,
                "{name}: nearest harvested tree is {nearest_m:.0} m away - the cap filled far from the camera"
            );
            let east = glam::DVec3::Y.cross(dir).normalize();
            let north = dir.normalize().cross(east).normalize();
            let mut quads = [false; 4];
            for t in &trees {
                let rel = t.dir - dir.normalize();
                let e = rel.dot(east);
                let n = rel.dot(north);
                let q = (if e >= 0.0 { 0 } else { 1 }) + (if n >= 0.0 { 0 } else { 2 });
                quads[q] = true;
            }
            assert!(
                quads.iter().filter(|q| **q).count() >= 3,
                "{name}: trees cover only {quads:?} - the harvest is spatially lopsided"
            );
        }
    }

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
        let (_, stats) = grass_tiller_mesh();
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
            let drawn = g
                .iter()
                .filter(|t| grass_live_emerge(t.thr, surf_d(&def, c, t.dir)) > 0.0)
                .count();
            println!(
                "[grass cost] {name} depth {depth}: {} tillers harvested (superset), \
                 {drawn} drawn, {} triangles drawn, {:.1} ms (this build profile)",
                g.len(),
                drawn * stats.triangles,
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }
        // The shipped Settings default is 0.6; tests run at the 1.0 the atomic
        // initialises to. Density is exactly linear in it (`count` is a
        // product), so the shipped figure is the measured one scaled.
        let dens = veg_density();
        let shipped = 0.6_f64 / dens as f64;
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
            let tillers_m2 = n / area;
            let blades_m2 = tillers_m2 * stats.blades as f64;
            // LAI: one-sided leaf area per unit ground. The mesh is built at
            // unit height, so an instance carries area * height^2 - and the
            // height is the LIVE one, exactly what the renderer scales by.
            let leaf: f64 = g
                .iter()
                .map(|t| {
                    let h = (t.height_m
                        * grass_live_emerge(t.thr, surf_d(&def, c, t.dir)))
                        as f64;
                    stats.one_sided_area_unit as f64 * h * h
                })
                .sum();
            let lai = leaf / area;
            println!(
                "[grass sward] {name}: {n} tillers, {tillers_m2:.1}/m2, \
                 {blades_m2:.0} blades/m2, LAI {lai:.2} (at veg_density {dens:.2}); \
                 shipped 0.6 -> {:.0} blades/m2, LAI {:.2}",
                blades_m2 * shipped,
                lai * shipped
            );
            assert!(
                blades_m2 * shipped >= 150.0,
                "{name}: {:.0} blades/m2 at the shipped density - a sward is a mat, not a \
                 scatter (the deleted bake managed 0.5)",
                blades_m2 * shipped
            );
            let lai_shipped = lai * shipped;
            assert!(
                (1.5..=5.0).contains(&lai_shipped),
                "{name}: effective LAI {lai_shipped:.2} at the shipped density, wanted \
                 1.5..5.0 against the measured turf range 1.9-6.0"
            );
        }
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
                for t in &g {
                    let rel = (t.dir - up) * def.radius;
                    let (e, n) = (rel.dot(east), rel.dot(north));
                    if e.abs() >= 5.5 || n.abs() >= 5.5 {
                        continue;
                    }
                    grid[(n + 5.5) as usize][(e + 5.5) as usize] += 1;
                }
                for (gy, row) in grid.iter().enumerate() {
                    for (gx, v) in row.iter().enumerate() {
                        let (dy, dx) = (gy as f64 - 5.0, gx as f64 - 5.0);
                        if dy * dy + dx * dx <= 25.0 {
                            counts.push(*v);
                        }
                    }
                }
            }
        }
        assert!(sites >= 4, "only {sites} vegetated sites sampled - test is not measuring");
        let n = counts.len() as f64;
        let mean = counts.iter().map(|c| *c as f64).sum::<f64>() / n;
        let var = counts.iter().map(|c| (*c as f64 - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let vmr = var / mean.max(1e-6);
        let empty = counts.iter().filter(|c| **c == 0).count() as f64 / n;
        println!(
            "[grass clumping] {} quadrats over {sites} sites: mean {mean:.1}/m2, \
             var {var:.1}, variance-to-mean {vmr:.2}, {:.1}% empty",
            counts.len(),
            empty * 100.0
        );
        assert!(
            vmr >= 2.0,
            "variance-to-mean {vmr:.2} - an exact Poisson process scores 1.0, which is what \
             the deleted bake produced. The field must have visibly thicker and thinner \
             patches."
        );
        // Sanity on the other side: the clump gain is meant to have mean 1, so
        // the realised density must still be the one GRASS_PEAK_PER_M2 claims.
        let want = (GRASS_PEAK_PER_M2 * veg_density()) as f64;
        assert!(
            mean > want * 0.75 && mean < want * 1.25,
            "mean {mean:.1} tillers/m2 against the nominal {want:.1} - grass_clump_gain's \
             mean has drifted off 1.0, so GRASS_PEAK_PER_M2 no longer means what it says"
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

    /// The shared tiller mesh has to be a fan of ARCHING, TAPERED blades with
    /// per-corner normals and the leaf organ bit - every one of those is a
    /// specific defect the deleted card had (ruler-straight edges, a normal
    /// pinned to the radial up so every tuft on the planet lit identically,
    /// and no transmission path at all because it went through the type-12
    /// terrain branch).
    #[test]
    fn grass_tiller_is_a_fan_of_arching_tapered_blades() {
        use crate::terrain::planet_surface::unpack_uv_to_color;
        let (b, stats) = grass_tiller_mesh();
        assert_eq!(stats.blades, GRASS_BLADES_PER_TILLER);
        assert_eq!(stats.triangles, b.indices.len() / 3);
        // 3 segments/blade: 2 quads (4 tris) + 1 tip tri = 5, doubled because
        // the opaque pipeline back-culls and a blade must be visible from
        // behind. 90 triangles for 9 blades.
        assert_eq!(stats.triangles, GRASS_BLADES_PER_TILLER * 10);
        assert!(
            stats.triangles <= 96,
            "{} triangles per tiller - MEASURED at v0.1090, a 22 m harvest is ~20,800 \
             tillers at veg_density 1.0 (~12,500 at the shipped 0.6), so every triangle \
             here is multiplied by five figures and again by the shadow pass",
            stats.triangles
        );
        // Every face carries the LEAF organ bit (shader `is_leaf`), so a
        // backlit sward glows instead of being shaded as bark.
        for v in &b.vertices {
            let packed = v.uv[0].round().max(0.0) as u32;
            assert!(
                packed & 524_288 != 0,
                "a grass face is not tagged as leaf tissue - it would shade as stem"
            );
        }
        // TAPER: the widest cross-section is at the crown, the tip is a
        // point. Measure the horizontal spread of vertices by height band.
        let mut low = 0.0f32;
        for v in &b.vertices {
            if v.position[1] < 0.15 {
                low = low.max((v.position[0].powi(2) + v.position[2].powi(2)).sqrt());
            }
        }
        // ARCH: the tips must have fallen OUTWARD, so the widest radius is
        // near the top, not at the crown - a straight blade reads as bristle.
        let high = b
            .vertices
            .iter()
            .filter(|v| v.position[1] > 0.60)
            .map(|v| (v.position[0].powi(2) + v.position[2].powi(2)).sqrt())
            .fold(0.0f32, f32::max);
        assert!(
            high > low * 1.5,
            "blade tips reach {high:.3} against a crown spread of {low:.3} - the blades \
             are not arching, they are standing straight up"
        );
        // NORMALS: not all identical (the card's defect - every tuft on the
        // planet lit the same because `nrm = up`), and never more than
        // half-way from the radial up toward the blade facing (the v0.896
        // black-slab guard: N.L must not be able to reach 0). Undersides
        // carry the mirrored normal, which is correct thin-tissue geometry,
        // so the guard is on the MAGNITUDE.
        let tops: Vec<_> = b.vertices.iter().filter(|v| v.normal[1] > 0.0).collect();
        assert!(!tops.is_empty(), "no upward-facing grass normals at all");
        let n0 = tops[0].normal;
        assert!(
            tops.iter()
                .any(|v| (v.normal[0] - n0[0]).abs() + (v.normal[2] - n0[2]).abs() > 0.3),
            "every corner carries the same normal - one tiller has no lit side and no \
             shaded side, which is exactly what the deleted card did"
        );
        for v in &b.vertices {
            assert!(
                v.normal[1].abs() >= 0.35,
                "a grass normal has tipped past half-way toward the blade facing \
                 (n.y {:.2}); at grazing sun that face goes black",
                v.normal[1]
            );
        }
        // BEER-LAMBERT RAMP: the crown segment must be markedly darker than
        // the tip segment. Colour rides the packed UV, one shade per face.
        let shade_at = |band: (f32, f32)| -> f32 {
            let mut sum = 0.0;
            let mut n = 0.0;
            for tri in b.indices.chunks_exact(3) {
                let vs: Vec<_> = tri.iter().map(|i| b.vertices[*i as usize]).collect();
                let y = vs.iter().map(|v| v.position[1]).sum::<f32>() / 3.0;
                if y >= band.0 && y < band.1 {
                    let (c, _) = unpack_uv_to_color(vs[0].uv);
                    sum += c[1];
                    n += 1.0;
                }
            }
            if n > 0.0 { sum / n } else { 0.0 }
        };
        let (baseg, tipg) = (shade_at((0.0, 0.25)), shade_at((0.55, 2.0)));
        assert!(
            baseg > 0.0 && tipg > 0.0,
            "no faces found in the base/tip bands ({baseg}, {tipg})"
        );
        assert!(
            baseg < tipg * 0.60,
            "sward base reads {baseg:.3} against tips {tipg:.3} - a real sward's base sits \
             at 20-30% of top-of-canopy irradiance; a uniformly lit blade is a sticker"
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
        assert_eq!(grass_density_at(0.0), GRASS_PEAK_PER_M2);
        assert_eq!(grass_density_at(GRASS_NEAR_M), GRASS_PEAK_PER_M2);
        assert!((grass_density_at(GRASS_MID_M) - GRASS_MID_PER_M2).abs() < 1e-3);
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
            worst < GRASS_PEAK_PER_M2 * 0.02,
            "the ramp steps by {worst:.2} tillers/m2 in 10 cm - that is an edge"
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
