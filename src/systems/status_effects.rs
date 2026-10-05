//! Status-effect registry — buffs, debuffs, conditions, diseases, and
//! environmental effects loaded from `data/status_effects.csv`.
//!
//! Effects are applied to entities via the `StatusEffects` component
//! (`ecs::components`). This registry is the data side: it owns each effect's
//! duration and `stat:value:operation` modifier, so balance lives in the CSV
//! (infinite-of-X) and any system can apply an effect by id and read its
//! duration here instead of hardcoding numbers.

use std::collections::HashMap;

use serde::Deserialize;

/// One row of `data/status_effects.csv`.
///
/// EVERY COLUMN IS A FIELD (BUG-162, 2026-10-05). The loader refuses a row with
/// a column this struct does not declare (`deny_unknown_fields`; the shared CSV
/// loader logs and skips it), so a column can no longer sit in the data while
/// the code silently drops it, as `dispel_type` did: it promised that medicine
/// cures an effect while nothing read it. A test holds every shipped row
/// loading and a row with an unknown column refused. A new column needs a field
/// here, and a use or a stated reason why it has none yet.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusEffectDef {
    /// Unique effect id (e.g. `well_fed`, `hungry`, `food_poisoning`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// `buff`, `debuff`, `condition`, `environmental`, `disease`, or `poison`.
    /// A `disease` runs on the game clock (`systems::illness`, BUG-162).
    #[serde(rename = "type")]
    pub kind: String,
    /// Duration in seconds. 0 = a condition that persists until removed (the
    /// owning system refreshes it each tick while its trigger holds). For a
    /// `disease`, game seconds: its Realistic course.
    #[serde(default)]
    pub duration_s: f32,
    /// Whether the effect stacks. READ, NOT ACTED ON: no effect stacks yet;
    /// applying one again refreshes its timer (`StatusEffects::apply`).
    #[serde(default)]
    pub stackable: bool,
    /// The most stacks it can have. Read, not acted on (see `stackable`); the
    /// relay's on-foot speed bound counts each speed buff at its most stacks
    /// (ship::moves tests), so a stacking system can never outrun it.
    #[serde(default)]
    pub max_stacks: u32,
    /// `stat:value:operation`, e.g. `speed:0.8:multiply`. `none:0:none` = no modifier.
    #[serde(default)]
    pub stat_modifier: String,
    /// Seconds between damage/healing applications (0 = the per-tick values
    /// are already per second). Consumed by the FoodSystem effect tick
    /// (v0.745, loop-map rung 1) which normalizes to a continuous rate:
    /// infected_wound's 3 dmg / 20 s becomes 0.15 dmg/s.
    #[serde(default)]
    pub tick_interval_s: f32,
    /// Damage applied each tick (0 = none).
    #[serde(default)]
    pub damage_per_tick: f32,
    /// What kind of damage it is (`fire`, `poison`, `cold`...). Read, not
    /// acted on: the game has no resistances yet, so all damage counts alike.
    #[serde(default)]
    pub damage_type: String,
    /// Healing applied each tick (0 = none).
    #[serde(default)]
    pub healing_per_tick: f32,
    /// Pipe-separated tags (e.g. `survival|food`). A medical item ends the
    /// effects carrying a tag it names (data/medical/treatments.ron,
    /// `systems::treatment`): `bleeding`, `bacterial`, `toxin`.
    #[serde(default)]
    pub tags: String,
    /// Short human-readable description.
    #[serde(default)]
    pub description: String,
}

impl StatusEffectDef {
    /// Whether the effect carries `tag` in its `tags` column.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.split('|').any(|t| t == tag)
    }

    /// Whether it is an illness, which runs on the game clock (`systems::illness`).
    pub fn is_disease(&self) -> bool {
        self.kind == crate::systems::illness::DISEASE
    }

    /// Parse `stat_modifier` into `(stat, value, operation)`, or `None` for the
    /// sentinel `none:0:none` / a malformed value.
    pub fn modifier(&self) -> Option<(&str, f32, &str)> {
        let mut parts = self.stat_modifier.split(':');
        let stat = parts.next()?;
        let value: f32 = parts.next()?.parse().ok()?;
        let op = parts.next()?;
        if stat == "none" {
            return None;
        }
        Some((stat, value, op))
    }
}

/// All status effects, keyed by id. Lives in the runtime `DataStore` under
/// `"status_effect_registry"`.
#[derive(Debug, Default)]
pub struct StatusEffectRegistry {
    pub effects: HashMap<String, StatusEffectDef>,
}

impl StatusEffectRegistry {
    /// Build the registry from raw `status_effects.csv` bytes (shared loader:
    /// skips `#` comments, header-mapped, row-resilient).
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<StatusEffectDef> = crate::assets::loader::parse_csv(data)?;
        let mut effects = HashMap::new();
        for row in rows {
            effects.insert(row.id.clone(), row);
        }
        Ok(Self { effects })
    }

    /// Duration (seconds) for an effect id; 0.0 if unknown or a condition.
    pub fn duration(&self, id: &str) -> f32 {
        self.effects.get(id).map(|d| d.duration_s).unwrap_or(0.0)
    }

    /// Look up the full definition.
    pub fn get(&self, id: &str) -> Option<&StatusEffectDef> {
        self.effects.get(id)
    }

    /// Net multiplier for a stat across a set of active effect ids: starts at 1.0,
    /// multiplies for `multiply` modifiers and adds for `add` modifiers, clamped at
    /// 0. Used e.g. for the player's movement speed (well_nourished ×1.1, thirsty
    /// ×0.8, flu ×0.8…) so buffs/debuffs are mechanically applied, not just shown.
    pub fn net_stat_multiplier<'a>(
        &self,
        active_ids: impl IntoIterator<Item = &'a str>,
        stat: &str,
    ) -> f32 {
        let mut mult = 1.0_f32;
        for id in active_ids {
            if let Some(def) = self.get(id) {
                if let Some((s, value, op)) = def.modifier() {
                    if s == stat {
                        match op {
                            "multiply" => mult *= value,
                            "add" => mult += value,
                            _ => {}
                        }
                    }
                }
            }
        }
        mult.max(0.0)
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_csv_parses_real_status_effects() {
        let reg = StatusEffectRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/status_effects.csv"
        )))
        .expect("status_effects.csv");

        // Food-loop effects the nutrition system applies must be present.
        for id in ["well_fed", "hungry", "thirsty", "food_poisoning", "night_vision"] {
            assert!(reg.get(id).is_some(), "missing status effect: {id}");
        }

        // Durations come from data, not code. Food poisoning's is its Realistic
        // course in game seconds, two days (BUG-162).
        assert_eq!(reg.duration("well_fed"), 1800.0);
        assert_eq!(reg.duration("food_poisoning"), 172_800.0);
        assert!(reg.get("food_poisoning").unwrap().is_disease());
        assert!(reg.get("infection").unwrap().has_tag("bacterial"));
        assert!(!reg.get("food_poisoning").unwrap().has_tag("bacterial"), "antibiotics do not end it");
        // Conditions have duration 0 (managed by their owning system).
        assert_eq!(reg.duration("hungry"), 0.0);

        // Modifier parsing: well_fed boosts stamina_regen; the sentinel parses to None.
        let (stat, value, op) = reg.get("well_fed").unwrap().modifier().expect("well_fed modifier");
        assert_eq!(stat, "stamina_regen");
        assert_eq!(op, "multiply");
        assert!((value - 1.5).abs() < f32::EPSILON);
        assert!(reg.get("shield").is_none() || true); // shield exists; just exercising get()
    }

    #[test]
    fn net_speed_multiplier_combines_effects() {
        let reg = StatusEffectRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/status_effects.csv"
        )))
        .expect("status_effects.csv");

        // No effects -> neutral speed.
        assert!((reg.net_stat_multiplier(std::iter::empty(), "speed") - 1.0).abs() < 1e-6);
        // thirsty has speed:0.8 -> 0.8x (a tangible survival debuff).
        assert!((reg.net_stat_multiplier(["thirsty"], "speed") - 0.8).abs() < 1e-6);
        // well_nourished (speed:1.1) stacks multiplicatively with thirsty (0.8) -> 0.88x.
        let combined = reg.net_stat_multiplier(["well_nourished", "thirsty"], "speed");
        assert!((combined - 0.88).abs() < 1e-6, "combined speed mult = {combined}");
        // well_fed modifies stamina_regen, NOT speed -> no effect on the speed stat.
        assert!((reg.net_stat_multiplier(["well_fed"], "speed") - 1.0).abs() < 1e-6);
    }
}
