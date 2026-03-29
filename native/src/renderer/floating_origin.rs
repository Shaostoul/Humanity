//! Floating origin system for large-scale rendering.
//!
//! All absolute world positions are stored as DVec3 (f64). Each frame,
//! the camera's world position is subtracted from all object positions,
//! yielding camera-relative Vec3 (f32) values the GPU can handle.
//!
//! This enables rendering at scales from meters (ship interior) to
//! billions of meters (planetary orbits) without floating-point jitter.

use glam::{DVec3, Vec3};

/// Converts absolute world positions (f64) to camera-relative render positions (f32).
pub struct FloatingOrigin {
    /// Camera absolute position in meters (f64 precision).
    pub camera_world_pos: DVec3,
}

impl FloatingOrigin {
    pub fn new() -> Self {
        Self {
            camera_world_pos: DVec3::ZERO,
        }
    }

    /// Convert an absolute world position to a camera-relative render position.
    /// The result fits in f32 because it's relative to the camera (nearby).
    pub fn to_render_pos(&self, world_pos: DVec3) -> Vec3 {
        let rel = world_pos - self.camera_world_pos;
        Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32)
    }

    /// Distance from camera to a world position (in meters, f64).
    pub fn distance_to(&self, world_pos: DVec3) -> f64 {
        (world_pos - self.camera_world_pos).length()
    }
}
