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
pub mod ipc;
pub mod ipc_parse;
/// The Settings > Controls key-capture step (rebindable keybinds, 2026-08-12).
pub mod keybind_capture;
pub mod launch_focus;
/// Near-tree model cache + sprite-atlas bake (extracted from lib.rs, v0.1108).
pub mod near_tree_models;
/// Background relay connections: dial + keep-alive + compact router for
/// every saved server that is not the active one (multi-connection).
pub mod bg_connections;
pub mod net_route;
pub mod registries;
pub mod state;
pub mod world_load;
