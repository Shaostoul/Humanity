//! From ship/door_panels.rs's tests: a corridor mouth's two half panels, opened by how the
//! simulation moves a door (`door_anim::panel_motion`).

use humanity_ship::door_panels::fixtures::corridor_ship;
use humanity_ship::door_panels::*;
use glam::Vec3;

#[test]
fn a_corridor_mouth_gets_two_half_panels_that_part_outward() {
    let panels = corridor_panel_placements(&corridor_ship());
    assert_eq!(panels.len(), 4, "two mouths x two halves");
    // The x = 10 mouth (home's shell): halves flank lat = 5 at z = 4.75 / 5.25, sill on the
    // deck, each half the 1 m door wide and the full 2.1 m tall.
    let (lo, hi) = (&panels[0], &panels[1]);
    assert!((lo.center - Vec3::new(10.0, 0.0, 4.75)).length() < 1e-4, "got {:?}", lo.center);
    assert!((hi.center - Vec3::new(10.0, 0.0, 5.25)).length() < 1e-4, "got {:?}", hi.center);
    for p in [lo, hi] {
        assert_eq!(p.size, Vec3::new(0.5, 2.1, CORRIDOR_PANEL_THICKNESS));
        assert_eq!(p.style, "slide");
        assert!(p.auto_open && !p.is_window && !p.locked && !p.control_panel);
        assert!(p.locks.is_empty());
        assert!((p.open_dist - CORRIDOR_DOOR_OPEN_DIST).abs() < 1e-5);
    }
    // The halves face OPPOSITE ways so the shared "slide" style parts them: local +X maps to
    // world -Z for the low half, +Z for the high half.
    let lo_x = lo.rotation * Vec3::X;
    let hi_x = hi.rotation * Vec3::X;
    assert!((lo_x.z + 1.0).abs() < 1e-4, "low half slides toward -z, got {lo_x:?}");
    assert!((hi_x.z - 1.0).abs() < 1e-4, "high half slides toward +z, got {hi_x:?}");
    // Fully open, each half has cleared the aperture (z 4.5..5.5): the slide translates a
    // half by its own width (0.5 m) into the flanking shell.
    let m = crate::door_anim::panel_motion(&lo.style, 1.0, lo.size.x, lo.size.y);
    let lo_open_z = (lo.center + lo.rotation * Vec3::new(m.offset.0, m.offset.1, m.offset.2)).z;
    assert!(lo_open_z <= 4.25 + 1e-4, "open low half fully in the pocket, got z {lo_open_z}");
    assert!(m.hidden, "a fully-open slide half is culled");
}
