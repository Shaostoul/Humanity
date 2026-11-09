//! The map ground's tests (whole-earth-maps.md 5.2 and 5.5, rung 2), run against the SHIPPED home
//! packs (data/maps/packs) through the very surface the player stands on (`DrawnPatchSurface`, the
//! walk oracle, at the two depths the game draws: 13 for a release without the tile tier, 20 with
//! it). They are the shore probe's rules in miniature, so the rig gate's red and green can be seen
//! without booting the game. Each was seen red; the switch or mutation and what it printed are in
//! its comment.

use super::*;
use crate::drawn_surface::fixtures::real_earth;
use crate::drawn_surface::DrawnPatchSurface;
use crate::osm_region::latlon_to_dir_f64;
use crate::planet_chunks::{DetailNoise, ElevationSource};

fn set() -> Arc<MapSet> {
    home_set().expect("the home map packs ship in data/maps/packs")
}

/// The ground (metres above the sea) every `step_m` along a great circle, through the walk oracle
/// at `depth`, with each sample's class from the pack.
fn transect(a: (f64, f64), b: (f64, f64), step_m: f64, depth: u8) -> Vec<(f64, Option<WaterRingKind>)> {
    let (hm, om, def) = real_earth();
    let dn = DetailNoise::new(def.terrain_seed);
    let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
    let s = set();
    let mut surf = DrawnPatchSurface::new(&def, &src, depth).with_map(Some(s.clone()));
    let sea_r = def.radius * crate::planet_surface::displaced_radius_f64_true(&def, def.sea_level.clamp(0.0, 1.0) as f64);
    let (pa, pb) = (latlon_to_dir_f64(a.0, a.1), latlon_to_dir_f64(b.0, b.1));
    let ang = pa.angle_between(pb);
    let n = (ang * def.radius / step_m).ceil() as usize;
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let d = (pa * ((1.0 - t) * ang).sin() + pb * (t * ang).sin()).normalize();
            (surf.radius_at(d) - sea_r, s.class_at(d).and_then(|c| c.0))
        })
        .collect()
}

/// The shore rules on a land-to-water transect: (worst wet sample past 15 m offshore, lowest dry
/// sample past 15 m inland, worst 10 m rise within 60 m of the coast).
fn shore_rules(samples: &[(f64, Option<WaterRingKind>)], step_m: f64) -> (f64, f64, f64) {
    let coast = samples.iter().rposition(|s| s.1.is_none()).expect("the land end is land") as f64 * step_m + step_m * 0.5;
    let (mut wet, mut dry, mut wall) = (f64::MIN, f64::MAX, 0.0f64);
    let k = (10.0 / step_m).round() as usize;
    for (i, s) in samples.iter().enumerate() {
        let at = i as f64 * step_m;
        if at - coast > 15.0 {
            wet = wet.max(s.0);
        }
        if coast - at > 15.0 {
            dry = dry.min(s.0);
        }
        // Going inland is going back along the transect.
        if (at - coast).abs() < 60.0 && i >= k {
            wall = wall.max(samples[i - k].0 - s.0);
        }
    }
    (wet, dry, wall)
}

/// THE WATERFRONT THE OPERATOR REPORTED TWICE (coast-silverdale-waterfront's own transect, 140 m
/// inland to 400 m out on longitude -122.695): water right up to the shore, the ground walking
/// down into it, at both depths the game draws. Seen red 2026-11-09 with the shore band off
/// (`band_on: false`, what `HUMANITY_SHORE_BAND=0` does): GEDTM30's land met ETOPO's seabed raw at
/// the coastline, "depth 13: a sample more than 15 m offshore stands at +0.99 m (a dry harbour)".
#[test]
fn the_silverdale_waterfront_is_wet_dry_and_walks_into_the_water() {
    for depth in [13u8, 20] {
        let s = transect((47.6450, -122.6950), (47.6400, -122.6950), 2.5, depth);
        let (wet, dry, wall) = shore_rules(&s, 2.5);
        println!("[shore] Silverdale waterfront depth {depth}: wettest past 15 m out {wet:+.2} m, driest past 15 m in {dry:+.2} m, steepest 10 m near the coast {wall:.2} m");
        assert!(wet <= -0.3, "depth {depth}: a sample more than 15 m offshore stands at {wet:+.2} m (a dry harbour)");
        assert!(dry >= 0.3, "depth {depth}: a sample more than 15 m inland stands at {dry:+.2} m (a drowned shore)");
        assert!(wall < 2.5, "depth {depth}: the ground rises {wall:.2} m in 10 m at the waterline (a wall)");
    }
}

/// THE OLD BOX EDGE (coast-silverdale-old-edge's meridian, -122.658, along the water across
/// latitude 47.590 where the region rectangle's carve and DEM used to stop; lengthened here to
/// 1.3 km so it also crosses level-12 tile borders, the seams a tile grid could make): one sheet of
/// water with an even bed. The rung 1 build stepped 16 m in 2.5 m at depth 20 (the probe's
/// numbers). Seen red 2026-11-09 with the tile borders' clip edges taken as shoreline
/// (`if true || !r.clip...` in the shore field): "depth 20: the bed rises to +0.90 m" (the shore
/// band drawn along a tile border in the middle of the inlet: the old box edge's seam, reborn).
#[test]
fn the_old_box_edge_is_one_sheet_of_water() {
    let set = set();
    // Where the transect crosses a level-12 tile border (it must, past its first 150 m: the
    // meridian passes about 20 m from a point of land near 47.599, where the shore band rightly
    // rises to the waterline).
    let tile_at = |lat: f64| -> u64 {
        let d = latlon_to_dir_f64(lat, -122.658);
        set.packs.iter().find(|p| p.holds(d)).map(|p| p.descend(d, 12).last().unwrap().0.path).unwrap()
    };
    let crossing = (61..=480).find(|i| tile_at(47.6000 - 0.000025 * *i as f64) != tile_at(47.6000 - 0.000025 * (*i - 1) as f64));
    assert!(crossing.is_some_and(|i| i > 60), "the transect crosses a level-12 tile border past 150 m ({crossing:?})");
    for depth in [13u8, 20] {
        let s = transect((47.6000, -122.6580), (47.5880, -122.6580), 2.5, depth);
        assert!(s.iter().all(|x| x.1 == Some(WaterRingKind::Sea)), "depth {depth}: every sample is the sea");
        let step = s.windows(2).map(|w| (w[1].0 - w[0].0).abs()).fold(0.0, f64::max);
        let high = s.iter().skip(60).map(|x| x.0).fold(f64::MIN, f64::max);
        println!("[shore] old box edge depth {depth}: largest step {step:.3} m, highest bed past 150 m {high:+.2} m");
        assert!(step < 0.5, "depth {depth}: a {step:.2} m step in 2.5 m along the water");
        // The fine mesh is the field: open water stays wet (the coarse one only must not step).
        if depth == 20 {
            assert!(high < -0.3, "depth {depth}: the bed rises to {high:+.2} m");
        }
    }
}

/// A lattice vertex well inland reads the pack's own sample exactly (section 4.2: no
/// interpolation at a vertex). Seen red 2026-11-09 before the vertex snap in `lattice_value`:
/// "inland vertex reads 38.799997 m, the pack says 38.799999 m" (a 27 m cell's barycentric
/// weights carry about 1e-6 of error, so a vertex read 0.9999993 of its own sample).
#[test]
fn a_lattice_vertex_reads_its_sample() {
    let s = set();
    let ps = &s.packs[0];
    let field = ps.shore(&s);
    let t = *ps.descend(latlon_to_dir_f64(47.55, -122.60), 12).last().unwrap();
    let tile = t.0;
    let dirs = crate::map_tiles::tile_lattice(&tile, 64).unwrap();
    let hs = ps.pack.heights(ChunkKind::LandH, 12, tile.path).unwrap();
    let mut checked = 0;
    for (i, d) in dirs.iter().enumerate() {
        if field.nearest(field.plane.to(*d), 650.0).is_some() || field.class(&t, *d).is_some() || i % 7 != 0 {
            continue;
        }
        let g = s.ground(*d, 14).expect("inside the pack");
        assert_eq!(g.h_m, hs[i] as f64, "inland vertex {i} reads {:.6} m, the pack says {:.6} m", g.h_m, hs[i]);
        checked += 1;
    }
    println!("[lattice] {checked} inland vertices of tile {} read their samples", tile.path);
    assert!(checked > 20, "only {checked} inland vertices checked");
}

/// Residency: a tile decoded on use is dropped once no patch has needed it for EVICT_FRAMES
/// frames, and a fresh generation (the reuse switch) starts with nothing decoded. Seen red
/// 2026-11-09 with `evict_if_older` comparing `used + EVICT_FRAMES > frame` (evicting the busy
/// instead of the idle): "a tile the selection still reads stays".
#[test]
fn tiles_leave_when_no_patch_needs_them() {
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let packs = load_packs(&data.join("maps").join("packs"));
    let s = MapSet::new(MapLevels::load(&data).unwrap(), packs);
    let d = latlon_to_dir_f64(47.55, -122.60);
    let ps = s.pack_of(d).unwrap();
    assert!(ps.land_h(&s, d, 12).is_some());
    assert!(s.resident() >= 1);
    let tile = ps.descend(d, 14).last().unwrap().0;
    for _ in 0..700 {
        s.tick(std::iter::once(tile));
    }
    assert!(s.resident() >= 1, "a tile the selection still reads stays");
    for _ in 0..700 {
        s.tick(std::iter::empty());
    }
    assert_eq!(s.resident(), 0, "after 700 idle frames {} tile(s) are still decoded", s.resident());
    assert!(ps.land_h(&s, d, 12).is_some());
    assert_eq!(s.fresh_generation().resident(), 0, "a fresh generation decodes everything again");
}

/// Across the border the two home packs share (it runs through Silverdale's old rectangle), the
/// distance to the nearest shore is continuous: each pack's shore index holds only its own tiles'
/// edges, so the reading takes every installed pack within reach. Checked at points 0.6 m either
/// side of the shared edge every 50 m along it. Seen red 2026-11-09 with `nearest_shore` reading
/// only the home pack (the first build of this rung did): "the shore distance jumps 579.6 m across
/// the pack border", a step in the ground wherever a shore lies within reach of the border.
#[test]
fn the_shore_is_continuous_across_a_pack_border() {
    let s = set();
    assert!(s.packs.len() >= 2, "the two home packs ship");
    let (a, b) = (&s.packs[0], &s.packs[1]);
    let shared: Vec<DVec3> = a.corners.iter().copied().filter(|c| b.corners.iter().any(|d| (*c - *d).length() < 1e-12)).collect();
    assert_eq!(shared.len(), 2, "the home packs share an edge");
    let n = a.normals.iter().copied().find(|n| n.dot(shared[0]).abs() < 1e-9 && n.dot(shared[1]).abs() < 1e-9).expect("the shared edge's normal");
    let steps = (shared[0].angle_between(shared[1]) * R / 50.0) as usize;
    let (mut worst, mut near_water) = (0.0f64, 0usize);
    for i in 1..steps {
        let t = i as f64 / steps as f64;
        let m = (shared[0] * (1.0 - t) + shared[1] * t).normalize();
        let (ga, gb) = (s.ground((m + n * 1e-7).normalize(), 20), s.ground((m - n * 1e-7).normalize(), 20));
        let (Some(ga), Some(gb)) = (ga, gb) else { continue };
        if ga.shore_m.abs() < s.levels.shore.reach_m || gb.shore_m.abs() < s.levels.shore.reach_m {
            near_water += 1;
            worst = worst.max((ga.shore_m - gb.shore_m).abs());
        }
    }
    println!("[shore] across the home packs' border: {near_water} points within reach of a shore, the shore distance jumps at most {worst:.2} m");
    assert!(near_water > 10, "only {near_water} border points near a shore: the test measures nothing");
    assert!(worst < 2.0, "the shore distance jumps {worst:.1} m across the pack border");
}

/// Every vertex of the drawn mesh over the open Sound west of Seattle stands under the water, the
/// ones on a level-12 tile border included. Those sit exactly on the edges of the rings cut at that
/// border, whose corners are rounded to OSM's 1e-7 degree integers, and the even-odd test read
/// about half of them as land: the ground there took the land layer (GEDTM30 reads 0 over water),
/// so a ridge rose from 190 m down to the surface along every tile border in the Sound, the
/// straight dark lines with stars at the tile corners in the 5 km capture over Seattle (and the
/// pale stepped streaks near the Silverdale waterfront, where the water is shallow). The vertices
/// here are the 16-segment lattices of the depth-13 patches over a 4.5 by 1.5 km box of open water
/// (ETOPO 150 to 200 m deep, no shore within 600 m). Seen red 2026-11-09 with `EDGE_NUDGE_M` at
/// 0 (the test-as-written before it): "174 of 3060 vertices of the drawn mesh in the open Sound read
/// land or stand within 20 m of the surface; the highest at 47.594045, -122.428793 stands at +0.00 m
/// (None)".
#[test]
fn vertices_on_tile_borders_stand_in_the_water_they_lie_in() {
    let s = set();
    let mut patches = std::collections::BTreeSet::new();
    for i in 0..=15 {
        for j in 0..=5 {
            let d = latlon_to_dir_f64(47.585 + i as f64 * 0.0027, -122.44 + j as f64 * 0.0027);
            patches.insert(crate::map_tiles::tile_containing_dir(d, 13));
        }
    }
    let (mut vertices, mut on_border, mut wrong) = (0usize, 0usize, Vec::new());
    for id in &patches {
        for v in crate::map_tiles::lattice_from_corners(&patch_corners(id), 16).unwrap() {
            let ps = s.packs.iter().find(|p| p.holds(v)).expect("inside a home pack");
            let t12 = *ps.descend(v, 12).last().unwrap();
            if firmness(&t12.1, v).abs() < 1e-12 {
                on_border += 1;
            }
            let g = s.ground(v, 13).expect("inside the pack");
            vertices += 1;
            if g.water != Some(WaterRingKind::Sea) || g.h_m > -20.0 {
                wrong.push((v, g.h_m, g.water));
            }
        }
    }
    println!("[border] {} depth-13 patches, {vertices} vertices ({on_border} on a level-12 tile border), {} read land or shallower than 20 m", patches.len(), wrong.len());
    assert!(on_border > 50, "only {on_border} vertices on a tile border: the test measures nothing");
    if let Some((v, h, w)) = wrong.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)) {
        let (lat, lon) = crate::osm_region::dir_to_latlon_f64(v);
        panic!("{} of {vertices} vertices of the drawn mesh in the open Sound read land or stand within 20 m of the surface; the highest at {lat:.6}, {lon:.6} stands at {h:+.2} m ({w:?})", wrong.len());
    }
}
