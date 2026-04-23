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

/// Measure the userbox width for a given sender name. The userbox is:
///   [icon square] [ vertical separator ] [name with side padding]
/// Width adjusts to the actual name length (with a minimum so very short
/// names still have a reasonable hit-area for the profile click).
fn measure_userbox_width(painter: &egui::Painter, theme: &Theme, name: &str) -> f32 {
    let icon = theme.header_height; // square icon area
    let name_galley = painter.layout_no_wrap(
        name.to_string(),
        egui::FontId::proportional(theme.name_size),
        Color32::WHITE,
    );
    let name_w = name_galley.size().x.max(40.0); // minimum 40px so the hit area is reasonable
    icon + 6.0 + name_w + 6.0 // icon + pad + name + trailing pad
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
    let painter = ui.painter();

    // Userbox width adapts to the actual sender name length so short names
    // produce a tight layout with no dead horizontal space before the
    // message text. Same width is used for continuation rows (same sender,
    // no userbox drawn) so those rows align under the first message's text.
    let ubox_w = measure_userbox_width(&painter, theme, name);
    let content_left_offset = ubox_w + 6.0;
    let narrow_width = (full_width - content_left_offset - 2.0).max(80.0);

    // ── Build the message text: "<timestamp> · <content>" ──
    // Strip " UTC" suffix if present so it fits in the line.
    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim().to_string();
    let message_text = if ts_clean.is_empty() {
        content.to_string()
    } else {
        format!("{}{}{}", ts_clean, INTERPUNCT, content)
    };

    // ── Wrap-around layout: content flows beside userbox, then under it ──
    //
    // 1. Layout at narrow width (beside userbox).
    // 2. If the galley is shorter than the userbox, we are done — fits next
    //    to it. row height = max(userbox, galley).
    // 3. If taller, split the text roughly at the line where it overflows
    //    the userbox bottom, re-layout part 1 narrow (beside box) and part
    //    2 full-width (below box). Simple word-boundary split.
    let narrow_galley = painter.layout(
        message_text.clone(),
        egui::FontId::proportional(below_font),
        text_color,
        narrow_width,
    );

    let line_h = if narrow_galley.rows.is_empty() {
        below_font + 2.0
    } else {
        narrow_galley.size().y / narrow_galley.rows.len() as f32
    };
    let lines_that_fit_beside =
        ((header_h / line_h).floor() as usize).max(1);

    // Decide whether we need the wrap-under split.
    let needs_split = show_header
        && narrow_galley.rows.len() > lines_that_fit_beside
        && narrow_galley.size().y > header_h + 0.5;

    // If splitting, compute a char-boundary split point around the line that
    // overflows the userbox bottom. We use the character index of the first
    // glyph in that row, falling back to a word-boundary split if rows are
    // unavailable.
    let (beside_text, below_text) = if needs_split {
        split_at_row(&message_text, &narrow_galley, lines_that_fit_beside)
    } else {
        (message_text.clone(), String::new())
    };

    let beside_galley = if needs_split {
        painter.layout(
            beside_text.clone(),
            egui::FontId::proportional(below_font),
            text_color,
            narrow_width,
        )
    } else {
        narrow_galley
    };

    let below_galley = if !below_text.is_empty() {
        Some(painter.layout(
            below_text.clone(),
            egui::FontId::proportional(below_font),
            text_color,
            (full_width - 4.0).max(120.0),
        ))
    } else {
        None
    };

    let beside_h = beside_galley.size().y;
    let below_h = below_galley.as_ref().map_or(0.0, |g| g.size().y);

    let row_h = if show_header {
        header_h.max(beside_h) + below_h + 2.0
    } else {
        beside_h + 2.0
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
        // ── Userbox: icon + name, left-anchored, top of row ──
        userbox_hit = Rect::from_min_size(egui::pos2(hx, hy), Vec2::new(ubox_w, header_h));

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

        // Name (shown at vertical centre of userbox)
        let name_galley = painter.layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(side_font),
            Color32::WHITE,
        );
        let name_x = sep_x + 6.0;
        let name_y = hy + (header_h - name_galley.size().y) / 2.0;
        painter.galley(egui::pos2(name_x, name_y), name_galley, Color32::WHITE);
    }

    // ── Content beside userbox (first N lines) ──
    let content_x = hx + content_left_offset;
    let content_y = hy + 2.0;
    painter.galley(egui::pos2(content_x, content_y), beside_galley, text_color);

    // ── Content below userbox (wrap-under remainder) ──
    if let Some(bg) = below_galley {
        let below_y = hy + header_h.max(beside_h) + 2.0;
        painter.galley(egui::pos2(hx + 2.0, below_y), bg, text_color);
    }

    if channeling {
        ui.ctx().request_repaint();
    }

    MessageRowResponse {
        response,
        userbox_rect: userbox_hit,
    }
}

/// Split `text` near the end of row `max_rows_before_split` in `galley`.
/// Falls back to a word-boundary split at the proportional character offset.
fn split_at_row(text: &str, galley: &egui::Galley, max_rows_before_split: usize) -> (String, String) {
    let total_rows = galley.rows.len();
    if total_rows <= max_rows_before_split {
        return (text.to_string(), String::new());
    }
    // Approximate split: character count * (rows_beside / total_rows).
    let ratio = max_rows_before_split as f32 / total_rows as f32;
    let approx_byte = ((text.len() as f32) * ratio) as usize;
    // Find nearest whitespace boundary at or before approx_byte to avoid
    // splitting a word in half.
    let mut split_at = approx_byte.min(text.len());
    if split_at > 0 && split_at < text.len() {
        while split_at > 0 && !text.is_char_boundary(split_at) {
            split_at -= 1;
        }
        // Walk back to whitespace if possible
        let bytes = text.as_bytes();
        while split_at > 0 && !(bytes[split_at - 1] as char).is_whitespace() {
            split_at -= 1;
        }
        if split_at == 0 {
            split_at = approx_byte;
            while split_at < text.len() && !text.is_char_boundary(split_at) {
                split_at += 1;
            }
        }
    }
    let (a, b) = text.split_at(split_at);
    (a.trim_end().to_string(), b.trim_start().to_string())
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
