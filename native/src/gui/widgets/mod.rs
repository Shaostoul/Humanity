//! Reusable egui widgets styled by the HumanityOS theme.

pub mod row;

use egui::{Color32, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
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

/// Aligned settings row: fixed-width label on left, control on right.
pub fn settings_row(ui: &mut Ui, theme: &Theme, label: &str, add_control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(theme.settings_label_width, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new(label).color(theme.text_secondary()));
            },
        );
        add_control(ui);
    });
}

/// Custom slider with visible track bar and accent fill.
/// Returns true if value changed.
pub fn custom_slider(ui: &mut Ui, theme: &Theme, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let min = *range.start();
    let max = *range.end();
    let track_h = theme.slider_track_height;
    let thumb_r = theme.slider_thumb_radius;
    let desired_width = ui.available_width().min(250.0);
    let widget_height = thumb_r * 2.0 + 4.0;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(desired_width, widget_height),
        Sense::click_and_drag(),
    );

    // Handle drag/click interaction
    let old_value = *value;
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = min + t * (max - min);
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center_y = rect.center().y;
        let t = if (max - min).abs() < f32::EPSILON { 0.5 } else { (*value - min) / (max - min) };
        let thumb_x = rect.left() + t * rect.width();

        // Draw dim track (full width)
        let track_rect = Rect::from_min_max(
            egui::pos2(rect.left(), center_y - track_h / 2.0),
            egui::pos2(rect.right(), center_y + track_h / 2.0),
        );
        painter.rect_filled(track_rect, Rounding::same((track_h / 2.0) as u8), theme.slider_track());

        // Draw filled portion (left to thumb)
        let fill_rect = Rect::from_min_max(
            egui::pos2(rect.left(), center_y - track_h / 2.0),
            egui::pos2(thumb_x, center_y + track_h / 2.0),
        );
        painter.rect_filled(fill_rect, Rounding::same((track_h / 2.0) as u8), theme.accent());

        // Draw thumb circle
        let thumb_color = if response.hovered() || response.dragged() {
            theme.accent_hover()
        } else {
            theme.accent()
        };
        painter.circle_filled(egui::pos2(thumb_x, center_y), thumb_r, thumb_color);
    }

    let changed = (*value - old_value).abs() > f32::EPSILON;
    changed
}

/// Labeled slider with aligned label and custom track. Returns true if value changed.
pub fn labeled_slider(ui: &mut Ui, theme: &Theme, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let mut changed = false;
    settings_row(ui, theme, label, |ui| {
        changed = custom_slider(ui, theme, value, range.clone());
        // Show numeric value after slider
        let max = *range.end();
        let value_text = if max <= 1.0 {
            format!("{:.2}", *value)
        } else if max <= 20.0 {
            format!("{:.1}", *value)
        } else {
            format!("{:.0}", *value)
        };
        ui.label(RichText::new(value_text).color(theme.text_muted()).size(theme.font_size_small));
    });
    changed
}

/// Custom checkbox with visible border when unchecked.
/// Returns true if value changed.
pub fn custom_checkbox(ui: &mut Ui, theme: &Theme, value: &mut bool) -> bool {
    let size = theme.checkbox_size;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());

    if response.clicked() {
        *value = !*value;
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let rounding = Rounding::same(3);

        if *value {
            // Checked: filled accent background + checkmark
            painter.rect_filled(rect, rounding, theme.accent());

            // Draw checkmark
            let check_color = theme.text_on_accent();
            let cx = rect.center().x;
            let cy = rect.center().y;
            let s = size * 0.25;
            let points = [
                egui::pos2(cx - s, cy),
                egui::pos2(cx - s * 0.3, cy + s * 0.7),
                egui::pos2(cx + s, cy - s * 0.6),
            ];
            painter.line_segment([points[0], points[1]], Stroke::new(2.0, check_color));
            painter.line_segment([points[1], points[2]], Stroke::new(2.0, check_color));
        } else {
            // Unchecked: visible border outline (always visible)
            painter.rect_stroke(rect, rounding, Stroke::new(1.5, theme.border()), egui::epaint::StrokeKind::Outside);
        }
    }

    response.clicked()
}

/// Toggle switch with label and visible checkbox. Returns true if value changed.
pub fn toggle(ui: &mut Ui, theme: &Theme, label: &str, value: &mut bool) -> bool {
    let mut changed = false;
    settings_row(ui, theme, label, |ui| {
        changed = custom_checkbox(ui, theme, value);
    });
    changed
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

