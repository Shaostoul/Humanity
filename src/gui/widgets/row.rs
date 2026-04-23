//! Universal message/item row widget.
//!
//! Layout (v0.95 — consistent-indent Discord-style):
//!
//!   [ICON] **Shaostoul** 12:34 · first message text
//!          12:35 · continuation (same sender, no name, same indent)
//!          12:36 · another continuation
//!   [ICON] **Other**      12:40 · new sender starts here
//!
//! The icon square is a fixed 32 px for every sender so all message text
//! aligns vertically at the same x for every user and every line. No
//! wrap-around-the-userbox — every line starts at one indent and wraps at
//! the right edge of the column. Continuation rows draw no icon but keep
//! the same indent so they align with the first message above.
//!
//! All sizing and fonts come from `Theme` so changes in theme.ron restyle
//! the whole chat at once.

use egui::{Color32, Rect, Rounding, Sense, Vec2};
use egui::epaint::StrokeKind;
use crate::gui::theme::Theme;

/// Blue highlight color for hovered bordered boxes.
const HOVER_BLUE: Color32 = Color32::from_rgb(52, 152, 219);

/// Interpunct separator between timestamp and message content.
pub const INTERPUNCT: &str = " \u{00B7} "; // ` · `

/// Fixed size of the icon/userbox column, used for every message in every
/// channel so all users' text aligns. Changing this number shifts every
/// message's indent consistently.
const USERBOX_SIZE: f32 = 32.0;

/// Horizontal gap between userbox and message text.
const USERBOX_GAP: f32 = 8.0;

/// Render a universal row with optional userbox and word-wrapped content.
///
/// `show_header` true  = first message of a sender group; draws userbox.
/// `show_header` false = continuation from same sender; no userbox but
///                       text starts at the same x offset so it aligns.
///
/// First row includes the sender's name in bold before the timestamp.
/// Continuation rows just show `timestamp · content`.
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
    let side_font = theme.name_size;
    let body_font = theme.body_size;
    let painter = ui.painter();

    // Constant content offset for every sender, every line — this is the
    // alignment invariant that keeps the channel visually tidy.
    let content_left_offset = USERBOX_SIZE + USERBOX_GAP;
    let content_width = (full_width - content_left_offset - 4.0).max(100.0);

    // Strip " UTC" so timestamps are tight.
    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim().to_string();

    // Build the text galley using a LayoutJob so we can bold the name only
    // on header rows and keep the rest at the normal weight.
    use egui::text::LayoutJob;
    let mut job = LayoutJob::default();
    job.wrap.max_width = content_width;

    if show_header && !name.is_empty() {
        job.append(
            name,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(side_font),
                color: Color32::WHITE,
                ..Default::default()
            },
        );
        job.append(
            "  ",
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(body_font),
                color: theme.text_muted(),
                ..Default::default()
            },
        );
    }
    if !ts_clean.is_empty() {
        job.append(
            &ts_clean,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(theme.small_size),
                color: theme.text_muted(),
                ..Default::default()
            },
        );
        job.append(
            INTERPUNCT,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(body_font),
                color: theme.text_muted(),
                ..Default::default()
            },
        );
    }
    job.append(
        content,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(body_font),
            color: text_color,
            ..Default::default()
        },
    );

    let galley = painter.layout_job(job);
    let text_h = galley.size().y;

    // Row height = exactly what the text needs, with a tiny padding. We do NOT
    // force a minimum row height based on USERBOX_SIZE any more — that was
    // producing 14 px of dead space under single-line first messages, which
    // read as an unwanted line break between the first message of a sender
    // block and their continuations. The userbox now adapts to the row.
    let row_h = (text_h + 2.0).max(16.0);

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
        // Userbox width is fixed (USERBOX_SIZE = 32) so message indents align
        // across all senders. Height matches the row exactly so there is no
        // dead space under short single-line messages.
        userbox_hit = Rect::from_min_size(
            egui::pos2(hx, hy),
            Vec2::new(USERBOX_SIZE, row_h),
        );

        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
        let userbox_hovered = pointer_pos
            .map(|p| userbox_hit.contains(p))
            .unwrap_or(false);

        let border_stroke = if channeling {
            rgb_from_time(ctx_time)
        } else if userbox_hovered {
            HOVER_BLUE
        } else {
            border_color
        };
        painter.rect_stroke(
            userbox_hit,
            theme.border_radius_widget,
            egui::Stroke::new(bw, border_stroke),
            StrokeKind::Inside,
        );

        // Filled circle with the sender's first letter. Icon sizes to fit
        // the row so short rows get a proportionally smaller icon, taller
        // rows keep a readable icon at the top.
        let icon_r = (row_h * 0.38).clamp(6.0, USERBOX_SIZE * 0.38);
        let icon_y = (hy + (USERBOX_SIZE / 2.0).min(row_h / 2.0)).max(hy + icon_r + 1.0);
        let icon_center = egui::pos2(userbox_hit.center().x, icon_y);
        painter.circle_filled(icon_center, icon_r, icon_color);
        painter.text(
            icon_center,
            egui::Align2::CENTER_CENTER,
            &icon_letter.to_uppercase().to_string(),
            egui::FontId::proportional(side_font.min(row_h - 4.0)),
            Color32::WHITE,
        );

        if channeling {
            ui.ctx().request_repaint();
        }
    }

    // Text content — fixed x offset for everyone so alignment is consistent.
    let content_x = hx + content_left_offset;
    let content_y = hy + 2.0;
    painter.galley(egui::pos2(content_x, content_y), galley, text_color);

    MessageRowResponse {
        response,
        userbox_rect: userbox_hit,
    }
}

/// Response bundle for `message_row`.
pub struct MessageRowResponse {
    pub response: egui::Response,
    pub userbox_rect: Rect,
}

impl MessageRowResponse {
    /// True if this frame had a click landing on the userbox specifically.
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
