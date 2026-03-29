//! Three-mode camera system: FirstPerson, ThirdPerson, Orbit.
//!
//! All modes share a single `Camera` struct and produce the same
//! view/projection matrices consumed by the renderer. Mode switching
//! uses smooth interpolation (slerp + lerp) to avoid jarring cuts.
//!
//! Platform-agnostic: pure math, no winit/web-sys imports.
//! Input flows through `CameraController` which accepts both
//! winit key codes (native) and string key names (WASM).

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Quat, Vec3};

// ── GPU uniform ──────────────────────────────────────────────

/// GPU-side camera uniform data (matches shader CameraUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub view_pos: [f32; 4],
}

// ── Camera mode enum ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraMode {
    FirstPerson,
    ThirdPerson,
    Orbit,
}

impl CameraMode {
    /// Cycle to the next mode: FP → TP → Orbit → FP.
    pub fn next(self) -> Self {
        match self {
            CameraMode::FirstPerson => CameraMode::ThirdPerson,
            CameraMode::ThirdPerson => CameraMode::Orbit,
            CameraMode::Orbit => CameraMode::FirstPerson,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    Perspective,
    Orthographic,
}

// ── Camera snapshot (for transitions) ────────────────────────

#[derive(Debug, Clone, Copy)]
struct CameraSnapshot {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    fov_degrees: f32,
    /// Third-person follow distance or orbit distance.
    distance: f32,
}

#[derive(Debug, Clone, Copy)]
struct CameraTransition {
    from: CameraSnapshot,
    to_mode: CameraMode,
    elapsed: f32,
    duration: f32,
}

// ── Camera ───────────────────────────────────────────────────

/// Unified camera supporting three modes.
pub struct Camera {
    // Shared state
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub up: Vec3,
    pub fov_degrees: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    pub projection: Projection,

    /// Absolute world position in meters (f64 precision).
    /// Updated each frame by accumulating the f32 `position` delta.
    /// The camera controller moves `position` (f32), which gets
    /// accumulated into `world_position` (f64) and reset to zero.
    pub world_position: DVec3,

    // Mode
    pub mode: CameraMode,

    // Third-person state
    /// Follow distance behind the player character.
    pub tp_distance: f32,
    /// Minimum follow distance.
    pub tp_distance_min: f32,
    /// Maximum follow distance.
    pub tp_distance_max: f32,
    /// Shoulder offset: positive = right, negative = left.
    pub tp_shoulder_offset: f32,
    /// The character position the camera follows (set by the game each frame).
    pub tp_target: Vec3,

    // Orbit state
    /// Focal point the camera orbits around.
    pub orbit_target: Vec3,
    /// Distance from focal point.
    pub orbit_distance: f32,
    pub orbit_distance_min: f32,
    pub orbit_distance_max: f32,
    /// Orthographic half-extent (used when projection == Orthographic).
    pub ortho_size: f32,

    // Transition
    transition: Option<CameraTransition>,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            yaw: 0.0,
            pitch: -0.3,
            up: Vec3::Y,
            fov_degrees: 90.0,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 1_000_000_000.0, // 1 billion meters = 1 million km
            projection: Projection::Perspective,

            mode: CameraMode::FirstPerson,

            tp_distance: 3.0,
            tp_distance_min: 1.5,
            tp_distance_max: 8.0,
            tp_shoulder_offset: 0.5,
            tp_target: Vec3::new(0.0, 1.0, 0.0),

            orbit_target: Vec3::ZERO,
            orbit_distance: 10.0,
            orbit_distance_min: 1.0,
            orbit_distance_max: 1000.0,
            ortho_size: 10.0,

            world_position: DVec3::ZERO,

            transition: None,
        }
    }

    /// Direction the camera is looking (forward vector from yaw/pitch).
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

    /// Horizontal forward (Y=0, for WASD movement).
    pub fn forward_xz(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos()).normalize()
    }

    /// Compute the effective camera position based on current mode.
    fn effective_position(&self) -> Vec3 {
        match self.mode {
            CameraMode::FirstPerson => self.position,
            CameraMode::ThirdPerson => {
                // Camera sits behind and above the character
                let back = -self.forward();
                let right = self.right();
                self.tp_target
                    + back * self.tp_distance
                    + right * self.tp_shoulder_offset
                    + Vec3::Y * 0.5 // slight height offset
            }
            CameraMode::Orbit => {
                let back = -self.forward();
                self.orbit_target + back * self.orbit_distance
            }
        }
    }

    /// Compute the effective look-at target based on current mode.
    fn effective_target(&self) -> Vec3 {
        match self.mode {
            CameraMode::FirstPerson => self.position + self.forward(),
            CameraMode::ThirdPerson => self.tp_target + Vec3::Y * 1.0,
            CameraMode::Orbit => self.orbit_target,
        }
    }

    /// View matrix (world to camera space).
    pub fn view_matrix(&self) -> Mat4 {
        // During transition, interpolate between snapshot and current
        if let Some(ref t) = self.transition {
            let alpha = ease_in_out_cubic(t.elapsed / t.duration);
            let from_pos = t.from.position;
            let to_pos = self.effective_position();
            let pos = from_pos.lerp(to_pos, alpha);

            // Reconstruct "from" forward from snapshot yaw/pitch
            let from_fwd = Vec3::new(
                t.from.yaw.sin() * t.from.pitch.cos(),
                t.from.pitch.sin(),
                -t.from.yaw.cos() * t.from.pitch.cos(),
            ).normalize();
            let from_target = from_pos + from_fwd;
            let to_target = self.effective_target();
            let target = from_target.lerp(to_target, alpha);

            return Mat4::look_at_rh(pos, target, self.up);
        }

        let pos = self.effective_position();
        let target = self.effective_target();
        Mat4::look_at_rh(pos, target, self.up)
    }

    /// Projection matrix.
    pub fn projection_matrix(&self) -> Mat4 {
        match self.projection {
            Projection::Perspective => {
                let fov = if let Some(ref t) = self.transition {
                    let alpha = ease_in_out_cubic(t.elapsed / t.duration);
                    let to_fov = self.fov_degrees;
                    lerp(t.from.fov_degrees, to_fov, alpha)
                } else {
                    self.fov_degrees
                };
                Mat4::perspective_rh(fov.to_radians(), self.aspect, self.near, self.far)
            }
            Projection::Orthographic => {
                let half_w = self.ortho_size * self.aspect;
                let half_h = self.ortho_size;
                Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, self.near, self.far)
            }
        }
    }

    /// Combined view-projection matrix.
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Build GPU uniform data from current state.
    pub fn uniforms(&self) -> CameraUniforms {
        let pos = self.effective_position();
        CameraUniforms {
            view_proj: self.view_projection_matrix().to_cols_array_2d(),
            view_pos: [pos.x, pos.y, pos.z, 1.0],
        }
    }

    /// Start a smooth transition to a new camera mode.
    pub fn switch_mode(&mut self, new_mode: CameraMode) {
        if new_mode == self.mode {
            return;
        }

        // Snapshot current state
        let snapshot = CameraSnapshot {
            position: self.effective_position(),
            yaw: self.yaw,
            pitch: self.pitch,
            fov_degrees: self.fov_degrees,
            distance: match self.mode {
                CameraMode::ThirdPerson => self.tp_distance,
                CameraMode::Orbit => self.orbit_distance,
                CameraMode::FirstPerson => 0.0,
            },
        };

        // When switching from FP to TP, set tp_target to current position
        if self.mode == CameraMode::FirstPerson && new_mode == CameraMode::ThirdPerson {
            self.tp_target = self.position;
        }

        // When switching to orbit, set orbit_target to what we're looking at
        if new_mode == CameraMode::Orbit {
            self.orbit_target = match self.mode {
                CameraMode::FirstPerson => self.position + self.forward_xz() * 5.0,
                CameraMode::ThirdPerson => self.tp_target,
                CameraMode::Orbit => self.orbit_target,
            };
        }

        // When switching from TP/Orbit to FP, set position to effective position
        if new_mode == CameraMode::FirstPerson {
            self.position = self.effective_position();
        }

        self.transition = Some(CameraTransition {
            from: snapshot,
            to_mode: new_mode,
            elapsed: 0.0,
            duration: 0.4,
        });
        self.mode = new_mode;
    }

    /// Advance transition timer. Call once per frame with dt in seconds.
    pub fn update_transition(&mut self, dt: f32) {
        if let Some(ref mut t) = self.transition {
            t.elapsed += dt;
            if t.elapsed >= t.duration {
                self.transition = None;
            }
        }
    }

    /// Whether the camera is currently transitioning between modes.
    pub fn is_transitioning(&self) -> bool {
        self.transition.is_some()
    }
}

// ── Easing helpers ───────────────────────────────────────────

fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0_f32).powi(3) / 2.0
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ── Camera controller ────────────────────────────────────────

/// Handles input and updates the camera each frame.
/// Behavior changes based on the camera's current mode.
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
    mouse_left: bool,
    mouse_right: bool,
    mouse_middle: bool,
    mouse_delta: (f64, f64),
    scroll_delta: f32,
    // Mode switch requests (consumed by update_camera)
    switch_to_next: bool,
    switch_fp_tp: bool,
    switch_orbit: bool,
    toggle_ortho: bool,
    toggle_shoulder: bool,
    // Gravity / jump state (first-person mode)
    vertical_velocity: f32,
    is_grounded: bool,
    eye_height: f32,
    jump_speed: f32,
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
            mouse_left: false,
            mouse_right: false,
            mouse_middle: false,
            mouse_delta: (0.0, 0.0),
            scroll_delta: 0.0,
            switch_to_next: false,
            switch_fp_tp: false,
            switch_orbit: false,
            toggle_ortho: false,
            toggle_shoulder: false,
            vertical_velocity: 0.0,
            is_grounded: true,
            eye_height: 1.7,
            jump_speed: 5.0,
        }
    }

    /// Process a keyboard event from winit.
    #[cfg(feature = "native")]
    pub fn process_keyboard(
        &mut self,
        key: winit::keyboard::KeyCode,
        state: winit::event::ElementState,
    ) {
        use winit::event::ElementState;
        use winit::keyboard::KeyCode;

        let pressed = state == ElementState::Pressed;
        match key {
            KeyCode::KeyW => self.forward = pressed,
            KeyCode::KeyS => self.backward = pressed,
            KeyCode::KeyA => self.left = pressed,
            KeyCode::KeyD => self.right = pressed,
            KeyCode::Space => self.ascend = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.descend = pressed,
            // Mode switches (on press only)
            KeyCode::Tab if pressed => self.switch_to_next = true,
            KeyCode::KeyF | KeyCode::KeyV if pressed => self.switch_fp_tp = true,
            KeyCode::KeyM if pressed => self.switch_orbit = true,
            KeyCode::KeyO if pressed => self.toggle_ortho = true,
            KeyCode::KeyQ if pressed => self.toggle_shoulder = true,
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
        let pressed = state == winit::event::ElementState::Pressed;
        match button {
            winit::event::MouseButton::Left => self.mouse_left = pressed,
            winit::event::MouseButton::Right => self.mouse_right = pressed,
            winit::event::MouseButton::Middle => self.mouse_middle = pressed,
            _ => {}
        }
    }

    /// Process a keyboard event from string key names (WASM-friendly).
    pub fn process_key(&mut self, key_code: &str, pressed: bool) {
        match key_code {
            "KeyW" => self.forward = pressed,
            "KeyS" => self.backward = pressed,
            "KeyA" => self.left = pressed,
            "KeyD" => self.right = pressed,
            "Space" => self.ascend = pressed,
            "ShiftLeft" | "ShiftRight" => self.descend = pressed,
            "Tab" if pressed => self.switch_to_next = true,
            "KeyF" | "KeyV" if pressed => self.switch_fp_tp = true,
            "KeyM" if pressed => self.switch_orbit = true,
            "KeyO" if pressed => self.toggle_ortho = true,
            "KeyQ" if pressed => self.toggle_shoulder = true,
            _ => {}
        }
    }

    /// Set mouse button state (WASM-friendly). button: 0=left, 1=middle, 2=right.
    pub fn set_mouse_button(&mut self, button: i32, pressed: bool) {
        match button {
            0 => self.mouse_left = pressed,
            1 => self.mouse_middle = pressed,
            2 => self.mouse_right = pressed,
            _ => {}
        }
    }

    /// Legacy helper: set right-mouse pressed state.
    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_right = pressed;
    }

    /// Accumulate mouse motion delta.
    pub fn process_mouse_motion(&mut self, dx: f64, dy: f64) {
        self.mouse_delta.0 += dx;
        self.mouse_delta.1 += dy;
    }

    /// Accumulate scroll wheel delta (positive = zoom in).
    pub fn process_scroll(&mut self, delta: f32) {
        self.scroll_delta += delta;
    }

    /// Apply accumulated input to the camera. Call once per frame with dt in seconds.
    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        // Handle mode switch requests
        if self.switch_to_next {
            self.switch_to_next = false;
            let next = camera.mode.next();
            camera.switch_mode(next);
        }
        if self.switch_fp_tp {
            self.switch_fp_tp = false;
            let target = match camera.mode {
                CameraMode::FirstPerson => CameraMode::ThirdPerson,
                CameraMode::ThirdPerson => CameraMode::FirstPerson,
                CameraMode::Orbit => CameraMode::FirstPerson,
            };
            camera.switch_mode(target);
        }
        if self.switch_orbit {
            self.switch_orbit = false;
            let target = if camera.mode == CameraMode::Orbit {
                CameraMode::FirstPerson
            } else {
                CameraMode::Orbit
            };
            camera.switch_mode(target);
        }
        if self.toggle_ortho {
            self.toggle_ortho = false;
            if camera.mode == CameraMode::Orbit {
                camera.projection = match camera.projection {
                    Projection::Perspective => Projection::Orthographic,
                    Projection::Orthographic => Projection::Perspective,
                };
            }
        }
        if self.toggle_shoulder {
            self.toggle_shoulder = false;
            if camera.mode == CameraMode::ThirdPerson {
                camera.tp_shoulder_offset = -camera.tp_shoulder_offset;
            }
        }

        // Advance transition
        camera.update_transition(dt);

        // Consume deltas
        let (dx, dy) = self.mouse_delta;
        self.mouse_delta = (0.0, 0.0);
        let scroll = self.scroll_delta;
        self.scroll_delta = 0.0;

        match camera.mode {
            CameraMode::FirstPerson => {
                self.update_first_person(camera, dt, dx, dy);
            }
            CameraMode::ThirdPerson => {
                self.update_third_person(camera, dt, dx, dy, scroll);
            }
            CameraMode::Orbit => {
                self.update_orbit(camera, dt, dx, dy, scroll);
            }
        }
    }

    /// First-person: WASD moves character, mouse rotates view.
    /// Includes gravity simulation and jump mechanics.
    fn update_first_person(
        &mut self,
        camera: &mut Camera,
        dt: f32,
        mouse_dx: f64,
        mouse_dy: f64,
    ) {
        // Mouse look (always active in FP mode — cursor is grabbed)
        camera.yaw += mouse_dx as f32 * self.mouse_sensitivity * 0.01;
        camera.pitch -= mouse_dy as f32 * self.mouse_sensitivity * 0.01;
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);

        // WASD movement relative to camera facing (horizontal only)
        let forward = camera.forward_xz();
        let right = forward.cross(Vec3::Y).normalize();

        let mut velocity = Vec3::ZERO;
        if self.forward { velocity += forward; }
        if self.backward { velocity -= forward; }
        if self.right { velocity += right; }
        if self.left { velocity -= right; }

        // Crouch: slow movement when shift is held
        let move_speed = if self.descend {
            self.speed * 0.4
        } else {
            self.speed
        };

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * move_speed * dt;
            camera.position += velocity;
        }

        // ── Gravity and jump ──
        // Jump: apply upward impulse if grounded and Space is pressed
        if self.ascend && self.is_grounded {
            self.vertical_velocity = self.jump_speed;
            self.is_grounded = false;
        }

        // Apply gravity
        self.vertical_velocity -= 9.8 * dt;
        camera.position.y += self.vertical_velocity * dt;

        // Ground collision
        let ground_height = 0.0_f32;
        if camera.position.y < ground_height + self.eye_height {
            camera.position.y = ground_height + self.eye_height;
            self.vertical_velocity = 0.0;
            self.is_grounded = true;
        }
    }

    /// Third-person: WASD moves character, mouse orbits around character.
    fn update_third_person(
        &self,
        camera: &mut Camera,
        dt: f32,
        mouse_dx: f64,
        mouse_dy: f64,
        scroll: f32,
    ) {
        // Mouse look (orbit around character)
        if self.mouse_right || self.mouse_left {
            camera.yaw += mouse_dx as f32 * self.mouse_sensitivity * 0.01;
            camera.pitch -= mouse_dy as f32 * self.mouse_sensitivity * 0.01;
            let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
            camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);
        }

        // Scroll to adjust follow distance
        if scroll.abs() > 0.01 {
            camera.tp_distance -= scroll * 0.5;
            camera.tp_distance = camera.tp_distance.clamp(
                camera.tp_distance_min,
                camera.tp_distance_max,
            );
        }

        // WASD moves the character (tp_target)
        let forward = camera.forward_xz();
        let right = forward.cross(Vec3::Y).normalize();

        let mut velocity = Vec3::ZERO;
        if self.forward { velocity += forward; }
        if self.backward { velocity -= forward; }
        if self.right { velocity += right; }
        if self.left { velocity -= right; }
        if self.ascend { velocity += Vec3::Y; }
        if self.descend { velocity -= Vec3::Y; }

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * self.speed * dt;
            camera.tp_target += velocity;
        }
    }

    /// Orbit: left-drag rotates, middle-drag pans, scroll zooms.
    fn update_orbit(
        &self,
        camera: &mut Camera,
        dt: f32,
        mouse_dx: f64,
        mouse_dy: f64,
        scroll: f32,
    ) {
        // Left-drag or right-drag to rotate
        if self.mouse_left || self.mouse_right {
            camera.yaw += mouse_dx as f32 * self.mouse_sensitivity * 0.01;
            camera.pitch -= mouse_dy as f32 * self.mouse_sensitivity * 0.01;
            let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
            camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);
        }

        // Middle-drag to pan the focal point
        if self.mouse_middle {
            let right = camera.right();
            let up_dir = Vec3::Y;
            let pan_speed = camera.orbit_distance * 0.002;
            camera.orbit_target -= right * mouse_dx as f32 * pan_speed;
            camera.orbit_target += up_dir * mouse_dy as f32 * pan_speed;
        }

        // Scroll to zoom
        if scroll.abs() > 0.01 {
            let zoom_factor = 1.0 - scroll * 0.1;
            camera.orbit_distance *= zoom_factor;
            camera.orbit_distance = camera.orbit_distance.clamp(
                camera.orbit_distance_min,
                camera.orbit_distance_max,
            );
            // Also adjust ortho size for orthographic zoom
            if camera.projection == Projection::Orthographic {
                camera.ortho_size *= zoom_factor;
                camera.ortho_size = camera.ortho_size.clamp(0.5, 500.0);
            }
        }

        // WASD pans the focal point (at the orbit plane)
        let forward = camera.forward_xz();
        let right = forward.cross(Vec3::Y).normalize();
        let pan_speed = camera.orbit_distance * 0.5;

        let mut pan = Vec3::ZERO;
        if self.forward { pan += forward; }
        if self.backward { pan -= forward; }
        if self.right { pan += right; }
        if self.left { pan -= right; }
        if self.ascend { pan += Vec3::Y; }
        if self.descend { pan -= Vec3::Y; }

        if pan.length_squared() > 0.0 {
            pan = pan.normalize() * pan_speed * dt;
            camera.orbit_target += pan;
        }
    }
}
