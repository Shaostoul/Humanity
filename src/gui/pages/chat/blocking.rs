//! The chat page's Block buttons (step C of docs/design/blocking-and-safe-mode.md, section 10d,
//! 2026-10-09): in the DM conversation header, on a person's profile (the member list's menu
//! opens it), and the Block entry of a member row's right-click menu. The message menu's Block
//! sits beside Report in chat.rs, and a contact request's in pages/safety.rs. What Block and
//! Unblock do is src/engine/block.rs.
//!
//! Takes `use super::*` like the page's other children.

use super::*;

/// What a Block button says when hovered.
pub(crate) const BLOCK_TIP: &str =
    "Hide everything from them on every server, and take back the pass you gave them. They are not told.";

/// What an Unblock button says when hovered.
pub(crate) const UNBLOCK_TIP: &str =
    "Show their messages again. They are not told, and nothing is given back: to be friends again, follow them.";

/// The DM conversation header: Block, or (once blocked) a "Blocked" mark and Unblock.
pub(super) fn draw_dm_header_block(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, peer: &str) {
    if peer.is_empty() || peer == state.profile_public_key {
        return;
    }
    if crate::engine::block::is_blocked(state, peer) {
        ui.label(RichText::new("Blocked").size(theme.font_size_small).color(theme.warning()));
        if widgets::Button::ghost("Unblock").tooltip(UNBLOCK_TIP).show(ui, theme) {
            crate::engine::block::unblock(state, peer);
        }
    } else if widgets::Button::ghost("Block").tooltip(BLOCK_TIP).show(ui, theme) {
        crate::engine::block::block(state, peer);
    }
}

/// A person's profile (opened from the member list, a message or a mention): Block, or a line
/// saying they are blocked and Unblock.
pub(super) fn draw_profile_block(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, key: &str) {
    if key.is_empty() || key == state.profile_public_key {
        return;
    }
    ui.add_space(theme.spacing_sm);
    if crate::engine::block::is_blocked(state, key) {
        ui.label(
            RichText::new("You blocked them. You see nothing from them, and they are not told.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        if widgets::Button::secondary("Unblock").full_width().tooltip(UNBLOCK_TIP).show(ui, theme) {
            crate::engine::block::unblock(state, key);
        }
    } else if widgets::Button::danger("Block").full_width().tooltip(BLOCK_TIP).show(ui, theme) {
        crate::engine::block::block(state, key);
    }
}

/// The Block (or Unblock) entry of a member row's right-click menu.
pub(super) fn member_menu(ui: &mut egui::Ui, state: &mut GuiState, key: &str) {
    if key.is_empty() || key == state.profile_public_key {
        return;
    }
    if crate::engine::block::is_blocked(state, key) {
        if ui.button("Unblock").on_hover_text(UNBLOCK_TIP).clicked() {
            crate::engine::block::unblock(state, key);
            ui.close_menu();
        }
    } else if ui.button("Block").on_hover_text(BLOCK_TIP).clicked() {
        crate::engine::block::block(state, key);
        ui.close_menu();
    }
}
