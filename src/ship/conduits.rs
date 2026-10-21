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

use crate::ship::home_structure::{FloorSlab, InteriorWall};
use glam::Vec3;

/// What a conduit physically is. The kind fixes how the run behaves (rigid with elbows, or
/// flexible) and its size; what it LOOKS like (its material's colour, metal, roughness) is data,
/// `data/piping/pipe_materials.ron` keyed by `id()` (2026-10-04, `ship::pipe_materials`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitKind {
    /// Rigid copper tube. MANDATORY for all potable water (operator: "no PVC or other synthetics
    /// that leach toxins"). Straight runs + elbow fittings.
    RigidCopper,
    /// A flexible hose (greywater, gas, air) -- bends + sags between supports.
    FlexibleHose,
    /// A flexible power extension cord -- sags between supports.
    PowerCord,
    /// A data cable (ethernet and the like), thin and flexible (2026-10-04: it was drawn as a
    /// 32 mm rubber hose before).
    DataCable,
}

impl ConduitKind {
    /// Every kind, for registries that must cover them all.
    pub const ALL: [ConduitKind; 4] =
        [ConduitKind::RigidCopper, ConduitKind::FlexibleHose, ConduitKind::PowerCord, ConduitKind::DataCable];

    /// The id `data/piping/pipe_materials.ron` keys this kind's material by.
    pub fn id(self) -> &'static str {
        match self {
            ConduitKind::RigidCopper => "rigid_copper",
            ConduitKind::FlexibleHose => "flexible_hose",
            ConduitKind::PowerCord => "power_cord",
            ConduitKind::DataCable => "data_cable",
        }
    }

    /// Map a machine-connection resource to the correct conduit. Potable water is ALWAYS rigid
    /// copper, and that includes the home's hot water (drinking water heated: the water heater
    /// is fed from the purifier); power is a flexible cord; data a data cable; everything
    /// fluid-ish defaults to a flexible hose.
    pub fn for_resource(resource: &str) -> ConduitKind {
        let r = resource.to_ascii_lowercase();
        if r.contains("potable") || r == "water" || r == "hot_water" || r.contains("drink") {
            ConduitKind::RigidCopper
        } else if r.contains("power") || r.contains("electric") || r.contains("volt") {
            ConduitKind::PowerCord
        } else if r == "data" || r.contains("ethernet") || r.contains("fibre") || r.contains("fiber") {
            ConduitKind::DataCable
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
            // A Cat6 cable is about 6 mm across.
            ConduitKind::DataCable => 0.004,
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

/// Add a gasketed passthrough (a floor sleeve) wherever a vertical leg of `route` crosses one of
/// `slabs` (ship homes increment 7: a riser from a machine downstairs to one upstairs goes up
/// through the floor between them, as a real one does, sleeved). `origin` is where the slabs' body
/// stands in the world (they are in its own metres). The sleeve sits at the slab's mid-plane and
/// carries the slab's material as the fittings know it: 2 (concrete, the structural slab the room
/// below sees) for a solid slab, 4 (glass) for a glass one.
pub fn add_slab_passthroughs(route: &mut ConduitRoute, slabs: &[FloorSlab], origin: Vec3) {
    let mut sleeves = Vec::new();
    for w in route.points.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a.x - b.x).abs() > 1e-3 || (a.z - b.z).abs() > 1e-3 {
            continue; // only a vertical leg rises through a floor
        }
        let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
        for s in slabs {
            let mid = origin.y + (s.underside() + s.top) * 0.5;
            if mid > lo && mid < hi && s.contains(a.x - origin.x, a.z - origin.z) {
                let material = if s.passes_light() { 4 } else { 2 };
                sleeves.push(ConduitFitting { at: Vec3::new(a.x, mid, a.z), kind: FittingKind::Passthrough, material });
            }
        }
    }
    route.fittings.extend(sleeves);
}

/// If conduit leg a->b crosses interior `wall` in plan (XZ) AT A HEIGHT THE WALL STANDS AT, return
/// the crossing point (at the leg's height) + the wall material. Only meaningful for the horizontal
/// legs (vertical legs share a single XZ point and never "cross"). The height test (ship homes
/// increment 7) keeps a run under the ground floor's ceiling from gasketing through the walls of the
/// storey above it, and through a half wall it passes over; the wall's base and top are in the
/// run's own frame (the caller moves a body's walls to the world, heights included).
fn leg_crosses_wall(a: Vec3, b: Vec3, wall: &InteriorWall) -> Option<(Vec3, u32)> {
    let (base, top) = wall.y_range();
    let y = (a.y + b.y) * 0.5;
    if y < base - 0.01 || y > top + 0.01 {
        return None;
    }
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
        InteriorWall { a, b, base: 0.0, height: 3.0, material, openings: Vec::new(), thickness: None, layers: Vec::new() }
    }

    #[test]
    fn potable_water_is_always_copper() {
        assert_eq!(ConduitKind::for_resource("potable_water"), ConduitKind::RigidCopper);
        assert_eq!(ConduitKind::for_resource("water"), ConduitKind::RigidCopper);
        assert_eq!(ConduitKind::for_resource("drinking water"), ConduitKind::RigidCopper);
        // The home's hot water is drinking water heated (the heater is fed from the purifier),
        // so the copper rule covers it once its lines carry their own kind (2026-10-04 review).
        assert_eq!(ConduitKind::for_resource("hot_water"), ConduitKind::RigidCopper);
        assert!(ConduitKind::RigidCopper.is_rigid());
    }

    #[test]
    fn power_and_other_are_flexible() {
        assert_eq!(ConduitKind::for_resource("power"), ConduitKind::PowerCord);
        assert_eq!(ConduitKind::for_resource("electricity"), ConduitKind::PowerCord);
        assert_eq!(ConduitKind::for_resource("greywater"), ConduitKind::FlexibleHose);
        assert!(!ConduitKind::PowerCord.is_rigid());
        // A data line is a thin cable, not a hose (2026-10-04).
        assert_eq!(ConduitKind::for_resource("data"), ConduitKind::DataCable);
        assert!(!ConduitKind::DataCable.is_rigid());
        assert!(ConduitKind::DataCable.radius() < ConduitKind::PowerCord.radius());
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

    /// A RUN PASSES THROUGH ONLY THE WALLS AT ITS HEIGHT (ship homes increment 7, plan section 3:
    /// conduits per storey). A run under the ground floor's ceiling, at 2.45 m, crosses the plan line
    /// of a wall upstairs (base 3.0) and of a 2.0 m half wall without passing through either, so
    /// neither gets a gasket; it does pass through a ground wall of 3.0 m, which gets one.
    ///
    /// Seen red 2026-10-20 with main's crossing test put back (a crossing in plan was one at any height): "3
    /// passthroughs, want 1, the ground wall's".
    #[test]
    fn a_run_passes_through_only_the_walls_at_its_height() {
        let mut upstairs = wall((3.0, -2.0), (3.0, 2.0), 2);
        upstairs.base = 3.0;
        let mut half = wall((5.0, -2.0), (5.0, 2.0), 2);
        half.height = 2.0;
        let ground = wall((7.0, -2.0), (7.0, 2.0), 3);
        let r = route_conduit(Vec3::new(0.0, 2.45, 0.0), Vec3::new(10.0, 2.45, 0.0), ConduitKind::FlexibleHose, 2.45, 1, &[upstairs, half, ground]);
        let pass: Vec<_> = r.fittings.iter().filter(|f| f.kind == FittingKind::Passthrough).collect();
        assert_eq!(pass.len(), 1, "{} passthroughs, want 1, the ground wall's", pass.len());
        assert!((pass[0].at.x - 7.0).abs() < 1e-3);
    }

    /// A RISER THROUGH A FLOOR GETS A SLEEVE (plan section 3: conduits per storey). A pipe from a
    /// machine downstairs to one standing on the floor slab above runs under the upstairs ceiling
    /// (5.7 m), so it rises through the slab once, at the machine downstairs, and gets a
    /// passthrough there, at the slab's mid-plane, in the slab's material (2, the concrete the room
    /// below sees); its drop to the machine upstairs stays above the slab. A pipe beside the slab,
    /// rising past its edge, gets none.
    ///
    /// Seen red 2026-10-20 with the sleeves left out (main added none): "0 sleeves where the riser goes through
    /// the floor".
    #[test]
    fn a_riser_through_a_floor_gets_a_sleeve() {
        let floor: FloorSlab = ron::from_str("(a: (0.0, 0.0), b: (10.0, 10.0), top: 3.0)").expect("a slab parses");
        let mut r = route_conduit(Vec3::new(2.0, 0.35, 2.0), Vec3::new(6.0, 3.35, 2.0), ConduitKind::RigidCopper, 5.7, 1, &[]);
        add_slab_passthroughs(&mut r, std::slice::from_ref(&floor), Vec3::ZERO);
        let sleeves: Vec<_> = r.fittings.iter().filter(|f| f.kind == FittingKind::Passthrough).collect();
        assert_eq!(sleeves.len(), 1, "{} sleeves where the riser goes through the floor", sleeves.len());
        let s = sleeves[0];
        assert!((s.at - Vec3::new(2.0, 2.875, 2.0)).length() < 1e-4 && s.material == 2, "a sleeve at {:?}, material {}", s.at, s.material);
        let mut beside = route_conduit(Vec3::new(12.0, 0.35, 2.0), Vec3::new(14.0, 0.35, 2.0), ConduitKind::RigidCopper, 5.7, 1, &[]);
        add_slab_passthroughs(&mut beside, std::slice::from_ref(&floor), Vec3::ZERO);
        assert!(beside.fittings.iter().all(|f| f.kind != FittingKind::Passthrough), "a sleeve on a pipe beside the floor");
    }
}
