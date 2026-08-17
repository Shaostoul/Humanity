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

/// Hermite smoothstep on 0..=1 input (clamped), local copy so this module
/// stays pure std + glam in every feature set.
fn smoothstep01(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
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

// ── Shore taper (v0.1153, operator: "the water edge looks like a sheer
// cliff instead of tapering into the water") ────────────────────────────────
// The first carve pressed full depth one bilinear cell from the polygon
// edge: a 5 m drop over ~10 m horizontal, i.e. a seawall. Real shorelines
// grade over tens of metres on BOTH sides of the waterline, so the mask now
// carries a distance-field beach profile: water reaches full depth only
// SHORE_TAPER_WATER_M from land, and land within SHORE_TAPER_LAND_M of
// water is pulled down toward the waterline (never raised), which kills
// the bluff the coarse base heightmap otherwise leaves standing at the
// polygon edge.

/// Water reaches its full carve depth this many metres from the shoreline.
const SHORE_TAPER_WATER_M: f64 = 90.0;
/// Land blends toward the waterline within this many metres of water.
const SHORE_TAPER_LAND_M: f64 = 60.0;
/// Depth right at the waterline: enough to read as wet, shallow enough to
/// wade in from the beach.
const SHORE_MIN_DEPTH_M: f32 = 0.7;
/// Beach land sits this far above its adjacent water surface.
const SHORE_LAND_TARGET_M: f32 = 0.75;

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
    /// Per-cell carve TARGET surface in metres relative to SEA LEVEL. On
    /// water cells this is the tapered bed; on shore land it is the beach
    /// blend level. Valid where `carve_w > 0`.
    target_rel_sea_m: Vec<f32>,
    /// Per-cell carve weight, 0..=255. Water cells are 255; shore land ramps
    /// down with distance from the waterline; open land is 0.
    carve_w: Vec<u8>,
    /// Real elevation for the region's area (HOSDEM1, ~13 m grid), when its
    /// sibling .dem.bin is installed. Inside its coverage the drawn ground
    /// IS this data (edge-blended into the coarse base over
    /// [`DEM_EDGE_BLEND_M`]); the carve then applies on top.
    dem: Option<crate::terrain::osm_region::RegionDem>,
}

/// The DEM fades into the coarse base heightmap over this many metres at its
/// coverage edge, so the region boundary never shows a terrain step.
const DEM_EDGE_BLEND_M: f64 = 400.0;

/// Bathymetry-seam spikes in the source data reach kilometres below sea
/// level over inlets (fetcher report, 2026-08-17: 0.1% of samples, deepest
/// -2827 m). The DEM governs LAND; underwater depth is the carve's job, so
/// floor it just below the waterline.
const DEM_FLOOR_M: f32 = -2.0;

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
        Self::from_region_with_dem(region, lake_surface_rel_sea_m, None)
    }

    /// `from_region` plus the region's real-elevation DEM (the sibling
    /// .dem.bin, when installed).
    pub fn from_region_with_dem(
        region: &OsmRegion,
        lake_surface_rel_sea_m: &dyn Fn(usize) -> f64,
        dem: Option<crate::terrain::osm_region::RegionDem>,
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

        // ── Shore taper: turn the binary water mask into a beach profile ──
        // Two chamfer feature transforms over the grid: distance to the
        // nearest LAND (for water cells: depth taper) and distance to the
        // nearest WATER plus that water's SURFACE level (for land cells:
        // beach blend toward the right waterline, sea and lakes alike).
        let cell_m =
            ((2.0 * half_e / GRID_N as f64) + (2.0 * half_n / GRID_N as f64)) * 0.5;
        let is_water = |k: u8| k == CELL_SEA || k == CELL_LAKE;
        let d_land = chamfer_distance(&kind, |k| !is_water(k));
        // Water SURFACE per water cell: sea = 0, lake = bed + LAKE_CARVE_M.
        let surface_of = |i: usize| -> f32 {
            if kind[i] == CELL_LAKE {
                bed[i] + LAKE_CARVE_M as f32
            } else {
                0.0
            }
        };
        let (d_water, near_surface) = chamfer_feature(&kind, is_water, &surface_of);

        let mut target = vec![0.0f32; GRID_N * GRID_N];
        let mut carve_w = vec![0u8; GRID_N * GRID_N];
        for i in 0..GRID_N * GRID_N {
            if is_water(kind[i]) {
                let surface = surface_of(i);
                let full_depth = surface - bed[i]; // 5 m sea, 1.5 m lake
                let d_m = d_land[i] as f64 * cell_m;
                let t = smoothstep01((d_m / SHORE_TAPER_WATER_M) as f32);
                let depth = SHORE_MIN_DEPTH_M + (full_depth - SHORE_MIN_DEPTH_M).max(0.0) * t;
                target[i] = surface - depth;
                carve_w[i] = 255;
            } else {
                let d_m = d_water[i] as f64 * cell_m;
                if d_m < SHORE_TAPER_LAND_M {
                    let w = 1.0 - smoothstep01((d_m / SHORE_TAPER_LAND_M) as f32);
                    target[i] = near_surface[i] + SHORE_LAND_TARGET_M;
                    carve_w[i] = (w * 255.0) as u8;
                }
            }
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
            target_rel_sea_m: target,
            carve_w,
            dem,
        })
    }

    /// The DEM-overlaid elevation at (lat, lon), replacing `e` inside the
    /// DEM's coverage and blending back to `e` over [`DEM_EDGE_BLEND_M`] at
    /// the edge. `e` is normalized; `sea`/`range` map metres to that domain.
    fn dem_overlay(&self, lat: f64, lon: f64, e: f32, sea: f32, range: f32) -> f32 {
        let Some(dem) = &self.dem else { return e };
        let Some(raw_m) = dem.sample_m(lat, lon) else { return e };
        let dem_m = raw_m.max(DEM_FLOOR_M);
        let e_dem = sea + dem_m / range;
        // Edge blend: distance INSIDE the coverage box, in metres.
        let (lat_s, lat_n, lon_w, lon_e) = dem.bounds_deg();
        let m_per_lat = crate::terrain::osm_region::M_PER_DEG_LAT;
        let m_per_lon = crate::terrain::osm_region::M_PER_DEG_LON_EQUATOR
            * self.origin_lat.to_radians().cos().abs().max(1e-9);
        let inside_m = ((lat - lat_s) * m_per_lat)
            .min((lat_n - lat) * m_per_lat)
            .min((lon - lon_w) * m_per_lon)
            .min((lon_e - lon) * m_per_lon);
        let w = smoothstep01((inside_m / DEM_EDGE_BLEND_M) as f32);
        e + (e_dem - e) * w
    }

    /// Bilinear carve read at region meters (e, n): `Some((weight, target))`
    /// where weight in 0..=1 blends the carve in (water 1.0, beach land
    /// ramping to 0 with distance from the waterline) and target is the
    /// weighted TARGET surface (metres relative to sea level: tapered bed on
    /// water, beach level on shore land). None outside the grid or where no
    /// cell in the 2x2 neighborhood carves.
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
            let w = self.carve_w[i] as f32 / 255.0;
            (w, self.target_rel_sea_m[i])
        };
        let (w00, b00) = cell(x0, y0);
        let (w10, b10) = cell(x0 + 1, y0);
        let (w01, b01) = cell(x0, y0 + 1);
        let (w11, b11) = cell(x0 + 1, y0 + 1);
        let w = (w00 * (1.0 - tx) + w10 * tx) * (1.0 - ty) + (w01 * (1.0 - tx) + w11 * tx) * ty;
        if w <= 0.001 {
            return None;
        }
        // Target blended weight-proportionally (zero-weight cells carry no
        // meaningful target).
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

/// Two-pass 3-4 chamfer distance transform: distance in CELLS from every
/// cell to the nearest cell where `is_source` holds. Distances are exact
/// enough for a 60-90 m taper (error < 8% of Euclidean), and the two passes
/// are O(n) over the megacell grid.
fn chamfer_distance(kind: &[u8], is_source: impl Fn(u8) -> bool) -> Vec<f32> {
    const ORTHO: f32 = 1.0;
    const DIAG: f32 = 1.4142135;
    let mut d = vec![f32::MAX; GRID_N * GRID_N];
    for i in 0..GRID_N * GRID_N {
        if is_source(kind[i]) {
            d[i] = 0.0;
        }
    }
    // Forward pass: relax from W, N, NW, NE.
    for y in 0..GRID_N {
        for x in 0..GRID_N {
            let i = y * GRID_N + x;
            let mut best = d[i];
            if x > 0 {
                best = best.min(d[i - 1] + ORTHO);
            }
            if y > 0 {
                best = best.min(d[i - GRID_N] + ORTHO);
                if x > 0 {
                    best = best.min(d[i - GRID_N - 1] + DIAG);
                }
                if x + 1 < GRID_N {
                    best = best.min(d[i - GRID_N + 1] + DIAG);
                }
            }
            d[i] = best;
        }
    }
    // Backward pass: relax from E, S, SE, SW.
    for y in (0..GRID_N).rev() {
        for x in (0..GRID_N).rev() {
            let i = y * GRID_N + x;
            let mut best = d[i];
            if x + 1 < GRID_N {
                best = best.min(d[i + 1] + ORTHO);
            }
            if y + 1 < GRID_N {
                best = best.min(d[i + GRID_N] + ORTHO);
                if x + 1 < GRID_N {
                    best = best.min(d[i + GRID_N + 1] + DIAG);
                }
                if x > 0 {
                    best = best.min(d[i + GRID_N - 1] + DIAG);
                }
            }
            d[i] = best;
        }
    }
    d
}

/// `chamfer_distance` that ALSO propagates a per-source payload (the nearest
/// water cell's surface level), so a beach on a lake shore blends toward the
/// LAKE's waterline, not sea level.
fn chamfer_feature(
    kind: &[u8],
    is_source: impl Fn(u8) -> bool,
    payload_of: &impl Fn(usize) -> f32,
) -> (Vec<f32>, Vec<f32>) {
    const ORTHO: f32 = 1.0;
    const DIAG: f32 = 1.4142135;
    let mut d = vec![f32::MAX; GRID_N * GRID_N];
    let mut p = vec![0.0f32; GRID_N * GRID_N];
    for i in 0..GRID_N * GRID_N {
        if is_source(kind[i]) {
            d[i] = 0.0;
            p[i] = payload_of(i);
        }
    }
    let mut relax = |i: usize, j: usize, cost: f32, d: &mut [f32], p: &mut [f32]| {
        if d[j] + cost < d[i] {
            d[i] = d[j] + cost;
            p[i] = p[j];
        }
    };
    for y in 0..GRID_N {
        for x in 0..GRID_N {
            let i = y * GRID_N + x;
            if x > 0 {
                relax(i, i - 1, ORTHO, &mut d, &mut p);
            }
            if y > 0 {
                relax(i, i - GRID_N, ORTHO, &mut d, &mut p);
                if x > 0 {
                    relax(i, i - GRID_N - 1, DIAG, &mut d, &mut p);
                }
                if x + 1 < GRID_N {
                    relax(i, i - GRID_N + 1, DIAG, &mut d, &mut p);
                }
            }
        }
    }
    for y in (0..GRID_N).rev() {
        for x in (0..GRID_N).rev() {
            let i = y * GRID_N + x;
            if x + 1 < GRID_N {
                relax(i, i + 1, ORTHO, &mut d, &mut p);
            }
            if y + 1 < GRID_N {
                relax(i, i + GRID_N, ORTHO, &mut d, &mut p);
                if x + 1 < GRID_N {
                    relax(i, i + GRID_N + 1, DIAG, &mut d, &mut p);
                }
                if x > 0 {
                    relax(i, i + GRID_N - 1, DIAG, &mut d, &mut p);
                }
            }
        }
    }
    (d, p)
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
        // Real elevation first (v0.1153): inside the region's DEM the drawn
        // ground IS the ~13 m survey data, not the ~460 m base average. The
        // carve then applies on top, so real coastal gradients and the
        // beach-profile taper compose.
        let e = m.dem_overlay(lat, lon, e, sea_norm, range);
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
        // The island (950..1050, 50..150) occupies the lake's centre, so the
        // widest water gap is between the west shore (e=800) and the island:
        // (875, 100) is ~75 m from land on both sides. That is inside the
        // 90 m taper, so expect NEAR-full depth (within 1 m), not exact.
        let e = carve_normalized_with(&m, dir_at(&r, 875.0, 100.0), 0.2, MIN_M, MAX_M, SEA_NORM);
        // Lake surface 50 m above sea, bed 1.5 m under it.
        let want = SEA_NORM + (50.0 - LAKE_CARVE_M as f32) / (MAX_M - MIN_M);
        assert!(
            (e - want).abs() < 1.0 / (MAX_M - MIN_M),
            "lake bed carved to {e}, want about {want} (within 1 m)"
        );
    }

    #[test]
    fn shore_water_is_shallow_and_deepens_with_distance() {
        // The taper itself: just inside the sea edge must be SHALLOW, the
        // middle must be full depth, and depth must increase monotonically
        // walking away from shore.
        let r = test_region();
        let m = masks();
        let land = 0.5f32;
        let near =
            carve_normalized_with(&m, dir_at(&r, -1010.0, 0.0), land, MIN_M, MAX_M, SEA_NORM);
        let mid = carve_normalized_with(&m, dir_at(&r, -1400.0, 0.0), land, MIN_M, MAX_M, SEA_NORM);
        let full = SEA_NORM - SEA_CARVE_M as f32 / (MAX_M - MIN_M);
        assert!(
            near > full + 2.0 / (MAX_M - MIN_M),
            "10 m from shore must be metres shallower than the full bed: near {near}, full {full}"
        );
        assert!((mid - full).abs() < 0.5 / (MAX_M - MIN_M), "mid-sea reaches full depth");
        let mut prev = near;
        for e_pos in [-1030.0, -1060.0, -1100.0, -1200.0] {
            let d = carve_normalized_with(&m, dir_at(&r, e_pos, 0.0), land, MIN_M, MAX_M, SEA_NORM);
            assert!(d <= prev + 1e-5, "depth must not decrease moving offshore at e={e_pos}");
            prev = d;
        }
    }

    #[test]
    fn beach_land_blends_toward_the_waterline() {
        // Land 20 m from the sea edge: a 150 m bluff must be pulled well
        // down toward the waterline (the operator's cliff report), while
        // land past the taper reach stays untouched.
        let r = test_region();
        let m = masks();
        let bluff = 0.25f32; // 250 m elevation = 150 m above the 100 m sea
        let at_beach =
            carve_normalized_with(&m, dir_at(&r, -980.0, 0.0), bluff, MIN_M, MAX_M, SEA_NORM);
        assert!(
            at_beach < 0.17,
            "a bluff 20 m from the water must drop toward the waterline, got {at_beach}"
        );
        let inland =
            carve_normalized_with(&m, dir_at(&r, -900.0, 0.0), bluff, MIN_M, MAX_M, SEA_NORM);
        assert!(
            (inland - bluff).abs() < 1e-6,
            "100 m inland is past the taper and must be untouched, got {inland}"
        );
    }

    #[test]
    fn island_stays_dry_and_far_land_stays_uncarved() {
        let r = test_region();
        let m = masks();
        // The island centre (1000, 100) sits ~50 m from the lake on every
        // side: the beach blend MAY lower it, but never below its lake's
        // waterline (it must stay a dry island, not a flooded shoal).
        let island_c =
            carve_normalized_with(&m, dir_at(&r, 1000.0, 100.0), 0.3, MIN_M, MAX_M, SEA_NORM);
        let lake_surface = SEA_NORM + 50.0 / (MAX_M - MIN_M);
        assert!(
            island_c > lake_surface,
            "island ground must stay above its lake's waterline: {island_c} vs {lake_surface}"
        );
        for (e, n, what) in [
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
