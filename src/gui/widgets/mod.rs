//! Reusable egui widgets styled by the HumanityOS theme.

pub mod row;
pub mod icons;
pub mod data_table;
pub mod search_bar;
pub mod item_list;
pub mod stat_display;
pub mod modal;
pub mod toolbar;
pub mod help_modal;

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
        let rounding = Rounding::same((track_h / 2.0) as u8);

        // Draw dim track (unfilled portion: thumb to right)
        let track_right = Rect::from_min_max(
            egui::pos2(thumb_x, center_y - track_h / 2.0),
            egui::pos2(rect.right(), center_y + track_h / 2.0),
        );
        painter.rect_filled(track_right, rounding, theme.slider_track());

        // Draw gradient filled portion (left to thumb): blue -> green -> red
        // Paint in thin vertical slices for smooth gradient
        let fill_left = rect.left();
        let fill_width = thumb_x - fill_left;
        if fill_width > 0.5 {
            let steps = (fill_width as usize).max(1).min(120);
            let step_w = fill_width / steps as f32;
            for i in 0..steps {
                let local_t = i as f32 / steps as f32;
                // Blue(0%) -> Green(50%) -> Red(100%)
                let (r, g, b) = if local_t < 0.5 {
                    let s = local_t * 2.0; // 0..1 within first half
                    (0.0, s, 1.0 - s) // blue to green
                } else {
                    let s = (local_t - 0.5) * 2.0; // 0..1 within second half
                    (s, 1.0 - s, 0.0) // green to red
                };
                let color = Color32::from_rgb(
                    (r * 220.0 + 35.0) as u8,
                    (g * 220.0 + 35.0) as u8,
                    (b * 200.0 + 30.0) as u8,
                );
                let x0 = fill_left + i as f32 * step_w;
                let x1 = x0 + step_w + 0.5; // slight overlap to avoid gaps
                let slice = Rect::from_min_max(
                    egui::pos2(x0, center_y - track_h / 2.0),
                    egui::pos2(x1.min(thumb_x), center_y + track_h / 2.0),
                );
                // Only round the leftmost and rightmost slices
                let slice_round = if i == 0 { rounding } else { Rounding::ZERO };
                painter.rect_filled(slice, slice_round, color);
            }
        }

        // Draw thumb: filled circle with animated RGB border
        let thumb_center = egui::pos2(thumb_x, center_y);
        let thumb_fill = if response.hovered() || response.dragged() {
            Color32::from_rgb(240, 240, 245)
        } else {
            Color32::from_rgb(210, 210, 220)
        };
        painter.circle_filled(thumb_center, thumb_r, thumb_fill);
        // RGB animated border (1.5px)
        let ctx_time = ui.ctx().input(|i| i.time);
        let rgb_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
        painter.circle_stroke(thumb_center, thumb_r, egui::Stroke::new(1.5, rgb_color));
        // Request repaint for animation
        ui.ctx().request_repaint();
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

// ─────────────────────── UNIVERSAL WIDGETS ───────────────────────
// Shared across all pages. Use these instead of building inline.

/// Colored badge pill. Replaces 20+ inline badge patterns across pages.
/// `text` is the display label, `color` is the badge background.
pub fn badge(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(theme.badge_radius as u8))
        .inner_margin(theme.badge_padding())
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(theme.small_size).color(Color32::WHITE));
        });
}

/// Small badge variant (tighter padding).
pub fn badge_sm(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(theme.badge_radius as u8))
        .inner_margin(Vec2::new(4.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(theme.small_size).color(Color32::WHITE));
        });
}

/// Label: Value detail row. Used in maps, inventory, crafting detail panels.
pub fn detail_row(ui: &mut Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}:", label))
                .color(theme.text_secondary())
                .size(theme.small_size),
        );
        ui.label(
            RichText::new(value)
                .color(theme.text_primary())
                .size(theme.small_size),
        );
    });
}

/// Bold label: Value row (for headers/important stats).
pub fn detail_row_bold(ui: &mut Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}:", label))
                .color(theme.text_secondary())
                .size(theme.body_size)
                .strong(),
        );
        ui.label(
            RichText::new(value)
                .color(theme.text_primary())
                .size(theme.body_size)
                .strong(),
        );
    });
}

/// Search bar with label. Returns true if the text changed.
pub fn search_bar(ui: &mut Ui, theme: &Theme, value: &mut String, hint: &str) -> bool {
    let before = value.clone();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search:").color(theme.text_secondary()).size(theme.body_size));
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(200.0)
                .hint_text(hint),
        );
    });
    *value != before
}

/// Sidebar navigation with active-state highlighting.
/// Returns Some(index) if a new item was clicked.
pub fn sidebar_nav(
    ui: &mut Ui,
    theme: &Theme,
    items: &[&str],
    active: usize,
) -> Option<usize> {
    let mut clicked = None;
    for (i, label) in items.iter().enumerate() {
        let is_active = i == active;
        let bg = if is_active {
            Color32::from_rgba_unmultiplied(
                theme.accent().r(),
                theme.accent().g(),
                theme.accent().b(),
                30,
            )
        } else {
            Color32::TRANSPARENT
        };
        let text_color = if is_active { theme.accent() } else { theme.text_secondary() };
        let btn = egui::Button::new(
            RichText::new(*label).size(theme.body_size).color(text_color),
        )
        .fill(bg)
        .rounding(Rounding::same(4))
        .min_size(Vec2::new(ui.available_width(), 28.0));
        if ui.add(btn).clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Horizontal category filter buttons. Returns Some(new_index) if changed.
pub fn category_filter(
    ui: &mut Ui,
    theme: &Theme,
    categories: &[&str],
    active: usize,
) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        for (i, cat) in categories.iter().enumerate() {
            let is_active = i == active;
            let bg = if is_active { theme.accent() } else { theme.bg_card() };
            let text_color = if is_active { theme.text_on_accent() } else { theme.text_secondary() };
            let btn = egui::Button::new(
                RichText::new(*cat).size(theme.small_size).color(text_color),
            )
            .fill(bg)
            .rounding(Rounding::same(theme.badge_radius as u8));
            if ui.add(btn).clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// Stat card for dashboards (label, big value, optional trend text, optional progress bar).
pub fn stat_card(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    value: &str,
    trend: Option<&str>,
    progress: Option<f32>,
) {
    card(ui, theme, |ui| {
        ui.label(RichText::new(label).size(theme.small_size).color(theme.text_muted()));
        ui.label(RichText::new(value).size(theme.heading_size).color(theme.text_primary()).strong());
        if let Some(trend_text) = trend {
            let color = if trend_text.starts_with('+') { theme.success() } else { theme.danger() };
            ui.label(RichText::new(trend_text).size(theme.small_size).color(color));
        }
        if let Some(pct) = progress {
            progress_bar(ui, theme, pct, None);
        }
    });
}

/// Standard page frame. Use instead of hardcoding Color32::from_rgb(20, 20, 25).
pub fn page_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.card_padding)
}

/// Standard sidebar frame. Use instead of hardcoding Color32::from_rgb(22, 22, 28).
pub fn sidebar_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.bg_sidebar()).inner_margin(theme.panel_margin)
}

/// Dark sidebar frame (for chat-style panels). Uses bg_sidebar_dark.
pub fn sidebar_dark_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0)
}

/// Section header with consistent styling across all pages.
pub fn section_header(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.section_gap);
    ui.label(
        RichText::new(text)
            .size(theme.heading_size)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.row_gap);
}

/// Separator with theme-consistent spacing.
pub fn themed_separator(ui: &mut Ui, theme: &Theme) {
    ui.add_space(theme.section_gap);
    ui.separator();
    ui.add_space(theme.section_gap);
}

