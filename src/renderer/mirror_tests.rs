//! The mirror view's pure parts (renderer/mirror.rs): the reflection camera, the picture size and
//! the levels file. No GPU.

use super::*;
use glam::{Vec3, Vec4Swizzles};

/// A 1 m x 2 m glass in the plane z = 0 facing +Z (a viewer at z > 0 looks along -Z, their right is
/// +X), its top-left corner at (-0.5, 2, 0): `QuadGeom`'s convention, v x u = normal.
fn glass() -> (Vec3, Vec3, Vec3, Vec3) {
    (Vec3::new(-0.5, 2.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -2.0, 0.0), Vec3::Z)
}

const FAR: f32 = 1.0e12;

/// Where the glass shows point `p` to an eye at `eye`: the (u, v) on the glass where the line from
/// the eye to p's reflection crosses it (the law of reflection, worked out without the camera).
fn seen_at(eye: Vec3, p: Vec3) -> (f32, f32) {
    let r = Vec3::new(p.x, p.y, -p.z);
    let t = eye.z / (eye.z - r.z);
    let x = eye + (r - eye) * t;
    let (origin, _, _, _) = glass();
    (x.x - origin.x, (origin.y - x.y) / 2.0)
}

/// THE REFLECTION CAMERA PUTS EVERY POINT WHERE THE GLASS SHOWS IT. For points in front of the
/// glass, the reflection camera's picture holds each at texture (1 - u, v), where (u, v) is the
/// point on the glass the law of reflection says the eye sees it at: the mirror image, left for
/// right, filling the texture corner to corner. Seen red 2026-11-21 with the camera's up given as +v
/// (the picture upside down and off the glass): "eye Vec3(0.3, 1.6, 2.0), point 0 lands at texture
/// (-0.200, 0.086), the glass shows it at (0.400, 0.314), so the texture should hold it at (0.600,
/// 0.314)".
#[test]
fn the_reflection_camera_puts_every_point_where_the_glass_shows_it() {
    let (o, u, v, n) = glass();
    for eye in [Vec3::new(0.3, 1.6, 2.0), Vec3::new(-0.8, 1.2, 0.9), Vec3::new(0.0, 1.0, 4.0)] {
        let view = reflected_view(o, u, v, n, eye, FAR).expect("an eye in front of the glass reflects");
        let vp = view.proj * view.view;
        let mut checked = 0;
        for (i, p) in [Vec3::new(-0.4, 1.2, 1.5), Vec3::new(0.6, 0.5, 0.8), Vec3::new(0.1, 1.9, 3.0), Vec3::new(-0.2, 0.3, 0.4)].into_iter().enumerate() {
            let (gu, gv) = seen_at(eye, p);
            if !(0.0..=1.0).contains(&gu) || !(0.0..=1.0).contains(&gv) {
                continue; // not in this mirror from this eye
            }
            let c = vp * p.extend(1.0);
            assert!(c.w > 0.0, "point {i} is in front of the reflection camera");
            let ndc = c.xyz() / c.w;
            let tex = ((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);
            assert!(
                (tex.0 - (1.0 - gu)).abs() < 1.0e-4 && (tex.1 - gv).abs() < 1.0e-4,
                "eye {eye:?}, point {i} lands at texture ({:.3}, {:.3}), the glass shows it at ({:.3}, {:.3}), so the texture should hold it at ({:.3}, {:.3})",
                tex.0, tex.1, gu, gv, 1.0 - gu, gv
            );
            assert!(ndc.z > 0.0 && ndc.z < 1.0, "point {i} is between the near plane (the glass) and the far plane: depth {}", ndc.z);
            checked += 1;
        }
        assert!(checked >= 2, "eye {eye:?}: only {checked} of the points are in the mirror; the test checked too little");
    }
}

/// WHAT LIES BEHIND THE GLASS IS CLIPPED: the wall the mirror stands against, and the mirror's own
/// body 2 mm behind the glass, are nearer to the reflection camera than its near plane (the glass),
/// so their depth is past 1 (reverse-Z) and the clipper drops them; a point just in front of the
/// glass is kept. Seen red 2026-11-21 with the near plane at half the eye's distance from the glass
/// (the frustum's shape through the glass kept): "a point at z = -0.002 has depth 0.50050: behind
/// the glass, it must be clipped".
#[test]
fn what_lies_behind_the_glass_is_clipped() {
    let (o, u, v, n) = glass();
    let eye = Vec3::new(0.2, 1.6, 2.0);
    let view = reflected_view(o, u, v, n, eye, FAR).unwrap();
    let vp = view.proj * view.view;
    for (z, behind) in [(-0.002, true), (-0.05, true), (-1.0, true), (0.002, false), (0.5, false)] {
        let c = vp * Vec4::new(0.0, 1.0, z, 1.0);
        let depth = c.z / c.w;
        assert_eq!(depth > 1.0, behind, "a point at z = {z} has depth {depth:.5}: {}", if behind { "behind the glass, it must be clipped" } else { "in front of the glass, it must be kept" });
    }
}

/// An eye behind the glass, or on it, sees no reflection. Seen red 2026-11-21 with the eye's distance
/// taken without its sign: "assertion failed: reflected_view(o, u, v, n, Vec3::new(0.0, 1.0, -1.0),
/// FAR).is_none()".
#[test]
fn an_eye_behind_the_glass_sees_no_reflection() {
    let (o, u, v, n) = glass();
    assert!(reflected_view(o, u, v, n, Vec3::new(0.0, 1.0, -1.0), FAR).is_none());
    assert!(reflected_view(o, u, v, n, Vec3::new(0.0, 1.0, 0.0), FAR).is_none());
    assert!(reflected_view(o, u, v, n, Vec3::new(0.0, 1.0, 1.0), FAR).is_some());
}

/// THE VIEW IS A ROTATION, NOT A REFLECTION: its 3 x 3 part has determinant +1, so every triangle
/// keeps its winding and back-face culling keeps working in the reflection. (A reflection matrix,
/// determinant -1, would turn every front face into a back face.) Seen red 2026-11-21 with an x
/// reflection multiplied into the view: "the view's rotation has determinant -1".
#[test]
fn the_reflection_view_keeps_triangle_winding() {
    let (o, u, v, n) = glass();
    let view = reflected_view(o, u, v, n, Vec3::new(0.4, 1.5, 1.2), FAR).unwrap();
    let det = glam::Mat3::from_mat4(view.view).determinant();
    assert!((det - 1.0).abs() < 1.0e-5, "the view's rotation has determinant {det}");
}

fn lvl(scale: f32, max_side: u32) -> MirrorLevel {
    MirrorLevel { id: "t".into(), scale, max_side }
}

/// THE PICTURE FOLLOWS THE GLASS ON SCREEN: the level's scale times the glass's size in pixels,
/// rounded up to the size step; capped at the level's longest side with the aspect kept; nothing
/// at Off or for a mirror too small on screen. Seen red 2026-11-21 with the cap left out: "left:
/// Some((2048, 1504)) right: Some((2048, 1024))" (the aspect lost).
#[test]
fn the_picture_follows_the_glass_on_screen() {
    assert_eq!(picture_size(&lvl(1.0, 2048), (300.0, 600.0)), Some((320, 608)));
    assert_eq!(picture_size(&lvl(0.5, 1024), (300.0, 600.0)), Some((160, 320)));
    assert_eq!(picture_size(&lvl(1.0, 2048), (3000.0, 1500.0)), Some((2048, 1024)));
    assert_eq!(picture_size(&lvl(0.0, 2048), (300.0, 600.0)), None, "Off draws nothing");
    assert_eq!(picture_size(&lvl(1.0, 2048), (10.0, 20.0)), None, "a mirror 20 px tall on screen is not drawn");
    assert_eq!(picture_size(&lvl(1.0, 2048), (f32::NAN, 20.0)), None);
    // The two levels the shipped file has differ in what they draw.
    let (low, high) = (picture_size(&lvl(0.5, 1024), (700.0, 1300.0)).unwrap(), picture_size(&lvl(1.0, 2048), (700.0, 1300.0)).unwrap());
    assert!(reflection_bytes(low) * 3 < reflection_bytes(high), "Low holds about a quarter of High's pixels: {low:?} against {high:?}");
}

/// THE SHIPPED LEVELS: Off, Low and High, in that order, Off drawing nothing and each later level
/// drawing more; the default is listed. And a file that does not parse still leaves a level. Seen red
/// 2026-11-21 with the fallback level left out: "left: 0 right: 1" (no level at all, so nothing to
/// fall back to).
#[test]
fn the_shipped_levels_are_off_low_and_high() {
    let text = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/data/gui/mirror_levels.json")).expect("the levels file");
    let levels = parse_levels(Some(&text));
    let ids: Vec<&str> = levels.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["off", "low", "high"]);
    assert_eq!(levels[0].scale, 0.0);
    assert!(levels[1].scale > 0.0 && levels[1].scale < levels[2].scale);
    assert!(levels.iter().any(|l| l.id == DEFAULT_LEVEL));
    let broken = parse_levels(Some("{ not json"));
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].id, DEFAULT_LEVEL);
}
