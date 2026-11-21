//! Waking at a respawner, on the shipped ship, measured against what the walk stands on
//! (docs/design/game-setting-and-intro.md rung 2's gate: "a respawn lands on the chosen pod's
//! floor on every storey; a guest wakes in general services; a bunk is a bed").
//!
//! RED EVIDENCE (2026-11-21, each with a sabotage put back after): `a_respawn_lands_on_the_chosen_
//! pods_floor_on_every_storey` with `Respawner::eye` put 3 m lower (an eye on the storey under the
//! pod): "home.ron, home: you wake on the pod's floor: floor 3, the walk stands at Some(0.0)".
//! (Put 3 m HIGHER it passed: the footing finds the floor under the feet, and the 6 m roof is no
//! floor, so the storey under is the one the test can catch, and the explicit floor-3 check
//! catches a pod placed on the wrong storey.) `you_wake_outside_the_pod_not_inside_it` with
//! `DOOR_CLEARANCE_M` at -0.45: "respawn_pod_1: the eye is 0.000 m from the pod's middle, inside
//! it". `a_bunk_is_a_bed` with the bunk's `provides` at None: "General Services' bunks sleep you:
//! []". `a_guest_wakes_in_general_services` was NOT seen red: with `places_on`'s has_home taken
//! out it still passed, because `respawners_in` leaves out a put-away home's pod twice over (its
//! zone is gone, and the away check); the relay's half of the rule was seen red (game_state.rs).

use super::*;
use crate::engine::footing::test_aboard::Aboard;
use humanity_ship::machines::MachineHome;
use std::path::{Path, PathBuf};

fn data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn machines(file: &str) -> MachineHome {
    humanity_systems::grow_machines::load_home(&data().join("machines").join(file)).unwrap_or_else(|| panic!("{file} loads"))
}

fn ship_on(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data(), Some(plot)).expect("the shipped ship assembles")
}

/// The shortest distance across the floor from `p` to a wall segment that stops a body standing
/// with its eye at `eye_y`.
fn nearest_wall(walls: &[humanity_ship::wall_collision::WallSegment], p: Vec3, eye_y: f32) -> f32 {
    walls
        .iter()
        .filter(|w| w.stops_body(eye_y))
        .map(|w| {
            let (a, b) = (glam::Vec2::new(w.a.0, w.a.1), glam::Vec2::new(w.b.0, w.b.1));
            let q = glam::Vec2::new(p.x, p.z);
            let ab = b - a;
            let s = if ab.length_squared() > 0.0 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            (q - (a + ab * s)).length() - w.half_thickness
        })
        .fold(f32::INFINITY, f32::min)
}

/// Every place a player may wake, on both storeys the shipped ship has: the Homestead's pod in the
/// main bedroom upstairs (both households), Cabin Block A's pod in its hall, and General Services'
/// pod, each on the ground. Waking there, the walk's own footing (what Respawn's `reseat_floor`
/// reads) stands the body on the pod's floor, not the storey under it or the roof over it, clear of
/// every wall and outside the pod.
#[test]
fn a_respawn_lands_on_the_chosen_pods_floor_on_every_storey() {
    let mut storeys = std::collections::BTreeSet::new();
    for (plot, file) in [("p1", "home.ron"), ("p1", "home_solo.ron"), ("c1", "cabin.ron")] {
        let ship = ship_on(plot);
        let all = respawners::respawners_in(&ship, &machines(file));
        let ship_rows = respawners::respawners_in(&ship, &MachineHome::load_file(&data().join("machines/ship.ron")).unwrap());
        let mut found = all.clone();
        for r in ship_rows {
            if !found.iter().any(|a| a.machine_id == r.machine_id) {
                found.push(r);
            }
        }
        let places = places_on(&ship, &found);
        assert!(places.iter().any(|w| w.is_home()), "{file}: a home place");
        assert!(places.iter().any(|w| w.place == "general-services-1"), "{file}: General Services");
        let aboard = Aboard::of(ship.clone());
        let footing = aboard.footing();
        for w in &places {
            let eye = w.respawner.eye();
            let floor = footing.reseat(eye);
            assert_eq!(
                floor.map(|f| (f * 1000.0).round() / 1000.0),
                Some((w.respawner.floor.y * 1000.0).round() / 1000.0),
                "{file}, {}: you wake on the pod's floor: floor {}, the walk stands at {floor:?}",
                w.place,
                w.respawner.floor.y
            );
            let clear = nearest_wall(&aboard.walls, eye, eye.y);
            assert!(clear > 0.3, "{file}, {}: the eye at {eye:?} is {clear:.3} m from a wall", w.place);
            storeys.insert((w.respawner.floor.y * 10.0).round() as i32);
        }
        // The home pod upstairs: a person waking there stands in the bedroom on the upper floor.
        if file != "cabin.ron" {
            let home = places.iter().find(|w| w.is_home()).unwrap();
            assert!((home.respawner.floor.y - 3.0).abs() < 1e-3, "{file}, home: you wake inside the bedroom upstairs, on its floor: floor {}", home.respawner.floor.y);
        }
    }
    assert_eq!(storeys.into_iter().collect::<Vec<_>>(), vec![0, 30], "both storeys are woken on");
}

/// The wake spot is outside the pod's box (a step out of its door), never inside it.
#[test]
fn you_wake_outside_the_pod_not_inside_it() {
    let ship = ship_on("p1");
    let home = machines("home.ron");
    let placed = home.placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()));
    for r in respawners::respawners_in(&ship, &home) {
        let p = placed.iter().find(|p| p.id == r.machine_id).unwrap();
        let mid = Vec3::new(p.pos.0, r.floor.y, p.pos.2);
        let off = (r.floor - mid).length();
        assert!(off > p.size.0.max(p.size.2) * 0.5, "{}: the eye is {off:.3} m from the pod's middle, inside it", r.machine_id);
    }
}

/// A guest (the shared world gave them no plot, so their home is put away) wakes in General
/// Services: the only place they may wake, and where the respawn stands them.
#[test]
fn a_guest_wakes_in_general_services() {
    let away = ship_on("p1").put_home_away().expect("the home is put away");
    let all = respawners::respawners_in(&away, &machines("home.ron"));
    let places = places_on(&away, &all);
    let ids: Vec<&str> = places.iter().map(|w| w.place.as_str()).collect();
    assert_eq!(ids, vec!["general-services-1"], "a guest's places");
    let w = respawners::choose(&places, Some("home"), Some("home")).expect("somewhere to wake");
    let gs = away.zones.iter().find(|z| z.id == "general-services-1").unwrap();
    let (x, z) = (w.respawner.floor.x - gs.origin.0, w.respawner.floor.z - gs.origin.2);
    assert!(x > 0.0 && x < gs.body.width && z > 0.0 && z < gs.body.depth, "inside General Services: {:?}", w.respawner.floor);
    // The relay is told the place by its zone, and stands the guest there itself.
    assert_eq!(join_field(Some(&away), w), Some(serde_json::json!({ "place": "general-services-1" })));
}

/// The home's pod goes to the relay in plot-local metres, its floor included, so a pod upstairs
/// is woken at upstairs on whichever plot the relay holds.
#[test]
fn the_home_pod_goes_to_the_relay_plot_local_with_its_floor() {
    let ship = ship_on("p2");
    let places = places_on(&ship, &respawners::respawners_in(&ship, &machines("home.ron")));
    let home = places.iter().find(|w| w.is_home()).unwrap();
    let field = join_field(Some(&ship), home).expect("a field");
    let at = field["at"].as_array().expect("at");
    let plot = ship.home_plot().unwrap();
    let local = |i: usize, o: f32| (at[i].as_f64().unwrap() as f32, home.respawner.floor[i] - o);
    for (got, want) in [local(0, plot.origin.0), local(1, plot.origin.1), local(2, plot.origin.2)] {
        assert!((got - want).abs() < 1e-3, "{field}");
    }
    assert!((at[1].as_f64().unwrap() - 3.0).abs() < 1e-3, "upstairs: {field}");
    assert_eq!(field["place"], "home");
}

/// General Services' bunks are beds: E at one asks for the night, as at the bedroom's bed.
#[test]
fn a_bunk_is_a_bed() {
    let rows = MachineHome::load_file(&data().join("machines/ship.ron")).expect("ship.ron");
    let sleepers: Vec<String> = rows
        .all_instances()
        .into_iter()
        .filter(|i| i.zone == "general-services-1")
        .filter(|i| rows.catalog.get(&i.machine).and_then(crate::engine::built_uses::machine_use) == Some(humanity_systems::construction::uses::StructureUse::Sleep))
        .map(|i| i.id)
        .collect();
    assert!(sleepers.len() >= 8, "General Services' bunks sleep you: {sleepers:?}");
    let bunk = &rows.catalog["bunk"];
    let label = crate::gui::MachineLabel {
        pos: Vec3::ZERO,
        foot_y: 0.0,
        name: "Bunk".into(),
        stats: Vec::new(),
        room: "gs-barracks".into(),
        machine_id: "bunk_gs_1".into(),
        on_use: crate::engine::built_uses::machine_use(bunk),
    };
    let mut data = DataStore::new();
    data.insert(humanity_systems::sleep::REQUEST_SLOT, Mutex::new(None::<String>));
    assert!(crate::engine::built_uses::use_machine_label(&data, &label), "E at a bunk is used, not opened");
    let asked = data.get::<Mutex<Option<String>>>(humanity_systems::sleep::REQUEST_SLOT).unwrap().lock().unwrap().clone();
    assert_eq!(asked.as_deref(), Some("Bunk"), "the night is asked for in the Bunk");
}

/// The usual place goes into a save as the game holds it, and a save written before any was
/// applied keeps the file's.
#[test]
fn the_usual_place_is_kept_in_the_save() {
    let mut data = DataStore::new();
    assert_eq!(usual_for_save(&data, || Some("general-services-1".into())), Some("general-services-1".into()), "the file's, before any");
    put_usual(&mut data, None);
    assert_eq!(usual_for_save(&data, || Some("general-services-1".into())), None, "home, as the game holds it");
    set_usual(&data, Some("general-services-1".into()));
    assert_eq!(usual(&data), Some(Some("general-services-1".into())));
}
