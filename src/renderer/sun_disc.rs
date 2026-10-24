//! THE SUN AS A BODY (2026-10-21): its disc at its true size, its photosphere,
//! its brightness, and the glare a camera sees round it.
//!
//! The operator, from a screenshot over Earth's limb: "the sun doesn't actually
//! seem to have a body. It seems like it is just a giant glare/bloom spot." It
//! had none. The disc was floored at `dist * 0.045`, an angular radius of 2.6
//! degrees, so it was drawn ten times its real width, as a smooth radial glow
//! (shader type 17) with a second glow three times wider round it, and both
//! tone-mapped to white whatever the light. This module is what replaced them.
//!
//! WHAT IS MODELLED, AND FROM WHERE (sources read 2026-10-21):
//!
//! * SIZE. The disc's angular radius is `asin(R / d)` from the Sun's real radius
//!   (IAU 2015 Resolution B3 nominal solar radius, 6.957e8 m) and its real
//!   distance this frame: 0.2666 degrees at 1 AU, 0.533 degrees across. Below
//!   [`DISC_FLOOR_PX`] pixels (the Sun from the outer planets) the drawn disc
//!   stops shrinking and dims instead, so a pixel still receives the light a
//!   camera pixel would ([`disc_angular_radius`], [`disc_energy_scale`]).
//!
//! * LIMB DARKENING. Hestroffer & Magnan (1998), "Wavelength dependency of the
//!   Solar limb darkening", A&A 333, 338-342: `I(mu) / I(1) = mu^alpha` with
//!   `alpha = -0.023 + 0.292 / lambda` (lambda in micrometres, their eq. 5,
//!   fitted to Pierce & Slaughter 1977 and Neckel & Labs 1994, 416-1099 nm,
//!   "accurate to a few +-0.02"). Weighted over each sRGB channel by a 5772 K
//!   spectrum and the CIE 1931 observer this is [`LIMB_ALPHA`] (the derivation
//!   is re-run by `the_limb_darkens_as_the_photosphere_does`); the disc is
//!   bluer at the centre and redder at the edge, as the real one is. HM98
//!   fitted the law inside 0.9 of the radius; beyond it the law runs darker
//!   than the measured limb (0 at the edge where Neckel & Labs have 0.28),
//!   which on a disc 3 to 6 pixels in radius is under half a pixel.
//!
//! * COLOUR. The Sun's chromaticity outside the air, x 0.3233 y 0.3326, from
//!   the Kurucz et al. (1984) solar flux atlas as computed by A. Hamilton,
//!   "What colour is the Sun?" (JILA, updated 2001, note of 2017): linear sRGB
//!   [`SUN_FLUX_RGB`] at luminance 1, a faint peach against the D65 white.
//!   Its effective temperature is 5772 K (IAU 2015 B3).
//!
//! * BRIGHTNESS. No hand-picked intensity. The engine lights a surface facing
//!   the Sun at 1 AU with irradiance `CELESTIAL_SUN_I` (2.5 scene units, with
//!   which a white Lambertian surface comes out at 0.8), so the disc's mean
//!   radiance is that irradiance over the disc's solid angle: about 36,800,
//!   46,000 times a sunlit white wall, which is the real ratio. Radiance does
//!   not change with distance; the disc's solid angle does, so the light it
//!   gives falls as 1/d^2 by itself. Every shader tone-maps it with the same
//!   ACES curve as everything else, so EXPOSURE decides how it looks: white at
//!   noon, yellow and then orange as the air in front of it thickens at sunset,
//!   limb-darkened and red where it is dim enough to be seen.
//!
//! * GLARE. A camera does not see a 0.53 degree disc: its aperture spreads
//!   part of the light into a halo, and against a clipping tone curve that
//!   halo is what makes a bright source read as one. Here it is the diffraction
//!   pattern of a circular aperture (the Airy pattern, Fraunhofer optics),
//!   ring-averaged, `lambda / (pi^3 D theta^3)` per steradian per unit
//!   irradiance, convolved with the limb-darkened disc ([`disc_glare`]), per
//!   colour channel at the channel's dominant wavelength. It conserves energy:
//!   the glare's light is light the disc does not keep ([`spill_fraction`]).
//!   The aperture is a setting (Settings > Graphics > Light and sky, 30 mm,
//!   [`GLARE_APERTURE_M`], by default): a narrower one spreads the glare
//!   wider (the scale is lambda / D). The human eye's own glare (Spencer,
//!   Shirley, Zimmerman & Greenberg 1995, "Physically-Based Glare Effects for
//!   Digital Images", the PSF of Vos 1984) was considered and not used: at
//!   this engine's exposure its scattered wings clip a region about 4.4
//!   degrees wide to white, which is the giant glare spot the operator
//!   rejected; a real eye does not clip, a display does.
//!
//! * CORONA. Baumbach (1937), the mean K+F corona from ten eclipses, as given
//!   by Reginald (NASA GSFC, dissertation ch. 5, eq. 5.5): `I(r) = 0.0532
//!   r^-2.5 + 1.425 r^-7 + 2.565 r^-17` in millionths of the disc-centre
//!   brightness, `r` in solar radii. Drawn whenever the Sun is, it shows only
//!   where it really would: round a disc the Moon or a planet has covered, in
//!   a dark sky. At this engine's fixed daylight exposure it is a faint ring,
//!   as it is to a camera exposed for daylight (an eye adapts; the engine has
//!   no adaptation yet). Prominences are not drawn: they are line emission
//!   tied to the solar cycle, there is no published mean model to draw them
//!   from, and they would show only in a total eclipse.
//!
//! * GRANULATION is not drawn. A granule is about 1 arcsecond, 1/1900 of the
//!   disc's width: about 0.005 of a pixel on a disc 10 pixels across, so it
//!   cannot be seen at any field of view the game has, and an exposure that
//!   showed it would show the rest of the scene black.
//!
//! WHERE IT IS DRAWN. The disc (and corona) is a sphere in the celestial
//! transparent list, type 17 in `90-fragment-main.wgsl`, behind everything;
//! where a scattering atmosphere is in front of it the shell draws the disc
//! itself (`30-atmosphere.wgsl`), with the per-pixel transmittance of its own
//! air, so a setting Sun reddens and flattens row by row and its light is
//! added to the air's own light BEFORE the tone curve. The glare is added in
//! the present pass, over everything (it happens in the camera), scaled by how
//! much of the disc is visible (`sun_glare.rs`, depth taps over the disc).

use std::f64::consts::PI;

/// The Sun's radius, metres (IAU 2015 Resolution B3, nominal solar radius).
pub const SUN_RADIUS_M: f64 = 6.957e8;

/// One astronomical unit, metres (IAU 2012 Resolution B2).
pub const AU_M: f64 = 1.495_978_707e11;

/// Limb-darkening exponents per linear sRGB channel, `I(mu)/I(1) = mu^alpha`
/// (Hestroffer & Magnan 1998 eq. 5 weighted over each channel by a 5772 K
/// spectrum and the CIE 1931 2-degree observer; recomputed by the tests).
/// WGSL twin: `SUN_LIMB_ALPHA` in 80-fragment-shared.wgsl.
pub const LIMB_ALPHA: [f32; 3] = [0.4346, 0.5132, 0.6417];

/// The Sun's colour outside the air, linear sRGB at luminance 1 (Hamilton's
/// x 0.3233 y 0.3326 from the Kurucz et al. 1984 flux atlas). This is the
/// disc-integrated (flux) colour; the centre is bluer, see [`centre_radiance`].
pub const SUN_FLUX_RGB: [f32; 3] = [1.0969, 0.9769, 0.9437];

/// The dominant wavelengths of the sRGB primaries, metres: the wavelength the
/// glare of each channel is diffracted at.
pub const CHANNEL_WAVELENGTH_M: [f64; 3] = [611.4e-9, 549.1e-9, 464.3e-9];

/// The camera's entrance pupil, metres: the aperture whose diffraction is the
/// glare. 30 mm is a 75 mm lens at f/2.5 (or a 50 mm at f/1.7). Narrower
/// spreads the glare wider: the angular scale is lambda / D. The default of
/// Settings > Graphics > Light and sky's "Sun glare: camera opening (mm)"
/// ([`aperture_from_setting_mm`]); dev pin: the showcase key `sun_aperture_mm`.
pub const GLARE_APERTURE_M: f64 = 0.030;

/// The narrowest and the widest aperture the glare takes, millimetres
/// (Settings, and the showcase key `sun_aperture_mm`): 0.5 mm is a phone's
/// ultra-wide camera (a 1.5 mm lens at f/2.4 is 0.6 mm), 1000 mm a big
/// telescope's mirror.
pub const APERTURE_MIN_MM: f32 = 0.5;
pub const APERTURE_MAX_MM: f32 = 1000.0;

/// [`GLARE_APERTURE_M`] in millimetres: the setting's default.
pub const DEFAULT_APERTURE_MM: f32 = 30.0;

/// The glare's aperture, metres, from the Settings one in millimetres
/// (Settings > Graphics > Light and sky, `SettingsState::sun_aperture_mm`;
/// and the showcase key `sun_aperture_mm`): held between
/// [`APERTURE_MIN_MM`] and [`APERTURE_MAX_MM`], the default
/// ([`GLARE_APERTURE_M`]) where it is not a number.
pub fn aperture_from_setting_mm(mm: f32) -> f64 {
    if !mm.is_finite() {
        return GLARE_APERTURE_M;
    }
    mm.clamp(APERTURE_MIN_MM, APERTURE_MAX_MM) as f64 / 1000.0
}

/// Below this many pixels of radius the drawn disc stops shrinking and dims
/// instead (energy kept), so a far Sun stays a point of the right brightness.
pub const DISC_FLOOR_PX: f64 = 0.7;

/// The sphere the disc and corona are drawn on, in solar radii. The Baumbach
/// corona at 3 radii is 4e-9 of the disc centre: nothing at any exposure that
/// shows the disc.
pub const MESH_SOLAR_RADII: f64 = 3.0;

/// Closed-form fit of the limb-darkened disc convolved with the glare's
/// far field, per channel: `h(s) = (1 + b s^2 + c s^4) / (1 - s^2)^p`,
/// `s = r / theta` (see [`disc_glare`]). `p = 1 - alpha/2` is the power of the
/// spill at the disc's edge for a `mu^alpha` limb, `b` the far-field multipole
/// term, `c` least squares against the brute-force convolution (within 7% for
/// theta >= 1.02 r; checked by the tests). WGSL twin: `SUN_GLARE_B/C/P` in
/// present.wgsl.
pub const GLARE_FIT_B: [f64; 3] = [0.2320, 0.2537, 0.2903];
pub const GLARE_FIT_C: [f64; 3] = [0.2582, 0.2992, 0.3762];
pub const GLARE_FIT_P: [f64; 3] = [0.7827, 0.7434, 0.6791];

/// The largest `s = r / theta` the glare is evaluated at: inside it is the
/// disc itself, drawn by the scene.
pub const GLARE_S_MAX: f64 = 0.98;

/// The Sun's angular radius, radians, from `dist_m` metres.
pub fn angular_radius(dist_m: f64) -> f64 {
    (SUN_RADIUS_M / dist_m.max(SUN_RADIUS_M * 1.0001)).min(1.0).asin()
}

/// The angular radius the disc is DRAWN at, radians: the true one, floored at
/// [`DISC_FLOOR_PX`] pixels of `px_angle` radians each.
///
/// Seen red 2026-10-21 with the engine's old floor (`dist * 0.045`, the
/// v0.887 sun): "the Sun at 1 AU is drawn 5.158 deg across, not 0.533".
pub fn disc_angular_radius(dist_m: f64, px_angle: f64) -> f64 {
    angular_radius(dist_m).max(DISC_FLOOR_PX * px_angle.max(0.0))
}

/// The angular radius of the sphere the disc and corona are drawn on: the
/// corona's three solar radii, or the floored disc and two pixels of edge.
pub fn mesh_angular_radius(dist_m: f64, px_angle: f64) -> f64 {
    let r = angular_radius(dist_m);
    let drawn = disc_angular_radius(dist_m, px_angle);
    (MESH_SOLAR_RADII * r).max(drawn + 2.0 * px_angle.max(0.0)).min(1.2)
}

/// The two camera-uniform pads the shaders read the disc from: light6_spot =
/// [centre radiance rgb, true angular radius], light7_spot = [unit direction
/// from the camera to the Sun's centre, 0] (80-fragment-shared.wgsl).
pub fn camera_pads(dist_m: f64, dir: glam::DVec3) -> [[f32; 4]; 2] {
    let l0 = centre_radiance();
    let d = dir.normalize_or_zero();
    [[l0[0], l0[1], l0[2], angular_radius(dist_m) as f32], [d.x as f32, d.y as f32, d.z as f32, 0.0]]
}

/// The factor a disc drawn at `drawn` radians instead of its true `true_r`
/// dims by, so the light it gives stays the same: `(true / drawn)^2`.
pub fn disc_energy_scale(true_r: f64, drawn: f64) -> f64 {
    if drawn <= 0.0 {
        return 0.0;
    }
    (true_r / drawn).min(1.0).powi(2)
}

/// `I(mu) / I(1)` per channel.
pub fn limb_darkening(mu: f32) -> [f32; 3] {
    let m = mu.clamp(0.0, 1.0);
    LIMB_ALPHA.map(|a| m.powf(a))
}

/// The disc's mean radiance over its centre radiance, for `mu^alpha`:
/// `2 / (alpha + 2)`.
pub fn disc_mean_over_centre(alpha: f32) -> f32 {
    2.0 / (alpha + 2.0)
}

/// The Sun's irradiance at 1 AU in scene units, per channel, before any air:
/// the lighting's `CELESTIAL_SUN_I` in the Sun's own colour.
pub fn irradiance_1au() -> [f32; 3] {
    let e = super::key_lights::CELESTIAL_SUN_I;
    [e * SUN_FLUX_RGB[0], e * SUN_FLUX_RGB[1], e * SUN_FLUX_RGB[2]]
}

/// The disc's CENTRE radiance, scene units, per channel, before any air. The
/// same at every distance (radiance is conserved); with the glare's share
/// taken out ([`spill_fraction`]) so disc and glare together give exactly the
/// Sun's irradiance.
pub fn centre_radiance() -> [f32; 3] {
    let omega_1au = PI * angular_radius(AU_M).powi(2);
    let e = irradiance_1au();
    let mut out = [0.0f32; 3];
    for c in 0..3 {
        let mean = e[c] as f64 / omega_1au;
        let keep = 1.0 - spill_fraction(angular_radius(AU_M), c, GLARE_APERTURE_M);
        out[c] = (mean / disc_mean_over_centre(LIMB_ALPHA[c]) as f64 * keep) as f32;
    }
    out
}

/// The irradiance the whole disc gives at `dist_m`, per channel, before any
/// air: the 1 AU value times `(1 AU / d)^2` (the disc's solid angle).
pub fn irradiance_at(dist_m: f64) -> [f32; 3] {
    let k = (angular_radius(dist_m) / angular_radius(AU_M)).powi(2) as f32;
    let e = irradiance_1au();
    [e[0] * k, e[1] * k, e[2] * k]
}

/// The glare's far-field constant for channel `c`, per unit irradiance:
/// `lambda / (pi^3 D)` (the ring-averaged Airy pattern is that over
/// theta^3, per steradian).
pub fn glare_coeff(c: usize, aperture_m: f64) -> f64 {
    CHANNEL_WAVELENGTH_M[c] / (PI.powi(3) * aperture_m.max(1.0e-4))
}

/// The closed-form disc-convolution shape, see [`GLARE_FIT_B`].
pub fn glare_h(s: f64, c: usize) -> f64 {
    let s = s.clamp(0.0, GLARE_S_MAX);
    let s2 = s * s;
    (1.0 + GLARE_FIT_B[c] * s2 + GLARE_FIT_C[c] * s2 * s2) / (1.0 - s2).powf(GLARE_FIT_P[c])
}

/// The glare's radiance (scene units) at `theta` radians from the Sun's
/// centre, channel `c`, for a disc of angular radius `r` giving irradiance
/// `e` in that channel through an aperture of `aperture_m`.
pub fn disc_glare(theta: f64, r: f64, e: f64, c: usize, aperture_m: f64) -> f64 {
    let th = theta.max(r / GLARE_S_MAX);
    e * glare_coeff(c, aperture_m) * glare_h(r / th, c) / th.powi(3)
}

/// The fraction of the disc's light the glare carries beyond `r / GLARE_S_MAX`
/// (the light the disc does not keep), channel `c`.
pub fn spill_fraction(r: f64, c: usize, aperture_m: f64) -> f64 {
    // (2 pi C / r) * integral of h(s) ds over s in (0, S_MAX): theta = r / s.
    let n = 400;
    let mut acc = 0.0;
    for i in 0..n {
        let s = (i as f64 + 0.5) / n as f64 * GLARE_S_MAX;
        acc += glare_h(s, c) * GLARE_S_MAX / n as f64;
    }
    2.0 * PI * glare_coeff(c, aperture_m) / r.max(1.0e-9) * acc
}

/// Baumbach's (1937) mean corona, millionths of the disc-centre brightness,
/// `rho` in solar radii from the centre (0 inside the disc).
pub fn baumbach_corona(rho: f64) -> f64 {
    if rho < 1.0 {
        return 0.0;
    }
    0.0532 * rho.powf(-2.5) + 1.425 * rho.powf(-7.0) + 2.565 * rho.powf(-17.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Sun at 1 AU is drawn at its real size, 0.533 degrees across (from
    /// R = 6.957e8 m), at any field of view a player has: the floor only
    /// starts below 0.7 pixels of radius.
    ///
    /// Seen red 2026-10-21 against the engine's old floor (`dist * 0.045`):
    /// "the Sun at 1 AU is drawn 5.158 deg across, not 0.533".
    #[test]
    fn the_sun_is_drawn_0_53_degrees_across_at_1_au() {
        // 60 and 100 degrees of vertical field over 1080 and 2160 rows.
        for (fov_deg, rows) in [(60.0_f64, 1080.0_f64), (100.0, 2160.0), (30.0, 720.0)] {
            let px = fov_deg.to_radians() / rows;
            let deg = 2.0 * disc_angular_radius(AU_M, px).to_degrees();
            assert!((deg - 0.5329).abs() < 0.002, "the Sun at 1 AU is drawn {deg:.3} deg across, not 0.533");
        }
        // Perihelion and aphelion: 0.542 and 0.524 degrees.
        let near = 2.0 * disc_angular_radius(0.98329 * AU_M, 1.0e-4).to_degrees();
        let far = 2.0 * disc_angular_radius(1.01671 * AU_M, 1.0e-4).to_degrees();
        assert!((near - 0.5420).abs() < 0.002 && (far - 0.5241).abs() < 0.002, "perihelion {near:.4}, aphelion {far:.4}");
    }

    /// The Sun is DRAWN, at its true size and with its own light, from every
    /// distance a camera reaches: the ground, the ship's orbit (35,870 km up),
    /// and 100,000 km and 2,000,000 km out looking back at a crescent Earth
    /// with the Sun 30 degrees to the side (the operator's second screenshot,
    /// 2026-10-21: a big soft glow and no disc). Drawn means: the disc at the
    /// true angular radius, light in the camera pads, a sphere that covers the
    /// disc with room for its antialiased rim, and that sphere inside the
    /// celestial pass's reverse-Z depth range (nearer than its far plane, at a
    /// depth above the cleared 0). The disc's light comes only from these pads
    /// and the view's own rays; nothing the player's state computes scales it
    /// (lib.rs: the v0.1541 disc was multiplied by the player's horizon and
    /// cloud estimate, so a view whose camera was not where those were
    /// computed could lose it while the god rays still drew their glow).
    ///
    /// Seen red 2026-10-21 against the engine's old floor (`dist * 0.045`):
    /// "the Sun from the ground is drawn 5.158 deg across, not 0.533".
    #[test]
    fn the_sun_is_drawn_with_its_light_from_every_distance() {
        let r_earth = 6_371_000.0_f64;
        let px = 90.0_f64.to_radians() / 1387.0; // the rig's 90 degree view, 1387 rows
        let sun = glam::DVec3::new(AU_M, 0.0, 0.0);
        // The camera over the night side, 150 degrees round from the sub-solar
        // point: from far out the Earth is a crescent 30 degrees from the Sun.
        let night = glam::DVec3::new(150.0_f64.to_radians().cos(), 150.0_f64.to_radians().sin(), 0.0);
        for (what, alt) in [("the ground", 30.0), ("the ship", 35_870e3), ("100,000 km out", 100_000e3), ("2,000,000 km out", 2_000_000e3)] {
            let cam = night * (r_earth + alt);
            let to_sun = sun - cam;
            let dist = to_sun.length();
            let r = disc_angular_radius(dist, px);
            let want = (SUN_RADIUS_M / dist).asin();
            assert!(
                (r - want).abs() < 1.0e-7,
                "the Sun from {what} is drawn {:.3} deg across, not {:.3}",
                2.0 * r.to_degrees(),
                2.0 * want.to_degrees()
            );
            let pads = camera_pads(dist, to_sun);
            assert!(pads[0][3] > 0.0 && pads[0].iter().take(3).all(|&l| l > 1.0e4), "no disc light from {what}: {pads:?}");
            let mesh = mesh_angular_radius(dist, px);
            assert!(mesh >= r + 1.5 * px, "the sphere from {what} ({mesh} rad) does not cover the disc ({r} rad)");
            let front = dist - dist * mesh.tan();
            let depth = super::super::camera::celestial_depth_at(front as f32);
            assert!(depth > 0.0 && dist + dist * mesh.tan() < super::super::camera::CELESTIAL_FAR_M as f64, "the Sun from {what} is outside the celestial depth range (depth {depth})");
        }
    }

    /// A Sun too small to cover a pixel is drawn at the floor and dimmed so
    /// it gives the same light: drawn area times the energy scale is the true
    /// area.
    ///
    /// Seen red 2026-10-21 against the old floor, which drew the Sun seen from
    /// Pluto 380 times its width at full brightness: "drawn
    /// 0.045015201356314066 px 0.0009696273622190719 true
    /// 0.00011773334865077522".
    #[test]
    fn a_sub_pixel_sun_keeps_its_light() {
        let px = 60.0_f64.to_radians() / 1080.0;
        let d = 39.5 * AU_M; // Pluto
        let r = angular_radius(d);
        let drawn = disc_angular_radius(d, px);
        assert!(drawn > r && (drawn / px - DISC_FLOOR_PX).abs() < 1e-9, "drawn {drawn} px {px} true {r}");
        let kept = drawn * drawn * disc_energy_scale(r, drawn);
        assert!((kept / (r * r) - 1.0).abs() < 1e-9);
    }

    // ── The CIE 1931 2-degree observer (Wyman, Sloan & Shirley 2013, "Simple
    // Analytic Approximations to the CIE XYZ Color Matching Functions", JCGT
    // 2(2), the multi-lobe fit) and Planck's law, to re-derive LIMB_ALPHA.
    fn g(x: f64, mu: f64, s1: f64, s2: f64) -> f64 {
        let s = if x < mu { s1 } else { s2 };
        let t = (x - mu) / s;
        (-0.5 * t * t).exp()
    }
    fn cmf(l: f64) -> [f64; 3] {
        [
            1.056 * g(l, 599.8, 37.9, 31.0) + 0.362 * g(l, 442.0, 16.0, 26.7) - 0.065 * g(l, 501.1, 20.4, 26.2),
            0.821 * g(l, 568.8, 46.9, 40.5) + 0.286 * g(l, 530.9, 16.3, 31.1),
            1.217 * g(l, 437.0, 11.8, 36.0) + 0.681 * g(l, 459.0, 26.0, 13.8),
        ]
    }
    fn planck(l_nm: f64, t: f64) -> f64 {
        let l = l_nm * 1e-9;
        let (h, c, k) = (6.626_070_15e-34, 2.997_924_58e8, 1.380_649e-23);
        1.0 / (l.powi(5) * ((h * c / (l * k * t)).exp() - 1.0))
    }
    /// Hestroffer & Magnan (1998) eq. 5, 1/lambda <~ 2.4 per micrometre.
    fn hm98_alpha(l_nm: f64) -> f64 {
        -0.023 + 0.292 / (l_nm.max(400.0) / 1000.0)
    }
    fn srgb_at(mu: f64) -> [f64; 3] {
        let m = [[3.2406, -1.5372, -0.4986], [-0.9689, 1.8758, 0.0415], [0.0557, -0.2040, 1.0570]];
        let mut xyz = [0.0; 3];
        let mut l = 380.0;
        while l <= 780.0 {
            let s = planck(l, 5772.0) * mu.powf(hm98_alpha(l));
            let c = cmf(l);
            for k in 0..3 {
                xyz[k] += s * c[k];
            }
            l += 1.0;
        }
        [0, 1, 2].map(|r| m[r][0] * xyz[0] + m[r][1] * xyz[1] + m[r][2] * xyz[2])
    }

    /// The disc darkens toward its edge as the real photosphere does: each
    /// channel follows Hestroffer & Magnan's (1998) law weighted over that
    /// channel, and the luminance profile follows the independent Neckel &
    /// Labs (1994) fifth-order fit at 579.88 nm (HM98 Table 1) to within the
    /// wavelength gap, over the range HM98 fitted the law to (r <= 0.9 of the
    /// radius, mu >= 0.436). Within the last tenth of the radius the power law
    /// runs darker than the measured limb (0.34 against NL's 0.43 at mu 0.12,
    /// and 0 against 0.28 at the edge itself): under half a pixel on a disc
    /// drawn 3 to 6 pixels in radius, and seen only when the disc is dim.
    ///
    /// Seen red 2026-10-21 against the old type-17 profile (mu^3 in every
    /// channel): "channel 0 at mu 0.90: 0.7290, the photosphere gives 0.9554".
    #[test]
    fn the_limb_darkens_as_the_photosphere_does() {
        let c1 = srgb_at(1.0);
        for &mu in &[0.9_f64, 0.7, 0.5, 0.3, 0.15] {
            let want = srgb_at(mu);
            let got = limb_darkening(mu as f32);
            for c in 0..3 {
                let w = want[c] / c1[c];
                assert!((got[c] as f64 - w).abs() < 0.01, "channel {c} at mu {mu:.2}: {:.4}, the photosphere gives {w:.4}", got[c]);
            }
        }
        // Neckel & Labs (1994) at 579.88 nm, the coefficients quoted in
        // Hestroffer & Magnan (1998) Table 1.
        let nl = [0.28392, 1.36896, -1.75998, 2.22154, -1.56074, 0.44630];
        for i in 44..=100 {
            let mu = i as f64 / 100.0;
            let p5: f64 = nl.iter().enumerate().map(|(k, a)| a * mu.powi(k as i32)).sum();
            let ld = limb_darkening(mu as f32);
            let lum = (0.2126 * ld[0] + 0.7152 * ld[1] + 0.0722 * ld[2]) as f64;
            assert!((lum - p5).abs() < 0.035, "luminance at mu {mu:.2}: {lum:.4}, Neckel & Labs {p5:.4}");
        }
    }

    /// Bessel J1 by its integral form, J1(x) = (1/pi) * integral over (0, pi)
    /// of cos(t - x sin t) dt.
    fn bessel_j1(x: f64) -> f64 {
        // The midpoint rule is exponentially accurate here once n exceeds x.
        let n = (2.0 * x) as usize + 400;
        let mut acc = 0.0;
        for i in 0..n {
            let t = (i as f64 + 0.5) / n as f64 * PI;
            acc += (t - x * t.sin()).cos();
        }
        acc / n as f64
    }

    /// Brute-force convolution of the `mu^alpha` disc of radius `r` (centre
    /// radiance 1) with the glare's far field `C / rho^3` (C = 1), at `theta`.
    fn brute_glare(theta: f64, r: f64, alpha: f64) -> f64 {
        let (nr, np) = (400, 400);
        let mut s = 0.0;
        for i in 0..nr {
            let u = (i as f64 + 0.5) / nr as f64;
            let rr = r * (1.0 - (1.0 - u) * (1.0 - u));
            let dr = r * 2.0 * (1.0 - u) / nr as f64;
            let mu = (1.0 - (rr / r).powi(2)).max(0.0).sqrt();
            for j in 0..np {
                let ph = (j as f64 + 0.5) / np as f64 * PI;
                let (dx, dy) = (theta - rr * ph.cos(), rr * ph.sin());
                let d = (dx * dx + dy * dy).sqrt();
                s += 2.0 * mu.powf(alpha) * rr * dr * (PI / np as f64) / (d * d * d);
            }
        }
        s
    }

    /// The glare carries exactly the light the aperture's diffraction throws
    /// out of the disc, no more: (a) the Airy pattern the glare is taken from
    /// is normalised (it integrates to 1 over the sky); (b) the far field the
    /// shader uses is that pattern's ring average; (c) the shader's closed form
    /// is the limb-darkened disc convolved with it; (d) integrated over the
    /// sky, the glare's light equals the share the disc gives up, so disc and
    /// glare together are the Sun's irradiance.
    ///
    /// Seen red 2026-10-21 against the old corona halo (a 2.2-intensity glow on
    /// a sphere 3x the floored disc, whatever the Sun's light), at part (c):
    /// "at 1.031 radii the closed form gives 2.1958e0, the convolution
    /// 3.6993e-3". Parts (a) and (b), the optics alone, passed then too.
    #[test]
    fn the_glare_carries_the_light_the_aperture_spills() {
        let lam = CHANNEL_WAVELENGTH_M[1];
        let d = GLARE_APERTURE_M;
        // (a) integral of I0 (2 J1(x)/x)^2 dOmega, I0 = pi D^2 / (4 lambda^2):
        // in x = pi D theta / lambda this is 2 * integral (2J1/x)^2 x dx / 4 * ...
        // = (1/2) * integral of (2 J1(x)/x)^2 x dx, which is 1.
        let (xmax, n) = (400.0, 40_000);
        let mut acc = 0.0;
        for i in 0..n {
            let x = (i as f64 + 0.5) / n as f64 * xmax;
            let a = 2.0 * bessel_j1(x) / x;
            acc += a * a * x * xmax / n as f64;
        }
        let total = 0.5 * acc + 2.0 / (PI * xmax); // + the analytic tail beyond xmax
        assert!((total - 1.0).abs() < 0.003, "the Airy pattern integrates to {total:.5}");
        // (b) ring average at theta = 1 degree against lambda / (pi^3 D theta^3).
        let theta = 1.0_f64.to_radians();
        let x0 = PI * d * theta / lam;
        let i0 = PI * d * d / (4.0 * lam * lam);
        let (m, span) = (2000, 2.0 * PI);
        let ring: f64 = (0..m)
            .map(|k| {
                let x = x0 + (k as f64 + 0.5) / m as f64 * span;
                let a = 2.0 * bessel_j1(x) / x;
                i0 * a * a * (x / x0).powi(3)
            })
            .sum::<f64>()
            / m as f64;
        let far = glare_coeff(1, d) / theta.powi(3);
        assert!((ring / far - 1.0).abs() < 0.02, "ring average {ring:.4e} against the far field {far:.4e}");
        // (c) the closed form against the brute-force convolution, unit centre
        // radiance, from just outside the edge to ten radii.
        let r = angular_radius(AU_M);
        let e_unit = PI * r * r * disc_mean_over_centre(LIMB_ALPHA[1]) as f64; // irradiance of a unit-centre disc
        for &s in &[0.97_f64, 0.9, 0.8, 0.6, 0.4, 0.2, 0.1] {
            let th = r / s;
            let brute = brute_glare(th, r, LIMB_ALPHA[1] as f64) * glare_coeff(1, d);
            let closed = disc_glare(th, r, e_unit, 1, d);
            assert!((closed / brute - 1.0).abs() < 0.07, "at {:.3} radii the closed form gives {closed:.4e}, the convolution {brute:.4e}", 1.0 / s);
        }
        // (d) the glare's light over the sky, per unit irradiance, against
        // the share the disc gives up.
        // Small-angle measure (the glare's far field is a small-angle law),
        // log-spaced to a radian, then the far field's own tail beyond it.
        let k = 4000;
        let (t0, t1) = (r / GLARE_S_MAX, 1.0_f64);
        let mut light = 0.0;
        for i in 0..k {
            let a = (t0.ln() + (t1.ln() - t0.ln()) * (i as f64 + 0.5) / k as f64).exp();
            let da = a * (t1.ln() - t0.ln()) / k as f64;
            light += disc_glare(a, r, 1.0, 1, d) * 2.0 * PI * a * da;
        }
        light += 2.0 * PI * glare_coeff(1, d) / t1;
        let spill = spill_fraction(r, 1, d);
        assert!((light / spill - 1.0).abs() < 0.03, "the glare holds {light:.4} of the sky's light per unit irradiance, the aperture spills {spill:.5}");
        assert!(spill > 1.0e-5 && spill < 1.0e-2, "a 30 mm aperture spills {spill}");
    }

    /// The shaders carry the same numbers as this file: the limb exponents
    /// (the disc, the sky's disc, the visibility taps) and the glare fit (the
    /// present pass).
    #[test]
    fn the_shaders_carry_the_same_numbers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/shaders");
        let read = |f: &str| std::fs::read_to_string(root.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        let shared = read("pbr/80-fragment-shared.wgsl");
        let alpha = format!("vec3<f32>({:.4}, {:.4}, {:.4})", LIMB_ALPHA[0], LIMB_ALPHA[1], LIMB_ALPHA[2]);
        assert!(shared.contains(&format!("const SUN_LIMB_ALPHA: vec3<f32> = {alpha};")), "80-fragment-shared.wgsl lost {alpha}");
        assert!(shared.contains(&format!("const SUN_DISC_FLOOR_PX: f32 = {DISC_FLOOR_PX:.1};")));
        assert!(read("sun_vis.wgsl").contains(&format!("const VIS_LIMB_ALPHA: f32 = {:.4};", LIMB_ALPHA[1])));
        let present = read("present.wgsl");
        for (name, v) in [("B", GLARE_FIT_B), ("C", GLARE_FIT_C), ("P", GLARE_FIT_P)] {
            let want = format!("const SUN_GLARE_{name}: vec3<f32> = vec3<f32>({:.4}, {:.4}, {:.4});", v[0], v[1], v[2]);
            assert!(present.contains(&want), "present.wgsl lost {want}");
        }
        assert!(present.contains(&format!("const SUN_GLARE_S_MAX: f32 = {GLARE_S_MAX:.2};")));
    }

    /// Baumbach's corona, at the values his formula gives (Reginald eq. 5.5).
    #[test]
    fn the_corona_is_baumbachs() {
        assert_eq!(baumbach_corona(0.99), 0.0);
        assert!((baumbach_corona(1.0) - (0.0532 + 1.425 + 2.565)).abs() < 1e-9);
        let at2 = 0.0532 / 2f64.powf(2.5) + 1.425 / 2f64.powi(7) + 2.565 / 2f64.powi(17);
        assert!((baumbach_corona(2.0) - at2).abs() < 1e-12);
    }

    /// The disc's light, from its centre radiance and limb darkening, is the
    /// lighting's irradiance at 1 AU less the glare's share: one Sun, not a
    /// picture of one with a separate light.
    #[test]
    fn the_disc_gives_the_light_the_world_is_lit_by() {
        let r = angular_radius(AU_M);
        let l0 = centre_radiance();
        let e = irradiance_1au();
        for c in 0..3 {
            let disc = l0[c] as f64 * PI * r * r * disc_mean_over_centre(LIMB_ALPHA[c]) as f64;
            let glare = e[c] as f64 * spill_fraction(r, c, GLARE_APERTURE_M);
            assert!(((disc + glare) / e[c] as f64 - 1.0).abs() < 1e-4, "channel {c}: disc {disc:.4} + glare {glare:.5} against {:.4}", e[c]);
        }
        // About 46,000 times a sunlit white wall (2.5 / pi), the real ratio.
        let wall = super::super::key_lights::CELESTIAL_SUN_I as f64 / PI;
        let mean_y = (super::super::key_lights::CELESTIAL_SUN_I as f64) / (PI * r * r);
        assert!((mean_y / wall / 46_000.0 - 1.0).abs() < 0.05, "the disc is {:.0} walls bright", mean_y / wall);
    }

    /// THE APERTURE IS A SETTING (the increment 7 follow-ups, 2026-10-22: it was the showcase key
    /// `sun_aperture_mm` alone, against the GUI-first rule). Settings > Graphics > Light and sky
    /// gives it in millimetres (`SettingsState::sun_aperture_mm`, saved with the other graphics
    /// settings), and the glare takes it in metres, held between `APERTURE_MIN_MM` and
    /// `APERTURE_MAX_MM`, the default where it is not a number; and lib.rs hands it to the renderer
    /// every frame unless the showcase pins one. A narrower aperture spreads the glare wider:
    /// a 3 mm phone camera's glare constant is ten times a 30 mm lens's.
    ///
    /// Seen red first (2026-10-22) with the setting not read (the aperture always 30 mm): "a
    /// 3 mm setting / left: 0.03 / right: 0.003".
    #[test]
    fn the_aperture_is_a_setting() {
        assert_eq!(aperture_from_setting_mm(3.0), 0.003, "a 3 mm setting");
        assert_eq!(aperture_from_setting_mm(30.0), GLARE_APERTURE_M, "the default setting is the default aperture");
        assert_eq!(aperture_from_setting_mm(0.0), APERTURE_MIN_MM as f64 / 1000.0, "held at the narrowest");
        assert_eq!(aperture_from_setting_mm(5000.0), APERTURE_MAX_MM as f64 / 1000.0, "held at the widest");
        assert_eq!(aperture_from_setting_mm(f32::NAN), GLARE_APERTURE_M, "not a number: the default");
        assert!((glare_coeff(1, aperture_from_setting_mm(3.0)) / glare_coeff(1, GLARE_APERTURE_M) - 10.0).abs() < 1e-6, "a narrower aperture spreads the glare wider");
        let lib = include_str!("../lib.rs").replace("\r\n", "\n");
        assert!(
            lib.contains("sun_disc::aperture_from_setting_mm(state.gui_state.settings.sun_aperture_mm)"),
            "src/lib.rs never hands the Settings aperture to the renderer"
        );
    }
}
