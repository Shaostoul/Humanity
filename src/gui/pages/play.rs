//! Play's own page (navigation rung 2, 2026-11-03; docs/design/navigation.md, section 16.1,
//! question 2). The operator, that day: "Maybe click and hold after booting the app. Then just
//! tapping Play brings up the subpages that are relevant to the game. Like characters, homes,
//! servers, etc.?"
//!
//! So a TAP on Play in the top menu opens this page, with the Play row under the menu (Characters,
//! Quests, Inventory, Crafting; Worlds and Servers join it when they are built, rung 7), and a
//! PRESS AND HOLD on Play goes straight into the world (pages/escape_menu.rs, the hold's length is
//! data/gui/navigation.json's `timings.play_hold_ms`). Holding is hard for some hands, so this
//! page has its own Enter the world button: a click on it, or Enter while it has the keyboard,
//! goes in at once.
//!
//! The smallest true page, and on purpose: who and where you played last, and that button. The
//! design's Play overview (the worlds, the servers, your homes) is rung 7's.

use egui::RichText;
use humanity_core::i18n::{t, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default().frame(widgets::page_frame(theme)).show(ctx, |ui| {
        widgets::section_header(ui, theme, t("nav.play"));
        ui.label(RichText::new(last_played(state)).size(theme.font_size_body).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        if widgets::Button::primary(t("play.enter")).hint("play.enter_hint").show(ui, theme) {
            enter_world(state, false);
        }
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new(t("play.hold_tip")).size(theme.font_size_small).color(theme.text_muted()));
    });
}

/// Who and where was played last, in words: what Enter the world (and Play's hold) goes into.
pub fn last_played(state: &GuiState) -> String {
    let last = state.launcher_last_world.as_str();
    let world = if let Some(home) = last.strip_prefix("home:") {
        // "home:" with no name is the fresh homestead, a real pairing (GuiState::record_pairing).
        if home.is_empty() { t("play.world_home").to_string() } else { home.to_string() }
    } else if let Some(id) = last.strip_prefix("server:").filter(|id| !id.is_empty()) {
        if id == crate::gui::pages::showroom::CONNECTED_SERVER_ID { t("play.world_server").to_string() } else { id.to_string() }
    } else {
        return t("play.none").to_string();
    };
    let who = state.launcher_last_character.as_str();
    if who.is_empty() {
        tf("play.last_no_character", &[("world", &world)])
    } else {
        tf("play.last", &[("character", &who), ("world", &world)])
    }
}

/// Go into the world: with the character and world played last, or through the character picker
/// when none is recorded or `picker` asks for it (Characters). One function for every way in:
/// Play's press and hold, this page's button, and the Characters item of the Play row.
///
/// Entering the world is not a step of the history (docs/design/navigation.md section 11): the
/// history stays on the page left, so Escape in the world returns to it and Back after that goes
/// to the page before it (gui/history.rs `enter_world`).
pub fn enter_world(state: &mut GuiState, picker: bool) {
    // apply_last_pairing restores the selection, asks for the character load, and sets solo or
    // shared from the world kind; with nothing recorded the picker opens instead (the showroom is
    // opened by lib.rs's per-frame handler when launcher_open_select is set).
    if picker || !state.apply_last_pairing() {
        state.launcher_open_select = true;
        state.launcher_homes_loaded = false; // rescan the saves
    }
    state.enter_world();
}
