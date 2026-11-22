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
        .stroke(Stroke::new(1.0_f32, theme.warning()))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(notice).size(theme.font_size_body).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            accept = widgets::Button::primary(humanity_protocol::key_card::ACCEPT_LABEL)
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
/// In the app's own voice ([`crate::gui::AppVoice`]): the person's name, which a relay listed, goes
/// in only through `honest_name` (the owner branch's eighth review, H2: a name of the relay's
/// choosing wrote a sentence into the app's "not sent" line).
pub(super) fn why_not_sent(state: &mut GuiState, reason: &str, partner: &str) -> crate::gui::AppVoice {
    use crate::gui::AppVoice;
    match reason {
        "no_own_key" => AppVoice::say("chat.dm_not_sent_no_own_key", &[]),
        "missing_peer_key" => crate::engine::key_cards::refusal_voice(state, partner).unwrap_or_else(|| AppVoice::say("chat.dm_not_sent_no_checked_key", &[])),
        "bad_own_key" => AppVoice::say("chat.dm_not_sent_bad_own_key", &[]),
        _ => AppVoice::say("chat.dm_not_sent_failed", &[]),
    }
}
