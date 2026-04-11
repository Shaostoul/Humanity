//! Interaction system — raycasts from camera to find interactable entities.
//!
//! Each tick, casts a ray from the camera position along the camera forward direction.
//! If the ray hits an entity with an `Interactable` component within range, that entity
//! becomes the current target and a prompt string is written to DataStore for the HUD.

use crate::ecs::components::{Interactable, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::input::InputState;
use glam::Vec3;
use std::sync::Mutex;

/// Default max interaction distance (meters).
const INTERACT_RANGE: f32 = 3.0;

/// Detects hoverable/interactable entities under the camera crosshair.
pub struct InteractionSystem {
    pub current_target: Option<hecs::Entity>,
    pub prompt_text: String,
}

impl InteractionSystem {
    pub fn new() -> Self {
        Self { current_target: None, prompt_text: String::new() }
    }
}

impl System for InteractionSystem {
    fn name(&self) -> &str {
        "Interaction"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        let cam_pos = data.get::<Vec3>("camera_position").copied().unwrap_or(Vec3::ZERO);
        let cam_dir = data.get::<Vec3>("camera_forward").copied().unwrap_or(Vec3::NEG_Z);

        // Find nearest interactable along the camera ray (sphere-vs-ray).
        let mut best: Option<(hecs::Entity, f32, String)> = None;
        for (entity, (tf, inter)) in world.query::<(&Transform, &Interactable)>().iter() {
            let range = inter.range.min(INTERACT_RANGE);
            let to_entity = tf.position - cam_pos;
            let along_ray = to_entity.dot(cam_dir);
            if along_ray < 0.0 || along_ray > range { continue; } // behind or too far
            let perp_dist = (to_entity - cam_dir * along_ray).length();
            if perp_dist > 1.0 { continue; } // too far off center
            if best.as_ref().map_or(true, |b| along_ray < b.1) {
                best = Some((entity, along_ray, inter.interaction_type.clone()));
            }
        }

        if let Some((entity, _, label)) = best {
            self.current_target = Some(entity);
            self.prompt_text = format!("Press E to {}", label);
        } else {
            self.current_target = None;
            self.prompt_text.clear();
        }

        // Write prompt to DataStore so the HUD can display it.
        if let Some(prompt) = data.get::<Mutex<String>>("interaction_prompt") {
            if let Ok(mut s) = prompt.lock() {
                s.clone_from(&self.prompt_text);
            }
        }

        // Handle interaction trigger.
        if self.current_target.is_some() {
            if let Some(input) = data.get::<InputState>("input_state") {
                if input.interact {
                    log::debug!("Interaction triggered: {}", self.prompt_text);
                }
            }
        }
    }
}
