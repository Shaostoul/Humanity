//! Item definitions — loaded from `data/items.csv`.

use serde::{Deserialize, Serialize};

/// An item type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub volume_m3: f32,
    pub mass_kg: f32,
    pub stackable: bool,
    pub max_stack: u32,
}
