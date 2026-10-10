//! What step F draws under a received message (docs/design/blocking-and-safe-mode.md, sections
//! 6.3 and 10g, 2026-10-10): each matching warning with its title, its explanation, what to do
//! and a "Got it" that hides it under that message; and, under a stranger's direct message that
//! holds a link, the line "<name> is not your friend. Links open only when you choose." with the
//! Open button that is then the only way the link opens (once clicked, that message's links open
//! as normal for the rest of the session). Which messages get which warnings, and
//! whether links wait, is decided in src/engine/warnings.rs; the matching rule is
//! src/net/warnings.rs. Nothing here is sent anywhere.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use crate::gui::widgets::msg_format::{FormatSpan, SpanKind};

/// What a "Got it" says when hovered.
const GOT_IT_TIP: &str = "Hide this warning under this message. Nothing is sent or reported.";

/// The row's formatting with its links made plain text, for a message whose links wait for the
/// link line's Open: a held link is not drawn as a link, so a click on it does nothing.
pub(super) fn hold_links(spans: Vec<FormatSpan>, held: bool) -> Vec<FormatSpan> {
    if !held {
        return spans;
    }
    spans.into_iter().filter(|s| !matches!(s.kind, SpanKind::Link(_))).collect()
}

/// The web address's site, for an Open button when a message holds more than one link.
fn site_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// A click under a message, applied by `apply` once the drawing is done (the drawing reads the
/// app state, which the message list holds borrowed).
pub(super) enum UnderAct {
    /// "Got it" under one warning: its dismiss key.
    GotIt(String),
    /// Open on the link line: that message's key. The link has been opened; from now on (this
    /// session) the message's links open as normal and the line is gone.
    Opened(String),
}

/// Remember a click under a message for the rest of the session (nothing is written anywhere).
pub(super) fn apply(ui_state: &mut crate::net::warnings::WarningsUi, act: UnderAct) {
    match act {
        UnderAct::GotIt(key) => ui_state.dismissed.insert(key),
        UnderAct::Opened(key) => ui_state.links_opened.insert(key),
    };
}

/// Draw the warnings for `msg` and the link line under it, indented like its code blocks.
/// `links` are the message's links (the row's link spans, before `hold_links`). Returns a click
/// to `apply`. Open opens its link at once and reports the message, so its links are let through.
pub(super) fn draw_under_message(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &GuiState,
    msg: &ChatMessage,
    links: &[String],
) -> Option<UnderAct> {
    let shown = crate::engine::warnings::shown_under(state, msg);
    let held = !links.is_empty() && crate::engine::warnings::links_held(state, msg);
    if shown.is_empty() && !held {
        return None;
    }
    let indent = theme.avatar_size + theme.avatar_gap;
    let panel_w = (ui.available_width() - indent - 8.0).max(80.0);
    let mut act = None;
    for w in shown {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                ui.set_max_width(panel_w);
                Frame::none()
                    .fill(theme.bg_card())
                    .stroke(Stroke::new(theme.border_width, theme.warning()))
                    .rounding(Rounding::same(4))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_min_width((panel_w - 16.0).max(0.0)); // one width for every card (the margins are 8 + 8)
                        ui.spacing_mut().item_spacing = Vec2::new(theme.spacing_sm, theme.spacing_xs);
                        ui.label(RichText::new(&w.title).size(theme.font_size_body).strong().color(theme.warning()));
                        ui.label(RichText::new(&w.explain).size(theme.font_size_small).color(theme.text_primary()));
                        ui.label(RichText::new(&w.advice).size(theme.font_size_small).color(theme.text_secondary()));
                        if widgets::Button::secondary("Got it").tooltip(GOT_IT_TIP).show(ui, theme) {
                            act = Some(UnderAct::GotIt(crate::engine::warnings::dismiss_key(msg, &w.id)));
                        }
                    });
            });
        });
        ui.add_space(2.0);
    }
    if held {
        let mut unique: Vec<&String> = Vec::new();
        for url in links {
            if !unique.contains(&url) {
                unique.push(url);
            }
        }
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(panel_w);
                ui.spacing_mut().item_spacing = Vec2::new(theme.spacing_sm, theme.spacing_xs);
                let line = crate::net::warnings::link_line(&msg.sender_name);
                ui.label(RichText::new(line).size(theme.font_size_small).color(theme.warning()));
                for url in &unique {
                    let label = if unique.len() == 1 { "Open".to_string() } else { format!("Open {}", site_of(url)) };
                    if widgets::Button::secondary(&label).tooltip(url.as_str()).show(ui, theme) {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url.as_str()));
                        act = Some(UnderAct::Opened(crate::engine::warnings::message_key(msg)));
                    }
                }
            });
        });
        ui.add_space(2.0);
    }
    act
}

/// The in-game chat's short form (it draws each message as one line): the titles of the warnings
/// under `msg`, or None. "Got it" lives on the Chat page, where the whole warning is.
pub(super) fn compact_line(state: &GuiState, msg: &ChatMessage) -> Option<String> {
    let titles: Vec<&str> = crate::engine::warnings::shown_under(state, msg).into_iter().map(|w| w.title.as_str()).collect();
    match titles.len() {
        0 => None,
        1 => Some(format!("Warning: {}. Open the Chat page to read it.", titles[0])),
        _ => Some(format!("Warnings: {}. Open the Chat page to read them.", titles.join(", "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A held message's links are drawn as plain text; any other message keeps them.
    /// Seen red 2026-10-10 with `hold_links` returning the spans unchanged: "held: the link span is
    /// gone" (2, not 1).
    #[test]
    fn held_links_are_drawn_as_plain_text() {
        let (_, spans) = crate::gui::widgets::msg_format::parse("see **this** at https://example.com/x now");
        assert_eq!(spans.len(), 2);
        assert_eq!(hold_links(spans.clone(), false).len(), 2, "not held: unchanged");
        let held = hold_links(spans, true);
        assert_eq!(held.len(), 1, "held: the link span is gone");
        assert!(matches!(held[0].kind, SpanKind::Bold), "and the rest of the formatting stays");
        assert_eq!(site_of("https://example.com/pay?x=1"), "example.com");
        assert_eq!(site_of("http://a.example#top"), "a.example");
    }

    /// The guard on the chat's one send path (`send_composed_content`: channel posts, replies,
    /// direct messages, group messages and the in-game chat all go through it): four words of
    /// our phrase in order stop the send before anything happens, so nothing is sent or echoed
    /// and the draft stays (false); the same text without the run goes as before (here, offline,
    /// it is echoed locally). The scratchpad is never sent, so it is not checked.
    /// Seen red 2026-10-10 twice: with the guard taken out of `send_composed_content` ("stopped: the
    /// draft stays"), and with PHRASE_RUN set to 3 ("three words go as before").
    #[test]
    fn the_chat_send_path_is_guarded() {
        let seed = [11u8; 32];
        let mut gs = GuiState::default();
        gs.private_key_bytes = Some(seed.to_vec());
        gs.profile_public_key = "me".into();
        gs.chat_active_channel = "general".into();
        let phrase = crate::net::identity::mnemonic_from_seed(&seed).unwrap();
        let words: Vec<&str> = phrase.split(' ').collect();
        let four = format!("my words are {}", words[2..6].join(" "));
        let three = format!("my words are {}", words[2..5].join(" "));

        assert!(!super::super::send_composed_content(&mut gs, &four), "stopped: the draft stays");
        assert!(gs.chat_messages.is_empty(), "nothing echoed, because nothing left");
        assert_eq!(gs.pending_notices, [crate::net::warnings::GUARD_LINE.to_string()]);
        assert!(super::super::send_composed_content(&mut gs, &three), "three words go as before");
        assert_eq!(gs.chat_messages.len(), 1);

        gs.chat_active_channel = "scratchpad".into();
        assert!(super::super::send_composed_content(&mut gs, &four), "the local scratchpad keeps it");
    }

    /// The in-game chat's short form names the warnings under a message, and nothing for a
    /// message with none.
    /// Seen red 2026-10-10 with the one-warning wording changed to the plural one.
    #[test]
    fn the_in_game_line_names_the_warnings() {
        let mut gs = GuiState::default();
        gs.profile_public_key = "me".into();
        gs.warnings.list = Some(crate::net::warnings::parse_warnings(crate::embedded_data::WARNINGS_JSON.as_bytes()).unwrap());
        let m = |text: &str| ChatMessage { sender_key: "dana".into(), content: text.into(), channel: "dm:dana".into(), ..Default::default() };
        assert_eq!(compact_line(&gs, &m("Buy me a gift card")).as_deref(), Some("Warning: Asking for money. Open the Chat page to read it."));
        assert_eq!(
            compact_line(&gs, &m("I am an admin, buy me a gift card")).as_deref(),
            Some("Warnings: Asking for money, Claiming to be staff. Open the Chat page to read them.")
        );
        assert_eq!(compact_line(&gs, &m("The tomatoes are ripe")), None);
    }
}
