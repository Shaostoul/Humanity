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
/// Real OpenStreetMap regions (data/maps/regions/*.bin): the HOSMREG1
/// reader, the fetcher's projection contract, a polygon ear clipper, and the
/// 3D extrusion mesher. Shared by the Maps page's 2D Planet view and the
/// in-world extruder so the two cannot drift (v0.1148, maps ladder rung 3
/// increment 2). Parser/projection/clipper are relay-safe; only the mesher
/// is native-gated.
pub mod osm_region;
pub mod ocean_fft;
pub mod ocean_mask;
pub mod ocean_waves;
pub mod terrain_tiles;
pub mod planet_heightmap;
// planet_registry was deleted 2026-08-12 (artificial-planet increment 5):
// a circular-orbit registry that was never instantiated anywhere. The
// canonical body catalog is src/cosmos.rs (Kepler, data-driven from
// data/star_systems/sol.json); per-planet terrain defs are planet.rs.
pub mod procedural_heightmap;
pub mod planet_surface;
