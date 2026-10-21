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
