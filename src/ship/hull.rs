//! Hull wrap generation (increment D of docs/design/ship-superstructure.md): a generated
//! exterior shell around the ship's zone cluster so the outside finally reads as a VESSEL
//! instead of floating boxes. Purely visual -- NO exterior collision (landing bays / EVA are a
//! later increment) and interiors untouched.
//!
//! The hull is DATA (infinite-of-x): `data/blueprints/hull_profile.ron` (embedded fallback in
//! `embedded_data.rs`, same disk-first philosophy as the planet RONs of v0.763) defines a
//! longitudinal silhouette as stations `(at, half-width scale, height scale)` plus greeble rows
//! (engines, radiators, comm masts). Generation, all headless math:
//!   1. pick the cluster's LONGER horizontal axis as the hull's longitudinal axis (the shipped
//!      two-zone ship is longer in X: home 0..55 + Commons 65..99 = 99 m vs an 89 m Z span);
//!   2. loft flat-shaded plating through the stations around the cluster AABB + `margin`,
//!      CLAMPED so plating never slices a zone box or corridor tube -- wherever a pressurized
//!      box lives the hull stays `margin` outside it and `deck_clearance` above it, so profile
//!      tapers only bite where the boxes leave empty space (nose, stern, empty corners);
//!   3. cut rectangular holes in the top plating over every GLASS zone roof and every glass
//!      corridor lid, so the gardens keep their starlight;
//!   4. plant greebles as primitive blocks (engines mounted on the bow/stern cap protruding
//!      outward at mid height; radiators / masts / unknown kinds standing on the top plating).
//! Deterministic: the same ship + profile always produce byte-identical vertex output (tested).
//!
//! Render wiring lives in `lib.rs`: `rebuild_hull` uploads `generate_hull`'s single plating
//! bucket into one mesh/material slot (`EngineState::homestead_hull`, reused across rebuilds
//! like every homestead slot) and the render loop draws it in the opaque pass, gated by
//! `GuiState::show_hull` (H key + the Settings toggle, mirroring the show_roof pattern).

use crate::renderer::mesh::Vertex;
use crate::ship::ship_structure::{CorridorAxis, ShipStructure};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where the profile lives, relative to the data dir (disk first, embedded fallback).
pub const HULL_PROFILE_REL: &str = "blueprints/hull_profile.ron";

fn default_stations() -> Vec<HullStation> {
    // A plain box wrap: honest fallback when a hand-edited file omits stations.
    vec![
        HullStation { at: 0.0, width: 1.0, height: 1.0 },
        HullStation { at: 1.0, width: 1.0, height: 1.0 },
    ]
}
fn default_margin() -> f32 {
    4.0
}
fn default_deck_clearance() -> f32 {
    2.5
}
fn default_material() -> u32 {
    1
}
fn default_skirt() -> bool {
    true
}
fn default_belly() -> f32 {
    2.0
}

/// One silhouette station. In RON these are compact positional tuples `(at, width, height)`
/// (the design doc's schema); the serde from/into bridge below maps them onto named fields.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "(f32, f32, f32)", into = "(f32, f32, f32)")]
pub struct HullStation {
    /// Position along the hull's long axis as a fraction 0..1 (0 = bow tip, 1 = stern tip).
    pub at: f32,
    /// Half-width scale: 1.0 = the cluster's lateral half-extent + margin.
    pub width: f32,
    /// Hull-top height scale: 1.0 = the tallest zone's top + deck_clearance, from deck level.
    pub height: f32,
}

impl From<(f32, f32, f32)> for HullStation {
    fn from(t: (f32, f32, f32)) -> Self {
        HullStation { at: t.0, width: t.1, height: t.2 }
    }
}

impl From<HullStation> for (f32, f32, f32) {
    fn from(s: HullStation) -> Self {
        (s.at, s.width, s.height)
    }
}

/// One greeble block: a free `kind` label rendered as a simple primitive.
/// "engine" mounts on the stern cap (bow when `at.0 < 0.5`) at mid height, protruding outward;
/// every other kind ("radiator", "mast", anything future) stands on the top plating.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HullGreeble {
    pub kind: String,
    /// (fraction along the long axis 0..1, fraction across 0..1 port -> starboard).
    pub at: (f32, f32),
    /// Size in metres, hull-local: (across the hull, vertical, along the hull).
    pub size: (f32, f32, f32),
}

/// The whole hull profile (`data/blueprints/hull_profile.ron`). Every field except `stations`
/// is serde-defaulted so a minimal hand-written file stays valid; `stations` defaults to a
/// plain box wrap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HullProfile {
    #[serde(default = "default_stations")]
    pub stations: Vec<HullStation>,
    /// Metres of standoff around the cluster AABB (and how far tapers stay off zone walls).
    #[serde(default = "default_margin")]
    pub margin: f32,
    /// The hull top sits this far above the tallest zone (and above every box it passes over).
    #[serde(default = "default_deck_clearance")]
    pub deck_clearance: f32,
    /// Hull plating material id (the wall-material palette zone shells use).
    #[serde(default = "default_material")]
    pub material: u32,
    /// Close the hull below deck level with an underbelly.
    #[serde(default = "default_skirt")]
    pub skirt: bool,
    /// How far below deck level the underbelly sits (only used when `skirt`).
    #[serde(default = "default_belly")]
    pub belly: f32,
    #[serde(default)]
    pub greebles: Vec<HullGreeble>,
}

impl HullProfile {
    /// Structural sanity, run on every load so a hand-edited file fails loudly (and falls back
    /// to the embedded default) instead of producing NaN plating.
    pub fn validate(&self) -> Result<(), String> {
        if self.stations.len() < 2 {
            return Err("hull_profile needs at least 2 stations".to_string());
        }
        let mut prev = -1.0_f32;
        for (i, s) in self.stations.iter().enumerate() {
            if !(0.0..=1.0).contains(&s.at) {
                return Err(format!("station {i}: at {} is outside 0..1", s.at));
            }
            if s.at <= prev {
                return Err(format!("station {i}: at {} is not strictly increasing", s.at));
            }
            prev = s.at;
            if s.width <= 0.0 || s.height <= 0.0 {
                return Err(format!("station {i}: width/height scales must be > 0"));
            }
        }
        if self.margin < 0.0 || self.deck_clearance < 0.0 || self.belly < 0.0 {
            return Err("margin, deck_clearance and belly must be >= 0".to_string());
        }
        for (i, g) in self.greebles.iter().enumerate() {
            if g.size.0 <= 0.0 || g.size.1 <= 0.0 || g.size.2 <= 0.0 {
                return Err(format!("greeble {i} ('{}'): size components must be > 0", g.kind));
            }
            if !(0.0..=1.0).contains(&g.at.0) || !(0.0..=1.0).contains(&g.at.1) {
                return Err(format!("greeble {i} ('{}'): at fractions must be in 0..1", g.kind));
            }
        }
        Ok(())
    }

    /// Load the profile: disk first (modding), embedded fallback. A PRESENT-but-invalid disk
    /// file warns and falls back to the embedded default, so a bad hand edit can never make the
    /// whole hull silently vanish. None only if both copies fail (should be impossible -- the
    /// embedded copy is test-locked).
    pub fn load(data_dir: &Path) -> Option<HullProfile> {
        if let Ok(text) = std::fs::read_to_string(data_dir.join(HULL_PROFILE_REL)) {
            match Self::parse(&text) {
                Ok(p) => return Some(p),
                Err(e) => log::warn!("hull_profile: disk file invalid ({e}); using the embedded default"),
            }
        }
        let text = crate::embedded_data::get_embedded(HULL_PROFILE_REL)?;
        match Self::parse(text) {
            Ok(p) => Some(p),
            Err(e) => {
                log::warn!("hull_profile: embedded default invalid: {e}");
                None
            }
        }
    }

    fn parse(text: &str) -> Result<HullProfile, String> {
        let p: HullProfile = ron::from_str(text).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }
}

/// The world axis the hull runs along (the cluster's longer horizontal extent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HullAxis {
    X,
    Z,
}

/// The hull's coordinate frame: plan coordinates are (long, lat) where `long` is a world
/// coordinate along the long axis and `lat` is the signed offset from the lateral centerline.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HullFrame {
    pub axis: HullAxis,
    /// World span along the long axis, cluster AABB + margin (long0 < long1).
    pub long0: f32,
    pub long1: f32,
    /// World coordinate of the lateral centerline (z when axis = X; x when axis = Z).
    pub lat_center: f32,
    /// Base half-width: the cluster's lateral half-extent + margin (what width scale 1.0 means).
    pub half_width: f32,
    /// Deck level: the cluster AABB's min y.
    pub deck_y: f32,
    /// Base hull top: the tallest zone's top + deck_clearance (what height scale 1.0 means).
    pub top_y: f32,
    /// Bottom of the hull sides (deck_y - belly when skirted, else deck_y).
    pub belly_y: f32,
}

impl HullFrame {
    /// Plan (long, lat, y) -> world position.
    fn world(&self, long: f32, lat: f32, y: f32) -> Vec3 {
        match self.axis {
            HullAxis::X => Vec3::new(long, y, self.lat_center + lat),
            HullAxis::Z => Vec3::new(self.lat_center + lat, y, long),
        }
    }
    fn long_dir(&self) -> Vec3 {
        match self.axis {
            HullAxis::X => Vec3::X,
            HullAxis::Z => Vec3::Z,
        }
    }
    fn lat_dir(&self) -> Vec3 {
        match self.axis {
            HullAxis::X => Vec3::Z,
            HullAxis::Z => Vec3::X,
        }
    }
}

/// A rectangle in hull plan coordinates (glass cutouts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlanRect {
    pub long0: f32,
    pub long1: f32,
    pub lat0: f32,
    pub lat1: f32,
}

/// A pressurized box (zone or corridor tube) in plan coordinates: the loft clamp set. The hull
/// must stay `margin` outside its lateral extent and `deck_clearance` above its top across its
/// whole longitudinal span.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PlanBox {
    long0: f32,
    long1: f32,
    lat0: f32,
    lat1: f32,
    top: f32,
}

/// A profile station RESOLVED to world numbers: position along the long axis, effective
/// half-width, effective hull-top height (both already clamped by the boxes at that position).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedStation {
    pub pos: f32,
    pub hw: f32,
    pub top: f32,
}

/// The fully resolved hull: frame + clamped stations + glass cutouts. The single source both
/// mesh emission and the tests read, so they can never disagree about where the hull is
/// (the `corridor_geometry` discipline from increment B).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HullGeom {
    pub frame: HullFrame,
    /// Sorted along the long axis. Includes every profile station AND every box's longitudinal
    /// boundaries, so linear interpolation between two clamped stations can never dip inside a
    /// box mid-segment.
    pub stations: Vec<ResolvedStation>,
    pub holes: Vec<PlanRect>,
}

impl HullGeom {
    /// Effective half-width at a longitudinal position (clamped piecewise-linear).
    pub(crate) fn hw_at(&self, pos: f32) -> f32 {
        self.sample(pos).0
    }
    /// Effective hull-top height at a longitudinal position.
    pub(crate) fn top_at(&self, pos: f32) -> f32 {
        self.sample(pos).1
    }
    fn sample(&self, pos: f32) -> (f32, f32) {
        let sts = &self.stations;
        let Some(first) = sts.first() else { return (0.0, 0.0) };
        if pos <= first.pos {
            return (first.hw, first.top);
        }
        for w in sts.windows(2) {
            if pos <= w[1].pos {
                let span = (w[1].pos - w[0].pos).max(1e-6);
                let t = (pos - w[0].pos) / span;
                return (w[0].hw + t * (w[1].hw - w[0].hw), w[0].top + t * (w[1].top - w[0].top));
            }
        }
        let last = sts.last().expect("non-empty checked above");
        (last.hw, last.top)
    }
}

/// Piecewise-linear (width, height) scales at a 0..1 fraction, clamped to the end stations.
fn profile_scales(sts: &[HullStation], frac: f32) -> (f32, f32) {
    let Some(first) = sts.first() else { return (1.0, 1.0) };
    if frac <= first.at {
        return (first.width, first.height);
    }
    for w in sts.windows(2) {
        if frac <= w[1].at {
            let span = (w[1].at - w[0].at).max(1e-6);
            let t = (frac - w[0].at) / span;
            return (
                w[0].width + t * (w[1].width - w[0].width),
                w[0].height + t * (w[1].height - w[0].height),
            );
        }
    }
    let last = sts.last().expect("non-empty checked above");
    (last.width, last.height)
}

/// Resolve a ship + profile to hull geometry. None when the ship has no zones.
pub(crate) fn hull_geom(ship: &ShipStructure, profile: &HullProfile) -> Option<HullGeom> {
    if ship.zones.is_empty() {
        return None;
    }
    let (mn, mx) = ship.world_bounds();
    // The LONGER horizontal extent is the longitudinal axis (ties go to X).
    let axis = if (mx.x - mn.x) >= (mx.z - mn.z) { HullAxis::X } else { HullAxis::Z };
    let (long_min, long_max, lat_min, lat_max) = match axis {
        HullAxis::X => (mn.x, mx.x, mn.z, mx.z),
        HullAxis::Z => (mn.z, mx.z, mn.x, mx.x),
    };
    let frame = HullFrame {
        axis,
        long0: long_min - profile.margin,
        long1: long_max + profile.margin,
        lat_center: (lat_min + lat_max) * 0.5,
        half_width: (lat_max - lat_min) * 0.5 + profile.margin,
        deck_y: mn.y,
        top_y: mx.y + profile.deck_clearance,
        belly_y: mn.y - if profile.skirt { profile.belly.max(0.0) } else { 0.0 },
    };
    // World XZ rect -> plan (long0, long1, lat0, lat1).
    let to_plan = |x0: f32, z0: f32, x1: f32, z1: f32| -> (f32, f32, f32, f32) {
        match axis {
            HullAxis::X => (x0, x1, z0 - frame.lat_center, z1 - frame.lat_center),
            HullAxis::Z => (z0, z1, x0 - frame.lat_center, x1 - frame.lat_center),
        }
    };
    // The clamp set (every pressurized box) + the cutout set (glass tops only).
    let mut boxes: Vec<PlanBox> = Vec::new();
    let mut holes: Vec<PlanRect> = Vec::new();
    for z in &ship.zones {
        let o = z.origin_vec();
        let (l0, l1, t0, t1) = to_plan(o.x, o.z, o.x + z.body.width, o.z + z.body.depth);
        boxes.push(PlanBox { long0: l0, long1: l1, lat0: t0, lat1: t1, top: o.y + z.body.height });
        if z.body.roof_is_glass() {
            holes.push(PlanRect { long0: l0, long1: l1, lat0: t0, lat1: t1 });
        }
    }
    for c in &ship.corridors {
        // Broken rows generate no tube (increment B skips them the same way), so no clamp/hole.
        let Ok(g) = ship.corridor_geometry(c) else { continue };
        let hwc = g.width * 0.5;
        let (x0, z0, x1, z1) = match g.axis {
            CorridorAxis::X => (g.start, g.lat - hwc, g.end, g.lat + hwc),
            CorridorAxis::Z => (g.lat - hwc, g.start, g.lat + hwc, g.end),
        };
        let (l0, l1, t0, t1) = to_plan(x0, z0, x1, z1);
        boxes.push(PlanBox { long0: l0, long1: l1, lat0: t0, lat1: t1, top: g.floor_y + g.height });
        if c.glass_top {
            holes.push(PlanRect { long0: l0, long1: l1, lat0: t0, lat1: t1 });
        }
    }
    // Station positions: the profile stations + every box's longitudinal boundaries. Inserting
    // the boundaries makes the per-station clamp EXACT across each box's whole span: inside a
    // segment the overlapping-box set is constant, and lerp between two endpoints that are both
    // clamped >= a box's requirement stays >= it. No steps, no plating through a living room.
    let l = frame.long1 - frame.long0;
    let mut positions: Vec<f32> = profile
        .stations
        .iter()
        .map(|s| frame.long0 + s.at.clamp(0.0, 1.0) * l)
        .collect();
    for b in &boxes {
        positions.push(b.long0);
        positions.push(b.long1);
    }
    positions.sort_by(|a, b| a.partial_cmp(b).expect("finite station positions"));
    positions.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let stations = positions
        .iter()
        .map(|&pos| {
            let frac = if l > 1e-6 { (pos - frame.long0) / l } else { 0.0 };
            let (ws, hs) = profile_scales(&profile.stations, frac);
            let mut hw = (ws * frame.half_width).max(0.05);
            let mut top = frame.deck_y + hs * (frame.top_y - frame.deck_y);
            for b in &boxes {
                if pos >= b.long0 - 1e-4 && pos <= b.long1 + 1e-4 {
                    hw = hw.max(b.lat0.abs().max(b.lat1.abs()) + profile.margin);
                    top = top.max(b.top + profile.deck_clearance);
                }
            }
            ResolvedStation { pos, hw, top }
        })
        .collect();
    Some(HullGeom { frame, stations, holes })
}

/// A greeble resolved to a world-axis-aligned box.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GreeblePlacement {
    pub kind: String,
    pub center: Vec3,
    pub half: Vec3,
}

/// Resolve every greeble row to its box. Split from emission so the tests can assert placement
/// without parsing triangles.
pub(crate) fn greeble_placements(geom: &HullGeom, profile: &HullProfile) -> Vec<GreeblePlacement> {
    let f = &geom.frame;
    let l = f.long1 - f.long0;
    profile
        .greebles
        .iter()
        .map(|g| {
            let p = f.long0 + g.at.0.clamp(0.0, 1.0) * l;
            let hw = geom.hw_at(p);
            let top = geom.top_at(p);
            let lat = -hw + g.at.1.clamp(0.0, 1.0) * (2.0 * hw);
            // size is hull-local (across, vertical, along) -> world half extents per axis.
            let (ha, hv, hl) = (g.size.0 * 0.5, g.size.1 * 0.5, g.size.2 * 0.5);
            let half = match f.axis {
                HullAxis::X => Vec3::new(hl, hv, ha),
                HullAxis::Z => Vec3::new(ha, hv, hl),
            };
            let center = if g.kind == "engine" {
                // Cap-mounted at mid height, protruding outward (aft when along >= 0.5).
                let dir = if g.at.0 >= 0.5 { 1.0 } else { -1.0 };
                f.world(p + dir * hl, lat, (f.belly_y + top) * 0.5)
            } else {
                // Standing on the top plating: radiator fins, comm masts, any future kind.
                f.world(p, lat, top + hv)
            };
            GreeblePlacement { kind: g.kind.clone(), center, half }
        })
        .collect()
}

/// Result bucket: all hull plating + greeble blocks, one flat-shaded mesh in the hull material.
#[derive(Default)]
pub struct HullMeshes {
    pub plating: (Vec<Vertex>, Vec<u32>),
}

/// UVs in metres from the dominant plane of the face (the world-scaled convention the
/// homestead's floor_quad uses, so plating texture density matches the zone shells).
fn uv_for(p: Vec3, n: Vec3) -> [f32; 2] {
    let a = n.abs();
    if a.y >= a.x && a.y >= a.z {
        [p.x, p.z]
    } else if a.x >= a.z {
        [p.z, p.y]
    } else {
        [p.x, p.y]
    }
}

/// Push one flat-shaded quad whose outward side faces `hint` (winding is corrected to match, so
/// callers never fight the back-face cull). Degenerate quads are skipped.
fn push_quad_facing(out: &mut (Vec<Vertex>, Vec<u32>), p: [Vec3; 4], hint: Vec3) {
    let mut p = p;
    let mut n = (p[1] - p[0]).cross(p[2] - p[0]);
    if n.length_squared() < 1e-10 {
        n = (p[2] - p[0]).cross(p[3] - p[0]);
    }
    if n.length_squared() < 1e-10 {
        return; // degenerate sliver
    }
    if n.dot(hint) < 0.0 {
        p.swap(1, 3);
        n = -n;
    }
    let n = n.normalize();
    let base = out.0.len() as u32;
    for q in p {
        out.0.push(Vertex { position: [q.x, q.y, q.z], normal: [n.x, n.y, n.z], uv: uv_for(q, n) });
    }
    out.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Plating is visible from BOTH sides (the opaque pass back-face culls, and the player looks up
/// at the hull's inside through their glass roof), so every panel is a front + back pair.
fn push_quad_double(out: &mut (Vec<Vertex>, Vec<u32>), p: [Vec3; 4], hint: Vec3) {
    push_quad_facing(out, p, hint);
    push_quad_facing(out, p, -hint);
}

/// A closed world-axis-aligned box (greebles): 6 outward single-sided faces.
fn push_box(out: &mut (Vec<Vertex>, Vec<u32>), c: Vec3, h: Vec3) {
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::X, Vec3::Y),
    ];
    for (n, u, v) in faces {
        let hn = n * n.abs().dot(h);
        let hu = u * u.dot(h);
        let hv = v * v.dot(h);
        push_quad_facing(out, [c + hn - hu - hv, c + hn + hu - hv, c + hn + hu + hv, c + hn - hu + hv], n);
    }
}

/// Lateral bound of a top-plating cell: the tapering hull edge, or a fixed cut from a hole edge.
#[derive(Clone, Copy)]
enum LatBound {
    Edge(f32), // -1.0 = port edge, +1.0 = starboard edge (follows the local half-width)
    At(f32),
}

fn eval_bound(b: LatBound, hw: f32) -> f32 {
    match b {
        LatBound::Edge(s) => s * hw,
        LatBound::At(c) => c,
    }
}

/// Top plating for one segment, with rectangular holes over the glass rects. The segment's plan
/// rectangle subdivides into a grid whose lines are the hole edges; every cell whose center sits
/// inside a hole is skipped. Because holes are pressurized-box tops and the loft is clamped
/// `margin` outside every box, hole edges are always strictly inside the tapering hull edge, so
/// interior cells are pure rectangles and only the two edge strips follow the taper.
fn emit_top_cells(out: &mut (Vec<Vertex>, Vec<u32>), geom: &HullGeom, si: &ResolvedStation, sj: &ResolvedStation) {
    let f = &geom.frame;
    let seg = sj.pos - si.pos;
    let holes: Vec<&PlanRect> = geom
        .holes
        .iter()
        .filter(|h| h.long0 < sj.pos - 1e-4 && h.long1 > si.pos + 1e-4)
        .collect();
    let hw = |p: f32| si.hw + (p - si.pos) / seg * (sj.hw - si.hw);
    let top = |p: f32| si.top + (p - si.pos) / seg * (sj.top - si.top);
    // Longitudinal grid lines: the segment ends + every hole edge inside it.
    let mut lcuts = vec![si.pos, sj.pos];
    for h in &holes {
        lcuts.push(h.long0.clamp(si.pos, sj.pos));
        lcuts.push(h.long1.clamp(si.pos, sj.pos));
    }
    lcuts.sort_by(|a, b| a.partial_cmp(b).expect("finite cut positions"));
    lcuts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    // Lateral grid lines: every overlapping hole's lat edges (filtered per strip below).
    let mut tcuts: Vec<f32> = Vec::new();
    for h in &holes {
        tcuts.push(h.lat0);
        tcuts.push(h.lat1);
    }
    tcuts.sort_by(|a, b| a.partial_cmp(b).expect("finite cut positions"));
    tcuts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    for lw in lcuts.windows(2) {
        let (pa, pb) = (lw[0], lw[1]);
        if pb - pa < 1e-4 {
            continue;
        }
        let (hw_a, hw_b) = (hw(pa), hw(pb));
        let hw_min = hw_a.min(hw_b);
        // Keep only cuts strictly inside the hull edge across this strip. (A hole that reaches
        // the very edge -- only possible at margin ~0 -- just removes the whole edge strip.)
        let cuts: Vec<f32> = tcuts.iter().copied().filter(|c| c.abs() < hw_min - 1e-3).collect();
        let mut bounds: Vec<LatBound> = Vec::with_capacity(cuts.len() + 2);
        bounds.push(LatBound::Edge(-1.0));
        bounds.extend(cuts.into_iter().map(LatBound::At));
        bounds.push(LatBound::Edge(1.0));
        let pm = (pa + pb) * 0.5;
        let hw_m = hw(pm);
        let (ta, tb) = (top(pa), top(pb));
        for bw in bounds.windows(2) {
            let (lo, hi) = (bw[0], bw[1]);
            let (lo_m, hi_m) = (eval_bound(lo, hw_m), eval_bound(hi, hw_m));
            if hi_m - lo_m < 1e-4 {
                continue;
            }
            // Cell-center hole test: grid lines include every hole edge, so a cell is either
            // fully inside a hole or fully outside -- the center decides.
            let lat_c = (lo_m + hi_m) * 0.5;
            if holes.iter().any(|h| pm > h.long0 && pm < h.long1 && lat_c > h.lat0 && lat_c < h.lat1) {
                continue;
            }
            let q = [
                f.world(pa, eval_bound(lo, hw_a), ta),
                f.world(pb, eval_bound(lo, hw_b), tb),
                f.world(pb, eval_bound(hi, hw_b), tb),
                f.world(pa, eval_bound(hi, hw_a), ta),
            ];
            push_quad_double(out, q, Vec3::Y);
        }
    }
}

/// Generate the whole hull: lofted top (with glass cutouts) + sides + optional underbelly +
/// bow/stern caps + greeble blocks. One bucket, one material. Deterministic, no randomness.
pub fn generate_hull(ship: &ShipStructure, profile: &HullProfile) -> HullMeshes {
    let Some(geom) = hull_geom(ship, profile) else {
        return HullMeshes::default();
    };
    let f = geom.frame.clone();
    let mut out: (Vec<Vertex>, Vec<u32>) = (Vec::new(), Vec::new());
    let lat = f.lat_dir();
    for w in geom.stations.windows(2) {
        let (si, sj) = (&w[0], &w[1]);
        if sj.pos - si.pos < 1e-4 {
            continue;
        }
        emit_top_cells(&mut out, &geom, si, sj);
        // Side plating: port (lat = -hw) and starboard (lat = +hw), belly to the lofted top.
        let port = [
            f.world(si.pos, -si.hw, f.belly_y),
            f.world(sj.pos, -sj.hw, f.belly_y),
            f.world(sj.pos, -sj.hw, sj.top),
            f.world(si.pos, -si.hw, si.top),
        ];
        push_quad_double(&mut out, port, -lat);
        let starboard = [
            f.world(si.pos, si.hw, f.belly_y),
            f.world(sj.pos, sj.hw, f.belly_y),
            f.world(sj.pos, sj.hw, sj.top),
            f.world(si.pos, si.hw, si.top),
        ];
        push_quad_double(&mut out, starboard, lat);
        if profile.skirt {
            let belly = [
                f.world(si.pos, -si.hw, f.belly_y),
                f.world(si.pos, si.hw, f.belly_y),
                f.world(sj.pos, sj.hw, f.belly_y),
                f.world(sj.pos, -sj.hw, f.belly_y),
            ];
            push_quad_double(&mut out, belly, Vec3::NEG_Y);
        }
    }
    // Bow + stern caps close the loft ends.
    let ld = f.long_dir();
    if let (Some(s0), Some(sn)) = (geom.stations.first(), geom.stations.last()) {
        if s0.hw > 0.01 {
            let bow = [
                f.world(s0.pos, -s0.hw, f.belly_y),
                f.world(s0.pos, -s0.hw, s0.top),
                f.world(s0.pos, s0.hw, s0.top),
                f.world(s0.pos, s0.hw, f.belly_y),
            ];
            push_quad_double(&mut out, bow, -ld);
        }
        if sn.hw > 0.01 {
            let stern = [
                f.world(sn.pos, -sn.hw, f.belly_y),
                f.world(sn.pos, -sn.hw, sn.top),
                f.world(sn.pos, sn.hw, sn.top),
                f.world(sn.pos, sn.hw, f.belly_y),
            ];
            push_quad_double(&mut out, stern, ld);
        }
    }
    for g in greeble_placements(&geom, profile) {
        push_box(&mut out, g.center, g.half);
    }
    HullMeshes { plating: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::home_structure::HomeStructure;
    use crate::ship::ship_structure::{ShipCorridor, ShipZone};

    fn body(w: f32, d: f32, h: f32) -> HomeStructure {
        ron::from_str::<HomeStructure>(&format!("(width: {w}, depth: {d}, height: {h})"))
            .expect("body literal parses")
    }

    fn zone(id: &str, origin: (f32, f32, f32), w: f32, d: f32, h: f32) -> ShipZone {
        ShipZone {
            id: id.to_string(),
            label: id.to_string(),
            purpose: "residence".to_string(),
            origin,
            body: body(w, d, h),
        }
    }

    /// The shipped two-zone cluster shape: home 55x89x3 at the origin, commons 34x55x8 at
    /// (65, 0, 20). Long axis = X (99 m against 89 m).
    fn two_zone_ship() -> ShipStructure {
        ShipStructure {
            zones: vec![
                zone("home", (0.0, 0.0, 0.0), 55.0, 89.0, 3.0),
                zone("commons", (65.0, 0.0, 20.0), 34.0, 55.0, 8.0),
            ],
            corridors: Vec::new(),
        }
    }

    /// A zone body with one door on a wall along x = `wall_x` (the corridor-fixture pattern
    /// from ship_structure's tests).
    fn body_with_door(w: f32, d: f32, h: f32, wall_x: f32, z1: f32, z2: f32, door_z: f32, door_w: f32) -> HomeStructure {
        let mut b = body(w, d, h);
        let at = door_z - z1 - door_w * 0.5;
        b.walls = vec![ron::from_str(&format!(
            "(a: ({wall_x}, {z1}), b: ({wall_x}, {z2}), height: {h}, material: 1, openings: [\
             (kind: Door, at: {at}, width: {door_w}, sill: 0.0, height: 2.1, style: \"swing\", \
             open_dist: 2.6, locked: false, auto_open: true, control_panel: false, locks: [])])"
        ))
        .expect("door wall literal parses")];
        b
    }

    /// Two small zones joined by a glass-topped corridor (doors face along X at world z = 5).
    fn corridor_ship() -> ShipStructure {
        ShipStructure {
            zones: vec![
                ShipZone {
                    id: "home".to_string(),
                    label: "Home".to_string(),
                    purpose: "residence".to_string(),
                    origin: (0.0, 0.0, 0.0),
                    body: body_with_door(10.0, 10.0, 3.0, 10.0, 3.0, 7.0, 5.0, 1.0),
                },
                ShipZone {
                    id: "commons".to_string(),
                    label: "Commons".to_string(),
                    purpose: "commons".to_string(),
                    origin: (20.0, 0.0, 2.0),
                    body: body_with_door(8.0, 8.0, 6.0, 0.0, 1.0, 5.0, 3.0, 1.0),
                },
            ],
            corridors: vec![ShipCorridor {
                from_zone: "home".to_string(),
                from_opening: 0,
                to_zone: "commons".to_string(),
                to_opening: 0,
                width: 3.0,
                glass_top: true,
            }],
        }
    }

    /// A tapered test profile (aggressive nose/stern so the clamp actually has work to do).
    fn test_profile() -> HullProfile {
        HullProfile {
            stations: vec![
                HullStation { at: 0.0, width: 0.2, height: 0.3 },
                HullStation { at: 0.5, width: 1.0, height: 1.0 },
                HullStation { at: 1.0, width: 0.5, height: 0.6 },
            ],
            margin: 4.0,
            deck_clearance: 2.5,
            material: 1,
            skirt: true,
            belly: 2.0,
            greebles: vec![
                HullGreeble { kind: "engine".to_string(), at: (1.0, 0.35), size: (5.0, 5.0, 7.0) },
                HullGreeble { kind: "mast".to_string(), at: (0.6, 0.5), size: (0.8, 9.0, 0.8) },
                HullGreeble { kind: "radiator".to_string(), at: (0.4, 0.02), size: (0.5, 3.0, 8.0) },
            ],
        }
    }

    fn shipped_profile_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("hull_profile.ron")
    }

    // ── Data plumbing ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parses_the_shipped_hull_profile() {
        let text = std::fs::read_to_string(shipped_profile_path()).expect("hull_profile.ron exists");
        let p = HullProfile::parse(&text).expect("shipped hull_profile.ron parses + validates");
        assert!(p.stations.len() >= 2);
        assert!(p.margin > 0.0, "the shipped default keeps a real standoff");
        assert!(!p.greebles.is_empty(), "the shipped default ships greebles");
    }

    #[test]
    fn embedded_hull_profile_parses_and_matches_the_shipped_file() {
        let embedded = crate::embedded_data::get_embedded(HULL_PROFILE_REL)
            .expect("hull profile has an embedded fallback");
        let p = HullProfile::parse(embedded).expect("embedded hull profile parses + validates");
        assert!(p.validate().is_ok());
        // include_str! locks these together at compile time; assert it anyway so a future
        // path-shuffle that breaks the pairing fails loudly here.
        let disk = std::fs::read_to_string(shipped_profile_path()).expect("shipped file exists");
        assert_eq!(embedded, disk, "embedded copy is byte-identical to the shipped file");
    }

    #[test]
    fn hull_profile_validation_rejects_bad_data() {
        let mut p = test_profile();
        p.stations.truncate(1);
        assert!(p.validate().unwrap_err().contains("at least 2"), "one station is not a loft");
        let mut p = test_profile();
        p.stations[1].at = 0.0; // duplicates station 0's position
        assert!(p.validate().unwrap_err().contains("strictly increasing"));
        let mut p = test_profile();
        p.stations[2].at = 1.5;
        assert!(p.validate().unwrap_err().contains("outside 0..1"));
        let mut p = test_profile();
        p.stations[1].width = 0.0;
        assert!(p.validate().unwrap_err().contains("> 0"));
        let mut p = test_profile();
        p.greebles[0].size.1 = 0.0;
        assert!(p.validate().unwrap_err().contains("size components"));
        let mut p = test_profile();
        p.greebles[0].at.0 = 2.0;
        assert!(p.validate().unwrap_err().contains("fractions"));
        let mut p = test_profile();
        p.margin = -1.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn a_minimal_profile_parses_via_serde_defaults() {
        // Everything defaulted: a bare "()" is a valid (boxy) profile.
        let p = HullProfile::parse("()").expect("all-default profile parses");
        assert_eq!(p.stations.len(), 2, "defaults to a plain box wrap");
        assert!(p.skirt);
        assert!(p.greebles.is_empty());
    }

    // ── Loft coverage ──────────────────────────────────────────────────────────────────────

    #[test]
    fn loft_covers_every_zone_with_margin_at_sampled_stations() {
        let ship = two_zone_ship();
        let profile = test_profile();
        let geom = hull_geom(&ship, &profile).expect("geometry resolves");
        // Long axis is X for this cluster (99 m > 89 m) and spans AABB + margin.
        assert_eq!(geom.frame.axis, HullAxis::X);
        assert!((geom.frame.long0 - -4.0).abs() < 1e-4);
        assert!((geom.frame.long1 - 103.0).abs() < 1e-4);
        // Sample every resolved station AND every segment midpoint: wherever a zone lives, the
        // hull is at least `margin` outside it laterally and `deck_clearance` above it.
        let mut samples: Vec<f32> = geom.stations.iter().map(|s| s.pos).collect();
        for w in geom.stations.windows(2) {
            samples.push((w[0].pos + w[1].pos) * 0.5);
        }
        let c = geom.frame.lat_center;
        for z in &ship.zones {
            let o = z.origin_vec();
            let (l0, l1) = (o.x, o.x + z.body.width);
            let req_hw = (o.z - c).abs().max((o.z + z.body.depth - c).abs()) + profile.margin;
            let req_top = o.y + z.body.height + profile.deck_clearance;
            for &p in &samples {
                if p >= l0 - 1e-4 && p <= l1 + 1e-4 {
                    assert!(
                        geom.hw_at(p) >= req_hw - 1e-3,
                        "zone '{}' at long {p}: hull hw {} < required {req_hw}",
                        z.id,
                        geom.hw_at(p)
                    );
                    assert!(
                        geom.top_at(p) >= req_top - 1e-3,
                        "zone '{}' at long {p}: hull top {} < required {req_top}",
                        z.id,
                        geom.top_at(p)
                    );
                }
            }
        }
        // Off the cluster's ends the profile taper actually applies (the clamp releases).
        let nose_hw = geom.hw_at(geom.frame.long0);
        assert!(nose_hw < geom.frame.half_width * 0.5, "the nose tapers, got {nose_hw}");
    }

    #[test]
    fn axis_flips_to_z_for_a_z_long_cluster() {
        let ship = ShipStructure {
            zones: vec![zone("home", (0.0, 0.0, 0.0), 20.0, 120.0, 3.0)],
            corridors: Vec::new(),
        };
        let geom = hull_geom(&ship, &test_profile()).expect("resolves");
        assert_eq!(geom.frame.axis, HullAxis::Z);
        assert!((geom.frame.long1 - 124.0).abs() < 1e-4, "long axis spans depth + margin");
        assert!((geom.frame.half_width - 14.0).abs() < 1e-4, "lateral = width/2 + margin");
    }

    // ── Glass cutouts ──────────────────────────────────────────────────────────────────────

    /// Collect the plan rectangles of every quad that could BLOCK starlight from above: upward-
    /// facing plating at or above deck level. Excluded: the doubled interior copies (face down),
    /// the underbelly's upward inner face (below deck -- under the zone floors, it can't shadow
    /// a roof), and greeble-box tops (above the lofted top). Quads are emitted 4 verts at a time.
    fn top_panel_rects(meshes: &HullMeshes, geom: &HullGeom) -> Vec<PlanRect> {
        let c = geom.frame.lat_center;
        let (verts, _) = &meshes.plating;
        let mut rects = Vec::new();
        for q in verts.chunks(4) {
            if q.len() < 4 || q[0].normal[1] < 0.9 {
                continue;
            }
            let (mut l0, mut l1, mut t0, mut t1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            let mut max_y = f32::MIN;
            for v in q {
                let (long, lat) = match geom.frame.axis {
                    HullAxis::X => (v.position[0], v.position[2] - c),
                    HullAxis::Z => (v.position[2], v.position[0] - c),
                };
                l0 = l0.min(long);
                l1 = l1.max(long);
                t0 = t0.min(lat);
                t1 = t1.max(lat);
                max_y = max_y.max(v.position[1]);
            }
            if max_y <= geom.frame.deck_y + 1e-3 {
                continue; // the underbelly's inner face -- below every zone floor
            }
            let mid = (l0 + l1) * 0.5;
            if max_y > geom.top_at(mid) + 1e-3 {
                continue; // a greeble top face, not hull plating
            }
            rects.push(PlanRect { long0: l0, long1: l1, lat0: t0, lat1: t1 });
        }
        rects
    }

    #[test]
    fn glass_roofs_and_corridor_lids_cut_holes_in_the_top_plating() {
        let ship = corridor_ship(); // both zones default to GLASS roofs + a glass corridor lid
        let mut profile = test_profile();
        profile.greebles.clear(); // plating only, so every upward quad is a top panel
        let geom = hull_geom(&ship, &profile).expect("resolves");
        assert_eq!(geom.holes.len(), 3, "two glass roofs + one glass corridor lid");
        let meshes = generate_hull(&ship, &profile);
        let panels = top_panel_rects(&meshes, &geom);
        assert!(!panels.is_empty(), "the margin ring still has top plating");
        // 1. No top panel overlaps any glass rect (shared edges allowed, interior area not).
        for panel in &panels {
            for hole in &geom.holes {
                let ol = (panel.long1.min(hole.long1) - panel.long0.max(hole.long0)).max(0.0);
                let ot = (panel.lat1.min(hole.lat1) - panel.lat0.max(hole.lat0)).max(0.0);
                assert!(
                    ol * ot < 1e-3,
                    "top panel ({},{})..({},{}) covers glass rect ({},{})..({},{})",
                    panel.long0, panel.lat0, panel.long1, panel.lat1,
                    hole.long0, hole.lat0, hole.long1, hole.lat1
                );
            }
        }
        // 2. Every glass rect is genuinely open: its center is covered by NO top panel.
        for hole in &geom.holes {
            let (cl, ct) = ((hole.long0 + hole.long1) * 0.5, (hole.lat0 + hole.lat1) * 0.5);
            assert!(
                !panels.iter().any(|p| cl > p.long0 && cl < p.long1 && ct > p.lat0 && ct < p.lat1),
                "glass rect center ({cl},{ct}) is roofed over"
            );
        }
    }

    #[test]
    fn an_opaque_roof_zone_gets_no_cutout() {
        let mut ship = corridor_ship();
        ship.zones[0].body.roof_material = 1; // steel roof
        ship.corridors[0].glass_top = false;
        let geom = hull_geom(&ship, &test_profile()).expect("resolves");
        assert_eq!(geom.holes.len(), 1, "only the remaining glass zone cuts a hole");
    }

    // ── Greebles ───────────────────────────────────────────────────────────────────────────

    #[test]
    fn greeble_placement_lands_on_the_hull_extents() {
        let ship = two_zone_ship();
        let profile = test_profile();
        let geom = hull_geom(&ship, &profile).expect("resolves");
        let placed = greeble_placements(&geom, &profile);
        assert_eq!(placed.len(), 3);
        let f = &geom.frame;
        // Engine (at along = 1.0): butted against the stern cap, protruding aft, mid height.
        let engine = &placed[0];
        assert_eq!(engine.kind, "engine");
        assert!((engine.center.x - (f.long1 + 3.5)).abs() < 1e-3, "engine protrudes half its length aft");
        let stern_top = geom.top_at(f.long1);
        assert!((engine.center.y - (f.belly_y + stern_top) * 0.5).abs() < 1e-3, "engine at cap mid height");
        // Mast: standing ON the top plating (base exactly at the lofted top).
        let mast = &placed[1];
        let p = f.long0 + 0.6 * (f.long1 - f.long0);
        assert!((mast.center.y - (geom.top_at(p) + 4.5)).abs() < 1e-3, "mast base sits on the hull top");
        assert!((mast.center.x - p).abs() < 1e-3);
        // Every greeble stays within the hull's lateral extents at its station.
        for g in &placed {
            let (long, lat) = (g.center.x, g.center.z - f.lat_center);
            let hw = geom.hw_at(long.clamp(f.long0, f.long1));
            assert!(lat.abs() <= hw + 1e-3, "greeble '{}' off the hull laterally", g.kind);
        }
    }

    // ── Determinism + growth ───────────────────────────────────────────────────────────────

    #[test]
    fn hull_generation_is_deterministic() {
        let ship = corridor_ship();
        let profile = test_profile();
        let a = generate_hull(&ship, &profile);
        let b = generate_hull(&ship, &profile);
        assert_eq!(a.plating.1, b.plating.1, "identical indices");
        assert_eq!(a.plating.0.len(), b.plating.0.len());
        let bits = |vs: &[Vertex]| -> Vec<u32> {
            vs.iter()
                .flat_map(|v| {
                    v.position
                        .iter()
                        .chain(v.normal.iter())
                        .map(|f| f.to_bits())
                        .chain(v.uv.iter().map(|f| f.to_bits()))
                        .collect::<Vec<u32>>()
                })
                .collect()
        };
        assert_eq!(bits(&a.plating.0), bits(&b.plating.0), "bit-identical vertex output");
    }

    #[test]
    fn adding_a_zone_grows_the_hull() {
        let mut ship = two_zone_ship();
        let profile = test_profile();
        let before = hull_geom(&ship, &profile).expect("resolves");
        ship.zones.push(zone("bay", (120.0, 0.0, 30.0), 10.0, 10.0, 4.0));
        let after = hull_geom(&ship, &profile).expect("resolves");
        assert!((after.frame.long1 - 134.0).abs() < 1e-4, "hull stern moved out to the new zone + margin");
        assert!(after.frame.long1 > before.frame.long1 + 20.0);
        // And the plating itself reaches the new extent.
        let meshes = generate_hull(&ship, &profile);
        let max_x = meshes.plating.0.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        assert!(max_x >= 134.0 - 1e-3, "plating spans the grown cluster, got {max_x}");
        // The new zone is covered with margin at its own span.
        let req = (30.0_f32 - after.frame.lat_center).abs().max((40.0 - after.frame.lat_center).abs()) + profile.margin;
        assert!(after.hw_at(125.0) >= req - 1e-3);
        assert!(after.top_at(125.0) >= 4.0 + profile.deck_clearance - 1e-3);
    }

    // ── Shipped-data integration ───────────────────────────────────────────────────────────

    #[test]
    fn shipped_hull_profile_wraps_the_shipped_ship() {
        let ship_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("ship_structure.ron");
        let ship = ShipStructure::load(&ship_path).expect("shipped ship_structure.ron loads");
        let text = std::fs::read_to_string(shipped_profile_path()).expect("shipped profile exists");
        let profile = HullProfile::parse(&text).expect("shipped profile parses");
        let geom = hull_geom(&ship, &profile).expect("resolves around the shipped cluster");
        // The shipped cluster is longer in X (home 55 wide + commons out to x = 99, vs 89 deep).
        assert_eq!(geom.frame.axis, HullAxis::X);
        // Both glass roofs + the glass corridor lid cut holes.
        assert_eq!(geom.holes.len(), 3, "home roof + commons roof + corridor lid are open");
        // Coverage: every zone stays margin-inside the hull at its own span.
        let c = geom.frame.lat_center;
        for z in &ship.zones {
            let o = z.origin_vec();
            let req_hw = (o.z - c).abs().max((o.z + z.body.depth - c).abs()) + profile.margin;
            let req_top = o.y + z.body.height + profile.deck_clearance;
            for p in [o.x, o.x + z.body.width * 0.5, o.x + z.body.width] {
                assert!(geom.hw_at(p) >= req_hw - 1e-3, "zone '{}' sliced at long {p}", z.id);
                assert!(geom.top_at(p) >= req_top - 1e-3, "zone '{}' scalped at long {p}", z.id);
            }
        }
        // And the mesh is real.
        let meshes = generate_hull(&ship, &profile);
        assert!(!meshes.plating.0.is_empty());
        assert_eq!(meshes.plating.1.len() % 3, 0, "triangle list");
    }
}
