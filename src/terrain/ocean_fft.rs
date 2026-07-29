//! FFT ocean (water-fft.md increment 1, v0.1029): a JONSWAP-spectrum
//! statistical sea on a camera-anchored periodic tile, computed on the
//! CPU and shared VERBATIM between the GPU displacement texture and the
//! buoyancy sampler - drawn == sampled because they are the same array.
//!
//! Pure module: no GPU, no engine types. The engine owns scheduling
//! (worker thread), upload, and the Settings toggle; this file owns the
//! math. Determinism: h0(k) seeds from the planet's terrain_seed, so
//! every client computes the identical sea with zero sync traffic.

const TAU: f32 = std::f32::consts::TAU;
const G: f32 = 9.81;

/// Engine-standard cascades (increment 3, v0.1040 - operator: "the
/// texture repeats A LOT... looks like a grid"): TWO tiles now. Cascade
/// A (64 m, 0.5 m texels) carries the short chop and rides the 64 m
/// ground anchor; cascade B (256 m, 2 m texels) carries the mid/long
/// waves and rides its OWN mod-256 anchor (light4_cone_inner.xyz), so
/// each tile still shifts by exactly one period per anchor snap. Each
/// cascade is BAND-LIMITED to its wavelength range so they never
/// double-count energy, and each fades at its own resolvable distance
/// (the seam fix: under-resolved short waves vanish before cross-LOD
/// patch borders can disagree about them).
pub const FFT_TILE_M: f32 = 64.0;
pub const FFT_N: usize = 128;
/// Cascade B tile (must divide ITS anchor modulus: 256 mod 256 = 0).
pub const FFT_B_TILE_M: f32 = 256.0;
/// Wavelength split between the cascades (metres): A holds (0, 32],
/// B holds (32, 256].
pub const FFT_SPLIT_LAMBDA_M: f32 = 32.0;
/// Texture layout: one 128x256 texture, cascade A in rows [0, 128),
/// cascade B in rows [128, 256). LOCKSTEP with the shader's
/// OCEAN_FFT_ROW_B constant.
pub const FFT_TEX_H: usize = 256;
/// Per-cascade resolution-fade wavelengths (fed to the shader's
/// ocean_train_fade): A dies by ~1 km, B reaches the mid-distance the
/// operator called flat.
pub const FFT_A_FADE_LAMBDA: f32 = 16.0;
pub const FFT_B_FADE_LAMBDA: f32 = 96.0;
/// Per-cascade RMS targets: together they keep the trains' total
/// energy envelope (sqrt(0.18^2 + 0.34^2) ~ 0.385).
pub const FFT_A_TARGET_RMS_M: f32 = 0.18;
pub const FFT_B_TARGET_RMS_M: f32 = 0.34;
/// Per-plane UV decorrelation offsets for the triplanar projection.
/// LOCKSTEP with the shader's OCEAN_FFT_OFF_X/Y/Z constants.
pub const PLANE_OFF: [[f32; 2]; 3] = [[0.0, 0.0], [0.271, 0.417], [0.613, 0.129]];
/// Target RMS height (m): matches the three anchored chop trains the FFT
/// field replaces (sqrt((0.4^2 + 0.35^2 + 0.1^2) / 2)), so toggling modes
/// keeps the same sea-energy envelope.
pub const FFT_TARGET_RMS_M: f32 = 0.386;

/// Choppy-displacement scale for the Jacobian foam mask (increment 2).
/// The GEOMETRY stays vertical-only for now (drawn == sampled with the
/// buoyancy twin holds exactly); the horizontal field is realized purely
/// to find where crests pinch - which is where real seas whitecap.
pub const CHOP_LAMBDA: f32 = 1.0;
/// Jacobian threshold + sharpness: foam = clamp((THR - J) * SHARP).
pub const FOAM_J_THR: f32 = 0.90;
pub const FOAM_SHARP: f32 = 3.0;

/// One FFT ocean cascade over a periodic `tile_m` square, `n` x `n`
/// samples (n must be a power of two).
pub struct OceanFft {
    pub n: usize,
    pub tile_m: f32,
    /// Initial spectrum h0(k), row-major complex; index [ky * n + kx].
    h0: Vec<[f32; 2]>,
    /// Dispersion angular frequency per bin (deep water: w = sqrt(g k)).
    omega: Vec<f32>,
    /// Signed wave vector (ku, kv) per bin, rad/m - the spectral
    /// derivative multipliers for slopes + choppy displacement.
    kvec: Vec<[f32; 2]>,
    /// The time-evolved spectrum h(k, t) (kept - derived fields multiply
    /// it per pass).
    spec: Vec<[f32; 2]>,
    /// Scratch consumed by each derived IFFT.
    work: Vec<[f32; 2]>,
    /// The realized height grid (metres), row-major, updated by `update`.
    /// THE buoyancy array - unchanged by increment 2.
    pub height: Vec<f32>,
    /// Physical slopes (dh/du, dh/dv) per texel (increment 2, shading).
    pub slope: Vec<[f32; 2]>,
    /// Choppy horizontal displacement (du, dv) per texel - foam input
    /// only until increment 3 moves the geometry.
    disp: Vec<[f32; 2]>,
    /// Jacobian whitecap factor 0..1 per texel (increment 2).
    pub foam: Vec<f32>,
    /// Packed (height, slope_u, slope_v, foam) - the upload buffer.
    pub texels: Vec<[f32; 4]>,
}

/// Seeded xorshift64* -> uniform [0,1).
struct Rng(u64);
impl Rng {
    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / 16_777_216.0
    }
    /// Box-Muller standard normal.
    fn gauss(&mut self) -> f32 {
        let u1 = self.next_f32().max(1.0e-7);
        let u2 = self.next_f32();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

/// JONSWAP spectrum S(w) with gamma 3.3, times a cos^2 directional
/// spread about the wind direction, expressed per wave-vector bin.
fn jonswap_dir(k: glam::Vec2, wind_speed: f32, wind_dir: glam::Vec2, fetch_m: f32) -> f32 {
    let kl = k.length();
    if kl < 1.0e-4 || wind_speed < 0.1 {
        return 0.0;
    }
    let w = (G * kl).sqrt();
    // Peak frequency from wind + fetch (JONSWAP empirical form).
    let x = (G * fetch_m / (wind_speed * wind_speed)).max(1.0);
    let wp = 22.0 * (G / wind_speed) * x.powf(-0.33);
    let alpha = 0.076 * x.powf(-0.22);
    let sigma = if w <= wp { 0.07 } else { 0.09 };
    let r = (-(w - wp) * (w - wp) / (2.0 * sigma * sigma * wp * wp)).exp();
    let gamma: f32 = 3.3;
    let s_w = alpha * G * G / w.powi(5) * (-1.25 * (wp / w).powi(4)).exp() * gamma.powf(r);
    // Direction: cos^2 spread, zero energy upwind (one-sided seas).
    let c = (k / kl).dot(wind_dir);
    if c <= 0.0 {
        return 0.0;
    }
    // S(k) dk = S(w) dw conversion: dw/dk = g / (2 w).
    let jac = G / (2.0 * w);
    s_w * jac * (c * c) * (2.0 / std::f32::consts::PI) / kl
}

impl OceanFft {
    /// `lambda_range` BAND-LIMITS the spectrum (metres, exclusive min /
    /// inclusive max): bins outside get zero amplitude. The two-cascade
    /// split assigns each wavelength to exactly one cascade so their sum
    /// carries the spectrum once.
    pub fn new(
        n: usize,
        tile_m: f32,
        wind_speed: f32,
        wind_dir_rad: f32,
        fetch_m: f32,
        seed: u64,
        lambda_range: (f32, f32),
    ) -> Self {
        assert!(n.is_power_of_two(), "FFT size must be a power of two");
        let wind_dir = glam::Vec2::new(wind_dir_rad.cos(), wind_dir_rad.sin());
        let mut rng = Rng(seed | 1);
        let mut h0 = vec![[0.0f32; 2]; n * n];
        let mut omega = vec![0.0f32; n * n];
        let mut kvec = vec![[0.0f32; 2]; n * n];
        let dk = TAU / tile_m;
        for y in 0..n {
            for x in 0..n {
                // Signed wave numbers: bins above n/2 are negative freqs.
                let kx = (if x <= n / 2 { x as i32 } else { x as i32 - n as i32 }) as f32 * dk;
                let ky = (if y <= n / 2 { y as i32 } else { y as i32 - n as i32 }) as f32 * dk;
                let k = glam::Vec2::new(kx, ky);
                let kl = k.length();
                let lam = if kl > 1.0e-6 { TAU / kl } else { f32::INFINITY };
                let s = if lam > lambda_range.0 && lam <= lambda_range.1 {
                    jonswap_dir(k, wind_speed, wind_dir, fetch_m)
                } else {
                    0.0
                };
                // Bin amplitude: sqrt(S * dkx * dky / 2) per Tessendorf.
                let amp = (s * dk * dk * 0.5).sqrt();
                h0[y * n + x] = [rng.gauss() * amp, rng.gauss() * amp];
                omega[y * n + x] = (G * k.length()).sqrt();
                kvec[y * n + x] = [kx, ky];
            }
        }
        Self {
            n,
            tile_m,
            h0,
            omega,
            kvec,
            spec: vec![[0.0; 2]; n * n],
            work: vec![[0.0; 2]; n * n],
            height: vec![0.0; n * n],
            slope: vec![[0.0; 2]; n * n],
            disp: vec![[0.0; 2]; n * n],
            foam: vec![0.0; n * n],
            texels: vec![[0.0; 4]; n * n],
        }
    }

    /// Realize the sea surface at time `t` (seconds): height (buoyancy +
    /// vertex displacement), spectral slopes (shading normals), choppy
    /// displacement (Jacobian input), foam mask, packed texels.
    pub fn update(&mut self, t: f32) {
        let n = self.n;
        // h(k,t) = h0(k) e^{iwt} + h0*(-k) e^{-iwt}  (hermitian -> real field)
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let j = ((n - y) % n) * n + ((n - x) % n); // -k bin
                let (c, s) = {
                    let a = self.omega[i] * t;
                    (a.cos(), a.sin())
                };
                let a = self.h0[i];
                let b = self.h0[j];
                // a*e^{iwt} + conj(b)*e^{-iwt}
                self.spec[i] = [
                    a[0] * c - a[1] * s + b[0] * c - b[1] * s,
                    a[0] * s + a[1] * c - b[0] * s - b[1] * c,
                ];
            }
        }
        // Height: IFFT of h(k,t) directly.
        self.work.copy_from_slice(&self.spec);
        ifft2(&mut self.work, n);
        for i in 0..n * n {
            self.height[i] = self.work[i][0];
        }
        // Slopes: S_u(k) = i ku h(k), S_v(k) = i kv h(k)  (physical rad/m
        // derivative - each is hermitian because k flips sign with the
        // conjugate bin, so the realized field stays real).
        for axis in 0..2 {
            for i in 0..n * n {
                let kk = self.kvec[i][axis];
                let h = self.spec[i];
                self.work[i] = [-kk * h[1], kk * h[0]]; // i*k*h
            }
            ifft2(&mut self.work, n);
            for i in 0..n * n {
                self.slope[i][axis] = self.work[i][0];
            }
        }
        // Choppy displacement: D(k) = -i (k/|k|) h(k) per axis.
        for axis in 0..2 {
            for i in 0..n * n {
                let k = self.kvec[i];
                let kl = (k[0] * k[0] + k[1] * k[1]).sqrt();
                let f = if kl > 1.0e-6 { k[axis] / kl } else { 0.0 };
                let h = self.spec[i];
                self.work[i] = [f * h[1], -f * h[0]]; // -i*f*h
            }
            ifft2(&mut self.work, n);
            for i in 0..n * n {
                self.disp[i][axis] = self.work[i][0];
            }
        }
        // Foam: finite-difference Jacobian of (u + L*du, v + L*dv). Where
        // the mapping folds (J < threshold) crests pinch - whitecaps.
        let texel = self.tile_m / n as f32;
        let inv2 = 1.0 / (2.0 * texel);
        for y in 0..n {
            let ym = (y + n - 1) % n;
            let yp = (y + 1) % n;
            for x in 0..n {
                let xm = (x + n - 1) % n;
                let xp = (x + 1) % n;
                let i = y * n + x;
                let dux = (self.disp[y * n + xp][0] - self.disp[y * n + xm][0]) * inv2;
                let dvy = (self.disp[yp * n + x][1] - self.disp[ym * n + x][1]) * inv2;
                let duy = (self.disp[yp * n + x][0] - self.disp[ym * n + x][0]) * inv2;
                let dvx = (self.disp[y * n + xp][1] - self.disp[y * n + xm][1]) * inv2;
                let jac = (1.0 + CHOP_LAMBDA * dux) * (1.0 + CHOP_LAMBDA * dvy)
                    - (CHOP_LAMBDA * duy) * (CHOP_LAMBDA * dvx);
                self.foam[i] = ((FOAM_J_THR - jac) * FOAM_SHARP).clamp(0.0, 1.0);
            }
        }
        for i in 0..n * n {
            self.texels[i] = [
                self.height[i],
                self.slope[i][0],
                self.slope[i][1],
                self.foam[i],
            ];
        }
    }

    /// Bilinear height sample at planet-anchored metres (wraps the tile).
    /// THE buoyancy twin: reads the same array the texture uploads.
    pub fn sample(&self, x_m: f32, z_m: f32) -> f32 {
        self.tile_sample(x_m / self.tile_m, z_m / self.tile_m)
    }

    /// Bilinear sample in tile-UV space (wraps). Mirrors the shader's
    /// `fft_tile_sample` textureLoad bilinear EXACTLY - same fract, same
    /// texel corners, same lerp order.
    fn tile_sample(&self, u_in: f32, v_in: f32) -> f32 {
        let n = self.n as f32;
        let u = (u_in - u_in.floor()) * n;
        let v = (v_in - v_in.floor()) * n;
        let (x0, y0) = (u.floor() as usize % self.n, v.floor() as usize % self.n);
        let (x1, y1) = ((x0 + 1) % self.n, (y0 + 1) % self.n);
        let (fx, fy) = (u.fract(), v.fract());
        let h = |xx: usize, yy: usize| self.height[yy * self.n + xx];
        let a = h(x0, y0) * (1.0 - fx) + h(x1, y0) * fx;
        let b = h(x0, y1) * (1.0 - fx) + h(x1, y1) * fx;
        a * (1.0 - fy) + b * fy
    }

    /// Triplanar height at a camera-anchored planet-local position (the
    /// shader's `fft_ocean_height` twin). A single 2D parameterization of
    /// a sphere degenerates somewhere (xz collapses at the equator), so
    /// the tile is projected on all three axis planes and blended by the
    /// squared radial normal - like the wave trains' three axis-aligned
    /// directions, this covers every latitude. Each plane gets a fixed UV
    /// offset so the three projections don't show correlated crests.
    pub fn triplanar_height(&self, p_anch: glam::Vec3, radial: glam::Vec3) -> f32 {
        let w2 = radial * radial; // sums to 1 for a unit radial
        let q = p_anch / self.tile_m;
        w2.x * self.tile_sample(q.y + PLANE_OFF[0][0], q.z + PLANE_OFF[0][1])
            + w2.y * self.tile_sample(q.x + PLANE_OFF[1][0], q.z + PLANE_OFF[1][1])
            + w2.z * self.tile_sample(q.x + PLANE_OFF[2][0], q.y + PLANE_OFF[2][1])
    }

    /// Scale h0 so a reference realization has the given RMS height. The
    /// increment-1 toggle changes wave STRUCTURE, not the amplitude
    /// envelope the rest of the engine is tuned around (backstop offset,
    /// patch radial bands assume the trains' MAX_WAVE_HEIGHT_M budget).
    /// Deterministic: same seed + wind -> same scale factor.
    pub fn normalize_to_rms(&mut self, target: f32) {
        self.update(0.0);
        let r = self.rms();
        if r > 1.0e-6 {
            let s = target / r;
            for c in &mut self.h0 {
                c[0] *= s;
                c[1] *= s;
            }
        }
    }

    /// RMS height of the current field (test + sea-state diagnostics).
    pub fn rms(&self) -> f32 {
        (self.height.iter().map(|h| h * h).sum::<f32>() / self.height.len() as f32).sqrt()
    }
}

/// The engine's two-cascade sea (v0.1040): built together, updated
/// together, uploaded as ONE 128x256 texture (A rows then B rows).
pub struct OceanCascades {
    pub a: OceanFft,
    pub b: OceanFft,
}

impl OceanCascades {
    pub fn build(wind_speed: f32, wind_dir_rad: f32, fetch_m: f32, seed: u64) -> Self {
        let mut a = OceanFft::new(
            FFT_N,
            FFT_TILE_M,
            wind_speed,
            wind_dir_rad,
            fetch_m,
            seed,
            (0.0, FFT_SPLIT_LAMBDA_M),
        );
        a.normalize_to_rms(FFT_A_TARGET_RMS_M);
        let mut b = OceanFft::new(
            FFT_N,
            FFT_B_TILE_M,
            wind_speed,
            wind_dir_rad,
            fetch_m,
            // Different Gaussian stream so the two tiles never share a
            // crest pattern.
            seed ^ 0x9E37_79B9_7F4A_7C15,
            (FFT_SPLIT_LAMBDA_M, FFT_B_TILE_M),
        );
        b.normalize_to_rms(FFT_B_TARGET_RMS_M);
        Self { a, b }
    }

    pub fn update(&mut self, t: f32) {
        self.a.update(t);
        self.b.update(t);
    }
}

/// Planet coords mod tile in f64 FIRST (the f32-at-scale rule): the
/// remainder downcasts with micrometre precision, and the GPU's anchored
/// domain differs from it by an exact integer number of tiles.
fn mod_tile(p_m: glam::DVec3, tile: f64) -> glam::Vec3 {
    glam::Vec3::new(
        p_m.x.rem_euclid(tile) as f32,
        p_m.y.rem_euclid(tile) as f32,
        p_m.z.rem_euclid(tile) as f32,
    )
}

/// Full FFT-mode buoyancy twin: the analytic long swells plus BOTH
/// cascades, shoal-damped exactly like the trains version. The vertex
/// shader's FFT branch (`ocean_wave_height_fft`) is the drawn side; both
/// read the SAME height arrays this frame, so drawn == sampled is
/// literal (per-cascade fades are ~1 at the player, matching the twin).
pub fn wave_height_shoaled_fft_m(
    p_m: glam::DVec3,
    t: f32,
    depth_m: f32,
    c: &OceanCascades,
) -> f32 {
    let swell = crate::terrain::ocean_waves::swell_height_m(p_m, t);
    let radial = p_m.normalize().as_vec3();
    let h = c.a.triplanar_height(mod_tile(p_m, c.a.tile_m as f64), radial)
        + c.b.triplanar_height(mod_tile(p_m, c.b.tile_m as f64), radial);
    (swell + h) * crate::terrain::ocean_waves::shoal_factor(depth_m)
}

/// 2D IFFT: rows then columns, each via the radix-2 line transform.
fn ifft2(buf: &mut [[f32; 2]], n: usize) {
    let mut line = vec![[0.0f32; 2]; n];
    for y in 0..n {
        line.copy_from_slice(&buf[y * n..(y + 1) * n]);
        ifft(&mut line);
        buf[y * n..(y + 1) * n].copy_from_slice(&line);
    }
    for x in 0..n {
        for y in 0..n {
            line[y] = buf[y * n + x];
        }
        ifft(&mut line);
        for y in 0..n {
            buf[y * n + x] = line[y];
        }
    }
}

/// In-place radix-2 inverse FFT (unscaled forward with conjugated
/// twiddles; the 1/N normalization cancels against the spectrum's
/// continuous-to-discrete convention absorbed in the bin amplitudes).
fn ifft(buf: &mut [[f32; 2]]) {
    let n = buf.len();
    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            buf.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = TAU / len as f32; // +sign = inverse transform
        let (wc, ws) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = buf[i + k];
                let b = buf[i + k + len / 2];
                let br = b[0] * cr - b[1] * ci;
                let bi = b[0] * ci + b[1] * cr;
                buf[i + k] = [a[0] + br, a[1] + bi];
                buf[i + k + len / 2] = [a[0] - br, a[1] - bi];
                let (ncr, nci) = (cr * wc - ci * ws, cr * ws + ci * wc);
                cr = ncr;
                ci = nci;
            }
            i += len;
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sea(wind: f32, seed: u64) -> OceanFft {
        let mut o = OceanFft::new(64, 256.0, wind, 0.6, 200_000.0, seed, (0.0, 256.0));
        o.update(3.7);
        o
    }

    #[test]
    fn field_is_real_scaled_and_deterministic() {
        let a = sea(9.0, 42);
        let b = sea(9.0, 42);
        assert_eq!(a.height, b.height, "same seed+wind must be identical");
        let calm = sea(2.0, 42).rms();
        let storm = sea(18.0, 42).rms();
        assert!(storm > calm * 3.0, "energy must grow with wind: calm rms {calm}, storm rms {storm}");
        assert!(storm.is_finite() && storm > 0.001, "storm sea must be nonzero");
    }

    #[test]
    fn tile_is_periodic_and_sampling_matches_grid() {
        let o = sea(11.0, 7);
        // Periodicity: sampling one tile apart is identical.
        for (x, z) in [(3.0, 40.0), (100.0, 9.0), (200.0, 200.0)] {
            let d = (o.sample(x, z) - o.sample(x + 256.0, z + 256.0)).abs();
            assert!(d < 1.0e-4, "tile not periodic at ({x},{z}): {d}");
        }
        // Grid-point sampling reproduces the raw array (the twin contract).
        let texel = 256.0 / 64.0;
        let raw = o.height[5 * 64 + 9];
        let s = o.sample(9.0 * texel, 5.0 * texel);
        assert!((raw - s).abs() < 1.0e-5, "twin sample {s} != grid {raw}");
    }

    #[test]
    fn sea_animates_over_time() {
        let mut o = OceanFft::new(64, 256.0, 10.0, 0.6, 200_000.0, 5, (0.0, 256.0));
        o.update(0.0);
        let h0 = o.height.clone();
        o.update(2.0);
        let moved = o
            .height
            .iter()
            .zip(&h0)
            .filter(|(a, b)| (**a - **b).abs() > 1.0e-4)
            .count();
        assert!(moved > o.height.len() / 2, "sea barely moved: {moved} cells");
    }

    #[test]
    fn calm_wind_is_near_glass() {
        let o = sea(0.05, 3);
        assert!(o.rms() < 0.01, "no wind should be near-flat, rms {}", o.rms());
    }

    /// v0.1040 cascades: band-limiting keeps each cascade's energy inside
    /// its wavelength range (complementary bands sum to the full sea; an
    /// empty band is a flat sea), and the pair builds with the expected
    /// per-cascade RMS split.
    #[test]
    fn cascades_are_band_limited_and_normalized() {
        // An impossible band (nothing between 1 and 1.001 m at 64 m tile
        // resolution) must produce a flat sea.
        let mut empty = OceanFft::new(64, 64.0, 12.0, 0.6, 200_000.0, 7, (1.0, 1.001));
        empty.update(2.0);
        assert!(empty.rms() < 1.0e-6, "empty band leaked energy: {}", empty.rms());
        // The short band must carry LESS raw energy than the full band
        // (it is a subset of the spectrum).
        let mut short_band = OceanFft::new(64, 64.0, 12.0, 0.6, 200_000.0, 7, (0.0, 8.0));
        short_band.update(2.0);
        let mut full = OceanFft::new(64, 64.0, 12.0, 0.6, 200_000.0, 7, (0.0, 64.0));
        full.update(2.0);
        assert!(
            short_band.rms() < full.rms(),
            "band subset {} should be below full {}",
            short_band.rms(),
            full.rms()
        );
        // The engine pair: both cascades normalized to their targets.
        let mut c = OceanCascades::build(8.0, 0.6, 200_000.0, 42);
        c.update(0.0);
        assert!((c.a.rms() - FFT_A_TARGET_RMS_M).abs() < 0.01, "A rms {}", c.a.rms());
        assert!((c.b.rms() - FFT_B_TARGET_RMS_M).abs() < 0.01, "B rms {}", c.b.rms());
        assert_eq!(c.a.tile_m, FFT_TILE_M);
        assert_eq!(c.b.tile_m, FFT_B_TILE_M);
        // The layout contract: two 128x128 cascades fill the 128x256 tile.
        assert_eq!(c.a.texels.len() + c.b.texels.len(), FFT_N * FFT_TEX_H);
    }

    #[test]
    fn normalization_hits_the_trains_energy_envelope() {
        let mut o = OceanFft::new(FFT_N, FFT_TILE_M, 8.0, 0.6, 200_000.0, 42, (0.0, FFT_TILE_M));
        o.normalize_to_rms(FFT_TARGET_RMS_M);
        o.update(0.0);
        let r = o.rms();
        assert!(
            (r - FFT_TARGET_RMS_M).abs() < 0.01,
            "normalized rms {r} != target {FFT_TARGET_RMS_M}"
        );
        // Triplanar blend is bounded by the field's own extremes.
        let hmax = o.height.iter().fold(0.0f32, |m, h| m.max(h.abs()));
        for i in 0..64 {
            let p = glam::Vec3::new(i as f32 * 0.97, 61.0 - i as f32, i as f32 * 0.31);
            let radial = glam::Vec3::new(0.6, 0.64, 0.48).normalize();
            let t = o.triplanar_height(p, radial);
            assert!(t.abs() <= hmax + 1.0e-4, "triplanar {t} exceeds field max {hmax}");
        }
    }

    /// LOCKSTEP guard: the vertex shader's FFT constants must match this
    /// module exactly, or drawn != sampled. Same pattern as the trains'
    /// shader_constants_match_cpu_twin.
    #[test]
    fn shader_fft_constants_match_cpu_twin() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/shaders/pbr/00-bindings-vertex.wgsl"),
        )
        .expect("vertex shader part readable");
        assert!(src.contains("const OCEAN_FFT_TILE_M: f32 = 64.0;"), "tile const");
        assert!(src.contains("const OCEAN_FFT_B_TILE_M: f32 = 256.0;"), "B tile const");
        assert!(src.contains("const OCEAN_FFT_N: i32 = 128;"), "N const");
        assert!(src.contains("const OCEAN_FFT_ROW_B: i32 = 128;"), "row-B const");
        assert!(src.contains("const OCEAN_FFT_A_FADE_LAMBDA: f32 = 16.0;"), "A fade");
        assert!(src.contains("const OCEAN_FFT_B_FADE_LAMBDA: f32 = 96.0;"), "B fade");
        assert_eq!(FFT_TILE_M, 64.0);
        assert_eq!(FFT_B_TILE_M, 256.0);
        assert_eq!(FFT_N, 128);
        assert_eq!(FFT_TEX_H, 256);
        assert_eq!(FFT_A_FADE_LAMBDA, 16.0);
        assert_eq!(FFT_B_FADE_LAMBDA, 96.0);
        // Each tile must divide ITS anchor modulus exactly: A rides the
        // 64 m ground anchor, B rides the 256 m ocean anchor.
        assert_eq!(64.0_f32 % FFT_TILE_M, 0.0);
        assert_eq!(256.0_f32 % FFT_B_TILE_M, 0.0);
        // Per-plane UV offsets, literal-for-literal.
        assert!(src.contains("vec2<f32>(0.0, 0.0)"), "OFF_X literal");
        assert!(src.contains("vec2<f32>(0.271, 0.417)"), "OFF_Y literal");
        assert!(src.contains("vec2<f32>(0.613, 0.129)"), "OFF_Z literal");
        assert_eq!(PLANE_OFF, [[0.0, 0.0], [0.271, 0.417], [0.613, 0.129]]);
        // The FFT branch must keep the three long swells analytic, paired
        // with the same direction constants as the trains path, and feed
        // BOTH anchored positions to the cascade sum.
        let at = src.find("fn ocean_wave_height_fft").expect("fft fn present");
        let body = &src[at..at + 1200];
        for pair in ["WAVE1_DIR, OCEAN_W1_LAMBDA", "WAVE3_DIR, OCEAN_W2_LAMBDA", "WAVE4_DIR, OCEAN_W3_LAMBDA"] {
            assert!(body.contains(pair), "swell pairing missing: {pair}");
        }
        assert!(body.contains("fft_ocean_height(p64, p256, radial, cam_dist)"), "fft term present");
        // Increment 2 channel layout: VS height from .x, the per-cascade
        // shading fn maps g/b slopes onto each plane's axes + foam from .w.
        let sat = src.find("fn fft_cascade_shading").expect("shading fn");
        let sbody = &src[sat..sat + 900];
        for m in ["vec3<f32>(0.0, sx.y, sx.z)", "vec3<f32>(sy.y, 0.0, sy.z)", "vec3<f32>(sz.y, sz.z, 0.0)", "sx.w", "sy.w", "sz.w"] {
            assert!(sbody.contains(m), "shading mapping missing: {m}");
        }
        // And the fragment side actually consumes both cascades.
        let fs = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/shaders/pbr/90-fragment-main.wgsl"),
        )
        .expect("fragment shader part readable");
        assert!(fs.contains("fft_ocean_shading(ptw, ptw256, dir, fdist)"), "FS consumes FFT shading");
        assert!(fs.contains("light4_cone_inner"), "FS reads the B anchor");
        // v0.1041 geomorph weld: the VS must morph odd verts toward their
        // parents' mean through the SAME dispatch helper it displaces
        // with, and the weld window constants must be present.
        assert!(src.contains("fn water_disp_height"), "dispatch helper present");
        // The weld window is in units of cell * K (K = px_per_rad /
        // split_px, fed per frame via light4_cone_inner.w) - derived from
        // screen_error_px's 1.15/0.7 hysteresis, NOT guessed (the guessed
        // version collapsed every patch a level early: giant plates).
        assert!(src.contains("const WATER_WELD_MORPH_START: f32 = 1.43;"), "weld start");
        assert!(src.contains("const WATER_WELD_MORPH_END: f32 = 1.74;"), "weld end");
        assert!(src.contains("weld_k = camera.light4_cone_inner.w"), "weld K from uniform");
        assert!(src.contains("weld_k > 1.0"), "weld disabled until K arrives");
        // v0.1043: the weld must move the BASE sphere position too - the
        // coarse neighbor's edge is a chord sagging |delta|^2/(2r) inside
        // the sphere, and morphing only the wave height left exactly that
        // gap at every LOD border (the operator's dusk seam). The term
        // must sit OUTSIDE the wave-fade branch: far patches are exact
        // spheres and exact spheres still crack.
        assert!(
            src.contains("var disp = -weld_w * (dl * dl) / (2.0 * r);"),
            "chord-sag morph present"
        );
        let vs_at = src.find("var disp = -weld_w").expect("sag term");
        let fade_at = src.find("let fade = 1.0 - smoothstep(2000.0, 8000.0, cam_dist);");
        assert!(
            fade_at.is_some_and(|f| f > vs_at),
            "chord sag must be applied BEFORE (outside) the wave fade gate"
        );
        // v0.1044: the WAVE-height morph is deliberately GONE (it halved
        // wave resolution over whole patches - the operator's flat blue
        // plates at water level). Only the chord-sag term welds now, and
        // it must stay that way unless the lattice is made subset-aligned
        // across LOD levels first.
        assert!(
            !src.contains("h = mix(h, 0.5 * hp, weld_w);"),
            "wave-height morph must NOT come back (it plates the sea)"
        );
    }

    /// Increment 2: spectral slopes must agree with finite differences of
    /// the realized height field (the FD estimate under-reads the
    /// shortest waves - sinc rolloff - so the bar is a loose relative
    /// RMS, not equality).
    #[test]
    fn spectral_slopes_match_height_differences() {
        let mut o = OceanFft::new(FFT_N, FFT_TILE_M, 8.0, 0.6, 200_000.0, 11, (0.0, FFT_TILE_M));
        o.update(2.5);
        let n = o.n;
        let texel = o.tile_m / n as f32;
        let (mut err2, mut mag2) = (0.0f64, 0.0f64);
        for y in 0..n {
            for x in 0..n {
                let xp = (x + 1) % n;
                let xm = (x + n - 1) % n;
                let fd = (o.height[y * n + xp] - o.height[y * n + xm]) / (2.0 * texel);
                let sp = o.slope[y * n + x][0];
                err2 += ((sp - fd) as f64).powi(2);
                mag2 += (sp as f64).powi(2);
            }
        }
        let rel = (err2 / mag2.max(1.0e-12)).sqrt();
        assert!(rel < 0.35, "spectral vs FD slope relative RMS {rel}");
        assert!(mag2 > 0.0, "slopes must be nonzero");
    }

    /// Increment 2: foam is a bounded mask that grows with wind, and the
    /// packed texels mirror the four source arrays exactly.
    #[test]
    fn foam_bounded_grows_with_wind_and_texels_pack() {
        let calm = sea(4.0, 9);
        let storm = sea(19.0, 9);
        for f in calm.foam.iter().chain(storm.foam.iter()) {
            assert!((0.0..=1.0).contains(f), "foam {f} out of range");
        }
        let mean = |o: &OceanFft| o.foam.iter().sum::<f32>() / o.foam.len() as f32;
        assert!(
            mean(&storm) > mean(&calm),
            "storm foam {} should exceed calm {}",
            mean(&storm),
            mean(&calm)
        );
        for i in [0usize, 77, 4095] {
            let t = storm.texels[i];
            assert_eq!(t[0], storm.height[i], "texel h");
            assert_eq!(t[1], storm.slope[i][0], "texel su");
            assert_eq!(t[2], storm.slope[i][1], "texel sv");
            assert_eq!(t[3], storm.foam[i], "texel foam");
        }
    }
}
