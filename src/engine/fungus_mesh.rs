//! How a mushroom crop is drawn (2026-09-27).
//!
//! A mushroom crop's "plant" is a unit of substrate, not a plant
//! (data/garden/yields.ron, MUSHROOMS): one 5 lb fruiting block for oyster and
//! shiitake, one square foot of cased compost bed for button. Until this
//! module the rack drew each as a small procedural plant recipe, a speck
//! floating on its shelf, because what the crop grows from was not drawn at
//! all. This draws what a fruiting room shows, from the numbers in
//! data/plants_visual.ron (`fungi`), where the sources are:
//!
//! - the BLOCK ([`FungusSubstrate::Block`]): a rounded, slightly lumpy block
//!   of the data's size, spawned sawdust mottling to white as the mycelium
//!   runs through it; in its bag (a glossy film, the excess folded over the
//!   top with its filter patch) until the grower takes the bag off; cut with
//!   an X where the fruit comes through; a shiitake block browning to its
//!   bark-like coat before it fruits bare.
//! - the BED ([`FungusSubstrate::Bed`]): a wooden tray of compost across the
//!   shelf, as large as the square feet its plot holds, the spawn running
//!   through it, then cased with dark peat.
//! - the FRUIT ([`FruitHabit`]): nothing while the substrate colonises; pins
//!   from `pin_at`, growing to the flush's full size at harvest, each
//!   mushroom a little ahead of or behind the others. Oyster as overlapping
//!   fan-shaped caps on short side stems bursting from the cut; shiitake as
//!   single umbrella caps standing on the bare block; button as round white
//!   caps coming up through the casing.
//!
//! Pure geometry into a [`PlantMeshBuilder`] (flat per-face colour packed in
//! the UV, the type-20 plant contract), built on the plant worker
//! (`engine::plant_pass`) and drawn with the RIGID plant material
//! (`renderer::plant_mesh::PLANT_RIGID_FLAG`), so a block never sways on its
//! shelf the way a leaf does in the wind.
//!
//! Which surface shading each part gets rides on the plant organ tag
//! (`Organ`): a bag is `Fruit` (the glossy skin shading reads as film); a
//! bare block, the casing and the tray are `Stem` (the matte, grained bark
//! shading; Agrodok 40 calls a shiitake block's coat its bark); a cap is
//! either, as the data's `cap_sheen` says.

use std::f32::consts::TAU;

use glam::{Quat, Vec3};

use crate::engine::plant_layout::{plot_plants, SHELF_POST_R};
use crate::renderer::mesh::Vertex;
use crate::renderer::plant_mesh::{
    BagCut, FruitHabit, FungusFruit, FungusSubstrate, FungusVisualDef, Organ, PlantMeshBuilder,
};

/// A crop's state as its stage says (see `plant_pass::CropDraw`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Growth {
    /// 0 to 1, bucketed by stage the way the Garden panel shows it.
    pub t: f32,
    /// 0 healthy to 1 dying.
    pub wilt: f32,
    pub dead: bool,
}

/// What one plot drew.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Drawn {
    /// Units of substrate: blocks, or square feet of bed.
    pub units: u32,
    /// Mushrooms drawn (pins included), and the widest cap, metres.
    pub caps: u32,
    pub widest_cap_m: f32,
}

impl Drawn {
    fn add_cap(&mut self, d: f32) {
        self.caps += 1;
        self.widest_cap_m = self.widest_cap_m.max(d);
    }
    fn absorb(&mut self, o: Drawn) {
        self.units += o.units;
        self.caps += o.caps;
        self.widest_cap_m = self.widest_cap_m.max(o.widest_cap_m);
    }
}

/// Rounding of a block's edges, metres: a bag pulled tight over packed
/// sawdust has soft edges, not a box's.
const BLOCK_EDGE_R: f32 = 0.018;
/// About this far between the vertices of a block's surface, metres: fine
/// enough that the mycelium's patches (each triangle one colour) read as
/// patches, not as a patchwork of triangles (3.5 cm did, rig 2026-09-27).
const BLOCK_CELL_M: f32 = 0.025;
/// The size of one lump, metres (the value-noise lattice the lumps come from).
const LUMP_CELL_M: f32 = 0.05;
/// How far a block may sit off square to its shelf, radians (about 7 degrees).
const BLOCK_YAW_JITTER: f32 = 0.12;
/// A pin's cap, metres: a mushroom initial just big enough to see.
const PIN_CAP_M: f32 = 0.005;
/// The half-length of one arm of the X cut in a bag, metres: an X about 6 cm
/// across, the cut a grower makes with a knife.
const CUT_ARM_M: f32 = 0.03;
/// A bed stands this far inside its shelf's edges (x, z), metres, so its
/// tray's corners clear the rack's round uprights at the shelf's corners.
const BED_MARGIN_M: [f32; 2] = [SHELF_POST_R * 2.4, SHELF_POST_R * 1.6];
/// A tray's boards: their thickness, and how far they stand above the casing.
const TRAY_WALL_M: f32 = 0.018;
const TRAY_LIP_M: f32 = 0.012;
/// About this far between the vertices of a bed's surface, metres.
const BED_CELL_M: f32 = 0.05;

/// Draw one plot of a mushroom crop: `holds` units of `def`'s substrate on a
/// plot of `size` (width, depth, metres) whose floor centre is `at`, turned
/// `yaw` radians about +Y, with the fruit its growth has reached. Blocks stand
/// where `plant_layout::plot_plants` puts a plot's plants; a bed is one tray
/// holding every unit. `unit_cap` is the most blocks a plot draws
/// (`plot_visual_cap`, 0 for all of them) and `vertex_budget` the most
/// vertices (`plot_vertex_budget`, 0 for no limit): past either, fewer blocks
/// or mushrooms are drawn, never bigger ones. `seed` keeps a rebuild from
/// reshuffling anything.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_plot(
    b: &mut PlantMeshBuilder,
    def: &FungusVisualDef,
    at: Vec3,
    yaw: f32,
    size: [f32; 2],
    holds: u32,
    g: Growth,
    seed: u64,
    unit_cap: u32,
    vertex_budget: u32,
) -> Drawn {
    let holds = holds.max(1);
    match BlockSpec::of(&def.substrate) {
        Some(spec) => {
            let mut n = if unit_cap == 0 { holds } else { holds.min(unit_cap) };
            if vertex_budget > 0 && n > 1 {
                let mut probe = PlantMeshBuilder::new();
                draw_block(&mut probe, &spec, &def.fruit, Vec3::ZERO, Quat::IDENTITY, g, seed);
                let fits = vertex_budget as usize / probe.vertices.len().max(1);
                n = n.min(u32::try_from(fits).unwrap_or(u32::MAX).max(1));
            }
            let spots = plot_plants(size, n, n, seed);
            let turn = Quat::from_rotation_y(yaw);
            let mut out = Drawn::default();
            for (k, s) in spots.spots.iter().enumerate() {
                let unit_seed = mix64(seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
                let turned = yaw + (unit01(unit_seed) * 2.0 - 1.0) * BLOCK_YAW_JITTER;
                let place = at + turn * Vec3::new(s[0], 0.0, s[1]);
                out.absorb(draw_block(b, &spec, &def.fruit, place, Quat::from_rotation_y(turned), g, unit_seed));
            }
            out
        }
        None => match &def.substrate {
            FungusSubstrate::Bed {
                unit_side_m,
                compost_depth_m,
                casing_depth_m,
                compost_color,
                mycelium_color,
                casing_color,
                tray_color,
                colonised_at,
                cased_at,
            } => {
                let spec = BedSpec {
                    side: *unit_side_m,
                    compost: *compost_depth_m,
                    casing: *casing_depth_m,
                    compost_color: *compost_color,
                    mycelium_color: *mycelium_color,
                    casing_color: *casing_color,
                    tray_color: *tray_color,
                    colonised_at: *colonised_at,
                    cased_at: *cased_at,
                };
                draw_bed(b, &spec, &def.fruit, at, Quat::from_rotation_y(yaw), size, holds, g, seed, vertex_budget)
            }
            FungusSubstrate::Block { .. } => Drawn::default(),
        },
    }
}

/// The floor one unit of `def` stands on (width, depth, metres): a block's
/// footprint, or one square of bed. For a crop sown somewhere with no plot
/// (a tower's net cup), which draws its one unit there.
pub(crate) fn unit_footprint(def: &FungusVisualDef) -> [f32; 2] {
    match &def.substrate {
        FungusSubstrate::Block { size_m, .. } => [size_m[0] * 1.2, size_m[2] * 1.2],
        FungusSubstrate::Bed { unit_side_m, .. } => {
            let s = unit_side_m + 2.0 * BED_MARGIN_M[0].max(BED_MARGIN_M[1]);
            [s, s]
        }
    }
}

/// How far a unit's fruit has grown: None before its pins show (or while
/// the substrate colonises), else 0 at the first pins to 1 at harvest.
fn fruit_growth(t: f32, pin_at: f32) -> Option<f32> {
    (t >= pin_at).then(|| ((t - pin_at) / (1.0 - pin_at).max(1e-3)).clamp(0.0, 1.0))
}

// ── Blocks ────────────────────────────────────────────────────────────────

/// A block substrate's numbers, out of its enum variant.
struct BlockSpec {
    size: Vec3,
    lump: f32,
    substrate: [f32; 3],
    mycelium: [f32; 3],
    colonised_at: f32,
    coat: Option<[f32; 3]>,
    coat_at: f32,
    bag: [f32; 3],
    patch: [f32; 3],
    bag_off_at: f32,
    cut: BagCut,
}

impl BlockSpec {
    fn of(s: &FungusSubstrate) -> Option<Self> {
        match s {
            FungusSubstrate::Block {
                size_m,
                lump_m,
                substrate_color,
                mycelium_color,
                colonised_at,
                coat_color,
                coat_at,
                bag_color,
                filter_patch_color,
                bag_off_at,
                cut,
            } => Some(BlockSpec {
                size: Vec3::from_array(*size_m).max(Vec3::splat(0.02)),
                lump: lump_m.max(0.0),
                substrate: *substrate_color,
                mycelium: *mycelium_color,
                colonised_at: *colonised_at,
                coat: *coat_color,
                coat_at: *coat_at,
                bag: *bag_color,
                patch: *filter_patch_color,
                bag_off_at: *bag_off_at,
                cut: *cut,
            }),
            FungusSubstrate::Bed { .. } => None,
        }
    }
}

/// A block's rounded, lumpy surface in its own frame: centred on x and z, its
/// base on y = 0, `half` its half size.
struct BlockShape {
    half: Vec3,
    r: f32,
    lump: f32,
    seed: u64,
}

impl BlockShape {
    /// The surface over the point `p` of the unrounded box's faces (that box
    /// centred on the origin), and the outward direction there. The rounding
    /// is the classic rounded box: the nearest point of the box shrunk by the
    /// radius, pushed out by it. Lumps push along the same direction and fade
    /// to nothing at the foot, so the block stands flat on its shelf.
    fn surface(&self, p: Vec3) -> (Vec3, Vec3) {
        let inner = (self.half - Vec3::splat(self.r)).max(Vec3::ZERO);
        let q = p.clamp(-inner, inner);
        let d = p - q;
        let dir = if d.length_squared() > 1e-12 { d.normalize() } else { Vec3::Y };
        let foot = ((p.y + self.half.y) / (self.r * 2.0).max(1e-4)).clamp(0.0, 1.0);
        let bump = noise3(p / LUMP_CELL_M, self.seed) * self.lump * foot;
        (q + dir * (self.r + bump) + Vec3::Y * self.half.y, dir)
    }
}

/// The five faces of a block that show (the bottom sits on the shelf): the
/// axis each faces along and which way.
const BLOCK_FACES: [(usize, f32); 5] = [(1, 1.0), (2, 1.0), (2, -1.0), (0, 1.0), (0, -1.0)];

/// The other two axes of `axis`.
fn tangents(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

/// Grid stations along one axis of a rounded box of half size `h` and edge
/// radius `r`: both ends, where the rounding starts on either side, and even
/// steps of about `cell` between, so every edge's curve has cells of its own.
fn stations(h: f32, r: f32, cell: f32) -> Vec<f32> {
    let flat = (h - r).max(0.0);
    let mut v = vec![-h];
    if flat > 1e-4 {
        let n = ((2.0 * flat / cell).round() as usize).max(1);
        v.extend((0..=n).map(|i| -flat + 2.0 * flat * i as f32 / n as f32));
    }
    v.push(h);
    v
}

/// One block and its fruit, standing at `base` (the middle of its foot) turned
/// by `rot`.
fn draw_block(
    b: &mut PlantMeshBuilder,
    s: &BlockSpec,
    fr: &FungusFruit,
    base: Vec3,
    rot: Quat,
    g: Growth,
    seed: u64,
) -> Drawn {
    let half = s.size * 0.5;
    let shape = BlockShape { half, r: BLOCK_EDGE_R.min(half.min_element() * 0.6), lump: s.lump, seed };
    let t = g.t.clamp(0.0, 1.0);
    let world = |p: Vec3| base + rot * p;
    // How far the spawn has run, how far the coat has come, and whether the
    // bag is still on and cut.
    let colonised = (t / s.colonised_at.max(1e-3)).clamp(0.0, 1.0);
    let coat = s.coat.map(|c| (c, ((t - s.colonised_at) / (s.coat_at - s.colonised_at).max(1e-3)).clamp(0.0, 1.0)));
    let bagged = t < s.bag_off_at;
    let cut_open = bagged && s.cut != BagCut::Removed && t >= s.colonised_at;

    // Each triangle is one colour, from where it is on the block: the
    // substrate turning white in patches that spread out from the spawn and
    // meet as the mycelium grows through (not all at once), then, for a
    // shiitake block, browning in patches to its coat; the bag's film a
    // little over all of it.
    let inv = rot.inverse();
    let tri_color = |at: Vec3| -> [f32; 3] {
        let l = inv * (at - base);
        let spawn = patches(l, seed);
        let mut col = lerp3(s.substrate, s.mycelium, spreading(colonised, spawn));
        let tone = noise01(l / 0.02, seed ^ 0x7E);
        if let Some((cc, k)) = coat {
            let brown = spreading(k, patches(l, seed ^ 0xC0A7));
            col = lerp3(col, scale3(cc, 0.85 + 0.3 * tone), brown);
        }
        col = scale3(col, 0.95 + 0.1 * tone);
        if bagged {
            col = lerp3(col, s.bag, 0.15);
        }
        wither(col, g.wilt * 0.6)
    };
    b.set_organ(if bagged { Organ::Fruit } else { Organ::Stem });
    for &(axis, sign) in BLOCK_FACES.iter() {
        let (ua, va) = tangents(axis);
        let su = stations(half[ua], shape.r, BLOCK_CELL_M);
        let sv = stations(half[va], shape.r, BLOCK_CELL_M);
        let (rows, cols) = (sv.len(), su.len());
        let mut pts = Vec::with_capacity(rows * cols);
        let mut dirs = Vec::with_capacity(rows * cols);
        for &v in &sv {
            for &u in &su {
                let mut p = Vec3::ZERO;
                p[axis] = sign * half[axis];
                p[ua] = u;
                p[va] = v;
                let (pos, dir) = shape.surface(p);
                pts.push(world(pos));
                dirs.push(rot * dir);
            }
        }
        emit_grid_at(b, &pts, rows, cols, false, &|r, c| dirs[r * cols + c], &mut |_, _, at| tri_color(at));
    }

    // The bag's excess, folded flat over the top, with its filter patch
    // (unless the top is where it was cut open).
    if bagged && !(cut_open && s.cut == BagCut::TopX) {
        let film = wither(s.bag, g.wilt * 0.4);
        let (x, z0, z1) = (half.x - shape.r * 0.5, -(half.z - shape.r * 0.5), half.z * 0.5);
        let (top, skirt) = (s.size.y + s.lump * 0.8 + 0.002, s.size.y - s.lump - 0.004);
        let v = |x: f32, y: f32, z: f32| world(Vec3::new(x, y, z));
        b.set_organ(Organ::Fruit);
        quad(b, [v(-x, top, z0), v(x, top, z0), v(x, top, z1), v(-x, top, z1)], rot * Vec3::Y, film);
        quad(b, [v(-x, skirt, z1), v(x, skirt, z1), v(x, top, z1), v(-x, top, z1)], rot * Vec3::Z, film);
        quad(b, [v(-x, skirt, z0), v(x, skirt, z0), v(x, top, z0), v(-x, top, z0)], rot * -Vec3::Z, film);
        quad(b, [v(x, skirt, z0), v(x, skirt, z1), v(x, top, z1), v(x, top, z0)], rot * Vec3::X, film);
        quad(b, [v(-x, skirt, z0), v(-x, skirt, z1), v(-x, top, z1), v(-x, top, z0)], rot * -Vec3::X, film);
        b.set_organ(Organ::Stem);
        let (pz, py) = (z0 + 0.012, top + 0.001);
        let patch = wither(s.patch, g.wilt * 0.4);
        quad(b, [v(-0.025, py, pz), v(0.025, py, pz), v(0.025, py, pz + 0.035), v(-0.025, py, pz + 0.035)], rot * Vec3::Y, patch);
    }

    // Where the fruit comes through: the middle of the cut, or of the front
    // face if the bag came off whole. In the block's centred frame.
    let (site_axis, site_uv) = match s.cut {
        BagCut::TopX => (1, [0.0, 0.0]),
        BagCut::FrontX | BagCut::Removed => (2, [0.0, half.y * 0.1]),
    };
    let site_point = |du: f32, dv: f32| {
        let (ua, va) = tangents(site_axis);
        let mut p = Vec3::ZERO;
        p[site_axis] = half[site_axis];
        p[ua] = site_uv[0] + du;
        p[va] = site_uv[1] + dv;
        shape.surface(p)
    };
    if cut_open {
        // The X: two strips riding the lumpy surface, a shade darker where
        // the film is parted and the substrate shows.
        b.set_organ(Organ::Stem);
        let arm = CUT_ARM_M.min(half[tangents(site_axis).0].min(half[tangents(site_axis).1]) * 0.8);
        let slit = wither(scale3(s.mycelium, 0.45), g.wilt * 0.6);
        for (k, diag) in [[1.0f32, 1.0], [1.0, -1.0]].iter().enumerate() {
            let len = (diag[0] * diag[0] + diag[1] * diag[1]).sqrt();
            let (d, n) = ([diag[0] / len, diag[1] / len], [-diag[1] / len, diag[0] / len]);
            let lift = 0.0012 + 0.0004 * k as f32;
            let mut pts = Vec::with_capacity(6);
            let mut dirs = Vec::with_capacity(6);
            for step in [-1.0f32, 0.0, 1.0] {
                for side in [-1.0f32, 1.0] {
                    let (du, dv) = (d[0] * arm * step + n[0] * 0.002 * side, d[1] * arm * step + n[1] * 0.002 * side);
                    let (pos, dir) = site_point(du, dv);
                    pts.push(world(pos + dir * lift));
                    dirs.push(rot * dir);
                }
            }
            emit_grid(b, &pts, 3, 2, false, &|r, c| dirs[r * 2 + c], &mut |_, _| slit);
        }
    }

    let mut drawn = Drawn { units: 1, ..Drawn::default() };
    let Some(gf) = fruit_growth(t, fr.pin_at).filter(|_| !g.dead) else { return drawn };
    let frame = BlockFrame { base, rot, half };
    let tissue = cap_tissue(fr);
    match fr.habit {
        FruitHabit::ShelfCluster => {
            let (pos, dir) = site_point(0.0, 0.0);
            drawn.absorb(shelf_cluster(b, fr, world(pos), rot * dir, gf, g.wilt, seed, &frame));
        }
        FruitHabit::Scattered => drawn.absorb(scattered(b, fr, &shape, &frame, gf, g.wilt, seed)),
        FruitHabit::Buttons => {
            // Round caps across the block's top (the data puts buttons on a
            // bed; this is what a block sown with them would show).
            let mut rng = Rng::new(seed ^ 0xB0_77_0B);
            let top = [half.x - shape.r, half.z - shape.r];
            let per = fr.per_unit;
            let spots = scatter(&mut rng, top, per, fr, gf);
            for (x, z, d, gi) in spots {
                let (pos, dir) = shape.surface(Vec3::new(x, half.y, z));
                let (len, rs) = stipe(fr, &mut rng, gi);
                b.set_organ(tissue);
                button(b, fr, world(pos) - rot * dir * 0.004, rot * dir, d, gi, len + 0.004, rs, g.wilt);
                drawn.add_cap(d);
            }
        }
    }
    drawn
}

/// Where a block stands (the middle of its foot, its turn, its half size),
/// for keeping what grows on it out of it.
struct BlockFrame {
    base: Vec3,
    rot: Quat,
    half: Vec3,
}

/// How far clear of a block a cap stays, metres.
const CAP_CLEARANCE_M: f32 = 0.0015;

impl BlockFrame {
    /// Move `vs` (a cap just built) along `n` (a unit direction, world) just
    /// far enough that none of it is inside the block, and say how far that
    /// was. A cap that would have grown into its block stands out on a longer
    /// stem instead, whole: moving it rather than squashing it keeps every
    /// face of it turned the way its normals say.
    fn clear(&self, vs: &mut [Vertex], n: Vec3) -> f32 {
        let inv = self.rot.inverse();
        let ln = inv * n;
        let h = self.half + Vec3::splat(CAP_CLEARANCE_M);
        let mut need = 0.0f32;
        for v in vs.iter() {
            let l = inv * (Vec3::from(v.position) - self.base) - Vec3::Y * self.half.y;
            if l.x.abs() >= h.x || l.z.abs() >= h.z || l.y >= h.y || l.y <= -self.half.y {
                continue;
            }
            // Where the ray from this vertex along `n` leaves the box.
            let mut exit = f32::INFINITY;
            for a in 0..3 {
                if ln[a].abs() > 1e-4 {
                    let bound = if ln[a] > 0.0 { h[a] } else if a == 1 { -self.half.y } else { -h[a] };
                    exit = exit.min((bound - l[a]) / ln[a]);
                }
            }
            if exit.is_finite() {
                need = need.max(exit);
            }
        }
        if need > 0.0 {
            let d = n * need;
            for v in vs {
                v.position = (Vec3::from(v.position) + d).to_array();
            }
        }
        need
    }
}

/// The surface shading a species' mushrooms get, from the data's
/// `cap_sheen`: a moist or smooth cap takes the glossy `Fruit` skin shading,
/// a dry one the matte, grained `Stem` shading.
fn cap_tissue(fr: &FungusFruit) -> Organ {
    if fr.cap_sheen {
        Organ::Fruit
    } else {
        Organ::Stem
    }
}

/// One mushroom's share of its flush's growth: a flush is never all one age,
/// so each is a little ahead of or behind `gf`.
fn cap_growth(rng: &mut Rng, gf: f32) -> f32 {
    (gf * rng.range(0.8, 1.15)).clamp(0.0, 1.0)
}

/// A cap's diameter at growth `gi`: a pin, growing to its size at harvest
/// (somewhere in the data's range).
fn cap_size(rng: &mut Rng, fr: &FungusFruit, gi: f32) -> f32 {
    let full = rng.range(fr.cap_diameter_m[0], fr.cap_diameter_m[1].max(fr.cap_diameter_m[0]));
    lerp(PIN_CAP_M, full, smooth(gi))
}

/// A stipe's length and radius at growth `gi`.
fn stipe(fr: &FungusFruit, rng: &mut Rng, gi: f32) -> (f32, f32) {
    let s = smooth(gi);
    (lerp(0.002, fr.stipe_length_m * rng.range(0.7, 1.15), s), lerp(0.0012, fr.stipe_diameter_m * 0.5, s))
}

/// Oyster: overlapping fan-shaped caps on short side stems, one over another
/// like shingles, all bursting from one knot where the bag is cut (`root`,
/// the cut facing `out`). From the side the caps fan out across the face;
/// from a cut in the top they spread all round.
#[allow(clippy::too_many_arguments)]
fn shelf_cluster(
    b: &mut PlantMeshBuilder,
    fr: &FungusFruit,
    root: Vec3,
    out: Vec3,
    gf: f32,
    wilt: f32,
    seed: u64,
    frame: &BlockFrame,
) -> Drawn {
    let up = Vec3::Y;
    let mut rng = Rng::new(seed ^ 0x0A57_E2C1);
    let from_top = out.y > 0.7;
    let out_h = horizontal(out, Vec3::Z);
    let side = up.cross(out_h).normalize();
    let grown = smooth(gf);
    let knot = lerp(0.004, 0.016, grown);
    // The height between the cluster's two rings of caps: a pin cluster is a
    // tight knot, a ripe one stands the depth of a cap's flesh and gills
    // apart.
    let tier_step = lerp(0.003, 0.25 * fr.cap_diameter_m[1], grown);
    let n = fr.per_unit.max(1);
    // Two rings like the petals of an open flower: the lower one wider and
    // reaching further out, the upper one narrower and closer in, so each
    // cap has most of its top in the open (a tall stack of caps one over the
    // other shaded itself black under the rack's top light, rig 2026-09-27).
    let lower = n.div_ceil(2);
    let mut drawn = Drawn::default();
    for i in 0..n {
        let gi = cap_growth(&mut rng, gf);
        let (ring, k, count) = if i < lower { (0.0f32, i, lower) } else { (1.0, i - lower, n - lower) };
        let d = cap_size(&mut rng, fr, gi) * (1.0 - 0.15 * ring);
        let (len, rs) = stipe(fr, &mut rng, gi);
        let frac = (k as f32 + 0.5) / count.max(1) as f32;
        let f = if from_top {
            let az = TAU * (k as f32 + 0.5 * ring + rng.range(-0.15, 0.15)) / count.max(1) as f32;
            Vec3::new(az.cos(), 0.0, az.sin())
        } else {
            let spread = lerp(1.05, 0.6, ring);
            let az = lerp(-spread, spread, frac) + rng.range(-0.1, 0.1);
            (out_h * az.cos() + side * az.sin()).normalize()
        };
        let rise = (ring - 0.5 + rng.range(-0.15, 0.15)) * tier_step;
        let reach = lerp(1.2, 0.7, ring);
        let start = root + up * rise * 0.6 + out * knot * 0.2;
        let stem_dir = (f * 0.7 + out * 0.4 + up * (0.2 + 0.2 * ring)).normalize();
        let a = start + stem_dir * (len + knot) * reach + up * rise * 0.4;
        // Each cap is held about level, the lower ring a touch down and the
        // upper a touch up. (Tipping them 6 to 29 degrees up turned their
        // tops away from any camera above the shelf and showed only the
        // unlit gills, rig 2026-09-27.)
        let lift = rng.range(-0.08, 0.08) + 0.1 * ring;
        b.set_organ(cap_tissue(fr));
        let c0 = b.vertices.len();
        oyster_cap(b, fr, a, f, d, gi, lift, wilt, &mut rng);
        let pushed = frame.clear(&mut b.vertices[c0..], out);
        b.tube(start.to_array(), (a + out * pushed).to_array(), rs * 1.3, rs, 5, wither(fr.stipe_color, wilt));
        drawn.add_cap(d);
    }
    // The knot every stem grows from.
    b.set_organ(cap_tissue(fr));
    blob(b, root + out * knot * 0.7, Vec3::new(knot, knot * 0.85 + tier_step * 0.6, knot), wither(fr.stipe_color, wilt));
    drawn
}

/// One oyster cap: a fan attached at its edge (`a`, where its stem ends),
/// reaching `f` (horizontal) with its margin lifted `lift` radians, `d`
/// across, kidney-shaped with a wavy margin, fleshy toward the stem and thin
/// at the edge, a shallow funnel at the stem and its margin rolled under
/// while it is young; gills underneath as radial pleats. Darker at the stem
/// and paler toward the margin, as Kuo's caps fade.
#[allow(clippy::too_many_arguments)]
fn oyster_cap(b: &mut PlantMeshBuilder, fr: &FungusFruit, a: Vec3, f: Vec3, d: f32, g: f32, lift: f32, wilt: f32, rng: &mut Rng) {
    const COLS: usize = 13;
    const PHI: f32 = 1.6;
    let s = Vec3::Y.cross(f).normalize();
    // The cap's own frame: its forward tipped up by the lift.
    let (fwd, up) = (f * lift.cos() + Vec3::Y * lift.sin(), Vec3::Y * lift.cos() - f * lift.sin());
    let wob = rng.range(0.0, TAU);
    let curl = 1.0 - 0.7 * g;
    let len = |phi: f32| 0.85 * d * (0.62 + 0.38 * (phi * 0.9).cos()) * (1.0 + 0.07 * (3.0 * phi + wob).sin());
    let mid = |rho: f32| d * (0.09 * rho - 0.24 * curl * rho * rho * rho - 0.03 * (1.0 - rho) * (1.0 - rho));
    let thick = |rho: f32| d * (0.13 * (1.0 - rho).powf(1.5) + 0.014);
    let phi_at = |c: usize| -PHI + 2.0 * PHI * c as f32 / (COLS - 1) as f32;
    let dir_at = |c: usize| fwd * phi_at(c).cos() + s * phi_at(c).sin();
    let cap_c = wither(scale3(lerp3(fr.cap_young_color, fr.cap_color, g), rng.range(0.92, 1.08)), wilt);
    let gill = wither(fr.gill_color, wilt);
    let point = |rho: f32, c: usize, dy: f32| a + dir_at(c) * rho * len(phi_at(c)) + up * (mid(rho) + dy);

    let top_rho = [0.0, 0.5, 0.82, 1.0];
    let pts: Vec<Vec3> =
        top_rho.iter().flat_map(|&rho| (0..COLS).map(move |c| (rho, c))).map(|(rho, c)| point(rho, c, thick(rho) * 0.5)).collect();
    emit_grid(b, &pts, top_rho.len(), COLS, false, &|_, _| up, &mut |r, _| scale3(cap_c, [0.78, 0.92, 1.06][r.min(2)]));
    // Gills: pleats that stand down between the stem and the margin.
    let under_rho = [1.0, 0.55, 0.08];
    let pts: Vec<Vec3> = under_rho
        .iter()
        .flat_map(|&rho| (0..COLS).map(move |c| (rho, c)))
        .map(|(rho, c)| {
            let pleat = if c % 2 == 1 { d * 0.08 * rho * (1.0 - rho) } else { 0.0 };
            point(rho, c, -thick(rho) * 0.5 + pleat)
        })
        .collect();
    emit_grid(b, &pts, under_rho.len(), COLS, false, &|_, _| -up, &mut |_, c| {
        if c % 2 == 0 {
            gill
        } else {
            scale3(gill, 0.82)
        }
    });
    // The margin between them.
    let pts: Vec<Vec3> = [0.5f32, -0.5]
        .iter()
        .flat_map(|&k| (0..COLS).map(move |c| (k, c)))
        .map(|(k, c)| point(1.0, c, thick(1.0) * k))
        .collect();
    emit_grid(b, &pts, 2, COLS, false, &|_, c| dir_at(c), &mut |_, _| scale3(cap_c, 0.95));
}

/// Shiitake: single umbrella caps standing out of the bare block's top and
/// sides, each stem leaving the surface square to it and turning up so the
/// cap faces up.
fn scattered(b: &mut PlantMeshBuilder, fr: &FungusFruit, shape: &BlockShape, frame: &BlockFrame, gf: f32, wilt: f32, seed: u64) -> Drawn {
    let (half, base, rot) = (shape.half, frame.base, frame.rot);
    let mut rng = Rng::new(seed ^ 0x5417_7A6E);
    // The faces a mushroom can come out of, by area: the top and the four
    // sides (the foot is on the shelf).
    let area = |axis: usize| {
        let (u, v) = tangents(axis);
        4.0 * half[u] * half[v]
    };
    let total: f32 = BLOCK_FACES.iter().map(|&(axis, _)| area(axis)).sum();
    let mut placed: Vec<(Vec3, f32)> = Vec::new();
    let mut drawn = Drawn::default();
    for _ in 0..fr.per_unit {
        let gi = cap_growth(&mut rng, gf);
        let d = cap_size(&mut rng, fr, gi);
        let (len, rs) = stipe(fr, &mut rng, gi);
        let room = d.max(0.02);
        let mut spot = None;
        for _try in 0..12 {
            let mut pick = rng.f() * total;
            let &(axis, sign) =
                BLOCK_FACES.iter().find(|&&(axis, _)| {
                    pick -= area(axis);
                    pick <= 0.0
                })
                .unwrap_or(&BLOCK_FACES[0]);
            let (ua, va) = tangents(axis);
            let mut p = Vec3::ZERO;
            p[axis] = sign * half[axis];
            p[ua] = rng.range(-0.7, 0.7) * half[ua];
            p[va] = rng.range(-0.7, 0.7) * half[va];
            let (pos, dir) = shape.surface(p);
            let (w, n) = (base + rot * pos, rot * dir);
            if placed.iter().all(|(q, dq)| q.distance(w) > 0.5 * (room + dq)) {
                spot = Some((w, n));
                break;
            }
        }
        let Some((p, n)) = spot else { continue };
        placed.push((p, room));
        let bend = (n * 0.35 + Vec3::Y).normalize();
        let (p0, p1) = (p - n * 0.003, p + n * len * 0.45);
        let p2 = p1 + bend * len * 0.6;
        b.set_organ(cap_tissue(fr));
        let c0 = b.vertices.len();
        umbrella(b, fr, p2, bend, d, gi, rs, wilt, &mut rng);
        let pushed = frame.clear(&mut b.vertices[c0..], n);
        let stem = wither(fr.stipe_color, wilt);
        b.tube(p0.to_array(), p1.to_array(), rs * 1.15, rs, 6, stem);
        b.tube(p1.to_array(), (p2 + n * pushed).to_array(), rs, rs * 0.95, 6, stem);
        drawn.add_cap(d);
    }
    drawn
}

/// A shiitake cap on the stem top `top`, its axis `axis`: a convex dome,
/// flattening as it opens, darker at the centre, its margin incurved and
/// flecked with the veil's remnants while young; gills underneath rising
/// from the margin to the stem.
#[allow(clippy::too_many_arguments)]
fn umbrella(b: &mut PlantMeshBuilder, fr: &FungusFruit, top: Vec3, axis: Vec3, d: f32, g: f32, rs: f32, wilt: f32, rng: &mut Rng) {
    const SEG: usize = 10;
    let r = d * 0.5;
    let flat = lerp(0.72, 0.45, g);
    let inc = lerp(1.0, 0.25, g);
    let prof = |u: f32| {
        let ang = u * 1.45;
        (r * ang.sin() / 1.45f32.sin(), r * flat * ang.cos())
    };
    let (xm, ym) = {
        let (x, y) = prof(1.0);
        (x - r * 0.06 * inc, y - r * 0.12 * inc)
    };
    let y_under = ym + r * 0.3 * flat;
    let origin = top - axis * y_under;
    let (sa, sb) = basis(axis);
    let turn = rng.range(0.0, TAU);
    let ring = |x: f32, y: f32, k: usize| {
        let th = turn + TAU * k as f32 / SEG as f32;
        origin + axis * y + (sa * th.cos() + sb * th.sin()) * x
    };
    let rows = [prof(0.0), prof(0.35), prof(0.7), (xm, ym)];
    let pts: Vec<Vec3> = rows.iter().flat_map(|&(x, y)| (0..SEG).map(move |k| ring(x, y, k))).collect();
    let cap_c = wither(scale3(lerp3(fr.cap_young_color, fr.cap_color, g), rng.range(0.92, 1.08)), wilt);
    let veil = wither([0.9, 0.87, 0.8], wilt);
    let flecks: Vec<bool> = (0..SEG).map(|_| g < 0.9 && rng.f() < 0.35).collect();
    emit_grid(b, &pts, rows.len(), SEG, true, &|r, k| (pts[r * SEG + k] - origin).normalize_or_zero(), &mut |r, k| {
        if r == 2 && flecks[k] {
            veil
        } else {
            scale3(cap_c, [0.78, 0.92, 1.05][r.min(2)])
        }
    });
    let gill = wither(fr.gill_color, wilt);
    let under = [(xm, ym), (rs * 1.3, y_under)];
    let pts: Vec<Vec3> = under.iter().flat_map(|&(x, y)| (0..SEG).map(move |k| ring(x, y, k))).collect();
    emit_grid(b, &pts, 2, SEG, true, &|_, _| -axis, &mut |_, k| if k % 2 == 0 { gill } else { scale3(gill, 0.84) });
}

/// A button: a round cap closed by its veil, on a short thick stem whose foot
/// is in the ground at `ground` (`axis` up the stem).
#[allow(clippy::too_many_arguments)]
fn button(b: &mut PlantMeshBuilder, fr: &FungusFruit, ground: Vec3, axis: Vec3, d: f32, g: f32, len: f32, rs: f32, wilt: f32) {
    const SEG: usize = 7;
    let r = d * 0.5;
    // "Convex to nearly round at first" (Kuo), opening a little by harvest.
    let flat = lerp(0.95, 0.75, g);
    let prof = |u: f32| {
        let ang = u * 1.75;
        (r * ang.sin(), r * flat * ang.cos())
    };
    let (xm, ym) = prof(1.0);
    let y_veil = ym + r * 0.05;
    let stem_top = ground + axis * len;
    let origin = stem_top - axis * y_veil;
    let (sa, sb) = basis(axis);
    let ring = |x: f32, y: f32, k: usize| {
        let th = TAU * k as f32 / SEG as f32;
        origin + axis * y + (sa * th.cos() + sb * th.sin()) * x
    };
    let stem = wither(fr.stipe_color, wilt);
    b.tube(ground.to_array(), stem_top.to_array(), rs * 1.05, rs, 5, stem);
    let rows = [prof(0.0), prof(0.4), prof(0.75), (xm, ym)];
    let pts: Vec<Vec3> = rows.iter().flat_map(|&(x, y)| (0..SEG).map(move |k| ring(x, y, k))).collect();
    let cap_c = wither(lerp3(fr.cap_young_color, fr.cap_color, g), wilt);
    emit_grid(b, &pts, rows.len(), SEG, true, &|r, k| (pts[r * SEG + k] - origin).normalize_or_zero(), &mut |r, _| {
        scale3(cap_c, [1.0, 0.98, 0.95][r.min(2)])
    });
    // The veil, closed over the gills.
    let under = [(xm, ym), (rs * 1.2, y_veil)];
    let pts: Vec<Vec3> = under.iter().flat_map(|&(x, y)| (0..SEG).map(move |k| ring(x, y, k))).collect();
    let veil = wither(fr.gill_color, wilt);
    emit_grid(b, &pts, 2, SEG, true, &|_, _| -axis, &mut |_, _| veil);
}

/// Up to `n` mushroom spots scattered over a rectangle of half size `half`
/// (x, z), each at least about its own width from the next: (x, z, cap
/// diameter, growth). A spot that finds no room in a few tries is skipped,
/// so a crowded patch draws fewer, never overlapping, caps.
fn scatter(rng: &mut Rng, half: [f32; 2], n: u32, fr: &FungusFruit, gf: f32) -> Vec<(f32, f32, f32, f32)> {
    let mut out: Vec<(f32, f32, f32, f32)> = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let gi = cap_growth(rng, gf);
        let d = cap_size(rng, fr, gi);
        let room = d.max(0.012);
        for _try in 0..8 {
            let (hx, hz) = ((half[0] - room * 0.5).max(0.0), (half[1] - room * 0.5).max(0.0));
            let (x, z) = (rng.range(-hx, hx), rng.range(-hz, hz));
            let free = out.iter().all(|&(ox, oz, od, _)| {
                ((ox - x).powi(2) + (oz - z).powi(2)).sqrt() > 0.55 * (room + od.max(0.012))
            });
            if free {
                out.push((x, z, d, gi));
                break;
            }
        }
    }
    out
}

// ── Beds ──────────────────────────────────────────────────────────────────

/// A bed substrate's numbers, out of its enum variant.
struct BedSpec {
    side: f32,
    compost: f32,
    casing: f32,
    compost_color: [f32; 3],
    mycelium_color: [f32; 3],
    casing_color: [f32; 3],
    tray_color: [f32; 3],
    colonised_at: f32,
    cased_at: f32,
}

/// The bed a plot of `units` square units makes on a plot of `plot` (width,
/// depth): the width and depth of its tray, metres. It runs the plot's width
/// when it holds at least a row of units, and is never larger than the plot
/// less its margins.
fn bed_size(side: f32, units: u32, plot: [f32; 2]) -> [f32; 2] {
    let area = units.max(1) as f32 * side * side;
    let wmax = (plot[0] - 2.0 * BED_MARGIN_M[0]).max(side * 0.25);
    let dmax = (plot[1] - 2.0 * BED_MARGIN_M[1]).max(side * 0.25);
    if area >= wmax * side {
        [wmax, (area / wmax).min(dmax)]
    } else {
        let d = side.min(dmax);
        [(area / d).min(wmax), d]
    }
}

/// A bed and its fruit: the tray centred at `at` (the plot's floor) turned by
/// `rot`, sized for `units` (see [`bed_size`]).
#[allow(clippy::too_many_arguments)]
fn draw_bed(
    b: &mut PlantMeshBuilder,
    s: &BedSpec,
    fr: &FungusFruit,
    at: Vec3,
    rot: Quat,
    plot: [f32; 2],
    units: u32,
    g: Growth,
    seed: u64,
    vertex_budget: u32,
) -> Drawn {
    let start = b.vertices.len();
    let [bw, bd] = bed_size(s.side, units, plot);
    let t = g.t.clamp(0.0, 1.0);
    let cased = t >= s.cased_at;
    let level = s.compost + if cased { s.casing } else { 0.0 };
    let wall_h = s.compost + s.casing + TRAY_LIP_M;
    let world = |p: Vec3| at + rot * p;
    let (hw, hd, tw) = (bw * 0.5, bd * 0.5, TRAY_WALL_M.min(bw * 0.2).min(bd * 0.2));

    // The tray: four boards, the grained `Stem` shading reading as wood.
    b.set_organ(Organ::Stem);
    let wood = wither(s.tray_color, g.wilt * 0.3);
    let boards = [
        (Vec3::new(-hw, 0.0, hd - tw), Vec3::new(hw, wall_h, hd)),
        (Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, wall_h, -hd + tw)),
        (Vec3::new(hw - tw, 0.0, -hd + tw), Vec3::new(hw, wall_h, hd - tw)),
        (Vec3::new(-hw, 0.0, -hd + tw), Vec3::new(-hw + tw, wall_h, hd - tw)),
    ];
    for (k, (lo, hi)) in boards.into_iter().enumerate() {
        let tone = 0.92 + 0.16 * unit01(mix64(seed ^ k as u64));
        flat_box(b, lo, hi, &world, rot, scale3(wood, tone));
    }

    // The surface: compost the spawn runs through in white patches, then
    // the casing, crumbly and dark, with the mycelium showing through it in
    // patches as the pins come.
    let (iw, id) = (hw - tw, hd - tw);
    let cols = ((2.0 * iw / BED_CELL_M).ceil() as usize).max(1) + 1;
    let rows = ((2.0 * id / BED_CELL_M).ceil() as usize).max(1) + 1;
    let crumb = if cased { 0.004 } else { 0.006 };
    let height = |x: f32, z: f32| level + noise3(Vec3::new(x, 0.0, z) / 0.03, seed) * crumb;
    let pts: Vec<Vec3> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .map(|(r, c)| {
            let (x, z) = (-iw + 2.0 * iw * c as f32 / (cols - 1) as f32, -id + 2.0 * id * r as f32 / (rows - 1) as f32);
            world(Vec3::new(x, height(x, z), z))
        })
        .collect();
    let colonised = (t / s.colonised_at.max(1e-3)).clamp(0.0, 1.0);
    let showing = ((t - (fr.pin_at - 0.1)) / 0.1).clamp(0.0, 1.0);
    let up = rot * Vec3::Y;
    let inv = rot.inverse();
    emit_grid_at(b, &pts, rows, cols, false, &|_, _| up, &mut |_, _, p| {
        let l = inv * (p - at);
        let tone = 0.9 + 0.2 * noise01(l / 0.015, seed ^ 0x7E);
        let col = if cased {
            // The mycelium coming up through the casing before the pins, in
            // patches: most of the casing stays dark peat.
            let a = spreading(showing * 0.35, patches(l, seed ^ 0xCA5E)) * 0.35;
            lerp3(scale3(s.casing_color, tone), s.mycelium_color, a)
        } else {
            let a = spreading(colonised, patches(l, seed)) * 0.75;
            lerp3(scale3(s.compost_color, tone), s.mycelium_color, a)
        };
        wither(col, g.wilt * 0.5)
    });

    let mut drawn = Drawn { units: units.max(1), ..Drawn::default() };
    // Nothing fruits through the compost before it is cased.
    let Some(gf) = fruit_growth(t, fr.pin_at).filter(|_| !g.dead && cased) else { return drawn };
    let mut rng = Rng::new(seed ^ 0xBED5);
    let mut n = fr.per_unit.saturating_mul(units.max(1));
    if vertex_budget > 0 {
        let mut probe = PlantMeshBuilder::new();
        mushroom_on_ground(&mut probe, fr, Vec3::ZERO, Vec3::Y, fr.cap_diameter_m[1], 1.0, 0.02, 0.005, 0.0);
        let per = probe.vertices.len().max(1);
        let left = (vertex_budget as usize).saturating_sub(b.vertices.len() - start);
        n = n.min(u32::try_from(left / per).unwrap_or(u32::MAX));
    }
    for (x, z, d, gi) in scatter(&mut rng, [iw, id], n, fr, gf) {
        let (len, rs) = stipe(fr, &mut rng, gi);
        let tilt = Vec3::new(rng.range(-0.12, 0.12), 1.0, rng.range(-0.12, 0.12)).normalize();
        let axis = rot * tilt;
        // The foot a little under the casing: it comes up through it.
        let ground = world(Vec3::new(x, height(x, z), z)) - axis * 0.005;
        mushroom_on_ground(b, fr, ground, axis, d, gi, len + 0.005, rs, g.wilt);
        drawn.add_cap(d);
    }
    drawn
}

/// One mushroom standing on the ground at `ground`: a button for the button
/// habit, else a stem and an umbrella cap.
#[allow(clippy::too_many_arguments)]
fn mushroom_on_ground(b: &mut PlantMeshBuilder, fr: &FungusFruit, ground: Vec3, axis: Vec3, d: f32, gi: f32, len: f32, rs: f32, wilt: f32) {
    b.set_organ(cap_tissue(fr));
    match fr.habit {
        FruitHabit::Buttons => button(b, fr, ground, axis, d, gi, len, rs, wilt),
        FruitHabit::Scattered | FruitHabit::ShelfCluster => {
            let top = ground + axis * len;
            b.tube(ground.to_array(), top.to_array(), rs * 1.1, rs, 6, wither(fr.stipe_color, wilt));
            let mut rng = Rng::new(mix64(ground.x.to_bits() as u64 ^ ((ground.z.to_bits() as u64) << 32)));
            umbrella(b, fr, top, axis, d, gi, rs, wilt, &mut rng);
        }
    }
}

// ── Mesh helpers ──────────────────────────────────────────────────────────

/// Draw a grid of points (`rows` x `cols`, row-major, world space) as quads of
/// one colour each (`color(row, col)` for the quad whose first corner that
/// is), smooth-shaded from the grid's own shape. `out(row, col)` points
/// roughly outward at a point: the normals and the winding follow it, so the
/// back-face cull keeps the side it names. `wrap` joins the last column to the
/// first (a surface of revolution). Triangles of no area (a dome's apex row)
/// are skipped.
fn emit_grid(
    b: &mut PlantMeshBuilder,
    pts: &[Vec3],
    rows: usize,
    cols: usize,
    wrap: bool,
    out: &dyn Fn(usize, usize) -> Vec3,
    color: &mut dyn FnMut(usize, usize) -> [f32; 3],
) {
    emit_grid_at(b, pts, rows, cols, wrap, out, &mut |r, c, _| color(r, c));
}

/// [`emit_grid`], each triangle coloured by `color(row, col, centroid)`, its
/// centre in world space: a pattern (the mycelium's patches) is then as fine
/// as the triangles, not the grid's squares.
fn emit_grid_at(
    b: &mut PlantMeshBuilder,
    pts: &[Vec3],
    rows: usize,
    cols: usize,
    wrap: bool,
    out: &dyn Fn(usize, usize) -> Vec3,
    color: &mut dyn FnMut(usize, usize, Vec3) -> [f32; 3],
) {
    if rows < 2 || cols < 2 || pts.len() < rows * cols {
        return;
    }
    let at = |r: usize, c: usize| pts[r * cols + c];
    let col = |c: isize| -> usize {
        if wrap {
            c.rem_euclid(cols as isize) as usize
        } else {
            c.clamp(0, cols as isize - 1) as usize
        }
    };
    let mut normals = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let du = at(r, col(c as isize + 1)) - at(r, col(c as isize - 1));
            let dv = at((r + 1).min(rows - 1), c) - at(r.saturating_sub(1), c);
            let hint = out(r, c);
            let mut n = du.cross(dv);
            if n.length_squared() < 1e-14 {
                n = hint;
            }
            if n.dot(hint) < 0.0 {
                n = -n;
            }
            normals.push(n.normalize_or_zero());
        }
    }
    let span = if wrap { cols } else { cols - 1 };
    for r in 0..rows - 1 {
        for c in 0..span {
            let c1 = (c + 1) % cols;
            let idx = [(r, c), (r, c1), (r + 1, c1), (r + 1, c)];
            let p = idx.map(|(r, c)| at(r, c));
            let n = idx.map(|(r, c)| normals[r * cols + c]);
            let hint = out(r, c) + out(r + 1, c1);
            for (i0, i1, i2) in [(0, 1, 2), (0, 2, 3)] {
                let face = (p[i1] - p[i0]).cross(p[i2] - p[i0]);
                if face.length_squared() < 1e-18 {
                    continue;
                }
                let (j1, j2) = if face.dot(hint) >= 0.0 { (i1, i2) } else { (i2, i1) };
                let shade = color(r, c, (p[i0] + p[i1] + p[i2]) / 3.0);
                b.tri_smooth(
                    [p[i0].to_array(), p[j1].to_array(), p[j2].to_array()],
                    [n[i0].to_array(), n[j1].to_array(), n[j2].to_array()],
                    shade,
                );
            }
        }
    }
}

/// One flat quad facing `out` (the corners in either turning order).
fn quad(b: &mut PlantMeshBuilder, p: [Vec3; 4], out: Vec3, color: [f32; 3]) {
    let facing = (p[1] - p[0]).cross(p[2] - p[0]);
    let p = if facing.dot(out) >= 0.0 { p } else { [p[0], p[3], p[2], p[1]] };
    b.tri(p[0].to_array(), p[1].to_array(), p[2].to_array(), color);
    b.tri(p[0].to_array(), p[2].to_array(), p[3].to_array(), color);
}

/// A box from `lo` to `hi` in a local frame (`world` maps it, `rot` turns
/// directions), its bottom left off: it stands on a shelf.
fn flat_box(b: &mut PlantMeshBuilder, lo: Vec3, hi: Vec3, world: &dyn Fn(Vec3) -> Vec3, rot: Quat, color: [f32; 3]) {
    for (axis, sign) in [(0usize, 1.0f32), (0, -1.0), (1, 1.0), (2, 1.0), (2, -1.0)] {
        let (u, v) = tangents(axis);
        let corner = |cu: bool, cv: bool| {
            let mut p = Vec3::ZERO;
            p[axis] = if sign > 0.0 { hi[axis] } else { lo[axis] };
            p[u] = if cu { hi[u] } else { lo[u] };
            p[v] = if cv { hi[v] } else { lo[v] };
            world(p)
        };
        let mut n = Vec3::ZERO;
        n[axis] = sign;
        quad(b, [corner(false, false), corner(true, false), corner(true, true), corner(false, true)], rot * n, color);
    }
}

/// A small lumpy ball (the knot an oyster cluster's stems grow from).
fn blob(b: &mut PlantMeshBuilder, c: Vec3, radii: Vec3, color: [f32; 3]) {
    const ROWS: usize = 5;
    const SEG: usize = 7;
    let pts: Vec<Vec3> = (0..ROWS)
        .flat_map(|r| (0..SEG).map(move |k| (r, k)))
        .map(|(r, k)| {
            let lat = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * r as f32 / (ROWS - 1) as f32;
            let lon = TAU * k as f32 / SEG as f32;
            c + Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()) * radii
        })
        .collect();
    emit_grid(b, &pts, ROWS, SEG, true, &|r, k| (pts[r * SEG + k] - c).normalize_or_zero(), &mut |_, _| color);
}

/// Two unit vectors square to `axis` and to each other.
fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let a = axis.cross(helper).normalize();
    (a, axis.cross(a).normalize())
}

/// `v` flattened onto the ground plane and made unit, or `fallback`.
fn horizontal(v: Vec3, fallback: Vec3) -> Vec3 {
    let h = Vec3::new(v.x, 0.0, v.z);
    if h.length_squared() > 1e-6 {
        h.normalize()
    } else {
        fallback
    }
}

// ── Small maths ───────────────────────────────────────────────────────────

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}
fn scale3(c: [f32; 3], k: f32) -> [f32; 3] {
    [(c[0] * k).clamp(0.0, 1.0), (c[1] * k).clamp(0.0, 1.0), (c[2] * k).clamp(0.0, 1.0)]
}
/// Pull a colour toward the dull brown of a spent crop as `w` rises.
fn wither(c: [f32; 3], w: f32) -> [f32; 3] {
    lerp3(c, [0.42, 0.34, 0.24], w.clamp(0.0, 1.0))
}
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Value noise in [0, 1].
fn noise01(p: Vec3, seed: u64) -> f32 {
    (noise3(p, seed) * 0.5 + 0.5).clamp(0.0, 1.0)
}

/// Where on a surface a spreading growth (the mycelium through a block, a
/// shiitake block's coat) arrives early or late, 0 to 1, at a point `l` in
/// the unit's own frame: patches a few centimetres across with finer ones in
/// them, stretched to use the whole range so the growth covers the surface
/// evenly over its course.
fn patches(l: Vec3, seed: u64) -> f32 {
    let p = noise01(l / 0.05, seed) * 0.7 + noise01(l / 0.018, seed ^ 0x51) * 0.3;
    ((p - 0.5) * 2.4 + 0.5).clamp(0.0, 1.0)
}

/// How far a growth that is `progress` of the way through (0 to 1) has come
/// at a place that it reaches at `when` (from [`patches`]): nothing at the
/// start, everything by the end, soft at the edge of each patch.
fn spreading(progress: f32, when: f32) -> f32 {
    let front = progress * 1.3 - 0.15;
    ((front - (when - 0.12)) / 0.24).clamp(0.0, 1.0)
}

/// splitmix64's finaliser: a stable, well-mixed 64-bit hash.
fn mix64(z: u64) -> u64 {
    let mut z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
/// A value in [0, 1) from a hash's top 24 bits.
fn unit01(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// A small deterministic random stream.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(mix64(seed))
    }
    fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        unit01(mix64(self.0))
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }
}

/// Value noise in [-1, 1] on a unit lattice, smoothly interpolated.
fn noise3(p: Vec3, seed: u64) -> f32 {
    let f = p.floor();
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - 2.0 * t);
    let (x, y, z) = (f.x as i64, f.y as i64, f.z as i64);
    let corner = |dx: i64, dy: i64, dz: i64| {
        let h = mix64(
            seed ^ ((x + dx) as u64).wrapping_mul(0x8CB9_2BA7_2F3D_8DD7)
                ^ ((y + dy) as u64).wrapping_mul(0x9E6C_63D0_676A_9A99)
                ^ ((z + dz) as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
        );
        unit01(h) * 2.0 - 1.0
    };
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let w = (if dx == 1 { s.x } else { 1.0 - s.x })
                    * (if dy == 1 { s.y } else { 1.0 - s.y })
                    * (if dz == 1 { s.z } else { 1.0 - s.z });
                acc += w * corner(dx, dy, dz);
            }
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::plant_mesh::PlantVisualRegistry;

    /// The shipped fungi, so the tests draw what the game draws.
    fn shipped() -> PlantVisualRegistry {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(root.join("data/plants_visual.ron")).expect("plants_visual.ron");
        PlantVisualRegistry::from_ron(&text).expect("plants_visual.ron parses")
    }

    fn grown(t: f32) -> Growth {
        Growth { t, wilt: 0.0, dead: false }
    }

    /// One unit of `id` (a block; for a bed, a plot of seven square feet) at
    /// `base` turned `yaw`, and what it drew.
    fn unit(id: &str, t: f32, base: Vec3, yaw: f32, seed: u64) -> (PlantMeshBuilder, Drawn) {
        let reg = shipped();
        let def = reg.fungus(id).unwrap_or_else(|| panic!("{id} is not a fungus"));
        let mut b = PlantMeshBuilder::new();
        let d = match BlockSpec::of(&def.substrate) {
            Some(spec) => draw_block(&mut b, &spec, &def.fruit, base, Quat::from_rotation_y(yaw), grown(t), seed),
            None => build_plot(&mut b, def, base, yaw, [1.2, 0.6], 7, grown(t), seed, 128, 0),
        };
        (b, d)
    }

    fn block_size(id: &str) -> Vec3 {
        match &shipped().fungus(id).expect("a fungus").substrate {
            FungusSubstrate::Block { size_m, .. } => Vec3::from_array(*size_m),
            FungusSubstrate::Bed { .. } => panic!("{id} is a bed"),
        }
    }

    /// Every triangle turns the way its vertices' normals say, so the
    /// back-face cull keeps its outside. Triangles too small to have a
    /// direction are passed.
    fn wound_outward(b: &PlantMeshBuilder) -> bool {
        b.indices.chunks(3).all(|t| {
            let p = |i: u32| Vec3::from(b.vertices[i as usize].position);
            let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            let said: Vec3 = t.iter().map(|&i| Vec3::from(b.vertices[i as usize].normal)).sum();
            face.length_squared() < 1e-16 || face.dot(said) > 0.0
        })
    }

    /// Positions from vertex `from` on, in a unit's own frame (its base at
    /// the origin, unturned).
    fn local(b: &PlantMeshBuilder, from: usize, base: Vec3, yaw: f32) -> Vec<Vec3> {
        let inv = Quat::from_rotation_y(yaw).inverse();
        b.vertices[from..].iter().map(|v| inv * (Vec3::from(v.position) - base)).collect()
    }

    /// A block is the data's size, stands on its shelf (nothing below its
    /// foot), and every face of it is turned out, at every stage and for both
    /// block species.
    #[test]
    fn fungus_block_is_the_data_size_and_stands_on_its_shelf() {
        let (base, yaw) = (Vec3::new(1.0, 0.5, -2.0), 0.4);
        for id in ["oyster_mushroom", "shiitake"] {
            let size = block_size(id);
            for t in [0.2, 0.4, 0.6, 0.8, 1.0] {
                let (b, d) = unit(id, t, base, yaw, 7);
                assert_eq!(d.units, 1);
                assert!(wound_outward(&b), "{id} at {t}: a face is turned inside out");
                let pts = local(&b, 0, base, yaw);
                let lo = pts.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
                assert!(lo.y > -1e-4, "{id} at {t}: something goes {} m through the shelf", -lo.y);
            }
            // The substrate alone (before any fruit) spans the block's size,
            // give or take its lumps and the bag's fold on top.
            let (b, _) = unit(id, 0.4, base, yaw, 7);
            let pts = local(&b, 0, base, yaw);
            let lo = pts.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
            let hi = pts.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(*p));
            let span = hi - lo;
            for (axis, name) in [(0, "width"), (1, "height"), (2, "depth")] {
                assert!(
                    (span[axis] - size[axis]).abs() < 0.02,
                    "{id}: drawn {name} {} m, the block is {} m",
                    span[axis],
                    size[axis]
                );
            }
        }
    }

    /// The fruit follows the stage: nothing while the substrate colonises,
    /// pins at the pinning stage, caps growing through the fruiting stage to
    /// the flush's size at harvest; a dead crop carries none.
    ///
    /// Seen red on 2026-09-27 by letting `fruit_growth` start a whole growth
    /// early: oyster mushrooms at 0.2, before the pins.
    #[test]
    fn fungus_fruit_follows_the_crop_stage() {
        let reg = shipped();
        for id in ["oyster_mushroom", "shiitake", "button_mushroom"] {
            let def = reg.fungus(id).expect("a fungus");
            let fr = &def.fruit;
            let [dmin, dmax] = fr.cap_diameter_m;
            for t in [0.2, 0.4] {
                let (_, d) = unit(id, t, Vec3::ZERO, 0.0, 3);
                assert_eq!(d.caps, 0, "{id}: mushrooms at growth {t}, before the pins");
            }
            let (_, pins) = unit(id, 0.6, Vec3::ZERO, 0.0, 3);
            assert!(pins.caps > 0, "{id}: no pins at the pinning stage");
            assert!(pins.widest_cap_m < 0.35 * dmax, "{id}: pins {} m across", pins.widest_cap_m);
            let (_, young) = unit(id, 0.8, Vec3::ZERO, 0.0, 3);
            let (_, ripe) = unit(id, 1.0, Vec3::ZERO, 0.0, 3);
            assert!(
                pins.widest_cap_m < young.widest_cap_m && young.widest_cap_m < ripe.widest_cap_m,
                "{id}: caps do not grow through the stages: {} {} {}",
                pins.widest_cap_m,
                young.widest_cap_m,
                ripe.widest_cap_m
            );
            assert!(
                (0.85 * dmin..=dmax + 1e-6).contains(&ripe.widest_cap_m),
                "{id}: the widest ripe cap is {} m, the data says {dmin} to {dmax}",
                ripe.widest_cap_m
            );
            let per_plot = if id == "button_mushroom" { fr.per_unit * 7 } else { fr.per_unit };
            assert!(ripe.caps * 10 >= per_plot * 7, "{id}: {} of {per_plot} mushrooms found room", ripe.caps);
            let mut b = PlantMeshBuilder::new();
            let dead = Growth { t: 1.0, wilt: 1.0, dead: true };
            let d = build_plot(&mut b, def, Vec3::ZERO, 0.0, [1.2, 0.6], 2, dead, 3, 128, 0);
            assert_eq!(d.caps, 0, "{id}: a dead crop still carries mushrooms");
        }
    }

    /// Where each species fruits: an oyster cluster comes out of the cut in
    /// the bag's front face and stays out in front of it; a shiitake's caps
    /// stand on the bare block and never inside it.
    #[test]
    fn fungus_fruit_grows_where_the_species_does() {
        let (base, yaw) = (Vec3::new(0.3, 1.0, 0.2), -0.7);
        for id in ["oyster_mushroom", "shiitake"] {
            let half = block_size(id) * 0.5;
            // At 0.5 the substrate is as it is at harvest and bears nothing,
            // so everything past its vertices is fruit.
            let (bare, d) = unit(id, 0.5, base, yaw, 11);
            assert_eq!(d.caps, 0);
            let (ripe, d) = unit(id, 1.0, base, yaw, 11);
            assert!(d.caps > 0);
            let fruit = local(&ripe, bare.vertices.len(), base, yaw);
            assert!(!fruit.is_empty(), "{id}: no fruit geometry");
            // A stem's foot and the cluster's knot are sunk a little into the
            // substrate they grow from; nothing reaches further in than that.
            for p in &fruit {
                let inside = p.x.abs() < half.x - 0.015 && p.z.abs() < half.z - 0.015 && p.y < 2.0 * half.y - 0.015;
                assert!(!inside, "{id}: fruit inside the block at {p}");
                assert!(p.y > -1e-4, "{id}: fruit below the shelf at {p}");
            }
            if id == "oyster_mushroom" {
                let front = fruit.iter().filter(|p| p.z > half.z - 0.01).count();
                assert!(front * 10 >= fruit.len() * 9, "{id}: only {front} of {} fruit vertices out front", fruit.len());
            }
        }
    }

    /// A bed is as big as the square feet its plot holds, stays clear of the
    /// rack's corner uprights, and its buttons stand on the casing inside the
    /// tray.
    #[test]
    fn fungus_bed_covers_its_units_and_its_buttons_stand_on_the_casing() {
        let side = 0.3048;
        for (units, plot) in [(7u32, [1.2f32, 0.6]), (1, [1.2, 0.6]), (3, [1.2, 0.6]), (40, [1.2, 0.6])] {
            let [bw, bd] = bed_size(side, units, plot);
            let want = units as f32 * side * side;
            let room = (plot[0] - 2.0 * BED_MARGIN_M[0]) * (plot[1] - 2.0 * BED_MARGIN_M[1]);
            assert!(bw * bd <= want * 1.001 && bw * bd >= want.min(room) * 0.97, "{units} units: {bw} x {bd}");
            // The upright in the plot's corner, and the tray's corner.
            let post = [plot[0] * 0.5 - SHELF_POST_R, plot[1] * 0.5 - SHELF_POST_R];
            let corner = [bw * 0.5, bd * 0.5];
            let gap = ((post[0] - corner[0]).max(0.0).powi(2) + (post[1] - corner[1]).max(0.0).powi(2)).sqrt();
            let clear = corner[0] < post[0] - SHELF_POST_R || corner[1] < post[1] - SHELF_POST_R || gap > SHELF_POST_R;
            assert!(clear, "{units} units: the tray's corner {corner:?} is in the upright at {post:?}");
        }
        let reg = shipped();
        let def = reg.fungus("button_mushroom").expect("button");
        let FungusSubstrate::Bed { compost_depth_m, casing_depth_m, .. } = def.substrate else { panic!("a bed") };
        let (bare, _) = unit("button_mushroom", 0.5, Vec3::ZERO, 0.0, 5);
        let (ripe, d) = unit("button_mushroom", 1.0, Vec3::ZERO, 0.0, 5);
        assert!(wound_outward(&ripe), "a face of the bed is turned inside out");
        assert!(d.caps > 0);
        let [bw, bd] = bed_size(side, 7, [1.2, 0.6]);
        let surface = compost_depth_m + casing_depth_m;
        for p in &local(&ripe, bare.vertices.len(), Vec3::ZERO, 0.0) {
            assert!(p.x.abs() < bw * 0.5 && p.z.abs() < bd * 0.5, "a button outside its tray: {p}");
            assert!(p.y > surface - 0.012, "a button below the casing: {p}");
        }
    }

    /// The same seed draws the same plot, bit for bit; another seed moves it.
    #[test]
    fn fungus_geometry_is_stable_for_a_seed() {
        let bits = |m: &PlantMeshBuilder| m.vertices.iter().flat_map(|v| v.position).map(f32::to_bits).collect::<Vec<_>>();
        for id in ["oyster_mushroom", "shiitake", "button_mushroom"] {
            let (a, _) = unit(id, 1.0, Vec3::ZERO, 0.0, 21);
            let (b, _) = unit(id, 1.0, Vec3::ZERO, 0.0, 21);
            let (c, _) = unit(id, 1.0, Vec3::ZERO, 0.0, 22);
            assert_eq!(bits(&a), bits(&b), "{id}: a rebuild moved something");
            assert_ne!(bits(&a), bits(&c), "{id}: the seed changes nothing");
        }
    }

    /// A plot never draws more vertices than its budget: past it, fewer
    /// blocks or buttons, never bigger ones.
    #[test]
    fn fungus_plot_keeps_inside_its_vertex_budget() {
        let reg = shipped();
        let oyster = reg.fungus("oyster_mushroom").expect("oyster");
        let mut one = PlantMeshBuilder::new();
        build_plot(&mut one, oyster, Vec3::ZERO, 0.0, [1.2, 0.6], 1, grown(1.0), 9, 128, 0);
        let per = one.vertices.len() as u32;
        let mut b = PlantMeshBuilder::new();
        let d = build_plot(&mut b, oyster, Vec3::ZERO, 0.0, [1.2, 0.6], 6, grown(1.0), 9, 128, per * 2 + per / 2);
        assert_eq!(d.units, 2, "two blocks fit a budget of two and a half");
        let button = reg.fungus("button_mushroom").expect("button");
        let mut all = PlantMeshBuilder::new();
        let full = build_plot(&mut all, button, Vec3::ZERO, 0.0, [1.2, 0.6], 7, grown(1.0), 9, 128, 0);
        let budget = (all.vertices.len() / 2) as u32;
        let mut half = PlantMeshBuilder::new();
        let cut = build_plot(&mut half, button, Vec3::ZERO, 0.0, [1.2, 0.6], 7, grown(1.0), 9, 128, budget);
        assert!(half.vertices.len() as u32 <= budget, "{} vertices over a budget of {budget}", half.vertices.len());
        assert!(cut.caps < full.caps && cut.caps > 0, "{} of {} buttons", cut.caps, full.caps);
        assert!(cut.widest_cap_m <= full.widest_cap_m + 1e-6, "a budget made the buttons bigger");
    }
}
