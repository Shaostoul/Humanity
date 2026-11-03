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
use super::fixtures::*;
use crate::planet_chunks::{
    near_tree_instances_on_drawn,
    tree_flare_radius_m, DetailNoise,
};
use crate::water_carve::PublishedForTest;
use std::collections::HashMap;

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
