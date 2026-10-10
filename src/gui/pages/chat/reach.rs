//! The chat page's half of "Who can reach me" (step B of docs/design/blocking-and-safe-mode.md,
//! section 10c, 2026-10-09): the notice shown where a message was refused, with its Send request
//! button, and the Requests section of the left rail. The list itself is drawn by
//! `pages::safety::draw_requests_list`, which Settings > Safety shows too.
//!
//! Takes `use super::*` like the page's other children.

use super::*;

/// When `peer`'s gate refused a message from us (`reach_refused`): the sentence 10c gives, word
/// for word, and a Send request button; once sent, what happens next. Drawn at the top of the
/// conversation with them and on their profile card; nothing when nothing was refused.
pub(super) fn draw_refusal_notice(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, peer: &str) {
    let Some(&refusal) = state.reach.refused.get(peer) else { return };
    let mut send = false;
    Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, theme.warning()))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if refusal == crate::net::reach::Refusal::NotTakingRequests {
                ui.label(RichText::new(crate::net::reach::NOT_TAKING_REQUESTS).size(theme.font_size_body).color(theme.text_primary()));
            } else if refusal == crate::net::reach::Refusal::RequestSent {
                ui.label(
                    RichText::new("Contact request sent. They will see only your name, and can accept or ignore it.")
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
            } else {
                ui.label(RichText::new(crate::engine::reach::REFUSED_SENTENCE).size(theme.font_size_body).color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);
                send = widgets::Button::primary("Send request")
                    .tooltip("Send them a request that carries only your name")
                    .show(ui, theme);
            }
            if !state.reach.status.is_empty() {
                ui.label(RichText::new(&state.reach.status).size(theme.font_size_small).color(theme.warning()));
            }
        });
    if send {
        if let Err(why) = crate::engine::reach::send_contact_request(state, peer) {
            state.reach.status = why;
        }
    }
}

/// The left rail's Requests section, under DMs: only when someone has asked, so it stays quiet
/// otherwise (10a: "a quiet Requests list").
pub(super) fn draw_requests_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let count = state.dm_store.as_ref().map_or(0, |s| s.requests().len());
    if count == 0 {
        return;
    }
    Frame::NONE
        .fill(theme.dm_bg())
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(format!("Requests ({count})"))
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.label(
                RichText::new("They asked to be in contact. You see only a name.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            crate::gui::pages::safety::draw_requests_list(ui, theme, state);
        });
}
