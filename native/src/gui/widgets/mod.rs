//! Reusable GUI widgets for HumanityOS pages.
//!
//! Provides styled building blocks so pages stay consistent without
//! duplicating layout code.

use egui::{Color32, Rounding, Stroke, Ui};
use crate::gui::theme::Theme;

/// Draw a collapsible section with a header and body content.
/// Returns true if the section is currently expanded.
pub fn collapsible_section(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    default_open: bool,
    add_body: impl FnOnce(&mut Ui),
) -> bool {
    let id = ui.make_persistent_id(title);
    let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(id, default_open));

    let _header_response = ui.horizontal(|ui| {
        let arrow = if open { "\u{25BC}" } else { "\u{25B6}" };
        if ui.button(egui::RichText::new(arrow).size(12.0).color(theme.accent)).clicked() {
            open = !open;
        }
        ui.label(theme.subheading(title));
    });

    if open {
        ui.indent(id, |ui| {
            add_body(ui);
        });
    }

    ui.data_mut(|d| d.insert_persisted(id, open));
    open
}

/// Draw a card-style container with a title bar and content area.
pub fn card(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    add_content: impl FnOnce(&mut Ui),
) {
    egui::Frame::none()
        .fill(theme.panel_bg)
        .rounding(theme.rounding)
        .stroke(Stroke::new(1.0, theme.primary.linear_multiply(0.3)))
        .inner_margin(egui::Margin::same(theme.padding as i8))
        .show(ui, |ui| {
            ui.label(theme.subheading(title));
            ui.separator();
            add_content(ui);
        });
}

/// Draw a horizontal progress bar (0.0 to 1.0) with a label.
pub fn progress_bar(
    ui: &mut Ui,
    fraction: f32,
    color: Color32,
    label: &str,
    width: f32,
) {
    let height = 16.0;
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(width, height),
        egui::Sense::hover(),
    );

    let painter = ui.painter();

    // Background
    painter.rect_filled(
        rect,
        Rounding::same(3),
        Color32::from_rgba_premultiplied(30, 30, 40, 200),
    );

    // Fill
    let fill_width = rect.width() * fraction.clamp(0.0, 1.0);
    if fill_width > 0.0 {
        let fill_rect = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(fill_width, rect.height()),
        );
        painter.rect_filled(fill_rect, Rounding::same(3), color);
    }

    // Label centered
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::new(11.0, egui::FontFamily::Proportional),
        Color32::WHITE,
    );
}

/// Draw a styled primary button. Returns true if clicked.
pub fn primary_button(ui: &mut Ui, theme: &Theme, text: &str) -> bool {
    let button = egui::Button::new(
        egui::RichText::new(text).size(16.0).color(Color32::WHITE),
    )
    .fill(theme.primary.linear_multiply(0.6))
    .rounding(theme.rounding)
    .min_size(egui::vec2(120.0, 36.0));

    ui.add(button).clicked()
}

/// Draw a styled secondary/outline button. Returns true if clicked.
pub fn secondary_button(ui: &mut Ui, theme: &Theme, text: &str) -> bool {
    let button = egui::Button::new(
        egui::RichText::new(text).size(14.0).color(theme.text_dim),
    )
    .fill(Color32::TRANSPARENT)
    .stroke(Stroke::new(1.0, theme.text_dim.linear_multiply(0.5)))
    .rounding(theme.rounding)
    .min_size(egui::vec2(100.0, 32.0));

    ui.add(button).clicked()
}

/// Labeled slider with value display.
pub fn labeled_slider(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        ui.label(theme.body(label));
        ui.add(egui::Slider::new(value, range).show_value(true));
    });
}
