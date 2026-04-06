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

const PRIVATE_DOT: Color32 = Color32::from_rgb(231, 76, 60);
const PERSONAL_DOT: Color32 = Color32::from_rgb(237, 140, 36);
const PUBLIC_DOT: Color32 = Color32::from_rgb(46, 204, 113);

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Left sidebar with section list
    egui::SidePanel::left("profile_sidebar")
        .default_width(200.0)
        .min_width(160.0)
        .max_width(260.0)
        .frame(Frame::none()
            .fill(Color32::from_rgb(22, 22, 28))
            .inner_margin(egui::Margin::symmetric(8, 12))
            .stroke(Stroke::new(1.0, theme.border())))
        .show(ctx, |ui| {
            ui.label(RichText::new("Profile").size(theme.font_size_heading).color(theme.text_primary()));
            ui.add_space(theme.spacing_md);

            // PRIVATE section header
            section_header(ui, theme, "PRIVATE", PRIVATE_DOT);
            sidebar_item(ui, theme, "Body & Measurements", ProfileSection::BodyMeasurements, PRIVATE_DOT, state);
            sidebar_item(ui, theme, "Identity", ProfileSection::Identity, PRIVATE_DOT, state);
            sidebar_item(ui, theme, "Private Notes", ProfileSection::PrivateNotes, PRIVATE_DOT, state);

            ui.add_space(theme.spacing_sm);

            // PERSONAL section header
            section_header(ui, theme, "PERSONAL", PERSONAL_DOT);
            sidebar_item(ui, theme, "Network Profile", ProfileSection::NetworkProfile, PERSONAL_DOT, state);
            sidebar_item(ui, theme, "Interests", ProfileSection::Interests, PERSONAL_DOT, state);
            sidebar_item(ui, theme, "Skills", ProfileSection::Skills, PERSONAL_DOT, state);

            ui.add_space(theme.spacing_sm);

            // PUBLIC section header
            section_header(ui, theme, "PUBLIC", PUBLIC_DOT);
            sidebar_item(ui, theme, "Social Links", ProfileSection::SocialLinks, PUBLIC_DOT, state);
            sidebar_item(ui, theme, "Streaming", ProfileSection::Streaming, PUBLIC_DOT, state);
        });

    // Right content area
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                match state.profile_section {
                    ProfileSection::BodyMeasurements => draw_body_measurements(ui, theme, state),
                    ProfileSection::Identity => draw_identity(ui, theme, state),
                    ProfileSection::PrivateNotes => draw_private_notes(ui, theme, state),
                    ProfileSection::NetworkProfile => draw_network_profile(ui, theme, state),
                    ProfileSection::Interests => draw_interests(ui, theme, state),
                    ProfileSection::Skills => draw_skills(ui, theme, state),
                    ProfileSection::SocialLinks => draw_social_links(ui, theme, state),
                    ProfileSection::Streaming => draw_streaming(ui, theme, state),
                }
            });
        });
}

fn section_header(ui: &mut egui::Ui, theme: &Theme, label: &str, color: Color32) {
    ui.horizontal(|ui| {
        let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
        ui.painter().circle_filled(dot_rect.center(), 4.0, color);
        ui.label(RichText::new(label).size(theme.font_size_small).color(color).strong());
    });
    ui.add_space(2.0);
}

fn sidebar_item(
    ui: &mut egui::Ui,
    theme: &Theme,
    label: &str,
    section: ProfileSection,
    dot_color: Color32,
    state: &mut GuiState,
) {
    let is_active = state.profile_section == section;
    let text_color = if is_active { Color32::WHITE } else { theme.text_muted() };
    let bg = if is_active {
        Color32::from_rgba_unmultiplied(dot_color.r(), dot_color.g(), dot_color.b(), 30)
    } else {
        Color32::TRANSPARENT
    };

    let btn = egui::Button::new(RichText::new(label).size(theme.font_size_body).color(text_color))
        .fill(bg)
        .stroke(if is_active {
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(dot_color.r(), dot_color.g(), dot_color.b(), 100))
        } else {
            Stroke::NONE
        })
        .rounding(Rounding::same(4))
        .min_size(Vec2::new(ui.available_width(), 28.0));

    if ui.add(btn).clicked() {
        state.profile_section = section;
    }
}

fn field_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme.text_secondary()).size(theme.font_size_body));
        ui.add_space(8.0);
        ui.add(egui::TextEdit::singleline(value).desired_width(200.0));
    });
    ui.add_space(2.0);
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
        // Add interest input
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut state.profile_interest_input)
                .desired_width(200.0)
                .hint_text("Add interest..."));
            if widgets::primary_button(ui, theme, "Add") && !state.profile_interest_input.trim().is_empty() {
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
        for (skill_name, progress) in state.profile_skills.iter() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(skill_name).color(theme.text_secondary()).size(theme.font_size_body));
                ui.label(RichText::new(format!("{:.0}%", progress * 100.0)).color(theme.text_muted()).size(theme.font_size_small));
            });
            widgets::progress_bar(ui, theme, *progress, None);
            ui.add_space(4.0);
        }
    });
}

fn draw_social_links(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Social Links").size(theme.font_size_title).color(theme.text_primary()));
    ui.add_space(theme.spacing_md);

    widgets::card(ui, theme, |ui| {
        // Add new link
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut state.profile_social_platform)
                .desired_width(100.0)
                .hint_text("Platform"));
            ui.add(egui::TextEdit::singleline(&mut state.profile_social_url)
                .desired_width(200.0)
                .hint_text("URL"));
            if widgets::primary_button(ui, theme, "Add") {
                if !state.profile_social_platform.trim().is_empty() && !state.profile_social_url.trim().is_empty() {
                    state.profile_social_links.push((
                        state.profile_social_platform.trim().to_string(),
                        state.profile_social_url.trim().to_string(),
                    ));
                    state.profile_social_platform.clear();
                    state.profile_social_url.clear();
                }
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
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 5.0, Color32::from_rgb(231, 76, 60));
                ui.label(RichText::new("LIVE").color(Color32::from_rgb(231, 76, 60)).strong());
            });
        }
    });
}
