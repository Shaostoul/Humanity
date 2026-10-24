//! The chat page's half of key cards (2026-10-22, docs/design/key-cards.md): when the person a
//! conversation is with signed a new encryption key, the notice saying so, with Accept, at the
//! top of the conversation. Nothing is sealed to them until it is accepted
//! (engine/key_cards.rs). The website's twin is web/chat/chat-key-cards.js `drawKeyChangeNotice`.
//!
//! Takes `use super::*` like the page's other children.

use super::*;

/// `peer`'s key changed and the new one is waiting: the notice (core/key_card.rs `change_notice`,
/// the same words on both apps) and the Accept button. Nothing when no change is waiting.
pub(super) fn draw_key_change_notice(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, peer: &str) {
    let Some(notice) = crate::engine::key_cards::change_notice(state, peer) else { return };
    let mut accept = false;
    Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, theme.warning()))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(notice).size(theme.font_size_body).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            accept = widgets::Button::primary(crate::relay::core::key_card::ACCEPT_LABEL)
                .tooltip("Lock what you send them with their new key from now on")
                .show(ui, theme);
        });
    if accept && crate::engine::key_cards::accept(state, peer) {
        state.ws_status = "Their new key is accepted.".to_string();
    }
}

/// Why a DM to `partner` was not sent, in plain words, for a `build_dm_puts` failure `reason`.
/// With no key from a checked key card (`missing_peer_key`), which of the three it is: none on the
/// server, still on its way, or changed and not yet accepted (engine/key_cards.rs `refusal_text`).
pub(super) fn why_not_sent(state: &mut GuiState, reason: &str, partner: &str) -> String {
    match reason {
        "no_own_key" => "Can't send: your account isn't unlocked on this device. Restore it from your recovery phrase first.".to_string(),
        "missing_peer_key" => crate::engine::key_cards::refusal_text(state, partner)
            .unwrap_or_else(|| "Nothing was sent: there is no checked key for this person yet.".to_string()),
        "bad_own_key" => "Can't send: your encryption key could not be derived. Try Identity → Recover.".to_string(),
        _ => "Can't send: encrypting the message failed.".to_string(),
    }
}
