//! DOOR AND WINDOW CASINGS (room finishes rung 2, docs/design/room-finishes.md): the board a joiner
//! fits round every opening on each face that has a room in front of it, as a profile from
//! data/blueprints/room_finishes.ron swept up each side and across the head, MITRED at the two head
//! corners. A door's legs stand on the floor (the skirting stops against their outer edge,
//! room_trim.rs); a window's stand on its stool, the inside sill, which runs past the casing at each
//! end (its horns) with the same casing board under it as the apron. A window whose face has no
//! stool is framed on all four sides, mitred at every corner. Near a corner, the crown or the
//! ceiling the casing is ripped narrower, as a joiner would, and where under 30 mm is left the
//! opening is not cased on that face.
//!
//! Everything here is in the plane of one wall face: `s` metres along the face's line (the same
//! line its wall's openings are measured on), `y` body-local height, and each profile's `out`
//! standing into the room. Baked with the home's other trim (room_trim.rs `bake_trim`); nothing
//! here runs per frame.

use crate::room_finishes::{ProfileKind, TrimProfile};
use crate::room_trim::{mitre, Src, Swept, TrimDetail};
use glam::{Vec2, Vec3};

/// Narrower than this, a casing is not fitted at all (the opening is left uncased on that face,
/// the skirting stopping at its jamb as before casings), metres.
pub(crate) const MIN_CASING_M: f32 = 0.03;
/// A window whose foot is lower than this over its floor is cased like a door, its legs down to
/// the floor with no stool (a glazed door, a window to the floor), metres.
pub(crate) const FLOOR_WINDOW_M: f32 = 0.3;
/// How close under the crown, the ceiling or the wall's top a head casing may come, metres.
const HEAD_CLEAR_M: f32 = 0.002;

/// Which opening a casing frames: one of a wall's, or one of the corridor cuts through the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OpeningId {
    Wall { wall: usize, index: usize },
    Cut { index: usize },
}

/// One opening on a face's line: its edges `s0 < s1` along the line, its foot and head (body-local
/// heights), and whether it is a door (a door, or a corridor's cut through the shell).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineOpening {
    pub id: OpeningId,
    pub s0: f32,
    pub s1: f32,
    pub bottom: f32,
    pub top: f32,
    pub door: bool,
}

/// The straight line a piece of a room's outline lies on (an interior wall's centreline, or a
/// perimeter edge), with the openings along it, each measured from `origin` along `dir`.
pub(crate) struct Line {
    pub origin: Vec2,
    pub dir: Vec2,
    pub openings: Vec<LineOpening>,
}

impl Line {
    /// Metres along the line to `p`'s foot on it.
    pub fn s(&self, p: Vec2) -> f32 {
        (p - self.origin).dot(self.dir)
    }
}

/// THE CASING ROUND ONE OPENING ON ONE FACE, as fitted: which face and opening, the face's plane
/// (`origin` + `dir` * s on the face itself, `n` into the room), the opening's edges, foot and head,
/// the profile and the width it is ripped to, and a window's stool.
#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub face: Src,
    pub opening: OpeningId,
    pub origin: Vec2,
    pub dir: Vec2,
    pub n: Vec2,
    pub s0: f32,
    pub s1: f32,
    /// The floor the face stands on (where a door's legs stand), body-local metres.
    pub floor: f32,
    pub bottom: f32,
    pub top: f32,
    /// Cased like a door: legs to the floor, no stool, no bottom piece.
    pub door: bool,
    pub casing: &'static TrimProfile,
    /// The casing's width as fitted: the profile's own, or less where it was ripped.
    pub width: f32,
    /// A window's stool; None at a door, and at a window framed on all four sides.
    pub sill: Option<&'static TrimProfile>,
    /// The casing board under the stool (left off where it would reach down into the skirting).
    pub apron: bool,
    /// How far the stool runs past the casing at each end.
    pub horn: f32,
    pub material: &'static str,
}

impl Frame {
    /// Where the legs start: the floor at a door, the opening's foot at a window.
    pub fn foot(&self) -> f32 {
        if self.door {
            self.floor
        } else {
            self.bottom
        }
    }

    /// The stool's underside, for a window with a stool.
    pub fn stool_under(&self) -> Option<f32> {
        self.sill.map(|p| self.bottom - p.height())
    }

    /// The heights the frame's trim covers, (lowest, highest).
    pub fn band(&self) -> (f32, f32) {
        let lo = if self.door {
            self.floor
        } else {
            match self.stool_under() {
                Some(u) => u - if self.apron { self.width } else { 0.0 },
                None => self.bottom - self.width,
            }
        };
        (lo, self.top + self.width)
    }

    /// How far past the opening's edges the frame reaches between heights `y0` and `y1`: the
    /// casing's width, or with the stool's horns where that band takes in the stool; None where the
    /// frame does not reach that band at all. A horizontal trim crossing the band stops this far
    /// out from the opening (room_trim.rs `gaps`).
    pub fn reach(&self, y0: f32, y1: f32) -> Option<f32> {
        let (lo, hi) = self.band();
        if !(y0 < hi - 1e-3 && y1 > lo + 1e-3) {
            return None;
        }
        let on_stool = self.stool_under().is_some_and(|u| y0 < self.bottom - 1e-3 && y1 > u + 1e-3);
        Some(if on_stool { self.width + self.horn } else { self.width })
    }
}

/// What a face offers an opening's casing: the room to spread into along the face (`room`, the
/// face's exposed stretch along the line, from wall to wall), how high the head may reach, where
/// the skirting's top is (an apron stops above it), and the stool's horns from the catalog.
pub(crate) struct FitRoom {
    pub room: (f32, f32),
    pub head_limit: f32,
    pub floor: f32,
    pub skirting_top: f32,
    pub horn: f32,
}

/// Fit `casing` (and the window stool `sill`) round opening `op` on the face `face` (its plane
/// `origin`, `dir`, `n`). Ripped narrower where the face ends or the head would reach the crown,
/// the ceiling or the wall's top; None where under [`MIN_CASING_M`] is left.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit(
    face: Src,
    op: &LineOpening,
    origin: Vec2,
    dir: Vec2,
    n: Vec2,
    at: &FitRoom,
    casing: &'static TrimProfile,
    sill: Option<&'static TrimProfile>,
    material: &'static str,
) -> Option<Frame> {
    let width = casing.height().min(op.s0 - at.room.0).min(at.room.1 - op.s1).min(at.head_limit - HEAD_CLEAR_M - op.top);
    if width < MIN_CASING_M {
        return None;
    }
    let door = op.door || op.bottom - at.floor < FLOOR_WINDOW_M;
    let sill = if door { None } else { sill };
    let horn = at.horn.min(op.s0 - at.room.0 - width).min(at.room.1 - op.s1 - width).max(0.0);
    let apron = sill.is_some_and(|p| op.bottom - p.height() - width >= at.skirting_top + 0.01);
    Some(Frame { face, opening: op.id, origin, dir, n, s0: op.s0, s1: op.s1, floor: at.floor, bottom: op.bottom, top: op.top, door, casing, width, sill, apron, horn, material })
}

/// `pts` ripped to `width` across the board (its `up`), as a joiner rips a casing narrower: the
/// part past `width` sawn off, a sawn edge from the wall out to the face where it crosses.
pub(crate) fn rip(pts: &[(f32, f32)], width: f32) -> Vec<(f32, f32)> {
    if pts.iter().all(|p| p.1 <= width + 1e-6) {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() + 2);
    if pts[0].1 > width {
        out.push((0.0, width));
    }
    for i in 0..pts.len() {
        let p = pts[i];
        if i > 0 {
            let q = pts[i - 1];
            if (q.1 > width) != (p.1 > width) {
                let t = (width - q.1) / (p.1 - q.1);
                out.push((q.0 + (p.0 - q.0) * t, width));
            }
        }
        if p.1 <= width {
            out.push(p);
        }
    }
    out.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
    out
}

/// One straight part of a frame in its face's plane: from `a` to `b` in (s, y), the profile's `up`
/// axis along `m` in that plane, the parts it mitres onto at each end, and its caps.
struct Part {
    kind: ProfileKind,
    a: Vec2,
    b: Vec2,
    m: Vec2,
    prev: Option<usize>,
    next: Option<usize>,
    cap_a: bool,
    cap_b: bool,
    pts: Vec<(f32, f32)>,
}

impl Part {
    fn d(&self) -> Vec2 {
        (self.b - self.a).normalize_or_zero()
    }
}

/// The parts of a frame (the legs, the head, a window's stool and apron or its bottom piece), each
/// ready to sweep. The legs and the head (and a bottom piece) share one ripped profile, so their
/// mitres meet point for point.
pub(crate) fn frame_sweeps(f: &Frame, detail: TrimDetail) -> Vec<Swept> {
    let of = |p: &TrimProfile| match detail {
        TrimDetail::Full => p.points.clone(),
        TrimDetail::Coarse => p.coarse(),
    };
    let cp = rip(&of(f.casing), f.width);
    let (s0, s1, top, foot) = (f.s0, f.s1, f.top, f.foot());
    let part = |kind, a: Vec2, b: Vec2, m: Vec2, cap: bool, pts: &Vec<(f32, f32)>| Part { kind, a, b, m, prev: None, next: None, cap_a: cap, cap_b: cap, pts: pts.clone() };
    let mut parts = vec![
        part(ProfileKind::Casing, Vec2::new(s0, foot), Vec2::new(s0, top), Vec2::new(-1.0, 0.0), false, &cp),
        part(ProfileKind::Casing, Vec2::new(s0, top), Vec2::new(s1, top), Vec2::new(0.0, 1.0), false, &cp),
        part(ProfileKind::Casing, Vec2::new(s1, top), Vec2::new(s1, foot), Vec2::new(1.0, 0.0), false, &cp),
    ];
    parts[0].next = Some(1);
    parts[1].prev = Some(0);
    parts[1].next = Some(2);
    parts[2].prev = Some(1);
    if !f.door {
        match (f.sill, f.stool_under()) {
            (Some(sill), Some(under)) => {
                let reach = f.width + f.horn;
                parts.push(part(ProfileKind::Sill, Vec2::new(s0 - reach, under), Vec2::new(s1 + reach, under), Vec2::new(0.0, 1.0), true, &of(sill)));
                if f.apron {
                    parts.push(part(ProfileKind::Casing, Vec2::new(s0 - f.width, under), Vec2::new(s1 + f.width, under), Vec2::new(0.0, -1.0), true, &cp));
                }
            }
            _ => {
                // Framed on all four sides: a bottom piece, mitred onto both legs.
                parts.push(part(ProfileKind::Casing, Vec2::new(s1, f.bottom), Vec2::new(s0, f.bottom), Vec2::new(0.0, -1.0), false, &cp));
                parts[2].next = Some(3);
                parts[3].prev = Some(2);
                parts[3].next = Some(0);
                parts[0].prev = Some(3);
            }
        }
    }
    let world = |p: Vec2, o: f32| Vec3::new(f.origin.x + f.dir.x * p.x + f.n.x * o, p.y, f.origin.y + f.dir.y * p.x + f.n.y * o);
    let plane = |v: Vec2| Vec3::new(f.dir.x * v.x, v.y, f.dir.y * v.x);
    let out = Vec3::new(f.n.x, 0.0, f.n.y);
    let mut sweeps = Vec::with_capacity(parts.len());
    for p in &parts {
        let d = p.d();
        let mut s = Vec::with_capacity(p.pts.len());
        let mut e = Vec::with_capacity(p.pts.len());
        for &(o, u) in &p.pts {
            let ps = match p.prev {
                Some(q) => mitre(p.a, parts[q].d(), parts[q].m, d, p.m, u),
                None => p.a + p.m * u,
            };
            let pe = match p.next {
                Some(q) => mitre(p.b, d, p.m, parts[q].d(), parts[q].m, u),
                None => p.b + p.m * u,
            };
            s.push(world(ps, o));
            e.push(world(pe, o));
        }
        sweeps.push(Swept { kind: p.kind, material: f.material, pts: p.pts.clone(), s, e, out, up: plane(p.m), along: plane(d), cap_a: p.cap_a, cap_b: p.cap_b });
    }
    sweeps
}
