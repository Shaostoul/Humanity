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
}
