//! The ship levels' tests (Maps increment 3). Each was seen red first; what broke it and what it
//! said is in its comment.

use super::*;
use crate::ship::fibonacci::HomesteadMeshes;
use crate::ship::home_structure::FloorSlab;

fn shipped() -> ShipStructure {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles at p1")
}

/// Every vertex of every family of `m`, with its normal.
fn all_vertices(m: &HomesteadMeshes) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    let mut take = |vs: &[crate::vertex::Vertex]| {
        out.extend(vs.iter().map(|v| (Vec3::from(v.position), Vec3::from(v.normal))));
    };
    for f in &m.floors {
        take(&f.0);
    }
    for w in &m.material_walls {
        take(&w.0);
    }
    for (v, _) in [&m.walls, &m.trim, &m.windows, &m.mirrors, &m.ceilings, &m.ceilings_opaque] {
        take(v);
    }
    out
}

/// The undersides of `slabs` at a body's corner `o` that a mesh still draws: (the slab's
/// footprint, its underside) for each slab with a downward face at its underside inside it.
fn undersides_drawn(m: &HomesteadMeshes, o: Vec3, slabs: &[FloorSlab]) -> Vec<(f32, f32)> {
    let verts = all_vertices(m);
    slabs
        .iter()
        .filter(|s| s.top > 0.5)
        .filter(|s| {
            let (x0, z0, x1, z1) = s.rect();
            let y = o.y + s.underside();
            // A slab's underside is one quad over its footprint: its corners are its vertices.
            verts.iter().any(|(p, n)| {
                n.y < -0.9 && (p.y - y).abs() < 0.01 && p.x >= o.x + x0 - 0.01 && p.x <= o.x + x1 + 0.01 && p.z >= o.z + z0 - 0.01 && p.z <= o.z + z1 + 0.01
            })
        })
        .map(|s| (s.top, o.y + s.underside()))
        .collect()
}

/// The shipped ship has two levels: the deck every zone stands on, named the ground, and the
/// homesteads' upper floor 3 m over it, which reaches the Commons' 8 m roof, the highest top.
///
/// Seen red 2026-10-22 with the floors not merged (`dedup_by` left out): "the shipped ship has
/// 44 levels: [0.0, 0.0, 0.0, ..., 3.0, 3.0, 3.0]" (every body's storeys, one level each).
#[test]
fn the_shipped_ship_has_the_deck_and_the_upper_floor() {
    let levels = ship_levels(&shipped());
    let floors: Vec<f32> = levels.iter().map(|l| l.floor).collect();
    assert_eq!(levels.len(), 2, "the shipped ship has {} levels: {floors:?}", levels.len());
    assert_eq!(levels[0], ShipLevel { ordinal: 0, floor: 0.0, top: 3.0 });
    assert_eq!(levels[1], ShipLevel { ordinal: 1, floor: 3.0, top: 8.0 });
    assert_eq!((levels[0].name(), levels[0].short_name()), ("Storey 1 (ground)".to_string(), "1".to_string()));
    assert_eq!((levels[1].name(), levels[1].short_name()), ("Storey 2".to_string(), "2".to_string()));
    assert_eq!(level_at(&levels, 1.7), Some(0), "standing on the deck");
    assert_eq!(level_at(&levels, 3.0 + 1.7), Some(1), "standing upstairs");
    assert_eq!(level_at(&levels, 2.995), Some(1), "on the upper floor within its tolerance");
    assert_eq!(level_at(&levels, -4.0), Some(0), "under the lowest floor is the lowest level");
    assert_eq!(level_at(&[], 0.0), None);
}

/// A level under the deck datum is a basement: ordinal -1, "Basement 1", "B1"; the deck's own
/// level stays the ground (ordinal 0), whatever stands under it.
///
/// Seen red 2026-10-22 with the lowest level taken as the ground: "ordinals [0, 1, 2], want
/// [-1, 0, 1]".
#[test]
fn a_level_under_the_deck_is_a_basement() {
    let body = |h: f32| ron::from_str::<HomeStructure>(&format!("(width: 10.0, depth: 8.0, height: {h})")).expect("a body literal parses");
    let cellar = body(3.0);
    let mut house = body(6.0);
    house.floors = vec![FloorSlab {
        a: (0.0, 0.0),
        b: (house.width, house.depth),
        top: 3.0,
        thickness: 0.25,
        material: Default::default(),
        label: None,
    }];
    let bodies = [(Vec3::new(0.0, -3.0, 0.0), &cellar), (Vec3::ZERO, &house)];
    let levels = levels_of(&bodies);
    let ordinals: Vec<i32> = levels.iter().map(|l| l.ordinal).collect();
    assert_eq!(ordinals, vec![-1, 0, 1], "ordinals {ordinals:?}, want [-1, 0, 1]");
    assert_eq!((levels[0].name(), levels[0].short_name()), ("Basement 1".to_string(), "B1".to_string()));
    assert_eq!(levels[1].name(), "Storey 1 (ground)");
}

/// The ground level cuts each two-storey home (the player's own at p1 and every neighbour's) at
/// its storey's top, 3 m, and no one-storey zone: the Commons, First Street and the cabin block
/// stay whole. The top level cuts nothing (the whole ship is the view).
///
/// Seen red 2026-10-22 with `storey_cut` cutting every body at the storey the level is on, above
/// it or not: "the ground level cuts a one-storey body at (65.0, 20.0): The Commons is cut".
#[test]
fn the_ground_level_cuts_the_two_storey_homes_and_no_one_storey_zone() {
    let ship = shipped();
    let levels = ship_levels(&ship);
    let cuts = level_cuts(&ship, &levels[0]);
    for z in ship.zones.iter().filter(|z| z.body.storeys().len() == 1) {
        assert!(
            !cuts.iter().any(|c| c.min == (z.origin.0, z.origin.2)),
            "the ground level cuts a one-storey body at ({:.1}, {:.1}): {} is cut",
            z.origin.0,
            z.origin.2,
            z.label
        );
    }
    let home = &ship.zones[ship.home_zone_index()];
    assert!(cuts.iter().any(|c| c.min == (home.origin.0, home.origin.2) && (c.cut_y - 3.0).abs() < 1e-4), "the player's own home is cut at 3 m");
    for plot in ship.neighbour_plots().filter(|p| p.kind == "homestead") {
        assert!(cuts.iter().any(|c| c.min == (plot.origin.0, plot.origin.2) && (c.cut_y - 3.0).abs() < 1e-4), "neighbour {} is cut at 3 m", plot.id);
    }
    assert!(level_cuts(&ship, &levels[1]).is_empty(), "nothing is above the top level");
}

/// THE CUTAWAY DRAWS NO FLOOR OVER THE LEVEL SHOWN. Drawn at the ground level, no upper-floor
/// slab's underside is drawn anywhere in the player's own home or a neighbour's (their storeys
/// over the ground are cut away by `HomeStructure::cut_away`, the editor's rule); drawn whole,
/// they are (the control: the check can see them). The Commons keeps its walls to its 8 m roof.
///
/// Seen red 2026-10-22 twice: with `level_meshes` leaving the zones uncut, "at the ground level
/// the home at (0.0, 0.0) still draws 23 slab undersides"; and with the neighbours drawn whole
/// (`generate_meshes_at(None)`), "at the ground level neighbour p2 still draws 23 slab undersides".
#[test]
fn the_ground_level_draws_no_floor_over_it() {
    let ship = shipped();
    let levels = ship_levels(&ship);
    let t0 = std::time::Instant::now();
    let whole = ship.generate_meshes();
    let whole_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t1 = std::time::Instant::now();
    let ground = level_meshes(&ship, &levels[0]).expect("the ground level cuts something");
    let cut_ms = t1.elapsed().as_secs_f64() * 1000.0;
    println!("whole ship meshes {whole_ms:.1} ms, the ground level's {cut_ms:.1} ms");

    let home = &ship.zones[ship.home_zone_index()];
    let ho = home.origin_vec();
    assert!(!undersides_drawn(&whole, ho, &home.body.floors).is_empty(), "the whole ship draws the home's upper floor (the control)");
    let left = undersides_drawn(&ground, ho, &home.body.floors);
    assert!(left.is_empty(), "at the ground level the home at ({:.1}, {:.1}) still draws {} slab undersides", ho.x, ho.z, left.len());

    let view = crate::ship::neighbours::neighbour_view(&ship);
    let n = view.neighbours.iter().find(|n| n.plot.id == "p2").expect("p2 is a neighbour's");
    let no = Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2);
    assert!(!undersides_drawn(&whole, no, &n.design.body.floors).is_empty(), "the whole ship draws p2's upper floor (the control)");
    let left = undersides_drawn(&ground, no, &n.design.body.floors);
    assert!(left.is_empty(), "at the ground level neighbour p2 still draws {} slab undersides", left.len());

    let commons = ship.zones.iter().find(|z| z.id == "commons").expect("the Commons");
    let co = commons.origin_vec();
    let top = all_vertices(&ground)
        .iter()
        .filter(|(p, _)| p.x >= co.x - 0.5 && p.x <= co.x + commons.body.width + 0.5 && p.z >= co.z - 0.5 && p.z <= co.z + commons.body.depth + 0.5)
        .map(|(p, _)| p.y)
        .fold(f32::MIN, f32::max);
    assert!(top >= commons.body.height - 0.3, "the Commons' walls reach {top:.2} m at the ground level, not its {:.1} m roof", commons.body.height);
}

/// The Maps page's level view and the game's whole view of the neighbours are both kept: moving
/// between them re-bakes neither (ship/neighbours.rs keeps two).
///
/// Seen red 2026-10-22 with the cache keeping one view: "moving between the level and the whole
/// view made 4 views, want 2".
#[test]
fn the_level_view_and_the_whole_view_are_both_kept() {
    let ship = shipped();
    let before = crate::ship::neighbours::views_made();
    for _ in 0..2 {
        let _ = crate::ship::neighbours::neighbour_view_at(&ship, None);
        let _ = crate::ship::neighbours::neighbour_view_at(&ship, Some(0.0));
    }
    let made = crate::ship::neighbours::views_made() - before;
    assert!(made <= 2, "moving between the level and the whole view made {made} views, want 2");
}
