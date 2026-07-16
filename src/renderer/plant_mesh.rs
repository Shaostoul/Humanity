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

    /// A leaf blade: 3 tapered segments along a bowing midrib with a V-fold
    /// cross-section (edges sit below the raised midrib), ending in a point.
    /// Reads as an actual leaf at close range instead of a diamond shard
    /// (operator close-up feedback, 2026-07-16). 16 double-sided triangles.
    /// `dir` is the midrib direction (unit-ish), `length`/`width` in metres,
    /// `fold` 0..1 controls how sharply the blade folds along the midrib.
    fn leaf(&mut self, base: [f32; 3], dir: [f32; 3], length: f32, width: f32, fold: f32, color: [f32; 3]) {
        let d = norm(dir);
        let side = norm(cross(d, [0.0, 1.0, 0.0]));
        let bow = length * 0.14; // tip bows down (gravity)
        let lift = (fold * length * 0.09).max(0.001); // midrib rides above the edges
        // Midrib station at fraction f (bow grows quadratically toward the tip).
        let st = |f: f32| {
            [
                base[0] + d[0] * length * f,
                base[1] + d[1] * length * f - bow * f * f + lift * (1.0 - (2.0 * f - 1.0).abs()),
                base[2] + d[2] * length * f,
            ]
        };
        // Edge point at station f offset s (+1 right / -1 left), width w.
        let edge = |m: [f32; 3], s: f32, w: f32| {
            [m[0] + side[0] * s * w, m[1] - lift, m[2] + side[2] * s * w]
        };
        let (m1, m2, tip) = (st(0.30), st(0.65), st(1.0));
        let (w1, w2) = (width * 0.5, width * 0.34);
        let (l1, r1) = (edge(m1, -1.0, w1), edge(m1, 1.0, w1));
        let (l2, r2) = (edge(m2, -1.0, w2), edge(m2, 1.0, w2));
        // Base wedge
        self.tri2(base, l1, m1, color);
        self.tri2(base, m1, r1, color);
        // Mid band
        self.tri2(l1, l2, m2, color);
        self.tri2(l1, m2, m1, color);
        self.tri2(r1, m1, m2, color);
        self.tri2(r1, m2, r2, color);
        // Tip taper
        self.tri2(l2, tip, m2, color);
        self.tri2(r2, m2, tip, color);
    }

    /// A small flat petal: single diamond, kept intentionally simple (flowers
    /// are tiny; the old full leaf() as a petal is what made blossoms read as
    /// giant pinwheels). 2 double-sided triangles.
    fn petal(&mut self, base: [f32; 3], dir: [f32; 3], length: f32, width: f32, color: [f32; 3]) {
        let d = norm(dir);
        let side = norm(cross(d, [0.0, 1.0, 0.0]));
        let mid = lerp3(base, [base[0] + d[0] * length, base[1] + d[1] * length, base[2] + d[2] * length], 0.55);
        let tip = [base[0] + d[0] * length, base[1] + d[1] * length, base[2] + d[2] * length];
        let l = [mid[0] - side[0] * width * 0.5, mid[1], mid[2] - side[2] * width * 0.5];
        let r = [mid[0] + side[0] * width * 0.5, mid[1], mid[2] + side[2] * width * 0.5];
        self.tri2(l, tip, base, color);
        self.tri2(r, base, tip, color);
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

/// Per-leaf brightness jitter so 50 plants of one species do not read as
/// clones (value-only: hue stays true to the species).
fn jit(rng: &mut Rng, c: [f32; 3]) -> [f32; 3] {
    let k = rng.range(0.88, 1.10);
    [(c[0] * k).min(1.0), (c[1] * k).min(1.0), (c[2] * k).min(1.0)]
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
                b.leaf(at, dir, spread * 0.4 * (1.0 - frac * 0.5), spread * 0.16, 0.8, jit(&mut rng, leaf_c));
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
                b.leaf(next, [ang.cos(), -droop.sin() * 0.5, ang.sin()], spread * 0.3, spread * 0.14, 0.6, jit(&mut rng, leaf_c));
                // Fruit truss at mid segments once fruiting: a short stalk
                // drops from the node and the fruits hang touching it (they
                // used to float beside the vine unattached).
                if t >= def.fruit_at && s >= 1 && def.fruit_kind != "none" {
                    let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                    let fr = def.fruit_size * 0.5 * (0.4 + 0.6 * ripeness);
                    let dx = rng.range(-0.03, 0.03);
                    let dz = rng.range(-0.03, 0.03);
                    let truss = [next[0] + dx, next[1] - 0.035, next[2] + dz];
                    b.tube(next, truss, 0.0028, 0.002, 3, stem_c);
                    for k in 0..2u32 {
                        let rp = (ripeness + rng.range(-0.3, 0.1)).clamp(0.0, 1.0);
                        let fc = wilted(lerp3(def.fruit_color_unripe, def.fruit_color_ripe, rp), wilt * 0.5);
                        let ka = rng.range(0.0, std::f32::consts::TAU);
                        b.fruit_sphere(
                            [truss[0] + ka.cos() * fr * 0.7, truss[1] - fr * (0.6 + 0.5 * k as f32), truss[2] + ka.sin() * fr * 0.7],
                            fr,
                            0.9,
                            fc,
                        );
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
                    b.leaf(at, [la.cos(), 0.2 - droop.sin(), la.sin()], spread * 0.2, spread * 0.1, 0.5, jit(&mut rng, leaf_c));
                }
                if t >= def.fruit_at && def.fruit_kind != "none" && i < def.fruit_count {
                    let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                    let fc = lerp3(def.fruit_color_unripe, def.fruit_color_ripe, ripeness);
                    let fs = def.fruit_size * 0.5 * (0.4 + 0.6 * ripeness);
                    // Fruit hangs from a short pedicel off the branch tip
                    // (was a sphere floating under the branch).
                    let stem_end = [tip[0], tip[1] - def.fruit_size * 0.45, tip[2]];
                    b.tube(tip, stem_end, 0.0025, 0.002, 3, stem_c);
                    b.fruit_sphere([stem_end[0], stem_end[1] - fs * 0.8, stem_end[2]], fs, 1.0, fc);
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
                b.leaf(base, norm(dir), height, def.spread_m * 0.08, 0.2, jit(&mut rng, leaf_c));
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
                b.leaf(base, dir, spread * 0.55, spread * 0.09, 0.1, jit(&mut rng, leaf_c));
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
        // ── Rosette (default; strawberry/dandelion/carrot): arcing petioles
        //    ending in TRIFOLIATE leaflet clusters, small true-scale blossoms,
        //    and berries hanging on drooping stalks with green calyxes. ──
        _ => {
            // Crown nub: the tiny green heart everything attaches to, so no
            // part of the plant ever reads as floating.
            b.fruit_sphere([base[0], base[1] + 0.008, base[2]], 0.012 + 0.01 * g, 0.7, stem_c);
            let leaves = ((def.leaf_count as f32) * (0.3 + 0.7 * g)).ceil().max(2.0) as u32;
            for i in 0..leaves {
                let ang = i as f32 * 2.39996 + rng.range(-0.15, 0.15); // golden angle spiral
                let up = (0.75 - droop.sin()).max(-0.25);
                let dir = norm([ang.cos(), up, ang.sin()]);
                let llen = spread * 0.5 * rng.range(0.85, 1.1);
                // Petiole arcs: rises steeply from the crown, then flattens
                // outward - two segments so the arc is visible.
                let knee = [
                    base[0] + dir[0] * llen * 0.22,
                    base[1] + (dir[1] * llen * 0.30).max(0.01),
                    base[2] + dir[2] * llen * 0.22,
                ];
                let pet_end = [
                    base[0] + dir[0] * llen * 0.55,
                    knee[1] + llen * 0.06 - droop.sin() * llen * 0.15,
                    base[2] + dir[2] * llen * 0.55,
                ];
                let pr = 0.0035 * (1.0 + g);
                b.tube(base, knee, pr, pr * 0.85, 3, stem_c);
                b.tube(knee, pet_end, pr * 0.85, pr * 0.6, 3, stem_c);
                // Trifoliate cluster: center leaflet along the petiole line,
                // two side leaflets splayed ~40 degrees, all slightly tilted
                // up toward the light like a real strawberry leaf.
                let blade = llen * 0.42;
                let lc = jit(&mut rng, leaf_c);
                let flat = norm([dir[0], 0.12 - droop.sin() * 0.5, dir[2]]);
                b.leaf(pet_end, flat, blade, blade * 0.6, 0.6, lc);
                for s in [-1.0f32, 1.0] {
                    let ra = 0.7 * s; // ~40 deg splay
                    let (sa, ca) = (ra.sin(), ra.cos());
                    let rot = [
                        flat[0] * ca - flat[2] * sa,
                        flat[1] * 0.9,
                        flat[0] * sa + flat[2] * ca,
                    ];
                    b.leaf(pet_end, norm(rot), blade * 0.85, blade * 0.5, 0.6, jit(&mut rng, leaf_c));
                }
            }
            // Blossoms: small true-scale flowers (a strawberry bloom is ~2 cm)
            // on short stalks leaning out of the crown. They linger a little
            // into fruiting, as real everbearers carry both at once.
            let flowering = t >= def.flower_at && t < (def.fruit_at + 0.2).min(1.01) && def.petal_count > 0;
            if flowering {
                for _ in 0..2u32 {
                    let ang = rng.range(0.0, std::f32::consts::TAU);
                    let lean = [ang.cos() * 0.35, 1.0, ang.sin() * 0.35];
                    let ftop = [
                        base[0] + lean[0] * height * 0.55,
                        base[1] + height * 0.62,
                        base[2] + lean[2] * height * 0.55,
                    ];
                    b.tube(base, ftop, 0.0025, 0.0018, 3, stem_c);
                    let psz = 0.022 + def.fruit_size * 0.25; // petal length, metres
                    for p in 0..def.petal_count.min(6) {
                        let pa = p as f32 / def.petal_count.min(6) as f32 * std::f32::consts::TAU;
                        b.petal(ftop, norm([pa.cos(), 0.18, pa.sin()]), psz, psz * 0.7, wilted(def.flower_color, wilt));
                    }
                    // Flower center: a tiny upward cone reads as the yellow eye
                    // without a whole sphere's triangle count.
                    b.fruit_cone([ftop[0], ftop[1] + 0.004, ftop[2]], psz * 0.22, 0.006, 4, def.flower_center_color);
                }
            }
            // Fruit: berries HANG below the canopy on arcing stalks (real
            // strawberries droop to the ground / out of the net cup), each
            // capped with a small green calyx so it connects visually.
            if t >= def.fruit_at && def.fruit_kind != "none" {
                let ripeness = ((t - def.fruit_at) / (1.0 - def.fruit_at).max(0.05)).clamp(0.0, 1.0);
                let n = def.fruit_count.max(1);
                for i in 0..n {
                    // Per-berry ripeness spread: the cluster ripens unevenly.
                    let rp = (ripeness + rng.range(-0.35, 0.15)).clamp(0.0, 1.0);
                    let fc = lerp3(def.fruit_color_unripe, def.fruit_color_ripe, rp);
                    let ang = i as f32 / n as f32 * std::f32::consts::TAU + rng.range(-0.3, 0.3);
                    let crown = [base[0], base[1] + height * 0.30, base[2]];
                    let elbow = [
                        base[0] + ang.cos() * spread * 0.30,
                        base[1] + height * 0.22,
                        base[2] + ang.sin() * spread * 0.30,
                    ];
                    let hang = [
                        base[0] + ang.cos() * spread * 0.38,
                        base[1] - 0.015 - 0.02 * rp,
                        base[2] + ang.sin() * spread * 0.38,
                    ];
                    b.tube(crown, elbow, 0.0028, 0.0022, 3, stem_c);
                    b.tube(elbow, hang, 0.0022, 0.0016, 3, stem_c);
                    let fs = def.fruit_size * (0.45 + 0.55 * rp);
                    if def.fruit_kind == "cone" {
                        b.fruit_cone(hang, fs * 0.42, fs, 6, fc);
                    } else {
                        b.fruit_sphere([hang[0], hang[1] - fs * 0.4, hang[2]], fs * 0.5, 1.0, fc);
                    }
                    // Calyx: three tiny green sepals draped over the berry top.
                    for s in 0..3u32 {
                        let sa = s as f32 / 3.0 * std::f32::consts::TAU;
                        b.petal(
                            [hang[0], hang[1] + 0.002, hang[2]],
                            norm([sa.cos(), -0.25, sa.sin()]),
                            fs * 0.5,
                            fs * 0.3,
                            wilted([0.30, 0.55, 0.25], wilt),
                        );
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
