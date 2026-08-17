//! Water carve: OSM region water polygons pressed into the drawn terrain.
//!
//! WHY THIS EXISTS. The planet's sea renders wherever the drawn ground dips
//! below sea level, because the ocean shell is a global sphere at the sea
//! radius. That works at ocean scale, but the base heightmap's ~460 m cells
//! average narrow inlets up onto dry land (the operator's field report: Dyes
//! Inlet missing from Silverdale while the much wider Hood Canal renders),
//! and lakes sit ABOVE sea level where the shell cannot exist at all. The
//! flight-simulator fix is a VECTOR WATER MASK: real water outlines override
//! the raster terrain. Our vector source is the HOSMREG2 region files
//! (`osm_region`), which carry sea polygons assembled from OSM coastline and
//! inland water polygons with island rings.
//!
//! WHAT IT DOES. Each installed region's water polygons are rasterized once
//! into a small grid over the region's meter space. Terrain samplers then
//! clamp their elevation through [`carve_normalized`]: ground under a sea
//! polygon is pressed to [`SEA_CARVE_M`] below sea level (the existing ocean
//! shell instantly becomes the water surface there, waves, depth tint and
//! all), and ground under a lake is pressed to [`LAKE_CARVE_M`] below the
//! lake's own surface (the flat sheet `osm_region::build_region_meshes`
//! emits). Island rings un-mark their cells, so real islands keep their
//! ground.
//!
//! WHO MUST CALL IT (the stand-on-drawn-ground rule, v0.835): every consumer
//! of the terrain elevation formula. Today that is `build_patch_mesh` (the
//! drawn patches), `drawn_elevation_normalized` (the walk clamp, grass, and
//! the region elevation grid through `ground_radius_m`), and the water
//! shell's coverage + depth bake ([`sea_weight_at`]). A consumer that skips
//! the carve disagrees with the drawn ground by up to [`SEA_CARVE_M`].
//!
//! THREADING. Masks are built on the main thread when regions load
//! (`engine::region_meshes`), published through [`set_global`], and read by
//! the scoped patch-build workers. Readers take one lock per PATCH (via
//! [`snapshot`]) or per sample (via the convenience [`carve_normalized`]);
//! the data behind the `Arc` is immutable after publish.
//!
//! Pure std + glam: compiles in every feature set, testable headless.

use crate::terrain::osm_region::{dir_to_latlon_f64, latlon_to_region_meters, OsmRegion, WaterKind};
use glam::DVec3;
use std::sync::{Arc, RwLock};

/// Sea beds carve to this many metres BELOW sea level. Depth drives the
/// shell's colour: the first probe used 2 m and the whole inlet read as
/// pale-white shallows against the deep-blue open sound, so the bed sits at
/// swimmable-harbour depth instead. (Real Dyes Inlet reaches ~30 m; a
/// constant is rung 1, real bathymetry inside polygons is a later rung.)
pub const SEA_CARVE_M: f64 = 5.0;

/// Lake beds carve to this many metres below the lake's own surface sheet.
pub const LAKE_CARVE_M: f64 = 1.5;

/// Mask resolution per axis. 1024 over the ~9 km Silverdale region is ~9 m
/// cells; the bilinear read in [`RegionMask::sample`] turns cell edges
/// into one-cell shoreline ramps instead of stair steps.
const GRID_N: usize = 1024;

/// Cell kinds in a region mask. Values are the paint layering order.
const CELL_LAND: u8 = 0;
const CELL_SEA: u8 = 1;
const CELL_LAKE: u8 = 2;
/// Road ribbon or building footprint: vegetation must not grow here. Never
/// painted OVER water cells (a bridge crossing the inlet must not un-carve
/// the water beneath it).
const CELL_BUILT: u8 = 3;

/// Curb strip added around a road's real carriageway when stroking the
/// built channel: trees at the exact asphalt edge would still hang canopy
/// over the lane.
const ROAD_CURB_MARGIN_M: f64 = 1.5;

/// One region rasterized into cells: water (the terrain carve) AND the
/// built-over footprint (vegetation suppression), one grid, one lookup.
pub struct RegionMask {
    origin_lat: f64,
    origin_lon: f64,
    half_e: f64,
    half_n: f64,
    /// Cheap pre-reject in degrees (meter conversion costs a cos).
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    /// Per cell: the CELL_* constants above. Row-major, row 0 south.
    kind: Vec<u8>,
    /// Bed target in metres relative to SEA LEVEL, valid on water cells.
    bed_rel_sea_m: Vec<f32>,
}

impl RegionMask {
    /// Rasterize one region: water polygons first (sea, lakes, islands in
    /// file order, islands un-marking their parent), then buildings and
    /// road ribbons as CELL_BUILT over remaining land. `lake_surface_rel_sea_m`
    /// maps an INLAND polygon to its surface level in metres above sea level
    /// (the caller samples its shore ring against the uncarved drawn ground;
    /// see `engine::region_meshes`). Returns None for a region with nothing
    /// to mask, so empty regions cost nothing at sample time.
    pub fn from_region(
        region: &OsmRegion,
        lake_surface_rel_sea_m: &dyn Fn(usize) -> f64,
    ) -> Option<Self> {
        if region.water.is_empty() && region.buildings.is_empty() && region.roads.is_empty() {
            return None;
        }
        let half_e = region.half_east_m as f64;
        let half_n = region.half_north_m as f64;
        let mut kind = vec![CELL_LAND; GRID_N * GRID_N];
        let mut bed = vec![0.0f32; GRID_N * GRID_N];

        // File order is the layering order (sea, then each inland polygon
        // followed by its islands), so painting sequentially is correct:
        // islands un-mark whatever water they sit inside.
        for (wi, w) in region.water.iter().enumerate() {
            let (k, b) = match w.kind {
                WaterKind::Sea => (CELL_SEA, -SEA_CARVE_M as f32),
                WaterKind::Inland => {
                    (CELL_LAKE, (lake_surface_rel_sea_m(wi) - LAKE_CARVE_M) as f32)
                }
                WaterKind::Island => (CELL_LAND, 0.0),
            };
            rasterize_ring(&w.ring, half_e, half_n, &mut kind, &mut bed, k, b);
        }

        // Built channel (v0.1152, operator: trees must not grow through
        // roads, buildings, or water). Buildings fill their footprint; roads
        // stroke their real class width plus a curb strip. Both paint ONLY
        // over land cells so a pier or bridge never un-carves the water.
        for b in &region.buildings {
            rasterize_ring_where_land(&b.ring, half_e, half_n, &mut kind);
        }
        for r in &region.roads {
            let half_w = crate::terrain::osm_region::road_full_width_m(r.class) as f64 * 0.5
                + ROAD_CURB_MARGIN_M;
            stroke_polyline_where_land(&r.points, half_w, half_e, half_n, &mut kind);
        }

        if kind.iter().all(|&c| c == CELL_LAND) {
            return None;
        }

        // Degree bounds for the pre-reject: invert the projection at the
        // region corners (the projection is axis-aligned, so corners bound).
        let m_per_lat = crate::terrain::osm_region::M_PER_DEG_LAT;
        let m_per_lon = crate::terrain::osm_region::M_PER_DEG_LON_EQUATOR
            * region.origin_lat.to_radians().cos().max(1e-9);
        let dlat = half_n / m_per_lat;
        let dlon = half_e / m_per_lon;
        Some(Self {
            origin_lat: region.origin_lat,
            origin_lon: region.origin_lon,
            half_e,
            half_n,
            lat_min: region.origin_lat - dlat,
            lat_max: region.origin_lat + dlat,
            lon_min: region.origin_lon - dlon,
            lon_max: region.origin_lon + dlon,
            kind,
            bed_rel_sea_m: bed,
        })
    }

    /// Bilinear water read at region meters (e, n): `Some((weight, bed))`
    /// where weight in 0..=1 is how watery the 2x2 cell neighborhood is and
    /// bed is the weighted bed target (metres relative to sea level).
    /// None outside the grid or on solid land.
    fn sample(&self, e: f64, n: f64) -> Option<(f32, f32)> {
        let step_e = (2.0 * self.half_e) / GRID_N as f64;
        let step_n = (2.0 * self.half_n) / GRID_N as f64;
        // Fractional cell coordinate, cell-centered.
        let fx = (e + self.half_e) / step_e - 0.5;
        let fy = (n + self.half_n) / step_n - 0.5;
        if fx < -1.0 || fy < -1.0 || fx > GRID_N as f64 || fy > GRID_N as f64 {
            return None;
        }
        let x0 = fx.floor() as i64;
        let y0 = fy.floor() as i64;
        let tx = (fx - x0 as f64) as f32;
        let ty = (fy - y0 as f64) as f32;
        let cell = |x: i64, y: i64| -> (f32, f32) {
            if x < 0 || y < 0 || x >= GRID_N as i64 || y >= GRID_N as i64 {
                return (0.0, 0.0);
            }
            let i = y as usize * GRID_N + x as usize;
            // Built cells are LAND to the carve: roads and buildings sit ON
            // the terrain, they do not change it.
            if self.kind[i] == CELL_SEA || self.kind[i] == CELL_LAKE {
                (1.0, self.bed_rel_sea_m[i])
            } else {
                (0.0, 0.0)
            }
        };
        let (w00, b00) = cell(x0, y0);
        let (w10, b10) = cell(x0 + 1, y0);
        let (w01, b01) = cell(x0, y0 + 1);
        let (w11, b11) = cell(x0 + 1, y0 + 1);
        let w = (w00 * (1.0 - tx) + w10 * tx) * (1.0 - ty) + (w01 * (1.0 - tx) + w11 * tx) * ty;
        if w <= 0.0 {
            return None;
        }
        // Bed blended over the WATER cells only (land cells carry no bed).
        let bw = (b00 * w00 * (1.0 - tx) + b10 * w10 * tx) * (1.0 - ty)
            + (b01 * w01 * (1.0 - tx) + b11 * w11 * tx) * ty;
        Some((w.min(1.0), bw / w))
    }

    /// Sea-only weight (lakes excluded) for the ocean-shell coverage and
    /// depth bake, same bilinear read as `sample`.
    fn sea_sample(&self, e: f64, n: f64) -> f32 {
        let step_e = (2.0 * self.half_e) / GRID_N as f64;
        let step_n = (2.0 * self.half_n) / GRID_N as f64;
        let fx = (e + self.half_e) / step_e - 0.5;
        let fy = (n + self.half_n) / step_n - 0.5;
        if fx < -1.0 || fy < -1.0 || fx > GRID_N as f64 || fy > GRID_N as f64 {
            return 0.0;
        }
        let x0 = fx.floor() as i64;
        let y0 = fy.floor() as i64;
        let tx = (fx - x0 as f64) as f32;
        let ty = (fy - y0 as f64) as f32;
        let cell = |x: i64, y: i64| -> f32 {
            if x < 0 || y < 0 || x >= GRID_N as i64 || y >= GRID_N as i64 {
                return 0.0;
            }
            if self.kind[y as usize * GRID_N + x as usize] == CELL_SEA {
                1.0
            } else {
                0.0
            }
        };
        (cell(x0, y0) * (1.0 - tx) + cell(x0 + 1, y0) * tx) * (1.0 - ty)
            + (cell(x0, y0 + 1) * (1.0 - tx) + cell(x0 + 1, y0 + 1) * tx) * ty
    }

    /// Built-over weight (roads/buildings) at region meters, bilinear like
    /// the water reads so the suppression edge is a ramp, not a stair.
    fn built_sample(&self, e: f64, n: f64) -> f32 {
        let step_e = (2.0 * self.half_e) / GRID_N as f64;
        let step_n = (2.0 * self.half_n) / GRID_N as f64;
        let fx = (e + self.half_e) / step_e - 0.5;
        let fy = (n + self.half_n) / step_n - 0.5;
        if fx < -1.0 || fy < -1.0 || fx > GRID_N as f64 || fy > GRID_N as f64 {
            return 0.0;
        }
        let x0 = fx.floor() as i64;
        let y0 = fy.floor() as i64;
        let tx = (fx - x0 as f64) as f32;
        let ty = (fy - y0 as f64) as f32;
        let cell = |x: i64, y: i64| -> f32 {
            if x < 0 || y < 0 || x >= GRID_N as i64 || y >= GRID_N as i64 {
                return 0.0;
            }
            if self.kind[y as usize * GRID_N + x as usize] == CELL_BUILT {
                1.0
            } else {
                0.0
            }
        };
        (cell(x0, y0) * (1.0 - tx) + cell(x0 + 1, y0) * tx) * (1.0 - ty)
            + (cell(x0, y0 + 1) * (1.0 - tx) + cell(x0 + 1, y0 + 1) * tx) * ty
    }

    #[inline]
    fn latlon_hit(&self, lat: f64, lon: f64) -> bool {
        lat >= self.lat_min && lat <= self.lat_max && lon >= self.lon_min && lon <= self.lon_max
    }
}

/// Even-odd scanline fill of one ring into the cell grids. `k`/`b` are the
/// kind and bed values to paint (k = 0 paints LAND, which is how islands
/// un-mark their parent water).
fn rasterize_ring(
    ring: &[(f32, f32)],
    half_e: f64,
    half_n: f64,
    kind: &mut [u8],
    bed: &mut [f32],
    k: u8,
    b: f32,
) {
    if ring.len() < 3 {
        return;
    }
    let step_e = (2.0 * half_e) / GRID_N as f64;
    let step_n = (2.0 * half_n) / GRID_N as f64;
    // Row range the ring can touch.
    let (mut n_min, mut n_max) = (f64::MAX, f64::MIN);
    for &(_, n) in ring {
        n_min = n_min.min(n as f64);
        n_max = n_max.max(n as f64);
    }
    let y_lo = (((n_min + half_n) / step_n - 0.5).ceil().max(0.0)) as usize;
    let y_hi = (((n_max + half_n) / step_n - 0.5).floor().min(GRID_N as f64 - 1.0)) as i64;
    if y_hi < y_lo as i64 {
        return;
    }
    let mut xs: Vec<f64> = Vec::with_capacity(8);
    for y in y_lo..=(y_hi as usize) {
        let n = -half_n + (y as f64 + 0.5) * step_n;
        xs.clear();
        for i in 0..ring.len() {
            let a = ring[i];
            let c = ring[(i + 1) % ring.len()];
            let (ay, cy) = (a.1 as f64, c.1 as f64);
            // Half-open span rule: counts each crossing exactly once even
            // when the scanline passes through a vertex.
            if (ay > n) != (cy > n) {
                let t = (n - ay) / (cy - ay);
                xs.push(a.0 as f64 + t * (c.0 as f64 - a.0 as f64));
            }
        }
        xs.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
        for pair in xs.chunks_exact(2) {
            let x_lo = (((pair[0] + half_e) / step_e - 0.5).ceil().max(0.0)) as usize;
            let x_hi = (((pair[1] + half_e) / step_e - 0.5).floor().min(GRID_N as f64 - 1.0)) as i64;
            if x_hi < x_lo as i64 {
                continue;
            }
            for x in x_lo..=(x_hi as usize) {
                let i = y * GRID_N + x;
                kind[i] = k;
                bed[i] = b;
            }
        }
    }
}

/// Even-odd fill like `rasterize_ring`, but painting CELL_BUILT and ONLY
/// onto CELL_LAND cells: water keeps its carve under piers and lakeside
/// decks, and built never downgrades to land by a later polygon.
fn rasterize_ring_where_land(ring: &[(f32, f32)], half_e: f64, half_n: f64, kind: &mut [u8]) {
    if ring.len() < 3 {
        return;
    }
    let step_e = (2.0 * half_e) / GRID_N as f64;
    let step_n = (2.0 * half_n) / GRID_N as f64;
    let (mut n_min, mut n_max) = (f64::MAX, f64::MIN);
    for &(_, n) in ring {
        n_min = n_min.min(n as f64);
        n_max = n_max.max(n as f64);
    }
    let y_lo = (((n_min + half_n) / step_n - 0.5).ceil().max(0.0)) as usize;
    let y_hi = (((n_max + half_n) / step_n - 0.5).floor().min(GRID_N as f64 - 1.0)) as i64;
    if y_hi < y_lo as i64 {
        return;
    }
    let mut xs: Vec<f64> = Vec::with_capacity(8);
    for y in y_lo..=(y_hi as usize) {
        let n = -half_n + (y as f64 + 0.5) * step_n;
        xs.clear();
        for i in 0..ring.len() {
            let a = ring[i];
            let c = ring[(i + 1) % ring.len()];
            let (ay, cy) = (a.1 as f64, c.1 as f64);
            if (ay > n) != (cy > n) {
                let t = (n - ay) / (cy - ay);
                xs.push(a.0 as f64 + t * (c.0 as f64 - a.0 as f64));
            }
        }
        xs.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
        for pair in xs.chunks_exact(2) {
            let x_lo = (((pair[0] + half_e) / step_e - 0.5).ceil().max(0.0)) as usize;
            let x_hi =
                (((pair[1] + half_e) / step_e - 0.5).floor().min(GRID_N as f64 - 1.0)) as i64;
            if x_hi < x_lo as i64 {
                continue;
            }
            for x in x_lo..=(x_hi as usize) {
                let i = y * GRID_N + x;
                if kind[i] == CELL_LAND {
                    kind[i] = CELL_BUILT;
                }
            }
        }
    }
}

/// Stroke a road centreline into CELL_BUILT: every land cell whose centre
/// lies within `half_w` metres of any segment. Cell-bbox bounded per
/// segment, so cost tracks road length, not grid area.
fn stroke_polyline_where_land(
    points: &[(f32, f32)],
    half_w: f64,
    half_e: f64,
    half_n: f64,
    kind: &mut [u8],
) {
    if points.len() < 2 {
        return;
    }
    let step_e = (2.0 * half_e) / GRID_N as f64;
    let step_n = (2.0 * half_n) / GRID_N as f64;
    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let (ax, ay) = (a.0 as f64, a.1 as f64);
        let (bx, by) = (b.0 as f64, b.1 as f64);
        let (min_x, max_x) = (ax.min(bx) - half_w, ax.max(bx) + half_w);
        let (min_y, max_y) = (ay.min(by) - half_w, ay.max(by) + half_w);
        let x_lo = (((min_x + half_e) / step_e - 0.5).floor().max(0.0)) as usize;
        let x_hi = (((max_x + half_e) / step_e - 0.5).ceil().min(GRID_N as f64 - 1.0)) as i64;
        let y_lo = (((min_y + half_n) / step_n - 0.5).floor().max(0.0)) as usize;
        let y_hi = (((max_y + half_n) / step_n - 0.5).ceil().min(GRID_N as f64 - 1.0)) as i64;
        if x_hi < x_lo as i64 || y_hi < y_lo as i64 {
            continue;
        }
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        for y in y_lo..=(y_hi as usize) {
            let cn = -half_n + (y as f64 + 0.5) * step_n;
            for x in x_lo..=(x_hi as usize) {
                let ce = -half_e + (x as f64 + 0.5) * step_e;
                // Point-to-segment distance, squared.
                let t = if len2 > 0.0 {
                    (((ce - ax) * dx + (cn - ay) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (pe, pn) = (ax + t * dx, ay + t * dy);
                let d2 = (ce - pe) * (ce - pe) + (cn - pn) * (cn - pn);
                if d2 <= half_w * half_w {
                    let i = y * GRID_N + x;
                    if kind[i] == CELL_LAND {
                        kind[i] = CELL_BUILT;
                    }
                }
            }
        }
    }
}

// ── The global registry ─────────────────────────────────────────────────────

static CARVE: RwLock<Option<Arc<Vec<RegionMask>>>> = RwLock::new(None);

/// Publish the rasterized masks (region load). Replaces any prior set.
pub fn set_global(masks: Arc<Vec<RegionMask>>) {
    *CARVE.write().expect("water carve lock poisoned") = Some(masks);
}

/// Drop all masks (world exit / tests).
pub fn clear_global() {
    *CARVE.write().expect("water carve lock poisoned") = None;
}

/// One lock, one `Arc` clone: take this once per PATCH and use
/// [`carve_normalized_with`] per vertex.
pub fn snapshot() -> Option<Arc<Vec<RegionMask>>> {
    CARVE.read().expect("water carve lock poisoned").clone()
}

/// Apply the carve to one NORMALIZED elevation sample against an explicit
/// mask set (see [`snapshot`]). `min_m..max_m` is the heightmap window and
/// `sea_norm` the planet's normalized sea level. Returns `e` untouched
/// outside every region.
pub fn carve_normalized_with(
    masks: &[RegionMask],
    dir: DVec3,
    e: f32,
    min_m: f32,
    max_m: f32,
    sea_norm: f32,
) -> f32 {
    let range = max_m - min_m;
    if range <= 0.0 || masks.is_empty() {
        return e;
    }
    let (lat, lon) = dir_to_latlon_f64(dir);
    for m in masks {
        if !m.latlon_hit(lat, lon) {
            continue;
        }
        let (me, mn) = latlon_to_region_meters(m.origin_lat, m.origin_lon, lat, lon);
        if let Some((w, bed_rel)) = m.sample(me, mn) {
            let target = sea_norm + bed_rel / range;
            if target < e {
                // Smoothstep the shoreline ramp so the carve fades in over
                // one mask cell instead of stepping.
                let s = w * w * (3.0 - 2.0 * w);
                return e + (target - e) * s;
            }
        }
        // Inside this region's bounds: no other region can also contain the
        // point (regions do not overlap in practice; first hit wins).
        return e;
    }
    e
}

/// Convenience single-sample carve: snapshots the registry itself. Fine for
/// per-call sites (the walk clamp); patch builders should [`snapshot`] once.
pub fn carve_normalized(dir: DVec3, e: f32, min_m: f32, max_m: f32, sea_norm: f32) -> f32 {
    match snapshot() {
        Some(masks) => carve_normalized_with(&masks, dir, e, min_m, max_m, sea_norm),
        None => e,
    }
}

/// Built-over weight at `dir` (0 = open ground, 1 = fully road/building),
/// for the vegetation gates: every tree/grass stream MUST consult this with
/// the same threshold discipline as the water carve, or streams disagree
/// about where a tree can stand.
pub fn built_weight_at(masks: &[RegionMask], dir: DVec3) -> f32 {
    let (lat, lon) = dir_to_latlon_f64(dir);
    built_weight_at_deg(masks, lat, lon)
}

/// `built_weight_at` for callers that already HAVE degrees. The vegetation
/// streams generate candidates AS lat/lon and only derive the dir from
/// them, so going dir-first would pay an asin+atan2 per candidate to
/// recover numbers the caller was holding all along.
pub fn built_weight_at_deg(masks: &[RegionMask], lat: f64, lon: f64) -> f32 {
    for m in masks {
        if !m.latlon_hit(lat, lon) {
            continue;
        }
        let (me, mn) = latlon_to_region_meters(m.origin_lat, m.origin_lon, lat, lon);
        return m.built_sample(me, mn);
    }
    0.0
}

/// True when a disc of `radius_m` around `center_dir` touches any mask's
/// bounds: ONE call per vegetation harvest, so the gates below cost nothing
/// anywhere on the planet outside an installed region.
pub fn any_mask_in_disc(masks: &[RegionMask], center_dir: DVec3, radius_m: f64) -> bool {
    let (lat, lon) = dir_to_latlon_f64(center_dir);
    let dlat = radius_m / crate::terrain::osm_region::M_PER_DEG_LAT;
    let dlon = radius_m
        / (crate::terrain::osm_region::M_PER_DEG_LON_EQUATOR
            * lat.to_radians().cos().abs().max(1e-6));
    masks.iter().any(|m| {
        lat + dlat >= m.lat_min
            && lat - dlat <= m.lat_max
            && lon + dlon >= m.lon_min
            && lon - dlon <= m.lon_max
    })
}

/// SEA coverage weight at `dir` (0 = no sea, 1 = fully sea), for the ocean
/// shell's coverage test and depth bake. Lakes report 0: they have their own
/// surface sheets.
pub fn sea_weight_at(masks: &[RegionMask], dir: DVec3) -> f32 {
    let (lat, lon) = dir_to_latlon_f64(dir);
    for m in masks {
        if !m.latlon_hit(lat, lon) {
            continue;
        }
        let (me, mn) = latlon_to_region_meters(m.origin_lat, m.origin_lon, lat, lon);
        return m.sea_sample(me, mn);
    }
    0.0
}

// ─────────────────────────────────── Tests ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::osm_region::{latlon_to_dir_f64, region_meters_to_latlon, OsmWater};

    /// A synthetic region: a 800 m sea square west, a 200 m lake east with a
    /// small island inside it.
    fn test_region() -> OsmRegion {
        OsmRegion {
            name: "Carve Test".into(),
            origin_lat: 47.6,
            origin_lon: -122.7,
            half_east_m: 2000.0,
            half_north_m: 2000.0,
            roads: Vec::new(),
            buildings: Vec::new(),
            water: vec![
                OsmWater {
                    kind: WaterKind::Sea,
                    name: None,
                    ring: vec![(-1800.0, -400.0), (-1000.0, -400.0), (-1000.0, 400.0), (-1800.0, 400.0)],
                    bounds: (-1800.0, -400.0, -1000.0, 400.0),
                },
                OsmWater {
                    kind: WaterKind::Inland,
                    name: Some("Test Lake".into()),
                    ring: vec![(800.0, -100.0), (1200.0, -100.0), (1200.0, 300.0), (800.0, 300.0)],
                    bounds: (800.0, -100.0, 1200.0, 300.0),
                },
                OsmWater {
                    kind: WaterKind::Island,
                    name: None,
                    ring: vec![(950.0, 50.0), (1050.0, 50.0), (1050.0, 150.0), (950.0, 150.0)],
                    bounds: (950.0, 50.0, 1050.0, 150.0),
                },
            ],
        }
    }

    fn dir_at(region: &OsmRegion, e: f64, n: f64) -> DVec3 {
        let (lat, lon) = region_meters_to_latlon(region.origin_lat, region.origin_lon, e, n);
        latlon_to_dir_f64(lat, lon)
    }

    // The synthetic planet: heightmap window 0..1000 m, sea at 0.1 (100 m).
    const MIN_M: f32 = 0.0;
    const MAX_M: f32 = 1000.0;
    const SEA_NORM: f32 = 0.1;

    fn masks() -> Vec<RegionMask> {
        // Lake surface 50 m above sea level.
        vec![RegionMask::from_region(&test_region(), &|_| 50.0).expect("has water")]
    }

    #[test]
    fn sea_cells_carve_to_below_sea_level() {
        let r = test_region();
        let m = masks();
        // Land elevation 0.15 normalized = 150 m, 50 m above the sea.
        let e = carve_normalized_with(&m, dir_at(&r, -1400.0, 0.0), 0.15, MIN_M, MAX_M, SEA_NORM);
        let want = SEA_NORM - (SEA_CARVE_M as f32) / (MAX_M - MIN_M);
        assert!(
            (e - want).abs() < 1e-4,
            "sea centre carved to {e}, want {want} (2 m below sea)"
        );
    }

    #[test]
    fn lake_cells_carve_below_the_lake_surface() {
        let r = test_region();
        let m = masks();
        let e = carve_normalized_with(&m, dir_at(&r, 900.0, -50.0), 0.2, MIN_M, MAX_M, SEA_NORM);
        // Lake surface 50 m above sea, bed 1.5 m under it.
        let want = SEA_NORM + (50.0 - LAKE_CARVE_M as f32) / (MAX_M - MIN_M);
        assert!(
            (e - want).abs() < 1e-4,
            "lake bed carved to {e}, want {want}"
        );
    }

    #[test]
    fn island_and_dry_land_stay_uncarved() {
        let r = test_region();
        let m = masks();
        for (e, n, what) in [
            (1000.0, 100.0, "island centre"),
            (0.0, 0.0, "dry land between sea and lake"),
            (-1900.0, 1900.0, "region corner"),
        ] {
            let before = 0.3f32;
            let after = carve_normalized_with(&m, dir_at(&r, e, n), before, MIN_M, MAX_M, SEA_NORM);
            assert_eq!(before, after, "{what} must not be carved");
        }
    }

    #[test]
    fn carve_never_raises_ground() {
        // A seabed already deeper than the carve target must stay put:
        // the carve is a min, not an assignment.
        let r = test_region();
        let m = masks();
        let deep = 0.02f32; // 20 m elevation = 80 m below the 100 m sea
        let after = carve_normalized_with(&m, dir_at(&r, -1400.0, 0.0), deep, MIN_M, MAX_M, SEA_NORM);
        assert_eq!(deep, after, "already-submerged ground must not be lifted");
    }

    #[test]
    fn shoreline_ramps_instead_of_stepping() {
        // Walking from dry land into the sea square, the carve must pass
        // through intermediate values (the bilinear + smoothstep ramp), not
        // jump from land to full depth in one sample.
        let r = test_region();
        let m = masks();
        let land = 0.15f32;
        let mut prev = land;
        let mut max_step = 0.0f32;
        let mut n_steps = 0;
        let mut e_pos = -960.0; // outside the sea's +400/-1000 east edge
        while e_pos > -1100.0 {
            let e = carve_normalized_with(&m, dir_at(&r, e_pos, 0.0), land, MIN_M, MAX_M, SEA_NORM);
            max_step = max_step.max((prev - e).abs());
            if e != prev {
                n_steps += 1;
            }
            prev = e;
            e_pos -= 1.0;
        }
        let full = land - (SEA_NORM - (SEA_CARVE_M as f32) / (MAX_M - MIN_M));
        assert!(n_steps >= 3, "carve must ramp over several samples, saw {n_steps} changes");
        assert!(
            max_step < full * 0.9,
            "largest single-metre step {max_step} is nearly the full carve {full}: no ramp"
        );
    }

    #[test]
    fn sea_weight_reports_sea_not_lakes() {
        let r = test_region();
        let m = masks();
        assert!(sea_weight_at(&m, dir_at(&r, -1400.0, 0.0)) > 0.99, "sea centre");
        assert!(sea_weight_at(&m, dir_at(&r, 900.0, -50.0)) < 0.01, "lake is not sea");
        assert!(sea_weight_at(&m, dir_at(&r, 0.0, 0.0)) < 0.01, "dry land");
    }

    #[test]
    fn built_mask_covers_buildings_and_roads_but_never_water() {
        use crate::terrain::osm_region::{OsmBuilding, OsmRoad};
        let mut region = test_region();
        // A 100 m building square on open land.
        region.buildings.push(OsmBuilding {
            height_m: 8.0,
            ring: vec![(-200.0, 1000.0), (-100.0, 1000.0), (-100.0, 1100.0), (-200.0, 1100.0)],
            bounds: (-200.0, 1000.0, -100.0, 1100.0),
        });
        // A straight north-south residential road (class 3: 6.5 m + curb).
        region.roads.push(OsmRoad {
            class: 3,
            name: None,
            points: vec![(500.0, -1500.0), (500.0, 1500.0)],
            bounds: (500.0, -1500.0, 500.0, 1500.0),
        });
        // A pier road straight across the SEA square: it must NOT un-carve
        // the water under it.
        region.roads.push(OsmRoad {
            class: 4,
            name: None,
            points: vec![(-1700.0, 0.0), (-1100.0, 0.0)],
            bounds: (-1700.0, 0.0, -1100.0, 0.0),
        });
        let m = vec![RegionMask::from_region(&region, &|_| 50.0).expect("has content")];

        // Building interior and road centreline are built.
        assert!(
            built_weight_at(&m, dir_at(&region, -150.0, 1050.0)) > 0.9,
            "building interior must be built"
        );
        assert!(
            built_weight_at(&m, dir_at(&region, 500.0, 200.0)) > 0.9,
            "road centreline must be built"
        );
        // 30 m off the road: open ground.
        assert!(
            built_weight_at(&m, dir_at(&region, 530.0, 200.0)) < 0.1,
            "30 m from a residential road must be open"
        );
        // Under the pier the SEA carve survives, and the cell does not
        // count as built (water wins the layering).
        let e = carve_normalized_with(&m, dir_at(&region, -1400.0, 0.0), 0.15, MIN_M, MAX_M, SEA_NORM);
        assert!(e < SEA_NORM, "the pier road must not un-carve the sea beneath it");
        assert!(
            built_weight_at(&m, dir_at(&region, -1400.0, 0.0)) < 0.1,
            "water cells never read as built"
        );
        // A region with ONLY roads/buildings (no water) still masks.
        let dry = OsmRegion {
            name: "Dry".into(),
            origin_lat: 47.0,
            origin_lon: -122.0,
            half_east_m: 2000.0,
            half_north_m: 2000.0,
            roads: region.roads.clone(),
            buildings: region.buildings.clone(),
            water: Vec::new(),
        };
        let dm = RegionMask::from_region(&dry, &|_| 0.0).expect("dry region still masks");
        assert!(built_weight_at(&[dm], dir_at(&dry, 500.0, 200.0)) > 0.9);
    }

    #[test]
    fn global_registry_round_trips() {
        clear_global();
        assert!(snapshot().is_none());
        set_global(Arc::new(masks()));
        let s = snapshot().expect("set_global publishes");
        assert_eq!(s.len(), 1);
        let r = test_region();
        let e = carve_normalized(dir_at(&r, -1400.0, 0.0), 0.15, MIN_M, MAX_M, SEA_NORM);
        assert!(e < 0.1, "convenience path applies the carve");
        clear_global();
        let e2 = carve_normalized(dir_at(&r, -1400.0, 0.0), 0.15, MIN_M, MAX_M, SEA_NORM);
        assert_eq!(e2, 0.15, "cleared registry carves nothing");
    }
}
