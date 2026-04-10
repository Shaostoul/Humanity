//! HumanityOS Game Systems
//!
//! All gameplay logic — farming, construction, combat, quests, economy, etc.
//! Every system is data-driven: configuration loaded from CSV, RON, or TOML files.

pub mod time;
pub mod farming;
pub mod construction;
pub mod inventory;
pub mod combat;
pub mod quests;
pub mod crafting;
pub mod logistics;
pub mod vehicles;
pub mod navigation;
pub mod ai;
pub mod skills;
pub mod ecology;
pub mod economy;
pub mod player;
pub mod interaction;
pub mod weather;
pub mod hydrology;
pub mod atmosphere;
pub mod disasters;
pub mod electrical;
pub mod plumbing;
pub mod hvac;
pub mod fire;
pub mod medical;
pub mod food;
pub mod governance;
pub mod psychology;
