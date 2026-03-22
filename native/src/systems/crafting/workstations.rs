//! Crafting stations — each station type unlocks specific recipe categories.
//!
//! Workstation definitions loaded from `data/workstations.csv`.

use serde::{Deserialize, Serialize};

/// A crafting workstation type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkstationDef {
    pub id: String,
    pub name: String,
    pub recipe_categories: Vec<String>,
}
