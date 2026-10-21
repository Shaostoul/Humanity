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
