//! Sky renderer — computes sky color, ambient light, fog, and sun intensity
//! based on time of day and weather conditions.
//!
//! Pure math module with no GPU dependencies. The main renderer reads these
//! values to set clear color, lighting uniforms, and fog parameters.

use crate::systems::weather::{Weather, WeatherCondition};

/// Computes sky and lighting parameters from time-of-day and weather.
pub struct SkyRenderer {
    sky_color: [f32; 3],
    ambient_light: [f32; 3],
    fog_color: [f32; 3],
    sun_intensity: f32,
}

impl SkyRenderer {
    /// Create with default midday clear-sky values.
    pub fn new() -> Self {
        Self {
            sky_color: [0.4, 0.6, 1.0],
            ambient_light: [0.3, 0.3, 0.35],
            fog_color: [0.7, 0.75, 0.8],
            sun_intensity: 1.0,
        }
    }

    /// Recompute all sky parameters from current hour and weather.
    pub fn update(&mut self, hour: f32, weather: &Weather) {
        // Step 1: Compute base clear-sky colors from time of day
        let (base_sky, base_ambient, base_fog, base_sun) = Self::time_of_day_colors(hour);

        // Step 2: Apply weather modifications
        let (sky, ambient, fog, sun) =
            Self::apply_weather(base_sky, base_ambient, base_fog, base_sun, weather);

        self.sky_color = sky;
        self.ambient_light = ambient;
        self.fog_color = fog;
        self.sun_intensity = sun;
    }

    /// Current sky color (used as clear color / background).
    pub fn sky_color(&self) -> [f32; 3] {
        self.sky_color
    }

    /// Ambient light color and intensity for indirect illumination.
    pub fn ambient_light(&self) -> [f32; 3] {
        self.ambient_light
    }

    /// Fog / atmosphere color for distance fade.
    pub fn fog_color(&self) -> [f32; 3] {
        self.fog_color
    }

    /// Directional (sun/moon) light strength multiplier.
    pub fn sun_intensity(&self) -> f32 {
        self.sun_intensity
    }

    /// Compute base colors from time of day (clear sky, no weather).
    fn time_of_day_colors(hour: f32) -> ([f32; 3], [f32; 3], [f32; 3], f32) {
        if hour < 5.0 {
            // Deep night
            (
                [0.02, 0.02, 0.06],  // sky: very dark blue
                [0.03, 0.03, 0.05],  // ambient: faint
                [0.02, 0.02, 0.05],  // fog: dark
                0.05,                 // sun: moonlight only
            )
        } else if hour < 7.0 {
            // Dawn (5-7): orange/pink gradient
            let t = (hour - 5.0) / 2.0;
            let t_smooth = t * t * (3.0 - 2.0 * t);
            (
                [
                    lerp(0.02, 0.9, t_smooth),   // dark -> warm orange-pink
                    lerp(0.02, 0.5, t_smooth),
                    lerp(0.06, 0.4, t_smooth),
                ],
                [
                    lerp(0.03, 0.25, t_smooth),
                    lerp(0.03, 0.2, t_smooth),
                    lerp(0.05, 0.15, t_smooth),
                ],
                [
                    lerp(0.02, 0.8, t_smooth),
                    lerp(0.02, 0.5, t_smooth),
                    lerp(0.05, 0.4, t_smooth),
                ],
                lerp(0.05, 0.7, t_smooth),
            )
        } else if hour < 17.0 {
            // Full day (7-17): blue sky, white sun
            let t = if hour < 9.0 {
                (hour - 7.0) / 2.0 // morning ramp-up
            } else if hour > 15.0 {
                (17.0 - hour) / 2.0 // afternoon ramp-down
            } else {
                1.0 // midday plateau
            };
            let t = t.clamp(0.0, 1.0);
            (
                [
                    lerp(0.9, 0.4, t),   // dawn warm -> day blue
                    lerp(0.5, 0.6, t),
                    lerp(0.4, 1.0, t),
                ],
                [
                    lerp(0.25, 0.35, t),
                    lerp(0.2, 0.35, t),
                    lerp(0.15, 0.4, t),
                ],
                [
                    lerp(0.8, 0.7, t),
                    lerp(0.5, 0.75, t),
                    lerp(0.4, 0.85, t),
                ],
                lerp(0.7, 1.0, t),
            )
        } else if hour < 19.0 {
            // Dusk (17-19): red/purple gradient
            let t = (hour - 17.0) / 2.0;
            let t_smooth = t * t * (3.0 - 2.0 * t);
            (
                [
                    lerp(0.4, 0.6, t_smooth),    // blue -> red/purple
                    lerp(0.6, 0.15, t_smooth),
                    lerp(1.0, 0.3, t_smooth),
                ],
                [
                    lerp(0.35, 0.1, t_smooth),
                    lerp(0.35, 0.08, t_smooth),
                    lerp(0.4, 0.1, t_smooth),
                ],
                [
                    lerp(0.7, 0.5, t_smooth),
                    lerp(0.75, 0.2, t_smooth),
                    lerp(0.85, 0.25, t_smooth),
                ],
                lerp(1.0, 0.1, t_smooth),
            )
        } else {
            // Night (19-5 wrapping): dark blue/black, stars visible
            let t = ((hour - 19.0) / 4.0).clamp(0.0, 1.0); // fade to full dark by 23
            (
                [
                    lerp(0.6, 0.02, t),
                    lerp(0.15, 0.02, t),
                    lerp(0.3, 0.06, t),
                ],
                [
                    lerp(0.1, 0.03, t),
                    lerp(0.08, 0.03, t),
                    lerp(0.1, 0.05, t),
                ],
                [
                    lerp(0.5, 0.02, t),
                    lerp(0.2, 0.02, t),
                    lerp(0.25, 0.05, t),
                ],
                lerp(0.1, 0.05, t),
            )
        }
    }

    /// Modify base sky parameters based on weather condition and intensity.
    fn apply_weather(
        base_sky: [f32; 3],
        base_ambient: [f32; 3],
        base_fog: [f32; 3],
        base_sun: f32,
        weather: &Weather,
    ) -> ([f32; 3], [f32; 3], [f32; 3], f32) {
        let i = weather.intensity;

        match weather.condition {
            WeatherCondition::Clear => {
                // No modification
                (base_sky, base_ambient, base_fog, base_sun)
            }
            WeatherCondition::Cloudy => {
                // Desaturate sky, slightly reduce sun
                let grey = [0.55, 0.55, 0.58];
                (
                    lerp_color(base_sky, grey, i * 0.6),
                    lerp_color(base_ambient, [0.25, 0.25, 0.27], i * 0.3),
                    lerp_color(base_fog, grey, i * 0.5),
                    base_sun * (1.0 - i * 0.3),
                )
            }
            WeatherCondition::Rain => {
                // Darken everything, grey sky
                let rain_sky = [0.3, 0.3, 0.35];
                let rain_fog = [0.4, 0.4, 0.45];
                (
                    lerp_color(base_sky, rain_sky, i * 0.8),
                    lerp_color(base_ambient, [0.15, 0.15, 0.18], i * 0.5),
                    lerp_color(base_fog, rain_fog, i * 0.7),
                    base_sun * (1.0 - i * 0.5),
                )
            }
            WeatherCondition::Storm => {
                // Heavy darkening, very grey
                let storm_sky = [0.15, 0.15, 0.18];
                let storm_fog = [0.2, 0.2, 0.22];
                (
                    lerp_color(base_sky, storm_sky, i * 0.9),
                    lerp_color(base_ambient, [0.08, 0.08, 0.1], i * 0.7),
                    lerp_color(base_fog, storm_fog, i * 0.85),
                    base_sun * (1.0 - i * 0.8),
                )
            }
            WeatherCondition::Snow => {
                // Brighten ambient (snow reflects light), whiten sky
                let snow_sky = [0.6, 0.6, 0.65];
                (
                    lerp_color(base_sky, snow_sky, i * 0.7),
                    // Snow brightens ambient due to ground reflection
                    [
                        base_ambient[0] + i * 0.15,
                        base_ambient[1] + i * 0.15,
                        base_ambient[2] + i * 0.18,
                    ],
                    lerp_color(base_fog, [0.65, 0.65, 0.7], i * 0.6),
                    base_sun * (1.0 - i * 0.35),
                )
            }
            WeatherCondition::Fog => {
                // Blend everything toward uniform fog color
                let fog = [0.6, 0.6, 0.6];
                (
                    lerp_color(base_sky, fog, i * 0.85),
                    lerp_color(base_ambient, [0.3, 0.3, 0.3], i * 0.4),
                    lerp_color(base_fog, fog, i * 0.9),
                    base_sun * (1.0 - i * 0.6),
                )
            }
            WeatherCondition::Sandstorm => {
                // Orange/brown tint
                let sand = [0.6, 0.45, 0.2];
                let sand_fog = [0.55, 0.4, 0.18];
                (
                    lerp_color(base_sky, sand, i * 0.85),
                    lerp_color(base_ambient, [0.3, 0.2, 0.1], i * 0.5),
                    lerp_color(base_fog, sand_fog, i * 0.8),
                    base_sun * (1.0 - i * 0.6),
                )
            }
        }
    }
}

/// Linear interpolation for a single value.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation between two RGB colors.
fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::weather::Weather;

    #[test]
    fn test_sky_renderer_new() {
        let sky = SkyRenderer::new();
        assert!(sky.sun_intensity() > 0.0);
    }

    #[test]
    fn test_midday_clear() {
        let mut sky = SkyRenderer::new();
        let weather = Weather::default();
        sky.update(12.0, &weather);
        // Midday should have high sun intensity
        assert!(sky.sun_intensity() > 0.8);
        // Sky should be blueish (b > r)
        assert!(sky.sky_color()[2] > sky.sky_color()[0]);
    }

    #[test]
    fn test_midnight_dark() {
        let mut sky = SkyRenderer::new();
        let weather = Weather::default();
        sky.update(0.0, &weather);
        // Night should be very dark
        assert!(sky.sun_intensity() < 0.15);
        assert!(sky.sky_color()[0] < 0.1);
    }

    #[test]
    fn test_storm_darkens() {
        let mut sky = SkyRenderer::new();
        let clear_weather = Weather::default();
        sky.update(12.0, &clear_weather);
        let clear_sun = sky.sun_intensity();

        let storm_weather = Weather {
            condition: WeatherCondition::Storm,
            intensity: 0.9,
            ..Weather::default()
        };
        sky.update(12.0, &storm_weather);
        assert!(sky.sun_intensity() < clear_sun);
    }

    #[test]
    fn test_dawn_colors() {
        let mut sky = SkyRenderer::new();
        let weather = Weather::default();
        sky.update(6.0, &weather);
        // Dawn should have warm/orange tones (r > b)
        assert!(sky.sky_color()[0] > sky.sky_color()[2]);
    }

    #[test]
    fn test_lerp_color() {
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 1.0, 1.0];
        let result = lerp_color(a, b, 0.5);
        assert!((result[0] - 0.5).abs() < f32::EPSILON);
        assert!((result[1] - 0.5).abs() < f32::EPSILON);
        assert!((result[2] - 0.5).abs() < f32::EPSILON);
    }
}
