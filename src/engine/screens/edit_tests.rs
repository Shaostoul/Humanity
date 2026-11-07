//! Build mode's screen handles, without a GPU: the handle under a point, and a drag carried all the
//! way through the home's data and `MachineHome::placements` back to the screen's quad and surface,
//! the same path the editor's per-frame drag takes (`apply_drag` is this test's `drag` +
//! `write_drag` with the cursor's ray).

use super::*;
use crate::engine::screens::quad_from_placement;
use crate::gui::screen_tuning::SizeTuning;
use humanity_ship::machines::{MachineHome, PlacedMachine, ZoneRect};

/// A home with one wall screen, its picture 1.16 x 0.66 m on a 1.2 x 0.7 m body, turned 90 degrees
/// (it faces -X), and one camera post.
fn home() -> MachineHome {
    ron::from_str(
        r#"(
            catalog: {
                "wall_screen": (shape: "box", size: (1.2, 0.7, 0.05), color: (0.1, 0.1, 0.1),
                    screen: Some((source: "inventory", px: (1280, 720), face: "front", bezel_m: 0.02, brightness: 1.0))),
                "camera_post": (shape: "box", size: (0.14, 1.7, 0.14), color: (0.2, 0.2, 0.2), camera: Some((0.0, -8.0, 70.0))),
            },
            instances: [
                (id: "tv", machine: "wall_screen", room: "g", offset: (5.0, 1.2, 5.0), rotation: 90.0),
                (id: "camera_post_1", machine: "camera_post", room: "g", offset: (8.0, 0.0, 8.0)),
            ],
            connections: [],
        )"#,
    )
    .expect("the test home parses")
}

fn placed(home: &MachineHome) -> PlacedMachine {
    let zones = vec![ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (20.0, 20.0, 4.0) }];
    home.placements(&std::collections::HashMap::new(), Some(&zones)).into_iter().find(|p| p.id == "tv").expect("the tv is placed")
}

/// The picture's quad as the screens build it from a placement.
fn quad_of(p: &PlacedMachine) -> QuadGeom {
    let sd = p.screen.as_ref().unwrap();
    quad_from_placement(Vec3::new(p.pos.0, p.pos.1, p.pos.2), p.size, p.rotation, &sd.face, sd.bezel_m)
}

/// A grab of `handle` on the tv as it is placed now, the cursor starting at picture point (x, y)
/// metres from its top-left corner.
fn grab(home: &MachineHome, handle: Handle, x: f32, y: f32) -> ScreenGrab {
    let frame = Frame::of(&quad_of(&placed(home)));
    let inst = home.instances.iter().find(|i| i.id == "tv").unwrap();
    ScreenGrab {
        id: "tv".into(),
        handle,
        frame,
        start_hit: Vec2::new(x, y),
        start_offset: inst.offset,
        def_size: (1.2, 0.7, 0.05),
        face: "front".into(),
        bezel: 0.02,
    }
}

/// The cursor's ray from 3 m in front of the picture through picture point (x, y): what the drag is
/// handed, proven to land where it was aimed.
fn hit_at(f: &Frame, x: f32, y: f32) -> Vec2 {
    let target = f.origin + f.right * x + f.down * y;
    let eye = target + f.normal * 3.0 + f.right * 0.4;
    f.hit(eye, (target - eye).normalize()).expect("the ray meets the plane")
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0e-4
}

/// Each part of the picture is the handle it is drawn as: the middle moves, a band along each edge
/// is that edge, both bands at a corner make the corner, and the cog sits two bands in from the
/// top-right corner, clear of both edges.
///
/// Seen red 2026-11-06 with the cog's test skipped in `handle_at`: the cog's centre read `Move`.
#[test]
fn each_part_of_the_picture_is_its_handle() {
    let s = SizeTuning::default();
    let (w, h) = (2.0, 1.125);
    assert_eq!(handle_at(0.5, 0.5, w, h, &s), Handle::Move);
    assert_eq!(handle_at(0.01, 0.5, w, h, &s), Handle::Edges { x: -1, y: 0 });
    assert_eq!(handle_at(0.99, 0.5, w, h, &s), Handle::Edges { x: 1, y: 0 });
    assert_eq!(handle_at(0.5, 0.01, w, h, &s), Handle::Edges { x: 0, y: -1 });
    assert_eq!(handle_at(0.5, 0.99, w, h, &s), Handle::Edges { x: 0, y: 1 });
    assert_eq!(handle_at(0.995, 0.005, w, h, &s), Handle::Edges { x: 1, y: -1 }, "the top-right corner is still a corner");
    let (c, _) = cog(w, h, &s);
    assert_eq!(handle_at(c.x / w, c.y / h, w, h, &s), Handle::Cog);
    let b = band_m(w, h, &s);
    assert!(close(b, 0.135), "12% of the 1.125 m side: {b}");
}

/// Dragging the right edge 0.5 m to the right makes the picture 0.5 m wider, the left edge stays
/// where it was, the height does not change, and once written into the home the placement's body,
/// quad and surface pixels follow (the surface keeps 720 rows, its width follows the aspect).
/// Dragging the bottom edge down keeps the top edge: the body sits lower by the growth.
///
/// Seen red 2026-11-06 with the new centre taken as the old one (`cx = w0 * 0.5`, the picture
/// growing about its middle): "the left edge stays put" failed by 0.25 m.
#[test]
fn dragging_an_edge_changes_the_size_and_keeps_the_opposite_edge() {
    let mut home = home();
    let before = quad_of(&placed(&home));
    let (w0, h0) = (before.u_axis.length(), before.v_axis.length());
    assert!(close(w0, 1.16) && close(h0, 0.66), "{w0} x {h0}");
    let left_before = before.origin;

    let g = grab(&home, Handle::Edges { x: 1, y: 0 }, w0 - 0.02, 0.3);
    let hit = hit_at(&g.frame, w0 + 0.48, 0.3);
    let out = drag(&g, hit, true, None, &SizeTuning::default());
    assert!(close(out.display.0, w0 + 0.5) && close(out.display.1, h0), "0.5 m wider, as tall: {:?}", out.display);
    assert!(write_drag(&mut home, &g, out), "the drag changed the home");
    assert_eq!(home.instances[0].screen_size_m, Some(out.display), "the size is the placed screen's own");

    let p = placed(&home);
    let after = quad_of(&p);
    assert!(close(after.u_axis.length(), w0 + 0.5) && close(after.v_axis.length(), h0), "the quad follows");
    assert!((after.origin - left_before).length() < 1.0e-4, "the left edge stays put: moved {:?}", after.origin - left_before);
    assert!(close(p.size.0, w0 + 0.5 + 0.04), "the body is the picture plus the bezel: {:?}", p.size);
    let px = p.screen.as_ref().unwrap().px;
    assert_eq!(px, humanity_ship::machines::screen_px_for_shape((1280, 720), out.display), "the surface follows the shape");
    assert_eq!(px.1, 720, "with the def's 720 rows");

    // The bottom edge down 0.3 m: taller, the top edge where it was, the base 0.3 m lower.
    let top_before = after.origin;
    let base_before = home.instances[0].offset.1;
    let g = grab(&home, Handle::Edges { x: 0, y: 1 }, 0.5, h0 - 0.01);
    let out = drag(&g, hit_at(&g.frame, 0.5, h0 + 0.29), true, None, &SizeTuning::default());
    write_drag(&mut home, &g, out);
    let after = quad_of(&placed(&home));
    assert!(close(after.v_axis.length(), h0 + 0.3), "0.3 m taller");
    assert!((after.origin - top_before).length() < 1.0e-4, "the top edge stays put");
    assert!(close(home.instances[0].offset.1, base_before - 0.3), "the body's base went down by the growth");
}

/// A corner keeps the picture's aspect while Shift is up (the larger stretch leads) and frees it
/// with Shift; the limits hold; the grid steps the size; and the middle moves the screen along its
/// own plane, never into or out of its wall.
///
/// Seen red 2026-11-06 with the corner's aspect rule switched off: "the aspect is kept" failed
/// with (1.74, 0.76) for a 1.16 x 0.66 picture.
#[test]
fn a_corner_keeps_the_aspect_unless_shift_and_the_middle_slides_on_the_wall() {
    let home = home();
    let s = SizeTuning::default();
    let q = quad_of(&placed(&home));
    let (w0, h0) = (q.u_axis.length(), q.v_axis.length());
    let g = grab(&home, Handle::Edges { x: 1, y: 1 }, w0, h0);
    let hit = hit_at(&g.frame, w0 + 0.58, h0 + 0.1);
    let kept = drag(&g, hit, true, None, &s);
    assert!(close(kept.display.0 / kept.display.1, w0 / h0), "the aspect is kept: {:?}", kept.display);
    assert!(close(kept.display.0, w0 + 0.58), "the larger stretch leads");
    let free = drag(&g, hit, false, None, &s);
    assert!(close(free.display.0, w0 + 0.58) && close(free.display.1, h0 + 0.1), "Shift frees it: {:?}", free.display);
    let tiny = drag(&g, hit_at(&g.frame, -5.0, -5.0), true, None, &s);
    assert!(tiny.display.0 >= s.min_m.0 - 1e-5 && tiny.display.1 >= s.min_m.1 - 1e-5, "never below the smallest: {:?}", tiny.display);
    let stepped = drag(&grab(&home, Handle::Edges { x: 1, y: 0 }, w0, 0.3), hit_at(&g.frame, w0 + 0.123, 0.3), true, Some(0.05), &s);
    assert!(close(stepped.display.0, 1.30), "the grid steps the width to 5 cm: {:?}", stepped.display);

    let m = grab(&home, Handle::Move, 0.5, 0.3);
    let out = drag(&m, hit_at(&m.frame, 1.2, 0.0), true, Some(0.05), &s);
    let moved = Vec3::new(out.offset.0 - m.start_offset.0, out.offset.1 - m.start_offset.1, out.offset.2 - m.start_offset.2);
    assert!(moved.dot(m.frame.normal).abs() < 1.0e-5, "never into or out of the wall: {moved:?}");
    assert!(close(moved.dot(m.frame.right), 0.70) && close(moved.y, 0.30), "along the wall 0.7 m and up 0.3 m: {moved:?}");
    assert_eq!(out.display, (m.frame.w, m.frame.h), "a move keeps the size");
}
