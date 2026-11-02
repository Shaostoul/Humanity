//! The always-allow list's half that touches the app and the sockets (notifications rung 3,
//! 2026-11-01; the list and its notes are `notifications::allow`, pure). The operator, verbatim:
//! "I like the default being calls don't go through on quiet hours unless I specifically add users
//! to the always allow list."
//!
//! - [`set`]: a conversation's menu (and Settings' Remove) puts someone on the list or takes them
//!   off, at once on this device, and tells my other devices with a note to myself: one sealed DM
//!   from me to me on every server I am connected to (`block::note_put_text`, `to_my_mailboxes`,
//!   the block list's own way), or, with none connected, kept and sent on the next ([`flush`]).
//! - [`screens_dm`]: an opened DM that is such a note is never stored or shown; one from me to me
//!   is applied (the newest note per person wins, so my own echo and a note delivered again change
//!   nothing), and one from anyone else is dropped unread.
//!
//! No server ever holds the list: a note is sealed to my own key, and the server sees an ordinary
//! DM to my own mailbox.

use crate::gui::GuiState;
use crate::net::dm_pq::DmInner;
use crate::notifications::allow;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Is `key` on my list now?
pub(crate) fn allows(gs: &GuiState, key: &str) -> bool {
    gs.settings.notifications.always_allows(&gs.profile_public_key, key)
}

/// Put `key` on my always-allow list (`on`) or take them off: here at once, and on my other devices
/// through a note to myself. Never myself, and nothing before an identity is unlocked.
pub(crate) fn set(gs: &mut GuiState, key: &str, on: bool) {
    let me = gs.profile_public_key.clone();
    if me.is_empty() || key.is_empty() || key.eq_ignore_ascii_case(&me) {
        return;
    }
    let at = now_ms();
    gs.settings.notifications.quiet_allow.entry(me).or_default().apply(key, on, at);
    let text = allow::note_text(key, on);
    if !send(gs, &text, at) {
        gs.settings.notifications.quiet_allow_unsent.push((text, at));
    }
    gs.settings_dirty = true;
}

/// The note to myself on every server I am connected to; false when none is (or the identity is
/// locked), so the caller keeps it.
fn send(gs: &GuiState, text: &str, at: u64) -> bool {
    let Some((put, _id)) = crate::engine::block::note_put_text(gs, text, at) else { return false };
    crate::engine::block::to_my_mailboxes(gs, &put.to_string()) > 0
}

/// Send the notes made while no server was connected (on every member list, beside the block
/// list's). Any that still cannot go out wait for the next connection.
pub(crate) fn flush(gs: &mut GuiState) {
    if gs.settings.notifications.quiet_allow_unsent.is_empty() {
        return;
    }
    let waiting = std::mem::take(&mut gs.settings.notifications.quiet_allow_unsent);
    let left: Vec<(String, u64)> = waiting.into_iter().filter(|(text, at)| !send(gs, text, *at)).collect();
    gs.settings.notifications.quiet_allow_unsent = left;
    gs.settings_dirty = true;
}

/// An opened, signature-checked DM: true when it is a note about the list, which is never stored,
/// listed or shown. One from me to me is applied; one from anyone else is ignored.
pub(crate) fn screens_dm(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !allow::is_note(&inner.text) {
        return false;
    }
    let me = gs.profile_public_key.clone();
    if inner.from != me || inner.to != me {
        log::warn!("an always-allow note not from us to us was ignored");
        return true;
    }
    if let Some((key, on)) = allow::parse(&inner.text) {
        if gs.settings.notifications.quiet_allow.entry(me).or_default().apply(&key, on, inner.ts) {
            gs.settings_dirty = true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(from: &str, to: &str, ts: u64, text: &str) -> DmInner {
        DmInner { from: from.into(), to: to.into(), ts, text: text.into(), sig_b64: String::new(), cert: None }
    }

    /// A note from me to me (another device's change, or my own coming back) puts the person on my
    /// list or takes them off, the newest winning; a note from anyone else changes nothing; a note
    /// is never shown either way; with no server connected the change waits for the next one. Seen
    /// red 2026-11-01 with `screens_dm` applying any sender's note: "a note from someone else
    /// changes nothing" failed (Eve put herself on my list).
    #[test]
    fn notes_to_myself_carry_the_list_and_nobody_else_can_write_it() {
        let mut gs = GuiState::default();
        gs.profile_public_key = "me".into();
        assert!(screens_dm(&mut gs, &note("me", "me", 100, &allow::note_text("ab12", true))));
        assert!(allows(&gs, "ab12"));
        assert!(screens_dm(&mut gs, &note("ee55", "me", 200, &allow::note_text("ee55", true))), "never shown");
        assert!(!allows(&gs, "ee55"), "a note from someone else changes nothing");
        assert!(screens_dm(&mut gs, &note("me", "me", 50, &allow::note_text("ab12", false))));
        assert!(allows(&gs, "ab12"), "an older note does not undo a newer one");
        assert!(!screens_dm(&mut gs, &note("ab12", "me", 300, "hello")), "an ordinary DM is not a note");
        // No server connected: the change is made here and its note waits.
        set(&mut gs, "ab12", false);
        assert!(!allows(&gs, "ab12"));
        set(&mut gs, "me", true);
        assert!(!allows(&gs, "me"), "never myself");
        assert_eq!(gs.settings.notifications.quiet_allow_unsent.len(), 1, "kept for the next connection");
    }
}
