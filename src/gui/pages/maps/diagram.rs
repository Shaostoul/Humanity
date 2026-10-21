//! The Maps page's DIAGRAM mode (Maps increment 2, decision 5 of
//! docs/design/maps-zoom-to-anything.md, accepted 2026-10-20): the simplified
//! mode of the dual-mode rule, the textbook orrery. True scale stays the
//! default; Diagram spaces the orbits by the LOGARITHM of their distance and
//! enlarges every body, and the page says plainly that it is not to scale.
//!
//! Why a logarithm: at true scale Neptune is 78 times as far out as Mercury, so
//! a view that shows Neptune's orbit draws the four inner planets inside a
//! few pixels of the Sun. `r' = K ln(1 + r / R_S)` keeps the order of every
//! orbit, keeps 1 AU at 1 AU (K is chosen so), and compresses the far system:
//! Mercury 0.58, Mars 1.22, the main belt 1.4 to 1.7, Jupiter 1.93, Neptune
//! 2.98, the Kuiper belt about 3.3 AU on the diagram.
//!
//! Everything that moves round a planet (moons, the ship, the reference
//! rings, the Earth-Moon Lagrange points) is placed from the planet's diagram
//! position by `near_offset`: its direction kept, its distance spaced the
//! same logarithmic way from the planet's ENLARGED surface, so a moon is
//! always outside its planet's disc and in the right order.
//!
//! Pure functions of positions, no state: `MapScene` applies them when the
//! page has Diagram on (`camera.rs`), so the camera, the orbits, the bodies
//! and the belts all move together.

use glam::DVec3;

use crate::cosmos::M_PER_AU;

/// The distance, AU, where the diagram's logarithm turns from linear to
/// logarithmic: inside it orbits keep roughly their true spacing.
const R_S_AU: f64 = 0.25;
/// The size of Earth on the diagram, AU (every other body scales from it).
const EARTH_DISPLAY_AU: f64 = 0.012;
/// How fast the size grows with the true radius: 0.45 keeps the Sun (109
/// Earths across) only 8 times Earth's diagram size and a 1 km asteroid a dot.
const SIZE_POWER: f64 = 0.45;
/// How far out round a planet things are spaced: a moon `n` planet radii out
/// stands `1 + SPREAD ln(n)` enlarged radii out.
const SPREAD: f64 = 0.9;

/// The diagram distance from the Sun for a true distance `r_au`.
pub fn radial_au(r_au: f64) -> f64 {
    let k = 1.0 / (1.0 + 1.0 / R_S_AU).ln();
    k * (1.0 + r_au.max(0.0) / R_S_AU).ln()
}

/// A heliocentric position (AU) on the diagram: its direction kept, its
/// distance from the Sun through `radial_au`.
pub fn helio_au(p_au: DVec3) -> DVec3 {
    let r = p_au.length();
    if r <= 0.0 {
        p_au
    } else {
        p_au * (radial_au(r) / r)
    }
}

/// A body's radius on the diagram, metres, from its true radius in km.
pub fn display_radius_m(radius_km: f64) -> f64 {
    EARTH_DISPLAY_AU * (radius_km.max(0.0) / 6371.0).powf(SIZE_POWER) * M_PER_AU
}

/// Something `offset_m` (true metres) from the centre of a planet of radius
/// `parent_radius_km`, on the diagram: its direction kept, its distance spaced
/// logarithmically from the planet's enlarged surface (`SPREAD`), and inside
/// the planet, in proportion.
pub fn near_offset(parent_radius_km: f64, offset_m: DVec3) -> DVec3 {
    let r = offset_m.length();
    let r_true = (parent_radius_km * 1000.0).max(1.0);
    if r <= 0.0 {
        return DVec3::ZERO;
    }
    let rd = display_radius_m(parent_radius_km);
    let d = if r <= r_true { rd * r / r_true } else { rd * (1.0 + SPREAD * (r / r_true).ln()) };
    offset_m * (d / r)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The diagram keeps every orbit in its order, keeps 1 AU at 1 AU, and
    /// fits Neptune within three times Earth's distance, where true scale has
    /// it thirty times as far.
    ///
    /// Seen red 2026-10-21 with the distance left linear (`radial_au(r) = r`):
    /// "Neptune is 30.1 AU out on the diagram, not within 3.2".
    #[test]
    fn the_diagram_keeps_the_order_and_brings_neptune_in() {
        let orbits = [0.387, 0.723, 1.0, 1.524, 2.77, 5.203, 9.537, 19.19, 30.07, 39.48];
        let d: Vec<f64> = orbits.iter().map(|&r| radial_au(r)).collect();
        assert!(d.windows(2).all(|w| w[0] < w[1]), "the order changed: {d:?}");
        assert!((radial_au(1.0) - 1.0).abs() < 1e-12, "1 AU is not 1 AU on the diagram");
        assert!(radial_au(30.07) < 3.2, "Neptune is {:.1} AU out on the diagram, not within 3.2", radial_au(30.07));
        assert!(radial_au(0.387) > 0.5, "Mercury is crowded into the Sun");
    }

    /// Round a planet, everything lands outside its enlarged disc and in its
    /// true order: the ship at 42,241 km, then the Moon at 384,400 km.
    ///
    /// Seen red 2026-10-21 with the true offsets kept: "the Moon is inside
    /// Earth's enlarged disc (0.003 AU, the disc is 0.012 AU)".
    #[test]
    fn moons_stand_outside_the_enlarged_planet_in_order() {
        let earth_km = 6371.0;
        let disc = display_radius_m(earth_km);
        let ship = near_offset(earth_km, DVec3::new(42_241_000.0, 0.0, 0.0)).length();
        let moon = near_offset(earth_km, DVec3::new(0.0, 0.0, 384_400_000.0)).length();
        assert!(moon > disc, "the Moon is inside Earth's enlarged disc ({:.3} AU, the disc is {:.3} AU)", moon / M_PER_AU, disc / M_PER_AU);
        assert!(ship > disc && ship < moon, "the ship ({ship:.0}) is not between the disc ({disc:.0}) and the Moon ({moon:.0})");
        // Direction is kept.
        let d = near_offset(earth_km, DVec3::new(3.0e7, 4.0e7, 0.0)).normalize();
        assert!((d - DVec3::new(0.6, 0.8, 0.0)).length() < 1e-12);
    }
}
