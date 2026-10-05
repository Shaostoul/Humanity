//! Food system -- nutrition, spoilage, cooking, and meal quality.
//!
//! Loads nutrition profiles, preservation methods, cooking methods, meal quality
//! levels, and temperature zones from `data/food_system.ron`, and the list of
//! which items are food (and which profile each one uses) from
//! `data/food/item_profiles.ron`.
//!
//! WHAT COUNTS AS FOOD is decided by that list and nothing else. An item that
//! is not in it cannot be eaten or drunk and never spoils. Until 2026-09-25
//! this was guessed from item-id prefixes, which made the grain mill and grain
//! silo edible (`grain_`), fish scales edible (`fish_`), gave 18 of the 26
//! cooking-recipe outputs no nutrition at all, and let legumes never spoil.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// One nutrition profile from `data/food_system.ron` (by food category). Only the
/// fields the nutrition loop consumes are modeled; the rest (macros, vitamins,
/// minerals, description) are ignored by serde.
#[derive(Debug, Clone, Deserialize)]
pub struct NutritionProfile {
    /// Profile id, e.g. `fruit`, `raw_vegetables`, `cooked_meat`.
    pub id: String,
    /// Category: `protein`, `produce`, `staple`, `preserved`, `beverage`,
    /// `meal`, `baked`, `fat`, `sweetener`. Drives hydration: a `beverage`
    /// hydrates like a drink and is the only thing the Drink action accepts.
    #[serde(default)]
    pub category: String,
    /// Energy density (kcal / 100 g) — drives satiation gain.
    #[serde(default)]
    pub calories_per_100g: u32,
    /// Hours the food stays safe under the storage it normally gets (see the
    /// header of food_system.ron). Drives the spoilage timer, in real seconds
    /// like every other survival clock here.
    #[serde(default)]
    pub spoilage_rate_hours: f32,
    /// Probability (0..1) of illness when eaten raw. 0 for cooked/preserved food.
    #[serde(default)]
    pub raw_consumption_risk: f32,
}

/// `data/food/item_profiles.ron`: which items are food, and the nutrition
/// profile each one uses. The ONLY source of edibility (see the module doc).
#[derive(Debug, Default, Deserialize)]
pub struct ItemProfiles {
    /// (item id, nutrition profile id) for every item a person can eat or drink.
    pub items: Vec<(String, String)>,
    /// (item id, reason) for items that items.csv files under category "food"
    /// but that nobody eats. The runtime never reads this; it lets the
    /// coverage test insist that every food-category item was decided.
    #[serde(default)]
    pub not_food: Vec<(String, String)>,
}

impl ItemProfiles {
    /// Path of the list, relative to the data directory.
    pub const FILE: &'static str = "food/item_profiles.ron";

    /// Disk first (modding), embedded copy as the fallback, the same way
    /// food_system.ron loads. A missing or unparseable file leaves the list
    /// empty, which means nothing is edible: loud in the log, never a crash.
    pub fn load(data_dir: &Path) -> Self {
        let Some(text) = crate::embedded_data::read_data_or_embedded(data_dir, Self::FILE) else {
            log::warn!("{} not found on disk or embedded: nothing is edible", Self::FILE);
            return Self::default();
        };
        ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse {}: {e}. Nothing is edible until it is fixed", Self::FILE);
            Self::default()
        })
    }
}

/// Which items a person eats or drinks, from the same two files the food
/// system reads (item_profiles.ron and food_system.ron): item id to true when
/// it is drunk (a beverage profile), false when it is eaten. Anything absent
/// is not food.
/// 2026-09-26: the inventory's Eat and Drink buttons ask this instead of
/// guessing from the item id, which offered Drink on the water pump, the
/// water tester and empty bottles.
/// 2026-10-03: read disk first like every other loader, the copy built into
/// the exe only when the data folder has none. It read the built-in copy
/// only, so an edit to either file changed nothing here until a rebuild, and
/// the source stamp (BUG-133) could not see that the binary was behind.
pub fn consume_kinds() -> &'static HashMap<String, bool> {
    static KINDS: std::sync::OnceLock<HashMap<String, bool>> = std::sync::OnceLock::new();
    KINDS.get_or_init(|| {
        let dir = crate::data_dir();
        let list: ItemProfiles = crate::embedded_data::read_data_or_embedded(&dir, ItemProfiles::FILE)
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default();
        let data: Option<FoodData> =
            crate::embedded_data::read_data_or_embedded(&dir, "food_system.ron").and_then(|t| ron::from_str(&t).ok());
        let mut out = HashMap::new();
        if let Some(data) = data {
            for (item, profile) in &list.items {
                if let Some(p) = data.nutrition_profiles.iter().find(|p| &p.id == profile) {
                    out.insert(item.clone(), p.category == "beverage");
                }
            }
        }
        out
    })
}

/// Top-level RON schema for `data/food_system.ron`.
#[derive(Debug, Deserialize)]
pub struct FoodData {
    pub nutrition_profiles: Vec<NutritionProfile>,
    pub preservation_methods: Vec<ron::Value>,
    pub cooking_methods: Vec<ron::Value>,
    pub meal_quality_levels: Vec<ron::Value>,
    pub temperature_zones: Vec<ron::Value>,
}

// Spoilage state is tracked in FoodSystem's own `spoilage: HashMap<FoodKey, ..>`
// side-table (below) rather than as an ECS component on the item -- items are
// plain data (item_id + quantity) with no per-instance component slot of their
// own, so keying by (entity, inventory-slot-index) is the practical way to
// attach per-stack state without an item-entity architecture change.

// Spoilage time comes from each food's nutrition profile
// (`spoilage_rate_hours`), so an item spoils on the timescale of the food it
// is: green peas in days, dried beans in a year. There is no cold storage yet,
// so nothing multiplies it.

// ── Nutrition tuning (real-time seconds). Vitals run 0..100. ──────────────
// v0.1005 REAL SCALE (operator: "my character keeps dying from dehydration
// [in minutes]. Can we set that to real scale? ... takes like three ish
// days to die from dehydration"): all survival clocks now run at human
// biology rates in REAL seconds - the two-realities axiom applied to
// vitals. Dehydration: ~2 days to drain + ~1 day of damage = ~3 days to
// kill. Starvation: ~1 week to drain + ~2 weeks of damage = ~3 weeks.
// The Settings > Gameplay "Vitals drain" slider still scales all of it
// (the "vitals_drain_scale" DataStore slot) for anyone who wants game-y
// pacing back.
/// Satiation lost per real second (full -> empty in ~7 days; the hungry
/// threshold at 25 lands around day 5).
const SATIATION_DECAY_PER_SEC: f32 = 100.0 / 604_800.0;
/// Hydration lost per real second (full -> empty in ~2 days; thirsty from
/// around day 1.5).
const HYDRATION_DECAY_PER_SEC: f32 = 100.0 / 172_800.0;
/// Below this satiation the `hungry` condition applies.
const HUNGRY_THRESHOLD: f32 = 25.0;
/// Below this hydration the `thirsty` condition applies.
const THIRSTY_THRESHOLD: f32 = 25.0;
/// At/above this satiation after a meal, the `well_fed` buff applies.
const WELL_FED_THRESHOLD: f32 = 70.0;
/// kcal/100g -> satiation points (so dense staples fill far more than veg).
const SATIATION_PER_CALORIE: f32 = 0.15;
/// Hydration restored by eating watery produce vs. everything else.
const PRODUCE_HYDRATION: f32 = 10.0;
const BASE_HYDRATION: f32 = 3.0;
/// Hydration restored per drink consumed (water/juice/etc. via the Drink action).
const DRINK_HYDRATION: f32 = 30.0;
/// Hydration points a litre of water is worth (2026-09-28), read off the
/// clock above: it empties 100 points in two days, at the roughly 2.5 L a day
/// a resting adult loses (urine, breath and skin: Jequier and Constant 2010,
/// "Water as an essential nutrient", Eur. J. Clin. Nutr. 64), so a point is
/// about 50 mL. What sweat costs on top (`body_heat::HeatOutcome::sweat_l`).
const HYDRATION_PER_LITRE: f32 = 20.0;
/// Health drained per second while fully starved / dehydrated (real scale,
/// v0.1005: an empty tank kills over ~2 weeks starved / ~1 day dehydrated,
/// so dehydration stays the far deadlier clock, matching human biology).
const STARVE_DAMAGE_PER_SEC: f32 = 100.0 / 1_209_600.0;
const DEHYDRATE_DAMAGE_PER_SEC: f32 = 100.0 / 86_400.0;
/// Conditions (hungry/thirsty) are refreshed to this many seconds each tick
/// while their trigger holds, so they linger briefly then fade once you recover.
const CONDITION_LINGER: f32 = 3.0;
/// Fallback durations (seconds) if status_effects.csv isn't loaded.
const FALLBACK_WELL_FED_S: f32 = 1800.0;
const FALLBACK_FOOD_POISONING_S: f32 = 5400.0;
const FALLBACK_RESTED_S: f32 = 3600.0;
/// Energy lost per real second while awake (real scale, v0.1005: full ->
/// fatigued threshold after ~16 waking hours; a sleep cycle refills).
const ENERGY_DECAY_PER_SEC: f32 = 75.0 / 57_600.0;
/// Below this energy the `fatigued` speed debuff applies; resting refills to full.
const FATIGUED_THRESHOLD: f32 = 25.0;
// ── Environment vitals (oxygen + body temperature; driven by EnvironmentContext). ──
/// Blood-oxygen lost per second in vacuum / unbreathable air (~40s of reserve).
const OXYGEN_DRAIN_PER_SEC: f32 = 2.5;
/// Blood-oxygen regained per second while breathing (catch your breath fast).
const OXYGEN_RECOVER_PER_SEC: f32 = 12.0;
/// Below this oxygen the `hypoxia` debuff applies; at 0 it's `suffocation` + damage.
const HYPOXIA_THRESHOLD: f32 = 50.0;
const SUFFOCATION_DAMAGE_PER_SEC: f32 = 8.0;
// Body temperature is `systems::body_heat` (2026-09-27): a heat balance of
// the body, its clothes and the weather, not a drift toward the air.
// ── Sanitation (organic waste → compost → fertilizer). ──
/// Waste accrued per real second while living, + per meal eaten (real
/// scale, v0.1005: background rise fills over ~3 days; meals dominate).
const WASTE_RISE_PER_SEC: f32 = 100.0 / 259_200.0;
const WASTE_PER_MEAL: f32 = 4.0;
/// Above this waste the `unsanitary` debuff applies (compost to clear it).
const UNSANITARY_THRESHOLD: f32 = 75.0;
/// Waste units consumed per unit of fertilizer produced when composting.
const WASTE_PER_FERTILIZER: f32 = 25.0;

// ── Urine: the household's largest nitrogen stream (2026-09-26). ──
//
// A person makes urine every day they live, and it carries most of the
// nitrogen a body gives back: 4.0 kg N a year of the 4.55 kg in all excreta
// (Jonsson et al. 2004, "Guidelines on the Use of Urine and Faeces in Crop
// Production", EcoSanRes 2004-2, Table 1, Swedish defaults). Before this the
// game had no urine at all: the waste meter composted into bags of compost
// at compost's 7% first-season nitrogen, and the nitrogen urine carries was
// simply not there.
//
// It rides the same sanitation path as the waste. While the player lives,
// their urine collects in the home's sealed tank, counted in PERSON-DAYS
// because the source says to ("the calculation should preferably be based
// upon the number of persons and days that it has been collected from",
// p. 7), and the Compost action that empties the waste also draws off every
// whole person-day as one `urine_stored_0` (1.5 kg, 10.9 g N; its analysis
// and the WHO month before harvest are in data/garden/nutrients.ron). The
// fraction of a day stays in the tank. The home has no urine-diverting
// toilet placed yet (data/home_outline.json lists it as "not in game yet"),
// so this stands in for its tank; like the waste meter, it is not saved.

/// The item one person-day of stored urine becomes.
const URINE_ITEM: &str = "urine_stored_0";
/// Real seconds in one person-day of urine: a person makes one person-day's
/// worth a day, on the same real clock the waste meter rises on.
const SECONDS_PER_URINE_DAY: f64 = 86_400.0;
/// The sealed tank holds this many person-days, then overflows to the septic
/// tank, where it is lost to the garden. A GAME CHOICE, not a measurement:
/// 20 L, the size of the game's water jerrycan (items.csv), because
/// households collect urine in jerrycans (Ouagadougou's "yellow jerry cans",
/// Richert et al. 2010, "Practical Guidance on the Use of Urine in Crop
/// Production", p. 25, which gives no size). At 1.5 L a person-day (550 kg a
/// year, Jonsson et al. 2004 Table 1) that is 13.3 person-days: about two
/// weeks of one person between Compost presses before any is lost.
const URINE_TANK_PERSON_DAYS: f64 = 20.0 / 1.5;

/// Add `dt` real seconds of one living person's urine to the tank, capped at
/// its size (the overflow goes to the septic tank). Returns the new level.
fn collect_urine(tank_person_days: f64, dt: f64) -> f64 {
    (tank_person_days + dt.max(0.0) / SECONDS_PER_URINE_DAY).min(URINE_TANK_PERSON_DAYS)
}

/// Unique key for tracking a specific food stack: (entity bits, inventory slot index).
type FoodKey = (u64, usize);

/// Per-item spoilage tracking.
#[derive(Debug, Clone)]
struct SpoilageState {
    spoilage_timer: f32,
    max_freshness: f32,
    spoiled: bool,
}

/// Which button a consume request came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Consume {
    /// The Eat button: anything that is food.
    Eat,
    /// The Drink button: only foods whose profile is a `beverage`.
    Drink,
}

/// Status-effect durations a meal can apply, resolved once per tick.
struct MealEffects {
    poisoning_s: f32,
    well_fed_s: f32,
    nourished_s: f32,
}

/// Tracks nutrition, spoilage, and cooking.
pub struct FoodSystem {
    pub data: FoodData,
    /// Edible item id -> index into `data.nutrition_profiles`, built from
    /// `data/food/item_profiles.ron`. An item absent from this map is not food.
    item_profile: HashMap<String, usize>,
    /// Per-item spoilage timers, keyed by (entity_id, slot_index).
    spoilage: HashMap<FoodKey, SpoilageState>,
    /// Accumulator to throttle log spam.
    log_cooldown: f32,
    /// Person-days of urine in the home's sealed collection tank, drawn off
    /// as `URINE_ITEM` by the Compost action (2026-09-26). Not saved, like
    /// the waste meter it rides beside.
    urine_person_days: f64,
    /// A night's sleep in a bed while it runs (2026-09-27, `systems::sleep`).
    asleep: Option<crate::systems::sleep::Asleep>,
    /// Each living body's heat state (2026-09-27, `systems::body_heat`). Only
    /// the core temperature is saved (in `Vitals`); the rest restarts from it.
    body_heat: HashMap<hecs::Entity, crate::systems::body_heat::Tracked>,
}

impl FoodSystem {
    pub fn new(data_dir: &Path) -> Self {
        // Disk-first (modding), embedded fallback (v0.744) — a zero-file
        // install keeps its nutrition/cooking data.
        let text = crate::embedded_data::read_data_or_embedded(data_dir, "food_system.ron")
            .unwrap_or_else(|| {
                log::warn!("food_system.ron not found on disk or embedded");
                "(nutrition_profiles:[],preservation_methods:[],cooking_methods:[],meal_quality_levels:[],temperature_zones:[])".to_string()
            });
        let data: FoodData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse food_system.ron: {e}");
            FoodData { nutrition_profiles: vec![], preservation_methods: vec![], cooking_methods: vec![], meal_quality_levels: vec![], temperature_zones: vec![] }
        });
        log::info!("Loaded food data: {} nutrition profiles, {} cooking methods", data.nutrition_profiles.len(), data.cooking_methods.len());

        // Resolve the item list against the profiles once, so a lookup per
        // inventory slot per tick is one hash probe. A row naming a profile
        // that does not exist is dropped with a warning (and fails the
        // coverage test), rather than silently feeding the item as something else.
        let list = ItemProfiles::load(data_dir);
        let mut item_profile = HashMap::with_capacity(list.items.len());
        for (item_id, profile_id) in &list.items {
            match data.nutrition_profiles.iter().position(|p| &p.id == profile_id) {
                Some(idx) => {
                    item_profile.insert(item_id.clone(), idx);
                }
                None => log::warn!(
                    "{}: {item_id} names nutrition profile '{profile_id}', which food_system.ron does not define; it is not edible",
                    ItemProfiles::FILE
                ),
            }
        }
        log::info!("Loaded {} edible items from {}", item_profile.len(), ItemProfiles::FILE);
        Self {
            data,
            item_profile,
            spoilage: HashMap::new(),
            log_cooldown: 0.0,
            urine_person_days: 0.0,
            asleep: None,
            body_heat: HashMap::new(),
        }
    }

    /// The nutrition profile of an item, or None when it is not food.
    fn profile_for(&self, item_id: &str) -> Option<&NutritionProfile> {
        self.item_profile
            .get(item_id)
            .map(|&idx| &self.data.nutrition_profiles[idx])
    }

    /// How long (real seconds) this item stays fresh, or None when it is not
    /// food and therefore never spoils.
    fn freshness_secs(&self, item_id: &str) -> Option<f32> {
        self.profile_for(item_id).map(|p| p.spoilage_rate_hours * 3600.0)
    }

    /// Eat or drink one `item_id` from the first player (Inventory + Vitals +
    /// StatusEffects) who carries it, applying the food's nutrition. Items that
    /// are not food, and non-beverages sent to Drink, are ignored. True when
    /// it was eaten or drunk (2026-10-04: the tick reports an eaten item to
    /// the quests, the opening's "eat something" step).
    fn consume(
        &self,
        world: &mut hecs::World,
        item_id: &str,
        how: Consume,
        fx: &MealEffects,
        returns: Option<(&str, u32)>,
    ) -> bool {
        use crate::ecs::components::{StatusEffects, Vitals};
        use crate::systems::inventory::Inventory;

        let Some(profile) = self.profile_for(item_id) else {
            log::debug!("[Food] {how:?} request for {item_id} ignored: not food ({})", ItemProfiles::FILE);
            return false;
        };
        let is_beverage = profile.category == "beverage";
        if how == Consume::Drink && !is_beverage {
            log::debug!("[Food] Drink request for {item_id} ignored: '{}' is not a beverage", profile.id);
            return false;
        }
        let calories = profile.calories_per_100g as f32;
        let risk = profile.raw_consumption_risk;
        // Drinks hydrate most, watery produce some, everything else barely.
        let hydration_gain = if is_beverage {
            DRINK_HYDRATION
        } else if profile.category == "produce" {
            PRODUCE_HYDRATION
        } else {
            BASE_HYDRATION
        };

        for (e, (inv, vitals, effects)) in
            world.query_mut::<(&mut Inventory, &mut Vitals, &mut StatusEffects)>()
        {
            if !inv.has_item(item_id, 1) {
                continue;
            }
            // Spoiled food (tracked by the spoilage pass in tick, §3) nourishes
            // far less and always poisons -- eating it is never a free meal.
            // Must match remove_item's OWN consumption order below (last-to-
            // first) or this can inspect a different slot's spoilage state
            // than the one actually eaten when the same item_id occupies
            // more than one slot (e.g. a fresh stack plus an older, spoiled
            // one after add_item split it across slots).
            let entity_bits: u64 = e.to_bits().into();
            let slot_idx = inv
                .slots
                .iter()
                .enumerate()
                .rev()
                .find(|(_, s)| s.as_ref().is_some_and(|stack| stack.item_id == item_id))
                .map(|(idx, _)| idx);
            let is_spoiled = slot_idx
                .and_then(|idx| self.spoilage.get(&(entity_bits, idx)))
                .is_some_and(|s| s.spoiled);

            inv.remove_item(item_id, 1);
            // Drinking from a vessel hands the empty vessel back (2026-09-26,
            // data/containers/fluids.ron), ready to fill at a tank again.
            // A slot is made for it, so a full pack never drops it (2026-09-26).
            if let Some((empty, max_stack)) = returns {
                let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
                inv.ensure_slots(occupied + 1);
                inv.add_item(empty, 1, max_stack);
            }
            let nutrition_mult = if is_spoiled { 0.25 } else { 1.0 };
            vitals.satiation = (vitals.satiation + calories * SATIATION_PER_CALORIE * nutrition_mult)
                .min(vitals.satiation_max);
            vitals.hydration =
                (vitals.hydration + hydration_gain * nutrition_mult).min(vitals.hydration_max);
            // Eating solid food leaves a little organic waste (scraps) to
            // compost later; a drink leaves none.
            if !is_beverage {
                vitals.waste = (vitals.waste + WASTE_PER_MEAL).min(vitals.waste_max);
            }
            // Spoiled food always poisons; otherwise raw food risks illness while
            // cooked/preserved food (risk 0) is safe.
            if is_spoiled || (risk > 0.0 && rand::random::<f32>() < risk) {
                effects.apply("food_poisoning", fx.poisoning_s);
                log::info!(
                    "[Food] {item_id} consumed {} -> food poisoning!",
                    if is_spoiled { "spoiled" } else { "raw" }
                );
            }
            // A satisfying meal grants well_fed (stamina regen) + well_nourished
            // (a tangible +10% move speed via the camera speed_multiplier).
            // Water has no food energy, so a glass of it is not a meal.
            if calories > 0.0 && vitals.satiation >= WELL_FED_THRESHOLD {
                effects.apply("well_fed", fx.well_fed_s);
                effects.apply("well_nourished", fx.nourished_s);
            }
            log::info!(
                "[Food] {} {item_id} ({}): satiation {:.0}/{:.0}, hydration {:.0}/{:.0}",
                if how == Consume::Drink { "drank" } else { "ate" },
                profile.id,
                vitals.satiation,
                vitals.satiation_max,
                vitals.hydration,
                vitals.hydration_max,
            );
            return true; // first player only
        }
        false
    }
}

impl System for FoodSystem {
    fn name(&self) -> &str {
        "FoodSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        use crate::ecs::components::{Health, Name, StatusEffects, Vitals};
        use crate::systems::inventory::{Inventory, ItemRegistry};
        use crate::systems::status_effects::StatusEffectRegistry;
        // THE BODY ON THE ONE CLOCK (2026-09-27, decision-briefs.md Brief 6).
        // The body's daily needs (hunger, thirst, the waking day's tiredness,
        // waste, urine, starving and dehydration, food spoiling) run on game
        // seconds, so the time-speed setting speeds them with the crops, the
        // tanks and the sun, and a night asleep costs a night of them. What
        // answers to this moment runs on real seconds (`dt`): breath, body
        // heat, a burn's g-load and timed status effects, because the player
        // moves and acts in real seconds, and at 72x a held breath would
        // otherwise last half a real second. At time speed 1 the two agree.
        let game_dt = crate::systems::time::scaled_dt(dt, data);

        // ── 1. EAT / DRINK: drain the consume_request (Eat button) and
        //    drink_request (Drink button) channels, written by the main-loop
        //    bridge, and apply the item's nutrition profile to the first player
        //    (Inventory + Vitals + StatusEffects) that actually has the item.
        //    Both go through consume(): whether an item is food, and what it
        //    does for you, comes from data/food/item_profiles.ron.
        let registry = data.get::<StatusEffectRegistry>("status_effect_registry");
        let item_registry = data.get::<ItemRegistry>("item_registry");
        let consumed = data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        let drank = data
            .get::<std::sync::Mutex<Option<String>>>("drink_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if consumed.is_some() || drank.is_some() {
            let effect_s = |id: &str, fallback: f32| {
                registry.map(|r| r.duration(id)).filter(|d| *d > 0.0).unwrap_or(fallback)
            };
            let fx = MealEffects {
                poisoning_s: effect_s("food_poisoning", FALLBACK_FOOD_POISONING_S),
                well_fed_s: effect_s("well_fed", FALLBACK_WELL_FED_S),
                nourished_s: effect_s("well_nourished", FALLBACK_WELL_FED_S),
            };
            let fluids = data.get::<crate::systems::fluids::FluidTable>("fluid_table");
            let empty_of = |id: &str| {
                fluids.and_then(|t| t.empty_of(id)).map(|e| (e.to_string(), item_registry.map_or(99, |r| r.max_stack_for(e))))
            };
            if let Some(item_id) = consumed {
                let back = empty_of(&item_id);
                // Eaten, it counts for an Eat quest step (2026-10-04, the
                // opening's "eat something"): reported here, where the item
                // leaves the backpack, so a click on food the player does not
                // carry counts nothing.
                if self.consume(world, &item_id, Consume::Eat, &fx, back.as_ref().map(|(e, m)| (e.as_str(), *m))) {
                    crate::systems::quests::push_quest_event(data, crate::systems::quests::eat_event_key(&item_id));
                }
            }
            if let Some(item_id) = drank {
                let back = empty_of(&item_id);
                self.consume(world, &item_id, Consume::Drink, &fx, back.as_ref().map(|(e, m)| (e.as_str(), *m)));
            }
        }

        // ── 1b. REST: drain the rest_request channel (the Rest button): a short
        //    rest, a ten-minute nap (systems::sleep::short_rest). It takes the
        //    edge off; only a night in a bed refills energy (2026-09-27).
        let do_rest = data
            .get::<std::sync::Mutex<bool>>("rest_request")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if do_rest {
            for (_e, (vitals, effects)) in world.query_mut::<(&mut Vitals, &mut StatusEffects)>() {
                let msg = crate::systems::sleep::short_rest(vitals, effects, ENERGY_DECAY_PER_SEC);
                log::info!("[Survival] short rest: {msg}");
                crate::systems::sleep::notice(data, msg);
                break; // first player only
            }
        }
        // ── 1b'. SLEEP (2026-09-27): a night in a built bed. The clock runs
        //    fast through it and the player wakes rested (systems::sleep).
        let rested_s = registry
            .map(|r| r.duration("rested"))
            .filter(|d| *d > 0.0)
            .unwrap_or(FALLBACK_RESTED_S);
        crate::systems::sleep::tick(&mut self.asleep, world, data, rested_s);

        // Player environment context (sealed / oxygenated, the air, the wind,
        // what the player is doing) for the oxygen + body heat vitals, computed
        // in the main loop (engine::survival_env). Absent = safe defaults: a
        // sealed, comfortable room.
        let env = data
            .get::<crate::ecs::components::EnvironmentContext>("environment_context")
            .cloned()
            .unwrap_or_default();
        let (env_oxygenated, env_g_load) = (env.oxygenated, env.g_load);
        let exposure = crate::systems::body_heat::Exposure::from_context(&env);
        let heat_mode = crate::systems::body_heat::Mode::from_store(data);
        // Asleep in a bed the body makes the least heat (ASHRAE: 0.7 met).
        let activity_met =
            if self.asleep.is_some() { crate::systems::body_heat::MET_SLEEPING } else { env.activity_met };
        // The crew g-tolerance row, if flight data loaded at all. Absent = the
        // drive is not modelled here, so nobody is crushed by a missing file.
        let g_tolerance = data
            .get::<crate::systems::flight::FlightData>("flight_data")
            .and_then(|f| f.tolerance("crew").cloned());

        // ── 1c. COMPOST: drain compost_request -> turn the player's accumulated waste
        //    into fertilizer items (the food -> waste -> compost -> soil cycle) + clear it.
        let do_compost = data
            .get::<std::sync::Mutex<bool>>("compost_request")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        // Urine collects in the sealed tank while the player lives, on the
        // same real clock the waste meter rises on (see URINE_ITEM).
        let player_alive = world
            .query::<(&Vitals, &crate::ecs::components::Controllable, Option<&crate::ecs::components::Dead>)>()
            .iter()
            .any(|(_, (_, _, dead))| dead.is_none());
        if player_alive {
            self.urine_person_days = collect_urine(self.urine_person_days, f64::from(game_dt));
        }
        if do_compost {
            let max_stack = item_registry
                .map(|r| r.max_stack_for("fertilizer_0"))
                .unwrap_or(99);
            // First entity that owns both an inventory and vitals = the player
            // (consistent with the eat pass; no Controllable filter needed).
            for (_e, (inv, vitals)) in world.query_mut::<(&mut Inventory, &mut Vitals)>() {
                let units = (vitals.waste / WASTE_PER_FERTILIZER).floor() as u32;
                if units > 0 {
                    // Volume-gated (Stage A slice 2): a full pack loses the
                    // surplus fertilizer — logged, never silent.
                    let unit_vol = item_registry
                        .map(|r| r.volume_for("fertilizer_0"))
                        .unwrap_or(0.0);
                    let lost = inv.add_item_volume_gated("fertilizer_0", units, max_stack, unit_vol);
                    if lost > 0 {
                        log::warn!("[Sanitation] pack full: {lost}x fertilizer_0 lost");
                    }
                    log::info!("[Sanitation] composted waste -> {units}x fertilizer_0");
                }
                // The same action draws the urine tank off: every whole
                // person-day becomes one stored urine for the garden. What
                // does not fit the pack stays in the tank for next time.
                let person_days = self.urine_person_days.floor().max(0.0) as u32;
                if person_days > 0 {
                    let unit_vol = item_registry.map(|r| r.volume_for(URINE_ITEM)).unwrap_or(0.0);
                    let stack = item_registry.map(|r| r.max_stack_for(URINE_ITEM)).unwrap_or(20);
                    let lost = inv.add_item_volume_gated(URINE_ITEM, person_days, stack, unit_vol);
                    let taken = person_days.saturating_sub(lost);
                    self.urine_person_days -= f64::from(taken);
                    log::info!("[Sanitation] drew off {taken}x {URINE_ITEM} ({lost} left in the tank)");
                }
                vitals.waste = 0.0;
                break; // first player only
            }
        }

        // ── 2. DECAY + CONDITIONS: every entity with Vitals gets hungrier/thirstier/
        //    short-of-breath/colder; low levels apply conditions; empty/extreme levels
        //    drain Health; timed buffs/debuffs count down and expire. All Health loss
        //    this tick is accumulated and applied once (Option<&mut Health> moves).
        //    v0.745 (loop-map rung 1): status-effect damage/healing-over-time acts
        //    here too, the largest drain source is remembered as the DEATH CAUSE,
        //    and a Controllable (player) reaching 0 health DIES: Dead is inserted
        //    after the pass and the cause is published to the "player_death" slot
        //    for the death screen.
        let mut player_died: Option<(hecs::Entity, String)> = None;
        // Worn gear's insulation (2026-09-27): each item's `clo` in
        // data/equipment.csv, added to the everyday outfit, feeds the body
        // heat model. (Until then a `cold_resist` stat scaled the freezing
        // damage, which did nothing to keep anyone warm.)
        let equipment = data
            .get::<crate::systems::economy::EquipmentRegistry>("equipment_registry");
        let mut heat_seen: Vec<hecs::Entity> = Vec::new();
        // Settings > Gameplay "Vitals drain" slider (v0.791): scales how fast
        // hunger/thirst/energy fall. 1.0 = normal, 0.0 = paused survival needs.
        // Written every frame by lib.rs from the persisted setting.
        let drain_scale = data
            .get::<std::sync::Mutex<f32>>("vitals_drain_scale")
            .and_then(|m| m.lock().ok().map(|g| *g))
            .unwrap_or(1.0)
            .clamp(0.0, 5.0);
        for (e, (vitals, effects, health, ctrl, dead, outfit)) in world.query_mut::<(
            &mut Vitals,
            &mut StatusEffects,
            Option<&mut Health>,
            Option<&crate::ecs::components::Controllable>,
            Option<&crate::ecs::components::Dead>,
            Option<&crate::ecs::components::Outfit>,
        )>() {
            heat_seen.push(e);
            let clo = crate::systems::body_heat::BASE_OUTFIT_CLO
                + match (equipment, outfit) {
                    (Some(reg), Some(o)) => reg.clo_total(o.equipped.values().map(|s| s.as_str())),
                    _ => 0.0,
                };
            // The dead do not hunger: vitals freeze until respawn so the death
            // screen is stable (no double-death, no draining while paused).
            if dead.is_some() {
                continue;
            }
            let mut health_drain = 0.0_f32;
            // Largest single drain source this tick -- becomes the death line
            // ("You died: starvation") if this is the tick that reaches zero.
            let mut worst: (&str, f32) = ("", 0.0);

            vitals.satiation =
                (vitals.satiation - SATIATION_DECAY_PER_SEC * drain_scale * game_dt).max(0.0);
            vitals.hydration =
                (vitals.hydration - HYDRATION_DECAY_PER_SEC * drain_scale * game_dt).max(0.0);
            if vitals.satiation < HUNGRY_THRESHOLD {
                effects.apply("hungry", CONDITION_LINGER);
            } else {
                effects.remove("hungry");
            }
            if vitals.hydration < THIRSTY_THRESHOLD {
                effects.apply("thirsty", CONDITION_LINGER);
            } else {
                effects.remove("thirsty");
            }
            if vitals.satiation <= 0.0 {
                let amt = STARVE_DAMAGE_PER_SEC * game_dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("starvation", amt);
                }
            }
            if vitals.hydration <= 0.0 {
                let amt = DEHYDRATE_DAMAGE_PER_SEC * game_dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("dehydration", amt);
                }
            }

            // Energy drains while awake; low energy -> fatigued (speed debuff, #3b),
            // unless a short rest is still keeping a tired (not exhausted)
            // person alert (systems::sleep::short_rest, 2026-09-27).
            vitals.energy = (vitals.energy - ENERGY_DECAY_PER_SEC * drain_scale * game_dt).max(0.0);
            if vitals.energy < FATIGUED_THRESHOLD && !crate::systems::sleep::nap_holds_off_fatigue(vitals.energy, effects) {
                effects.apply("fatigued", CONDITION_LINGER);
            } else {
                effects.remove("fatigued");
            }

            // Oxygen: recover when breathing, drain in vacuum -> hypoxia then suffocation.
            if env_oxygenated {
                vitals.oxygen =
                    (vitals.oxygen + OXYGEN_RECOVER_PER_SEC * dt).min(vitals.oxygen_max);
            } else {
                vitals.oxygen = (vitals.oxygen - OXYGEN_DRAIN_PER_SEC * dt).max(0.0);
            }
            if vitals.oxygen <= 0.0 {
                effects.remove("hypoxia");
                effects.apply("suffocation", CONDITION_LINGER);
                let amt = SUFFOCATION_DAMAGE_PER_SEC * dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("suffocation", amt);
                }
            } else if vitals.oxygen < HYPOXIA_THRESHOLD {
                effects.remove("suffocation");
                effects.apply("hypoxia", CONDITION_LINGER);
            } else {
                effects.remove("hypoxia");
                effects.remove("suffocation");
            }

            // Body heat (2026-09-27, systems::body_heat): the core temperature
            // is a heat balance of the body's own heat, its clothes, and the
            // air, wind, wet and shelter around it (the Gagge two-node model),
            // shown and harmful per the Settings mode. It used to drift straight
            // to the air temperature, which made 15 C hypothermic in 4.5 s.
            let tracked = self
                .body_heat
                .entry(e)
                .or_insert_with(|| crate::systems::body_heat::Tracked::new(vitals.body_temp_c, heat_mode));
            let heat = crate::systems::body_heat::vitals_tick(
                tracked,
                &mut vitals.body_temp_c,
                effects,
                &exposure,
                clo,
                activity_met,
                heat_mode,
                dt,
                CONDITION_LINGER,
            );
            // Sweat costs water (2026-09-28): out of hydration on top of the
            // daily clock (an hour walking in dry 35 C air is about 0.2 L,
            // 4 points; hard work in the heat is several times that). The
            // Vitals drain slider scales it like the other needs.
            vitals.hydration = (vitals.hydration - heat.sweat_l * HYDRATION_PER_LITRE * drain_scale).max(0.0);
            if heat.harm > 0.0 {
                health_drain += heat.harm;
                if heat.harm > worst.1 {
                    worst = (heat.cause, heat.harm);
                }
            }

            // ── SUSTAINED ACCELERATION ──
            // A hard burn is felt everywhere aboard, sealed hull or not. Harm
            // ramps from the tolerance row's safe band, so ordinary cruise (a
            // twentieth of a g on top of the drum's spin) is silent and an
            // evasion burn is not. Nothing here is scripted: the same vector
            // sum that tilts the drum floor also fills this number.
            //
            // Zero-g is deliberately NOT harmful. Weightlessness has real
            // long-term costs, but they are bone and muscle over months, which
            // belongs in a medical model rather than in a per-second drain.
            if let Some(ref tol) = g_tolerance {
                let rate = crate::systems::flight::harm_per_sec(tol, env_g_load);
                if rate > 0.0 {
                    effects.apply("high_g", CONDITION_LINGER);
                    let amt = rate * dt;
                    health_drain += amt;
                    if amt > worst.1 {
                        worst = ("crushed by acceleration", amt);
                    }
                } else {
                    effects.remove("high_g");
                }
            }

            // Organic waste accrues while living; high waste -> the unsanitary debuff.
            vitals.waste = (vitals.waste + WASTE_RISE_PER_SEC * game_dt).min(vitals.waste_max);
            if vitals.waste > UNSANITARY_THRESHOLD {
                effects.apply("unsanitary", CONDITION_LINGER);
            } else {
                effects.remove("unsanitary");
            }

            // ── EFFECT TICK (v0.745, loop-map rung 1): damage/healing-over-time
            // rows from status_effects.csv finally act. Per-tick values are
            // normalized to a continuous per-second rate by tick_interval_s
            // (0 = the value is already per second): food_poisoning's
            // 3 dmg / 15 s drains 0.2/s; regeneration's 5 heal / 3 s restores
            // ~1.67/s. This is also the game's first health REGENERATION path.
            let mut effect_heal = 0.0_f32;
            if let Some(reg) = registry {
                for active in &effects.active {
                    if let Some(def) = reg.get(&active.id) {
                        let interval =
                            if def.tick_interval_s > 0.0 { def.tick_interval_s } else { 1.0 };
                        if def.damage_per_tick > 0.0 {
                            let amt = def.damage_per_tick / interval * dt;
                            health_drain += amt;
                            if amt > worst.1 {
                                worst = (def.name.as_str(), amt);
                            }
                        }
                        if def.healing_per_tick > 0.0 {
                            effect_heal += def.healing_per_tick / interval * dt;
                        }
                    }
                }
            }

            // Apply the tick's accumulated Health drain + healing (all sources).
            // A Controllable (the player) whose health reaches zero THIS tick
            // dies: recorded here, Dead inserted after the query borrow ends.
            if let Some(health) = health {
                let before = health.current;
                if health_drain > 0.0 || effect_heal > 0.0 {
                    health.current =
                        (health.current - health_drain + effect_heal).clamp(0.0, health.max);
                }
                if ctrl.is_some() && before > 0.0 && health.current <= 0.0 {
                    let cause =
                        if worst.0.is_empty() { "injuries".to_string() } else { worst.0.to_string() };
                    player_died = Some((e, cause));
                }
            }

            // Expire timed effects (conditions were just refreshed, so they survive dt).
            effects.tick(dt);
        }
        // Forget the heat state of bodies that are gone.
        self.body_heat.retain(|e, _| heat_seen.contains(e));

        // Death (v0.745): mark the player Dead + publish the cause for the death
        // screen (the "player_death" DataStore slot; lib.rs surfaces it). Done
        // outside the query pass because hecs cannot insert mid-borrow.
        if let Some((entity, cause)) = player_died {
            let _ = world.insert_one(entity, crate::ecs::components::Dead::default());
            if let Some(slot) = data.get::<std::sync::Mutex<Option<String>>>("player_death") {
                if let Ok(mut s) = slot.lock() {
                    *s = Some(cause.clone());
                }
            }
            log::info!("[Vitals] player died: {cause}");
        }

        // ── 3. SPOILAGE (existing): age food sitting in inventories. ──
        let should_log = self.log_cooldown <= 0.0;
        if should_log {
            self.log_cooldown = 10.0;
        }
        self.log_cooldown -= dt;

        // Track which food keys are still alive this tick (for cleanup)
        let mut active_keys = std::collections::HashSet::new();

        // Scan all entities that have an inventory
        for (entity, (inv, name)) in world.query::<(&Inventory, Option<&Name>)>().iter() {
            let entity_bits: u64 = entity.to_bits().into();
            let owner = name.map_or_else(
                || format!("entity_{entity_bits}"),
                |n| n.0.clone(),
            );

            for (slot_idx, slot) in inv.slots.iter().enumerate() {
                let stack = match slot.as_ref() {
                    Some(s) => s,
                    None => continue,
                };

                // Only food spoils, on its own profile's timescale.
                let Some(max_freshness) = self.freshness_secs(&stack.item_id) else {
                    continue;
                };

                let key: FoodKey = (entity_bits, slot_idx);
                active_keys.insert(key);

                let state = self.spoilage.entry(key).or_insert_with(|| SpoilageState {
                    spoilage_timer: 0.0,
                    max_freshness,
                    spoiled: false,
                });

                // Update max_freshness in case the item changed (slot reuse)
                state.max_freshness = max_freshness;

                if state.spoiled {
                    continue; // already spoiled, nothing more to do
                }

                // Advance spoilage timer
                state.spoilage_timer += game_dt;

                if state.spoilage_timer >= state.max_freshness {
                    state.spoiled = true;
                    log::info!(
                        "[Food] {owner}'s {} (slot {slot_idx}) has spoiled after {:.0}s",
                        stack.item_id, state.spoilage_timer,
                    );
                    // The item itself stays as-is (no item-def swap, so it still
                    // stacks/sells as the same item_id); consume() (§1, eat and
                    // drink) looks up this slot's spoiled flag and applies the real
                    // consequence -- reduced nutrition + guaranteed food poisoning.
                } else if should_log {
                    let pct = (state.spoilage_timer / state.max_freshness * 100.0) as u32;
                    if pct >= 75 {
                        log::debug!(
                            "[Food] {owner}'s {} (slot {slot_idx}) is {pct}% spoiled",
                            stack.item_id,
                        );
                    }
                }
            }
        }

        // Garbage-collect spoilage entries for items that no longer exist
        self.spoilage.retain(|k, _| active_keys.contains(k));
    }
}

#[cfg(test)]
mod nutrition_tests {
    use super::*;
    use crate::ecs::components::{Health, StatusEffects, Vitals};
    use crate::ecs::systems::System;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::inventory::Inventory;
    use crate::systems::status_effects::StatusEffectRegistry;
    use std::path::Path;

    fn data_dir() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
    }

    /// DataStore with the status-effect registry + the consume_request channel,
    /// mirroring the runtime wiring in lib.rs.
    fn make_store() -> DataStore {
        let mut data = DataStore::new();
        let reg = StatusEffectRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/status_effects.csv"
        )))
        .expect("status_effects.csv");
        data.insert("status_effect_registry", reg);
        data.insert(
            "consume_request",
            std::sync::Mutex::new(Option::<String>::None),
        );
        data.insert("rest_request", std::sync::Mutex::new(false));
        data.insert("compost_request", std::sync::Mutex::new(false));
        data.insert("drink_request", std::sync::Mutex::new(Option::<String>::None));
        data.insert("player_death", std::sync::Mutex::new(Option::<String>::None));
        data
    }

    fn vitals(satiation: f32, hydration: f32) -> Vitals {
        Vitals {
            satiation,
            hydration,
            energy: 100.0,
            oxygen: 100.0,
            body_temp_c: 37.0,
            waste: 0.0,
            satiation_max: 100.0,
            hydration_max: 100.0,
            energy_max: 100.0,
            oxygen_max: 100.0,
            waste_max: 100.0,
        }
    }

    /// v0.791 Settings > Gameplay "Vitals drain": the slider scales hunger/
    /// thirst/energy decay; 0 pauses survival needs entirely.
    #[test]
    fn vitals_drain_scale_slows_and_pauses_decay() {
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        data.insert("vitals_drain_scale", std::sync::Mutex::new(0.0_f32));
        let mut world = hecs::World::new();
        let e = world.spawn((
            Inventory::new(4),
            vitals(50.0, 50.0),
            StatusEffects::default(),
            Health { current: 100.0, max: 100.0 },
        ));
        // Scale 0: an hour passes, nothing drains.
        sys.tick(&mut world, 3600.0, &data);
        {
            let v = world.get::<&Vitals>(e).unwrap();
            assert_eq!(v.satiation, 50.0, "satiation paused at scale 0");
            assert_eq!(v.hydration, 50.0, "hydration paused at scale 0");
            assert_eq!(v.energy, 100.0, "energy paused at scale 0");
        }
        // Scale 2: decays exactly twice the base rate.
        *data
            .get::<std::sync::Mutex<f32>>("vitals_drain_scale")
            .unwrap()
            .lock()
            .unwrap() = 2.0;
        sys.tick(&mut world, 10.0, &data);
        let v = world.get::<&Vitals>(e).unwrap();
        assert!((v.hydration - (50.0 - HYDRATION_DECAY_PER_SEC * 2.0 * 10.0)).abs() < 1e-3);
        assert!((v.satiation - (50.0 - SATIATION_DECAY_PER_SEC * 2.0 * 10.0)).abs() < 1e-3);
    }

    /// The body's daily needs follow the one game clock (2026-09-27): at time
    /// speed 72, ten real seconds cost 72 times the hunger, thirst, tiredness
    /// and waste they cost at 1; the moment-to-moment body does not scale
    /// (breath comes back at the same real rate). Thirst is both since
    /// 2026-09-29: the daily clock, which scales, plus sweat, which the body
    /// heat model makes in real seconds like breath, so only the clock's share
    /// of it is 72 times larger (the body here starts at 37 C, a little over
    /// neutral, and sweats a little). Red check, run: the decay on raw `dt`
    /// instead of `game_dt` makes the 72x drops equal the 1x ones and the
    /// ratio assertions fail.
    #[test]
    fn the_time_speed_runs_the_bodys_daily_needs_and_not_its_breath() {
        let drops = |speed: f32| -> [f32; 5] {
            let mut sys = FoodSystem::new(data_dir());
            let data = make_store();
            let mut data = data;
            let mut gt = crate::systems::time::GameTime::default();
            gt.time_scale = speed;
            data.insert("game_time", std::sync::Mutex::new(gt));
            let mut world = hecs::World::new();
            let mut v = vitals(1.0, 1.0);
            v.oxygen = 50.0;
            let e = world.spawn((Inventory::new(4), v, StatusEffects::default(), Health { current: 100.0, max: 100.0 }));
            sys.tick(&mut world, 2.0, &data);
            let v = world.get::<&Vitals>(e).unwrap();
            [1.0 - v.satiation, 1.0 - v.hydration, 100.0 - v.energy, v.waste, v.oxygen - 50.0]
        };
        let slow = drops(1.0);
        let fast = drops(72.0);
        for (i, name) in [(0, "hunger"), (2, "tiredness"), (3, "waste")] {
            assert!(slow[i] > 0.0, "{name} moves at 1x");
            assert!((fast[i] / slow[i] / 72.0 - 1.0).abs() < 0.01, "{name}: {} vs {}", fast[i], slow[i]);
        }
        // Thirst: the same real-second sweat at both speeds, so the difference
        // is the daily clock's share, 71 times its 1x amount over 2 s.
        let clock_extra = 71.0 * HYDRATION_DECAY_PER_SEC * 2.0;
        assert!(slow[1] > 0.0, "thirst moves at 1x");
        assert!(((fast[1] - slow[1]) / clock_extra - 1.0).abs() < 0.01, "thirst: {} vs {}", fast[1], slow[1]);
        assert_eq!(fast[4], slow[4], "breath recovers in real seconds at any time speed");
    }

    /// Clothing keeps a body warm (2026-09-27): the winter coat's 0.70 clo
    /// (data/equipment.csv, ISO 7730's parka) goes on top of the everyday
    /// outfit, so at -20 C in a 5 m/s wind, standing, the coated person's core
    /// is still near 36.6 C after 3 hours while the one in everyday clothes has
    /// cooled to about 35.1 C, shivering tiring. (Until 2026-09-27 a coat only scaled the
    /// freezing damage, and this test's environment was inserted as the wrong
    /// type, so it ran in a warm room and passed whatever the coat did.)
    /// Red check: drop the Outfit's `clo_total` from the food system's
    /// clothing and the two cores are the same.
    #[test]
    fn a_winter_coat_keeps_the_core_warm() {
        use crate::ecs::components::{EnvironmentContext, Outfit};
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        let equip = crate::systems::economy::EquipmentRegistry::from_csv(include_bytes!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/data/equipment.csv")
        ))
        .expect("equipment.csv");
        data.insert("equipment_registry", equip);
        data.insert(
            "environment_context",
            EnvironmentContext {
                sealed: false,
                oxygenated: true,
                ambient_temp_c: -20.0,
                relative_humidity: 0.7,
                wind_m_s: 5.0,
                ..Default::default()
            },
        );

        let mut world = hecs::World::new();
        let bare = world.spawn((Inventory::new(4), vitals(80.0, 80.0), StatusEffects::default(), Health::default()));
        let mut coat_outfit = Outfit::default();
        coat_outfit.equipped.insert("chest".to_string(), "coat_winter_0".to_string());
        let coated =
            world.spawn((Inventory::new(4), vitals(80.0, 80.0), StatusEffects::default(), Health::default(), coat_outfit));

        for _ in 0..(3 * 60) {
            sys.tick(&mut world, 60.0, &data);
        }
        let bare_c = world.get::<&Vitals>(bare).unwrap().body_temp_c;
        let coated_c = world.get::<&Vitals>(coated).unwrap().body_temp_c;
        assert!(coated_c > 36.0, "the coat held the core: {coated_c}");
        assert!(coated_c > bare_c + 0.5, "coated {coated_c}, everyday clothes {bare_c}");
        // Conditions linger 3 s past their trigger, so a minute-long tick
        // expires them inside itself; look after a frame-sized one.
        sys.tick(&mut world, 1.0, &data);
        assert!(world.get::<&StatusEffects>(bare).unwrap().has("shivering"), "the cold body shivers");
    }

    /// SWEAT COSTS WATER (2026-09-28). An hour walking in dry 35 C air
    /// against the same hour at 20 C: the heat's sweat comes out of hydration
    /// on top of the daily clock, which both pay alike. Red check, run: not
    /// taking `sweat_l` out of hydration makes the two drops equal and fails
    /// the first assertion.
    #[test]
    fn sweat_costs_water() {
        use crate::ecs::components::EnvironmentContext;
        let hour_walking_at = |air_c: f32| {
            let mut sys = FoodSystem::new(data_dir());
            let mut data = make_store();
            data.insert(
                "environment_context",
                EnvironmentContext {
                    sealed: false,
                    oxygenated: true,
                    ambient_temp_c: air_c,
                    relative_humidity: 0.3,
                    wind_m_s: 1.0,
                    activity_met: crate::systems::body_heat::MET_WALKING,
                    ..Default::default()
                },
            );
            let mut world = hecs::World::new();
            let e = world.spawn((Inventory::new(4), vitals(80.0, 80.0), StatusEffects::default(), Health::default()));
            for _ in 0..60 {
                sys.tick(&mut world, 60.0, &data);
            }
            let left = world.get::<&Vitals>(e).unwrap().hydration;
            80.0 - left
        };
        let (hot, mild) = (hour_walking_at(35.0), hour_walking_at(20.0));
        // Measured 2026-09-28: 6.63 points at 35 C against 3.20 at 20 C (the
        // daily clock alone is 2.08 an hour): about 0.17 L more sweat.
        const SWEAT_MARGIN_POINTS: f32 = 3.0;
        assert!(hot > mild + SWEAT_MARGIN_POINTS, "an hour walking: {hot:.2} points at 35 C, {mild:.2} at 20 C");
        assert!(mild - HYDRATION_DECAY_PER_SEC * 3600.0 < 1.5, "walking at 20 C sweats only a little: {mild:.2}");
    }

    /// v0.745 EFFECT TICK (loop-map rung 1): damage/healing-over-time rows in
    /// status_effects.csv finally act. food_poisoning (3 dmg / 15 s) drains
    /// health at 0.2/s; regeneration (5 heal / 3 s) restores it. Pinned with
    /// exact rates so a CSV rebalance shows up as a test diff, not a surprise.
    #[test]
    fn status_effect_damage_and_healing_tick_on_health() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();
        let mut world = hecs::World::new();
        let mut fx = StatusEffects::default();
        fx.apply("food_poisoning", 5400.0);
        let e = world.spawn((Inventory::new(4), vitals(80.0, 80.0), fx, Health::default()));

        // 30 simulated seconds of poison at 3/15 = 0.2 dmg/s -> ~6 damage.
        for _ in 0..30 {
            sys.tick(&mut world, 1.0, &data);
        }
        let after_poison = world.get::<&Health>(e).unwrap().current;
        assert!(
            (93.0..95.5).contains(&after_poison),
            "food_poisoning drained ~6 HP over 30s, got {after_poison}"
        );

        // Swap poison for regeneration (5 heal / 3 s): health climbs back.
        {
            let mut fx = world.get::<&mut StatusEffects>(e).unwrap();
            fx.remove("food_poisoning");
            fx.apply("regeneration", 300.0);
        }
        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
        }
        let after_regen = world.get::<&Health>(e).unwrap().current;
        assert!(
            after_regen > after_poison + 3.0,
            "regeneration healed (was {after_poison}, now {after_regen})"
        );
    }

    /// v0.745 DEATH (loop-map rung 1): a Controllable whose health reaches zero
    /// gets the Dead component and the death CAUSE lands in the player_death
    /// slot for the death screen. Dead entities stop decaying (no double death).
    #[test]
    fn player_death_inserts_dead_and_records_the_cause() {
        use crate::ecs::components::{Controllable, Dead};
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();
        let mut world = hecs::World::new();
        // Starving: satiation 0 drains health every tick.
        let mut v = vitals(0.0, 80.0);
        v.energy = 100.0;
        let e = world.spawn((
            Inventory::new(4),
            v,
            StatusEffects::default(),
            Health { current: 2.0, max: 100.0 },
            Controllable,
        ));

        // Real-scale clocks (v0.1005): starvation drains ~100 HP over two
        // weeks, so 2 HP takes ~6.7 hours - three 3-hour ticks cover it,
        // while hydration (80, ~2-day drain) only falls ~19 points and
        // stays far from zero, keeping the cause unambiguously starvation.
        for _ in 0..3 {
            sys.tick(&mut world, 10_800.0, &data);
        }
        assert!(world.get::<&Dead>(e).is_ok(), "player marked Dead at 0 HP");
        let cause = data
            .get::<std::sync::Mutex<Option<String>>>("player_death")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(cause.as_deref(), Some("starvation"), "cause recorded for the death screen");

        // Dead = frozen: hydration would keep decaying if the pass still ran.
        let hyd_before = world.get::<&Vitals>(e).unwrap().hydration;
        sys.tick(&mut world, 3600.0, &data);
        let hyd_after = world.get::<&Vitals>(e).unwrap().hydration;
        assert_eq!(hyd_before, hyd_after, "vitals freeze while dead");
    }

    /// Eating a cooked/preserved food restores satiation + hydration on the
    /// player's ECS Vitals, consumes the item, and never causes food poisoning
    /// (canned_food has raw_consumption_risk 0, so the outcome is deterministic).
    #[test]
    fn eating_cooked_food_feeds_the_player_safely() {
        let mut sys = FoodSystem::new(data_dir());
        // The typed nutrition profiles must have loaded from food_system.ron.
        assert!(
            sys.data.nutrition_profiles.iter().any(|p| p.id == "canned_food"),
            "nutrition profiles parsed from food_system.ron"
        );

        let data = make_store();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item("canned_food_0", 1, 99);
        let player = world.spawn((inv, vitals(40.0, 50.0), StatusEffects::default(), Health::default()));

        *data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
            .unwrap()
            .lock()
            .unwrap() = Some("canned_food_0".to_string());

        sys.tick(&mut world, 1.0, &data);

        let inv = world.get::<&Inventory>(player).unwrap();
        let v = world.get::<&Vitals>(player).unwrap();
        let effects = world.get::<&StatusEffects>(player).unwrap();
        assert_eq!(inv.count_item("canned_food_0"), 0, "the eaten item is consumed");
        assert!(v.satiation > 40.0, "satiation rose from eating (40 -> {})", v.satiation);
        assert!(v.hydration > 49.0, "hydration rose from eating (50 -> {})", v.hydration);
        assert!(
            !effects.has("food_poisoning"),
            "cooked/preserved food (risk 0) never poisons"
        );
    }

    /// Eating a SPOILED item (tracked by the spoilage side-table in §3 of tick())
    /// always causes food poisoning and grants far less nutrition than eating the
    /// same fresh item -- even though cooked_meat's own raw_consumption_risk is 0.
    /// (roast_chicken_0 is a real items.csv row that uses the cooked_meat
    /// profile; these tests used an invented `cooked_meat_0` id until
    /// 2026-09-25, which only worked while edibility was guessed from prefixes.)
    #[test]
    fn eating_spoiled_food_poisons_and_reduces_nutrition() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item("roast_chicken_0", 1, 99);
        let player = world.spawn((inv, vitals(40.0, 50.0), StatusEffects::default(), Health::default()));

        // One tick (no consume_request) registers the item's slot in the
        // spoilage side-table via §3; then force it spoiled, mirroring what
        // happens naturally once max_freshness elapses.
        sys.tick(&mut world, 0.0, &data);
        let entity_bits: u64 = player.to_bits().into();
        let slot_idx = world
            .get::<&Inventory>(player)
            .unwrap()
            .slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "roast_chicken_0"))
            .expect("roast_chicken_0 tracked in spoilage side-table");
        sys.spoilage.get_mut(&(entity_bits, slot_idx)).unwrap().spoiled = true;

        *data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
            .unwrap()
            .lock()
            .unwrap() = Some("roast_chicken_0".to_string());
        sys.tick(&mut world, 0.0, &data);

        let v = world.get::<&Vitals>(player).unwrap();
        let effects = world.get::<&StatusEffects>(player).unwrap();
        assert!(
            effects.has("food_poisoning"),
            "spoiled cooked_meat (own risk=0) still poisons once spoiled"
        );
        // 165 kcal/100g * SATIATION_PER_CALORIE (0.15) = 24.75 fresh, so a full
        // gain would land near 64.75; the 0.25x spoiled multiplier caps it well
        // under 50.
        assert!(
            v.satiation < 50.0,
            "spoiled food gives much less satiation than fresh (got {})",
            v.satiation
        );
    }

    /// When the SAME item_id occupies two separate slots (a fresh stack plus
    /// an older, spoiled one -- a normal reachable state once `add_item`
    /// splits a stack across slots after the first one fills), eating must
    /// check the spoilage of whichever slot `remove_item` ACTUALLY consumes
    /// from (last-to-first, see `Inventory::remove_item`), not just the
    /// first matching slot found. This is the exact bug an adversarial
    /// review caught in the initial BUG-044 fix.
    #[test]
    fn spoilage_check_matches_the_slot_remove_item_actually_consumes() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        // Two separate stacks of the same item_id: slot 0 (fresh) and slot 3
        // (will be marked spoiled). remove_item consumes last-to-first, so a
        // single eat should draw from slot 3, not slot 0.
        inv.slots[0] = Some(crate::systems::inventory::ItemStack::new("roast_chicken_0".to_string(), 1, 99));
        inv.slots[3] = Some(crate::systems::inventory::ItemStack::new("roast_chicken_0".to_string(), 1, 99));
        let player = world.spawn((inv, vitals(40.0, 50.0), StatusEffects::default(), Health::default()));

        // Register both slots in the spoilage side-table, then mark ONLY
        // slot 3 (the one that will actually be eaten) as spoiled.
        sys.tick(&mut world, 0.0, &data);
        let entity_bits: u64 = player.to_bits().into();
        sys.spoilage.get_mut(&(entity_bits, 0)).unwrap().spoiled = false;
        sys.spoilage.get_mut(&(entity_bits, 3)).unwrap().spoiled = true;

        *data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
            .unwrap()
            .lock()
            .unwrap() = Some("roast_chicken_0".to_string());
        sys.tick(&mut world, 0.0, &data);

        // Exactly one unit should have been removed, from slot 3 (last
        // matching slot) -- slot 0's fresh stack must be untouched.
        let inv = world.get::<&Inventory>(player).unwrap();
        assert!(inv.slots[0].is_some(), "the fresh stack in slot 0 must be untouched");
        assert!(inv.slots[3].is_none(), "the spoiled stack in slot 3 is the one actually eaten");
        drop(inv);

        let effects = world.get::<&StatusEffects>(player).unwrap();
        assert!(
            effects.has("food_poisoning"),
            "the slot actually eaten (3) was spoiled -- must poison regardless of slot 0's fresh state"
        );
    }

    /// Below the hunger threshold the decay pass applies the `hungry` condition;
    /// at zero satiation the player loses Health (starvation).
    #[test]
    fn low_satiation_triggers_hungry_then_starvation_damage() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store(); // consume_request stays None — no eating

        let mut world = hecs::World::new();
        let player = world.spawn((
            Inventory::new(4),
            vitals(10.0, 80.0),
            StatusEffects::default(),
            Health::default(),
        ));

        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&StatusEffects>(player).unwrap().has("hungry"),
            "low satiation applies the hungry condition"
        );

        // Drive satiation to empty; the next tick should drain health (starvation).
        world.get::<&mut Vitals>(player).unwrap().satiation = 0.0;
        let before = world.get::<&Health>(player).unwrap().current;
        sys.tick(&mut world, 1.0, &data);
        let after = world.get::<&Health>(player).unwrap().current;
        assert!(after < before, "starvation drains health ({before} -> {after})");
    }

    /// Low energy applies the `fatigued` speed debuff. The Rest button is a
    /// short rest now (2026-09-27, systems::sleep::short_rest): a ten-minute
    /// nap that lifts the fatigue slowdown for a while but gives back only
    /// ten minutes of sleep, so a night in a bed is still what restores energy.
    /// Red check: the old Rest set energy to full, which `energy < 25` catches.
    #[test]
    fn low_energy_fatigues_and_a_short_rest_takes_the_edge_off() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();

        let mut world = hecs::World::new();
        let mut tired = vitals(80.0, 80.0);
        tired.energy = 20.0;
        let player = world.spawn((Inventory::new(4), tired, StatusEffects::default(), Health::default()));

        // Low energy -> fatigued after a tick.
        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&StatusEffects>(player).unwrap().has("fatigued"),
            "low energy applies the fatigued speed debuff"
        );

        // Rest -> a nap: fatigue lifted, energy barely moved.
        *data
            .get::<std::sync::Mutex<bool>>("rest_request")
            .unwrap()
            .lock()
            .unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        let energy = world.get::<&Vitals>(player).unwrap().energy;
        assert!(energy > 20.0 && energy < 25.0, "a nap, not a night (got {energy})");
        let fx = world.get::<&StatusEffects>(player).unwrap();
        assert!(fx.has("refreshed"), "the nap refreshed the player");
        assert!(!fx.has("fatigued"), "and lifted the fatigue slowdown");
    }

    /// Exposure to vacuum (an exposed, airless EnvironmentContext) drains
    /// oxygen: hypoxia, then suffocation and health loss within a minute;
    /// re-sealing recovers the oxygen. The body does NOT freeze in that minute:
    /// a person's core takes hours to cool (systems::body_heat), and the old
    /// drift model's "below 35 C within 30 s at -40 C" is the defect this
    /// test used to assert.
    #[test]
    fn exposure_drains_oxygen_and_recovers_when_sealed() {
        use crate::ecs::components::EnvironmentContext;
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        data.insert(
            "environment_context",
            EnvironmentContext {
                sealed: false,
                oxygenated: false,
                ambient_temp_c: -40.0,
                pressure_kpa: 0.0,
                ..Default::default()
            },
        );

        let mut world = hecs::World::new();
        let player = world.spawn((
            Inventory::new(4),
            vitals(80.0, 80.0),
            StatusEffects::default(),
            Health::default(),
        ));

        for _ in 0..50 {
            sys.tick(&mut world, 1.0, &data);
        }
        {
            let v = world.get::<&Vitals>(player).unwrap();
            assert!(v.oxygen < 50.0, "oxygen drained while exposed (got {})", v.oxygen);
            assert!(v.body_temp_c > 36.5, "a minute does not freeze a body (got {})", v.body_temp_c);
            let fx = world.get::<&StatusEffects>(player).unwrap();
            assert!(fx.has("hypoxia") || fx.has("suffocation"), "an oxygen condition applied");
            assert!(!fx.has("hypothermia"), "no hypothermia in a minute");
        }
        assert!(
            world.get::<&Health>(player).unwrap().current < 100.0,
            "suffocation damaged health"
        );

        // Re-seal the environment → oxygen recovers, oxygen conditions clear.
        data.insert("environment_context", EnvironmentContext::default());
        for _ in 0..15 {
            sys.tick(&mut world, 1.0, &data);
        }
        let v = world.get::<&Vitals>(player).unwrap();
        let fx = world.get::<&StatusEffects>(player).unwrap();
        assert!(v.oxygen > 50.0, "oxygen recovered when sealed (got {})", v.oxygen);
        assert!(
            !fx.has("hypoxia") && !fx.has("suffocation"),
            "oxygen conditions cleared when sealed"
        );
    }

    /// THE DEFECT (2026-09-27): outside at 15 C in still air, in everyday
    /// clothes, walking, a person is comfortable all day. Through the food
    /// system in both modes: the core stays between 36.5 and 37.5 C for 8 h,
    /// nothing is hypothermic, and no health is lost. SEEN RED on 2026-09-27:
    /// the same run on the old drift model (the core moved toward the air at
    /// 0.5 C a second) failed with "hypothermia at 15 C after 4.5 s", and the
    /// constants it ran on would have killed the player about 47 s later.
    #[test]
    fn outside_at_15c_a_walking_person_lasts_hours() {
        use crate::ecs::components::EnvironmentContext;
        use crate::systems::body_heat;
        for mode in [body_heat::Mode::Realistic, body_heat::Mode::Forgiving] {
            let mut sys = FoodSystem::new(data_dir());
            let mut data = make_store();
            data.insert(body_heat::MODE_KEY, mode);
            data.insert(
                "environment_context",
                EnvironmentContext {
                    sealed: false,
                    oxygenated: true,
                    ambient_temp_c: 15.0,
                    relative_humidity: 0.6,
                    wind_m_s: 0.0,
                    activity_met: body_heat::MET_WALKING,
                    ..Default::default()
                },
            );
            let mut world = hecs::World::new();
            let p = world.spawn((Inventory::new(4), vitals(80.0, 80.0), StatusEffects::default(), Health::default()));
            for tick in 0..(8 * 360) {
                sys.tick(&mut world, 10.0, &data);
                let c = world.get::<&Vitals>(p).unwrap().body_temp_c;
                assert!((36.5..37.5).contains(&c), "{mode:?}: core {c} after {} s", (tick + 1) * 10);
            }
            let fx = world.get::<&StatusEffects>(p).unwrap();
            assert!(!fx.has("hypothermia") && !fx.has("shivering"), "{mode:?}: comfortable");
            assert_eq!(world.get::<&Health>(p).unwrap().current, 100.0, "{mode:?}: unharmed");
        }
    }

    /// Waste accrues → the `unsanitary` debuff; Compost turns it into fertilizer and
    /// clears it (the food → waste → compost → fertilizer cycle).
    #[test]
    fn waste_accrues_unsanitary_and_composts_to_fertilizer() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();

        let mut world = hecs::World::new();
        let mut v = vitals(80.0, 80.0);
        v.waste = 80.0; // already above UNSANITARY_THRESHOLD (75)
        let player = world.spawn((Inventory::new(8), v, StatusEffects::default(), Health::default()));

        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&StatusEffects>(player).unwrap().has("unsanitary"),
            "high waste applies the unsanitary debuff"
        );

        // Compost → fertilizer + waste cleared + unsanitary lifts.
        *data
            .get::<std::sync::Mutex<bool>>("compost_request")
            .unwrap()
            .lock()
            .unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        let fert = world.get::<&Inventory>(player).unwrap().count_item("fertilizer_0");
        let waste = world.get::<&Vitals>(player).unwrap().waste;
        assert!(fert >= 3, "compost produced fertilizer (80/25 ≈ 3, got {fert})");
        assert!(waste < 5.0, "compost cleared the waste (got {waste})");
        assert!(
            !world.get::<&StatusEffects>(player).unwrap().has("unsanitary"),
            "composting lifted the unsanitary debuff"
        );
    }

    /// The urine tank (2026-09-26): a living player's urine collects at one
    /// person-day per real day, the Compost action draws off every whole
    /// person-day as one stored urine and leaves the fraction in the tank,
    /// and past 13.3 person-days (a 20 L tank at 1.5 L a day) the rest
    /// overflows to the septic tank. Seen red by dropping the tank's cap in
    /// `collect_urine` (forty days made 40 more, not 13).
    #[test]
    fn urine_collects_by_the_person_day_and_the_tank_overflows_after_two_weeks() {
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        data.insert("vitals_drain_scale", std::sync::Mutex::new(0.0_f32));
        let mut world = hecs::World::new();
        let player = world.spawn((
            Inventory::new(8),
            vitals(80.0, 80.0),
            StatusEffects::default(),
            Health::default(),
            crate::ecs::components::Controllable,
        ));
        let compost = |sys: &mut FoodSystem, world: &mut hecs::World| {
            *data.get::<std::sync::Mutex<bool>>("compost_request").unwrap().lock().unwrap() = true;
            sys.tick(world, 1.0, &data);
            world.get::<&Inventory>(player).unwrap().count_item(URINE_ITEM)
        };

        sys.tick(&mut world, 1.5 * 86_400.0, &data);
        assert_eq!(compost(&mut sys, &mut world), 1, "a day and a half: one whole person-day");
        sys.tick(&mut world, 0.5 * 86_400.0, &data);
        assert_eq!(compost(&mut sys, &mut world), 2, "the half day left in the tank made the second");
        sys.tick(&mut world, 40.0 * 86_400.0, &data);
        assert_eq!(
            compost(&mut sys, &mut world),
            2 + 13,
            "the tank holds 13.3 person-days; the rest of forty went to the septic tank"
        );
    }

    /// A pack with every slot full still gets the empty bottle back
    /// (2026-09-26: it used to be dropped).
    #[test]
    fn a_full_pack_still_gets_the_empty_bottle_back() {
        let mut data = make_store();
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/containers/fluids.ron")).unwrap();
        data.insert("fluid_table", crate::systems::fluids::FluidTable::from_ron(&bytes).unwrap());
        let mut sys = FoodSystem::new(data_dir());
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(2);
        inv.add_item("water_bottle_0", 2, 10);
        inv.add_item("bread_0", 1, 10);
        let player = world.spawn((inv, vitals(80.0, 40.0), StatusEffects::default(), Health::default()));
        *data.get::<std::sync::Mutex<Option<String>>>("drink_request").unwrap().lock().unwrap() =
            Some("water_bottle_0".to_string());
        sys.tick(&mut world, 1.0, &data);
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("water_bottle_0"), 1);
        assert_eq!(inv.count_item("water_bottle_empty_0"), 1, "the empty came back into a new slot");
    }

    /// Drinking a bottle hands the empty bottle back (2026-09-26): the vessel
    /// comes from data/containers/fluids.ron.
    #[test]
    fn drinking_a_bottle_hands_the_empty_bottle_back() {
        let mut data = make_store();
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/containers/fluids.ron")).unwrap();
        data.insert("fluid_table", crate::systems::fluids::FluidTable::from_ron(&bytes).unwrap());
        let mut sys = FoodSystem::new(data_dir());
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item("water_bottle_0", 1, 99);
        let player = world.spawn((inv, vitals(80.0, 40.0), StatusEffects::default(), Health::default()));
        *data.get::<std::sync::Mutex<Option<String>>>("drink_request").unwrap().lock().unwrap() =
            Some("water_bottle_0".to_string());
        sys.tick(&mut world, 1.0, &data);
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!(inv.count_item("water_bottle_0"), 0, "drunk");
        assert_eq!(inv.count_item("water_bottle_empty_0"), 1, "the empty bottle comes back");
        assert!(world.get::<&Vitals>(player).unwrap().hydration > 40.0);
    }

    /// The Eat and Drink buttons ask consume_kinds (2026-09-26): water and
    /// bottles are drunk, bread is eaten, and the water pump, the tester and
    /// an empty bottle are neither.
    #[test]
    fn consume_kinds_says_what_is_drunk_what_is_eaten_and_what_is_neither() {
        let k = consume_kinds();
        assert_eq!(k.get("water_purified_0"), Some(&true));
        assert_eq!(k.get("water_bottle_0"), Some(&true));
        assert_eq!(k.get("bread_0"), Some(&false));
        for id in ["water_pump_0", "water_tester_0", "water_bottle_empty_0", "shampoo_bottle_0", "water_jerrycan_0"] {
            assert!(k.get(id).is_none(), "{id} is not food");
        }
    }

    /// The Drink action consumes a beverage and restores hydration (mirrors Eat).
    #[test]
    fn drinking_restores_hydration() {
        let data = make_store();
        let mut sys = FoodSystem::new(data_dir());
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item("water_purified_0", 1, 99);
        let player = world.spawn((inv, vitals(80.0, 40.0), StatusEffects::default(), Health::default()));

        *data
            .get::<std::sync::Mutex<Option<String>>>("drink_request")
            .unwrap()
            .lock()
            .unwrap() = Some("water_purified_0".to_string());
        sys.tick(&mut world, 1.0, &data);

        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("water_purified_0"),
            0,
            "drinking consumed the water"
        );
        assert!(
            world.get::<&Vitals>(player).unwrap().hydration > 40.0,
            "drinking restored hydration"
        );
    }

    // ── Which items are food: data/food/item_profiles.ron (2026-09-25) ──
    //
    // Each test below failed against the old id-prefix logic (same data files,
    // prefix functions still deciding); see the commit message for the run.

    /// The two columns of items.csv these tests need.
    #[derive(serde::Deserialize)]
    struct ItemRow {
        id: String,
        #[serde(default)]
        category: String,
    }

    fn items_csv() -> Vec<ItemRow> {
        let bytes = std::fs::read(data_dir().join("items.csv")).expect("read data/items.csv");
        crate::assets::loader::parse_csv(&bytes).expect("parse data/items.csv")
    }

    /// A fresh world holding one player who carries a single `item_id`.
    fn player_with(item_id: &str, satiation: f32, hydration: f32) -> (hecs::World, hecs::Entity) {
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item(item_id, 1, 99);
        let player = world.spawn((
            inv,
            vitals(satiation, hydration),
            StatusEffects::default(),
            Health::default(),
        ));
        (world, player)
    }

    fn request(data: &DataStore, channel: &str, item_id: &str) {
        *data
            .get::<std::sync::Mutex<Option<String>>>(channel)
            .unwrap()
            .lock()
            .unwrap() = Some(item_id.to_string());
    }

    /// Every items.csv row filed under category "food" has been decided: it has
    /// a nutrition profile, or item_profiles.ron lists it as not food with a
    /// reason (the medical supplies, trees and flowers items.csv files there).
    /// Also keeps the list honest: every id it names exists in items.csv, none
    /// is listed twice, every profile it names exists and spoils on a real
    /// timescale.
    #[test]
    fn every_food_category_item_has_a_profile_or_a_stated_reason() {
        use std::collections::HashSet;
        let sys = FoodSystem::new(data_dir());
        let list = ItemProfiles::load(data_dir());
        let rows = items_csv();
        let not_food: HashSet<&str> = list.not_food.iter().map(|(id, _)| id.as_str()).collect();

        let undecided: Vec<&str> = rows
            .iter()
            .filter(|r| r.category == "food")
            .map(|r| r.id.as_str())
            .filter(|id| sys.profile_for(id).is_none() && !not_food.contains(id))
            .collect();
        assert!(
            undecided.is_empty(),
            "{} items.csv food items have no nutrition profile and no not_food reason in \
             data/food/item_profiles.ron: {undecided:?}",
            undecided.len()
        );

        let known: HashSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        let mut seen = HashSet::new();
        for (id, _) in list.items.iter().chain(list.not_food.iter()) {
            assert!(seen.insert(id.as_str()), "{id} is listed twice in item_profiles.ron");
            assert!(known.contains(id.as_str()), "item_profiles.ron names {id}, which items.csv does not define");
        }
        for (id, reason) in &list.not_food {
            assert!(!reason.trim().is_empty(), "{id} is listed as not food without a reason");
        }
        for (id, profile_id) in &list.items {
            let p = sys.profile_for(id).unwrap_or_else(|| {
                panic!("{id} uses profile '{profile_id}', which food_system.ron does not define")
            });
            assert!(p.spoilage_rate_hours > 0.0, "profile '{}' has no spoilage time", p.id);
        }
    }

    /// Everything the cooking recipes make is either food with a profile or
    /// listed as not food (the spice mix). Before the list, 18 of the 26
    /// cooking outputs (stew, cake, pie, omelette, juice...) did nothing when eaten.
    #[test]
    fn every_cooking_recipe_output_is_decided() {
        #[derive(serde::Deserialize)]
        struct RecipeRow {
            #[serde(default)]
            category: String,
            #[serde(default)]
            outputs: String,
        }
        let sys = FoodSystem::new(data_dir());
        let list = ItemProfiles::load(data_dir());
        let bytes = std::fs::read(data_dir().join("recipes.csv")).expect("read data/recipes.csv");
        let recipes: Vec<RecipeRow> = crate::assets::loader::parse_csv(&bytes).expect("parse recipes.csv");
        let mut undecided = Vec::new();
        for r in recipes.iter().filter(|r| r.category == "cooking") {
            for out in r.outputs.split('|') {
                let id = out.split(':').next().unwrap_or("").trim();
                if id.is_empty() {
                    continue;
                }
                if sys.profile_for(id).is_none() && !list.not_food.iter().any(|(nf, _)| nf == id) {
                    undecided.push(id.to_string());
                }
            }
        }
        assert!(undecided.is_empty(), "cooking recipes make undecided items: {undecided:?}");
    }

    /// The grain mill and the grain silo are machines. The old prefix rule
    /// ("grain_") made both edible and tracked them for spoilage.
    #[test]
    fn grain_mill_and_grain_silo_are_not_food() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item("grain_mill_0", 1, 1);
        inv.add_item("grain_silo_0", 1, 1);
        let player = world.spawn((inv, vitals(40.0, 50.0), StatusEffects::default(), Health::default()));

        for machine in ["grain_mill_0", "grain_silo_0"] {
            request(&data, "consume_request", machine);
            sys.tick(&mut world, 0.0, &data);
            assert_eq!(
                world.get::<&Inventory>(player).unwrap().count_item(machine),
                1,
                "{machine} was eaten"
            );
        }
        assert_eq!(world.get::<&Vitals>(player).unwrap().satiation, 40.0, "a machine fed the player");
        assert!(sys.profile_for("grain_mill_0").is_none() && sys.profile_for("grain_silo_0").is_none());
        assert!(
            sys.spoilage.is_empty(),
            "machines are being tracked for spoilage ({} slots)",
            sys.spoilage.len()
        );
    }

    /// A cooked dish a recipe makes actually feeds you: the meat stew raises
    /// satiation by exactly its calories (107 kcal/100 g, USDA FNDDS "Stew,
    /// beef") times SATIATION_PER_CALORIE, and is used up.
    #[test]
    fn eating_the_meat_stew_raises_satiation() {
        let data = make_store();
        let mut sys = FoodSystem::new(data_dir());
        let (mut world, player) = player_with("stew_meat_0", 40.0, 50.0);
        request(&data, "consume_request", "stew_meat_0");
        sys.tick(&mut world, 0.0, &data);

        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("stew_meat_0"),
            0,
            "the stew was eaten"
        );
        let satiation = world.get::<&Vitals>(player).unwrap().satiation;
        assert!(satiation > 40.0, "eating the stew raised satiation (40 -> {satiation})");
        let kcal = sys.profile_for("stew_meat_0").expect("stew has a profile").calories_per_100g as f32;
        assert!(
            (satiation - (40.0 + kcal * SATIATION_PER_CALORIE)).abs() < 1e-3,
            "stew gave {} satiation, expected {kcal} kcal x {SATIATION_PER_CALORIE}",
            satiation - 40.0
        );
    }

    /// Every item on the list can really be consumed through the button the
    /// GUI shows for it, and does something: Drink for beverages, Eat otherwise.
    #[test]
    fn every_listed_item_can_be_eaten_or_drunk() {
        let list = ItemProfiles::load(data_dir());
        assert!(!list.items.is_empty(), "item_profiles.ron loaded no items");
        for (id, _) in &list.items {
            let data = make_store();
            let mut sys = FoodSystem::new(data_dir());
            let beverage = sys.profile_for(id).is_some_and(|p| p.category == "beverage");
            let (mut world, player) = player_with(id, 40.0, 40.0);
            request(&data, if beverage { "drink_request" } else { "consume_request" }, id);
            sys.tick(&mut world, 0.0, &data);
            let v = world.get::<&Vitals>(player).unwrap();
            assert_eq!(world.get::<&Inventory>(player).unwrap().count_item(id), 0, "{id} was not consumed");
            assert!(
                v.satiation > 40.0 || v.hydration > 40.0,
                "{id} was consumed but did nothing (satiation {}, hydration {})",
                v.satiation,
                v.hydration
            );
        }
    }

    /// Legumes spoil on their own profile's clock. Green peas (legumes_fresh)
    /// go off in days; dry beans (legumes_dry) sitting in the same pack for
    /// the same time are still fine. The old prefix rule never tracked legumes
    /// at all, so neither ever spoiled.
    #[test]
    fn a_fresh_legume_spoils_on_its_profile_timescale_and_a_dry_one_keeps() {
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        // Pause hunger and thirst: this test is about the spoilage clock only.
        data.insert("vitals_drain_scale", std::sync::Mutex::new(0.0_f32));
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(4);
        inv.slots[0] = Some(crate::systems::inventory::ItemStack::new("legume_pea_0".to_string(), 1, 99));
        inv.slots[1] = Some(crate::systems::inventory::ItemStack::new("legume_bean_0".to_string(), 1, 99));
        let player = world.spawn((inv, vitals(80.0, 80.0), StatusEffects::default(), Health::default()));
        let bits: u64 = player.to_bits().into();

        // A zero-length tick registers both stacks with the spoilage pass.
        sys.tick(&mut world, 0.0, &data);
        let pea = sys.spoilage.get(&(bits, 0)).expect("green peas are not tracked for spoilage").clone();
        let bean = sys.spoilage.get(&(bits, 1)).expect("dry beans are not tracked for spoilage").clone();

        // The clocks are the profiles' own, and they are the right scale:
        // green peas keep days (FoodKeeper 3-5), dry beans a year or more.
        let hours = |id: &str| sys.profile_for(id).map(|p| p.spoilage_rate_hours).unwrap();
        assert_eq!(pea.max_freshness, hours("legume_pea_0") * 3600.0);
        assert_eq!(bean.max_freshness, hours("legume_bean_0") * 3600.0);
        assert!(hours("legume_pea_0") <= 7.0 * 24.0, "green peas keep {} h", hours("legume_pea_0"));
        assert!(hours("legume_bean_0") >= 180.0 * 24.0, "dry beans keep {} h", hours("legume_bean_0"));

        // One minute short of the pea's limit: still fresh.
        sys.tick(&mut world, pea.max_freshness - 60.0, &data);
        assert!(!sys.spoilage[&(bits, 0)].spoiled, "peas spoiled before their time");
        // Two minutes later: spoiled. The dry beans beside them are fine.
        sys.tick(&mut world, 120.0, &data);
        assert!(sys.spoilage[&(bits, 0)].spoiled, "peas never spoiled");
        assert!(!sys.spoilage[&(bits, 1)].spoiled, "dry beans spoiled on the green-pea clock");
    }

    /// Drink takes only beverages. The GUI offers Drink for every item whose
    /// id starts with `water_` (pump, tank, purifier...), and the old handler
    /// consumed whatever arrived and added hydration, so a water pump could
    /// be drunk. Bread is food but not a drink. Juice is both a drink and a
    /// little food.
    #[test]
    fn drink_accepts_only_beverages() {
        let data = make_store();
        let mut sys = FoodSystem::new(data_dir());
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        for id in ["water_pump_0", "bread_0", "juice_0"] {
            inv.add_item(id, 1, 99);
        }
        let player = world.spawn((inv, vitals(40.0, 40.0), StatusEffects::default(), Health::default()));

        for refused in ["water_pump_0", "bread_0"] {
            request(&data, "drink_request", refused);
            sys.tick(&mut world, 0.0, &data);
            assert_eq!(
                world.get::<&Inventory>(player).unwrap().count_item(refused),
                1,
                "{refused} was drunk"
            );
        }
        {
            let v = world.get::<&Vitals>(player).unwrap();
            assert_eq!((v.satiation, v.hydration), (40.0, 40.0), "a refused drink changed vitals");
        }

        request(&data, "drink_request", "juice_0");
        sys.tick(&mut world, 0.0, &data);
        let v = world.get::<&Vitals>(player).unwrap();
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("juice_0"), 0);
        assert!(v.hydration > 40.0, "juice hydrates");
        assert!(v.satiation > 40.0, "juice carries its sugar's calories");
    }
}

#[cfg(test)]
mod g_load_wiring_tests {
    use super::*;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::status_effects::StatusEffectRegistry;

    // Local copies rather than widening nutrition_tests' visibility: these
    // tests are about the g-load wiring and should not make another module's
    // internals public just to borrow a fixture.
    fn data_dir() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
    }

    fn make_store() -> DataStore {
        let mut data = DataStore::new();
        let reg = StatusEffectRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/status_effects.csv"
        )))
        .expect("status_effects.csv");
        data.insert("status_effect_registry", reg);
        data.insert("consume_request", std::sync::Mutex::new(Option::<String>::None));
        data.insert("rest_request", std::sync::Mutex::new(false));
        data.insert("compost_request", std::sync::Mutex::new(false));
        data.insert("drink_request", std::sync::Mutex::new(Option::<String>::None));
        data.insert("player_death", std::sync::Mutex::new(Option::<String>::None));
        data
    }

    fn vitals(satiation: f32, hydration: f32) -> Vitals {
        Vitals {
            satiation,
            hydration,
            energy: 100.0,
            oxygen: 100.0,
            body_temp_c: 37.0,
            waste: 0.0,
            satiation_max: 100.0,
            hydration_max: 100.0,
            energy_max: 100.0,
            oxygen_max: 100.0,
            waste_max: 100.0,
        }
    }
    use crate::ecs::components::{EnvironmentContext, Health, StatusEffects, Vitals};
    use crate::systems::flight::{felt_gravity, FlightData, FlightState};

    fn flight_data() -> FlightData {
        FlightData::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data"))
    }

    /// Put the player in an environment at a given felt gravity and run one
    /// tick. This drives the REAL FoodSystem through the REAL DataStore slots,
    /// so it proves the wiring rather than re-testing the arithmetic.
    fn health_lost_at(g: f32, secs: f32) -> (f32, bool) {
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        data.insert("flight_data", flight_data());
        data.insert(
            "environment_context",
            EnvironmentContext { g_load: g, ..Default::default() },
        );
        let mut world = hecs::World::new();
        let e = world.spawn((
            vitals(80.0, 80.0),
            StatusEffects::default(),
            Health { current: 100.0, max: 100.0 },
        ));
        sys.tick(&mut world, secs, &data);
        let health = world.get::<&Health>(e).unwrap().current;
        let flagged = world.get::<&StatusEffects>(e).unwrap().has("high_g");
        (100.0 - health, flagged)
    }

    /// Ordinary cruise must be completely silent. If this ever fails, living
    /// aboard a moving ship has become a slow death.
    #[test]
    fn cruise_gravity_costs_the_crew_nothing() {
        let (lost, flagged) = health_lost_at(1.005, 60.0);
        assert!(lost.abs() < 1e-4, "lost {lost} health at cruise");
        assert!(!flagged, "high_g must not be flagged at cruise");
    }

    /// Weightlessness is not an injury. Its real costs are bone and muscle over
    /// months, which belong in a medical model, not a per-second drain.
    #[test]
    fn zero_g_is_not_harmful() {
        let (lost, flagged) = health_lost_at(0.0, 60.0);
        assert!(lost.abs() < 1e-4, "lost {lost} health in free fall");
        assert!(!flagged);
    }

    /// The operator's scenario, end to end through the real system: a sustained
    /// evasion burn hurts, flags the condition, and does so at a rate that
    /// kills in well under a minute.
    #[test]
    fn an_evasion_burn_injures_and_then_kills_the_crew() {
        let burn = FlightState { thrust_g: 5.0, spin_g: 1.0, dampeners_online: true };
        let felt = felt_gravity(&burn, &flight_data().dampener).felt_g;
        assert!(felt > 4.9, "dampeners must not rescue a 5 g burn; felt {felt}");

        let (lost, flagged) = health_lost_at(felt, 1.0);
        assert!(lost > 0.0, "a 5 g burn must hurt");
        assert!(flagged, "the high_g condition must be visible to the player");

        let seconds_to_die = 100.0 / lost;
        assert!(
            (10.0..=60.0).contains(&seconds_to_die),
            "death in {seconds_to_die:.0}s is outside the band that reads as \
             'get to a couch NOW' rather than instant or ignorable"
        );
    }

    /// The condition must CLEAR when the burn eases, or one hard burn would
    /// leave the player permanently debuffed.
    #[test]
    fn the_condition_clears_once_the_burn_eases() {
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        data.insert("flight_data", flight_data());
        data.insert(
            "environment_context",
            EnvironmentContext { g_load: 5.0, ..Default::default() },
        );
        let mut world = hecs::World::new();
        let e = world.spawn((
            vitals(80.0, 80.0),
            StatusEffects::default(),
            Health { current: 100.0, max: 100.0 },
        ));
        sys.tick(&mut world, 0.5, &data);
        assert!(world.get::<&StatusEffects>(e).unwrap().has("high_g"), "burn should flag");

        data.insert(
            "environment_context",
            EnvironmentContext { g_load: 1.0, ..Default::default() },
        );
        sys.tick(&mut world, 0.5, &data);
        assert!(
            !world.get::<&StatusEffects>(e).unwrap().has("high_g"),
            "the condition must clear once the burn eases"
        );
    }

    /// Fail-safe: with no flight data loaded, nobody is ever harmed. The loader
    /// degrades to empty on a missing or malformed file, and this pins that a
    /// degraded load cannot start crushing people.
    #[test]
    fn absent_flight_data_harms_nobody_even_at_lethal_g() {
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        // deliberately NO "flight_data" slot
        data.insert(
            "environment_context",
            EnvironmentContext { g_load: 50.0, ..Default::default() },
        );
        let mut world = hecs::World::new();
        let e = world.spawn((
            vitals(80.0, 80.0),
            StatusEffects::default(),
            Health { current: 100.0, max: 100.0 },
        ));
        sys.tick(&mut world, 5.0, &data);
        assert_eq!(
            world.get::<&Health>(e).unwrap().current,
            100.0,
            "a missing flight.ron must never damage the player"
        );
    }
}
