//! The names every frame stage uses (the file-size split, step 1, 2026-11-14,
//! docs/design/crate-split.md "The file-size split").
//!
//! The frame stages (`src/engine/frame_*.rs`) are code moved out of `window_event` in src/lib.rs
//! as it was, so they call things by the bare names `mod native_app` imports there. Each stage
//! glob-imports this module instead of repeating that import list: the same names, in one place.
//! A glob import never warns for the names a stage does not use.

#![allow(unused_imports)]

pub(crate) use glam::{Quat, Vec3};
pub(crate) use crate::engine::{
    color::*, dm::*, editor::*, frame_lock::*, geom::*, home_meshes::*, home_spawn::*, ipc::*, ipc_parse::*, near_tree_models::*, net_route::*, registries::*, world_load::*,
    state::*,
};
pub(crate) use crate::engine::frame_vars::FrameVars;
pub(crate) use crate::assets::AssetManager;
pub(crate) use humanity_core::ecs::GameWorld;
pub(crate) use humanity_core::ecs::components::{Controllable, Health, Name, Transform};
pub(crate) use humanity_core::ecs::systems::SystemRunner;
pub(crate) use crate::gui::{GuiGameTime, GuiItemSlot, GuiPage, GuiState, GuiWeather};
pub(crate) use crate::gui::pages::{
    main_menu, escape_menu, chat, hud,
};
pub(crate) use crate::gui::widgets::help_modal;
pub(crate) use humanity_core::hot_reload::HotReloadCoordinator;
pub(crate) use humanity_core::hot_reload::data_store::DataStore;
pub(crate) use humanity_core::input::InputState;
pub(crate) use crate::renderer::camera::{Camera, CameraController};
pub(crate) use crate::renderer::mesh::Mesh;
pub(crate) use crate::renderer::{RenderObject, Renderer};
pub(crate) use humanity_ship::ship_structure::{zone_body, zone_body_mut};
pub(crate) use humanity_ship::storey_edits::EditStorey;
pub(crate) use humanity_systems::crafting::CraftingSystem;
pub(crate) use humanity_systems::farming::FarmingSystem;
pub(crate) use humanity_systems::food::FoodSystem;
pub(crate) use humanity_systems::mining::DroneSystem;
pub(crate) use humanity_systems::skills::{PlayerSkills, SkillRegistry, SkillSystem, SkillXPEvent};
pub(crate) use humanity_systems::quests::{QuestRegistry, QuestSystem, QuestTracker};
pub(crate) use humanity_systems::interaction::InteractionSystem;
pub(crate) use humanity_systems::inventory::{Inventory, InventorySystem, ItemRegistry};
pub(crate) use humanity_systems::inventory::containers::ContainerCompatibilitySystem;
pub(crate) use humanity_systems::player::PlayerControllerSystem;
pub(crate) use humanity_systems::time::{GameTime, TimeSystem};
pub(crate) use humanity_systems::weather::{Weather, WeatherSystem};
pub(crate) use std::path::PathBuf;
pub(crate) use std::sync::Arc;
pub(crate) use std::time::Instant;
pub(crate) use winit::event::{DeviceEvent, DeviceId, WindowEvent};
pub(crate) use winit::event_loop::ActiveEventLoop;
pub(crate) use winit::keyboard::{KeyCode, PhysicalKey};
pub(crate) use winit::window::{Window, WindowId};
// The window helpers that stay beside startup in src/lib.rs.
pub(crate) use crate::native_app::{
    apply_opening_resize, apply_window_mode, find_data_dir, apply_window_mode_once, machine_has_port, open_showroom, port_drop_node_target,
    port_drop_target, reconcile_cursor, try_grab_opening, try_grab_opening_resize,
};
