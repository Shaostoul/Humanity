//! Procedural CONDUIT ROUTING (v0.535): the realistic routed pipes/cords the operator asked for --
//! "the straight lines that pass through everything for the pipes is wrong. We want realistic routed
//! pipes/conduits ... copper pipes for all potable water ... rigid tubes ... flexible tubes are
//! hoses and power extension cords ... automatically procedurally draw their support structures,
//! mounting brackets and passthrough gaskets."
//!
//! This module is the PURE-GEOMETRY core: given two endpoints + the home's interior walls, it routes
//! a conduit ALONG the structure (up to a service height near the ceiling, a Manhattan run, then down
//! to the fixture) instead of straight through the room, and it places the FITTINGS -- brackets where
//! it mounts, elbows at bends, and a passthrough gasket wherever a run crosses an interior wall (the
//! fitting carries the MATERIAL of what it attaches to, so the renderer picks the right bracket).
//! Mesh generation (copper cylinders, sagging hoses, bracket shapes) is a render step that consumes
//! this. Keeping it GPU-free makes it unit-testable and lets it compile in the headless relay.

use crate::ship::home_structure::InteriorWall;
use glam::Vec3;

/// What a conduit physically is. The kind fixes the material + look + whether it may sag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitKind {
    /// Rigid copper tube. MANDATORY for all potable water (operator: "no PVC or other synthetics
    /// that leach toxins"). Straight runs + elbow fittings.
    RigidCopper,
    /// A flexible hose (greywater, gas, air) -- bends + sags between supports.
    FlexibleHose,
    /// A flexible power extension cord -- sags between supports.
    PowerCord,
}

impl ConduitKind {
    /// Map a machine-connection resource to the correct conduit. Potable water is ALWAYS rigid
    /// copper; power is a flexible cord; everything fluid-ish defaults to a flexible hose.
    pub fn for_resource(resource: &str) -> ConduitKind {
        let r = resource.to_ascii_lowercase();
        if r.contains("potable") || r == "water" || r.contains("drink") {
            ConduitKind::RigidCopper
        } else if r.contains("power") || r.contains("electric") || r.contains("volt") {
            ConduitKind::PowerCord
        } else {
            ConduitKind::FlexibleHose
        }
    }

    /// Is this a rigid run (straight segments + elbows) vs a flexible one (it may sag)?
    pub fn is_rigid(self) -> bool {
        matches!(self, ConduitKind::RigidCopper)
    }

    /// Outer radius (metres) for the run.
    pub fn radius(self) -> f32 {
        match self {
            ConduitKind::RigidCopper => 0.012,
            ConduitKind::FlexibleHose => 0.016,
            ConduitKind::PowerCord => 0.008,
        }
    }

    /// Display colour (rgba) for the run.
    pub fn color(self) -> [f32; 4] {
        match self {
            ConduitKind::RigidCopper => [0.72, 0.45, 0.20, 1.0], // copper
            ConduitKind::FlexibleHose => [0.20, 0.22, 0.24, 1.0], // dark rubber
            ConduitKind::PowerCord => [0.10, 0.10, 0.12, 1.0],   // black cord
        }
    }
}

/// A procedurally placed support/fitting along a conduit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConduitFitting {
    pub at: Vec3,
    pub kind: FittingKind,
    /// Material id of the surface the fitting attaches to / passes through (0=grid, 1=steel,
    /// 2=concrete, 3=wood) -- the renderer picks a bracket/gasket suited to that material.
    pub material: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FittingKind {
    /// A mounting bracket clamping the run to a surface (placed along long runs + at the ends).
    Bracket,
    /// An elbow at a bend in a rigid run.
    Elbow,
    /// A gasketed passthrough where the run crosses a wall.
    Passthrough,
}

/// A fully routed conduit: the centre-line polyline + the fittings to draw along it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConduitRoute {
    pub kind: ConduitKind,
    pub points: Vec<Vec3>,
    pub fittings: Vec<ConduitFitting>,
}

/// Spacing (metres) between mounting brackets along a straight run.
const BRACKET_SPACING: f32 = 1.5;

/// Route a conduit from `from` to `to`, running it up to `service_y` (a height near the ceiling) and
/// across in Manhattan legs rather than straight through the room. `shell_material` is the box
/// material (the ceiling/outer surface the horizontal run mounts to); `walls` are the interior walls,
/// used to place passthrough fittings where a run crosses one.
pub fn route_conduit(
    from: Vec3,
    to: Vec3,
    kind: ConduitKind,
    service_y: f32,
    shell_material: u32,
    walls: &[InteriorWall],
) -> ConduitRoute {
    // Service height sits above both endpoints (run along the ceiling, drop to fixtures).
    let sy = service_y.max(from.y).max(to.y);
    let rise = Vec3::new(from.x, sy, from.z);
    let corner = Vec3::new(to.x, sy, from.z);
    let above = Vec3::new(to.x, sy, to.z);
    // Collapse degenerate legs so we don't emit zero-length segments / duplicate points.
    let mut points: Vec<Vec3> = Vec::with_capacity(5);
    for p in [from, rise, corner, above, to] {
        if points.last().map_or(true, |last: &Vec3| last.distance(p) > 1e-3) {
            points.push(p);
        }
    }

    let mut fittings: Vec<ConduitFitting> = Vec::new();
    // A bracket at each interior bend; an elbow too for a rigid run.
    for i in 1..points.len().saturating_sub(1) {
        fittings.push(ConduitFitting { at: points[i], kind: FittingKind::Bracket, material: shell_material });
        if kind.is_rigid() {
            fittings.push(ConduitFitting { at: points[i], kind: FittingKind::Elbow, material: shell_material });
        }
    }
    // Brackets spaced along each long straight leg (rigid runs need support; cords droop without it).
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        if len > BRACKET_SPACING {
            let n = (len / BRACKET_SPACING).floor() as i32;
            for k in 1..n {
                let t = k as f32 / n as f32;
                fittings.push(ConduitFitting { at: a.lerp(b, t), kind: FittingKind::Bracket, material: shell_material });
            }
        }
    }
    // Passthrough gaskets where any horizontal leg crosses an interior wall (in plan, XZ).
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        for wall in walls {
            if let Some((hit, mat)) = leg_crosses_wall(a, b, wall) {
                fittings.push(ConduitFitting { at: hit, kind: FittingKind::Passthrough, material: mat });
            }
        }
    }

    ConduitRoute { kind, points, fittings }
}

/// If conduit leg a->b crosses interior `wall` in plan (XZ), return the crossing point (at the leg's
/// height) + the wall material. Only meaningful for the horizontal legs (vertical legs share a single
/// XZ point and never "cross").
fn leg_crosses_wall(a: Vec3, b: Vec3, wall: &InteriorWall) -> Option<(Vec3, u32)> {
    let p = (a.x, a.z);
    let r = (b.x - a.x, b.z - a.z);
    let q = (wall.a.0, wall.a.1);
    let s = (wall.b.0 - wall.a.0, wall.b.1 - wall.a.1);
    let rxs = r.0 * s.1 - r.1 * s.0;
    if rxs.abs() < 1e-6 {
        return None; // parallel / collinear -- no clean single crossing
    }
    let qp = (q.0 - p.0, q.1 - p.1);
    let t = (qp.0 * s.1 - qp.1 * s.0) / rxs;
    let u = (qp.0 * r.1 - qp.1 * r.0) / rxs;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        let hit = Vec3::new(a.x + r.0 * t, (a.y + b.y) * 0.5, a.z + r.1 * t);
        Some((hit, wall.material))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(a: (f32, f32), b: (f32, f32), material: u32) -> InteriorWall {
        InteriorWall { a, b, height: 3.0, material, openings: Vec::new(), thickness: None }
    }

    #[test]
    fn potable_water_is_always_copper() {
        assert_eq!(ConduitKind::for_resource("potable_water"), ConduitKind::RigidCopper);
        assert_eq!(ConduitKind::for_resource("water"), ConduitKind::RigidCopper);
        assert_eq!(ConduitKind::for_resource("drinking water"), ConduitKind::RigidCopper);
        assert!(ConduitKind::RigidCopper.is_rigid());
    }

    #[test]
    fn power_and_other_are_flexible() {
        assert_eq!(ConduitKind::for_resource("power"), ConduitKind::PowerCord);
        assert_eq!(ConduitKind::for_resource("electricity"), ConduitKind::PowerCord);
        assert_eq!(ConduitKind::for_resource("greywater"), ConduitKind::FlexibleHose);
        assert!(!ConduitKind::PowerCord.is_rigid());
    }

    #[test]
    fn route_goes_up_over_and_down_not_straight() {
        let from = Vec3::new(2.0, 0.8, 2.0);
        let to = Vec3::new(20.0, 0.8, 30.0);
        let r = route_conduit(from, to, ConduitKind::RigidCopper, 2.7, 1, &[]);
        assert_eq!(*r.points.first().unwrap(), from);
        assert_eq!(*r.points.last().unwrap(), to);
        // It must rise to the service height -- not run straight at fixture height.
        assert!(r.points.iter().any(|p| p.y >= 2.7 - 1e-3), "conduit routes up to the ceiling");
        // Rigid bends get elbows.
        assert!(r.fittings.iter().any(|f| f.kind == FittingKind::Elbow), "rigid bends get elbows");
    }

    #[test]
    fn long_run_gets_spaced_brackets() {
        let r = route_conduit(Vec3::new(0.0, 0.5, 0.0), Vec3::new(10.0, 0.5, 0.0), ConduitKind::PowerCord, 2.7, 1, &[]);
        let brackets = r.fittings.iter().filter(|f| f.kind == FittingKind::Bracket).count();
        assert!(brackets >= 3, "a 10 m run gets several brackets, got {brackets}");
    }

    #[test]
    fn crossing_an_interior_wall_adds_a_material_aware_passthrough() {
        // A horizontal run along z=0 at service height; a wood wall crossing its path at x=5.
        let from = Vec3::new(0.0, 2.7, 0.0);
        let to = Vec3::new(10.0, 2.7, 0.0);
        let walls = [wall((5.0, -2.0), (5.0, 2.0), 3)]; // material 3 = wood
        let r = route_conduit(from, to, ConduitKind::FlexibleHose, 2.7, 1, &walls);
        let pass: Vec<_> = r.fittings.iter().filter(|f| f.kind == FittingKind::Passthrough).collect();
        assert_eq!(pass.len(), 1, "one passthrough where the run crosses the wall");
        assert_eq!(pass[0].material, 3, "the passthrough carries the WALL's material (wood)");
        assert!((pass[0].at.x - 5.0).abs() < 1e-3, "crossing is at the wall, x=5");
    }
}
