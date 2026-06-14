//! Auto-routing for pipes, wiring, and ventilation through structures.
//!
//! Routing rules loaded from `data/routing_rules.ron`. The router turns a straight
//! machine-to-machine connection into a realistic ORTHOGONAL run that a plumber or
//! electrician would actually install: a riser up out of the source, a right-angle run
//! along an overhead service band, and a riser down into the destination (the universal
//! "up-over-down" pattern). It then decorates the run with real fittings: elbows at every
//! corner, fitting collars at the machine ports (the "this is hollow pipe, not a solid
//! rod" tell), support brackets at code spacing along the horizontal runs, and a shutoff
//! valve on fluid lines at the destination inlet (IPC 606 requires one per fixture).
//!
//! Grounded in ASME A13.1 (pipe color), IPC 308.5 / NEC 358.30 (support spacing), and
//! NEC 300.4 (electrical-above-water separation). See docs/design (routing spec).
//!
//! Pure geometry (glam only) so it compiles under every feature set; the renderer turns
//! the `PipePart` plan into meshes in `lib.rs::load_world` (native).

use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Route type for infrastructure lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteType {
    Pipe,
    Wire,
    Ventilation,
}

/// Auto-routes infrastructure through a structure.
pub struct AutoRouter;

impl AutoRouter {
    pub fn new() -> Self {
        Self
    }
}

/// Per-kind run parameters: which vertical lane the service rides in (so power, water,
/// waste etc. stack without overlapping) and the pipe's outer radius (water pipe is
/// fatter than electrical conduit).
#[derive(Debug, Clone, Deserialize)]
pub struct LaneRule {
    /// Height offset (m) added to the base run band. Higher = nearer the ceiling. Power
    /// rides highest (above any wet line) per NEC 300.4; waste/drain rides lowest.
    pub lane: f32,
    /// Outer radius (m) of the pipe/conduit for this kind.
    pub radius: f32,
}

/// Routing rules: the service-band geometry + per-kind lanes. Loaded from
/// `data/routing_rules.ron` so an operator can retune run height, support spacing, and
/// pipe sizes without recompiling (data-driven, infinite-of-X).
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingRules {
    /// Height (m) of the main overhead service band above the room floor, before the
    /// per-kind lane offset. Must clear the tallest machine (towers ~2.4 m).
    pub base_run_height: f32,
    /// Keep runs at least this far (m) below the ceiling.
    pub ceiling_margin: f32,
    /// Straight stub (m) out of a machine before the first elbow (unused by the simple
    /// vertical-riser router but kept for face-aware routing later).
    pub stub: f32,
    /// Meters between support brackets along a horizontal run (IPC/NEC max spacing).
    pub bracket_spacing: f32,
    /// Elbow sphere radius = pipe_radius * this.
    pub elbow_mult: f32,
    /// Fitting-collar radius = pipe_radius * this (the "real pipe" diameter-step tell).
    pub collar_mult: f32,
    /// Collar length (m) along the pipe at each machine port.
    pub collar_len: f32,
    /// Per-kind lane + radius.
    pub lanes: HashMap<String, LaneRule>,
    /// Which connection kinds are fluids (get a shutoff valve at the inlet).
    pub fluid_kinds: Vec<String>,
}

impl Default for RoutingRules {
    fn default() -> Self {
        let mut lanes = HashMap::new();
        // top-of-stack (power) down to lowest (waste), per NEC 300.4 separation.
        lanes.insert("power".into(), LaneRule { lane: 0.45, radius: 0.022 });
        lanes.insert("air".into(), LaneRule { lane: 0.35, radius: 0.024 });
        lanes.insert("fuel".into(), LaneRule { lane: 0.30, radius: 0.024 });
        lanes.insert("water".into(), LaneRule { lane: 0.20, radius: 0.028 });
        lanes.insert("nutrient".into(), LaneRule { lane: 0.00, radius: 0.030 });
        lanes.insert("waste".into(), LaneRule { lane: -0.15, radius: 0.034 });
        Self {
            base_run_height: 3.0,
            ceiling_margin: 0.4,
            stub: 0.15,
            bracket_spacing: 1.8,
            elbow_mult: 1.4,
            collar_mult: 1.3,
            collar_len: 0.08,
            lanes,
            fluid_kinds: vec!["water".into(), "nutrient".into(), "fuel".into(), "waste".into()],
        }
    }
}

impl RoutingRules {
    /// Load from RON, falling back to `Default` on a missing or invalid file so the
    /// renderer always has working rules.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match ron::from_str::<RoutingRules>(&text) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("routing: failed to parse {}: {e}; using defaults", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Lane offset + radius for a kind, falling back to a neutral default.
    pub fn lane(&self, kind: &str) -> (f32, f32) {
        self.lanes
            .get(kind)
            .map(|l| (l.lane, l.radius))
            .unwrap_or((0.0, 0.025))
    }

    /// Is this kind a fluid (gets a shutoff valve)?
    pub fn is_fluid(&self, kind: &str) -> bool {
        self.fluid_kinds.iter().any(|k| k == kind)
    }

    /// The run-band height for a connection, clamped into the available headroom of the
    /// two rooms it spans. `floor` = the higher of the two room floors, `ceiling` = the
    /// lower of the two ceilings; the run must clear machines yet stay under the ceiling.
    pub fn run_height(&self, kind: &str, floor: f32, ceiling: f32) -> f32 {
        let (lane, _) = self.lane(kind);
        let want = floor + self.base_run_height + lane;
        let hi = (ceiling - self.ceiling_margin).max(floor + 1.0);
        want.clamp(floor + 1.0, hi)
    }
}

/// One renderable piece of a routed run. The renderer maps each to a mesh: `Tube` ->
/// round pipe (also used for the fat short collars), `Elbow` -> sphere, `Bracket` ->
/// grey clamp box, `Valve` -> a fat body + a lever box, `Penetration` -> a wall sleeve +
/// escutcheon ring where the pipe pierces a wall.
#[derive(Debug, Clone, PartialEq)]
pub enum PipePart {
    Tube { a: Vec3, b: Vec3, radius: f32 },
    Elbow { at: Vec3, radius: f32 },
    Bracket { at: Vec3 },
    Valve { at: Vec3, axis: Vec3, radius: f32 },
    Penetration { at: Vec3, axis: Vec3, radius: f32 },
}

/// The orthogonal "up-over-down" waypoints from machine port `a` to `b`, running through
/// the overhead band at height `run_h`. Travels one axis at a time (X then Z) so every
/// turn is a single 90-degree elbow. Degenerate (zero-length) hops are removed, so a run
/// that shares a column collapses to a clean U or straight riser pair.
pub fn route_orthogonal(a: Vec3, b: Vec3, run_h: f32) -> Vec<Vec3> {
    let raw = [
        a,
        Vec3::new(a.x, run_h, a.z), // riser up from A
        Vec3::new(b.x, run_h, a.z), // along X in the band
        Vec3::new(b.x, run_h, b.z), // along Z in the band
        b,                          // riser down to B
    ];
    let mut out: Vec<Vec3> = Vec::with_capacity(5);
    for p in raw {
        if out.last().map(|q| q.distance_squared(p) > 1e-6).unwrap_or(true) {
            out.push(p);
        }
    }
    out
}

/// The points where a routed polyline crosses a wall: a transition between which room
/// AABB (if any) contains the line. Returns (crossing_point, leg_direction) for each, so
/// the renderer can sleeve the pipe where it pierces a wall. Sampled along each leg at
/// `step` m. `rooms` are (min, max) AABBs; a point in no room (corridor/gap) reads as
/// room index -1, so leaving a room into open space OR crossing into an adjacent room
/// both register as a wall.
pub fn wall_crossings(waypoints: &[Vec3], rooms: &[(Vec3, Vec3)], step: f32) -> Vec<(Vec3, Vec3)> {
    let containing = |p: Vec3| -> i32 {
        for (i, (mn, mx)) in rooms.iter().enumerate() {
            if p.x >= mn.x && p.x <= mx.x && p.y >= mn.y && p.y <= mx.y && p.z >= mn.z && p.z <= mx.z {
                return i as i32;
            }
        }
        -1
    };
    let mut out: Vec<(Vec3, Vec3)> = Vec::new();
    let step = step.max(0.05);
    for seg in waypoints.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let len = (b - a).length();
        if len < 1e-4 {
            continue;
        }
        let dir = (b - a) / len;
        let n = (len / step).ceil() as i32;
        let mut prev = containing(a);
        for k in 1..=n {
            let t = (k as f32 * step).min(len);
            let p = a + dir * t;
            let cur = containing(p);
            if cur != prev {
                out.push((p - dir * (step * 0.5), dir)); // midpoint of the crossing step
                prev = cur;
            }
        }
    }
    out
}

/// Plan the full set of renderable parts for one connection: pipe bodies, elbows at the
/// corners, fitting collars at the two machine ports, support brackets along the
/// horizontal runs, a wall sleeve where the run pierces a wall (`rooms` = room AABBs), and
/// (for fluids) a shutoff valve on the destination riser.
pub fn plan_pipe(
    a: Vec3,
    b: Vec3,
    radius: f32,
    run_h: f32,
    is_fluid: bool,
    rooms: &[(Vec3, Vec3)],
    rules: &RoutingRules,
) -> Vec<PipePart> {
    let wp = route_orthogonal(a, b, run_h);
    let mut parts: Vec<PipePart> = Vec::new();
    if wp.len() < 2 {
        return parts;
    }
    // Pipe bodies, one per leg.
    for seg in wp.windows(2) {
        parts.push(PipePart::Tube { a: seg[0], b: seg[1], radius });
    }
    // Elbows at the interior corners.
    for p in &wp[1..wp.len() - 1] {
        parts.push(PipePart::Elbow { at: *p, radius: radius * rules.elbow_mult });
    }
    // Fitting collars at the two machine ports (short fat sleeve over the joint).
    let cr = radius * rules.collar_mult;
    let d0 = (wp[1] - wp[0]).normalize_or_zero();
    parts.push(PipePart::Tube { a: wp[0], b: wp[0] + d0 * rules.collar_len, radius: cr });
    let n = wp.len();
    let dn = (wp[n - 2] - wp[n - 1]).normalize_or_zero();
    parts.push(PipePart::Tube { a: wp[n - 1], b: wp[n - 1] + dn * rules.collar_len, radius: cr });
    // Support brackets along each horizontal leg at code spacing.
    for seg in wp.windows(2) {
        let (p, q) = (seg[0], seg[1]);
        let len = (q - p).length();
        if (p.y - q.y).abs() < 1e-3 && len > 0.05 {
            let dir = (q - p) / len;
            let mut s = rules.bracket_spacing * 0.5;
            while s < len {
                parts.push(PipePart::Bracket { at: p + dir * s });
                s += rules.bracket_spacing;
            }
        }
    }
    // Shutoff valve on the destination riser (fluid lines only), a bit up from the inlet.
    if is_fluid {
        let riser_dir = (wp[n - 2] - wp[n - 1]).normalize_or_zero();
        let at = wp[n - 1] + riser_dir * 0.3;
        parts.push(PipePart::Valve { at, axis: riser_dir, radius: radius * 1.6 });
    }
    // Wall sleeve / escutcheon wherever the run pierces a wall (a real plumbing detail:
    // pipes never just pass through a wall, they go through a sleeved penetration).
    for (at, axis) in wall_crossings(&wp, rooms, 0.25) {
        parts.push(PipePart::Penetration { at, axis, radius: radius * 1.6 });
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_is_orthogonal_and_clears_to_band() {
        let a = Vec3::new(0.0, 0.35, 0.0);
        let b = Vec3::new(5.0, 0.35, 3.0);
        let wp = route_orthogonal(a, b, 3.0);
        // up-over-over-down = 5 waypoints (none degenerate here).
        assert_eq!(wp.len(), 5);
        // Every leg moves along exactly one axis (orthogonal, no diagonals).
        for seg in wp.windows(2) {
            let d = seg[1] - seg[0];
            let moved = [d.x.abs() > 1e-4, d.y.abs() > 1e-4, d.z.abs() > 1e-4];
            assert_eq!(moved.iter().filter(|m| **m).count(), 1, "leg {seg:?} is not axis-aligned");
        }
        // The run reaches the band height.
        assert!(wp.iter().any(|p| (p.y - 3.0).abs() < 1e-4));
        // Endpoints preserved.
        assert_eq!(wp[0], a);
        assert_eq!(*wp.last().unwrap(), b);
    }

    #[test]
    fn shared_column_collapses_degenerate_legs() {
        // Same x and z -> only a vertical move; should collapse to 2 points.
        let a = Vec3::new(1.0, 0.0, 2.0);
        let b = Vec3::new(1.0, 1.5, 2.0);
        let wp = route_orthogonal(a, b, 3.0);
        // up to band then back down to b: a, (1,3,2), b  -> 3 points (the two band hops
        // along x/z are degenerate and removed).
        assert_eq!(wp.len(), 3);
        for seg in wp.windows(2) {
            assert!((seg[1] - seg[0]).length() > 1e-4, "no degenerate legs remain");
        }
    }

    #[test]
    fn plan_has_bodies_elbows_collars_and_valve_for_fluid() {
        let rules = RoutingRules::default();
        let a = Vec3::new(0.0, 0.35, 0.0);
        let b = Vec3::new(5.0, 0.35, 3.0);
        let parts = plan_pipe(a, b, 0.028, 3.0, true, &[], &rules);
        let tubes = parts.iter().filter(|p| matches!(p, PipePart::Tube { .. })).count();
        let elbows = parts.iter().filter(|p| matches!(p, PipePart::Elbow { .. })).count();
        let valves = parts.iter().filter(|p| matches!(p, PipePart::Valve { .. })).count();
        // 4 body legs + 2 collars.
        assert_eq!(tubes, 6, "4 legs + 2 port collars");
        // 3 interior corners.
        assert_eq!(elbows, 3);
        // fluid -> exactly one shutoff valve.
        assert_eq!(valves, 1);
        // non-fluid (power) -> no valve.
        let dry = plan_pipe(a, b, 0.022, 3.0, false, &[], &rules);
        assert_eq!(dry.iter().filter(|p| matches!(p, PipePart::Valve { .. })).count(), 0);
    }

    #[test]
    fn wall_penetration_emitted_when_run_leaves_a_room() {
        let rules = RoutingRules::default();
        // Two small rooms with a gap between them along X; the run must exit room A's
        // wall and enter room B's wall -> 2 penetrations.
        let room_a = (Vec3::new(-3.0, 0.0, -3.0), Vec3::new(3.0, 6.0, 3.0));
        let room_b = (Vec3::new(7.0, 0.0, -3.0), Vec3::new(13.0, 6.0, 3.0));
        let rooms = [room_a, room_b];
        let a = Vec3::new(0.0, 0.35, 0.0); // inside room A
        let b = Vec3::new(10.0, 0.35, 0.0); // inside room B
        let parts = plan_pipe(a, b, 0.028, 3.0, false, &rooms, &rules);
        let pens = parts.iter().filter(|p| matches!(p, PipePart::Penetration { .. })).count();
        assert!(pens >= 2, "expected at least 2 wall penetrations (exit A, enter B), got {pens}");
        // A run that stays entirely inside one room pierces no wall.
        let inside = plan_pipe(
            Vec3::new(-2.0, 0.35, -2.0),
            Vec3::new(2.0, 0.35, 2.0),
            0.028,
            3.0,
            false,
            &rooms,
            &rules,
        );
        assert_eq!(
            inside.iter().filter(|p| matches!(p, PipePart::Penetration { .. })).count(),
            0,
            "a same-room run pierces no wall"
        );
    }

    #[test]
    fn run_height_clamps_into_low_room() {
        let rules = RoutingRules::default();
        // A 3 m room: base 3.0 + lane would exceed ceiling; clamp to ceiling - margin.
        let h = rules.run_height("power", 0.0, 3.0);
        assert!(h <= 3.0 - rules.ceiling_margin + 1e-4, "must stay under the ceiling");
        assert!(h >= 1.0, "must clear machines");
    }
}
