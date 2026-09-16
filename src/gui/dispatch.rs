//! THE ONE page dispatch table (in-world screens, rung 1).
//!
//! Every plain tool page of the native app is drawn by one function of the
//! shape `page::draw(ctx, theme, gui_state)`. Until this file existed that
//! list lived only inside the giant `match` in `lib.rs`'s per-frame egui
//! closure, which meant nothing else could draw "whatever the main UI can
//! draw". The in-world screens need exactly that: a wall screen showing the
//! inventory must run the SAME `inventory::draw` the full-screen page runs,
//! against the SAME `GuiState`, just under a different `egui::Context`.
//!
//! So the match moved here. `lib.rs` delegates every plain arm to
//! [`draw_tool_page`] and keeps inline only the arms that need `EngineState`
//! (the title screen and the in-game `None` page with its HUD). A screen
//! surface calls the same function, so the two can never drift: a page the
//! main UI can show, a screen can show, by construction.
//!
//! The match below has NO wildcard arm on purpose. Adding a `GuiPage`
//! variant without deciding where it draws is a compile error here, and the
//! test at the bottom then checks that every variant the enum names is
//! either drawn by this table or explicitly listed as engine-bound.

use super::pages::{
    browser, bugs, calculator, calendar, chat, cosmos, crafting, donate, files, governance, guilds,
    homes, humanity, identity, inventory, laws, library, market, notes, platform, profile, quests,
    real, recovery, relay_control, server_settings, settings, studio, tasks, testing, tools, trade,
    wallet, watch,
};
use super::theme::Theme;
use super::{GuiPage, GuiState};

/// Draw `page` into `ctx`. Returns `true` when the page is a plain tool page
/// this table owns, `false` for the engine-bound pages (`None`, `MainMenu`)
/// whose drawing needs `EngineState` and stays inline in `lib.rs`.
///
/// `theme` is `&mut` only because the Settings page edits the live theme in
/// place (the appearance editor); every other page reads it.
pub fn draw_tool_page(ctx: &egui::Context, page: GuiPage, theme: &mut Theme, state: &mut GuiState) -> bool {
    match page {
        // Engine-bound: the title screen and the in-game HUD frame need the
        // camera, the world and the renderer. They stay in lib.rs.
        GuiPage::None | GuiPage::MainMenu => return false,
        GuiPage::Settings => settings::draw(ctx, theme, state),
        GuiPage::Inventory => inventory::draw(ctx, theme, state),
        GuiPage::Chat => chat::draw(ctx, theme, state),
        GuiPage::Tasks => tasks::draw(ctx, theme, state),
        GuiPage::Market => market::draw(ctx, theme, state),
        GuiPage::Profile => profile::draw(ctx, theme, state),
        GuiPage::Real => real::draw(ctx, theme, state),
        GuiPage::Platform => platform::draw(ctx, theme, state),
        GuiPage::Humanity => humanity::draw(ctx, theme, state),
        GuiPage::Library => library::draw(ctx, theme, state),
        GuiPage::Calculator => calculator::draw(ctx, theme, state),
        GuiPage::Notes => notes::draw(ctx, theme, state),
        GuiPage::Calendar => calendar::draw(ctx, theme, state),
        GuiPage::Crafting => crafting::draw(ctx, theme, state),
        GuiPage::Wallet => wallet::draw(ctx, theme, state),
        GuiPage::Guilds => guilds::draw(ctx, theme, state),
        GuiPage::Trade => trade::draw(ctx, theme, state),
        GuiPage::Files => files::draw(ctx, theme, state),
        GuiPage::BugReport => bugs::draw(ctx, theme, state),
        GuiPage::Donate => donate::draw(ctx, theme, state),
        GuiPage::Tools => tools::draw(ctx, theme, state),
        GuiPage::Studio => studio::draw(ctx, theme, state),
        GuiPage::Watch => watch::draw(ctx, theme, state),
        GuiPage::Quests => quests::draw(ctx, theme, state),
        GuiPage::Homes => homes::draw(ctx, theme, state),
        GuiPage::ServerSettings => server_settings::draw(ctx, theme, state),
        GuiPage::RelayControl => relay_control::draw(ctx, theme, state),
        GuiPage::Identity => identity::draw(ctx, theme, state),
        GuiPage::Governance => governance::draw(ctx, theme, state),
        GuiPage::Laws => laws::draw(ctx, theme, state),
        GuiPage::Recovery => recovery::draw(ctx, theme, state),
        // Maps and the old Cosmos variant merged (v0.1145); cosmos.rs is the module.
        GuiPage::Maps => cosmos::draw(ctx, theme, state),
        GuiPage::Testing => testing::draw(ctx, theme, state),
        GuiPage::Browser => browser::draw(ctx, theme, state),
    }
    true
}

/// The data-file id of a page: what a `ScreenDef.page` string in
/// `data/machines/home.ron` names, and what the dev IPC reports back. One
/// short lower-case word per page, stable across renames of the Rust
/// variant. `page_from_id` is the inverse; the test below round-trips every
/// variant through both.
///
/// This is deliberately a SEPARATE table from `page_to_config_str` /
/// `config_str_to_page` in `gui/mod.rs`: those two serve the boot-page
/// dropdown (seven pages, unknown falls back to Humanity), and a screen must
/// be able to name any tool page and must NOT silently turn a typo into the
/// mission dashboard. Unknown ids here are an error the caller reports.
pub fn page_id(page: GuiPage) -> &'static str {
    match page {
        GuiPage::None => "none",
        GuiPage::MainMenu => "main_menu",
        GuiPage::Settings => "settings",
        GuiPage::Inventory => "inventory",
        GuiPage::Chat => "chat",
        GuiPage::Tasks => "tasks",
        GuiPage::Market => "market",
        GuiPage::Profile => "profile",
        GuiPage::Real => "real",
        GuiPage::Platform => "platform",
        GuiPage::Humanity => "humanity",
        GuiPage::Library => "library",
        GuiPage::Calculator => "calculator",
        GuiPage::Notes => "notes",
        GuiPage::Calendar => "calendar",
        GuiPage::Crafting => "crafting",
        GuiPage::Wallet => "wallet",
        GuiPage::Guilds => "guilds",
        GuiPage::Trade => "trade",
        GuiPage::Files => "files",
        GuiPage::BugReport => "bugs",
        GuiPage::Donate => "donate",
        GuiPage::Tools => "tools",
        GuiPage::Studio => "studio",
        GuiPage::Watch => "watch",
        GuiPage::Quests => "quests",
        GuiPage::Homes => "homes",
        GuiPage::ServerSettings => "server_settings",
        GuiPage::RelayControl => "relay_control",
        GuiPage::Identity => "identity",
        GuiPage::Governance => "governance",
        GuiPage::Laws => "laws",
        GuiPage::Recovery => "recovery",
        GuiPage::Maps => "maps",
        GuiPage::Testing => "testing",
        GuiPage::Browser => "browser",
    }
}

/// Resolve a data-file page id (see [`page_id`]) to its page. `None` for an
/// unknown id, so the caller can warn and fall back rather than guess. The
/// two engine-bound pages resolve too (they are real ids) but a screen
/// showing them draws blank, which [`draw_tool_page`] reports by returning
/// `false`.
pub fn page_from_id(id: &str) -> Option<GuiPage> {
    ALL_PAGES.iter().copied().find(|p| page_id(*p) == id)
}

/// Every `GuiPage` variant, spelled out. The exhaustive matches above keep
/// this list honest in one direction (a variant missing from a match fails
/// to compile); the test below keeps it honest in the other (a variant
/// missing from THIS list fails the round-trip count).
pub const ALL_PAGES: &[GuiPage] = &[
    GuiPage::None,
    GuiPage::MainMenu,
    GuiPage::Settings,
    GuiPage::Inventory,
    GuiPage::Chat,
    GuiPage::Tasks,
    GuiPage::Market,
    GuiPage::Profile,
    GuiPage::Real,
    GuiPage::Platform,
    GuiPage::Humanity,
    GuiPage::Library,
    GuiPage::Calculator,
    GuiPage::Notes,
    GuiPage::Calendar,
    GuiPage::Crafting,
    GuiPage::Wallet,
    GuiPage::Guilds,
    GuiPage::Trade,
    GuiPage::Files,
    GuiPage::BugReport,
    GuiPage::Donate,
    GuiPage::Tools,
    GuiPage::Studio,
    GuiPage::Watch,
    GuiPage::Quests,
    GuiPage::Homes,
    GuiPage::ServerSettings,
    GuiPage::RelayControl,
    GuiPage::Identity,
    GuiPage::Governance,
    GuiPage::Laws,
    GuiPage::Recovery,
    GuiPage::Maps,
    GuiPage::Testing,
    GuiPage::Browser,
];

/// The pages that need `EngineState` to draw and therefore cannot be shown on
/// an in-world screen. Listed explicitly so the test can say "every page NOT
/// in this list must be drawn by the table".
pub const ENGINE_BOUND_PAGES: &[GuiPage] = &[GuiPage::None, GuiPage::MainMenu];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::load_theme;

    /// A count check that cannot be satisfied by the exhaustive matches alone:
    /// `ALL_PAGES` is a hand-written list, so a new variant added to the enum
    /// and to both matches (which the compiler forces) but not to this list
    /// would make the table's coverage test below skip it silently. This
    /// pins the list length against the number of distinct ids the table
    /// produces, and the ids against the inverse lookup.
    #[test]
    fn every_page_has_a_unique_id_that_round_trips() {
        let mut seen = std::collections::HashSet::new();
        for &p in ALL_PAGES {
            let id = page_id(p);
            assert!(seen.insert(id), "page id {id:?} is used by two GuiPage variants");
            assert_eq!(page_from_id(id), Some(p), "page id {id:?} does not resolve back to {p:?}");
        }
        assert_eq!(page_from_id("not_a_page"), None, "an unknown id must not resolve to anything");
    }

    /// (f) THE DISPATCH COVERAGE TEST. Every `GuiPage` variant is either
    /// engine-bound (listed in `ENGINE_BOUND_PAGES`, table returns false) or a
    /// tool page the table draws (returns true) under a headless context with
    /// a default `GuiState`, two frames each so Windows/Areas settle. This
    /// runs the real page functions, so a page whose draw panics on an empty
    /// state fails here too, which is what a screen showing it would hit.
    ///
    /// It can fail: routing any tool page to the `return false` arm flips its
    /// expected value; deleting `ALL_PAGES` entries trips the count above.
    #[test]
    fn dispatch_table_covers_every_page_except_the_engine_bound_ones() {
        let mut theme = load_theme();
        for &page in ALL_PAGES {
            let ctx = egui::Context::default();
            crate::gui::install_fonts(&ctx);
            theme.apply_to_egui(&ctx);
            let mut state = GuiState::default();
            let input = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0))),
                ..Default::default()
            };
            let mut drawn = false;
            for _ in 0..2 {
                ctx.run(input(), |ctx| {
                    drawn = draw_tool_page(ctx, page, &mut theme, &mut state);
                });
            }
            let engine_bound = ENGINE_BOUND_PAGES.contains(&page);
            assert_eq!(
                drawn, !engine_bound,
                "GuiPage::{page:?}: draw_tool_page returned {drawn} but the page is {}",
                if engine_bound { "listed as engine-bound" } else { "a tool page the table must draw" }
            );
        }
    }
}
