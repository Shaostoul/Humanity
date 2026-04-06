//! Spatial audio — 3D sound positioning (stub for Steam Audio integration).

use glam::Vec3;

/// Spatial audio source with 3D position.
pub struct SpatialSource {
    pub position: Vec3,
    pub volume: f32,
    pub radius: f32,
}

impl SpatialSource {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            volume: 1.0,
            radius: 50.0,
        }
    }
}
