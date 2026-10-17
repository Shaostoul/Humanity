//! The viewer's camera: fitted to the model, turned, moved and zoomed by the person, exactly as
//! docs/design/model-viewer.md ("Camera and controls") gives it, so the website's
//! (web/shared/model-view.js) behaves the same.

use super::load::Bounds;
use glam::{Mat4, Vec3};

/// Vertical field of view.
pub const FOV_Y_DEG: f32 = 35.0;
const START_YAW_DEG: f32 = 35.0;
const START_PITCH_DEG: f32 = 20.0;
const MAX_PITCH_DEG: f32 = 89.0;
/// Radians turned per point dragged.
pub const TURN_PER_POINT: f32 = 0.008;
/// Zoom per point scrolled (`d *= exp(-points * ZOOM_PER_POINT)`).
pub const ZOOM_PER_POINT: f32 = 0.0015;

/// Where the camera is: turned `yaw` and `pitch` about `target`, `distance` from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    /// The model's bounding radius (zoom limits and clipping planes follow it).
    pub radius: f32,
    /// Where `reset` goes back to.
    fit_target: Vec3,
    fit_distance: f32,
}

impl Orbit {
    /// The camera fitted to `b`: the whole model in view, from a little above and to the side.
    pub fn fit(b: &Bounds) -> Orbit {
        let radius = b.radius.max(1e-6);
        let distance = radius / (FOV_Y_DEG.to_radians() * 0.5).sin() * 1.15;
        Orbit {
            yaw: START_YAW_DEG.to_radians(),
            pitch: START_PITCH_DEG.to_radians(),
            distance,
            target: b.center,
            radius,
            fit_target: b.center,
            fit_distance: distance,
        }
    }

    /// Back to the fit.
    pub fn reset(&mut self) {
        self.yaw = START_YAW_DEG.to_radians();
        self.pitch = START_PITCH_DEG.to_radians();
        self.distance = self.fit_distance;
        self.target = self.fit_target;
    }

    /// Turn by a drag of `dx`, `dy` points.
    pub fn turn(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * TURN_PER_POINT;
        let max = MAX_PITCH_DEG.to_radians();
        self.pitch = (self.pitch + dy * TURN_PER_POINT).clamp(-max, max);
    }

    /// Move the target by a drag of `dx`, `dy` points, on a view `view_height` points tall: the
    /// model follows the pointer.
    pub fn pan(&mut self, dx: f32, dy: f32, view_height: f32) {
        let per_point = 2.0 * self.distance * (FOV_Y_DEG.to_radians() * 0.5).tan() / view_height.max(1.0);
        let (right, up) = self.right_up();
        self.target += (-dx * right + dy * up) * per_point;
    }

    /// Zoom by `points` scrolled (positive: closer).
    pub fn zoom(&mut self, points: f32) {
        self.distance = (self.distance * (-points * ZOOM_PER_POINT).exp()).clamp(0.05 * self.radius, 50.0 * self.radius);
    }

    /// The camera's position.
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + self.distance * Vec3::new(cp * sy, sp, cp * cy)
    }

    /// The camera's right and up directions in the world.
    fn right_up(&self) -> (Vec3, Vec3) {
        let forward = (self.target - self.eye()).normalize_or(Vec3::NEG_Z);
        let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        (right, right.cross(forward))
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    /// The clipping planes: close enough that zooming in never cuts the model, far enough to hold
    /// all of it.
    pub fn near_far(&self) -> (f32, f32) {
        let r = self.radius;
        ((self.distance - 2.0 * r).max(self.distance * 0.001), self.distance + 2.0 * r)
    }

    /// The projection for a view `aspect` wide to tall (wgpu's 0..1 depth).
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let (near, far) = self.near_far();
        Mat4::perspective_rh(FOV_Y_DEG.to_radians(), aspect.max(1e-3), near, far)
    }
}
