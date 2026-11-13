//! humanity-ship: the home and the ship as places a person stands in (the crate split,
//! docs/design/crate-split.md; its card is docs/crates/humanity-ship.md). The home's walls, doors,
//! storeys and rooms, the ship's zones, corridors and hull, where a person can stand, see and walk
//! aboard, the starts a new player chooses, the machines a home holds (`machines`) and the
//! homestead station's orbit (`station`): plain data and geometry, no GPU, compiled into the server
//! too.
//!
//! Ships are fleet vessels where players live, work, and travel between planets. Layouts are defined
//! in RON data files under `data/ships/`.
//!
//! It uses only humanity-core. Cut out of the one crate in rung 2 (2026-11-02): what was the `ship`
//! module is the crate root (`humanity_ship::ship_structure`), and `machines` and `station` kept
//! their names under it (`humanity_ship::machines::MachineHome`). Feature flags: `native` (the
//! desktop app's parts; the root's `native` turns it on) and `test-support` (the fixtures other
//! crates' tests use, turned on only by their [dev-dependencies]).

pub mod assembly;
/// Where a piece built in the shared world is kept: a plot (`plot:p3`) or a shared space
/// (`zone:commons`), and what lies inside one (increment 5 of
/// docs/design/ship-homes-and-logistics.md).
pub mod build_frames;
pub mod conduits;
/// What a pipe, hose or cable is made of, as drawn today: today's four conduit kinds over the one
/// material catalog (`humanity_core::run_materials`, data/piping/pipe_materials.ron).
pub mod pipe_materials;
/// The "to code" rules for runs and the profiles that group them (data/piping/run_rules.ron;
/// spline routing rung 0, 2026-11-02). Data only until rung 8 evaluates them.
pub mod run_rules;
/// Fittings for runs: elbows, drain bends, tees, couplings, glands... (data/piping/fittings.ron).
pub mod run_fittings;
/// Anchors that hold runs: straps, hangers, J-hooks, trays, ramps (data/piping/anchors.ron).
pub mod run_anchors;
/// Benders and the radius each bends at (data/piping/benders.ron).
pub mod run_benders;
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
/// The ship's elevators with their stops (Lift, LiftKey, `lifts`): what the footing runs and the
/// HUD names.
pub mod lifts;
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
/// What a storey edit does to a home, with no editor around it (the B editor's pure half, ship homes
/// increment 7): the storey edited, its name, a new wall or floor slab, the cut-away's box.
pub mod storey_edits;
/// The starts as data (increment 6, data/homes/starts.ron): Cabin, Apartment, Homestead, what
/// each builds, who lives there and what a new character carries; and the `game_home` wire.
pub mod starts;
pub mod structure;
/// What the eye can see aboard, in three dimensions: the walk-up target and the HUD's labels
/// (ship homes increment 7 review). Walls block across their own heights, solid slabs block.
pub mod sight;
pub mod wall_collision;
/// People who walk somewhere, through doors, round furniture and each other (crowd navigation
/// rung 1, 2026-11-13, docs/design/crowd-navigation.md): the navigation mesh built from the walls,
/// doors, floors and furniture, the path over it, and the avoidance between people.
pub mod crowd_nav;
pub mod room_types;

// Data-driven machine layout for the 3D home (pure data, relay-safe). The renderer
// placement lives in load_world (native); the structs + RON loader compile everywhere.
pub mod machines;
/// Crewed stations: the homestead as a real orbiting, pointed object.
pub mod station;

// Tests of the crates below that reach the ship to do it, moved up from beside their code (the crate
// split, 2026-10-31, docs/design/crate-split.md): crossing_tests.rs.
#[cfg(test)]
mod crossing_tests;
