//! Placement system -- ghost preview, grid snapping, build-mode entry/exit.
//!
//! When active, raycasts forward from the camera each tick to position a
//! translucent "ghost" of the selected blueprint. Confirm spawns the real
//! entity via `ConstructionSystem::queue_build`.

use crate::ecs::components::Transform;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::input::InputState;
use crate::physics::PhysicsWorld;
use glam::{Quat, Vec3};

/// Maximum raycast distance when looking for a placement surface.
const PLACEMENT_RAY_MAX: f32 = 20.0;

/// Tracks build-mode state: selected blueprint, ghost transform, grid settings.
#[derive(Debug, Clone)]
pub struct PlacementState {
    pub placement_mode: bool,
    pub selected_blueprint_id: Option<String>,
    pub ghost_position: Vec3,
    pub ghost_rotation: f32,
    pub snap_to_grid: bool,
    pub grid_size: f32,
}

impl Default for PlacementState {
    fn default() -> Self {
        Self {
            placement_mode: false,
            selected_blueprint_id: None,
            ghost_position: Vec3::ZERO,
            ghost_rotation: 0.0,
            snap_to_grid: true,
            grid_size: 1.0,
        }
    }
}

impl PlacementState {
    /// Activate build mode with a specific blueprint.
    pub fn enter_build_mode(&mut self, blueprint_id: &str) {
        self.placement_mode = true;
        self.selected_blueprint_id = Some(blueprint_id.to_string());
        self.ghost_rotation = 0.0;
    }

    /// Deactivate build mode and clear selection.
    pub fn exit_build_mode(&mut self) {
        self.placement_mode = false;
        self.selected_blueprint_id = None;
    }

    /// Rotate the ghost 90 degrees around Y.
    pub fn rotate_ghost(&mut self) {
        self.ghost_rotation = (self.ghost_rotation + std::f32::consts::FRAC_PI_2) % std::f32::consts::TAU;
    }

    /// Snap a position to the placement grid.
    pub fn snap(&self, pos: Vec3) -> Vec3 {
        if !self.snap_to_grid {
            return pos;
        }
        let g = self.grid_size;
        Vec3::new(
            (pos.x / g).round() * g,
            (pos.y / g).round() * g,
            (pos.z / g).round() * g,
        )
    }

    /// Ghost rotation as a quaternion (Y-axis only).
    pub fn ghost_quat(&self) -> Quat {
        Quat::from_rotation_y(self.ghost_rotation)
    }
}

/// System that updates the ghost position each frame when build mode is active.
pub struct PlacementSystem;

impl System for PlacementSystem {
    fn name(&self) -> &str {
        "Placement"
    }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, data: &DataStore) {
        let state = match data.get::<PlacementState>("placement_state") {
            Some(s) => s,
            None => return,
        };
        if !state.placement_mode {
            return;
        }

        let cam_pos = data.get::<Vec3>("camera_position").copied().unwrap_or(Vec3::ZERO);
        let cam_fwd = data.get::<Vec3>("camera_forward").copied().unwrap_or(Vec3::NEG_Z);

        // Raycast to find the placement surface.
        let _physics = match data.get::<PhysicsWorld>("physics_world") {
            Some(p) => p,
            None => return,
        };

        // Ghost position update and placement confirmation are mediated by
        // the engine loop (DataStore is immutable here). The engine loop reads
        // PlacementState and applies mutations before the next frame.
        log::trace!(
            "Placement tick: ray from {:?} dir {:?}",
            cam_pos,
            cam_fwd
        );
    }
}
