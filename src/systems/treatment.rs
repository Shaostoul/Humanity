//! Medical items: what the Inventory's Use button does (BUG-162, 2026-10-05).
//!
//! Until 2026-10-05 the Use button discarded its click, so a Bandage, a Medkit or Antibiotics
//! did nothing. Now each medical item's effect is DATA, a row of data/medical/treatments.ron:
//! the Health it restores, the status effects it ends, and what the player is told. An effect
//! is ended by a TAG on it in data/status_effects.csv, so Antibiotics end only effects tagged
//! `bacterial`, and Food Poisoning is not one. An item that would do nothing for the player
//! (nothing hurt, nothing it ends, or a use the game does not model yet) is kept, not used up,
//! and its row says why in plain words; the player is also told what does help with any
//! illness they have (`systems::illness`).
//!
//! THE PATH. The inventory card shows Use for exactly the items with a row (`has_use`); a
//! click goes through lib.rs's inventory bridge onto the `USE_REQUEST_KEY` channel
//! (`request_use`), and the food system takes it (`take_request`) and applies it to the
//! player with `apply`, removing one of the item when it was used.
//!
//! Pure data and logic with no engine state, so it compiles in every feature set.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use serde::Deserialize;

use crate::ecs::components::{Health, StatusEffects};
use crate::hot_reload::data_store::DataStore;
use crate::systems::status_effects::StatusEffectRegistry;

/// The DataStore channel a Use click rides from the inventory to the food system.
pub const USE_REQUEST_KEY: &str = "use_item_request";

/// One row of data/medical/treatments.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct Treatment {
    /// The items.csv id.
    pub item: String,
    /// Health it restores, up to full.
    #[serde(default)]
    pub heals: f32,
    /// Tags of the status effects it ends (data/status_effects.csv `tags`).
    #[serde(default)]
    pub ends: Vec<String>,
    /// What the player is told when it is used.
    #[serde(default)]
    pub used: String,
    /// What the player is told when it would do nothing; it is then kept.
    #[serde(default)]
    pub no_help: String,
}

/// data/medical/treatments.ron.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Treatments {
    pub treatments: Vec<Treatment>,
}

impl Treatments {
    /// Path of the file, relative to the data directory.
    pub const FILE: &'static str = "medical/treatments.ron";

    /// Disk first (modding), the copy built into the game when the disk copy is missing or
    /// this version cannot read it (BUG-163, `embedded_data::load_text_or_embedded`). With
    /// neither there are no treatments, so no item shows a Use button: loud in the log, never
    /// a crash.
    pub fn load(data_dir: &Path) -> Self {
        crate::embedded_data::load_text_or_embedded(data_dir, Self::FILE, Self::from_ron).unwrap_or_else(|e| {
            log::warn!("{e}; no medical item can be used until it is fixed");
            Self::default()
        })
    }

    /// Parse the file's text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The row for an item, if it has a Use.
    pub fn get(&self, item_id: &str) -> Option<&Treatment> {
        self.treatments.iter().find(|t| t.item == item_id)
    }
}

/// Whether an item has a Use (a row in treatments.ron), from the same file the food system
/// reads, disk first. The inventory card asks, and shows Use only when it does.
pub fn has_use(item_id: &str) -> bool {
    static ITEMS: std::sync::OnceLock<HashSet<String>> = std::sync::OnceLock::new();
    ITEMS
        .get_or_init(|| Treatments::load(&crate::data_dir()).treatments.into_iter().map(|t| t.item).collect())
        .contains(item_id)
}

/// Put a Use click on the channel the food system takes it from (lib.rs's inventory bridge).
pub fn request_use(data: &mut DataStore, item_id: String) {
    data.insert(USE_REQUEST_KEY, Mutex::new(Some(item_id)));
}

/// Take this frame's Use request, if there is one.
pub fn take_request(data: &DataStore) -> Option<String> {
    data.get::<Mutex<Option<String>>>(USE_REQUEST_KEY).and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
}

/// What a Use did.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// True when it helped, so one is used up.
    pub used: bool,
    /// What the player is told.
    pub message: String,
}

/// Whether a status effect carries any of `tags` (its `tags` column).
pub fn carries_any(registry: Option<&StatusEffectRegistry>, effect_id: &str, tags: &[String]) -> bool {
    registry.and_then(|r| r.get(effect_id)).is_some_and(|d| tags.iter().any(|t| d.has_tag(t)))
}

/// Apply `t` to a body: restore its Health, up to full, and end every effect carrying a tag it
/// names. When it would do neither, nothing changes and the outcome says why (`no_help`); the
/// caller removes one of the item only when `used`.
pub fn apply(
    t: &Treatment,
    health: &mut Health,
    effects: &mut StatusEffects,
    registry: Option<&StatusEffectRegistry>,
) -> Outcome {
    let ended: Vec<String> =
        effects.active.iter().filter(|e| carries_any(registry, &e.id, &t.ends)).map(|e| e.id.clone()).collect();
    let heal = t.heals.max(0.0).min((health.max - health.current).max(0.0));
    if heal <= 0.0 && ended.is_empty() {
        return Outcome { used: false, message: t.no_help.clone() };
    }
    let before = health.current;
    health.current += heal;
    for id in &ended {
        effects.remove(id);
    }
    let mut message = t.used.clone();
    if heal > 0.0 {
        message.push_str(&format!(" Health {before:.0} to {:.0}.", health.current));
    }
    if !ended.is_empty() {
        let names: Vec<&str> = ended
            .iter()
            .map(|id| registry.and_then(|r| r.get(id)).map_or(id.as_str(), |d| d.name.as_str()))
            .collect();
        message.push_str(&format!(" Ended: {}.", names.join(", ")));
    }
    Outcome { used: true, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Treatments {
        Treatments::from_ron(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/medical/treatments.ron")).unwrap())
            .expect("treatments.ron parses")
    }

    fn registry() -> StatusEffectRegistry {
        StatusEffectRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/status_effects.csv")))
            .expect("status_effects.csv")
    }

    /// The shipped treatments parse, and Use is offered for a medical item and not for a
    /// hammer.
    #[test]
    fn shipped_treatments_parse_and_say_which_items_have_a_use() {
        let t = shipped();
        assert!(t.get("bandage_0").is_some_and(|b| b.heals > 0.0));
        assert!(has_use("medkit_0") && has_use("antibiotics_0"));
        assert!(!has_use("hammer_0") && !has_use("water_bottle_0"), "only items with a treatment show Use");
    }

    /// A Medkit on a hurt body heals it and stops the bleeding, and says so; on a body that is
    /// well it changes nothing and says why.
    #[test]
    fn a_medkit_heals_the_hurt_and_is_kept_by_the_well() {
        let reg = registry();
        let t = shipped();
        let kit = t.get("medkit_0").expect("medkit_0");
        let mut h = Health { current: 40.0, max: 100.0 };
        let mut fx = StatusEffects::default();
        fx.apply("bleeding", 30.0);
        let out = apply(kit, &mut h, &mut fx, Some(&reg));
        assert!(out.used && !fx.has("bleeding"));
        assert_eq!(h.current, 40.0 + kit.heals);
        assert!(out.message.contains("Bleeding"), "{}", out.message);
        let mut well = Health::default();
        let out = apply(kit, &mut well, &mut StatusEffects::default(), Some(&reg));
        assert_eq!(out, Outcome { used: false, message: kit.no_help.clone() });
    }
}
