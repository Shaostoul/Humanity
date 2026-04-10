//! Food system -- nutrition, spoilage, cooking, and meal quality.
//!
//! Loads nutrition profiles, preservation methods, cooking methods, meal quality
//! levels, and temperature zones from `data/food_system.ron`.

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

/// Tracks nutrition, spoilage, and cooking.
pub struct FoodSystem {
    pub data: FoodData,
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
        Self { data }
    }
}

impl System for FoodSystem {
    fn name(&self) -> &str {
        "FoodSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // TODO: implement nutrition tracking, spoilage ticking, and cooking
    }
}
