//! From terrain/drawn_surface_pack_tests.rs (BUG-156): the player's feet in Silverdale, stood by
//! the simulation's walk clamp (`surface_walk`) on the ground the terrain draws, inside the home map
//! pack (whole-Earth maps rung 2). The Silverdale fixtures stay with terrain's tests
//! (`drawn_surface::pack_tests`), which share them.

use humanity_core::ecs::components::EYE_HEIGHT_M;
use crate::surface_walk::{rest_radius, walk_ground_radius, DRAWN_CLEARANCE_M};
use humanity_terrain::drawn_surface::fixtures::{near_stand, probe_grid, real_earth, silverdale_map, Meshes};
use humanity_terrain::planet_chunks::{drawn_elevation_normalized, DetailNoise, ElevationSource};
use humanity_terrain::map_ground::{self, PublishedForTest};
use std::collections::HashMap;

/// The player's feet, the third client of the same sampler: standing on the
/// Silverdale slope, the eye rests one body height above the ground drawn
/// under it (the walk clamp's own chain: walk_ground_radius, then
/// rest_radius), not above the coarse grid the pack replaced.
#[test]
fn the_player_stands_on_the_drawn_ground_in_silverdale() {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
    let _published = PublishedForTest::publish(silverdale_map());
    let mut meshes = Meshes { def: &def, src: &src, built: HashMap::new() };
    let want = EYE_HEIGHT_M + DRAWN_CLEARANCE_M;
    let (mut worst, mut n) = (0.0f64, 0usize);
    for depth in [17u8, 20] {
        for dir in probe_grid().into_iter().step_by(4) {
            let Some(mesh_r) = meshes.ground(dir, depth) else { continue };
            // The field sample the engine feeds in as the fallback (it is not
            // the reference while standing on drawn ground).
            let e = drawn_elevation_normalized(&hm, &def, &dn, None, dir);
            let field_r = def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(&def, e as f64);
            let (g, clearance) =
                walk_ground_radius(field_r, Some(&def), Some(&hm), Some(&dn), None, Some(&om), depth, 0.0, dir);
            let eye_above = rest_radius(g, EYE_HEIGHT_M, clearance) - mesh_r;
            worst = worst.max((eye_above - want).abs());
            n += 1;
        }
    }
    println!("[pack eye] Silverdale: {n} stands, worst |eye above drawn ground - {want}| {worst:.3} m");
    assert!(n >= 40, "only {n} stands landed on a built patch");
    assert!(
        worst < 0.02,
        "standing in Silverdale the eye is {worst:.2} m away from one body height above the drawn \
         ground: the walk clamp stands the player on a surface the map pack replaced"
    );
    // The registry really is what moved it (a check that could not fail
    // would be worth nothing): the same chain with the packs withdrawn
    // stands somewhere else. The drawn ground is read BEFORE the withdrawal,
    // since a patch built after it would be without the pack too.
    let dir = near_stand(0.0, 0.0);
    let mesh_r = Meshes { def: &def, src: &src, built: HashMap::new() }
        .ground(dir, 20)
        .expect("the standing point has a patch");
    map_ground::clear();
    let e = drawn_elevation_normalized(&hm, &def, &dn, None, dir);
    let field_r = def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(&def, e as f64);
    let (g, _) = walk_ground_radius(field_r, Some(&def), Some(&hm), Some(&dn), None, Some(&om), 20, 0.0, dir);
    assert!(
        (g - mesh_r).abs() > 2.0,
        "without the pack the walk ground is only {:.2} m from the drawn ground at the standing \
         point: this place no longer tests the pack",
        g - mesh_r
    );
}
