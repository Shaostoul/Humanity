//! Universal message/item row widget.
//!
//! Renders a header (icon + name + timestamp) with word-wrapped content beside
//! and below it. Reusable for chat messages, inventory items, file browser
//! entries, etc.
//!
//! All sizing, spacing, and font values come from `Theme` widget variables
//! so changing one value affects the entire UI consistently.

use egui::{Color32, Rect, Sense, Vec2};
use egui::epaint::StrokeKind;
use crate::gui::theme::Theme;

/// Blue highlight color for hovered bordered boxes.
const HOVER_BLUE: Color32 = Color32::from_rgb(52, 152, 219);

/// Render a universal row with optional header and word-wrapped content.
///
/// The header displays a bordered icon box and a bordered name/timestamp box
/// side by side, with the first ~2 lines of content beside them (word-wrapped
/// to fit the available width). Remaining content wraps below at full width.
///
/// When `show_header` is false (same-user continuation), all content renders
/// below at full width, word-wrapped.
///
/// When `channeling` is true, the button border animates through the RGB
/// spectrum.
///
/// Returns the `Response` for the allocated area.
pub fn message_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon_letter: char,
    icon_color: Color32,
    name: &str,
    timestamp: &str,
    content: &str,
    show_header: bool,
    bg_color: Color32,
    channeling: bool,
    ctx_time: f64,
) -> egui::Response {
    let full_width = ui.available_width();
    let border_color = theme.border();
    let text_color = theme.text_primary();
    let gap = theme.half_gap();
    let bw = theme.border_width;
    let icon_sz = theme.icon_size;
    let header_h = theme.header_height;
    let side_font = theme.name_size;
    let below_font = theme.body_size;
    let ts_font = theme.small_size;
    let painter = ui.painter();

    if show_header {
        // ── Measure header elements to determine button width ──

        let name_galley = painter.layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(side_font),
            Color32::WHITE,
        );
        let ts_galley = painter.layout_no_wrap(
            timestamp.to_string(),
            egui::FontId::proportional(ts_font),
            Color32::from_rgb(106, 106, 117),
        );
        let text_content_w = name_galley.size().x.max(ts_galley.size().x);
        let name_box_w = (text_content_w + 4.0).ceil();

        // Icon outer box is header_h x header_h
        let icon_outer_sz = header_h;
        // Button right edge: icon_outer + gap + name_box_w
        let button_right_x = icon_outer_sz + gap + name_box_w;

        // ── Side text: word-wrapped galley beside the header ──
        let side_text_x_offset = button_right_x + 4.0; // 2px gap each side
        let side_width = (full_width - side_text_x_offset - 2.0).max(30.0);

        let side_galley = painter.layout(
            content.to_string(),
            egui::FontId::proportional(side_font),
            text_color,
            side_width,
        );

        // Determine how many galley rows fit beside the header.
        let max_side_y = icon_sz; // inner height available
        let side_rows = &side_galley.rows;
        let mut side_line_count = 0usize;
        for row in side_rows.iter() {
            if row.rect.min.y < max_side_y && side_line_count < 2 {
                side_line_count += 1;
            } else {
                break;
            }
        }

        // ── Determine below-header text ──
        let below_text = if side_line_count < side_rows.len() {
            let mut char_count = 0usize;
            for row_idx in 0..side_line_count {
                char_count += side_rows[row_idx].glyphs.len();
                if side_rows[row_idx].ends_with_newline {
                    char_count += 1;
                }
            }
            let byte_offset: usize = content.char_indices()
                .nth(char_count)
                .map(|(idx, _)| idx)
                .unwrap_or(content.len());
            content[byte_offset..].trim_start_matches([' ', '\n', '\r'])
        } else {
            ""
        };

        // Create below-header galley if there's overflow text
        let below_width = (full_width - 4.0).max(30.0);
        let below_galley = if !below_text.is_empty() {
            Some(painter.layout(
                below_text.to_string(),
                egui::FontId::proportional(below_font),
                text_color,
                below_width,
            ))
        } else {
            None
        };

        let below_h = below_galley.as_ref().map_or(0.0, |g| g.size().y + 4.0);
        let total_height = header_h + below_h;

        // ── Allocate and draw ──
        let (full_rect, response) =
            ui.allocate_exact_size(Vec2::new(full_width, total_height), Sense::click());

        if !ui.is_rect_visible(full_rect) {
            if channeling {
                ui.ctx().request_repaint();
            }
            return response;
        }

        let painter = ui.painter();
        painter.rect_filled(full_rect, theme.border_radius_widget, bg_color);

        let hx = full_rect.min.x;
        let hy = full_rect.min.y;

        // Hover detection for button area
        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
        let icon_outer = Rect::from_min_size(egui::pos2(hx, hy), Vec2::new(icon_outer_sz, header_h));
        let name_box_outer = Rect::from_min_size(
            egui::pos2(hx + icon_outer_sz + gap, hy),
            Vec2::new(name_box_w, header_h),
        );
        let button_rect = Rect::from_min_max(icon_outer.min, name_box_outer.max);
        let button_hovered = pointer_pos
            .map(|p| button_rect.contains(p))
            .unwrap_or(false);

        // Button border
        let btn_border_color = if channeling {
            rgb_from_time(ctx_time)
        } else if button_hovered {
            HOVER_BLUE
        } else {
            border_color
        };
        painter.rect_stroke(
            button_rect,
            theme.border_radius_widget,
            egui::Stroke::new(bw, btn_border_color),
            StrokeKind::Inside,
        );

        // Icon circle + letter (centered in the icon area with padding)
        let pad = (header_h - icon_sz) / 2.0;
        let icon_inner = Rect::from_min_size(
            egui::pos2(hx + pad, hy + pad),
            Vec2::new(icon_sz, icon_sz),
        );
        painter.circle_filled(icon_inner.center(), theme.icon_radius(), icon_color);
        painter.text(
            icon_inner.center(),
            egui::Align2::CENTER_CENTER,
            &icon_letter.to_uppercase().to_string(),
            egui::FontId::proportional(side_font),
            Color32::WHITE,
        );

        // Vertical separator between icon and name
        painter.line_segment(
            [
                egui::pos2(name_box_outer.min.x - 0.5, hy + pad),
                egui::pos2(name_box_outer.min.x - 0.5, hy + header_h - pad),
            ],
            egui::Stroke::new(bw, border_color),
        );

        // Name and timestamp text
        let name_x = name_box_outer.min.x + 2.0;
        let name_y = name_box_outer.min.y + pad;
        painter.galley(egui::pos2(name_x, name_y), name_galley, Color32::WHITE);
        let ts_y = name_y + side_font + 2.0;
        painter.galley(
            egui::pos2(name_x, ts_y),
            ts_galley,
            Color32::from_rgb(106, 106, 117),
        );

        // ── Draw side text (word-wrapped, clipped to header height) ──
        if !content.is_empty() {
            let side_text_pos = egui::pos2(hx + side_text_x_offset, hy + pad);
            let clip_rect = Rect::from_min_size(
                side_text_pos,
                Vec2::new(side_width, icon_sz),
            );
            let clipped = painter.with_clip_rect(clip_rect);
            clipped.galley(side_text_pos, side_galley, text_color);
        }

        // ── Draw below-header text ──
        if let Some(galley) = below_galley {
            let below_y = hy + header_h + 2.0;
            painter.galley(egui::pos2(hx + 2.0, below_y), galley, text_color);
        }

        if channeling {
            ui.ctx().request_repaint();
        }

        response
    } else {
        // ── No header: continuation rows, full-width word wrap ──
        let wrap_width = (full_width - 4.0).max(30.0);

        if content.is_empty() {
            let (_, resp) = ui.allocate_exact_size(Vec2::ZERO, Sense::click());
            return resp;
        }

        let galley = painter.layout(
            content.to_string(),
            egui::FontId::proportional(below_font),
            text_color,
            wrap_width,
        );

        let total_height = galley.size().y + 4.0;
        let (full_rect, response) =
            ui.allocate_exact_size(Vec2::new(full_width, total_height), Sense::click());

        if !ui.is_rect_visible(full_rect) {
            return response;
        }

        let painter = ui.painter();
        painter.rect_filled(full_rect, theme.border_radius_widget, bg_color);
        painter.galley(
            egui::pos2(full_rect.min.x + 2.0, full_rect.min.y + 2.0),
            galley,
            text_color,
        );

        response
    }
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
