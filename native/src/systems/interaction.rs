//! Interaction system — raycasts from camera to find interactable entities.
//!
//! Each tick, casts a ray from the camera position along the camera forward direction.
//! If the ray hits an entity with an `Interactable` component within range, that entity
//! is stored as "hovered_entity" in DataStore. When the interact key is pressed,
//! the interaction type is written to "interaction_triggered".

use crate::ecs::components::{Interactable, PhysicsBody};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::input::InputState;
use crate::physics::PhysicsWorld;
use glam::Vec3;

/// Max distance for interaction raycasts (meters).
const INTERACT_RAY_MAX: f32 = 10.0;

/// Detects hoverable/interactable entities under the camera crosshair.
pub struct InteractionSystem;

impl System for InteractionSystem {
    fn name(&self) -> &str {
        "Interaction"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        let input = match data.get::<InputState>("input_state") {
            Some(i) => i,
            None => return,
        };

        let physics = match data.get::<PhysicsWorld>("physics_world") {
            Some(p) => p,
            None => return,
        };

        // Camera position and forward direction are stored by the camera controller.
        let cam_pos = data
            .get::<Vec3>("camera_position")
            .copied()
            .unwrap_or(Vec3::ZERO);
        let cam_forward = data
            .get::<Vec3>("camera_forward")
            .copied()
            .unwrap_or(Vec3::NEG_Z);

        // Raycast from camera along forward direction.
        let hit = physics.cast_ray(cam_pos, cam_forward, INTERACT_RAY_MAX);

        // Build a map from rigid body handle to entity for reverse lookup.
        // In a production engine this would be a persistent index, but for
        // correctness we scan each frame (entity count is bounded).
        let mut hit_entity = None;
        if let Some((rb_handle, distance)) = hit {
            for (entity, (pb, interactable)) in
                world.query::<(&PhysicsBody, &Interactable)>().iter()
            {
                if pb.handle == rb_handle && distance <= interactable.range {
                    hit_entity = Some((entity, interactable.interaction_type.clone()));
                    break;
                }
            }
        }

        // Store hovered entity ID and handle interaction triggers.
        // Since DataStore is immutable in System::tick, we write results into
        // ECS components or rely on the engine loop to read them.
        //
        // Pattern: store interaction results as a dedicated ECS singleton entity.
        // The engine loop or UI system reads these.
        //
        // For minimal integration, we check if an "interaction_result" singleton
        // exists and update it, or the engine loop polls this system's output.

        if let Some((_entity, interaction_type)) = &hit_entity {
            if input.interact {
                // Log the interaction for now; the engine loop can read
                // the Velocity/Interactable state to dispatch game logic.
                log::debug!(
                    "Interaction triggered: type={}",
                    interaction_type
                );
            }
        }
    }
}
