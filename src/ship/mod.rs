//! Ship interior system — layout parsing and room mesh generation.
//!
//! Ships are fleet vessels where players live, work, and travel between planets.
//! Layouts are defined in RON data files under `data/ships/`.

pub mod conduits;
pub mod door_panels;
pub mod fibonacci;
pub mod home_structure;
pub mod hull;
pub mod layout;
pub mod lock_types;
pub mod rooms;
pub mod ship_structure;
pub mod structure;
pub mod wall_collision;
pub mod room_types;
