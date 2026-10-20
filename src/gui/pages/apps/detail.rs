//! The Apps page's details, at the right of the parts while something is chosen, in the shelf's
//! details panel (widgets/shelf.rs, its contents clear of its scroll bar): a game's or a thing
//! added's (apps/game_detail.rs), a catalog entry's (apps/entry_detail.rs), or one of HumanityOS's
//! own tools' (here: its banner, its line, and Open).

use egui::{RichText, Sense, Vec2};

use super::{entry_detail, game_detail, own_tools, Act, Picked};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cover_tile, icons, shelf};
use crate::gui::GuiState;

/// The details' width when they open (the person can drag their edge).
pub(super) const DETAILS_WIDTH: f32 = 440.0;

/// The details, when something is chosen. Returns what was clicked.
pub(super) fn panel(ctx: &egui::Context, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let mut acts = Vec::new();
    let page = &state.apps;
    match &page.selected {
        Some(Picked::Game(..)) => {
            let Some(game) = page.selected_game() else { return acts };
            shelf::details_panel(ctx, theme, "apps_details", DETAILS_WIDTH, |ui| game_detail::body(ui, theme, page, game, &mut acts));
        }
        Some(Picked::Entry(i)) => {
            let Some(entry) = state.external_catalog.get(*i) else { return acts };
            shelf::details_panel(ctx, theme, "apps_details", DETAILS_WIDTH, |ui| entry_detail::body(ui, theme, &page.programs, *i, entry, &mut acts));
        }
        Some(Picked::Tool(i)) => {
            let Some(tool) = page.own_tools.get(*i) else { return acts };
            shelf::details_panel(ctx, theme, "apps_details", DETAILS_WIDTH, |ui| tool_body(ui, theme, tool, &mut acts));
        }
        None => {}
    }
    acts
}

/// One of HumanityOS's own tools: its banner with its page's mark, Close, its line, and Open.
fn tool_body(ui: &mut egui::Ui, theme: &Theme, tool: &own_tools::OwnTool, acts: &mut Vec<Act>) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, (width / 3.1).round().max(80.0)), Sense::hover());
    cover_tile::paint_cover(ui.painter(), theme, rect, None, None, &tool.name, &tool.page, theme.font_size_title, theme.border_radius_lg);
    if let Some(page) = tool.gui_page() {
        let side = (rect.height() * 0.4).round();
        let pad = theme.spacing_md;
        let at = egui::Rect::from_min_size(egui::pos2(rect.right() - pad - side, rect.top() + pad), Vec2::splat(side));
        icons::paint_nav_icon(ui.painter(), at, page, theme.text_strong().gamma_multiply(0.85));
    }
    ui.add_space(theme.spacing_sm);
    ui.horizontal(|ui| {
        ui.label(RichText::new(super::Section::Tools.words()).size(theme.font_size_small).color(theme.text_secondary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::Button::ghost(super::CLOSE).size(widgets::ButtonSize::Small).tooltip("Close its details.").show(ui, theme) {
                acts.push(Act::CloseDetails);
            }
        });
    });
    ui.add(egui::Label::new(RichText::new(&tool.line).size(theme.font_size_body).color(theme.text_primary())).wrap());
    ui.add_space(theme.spacing_sm);
    match tool.gui_page() {
        Some(page) => {
            if widgets::Button::primary(own_tools::OPEN).size(widgets::ButtonSize::Large).tooltip("Opens its page here.").show(ui, theme) {
                acts.push(Act::OpenPage(page));
            }
            ui.label(RichText::new("Part of HumanityOS: it opens here, and nothing is installed or started outside the app.").size(theme.font_size_small).color(theme.text_muted()));
        }
        None => {
            ui.label(RichText::new(format!("This version of HumanityOS has no page named \"{}\".", tool.page)).size(theme.font_size_small).color(theme.warning()));
        }
    }
}
