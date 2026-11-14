//! From ship/build_frames.rs's tests: the shipped ship's build frames, and where a piece placed by
//! the simulation's build rules (`placement_pose`) lands against a plot's line.

use humanity_ship::build_frames::*;
use humanity_ship::ship_structure::ShipStructure;
use crate::construction::placement::placement_pose;
use crate::construction::BlueprintRegistry;
use glam::{Quat, Vec3};
use humanity_core::ecs::components::Transform;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn assembled(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).expect("the shipped ship assembles")
}

fn shipped_blueprints() -> BlueprintRegistry {
    BlueprintRegistry::from_ron(include_bytes!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/blueprints/basic.ron"))).unwrap()
}

/// THE SHIPPED SHIP'S FRAMES, the same from every side. The relay's ship file, the game's
/// ship assembled at p1 and at p2, and a guest's ship (the home put away) all give the same
/// frames: the shared zones (the Commons and First Street, and since ship homes increment 6
/// the cabin block's hall and the South Commons), then the twelve homesteads and the sixteen
/// cabins, with the ship file's corners and sizes, and never the assembled ship's `home`
/// zone, which is the
/// player's own home standing inside its plot. `frame_at` finds the plot, the street or the
/// Commons a point stands over, and nothing in a corridor or between two plots;
/// `parse_frame_id` knows a plot from a zone and refuses a planet site; moving a pose to a
/// frame's metres and back gives the same bits; and the floor distances from p1's door are
/// the ones the relay's view radius (250 m) will judge by: p4 at 256.5 m is out of it.
/// Seen red 2026-10-05 with the `home` zone not filtered out of `of_ship`: "assembled at p1:
/// the game's frames are the relay's", left: `BuildFrames { frames: [BuildFrame { id:
/// "zone:home", kind: Zone, origin: Vec3(0.0, 0.0, 0.0), size: Vec3(55.0, 3.0, 89.0) },
/// BuildFrame { id: "zone:commons", ...`. The home fills p1's box and comes first, so
/// `frame_at` would have taken a piece in your own home for one in the ship's shared spaces.
#[test]
fn frames_of_the_shipped_ship() {
    let relay = ShipStructure::ship_for_relay(&data_dir()).expect("the relay's ship file loads");
    let frames = BuildFrames::of_ship(&relay);
    let ids: Vec<&str> = frames.frames.iter().map(|f| f.id.as_str()).collect();
    // Since ship homes increment 6 (lane 1A): the cabin block's hall and the South Commons
    // are zones too, and the sixteen cabins plots, after the twelve homesteads.
    // And since 2026-11-13 Row Street, its five alleys and its green.
    let mut want: Vec<String> = ["zone:commons", "zone:street-1", "zone:cabins-1", "zone:commons-2", "zone:row-street-1", "zone:row-alley-1", "zone:row-alley-2", "zone:row-alley-3", "zone:row-alley-4", "zone:row-alley-5", "zone:row-green-1"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    want.extend((1..=12).map(|n| format!("plot:p{n}")));
    want.extend((1..=16).map(|n| format!("plot:c{n}")));
    // Since 2026-11-13 the room program's two homesteads, after the cabins.
    want.extend(["plot:h1".to_string(), "plot:o1".to_string()]);
    // And the twenty row houses of Row Street after them.
    want.extend((1..=20).map(|n| format!("plot:r{n}")));
    assert_eq!(ids, want, "the relay's frames");
    for (game, ship) in [("assembled at p1", assembled("p1")), ("assembled at p2", assembled("p2"))] {
        assert_eq!(BuildFrames::of_ship(&ship), frames, "{game}: the game's frames are the relay's");
    }
    let guest = assembled("p1").put_home_away().expect("the home can be put away");
    assert_eq!(BuildFrames::of_ship(&guest), frames, "a guest's frames are the relay's");
    assert!(frames.get("zone:home").is_none());

    // The ship file's numbers (data/blueprints/ship_structure.ron).
    let commons = frames.get("zone:commons").unwrap();
    assert_eq!((commons.kind, commons.origin, commons.size), (FrameKind::Zone, Vec3::new(65.0, 0.0, 20.0), Vec3::new(34.0, 8.0, 55.0)));
    let street = frames.get("zone:street-1").unwrap();
    assert_eq!((street.origin, street.size), (Vec3::new(65.0, 0.0, 85.0), Vec3::new(10.0, 4.0, 1100.0)));
    let p3 = frames.get("plot:p3").unwrap();
    // 6 m tall since ship homes increment 7 (two storeys).
    assert_eq!((p3.kind, p3.origin, p3.size), (FrameKind::Plot, Vec3::new(0.0, 0.0, 198.0), Vec3::new(55.0, 6.0, 89.0)));
    assert_eq!((p3.place_id(), p3.plot_id(), commons.plot_id()), ("p3", Some("p3"), None));

    // Where a point stands.
    let at = |x: f32, z: f32| frames.frame_at(Vec3::new(x, 0.0, z)).map(|f| f.id.as_str());
    assert_eq!(at(30.0, 240.0), Some("plot:p3"));
    assert_eq!(at(55.0, 240.0), Some("plot:p3"), "a wall's centre on the plot's line is the plot's");
    assert_eq!(at(80.0, 40.0), Some("zone:commons"));
    assert_eq!(at(70.0, 500.0), Some("zone:street-1"));
    assert_eq!(at(82.0, 89.0), Some("plot:c1"), "the first cabin");
    assert_eq!(at(78.5, 100.0), Some("zone:cabins-1"), "the cabins' hall");
    assert_eq!(at(60.0, 40.0), None, "p1's door corridor is no frame");
    assert_eq!(at(30.0, 94.0), None, "nor the gap between p1 and p2");
    assert_eq!(at(-5.0, 40.0), None, "nor off the ship");

    // Frame ids.
    assert_eq!(parse_frame_id("plot:p3"), Some((FrameKind::Plot, "p3")));
    assert_eq!(parse_frame_id("zone:commons"), Some((FrameKind::Zone, "commons")));
    for not_a_frame in ["site:moon-1", "p3", "plot:", "zone:", "", "home"] {
        assert_eq!(parse_frame_id(not_a_frame), None, "{not_a_frame:?}");
    }
    assert_eq!((plot_frame_id("p3"), zone_frame_id("commons")), ("plot:p3".to_string(), "zone:commons".to_string()));

    // Ship metres to frame metres and back, exactly, for the poses the placer makes.
    let reg = shipped_blueprints();
    let world = hecs::World::new();
    for (id, x, z, turns) in [("wood_foundation", 30.0, 240.0, 0u8), ("wood_wall", 55.0, 260.0, 1), ("roof", 12.0, 285.0, 3)] {
        let ship = placement_pose(reg.get(id).unwrap(), Vec3::new(x, 0.0, z), turns, &world, &reg, None);
        let local = p3.to_local(&ship);
        assert_eq!(local.position, Vec3::new(x, 0.0, z - 198.0), "{id}: measured from p3's corner");
        let back = p3.to_ship(&local);
        let bits = |t: &Transform| (t.position.to_array().map(f32::to_bits), t.rotation.to_array().map(f32::to_bits), t.scale.to_array().map(f32::to_bits));
        assert_eq!(bits(&back), bits(&ship), "{id}: there and back, to the bit");
    }
    let lifted = Transform { position: Vec3::new(30.0, 3.2, 240.0), rotation: Quat::IDENTITY, scale: Vec3::ONE };
    let back = p3.to_ship(&p3.to_local(&lifted)).position;
    assert_eq!(back.to_array().map(f32::to_bits), lifted.position.to_array().map(f32::to_bits), "a roof's height survives too");

    // How far each frame's floor is from p1's door (door_points: (53.5, 1.7, 40.5)).
    let door = Vec3::new(53.5, 1.7, 40.5);
    let dist = |id: &str| frames.get(id).unwrap().floor_distance(door);
    assert_eq!(dist("plot:p1"), 0.0);
    assert_eq!(dist("plot:p2"), 58.5);
    assert_eq!(dist("plot:p3"), 157.5);
    assert_eq!(dist("plot:p4"), 256.5, "out of a 250 m view");
    assert_eq!(dist("zone:commons"), 11.5);
    assert!((dist("zone:street-1") - (11.5f32 * 11.5 + 44.5 * 44.5).sqrt()).abs() < 1e-4);
}

/// A FOOTPRINT OVER THE PLOT LINE IS OUTSIDE (the plot cases of engine/build_place.rs's
/// `a_piece_goes_aboard_only_inside_your_own_plot`, moved here with the rule). On p1
/// (0..55 x 0..89), with real blueprints placed the way the ghost places them: a foundation
/// in the yard and one flush in the far corner are inside; a 4 x 4 m foundation centred at
/// (54, 40) is not, because it spans x 52..56 across the home's door (the critic's case
/// against a centre-only test); a wall on the east line running north-south is inside and
/// running east-west it reaches x 57; every wall laid ON a line (stone 0.3 m, wood 0.2,
/// metal 0.15), on all four lines, is inside, the case the slack is for; a chest turned to
/// run east-west on the east line pokes 30 cm over and is not; nor is the Commons or p2 for
/// p1. On p2 (corner z 99) the same pieces measured from p2's corner follow its box. Heights:
/// a roof on walls is inside, a piece below the deck by more than a metre or over 40 m is not.
/// Seen red 2026-10-05 with the slack doubled to 0.30 m: "a chest pokes 30 cm over the line".
#[test]
fn a_footprint_over_the_plot_line_is_outside() {
    let frames = BuildFrames::of_ship(&ShipStructure::ship_for_relay(&data_dir()).unwrap());
    let (p1, p2) = (frames.get("plot:p1").unwrap(), frames.get("plot:p2").unwrap());
    let reg = shipped_blueprints();
    let world = hecs::World::new();
    let pose = |id: &str, x: f32, z: f32, turns: u8| {
        placement_pose(reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron")), Vec3::new(x, 0.0, z), turns, &world, &reg, None)
    };
    let inside = |f: &BuildFrame, id: &str, x: f32, z: f32, turns: u8| f.footprint_inside(&f.to_local(&pose(id, x, z, turns)));
    assert!(inside(p1, "wood_foundation", 30.0, 20.0, 0), "a foundation in the yard");
    assert!(inside(p1, "wood_foundation", 53.0, 87.0, 0), "flush in the far corner");
    assert!(!inside(p1, "wood_foundation", 54.0, 40.0, 0), "centred inside, but it covers the door and a metre of the corridor");
    assert!(inside(p1, "wood_wall", 55.0, 30.0, 1), "a wall on the east line, running north-south");
    assert!(!inside(p1, "wood_wall", 55.0, 40.0, 0), "running east-west from the line it reaches x 57");
    // Every wall laid on each of the four lines: half its thickness over, never more than the slack.
    for wall in ["stone_wall", "wood_wall", "metal_wall"] {
        for (x, z, turns) in [(55.0, 30.0, 1u8), (55.0, 30.0, 3), (0.0, 30.0, 1), (0.0, 30.0, 3), (20.0, 0.0, 0), (20.0, 0.0, 2), (20.0, 89.0, 0), (20.0, 89.0, 2)] {
            assert!(inside(p1, wall, x, z, turns), "a {wall} laid on p1's line at ({x}, {z}), {turns} turns");
        }
    }
    // A chest (1.0 x 0.6) turned so its 0.6 m side runs east-west, centred on the east line: 0.3 m over.
    assert!(!inside(p1, "storage_chest", 55.0, 30.0, 1), "a chest pokes 30 cm over the line");
    assert!(!inside(p1, "wood_foundation", 80.0, 40.0, 0), "the Commons is not p1");
    assert!(!inside(p1, "wood_foundation", 30.0, 140.0, 0), "p2 is not p1");
    assert!(inside(p2, "wood_foundation", 30.0, 140.0, 0), "p2's yard, measured from p2's corner");
    assert!(!inside(p2, "wood_foundation", 30.0, 20.0, 0), "and p1 is not p2");
    assert!(inside(p2, "wood_wall", 20.0, 99.0, 0), "a wall on p2's north line");
    assert!(!inside(p2, "wood_foundation", 20.0, 98.0, 0), "a foundation a metre over it");

    // Heights: no plot box bounds them, a sanity range does.
    let roof = p1.to_local(&Transform { position: Vec3::new(20.0, 3.2, 20.0), ..pose("roof", 20.0, 20.0, 0) });
    assert!(p1.height_inside(&roof), "a roof on walls on a foundation, inside the 6 m plot box");
    let sunk = Transform { position: Vec3::new(20.0, -1.5, 20.0), ..roof.clone() };
    let high = Transform { position: Vec3::new(20.0, 39.9, 20.0), ..roof.clone() };
    assert!(!p1.height_inside(&sunk) && !p1.height_inside(&high), "below the deck, or reaching past 40 m");
    // A NaN reads as outside, never inside: in x for the footprint, in y for the height (each
    // looks only at its own axes; the relay's `validate_pose` refuses any NaN before either).
    let nan_x = Transform { position: Vec3::new(f32::NAN, 0.0, 20.0), ..roof.clone() };
    let nan_y = Transform { position: Vec3::new(20.0, f32::NAN, 20.0), ..roof };
    assert!(!p1.footprint_inside(&nan_x), "a NaN in x is outside the footprint");
    assert!(!p1.height_inside(&nan_y), "a NaN in y is outside the heights");
}
