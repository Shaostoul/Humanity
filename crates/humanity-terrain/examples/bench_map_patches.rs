//! How long a terrain patch takes to build inside an installed map pack, by depth (whole-Earth
//! maps, the rung between 2 and 3, 2026-11-09; docs/design/whole-earth-maps.md section 13, that
//! rung's outcome). Release build, the shipped Earth and home packs, the patches under downtown
//! Silverdale; each depth built twice (cold: the bed tiles and land tiles decoded on first use;
//! warm: what a rebuild costs).
//!
//!   cargo run --release -p humanity-terrain --features test-support --example bench_map_patches
//!   HUMANITY_GROUND_BEDS=0 cargo run ...   the same without road beds and building pads
//!
//! A developer measurement, never part of the app.

use humanity_terrain::drawn_surface::fixtures::real_earth;
use humanity_terrain::map_ground;
use humanity_terrain::map_tiles::tile_containing;
use humanity_terrain::planet_chunks::{build_patch_mesh, DetailNoise, ElevationSource, PatchId};
use std::time::Instant;

fn main() {
    let (hm, om, def) = real_earth();
    let set = map_ground::home_set().expect("the home packs ship in data/maps/packs");
    map_ground::publish(set.clone());
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
    println!("beds {}", if map_ground::beds_enabled() { "on" } else { "OFF" });
    // Downtown Silverdale, just north of the waterfront: streets and houses on a slope.
    let (lat, lon) = (47.6470, -122.6960);
    for depth in [13u8, 15, 17, 18, 19, 20] {
        // The 64 patches of this depth under its ancestor three levels up (a 4 x 4 x 4 block).
        let top = tile_containing(lat, lon, depth - 3);
        let mut ids: Vec<PatchId> = vec![top];
        for _ in 0..3 {
            ids = ids.iter().flat_map(|p| (0..4).map(move |i| p.child(i))).collect();
        }
        let mut times = [0.0f64; 2];
        let mut verts = 0usize;
        for pass in 0..2 {
            let t = Instant::now();
            for id in &ids {
                let m = build_patch_mesh(&def, &src, None, id);
                if pass == 0 {
                    verts += m.mesh.vertices.len();
                }
            }
            times[pass] = t.elapsed().as_secs_f64() * 1000.0 / ids.len() as f64;
        }
        println!("depth {depth:2}: {} patches, {:.3} ms each cold, {:.3} ms each warm ({} vertices each)", ids.len(), times[0], times[1], verts / ids.len());
    }
    println!("resident decoded tiles (land, sea, cover, built, beds): {}", set.resident());

    // One district's OSM geometry (the level-9 tile holding the waterfront), the way the engine's
    // map_meshes::build_district builds it: every vertex on the finest drawn ground.
    use humanity_terrain::osm_region::{build_region_meshes, dir_to_latlon_f64, region_from_pack, MeshGround};
    use humanity_terrain::planet_chunks::{drawn_elevation_at_depth, patch_corners};
    let id = tile_containing(47.6445, -122.6950, 9);
    let c = patch_corners(&id);
    let (clat, clon) = dir_to_latlon_f64((c[0] + c[1] + c[2]).normalize());
    let tiles = humanity_terrain::map_tiles::descendants_at(id, set.levels.finest());
    let sea_r = def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(&def, def.sea_level.clamp(0.0, 1.0) as f64);
    let t = Instant::now();
    let mut region = humanity_terrain::osm_region::OsmRegion { name: String::new(), origin_lat: clat, origin_lon: clon, half_east_m: 7_000.0, half_north_m: 7_000.0, roads: Vec::new(), buildings: Vec::new(), water: Vec::new() };
    for ps in &set.packs {
        if humanity_terrain::map_tiles::ancestor_at(id, ps.pack.patch().depth) != Some(ps.pack.patch()) {
            continue;
        }
        let part = region_from_pack(&ps.pack, &tiles, "", clat, clon, 7_000.0, 7_000.0, &|_| None);
        region.roads.extend(part.roads);
        region.buildings.extend(part.buildings);
        region.water.extend(part.water);
    }
    let cut = t.elapsed().as_secs_f64();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let elev = |d: glam::DVec3| -> f64 {
        calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let e = drawn_elevation_at_depth(&hm, &def, &dn, None, Some(&set), d, 24);
        (def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(&def, e as f64)).max(sea_r)
    };
    let natural = |d: glam::DVec3| set.raw_land_m(d).map_or_else(|| elev(d), |(h, _)| (sea_r + h).max(sea_r));
    let t = Instant::now();
    let out = build_region_meshes(&region, &MeshGround { elev: &elev, natural: &natural, beds: map_ground::beds_enabled() });
    let tris: usize = out.classes.iter().map(|c| c.indices.len() / 3).sum();
    println!(
        "district {}/{}: {} roads, {} buildings cut in {:.2} s; meshed in {:.2} s ({} ground calls, {:.1} us each; {} triangles)",
        id.depth,
        id.path,
        region.roads.len(),
        region.buildings.len(),
        cut,
        t.elapsed().as_secs_f64(),
        calls.load(std::sync::atomic::Ordering::Relaxed),
        t.elapsed().as_secs_f64() * 1e6 / calls.load(std::sync::atomic::Ordering::Relaxed).max(1) as f64,
        tris
    );
}
