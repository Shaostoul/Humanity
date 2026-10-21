//! Tests for the B editor's storeys (engine/editor_storeys.rs): ship homes increment 7, lane C2.
//! Each was seen red first, on the editor's behaviour before storeys (the code each function was
//! taken out of: `try_place_wall_node`, `cursor_floor_hit`, `try_pick_wall`, `apply_node_drag`,
//! `try_place_held_light`), or on a stub where nothing existed yet; the red line is in each doc.

use super::*;
use crate::ship::home_structure::{FloorSlab, HomeStructure, SlabMaterial, Storey};
use glam::Vec3;

fn body(text: &str) -> HomeStructure {
    ron::from_str(text).expect("the test body parses")
}

/// A 10 x 10 x 6 m body with no slabs: one storey, the ground to the roof.
fn tall_box() -> HomeStructure {
    body("(width: 10.0, depth: 10.0, height: 6.0)")
}

/// The same box with a solid slab over its north half (z 0..5) at 3.0: two storeys.
fn two_storeys() -> HomeStructure {
    body("(width: 10.0, depth: 10.0, height: 6.0, floors: [(a: (0.0, 0.0), b: (10.0, 5.0), top: 3.0)])")
}

/// A WALL DRAWN ON STOREY 2 STANDS ON ITS FLOOR: base 3.0, as tall as the storey (3.0 m to the
/// roof), where the wall tool used to stand every wall on the ground, as tall as the body.
///
/// Seen red 2026-10-20 on the wall tool's code before storeys: "a wall drawn on storey 2 stands on
/// its floor / left: (0.0, 6.0) / right: (3.0, 3.0)".
#[test]
fn a_wall_drawn_on_storey_2_stands_on_its_floor() {
    let b = two_storeys();
    let up = new_wall_on_storey(&b, EditStorey::of(&b, 1), (1.0, 1.0), (4.0, 1.0));
    assert_eq!((up.base, up.height), (3.0, 3.0), "a wall drawn on storey 2 stands on its floor");
    let ground = new_wall_on_storey(&b, EditStorey::of(&b, 0), (1.0, 1.0), (4.0, 1.0));
    assert_eq!((ground.base, ground.height), (0.0, 3.0), "a wall drawn on the ground storey reaches the slab over it");
    let one = tall_box();
    let w = new_wall_on_storey(&one, EditStorey::of(&one, 0), (1.0, 1.0), (4.0, 1.0));
    assert_eq!((w.base, w.height), (0.0, 6.0), "with one storey a wall is as tall as the body, as before");
}

/// THE FLOOR PICK ON STOREY 1 TAKES THE GROUND FLOOR UNDER A SLAB. Looking straight down through
/// the upper floor at the ground storey, the pick must land on the ground (y 0), not on the room
/// upstairs that the ray meets first: that is what lets the ground floor be edited under the
/// upper one (the cut-away hides the slab; the pick has to skip it too).
///
/// Seen red 2026-10-20 on `cursor_floor_hit`'s code before storeys (the nearest room floor of any
/// height): "storey 1's pick lands on the ground room / left: Some(StoreyFloorHit { room:
/// Some(0), x: 2.0, y: 3.0, z: 2.0 })".
#[test]
fn the_floor_pick_on_storey_1_takes_the_ground_floor_under_a_slab() {
    let upstairs = (Vec3::new(0.0, 3.0, 0.0), Vec3::new(10.0, 6.0, 5.0));
    let ground = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 3.0, 10.0));
    let plan = StoreyPlan { floor_y: 0.0, min: (0.0, 0.0), max: (10.0, 10.0), plane: false };
    let hit = storey_floor_hit(Vec3::new(2.0, 20.0, 2.0), Vec3::NEG_Y, [upstairs, ground], Some(plan));
    assert_eq!(
        hit,
        Some(StoreyFloorHit { room: Some(1), x: 2.0, y: 0.0, z: 2.0 }),
        "storey 1's pick lands on the ground room"
    );
    // With no storey to keep to (a one-storey zone), the nearest room floor, as before storeys.
    let any = storey_floor_hit(Vec3::new(2.0, 20.0, 2.0), Vec3::NEG_Y, [upstairs, ground], None);
    assert_eq!(any.map(|h| (h.room, h.y)), Some((Some(0), 3.0)));
}

/// THE FLOOR PICK UPSTAIRS LANDS ON A NEW SLAB THAT HAS NO ROOM YET. A room above the ground
/// exists only where a zone of its storey says so (`detect_rooms`), so a slab just drawn has
/// none; the pick still has to land on its plane, or no wall could ever be drawn on a new floor.
/// Outside the body's footprint it lands nowhere.
///
/// Seen red 2026-10-20 on `cursor_floor_hit`'s code before storeys: "storey 2's plan is picked at
/// its floor / left: Some(StoreyFloorHit { room: Some(0), x: 2.0, y: 0.0, z: 2.0 })".
#[test]
fn the_floor_pick_upstairs_lands_on_a_slab_with_no_room_yet() {
    let ground = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 3.0, 10.0));
    let plan = StoreyPlan { floor_y: 3.0, min: (0.0, 0.0), max: (10.0, 10.0), plane: true };
    let hit = storey_floor_hit(Vec3::new(2.0, 20.0, 2.0), Vec3::NEG_Y, [ground], Some(plan));
    assert_eq!(hit, Some(StoreyFloorHit { room: None, x: 2.0, y: 3.0, z: 2.0 }), "storey 2's plan is picked at its floor");
    let outside = storey_floor_hit(Vec3::new(12.0, 20.0, 2.0), Vec3::NEG_Y, [ground], Some(plan));
    assert_eq!(outside, None, "outside the body there is no floor to pick");
}

/// ADDING A SLAB MAKES A SECOND STOREY. Drawn as "a floor above" on the one storey of a 6 m box,
/// the slab goes 3.0 m up (plan decision 2) and the box has two storeys; a second slab drawn the
/// same way joins that floor rather than making a third; "more of this floor" upstairs goes at
/// its floor; and where there is no room for a storey, or on the ground (whose floor is the
/// home's own), nothing is added and the reason says why.
///
/// Seen red 2026-10-20 on a stub (no slab could be drawn before): "a floor above on a 6 m box /
/// left: Err(\"floor slabs cannot be drawn yet\")".
#[test]
fn adding_a_slab_makes_a_second_storey() {
    let mut b = tall_box();
    let es = EditStorey::of(&b, 0);
    assert_eq!(add_floor_slab(&mut b, es, SlabDraw::Above, (0.0, 0.0), (10.0, 5.0)), Ok(0), "a floor above on a 6 m box");
    assert_eq!(b.storeys(), vec![Storey { floor: 0.0, top: 3.0 }, Storey { floor: 3.0, top: 6.0 }], "the slab makes a second storey");
    assert_eq!((b.floors[0].top, b.floors[0].thickness, b.floors[0].material), (3.0, 0.25, SlabMaterial::Solid));
    // Drawn corners either way round become the footprint, inside the body.
    let es = EditStorey::of(&b, 0);
    assert_eq!(add_floor_slab(&mut b, es, SlabDraw::Above, (10.0, 9.0), (6.0, 6.0)), Ok(1));
    assert_eq!(b.storeys().len(), 2, "a second floor above the ground joins storey 2's floor");
    assert_eq!((b.floors[1].a, b.floors[1].b, b.floors[1].top), ((6.0, 6.0), (10.0, 9.0), 3.0));
    let es = EditStorey::of(&b, 1);
    assert_eq!(add_floor_slab(&mut b, es, SlabDraw::Here, (0.0, 5.0), (6.0, 6.0)), Ok(2));
    assert_eq!(b.floors[2].top, 3.0, "more of storey 2's floor goes at its floor");
    // The top storey of a 6 m box has no room for another 3 m storey; the ground's floor is the
    // home's own; a slip of a click (under 0.25 m a side) is not a floor.
    let why = add_floor_slab(&mut b, es, SlabDraw::Above, (0.0, 0.0), (4.0, 4.0)).unwrap_err();
    assert!(why.contains("6.0 m") && why.contains("plot"), "{why}");
    let ground = EditStorey::of(&b, 0);
    let why = add_floor_slab(&mut b, ground, SlabDraw::Here, (0.0, 0.0), (4.0, 4.0)).unwrap_err();
    assert!(why.contains("ground"), "{why}");
    assert!(add_floor_slab(&mut b, es, SlabDraw::Here, (1.0, 1.0), (1.1, 4.0)).is_err(), "a sliver is refused");
    assert_eq!(b.floors.len(), 3, "a refused floor adds nothing");
    let mut low = body("(width: 10.0, depth: 10.0, height: 3.0)");
    let es = EditStorey::of(&low, 0);
    let why = add_floor_slab(&mut low, es, SlabDraw::Above, (0.0, 0.0), (4.0, 4.0)).unwrap_err();
    assert!(why.contains("3.0 m"), "a 3 m box has room for one storey: {why}");
}

/// REMOVING THE SLAB REMOVES ITS STOREY: the one slab of storey 2 taken out, the box is one
/// storey again; a slab that is not there removes nothing.
///
/// Seen red 2026-10-20 on a stub (no slab could be removed before): "the slab comes out / left:
/// None".
#[test]
fn removing_the_slab_removes_its_storey() {
    let mut b = two_storeys();
    assert_eq!(b.storeys().len(), 2);
    let out = remove_floor_slab(&mut b, 0);
    assert_eq!(out.map(|s| s.top), Some(3.0), "the slab comes out");
    assert_eq!(b.storeys(), vec![Storey { floor: 0.0, top: 6.0 }], "and its storey with it");
    assert_eq!(remove_floor_slab(&mut b, 0), None, "a slab that is not there removes nothing");
}

/// TRY_PICK_WALL MISSES AN UPSTAIRS WALL WHEN THE RAY PASSES UNDER IT: a wall from 3.0 to 6.0 is
/// hit only between its base and its top. Its zone sits away from the origin, so the test also
/// holds the wall in its zone's place.
///
/// Seen red 2026-10-20 on `try_pick_wall`'s code before storeys (a wall from the deck to its
/// height): "a ray at 1.5 m passes under the upstairs wall / left: Some(5.0)".
#[test]
fn try_pick_wall_misses_an_upstairs_wall_when_the_ray_passes_under_it() {
    let up: InteriorWall = ron::from_str("(a: (5.0, 0.0), b: (5.0, 5.0), base: 3.0, height: 3.0)").unwrap();
    let ground: InteriorWall = ron::from_str("(a: (5.0, 0.0), b: (5.0, 5.0), height: 3.0)").unwrap();
    let zo = Vec3::new(100.0, 0.0, 200.0);
    let at = |y: f32| Vec3::new(100.0, y, 202.5);
    assert_eq!(ray_hits_wall(&up, zo, at(1.5), Vec3::X, f32::INFINITY), None, "a ray at 1.5 m passes under the upstairs wall");
    assert_eq!(ray_hits_wall(&up, zo, at(4.5), Vec3::X, f32::INFINITY), Some(5.0), "a ray at 4.5 m hits it");
    assert_eq!(ray_hits_wall(&ground, zo, at(1.5), Vec3::X, f32::INFINITY), Some(5.0), "the ground wall below it is hit at 1.5 m");
    assert_eq!(ray_hits_wall(&ground, zo, at(4.5), Vec3::X, f32::INFINITY), None, "and missed above its top");
}

/// DRAGGING AN UPSTAIRS CORNER LEAVES THE GROUND WALL UNDER IT. Corners are shared by position,
/// so two walls meeting at one (x, z) move together; with storeys that must hold only on one
/// storey, or moving an upstairs partition would drag the wall below it along.
///
/// Seen red 2026-10-20 on `apply_node_drag`'s loop before storeys (every wall end at the corner):
/// "the ground wall under it stays / left: (6.0, 6.0) / right: (5.0, 5.0)".
#[test]
fn dragging_an_upstairs_corner_leaves_the_ground_wall_under_it() {
    let mut b = body(
        "(width: 10.0, depth: 10.0, height: 6.0, \
         walls: [(a: (0.0, 5.0), b: (5.0, 5.0), height: 3.0), (a: (5.0, 0.0), b: (5.0, 5.0), base: 3.0, height: 3.0), \
                 (a: (5.0, 5.0), b: (9.0, 5.0), base: 3.0, height: 3.0)], \
         floors: [(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)])",
    );
    assert!(move_storey_corner(&mut b, 1, (5.0, 5.0), (6.0, 6.0)), "a corner on storey 2 moved");
    assert_eq!(b.walls[0].b, (5.0, 5.0), "the ground wall under it stays");
    assert_eq!((b.walls[1].b, b.walls[2].a), ((6.0, 6.0), (6.0, 6.0)), "the two upstairs walls sharing it move together");
}

/// A CEILING LIGHT HANGS UNDER THE SLAB ABOVE IT: placed on the ground storey under the upper
/// floor, a ceiling panel goes 0.3 m under the slab's underside (2.75), not 0.3 m under the roof
/// three metres higher; upstairs it hangs under the roof; a warm lamp sits 0.8 m up its storey.
///
/// Seen red 2026-10-20 on `try_place_held_light`'s heights before storeys: "a panel under the
/// upper floor / left: 5.7 / right: 2.45".
#[test]
fn a_ceiling_light_hangs_under_the_slab_above_it() {
    let b = two_storeys();
    let near = |a: f32, e: f32| (a - e).abs() < 1e-4;
    let under = light_height_on_storey(&b, EditStorey::of(&b, 0), 2.0, 2.0, "ceiling_panel");
    assert!(near(under, 2.45), "a panel under the upper floor / left: {under} / right: 2.45");
    let open = light_height_on_storey(&b, EditStorey::of(&b, 0), 2.0, 8.0, "ceiling_panel");
    assert!(near(open, 5.7), "where no slab is over it, under the roof: {open}");
    let up = light_height_on_storey(&b, EditStorey::of(&b, 1), 2.0, 2.0, "ceiling_panel");
    assert!(near(up, 5.7), "upstairs, under the roof: {up}");
    let lamp = light_height_on_storey(&b, EditStorey::of(&b, 1), 2.0, 2.0, "warm_lamp");
    assert!(near(lamp, 3.8), "a lamp 0.8 m up storey 2: {lamp}");
}

/// THE CUT-AWAY HIDES THE STOREYS ABOVE. Editing the ground storey of a two-storey body draws a
/// copy with the upper floor, the walls standing on it, the pieces, lights and zones up there
/// taken out, and a wall that rises through the slab cut at it (its opening up there with it);
/// the shell stays its full height, so there is no lid at the cut. The top storey has nothing
/// above it: no cut.
///
/// Seen red 2026-10-20 on a stub that drew the whole body: "the upper floor is hidden / left: 1".
#[test]
fn the_cut_away_hides_the_storeys_above() {
    let b = body(
        "(width: 10.0, depth: 10.0, height: 6.0, \
         walls: [(a: (0.0, 5.0), b: (10.0, 5.0), height: 6.0, openings: [ \
                    (kind: Door, at: 1.0, width: 1.0, sill: 0.0, height: 2.1), \
                    (kind: Window, at: 4.0, width: 1.0, sill: 4.0, height: 1.0)]), \
                 (a: (5.0, 0.0), b: (5.0, 5.0), base: 3.0, height: 3.0)], \
         floors: [(a: (0.0, 0.0), b: (10.0, 5.0), top: 3.0)], \
         structures: [(type_id: \"stairs\", pos: (1.0, 0.0, 7.0)), (type_id: \"stairs\", pos: (1.0, 3.0, 2.0))], \
         lights: [(type_id: \"ceiling_panel\", pos: (2.0, 2.45, 2.0)), (type_id: \"ceiling_panel\", pos: (2.0, 5.7, 2.0))], \
         zones: [(id: \"down\", type_id: \"residential\", origin: (0.0, 0.0, 0.0), size: (10.0, 3.0, 5.0)), \
                 (id: \"up\", type_id: \"residential\", origin: (0.0, 3.0, 0.0), size: (10.0, 3.0, 5.0))])",
    );
    let cut = cut_away_body(&b, 0).expect("the ground storey has storeys above it");
    assert_eq!(cut.floors.len(), 0, "the upper floor is hidden");
    assert_eq!(cut.walls.len(), 1, "the wall standing on it is hidden");
    assert_eq!(cut.walls[0].height, 3.0, "the wall rising through the slab is cut at it");
    assert_eq!(cut.walls[0].openings.len(), 1, "its window above the cut goes with it, its door stays");
    assert_eq!((cut.structures.len(), cut.structures[0].pos.1), (1, 0.0), "the piece upstairs is hidden");
    assert_eq!((cut.lights.len(), cut.lights[0].pos.1), (1, 2.45), "the light upstairs is hidden");
    assert_eq!(cut.zones.iter().map(|z| z.id.as_str()).collect::<Vec<_>>(), vec!["down"], "the zone upstairs is hidden");
    assert_eq!(cut.height, 6.0, "the shell keeps its height: no lid at the cut");
    assert!(cut_away_body(&b, 1).is_none(), "nothing is above the top storey");
}

/// THE PALETTE'S GUARD DRAWS A GUARD WALL ON THE CHOSEN STOREY. A guard is a wall in a
/// balustrade material (lane G: `InteriorWall::guard`, wall materials whose form is the `guard`
/// structure type), so the palette's Guard entry draws one with the wall tool rather than placing
/// a loose section nothing collides with: on storey 2 it stands on the floor at 3.0, as tall as
/// the guard type (1.0 m), in the first balustrade material (steel), and draws as a guard.
///
/// Seen red 2026-10-20 on a stub (the palette's Guard placed a structure piece, no wall):
/// "the guard tool draws a wall / left: None".
#[test]
fn the_palette_guard_draws_a_guard_wall_on_the_chosen_storey() {
    let b = two_storeys();
    let g = new_guard_on_storey(EditStorey::of(&b, 1), (1.0, 1.0), (4.0, 1.0));
    assert!(g.is_some(), "the guard tool draws a wall / left: {g:?}");
    let g = g.unwrap();
    assert_eq!((g.base, g.height), (3.0, 1.0), "on storey 2's floor, as tall as the guard type");
    let mat = crate::ship::home_structure::wall_material(g.material).map(|m| m.name.as_str());
    assert_eq!(mat, Some("Steel balustrade"));
    assert!(g.guard().is_some(), "it draws as a guard (posts, balusters and a rail)");
}

/// THE EDITOR OPENS ON YOUR STOREY WITH THE AVATAR AT YOUR FEET (the review of 2026-10-21, finding
/// 2). The build-mode avatar is seeded where the player stands, on the storey their feet are on,
/// and the storey selector opens there: upstairs over the slab of the two-storey box (eye 4.7 m),
/// storey index 1; on the ground under the slab, 0; from outside the body, its nearest spot 0.3 m
/// inside, on the storey of the height the feet are at.
///
/// Seen red first (2026-10-21) with the seed on the ground storey, where the selector opened before
/// this fix (its default, 0) whatever storey the player was on: "upstairs / left: ((2.0, 2.0), 0) /
/// right: ((2.0, 2.0), 1)".
#[test]
fn the_editor_opens_on_your_storey_with_the_avatar_at_your_feet() {
    let b = two_storeys();
    let eye = 1.7;
    let o = Vec3::new(100.0, 0.0, 50.0);
    assert_eq!(avatar_seed(&b, o, o + Vec3::new(2.0, 3.0 + eye, 2.0), eye), ((2.0, 2.0), 1), "upstairs");
    assert_eq!(avatar_seed(&b, o, o + Vec3::new(2.0, eye, 2.0), eye), ((2.0, 2.0), 0), "on the ground under the slab");
    assert_eq!(avatar_seed(&b, o, o + Vec3::new(-4.0, eye, 20.0), eye), ((0.3, 9.7), 0), "from outside the body");
    assert_eq!(avatar_seed(&tall_box(), o, o + Vec3::new(5.0, 3.0 + eye, 5.0), eye), ((5.0, 5.0), 0), "one storey: always the ground's");
}

/// THE EDITOR'S SAVE WRITES THE HOME'S SPAWN ONLY FROM AN AVATAR MOVED ONTO THE GROUND FLOOR. The
/// avatar now opens at the player's feet (`seed_build_avatar`), so a Save without moving it must
/// leave the spawn (where the player enters the world) as it was, and an avatar moved upstairs is
/// no ground spot for it. Moved on the ground storey, it is the new spawn, as v0.582 made it.
///
/// Seen red first (2026-10-21) with the Save writing the avatar's spot whatever it was (the code
/// before this fix): "not moved since it was put at the player's feet / left: Some((2.0, 2.0)) /
/// right: None".
#[test]
fn the_editors_save_writes_the_spawn_only_from_an_avatar_moved_onto_the_ground() {
    let zone: crate::ship::ship_structure::ShipZone = ron::from_str(
        "(id: \"home\", purpose: \"residence\", body: (width: 10.0, depth: 10.0, height: 6.0, \
         floors: [(a: (0.0, 0.0), b: (10.0, 5.0), top: 3.0)]))",
    )
    .expect("the test zone parses");
    let mut g = crate::gui::GuiState::default();
    g.ship_structure = Some(crate::ship::ship_structure::ShipStructure { zones: vec![zone], ..Default::default() });
    g.construction_zone = 0;
    g.build_char_pos = Some((2.0, 2.0));
    g.construction_storeys.chosen = 0;
    g.construction_storeys.avatar_seed = Some((0, (2.0, 2.0), 0));
    assert_eq!(spawn_from_avatar(&g), None, "not moved since it was put at the player's feet");
    g.build_char_pos = Some((6.0, 8.0));
    assert_eq!(spawn_from_avatar(&g), Some((6.0, 8.0)), "moved on the ground floor");
    g.construction_storeys.chosen = 1;
    assert_eq!(spawn_from_avatar(&g), None, "moved upstairs: no ground spot");
    g.construction_storeys.avatar_seed = Some((0, (6.0, 8.0), 1));
    g.construction_storeys.chosen = 0;
    assert_eq!(spawn_from_avatar(&g), Some((6.0, 8.0)), "seeded upstairs, then taken down to the ground floor");
}

/// THE EDITOR'S SAVE NEVER PUTS THE SPAWN INSIDE A STAIR (the seams review of 2026-10-21, finding
/// 1). The spawn has no height: world entry stands the player on the ground floor at it, and the
/// footing steps a body up no more than 0.6 m from there, so a spawn over a stair's flight where
/// its steps are higher than that is a spawn inside the flight, and every world entry after the
/// Save would start there, pushed out sideways through a wall. With the avatar dragged onto step 8
/// of the shipped homestead's stairs-1 (1.5 m up) and Saved, the spawn written is off the flight,
/// clear of it by a body's radius, and a place to stand on the ground storey; an avatar moved onto
/// the first steps (no higher than a step up) is written where it is.
///
/// Seen red first (2026-10-21) with the Save writing the avatar's spot as it was: "the spawn
/// written from step 8 of stairs-1 is (38.89, 38.65), on the flight".
#[test]
fn the_editors_save_never_puts_the_spawn_inside_a_stair() {
    use crate::ship::ship_structure::ShipStructure;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let ship = ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles");
    let hz = ship.home_zone_index();
    let mut g = crate::gui::GuiState::default();
    g.ship_structure = Some(ship);
    g.construction_zone = hz;
    g.construction_storeys.chosen = 0;
    g.construction_storeys.avatar_seed = Some((hz, (53.5, 40.5), 0));
    // Where the steps more than a step up stand (x 36.5 to 40.16, z 38.05 to 39.25), and a body's
    // radius round it.
    let r = crate::ship::wall_collision::PLAYER_RADIUS;
    let on_flight = |p: (f32, f32)| p.0 > 36.5 - r && p.0 < 40.16 + r && p.1 > 38.05 - r && p.1 < 39.25 + r;
    g.build_char_pos = Some((38.89, 38.65));
    let spawn = spawn_from_avatar(&g).expect("an avatar moved on the ground storey writes a spawn");
    assert!(!on_flight(spawn), "the spawn written from step 8 of stairs-1 is ({:.2}, {:.2}), on the flight", spawn.0, spawn.1);
    let hs = zone_body(&g.ship_structure, hz).expect("the home's body");
    assert_eq!(crate::ship::standing::standing_on(hs, 0, spawn.0, spawn.1), Ok(()), "the spawn written from step 8, ({:.2}, {:.2})", spawn.0, spawn.1);
    assert_eq!(crate::ship::standing::tread_at(hs, 0, spawn.0, spawn.1), None, "the spawn written from step 8 is over a tread");
    g.build_char_pos = Some((40.6, 38.65));
    assert_eq!(spawn_from_avatar(&g), Some((40.6, 38.65)), "step 2 of stairs-1, 0.375 m up, is stood on from the floor");
}

/// THE CUT-AWAY LEAVES OUT WHAT STANDS OVER IT, NOT ONLY WHAT IS IN THE BODY (ship homes increment
/// 7 review, 2026-10-21). The machines, their screens (which share the machine's position), the
/// door panels and the elevator cars are drawn from the whole ship, not the cut body, so `CutAway`
/// decides them by where they stand. On the shipped homestead (the home on p1), the ground storey's
/// cut hides every machine of the upper floor (the bed, the study's desk and its screens, the
/// bathroom) and nothing else; it hides exactly the door panels the cut body has none of (the doors
/// it shows are `cut_away_ship`'s own); and it agrees with `cut_away_body` on every piece and light
/// of the body. Nothing is over the upper floor, so it has no cut.
///
/// Seen red first (2026-10-21) with `CutAway::hides` answering false, everything drawn as it was:
/// "35 of 345 machines are drawn wrong in the cut-away", the first "desk_1 at (50.6, 3.0, 34.5)
/// (upstairs, drawn)".
#[test]
fn the_cut_away_leaves_out_what_stands_over_it() {
    use std::collections::HashMap;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let ship = ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles");
    let hz = ship.home_zone_index();
    let cut = cut_away_of(&ship, hz, 0).expect("the homestead's ground storey has the upper floor over it");
    assert!(cut_away_of(&ship, hz, 1).is_none(), "nothing is over the upper floor");
    let o = ship.zones[hz].origin_vec();

    // Machines: hidden exactly when the layout puts them on the home's upper floor.
    let home = crate::machines::MachineHome::load(&data.join("machines/home.ron")).expect("the household's machines load");
    let rects = ship.zone_rects();
    let upstairs: std::collections::HashSet<String> = home
        .all_instances()
        .into_iter()
        .filter(|i| i.offset.1 >= 3.0 - STOREY_EPS)
        .filter(|i| crate::machines::resolve_zone_rect(&rects, &i.zone).is_some_and(|z| z.id == ship.zones[hz].id))
        .map(|i| i.id)
        .collect();
    assert!(upstairs.contains("bed_1") && upstairs.contains("desk_monitor_1"), "the bed and the study's monitor are upstairs");
    let placed = home.placements(&HashMap::new(), Some(&rects));
    let wrong: Vec<String> = placed
        .iter()
        .filter(|p| cut.hides(Vec3::new(p.pos.0, p.pos.1, p.pos.2)) != upstairs.contains(&p.id))
        .map(|p| format!("{} at {:?} ({})", p.id, p.pos, if upstairs.contains(&p.id) { "upstairs, drawn" } else { "not upstairs, hidden" }))
        .collect();
    assert!(wrong.is_empty(), "{} of {} machines are drawn wrong in the cut-away: {wrong:#?}", wrong.len(), placed.len());

    // Door panels: the ones shown are the cut body's own.
    let key = |p: &crate::ship::door_panels::PanelPlacement| {
        ((p.center.x * 100.0).round() as i64, (p.center.y * 100.0).round() as i64, (p.center.z * 100.0).round() as i64)
    };
    let all = crate::ship::door_panels::ship_panel_placements(&ship);
    let mut shown: Vec<_> = all.iter().filter(|p| !cut.hides(p.center)).map(key).collect();
    let cut_ship = cut_away_ship(&ship, hz, 0).expect("the cut ship");
    let mut own: Vec<_> = crate::ship::door_panels::ship_panel_placements(&cut_ship).iter().map(key).collect();
    shown.sort_unstable();
    own.sort_unstable();
    assert!(all.len() > own.len(), "the cut body has fewer doors than the whole ship ({} vs {})", own.len(), all.len());
    assert_eq!(shown, own, "the doors the cut-away shows are not the cut body's");

    // The body's pieces and lights: the same rule as `cut_away_body`.
    let body = &ship.zones[hz].body;
    let cut_body = cut_away_body(body, 0).expect("the cut body");
    let shown_pieces = body.structures.iter().filter(|s| !cut.hides(o + Vec3::new(s.pos.0, s.pos.1, s.pos.2))).count();
    assert_eq!(shown_pieces, cut_body.structures.len(), "pieces: the cut-away and `cut_away_body` disagree");
    let shown_lights = body.lights.iter().filter(|l| !cut.hides(o + Vec3::new(l.pos.0, l.pos.1, l.pos.2))).count();
    assert_eq!(shown_lights, cut_body.lights.len(), "lights: the cut-away and `cut_away_body` disagree");
}
