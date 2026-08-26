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

/// Convenience: apply [`hull_frame_rot`] to a direction or offset.
pub fn to_hull(riding: bool, station_rot: glam::DQuat, v: glam::DVec3) -> glam::DVec3 {
    if riding {
        station_rot.inverse() * v
    } else {
        v
    }
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
        name: "Homestead".to_string(),
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
        assert_eq!(def.parent, "earth");
        assert_eq!(def.orbit.period, PeriodSpec::Synchronous);
        assert!(
            matches!(def.attitude, AttitudeMode::Lvlh { .. }),
            "the home must fly nadir-pointing, or its lighting stops tracking the clock"
        );
    }
}
