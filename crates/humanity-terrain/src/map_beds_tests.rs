//! The road beds' and building pads' tests (whole-earth-maps.md 6.1 to 6.3), run against the
//! SHIPPED home packs through the walk oracle (`DrawnPatchSurface`, the very surface the player
//! stands on and the mesher draws) at depth 20, the depth an installed pack refines to near the
//! camera since this rung. Each picks its streets and footprints from the Silverdale tile by their
//! own slope (no coordinates written in), and each carries its own negative control: the same
//! check on a set with the beds off must FAIL, so a test that cannot fail cannot pass.

use super::*;
use crate::drawn_surface::fixtures::real_earth;
use crate::drawn_surface::DrawnPatchSurface;
use crate::map_ground::{home_set, load_packs};
use crate::map_levels::MapLevels;
use crate::osm_region::{dir_to_latlon_f64, latlon_to_dir_f64};
use crate::planet_chunks::{DetailNoise, ElevationSource};
use std::sync::Arc;

/// Downtown Silverdale, the waterfront vantage's shore (whole-earth-maps.md 5.5).
const SILVERDALE: (f64, f64) = (47.6455, -122.6930);

fn beds_on() -> Arc<MapSet> {
    home_set().expect("the home map packs ship in data/maps/packs")
}

/// The same packs with the beds and pads off: the red run, as `HUMANITY_GROUND_BEDS=0` makes it.
fn beds_off() -> Arc<MapSet> {
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let mut s = MapSet::new(MapLevels::load(&data).unwrap(), load_packs(&data.join("maps").join("packs")));
    s.beds_on = false;
    Arc::new(s)
}

/// The ground (metres above the sea) at `d` through the walk oracle at `depth`.
fn walk(set: &Arc<MapSet>, depth: u8) -> impl FnMut(DVec3) -> f64 {
    let (hm, om, def) = real_earth();
    let sea_r = def.radius * crate::planet_surface::displaced_radius_f64_true(&def, def.sea_level.clamp(0.0, 1.0) as f64);
    let set = set.clone();
    move |d: DVec3| {
        let dn = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap { hm: &hm, detail: &dn, tiles: None, ocean: Some(&om) };
        DrawnPatchSurface::new(&def, &src, depth).with_map(Some(set.clone())).radius_at(d) - sea_r
    }
}

/// The level-12 tile holding Silverdale, its pack, and its bed tile.
fn silverdale_tile(set: &MapSet) -> (usize, PatchId, Arc<BedTile>) {
    let d = latlon_to_dir_f64(SILVERDALE.0, SILVERDALE.1);
    let pi = set.packs.iter().position(|p| p.holds(d)).expect("Silverdale is in a home pack");
    let tile = set.packs[pi].descend(d, set.levels.finest()).last().unwrap().0;
    let bt = set.packs[pi].bed_tile(set, &tile).expect("a bed tile");
    (pi, tile, bt)
}

/// The point `off` metres along the unit tangent `t` from `c`.
fn off(c: DVec3, t: DVec3, m: f64) -> DVec3 {
    (c + t * (m / R)).normalize()
}

/// Up to `n` road pieces of the tile (carriageways, classes 1 to 4, at least 60 m long) whose
/// natural ground falls at least `rise_m` across their carriageway at their middle: (index, the
/// middle point, the direction across, the rise).
fn sloped_roads(set: &MapSet, bt: &BedTile, pi: usize, rise_m: f64, n: usize) -> Vec<(usize, DVec3, DVec3, f64)> {
    let ps = &set.packs[pi];
    let mut out = Vec::new();
    for (i, r) in bt.roads.iter().enumerate() {
        let len: f64 = r.dirs.windows(2).map(|w| (w[1] - w[0]).length() * R).sum();
        if !(1..=4).contains(&r.class) || len < 60.0 || r.dirs.len() < 2 {
            continue;
        }
        let k = (r.dirs.len() - 1) / 2;
        let (a, b) = (r.dirs[k], r.dirs[k + 1]);
        let c = (a + b).normalize();
        let across = c.cross((b - a).normalize()).normalize();
        let (Some(h0), Some(h1)) = (set.land_at(off(c, across, -r.half_w), 12), set.land_at(off(c, across, r.half_w), 12)) else { continue };
        // Keep off the shore band, which wins there by design (its own test is the waterfront's).
        // And where this road alone owns the cross-section: at a junction or across a pad's apron
        // two cores share the ground and blend by design.
        let plane = ps.plane();
        let alone = (0..=8).all(|j| bt.cores_at(set, plane.to(off(c, across, r.half_w * (-1.0 + j as f64 * 0.25)))).len() == 1);
        if alone && set.ground(c, 24).is_some_and(|g| g.shore_m > 80.0) && (h1 - h0).abs() >= rise_m {
            out.push((i, c, across, (h1 - h0).abs()));
        }
        if out.len() == n {
            break;
        }
    }
    out
}

/// The largest spread of the walk oracle across the carriageway (90% of it) of each road.
fn spread_across(set: &Arc<MapSet>, bt: &BedTile, roads: &[(usize, DVec3, DVec3, f64)]) -> Vec<f64> {
    let mut g = walk(set, 20);
    roads
        .iter()
        .map(|&(i, c, across, _)| {
            let h = bt.roads[i].half_w * 0.9;
            let hs: Vec<f64> = (0..=18).map(|j| g(off(c, across, -h + j as f64 * h / 9.0))).collect();
            hs.iter().copied().fold(f64::MIN, f64::max) - hs.iter().copied().fold(f64::MAX, f64::min)
        })
        .collect()
}

/// A street on a side slope lies on a flat bed: across its carriageway the ground the player
/// stands on varies by under 3 cm at depth 20, where the natural ground falls 0.3 m or more.
/// Negative control: with the beds off the same cross-sections follow the slope. Seen red
/// 2026-11-09 with `BedTile::apply` returning the natural ground (the beds wired but inert): "the
/// bed is flat across: spreads [0.476, 0.293, 0.445, 0.374, 0.705, 0.646, 0.589, 0.295, 0.294,
/// 0.469]" (metres); green, every spread 0.001 to 0.003 m.
#[test]
fn a_street_on_a_side_slope_lies_on_a_flat_bed() {
    let on = beds_on();
    let (pi, _, bt) = silverdale_tile(&on);
    let roads = sloped_roads(&on, &bt, pi, 0.3, 10);
    assert!(roads.len() >= 3, "only {} sloped streets found in the Silverdale tile: the test is misconfigured", roads.len());
    let off_set = beds_off();
    let (_, _, bt_off) = silverdale_tile(&off_set);
    let (s_on, s_off) = (spread_across(&on, &bt, &roads), spread_across(&off_set, &bt_off, &roads));
    for (k, (i, c, _, rise)) in roads.iter().enumerate() {
        let (lat, lon) = dir_to_latlon_f64(*c);
        println!("[beds] way {} class {} at {lat:.6}, {lon:.6}: natural rise {rise:.2} m across; drawn spread {:.3} m with beds, {:.3} m without", bt.roads[*i].osm_id, bt.roads[*i].class, s_on[k], s_off[k]);
    }
    assert!(s_on.iter().all(|s| *s < 0.03), "the bed is flat across: spreads {s_on:?}");
    assert!(s_off.iter().any(|s| *s > 0.2), "negative control: without beds the ground follows the slope, spreads {s_off:?}");
}

/// A footprint on a slope stands on a flat pad: at every corner and half an apron outside it the
/// ground is the pad's floor within 3 cm at depth 20, where the natural ground under the corners
/// spreads 0.8 m or more. Negative control: with the pads off the corners follow the slope. Seen
/// red 2026-11-09 with `BedTile::apply` returning the natural ground: "a corner stands off its
/// floor by 1.600 m"; green, every footprint within 0.002 m.
#[test]
fn a_footprint_on_a_slope_stands_on_a_flat_pad() {
    let on = beds_on();
    let (pi, _, bt) = silverdale_tile(&on);
    let ps = &on.packs[pi];
    let mut picked = Vec::new();
    for (i, p) in bt.pads.iter().enumerate() {
        let ring: Vec<DVec3> = p.ring_e7.iter().map(|q| e7_to_dir(q.0, q.1)).collect();
        let nat: Vec<f64> = ring.iter().filter_map(|d| on.land_at(*d, 12)).collect();
        let c = ring.iter().fold(DVec3::ZERO, |a, v| a + *v).normalize();
        let plane = ps.plane();
        let alone = ring.iter().all(|v| {
            let out = (*v - c * v.dot(c)).normalize();
            [*v, off(*v, out, 0.75)].iter().all(|d| bt.cores_at(&on, plane.to(*d)).len() == 1)
        });
        if !alone {
            continue;
        }
        if nat.len() == ring.len() && p.size_m > 6.0 && on.ground(c, 24).is_some_and(|g| g.shore_m > 80.0) {
            let spread = nat.iter().copied().fold(f64::MIN, f64::max) - nat.iter().copied().fold(f64::MAX, f64::min);
            if spread >= 0.8 {
                picked.push((i, ring, c, spread));
            }
        }
        if picked.len() == 10 {
            break;
        }
    }
    assert!(picked.len() >= 3, "only {} sloped footprints found: the test is misconfigured", picked.len());
    let off_set = beds_off();
    let (mut g_on, mut g_off) = (walk(&on, 20), walk(&off_set, 20));
    let (mut worst_on, mut worst_off) = (0.0f64, 0.0f64);
    for (i, ring, c, spread) in &picked {
        let floor = bt.pads[*i].floor(&on).expect("a floor");
        let mut w = 0.0f64;
        let mut w_off = 0.0f64;
        for v in ring {
            let out = (*v - *c * v.dot(*c)).normalize();
            for d in [*v, off(*v, out, 0.75)] {
                w = w.max((g_on(d) - floor).abs());
                w_off = w_off.max((g_off(d) - floor).abs());
            }
        }
        let (lat, lon) = dir_to_latlon_f64(*c);
        println!("[pads] building {} at {lat:.6}, {lon:.6}: natural spread {spread:.2} m; floor {floor:.2} m; corners off it by {w:.3} m with pads, {w_off:.3} m without", bt.pads[*i].osm_id);
        worst_on = worst_on.max(w);
        worst_off = worst_off.max(w_off);
    }
    assert!(worst_on < 0.03, "a corner stands off its floor by {worst_on:.3} m");
    assert!(worst_off > 0.3, "negative control: without pads a corner stands off the floor by only {worst_off:.3} m");
}

/// THE IDENTITY RULE: where a road crosses a level-12 tile border, its two pieces are pinned to the
/// same natural ground at the shared crossing, and the ground along the road there is the same
/// whichever tile's beds evaluate it, with no step. Seen red 2026-11-09 with the end pins and the
/// grade passes removed from `profile_of`: "a step of 0.058 m along the road at the border" (the
/// two pieces' averaged beds meet the border at different heights); green, the worst step 0.046 m
/// in 0.25 m (a 18% street's own climb), and both tiles agree to 1e-6 m everywhere.
#[test]
fn a_road_meets_itself_across_a_tile_border() {
    let set = beds_on();
    let (pi, tile, bt) = silverdale_tile(&set);
    let ps = &set.packs[pi];
    let plane = ps.plane();
    let mut checked = 0;
    for r in bt.roads.iter().filter(|r| r.end_clipped && r.class <= 4 && r.dirs.len() >= 2) {
        let p = *r.dirs.last().unwrap();
        // The tile on the other side, and its piece starting at the same point.
        let a = r.dirs[r.dirs.len() - 2];
        let beyond = (p + (p - a).normalize() * (1.0 / R)).normalize();
        let other = crate::map_tiles::tile_containing_dir(beyond, tile.depth);
        if other == tile || crate::map_tiles::ancestor_at(other, ps.id.depth) != Some(ps.id) {
            continue;
        }
        let bt2 = ps.bed_tile(&set, &other).unwrap();
        let Some(r2) = bt2.roads.iter().find(|q| q.osm_id == r.osm_id && q.start_clipped && (q.dirs[0] - p).length() * R < 0.05) else { continue };
        let k = r.dirs.len() - 2;
        let (e1, e2) = (r.target(&set, k, 1.0).unwrap(), r2.target(&set, 0, 0.0).unwrap());
        let nat = set.land_at(p, 12).unwrap();
        assert!((e1 - nat).abs() < 0.01 && (e2 - nat).abs() < 0.01, "both pieces are pinned to the natural ground at the crossing: {e1:.3}, {e2:.3}, natural {nat:.3}");
        // Along the road from 6 m before the border to 6 m after, through both tiles' beds.
        let dir = (r2.dirs[1] - r2.dirs[0]).normalize();
        let mut prev: Option<f64> = None;
        let mut worst = 0.0f64;
        for j in -24..=24 {
            let q = off(p, dir, j as f64 * 0.25);
            let n = set.land_at(q, 12).unwrap();
            let (h1, h2) = (bt.apply(&set, plane.to(q), n, 0.1), bt2.apply(&set, plane.to(q), n, 0.1));
            assert!((h1 - h2).abs() < 1e-6, "the two tiles disagree at {:.2} m from the border: {h1:.4} and {h2:.4}", j as f64 * 0.25);
            if let Some(pv) = prev {
                worst = worst.max((h1 - pv).abs());
            }
            prev = Some(h1);
        }
        let (lat, lon) = dir_to_latlon_f64(p);
        println!("[beds] way {} crosses the tile border at {lat:.6}, {lon:.6}: pinned at {nat:.3} m, worst step along it {worst:.4} m in 0.25 m", r.osm_id);
        assert!(worst < 0.05, "a step of {worst:.3} m along the road at the border");
        checked += 1;
        if checked == 4 {
            break;
        }
    }
    assert!(checked >= 2, "only {checked} border crossings checked: the test is misconfigured");
}

/// Section 6.3: a bed changes a patch's mesh only where it is wider than the patch's vertex
/// spacing and never above `first_depth`, so a depth-13 or depth-14 patch keeps the natural ground
/// on a street, and a depth-20 patch draws the bed. Seen red 2026-11-09 with the depth gate and the
/// width gate removed: "depth 13 keeps the natural ground: 0.205 m off it"; and with
/// `BedTile::apply` returning the natural ground: "depth 20 draws the beds: they moved the ground
/// by only 0.000 m".
#[test]
fn coarse_patches_keep_the_natural_ground() {
    let (on, off_set) = (beds_on(), beds_off());
    let (pi, _, bt) = silverdale_tile(&on);
    let roads = sloped_roads(&on, &bt, pi, 0.3, 6);
    let mut moved = 0.0f64;
    for &(i, c, across, _) in &roads {
        let d = off(c, across, bt.roads[i].half_w * 0.8);
        for depth in [13u8, 14] {
            let (a, b) = (on.ground(d, depth).unwrap().h_m, off_set.ground(d, depth).unwrap().h_m);
            assert!((a - b).abs() < 1e-9, "depth {depth} keeps the natural ground: {:.3} m off it", (a - b).abs());
        }
        moved = moved.max((on.ground(d, 20).unwrap().h_m - off_set.ground(d, 20).unwrap().h_m).abs());
    }
    assert!(moved > 0.05, "depth 20 draws the beds: they moved the ground by only {moved:.3} m");
}

/// A road's profile keeps to its class's grade (plus the end pins' own small grade) on a long
/// piece. Seen red 2026-11-09 with the grade passes removed from `profile_of`: "way 5915599 climbs
/// at 0.197 where its class allows 0.15".
#[test]
fn a_profile_keeps_to_its_grade() {
    let set = beds_on();
    let (_, _, bt) = silverdale_tile(&set);
    let bp = &set.levels.beds;
    let mut checked = 0;
    for r in bt.roads.iter().filter(|r| r.class <= 4) {
        let len: f64 = r.dirs.windows(2).map(|w| (w[1] - w[0]).length() * R).sum();
        if len < 4.0 * bp.pin_blend_m {
            continue;
        }
        let Some(p) = r.profile.get_or_init(|| profile_of(&set, r)).as_ref() else { continue };
        let allowed = bp.class(r.class).unwrap().grade + bp.pin_grade + 1e-3;
        let mut worst = 0.0f64;
        for k in 0..r.dirs.len() - 1 {
            let seg = (r.dirs[k + 1] - r.dirs[k]).length() * R;
            let step = seg / p.steps[k] as f64;
            for j in 0..p.steps[k] as usize {
                let i = p.start[k] as usize + j;
                worst = worst.max(((p.vals[i + 1] - p.vals[i]) as f64).abs() / step);
            }
        }
        assert!(worst <= allowed, "way {} climbs at {worst:.3} where its class allows {:.2}", r.osm_id, bp.class(r.class).unwrap().grade);
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} long pieces checked: the test is misconfigured");
}





/// PRECEDENCE AT THE WATER (the operator's priority is the coast): on the Silverdale waterfront
/// street, Washington Avenue Northwest (OSM way 1193428234, 10.7 m from Dyes Inlet at its closest),
/// the shore band owns the ground within its band and bounds the bed: every water sample is the same
/// with or without beds, every land sample within the band stays between the band's floor and its cap,
/// and where the cap does not bind the carriageway lies flat. Letting a bed override the cap would
/// stand a street's GEDTM30 height up against the waterline again (Tracyton Boulevard NW stands 6 m
/// up 10 m from the water there: the false wall the band exists to prevent). Seen red 2026-11-09 with
/// the beds applied AFTER the band in `MapSet::ground_with`: "a land sample at 9.0 m from the water
/// stands 0.026 m over the band's cap"; and with `BedTile::apply` returning the natural ground:
/// "where the cap leaves it alone the waterfront street lies flat: spread 0.193 m". Green: 30
/// water samples unchanged, 126 land samples inside the band, the carriageway's free part 0.021 m.
#[test]
fn the_coast_owns_the_waterfront_street() {
    let (on, off_set) = (beds_on(), beds_off());
    let (mut g_on, mut g_off) = (walk(&on, 20), walk(&off_set, 20));
    let r = on.road_near(latlon_to_dir_f64(47.643706, -122.695275)).expect("a road there");
    assert_eq!(r.osm_id, 1193428234, "the waterfront street (Washington Avenue Northwest) is the road there");
    let sh = &on.levels.shore;
    let k = (sh.datum_m + sh.wet_depth_m) / sh.wet_offshore_m;
    let band = sh.min_band_m.max(sh.wet_offshore_m + sh.band_spacings * crate::planet_chunks::vertex_spacing_m(20, R));
    let (mut flat_lo, mut flat_hi, mut n_flat, mut n_band, mut n_water) = (f64::MAX, f64::MIN, 0, 0, 0);
    for j in -80..=80 {
        let m = j as f64 * 0.25;
        let p = off(r.centre, r.across, m);
        let g = on.ground(p, 24).unwrap();
        let (a, b) = (g_on(p), g_off(p));
        if g.water.is_some() {
            assert_eq!(a, b, "a water sample {m:+.2} m from the street's centre moved with the beds");
            n_water += 1;
            continue;
        }
        if g.shore_m < band {
            let cap = sh.datum_m + k * g.shore_m;
            assert!(a <= cap + 1e-3, "a land sample at {:.1} m from the water stands {:.3} m over the band's cap", g.shore_m, a - cap);
            assert!(a >= sh.datum_m - 1e-3, "a land sample at {:.1} m from the water is under the band's floor", g.shore_m);
            n_band += 1;
            if m.abs() <= r.half_w_m && a < cap - 0.005 {
                flat_lo = flat_lo.min(a);
                flat_hi = flat_hi.max(a);
                n_flat += 1;
            }
        }
    }
    println!("[beds] Washington Avenue Northwest: {n_water} water samples unchanged, {n_band} land samples inside the band, {n_flat} carriageway samples the cap leaves alone spread {:.3} m", flat_hi - flat_lo);
    assert!(n_water > 10 && n_band > 10 && n_flat >= 10, "the cross-section reaches the water, the band and the street");
    assert!(flat_hi - flat_lo < 0.03, "where the cap leaves it alone the waterfront street lies flat: spread {:.3} m", flat_hi - flat_lo);
}

/// The vegetation gates and the water shell's depth bake never build a bed tile: no bed can change
/// their answer (a gate asks whether a tree grows; land reads depth 0), and the far forest's gate
/// samples over a city built one per level-12 tile on the main thread (measured over Seattle in
/// rung 2b: 1.6 s a frame on arrival). Seen red 2026-11-09 with `apply_normalized_gate` calling
/// `ground_with`: "the gates built 4 bed tile(s)" (over 2.2 by 1.5 km of downtown Seattle).
#[test]
fn the_vegetation_gates_build_no_bed_tiles() {
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let set = MapSet::new(MapLevels::load(&data).unwrap(), load_packs(&data.join("maps").join("packs")));
    assert!(set.beds_on, "the set under test has its beds on");
    for i in 0..40 {
        for j in 0..40 {
            let d = latlon_to_dir_f64(47.600 + i as f64 * 0.0005, -122.345 + j as f64 * 0.0005);
            crate::map_ground::apply_normalized_gate(&set, d, 0.5, 10_000.0, 0.4);
            set.sea_depth_m(d);
        }
    }
    let built = |s: &MapSet| -> usize { s.packs.iter().map(|p| p.beds.iter().filter(|b| b.loaded()).count()).sum() };
    assert_eq!(built(&set), 0, "the gates built {} bed tile(s)", built(&set));
    // The formula with beds does build one there (the test is not vacuous).
    set.ground(latlon_to_dir_f64(47.6062, -122.3321), 24);
    assert!(built(&set) > 0, "a ground query at depth 24 builds its bed tile");
}


