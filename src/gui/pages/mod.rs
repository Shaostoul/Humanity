//! GUI pages composing widgets into full screens.

pub mod main_menu;
pub mod escape_menu;
pub mod settings;
pub mod settings_time;
pub mod settings_garden;
pub mod inventory;
pub mod chat;
pub mod chat_text;
pub mod hud;
pub mod vendor;
pub mod showroom;
pub mod construction;
pub mod keymap;
pub mod diagnostics;
pub mod weather_panel;
pub mod cloud_dev;
pub mod placeholder;
pub mod tasks;
pub mod privacy;
/// This person's own profile on their server: which copy is theirs, and the whole profile every
/// save sends (2026-10-11, BUG-193 and BUG-194).
pub mod own_profile;
/// Settings > Safety: who can reach me, People I choose, contact requests (step B, 2026-10-09).
pub mod safety;
/// Settings > Safety > Protected setup, and the PIN prompt its locked actions open (step G,
/// 2026-10-10).
pub mod safety_protected;
/// Settings > Safety > Reports about your groups, and the count on a group in the chat (10j,
/// 2026-10-10).
pub mod safety_group_reports;
pub mod profile;
pub mod real;
// v0.415.0: play module removed (the dead v0.360 Crafting/Studio fold —
// nothing navigated to GuiPage::Play; the nav's Play button is GuiPage::None).
// The Platform tab (v0.360) was retired on 2026-10-20: its developer tools are
// the Dev tab (dev_tab.rs), its Calculator, Notes, Calendar and Browser are on
// Apps, its Recovery is in Settings > Account.
pub mod humanity;
pub mod library;
pub mod quests;
pub mod homes;
// The Home page's loops as cards (2026-10-30): the home's own, the ideal outline's, and what one
// home cannot close.
pub mod home_loops;
/// Home > Household: giving, renewing and taking back household permits (ship homes 5b, 2026-10-12).
pub mod household;
/// Choose where you live: the start and household of the live save, in Settings > Gameplay, on
/// the Home page and in Characters (ship homes increment 6, 2026-10-12).
pub mod where_you_live;
/// Home > Animals: each farm animal, how it is and what it needs (2026-10-12, first-hour audit S6).
pub mod herd;
pub mod market;
pub mod market_directory;
pub mod market_publish;
pub mod calculator;
pub mod calendar;
pub mod notes;
pub mod civilization;
pub mod wallet;
pub mod crafting;
pub mod guilds;
pub mod trade;
pub mod bugs;
// v0.415.0: resources module removed (retired into the Library, v0.374-375).
pub mod donate;
pub mod studio;
pub mod watch;
/// The Files page (docs/design/files-showcase.md, increment 2, 2026-10-11; the Media page until
/// 2026-10-19, when the developer's Files page joined it).
pub mod files;
/// Apps (2026-10-19): the Games library and the Tools page as one page, in six coloured parts.
pub mod apps;
/// The server strip under the top menu on Media, Watch, Chat and Server Settings, with Switch.
pub mod server_strip;
// passphrase_modal moved to widgets/ in v0.115.0 (it's a modal, not a page).
// Re-export from its new location to keep callers compiling.
pub use crate::gui::widgets::passphrase_modal;
pub mod server_settings;
pub mod relay_control;
// "Host a node": start/stop a relay on THIS machine from inside the app
// (rendered as a section of relay_control). GUI-first rule, CLAUDE.md.
pub mod host_node;
pub mod game_admin;
// Server Settings > ADMIN > Shared world clock (2026-10-04), drawn by game_admin.
pub mod world_clock_admin;
// The fleet ledger (2026-10-04): Inventory > The fleet, and Server Settings > ADMIN > Fleet supply.
pub mod fleet_ledger;
pub mod identity;
pub mod governance;
pub mod laws;
/// How-to (docs/design/how-to.md, increment 1, 2026-10-23): step-by-step procedures, with runs.
pub mod how_to;
// Recovery through trusted people moved into Settings > Account with the
// Platform tab's retirement (2026-10-20): pages/settings/recovery.rs.
// The Maps page (was cosmos.rs until Maps increment 0, 2026-10-20): a folder,
// one file per view. `crate::cosmos` is the Sol model, not this page.
pub mod maps;
// v0.197.0: agents and ai_usage page modules removed. The multi-AI
// orchestration infrastructure (data/coordination/* + relay
// agent_sessions table) is unaffected — only the UI surface is gone.
pub mod testing;
pub mod browser;
pub mod dev;
pub mod planet_tuner;
// Resource budgets increment 1: the live GPU/CPU/VRAM/RAM pies, shown as a
// section of the Dev tab (the page-sized sibling of the F2 overlay).
pub mod performance;
// Dev, the developer tools tab (2026-10-20, replaced the Platform tab): Bugs,
// Testing, Performance, Dev, Planet Tuner and Claude in one side list.
pub mod dev_tab;
// Dev > Claude (2026-10-22): what the agents are doing, read-only, from this
// computer's files (worktrees, journal, TIER 0, delivery, the guards, the boot lock).
pub mod dev_agents;
// Dev > Journal: the play journal in the app (2026-10-24, docs/design/play-journal.md rung 2).
pub mod dev_journal;
// v0.699.0: category_overview + settings_pages modules removed with the dead
// two-tier-nav category-browse subsystem (5 Overview* + 12 Settings* pages).
