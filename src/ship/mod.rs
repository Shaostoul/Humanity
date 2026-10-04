//! Ship interior system — layout parsing and room mesh generation.
//!
//! Ships are fleet vessels where players live, work, and travel between planets.
//! Layouts are defined in RON data files under `data/ships/`.

pub mod assembly;
pub mod conduits;
/// What a pipe, hose or cable is made of, as data (2026-10-04, data/piping/pipe_materials.ron).
pub mod pipe_materials;
/// The coloured marker bands that say what flows in a pipe, scheme by scheme (2026-10-04,
/// data/piping/marking_schemes.ron; docs/reference/findings/2026-10-04-pipe-marking-standards.md).
pub mod pipe_marking;
pub mod door_panels;
pub mod door_points;
pub mod fibonacci;
pub mod home_structure;
pub mod hull;
pub mod layout;
pub mod lock_types;
pub mod neighbours;
pub mod rooms;
pub mod ship_structure;
pub mod structure;
pub mod wall_collision;
pub mod room_types;
