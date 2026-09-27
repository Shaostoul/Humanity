//! CPU twin of the megashader's shared 2D lattice value noise
//! (`lattice_hash` / `value_noise` in `assets/shaders/pbr/10-lighting-patterns.wgsl`),
//! plus the old float `hash21` that the per-pixel jitters and voronoi still use.
//!
//! ## Why the lattice hash is an integer hash (BUG-103, 2026-09-27)
//!
//! `value_noise` used to hash its four corners with the float `hash21` of
//! `i + vec2(1.0, 0.0)` and friends. Two neighbouring cells share a corner,
//! and a GPU compiler may round the two routes to it differently: the classic
//! case is folding the `+ 1.0` into hash21's first multiply for one cell
//! (`i * 0.1031 + 0.1031`, two roundings) while the other cell multiplies the
//! floored value directly (one rounding). At lattice coordinates in the
//! thousands, which every planet-scale caller reaches, one ulp there changes
//! the float hash's answer completely, so the cells disagreed about the corner
//! they share and the field stepped along every lattice line: the rectangular
//! blocks in the open-sea colour seen from orbit (`deck-55-nadir`,
//! `ocean-glint-150km`).
//!
//! The corner is now named by the INTEGER lattice point: the cell is floored
//! once, converted to `i32`, every corner is an integer add, and the hash is
//! pure integer arithmetic down to an exact 24-bit conversion. Integer
//! arithmetic has no rounding, so the corner value is a function of the
//! lattice point alone on every GPU and here, bit for bit. The tests below pin
//! that property (and prove the old hash fails it), pin the WGSL constants and
//! body against this file, and hold a device test that runs the real WGSL on
//! the real GPU and compares every hash bit.
//!
//! Every pattern built on `value_noise` re-rolled with this change (sea
//! colour, shore depth noise and surf, the wave-crest warp, land detail, and
//! the material textures that use `value_noise`/`fbm`); their statistics did
//! not change. The float `hash21` itself is unchanged, and voronoi, the cloud
//! lattice (`cloud_noise`, float `hash13`) and the ground micro noise kept
//! their exact patterns: they were moved to integer corners that convert to
//! the identical float values, which removes the same mechanism without a
//! re-roll.

/// Odd multiplier for the x coordinate in the linear combine. Mirrors
/// `LATTICE_HASH_KX` in the WGSL.
pub const LATTICE_HASH_KX: u32 = 0x27D4_EB2D;
/// Odd multiplier for the y coordinate. Mirrors `LATTICE_HASH_KY`.
pub const LATTICE_HASH_KY: u32 = 0x1656_67B1;
/// lowbias32 finaliser multipliers (C. Wellons, "Prospecting for Hash
/// Functions", 2018, public domain). Mirror `LATTICE_HASH_M1` / `_M2`.
pub const LATTICE_HASH_M1: u32 = 0x7FEB_352D;
pub const LATTICE_HASH_M2: u32 = 0x846C_A68B;

/// Mirrors WGSL `lattice_hash`: an integer lattice point to [0, 1), exact.
/// Wrapping arithmetic is WGSL's u32 arithmetic; `as u32` on an i32 is the
/// WGSL `bitcast<u32>`.
pub fn lattice_hash(cx: i32, cy: i32) -> f32 {
    let mut h = (cx as u32)
        .wrapping_mul(LATTICE_HASH_KX)
        .wrapping_add((cy as u32).wrapping_mul(LATTICE_HASH_KY));
    h = (h ^ (h >> 16)).wrapping_mul(LATTICE_HASH_M1);
    h = (h ^ (h >> 15)).wrapping_mul(LATTICE_HASH_M2);
    h ^= h >> 16;
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// The integer cell of `p` and the four corner values, in the WGSL's order
/// (a = cell, b = +x, c = +y, d = +x+y). Split out so the tests can compare a
/// corner as seen from each of the cells that share it.
pub fn value_noise_corners(px: f32, py: f32) -> ((i32, i32), [f32; 4]) {
    // WGSL `vec2<i32>(floor(p))`. Rust's `as` saturates out of range, and so
    // does the WGSL conversion; the callers stay far inside i32 anyway.
    let cx = px.floor() as i32;
    let cy = py.floor() as i32;
    let corners = [
        lattice_hash(cx, cy),
        lattice_hash(cx.wrapping_add(1), cy),
        lattice_hash(cx, cy.wrapping_add(1)),
        lattice_hash(cx.wrapping_add(1), cy.wrapping_add(1)),
    ];
    ((cx, cy), corners)
}

/// Mirrors WGSL `value_noise`: bilinear over the integer-lattice corners with
/// a smoothstep fade. The corners are bit-identical to the GPU's; the blend is
/// a handful of float ops, so the two agree to about one ulp.
pub fn value_noise(px: f32, py: f32) -> f32 {
    let (_, [a, b, c, d]) = value_noise_corners(px, py);
    let fx = px - px.floor();
    let fy = py - py.floor();
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uy
}

/// WGSL `fract` semantics: `x - floor(x)`, always in [0, 1), NOT Rust's
/// `f32::fract`, which is negative for negative inputs.
fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Mirrors the float `hash21` in 10-lighting-patterns.wgsl, unchanged. The
/// per-pixel jitters and `voronoi_point` use it; a lattice must feed it only a
/// value converted from an exact integer (see the module doc).
pub fn hash21(px: f32, py: f32) -> f32 {
    let mut p3 = [fract(px * 0.1031), fract(py * 0.1031), fract(px * 0.1031)];
    // dot(p3, p3.yzx + 33.33)
    let d = p3[0] * (p3[1] + 33.33) + p3[1] * (p3[2] + 33.33) + p3[2] * (p3[0] + 33.33);
    p3[0] += d;
    p3[1] += d;
    p3[2] += d;
    fract((p3[0] + p3[1]) * p3[2])
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small deterministic generator for the test samples (no rand dep).
    pub(crate) struct Lcg(pub u64);
    impl Lcg {
        pub(crate) fn next_u32(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as u32
        }
        pub(crate) fn unit(&mut self) -> f32 {
            (self.next_u32() >> 8) as f32 / 16_777_216.0
        }
        /// A lattice coordinate of magnitude about `m`, either sign.
        pub(crate) fn coord(&mut self, m: f32) -> i32 {
            let v = (m * (0.5 + self.unit())) as i32;
            if self.next_u32() & 1 == 0 { v } else { -v }
        }
    }

    /// Magnitudes the planet-scale callers actually reach: sea colour 700 to
    /// 3,500, the shore 70,000, the finest land octave 800,000, and beyond.
    const MAGNITUDES: [f32; 7] = [100.0, 1_000.0, 3_500.0, 70_000.0, 800_000.0, 4_000_000.0, 8_000_000.0];

    /// THE SHARED-CORNER PROPERTY: two neighbouring cells, each evaluated from
    /// a point inside itself, hand back bit-identical values for every corner
    /// they share, at coordinates in the thousands and far beyond.
    #[test]
    fn neighbouring_cells_agree_bit_for_bit_on_every_shared_corner() {
        let mut rng = Lcg(0x5EA_B10C);
        for m in MAGNITUDES {
            for _ in 0..4000 {
                let (x, y) = (rng.coord(m), rng.coord(m));
                // A point inside cell (x, y) and inside each of its +x, +y
                // neighbours. Past 2^21 the quarter offsets stop being exact,
                // so there the cell's own integer point stands in (f = 0).
                let within = |c: i32, off: f32| {
                    let v = c as f32 + off;
                    if v.floor() as i32 == c { v } else { c as f32 }
                };
                let inside = |cx: i32, cy: i32| (within(cx, 0.25), within(cy, 0.75));
                let (p0x, p0y) = inside(x, y);
                let (px_x, px_y) = inside(x + 1, y);
                let (py_x, py_y) = inside(x, y + 1);
                let (c0, [_, b0, c0v, d0]) = value_noise_corners(p0x, p0y);
                let (cx1, [a1, _, c1, _]) = value_noise_corners(px_x, px_y);
                let (cy1, [a2, b2, _, _]) = value_noise_corners(py_x, py_y);
                assert_eq!(c0, (x, y), "cell of the inside point at magnitude {m}");
                assert_eq!(cx1, (x + 1, y));
                assert_eq!(cy1, (x, y + 1));
                // +x neighbour: its a/c are our b/d.
                assert_eq!(b0.to_bits(), a1.to_bits(), "corner ({}, {y}) at magnitude {m}", x + 1);
                assert_eq!(d0.to_bits(), c1.to_bits(), "corner ({}, {}) at magnitude {m}", x + 1, y + 1);
                // +y neighbour: its a/b are our c/d.
                assert_eq!(c0v.to_bits(), a2.to_bits(), "corner ({x}, {}) at magnitude {m}", y + 1);
                assert_eq!(d0.to_bits(), b2.to_bits(), "corner ({}, {}) at magnitude {m}", x + 1, y + 1);
            }
        }
    }

    /// RED CHECK: the old corner route fails the same property. The old
    /// value_noise reached corner x+1 through `hash21(i + 1.0)`; a compiler may
    /// evaluate the scaled input as `i * 0.1031 + 0.1031` (two roundings) for
    /// that cell while the neighbouring cell computes `(i + 1) * 0.1031` (one).
    /// This emulates both routes in f32 and shows the old float hash hands the
    /// two cells different corners, which is exactly the seam; the integer
    /// hash has no float route to perturb.
    #[test]
    fn the_old_float_hash_fails_the_shared_corner_test() {
        let c = 0.1031f32;
        // hash21 with its first multiply already done (ax = px * 0.1031).
        let hash21_scaled = |ax: f32, ay: f32| -> f32 {
            let mut p3 = [fract(ax), fract(ay), fract(ax)];
            let d = p3[0] * (p3[1] + 33.33) + p3[1] * (p3[2] + 33.33) + p3[2] * (p3[0] + 33.33);
            p3[0] += d;
            p3[1] += d;
            p3[2] += d;
            fract((p3[0] + p3[1]) * p3[2])
        };
        let mut rng = Lcg(0x0DD_5EA);
        for m in [1_000.0f32, 3_500.0, 70_000.0] {
            let (mut n, mut disagree, mut worst) = (0u32, 0u32, 0.0f32);
            for _ in 0..4000 {
                let (x, y) = (rng.coord(m), rng.coord(m));
                let ay = y as f32 * c;
                // Cell x+1 multiplies its own floored corner: one rounding.
                let own = hash21_scaled((x + 1) as f32 * c, ay);
                // Cell x reaches the same corner with the offset folded in:
                // `x * c` rounds, then `+ c` rounds again (Rust never fuses).
                let folded = hash21_scaled(x as f32 * c + c, ay);
                n += 1;
                let d = (own - folded).abs();
                let d = d.min(1.0 - d);
                if d > 1.0 / 255.0 {
                    disagree += 1;
                }
                worst = worst.max(d);
                // The integer hash of the same corner, from both cells: equal.
                assert_eq!(lattice_hash(x + 1, y).to_bits(), value_noise_corners(x as f32 + 0.5, y as f32 + 0.5).1[1].to_bits());
            }
            let share = disagree as f32 / n as f32;
            assert!(
                share > 0.15 && worst > 0.1,
                "the emulated old route should disagree at magnitude {m} (share {share}, worst {worst}); \
                 if it no longer does, this red check has lost its teeth"
            );
        }
    }

    /// Known answers. These pin the function itself; the device test below
    /// proves the GPU returns exactly these bits.
    #[test]
    fn lattice_hash_known_answers() {
        let kat: [((i32, i32), u32); 6] = [
            ((0, 0), lattice_hash_bits(0, 0)),
            ((1, 0), lattice_hash_bits(1, 0)),
            ((0, 1), lattice_hash_bits(0, 1)),
            ((-1, -1), lattice_hash_bits(-1, -1)),
            ((2055, 1483), lattice_hash_bits(2055, 1483)),
            ((-800_000, 123_456), lattice_hash_bits(-800_000, 123_456)),
        ];
        for ((x, y), want) in kat {
            assert_eq!(lattice_hash(x, y).to_bits(), want);
        }
        // Frozen values (computed once from the reference above; a change to
        // the constants or the mixer changes them and must be deliberate).
        assert_eq!(lattice_hash(0, 0), 0.0);
        assert_eq!(lattice_hash(1, 0).to_bits(), FROZEN_1_0);
        assert_eq!(lattice_hash(2055, 1483).to_bits(), FROZEN_2055_1483);
    }

    /// Independent restatement of the mixer in u64 arithmetic, so the KAT is
    /// not just the function compared with itself.
    fn lattice_hash_bits(x: i32, y: i32) -> u32 {
        let m = 1u64 << 32;
        let mut h = ((x as u32 as u64) * LATTICE_HASH_KX as u64 + (y as u32 as u64) * LATTICE_HASH_KY as u64) % m;
        h = ((h ^ (h >> 16)) * LATTICE_HASH_M1 as u64) % m;
        h = ((h ^ (h >> 15)) * LATTICE_HASH_M2 as u64) % m;
        h ^= h >> 16;
        ((h >> 8) as f32 * (1.0 / 16_777_216.0)).to_bits()
    }
    // Computed independently (BigInt in Node) when the hash was written.
    const FROZEN_1_0: u32 = 0x3F03_D1E3; // 0.51492137
    const FROZEN_2055_1483: u32 = 0x3F60_3EEC; // 0.87596011

    /// Statistics: uniform on [0, 1), mean 0.5, and neighbouring corners
    /// uncorrelated, at small and planet-scale coordinates alike (the float
    /// hash's quality fell apart at 1e5; this one must not).
    #[test]
    fn corners_are_uniform_and_uncorrelated_at_every_magnitude() {
        let mut rng = Lcg(0xC0FF_EE);
        for m in MAGNITUDES {
            let n = 20_000;
            let (mut s, mut s2, mut sxy, mut sy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let mut bins = [0u32; 16];
            for _ in 0..n {
                let (x, y) = (rng.coord(m), rng.coord(m));
                let a = lattice_hash(x, y) as f64;
                let b = lattice_hash(x + 1, y) as f64;
                assert!((0.0..1.0).contains(&a));
                bins[(a * 16.0) as usize] += 1;
                s += a;
                s2 += a * a;
                sy += b;
                sxy += a * b;
            }
            let nf = n as f64;
            let mean = s / nf;
            let var = s2 / nf - mean * mean;
            let cov = sxy / nf - mean * (sy / nf);
            assert!((mean - 0.5).abs() < 0.01, "mean {mean} at magnitude {m}");
            assert!((var - 1.0 / 12.0).abs() < 0.004, "variance {var} at magnitude {m}");
            assert!((cov / var).abs() < 0.03, "neighbour correlation {} at magnitude {m}", cov / var);
            for b in bins {
                let share = b as f64 / nf;
                assert!((share - 1.0 / 16.0).abs() < 0.008, "bin share {share} at magnitude {m}");
            }
        }
    }

    /// The linear combine makes the field repeat along the shortest vector of
    /// {(dx, dy) : dx*KX + dy*KY = 0 mod 2^32}. Gauss-reduce that lattice and
    /// pin the answer the WGSL comment quotes: (33863, -40683), 52,932 cells,
    /// which is 420 km at the finest 8 m land octave.
    #[test]
    fn the_hash_only_repeats_along_a_vector_longer_than_any_view() {
        let m: i128 = 1 << 32;
        let md = |a: i128| ((a % m) + m) % m;
        // Modular inverse of the odd KX.
        let inv = |a: i128| {
            let (mut t, mut nt, mut r, mut nr) = (0i128, 1i128, m, md(a));
            while nr != 0 {
                let q = r / nr;
                (t, nt) = (nt, t - q * nt);
                (r, nr) = (nr, r - q * nr);
            }
            md(t)
        };
        let t = md(-(LATTICE_HASH_KY as i128) * inv(LATTICE_HASH_KX as i128));
        let (mut u, mut v) = ((m, 0i128), (t, 1i128));
        let dot = |a: (i128, i128), b: (i128, i128)| a.0 * b.0 + a.1 * b.1;
        for _ in 0..1000 {
            if dot(u, u) < dot(v, v) {
                std::mem::swap(&mut u, &mut v);
            }
            let (num, den) = (dot(u, v), dot(v, v));
            let q = (num as f64 / den as f64).round() as i128;
            if q == 0 {
                break;
            }
            u = (u.0 - q * v.0, u.1 - q * v.1);
        }
        let s = if dot(u, u) < dot(v, v) { u } else { v };
        let len = (dot(s, s) as f64).sqrt();
        assert_eq!(md(s.0 * LATTICE_HASH_KX as i128 + s.1 * LATTICE_HASH_KY as i128), 0);
        assert!((len - 52_932.0).abs() < 1.0, "shortest repeat {s:?}, length {len}");
        assert!(s == (33863, -40683) || s == (-33863, 40683), "shortest repeat {s:?}");
        // And the field really does repeat there (the reduction is right).
        assert_eq!(lattice_hash(5, 9).to_bits(), lattice_hash(5 + 33863, 9 - 40683).to_bits());
    }

    #[test]
    fn hash_and_value_noise_stay_in_unit_range() {
        for i in 0..500 {
            let x = (i as f32) * 0.73 - 180.0; // negative coords included:
            let y = (i as f32) * 1.19 - 250.0; // the WGSL-fract mirror matters
            let h = hash21(x, y);
            assert!((0.0..1.0).contains(&h), "hash out of range: {h}");
            let v = value_noise(x, y);
            assert!((0.0..=1.0).contains(&v), "value_noise out of range: {v}");
        }
    }

    /// Extract one WGSL function (through its matching brace) from a source.
    pub(crate) fn wgsl_fn(src: &str, name: &str) -> String {
        let start = src
            .find(&format!("fn {name}("))
            .unwrap_or_else(|| panic!("fn {name} missing from the WGSL"));
        let open = start + src[start..].find('{').expect("fn body");
        let mut depth = 0i32;
        for (k, ch) in src[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return src[start..open + k + 1].to_string();
                    }
                }
                _ => {}
            }
        }
        panic!("unbalanced braces in fn {name}");
    }

    /// The text with every `//` comment removed, so a negative check cannot
    /// trip on a comment that quotes the old code.
    fn code_only(s: &str) -> String {
        s.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
    }

    fn patterns_wgsl() -> String {
        crate::renderer::shader_loader::PBR_PARTS
            .iter()
            .find(|(n, _)| *n == "10-lighting-patterns.wgsl")
            .expect("10-lighting-patterns.wgsl in PBR_PARTS")
            .1
            .replace("\r\n", "\n")
    }

    /// LOCKSTEP, text half: the WGSL constants equal these, the WGSL mixer
    /// has the same shifts and the same exact conversion, and the WGSL
    /// value_noise names its corners by integer adds through lattice_hash,
    /// never through the float hash (the regression this bug was).
    #[test]
    fn wgsl_lattice_hash_is_this_hash() {
        let src = patterns_wgsl();
        for (name, v) in [
            ("LATTICE_HASH_KX", LATTICE_HASH_KX),
            ("LATTICE_HASH_KY", LATTICE_HASH_KY),
            ("LATTICE_HASH_M1", LATTICE_HASH_M1),
            ("LATTICE_HASH_M2", LATTICE_HASH_M2),
        ] {
            let want = format!("const {name}: u32 = 0x{v:08X}u;");
            assert!(src.contains(&want), "WGSL must declare `{want}`");
        }
        let body = wgsl_fn(&src, "lattice_hash");
        for piece in [
            "bitcast<u32>(c.x) * LATTICE_HASH_KX + bitcast<u32>(c.y) * LATTICE_HASH_KY",
            "h = (h ^ (h >> 16u)) * LATTICE_HASH_M1;",
            "h = (h ^ (h >> 15u)) * LATTICE_HASH_M2;",
            "h = h ^ (h >> 16u);",
            "return f32(h >> 8u) * (1.0 / 16777216.0);",
        ] {
            assert!(body.contains(piece), "WGSL lattice_hash lost `{piece}`:\n{body}");
        }
        let vn = code_only(&wgsl_fn(&src, "value_noise"));
        assert!(!vn.contains("hash21"), "value_noise must not hash corners with the float hash21 (BUG-103):\n{vn}");
        for piece in [
            "let c = vec2<i32>(i);",
            "lattice_hash(c)",
            "lattice_hash(c + vec2<i32>(1, 0))",
            "lattice_hash(c + vec2<i32>(0, 1))",
            "lattice_hash(c + vec2<i32>(1, 1))",
        ] {
            assert!(vn.contains(piece), "WGSL value_noise lost `{piece}`:\n{vn}");
        }
        // Voronoi keeps the float hash (its pattern is unchanged) but must
        // name cells by integer: no float `i + neighbor` corner route.
        for f in ["voronoi", "voronoi_edge"] {
            let body = code_only(&wgsl_fn(&src, f));
            assert!(body.contains("voronoi_point(ci + vec2<i32>(x, y))"), "{f} must name cells by integer:\n{body}");
            assert!(!body.contains("hash21(i + neighbor"), "{f} is back on a float cell route:\n{body}");
        }
    }

    /// The cloud lattice and the ground micro noise keep their float hashes
    /// (and so their exact patterns) but must reach their far corners by an
    /// integer add, never `i + vec3<f32>(1.0, ...)`.
    #[test]
    fn cloud_and_micro_lattices_use_integer_corners() {
        let part = |n: &str| {
            crate::renderer::shader_loader::PBR_PARTS
                .iter()
                .find(|(p, _)| *p == n)
                .unwrap_or_else(|| panic!("{n} in PBR_PARTS"))
                .1
                .replace("\r\n", "\n")
        };
        let clouds = code_only(&wgsl_fn(&part("40-clouds.wgsl"), "cloud_noise"));
        assert!(clouds.contains("let i1 = vec3<f32>(vec3<i32>(i) + vec3<i32>(1));"), "{clouds}");
        assert!(!clouds.contains("hash13(i + vec3<f32>("), "cloud_noise is back on a float corner route:\n{clouds}");
        // The comments quote the old arithmetic; check the code alone.
        let micro = code_only(&wgsl_fn(&part("20-surface-detail.wgsl"), "micro_noise"));
        assert!(micro.contains("var ci = w0 + vec3<i32>(dx, dy, dz);"), "{micro}");
        assert!(!micro.contains("floor(c / period)"), "micro_noise is back on the per-corner float wrap:\n{micro}");
    }
}

/// LOCKSTEP, device half: compile the SHIPPED WGSL `lattice_hash` and
/// `value_noise` (plus the old float value_noise, for the record) into a
/// compute shader, run them on the real GPU through the same DX12 backend the
/// game uses, and compare with this file. Ignored because CI has no adapter;
/// run it deliberately (point HUMANITY_DXC_DIR at a folder holding
/// dxcompiler.dll + dxil.dll to use the game's DXC rather than FXC):
///   cargo test --features native --lib lattice_noise -- --ignored --nocapture
#[cfg(all(test, feature = "native"))]
mod device_tests {
    use super::tests::{wgsl_fn, Lcg};
    use super::*;
    use wgpu::util::DeviceExt;

    const OLD_VALUE_NOISE: &str = "
fn old_value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
";

    const ENTRY: &str = "
struct Probe { ix: i32, iy: i32, px: f32, py: f32 }
@group(0) @binding(0) var<storage, read> probes: array<Probe>;
@group(0) @binding(1) var<storage, read_write> outs: array<vec4<u32>>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= arrayLength(&probes)) { return; }
    let q = probes[k];
    let p = vec2<f32>(q.px, q.py);
    outs[k] = vec4<u32>(
        bitcast<u32>(lattice_hash(vec2<i32>(q.ix, q.iy))),
        bitcast<u32>(value_noise(p)),
        bitcast<u32>(old_value_noise(p)),
        bitcast<u32>(hash21(vec2<f32>(f32(q.ix), f32(q.iy)))),
    );
}
";

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct Probe {
        ix: i32,
        iy: i32,
        px: f32,
        py: f32,
    }

    fn device() -> (wgpu::Device, wgpu::Queue, String) {
        let dxc = std::env::var("HUMANITY_DXC_DIR").ok().map(std::path::PathBuf::from).filter(|d| {
            d.join("dxcompiler.dll").exists() && d.join("dxil.dll").exists()
        });
        let mut backend_options = wgpu::BackendOptions::default();
        let mut compiler = "FXC".to_string();
        if let Some(d) = dxc {
            backend_options.dx12 = wgpu::Dx12BackendOptions {
                shader_compiler: wgpu::Dx12Compiler::DynamicDxc {
                    dxc_path: d.join("dxcompiler.dll").to_string_lossy().into_owned(),
                    dxil_path: d.join("dxil.dll").to_string_lossy().into_owned(),
                },
            };
            compiler = format!("DXC ({})", d.display());
        }
        #[cfg(target_os = "windows")]
        let backends = wgpu::Backends::DX12;
        #[cfg(not(target_os = "windows"))]
        let backends = wgpu::Backends::VULKAN | wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, backend_options, ..Default::default() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no adapter");
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("lattice noise test"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            },
            None,
        ))
        .expect("no device");
        device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));
        (device, queue, format!("{} / {:?} / {compiler}", info.name, info.backend))
    }

    fn run(device: &wgpu::Device, queue: &wgpu::Queue, probes: &[Probe]) -> Vec<[u32; 4]> {
        let src = crate::renderer::shader_loader::PBR_PARTS
            .iter()
            .find(|(n, _)| *n == "10-lighting-patterns.wgsl")
            .unwrap()
            .1
            .replace("\r\n", "\n");
        let mut wgsl = String::new();
        for line in src.lines().filter(|l| l.starts_with("const LATTICE_HASH_")) {
            wgsl.push_str(line);
            wgsl.push('\n');
        }
        for f in ["hash21", "lattice_hash", "value_noise"] {
            wgsl.push_str(&wgsl_fn(&src, f));
            wgsl.push('\n');
        }
        wgsl.push_str(OLD_VALUE_NOISE);
        wgsl.push_str(ENTRY);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lattice noise probe"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lattice noise probe"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let inp = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("probes"),
            contents: bytemuck::cast_slice(probes),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let size = (probes.len() * 16) as u64;
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("outs"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lattice noise probe"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: inp.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: out.as_entire_binding() },
            ],
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups((probes.len() as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&out, 0, &read, 0, size);
        queue.submit([enc.finish()]);
        let slice = read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let words: &[u32] = bytemuck::cast_slice(&data);
        words.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
    }

    #[test]
    #[ignore]
    fn the_gpu_runs_the_same_lattice_noise() {
        let (device, queue, what) = device();
        println!("[lattice_noise] device: {what}");
        let mut rng = Lcg(0x6E0_1A77);

        // 1. Lockstep: hash bits and value_noise at every magnitude.
        let mut probes = Vec::new();
        for m in [10.0f32, 1_000.0, 3_500.0, 70_000.0, 800_000.0, 8_000_000.0] {
            for _ in 0..4096 {
                let (ix, iy) = (rng.coord(m), rng.coord(m));
                let px = ix as f32 + rng.unit();
                let py = iy as f32 + rng.unit();
                probes.push(Probe { ix, iy, px, py });
            }
        }
        let got = run(&device, &queue, &probes);
        let (mut hash_mismatch, mut vn_worst, mut old_hash_mismatch) = (0usize, 0.0f32, 0usize);
        for (p, g) in probes.iter().zip(&got) {
            if g[0] != lattice_hash(p.ix, p.iy).to_bits() {
                hash_mismatch += 1;
            }
            vn_worst = vn_worst.max((f32::from_bits(g[1]) - value_noise(p.px, p.py)).abs());
            if g[3] != hash21(p.ix as f32, p.iy as f32).to_bits() {
                old_hash_mismatch += 1;
            }
        }
        println!(
            "[lattice_noise] {} probes: lattice_hash bit mismatches {hash_mismatch}, value_noise worst |gpu - cpu| {vn_worst:e}, \
             float hash21 bit mismatches {old_hash_mismatch} (the float hash is NOT in lockstep, for the record)",
            probes.len()
        );
        assert_eq!(hash_mismatch, 0, "the GPU lattice_hash must equal the CPU twin bit for bit");
        assert!(vn_worst < 1.0e-6, "value_noise GPU/CPU drift {vn_worst}");

        // 2. Continuity across lattice lines on the real compiler: the field at
        // X and one 2^-10 step before it must agree to second order (the
        // smoothstep's across-line slope is zero there). Coordinates stay
        // under 8192 so the step is exact. A seam is a corner disagreement.
        let step = 1.0f32 / 1024.0;
        let mags = [100.0f32, 1_000.0, 3_000.0, 6_000.0];
        let mut pairs = Vec::new();
        for m in mags {
            for _ in 0..4096 {
                let (x, y) = (rng.coord(m), rng.coord(m));
                let t = rng.unit();
                let (lx, ly) = (x as f32, y as f32 + t);
                pairs.push(Probe { ix: 0, iy: 0, px: lx - step, py: ly });
                pairs.push(Probe { ix: 0, iy: 0, px: lx, py: ly });
                pairs.push(Probe { ix: 0, iy: 0, px: y as f32 + t, py: x as f32 - step });
                pairs.push(Probe { ix: 0, iy: 0, px: y as f32 + t, py: x as f32 });
            }
        }
        let got = run(&device, &queue, &pairs);
        let (mut new_seams, mut old_seams, mut new_worst, mut old_worst) = (0usize, 0usize, 0.0f32, 0.0f32);
        let per_mag = got.len() / 2 / mags.len();
        let mut old_by_mag = vec![0usize; mags.len()];
        for (k, g) in got.chunks(2).enumerate() {
            let dn = (f32::from_bits(g[0][1]) - f32::from_bits(g[1][1])).abs();
            let dold = (f32::from_bits(g[0][2]) - f32::from_bits(g[1][2])).abs();
            new_worst = new_worst.max(dn);
            old_worst = old_worst.max(dold);
            if dn > 1.0e-4 {
                new_seams += 1;
            }
            if dold > 1.0e-4 {
                old_seams += 1;
                old_by_mag[k / per_mag] += 1;
            }
        }
        println!(
            "[lattice_noise] {} lattice-line crossings: new value_noise seams {new_seams} (worst {new_worst:e}), \
             OLD value_noise seams {old_seams} (worst {old_worst:e}); old seams per {per_mag} crossings at |coord| ~ {:?}: {:?}",
            pairs.len() / 2,
            mags,
            old_by_mag
        );
        assert_eq!(new_seams, 0, "the integer-lattice value_noise stepped at a lattice line on this GPU");
    }
}
