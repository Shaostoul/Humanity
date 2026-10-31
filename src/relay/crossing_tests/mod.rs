//! TESTS OF THE CRATES BELOW, TOGETHER WITH THE SERVER (the crate split, 2026-10-31,
//! docs/design/crate-split.md, "Not counted, and needed before each crate is cut").
//!
//! Each of these was written beside the code it tests, in a crate under the server (the protocol,
//! the simulation), but reaches into the server to do it: a frame the server's message type must
//! read, the meal counter's rules that must be the server's. Once the crates are cut a crate's tests
//! cannot reach a crate above it, so each moved up WHOLE to the lowest crate that may use
//! everything it reaches, which for these is the server. Nothing in a test changed but the paths it
//! names things by. One file per file they came from, named for it; the comment at the top of each
//! says where. `node scripts/check-crate-layers.js` (in `just lints`) holds test code to the plan
//! the way it holds code, so a test cannot reach up again.

mod profile_copy;
mod ship_meals;
