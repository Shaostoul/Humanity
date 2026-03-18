//! Planet surface navigation — terrain, heightmap, biomes.
//!
//! Biome definitions loaded from `data/biomes.csv`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// A point on a planet's surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfacePoint {
    pub position: Vec3,
    pub biome_id: String,
    pub elevation_m: f32,
}
