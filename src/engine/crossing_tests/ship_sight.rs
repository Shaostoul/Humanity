//! From ship/sight.rs's tests: every shipped machine is E's from its own floor, the floor read by the
//! engine's footing (`footing::Footing::floor_under`), not by the storey rule under test.

use crate::ship::sight::tests::aboard;
use crate::ship::sight::*;
use glam::Vec3;
use crate::ship::sight::tests::e_from;

/// The REAL floor under a fixture whose foot is at `foot` over (x, z) (ship frame): the highest
/// floor at or under its foot there that a body stands on (`Footing::floor_under`, measured so
/// it reaches no higher than the foot), so a unit hung under a slab is over the room's floor,
/// not the slab's top. Where the footing finds nothing (a doorway's cells belong to no room, and
/// nothing else stands there), the home's own floor, `ground`.
#[cfg(feature = "native")]
fn floor_under_fixture(footing: &crate::engine::footing::Footing, x: f32, foot: f32, z: f32, ground: f32) -> f32 {
    let step = crate::engine::footing::STEP_UP_M;
    footing.floor_under(Vec3::new(x, foot + 1.7, z), foot - step + 0.01).unwrap_or(ground)
}

/// EVERY SHIPPED MACHINE IS E'S FROM ITS OWN FLOOR, AND SEEN FROM IT. For each machine of
/// data/machines/home.ron, standing on the real floor under it (`Footing::floor_under`, the
/// highest a body stands on at or under its foot: the seams review of 2026-10-21, finding 4,
/// found this test reading "its own floor" off the storey rule under test, which put the eye
/// 4.7 m up in the greenhouse's air for a unit hung 3 m or more up there), half a metre from
/// it towards the middle of the home and looking at it, with nothing else about: E reaches it
/// (`storey_floor` puts both on one storey), and no solid slab stands between that eye and its
/// label (a label poking up into or through the floor over its own room would hide it from
/// that room). A unit hung higher than its storey, or a label that pierces the slab over it,
/// fails here and not on the operator's walk-through.
///
/// Seen red first (2026-10-21) twice: in an earlier form, which read the storey by a fixed
/// band over the feet, with that band at the relay's 2.5 m, "air_handler_g1 stands 2.70 m
/// over its floor at 0.0, out of the walk-up's storey band"; and in this form, with the
/// home's storeys ignored (the relay's band alone), "from its own floor at 0.0, E misses
/// air_handler_g1 (its foot 2.70 m over it)".
#[cfg(feature = "native")]
#[test]
fn every_shipped_machine_is_es_from_its_own_floor_and_seen_from_it() {
    let w = aboard();
    assert!(w.storeys.len() >= 2, "the homestead has an upper floor");
    let feet = crate::engine::footing::test_aboard::Aboard::of(w.ship.clone());
    let footing = feet.footing();
    let home = &w.ship.zones[w.ship.home_zone_index()].body;
    let middle = w.at(home.width * 0.5, 0.0, home.depth * 0.5);
    for (id, _room, label, foot) in &w.machines {
        let local = foot - w.origin.y;
        let floor = floor_under_fixture(&footing, label.x, *foot, label.z, w.origin.y) - w.origin.y;
        let toward = (Vec3::new(middle.x - label.x, 0.0, middle.z - label.z)).normalize_or_zero() * 0.5;
        let eye = Vec3::new(label.x + toward.x, w.origin.y + floor + 1.7, label.z + toward.z);
        let got = e_from(eye, &[(*label, *foot)], |p: Vec3, foot: Foot| storey_floor(&w.ship, p, foot));
        assert_eq!(got, Some(0), "from its own floor at {floor:.1}, E misses {id} (its foot {:.2} m over it)", local - floor);
        let slab = w.blockers.slabs.iter().any(|s| s.blocks(eye, *label));
        assert!(!slab, "{id}'s label at {:.2} m is behind a solid slab from its own floor at {floor:.1}", label.y - w.origin.y);
    }
}

/// A UNIT HUNG HIGH IN A ROOM OPEN TO THE ROOF IS ON THE FLOOR UNDER IT (the seams review of
/// 2026-10-21, finding 4). The greenhouse (x 10 to 55, z 51 to 73) is open to the roof, 6 m, with
/// no slab of the upper floor over it. A unit hung with its foot 4.0 m up at (30, 60), its label
/// 1.0 m over that, belongs to the greenhouse floor: from that floor (`Footing::floor_under`),
/// half a metre from it and looking up at it, E reaches it. A unit standing on the upper floor's
/// slab is still on the upper floor: from the dining room under the bedroom, E does not reach
/// one standing in the bedroom. A body's feet are on the storey of their height wherever they
/// are (`Foot::Body`): a rider on the elevator's car at the top of its shaft is upstairs.
///
/// Seen red first (2026-10-21) with `storey_floor` taking the storey from the slab tops alone,
/// wherever the point is in plan: "from the greenhouse floor at 0.0, E misses a unit hung 4.0 m
/// up (left: None, right: Some(0))".
#[cfg(feature = "native")]
#[test]
fn a_unit_hung_high_in_a_room_open_to_the_roof_is_on_the_floor_under_it() {
    let w = aboard();
    let feet = crate::engine::footing::test_aboard::Aboard::of(w.ship.clone());
    let footing = feet.footing();
    let foot = w.origin.y + 4.0;
    let label = w.at(30.0, 5.0, 60.0);
    let floor = floor_under_fixture(&footing, label.x, foot, label.z, w.origin.y);
    assert!((floor - w.origin.y).abs() < 1e-4, "the greenhouse floor under the unit is at {floor}");
    let eye = Vec3::new(label.x, floor + 1.7, label.z + 0.5);
    let got = e_from(eye, &[(label, foot)], |p: Vec3, foot: Foot| storey_floor(&w.ship, p, foot));
    assert_eq!(got, Some(0), "from the greenhouse floor at {:.1}, E misses a unit hung 4.0 m up", floor - w.origin.y);
    // A unit on the bedroom's floor (the slab at 3.0), from the dining room under it.
    let upstairs = (w.at(38.0, 3.9, 34.0), w.origin.y + 3.0);
    assert_eq!(e_from(w.at(38.0, 1.7, 34.6), &[upstairs], |p: Vec3, foot: Foot| storey_floor(&w.ship, p, foot)), None, "from the dining room, E reaches a unit in the bedroom over it");
    // A body's feet are held up by what they stand on: on the elevator's car at the top of its
    // shaft (31, 26), where no slab is, the rider is upstairs; a fixture there is not.
    let on_car = w.at(31.0, 3.0, 26.0);
    assert_eq!(storey_floor(&w.ship, on_car, Foot::Body), Some(w.origin.y + 3.0), "a rider on the car at the top of its shaft");
    assert_eq!(storey_floor(&w.ship, on_car, Foot::Fixture), Some(w.origin.y), "a fixture hung 3 m up in the shaft");
}
