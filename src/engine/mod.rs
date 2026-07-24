//! Satellites of the native app's main module, extracted from lib.rs
//! (v0.932, tiers A+B of docs/dev/code-structure-plan.md). Everything here is
//! self-contained: pure math and parsers, plus loaders that take only
//! `DataStore` / `hecs::World`, never `EngineState`. lib.rs glob-imports these
//! inside `mod native_app`, so call sites are unchanged. Later tiers (IPC
//! pollers, frame-lock math, the editor cluster) land beside them as they
//! extract.

pub mod color;
pub mod dm;
pub mod geom;
pub mod home_spawn;
pub mod ipc_parse;
pub mod registries;
