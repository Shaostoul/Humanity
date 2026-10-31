//! TESTS OF THE CRATES BELOW, TOGETHER WITH THE ENGINE (the crate split, 2026-10-31,
//! docs/design/crate-split.md, "Not counted, and needed before each crate is cut").
//!
//! Each of these was written beside the code it tests, in a crate under the engine (the GUI, the
//! simulation, the ship, the network side), but reaches into the engine to do it: the save and
//! load, the footing, the frame's routing of the network, the welcome. Once the crates are cut a
//! crate's tests cannot reach a crate above it, so each moved up WHOLE to the lowest crate that may
//! use everything it reaches, which for these is the engine. Nothing in a test changed but the
//! paths it names things by. One file per file they came from, named for it; the comment at the top
//! of each says where. `node scripts/check-crate-layers.js` (in `just lints`) holds test code to
//! the plan the way it holds code, so a test cannot reach up again.
//!
//! A test that reaches two crates neither of which may use the other (the network side and the
//! server, the ship and the renderer) goes to the lowest crate over both: the GUI's
//! `crossing_tests` for most, and only here when the engine is among them.

mod block;
mod chat_connect_form;
mod food_spoilage;
mod machines;
mod main_menu_first_session;
mod maps;
mod net_sync;
mod put_answer_parity;
mod quests_opening;
mod reach;
mod ship_sight;
mod ship_structure;
mod surface_move_stand_still;
mod surface_walk_drawn_ground;
mod trade;
mod ui_snapshots;
mod wall_collision_homestead_walk;
mod wall_collision_placed_piece_walk;
mod wounds;
