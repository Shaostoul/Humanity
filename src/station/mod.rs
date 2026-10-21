//! Crewed stations: the player's homestead as a real orbiting, pointed object.
//!
//! See [`orbit`] for the mechanics and for why attitude, not position, is what
//! makes the homestead's lighting follow the clock.
//!
//! Infinite-of-X: a station is a data file, never a constant in code. Before
//! this module the home's orbit was four hardcoded numbers inside the frame loop
//! in `lib.rs`, which meant there could only ever be exactly one station and its
//! orbit could not be changed without a recompile.

pub mod orbit;
pub mod station_keeping;

pub use orbit::{AttitudeMode, OrbitDef, PeriodSpec, StationDef};

use std::path::Path;

/// The rotation that takes a WORLD direction into the render frame.
///
/// While riding the station the render frame IS the hull's body frame, so the
/// world has to be counter-rotated into it; off the station it is world-aligned
/// and this is the identity. Rotating the WORLD rather than the hull is what
/// keeps this change small: home-local coordinates, colliders and the whole
/// `station_off` translation path are untouched, and every external thing (sun,
/// Earth, planets) passes through one transform.
///
/// Every consumer must use this, not its own inverse: a site that forgets it
/// leaves the sun sweeping the deck while Earth sits frozen in the window.
pub fn hull_frame_rot(riding: bool, station_rot: glam::DQuat) -> glam::DQuat {
    if riding {
        station_rot.inverse()
    } else {
        glam::DQuat::IDENTITY
    }
}

/// The rotation taking a RENDER-frame direction back out into world axes -
/// the inverse of [`hull_frame_rot`].
///
/// Needed by anything drawn in WORLD/celestial coordinates that is handed a
/// render-frame direction. The star field is the case that motivated it: its
/// sky rotation is built from the camera's forward/up, which while riding are
/// hull-frame vectors, so without this the whole sky is pinned to the deck and
/// turns with the station instead of standing still.
pub fn render_to_world_rot(riding: bool, station_rot: glam::DQuat) -> glam::DQuat {
    if riding {
        station_rot
    } else {
        glam::DQuat::IDENTITY
    }
}

/// Convenience: apply [`hull_frame_rot`] to a direction or offset.
pub fn to_hull(riding: bool, station_rot: glam::DQuat, v: glam::DVec3) -> glam::DVec3 {
    if riding {
        station_rot.inverse() * v
    } else {
        v
    }
}

/// The camera yaw at which true north lies, for the HUD compass, or None when
/// no north lies on the camera's horizon (2026-10-21).
///
/// North is Earth's spin axis (world +Y; the engine has no axial tilt) laid
/// flat on the plane the player stands in, which is what north means on the
/// ground. On a planet's surface the camera's yaw is already measured from that
/// north (`surface_walk::tangent_basis`), so the answer is 0. Aboard, the
/// render frame is the hull's ([`hull_frame_rot`]) and the camera's yaw turns
/// about hull +Y from hull -Z, which under the LVLH attitude is WEST along the
/// orbit: until 2026-10-21 the compass drew N there and printed E over the real
/// north (hull +X, the orbit's normal for the equatorial home). Free in space
/// the camera's up is the pole itself, so there is no north on its horizon.
pub fn compass_north_yaw(surface_mode: bool, riding: bool, station_rot: glam::DQuat) -> Option<f32> {
    if surface_mode {
        return Some(0.0);
    }
    if !riding {
        return None;
    }
    let n = hull_frame_rot(true, station_rot) * glam::DVec3::Y;
    // Within about 6 degrees of the deck's up (a polar orbit over a pole) the
    // flattened axis is too short to name a direction.
    if n.x * n.x + n.z * n.z < 0.01 {
        return None;
    }
    // The camera's world-basis yaw convention: forward = (sin yaw, 0, -cos yaw).
    Some(n.x.atan2(-n.z) as f32)
}

/// Load every `data/stations/*.ron`.
///
/// A missing or unreadable directory is not fatal: the caller falls back to the
/// built-in default so a broken data edit cannot make the world unenterable.
pub fn load_all(data_dir: &Path) -> Vec<StationDef> {
    let dir = data_dir.join("stations");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        log::warn!("stations: no {} directory, using the default", dir.display());
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("ron") {
            continue;
        }
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| {
            ron::from_str::<StationDef>(&t).map_err(|e| e.to_string())
        }) {
            Ok(def) => {
                log::info!("stations: loaded {} ({})", def.id, p.display());
                out.push(def);
            }
            Err(err) => log::warn!("stations: {} failed to parse: {err}", p.display()),
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The home station, either from data or the built-in fallback.
pub fn home(defs: &[StationDef]) -> StationDef {
    defs.iter()
        .find(|d| d.id == "home")
        .cloned()
        .unwrap_or_else(default_home)
}

/// Built-in fallback so a missing data file degrades to a sensible station
/// rather than to no station.
pub fn default_home() -> StationDef {
    StationDef {
        id: "home".to_string(),
        name: "UHS Mothership".to_string(),
        parent: "earth".to_string(),
        orbit: OrbitDef {
            period: PeriodSpec::Synchronous,
            eccentricity: 0.0,
            inclination_deg: 0.0,
            raan_deg: 0.0,
            arg_periapsis_deg: 0.0,
            mean_anomaly_at_epoch_deg: -122.3,
            epoch_game_seconds: 0.0,
        },
        attitude: AttitudeMode::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped data file must parse, and must be the synchronous orbit the
    /// engine assumes when it says the home hangs over one spot on Earth. A
    /// silent parse failure would fall back to the default and look identical,
    /// so this asserts the FILE parses rather than that some station exists.
    /// Since Maps increment 1 it also holds the ship to its one name; seen
    /// red 2026-10-21 with the file's old name: left "Homestead", right "UHS
    /// Mothership".
    #[test]
    fn shipped_home_station_parses_and_is_synchronous() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("stations")
            .join("home.ron");
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("data/stations/home.ron must ship: {e}"));
        let def: StationDef = ron::from_str(&text)
            .unwrap_or_else(|e| panic!("data/stations/home.ron must parse: {e}"));
        assert_eq!(def.id, "home");
        assert_eq!(def.name, "UHS Mothership", "the ship has one name everywhere (data/fleet.json's)");
        assert_eq!(def.name, default_home().name, "the fallback carries the same name");
        assert_eq!(def.parent, "earth");
        assert_eq!(def.orbit.period, PeriodSpec::Synchronous);
        assert!(
            matches!(def.attitude, AttitudeMode::Lvlh { .. }),
            "the home must fly nadir-pointing, or its lighting stops tracking the clock"
        );
    }

    /// ABOARD, THE COMPASS POINTS TO EARTH'S NORTH (2026-10-21, BUG-212's
    /// list). The camera's yaw aboard is in the hull's frame, so a compass that
    /// drew N at yaw 0 drew it at hull -Z, west along the orbit under LVLH, and
    /// printed E over the real north. Driven through the engine's chain: the
    /// shipped home's orbit re-phased by the sun's azimuth, its LVLH attitude,
    /// then the look at the compass's north turned back out of the hull frame
    /// must be the world's north (+Y, Earth's axis) at every date and hour. On
    /// a planet's ground the compass is unchanged (yaw 0 is north there), and
    /// free in space there is no north on the horizon to draw.
    ///
    /// Seen red 2026-10-21 with the function answering 0 aboard (the old
    /// compass): "aboard at sun azimuth 0, hour 0: the compass N points 90.0
    /// degrees from Earth's north".
    #[test]
    fn aboard_the_compass_points_to_earths_north_not_along_the_orbit() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("stations").join("home.ron");
        let def: StationDef = ron::from_str(&std::fs::read_to_string(&p).expect("home.ron")).expect("home.ron parses");
        let day = crate::systems::time::EARTH_DAY_S;
        for sun_az_deg in [0.0_f64, 90.0, 200.0] {
            let o = orbit::over_its_longitude(&def.orbit, sun_az_deg.to_radians());
            for hour in [0.0_f64, 6.0, 13.5] {
                let t = orbit::sim_seconds(hour * 3600.0, day);
                let (pos, vel) = orbit::propagate(&o, orbit::MU_EARTH, orbit::REAL_SECONDS_PER_DAY, t, day);
                let rot = orbit::attitude(&def.attitude, pos, vel);
                let yaw = compass_north_yaw(false, true, rot).expect("aboard an equatorial home there is a north");
                let look = render_to_world_rot(true, rot) * crate::dev_travel::look_dir(yaw, 0.0);
                let off = look.dot(glam::DVec3::Y).clamp(-1.0, 1.0).acos().to_degrees();
                assert!(off < 0.1, "aboard at sun azimuth {sun_az_deg}, hour {hour}: the compass N points {off:.1} degrees from Earth's north");
            }
        }
        assert_eq!(compass_north_yaw(true, false, glam::DQuat::IDENTITY), Some(0.0), "on the ground north is yaw 0, as it always was");
        assert_eq!(compass_north_yaw(false, false, glam::DQuat::IDENTITY), None, "free in space there is no north on the horizon");
    }
}
