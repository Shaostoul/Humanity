//! Procedural plant mesh generation (v0.862, 2026-07-16).
//!
//! Operator ask: "It would be cool if the data we provide for the plant some
//! how generates the 3D model" - so this module turns a small set of numbers
//! in `data/plants_visual.ron` into a full plant mesh, parameterized by a
//! continuous growth value `t` (0 = just planted, 1 = mature) plus a `wilt`
//! value (0 = healthy, 1 = dying). No 3D model files anywhere: stems are
//! generalized cylinders, leaves are folded diamond fans, flowers are petal
//! fans, fruits are low-poly spheres/cones - all math.
//!
//! COLOR: the PBR vertex format has no color channel, so we use the SAME
//! trick as planet surfaces (material type 12): every face is flat-shaded
//! and its RGB is packed into the UV channel by `pack_color_to_uv`. All
//! three corners of a face carry the identical UV so interpolation cannot
//! corrupt the packed integer. One merged mesh per tower therefore carries
//! every plant in every color in a single draw call (MAX_OBJECTS safety).
//!
//! DETERMINISM: every plant takes a `seed`; the tiny xorshift RNG below
//! gives it stable per-plant variation (lean, leaf jitter) so 50 slots do
//! not look cloned, and the same slot always regrows the same character.
//!
//! Archetypes (the `form` field) cover the operator's requested species:
//!   rosette   - strawberry, dandelion, carrot (leafy crown, runners/berries)
//!   herb      - amaranth (upright stem, plume head)
//!   vine      - tomato (leaning stem, fruit clusters)
//!   tree      - apple / orange / lemon (trunk, branch whorls, crown fruit)
//!   bulb      - garlic (strap leaves from a bulb)
//!   bromeliad - pineapple (stiff rosette, central crowned fruit)
//! Unknown forms fall back to `rosette` so a new species always renders.

use crate::renderer::mesh::Vertex;
use crate::terrain::planet_surface::pack_color_to_uv;
use serde::Deserialize;
use std::collections::HashMap;

// ── Data definitions (deserialized from data/plants_visual.ron) ──────────

#[derive(Debug, Clone, Deserialize)]
pub struct PlantVisualDef {
    /// Archetype selector: rosette | herb | vine | tree | bulb | bromeliad.
    pub form: String,
    /// Mature plant height in metres (leaf-tip to base for rosettes).
    pub height_m: f32,
    /// Mature spread (leaf span diameter) in metres.
    pub spread_m: f32,
    /// Stem/trunk radius in metres at the base, at maturity.
    pub stem_radius: f32,
    pub stem_color: [f32; 3],
    /// Leaf count at maturity (rosette leaves, or leaves per branch level).
    pub leaf_count: u32,
    pub leaf_color: [f32; 3],
    /// Leaf droop below horizontal, degrees (wilt adds more).
    pub leaf_droop_deg: f32,
    /// Petal count; 0 = never flowers visibly.
    pub petal_count: u32,
    pub flower_color: [f32; 3],
    pub flower_center_color: [f32; 3],
    /// Growth fraction where flowers appear / where fruit replaces them.
    pub flower_at: f32,
    pub fruit_at: f32,
    /// Fruit shape: "sphere" | "cone" | "none". Cone points down (strawberry).
    pub fruit_kind: String,
    /// Fruit diameter in metres at full ripeness.
    pub fruit_size: f32,
    pub fruit_count: u32,
    pub fruit_color_unripe: [f32; 3],
    pub fruit_color_ripe: [f32; 3],
    /// Aeroponic net-cup roots hanging below the plant base.
    pub show_roots: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PlantVisualRegistry {
    pub plants: HashMap<String, PlantVisualDef>,
}

impl PlantVisualRegistry {
    /// Load from the given RON text (the caller decides file vs embedded).
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }
    pub fn get(&self, id: &str) -> Option<&PlantVisualDef> {
        self.plants.get(id)
    }
}

// ── Tiny deterministic RNG (xorshift64*) ─────────────────────────────────

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15).max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// Uniform in [lo, hi).
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() >> 40) as f32 / ((1u64 << 24) as f32) * (hi - lo)
    }
}

// ── Mesh assembly ─────────────────────────────────────────────────────────

/// Accumulates flat-shaded triangles with packed per-face color.
pub struct PlantMeshBuilder {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl PlantMeshBuilder {
    pub fn new() -> Self {
        PlantMeshBuilder { vertices: Vec::new(), indices: Vec::new() }
    }

    /// Push one flat-shaded triangle. The face normal is computed from the
    /// winding; the packed color rides identically on all three corners
    /// (material type 12 contract - see module docs).
    fn tri(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], color: [f32; 3]) {
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        n = [n[0] / len, n[1] / len, n[2] / len];
        let uv = pack_color_to_uv(color, false);
        let base = self.vertices.len() as u32;
        for p in [a, b, c] {
            self.vertices.push(Vertex { position: p, normal: n, uv });
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// Double-sided triangle (leaves/petals must be visible from both sides
    /// because the opaque pipeline back-culls).
    fn tri2(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], color: [f32; 3]) {
        self.tri(a, b, c, color);
        self.tri(a, c, b, color);
    }

    /// Tapered tube from `from` to `to` with `sides` around (a stem segment).
    fn tube(&mut self, from: [f32; 3], to: [f32; 3], r0: f32, r1: f32, sides: u32, color: [f32; 3]) {
        let axis = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let alen = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt().max(1e-6);
        let ax = [axis[0] / alen, axis[1] / alen, axis[2] / alen];
        // Any perpendicular frame:
        let helper = if ax[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let side = norm(cross(ax, helper));
        let up = cross(side, ax);
        let n = sides.max(3);
        for i in 0..n {
            let a0 = (i as f32) / (n as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (n as f32) * std::f32::consts::TAU;
            let p = |ang: f32, at: [f32; 3], r: f32| {
                [
                    at[0] + (side[0] * ang.cos() + up[0] * ang.sin()) * r,
                    at[1] + (side[1] * ang.cos() + up[1] * ang.sin()) * r,
                    at[2] + (side[2] * ang.cos() + up[2] * ang.sin()) * r,
                ]
            };
            let (b0, b1) = (p(a0, from, r0), p(a1, from, r0));
            let (t0, t1) = (p(a0, to, r1), p(a1, to, r1));
            self.tri(b0, t0, t1, color);
            self.tri(b0, t1, b1, color);
        }
    }

    /// A leaf as a folded diamond: base -> two mid points (left/right of the
    /// midrib, folded up) -> tip. 4 double-sided triangles. `dir` is the
    /// midrib direction (unit-ish), `length`/`width` in metres.
    fn leaf(&mut self, base: [f32; 3], dir: [f32; 3], length: f32, width: f32, fold: f32, color: [f32; 3]) {
        let d = norm(dir);
        let side = norm(cross(d, [0.0, 1.0, 0.0]));
        let mid = [
            base[0] + d[0] * length * 0.5,
            base[1] + d[1] * length * 0.5 + fold * length * 0.12,
            base[2] + d[2] * length * 0.5,
        ];
        let tip = [base[0] + d[0] * length, base[1] + d[1] * length, base[2] + d[2] * length];
        let l = [mid[0] - side[0] * width * 0.5, mid[1] - fold * length * 0.06, mid[2] - side[2] * width * 0.5];
        let r = [mid[0] + side[0] * width * 0.5, mid[1] - fold * length * 0.06, mid[2] + side[2] * width * 0.5];
        self.tri2(base, l, mid, color);
        self.tri2(base, mid, r, color);
        self.tri2(l, tip, mid, color);
        self.tri2(mid, tip, r, color);
    }

    /// Low-poly fruit sphere (octahedron subdivided once = 32 faces).
    fn fruit_sphere(&mut self, center: [f32; 3], r: f32, squash: f32, color: [f32; 3]) {
        let ico = octa_sub1();
        for f in ico.chunks(3) {
            let p = |v: [f32; 3]| [center[0] + v[0] * r, center[1] + v[1] * r * squash, center[2] + v[2] * r];
            self.tri(p(f[0]), p(f[1]), p(f[2]), color);
        }
    }

    /// Downward-pointing cone (strawberry): rim circle at top, apex below.
    fn fruit_cone(&mut self, top: [f32; 3], r: f32, len: f32, sides: u32, color: [f32; 3]) {
        let apex = [top[0], top[1] - len, top[2]];
        let n = sides.max(4);
        for i in 0..n {
            let a0 = (i as f32) / (n as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (n as f32) * std::f32::consts::TAU;
            let p0 = [top[0] + a0.cos() * r, top[1], top[2] + a0.sin() * r];
            let p1 = [top[0] + a1.cos() * r, top[1], top[2] + a1.sin() * r];
            self.tri(p0, apex, p1, color);
            // top cap
            self.tri(p0, p1, top, color);
        }
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Octahedron subdivided once, on the unit sphere (32 faces). Cheap and
/// round enough for berry/apple-scale fruit.
fn octa_sub1() -> Vec<[f32; 3]> {
    let top = [0.0f32, 1.0, 0.0];
    let bot = [0.0f32, -1.0, 0.0];
    let eq = [
        [1.0f32, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    ];
    let mut out: Vec<[f32; 3]> = Vec::with_capacity(32 * 3);
    let mid = |a: [f32; 3], b: [f32; 3]| norm([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0]);
    let mut emit = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        let (ab, bc, ca) = (mid(a, b), mid(b, c), mid(c, a));
        out.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
    };
    for i in 0..4 {
        let (e0, e1) = (eq[i], eq[(i + 1) % 4]);
        emit(top, e0, e1);
        emit(bot, e1, e0);
    }
    out
}

// ── The generator ─────────────────────────────────────────────────────────

/// Wilt/senescence color shift: pull toward a dry brown as wilt rises.
fn wilted(c: [f32; 3], wilt: f32) -> [f32; 3] {
    lerp3(c, [0.45, 0.33, 0.15], wilt * 0.8)
}

/// Build one plant at `base` (its soil/net-cup point) straight into the
/// builder. `t` = growth 0..1, `wilt` = 0..1 (health inverse), `seed` gives
/// stable per-plant variation. Geometry is world-scale metres.
pub fn build_plant(
    b: &mut PlantMeshBuilder,
    def: &PlantVisualDef,
    base: [f32; 3],
    out_dir: [f32; 3],
    t: f32,
    wilt: f32,
    seed: u64,
) {
    let t = t.clamp(0.02, 1.0);
    let wilt = wilt.clamp(0.0, 1.0);
    let mut rng = Rng::new(seed);
    let leaf_c = wilted(def.leaf_color, wilt);
    let stem_c = wilted(def.stem_color, wilt);
    // Growth eases in fast then slows (plants bulk up early).
    let g = t.sqrt();
    let height = def.height_m * g;
    let spread = def.spread_m * g;
    let droop = (def.leaf_droop_deg + wilt * 45.0).to_radians();
    // Aeroponic roots: a few thin pale strands hanging under the base.
    if def.show_roots {
        for _ in 0..3 {
            let dx = rng.range(-0.02, 0.02);
            let dz = rng.range(-0.02, 0.02);
            let len = 0.08 + 0.1 * g;
            b.tube(base, [base[0] + dx, base[1] - len, base[2] + dz], 0.004, 0.001, 3, [0.85, 0.8, 0.7]);
        }
    }

    match def.form.as_str() {
        // ── Upright single stem with leaf whorls and a seed-plume top ──
        "herb" => {
            let lean = rng.range(-0.06, 0.06);
            let top = [base[0] + lean + out_dir[0] * 0.05, base[1] + height, base[2] + out_dir[2] * 0.05];
            b.tube(base, top, def.stem_radius * g, def.stem_radius * 0.4 * g, 5, stem_c);
            let leaves = ((def.leaf_count as f32) * g).ceil() as u32;
            for i in 0..leaves {
                let frac = (i as f32 + 1.0) / (leaves as f32 + 1.0);
                let at = lerp3(base, top, frac);
                let ang = i as f32 * 2.39996 + rng.range(-0.2, 0.2); // golden angle
                let dir = [ang.cos(), -droop.sin(), ang.sin()];
                b.leaf(at, dir, spread * 0.4 * (1.0 - frac * 0.5), spread * 0.16, 0.8, leaf_c);
            }
            // Amaranth-style plume once flowering: a warm cone of color up top.
            if t >= def.flower_at && def.petal_count > 0 {
                let plume_h = height * 0.25;
                b.tube(top, [top[0], top[1] + plume_h, top[2]], def.stem_radius * 2.5 * g, 0.004, 5, wilted(def.flower_color, wilt));
            }
        }
        // ── Leaning vine: stem arcs outward, leaves along it, fruit clusters ──
        "vine" => {
            let segs = 4;
            let mut prev = base;
            for s in 0..segs {
                let frac = (s as f32 + 1.0) / segs as f32;
                let sag = (frac * frac) * 0.25 * height * (0.4 + wilt);
                let next = [
                    base[0] + out_dir[0] * spread * 0.5 * frac + rng.range(-0.02, 0.02),
                    base[1] + height * frac - sag,
                    base[2] + out_dir[2] * spread * 0.5 * frac + rng.range(-0.02, 0.02),
                ];
                b.tube(prev, next, def.stem_radius * g * (1.0 - frac * 0.5), def.stem_radius * g * (1.0 - frac * 0.6), 4, stem_c);
                let ang = rng.range(0.0, std::f32::consts::TAU);
                b.leaf(next, [ang.cos(), -droop.sin() * 0.5, ang.sin()], spread * 0.3, spread * 0.14, 0.6, leaf_c);
                // Fruit cluster at mid segments once fruiting.
                if t >= def.fruit_at && s >= 1 && def.fruit_kind != "none" {
                    let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                    let fc = wilted(lerp3(def.fruit_color_unripe, def.fruit_color_ripe, ripeness), wilt * 0.5);
                    let fr = def.fruit_size * 0.5 * (0.4 + 0.6 * ripeness);
                    for k in 0..2u32 {
                        let off = [rng.range(-0.05, 0.05), -0.04 - 0.03 * k as f32, rng.range(-0.05, 0.05)];
                        b.fruit_sphere([next[0] + off[0], next[1] + off[1], next[2] + off[2]], fr, 0.9, fc);
                    }
                }
                prev = next;
            }
        }
        // ── Tree: trunk, 2 branch whorls, leaf crown, hanging fruit ──
        "tree" => {
            let trunk_top = [base[0], base[1] + height * 0.45, base[2]];
            b.tube(base, trunk_top, def.stem_radius * g, def.stem_radius * 0.6 * g, 6, stem_c);
            let branches = 5u32;
            for i in 0..branches {
                let ang = i as f32 / branches as f32 * std::f32::consts::TAU + rng.range(-0.2, 0.2);
                let tip = [
                    trunk_top[0] + ang.cos() * spread * 0.5,
                    trunk_top[1] + height * 0.35 - droop.sin() * 0.2,
                    trunk_top[2] + ang.sin() * spread * 0.5,
                ];
                b.tube(trunk_top, tip, def.stem_radius * 0.5 * g, 0.01 * g, 4, stem_c);
                // Leaf tufts along the branch tip.
                for k in 0..(def.leaf_count.max(2) / 2) {
                    let f = (k as f32 + 1.0) / ((def.leaf_count.max(2) / 2) as f32 + 1.0);
                    let at = lerp3(trunk_top, tip, f);
                    let la = rng.range(0.0, std::f32::consts::TAU);
                    b.leaf(at, [la.cos(), 0.2 - droop.sin(), la.sin()], spread * 0.2, spread * 0.1, 0.5, leaf_c);
                }
                if t >= def.fruit_at && def.fruit_kind != "none" && i < def.fruit_count {
                    let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                    let fc = lerp3(def.fruit_color_unripe, def.fruit_color_ripe, ripeness);
                    b.fruit_sphere([tip[0], tip[1] - def.fruit_size * 0.8, tip[2]], def.fruit_size * 0.5 * (0.4 + 0.6 * ripeness), 1.0, fc);
                }
            }
        }
        // ── Bulb (garlic): strap leaves fanning up from the base ──
        "bulb" => {
            let straps = ((def.leaf_count as f32) * g).ceil().max(2.0) as u32;
            for i in 0..straps {
                let ang = i as f32 / straps as f32 * std::f32::consts::TAU + rng.range(-0.15, 0.15);
                let bend = droop.sin() * 0.4 + rng.range(0.0, 0.15);
                let dir = [ang.cos() * (0.25 + bend), 1.0 - bend, ang.sin() * (0.25 + bend)];
                b.leaf(base, norm(dir), height, def.spread_m * 0.08, 0.2, leaf_c);
            }
            // Bulb hint at the base.
            b.fruit_sphere([base[0], base[1] + 0.015, base[2]], 0.02 + 0.02 * g, 0.8, [0.92, 0.9, 0.82]);
        }
        // ── Bromeliad (pineapple): stiff rosette + central crowned fruit ──
        "bromeliad" => {
            let straps = def.leaf_count.max(6);
            for i in 0..straps {
                let ang = i as f32 * 2.39996;
                let tilt = 0.5 + (i % 3) as f32 * 0.18 + droop.sin() * 0.3;
                let dir = norm([ang.cos() * tilt, 1.0 - tilt * 0.6, ang.sin() * tilt]);
                b.leaf(base, dir, spread * 0.55, spread * 0.09, 0.1, leaf_c);
            }
            if t >= def.fruit_at {
                let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                let fc = lerp3(def.fruit_color_unripe, def.fruit_color_ripe, ripeness);
                let fh = base[1] + height * 0.45;
                let stalk_top = [base[0], fh, base[2]];
                b.tube(base, stalk_top, 0.015, 0.012, 4, stem_c);
                b.fruit_sphere([base[0], fh + def.fruit_size * 0.5, base[2]], def.fruit_size * 0.5, 1.35, fc);
                // Crown of stiff leaflets on top of the fruit.
                for i in 0..5u32 {
                    let ang = i as f32 / 5.0 * std::f32::consts::TAU;
                    let ctop = [base[0], fh + def.fruit_size * 1.15, base[2]];
                    b.leaf(ctop, norm([ang.cos() * 0.4, 1.0, ang.sin() * 0.4]), def.fruit_size * 0.9, def.fruit_size * 0.2, 0.1, leaf_c);
                }
            }
        }
        // ── Rosette (default; strawberry/dandelion/carrot): leaf crown from
        //    the base, flowers on thin stalks, then hanging/held fruit. ──
        _ => {
            let leaves = ((def.leaf_count as f32) * (0.3 + 0.7 * g)).ceil().max(2.0) as u32;
            for i in 0..leaves {
                let ang = i as f32 * 2.39996 + rng.range(-0.15, 0.15); // golden angle spiral
                let up = 0.65 - droop.sin();
                let dir = norm([ang.cos(), up.max(-0.3), ang.sin()]);
                let llen = spread * 0.5 * rng.range(0.85, 1.1);
                // Petiole (thin stalk) then the leaf blade at its end.
                let pet_end = [
                    base[0] + dir[0] * llen * 0.4,
                    base[1] + dir[1] * llen * 0.4,
                    base[2] + dir[2] * llen * 0.4,
                ];
                b.tube(base, pet_end, 0.004 * (1.0 + g), 0.003, 3, stem_c);
                b.leaf(pet_end, dir, llen * 0.6, llen * 0.42, 0.7, leaf_c);
            }
            // Flowers: thin stalks rising from the crown with a petal fan.
            let flowering = t >= def.flower_at && t < def.fruit_at.max(def.flower_at + 0.01);
            if flowering && def.petal_count > 0 {
                for i in 0..2u32 {
                    let ang = rng.range(0.0, std::f32::consts::TAU);
                    let ftop = [
                        base[0] + ang.cos() * spread * 0.15,
                        base[1] + height * 0.9,
                        base[2] + ang.sin() * spread * 0.15,
                    ];
                    b.tube(base, ftop, 0.003, 0.002, 3, stem_c);
                    let _ = i;
                    // Petal fan (flat shading makes this read as a flower).
                    for p in 0..def.petal_count {
                        let pa = p as f32 / def.petal_count as f32 * std::f32::consts::TAU;
                        b.leaf(ftop, norm([pa.cos(), 0.25, pa.sin()]), def.fruit_size * 0.8, def.fruit_size * 0.45, 0.1, wilted(def.flower_color, wilt));
                    }
                    b.fruit_sphere(ftop, def.fruit_size * 0.14, 1.0, def.flower_center_color);
                }
            }
            // Fruit: strawberry cones hang below the crown on drooping stalks.
            if t >= def.fruit_at && def.fruit_kind != "none" {
                let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                let fc = lerp3(def.fruit_color_unripe, def.fruit_color_ripe, ripeness);
                let n = def.fruit_count.max(1);
                for i in 0..n {
                    let ang = i as f32 / n as f32 * std::f32::consts::TAU + rng.range(-0.3, 0.3);
                    let fx = [
                        base[0] + ang.cos() * spread * 0.35,
                        base[1] + height * 0.35,
                        base[2] + ang.sin() * spread * 0.35,
                    ];
                    b.tube([base[0], base[1] + height * 0.5, base[2]], fx, 0.003, 0.002, 3, stem_c);
                    let fs = def.fruit_size * (0.5 + 0.5 * ripeness);
                    if def.fruit_kind == "cone" {
                        b.fruit_cone(fx, fs * 0.45, fs, 6, fc);
                    } else {
                        b.fruit_sphere([fx[0], fx[1] - fs * 0.4, fx[2]], fs * 0.5, 1.0, fc);
                    }
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn strawberry() -> PlantVisualDef {
        PlantVisualDef {
            form: "rosette".into(),
            height_m: 0.22,
            spread_m: 0.35,
            stem_radius: 0.006,
            stem_color: [0.35, 0.5, 0.25],
            leaf_count: 8,
            leaf_color: [0.16, 0.45, 0.18],
            leaf_droop_deg: 15.0,
            petal_count: 5,
            flower_color: [0.95, 0.95, 0.9],
            flower_center_color: [0.95, 0.8, 0.2],
            flower_at: 0.55,
            fruit_at: 0.75,
            fruit_kind: "cone".into(),
            fruit_size: 0.035,
            fruit_count: 4,
            fruit_color_unripe: [0.55, 0.7, 0.4],
            fruit_color_ripe: [0.85, 0.12, 0.15],
            show_roots: true,
        }
    }

    #[test]
    fn builds_nonempty_indexed_geometry_at_every_growth_stage() {
        for &t in &[0.05f32, 0.3, 0.6, 0.8, 1.0] {
            let mut b = PlantMeshBuilder::new();
            build_plant(&mut b, &strawberry(), [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], t, 0.0, 42);
            assert!(!b.vertices.is_empty(), "t={t}: no vertices");
            assert_eq!(b.indices.len() % 3, 0);
            let max = *b.indices.iter().max().unwrap() as usize;
            assert!(max < b.vertices.len(), "t={t}: index out of bounds");
        }
    }

    #[test]
    fn growth_monotonically_adds_geometry() {
        let counts: Vec<usize> = [0.1f32, 0.5, 1.0]
            .iter()
            .map(|&t| {
                let mut b = PlantMeshBuilder::new();
                build_plant(&mut b, &strawberry(), [0.0; 3], [1.0, 0.0, 0.0], t, 0.0, 7);
                b.indices.len()
            })
            .collect();
        assert!(counts[0] <= counts[1] && counts[1] <= counts[2], "{counts:?}");
    }

    #[test]
    fn same_seed_is_deterministic_and_seeds_differ() {
        let gen = |seed: u64| {
            let mut b = PlantMeshBuilder::new();
            build_plant(&mut b, &strawberry(), [0.0; 3], [1.0, 0.0, 0.0], 0.9, 0.0, seed);
            (b.vertices.len(), b.vertices.get(3).map(|v| v.position))
        };
        assert_eq!(gen(5), gen(5));
        assert_ne!(gen(5).1, gen(6).1);
    }

    #[test]
    fn packed_face_colors_survive_the_type12_roundtrip() {
        let mut b = PlantMeshBuilder::new();
        build_plant(&mut b, &strawberry(), [0.0; 3], [1.0, 0.0, 0.0], 1.0, 0.0, 1);
        // Every face's 3 corners must share one UV (the flat-shading contract).
        for f in b.indices.chunks(3) {
            let uv0 = b.vertices[f[0] as usize].uv;
            assert_eq!(uv0, b.vertices[f[1] as usize].uv);
            assert_eq!(uv0, b.vertices[f[2] as usize].uv);
            let (c, water) = crate::terrain::planet_surface::unpack_uv_to_color(uv0);
            assert!(!water);
            assert!(c.iter().all(|&x| (0.0..=1.0).contains(&x)));
        }
    }

    #[test]
    fn registry_parses_the_shipped_ron() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/plants_visual.ron"),
        )
        .expect("data/plants_visual.ron exists");
        let reg = PlantVisualRegistry::from_ron(&text).expect("plants_visual.ron parses");
        for id in [
            "strawberry", "tomato", "amaranth", "apple", "orange", "lemon",
            "garlic", "carrot", "dandelion", "pineapple",
        ] {
            assert!(reg.get(id).is_some(), "missing visual def for {id}");
        }
    }
}
