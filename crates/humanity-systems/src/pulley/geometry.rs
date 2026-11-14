//! A block and tackle drawn from math alone (docs/design/pulleys.md, "The geometry"): every
//! dimension of a block comes from the rope it carries and the load it holds, and the mesh and
//! the collision shapes are built from those same numbers. No mesh file is read.
//!
//! THE BLOCK'S FRAME. The pin (axle) runs along X; the sheaves turn in the Y-Z plane; Y points
//! along the line of pull. The origin is the pin's centre. A block whose head points up (the fixed
//! block, hung from its shackle) has its shackle above; a moving block's head points down, to the
//! load. Lengths are metres, f64 while sizing, f32 in the mesh (a block is a few metres at most).
//!
//! THE SIZING (each rule's source is in data/pulleys/pulleys.ron and docs/design/pulleys.md):
//! - sheave tread diameter D = the rope's sheave ratio x d (twice the circumference for fibre
//!   rope, 20 d for wire rope); the flanges stand 1.5 d above the tread; a sheave is 1.8 d wide;
//! - the ultimate load the block is built for is its working load x the block design factor (4);
//! - the pin carries it in double shear at 0.6 of the material's tensile strength, and in bending
//!   across the sheave stack (a simply supported bar, M = F L / 8), whichever needs more;
//! - each side plate (cheek) carries half of it across the net section beside the pin hole, and in
//!   bearing at 1.5 times the tensile strength, whichever needs more;
//! - the shackle's two legs carry it with a factor of 2 for the bend of the bow.
//!
//! These are first-principles strength-of-materials rules (our derivation, stated in the design
//! doc), not a manufacturer's drawing: the point is that a block for 100 t is visibly a different
//! object from a block for 1 t, with every proportion following from the load.

use glam::Vec3;
use std::f64::consts::PI;

use super::data::{BlockRules, Material, Rope};

/// Shear strength as a share of tensile strength (our assumption, the usual engineering ratio
/// for steels).
const SHEAR_OF_TENSILE: f64 = 0.6;
/// Bearing strength at a pin hole as a multiple of tensile strength (our assumption).
const BEARING_OF_TENSILE: f64 = 1.5;
/// The thinnest plate a block is drawn with, metres.
const MIN_PLATE_M: f64 = 0.002;
/// Sides of every lathe and tube ring.
const RING_SIDES: u32 = 16;

/// Every dimension of one block, worked out from its rope and its load.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockDims {
    /// Sheaves on the pin (0 for the plain shackle that holds a one-part tackle's load).
    pub sheaves: u32,
    pub rope_d: f64,
    /// Tread (groove bottom) diameter.
    pub tread_d: f64,
    pub flange_d: f64,
    pub sheave_w: f64,
    pub pin_d: f64,
    pub cheek_t: f64,
    pub partition_t: f64,
    /// Running clearance between a sheave and a plate.
    pub gap: f64,
    /// Side plate extent across the sheave (Z).
    pub cheek_w: f64,
    /// Side plate extent along the pull (Y), the block's length.
    pub cheek_h: f64,
    /// Outside to outside along the pin (X).
    pub width: f64,
    pub shackle_d: f64,
    pub shackle_pin_d: f64,
    pub shackle_inner_w: f64,
    pub shackle_inner_h: f64,
    /// The working load the block is built for, newtons.
    pub wll_n: f64,
    pub mass_kg: f64,
}

impl BlockDims {
    /// Size a block of `sheaves` sheaves for rope `rope` at `rope_d_mm`, holding `wll_n`.
    pub fn design(sheaves: u32, rope: &Rope, rope_d_mm: f64, wll_n: f64, mat: Material, rules: &BlockRules) -> Self {
        let d = rope_d_mm / 1000.0;
        let sigma = mat.tensile_pa.max(1.0);
        let tau = SHEAR_OF_TENSILE * sigma;
        let f_u = wll_n.max(1.0) * rules.design_factor;
        // The shackle: two legs, a bend factor of 2: pi/4 d^2 sigma = F_u.
        let shackle_d = (4.0 * f_u / (PI * sigma)).sqrt().max(MIN_PLATE_M * 2.0);
        let shackle_pin_d = 1.15 * shackle_d;
        if sheaves == 0 {
            let inner_w = (2.0 * shackle_d).max(1.5 * d);
            let inner_h = 1.6 * inner_w;
            let mut b = BlockDims {
                sheaves: 0,
                rope_d: d,
                tread_d: 0.0,
                flange_d: 0.0,
                sheave_w: 0.0,
                pin_d: 0.0,
                cheek_t: 0.0,
                partition_t: 0.0,
                gap: 0.0,
                cheek_w: 0.0,
                cheek_h: 0.0,
                width: 0.0,
                shackle_d,
                shackle_pin_d,
                shackle_inner_w: inner_w,
                shackle_inner_h: inner_h,
                wll_n,
                mass_kg: 0.0,
            };
            b.mass_kg = b.volume_m3() * mat.density;
            return b;
        }
        let n = sheaves as f64;
        let tread_d = rope.sheave_ratio * d;
        let flange_d = tread_d + 3.0 * d;
        let sheave_w = 1.8 * d;
        let gap = 0.1 * d;
        // Pin: double shear, or bending over an estimate of the sheave stack's span.
        let span_est = n * sheave_w + (n - 1.0) * 0.3 * d + 2.0 * gap;
        let pin_shear = (2.0 * f_u / (PI * tau)).sqrt();
        let pin_bend = (32.0 * (f_u * span_est / 8.0) / (PI * sigma)).cbrt();
        let pin_d = pin_shear.max(pin_bend).max(0.25 * d);
        let cheek_w = flange_d + 2.0 * (0.1 * flange_d).max(d);
        let cheek_t = (f_u / (2.0 * sigma * (cheek_w - pin_d).max(pin_d)))
            .max(f_u / (2.0 * BEARING_OF_TENSILE * sigma * pin_d))
            .max(MIN_PLATE_M);
        let partition_t = (0.5 * cheek_t).max(MIN_PLATE_M);
        let stack = n * sheave_w + (n - 1.0) * (partition_t + 2.0 * gap);
        let width = stack + 2.0 * (cheek_t + gap);
        let inner_w = width + 2.0 * gap.max(0.25 * shackle_d);
        let inner_h = (1.5 * inner_w).max(3.0 * shackle_d);
        // Length: from the plates' far edge past the sheave to the head where the shackle pin goes.
        let geometric = cheek_w + 2.5 * shackle_pin_d;
        let cheek_h = geometric.max(rope.block_length_ratio * d);
        let mut b = BlockDims {
            sheaves,
            rope_d: d,
            tread_d,
            flange_d,
            sheave_w,
            pin_d,
            cheek_t,
            partition_t,
            gap,
            cheek_w,
            cheek_h,
            width,
            shackle_d,
            shackle_pin_d,
            shackle_inner_w: inner_w,
            shackle_inner_h: inner_h,
            wll_n,
            mass_kg: 0.0,
        };
        b.mass_kg = b.volume_m3() * mat.density;
        b
    }

    /// The pitch radius: where the rope's centre runs round a sheave.
    pub fn pitch_r(&self) -> f64 {
        0.5 * (self.tread_d + self.rope_d)
    }

    /// The X of sheave `i`'s mid-plane.
    pub fn sheave_x(&self, i: u32) -> f64 {
        let n = self.sheaves as f64;
        let stack = n * self.sheave_w + (n - 1.0).max(0.0) * (self.partition_t + 2.0 * self.gap);
        -0.5 * stack + 0.5 * self.sheave_w + i as f64 * (self.sheave_w + self.partition_t + 2.0 * self.gap)
    }

    /// The plates' extent along Y with the head up: (far end, head end).
    fn plate_y(&self) -> (f64, f64) {
        let lo = -0.5 * self.cheek_w;
        (lo, lo + self.cheek_h)
    }

    /// The shackle pin's Y with the head up.
    pub fn shackle_pin_y(&self) -> f64 {
        if self.sheaves == 0 {
            0.0
        } else {
            self.plate_y().1 - 1.25 * self.shackle_pin_d
        }
    }

    /// The block's end that faces the other block of its stage (Y, with the head up): where a
    /// dead end is made fast.
    pub fn far_end_y(&self) -> f64 {
        if self.sheaves == 0 {
            -0.5 * self.shackle_pin_d
        } else {
            self.plate_y().0
        }
    }

    /// Solid volume, cubic metres (the mass is this x the material's density).
    pub fn volume_m3(&self) -> f64 {
        let shackle_bow = 2.0 * self.shackle_inner_h + PI * 0.5 * (self.shackle_inner_w + self.shackle_d);
        let shackle = PI / 4.0 * self.shackle_d.powi(2) * shackle_bow
            + PI / 4.0 * self.shackle_pin_d.powi(2) * (self.shackle_inner_w + 2.0 * self.shackle_d);
        if self.sheaves == 0 {
            return shackle;
        }
        let n = self.sheaves as f64;
        // A sheave is a disc less its bore, about three quarters solid once the groove is cut.
        let sheave = PI / 4.0 * (self.flange_d.powi(2) - self.pin_d.powi(2)) * self.sheave_w * 0.75;
        let pin = PI / 4.0 * self.pin_d.powi(2) * self.width;
        let plates = (2.0 * self.cheek_t + (n - 1.0) * self.partition_t) * self.cheek_w * self.cheek_h;
        n * sheave + pin + plates + shackle
    }

    /// The block's collision boxes in its own frame (head up or down), each enclosing its part.
    pub fn colliders(&self, head_up: bool) -> Vec<Aabb> {
        let s = if head_up { 1.0 } else { -1.0 };
        let mut out = Vec::new();
        if self.sheaves > 0 {
            let (lo, hi) = self.plate_y();
            out.push(Aabb::from_y_span(0.5 * self.width, (s * lo, s * hi), 0.5 * self.cheek_w));
        }
        let py = self.shackle_pin_y();
        let half_x = 0.5 * self.shackle_inner_w + self.shackle_d;
        let bottom = py - 0.5 * self.shackle_pin_d.max(self.shackle_d);
        let top = py + self.shackle_inner_h + self.shackle_d;
        let half_z = 0.5 * self.shackle_pin_d.max(self.shackle_d);
        out.push(Aabb::from_y_span(half_x, (s * bottom, s * top), half_z));
        out
    }

    /// The block as triangles, in its own frame.
    pub fn mesh(&self, head_up: bool) -> MeshData {
        let s = if head_up { 1.0f32 } else { -1.0 };
        let mut m = MeshData::default();
        if self.sheaves > 0 {
            let (lo, hi) = self.plate_y();
            let (lo, hi) = (lo as f32 * s, hi as f32 * s);
            let yc = 0.5 * (lo + hi);
            let half_y = 0.5 * (hi - lo).abs();
            let half_z = 0.5 * self.cheek_w as f32;
            // The two side plates.
            let xo = 0.5 * self.width as f32 - 0.5 * self.cheek_t as f32;
            for sx in [-1.0f32, 1.0] {
                m.add_box(Vec3::new(sx * xo, yc, 0.0), Vec3::new(0.5 * self.cheek_t as f32, half_y, half_z));
            }
            // Partitions between neighbouring sheaves.
            for i in 0..self.sheaves.saturating_sub(1) {
                let x = 0.5 * (self.sheave_x(i) + self.sheave_x(i + 1)) as f32;
                m.add_box(Vec3::new(x, yc, 0.0), Vec3::new(0.5 * self.partition_t as f32, half_y, half_z));
            }
            // The sheaves: a grooved profile turned about the pin.
            for i in 0..self.sheaves {
                let x = self.sheave_x(i);
                m.add_lathe(&self.sheave_profile(x));
            }
            // The pin, the full width.
            let r = 0.5 * self.pin_d;
            let hw = 0.5 * self.width;
            m.add_lathe(&[(-hw, 0.0), (-hw, r), (hw, r), (hw, 0.0)]);
        }
        // The shackle: its pin through the head, and the bow, a tube swept along a U.
        let py = self.shackle_pin_y() as f32 * s;
        let pr = 0.5 * self.shackle_pin_d;
        let ph = 0.5 * self.shackle_inner_w + self.shackle_d;
        let mut pin = MeshData::default();
        pin.add_lathe(&[(-ph, 0.0), (-ph, pr), (ph, pr), (ph, 0.0)]);
        pin.translate(Vec3::new(0.0, py, 0.0));
        m.append(&pin);
        let leg_x = (0.5 * (self.shackle_inner_w + self.shackle_d)) as f32;
        let crown_r = leg_x;
        let crown_c = py + s * (self.shackle_inner_h as f32 - 0.5 * self.shackle_inner_w as f32);
        let mut path = vec![Vec3::new(-leg_x, py, 0.0)];
        let steps = 12;
        for k in 0..=steps {
            let a = std::f32::consts::PI * (1.0 - k as f32 / steps as f32);
            path.push(Vec3::new(crown_r * a.cos(), crown_c + s * crown_r * a.sin(), 0.0));
        }
        path.push(Vec3::new(leg_x, py, 0.0));
        m.add_tube(&path, 0.5 * self.shackle_d as f32, RING_SIDES);
        m
    }

    /// A sheave's half-section (x along the pin, r from it): bore, flange, groove, flange, bore.
    fn sheave_profile(&self, x: f64) -> Vec<(f64, f64)> {
        let hw = 0.5 * self.sheave_w;
        let bore = 0.5 * self.pin_d;
        let flange = 0.5 * self.flange_d;
        let tread = 0.5 * self.tread_d;
        let groove_r = 0.55 * self.rope_d;
        let mut p = vec![(x - hw, bore), (x - hw, flange)];
        // The groove: a round bottom of 0.55 d (it seats the rope) rising to the flanges.
        let steps = 8;
        for k in 0..=steps {
            let a = PI * k as f64 / steps as f64;
            p.push((x - groove_r * a.cos(), tread + groove_r - groove_r * a.sin()));
        }
        p.push((x + hw, flange));
        p.push((x + hw, bore));
        p
    }
}

/// An axis-aligned box: the collision shape of a block or a shackle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    fn from_y_span(half_x: f64, (a, b): (f64, f64), half_z: f64) -> Self {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        Aabb {
            min: Vec3::new(-half_x as f32, lo as f32, -half_z as f32),
            max: Vec3::new(half_x as f32, hi as f32, half_z as f32),
        }
    }
    pub fn translated(&self, by: Vec3) -> Self {
        Aabb { min: self.min + by, max: self.max + by }
    }
    /// Is `p` inside, with `eps` of slack for rounding?
    pub fn contains(&self, p: Vec3, eps: f32) -> bool {
        p.cmpge(self.min - Vec3::splat(eps)).all() && p.cmple(self.max + Vec3::splat(eps)).all()
    }
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

/// A capsule: the collision shape of a straight run of rope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Capsule {
    pub fn contains(&self, p: Vec3, eps: f32) -> bool {
        let ab = self.b - self.a;
        let t = if ab.length_squared() > 0.0 { ((p - self.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        (self.a + ab * t).distance(p) <= self.radius + eps
    }
}

/// Triangles: positions, normals and indices, ready for the renderer to upload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    fn push(&mut self, p: Vec3, n: Vec3) -> u32 {
        self.positions.push(p.to_array());
        self.normals.push(n.normalize_or_zero().to_array());
        (self.positions.len() - 1) as u32
    }

    pub fn append(&mut self, other: &MeshData) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    pub fn translate(&mut self, by: Vec3) {
        for p in &mut self.positions {
            *p = (Vec3::from_array(*p) + by).to_array();
        }
    }

    pub fn vertices(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.positions.iter().map(|p| Vec3::from_array(*p))
    }

    /// A box with flat-shaded faces.
    pub fn add_box(&mut self, c: Vec3, h: Vec3) {
        let faces = [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (-Vec3::X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (-Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (-Vec3::Z, Vec3::Y, Vec3::X),
        ];
        for (n, u, v) in faces {
            let o = c + n * h;
            let (du, dv) = (u * h, v * h);
            let i0 = self.push(o - du - dv, n);
            let i1 = self.push(o + du - dv, n);
            let i2 = self.push(o + du + dv, n);
            let i3 = self.push(o - du + dv, n);
            self.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
        }
    }

    /// A surface of revolution about the X axis from a half-section `(x, r)`, smooth-shaded.
    pub fn add_lathe(&mut self, profile: &[(f64, f64)]) {
        let n = RING_SIDES;
        let base = self.positions.len() as u32;
        for w in 0..profile.len() {
            let (x, r) = profile[w];
            // The profile's normal in the (x, r) plane, from its neighbours.
            let prev = profile[w.saturating_sub(1)];
            let next = profile[(w + 1).min(profile.len() - 1)];
            let (tx, tr) = (next.0 - prev.0, next.1 - prev.1);
            let (nx, nr) = (tr, -tx);
            for k in 0..n {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                let (c, s) = (a.cos(), a.sin());
                let p = Vec3::new(x as f32, (r * s) as f32, (r * c) as f32);
                let nn = Vec3::new(nx as f32, (nr * s) as f32, (nr * c) as f32);
                self.push(p, nn);
            }
        }
        for w in 0..profile.len().saturating_sub(1) as u32 {
            for k in 0..n {
                let a = base + w * n + k;
                let b = base + w * n + (k + 1) % n;
                let c = a + n;
                let d = b + n;
                self.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }

    /// A tube of `radius` swept along `points` with parallel-transported rings, so a sampled
    /// curve reads smooth and the tube never twists. Consecutive duplicate points are skipped.
    pub fn add_tube(&mut self, points: &[Vec3], radius: f32, sides: u32) {
        let mut pts: Vec<Vec3> = Vec::with_capacity(points.len());
        for &p in points {
            if pts.last().is_none_or(|q: &Vec3| q.distance(p) > 1e-6) {
                pts.push(p);
            }
        }
        if pts.len() < 2 {
            return;
        }
        let n = sides.max(3);
        let tangent = |i: usize| -> Vec3 {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(pts.len() - 1)];
            (b - a).normalize_or(Vec3::Y)
        };
        let t0 = tangent(0);
        let helper = if t0.dot(Vec3::Y).abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let mut normal = t0.cross(helper).normalize();
        let mut prev_t = t0;
        let base = self.positions.len() as u32;
        for i in 0..pts.len() {
            let t = tangent(i);
            // Parallel transport: turn the frame by the rotation from the last tangent to this one.
            let rot = glam::Quat::from_rotation_arc(prev_t, t);
            normal = (rot * normal - t * (rot * normal).dot(t)).normalize_or(normal);
            prev_t = t;
            let binormal = t.cross(normal);
            for k in 0..n {
                let a = std::f32::consts::TAU * k as f32 / n as f32;
                let off = normal * a.cos() + binormal * a.sin();
                self.push(pts[i] + off * radius, off);
            }
        }
        for i in 0..(pts.len() as u32 - 1) {
            for k in 0..n {
                let a = base + i * n + k;
                let b = base + i * n + (k + 1) % n;
                let c = a + n;
                let d = b + n;
                self.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
}

/// One stage drawn: its two blocks, its rope reeved between them, and the collision shapes, in
/// the stage's frame (the fixed block's pin at the origin, the moving block's `span` below).
#[derive(Debug, Clone, PartialEq)]
pub struct StageShape {
    pub upper: MeshData,
    pub lower: MeshData,
    pub rope: MeshData,
    pub boxes: Vec<Aabb>,
    pub capsules: Vec<Capsule>,
    /// The rope's centre line, for a schematic or a later physics rope.
    pub rope_path: Vec<Vec3>,
}

impl StageShape {
    /// Reeve `parts` parts of rope between `upper` (head up, at the origin) and `lower` (head
    /// down, its pin `span` below), the hauling end hanging `lead` below the fixed block.
    ///
    /// The reeving (the order the rope meets the sheaves): sheave 1 is the fixed block's, sheave 2
    /// the moving block's, and so on, alternating; the rope rises on the +Z side of a sheave and
    /// falls on the -Z side, over the top of a fixed sheave and under a moving one; after the last
    /// sheave the dead end is made fast to the other block's facing end.
    pub fn build(parts: u32, upper: &BlockDims, lower: &BlockDims, span: f64, lead: f64) -> Self {
        let span = span as f32;
        let rope_r = 0.5 * upper.rope_d as f32;
        let mut path: Vec<Vec3> = Vec::new();
        let mut straights: Vec<(Vec3, Vec3)> = Vec::new();
        let arc_steps = 10;
        // A straight run from the last point to `to`: a capsule, and the next point of the path.
        let start_straight = |path: &mut Vec<Vec3>, from: Option<Vec3>, to: Vec3, straights: &mut Vec<(Vec3, Vec3)>| {
            if let Some(f) = from {
                straights.push((f, to));
            }
            path.push(to);
        };
        // The hauling end, below the fixed block on its first sheave's rising side.
        let r_up = upper.pitch_r() as f32;
        let x0 = if upper.sheaves > 0 { upper.sheave_x(0) as f32 } else { 0.0 };
        let haul = Vec3::new(x0, -(lead as f32), r_up);
        path.push(haul);
        let mut last_end = Some(haul);
        for k in 1..=parts {
            let fixed = k % 2 == 1;
            let (b, idx, y0, sgn) = if fixed {
                (upper, (k - 1) / 2, 0.0f32, 1.0f32)
            } else {
                (lower, k / 2 - 1, -span, -1.0f32)
            };
            if b.sheaves == 0 || idx >= b.sheaves {
                break;
            }
            let x = b.sheave_x(idx) as f32;
            let r = b.pitch_r() as f32;
            // Arc in the Y-Z plane: over the top of a fixed sheave (from +Z to -Z), under a moving
            // sheave (from -Z to +Z).
            let first = Vec3::new(x, y0, sgn * r);
            start_straight(&mut path, last_end, first, &mut straights);
            for s in 1..=arc_steps {
                let a = std::f32::consts::PI * s as f32 / arc_steps as f32;
                path.push(Vec3::new(x, y0 + sgn * r * a.sin(), sgn * r * a.cos()));
            }
            last_end = path.last().copied();
        }
        // The dead end, to the other block's facing end, on the stage's centre plane.
        let dead = if parts % 2 == 1 {
            Vec3::new(0.0, -span - lower.far_end_y() as f32, 0.0)
        } else {
            Vec3::new(0.0, upper.far_end_y() as f32, 0.0)
        };
        start_straight(&mut path, last_end, dead, &mut straights);

        let mut rope = MeshData::default();
        rope.add_tube(&path, rope_r, RING_SIDES);
        let mut boxes = upper.colliders(true);
        boxes.extend(lower.colliders(false).into_iter().map(|b| b.translated(Vec3::new(0.0, -span, 0.0))));
        let capsules = straights.into_iter().map(|(a, b)| Capsule { a, b, radius: rope_r }).collect();
        let mut lower_mesh = lower.mesh(false);
        lower_mesh.translate(Vec3::new(0.0, -span, 0.0));
        StageShape { upper: upper.mesh(true), lower: lower_mesh, rope, boxes, capsules, rope_path: path }
    }
}

/// A key for a shape: a version of every number it is built from, so a changed number builds a
/// new shape and an unchanged one never does (CLAUDE.md, the reuse rule).
pub fn shape_key(numbers: &[f64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in numbers {
        v.to_bits().hash(&mut h);
    }
    h.finish()
}

/// Built once per configuration: the last shape and the key of the numbers it was built from.
/// With the reuse switch off (`humanity_core::reuse::enabled()`, the Dev page) it builds anew
/// every time it is asked.
#[derive(Debug, Default)]
/// reuse: site: the last block-and-tackle shapes, kept under a key of every number they are built
/// from; with the switch off they are worked out again every time.
pub struct ShapeStore {
    kept: Option<(u64, std::sync::Arc<Vec<StageShape>>)>,
    /// How many times it has built (the tests read it).
    pub builds: u32,
}

impl ShapeStore {
    pub fn get(&mut self, key: u64, build: impl FnOnce() -> Vec<StageShape>) -> std::sync::Arc<Vec<StageShape>> {
        self.get_with(key, humanity_core::reuse::enabled(), build)
    }

    /// `get`, with the switch passed in (so tests need not flip the process-wide switch).
    pub fn get_with(&mut self, key: u64, reuse: bool, build: impl FnOnce() -> Vec<StageShape>) -> std::sync::Arc<Vec<StageShape>> {
        if reuse {
            if let Some((k, shape)) = &self.kept {
                if *k == key {
                    return shape.clone();
                }
            }
        }
        self.builds += 1;
        let shape = std::sync::Arc::new(build());
        self.kept = if reuse { Some((key, shape.clone())) } else { None };
        shape
    }
}
