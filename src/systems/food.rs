//! Food system -- nutrition, spoilage, cooking, and meal quality.
//!
//! Loads nutrition profiles, preservation methods, cooking methods, meal quality
//! levels, and temperature zones from `data/food_system.ron`.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// One nutrition profile from `data/food_system.ron` (by food category). Only the
/// fields the nutrition loop consumes are modeled; the rest (macros, vitamins,
/// minerals, spoilage_rate_hours, description) are ignored by serde.
#[derive(Debug, Clone, Deserialize)]
pub struct NutritionProfile {
    /// Profile id, e.g. `fruit`, `raw_vegetables`, `cooked_meat`.
    pub id: String,
    /// Macro category: `protein`, `produce`, `staple`, `preserved`.
    #[serde(default)]
    pub category: String,
    /// Energy density (kcal / 100 g) — drives satiation gain.
    #[serde(default)]
    pub calories_per_100g: u32,
    /// Probability (0..1) of illness when eaten raw. 0 for cooked/preserved food.
    #[serde(default)]
    pub raw_consumption_risk: f32,
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

// TODO: Add a FoodSpoilage component (or item metadata) to ecs/components.rs:
//   pub struct FoodSpoilage { pub spoilage_timer: f32, pub max_freshness: f32, pub spoiled: bool }
// Then items in an Inventory could carry spoilage state directly.

/// Spoilage time limits (seconds) by item category.
/// Categories are inferred from item_id prefixes until a proper item-metadata system exists.
const RAW_MEAT_FRESHNESS: f32 = 86_400.0;    // 24 hours
const RAW_PRODUCE_FRESHNESS: f32 = 172_800.0; // 48 hours
const COOKED_FOOD_FRESHNESS: f32 = 43_200.0;  // 12 hours
const DEFAULT_FRESHNESS: f32 = 259_200.0;     // 72 hours

/// Preservation multipliers applied to base freshness time.
const REFRIGERATED_MULT: f32 = 4.0;
const CANNED_MULT: f32 = 100.0;

// ── Nutrition tuning (real-time seconds; dev values, tunable — could move to
//    food_system.ron config later). Vitals run 0..100. ──────────────────────
/// Satiation lost per real second (full -> hungry threshold in ~12 min).
const SATIATION_DECAY_PER_SEC: f32 = 0.10;
/// Hydration lost per real second (slightly faster than hunger; ~10 min).
const HYDRATION_DECAY_PER_SEC: f32 = 0.13;
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
/// Health drained per second while fully starved / dehydrated (dehydration
/// kills roughly twice as fast, matching the `thirsty` effect's data note).
const STARVE_DAMAGE_PER_SEC: f32 = 1.0;
const DEHYDRATE_DAMAGE_PER_SEC: f32 = 2.0;
/// Conditions (hungry/thirsty) are refreshed to this many seconds each tick
/// while their trigger holds, so they linger briefly then fade once you recover.
const CONDITION_LINGER: f32 = 3.0;
/// Fallback durations (seconds) if status_effects.csv isn't loaded.
const FALLBACK_WELL_FED_S: f32 = 1800.0;
const FALLBACK_FOOD_POISONING_S: f32 = 5400.0;

/// Unique key for tracking a specific food stack: (entity bits, inventory slot index).
type FoodKey = (u64, usize);

/// Per-item spoilage tracking.
#[derive(Debug, Clone)]
struct SpoilageState {
    spoilage_timer: f32,
    max_freshness: f32,
    spoiled: bool,
}

/// Tracks nutrition, spoilage, and cooking.
pub struct FoodSystem {
    pub data: FoodData,
    /// Per-item spoilage timers, keyed by (entity_id, slot_index).
    spoilage: HashMap<FoodKey, SpoilageState>,
    /// Accumulator to throttle log spam.
    log_cooldown: f32,
}

impl FoodSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("food_system.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(nutrition_profiles:[],preservation_methods:[],cooking_methods:[],meal_quality_levels:[],temperature_zones:[])".to_string()
        });
        let data: FoodData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse food_system.ron: {e}");
            FoodData { nutrition_profiles: vec![], preservation_methods: vec![], cooking_methods: vec![], meal_quality_levels: vec![], temperature_zones: vec![] }
        });
        log::info!("Loaded food data: {} nutrition profiles, {} cooking methods", data.nutrition_profiles.len(), data.cooking_methods.len());
        Self { data, spoilage: HashMap::new(), log_cooldown: 0.0 }
    }

    /// Determine base freshness (seconds) for an item based on its ID prefix.
    fn base_freshness(item_id: &str) -> f32 {
        if item_id.starts_with("raw_meat") || item_id.starts_with("fish_raw") {
            RAW_MEAT_FRESHNESS
        } else if item_id.starts_with("fruit_") || item_id.starts_with("vegetable_") {
            RAW_PRODUCE_FRESHNESS
        } else if item_id.starts_with("cooked_") || item_id.starts_with("meal_") {
            COOKED_FOOD_FRESHNESS
        } else {
            DEFAULT_FRESHNESS
        }
    }

    /// Determine preservation multiplier from item ID suffix conventions.
    fn preservation_multiplier(item_id: &str) -> f32 {
        if item_id.contains("_canned") {
            CANNED_MULT
        } else if item_id.contains("_refrigerated") || item_id.contains("_chilled") {
            REFRIGERATED_MULT
        } else {
            1.0
        }
    }

    /// Check whether an item_id represents food (vs tools, ores, etc.).
    fn is_food(item_id: &str) -> bool {
        let food_prefixes = [
            "raw_meat", "fish_raw", "fruit_", "vegetable_", "cooked_",
            "meal_", "bread_", "grain_", "food_", "berry_", "herb_",
        ];
        food_prefixes.iter().any(|p| item_id.starts_with(p))
    }

    /// Classify a food item to its nutrition-profile id. Mirrors the prefix
    /// classification already used for spoilage. (A fully data-driven
    /// item->profile link — e.g. a column on items.csv — is a tracked future
    /// refinement; see docs/design/gameplay-loops.md.)
    fn profile_id_for(item_id: &str) -> Option<&'static str> {
        let id = item_id;
        // Cooked/specific prefixes first, then the generic raw families.
        if id.starts_with("cooked_meat") || id.starts_with("meat_cooked") {
            Some("cooked_meat")
        } else if id.starts_with("cooked_veg") || id.starts_with("vegetable_cooked") {
            Some("cooked_vegetables")
        } else if id.starts_with("raw_meat") || id.starts_with("meat_") {
            Some("raw_meat")
        } else if id.starts_with("fish_") || id.starts_with("raw_fish") {
            Some("raw_fish")
        } else if id.starts_with("fruit_") || id.starts_with("berry_") {
            Some("fruit")
        } else if id.starts_with("vegetable_") || id.starts_with("veg_") {
            Some("raw_vegetables")
        } else if id.starts_with("egg_") {
            Some("eggs")
        } else if id.starts_with("milk_") || id.starts_with("cheese_") || id.starts_with("dairy_") {
            Some("dairy")
        } else if id.starts_with("canned_") || id.starts_with("mre_") {
            Some("canned_food")
        } else if id.starts_with("jerky_") || id.starts_with("dried_") {
            Some("dried_food")
        } else if id.starts_with("frozen_") {
            Some("frozen_food")
        } else if id.starts_with("bread_")
            || id.starts_with("grain_")
            || id.starts_with("flour_")
            || id.starts_with("protein_bar")
            || id.starts_with("rice_")
            || id.starts_with("cereal_")
        {
            Some("grains")
        } else {
            None
        }
    }

    /// Resolve an item to its loaded nutrition profile, if it is food.
    fn profile_for(&self, item_id: &str) -> Option<&NutritionProfile> {
        let pid = Self::profile_id_for(item_id)?;
        self.data.nutrition_profiles.iter().find(|p| p.id == pid)
    }
}

impl System for FoodSystem {
    fn name(&self) -> &str {
        "FoodSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        use crate::ecs::components::{Health, Name, StatusEffects, Vitals};
        use crate::systems::inventory::Inventory;
        use crate::systems::status_effects::StatusEffectRegistry;

        // ── 1. EAT: drain the consume_request channel (the Eat button writes it
        //    via the main-loop bridge) and apply the food's nutrition to the first
        //    player (Inventory + Vitals + StatusEffects) that actually has the item.
        let registry = data.get::<StatusEffectRegistry>("status_effect_registry");
        let consumed = data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(item_id) = consumed {
            // Resolve nutrition once (immutable self borrow) and copy out the scalars,
            // so no self borrow is held across the &mut World pass below.
            if let Some((calories, risk, is_produce)) = self.profile_for(&item_id).map(|p| {
                (
                    p.calories_per_100g as f32,
                    p.raw_consumption_risk,
                    p.category == "produce",
                )
            }) {
                let poison_dur = registry
                    .map(|r| r.duration("food_poisoning"))
                    .filter(|d| *d > 0.0)
                    .unwrap_or(FALLBACK_FOOD_POISONING_S);
                let well_fed_dur = registry
                    .map(|r| r.duration("well_fed"))
                    .filter(|d| *d > 0.0)
                    .unwrap_or(FALLBACK_WELL_FED_S);
                let nourished_dur = registry
                    .map(|r| r.duration("well_nourished"))
                    .filter(|d| *d > 0.0)
                    .unwrap_or(FALLBACK_WELL_FED_S);
                for (_e, (inv, vitals, effects)) in
                    world.query_mut::<(&mut Inventory, &mut Vitals, &mut StatusEffects)>()
                {
                    if !inv.has_item(&item_id, 1) {
                        continue;
                    }
                    inv.remove_item(&item_id, 1);
                    vitals.satiation = (vitals.satiation + calories * SATIATION_PER_CALORIE)
                        .min(vitals.satiation_max);
                    let hydration_gain = if is_produce { PRODUCE_HYDRATION } else { BASE_HYDRATION };
                    vitals.hydration = (vitals.hydration + hydration_gain).min(vitals.hydration_max);
                    // Eating raw food risks illness; cooked/preserved food is safe.
                    if risk > 0.0 && rand::random::<f32>() < risk {
                        effects.apply("food_poisoning", poison_dur);
                        log::info!("[Food] {item_id} eaten raw -> food poisoning!");
                    }
                    // A satisfying meal grants well_fed (stamina regen) + well_nourished
                    // (a tangible +10% move speed via the camera speed_multiplier).
                    if vitals.satiation >= WELL_FED_THRESHOLD {
                        effects.apply("well_fed", well_fed_dur);
                        effects.apply("well_nourished", nourished_dur);
                    }
                    log::info!(
                        "[Food] ate {item_id}: satiation {:.0}/{:.0}, hydration {:.0}/{:.0}",
                        vitals.satiation,
                        vitals.satiation_max,
                        vitals.hydration,
                        vitals.hydration_max,
                    );
                    break; // first player only
                }
            } else {
                log::debug!("[Food] consume_request for non-food item {item_id} ignored");
            }
        }

        // ── 2. DECAY + CONDITIONS: every entity with Vitals gets hungrier/thirstier;
        //    low levels apply the hungry/thirsty conditions; empty levels drain Health;
        //    timed buffs/debuffs count down and expire.
        for (_e, (vitals, effects, health)) in
            world.query_mut::<(&mut Vitals, &mut StatusEffects, Option<&mut Health>)>()
        {
            vitals.satiation = (vitals.satiation - SATIATION_DECAY_PER_SEC * dt).max(0.0);
            vitals.hydration = (vitals.hydration - HYDRATION_DECAY_PER_SEC * dt).max(0.0);

            // Conditions: refreshed each tick while the trigger holds, else cleared.
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

            // Starvation / dehydration damage at zero.
            if let Some(health) = health {
                if vitals.satiation <= 0.0 {
                    health.current = (health.current - STARVE_DAMAGE_PER_SEC * dt).max(0.0);
                }
                if vitals.hydration <= 0.0 {
                    health.current = (health.current - DEHYDRATE_DAMAGE_PER_SEC * dt).max(0.0);
                }
            }

            // Expire timed effects (conditions were just refreshed, so they survive dt).
            effects.tick(dt);
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

                if !Self::is_food(&stack.item_id) {
                    continue;
                }

                let key: FoodKey = (entity_bits, slot_idx);
                active_keys.insert(key);

                let max_freshness =
                    Self::base_freshness(&stack.item_id) * Self::preservation_multiplier(&stack.item_id);

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
                state.spoilage_timer += dt;

                if state.spoilage_timer >= state.max_freshness {
                    state.spoiled = true;
                    log::info!(
                        "[Food] {owner}'s {} (slot {slot_idx}) has spoiled after {:.0}s",
                        stack.item_id, state.spoilage_timer,
                    );
                    // TODO: Replace item with "spoiled_food" variant or reduce nutrition value.
                    // For now, just mark it in the spoilage map.
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
        data
    }

    fn vitals(satiation: f32, hydration: f32) -> Vitals {
        Vitals {
            satiation,
            hydration,
            satiation_max: 100.0,
            hydration_max: 100.0,
        }
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
}
