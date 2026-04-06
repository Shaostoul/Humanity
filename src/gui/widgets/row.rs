//! Universal message/item row widget.
//!
//! Renders a header (icon + name + timestamp) with word-wrapped content below.
//! Reusable for chat messages, inventory items, file browser entries, etc.
//!
//! All sizing, spacing, and font values come from `Theme` widget variables
//! so changing one value affects the entire UI consistently.

use egui::{Color32, Rect, Rounding, Sense, Vec2};
use egui::epaint::StrokeKind;
use crate::gui::theme::Theme;

/// Blue highlight color for hovered bordered boxes.
const HOVER_BLUE: Color32 = Color32::from_rgb(52, 152, 219);

/// Render a universal row with optional header and word-wrapped content.
///
/// Header: bordered icon box + name/timestamp box (clickable).
/// Content: full-width word-wrapped text below the header.
///
/// When `show_header` is false (same-user continuation), only content renders.
///
/// When `channeling` is true, the button border animates through the RGB spectrum.
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
        // ── Measure header elements ──
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
        let icon_outer_sz = header_h;

        // ── Content galley: full width below header ──
        let content_width = (full_width - 4.0).max(30.0);
        let content_galley = if !content.is_empty() {
            Some(painter.layout(
                content.to_string(),
                egui::FontId::proportional(below_font),
                text_color,
                content_width,
            ))
        } else {
            None
        };

        let content_h = content_galley.as_ref().map_or(0.0, |g| g.size().y + 4.0);
        let total_height = header_h + content_h;

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
        painter.rect_filled(full_rect, Rounding::same(8), bg_color);

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

        // Icon circle + letter
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

        // ── Content text below header ──
        if let Some(galley) = content_galley {
            let content_y = hy + header_h + 2.0;
            painter.galley(egui::pos2(hx + 2.0, content_y), galley, text_color);
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
        painter.rect_filled(full_rect, Rounding::same(8), bg_color);
        painter.galley(
            egui::pos2(full_rect.min.x + 2.0, full_rect.min.y + 2.0),
            galley,
            text_color,
        );

        response
    }
}

/// Generate an RGB color cycling through the hue spectrum over time.
pub fn rgb_from_time(time: f64) -> Color32 {
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
