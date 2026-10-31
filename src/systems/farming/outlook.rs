//! A crop's wait and its water, in plain words (2026-10-12, first-hour audit
//! F11).
//!
//! The Garden card showed a crop's stage, a growth bar by stage and its water
//! level as a percentage. Nothing told a player that a lettuce takes 45 days,
//! that at the normal speed those are 45 days of real time, or that a crop
//! nothing waters is thirsty in a few hours and dead two hours after that.
//! This module says both, from the numbers the farming tick grows the crop by:
//!
//! - **When it ripens**: what is left of its plants.csv `growth_days` at its
//!   present health (the tick's `effective_progress`: an unhealthy crop grows
//!   slower), on the game clock, and what that is in real time at the clock's
//!   speed. Normal play runs at 1x, so a day is a real day; nothing here
//!   makes a crop faster, it only says how long it takes. Light is left out
//!   on purpose: the tick runs a crop's age at twice the clock while it is
//!   lit and holds it in the dark, which comes to one day a day under the
//!   sun, the pace plants.csv is written at. A grow light at night makes a
//!   crop come sooner than this says.
//! - **Its water**: whether the home's irrigation holds it (`WaterSupply`,
//!   the one test the tick waters by, so the card can never promise water the
//!   tick does not give), and if not, how long until it is thirsty or, once
//!   thirsty, how long until it dies (the tick's own rates).
//!
//! The planting notice (sowing.rs) and the Garden card (lib.rs's crop bridge
//! fills `GardenPests::outlook` from `OutlookView`) both read it. An outdoor
//! field's season and weather also slow it; that is not counted here.

use std::collections::HashMap;

use crate::ecs::components::{CropInstance, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;

use super::{PlantDef, DEFAULT_AUTO_IRRIGATION, DEHYDRATION_RATE, HEALTH_DECAY_RATE, SECONDS_PER_DAY, WATER_STRESS_THRESHOLD};

/// Where a crop's water comes from this tick: the home's tanks, its
/// irrigation, and the level each grow area's slider asks for. The farming
/// tick waters by `holds`, and the card reads the same answer.
#[derive(Debug, Clone, Default)]
pub struct WaterSupply {
    /// The tanks hold water, or the home has no tanks to run dry (no
    /// plumbing, or a cistern of capacity 0). An empty cistern (under 2% of
    /// its capacity) cuts off the irrigation and the hand watering both.
    pub water_available: bool,
    /// A machine with the `Irrigator` marker is running: one that needs no
    /// power, or a powered one whose power is on.
    pub irrigation_on: bool,
    /// The water level each grow area's irrigation slider asks for (the
    /// garden edit modal, "garden_irrigation"); an area without one is held
    /// at `DEFAULT_AUTO_IRRIGATION`.
    pub targets: HashMap<String, f32>,
}

impl WaterSupply {
    /// Read it from the world and the DataStore, the way the tick always has.
    pub fn now(world: &hecs::World, data: &DataStore) -> Self {
        let irrigation_on = world
            .query::<(
                &crate::ecs::components::Irrigator,
                Option<&crate::ecs::components::WaterConsumer>,
                Option<&crate::ecs::components::PowerConsumer>,
            )>()
            .iter()
            .any(|(_, (_, wc, pc))| {
                let needs_power = wc.map_or(false, |w| w.needs_power);
                !needs_power || pc.map_or(false, |p| p.enabled)
            });
        let water_available = data
            .get::<std::sync::Mutex<crate::systems::plumbing::WaterStatus>>("water_status")
            .and_then(|m| m.lock().ok())
            .map(|ws| ws.capacity_l <= 0.0 || ws.stored_l > ws.capacity_l * 0.02)
            .unwrap_or(true);
        let targets = data
            .get::<std::sync::Mutex<HashMap<String, f32>>>("garden_irrigation")
            .and_then(|m| m.lock().ok())
            .map(|g| g.clone())
            .unwrap_or_default();
        Self { water_available, irrigation_on, targets }
    }

    /// The water level the home's irrigation holds a crop in grow area `area`
    /// at, or None when nothing waters it automatically: no water in the
    /// tanks, no irrigation running, or a crop in no grow area.
    pub fn holds(&self, area: Option<&str>) -> Option<f32> {
        if !(self.water_available && self.irrigation_on) {
            return None;
        }
        let area = area?;
        Some(self.targets.get(area).copied().unwrap_or(DEFAULT_AUTO_IRRIGATION))
    }
}

/// What the card and the planting notice say about one crop. Every field is
/// empty for a dead crop, which has nothing left to wait for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CropOutlook {
    /// When it ripens: "about 45 days of real time", or "fully grown".
    /// Empty when the species is unknown.
    pub ripens: String,
    /// It has reached its last stage (ripe, or bearing between picks).
    pub grown: bool,
    /// Its water, as a sentence: "Kept watered by the home's irrigation.",
    /// or how long it has until it needs a hand watering.
    pub water: String,
    /// It needs the player's water now (the card shows it as a warning).
    pub thirsty: bool,
    /// A word or two for its slot tile, in real time: "45 days", "needs
    /// water". Empty once it is fully grown (the tile already says so).
    pub tile: String,
}

/// Game seconds until a crop reaches its last stage at its present health:
/// the tick ripens it once `age / growth x health` reaches 1 (health floored
/// at 0.1, as there), so what is left is `growth / health - age`.
pub fn ripe_in_game_s(crop: &CropInstance, def: &PlantDef, elapsed_s: f64) -> f64 {
    let growth = f64::from(def.growth_days.max(0.0)) * SECONDS_PER_DAY;
    let health = f64::from((crop.health / 100.0).max(0.1));
    let age = (elapsed_s - crop.planted_at).max(0.0);
    (growth / health - age).max(0.0)
}

/// A span of time in plain words: "about 45 days", "about 6 hours", "about
/// 20 minutes", "under two minutes". Whole days from a day and a half up,
/// whole hours from an hour and a half.
pub fn duration_words(secs: f64) -> String {
    let s = if secs.is_finite() { secs.max(0.0) } else { 0.0 };
    let (n, unit) = if s >= 1.5 * 86_400.0 {
        (s / 86_400.0, "days")
    } else if s >= 1.5 * 3600.0 {
        (s / 3600.0, "hours")
    } else if s >= 90.0 {
        (s / 60.0, "minutes")
    } else {
        return "under two minutes".to_string();
    };
    format!("about {:.0} {unit}", n.round())
}

/// The same span for a slot tile, without the "about": "45 days", "6 hours",
/// "20 min", "now".
fn short_words(secs: f64) -> String {
    let s = if secs.is_finite() { secs.max(0.0) } else { 0.0 };
    if s >= 1.5 * 86_400.0 {
        format!("{:.0} days", (s / 86_400.0).round())
    } else if s >= 1.5 * 3600.0 {
        format!("{:.0} hours", (s / 3600.0).round())
    } else if s >= 90.0 {
        format!("{:.0} min", (s / 60.0).round())
    } else {
        "now".to_string()
    }
}

/// The clock's speed as the player sets it: "72", "1.5".
fn speed_words(scale: f64) -> String {
    if (scale - scale.round()).abs() < 0.05 {
        format!("{:.0}", scale.round())
    } else {
        format!("{scale:.1}")
    }
}

/// How long `game_s` seconds of the game clock are for the player, at
/// `time_scale` game seconds a real second. At 1x (Normal play, the
/// default) they are real time and it says so; at any other speed it gives
/// both; with the clock held still it says the wait has stopped.
pub fn wait_words(game_s: f64, time_scale: f32) -> String {
    let scale = f64::from(time_scale);
    if !(scale > 0.0) {
        return format!("{} of game time, once the clock runs again (it is held still)", duration_words(game_s));
    }
    if (scale - 1.0).abs() < 0.01 {
        return format!("{} of real time", duration_words(game_s));
    }
    format!(
        "{} of game time, {} of real time at {}x speed",
        duration_words(game_s),
        duration_words(game_s / scale),
        speed_words(scale)
    )
}

/// Real time for `game_s` of the game clock, as the water warnings say it
/// (the clock held still: in game time).
fn real_words(game_s: f64, time_scale: f32) -> String {
    let scale = f64::from(time_scale);
    if scale > 0.0 {
        duration_words(game_s / scale)
    } else {
        format!("{} of game time", duration_words(game_s))
    }
}

/// The outlook of one crop. `holds` is what the home's irrigation holds it
/// at (`WaterSupply::holds` for its area), `water_available` whether the
/// tanks hold water for a hand watering.
pub fn outlook(
    crop: &CropInstance,
    def: Option<&PlantDef>,
    elapsed_s: f64,
    time_scale: f32,
    holds: Option<f32>,
    water_available: bool,
) -> CropOutlook {
    if crop.growth_stage == STAGE_DEAD {
        return CropOutlook::default();
    }
    let mut out = CropOutlook::default();
    if let Some(d) = def {
        if crop.growth_stage == d.last_stage() {
            out.ripens = "fully grown".to_string();
            out.grown = true;
        } else {
            let left = ripe_in_game_s(crop, d, elapsed_s);
            out.ripens = wait_words(left, time_scale);
            out.tile = if time_scale > 0.0 { short_words(left / f64::from(time_scale)) } else { "held".to_string() };
        }
    }
    // A full hand watering (to 1.0) lasts this long before the crop is thirsty.
    let lasts = real_words(f64::from((1.0 - WATER_STRESS_THRESHOLD) / DEHYDRATION_RATE), time_scale);
    match holds {
        Some(level) if level >= WATER_STRESS_THRESHOLD => {
            out.water = "The home's irrigation keeps it watered.".to_string();
        }
        _ if crop.water_level < WATER_STRESS_THRESHOLD => {
            out.thirsty = true;
            out.tile = "needs water".to_string();
            let dies_in = real_words(f64::from(crop.health.max(0.0) / HEALTH_DECAY_RATE), time_scale);
            out.water = if water_available {
                format!("Thirsty: water it now, or it dies in {dies_in}.")
            } else {
                format!("Thirsty, and the water tanks are empty: it dies in {dies_in} unless they refill.")
            };
        }
        _ => {
            let thirsty_in = real_words(f64::from((crop.water_level - WATER_STRESS_THRESHOLD) / DEHYDRATION_RATE), time_scale);
            out.water = if !water_available {
                format!("Nothing waters it automatically, and the water tanks are empty: it is thirsty in {thirsty_in}.")
            } else if crop.water_level >= 0.999 {
                format!("Nothing waters it automatically: water it by hand before it dries out (a full watering lasts {lasts}).")
            } else {
                format!("Nothing waters it automatically: water it by hand within {thirsty_in} (a full watering lasts {lasts}).")
            };
        }
    }
    out
}

/// The lowest value `s` may fall to with `duration_words(s)` and `short_words(s)` unchanged
/// (both step at the same places: a whole day from a day and a half up, a whole hour from an hour
/// and a half, a whole minute from 90 s, nothing under that). A count shown as `k` holds while the
/// value stays at or above `k - 0.5` units (`round` keeps `k` down to there).
fn words_floor(s: f64) -> f64 {
    let s = if s.is_finite() { s.max(0.0) } else { 0.0 };
    let step = |unit: f64, floor: f64| (((s / unit).round() - 0.5) * unit).max(floor);
    if s >= 1.5 * 86_400.0 {
        step(86_400.0, 1.5 * 86_400.0)
    } else if s >= 1.5 * 3600.0 {
        step(3600.0, 1.5 * 3600.0)
    } else if s >= 90.0 {
        step(60.0, 90.0)
    } else {
        f64::NEG_INFINITY
    }
}

/// The game clock's second until which `outlook` gives this crop the same words, all else equal
/// (2026-10-31, docs/design/sim-frame-cost.md, "The Garden views by area": the Garden page keeps a
/// crop's row until something it reads changes, and the clock is the one input of this row that
/// moves by itself). Only "when it ripens" and the tile's count read the clock, and they count
/// down by whole days, hours or minutes, so they hold until the next step of the count, in game
/// time and in real time at the clock's speed. A little early (by `MARGIN_S`), never late: a row
/// built again early is the same row. Infinite for a dead or fully grown crop, an unknown
/// species, or a wait already at zero.
pub fn words_hold_until(crop: &CropInstance, def: Option<&PlantDef>, elapsed_s: f64, time_scale: f32) -> f64 {
    /// How much earlier than the exact step the row is built again, game seconds: the steps are
    /// worked out in floating point, and a row must never outlive its words.
    const MARGIN_S: f64 = 0.01;
    let Some(d) = def else { return f64::INFINITY };
    if crop.growth_stage == STAGE_DEAD || crop.growth_stage == d.last_stage() {
        return f64::INFINITY;
    }
    let left = ripe_in_game_s(crop, d, elapsed_s);
    if !(left > 0.0) {
        return f64::INFINITY;
    }
    // `left` falls one game second a second once the crop is sown (its age is held at 0 before).
    let mut lowest = words_floor(left);
    let scale = f64::from(time_scale);
    if scale > 0.0 {
        // The real-time count (and the tile's) steps where left / scale does.
        lowest = lowest.max(words_floor(left / scale) * scale);
    }
    let mut until = if lowest == f64::NEG_INFINITY { f64::INFINITY } else { elapsed_s + (left - lowest) - MARGIN_S };
    if elapsed_s < crop.planted_at {
        until = until.min(crop.planted_at);
    }
    until
}

/// The outlook of every crop on one frame, for the Garden card: the clock and
/// the water supply read once, then `row` per crop. lib.rs's crop bridge
/// fills `GardenPests::outlook` with it, keyed by the crop's entity bits.
pub struct OutlookView {
    supply: WaterSupply,
    elapsed_s: f64,
    time_scale: f32,
}

impl OutlookView {
    pub fn new(world: &hecs::World, data: &DataStore) -> Self {
        let (elapsed_s, time_scale) = clock(data);
        Self { supply: WaterSupply::now(world, data), elapsed_s, time_scale }
    }

    /// `words_hold_until` for this view's clock.
    pub fn hold_until(&self, crop: &CropInstance, def: Option<&PlantDef>) -> f64 {
        words_hold_until(crop, def, self.elapsed_s, self.time_scale)
    }

    pub fn row(&self, crop: &CropInstance, def: Option<&PlantDef>) -> CropOutlook {
        outlook(
            crop,
            def,
            self.elapsed_s,
            self.time_scale,
            self.supply.holds(crop.tower_id.as_deref()),
            self.supply.water_available,
        )
    }
}

/// The game clock's elapsed seconds and speed (no clock: the start, at 1x).
pub fn clock(data: &DataStore) -> (f64, f32) {
    data.get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| (g.elapsed_seconds, g.time_scale)))
        .unwrap_or((0.0, crate::systems::time::DEFAULT_TIME_SPEED))
}
