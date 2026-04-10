//! Civilization dashboard — community stats (Real) or colony stats (Sim).
//! 3-column stats grid, trend arrows, progress bars, events timeline, charts placeholder.

use egui::{Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let is_real = state.context_real;
    let title = if is_real { "Community Dashboard" } else { "Colony Dashboard" };

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_label = if is_real { "Real" } else { "Sim" };
                    widgets::badge(ui, theme, mode_label, theme.accent());
                });
            });

            ui.add_space(theme.spacing_md);

            ScrollArea::vertical().show(ui, |ui| {
                // Stats definitions with trend arrows
                let stats: Vec<(&str, String, &str, f32)> = if is_real {
                    vec![
                        ("Population", state.civ_population.to_string(), "+12", 0.0),
                        ("Buildings Built", state.civ_buildings.to_string(), "+3", 0.0),
                        ("Resources Gathered", state.civ_resources.to_string(), "+45", 0.0),
                        ("Technology Level", format!("Level {}", state.civ_tech_level), "", civ_tech_progress(state.civ_tech_level)),
                        ("Food Supply", format!("{:.0}%", state.civ_food * 100.0), "", state.civ_food),
                        ("Energy Production", format!("{:.0}%", state.civ_energy * 100.0), "", state.civ_energy),
                    ]
                } else {
                    vec![
                        ("Colonists", state.civ_population.to_string(), "+5", 0.0),
                        ("Structures", state.civ_buildings.to_string(), "+2", 0.0),
                        ("Stockpile", state.civ_resources.to_string(), "+28", 0.0),
                        ("Research Tier", format!("Tier {}", state.civ_tech_level), "", civ_tech_progress(state.civ_tech_level)),
                        ("Food Reserves", format!("{:.0}%", state.civ_food * 100.0), "", state.civ_food),
                        ("Power Grid", format!("{:.0}%", state.civ_energy * 100.0), "", state.civ_energy),
                    ]
                };

                // 3-column stats grid
                egui::Grid::new("civ_stats_grid_3col")
                    .num_columns(3)
                    .spacing(Vec2::new(theme.spacing_sm, theme.spacing_sm))
                    .show(ui, |ui| {
                        for (i, (label, value, trend, progress)) in stats.iter().enumerate() {
                            draw_stat_card(ui, theme, label, value, trend, *progress);
                            if (i + 1) % 3 == 0 {
                                ui.end_row();
                            }
                        }
                    });

                ui.add_space(theme.spacing_md);

                // Key metrics with progress bars
                let metrics: Vec<(&str, f32)> = if is_real {
                    vec![
                        ("Water Supply", state.civ_water),
                        ("Happiness", state.civ_happiness),
                    ]
                } else {
                    vec![
                        ("Water Recycling", state.civ_water),
                        ("Morale", state.civ_happiness),
                    ]
                };

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Key Metrics")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);
                    for (label, value) in &metrics {
                        ui.horizontal(|ui| {
                            ui.set_min_width(120.0);
                            ui.label(
                                RichText::new(*label)
                                    .size(theme.font_size_body)
                                    .color(theme.text_secondary()),
                            );
                        });
                        let color = if *value >= 0.7 {
                            theme.success()
                        } else if *value >= 0.4 {
                            theme.warning()
                        } else {
                            theme.danger()
                        };
                        let bar = egui::ProgressBar::new(value.clamp(0.0, 1.0))
                            .fill(color)
                            .text(format!("{:.0}%", value * 100.0));
                        ui.add(bar);
                        ui.add_space(theme.spacing_xs);
                    }
                });

                ui.add_space(theme.spacing_md);

                // Charts placeholder
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Charts")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_sm);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(500.0), 120.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, Rounding::same(8), theme.bg_sidebar());
                    ui.painter().rect_stroke(rect, Rounding::same(8), Stroke::new(1.0, theme.border()), egui::StrokeKind::Outside);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Charts coming soon",
                        egui::FontId::proportional(theme.font_size_body),
                        theme.text_muted(),
                    );
                });

                ui.add_space(theme.spacing_md);

                // Recent events timeline
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Recent Events")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);

                    if state.civ_events.is_empty() {
                        ui.label(
                            RichText::new("No recent events")
                                .color(theme.text_muted()),
                        );
                    } else {
                        ScrollArea::vertical()
                            .id_salt("civ_events_scroll")
                            .max_height(180.0)
                            .show(ui, |ui| {
                                for (i, event) in state.civ_events.iter().rev().enumerate() {
                                    ui.horizontal(|ui| {
                                        // Timeline dot
                                        let (dot_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(8.0, 8.0),
                                            egui::Sense::hover(),
                                        );
                                        let dot_color = if i == 0 { theme.accent() } else { theme.text_muted() };
                                        ui.painter().circle_filled(dot_rect.center(), 4.0, dot_color);

                                        ui.label(
                                            RichText::new(event)
                                                .size(theme.font_size_small)
                                                .color(if i == 0 { theme.text_primary() } else { theme.text_secondary() }),
                                        );
                                    });
                                }
                            });
                    }
                });
            });
        });
}

/// Draw a stat card with large number, trend arrow, and optional progress bar.
fn draw_stat_card(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str, trend: &str, progress: f32) {
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(label)
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(value)
                    .size(theme.font_size_title)
                    .color(theme.accent()),
            );
            if !trend.is_empty() {
                let (arrow, color) = if trend.starts_with('+') {
                    ("^", theme.success())
                } else if trend.starts_with('-') {
                    ("v", theme.danger())
                } else {
                    ("-", theme.text_muted())
                };
                ui.label(
                    RichText::new(format!("{} {}", arrow, trend))
                        .size(theme.font_size_small)
                        .color(color),
                );
            }
        });
        if progress > 0.0 {
            widgets::progress_bar(ui, theme, progress, None);
        }
    });
}

/// Calculate tech progress as a fraction for progress bar display.
fn civ_tech_progress(level: u32) -> f32 {
    // Show progress within current level (each level requires more)
    let max_level = 10u32;
    (level as f32 / max_level as f32).clamp(0.0, 1.0)
}
