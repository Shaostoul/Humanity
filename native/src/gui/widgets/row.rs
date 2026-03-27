//! Universal message/item row widget.
//!
//! Renders a header (icon + name + timestamp) followed by content lines.
//! Reusable for chat messages, inventory items, file browser entries, etc.

use egui::{Color32, Rect, RichText, Sense, Vec2};

/// Render a universal row with optional header and content lines.
///
/// The header displays a colored circle with an initial letter, a bold name,
/// and a muted timestamp. Content lines render below at 18px each.
///
/// When `show_header` is false (same-user continuation), only content lines
/// are drawn. The caller toggles `bg_color` between `#000000` and `#030303`
/// per unique sender group.
///
/// When `channeling` is true, the 1px top border animates through the RGB
/// spectrum using `ctx_time` (seconds from `egui::Context::input().time`).
///
/// Returns the `Response` for the header row (clickable for profile modals).
/// If the header is hidden, returns the response for the full content area.
pub fn message_row(
    ui: &mut egui::Ui,
    icon_letter: char,
    icon_color: Color32,
    name: &str,
    timestamp: &str,
    content_lines: &[&str],
    show_header: bool,
    bg_color: Color32,
    channeling: bool,
    ctx_time: f64,
) -> egui::Response {
    let full_width = ui.available_width();
    let border_color = Color32::from_rgb(40, 40, 48);

    // Calculate total height: header (36px if shown) + content rows (18px each)
    let header_height = if show_header { 36.0 } else { 0.0 };
    let content_height = content_lines.len() as f32 * 18.0;
    let total_height = header_height + content_height;

    // Allocate the full rect and make the header area clickable
    let (full_rect, response) =
        ui.allocate_exact_size(Vec2::new(full_width, total_height), Sense::click());

    if ui.is_rect_visible(full_rect) {
        let painter = ui.painter();

        // Fill background
        painter.rect_filled(full_rect, 0.0, bg_color);

        if show_header {
            let header_rect = Rect::from_min_size(full_rect.min, Vec2::new(full_width, 36.0));

            // 1px RGB top line (animated hue shift when channeling, else border color)
            let top_line_color = if channeling {
                rgb_from_time(ctx_time)
            } else {
                border_color
            };
            painter.rect_filled(
                Rect::from_min_size(header_rect.min, Vec2::new(full_width, 1.0)),
                0.0,
                top_line_color,
            );

            // 1px black gap
            let y = header_rect.min.y + 1.0;
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(header_rect.min.x, y), Vec2::new(full_width, 1.0)),
                0.0,
                Color32::BLACK,
            );

            // 32px icon area (left side, starting at y+2)
            let icon_y = y + 1.0;
            let icon_rect = Rect::from_min_size(
                egui::pos2(header_rect.min.x, icon_y),
                Vec2::new(32.0, 32.0),
            );
            // Colored circle with initial letter
            painter.circle_filled(icon_rect.center(), 14.0, icon_color);
            let initial = icon_letter.to_uppercase().to_string();
            painter.text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                &initial,
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );

            // Right of icon: 1px border, 1px black gap, then text area
            let text_x = header_rect.min.x + 32.0;
            // 1px vertical border right of icon
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(text_x, icon_y), Vec2::new(1.0, 32.0)),
                0.0,
                border_color,
            );
            // 1px black gap
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(text_x + 1.0, icon_y), Vec2::new(1.0, 32.0)),
                0.0,
                Color32::BLACK,
            );

            // Name text (bold, white) - 15px tall, baseline-aligned in the 32px icon area
            let name_y = icon_y + 7.0; // vertically center the two text lines in 32px
            painter.text(
                egui::pos2(text_x + 4.0, name_y),
                egui::Align2::LEFT_TOP,
                name,
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            );

            // Timestamp text (muted) below name with 2px gap
            let ts_y = name_y + 15.0 + 2.0;
            painter.text(
                egui::pos2(text_x + 4.0, ts_y),
                egui::Align2::LEFT_TOP,
                timestamp,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(106, 106, 117), // text_muted default
            );

            // 1px gap below icon area, 1px border bottom
            let bottom_y = icon_y + 32.0;
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(header_rect.min.x, bottom_y + 1.0),
                    Vec2::new(full_width, 1.0),
                ),
                0.0,
                border_color,
            );
        }

        // Content rows: 18px each (1px gap, 16px text, 1px gap)
        let content_start_y = full_rect.min.y + header_height;
        for (i, line) in content_lines.iter().enumerate() {
            let line_y = content_start_y + (i as f32 * 18.0);
            // Text at 1px offset from top of the 18px row, left-padded to align with name
            let text_x = if show_header {
                full_rect.min.x + 38.0 // 32px icon + 2px border/gap + 4px pad
            } else {
                full_rect.min.x + 38.0
            };
            painter.text(
                egui::pos2(text_x, line_y + 1.0),
                egui::Align2::LEFT_TOP,
                *line,
                egui::FontId::proportional(14.0),
                Color32::from_rgb(232, 232, 234), // text_primary default
            );
        }
    }

    // Request animation repaint when channeling
    if channeling {
        ui.ctx().request_repaint();
    }

    response
}

/// Generate an RGB color cycling through the hue spectrum over time.
fn rgb_from_time(time: f64) -> Color32 {
    let hue = ((time * 30.0) % 360.0) as f32; // 12-second full cycle
    let s = 1.0_f32;
    let l = 0.5_f32;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
