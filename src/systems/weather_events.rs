//! Extreme-weather EVENT registry (v0.1034, weather-water roadmap
//! section 5): tornadoes, blizzards, meteor showers - and whatever a
//! modder invents - as DATA entries in `data/weather/events.ron`, per
//! the infinite-of-x rule. This increment is the SCHEMA + loader +
//! validation only; WeatherSystem consumption (trigger rolls, wind
//! application, hazard damage) is the next rung, so nothing here
//! affects gameplay yet.
//!
//! Posture mirrors the other registries: parse failures degrade to an
//! empty registry with a log warning, and the shipped-data tests are
//! the real gate against a bad edit reaching players silently.

use serde::Deserialize;

/// Inclusive numeric range (RON: `(min: a, max: b)`).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Range {
    pub min: f32,
    pub max: f32,
}

/// How an event shapes the wind while active.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum WindProfile {
    /// No wind change (e.g. a meteor shower).
    None,
    /// A gust front: stronger wind with direction wobble.
    Front { gust_mps: f32, direction_jitter_deg: f32 },
    /// A rotating core (tornado / future hurricane eye-wall).
    Vortex { core_radius_m: f32, peak_mps: f32 },
}

/// Cloud-deck overrides while the event runs.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CloudOverride {
    pub coverage_boost: f32,
    pub tint: (f32, f32, f32),
}

/// Hazard placeholder: validated now, dealt by the future hazard rung.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Hazard {
    pub damage_per_s: f32,
    pub radius_m: f32,
}

/// One extreme-weather event definition.
#[derive(Debug, Clone, Deserialize)]
pub struct WeatherEvent {
    pub id: String,
    pub name: String,
    /// Season names matching `systems::time::Season` variants.
    pub seasons: Vec<String>,
    pub temp_c: Range,
    pub wind_mps: Range,
    /// Selection weight among simultaneously-eligible events (> 0).
    pub rarity_weight: f32,
    pub duration_s: Range,
    pub wind_profile: WindProfile,
    /// Particle emitter ids from data/particles.ron.
    pub emitters: Vec<String>,
    pub cloud: CloudOverride,
    pub hazard: Hazard,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeatherEventRegistry {
    pub events: Vec<WeatherEvent>,
}

const SEASON_NAMES: [&str; 4] = ["Spring", "Summer", "Autumn", "Winter"];

impl WeatherEventRegistry {
    /// Parse + structurally validate. Errors name the offending entry so
    /// a data edit fails loud in tests instead of half-loading.
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| format!("events.ron not UTF-8: {e}"))?;
        let reg: WeatherEventRegistry =
            ron::from_str(text).map_err(|e| format!("events.ron parse error: {e}"))?;
        reg.validate()?;
        Ok(reg)
    }

    fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for e in &self.events {
            let id = &e.id;
            if id.is_empty() || e.name.is_empty() {
                return Err(format!("event '{id}': id/name must be non-empty"));
            }
            if !seen.insert(id.clone()) {
                return Err(format!("duplicate event id '{id}'"));
            }
            for r in [("temp_c", e.temp_c), ("wind_mps", e.wind_mps), ("duration_s", e.duration_s)]
            {
                if r.1.min > r.1.max {
                    return Err(format!("event '{id}': {} min > max", r.0));
                }
            }
            if e.rarity_weight <= 0.0 {
                return Err(format!("event '{id}': rarity_weight must be > 0"));
            }
            if e.seasons.is_empty() {
                return Err(format!("event '{id}': needs at least one season"));
            }
            for s in &e.seasons {
                if !SEASON_NAMES.contains(&s.as_str()) {
                    return Err(format!(
                        "event '{id}': unknown season '{s}' (expected one of {SEASON_NAMES:?})"
                    ));
                }
            }
            if e.emitters.is_empty() {
                return Err(format!("event '{id}': needs at least one particle emitter"));
            }
            if e.hazard.damage_per_s > 0.0 && e.hazard.radius_m <= 0.0 {
                return Err(format!("event '{id}': damaging hazard needs a positive radius"));
            }
        }
        Ok(())
    }

    /// Cross-file check: every referenced emitter id must exist in the
    /// particle system's defs. Called by the shipped-data test (and by
    /// the future WeatherSystem rung at startup).
    pub fn validate_emitters(
        &self,
        known: &std::collections::HashSet<String>,
    ) -> Result<(), String> {
        for e in &self.events {
            for em in &e.emitters {
                if !known.contains(em) {
                    return Err(format!(
                        "event '{}' references unknown particle emitter '{em}'",
                        e.id
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Events whose trigger window contains the current conditions (v0.1035,
/// the consumption rung). `season` is the `Season` enum's Debug name.
pub fn eligible<'a>(
    reg: &'a WeatherEventRegistry,
    season: &str,
    temp_c: f32,
    wind_mps: f32,
) -> Vec<&'a WeatherEvent> {
    reg.events
        .iter()
        .filter(|e| {
            e.seasons.iter().any(|s| s == season)
                && temp_c >= e.temp_c.min
                && temp_c <= e.temp_c.max
                && wind_mps >= e.wind_mps.min
                && wind_mps <= e.wind_mps.max
        })
        .collect()
}

/// Place a Vortex event's core on the planet surface (v0.1038): from
/// the player's PLANET-FRAME position, walk `dist_m` along the surface
/// at `bearing_rad`, then re-project onto the player's radius so the
/// core sits on the sphere (a tornado core is a ground feature). Pure
/// and deterministic - the caller supplies the bearing roll.
pub fn place_event_core(anchor: glam::DVec3, bearing_rad: f64, dist_m: f64) -> glam::DVec3 {
    let r = anchor.length();
    if r < 1.0 {
        return anchor;
    }
    let dir = anchor / r;
    // Tangent basis at the anchor (east-ish / north-ish; exact compass
    // orientation is irrelevant for a random bearing).
    let mut east = glam::DVec3::Y.cross(dir);
    if east.length_squared() < 1.0e-9 {
        east = glam::DVec3::X.cross(dir);
    }
    let east = east.normalize();
    let north = dir.cross(east);
    let offset = (east * bearing_rad.cos() + north * bearing_rad.sin()) * dist_m;
    (anchor + offset).normalize() * r
}

/// Hazard proximity bands: `near` inside 3x the hazard radius (the HUD
/// warning), `inside` within the radius itself (the future damage rung).
pub fn hazard_proximity(dist_m: f64, radius_m: f32) -> (bool, bool) {
    if radius_m <= 0.0 {
        return (false, false);
    }
    (dist_m < radius_m as f64 * 3.0, dist_m < radius_m as f64)
}

/// Deterministic weighted selection: `roll01` in [0,1) maps onto the
/// cumulative rarity weights, so rare events (tornado 0.08) fire far
/// less often than common ones (thunderstorm 1.0). Pure function of its
/// inputs - the caller owns the RNG, which keeps this testable.
pub fn weighted_pick<'a>(events: &[&'a WeatherEvent], roll01: f32) -> Option<&'a WeatherEvent> {
    let total: f32 = events.iter().map(|e| e.rarity_weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = roll01.clamp(0.0, 0.999_99) * total;
    for e in events {
        if x < e.rarity_weight {
            return Some(e);
        }
        x -= e.rarity_weight;
    }
    events.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The shipped events file parses, validates, and every emitter it
    /// references exists in the shipped particles.ron - the cross-file
    /// contract a lone data edit is most likely to break.
    #[test]
    fn shipped_weather_events_parse_and_emitter_refs_resolve() {
        let bytes = std::fs::read(data_dir().join("weather/events.ron"))
            .expect("data/weather/events.ron readable");
        let reg = WeatherEventRegistry::from_ron(&bytes).expect("events.ron valid");
        assert!(reg.len() >= 4, "expected the seed event set, got {}", reg.len());
        // Emitter ids from the shipped particles.ron (top-level map keys).
        let ptext = std::fs::read_to_string(data_dir().join("particles.ron"))
            .expect("particles.ron readable");
        let pmap: std::collections::HashMap<String, ron::Value> =
            ron::from_str(&ptext).expect("particles.ron parses");
        let known: std::collections::HashSet<String> = pmap.keys().cloned().collect();
        reg.validate_emitters(&known).expect("all emitter refs resolve");
        // The seed set exercises every wind-profile variant.
        assert!(reg.events.iter().any(|e| matches!(e.wind_profile, WindProfile::Vortex { .. })));
        assert!(reg.events.iter().any(|e| matches!(e.wind_profile, WindProfile::Front { .. })));
        assert!(reg.events.iter().any(|e| e.wind_profile == WindProfile::None));
    }

    #[test]
    fn eligibility_and_weighted_pick_behave() {
        let bytes = std::fs::read(data_dir().join("weather/events.ron")).unwrap();
        let reg = WeatherEventRegistry::from_ron(&bytes).unwrap();
        // Blizzard needs Winter + hard cold + wind: a mild spring day
        // can't produce it, a windy arctic winter can.
        let spring = eligible(&reg, "Spring", 20.0, 8.0);
        assert!(spring.iter().all(|e| e.id != "blizzard"));
        assert!(spring.iter().any(|e| e.id == "thunderstorm"));
        let arctic = eligible(&reg, "Winter", -15.0, 20.0);
        assert!(arctic.iter().any(|e| e.id == "blizzard"));
        assert!(arctic.iter().all(|e| e.id != "thunderstorm"), "thunderstorm at -15C");
        // Out-of-range wind excludes everything wind-gated.
        assert!(eligible(&reg, "Summer", 25.0, 0.5).iter().all(|e| e.id != "tornado"));
        // Weighted pick: roll 0 lands in the first (cumulative) slot,
        // roll near 1 lands in the last; a rare event occupies a slice
        // proportional to its weight.
        let all: Vec<&WeatherEvent> = reg.events.iter().collect();
        assert!(weighted_pick(&all, 0.0).is_some());
        assert_eq!(
            weighted_pick(&all, 0.999).unwrap().id,
            all.last().unwrap().id,
            "top roll picks the last cumulative slot"
        );
        assert!(weighted_pick(&[], 0.5).is_none());
        // Proportionality: sweep the roll space; the tornado (weight
        // 0.08 of ~1.83 total) should win well under 10% of rolls.
        let hits = (0..1000)
            .filter(|i| weighted_pick(&all, *i as f32 / 1000.0).map(|e| e.id == "tornado") == Some(true))
            .count();
        assert!(hits < 100, "tornado won {hits}/1000 rolls despite 0.08 weight");
    }

    #[test]
    fn core_placement_stays_on_sphere_in_the_distance_band() {
        let anchor = glam::DVec3::new(3_000_000.0, 4_000_000.0, 2_500_000.0);
        let r0 = anchor.length();
        for i in 0..24 {
            let bearing = i as f64 * 0.26;
            let dist = 200.0 + (i as f64 * 17.3) % 400.0;
            let core = place_event_core(anchor, bearing, dist);
            assert!((core.length() - r0).abs() < 0.01, "core left the sphere");
            let d = (core - anchor).length();
            // Chord vs arc at <= 600 m on a planet radius is sub-mm; the
            // re-projection shortens the offset slightly, never grows it.
            assert!(d > dist * 0.98 && d < dist * 1.001, "band miss: {d} vs {dist}");
        }
        // Proximity bands (tornado hazard radius 80 m).
        assert_eq!(hazard_proximity(500.0, 80.0), (false, false));
        assert_eq!(hazard_proximity(180.0, 80.0), (true, false));
        assert_eq!(hazard_proximity(50.0, 80.0), (true, true));
        assert_eq!(hazard_proximity(50.0, 0.0), (false, false), "no hazard, no warning");
    }

    #[test]
    fn validation_rejects_bad_entries() {
        let base = std::fs::read_to_string(data_dir().join("weather/events.ron")).unwrap();
        // Unknown season fails loud.
        let bad = base.replacen("\"Winter\"", "\"Wintre\"", 1);
        assert!(WeatherEventRegistry::from_ron(bad.as_bytes()).is_err(), "typo season accepted");
        // Duplicate id fails loud.
        let dup = base.replacen("\"blizzard\"", "\"thunderstorm\"", 1);
        assert!(WeatherEventRegistry::from_ron(dup.as_bytes()).is_err(), "dup id accepted");
        // Unknown emitter is caught by the cross-file check.
        let reg = WeatherEventRegistry::from_ron(base.as_bytes()).unwrap();
        let empty: std::collections::HashSet<String> = Default::default();
        assert!(reg.validate_emitters(&empty).is_err());
    }
}
