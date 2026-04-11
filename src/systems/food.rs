//! Food system -- nutrition, spoilage, cooking, and meal quality.
//!
//! Loads nutrition profiles, preservation methods, cooking methods, meal quality
//! levels, and temperature zones from `data/food_system.ron`.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/food_system.ron`.
#[derive(Debug, Deserialize)]
pub struct FoodData {
    pub nutrition_profiles: Vec<ron::Value>,
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
}

impl System for FoodSystem {
    fn name(&self) -> &str {
        "FoodSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        use crate::ecs::components::Name;
        use crate::systems::inventory::Inventory;

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
