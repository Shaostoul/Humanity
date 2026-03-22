//! Crop types and growth stages — definitions loaded from `data/crops.csv`.

use serde::{Deserialize, Serialize};

/// Growth stage of a crop.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GrowthStage {
    Seed,
    Sprout,
    Vegetative,
    Flowering,
    Fruiting,
    Harvest,
}

/// A crop type definition (loaded from CSV).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropDef {
    pub id: String,
    pub name: String,
    pub growth_days: f32,
    pub water_need: f32,
    pub optimal_ph_min: f32,
    pub optimal_ph_max: f32,
}
