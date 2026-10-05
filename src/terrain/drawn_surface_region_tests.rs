//! BUG-156: the drawn-ground contract INSIDE A REGION WITH A SURVEY DEM.
//!
//! `DrawnPatchSurface` exists so that whatever stands on the ground (a tree's
//! base, a grass blade, the player's eye) stands on the triangle that is DRAWN,
//! not on a second opinion about it. Its own gates (the parent module's tests,
//! the grass twin, `surface_walk`'s eye gate) all run at Fuji, the Amazon, the
//! Alps: places no OSM region covers. Inside a region the drawn ground is not
//! the base grid at all. The water carve presses it down under the sea and
//! lake polygons, and since v0.1153 it REPLACES the elevation with the region's
//! ~13 m survey DEM. `build_patch_mesh` applied both; this surface applied
//! neither, so in Silverdale every tree, blade and the player's feet stood on
//! the ~5.5 km grid plus detail noise, metres from the ground they were drawn
//! over: the trees the probe rig caught hanging in the air beside the Dyes
//! Inlet waterfront.
//!
//! Every measurement here is against the REAL BUILT PATCH MESH (ray cast into
//! `build_patch_mesh`'s triangles, with the region's masks published the way
//! `engine::region_meshes` publishes them), never against this surface itself.
//! Each test also proves the region genuinely moves the ground at the place it
//! measures, so none of them can pass by measuring nowhere in particular.
//!
//! A `#[path]` child of `drawn_surface`, in its own file because the parent is
//! on the file-size ratchet.

use super::*;
use crate::surface_walk::{rest_radius, walk_ground_radius, DRAWN_CLEARANCE_M, EYE_HEIGHT_M};
use crate::terrain::ocean_mask::OceanMask;
use crate::terrain::osm_region::{
    latlon_to_dir_f64, parse_dem, parse_region, region_meters_to_latlon, M_PER_DEG_LAT,
    M_PER_DEG_LON_EQUATOR,
};
use crate::terrain::planet_chunks::{
    build_patch_mesh, drawn_elevation_normalized, near_tree_instances_on_drawn, patch_corners,
    tree_flare_radius_m, DetailNoise, PatchId, PatchMesh,
};
use crate::terrain::planet_heightmap::PlanetHeightmap;
use crate::terrain::water_carve::{self, PublishedForTest};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The shipped Earth grids, ocean mask and the real earth.ron (not the test
/// fixture, whose surface relief is 3.5x the shipped one), with the loader's
/// sea-level override: the planet the game draws.
fn real_earth() -> (PlanetHeightmap, OceanMask, PlanetDef) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
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
fn silverdale_masks() -> Arc<Vec<RegionMask>> {
    static MASKS: OnceLock<Arc<Vec<RegionMask>>> = OnceLock::new();
    MASKS
        .get_or_init(|| {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/maps/regions");
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
const STAND_LAT: f64 = 47.645 + 200.0 / M_PER_DEG_LAT;
const STAND_LON: f64 = -122.695;

/// A direction `east_m`, `north_m` metres from the standing point.
fn near_stand(east_m: f64, north_m: f64) -> DVec3 {
    let m_per_lon = M_PER_DEG_LON_EQUATOR * STAND_LAT.to_radians().cos();
    latlon_to_dir_f64(STAND_LAT + north_m / M_PER_DEG_LAT, STAND_LON + east_m / m_per_lon)
}

/// The patch of `depth` whose spherical triangle contains `dir`.
fn patch_containing(dir: DVec3, depth: u8) -> PatchId {
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

/// A cache of built patches, so a probe grid pays for each patch once.
struct Meshes<'s, 'a> {
    def: &'s PlanetDef,
    src: &'s ElevationSource<'a>,
    built: HashMap<PatchId, PatchMesh>,
}

impl Meshes<'_, '_> {
    fn ground(&mut self, dir: DVec3, depth: u8) -> Option<f64> {
        let id = patch_containing(dir, depth);
        let (def, src) = (self.def, self.src);
        let pm = self.built.entry(id).or_insert_with(|| build_patch_mesh(def, src, None, &id));
        drawn_radius_along(pm, dir)
    }
}

/// A probe grid over the slope the vantage looks across: 25 m steps out to
/// 150 m, which reaches from the shore taper up into the trees.
fn probe_grid() -> Vec<DVec3> {
    let mut out = Vec::new();
    for i in -6..=6 {
        for j in -6..=6 {
            out.push(near_stand(i as f64 * 25.0 + 3.1, j as f64 * 25.0 + 1.7));
        }
    }
    out
}

/// THE ROOT OF BUG-156: the drawn-surface sampler IS the drawn mesh inside a
/// DEM region, at the depths a walker sees (17, 3.4 m triangles; 20, 0.4 m).
/// Red before the fix by the whole DEM-versus-grid difference (metres to tens
/// of metres on this slope); the uncarved control proves that difference is
/// really there to catch.
#[test]
fn the_drawn_surface_is_the_drawn_mesh_inside_a_dem_region() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
    let _published = PublishedForTest::publish(silverdale_masks());
    let mut meshes = Meshes { def: &def, src: &src, built: HashMap::new() };
    let (mut worst, mut control, mut n) = (0.0f64, 0.0f64, 0usize);
    for depth in [17u8, 20] {
        // The production constructors, exactly as the tree and grass
        // harvests and the walk clamp build them.
        let mut surf = DrawnPatchSurface::new(&def, &src, depth);
        let mut single = DrawnPatchSurface::new_single_shot(&def, &src, depth);
        let mut uncarved = DrawnPatchSurface::new(&def, &src, depth).with_carve(None);
        for dir in probe_grid().into_iter().step_by(if depth == 20 { 3 } else { 1 }) {
            let Some(mesh_r) = meshes.ground(dir, depth) else { continue };
            worst = worst.max((surf.radius_at(dir) - mesh_r).abs());
            worst = worst.max((single.radius_at(dir) - mesh_r).abs());
            control = control.max((uncarved.radius_at(dir) - mesh_r).abs());
            n += 1;
        }
    }
    println!(
        "[region ground] Silverdale: {n} probes, worst |drawn surface - mesh| {worst:.4} m, \
         worst |uncarved formula - mesh| {control:.2} m"
    );
    assert!(n >= 150, "only {n} probes landed on a built patch");
    assert!(
        control > 2.0,
        "without the region carve the formula is only {control:.2} m off the mesh here, so this \
         place no longer tests the region at all: pick a probe area the DEM moves"
    );
    assert!(
        worst < 0.01,
        "DrawnPatchSurface is {worst:.3} m off the patch mesh it reproduces inside the Silverdale \
         region: it is not applying the region carve and DEM that build_patch_mesh applies \
         (water_carve::snapshot through drawn_elevation_at_depth)"
    );
}

/// The trees the vantage photographed: harvested around its standing point and
/// ray cast against the built patch under each one. The same bar as the Fuji
/// and Amazon gate (`tree_bases_sit_on_the_drawn_surface`): no base floats
/// more than 0.15 m, 95% within 0.30 m, none sunk past half its root flare.
#[test]
fn trees_stand_on_the_drawn_ground_beside_the_silverdale_waterfront() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
    let _published = PublishedForTest::publish(silverdale_masks());
    let depth = 17u8;
    let trees = near_tree_instances_on_drawn(&def, &src, None, near_stand(0.0, 0.0), 220.0, depth, 600);
    assert!(trees.len() >= 30, "only {} trees harvested around the vantage", trees.len());
    let mut meshes = Meshes { def: &def, src: &src, built: HashMap::new() };
    let (mut offs, mut flares) = (Vec::new(), Vec::new());
    for t in &trees {
        if meshes.built.len() >= 64 && !meshes.built.contains_key(&patch_containing(t.dir, depth)) {
            continue;
        }
        if let Some(r) = meshes.ground(t.dir, depth) {
            offs.push(t.r_m - r);
            flares.push(tree_flare_radius_m(t.height_m));
        }
    }
    let mut sorted = offs.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();
    let p95 = sorted[n * 95 / 100].max(-sorted[n * 5 / 100]);
    let floating = offs.iter().filter(|o| **o > 0.15).count();
    let buried = offs.iter().zip(&flares).filter(|(o, f)| **o < -(*f * 0.5)).count();
    println!(
        "[region trees] Silverdale depth {depth}: n={n} min {:+.3} median {:+.3} max {:+.3} \
         floating>0.15m {floating} buried-past-half-flare {buried}",
        sorted[0],
        sorted[n / 2],
        sorted[n - 1]
    );
    assert!(n >= 30, "only {n} trees landed on a built patch");
    assert_eq!(
        floating, 0,
        "{floating} of {n} trees beside the Silverdale waterfront float more than 0.15 m above \
         the drawn ground (worst {:+.2} m): BUG-156, reproduced",
        sorted[n - 1]
    );
    assert_eq!(buried, 0, "{buried} of {n} trees are sunk past half their root flare");
    assert!(p95 < 0.30, "5% of trees sit more than {p95:.3} m off the drawn ground");
}

/// The player's feet, the third client of the same sampler: standing on the
/// Silverdale slope, the eye rests one body height above the ground drawn
/// under it (the walk clamp's own chain: walk_ground_radius, then
/// rest_radius), not above the coarse grid the DEM replaced.
#[test]
fn the_player_stands_on_the_drawn_ground_in_silverdale() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
    let _published = PublishedForTest::publish(silverdale_masks());
    let mut meshes = Meshes { def: &def, src: &src, built: HashMap::new() };
    let want = EYE_HEIGHT_M + DRAWN_CLEARANCE_M;
    let (mut worst, mut n) = (0.0f64, 0usize);
    for depth in [17u8, 20] {
        for dir in probe_grid().into_iter().step_by(4) {
            let Some(mesh_r) = meshes.ground(dir, depth) else { continue };
            // The field sample the engine feeds in as the fallback (it is not
            // the reference while standing on drawn ground).
            let e = drawn_elevation_normalized(&hm, &def, &dn, None, dir);
            let field_r = def.radius * crate::terrain::planet_surface::displaced_radius_f64_true(&def, e as f64);
            let (g, clearance) =
                walk_ground_radius(field_r, Some(&def), Some(&hm), Some(&dn), None, Some(&om), depth, 0.0, dir);
            let eye_above = rest_radius(g, EYE_HEIGHT_M, clearance) - mesh_r;
            worst = worst.max((eye_above - want).abs());
            n += 1;
        }
    }
    println!("[region eye] Silverdale: {n} stands, worst |eye above drawn ground - {want}| {worst:.3} m");
    assert!(n >= 40, "only {n} stands landed on a built patch");
    assert!(
        worst < 0.02,
        "standing in Silverdale the eye is {worst:.2} m away from one body height above the drawn \
         ground: the walk clamp stands the player on a surface the DEM replaced"
    );
    // The registry really is what moved it (a check that could not fail
    // would be worth nothing): the same chain with the masks withdrawn
    // stands somewhere else. The drawn ground is read BEFORE the withdrawal,
    // since a patch built after it would be uncarved too.
    let dir = near_stand(0.0, 0.0);
    let mesh_r = Meshes { def: &def, src: &src, built: HashMap::new() }
        .ground(dir, 20)
        .expect("the standing point has a patch");
    water_carve::clear_global();
    let e = drawn_elevation_normalized(&hm, &def, &dn, None, dir);
    let field_r = def.radius * crate::terrain::planet_surface::displaced_radius_f64_true(&def, e as f64);
    let (g, _) = walk_ground_radius(field_r, Some(&def), Some(&hm), Some(&dn), None, Some(&om), 20, 0.0, dir);
    assert!(
        (g - mesh_r).abs() > 2.0,
        "without the region the walk ground is only {:.2} m from the drawn ground at the standing \
         point: this place no longer tests the region",
        g - mesh_r
    );
}
