//! ROOM GI, RUNG 1: per-room irradiance probes (docs/design/room-gi.md).
//!
//! WHAT WAS WRONG. Ship interiors had no indirect light at all. The interior
//! passes zero the camera-uniform pad that carries the local up, so the shared
//! PBR tail decides the fragment is not under a sky, `sky_ambient` returns 0,
//! and the indirect term collapses to the 0.005 silhouette floor. A wall that
//! no lamp faces directly rendered at the floor (sRGB 3.7 in the 25b mushroom
//! capture) while the floor beside it read 147. A real room at mean
//! reflectance 0.5 returns roughly a quarter of the floor's light to its walls
//! by interreflection alone (the integrating-sphere relation
//! `E_ind = rho * Phi / (A * (1 - rho))`).
//!
//! THE DESTINATION is DDGI: irradiance probes updated by ray tracing every
//! frame (Majercik et al., "Dynamic Diffuse Global Illumination with Ray-Traced
//! Irradiance Fields", JCGT 8(2) 2019; probe relocation and classification,
//! JCGT 10(2) 2021), one probe grid PER ROOM. A fragment samples only its own
//! room's grid, so DDGI's classic failure, light leaking through a thin wall
//! from the probe on the other side, cannot happen by construction: the probe
//! on the other side belongs to a different grid.
//!
//! RUNG 1 (this file and its GPU half, `room_probes_gpu.rs`). Each room's
//! probes trace their 64 rays against the ROOM'S OWN BOX, analytically, not
//! against its contents. That is the real architecture minus the scene
//! representation: the storage format (8x8 octahedral irradiance plus 16x16
//! depth moments per probe, one atlas, a 1-texel border), the update (rotated
//! ray set, hysteresis, infinite bounce through the previous frame's probes),
//! and the sampling (8 probes, Chebyshev visibility, backface weight, normal
//! bias) are all DDGI as published. Rung 2 replaces the box intersection with
//! a trace against a voxelization of the room's contents; nothing else here
//! changes when it does.
//!
//! THIS FILE is the layout, the math and a CPU TWIN of the update and the
//! sampling. The twin exists so the physics can be tested without a GPU: the
//! WGSL (`assets/shaders/pbr/85-room-gi.wgsl` for sampling, shared by both
//! shaders, and `assets/shaders/room_probes_update.wgsl` for the update) is a
//! transcription of the functions below, name for name. Change one, change the
//! other in the same edit.
//!
//! UNITS. Nothing new. A probe stores the COSINE-WEIGHTED MEAN RADIANCE over
//! its hemisphere (E / pi), the same quantity `sky_ambient` returns, so the
//! shared tail's `ambient = albedo * indirect * ao` reads it directly. Hit
//! points are shaded with exactly the light the fragment loop evaluates (the
//! same attenuation, the same cone, the same line-light closest point), taken
//! as Lambertian, `rho / pi * E`. `watts` is not read anywhere.

use glam::{Mat3, Vec2, Vec3};

use super::light::{line_light_closest_point, spot_cone_attenuation, RoomLight};

/// Interior octahedral irradiance texels per side (DDGI: 6 to 8).
pub const IRR_N: u32 = 8;
/// Interior octahedral depth-moment texels per side (DDGI: 14 to 16).
pub const DEPTH_N: u32 = 16;
/// A tile is its interior plus a 1-texel border on each side, the border
/// holding the octahedral wrap so a bilinear tap at the seam reads the right
/// neighbour.
pub const IRR_TILE: u32 = IRR_N + 2;
pub const DEPTH_TILE: u32 = DEPTH_N + 2;
/// Rays traced per probe per update. The workgroup in the compute shader is
/// exactly this wide, one ray per thread.
pub const RAYS_PER_PROBE: u32 = 64;
/// Target probe spacing (metres). The mushroom room, about 10 x 3 x 10 m,
/// gets an 11 x 4 x 11 lattice.
pub const PROBE_SPACING_M: f32 = 1.0;
/// Temporal hysteresis once a probe has settled (DDGI's 0.97). Before that a
/// probe keeps a running mean (see `blend_weight_h`).
pub const HYSTERESIS: f32 = 0.97;
/// A probe's update count saturates here (an f16 holds every integer to 2048 exactly).
pub const MAX_UPDATE_COUNT: f32 = 255.0;
/// Sample point offset along the surface normal (metres). DDGI's normal bias.
pub const NORMAL_BIAS_M: f32 = 0.1;
/// DDGI's depth lobe exponent: how sharply each depth texel weights the rays
/// near its own direction.
pub const DEPTH_SHARPNESS: f32 = 50.0;
/// A ray further than acos(0.9) = 26 degrees from a depth texel's direction
/// contributes under 0.5% (0.9^50) of a centred ray's weight, so it is skipped
/// outright. Same cut in the WGSL.
pub const DEPTH_MIN_COS: f32 = 0.9;
/// DDGI's weight crush: weights under this are cubed-and-scaled toward zero so
/// a barely visible probe cannot tint the result.
pub const CRUSH_THRESHOLD: f32 = 0.2;
/// Which room a fragment belongs to is decided at `p + n * ROOM_PICK_OFFSET_M`
/// (half a metre into whatever the surface faces), then accepted only if that
/// point lies within `ROOM_PICK_MARGIN_M` of a room box. An interior wall face
/// therefore picks the room it faces, and the OUTSIDE face of the hull, whose
/// pick point lands half a metre out in space, picks nothing.
pub const ROOM_PICK_OFFSET_M: f32 = 0.5;
pub const ROOM_PICK_MARGIN_M: f32 = 0.25;
/// Atlas width in texels: a multiple of both tile sizes (10 and 18), under the
/// 8192 texture-dimension floor `wgpu::Limits::default()` guarantees.
pub const ATLAS_WIDTH: u32 = 8100;
/// Upper bound on probes across the whole ship: the default ship holds 31,874
/// (the acre about 22k at 1 m, the commons the rest); past this every room's
/// spacing grows until the lattice fits, so the atlas stays under about 110 MB.
pub const MAX_TOTAL_PROBES: u32 = 32_768;
/// Upper bound on one room's probes; a bigger room coarsens alone.
pub const MAX_PROBES_PER_ROOM: u32 = 8_192;
/// The six faces of a room box, in the order every per-face array uses.
pub const FACE_NEG_X: usize = 0;
pub const FACE_POS_X: usize = 1;
pub const FACE_FLOOR: usize = 2;
pub const FACE_LID: usize = 3;
pub const FACE_NEG_Z: usize = 4;
pub const FACE_POS_Z: usize = 5;

/// One room as the probe system sees it: an axis-aligned box and what its six
/// faces reflect. Built by `engine::room_gi` from the ship's detected rooms,
/// with each vertical face snapped out to the wall surface it stands against.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomBox {
    pub min: Vec3,
    pub max: Vec3,
    /// Diffuse reflectance (linear RGB) per face, in FACE order: -x, +x,
    /// floor (-y), lid (+y), -z, +z. From data/blueprints/wall_materials.ron.
    pub refl: [[f32; 3]; 6],
    /// Fraction of light the lid lets through; 0 for an opaque roof. The
    /// renderer draws glass with an alpha blend that passes `1 - alpha` of
    /// whatever is behind the pane, so that is what the engine passes here.
    pub lid_transmittance: f32,
}

impl RoomBox {
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// The same box moved by `off` (home frame to render space).
    pub fn shifted(&self, off: Vec3) -> RoomBox {
        RoomBox { min: self.min + off, max: self.max + off, ..self.clone() }
    }

    /// True when `p` lies inside the box grown by `margin` on every side.
    pub fn contains(&self, p: Vec3, margin: f32) -> bool {
        p.cmpge(self.min - Vec3::splat(margin)).all() && p.cmple(self.max + Vec3::splat(margin)).all()
    }

    /// Distance from `p` to the box (0 inside).
    pub fn outside_distance(&self, p: Vec3) -> f32 {
        (self.min - p).max(p - self.max).max(Vec3::ZERO).length()
    }

    pub fn volume(&self) -> f32 {
        let s = self.size();
        s.x * s.y * s.z
    }
}

/// Probes per axis for a box of `size` at `spacing`: ceil(size / spacing) + 1,
/// never fewer than 2 (trilinear interpolation needs a pair on every axis).
/// The small subtraction keeps an exact 10.0 m from rounding up to 12 probes.
pub fn probe_counts(size: Vec3, spacing: f32) -> [u32; 3] {
    let f = |d: f32| (((d / spacing) - 1.0e-3).ceil().max(0.0) as u32 + 1).max(2);
    [f(size.x), f(size.y), f(size.z)]
}

/// Probes sit at CELL CENTRES: the box is cut into `counts` equal cells per
/// axis and each probe is at a cell's middle, so the outermost layer stands
/// half a cell off every wall and never sits on a surface.
pub fn probe_position(room: &RoomBox, counts: [u32; 3], grid: [u32; 3]) -> Vec3 {
    let cell = room.size() / Vec3::new(counts[0] as f32, counts[1] as f32, counts[2] as f32);
    room.min + (Vec3::new(grid[0] as f32, grid[1] as f32, grid[2] as f32) + 0.5) * cell
}

/// Local probe index -> grid coordinate (x fastest, then y, then z).
pub fn probe_grid(counts: [u32; 3], local: u32) -> [u32; 3] {
    [local % counts[0], (local / counts[0]) % counts[1], local / (counts[0] * counts[1])]
}

/// Probe counts for every room. Each room starts at `PROBE_SPACING_M`; a room
/// that alone would hold more than `MAX_PROBES_PER_ROOM` (the 8 m tall commons
/// hall, 17,640 at 1 m) coarsens by itself, so one hall cannot coarsen every
/// bedroom; then, only if the ship still does not fit `MAX_TOTAL_PROBES`,
/// every room coarsens together. Returns (the finest spacing used, counts per
/// room).
pub fn lattice_for(rooms: &[RoomBox]) -> (f32, Vec<[u32; 3]>) {
    let n = |c: [u32; 3]| c[0] as u64 * c[1] as u64 * c[2] as u64;
    let mut spacing: Vec<f32> = rooms
        .iter()
        .map(|r| {
            let mut s = PROBE_SPACING_M;
            while n(probe_counts(r.size(), s)) > MAX_PROBES_PER_ROOM as u64 && s < 64.0 {
                s *= 1.1;
            }
            s
        })
        .collect();
    loop {
        let counts: Vec<[u32; 3]> = rooms.iter().zip(&spacing).map(|(r, s)| probe_counts(r.size(), *s)).collect();
        let total: u64 = counts.iter().map(|c| n(*c)).sum();
        let finest = spacing.iter().copied().fold(f32::INFINITY, f32::min);
        if total <= MAX_TOTAL_PROBES as u64 || finest > 64.0 {
            return (if finest.is_finite() { finest } else { PROBE_SPACING_M }, counts);
        }
        for s in &mut spacing {
            *s *= 1.1;
        }
    }
}

/// Where each probe's two tiles live. The irradiance tiles fill the top of the
/// atlas row by row; the depth tiles start below them at `depth_y0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasLayout {
    pub width: u32,
    pub height: u32,
    pub irr_per_row: u32,
    pub depth_per_row: u32,
    pub depth_y0: u32,
    pub probes: u32,
}

impl AtlasLayout {
    /// A layout holding `probes` probes (at least one, so every texture and
    /// binding stays non-empty).
    pub fn for_probes(probes: u32) -> Self {
        let probes = probes.max(1);
        let width = ATLAS_WIDTH;
        let irr_per_row = width / IRR_TILE;
        let depth_per_row = width / DEPTH_TILE;
        let irr_rows = probes.div_ceil(irr_per_row);
        let depth_rows = probes.div_ceil(depth_per_row);
        let depth_y0 = irr_rows * IRR_TILE;
        AtlasLayout {
            width,
            height: depth_y0 + depth_rows * DEPTH_TILE,
            irr_per_row,
            depth_per_row,
            depth_y0,
            probes,
        }
    }

    /// Texel of the TOP-LEFT BORDER texel of probe `p`'s irradiance tile.
    pub fn irr_origin(&self, p: u32) -> (u32, u32) {
        ((p % self.irr_per_row) * IRR_TILE, (p / self.irr_per_row) * IRR_TILE)
    }

    /// Texel of the top-left border texel of probe `p`'s depth tile.
    pub fn depth_origin(&self, p: u32) -> (u32, u32) {
        ((p % self.depth_per_row) * DEPTH_TILE, self.depth_y0 + (p / self.depth_per_row) * DEPTH_TILE)
    }

    pub fn texel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

// ── Octahedral mapping (Cigolle et al., "A Survey of Efficient Representations
//    for Independent Unit Vectors", JCGT 3(2) 2014; the form DDGI uses) ──

fn sign_not_zero(v: f32) -> f32 {
    if v >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Unit direction -> octahedral coordinate in [-1, 1]^2. +z is the pole of the
/// inner diamond; the -z hemisphere folds into the four corners.
pub fn oct_encode(d: Vec3) -> Vec2 {
    let l1 = d.x.abs() + d.y.abs() + d.z.abs();
    let p = Vec2::new(d.x, d.y) / l1.max(1e-20);
    if d.z < 0.0 {
        Vec2::new((1.0 - p.y.abs()) * sign_not_zero(p.x), (1.0 - p.x.abs()) * sign_not_zero(p.y))
    } else {
        p
    }
}

/// Octahedral coordinate -> unit direction. Exact inverse of `oct_encode`.
pub fn oct_decode(p: Vec2) -> Vec3 {
    let mut v = Vec3::new(p.x, p.y, 1.0 - p.x.abs() - p.y.abs());
    if v.z < 0.0 {
        let (x, y) = (v.x, v.y);
        v.x = (1.0 - y.abs()) * sign_not_zero(x);
        v.y = (1.0 - x.abs()) * sign_not_zero(y);
    }
    v.normalize()
}

/// The direction an interior texel (x, y) of an N x N octahedral tile stands
/// for: its centre, decoded.
pub fn texel_direction(n: u32, x: u32, y: u32) -> Vec3 {
    let uv = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
    oct_decode(uv)
}

/// Which interior texel a tile texel (bx, by), border included (0..=n+1 on
/// each axis), shows. Interior texels show themselves; a border texel shows the
/// interior texel across the octahedral seam: the edge mirrored, the corner the
/// diagonally opposite corner (DDGI's border copy).
pub fn border_source(n: u32, bx: u32, by: u32) -> (u32, u32) {
    let last = n + 1;
    let ix = |b: u32| b.clamp(1, n) - 1;
    match (bx, by) {
        (0, 0) => (n - 1, n - 1),
        (b, 0) if b == last => (0, n - 1),
        (0, b) if b == last => (n - 1, 0),
        (a, b) if a == last && b == last => (0, 0),
        (_, 0) => (n - bx, 0),
        (_, b) if b == last => (n - bx, n - 1),
        (0, _) => (0, n - by),
        (a, _) if a == last => (n - 1, n - by),
        _ => (ix(bx), ix(by)),
    }
}

/// Texel coordinates (continuous, texel units) of direction `d` inside a tile
/// whose top-left border texel is `origin`: the interior spans [1, n+1].
pub fn tile_coord(origin: (u32, u32), n: u32, d: Vec3) -> Vec2 {
    let oct = oct_encode(d);
    Vec2::new(origin.0 as f32, origin.1 as f32) + 1.0 + (oct * 0.5 + 0.5) * n as f32
}

// ── The ray set ──

/// Ray `i` of an `n`-ray spherical Fibonacci set (evenly spread over the
/// sphere, DDGI's choice).
pub fn fibonacci_ray(i: u32, n: u32) -> Vec3 {
    // The golden angle as a fraction of a turn: frac(i * (sqrt(5) - 1) / 2).
    let phi = std::f32::consts::TAU * ((i as f32 * 0.618_034) % 1.0);
    let cos_t = 1.0 - (2.0 * i as f32 + 1.0) / n as f32;
    let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
    Vec3::new(phi.cos() * sin_t, phi.sin() * sin_t, cos_t)
}

/// A uniformly random rotation for update number `frame` (Shoemake's method on
/// three hashed uniforms), so successive updates see different directions and
/// the hysteresis integrates them into a full-sphere estimate.
pub fn ray_rotation(frame: u32) -> Mat3 {
    let h = |k: u32| {
        let mut x = frame.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 16;
        x = x.wrapping_mul(0x7FEB_352D);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846C_A68B);
        x ^= x >> 16;
        (x >> 8) as f32 / (1u32 << 24) as f32
    };
    let (u1, u2, u3) = (h(1), h(2), h(3));
    let (a, b) = ((1.0 - u1).sqrt(), u1.sqrt());
    let tau = std::f32::consts::TAU;
    let q = glam::Quat::from_xyzw(a * (tau * u2).sin(), a * (tau * u2).cos(), b * (tau * u3).sin(), b * (tau * u3).cos());
    Mat3::from_quat(q.normalize())
}

// ── Tracing and shading, against the room's own box ──

/// Inward normal of each face.
pub fn face_normal(face: usize) -> Vec3 {
    match face {
        FACE_NEG_X => Vec3::X,
        FACE_POS_X => Vec3::NEG_X,
        FACE_FLOOR => Vec3::Y,
        FACE_LID => Vec3::NEG_Y,
        FACE_NEG_Z => Vec3::Z,
        _ => Vec3::NEG_Z,
    }
}

/// Distance along `d` from `p` (inside the box) to the wall it hits, and which
/// face that is. Analytic: the nearest of the three exit planes.
pub fn trace_box(p: Vec3, d: Vec3, min: Vec3, max: Vec3) -> (f32, usize) {
    let mut best = (f32::INFINITY, FACE_FLOOR);
    let axes = [(d.x, p.x, min.x, max.x, FACE_NEG_X, FACE_POS_X), (d.y, p.y, min.y, max.y, FACE_FLOOR, FACE_LID), (d.z, p.z, min.z, max.z, FACE_NEG_Z, FACE_POS_Z)];
    for (dc, pc, lo, hi, f_lo, f_hi) in axes {
        let (t, f) = if dc > 1e-8 {
            ((hi - pc) / dc, f_hi)
        } else if dc < -1e-8 {
            ((lo - pc) / dc, f_lo)
        } else {
            continue;
        };
        if t < best.0 {
            best = (t.max(0.0), f);
        }
    }
    best
}

/// The irradiance ONE light delivers to a surface at `p` with normal `n`: the
/// fragment loop in 80-fragment-shared.wgsl without the BRDF (the same line
/// light closest point, range cut, `intensity / (1 + d^2)` falloff with the
/// linear range window, spot cone, and the 0.001 attenuation floor).
pub fn light_irradiance(l: &RoomLight, p: Vec3, n: Vec3) -> Vec3 {
    let mut pos = l.pos;
    if l.cos_outer < -1.5 {
        pos = line_light_closest_point(l.pos, l.dir, p);
    }
    let to_light = pos - p;
    let dist = to_light.length();
    if dist >= l.range {
        return Vec3::ZERO;
    }
    let dir = to_light / dist.max(0.001);
    let mut att = l.intensity / (1.0 + dist * dist) * (1.0 - dist / l.range.max(0.001)).max(0.0);
    if l.cos_outer > -1.0 {
        att *= spot_cone_attenuation(-dir, l.dir, l.cos_inner, l.cos_outer);
    }
    if att <= 0.001 {
        return Vec3::ZERO;
    }
    Vec3::from(l.color) * att * n.dot(dir).max(0.0)
}

/// The point a light is judged to be IN, for giving it to a room: its position,
/// or a line light's midpoint.
pub fn light_anchor(l: &RoomLight) -> Vec3 {
    if l.cos_outer < -1.5 {
        (l.pos + l.dir) * 0.5
    } else {
        l.pos
    }
}

/// Each room's lights: every light whose anchor lies inside the room box
/// (grown by 5 cm so a lamp mounted flush on the ceiling still counts). A light
/// belongs to the room it is in and no other, which is what keeps one room's
/// lamp out of the next room's probes. Returns, per room, indices into
/// `lights`.
pub fn assign_lights(rooms: &[RoomBox], lights: &[RoomLight]) -> Vec<Vec<usize>> {
    rooms
        .iter()
        .map(|r| (0..lights.len()).filter(|&i| r.contains(light_anchor(&lights[i]), 0.05)).collect())
        .collect()
}

/// The sun as the probe update sees it: direction toward the sun, colour, and
/// intensity, exactly `Renderer::cur_sun`.
#[derive(Debug, Clone, Copy)]
pub struct Sun {
    pub dir: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
}

/// Transmittance of the path from `h` (on a face with inward normal `n`)
/// toward the sun: the lid's transmittance if the ray leaves the box through
/// the lid and the lid is glass, otherwise 0 (a wall or an opaque roof is in
/// the way). The box is convex, so leaving through the lid is the whole test.
pub fn sun_path(room: &RoomBox, h: Vec3, n: Vec3, sun_dir: Vec3) -> f32 {
    if room.lid_transmittance <= 0.0 || sun_dir.y <= 0.0 || n.dot(sun_dir) <= 0.0 {
        return 0.0;
    }
    let (_, face) = trace_box(h + n * 1.0e-3, sun_dir, room.min, room.max);
    if face == FACE_LID {
        room.lid_transmittance
    } else {
        0.0
    }
}

/// Which room a surface at `p` with normal `n` samples, or None. Decided at
/// the pick point half a metre along the normal. `rooms` is in PICK ORDER
/// (`pick_order`): the first box that contains the point wins; a point in no
/// box takes the nearest one within the pick margin.
pub fn pick_room(rooms: &[RoomBox], p: Vec3, n: Vec3) -> Option<usize> {
    let q = p + n * ROOM_PICK_OFFSET_M;
    let mut best: Option<usize> = None;
    let mut best_d = ROOM_PICK_MARGIN_M + 1.0e-3;
    for (i, r) in rooms.iter().enumerate() {
        let d = r.outside_distance(q);
        if d <= 0.0 {
            return Some(i);
        }
        if d < best_d {
            best = Some(i);
            best_d = d;
        }
    }
    best
}

/// The order the room table is written in each frame, which is the order the
/// pick walks it (the first box containing the point wins). Two rules:
/// - COST: nearest the eye first (the rooms holding the eye at distance 0,
///   smallest first), because what is on screen is mostly the room you stand
///   in and the ones you see into, so most fragments stop within a step or two.
///   Ordering by size alone put the greenhouse, seen through the great room's
///   glass wall, behind thirty smaller rooms.
/// - CORRECTNESS: a room whose box lies inside another's comes BEFORE it. The
///   commons hall's box (a ring of corridor around the small rooms) contains
///   nine of them; a fragment in one of those must not pick the hall just
///   because the eye stands in the hall.
pub fn pick_order(rooms: &[RoomBox], eye: Vec3) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rooms.len()).collect();
    order.sort_by(|&a, &b| {
        let (da, db) = (rooms[a].outside_distance(eye), rooms[b].outside_distance(eye));
        da.total_cmp(&db).then(rooms[a].volume().total_cmp(&rooms[b].volume()))
    });
    let inside = |inner: &RoomBox, outer: &RoomBox| {
        inner.min.cmpge(outer.min - Vec3::splat(1e-3)).all() && inner.max.cmple(outer.max + Vec3::splat(1e-3)).all()
    };
    // Move every contained room ahead of its container. Bounded: each move
    // puts a room strictly earlier, and containment is acyclic up to equal
    // boxes, which the volume tie-break already ordered.
    let mut moved = true;
    let mut guard = 0;
    while moved && guard < rooms.len() * rooms.len() + 1 {
        moved = false;
        guard += 1;
        'scan: for i in 0..order.len() {
            for j in i + 1..order.len() {
                let (a, b) = (order[i], order[j]);
                if a != b && inside(&rooms[b], &rooms[a]) && rooms[b].volume() < rooms[a].volume() {
                    let r = order.remove(j);
                    order.insert(i, r);
                    moved = true;
                    break 'scan;
                }
            }
        }
    }
    order
}

/// The weight the NEW estimate gets in the blend (1 - hysteresis), given how
/// many updates the probe has had: a running mean over its first updates (1,
/// 1/2, 1/3, ... from a cold atlas, so a fresh probe is right after one update
/// instead of creeping up at 3% a frame), settling at `1 - hysteresis` once
/// `count / (count + 1)` reaches it (about 32 updates at 0.97). The count rides
/// in the alpha of the probe's irradiance texels, so it costs no memory.
pub fn blend_weight_h(count: f32, hysteresis: f32) -> f32 {
    let warm = count / (count + 1.0);
    1.0 - warm.min(hysteresis)
}

// ── GPU records (the WGSL structs in 85-room-gi.wgsl and the update shader) ──

/// `GiHeader`, 64 bytes, at the start of the room table.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GiHeaderGpu {
    /// xyz = min corner of every room (render space), w = 1 when room GI is
    /// on (the dev switch writes 0 and every fragment keeps the old floor).
    pub gmin: [f32; 4],
    /// xyz = max corner of every room.
    pub gmax: [f32; 4],
    /// x = room count, y = probe count, z = irradiance tiles per atlas row,
    /// w = depth tiles per atlas row.
    pub info: [u32; 4],
    /// x = atlas width, y = atlas height, z = first texel row of the depth
    /// region.
    pub atlas: [u32; 4],
}

/// `GiRoom`, 160 bytes, one per room after the header.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GiRoomGpu {
    /// xyz = box min (render space).
    pub bmin: [f32; 4],
    /// xyz = box max, w = lid transmittance.
    pub bmax: [f32; 4],
    /// xyz = probes per axis, w = the room's first probe in the atlas.
    pub counts: [u32; 4],
    /// x = first light in the update's light list, y = how many.
    pub lights: [u32; 4],
    /// Per-face reflectance, FACE order, rgb.
    pub refl: [[f32; 4]; 6],
}

/// `GiParams`, 128 bytes, the update dispatch's uniform.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GiParamsGpu {
    /// Rows of this update's ray rotation (xyz).
    pub rot: [[f32; 4]; 3],
    /// xyz = toward the sun, w = sun intensity.
    pub sun_dir: [f32; 4],
    /// rgb = sun colour, w = 1 when the sky-view table was rendered this frame.
    pub sun_color: [f32; 4],
    /// xyz = local up for the sky table (render frame), w = the sky's
    /// LIGHTING exposure (`SKY_LIGHTING_EXPOSURE`).
    pub sky_up: [f32; 4],
    /// x = settled hysteresis, y = warm-up updates, z = normal bias, w = unused.
    pub misc: [f32; 4],
    /// x = probes in this dispatch, y = scratch irradiance tiles per row,
    /// z = scratch depth tiles per row, w = first row of the scratch depth region.
    pub counts: [u32; 4],
}

/// The sky's lighting exposure: `sky_ambient` in 90-fragment-main.wgsl reads
/// the table through `water_sky_lut` (x15, the DRAWN-sky exposure) and scales
/// by `SKY_AMBIENT_LUT_SCALE` (0.24), which that comment derives from the sun.
/// The update shader reads the raw table, so it applies the product. LOCKSTEP.
pub const SKY_LIGHTING_EXPOSURE: f32 = 15.0 * 0.24;

pub const HEADER_BYTES: u64 = std::mem::size_of::<GiHeaderGpu>() as u64;
pub const ROOM_BYTES: u64 = std::mem::size_of::<GiRoomGpu>() as u64;

/// Room flag in `GiRoomGpu::lights[2]` (WGSL `GI_ROOM_VISIBILITY`): this
/// room's fragments run DDGI's Chebyshev visibility test. Off for a room whose
/// probes trace only its own box, where the test is an identity
/// (`visibility_is_an_identity_inside_a_box`); rung 2 sets it for the rooms
/// whose contents it traces, and the dev switch `{"room_gi_vis":"1"}` sets it
/// everywhere.
pub const ROOM_FLAG_VISIBILITY: u32 = 1;

/// Pack one room for the GPU.
pub fn pack_room(
    r: &RoomBox,
    counts: [u32; 3],
    first_probe: u32,
    first_light: u32,
    light_count: u32,
    visibility: bool,
) -> GiRoomGpu {
    let mut refl = [[0.0f32; 4]; 6];
    for (dst, src) in refl.iter_mut().zip(r.refl.iter()) {
        *dst = [src[0], src[1], src[2], 0.0];
    }
    GiRoomGpu {
        bmin: [r.min.x, r.min.y, r.min.z, 0.0],
        bmax: [r.max.x, r.max.y, r.max.z, r.lid_transmittance],
        counts: [counts[0], counts[1], counts[2], first_probe],
        lights: [first_light, light_count, if visibility { ROOM_FLAG_VISIBILITY } else { 0 }, 0],
        refl,
    }
}

// ── The CPU twin ──

/// What lights the room surfaces the probes see. `Scene` is the real thing
/// (the room's own lights plus the sun through a glass lid); `Uniform` is a
/// test source that puts the same irradiance on every surface, which is the
/// exact premise of the integrating-sphere relation.
#[derive(Debug, Clone)]
pub enum DirectSource {
    Scene { lights: Vec<RoomLight>, sun: Option<Sun> },
    Uniform(Vec3),
}

/// One room in the twin: its box, its lattice, its place in the atlas, and
/// what lights it.
#[derive(Debug, Clone)]
pub struct TwinRoom {
    pub room: RoomBox,
    pub counts: [u32; 3],
    pub first_probe: u32,
    pub direct: DirectSource,
    /// The room's `ROOM_FLAG_VISIBILITY`: false for a box-only room (rung 1).
    pub visibility: bool,
}

impl TwinRoom {
    pub fn probe_count(&self) -> u32 {
        self.counts[0] * self.counts[1] * self.counts[2]
    }
}

/// The CPU twin of the whole probe system: the rooms, the atlas (f32 RGBA,
/// laid out exactly as the GPU's), and the update counter.
pub struct Twin {
    pub rooms: Vec<TwinRoom>,
    pub layout: AtlasLayout,
    pub atlas: Vec<[f32; 4]>,
    pub frame: u32,
    /// Settled hysteresis; tests that only care about the fixed point may
    /// lower it to converge in fewer updates (the fixed point does not depend
    /// on it).
    pub hysteresis: f32,
}

impl Twin {
    /// Build a twin at the standard spacing, every probe cold (zero).
    pub fn new(rooms: Vec<(RoomBox, DirectSource)>) -> Self {
        let mut first = 0u32;
        let rooms: Vec<TwinRoom> = rooms
            .into_iter()
            .map(|(room, direct)| {
                let counts = probe_counts(room.size(), PROBE_SPACING_M);
                let r = TwinRoom { room, counts, first_probe: first, direct, visibility: false };
                first += r.probe_count();
                r
            })
            .collect();
        let layout = AtlasLayout::for_probes(first);
        let atlas = vec![[0.0; 4]; layout.texel_count()];
        Twin { rooms, layout, atlas, frame: 0, hysteresis: HYSTERESIS }
    }

    fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let x = x.clamp(0, self.layout.width as i32 - 1) as usize;
        let y = y.clamp(0, self.layout.height as i32 - 1) as usize;
        self.atlas[y * self.layout.width as usize + x]
    }

    /// Bilinear fetch at continuous texel coordinates, the way a Linear /
    /// ClampToEdge sampler reads the atlas (texel centres at +0.5).
    pub fn bilinear(&self, c: Vec2) -> [f32; 4] {
        let f = c - 0.5;
        let (x0, y0) = (f.x.floor(), f.y.floor());
        let (tx, ty) = (f.x - x0, f.y - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        let a = self.texel(x0, y0);
        let b = self.texel(x0 + 1, y0);
        let c0 = self.texel(x0, y0 + 1);
        let d = self.texel(x0 + 1, y0 + 1);
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bot = c0[k] + (d[k] - c0[k]) * tx;
            out[k] = top + (bot - top) * ty;
        }
        out
    }

    /// The cosine-weighted mean radiance room `ri`'s probes give a surface at
    /// `p` facing `n`: DDGI's 8-probe sample with the backface weight, the
    /// Chebyshev (moment) visibility test, the weight crush, trilinear weights
    /// and the square-root blend. Mirrors `gi_sample_room` in 85-room-gi.wgsl,
    /// running the visibility test when the room is flagged for it, as the
    /// GPU does.
    pub fn sample(&self, ri: usize, p: Vec3, n: Vec3) -> Vec3 {
        self.sample_with(ri, p, n, self.rooms[ri].visibility)
    }

    /// `sample` with the Chebyshev visibility test on or off (the GPU runs it
    /// only in rooms flagged `ROOM_FLAG_VISIBILITY`).
    pub fn sample_with(&self, ri: usize, p: Vec3, n: Vec3, visibility: bool) -> Vec3 {
        let r = &self.rooms[ri];
        sample_grid(&r.room, r.counts, r.first_probe, &self.layout, |c| self.bilinear(c), p, n, visibility)
    }

    /// One update of EVERY probe from the current atlas (the GPU reads the
    /// previous atlas and writes a scratch copy, so every probe in a dispatch
    /// sees the same "previous frame"; this does the same by building the new
    /// atlas beside the old one).
    pub fn update_all(&mut self) {
        let rot = ray_rotation(self.frame);
        self.frame = self.frame.wrapping_add(1);
        let mut next = self.atlas.clone();
        for ri in 0..self.rooms.len() {
            for local in 0..self.rooms[ri].probe_count() {
                self.update_probe(ri, local, rot, &mut next);
            }
        }
        self.atlas = next;
    }

    /// Radiance arriving back along ray `d` from probe position `p` in room
    /// `ri`, and the hit distance: `cs_trace`'s per-thread half.
    pub fn ray_radiance(&self, ri: usize, p: Vec3, d: Vec3) -> (Vec3, f32) {
        let r = &self.rooms[ri];
        let (t, face) = trace_box(p, d, r.room.min, r.room.max);
        let h = p + d * t;
        let n = face_normal(face);
        let refl = Vec3::from(r.room.refl[face]);
        let e = match &r.direct {
            DirectSource::Uniform(e0) => *e0,
            DirectSource::Scene { lights, sun } => {
                let mut e = lights.iter().fold(Vec3::ZERO, |acc, l| acc + light_irradiance(l, h, n));
                if let Some(s) = sun {
                    let tr = sun_path(&r.room, h, n, s.dir);
                    e += Vec3::from(s.color) * s.intensity * n.dot(s.dir).max(0.0) * tr;
                }
                e
            }
        };
        let l = refl / std::f32::consts::PI * e + refl * self.sample(ri, h, n);
        (l, t)
    }

    fn update_probe(&self, ri: usize, local: u32, rot: Mat3, next: &mut [[f32; 4]]) {
        let r = &self.rooms[ri];
        let probe = r.first_probe + local;
        let p = probe_position(&r.room, r.counts, probe_grid(r.counts, local));
        let rays: Vec<(Vec3, Vec3, f32)> = (0..RAYS_PER_PROBE)
            .map(|i| {
                let d = rot * fibonacci_ray(i, RAYS_PER_PROBE);
                let (l, t) = self.ray_radiance(ri, p, d);
                (d, l, t)
            })
            .collect();
        let io = self.layout.irr_origin(probe);
        let count = self.texel(io.0 as i32 + 1, io.1 as i32 + 1)[3];
        let a = blend_weight_h(count, self.hysteresis);
        // Irradiance: cosine-weighted mean radiance per texel direction.
        let mut irr = vec![[0.0f32; 4]; (IRR_N * IRR_N) as usize];
        for y in 0..IRR_N {
            for x in 0..IRR_N {
                let td = texel_direction(IRR_N, x, y);
                let (mut acc, mut ws) = (Vec3::ZERO, 0.0);
                for (d, l, _) in &rays {
                    let w = td.dot(*d).max(0.0);
                    acc += *l * w;
                    ws += w;
                }
                let prev = self.texel((io.0 + 1 + x) as i32, (io.1 + 1 + y) as i32);
                let prev3 = Vec3::new(prev[0], prev[1], prev[2]);
                let new = if ws > 1e-6 { acc / ws } else { prev3 };
                let v = prev3 + (new - prev3) * a;
                irr[(y * IRR_N + x) as usize] = [v.x, v.y, v.z, (count + 1.0).min(MAX_UPDATE_COUNT)];
            }
        }
        write_tile(next, &self.layout, io, IRR_N, &irr);
        // Depth moments: mean hit distance and mean squared distance.
        let dg = self.layout.depth_origin(probe);
        let mut dep = vec![[0.0f32; 4]; (DEPTH_N * DEPTH_N) as usize];
        for y in 0..DEPTH_N {
            for x in 0..DEPTH_N {
                let td = texel_direction(DEPTH_N, x, y);
                let (mut acc, mut ws) = (Vec2::ZERO, 0.0);
                for (d, _, t) in &rays {
                    let c = td.dot(*d);
                    if c < DEPTH_MIN_COS {
                        continue;
                    }
                    let w = c.powf(DEPTH_SHARPNESS);
                    acc += Vec2::new(*t, t * t) * w;
                    ws += w;
                }
                let prev = self.texel((dg.0 + 1 + x) as i32, (dg.1 + 1 + y) as i32);
                let prev2 = Vec2::new(prev[0], prev[1]);
                // No ray in this texel's lobe this update: keep what it had.
                let new = if ws > 1e-6 { acc / ws } else { prev2 };
                let v = prev2 + (new - prev2) * a;
                dep[(y * DEPTH_N + x) as usize] = [v.x, v.y, 0.0, 0.0];
            }
        }
        write_tile(next, &self.layout, dg, DEPTH_N, &dep);
    }

    /// Mean over every probe of the room and every irradiance texel of the
    /// stored value: the room's average indirect light, as E / pi.
    pub fn room_mean(&self, ri: usize) -> Vec3 {
        let r = &self.rooms[ri];
        let mut acc = Vec3::ZERO;
        let mut n = 0.0;
        for local in 0..r.probe_count() {
            let io = self.layout.irr_origin(r.first_probe + local);
            for y in 0..IRR_N {
                for x in 0..IRR_N {
                    let t = self.texel((io.0 + 1 + x) as i32, (io.1 + 1 + y) as i32);
                    acc += Vec3::new(t[0], t[1], t[2]);
                    n += 1.0;
                }
            }
        }
        acc / n
    }

    /// Largest stored value anywhere in room `ri` (any channel).
    pub fn room_max(&self, ri: usize) -> f32 {
        let r = &self.rooms[ri];
        let mut m = 0.0f32;
        for local in 0..r.probe_count() {
            let io = self.layout.irr_origin(r.first_probe + local);
            for y in 0..IRR_TILE {
                for x in 0..IRR_TILE {
                    let t = self.texel((io.0 + x) as i32, (io.1 + y) as i32);
                    m = m.max(t[0]).max(t[1]).max(t[2]);
                }
            }
        }
        m
    }
}

/// Write an N x N interior into its tile and fill the 1-texel border from it.
fn write_tile(atlas: &mut [[f32; 4]], layout: &AtlasLayout, origin: (u32, u32), n: u32, interior: &[[f32; 4]]) {
    for by in 0..n + 2 {
        for bx in 0..n + 2 {
            let (sx, sy) = border_source(n, bx, by);
            let v = interior[(sy * n + sx) as usize];
            let idx = (origin.1 + by) as usize * layout.width as usize + (origin.0 + bx) as usize;
            atlas[idx] = v;
        }
    }
}

/// The 8-probe DDGI sample, generic over how the atlas is fetched (the twin's
/// bilinear). See `Twin::sample`.
pub fn sample_grid(
    room: &RoomBox,
    counts: [u32; 3],
    first_probe: u32,
    layout: &AtlasLayout,
    fetch: impl Fn(Vec2) -> [f32; 4],
    p: Vec3,
    n: Vec3,
    visibility: bool,
) -> Vec3 {
    let cf = Vec3::new(counts[0] as f32, counts[1] as f32, counts[2] as f32);
    let cell = room.size() / cf;
    let ps = p + n * NORMAL_BIAS_M;
    let g = (ps - room.min) / cell - 0.5;
    let base = g.floor().clamp(Vec3::ZERO, cf - 2.0);
    let alpha = (g - base).clamp(Vec3::ZERO, Vec3::ONE);
    let (mut sum, mut wsum) = (Vec3::ZERO, 0.0f32);
    for i in 0..8u32 {
        let off = Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32);
        let pc = base + off;
        let grid = [pc.x as u32, pc.y as u32, pc.z as u32];
        let probe = first_probe + grid[0] + counts[0] * (grid[1] + counts[1] * grid[2]);
        let ppos = room.min + (pc + 0.5) * cell;
        // WGSL `mix(1 - alpha, alpha, off)`, per component.
        let tri3 = (Vec3::ONE - alpha) * (Vec3::ONE - off) + alpha * off;
        let tri = tri3.x * tri3.y * tri3.z;
        // No trilinear weight, no contribution: skipped (the WGSL skips its
        // two fetches the same way).
        if tri <= 0.0 {
            continue;
        }
        // Smooth backface weight: a probe behind the surface counts for little.
        let to_probe = (ppos - p).normalize_or_zero();
        let bf = (to_probe.dot(n) + 1.0) * 0.5;
        let mut w = bf * bf + 0.2;
        // Chebyshev visibility from the stored depth moments.
        if visibility {
            let p2pt = ps - ppos;
            let dist = p2pt.length();
            let dir = if dist > 1e-4 { p2pt / dist } else { n };
            let m = fetch(tile_coord(layout.depth_origin(probe), DEPTH_N, dir));
            if dist > m[0] {
                let variance = (m[1] - m[0] * m[0]).abs();
                let excess = dist - m[0];
                let cheb = variance / (variance + excess * excess).max(1e-9);
                w *= (cheb * cheb * cheb).max(0.0);
            }
        }
        w = w.max(1e-6);
        if w < CRUSH_THRESHOLD {
            w *= w * w / (CRUSH_THRESHOLD * CRUSH_THRESHOLD);
        }
        w *= tri;
        let irr = fetch(tile_coord(layout.irr_origin(probe), IRR_N, n));
        let irr = Vec3::new(irr[0], irr[1], irr[2]).max(Vec3::ZERO);
        sum += Vec3::new(irr.x.sqrt(), irr.y.sqrt(), irr.z.sqrt()) * w;
        wsum += w;
    }
    let r = sum / wsum.max(1e-9);
    r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey_box(min: Vec3, max: Vec3, rho: f32) -> RoomBox {
        RoomBox { min, max, refl: [[rho; 3]; 6], lid_transmittance: 0.0 }
    }

    /// The octahedral map round-trips: every texel centre decodes to a
    /// direction that encodes back to the same point, and every direction
    /// (the axes included, where a sign(0) = 0 convention breaks the fold)
    /// encodes to a point that decodes back to it. Seen fail first with
    /// `sign_not_zero` replaced by a sign that returns 0 for 0 (WGSL's
    /// plain `sign`): the texel-centre round trip broke on the fold.
    #[test]
    fn octahedral_encode_decode_round_trips() {
        for n in [IRR_N, DEPTH_N] {
            for y in 0..n {
                for x in 0..n {
                    let uv = (Vec2::new(x as f32, y as f32) + 0.5) / n as f32 * 2.0 - 1.0;
                    let back = oct_encode(texel_direction(n, x, y));
                    assert!((back - uv).length() < 1e-5, "texel ({x},{y}) of {n}: {uv} -> {back}");
                }
            }
        }
        let mut dirs = vec![Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
        dirs.push(Vec3::new(1.0, 0.0, -1.0).normalize());
        dirs.push(Vec3::new(0.0, -1.0, -1.0).normalize());
        dirs.extend((0..500).map(|i| fibonacci_ray(i, 500)));
        for d in dirs {
            let back = oct_decode(oct_encode(d));
            assert!((back - d).length() < 1e-5, "{d} -> {back}");
        }
    }

    /// The tile border holds the octahedral wrap: bilinear taps straddling a
    /// seam blend two texels that stand for directions close together. Checked
    /// by decoding each border texel's SOURCE and comparing it with the
    /// direction its own (outside-the-square) position extrapolates to.
    #[test]
    fn tile_border_copies_the_texel_across_the_seam() {
        let n = IRR_N;
        for by in 0..n + 2 {
            for bx in 0..n + 2 {
                let interior = (1..=n).contains(&bx) && (1..=n).contains(&by);
                let (sx, sy) = border_source(n, bx, by);
                if interior {
                    assert_eq!((sx, sy), (bx - 1, by - 1));
                    continue;
                }
                // The border texel's centre, in octahedral coordinates, lies
                // just outside [-1, 1]; fold it back the way the octahedron
                // folds and it must land on its source texel's centre.
                let uv = (Vec2::new(bx as f32 - 1.0, by as f32 - 1.0) + 0.5) / n as f32 * 2.0 - 1.0;
                let mut f = uv;
                if f.x.abs() > 1.0 {
                    f = Vec2::new(f.x.signum() * (2.0 - f.x.abs()), -f.y);
                }
                if f.y.abs() > 1.0 {
                    f = Vec2::new(-f.x, f.y.signum() * (2.0 - f.y.abs()));
                }
                let src = (Vec2::new(sx as f32, sy as f32) + 0.5) / n as f32 * 2.0 - 1.0;
                assert!((f - src).length() < 1e-5, "border ({bx},{by}) folds to {f}, source centre {src}");
            }
        }
    }

    /// THE ENERGY BOOKKEEPING. A closed box with every face at reflectance rho
    /// and the same direct irradiance E0 on every surface has, at equilibrium,
    /// uniform indirect irradiance E_ind = rho * E0 / (1 - rho) everywhere:
    /// the integrating-sphere relation, which for a UNIFORMLY lit closed
    /// Lambertian enclosure holds for any shape. The probes store E / pi, so
    /// the mean stored value must be rho * E0 / (pi * (1 - rho)).
    ///
    /// Measured 2026-09-27: 0.7424 against 0.7427 at rho = 0.7. Seen fail
    /// first two ways (scripted mutations, each restored): with the bounce
    /// term dropped from `ray_radiance` (one bounce only) it stored 0.0955
    /// against 0.1364 at rho = 0.3, and with the hit shaded `refl * E`
    /// instead of `refl / pi * E` it stored 0.4286, pi times high.
    #[test]
    fn closed_box_converges_to_the_integrating_sphere_value() {
        for rho in [0.3f32, 0.5, 0.7] {
            let e0 = 1.0f32;
            let mut twin = Twin::new(vec![(
                grey_box(Vec3::ZERO, Vec3::new(3.0, 2.0, 3.0), rho),
                DirectSource::Uniform(Vec3::splat(e0)),
            )]);
            twin.hysteresis = 0.8;
            for _ in 0..120 {
                twin.update_all();
            }
            let want = rho * e0 / (std::f32::consts::PI * (1.0 - rho));
            let got = twin.room_mean(0);
            for c in got.to_array() {
                assert!((c / want - 1.0).abs() < 0.10, "rho {rho}: stored {c}, integrating sphere {want}");
            }
            // And a fragment on a wall reads the same through the full sample.
            let s = twin.sample(0, Vec3::new(0.0, 1.0, 1.5), Vec3::X);
            assert!((s.x / want - 1.0).abs() < 0.10, "rho {rho}: wall sample {s}, want {want}");
        }
    }

    /// The same relation with a REAL lamp: one point light in the middle of a
    /// 4 m cube at rho = 0.5. Its direct light is not uniform, so the relation
    /// holds on average: the room-mean indirect irradiance must match
    /// rho * (mean direct irradiance over the walls) / (1 - rho). Measured
    /// 2026-09-27: 0.2426 against 0.2259, +7.4% (the centre of each face is
    /// both brightest and nearest the probes, which is the part the
    /// integrating-sphere average cannot see). Seen fail first with the
    /// texel estimate divided by the ray count instead of by its cosine
    /// weights (the DDGI normalization dropped): 0.0343 against 0.2259.
    #[test]
    fn closed_box_with_a_lamp_matches_the_mean_direct_light() {
        let rho = 0.5f32;
        let size = 4.0f32;
        let room = grey_box(Vec3::ZERO, Vec3::splat(size), rho);
        let lamp = RoomLight::point(Vec3::splat(size * 0.5), [1.0, 1.0, 1.0], 8.0, 12.0);
        // Area-mean direct irradiance over the six faces, by midpoint rule.
        let steps = 60;
        let mut e_sum = 0.0f32;
        for face in 0..6 {
            let n = face_normal(face);
            for i in 0..steps {
                for j in 0..steps {
                    let (u, v) = ((i as f32 + 0.5) / steps as f32 * size, (j as f32 + 0.5) / steps as f32 * size);
                    let p = match face {
                        FACE_NEG_X => Vec3::new(0.0, u, v),
                        FACE_POS_X => Vec3::new(size, u, v),
                        FACE_FLOOR => Vec3::new(u, 0.0, v),
                        FACE_LID => Vec3::new(u, size, v),
                        FACE_NEG_Z => Vec3::new(u, v, 0.0),
                        _ => Vec3::new(u, v, size),
                    };
                    e_sum += light_irradiance(&lamp, p, n).x;
                }
            }
        }
        let e_avg = e_sum / (6 * steps * steps) as f32;
        let mut twin = Twin::new(vec![(room, DirectSource::Scene { lights: vec![lamp], sun: None })]);
        twin.hysteresis = 0.9;
        for _ in 0..90 {
            twin.update_all();
        }
        let want = rho * e_avg / (std::f32::consts::PI * (1.0 - rho));
        let got = twin.room_mean(0).x;
        assert!((got / want - 1.0).abs() < 0.10, "lamp box: room mean {got}, rho*E_avg/(pi(1-rho)) {want}");
    }

    /// NO SELF-SUSTAINING ENERGY. Converge a lit room, switch every light off,
    /// and the stored light must decay geometrically to (well under) the
    /// ambient floor, never hold or grow. The per-update gain with no source
    /// is h + (1 - h) * rho < 1; the test runs at h = 0.8 so it gets there in
    /// 150 updates (the zero fixed point does not depend on h). Seen fail
    /// first with the bounce term missing its reflectance
    /// (`+ self.sample(..)` instead of `+ refl * self.sample(..)`, a gain of
    /// 1 per bounce): after 150 dark updates the room held 2.33 and rising.
    #[test]
    fn with_the_lights_off_the_glow_decays_to_the_floor() {
        let room = grey_box(Vec3::ZERO, Vec3::new(3.0, 2.0, 3.0), 0.7);
        let lamp = RoomLight::point(Vec3::new(1.5, 1.8, 1.5), [1.0, 0.95, 0.9], 8.0, 10.0);
        let mut twin = Twin::new(vec![(room, DirectSource::Scene { lights: vec![lamp], sun: None })]);
        twin.hysteresis = 0.8;
        for _ in 0..40 {
            twin.update_all();
        }
        let lit = twin.room_max(0);
        assert!(lit > 0.02, "the lamp should light the room first ({lit})");
        twin.rooms[0].direct = DirectSource::Scene { lights: vec![], sun: None };
        let mut prev = lit;
        for k in 0..150 {
            twin.update_all();
            let now = twin.room_max(0);
            assert!(now <= prev * 1.0001 + 1e-9, "update {k}: glow rose {prev} -> {now} with no light");
            prev = now;
        }
        // AMBIENT_FLOOR in 90-fragment-main.wgsl is 0.005; the probes must go
        // well under it so the floor, not a ghost of the lamp, is what shows.
        assert!(prev < 0.005 * 0.1, "after 150 dark updates the room still holds {prev}");
    }

    /// ROOM ISOLATION. Two rooms share a wall; the lamp is in room A. Every
    /// probe of room B must read exactly zero, and a fragment on the shared
    /// wall must pick the room it FACES. Seen fail first by handing room B
    /// the whole light list instead of `assign_lights`' per-room list: room
    /// B's probes lit up through the wall and the equality failed.
    #[test]
    fn a_probe_never_reads_the_neighbouring_rooms_light() {
        let a = grey_box(Vec3::ZERO, Vec3::new(3.0, 2.5, 3.0), 0.6);
        let b = grey_box(Vec3::new(3.1, 0.0, 0.0), Vec3::new(6.1, 2.5, 3.0), 0.6);
        let lamp = RoomLight::point(Vec3::new(1.5, 2.4, 1.5), [1.0, 1.0, 1.0], 10.0, 14.0);
        let lights = vec![lamp];
        let rooms = vec![a.clone(), b.clone()];
        let owned = assign_lights(&rooms, &lights);
        assert_eq!(owned, vec![vec![0], vec![]], "the lamp belongs to room A only");
        let per_room = |i: usize| owned[i].iter().map(|&k| lights[k]).collect::<Vec<_>>();
        let mut twin = Twin::new(vec![
            (a, DirectSource::Scene { lights: per_room(0), sun: None }),
            (b, DirectSource::Scene { lights: per_room(1), sun: None }),
        ]);
        for _ in 0..40 {
            twin.update_all();
        }
        assert!(twin.room_max(0) > 0.01, "room A is lit");
        assert_eq!(twin.room_max(1), 0.0, "room B must hold no light at all");
        // The shared wall: x = 3.0 is A's face (normal -x points into A),
        // x = 3.1 is B's face (normal +x points into B).
        assert_eq!(pick_room(&rooms, Vec3::new(3.0, 1.0, 1.5), Vec3::NEG_X), Some(0));
        assert_eq!(pick_room(&rooms, Vec3::new(3.1, 1.0, 1.5), Vec3::X), Some(1));
        // The OUTSIDE of the hull picks nothing.
        assert_eq!(pick_room(&rooms, Vec3::new(-0.06, 1.0, 1.5), Vec3::NEG_X), None);
        // And the sample B's wall fragment takes is B's, which is dark.
        assert_eq!(twin.sample(1, Vec3::new(3.1, 1.0, 1.5), Vec3::X), Vec3::ZERO);
    }

    /// WHY RUNG 1 SKIPS THE VISIBILITY TEST. Inside a room whose probes trace
    /// only its box, DDGI's Chebyshev test changes nothing: the box is convex,
    /// so the stored mean depth toward any point inside it is never short of
    /// that point's distance. Checked on every face (the surfaces fragments
    /// actually sit on, with their inward normals) and through the interior,
    /// against the same converged probes sampled with and without the test.
    /// Measured 2026-09-27: no sample moved by as much as 0.05%. Seen fail
    /// first by storing the depth moments at half the hit distance (`t * 0.5`
    /// in `update_probe`): the test then moved a sample by 100%. (At 20% short
    /// it still passed: a surface point sits nearer its probes than the wall
    /// behind it, by the bias and half a cell.)
    #[test]
    fn visibility_is_an_identity_inside_a_box() {
        let room = grey_box(Vec3::ZERO, Vec3::new(4.0, 3.0, 5.0), 0.5);
        let lamp = RoomLight::point(Vec3::new(1.0, 2.8, 1.2), [1.0, 0.9, 0.8], 10.0, 12.0);
        let mut twin = Twin::new(vec![(room, DirectSource::Scene { lights: vec![lamp], sun: None })]);
        twin.hysteresis = 0.9;
        for _ in 0..40 {
            twin.update_all();
        }
        let mut worst = 0.0f32;
        let mut check = |p: Vec3, n: Vec3| {
            let with = twin.sample_with(0, p, n, true);
            let without = twin.sample_with(0, p, n, false);
            let rel = (with - without).abs().max_element() / without.max_element().max(1e-6);
            worst = worst.max(rel);
        };
        let size = Vec3::new(4.0, 3.0, 5.0);
        for i in 0..9 {
            for j in 0..9 {
                let (u, v) = ((i as f32 + 0.5) / 9.0, (j as f32 + 0.5) / 9.0);
                check(Vec3::new(u * size.x, 0.0, v * size.z), Vec3::Y);
                check(Vec3::new(u * size.x, size.y, v * size.z), Vec3::NEG_Y);
                check(Vec3::new(0.0, u * size.y, v * size.z), Vec3::X);
                check(Vec3::new(size.x, u * size.y, v * size.z), Vec3::NEG_X);
                check(Vec3::new(u * size.x, v * size.y, 0.0), Vec3::Z);
                check(Vec3::new(u * size.x, v * size.y, size.z), Vec3::NEG_Z);
                check(Vec3::new(u * size.x, 1.2, v * size.z), fibonacci_ray(i * 9 + j, 81));
            }
        }
        assert!(worst < 0.02, "the visibility test moved a sample inside the box by {:.1}%", worst * 100.0);
    }

    /// The pick order: nearest the eye first, but a room inside another
    /// room's box always ahead of it. The commons hall's box contains nine
    /// small rooms; standing in the hall, a fragment in one of them must
    /// still pick that room. Seen fail first with the containment step
    /// switched off (nearest first alone, which is what the first cut's
    /// "camera's room first" amounted to): the hall came first, [1, 2, 0].
    #[test]
    fn a_room_inside_another_rooms_box_is_picked_first() {
        let hall = grey_box(Vec3::ZERO, Vec3::new(30.0, 8.0, 30.0), 0.5);
        let cell = grey_box(Vec3::new(10.0, 0.0, 10.0), Vec3::new(14.0, 8.0, 14.0), 0.5);
        let far = grey_box(Vec3::new(40.0, 0.0, 0.0), Vec3::new(50.0, 3.0, 10.0), 0.5);
        let rooms = vec![far.clone(), hall.clone(), cell.clone()];
        let eye = Vec3::new(2.0, 1.7, 2.0); // in the hall, outside the cell
        let order = pick_order(&rooms, eye);
        let pos = |i: usize| order.iter().position(|&o| o == i).unwrap();
        assert!(pos(2) < pos(1), "the cell comes before the hall that contains it: {order:?}");
        assert!(pos(1) < pos(0), "the hall (holding the eye) before the far room: {order:?}");
        let table: Vec<RoomBox> = order.iter().map(|&i| rooms[i].clone()).collect();
        let picked = pick_room(&table, Vec3::new(12.0, 0.0, 12.0), Vec3::Y).map(|t| order[t]);
        assert_eq!(picked, Some(2), "a floor point in the cell picks the cell");
        let picked = pick_room(&table, Vec3::new(5.0, 0.0, 5.0), Vec3::Y).map(|t| order[t]);
        assert_eq!(picked, Some(1), "a floor point in the hall picks the hall");
    }

    /// The sun reaches a surface only through a glass lid, and only when the
    /// box does not put a wall in the way.
    #[test]
    fn the_sun_comes_in_through_the_glass_lid_only() {
        let mut room = grey_box(Vec3::ZERO, Vec3::new(4.0, 3.0, 4.0), 0.5);
        room.lid_transmittance = 0.65;
        let high = Vec3::new(0.2, 1.0, 0.1).normalize();
        assert_eq!(sun_path(&room, Vec3::new(2.0, 0.0, 2.0), Vec3::Y, high), 0.65);
        // A low sun from +x: the floor near the +x wall is in that wall's lee.
        let low = Vec3::new(1.0, 0.2, 0.0).normalize();
        assert_eq!(sun_path(&room, Vec3::new(3.5, 0.0, 2.0), Vec3::Y, low), 0.0);
        // The wall it strikes from the inside faces AWAY from it.
        assert_eq!(sun_path(&room, Vec3::new(0.0, 2.9, 2.0), Vec3::X, low), 0.65);
        room.lid_transmittance = 0.0;
        assert_eq!(sun_path(&room, Vec3::new(2.0, 0.0, 2.0), Vec3::Y, high), 0.0);
    }

    #[test]
    fn lattice_and_layout_are_what_the_design_says() {
        assert_eq!(probe_counts(Vec3::new(10.0, 3.0, 10.0), 1.0), [11, 4, 11]);
        assert_eq!(probe_counts(Vec3::new(0.4, 0.4, 0.4), 1.0), [2, 2, 2]);
        let l = AtlasLayout::for_probes(484);
        assert_eq!(l.width % IRR_TILE, 0);
        assert_eq!(l.width % DEPTH_TILE, 0);
        assert!(l.height <= 8192 && l.width <= 8192);
        // Tiles never overlap: the last irradiance tile ends above the depth region.
        let last = l.irr_origin(483);
        assert!(last.1 + IRR_TILE <= l.depth_y0);
        let big = AtlasLayout::for_probes(MAX_TOTAL_PROBES);
        assert!(big.height <= 8192, "the probe cap must fit the texture limit ({})", big.height);
        // A room too big for the per-room cap coarsens ALONE: the bedroom
        // beside it keeps its 1 m lattice.
        let huge = vec![
            grey_box(Vec3::ZERO, Vec3::new(200.0, 3.0, 200.0), 0.5),
            grey_box(Vec3::ZERO, Vec3::new(10.0, 3.0, 10.0), 0.5),
        ];
        let (_, counts) = lattice_for(&huge);
        let n = |c: [u32; 3]| c[0] * c[1] * c[2];
        assert!(n(counts[0]) <= MAX_PROBES_PER_ROOM, "the hall holds {}", n(counts[0]));
        assert_eq!(counts[1], [11, 4, 11], "the small room keeps 1 m");
        // A ship too big for the total cap coarsens everywhere instead of overflowing.
        let many: Vec<RoomBox> = (0..8).map(|i| grey_box(Vec3::new(i as f32 * 50.0, 0.0, 0.0), Vec3::new(i as f32 * 50.0 + 40.0, 3.0, 40.0), 0.5)).collect();
        let (spacing, counts) = lattice_for(&many);
        let total: u32 = counts.iter().map(|c| n(*c)).sum();
        assert!(spacing > 1.0 && total <= MAX_TOTAL_PROBES, "spacing {spacing}, {total} probes");
    }

    /// The GPU records are the sizes the WGSL structs have, which naga checks
    /// against the shader text in `room_probes_gpu`'s tests.
    #[test]
    fn gpu_record_sizes() {
        assert_eq!(HEADER_BYTES, 64);
        assert_eq!(ROOM_BYTES, 160);
        assert_eq!(std::mem::size_of::<GiParamsGpu>(), 128);
    }
}
