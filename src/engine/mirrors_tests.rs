//! The mirrors' pure parts (engine/mirrors.rs) and the shipped bedroom: no GPU.

use super::*;
use crate::renderer::camera::Camera;

/// The bedroom's standing mirror as placed: base at (41.892, 3, 32.5), turned 90 degrees, so its
/// front (the glass) faces -X into the room; 1 x 2 x 0.08 m with a 4 cm frame.
fn bedroom_glass() -> QuadGeom {
    quad_from_placement(Vec3::new(41.892, 3.0, 32.5), (1.0, 2.0, 0.08), 90.0, "front", 0.04)
}

/// A camera at `pos` looking along yaw `yaw` (radians, the camera's own convention), 16:9.
fn camera_at(pos: Vec3, yaw: f32) -> Mat4 {
    let mut c = Camera::new();
    c.position = pos;
    c.yaw = yaw;
    c.pitch = -0.1;
    c.aspect = 2560.0 / 1440.0;
    c.view_projection_matrix()
}

const SCREEN: (u32, u32) = (2560, 1440);

/// A MIRROR IN VIEW IS PICKED, ONE OUT OF VIEW IS NOT. Standing in the bedroom 2.3 m from the
/// glass and facing it (yaw +90 degrees: the camera looks along +X), the mirror is picked and its
/// glass covers hundreds of pixels each way; turned round (facing -X), standing behind the glass,
/// farther than the reach, or with the glass hidden, it is not. Seen red 2026-11-21 with the
/// frustum test left out: "turned away from the mirror" (the mirror behind the viewer picked).
#[test]
fn a_mirror_in_view_is_picked_and_one_out_of_view_is_not() {
    let glass = [bedroom_glass()];
    let eye = Vec3::new(39.6, 4.7, 32.5);
    let facing = pick(&glass, eye, camera_at(eye, std::f32::consts::FRAC_PI_2), SCREEN, MAX_DISTANCE_M, |_| true);
    let (i, (w, h)) = facing.expect("facing the mirror: None");
    assert_eq!(i, 0);
    // 0.92 x 1.92 m of glass 2.25 m away under a 90 degree field of view on 1440 rows: about 294 x
    // 614 pixels.
    assert!(w > 250.0 && w < 350.0 && h > 550.0 && h < 700.0, "the glass covers {w:.0} x {h:.0} px from 2.3 m");
    assert!(pick(&glass, eye, camera_at(eye, -std::f32::consts::FRAC_PI_2), SCREEN, MAX_DISTANCE_M, |_| true).is_none(), "turned away from the mirror");
    let behind = Vec3::new(43.0, 4.7, 32.5);
    assert!(pick(&glass, behind, camera_at(behind, -std::f32::consts::FRAC_PI_2), SCREEN, MAX_DISTANCE_M, |_| true).is_none(), "behind the glass, looking at its back");
    let far = Vec3::new(20.0, 4.7, 32.5);
    assert!(pick(&glass, far, camera_at(far, std::f32::consts::FRAC_PI_2), SCREEN, MAX_DISTANCE_M, |_| true).is_none(), "22 m away");
    assert!(pick(&glass, eye, camera_at(eye, std::f32::consts::FRAC_PI_2), SCREEN, MAX_DISTANCE_M, |_| false).is_none(), "hidden behind a wall");
}

/// THE NEAREST OF TWO MIRRORS IN VIEW REFLECTS. Seen red 2026-11-21 with the farther one kept:
/// "left: Some(0) right: Some(1)".
#[test]
fn the_nearest_mirror_in_view_reflects() {
    let near = bedroom_glass();
    let far = quad_from_placement(Vec3::new(41.892, 3.0, 35.5), (1.0, 2.0, 0.08), 90.0, "front", 0.04);
    let eye = Vec3::new(38.5, 4.7, 33.0);
    let vp = camera_at(eye, std::f32::consts::FRAC_PI_2);
    assert_eq!(pick(&[far, near], eye, vp, SCREEN, MAX_DISTANCE_M, |_| true).map(|p| p.0), Some(1));
    assert_eq!(pick(&[near, far], eye, vp, SCREEN, MAX_DISTANCE_M, |_| true).map(|p| p.0), Some(0));
}

/// THE GLASS'S U RUNS THE OTHER WAY: the reflection's texture holds the glass's u at 1 - u (the
/// mirror image, `renderer::mirror::reflected_view`), so the glass quad's corner that a viewer sees
/// top-left samples the texture's top-RIGHT. Seen red 2026-11-21 with the screen quad's own UVs:
/// "corner 0 left: [0.0, 0.0] right: [1.0, 0.0]".
#[test]
fn the_glass_samples_the_reflection_left_for_right() {
    let local = quad_from_placement(Vec3::ZERO, (1.0, 2.0, 0.08), 0.0, "front", 0.04);
    let (v, i) = glass_vertices(&local);
    let (s, si) = screen_quad_vertices(&local);
    assert_eq!(i, si, "the same triangles, the same winding");
    for k in 0..4 {
        assert_eq!(v[k].position, s[k].position);
        assert_eq!(v[k].uv, [1.0 - s[k].uv[0], s[k].uv[1]], "corner {k}");
    }
}

fn obj(mesh: usize, material: usize) -> RenderObject {
    RenderObject { fade: 0.0, position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE, mesh, material }
}

/// THE REFLECTION LEAVES OUT ITS OWN GLASS AND THE EYE'S FOLDED BODY, nothing else: the glass
/// samples the texture being drawn (a usage the GPU refuses within one pass) and the folded body
/// would show a headless person, whose whole twin the reflection draws instead. Seen red 2026-11-21
/// with the eye-only meshes kept: "left: [1, 3, 4] right: [1, 4]".
#[test]
fn the_reflection_leaves_out_its_own_glass_and_the_eyes_folded_body() {
    let list = [obj(1, 10), obj(2, 77), obj(3, 11), obj(4, 12), obj(5, 77)];
    let kept = reflection_list(&list, Some(77), &[3]);
    let meshes: Vec<usize> = kept.iter().map(|o| o.mesh).collect();
    assert_eq!(meshes, [1, 4]);
    assert_eq!(reflection_list(&list, None, &[]).len(), 5, "no reflection material yet, nothing eye-only: everything");
}

/// THE BEDROOM HAS A REAL MIRROR AND STILL A PROFILE SCREEN (2026-11-21). In both households of the
/// homestead (home.ron, the family; home_solo.ron, just you), the standing mirror is placed in the
/// bedroom with a mirror and no screen, and a screen showing the Profile page is placed in the
/// bedroom too (the mirror showed it until it became a real one; the bedroom's TVs for the game
/// pages come later). Seen red 2026-11-21 after the mirror's screen was taken away and before the
/// Profile screen was placed: "home.ron: no screen in the bedroom shows the Profile page".
#[test]
fn the_bedroom_has_a_real_mirror_and_still_a_profile_screen() {
    for file in ["home.ron", "home_solo.ron"] {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/machines").join(file);
        let home = humanity_systems::grow_machines::load_home(&path).unwrap_or_else(|| panic!("{file} loads"));
        // Every instance of these homes is in the "home" zone; one big zone at the origin places
        // them all (the rooms are only checked by id).
        let zones = vec![humanity_ship::machines::ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (200.0, 200.0, 12.0) }];
        let placed = home.placements(&std::collections::HashMap::new(), Some(&zones));
        let mirror = placed.iter().find(|p| p.id == "standing_mirror_1").unwrap_or_else(|| panic!("{file}: the standing mirror is placed"));
        assert_eq!(mirror.room, "room-bedroom", "{file}");
        assert!(mirror.mirror.is_some(), "{file}: the standing mirror is a mirror");
        assert!(mirror.screen.is_none(), "{file}: the standing mirror is no longer a screen");
        let profile = placed.iter().filter(|p| p.room == "room-bedroom").find(|p| p.screen.as_ref().is_some_and(|s| crate::gui::screen_surface::ScreenSource::parse(&s.source) == crate::gui::screen_surface::ScreenSource::parse("profile")));
        assert!(profile.is_some(), "{file}: no screen in the bedroom shows the Profile page");
    }
}
