//! THE TRIM SWEPT ALONG EVERY ROOM'S WALLS (room finishes rung 1, docs/design/room-finishes.md):
//! a skirting at the foot of each wall face and a crown where the wall meets the ceiling, each a
//! profile from data/blueprints/room_finishes.ron swept along the face, MITRED where two faces meet
//! (inside and outside corners alike), stopped square at a door (and, for the crown, at an opening
//! that reaches into it), in the material the face resolves to (room_finishes.rs). Baked once with
//! the home's other meshes; nothing here runs per frame.
//!
//! HOW THE FACES ARE FOUND. On each storey every wall standing in it is a footprint polygon in the
//! floor plan: each interior wall as drawn (`HomeStructure::wall_feet`, mitres and clips included),
//! the perimeter shell's pieces, the corner columns. The trim runs along the BOUNDARY OF THEIR UNION
//! that faces into the box: each polygon edge is split where any other polygon's edges cross it,
//! and a piece is kept when a point just off it, on its outward side, is inside no other polygon and
//! inside the box. So a wall meeting another mid-span, a T, an X, a free end, a column and the shell
//! all come out as one closed outline per room, and a corner is where two kept pieces meet. A glass
//! wall and a guard stand in the union (the trim stops against them) but carry none.
//!
//! WHICH ROOM A FACE IS IN: the smallest zone of its storey that holds a point 5 cm in front of it
//! (the zone is the room, `Zone::finish`, `Zone::room_type`). Upstairs, a face over no slab or in no
//! zone is no room's, and carries no trim.

use crate::fibonacci::TrimGroup;
use crate::home_structure::{perimeter_pieces, storey_index, wall_in_storey, HomeStructure, ShellCut, Storey, Zone, CORNER_COLUMN_SEGMENTS, SLAB_DRAW_LIFT_M, STOREY_EPS};
use crate::room_finishes::{catalog, ProfileKind, ResolvedFinish, TrimProfile};
use glam::{Vec2, Vec3};
use humanity_core::vertex::Vertex;
use std::sync::atomic::{AtomicBool, Ordering};

/// How finely the trim is drawn: the player's own home in full, a neighbour's at its profiles'
/// bounding outlines (`TrimProfile::coarse`, two quads a run where the full has a dozen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimDetail {
    Full,
    Coarse,
}

/// Whether the trim is baked at all: the showcase's `{"room_trim":"0"}` turns it off for a
/// same-boot look at the rooms without it (and the cost of drawing it), "1" back on. Permanent dev
/// tooling, like neighbours.rs `GLASS_DRAWN`.
static TRIM_DRAWN: AtomicBool = AtomicBool::new(true);

/// Whether the trim is baked (`TRIM_DRAWN`).
pub fn trim_drawn() -> bool {
    TRIM_DRAWN.load(Ordering::Relaxed)
}

/// Set `TRIM_DRAWN`; true when it changed (the caller rebuilds the home's meshes then).
pub fn set_trim_drawn(on: bool) -> bool {
    TRIM_DRAWN.swap(on, Ordering::Relaxed) != on
}

/// How far off a face its outward test point is, metres: under any wall's thickness (a 1 mm paper
/// screen is the thinnest), over the float error at the home's size.
const OFF_FACE_M: f32 = 2e-4;
/// How far in front of a face the point that names its room is, metres.
const ROOM_PROBE_M: f32 = 0.05;
/// Two run ends this close are one corner, metres.
const JOIN_M: f32 = 1e-4;

/// What a boundary edge belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Src {
    /// A long face of interior wall `wall`, `left` the side on your left walking from its `a` to `b`.
    Face { wall: usize, left: bool },
    /// An end of interior wall `wall`.
    Cap { wall: usize },
    /// Perimeter edge `edge` (0..4, the shell's build order).
    Shell { edge: usize },
    /// A corner column.
    Column,
}

/// One footprint polygon of a storey, counter-clockwise in (x, z).
struct Poly {
    pts: Vec<Vec2>,
    srcs: Vec<Src>,
    /// It carries trim (a solid wall); a glass wall or a guard only stops it.
    emits: bool,
    /// The wall's top, body-local metres (where a crown can sit at most).
    top: f32,
    min: Vec2,
    max: Vec2,
}

impl Poly {
    fn new(mut pts: Vec<Vec2>, mut srcs: Vec<Src>, emits: bool, top: f32) -> Self {
        let area: f32 = (0..pts.len()).map(|i| cross(pts[i], pts[(i + 1) % pts.len()])).sum();
        if area < 0.0 {
            // Clockwise: reverse, keeping each edge's source with it (edge i ran pts[i] -> pts[i+1]).
            pts.reverse();
            let n = srcs.len();
            srcs = (0..n).map(|i| srcs[(2 * n - 2 - i) % n]).collect();
        }
        let min = pts.iter().fold(Vec2::splat(f32::MAX), |m, p| m.min(*p));
        let max = pts.iter().fold(Vec2::splat(f32::MIN), |m, p| m.max(*p));
        Poly { pts, srcs, emits, top, min, max }
    }

    fn edge(&self, i: usize) -> (Vec2, Vec2) {
        (self.pts[i], self.pts[(i + 1) % self.pts.len()])
    }

    fn near(&self, lo: Vec2, hi: Vec2) -> bool {
        self.min.x <= hi.x + 1e-3 && self.max.x >= lo.x - 1e-3 && self.min.y <= hi.y + 1e-3 && self.max.y >= lo.y - 1e-3
    }

    /// Crossing-number test: `p` strictly inside (a point on the outline may go either way, and
    /// the callers keep their points `OFF_FACE_M` off every outline they care about).
    fn contains(&self, p: Vec2) -> bool {
        if p.x < self.min.x || p.x > self.max.x || p.y < self.min.y || p.y > self.max.y {
            return false;
        }
        let mut inside = false;
        let n = self.pts.len();
        for i in 0..n {
            let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// The outward normal of an edge of a counter-clockwise polygon running along `d`.
fn outward(d: Vec2) -> Vec2 {
    Vec2::new(d.y, -d.x).normalize_or_zero()
}

/// A kept piece of a storey's outline: from `a` to `b` (the walls on its left), `n` pointing into
/// the room.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Piece {
    pub a: Vec2,
    pub b: Vec2,
    pub n: Vec2,
    pub src: Src,
    pub top: f32,
}

/// One trim run to sweep: a stretch of one piece with one profile.
#[derive(Debug, Clone)]
pub(crate) struct Run<'a> {
    pub kind: ProfileKind,
    pub a: Vec2,
    pub b: Vec2,
    pub n: Vec2,
    pub profile: &'a TrimProfile,
    pub material: &'a str,
    /// The height the profile's `up` is measured from: the floor (skirting) or the crown's top.
    pub y: f32,
    /// This end was stopped (a door, a wall end with nothing to turn onto): capped square.
    pub free_a: bool,
    pub free_b: bool,
}

/// Every footprint polygon standing in storey `k`.
fn storey_polys(hs: &HomeStructure, storeys: &[Storey], k: usize, feet: &[crate::home_structure::WallFoot]) -> Vec<Poly> {
    let storey = storeys[k];
    let mut polys = Vec::new();
    let opaque = |m: u32| HomeStructure::material_color(m)[3] >= 0.999;
    for (i, w) in hs.walls.iter().enumerate() {
        if !wall_in_storey(storeys, w, k) {
            continue;
        }
        let f = feet[i];
        let v = |p: (f32, f32)| Vec2::new(p.0, p.1);
        if (v(f.cb) - v(f.ca)).length() < 1e-3 {
            continue;
        }
        // ar -> br is the -perpendicular face (your left walking a to b, x east and z south), br -> bl
        // the b end, bl -> al the +perpendicular face, al -> ar the a end.
        let pts = vec![v(f.ar), v(f.br), v(f.bl), v(f.al)];
        let srcs = vec![Src::Face { wall: i, left: true }, Src::Cap { wall: i }, Src::Face { wall: i, left: false }, Src::Cap { wall: i }];
        polys.push(Poly::new(pts, srcs, w.guard().is_none() && opaque(w.exposed_material()), w.top()));
    }
    // The perimeter shell, piece by piece round its open spans (`perimeter_pieces`), as drawn by
    // `generate_meshes_with_shell_cuts`: each piece a box centred on the plot's edge.
    let (w, d, h) = (hs.width.max(1.0), hs.depth.max(1.0), hs.height.max(1.0));
    let corners = [Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, d), Vec2::new(0.0, d)];
    let shell_t = hs.shell_resolved_thickness();
    for ei in 0..4 {
        let (a, b) = (corners[ei], corners[(ei + 1) % 4]);
        let len = if ei % 2 == 0 { w } else { d };
        let u = (b - a) / len;
        let p = Vec2::new(-u.y, u.x); // into the box: the perimeter winds counter-clockwise
        for (s, e, span) in perimeter_pieces(&hs.open_shell, w, d, ei, len) {
            let (top, thick, mat) = match span {
                None => (h, shell_t, hs.shell_material),
                Some(sp) if sp.height > 1e-3 => (sp.height.min(h), sp.drawn_thickness(), sp.material),
                Some(_) => continue,
            };
            if top <= storey.floor + STOREY_EPS {
                continue;
            }
            let (pa, pb) = (a + u * s, a + u * e);
            let half = p * (thick * 0.5);
            polys.push(Poly::new(vec![pa - half, pb - half, pb + half, pa + half], vec![Src::Shell { edge: ei }; 4], opaque(mat), top));
        }
    }
    for c in hs.corner_columns() {
        if c.top <= storey.floor + STOREY_EPS || c.bottom >= storey.top - STOREY_EPS {
            continue;
        }
        let n = CORNER_COLUMN_SEGMENTS as usize;
        let pts: Vec<Vec2> = (0..n)
            .map(|i| {
                let ang = (i as f32 / n as f32) * std::f32::consts::TAU;
                Vec2::new(c.cx + ang.cos() * c.radius, c.cz + ang.sin() * c.radius)
            })
            .collect();
        polys.push(Poly::new(pts, vec![Src::Column; n], opaque(c.material), c.top));
    }
    polys
}

/// The outline of a storey's walls that faces into the box (see the module doc).
fn outline(polys: &[Poly], w: f32, d: f32) -> Vec<Piece> {
    let mut out = Vec::new();
    let in_box = |p: Vec2| p.x > 0.0 && p.x < w && p.y > 0.0 && p.y < d;
    for (pi, poly) in polys.iter().enumerate() {
        if !poly.emits {
            continue;
        }
        for ei in 0..poly.pts.len() {
            let (p, q) = poly.edge(ei);
            let dv = q - p;
            let len = dv.length();
            if len < 1e-5 {
                continue;
            }
            let n = outward(dv);
            let (lo, hi) = (p.min(q), p.max(q));
            let mut ts = vec![0.0f32, 1.0];
            for (qi, other) in polys.iter().enumerate() {
                if qi == pi || !other.near(lo, hi) {
                    continue;
                }
                for oj in 0..other.pts.len() {
                    let (r, s) = other.edge(oj);
                    let d2 = s - r;
                    let den = cross(dv, d2);
                    if den.abs() > 1e-9 {
                        let t = cross(r - p, d2) / den;
                        let u = cross(r - p, dv) / den;
                        if t > 0.0 && t < 1.0 && (-1e-6..=1.0 + 1e-6).contains(&u) {
                            ts.push(t);
                        }
                    } else if cross(r - p, dv).abs() < 1e-7 * len.max(1.0) {
                        // Collinear: the other edge's ends that lie along this one split it.
                        for e in [r, s] {
                            let t = (e - p).dot(dv) / (len * len);
                            if t > 0.0 && t < 1.0 {
                                ts.push(t);
                            }
                        }
                    }
                }
            }
            ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            ts.dedup_by(|a, b| (*a - *b).abs() * len < 1e-5);
            for win in ts.windows(2) {
                let (t0, t1) = (win[0], win[1]);
                let mid = p + dv * ((t0 + t1) * 0.5);
                let probe = mid + n * OFF_FACE_M;
                if !in_box(probe) {
                    continue;
                }
                if polys.iter().enumerate().any(|(qi, o)| qi != pi && o.contains(probe)) {
                    continue;
                }
                // Two polygons with the same face on the same line (a wall drawn twice): the first keeps it.
                let dup = polys[..pi].iter().any(|o| {
                    o.emits
                        && (0..o.pts.len()).any(|oj| {
                            let (r, s) = o.edge(oj);
                            let od = s - r;
                            let ol = od.length();
                            ol > 1e-5 && outward(od).dot(n) > 0.999 && cross(mid - r, od).abs() / ol < 1e-5 && {
                                let t = (mid - r).dot(od) / (ol * ol);
                                t > 0.0 && t < 1.0
                            }
                        })
                });
                if dup {
                    continue;
                }
                out.push(Piece { a: p + dv * t0, b: p + dv * t1, n, src: poly.srcs[ei], top: poly.top });
            }
        }
    }
    out
}

/// `piece` cut where it crosses an edge of any floor slab (so each part has one ceiling and one
/// floor under it).
fn split_at_slabs(piece: Piece, hs: &HomeStructure) -> Vec<Piece> {
    let dv = piece.b - piece.a;
    let mut ts = vec![0.0f32, 1.0];
    for s in &hs.floors {
        let (x0, z0, x1, z1) = s.rect();
        let c = [Vec2::new(x0, z0), Vec2::new(x1, z0), Vec2::new(x1, z1), Vec2::new(x0, z1)];
        for i in 0..4 {
            let (r, e) = (c[i], c[(i + 1) % 4]);
            let d2 = e - r;
            let den = cross(dv, d2);
            if den.abs() > 1e-9 {
                let t = cross(r - piece.a, d2) / den;
                let u = cross(r - piece.a, dv) / den;
                if t > 1e-5 && t < 1.0 - 1e-5 && (0.0..=1.0).contains(&u) {
                    ts.push(t);
                }
            }
        }
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    ts.windows(2).map(|w| Piece { a: piece.a + dv * w[0], b: piece.a + dv * w[1], ..piece }).collect()
}

/// The smallest zone of storey `k` holding `p` (the room a face is in).
fn zone_at<'a>(hs: &'a HomeStructure, storeys: &[Storey], k: usize, p: Vec2) -> Option<&'a Zone> {
    hs.zones
        .iter()
        .filter(|z| storey_index(storeys, z.origin.1) == k && z.contains_xz(p.x, p.y))
        .min_by(|a, b| (a.size.0 * a.size.2).partial_cmp(&(b.size.0 * b.size.2)).unwrap_or(std::cmp::Ordering::Equal))
}

/// The stretches of a piece an opening keeps a trim off, as (t0, t1) along the piece: the openings
/// in its wall (or the corridor cuts through its shell edge) whose height band `blocks` says reaches
/// the trim. (bottom, top) are body-local heights.
fn blocked(piece: &Piece, hs: &HomeStructure, cuts: &[ShellCut], blocks: impl Fn(f32, f32) -> bool) -> Vec<(f32, f32)> {
    // The piece's ends in the line's own coordinate (metres from its start) and the openings on it.
    let (origin, dir, spans): (Vec2, Vec2, Vec<(f32, f32)>) = match piece.src {
        Src::Face { wall, .. } => {
            let wl = &hs.walls[wall];
            let a = Vec2::new(wl.a.0, wl.a.1);
            let dir = (Vec2::new(wl.b.0, wl.b.1) - a).normalize_or_zero();
            let spans = wl.openings.iter().filter(|o| blocks(wl.base + o.sill, wl.base + o.sill + o.height)).map(|o| (o.at, o.at + o.width)).collect();
            (a, dir, spans)
        }
        Src::Shell { edge } => {
            let (w, d) = (hs.width.max(1.0), hs.depth.max(1.0));
            let corners = [Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, d), Vec2::new(0.0, d)];
            let (a, b) = (corners[edge], corners[(edge + 1) % 4]);
            let spans = cuts.iter().filter(|c| c.edge == edge && blocks(c.base, c.base + c.height)).map(|c| (c.at, c.at + c.width)).collect();
            (a, (b - a).normalize_or_zero(), spans)
        }
        _ => return Vec::new(),
    };
    let (sa, sb) = ((piece.a - origin).dot(dir), (piece.b - origin).dot(dir));
    if (sb - sa).abs() < 1e-6 {
        return Vec::new();
    }
    spans
        .into_iter()
        .filter_map(|(o0, o1)| {
            let (t0, t1) = ((o0 - sa) / (sb - sa), (o1 - sa) / (sb - sa));
            let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
            (hi > lo).then_some((lo, hi))
        })
        .collect()
}

/// [0, 1] less the `gaps`, as kept stretches with whether each end was made by a gap.
fn keep_between(mut gaps: Vec<(f32, f32)>) -> Vec<(f32, f32, bool, bool)> {
    gaps.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    let (mut at, mut free) = (0.0f32, false);
    for (g0, g1) in gaps {
        if g0 > at + 1e-5 {
            out.push((at, g0, free, true));
        }
        if g1 > at {
            at = g1;
            free = true;
        }
    }
    if at < 1.0 - 1e-5 {
        out.push((at, 1.0, free, false));
    }
    out
}

/// Every trim run of the home (each storey's outline, its rooms' finishes resolved, cut at the
/// openings). The bake sweeps them; the tests read them.
pub(crate) fn runs(hs: &HomeStructure, cuts: &[ShellCut]) -> Vec<Run<'static>> {
    let cat = catalog();
    let storeys = hs.storeys();
    let feet = hs.wall_feet();
    let (w, d, h) = (hs.width.max(1.0), hs.depth.max(1.0), hs.height.max(1.0));
    let mut out: Vec<Run<'static>> = Vec::new();
    for k in 0..storeys.len() {
        let storey = storeys[k];
        let polys = storey_polys(hs, &storeys, k, &feet);
        let floor_y = if k == 0 { 0.0 } else { storey.floor + SLAB_DRAW_LIFT_M };
        for whole in outline(&polys, w, d) {
            for piece in split_at_slabs(whole, hs) {
                let probe = (piece.a + piece.b) * 0.5 + piece.n * ROOM_PROBE_M;
                if k > 0 && !hs.floors.iter().any(|s| s.is_floor_of(&storey) && s.contains(probe.x, probe.y)) {
                    continue; // over a stairwell or the open: no floor to run along
                }
                let zone = zone_at(hs, &storeys, k, probe);
                if k > 0 && zone.is_none() {
                    continue; // upstairs, only the rooms its zones make are rooms
                }
                let face = match piece.src {
                    Src::Face { wall, left } => Some(hs.walls[wall].finish.side(left)),
                    _ => None,
                };
                let fin: ResolvedFinish<'static> = cat.resolve(face, zone.map(|z| &z.finish), zone.and_then(|z| z.room_type.as_deref()));
                // The ceiling over this face: the lowest slab underside above its floor, else the roof.
                let ceiling = hs
                    .floors
                    .iter()
                    .filter(|s| s.underside() > storey.floor + 0.1 && s.contains(probe.x, probe.y))
                    .map(|s| s.underside())
                    .fold(h, f32::min);
                let mut push = |kind: ProfileKind, profile: &'static TrimProfile, y: f32, gaps: Vec<(f32, f32)>| {
                    let dv = piece.b - piece.a;
                    for (t0, t1, fa, fb) in keep_between(gaps) {
                        out.push(Run { kind, a: piece.a + dv * t0, b: piece.a + dv * t1, n: piece.n, profile, material: fin.material.id.as_str(), y, free_a: fa, free_b: fb });
                    }
                };
                if let Some(p) = fin.skirting {
                    let top = floor_y + p.height();
                    // A door, or a window down to the floor, stops it: an opening whose foot is under its top.
                    push(ProfileKind::Skirting, p, floor_y, blocked(&piece, hs, cuts, |bottom, _| bottom < top - 1e-3));
                }
                if let Some(p) = fin.crown {
                    // Only where the wall meets the ceiling: a wall that stops short of it is a partition.
                    if piece.top >= ceiling - 0.02 {
                        let y = ceiling.min(piece.top);
                        let foot = y - p.height();
                        push(ProfileKind::Crown, p, y, blocked(&piece, hs, cuts, |_, top| top > foot + 1e-3));
                    }
                }
            }
        }
    }
    // The corners: a run's end that is not stopped turns onto the run of its kind starting there;
    // with none (a glass wall, a face whose finish has none), it is stopped after all.
    let n = out.len();
    let mut has_next = vec![false; n];
    let mut has_prev = vec![false; n];
    for i in 0..n {
        if out[i].free_b {
            continue;
        }
        if let Some(j) = (0..n).find(|&j| !out[j].free_a && out[j].kind == out[i].kind && (out[j].a - out[i].b).length() < JOIN_M && j != i && (out[j].y - out[i].y).abs() < 0.05) {
            has_next[i] = true;
            has_prev[j] = true;
        }
    }
    for i in 0..n {
        out[i].free_b |= !has_next[i];
        out[i].free_a |= !has_prev[i];
    }
    out
}

/// The run that ends where `run` starts (and the one starting where it ends), for the mitres.
fn neighbour<'a>(all: &'a [Run<'static>], at: Vec2, kind: ProfileKind, y: f32, starts: bool) -> Option<&'a Run<'static>> {
    all.iter().find(|r| r.kind == kind && (r.y - y).abs() < 0.05 && if starts { !r.free_a && (r.a - at).length() < JOIN_M } else { !r.free_b && (r.b - at).length() < JOIN_M })
}

/// Where profile offset `o` sits at the corner `at` between a run along `d1` (normal `n1`) and one
/// along `d2` (normal `n2`): where the two offset lines cross, the MITRE. Parallel runs meet at the
/// plain offset; a corner so sharp the mitre would run out past 8x the offset stops there.
pub(crate) fn mitre(at: Vec2, d1: Vec2, n1: Vec2, d2: Vec2, n2: Vec2, o: f32) -> Vec2 {
    let c = cross(d1, d2);
    if c.abs() < 1e-6 {
        return at + n1 * o;
    }
    let t = cross((n2 - n1) * o, d2) / c;
    let t = t.clamp(-8.0 * o - 1e-4, 8.0 * o + 1e-4);
    at + n1 * o + d1 * t
}

/// The plan positions of every profile point at both ends of `run` (mitred where it turns onto a
/// neighbour, square where it is stopped).
pub(crate) fn run_ends(all: &[Run<'static>], run: &Run<'static>, pts: &[(f32, f32)]) -> (Vec<Vec2>, Vec<Vec2>) {
    let dir = (run.b - run.a).normalize_or_zero();
    let prev = if run.free_a { None } else { neighbour(all, run.a, run.kind, run.y, false) };
    let next = if run.free_b { None } else { neighbour(all, run.b, run.kind, run.y, true) };
    let mut s = Vec::with_capacity(pts.len());
    let mut e = Vec::with_capacity(pts.len());
    for &(o, _) in pts {
        let ps = match prev {
            Some(p) => mitre(run.a, (p.b - p.a).normalize_or_zero(), p.n, dir, run.n, o),
            None => run.a + run.n * o,
        };
        let pe = match next {
            Some(q) => mitre(run.b, dir, run.n, (q.b - q.a).normalize_or_zero(), q.n, o),
            None => run.b + run.n * o,
        };
        // A run shorter than its inside mitres would turn inside out: both ends meet in the middle.
        if (pe - ps).dot(dir) < 0.0 {
            let m = (ps + pe) * 0.5;
            s.push(m);
            e.push(m);
        } else {
            s.push(ps);
            e.push(pe);
        }
    }
    (s, e)
}

/// Push one triangle, wound so its front faces `n`.
fn tri(v: &mut Vec<Vertex>, idx: &mut Vec<u32>, p: [Vec3; 3], nrm: [Vec3; 3], uv: [[f32; 2]; 3], face: Vec3) {
    let g = (p[1] - p[0]).cross(p[2] - p[0]);
    let order: [usize; 3] = if g.dot(face) >= 0.0 { [0, 1, 2] } else { [0, 2, 1] };
    let base = v.len() as u32;
    for &k in &order {
        v.push(Vertex { position: p[k].to_array(), normal: nrm[k].to_array(), uv: uv[k] });
    }
    idx.extend([base, base + 1, base + 2]);
}

/// Push one quad (corners in order round it), its four corners shared by its two triangles, wound so
/// its front faces `face`. A quad whose first triangle has collapsed (a wedge at a short run's
/// meeting mitres) takes its winding from the other.
fn quad(v: &mut Vec<Vertex>, idx: &mut Vec<u32>, p: [Vec3; 4], nrm: [Vec3; 4], uv: [[f32; 2]; 4], face: Vec3) {
    let g1 = (p[1] - p[0]).cross(p[2] - p[0]);
    let g = if g1.length_squared() > 1e-14 { g1 } else { (p[2] - p[0]).cross(p[3] - p[0]) };
    let base = v.len() as u32;
    for k in 0..4 {
        v.push(Vertex { position: p[k].to_array(), normal: nrm[k].to_array(), uv: uv[k] });
    }
    if g.dot(face) >= 0.0 {
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        idx.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

/// Sweep one run into `v`/`idx`: a quad per profile segment (smooth across a gentle bend of the
/// profile, crisp across a fillet), and a cap on each stopped end.
fn sweep(all: &[Run<'static>], run: &Run<'static>, pts: &[(f32, f32)], v: &mut Vec<Vertex>, idx: &mut Vec<u32>) {
    if pts.len() < 2 || (run.b - run.a).length() < 1e-4 {
        return;
    }
    let up = if run.kind == ProfileKind::Crown { -1.0 } else { 1.0 };
    let (s, e) = run_ends(all, run, pts);
    let at = |p: Vec2, u: f32| Vec3::new(p.x, run.y + up * u, p.y);
    let n3 = Vec3::new(run.n.x, 0.0, run.n.y);
    // Each segment's normal: the profile's (-du, do), carried into the world.
    let seg_n: Vec<Vec3> = pts
        .windows(2)
        .map(|w| {
            let m = Vec2::new(-(w[1].1 - w[0].1), w[1].0 - w[0].0).normalize_or_zero();
            (n3 * m.x + Vec3::Y * (up * m.y)).normalize_or_zero()
        })
        .collect();
    let smooth = |a: Vec3, b: Vec3| a.dot(b) > 0.766; // under 40 degrees apart
    let len = (run.b - run.a).length();
    let mut arc = 0.0f32;
    for j in 0..seg_n.len() {
        let nj = seg_n[j];
        let n0 = if j > 0 && smooth(seg_n[j - 1], nj) { (seg_n[j - 1] + nj).normalize() } else { nj };
        let n1 = if j + 1 < seg_n.len() && smooth(seg_n[j + 1], nj) { (seg_n[j + 1] + nj).normalize() } else { nj };
        let (a0, a1) = (at(s[j], pts[j].1), at(s[j + 1], pts[j + 1].1));
        let (b0, b1) = (at(e[j], pts[j].1), at(e[j + 1], pts[j + 1].1));
        let seg = Vec2::new(pts[j + 1].0 - pts[j].0, pts[j + 1].1 - pts[j].1).length();
        let (u0, u1) = (arc, arc + seg);
        arc = u1;
        quad(v, idx, [a0, a1, b1, b0], [n0, n1, n1, n0], [[0.0, u0], [0.0, u1], [len, u1], [len, u0]], nj);
    }
    // The stopped ends: the cross-section filled from the wall corner (0, 0).
    let dir = Vec3::new(run.b.x - run.a.x, 0.0, run.b.y - run.a.y).normalize_or_zero();
    for (ends, face, free) in [(&s, -dir, run.free_a), (&e, dir, run.free_b)] {
        if !free {
            continue;
        }
        let corner_plan = ends[0] - run.n * pts[0].0; // the wall's line at this end
        let c = at(corner_plan, 0.0);
        for j in 0..pts.len() - 1 {
            let (p0, p1) = (at(ends[j], pts[j].1), at(ends[j + 1], pts[j + 1].1));
            tri(v, idx, [c, p0, p1], [face; 3], [[0.0, 0.0], [pts[j].0, pts[j].1], [pts[j + 1].0, pts[j + 1].1]], face);
        }
    }
}

/// THE BAKE: every trim run of the home swept, one group per trim material (sorted by its id, so a
/// rebuild keeps its draw order). Empty when the showcase has turned the trim off.
pub fn bake_trim(hs: &HomeStructure, cuts: &[ShellCut], detail: TrimDetail) -> Vec<TrimGroup> {
    if !trim_drawn() {
        return Vec::new();
    }
    let all = runs(hs, cuts);
    let cat = catalog();
    let mut groups: std::collections::BTreeMap<&str, (Vec<Vertex>, Vec<u32>)> = std::collections::BTreeMap::new();
    for run in &all {
        let pts: Vec<(f32, f32)> = match detail {
            TrimDetail::Full => run.profile.points.clone(),
            TrimDetail::Coarse => run.profile.coarse(),
        };
        let g = groups.entry(run.material).or_default();
        sweep(&all, run, &pts, &mut g.0, &mut g.1);
    }
    groups
        .into_iter()
        .filter(|(_, (v, _))| !v.is_empty())
        .filter_map(|(id, (verts, indices))| cat.material(id).map(|m| TrimGroup { verts, indices, look: m.trim_look() }))
        .collect()
}

#[cfg(test)]
#[path = "room_trim_tests.rs"]
mod tests;
