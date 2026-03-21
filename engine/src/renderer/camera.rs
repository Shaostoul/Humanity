//! Camera controller — first-person and third-person modes.
//!
//! Camera settings loaded from `config/camera.toml`.
//! The Camera struct is platform-agnostic (pure math).
//! CameraController uses winit types on native, raw input on WASM.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// GPU-side camera uniform data (matches shader CameraUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub view_pos: [f32; 4],
}

/// Camera with configurable projection and view modes.
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // radians, 0 = looking along -Z
    pub pitch: f32,  // radians, clamped to avoid gimbal lock
    pub up: Vec3,
    pub fov_degrees: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            yaw: 0.0,
            pitch: -0.3, // slightly looking down
            up: Vec3::Y,
            fov_degrees: 60.0,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 10000.0,
        }
    }

    /// Direction the camera is looking (forward vector).
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    /// Right vector (perpendicular to forward and up).
    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up).normalize()
    }

    /// View matrix (world to camera space).
    pub fn view_matrix(&self) -> Mat4 {
        let target = self.position + self.forward();
        Mat4::look_at_rh(self.position, target, self.up)
    }

    /// Projection matrix.
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(
            self.fov_degrees.to_radians(),
            self.aspect,
            self.near,
            self.far,
        )
    }

    /// Combined view-projection matrix.
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Build GPU uniform data from current state.
    pub fn uniforms(&self) -> CameraUniforms {
        CameraUniforms {
            view_proj: self.view_projection_matrix().to_cols_array_2d(),
            view_pos: [self.position.x, self.position.y, self.position.z, 1.0],
        }
    }
}

/// FPS-style camera controller: WASD movement + mouse look.
/// On native, feed winit events. On WASM, feed raw key/mouse data.
pub struct CameraController {
    pub speed: f32,
    pub mouse_sensitivity: f32,
    // Movement keys held
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    ascend: bool,
    descend: bool,
    // Mouse state
    mouse_pressed: bool,
    mouse_delta: (f64, f64),
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            mouse_sensitivity: sensitivity,
            forward: false,
            backward: false,
            left: false,
            right: false,
            ascend: false,
            descend: false,
            mouse_pressed: false,
            mouse_delta: (0.0, 0.0),
        }
    }

    /// Process a keyboard event from winit.
    #[cfg(feature = "native")]
    pub fn process_keyboard(
        &mut self,
        key: winit::keyboard::KeyCode,
        state: winit::event::ElementState,
    ) {
        use winit::keyboard::KeyCode;
        use winit::event::ElementState;

        let pressed = state == ElementState::Pressed;
        match key {
            KeyCode::KeyW => self.forward = pressed,
            KeyCode::KeyS => self.backward = pressed,
            KeyCode::KeyA => self.left = pressed,
            KeyCode::KeyD => self.right = pressed,
            KeyCode::Space => self.ascend = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.descend = pressed,
            _ => {}
        }
    }

    /// Process a mouse button event from winit.
    #[cfg(feature = "native")]
    pub fn process_mouse_button(
        &mut self,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        if button == winit::event::MouseButton::Right {
            self.mouse_pressed = state == winit::event::ElementState::Pressed;
        }
    }

    /// Process a keyboard event from string key names (WASM-friendly).
    /// key_code: "KeyW", "KeyA", "KeyS", "KeyD", "Space", "ShiftLeft", etc.
    pub fn process_key(&mut self, key_code: &str, pressed: bool) {
        match key_code {
            "KeyW" => self.forward = pressed,
            "KeyS" => self.backward = pressed,
            "KeyA" => self.left = pressed,
            "KeyD" => self.right = pressed,
            "Space" => self.ascend = pressed,
            "ShiftLeft" | "ShiftRight" => self.descend = pressed,
            _ => {}
        }
    }

    /// Set whether the mouse look button is pressed (WASM-friendly).
    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    /// Accumulate mouse motion delta.
    pub fn process_mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.mouse_pressed {
            self.mouse_delta.0 += dx;
            self.mouse_delta.1 += dy;
        }
    }

    /// Apply accumulated input to the camera. Call once per frame with dt in seconds.
    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        // Mouse look
        let (dx, dy) = self.mouse_delta;
        self.mouse_delta = (0.0, 0.0);

        camera.yaw += dx as f32 * self.mouse_sensitivity * 0.01;
        camera.pitch -= dy as f32 * self.mouse_sensitivity * 0.01;

        // Clamp pitch to avoid flipping
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);

        // WASD movement relative to camera direction
        let forward = Vec3::new(camera.yaw.sin(), 0.0, -camera.yaw.cos()).normalize();
        let right = forward.cross(Vec3::Y).normalize();

        let mut velocity = Vec3::ZERO;
        if self.forward {
            velocity += forward;
        }
        if self.backward {
            velocity -= forward;
        }
        if self.right {
            velocity += right;
        }
        if self.left {
            velocity -= right;
        }
        if self.ascend {
            velocity += Vec3::Y;
        }
        if self.descend {
            velocity -= Vec3::Y;
        }

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * self.speed * dt;
            camera.position += velocity;
        }
    }
}
