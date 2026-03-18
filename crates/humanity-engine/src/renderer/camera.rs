//! Camera controller — first-person and third-person modes.
//!
//! Camera settings loaded from `config/camera.toml`.

use glam::{Mat4, Vec3};

/// Camera with configurable projection and view modes.
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 5.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_degrees: 60.0,
            near: 0.1,
            far: 10000.0,
        }
    }

    /// Build the view-projection matrix.
    pub fn view_projection(&self, aspect_ratio: f32) -> Mat4 {
        let view = Mat4::look_at_rh(self.position, self.target, self.up);
        let proj = Mat4::perspective_rh(
            self.fov_degrees.to_radians(),
            aspect_ratio,
            self.near,
            self.far,
        );
        proj * view
    }
}
