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
//!
//! TWO MORE THINGS LIVE HERE SINCE 2026-10-21 (docs/design/room-gi.md,
//! "The room's ambient colour" and "Probe streaming"):
//! - THE ROOM'S AMBIENT COLOUR (`room_ambient`): one colour per room, worked
//!   out every frame from that room's own lamps and windows with the
//!   integrating-sphere relation instead of a probe lattice. It is what the
//!   Bounced light detail level "Ambient" shows everywhere, and what any room
//!   shows while its probes are not resident.
//! - PROBE STREAMING (`Residency`): only the rooms nearest the player hold
//!   probes, within the level's fixed probe budget. A room coming in has its
//!   probes seeded with its ambient colour (`Twin::seed`, `cs_seed`), so it
//!   starts exactly where it was and converges; a room going out fades to its
//!   ambient colour before its probes are given away.

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
/// The probes resident at once at the default level (High): the atlas holds
/// exactly this many (106 MB) and the rooms nearest the player share them
/// (`Residency`). Since probe streaming (2026-10-21) it is a budget for the
/// rooms around the player, not for the whole ship: the shipped ship's 41
/// rooms would hold 109,497 probes at 1 m (2026-10-20), the 1.1 km street
/// alone 60,555, so before streaming the rest of the ship had to coarsen to
/// 2.5 m to keep the home at 1 m.
pub const MAX_TOTAL_PROBES: u32 = 32_768;
/// Upper bound on one room's probes; a bigger room coarsens alone. A quarter
/// of the budget (`ProbeBudget::per_room`); this is the default budget's.
pub const MAX_PROBES_PER_ROOM: u32 = MAX_TOTAL_PROBES / 4;

/// The two numbers a Bounced light detail level sets (Settings > Graphics,
/// `bounced_light_detail::DetailLevel`, data/gui/bounced_light_detail.json):
/// the spacing every room's probes aim for (`lattice_for`) and how many
/// probes may be resident at once (`Residency`, the atlas's size). The default
/// is the design's, `PROBE_SPACING_M` and `MAX_TOTAL_PROBES`. A level with
/// `max_probes` 0 holds no probes at all: every room shows its ambient colour
/// (`room_ambient`), the "Ambient" level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeBudget {
    pub spacing_m: f32,
    pub max_probes: u32,
}

impl Default for ProbeBudget {
    fn default() -> Self {
        ProbeBudget { spacing_m: PROBE_SPACING_M, max_probes: MAX_TOTAL_PROBES }
    }
}

impl ProbeBudget {
    /// One room's cap: a quarter of the budget, so one hall coarsens alone
    /// and never fills the atlas by itself (`MAX_PROBES_PER_ROOM` at the
    /// default). Never under 8, a 2 x 2 x 2 lattice.
    pub fn per_room(&self) -> u32 {
        (self.max_probes / 4).max(8)
    }

    /// True for the Ambient level: no probes, one colour per room.
    pub fn ambient_only(&self) -> bool {
        self.max_probes == 0
    }
}
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

/// Probe counts for every room: each room at the level's spacing
/// (`budget.spacing_m`), and only a room that ALONE would hold more than the
/// per-room cap (`ProbeBudget::per_room`, a quarter of the budget: the 8 m
/// commons hall, 17,640 at 1 m; the 1.1 km street, 60,555) coarsens, by
/// itself, to the finest spacing at which it fits.
///
/// WHY NO SHIP-WIDE FIT (probe streaming, 2026-10-21). Until then the whole
/// ship had to fit the atlas at once, so the rooms added elsewhere on
/// 2026-10-05 first took the home off its 1 m lattice to 1.33 m, and then
/// (the home-first priority of 2026-10-20) took every room but the home's to
/// 2.5 m. Now only the rooms nearest the player hold probes (`Residency`), so
/// how big the ship is no longer coarsens any room: the Commons keeps the
/// level's spacing like the home does.
pub fn lattice_for(rooms: &[RoomBox], budget: ProbeBudget) -> Lattice {
    let n = |c: [u32; 3]| c[0] as u64 * c[1] as u64 * c[2] as u64;
    let spacing: Vec<f32> = rooms
        .iter()
        .map(|r| finest_fitting(budget.spacing_m, budget.per_room() as u64, |s| n(probe_counts(r.size(), s))))
        .collect();
    let counts = rooms.iter().zip(&spacing).map(|(r, s)| probe_counts(r.size(), *s)).collect();
    Lattice { counts, spacing }
}

/// The coarsest spacing `lattice_for` takes any room to (metres). A room that
/// does not fit its cap even here keeps it.
pub const MAX_PROBE_SPACING_M: f32 = 64.0;

/// The finest spacing in `[finest, MAX_PROBE_SPACING_M]` (`finest` is the
/// budget's spacing) at which
/// `probes(spacing)` is at most `budget`, or `MAX_PROBE_SPACING_M` when even
/// that does not fit. `probes` must never grow as the spacing grows, which
/// holds for any sum of `probe_counts` products (each axis is a ceiling of
/// size / spacing), so a bisection finds the threshold: 32 halvings of the
/// 63 m range leave it within 15 nm, past f32's own resolution near 1 m.
/// The 1.1x steps this replaced (2026-10-20) cut a box's probes by up to
/// 1.1^3 = 1.33x a step, so they could leave a quarter of the budget unused.
fn finest_fitting(finest: f32, budget: u64, probes: impl Fn(f32) -> u64) -> f32 {
    let (mut lo, mut hi) = (finest.min(MAX_PROBE_SPACING_M), MAX_PROBE_SPACING_M);
    if probes(lo) <= budget {
        return lo;
    }
    if probes(hi) > budget {
        return hi;
    }
    // Invariant: probes(lo) > budget >= probes(hi).
    for _ in 0..32 {
        let mid = 0.5 * (lo + hi);
        if probes(mid) <= budget {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// What `lattice_for` chose for a set of rooms.
#[derive(Debug, Clone, PartialEq)]
pub struct Lattice {
    /// Probes per axis, one entry per room, in the order the rooms were given.
    pub counts: Vec<[u32; 3]>,
    /// Each room's spacing (metres). The probes sit at the centres of
    /// `counts` equal cells, so this is the spacing asked for; the cells are
    /// that or a little finer.
    pub spacing: Vec<f32>,
}

impl Lattice {
    /// Probes in room `i`.
    pub fn probes_in(&self, i: usize) -> u32 {
        let c = self.counts[i];
        c[0] * c[1] * c[2]
    }

    /// Probes in every room (resident or not).
    pub fn total(&self) -> u64 {
        (0..self.counts.len()).map(|i| self.probes_in(i) as u64).sum()
    }
}

/// Rooms the AMBIENT ROW can hold: one texel per room, one atlas row wide
/// (`AtlasLayout::amb_y0`). A ship with more rooms keeps the first this many
/// (`room_probes_gpu` warns).
pub const MAX_AMBIENT_ROOMS: usize = ATLAS_WIDTH as usize;

/// Where each probe's two tiles live. The irradiance tiles fill the top of the
/// atlas row by row; the depth tiles start below them at `depth_y0`; the last
/// row (`amb_y0`) is the AMBIENT ROW, one texel per room holding its ambient
/// colour (`room_ambient`), texel x = the room's index. A layout for 0 probes
/// is that one row alone: the Ambient level's whole atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasLayout {
    pub width: u32,
    pub height: u32,
    pub irr_per_row: u32,
    pub depth_per_row: u32,
    pub depth_y0: u32,
    /// The ambient row (always `height - 1`; the shaders read it so).
    pub amb_y0: u32,
    pub probes: u32,
}

impl AtlasLayout {
    /// A layout holding `probes` probes (0 for the ambient row alone).
    pub fn for_probes(probes: u32) -> Self {
        let width = ATLAS_WIDTH;
        let irr_per_row = width / IRR_TILE;
        let depth_per_row = width / DEPTH_TILE;
        let irr_rows = probes.div_ceil(irr_per_row);
        let depth_rows = probes.div_ceil(depth_per_row);
        let depth_y0 = irr_rows * IRR_TILE;
        let amb_y0 = depth_y0 + depth_rows * DEPTH_TILE;
        AtlasLayout { width, height: amb_y0 + 1, irr_per_row, depth_per_row, depth_y0, amb_y0, probes }
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
/// light closest point, range cut, the floor slabs that stand between,
/// `intensity / (1 + d^2)` falloff with the linear range window, spot cone,
/// and the 0.001 attenuation floor), in that order.
///
/// `slabs` is the light's RUN of floor slabs (`slabs_touched`), exactly the rows
/// its GPU record points at (`cone_inner.yz`, `light::gpu_packed_with_slabs`):
/// a light whose path to `p` crosses one of them delivers nothing. Empty for a
/// light no solid slab is near (every light of a one-storey home), and for the
/// probe update (`Twin::ray_radiance`, and `cs_trace` in
/// room_probes_update.wgsl, which tests no slab): a room box never holds a solid
/// slab, because `engine::room_gi` puts a room's lid at the underside of the
/// slab over it and its floor at the top of the slab under it, so a lamp inside
/// a box and a point on that box are always on the same side of every slab
/// (pinned by `engine::room_gi::tests::no_room_box_straddles_a_solid_slab`).
pub fn light_irradiance(l: &RoomLight, slabs: &[OccluderSlab], p: Vec3, n: Vec3) -> Vec3 {
    let mut pos = l.pos;
    if l.cos_outer < -1.5 {
        pos = line_light_closest_point(l.pos, l.dir, p);
    }
    let to_light = pos - p;
    let dist = to_light.length();
    if dist >= l.range {
        return Vec3::ZERO;
    }
    // A floor between storeys: the WGSL tests the light's run right here, after
    // the range reject (`slab_blocks_light`).
    if slabs.iter().any(|s| s.blocks_segment(p, pos)) {
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

// ── FLOORS BETWEEN STOREYS (ship homes increment 7, lane L) ──
//
// WHAT WAS WRONG. Point, spot and strip lights had no occlusion at all, and the
// home never casts into the sun shadow map, so with a second storey a lamp in
// the upstairs bedroom lit the dining room under it through the floor, and the
// sun through the glass roof lit every ground room under a solid floor. A floor
// slab is the one occluder in a home whose shape is exact and simple: a level
// box. So the renderer tests it exactly, in the fragment's light loop and at the
// sun term (docs/design/ship-homes-increment-7-plan.md section 6, the light-
// budget analysis), and nowhere else.
//
// THE TEST. A slab blocks the straight path from a point to a light when the two
// are on opposite sides of the slab's MID-PLANE and the path crosses that plane
// inside the slab's rectangle. The mid-plane rather than the box because it is
// one plane and one crossing: a path through the slab's top or underside crosses
// it, and a path that only clips a 0.25 m edge corner is the one case it lets
// by. The rectangle is shrunk by `SLAB_EDGE_TOL_M` so a fragment ON the slab's
// own edge face, whose crossing lands on the rectangle's border to within
// rounding, is not shadowed by its own slab. (The slab's top and underside need
// no tolerance: a light on the far side of either face already lies behind it,
// n.l < 0.)
//
// WHERE THE ROWS LIVE. In the room GI table every pipeline binds (group 0
// binding 7), after the rooms (`room_probes_gpu::RoomGi`): first each solid
// slab once (`GiHeader::atlas.w` of them, the sun's list), then for each scene
// light the RUN of slabs its range touches, copied, so a light's record names
// its run as (first, count) in `cone_inner.yz`. A light no slab is near carries
// (0, 0) and the fragment skips the test with one compare.
//
// CPU TWIN: this section and 85-room-gi.wgsl. Both walks go over
// `layered_rows`' packing: the lamp's `run_blocks` (the one-layer shortcut,
// then `segment_walk`) is `slab_blocks_light` and the sun's `sun_walk` is
// `slab_sun_blocked`, and their answers equal the
// per-slab references (`OccluderSlab::blocks_segment`, which
// `light_irradiance` uses, and `sun_blocked_by_slabs`). LOCKSTEP: change one,
// change both.

/// A crossing counts only this far inside the slab's rectangle (metres), so a
/// fragment on the slab's own edge face is not shadowed by its own slab. 1 cm:
/// the light it lets round a floor's edge is a 1 cm band.
pub const SLAB_EDGE_TOL_M: f32 = 0.01;

/// The sun test runs only for points within this distance of the box around
/// every room (`GiHeader` gmin/gmax, metres). A floor's shadow outside the ship's
/// rooms would land on the hull or in space, which nothing else aboard shadows
/// either, and planet vegetation seen from orbit skips the loop entirely.
pub const SLAB_SUN_REACH_M: f32 = 1.0;

/// A SOLID floor slab as the light sees it: an axis-aligned box from the
/// slab's underside to its top. Built by `engine::room_gi::occluder_slabs` in
/// the home frame and shifted into render space each frame with the rooms. A
/// glass slab passes light and is never one (`FloorSlab::passes_light`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OccluderSlab {
    pub min: Vec3,
    pub max: Vec3,
}

impl OccluderSlab {
    /// The same slab moved by `off` (home frame to render space).
    pub fn shifted(&self, off: Vec3) -> OccluderSlab {
        OccluderSlab { min: self.min + off, max: self.max + off }
    }

    /// The height of the plane the crossing test uses.
    pub fn mid_y(&self) -> f32 {
        (self.min.y + self.max.y) * 0.5
    }

    /// Distance from `p` to the slab's box (0 inside).
    pub fn outside_distance(&self, p: Vec3) -> f32 {
        (self.min - p).max(p - self.max).max(Vec3::ZERO).length()
    }

    /// True when the crossing point `c` lies inside the rectangle, shrunk by
    /// `SLAB_EDGE_TOL_M` (WGSL `slab_rect_holds`).
    fn rect_holds(&self, c: Vec3) -> bool {
        c.x > self.min.x + SLAB_EDGE_TOL_M
            && c.x < self.max.x - SLAB_EDGE_TOL_M
            && c.z > self.min.z + SLAB_EDGE_TOL_M
            && c.z < self.max.z - SLAB_EDGE_TOL_M
    }

    /// True when the slab stands between `p` and the point `q` (a lamp): they
    /// are on opposite sides of its mid-plane and the path crosses it inside
    /// the rectangle. WGSL `slab_segment_blocks`.
    pub fn blocks_segment(&self, p: Vec3, q: Vec3) -> bool {
        let m = self.mid_y();
        let (dp, dq) = (p.y - m, q.y - m);
        if dp * dq >= 0.0 {
            return false;
        }
        // dp and dq have opposite signs, so dp - dq is never 0.
        let t = dp / (dp - dq);
        self.rect_holds(p + (q - p) * t)
    }

    /// True when the slab stands between `p` and the sun, which lies along `d`
    /// at infinity (any length; the crossing does not depend on it). The
    /// per-slab reference the GPU's layered walk (`sun_walk`) must equal.
    pub fn blocks_ray(&self, p: Vec3, d: Vec3) -> bool {
        let dp = p.y - self.mid_y();
        if dp * d.y >= 0.0 {
            return false;
        }
        // Opposite signs again: d.y is not 0.
        let t = -dp / d.y;
        self.rect_holds(p + d * t)
    }
}

/// The slabs (indices into `slabs`) a light's range can reach past: the box is
/// nearer the light than its range (a point or spot light's sphere; for a line
/// light, conservatively, the segment's bounding box grown by the range). A
/// slab further away cannot change anything the light lights, because the
/// light's range window is already 0 there. Nearest first. These are the rows
/// the light's run copies (`room_probes_gpu`).
pub fn slabs_touched(l: &RoomLight, slabs: &[OccluderSlab]) -> Vec<usize> {
    let mut out: Vec<(f32, usize)> = Vec::new();
    for (i, s) in slabs.iter().enumerate() {
        let d = if l.cos_outer < -1.5 {
            // The segment's box grown by the range against the slab's box:
            // a superset of the true capsule test, which is all a run needs.
            let (lo, hi) = (l.pos.min(l.dir) - Vec3::splat(l.range), l.pos.max(l.dir) + Vec3::splat(l.range));
            if lo.cmple(s.max).all() && hi.cmpge(s.min).all() {
                s.outside_distance((l.pos + l.dir) * 0.5)
            } else {
                continue;
            }
        } else {
            let d = s.outside_distance(l.pos);
            if d >= l.range {
                continue;
            }
            d
        };
        out.push((d, i));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, i)| i).collect()
}

/// True when a floor slab stands between `p` and the sun (direction `sun_dir`,
/// toward the sun): the per-slab REFERENCE for `slab_sun_blocked` in
/// 85-room-gi.wgsl (which walks layers, `sun_walk`, to the same answer), which
/// `sun_shadow_offset` applies ahead of the shadow map, so it holds with sun
/// shadows switched off (a floor is not a quality option). `rooms_box` is the
/// room table's (gmin, gmax): only a point within `SLAB_SUN_REACH_M` of it is
/// tested.
pub fn sun_blocked_by_slabs(slabs: &[OccluderSlab], rooms_box: (Vec3, Vec3), p: Vec3, sun_dir: Vec3) -> bool {
    let reach = Vec3::splat(SLAB_SUN_REACH_M);
    if p.cmplt(rooms_box.0 - reach).any() || p.cmpgt(rooms_box.1 + reach).any() {
        return false;
    }
    slabs.iter().any(|s| s.blocks_ray(p, sun_dir))
}

/// SLABS AS THE GPU WALKS THEM (2026-10-21), the sun's list and every light's
/// run alike. Every slab with the same underside and top shares one
/// mid-plane, so a path (to the sun, or to a lamp) crosses all of them at ONE
/// point. The rows are grouped into LAYERS: a layer row (`bmin`/`bmax` = the
/// union box of its slabs, `counts.x` = how many slab rows follow) and then its
/// slabs, nearest `near` first (the eye for the sun, the light for a run), the
/// layers too. A fragment tests the layer's plane once, skips the whole layer
/// when it and the light are on the same side of it (a lamp on the fragment's
/// own storey) or when the crossing falls outside the union box (a low sun's
/// path), and only then tests the slabs. Measured with the upper-floor
/// layout's slabs, floors on against off: the per-slab walks read gpu.scene
/// +0.47 ms at the dawn park console-face-6 (the sun's 13 crossings, none
/// blocking) and, with the sun layered, +0.3 ms there (each lamp testing every
/// slab of its run). Exactly the per-slab answer (`sun_blocked_by_slabs`,
/// `OccluderSlab::blocks_segment`), pinned by
/// `the_layered_sun_walk_answers_as_every_slab_would` and
/// `the_layered_lamp_walk_answers_as_every_slab_would`.
pub fn layered_rows(slabs: &[OccluderSlab], near: Vec3) -> Vec<GiRoomGpu> {
    let mut layers: Vec<(f32, f32, Vec<OccluderSlab>)> = Vec::new();
    for s in slabs {
        match layers.iter_mut().find(|(lo, hi, _)| (lo - s.min.y).abs() <= 1e-4 && (hi - s.max.y).abs() <= 1e-4) {
            Some(l) => l.2.push(*s),
            None => layers.push((s.min.y, s.max.y, vec![*s])),
        }
    }
    let eye = near;
    let near = |v: &[OccluderSlab]| v.iter().map(|s| s.outside_distance(eye)).fold(f32::INFINITY, f32::min);
    for l in &mut layers {
        l.2.sort_by(|a, b| a.outside_distance(eye).total_cmp(&b.outside_distance(eye)));
    }
    layers.sort_by(|a, b| near(&a.2).total_cmp(&near(&b.2)));
    let mut rows = Vec::with_capacity(slabs.len() + layers.len());
    for (_, _, members) in &layers {
        let (lo, hi) =
            members.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), s| (a.min(s.min), b.max(s.max)));
        // The layer's OCCUPANCY GRID: a bit per cell of a LAYER_GRID_X by
        // LAYER_GRID_Z grid over the union box, set where any slab touches
        // the cell (its range widened by a thousandth of a cell, so a crossing
        // inside a slab always lands in a set cell whatever the rounding).
        let (dx, dz) = layer_cell_size(lo, hi);
        let mut words = [0u32; LAYER_GRID_WORDS];
        for s in members {
            let span = |a: f32, b: f32, o: f32, d: f32, n: u32| {
                let i0 = ((a - o) / d - 1e-3).floor().clamp(0.0, (n - 1) as f32) as u32;
                let i1 = ((b - o) / d + 1e-3).floor().clamp(0.0, (n - 1) as f32) as u32;
                (i0, i1)
            };
            let (x0, x1) = span(s.min.x, s.max.x, lo.x, dx, LAYER_GRID_X);
            let (z0, z1) = span(s.min.z, s.max.z, lo.z, dz, LAYER_GRID_Z);
            for iz in z0..=z1 {
                for ix in x0..=x1 {
                    let bit = iz * LAYER_GRID_X + ix;
                    words[(bit / 32) as usize] |= 1 << (bit % 32);
                }
            }
        }
        rows.push(GiRoomGpu {
            bmin: [lo.x, lo.y, lo.z, 0.0],
            bmax: [hi.x, hi.y, hi.z, 0.0],
            counts: [members.len() as u32, words[0], words[1], words[2]],
            lights: [words[3], words[4], words[5], words[6]],
            ..GiRoomGpu::default()
        });
        rows.extend(members.iter().map(pack_slab));
    }
    rows
}

/// A layer row's occupancy grid (`layered_rows`): LAYER_GRID_X columns along
/// x by LAYER_GRID_Z rows along z over the layer's union box, one bit a cell,
/// held in the row's seven spare integer lanes (`counts.yzw`, then
/// `lights.xyzw`). A crossing in a cell no slab touches misses every slab of
/// the layer, so the walk skips the slab tests: the common miss of a path
/// from upstairs to a lamp under glass or a gap (measured 2026-10-21 on the
/// shipped homestead: the upstairs bedroom read gpu.scene +0.59 ms with the
/// floors on, the walks of the downstairs lamps in its range testing every
/// slab of their runs and finding none). WGSL `SLAB_GRID_X` / `SLAB_GRID_Z`.
pub const LAYER_GRID_X: u32 = 16;
pub const LAYER_GRID_Z: u32 = 14;
const LAYER_GRID_WORDS: usize = 7;

/// The size of one grid cell over a union box, exactly as the shader computes
/// it.
fn layer_cell_size(lo: Vec3, hi: Vec3) -> (f32, f32) {
    (((hi.x - lo.x) / LAYER_GRID_X as f32).max(1e-6), ((hi.z - lo.z) / LAYER_GRID_Z as f32).max(1e-6))
}

/// Whether the cell of layer row `l`'s grid holding the crossing `c` has a
/// slab in it (WGSL `slab_layer_may_hold`).
pub fn layer_grid_holds(l: &GiRoomGpu, c: Vec3) -> bool {
    let (lo, hi) = (Vec3::new(l.bmin[0], l.bmin[1], l.bmin[2]), Vec3::new(l.bmax[0], l.bmax[1], l.bmax[2]));
    let (dx, dz) = layer_cell_size(lo, hi);
    let ix = ((c.x - lo.x) / dx).floor().clamp(0.0, (LAYER_GRID_X - 1) as f32) as u32;
    let iz = ((c.z - lo.z) / dz).floor().clamp(0.0, (LAYER_GRID_Z - 1) as f32) as u32;
    let bit = iz * LAYER_GRID_X + ix;
    let w = (bit / 32) as usize;
    let word = if w < 3 { l.counts[w + 1] } else { l.lights[w - 3] };
    (word >> (bit % 32)) & 1 != 0
}

/// A run's `cone_inner.w` when it has more than one layer: below every real
/// height, so the shader's one-layer shortcut (`plane > -1e38`) is off and it
/// walks the rows. WGSL `SLAB_RUN_MANY_LAYERS`.
pub const RUN_MANY_LAYERS: f32 = -3.0e38;

/// The mid-plane height of a run's one layer (`layered_rows` packing), or
/// `RUN_MANY_LAYERS` when it has several (or none). A two-storey home has one
/// floor layer, so every run there has one, and the light record carries it
/// (`light::gpu_packed_with_slabs`): a lamp on the fragment's own side of it
/// is settled without reading a row (measured 2026-10-21 on the shipped
/// homestead: the upstairs bedroom read gpu.scene +0.59 ms with the floors on,
/// every in-range lamp reading its layer row on every fragment).
pub fn run_plane(rows: &[GiRoomGpu]) -> f32 {
    match rows.first() {
        Some(l) if l.counts[0] as usize + 1 == rows.len() => (l.bmin[1] + l.bmax[1]) * 0.5,
        _ => RUN_MANY_LAYERS,
    }
}

/// The CPU transcription of `slab_blocks_light` in 85-room-gi.wgsl: the
/// one-layer shortcut on the light record's `plane` (`run_plane`), then the
/// walk over the run (`segment_walk`). Whether a slab stands between `p` and
/// the lamp at `q`, and how many rows were read.
pub fn run_blocks(rows: &[GiRoomGpu], plane: f32, p: Vec3, q: Vec3) -> (bool, u32) {
    if plane > -1.0e38 && (p.y - plane) * (q.y - plane) >= 0.0 {
        return (false, 0);
    }
    segment_walk(rows, p, q)
}

/// The walk half of `slab_blocks_light` in 85-room-gi.wgsl, over a light's
/// run (`layered_rows` of its `slabs_touched`, nearest the light first)
/// exactly as the GPU does: whether a slab stands between `p` and the lamp at
/// `q`, and how many rows the walk read (the cost).
pub fn segment_walk(rows: &[GiRoomGpu], p: Vec3, q: Vec3) -> (bool, u32) {
    let (mut k, mut read) = (0usize, 0u32);
    while k < rows.len() {
        let l = &rows[k];
        let n = l.counts[0] as usize;
        read += 1;
        let (lmin, lmax) = (Vec3::new(l.bmin[0], l.bmin[1], l.bmin[2]), Vec3::new(l.bmax[0], l.bmax[1], l.bmax[2]));
        let m = (lmin.y + lmax.y) * 0.5;
        let (dp, dq) = (p.y - m, q.y - m);
        if dp * dq < 0.0 {
            let c = p + (q - p) * (dp / (dp - dq));
            if c.x > lmin.x && c.x < lmax.x && c.z > lmin.z && c.z < lmax.z && layer_grid_holds(l, c) {
                for j in 1..=n {
                    read += 1;
                    let s = &rows[k + j];
                    let slab = OccluderSlab {
                        min: Vec3::new(s.bmin[0], s.bmin[1], s.bmin[2]),
                        max: Vec3::new(s.bmax[0], s.bmax[1], s.bmax[2]),
                    };
                    if slab.rect_holds(c) {
                        return (true, read);
                    }
                }
            }
        }
        k += 1 + n;
    }
    (false, read)
}

/// The CPU transcription of `slab_sun_blocked` in 85-room-gi.wgsl, walking
/// `layered_rows`' packing exactly as the GPU does: whether a slab stands between
/// `p` and the sun along `d`, and how many rows the walk read (the cost). With
/// `rooms_box` the room table's (gmin, gmax).
pub fn sun_walk(rows: &[GiRoomGpu], rooms_box: (Vec3, Vec3), p: Vec3, d: Vec3) -> (bool, u32) {
    let reach = Vec3::splat(SLAB_SUN_REACH_M);
    if p.cmplt(rooms_box.0 - reach).any() || p.cmpgt(rooms_box.1 + reach).any() {
        return (false, 0);
    }
    let (mut k, mut read) = (0usize, 0u32);
    while k < rows.len() {
        let l = &rows[k];
        let n = l.counts[0] as usize;
        read += 1;
        let (lmin, lmax) = (Vec3::new(l.bmin[0], l.bmin[1], l.bmin[2]), Vec3::new(l.bmax[0], l.bmax[1], l.bmax[2]));
        let dp = p.y - (lmin.y + lmax.y) * 0.5;
        if dp * d.y < 0.0 {
            let c = p + d * (-dp / d.y);
            if c.x > lmin.x && c.x < lmax.x && c.z > lmin.z && c.z < lmax.z && layer_grid_holds(l, c) {
                for j in 1..=n {
                    read += 1;
                    let s = &rows[k + j];
                    let slab = OccluderSlab {
                        min: Vec3::new(s.bmin[0], s.bmin[1], s.bmin[2]),
                        max: Vec3::new(s.bmax[0], s.bmax[1], s.bmax[2]),
                    };
                    if slab.rect_holds(c) {
                        return (true, read);
                    }
                }
            }
        }
        k += 1 + n;
    }
    (false, read)
}

/// Merge slabs whose union is exactly one box: the same underside and top, and
/// a whole shared edge (side by side in x over the same z span, or in z over
/// the same x span). Exact, so the union covers the same floor and nothing
/// more; it only shortens the lists every fragment walks. The homestead's
/// upper floor (19 slabs, 16 solid) merges to 13: the plant room, forge and
/// workshop roofs become one, and the mushroom and aquaponics roofs another.
pub fn merge_slabs(mut slabs: Vec<OccluderSlab>) -> Vec<OccluderSlab> {
    const EPS: f32 = 1e-4;
    let near = |a: f32, b: f32| (a - b).abs() <= EPS;
    loop {
        let mut merged = None;
        'scan: for i in 0..slabs.len() {
            for j in i + 1..slabs.len() {
                let (a, b) = (slabs[i], slabs[j]);
                if !(near(a.min.y, b.min.y) && near(a.max.y, b.max.y)) {
                    continue;
                }
                let same_z = near(a.min.z, b.min.z) && near(a.max.z, b.max.z);
                let same_x = near(a.min.x, b.min.x) && near(a.max.x, b.max.x);
                let x_touch = near(a.max.x, b.min.x) || near(b.max.x, a.min.x);
                let z_touch = near(a.max.z, b.min.z) || near(b.max.z, a.min.z);
                if (same_z && x_touch) || (same_x && z_touch) {
                    merged = Some((i, j, OccluderSlab { min: a.min.min(b.min), max: a.max.max(b.max) }));
                    break 'scan;
                }
            }
        }
        let Some((i, j, m)) = merged else {
            return slabs;
        };
        slabs[i] = m;
        slabs.remove(j);
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

// ── THE ROOM'S AMBIENT COLOUR (2026-10-21, docs/design/room-gi.md) ──
//
// WHAT IT IS FOR. The operator asked (2026-10-20) why a room needs thousands of
// probes rather than a brighter texture. The honest middle answer: one colour
// per room, worked out from that room's OWN lamps and windows. It follows the
// lamps (switch one off and its room darkens) and keeps rooms apart (a lamp
// lights only the room it is in, `assign_lights`), but it is flat within a
// room: the wall by the lamp and the corner across the room read the same. It
// is the Bounced light detail level "Ambient" (no probes at all), and the
// colour every room shows while its probes are not resident (probe streaming,
// `Residency`), which is also where a room's probes start when they come in
// (`Twin::seed`, `cs_seed`).
//
// THE PHYSICS is RADIOSITY ON THE ROOM'S SIX FACES, the original radiosity
// method on the box it was first shown on (Goral, Torrance, Greenberg and
// Battaile, "Modeling the Interaction of Light Between Diffuse Surfaces",
// SIGGRAPH 1984): each face one patch of uniform radiosity B_f, exchanging
// light with the other five through the ANALYTIC form factors of a box (two
// cases cover all thirty: opposite faces are directly opposed parallel
// rectangles, Howell's catalogue C-11; neighbouring faces are perpendicular
// rectangles sharing an edge, C-14; J. R. Howell, "A Catalog of Radiation Heat
// Transfer Configuration Factors"). Solved exactly, a 6 x 6 system per channel:
//
//     B_f = S_f + rho_f ( Phi_f / A_f + sum_g F_fg B_g )
//
// with Phi_f the direct flux on face f (the room's lamps, and the sun through a
// glass lid) and S the sky coming in through a glass lid (the lid's exitance
// into the room, T x pi x the cosine-weighted mean sky radiance). The light
// arriving INDIRECTLY at face f is then H_f = sum_g F_fg B_g: every reflection,
// and the sky's first arrival (the fragment has no sky term indoors, the probes
// carry it the same way), but not the lamps' or the sun's first arrival, which
// the fragment lights itself. A lid's reflectance is its glass's alpha x colour,
// so what the glass lets out is lost, as in the probes. The room's ONE colour
// is the area-weighted mean of H_f / pi over its opaque faces (a glass lid's
// fragments keep the old floor, so its own arrival is left out).
//
// Why not the integrating-sphere relation (rho_bar Phi / (A (1 - rho_bar)), the
// form progressive radiosity's ambient term takes, Cohen et al. 1988)? It is
// exact for a uniformly lit room, and this solve reduces to it there
// (`the_ambient_colour_is_the_integrating_sphere_value`), but it hands each
// face's reflection back to every face by area, the face itself included. A
// floor in the sun under a glass roof reflects mostly up into the glass, where
// it is lost; the sphere handed a third of it back to the floor and read 1.6
// times what the converged probes give the room's surfaces (measured
// 2026-10-21, `the_ambient_colour_is_the_mean_of_the_converged_probes`).
//
// THE INTEGRAL is a fixed grid: each face cut into AMBIENT_GRID x AMBIENT_GRID
// cells, the direct light taken at each cell's centre with exactly the probes'
// sources (`direct_irradiance`), and the sky read along AMBIENT_SKY_DIRS fixed
// directions spread over the upper hemisphere with a cosine weighting. Fixed,
// not random: the same lamps give the same colour every frame, so a whole room
// never shimmers, and a lamp switched off changes it that very frame. The GPU
// runs it as `cs_ambient`, one workgroup of 64 per room (a cell of every face
// and one sky direction per thread, the solve on thread 0), every frame.
//
// LOCKSTEP: `face_cell_point`, `sky_dir`, `ff_parallel`, `ff_perpendicular`,
// `box_form_factor`, `ambient_from_faces` and the constants here are
// `gi_amb_cell_point`, `gi_amb_sky_dir`, `gi_ff_parallel`,
// `gi_ff_perpendicular`, `gi_box_form_factor`, `gi_ambient_from_faces` and
// `GI_AMB_*` in room_probes_update.wgsl.

/// Cells per side of each face's grid (64 a face, one per thread of
/// `cs_ambient`). WGSL `GI_AMB_GRID`.
pub const AMBIENT_GRID: u32 = 8;
pub const AMBIENT_CELLS: u32 = AMBIENT_GRID * AMBIENT_GRID;
/// Directions the sky is read along through a glass lid (one per thread).
/// WGSL `GI_AMB_SKY_DIRS`.
pub const AMBIENT_SKY_DIRS: u32 = 64;
/// The update count a seeded probe starts at (`Twin::seed`, `cs_seed`): the
/// seed weighs as this many updates in the running mean (`blend_weight_h`),
/// so the probe's own estimates take over over its first few updates. Taking
/// the first estimate whole (count 0) would throw the seed away for one noisy
/// 64-ray estimate, the pop the seed is there to prevent. WGSL `GI_SEED_COUNT`.
pub const SEED_COUNT: f32 = 8.0;

/// Area of face `face` of `room`.
pub fn face_area(room: &RoomBox, face: usize) -> f32 {
    let s = room.size();
    match face {
        FACE_NEG_X | FACE_POS_X => s.y * s.z,
        FACE_FLOOR | FACE_LID => s.x * s.z,
        _ => s.x * s.y,
    }
}

/// The centre of cell `cell` (x fastest) of face `face`'s AMBIENT_GRID x
/// AMBIENT_GRID grid, on the face itself.
pub fn face_cell_point(room: &RoomBox, face: usize, cell: u32) -> Vec3 {
    let u = ((cell % AMBIENT_GRID) as f32 + 0.5) / AMBIENT_GRID as f32;
    let v = ((cell / AMBIENT_GRID) as f32 + 0.5) / AMBIENT_GRID as f32;
    let (a, b) = (room.min, room.max);
    let at = |lo: f32, hi: f32, t: f32| lo + (hi - lo) * t;
    match face {
        FACE_NEG_X => Vec3::new(a.x, at(a.y, b.y, u), at(a.z, b.z, v)),
        FACE_POS_X => Vec3::new(b.x, at(a.y, b.y, u), at(a.z, b.z, v)),
        FACE_FLOOR => Vec3::new(at(a.x, b.x, u), a.y, at(a.z, b.z, v)),
        FACE_LID => Vec3::new(at(a.x, b.x, u), b.y, at(a.z, b.z, v)),
        FACE_NEG_Z => Vec3::new(at(a.x, b.x, u), at(a.y, b.y, v), a.z),
        _ => Vec3::new(at(a.x, b.x, u), at(a.y, b.y, v), b.z),
    }
}

/// Sky direction `i` of AMBIENT_SKY_DIRS: a Fibonacci spiral over the upper
/// hemisphere (+y up) with sin^2 of the angle from the zenith spread evenly,
/// which is a cosine weighting, so the plain mean of the sky's radiance along
/// these is (1 / pi) x the integral of radiance x cos over the hemisphere.
pub fn sky_dir(i: u32) -> Vec3 {
    let s2 = (i as f32 + 0.5) / AMBIENT_SKY_DIRS as f32;
    let s = s2.sqrt();
    let phi = std::f32::consts::TAU * ((i as f32 * 0.618_034) % 1.0);
    Vec3::new(phi.cos() * s, (1.0 - s2).max(0.0).sqrt(), phi.sin() * s)
}

/// The form factor between two equal, directly opposed parallel rectangles
/// `a` x `b` a distance `c` apart (Howell's catalogue C-11). A unit cube's
/// opposite faces: 0.19982.
pub fn ff_parallel(a: f32, b: f32, c: f32) -> f32 {
    let (x, y) = (a / c.max(1.0e-6), b / c.max(1.0e-6));
    let (x2, y2) = (x * x, y * y);
    let (sx, sy) = ((1.0 + x2).sqrt(), (1.0 + y2).sqrt());
    let t = 0.5 * ((1.0 + x2) * (1.0 + y2) / (1.0 + x2 + y2)).ln() + x * sy * (x / sy).atan() + y * sx * (y / sx).atan()
        - x * x.atan()
        - y * y.atan();
    (2.0 / (std::f32::consts::PI * x * y) * t).max(0.0)
}

/// The form factor FROM a rectangle `l` x `w` TO a perpendicular rectangle
/// `l` x `h` sharing its edge of length `l` (Howell's catalogue C-14). A unit
/// cube's neighbouring faces: 0.20004. The logarithm is taken as a sum of
/// logarithms so the powers W^2 and H^2 of a long room do not overflow.
pub fn ff_perpendicular(l: f32, w: f32, h: f32) -> f32 {
    let l = l.max(1.0e-6);
    let (ww, hh) = (w / l, h / l);
    let (w2, h2) = (ww * ww, hh * hh);
    let r = (w2 + h2).sqrt();
    let atans = ww * (1.0 / ww).atan() + hh * (1.0 / hh).atan() - r * (1.0 / r).atan();
    let logs = ((1.0 + w2) * (1.0 + h2) / (1.0 + w2 + h2)).ln()
        + w2 * (w2 * (1.0 + w2 + h2) / ((1.0 + w2) * (w2 + h2))).ln()
        + h2 * (h2 * (1.0 + w2 + h2) / ((1.0 + h2) * (w2 + h2))).ln();
    ((atans + 0.25 * logs) / (std::f32::consts::PI * ww)).max(0.0)
}

/// The axis a face is perpendicular to (0 x, 1 y, 2 z) in FACE order.
fn face_axis(face: usize) -> usize {
    face / 2
}

/// The form factor from face `i` to face `j` of a box of `size` (0 for a face
/// to itself): C-11 for opposite faces, C-14 for neighbours. The rows are not
/// yet held to sum to 1 (`box_form_factors` does that).
pub fn box_form_factor(size: Vec3, i: usize, j: usize) -> f32 {
    let s = size.to_array();
    let (ki, kj) = (face_axis(i), face_axis(j));
    if i == j {
        0.0
    } else if ki == kj {
        ff_parallel(s[(ki + 1) % 3], s[(ki + 2) % 3], s[ki])
    } else {
        // The edge they share runs along the third axis; face i reaches away
        // from it along face j's axis, face j along face i's.
        ff_perpendicular(s[3 - ki - kj], s[kj], s[ki])
    }
}

/// Every form factor of a box, F[i][j] from face i to face j, each row
/// scaled to sum to exactly 1 (a closed box: all of a face's light lands on
/// the other five), which also absorbs the formulas' rounding in a very long
/// or very flat room.
pub fn box_form_factors(size: Vec3) -> [[f32; 6]; 6] {
    let mut f = [[0.0f32; 6]; 6];
    for (i, row) in f.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = box_form_factor(size, i, j);
        }
        let sum: f32 = row.iter().sum();
        if sum > 0.0 {
            row.iter_mut().for_each(|v| *v /= sum);
        }
    }
    f
}

/// THE ROOM'S AMBIENT COLOUR from its faces' direct flux `phi` (FACE order)
/// and the sky's cosine-weighted mean radiance `sky_mean` through a glass lid:
/// the six-face radiosity solve (the section comment), then the area-weighted
/// mean over the opaque faces of the light arriving indirectly, as E / pi.
/// Gaussian elimination without pivoting is safe here: each row of
/// I - rho F has 1 on the diagonal against at most rho < 1 off it.
pub fn ambient_from_faces(room: &RoomBox, phi: [Vec3; 6], sky_mean: Vec3) -> Vec3 {
    let size = room.size();
    let f = box_form_factors(size);
    let mut m = [[Vec3::ZERO; 6]; 6];
    let mut rhs = [Vec3::ZERO; 6];
    for i in 0..6 {
        let rho = Vec3::from(room.refl[i]);
        let area = face_area(room, i).max(1.0e-6);
        for j in 0..6 {
            m[i][j] = if i == j { Vec3::ONE } else { Vec3::ZERO } - rho * f[i][j];
        }
        let sky = if i == FACE_LID { sky_mean * std::f32::consts::PI * room.lid_transmittance } else { Vec3::ZERO };
        rhs[i] = sky + rho * phi[i] / area;
    }
    for k in 0..6 {
        for i in k + 1..6 {
            let q = m[i][k] / m[k][k];
            for j in k..6 {
                m[i][j] -= q * m[k][j];
            }
            rhs[i] -= q * rhs[k];
        }
    }
    let mut b = [Vec3::ZERO; 6];
    for i in (0..6).rev() {
        let mut s = rhs[i];
        for j in i + 1..6 {
            s -= m[i][j] * b[j];
        }
        b[i] = s / m[i][i];
    }
    let (mut sum, mut area) = (Vec3::ZERO, 0.0f32);
    for i in 0..6 {
        if i == FACE_LID && room.lid_transmittance > 0.0 {
            continue;
        }
        let incident = (0..6).fold(Vec3::ZERO, |acc, j| acc + b[j] * f[i][j]);
        sum += incident * face_area(room, i);
        area += face_area(room, i);
    }
    if area <= 1.0e-6 {
        return Vec3::ZERO;
    }
    sum / (area * std::f32::consts::PI)
}

/// THE ROOM'S AMBIENT COLOUR, E / pi: `room`'s faces lit by `direct`
/// (`direct_irradiance`) on the fixed grid, `sky(d)` the sky's radiance along
/// d through a glass lid (zero for a closed lid; the twin passes zero, the GPU
/// reads the frame's sky-view table), then `ambient_from_faces`. `cs_ambient`.
pub fn room_ambient(room: &RoomBox, direct: &DirectSource, sky: impl Fn(Vec3) -> Vec3) -> Vec3 {
    let mut phi = [Vec3::ZERO; 6];
    for (face, flux) in phi.iter_mut().enumerate() {
        let n = face_normal(face);
        let cell_area = face_area(room, face) / AMBIENT_CELLS as f32;
        for cell in 0..AMBIENT_CELLS {
            *flux += direct_irradiance(room, direct, face_cell_point(room, face, cell), n) * cell_area;
        }
    }
    let sky_mean = if room.lid_transmittance > 0.0 {
        (0..AMBIENT_SKY_DIRS).fold(Vec3::ZERO, |acc, i| acc + sky(sky_dir(i))) / AMBIENT_SKY_DIRS as f32
    } else {
        Vec3::ZERO
    };
    ambient_from_faces(room, phi, sky_mean)
}

// ── PROBE STREAMING (2026-10-21, docs/design/room-gi.md "Probe streaming") ──
//
// WHAT WAS WRONG. The atlas held every room of the ship at once, so the ship's
// size decided every room's spacing: the 1.1 km street and the second commons
// (2026-10-05) coarsened the home to 1.33 m, and the home-first priority
// (2026-10-20) kept the home at 1 m only by taking everything else to 2.5 m.
// The operator asked whether there was another way; this is it.
//
// WHAT IT DOES. The atlas holds a fixed budget of probes (the level's cap), and
// the rooms NEAREST THE PLAYER hold them (`room_priority`: distance from the
// eye to the room's box, plus `STOREY_PENALTY_M` for a room on another
// storey). Every frame `Residency::step` works out the set that fits the
// budget nearest first, lets a resident room that dropped out of it fade to
// its ambient colour over `LEAVE_FADE_FRAMES` (its probe weight going 1 to 0)
// before its probes are given away, and places each room that came into it in
// the atlas, where its probes are seeded with its ambient colour (`Twin::seed`,
// `cs_seed`): the frame before, the room showed its ambient colour; the frame
// it comes in, its probes read that same colour; from then on its own updates
// take over. A resident room keeps its place against a newcomer up to
// `RESIDENCY_HYSTERESIS_M` nearer, so a player standing at the boundary does
// not swap two rooms in and out every frame.

/// A room on a different storey from the eye (the eye's height outside its
/// box by more than `STOREY_SLACK_M`) counts this much farther: a floor stands
/// between, so it is mostly out of sight.
pub const STOREY_PENALTY_M: f32 = 8.0;
pub const STOREY_SLACK_M: f32 = 0.5;
/// A resident room counts this much nearer when the budget is shared out, so
/// it is not given up for a room only a step nearer (no thrash at a
/// boundary).
pub const RESIDENCY_HYSTERESIS_M: f32 = 4.0;
/// Frames a leaving room takes to fade from its probes to its ambient colour
/// (and a room that is wanted back mid-fade takes to return).
pub const LEAVE_FADE_FRAMES: u32 = 20;
/// The most probes seeded in one frame (`cs_seed`, 424 texel writes each). A
/// room is seeded whole or waits a frame; the largest room is a quarter of
/// the largest budget, 16,384 at Ultra, so it always fits an empty frame.
pub const MAX_SEEDS_PER_FRAME: u32 = 16_384;

/// How far room `room` is from the eye for sharing out the probe budget:
/// the distance from the eye to its box (0 inside), plus `STOREY_PENALTY_M`
/// when it is on another storey.
pub fn room_priority(room: &RoomBox, eye: Vec3) -> f32 {
    let other_storey = eye.y < room.min.y - STOREY_SLACK_M || eye.y > room.max.y + STOREY_SLACK_M;
    room.outside_distance(eye) + if other_storey { STOREY_PENALTY_M } else { 0.0 }
}

/// A room holding probes: where (`first`, `count` probes from there), its
/// probe weight, and whether it is fading out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resident {
    pub first: u32,
    pub count: u32,
    pub weight: f32,
    pub leaving: bool,
}

/// One room's probes moved within the atlas when it is packed: `count`
/// probes from slot `from` to slot `to` (always down: `to < from`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileMove {
    pub room: usize,
    pub from: u32,
    pub to: u32,
    pub count: u32,
}

/// What one `Residency::step` changed (room indices).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResidencyStep {
    /// Placed this frame: seed their probes now.
    pub entered: Vec<usize>,
    /// Began fading out this frame.
    pub started_leaving: Vec<usize>,
    /// Finished fading: their probes are free.
    pub left: Vec<usize>,
    /// Wanted but not placed this frame (the space they need is still held
    /// by rooms fading out, or the frame's seed budget is spent).
    pub waiting: Vec<usize>,
    /// The atlas was packed this frame: these rooms' tiles move down, in this
    /// order, before anything reads or writes them (`room_probes_gpu`, through
    /// the scratch, every move in one frame).
    pub moved: Vec<TileMove>,
}

impl ResidencyStep {
    pub fn changed(&self) -> bool {
        !self.entered.is_empty() || !self.started_leaving.is_empty() || !self.left.is_empty() || !self.moved.is_empty()
    }
}

/// Which rooms hold probes, and where in the atlas (probe indices
/// `0..cap`). Pure bookkeeping, so it is tested without a GPU.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Residency {
    pub cap: u32,
    pub slots: Vec<Option<Resident>>,
}

impl Residency {
    pub fn new(cap: u32, rooms: usize) -> Self {
        Residency { cap, slots: vec![None; rooms] }
    }

    /// Where room `i`'s probes are this frame, for its table row.
    pub fn placement(&self, i: usize) -> Placement {
        match self.slots.get(i).copied().flatten() {
            Some(r) => Placement { first_probe: r.first, weight: r.weight },
            None => NOT_RESIDENT,
        }
    }

    /// Room `i` holds probes that are updated (resident, not fading out).
    pub fn updated(&self, i: usize) -> Option<Resident> {
        self.slots.get(i).copied().flatten().filter(|r| !r.leaving)
    }

    /// Probes held, fading rooms included (they still hold their tiles).
    pub fn resident_probes(&self) -> u32 {
        self.slots.iter().flatten().map(|r| r.count).sum()
    }

    pub fn resident_rooms(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    /// The free stretches of the atlas, (start, length), in order.
    pub fn free_ranges(&self) -> Vec<(u32, u32)> {
        let mut held: Vec<(u32, u32)> = self.slots.iter().flatten().map(|r| (r.first, r.count)).collect();
        held.sort();
        let mut out = Vec::new();
        let mut at = 0u32;
        for (first, count) in held {
            if first > at {
                out.push((at, first - at));
            }
            at = at.max(first + count);
        }
        if self.cap > at {
            out.push((at, self.cap - at));
        }
        out
    }

    /// The same rooms after a relayout (the construction editor rebuilt the
    /// home): a new room that is exactly an old resident room (the same box,
    /// faces and lattice) keeps that room's probes where they are; every other
    /// room starts out of the atlas and comes back in through `step`, seeded.
    pub fn carried_over(&self, old: (&[RoomBox], &[[u32; 3]]), new: (&[RoomBox], &[[u32; 3]])) -> Residency {
        let mut out = Residency::new(self.cap, new.0.len());
        let mut taken = vec![false; old.0.len()];
        for (ni, (room, counts)) in new.0.iter().zip(new.1).enumerate() {
            let found = (0..old.0.len()).find(|&oi| {
                !taken[oi]
                    && old.0[oi] == *room
                    && old.1[oi] == *counts
                    && self.slots.get(oi).copied().flatten().is_some_and(|r| !r.leaving)
            });
            if let Some(oi) = found {
                taken[oi] = true;
                out.slots[ni] = self.slots[oi];
            }
        }
        out
    }

    /// One frame of residency for `rooms` (render space, the eye's frame)
    /// with `probes[i]` probes each:
    /// 1. fades advance (a leaving room's weight falls by 1 / LEAVE_FADE_FRAMES
    ///    and its probes are freed at 0; a room wanted back rises again);
    /// 2. the WANTED set is the rooms that fit the budget nearest first
    ///    (`room_priority`, residents `RESIDENCY_HYSTERESIS_M` nearer);
    /// 3. a resident room no longer wanted starts to fade out;
    /// 4. each wanted room not resident is placed in the smallest free gap
    ///    that holds it (best fit) while the frame's seed budget lasts. When
    ///    the atlas has the probes free but in pieces, it is PACKED first
    ///    (`pack`): every resident room's tiles slide down to close the gaps,
    ///    so the free probes become one gap at the end.
    pub fn step(&mut self, rooms: &[RoomBox], probes: &[u32], eye: Vec3) -> ResidencyStep {
        let n = rooms.len().min(probes.len());
        self.slots.resize(n, None);
        let mut out = ResidencyStep::default();
        let fade = 1.0 / LEAVE_FADE_FRAMES as f32;
        // 1. Fades.
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(r) = slot {
                if r.leaving {
                    r.weight -= fade;
                    if r.weight <= 1.0e-4 {
                        *slot = None;
                        out.left.push(i);
                    }
                } else if r.weight < 1.0 {
                    r.weight = (r.weight + fade).min(1.0);
                }
            }
        }
        // 2. The wanted set.
        let key: Vec<f32> = (0..n)
            .map(|i| {
                let held = self.slots[i].is_some_and(|r| !r.leaving);
                room_priority(&rooms[i], eye) - if held { RESIDENCY_HYSTERESIS_M } else { 0.0 }
            })
            .collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| key[a].total_cmp(&key[b]).then(a.cmp(&b)));
        let mut wanted = vec![false; n];
        let mut left = self.cap;
        for &i in &order {
            if probes[i] > 0 && probes[i] <= left {
                wanted[i] = true;
                left -= probes[i];
            }
        }
        // 3. Out of the wanted set: fade out. Wanted back mid-fade: return.
        for i in 0..n {
            if let Some(r) = self.slots[i].as_mut() {
                if !r.leaving && !wanted[i] {
                    r.leaving = true;
                    out.started_leaving.push(i);
                } else if r.leaving && wanted[i] {
                    r.leaving = false;
                }
            }
        }
        // 4. Place the newcomers, nearest first.
        let mut seeded = 0u32;
        for &i in &order {
            if !wanted[i] || self.slots[i].is_some() {
                continue;
            }
            let need = probes[i];
            if seeded + need > MAX_SEEDS_PER_FRAME {
                out.waiting.push(i);
                continue;
            }
            let mut free = self.free_ranges();
            let fits = |free: &[(u32, u32)]| free.iter().filter(|g| g.1 >= need).min_by_key(|g| (g.1, g.0)).copied();
            if fits(&free).is_none() && free.iter().map(|g| g.1).sum::<u32>() >= need {
                out.moved.extend(self.pack());
                free = self.free_ranges();
            }
            match fits(&free) {
                Some((start, _)) => {
                    self.slots[i] = Some(Resident { first: start, count: need, weight: 1.0, leaving: false });
                    seeded += need;
                    out.entered.push(i);
                }
                None => out.waiting.push(i),
            }
        }
        out
    }

    /// PACK the atlas: every resident room (fading ones too, which are still
    /// drawn) slides down to the end of the room before it, in atlas order, so
    /// the free probes become one gap at the end. Returns the moves, lowest
    /// first, each one down, which is the order a memmove copies them in
    /// safely: a room's new place never covers a later room's old one.
    ///
    /// WHY PACK, NOT EVICT (2026-10-21). The first cut made the farthest room
    /// whose place would open a big enough gap leave. On the shipped ship at
    /// Low the street then stayed out in the Commons: every room in the way
    /// was nearer than it, so none would give way, while 2,000 free probes lay
    /// in pieces between them (`engine::room_gi::tests::the_shipped_ship_
    /// streams_its_probes_from_the_home_to_the_commons_and_back`). The wanted
    /// set always fits the budget, so once the rooms fading out are gone,
    /// packing always makes room, and nobody's light is lost: the tiles move,
    /// they are not seeded again.
    pub fn pack(&mut self) -> Vec<TileMove> {
        let mut order: Vec<usize> = (0..self.slots.len()).filter(|&i| self.slots[i].is_some()).collect();
        order.sort_by_key(|&i| self.slots[i].map_or(0, |r| r.first));
        let mut moves = Vec::new();
        let mut at = 0u32;
        for i in order {
            if let Some(r) = self.slots[i].as_mut() {
                if r.first != at {
                    moves.push(TileMove { room: i, from: r.first, to: at, count: r.count });
                    r.first = at;
                }
                at += r.count;
            }
        }
        moves
    }
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
    /// x = room count, y = RESIDENT probe count, z = irradiance tiles per
    /// atlas row, w = depth tiles per atlas row.
    pub info: [u32; 4],
    /// x = atlas width, y = atlas height (its last row is the ambient row),
    /// z = first texel row of the depth region.
    pub atlas: [u32; 4],
}

/// `GiRoom`, 160 bytes, one per room after the header.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GiRoomGpu {
    /// xyz = box min (render space), w = the PROBE WEIGHT: 1 = the room's
    /// probes alone, 0 = its ambient colour alone (not resident, or the
    /// Ambient level), between = a resident room fading out (`Residency`).
    pub bmin: [f32; 4],
    /// xyz = box max, w = lid transmittance.
    pub bmax: [f32; 4],
    /// xyz = probes per axis, w = the room's first probe in the atlas (0 and
    /// never read while the weight is 0).
    pub counts: [u32; 4],
    /// x = first light in the update's light list, y = how many, z = flags
    /// (`ROOM_FLAG_VISIBILITY`), w = the room's own index, which is its texel
    /// in the ambient row (the table is in pick order, which changes every
    /// frame; the index does not).
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
    /// x = settled hysteresis, y = unused, z = normal bias, w = unused.
    pub misc: [f32; 4],
    /// x = probes traced in this dispatch (the list's first x entries; the
    /// probes to seed follow them, `cs_seed`), y = scratch irradiance tiles
    /// per row, z = scratch depth tiles per row, w = first row of the scratch
    /// depth region.
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

/// Where a room stands in the probe atlas this frame, for its table row:
/// its first probe and its probe weight (`GiRoomGpu::bmin.w`). A room that is
/// not resident is `NOT_RESIDENT`: weight 0, its ambient colour alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub first_probe: u32,
    pub weight: f32,
}

pub const NOT_RESIDENT: Placement = Placement { first_probe: 0, weight: 0.0 };

/// Pack one room for the GPU. `index` is the room's own index (its texel in
/// the ambient row), `at` where its probes are, if anywhere.
pub fn pack_room(
    r: &RoomBox,
    index: u32,
    counts: [u32; 3],
    at: Placement,
    first_light: u32,
    light_count: u32,
    visibility: bool,
) -> GiRoomGpu {
    let mut refl = [[0.0f32; 4]; 6];
    for (dst, src) in refl.iter_mut().zip(r.refl.iter()) {
        *dst = [src[0], src[1], src[2], 0.0];
    }
    GiRoomGpu {
        bmin: [r.min.x, r.min.y, r.min.z, at.weight.clamp(0.0, 1.0)],
        bmax: [r.max.x, r.max.y, r.max.z, r.lid_transmittance],
        counts: [counts[0], counts[1], counts[2], at.first_probe],
        lights: [first_light, light_count, if visibility { ROOM_FLAG_VISIBILITY } else { 0 }, index],
        refl,
    }
}

/// Pack one floor slab as a row of the room table (ship homes increment 7):
/// a `GiRoomGpu`-sized row whose bmin.xyz and bmax.xyz are the slab's box and
/// nothing else is read (85-room-gi.wgsl, "FLOORS BETWEEN STOREYS"). Rows of
/// the one record type, so the table every pipeline binds keeps its layout.
pub fn pack_slab(s: &OccluderSlab) -> GiRoomGpu {
    GiRoomGpu {
        bmin: [s.min.x, s.min.y, s.min.z, 0.0],
        bmax: [s.max.x, s.max.y, s.max.z, 0.0],
        ..GiRoomGpu::default()
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

/// The irradiance a surface of `room` at `h` facing `n` takes STRAIGHT from
/// its sources: the room's own lights (`light_irradiance`, no slab run: a
/// room box never holds a solid slab) and the sun if the way to it leaves
/// through a glass lid. The one half `cs_trace` (a ray's hit) and
/// `cs_ambient` (a face cell) share, so the probes and the ambient colour
/// light a room with exactly the same sources. WGSL: `gi_lights_irradiance`
/// plus the sun line in both entries.
pub fn direct_irradiance(room: &RoomBox, direct: &DirectSource, h: Vec3, n: Vec3) -> Vec3 {
    match direct {
        DirectSource::Uniform(e0) => *e0,
        DirectSource::Scene { lights, sun } => {
            let mut e = lights.iter().fold(Vec3::ZERO, |acc, l| acc + light_irradiance(l, &[], h, n));
            if let Some(s) = sun {
                let tr = sun_path(room, h, n, s.dir);
                e += Vec3::from(s.color) * s.intensity * n.dot(s.dir).max(0.0) * tr;
            }
            e
        }
    }
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
    /// The room's probe weight (`GiRoomGpu::bmin.w`): 1 = its probes alone
    /// (every twin room starts so), 0 = its ambient colour alone.
    pub weight: f32,
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
                let r = TwinRoom { room, counts, first_probe: first, direct, visibility: false, weight: 1.0 };
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
        // No slab run: a room box never holds a solid slab (see
        // `light_irradiance`), so the update, like `cs_trace`, tests none.
        let e = direct_irradiance(&r.room, &r.direct, h, n);
        let l = refl / std::f32::consts::PI * e + refl * self.sample(ri, h, n);
        (l, t)
    }

    /// Work out every room's ambient colour (`room_ambient`, no sky: the twin
    /// has no sky table) into the ambient row, as `cs_ambient` and
    /// `cs_ambient_resolve` do each frame.
    pub fn update_ambient(&mut self) {
        for ri in 0..self.rooms.len() {
            let r = &self.rooms[ri];
            let a = room_ambient(&r.room, &r.direct, |_| Vec3::ZERO);
            let idx = self.layout.amb_y0 as usize * self.layout.width as usize + ri;
            self.atlas[idx] = [a.x, a.y, a.z, 1.0];
        }
    }

    /// Room `ri`'s ambient colour as the ambient row holds it.
    pub fn stored_ambient(&self, ri: usize) -> Vec3 {
        let t = self.texel(ri as i32, self.layout.amb_y0 as i32);
        Vec3::new(t[0], t[1], t[2])
    }

    /// SEED room `ri`'s probes with its ambient colour (`cs_seed`): every
    /// irradiance texel the stored ambient colour with `SEED_COUNT` as its
    /// update count, every depth texel the exact distance to the box along
    /// its direction (a box-only room's depth moments, no variance). The
    /// probes then read exactly the ambient colour, and their own updates take
    /// over gradually.
    pub fn seed(&mut self, ri: usize) {
        let amb = self.stored_ambient(ri);
        let r = self.rooms[ri].clone();
        let mut atlas = std::mem::take(&mut self.atlas);
        for local in 0..r.probe_count() {
            let probe = r.first_probe + local;
            let p = probe_position(&r.room, r.counts, probe_grid(r.counts, local));
            let irr = vec![[amb.x, amb.y, amb.z, SEED_COUNT]; (IRR_N * IRR_N) as usize];
            write_tile(&mut atlas, &self.layout, self.layout.irr_origin(probe), IRR_N, &irr);
            let mut dep = vec![[0.0f32; 4]; (DEPTH_N * DEPTH_N) as usize];
            for y in 0..DEPTH_N {
                for x in 0..DEPTH_N {
                    let (t, _) = trace_box(p, texel_direction(DEPTH_N, x, y), r.room.min, r.room.max);
                    dep[(y * DEPTH_N + x) as usize] = [t, t * t, 0.0, 0.0];
                }
            }
            write_tile(&mut atlas, &self.layout, self.layout.depth_origin(probe), DEPTH_N, &dep);
        }
        self.atlas = atlas;
    }

    /// What a fragment of room `ri` at `p` facing `n` reads: its probe weight
    /// of the probes' sample plus the rest of its ambient colour.
    /// `room_gi_irradiance` and `room_gi_ambient` in 85-room-gi.wgsl.
    pub fn fragment_light(&self, ri: usize, p: Vec3, n: Vec3) -> Vec3 {
        let w = self.rooms[ri].weight;
        let probes = if w > 0.0 { self.sample(ri, p, n) * w } else { Vec3::ZERO };
        let ambient = if w < 1.0 { self.stored_ambient(ri) * (1.0 - w) } else { Vec3::ZERO };
        probes + ambient
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
                    e_sum += light_irradiance(&lamp, &[], p, n).x;
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
        let counts = lattice_for(&huge, ProbeBudget::default()).counts;
        let n = |c: [u32; 3]| c[0] * c[1] * c[2];
        assert!(n(counts[0]) <= MAX_PROBES_PER_ROOM, "the hall holds {}", n(counts[0]));
        assert_eq!(counts[1], [11, 4, 11], "the small room keeps 1 m");
        // THE SHIP DOES NOT COARSEN ANY ROOM (probe streaming, 2026-10-21): a
        // ship far over the budget keeps every room at the level's spacing,
        // and `Residency` decides which rooms hold their probes. Seen red
        // first (2026-10-21) with the ship-wide fit of 2026-10-20 still in
        // place: with every room coarsened together to fit the budget (the ship-wide fit before streaming): "room 0 of a ship 1.0 times the budget: [32, 4, 32] against [41, 4, 41]".
        let many: Vec<RoomBox> = (0..8).map(|i| grey_box(Vec3::new(i as f32 * 50.0, 0.0, 0.0), Vec3::new(i as f32 * 50.0 + 40.0, 3.0, 40.0), 0.5)).collect();
        let lat = lattice_for(&many, ProbeBudget::default());
        for (i, r) in many.iter().enumerate() {
            let want = probe_counts(r.size(), PROBE_SPACING_M);
            assert!(n(want) <= MAX_PROBES_PER_ROOM, "the test room fits its own cap");
            assert_eq!(lat.counts[i], want, "room {i} of a ship {:.1} times the budget: {:?} against {want:?}", lat.total() as f64 / MAX_TOTAL_PROBES as f64, lat.counts[i]);
        }
        assert!(lat.total() > MAX_TOTAL_PROBES as u64, "the ship holds more than the budget: {}", lat.total());
        // THE AMBIENT ROW is the atlas's last row, below every tile, and a
        // layout for no probes is that row alone (the Ambient level).
        assert_eq!(l.amb_y0, l.height - 1);
        assert!(l.depth_origin(483).1 + DEPTH_TILE <= l.amb_y0, "the last depth tile ends above the ambient row");
        let none = AtlasLayout::for_probes(0);
        assert_eq!((none.height, none.amb_y0, none.probes), (1, 0, 0), "{none:?}");
    }

    // ── THE ROOM'S AMBIENT COLOUR (2026-10-21) ──

    /// A grey box with a glass lid of transmittance `t` and reflectance
    /// `alpha_rho` on the lid face (what the glass reflects).
    fn glass_box(min: Vec3, max: Vec3, rho: f32, t: f32, lid_rho: f32) -> RoomBox {
        let mut b = grey_box(min, max, rho);
        b.lid_transmittance = t;
        b.refl[FACE_LID] = [lid_rho; 3];
        b
    }

    /// THE BOX'S FORM FACTORS are the catalogue's. A unit cube: 0.19982 to the
    /// opposite face, 0.20004 to each neighbour (Howell, C-11 and C-14), which
    /// sum to 1 before any scaling. Any box: every row sums to 1, and
    /// reciprocity A_i F_ij = A_j F_ji holds (the two formulas are separate,
    /// so this checks each against the other), for a room, a hall, a long
    /// corridor and the 1.1 km street. Seen red first (2026-10-21) with neighbouring faces' extents swapped in box_form_factor: "".
    #[test]
    fn the_box_form_factors_are_the_catalogue_s() {
        assert!((ff_parallel(1.0, 1.0, 1.0) - 0.19982).abs() < 2.0e-5, "{}", ff_parallel(1.0, 1.0, 1.0));
        assert!((ff_perpendicular(1.0, 1.0, 1.0) - 0.20004).abs() < 2.0e-5, "{}", ff_perpendicular(1.0, 1.0, 1.0));
        let raw: f32 = (0..6).map(|j| box_form_factor(Vec3::ONE, FACE_FLOOR, j)).sum();
        assert!((raw - 1.0).abs() < 1.0e-4, "a cube face's raw row sums to {raw}");
        for size in [Vec3::new(10.0, 3.0, 10.0), Vec3::new(30.0, 8.0, 30.0), Vec3::new(2.0, 3.0, 40.0), Vec3::new(10.0, 4.0, 1100.0)] {
            let room = grey_box(Vec3::ZERO, size, 0.5);
            let f = box_form_factors(size);
            for i in 0..6 {
                let raw: f32 = (0..6).map(|j| box_form_factor(size, i, j)).sum();
                assert!((raw - 1.0).abs() < 0.02, "{size}: face {i}'s raw row sums to {raw}");
                assert!((f[i].iter().sum::<f32>() - 1.0).abs() < 1.0e-5);
                for j in 0..6 {
                    let (a, b) = (face_area(&room, i) * f[i][j], face_area(&room, j) * f[j][i]);
                    assert!((a - b).abs() <= 0.02 * a.max(b) + 1.0e-4, "{size}: A{i} F{i}{j} {a} against A{j} F{j}{i} {b}");
                }
            }
        }
    }

    /// THE AMBIENT COLOUR IS THE INTEGRATING-SPHERE VALUE. Uniform direct
    /// light E0 on every face of a closed box at reflectance rho: the light
    /// arriving indirectly is rho E0 / (1 - rho) everywhere, so the ambient
    /// colour (E / pi) is rho E0 / (pi (1 - rho)), the same value the probes
    /// converge to (`closed_box_converges_to_the_integrating_sphere_value`),
    /// and the twin's converged probes agree with it. The grid integral of a
    /// uniform field is exact, so the tolerance is float rounding. Seen red
    /// first (2026-10-21) with one bounce only (the faces' exchange dropped from the solve): "rho 0.3: ambient 0.09549297, integrating sphere 0.13641854".
    #[test]
    fn the_ambient_colour_is_the_integrating_sphere_value() {
        for rho in [0.3f32, 0.5, 0.7] {
            let room = grey_box(Vec3::ZERO, Vec3::new(3.0, 2.0, 3.0), rho);
            let want = rho / (std::f32::consts::PI * (1.0 - rho));
            let got = room_ambient(&room, &DirectSource::Uniform(Vec3::ONE), |_| Vec3::ZERO);
            for c in got.to_array() {
                assert!((c / want - 1.0).abs() < 1.0e-4, "rho {rho}: ambient {c}, integrating sphere {want}");
            }
            let mut twin = Twin::new(vec![(room, DirectSource::Uniform(Vec3::ONE))]);
            twin.hysteresis = 0.8;
            for _ in 0..120 {
                twin.update_all();
            }
            let probes = twin.room_mean(0).x;
            assert!((probes / got.x - 1.0).abs() < 0.10, "rho {rho}: converged probes {probes}, ambient {}", got.x);
        }
    }

    /// THE AMBIENT COLOUR IS WHAT THE PROBES AVERAGE TO in a room lit by its
    /// own sources: a lamp in a 4 m cube, a lamp off-centre in a long room,
    /// and the sun through a glass lid onto a 10 x 3 x 10 m room. Each against
    /// the twin's converged probes' room mean (every probe, every direction).
    /// Seen red first (2026-10-21) with the glass lid's own arrival counted in the room's colour: "sun through glass: ambient [0.16929723, 0.16083233, 0.15236753] against what the surfaces read [0.11875098, 0.11281345, 0.106875874]".
    #[test]
    fn the_ambient_colour_is_the_mean_of_the_converged_probes() {
        let lamp_cube = (
            grey_box(Vec3::ZERO, Vec3::splat(4.0), 0.5),
            DirectSource::Scene { lights: vec![RoomLight::point(Vec3::splat(2.0), [1.0; 3], 8.0, 12.0)], sun: None },
        );
        let long_room = (
            grey_box(Vec3::ZERO, Vec3::new(12.0, 3.0, 4.0), 0.6),
            DirectSource::Scene {
                lights: vec![RoomLight::point(Vec3::new(3.0, 2.7, 2.0), [1.0, 0.9, 0.8], 9.0, 10.0)],
                sun: None,
            },
        );
        let sunlit = (
            glass_box(Vec3::ZERO, Vec3::new(10.0, 3.0, 10.0), 0.55, 0.65, 0.1),
            DirectSource::Scene {
                lights: vec![],
                sun: Some(Sun { dir: Vec3::new(0.2, 1.0, 0.1).normalize(), color: [1.0, 0.95, 0.9], intensity: 3.0 }),
            },
        );
        for (name, (room, direct)) in [("lamp cube", lamp_cube), ("long room", long_room), ("sun through glass", sunlit)] {
            let ambient = room_ambient(&room, &direct, |_| Vec3::ZERO);
            let mut twin = Twin::new(vec![(room, direct)]);
            twin.hysteresis = 0.9;
            for _ in 0..90 {
                twin.update_all();
            }
            let probes = twin.room_mean(0);
            // What the room's opaque surfaces read from the probes, area-
            // weighted (a glass lid's fragments keep the old floor, so it is
            // left out): the light the ambient colour stands in for.
            let r = &twin.rooms[0].room;
            let (mut surf, mut area) = (Vec3::ZERO, 0.0f32);
            for f in (0..6).filter(|&f| f != FACE_LID || r.lid_transmittance <= 0.0) {
                let a = face_area(r, f) / AMBIENT_CELLS as f32;
                for c in 0..AMBIENT_CELLS {
                    surf += twin.sample(0, face_cell_point(r, f, c), face_normal(f)) * a;
                    area += a;
                }
            }
            let surf = surf / area;
            println!(
                "{name}: ambient {ambient}, probe mean {probes} (ratio {:.3}), opaque surfaces read {surf} (ratio {:.3})",
                ambient.x / probes.x,
                ambient.x / surf.x
            );
            for (a, s) in ambient.to_array().iter().zip(surf.to_array()) {
                assert!(*a > 0.0, "{name}: a lit room has a colour");
                assert!((a / s - 1.0).abs() < 0.15, "{name}: ambient {ambient} against what the surfaces read {surf}");
            }
        }
    }

    /// IT FOLLOWS THE LAMPS AND KEEPS ROOMS APART. Two rooms share a wall and
    /// the lamp is in room A: A's ambient colour is lit, B's is exactly zero
    /// (each room takes only its own lights, `assign_lights`), and with A's
    /// lamp switched off A's is exactly zero too, that same evaluation (no
    /// fade: the colour is worked out afresh every frame). Seen red first
    /// (2026-10-21) with a constant glow added (the brighter texture the level stands in for): "room B holds no light at all".
    #[test]
    fn a_lamp_switched_off_darkens_its_room_s_ambient_and_no_other() {
        let a = grey_box(Vec3::ZERO, Vec3::new(3.0, 2.5, 3.0), 0.6);
        let b = grey_box(Vec3::new(3.1, 0.0, 0.0), Vec3::new(6.1, 2.5, 3.0), 0.6);
        let lights = vec![RoomLight::point(Vec3::new(1.5, 2.4, 1.5), [1.0; 3], 10.0, 14.0)];
        let rooms = vec![a.clone(), b.clone()];
        let owned = assign_lights(&rooms, &lights);
        let scene = |i: usize| DirectSource::Scene { lights: owned[i].iter().map(|&k| lights[k]).collect(), sun: None };
        let none = |_: Vec3| Vec3::ZERO;
        let lit = room_ambient(&a, &scene(0), none);
        assert!(lit.min_element() > 0.01, "room A is lit: {lit}");
        assert_eq!(room_ambient(&b, &scene(1), none), Vec3::ZERO, "room B holds no light at all");
        let off = DirectSource::Scene { lights: vec![], sun: None };
        assert_eq!(room_ambient(&a, &off, none), Vec3::ZERO, "the lamp off: room A is dark at once");
    }

    /// THE SKY THROUGH A GLASS LID is in the ambient colour, and only through
    /// glass. A black cube (nothing reflects) under a glass lid passing T of a
    /// sky of constant radiance L: the lid's exitance into the room is
    /// T pi L, the floor takes the opposite face's share of it (C-11, 0.19982
    /// for a cube) and each wall its neighbour's (C-14, 0.20004), so the mean
    /// over the five opaque faces is T L (0.19982 + 4 x 0.20004) / 5, E / pi.
    /// An opaque lid lets none in. Seen red first (2026-10-21) with the sky through the lid without the lid's transmittance: "".
    #[test]
    fn the_sky_through_a_glass_lid_is_in_the_ambient_colour() {
        let room = glass_box(Vec3::ZERO, Vec3::splat(3.0), 0.0, 0.65, 0.2);
        let sky = Vec3::new(0.3, 0.4, 0.6);
        let dark = DirectSource::Scene { lights: vec![], sun: None };
        let got = room_ambient(&room, &dark, |_| sky);
        let want = sky * 0.65 * (0.19982 + 4.0 * 0.20004) / 5.0;
        assert!((got - want).abs().max_element() < 1.0e-4, "ambient {got}, want {want}");
        // The cosine-weighted directions average a sky that brightens toward
        // the zenith as the cosine-weighted integral does: L(d) = d.y gives
        // (1 / pi) * integral of cos^2 = 2 / 3.
        let mean = (0..AMBIENT_SKY_DIRS).map(|i| sky_dir(i).y).sum::<f32>() / AMBIENT_SKY_DIRS as f32;
        assert!((mean - 2.0 / 3.0).abs() < 0.01, "cosine-weighted mean of cos theta {mean}");
        let shut = RoomBox { lid_transmittance: 0.0, ..room };
        assert_eq!(room_ambient(&shut, &dark, |_| sky), Vec3::ZERO, "an opaque lid lets no sky in");
    }

    /// A SEEDED ROOM READS ITS AMBIENT COLOUR, THEN CONVERGES: the no-pop rule
    /// of probe streaming. Before it comes in (weight 0) a fragment reads the
    /// room's ambient colour; the frame it comes in (seeded, weight 1) it reads
    /// the same colour to float rounding, everywhere in the room; its first
    /// update moves it only part of the way to the probes' own answer (the
    /// seed weighs as `SEED_COUNT` updates); and it ends where a room that was
    /// never seeded ends. Seen red first (2026-10-21) with seed count 0 (the first update takes its estimate whole): "the first update after the seed moved 0.74 of the way at once".
    #[test]
    fn a_seeded_room_starts_at_its_ambient_colour_and_converges() {
        let room = grey_box(Vec3::ZERO, Vec3::new(6.0, 3.0, 5.0), 0.55);
        let lamp = RoomLight::point(Vec3::new(1.5, 2.7, 1.5), [1.0, 0.9, 0.8], 9.0, 10.0);
        let direct = DirectSource::Scene { lights: vec![lamp], sun: None };
        let mut twin = Twin::new(vec![(room.clone(), direct.clone())]);
        twin.update_ambient();
        let ambient = twin.stored_ambient(0);
        assert!(ambient.min_element() > 0.0);
        let points: Vec<(Vec3, Vec3)> = (0..64)
            .map(|i| {
                let f = (i % 6) as usize;
                (face_cell_point(&room, f, (i * 7) as u32 % AMBIENT_CELLS), face_normal(f))
            })
            .collect();
        twin.rooms[0].weight = 0.0;
        let before: Vec<Vec3> = points.iter().map(|(p, n)| twin.fragment_light(0, *p, *n)).collect();
        twin.seed(0);
        twin.rooms[0].weight = 1.0;
        let mut worst = 0.0f32;
        for ((p, n), b) in points.iter().zip(&before) {
            assert_eq!(*b, ambient, "out of the atlas a fragment reads the ambient colour");
            let now = twin.fragment_light(0, *p, *n);
            worst = worst.max(((now - *b).abs() / b.max(Vec3::splat(1e-6))).max_element());
        }
        assert!(worst < 1.0e-5, "the frame it comes in, a fragment moved by {:.2e} of its light", worst);
        // Converge an unseeded twin for the answer.
        let mut cold = Twin::new(vec![(room, direct)]);
        cold.hysteresis = 0.9;
        twin.hysteresis = 0.9;
        for _ in 0..120 {
            cold.update_all();
        }
        let target: Vec<Vec3> = points.iter().map(|(p, n)| cold.fragment_light(0, *p, *n)).collect();
        twin.update_all();
        let mut moved = 0.0f32;
        let mut gap = 0.0f32;
        for (((p, n), b), t) in points.iter().zip(&before).zip(&target) {
            moved += (twin.fragment_light(0, *p, *n) - *b).abs().x;
            gap += (*t - *b).abs().x;
        }
        println!("first update after the seed moved {:.3} of the way", moved / gap);
        assert!(moved / gap < 0.35, "the first update after the seed moved {:.2} of the way at once", moved / gap);
        for _ in 0..120 {
            twin.update_all();
        }
        for ((p, n), t) in points.iter().zip(&target) {
            let got = twin.fragment_light(0, *p, *n);
            assert!(((got - *t).abs() / t.max(Vec3::splat(1e-4))).max_element() < 0.03, "at {p}: seeded {got}, cold {t}");
        }
    }

    // ── PROBE STREAMING (2026-10-21) ──

    /// A ship in a line along +x: six 10 x 3 x 10 m rooms of a home, a 30 x 8
    /// x 30 m commons hall with a 6 x 3 x 6 m room inside it, and a 300 x 4
    /// x 6 m street, with their probe counts at 1 m and a budget of
    /// `LINE_BUDGET` (each room's own cap a quarter of it).
    const LINE_BUDGET: u32 = 1936;
    fn line_ship() -> (Vec<RoomBox>, Vec<u32>) {
        let mut rooms: Vec<RoomBox> = (0..6)
            .map(|i| {
                let x = i as f32 * 12.0;
                grey_box(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 10.0, 3.0, 10.0), 0.5)
            })
            .collect();
        rooms.push(grey_box(Vec3::new(90.0, 0.0, 0.0), Vec3::new(120.0, 8.0, 30.0), 0.5));
        rooms.push(grey_box(Vec3::new(100.0, 0.0, 10.0), Vec3::new(106.0, 3.0, 16.0), 0.5));
        rooms.push(grey_box(Vec3::new(130.0, 0.0, 0.0), Vec3::new(430.0, 4.0, 6.0), 0.5));
        let lat = lattice_for(&rooms, ProbeBudget { spacing_m: 1.0, max_probes: LINE_BUDGET });
        let probes = (0..rooms.len()).map(|i| lat.probes_in(i)).collect();
        (rooms, probes)
    }

    /// The atlas as residency has it: every resident room inside `0..cap`, no
    /// two overlapping, and the total within the cap.
    fn check_atlas(res: &Residency, probes: &[u32], what: &str) {
        let mut held: Vec<(u32, u32, usize)> =
            res.slots.iter().enumerate().filter_map(|(i, s)| s.map(|r| (r.first, r.count, i))).collect();
        held.sort();
        for w in held.windows(2) {
            assert!(w[0].0 + w[0].1 <= w[1].0, "{what}: rooms {} and {} overlap in the atlas: {held:?}", w[0].2, w[1].2);
        }
        for (first, count, i) in &held {
            assert_eq!(*count, probes[*i], "{what}: room {i} holds its own probe count");
            assert!(first + count <= res.cap, "{what}: room {i} runs past the atlas");
        }
        assert!(res.resident_probes() <= res.cap, "{what}: {} probes held, budget {}", res.resident_probes(), res.cap);
    }

    /// ROOMS NEAR THE PLAYER HOLD PROBES, WITHIN THE BUDGET. On a ship whose
    /// rooms hold twice the budget: standing in the first room of the
    /// home, that room and its neighbours hold probes and the street does not;
    /// walking to the commons, the commons comes in and the far end of the
    /// home goes out, the room the player stands in is in within a fade of
    /// arriving, and at no frame is the budget passed or two rooms given the
    /// same probes; walking back, the home returns. Seen red first
    /// (2026-10-21) with the wanted set ignoring the budget: "the commons hall came in 4294967295 frames after the player".
    #[test]
    fn rooms_near_the_player_hold_probes_within_the_budget() {
        let (rooms, probes) = line_ship();
        let cap = LINE_BUDGET;
        let total: u32 = probes.iter().sum();
        println!("probes per room {probes:?}, {total} in all against a budget of {cap}");
        assert!(total > cap * 2, "the ship must not fit at once");
        let mut res = Residency::new(cap, rooms.len());
        let walk = |res: &mut Residency, eye: Vec3, frames: u32, what: &str| -> u32 {
            let mut in_at = None;
            for f in 0..frames {
                res.step(&rooms, &probes, eye);
                check_atlas(res, &probes, what);
                let here = (0..rooms.len()).filter(|&i| rooms[i].contains(eye, 0.0)).min_by(|&a, &b| rooms[a].volume().total_cmp(&rooms[b].volume()));
                if in_at.is_none() && here.is_some_and(|h| res.placement(h).weight >= 1.0) {
                    in_at = Some(f);
                }
            }
            in_at.unwrap_or(u32::MAX)
        };
        let home = Vec3::new(5.0, 1.7, 5.0);
        let at = walk(&mut res, home, 40, "at home");
        assert_eq!(at, 0, "the player's own room comes in the first frame");
        assert!(res.placement(1).weight > 0.0, "the next room of the home holds probes");
        assert_eq!(res.placement(8).weight, 0.0, "the street does not");
        let commons = Vec3::new(112.0, 1.7, 25.0);
        let at = walk(&mut res, commons, 60, "in the commons");
        assert!(at <= LEAVE_FADE_FRAMES + 2, "the commons hall came in {at} frames after the player");
        assert!(res.placement(6).weight >= 1.0, "the hall holds probes");
        assert_eq!(res.placement(0).weight, 0.0, "the far end of the home has gone out");
        let at = walk(&mut res, home, 60, "back home");
        assert!(at <= LEAVE_FADE_FRAMES + 2, "home came back in {at} frames");
        assert!(res.placement(0).weight >= 1.0 && res.placement(1).weight >= 1.0);
    }

    /// A ROOM GOING OUT FADES TO ITS AMBIENT COLOUR: its weight falls by
    /// 1 / LEAVE_FADE_FRAMES a frame, so a fragment there moves at most that
    /// share of the way from its probes to its ambient colour each frame, and
    /// its probes are given away only once the weight reaches 0. Seen red
    /// first (2026-10-21) with a leaving room dropped to its ambient colour at once (no fade): "frame 1: moved 0.021191524, a fade step is 0.0010595762".
    #[test]
    fn a_room_going_out_fades_to_its_ambient_colour() {
        let a = grey_box(Vec3::ZERO, Vec3::new(10.0, 3.0, 10.0), 0.5);
        let b = grey_box(Vec3::new(40.0, 0.0, 0.0), Vec3::new(50.0, 3.0, 10.0), 0.5);
        let probes = vec![484, 484];
        let rooms = vec![a.clone(), b];
        // Room for one room only.
        let mut res = Residency::new(600, 2);
        res.step(&rooms, &probes, Vec3::new(5.0, 1.7, 5.0));
        assert!(res.placement(0).weight >= 1.0 && res.slots[1].is_none());
        // The twin's fragment light in room A at full weight and at none.
        let lamp = RoomLight::point(Vec3::new(2.0, 2.7, 2.0), [1.0; 3], 9.0, 10.0);
        let mut twin = Twin::new(vec![(a, DirectSource::Scene { lights: vec![lamp], sun: None })]);
        twin.hysteresis = 0.9;
        for _ in 0..60 {
            twin.update_all();
        }
        twin.update_ambient();
        let p = (Vec3::new(9.9, 1.0, 5.0), Vec3::NEG_X);
        let full = twin.fragment_light(0, p.0, p.1);
        let gap = (full - twin.stored_ambient(0)).abs().max_element();
        assert!(gap > 1.0e-3, "the probes and the ambient colour differ here");
        let eye_b = Vec3::new(45.0, 1.7, 5.0);
        let mut last = full;
        let mut frames = 0;
        loop {
            res.step(&rooms, &probes, eye_b);
            frames += 1;
            twin.rooms[0].weight = res.placement(0).weight;
            let now = twin.fragment_light(0, p.0, p.1);
            let step = (now - last).abs().max_element();
            assert!(step <= gap / LEAVE_FADE_FRAMES as f32 + 1.0e-5, "frame {frames}: moved {step}, a fade step is {}", gap / LEAVE_FADE_FRAMES as f32);
            last = now;
            if res.slots[0].is_none() {
                break;
            }
            assert!(res.slots[1].is_none(), "frame {frames}: room B took room A's probes before A had faded");
            assert!(frames <= LEAVE_FADE_FRAMES + 1, "room A never finished leaving");
        }
        assert_eq!(frames, LEAVE_FADE_FRAMES + 1, "a frame to see room A is no longer wanted, then the fade");
        assert!((last - twin.stored_ambient(0)).abs().max_element() < 1.0e-6, "faded out, room A shows its ambient colour");
        assert!(res.placement(1).weight >= 1.0, "room B comes in the frame room A's probes are freed");
    }

    /// NO SWAPPING AT A BOUNDARY: two equal rooms, room for one, the player
    /// standing between them and swaying half a metre either way. The room
    /// that came in first stays; nothing comes in or goes out again. Seen red
    /// first (2026-10-21) with no residency hysteresis: "frame 0: the boundary swapped rooms: ResidencyStep { entered: [], started_leaving: [0], left: [], waiting: [1], moved: [] }".
    #[test]
    fn residency_does_not_swap_rooms_at_a_boundary() {
        let a = grey_box(Vec3::ZERO, Vec3::new(10.0, 3.0, 10.0), 0.5);
        let b = grey_box(Vec3::new(12.0, 0.0, 0.0), Vec3::new(22.0, 3.0, 10.0), 0.5);
        let rooms = vec![a, b];
        let probes = vec![484, 484];
        let mut res = Residency::new(600, 2);
        let mid = Vec3::new(11.0, 1.7, 5.0);
        res.step(&rooms, &probes, mid - Vec3::X * 0.5);
        for _ in 0..LEAVE_FADE_FRAMES + 2 {
            res.step(&rooms, &probes, mid - Vec3::X * 0.5);
        }
        let first = res.slots[0].is_some();
        for f in 0..200 {
            let eye = mid + Vec3::X * if f % 2 == 0 { 0.5 } else { -0.5 };
            let step = res.step(&rooms, &probes, eye);
            assert!(!step.changed(), "frame {f}: the boundary swapped rooms: {step:?}");
        }
        assert_eq!(res.slots[0].is_some(), first);
    }

    /// A ROOM ON ANOTHER STOREY COUNTS FARTHER: the room under the player's
    /// floor is 0.25 m away by its box but a floor stands between them.
    #[test]
    fn a_room_on_another_storey_counts_farther() {
        let below = grey_box(Vec3::ZERO, Vec3::new(10.0, 2.75, 10.0), 0.5);
        let beside = grey_box(Vec3::new(10.5, 3.0, 0.0), Vec3::new(20.0, 6.0, 10.0), 0.5);
        let eye = Vec3::new(5.0, 4.7, 5.0);
        assert!((room_priority(&below, eye) - (1.95 + STOREY_PENALTY_M)).abs() < 1e-4);
        assert!(room_priority(&beside, eye) < room_priority(&below, eye), "the room beside on this storey comes first");
        assert_eq!(room_priority(&below, Vec3::new(5.0, 1.7, 5.0)), 0.0, "standing in it");
    }

    /// A FRAGMENTED ATLAS IS PACKED AND TAKES THE ROOM IT NEEDS. Four small
    /// rooms in a row leave gaps no bigger than one of them, and a big room
    /// nearer than all of them comes into view: it fits the free probes but no
    /// gap. The same frame, every small room's tiles slide down (moves listed
    /// lowest first, each one down, none overlapping a later room's old place)
    /// and the big room comes in; no room is given up, each keeps its own
    /// probe count. Then a wanted room that would fit only once a fading room
    /// is gone waits rather than packing over it. Seen red first (2026-10-21)
    /// with no packing of a fragmented atlas: "the big room comes in the frame it is wanted: ResidencyStep { entered: [], started_leaving: [], left: [], waiting: [4], moved: [] }".
    #[test]
    fn a_fragmented_atlas_is_packed_and_takes_the_room_it_needs() {
        // Small rooms of 100 probes at 0, 200, 400, 600 in an atlas of 800:
        // 400 free in four gaps of 100.
        let small = |x: f32| grey_box(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 4.0, 3.0, 4.0), 0.5);
        let mut rooms: Vec<RoomBox> = (0..4).map(|k| small(k as f32 * 6.0)).collect();
        rooms.push(grey_box(Vec3::new(-20.0, 0.0, 0.0), Vec3::new(-10.0, 3.0, 10.0), 0.5));
        let probes = vec![100, 100, 100, 100, 200];
        let mut res = Residency::new(800, 5);
        for k in 0..4 {
            res.slots[k] = Some(Resident { first: k as u32 * 200, count: 100, weight: 1.0, leaving: false });
        }
        assert_eq!(res.free_ranges().iter().map(|g| g.1).max(), Some(100));
        let eye = Vec3::new(-8.0, 1.7, 2.0);
        let step = res.step(&rooms, &probes, eye);
        check_atlas(&res, &probes, "packed");
        assert_eq!(step.entered, vec![4], "the big room comes in the frame it is wanted: {step:?}");
        assert!(step.started_leaving.is_empty(), "no room gives way: {step:?}");
        assert_eq!(
            step.moved.iter().map(|m| (m.room, m.from, m.to)).collect::<Vec<_>>(),
            vec![(1, 200, 100), (2, 400, 200), (3, 600, 300)],
            "the small rooms slide down in atlas order"
        );
        for (k, m) in step.moved.iter().enumerate() {
            assert!(m.to < m.from && m.count == 100, "{m:?}");
            for later in &step.moved[k + 1..] {
                assert!(m.to + m.count <= later.from, "{m:?} lands on {later:?}'s old place before it is copied");
            }
        }
        assert_eq!(res.placement(4).first_probe, 400);
        // A room fading out holds its probes until it is gone: a newcomer that
        // needs them waits, and nothing is packed over a fading room's place.
        let mut res = Residency::new(300, 2);
        let a = grey_box(Vec3::ZERO, Vec3::new(4.0, 3.0, 4.0), 0.5);
        let b = grey_box(Vec3::new(40.0, 0.0, 0.0), Vec3::new(44.0, 3.0, 4.0), 0.5);
        let two = vec![a, b];
        let sizes = vec![200, 200];
        res.step(&two, &sizes, Vec3::new(2.0, 1.7, 2.0));
        let step = res.step(&two, &sizes, Vec3::new(42.0, 1.7, 2.0));
        assert_eq!((step.started_leaving.clone(), step.waiting.clone(), step.moved.len()), (vec![0], vec![1], 0), "{step:?}");
    }


    /// A RELAYOUT KEEPS THE PROBES OF ROOMS THAT DID NOT CHANGE: the editor
    /// moves one wall, one room's box changes, the others keep their place
    /// (and their converged light); the changed room comes back in seeded.
    #[test]
    fn a_relayout_keeps_the_probes_of_rooms_that_did_not_change() {
        let (rooms, probes) = line_ship();
        let counts = lattice_for(&rooms, ProbeBudget { spacing_m: 1.0, max_probes: LINE_BUDGET }).counts;
        let mut res = Residency::new(LINE_BUDGET, rooms.len());
        let eye = Vec3::new(5.0, 1.7, 5.0);
        for _ in 0..3 {
            res.step(&rooms, &probes, eye);
        }
        let mut moved = rooms.clone();
        moved[1].max.x += 0.5;
        // The editor's rebuild also reorders the rooms.
        moved.swap(0, 2);
        let new_counts = lattice_for(&moved, ProbeBudget { spacing_m: 1.0, max_probes: LINE_BUDGET }).counts;
        let kept = res.carried_over((&rooms, &counts), (&moved, &new_counts));
        assert_eq!(kept.slots[2], res.slots[0], "room 0 (now 2) keeps its probes");
        assert_eq!(kept.slots[0], res.slots[2], "room 2 (now 0) keeps its probes");
        assert!(kept.slots[1].is_none(), "the changed room starts over");
        assert_eq!(kept.slots[3], res.slots[3]);
    }

    /// The GPU records are the sizes the WGSL structs have, which naga checks
    /// against the shader text in `room_probes_gpu`'s tests.
    #[test]
    fn gpu_record_sizes() {
        assert_eq!(HEADER_BYTES, 64);
        assert_eq!(ROOM_BYTES, 160);
        assert_eq!(std::mem::size_of::<GiParamsGpu>(), 128);
    }

    // ── FLOORS BETWEEN STOREYS (ship homes increment 7, lane L) ──

    /// A slab of the homestead's upper floor: top 3.0, 0.25 m thick.
    fn floor_slab(x0: f32, z0: f32, x1: f32, z1: f32) -> OccluderSlab {
        OccluderSlab { min: Vec3::new(x0, 2.75, z0), max: Vec3::new(x1, 3.0, z1) }
    }

    /// The upper bedroom's warm lamp, 0.4 m under the 6 m roof, over a
    /// 10 x 10 m floor at 3.0.
    fn upstairs_lamp() -> RoomLight {
        RoomLight::point(Vec3::new(5.0, 5.6, 5.0), [1.0, 0.85, 0.6], 4.0, 7.0)
    }

    /// A LAMP ABOVE A SOLID FLOOR DOES NOT REACH THE ROOM UNDER IT. The lamp
    /// lights the dining room's floor 5.1 m below it with no slab between (the
    /// light loop as it was before storeys), and delivers exactly nothing once
    /// the slab is in its run. The fragment faces the lamp (n.l = 1), so the
    /// zero is the floor's doing and nothing else's; and the slab's own top,
    /// which the lamp faces, stays lit. Seen red first (2026-10-20) with the
    /// slab test switched off in `light_irradiance` (`if false && ...`): "a
    /// solid floor between them: got [0.040196758, 0.034167245, 0.024118057]".
    #[test]
    fn a_lamp_above_a_solid_floor_does_not_reach_the_room_under_it() {
        let lamp = upstairs_lamp();
        let slab = floor_slab(0.0, 0.0, 10.0, 10.0);
        let below = Vec3::new(5.0, 0.5, 5.0);
        let open = light_irradiance(&lamp, &[], below, Vec3::Y);
        assert!(open.x > 0.01, "with no floor the lamp lights the room below: {open}");
        assert_eq!(slabs_touched(&lamp, &[slab]), vec![0], "the lamp's range reaches the floor");
        let got = light_irradiance(&lamp, &[slab], below, Vec3::Y);
        assert_eq!(got, Vec3::ZERO, "a solid floor between them: got {got}");
        // The floor's own top, under the lamp: lit as before.
        let top = Vec3::new(5.0, 3.0, 5.0);
        assert_eq!(light_irradiance(&lamp, &[slab], top, Vec3::Y), light_irradiance(&lamp, &[], top, Vec3::Y));
        // And a lamp downstairs does not light the room above: the upstairs
        // wall a metre over the floor, facing down toward it.
        let dining = RoomLight::point(Vec3::new(5.0, 2.6, 5.0), [1.0, 0.85, 0.6], 4.0, 4.0);
        let up_wall = Vec3::new(6.0, 4.0, 5.0);
        assert!(light_irradiance(&dining, &[], up_wall, Vec3::NEG_Y).x > 0.0);
        assert_eq!(light_irradiance(&dining, &[slab], up_wall, Vec3::NEG_Y), Vec3::ZERO);
    }

    /// THROUGH THE STAIRWELL IT DOES. The upper floor as two slabs with the
    /// stairwell between them (x 4 to 6, the whole depth): a lamp over the gap
    /// lights the stair below it exactly as it would with no floor at all,
    /// while a point under either slab is dark. And a fragment on the slab's
    /// own edge face, facing into the stairwell, is not shadowed by its own
    /// slab (the crossing lands on the rectangle's border, inside the 1 cm
    /// tolerance). Seen red first (2026-10-20) with the rectangle test
    /// answering yes to any crossing of the mid-plane (which closes the gap):
    /// "the lamp over the stairwell lights the stair: got [0, 0, 0], without a
    /// floor [0.093045644, 0.0790888, 0.05582739]".
    #[test]
    fn a_lamp_over_the_stairwell_lights_the_stair_below() {
        let west = floor_slab(0.0, 0.0, 4.0, 10.0);
        let east = floor_slab(6.0, 0.0, 10.0, 10.0);
        let slabs = [west, east];
        let lamp = RoomLight::point(Vec3::new(5.0, 5.6, 5.0), [1.0, 0.85, 0.6], 4.0, 7.0);
        let run: Vec<OccluderSlab> = slabs_touched(&lamp, &slabs).iter().map(|&i| slabs[i]).collect();
        assert_eq!(run.len(), 2, "the lamp's range reaches both slabs");
        let stair = Vec3::new(5.0, 1.5, 5.0);
        let open = light_irradiance(&lamp, &[], stair, Vec3::Y);
        let got = light_irradiance(&lamp, &run, stair, Vec3::Y);
        assert!(open.x > 0.01);
        assert_eq!(got, open, "the lamp over the stairwell lights the stair: got {got}, without a floor {open}");
        // Under either slab: dark.
        for p in [Vec3::new(2.0, 0.5, 5.0), Vec3::new(8.0, 0.5, 5.0)] {
            assert_eq!(light_irradiance(&lamp, &run, p, Vec3::Y), Vec3::ZERO, "under a slab at {p}");
        }
        // The west slab's edge face at x = 4, mid-thickness and a little below
        // the mid-plane, facing +x into the stairwell, lit from above it: the
        // crossing is at x = 4.0 plus rounding, on the border.
        let edge = Vec3::new(4.0, 2.8, 5.0);
        assert_eq!(light_irradiance(&lamp, &run, edge, Vec3::X), light_irradiance(&lamp, &[], edge, Vec3::X));
    }

    /// THE SUN UNDER A SOLID FLOOR IS BLOCKED, and where no slab is over a
    /// point (the stairwell, the greenhouse, outside the rooms' box) it is
    /// not. A high sun from the south-east; the room under the floor is dark,
    /// the stair under the opening is lit, and so is the floor's own top. A
    /// LOW sun from the stairwell's side reaches under the slab's edge for
    /// exactly as far as the geometry says. Seen red first (2026-10-20) with
    /// `sun_blocked_by_slabs` never blocking (`false && ...`): it panicked
    /// with "the room under the floor is in the floor's shadow".
    #[test]
    fn the_sun_under_a_solid_floor_is_blocked() {
        let west = floor_slab(0.0, 0.0, 4.0, 10.0);
        let east = floor_slab(6.0, 0.0, 10.0, 10.0);
        let slabs = [west, east];
        let rooms_box = (Vec3::ZERO, Vec3::new(10.0, 6.0, 10.0));
        let high = Vec3::new(0.1, 1.0, 0.05);
        assert!(sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(2.0, 0.0, 5.0), high), "the room under the floor is in the floor's shadow");
        assert!(!sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(5.0, 0.0, 5.0), high), "the stair under the opening is in the sun");
        assert!(!sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(2.0, 3.0, 5.0), high), "the floor's top is in the sun");
        // A sun 45 degrees up from +x: from the floor at x = 3.5 the path to it
        // rises 2.875 m to the mid-plane in 2.875 m of x, crossing at 6.375,
        // under the east slab: blocked. From x = 2.0 it crosses at 4.875, in
        // the opening: lit.
        let low = Vec3::new(1.0, 1.0, 0.0);
        assert!(sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(3.5, 0.0, 5.0), low));
        assert!(!sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(2.0, 0.0, 5.0), low));
        // Outside the rooms' box (grown by SLAB_SUN_REACH_M) nothing is tested.
        assert!(!sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(2.0, -5.0, 5.0), high));
        // No slabs, no shadow.
        assert!(!sun_blocked_by_slabs(&[], rooms_box, Vec3::new(2.0, 0.0, 5.0), high));
    }

    /// A ONE-STOREY HOME IS EXACTLY AS BEFORE. With no slab in a light's run
    /// `light_irradiance` is the pre-storey light loop, and a slab the path
    /// does not cross changes nothing, bit for bit, at every point of a room;
    /// a light with no run packs exactly the record it always did (cone_inner
    /// = [cos_inner, 0, 0, 0], which the shader's `cone_inner.z > 0.5` guard
    /// skips); and a run packs as whole numbers the shader reads back with
    /// `u32()`. The other half, the shader taking the same path with no slab
    /// rows, is pinned by `pipeline::permutation_tests::
    /// the_floor_slab_tests_are_guarded_by_their_switch` (both guards test a
    /// count first). Seen red first (2026-10-20) with the run packed in
    /// `cone_inner.zw` instead of `yz`: "the run rides in cone_inner.yz,
    /// left: (0, 5), right: (5, 7)" (the no-run record still read as before,
    /// which is why the run's own read-back is the check).
    #[test]
    fn a_home_with_no_floor_slabs_is_lit_exactly_as_before() {
        let lamp = RoomLight::point(Vec3::new(1.5, 2.6, 1.5), [1.0, 0.9, 0.8], 6.0, 8.0);
        let spot = RoomLight::spot(Vec3::new(2.0, 2.7, 2.0), [1.0, 1.0, 1.0], 12.0, 10.0, Vec3::NEG_Y, 18.0, 28.0);
        let strip = RoomLight::line(Vec3::new(0.5, 2.6, 0.5), Vec3::new(3.5, 2.6, 0.5), [0.9, 0.95, 1.0], 6.0, 5.0);
        // A slab off to one side, never between: 20 m east.
        let aside = floor_slab(20.0, 0.0, 30.0, 10.0);
        for l in [lamp, spot, strip] {
            for i in 0..64 {
                let p = Vec3::new((i % 4) as f32, (i / 16) as f32 * 0.8, ((i / 4) % 4) as f32);
                let n = fibonacci_ray(i, 64);
                assert_eq!(light_irradiance(&l, &[aside], p, n), light_irradiance(&l, &[], p, n), "at {p}");
            }
            let packed = super::super::light::gpu_packed(&l);
            assert_eq!(&packed[12..], &[l.cos_inner, 0.0, 0.0, 0.0], "a light with no run packs as before");
        }
        let run = super::super::light::gpu_packed_with_slabs(&lamp, 5, 7, 2.875);
        assert_eq!((run[13] as u32, run[14] as u32), (5, 7), "the run rides in cone_inner.yz");
        assert_eq!(run[15], 2.875, "and its one layer's plane in cone_inner.w");
        assert_eq!(run[12], lamp.cos_inner);
    }

    /// WHICH SLABS A LIGHT'S RUN HOLDS: every slab its range reaches, nearest
    /// first, and none it cannot reach (its range window is 0 there). A line
    /// light's segment box grown by its range. Seen red first (2026-10-20)
    /// with the range test switched off in `slabs_touched` (every slab
    /// listed): "left: [0, 1, 2, 3], right: [0, 1]".
    #[test]
    fn a_light_lists_the_slabs_its_range_reaches_nearest_first() {
        let slabs = [
            floor_slab(0.0, 0.0, 10.0, 10.0),  // under the lamp: 2.6 m
            floor_slab(10.0, 0.0, 20.0, 10.0), // 5.6 m from it
            floor_slab(12.0, 0.0, 22.0, 10.0), // 7.5 m: out of its 7 m range
            floor_slab(40.0, 0.0, 50.0, 10.0), // far
        ];
        let lamp = upstairs_lamp();
        assert_eq!(slabs_touched(&lamp, &slabs), vec![0, 1]);
        // A strip from x 16 to 44: its box grown by its 5 m range reaches x 11
        // to 49, so slabs 1 to 3 and not slab 0.
        let strip = RoomLight::line(Vec3::new(16.0, 5.6, 5.0), Vec3::new(44.0, 5.6, 5.0), [1.0; 3], 6.0, 5.0);
        let got = slabs_touched(&strip, &slabs);
        assert!(got.contains(&1) && got.contains(&2) && got.contains(&3) && !got.contains(&0), "{got:?}");
    }

    /// THE MERGE IS EXACT. The homestead's upper floor (docs/design/homestead-
    /// upper-floor.md section 1, its 16 solid slabs) merges to 13: the plant
    /// room, forge and workshop roofs become one, the mushroom and aquaponics
    /// roofs another; nothing else shares a whole edge. The merged boxes cover
    /// exactly the same floor (the same area, every merged box a union of
    /// originals). Seen red first (2026-10-20) with the edge test accepting a
    /// partial shared edge (`x_touch` alone, without `same_z`): 8 boxes, the
    /// first of them (0, 2.75, 0) to (55, 3, 51), covering the stairwell, the
    /// shaft, the hatch and two glass roofs ("left: 8, right: 13").
    #[test]
    fn merging_slabs_covers_exactly_the_same_floor() {
        let solid = [
            (20.0, 0.0, 55.0, 22.0),
            (0.0, 22.0, 30.1, 30.0),
            (30.1, 22.0, 34.0, 25.1),
            (31.9, 25.1, 34.0, 26.9),
            (30.1, 26.9, 32.5, 30.0),
            (32.5, 28.1, 34.0, 30.0),
            (0.0, 30.0, 12.0, 51.0),
            (12.0, 30.0, 20.0, 51.0),
            (20.0, 30.0, 34.0, 51.0),
            (34.0, 30.0, 55.0, 38.0),
            (34.0, 38.0, 36.5, 39.3),
            (40.5, 38.0, 55.0, 39.3),
            (34.0, 39.3, 55.0, 51.0),
            (0.0, 51.0, 10.0, 61.0),
            (0.0, 61.0, 10.0, 73.0),
            (40.0, 73.0, 55.0, 89.0),
        ];
        let slabs: Vec<OccluderSlab> = solid.iter().map(|&(a, b, c, d)| floor_slab(a, b, c, d)).collect();
        let area = |v: &[OccluderSlab]| v.iter().map(|s| (s.max.x - s.min.x) * (s.max.z - s.min.z)).sum::<f32>();
        let merged = merge_slabs(slabs.clone());
        assert_eq!(merged.len(), 13, "{merged:?}");
        assert!((area(&merged) - area(&slabs)).abs() < 0.01, "merged {} m2, the floor {} m2", area(&merged), area(&slabs));
        assert!(merged.contains(&floor_slab(0.0, 30.0, 34.0, 51.0)), "plant room + forge + workshop");
        assert!(merged.contains(&floor_slab(0.0, 51.0, 10.0, 73.0)), "mushroom room + aquaponics");
        // Every original lies inside exactly one merged box.
        for s in &slabs {
            let holders = merged
                .iter()
                .filter(|m| m.min.cmple(s.min + Vec3::splat(1e-4)).all() && m.max.cmpge(s.max - Vec3::splat(1e-4)).all())
                .count();
            assert_eq!(holders, 1, "{s:?}");
        }
        // Different thicknesses never merge.
        let thin = OccluderSlab { min: Vec3::new(10.0, 2.9, 0.0), max: Vec3::new(20.0, 3.0, 10.0) };
        assert_eq!(merge_slabs(vec![floor_slab(0.0, 0.0, 10.0, 10.0), thin]).len(), 2);
    }

    /// A CROSSING IN AN EMPTY CELL READS NO SLAB. Two slabs with a 10 m gap
    /// between them in one layer: a lamp under the gap and a fragment over it
    /// (and the sun straight down onto a point under it) cross the floor's
    /// plane in the gap, inside the layer's union box, in a cell of its
    /// occupancy grid no slab touches: the walk reads the layer row and no slab
    /// row, and answers as every slab would (no block). A crossing over a slab
    /// still reads its slab and blocks. Seen red first (2026-10-21) with
    /// `layer_grid_holds` answering yes everywhere: "the gap: the layer row
    /// only, left: (false, 3), right: (false, 1)".
    #[test]
    fn a_crossing_in_an_empty_cell_reads_no_slab() {
        let slabs = [floor_slab(0.0, 0.0, 10.0, 10.0), floor_slab(20.0, 0.0, 30.0, 10.0)];
        // A floor lamp, so a path from a point over the first slab crosses the
        // plane over that slab (at x 8.2), not in the gap beside the lamp.
        let lamp = RoomLight::point(Vec3::new(15.0, 0.5, 5.0), [1.0; 3], 6.0, 20.0);
        let run = layered_rows(&slabs, lamp.pos);
        assert_eq!(run.len(), 3, "one layer row and its two slabs");
        let up = Vec3::new(15.0, 4.0, 5.0);
        assert_eq!(segment_walk(&run, up, lamp.pos), (false, 1), "the gap: the layer row only");
        assert!(!slabs.iter().any(|s| s.blocks_segment(up, lamp.pos)));
        let over = Vec3::new(5.0, 4.0, 5.0);
        let (blocked, read) = segment_walk(&run, over, lamp.pos);
        assert!(blocked && read >= 2, "over a slab: blocked after reading it ({read} rows)");
        let rooms_box = (Vec3::ZERO, Vec3::new(30.0, 6.0, 10.0));
        let sun = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(sun_walk(&run, rooms_box, Vec3::new(15.0, 0.5, 5.0), sun), (false, 1), "the sun through the gap");
        assert_eq!(sun_walk(&run, rooms_box, Vec3::new(25.0, 0.5, 5.0), sun).0, true, "the sun onto a slab");
    }

    /// The homestead upper floor's 16 solid slabs (homestead-upper-floor.md
    /// section 1), merged.
    fn homestead_floor() -> Vec<OccluderSlab> {
        let solid = [
            (20.0, 0.0, 55.0, 22.0),
            (0.0, 22.0, 30.1, 30.0),
            (30.1, 22.0, 34.0, 25.1),
            (31.9, 25.1, 34.0, 26.9),
            (30.1, 26.9, 32.5, 30.0),
            (32.5, 28.1, 34.0, 30.0),
            (0.0, 30.0, 12.0, 51.0),
            (12.0, 30.0, 20.0, 51.0),
            (20.0, 30.0, 34.0, 51.0),
            (34.0, 30.0, 55.0, 38.0),
            (34.0, 38.0, 36.5, 39.3),
            (40.5, 38.0, 55.0, 39.3),
            (34.0, 39.3, 55.0, 51.0),
            (0.0, 51.0, 10.0, 61.0),
            (0.0, 61.0, 10.0, 73.0),
            (40.0, 73.0, 55.0, 89.0),
        ];
        merge_slabs(solid.iter().map(|&(a, b, c, d)| floor_slab(a, b, c, d)).collect())
    }

    /// THE LAYERED SUN WALK ANSWERS AS EVERY SLAB WOULD. `layered_rows` groups the
    /// slabs by mid-plane behind a layer row each, and `sun_walk` (the WGSL's
    /// `slab_sun_blocked`, transcribed) must give the per-slab reference's
    /// answer (`sun_blocked_by_slabs`) at every point of a grid through the
    /// house, on both storeys and above the roof, for a noon sun, low dawn suns
    /// from either side, a steep sun and a sun below the deck, with two layers
    /// (the homestead's floor at 3.0 and a test floor at 6.0). And the reason
    /// for the layers: a dawn sun from the console room reads the two layer
    /// rows and no slab (the per-slab walk tested all 14 slabs there). Seen red
    /// first (2026-10-21) with a layer's union box taken from its nearest slab
    /// alone (`members[0]`): "the walk and the reference differ at [0, 0, 0]
    /// toward [1, 0.08, 0.1]: false against true".
    #[test]
    fn the_layered_sun_walk_answers_as_every_slab_would() {
        let mut slabs = homestead_floor();
        slabs.push(OccluderSlab { min: Vec3::new(10.0, 5.75, 40.0), max: Vec3::new(30.0, 6.0, 60.0) });
        let rooms_box = (Vec3::ZERO, Vec3::new(55.0, 9.0, 89.0));
        let eye = Vec3::new(45.0, 1.7, 45.0);
        let rows = layered_rows(&slabs, eye);
        assert_eq!(rows.len(), slabs.len() + 2, "two layers, each behind its own row");
        let suns = [
            Vec3::new(0.1, 1.0, 0.05),
            Vec3::new(1.0, 0.08, 0.1),
            Vec3::new(-1.0, 0.06, -0.2),
            Vec3::new(0.0, 0.5, -1.0),
            Vec3::new(0.2, -1.0, 0.1),
        ];
        let mut checked = 0;
        for xi in 0..=37 {
            for zi in 0..=60 {
                for y in [0.0f32, 1.2, 2.7, 3.0, 4.5, 6.5] {
                    let p = Vec3::new(xi as f32 * 1.5, y, zi as f32 * 1.5);
                    for d in suns {
                        let walk = sun_walk(&rows, rooms_box, p, d).0;
                        let reference = sun_blocked_by_slabs(&slabs, rooms_box, p, d);
                        assert_eq!(
                            walk, reference,
                            "the walk and the reference differ at {p} toward {d}: {walk} against {reference}"
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 38 * 61 * 6 * 5);
        // The cost at the dawn park: the path from the console room misses both
        // layers' union boxes, so the walk reads the two layer rows and stops.
        let (blocked, read) = sun_walk(&rows, rooms_box, Vec3::new(45.0, 1.0, 45.0), Vec3::new(1.0, 0.08, 0.1));
        assert!(!blocked);
        assert_eq!(read, 2, "a dawn sun reads the layer rows only");
        // Under the floor at noon the nearest slab blocks first: the layer row,
        // then the slab over the eye.
        let (blocked, read) = sun_walk(&rows, rooms_box, Vec3::new(45.0, 1.0, 45.0), Vec3::new(0.1, 1.0, 0.05));
        assert!(blocked);
        assert_eq!(read, 2, "the slab nearest the eye is the first slab row");
    }

    /// THE LAYERED LAMP WALK ANSWERS AS EVERY SLAB WOULD. For five lamps (the
    /// console room's panel downstairs, a lamp upstairs over the stairwell, the
    /// lift lobby's panel by the shaft and the hatch, the workshop's 15 m
    /// panel, the upper hall's strip), the light's run (`slabs_touched`, packed
    /// by `layered_rows`) taken as the GPU takes it (`run_blocks`: the record's
    /// one-layer plane, then the walk; the WGSL's `slab_blocks_light`
    /// transcribed) must give the per-slab answer over EVERY
    /// slab, at every point of a grid through both storeys within the lamp's
    /// range. That also proves the run never leaves out a slab that matters: a
    /// path to a point in range crosses only slabs within range of the lamp.
    /// And the reason for the layers: the console room's panel and a fragment
    /// in its own room read one row, the layer's, where the per-slab run tested
    /// each of its slabs. Seen red first (2026-10-21) with a layer row counting
    /// one slab fewer than follow it (`members.len() - 1`): "the walk and every
    /// slab differ at [32.5, 2.8, 37.5] for the lamp at [38.5, 5.6, 38.6]: false
    /// against true"; and, once the one-layer shortcut joined it, with the
    /// shortcut's side test inverted (`<= 0.0`): "the walk and every slab
    /// differ at [38.75, 2.9, 42.5] for the lamp at [45, 2.7, 45]: false
    /// against true".
    #[test]
    fn the_layered_lamp_walk_answers_as_every_slab_would() {
        let mut slabs = homestead_floor();
        slabs.push(OccluderSlab { min: Vec3::new(10.0, 5.75, 40.0), max: Vec3::new(30.0, 6.0, 60.0) });
        let lamps = [
            RoomLight::point(Vec3::new(45.0, 2.7, 45.0), [1.0, 0.95, 0.85], 9.0, 7.0),
            RoomLight::point(Vec3::new(38.5, 5.6, 38.6), [1.0, 0.85, 0.6], 4.0, 7.0),
            RoomLight::point(Vec3::new(32.0, 5.7, 28.5), [1.0, 0.95, 0.85], 9.0, 7.0),
            RoomLight::point(Vec3::new(24.0, 2.7, 33.0), [0.8, 0.9, 1.0], 11.0, 15.0),
            RoomLight::line(Vec3::new(34.5, 5.6, 39.9), Vec3::new(54.5, 5.6, 39.9), [0.9, 0.95, 1.0], 7.0, 6.0),
        ];
        let mut checked = 0;
        for l in lamps {
            let touched: Vec<OccluderSlab> = slabs_touched(&l, &slabs).iter().map(|&i| slabs[i]).collect();
            let run = layered_rows(&touched, light_anchor(&l));
            let plane = run_plane(&run);
            for xi in 0..=44 {
                for zi in 0..=71 {
                    for y in [0.2f32, 1.5, 2.8, 2.9, 3.0, 4.5, 6.2] {
                        let p = Vec3::new(xi as f32 * 1.25, y, zi as f32 * 1.25);
                        let q = if l.cos_outer < -1.5 { line_light_closest_point(l.pos, l.dir, p) } else { l.pos };
                        if (q - p).length() >= l.range {
                            continue;
                        }
                        let walk = run_blocks(&run, plane, p, q).0;
                        let every = slabs.iter().any(|s| s.blocks_segment(p, q));
                        assert_eq!(walk, every, "the walk and every slab differ at {p} for the lamp at {}: {walk} against {every}", l.pos);
                        checked += 1;
                    }
                }
            }
        }
        // 6,007 grid points lie in the five lamps' ranges (2026-10-21).
        assert!(checked > 5_000, "{checked} points in range");
        // The cost for a lamp on the fragment's own storey: one row per layer.
        let panel = lamps[0];
        let touched: Vec<OccluderSlab> = slabs_touched(&panel, &slabs).iter().map(|&i| slabs[i]).collect();
        let run = layered_rows(&touched, panel.pos);
        assert!(touched.len() >= 2, "the panel's range reaches several slabs ({})", touched.len());
        let (blocked, read) = segment_walk(&run, Vec3::new(44.0, 0.5, 46.0), panel.pos);
        assert!(!blocked);
        assert_eq!(read, 1, "one layer, one row, for {} slabs", touched.len());
        // And with the light record's plane (a one-layer run), no row at all;
        // a run reaching both layers (the workshop's 15 m panel reaches the 6 m
        // test floor) carries no plane and walks.
        assert!((run_plane(&run) - 2.875).abs() < 1e-6, "the floor's mid-plane");
        assert_eq!(run_blocks(&run, run_plane(&run), Vec3::new(44.0, 0.5, 46.0), panel.pos), (false, 0));
        let workshop = lamps[3];
        let wt: Vec<OccluderSlab> = slabs_touched(&workshop, &slabs).iter().map(|&i| slabs[i]).collect();
        assert_eq!(run_plane(&layered_rows(&wt, workshop.pos)), RUN_MANY_LAYERS, "two layers: no shortcut");
    }
}
