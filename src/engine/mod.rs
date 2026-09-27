//! Satellites of the native app's main module, extracted from lib.rs
//! (v0.932, tiers A+B of docs/dev/code-structure-plan.md). Everything here is
//! self-contained: pure math and parsers, plus loaders that take only
//! `DataStore` / `hecs::World`, never `EngineState`. lib.rs glob-imports these
//! inside `mod native_app`, so call sites are unchanged. Later tiers (IPC
//! pollers, frame-lock math, the editor cluster) land beside them as they
//! extract.

pub mod color;
pub mod dm;
pub mod editor;
pub mod frame_lock;
pub mod geom;
pub mod home_meshes;
pub mod home_spawn;
/// The F10 sidebar's key rules (F10 toggle, Escape-closes-first), the
/// cursor-free predicate and the flag-by-name lookup the dev IPC reports
/// (2026-09-18). Pure GuiState logic: the key, the tests and the IPC share it.
pub mod input;
pub mod ipc;
pub mod ipc_parse;
/// The Settings > Controls key-capture step (rebindable keybinds, 2026-08-12).
pub mod keybind_capture;
pub mod launch_focus;
/// How a mushroom crop is drawn: its fruiting blocks or cased bed, and the
/// mushrooms on them by stage (2026-09-27).
pub mod fungus_mesh;
/// Where a garden plot's plants stand: plots of a machine, the crop's rows at
/// its real spacing, and the clump rule above the visual cap (2026-09-26).
pub mod plant_layout;
/// The garden's plant geometry, built on a worker thread and uploaded a few
/// milliseconds a frame (2026-09-27).
pub mod plant_pass;
/// Near-tree model cache + sprite-atlas bake (extracted from lib.rs, v0.1108).
pub mod near_tree_models;
/// One frame's relay WebSocket message pump: every `type` the relay can send,
/// plus the socket-died teardown (extracted from lib.rs, v0.1320).
pub mod frame_ws_poll;
/// The planet's two water shells (coarse backstop + displaced wave surface),
/// built and pushed once per frame (extracted from lib.rs, v0.1320).
pub mod frame_water;
/// Near-field 3D trees: the gated harvest, the per-tree draw plan, and the
/// card-hide promise the far LOD depends on (extracted from lib.rs, v0.1320).
pub mod frame_near_trees;
/// A planet's cloud deck and atmosphere dome, plus the view-dependent order
/// the two composite in (extracted from lib.rs, v0.1320).
pub mod frame_shells;
pub mod region_meshes;
/// Background relay connections: dial + keep-alive + compact router for
/// every saved server that is not the active one (multi-connection).
pub mod bg_connections;
/// Built beds and chests in use: the crosshair prompt, the E press, and
/// built chests as containers in the places tree (2026-09-27).
pub mod built_uses;
/// Placing a built piece: the ghost that follows the crosshair, R to turn
/// it, E to build it there, Esc to stop (2026-09-27).
pub mod build_place;
/// The survival environment context (the home's air or the weather) and the
/// body heat mode, published once a frame (moved out of lib.rs 2026-09-27).
pub mod survival_env;
pub mod net_route;
pub mod registries;
/// In-world screens: native pages on flat displays placed in the 3D world
/// (quads, look-ray hit tests, input routing, per-frame surface drawing).
pub mod screens;
pub mod state;
/// Visible storage: crates in the storage zones and tank level bars.
pub mod stock_piles;
pub mod world_load;
