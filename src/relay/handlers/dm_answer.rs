//! The answer to a `dm_put` (spec 10l, docs/design/blocking-and-safe-mode.md, 2026-10-10).
//!
//! Why it exists: both apps used to count a friendship pass as given the moment it was written to
//! the socket (and withdraw the pass it replaced), but the relay can refuse a `dm_put` (the send
//! limiter, handlers/dm_rate.rs; a daily budget; the recipient's "who can reach me",
//! handlers/reach.rs) and never said WHICH send it refused, so a refused pass looked given while
//! the friend held only a withdrawn one.
//!
//! The protocol, exactly as the spec gives it:
//! - a `dm_put` may carry `"ref"`, the app's own id for the send, 1 to 64 characters of
//!   `[A-Za-z0-9_-]`; anything else is treated as no ref, and a put without one is handled as it
//!   always was;
//! - a put to SOMEONE ELSE that carries a ref is answered to its sender alone:
//!   `{"type":"dm_put_ok","ref"}` once it is in the mailbox, or
//!   `{"type":"dm_put_refused","ref","reason":"rate"|"reach"|"size"|"other"}` when it is not.
//!   The notices and `reach_refused` that said so before still go too;
//! - the self-copy (the sender's sent history, addressed to their own key) gets neither.
//!
//! The relay keeps nothing of a ref: it is echoed once, in the answer, and forgotten.
//! `handle_dm_put` (msg_handlers.rs) does the storing and calls in here; the two messages and
//! their routing to the sender alone are `RelayMessage::DmPutOk` and `DmPutRefused` (relay.rs).

use crate::relay::relay::{RelayMessage, RelayState};

/// Why a `dm_put` was not stored: the `reason` word of `dm_put_refused`. The app does the same
/// thing for every reason (records nothing, tries again on its next sweep); the words let it tell
/// the person why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmPutRefusal {
    /// Too much, too fast: the send limiter or one of the sender's daily budgets (a stranger's
    /// knocks, contact requests, group reports, handlers/reach.rs `pay`). The same send can land
    /// later, which is what "rate" tells the app, so a spent budget is "rate" and not "reach".
    Rate,
    /// The recipient's "who can reach me" refused the sender (handlers/reach.rs `dm_gate`).
    Reach,
    /// The envelope is over msg_handlers.rs `DM_ENVELOPE_MAX`.
    Size,
    /// Anything else: an empty field, not a sealed v2 envelope, a bot, a muted sender, the mailbox
    /// could not be written, or the owner switched chat off.
    Other,
}

impl DmPutRefusal {
    pub fn word(self) -> &'static str {
        match self {
            DmPutRefusal::Rate => "rate",
            DmPutRefusal::Reach => "reach",
            DmPutRefusal::Size => "size",
            DmPutRefusal::Other => "other",
        }
    }
}

/// The longest `ref` the relay answers to. Long enough for any id an app makes (a UUID is 36),
/// short enough that an answer stays a few dozen bytes.
const PUT_REF_MAX: usize = 64;

/// Is `r` a ref the relay answers to: 1 to 64 characters of `[A-Za-z0-9_-]`? The ref is copied
/// into the answer verbatim, so the relay only ever repeats back a plain token, never text a
/// client could shape into something else.
pub(crate) fn valid_put_ref(r: &str) -> bool {
    (1..=PUT_REF_MAX).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Serde reader for `DmPut::put_ref` (relay.rs): text becomes Some, anything else (a number, an
/// object, null) None. Without it a `ref` of the wrong type would fail the whole frame's parse
/// and the DM itself would be dropped, when the spec says a bad ref is simply no ref.
pub(crate) fn lenient_put_ref<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    use serde::Deserialize;
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    })
}

/// The ref a put to `to` from `sender` is answered under, or None when it gets no answer: no
/// ref, a ref the spec does not allow, or the self-copy (`to` is the sender). The self-copy is the
/// second half of a send whose recipient copy is answered, so answering it as well would give the
/// app two answers for one ref.
pub(crate) fn answered_ref(sender: &str, to: &str, put_ref: Option<&str>) -> Option<String> {
    put_ref.filter(|r| to != sender && valid_put_ref(r)).map(str::to_string)
}

/// Tell `sender` alone how the put named `put_ref` went.
pub(crate) fn answer_dm_put(state: &RelayState, sender: &str, put_ref: String, outcome: Result<(), DmPutRefusal>) {
    let sender = sender.to_string();
    let _ = state.broadcast_tx.send(match outcome {
        Ok(()) => RelayMessage::DmPutOk { sender, put_ref },
        Err(why) => RelayMessage::DmPutRefused { sender, put_ref, reason: why.word().to_string() },
    });
}

/// A `dm_put` the feature gate refused because the owner switched chat off (relay.rs): answer
/// its ref "other" as any refused put is, so the app stops waiting at once instead of after its
/// 30-second timeout. `raw` is the frame as sent.
pub fn refuse_gated_dm_put(state: &RelayState, sender: &str, raw: &serde_json::Value) {
    let to = raw.get("to").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(r) = answered_ref(sender, to, raw.get("ref").and_then(|v| v.as_str())) {
        answer_dm_put(state, sender, r, Err(DmPutRefusal::Other));
    }
}

#[cfg(test)]
#[path = "dm_answer_tests.rs"]
mod tests;
