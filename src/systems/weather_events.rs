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
