//! GUI pages composing widgets into full screens.

pub mod main_menu;
pub mod escape_menu;
pub mod settings;
pub mod inventory;
pub mod chat;
pub mod hud;
pub mod vendor;
pub mod showroom;
pub mod construction;
pub mod keymap;
pub mod diagnostics;
pub mod placeholder;
pub mod tasks;
pub mod profile;
pub mod real;
// v0.415.0: play module removed (the dead v0.360 Crafting/Studio fold —
// nothing navigated to GuiPage::Play; the nav's Play button is GuiPage::None).
pub mod platform;
pub mod humanity;
pub mod library;
pub mod quests;
pub mod homes;
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
// v0.415.0: resources module removed (retired into the Library, v0.374-375).
pub mod donate;
pub mod tools;
pub mod studio;
// passphrase_modal moved to widgets/ in v0.115.0 (it's a modal, not a page).
// Re-export from its new location to keep callers compiling.
pub use crate::gui::widgets::passphrase_modal;
pub mod onboarding;
pub mod server_settings;
pub mod game_admin;
pub mod identity;
pub mod governance;
pub mod laws;
pub mod recovery;
pub mod cosmos;
// v0.197.0: agents and ai_usage page modules removed. The multi-AI
// orchestration infrastructure (data/coordination/* + relay
// agent_sessions table) is unaffected — only the UI surface is gone.
pub mod testing;
pub mod browser;
pub mod dev;
// v0.699.0: category_overview + settings_pages modules removed with the dead
// two-tier-nav category-browse subsystem (5 Overview* + 12 Settings* pages).
