//! Quests: the single learn-by-doing quest list, promoted to its own top-level
//! tab (operator 2026-06-06: "add a top level quests page for now"). Renders the
//! self-sufficiency chains (`data/onboarding/quests.json`) via the shared
//! `onboarding::draw_quests`; First Steps (the onboarding chain) is first, so it
//! sits at the top. Was a section under Real (v0.373); moved up here.

use egui::{Frame, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use super::onboarding;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                onboarding::draw_quests(ui, theme, state);
            });
        });
}
