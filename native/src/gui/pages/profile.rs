//! Player Profile page — identity, stats, skills, and wallet info.
//!
//! Context-aware: Real mode shows real profile data, Sim mode shows
//! game character stats (health, level, XP).

use egui::{Color32, Frame, RichText, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Profile").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Mode indicator
                    let mode_text = if state.context_real { "Real Mode" } else { "Sim Mode" };
                    ui.label(RichText::new(mode_text).color(theme.accent()).size(theme.font_size_small));
                });
            });

            ui.add_space(theme.spacing_md);

            ScrollArea::vertical().show(ui, |ui| {
                // Two-panel layout
                ui.columns(2, |cols| {
                    // Left panel: avatar, name, key, status
                    cols[0].vertical(|ui| {
                        // Avatar placeholder (colored circle with initials)
                        let initials = if state.profile_name.is_empty() {
                            "?".to_string()
                        } else {
                            state.profile_name.chars()
                                .filter(|c| c.is_alphabetic())
                                .take(2)
                                .collect::<String>()
                                .to_uppercase()
                        };

                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(80.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 40.0, theme.accent());
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &initials,
                            egui::FontId::proportional(28.0),
                            theme.text_on_accent(),
                        );

                        ui.add_space(theme.spacing_sm);

                        // Name
                        let display_name = if state.profile_name.is_empty() {
                            if state.user_name.is_empty() { "Anonymous" } else { &state.user_name }
                        } else {
                            &state.profile_name
                        };
                        ui.label(RichText::new(display_name).size(theme.font_size_heading).color(theme.text_primary()));

                        // Public key (truncated)
                        let key_display = if state.profile_public_key.is_empty() {
                            "No key generated".to_string()
                        } else if state.profile_public_key.len() > 16 {
                            format!("{}...{}", &state.profile_public_key[..8], &state.profile_public_key[state.profile_public_key.len()-8..])
                        } else {
                            state.profile_public_key.clone()
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&key_display).color(theme.text_muted()).size(theme.font_size_small));
                            if widgets::secondary_button(ui, theme, "Copy") {
                                ui.ctx().copy_text(state.profile_public_key.clone());
                            }
                        });

                        ui.add_space(theme.spacing_xs);
                        // Online status
                        let status_color = if state.server_connected { theme.success() } else { theme.text_muted() };
                        let status_text = if state.server_connected { "Online" } else { "Offline" };
                        ui.horizontal(|ui| {
                            let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot_rect.center(), 5.0, status_color);
                            ui.label(RichText::new(status_text).color(status_color).size(theme.font_size_small));
                        });

                        ui.add_space(theme.spacing_md);

                        // Bio
                        ui.label(RichText::new("Bio").size(theme.font_size_body).color(theme.text_secondary()));
                        ui.add(egui::TextEdit::multiline(&mut state.profile_bio)
                            .desired_width(280.0)
                            .desired_rows(4)
                            .hint_text("Tell us about yourself..."));
                    });

                    // Right panel: stats, skills, wallet
                    cols[1].vertical(|ui| {
                        if state.context_real {
                            // Real mode: reputation, stats
                            widgets::card_with_header(ui, theme, "Stats", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Reputation:").color(theme.text_secondary()));
                                    ui.label(RichText::new("0").color(theme.text_primary()));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Member Since:").color(theme.text_secondary()));
                                    ui.label(RichText::new("--").color(theme.text_primary()));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Messages Sent:").color(theme.text_secondary()));
                                    ui.label(RichText::new("0").color(theme.text_primary()));
                                });
                            });
                        } else {
                            // Sim mode: game character stats
                            widgets::card_with_header(ui, theme, "Character Stats", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Health:").color(theme.text_secondary()));
                                    ui.label(RichText::new(format!("{:.0}/{:.0}",
                                        state.player_health * state.player_health_max,
                                        state.player_health_max))
                                        .color(theme.text_primary()));
                                });
                                widgets::progress_bar(ui, theme, state.player_health, Some("HP"));

                                ui.add_space(theme.spacing_xs);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Level:").color(theme.text_secondary()));
                                    ui.label(RichText::new("1").color(theme.text_primary()));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("XP:").color(theme.text_secondary()));
                                    ui.label(RichText::new("0 / 100").color(theme.text_primary()));
                                });
                                widgets::progress_bar(ui, theme, 0.0, Some("XP"));
                            });
                        }

                        ui.add_space(theme.spacing_md);

                        // Skills section
                        widgets::card_with_header(ui, theme, "Skills", |ui| {
                            let sample_skills = [
                                ("Farming", 0.3),
                                ("Crafting", 0.1),
                                ("Trading", 0.0),
                                ("Building", 0.05),
                            ];
                            for (skill, progress) in &sample_skills {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(*skill).color(theme.text_secondary()).size(theme.font_size_small));
                                });
                                widgets::progress_bar(ui, theme, *progress, None);
                                ui.add_space(2.0);
                            }
                        });

                        ui.add_space(theme.spacing_md);

                        // Wallet section
                        widgets::card_with_header(ui, theme, "Wallet", |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Address:").color(theme.text_secondary()));
                                let addr = if state.profile_public_key.is_empty() {
                                    "--".to_string()
                                } else if state.profile_public_key.len() > 12 {
                                    format!("{}...{}", &state.profile_public_key[..6], &state.profile_public_key[state.profile_public_key.len()-6..])
                                } else {
                                    state.profile_public_key.clone()
                                };
                                ui.label(RichText::new(&addr).color(theme.text_muted()).size(theme.font_size_small));
                                if widgets::secondary_button(ui, theme, "Copy") {
                                    ui.ctx().copy_text(state.profile_public_key.clone());
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("SOL Balance:").color(theme.text_secondary()));
                                ui.label(RichText::new("0.00").color(theme.text_primary()));
                            });
                        });
                    });
                });
            });
        });
}
