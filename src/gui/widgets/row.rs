//! Universal message/item row widget.
//!
//! Layout (per user feedback, v0.92.1):
//!   <userbox (first of sender-group only)>  <timestamp> · <content>
//!   The userbox anchors to the top-left. Message content wraps inside the
//!   right column. Each message gets a `timestamp · content` prefix even on
//!   continuation rows (same sender sending multiple short messages).
//!
//! All sizing, spacing, and font values come from `Theme` widget variables
//! so changing one value affects the entire UI consistently.

use egui::{Color32, Rect, Rounding, Sense, Vec2};
use egui::epaint::StrokeKind;
use crate::gui::theme::Theme;

/// Blue highlight color for hovered bordered boxes.
const HOVER_BLUE: Color32 = Color32::from_rgb(52, 152, 219);

/// Interpunct separator between timestamp and message content.
/// Kept as a single-character constant so it is easy to change globally.
pub const INTERPUNCT: &str = " \u{00B7} "; // ` · ` with spaces either side

/// Fixed width of the userbox column so all messages in a channel align.
/// Based on icon size + name column + border stroke + gap.
fn userbox_column_width(theme: &Theme) -> f32 {
    let icon = theme.header_height;             // square icon area
    let name_col = 84.0;                        // fixed name column width
    icon + name_col + theme.border_width + 2.0  // + thin vertical separator
}

/// The clickable area of the userbox (for opening profile modal).
/// Returns the rect relative to the row origin.
fn userbox_rect(row_origin: egui::Pos2, theme: &Theme) -> Rect {
    let w = userbox_column_width(theme);
    let h = theme.header_height;
    Rect::from_min_size(row_origin, Vec2::new(w, h))
}

/// Render a universal row with optional userbox and word-wrapped content.
///
/// `show_header` true = first message of a sender group; draws the userbox.
/// `show_header` false = continuation from same sender; indents content into
/// the right column but draws no userbox.
///
/// Every message, regardless of `show_header`, gets `timestamp · content`.
///
/// When `channeling` is true, the userbox border animates through the RGB
/// spectrum (used when the profile modal for this sender is open).
///
/// Returns the Response for the whole row. To find out whether the click
/// was on the userbox specifically (profile open) vs on the content area,
/// use `userbox_hit_rect()` with the response's interact pointer.
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
) -> MessageRowResponse {
    let full_width = ui.available_width();
    let border_color = theme.border();
    let text_color = theme.text_primary();
    let bw = theme.border_width;
    let icon_sz = theme.icon_size;
    let header_h = theme.header_height;
    let side_font = theme.name_size;
    let below_font = theme.body_size;
    let ts_font = theme.small_size;
    let painter = ui.painter();

    let ubox_w = userbox_column_width(theme);
    let content_left_offset = ubox_w + 4.0;
    let content_width = (full_width - content_left_offset - 2.0).max(60.0);

    // ── Build the message text: "<timestamp> · <content>" ──
    // Strip " UTC" suffix if present so it fits in the line.
    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim().to_string();
    let message_text = if ts_clean.is_empty() {
        content.to_string()
    } else {
        format!("{}{}{}", ts_clean, INTERPUNCT, content)
    };

    let content_galley = painter.layout(
        message_text.clone(),
        egui::FontId::proportional(below_font),
        text_color,
        content_width,
    );
    let content_h = content_galley.size().y.max(header_h) + 4.0;

    // Row height is at least the userbox height if we're drawing one, else
    // just the content height.
    let row_h = if show_header {
        header_h.max(content_h)
    } else {
        content_galley.size().y + 4.0
    };

    // ── Allocate + interact ──
    let (full_rect, response) =
        ui.allocate_exact_size(Vec2::new(full_width, row_h), Sense::click());

    if !ui.is_rect_visible(full_rect) {
        if channeling {
            ui.ctx().request_repaint();
        }
        return MessageRowResponse {
            response,
            userbox_rect: Rect::NOTHING,
        };
    }

    let painter = ui.painter();
    painter.rect_filled(full_rect, Rounding::same(4), bg_color);

    let hx = full_rect.min.x;
    let hy = full_rect.min.y;

    let mut userbox_hit = Rect::NOTHING;

    if show_header {
        // ── Userbox: icon + name/timestamp, left-anchored, top of row ──
        userbox_hit = userbox_rect(egui::pos2(hx, hy), theme);

        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
        let userbox_hovered = pointer_pos
            .map(|p| userbox_hit.contains(p))
            .unwrap_or(false);

        let btn_border_color = if channeling {
            rgb_from_time(ctx_time)
        } else if userbox_hovered {
            HOVER_BLUE
        } else {
            border_color
        };
        painter.rect_stroke(
            userbox_hit,
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
        let sep_x = hx + header_h;
        painter.line_segment(
            [
                egui::pos2(sep_x - 0.5, hy + pad),
                egui::pos2(sep_x - 0.5, hy + header_h - pad),
            ],
            egui::Stroke::new(bw, border_color),
        );

        // Name (truncate if too long)
        let name_x = sep_x + 4.0;
        let name_y = hy + 2.0;
        let name_galley = painter.layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(side_font),
            Color32::WHITE,
        );
        painter.galley(egui::pos2(name_x, name_y), name_galley, Color32::WHITE);
    }

    // ── Content column: always present, always offset by userbox width ──
    let content_x = hx + content_left_offset;
    let content_y = hy + 2.0;
    painter.galley(egui::pos2(content_x, content_y), content_galley, text_color);

    if channeling {
        ui.ctx().request_repaint();
    }

    MessageRowResponse {
        response,
        userbox_rect: userbox_hit,
    }
}

/// Response bundle for `message_row`.
/// - `response` is the full-row response (useful for right-click menus).
/// - `userbox_rect` is the exact hit area of the userbox button (empty when
///   show_header is false). Use `userbox_rect.contains(click_pos)` to tell
///   whether a click was on the userbox (open profile) or elsewhere.
pub struct MessageRowResponse {
    pub response: egui::Response,
    pub userbox_rect: Rect,
}

impl MessageRowResponse {
    /// Convenience: true if this frame had a click on the userbox specifically.
    pub fn userbox_clicked(&self, ctx: &egui::Context) -> bool {
        if !self.response.clicked() {
            return false;
        }
        let click_pos = ctx.input(|i| i.pointer.interact_pos().unwrap_or_default());
        self.userbox_rect.contains(click_pos)
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
