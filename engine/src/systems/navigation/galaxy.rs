//! Galaxy view — star positions, sector grid, jump routes.
//!
//! Galaxy generation parameters loaded from `config/galaxy.ron`.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// A star in the galaxy map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Star {
    pub name: String,
    pub position: Vec3,
    pub spectral_class: String,
    pub luminosity: f32,
}
