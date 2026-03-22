//! Terrain systems: icosphere planets and voxel asteroids.
//!
//! Planets use recursive icosphere subdivision for LOD.
//! Asteroids use sparse octree voxel volumes.
//! Both environments are procedurally generated from seed data.

pub mod asteroid;
pub mod icosphere;
pub mod planet;
