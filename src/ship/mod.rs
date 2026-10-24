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
/// The ship's levels (its distinct storey floors), and the ship cut away at one (Maps increment 3:
/// the Maps page's level picker, cutaway, room plan and plan tiles read one list).
pub mod levels;
/// The homestead's upper floor checked as the shipped design carries it (ship homes increment 7).
#[cfg(test)]
mod homestead_layout_tests;
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
/// Where on a storey of a home a person can stand: a floor with room over it, inside the rooms,
/// never a glass lid or an opening (ship homes increment 7; the build editor's close).
pub mod standing;
/// The starts as data (increment 6, data/homes/starts.ron): Cabin, Apartment, Homestead, what
/// each builds, who lives there and what a new character carries; and the `game_home` wire.
pub mod starts;
pub mod structure;
/// What the eye can see aboard, in three dimensions: the walk-up target and the HUD's labels
/// (ship homes increment 7 review). Walls block across their own heights, solid slabs block.
pub mod sight;
pub mod wall_collision;
pub mod room_types;
