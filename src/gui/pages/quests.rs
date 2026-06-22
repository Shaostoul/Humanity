//! Quests: the single quest surface — one page, two kinds (operator 2026-06-06:
//! gameplay quests [auto-track + XP] AND learn-by-doing chains). Live sim quests
//! from the in-game QuestSystem render first; the self-sufficiency chains
//! (`data/onboarding/quests.json`, via the shared `onboarding::draw_quests`)
//! follow, with First Steps (the onboarding chain) at the top. The Profile
//! page's game-quests section was folded in here in v0.415.0.

use egui::{Frame, RichText, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use super::onboarding;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // Responsive two-column: the auto-tracked sim quests on the left,
                // the learn-by-doing chains on the right when wide; stacked narrow.
                if ui.available_width() >= 900.0 {
                    ui.columns(2, |cols| {
                        draw_game_quests(&mut cols[0], theme, state);
                        onboarding::draw_quests(&mut cols[1], theme, state);
                    });
                } else {
                    draw_game_quests(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                    onboarding::draw_quests(ui, theme, state);
                }
            });
        });
}

/// Live sim quests from the in-game QuestSystem (auto-tracked, XP rewards):
/// active quests with step progress, then the completed list. Moved from the
/// Profile page's retired Quests section (v0.415.0).
fn draw_game_quests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(
        RichText::new("SIM QUESTS")
            .size(theme.font_size_small)
            .color(theme.accent())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new("Tracked automatically in-game")
            .size(theme.font_size_heading)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.spacing_md);

    let has_active = state.quests.iter().any(|q| !q.completed);
    let has_completed = state.quests.iter().any(|q| q.completed);

    if !has_active && !has_completed {
        widgets::card(ui, theme, |ui| {
            ui.label(
                RichText::new("No quests yet, start a game session to receive your first quest.")
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
        });
        return;
    }

    // Active quests: current step + a step-progress bar.
    if has_active {
        ui.label(RichText::new("Active").size(theme.font_size_body).color(theme.text_secondary()));
        ui.add_space(theme.spacing_xs);
        for q in state.quests.iter().filter(|q| !q.completed) {
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new(&q.name).size(theme.font_size_body).color(theme.text_primary()));
                if q.step_total > 0 {
                    ui.label(
                        RichText::new(format!(
                            "Step {} of {}: {}",
                            q.step_index + 1,
                            q.step_total,
                            q.step_desc
                        ))
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                    );
                    let frac = (q.step_index as f32 / q.step_total as f32).clamp(0.0, 1.0);
                    widgets::progress_bar(ui, theme, frac, None);
                }
            });
            ui.add_space(theme.spacing_xs);
        }
    }

    // Completed quests.
    if has_completed {
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new("Completed").size(theme.font_size_body).color(theme.text_secondary()));
        ui.add_space(theme.spacing_xs);
        widgets::card(ui, theme, |ui| {
            for q in state.quests.iter().filter(|q| q.completed) {
                ui.label(
                    RichText::new(format!("\u{2713} {}", q.name))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
        });
    }
}
