//! Doors set into walls (2026-09-28).
//!
//! A blueprint with a `doorway` (a wall with a door in it) is still ONE
//! piece to place, save and take down, but it is drawn and walked into as its
//! parts: the wall either side of the gap, the lintel over it, and the door
//! leaf. [`parts`] is the one place those boxes are made, so what is drawn
//! (`engine::planet_build::push_render_objects`) and what blocks
//! (`engine::build_place::built_piece_segments`) cannot disagree. The door
//! stands in the gap when shut and swings a quarter turn open, out of the
//! gap (the `DoorOpen` marker, which E toggles).
//!
//! Before this, a door could not be set into a wall at all, and since built
//! pieces became solid (v0.1400.0) a closed room could only be entered by
//! taking a wall down.

use super::Doorway;
use crate::ecs::components::Transform;
use glam::Vec3;

/// What a part of a doorway piece is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Wall: either side of the gap, or the lintel over it.
    Wall,
    /// The door leaf.
    Leaf,
}

/// The boxes a piece with a doorway is made of, in the piece's frame (its
/// transform carried through): the wall left and right of the gap, the lintel
/// over it when the gap is lower than the wall, and the door leaf, filling
/// the gap when shut and swung a quarter turn on its hinge (the gap's -x edge)
/// out past the wall's +z face when open. Each is a box in the renderer's
/// convention: `position` the bottom centre, `scale` the size. The gap is
/// kept at least 0.1 m inside each end of the wall.
pub fn parts(tf: &Transform, d: &Doorway, open: bool) -> Vec<(Transform, Part)> {
    let (wall_w, wall_h, thick) = (tf.scale.x, tf.scale.y, tf.scale.z);
    let w = d.width.clamp(0.1, (wall_w - 0.2).max(0.1));
    let h = d.height.clamp(0.1, wall_h);
    let side = (wall_w - w) * 0.5;
    let leaf_t = (thick * 0.4).max(0.03);
    let at = |local: Vec3, size: Vec3, kind: Part| {
        (Transform { position: tf.position + tf.rotation * local, rotation: tf.rotation, scale: size }, kind)
    };
    let mut out = vec![
        at(Vec3::new(-(w + side) * 0.5, 0.0, 0.0), Vec3::new(side, wall_h, thick), Part::Wall),
        at(Vec3::new((w + side) * 0.5, 0.0, 0.0), Vec3::new(side, wall_h, thick), Part::Wall),
    ];
    if wall_h - h > 1.0e-3 {
        out.push(at(Vec3::new(0.0, h, 0.0), Vec3::new(w, wall_h - h, thick), Part::Wall));
    }
    out.push(if open {
        at(Vec3::new(-w * 0.5 + leaf_t * 0.5, 0.0, thick * 0.5 + w * 0.5), Vec3::new(leaf_t, h, w), Part::Leaf)
    } else {
        at(Vec3::ZERO, Vec3::new(w, h, leaf_t), Part::Leaf)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::construction::placement::world_aabb;
    use glam::Quat;

    fn wall() -> Transform {
        Transform { position: Vec3::new(10.0, 0.0, 5.0), rotation: Quat::IDENTITY, scale: Vec3::new(4.0, 3.0, 0.2) }
    }
    const DOOR: Doorway = Doorway { width: 1.0, height: 2.1 };

    /// The parts fill the wall exactly, less the gap: the two sides and the
    /// lintel cover 4 x 3 m minus the 1 x 2.1 m door, the shut leaf fills the
    /// gap, and the open leaf stands clear of it. Red check, run: dropping the
    /// lintel fails the area sum.
    #[test]
    fn a_doorway_wall_is_its_sides_its_lintel_and_its_leaf() {
        let shut = parts(&wall(), &DOOR, false);
        let area: f32 = shut.iter().filter(|(_, k)| *k == Part::Wall).map(|(t, _)| t.scale.x * t.scale.y).sum();
        assert!((area - (4.0 * 3.0 - 1.0 * 2.1)).abs() < 1e-4, "wall area {area}");
        let (leaf, _) = shut.iter().find(|(_, k)| *k == Part::Leaf).unwrap();
        let (lo, hi) = world_aabb(leaf);
        assert!((lo.x - 9.5).abs() < 1e-4 && (hi.x - 10.5).abs() < 1e-4 && (hi.y - 2.1).abs() < 1e-4, "{lo} {hi}");

        let open = parts(&wall(), &DOOR, true);
        let (leaf, _) = open.iter().find(|(_, k)| *k == Part::Leaf).unwrap();
        let (lo, hi) = world_aabb(leaf);
        assert!(lo.z >= 5.1 - 1e-4 && hi.z <= 6.1 + 1e-4, "swung out past the +z face: {lo} {hi}");
        assert!(hi.x <= 9.5 + 0.2, "on its hinge at the gap's edge: {hi}");
        // Nothing of the wall stands in the gap below the lintel.
        for (t, _) in open.iter().filter(|(_, k)| *k == Part::Wall) {
            let (lo, hi) = world_aabb(t);
            let in_gap_x = hi.x > 9.5 + 1e-4 && lo.x < 10.5 - 1e-4;
            assert!(!in_gap_x || lo.y >= 2.1 - 1e-4, "a wall part in the doorway: {lo} {hi}");
        }
    }

    /// Turned a quarter, the parts turn with the wall.
    #[test]
    fn the_parts_turn_with_the_wall() {
        let mut t = wall();
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        for (p, _) in parts(&t, &DOOR, false) {
            assert_eq!(p.rotation, t.rotation);
            assert!((p.position.x - 10.0).abs() < 1e-4, "along z now: {}", p.position);
        }
    }
}
