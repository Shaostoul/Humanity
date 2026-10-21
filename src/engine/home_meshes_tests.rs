//! Tests of home_meshes.rs's pure parts: what an OPEN plot lights (ship homes increment 5b,
//! docs/design/ship-homes-increment-5b-plan.md section 3.7). Each was seen to fail before it
//! passed; the failure is in its comment.

use super::*;
use crate::ship::ship_structure::{HomeDesign, OpenPlots};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The shipped ship with the home on `plot`.
fn ship(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).expect("the shipped ship assembles")
}

/// `plots` open, `lit` the one lit.
fn opened(mut s: ShipStructure, plots: &[&str], lit: Option<&str>) -> ShipStructure {
    s.open = OpenPlots { plots: plots.iter().map(|p| p.to_string()).collect(), lit: lit.map(str::to_string) };
    s
}

/// THE OPEN PLOT YOU STAND IN IS LIT, AND ON NOBODY'S METER (plan section 5, lane 1B; section 3.7's
/// table, "Light inside"). On a ship whose home is on p2, p1 open and lit: the room lights grow by
/// exactly the lights of p1's shipped design (every switched-on one, a strip as its segments), all
/// of them standing on p1, and the home's own lights are unchanged and first. The lighting meter
/// (`ShipStructure::lighting_watts`, what the house lighting consumer draws) reads the same: those
/// lights are no zone's. Open but not lit, lit but not open, and the home's own plot "lit" all
/// add nothing: a visited plot is lit only while its permit holder stands in it.
/// Seen red 2026-10-12 with the lights not added (`home_lights` dropping `open_plot_lights`):
/// "the lit open plot p1 adds 0 lights; its shipped design lights it with 49" failed.
#[test]
fn the_open_plot_you_stand_in_is_lit_and_on_nobodys_meter() {
    let watts_of = |id: &str| crate::renderer::light::light_type(id).map(|t| t.watts).unwrap_or(0.0);
    let closed = ship("p2");
    let before = home_lights(Some(&closed), Vec::new(), true);
    assert!(!before.is_empty(), "the home on p2 has lights of its own");

    let lit = opened(closed.clone(), &["p1"], Some("p1"));
    let after = home_lights(Some(&lit), Vec::new(), true);
    let design = HomeDesign::built_in_ref("homestead").expect("the homestead is built in");
    let p1 = lit.plots.iter().find(|p| p.id == "p1").expect("p1");
    let want = placed_room_lights(&design.body.lights, Vec3::new(p1.origin.0, p1.origin.1, p1.origin.2));
    let switched_on = design.body.lights.iter().filter(|l| l.on).count();
    assert!(switched_on > 10 && want.len() >= switched_on, "the shipped homestead's lights: {switched_on} on, {} drawn", want.len());
    let added = after.len() - before.len();
    assert_eq!(added, want.len(), "the lit open plot p1 adds {added} lights; its shipped design lights it with {}", want.len());
    // The home's own lights come first, unchanged; the added ones all stand on p1.
    for (a, b) in before.iter().zip(&after) {
        assert!((a.pos - b.pos).length() < 1e-5 && a.intensity == b.intensity, "the home's own light moved: {:?} vs {:?}", a.pos, b.pos);
    }
    let (lo, hi) = p1.aabb();
    for l in &after[before.len()..] {
        assert!(
            l.pos.x >= lo.x - 0.01 && l.pos.x <= hi.x + 0.01 && l.pos.z >= lo.z - 0.01 && l.pos.z <= hi.z + 0.01,
            "a light of the open plot stands off p1 ({lo:?} to {hi:?}): {:?}",
            l.pos
        );
    }
    // Nobody's meter: the zones' lights, which the meter reads, are the same.
    assert_eq!(lit.lighting_watts(watts_of), closed.lighting_watts(watts_of), "the open plot's lights are charged to a meter");

    // Only where you stand, only what is open, only a neighbour's plot.
    for (plots, lit_plot, why) in [
        (&["p1"][..], None, "p1 open, nothing lit"),
        (&["p3"][..], Some("p1"), "p1 lit but not open"),
        (&["p2"][..], Some("p2"), "the home's own plot"),
    ] {
        let s = opened(closed.clone(), plots, lit_plot);
        assert_eq!(home_lights(Some(&s), Vec::new(), true).len(), before.len(), "{why}: lights were added");
    }
    // The open plot's lights never decide the auto fill: with the auto fill in use (a ship whose
    // zones have no placed lights), it stays, and the plot's lights come after it.
    let mut dark = lit.clone();
    for z in dark.zones.iter_mut() {
        z.body.lights.clear();
    }
    let auto = auto_room_lights(Some(&dark), std::iter::once((Vec3::new(80.0, 2.0, 40.0), Vec3::new(10.0, 4.0, 10.0))));
    let filled = home_lights(Some(&dark), auto, true);
    assert_eq!(filled.len(), 1 + want.len(), "the auto fill went out when a neighbour's plot was lit");
}

/// A DOOR'S STOREY TOLERANCE IS THE STEP UP (ship homes increment 7): a body counts for a door when
/// its feet are within `door_anim::DOOR_STOREY_TOLERANCE_M` of the door's band, which says it is
/// footing's `STEP_UP_M`, the height a body steps up onto a raised floor. door_anim.rs compiles in
/// the relay, where footing.rs does not, so the number is written twice and held equal here.
/// Seen red 2026-10-20 with the tolerance at 0.5: "left: 0.5, right: 0.6".
#[test]
fn a_doors_storey_tolerance_is_the_step_up() {
    assert_eq!(crate::systems::door_anim::DOOR_STOREY_TOLERANCE_M, crate::engine::footing::STEP_UP_M);
}

/// PIPES RUN UNDER EACH STOREY'S OWN CEILING (ship homes increment 7, plan section 3: conduits per
/// storey). In a 6 m home with a 0.25 m floor slab at 3.0, the ground storey's pipes hang from the
/// slab's underside (2.75) and run 0.3 m under it, at 2.45; the upper storey's from the roof, at
/// 5.7; a run from a machine downstairs to one upstairs takes the upper storey's level (up through
/// the floor, along, down). A home with no slab keeps the one level it always had, 0.3 m under its
/// roof, and a body standing higher in the ship carries its levels with it.
///
/// Seen red 2026-10-20 with main's one level put back: "left: [(0.0, 6.0)], right: [(0.0, 2.75),
/// (3.0, 6.0)]".
#[test]
fn pipes_run_under_each_storeys_own_ceiling() {
    let mut body: crate::ship::home_structure::HomeStructure = ron::from_str("(width: 10.0, depth: 10.0, height: 6.0)").unwrap();
    let flat = service_levels(&body, Vec3::ZERO);
    assert_eq!(flat, vec![(0.0, 6.0)], "a body with no slab has one level, floor to roof");
    let (a, b) = (Vec3::new(1.0, 0.35, 1.0), Vec3::new(8.0, 0.35, 8.0));
    let (sy, ceiling) = run_level(&flat, a, b);
    assert!((sy - 5.7).abs() < 1e-5 && ceiling == 6.0, "one storey's pipes run 0.3 m under the roof: {sy}, {ceiling}");
    body.floors.push(ron::from_str("(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)").unwrap());
    let levels = service_levels(&body, Vec3::ZERO);
    assert_eq!(levels, vec![(0.0, 2.75), (3.0, 6.0)]);
    let (sy, ceiling) = run_level(&levels, a, b);
    assert!((sy - 2.45).abs() < 1e-5 && (ceiling - 2.75).abs() < 1e-6, "the ground storey's pipes run at {sy} under {ceiling}, want 2.45 under 2.75");
    let (sy, ceiling) = run_level(&levels, a, Vec3::new(8.0, 3.35, 8.0));
    assert!((sy - 5.7).abs() < 1e-5 && ceiling == 6.0, "a run to a machine upstairs runs at {sy} under {ceiling}, want 5.7 under 6");
    assert_eq!(service_levels(&body, Vec3::new(0.0, 10.0, 0.0))[1], (13.0, 16.0));
}

/// A MACHINE'S PIPES AND A ROOM'S LIGHT KEEP TO THEIR STOREY (plan section 3: the pipe anchor from
/// the machine's storey floor; auto room lights at the room's own ceiling). On the shipped ship
/// with the home given an upper floor (a slab at 3.0 over its whole box, the body 6 m tall), a
/// machine standing on that floor anchors its pipes on it, 3.0 over the zone's floor (the
/// placement's own floor is the zone's), one on the ground on the ground, and a point in no zone
/// keeps the placement's floor. A room under the slab is lit 0.1 m under the slab's underside, not
/// inside the slab at its top; a room upstairs 0.1 m under the roof.
///
/// Seen red 2026-10-20 with main's anchor and light put back: "a machine upstairs anchors its pipes at
/// 0, not on its floor at 3".
#[test]
fn a_machines_pipes_and_a_rooms_light_keep_to_their_storey() {
    let mut ship = ship("p1");
    let hz = ship.home_zone_index();
    let o = ship.zones[hz].origin_vec();
    let (w, d) = (ship.zones[hz].body.width, ship.zones[hz].body.depth);
    ship.zones[hz].body.height = 6.0;
    ship.zones[hz].body.floors.push(ron::from_str(&format!("(a: (0.0, 0.0), b: ({w}, {d}), top: 3.0)")).unwrap());
    let at = |y: f32| o + Vec3::new(w * 0.5, y, d * 0.5);
    let up = storey_floor_at(Some(&ship), at(3.0), o.y);
    assert!((up - (o.y + 3.0)).abs() < 1e-5, "a machine upstairs anchors its pipes at {up}, not on its floor at {}", o.y + 3.0);
    assert!((storey_floor_at(Some(&ship), at(0.0), o.y) - o.y).abs() < 1e-5, "a machine on the ground anchors on the ground");
    assert_eq!(storey_floor_at(Some(&ship), Vec3::new(-5000.0, 0.0, -5000.0), 7.0), 7.0, "a point in no zone keeps its own floor");
    let room = Vec3::new(10.0, 3.0, 10.0);
    let below = auto_room_lights(Some(&ship), std::iter::once((at(1.5), room)));
    assert!((below[0].pos.y - (o.y + 2.65)).abs() < 1e-4, "the room under the floor is lit at {}, want {} under the slab", below[0].pos.y, o.y + 2.65);
    let above = auto_room_lights(Some(&ship), std::iter::once((at(4.5), room)));
    assert!((above[0].pos.y - (o.y + 5.9)).abs() < 1e-4, "the room upstairs is lit at {}", above[0].pos.y);
}
