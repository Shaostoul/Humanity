//! OSM features around an origin: the local metric frame, a polygon ear
//! clipper, and the 3D extrusion mesher.
//!
//! An `OsmRegion` is a set of REAL OpenStreetMap roads, building footprints
//! and water polygons in metres east/north of an origin. Since whole-Earth
//! maps rung 2 (2026-11-09) it is cut from the installed map packs
//! (`region_from_pack`, any group of level-12 tiles), no longer read from the
//! retired HOSMREG2 region files. Two consumers share it so they cannot drift:
//!
//! - the Maps page's 2D Planet view (`src/gui/pages/maps/planet_view.rs`), which draws
//!   a named place as a slippy-map style plan; and
//! - the in-world 3D extruder (`src/engine/map_meshes.rs`), which raises
//!   the same footprints as real geometry on the chunked-LOD Earth.
//!
//! ODbL: everything here is "Data (c) OpenStreetMap contributors, ODbL 1.0".
//! Any surface that DRAWS it must show that credit. That is a licence
//! obligation, not a courtesy.
//!
//! ## Precision discipline (the f64 rule, CLAUDE.md)
//!
//! Coordinates are metres east/north of the origin as f32, EXACT at these
//! magnitudes (a few km, so ulp is well under a millimetre); the pack holds
//! OSM's own 10^7-degree integers and the conversion runs in f64. Everything
//! after that stays f64 until the very last step:
//!
//! ```text
//!   region metres (f32, exact)
//!     -> lat/lon degrees          (f64, region_meters_to_latlon)
//!     -> unit direction           (f64, latlon_to_dir_f64)
//!     -> drawn ground radius      (f64, the caller's elev closure)
//!     -> dir * radius - anchor    (f64 subtraction at planet scale)
//!     -> f32 ANCHOR-RELATIVE offset (<= ~1.7 km, ulp ~0.1 mm)
//! ```
//!
//! The region anchor (`RegionMeshes::anchor_local`) is the only planet-scale
//! quantity in the output and it never passes through f32.
//!
//! Relay-safe on purpose: the conversion, the projection, and the ear clipper
//! are pure std + glam, so they compile and unit-test under
//! `--features relay --no-default-features`. Only the mesher (which speaks
//! `vertex::Vertex`) is gated behind `native`.

use glam::DVec3;

#[cfg(feature = "native")]
use humanity_core::vertex::Vertex;

// ───────────────────────────── Region data types ────────────────────────────

/// One road. Coordinates are METRES east/north of the region origin.
#[derive(Debug, Clone)]
pub struct OsmRoad {
    /// 0 motorway/trunk, 1 primary, 2 secondary, 3 tertiary/residential,
    /// 4 service, 5 footway/path/cycleway/pedestrian.
    pub class: u8,
    pub name: Option<Box<str>>,
    pub points: Vec<(f32, f32)>,
    /// Precomputed bounds (min_e, min_n, max_e, max_n) for viewport culling.
    pub bounds: (f32, f32, f32, f32),
    /// The tagged width in metres, 0 when untagged (the class width applies).
    pub width_m: f32,
    /// A bridge or tunnel leaves the ground alone (no road bed under it).
    pub bridge: bool,
    pub tunnel: bool,
}

/// One building footprint ring, metres east/north; height 0.0 = UNKNOWN
/// (not "flat"), so the renderer substitutes a class default.
///
/// The ring is open: the closing point is NOT repeated, so a consumer joins
/// the last point back to the first itself.
#[derive(Debug, Clone)]
pub struct OsmBuilding {
    pub height_m: f32,
    pub ring: Vec<(f32, f32)>,
    pub bounds: (f32, f32, f32, f32),
}

/// Water polygon kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    /// Tidal water assembled from OSM coastline ways: at SEA level (the
    /// global ocean shell is the water surface).
    Sea,
    /// Lake / pond / river surface (OSM natural=water, waterway=riverbank):
    /// sits at its OWN level, derived from the terrain around its shore.
    Inland,
    /// An inner LAND ring (island) inside a Sea or Inland ring. Renderers
    /// paint land over water here.
    Island,
}

/// One water polygon, metres east/north. Ring convention matches buildings:
/// open ring, closing point not repeated.
#[derive(Debug, Clone)]
pub struct OsmWater {
    pub kind: WaterKind,
    pub name: Option<Box<str>>,
    pub ring: Vec<(f32, f32)>,
    pub bounds: (f32, f32, f32, f32),
    /// The water's surface as a radius from the planet centre (metres), when
    /// known: an Inland sheet is drawn there. None falls back to the lowest
    /// shore point of the ring (a ring clipped to a tile border must never use
    /// that: its border runs across the lake bed).
    pub surface_r: Option<f64>,
}

/// A set of OSM features around an origin (a named place, or a group of map tiles).
#[derive(Debug, Clone)]
pub struct OsmRegion {
    pub name: String,
    /// Bounding-box CENTRE latitude in degrees. All region metres are
    /// measured from here.
    pub origin_lat: f64,
    /// Bounding-box CENTRE longitude in degrees.
    pub origin_lon: f64,
    pub half_east_m: f32,
    pub half_north_m: f32,
    pub roads: Vec<OsmRoad>,
    pub buildings: Vec<OsmBuilding>,
    /// Water polygons: see `WaterKind`.
    pub water: Vec<OsmWater>,
}

/// The features of the level-12 tiles `tiles` of `pack`, in metres around (`origin_lat`,
/// `origin_lon`) by this module's projection (f64 from OSM's integers, narrowed last). A lake's
/// or river's sheet stands at `surface_r(feature)` when the caller knows its surface.
#[allow(clippy::too_many_arguments)]
pub fn region_from_pack(
    pack: &crate::map_pack::Pack,
    tiles: &[crate::planet_chunks::PatchId],
    name: &str,
    origin_lat: f64,
    origin_lon: f64,
    half_east_m: f32,
    half_north_m: f32,
    surface_r: &dyn Fn(u64) -> Option<f64>,
) -> OsmRegion {
    use crate::map_pack::WaterRingKind;
    let m = |p: &(i32, i32)| -> (f32, f32) {
        let (e, n) = latlon_to_region_meters(origin_lat, origin_lon, crate::map_tiles::decode_e7(p.0), crate::map_tiles::decode_e7(p.1));
        (e as f32, n as f32)
    };
    let mut r = OsmRegion { name: name.into(), origin_lat, origin_lon, half_east_m, half_north_m, roads: Vec::new(), buildings: Vec::new(), water: Vec::new() };
    for t in tiles {
        for w in pack.ways(t.path).unwrap_or_default() {
            if w.class > 5 {
                continue;
            }
            let points: Vec<(f32, f32)> = w.points.iter().map(m).collect();
            // A way raised over the ground without a bridge tag (`layer` above 0) counts as one,
            // the rule the road beds use (map_beds), so no ribbon lies on a bed it does not have.
            r.roads.push(OsmRoad {
                class: w.class,
                name: w.name.map(Into::into),
                bounds: bounds_of(&points),
                points,
                width_m: w.width_m,
                bridge: w.bridge || w.layer > 0,
                tunnel: w.tunnel,
            });
        }
        for b in pack.buildings(t.path).unwrap_or_default() {
            let ring: Vec<(f32, f32)> = b.ring.iter().map(m).collect();
            r.buildings.push(OsmBuilding { height_m: b.height_m, bounds: bounds_of(&ring), ring });
        }
        for w in pack.water(t.path).unwrap_or_default() {
            let kind = match w.kind {
                WaterRingKind::Sea => WaterKind::Sea,
                WaterRingKind::Lake | WaterRingKind::River => WaterKind::Inland,
                WaterRingKind::Island => WaterKind::Island,
            };
            let ring: Vec<(f32, f32)> = w.points.iter().map(m).collect();
            let surface_r = (kind == WaterKind::Inland).then(|| surface_r(w.feature)).flatten();
            r.water.push(OsmWater { kind, name: w.name.map(Into::into), bounds: bounds_of(&ring), ring, surface_r });
        }
    }
    r
}

/// Axis-aligned bounds (min_e, min_n, max_e, max_n) of a point run.
fn bounds_of(points: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(e, n) in points {
        b.0 = b.0.min(e);
        b.1 = b.1.min(n);
        b.2 = b.2.max(e);
        b.3 = b.3.max(n);
    }
    b
}

// ─────────────────────── The projection contract (f64) ──────────────────────
//
// The local metric frame every OsmRegion uses:
//     east_m  = (lon - origin_lon) * cos(origin_lat_rad) * 111320.0
//     north_m = (lat - origin_lat) * 110540.0
// and its exact inverse. It is only a frame, never a measurement: features go
// in from lat/lon and come back out to lat/lon through the SAME two numbers,
// so the frame's 0.6% north scale at mid latitudes never moves anything on
// the ground (the retired region fetcher wrote its files in it).

/// Metres per degree of LONGITUDE at the equator (2*pi*R/360 for the WGS84
/// equatorial radius 6378137 m), scaled by cos(origin_lat) by the caller.
pub const M_PER_DEG_LON_EQUATOR: f64 = 111320.0;

/// Metres per degree of LATITUDE (a fixed near-equatorial WGS84 value, part
/// of the contract; the fetcher does not use a per-latitude series).
pub const M_PER_DEG_LAT: f64 = 110540.0;

/// Smallest cos(origin_lat) the projection will divide by. At |lat| > ~89.99
/// degrees a degree of longitude collapses and the flat projection is
/// meaningless; clamping keeps the arithmetic finite instead of returning
/// infinities into the mesher.
const MIN_COS_LAT: f64 = 1e-9;

/// Region metres east/north -> (latitude, longitude) in DEGREES.
///
/// The exact inverse of the fetcher's forward projection, using literally the
/// two contract constants. All f64: at 6371 km a 1e-7 degree slip is ~1 cm,
/// and f32 degrees would only resolve ~1 m near this magnitude.
pub fn region_meters_to_latlon(
    origin_lat: f64,
    origin_lon: f64,
    e_m: f64,
    n_m: f64,
) -> (f64, f64) {
    let cos_lat = origin_lat.to_radians().cos();
    let m_per_deg_lon = (M_PER_DEG_LON_EQUATOR * cos_lat).abs().max(MIN_COS_LAT);
    let lat = origin_lat + n_m / M_PER_DEG_LAT;
    let lon = origin_lon + e_m / m_per_deg_lon;
    (lat, lon)
}

/// (latitude, longitude) in DEGREES -> region metres east/north.
///
/// The forward direction of the same contract. Not needed by the extruder
/// (which only ever goes metres -> world), but it closes the round trip the
/// unit tests assert, and a later point-in-footprint collider needs it to map
/// a player position back into region metres.
pub fn latlon_to_region_meters(
    origin_lat: f64,
    origin_lon: f64,
    lat_deg: f64,
    lon_deg: f64,
) -> (f64, f64) {
    let cos_lat = origin_lat.to_radians().cos();
    let m_per_deg_lon = (M_PER_DEG_LON_EQUATOR * cos_lat).abs().max(MIN_COS_LAT);
    let e_m = (lon_deg - origin_lon) * m_per_deg_lon;
    let n_m = (lat_deg - origin_lat) * M_PER_DEG_LAT;
    (e_m, n_m)
}

/// (latitude, longitude) in DEGREES -> unit direction in the body's
/// UNROTATED local frame.
///
/// f64 twin of `planet_heightmap::latlon_to_dir`, with the SAME handedness:
/// +Y is the north pole and east is -z, so a viewer outside the sphere with
/// north up sees the continents unmirrored. The
/// `latlon_to_dir_f64_matches_f32_twin` test locks the two together; changing
/// the handedness in one without the other silently mirrors either the
/// terrain sampler or the region geometry.
///
/// f64 is not optional here: an f32 unit vector quantizes ground position at
/// ~0.4 to 0.8 m at Earth radius, which is the same defect class that drew
/// the v0.1010 terrain ripples. A building corner placed with that error
/// would visibly wander against the ground.
pub fn latlon_to_dir_f64(lat_deg: f64, lon_deg: f64) -> DVec3 {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let cl = lat.cos();
    DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin())
}

/// Unit direction in the body's UNROTATED local frame -> (latitude,
/// longitude) in DEGREES. The exact inverse of `latlon_to_dir_f64`.
///
/// Delegates to `planet_heightmap::dir_to_latlon_deg_f64` on purpose: the
/// handedness convention (+Y north, east = -z) lives in ONE place, so this
/// module cannot drift away from the terrain sampler. Kept under a local
/// name because the region pipeline reads better in region terms, and the
/// `latlon_dir_f64_round_trips` test locks it against the forward direction
/// here as well.
///
/// Longitude comes back in (-180, 180]; latitude is undefined at the exact
/// poles (the longitude of a pole is arbitrary), which no region ever uses.
#[inline]
pub fn dir_to_latlon_f64(dir: DVec3) -> (f64, f64) {
    crate::planet_heightmap::dir_to_latlon_deg_f64(dir)
}

// ────────────────────────── Ear-clipping triangulator ───────────────────────

/// Twice the signed area of a ring in the (east, north) plane. Positive means
/// counter-clockwise when viewed from +up (the local radial), because
/// east x north = up in this frame.
fn signed_area2(ring: &[(f32, f32)]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut acc = 0.0f64;
    for i in 0..n {
        let (ax, ay) = ring[i];
        let (bx, by) = ring[(i + 1) % n];
        acc += ax as f64 * by as f64 - bx as f64 * ay as f64;
    }
    acc
}

/// Cross product of (b - a) x (c - a) in f64. Positive = left turn = convex
/// for a counter-clockwise ring.
fn cross_at(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f64 {
    let (ax, ay) = (a.0 as f64, a.1 as f64);
    let (bx, by) = (b.0 as f64, b.1 as f64);
    let (cx, cy) = (c.0 as f64, c.1 as f64);
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Is `p` inside (or on) triangle a-b-c? Barycentric sign test in f64. The
/// boundary counts as inside, which is the conservative choice for an ear
/// test: it refuses an ear that a stray vertex touches.
fn point_in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let d1 = cross_at(a, b, p);
    let d2 = cross_at(b, c, p);
    let d3 = cross_at(c, a, p);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// A ring whose doubled area is under this is treated as degenerate. Real
/// building footprints are tens of square metres; anything at 1e-9 m^2 is a
/// collinear run or a zero-area artefact, not a polygon.
const MIN_RING_AREA2: f64 = 1e-9;

/// Ear-clip a simple polygon ring into triangles.
///
/// `ring` is OPEN (the closing point is not repeated), in region metres.
/// Returns index triples into `ring`, ALWAYS wound counter-clockwise in the
/// (east, north) plane regardless of the input winding, so a consumer can
/// hand them straight to a front-face-CCW pipeline with the local radial as
/// the surface normal.
///
/// Returns `None` and never panics for input this cannot handle:
/// fewer than 3 points, a degenerate (zero-area / fully collinear) ring, or
/// a ring where no valid ear remains, which is the signature of a
/// self-intersecting OSM footprint. The mesher counts those instead of
/// crashing, because bad rings do survive the fetcher.
///
/// O(n^2). Typical footprints are ~12 points and the worst in
/// a downtown district is well under a hundred, so the quadratic term is
/// nothing next to the per-vertex terrain sampling.
pub fn triangulate_ring(ring: &[(f32, f32)]) -> Option<Vec<u32>> {
    let n = ring.len();
    if n < 3 {
        return None;
    }
    for &(e, nn) in ring {
        if !e.is_finite() || !nn.is_finite() {
            return None;
        }
    }
    let area2 = signed_area2(ring);
    if !area2.is_finite() || area2.abs() < MIN_RING_AREA2 {
        return None;
    }

    // Work on an index list normalized to counter-clockwise, so "convex"
    // is always "positive cross" below and the emitted triangles inherit
    // the CCW winding in the ORIGINAL coordinate space.
    let mut idx: Vec<u32> = (0..n as u32).collect();
    if area2 < 0.0 {
        idx.reverse();
    }

    let mut out: Vec<u32> = Vec::with_capacity((n - 2) * 3);
    while idx.len() > 3 {
        let m = idx.len();
        // Reflex flags for the CURRENT polygon. Only reflex vertices can sit
        // inside a candidate ear of a simple polygon, so testing just those
        // is both faster and more forgiving of collinear runs (a collinear
        // vertex is neither convex nor reflex and blocks nothing).
        let reflex: Vec<bool> = (0..m)
            .map(|k| {
                let a = ring[idx[(k + m - 1) % m] as usize];
                let b = ring[idx[k] as usize];
                let c = ring[idx[(k + 1) % m] as usize];
                cross_at(a, b, c) < 0.0
            })
            .collect();

        let mut clipped = None;
        for k in 0..m {
            let ia = idx[(k + m - 1) % m];
            let ib = idx[k];
            let ic = idx[(k + 1) % m];
            let (a, b, c) = (ring[ia as usize], ring[ib as usize], ring[ic as usize]);
            // Strictly convex only: a collinear vertex would emit a
            // zero-area triangle and teaches the mesher nothing.
            if cross_at(a, b, c) <= 0.0 {
                continue;
            }
            let mut blocked = false;
            for (j, &jv) in idx.iter().enumerate() {
                if !reflex[j] || jv == ia || jv == ib || jv == ic {
                    continue;
                }
                if point_in_triangle(ring[jv as usize], a, b, c) {
                    blocked = true;
                    break;
                }
            }
            if !blocked {
                out.extend_from_slice(&[ia, ib, ic]);
                clipped = Some(k);
                break;
            }
        }

        match clipped {
            Some(k) => {
                idx.remove(k);
            }
            // No ear anywhere: the ring is not simple. Skip it rather than
            // emitting garbage geometry or looping forever.
            None => return None,
        }
    }
    out.extend_from_slice(&[idx[0], idx[1], idx[2]]);
    Some(out)
}

// ─────────────────────────── Extrusion mesher ───────────────────────────────

/// RUNG 2'S buried skirt under every wall, kept only for the bed gate's red run
/// (`HUMANITY_GROUND_BEDS=0`). Since the building pads (whole-earth-maps.md
/// 6.2) the ground under a footprint IS its floor, and a coarse patch that
/// cannot draw the pad meets the wall's plinth instead, which reaches down to
/// the natural ground ([`building_levels`]).
#[cfg(feature = "native")]
pub const WALL_SKIRT_M: f64 = 2.5;

/// Longest segment of a carriageway's ribbon on its bed before resampling
/// splits it. The bed's profile bends at vertical curves: a residential bed
/// (levels.ron: a 15% grade, a 40 m window) turns at most from +15% to -15%
/// over its window, 0.75% a metre, and a chord of length L sags c L^2 / 8
/// under it: 3.4 cm at 6 m, inside the 4 cm decal offset (15 m sagged 21 cm).
/// Primary and secondary beds bend less. Each point costs two ground lookups,
/// so a longer segment is a cheaper district (whole-earth-maps.md, rung 2b).
#[cfg(feature = "native")]
pub const ROAD_BED_SEGMENT_M: f32 = 6.0;

/// A service road's or footway's ribbon on its bed (sharper beds: a service
/// road's 18% over 24 m sags 3 cm at 4 m).
#[cfg(feature = "native")]
pub const FOOT_BED_SEGMENT_M: f32 = 4.0;

/// Height used when OSM gives none (`height_m == 0.0` means UNKNOWN, not
/// flat). Two storeys is the honest guess for an untagged footprint.
#[cfg(feature = "native")]
pub const DEFAULT_BUILDING_HEIGHT_M: f64 = 6.0;

/// Longest road segment before resampling splits it. Below this a ribbon
/// follows the fine relief instead of chording straight through a hill.
#[cfg(feature = "native")]
pub const ROAD_MAX_SEGMENT_M: f32 = 15.0;

/// Total carriageway width in metres for a road class (0 motorway .. 5 foot).
/// Ungated (pure math): the mesher widths its ribbons with this AND the
/// map ground's built-mask rasterizer (map_ground) strokes road cells with it, and the
/// rasterizer compiles in every feature set.
pub fn road_full_width_m(class: u8) -> f32 {
    match class {
        0 => 14.0, // motorway/trunk
        1 => 10.0, // primary
        2 => 8.0,  // secondary
        3 => 6.5,  // tertiary/residential/unclassified
        4 => 4.0,  // service
        _ => 2.0,  // footway/path/cycleway/pedestrian
    }
}

/// RUNG 2'S lift: how far a road surface floated above the drawn ground to
/// clear the ground's own bumps across it. Kept for the bed gate's red run and
/// for bridges until their deck mesh (deferred). Class-ordered so footways
/// never fight vehicular surfaces where they overlap.
#[cfg(feature = "native")]
pub fn road_lift_m(class: u8) -> f64 {
    if class >= 5 {
        0.12
    } else {
        0.18
    }
}

/// How far a ribbon lies above its road bed: a depth-test separation from the
/// terrain triangles drawn in the same plane under it (they meet the bed to
/// the millimetre at the finest patches), never a cover for a ground that
/// disagrees. Class-ordered like the old lift, so a footway crossing a
/// carriageway never fights it.
#[cfg(feature = "native")]
pub fn road_decal_m(class: u8) -> f64 {
    if class >= 5 {
        0.02
    } else {
        0.04
    }
}

/// Half a road's carriageway: its tagged width, else its class's. The road
/// bed (map_beds) uses the same rule, so the ribbon is exactly as wide as the
/// flat bed's carriageway.
pub fn road_half_width_m(width_m: f32, class: u8) -> f32 {
    if width_m > 0.0 {
        width_m * 0.5
    } else {
        road_full_width_m(class) * 0.5
    }
}

/// The ground the mesher stands geometry on (the caller's oracles) and the
/// rules it follows.
#[cfg(feature = "native")]
pub struct MeshGround<'a> {
    /// The finest drawn ground as a radius (metres from the body centre), road
    /// beds and building pads included. THE SEA CLAMP IS THE CALLER'S JOB (see
    /// [`build_region_meshes`]).
    pub elev: &'a dyn Fn(DVec3) -> f64,
    /// The natural ground, no bed or pad (the land layer), as a radius: how
    /// far down a building's plinth reaches, to meet a patch too coarse to
    /// draw its pad.
    pub natural: &'a dyn Fn(DVec3) -> f64,
    /// Road beds and building pads are in the ground (always, in play). False:
    /// rung 2's rules (the lifts, the centre sample and the skirt), for the bed
    /// gate's red run (`HUMANITY_GROUND_BEDS=0`).
    pub beds: bool,
}

#[cfg(feature = "native")]
impl<'a> MeshGround<'a> {
    /// One oracle for both, beds on (flat test ground).
    pub fn flat(elev: &'a dyn Fn(DVec3) -> f64) -> Self {
        MeshGround { elev, natural: elev, beds: true }
    }
}

/// THE RIBBON'S RADIUS at one edge point of a road: the rule the mesher draws
/// by and the bed probe judges. On a bed the ribbon lies on the drawn ground
/// at that edge (the bed is flat across, so both edges agree, and where the
/// shore band tilts a waterfront street the ribbon tilts with it) plus the
/// decal offset. Rung 2 (beds off) and bridges: the centreline's ground plus
/// the old lift, both edges alike.
#[cfg(feature = "native")]
pub fn road_edge_radius(g: &MeshGround, centre: DVec3, edge: DVec3, class: u8, bridge: bool) -> f64 {
    if g.beds && !bridge {
        (g.elev)(edge) + road_decal_m(class)
    } else {
        (g.elev)(centre) + road_lift_m(class)
    }
}

/// THE BUILDING'S LEVELS for a footprint whose corners are `dirs`, `h` metres
/// tall: (wall bottom, floor, roof) as radii. The rule the mesher draws by and
/// the bed probe judges. With pads: the floor is the ground at the corners
/// (their median: the pad holds them all at its floor), and the walls reach
/// down to the lowest of that and the natural ground, a plinth that meets a
/// coarse patch drawing no pad. Rung 2 (beds off): the floor was the lowest
/// corner and the walls carried a 2.5 m buried skirt.
#[cfg(feature = "native")]
pub fn building_levels(g: &MeshGround, dirs: &[DVec3], h: f64) -> Option<(f64, f64, f64)> {
    let elev: Vec<f64> = dirs.iter().map(|d| (g.elev)(*d)).collect();
    if elev.is_empty() || elev.iter().any(|r| !r.is_finite()) {
        return None;
    }
    let min = elev.iter().copied().fold(f64::MAX, f64::min);
    if g.beds {
        let mut s = elev;
        s.sort_by(f64::total_cmp);
        let floor = s[s.len() / 2];
        let natural = dirs.iter().map(|d| (g.natural)(*d)).filter(|r| r.is_finite()).fold(f64::MAX, f64::min);
        Some((min.min(natural), floor, floor + h))
    } else {
        Some((min - WALL_SKIRT_M, min, min + h))
    }
}

/// Which material a batch wants. One mesh per kind, because `Vertex` has no
/// colour channel and a `RenderObject` carries exactly one material.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionMeshKind {
    /// Under 12 m: houses, low-rise, and every unknown-height footprint.
    BuildingLow,
    /// 12 m to 50 m: mid-rise concrete.
    BuildingMid,
    /// 50 m and up: towers, read as glass.
    BuildingHigh,
    /// Vehicular carriageways (classes 0 to 4).
    RoadAsphalt,
    /// Footways, paths, cycleways (class 5).
    RoadFoot,
    /// INLAND water surfaces (lakes, ponds): a flat sheet at each polygon's
    /// own level (the lowest shore point). Sea polygons are NOT meshed here;
    /// the terrain carve lets the global ocean shell be the sea surface.
    WaterInland,
}

#[cfg(feature = "native")]
impl RegionMeshKind {
    /// Fixed emission order, also the slot order of the internal buffers.
    pub const ALL: [RegionMeshKind; 6] = [
        RegionMeshKind::BuildingLow,
        RegionMeshKind::BuildingMid,
        RegionMeshKind::BuildingHigh,
        RegionMeshKind::RoadAsphalt,
        RegionMeshKind::RoadFoot,
        RegionMeshKind::WaterInland,
    ];

    fn slot(self) -> usize {
        match self {
            RegionMeshKind::BuildingLow => 0,
            RegionMeshKind::BuildingMid => 1,
            RegionMeshKind::BuildingHigh => 2,
            RegionMeshKind::RoadAsphalt => 3,
            RegionMeshKind::RoadFoot => 4,
            RegionMeshKind::WaterInland => 5,
        }
    }
}

/// One material batch: raw vertex/index arrays ready for
/// `Mesh::from_vertices`. Indices are LOCAL to this batch.
#[cfg(feature = "native")]
pub struct RegionMeshClass {
    pub kind: RegionMeshKind,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// The whole region, meshed.
///
/// ## Anchor semantics (the contract with the draw side)
///
/// `anchor_local` is `origin_dir * elev(origin_dir)`: the drawn-ground point
/// directly under the region's bounding-box centre, in metres from the body
/// centre, in the planet's UNROTATED local frame (the same frame
/// `planet_chunks` selects patches in, and the frame `latlon_to_dir_f64`
/// returns directions in). It is f64 and must stay f64.
///
/// Every vertex position in every class is an f32 offset from that anchor, in
/// that same unrotated frame. So the per-frame draw is exactly the classic
/// patch pattern:
///
/// ```text
///   position = (render_off + rot_d * anchor_local).as_vec3()   // narrow LAST
///   rotation = rot_d.as_quat()                                 // planet spin
///   scale    = 1.0
/// ```
///
/// where `rot_d = DQuat::from_rotation_y(spin_f64)` and
/// `render_off = rel_earth_m - ship_world_pos`. Do NOT pre-rotate the
/// vertices; the spin belongs in the object rotation, exactly like patches.
#[cfg(feature = "native")]
pub struct RegionMeshes {
    pub classes: Vec<RegionMeshClass>,
    /// Footprints the ear clipper refused (self-intersecting OSM rings).
    /// Surfaced for diagnostics; a nonzero count is data quality, not a bug.
    pub skipped_rings: usize,
    /// See the struct docs: planet-unrotated, f64, metres from body centre.
    pub anchor_local: DVec3,
}

#[cfg(feature = "native")]
#[derive(Default)]
struct ClassBuf {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

#[cfg(feature = "native")]
impl ClassBuf {
    /// Emit one quad as two triangles with a shared flat normal. Winding
    /// matches `ship::rooms::emit_quad`: bl, br, tr, tl is front-facing when
    /// the geometric normal points at the viewer (pipeline is FrontFace::Ccw
    /// with back-face culling).
    #[allow(clippy::too_many_arguments)]
    fn quad(
        &mut self,
        bl: DVec3,
        br: DVec3,
        tr: DVec3,
        tl: DVec3,
        normal: glam::Vec3,
        u0: f32,
        u1: f32,
        v0: f32,
        v1: f32,
    ) {
        let base = self.vertices.len() as u32;
        let n = [normal.x, normal.y, normal.z];
        for (p, uv) in [
            (bl, [u0, v0]),
            (br, [u1, v0]),
            (tr, [u1, v1]),
            (tl, [u0, v1]),
        ] {
            let p = p.as_vec3();
            self.vertices.push(Vertex { position: [p.x, p.y, p.z], normal: n, uv });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Split a polyline so no segment exceeds `max_seg_m`, dropping consecutive
/// duplicate points first. Returns the resampled points in order; the
/// endpoints are preserved exactly.
#[cfg(feature = "native")]
fn resample_polyline(points: &[(f32, f32)], max_seg_m: f32) -> Vec<(f32, f32)> {
    let mut clean: Vec<(f32, f32)> = Vec::with_capacity(points.len());
    for &p in points {
        if !p.0.is_finite() || !p.1.is_finite() {
            continue;
        }
        if clean.last().is_none_or(|&q| seg_len(q, p) > 1e-4) {
            clean.push(p);
        }
    }
    if clean.len() < 2 {
        return clean;
    }
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(clean.len() * 2);
    out.push(clean[0]);
    for w in clean.windows(2) {
        let (a, b) = (w[0], w[1]);
        let steps = ((seg_len(a, b) / max_seg_m).ceil() as usize).max(1);
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

#[cfg(feature = "native")]
fn seg_len(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    (dx * dx + dy * dy).sqrt()
}

/// Unit normal to the LEFT of a -> b in the (east, north) plane, or `None`
/// for a zero-length segment.
#[cfg(feature = "native")]
fn left_normal(a: (f32, f32), b: (f32, f32)) -> Option<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return None;
    }
    Some((-dy / len, dx / len))
}

/// Mitered left offset direction at point `k` of a polyline, already scaled
/// so that offsetting by `dir * half_width` keeps the ribbon's constant width
/// through the corner. Spikes at hairpins are clamped (a 4x miter limit).
#[cfg(feature = "native")]
fn miter_left(points: &[(f32, f32)], k: usize) -> (f32, f32) {
    let prev = if k > 0 { left_normal(points[k - 1], points[k]) } else { None };
    let next = if k + 1 < points.len() { left_normal(points[k], points[k + 1]) } else { None };
    match (prev, next) {
        (Some(a), Some(b)) => {
            let (mx, my) = (a.0 + b.0, a.1 + b.1);
            let len = (mx * mx + my * my).sqrt();
            if len < 1e-4 {
                return b; // ~180 degree switchback: no sane miter, butt it
            }
            let m = (mx / len, my / len);
            // 1 / cos(half angle), clamped so a hairpin cannot fire a spike
            // across the map.
            let d = (m.0 * b.0 + m.1 * b.1).max(0.25);
            (m.0 / d, m.1 / d)
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => (0.0, 1.0),
    }
}

/// Raise a whole region into 3D geometry on the drawn planet surface.
///
/// `ground.elev(unit_dir) -> radius_metres` is the caller's drawn-ground
/// oracle: in the app, the one ground formula at the finest depth (the map
/// packs' ground with its road beds and building pads), exactly the surface
/// the player's walk clamp uses. THE SEA CLAMP IS THE CALLER'S JOB:
/// waterfront footprints sample Puget Sound bathymetry, so the closure should
/// clamp its result to at least sea level before returning it. This function
/// does not know the planet definition and deliberately does not guess.
///
/// Cost is dominated by `elev`: one call per footprint vertex and per ribbon
/// edge point (roughly 100k for a downtown district). Run it off the main
/// thread.
#[cfg(feature = "native")]
pub fn build_region_meshes(region: &OsmRegion, ground: &MeshGround) -> RegionMeshes {
    let elev = ground.elev;
    let origin_dir = latlon_to_dir_f64(region.origin_lat, region.origin_lon);
    let anchor_local = origin_dir * elev(origin_dir);

    let mut bufs: [ClassBuf; 6] = Default::default();
    let mut skipped_rings = 0usize;

    // Region metres -> unit direction in the planet's unrotated frame.
    let to_dir = |e: f32, n: f32| -> DVec3 {
        let (lat, lon) =
            region_meters_to_latlon(region.origin_lat, region.origin_lon, e as f64, n as f64);
        latlon_to_dir_f64(lat, lon)
    };

    // ── Buildings: walls + ear-clipped roof cap ──────────────────────────
    let mut dirs: Vec<DVec3> = Vec::new();
    for b in &region.buildings {
        let Some(tris) = triangulate_ring(&b.ring) else {
            skipped_rings += 1;
            continue;
        };

        // Per-vertex direction; the wall bottom, floor and roof by the one
        // rule the bed probe also judges (`building_levels`).
        dirs.clear();
        dirs.reserve(b.ring.len());
        for &(e, n) in &b.ring {
            dirs.push(to_dir(e, n));
        }
        let h = if b.height_m.is_finite() && b.height_m > 0.0 {
            b.height_m as f64
        } else {
            DEFAULT_BUILDING_HEIGHT_M
        };
        let Some((bottom_r, base_r, top_r)) = building_levels(ground, &dirs, h) else {
            skipped_rings += 1;
            continue;
        };
        let kind = if h < 12.0 {
            RegionMeshKind::BuildingLow
        } else if h < 50.0 {
            RegionMeshKind::BuildingMid
        } else {
            RegionMeshKind::BuildingHigh
        };
        let buf = &mut bufs[kind.slot()];

        // Traverse counter-clockwise so cross(edge, up) points OUTWARD
        // (east x north = up in this frame, so a CCW ring's edge normals
        // face away from the interior).
        let ccw: Vec<usize> = if signed_area2(&b.ring) >= 0.0 {
            (0..b.ring.len()).collect()
        } else {
            (0..b.ring.len()).rev().collect()
        };

        let v_top = ((top_r - bottom_r) / 3.0) as f32;
        let mut u_acc = 0.0f32;
        let m = ccw.len();
        for k in 0..m {
            let ia = ccw[k];
            let ib = ccw[(k + 1) % m];
            let len = seg_len(b.ring[ia], b.ring[ib]);
            if len < 1e-4 {
                continue; // duplicate ring point, no wall to draw
            }
            let (da, db) = (dirs[ia], dirs[ib]);
            let up = (da + db).normalize_or_zero();
            let edge = (db - da) * base_r;
            let normal = edge.cross(up).normalize_or_zero();
            if normal == DVec3::ZERO || up == DVec3::ZERO {
                continue;
            }
            buf.quad(
                da * bottom_r - anchor_local,
                db * bottom_r - anchor_local,
                db * top_r - anchor_local,
                da * top_r - anchor_local,
                normal.as_vec3(),
                u_acc / 3.0,
                (u_acc + len) / 3.0,
                0.0,
                v_top,
            );
            u_acc += len;
        }

        // Roof: one vertex per ring point at the top radius, normal = local
        // radial up, planar UVs off the region metres. The clipper's triples
        // are CCW in (east, north), which is front-facing seen from above.
        let roof_base = buf.vertices.len() as u32;
        for (i, &(e, n)) in b.ring.iter().enumerate() {
            let d = dirs[i];
            let p = (d * top_r - anchor_local).as_vec3();
            let nrm = d.as_vec3();
            buf.vertices.push(Vertex {
                position: [p.x, p.y, p.z],
                normal: [nrm.x, nrm.y, nrm.z],
                uv: [e / 3.0, n / 3.0],
            });
        }
        for t in &tris {
            buf.indices.push(roof_base + t);
        }
    }

    // ── Roads: mitered ribbons on their beds ─────────────────────────────
    for road in &region.roads {
        // A tunnel leaves the ground alone and its road is hidden (the
        // under-surface view is maps-zoom-to-anything increment 7).
        if ground.beds && road.tunnel {
            continue;
        }
        let on_bed = ground.beds && !road.bridge;
        let seg = if !on_bed {
            ROAD_MAX_SEGMENT_M
        } else if road.class >= 4 {
            FOOT_BED_SEGMENT_M
        } else {
            ROAD_BED_SEGMENT_M
        };
        let pts = resample_polyline(&road.points, seg);
        if pts.len() < 2 {
            continue;
        }
        // Rung 2 drew every road at its class width; the bed's carriageway is
        // the tagged width where there is one.
        let half_w = if ground.beds { road_half_width_m(road.width_m, road.class) } else { road_full_width_m(road.class) * 0.5 };
        let kind = if road.class >= 5 { RegionMeshKind::RoadFoot } else { RegionMeshKind::RoadAsphalt };
        let buf = &mut bufs[kind.slot()];

        let base_v = buf.vertices.len() as u32;
        let mut u_acc = 0.0f32;
        for k in 0..pts.len() {
            let p = pts[k];
            if k > 0 {
                u_acc += seg_len(pts[k - 1], p);
            }
            let off = miter_left(&pts, k);
            let left = (p.0 + off.0 * half_w, p.1 + off.1 * half_w);
            let right = (p.0 - off.0 * half_w, p.1 - off.1 * half_w);

            // Each edge by `road_edge_radius`: on a bed, the drawn ground at
            // that edge (flat across, so both agree) plus the decal offset;
            // rung 2 and bridges, the centre's ground plus the old lift.
            let dc = to_dir(p.0, p.1);
            let u = u_acc / 5.0;
            for (side, v) in [(left, 0.0f32), (right, 1.0f32)] {
                let d = to_dir(side.0, side.1);
                let r = road_edge_radius(ground, dc, d, road.class, road.bridge);
                let pos = (d * r - anchor_local).as_vec3();
                let nrm = d.as_vec3();
                buf.vertices.push(Vertex {
                    position: [pos.x, pos.y, pos.z],
                    normal: [nrm.x, nrm.y, nrm.z],
                    uv: [u, v],
                });
            }
        }
        for k in 0..pts.len() as u32 - 1 {
            let i = base_v + k * 2;
            buf.indices
                .extend_from_slice(&[i, i + 1, i + 3, i, i + 3, i + 2]);
        }
    }

    // ── Inland water: flat sheets at each polygon's own level ────────────
    // Sea polygons are deliberately NOT meshed: the map ground's shore band
    // (map_ground) draws the seabed below sea level and the global ocean
    // shell becomes the water surface, waves and all. A lake sits ABOVE sea
    // level, where no shell exists, so it gets its own flat sheet at its
    // surface (`surface_r`, the map ground's lake level), else at the LOWEST
    // shore point of its ring (water cannot stand above its lowest shore).
    // Island rings need no geometry here: the ground inside them stands
    // above the surface and pokes through the sheet on its own.
    for w in &region.water {
        if w.kind != WaterKind::Inland {
            continue;
        }
        let Some(tris) = triangulate_ring(&w.ring) else {
            skipped_rings += 1;
            continue;
        };
        let mut level_r = f64::MAX;
        dirs.clear();
        dirs.reserve(w.ring.len());
        for &(e, n) in &w.ring {
            let d = to_dir(e, n);
            let r = elev(d);
            if r < level_r {
                level_r = r;
            }
            dirs.push(d);
        }
        if !level_r.is_finite() {
            skipped_rings += 1;
            continue;
        }
        // A hair above the shore-min ground so the sheet never z-fights the
        // terrain right at its own waterline (the known surface: exactly there).
        let level_r = w.surface_r.unwrap_or(level_r + 0.35);
        let buf = &mut bufs[RegionMeshKind::WaterInland.slot()];
        let base = buf.vertices.len() as u32;
        for (i, &(e, n)) in w.ring.iter().enumerate() {
            let d = dirs[i];
            let p = (d * level_r - anchor_local).as_vec3();
            let nrm = d.as_vec3();
            buf.vertices.push(Vertex {
                position: [p.x, p.y, p.z],
                normal: [nrm.x, nrm.y, nrm.z],
                uv: [e / 12.0, n / 12.0],
            });
        }
        for t in &tris {
            buf.indices.push(base + t);
        }
    }

    // Emit only the classes that got geometry, in the fixed ALL order.
    let mut classes = Vec::new();
    for (buf, kind) in bufs.into_iter().zip(RegionMeshKind::ALL) {
        if !buf.indices.is_empty() {
            classes.push(RegionMeshClass { kind, vertices: buf.vertices, indices: buf.indices });
        }
    }

    RegionMeshes { classes, skipped_rings, anchor_local }
}

// ─────────────────────────────────── Tests ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Projection contract ──────────────────────────────────────────────

    /// The f64 direction twin must agree with the f32 `latlon_to_dir` the
    /// terrain sampler and the probe rig use. If these ever disagree the
    /// region geometry lands somewhere the heightmap is not.
    #[test]
    fn latlon_to_dir_f64_matches_f32_twin() {
        let mut worst = 0.0f64;
        let mut lat = -85.0f64;
        while lat <= 85.0 {
            let mut lon = -180.0f64;
            while lon <= 180.0 {
                let a = latlon_to_dir_f64(lat, lon);
                let b = crate::planet_heightmap::latlon_to_dir(lat as f32, lon as f32);
                for (x, y) in [(a.x, b.x as f64), (a.y, b.y as f64), (a.z, b.z as f64)] {
                    worst = worst.max((x - y).abs());
                }
                lon += 17.0;
            }
            lat += 11.0;
        }
        assert!(worst < 1e-6, "f64/f32 latlon_to_dir handedness drift: {worst:e}");
    }

    /// Region metres -> lat/lon -> unit dir -> lat/lon -> region metres must
    /// come back where it started. This is the whole placement chain, so a
    /// sign flip or a wrong constant anywhere in it shows up here.
    #[test]
    fn meters_geodetic_round_trip_holds_to_a_tenth_of_a_millimetre() {
        let (olat, olon) = (47.6165f64, -122.342f64);
        let mut worst = 0.0f64;
        let mut e = -1400.0f64;
        while e <= 1400.0 {
            let mut n = -1400.0f64;
            while n <= 1400.0 {
                let (lat, lon) = region_meters_to_latlon(olat, olon, e, n);
                let dir = latlon_to_dir_f64(lat, lon);
                let (lat2, lon2) = dir_to_latlon_f64(dir);
                let (e2, n2) = latlon_to_region_meters(olat, olon, lat2, lon2);
                worst = worst.max((e2 - e).abs()).max((n2 - n).abs());
                n += 175.0;
            }
            e += 175.0;
        }
        assert!(worst < 1e-4, "region round-trip drift {worst:e} m, want < 1e-4");
    }

    /// `latlon_to_dir_f64` and `dir_to_latlon_f64` are exact inverses. The
    /// engine's residency check goes the OTHER way (camera direction -> where
    /// am I in region metres), so a one-sided guarantee would not be enough.
    #[test]
    fn latlon_dir_f64_round_trips() {
        let mut worst_lat = 0.0f64;
        let mut worst_lon = 0.0f64;
        let mut lat = -85.0f64;
        while lat <= 85.0 {
            let mut lon = -179.0f64;
            while lon <= 179.0 {
                let (lat2, lon2) = dir_to_latlon_f64(latlon_to_dir_f64(lat, lon));
                worst_lat = worst_lat.max((lat2 - lat).abs());
                worst_lon = worst_lon.max((lon2 - lon).abs());
                lon += 13.0;
            }
            lat += 7.0;
        }
        assert!(worst_lat < 1e-9, "latitude round-trip drift {worst_lat:e} deg");
        assert!(worst_lon < 1e-9, "longitude round-trip drift {worst_lon:e} deg");
    }

    /// `region_meters_to_latlon` and `latlon_to_region_meters` are exact
    /// inverses of each other, independent of the sphere step above.
    #[test]
    fn region_meters_latlon_are_exact_inverses() {
        let (olat, olon) = (47.6165f64, -122.342f64);
        let mut worst = 0.0f64;
        for &(e, n) in &[
            (0.0f64, 0.0f64),
            (1450.6, 1671.2),
            (-1450.6, -1671.2),
            (250.0, -900.0),
            (-33.3, 77.7),
        ] {
            let (lat, lon) = region_meters_to_latlon(olat, olon, e, n);
            let (e2, n2) = latlon_to_region_meters(olat, olon, lat, lon);
            worst = worst.max((e2 - e).abs()).max((n2 - n).abs());
        }
        assert!(worst < 1e-9, "projection inverse drift {worst:e} m");
    }

    /// The two contract constants, asserted literally. If someone "fixes"
    /// the latitude constant to the true WGS84 meridian arc here without
    /// changing every place that converts into this frame, features shift
    /// by ~0.6% against the terrain.
    #[test]
    fn projection_uses_the_frame_constants() {
        assert_eq!(M_PER_DEG_LON_EQUATOR, 111320.0);
        assert_eq!(M_PER_DEG_LAT, 110540.0);
        let (lat, lon) = region_meters_to_latlon(0.0, 0.0, 111320.0, 110540.0);
        assert!((lat - 1.0).abs() < 1e-12, "one degree north = 110540 m");
        assert!((lon - 1.0).abs() < 1e-12, "one degree east at the equator = 111320 m");
    }

    // ── Ear clipper ──────────────────────────────────────────────────────

    /// Sum of the signed areas of the emitted triangles. Equals the polygon
    /// area exactly when the triangulation covers it once with consistent
    /// winding.
    fn tri_area_sum(ring: &[(f32, f32)], tris: &[u32]) -> f64 {
        tris.chunks(3)
            .map(|t| {
                let (a, b, c) =
                    (ring[t[0] as usize], ring[t[1] as usize], ring[t[2] as usize]);
                cross_at(a, b, c) * 0.5
            })
            .sum()
    }

    #[test]
    fn ear_clip_convex_square() {
        let ring = [(0.0f32, 0.0f32), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let tris = triangulate_ring(&ring).expect("a square triangulates");
        assert_eq!(tris.len(), 6, "n-2 = 2 triangles");
        assert!((tri_area_sum(&ring, &tris) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn ear_clip_l_shape_is_concave_correct() {
        // Area 12: a 4x2 foot plus a 2x2 upright.
        let ring = [(0.0f32, 0.0f32), (4.0, 0.0), (4.0, 2.0), (2.0, 2.0), (2.0, 4.0), (0.0, 4.0)];
        let tris = triangulate_ring(&ring).expect("an L triangulates");
        assert_eq!(tris.len(), (ring.len() - 2) * 3);
        assert!(
            (tri_area_sum(&ring, &tris) - 12.0).abs() < 1e-9,
            "triangles must cover the concave polygon exactly once"
        );
        // No triangle may be wound clockwise: the roof cap relies on CCW.
        for t in tris.chunks(3) {
            let (a, b, c) = (ring[t[0] as usize], ring[t[1] as usize], ring[t[2] as usize]);
            assert!(cross_at(a, b, c) > 0.0, "every emitted triangle is CCW");
        }
    }

    #[test]
    fn ear_clip_survives_collinear_runs() {
        // A square with an extra midpoint on the bottom edge: OSM does this
        // constantly where a way was split at a junction.
        let ring = [(0.0f32, 0.0f32), (2.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let tris = triangulate_ring(&ring).expect("collinear runs still clip");
        assert_eq!(tris.len(), (ring.len() - 2) * 3);
        assert!((tri_area_sum(&ring, &tris) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn ear_clip_normalizes_both_windings() {
        let ccw = [(0.0f32, 0.0f32), (4.0, 0.0), (4.0, 2.0), (2.0, 2.0), (2.0, 4.0), (0.0, 4.0)];
        let mut cw = ccw;
        cw.reverse();
        let a = triangulate_ring(&ccw).expect("ccw clips");
        let b = triangulate_ring(&cw).expect("cw clips");
        assert_eq!(a.len(), b.len());
        // Both must come out POSITIVE (counter-clockwise) despite the
        // opposite input winding.
        assert!((tri_area_sum(&ccw, &a) - 12.0).abs() < 1e-9);
        assert!((tri_area_sum(&cw, &b) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn ear_clip_rejects_degenerate_rings() {
        assert!(triangulate_ring(&[]).is_none(), "empty");
        assert!(triangulate_ring(&[(0.0, 0.0), (1.0, 0.0)]).is_none(), "two points");
        assert!(
            triangulate_ring(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]).is_none(),
            "fully collinear ring has no area"
        );
        assert!(
            triangulate_ring(&[(0.0, 0.0), (1.0, 0.0), (1.0, f32::NAN)]).is_none(),
            "non-finite coordinates are refused, never panicked on"
        );
    }

    #[test]
    fn ear_clip_never_overproduces_on_a_bowtie() {
        // Self-intersecting footprints DO survive the fetcher. Either answer
        // is acceptable (skip it, or produce a bounded triangulation), but
        // it must never hang, panic, or invent triangles.
        let symmetric = [(0.0f32, 0.0f32), (4.0, 0.0), (0.0, 3.0), (4.0, 3.0)];
        let asymmetric = [(0.0f32, 0.0f32), (6.0, 0.0), (1.0, 5.0), (5.0, 5.0)];
        for ring in [&symmetric[..], &asymmetric[..]] {
            match triangulate_ring(ring) {
                None => {}
                Some(tris) => {
                    assert_eq!(tris.len() % 3, 0);
                    assert!(
                        tris.len() / 3 <= ring.len() - 2,
                        "a triangulation can never exceed n-2 triangles"
                    );
                    assert!(tris.iter().all(|&i| (i as usize) < ring.len()));
                }
            }
        }
    }

    // ── The shipped file ─────────────────────────────────────────────────


    // ── Mesher (native only: it speaks vertex::Vertex) ───────────

    #[cfg(feature = "native")]
    const FLAT_R: f64 = 6_371_000.0;

    #[cfg(feature = "native")]
    fn flat_region(buildings: Vec<OsmBuilding>, roads: Vec<OsmRoad>) -> OsmRegion {
        OsmRegion {
            name: "Flat".into(),
            origin_lat: 47.6,
            origin_lon: -122.3,
            half_east_m: 1000.0,
            half_north_m: 1000.0,
            roads,
            buildings,
            water: Vec::new(),
        }
    }

    #[cfg(feature = "native")]
    fn building(height_m: f32, ring: Vec<(f32, f32)>) -> OsmBuilding {
        let bounds = bounds_of(&ring);
        OsmBuilding { height_m, ring, bounds }
    }

    /// Radius (metres from the body centre) of a meshed vertex, reconstructed
    /// through the anchor exactly the way the draw path does.
    #[cfg(feature = "native")]
    fn vertex_radius(anchor: DVec3, v: &Vertex) -> f64 {
        (anchor + DVec3::new(v.position[0] as f64, v.position[1] as f64, v.position[2] as f64))
            .length()
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_builds_a_lake_sheet_at_shore_min_and_skips_sea() {
        let mut region = flat_region(vec![], vec![]);
        region.water = vec![
            // Sea polygon: must produce NO geometry (the shore band + ocean shell
            // own the sea surface).
            OsmWater {
                kind: WaterKind::Sea,
                name: None,
                ring: vec![(-900.0, -900.0), (-500.0, -900.0), (-700.0, -600.0)],
                bounds: (-900.0, -900.0, -500.0, -600.0),
                surface_r: None,
            },
            // A square lake on ground that dips: the sheet must sit at the
            // LOWEST shore point (+0.35 m anti-z-fight lift), not the mean.
            OsmWater {
                kind: WaterKind::Inland,
                name: Some("Test Lake".into()),
                ring: vec![(400.0, 400.0), (500.0, 400.0), (500.0, 500.0), (400.0, 500.0)],
                bounds: (400.0, 400.0, 500.0, 500.0),
                surface_r: None,
            },
            // An island ring: also no geometry (terrain pokes through).
            OsmWater {
                kind: WaterKind::Island,
                name: None,
                ring: vec![(440.0, 440.0), (460.0, 440.0), (450.0, 460.0)],
                bounds: (440.0, 440.0, 460.0, 460.0),
                surface_r: None,
            },
        ];
        // Ground dips by 3 m toward the lake's north-east corner.
        let dip_dir = latlon_to_dir_f64(
            region_meters_to_latlon(region.origin_lat, region.origin_lon, 500.0, 500.0).0,
            region_meters_to_latlon(region.origin_lat, region.origin_lon, 500.0, 500.0).1,
        );
        let elev = move |d: DVec3| -> f64 {
            if d.dot(dip_dir) > 0.999_999_999_9 { FLAT_R - 3.0 } else { FLAT_R }
        };
        let out = build_region_meshes(&region, &MeshGround::flat(&elev));

        assert_eq!(out.classes.len(), 1, "sea + island make no geometry, the lake does");
        let c = &out.classes[0];
        assert_eq!(c.kind, RegionMeshKind::WaterInland);
        assert_eq!(c.vertices.len(), 4, "one flat sheet vertex per ring point");
        assert_eq!(c.indices.len(), 2 * 3, "a quad ear-clips into two triangles");
        let want = FLAT_R - 3.0 + 0.35;
        for v in &c.vertices {
            let r = vertex_radius(out.anchor_local, v);
            assert!(
                (r - want).abs() < 1e-2,
                "lake sheet at {} relative to flat ground, expected {}",
                r - FLAT_R,
                want - FLAT_R
            );
        }
    }

    // The gate this test was missing is why the relay test target stopped
    // compiling: the duplicated `#[cfg]` above was the same attribute applied
    // twice to the lake test instead of once to each, so this one reached for
    // `flat_region`/`FLAT_R`/`build_region_meshes` in a build where all three
    // are configured out.
    #[cfg(feature = "native")]
    #[test]
    fn mesher_extrudes_a_square_building_on_flat_ground() {
        let h = 20.0f32; // 12..50 -> mid-rise class
        let region = flat_region(
            vec![building(h, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])],
            vec![],
        );
        let out = build_region_meshes(&region, &MeshGround::flat(&|_| FLAT_R));

        assert_eq!(out.skipped_rings, 0);
        assert_eq!(out.classes.len(), 1, "one building, no roads = one batch");
        let c = &out.classes[0];
        assert_eq!(c.kind, RegionMeshKind::BuildingMid, "20 m is mid-rise");

        // 4 walls (4 verts each, flat normals) + 4 shared roof verts.
        assert_eq!(c.vertices.len(), 4 * 4 + 4);
        // 4 walls * 2 triangles + 2 roof triangles.
        assert_eq!(c.indices.len(), (4 * 2 + 2) * 3);
        assert!(c.indices.iter().all(|&i| (i as usize) < c.vertices.len()));

        // The anchor is the ground under the region origin.
        assert!(
            (out.anchor_local.length() - FLAT_R).abs() < 1e-6,
            "anchor sits on the drawn ground"
        );

        // Offsets stay tiny: that is the whole point of anchoring.
        for v in &c.vertices {
            let m = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!(m < 60.0, "anchor-relative offset {m} m is out of range for a 10 m footprint");
        }

        // The last 4 vertices are the roof cap: exactly `h` above the ground.
        for v in &c.vertices[16..] {
            let r = vertex_radius(out.anchor_local, v);
            assert!(
                (r - (FLAT_R + h as f64)).abs() < 1e-2,
                "roof at {} above ground, expected {h}",
                r - FLAT_R
            );
        }
        // Wall bottoms stand on the ground (no buried skirt since the pads); wall tops match the roof.
        // Seen red 2026-11-09 with rung 2's rule kept: "the wall starts on the ground, -2.50 m off it".
        let mut lowest = f64::MAX;
        let mut highest = f64::MIN;
        for v in &c.vertices[..16] {
            let r = vertex_radius(out.anchor_local, v);
            lowest = lowest.min(r);
            highest = highest.max(r);
        }
        assert!((lowest - FLAT_R).abs() < 1e-2, "the wall starts on the ground, {} m off it", lowest - FLAT_R);
        assert!((highest - (FLAT_R + h as f64)).abs() < 1e-2, "wall top meets the roof");

        // Wall normals are horizontal (perpendicular to the local radial).
        let up = out.anchor_local.normalize();
        for v in &c.vertices[..16] {
            let n = DVec3::new(v.normal[0] as f64, v.normal[1] as f64, v.normal[2] as f64);
            assert!(n.dot(up).abs() < 1e-3, "wall normal must be horizontal");
        }
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_defaults_unknown_heights_and_classes_by_height() {
        let region = flat_region(
            vec![
                building(0.0, vec![(0.0, 0.0), (8.0, 0.0), (8.0, 8.0)]), // unknown -> low
                building(30.0, vec![(20.0, 0.0), (28.0, 0.0), (28.0, 8.0)]), // mid
                building(80.0, vec![(40.0, 0.0), (48.0, 0.0), (48.0, 8.0)]), // high
            ],
            vec![],
        );
        let out = build_region_meshes(&region, &MeshGround::flat(&|_| FLAT_R));
        let kinds: Vec<_> = out.classes.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                RegionMeshKind::BuildingLow,
                RegionMeshKind::BuildingMid,
                RegionMeshKind::BuildingHigh
            ],
            "classes come out in the fixed ALL order, empty ones omitted"
        );
        // The unknown-height footprint got the 6 m default, not a flat slab.
        let low = &out.classes[0];
        let top = low
            .vertices
            .iter()
            .map(|v| vertex_radius(out.anchor_local, v))
            .fold(f64::MIN, f64::max);
        assert!(
            (top - (FLAT_R + DEFAULT_BUILDING_HEIGHT_M)).abs() < 1e-2,
            "unknown height must extrude to the {DEFAULT_BUILDING_HEIGHT_M} m default"
        );
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_skips_bad_rings_instead_of_panicking() {
        // A fully collinear "footprint": the clipper refuses it.
        let region = flat_region(
            vec![building(10.0, vec![(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)])],
            vec![],
        );
        let out = build_region_meshes(&region, &MeshGround::flat(&|_| FLAT_R));
        assert_eq!(out.skipped_rings, 1);
        assert!(out.classes.is_empty(), "nothing drawn for a refused ring");
    }

    /// A building on a slope (whole-earth-maps.md 6.2): the pad holds every corner at the floor,
    /// the plinth reaches down to the lowest natural ground, and the roof stands `h` over the
    /// floor; with the beds off (the red run) it is rung 2's rule, the lowest corner and a 2.5 m
    /// buried skirt. Seen red 2026-11-09 with `building_levels` keeping rung 2's rule in both
    /// branches: "the plinth stops at the natural ground: bottom -2.500 m, natural low -0.499 m".
    #[cfg(feature = "native")]
    #[test]
    fn a_building_stands_on_its_pad_with_a_plinth_to_the_natural_ground() {
        let region = flat_region(vec![building(6.0, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])], vec![]);
        let origin = latlon_to_dir_f64(region.origin_lat, region.origin_lon);
        let east = DVec3::Y.cross(origin).normalize();
        // The natural ground falls 1 m in 10 m to the east; the pad (the drawn ground) holds the
        // footprint at its middle, +0.0 m.
        let natural = move |d: DVec3| FLAT_R - 0.1 * ((d - origin).dot(east) * FLAT_R - 5.0);
        let pad = |_: DVec3| FLAT_R;
        let g = MeshGround { elev: &pad, natural: &natural, beds: true };
        let dirs: Vec<DVec3> = region.buildings[0].ring.iter().map(|&(e, n)| {
            let (la, lo) = region_meters_to_latlon(region.origin_lat, region.origin_lon, e as f64, n as f64);
            latlon_to_dir_f64(la, lo)
        }).collect();
        let (bottom, floor, top) = building_levels(&g, &dirs, 6.0).unwrap();
        assert!((floor - FLAT_R).abs() < 1e-6 && (top - floor - 6.0).abs() < 1e-6, "the floor is the pad and the roof 6 m over it");
        let low = dirs.iter().map(|d| natural(*d)).fold(f64::MAX, f64::min);
        assert!((bottom - low).abs() < 1e-3, "the plinth stops at the natural ground: bottom {:.3} m, natural low {:.3} m", bottom - FLAT_R, low - FLAT_R);
        let red = MeshGround { elev: &natural, natural: &natural, beds: false };
        let (rb, rf, _) = building_levels(&red, &dirs, 6.0).unwrap();
        assert!((rf - low).abs() < 1e-3 && (rb - (low - WALL_SKIRT_M)).abs() < 1e-3, "beds off is rung 2's lowest corner and skirt");
    }

    /// On a bed each ribbon edge lies on the drawn ground at that edge plus the decal offset, so a
    /// ground that tilts across (the shore band holding a waterfront street) tilts the ribbon with
    /// it; a bridge keeps rung 2's centre sample and lift (its deck is deferred), and a tunnel is
    /// hidden. Seen red 2026-11-09 with `road_edge_radius` sampling the centre on beds too: "the
    /// left edge lies 0.37 m over its own ground, not 0.04".
    #[cfg(feature = "native")]
    #[test]
    fn ribbon_edges_lie_on_their_own_ground() {
        let origin = latlon_to_dir_f64(47.6, -122.3);
        let east = DVec3::Y.cross(origin).normalize();
        let tilt = move |d: DVec3| FLAT_R + 0.1 * (d - origin).dot(east) * FLAT_R;
        let g = MeshGround { elev: &tilt, natural: &tilt, beds: true };
        let left = (origin - east * (3.25 / FLAT_R)).normalize();
        let r = road_edge_radius(&g, origin, left, 3, false);
        assert!((r - tilt(left) - road_decal_m(3)).abs() < 1e-6, "the left edge lies {:.2} m over its own ground, not {:.2}", r - tilt(left), road_decal_m(3));
        let rb = road_edge_radius(&g, origin, left, 3, true);
        assert!((rb - tilt(origin) - road_lift_m(3)).abs() < 1e-6, "a bridge keeps the centre sample and the lift");
        let points = vec![(0.0f32, 0.0f32), (0.0, 40.0)];
        let bounds = bounds_of(&points);
        let tunnel = OsmRoad { class: 3, name: None, points, bounds, width_m: 0.0, bridge: false, tunnel: true };
        let out = build_region_meshes(&flat_region(vec![], vec![tunnel]), &MeshGround::flat(&|_| FLAT_R));
        assert!(out.classes.is_empty(), "a tunnel's road is hidden");
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_drapes_a_road_ribbon() {
        // 40 m north then 30 m east. Resampling at a carriageway bed's 6 m gives
        // 1 + ceil(40/6) + ceil(30/6) = 1 + 7 + 5 = 13 points.
        let points = vec![(0.0f32, 0.0f32), (0.0, 40.0), (30.0, 40.0)];
        let bounds = bounds_of(&points);
        let road = OsmRoad { class: 3, name: None, points: points.clone(), bounds, width_m: 0.0, bridge: false, tunnel: false };
        let region = flat_region(vec![], vec![road]);
        let out = build_region_meshes(&region, &MeshGround::flat(&|_| FLAT_R));

        let expected_pts = resample_polyline(&points, ROAD_BED_SEGMENT_M).len();
        assert_eq!(expected_pts, 13, "resampling is deterministic");

        assert_eq!(out.classes.len(), 1);
        let c = &out.classes[0];
        assert_eq!(c.kind, RegionMeshKind::RoadAsphalt, "class 3 is vehicular");
        assert_eq!(c.vertices.len(), expected_pts * 2, "two ribbon edges per point");
        assert_eq!(c.indices.len(), (expected_pts - 1) * 6, "two triangles per segment");
        assert!(c.indices.iter().all(|&i| (i as usize) < c.vertices.len()));

        // Ribbon width at the start equals the class width.
        let a = glam::Vec3::from(c.vertices[0].position);
        let b = glam::Vec3::from(c.vertices[1].position);
        assert!(
            ((a - b).length() - road_full_width_m(3)).abs() < 0.02,
            "ribbon width {} m, expected {}",
            (a - b).length(),
            road_full_width_m(3)
        );

        // Every road vertex lies one class decal offset over its bed (no lift since the beds).
        for v in &c.vertices {
            let r = vertex_radius(out.anchor_local, v);
            assert!(
                (r - (FLAT_R + road_decal_m(3))).abs() < 1e-2,
                "road over its bed by {} m, expected {}",
                r - FLAT_R,
                road_decal_m(3)
            );
        }
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_separates_footways_from_carriageways() {
        let mk = |class: u8, e: f32| {
            let points = vec![(e, 0.0f32), (e, 30.0f32)];
            let bounds = bounds_of(&points);
            OsmRoad { class, name: None, points, bounds, width_m: 0.0, bridge: false, tunnel: false }
        };
        let region = flat_region(vec![], vec![mk(1, 0.0), mk(5, 50.0)]);
        let out = build_region_meshes(&region, &MeshGround::flat(&|_| FLAT_R));
        let kinds: Vec<_> = out.classes.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, vec![RegionMeshKind::RoadAsphalt, RegionMeshKind::RoadFoot]);
        // Footways sit lower so they never fight a road they cross.
        assert!(road_decal_m(5) < road_decal_m(1));
        assert!(road_lift_m(5) < road_lift_m(1));
    }

    #[cfg(feature = "native")]
    #[test]
    fn resampling_never_exceeds_the_segment_cap() {
        let pts = vec![(0.0f32, 0.0f32), (0.0, 100.0), (7.0, 100.0), (7.0, 100.0)];
        let out = resample_polyline(&pts, ROAD_MAX_SEGMENT_M);
        assert!(out.len() >= 3);
        for w in out.windows(2) {
            assert!(
                seg_len(w[0], w[1]) <= ROAD_MAX_SEGMENT_M + 1e-3,
                "segment {} m exceeds the {ROAD_MAX_SEGMENT_M} m cap",
                seg_len(w[0], w[1])
            );
        }
        assert_eq!(*out.first().unwrap(), pts[0], "endpoints are preserved exactly");
        assert_eq!(*out.last().unwrap(), (7.0, 100.0));
    }

}
