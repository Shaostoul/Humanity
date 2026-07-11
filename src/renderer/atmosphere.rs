//! Analytic atmosphere scattering: the Rust mirror of the WGSL math.
//!
//! The actual rendering lives in `assets/shaders/pbr_simple.wgsl` (material
//! type 14, `atmosphere_scattering`). This module exists for three reasons:
//!
//! 1. **Material packing** (`shell_packing`): lib.rs squeezes the physical
//!    parameters into the two unused material-uniform slots (metallic ->
//!    planet/shell radius ratio, roughness -> scale height / shell radius),
//!    so no bind-group layout, pipeline, or uniform struct changes were
//!    needed (the v0.782 device-limit incident is why we avoid layout churn).
//! 2. **Testable mirrors** of every pure shader helper (Chapman optical
//!    depth, phase functions, the color -> coefficient mapping), verified
//!    against brute-force numeric references. A WGSL-only formula would be
//!    correct-by-eyeball at best.
//! 3. **One documented home** for the scattering model and its constants;
//!    keep the `ATMO_*` constants below byte-identical with the WGSL.
//!
//! ## The model (O'Neil-class single scattering)
//!
//! Per fragment on the oversized shell sphere, the shader marches
//! `ATMO_SAMPLES` midpoints along the view ray's intersection with the shell
//! (clamped to start at the camera when inside, clipped at the planet
//! surface). At each sample the transmittance toward the sun uses the
//! ANALYTIC Chapman-function optical depth (`od_to_space`) instead of a
//! nested sampling loop -- that single substitution is what makes the whole
//! thing one cheap pass instead of an O(N^2) march or a Bruneton-style LUT
//! bake. Approximations, stated honestly:
//!
//! - Single scattering only: no multiple-scatter ambient term, so twilight
//!   is a little darker than reality. Acceptable at gameplay scales.
//! - The Chapman closed form is a few-percent approximation of the true
//!   integral (tested below at < 5% relative in the regimes we sample).
//! - One exponential density profile shared by Rayleigh and Mie (real Mie
//!   hugs the ground with its own smaller scale height); the Mie term here
//!   is a small stylistic forward-glow, not an aerosol simulation.
//! - Fixed-function alpha blending forces a GRAY background transmittance
//!   (the mean of the per-channel values), so the sunset reddening of the
//!   SURFACE seen through the limb is approximated; the in-scattered color
//!   itself is fully chromatic.
//!
//! ## Color -> coefficient mapping (data-driven planets)
//!
//! `atmosphere_color.rgb` in `data/planets/<id>.ron` is treated as RELATIVE
//! per-channel scattering strengths (linear space), and `.a` as an overall
//! density multiplier: per-channel vertical optical depth
//! `tau_i = rgb_i * a * TAU_RAYLEIGH`, and the scattering coefficient is
//! `beta_i = tau_i / H` (an exponential profile integrates vertically to
//! exactly `beta * H`). Earth ships `(0.17, 0.41, 1.0, 0.5)` which lands at
//! `tau = (0.051, 0.123, 0.30)`, matching real Rayleigh depths of roughly
//! `(0.05, 0.12, 0.28)`; Mars' red-dominant color scatters red hardest and
//! stays butterscotch. Any modded planet gets a plausible sky from its color
//! alone -- blue-ish colors behave like clean air, red-ish like dust.

/// Mirrors `ATMO_TAU_RAYLEIGH` in pbr_simple.wgsl: vertical optical depth of
/// a 1.0-strength color channel at density (alpha) 1.0.
pub const TAU_RAYLEIGH: f32 = 0.6;
/// Mirrors `ATMO_TAU_MIE`: gray aerosol vertical depth at density 1.0.
pub const TAU_MIE: f32 = 0.02;
/// Mirrors `ATMO_MIE_G`: Henyey-Greenstein forward-lobe asymmetry.
pub const MIE_G: f32 = 0.76;
/// Earth's density scale height as a fraction of its radius (8.5 km over
/// 6371 km). Planets without an explicit `scale_height_m` get this RATIO
/// applied to their own radius, so a modded planet with an atmosphere color
/// gets Earth-like proportions without hand-tuning.
pub const EARTH_SCALE_HEIGHT_RATIO: f64 = 8_500.0 / 6_371_000.0;

/// Pack the physical atmosphere parameters for the type-14 material slots.
///
/// Returns `(rp_ratio, h_rel)` where `rp_ratio` = planet radius / shell
/// radius and `h_rel` = scale height / shell radius. Both are RATIOS of the
/// drawn shell, which makes the shader invariant to the far-body disc-size
/// floor in lib.rs (a floored disc inflates planet and shell together, so
/// the ratios -- and therefore the look -- never change).
///
/// The `atmosphere_scale.max(0.005)` clamp MUST match the shell-mesh scale
/// expression in lib.rs (`1.0 + atmosphere_scale.max(0.005) * 2.0`) or the
/// shader's idea of the planet surface would drift off the drawn geometry.
pub fn shell_packing(atmosphere_scale: f32, scale_height_m: f32, radius_m: f64) -> (f32, f32) {
    let rp_ratio = 1.0 / (1.0 + atmosphere_scale.max(0.005) * 2.0);
    let h_rel = (scale_height_m as f64 / radius_m) as f32 * rp_ratio;
    (rp_ratio, h_rel)
}

/// Mirrors `atmo_erfcx` in pbr_simple.wgsl: the scaled complementary error
/// function `erfcx(z) = exp(z^2) * erfc(z)` for `z >= 0`. Two branches,
/// both sub-percent: Abramowitz-Stegun 7.1.26 for small z (its `exp(-z^2)`
/// factor cancels ours exactly; but its ABSOLUTE 1.5e-7 erfc error becomes
/// a huge RELATIVE error at large z once scaled by `exp(z^2)`, hence the
/// switch) and the 3-term asymptotic series beyond z = 2.5.
pub fn erfcx(z: f32) -> f32 {
    if z <= 2.5 {
        let t = 1.0 / (1.0 + 0.327_591_1 * z);
        return t
            * (0.254_829_592
                + t * (-0.284_496_736
                    + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    }
    let inv_z2 = 1.0 / (z * z);
    0.564_189_6 / z * (1.0 + inv_z2 * (-0.5 + 0.75 * inv_z2))
}

/// Mirrors `atmo_chapman` in pbr_simple.wgsl: closed-form Chapman function
/// (relative slant-path air mass) at radius `x` in scale heights for zenith
/// cosine `mu >= 0`, via the large-x asymptotic
/// `Ch(x, mu) = sqrt(pi*x/2) * erfcx(mu * sqrt(x/2))`. ~1 at the zenith,
/// `sqrt(pi*x/2)` at the horizon; ~0.1% error for planetary `x` (a simpler
/// rational interpolation missed by ~10% at mid angles -- caught by
/// `optical_depth_matches_brute_force_integration`).
pub fn chapman(x: f32, mu: f32) -> f32 {
    (1.570_796_4 * x).sqrt() * erfcx(mu * (0.5 * x).sqrt())
}

/// Mirrors `atmo_od_to_space` in pbr_simple.wgsl: density-integrated path
/// length (units: shell radii at surface density) from radius `r` along
/// zenith cosine `mu` out to space, for an exponential atmosphere over
/// planet radius `rp` with scale height `h`. Rays that dip below the planet
/// surface return a huge depth (sun fully occluded).
pub fn od_to_space(r: f32, mu: f32, rp: f32, h: f32) -> f32 {
    let x = r / h;
    let alt = (r - rp).max(0.0) / h;
    if mu >= 0.0 {
        return h * (-alt).exp() * chapman(x, mu);
    }
    // Downward ray: mirror the path at the tangent point (lowest radius on
    // the ray) -- down-leg = twice the horizontal integral at the tangent
    // minus the upward leg we did not traverse.
    let sin_chi = (1.0 - mu * mu).max(0.0).sqrt();
    let rt = r * sin_chi;
    if rt < rp {
        return 1.0e9;
    }
    let alt_t = (rt - rp) / h;
    let horiz_t = h * (-alt_t).exp() * chapman(rt / h, 0.0);
    (2.0 * horiz_t - h * (-alt).exp() * chapman(x, -mu)).max(0.0)
}

/// Mirrors `atmo_rayleigh_phase`: `3/(16*pi) * (1 + cos^2)`, normalized so
/// the integral over the full sphere of directions is exactly 1.
pub fn rayleigh_phase(cos_theta: f32) -> f32 {
    0.059_683_1 * (1.0 + cos_theta * cos_theta)
}

/// Mirrors `atmo_mie_phase`: Henyey-Greenstein with asymmetry `g`,
/// normalized to integrate to 1 over the sphere.
pub fn hg_phase(cos_theta: f32, g: f32) -> f32 {
    let denom = 1.0 + g * g - 2.0 * g * cos_theta;
    (1.0 - g * g) / (12.566_371 * denom * denom.sqrt())
}

/// Mirrors the `beta_ray` construction in the shader: the documented
/// atmosphere_color -> Rayleigh scattering coefficient mapping. `h_rel` is
/// the scale height in shell radii; the result is per shell radius.
pub fn rayleigh_beta(atmosphere_color: [f32; 4], h_rel: f32) -> [f32; 3] {
    let k = atmosphere_color[3] * TAU_RAYLEIGH / h_rel.max(1.0e-6);
    [
        atmosphere_color[0] * k,
        atmosphere_color[1] * k,
        atmosphere_color[2] * k,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brute-force f64 reference for `od_to_space`: straight numeric march
    /// of the exponential density along the ray until it leaves the
    /// atmosphere (or None when the ray strikes the planet). 2D suffices --
    /// the problem is rotationally symmetric about the local vertical.
    fn brute_force_od(r: f64, mu: f64, rp: f64, h: f64) -> Option<f64> {
        let sin_chi = (1.0 - mu * mu).max(0.0).sqrt();
        // Start at (0, r); direction (sin_chi, mu). Integrate to the "top of
        // the atmosphere" at rp + 40 scale heights (density e^-40 ~ 0).
        let r_top = rp + 40.0 * h;
        let (px, py, dx, dy) = (0.0_f64, r, sin_chi, mu);
        // Path length to the top-sphere exit (largest quadratic root).
        let b = px * dx + py * dy;
        let c = px * px + py * py - r_top * r_top;
        let disc = b * b - c;
        let t_end = if c >= 0.0 {
            // Already above the top (possible for high starting r): only a
            // downward ray re-enters; an upward one has zero depth.
            if disc <= 0.0 || -b - disc.sqrt() < 0.0 {
                return Some(0.0);
            }
            -b + disc.sqrt()
        } else {
            -b + disc.sqrt()
        };
        let n = 400_000;
        let dt = t_end / n as f64;
        let mut od = 0.0;
        for i in 0..n {
            let t = (i as f64 + 0.5) * dt;
            let rr = ((px + dx * t).powi(2) + (py + dy * t).powi(2)).sqrt();
            if rr < rp {
                return None; // occluded by the planet
            }
            od += (-(rr - rp).max(0.0) / h).exp() * dt;
        }
        Some(od)
    }

    /// Earth-like test geometry in shell units (shell = 1.03 planet radii,
    /// exactly what earth.ron produces via shell_packing).
    const RP: f32 = 0.970_873_8; // 1 / 1.03
    const H: f32 = 0.001_295_4; // (8500 / 6371000) * RP

    #[test]
    fn optical_depth_matches_brute_force_integration() {
        // Altitudes in scale heights above the surface, staying inside the
        // shell (its top sits ~22.5 H up for the Earth-like geometry).
        let alts = [0.0_f32, 0.25, 1.0, 3.0, 8.0, 16.0];
        // Zenith cosines from straight up through horizontal to below the
        // horizon (where the tangent-point mirror formula takes over).
        let mus = [1.0_f32, 0.7, 0.4, 0.15, 0.05, 0.0, -0.01, -0.03];
        let mut checked = 0;
        for &alt in &alts {
            let r = RP + alt * H;
            for &mu in &mus {
                let approx = od_to_space(r, mu, RP, H);
                match brute_force_od(r as f64, mu as f64, RP as f64, H as f64) {
                    None => {
                        // Geometric occlusion: the approximation must also
                        // report an effectively infinite depth.
                        assert!(
                            approx > 1.0e6,
                            "occluded ray (alt {alt} H, mu {mu}) not flagged: {approx}"
                        );
                    }
                    Some(reference) => {
                        // The Chapman closed form is a few-percent
                        // approximation; 5% relative keeps us honest while
                        // leaving headroom for f32 vs f64 noise. Depths this
                        // small are visually indistinguishable from zero, so
                        // tiny references only need absolute agreement.
                        let rel = (approx as f64 - reference).abs() / reference.max(1.0e-9);
                        assert!(
                            rel < 0.05 || (approx as f64 - reference).abs() < 1.0e-7,
                            "od mismatch at alt {alt} H, mu {mu}: approx {approx}, reference {reference}, rel {rel:.4}"
                        );
                        checked += 1;
                    }
                }
            }
        }
        // Make sure the grid actually exercised the un-occluded formula.
        assert!(checked >= 30, "too few un-occluded comparisons: {checked}");
    }

    #[test]
    fn occlusion_engages_exactly_when_the_ray_dips_below_the_surface() {
        // From one scale height up, a ray needs to point distinctly below
        // the horizon to strike the planet. Just above that critical angle
        // the depth is finite; just below, effectively infinite.
        let r = RP + 1.0 * H;
        // Critical sin: rt == rp -> sin_chi = rp / r.
        let sin_crit = RP / r;
        let mu_crit = -(1.0 - sin_crit * sin_crit).max(0.0).sqrt();
        assert!(od_to_space(r, mu_crit + 1.0e-4, RP, H) < 1.0e6);
        assert!(od_to_space(r, mu_crit - 1.0e-4, RP, H) > 1.0e6);
    }

    /// Numerically integrate a phase function over the sphere of directions:
    /// `2 * pi * integral p(cos) d cos` must be 1 for a normalized phase.
    fn sphere_integral(p: impl Fn(f32) -> f32) -> f64 {
        let n = 200_000;
        let dc = 2.0 / n as f64;
        let mut sum = 0.0;
        for i in 0..n {
            let c = -1.0 + (i as f64 + 0.5) * dc;
            sum += p(c as f32) as f64 * dc;
        }
        sum * 2.0 * std::f64::consts::PI
    }

    #[test]
    fn rayleigh_phase_integrates_to_one() {
        let total = sphere_integral(rayleigh_phase);
        assert!((total - 1.0).abs() < 1.0e-3, "rayleigh integral {total}");
    }

    #[test]
    fn mie_phase_integrates_to_one() {
        let total = sphere_integral(|c| hg_phase(c, MIE_G));
        assert!((total - 1.0).abs() < 1.0e-3, "mie integral {total}");
        // And it is genuinely forward-lobed: straight ahead beats backward.
        assert!(hg_phase(1.0, MIE_G) > 50.0 * hg_phase(-1.0, MIE_G));
    }

    #[test]
    fn color_mapping_preserves_channel_ordering() {
        // Earth: blue-dominant color -> blue scatters hardest (blue sky).
        let earth = rayleigh_beta([0.17, 0.41, 1.0, 0.5], H);
        assert!(earth[2] > earth[1] && earth[1] > earth[0]);
        // Earth's blue vertical depth should land near the real ~0.28-0.30:
        // tau = beta * H by construction.
        let tau_blue = earth[2] * H;
        assert!((tau_blue - 0.30).abs() < 1.0e-3, "earth tau_blue {tau_blue}");
        // Mars: red-dominant color -> red scatters hardest (butterscotch).
        let mars = rayleigh_beta([0.85, 0.55, 0.35, 0.18], 0.003);
        assert!(mars[0] > mars[1] && mars[1] > mars[2]);
        // Zero density -> zero coefficients (an airless body stays airless
        // even if someone leaves a color in the RON with alpha 0).
        let none = rayleigh_beta([1.0, 1.0, 1.0, 0.0], H);
        assert_eq!(none, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn shell_packing_matches_earth_geometry() {
        let (rp_ratio, h_rel) = shell_packing(0.015, 8_500.0, 6_371_000.0);
        assert!((rp_ratio - RP).abs() < 1.0e-5, "rp_ratio {rp_ratio}");
        assert!((h_rel - H).abs() < 1.0e-6, "h_rel {h_rel}");
        // The 0.005 minimum shell thickness clamp mirrors lib.rs.
        let (thin, _) = shell_packing(0.0, 8_500.0, 6_371_000.0);
        assert!((thin - 1.0 / 1.01).abs() < 1.0e-5);
    }

    #[test]
    fn chapman_hits_its_analytic_anchors() {
        // Zenith air mass approaches 1 (the large-x asymptotic carries an
        // O(1/x) correction, so allow a few parts per thousand); horizontal
        // is sqrt(pi * x / 2) exactly (erfc(0) polynomial sums to 1).
        let x = RP / H;
        assert!((chapman(x, 1.0) - 1.0).abs() < 3.0e-3, "zenith {}", chapman(x, 1.0));
        let horiz = chapman(x, 0.0);
        let expected = (std::f32::consts::PI * x / 2.0).sqrt();
        assert!((horiz - expected).abs() / expected < 1.0e-5);
    }
}
