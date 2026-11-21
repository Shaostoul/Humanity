//! Respawners as places, on the SHIPPED data (docs/design/game-setting-and-intro.md rung 2).
//!
//! RED EVIDENCE (2026-11-21): every test here that finds a respawner was run first with
//! `respawners::PROVIDES` set to "respawn_x", which no machine type provides (what the data was
//! before the pods: nothing a respawner), and each failed: `the_homestead_wakes_you_upstairs...`
//! "the Homestead (home.ron) has its own pod: []", `a_cabin_holder_wakes_at_the_block_pod...`
//! "a cabin holder's home place is the block's pod", `a_guest_has_only_general_services`
//! "left: [] right: [\"general-services-1\"]", `the_choice_follows_the_ask...` (no places at all),
//! `the_relay_knows_where_general_services_is` "the relay's ship has General Services' pod".
//! `you_wake_just_outside_the_pods_door` was run with `DOOR_CLEARANCE_M` at -0.45 (the eye at the
//! pod's middle, inside it) and failed with "Vec3(10.0, 3.0, 20.0)".

use crate::machines::MachineHome;
use crate::respawners::*;
use crate::ship_structure::{ShipPlots, ShipStructure};
use std::path::{Path, PathBuf};

fn data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn machines(file: &str) -> MachineHome {
    MachineHome::load(&data().join("machines").join(file)).unwrap_or_else(|| panic!("{file} loads"))
}

fn ship_on(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data(), Some(plot)).expect("the shipped ship assembles")
}

/// The box of a zone's labelled volume (or the zone itself), ship metres, (min, max) on x and z.
fn area(ship: &ShipStructure, zone: &str, volume: Option<&str>) -> ((f32, f32), (f32, f32)) {
    let z = ship.zones.iter().find(|z| z.id == zone).expect("the zone");
    let (ox, _, oz) = z.origin;
    match volume {
        None => ((ox, ox + z.body.width), (oz, oz + z.body.depth)),
        Some(v) => {
            let v = z.body.zones.iter().find(|x| x.id == v).expect("the volume");
            ((ox + v.origin.0, ox + v.origin.0 + v.size.0), (oz + v.origin.2, oz + v.origin.2 + v.size.2))
        }
    }
}

fn inside(p: glam::Vec3, a: ((f32, f32), (f32, f32))) -> bool {
    p.x > a.0 .0 && p.x < a.0 .1 && p.z > a.1 .0 && p.z < a.1 .1
}

/// Both Homestead households have their own pod in the main bedroom, UPSTAIRS: you wake on the
/// upper floor (3.0 m), inside the bedroom, and the ship's public pod is in General Services'
/// hall on the ground floor, beside the Medical Bay.
#[test]
fn the_homestead_wakes_you_upstairs_and_general_services_is_public() {
    let ship = ship_on("p1");
    for file in ["home.ron", "home_solo.ron"] {
        let all = respawners_in(&ship, &machines(file));
        let own: Vec<&Respawner> = all.iter().filter(|r| r.access == Access::Own).collect();
        assert_eq!(own.len(), 1, "the Homestead ({file}) has its own pod: {all:?}");
        let pod = own[0];
        assert_eq!(pod.zone, "home");
        assert!((pod.floor.y - 3.0).abs() < 1e-4, "{file}: you wake on the upper floor, not at {}", pod.floor.y);
        let bedroom = area(&ship, "home", Some("room-bedroom"));
        assert!(inside(pod.floor, bedroom), "{file}: you wake inside the bedroom, at {:?}", pod.floor);
    }
    // home.ron merges the ship's rows, so General Services' pod is among them, public.
    let all = respawners_in(&ship, &machines("home.ron"));
    let gs: Vec<&Respawner> = all.iter().filter(|r| r.zone == "general-services-1").collect();
    assert_eq!(gs.len(), 1, "one pod in General Services: {all:?}");
    assert_eq!(gs[0].access, Access::Public);
    assert_eq!(gs[0].zone_label, "General Services");
    assert!(gs[0].floor.y.abs() < 1e-4, "on the ground floor");
    let ((x0, x1), (z0, z1)) = area(&ship, "general-services-1", None);
    // The hall is the zone's floor north of its barracks wall (local z 34).
    assert!(inside(gs[0].floor, ((x0, x1), (z0 + 34.0, z1))), "in the hall, at {:?}", gs[0].floor);
}

/// A cabin has no room for a pod, so a cabin holder's "home" place is Cabin Block A's shared pod,
/// by the washroom: the residence zone their door opens on. A homestead holder may not wake there.
#[test]
fn a_cabin_holder_wakes_at_the_block_pod_and_nobody_else_does() {
    let ship = ship_on("c1");
    let door = ship.home_plot().expect("the cabin stands on c1").door.zone.clone();
    assert_eq!(door, "cabins-1", "the cabin's door opens on the block's hall");
    let all = respawners_in(&ship, &machines("cabin.ron"));
    assert!(all.iter().all(|r| r.access != Access::Own), "the cabin itself has no pod");
    let places = wake_places(&all, true, Some(&door));
    let home = places.iter().find(|w| w.is_home()).expect("a cabin holder's home place is the block's pod");
    assert_eq!(home.respawner.zone, "cabins-1");
    assert_eq!(home.respawner.access, Access::Residents);
    assert!(inside(home.respawner.floor, area(&ship, "cabins-1", None)), "in the hall: {:?}", home.respawner.floor);
    assert!(places.iter().any(|w| w.place == "general-services-1"), "General Services is always there too");
    // Someone whose door opens elsewhere (a homestead on First Street) is not a resident.
    let not_theirs = wake_places(&all, true, Some("street-1"));
    assert!(not_theirs.iter().all(|w| w.respawner.zone != "cabins-1"), "{not_theirs:?}");
}

/// A guest (no home aboard) may wake only at the public pods: General Services.
#[test]
fn a_guest_has_only_general_services() {
    let ship = ship_on("p1");
    let all = respawners_in(&ship, &machines("home.ron"));
    let places: Vec<String> = wake_places(&all, false, None).into_iter().map(|w| w.place).collect();
    assert_eq!(places, vec!["general-services-1".to_string()]);
}

/// Where a player wakes: what they asked for when allowed, else their usual place, else home,
/// else the first public one; and the death screen's list has the usual place first.
#[test]
fn the_choice_follows_the_ask_then_the_usual_place_then_home() {
    let ship = ship_on("p1");
    let all = respawners_in(&ship, &machines("home.ron"));
    let places = wake_places(&all, true, Some("street-1"));
    let ids = |v: &[WakePlace]| v.iter().map(|w| w.place.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&places), vec!["home", "general-services-1"]);
    assert_eq!(choose(&places, None, None).unwrap().place, "home");
    assert_eq!(choose(&places, Some("general-services-1"), None).unwrap().place, "general-services-1");
    assert_eq!(choose(&places, None, Some("general-services-1")).unwrap().place, "general-services-1");
    // A place they may not wake (another block's pod) or that does not exist falls back.
    assert_eq!(choose(&places, Some("cabins-1"), Some("nowhere")).unwrap().place, "home");
    assert_eq!(ids(&usual_first(&places, Some("general-services-1"))), vec!["general-services-1", "home"]);
    assert_eq!(ids(&usual_first(&places, None)), vec!["home", "general-services-1"]);
    let guest = wake_places(&all, false, None);
    assert_eq!(choose(&guest, Some("home"), Some("home")).unwrap().place, "general-services-1", "a guest has no home pod");
    assert!(choose(&[], None, None).is_none());
}

/// The relay reads the same public pods from the ship's own rows, so a waking guest, or a
/// player who chose General Services, stands where the game stands them.
#[test]
fn the_relay_knows_where_general_services_is() {
    let plots = ShipPlots::load(&data()).expect("the relay's ship loads");
    let eye = plots.wake_eye("general-services-1", None).expect("the relay's ship has General Services' pod");
    assert_eq!(plots.public_wake_eye(), Some(eye), "where a guest wakes");
    let ship = ship_on("p1");
    let game = wake_places(&respawners_in(&ship, &machines("home.ron")), false, None);
    assert_eq!(game[0].respawner.eye(), eye, "the game and the relay agree to the millimetre");
    assert!((eye.y - crate::ship_structure::SPAWN_EYE_HEIGHT_M).abs() < 1e-4, "at eye height over the ground floor");
    // Cabin Block A's pod is its cabins' only: c1's door opens on its hall, p1's on the Commons.
    let block = plots.wake_eye("cabins-1", Some("c1")).expect("a cabin holder may wake at the block's pod");
    let cabin_ship = ship_on("c1");
    let door = cabin_ship.home_plot().unwrap().door.zone.clone();
    let game = wake_places(&respawners_in(&cabin_ship, &machines("cabin.ron")), true, Some(&door));
    assert_eq!(game[0].respawner.eye(), block, "the game and the relay agree on the block's pod");
    assert_eq!(plots.wake_eye("cabins-1", Some("p1")), None, "a homestead holder may not");
    assert_eq!(plots.wake_eye("cabins-1", None), None, "nor may a guest");
    assert_eq!(plots.wake_eye("nowhere", None), None);
}

/// The door is the pod's +Z face turned by its yaw, as the renderer turns it.
#[test]
fn you_wake_just_outside_the_pods_door() {
    let p = glam::Vec3::new(10.0, 3.0, 20.0);
    let f = wake_floor(p, 0.9, 0.0);
    assert!((f - glam::Vec3::new(10.0, 3.0, 20.95)).length() < 1e-4, "{f:?}");
    let f = wake_floor(p, 0.9, 90.0);
    assert!((f - glam::Vec3::new(10.95, 3.0, 20.0)).length() < 1e-4, "{f:?}");
    let f = wake_floor(p, 0.9, 270.0);
    assert!((f - glam::Vec3::new(9.05, 3.0, 20.0)).length() < 1e-4, "{f:?}");
}
