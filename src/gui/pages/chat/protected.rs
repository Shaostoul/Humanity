//! The chat page's half of the protected setup (step G of docs/design/blocking-and-safe-mode.md,
//! section 10h, 2026-10-10): the always-visible line in the left rail, the line where public
//! rooms are left out of the lists, the line drawn instead of a left-out room's messages, and the
//! line drawn instead of the pictures and files of someone who is not a friend. Every sentence is
//! the preset's (data/gui/safety_presets.json), read through engine/protected.rs; nothing here is
//! a sentence of its own. The rules (which room, whose pictures) are engine/protected.rs.
//!
//! Takes `use super::*` like the page's other children.

use super::*;

/// One muted line in a card the width of the rail or panel.
fn line_card(ui: &mut egui::Ui, theme: &Theme, text: &str, strong: bool) {
    Frame::NONE
        .fill(theme.bg_card())
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let mut words = RichText::new(text).size(theme.font_size_small);
            words = if strong { words.color(theme.text_primary()) } else { words.color(theme.text_muted()) };
            ui.label(words);
        });
}

/// THE ALWAYS-VISIBLE LINE (10h, the ICO standard 11 point: the person is told), one line in the
/// chat sidebar above the direct messages while the setup is on: the preset's `status_line`,
/// whose words do not depend on who turned it on. Nothing while it is off.
pub(super) fn draw_sidebar_line(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !crate::engine::protected::is_on(state) || !crate::engine::protected::ensure_preset(state) {
        return;
    }
    let Some(line) = state.protected.preset.as_ref().map(|p| p.status_line.clone()) else { return };
    line_card(ui, theme, &line, true);
}

/// The one line in the channel list while public rooms are left out (10h): the preset's
/// `public_rooms_hidden_line`, at the top of the Servers section, so every server's list below it
/// is covered and the rows of the active and the saved servers stay the same height.
pub(super) fn draw_hidden_rooms_line(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !crate::engine::protected::hides_public_rooms(state) || !crate::engine::protected::ensure_preset(state) {
        return;
    }
    let Some(line) = state.protected.preset.as_ref().map(|p| p.public_rooms_hidden_line.clone()) else { return };
    line_card(ui, theme, &line, false);
}

/// The centre panel (and the in-world chat panel) for a public room the setup leaves out: the same
/// line instead of the room's messages and composer. True when it drew, and the caller then draws
/// nothing more of the room. Also loads the preset for the message rows' picture line.
pub(super) fn draws_hidden_room(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> bool {
    if !crate::engine::protected::is_on(state) || !crate::engine::protected::ensure_preset(state) {
        return false;
    }
    if !crate::engine::protected::hides_active_room(state) {
        return false;
    }
    let Some(line) = state.protected.preset.as_ref().map(|p| p.public_rooms_hidden_line.clone()) else { return false };
    ui.add_space(theme.spacing_md);
    line_card(ui, theme, &line, true);
    true
}

/// What a message row shows of its text while the setup leaves out the sender's pictures and
/// files (10h: "Pictures and files from non-friends are not shown at all"): None when there is
/// nothing to leave out (no picture, no file) or the rule does not apply to this sender. Some
/// otherwise: for a private file's marker, the preset's line itself; for a message with links,
/// its text without every picture link and every link to an audio, video or document file
/// (`net::protected::file_urls`, the web's list), the row then drawing the line below it.
/// (Before the 2026-10-10 review only pictures and private files were left out here, while the
/// web also left out audio, video and document links.)
pub(super) fn withheld(state: &GuiState, msg: &ChatMessage) -> Option<String> {
    let marker = attach_view::file_in(msg).is_some();
    let pictures = crate::gui::widgets::image_cache::extract_image_urls(&msg.content);
    let files = crate::net::protected::file_urls(&msg.content);
    if !marker && pictures.is_empty() && files.is_empty() {
        return None;
    }
    if !crate::engine::protected::hides_pictures_from(state, &msg.sender_key) {
        return None;
    }
    if marker {
        return Some(picture_hidden_line(state).to_string());
    }
    let without_pictures = crate::gui::widgets::image_cache::strip_image_urls(&msg.content);
    Some(crate::net::protected::strip_file_urls(&without_pictures))
}

/// The preset's `picture_hidden_line` ("A picture from someone who is not a friend is not
/// shown."), for a message row whose pictures or file `engine::protected::hides_pictures_from`
/// leaves out. Empty when the preset is not loaded (then nothing is shown in its place either).
pub(super) fn picture_hidden_line(state: &GuiState) -> &str {
    state.protected.preset.as_ref().map(|p| p.picture_hidden_line.as_str()).unwrap_or("")
}

/// That line, drawn under a message where its pictures would be, indented like them.
pub(super) fn draw_picture_hidden(ui: &mut egui::Ui, theme: &Theme, row_bg: Color32, indent: f32, line: &str) {
    let row_w = ui.available_width();
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(row_w, theme.font_size_small + 10.0), egui::Sense::hover());
    ui.painter().rect_filled(row_rect, 0.0, row_bg);
    ui.painter_at(row_rect).text(
        egui::pos2(row_rect.left() + indent, row_rect.center().y),
        egui::Align2::LEFT_CENTER,
        line,
        egui::FontId::proportional(theme.font_size_small),
        theme.text_muted(),
    );
}

#[cfg(test)]
#[path = "protected_tests.rs"]
mod tests;
