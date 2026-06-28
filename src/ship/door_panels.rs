//! Door/window PANEL PLACEMENT (v0.537): the static, world-space geometry of the panel that fills
//! each opening cut into a wall. The renderer animates these via `systems::door_anim` (a PanelMotion
//! in the panel's local frame: x along the wall, y up, z the wall normal), so a panel can swing,
//! slide, iris, dissolve, etc. by its data-driven `style`.
//!
//! Pure geometry, GPU-free + unit-testable: given a HomeStructure it returns one PanelPlacement per
//! opening. The box min corner sits at the world origin, so wall (x, z) == world (x, z). The hinge is
//! the opening's `a`-side vertical edge (for swing/rotate styles); slides/irises ignore it.

use crate::ship::home_structure::{HomeStructure, OpeningKind};
use glam::{Quat, Vec3};

/// Panel thickness (metres) -- a door slab / window pane.
pub const PANEL_THICKNESS: f32 = 0.06;

/// A lock resolved to world space for the render + the runtime (v0.570): its catalog type, its
/// AUTHORED initial state, and where it mounts on the door face (a small red/green indicator). The
/// LIVE state lives in EngineState (parallel to door_panels); this carries the initial value.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLock {
    pub type_id: String,
    pub state: crate::ship::lock_types::LockState,
    /// World mount position of the lock indicator (on the door face, stacked by index).
    pub pos: Vec3,
}

/// A door/window panel's CLOSED placement in world space + the metadata the animator needs.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelPlacement {
    /// Panel anchor (world) when closed: the opening's centre along the wall + on the wall line in
    /// x/z, and its BOTTOM (the sill) in y -- the panel mesh (`box_xyz`) extends UP from here, so the
    /// panel fills [sill, sill+height] rather than floating at mid-height. (v0.540)
    pub center: Vec3,
    /// Orientation: maps panel-local (x = along the wall a->b, y = up, z = wall normal) to world.
    /// Pure yaw about Y (walls are vertical), so "up" always stays up.
    pub rotation: Quat,
    /// World pivot for a hinge swing: the opening's `a`-side vertical edge, at the panel bottom.
    pub hinge: Vec3,
    /// Panel size (along-wall width, height, thickness).
    pub size: Vec3,
    /// The opening's data-driven animation style (see systems::door_anim).
    pub style: String,
    /// True for a window (a fixed glass pane); false for an operable door.
    pub is_window: bool,
    /// Auto-open (interaction) distance in metres -- the door opens within this horizontal range, and
    /// the editor draws a ground ring at this radius. (v0.547)
    pub open_dist: f32,
    /// Locked: the panel stays shut; an energy door glows red (vs green unlocked). (v0.554)
    pub locked: bool,
    /// AUTO-open within open_dist, vs MANUAL (stays shut until acted on). (v0.564)
    pub auto_open: bool,
    /// This door has a wall-mounted control panel (v0.567); `control_panel_pos` is its world position
    /// (beside the door at hand height) for the render + the interact raycast.
    pub control_panel: bool,
    pub control_panel_pos: Vec3,
    /// LOCKS on this door (v0.570), resolved to world mount positions + authored initial states. The
    /// door is passable only when every lock is open; the LIVE state is tracked in EngineState.
    pub locks: Vec<ResolvedLock>,
}

/// Compute a PanelPlacement for every opening in the home (world space).
pub fn panel_placements(home: &HomeStructure) -> Vec<PanelPlacement> {
    let mut out = Vec::new();
    for wall in &home.walls {
        let a = glam::Vec2::new(wall.a.0, wall.a.1);
        let b = glam::Vec2::new(wall.b.0, wall.b.1);
        let span = b - a;
        let len = span.length();
        if len < 1e-4 {
            continue;
        }
        let dir = span / len; // 2D unit along the wall
        // Pure yaw that maps panel-local +X (1,0,0) onto the wall direction (dir.x, 0, dir.y).
        let rotation = Quat::from_rotation_y((-dir.y).atan2(dir.x));
        for op in &wall.openings {
            if op.width <= 0.01 {
                continue;
            }
            let s_center = (op.at + op.width * 0.5).clamp(0.0, len);
            let s_a = op.at.clamp(0.0, len);
            // box_xyz is y-bottom-origin (spans [0, h]), so anchor the panel at the SILL; it extends
            // up by `height` to fill [sill, sill+height]. (v0.540 -- fixes the panel floating ~h/2
            // too high and clipping the roof.)
            let cy = op.sill;
            let c_xz = a + dir * s_center;
            let h_xz = a + dir * s_a;
            // A WINDOW's glass pane is INSET (v0.564) so its edges don't sit exactly on the wall frame
            // around it (which z-fights); a DOOR fills its aperture so it seals.
            let is_window = op.kind == OpeningKind::Window;
            let inset = if is_window { 0.05 } else { 0.0 };
            // Control panel beside the door at hand height (v0.567): prefer just past the door's FAR
            // edge, but if that falls off the wall end, place it past the NEAR edge instead; if the
            // door spans (almost) the whole wall, centre it. Always lands on the wall span, never
            // floating in the void past a corner.
            let far = (op.at + op.width).clamp(0.0, len);
            let near = op.at.clamp(0.0, len);
            let s_cp = if far + 0.25 <= len {
                far + 0.25
            } else if near - 0.25 >= 0.0 {
                near - 0.25
            } else {
                (far + near) * 0.5
            };
            let cp_xz = a + dir * s_cp;
            let control_panel_pos = Vec3::new(cp_xz.x, 1.2, cp_xz.y);
            // Resolve each lock to a world mount on the door face (v0.570): along the wall at the
            // door centre + the lock's offset, stacked UP by index so multiple locks form a column.
            let locks: Vec<ResolvedLock> = op
                .locks
                .iter()
                .enumerate()
                .map(|(li, lock)| {
                    let s = (op.at + op.width * 0.5 + lock.offset).clamp(0.0, len);
                    let m = a + dir * s;
                    ResolvedLock {
                        type_id: lock.type_id.clone(),
                        state: lock.state,
                        pos: Vec3::new(m.x, 1.0 + 0.28 * li as f32, m.y),
                    }
                })
                .collect();
            out.push(PanelPlacement {
                center: Vec3::new(c_xz.x, cy + inset * 0.5, c_xz.y),
                rotation,
                hinge: Vec3::new(h_xz.x, cy, h_xz.y),
                size: Vec3::new((op.width - inset).max(0.05), (op.height - inset).max(0.05), PANEL_THICKNESS),
                style: op.style.clone(),
                is_window,
                open_dist: op.open_dist,
                locked: op.locked,
                auto_open: op.auto_open,
                control_panel: op.control_panel,
                control_panel_pos,
                locks,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::home_structure::{InteriorWall, Opening};

    fn home_with(openings: Vec<Opening>) -> HomeStructure {
        HomeStructure {
            width: 20.0,
            depth: 20.0,
            height: 3.0,
            shell_material: 1,
            roof_material: 4,
            walls: vec![InteriorWall {
                a: (0.0, 0.0),
                b: (10.0, 0.0), // along +X
                height: 3.0,
                material: 1,
                openings,
                thickness: None,
            }],
            shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new(),
        }
    }

    #[test]
    fn a_door_centers_in_its_aperture_with_an_a_side_hinge() {
        let p = panel_placements(&home_with(vec![Opening {
            kind: OpeningKind::Door,
            at: 4.0,
            width: 2.0,
            sill: 0.0,
            height: 2.1,
            style: "swing".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
        }]));
        assert_eq!(p.len(), 1);
        // Centre at s = 4 + 1 = 5 along +X; bottom-anchored at the sill (y = 0 for a door).
        assert!((p[0].center.x - 5.0).abs() < 1e-4 && (p[0].center.z - 0.0).abs() < 1e-4);
        assert!(p[0].center.y.abs() < 1e-4, "panel bottom sits at the sill (floor)");
        // Hinge at the a-side edge, s = 4.
        assert!((p[0].hinge.x - 4.0).abs() < 1e-4);
        assert_eq!(p[0].size, Vec3::new(2.0, 2.1, PANEL_THICKNESS));
        assert!(!p[0].is_window);
    }

    #[test]
    fn a_window_is_flagged_and_sits_at_its_sill() {
        let p = panel_placements(&home_with(vec![Opening {
            kind: OpeningKind::Window,
            at: 2.0,
            width: 1.5,
            sill: 1.0,
            height: 1.2,
            style: "fixed".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
        }]));
        assert_eq!(p.len(), 1);
        assert!(p[0].is_window);
        // Anchored just above the sill (1.0) -- the glass is inset 0.05 m so it does not z-fight the
        // wall frame, so the pane bottom sits at sill + inset/2 = 1.025. (v0.564)
        assert!((p[0].center.y - 1.025).abs() < 1e-4, "got {}", p[0].center.y);
        assert!(p[0].size.y < 1.2, "window pane height is inset below the aperture, got {}", p[0].size.y);
    }

    #[test]
    fn a_wall_along_z_yaws_the_panel_ninety_degrees() {
        let mut home = home_with(vec![Opening {
            kind: OpeningKind::Door,
            at: 1.0,
            width: 1.0,
            sill: 0.0,
            height: 2.1,
            style: "slide".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
        }]);
        home.walls[0].a = (5.0, 0.0);
        home.walls[0].b = (5.0, 10.0); // along +Z
        let p = panel_placements(&home);
        assert_eq!(p.len(), 1);
        // Panel-local +X should map to world +Z. Rotate (1,0,0) by the panel rotation.
        let mapped = p[0].rotation * Vec3::X;
        assert!((mapped.z - 1.0).abs() < 1e-4, "wall along +Z yaws local X to world +Z");
        assert!(mapped.y.abs() < 1e-4, "up stays up");
    }

    #[test]
    fn a_control_panel_sits_beside_the_door_at_hand_height() {
        // Door spans s = 4..6 along the +X wall; the panel goes just past the far edge (6) + 0.25 m,
        // on the wall line, at hand height 1.2 m. (v0.567)
        let p = panel_placements(&home_with(vec![Opening {
            kind: OpeningKind::Door,
            at: 4.0,
            width: 2.0,
            sill: 0.0,
            height: 2.1,
            style: "swing".into(), open_dist: 2.6, locked: false, auto_open: false, control_panel: true, locks: Vec::new()
        }]));
        assert_eq!(p.len(), 1);
        assert!(p[0].control_panel, "panel flag carried through");
        let cp = p[0].control_panel_pos;
        assert!((cp.x - 6.25).abs() < 1e-4, "panel past the far door edge, got x={}", cp.x);
        assert!((cp.z - 0.0).abs() < 1e-4, "panel on the wall line, got z={}", cp.z);
        assert!((cp.y - 1.2).abs() < 1e-4, "panel at hand height, got y={}", cp.y);
    }

    #[test]
    fn a_control_panel_at_the_wall_end_falls_back_to_the_near_side() {
        // Door at s = 8..10 -- its far edge IS the wall end (len 10), so far+0.25 would float past the
        // corner; the panel must fall back to the near edge (8) - 0.25 = 7.75, still on the wall. (v0.567)
        let p = panel_placements(&home_with(vec![Opening {
            kind: OpeningKind::Door,
            at: 8.0,
            width: 2.0,
            sill: 0.0,
            height: 2.1,
            style: "swing".into(), open_dist: 2.6, locked: false, auto_open: false, control_panel: true, locks: Vec::new()
        }]));
        let cp = p[0].control_panel_pos;
        assert!((cp.x - 7.75).abs() < 1e-4, "panel falls back inside the wall, got x={}", cp.x);
        assert!(cp.x <= 10.0, "panel never floats past the wall end, got x={}", cp.x);
    }

    #[test]
    fn no_control_panel_by_default() {
        let p = panel_placements(&home_with(vec![Opening {
            kind: OpeningKind::Door,
            at: 4.0, width: 2.0, sill: 0.0, height: 2.1,
            style: "swing".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
        }]));
        assert!(!p[0].control_panel, "no panel unless requested");
    }

    #[test]
    fn no_walls_no_panels() {
        let home = HomeStructure { width: 10.0, depth: 10.0, height: 3.0, shell_material: 1, roof_material: 4, walls: vec![], shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new() };
        assert!(panel_placements(&home).is_empty());
    }
}
