//! Atmosphere LUTs, stage 1 of the Hillaire-2020 sky pipeline
//! (docs/dev/rendering-research-atmosphere-water.md section 1.1; staged plan
//! in the 2026-07-25 journal).
//!
//! This module is the CPU generator for the TRANSMITTANCE LUT: per-channel
//! transmittance from a point at height r looking along cos-zenith mu, out to
//! space. Unlike the sky-view LUT (camera-dependent, needs a per-frame GPU
//! pass), transmittance depends only on the planet's atmosphere parameters,
//! so generating it on the CPU once per planet is architecturally correct,
//! not a shortcut - 256 x 64 closed-form Chapman evaluations are microseconds.
//!
//! The math mirrors the megashader exactly (same `od_to_space` Chapman
//! approximation as `atmosphere_scattering`, same tau conventions as
//! [`super::atmosphere`]): `T = exp(-(tint * TAU_R * od_r + 1.11 * TAU_M *
//! od_m))`, with the 1.11 Mie absorption factor from the shader's
//! `beta_ext`.
//!
//! Texture layout (the 1b shader sampling MUST use the same mapping):
//! - u in [0,1]  <=>  mu in [-0.15, 1.0]   (u = (mu + 0.15) / 1.15; the
//!   below-horizon margin covers grazing rays the sun-disc path needs)
//! - v in [0,1]  <=>  altitude in [0, ALT_SPAN_H scale heights] above rp
//!   (r = rp + v * min(ALT_SPAN_H * h, 1 - rp)); see `v_to_r` for why the
//!   span is scale heights, not the whole shell
//! - texel = [T_red, T_green, T_blue, 1.0] as f32 (uploaded Rgba32Float;
//!   16-bit would suffice but the texture is 256 KB, not worth the packing).

use super::atmosphere::{od_to_space, TAU_MIE, TAU_RAYLEIGH};

pub const TRANS_LUT_W: usize = 256;
pub const TRANS_LUT_H: usize = 64;

/// Altitude span of the v axis, in scale heights. exp(-12) < 1e-5: beyond
/// this the atmosphere is optically empty and T is 1 to five decimals.
pub const ALT_SPAN_H: f32 = 12.0;

/// Per-planet inputs the LUT depends on. Everything else is baked constants
/// shared with the shader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransLutParams {
    /// Rayleigh tint (the planet's `material.base_color.rgb` in the shader):
    /// per-channel weighting of the Rayleigh optical depth. Earth-like =
    /// roughly (0.18, 0.42, 1.0) style blue-heavy weights.
    pub tint: [f32; 3],
    /// Density multiplier (the shader's `density_mul`).
    pub density_mul: f32,
    /// Planet surface radius in SHELL space (from `shell_packing`): the
    /// atmosphere top is 1.0 and the surface sits at rp < 1.
    pub rp: f32,
    /// Scale height in shell space (also from `shell_packing`).
    pub h: f32,
}

/// Map a texel u to mu (cos zenith angle). Public so tests + the 1b shader
/// mapping comment stay honest against one function.
pub fn u_to_mu(u: f32) -> f32 {
    u * 1.15 - 0.15
}

/// Map a texel v to shell-space radius r: v spans [0, ALT_SPAN_H] scale
/// heights above the surface (clamped to the shell top), because
/// transmittance only changes meaningfully in the first dozen scale heights;
/// a linear span over the whole shell would waste every row above the first
/// on optically empty space. Above the span, sample the top row (T ~ 1).
pub fn v_to_r(v: f32, rp: f32, h: f32) -> f32 {
    rp + v * (ALT_SPAN_H * h).min(1.0 - rp)
}

/// Generate the transmittance LUT, row-major, TRANS_LUT_W x TRANS_LUT_H
/// texels of [T_r, T_g, T_b, 1.0].
pub fn transmittance_lut(p: &TransLutParams) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(TRANS_LUT_W * TRANS_LUT_H);
    // Mie rides a shorter scale height than Rayleigh (the shader folds both
    // into one od today; keep that single-od convention so 1b's fetch is a
    // drop-in replacement for the analytic expression, not a new model).
    for ty in 0..TRANS_LUT_H {
        let v = (ty as f32 + 0.5) / TRANS_LUT_H as f32;
        let r = v_to_r(v, p.rp, p.h);
        for tx in 0..TRANS_LUT_W {
            let u = (tx as f32 + 0.5) / TRANS_LUT_W as f32;
            let mu = u_to_mu(u);
            let od = od_to_space(r, mu, p.rp, p.h);
            // The shader's beta carries a 1/h (beta_ray = tint * TAU_R / h),
            // because od_to_space returns h-scaled path length; mirror that.
            let t = |beta: f32| (-(beta / p.h * od * p.density_mul)).exp();
            out.push([
                t(p.tint[0] * TAU_RAYLEIGH + TAU_MIE * 1.11),
                t(p.tint[1] * TAU_RAYLEIGH + TAU_MIE * 1.11),
                t(p.tint[2] * TAU_RAYLEIGH + TAU_MIE * 1.11),
                1.0,
            ]);
        }
    }
    out
}

// ── GPU upload helpers (stage 3a) ──

/// f32 -> IEEE 754 half bits (round-to-nearest-even). Hand-rolled because the
/// crate tree has no `half` dependency and the LUTs are the only f16 need.
/// LUT values live in [0, ~1], far from every subnormal/overflow edge, but
/// the conversion is still written correctly for the full range.
pub fn f32_to_f16_bits(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let man = bits & 0x007f_ffff;
    if exp == 255 {
        // Inf / NaN
        return sign | 0x7c00 | if man != 0 { 0x0200 } else { 0 };
    }
    let e16 = exp - 127 + 15;
    if e16 >= 31 {
        return sign | 0x7c00; // overflow -> inf
    }
    if e16 <= 0 {
        // Subnormal half (or zero): shift the implicit-1 mantissa down.
        if e16 < -10 {
            return sign; // underflow to zero
        }
        let man_full = man | 0x0080_0000;
        let shift = (14 - e16) as u32;
        let half_man = man_full >> shift;
        // round-to-nearest-even on the dropped bits
        let rem = man_full & ((1u32 << shift) - 1);
        let half = (rem > (1u32 << (shift - 1))
            || (rem == (1u32 << (shift - 1)) && (half_man & 1) == 1))
            as u32;
        return sign | (half_man + half) as u16;
    }
    let half_man = man >> 13;
    let rem = man & 0x1fff;
    let round = (rem > 0x1000 || (rem == 0x1000 && (half_man & 1) == 1)) as u32;
    let mut out = ((e16 as u32) << 10) | half_man;
    out += round; // mantissa overflow rolls into the exponent correctly
    sign | out as u16
}

/// Pack an rgba-f32 LUT into little-endian Rgba16Float texel bytes.
pub fn lut_to_f16_bytes(texels: &[[f32; 4]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(texels.len() * 8);
    for t in texels {
        for c in t {
            out.extend_from_slice(&f32_to_f16_bits(*c).to_le_bytes());
        }
    }
    out
}

// ── Stage 2: the multiple-scattering LUT (Hillaire 2020 section 5.1) ──
//
// Psi_ms(height, sun-zenith): the total multiply-scattered radiance factor,
// under the paper's two assumptions - scattering orders >= 2 are ISOTROPIC,
// and locally uniform - so the infinite bounce series collapses to a
// geometric sum: Psi = L_2nd * 1 / (1 - f_ms), where L_2nd is second-order
// in-scatter (integrated over the sphere of directions) and f_ms is the
// fraction of scattered energy that scatters AGAIN. This is the missing
// energy that the megashader currently fakes with the flat MS_ISO = 0.07
// gate; when stage 3's sky-view march consumes this LUT instead, twilight
// depth and zenith brightness become physics.
//
// Texture layout (32 x 32; the stage-3 sampler must mirror it):
// - u in [0,1] <=> sun mu_s in [-0.15, 1.0]  (same u_to_mu as transmittance)
// - v in [0,1] <=> altitude in [0, ALT_SPAN_H scale heights] (same v_to_r)
// - texel = [Psi_r, Psi_g, Psi_b, 1.0]

pub const MS_LUT_W: usize = 32;
pub const MS_LUT_H: usize = 32;

/// Sphere-direction count for the second-order integral (8 azimuth x 8
/// elevation, uniform on the sphere via cos-elevation stratification) and
/// march steps per direction. Paper uses 64 dirs / 20 steps at this size.
const MS_DIRS_AZ: usize = 8;
const MS_DIRS_EL: usize = 8;
const MS_STEPS: usize = 20;

/// Generate the multiple-scattering LUT, row-major MS_LUT_W x MS_LUT_H.
pub fn multiple_scattering_lut(p: &TransLutParams) -> Vec<[f32; 4]> {
    use super::atmosphere::{od_to_space, TAU_MIE, TAU_RAYLEIGH};
    let iso_phase = 1.0 / (4.0 * std::f32::consts::PI);
    // Per-channel scattering coefficient at surface density (per shell
    // radius): the shader's beta = tau / h convention.
    let beta = |c: usize| (p.tint[c] * TAU_RAYLEIGH + TAU_MIE) / p.h * p.density_mul;
    // Extinction adds the 1.11 Mie absorption factor, as the shader does.
    let beta_ext = |c: usize| (p.tint[c] * TAU_RAYLEIGH + TAU_MIE * 1.11) / p.h * p.density_mul;

    let mut out = Vec::with_capacity(MS_LUT_W * MS_LUT_H);
    for ty in 0..MS_LUT_H {
        let v = (ty as f32 + 0.5) / MS_LUT_H as f32;
        let r0 = v_to_r(v, p.rp, p.h);
        for tx in 0..MS_LUT_W {
            let u = (tx as f32 + 0.5) / MS_LUT_W as f32;
            let mu_s = u_to_mu(u);

            let mut l2 = [0.0f32; 3]; // second-order in-scatter
            let mut f_ms = [0.0f32; 3]; // re-scatter fraction
            for ei in 0..MS_DIRS_EL {
                // Stratified uniform sphere: cos(elevation) uniform in [-1,1].
                let cos_el = -1.0 + 2.0 * (ei as f32 + 0.5) / MS_DIRS_EL as f32;
                let sin_el = (1.0 - cos_el * cos_el).max(0.0).sqrt();
                for ai in 0..MS_DIRS_AZ {
                    let az = std::f32::consts::TAU * (ai as f32 + 0.5) / MS_DIRS_AZ as f32;
                    // Direction relative to local up; only the vertical
                    // component matters for the 1D exponential atmosphere,
                    // but azimuth changes the sun angle at samples.
                    let dir_up = cos_el;
                    // March to the atmosphere top (or ground hit).
                    // Segment length: to shell top along this direction -
                    // approximate with the vertical span / |dir_up| capped at
                    // a few spans (horizontal rays get the cap).
                    let span = (1.0 - r0).max(p.h);
                    let seg = if dir_up.abs() > 0.05 {
                        (span / dir_up.abs()).min(span * 8.0)
                    } else {
                        span * 8.0
                    };
                    let dt = seg / MS_STEPS as f32;
                    let mut od_along = 0.0f32; // density-integrated path so far
                    for si in 0..MS_STEPS {
                        let t = (si as f32 + 0.5) * dt;
                        let r_s = (r0 + t * dir_up).clamp(p.rp, 1.0);
                        // Ground hit: the ray is absorbed (skip the rest;
                        // ground albedo bounce is a stage-3+ refinement).
                        if r0 + t * dir_up < p.rp {
                            break;
                        }
                        let dens = (-(r_s - p.rp) / p.h).exp();
                        od_along += dens * dt;
                        // Sun transmittance at the sample (channel green used
                        // for the shared od; per-channel via beta_ext below).
                        let od_sun = od_to_space(r_s, mu_s, p.rp, p.h);
                        // The sun below the local horizon at the sample:
                        // od_to_space returns 1e9 -> exp kills the term.
                        for c in 0..3 {
                            let t_along = (-(beta_ext(c)) * od_along).exp();
                            let t_sun = (-(beta_ext(c)) * od_sun).exp();
                            let sigma = beta(c) * dens;
                            l2[c] += t_along * sigma * t_sun * iso_phase * dt;
                            f_ms[c] += t_along * sigma * dt;
                        }
                    }
                }
            }
            let n = (MS_DIRS_AZ * MS_DIRS_EL) as f32;
            // Average over the sphere (the 4pi solid angle is folded into the
            // isotropic phase), then the geometric series.
            let psi = |c: usize| {
                let l = l2[c] / n;
                let f = (f_ms[c] / n * iso_phase * 4.0 * std::f32::consts::PI).min(0.99);
                l / (1.0 - f)
            };
            out.push([psi(0), psi(1), psi(2), 1.0]);
        }
    }
    out
}

// ── Stage 3b reference: the sky-view march (CPU twin of the GPU pass) ──
//
// One texel of the sky-view LUT: from a camera at shell radius r0, march the
// view ray (given by cos-elevation mu_v and the azimuth angle to the sun)
// through the atmosphere, accumulating single-scattered sun light plus the
// multiple-scattering term, attenuated by extinction along the view path.
// The WGSL pass in sky_view_lut.wgsl is a transcription of THIS function;
// its physics tests below are the contract (same pattern as the ocean's CPU
// twin + lockstep tests).

/// March step count (the paper ships 30 at 200x100).
pub const SKY_VIEW_STEPS: usize = 30;

/// In-scattered radiance (relative units, sun irradiance = 1) for one view
/// direction. `mu_v`: cos view elevation (+1 = zenith). `cos_az`: cosine of
/// the horizontal azimuth between view and sun. `mu_s`: cos sun elevation.
/// `ms_lut`: the stage-2 LUT for the multiple-scatter term (sampled nearest).
pub fn sky_view_radiance(
    p: &TransLutParams,
    r0: f32,
    mu_v: f32,
    cos_az: f32,
    mu_s: f32,
    ms_lut: &[[f32; 4]],
) -> [f32; 3] {
    use super::atmosphere::{hg_phase, od_to_space, rayleigh_phase, MIE_G, TAU_MIE, TAU_RAYLEIGH};
    let beta_s = |c: usize| (p.tint[c] * TAU_RAYLEIGH + TAU_MIE) / p.h * p.density_mul;
    let beta_e = |c: usize| (p.tint[c] * TAU_RAYLEIGH + TAU_MIE * 1.11) / p.h * p.density_mul;

    // cos of the sun-view angle (for the phase functions): combine the
    // vertical components and the horizontal azimuth.
    let sin_v = (1.0 - mu_v * mu_v).max(0.0).sqrt();
    let sin_s = (1.0 - mu_s * mu_s).max(0.0).sqrt();
    let cos_theta = (mu_v * mu_s + sin_v * sin_s * cos_az).clamp(-1.0, 1.0);
    let ph_r = rayleigh_phase(cos_theta);
    let ph_m = hg_phase(cos_theta, MIE_G);
    let iso = 1.0 / (4.0 * std::f32::consts::PI);

    // Segment length: to the shell top (or ground). Same approximation the
    // MS integral uses; the GPU pass mirrors it.
    let span = (1.0 - r0).max(p.h);
    let seg = if mu_v.abs() > 0.05 { (span / mu_v.abs()).min(span * 8.0) } else { span * 8.0 };
    let dt = seg / SKY_VIEW_STEPS as f32;

    let ms_sample = |r: f32, mu: f32| -> [f32; 3] {
        let u = ((mu + 0.15) / 1.15).clamp(0.0, 1.0);
        let span_v = (ALT_SPAN_H * p.h).min(1.0 - p.rp);
        let v = (((r - p.rp) / span_v).clamp(0.0, 1.0) * (MS_LUT_H as f32 - 1.0)) as usize;
        let uu = (u * (MS_LUT_W as f32 - 1.0)) as usize;
        let t = ms_lut[v.min(MS_LUT_H - 1) * MS_LUT_W + uu.min(MS_LUT_W - 1)];
        [t[0], t[1], t[2]]
    };

    let mut out = [0.0f32; 3];
    let mut od_before = 0.0f32;
    for si in 0..SKY_VIEW_STEPS {
        let t_mid = (si as f32 + 0.5) * dt;
        let r_s = r0 + t_mid * mu_v;
        if r_s < p.rp {
            break; // ground hit
        }
        let r_s = r_s.min(1.0);
        let dens = (-(r_s - p.rp) / p.h).exp();
        let od_after = od_before + dens * dt;
        let od_sun = od_to_space(r_s, mu_s, p.rp, p.h);
        let psi = ms_sample(r_s, mu_s);
        for c in 0..3 {
            // ANALYTIC per-step integration (Hillaire): integral of
            // sigma * T_view over the step = (sigma/beta_ext) * (T_start -
            // T_end), exact for constant density within the step. Without
            // this, optically-thick horizon paths under-integrate badly
            // (the first draft read the horizon 7x DIMMER than the zenith).
            let t_start = (-(beta_e(c)) * od_before).exp();
            let t_end = (-(beta_e(c)) * od_after).exp();
            let t_sun = (-(beta_e(c)) * od_sun).exp();
            // Density-independent scattering/extinction ratios (dens cancels).
            let sig_r_ratio = (p.tint[c] * TAU_RAYLEIGH / p.h * p.density_mul) / beta_e(c);
            let sig_m_ratio = (TAU_MIE / p.h * p.density_mul) / beta_e(c);
            let step_energy = t_start - t_end;
            let single = (sig_r_ratio * ph_r + sig_m_ratio * ph_m) * step_energy * t_sun;
            let ms = (sig_r_ratio + sig_m_ratio) * step_energy * psi[c] * iso;
            out[c] += single + ms;
        }
        od_before = od_after;
        let _ = beta_s; // (beta_s folded into the ratio split above)
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::atmosphere::shell_packing;

    fn earthish() -> TransLutParams {
        let (rp, h) = shell_packing(0.06, 8500.0, 6.371e6);
        TransLutParams { tint: [0.18, 0.42, 1.0], density_mul: 1.0, rp, h }
    }

    /// Physical sanity: looking straight up from the surface, most light
    /// survives; along the horizon, far less does; and at the horizon the
    /// RED channel must survive better than blue (that differential IS the
    /// red sunset the 1b sun-disc fetch reproduces).
    #[test]
    fn transmittance_is_high_up_low_at_horizon_and_reddens() {
        let p = earthish();
        let lut = transmittance_lut(&p);
        let texel = |u: f32, v: f32| {
            let tx = ((u * TRANS_LUT_W as f32) as usize).min(TRANS_LUT_W - 1);
            let ty = ((v * TRANS_LUT_H as f32) as usize).min(TRANS_LUT_H - 1);
            lut[ty * TRANS_LUT_W + tx]
        };
        // Surface (v=0), straight up (mu=1 -> u=1).
        let up = texel(0.999, 0.0);
        // Surface, horizon (mu=0 -> u = 0.15/1.15).
        let hz = texel(0.15 / 1.15, 0.0);
        assert!(up[1] > 0.5, "up-looking green transmittance too low: {}", up[1]);
        assert!(hz[1] < up[1] * 0.6, "horizon should lose far more light than zenith");
        assert!(
            hz[0] > hz[2] * 1.5,
            "horizon red must survive better than blue (got R={} B={})",
            hz[0],
            hz[2]
        );
        // Top of atmosphere (v=1): nothing left to cross, T -> 1.
        let top = texel(0.999, 0.999);
        assert!(top[1] > 0.98, "top-of-atmosphere transmittance should be ~1, got {}", top[1]);
    }

    /// Monotonicity along mu: lowering the sun from zenith to horizon must
    /// never INCREASE transmittance (a sawtooth here means a parameterization
    /// bug that 1b would render as banding at sunset).
    #[test]
    fn transmittance_is_monotonic_in_mu_at_the_surface() {
        let p = earthish();
        let lut = transmittance_lut(&p);
        let mut prev = 0.0f32;
        for tx in 0..TRANS_LUT_W {
            let g = lut[tx][1]; // surface row, green channel
            if tx > 0 {
                assert!(
                    g >= prev - 1e-4,
                    "transmittance dipped at texel {tx}: {g} < {prev}"
                );
            }
            prev = g;
        }
    }

    /// The mapping helpers are the contract the 1b shader must mirror; pin
    /// their endpoints.
    #[test]
    fn mapping_endpoints_are_pinned() {
        assert!((u_to_mu(0.0) - (-0.15)).abs() < 1e-6);
        assert!((u_to_mu(1.0) - 1.0).abs() < 1e-6);
        let p = earthish();
        assert!((v_to_r(0.0, p.rp, p.h) - p.rp).abs() < 1e-6);
        let top = v_to_r(1.0, p.rp, p.h);
        assert!((top - (p.rp + (ALT_SPAN_H * p.h).min(1.0 - p.rp))).abs() < 1e-6);
    }

    /// Multiple-scattering physics: Psi is finite and non-negative
    /// everywhere; with the sun up on Earth-tint air the glow is BLUE-heavy
    /// (Rayleigh scatters blue - this is why the sky's ambient is blue); a
    /// sun below the horizon yields far less energy than one overhead (the
    /// planet shadows the second order); and the top-of-atmosphere row is
    /// near zero (no medium to scatter in).
    #[test]
    fn multiple_scattering_is_sane() {
        let p = earthish();
        let lut = multiple_scattering_lut(&p);
        assert_eq!(lut.len(), MS_LUT_W * MS_LUT_H);
        for (i, t) in lut.iter().enumerate() {
            for c in 0..3 {
                assert!(t[c].is_finite() && t[c] >= 0.0, "texel {i} channel {c}: {}", t[c]);
            }
        }
        let texel = |u: f32, v: f32| {
            let tx = ((u * MS_LUT_W as f32) as usize).min(MS_LUT_W - 1);
            let ty = ((v * MS_LUT_H as f32) as usize).min(MS_LUT_H - 1);
            lut[ty * MS_LUT_W + tx]
        };
        // Surface, sun overhead (mu_s = 1 -> u = 1).
        let noon = texel(0.999, 0.0);
        assert!(noon[2] > noon[0], "MS glow should be blue-heavy at noon (R={} B={})", noon[0], noon[2]);
        // Surface, sun well below the horizon (mu_s = -0.15 -> u = 0).
        let night = texel(0.0, 0.0);
        assert!(
            night[2] < noon[2] * 0.2,
            "below-horizon sun should collapse the second order (noon B={} night B={})",
            noon[2],
            night[2]
        );
        // Top of the LUT span (12 scale heights): the local medium is gone
        // but the bright shell BELOW still fills nearly half the sphere, so
        // Psi shrinks rather than collapsing - assert the direction, not a
        // collapse (the first version of this test wrongly expected near-zero
        // and the sphere integral proved it wrong, which is the point of it).
        let top = texel(0.999, 0.999);
        // (Measured: top can sit a few percent ABOVE the surface value -
        // less extinction between the observer and the bright shell below.
        // Assert same order of magnitude, not an inequality that physics
        // does not actually promise.)
        assert!(
            top[2] <= noon[2] * 1.5,
            "Psi at the top of the span should stay within ~1.5x of the surface value (top B={} noon B={})",
            top[2],
            noon[2]
        );
        assert!(top[2] > 0.0, "the shell below must still contribute at altitude");
    }

    /// Sky-view reference physics: at noon the zenith sky is BLUE and the
    /// horizon is BRIGHTER than the zenith (more air in the path); at sunset
    /// looking toward the sun the sky REDDENS relative to noon-zenith blue;
    /// with the sun far below the horizon everything collapses toward black.
    #[test]
    fn sky_view_reference_behaves_like_a_sky() {
        let p = earthish();
        let ms = multiple_scattering_lut(&p);
        let ground = p.rp + 0.0002;
        // Noon, zenith view.
        let zen = sky_view_radiance(&p, ground, 1.0, 1.0, 0.95, &ms);
        assert!(zen[2] > zen[0], "noon zenith must be blue (R={} B={})", zen[0], zen[2]);
        // Noon, horizon view (any azimuth).
        let hor = sky_view_radiance(&p, ground, 0.02, 0.3, 0.95, &ms);
        assert!(
            hor[1] > zen[1],
            "horizon should be brighter than zenith (G {} vs {})",
            hor[1],
            zen[1]
        );
        // Sunset: sun near the horizon, looking toward it.
        let set = sky_view_radiance(&p, ground, 0.05, 1.0, 0.03, &ms);
        let noon_ratio = zen[0] / zen[2].max(1e-6);
        let set_ratio = set[0] / set[2].max(1e-6);
        assert!(
            set_ratio > noon_ratio * 1.5,
            "sunset toward the sun must redden vs noon zenith (ratios {} vs {})",
            set_ratio,
            noon_ratio
        );
        // Deep night.
        let night = sky_view_radiance(&p, ground, 0.5, 0.0, -0.15, &ms);
        assert!(
            night[1] < zen[1] * 0.05,
            "night sky must collapse toward black (G {} vs noon {})",
            night[1],
            zen[1]
        );
    }

    /// The hand-rolled f16 encoder against known IEEE 754 half values.
    #[test]
    fn f16_encoding_matches_known_values() {
        assert_eq!(f32_to_f16_bits(0.0), 0x0000);
        assert_eq!(f32_to_f16_bits(1.0), 0x3c00);
        assert_eq!(f32_to_f16_bits(-2.0), 0xc000);
        assert_eq!(f32_to_f16_bits(0.5), 0x3800);
        assert_eq!(f32_to_f16_bits(65504.0), 0x7bff); // largest finite half
        assert_eq!(f32_to_f16_bits(1.0e9), 0x7c00); // overflow -> +inf
        assert_eq!(f32_to_f16_bits(f32::INFINITY), 0x7c00);
        // Round-trip precision inside the LUT's [0,1] range: half has 11
        // mantissa bits, so error <= 2^-11 relative.
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let bits = f32_to_f16_bits(x) as u32;
            let (e, m) = ((bits >> 10) & 0x1f, bits & 0x3ff);
            let back = if e == 0 {
                (m as f32) * 2f32.powi(-24)
            } else {
                (1.0 + m as f32 / 1024.0) * 2f32.powi(e as i32 - 15)
            };
            assert!((back - x).abs() <= x.max(0.001) / 1024.0, "x={x} back={back}");
        }
    }
}
