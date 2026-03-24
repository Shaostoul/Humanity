//! Civilization dashboard — community stats (Real) or colony stats (Sim).

use egui::{RichText, Vec2};
use crate::gui::{GuiPage, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let is_real = state.context_real;
    let title = if is_real { "Community Dashboard" } else { "Colony Dashboard" };

    egui::Window::new("Civilization")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(560.0, 480.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_label = if is_real { "Real" } else { "Sim" };
                    ui.label(
                        RichText::new(mode_label)
                            .size(theme.font_size_small)
                            .color(theme.accent()),
                    );
                });
            });

            ui.add_space(theme.spacing_md);

            // ── Stats grid (2 columns) ──
            let stats = if is_real {
                vec![
                    ("Population", state.civ_population.to_string()),
                    ("Buildings", state.civ_buildings.to_string()),
                    ("Resources", state.civ_resources.to_string()),
                    ("Tech Level", format!("Level {}", state.civ_tech_level)),
                ]
            } else {
                vec![
                    ("Colonists", state.civ_population.to_string()),
                    ("Structures", state.civ_buildings.to_string()),
                    ("Stockpile", state.civ_resources.to_string()),
                    ("Research", format!("Tier {}", state.civ_tech_level)),
                ]
            };

            egui::Grid::new("civ_stats_grid")
                .num_columns(2)
                .spacing(Vec2::new(theme.spacing_md, theme.spacing_sm))
                .show(ui, |ui| {
                    for (i, (label, value)) in stats.iter().enumerate() {
                        widgets::card(ui, theme, |ui| {
                            ui.set_min_width(220.0);
                            ui.label(
                                RichText::new(*label)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            ui.label(
                                RichText::new(value)
                                    .size(theme.font_size_title)
                                    .color(theme.accent()),
                            );
                        });
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });

            ui.add_space(theme.spacing_md);

            // ── Progress bars for key metrics ──
            let metrics = if is_real {
                vec![
                    ("Food Supply", state.civ_food),
                    ("Energy", state.civ_energy),
                    ("Water", state.civ_water),
                    ("Happiness", state.civ_happiness),
                ]
            } else {
                vec![
                    ("Food Reserves", state.civ_food),
                    ("Power Grid", state.civ_energy),
                    ("Water Recycling", state.civ_water),
                    ("Morale", state.civ_happiness),
                ]
            };

            widgets::card(ui, theme, |ui| {
                ui.label(
                    RichText::new("Key Metrics")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                ui.add_space(theme.spacing_xs);
                for (label, value) in &metrics {
                    ui.horizontal(|ui| {
                        ui.set_min_width(100.0);
                        ui.label(
                            RichText::new(*label)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    });
                    widgets::progress_bar(ui, theme, *value, Some(&format!("{:.0}%", value * 100.0)));
                    ui.add_space(theme.spacing_xs);
                }
            });

            ui.add_space(theme.spacing_md);

            // ── Recent events log ──
            widgets::card(ui, theme, |ui| {
                ui.label(
                    RichText::new("Recent Events")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                ui.add_space(theme.spacing_xs);

                if state.civ_events.is_empty() {
                    ui.label(
                        RichText::new("No recent events")
                            .color(theme.text_muted()),
                    );
                } else {
                    egui::ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
                        for event in state.civ_events.iter().rev() {
                            ui.label(
                                RichText::new(event)
                                    .size(theme.font_size_small)
                                    .color(theme.text_secondary()),
                            );
                        }
                    });
                }
            });

            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Close") {
                state.active_page = GuiPage::EscapeMenu;
            }
        });
}
