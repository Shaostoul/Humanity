//! GUI pages composing widgets into full screens.

pub mod main_menu;
pub mod escape_menu;
pub mod settings;
pub mod inventory;
pub mod chat;
pub mod hud;
pub mod placeholder;
pub mod tasks;
pub mod profile;
pub mod maps;
pub mod market;
pub mod calculator;
pub mod calendar;
pub mod notes;
pub mod civilization;
pub mod wallet;
pub mod crafting;
pub mod guilds;
pub mod trade;
pub mod files;
pub mod bugs;
pub mod resources;
pub mod donate;
pub mod tools;
pub mod studio;
// passphrase_modal moved to widgets/ in v0.115.0 (it's a modal, not a page).
// Re-export from its new location to keep callers compiling.
pub use crate::gui::widgets::passphrase_modal;
pub mod onboarding;
pub mod server_settings;
pub mod identity;
pub mod governance;
pub mod recovery;
