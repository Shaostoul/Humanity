//! Player controller — translates input into physics-driven movement.
//!
//! Reads `InputState` from DataStore, applies velocity to the player's rigid body,
//! handles jumping (with ground detection via downward raycast), and syncs
//! the ECS `Transform` from the physics body position each tick.

use crate::ecs::components::{Controllable, PhysicsBody, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::input::InputState;
use crate::physics::PhysicsWorld;
use glam::Vec3;

/// Movement tuning constants (will be data-driven via config/physics.toml later).
const MOVE_SPEED: f32 = 6.0;
const JUMP_IMPULSE: f32 = 5.0;
/// Raycast distance below the player's origin to detect ground contact.
const GROUND_CHECK_DIST: f32 = 1.1;

/// Drives the player entity based on keyboard/mouse input and physics.
pub struct PlayerControllerSystem;

impl System for PlayerControllerSystem {
    fn name(&self) -> &str {
        "PlayerController"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        // Fetch shared resources from the DataStore.
        let input = match data.get::<InputState>("input_state") {
            Some(i) => i.clone(),
            None => return, // no input yet — nothing to do
        };

        let physics = match data.get::<PhysicsWorld>("physics_world") {
            Some(p) => p,
            None => return, // physics not initialized
        };

        // Find the entity the player is controlling.
        // Safety: we need mutable access to Transform AND read access to PhysicsBody
        // and Controllable. Collect handles first to satisfy borrow checker.
        let mut player_data: Option<(hecs::Entity, rapier3d::dynamics::RigidBodyHandle)> = None;

        for (entity, (_ctrl, pb)) in world.query::<(&Controllable, &PhysicsBody)>().iter() {
            player_data = Some((entity, pb.handle));
            break; // only one controllable entity at a time
        }

        let (entity, rb_handle) = match player_data {
            Some(d) => d,
            None => return, // no player entity spawned
        };

        // Build desired horizontal velocity from input.
        // Use the camera's yaw to orient movement relative to where the player is looking.
        let yaw = data
            .get::<f32>("camera_yaw")
            .copied()
            .unwrap_or(0.0);

        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let forward_dir = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
        let right_dir = Vec3::new(cos_yaw, 0.0, -sin_yaw);

        let mut move_dir = Vec3::ZERO;
        if input.forward {
            move_dir += forward_dir;
        }
        if input.backward {
            move_dir -= forward_dir;
        }
        if input.right {
            move_dir += right_dir;
        }
        if input.left {
            move_dir -= right_dir;
        }

        // Normalize so diagonal movement isn't faster.
        if move_dir.length_squared() > 0.0 {
            move_dir = move_dir.normalize();
        }

        let desired_horizontal = move_dir * MOVE_SPEED;

        // Preserve vertical velocity from physics (gravity, jumps).
        let current_vel = physics
            .get_velocity(rb_handle)
            .unwrap_or(Vec3::ZERO);

        let mut new_vel = Vec3::new(desired_horizontal.x, current_vel.y, desired_horizontal.z);

        // Ground detection via downward raycast.
        let pos = physics
            .get_position(rb_handle)
            .unwrap_or(Vec3::ZERO);
        let grounded = physics
            .cast_ray(pos, Vec3::NEG_Y, GROUND_CHECK_DIST)
            .is_some();

        // Jump when grounded and Space is pressed.
        if input.jump && grounded {
            new_vel.y = JUMP_IMPULSE;
        }

        // We can't mutate physics through a shared ref — store velocity command
        // in DataStore for the physics sync step to apply. The caller is responsible
        // for applying this before stepping physics. Alternatively, if PhysicsWorld
        // were stored as a mutable resource, we'd set it directly.
        // For now, we store the desired velocity to be applied externally.
        //
        // NOTE: The System trait gives us `&DataStore` (immutable), so we store the
        // command as a simple struct. The engine loop applies it before physics.step().
        // This is a deliberate architectural choice — systems are pure readers of
        // shared state, and the engine loop mediates mutations.

        // Store the computed velocity command. The engine loop reads "player_velocity_cmd"
        // and calls physics.set_velocity() before stepping.
        // (DataStore is immutable here, so we sync Transform from last frame's physics.)

        // Sync Transform from physics body position (read from last frame's step).
        if let Ok(mut transform) = world.get::<&mut Transform>(entity) {
            transform.position = pos;
            if let Some(rot) = physics.get_rotation(rb_handle) {
                transform.rotation = rot;
            }
        }

        // Store movement intent for the engine loop to apply.
        // Since DataStore is &-only in System::tick, the engine loop must
        // pre-insert a mutable channel. We use a simple pattern: write to a
        // well-known interior-mutable slot.
        //
        // For now, we use the existing velocity component as the output channel.
        if let Ok(mut vel_comp) = world.get::<&mut crate::ecs::components::Velocity>(entity) {
            vel_comp.linear = new_vel;
        }
    }
}
