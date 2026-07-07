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

// Spoilage state is tracked in FoodSystem's own `spoilage: HashMap<FoodKey, ..>`
// side-table (below) rather than as an ECS component on the item -- items are
// plain data (item_id + quantity) with no per-instance component slot of their
// own, so keying by (entity, inventory-slot-index) is the practical way to
// attach per-stack state without an item-entity architecture change.

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
/// Hydration restored per drink consumed (water/juice/etc. via the Drink action).
const DRINK_HYDRATION: f32 = 30.0;
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
const FALLBACK_RESTED_S: f32 = 3600.0;
/// Energy lost per real second while awake (full -> fatigued threshold in ~20 min).
const ENERGY_DECAY_PER_SEC: f32 = 0.06;
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
/// Body temperature moves this many °C/sec toward its target (37 sealed, else ambient).
const BODY_TEMP_RATE: f32 = 0.5;
/// Core temp below this = hypothermia; above HEAT_EXHAUSTION_C = heat exhaustion.
const HYPOTHERMIA_C: f32 = 35.0;
const HEAT_EXHAUSTION_C: f32 = 39.0;
const TEMP_DAMAGE_PER_SEC: f32 = 2.0;
// ── Sanitation (organic waste → compost → fertilizer). ──
/// Waste accrued per real second while living, + per meal eaten.
const WASTE_RISE_PER_SEC: f32 = 0.05;
const WASTE_PER_MEAL: f32 = 4.0;
/// Above this waste the `unsanitary` debuff applies (compost to clear it).
const UNSANITARY_THRESHOLD: f32 = 75.0;
/// Waste units consumed per unit of fertilizer produced when composting.
const WASTE_PER_FERTILIZER: f32 = 25.0;

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
        use crate::systems::inventory::{Inventory, ItemRegistry};
        use crate::systems::status_effects::StatusEffectRegistry;

        // ── 1. EAT: drain the consume_request channel (the Eat button writes it
        //    via the main-loop bridge) and apply the food's nutrition to the first
        //    player (Inventory + Vitals + StatusEffects) that actually has the item.
        let registry = data.get::<StatusEffectRegistry>("status_effect_registry");
        let item_registry = data.get::<ItemRegistry>("item_registry");
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
                for (e, (inv, vitals, effects)) in
                    world.query_mut::<(&mut Inventory, &mut Vitals, &mut StatusEffects)>()
                {
                    if !inv.has_item(&item_id, 1) {
                        continue;
                    }
                    // Spoiled food (tracked by the spoilage pass below, §3) nourishes
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

                    inv.remove_item(&item_id, 1);
                    let nutrition_mult = if is_spoiled { 0.25 } else { 1.0 };
                    vitals.satiation = (vitals.satiation + calories * SATIATION_PER_CALORIE * nutrition_mult)
                        .min(vitals.satiation_max);
                    let hydration_gain = if is_produce { PRODUCE_HYDRATION } else { BASE_HYDRATION };
                    vitals.hydration =
                        (vitals.hydration + hydration_gain * nutrition_mult).min(vitals.hydration_max);
                    // Eating produces a little organic waste (scraps) to compost later.
                    vitals.waste = (vitals.waste + WASTE_PER_MEAL).min(vitals.waste_max);
                    // Spoiled food always poisons; otherwise raw food risks illness while
                    // cooked/preserved food (risk 0) is safe.
                    if is_spoiled || (risk > 0.0 && rand::random::<f32>() < risk) {
                        effects.apply("food_poisoning", poison_dur);
                        log::info!(
                            "[Food] {item_id} eaten {} -> food poisoning!",
                            if is_spoiled { "spoiled" } else { "raw" }
                        );
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

        // ── 1a. DRINK: drain drink_request -> consume a drink item -> restore hydration
        //    (mirrors EAT for beverages, which carry no nutrition profile).
        let drank = data
            .get::<std::sync::Mutex<Option<String>>>("drink_request")
            .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
        if let Some(item_id) = drank {
            for (_e, (inv, vitals)) in world.query_mut::<(&mut Inventory, &mut Vitals)>() {
                if inv.has_item(&item_id, 1) {
                    inv.remove_item(&item_id, 1);
                    vitals.hydration =
                        (vitals.hydration + DRINK_HYDRATION).min(vitals.hydration_max);
                    log::info!("[Food] drank {item_id}: hydration {:.0}", vitals.hydration);
                }
                break; // first player only
            }
        }

        // ── 1b. REST: drain the rest_request channel (the Rest button) -> refill the
        //    player's energy, clear fatigue, grant the `rested` buff.
        let do_rest = data
            .get::<std::sync::Mutex<bool>>("rest_request")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if do_rest {
            let rested_dur = registry
                .map(|r| r.duration("rested"))
                .filter(|d| *d > 0.0)
                .unwrap_or(FALLBACK_RESTED_S);
            for (_e, (vitals, effects)) in world.query_mut::<(&mut Vitals, &mut StatusEffects)>() {
                vitals.energy = vitals.energy_max;
                effects.remove("fatigued");
                effects.apply("rested", rested_dur);
                log::info!("[Survival] player rested -> energy restored to full");
                break; // first player only
            }
        }

        // Player environment context (sealed / oxygenated / ambient temp) for the
        // oxygen + body-temperature vitals — computed in the main loop from the
        // player's position vs the sealed homestead volume. Absent = safe defaults.
        let (env_sealed, env_oxygenated, env_ambient_c) = data
            .get::<crate::ecs::components::EnvironmentContext>("environment_context")
            .map(|e| (e.sealed, e.oxygenated, e.ambient_temp_c))
            .unwrap_or((true, true, 21.0));

        // ── 1c. COMPOST: drain compost_request -> turn the player's accumulated waste
        //    into fertilizer items (the food -> waste -> compost -> soil cycle) + clear it.
        let do_compost = data
            .get::<std::sync::Mutex<bool>>("compost_request")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
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
        // Gear temperature resists (v0.750, ladder rung 8): worn equipment's
        // cold_resist/heat_resist scale the temperature health drain — an
        // insulated coat halves freezing damage. Same stat grammar as buffs.
        let equipment = data
            .get::<crate::systems::economy::EquipmentRegistry>("equipment_registry");
        for (e, (vitals, effects, health, ctrl, dead, outfit)) in world.query_mut::<(
            &mut Vitals,
            &mut StatusEffects,
            Option<&mut Health>,
            Option<&crate::ecs::components::Controllable>,
            Option<&crate::ecs::components::Dead>,
            Option<&crate::ecs::components::Outfit>,
        )>() {
            let (cold_resist, heat_resist) = match (equipment, outfit) {
                (Some(reg), Some(o)) => (
                    reg.stat_add_total(o.equipped.values().map(|s| s.as_str()), "cold_resist")
                        .clamp(0.0, 0.9),
                    reg.stat_add_total(o.equipped.values().map(|s| s.as_str()), "heat_resist")
                        .clamp(0.0, 0.9),
                ),
                _ => (0.0, 0.0),
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

            vitals.satiation = (vitals.satiation - SATIATION_DECAY_PER_SEC * dt).max(0.0);
            vitals.hydration = (vitals.hydration - HYDRATION_DECAY_PER_SEC * dt).max(0.0);
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
                let amt = STARVE_DAMAGE_PER_SEC * dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("starvation", amt);
                }
            }
            if vitals.hydration <= 0.0 {
                let amt = DEHYDRATE_DAMAGE_PER_SEC * dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("dehydration", amt);
                }
            }

            // Energy drains while awake; low energy -> fatigued (speed debuff, #3b).
            vitals.energy = (vitals.energy - ENERGY_DECAY_PER_SEC * dt).max(0.0);
            if vitals.energy < FATIGUED_THRESHOLD {
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

            // Body temperature drifts toward 37 °C when sealed, toward ambient when
            // exposed; far from baseline -> hypothermia / heat exhaustion + damage.
            let temp_target = if env_sealed { 37.0 } else { env_ambient_c };
            let diff = temp_target - vitals.body_temp_c;
            let step = (BODY_TEMP_RATE * dt).min(diff.abs());
            vitals.body_temp_c += step * diff.signum();
            if vitals.body_temp_c < HYPOTHERMIA_C {
                effects.remove("heat_exhaustion");
                effects.apply("hypothermia", CONDITION_LINGER);
                let amt = TEMP_DAMAGE_PER_SEC * (1.0 - cold_resist) * dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("freezing", amt);
                }
            } else if vitals.body_temp_c > HEAT_EXHAUSTION_C {
                effects.remove("hypothermia");
                effects.apply("heat_exhaustion", CONDITION_LINGER);
                let amt = TEMP_DAMAGE_PER_SEC * (1.0 - heat_resist) * dt;
                health_drain += amt;
                if amt > worst.1 {
                    worst = ("heat exhaustion", amt);
                }
            } else {
                effects.remove("hypothermia");
                effects.remove("heat_exhaustion");
            }

            // Organic waste accrues while living; high waste -> the unsanitary debuff.
            vitals.waste = (vitals.waste + WASTE_RISE_PER_SEC * dt).min(vitals.waste_max);
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
                    // The item itself stays as-is (no item-def swap, so it still
                    // stacks/sells as the same item_id); the EAT handler above (§1)
                    // looks up this slot's spoiled flag and applies the real
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

    /// v0.750 GEAR RESISTS (ladder rung 8): a winter coat (cold_resist 0.6)
    /// takes 60 percent off the freezing health drain — clothing is survival
    /// equipment now, not a cosmetic.
    #[test]
    fn winter_coat_reduces_freezing_damage() {
        use crate::ecs::components::Outfit;
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        let equip = crate::systems::economy::EquipmentRegistry::from_csv(include_bytes!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/data/equipment.csv")
        ))
        .expect("equipment.csv");
        data.insert("equipment_registry", equip);
        // Exposed to hard cold: unsealed environment at -20 C.
        data.insert(
            "environment_context",
            std::sync::Mutex::new(crate::ecs::components::EnvironmentContext {
                sealed: false,
                oxygenated: true,
                ambient_temp_c: -20.0,
            }),
        );

        let mut world = hecs::World::new();
        let mut frozen = vitals(80.0, 80.0);
        frozen.body_temp_c = 20.0; // already hypothermic
        let bare = world.spawn((Inventory::new(4), frozen.clone(), StatusEffects::default(), Health::default()));
        let mut coat_outfit = Outfit::default();
        coat_outfit.equipped.insert("chest".to_string(), "coat_winter_0".to_string());
        let coated = world.spawn((Inventory::new(4), frozen, StatusEffects::default(), Health::default(), coat_outfit));

        for _ in 0..10 {
            sys.tick(&mut world, 1.0, &data);
        }
        let bare_hp = world.get::<&Health>(bare).unwrap().current;
        let coated_hp = world.get::<&Health>(coated).unwrap().current;
        assert!(
            coated_hp > bare_hp + 5.0,
            "the coat blunted the cold: bare {bare_hp}, coated {coated_hp}"
        );
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

        // Starvation drains 1 HP/s; 2 HP is gone within three 1s ticks, while
        // hydration (80, decaying 0.13/s) stays far from zero — so the cause
        // is unambiguously starvation.
        for _ in 0..3 {
            sys.tick(&mut world, 1.0, &data);
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
    #[test]
    fn eating_spoiled_food_poisons_and_reduces_nutrition() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();

        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item("cooked_meat_0", 1, 99);
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
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "cooked_meat_0"))
            .expect("cooked_meat_0 tracked in spoilage side-table");
        sys.spoilage.get_mut(&(entity_bits, slot_idx)).unwrap().spoiled = true;

        *data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
            .unwrap()
            .lock()
            .unwrap() = Some("cooked_meat_0".to_string());
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
        inv.slots[0] = Some(crate::systems::inventory::ItemStack::new("cooked_meat_0".to_string(), 1, 99));
        inv.slots[3] = Some(crate::systems::inventory::ItemStack::new("cooked_meat_0".to_string(), 1, 99));
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
            .unwrap() = Some("cooked_meat_0".to_string());
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

    /// Low energy applies the `fatigued` speed debuff; the Rest action (rest_request)
    /// refills energy to full and clears fatigue.
    #[test]
    fn low_energy_fatigues_and_rest_restores() {
        let mut sys = FoodSystem::new(data_dir());
        let data = make_store();

        let mut world = hecs::World::new();
        let player = world.spawn((
            Inventory::new(4),
            Vitals {
                satiation: 80.0,
                hydration: 80.0,
                energy: 10.0,
                oxygen: 100.0,
                body_temp_c: 37.0,
                waste: 0.0,
                satiation_max: 100.0,
                hydration_max: 100.0,
                energy_max: 100.0,
                oxygen_max: 100.0,
                waste_max: 100.0,
            },
            StatusEffects::default(),
            Health::default(),
        ));

        // Low energy -> fatigued after a tick.
        sys.tick(&mut world, 1.0, &data);
        assert!(
            world.get::<&StatusEffects>(player).unwrap().has("fatigued"),
            "low energy applies the fatigued speed debuff"
        );

        // Rest -> energy refilled to (near) full + fatigue cleared.
        *data
            .get::<std::sync::Mutex<bool>>("rest_request")
            .unwrap()
            .lock()
            .unwrap() = true;
        sys.tick(&mut world, 1.0, &data);
        let energy = world.get::<&Vitals>(player).unwrap().energy;
        assert!(energy > 90.0, "rest restored energy (got {energy})");
        assert!(
            !world.get::<&StatusEffects>(player).unwrap().has("fatigued"),
            "rest cleared fatigue"
        );
    }

    /// Exposure to vacuum/cold (an exposed EnvironmentContext) drains oxygen + chills
    /// the body → hypoxia/hypothermia + health loss; re-sealing recovers oxygen.
    #[test]
    fn exposure_drains_oxygen_and_chills_then_recovers_when_sealed() {
        use crate::ecs::components::EnvironmentContext;
        let mut sys = FoodSystem::new(data_dir());
        let mut data = make_store();
        data.insert(
            "environment_context",
            EnvironmentContext {
                sealed: false,
                oxygenated: false,
                ambient_temp_c: -40.0,
            },
        );

        let mut world = hecs::World::new();
        let player = world.spawn((
            Inventory::new(4),
            vitals(80.0, 80.0),
            StatusEffects::default(),
            Health::default(),
        ));

        for _ in 0..30 {
            sys.tick(&mut world, 1.0, &data);
        }
        {
            let v = world.get::<&Vitals>(player).unwrap();
            assert!(v.oxygen < 50.0, "oxygen drained while exposed (got {})", v.oxygen);
            assert!(v.body_temp_c < 35.0, "body chilled while exposed (got {})", v.body_temp_c);
            let fx = world.get::<&StatusEffects>(player).unwrap();
            assert!(fx.has("hypoxia") || fx.has("suffocation"), "an oxygen condition applied");
            assert!(fx.has("hypothermia"), "hypothermia applied");
        }
        assert!(
            world.get::<&Health>(player).unwrap().current < 100.0,
            "exposure damaged health"
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
}
