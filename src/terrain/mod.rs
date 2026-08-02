//! Terrain systems: icosphere planets and voxel asteroids.
//!
//! Planets use recursive icosphere subdivision for LOD.
//! Asteroids use sparse octree voxel volumes.
//! Both environments are procedurally generated from seed data.

pub mod asteroid;
pub mod heightmap;
pub mod icosphere;
pub mod planet;
pub mod planet_albedo;
pub mod planet_chunks;
/// Near-field grass strands (extracted from `planet_chunks`, v0.1092).
/// `planet_chunks` glob re-exports it, so both paths resolve.
pub mod grass;
/// The DRAWN patch surface: where the ground TRIANGLE sits, as opposed to
/// what the elevation field says (promoted out of `grass`, v0.1097 - grass
/// tillers and near-field trees both stand on it).
pub mod drawn_surface;
pub mod far_trees;
pub mod ocean_fft;
pub mod ocean_mask;
pub mod ocean_waves;
pub mod terrain_tiles;
pub mod planet_heightmap;
pub mod planet_registry;
pub mod procedural_heightmap;
pub mod planet_surface;
