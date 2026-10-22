//! The engine's KEY lights, as testable math (extracted from `lib.rs`'s frame
//! loop in v0.1104 alongside the indirect-light work; it is the same subject
//! and it was the only untested part of it).
//!
//! There is exactly one fill-light slot in the camera uniform, and as of
//! v0.1104 it carries exactly one thing: MOONLIGHT.
//!
//! What it used to carry as well was a "daylight fill" - a world-fixed
//! direction (-0.5, 0.3, -0.3) in a cool blue at intensity 0.6, roughly 12.4%
//! of the sun. That was this engine's entire stand-in for indirect light, and
//! it was wrong in three ways at once: the direction is fixed in WORLD space,
//! so on a rotating true-scale planet it sinks below the local horizon over a
//! day and gives whole classes of surface nothing; it is N.L-gated, so it
//! lights a hemisphere rather than a sky; and it runs through the full
//! `evaluate_light`, so it paints a physically meaningless specular lobe on
//! everything. Real sky irradiance now arrives per fragment from
//! [`super::sky_ambient`], so the daylight fill is deleted rather than stacked
//! (stacking would add ~27% of extra ambient to every up-facing surface).
//!
//! Moonlight stays because it is a genuine key light: it tracks the real Moon,
//! its phase is real, and without it the v0.1052 terrain terminator gate
//! leaves a moonless night at ambient only - measured in the probe rig as very
//! nearly pure black. Physically the Moon is ~1e-6 of daylight, which no
//! display can show, so like every game it is lifted to a few percent.

use glam::{DVec3, Vec3};

/// The fill-light slot for this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillLight {
    /// World-space direction toward the light.
    pub dir: Vec3,
    /// Linear RGB.
    pub rgb: [f32; 3],
    /// Strength, in the same units as the sun's 2.5. Zero means "off", which
    /// is the daytime and deep-space answer.
    pub intensity: f32,
}

/// Moonlight tint. Cool, and deliberately not the sun's warm white: at night
/// the eye's scotopic response shifts blue (the Purkinje shift), which is why
/// moonlit scenes read as blue in film and in life.
pub const MOON_RGB: [f32; 3] = [0.62, 0.68, 0.90];

/// Floor under the moonlight strength once night has fallen, so a moonless or
/// set-Moon night is navigable rather than a black screen. Starlight and
/// airglow do light the ground a little, and a game has to be playable.
pub const NIGHT_FLOOR_I: f32 = 0.12;

/// Additional strength at a risen full Moon.
pub const MOON_FULL_I: f32 = 0.26;

/// The fill light for a point on a planet.
///
/// `up` is the LOCAL RADIAL up in world space - the planet-local anchor with
/// the planet's current spin already applied. Without the spin, both the
/// sunset test and the moonrise test are meaningless (measured before v0.1052:
/// night never triggered at all). Pass a zero vector when there is no ground.
///
/// `moon_dir` / `sun_dir` point from here toward the body; pass zero for a
/// Moon that is not in the scene.
pub fn moon_fill(up: DVec3, moon_dir: DVec3, sun_dir: DVec3) -> FillLight {
    let off = FillLight { dir: Vec3::Y, rgb: MOON_RGB, intensity: 0.0 };
    if up.length_squared() < 0.5 || moon_dir.length_squared() < 0.5 {
        return off;
    }
    // How far past sunset we are, at this spot.
    let mu_sun = up.dot(sun_dir) as f32;
    let night = 1.0 - ((mu_sun + 0.05) / 0.15).clamp(0.0, 1.0);
    // Moon above the local horizon, and how full it is (sun-Moon elongation
    // as seen from here): +1 at new moon (Moon toward the Sun), -1 at full,
    // so the illuminated fraction is (1 - e) / 2.
    let risen = ((up.dot(moon_dir) as f32 + 0.02) / 0.12).clamp(0.0, 1.0);
    let phase = ((1.0 - moon_dir.dot(sun_dir) as f32) * 0.5).clamp(0.0, 1.0);
    let moon_i = NIGHT_FLOOR_I + MOON_FULL_I * risen * phase;
    FillLight {
        // Always AIM at the Moon: intensity is what switches this light on
        // now that the daylight fill is gone, and it reaches zero before
        // sunrise, so there is no daytime direction left to fall back to.
        dir: Vec3::new(moon_dir.x as f32, moon_dir.y as f32, moon_dir.z as f32),
        rgb: MOON_RGB,
        intensity: moon_i * night,
    }
}

// ── THE SUN A BODY IN THE SKY IS LIT BY (2026-10-21) ──────────────────────
//
// The celestial pass has ONE sun in its camera uniform, and it is the sun AT
// THE CAMERA: lib.rs tints its colour by the air's transmittance toward the
// Sun from where the camera stands (`atmosphere::sun_transmittance`, v0.915
// and v0.1004) and the pass scales its strength by the camera's day factor
// (`celestial_sun_day`, BUG-057 #1). Both are right for the ground and
// everything near the camera, which the Sun stops lighting at sunset. They
// were wrong for another body in the sky: the Sun lights the Moon whatever
// time it is where the camera stands, so from the ground at night the Moon
// was drawn black and its phase could not be seen, which is exactly when it
// is up to be seen (found 2026-10-21 by BUG-140's rig frame).
//
// So a body that is NOT the one the camera stands on or flies over carries its
// own sunlight in its material's `params2` ([`sky_body_sun`]): the Sun before
// any air, with day factor 1, coloured by the air on the camera's own line of
// sight TO THAT BODY, never toward the Sun. The shader's `body_sun()`
// (80-fragment-shared.wgsl, Rust twin [`celestial_sun_for`]) reads it.

/// The Sun's colour before any atmosphere, linear RGB: the colour lib.rs
/// hands the lights before the camera's air tints it.
pub const SUN_RGB: [f32; 3] = [1.0, 0.97, 0.92];

/// The Sun's strength in the celestial pass at full day (the pass writes it
/// times the camera's day factor into `sun_direction.w`).
pub const CELESTIAL_SUN_I: f32 = 2.5;

/// The camera's day factor: 0 once the Sun is a little under the local
/// horizon, 1 a little above, from the cosine of the Sun's elevation where the
/// camera stands. The celestial pass scales its sun by it (BUG-057 #1).
pub fn camera_day_factor(mu_sun: f32) -> f32 {
    ((mu_sun + 0.02) / 0.27).clamp(0.0, 1.0)
}

/// A body's `params2` for the celestial pass.
///
/// `frame_body`: the body the camera stands on or flies over (the frame
/// lock's). Its ground, sea, trees and everything on it keep the camera's
/// sun, so it gets zeros, which the shader reads as "absent".
///
/// `los`: the per-channel transmittance of the camera's air on the line of
/// sight to this body (`atmosphere::sun_transmittance` at the body's own
/// elevation), or None when the camera is not inside an atmosphere or the
/// material is shared by several bodies. Only its COLOUR is applied here:
/// the atmosphere dome is drawn over the sky bodies and already dims them by
/// its grey transmittance on that same line (the blend's `1 - alpha`, the
/// mean of the three channels, 30-atmosphere.wgsl), so applying the
/// brightness again would dim them twice. Colour times that grey is the line
/// of sight's own transmittance, channel for channel.
pub fn sky_body_sun(frame_body: bool, los: Option<[f32; 3]>) -> [f32; 4] {
    if frame_body {
        return [0.0; 4];
    }
    // The line's colour without its brightness: the transmittance over its
    // own grey. A body under the horizon (all three zero) is hidden by the
    // planet anyway; it keeps the plain Sun rather than a division by zero.
    let chroma = match los {
        Some(t) if (t[0] + t[1] + t[2]) > 1.0e-6 => {
            let grey = (t[0] + t[1] + t[2]) / 3.0;
            [t[0] / grey, t[1] / grey, t[2] / grey]
        }
        _ => [1.0; 3],
    };
    [SUN_RGB[0] * chroma[0], SUN_RGB[1] * chroma[1], SUN_RGB[2] * chroma[2], CELESTIAL_SUN_I]
}

/// One body surface the celestial loop drew this frame: its material, whether
/// that body is the frame body, and the unit direction from the camera to its
/// centre in the render frame.
#[derive(Debug, Clone, Copy)]
pub struct BodyDraw {
    pub material: usize,
    pub frame_body: bool,
    pub dir: DVec3,
}

/// The camera's air, when it stands in one: the shell-unit inputs of
/// `atmosphere::sun_transmittance` (camera radius, planet radius, scale
/// height, the planet's `atmosphere_color`) and the camera's local up in the
/// render frame.
#[derive(Debug, Clone, Copy)]
pub struct CameraAir {
    pub cam_r: f32,
    pub rp: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub up: DVec3,
}

/// Every drawn body material's `params2` this frame, one entry per material.
/// A material any frame-body draw uses keeps the camera's sun (zeros), even
/// if a sky body shares it, because the ground must never be lit after
/// sunset. A material only one sky body uses takes the colour of the air on
/// the line of sight to that body; one shared by several takes the plain Sun
/// (no single line of sight is theirs).
pub fn sky_body_params(draws: &[BodyDraw], air: Option<&CameraAir>) -> Vec<(usize, [f32; 4])> {
    // Per material: does the frame body use it, how many draws, and the line
    // of sight of the first.
    let mut mats: Vec<(usize, bool, u32, DVec3)> = Vec::new();
    for d in draws {
        match mats.iter_mut().find(|m| m.0 == d.material) {
            Some(m) => {
                m.1 |= d.frame_body;
                m.2 += 1;
            }
            None => mats.push((d.material, d.frame_body, 1, d.dir)),
        }
    }
    mats.into_iter()
        .map(|(material, frame_body, users, dir)| {
            let los = match (air, users) {
                (Some(a), 1) => Some(super::atmosphere::sun_transmittance(a.cam_r, a.up.dot(dir) as f32, a.rp, a.h, a.color)),
                _ => None,
            };
            (material, sky_body_sun(frame_body, los))
        })
        .collect()
}

/// What the shader's `body_sun()` returns for a fragment: the sky body's own
/// sunlight when its material carries one, otherwise the camera's sun. The
/// cloud deck (type 15) uses `params2` for its slab and is never a sky body.
pub fn celestial_sun_for(params2: [f32; 4], material_type: f32, camera_rgb: [f32; 3], camera_i: f32) -> ([f32; 3], f32) {
    let cloud = (14.5..15.5).contains(&material_type);
    if params2[3] > 0.0 && !cloud {
        ([params2[0], params2[1], params2[2]], params2[3])
    } else {
        (camera_rgb, camera_i)
    }
}

// ── SKY BODIES, LEFT BY BUG-211 AND BUG-212 (2026-10-21) ──────────────────
//
// Four more places where the camera's own sun or the camera's own sky decided
// how another body looks, each with its Rust twin here and its shader half
// named beside it.

/// The camera's sun hands over to the plain Sun as the camera climbs out of
/// the frame body's air: 0 below `ceiling_m` (the frame lock's co-rotation
/// ceiling, 100 km for Earth, scaled by the body's size like every band),
/// 1 from twice it up.
///
/// The camera's sun is shut after sunset where the camera stands (its colour
/// by the air toward the Sun, its strength by `camera_day_factor`), which is
/// right for everything beside the camera: nothing shadows a tree from a sun
/// under the horizon but that gate. From low orbit over the night side it
/// shut the whole frame body as well, the day side past the terminator and
/// the blue limb included, though nothing near the camera is left to light
/// there: the ground, the sea, the clouds and the air each have their own
/// terminator, per fragment and per sample (found by reading the code, BUG-212's
/// list, and seen in the rig: `release_tests`).
pub fn camera_sun_release(alt_m: f64, ceiling_m: f64) -> f32 {
    let lo = ceiling_m.max(1.0);
    let t = ((alt_m - lo) / lo).clamp(0.0, 1.0);
    (t * t * (3.0 - 2.0 * t)) as f32
}

/// The camera's sun after [`camera_sun_release`]: its colour (the air's tint
/// toward the Sun) and its day factor, each eased toward the plain Sun's (1).
pub fn released_camera_sun(tint: [f32; 3], day: f32, release: f32) -> ([f32; 3], f32) {
    let r = release.clamp(0.0, 1.0);
    (
        [tint[0] + (1.0 - tint[0]) * r, tint[1] + (1.0 - tint[1]) * r, tint[2] + (1.0 - tint[2]) * r],
        day + (1.0 - day) * r,
    )
}

/// The sun a SHELL (an atmosphere, type 14, or a cloud deck, type 15) is lit
/// by: the camera's for the frame body's own shells, the Sun before any air
/// for another body's. Rust twin of `layer_sun()` in 80-fragment-shared.wgsl:
/// a shell is the frame body's when its centre is within 1% of the frame
/// body's radius of the frame body's centre (`frame` = [centre, radius], w <= 0
/// when there is none, and then every shell keeps the camera's sun).
pub fn layer_sun_for(layer_centre: [f32; 3], frame: [f32; 4], camera_sun: [f32; 4], bare_sun: [f32; 4]) -> [f32; 4] {
    if frame[3] <= 0.0 || bare_sun[3] <= 0.0 {
        return camera_sun;
    }
    let d = Vec3::from_array(layer_centre).distance(Vec3::new(frame[0], frame[1], frame[2]));
    if d < 0.01 * frame[3] {
        camera_sun
    } else {
        bare_sun
    }
}

/// One body the celestial loop drew this frame, as a candidate for the disc
/// the camera's air lets show through ([`sky_disc`]).
#[derive(Debug, Clone, Copy)]
pub struct DiscCandidate {
    /// Unit direction from the camera to the body's centre, render frame.
    pub dir: DVec3,
    /// The DRAWN disc's angular radius, radians (the floored one).
    pub ang_radius: f64,
    /// True when the disc was floored to stay visible: a point-like body
    /// (every planet), which the day sky hides like a star.
    pub floored: bool,
    /// The Sun (its own window in the dome) or the frame body.
    pub excluded: bool,
}

/// The body the camera's air lets show through by day and at twilight: the
/// largest drawn true-size disc that is neither the Sun nor the frame body.
/// From Earth that is the Moon (0.26 degrees); every planet is a floored
/// point, hidden by day like the stars, as a daytime Venus is all but
/// invisible to the eye. `[dir, angular radius]`, zeros when none. The dome
/// reads it as camera.light0_spot (30-atmosphere.wgsl `sky_disc_cover`).
pub fn sky_disc(cands: &[DiscCandidate]) -> [f32; 4] {
    cands
        .iter()
        .filter(|c| !c.excluded && !c.floored && c.ang_radius > 0.0)
        .max_by(|a, b| a.ang_radius.total_cmp(&b.ang_radius))
        .map(|c| [c.dir.x as f32, c.dir.y as f32, c.dir.z as f32, c.ang_radius as f32])
        .unwrap_or([0.0; 4])
}

/// How much of a ray is that disc, 0..1 across one pixel's angle at its edge.
/// Twin of `sky_disc_cover` in 30-atmosphere.wgsl.
pub fn sky_disc_cover(rd: Vec3, disc: [f32; 4], px_angle: f32, applies: bool) -> f32 {
    if !applies || disc[3] <= 0.0 {
        return 0.0;
    }
    let ang = 2.0 * (0.5 * (rd - Vec3::new(disc[0], disc[1], disc[2])).length()).clamp(0.0, 1.0).asin();
    let px = px_angle.max(1.0e-6);
    let t = ((ang - (disc[3] - px)) / (2.0 * px)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The atmosphere dome's coverage of what lies behind a sky ray: the sky's
/// (`alpha_sky`, the stars' rule: raised to 0.985 by day and by the sky's own
/// brightness at twilight; since 2026-10-21 right up to the Sun's disc, which
/// the shell now draws itself, so the old window round the Sun is gone) and
/// the coverage over the body seen through it (`alpha_body`, the plain
/// transmittance coverage `alpha` on its disc). Twin of the end of
/// `atmosphere_scattering`.
pub fn dome_cover(alpha: f32, sky_lum: f32, day: f32, disc_cover: f32, hits_surface: bool) -> (f32, f32) {
    let mut a = alpha;
    if !hits_surface {
        a = alpha.max((sky_lum * 4.5).clamp(0.0, 1.0).max(day * 0.985));
    }
    let alpha_sky = a;
    let alpha_body = alpha_sky + (alpha - alpha_sky) * disc_cover;
    (alpha_sky, alpha_body)
}

/// The weather's share of the aerial layer's optical depth: 0 in clear air,
/// toward 1 as the fog thickens. The clear-air haze is the dome's own
/// extinction already; only the weather's excess hides more of the sky
/// behind it. lib.rs passes it to the dome as camera.light1_spot.y.
pub fn fog_share(sigma_clear: f32, cap_clear: f32, sigma_weather: f32, cap_weather: f32) -> f32 {
    let total = sigma_weather * cap_weather;
    if total <= 1.0e-12 {
        return 0.0;
    }
    (1.0 - sigma_clear * cap_clear / total).clamp(0.0, 1.0)
}

/// What the fog passes of what is behind it: exp(-tau * share), tau the
/// layer's slant optical depth on the ray. Twin of `fog_pass` in
/// 30-atmosphere.wgsl.
pub fn fog_passes(tau: f32, share: f32) -> f32 {
    (-tau * share.clamp(0.0, 1.0)).exp()
}

/// What a sky ray shows: the dome's light (its tone-mapped in-scatter
/// `mapped`, capped by the sky's coverage, as the old straight blend capped
/// it) plus what lies behind passed through the body's coverage. Twin of the
/// premultiplied output of `atmosphere_scattering` and the Shell pipeline's
/// blend (One, OneMinusSrcAlpha).
pub fn dome_over(behind: [f32; 3], mapped: [f32; 3], alpha_sky: f32, alpha_body: f32) -> [f32; 3] {
    let cap = alpha_sky;
    let mut out = [0.0; 3];
    for c in 0..3 {
        let rgb = (mapped[c] / cap.max(1.0e-3)).clamp(0.0, 1.0);
        out[c] = rgb * cap + behind[c] * (1.0 - alpha_body);
    }
    out
}

#[cfg(test)]
mod sky_body_tests {
    use super::*;
    use crate::renderer::atmosphere;

    /// A direction from its elevation and azimuth (degrees; azimuth 0 = +X,
    /// 90 = -Z), in a frame whose local up is +Y.
    fn dir(elev_deg: f64, az_deg: f64) -> DVec3 {
        let (e, a) = (elev_deg.to_radians(), az_deg.to_radians());
        DVec3::new(e.cos() * a.cos(), e.sin(), -e.cos() * a.sin())
    }

    /// FROM THE GROUND AT NIGHT THE MOON IS LIT BY THE SUN (2026-10-21).
    /// BUG-140's rig frame over Silverdale at 20:04 local: the Sun 20.6
    /// degrees under the west-north-west horizon, the Moon 39.6 degrees up due
    /// south, waxing gibbous. The camera's own sun is shut, as it must be for
    /// the ground (no light, no day). The Moon's material must carry the Sun
    /// before any air at full strength, coloured by the air on the line of
    /// sight to the MOON, so that the dome's grey on that line times this
    /// colour is the line's own transmittance, channel for channel. The ground
    /// (the frame body) keeps the camera's sun; the cloud deck never reads
    /// `params2` as a sky body. Earth's air is read from the shipped
    /// data/planets/earth.ron.
    ///
    /// Seen red 2026-10-21 with every body taking the camera's sun (`params2`
    /// zero, the shader's only sun until then): "from the ground at night the
    /// Moon is lit at strength 0.000, colour [0.0, 0.0, 0.0]: drawn black".
    #[test]
    fn from_the_ground_at_night_the_moon_is_lit_by_the_sun_and_the_ground_is_not() {
        let earth: crate::terrain::planet::PlanetDef =
            ron::from_str(crate::embedded_data::PLANET_EARTH_RON).expect("earth.ron parses");
        let ac = earth.atmosphere_color.expect("Earth has air");
        let (rp, h) = atmosphere::shell_packing(earth.atmosphere_scale, earth.scale_height_or_default(), earth.radius);
        // Standing on the ground: the frame clamps the camera's radius to the surface.
        let cam_r = rp;
        let up = DVec3::Y;
        let sun = dir(-20.6, 160.0);
        let moon = dir(39.6, 270.0);

        // The camera's sun, built the way lib.rs and the celestial pass build it.
        let mu_sun = up.dot(sun) as f32;
        let tint = atmosphere::sun_transmittance(cam_r, mu_sun, rp, h, ac);
        let cam_rgb = [SUN_RGB[0] * tint[0], SUN_RGB[1] * tint[1], SUN_RGB[2] * tint[2]];
        let cam_i = CELESTIAL_SUN_I * camera_day_factor(mu_sun);
        assert_eq!(cam_i, 0.0, "the camera's sun is shut at night (right for the ground)");

        // The Moon.
        let los = atmosphere::sun_transmittance(cam_r, up.dot(moon) as f32, rp, h, ac);
        let grey = (los[0] + los[1] + los[2]) / 3.0;
        assert!(grey > 0.3 && grey < 1.0, "Earth's air 40 degrees up passes {grey}");
        let (rgb, i) = celestial_sun_for(sky_body_sun(false, Some(los)), 12.0, cam_rgb, cam_i);
        assert!(
            i == CELESTIAL_SUN_I,
            "from the ground at night the Moon is lit at strength {i:.3}, colour {rgb:?}: drawn black"
        );
        for c in 0..3 {
            // Through the dome (its grey) the Moon gets the line of sight's own transmittance.
            let seen = rgb[c] * grey;
            let want = SUN_RGB[c] * los[c];
            assert!((seen - want).abs() < 1.0e-5, "channel {c}: the Moon reaches the eye at {seen}, the air on its line passes {want}");
        }
        assert!(rgb[0] / SUN_RGB[0] > rgb[2] / SUN_RGB[2], "the air reddens the Moon, never blues it: {rgb:?}");

        // Outside any air the Moon takes the plain Sun.
        let (bare, bi) = celestial_sun_for(sky_body_sun(false, None), 12.0, cam_rgb, cam_i);
        assert_eq!((bare, bi), (SUN_RGB, CELESTIAL_SUN_I));

        // The ground keeps the camera's sun, and the cloud deck never reads params2 as a sky body.
        assert_eq!(celestial_sun_for(sky_body_sun(true, Some(los)), 12.0, cam_rgb, cam_i), (cam_rgb, cam_i));
        assert_eq!(celestial_sun_for([1.0, 1.0, 1.0, 4.0], 15.0, cam_rgb, cam_i), (cam_rgb, cam_i));
    }

    /// Which drawn material gets which sun. From the ground: Earth's own
    /// material (the frame body) keeps the camera's sun; the Moon's own
    /// material takes the colour of the air on ITS line of sight, not the
    /// Sun's; a material shared by two sky bodies takes the plain Sun; a
    /// material the frame body shares with a sky body stays with the ground.
    /// One entry per material, whatever the order of the draws.
    ///
    /// Seen red 2026-10-21 with the frame writing nothing: "4 materials drawn,
    /// 0 given a sun".
    #[test]
    fn each_drawn_material_takes_the_sun_its_bodies_need() {
        let earth: crate::terrain::planet::PlanetDef =
            ron::from_str(crate::embedded_data::PLANET_EARTH_RON).expect("earth.ron parses");
        let (rp, h) = atmosphere::shell_packing(earth.atmosphere_scale, earth.scale_height_or_default(), earth.radius);
        let air = CameraAir { cam_r: rp, rp, h, color: earth.atmosphere_color.unwrap(), up: DVec3::Y };
        let moon_dir = dir(25.0, 200.0);
        let (earth_m, moon_m, shared_m, mixed_m) = (10, 11, 12, 13);
        let draws = [
            BodyDraw { material: earth_m, frame_body: true, dir: -DVec3::Y },
            BodyDraw { material: moon_m, frame_body: false, dir: moon_dir },
            BodyDraw { material: shared_m, frame_body: false, dir: dir(10.0, 30.0) },
            BodyDraw { material: shared_m, frame_body: false, dir: dir(60.0, 90.0) },
            BodyDraw { material: mixed_m, frame_body: false, dir: dir(30.0, 0.0) },
            BodyDraw { material: mixed_m, frame_body: true, dir: -DVec3::Y },
        ];
        let got = sky_body_params(&draws, Some(&air));
        let materials = [earth_m, moon_m, shared_m, mixed_m];
        assert!(got.len() == materials.len(), "{} materials drawn, {} given a sun", materials.len(), got.len());
        let of = |m: usize| got.iter().find(|e| e.0 == m).unwrap_or_else(|| panic!("material {m} given no sun")).1;
        assert_eq!(of(earth_m), [0.0; 4], "the ground keeps the camera's sun");
        assert_eq!(of(mixed_m), [0.0; 4], "a material the ground shares stays with the ground");
        let los = atmosphere::sun_transmittance(rp, DVec3::Y.dot(moon_dir) as f32, rp, h, air.color);
        assert_eq!(of(moon_m), sky_body_sun(false, Some(los)), "the Moon takes the air on its own line of sight");
        assert_eq!(of(shared_m), sky_body_sun(false, None), "two bodies on one material take the plain Sun");
        // Outside any air every sky body takes the plain Sun.
        let bare = sky_body_params(&draws, None);
        assert_eq!(bare.iter().find(|e| e.0 == moon_m).map(|e| e.1), Some(sky_body_sun(false, None)));
    }

    /// The shader reads what the frame writes. `body_sun()` in
    /// 80-fragment-shared.wgsl is [`celestial_sun_for`]'s twin, and every sun
    /// term a body's surface takes goes through it: the shared lighting tail,
    /// the sea's shading and the type-12 ocean glint. A sky body skips the
    /// near-scene haze (`aerial_apply`): the dome over it is the extinction on
    /// its line of sight, and the haze on top dimmed the Moon a second time
    /// (to 0.42 of itself 40 degrees up at sea level).
    ///
    /// Seen red 2026-10-21 before the shader carried it: "80-fragment-shared.wgsl
    /// has no fn body_sun()".
    #[test]
    fn the_shader_lights_a_sky_body_with_its_own_sun() {
        let read = |f: &str| {
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/shaders/pbr").join(f))
                .unwrap_or_else(|e| panic!("{f}: {e}"))
        };
        let shared = read("80-fragment-shared.wgsl");
        assert!(shared.contains("fn body_sun() -> vec4<f32>"), "80-fragment-shared.wgsl has no fn body_sun()");
        assert!(shared.contains("fn is_sky_body() -> bool"), "80-fragment-shared.wgsl has no fn is_sky_body()");
        // The twin's rule: params2.w > 0 on anything but the cloud deck.
        let rule = shared.split("fn is_sky_body() -> bool").nth(1).unwrap_or("");
        let rule = &rule[..rule.find("\n}").unwrap_or(rule.len())];
        assert!(rule.contains("material.params2.w > 0.0") && rule.contains("14.5") && rule.contains("15.5"), "is_sky_body: {rule}");
        let tail = shared.split("fn frag_tail(").nth(1).expect("frag_tail");
        assert!(!tail.contains("camera.sun_color.rgb, camera.sun_direction.w"), "frag_tail still lights everything with the camera's sun");
        assert!(tail.contains("bsun.rgb, bsun.w"), "frag_tail does not light with body_sun()");
        assert!(tail.contains("if (!is_sky_body())") && tail.contains("aerial_apply(color, in.world_position)"), "a sky body still takes the near-scene haze");
        let water = read("20-surface-detail.wgsl");
        let shade = water.split("fn water_shade(").nth(1).expect("water_shade");
        let shade = &shade[..shade.find("\n}").unwrap_or(shade.len())];
        assert!(shade.contains("body_sun()") && !shade.contains("camera.sun_color") && !shade.contains("camera.sun_direction.w"), "water_shade still reads the camera's sun");
        let main = read("90-fragment-main.wgsl");
        assert!(!main.contains("let old_glint = camera.sun_color.rgb * camera.sun_direction.w"), "the type-12 ocean glint still reads the camera's sun");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn up() -> DVec3 {
        DVec3::Y
    }

    /// THE POINT OF v0.1104: in daylight the fill contributes NOTHING. If this
    /// regresses, the old world-fixed daylight fill is back and it is being
    /// double-counted against the sky-irradiance term.
    #[test]
    fn daylight_has_no_fill_at_all() {
        // Sun overhead, Moon somewhere.
        let f = moon_fill(up(), DVec3::new(1.0, 0.3, 0.0).normalize(), DVec3::Y);
        assert_eq!(f.intensity, 0.0, "a fill light is on at noon");
        // Sun still comfortably up.
        let f = moon_fill(up(), DVec3::X, DVec3::new(0.0, 0.4, 1.0).normalize());
        assert_eq!(f.intensity, 0.0);
    }

    /// Deep space and windowless interiors: no ground, no Moon, no fill. The
    /// sun is the only light there is, and the shader's ambient floor is what
    /// keeps unlit faces off absolute black.
    #[test]
    fn no_ground_and_no_moon_means_no_fill() {
        assert_eq!(moon_fill(DVec3::ZERO, DVec3::X, DVec3::Y).intensity, 0.0);
        assert_eq!(moon_fill(up(), DVec3::ZERO, DVec3::Y).intensity, 0.0);
    }

    /// Night is navigable even with no Moon in the sky, and brighter with a
    /// risen full Moon - and it comes from where the Moon actually is.
    #[test]
    fn night_is_lit_by_the_moon_and_floored_when_there_is_none() {
        let sun_down = DVec3::new(0.0, -1.0, 0.0);
        // Moon below the horizon: only the starlight floor.
        let set = moon_fill(up(), DVec3::new(0.0, -0.9, 0.4).normalize(), sun_down);
        assert!((set.intensity - NIGHT_FLOOR_I).abs() < 1e-6, "got {}", set.intensity);
        // Risen and full (opposite the sun): floor plus the full-moon term.
        let full = moon_fill(up(), DVec3::Y, sun_down);
        assert!(
            (full.intensity - (NIGHT_FLOOR_I + MOON_FULL_I)).abs() < 1e-6,
            "got {}",
            full.intensity
        );
        assert!((full.dir - Vec3::Y).length() < 1e-6, "the fill must aim at the Moon");
        // New moon (toward the sun) is dark even when risen.
        let new = moon_fill(up(), sun_down, sun_down);
        assert!((new.intensity - NIGHT_FLOOR_I).abs() < 1e-6, "got {}", new.intensity);
    }

    /// Continuity through the terminator: no step, and never brighter than the
    /// full-night value (a step here reads on screen as a lighting pop at
    /// sunset, which is the class of bug v0.1052 was chasing).
    #[test]
    fn the_terminator_ramp_is_monotone_and_continuous() {
        // Moon at the local zenith throughout, so only the sun moves.
        let mut prev = 0.0f32;
        let mut i = 0;
        while i <= 40 {
            // Sun elevation sweeping from +0.15 (day) down to -0.05 (night).
            let mu = 0.15 - 0.005 * i as f64;
            let sun = DVec3::new((1.0 - mu * mu).sqrt(), mu, 0.0);
            let f = moon_fill(up(), DVec3::Y, sun);
            if i == 0 {
                assert_eq!(f.intensity, 0.0, "the ramp must start dark (still daylight)");
            }
            assert!(f.intensity >= prev - 1e-6, "fill dipped at mu {mu}");
            assert!(f.intensity - prev < 0.05, "fill stepped at mu {mu}");
            prev = f.intensity;
            i += 1;
        }
        // Full night. The Moon is overhead but only 52.5% illuminated at this
        // elongation, so the closed form, not the full-moon maximum.
        let want = NIGHT_FLOOR_I + MOON_FULL_I * ((1.0 - -0.05f32) * 0.5);
        assert!((prev - want).abs() < 1e-6, "night settled at {prev}, want {want}");
    }
}

#[cfg(test)]
mod sky_dome_tests {
    //! The sky bodies left by BUG-211 and BUG-212 (2026-10-21): the daytime
    //! Moon, fog, another body's shells, and the frame body from low orbit.
    use super::*;
    use crate::renderer::atmosphere;

    /// A direction from its elevation and azimuth (degrees; azimuth 0 = +X,
    /// 90 = -Z), in a frame whose local up is +Y.
    fn dir(elev_deg: f64, az_deg: f64) -> DVec3 {
        let (e, a) = (elev_deg.to_radians(), az_deg.to_radians());
        DVec3::new(e.cos() * a.cos(), e.sin(), -e.cos() * a.sin())
    }

    fn earth() -> crate::terrain::planet::PlanetDef {
        ron::from_str(crate::embedded_data::PLANET_EARTH_RON).expect("earth.ron parses")
    }

    fn lum(c: [f32; 3]) -> f32 {
        0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    }

    /// The pipeline's ACES curve (30-atmosphere.wgsl, clamped).
    fn aces(x: f32) -> f32 {
        ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
    }

    /// The grey transmittance of Earth's air from the ground to space at an
    /// elevation, and the dome's coverage from it.
    fn grey_t(elev_deg: f64) -> f32 {
        let e = earth();
        let (rp, h) = atmosphere::shell_packing(e.atmosphere_scale, e.scale_height_or_default(), e.radius);
        let t = atmosphere::sun_transmittance(rp, elev_deg.to_radians().sin() as f32, rp, h, e.atmosphere_color.unwrap());
        (t[0] + t[1] + t[2]) / 3.0
    }

    /// The Moon as the celestial pass draws it before any air, linear: the
    /// night capture of BUG-212 (Silverdale, 2026-10-21, a 79% lit Moon 42
    /// degrees up) reads the mean of its lit disc at 0.177 linear luminance
    /// (sRGB 108), through the dome's grey on that line (at night the
    /// coverage there is the plain transmittance).
    fn moon_above_the_air() -> f32 {
        0.177 / grey_t(42.0)
    }

    /// A DAYTIME MOON SHOWS THROUGH THE BLUE SKY (2026-10-21, BUG-212's list).
    /// Earth's air from the shipped earth.ron, the camera on the ground, the
    /// Sun 30 degrees up and a gibbous Moon 35 degrees up 110 degrees round
    /// the sky from it (the geometry of a late-afternoon waxing Moon). The sky
    /// beside the Moon is the dome's own (the sky-view table at the dome's
    /// exposure, through the pipeline's ACES curve); the Moon is the
    /// celestial pass's, before any air (`moon_above_the_air`). What shows is
    /// [`dome_over`] with [`dome_cover`]'s two coverages.
    ///
    /// Published figures (Nickel & Calderwood 2021, JAAVSO 49, measured V
    /// sky 3.3 to 4.8 mag per square arcsecond 50 to 95 degrees from the Sun;
    /// the CIE clear sky type 12 at a Sun 30 to 60 degrees up; the Moon's
    /// mean luminance from Krisciunas & Schaefer 1991's phase law) put a
    /// quarter-to-gibbous Moon's contrast against the sky beside it at about
    /// 0.1 to 0.7 (1.1 to 1.7 times as bright), and never near zero: the
    /// daytime Moon is plainly visible. The test asks for 0.1 to 1.0, wide
    /// enough for the game's own Moon and sky, and asks that it is lower than
    /// the same Moon's contrast at night.
    ///
    /// Seen red 2026-10-21 with the old rule (the Moon's disc covered like the
    /// stars, `dome_cover` returning the sky's coverage for both): "a gibbous
    /// Moon 35 degrees up, 110 degrees from a Sun 30 degrees up: contrast 0.0164
    /// against the sky beside it (0.2098 against 0.2064, coverage 0.985 over it,
    /// 0.985 beside it)".
    #[test]
    fn a_daytime_moon_shows_through_the_blue_sky() {
        let e = earth();
        let ac = e.atmosphere_color.unwrap();
        let (rp, h) = atmosphere::shell_packing(e.atmosphere_scale, e.scale_height_or_default(), e.radius);
        let p = crate::renderer::atmo_luts::TransLutParams { tint: [ac[0], ac[1], ac[2]], density_mul: ac[3], rp, h };
        let ms = crate::renderer::atmo_luts::multiple_scattering_lut(&p);
        let (sun_el, moon_el, apart_az) = (30.0_f64, 35.0_f64, 110.0_f64);
        let sun = dir(sun_el, 0.0);
        let moon = dir(moon_el, apart_az);
        let sky = crate::renderer::atmo_luts::sky_view_radiance(
            &p,
            rp,
            moon_el.to_radians().sin() as f32,
            apart_az.to_radians().cos() as f32,
            sun_el.to_radians().sin() as f32,
            &ms,
        );
        let mapped = [
            aces(sky[0] * crate::renderer::sky_ambient::SKY_LUT_EXPOSURE),
            aces(sky[1] * crate::renderer::sky_ambient::SKY_LUT_EXPOSURE),
            aces(sky[2] * crate::renderer::sky_ambient::SKY_LUT_EXPOSURE),
        ];
        let alpha = 1.0 - grey_t(moon_el);
        // The dome's day term at the camera (30-atmosphere.wgsl `day`), on the ground.
        let mu = DVec3::Y.dot(sun) as f32;
        let t = ((mu + 0.08) / 0.20).clamp(0.0, 1.0);
        let day = t * t * (3.0 - 2.0 * t);
        let cover_disc = sky_disc_cover(moon.as_vec3(), [moon.x as f32, moon.y as f32, moon.z as f32, 0.0045], 1.0e-4, true);
        assert!(cover_disc > 0.99, "the Moon's centre is inside its disc: {cover_disc}");
        let (a_sky, a_body) = dome_cover(alpha, lum(mapped), day, cover_disc, false);
        let m = moon_above_the_air();
        let on_moon = lum(dome_over([m; 3], mapped, a_sky, a_body));
        let beside = lum(dome_over([0.0; 3], mapped, a_sky, a_sky));
        let c_day = (on_moon - beside) / beside;
        println!("daytime Moon contrast {c_day:.3} ({on_moon:.4} against {beside:.4})");
        assert!(
            (0.1..1.0).contains(&c_day),
            "a gibbous Moon 35 degrees up, 110 degrees from a Sun 30 degrees up: contrast {c_day:.4} against the sky beside it ({on_moon:.4} against {beside:.4}, coverage {a_body:.3} over it, {a_sky:.3} beside it)"
        );
        // At night the same Moon stands out far more: the sky is near black.
        let (n_sky, n_body) = dome_cover(alpha, 0.002, 0.0, cover_disc, false);
        let night_beside = lum(dome_over([0.0; 3], [0.002; 3], n_sky, n_sky)).max(1.0e-4);
        let c_night = (lum(dome_over([m; 3], [0.002; 3], n_sky, n_body)) - night_beside) / night_beside;
        assert!(c_day < c_night, "the day sky lowers the Moon's contrast: {c_day} by day, {c_night} at night");
        // The stars stay hidden by day: off the disc the coverage is the day rule's.
        let (star_sky, star_cover) = dome_cover(alpha, lum(mapped), day, 0.0, false);
        assert!(star_sky >= 0.985 && star_cover == star_sky, "a star beside the Moon is still hidden: {star_cover}");
    }

    /// FOG HIDES THE MOON AND THE STARS BY ITS DEPTH (2026-10-21). Weather fog
    /// coloured the dome and left its coverage alone. The fog's share of the
    /// layer's optical depth now passes exp(-tau * share) of what is behind
    /// it: at night, through 60 m fog (the weather's share 0.89 of a slant
    /// depth of 7.8 toward a Moon 42 degrees up), a star or the Moon keeps
    /// under 1% of itself; in clear air (share 0) nothing changes.
    ///
    /// Seen red 2026-10-21 with the fog passing everything (`fog_passes`
    /// returning 1, the old dome): "the Moon through fog of slant depth 5:
    /// 0.17700 of 0.17700 reaches the eye" (the test used a depth of 5 then;
    /// the old dome passes all of it at any depth).
    #[test]
    fn fog_hides_the_moon_and_the_stars_by_its_depth() {
        // The weather's share, as lib.rs computes it: clear air 2.2e-5 /m over
        // a 25.5 km cap, fog at 60 m visibility (ln 50 / 60) over an 80 m cap.
        let share = fog_share(2.2e-5, 25_500.0, (50.0_f32).ln() / 60.0, 80.0);
        assert!(share > 0.85, "fog is mostly the weather's: {share}");
        assert_eq!(fog_share(2.2e-5, 25_500.0, 2.2e-5, 25_500.0), 0.0, "clear air is none of it");
        let alpha = 1.0 - grey_t(42.0);
        let (a_sky, a_body) = dome_cover(alpha, 0.0, 0.0, 1.0, false);
        // The fog layer's slant depth on the line to a Moon 42 degrees up, as the
        // dome integrates it: sigma times the 80 m cap over the sine of the elevation.
        let tau = (50.0_f32).ln() / 60.0 * 80.0 / (42.0_f32).to_radians().sin();
        let pass = fog_passes(tau, share);
        let behind = moon_above_the_air();
        let through = (1.0 - a_body) * behind;
        // What the dome passes over the Moon once the fog covers too (the
        // shader's alpha_body_f = 1 - (1 - alpha_body) * fog_pass).
        let through_fog = (1.0 - a_body) * pass * behind;
        assert!(
            through_fog < 0.01 * through,
            "the Moon through fog of slant depth {tau:.1}: {through_fog:.5} of {through:.5} reaches the eye"
        );
        let star = (1.0 - a_sky) * 1.0;
        let star_fog = (1.0 - a_sky) * pass;
        assert!(star_fog < 0.01 * star, "a star through the fog keeps {star_fog} of {star}");
        assert_eq!(fog_passes(tau, 0.0), 1.0, "clear air hides nothing more than the dome's own extinction");
    }

    /// ANOTHER BODY'S CLOUDS AND AIR TAKE THE SUN BEFORE ANY AIR (2026-10-21).
    /// Standing on the Moon's night side, Earth's cloud deck and atmosphere
    /// were lit by the camera's sun, shut by the Moon's night: Earth's lit
    /// crescent showed bare ground under unlit clouds and no limb. A shell
    /// whose centre is the frame body's keeps the camera's sun; any other
    /// takes the Sun before any air. Off any body every shell keeps the
    /// camera's, which out there is the plain Sun.
    ///
    /// Seen red 2026-10-21 with every shell on the camera's sun (the old
    /// shaders): "Earth's clouds seen from the Moon's night are lit at [0.0,
    /// 0.0, 0.0, 0.0], not the Sun before any air".
    #[test]
    fn another_bodys_clouds_and_air_take_the_sun_before_any_air() {
        let bare = [SUN_RGB[0], SUN_RGB[1], SUN_RGB[2], CELESTIAL_SUN_I];
        let night = [0.0, 0.0, 0.0, 0.0];
        // On the Moon: its centre 1,737 km below the camera, Earth 384,400 km off.
        let moon = [0.0, -1_737_450.0, 0.0, 1_737_400.0];
        let earth_centre = [2.0e8, 3.0e8, -1.0e8];
        let got = layer_sun_for(earth_centre, moon, night, bare);
        assert!(got == bare, "Earth's clouds seen from the Moon's night are lit at {got:?}, not the Sun before any air");
        // The frame body's own shells keep the camera's sun.
        assert_eq!(layer_sun_for([0.0, -1_737_450.0, 0.0], moon, night, bare), night);
        // No frame body: the camera's sun.
        assert_eq!(layer_sun_for(earth_centre, [0.0, -1.0, 0.0, -1.0], bare, bare), bare);
    }

    /// FROM LOW ORBIT OVER THE NIGHT SIDE THE FRAME BODY IS LIT BY THE SUN
    /// (2026-10-21, BUG-212's list, seen in the rig: 400 km over the equator
    /// at local 5:20, the Sun 10 degrees under the local horizontal but over
    /// the limb, drew the whole Earth dark, its day side and blue limb
    /// included). The camera's sun is the ground's gate; above the air it hands
    /// over to the plain Sun. On the ground at night it stays shut.
    ///
    /// Seen red 2026-10-21 with no release (the old frame): "400 km over the
    /// night side the frame body is lit at 0.000 by colour [0.9999086,
    /// 0.99980265, 0.9995423]: its day side drawn dark" (the air passes the Sun
    /// over the limb; only the day factor shut it).
    #[test]
    fn from_low_orbit_over_the_night_side_the_frame_body_is_lit_by_the_sun() {
        let e = earth();
        let ac = e.atmosphere_color.unwrap();
        let (rp, h) = atmosphere::shell_packing(e.atmosphere_scale, e.scale_height_or_default(), e.radius);
        let mu = (-10.0_f64).to_radians().sin() as f32;
        // The camera's own sun at 400 km, as lib.rs builds it (cam_r clamped to the shell top).
        let tint = atmosphere::sun_transmittance(1.0, mu, rp, h, ac);
        let day = camera_day_factor(mu);
        let ceiling = 100_000.0;
        let (rgb, d) = released_camera_sun(tint, day, camera_sun_release(400_000.0, ceiling));
        assert!(
            d > 0.99 && rgb.iter().all(|c| *c > 0.99),
            "400 km over the night side the frame body is lit at {d:.3} by colour {rgb:?}: its day side drawn dark"
        );
        // On the ground at night the gate holds.
        let (g_rgb, g_d) = released_camera_sun(tint, day, camera_sun_release(2.0, ceiling));
        assert_eq!(g_d, day);
        assert_eq!(g_rgb, tint);
        // It eases, never steps.
        let mut last = 0.0;
        for k in 0..=40 {
            let r = camera_sun_release(80_000.0 + k as f64 * 4_000.0, ceiling);
            assert!(r >= last && r - last < 0.1, "a step in the release at {k}");
            last = r;
        }
    }

    /// The shaders carry the twins (read from disk, as the build compiles
    /// them): the dome's coverage for the disc and the fog, its premultiplied
    /// output on the premultiplied Shell pipeline, and every shell's sun
    /// through `layer_sun()`.
    ///
    /// Seen red 2026-10-21 against the shaders and pipeline.rs before the
    /// change: "30-atmosphere.wgsl has no sky_disc_cover".
    #[test]
    fn the_shaders_let_the_moon_through_and_light_other_bodies_shells() {
        let read = |f: &str| {
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(f)).unwrap_or_else(|e| panic!("{f}: {e}"))
        };
        let atmo = read("assets/shaders/pbr/30-atmosphere.wgsl");
        assert!(atmo.contains("fn sky_disc_cover(rd: vec3<f32>, applies: bool) -> f32"), "30-atmosphere.wgsl has no sky_disc_cover");
        assert!(atmo.contains("let alpha_body = mix(alpha_sky, alpha, sky_disc_cover(rd, cam_inside && !hits_surface));"), "the dome does not let the disc through");
        assert!(atmo.contains("let fog_pass = select(1.0, exp(-tau_fog * clamp(camera.light1_spot.y, 0.0, 1.0)), !hits_surface);"), "the fog does not cover what is behind it");
        // Premultiplied: the sky's light is rgb * cap, and since 2026-10-21 the
        // Sun's disc (sun_disc.rs) is mixed into it by its coverage.
        assert!(
            atmo.contains("var out_rgb = rgb * cap;") && atmo.contains("return vec4<f32>(out_rgb, out_a);"),
            "the dome's output is not premultiplied"
        );
        assert!(atmo.contains("let lsun = layer_sun();"), "the air is lit by the camera's sun");
        let shared = read("assets/shaders/pbr/80-fragment-shared.wgsl");
        assert!(shared.contains("fn layer_sun() -> vec4<f32>") && shared.contains("fn is_frame_layer() -> bool"), "80-fragment-shared.wgsl has no layer_sun");
        let clouds = read("assets/shaders/pbr/40-clouds.wgsl");
        assert!(!clouds.contains("camera.sun_color.rgb * camera.sun_direction.w"), "a cloud site still reads the camera's sun");
        assert_eq!(clouds.matches("let sun_energy = layer_sun_energy();").count(), 4, "the deck's four sun sites");
        let pipe = read("src/renderer/pipeline.rs");
        assert!(pipe.contains("pbr(l, SHELL_TRANSPARENT_STATE)") && pipe.contains("PREMULTIPLIED_ALPHA_BLENDING"), "the Shell pipeline does not blend premultiplied");
        let main = read("assets/shaders/pbr/90-fragment-main.wgsl");
        let shell = main.split("fn fs_shell(").nth(1).expect("fs_shell");
        let shell = &shell[..shell.find("\n}").unwrap_or(shell.len())];
        assert!(shell.contains("return vec4<f32>(c.rgb * c.a, c.a);"), "fs_shell does not premultiply the fallback dome");
    }
}
