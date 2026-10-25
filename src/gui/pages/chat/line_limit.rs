//! A chat line's length against the server's limit, and whether a line I sent was posted
//! (2026-10-24): the desktop half of the website's `showOverLimit` (web/chat/chat-ui.js) and
//! `markLineSending` / `markLineSent` / `markLineNotSent` (web/chat/app.js).
//!
//! Operator, 2026-10-24: "highlight the character spots that go beyond the character limit have
//! their background highlight dark red and the text change from white to red. We need to improve
//! the visual ques for some errors." The desktop app had no limit at all: a long line went out,
//! the server refused it, and the app's own copy still looked posted.
//!
//! The limit is the one the server tells me: `my_limits` on sign-in and whenever my role or its
//! definition changes (relay handlers/broadcast.rs `send_line_limit`), or the limit a refusal
//! names. Each server's is kept under its address (`GuiState::chat_line_limits`), so a parked
//! server's limit is still right when the Commons sends through it.

use egui::{Color32, RichText, Vec2};

use crate::gui::i18n::{t, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{ChatMessage, GuiState};

/// Whether one of my own room lines was posted (`ChatMessage::send_state`): "Sending..." until
/// the server's copy of it comes back, or "Not sent" with why. Everyone else's line is Posted.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum LineSend {
    /// Posted, or not a line of mine the app follows.
    #[default]
    Posted,
    /// Sent; the server's copy of it has not come back yet.
    Sending,
    /// The server refused it: why (`too_long`, `too_fast`, `new_account_wait`) and, when too
    /// long, the limit it named.
    NotSent { reason: String, limit: Option<usize> },
}

/// How long a line may read "Sending..." before it says the server has not confirmed it: the
/// website's own wait (app.js `markLineSending`).
pub(crate) const UNCONFIRMED_AFTER_MS: u64 = 20_000;

/// Lines the server measures: a room's and a Commons room's. Not a private message, a group's or
/// the scratch pad's: those are sealed or never leave this device, and the limit is not theirs.
pub(crate) fn measured(channel: &str) -> bool {
    !(channel.starts_with("dm:") || channel.starts_with("group:") || channel.starts_with("p2pgroup:") || channel == "scratchpad")
}

/// The server a line typed in the open view goes to, by normalized address: the Commons carrier
/// when the active server does not carry the room (`super::commons_route`, the send's own route).
fn target_server(state: &GuiState) -> String {
    if let Some(room) = super::commons_room_of(&state.chat_active_channel) {
        if let (_, Some(ci)) = super::commons_route(state, room) {
            return state.connections[ci].url.clone();
        }
    }
    super::norm_server_url(&state.server_url)
}

/// The longest line I may post in the open view, or None where no limit applies. Before a server
/// has said (an older relay), the same guess the website makes: 10,000 for an admin, else 2,000.
pub(crate) fn limit_here(state: &GuiState) -> Option<usize> {
    if !measured(&state.chat_active_channel) {
        return None;
    }
    if let Some(n) = state.chat_line_limits.get(&target_server(state)) {
        return Some(*n);
    }
    let admin = state.chat_users.iter().any(|u| u.public_key == state.profile_public_key && u.role == "admin");
    Some(if admin { 10_000 } else { 2_000 })
}

/// Now, in milliseconds since 1970: what a line's age is measured against.
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The counter beside the box, as the website shows it (`updateCharCounter`): only within 200
/// characters of the limit, then "312 / 280", in the warning colour within 100 and the danger
/// colour past it (widgets/over_limit.rs `count`). The red characters themselves are that
/// widget's `layouter`.
pub(crate) fn counter(len: usize, limit: usize, theme: &Theme) -> Option<(String, Color32)> {
    (len + 200 > limit).then(|| crate::gui::widgets::over_limit::count(len, limit, 100, theme))
}

/// A server told me my limit (`my_limits`) or named it in a refusal: kept under its address.
fn learn(state: &mut GuiState, server: &str, limit: Option<u64>) {
    if let Some(n) = limit.filter(|n| *n > 0) {
        state.chat_line_limits.insert(server.to_string(), n as usize);
    }
}

/// A `my_limits` frame from `server` (the normalized address of the socket it came on).
pub(crate) fn on_my_limits(state: &mut GuiState, server: &str, frame: &serde_json::Value) {
    learn(state, server, frame.get("max_chars").and_then(|v| v.as_u64()));
}

/// A `chat_refused` frame from `server`: my line with that timestamp is marked not sent, with
/// why, and a limit it names is learned. `messages` is the list that holds my copy: the active
/// server's, or a parked carrier's (`ServerConnection::messages`).
pub(crate) fn on_refused(state: &mut GuiState, server: &str, frame: &serde_json::Value, parked: Option<usize>) {
    let reason = frame.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let limit = frame.get("limit").and_then(|v| v.as_u64());
    // The play journal's net kind: which server refused a line and why, never the line's words.
    crate::engine::play_journal::record("net", &format!("A chat line was refused: {reason}"), serde_json::json!({ "server": server, "reason": reason, "limit": limit }));
    if reason == "too_long" {
        learn(state, server, limit);
    }
    let Some(ts) = frame.get("timestamp").and_then(|v| v.as_u64()) else { return };
    let me = state.profile_public_key.clone();
    let messages = match parked {
        Some(ci) => &mut state.connections[ci].messages,
        None => &mut state.chat_messages,
    };
    if let Some(m) = messages.iter_mut().find(|m| m.sender_key == me && m.timestamp_ms == ts) {
        m.send_state = LineSend::NotSent { reason, limit: limit.map(|n| n as usize) };
    }
}

/// The server's copy of my line came back: it was posted.
pub(crate) fn confirm(messages: &mut [ChatMessage], me: &str, ts: u64) {
    if let Some(m) = messages.iter_mut().find(|m| m.sender_key == me && m.timestamp_ms == ts) {
        m.send_state = LineSend::Posted;
    }
}

/// What the line under one of my unposted messages says, at `now_ms`; None once it is posted.
pub(crate) fn status_words(msg: &ChatMessage, now_ms: u64) -> Option<String> {
    match &msg.send_state {
        LineSend::Posted => None,
        LineSend::Sending if now_ms.saturating_sub(msg.timestamp_ms) >= UNCONFIRMED_AFTER_MS => Some(t("chat.line_unconfirmed").to_string()),
        LineSend::Sending => Some(t("chat.line_sending").to_string()),
        LineSend::NotSent { reason, limit } => Some(match reason.as_str() {
            "too_long" => {
                let n: &dyn std::fmt::Display = match limit {
                    Some(n) => n,
                    None => &"?",
                };
                tf("chat.line_not_sent_too_long", &[("limit", n)])
            }
            "too_fast" => t("chat.line_not_sent_too_fast").to_string(),
            "new_account_wait" => t("chat.line_not_sent_new_account").to_string(),
            _ => t("chat.line_not_sent").to_string(),
        }),
    }
}

/// The line under one of my unposted messages: "Sending...", then "Not confirmed" after a while,
/// or "Not sent" with why and an "Edit and send again" button. Returns the message's words when
/// that button is pressed, for the caller to put back in the box.
pub(crate) fn draw_status(ui: &mut egui::Ui, theme: &Theme, msg: &ChatMessage) -> Option<String> {
    let words = status_words(msg, now_ms())?;
    let not_sent = matches!(msg.send_state, LineSend::NotSent { .. });
    if matches!(msg.send_state, LineSend::Sending) {
        // So "Sending..." turns into "Not confirmed" on time with nothing else redrawing.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
    }
    let indent = theme.avatar_size + theme.avatar_gap;
    let mut again = None;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.spacing_mut().item_spacing = Vec2::new(theme.spacing_sm, theme.spacing_xs);
        let color = if not_sent { theme.danger() } else { theme.text_muted() };
        ui.label(RichText::new(words).size(theme.font_size_small).color(color).italics());
        if not_sent && widgets::Button::secondary(t("chat.line_edit_again")).tooltip(t("chat.line_edit_again_hint")).show(ui, theme) {
            again = Some(msg.content.clone());
        }
    });
    again
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room_state(server: &str) -> GuiState {
        let mut gs = GuiState::default();
        gs.server_url = server.to_string();
        gs.chat_active_channel = "general".to_string();
        gs.profile_public_key = "me".to_string();
        gs
    }

    #[test]
    fn the_limit_is_the_one_the_server_told_and_only_where_it_applies() {
        let mut gs = room_state("wss://public.guide/ws");
        let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
        assert_eq!(limit_here(&gs), Some(2_000), "before the server says, the website's guess");
        on_my_limits(&mut gs, &server, &serde_json::json!({ "type": "my_limits", "max_chars": 280 }));
        assert_eq!(limit_here(&gs), Some(280));
        for private in ["dm:abc", "group:g1", "p2pgroup:p1", "scratchpad"] {
            gs.chat_active_channel = private.to_string();
            assert_eq!(limit_here(&gs), None, "{private} is not measured");
        }
    }

    #[test]
    fn a_refused_line_says_why_and_a_returned_one_is_posted() {
        let mut gs = room_state("wss://public.guide/ws");
        let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
        for ts in [1_000, 2_000] {
            gs.chat_messages.push(ChatMessage { sender_key: "me".into(), content: format!("line {ts}"), timestamp_ms: ts, send_state: LineSend::Sending, ..Default::default() });
        }
        on_refused(&mut gs, &server, &serde_json::json!({ "type": "chat_refused", "timestamp": 1_000, "reason": "too_long", "limit": 280 }), None);
        assert_eq!(gs.chat_messages[0].send_state, LineSend::NotSent { reason: "too_long".into(), limit: Some(280) });
        assert_eq!(gs.chat_line_limits.get(&server), Some(&280), "the limit a refusal names is learned");
        assert_eq!(status_words(&gs.chat_messages[0], 1_500).as_deref(), Some("Not sent: longer than your limit of 280 characters."));
        // The other line came back from the server.
        confirm(&mut gs.chat_messages, "me", 2_000);
        assert_eq!(gs.chat_messages[1].send_state, LineSend::Posted);
        assert_eq!(status_words(&gs.chat_messages[1], 2_500), None);
    }

    #[test]
    fn a_line_the_server_never_answers_says_so_after_a_while() {
        let m = ChatMessage { sender_key: "me".into(), timestamp_ms: 10_000, send_state: LineSend::Sending, ..Default::default() };
        assert_eq!(status_words(&m, 10_500).as_deref(), Some("Sending..."));
        assert_eq!(status_words(&m, 10_000 + UNCONFIRMED_AFTER_MS).as_deref(), Some("Not confirmed by the server yet."));
    }

    #[test]
    fn the_counter_shows_near_the_limit_and_turns_red_past_it() {
        let theme = crate::gui::theme::load_theme();
        assert_eq!(counter(10, 280, &theme), None, "far from the limit");
        assert_eq!(counter(100, 280, &theme).map(|c| c.1), Some(theme.text_muted()));
        assert_eq!(counter(200, 280, &theme).map(|c| c.1), Some(theme.warning()));
        assert_eq!(counter(312, 280, &theme), Some(("312 / 280".to_string(), theme.danger())));
    }
}
