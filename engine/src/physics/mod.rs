//! Physics — Rapier3d world setup and stepping.
//!
//! Wraps the full Rapier3d pipeline: rigid bodies, colliders, joints, raycasting.
//! Physics config loaded from `config/physics.toml`.

pub mod fluid;
pub mod collision;

use glam::Vec3;
use rapier3d::prelude::*;

/// Wraps the complete Rapier3d physics pipeline.
pub struct PhysicsWorld {
    pub gravity: nalgebra::Vector3<f32>,
    pub integration_parameters: IntegrationParameters,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub physics_pipeline: PhysicsPipeline,
    pub query_pipeline: QueryPipeline,
}

impl PhysicsWorld {
    /// Create a new physics world with Earth-like gravity (-9.81 m/s^2 on Y).
    pub fn new() -> Self {
        Self {
            gravity: nalgebra::vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            physics_pipeline: PhysicsPipeline::new(),
            query_pipeline: QueryPipeline::new(),
        }
    }

    /// Advance the physics simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
    }

    /// Add a rigid body to the world, returns its handle.
    pub fn add_rigid_body(&mut self, rb: RigidBodyBuilder) -> RigidBodyHandle {
        self.rigid_body_set.insert(rb.build())
    }

    /// Attach a collider to an existing rigid body.
    pub fn add_collider(
        &mut self,
        parent: RigidBodyHandle,
        collider: ColliderBuilder,
    ) -> ColliderHandle {
        self.collider_set
            .insert_with_parent(collider.build(), parent, &mut self.rigid_body_set)
    }

    /// Add a collider without a parent rigid body (static geometry).
    pub fn add_static_collider(&mut self, collider: ColliderBuilder) -> ColliderHandle {
        self.collider_set.insert(collider.build())
    }

    /// Cast a ray and return the first hit (rigid body handle + distance along ray).
    pub fn cast_ray(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_dist: f32,
    ) -> Option<(RigidBodyHandle, f32)> {
        let ray = Ray::new(
            nalgebra::point![origin.x, origin.y, origin.z],
            nalgebra::vector![direction.x, direction.y, direction.z],
        );

        self.query_pipeline
            .cast_ray(
                &self.rigid_body_set,
                &self.collider_set,
                &ray,
                max_dist,
                true, // solid
                QueryFilter::default(),
            )
            .and_then(|(collider_handle, toi)| {
                let collider = self.collider_set.get(collider_handle)?;
                let rb_handle = collider.parent()?;
                Some((rb_handle, toi))
            })
    }

    /// Get the world-space position of a rigid body.
    pub fn get_position(&self, handle: RigidBodyHandle) -> Option<Vec3> {
        let rb = self.rigid_body_set.get(handle)?;
        let pos = rb.translation();
        Some(Vec3::new(pos.x, pos.y, pos.z))
    }

    /// Get the world-space rotation of a rigid body as a glam Quat.
    pub fn get_rotation(&self, handle: RigidBodyHandle) -> Option<glam::Quat> {
        let rb = self.rigid_body_set.get(handle)?;
        let rot = rb.rotation();
        Some(glam::Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w))
    }

    /// Set the linear velocity of a rigid body.
    pub fn set_velocity(&mut self, handle: RigidBodyHandle, velocity: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.set_linvel(nalgebra::vector![velocity.x, velocity.y, velocity.z], true);
        }
    }

    /// Get the linear velocity of a rigid body.
    pub fn get_velocity(&self, handle: RigidBodyHandle) -> Option<Vec3> {
        let rb = self.rigid_body_set.get(handle)?;
        let vel = rb.linvel();
        Some(Vec3::new(vel.x, vel.y, vel.z))
    }

    /// Apply an impulse to a rigid body (instantaneous force).
    pub fn apply_impulse(&mut self, handle: RigidBodyHandle, impulse: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.apply_impulse(nalgebra::vector![impulse.x, impulse.y, impulse.z], true);
        }
    }

    /// Remove a rigid body and all its attached colliders.
    pub fn remove_rigid_body(&mut self, handle: RigidBodyHandle) {
        self.rigid_body_set.remove(
            handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true, // remove attached colliders
        );
    }
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}
