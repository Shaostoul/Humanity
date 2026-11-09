//! TESTS OF THE SHIP AND TERRAIN, TOGETHER WITH THE SIMULATION (the crate split, 2026-10-31,
//! docs/design/crate-split.md, "Not counted, and needed before each crate is cut").
//!
//! Each of these was written beside the code it tests, in the ship or terrain crate under the
//! simulation, but reaches into the simulation to do it: the shipped homes and starts against the
//! recipes, quests and items; a frame's placements against the build rules; a door's halves against
//! how a door moves; the station's sky against the home's clock; the plant models against the
//! plants. Once the crates are cut a crate's tests cannot reach a crate above it, so each moved up
//! WHOLE to the lowest crate that may use everything it reaches, which for these is the simulation.
//! Nothing in a test changed but the paths it names things by. One file per file they came from,
//! named for it; the comment at the top of each says where. `node scripts/check-crate-layers.js`
//! (in `just lints`) holds test code to the plan the way it holds code, so a test cannot reach up
//! again.

mod build_frames;
mod door_panels;
mod machines;
mod plant_mesh;
mod starts;
mod station_orbit;
mod drawn_surface_pack;
