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

use super::light::RoomLight;

/// First-person gravity (m/s^2). Tuned for a snappy game jump rather than realism:
/// with jump_speed 5.0 it gives a peak height of ~1.0 m.
const GRAVITY: f32 = 12.0;

// ── GPU uniform ──────────────────────────────────────────────

/// GPU-side camera uniform data (matches shader CameraUniforms).
/// Includes up to 8 point lights packed into the uniform buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub view_pos: [f32; 4],
    /// Point light positions: xyz = position, w = intensity. Up to 8 lights.
    pub light_positions: [[f32; 4]; 8],
    /// Point light colors: xyz = color, w = radius.
    pub light_colors: [[f32; 4]; 8],
    /// Spot cone aim (v0.639): xyz = normalized aim direction in the light-to-fragment sense
    /// (the direction the fixture points), w = cos(outer cone half-angle). A Point/Bar light
    /// (no cone) uses the sentinel w = -1.0, which the shader's `spot.w > -1.0` guard skips
    /// entirely -- zero extra cost, zero behavior change for every pre-existing light.
    pub light_spot: [[f32; 4]; 8],
    /// Spot cone inner angle (v0.639): x = cos(inner cone half-angle), yzw = unused padding.
    /// Only meaningful when the matching `light_spot[i].w > -1.0`.
    pub light_cone_inner: [[f32; 4]; 8],
    /// x = number of active point lights, yzw = unused.
    pub light_count: [f32; 4],
    /// Directional sun light: xyz = direction (toward light), w = intensity.
    pub sun_direction: [f32; 4],
    /// Sun light color: rgb, w = unused.
    pub sun_color: [f32; 4],
    /// Fill light: xyz = direction (toward light), w = intensity.
    pub fill_direction: [f32; 4],
    /// Fill light color: rgb, w = unused.
    pub fill_color: [f32; 4],
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
            position: Vec3::new(0.0, 1.7, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            up: Vec3::Y,
            fov_degrees: 90.0,
            aspect: 16.0 / 9.0,
            near: 0.1,
            // Far plane pushed well past Sun distance (~1.496e11 m) so the
            // Sun and other inner-system bodies can be rendered from GEO
            // without being clipped. The previous 1e9 m value clipped at
            // about 1 million km, hiding everything beyond the Moon's
            // orbit. 1e12 m = 1 trillion metres, comfortably past Jupiter.
            far: 1_000_000_000_000.0,
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
                // Reverse-Z: swap near/far for better far-field depth precision
                Mat4::perspective_rh(fov.to_radians(), self.aspect, self.far, self.near)
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

    /// Build a world-space PICK RAY from a cursor pixel (for 3D selection). `cursor` is (x, y)
    /// in physical pixels (top-left origin); `viewport` is (width, height) in the same units.
    /// Returns (origin, direction-normalized). Unprojects the near + far clip points through
    /// the inverse view-projection; works in every camera mode (and for orthographic, the rays
    /// come out parallel). Reverse-Z aware: the near plane is clip z = 1.0, far is z = 0.0.
    /// (v0.466)
    pub fn pick_ray(&self, cursor: (f32, f32), viewport: (f32, f32)) -> (Vec3, Vec3) {
        let ndc_x = (cursor.0 / viewport.0.max(1.0)) * 2.0 - 1.0;
        let ndc_y = 1.0 - (cursor.1 / viewport.1.max(1.0)) * 2.0; // screen Y is flipped
        let inv = self.view_projection_matrix().inverse();
        let near = inv * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let far = inv * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
        let near = near.truncate() / near.w;
        let far = far.truncate() / far.w;
        (near, (far - near).normalize_or_zero())
    }

    /// Build GPU uniform data from current state (no point lights).
    /// Uses default directional sun/fill lights matching the former shader constants.
    pub fn uniforms(&self) -> CameraUniforms {
        let pos = self.effective_position();
        CameraUniforms {
            view_proj: self.view_projection_matrix().to_cols_array_2d(),
            view_pos: [pos.x, pos.y, pos.z, 1.0],
            light_positions: [[0.0; 4]; 8],
            light_colors: [[0.0; 4]; 8],
            light_spot: [[0.0, -1.0, 0.0, -1.0]; 8],
            light_cone_inner: [[0.0; 4]; 8],
            light_count: [0.0, 0.0, 0.0, 0.0],
            // Default sun: warm sunlight from upper-right (same as former shader constants)
            sun_direction: [0.3, 1.0, 0.5, 2.5],
            sun_color: [1.0, 0.95, 0.9, 0.0],
            // Default fill: cool, from lower-left
            fill_direction: [-0.5, 0.3, -0.3, 0.6],
            fill_color: [0.4, 0.5, 0.7, 0.0],
        }
    }

    /// Camera uniforms for the CELESTIAL pass (v0.450): same view + lighting as `uniforms`,
    /// but a HUGE far plane (1e13) so the solar system -- Earth (~42,000 km), the Sun + planets
    /// (AU scale), out to the outer system (tens of AU) -- is not clipped by the gameplay far
    /// (~500 m). Reverse-Z (near/far swapped) keeps far-field depth precision.
    pub fn celestial_uniforms(&self) -> CameraUniforms {
        let proj = Mat4::perspective_rh(self.fov_degrees.to_radians(), self.aspect, 1.0e13, 1.0);
        let mut u = self.uniforms();
        u.view_proj = (proj * self.view_matrix()).to_cols_array_2d();
        u
    }

    /// Build GPU uniform data with room lights (v0.639: point OR spot, see `RoomLight`).
    pub fn uniforms_with_lights(&self, lights: &[RoomLight]) -> CameraUniforms {
        let pos = self.effective_position();
        let mut light_positions = [[0.0_f32; 4]; 8];
        let mut light_colors = [[0.0_f32; 4]; 8];
        let mut light_spot = [[0.0_f32, -1.0, 0.0, -1.0]; 8];
        let mut light_cone_inner = [[0.0_f32; 4]; 8];
        let count = lights.len().min(8);
        for (i, l) in lights.iter().take(8).enumerate() {
            light_positions[i] = [l.pos.x, l.pos.y, l.pos.z, l.intensity];
            light_colors[i] = [l.color[0], l.color[1], l.color[2], l.range];
            light_spot[i] = [l.dir.x, l.dir.y, l.dir.z, l.cos_outer];
            light_cone_inner[i] = [l.cos_inner, 0.0, 0.0, 0.0];
        }
        CameraUniforms {
            view_proj: self.view_projection_matrix().to_cols_array_2d(),
            view_pos: [pos.x, pos.y, pos.z, 1.0],
            light_positions,
            light_colors,
            light_spot,
            light_cone_inner,
            light_count: [count as f32, 0.0, 0.0, 0.0],
            // Default sun: warm sunlight from upper-right (same as former shader constants)
            sun_direction: [0.3, 1.0, 0.5, 2.5],
            sun_color: [1.0, 0.95, 0.9, 0.0],
            // Default fill: cool, from lower-left
            fill_direction: [-0.5, 0.3, -0.3, 0.6],
            fill_color: [0.4, 0.5, 0.7, 0.0],
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
    /// Multiplier on movement speed from the player's active status effects
    /// (1.0 = normal). Set each frame by the main loop from the player's
    /// StatusEffects + the status-effect registry. Look/rotation is unaffected.
    pub speed_multiplier: f32,
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
    /// World-Y the camera rests at when grounded (floor_y + eye_height). Set each frame
    /// by the main loop from the room the player is standing in; defaults to floor 0.
    ground_y: f32,
    /// Ladder CLIMB zone (v0.589): Some((base_floor, top_floor)) when the player stands at a ladder,
    /// set each frame by the main loop. While set AND an up/down input is held, the player moves
    /// vertically (gravity suspended) clamped to the ladder span -- so they climb to an upper deck.
    climb_zone: Option<(f32, f32)>,
    /// Character-showroom lock (v0.443): when true the orbit camera is FIXED on the avatar
    /// -- only a mouse drag spins it and the wheel zooms; WASD and panning are disabled.
    pub showroom_lock: bool,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            speed_multiplier: 1.0,
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
            ground_y: 1.7,
            climb_zone: None,
            showroom_lock: false,
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

    /// Set the floor the player is standing on (world Y of the room floor). The grounded
    /// rest height becomes `floor_y + eye_height`. Called each frame by the main loop from
    /// the room the player is in, so walking between rooms keeps you on their floors.
    pub fn set_ground_floor(&mut self, floor_y: f32) {
        self.ground_y = floor_y + self.eye_height;
    }

    /// The floor the player currently rests on (world Y), i.e. the inverse of `set_ground_floor`.
    /// The structure footing sampler (v0.584) reads this to cap how high a single step-up may be.
    pub fn ground_floor(&self) -> f32 {
        self.ground_y - self.eye_height
    }

    /// Set (or clear) the ladder CLIMB zone (v0.589): Some((base_floor, top_floor)) when the player
    /// is at a ladder. Set each frame by the main loop from the structure pieces near the player.
    pub fn set_climb_zone(&mut self, zone: Option<(f32, f32)>) {
        self.climb_zone = zone;
    }

    /// Eye height (camera Y above the feet). The footing sampler uses it to get the player's ACTUAL
    /// feet height (`camera.y - eye_height`) -- which, unlike `ground_floor()`, tracks the live
    /// climbed height, so a deck at a ladder top is reachable. (v0.589)
    pub fn eye_height(&self) -> f32 {
        self.eye_height
    }

    /// Is the player currently at a ladder (climb zone set)? The footing sampler uses LIVE height for
    /// the step-up cap ONLY while climbing -- so a deck at the ladder top is reachable -- but the
    /// lagging rest floor otherwise, so a normal JUMP can't cheese you up a tall box. (v0.589)
    pub fn in_climb_zone(&self) -> bool {
        self.climb_zone.is_some()
    }

    /// First-person: WASD walks (Shift = sprint), Space = jump, gravity pulls you to the
    /// floor. Mouse rotates the view. (Was free-fly noclip: Shift floated you down and
    /// Space up with no sprint, the operator's BUG-039.)
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

        // Shift = SPRINT (hold to move faster). `speed_multiplier` carries status-effect
        // modifiers (well_nourished speeds up, thirsty/flu slow down).
        let sprint = if self.descend { 1.9 } else { 1.0 };
        let move_speed = self.speed * sprint * self.speed_multiplier;

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * move_speed * dt;
            // Horizontal only — gravity owns Y so a sprint can't cancel a jump.
            camera.position.x += velocity.x;
            camera.position.z += velocity.z;
        }

        // ── Ladder climb (v0.589) ──
        // At a ladder (climb_zone set) WITH an up/down input held, move vertically + suspend gravity,
        // clamped to the ladder span -- so you climb to an upper deck. Space = up, Shift = down. No
        // input near a ladder falls through to normal gravity, so you can still walk past one.
        if let Some((base, top)) = self.climb_zone {
            let lo = base + self.eye_height;
            let hi = top + self.eye_height + 0.2; // eye can reach just over the top rung
            // Only ENGAGE the ladder when the camera is already near its span -- if you jumped in
            // from above or arrived from below, fall through to gravity and LAND first, so the clamp
            // never teleport-snaps you onto the ladder. (v0.589 review fix)
            let near = camera.position.y >= lo - 0.5 && camera.position.y <= hi + 0.5;
            if near && (self.ascend || self.descend) {
                let climb = self.speed * 0.6 * dt;
                if self.ascend {
                    camera.position.y += climb;
                }
                if self.descend {
                    camera.position.y -= climb;
                }
                camera.position.y = camera.position.y.clamp(lo, hi);
                self.vertical_velocity = 0.0;
                self.is_grounded = false;
                return; // skip gravity this frame -- the ladder holds you
            }
        }

        // ── Gravity + jump (grounded on the room floor) ──
        // Space launches a jump only when grounded.
        if self.ascend && self.is_grounded {
            self.vertical_velocity = self.jump_speed;
            self.is_grounded = false;
        }
        // Apply gravity and integrate height.
        self.vertical_velocity -= GRAVITY * dt;
        camera.position.y += self.vertical_velocity * dt;
        // Land on the floor.
        if camera.position.y <= self.ground_y {
            camera.position.y = self.ground_y;
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
        // Character showroom: the camera is FIXED on the avatar. A mouse drag (middle, or
        // left/right) spins it; the wheel zooms. No WASD, no panning. (v0.443)
        if self.showroom_lock {
            if self.mouse_middle || self.mouse_left || self.mouse_right {
                camera.yaw += mouse_dx as f32 * self.mouse_sensitivity * 0.01;
                camera.pitch -= mouse_dy as f32 * self.mouse_sensitivity * 0.01;
                let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
                camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);
            }
            if scroll.abs() > 0.01 {
                let zoom_factor = 1.0 - scroll * 0.1;
                camera.orbit_distance = (camera.orbit_distance * zoom_factor)
                    .clamp(camera.orbit_distance_min, camera.orbit_distance_max);
            }
            return;
        }

        // MIDDLE-drag to ORBIT (rotate). LEFT is reserved for interaction / 3D picking (grab a
        // room in the orbit view). (operator remap v0.465)
        if self.mouse_middle {
            camera.yaw += mouse_dx as f32 * self.mouse_sensitivity * 0.01;
            camera.pitch -= mouse_dy as f32 * self.mouse_sensitivity * 0.01;
            let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
            camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);
        }

        // RIGHT-drag to PAN along the FLOOR plane (horizontal), so panning stays level with the
        // floor you're looking at instead of sliding up/down the screen. (operator remap v0.465)
        if self.mouse_right {
            let fwd = camera.forward_xz();
            let right_h = fwd.cross(Vec3::Y).normalize();
            let pan_speed = camera.orbit_distance * 0.0015;
            camera.orbit_target -= right_h * mouse_dx as f32 * pan_speed;
            camera.orbit_target += fwd * mouse_dy as f32 * pan_speed;
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
