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
pub const PATCH_CACHE_MAX_BYTES: usize = 1536 * 1024 * 1024;

/// Cache floor applied ONCE when a planet leaves chunked mode (the camera
/// flew away): shrink to this so a departed planet parks ~64 MB of warm
/// patches (fast re-approach) instead of the full 256 MB. Roots stay
/// pinned regardless, so re-activation never starts from zero.
pub const PATCH_CACHE_WARM_BYTES: usize = 64 * 1024 * 1024;

/// GPU byte estimate for one built patch (see PATCH_CACHE_MAX_BYTES).
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
/// Patch depth at which GRASS tufts appear (27 m patches, walking range).
pub const GRASS_MIN_DEPTH: u8 = 18;
/// Vegetation cell grid (v0.897): plant positions come from a PLANET-FIXED
/// lat/lon hash grid, not from each patch's own rng - so the same plants
/// stand in the same spots at every LOD depth. (They used to reshuffle on
/// every split: with 40 trees per patch the whole forest visibly rolled on
/// each LOD swap - a big share of the residual "terrain flicker" the
/// operator reported, and why grass seemed to vanish near the player.)
/// Cell size is radians of arc on the unit sphere.
pub const TREE_CELL_RAD: f64 = 3.45e-5; // ~220 m at the equator
/// Expected trees per tree cell at the equator (~864 trees per km^2);
/// scaled by cos(lat) so density stays constant per square km.
pub const TREES_PER_CELL: u32 = 42;
/// Grass cell size (~33 m) and tufts per cell (~0.033 per m^2).
pub const GRASS_CELL_RAD: f64 = 5.2e-6;
pub const GRASS_PER_CELL: u32 = 36;
/// Real-meter elevation ceiling for trees (a global treeline placeholder).
pub const TREELINE_M: f32 = 1700.0;

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
pub fn tile_or_base(
    hm: &PlanetHeightmap,
    tiles: Option<&super::terrain_tiles::TerrainTiles>,
    dir: DVec3,
    depth: u8,
) -> (f32, bool) {
    if depth >= TILE_MIN_DEPTH {
        if let Some(t) = tiles {
            let (lat, lon) = super::planet_heightmap::dir_to_latlon_deg(dir.as_vec3());
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
    dir: glam::Vec3,
) -> f32 {
    let (base, from_tile) = tile_or_base(hm, tiles, dir.as_dvec3(), FINEST_DETAIL_DEPTH);
    let range_m = hm.max_meters() - hm.min_meters();
    if range_m <= 0.0 {
        return base.clamp(0.0, 1.0);
    }
    let sea = def.sea_level.clamp(0.0, 1.0);
    let above_sea_m = (base - sea) * range_m;
    let mask = smoothstep01(above_sea_m / DETAIL_LAND_FADE_M);
    let e = if mask > 0.0 {
        let dm = if from_tile {
            detail.sample_m_tile_gated(dir.as_dvec3(), FINEST_DETAIL_DEPTH)
        } else {
            detail.sample_m(dir.as_dvec3(), FINEST_DETAIL_DEPTH)
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
}

/// Selection outcome for one planet this frame.
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
    let mut prefetches: usize = 0;

    // Visibility check shared by roots and children. Returns None when
    // culled (and counts why).
    let mut visible = |corners: &[DVec3; 3],
                       band: &RadialBand,
                       stats: &mut SelectStats|
     -> Option<PatchBounds> {
        let b = patch_bounds(corners, params.radius_m, band);
        if horizon_culled(&b, cam_local_m, params.band.min_r_m) {
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
        let split_thr = if was_split {
            params.split_px * 0.7
        } else {
            params.split_px
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

    Selection { draws, build_requests, fully_covered, stats }
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

    let mut emit_face = |ia: usize, ib: usize, ic: usize,
                         vertices: &mut Vec<SurfaceVertexData>,
                         indices: &mut Vec<u32>| {
        let mean_e = (elevs[ia] + elevs[ib] + elevs[ic]) / 3.0;
        let centroid_dir = ((dirs[ia] + dirs[ib] + dirs[ic]) / 3.0).normalize();
        // Real imagery when the def ships an albedo grid (Earth), the
        // elevation-band classifier otherwise -- shared with the uniform
        // sphere path so zero color logic is duplicated.
        let color = surface_color(def, albedo, centroid_dir.as_vec3(), mean_e);
        let underwater = def.has_water && mean_e < sea;
        if underwater {
            // Smooth ocean: per-corner spherical normals. Positions are
            // already on the undisplaced sphere (displaced_radius clamps
            // below-sea to 1.0 on water worlds). water: true drives the
            // shader's sun glint.
            for &i in &[ia, ib, ic] {
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: offsets[i].to_array(),
                    normal: dirs[i].as_vec3().to_array(),
                    color,
                    water: true,
                });
            }
        } else {
            let (p0, p1, p2) = (offsets[ia], offsets[ib], offsets[ic]);
            let mut nrm = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            let out = centroid_dir.as_vec3();
            if nrm.length_squared() < 1e-9 || nrm.dot(out) < 0.0 {
                // Degenerate or inward-wound: outward spherical fallback,
                // never an inside-out face.
                nrm = out;
            }
            // Slope shading stays per-FACE (color corners must match for
            // the packed transport); LIGHTING normals are the smooth
            // per-vertex averages (v0.884) so quantization ledges melt.
            let shade = slope_shade(nrm, out);
            let color = [color[0] * shade, color[1] * shade, color[2] * shade];
            for &i in &[ia, ib, ic] {
                indices.push(vertices.len() as u32);
                vertices.push(SurfaceVertexData {
                    position: offsets[i].to_array(),
                    normal: vnorm[i].to_array(),
                    color,
                    water: false,
                });
            }
        }
    };

    // Grid triangles: between rows r and r+1 there are r+1 up-pointing and
    // r down-pointing triangles; both windings verified CCW-from-outside
    // (they match the parent corner orientation, which matches the
    // icosphere the backface-culling pipeline already draws correctly).
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

    // ── Procedural vegetation (v0.888; planet-fixed cells v0.897) ──
    // Crossed-quad trunks + diamond canopies, grass as crossed cards. Plant
    // positions and looks come from a PLANET-FIXED lat/lon cell grid hashed
    // per cell, so every patch depth regenerates the identical plants and
    // LOD swaps never reshuffle the forest.
    {
        let range_m = match source {
            ElevationSource::Heightmap { hm, .. } => hm.max_meters() - hm.min_meters(),
            ElevationSource::Noise(_) => 8000.0,
        };
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
            for tri in [[p00, p01, p11], [p00, p11, p10], [p00, p11, p01], [p00, p10, p11]] {
                for p in tri {
                    indices.push(vertices.len() as u32);
                    vertices.push(SurfaceVertexData {
                        position: p.to_array(),
                        normal: nrm.to_array(),
                        color,
                        water: false,
                    });
                }
            }
        };
        let want_trees = id.depth >= TREE_MIN_DEPTH;
        let want_grass = id.depth >= GRASS_MIN_DEPTH;
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
        for pass in 0..2 {
            let is_tree = pass == 0;
            if polar || (is_tree && !want_trees) || (!is_tree && !want_grass) {
                continue;
            }
            let cell = if is_tree { TREE_CELL_RAD } else { GRASS_CELL_RAD };
            let per_cell = if is_tree { TREES_PER_CELL } else { GRASS_PER_CELL };
            let salt: u64 = if is_tree { 0x51F0_A11C } else { 0x9A55_77EE };
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
                        if elev_m < 3.0 || elev_m > TREELINE_M {
                            continue;
                        }
                        // Biome gate (v0.896): vegetation only where the
                        // surface COLOR is green-dominant - the same
                        // imagery/ramp the ground renders with. Real Earth
                        // imagery is the planet-wide biome map for free.
                        let sc = surface_color(def, albedo, dir.as_vec3(), e);
                        if !(sc[1] > sc[0] * 1.04 && sc[1] > sc[2] * 1.04) {
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
                        if !is_tree {
                            // Grass tuft: two crossed cards, straw-green.
                            let g = 0.25 + (r3 % 100) as f32 / 400.0;
                            let col = [0.24, 0.34 + (r4 % 100) as f32 / 1000.0, 0.10];
                            emit_card(base, up, side_a, 0.5, 0.0, g, col, &mut vertices, &mut indices);
                            emit_card(base, up, side_b, 0.5, 0.0, g, col, &mut vertices, &mut indices);
                        } else {
                            // Tree: trunk cards (brown) + canopy cards (dark
                            // green), 7-13 m - a conifer silhouette at range.
                            let h = 7.0 + (r3 % 100) as f32 / 100.0 * 6.0;
                            let trunk = [0.30, 0.22, 0.13];
                            let canopy = [
                                0.08 + (r4 % 60) as f32 / 1000.0,
                                0.26 + (r5 % 80) as f32 / 1000.0,
                                0.10,
                            ];
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

/// Water-shell patch cap: waves need mesh only down to the finest geometric
/// train (50 m wavelength -> Nyquist at ~25 m triangles = depth 14); the
/// vertex shader adds the height, so deeper mesh buys nothing.
pub const WATER_MAX_PATCH_DEPTH: u8 = 14;

/// Water-shell leaf budget: the shell shares MAX_OBJECTS with terrain
/// patches (640-768) + sky bodies, so it gets a deliberately small slice --
/// a smooth constant-radius sphere needs far fewer leaves than terrain.
pub const WATER_MAX_LEAVES: usize = 144;

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
    for iy in ylo..=yhi {
        let cell_lat = (iy as f64 + 0.5) * cell;
        let count = ((TREES_PER_CELL as f64) * cell_lat.cos().max(0.0)).round() as u32;
        for ix in xlo..=xhi {
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
                let lat = (iy as f64 + (r0 % 4096) as f64 / 4096.0) * cell;
                let lon = (ix as f64 + (r1 % 4096) as f64 / 4096.0) * cell;
                let cl = lat.cos();
                let dir = DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
                if dir.dot(center) < cos_ang {
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
                if elev_m < 3.0 || elev_m > TREELINE_M {
                    continue;
                }
                let sc = surface_color(def, albedo, dir.as_vec3(), e);
                if !(sc[1] > sc[0] * 1.04 && sc[1] > sc[2] * 1.04) {
                    continue;
                }
                let r = def.radius
                    * if bathymetric {
                        displaced_radius_f64_true(def, e as f64)
                    } else {
                        displaced_radius_f64(def, e as f64)
                    };
                let yaw = (r2 % 6283) as f32 / 1000.0;
                let h = 7.0 + (r3 % 100) as f32 / 100.0 * 6.0;
                out.push(NearTree {
                    dir,
                    r_m: r,
                    yaw,
                    height_m: h,
                    species: ((r5 >> 9) & 1) as u8,
                });
            }
        }
    }
    out
}

/// Conservative radial band for water-shell selection/culling: the sea
/// sphere plus the worst-case analytic wave height either way (the vertex
/// shader displaces within this envelope), plus skirt + slop.
pub fn water_band(radius_m: f64) -> RadialBand {
    let wave = crate::terrain::ocean_waves::MAX_WAVE_HEIGHT_M as f64;
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
    id: &PatchId,
) -> Option<PatchMesh> {
    let n = PATCH_TESS;
    let corners = patch_corners(id);
    let radius_m = def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64;
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
            if ocean.is_ocean(dir.as_vec3()) {
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
    // Color is unused by the type-16 shader (it derives everything from the
    // planet-local frame), but keep the def's water color so any debug view
    // of the raw mesh reads sensibly.
    let color = [def.water_color[0], def.water_color[1], def.water_color[2]];
    let mut emit_face = |ia: usize, ib: usize, ic: usize,
                         vertices: &mut Vec<SurfaceVertexData>,
                         indices: &mut Vec<u32>| {
        for &i in &[ia, ib, ic] {
            indices.push(vertices.len() as u32);
            vertices.push(SurfaceVertexData {
                position: offsets[i].to_array(),
                normal: dirs[i].as_vec3().to_array(),
                color,
                water: true,
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
    /// Index into Renderer::meshes.
    pub mesh: usize,
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
        }
    }

    pub fn insert(&mut self, id: PatchId, mesh: usize, bytes: usize, anchor: DVec3, band: RadialBand) {
        if let Some(old) = self.cache.insert(
            id,
            PatchEntry { mesh, bytes, anchor, band, last_used: self.frame },
        ) {
            // Should not happen (selection never requests a built patch),
            // but never leak the byte count if it does.
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
        }
        self.total_bytes += bytes;
    }

    /// Pop LRU entries until under the byte cap. Returns the (id, mesh
    /// index) pairs removed so the engine can recycle the renderer slots.
    /// Never evicts roots (depth 0: the permanent whole-planet fallback)
    /// or anything used this frame.
    pub fn collect_evictions(&mut self, byte_cap: usize) -> Vec<(PatchId, usize)> {
        let mut evicted = Vec::new();
        // Recency guard (v0.898): never evict anything used in the last ~2
        // seconds. When the working set genuinely exceeds the cap, evicting
        // the just-culled ring made every camera micro-turn a rebuild storm;
        // running temporarily over cap is strictly cheaper than thrash.
        let recent = self.frame.saturating_sub(120);
        while self.total_bytes > byte_cap {
            let victim = self
                .cache
                .iter()
                .filter(|(id, e)| id.depth > 0 && e.last_used < recent)
                .min_by_key(|(id, e)| (e.last_used, **id))
                .map(|(id, _)| *id);
            let Some(id) = victim else { break };
            if let Some(e) = self.cache.remove(&id) {
                self.total_bytes = self.total_bytes.saturating_sub(e.bytes);
                evicted.push((id, e.mesh));
            }
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let pm = build_water_patch_mesh(&def, &synth_mask(true), &id)
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
            build_water_patch_mesh(&def, &synth_mask(false), &id).is_none(),
            "all-land patch must not build water"
        );
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
        assert_eq!(pm.mesh.vertices.len(), (grid_tris + skirt_tris) * 3);
        assert_eq!(pm.mesh.indices.len(), (grid_tris + skirt_tris) * 3);
        assert_eq!(pm.mesh.vertices.len(), 1056, "the documented 37 KB patch");
        // Sequential indices (flat shading).
        assert!(pm.mesh.indices.iter().enumerate().all(|(i, &v)| v as usize == i));
        // Every GRID face must wind CCW from outside: its geometric normal
        // (recomputed from positions) points away from the planet center.
        let anchor = pm.anchor;
        for t in 0..grid_tris {
            let p = |k: usize| glam::Vec3::from_array(pm.mesh.vertices[t * 3 + k].position);
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
        let grid_tris = (n * n) as usize;
        let skirt_verts = &pm.mesh.vertices[grid_tris * 3..];
        assert_eq!(skirt_verts.len(), (3 * n * 2) as usize * 3);
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
        let mut vi = 0usize; // walks the flat-shaded grid emission order
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
                for (rr, cc) in face {
                    let w0 = (n - rr) as f64;
                    let w1 = (rr - cc) as f64;
                    let w2 = cc as f64;
                    let dir = (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize();
                    // Same elevation pipeline as the builder.
                    let base = hm.normalized_at(dir.as_vec3());
                    let above = (base - sea) * range_m;
                    let mask = smoothstep01(above / DETAIL_LAND_FADE_M);
                    let e = if mask > 0.0 {
                        (base + detail.sample_m(dir, id.depth) * mask / range_m).clamp(0.0, 1.0)
                    } else {
                        base.clamp(0.0, 1.0)
                    };
                    let exact = dir * (def.radius * displaced_radius_f64(&def, e as f64));
                    let recon = pm.anchor
                        + glam::Vec3::from_array(pm.mesh.vertices[vi].position).as_dvec3();
                    worst = worst.max((exact - recon).length());
                    vi += 1;
                }
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
            pm.mesh.vertices[..(PATCH_TESS * PATCH_TESS) as usize * 3]
                .iter()
                .map(|v| pm.anchor + glam::Vec3::from_array(v.position).as_dvec3())
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
        for v in &pm.mesh.vertices[..(n * n) as usize * 3] {
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
        for v in &pm.mesh.vertices[..(n * n) as usize * 3] {
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
        assert_eq!(a.mesh.vertices.len(), 1056);
        let world = |pm: &PatchMesh| -> Vec<DVec3> {
            pm.mesh.vertices[..(PATCH_TESS * PATCH_TESS) as usize * 3]
                .iter()
                .map(|v| pm.anchor + glam::Vec3::from_array(v.position).as_dvec3())
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
        for (id, _) in &evicted {
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
}
