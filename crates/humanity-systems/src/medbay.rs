//! The medbay's treatment (2026-11-21; the operator's call on botulism, docs/design/preserving.md).
//!
//! Botulism kills in Realistic unless a medbay treats it, and is serious but survivable in
//! Forgiving (`illness::Illness::health_per_h`, `forgiving_floor`). Treated means REACHED: the
//! player stands in a medbay (a room of the `medbay` type, data/rooms.ron: General Services'
//! medbay today; the Medical Bay district is a block on the map with no rooms yet, and becomes one
//! by the same room type the day it gets an interior) and asks for its treatment there, within the
//! course. In
//! real life botulism is treated with antitoxin in hospital (CDC, "Treatment of Botulism"); the
//! game's medbay stands for that, and ends every illness whose row says `medbay_treats`.
//!
//! THE PATH. The main loop publishes whether the player is in a medbay each frame
//! (`publish_here`, engine::medbay, from the room the HUD's room card names); the Inventory
//! page's "Get treated" button and the walk-up [E] in a medbay put a request on `REQUEST_KEY`
//! (`request`); the food system, which owns the body, takes it (`take_request`) and applies
//! `treat`. A request from anywhere else is refused with words that say where to go.
//!
//! Pure data and logic with no engine state, so it compiles in every feature set.

use std::sync::Mutex;

use humanity_core::ecs::components::StatusEffects;
use humanity_core::hot_reload::data_store::DataStore;

use crate::illness::{word, Illnesses};
use crate::status_effects::StatusEffectRegistry;

/// The DataStore channel a treatment request rides to the food system.
pub const REQUEST_KEY: &str = "medbay_treatment_request";

/// DataStore key (bool): the player stands in a medbay, published each frame by the main loop.
/// Absent is no: nowhere is a medbay until the world says so.
pub const HERE_KEY: &str = "at_medbay";

/// Ask for the medbay's treatment (the button, or [E] in a medbay).
pub fn request(data: &mut DataStore) {
    data.insert(REQUEST_KEY, Mutex::new(true));
}

/// Take this frame's treatment request, if there is one.
pub fn take_request(data: &DataStore) -> bool {
    data.get::<Mutex<bool>>(REQUEST_KEY).and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false))).unwrap_or(false)
}

/// Say whether the player stands in a medbay this frame.
pub fn publish_here(data: &mut DataStore, here: bool) {
    data.insert(HERE_KEY, here);
}

/// The player stands in a medbay.
pub fn here(data: &DataStore) -> bool {
    data.get::<bool>(HERE_KEY).copied().unwrap_or(false)
}

/// Whether anything on a body is an illness a medbay treats (the button is shown then).
pub fn treatable(ill: &Illnesses, effects: &StatusEffects) -> bool {
    ill.on(effects).any(|i| i.medbay_treats)
}

/// The medbay's treatment on a body: every illness on it that a medbay treats ends, when the
/// body is `at_medbay`. Returns what the player is told, and whether anything was treated.
pub fn treat(ill: &Illnesses, effects: &mut StatusEffects, registry: Option<&StatusEffectRegistry>, at_medbay: bool) -> (String, bool) {
    let treat: Vec<(String, String)> = ill
        .on(effects)
        .filter(|i| i.medbay_treats)
        .map(|i| (i.effect.clone(), registry.and_then(|r| r.get(&i.effect)).map_or_else(|| i.effect.clone(), |d| d.name.clone())))
        .collect();
    if !at_medbay {
        let key = if treat.is_empty() { "medbay.not_here" } else { "medbay.go_there" };
        return (word(key), false);
    }
    if treat.is_empty() {
        return (word("medbay.nothing"), false);
    }
    let names: Vec<&str> = treat.iter().map(|(_, n)| n.as_str()).collect();
    for (effect, _) in &treat {
        effects.remove(effect);
    }
    (word("medbay.treated").replace("{{names}}", &names.join(", ")), true)
}
