//! The ship view's tests (Maps increment 3). Each was seen red first; what broke it and what it
//! said is in its comment.

use super::*;
use crate::gui::pages::maps::camera::tests::feed_at;
use crate::ship::ship_structure::ShipStructure;

fn shipped() -> (ShipStructure, Vec<RoomBounds>) {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let ship = ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles at p1");
    let rooms = crate::ship::room_types::RoomTypeRegistry::load(&data).room_bounds(&ship.generate_meshes().room_info);
    (ship, rooms)
}

/// The page as opening it leaves it: the feed of the shipped home, following you.
fn opened() -> MapsState {
    let me = feed_at(3600.0, DVec3::ZERO);
    let mut maps = MapsState::default();
    maps.sim_time_seconds = me.sky_seconds;
    maps.camera.open_on_me(Some(&me));
    maps.me = Some(me);
    maps
}

/// THE UPSTAIRS BEDROOM IS NAMED ON STOREY 2 AND THE ROOM UNDER IT IS NOT (the design's
/// `maps-ship-upstairs` proof, held without an engine): on the shipped homestead, the rooms on the
/// upper level include the Bedroom and never the Dining Room under it, and the ground level the
/// other way round. The names are the HUD's (`RoomBounds::display_name`), and every room has a
/// fill colour from its room type in data/rooms.ron when it has a type.
///
/// Seen red 2026-10-22 with a room's level found from the TOP of its box: "the ground: []" (every
/// room's top is at or over the upper floor's 3 m, so the ground level had no room at all).
#[test]
fn the_bedroom_is_upstairs_and_the_dining_room_under_it_is_not() {
    let (ship, rooms) = shipped();
    let levels = levels::ship_levels(&ship);
    let names = |i: usize| rooms_on_level(&rooms, &levels, i).iter().map(|r| r.display_name.clone()).collect::<Vec<_>>();
    let (ground, upstairs) = (names(0), names(1));
    assert!(upstairs.iter().any(|n| n == "Bedroom"), "Storey 2 has no Bedroom: {upstairs:?}");
    assert!(!upstairs.iter().any(|n| n == "Dining Room"), "Storey 2 holds the Dining Room, which is under it");
    assert!(ground.iter().any(|n| n == "Dining Room") && !ground.iter().any(|n| n == "Bedroom"), "the ground: {ground:?}");
    let bedroom = rooms.iter().find(|r| r.display_name == "Bedroom").unwrap();
    assert!(bedroom.fill.is_some(), "the bedroom has no fill colour from data/rooms.ron");
}

/// A room the data gives no name is filled but not named: on the shipped ship's ground level the
/// corridor tubes (`corridor_0`) and a zone's leftover floor (`commons:room_1`), which the HUD
/// shows under their ids, are not named, and the Dining Room is.
///
/// Seen red 2026-10-23 with every room named: "corridor_0 is named on the map" (the first rig run
/// showed "corridor_0" and "commons:room_1" over the Commons and its corridors).
#[test]
fn a_room_with_no_name_in_the_data_is_not_named() {
    let (_, rooms) = shipped();
    let by_id = |id: &str| rooms.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no room {id}"));
    for id in ["corridor_0", "commons:room_1"] {
        assert!(!room_is_named(by_id(id)), "{id} is named on the map");
    }
    let dining = rooms.iter().find(|r| r.display_name == "Dining Room").expect("the Dining Room");
    assert!(room_is_named(dining));
}

/// The picker shows near your ship and a level picked is shown there; zoomed far out, neither
/// (the whole ship is the view again); on the plan with nothing picked, your own level.
///
/// Seen red 2026-10-22 with `shown_level` not asking whether the picker shows: "8 ship lengths
/// out, Storey 2 is still shown cut open".
#[test]
fn a_level_is_shown_near_your_ship_and_not_from_far_out() {
    let mut maps = opened();
    assert!(picker_shown(&maps), "the opening view (1.5 ship lengths) shows the picker");
    assert_eq!(shown_level(&maps), None, "the page opens on the whole ship");
    pick_level(&mut maps, Some(1));
    assert_eq!(shown_level(&maps), Some(1));
    assert!(maps.camera.fly.as_ref().is_some_and(|f| f.to == MapAnchor::You), "picking from the opening view flies to you");
    maps.camera.tick(10.0);
    assert!(maps.camera.follows_me() && maps.camera.anchor == MapAnchor::You);
    let length = maps.me.as_ref().unwrap().ship_length_m();
    maps.camera.set_distance(length * 8.0);
    assert!(!picker_shown(&maps));
    assert_eq!(shown_level(&maps), None, "8 ship lengths out, Storey 2 is still shown cut open");
    maps.camera.set_distance(INSIDE_VIEW_M);
    pick_level(&mut maps, None);
    maps.me.as_mut().unwrap().level = Some(0);
    set_plan(&mut maps, true);
    assert_eq!(shown_level(&maps), Some(0), "the plan with nothing picked shows your level");
    assert!(plan_shown(&maps));
}

/// The plan asks for the tiles that cover its view, at the zoom its pixels need (a tile pixel no
/// bigger than a screen pixel), only over the ship, the one under the centre first, and never more
/// than `MAX_TILES_WANTED`.
///
/// Seen red 2026-10-22 with the floor hits taken in the world's axes, not the ship's: "the plan
/// 90 m over you wants no tile (its view missed the floor)".
#[test]
fn the_plan_wants_the_tiles_under_its_view() {
    let mut maps = opened();
    maps.me.as_mut().unwrap().level = Some(0);
    pick_level(&mut maps, Some(0));
    set_plan(&mut maps, true);
    maps.camera.tick(10.0);
    maps.camera.set_distance(INSIDE_VIEW_M);
    let me = maps.me.clone().unwrap();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0));
    let proj = MapProjector::new(maps.camera.pose(&maps.scene()), rect);
    let level = ShipLevel { ordinal: 0, floor: 0.0, top: 3.0 };
    let keys = tiles_wanted(&proj, &me, &level);
    assert!(!keys.is_empty(), "the plan 90 m over you wants no tile (its view missed the floor)");
    assert!(keys.len() <= MAX_TILES_WANTED);
    let z = keys[0].z;
    assert!(keys.iter().all(|k| k.z == z && k.level == 0), "one zoom, one level");
    let hits = floor_hits(&proj, &me, 0.0).unwrap();
    let m_per_px = (hits[1] - hits[0]).length() / 1000.0;
    assert!(TileKey::size_m(z) as f64 / 512.0 <= m_per_px * 1.0001 || z == crate::gui::pages::maps::tile_grid::MAX_ZOOM, "zoom {z} is coarser than the view's {m_per_px:.3} m pixels");
    let under = DVec3::new(me.you_ship_m.x, 0.0, me.you_ship_m.z);
    let first = keys[0].rect();
    assert!(under.x as f32 >= first[0] - 1.0 && under.x as f32 <= first[2] + 1.0 && under.z as f32 >= first[1] - 1.0 && under.z as f32 <= first[3] + 1.0, "the first tile is not the one under you: {first:?}");
}

/// The web Maps page's copy of the ship's plan (Maps increment 3, the web mirror): the levels,
/// every body standing in the ship with its storeys and walls (a neighbour's home by its design,
/// drawn on its plot), and the rooms on each level with the HUD's names and their room types'
/// colours, all from the app's own rules (ship/levels.rs, `detect_rooms`, `RoomTypeRegistry`). The
/// web cannot read RON (the deploy copies only JSON), so the app writes it.
fn web_ship_plan() -> serde_json::Value {
    let (ship, rooms) = shipped();
    let r2 = |v: f32| ((v as f64) * 100.0).round() / 100.0;
    let levels = levels::ship_levels(&ship);
    let body_json = |o: glam::Vec3, b: &crate::ship::home_structure::HomeStructure| {
        let storeys: Vec<[f64; 2]> = b.storeys().iter().map(|s| [r2(s.floor), r2(s.top)]).collect();
        let walls: Vec<[f64; 6]> = b
            .walls
            .iter()
            .map(|w| {
                let (base, top) = w.y_range();
                [r2(w.a.0), r2(w.a.1), r2(w.b.0), r2(w.b.1), r2(base), r2(top)]
            })
            .collect();
        serde_json::json!({ "origin": [r2(o.x), r2(o.y), r2(o.z)], "size": [r2(b.width), r2(b.height), r2(b.depth)], "storeys": storeys, "walls": walls })
    };
    let mut bodies = Vec::new();
    for (zi, z) in ship.zones.iter().enumerate().filter(|(zi, _)| !ship.is_away_home(*zi)) {
        let mut j = body_json(z.origin_vec(), &z.body);
        j["id"] = serde_json::json!(z.id);
        j["label"] = serde_json::json!(if zi == ship.home_zone_index() { "Your home".to_string() } else { z.label.clone() });
        bodies.push(j);
    }
    let mut designs = serde_json::Map::new();
    let view = crate::ship::neighbours::neighbour_view(&ship);
    for n in &view.neighbours {
        let o = glam::Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2);
        designs.entry(n.design.kind.clone()).or_insert_with(|| {
            let mut d = body_json(glam::Vec3::ZERO, &n.design.body);
            d.as_object_mut().unwrap().remove("origin");
            d
        });
        bodies.push(serde_json::json!({ "id": format!("plot:{}", n.plot.id), "label": format!("A neighbour's home ({})", n.plot.id), "origin": [r2(o.x), r2(o.y), r2(o.z)], "design": n.design.kind }));
    }
    let room_list: Vec<serde_json::Value> = rooms
        .iter()
        .filter_map(|r| {
            let level = levels[levels::level_at(&levels, r.min.y)?].ordinal;
            Some(serde_json::json!({
                "id": r.id, "name": r.display_name, "level": level,
                "min": [r2(r.min.x), r2(r.min.z)], "max": [r2(r.max.x), r2(r.max.z)],
                "fill": r.fill.map(|c| [r2(c[0]), r2(c[1]), r2(c[2])]),
            }))
        })
        .collect();
    serde_json::json!({
        "_generated": "By the app (src/gui/pages/maps/ship_view_tests.rs, `the_web_ship_plan_is_current`) from data/blueprints/ship_structure.ron, data/homes/ and data/rooms.ron. Do not edit by hand: run HUMANITY_WRITE_SHIP_PLAN=1 node scripts/lib-tests.js the_web_ship_plan_is_current to write it again after a change to the ship.",
        "ship": ship.id,
        "name": crate::station::home(&crate::station::load_all(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data"))).name,
        "home_plot": ship.home_plot().map(|p| p.id.clone()),
        "levels": levels.iter().map(|l| serde_json::json!({ "ordinal": l.ordinal, "name": l.name(), "short": l.short_name(), "floor": r2(l.floor), "top": r2(l.top) })).collect::<Vec<_>>(),
        "bodies": bodies,
        "designs": designs,
        "rooms": room_list,
    })
}

/// THE WEB'S PLAN IS THE APP'S: data/maps/ship_plan.json is what the app makes of the shipped ship
/// today (`web_ship_plan`). Fails when the ship, the homes or the rooms changed and the file was
/// not written again (`HUMANITY_WRITE_SHIP_PLAN=1` writes it).
///
/// Seen red 2026-10-22 before the file existed: "data/maps/ship_plan.json is not the app's plan
/// of the shipped ship (missing); write it again with HUMANITY_WRITE_SHIP_PLAN=1".
#[test]
fn the_web_ship_plan_is_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/maps/ship_plan.json");
    let want = serde_json::to_string(&web_ship_plan()).expect("the plan serialises") + "\n";
    if std::env::var("HUMANITY_WRITE_SHIP_PLAN").as_deref() == Ok("1") {
        std::fs::write(&path, &want).expect("write data/maps/ship_plan.json");
    }
    let have = std::fs::read_to_string(&path).map(|t| t.replace("\r\n", "\n"));
    let why = match &have {
        Ok(t) if *t == want => return,
        Ok(_) => "stale",
        Err(_) => "missing",
    };
    panic!("data/maps/ship_plan.json is not the app's plan of the shipped ship ({why}); write it again with HUMANITY_WRITE_SHIP_PLAN=1");
}
