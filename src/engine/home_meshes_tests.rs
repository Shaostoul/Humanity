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
/// 5.7. A run between the storeys takes the LOWER one's level (the increment 7 review, 2026-10-21:
/// it took the upper one's, so it crossed the upper storey; `route_run` rises to the machine
/// upstairs under it). A home with no slab keeps the one level it always had, 0.3 m under its
/// roof, and a body standing higher in the ship carries its levels with it. Each end is the floor
/// it stands on.
///
/// Seen red 2026-10-20 with main's one level put back: "left: [(0.0, 6.0)], right: [(0.0, 2.75),
/// (3.0, 6.0)]". Its run between the storeys seen red first (2026-10-21) on the higher level, as
/// it was: "a run between the storeys runs at 5.7 under 6, want 2.45 under 2.75".
#[test]
fn pipes_run_under_each_storeys_own_ceiling() {
    let mut body: crate::ship::home_structure::HomeStructure = ron::from_str("(width: 10.0, depth: 10.0, height: 6.0)").unwrap();
    let flat = service_levels(&body, Vec3::ZERO);
    assert_eq!(flat, vec![(0.0, 6.0)], "a body with no slab has one level, floor to roof");
    let (sy, ceiling) = run_level(&flat, 0.0, 0.0);
    assert!((sy - 5.7).abs() < 1e-5 && ceiling == 6.0, "one storey's pipes run 0.3 m under the roof: {sy}, {ceiling}");
    body.floors.push(ron::from_str("(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)").unwrap());
    let levels = service_levels(&body, Vec3::ZERO);
    assert_eq!(levels, vec![(0.0, 2.75), (3.0, 6.0)]);
    let (sy, ceiling) = run_level(&levels, 0.0, 0.0);
    assert!((sy - 2.45).abs() < 1e-5 && (ceiling - 2.75).abs() < 1e-6, "the ground storey's pipes run at {sy} under {ceiling}, want 2.45 under 2.75");
    let (sy, ceiling) = run_level(&levels, 3.0, 3.0);
    assert!((sy - 5.7).abs() < 1e-5 && ceiling == 6.0, "the upper storey's pipes run at {sy} under {ceiling}, want 5.7 under 6");
    let (sy, ceiling) = run_level(&levels, 0.0, 3.0);
    assert!((sy - 2.45).abs() < 1e-5 && (ceiling - 2.75).abs() < 1e-6, "a run between the storeys runs at {sy} under {ceiling}, want 2.45 under 2.75");
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

/// The shipped homestead's elevator (elevator-1, at (31, 0, 26) of the home zone): its key in
/// `ElevatorCars`.
fn homestead_lift(ship: &ShipStructure) -> crate::engine::footing::LiftKey {
    let hz = ship.home_zone_index();
    let at = ship.zones[hz].origin_vec() + Vec3::new(31.0, 0.0, 26.0);
    crate::engine::footing::lifts(ship)
        .into_iter()
        .find(|l| l.key.0 == ship.zones[hz].id && (l.base - at).length() < 0.01)
        .expect("the homestead's elevator-1 at (31, 0, 26)")
        .key
}

/// THE CUT-AWAY SHOWN AND HIDDEN AGAIN AND AGAIN KEEPS THE SHELL'S SLOTS (ship homes increment 7
/// review, 2026-10-21). The renderer only appends meshes and materials, so the home's rebuild
/// must reuse its slots. On the shipped homestead, the ground storey's cut-away needs fewer floor
/// and wall meshes than the whole ship; six rebuilds alternating the two (each family's needs from
/// `shell_family_needs`, its slots from `assign_slots`, as `apply_homestead_meshes` takes them)
/// leave the renderer holding as many slots after the sixth as after the second, every slot it
/// made either in use or spare, and none in use twice.
///
/// Seen red first (2026-10-21) with the slots reused by index only, as they were: "after six
/// rebuilds the renderer holds 130 shell slots; after two it held 122 ([122, 122, 126, 126, 130,
/// 130] by rebuild)".
#[test]
fn the_cut_away_toggled_again_and_again_keeps_the_shells_slots() {
    let ship = ship("p1");
    let hz = ship.home_zone_index();
    let cut = crate::engine::editor::cut_away_ship(&ship, hz, 0).expect("the homestead's ground storey has the upper floor over it");
    let whole = shell_family_needs(&ship.generate_meshes());
    let cut_needs = shell_family_needs(&cut.generate_meshes());
    assert_ne!(whole, cut_needs, "the cut-away needs the same meshes as the whole ship, so this proves nothing");
    let mut made = 0usize;
    let mut spare: Vec<Slot> = Vec::new();
    let mut held: Vec<Vec<Slot>> = vec![Vec::new(); 8];
    let (mut made_by_round, mut problems) = (Vec::new(), Vec::new());
    for round in 0..6 {
        let needs = if round % 2 == 0 { whole } else { cut_needs };
        let (got, idle) = assign_slots(&mut spare, held.into_iter().zip(needs).collect(), || {
            made += 1;
            (made, made)
        });
        for (family, (slots, need)) in got.iter().zip(needs).enumerate() {
            if slots.len() != need {
                problems.push(format!("rebuild {round}: family {family} got {} slots for {need} meshes", slots.len()));
            }
        }
        let mut all: Vec<Slot> = got.iter().flatten().copied().chain(spare.iter().copied()).collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        if all.len() != n {
            problems.push(format!("rebuild {round}: a slot is held twice"));
        }
        if n != made {
            problems.push(format!("rebuild {round}: {made} slots made, {n} in use or spare"));
        }
        if !idle.iter().all(|s| spare.contains(s)) {
            problems.push(format!("rebuild {round}: a slot let go is not among the spares"));
        }
        held = got;
        made_by_round.push(made);
    }
    assert_eq!(
        made_by_round[5], made_by_round[1],
        "after six rebuilds the renderer holds {} shell slots; after two it held {} ({made_by_round:?} by rebuild)",
        made_by_round[5], made_by_round[1]
    );
    assert!(problems.is_empty(), "{problems:#?}");
}

/// AN ELEVATOR'S LANDING DOOR OPENS ONLY WITH THE CAR THERE (ship homes increment 7 review,
/// 2026-10-21; docs/design/homestead-upper-floor.md section 5). On the shipped homestead, the
/// upper lift lobby's slide door onto the shaft (U11, x 30.1 to 31.9 at z 26.9, on the floor at
/// 3.0) is elevator-1's landing door. With the car at the bottom, a body at the door does not open
/// it (it slid open onto the empty shaft, and the gate behind it stopped the body); with the car at
/// the top it opens as before. A door at no landing opens as it always has.
///
/// Seen red first (2026-10-21) with `door_target` not asking the landing, the door as it was:
/// "with the car at the bottom a body at U11 drives it to 1 (0 is shut): it opens onto the empty
/// shaft".
#[test]
fn an_elevators_landing_door_opens_only_with_the_car_there() {
    use crate::engine::footing::{lift_landing_open, ElevatorCars};
    let ship = ship("p1");
    let o = ship.zones[ship.home_zone_index()].origin_vec();
    let doors = crate::ship::door_panels::ship_panel_placements(&ship);
    let u11 = doors
        .iter()
        .find(|p| !p.is_window && (p.center - (o + Vec3::new(31.0, 3.0, 26.9))).length() < 0.2)
        .expect("U11, the lift lobby's door onto the shaft, its bottom middle at (31, 3, 26.9)");
    let key = homestead_lift(&ship);
    let mut cars = ElevatorCars::new();
    cars.insert(key.clone(), (0.0, 0.0, false));
    let at_door = 0.3; // a body 0.3 m from the door, well inside any door's opening distance
    let landing = lift_landing_open(&ship, &cars, u11.center);
    assert_eq!(landing, Some(false), "U11 is elevator-1's upper landing door, the car at the bottom");
    for manual in [false, true] {
        let t = door_target(u11, 0.0, false, manual, landing, at_door);
        assert_eq!(t, 0.0, "with the car at the bottom a body at U11 drives it to {t} (0 is shut): it opens onto the empty shaft");
    }
    cars.insert(key, (3.0, 3.0, false));
    let landing = lift_landing_open(&ship, &cars, u11.center);
    assert_eq!(landing, Some(true), "the car at the top");
    let t = door_target(u11, 0.0, false, true, landing, at_door);
    assert_eq!(t, 1.0, "with the car at the top U11 opens for the body at it ({t})");
    // A door at no landing, and every door's other rules, as before.
    let ordinary = doors
        .iter()
        .find(|p| !p.is_window && p.auto_open && crate::systems::door_anim::is_operable(&p.style) && lift_landing_open(&ship, &cars, p.center).is_none())
        .expect("an ordinary auto door");
    assert_eq!(door_target(ordinary, 0.0, false, false, None, at_door), 1.0, "an ordinary door opens for a body at it");
    assert_eq!(door_target(ordinary, 0.0, true, false, None, at_door), 0.0, "a locked door stays shut");
    assert_eq!(door_target(ordinary, 1.0, false, false, None, 50.0), 0.0, "an open door closes when nobody is near");
}

/// THE ELEVATOR CAR IS DRAWN TO ITS FOOTPRINT (ship homes increment 7 review, 2026-10-21). What a
/// rider stands on and what the landing gates close round is the elevator's whole footprint
/// (`footing::lifts`, `footing::elevator_gates`), so the car is drawn to it, less a 2 cm running
/// clearance a side (`CAR_CLEARANCE_M`), centred on the shaft, its top the car's floor. And the
/// editor's cut-away of the ground storey leaves out the homestead's car while it is at the top
/// (it stands on the floor above) and draws it at the bottom.
///
/// Seen red first (2026-10-21) with the car drawn at 0.85 of its footprint, as it was: "elevator
/// (0, 4): the car is drawn 1.53 x 1.53 on a 1.8 x 1.8 footprint; want 1.76 x 1.76".
#[test]
fn the_elevator_car_is_drawn_to_its_footprint() {
    use crate::engine::footing::ElevatorCars;
    let ship = ship("p1");
    let lifts = crate::engine::footing::lifts(&ship);
    let mut cars = ElevatorCars::new();
    let poses = elevator_car_poses(&ship, &cars, None);
    assert_eq!(poses.len(), lifts.len(), "one car per elevator");
    assert!(CAR_CLEARANCE_M > 0.0 && CAR_CLEARANCE_M <= 0.03, "a running clearance of {CAR_CLEARANCE_M} m");
    for (lift, (pos, _turn, scale)) in lifts.iter().zip(&poses) {
        let (w, _, d) = lift.ty.size;
        let (want_w, want_d) = (w - 2.0 * CAR_CLEARANCE_M, d - 2.0 * CAR_CLEARANCE_M);
        assert!(
            (scale.x - want_w).abs() < 1e-5 && (scale.z - want_d).abs() < 1e-5,
            "elevator {:?}: the car is drawn {:.2} x {:.2} on a {w} x {d} footprint; want {want_w:.2} x {want_d:.2}",
            lift.key, scale.x, scale.z
        );
        assert!((pos.y + scale.y - lift.base.y).abs() < 1e-5, "elevator {:?}: the car's top is not its floor", lift.key);
        assert!((pos.x - lift.base.x).abs() < 1e-5 && (pos.z - lift.base.z).abs() < 1e-5, "elevator {:?}: the car is off its shaft", lift.key);
    }
    let hz = ship.home_zone_index();
    let cut = crate::engine::editor::cut_away_of(&ship, hz, 0);
    assert!(cut.is_some(), "the homestead's ground storey has the upper floor over it");
    let key = homestead_lift(&ship);
    cars.insert(key.clone(), (3.0, 3.0, false));
    assert_eq!(elevator_car_poses(&ship, &cars, cut).len(), lifts.len() - 1, "the cut-away leaves out the car at the top");
    cars.insert(key, (0.0, 0.0, false));
    assert_eq!(elevator_car_poses(&ship, &cars, cut).len(), lifts.len(), "the cut-away draws the car at the bottom");
}

/// The shipped homestead's runs, as the home's rebuild routes them: the ship (home on p1), its
/// household's machine layout placed, and every run of it (`conduit_runs`).
fn shipped_runs() -> (ShipStructure, Vec<ConduitRun>) {
    let ship = ship("p1");
    let home = crate::machines::MachineHome::load(&data_dir().join("machines/home.ron")).expect("the household's machines load");
    let placements = home.placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()));
    let picks = port_picks(&home, &placements);
    let runs = conduit_runs(Some(&ship), &home, &placements, &picks);
    (ship, runs)
}

/// A RUN KEEPS TO THE STOREYS IT JOINS (ship homes increment 7 review, 2026-10-21). The upstairs
/// bedroom showed pipes along its walls' tops and hangers up to the glass roof (the verify-storeys
/// bedroom shot). 28 of the shipped homestead's 101 runs hung at 5.7 m, under the 6 m roof: the 14
/// from a machine downstairs to one upstairs (the study's server, router and uplink, the bathroom,
/// the washer, the toilet's waste) because a run took the higher end's level, so it rose at the
/// downstairs end and crossed the acre over the glass lids and through the bedroom; and 14 between
/// two machines downstairs (battery 6 and the cistern to the seven air handlers) because the
/// handlers hang 2.5 m up, so their port gizmos, 0.3 m over them, read as upstairs.
///
/// Every run now crosses under the ceiling of the lower storey it joins: on the shipped homestead
/// every level leg of a run is under that storey's ceiling, a run between two machines downstairs
/// never goes over the ground storey's ceiling at all, and no run passes through the upstairs
/// bedroom, where no machine of any run stands.
///
/// Seen red first (2026-10-21) with the routing as it was (the higher end's level, by the ends'
/// heights, and no risers): "64 problems in 101 runs", among them "power battery_8 -> server_1:
/// through the upstairs bedroom from Vec3(3.9, 5.7, 34.2) to Vec3(55.149998, 5.7, 34.2)" and
/// "power battery_7 -> exhaust_fan_1: two machines downstairs, the run goes up to 2.86".
#[test]
fn a_run_keeps_to_the_storeys_it_joins() {
    let (ship, runs) = shipped_runs();
    assert!(runs.len() > 90, "the shipped homestead has its runs ({})", runs.len());
    let site = run_site(Some(&ship));
    let hz = ship.home_zone_index();
    let levels = service_levels(&ship.zones[hz].body, ship.zones[hz].origin_vec());
    assert_eq!(levels.len(), 2, "the homestead has two storeys");
    let on = |y: f32| levels.iter().rposition(|(floor, _)| *floor <= y + 0.01).unwrap_or(0);
    // The upstairs bedroom's box, from the rooms the rebuild finds.
    let rooms = ship.generate_meshes().room_info;
    let bedroom = rooms
        .iter()
        .find(|r| r.id == "room-bedroom" && r.center.y > levels[1].0)
        .expect("the upstairs bedroom, room-bedroom");
    let (bmin, bmax) = (bedroom.center - bedroom.dimensions * 0.5 + Vec3::splat(0.01), bedroom.center + bedroom.dimensions * 0.5 - Vec3::splat(0.01));
    let in_bedroom = |p: Vec3| p.x > bmin.x && p.x < bmax.x && p.z > bmin.z && p.z < bmax.z && p.y > bmin.y;
    let mut problems = Vec::new();
    for run in &runs {
        let name = format!("{} {} -> {}", run.kind, run.from, run.to);
        let (route, _, _) = route_run(&site, run);
        let lower = on(run.foot_a).min(on(run.foot_b));
        let ceiling = levels[lower].1;
        let both_down = on(run.foot_a) == 0 && on(run.foot_b) == 0;
        let ends_in_bedroom = in_bedroom(run.a) || in_bedroom(run.b);
        for w in route.points.windows(2) {
            let (p, q) = (w[0], w[1]);
            if (p.y - q.y).abs() < 1e-4 && p.y > ceiling + 1e-3 {
                problems.push(format!("{name}: a level leg at {:.2} over its storey's ceiling at {ceiling:.2}", p.y));
            }
            if both_down && p.y.max(q.y) > levels[0].1 + 1e-3 {
                problems.push(format!("{name}: two machines downstairs, the run goes up to {:.2}", p.y.max(q.y)));
            }
            // The leg's box against the bedroom's.
            let (lo, hi) = (p.min(q), p.max(q));
            let through = lo.x < bmax.x && hi.x > bmin.x && lo.z < bmax.z && hi.z > bmin.z && hi.y > bmin.y && lo.y < bmax.y;
            if through && !ends_in_bedroom {
                problems.push(format!("{name}: through the upstairs bedroom from {p:?} to {q:?}"));
            }
        }
    }
    problems.dedup();
    assert!(problems.is_empty(), "{} problems in {} runs: {problems:#?}", problems.len(), runs.len());
}

/// THE CUT-AWAY LEAVES OUT THE RUNS OVER IT (ship homes increment 7 review, 2026-10-21): the
/// pipes are drawn from the whole ship like the machines, so with the ground storey's cut-away the
/// parts of the shipped homestead's runs over the cut, in the home's box (the risers up to the
/// bathroom and the study, the runs among the study's machines), are left out
/// (`parts_under_cut`), and what is left is everything under it: nothing under the cut is lost
/// but the centimetre the cut-away keeps clear under it (`STOREY_EPS`) on each riser.
///
/// Seen red first (2026-10-21) with `CutAway::hides` answering false, every run drawn whole (and
/// the routing as it was): "the cut-away draws 1119.93 m of runs over its cut (of 1119.9 m)".
#[test]
fn the_cut_away_leaves_out_the_runs_over_it() {
    let (ship, runs) = shipped_runs();
    let site = run_site(Some(&ship));
    let hz = ship.home_zone_index();
    let cut = crate::engine::editor::cut_away_of(&ship, hz, 0).expect("the ground storey has the upper floor over it");
    let (o, body) = (ship.zones[hz].origin_vec(), &ship.zones[hz].body);
    // Over the cut in the home's box, and the edge round it the cut takes with it (a wall unit's
    // port gizmos stand past the shell: the study's uplink and router risers are at x 55.05), by
    // the numbers, not by `hides`.
    let m = crate::engine::editor::CUT_EDGE_M;
    let over = |p: Vec3| {
        p.y > cut.cut_y + 1e-3 && p.x >= o.x - m && p.x <= o.x + body.width + m && p.z >= o.z - m && p.z <= o.z + body.depth + m
    };
    // A path's length over and under the cut, a centimetre at a time.
    let measure = |path: &[Vec3]| -> (f32, f32) {
        let (mut up, mut down) = (0.0, 0.0);
        for w in path.windows(2) {
            let len = w[0].distance(w[1]);
            let n = (len / 0.01).ceil().max(1.0) as usize;
            for k in 0..n {
                let p = w[0].lerp(w[1], (k as f32 + 0.5) / n as f32);
                if over(p) {
                    up += len / n as f32;
                } else {
                    down += len / n as f32;
                }
            }
        }
        (up, down)
    };
    let (mut whole_over, mut drawn_over, mut whole_under, mut drawn_under, mut risers) = (0.0, 0.0, 0.0, 0.0, 0);
    for run in &runs {
        let (route, _, _) = route_run(&site, run);
        let parts = parts_under_cut(&route.points, Some(cut));
        let (up, down) = measure(&route.points);
        whole_over += up;
        whole_under += down;
        for part in &parts {
            let (up, down) = measure(part);
            drawn_over += up;
            drawn_under += down;
        }
        risers += route.points.windows(2).filter(|w| (w[0].y - cut.cut_y) * (w[1].y - cut.cut_y) < 0.0).count();
        assert_eq!(parts_under_cut(&route.points, None), vec![route.points.clone()], "with no cut a run is drawn whole");
    }
    assert!(whole_over > 10.0, "the runs have {whole_over:.1} m over the cut, too little to prove anything");
    assert!(drawn_over < 0.01, "the cut-away draws {drawn_over:.2} m of runs over its cut (of {whole_over:.1} m)");
    // Per riser: the centimetre kept clear, and a centimetre's sample either side of the cut.
    let slack = risers as f32 * 0.012;
    let kept_clear = risers as f32 * crate::ship::home_structure::STOREY_EPS + slack;
    assert!(
        drawn_under <= whole_under + slack + 0.01 && drawn_under >= whole_under - kept_clear,
        "the cut-away draws {drawn_under:.2} m of runs under its cut, of {whole_under:.2} m ({risers} risers through it)"
    );
}
