//! Guilds page — guild browser with search, grid of guild cards,
//! detail view with members/chat placeholder, create guild form.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiGuild, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// Page-local state for guild chat.
struct GuildsPageState {
    chat_input: String,
    chat_messages: Vec<(String, String)>, // (sender, message)
    visibility_public: bool,
}

impl Default for GuildsPageState {
    fn default() -> Self {
        Self {
            chat_input: String::new(),
            chat_messages: Vec::new(),
            visibility_public: true,
        }
    }
}

thread_local! {
    static LOCAL: RefCell<GuildsPageState> = RefCell::new(GuildsPageState::default());
}

fn with_local<R>(f: impl FnOnce(&mut GuildsPageState) -> R) -> R {
    LOCAL.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Guilds")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, theme, "+ Create Guild") {
                        state.guild_show_create = true;
                        state.guild_selected = None;
                    }
                });
            });

            ui.add_space(theme.spacing_sm);

            // Search bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search:").color(theme.text_muted()));
                ui.add(
                    egui::TextEdit::singleline(&mut state.guild_search)
                        .desired_width(300.0)
                        .hint_text("Search guilds by name..."),
                );
            });

            ui.add_space(theme.spacing_md);

            if state.guild_show_create {
                draw_create_form(ui, theme, state);
            } else if let Some(idx) = state.guild_selected {
                if idx < state.guilds.len() {
                    draw_guild_detail(ui, theme, state, idx);
                } else {
                    state.guild_selected = None;
                }
            } else {
                draw_guild_grid(ui, theme, state);
            }
        });
}

fn draw_guild_grid(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let search = state.guild_search.to_lowercase();
    let filtered: Vec<_> = state
        .guilds
        .iter()
        .enumerate()
        .filter(|(_, g)| search.is_empty() || g.name.to_lowercase().contains(&search))
        .collect();

    if filtered.is_empty() {
        ui.add_space(theme.spacing_xl);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("No guilds found")
                    .size(theme.font_size_heading)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Create one to get started!")
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
        });
        return;
    }

    ScrollArea::vertical()
        .id_salt("guilds_grid")
        .show(ui, |ui| {
            let card_width = 260.0;
            let available_width = ui.available_width();
            let cols = ((available_width / (card_width + theme.spacing_sm)).floor() as usize).max(1);

            let mut col = 0;
            let mut click_idx = None;

            egui::Grid::new("guilds_card_grid")
                .spacing(Vec2::new(theme.spacing_sm, theme.spacing_sm))
                .show(ui, |ui| {
                    for (idx, guild) in &filtered {
                        let frame = egui::Frame::none()
                            .fill(theme.bg_card())
                            .rounding(Rounding::same(theme.border_radius as u8))
                            .stroke(Stroke::new(1.0, theme.border()))
                            .inner_margin(theme.card_padding);

                        frame.show(ui, |ui| {
                            ui.set_min_width(card_width - theme.card_padding * 2.0);
                            ui.set_max_width(card_width - theme.card_padding * 2.0);

                            let resp = ui.vertical(|ui| {
                                // Header: color dot + name
                                ui.horizontal(|ui| {
                                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                                    ui.painter().circle_filled(dot_rect.center(), 6.0, guild.color);
                                    ui.label(
                                        RichText::new(&guild.name)
                                            .size(theme.font_size_heading)
                                            .color(theme.text_primary()),
                                    );
                                });

                                // Member count
                                ui.label(
                                    RichText::new(format!("{} members", guild.members.len()))
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );

                                ui.add_space(theme.spacing_xs);

                                // Description preview
                                let preview: String = guild.description.chars().take(80).collect();
                                let preview = if guild.description.len() > 80 {
                                    format!("{}...", preview)
                                } else {
                                    preview
                                };
                                ui.label(
                                    RichText::new(preview)
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );

                                ui.add_space(theme.spacing_sm);

                                // Join / View button
                                if guild.is_member {
                                    let btn = egui::Button::new(
                                        RichText::new("View").size(theme.font_size_small).color(theme.text_on_accent()),
                                    ).fill(theme.accent()).min_size(Vec2::new(80.0, 28.0));
                                    if ui.add(btn).clicked() {
                                        click_idx = Some(*idx);
                                    }
                                } else {
                                    let btn = egui::Button::new(
                                        RichText::new("Join").size(theme.font_size_small).color(theme.text_primary()),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, theme.accent()))
                                    .min_size(Vec2::new(80.0, 28.0));
                                    if ui.add(btn).clicked() {
                                        click_idx = Some(*idx);
                                    }
                                }
                            }).response;

                            if resp.interact(egui::Sense::click()).clicked() {
                                click_idx = Some(*idx);
                            }
                        });

                        col += 1;
                        if col >= cols {
                            ui.end_row();
                            col = 0;
                        }
                    }
                });

            if let Some(idx) = click_idx {
                state.guild_selected = Some(idx);
            }
        });
}

fn draw_guild_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, idx: usize) {
    // Back button
    if widgets::secondary_button(ui, theme, "< Back to Guilds") {
        state.guild_selected = None;
        return;
    }

    ui.add_space(theme.spacing_sm);

    let guild = match state.guilds.get(idx) {
        Some(g) => g.clone(),
        None => {
            state.guild_selected = None;
            return;
        }
    };

    ScrollArea::vertical()
        .id_salt("guild_detail")
        .show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 8.0, guild.color);
                ui.label(
                    RichText::new(&guild.name)
                        .size(theme.font_size_title)
                        .color(theme.accent()),
                );
            });

            ui.add_space(theme.spacing_sm);

            // Full description
            widgets::card(ui, theme, |ui| {
                ui.label(
                    RichText::new("About")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                );
                ui.add_space(theme.spacing_xs);
                if guild.description.is_empty() {
                    ui.label(
                        RichText::new("No description provided")
                            .color(theme.text_muted()),
                    );
                } else {
                    ui.label(
                        RichText::new(&guild.description)
                            .color(theme.text_secondary()),
                    );
                }
            });

            ui.add_space(theme.spacing_md);

            ui.horizontal(|ui| {
                // Left: member list
                ui.vertical(|ui| {
                    ui.set_min_width(250.0);

                    widgets::card_with_header(ui, theme, &format!("Members ({})", guild.members.len()), |ui| {
                        ScrollArea::vertical()
                            .id_salt("guild_members_detail")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for (i, member) in guild.members.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        let role = if i == 0 { "Owner" } else { "Member" };
                                        ui.label(
                                            RichText::new(member)
                                                .color(theme.text_primary()),
                                        );
                                        ui.label(
                                            RichText::new(role)
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                    });
                                }
                            });
                    });
                });

                ui.add_space(theme.spacing_md);

                // Right: chat placeholder
                ui.vertical(|ui| {
                    widgets::card_with_header(ui, theme, "Guild Chat", |ui| {
                        ScrollArea::vertical()
                            .id_salt("guild_chat")
                            .max_height(160.0)
                            .show(ui, |ui| {
                                with_local(|local| {
                                    if local.chat_messages.is_empty() {
                                        ui.label(
                                            RichText::new("No messages yet. Start the conversation!")
                                                .color(theme.text_muted()),
                                        );
                                    } else {
                                        for (sender, msg) in &local.chat_messages {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(format!("{}:", sender))
                                                        .size(theme.font_size_small)
                                                        .color(theme.accent()),
                                                );
                                                ui.label(
                                                    RichText::new(msg)
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_primary()),
                                                );
                                            });
                                        }
                                    }
                                });
                            });

                        ui.add_space(theme.spacing_xs);
                        with_local(|local| {
                            ui.horizontal(|ui| {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut local.chat_input)
                                        .desired_width(200.0)
                                        .hint_text("Type a message..."),
                                );
                                if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                    || widgets::primary_button(ui, theme, "Send")
                                {
                                    if !local.chat_input.trim().is_empty() {
                                        local.chat_messages.push(("You".to_string(), local.chat_input.trim().to_string()));
                                        local.chat_input.clear();
                                    }
                                }
                            });
                        });
                    });
                });
            });

            ui.add_space(theme.spacing_md);

            // Join/Leave button
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
        });
}

fn draw_create_form(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Back button
    if widgets::secondary_button(ui, theme, "< Back to Guilds") {
        state.guild_show_create = false;
        return;
    }

    ui.add_space(theme.spacing_md);

    ui.label(
        RichText::new("Create Guild")
            .size(theme.font_size_heading)
            .color(theme.accent()),
    );

    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Name:").color(theme.text_secondary()).size(theme.font_size_body));
            ui.add(
                egui::TextEdit::singleline(&mut state.guild_new_name)
                    .desired_width(300.0)
                    .hint_text("Guild name"),
            );
        });

        ui.add_space(theme.spacing_sm);

        ui.label(RichText::new("Description:").color(theme.text_secondary()).size(theme.font_size_body));
        ui.add(
            egui::TextEdit::multiline(&mut state.guild_new_desc)
                .desired_width(f32::INFINITY)
                .desired_rows(4)
                .hint_text("What is this guild about?"),
        );

        ui.add_space(theme.spacing_sm);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Color:").color(theme.text_secondary()).size(theme.font_size_body));
            ui.color_edit_button_srgba(&mut state.guild_new_color);
        });

        ui.add_space(theme.spacing_sm);

        // Visibility toggle
        with_local(|local| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Visibility:").color(theme.text_secondary()).size(theme.font_size_body));
                let pub_btn = egui::Button::new(
                    RichText::new("Public")
                        .size(theme.font_size_small)
                        .color(if local.visibility_public { theme.text_on_accent() } else { theme.text_secondary() }),
                )
                .fill(if local.visibility_public { theme.accent() } else { theme.bg_card() });
                if ui.add(pub_btn).clicked() {
                    local.visibility_public = true;
                }

                let priv_btn = egui::Button::new(
                    RichText::new("Private")
                        .size(theme.font_size_small)
                        .color(if !local.visibility_public { theme.text_on_accent() } else { theme.text_secondary() }),
                )
                .fill(if !local.visibility_public { theme.accent() } else { theme.bg_card() });
                if ui.add(priv_btn).clicked() {
                    local.visibility_public = false;
                }
            });
        });
    });

    ui.add_space(theme.spacing_md);

    ui.horizontal(|ui| {
        let can_create = !state.guild_new_name.trim().is_empty();
        ui.add_enabled_ui(can_create, |ui| {
            if widgets::primary_button(ui, theme, "Create Guild") {
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
                state.guild_selected = Some(state.guilds.len() - 1);
            }
        });
        if widgets::secondary_button(ui, theme, "Cancel") {
            state.guild_show_create = false;
        }
    });
}
