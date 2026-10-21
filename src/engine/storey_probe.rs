//! What the dev IPC reports of the walk between storeys (ship homes increment 7, 2026-10-21).
//! Permanent dev tooling.
//!
//! WHY IT EXISTS. Increment 7 gave the homestead an upper floor (floor slabs, walls and doors per
//! storey, the head clamp, the elevator's stops, the ladder hatch's gate) with unit tests over a
//! small synthetic two-storey home, and nothing that looked at the running game: no probe said
//! which floor the body stood on, which room the HUD named, where the ceiling was or where the
//! elevator's car was, so a homestead whose slabs never reached the walk (a stair that ends in
//! air, a HUD that names the room downstairs) would have left every test green. This is what
//! scripts/verify-storeys.js reads (judged by scripts/lib/storeys-judge.js):
//!   - every recorder frame carries `storey` ([`frame_json`]): the floor the body stands on, the
//!     slab underside its head stays under, whether it is on a ladder, at one, or riding a car,
//!     the room the HUD's room card names (`gui::pages::hud::room_at`, the HUD's own pick: its id,
//!     the name shown and its floor), whether the HUD is drawn, every elevator car's height, and
//!     the landings handed to the body so far with the last one's speed (so a rig sees each
//!     landing on the frame it happened, not only the last of a leg);
//!   - the recorder's done JSON carries `storeys` ([`done_json`]): every elevator aboard, its id,
//!     base, stops and its car;
//!   - the showcase `face` verb ([`parse_face`]) turns the camera to a yaw and pitch without
//!     moving the body, as a mouse turn does, so a rig can walk a route of straight legs on the
//!     held movement keys (`hold`) without a placement between them.
//!
//! Every height is world metres, the camera's frame: a rig subtracts the home's box corner
//! (`home_session.home.box.min`) for the home's own metres.

use crate::engine::footing::{lifts, ElevatorCars};
use crate::engine::state::EngineState;
use crate::gui::RoomBounds;
use crate::ship::ship_structure::ShipStructure;

/// The room the HUD names, as the probe reports it: its id (the zone's, `room-upper-hall`), the
/// name the HUD shows, and its floor (the box's foot, world metres). Null outside every room.
pub(crate) fn room_json(room: Option<&RoomBounds>) -> serde_json::Value {
    match room {
        Some(r) => serde_json::json!({ "id": r.id, "name": r.display_name, "floor": r.min.y }),
        None => serde_json::Value::Null,
    }
}

/// Every elevator aboard, bottom stop first, as the probe's done block lists it: its piece id, its
/// key in `ElevatorCars`, its base (world metres), its stops (metres over the base), and its car
/// now (its height over the base, the stop it is heading for, whether someone rides it; a car
/// nobody has moved yet stands at its base, as `elevator_tick` starts it).
pub(crate) fn lifts_json(ship: &ShipStructure, cars: &ElevatorCars) -> Vec<serde_json::Value> {
    lifts(ship)
        .into_iter()
        .map(|l| {
            let id = ship.zones.get(l.key.0).and_then(|z| z.body.structures.get(l.key.1)).map(|p| p.id.clone());
            let (car, target, riding) = cars.get(&l.key).copied().unwrap_or((0.0, 0.0, false));
            serde_json::json!({
                "id": id,
                "key": [l.key.0, l.key.1],
                "base": [l.base.x, l.base.y, l.base.z],
                "stops": l.stops,
                "car_m": car,
                "target_m": target,
                "riding": riding,
            })
        })
        .collect()
}

/// Every elevator car's height over its base this frame, in `lifts` order (the done block's), and
/// the stop it is heading for: [[car, target], ...]. Empty without a ship.
pub(crate) fn cars_now(ship: Option<&ShipStructure>, cars: &ElevatorCars) -> Vec<[f32; 2]> {
    let Some(ship) = ship else { return Vec::new() };
    lifts(ship)
        .into_iter()
        .map(|l| {
            let (car, target, _) = cars.get(&l.key).copied().unwrap_or((0.0, 0.0, false));
            [car, target]
        })
        .collect()
}

/// One recorder frame's `storey` block for the running game (see the module doc).
pub(crate) fn frame_json(state: &EngineState) -> serde_json::Value {
    let c = &state.controller;
    let eye = state.camera.position;
    let (landings, last) = c.landings_taken();
    let gs = &state.gui_state;
    serde_json::json!({
        "floor": c.ground_floor(),
        "ceiling": c.ceiling(),
        "on_ladder": c.on_ladder(eye.y),
        "at_ladder": c.in_climb_zone(),
        "on_lift": c.on_lift(),
        "aboard": state.aboard_station,
        "room": room_json(crate::gui::pages::hud::room_card_at(&gs.room_bounds, gs.ship_structure.as_ref(), &gs.keybinds, eye).as_deref()),
        // The HUD (and its room card) is drawn this frame: lib.rs's own conditions for hud::draw.
        "hud_shown": gs.active_page == crate::gui::GuiPage::None && gs.show_hud && !gs.showroom_active && !gs.construction_active,
        "cars": cars_now(gs.ship_structure.as_ref(), &state.elevator_state),
        "landings": landings,
        "last_m_s": last,
    })
}

/// The recorder's done-JSON `storeys` block for the running game: every elevator aboard.
pub(crate) fn done_json(state: &EngineState) -> serde_json::Value {
    let elevators = state.gui_state.ship_structure.as_ref().map(|s| lifts_json(s, &state.elevator_state)).unwrap_or_default();
    serde_json::json!({ "elevators": elevators })
}

/// Read a showcase `face` value, "yaw,pitch" in radians (yaw 0 looks north, -Z; +PI/2 east;
/// pitch positive up), as a mouse turn leaves the camera. None for anything else (a typo turns
/// nothing). The pitch is held inside the camera's own limit, just short of straight up or down.
pub(crate) fn parse_face(raw: &str) -> Option<(f32, f32)> {
    let v: Vec<f32> = raw.split(',').map(|p| p.trim().parse::<f32>()).collect::<Result<_, _>>().ok()?;
    let [yaw, pitch] = v[..] else { return None };
    if !yaw.is_finite() || !pitch.is_finite() {
        return None;
    }
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    Some((yaw, pitch.clamp(-max_pitch, max_pitch)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::footing::test_aboard::Aboard;
    use glam::Vec3;

    fn room(id: &str, name: &str, min: Vec3, max: Vec3) -> RoomBounds {
        RoomBounds {
            id: id.into(),
            min,
            max,
            display_name: name.into(),
            purpose: String::new(),
            actions: Vec::new(),
            action_pages: Vec::new(),
            access: String::new(),
        }
    }

    /// THE HUD NAMES THE ROOM OF THE STOREY THE EYE IS ON. A dining room under a bedroom of the
    /// same x/z rectangle (the homestead's quiet wing, docs/design/homestead-upper-floor.md): an
    /// eye downstairs is in the dining room, one upstairs in the bedroom, whichever is listed
    /// first; an eye outside both is in no room. The probe reports the pick by id, name and floor.
    ///
    /// Seen red 2026-10-21 with `room_at` testing x and z only: "an eye upstairs" (left:
    /// Some("room-dining"), right: Some("room-bedroom")).
    #[test]
    fn the_hud_names_the_room_of_the_eyes_storey() {
        let dining = room("room-dining", "Dining Room", Vec3::new(34.0, 0.0, 30.0), Vec3::new(42.0, 3.0, 38.0));
        let bedroom = room("room-bedroom", "Bedroom", Vec3::new(34.0, 3.0, 30.0), Vec3::new(42.0, 6.0, 38.0));
        for rooms in [vec![dining.clone(), bedroom.clone()], vec![bedroom.clone(), dining.clone()]] {
            let at = |y: f32| crate::gui::pages::hud::room_at(&rooms, Vec3::new(38.0, y, 34.0)).map(|r| r.id.as_str());
            assert_eq!(at(1.7), Some("room-dining"), "an eye downstairs");
            assert_eq!(at(4.7), Some("room-bedroom"), "an eye upstairs");
            assert_eq!(at(7.0), None, "an eye over both");
        }
        let v = room_json(Some(&bedroom));
        assert_eq!((v["id"].as_str(), v["name"].as_str(), v["floor"].as_f64()), (Some("room-bedroom"), Some("Bedroom"), Some(3.0)));
        assert_eq!(room_json(None), serde_json::Value::Null);
    }

    /// THE ELEVATORS AS THE PROBE LISTS THEM. The synthetic two-storey home's elevator-1 (engine/
    /// footing.rs `test_aboard`): its id, base and stops 0 and 3; a car nobody has moved stands at
    /// its base, and a car the walk has sent up reads where it is and where it is heading, in the
    /// done block and in the per-frame list alike.
    ///
    /// Seen red 2026-10-21 with `car_m` reporting the stop the car heads for: "assertion `left ==
    /// right` failed" (left: (Some(3.0), Some(3.0), Some(true)), right: (Some(1.25), Some(3.0),
    /// Some(true))).
    #[test]
    fn the_probe_lists_each_elevator_its_stops_and_its_car() {
        let mut w = Aboard::two_storeys();
        let v = lifts_json(&w.ship, &w.cars);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0]["id"].as_str(), Some("elevator-1"));
        assert_eq!(v[0]["stops"], serde_json::json!([0.0, 3.0]));
        assert_eq!((v[0]["car_m"].as_f64(), v[0]["riding"].as_bool()), (Some(0.0), Some(false)));
        assert_eq!(cars_now(Some(&w.ship), &w.cars), vec![[0.0, 0.0]]);
        w.cars.insert((0, 0), (1.25, 3.0, true));
        let v = lifts_json(&w.ship, &w.cars);
        assert_eq!((v[0]["car_m"].as_f64(), v[0]["target_m"].as_f64(), v[0]["riding"].as_bool()), (Some(1.25), Some(3.0), Some(true)));
        assert_eq!(cars_now(Some(&w.ship), &w.cars), vec![[1.25, 3.0]]);
        assert!(cars_now(None, &w.cars).is_empty(), "no ship, no cars");
    }

    /// The `face` verb's words: a yaw and a pitch, the pitch held short of straight up.
    ///
    /// Seen red 2026-10-21 with the pitch not clamped: "the pitch is held short of straight up: 9".
    #[test]
    fn the_face_verb_reads_a_yaw_and_a_pitch() {
        assert_eq!(parse_face("-1.5708, 0.2"), Some((-1.5708, 0.2)));
        assert_eq!(parse_face("3.14159,0"), Some((3.14159, 0.0)));
        let (_, p) = parse_face("0,9").unwrap();
        assert!(p < std::f32::consts::FRAC_PI_2, "the pitch is held short of straight up: {p}");
        for bad in ["", "1", "1,2,3", "a,b", "NaN,0", "inf,0"] {
            assert_eq!(parse_face(bad), None, "{bad:?}");
        }
    }

    /// THE PROBE IS WIRED IN: every recorder frame carries the `storey` block, the done JSON the
    /// `storeys` block, the showcase poll reads the `face` verb, and the HUD's room card is drawn
    /// from `room_at` (the pick this probe reports). Without these the functions above are tested
    /// and never run, or the probe reports a pick the HUD does not make.
    ///
    /// Seen red 2026-10-21 with the verb read as `grab("facing")`: "src/engine/ipc.rs never has
    /// grab(\"face\")".
    #[test]
    fn the_probe_is_wired_into_the_dev_ipc_and_the_hud() {
        let ipc = include_str!("ipc.rs").replace("\r\n", "\n");
        for wired in [
            "\"storey\": crate::engine::storey_probe::frame_json(state)",
            "\"storeys\": crate::engine::storey_probe::done_json(state)",
            "grab(\"face\")",
            "crate::engine::storey_probe::parse_face(&raw)",
        ] {
            assert!(ipc.contains(wired), "src/engine/ipc.rs never has {wired}");
        }
        let hud = include_str!("../gui/pages/hud.rs").replace("\r\n", "\n");
        assert!(
            hud.contains("let current_room_info = room_card_at(&state.room_bounds, state.ship_structure.as_ref(), &state.keybinds, cam_pos);"),
            "the HUD's room card no longer picks its place with room_card_at, so the probe would report a pick the HUD does not make"
        );
    }
}
