//! DOOR POINTS (increment 2 of docs/design/ship-homes-and-logistics.md, "Meet in the Commons"):
//! the ship's places and every corridor's door points, computed by the Rust corridor geometry, so a
//! rig that walks a scripted player (scripts/second-player.js) or the game itself from a home's door
//! into the Commons never re-implements corridor maths in JavaScript. The game reports it on request
//! (engine/ipc.rs, `debug/door_points_request.json`); scripts/lib/copresence-judge.js `doorRoute`
//! walks it.
//!
//! PLACES are the boxes a person stands in: every shared zone, and every plot (the home on it is
//! the player's own on their plot, a neighbour's default design on every other, src/ship/
//! neighbours.rs). A home put away (a guest's, `ShipStructure::put_home_away`) is no place of the
//! ship. DOORS join two places: every ship corridor, and every plot's door corridor, each with its two
//! mouths and a step a metre inside each end at eye height, which is where a walk goes through.

use crate::ship::ship_structure::{CorridorAxis, CorridorGeom, HomeDesign, ShipStructure, HOME_ZONE_ID, SPAWN_EYE_HEIGHT_M};
use glam::Vec3;
use serde::Serialize;

/// How far inside a place, from a door's mouth, its step stands, metres: past the mouth's door
/// panels and clear of the wall either side.
pub const STEP_IN_M: f32 = 1.0;

/// A place a person stands in: a shared zone or a plot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoutePlace {
    /// "zone:<zone id>" or "plot:<plot id>": a zone and a plot may share an id in the files.
    pub id: String,
    /// "zone" or "plot".
    pub kind: &'static str,
    /// A zone's purpose (commons, street, ...); "home" for a plot.
    pub purpose: String,
    /// The box, ship metres (y is the deck and the top).
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// A plot's door: where its holder arrives, at eye height in ship metres, with the home drawn
    /// there (this player's own on their own plot, the kind's default on a neighbour's), and the
    /// same point in plot-local metres (x, z), which is what a game names in its `game_join`.
    pub door: Option<[f32; 3]>,
    pub door_local: Option<[f32; 2]>,
    /// True for the plot this game's home stands on.
    pub own: bool,
}

/// A door between two places: a corridor, as the game draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RouteDoor {
    /// The places it joins (`RoutePlace::id`).
    pub from: String,
    pub to: String,
    /// The axis the tube runs along ("x" or "z") and its centreline across the run.
    pub axis: &'static str,
    pub lat: f32,
    /// The two mouth centres on the floor: on `from`'s wall, then on `to`'s.
    pub mouths: [[f32; 3]; 2],
    /// A step `STEP_IN_M` inside each place from its mouth, at eye height: in `from`, then in `to`.
    pub steps: [[f32; 3]; 2],
    /// The tube's box (min, max): floor to lid, wall to wall, mouth to mouth.
    pub tube: [[f32; 3]; 2],
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoorPoints {
    pub ship_hash: String,
    pub places: Vec<RoutePlace>,
    pub doors: Vec<RouteDoor>,
}

fn door_of(from: String, to: String, g: &CorridorGeom) -> RouteDoor {
    let dir = (g.end_to - g.end_from).normalize_or_zero();
    let eye = Vec3::Y * SPAWN_EYE_HEIGHT_M;
    let hw = g.width * 0.5;
    let (lo, hi) = match g.axis {
        CorridorAxis::X => (Vec3::new(g.start, g.floor_y, g.lat - hw), Vec3::new(g.end, g.floor_y + g.height, g.lat + hw)),
        CorridorAxis::Z => (Vec3::new(g.lat - hw, g.floor_y, g.start), Vec3::new(g.lat + hw, g.floor_y + g.height, g.end)),
    };
    RouteDoor {
        from,
        to,
        axis: if g.axis == CorridorAxis::X { "x" } else { "z" },
        lat: g.lat,
        mouths: [g.end_from.to_array(), g.end_to.to_array()],
        steps: [(g.end_from - dir * STEP_IN_M + eye).to_array(), (g.end_to + dir * STEP_IN_M + eye).to_array()],
        tube: [lo.to_array(), hi.to_array()],
    }
}

/// The door points of the ship this game runs (see the top of this file). Pure.
pub fn door_points(ship: &ShipStructure) -> DoorPoints {
    let mut places = Vec::new();
    let mut doors = Vec::new();
    for z in ship.zones.iter().filter(|z| z.id != HOME_ZONE_ID) {
        let o = z.origin_vec();
        places.push(RoutePlace {
            id: format!("zone:{}", z.id),
            kind: "zone",
            purpose: z.purpose.clone(),
            min: o.to_array(),
            max: (o + Vec3::new(z.body.width, z.body.height, z.body.depth)).to_array(),
            door: None,
            door_local: None,
            own: false,
        });
    }
    // Ship corridors between shared zones (the home's own corridor is its plot's door, below).
    for c in ship.corridors.iter().filter(|c| !ship.is_plot_door(c)) {
        if let Ok(g) = ship.corridor_geometry(c) {
            doors.push(door_of(format!("zone:{}", c.from_zone), format!("zone:{}", c.to_zone), &g));
        }
    }
    let own_plot = ship.home_plot().map(|p| p.id.clone());
    for p in &ship.plots {
        let own = own_plot.as_deref() == Some(p.id.as_str());
        // The home drawn on it: this player's own design on their plot, else the kind's default.
        let design: Option<HomeDesign> = if own { ship.home_design() } else { HomeDesign::built_in(&p.kind) };
        let (lo, hi) = p.aabb();
        let door = design.as_ref().map(|d| ShipStructure::plot_spawn(p, d));
        places.push(RoutePlace {
            id: format!("plot:{}", p.id),
            kind: "plot",
            purpose: "home".to_string(),
            min: lo.to_array(),
            max: hi.to_array(),
            door: door.map(|d| d.to_array()),
            door_local: door.map(|d| [d.x - p.origin.0, d.z - p.origin.2]),
            own,
        });
        if let Ok(g) = ship.plot_door_tube(p, design.as_ref()) {
            doors.push(door_of(format!("plot:{}", p.id), format!("zone:{}", p.door.zone), &g));
        }
    }
    DoorPoints { ship_hash: ship.ship_hash(), places, doors }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(plot: &str) -> ShipStructure {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        ShipStructure::load_and_assemble(&data, Some(plot)).expect("the shipped ship assembles")
    }

    fn place<'a>(d: &'a DoorPoints, id: &str) -> &'a RoutePlace {
        d.places.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("no place {id}"))
    }

    fn door<'a>(d: &'a DoorPoints, from: &str) -> &'a RouteDoor {
        d.doors.iter().find(|x| x.from == from).unwrap_or_else(|| panic!("no door from {from}"))
    }

    fn inside(p: [f32; 3], pl: &RoutePlace) -> bool {
        (0..3).all(|k| p[k] >= pl.min[k] - 1e-3 && p[k] <= pl.max[k] + 1e-3)
    }

    /// The shipped ship, from either plot: the Commons, First Street and both plots are places; the
    /// three corridors are doors (the Commons to the street, each plot to its zone); each door's
    /// steps stand inside the two places it joins, and its mouths are the corridor the game draws
    /// (the home's own corridor for its plot, the neighbour's tube for the other). The numbers are
    /// the ship file's (section 2.4 of the design). Seen red 2026-10-04 with the steps taken a
    /// metre OUTWARD from each mouth instead of in: "plot:p1 -> zone:commons: step [56.0, 1.7, 40.0]
    /// is not inside plot:p1".
    #[test]
    fn every_door_of_the_shipped_ship_joins_two_places_where_the_game_draws_it() {
        for own in ["p1", "p2"] {
            let ship = on(own);
            let d = door_points(&ship);
            assert_eq!(d.ship_hash, ship.ship_hash());
            let ids: Vec<&str> = d.places.iter().map(|p| p.id.as_str()).collect();
            assert_eq!(ids, ["zone:commons", "zone:street-1", "plot:p1", "plot:p2"], "on {own}");
            assert_eq!(d.doors.len(), 3, "on {own}: {:?}", d.doors);
            for x in &d.doors {
                for (step, end) in x.steps.iter().zip([&x.from, &x.to]) {
                    assert!(inside(*step, place(&d, end)), "{} -> {}: step {step:?} is not inside {end}", x.from, x.to);
                }
            }
            let street = door(&d, "zone:commons");
            assert_eq!((street.to.as_str(), street.axis, street.lat), ("zone:street-1", "z", 70.0));
            assert_eq!(street.mouths, [[70.0, 0.0, 75.0], [70.0, 0.0, 85.0]]);
            let p1 = door(&d, "plot:p1");
            assert_eq!((p1.to.as_str(), p1.axis, p1.lat), ("zone:commons", "x", 40.0));
            assert_eq!(p1.mouths, [[55.0, 0.0, 40.0], [65.0, 0.0, 40.0]]);
            assert_eq!(p1.steps, [[54.0, 1.7, 40.0], [66.0, 1.7, 40.0]]);
            let p2 = door(&d, "plot:p2");
            assert_eq!((p2.to.as_str(), p2.lat), ("zone:street-1", 139.0));
            // Both plots' doors: where their holders arrive with the default homestead (its spawn).
            assert_eq!(place(&d, "plot:p1").door, Some([53.5, 1.7, 40.5]));
            assert_eq!(place(&d, "plot:p2").door, Some([53.5, 1.7, 139.5]));
            assert_eq!(place(&d, "plot:p2").door_local, Some([53.5, 40.5]));
            assert!(place(&d, &format!("plot:{own}")).own);
            // The home's own corridor is reported as its plot's door, with the geometry the game draws.
            let home_corridor = ship.corridors.iter().find(|c| ship.is_plot_door(c)).unwrap();
            let g = ship.corridor_geometry(home_corridor).unwrap();
            assert_eq!(door(&d, &format!("plot:{own}")).mouths, [g.end_from.to_array(), g.end_to.to_array()]);
        }
    }

    /// The rig's own tests (scripts/tests/copresence-judge.test.js) route on a copy of this report
    /// for the shipped ship from p1 (scripts/tests/fixtures/door-points-p1.json); it must be what the
    /// game really reports, or those tests prove routes on a ship that does not exist. Everything but
    /// the hash, which the routes never read. When the ship file changes this fails: write the
    /// report it prints into the fixture. Seen red 2026-10-04 with the fixture's p2 door lat set to
    /// 138: the two values differ at `doors[2].lat`.
    #[test]
    fn the_rigs_fixture_is_what_the_game_reports() {
        let ours = serde_json::to_value(door_points(&on("p1"))).unwrap();
        // Through text, so f32 values compare as the decimals they print as.
        let mut ours: serde_json::Value = serde_json::from_str(&ours.to_string()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/door-points-p1.json");
        let mut fixture: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        ours.as_object_mut().unwrap().remove("ship_hash");
        fixture.as_object_mut().unwrap().remove("ship_hash");
        let ours_f32 = |v: &serde_json::Value| -> String { serde_json::to_string(&round_floats(v)).unwrap() };
        assert_eq!(ours_f32(&ours), ours_f32(&fixture), "the report differs from the rig's fixture; the report is:\n{}", serde_json::to_string_pretty(&door_points(&on("p1"))).unwrap());
    }

    /// Every number rounded to 4 decimals (an f32 printed through serde_json carries its binary
    /// tail, 1.7 as 1.7000000476837158).
    fn round_floats(v: &serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Number(n) => serde_json::json!((n.as_f64().unwrap_or(0.0) * 1e4).round() / 1e4),
            serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(round_floats).collect()),
            serde_json::Value::Object(o) => serde_json::Value::Object(o.iter().map(|(k, x)| (k.clone(), round_floats(x))).collect()),
            other => other.clone(),
        }
    }

    /// A guest's home put away is no place of the ship, and every plot is still one: the guest walks
    /// past both homes as a neighbour's.
    #[test]
    fn a_home_put_away_is_no_place_of_the_ship() {
        let d = door_points(&on("p1").put_home_away().expect("put away"));
        let ids: Vec<&str> = d.places.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["zone:commons", "zone:street-1", "plot:p1", "plot:p2"]);
        assert!(d.places.iter().all(|p| !p.own), "a guest owns no plot");
        assert_eq!(d.doors.len(), 3);
    }
}
