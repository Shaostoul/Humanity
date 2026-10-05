//! Ship interior system — layout parsing and room mesh generation.
//!
//! Ships are fleet vessels where players live, work, and travel between planets.
//! Layouts are defined in RON data files under `data/ships/`.

pub mod assembly;
/// Where a piece built in the shared world is kept: a plot (`plot:p3`) or a shared space
/// (`zone:commons`), and what lies inside one (increment 5 of
/// docs/design/ship-homes-and-logistics.md).
pub mod build_frames;
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
/// Moving aboard the shared ship: the relay's speed rules, how a fast move is declared, who is in
/// view (increment 4 of docs/design/ship-homes-and-logistics.md, data/ship/shared_world.ron).
pub mod moves;
pub mod neighbours;
/// Where a point is in the ship: aboard or not (its bounds), and whose air it breathes (a home's
/// own, the ship's shared spaces', or none) (increment 4).
pub mod ship_space;
/// Transit links: a teleporter and its partner, by stable ids (increment 4).
pub mod transit;
pub mod rooms;
pub mod ship_structure;
pub mod structure;
pub mod wall_collision;
pub mod room_types;
