//! Quote / reply in the chat (the 2026-10-10 batch review of
//! docs/design/blocking-and-safe-mode.md): what a reply carries, and where it may go.
//!
//! THE LEAK THIS CLOSES. Reply is offered on every row, direct messages and groups included. The
//! reply target (`GuiState::chat_reply_to`) used to survive a change of conversation, and a send
//! in a PUBLIC channel attached `reply_to.content`: the first 80 bytes of the message replied to.
//! So replying to a private message, moving to a public channel and sending put the start of the
//! private text into a public room, readable by everyone on the server and kept in its history.
//! A file's `[[hum:file:` marker carries the file's key, so its first 80 bytes could carry part
//! of that key too.
//!
//! THE RULES, each held by a test in reply_tests.rs:
//! - a reply goes only with a send in the conversation it was started in (`made_in`), and the
//!   target is dropped as soon as another conversation is open (`drop_if_elsewhere`);
//! - a reply to a message in a DM or a group never goes onto the wire at all (`wire_reply`):
//!   inside a DM or group it is shown only on this device's own copy;
//! - a preview never holds a file marker or any part of it (`preview_of`), only "Photo" or the
//!   file's name, as the DM list's preview does;
//! - every preview is cut on a character boundary (`cut`): the old byte slicing panicked on a
//!   message whose 80th byte fell inside a multi-byte character (an accented letter, any CJK
//!   text, an emoji), which closed the app.
//!
//! Takes `use super::*` like the page's other children.

use super::*;

/// How many characters of a message a reply keeps, and how many a quoted line shows.
pub(crate) const PREVIEW_CHARS: usize = 80;
pub(crate) const QUOTE_CHARS: usize = 60;

/// The first `max` characters of `s`, with "…" when there was more. Cut between characters, never
/// inside one.
pub(crate) fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_string(),
    }
}

/// What a reply keeps of `content`: never a file marker or any part of it (its key opens the
/// file), but "Photo" or the file's name, as the DM list's preview says; any other text, cut to
/// `PREVIEW_CHARS` characters. Anything after a marker that does not read is left out whole.
pub(crate) fn preview_of(content: &str) -> String {
    if crate::net::dm_pq::parse_file_marker(content).is_some() {
        return crate::engine::dm::dm_preview_text(content);
    }
    // Any version of the marker anywhere (the relay refuses reports holding one for the same
    // reason, `DM_FILE_MARKER_PREFIX`): only what comes before it is kept.
    match content.find("[[hum:file:") {
        Some(at) if content[..at].trim().is_empty() => "File".to_string(),
        Some(at) => cut(content[..at].trim_end(), PREVIEW_CHARS),
        None => cut(content, PREVIEW_CHARS),
    }
}

/// The reply target for `msg`, started while `conversation` is open (`chat_active_channel`).
pub(crate) fn context_for(msg: &ChatMessage, conversation: &str, timestamp_ms: u64) -> crate::gui::ReplyContext {
    crate::gui::ReplyContext {
        sender_key: msg.sender_key.clone(),
        sender_name: msg.sender_name.clone(),
        preview: preview_of(&msg.content),
        timestamp_ms,
        conversation: conversation.to_string(),
        private: is_private_channel(&msg.channel) || is_private_channel(conversation),
    }
}

/// The reply target, when it was started in `channel` (the conversation a send goes to).
pub(crate) fn made_in(state: &GuiState, channel: &str) -> Option<crate::gui::ReplyContext> {
    state.chat_reply_to.clone().filter(|r| r.conversation == channel)
}

/// The reply a send to `channel` may put on the wire: one started in that same conversation, and
/// never one to a private message. (The DM and group paths do not carry a reply anyway; this is
/// what keeps a public channel's `reply_to` from ever holding private words.)
pub(crate) fn wire_reply(state: &GuiState, channel: &str) -> Option<crate::gui::ReplyContext> {
    made_in(state, channel).filter(|r| !r.private && !is_private_channel(channel))
}

/// Drop the reply target once another conversation is open, however it was opened (a row of the
/// left rail, the in-world panel, a notice, a link): a reply belongs to where it was started.
pub(crate) fn drop_if_elsewhere(state: &mut GuiState) {
    if state.chat_reply_to.as_ref().is_some_and(|r| r.conversation != state.chat_active_channel) {
        state.chat_reply_to = None;
    }
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod tests;
