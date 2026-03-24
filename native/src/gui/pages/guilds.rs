//! Guilds page — guild list, detail panel, create form.

use egui::{Color32, RichText, Rounding, Stroke, Vec2};
use crate::gui::{GuiGuild, GuiPage, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Guilds")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(620.0, 480.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Guilds")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);

            // Search bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search:").color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.guild_search)
                        .desired_width(200.0)
                        .hint_text("Filter guilds..."),
                );
            });

            ui.add_space(theme.spacing_sm);

            ui.horizontal(|ui| {
                // ── Left: guild list ──
                ui.vertical(|ui| {
                    ui.set_min_width(240.0);
                    ui.set_max_width(240.0);

                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        let search = state.guild_search.to_lowercase();
                        let filtered: Vec<_> = state
                            .guilds
                            .iter()
                            .enumerate()
                            .filter(|(_, g)| {
                                search.is_empty() || g.name.to_lowercase().contains(&search)
                            })
                            .collect();

                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("No guilds found")
                                    .color(theme.text_muted()),
                            );
                        }

                        for (idx, guild) in filtered {
                            let is_selected = state.guild_selected == Some(idx);
                            let fill = if is_selected { theme.bg_card() } else { Color32::TRANSPARENT };
                            let stroke = if is_selected {
                                Stroke::new(1.0, theme.accent())
                            } else {
                                Stroke::NONE
                            };

                            let frame = egui::Frame::none()
                                .fill(fill)
                                .rounding(Rounding::same(4))
                                .stroke(stroke)
                                .inner_margin(8.0);

                            frame.show(ui, |ui| {
                                let resp = ui.horizontal(|ui| {
                                    // Color dot
                                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(dot_rect.center(), 5.0, guild.color);

                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(&guild.name)
                                                .color(theme.text_primary()),
                                        );
                                        ui.label(
                                            RichText::new(format!("{} members", guild.members.len()))
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                        // Description preview
                                        let preview: String = guild.description.chars().take(40).collect();
                                        let preview = if guild.description.len() > 40 {
                                            format!("{}...", preview)
                                        } else {
                                            preview
                                        };
                                        ui.label(
                                            RichText::new(preview)
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                    });
                                }).response;
                                if resp.interact(egui::Sense::click()).clicked() {
                                    state.guild_selected = Some(idx);
                                }
                            });
                        }
                    });
                });

                ui.separator();

                // ── Right: detail or create form ──
                ui.vertical(|ui| {
                    if state.guild_show_create {
                        // Create guild form
                        ui.label(
                            RichText::new("Create Guild")
                                .size(theme.font_size_heading)
                                .color(theme.accent()),
                        );
                        ui.add_space(theme.spacing_sm);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Name:").color(theme.text_secondary()));
                            ui.text_edit_singleline(&mut state.guild_new_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Description:").color(theme.text_secondary()));
                        });
                        ui.add(
                            egui::TextEdit::multiline(&mut state.guild_new_desc)
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .hint_text("What is this guild about?"),
                        );
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Color:").color(theme.text_secondary()));
                            ui.color_edit_button_srgba(&mut state.guild_new_color);
                        });

                        ui.add_space(theme.spacing_md);
                        ui.horizontal(|ui| {
                            if widgets::primary_button(ui, theme, "Create") && !state.guild_new_name.trim().is_empty() {
                                let id = state.guild_next_id;
                                state.guild_next_id += 1;
                                state.guilds.push(GuiGuild {
                                    id,
                                    name: state.guild_new_name.trim().to_string(),
                                    description: state.guild_new_desc.clone(),
                                    color: state.guild_new_color,
                                    members: vec!["You".to_string()],
                                    is_member: true,
                                });
                                state.guild_new_name.clear();
                                state.guild_new_desc.clear();
                                state.guild_show_create = false;
                            }
                            if widgets::secondary_button(ui, theme, "Cancel") {
                                state.guild_show_create = false;
                            }
                        });
                    } else if let Some(idx) = state.guild_selected {
                        if let Some(guild) = state.guilds.get(idx).cloned() {
                            ui.label(
                                RichText::new(&guild.name)
                                    .size(theme.font_size_heading)
                                    .color(theme.accent()),
                            );
                            ui.add_space(theme.spacing_xs);
                            ui.label(
                                RichText::new(&guild.description)
                                    .color(theme.text_secondary()),
                            );
                            ui.add_space(theme.spacing_md);

                            // Member list
                            widgets::card_with_header(ui, theme, &format!("Members ({})", guild.members.len()), |ui| {
                                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                                    for member in &guild.members {
                                        ui.label(
                                            RichText::new(member)
                                                .color(theme.text_primary()),
                                        );
                                    }
                                });
                            });

                            ui.add_space(theme.spacing_md);

                            // Join/leave
                            if guild.is_member {
                                if widgets::danger_button(ui, theme, "Leave Guild") {
                                    if let Some(g) = state.guilds.get_mut(idx) {
                                        g.is_member = false;
                                        g.members.retain(|m| m != "You");
                                    }
                                }
                            } else if widgets::primary_button(ui, theme, "Join Guild") {
                                if let Some(g) = state.guilds.get_mut(idx) {
                                    g.is_member = true;
                                    g.members.push("You".to_string());
                                }
                            }
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Select a guild or create one")
                                    .color(theme.text_muted()),
                            );
                        });
                    }

                    if !state.guild_show_create {
                        ui.add_space(theme.spacing_md);
                        if widgets::primary_button(ui, theme, "+ Create Guild") {
                            state.guild_show_create = true;
                            state.guild_selected = None;
                        }
                    }
                });
            });

            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Close") {
                state.active_page = GuiPage::EscapeMenu;
            }
        });
}
