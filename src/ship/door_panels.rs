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

/// Corridor door-panel thickness (metres, v0.795) -- a chunkier slab than an interior door,
/// because a corridor mouth is a pressure boundary between zones, not a room divider.
pub const CORRIDOR_PANEL_THICKNESS: f32 = 0.10;

/// Corridor doors auto-open within this horizontal range (metres). A touch wider than the
/// interior-door default (2.6) so the halves finish parting before a walking player reaches
/// the aperture instead of face-planting a still-opening panel.
pub const CORRIDOR_DOOR_OPEN_DIST: f32 = 3.0;

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

/// Compute a PanelPlacement for every opening in EVERY zone of the ship (v0.754, ship-superstructure
/// increment A): each zone's placements (from the unchanged per-body `panel_placements`) with every
/// world position translated by that zone's origin, concatenated in zone order. Doors in a second
/// zone therefore open/collide exactly like the home's. Corridor mouths append their sliding door
/// pairs at the END of the list (v0.795, `corridor_panel_placements`) so zone-opening indices stay
/// stable relative to each other.
pub fn ship_panel_placements(ship: &crate::ship::ship_structure::ShipStructure) -> Vec<PanelPlacement> {
    let mut out = Vec::new();
    for zone in &ship.zones {
        let o = zone.origin_vec();
        out.extend(panel_placements(&zone.body).into_iter().map(|mut p| {
            p.center += o;
            p.hinge += o;
            p.control_panel_pos += o;
            for l in p.locks.iter_mut() {
                l.pos += o;
            }
            p
        }));
    }
    // Corridor mouths get their own sliding door pair (v0.795). These are built in WORLD space
    // already (corridor geometry is world-resolved), so no zone-origin offset applies.
    out.extend(corridor_panel_placements(ship));
    out
}

/// Two pocket-door HALF-PANELS per corridor mouth (v0.795, the operator's corridor doors): each
/// aperture a corridor cuts through a zone shell -- both tube ends plus any intervening-zone
/// crossings (`ShipStructure::corridor_mouths`) -- gets a pair of half-width slabs that SLIDE
/// APART into the wall on approach and close behind you. Implemented entirely on the existing
/// panel pipeline: each half is an ordinary `PanelPlacement` with the "slide" style, and the two
/// halves face OPPOSITE ways (yaw flipped a half turn), so the one shared style translates each
/// along its own local +X -- away from the centre -- by exactly its own width (dw/2). Closed they
/// tile the aperture seamlessly; fully open both have vanished into the flanking shell. Animation,
/// live collision (a closed door blocks the mouth), hysteresis, and rendering all come from the
/// same code interior doors use -- zero new mechanisms to maintain.
pub fn corridor_panel_placements(
    ship: &crate::ship::ship_structure::ShipStructure,
) -> Vec<PanelPlacement> {
    use crate::ship::ship_structure::CorridorAxis;
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut out = Vec::new();
    for m in ship.corridor_mouths() {
        let (dw, dh) = m.door;
        if dw <= 0.01 || dh <= 0.01 {
            continue; // a degenerate aperture (clamped to nothing) gets no panels
        }
        // side = -1 is the low-coordinate half, +1 the high half, along the axis ACROSS the run.
        for side in [-1.0f32, 1.0] {
            // Each half's yaw points panel-local +X INTO its pocket (the "slide" style translates
            // +local-X), so the pair parts outward from the aperture centre.
            let (center, hinge, yaw) = match m.axis {
                // X-run: the mouth plane is x = plane and the wall runs along Z.
                // from_rotation_y(-PI/2) maps local +X to world +Z (the panel_placements
                // convention for a wall heading +Z); +PI/2 maps it to world -Z.
                CorridorAxis::X => (
                    Vec3::new(m.plane, m.floor_y, m.lat + side * dw * 0.25),
                    Vec3::new(m.plane, m.floor_y, m.lat + side * dw * 0.5),
                    if side > 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 },
                ),
                // Z-run: the mouth plane is z = plane and the wall runs along X. Yaw 0 keeps
                // local +X on world +X; the low half flips a half turn to slide the other way.
                CorridorAxis::Z => (
                    Vec3::new(m.lat + side * dw * 0.25, m.floor_y, m.plane),
                    Vec3::new(m.lat + side * dw * 0.5, m.floor_y, m.plane),
                    if side > 0.0 { 0.0 } else { PI },
                ),
            };
            out.push(PanelPlacement {
                center,
                rotation: Quat::from_rotation_y(yaw),
                // The hinge (unused by "slide") sits at the half's OUTER aperture edge, so a
                // future style swap to swing/rotate would pivot plausibly instead of at a corner.
                hinge,
                size: Vec3::new(dw * 0.5, dh, CORRIDOR_PANEL_THICKNESS),
                style: "slide".to_string(),
                is_window: false,
                open_dist: CORRIDOR_DOOR_OPEN_DIST,
                locked: false,
                // Corridor doors are always automatic: they exist to seal the mouth behind
                // traffic, not to gate access (locks/manual control stay an authored-door thing).
                auto_open: true,
                control_panel: false,
                // Unused while control_panel is false; kept sane (hand height at the door) so a
                // future "add a panel to this door" toggle needs no re-derivation.
                control_panel_pos: Vec3::new(center.x, m.floor_y + 1.2, center.z),
                locks: Vec::new(),
            });
        }
    }
    out
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
                layers: Vec::new(),
            }],
            shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new(),
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
        let home = HomeStructure { width: 10.0, depth: 10.0, height: 3.0, shell_material: 1, roof_material: 4, walls: vec![], shell_thickness: None, lights: Vec::new(), spawn: None, structures: Vec::new(), road_nodes: Vec::new(), road_edges: Vec::new(), zones: Vec::new(), rail_nodes: Vec::new(), rail_edges: Vec::new() };
        assert!(panel_placements(&home).is_empty());
    }

    // ── Corridor door panels (v0.795) ────────────────────────────────────────

    use crate::ship::ship_structure::{ShipCorridor, ShipStructure, ShipZone};

    fn plain_body(w: f32, d: f32, h: f32) -> HomeStructure {
        ron::from_str(&format!("(width: {w}, depth: {d}, height: {h})"))
            .expect("corridor test body parses")
    }

    /// The wall_collision test fixture: home 10x10x3 at the origin, commons 8x8x6 at (20, 0, 2),
    /// one X-run corridor at world z = 5 with a 1 m x 2.1 m door mouth on each end.
    fn corridor_ship() -> ShipStructure {
        ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".into(),
                    label: String::new(),
                    purpose: "residence".into(),
                    origin: (0.0, 0.0, 0.0),
                    body: plain_body(10.0, 10.0, 3.0),
                },
                ShipZone {
                    id: "commons".into(),
                    label: String::new(),
                    purpose: "commons".into(),
                    origin: (20.0, 0.0, 2.0),
                    body: plain_body(8.0, 8.0, 6.0),
                },
            ],
            corridors: vec![ShipCorridor {
                from_zone: "home".into(),
                to_zone: "commons".into(),
                lat: 5.0,
                width: 3.0,
                door_width: 1.0,
                door_height: 2.1,
                glass_top: false,
            }],
        }
    }

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
        let m = crate::systems::door_anim::panel_motion(&lo.style, 1.0, lo.size.x, lo.size.y);
        let lo_open_z = (lo.center + lo.rotation * Vec3::new(m.offset.0, m.offset.1, m.offset.2)).z;
        assert!(lo_open_z <= 4.25 + 1e-4, "open low half fully in the pocket, got z {lo_open_z}");
        assert!(m.hidden, "a fully-open slide half is culled");
    }

    #[test]
    fn a_z_run_corridor_yaws_its_halves_along_x() {
        // Stack the zones along +Z instead: the run flips axes, so the halves flank lat on X and
        // slide along X.
        let mut ship = corridor_ship();
        ship.zones[1].origin = (2.0, 0.0, 20.0);
        let panels = corridor_panel_placements(&ship);
        assert_eq!(panels.len(), 4);
        let (lo, hi) = (&panels[0], &panels[1]);
        assert!((lo.center - Vec3::new(4.75, 0.0, 10.0)).length() < 1e-4, "got {:?}", lo.center);
        assert!((hi.center - Vec3::new(5.25, 0.0, 10.0)).length() < 1e-4, "got {:?}", hi.center);
        let lo_x = lo.rotation * Vec3::X;
        let hi_x = hi.rotation * Vec3::X;
        assert!((lo_x.x + 1.0).abs() < 1e-4, "low half slides toward -x, got {lo_x:?}");
        assert!((hi_x.x - 1.0).abs() < 1e-4, "high half slides toward +x, got {hi_x:?}");
        // Up stays up under both yaws (pure rotation about Y).
        assert!((lo.rotation * Vec3::Y - Vec3::Y).length() < 1e-4);
    }

    #[test]
    fn ship_panel_placements_appends_corridor_doors_after_zone_openings() {
        // A zone door + a corridor: the authored opening keeps index 0 (offset by its zone origin)
        // and the 4 corridor halves follow, in world space with NO zone offset applied to them.
        let mut ship = corridor_ship();
        ship.zones[1].body.walls = home_with(vec![Opening {
            kind: OpeningKind::Door,
            at: 4.0, width: 2.0, sill: 0.0, height: 2.1,
            style: "swing".into(), open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: Vec::new()
        }]).walls;
        let all = ship_panel_placements(&ship);
        assert_eq!(all.len(), 1 + 4, "one authored door + two corridor door pairs");
        // The authored door: local centre (5, 0) on the commons wall, offset by origin (20, 0, 2).
        assert!((all[0].center - Vec3::new(25.0, 0.0, 2.0)).length() < 1e-4, "got {:?}", all[0].center);
        // The first corridor half sits at the home-shell mouth -- world coordinates, un-offset.
        assert!((all[1].center - Vec3::new(10.0, 0.0, 4.75)).length() < 1e-4, "got {:?}", all[1].center);
    }
}
