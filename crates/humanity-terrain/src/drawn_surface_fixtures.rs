//! The Silverdale fixtures of the region tests (BUG-156, drawn_surface_region_tests.rs), which the
//! simulation's walk test uses too (systems::crossing_tests::drawn_surface_region): the shipped
//! Earth, the Silverdale masks, the probe grid and the built patches the ground is read from. Apart
//! from the tests so that the crate split can put them behind its `test-support` feature (rung 2).
//! A `#[path]` child of `drawn_surface`, in its own file because the parent is on the file-size
//! ratchet.

use super::*;
use crate::ocean_mask::OceanMask;
use crate::osm_region::{
    latlon_to_dir_f64, parse_dem, parse_region, region_meters_to_latlon, M_PER_DEG_LAT,
    M_PER_DEG_LON_EQUATOR,
};
use crate::planet_chunks::{build_patch_mesh, patch_corners, PatchId, PatchMesh};
use crate::planet_heightmap::PlanetHeightmap;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The shipped Earth grids, ocean mask and the real earth.ron (not the test
/// fixture, whose surface relief is 3.5x the shipped one), with the loader's
/// sea-level override: the planet the game draws.
pub fn real_earth() -> (PlanetHeightmap, OceanMask, PlanetDef) {
    let base = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join("planets");
    let hm = PlanetHeightmap::load(&base.join("earth_heightmap.bin")).expect("earth heightmap loads");
    let om = OceanMask::load(&base.join("earth_ocean_mask.bin")).expect("ocean mask loads");
    let text = std::fs::read_to_string(base.join("earth.ron")).expect("earth.ron reads");
    let mut def: PlanetDef = ron::from_str(&text).expect("earth.ron parses");
    def.sea_level = hm.sea_level_normalized();
    (hm, om, def)
}

/// The shipped Silverdale region with its survey DEM, rasterised the way
/// `engine::region_meshes` does it when the region loads (lake levels from the
/// lowest survey sample on each shore ring). Built once per test binary: a
/// 1024x1024 rasterisation with two distance transforms is seconds in debug.
pub fn silverdale_masks() -> Arc<Vec<RegionMask>> {
    static MASKS: OnceLock<Arc<Vec<RegionMask>>> = OnceLock::new();
    MASKS
        .get_or_init(|| {
            let dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/maps/regions");
            let region = parse_region(&std::fs::read(dir.join("silverdale.bin")).expect("region ships"))
                .expect("region parses");
            let dem = parse_dem(&std::fs::read(dir.join("silverdale.dem.bin")).expect("DEM ships"))
                .expect("DEM parses");
            let lake_level = |wi: usize| -> f64 {
                let low = region.water[wi]
                    .ring
                    .iter()
                    .filter_map(|&(e, n)| {
                        let (lat, lon) =
                            region_meters_to_latlon(region.origin_lat, region.origin_lon, e as f64, n as f64);
                        dem.sample_m(lat, lon).map(|m| m as f64)
                    })
                    .fold(f64::MAX, f64::min);
                if low == f64::MAX {
                    0.5
                } else {
                    low.max(0.5)
                }
            };
            let mask = RegionMask::from_region_with_dem(&region, &lake_level, Some(dem.clone()))
                .expect("Silverdale has water, roads and buildings to mask");
            Arc::new(vec![mask])
        })
        .clone()
}

/// The `silverdale-home-marker` vantage's standing point (tests/visual/
/// vantages.json): 200 m north of 47.645 N, 122.695 W, on the slope above the
/// Dyes Inlet waterfront.
pub const STAND_LAT: f64 = 47.645 + 200.0 / M_PER_DEG_LAT;
pub const STAND_LON: f64 = -122.695;

/// A direction `east_m`, `north_m` metres from the standing point.
pub fn near_stand(east_m: f64, north_m: f64) -> DVec3 {
    let m_per_lon = M_PER_DEG_LON_EQUATOR * STAND_LAT.to_radians().cos();
    latlon_to_dir_f64(STAND_LAT + north_m / M_PER_DEG_LAT, STAND_LON + east_m / m_per_lon)
}

/// The patch of `depth` whose spherical triangle contains `dir`.
pub fn patch_containing(dir: DVec3, depth: u8) -> PatchId {
    let inside = |id: &PatchId, d: DVec3| -> bool {
        let c = patch_corners(id);
        let en = [c[0].cross(c[1]), c[1].cross(c[2]), c[2].cross(c[0])];
        let es = [en[0].dot(c[2]), en[1].dot(c[0]), en[2].dot(c[1])];
        (0..3).all(|i| en[i].dot(d) * es[i] >= 0.0)
    };
    let d = dir.normalize();
    let mut id = (0..20u8).map(PatchId::root).find(|r| inside(r, d)).expect("a root face holds it");
    while id.depth < depth {
        id = (0..4u32).map(|c| id.child(c)).find(|ch| inside(ch, d)).expect("a child holds it");
    }
    id
}

/// Where a ray from the planet centre along `dir` leaves the built patch's
/// GROUND triangles (the first PATCH_TESS^2; the cards and the skirt follow).
pub fn drawn_radius_along(pm: &PatchMesh, dir: DVec3) -> Option<f64> {
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

/// A cache of built patches, so a probe grid pays for each patch once.
pub struct Meshes<'s, 'a> {
    pub def: &'s PlanetDef,
    pub src: &'s ElevationSource<'a>,
    pub built: HashMap<PatchId, PatchMesh>,
}

impl Meshes<'_, '_> {
    pub fn ground(&mut self, dir: DVec3, depth: u8) -> Option<f64> {
        let id = patch_containing(dir, depth);
        let (def, src) = (self.def, self.src);
        let pm = self.built.entry(id).or_insert_with(|| build_patch_mesh(def, src, None, &id));
        drawn_radius_along(pm, dir)
    }
}

/// A probe grid over the slope the vantage looks across: 25 m steps out to
/// 150 m, which reaches from the shore taper up into the trees.
pub fn probe_grid() -> Vec<DVec3> {
    let mut out = Vec::new();
    for i in -6..=6 {
        for j in -6..=6 {
            out.push(near_stand(i as f64 * 25.0 + 3.1, j as f64 * 25.0 + 1.7));
        }
    }
    out
}
