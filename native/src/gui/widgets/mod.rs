//! Reusable egui widgets styled by the HumanityOS theme.

pub mod row;

use egui::{Color32, RichText, Rounding, Stroke, Ui, Vec2};
use super::theme::Theme;

/// Orange accent button. Returns true if clicked.
pub fn primary_button(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label).color(theme.text_on_accent()).size(theme.font_size_body),
    )
    .fill(theme.accent())
    .min_size(Vec2::new(0.0, theme.button_height))
    .rounding(Rounding::same(theme.border_radius as u8));
    ui.add(btn).clicked()
}

/// Outline button. Returns true if clicked.
pub fn secondary_button(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label).color(theme.text_primary()).size(theme.font_size_body),
    )
    .fill(Color32::TRANSPARENT)
    .stroke(Stroke::new(1.0, theme.border()))
    .min_size(Vec2::new(0.0, theme.button_height))
    .rounding(Rounding::same(theme.border_radius as u8));
    ui.add(btn).clicked()
}

/// Red danger button. Returns true if clicked.
pub fn danger_button(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label).color(Color32::WHITE).size(theme.font_size_body),
    )
    .fill(theme.danger())
    .min_size(Vec2::new(0.0, theme.button_height))
    .rounding(Rounding::same(theme.border_radius as u8));
    ui.add(btn).clicked()
}

/// Styled card container with background.
pub fn card(ui: &mut Ui, theme: &Theme, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            add_contents(ui);
        });
}

/// Card with a title header.
pub fn card_with_header(ui: &mut Ui, theme: &Theme, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    card(ui, theme, |ui| {
        ui.label(RichText::new(title).size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_sm);
        add_contents(ui);
    });
}

/// Collapsible section with header.
pub fn collapsible_section(ui: &mut Ui, title: &str, default_open: bool, add_contents: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title)
        .default_open(default_open)
        .show(ui, |ui| {
            add_contents(ui);
        });
}

/// Labeled slider. Returns true if value changed.
pub fn labeled_slider(ui: &mut Ui, theme: &Theme, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme.text_secondary()));
        ui.add(egui::Slider::new(value, range).show_value(true))
    }).inner.changed()
}

/// Toggle switch with label. Returns true if value changed.
pub fn toggle(ui: &mut Ui, theme: &Theme, label: &str, value: &mut bool) -> bool {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme.text_secondary()));
        let response = ui.checkbox(value, "");
        response
    }).inner.changed()
}

/// Progress bar (0.0 to 1.0).
pub fn progress_bar(ui: &mut Ui, theme: &Theme, progress: f32, label: Option<&str>) {
    let bar = egui::ProgressBar::new(progress.clamp(0.0, 1.0))
        .fill(theme.accent());
    let bar = if let Some(text) = label {
        bar.text(text)
    } else {
        bar
    };
    ui.add(bar);
}

/// Tab bar. Updates active index, returns true if changed.
pub fn tab_bar(ui: &mut Ui, theme: &Theme, tabs: &[&str], active: &mut usize) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (i, tab) in tabs.iter().enumerate() {
            let is_active = i == *active;
            let text = if is_active {
                RichText::new(*tab).color(theme.text_on_accent()).size(theme.font_size_body)
            } else {
                RichText::new(*tab).color(theme.text_secondary()).size(theme.font_size_body)
            };
            let fill = if is_active { theme.accent() } else { Color32::TRANSPARENT };
            let btn = egui::Button::new(text)
                .fill(fill)
                .rounding(Rounding::same(theme.border_radius as u8));
            if ui.add(btn).clicked() && !is_active {
                *active = i;
                changed = true;
            }
        }
    });
    changed
}

/// Role badge pill.
pub fn role_badge(ui: &mut Ui, theme: &Theme, role: &str) {
    let (color, letter) = match role {
        "admin" => (Theme::c32(&theme.badge_admin), "A"),
        "mod" => (Theme::c32(&theme.badge_mod), "M"),
        "verified" => (Theme::c32(&theme.badge_verified), "V"),
        "donor" => (Theme::c32(&theme.badge_donor), "D"),
        _ => return,
    };
    let text = RichText::new(letter).size(theme.font_size_small).color(Color32::WHITE);
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(3))
        .inner_margin(Vec2::new(4.0, 1.0))
        .show(ui, |ui| { ui.label(text); });
}

