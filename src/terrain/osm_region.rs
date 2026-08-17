//! OSM regions: the HOSMREG2 reader, the fetcher's projection contract, a
//! polygon ear clipper, and the 3D extrusion mesher.
//!
//! One region file (`data/maps/regions/*.bin`) is a bounding box of REAL
//! OpenStreetMap roads and building footprints, produced at development time
//! by `scripts/fetch-osm-region.mjs`. That script's header block is the
//! format CONTRACT; this module is its only Rust reader, shared by two
//! consumers so they cannot drift:
//!
//! - the Maps page's 2D Planet view (`src/gui/pages/cosmos.rs`), which draws
//!   the region as a slippy-map style plan; and
//! - the in-world 3D extruder (`src/engine/region_meshes.rs`), which raises
//!   the same footprints as real geometry on the chunked-LOD Earth.
//!
//! ODbL: everything a region file carries is "Data (c) OpenStreetMap
//! contributors, ODbL 1.0". Any surface that DRAWS a region must show that
//! credit. That is a licence obligation, not a courtesy.
//!
//! ## Precision discipline (the f64 rule, CLAUDE.md)
//!
//! Region coordinates are metres east/north of the region origin, stored as
//! f32 in the file, which is EXACT at these magnitudes (a couple of km, so
//! ulp is well under a millimetre). Everything after that stays f64 until the
//! very last step:
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
//! Relay-safe on purpose: the parser, the projection, and the ear clipper are
//! pure std + glam, so they compile and unit-test under
//! `--features relay --no-default-features`. Only the mesher (which speaks
//! `renderer::mesh::Vertex`) is gated behind `native`.

use glam::DVec3;

#[cfg(feature = "native")]
use crate::renderer::mesh::Vertex;

// ───────────────────────────── Region data types ────────────────────────────

/// One road from a HOSMREG2 region file. Coordinates are METRES east/north
/// of the region origin (see `scripts/fetch-osm-region.mjs` for the spec).
#[derive(Debug, Clone)]
pub struct OsmRoad {
    /// 0 motorway/trunk, 1 primary, 2 secondary, 3 tertiary/residential,
    /// 4 service, 5 footway/path/cycleway/pedestrian.
    pub class: u8,
    pub name: Option<Box<str>>,
    pub points: Vec<(f32, f32)>,
    /// Precomputed bounds (min_e, min_n, max_e, max_n) for viewport culling.
    pub bounds: (f32, f32, f32, f32),
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

/// Water polygon kind (HOSMREG2 water records).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    /// Tidal water assembled from OSM coastline ways: render/carve at SEA
    /// level (the global ocean shell is the water surface).
    Sea,
    /// Lake / pond / river surface (OSM natural=water, waterway=riverbank):
    /// sits at its OWN level, derived from the terrain around its shore.
    Inland,
    /// An inner LAND ring (island) belonging to the nearest preceding Sea or
    /// Inland record. Renderers paint land over water here; the carve
    /// rasterizer un-marks these cells.
    Island,
}

/// One water polygon from a HOSMREG2 region file, metres east/north.
/// Ring convention matches buildings: open ring, closing point not repeated.
#[derive(Debug, Clone)]
pub struct OsmWater {
    pub kind: WaterKind,
    pub name: Option<Box<str>>,
    pub ring: Vec<(f32, f32)>,
    pub bounds: (f32, f32, f32, f32),
}

/// One installed OSM region (`data/maps/regions/*.bin`).
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
    /// Water polygons (HOSMREG2): sea first, then inland records each
    /// followed immediately by its islands. See `WaterKind`.
    pub water: Vec<OsmWater>,
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

/// Cap on speculative `with_capacity` from a file-supplied count. The counts
/// are u32, so a corrupt header could otherwise ask for a 250 GB reserve
/// before the walk ever notices the file is short. Parse RESULTS are
/// unaffected: the record walk still reads exactly what the header claims
/// and still fails if the bytes are not there.
const CAPACITY_SANITY_CAP: usize = 1 << 20;

/// Parse one HOSMREG2 file. `None` on any structural problem.
///
/// The walk is strictly front to back with no padding and no alignment, and
/// it must land EXACTLY on EOF: trailing bytes mean the file disagrees with
/// its own header, which is corruption, not a tolerable extension.
///
/// v2 (water polygons) replaced v1 outright per the no-compat-debt rule:
/// both shipped region files were refetched the same commit, and nothing
/// else ever wrote v1.
pub fn parse_region(bytes: &[u8]) -> Option<OsmRegion> {
    if bytes.len() < 20 || &bytes[0..8] != b"HOSMREG2" {
        return None;
    }
    let road_count = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    let building_count = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    let water_count = u32::from_le_bytes(bytes[16..20].try_into().ok()?) as usize;
    let mut off = 20usize;
    let take = |off: &mut usize, n: usize| -> Option<&[u8]> {
        let s = bytes.get(*off..*off + n)?;
        *off += n;
        Some(s)
    };
    let f64le = |b: &[u8]| f64::from_le_bytes(b.try_into().unwrap());
    let f32le = |b: &[u8]| f32::from_le_bytes(b.try_into().unwrap());
    let origin_lat = f64le(take(&mut off, 8)?);
    let origin_lon = f64le(take(&mut off, 8)?);
    let half_east_m = f32le(take(&mut off, 4)?);
    let half_north_m = f32le(take(&mut off, 4)?);
    let name_len = take(&mut off, 1)?[0] as usize;
    let name = std::str::from_utf8(take(&mut off, name_len)?).ok()?.to_string();

    let mut roads = Vec::with_capacity(road_count.min(CAPACITY_SANITY_CAP));
    for _ in 0..road_count {
        let class = take(&mut off, 1)?[0];
        let rn_len = take(&mut off, 1)?[0] as usize;
        let rname = if rn_len > 0 {
            Some(std::str::from_utf8(take(&mut off, rn_len)?).ok()?.into())
        } else {
            None
        };
        let pc = u16::from_le_bytes(take(&mut off, 2)?.try_into().ok()?) as usize;
        if pc < 2 {
            return None;
        }
        let mut points = Vec::with_capacity(pc);
        for _ in 0..pc {
            let e = f32le(take(&mut off, 4)?);
            let n = f32le(take(&mut off, 4)?);
            points.push((e, n));
        }
        let bounds = bounds_of(&points);
        roads.push(OsmRoad { class, name: rname, points, bounds });
    }
    let mut buildings = Vec::with_capacity(building_count.min(CAPACITY_SANITY_CAP));
    for _ in 0..building_count {
        let height_m = f32le(take(&mut off, 4)?);
        let pc = u16::from_le_bytes(take(&mut off, 2)?.try_into().ok()?) as usize;
        if pc < 3 {
            return None;
        }
        let mut ring = Vec::with_capacity(pc);
        for _ in 0..pc {
            let e = f32le(take(&mut off, 4)?);
            let n = f32le(take(&mut off, 4)?);
            ring.push((e, n));
        }
        let bounds = bounds_of(&ring);
        buildings.push(OsmBuilding { height_m, ring, bounds });
    }
    let mut water = Vec::with_capacity(water_count.min(CAPACITY_SANITY_CAP));
    for _ in 0..water_count {
        let kind = match take(&mut off, 1)?[0] {
            0 => WaterKind::Sea,
            1 => WaterKind::Inland,
            2 => WaterKind::Island,
            _ => return None,
        };
        let wn_len = take(&mut off, 1)?[0] as usize;
        let wname = if wn_len > 0 {
            Some(std::str::from_utf8(take(&mut off, wn_len)?).ok()?.into())
        } else {
            None
        };
        let pc = u16::from_le_bytes(take(&mut off, 2)?.try_into().ok()?) as usize;
        if pc < 3 {
            return None;
        }
        let mut ring = Vec::with_capacity(pc);
        for _ in 0..pc {
            let e = f32le(take(&mut off, 4)?);
            let n = f32le(take(&mut off, 4)?);
            ring.push((e, n));
        }
        let bounds = bounds_of(&ring);
        water.push(OsmWater { kind, name: wname, ring, bounds });
    }
    if off != bytes.len() {
        return None; // trailing garbage = corrupt file
    }
    Some(OsmRegion {
        name,
        origin_lat,
        origin_lon,
        half_east_m,
        half_north_m,
        roads,
        buildings,
        water,
    })
}

// ───────────────────── Region DEM (HOSDEM1, real elevation) ─────────────────

/// Real elevation for a region's area, fetched once at dev time from the
/// public AWS Terrain Tiles by `scripts/fetch-region-dem.mjs` (see its
/// header for the byte-exact spec and the source attribution). ~13 m grid at
/// Puget Sound latitudes vs the ~460 m global base heightmap: this is what
/// gives a region real coastline gradients instead of a cliff at the carve
/// edge (operator field report 2026-08-17).
#[derive(Debug, Clone)]
pub struct RegionDem {
    pub width: u32,
    pub height: u32,
    /// Latitude of ROW 0's sample points (degrees). Rows go SOUTH from here.
    pub lat_north: f64,
    /// Longitude of column 0's sample points (degrees).
    pub lon_west: f64,
    /// Degrees between row sample points (positive; subtract going south).
    pub lat_step: f64,
    /// Degrees between column sample points (positive going east).
    pub lon_step: f64,
    pub min_m: f32,
    pub max_m: f32,
    /// Quantized elevations, row-major from the north row.
    pub samples: Vec<u16>,
}

/// Parse one HOSDEM1 file. Same house rules as `parse_region`: strict
/// front-to-back walk, must land exactly on EOF.
pub fn parse_dem(bytes: &[u8]) -> Option<RegionDem> {
    if bytes.len() < 55 || &bytes[0..7] != b"HOSDEM1" {
        return None;
    }
    let u32le = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let f64le = |o: usize| f64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
    let f32le = |o: usize| f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let width = u32le(7);
    let height = u32le(11);
    let lat_north = f64le(15);
    let lon_west = f64le(23);
    let lat_step = f64le(31);
    let lon_step = f64le(39);
    let min_m = f32le(47);
    let max_m = f32le(51);
    if width == 0 || height == 0 || !(max_m > min_m) || lat_step <= 0.0 || lon_step <= 0.0 {
        return None;
    }
    let n = width as usize * height as usize;
    if n > (1 << 26) {
        return None; // 64M samples = 128 MB: nothing legitimate is that big
    }
    if bytes.len() != 55 + n * 2 {
        return None;
    }
    let samples = bytes[55..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some(RegionDem {
        width,
        height,
        lat_north,
        lon_west,
        lat_step,
        lon_step,
        min_m,
        max_m,
        samples,
    })
}

impl RegionDem {
    /// Bilinear elevation in metres at (lat, lon) degrees, None outside the
    /// grid. Row 0 is NORTH: the row coordinate grows southward.
    pub fn sample_m(&self, lat: f64, lon: f64) -> Option<f32> {
        let fx = (lon - self.lon_west) / self.lon_step;
        let fy = (self.lat_north - lat) / self.lat_step;
        // Half-cell slack on the bounds: a query landing EXACTLY on the
        // boundary row computes fy = 1.0000000000000004 in f64 and a strict
        // test rejects the very corner the caller asked for. Values inside
        // the slack clamp onto the edge sample.
        let max_x = (self.width - 1) as f64;
        let max_y = (self.height - 1) as f64;
        if fx < -0.5 || fy < -0.5 || fx > max_x + 0.5 || fy > max_y + 0.5 {
            return None;
        }
        let fx = fx.clamp(0.0, max_x);
        let fy = fy.clamp(0.0, max_y);
        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(self.width as usize - 1);
        let y1 = (y0 + 1).min(self.height as usize - 1);
        let tx = (fx - x0 as f64) as f32;
        let ty = (fy - y0 as f64) as f32;
        let range = self.max_m - self.min_m;
        let at = |x: usize, y: usize| -> f32 {
            self.min_m + self.samples[y * self.width as usize + x] as f32 / 65535.0 * range
        };
        Some(
            (at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx) * (1.0 - ty)
                + (at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx) * ty,
        )
    }

    /// The grid's coverage in degrees: (lat_south, lat_north, lon_west,
    /// lon_east), for the overlay's edge blend.
    pub fn bounds_deg(&self) -> (f64, f64, f64, f64) {
        (
            self.lat_north - (self.height - 1) as f64 * self.lat_step,
            self.lat_north,
            self.lon_west,
            self.lon_west + (self.width - 1) as f64 * self.lon_step,
        )
    }
}

// ─────────────────────── The projection contract (f64) ──────────────────────
//
// These two constants are the CONTRACT with scripts/fetch-osm-region.mjs.
// The fetcher writes every coordinate as
//     east_m  = (lon - origin_lon) * cos(origin_lat_rad) * 111320.0
//     north_m = (lat - origin_lat) * 110540.0
// so this reader must invert with the SAME two numbers or the geometry drifts
// against the terrain. Do not "improve" them to a proper WGS84 meridian arc
// on this side alone: the fetcher's simplification is uniform across a region
// (it scales the map by ~0.6% at mid latitudes rather than distorting it) and
// the two sides must agree exactly.

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
    crate::terrain::planet_heightmap::dir_to_latlon_deg_f64(dir)
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
/// seattle-center.bin is well under a hundred, so the quadratic term is
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

/// Buried skirt under every wall. Terrain LOD and the sampled ground
/// disagree by a few metres at range, so a wall that started exactly at the
/// footprint's lowest sampled ground would show daylight underneath on a
/// coarser patch. Sinking it hides that without changing the visible height.
#[cfg(feature = "native")]
pub const WALL_SKIRT_M: f64 = 2.5;

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
/// water_carve built-mask rasterizer strokes road cells with it, and the
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

/// How far a road surface floats above the drawn ground. Class-ordered so
/// footways never fight vehicular surfaces where they overlap.
#[cfg(feature = "native")]
pub fn road_lift_m(class: u8) -> f64 {
    if class >= 5 {
        0.12
    } else {
        0.18
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
/// `elev(unit_dir) -> radius_metres` is the caller's drawn-ground oracle: in
/// the app that is `engine::frame_lock::ground_radius_m` closed over the
/// heightmap, the detail noise, and any resident 460 m terrain tile, i.e.
/// exactly the surface the player's walk clamp uses. THE SEA CLAMP IS THE
/// CALLER'S JOB: waterfront footprints sample Puget Sound bathymetry, so the
/// closure should clamp its result to at least sea level plus
/// `ocean_waves::SURFACE_LIFT_M` before returning it. This function does not
/// know the planet definition and deliberately does not guess.
///
/// Cost is dominated by `elev`: one call per footprint vertex and per
/// resampled road point (roughly 40k for seattle-center.bin). Run it off the
/// main thread.
#[cfg(feature = "native")]
pub fn build_region_meshes(region: &OsmRegion, elev: &dyn Fn(DVec3) -> f64) -> RegionMeshes {
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

        // Per-vertex direction and drawn radius. The base is the MINIMUM
        // over the ring: a footprint on a slope must not leave a corner
        // hanging in the air, and the skirt swallows the extra depth.
        dirs.clear();
        dirs.reserve(b.ring.len());
        let mut base_r = f64::MAX;
        for &(e, n) in &b.ring {
            let d = to_dir(e, n);
            let r = elev(d);
            if r < base_r {
                base_r = r;
            }
            dirs.push(d);
        }
        if !base_r.is_finite() {
            skipped_rings += 1;
            continue;
        }

        let h = if b.height_m.is_finite() && b.height_m > 0.0 {
            b.height_m as f64
        } else {
            DEFAULT_BUILDING_HEIGHT_M
        };
        let kind = if h < 12.0 {
            RegionMeshKind::BuildingLow
        } else if h < 50.0 {
            RegionMeshKind::BuildingMid
        } else {
            RegionMeshKind::BuildingHigh
        };
        let buf = &mut bufs[kind.slot()];

        let bottom_r = base_r - WALL_SKIRT_M;
        let top_r = base_r + h;

        // Traverse counter-clockwise so cross(edge, up) points OUTWARD
        // (east x north = up in this frame, so a CCW ring's edge normals
        // face away from the interior).
        let ccw: Vec<usize> = if signed_area2(&b.ring) >= 0.0 {
            (0..b.ring.len()).collect()
        } else {
            (0..b.ring.len()).rev().collect()
        };

        let v_top = ((WALL_SKIRT_M + h) / 3.0) as f32;
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

    // ── Roads: draped, mitered ribbons ───────────────────────────────────
    for road in &region.roads {
        let pts = resample_polyline(&road.points, ROAD_MAX_SEGMENT_M);
        if pts.len() < 2 {
            continue;
        }
        let half_w = road_full_width_m(road.class) * 0.5;
        let lift = road_lift_m(road.class);
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

            // ONE ground sample per centreline point: a carriageway is flat
            // ACROSS its width in the real world, so both edges ride the
            // centre's radius. Sampling the edges separately would twist the
            // ribbon on rough ground for no gain.
            let dc = to_dir(p.0, p.1);
            let r = elev(dc) + lift;
            let u = u_acc / 5.0;
            for (side, v) in [(left, 0.0f32), (right, 1.0f32)] {
                let d = to_dir(side.0, side.1);
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
    // Sea polygons are deliberately NOT meshed: the terrain carve
    // (water_carve) lowers the ground under them below sea level and the
    // global ocean shell becomes the water surface, waves and all. A lake
    // sits ABOVE sea level, where no shell exists, so it gets its own flat
    // sheet at the LOWEST shore point of its ring (water cannot stand above
    // its lowest shore). Island rings need no geometry here: the terrain
    // inside them stays uncarved and pokes through the sheet on its own.
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
        // terrain right at its own waterline.
        let level_r = level_r + 0.35;
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

    // ── Parser ───────────────────────────────────────────────────────────

    /// A minimal but structurally valid HOSMREG2 file: 1 road, 1 building,
    /// 2 water records (a sea polygon and a named lake).
    fn synth_region_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"HOSMREG2");
        b.extend_from_slice(&1u32.to_le_bytes()); // road_count
        b.extend_from_slice(&1u32.to_le_bytes()); // building_count
        b.extend_from_slice(&2u32.to_le_bytes()); // water_count
        b.extend_from_slice(&47.6f64.to_le_bytes()); // origin_lat
        b.extend_from_slice(&(-122.3f64).to_le_bytes()); // origin_lon
        b.extend_from_slice(&1000.0f32.to_le_bytes()); // half_east_m
        b.extend_from_slice(&1200.0f32.to_le_bytes()); // half_north_m
        b.push(4); // name_len
        b.extend_from_slice(b"Test");
        // Road: class 3, named "Main", 2 points.
        b.push(3);
        b.push(4);
        b.extend_from_slice(b"Main");
        b.extend_from_slice(&2u16.to_le_bytes());
        for &(e, n) in &[(0.0f32, 0.0f32), (0.0f32, 40.0f32)] {
            b.extend_from_slice(&e.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
        }
        // Building: 12 m, square ring.
        b.extend_from_slice(&12.0f32.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        for &(e, n) in &[(0.0f32, 0.0f32), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
            b.extend_from_slice(&e.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
        }
        // Water: an unnamed sea triangle, then a named square lake.
        b.push(0); // kind Sea
        b.push(0); // unnamed
        b.extend_from_slice(&3u16.to_le_bytes());
        for &(e, n) in &[(-900.0f32, -900.0f32), (-500.0, -900.0), (-700.0, -600.0)] {
            b.extend_from_slice(&e.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
        }
        b.push(1); // kind Inland
        b.push(9);
        b.extend_from_slice(b"Test Lake");
        b.extend_from_slice(&4u16.to_le_bytes());
        for &(e, n) in &[(400.0f32, 400.0f32), (500.0, 400.0), (500.0, 500.0), (400.0, 500.0)] {
            b.extend_from_slice(&e.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
        }
        b
    }

    #[test]
    fn parses_a_well_formed_region() {
        let r = parse_region(&synth_region_bytes()).expect("valid file parses");
        assert_eq!(r.name, "Test");
        assert_eq!(r.origin_lat, 47.6);
        assert_eq!(r.origin_lon, -122.3);
        assert_eq!(r.half_east_m, 1000.0);
        assert_eq!(r.half_north_m, 1200.0);
        assert_eq!(r.roads.len(), 1);
        assert_eq!(r.roads[0].class, 3);
        assert_eq!(r.roads[0].name.as_deref(), Some("Main"));
        assert_eq!(r.roads[0].bounds, (0.0, 0.0, 0.0, 40.0));
        assert_eq!(r.buildings.len(), 1);
        assert_eq!(r.buildings[0].height_m, 12.0);
        assert_eq!(r.buildings[0].ring.len(), 4);
        assert_eq!(r.buildings[0].bounds, (0.0, 0.0, 10.0, 10.0));
        assert_eq!(r.water.len(), 2);
        assert_eq!(r.water[0].kind, WaterKind::Sea);
        assert!(r.water[0].name.is_none());
        assert_eq!(r.water[0].ring.len(), 3);
        assert_eq!(r.water[1].kind, WaterKind::Inland);
        assert_eq!(r.water[1].name.as_deref(), Some("Test Lake"));
        assert_eq!(r.water[1].bounds, (400.0, 400.0, 500.0, 500.0));
    }

    #[test]
    fn parser_rejects_unknown_water_kind() {
        let mut b = synth_region_bytes();
        // The first water record's kind byte sits right after the building
        // ring. Find it robustly: it is the first of the last two records,
        // whose combined size is fixed (4 + 3*8) + (2 + 9 + 2 + 4*8).
        let sea_rec = 1 + 1 + 2 + 3 * 8;
        let lake_rec = 1 + 1 + 9 + 2 + 4 * 8;
        let kind_off = b.len() - sea_rec - lake_rec;
        assert_eq!(b[kind_off], 0, "offset arithmetic must land on the Sea kind byte");
        b[kind_off] = 7;
        assert!(parse_region(&b).is_none(), "unknown water kind must be refused");
    }

    #[test]
    fn parser_rejects_bad_magic() {
        let mut b = synth_region_bytes();
        b[3] = b'X';
        assert!(parse_region(&b).is_none(), "wrong magic must be refused");
        assert!(parse_region(b"HOSM").is_none(), "shorter than the header");
        assert!(parse_region(&[]).is_none(), "empty input");
    }

    #[test]
    fn parser_rejects_truncated_file() {
        let full = synth_region_bytes();
        // Every prefix short of the whole file must fail, not read garbage.
        for cut in [16, 30, 41, 50, full.len() - 1] {
            assert!(
                parse_region(&full[..cut]).is_none(),
                "a {cut}-byte prefix must be refused"
            );
        }
    }

    #[test]
    fn parser_rejects_trailing_bytes() {
        let mut b = synth_region_bytes();
        b.push(0);
        assert!(
            parse_region(&b).is_none(),
            "the walk must land exactly on EOF; trailing bytes are corruption"
        );
    }

    // ── Region DEM (HOSDEM1) ─────────────────────────────────────────────

    /// A 3x2 synthetic DEM: values laid out row-major from the NORTH row.
    fn synth_dem_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"HOSDEM1");
        b.extend_from_slice(&3u32.to_le_bytes()); // width
        b.extend_from_slice(&2u32.to_le_bytes()); // height
        b.extend_from_slice(&48.0f64.to_le_bytes()); // lat_north
        b.extend_from_slice(&(-123.0f64).to_le_bytes()); // lon_west
        b.extend_from_slice(&0.1f64.to_le_bytes()); // lat_step
        b.extend_from_slice(&0.1f64.to_le_bytes()); // lon_step
        b.extend_from_slice(&0.0f32.to_le_bytes()); // min_m
        b.extend_from_slice(&100.0f32.to_le_bytes()); // max_m
        // North row: 0, 50, 100 m. South row: 100, 50, 0 m.
        for q in [0u16, 32768, 65535, 65535, 32768, 0] {
            b.extend_from_slice(&q.to_le_bytes());
        }
        b
    }

    #[test]
    fn dem_parses_samples_and_keeps_north_up() {
        let d = parse_dem(&synth_dem_bytes()).expect("valid DEM parses");
        assert_eq!((d.width, d.height), (3, 2));
        // Row 0 is NORTH: the north-west corner (lat_north, lon_west) must
        // read the FIRST sample (0 m), and the south-west corner the fourth
        // (100 m). An orientation flip would swap these, which on the real
        // Silverdale file would put the ridge in the inlet.
        let nw = d.sample_m(48.0, -123.0).unwrap();
        let sw = d.sample_m(48.0 - 0.1, -123.0).unwrap();
        assert!(nw < 0.01, "north-west corner must be 0 m, got {nw}");
        assert!((sw - 100.0).abs() < 0.01, "south-west corner must be 100 m, got {sw}");
        // Bilinear midpoint between 0 and 50 on the north row.
        let mid = d.sample_m(48.0, -123.0 + 0.05).unwrap();
        assert!((mid - 25.0).abs() < 0.1, "bilinear midpoint {mid}, want 25");
        // Outside coverage (past the half-cell edge slack): None, never an
        // extrapolation.
        assert!(d.sample_m(48.06, -123.0).is_none());
        assert!(d.sample_m(47.84, -123.0).is_none());
        // Truncation + trailing bytes are refused.
        let full = synth_dem_bytes();
        assert!(parse_dem(&full[..full.len() - 1]).is_none());
        let mut extra = full.clone();
        extra.push(0);
        assert!(parse_dem(&extra).is_none());
    }

    /// The SHIPPED silverdale.dem.bin against known geography, mirroring the
    /// fetcher's own gates so parser and generator lock through the real
    /// file: mid Dyes Inlet at sea level, the SW ridge high. An orientation
    /// or step-sign bug lands the ridge in the inlet and fails loudly.
    #[test]
    fn shipped_silverdale_dem_is_real() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/maps/regions/silverdale.dem.bin"
        ))
        .expect("data/maps/regions/silverdale.dem.bin ships with the repo");
        let d = parse_dem(&bytes).expect("parses to exactly EOF");
        let inlet = d.sample_m(47.6230, -122.6870).expect("inlet is inside coverage");
        assert!(inlet <= 2.0, "mid Dyes Inlet must be at sea level, got {inlet} m");
        let town = d.sample_m(47.6450, -122.6950).expect("town is inside coverage");
        assert!((5.0..=120.0).contains(&town), "central Silverdale terrace, got {town} m");
        // The highest ground is in the SW (toward Green/Gold Mountain).
        let ridge = d.sample_m(47.57184, -122.75220).unwrap_or(0.0);
        assert!(ridge > 150.0, "SW ridge must exceed 150 m, got {ridge} m");
    }

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
                let b = crate::terrain::planet_heightmap::latlon_to_dir(lat as f32, lon as f32);
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
    /// changing scripts/fetch-osm-region.mjs, every shipped region shifts
    /// by ~0.6% against the terrain.
    #[test]
    fn projection_uses_the_fetcher_contract_constants() {
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

    /// Independent check of the SHIPPED seattle-center.bin against known
    /// geography: parser and generator (scripts/fetch-osm-region.mjs) are
    /// locked through the real file, and the file is proven to carry real
    /// OSM data (Pike Street exists, Rainier Square Tower's 259 m height),
    /// not just plausible bytes. Moved here from src/gui/pages/cosmos.rs
    /// when the parser became shared (the 3D extruder reads the same file).
    #[test]
    fn shipped_seattle_region_is_real() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/maps/regions/seattle-center.bin"
        ))
        .expect("data/maps/regions/seattle-center.bin ships with the repo");
        let region = parse_region(&bytes).expect("parses to exactly EOF");
        assert_eq!(region.name, "Seattle Center");
        assert!(region.roads.len() > 5_000, "downtown road count, got {}", region.roads.len());
        assert!(region.buildings.len() > 1_000, "building count, got {}", region.buildings.len());
        assert!(
            region.roads.iter().any(|r| r.name.as_deref() == Some("Pike Street")),
            "Pike Street is in the region"
        );
        // The origin the whole placement chain hangs off.
        assert!((region.origin_lat - 47.6165).abs() < 1e-4, "origin lat {}", region.origin_lat);
        assert!((region.origin_lon + 122.342).abs() < 1e-4, "origin lon {}", region.origin_lon);
        // Every coordinate within the half-spans + the 400 m clip slack.
        let (he, hn) = (region.half_east_m + 400.0, region.half_north_m + 400.0);
        for r in &region.roads {
            assert!(r.bounds.0 >= -he && r.bounds.2 <= he && r.bounds.1 >= -hn && r.bounds.3 <= hn);
        }
        // The tallest building is Rainier Square Tower, 259 m.
        let tallest = region.buildings.iter().map(|b| b.height_m).fold(0.0f32, f32::max);
        assert!((tallest - 259.0).abs() < 1.0, "tallest {tallest} m, expected ~259");
        // v2 water: the Elliott Bay corner assembled from coastline, and
        // Lake Union's south tip as a named multipolygon.
        assert!(
            region.water.iter().any(|w| w.kind == WaterKind::Sea),
            "Elliott Bay sea polygon missing"
        );
        assert!(
            region.water.iter().any(|w| w.name.as_deref() == Some("Lake Union")),
            "Lake Union missing from the water records"
        );
    }

    /// Same real-geography lock for the operator's town: Dyes Inlet must be
    /// SEA (the whole reason the water arc exists: their field report showed
    /// the inlet dry) and Island Lake must be an inland record with its
    /// Clark Island hole following it.
    #[test]
    fn shipped_silverdale_region_has_its_water() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/maps/regions/silverdale.bin"
        ))
        .expect("data/maps/regions/silverdale.bin ships with the repo");
        let region = parse_region(&bytes).expect("parses to exactly EOF");
        assert_eq!(region.name, "Silverdale");
        assert!(region.roads.len() > 5_000, "road count, got {}", region.roads.len());
        let sea_count = region.water.iter().filter(|w| w.kind == WaterKind::Sea).count();
        assert!(sea_count >= 1, "Dyes Inlet sea polygon missing");
        // Sea area via the shoelace: the inlet plus the Port Orchard arm
        // measured 14.5 km2 at fetch; anything under 5 km2 means the
        // coastline assembly regressed.
        let sea_m2: f64 = region
            .water
            .iter()
            .filter(|w| w.kind == WaterKind::Sea)
            .map(|w| signed_area2(&w.ring).abs() * 0.5)
            .sum();
        assert!(sea_m2 > 5.0e6, "sea area {sea_m2:.0} m2, expected > 5 km2");
        let island_lake = region
            .water
            .iter()
            .position(|w| w.name.as_deref() == Some("Island Lake"))
            .expect("Island Lake is in the region");
        assert_eq!(region.water[island_lake].kind, WaterKind::Inland);
        assert!(
            region.water[island_lake + 1..]
                .iter()
                .take_while(|w| w.kind == WaterKind::Island)
                .any(|w| w.name.as_deref() == Some("Clark Island")),
            "Clark Island must follow Island Lake as its inner ring"
        );
    }

    // ── Mesher (native only: it speaks renderer::mesh::Vertex) ───────────

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
    #[cfg(feature = "native")]
    #[test]
    fn mesher_builds_a_lake_sheet_at_shore_min_and_skips_sea() {
        let mut region = flat_region(vec![], vec![]);
        region.water = vec![
            // Sea polygon: must produce NO geometry (the carve + ocean shell
            // own the sea surface).
            OsmWater {
                kind: WaterKind::Sea,
                name: None,
                ring: vec![(-900.0, -900.0), (-500.0, -900.0), (-700.0, -600.0)],
                bounds: (-900.0, -900.0, -500.0, -600.0),
            },
            // A square lake on ground that dips: the sheet must sit at the
            // LOWEST shore point (+0.35 m anti-z-fight lift), not the mean.
            OsmWater {
                kind: WaterKind::Inland,
                name: Some("Test Lake".into()),
                ring: vec![(400.0, 400.0), (500.0, 400.0), (500.0, 500.0), (400.0, 500.0)],
                bounds: (400.0, 400.0, 500.0, 500.0),
            },
            // An island ring: also no geometry (terrain pokes through).
            OsmWater {
                kind: WaterKind::Island,
                name: None,
                ring: vec![(440.0, 440.0), (460.0, 440.0), (450.0, 460.0)],
                bounds: (440.0, 440.0, 460.0, 460.0),
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
        let out = build_region_meshes(&region, &elev);

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

    #[test]
    fn mesher_extrudes_a_square_building_on_flat_ground() {
        let h = 20.0f32; // 12..50 -> mid-rise class
        let region = flat_region(
            vec![building(h, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])],
            vec![],
        );
        let out = build_region_meshes(&region, &|_| FLAT_R);

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
        // Wall bottoms carry the buried skirt; wall tops match the roof.
        let mut lowest = f64::MAX;
        let mut highest = f64::MIN;
        for v in &c.vertices[..16] {
            let r = vertex_radius(out.anchor_local, v);
            lowest = lowest.min(r);
            highest = highest.max(r);
        }
        assert!((lowest - (FLAT_R - WALL_SKIRT_M)).abs() < 1e-2, "skirt depth");
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
        let out = build_region_meshes(&region, &|_| FLAT_R);
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
        let out = build_region_meshes(&region, &|_| FLAT_R);
        assert_eq!(out.skipped_rings, 1);
        assert!(out.classes.is_empty(), "nothing drawn for a refused ring");
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_drapes_a_road_ribbon() {
        // 40 m north then 30 m east. Resampling at 15 m gives
        // 1 + ceil(40/15) + ceil(30/15) = 1 + 3 + 2 = 6 points.
        let points = vec![(0.0f32, 0.0f32), (0.0, 40.0), (30.0, 40.0)];
        let bounds = bounds_of(&points);
        let road = OsmRoad { class: 3, name: None, points: points.clone(), bounds };
        let region = flat_region(vec![], vec![road]);
        let out = build_region_meshes(&region, &|_| FLAT_R);

        let expected_pts = resample_polyline(&points, ROAD_MAX_SEGMENT_M).len();
        assert_eq!(expected_pts, 6, "resampling is deterministic");

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

        // Every road vertex floats exactly one class lift above the ground.
        for v in &c.vertices {
            let r = vertex_radius(out.anchor_local, v);
            assert!(
                (r - (FLAT_R + road_lift_m(3))).abs() < 1e-2,
                "road lift {} m, expected {}",
                r - FLAT_R,
                road_lift_m(3)
            );
        }
    }

    #[cfg(feature = "native")]
    #[test]
    fn mesher_separates_footways_from_carriageways() {
        let mk = |class: u8, e: f32| {
            let points = vec![(e, 0.0f32), (e, 30.0f32)];
            let bounds = bounds_of(&points);
            OsmRoad { class, name: None, points, bounds }
        };
        let region = flat_region(vec![], vec![mk(1, 0.0), mk(5, 50.0)]);
        let out = build_region_meshes(&region, &|_| FLAT_R);
        let kinds: Vec<_> = out.classes.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, vec![RegionMeshKind::RoadAsphalt, RegionMeshKind::RoadFoot]);
        // Footways sit lower so they never fight a road they cross.
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

    /// The mesher must survive the REAL file: 1,457 footprints of genuine OSM
    /// data, including whatever self-intersecting rings survived the fetcher.
    #[cfg(feature = "native")]
    #[test]
    fn mesher_handles_the_shipped_seattle_region() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/maps/regions/seattle-center.bin"
        ))
        .expect("data/maps/regions/seattle-center.bin ships with the repo");
        let region = parse_region(&bytes).expect("parses");
        let out = build_region_meshes(&region, &|_| FLAT_R);
        assert!(!out.classes.is_empty(), "the region produces geometry");
        let tris: usize = out.classes.iter().map(|c| c.indices.len() / 3).sum();
        let verts: usize = out.classes.iter().map(|c| c.vertices.len()).sum();
        eprintln!(
            "seattle-center: {} batches, {verts} verts, {tris} tris, {} rings refused",
            out.classes.len(),
            out.skipped_rings
        );
        // Measured 2026-08-16 on the shipped file: 5 batches, 188,809 verts,
        // 131,817 tris, 0 rings refused. Bad rings are a data-quality number
        // rather than a crash, so this tolerates a few from a future refetch
        // while still failing if the clipper starts rejecting wholesale.
        assert!(
            out.skipped_rings * 20 < region.buildings.len(),
            "{} of {} rings refused: that is too many to be data noise (measured 0)",
            out.skipped_rings,
            region.buildings.len()
        );
        // A whole 2.5 km downtown is a handful of draw calls, not a budget
        // problem: this catches a mesher that starts emitting 10x geometry.
        assert!(out.classes.len() <= 6);
        assert!(
            out.classes.iter().any(|c| c.kind == RegionMeshKind::WaterInland),
            "the v2 file carries Lake Union: an inland water sheet must exist"
        );
        assert!(tris < 400_000, "{tris} triangles for one region is a regression");
        for c in &out.classes {
            assert!(!c.indices.is_empty());
            assert!(c.indices.iter().all(|&i| (i as usize) < c.vertices.len()));
            for v in &c.vertices {
                assert!(
                    v.position.iter().all(|p| p.is_finite())
                        && v.normal.iter().all(|n| n.is_finite()),
                    "no NaN may reach the vertex buffer"
                );
                // Region half spans are ~1.05 x 1.27 km plus the 400 m clip
                // slack plus the tallest tower: everything must stay inside
                // the f32-safe anchor-relative band.
                let m =
                    (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
                assert!(m < 3000.0, "offset {m} m escaped the region");
            }
        }
    }
}
