//! Quests: the SIM quest surface, gameplay only. Live quests from the in-game
//! QuestSystem (auto-tracked, XP rewards): active with step progress, available
//! to accept, then completed.
//!
//! The learn-by-doing chains (`data/onboarding/quests.json`) used to share this
//! page as a second column; they moved to the Tasks page's guide panel
//! (operator 2026-08-16: "Quests is for gameplay stuff and Tasks is for
//! not-game stuff"). The Profile page's game-quests section was folded in here
//! in v0.415.0.

use egui::{Frame, RichText, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Widest the content column gets. With the second column gone, an unclamped
/// card would stretch the full width of an ultrawide window and read as a
/// single line of text floating in space.
const CONTENT_MAX_WIDTH: f32 = 820.0;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // Single column, clamped: title, one-line subtitle, then the
                // live sim quests.
                ui.vertical(|ui| {
                    ui.set_max_width(CONTENT_MAX_WIDTH);

                    // Page title matches the nav button; before v0.1144 the
                    // page never stated its own name (only section eyebrows).
                    ui.label(
                        RichText::new("Quests")
                            .size(theme.font_size_title)
                            .color(theme.text_primary()),
                    );
                    // Subtitle replaces the old SIM QUESTS eyebrow (the whole
                    // page is sim now) and points at where the real-life
                    // tutorials went.
                    ui.label(
                        RichText::new("Auto-tracked from the game. Real-life guides live in Tasks.")
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_md);

                    draw_game_quests(ui, theme, state);
                });
            });
        });
}

/// Live sim quests from the in-game QuestSystem (auto-tracked, XP rewards):
/// active quests with step progress, available quests to accept, then the
/// completed list. Moved from the Profile page's retired Quests section
/// (v0.415.0). The SIM QUESTS eyebrow and its "Tracked automatically in-game"
/// heading came off when the page went sim-only: the page title plus its
/// subtitle now say the same thing once instead of twice.
fn draw_game_quests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let has_active = state.quests.iter().any(|q| !q.completed);
    let has_completed = state.quests.iter().any(|q| q.completed);

    // Empty only when there is nothing at all to show. The available list has
    // to count here too, otherwise a fresh session with acceptable quests but
    // no accepted ones bails out above the Accept buttons.
    if !has_active && !has_completed && state.quests_available.is_empty() {
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

    // Available quests (v0.748, ladder rung 4): accept-able content the
    // prerequisite chaining alone never surfaced. One card per quest with an
    // Accept button; the frame bridge applies it to the live tracker.
    if !state.quests_available.is_empty() {
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new("Available")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_xs);
        let available = state.quests_available.clone();
        for q in &available {
            widgets::card(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&q.name)
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::compact_button(ui, theme, "Accept", widgets::ButtonVariant::Primary) {
                            state.pending_accept_quest = Some(q.id.clone());
                        }
                    });
                });
                if !q.description.is_empty() {
                    ui.label(
                        RichText::new(&q.description)
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
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
