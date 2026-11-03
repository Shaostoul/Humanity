//! THE ROOM ROW of the chat sidebar (BUG-242, 2026-11-03): one drawer for a room under ANY
//! server, the one the app is on and every other, so a room looks the same whichever server is
//! active. The operator: "For some reason the formatting changes depending on the active server.
//! The active server everything is really tight. The inactive server channels are loose." Until
//! then chat/left_panel.rs drew the active server's rooms in one block (painted rows the theme's
//! row height tall, shorter than a line of their text, the name at the panel's edge and pushed
//! right by a voice mark or an unread dot) and every other server's in another (a label in a
//! horizontal layout, indented 24 points, a line of text tall). Like the server row above it
//! (`draw_server_row`, one template for every server since field tests 3-6), a room row is now
//! painted from FIXED slots, so nothing moves when the active server changes or when a room
//! gains voice, an unread line or a mute.
//!
//! The row, left to right (every size a theme token):
//!   * the voice column, `room_indent` wide: the mic (or the speaker while you are in the room's
//!     voice) for a room with voice, empty for one without, so every room's name starts at the
//!     same place, indented under its server's name;
//!   * `# name`, then its marks: the read level's badge, the eye (only moderators post), the
//!     node-graph (federated);
//!   * at fixed places from the right edge: the unread dot, then the muted mark (a bell with a
//!     slash, while the room is muted).
//! Its height is a line of the body text, never less than the theme's row height: the "loose"
//! rows the operator saw under the other servers, which read better and match the tree under
//! each server's name. The theme's row height (15.9 points in the shipped theme) is shorter than
//! a line of the 16.4-point body text, which is what packed the active server's rows together.
//!
//! The drawer only draws and reports; the caller does what a click means, which differs: on the
//! server the app is on a click opens the room and the voice mark joins or leaves its voice; on
//! another server either switches to that server and lands in the room (chat/left_panel.rs).

use super::*;
use crate::gui::widgets::Hint as _;
use humanity_core::i18n::tf;
use humanity_protocol::chat_access::{PostLevel, ReadLevel};

/// Everything one room row shows, prepared by the caller.
pub(super) struct RoomRowSpec<'a> {
    pub id: &'a str,
    pub name: &'a str,
    /// The server the room is on: its mute is kept under this address.
    pub server_url: &'a str,
    pub server_name: &'a str,
    /// The room is on the server the app is on (a click opens it rather than switching).
    pub here: bool,
    /// The room open in the centre panel.
    pub open: bool,
    pub unread: bool,
    pub read_level: ReadLevel,
    pub post_level: PostLevel,
    pub federated: bool,
    /// The room has voice: `Some(true)` while you are in it.
    pub voice: Option<bool>,
}

/// What the person did on the row; the caller applies it.
pub(super) struct RoomRowOut {
    /// The whole row, for its right-click menu.
    pub row: egui::Response,
    /// A click on the row, away from the voice mark.
    pub opened: bool,
    /// A click on the voice mark of a room with voice.
    pub voice_clicked: bool,
}

/// The size of a row's marks: the voice mark, the level badge, the eye, the node-graph, the muted
/// mark. The theme's small icon, never taller than the text beside it.
fn mark_size(theme: &Theme) -> f32 {
    theme.icon_small.min(theme.body_size * 0.9)
}

/// Where a room's name starts, from the row's left edge: past the voice column every row keeps,
/// so the names line up under their server's name with or without a voice mark (BUG-242).
pub(super) fn room_indent(theme: &Theme) -> f32 {
    theme.item_padding + mark_size(theme) + theme.spacing_sm
}

/// A room row's height: a line of the body text, never less than the theme's row height or a mark.
/// The DM rows' name line is the same height (chat/left_panel.rs).
pub(super) fn text_row_height(ui: &egui::Ui, theme: &Theme) -> f32 {
    let line = ui.fonts(|f| f.row_height(&egui::FontId::proportional(theme.body_size)));
    theme.row_height.max(line).max(mark_size(theme))
}

/// Draw one room row at the cursor, the full width of the list.
pub(super) fn draw_room_row(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, spec: &RoomRowSpec) -> RoomRowOut {
    let mark = mark_size(theme);
    let indent = room_indent(theme);
    let height = text_row_height(ui, theme);
    let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), egui::Sense::click());
    let cy = rect.center().y;
    // The voice column is the indent itself, the row's full height: easy to hit.
    let voice_col = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + indent, rect.bottom()));
    let on_voice = spec.voice.is_some() && row.hover_pos().map_or(false, |p| voice_col.contains(p));
    let clicked_voice = spec.voice.is_some() && row.clicked() && row.interact_pointer_pos().map_or(false, |p| voice_col.contains(p));
    // The right-hand slots, fixed from the right edge: the muted mark outermost, the unread dot
    // beside it; the name and its marks stop before them.
    let mute_c = egui::pos2(rect.right() - theme.item_padding - mark * 0.5, cy);
    let dot_r = theme.status_dot_size * 0.375;
    let unread_c = egui::pos2(mute_c.x - mark * 0.5 - theme.spacing_xs - dot_r, cy);
    let text_right = unread_c.x - dot_r - theme.spacing_xs;
    // Resting on the level badge shows its own card (who reads, who posts) instead of the row's hint.
    let mut on_badge = false;

    if ui.is_rect_visible(rect) {
        let p = ui.painter().clone();
        let accent = theme.accent();
        let fill = if spec.open {
            // The open room: a dim wash of the accent, as the DM and scratchpad rows mark theirs.
            Color32::from_rgb( // theme-exempt: a wash of the accent token
                accent.r() / 5 + 15,
                accent.g() / 5 + 15,
                accent.b() / 5 + 15,
            )
        } else if row.hovered() {
            theme.server_row_hover()
        } else {
            theme.server_row_bg()
        };
        p.rect_filled(rect, 0.0, fill);
        if spec.open {
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(theme.row_accent_width, rect.height()));
            p.rect_filled(bar, 0.0, accent);
        }

        // The voice column.
        if let Some(joined) = spec.voice {
            let r = egui::Rect::from_center_size(egui::pos2(rect.left() + theme.item_padding + mark * 0.5, cy), Vec2::splat(mark));
            if joined {
                crate::gui::widgets::icons::paint_speaker(&p, r, theme.success());
            } else {
                let color = if on_voice { theme.text_primary() } else { theme.text_muted() };
                crate::gui::widgets::icons::paint_mic(&p, r, color);
            }
        }

        // The name and its marks, clipped before the right-hand slots.
        let text_color = if spec.open || spec.unread { theme.text_primary() } else { theme.text_secondary() };
        let clip = egui::Rect::from_min_max(rect.min, egui::pos2(text_right, rect.bottom())).intersect(p.clip_rect());
        let tp = p.with_clip_rect(clip);
        let name = tp.text(
            egui::pos2(rect.left() + indent, cy),
            egui::Align2::LEFT_CENTER,
            format!("# {}", spec.name),
            egui::FontId::proportional(theme.body_size),
            text_color,
        );
        // Each mark in turn after the name, drawn only where it fits whole (a narrow sidebar
        // shows fewer marks, never half of one).
        let mut x = name.right() + theme.spacing_sm;
        let mut next_mark = || {
            let r = egui::Rect::from_min_size(egui::pos2(x, cy - mark * 0.5), Vec2::splat(mark));
            x += mark + theme.spacing_xs;
            (r.right() <= text_right).then_some(r)
        };
        if let Some(badge) = next_mark() {
            super::access::paint_level_badge(&tp, badge, spec.read_level, theme.text_muted());
            if row.hovered() && ui.rect_contains_pointer(badge) {
                on_badge = true;
                super::access::badge_tooltip(ui, spec.id, spec.read_level, spec.post_level);
            }
        }
        if spec.post_level.staff_only() {
            if let Some(r) = next_mark() {
                crate::gui::widgets::icons::paint_eye(&tp, r, theme.text_muted());
            }
        }
        if spec.federated {
            if let Some(r) = next_mark() {
                crate::gui::widgets::icons::paint_federation(&tp, r, theme.text_muted());
            }
        }

        // The unread dot (the DM and group rows' mark).
        if spec.unread {
            p.circle_filled(unread_c, dot_r, theme.danger());
        }
    }
    // The muted mark (Mute room, chat/mute_menu.rs), saying until when on hover.
    if let Some(until) = super::mute_menu::room_mute(state, spec.server_url, spec.id) {
        let id = egui::Id::new(("room_muted_mark", norm_server_url(spec.server_url), spec.id));
        super::mute_menu::paint_muted_mark(ui, mute_c, theme.text_muted(), id, until);
    }

    if row.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // What a click does, in plain words: on a voice mark here, join or leave; elsewhere, open the
    // room, or on another server switch there and open it.
    let row = if on_badge {
        row
    } else if on_voice && spec.here {
        if spec.voice == Some(true) { row.hint("chat.voice_leave_hint") } else { row.hint("chat.voice_join_hint") }
    } else if spec.here {
        row.hint("chat.room_row_hint")
    } else {
        let words = tf("chat.room_row_other_hint", &[("room", &spec.name), ("server", &spec.server_name)]);
        crate::gui::widgets::hint::show_words(row, &words)
    };
    let opened = row.clicked() && !clicked_voice;
    RoomRowOut { row, opened, voice_clicked: clicked_voice }
}
