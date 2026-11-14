//! humanity-terrain: planets as shapes and numbers (the crate split, docs/design/crate-split.md; its
//! card is docs/crates/humanity-terrain.md). The planet's surface and heights, the level-of-detail
//! patches and their meshes as plain data, the oceans and water, the plants' and trees' geometry,
//! real map regions, asteroids and the celestial projection. It draws nothing: the renderer uploads
//! what it builds and the simulation reads its heights.
//!
//! It uses only humanity-core. Cut out of the one crate in rung 2 (2026-11-02): what was the
//! `terrain` module is the crate root (`humanity_terrain::planet::PlanetDef`). Feature flags:
//! `native` (the desktop app's parts; the root's `native` turns it on) and `test-support` (the
//! fixtures other crates' tests use, turned on only by their [dev-dependencies]).
//!
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
/// The tree card stage's supply (the depth floor the selection refines to within the card
/// distance, so the stage exists without the tile tier) and its per-frame counter (BUG-301).
pub mod tree_card_stage;
/// The tree species list (data/vegetation/trees.ron), the card atlas's tile for each species
/// and variant, and each tile's framing: plain data the patch builder and the renderer's tree
/// builder both read (the top of `renderer::tree_mesh` until the crate split's rung 0).
pub mod tree_registry;
/// Procedural plant geometry (crops, fungi, grass tillers, the trees' builder): plain
/// `Vec<Vertex>`, here since the crate split's rung 0 (it was `renderer::plant_mesh`).
pub mod plant_mesh;
/// The near-tree harvest's re-run gate (moved / LOD changed / density
/// changed, the change terms rate-limited), pure so it is testable without
/// a planet and so the frame loop's `[NearTree] recompute` log can name
/// its reason (2026-09-18, the parked-camera log flood).
pub mod near_tree_gate;
/// OpenStreetMap features around an origin, cut from the whole-Earth map packs
/// (`region_from_pack`): the local metric frame, a polygon ear clipper, and the
/// 3D extrusion mesher. Shared by the Maps page's 2D Planet view and the
/// in-world districts so the two cannot drift (v0.1148; packs since whole-Earth
/// maps rung 2). Conversion/projection/clipper are relay-safe; only the mesher
/// is native-gated.
pub mod osm_region;
/// Map tiles keyed to the terrain's own quadtree (whole-Earth maps rung 1,
/// docs/design/whole-earth-maps.md): a tile's id is a `PatchId`; the level
/// for a depth, the ancestor tile by mask, the tile containing a point, the
/// nested height lattice, the great-circle clipper with clip-edge flags and
/// OSM's 10^7-degree codec. Relay-safe, pure std + glam.
pub mod map_tiles;
/// The whole-Earth map pack format, HOSMPAK1 (rung 2): one file per depth-6 patch, every layer
/// cut into the data levels' tiles by the same key, with each layer's source, date and licence
/// and the notices the licences ask for. Reader and writer side by side. Relay-safe.
pub mod map_pack;
/// The map's data files: `data/maps/levels.ron` (levels, lattice, road classes, the shore band's
/// numbers) and `data/maps/places.ron` (named places and their gates). Relay-safe.
pub mod map_levels;
/// The offline map pack builder (our own: OSM PBF, GEDTM30, WorldCover and ETOPO to packs). Built
/// only for its tests and for the `build_map_packs` example (`map-build` feature): never in the app.
#[cfg(any(test, feature = "map-build"))]
pub mod map_build;
/// The installed map packs drawn into the one ground formula (whole-Earth maps rung 2): GEDTM30
/// land on the nested lattice, OSM's coastline deciding land from water, the shore band joining
/// them to ETOPO's seabed, tile residency riding the patches, and the built-over mask the
/// vegetation gates read. Replaced the region water carve (v0.1149 to rung 1). Relay-safe.
pub mod map_ground;
/// Analytic ocean disaster fields (tsunami, rogue wave, maelstrom,
/// hurricane) - ABYSSAL adoption arc rung 1 (docs/reference/
/// abyssal-ocean-weather.md). Pure math, relay-safe; the WGSL twin and the
/// event lifecycle arrive at rungs 2-3.
pub mod ocean_events;
pub mod ocean_fft;
pub mod ocean_mask;
pub mod ocean_waves;
pub mod terrain_tiles;
pub mod planet_heightmap;
// planet_registry was deleted 2026-08-12 (artificial-planet increment 5):
// a circular-orbit registry that was never instantiated anywhere. The
// canonical body catalog is crates/humanity-core/src/cosmos.rs (Kepler, data-driven from
// data/star_systems/sol.json); per-planet terrain defs are planet.rs.
pub mod procedural_heightmap;
pub mod planet_surface;
/// The celestial pass's projection (reverse-Z, 5 cm to 1e13 m) the patch selector's frustum is
/// built from: `renderer::camera`'s until the crate split's rung 0.
pub mod celestial_projection;

// Tests of the crates below that reach terrain to do it, moved up from beside their code (the crate
// split, 2026-10-31, docs/design/crate-split.md): crates/humanity-terrain/src/crossing_tests.rs.
#[cfg(test)]
mod crossing_tests;
