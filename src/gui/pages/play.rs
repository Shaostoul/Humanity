//! Play: THE GAME'S HUB (2026-11-11). The operator, that day: "Let's turn the play page into the
//! MCP/overview of all the dedicated game related pages. Then we can have the dedicated sub-pages
//! of the game. Let's remove clicking play immediately starting the game. We need to make more use
//! of the play."
//!
//! So Play in the top menu only ever opens this page (navigation rung 2's press and hold, which
//! went straight into the world, is gone: a press of any length opens the hub), and the page is a
//! master control panel for the game:
//!
//! - THE WORLD YOU WILL ENTER, in a tint section: who and where was played last, in words, and the
//!   Enter the world button (a click, or Enter while it has the keyboard). It is the one way in
//!   from here; the title screen's Enter World and the Characters picker's Enter bar are the
//!   others, each a button a person presses on purpose.
//! - A CARD FOR EACH OF THE GAME'S PAGES, each a tint section with a live summary of what is in it
//!   now and a button that opens it. The cards are not a list in this file: they are the Play
//!   row of data/gui/navigation.json (the same file the top menu draws), every item that opens
//!   something, in the file's order. A page added to Play's row there gets its card here the same
//!   day, its summary its own hint until its page writes a `hub_summary`.
//!
//! A card's page opens as a step FROM the hub (gui/history.rs `How::From`), so Escape on it comes
//! back here; Characters opens the character picker, as its row item does.
//!
//! COST: a handful of labels and buttons on a page frame the app draws anyway; the summaries read
//! what GuiState already holds (no file read, no disk scan: Characters counts the saves the
//! picker last scanned, and says only the last played until it has).
//!
//! The website: no game runs in a browser, so the website's Play opens /download
//! (data/gui/navigation.json says so) and has no hub to mirror; its Inventory and Crafting pages
//! stand on their own (docs/design/top-menu-and-game-pages.md, section 7).

use egui::RichText;
use humanity_core::i18n::{t, t_dyn, tf};

use crate::gui::navigation::{self, NavButton};
use crate::gui::theme::Theme;
use crate::gui::widgets::tint_section::{self, Tint};
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiPage, GuiState};

/// The world section's colour: the Play category's own is purple, so magenta stands for it.
pub const WORLD_TINT: Tint = Tint::Magenta;
/// The cards' colours, in order, round again after the last: every card in a colour of its own
/// beside its neighbours, and never the world section's.
pub const CARD_TINTS: [Tint; 5] = [Tint::Red, Tint::Green, Tint::Blue, Tint::Cyan, Tint::Yellow];
/// Cards across, at most: three reads at a glance on the operator's TV and on a laptop.
const MAX_COLUMNS: usize = 3;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default().frame(widgets::page_frame(theme)).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            widgets::section_header(ui, theme, t("nav.play"));
            ui.label(RichText::new(t("play.hub.lead")).size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(theme.spacing_sm);

            tint_section::show(ui, theme, WORLD_TINT, t("play.hub.world"), |ui| {
                ui.label(RichText::new(last_played(state)).size(theme.font_size_body).color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);
                if widgets::Button::primary(t("play.enter")).hint("play.enter_hint").show(ui, theme) {
                    enter_world(state, false);
                }
            });
            ui.add_space(theme.spacing_sm);

            let menu = navigation::menu();
            let cards = cards(&menu);
            let width = ui.available_width();
            let across = columns_for(width, theme);
            let mut open: Option<NavButton> = None;
            for (row, chunk) in cards.chunks(across).enumerate() {
                ui.columns(across, |cols| {
                    for (i, b) in chunk.iter().enumerate() {
                        let tint = CARD_TINTS[(row * across + i) % CARD_TINTS.len()];
                        if card(&mut cols[i], theme, state, tint, b) {
                            open = Some((*b).clone());
                        }
                    }
                });
                ui.add_space(theme.spacing_sm);
            }
            if let Some(b) = open {
                open_card(state, &b);
            }
        });
    });
}

/// The hub's cards: Play's row in the menu file, each item that opens something (a page, or the
/// character picker), in the file's order. Worlds and Servers, not built yet, open nothing and
/// have no card until they do.
pub fn cards(menu: &navigation::Menu) -> Vec<&NavButton> {
    menu.desktop
        .iter()
        .find(|c| c.button.page == GuiPage::Play)
        .map(|c| c.items.iter().filter(|b| b.page != GuiPage::None || b.picker()).collect())
        .unwrap_or_default()
}

/// How many cards across fit `width`: each needs room for its summary on a line or two.
fn columns_for(width: f32, theme: &Theme) -> usize {
    let min_card = theme.font_size_body * 18.0;
    ((width / min_card).floor() as usize).clamp(1, MAX_COLUMNS)
}

/// One card: the page's name as the section's title, its summary, and its Open button. True when
/// the button was pressed.
fn card(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, tint: Tint, b: &NavButton) -> bool {
    let name = t_dyn(&b.label);
    tint_section::show(ui, theme, tint, name, |ui| {
        for line in summary(state, b) {
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_primary()));
        }
        ui.add_space(theme.spacing_xs);
        ui.button(tf("play.hub.open", &[("page", &name)])).hint_dyn(&b.hint).clicked()
    })
}

/// What a card says: its page's own `hub_summary`, read from what GuiState holds now. A page with
/// none yet says its row item's hint, so a new card is never empty.
pub fn summary(state: &GuiState, b: &NavButton) -> Vec<String> {
    if b.picker() {
        return characters_summary(state);
    }
    match b.page {
        GuiPage::Quests => crate::gui::pages::quests::hub_summary(state),
        GuiPage::Inventory => crate::gui::pages::inventory::hub_summary(state),
        GuiPage::Crafting => crate::gui::pages::crafting::hub_summary(state),
        _ => vec![t_dyn(&b.hint).to_string()],
    }
}

/// The Characters card: who was played last, and how many characters are saved on this computer
/// once the picker has counted them (it scans the saves each time it opens; the hub never does).
fn characters_summary(state: &GuiState) -> Vec<String> {
    let mut lines = Vec::new();
    let who = state.launcher_last_character.as_str();
    lines.push(if who.is_empty() { t("play.hub.characters_none").to_string() } else { tf("play.hub.characters_last", &[("character", &who)]) });
    if state.launcher_homes_loaded {
        let n = state.launcher_homes.len().to_string();
        lines.push(tf("play.hub.characters_saved", &[("count", &n)]));
    }
    lines.push(t("play.hub.characters_open").to_string());
    lines
}

/// Open a card's page, a step from the hub so Escape on it comes back here; Characters opens the
/// character picker (the world's showroom), as its row item in the top menu does.
pub fn open_card(state: &mut GuiState, b: &NavButton) {
    if b.page == GuiPage::None {
        enter_world(state, b.picker());
        return;
    }
    state.go(b.page, crate::gui::How::From);
}

/// Who and where was played last, in words: what Enter the world goes into.
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
/// when none is recorded or `picker` asks for it (Characters). One function for every way in from
/// the pages: the hub's Enter the world, its Characters card, and the Characters item of the Play
/// row. Play itself never calls it (2026-11-11: a press on Play only opens the hub).
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
