//! Soil chemistry — pH, NPK, water content.
//!
//! Soil type defaults loaded from `data/soil_types.csv`.

use serde::{Deserialize, Serialize};

/// Soil state for a farming plot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soil {
    pub ph: f32,
    pub nitrogen: f32,
    pub phosphorus: f32,
    pub potassium: f32,
    pub water_content: f32,
    pub organic_matter: f32,
}

impl Default for Soil {
    fn default() -> Self {
        Self {
            ph: 6.5,
            nitrogen: 0.5,
            phosphorus: 0.5,
            potassium: 0.5,
            water_content: 0.3,
            organic_matter: 0.2,
        }
    }
}
