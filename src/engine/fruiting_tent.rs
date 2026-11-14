//! A mushroom rack's fruiting tent, drawn as the fitted plastic cover it is (2026-11-13).
//!
//! The operator, 2026-11-10, on a screenshot of the racks: "Do you see how the shelving looks
//! like it's kind of encased in tinted glass?" Until this module the tent was five flat quads
//! drawn in the glass material (type 4, which darkens its colour to 15% and mirrors the room at
//! roughness 0.05): a tinted glass case. A real fruiting tent (a "Martha tent") is a zippered
//! PVC cover pulled over wire shelving: a soft sheet that bows a little between the frame's
//! posts and shelves, keeps the fold lines of the packet it came in, has welded seams along
//! its edges, a front door that zips down both sides and rolls up to the top (tied there with
//! two straps) while someone tends the shelves, and the holes growers cut or buy for the fan's
//! exhaust and the humidifier's tubing. The references, and the numbers each one gave, are in
//! data/garden/grow_media.ron beside the cover's data (`enclosure.cover`).
//!
//! This builds that cover from the RACK'S FRAME (its size and its shelves' heights, the
//! same `plant_layout::plot_rects` the rack's decks are drawn from) and the cover's data, in
//! the machine's own frame (centred on x and z, its foot at y = 0, its front toward +z):
//!
//! - [`cover_geometry`]: the cover in its three materials: the FILM (every panel, the door,
//!   the vent sleeve), the SEAMS (the doubled, welded bands along its edges, the hem, the
//!   rolled door, the patches round its ports: two or more layers of film, so milkier), and
//!   the FITTINGS (zipper tapes, the ties, the hose grommet: opaque).
//! - [`film_material`], [`seam_material`], [`fittings_material`]: how each is shaded. The film
//!   is material type 27 ([`MATERIAL_TYPE_FILM`]): a frosted sheet lit from
//!   the side it is seen from, passing on part of the light that falls on its far side.
//! - [`roll_at`] and [`tended`]: the door rolls up while someone tends the rack (its card
//!   pinned open, standing at it) and rolls back down after.
//!
//! Pure geometry and rules with no engine state, so it is unit tested here;
//! `home_meshes::push_grow_enclosures` uploads and places what it builds.

use glam::Vec3;

use humanity_core::vertex::Vertex;
use humanity_systems::grow_machines::{Cover, CoverFace, Film, PortKind};

/// Material type 27: a FROSTED FILM (2026-11-13; the tent was type 4, glass, until then, which
/// darkens its colour to 15% and mirrors the room). Plain PBR of its milky base colour in the
/// transparent pass: lit from the side it is seen from, as a pane is, and more opaque at a slant
/// (Beer-Lambert, opacity 1 - (1 - a)^(1 / cos)) and along a fold line (uv.x, `Drape::whiten`),
/// both in `90-fragment-main.wgsl`; passing on `params2.x` of the light on its far side, scattered
/// (`thin_transmit`, `50-brdf.wgsl`); reading its room's bounced light, which clear panes do not
/// (`80-fragment-shared.wgsl`). Blended like glass, it writes its final opacity as TAA's responsive
/// mask (`surface_cover`). At full opacity with `params2.x` 0 it is the plain PBR of its colour.
/// The renderer's type list is in `renderer::materials`; the shaders' band is 26.5 to 27.5.
pub(crate) const MATERIAL_TYPE_FILM: f32 = 27.0;

/// One mesh's geometry: its vertices and its triangle list.
pub(crate) type Geometry = (Vec<Vertex>, Vec<u32>);

/// The rack a cover is fitted to: its size (width, height, depth, metres: the machine's
/// `size` in data/machines/home.ron) and the tops of its shelves above its base, bottom up.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RackFrame {
    pub size: (f32, f32, f32),
    pub shelves: Vec<f32>,
}

impl RackFrame {
    /// The frame of a grow machine of `size` holding `plots` plots: a stacked medium's plots
    /// are its shelves (`plant_layout::plot_rects`), a flat one has none inside its cover.
    pub(crate) fn of(size: (f32, f32, f32), plots: u32, stacked: bool) -> Self {
        let shelves = if stacked {
            crate::engine::plant_layout::plot_rects(size, plots, true).iter().map(|p| p.floor).collect()
        } else {
            Vec::new()
        };
        Self { size, shelves }
    }
}

/// A tent's cover in its three materials (see the module notes).
#[derive(Debug, Default)]
pub(crate) struct CoverGeometry {
    pub film: Geometry,
    pub seams: Geometry,
    pub fittings: Geometry,
}

/// How one of the cover's parts is shaded: the arguments of `Renderer::add_material_full`
/// plus `params2` (`update_material_params2`; x is the film's diffuse transmission).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PartMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub material_type: f32,
    pub params2: [f32; 4],
}

/// The film's material: its colour and face-on opacity, its roughness, no metal, and in
/// `params2` its diffuse transmission (x), the share of a lamp's light that gets through it
/// face-on (y: the shadow it casts, renderer/shadow_transmit.rs), its scattered light's spread (z:
/// how much what is behind it blurs, renderer/film_blur.rs) and the farthest behind it the blur
/// looks (w, metres), as material type 27 (a frosted sheet; never the glass type).
pub(crate) fn film_material(film: &Film) -> PartMaterial {
    let [r, g, b] = film.color;
    let transmission = film.transmission.clamp(0.0, 1.0);
    PartMaterial {
        base_color: [r, g, b, film.alpha.clamp(0.0, 1.0)],
        metallic: 0.0,
        roughness: film.roughness.clamp(0.0, 1.0),
        material_type: MATERIAL_TYPE_FILM,
        // What gets through can never be less than what gets through scattered.
        params2: [transmission, film.light_transmittance.clamp(transmission, 1.0), film.scatter_spread.max(0.0), film.blur_reach_m.max(0.0)],
    }
}

/// A welded seam, the rolled door and a port's patch: the film doubled (or more), so the
/// film's material at the seam's opacity.
pub(crate) fn seam_material(cover: &Cover) -> PartMaterial {
    let mut m = film_material(&cover.film);
    m.base_color[3] = cover.seam_alpha.clamp(m.base_color[3], 1.0);
    m
}

/// The zipper tapes, the ties and the hose grommet: opaque, in the fittings' colour, with no
/// transmission (type 27 at full opacity is plain PBR of its colour).
pub(crate) fn fittings_material(cover: &Cover) -> PartMaterial {
    let [r, g, b] = cover.fittings_color;
    PartMaterial {
        base_color: [r, g, b, 1.0],
        metallic: 0.0,
        roughness: 0.6,
        material_type: MATERIAL_TYPE_FILM,
        params2: [0.0; 4],
    }
}

/// Where a door's roll is `elapsed_s` after it began moving from `from` toward `target` (0
/// hanging down, 1 rolled up), taking `roll_s` for the whole way, eased at both ends as hands
/// roll a flap. A part of the way takes that part of the time.
pub(crate) fn roll_at(from: f32, target: f32, elapsed_s: f32, roll_s: f32) -> f32 {
    let span = (target - from).abs();
    if span < 1e-4 || roll_s <= 0.0 {
        return target;
    }
    let t = (elapsed_s / (roll_s * span)).clamp(0.0, 1.0);
    from + (target - from) * t * t * (3.0 - 2.0 * t)
}

/// Is someone tending the rack with this tent: its card pinned open (`pinned`, the machine id
/// of the card that is) and standing within `reach` metres of it on its storey (`eye`, the
/// player's eye, and `foot`, the machine's foot, in one frame)? That is when its door rolls up.
pub(crate) fn tended(pinned: Option<&str>, tent_id: &str, eye: Vec3, foot: Vec3, reach: f32) -> bool {
    let d = eye - foot;
    pinned == Some(tent_id) && (d.x * d.x + d.z * d.z).sqrt() <= reach && d.y > -0.5 && d.y < 2.6
}

/// The roll quantised to the step the door's meshes are rebuilt at (a 64th of the way: under
/// 3 cm of film on a 1.9 m tent, finer than the eye follows while it moves).
pub(crate) fn roll_step(rolled: f32) -> u32 {
    (rolled.clamp(0.0, 1.0) * 64.0).round() as u32
}

/// How far an outward offset lifts a seam, a tape or a tie off the film it lies on, metres.
const LIFT_M: f32 = 0.0012;
/// The grid's spacing between the lines a panel needs (its supports, folds, ports), metres:
/// fine enough that a panel's bow reads as a curve.
const STEP_M: f32 = 0.07;
/// How close to an edge a fold line fades out, metres (a seam holds the film flat there).
const FOLD_FADE_M: f32 = 0.06;
/// Segments round a roll, a sleeve or a ring.
const ROUND: u32 = 16;

/// One face of the cover's box in the machine's frame: a point is `o + u * a + v * b`
/// (`a` across, `b` up or back, metres), `n` its outward normal, `u x v = n`. A side face's
/// `u` runs left to right seen from outside and its `v` up.
#[derive(Debug, Clone, Copy)]
struct Face {
    o: Vec3,
    u: Vec3,
    v: Vec3,
    n: Vec3,
    lu: f32,
    lv: f32,
    side: bool,
}

impl Face {
    fn of(face: CoverFace, x: f32, h: f32, z: f32) -> Face {
        let side = |o: Vec3, u: Vec3, n: Vec3, lu: f32| Face { o, u, v: Vec3::Y, n, lu, lv: h, side: true };
        match face {
            CoverFace::Front => side(Vec3::new(-x, 0.0, z), Vec3::X, Vec3::Z, 2.0 * x),
            CoverFace::Back => side(Vec3::new(x, 0.0, -z), Vec3::NEG_X, Vec3::NEG_Z, 2.0 * x),
            CoverFace::Right => side(Vec3::new(x, 0.0, z), Vec3::NEG_Z, Vec3::X, 2.0 * z),
            CoverFace::Left => side(Vec3::new(-x, 0.0, -z), Vec3::Z, Vec3::NEG_X, 2.0 * z),
            CoverFace::Top => Face { o: Vec3::new(-x, h, z), u: Vec3::X, v: Vec3::NEG_Z, n: Vec3::Y, lu: 2.0 * x, lv: 2.0 * z, side: false },
        }
    }

    fn at(&self, a: f32, b: f32) -> Vec3 {
        self.o + self.u * a + self.v * b
    }
}

const FACES: [CoverFace; 5] = [CoverFace::Front, CoverFace::Back, CoverFace::Right, CoverFace::Left, CoverFace::Top];

/// What a face's film needs to know to drape: the cover, the supports a side panel hangs
/// between (its floor hem, the rack's shelves, its top seam) and the ports cut in it.
struct Drape<'a> {
    cover: &'a Cover,
    face: Face,
    supports: Vec<f32>,
    /// Each port in this face: (across, up, hole radius), metres.
    holes: Vec<(f32, f32, f32)>,
}

impl Drape<'_> {
    /// How far the film at (a, b) lies inside the face's plane, metres (negative: outside).
    /// A side panel bows in between its corner seams and between the shelves the frame holds it
    /// out at; the top sags in the middle; both keep the packet's fold lines, which fade out at
    /// the seams; and the film lies flat round a port, where a welded patch holds it.
    fn inset(&self, a: f32, b: f32) -> f32 {
        let f = &self.face;
        let c = self.cover;
        let across = (std::f32::consts::PI * (a / f.lu).clamp(0.0, 1.0)).sin();
        let sag = if f.side {
            let up = self.supports.windows(2).find(|w| b >= w[0] && b <= w[1]).map_or(0.0, |w| {
                let span = (w[1] - w[0]).max(1e-4);
                (std::f32::consts::PI * ((b - w[0]) / span)).sin()
            });
            c.sag_m * across * up
        } else {
            c.top_sag_m * across * (std::f32::consts::PI * (b / f.lv).clamp(0.0, 1.0)).sin()
        };
        let edge = a.min(f.lu - a).min(b).min(f.lv - b).max(0.0);
        let fade = (edge / FOLD_FADE_M).min(1.0);
        let folds = c.crease_m * (fold(a, f.lu, c) + fold(b, f.lv, c)) * fade;
        let flat = self
            .holes
            .iter()
            .map(|&(pa, pb, r)| ((((a - pa).powi(2) + (b - pb).powi(2)).sqrt() - 1.5 * r) / r.max(0.01)).clamp(0.0, 1.0))
            .fold(1.0_f32, f32::min);
        (sag + folds) * flat
    }

    /// The film's point at (a, b).
    fn point(&self, a: f32, b: f32) -> Vec3 {
        self.face.at(a, b) - self.face.n * self.inset(a, b)
    }

    /// How much more of what is behind the film hides at (a, b) for a fold line through it
    /// (`Cover::crease_white` at a fold's middle, nothing between folds): PVC whitens where it
    /// was creased. The film's vertices carry it in uv.x for the type 27 shader.
    fn whiten(&self, a: f32, b: f32) -> f32 {
        let f = &self.face;
        let c = self.cover;
        let edge = a.min(f.lu - a).min(b).min(f.lv - b).max(0.0);
        let fade = (edge / FOLD_FADE_M).min(1.0);
        let line = fold(a, f.lu, c).abs().max(fold(b, f.lv, c).abs()).min(1.0);
        c.crease_white.clamp(0.0, 1.0) * line * fade
    }
}

/// The fold lines across a length `len`: as many as the packet's pitch fits, centred, folded
/// alternately up and down (an accordion fold).
fn fold_lines(len: f32, cover: &Cover) -> Vec<(f32, f32)> {
    let pitch = cover.crease_pitch_m;
    if pitch <= 0.0 || cover.crease_m == 0.0 || len < pitch {
        return Vec::new();
    }
    let n = (len / pitch).floor() as u32;
    let first = (len - (n.saturating_sub(1)) as f32 * pitch) * 0.5;
    (0..n).map(|k| (first + k as f32 * pitch, if k % 2 == 0 { 1.0 } else { -1.0 })).collect()
}

/// The fold profile at `s` along a length `len`: each line a soft V half `crease_width_m` wide.
fn fold(s: f32, len: f32, cover: &Cover) -> f32 {
    let hw = (cover.crease_width_m * 0.5).max(1e-3);
    fold_lines(len, cover).iter().map(|&(p, sign)| sign * (1.0 - (s - p).abs() / hw).max(0.0)).sum()
}

/// The grid lines along a length `len`: both ends, every `extra` line inside, and evenly spaced
/// lines between them no further than `STEP_M` apart.
fn lines(len: f32, extra: &[f32]) -> Vec<f32> {
    let mut keys: Vec<f32> = vec![0.0, len];
    keys.extend(extra.iter().copied().filter(|&e| e > 0.0 && e < len));
    keys.sort_by(f32::total_cmp);
    keys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let mut out = Vec::new();
    for w in keys.windows(2) {
        let n = ((w[1] - w[0]) / STEP_M).ceil().max(1.0) as u32;
        for k in 0..n {
            out.push(w[0] + (w[1] - w[0]) * k as f32 / n as f32);
        }
    }
    out.push(len);
    out
}

/// The lines a face's grid needs along `len` for its folds (each line and its V's two sides)
/// and any `more`.
fn face_lines(len: f32, cover: &Cover, more: &[f32]) -> Vec<f32> {
    let hw = cover.crease_width_m * 0.5;
    let mut extra: Vec<f32> = fold_lines(len, cover).iter().flat_map(|&(p, _)| [p - hw, p, p + hw]).collect();
    extra.extend_from_slice(more);
    lines(len, &extra)
}

/// Append a grid of the drape's film over `a_lines` x `b_lines`, offset `lift` outward,
/// leaving out the cells inside a port's hole. Smooth normals from the displaced grid, wound
/// counter-clockwise from outside (`u x v = n`).
fn push_grid(out: &mut Geometry, d: &Drape, a_lines: &[f32], b_lines: &[f32], lift: f32) {
    let (na, nb) = (a_lines.len(), b_lines.len());
    if na < 2 || nb < 2 {
        return;
    }
    let p: Vec<Vec3> = b_lines
        .iter()
        .flat_map(|&b| a_lines.iter().map(move |&a| (a, b)))
        .map(|(a, b)| d.point(a, b) + d.face.n * lift)
        .collect();
    let idx = |i: usize, j: usize| j * na + i;
    let base = out.0.len() as u32;
    for j in 0..nb {
        for i in 0..na {
            let du = p[idx((i + 1).min(na - 1), j)] - p[idx(i.saturating_sub(1), j)];
            let dv = p[idx(i, (j + 1).min(nb - 1))] - p[idx(i, j.saturating_sub(1))];
            let n = du.cross(dv).try_normalize().unwrap_or(d.face.n);
            // uv.x: the fold line's whitening (`Drape::whiten`); the cover's other parts carry 0.
            let uv = [if lift == 0.0 { d.whiten(a_lines[i], b_lines[j]) } else { 0.0 }, 0.0];
            out.0.push(Vertex { position: p[idx(i, j)].to_array(), normal: n.to_array(), uv });
        }
    }
    for j in 0..nb - 1 {
        for i in 0..na - 1 {
            let (ca, cb) = ((a_lines[i] + a_lines[i + 1]) * 0.5, (b_lines[j] + b_lines[j + 1]) * 0.5);
            if d.holes.iter().any(|&(pa, pb, r)| (ca - pa).abs() < r && (cb - pb).abs() < r) {
                continue;
            }
            let (q0, q1, q2, q3) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            out.1.extend([q0, q1, q2, q2, q3, q0].map(|q| base + q as u32));
        }
    }
}

/// Append the same triangles again wound the other way with flipped normals, lifted `back`
/// along them: an opaque ribbon (drawn with back faces culled) seen from both sides.
fn push_both_sides(out: &mut Geometry, part: Geometry, back: f32) {
    let base = out.0.len() as u32;
    let n = part.0.len() as u32;
    out.0.extend(part.0.iter().copied());
    out.0.extend(part.0.iter().map(|v| {
        let nn = -Vec3::from(v.normal);
        Vertex { position: (Vec3::from(v.position) + nn * back).to_array(), normal: nn.to_array(), uv: v.uv }
    }));
    out.1.extend(part.1.iter().map(|i| base + i));
    for t in part.1.chunks_exact(3) {
        out.1.extend([t[0], t[2], t[1]].map(|i| base + n + i));
    }
}

/// A tube round the polyline `path` with radius `radius[k]` at each point, its rings turned
/// by `side` (a direction across the path), outward normals. Open at both ends.
fn push_tube(out: &mut Geometry, path: &[Vec3], radius: &[f32], side: Vec3) {
    if path.len() < 2 {
        return;
    }
    let base = out.0.len() as u32;
    let ring = ROUND + 1;
    for k in 0..path.len() {
        let t = (path[(k + 1).min(path.len() - 1)] - path[k.saturating_sub(1)]).normalize_or_zero();
        let b1 = (side - t * side.dot(t)).normalize_or_zero();
        let b2 = t.cross(b1);
        for s in 0..ring {
            let (sn, cs) = (s as f32 / ROUND as f32 * std::f32::consts::TAU).sin_cos();
            let n = b1 * cs + b2 * sn;
            out.0.push(Vertex { position: (path[k] + n * radius[k]).to_array(), normal: n.to_array(), uv: [0.0, 0.0] });
        }
    }
    for k in 0..path.len() as u32 - 1 {
        for s in 0..ROUND {
            let (a0, a1) = (base + k * ring + s, base + k * ring + s + 1);
            let (b0, b1) = (a0 + ring, a1 + ring);
            out.1.extend([a0, a1, b1, b1, b0, a0]);
        }
    }
}

/// A flat ring on a face round (a, b), from `inner` to `outer` metres, lifted `lift` off it.
fn push_ring(out: &mut Geometry, f: &Face, a: f32, b: f32, inner: f32, outer: f32, lift: f32) {
    let base = out.0.len() as u32;
    let c = f.at(a, b) + f.n * lift;
    for s in 0..=ROUND {
        let (sn, cs) = (s as f32 / ROUND as f32 * std::f32::consts::TAU).sin_cos();
        let dir = f.u * cs + f.v * sn;
        for r in [inner, outer] {
            out.0.push(Vertex { position: (c + dir * r).to_array(), normal: f.n.to_array(), uv: [0.0, 0.0] });
        }
    }
    for s in 0..ROUND {
        let (i0, o0, i1, o1) = (base + 2 * s, base + 2 * s + 1, base + 2 * s + 2, base + 2 * s + 3);
        out.1.extend([i0, o0, o1, o1, i1, i0]);
    }
}

/// A flat ribbon `width` wide along the polyline `path`, across `side`, facing `t x side`
/// (`t` the path's direction): a tie over the rolled door.
fn ribbon(path: &[Vec3], side: Vec3, width: f32) -> Geometry {
    let (mut v, mut ix) = (Vec::new(), Vec::new());
    for k in 0..path.len() {
        let t = (path[(k + 1).min(path.len() - 1)] - path[k.saturating_sub(1)]).normalize_or_zero();
        let n = t.cross(side).normalize_or_zero();
        for s in [-0.5_f32, 0.5] {
            v.push(Vertex { position: (path[k] + side * (s * width)).to_array(), normal: n.to_array(), uv: [0.0, 0.0] });
        }
    }
    for k in 0..path.len().saturating_sub(1) as u32 {
        let (a0, a1, b0, b1) = (2 * k, 2 * k + 1, 2 * k + 2, 2 * k + 3);
        ix.extend([a0, b0, b1, b1, a1, a0]);
    }
    (v, ix)
}

/// How much film the door holds rolled when it is all the way up, metres, and the radius of
/// the roll of `rolled_m` of it: a loose roll of `core` radius growing by a `layer` each turn.
fn roll_radius(rolled_m: f32, core: f32, layer: f32) -> f32 {
    (core * core + rolled_m.max(0.0) * layer.max(0.0) / std::f32::consts::PI).sqrt()
}

/// The longest length of door that rolls up under a top seam at `top`: the roll's bottom at
/// the length rolled, its top `2 R` above, no higher than `top`.
fn max_rolled(top: f32, core: f32, layer: f32) -> f32 {
    let (mut lo, mut hi) = (0.0_f32, top.max(0.0));
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if mid + 2.0 * roll_radius(mid, core, layer) <= top {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The cover of a tent of `tent` size (width, height, depth, metres; `Enclosure::size`) fitted
/// to `frame`, made as `cover` says, its door rolled up by `rolled` (0 down, 1 all the way up).
/// See the module notes for what goes into which of its three parts.
pub(crate) fn cover_geometry(tent: (f32, f32, f32), frame: &RackFrame, cover: &Cover, rolled: f32) -> CoverGeometry {
    let (x, h, z) = (tent.0.max(0.05) * 0.5, tent.1.max(0.05), tent.2.max(0.05) * 0.5);
    let mut g = CoverGeometry::default();
    let seam = cover.seam_m.max(0.0);
    let door = &cover.door;
    let door_face = if door.face == CoverFace::Top { CoverFace::Front } else { door.face };
    // The side panels hang from the top seam to the floor hem and are held out at each shelf.
    let mut supports = vec![0.0];
    supports.extend(frame.shelves.iter().copied().filter(|&s| s > 0.02 && s < h - 0.02));
    supports.push(h);
    for which in FACES {
        let f = Face::of(which, x, h, z);
        let holes: Vec<(f32, f32, f32)> =
            cover.ports.iter().filter(|p| p.face == which).map(|p| (p.at.0 * f.lu, p.at.1 * f.lv, p.radius_m)).collect();
        let d = Drape { cover, face: f, supports: if f.side { supports.clone() } else { Vec::new() }, holes };
        let mut more: Vec<f32> = d.holes.iter().flat_map(|&(pa, _, r)| [pa - r, pa + r]).collect();
        let mut more_b: Vec<f32> = d.holes.iter().flat_map(|&(_, pb, r)| [pb - r, pb + r]).collect();
        if f.side {
            more_b.extend_from_slice(&d.supports);
        }
        let is_door = which == door_face;
        let inset = door.zip_inset_m.max(seam).min(f.lu * 0.25);
        if is_door {
            more.extend([inset, f.lu - inset]);
        }
        let a_lines = face_lines(f.lu, cover, &more);
        let b_lines = face_lines(f.lv, cover, &more_b);
        if is_door {
            // The two fixed strips beside the zips; the door between them is built below.
            let left: Vec<f32> = a_lines.iter().copied().filter(|&a| a <= inset + 1e-4).collect();
            let right: Vec<f32> = a_lines.iter().copied().filter(|&a| a >= f.lu - inset - 1e-4).collect();
            push_grid(&mut g.film, &d, &left, &b_lines, 0.0);
            push_grid(&mut g.film, &d, &right, &b_lines, 0.0);
            push_door(&mut g, &d, &a_lines, &b_lines, inset, rolled);
        } else {
            push_grid(&mut g.film, &d, &a_lines, &b_lines, 0.0);
        }
        push_seams(&mut g.seams, &d, &a_lines, &b_lines, seam, is_door, inset, rolled);
        push_ports(&mut g, &d, which);
    }
    g
}

/// The welded bands along a face's edges: half a seam on each face either side of a corner,
/// the hem along a side panel's foot. On the door's face the hem is on the fixed strips, and
/// on the door too while it hangs down.
#[allow(clippy::too_many_arguments)]
fn push_seams(out: &mut Geometry, d: &Drape, a_lines: &[f32], b_lines: &[f32], seam: f32, is_door: bool, inset: f32, rolled: f32) {
    if seam <= 0.0 {
        return;
    }
    let f = &d.face;
    let half = seam * 0.5;
    let within = |ls: &[f32], lo: f32, hi: f32| -> Vec<f32> {
        let mut v: Vec<f32> = ls.iter().copied().filter(|&s| s > lo && s < hi).collect();
        v.insert(0, lo);
        v.push(hi);
        v
    };
    let full_a = a_lines.to_vec();
    let full_b = b_lines.to_vec();
    // Left and right edges, the top edge (the bottom one too on the top face).
    push_grid(out, d, &within(&full_a, 0.0, half), &full_b, LIFT_M);
    push_grid(out, d, &within(&full_a, f.lu - half, f.lu), &full_b, LIFT_M);
    push_grid(out, d, &full_a, &within(&full_b, f.lv - half, f.lv), LIFT_M);
    if !f.side {
        push_grid(out, d, &full_a, &within(&full_b, 0.0, half), LIFT_M);
        return;
    }
    let hem = within(&full_b, 0.0, seam * 1.5);
    if is_door {
        push_grid(out, d, &within(&full_a, 0.0, inset), &hem, LIFT_M);
        push_grid(out, d, &within(&full_a, f.lu - inset, f.lu), &hem, LIFT_M);
        if rolled <= 0.0 {
            push_grid(out, d, &within(&full_a, inset, f.lu - inset), &hem, LIFT_M);
        }
    } else {
        push_grid(out, d, &full_a, &hem, LIFT_M);
    }
}

/// The door: the film from its top seam down to the roll (all of it while it hangs down), the
/// roll of film it is wound into, the zip tapes up both its sides (the fixed half the whole
/// height, the door's half as far as the door still hangs) and the ties: hanging loose from the
/// top seam while it is down, round the roll while it is up.
fn push_door(g: &mut CoverGeometry, d: &Drape, a_lines: &[f32], b_lines: &[f32], inset: f32, rolled: f32) {
    let f = &d.face;
    let c = d.cover;
    let door = &c.door;
    let top = f.lv - c.seam_m.max(0.0) * 2.0 - 0.01;
    let rolled_m = max_rolled(top, door.roll_core_m, door.roll_layer_m) * rolled.clamp(0.0, 1.0);
    let rolled_up = rolled_m > 0.005;
    let r = roll_radius(rolled_m, door.roll_core_m, door.roll_layer_m);
    // The door's foot: the floor while it hangs, the top of its roll's tangent once rolled.
    let foot = if rolled_up { rolled_m + r } else { 0.0 };
    let (a0, a1) = (inset, f.lu - inset);
    let door_a: Vec<f32> = a_lines.iter().copied().filter(|&a| a >= a0 - 1e-4 && a <= a1 + 1e-4).collect();
    let mut door_b: Vec<f32> = b_lines.iter().copied().filter(|&b| b > foot + 1e-4).collect();
    door_b.insert(0, foot);
    // Rolled, the film comes off the roll flat: its drape fades in over the first 10 cm.
    let hang = d;
    if rolled_up {
        let mut part: Geometry = (Vec::new(), Vec::new());
        push_grid(&mut part, hang, &door_a, &door_b, 0.0);
        for (vtx, &b) in part.0.iter_mut().zip(door_b.iter().flat_map(|b| std::iter::repeat(b).take(door_a.len()))) {
            let keep = ((b - foot) / 0.1).clamp(0.0, 1.0);
            let flat = f.at(0.0, b).dot(f.n);
            let p = Vec3::from(vtx.position);
            let off = p.dot(f.n) - flat;
            vtx.position = (p - f.n * off * (1.0 - keep)).to_array();
        }
        let base = g.film.0.len() as u32;
        g.film.0.extend(part.0);
        g.film.1.extend(part.1.iter().map(|i| base + i));
        // The roll, a loose cylinder in front of the door's plane, under the film coming off it.
        let centre = |a: f32| f.at(a, rolled_m + r) + f.n * r;
        push_tube(&mut g.seams, &[centre(a0), centre(0.5 * (a0 + a1)), centre(a1)], &[r, r, r], f.n);
    } else {
        push_grid(&mut g.film, hang, &door_a, &door_b, 0.0);
    }
    // The zip tapes: the fixed half beside each zip the whole height, the door's half as far
    // as the door hangs.
    let zip_half = (door.zip_width_m * 0.5).max(0.0);
    let zip_top = f.lv - c.seam_m.max(0.0);
    let tape = |out: &mut Geometry, a_lo: f32, a_hi: f32, b_lo: f32| {
        let bl: Vec<f32> = std::iter::once(b_lo).chain(b_lines.iter().copied().filter(|&b| b > b_lo && b < zip_top)).chain(std::iter::once(zip_top)).collect();
        let mut part: Geometry = (Vec::new(), Vec::new());
        push_grid(&mut part, hang, &[a_lo, a_hi], &bl, LIFT_M * 1.5);
        push_both_sides(out, part, 0.0008);
    };
    if zip_half > 0.0 {
        tape(&mut g.fittings, a0 - zip_half, a0, 0.0);
        tape(&mut g.fittings, a1, a1 + zip_half, 0.0);
        tape(&mut g.fittings, a0, a0 + zip_half, foot);
        tape(&mut g.fittings, a1 - zip_half, a1, foot);
    }
    // The ties, evenly across the door.
    let ties = door.ties.min(8);
    for k in 0..ties {
        let a = a0 + (a1 - a0) * (k + 1) as f32 / (ties + 1) as f32;
        let lift = LIFT_M * 2.5;
        let path: Vec<Vec3> = if rolled_up {
            // From the top seam down onto the roll, over its front, under it and up its back.
            let rr = r + lift;
            let centre = f.at(a, rolled_m + r) + f.n * r;
            let mut p = vec![f.at(a, zip_top) + f.n * lift];
            for s in 0..=12 {
                let th = std::f32::consts::FRAC_PI_2 - s as f32 / 12.0 * 1.5 * std::f32::consts::PI;
                p.push(centre + (f.n * th.cos() + f.v * th.sin()) * rr);
            }
            p
        } else {
            // Hanging down the door from the top seam.
            [0.0, 0.5, 1.0].iter().map(|t| hang.point(a, zip_top - door.tie_m * t) + f.n * lift).collect()
        };
        push_both_sides(&mut g.fittings, ribbon(&path, f.u, door.tie_width_m), 0.0008);
    }
}

/// A face's ports: a vent sleeve (a tube of film welded round its hole on a patch, hanging out
/// and drooping, gathered and tied at its end until a duct is pushed into it) or a grommet (an
/// opaque ring round a small hole, the humidifier's refill tubing coming out of it and down).
fn push_ports(g: &mut CoverGeometry, d: &Drape, which: CoverFace) {
    let f = &d.face;
    for p in d.cover.ports.iter().filter(|p| p.face == which) {
        let (pa, pb, r) = (p.at.0 * f.lu, p.at.1 * f.lv, p.radius_m.max(0.002));
        let mouth = f.at(pa, pb);
        // The way down along the face (the top face's "down" is out over its front edge).
        let down = if f.side { -f.v } else { f.v };
        match p.kind {
            PortKind::Sleeve => {
                push_ring(&mut g.seams, f, pa, pb, r, r * 1.5, LIFT_M);
                let len = p.length_m.max(0.0);
                let ts = [0.0_f32, 0.25, 0.5, 0.7, 0.85, 1.0];
                let path: Vec<Vec3> = ts.iter().map(|&t| mouth + f.n * (len * t * 0.85) + down * (len * 0.35 * t * t)).collect();
                // Full bore, then gathered to a third at its tied end.
                let radius: Vec<f32> = ts.iter().map(|&t| r * if t <= 0.5 { 1.0 } else { 1.0 - 0.7 * ((t - 0.5) / 0.5).powf(0.7) }).collect();
                push_tube(&mut g.film, &path, &radius, f.u);
                let tie_at = |t: f32| mouth + f.n * (len * t * 0.85) + down * (len * 0.35 * t * t);
                let tie_r = r * (1.0 - 0.7 * (0.35_f32 / 0.5).powf(0.7)) + LIFT_M * 2.0;
                push_tube(&mut g.fittings, &[tie_at(0.81), tie_at(0.89)], &[tie_r, tie_r], f.u);
            }
            PortKind::Grommet => {
                push_ring(&mut g.fittings, f, pa, pb, r, r * 1.7, LIFT_M);
                let len = p.length_m.max(0.0);
                let ts = [0.0_f32, 0.2, 0.45, 0.7, 1.0];
                let path: Vec<Vec3> = ts.iter().map(|&t| mouth + f.n * (len * 0.3 * (1.0 - (1.0 - t).powi(2))) + down * (len * t * t)).collect();
                push_tube(&mut g.fittings, &path, &vec![r * 0.8; ts.len()], f.u);
            }
        }
    }
}

#[cfg(test)]
#[path = "fruiting_tent_tests.rs"]
mod tests;
