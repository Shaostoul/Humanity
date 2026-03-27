//! Universal message/item row widget.
//!
//! Renders a header (icon + name + timestamp) with content lines beside and
//! below it. Reusable for chat messages, inventory items, file browser entries,
//! etc.
//!
//! Layout (from pixel spec):
//! ```text
//! +--1px top border (RGB when channeling)------------------------------+
//! | 1px gap                                                            |
//! | +--1px--+ +--1px--------+  1px | text row 1                       |
//! | |1px gap| |1px gap      |  gap |                                   |
//! | |32x32  | |15px name    |      |                                   |
//! | | icon  | |2px gap      | -----+--                                 |
//! | |       | |15px time    |      | text row 2                        |
//! | |1px gap| |1px gap      |      |                                   |
//! | +--1px--+ +--1px--------+      |                                   |
//! | 1px gap                                                            |
//! +--1px bottom border---------------------------------------------+
//!   2px gap
//!   text row 3 (full width, no border)
//!   2px gap
//!   text row 4 ...
//! ```

use egui::{Color32, Rect, Sense, Vec2};
use egui::epaint::StrokeKind;

/// Blue highlight color for hovered bordered boxes.
const HOVER_BLUE: Color32 = Color32::from_rgb(52, 152, 219);

/// Header row height: 1px border + 1px gap + 1px border + 1px gap + 32px icon
/// + 1px gap + 1px border + 1px gap + 1px border = 36px outer total.
const HEADER_HEIGHT: f32 = 36.0;

/// Height per content row below the header: 2px gap + 16px text = 18px pitch.
const CONTENT_ROW_HEIGHT: f32 = 18.0;

/// Height per content row inside the header (text rows 1 and 2): 15px.
const HEADER_TEXT_HEIGHT: f32 = 15.0;

/// Render a universal row with optional header and content lines.
///
/// The header displays a bordered icon box and a bordered name/timestamp box
/// side by side, with the first two content lines beside them. Content lines
/// 3+ render below at full width.
///
/// When `show_header` is false (same-user continuation), only content lines
/// are drawn with 2px gaps between them.
///
/// When `channeling` is true, the top and bottom 1px borders animate through
/// the RGB spectrum.
///
/// Returns the `Response` for the header button area (icon + name boxes).
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
    let border_color = Color32::from_rgb(42, 42, 53); // #2a2a35

    // How many content lines go beside the header (rows 1 and 2)?
    let header_content_count = if show_header {
        content_lines.len().min(2)
    } else {
        0
    };
    // How many content lines go below the header?
    let below_count = if show_header {
        content_lines.len().saturating_sub(2)
    } else {
        content_lines.len()
    };

    // Total height calculation
    let header_h = if show_header { HEADER_HEIGHT } else { 0.0 };
    // Below-header rows: each is 2px gap + 16px text = 18px
    let below_h = below_count as f32 * CONTENT_ROW_HEIGHT;
    let total_height = header_h + below_h;

    if total_height <= 0.0 {
        // Nothing to draw; return a dummy response
        let (_, resp) = ui.allocate_exact_size(Vec2::ZERO, Sense::click());
        return resp;
    }

    let (full_rect, response) =
        ui.allocate_exact_size(Vec2::new(full_width, total_height), Sense::click());

    if !ui.is_rect_visible(full_rect) {
        if channeling {
            ui.ctx().request_repaint();
        }
        return response;
    }

    let painter = ui.painter();

    // Fill background across the full rect
    painter.rect_filled(full_rect, 0.0, bg_color);

    if show_header {
        let hx = full_rect.min.x;
        let hy = full_rect.min.y;

        // Determine if the mouse is over the icon+name button area for hover
        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());

        // ── Icon box dimensions ──
        // Outer: 36x36 (1 border + 1 gap + 32 content + 1 gap + 1 border)
        let icon_outer = Rect::from_min_size(egui::pos2(hx, hy), Vec2::new(36.0, 36.0));

        // ── Name/timestamp box dimensions ──
        // Measure text widths to size the name box
        let name_galley = painter.layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(13.0),
            Color32::WHITE,
        );
        let ts_galley = painter.layout_no_wrap(
            timestamp.to_string(),
            egui::FontId::proportional(11.0),
            Color32::from_rgb(106, 106, 117),
        );
        let text_content_w = name_galley.size().x.max(ts_galley.size().x);
        // Inner padding: 1px border + 1px gap on each side = 4px total horizontal
        let name_box_w = (text_content_w + 4.0).ceil();
        let name_box_outer = Rect::from_min_size(
            egui::pos2(hx + 36.0 + 1.0, hy), // 1px gap between icon box and name box
            Vec2::new(name_box_w, 36.0),
        );

        // The clickable "button" area = icon box + gap + name box
        let button_rect = Rect::from_min_max(icon_outer.min, name_box_outer.max);
        let button_hovered = pointer_pos
            .map(|p| button_rect.contains(p))
            .unwrap_or(false);
        let active_border = if channeling {
            rgb_from_time(ctx_time)
        } else if button_hovered {
            HOVER_BLUE
        } else {
            border_color
        };

        // ── Button border (wraps ONLY icon + name/timestamp, not text rows) ──
        let btn_border_color = if channeling {
            rgb_from_time(ctx_time)
        } else if button_hovered {
            HOVER_BLUE
        } else {
            border_color
        };
        painter.rect_stroke(
            button_rect,
            0.0,
            egui::Stroke::new(1.0, btn_border_color),
            StrokeKind::Outside,
        );

        // ── Icon box: inside the button border, no separate border needed ──
        // Inner area starts at (hx+1+1, hy+1+1), size 32x32
        let icon_inner = Rect::from_min_size(
            egui::pos2(hx + 2.0, hy + 2.0),
            Vec2::new(32.0, 32.0),
        );
        // Draw icon circle and letter
        painter.circle_filled(icon_inner.center(), 14.0, icon_color);
        painter.text(
            icon_inner.center(),
            egui::Align2::CENTER_CENTER,
            &icon_letter.to_uppercase().to_string(),
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );

        // ── Name/timestamp: inside the button border, separated from icon by a vertical line ──
        painter.line_segment(
            [egui::pos2(name_box_outer.min.x - 0.5, hy + 2.0), egui::pos2(name_box_outer.min.x - 0.5, hy + 34.0)],
            egui::Stroke::new(1.0, border_color),
        );
        // Name text: inside at (left + 1border + 1gap, top + 1border + 1gap)
        let name_x = name_box_outer.min.x + 2.0;
        let name_y = name_box_outer.min.y + 2.0;
        painter.galley(egui::pos2(name_x, name_y), name_galley, Color32::WHITE);
        // 2px gap between name and timestamp
        let ts_y = name_y + HEADER_TEXT_HEIGHT + 2.0;
        painter.galley(egui::pos2(name_x, ts_y), ts_galley, Color32::from_rgb(106, 106, 117));

        // ── Text rows 1 and 2: right of name box, aligned with name and timestamp ──
        let text_start_x = name_box_outer.max.x + 2.0; // 1px gap + 1px visual separation
        let text_color = Color32::from_rgb(232, 232, 234);

        if header_content_count >= 1 {
            // Text row 1: vertically aligned with the name line
            painter.text(
                egui::pos2(text_start_x, name_y),
                egui::Align2::LEFT_TOP,
                content_lines[0],
                egui::FontId::proportional(13.0),
                text_color,
            );
        }
        if header_content_count >= 2 {
            // Text row 2: vertically aligned with the timestamp line
            painter.text(
                egui::pos2(text_start_x, ts_y),
                egui::Align2::LEFT_TOP,
                content_lines[1],
                egui::FontId::proportional(13.0),
                text_color,
            );
        }

        // ── Content rows 3+ below the header ──
        let below_start_y = hy + HEADER_HEIGHT;
        for i in 0..below_count {
            let line_idx = i + 2; // content_lines index (skip first 2)
            let row_y = below_start_y + (i as f32 * CONTENT_ROW_HEIGHT);
            // 2px gap then 16px text
            painter.text(
                egui::pos2(hx + 2.0, row_y + 2.0),
                egui::Align2::LEFT_TOP,
                content_lines[line_idx],
                egui::FontId::proportional(14.0),
                text_color,
            );
        }
    } else {
        // No header -- continuation rows only
        let text_color = Color32::from_rgb(232, 232, 234);
        for (i, line) in content_lines.iter().enumerate() {
            let row_y = full_rect.min.y + (i as f32 * CONTENT_ROW_HEIGHT);
            // 2px gap then 16px text
            painter.text(
                egui::pos2(full_rect.min.x + 2.0, row_y + 2.0),
                egui::Align2::LEFT_TOP,
                *line,
                egui::FontId::proportional(14.0),
                text_color,
            );
        }
    }

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
