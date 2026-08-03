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
