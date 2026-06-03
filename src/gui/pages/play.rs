//! Play — the simulation tab (page carve, v0.360).
//!
//! Folds Crafting + Studio into one tab via a `section_nav` sidebar; the content
//! area delegates to each existing page's `draw` (same delegate pattern as the
//! Real tab — no page rewrite). The sim launcher / boot-into-character flow grows
//! here later; for now the fold collapses two nav buttons into one "Play" tab.

use egui::{Frame, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::{crafting, studio};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::SidePanel::left("play_section_nav")
        .default_width(190.0)
        .min_width(150.0)
        .max_width(260.0)
        .frame(
            Frame::none()
                .fill(theme.bg_sidebar())
                .inner_margin(egui::Margin::symmetric(8, 12))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            let c = theme.nav_sim();
            let items = [
                SectionNavItem::new("crafting", "Crafting", c),
                SectionNavItem::new("studio", "Studio", c),
            ];
            if let Some(clicked) =
                widgets::section_nav(ui, theme, Some("Play"), &items, &state.active_play_section)
            {
                state.active_play_section = clicked;
            }
        });

    let section = state.active_play_section.clone();
    match section.as_str() {
        "studio" => studio::draw(ctx, theme, state),
        _ => crafting::draw(ctx, theme, state),
    }
}
