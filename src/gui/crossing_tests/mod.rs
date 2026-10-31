//! TESTS OF THE CRATES BELOW, TOGETHER WITH THE GUI OR WITH EACH OTHER (the crate split,
//! 2026-10-31, docs/design/crate-split.md, "Not counted, and needed before each crate is cut").
//!
//! Each of these was written beside the code it tests, in a crate under the GUI, but reaches a
//! crate it may not use to do it: into the GUI itself (the settings, a page's list), or across to
//! a crate beside its own (the network side and the server, which must agree on a word or a frame;
//! the ship or the simulation and the renderer, whose constants and shaders they check). Once the
//! crates are cut a crate's tests cannot reach a crate above or beside it, so each moved up WHOLE
//! to the lowest crate that may use everything it reaches, which for these is the GUI (the engine's
//! `crossing_tests` holds the ones that reach the engine too). Nothing in a test changed but the
//! paths it names things by. One file per file they came from, named for it; the comment at the
//! top of each says where. `node scripts/check-crate-layers.js` (in `just lints`) holds test code
//! to the plan the way it holds code, so a test cannot reach up again.
//!
//! A test that checks a lower crate against the server (`relay`) leaves the relay's own test
//! battery (`just verify-relay`) by coming here: the server build no longer compiles the crate it
//! also checks. It still runs in every `just test-lib`.

mod assets;
mod env_layer1;
mod far_trees;
mod files_policy;
mod food_spoilage;
mod how_to;
mod net_household;
mod net_live;
mod reports;
mod ship_lighting;
mod ship_moves;
mod surface_move;
mod wounds;
