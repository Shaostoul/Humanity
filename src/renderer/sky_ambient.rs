//! CPU twin of the megashader's INDIRECT-LIGHT terms (v0.1104).
//!
//! Until v0.1104 this engine had no indirect light at all. The only ambient
//! term in the whole renderer was `albedo * vec3(0.005, 0.005, 0.006)` - a
//! silhouette floor - plus one N.L-gated fill light whose direction was a
//! world-fixed constant, so on a rotating true-scale planet it swung below the
//! local horizon over a day and lit whole classes of surface not at all.
//!
//! `sky_ambient` in `assets/shaders/pbr/90-fragment-main.wgsl` replaces that
//! with real sky irradiance, read from the per-frame Hillaire sky-view table
//! that was already bound and already had a tested CPU twin
//! ([`super::atmo_luts::sky_view_radiance`]). This module is the twin of the
//! evaluation on top of that table, and it exists for three jobs the shader
//! cannot do for itself:
//!
//! 1. CALIBRATION. The lighting exposure of the table is DERIVED from the
//!    sun's own calibration here, not guessed in the shader
//!    ([`ambient_lut_scale`] and its test).
//! 2. THE NO-REGRESSION GUARD. With no sky (ship interiors, deep space) the
//!    new expression must be BIT-IDENTICAL to the constant it replaced.
//! 3. LOCKSTEP. The shader pins the world size of a shadow texel as a literal;
//!    the test here fails if either side drifts.
//!
//! Pure math: no wgpu, no GPU state. `renderer` compiles under the relay
//! feature too, so this module must stay that way.

/// The megashader's `AMBIENT_FLOOR`: the pre-v0.1104 ambient constant, kept
/// exactly, as a floor under the sky term.
pub const AMBIENT_FLOOR: [f32; 3] = [0.005, 0.005, 0.006];

/// Cosine-weighted share of the upper hemisphere carried by the zenith tap.
/// Mirrors `SKY_ZENITH_W`.
pub const SKY_ZENITH_W: f32 = 0.65;

/// Mean terrestrial hemispherical albedo used for the ground-bounce lobe.
/// Mirrors `SKY_GROUND_BOUNCE`.
pub const SKY_GROUND_BOUNCE: f32 = 0.22;

/// The lighting exposure applied to the sky-view table, as a MULTIPLE of the
/// drawn-sky exposure. Mirrors `SKY_AMBIENT_LUT_SCALE`.
pub const SKY_AMBIENT_LUT_SCALE: f32 = 0.24;

/// The drawn-sky exposure (`SKY_LUT_EXPOSURE` / `WATER_SKY_LUT_EXPOSURE`).
pub const SKY_LUT_EXPOSURE: f32 = 15.0;

/// Sun intensity the celestial pass stamps, at full day.
pub const SUN_INTENSITY: f32 = 2.5;

/// Fraction of direct-normal irradiance that arrives as diffuse sky light on
/// a clear day at high sun. Real clear-sky values run ~150 W/m^2 diffuse
/// against ~850 W/m^2 direct; 0.15 is the middle of that band and is what the
/// lighting exposure is calibrated to reproduce.
pub const CLEAR_SKY_DIFFUSE_FRACTION: f32 = 0.15;

/// The reflected radiance of a white Lambert surface facing the sun, in render
/// units: `evaluate_light` divides diffuse by PI, so it is `intensity / PI`.
/// This is the yardstick every other lighting magnitude in the engine is
/// measured against.
pub fn sunlit_white_reference() -> f32 {
    SUN_INTENSITY / std::f32::consts::PI
}

/// The lighting exposure the sky-view table should be read at, derived from
/// the sun rather than picked: pick the scale that makes an up-facing white
/// surface return [`CLEAR_SKY_DIFFUSE_FRACTION`] of its sunlit value under a
/// noon sky whose measured 2-tap hemisphere average is `noon_two_tap_raw`
/// (raw table units, before any exposure).
pub fn ambient_lut_scale(noon_two_tap_raw: f32) -> f32 {
    let target = CLEAR_SKY_DIFFUSE_FRACTION * sunlit_white_reference();
    (target / noon_two_tap_raw.max(1e-9)) / SKY_LUT_EXPOSURE
}

/// The two-lobe hemispheric evaluation, mirroring `sky_ambient`'s tail:
/// `zenith` and `horizon` are the two table taps ALREADY scaled to the
/// lighting exposure, `n_dot_up` is the fragment normal against local up.
pub fn hemispheric(zenith: [f32; 3], horizon: [f32; 3], n_dot_up: f32) -> [f32; 3] {
    let w = 0.5 + 0.5 * n_dot_up.clamp(-1.0, 1.0);
    let mut out = [0.0f32; 3];
    for c in 0..3 {
        let sky = zenith[c] * SKY_ZENITH_W + horizon[c] * (1.0 - SKY_ZENITH_W);
        let ground = sky * SKY_GROUND_BOUNCE;
        out[c] = ground + (sky - ground) * w;
    }
    out
}

/// The whole ambient expression at the bottom of `fs_main`:
/// `albedo * max(sky_ambient(...), AMBIENT_FLOOR) * ao`.
pub fn ambient_term(albedo: [f32; 3], sky: [f32; 3], ao: f32) -> [f32; 3] {
    let mut out = [0.0f32; 3];
    for c in 0..3 {
        out[c] = albedo[c] * sky[c].max(AMBIENT_FLOOR[c]) * ao;
    }
    out
}

/// World size of one sun-shadow-map texel, in metres. The megashader pins this
/// as the literal `SHADOW_TEXEL_M`; the lockstep test below compares them.
pub fn shadow_texel_m() -> f32 {
    2.0 * super::SUN_SHADOW_EXTENT_M / super::SUN_SHADOW_MAP_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::atmo_luts::{
        multiple_scattering_lut, sky_view_radiance, TransLutParams,
    };
    use crate::renderer::atmosphere::shell_packing;

    fn earthish() -> TransLutParams {
        let (rp, h) = shell_packing(0.06, 8500.0, 6.371e6);
        TransLutParams { tint: [0.18, 0.42, 1.0], density_mul: 1.0, rp, h }
    }

    /// Two table taps at the same parameterization the shader uses: the zenith
    /// (elevation +90) and the horizon band 90 degrees of azimuth from the sun
    /// (`cos_az = 0`, the azimuthal mean of the band).
    fn taps(mu_s: f32) -> ([f32; 3], [f32; 3]) {
        let p = earthish();
        let ms = multiple_scattering_lut(&p);
        let ground = p.rp;
        let zen = sky_view_radiance(&p, ground, 1.0, 1.0, mu_s, &ms);
        let hor = sky_view_radiance(&p, ground, 0.02, 0.0, mu_s, &ms);
        (zen, hor)
    }

    fn lit_taps(mu_s: f32) -> ([f32; 3], [f32; 3]) {
        let (z, h) = taps(mu_s);
        let s = SKY_LUT_EXPOSURE * SKY_AMBIENT_LUT_SCALE;
        ([z[0] * s, z[1] * s, z[2] * s], [h[0] * s, h[1] * s, h[2] * s])
    }

    /// THE NO-REGRESSION GUARD (ship interiors, deep space). `sky_ambient`
    /// returns exactly zero when there is no sky-view table this frame or no
    /// local up, and the floored expression must then be BIT-IDENTICAL to the
    /// `albedo * vec3(0.005, 0.005, 0.006)` line it replaced. Those passes
    /// zero the uniform pads that carry sky colour and up (which is also why
    /// rooms never fog), so this is the common case indoors, not a corner.
    #[test]
    fn zero_sky_is_bit_exactly_the_old_ambient_floor() {
        for albedo in [
            [1.0f32, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.5, 0.25, 0.125],
            [0.831_23, 0.017_9, 0.999_9],
            [0.003_1, 0.777, 0.041_5],
        ] {
            let got = ambient_term(albedo, [0.0, 0.0, 0.0], 1.0);
            let want = [
                albedo[0] * 0.005,
                albedo[1] * 0.005,
                albedo[2] * 0.006,
            ];
            for c in 0..3 {
                assert_eq!(
                    got[c].to_bits(),
                    want[c].to_bits(),
                    "channel {c} of albedo {albedo:?}: {} != {}",
                    got[c],
                    want[c]
                );
            }
        }
    }

    /// The floor is a FLOOR, not an addition: a bright sky must not stack the
    /// old constant on top of itself (which would be a systematic lift of
    /// every surface in the world).
    #[test]
    fn a_real_sky_replaces_the_floor_rather_than_adding_to_it() {
        let (z, h) = lit_taps(0.95);
        let sky = hemispheric(z, h, 1.0);
        let got = ambient_term([1.0, 1.0, 1.0], sky, 1.0);
        for c in 0..3 {
            assert!(sky[c] > AMBIENT_FLOOR[c], "noon sky below the floor in channel {c}");
            assert_eq!(got[c].to_bits(), sky[c].to_bits());
        }
    }

    /// CALIBRATION. The shader's `SKY_AMBIENT_LUT_SCALE` must be the value the
    /// sun's own calibration implies, not a number someone liked: an up-facing
    /// white surface under a noon sky returns ~15% of its sunlit value. This
    /// is the test that would fail if a future tune of the DRAWN sky exposure
    /// silently changed how bright the world's shade is.
    #[test]
    fn the_lighting_exposure_is_derived_from_the_sun_not_guessed() {
        let (z, h) = taps(0.95);
        let raw = hemispheric(z, h, 1.0);
        let derived = ambient_lut_scale(raw[1]); // green, the luminance proxy
        assert!(
            (derived - SKY_AMBIENT_LUT_SCALE).abs() < 0.03,
            "shader constant {SKY_AMBIENT_LUT_SCALE} drifted from the derived {derived}"
        );
        // And the consequence, stated as the thing we actually care about.
        let (lz, lh) = lit_taps(0.95);
        let ambient = hemispheric(lz, lh, 1.0);
        let ratio = ambient[1] / sunlit_white_reference();
        assert!(
            (0.12..=0.19).contains(&ratio),
            "noon up-facing ambient is {ratio} of sunlit, want the clear-sky 0.12-0.19"
        );
    }

    /// The whole point of S2: with shadow strength 1.0, a fully shadowed
    /// up-facing surface at noon keeps 10-20% of its sunlit value AND is
    /// BLUE. At the old 0.6 strength it kept 41-43% in the sun's own warm
    /// colour, which is what made shade read as washed-out haze.
    #[test]
    fn a_noon_shadow_lands_in_the_physical_band_and_is_blue() {
        let (z, h) = lit_taps(0.95);
        let ambient = hemispheric(z, h, 1.0);
        // Full sun on a white up-facing surface, plus its own ambient.
        let sunlit = sunlit_white_reference() + ambient[1];
        let ratio = ambient[1] / sunlit;
        assert!(
            (0.10..=0.20).contains(&ratio),
            "shadow/sunlit is {ratio}, want the clear-sky 0.10-0.20"
        );
        assert!(
            ambient[2] > ambient[0] * 1.2,
            "noon shade must be BLUE (R={} B={})",
            ambient[0],
            ambient[2]
        );
    }

    /// Directionality: the term is real hemispheric lighting, not a constant
    /// wearing a function's clothes. A down-facing surface sees only the
    /// ground bounce and must land at exactly the bounce albedo.
    #[test]
    fn down_facing_surfaces_get_only_the_ground_bounce() {
        let (z, h) = lit_taps(0.95);
        let up = hemispheric(z, h, 1.0);
        let side = hemispheric(z, h, 0.0);
        let down = hemispheric(z, h, -1.0);
        for c in 0..3 {
            assert!(up[c] > side[c] && side[c] > down[c], "channel {c} not monotone in n.up");
            assert!(
                (down[c] / up[c] - SKY_GROUND_BOUNCE).abs() < 1e-5,
                "down/up should be the bounce albedo, got {}",
                down[c] / up[c]
            );
            // A vertical wall sees half sky, half bounce.
            let want = up[c] * 0.5 * (1.0 + SKY_GROUND_BOUNCE);
            assert!((side[c] - want).abs() < 1e-5);
        }
    }

    /// The sky term must carry the DAY, not a fixed tint: warm and dim near
    /// sunset, blue and bright at noon, and effectively gone once the sun is
    /// below the horizon (which is what stops v0.1104 from resurrecting the
    /// daytime-bright-fog-at-midnight class of bug in the ambient slot).
    #[test]
    fn the_sky_term_tracks_the_sun_through_the_day() {
        let noon = { let (z, h) = lit_taps(0.95); hemispheric(z, h, 1.0) };
        let low = { let (z, h) = lit_taps(0.15); hemispheric(z, h, 1.0) };
        let set = { let (z, h) = lit_taps(0.02); hemispheric(z, h, 1.0) };
        let night = { let (z, h) = lit_taps(-0.08); hemispheric(z, h, 1.0) };
        assert!(noon[2] > noon[0], "noon must be blue");
        assert!(low[0] > low[2], "a low sun must warm the shade");
        assert!(set[1] < noon[1] * 0.25, "sunset shade must be far dimmer than noon");
        assert!(
            night[1] < AMBIENT_FLOOR[1],
            "below the horizon the sky term must fall under the floor, got {}",
            night[1]
        );
    }

    /// AO belongs to the indirect term only. Two identical surfaces, one in a
    /// crevice, must differ ONLY in their sky share - never in how much sun
    /// they take.
    #[test]
    fn ao_scales_the_indirect_term_and_nothing_else() {
        let (z, h) = lit_taps(0.95);
        let sky = hemispheric(z, h, 1.0);
        let open = ambient_term([0.5, 0.5, 0.5], sky, 1.0);
        let cavity = ambient_term([0.5, 0.5, 0.5], sky, 0.4);
        for c in 0..3 {
            assert!((cavity[c] - open[c] * 0.4).abs() < 1e-7);
        }
    }

    /// LOCKSTEP: the megashader pins the world size of a shadow texel as a
    /// literal because ShadowUniforms has no free slot and adding one is a
    /// bind-group-layout change (the v0.1029-v0.1038 incident). Fail loudly if
    /// the map size or the ortho extent moves without the shader following.
    #[test]
    fn shadow_texel_constant_matches_the_shader() {
        let src = crate::renderer::shader_loader::assembled_pbr_source();
        let needle = "const SHADOW_TEXEL_M: f32 = ";
        let i = src.find(needle).expect("SHADOW_TEXEL_M missing from the megashader");
        let rest = &src[i + needle.len()..];
        let lit: String = rest.chars().take_while(|c| *c != ';').collect();
        let in_shader: f32 = lit.trim().parse().expect("SHADOW_TEXEL_M is not a float literal");
        assert_eq!(
            in_shader.to_bits(),
            shadow_texel_m().to_bits(),
            "shader says {in_shader}, renderer says {}",
            shadow_texel_m()
        );
    }

    /// The shadow strength is no longer a hardcoded constant in the render
    /// loop: if someone re-hardcodes it, the Settings slider silently stops
    /// working, which is exactly the failure the settings-persistence lint
    /// exists to catch on the other end of the wire.
    #[test]
    fn shadow_strength_comes_from_the_renderer_field() {
        let src = include_str!("mod.rs");
        assert!(
            src.contains("su[17] = self.shadow_strength"),
            "su[17] must read the renderer field, not a literal"
        );
        assert!(
            !src.contains("su[17] = 0.6"),
            "the v0.899 hardcoded 0.6 shadow strength is back"
        );
    }
}
