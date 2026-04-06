//! Universal scrollable item list with selection.
//! Used by: file browser, quest log, skill list, tool catalog, etc.

use egui::{Color32, RichText, Rounding, Sense, Ui, Vec2};
use crate::gui::theme::Theme;

/// A single item in the list.
pub struct ListItem {
    /// Optional leading icon character.
    pub icon: Option<char>,
    /// Color for the icon circle background.
    pub icon_color: Option<Color32>,
    /// Main text line.
    pub primary_text: String,
    /// Smaller secondary text below the primary text.
    pub secondary_text: Option<String>,
    /// Right-aligned trailing text (count, date, etc.).
    pub trailing_text: Option<String>,
    /// Optional colored badge pill.
    pub badge: Option<(String, Color32)>,
}

/// Render a scrollable list with selection highlight.
///
/// Returns the index of an item that was clicked this frame, if any.
/// `selected` is updated in-place on click.
pub fn item_list(
    ui: &mut Ui,
    theme: &Theme,
    items: &[ListItem],
    selected: &mut Option<usize>,
    max_height: f32,
) -> Option<usize> {
    let mut clicked: Option<usize> = None;
    let row_h = theme.row_height + theme.item_padding * 3.0;
    let available_w = ui.available_width();

    egui::ScrollArea::vertical()
        .max_height(max_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (idx, item) in items.iter().enumerate() {
                let is_selected = *selected == Some(idx);

                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(available_w, row_h), Sense::click());

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();

                    // Background
                    let bg = if is_selected {
                        theme.accent_pressed()
                    } else if response.hovered() {
                        Theme::c32(&theme.bg_tertiary)
                    } else if idx % 2 == 0 {
                        theme.bg_primary()
                    } else {
                        theme.bg_secondary()
                    };
                    painter.rect_filled(rect, Rounding::ZERO, bg);

                    let pad = theme.item_padding;
                    let mut x = rect.min.x + pad;
                    let cy = rect.center().y;

                    // Icon circle
                    if let Some(ch) = item.icon {
                        let icon_r = theme.icon_small / 2.0;
                        let icon_cx = x + icon_r;
                        let icon_color = item.icon_color.unwrap_or_else(|| theme.accent());
                        painter.circle_filled(egui::pos2(icon_cx, cy), icon_r, icon_color);
                        painter.text(
                            egui::pos2(icon_cx, cy),
                            egui::Align2::CENTER_CENTER,
                            ch.to_string(),
                            egui::FontId::proportional(theme.font_size_small),
                            Color32::WHITE,
                        );
                        x += theme.icon_small + pad;
                    }

                    // Primary + secondary text
                    let text_x = x;
                    if item.secondary_text.is_some() {
                        // Two-line layout
                        let top_y = cy - theme.font_size_body * 0.6;
                        painter.text(
                            egui::pos2(text_x, top_y),
                            egui::Align2::LEFT_TOP,
                            &item.primary_text,
                            egui::FontId::proportional(theme.font_size_body),
                            theme.text_primary(),
                        );
                        if let Some(ref sec) = item.secondary_text {
                            let bot_y = cy + 1.0;
                            painter.text(
                                egui::pos2(text_x, bot_y),
                                egui::Align2::LEFT_TOP,
                                sec,
                                egui::FontId::proportional(theme.font_size_small),
                                theme.text_muted(),
                            );
                        }
                    } else {
                        painter.text(
                            egui::pos2(text_x, cy),
                            egui::Align2::LEFT_CENTER,
                            &item.primary_text,
                            egui::FontId::proportional(theme.font_size_body),
                            theme.text_primary(),
                        );
                    }

                    // Trailing text (right-aligned)
                    let mut right_x = rect.max.x - pad;
                    if let Some(ref trailing) = item.trailing_text {
                        painter.text(
                            egui::pos2(right_x, cy),
                            egui::Align2::RIGHT_CENTER,
                            trailing,
                            egui::FontId::proportional(theme.font_size_small),
                            theme.text_muted(),
                        );
                        // Shift left if we also have a badge
                        right_x -= 60.0;
                    }

                    // Badge pill
                    if let Some((ref label, color)) = item.badge {
                        let font = egui::FontId::proportional(theme.font_size_small);
                        let galley = painter.layout_no_wrap(label.clone(), font, Color32::WHITE);
                        let pill_w = galley.size().x + 8.0;
                        let pill_h = galley.size().y + 4.0;
                        let pill_rect = egui::Rect::from_min_size(
                            egui::pos2(right_x - pill_w, cy - pill_h / 2.0),
                            Vec2::new(pill_w, pill_h),
                        );
                        painter.rect_filled(pill_rect, Rounding::same(3), color);
                        painter.galley(
                            egui::pos2(pill_rect.min.x + 4.0, pill_rect.min.y + 2.0),
                            galley,
                            Color32::WHITE,
                        );
                    }
                }

                if response.clicked() {
                    *selected = Some(idx);
                    clicked = Some(idx);
                }
            }
        });

    clicked
}
