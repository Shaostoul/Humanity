//! Map Viewer — solar system orbit view with planet details.
//!
//! Left sidebar: planet list (clickable). Center: orbit visualization.
//! Right panel: selected planet details. Context-aware for Real/Sim.

use egui::{Color32, Pos2, RichText, Rounding, Stroke, Vec2};
use crate::gui::{GuiPage, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Assign each planet a distinct color for the orbit view.
fn planet_color(name: &str) -> Color32 {
    match name {
        "Mercury" => Color32::from_rgb(180, 170, 160),
        "Venus" => Color32::from_rgb(220, 190, 120),
        "Earth" => Color32::from_rgb(80, 160, 220),
        "Mars" => Color32::from_rgb(200, 100, 70),
        "Jupiter" => Color32::from_rgb(210, 180, 140),
        "Saturn" => Color32::from_rgb(220, 200, 150),
        "Uranus" => Color32::from_rgb(150, 210, 220),
        "Neptune" => Color32::from_rgb(80, 100, 200),
        _ => Color32::from_rgb(160, 160, 160),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 200));

    egui::Window::new("Maps")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(900.0, 600.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Solar System Map").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Back") {
                        state.active_page = GuiPage::EscapeMenu;
                    }
                    let mode_text = if state.context_real { "Real" } else { "Sim" };
                    ui.label(RichText::new(mode_text).color(theme.accent()).size(theme.font_size_small));
                });
            });

            ui.add_space(theme.spacing_sm);

            if state.context_real {
                // Real mode: earth-only with note
                widgets::card(ui, theme, |ui| {
                    ui.label(RichText::new("Earth - Real World").size(theme.font_size_heading).color(theme.accent()));
                    ui.add_space(theme.spacing_xs);
                    ui.label(RichText::new("Interactive map integration (OpenStreetMap) is planned for the native app.").color(theme.text_secondary()));
                    ui.label(RichText::new("Use the web version for real-world mapping features.").color(theme.text_muted()));
                });
            } else {
                // Sim mode: solar system view
                ui.columns(3, |cols| {
                    // Left sidebar: planet list
                    cols[0].vertical(|ui| {
                        ui.set_min_width(140.0);
                        ui.label(RichText::new("Planets").size(theme.font_size_heading).color(theme.text_primary()));
                        ui.add_space(theme.spacing_xs);

                        for (i, planet) in state.map_planets.iter().enumerate() {
                            let selected = state.map_selected_planet == Some(i);
                            let text_color = if selected { theme.accent() } else { theme.text_secondary() };
                            let pc = planet_color(&planet.name);

                            ui.horizontal(|ui| {
                                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot_rect.center(), 5.0, pc);
                                if ui.selectable_label(selected, RichText::new(&planet.name).color(text_color)).clicked() {
                                    state.map_selected_planet = Some(i);
                                }
                            });
                        }
                    });

                    // Center: orbit visualization
                    cols[1].vertical(|ui| {
                        let available = ui.available_size();
                        let size = available.x.min(available.y).min(400.0);
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
                        let center = rect.center();
                        let paint = ui.painter_at(rect);

                        // Background
                        paint.rect_filled(rect, Rounding::same(4), Color32::from_rgb(10, 10, 20));

                        // Sun at center
                        paint.circle_filled(center, 8.0, Color32::from_rgb(255, 200, 50));

                        // Draw orbits and planets
                        let max_au = 32.0; // Neptune is ~30 AU
                        let scale = (size / 2.0 - 15.0) / max_au * state.map_zoom;

                        for (i, planet) in state.map_planets.iter().enumerate() {
                            let orbit_r = planet.orbit_radius_au as f32 * scale;
                            if orbit_r < 5.0 || orbit_r > size / 2.0 { continue; }

                            // Orbit ring
                            paint.circle_stroke(center, orbit_r, Stroke::new(0.5, Color32::from_rgb(40, 40, 60)));

                            // Planet dot (place at a fixed angle offset per planet for visual spread)
                            let angle = (i as f32) * 0.75 + 0.5;
                            let px = center.x + orbit_r * angle.cos();
                            let py = center.y + orbit_r * angle.sin();
                            let planet_r = match planet.name.as_str() {
                                "Jupiter" | "Saturn" => 5.0,
                                "Uranus" | "Neptune" => 4.0,
                                _ => 3.0,
                            };
                            let pc = planet_color(&planet.name);
                            let is_selected = state.map_selected_planet == Some(i);

                            if is_selected {
                                paint.circle_stroke(Pos2::new(px, py), planet_r + 3.0, Stroke::new(1.0, theme.accent()));
                            }
                            paint.circle_filled(Pos2::new(px, py), planet_r, pc);
                        }
                    });

                    // Right panel: selected planet details
                    cols[2].vertical(|ui| {
                        ui.set_min_width(200.0);
                        if let Some(idx) = state.map_selected_planet {
                            if let Some(planet) = state.map_planets.get(idx) {
                                ui.label(RichText::new(&planet.name).size(theme.font_size_heading).color(theme.accent()));
                                ui.add_space(theme.spacing_sm);

                                widgets::card(ui, theme, |ui| {
                                    detail_row(ui, theme, "Type", &planet.planet_type);
                                    detail_row(ui, theme, "Radius", &format!("{:.0} km", planet.radius_km));
                                    detail_row(ui, theme, "Gravity", &format!("{:.2} m/s2", planet.gravity));
                                    detail_row(ui, theme, "Atmosphere", &planet.atmosphere);
                                    detail_row(ui, theme, "Moons", &planet.moons.to_string());
                                    detail_row(ui, theme, "Orbit", &format!("{:.2} AU", planet.orbit_radius_au));
                                });
                            }
                        } else {
                            ui.label(RichText::new("Select a planet").color(theme.text_muted()));
                        }
                    });
                });

                // Zoom slider at bottom
                ui.add_space(theme.spacing_sm);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Zoom:").color(theme.text_secondary()));
                    ui.add(egui::Slider::new(&mut state.map_zoom, 0.5..=5.0).show_value(true));
                });
            }
        });
}

fn detail_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", label)).color(theme.text_secondary()).size(theme.font_size_small));
        ui.label(RichText::new(value).color(theme.text_primary()).size(theme.font_size_small));
    });
}
