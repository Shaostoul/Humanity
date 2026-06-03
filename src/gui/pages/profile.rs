//! Player Profile page with sidebar navigation and content panels.
//!
//! Sections organized by privacy level:
//! - PRIVATE (red): Body & Measurements, Identity, Private Notes
//! - PERSONAL (orange): Network Profile, Interests, Skills
//! - PUBLIC (green): Social Links, Streaming

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiState, ProfileSection};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub const PRIVATE_DOT: Color32 = Color32::from_rgb(231, 76, 60);
pub const PERSONAL_DOT: Color32 = Color32::from_rgb(237, 140, 36);
pub const PUBLIC_DOT: Color32 = Color32::from_rgb(46, 204, 113);

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Left sidebar with section list
    egui::SidePanel::left("profile_sidebar")
        .default_width(200.0)
        .min_width(160.0)
        .max_width(260.0)
        .frame(Frame::none()
            .fill(theme.bg_sidebar())
            .inner_margin(egui::Margin::symmetric(8, 12))
            .stroke(Stroke::new(1.0, theme.border())))
        .show(ctx, |ui| {
            // Universal section-nav widget (replaces the old local section_header +
            // sidebar_item helpers). Same grouped, dot-coded, switcher behaviour —
            // now reusable by every page in the coming Real/Play consolidation.
            let items = [
                widgets::SectionNavItem::new("body", "Body & Measurements", PRIVATE_DOT).group("PRIVATE"),
                widgets::SectionNavItem::new("identity", "Identity", PRIVATE_DOT),
                widgets::SectionNavItem::new("notes", "Private Notes", PRIVATE_DOT),
                widgets::SectionNavItem::new("network", "Network Profile", PERSONAL_DOT).group("PERSONAL"),
                widgets::SectionNavItem::new("interests", "Interests", PERSONAL_DOT),
                widgets::SectionNavItem::new("skills", "Skills", PERSONAL_DOT),
                widgets::SectionNavItem::new("quests", "Quests", PERSONAL_DOT),
                widgets::SectionNavItem::new("social", "Social Links", PUBLIC_DOT).group("PUBLIC"),
                widgets::SectionNavItem::new("streaming", "Streaming", PUBLIC_DOT),
            ];
            if let Some(clicked) =
                widgets::section_nav(ui, theme, Some("Profile"), &items, section_id(state.profile_section))
            {
                state.profile_section = section_from_id(&clicked);
            }
        });

    // Right content area
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                draw_section_content(ui, theme, state);
            });
        });
}

/// Render the currently-selected Profile section's content into `ui` — extracted
/// from `draw` so the merged **Real** tab can compose Profile's sections
/// alongside Inventory / Wallet / Tasks / Map / Market in ONE unified
/// section_nav page. The caller supplies the panel + scroll area.
pub fn draw_section_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    match state.profile_section {
        ProfileSection::BodyMeasurements => draw_body_measurements(ui, theme, state),
        ProfileSection::Identity => draw_identity(ui, theme, state),
        ProfileSection::PrivateNotes => draw_private_notes(ui, theme, state),
        ProfileSection::NetworkProfile => draw_network_profile(ui, theme, state),
        ProfileSection::Interests => draw_interests(ui, theme, state),
        ProfileSection::Skills => draw_skills(ui, theme, state),
        ProfileSection::Quests => draw_quests(ui, theme, state),
        ProfileSection::SocialLinks => draw_social_links(ui, theme, state),
        ProfileSection::Streaming => draw_streaming(ui, theme, state),
    }
}

/// Map a `ProfileSection` to the stable string id the section-nav widget uses.
pub fn section_id(section: ProfileSection) -> &'static str {
    match section {
        ProfileSection::BodyMeasurements => "body",
        ProfileSection::Identity => "identity",
        ProfileSection::PrivateNotes => "notes",
        ProfileSection::NetworkProfile => "network",
        ProfileSection::Interests => "interests",
        ProfileSection::Skills => "skills",
        ProfileSection::Quests => "quests",
        ProfileSection::SocialLinks => "social",
        ProfileSection::Streaming => "streaming",
    }
}

/// Inverse of [`section_id`] — unknown ids fall back to BodyMeasurements.
pub fn section_from_id(id: &str) -> ProfileSection {
    match id {
        "identity" => ProfileSection::Identity,
        "notes" => ProfileSection::PrivateNotes,
        "network" => ProfileSection::NetworkProfile,
        "interests" => ProfileSection::Interests,
        "skills" => ProfileSection::Skills,
        "quests" => ProfileSection::Quests,
        "social" => ProfileSection::SocialLinks,
        "streaming" => ProfileSection::Streaming,
        _ => ProfileSection::BodyMeasurements,
    }
}

/// Local helper — delegates to the universal form_row widget so all profile
/// fields share the same label-column alignment as the rest of the app.
fn field_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &mut String) {
    // Strip a trailing colon if present — form_row owns the visual style.
    let clean_label = label.trim_end_matches(':');
    widgets::form_row(ui, theme, clean_label, |ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(220.0));
    });
}

fn draw_body_measurements(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Body & Measurements").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card_with_header(ui, theme, "General", |ui| {
        field_row(ui, theme, "Height:", &mut state.profile_height);
        field_row(ui, theme, "Weight:", &mut state.profile_weight);
        field_row(ui, theme, "Eye Color:", &mut state.profile_eye_color);
        field_row(ui, theme, "Blood Type:", &mut state.profile_blood_type);
    });

    ui.add_space(theme.spacing_md);

    widgets::card_with_header(ui, theme, "Hair", |ui| {
        field_row(ui, theme, "Color:", &mut state.profile_hair_color);
        field_row(ui, theme, "Length:", &mut state.profile_hair_length);
        field_row(ui, theme, "Style:", &mut state.profile_hair_style);
        field_row(ui, theme, "Texture:", &mut state.profile_hair_texture);
    });

    ui.add_space(theme.spacing_md);

    widgets::card_with_header(ui, theme, "Clothing Measurements", |ui| {
        field_row(ui, theme, "Neck:", &mut state.profile_neck);
        field_row(ui, theme, "Shoulders:", &mut state.profile_shoulders);
        field_row(ui, theme, "Chest:", &mut state.profile_chest);
        field_row(ui, theme, "Waist:", &mut state.profile_waist);
        field_row(ui, theme, "Hips:", &mut state.profile_hips);
        field_row(ui, theme, "Thighs:", &mut state.profile_thighs);
        field_row(ui, theme, "Inseam:", &mut state.profile_inseam);
        field_row(ui, theme, "Shoe Size:", &mut state.profile_shoe_size);
        field_row(ui, theme, "Shirt Size:", &mut state.profile_shirt_size);
        field_row(ui, theme, "Pants Size:", &mut state.profile_pants_size);
    });
}

fn draw_identity(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Identity").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        field_row(ui, theme, "Display Name:", &mut state.profile_name);

        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Bio:").color(theme.text_secondary()));
        ui.add(egui::TextEdit::multiline(&mut state.profile_bio)
            .desired_width(ui.available_width().min(400.0))
            .desired_rows(4)
            .hint_text("Tell us about yourself..."));

        ui.add_space(theme.spacing_sm);
        field_row(ui, theme, "Pronouns:", &mut state.profile_pronouns);
        field_row(ui, theme, "Location:", &mut state.profile_location);
        field_row(ui, theme, "Website:", &mut state.profile_website);

        ui.add_space(theme.spacing_sm);
        // Public key display
        ui.horizontal(|ui| {
            ui.label(RichText::new("Public Key:").color(theme.text_secondary()));
            let key_display = if state.profile_public_key.is_empty() {
                "No key generated".to_string()
            } else if state.profile_public_key.len() > 16 {
                format!("{}...{}", &state.profile_public_key[..8], &state.profile_public_key[state.profile_public_key.len()-8..])
            } else {
                state.profile_public_key.clone()
            };
            ui.label(RichText::new(&key_display).color(theme.text_muted()).size(theme.font_size_small));
            if widgets::secondary_button(ui, theme, "Copy") {
                ui.ctx().copy_text(state.profile_public_key.clone());
            }
        });
    });
}

fn draw_private_notes(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Private Notes").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("These notes are stored locally and never shared.").color(theme.text_muted()).size(theme.font_size_small));
    ui.add_space(theme.spacing_md);

    ui.add(egui::TextEdit::multiline(&mut state.profile_private_notes)
        .desired_width(ui.available_width().min(600.0))
        .desired_rows(16)
        .hint_text("Write private notes here..."));
}

fn draw_network_profile(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Network Profile").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new("Your public-facing profile shown to other users.").color(theme.text_muted()).size(theme.font_size_small));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        field_row(ui, theme, "Display Name:", &mut state.profile_network_name);

        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Bio:").color(theme.text_secondary()));
        ui.add(egui::TextEdit::multiline(&mut state.profile_network_bio)
            .desired_width(ui.available_width().min(400.0))
            .desired_rows(3)
            .hint_text("Public bio..."));

        ui.add_space(theme.spacing_sm);
        field_row(ui, theme, "Avatar URL:", &mut state.profile_network_avatar);

        // Online status
        ui.add_space(theme.spacing_sm);
        let status_color = if state.server_connected { theme.success() } else { theme.text_muted() };
        let status_text = if state.server_connected { "Online" } else { "Offline" };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Status:").color(theme.text_secondary()));
            let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
            ui.painter().circle_filled(dot_rect.center(), 5.0, status_color);
            ui.label(RichText::new(status_text).color(status_color));
        });
    });
}

fn draw_interests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Interests").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, "New interest", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.profile_interest_input)
                .desired_width(200.0)
                .hint_text("Add interest..."));
            ui.add_space(theme.spacing_sm);
            if widgets::Button::primary("Add").show(ui, theme)
                && !state.profile_interest_input.trim().is_empty()
            {
                let interest = state.profile_interest_input.trim().to_string();
                if !state.profile_interests.contains(&interest) {
                    state.profile_interests.push(interest);
                }
                state.profile_interest_input.clear();
            }
        });

        ui.add_space(theme.spacing_sm);

        // Display interests as tags
        let mut to_remove = None;
        ui.horizontal_wrapped(|ui| {
            for (i, interest) in state.profile_interests.iter().enumerate() {
                let tag = egui::Frame::none()
                    .fill(theme.bg_card())
                    .rounding(Rounding::same(12))
                    .inner_margin(Vec2::new(8.0, 4.0))
                    .stroke(Stroke::new(1.0, theme.border()));
                tag.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new(interest).color(theme.text_primary()).size(theme.font_size_small));
                        if ui.small_button("x").clicked() {
                            to_remove = Some(i);
                        }
                    });
                });
            }
        });
        if let Some(idx) = to_remove {
            state.profile_interests.remove(idx);
        }
    });
}

fn draw_skills(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Skills").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        if !state.skills.is_empty() {
            // Live skills from the running game: level + XP toward the next level.
            // Earned by doing actions — craft → recipe's skill, harvest → farming,
            // mine → mining (SkillSystem applies the XP, the ECS syncs it here).
            // One row per skill in a Grid so the columns auto-align cleanly
            // (name | Lv | progress | XP) — was two rows each with a near-empty
            // bar that read as a stray dot. Operator: "one row, not two ...
            // cleanly stacked columns."
            egui::Grid::new("skills_grid")
                .num_columns(4)
                .spacing([16.0, theme.row_gap])
                .show(ui, |ui| {
                    for sk in state.skills.iter() {
                        ui.label(RichText::new(&sk.name).color(theme.text_secondary()).size(theme.font_size_body));
                        ui.label(RichText::new(format!("Lv {}", sk.level)).color(theme.text_primary()).size(theme.font_size_small));
                        let frac = if sk.xp_needed > 0 {
                            (sk.xp as f32 / sk.xp_needed as f32).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        ui.add(egui::ProgressBar::new(frac).desired_width(180.0).desired_height(8.0));
                        ui.label(
                            RichText::new(format!("{} / {} XP", sk.xp, sk.xp_needed))
                                .color(theme.text_muted())
                                .size(theme.font_size_small),
                        );
                        ui.end_row();
                    }
                });
        } else {
            // No live skill data (not in-world yet). The old static placeholder
            // list was misleading (wrong skill names, %s) — show a clear hint instead.
            ui.label(
                RichText::new("Enter the world to start gaining skills.")
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
        }
    });

    // Dev: max all skills (testing affordance — unlocks every #8b skill-gated
    // recipe in one click, mirroring "Dev: stock materials" for inventory).
    ui.add_space(theme.spacing_sm);
    if widgets::Button::secondary("Dev: max skills").show(ui, theme) {
        state.pending_dev_max_skills = true;
    }
}

fn draw_quests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Quests").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    let has_active = state.quests.iter().any(|q| !q.completed);
    let has_completed = state.quests.iter().any(|q| q.completed);

    if !has_active && !has_completed {
        widgets::card(ui, theme, |ui| {
            ui.label(
                RichText::new("No quests yet — start a game session to receive your first quest.")
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

fn draw_social_links(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Social Links").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        widgets::form_row(ui, theme, "Platform", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.profile_social_platform)
                .desired_width(120.0)
                .hint_text("e.g. Mastodon"));
        });
        widgets::form_row(ui, theme, "URL", |ui| {
            ui.add(egui::TextEdit::singleline(&mut state.profile_social_url)
                .desired_width(220.0)
                .hint_text("https://..."));
            ui.add_space(theme.spacing_sm);
            if widgets::Button::primary("Add").show(ui, theme)
                && !state.profile_social_platform.trim().is_empty()
                && !state.profile_social_url.trim().is_empty()
            {
                state.profile_social_links.push((
                    state.profile_social_platform.trim().to_string(),
                    state.profile_social_url.trim().to_string(),
                ));
                state.profile_social_platform.clear();
                state.profile_social_url.clear();
            }
        });

        ui.add_space(theme.spacing_sm);

        // Display existing links
        let mut to_remove = None;
        for (i, (platform, url)) in state.profile_social_links.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", platform)).color(theme.text_secondary()).strong());
                ui.label(RichText::new(url).color(theme.accent()).size(theme.font_size_small));
                if ui.small_button("Remove").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(idx) = to_remove {
            state.profile_social_links.remove(idx);
        }

        if state.profile_social_links.is_empty() {
            ui.label(RichText::new("No social links added yet.").color(theme.text_muted()));
        }
    });
}

fn draw_streaming(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Streaming").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        field_row(ui, theme, "Streaming URL:", &mut state.profile_streaming_url);

        ui.add_space(theme.spacing_sm);
        widgets::toggle(ui, theme, "Live Status", &mut state.profile_streaming_live);

        if state.profile_streaming_live {
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                let live_color = theme.danger();
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 5.0, live_color);
                ui.label(RichText::new("LIVE").color(live_color).strong());
            });
        }
    });
}
