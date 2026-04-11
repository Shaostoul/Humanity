//! Crop types — definitions loaded from `data/plants.csv`.
//!
//! Growth stages are data-driven strings defined per plant species,
//! so there is no hardcoded enum. See `PlantDef.growth_stages` in `mod.rs`.

use serde::{Deserialize, Serialize};

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
