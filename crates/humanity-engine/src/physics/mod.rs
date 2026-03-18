//! Physics — Rapier3d world setup and stepping.
//!
//! Physics config loaded from `config/physics.toml`.

pub mod fluid;
pub mod collision;

use rapier3d::prelude::*;

/// Wraps the Rapier3d physics pipeline.
pub struct PhysicsWorld {
    pub gravity: nalgebra::Vector3<f32>,
    pub integration_parameters: IntegrationParameters,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            gravity: nalgebra::vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
        }
    }
}
