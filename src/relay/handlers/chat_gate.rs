//! The checks a channel chat line meets before it is posted, after the mute and shared-room ban
//! checks: a new account's wait, the role's own length limit, the send limiter (chat_rate.rs) and
//! the server-wide length limit, in that order. Moved out of relay.rs's chat arm unchanged on
//! 2026-10-24, under the file-size ratchet (tests/file_size_ratchet.rs).
//!
//! Each refusal tells the sender twice: in words (a `Private` message, except the limiter's,
//! which never had words) and as `chat_refused` with the line's timestamp and a reason
//! (`new_account_wait`, `too_long` with the limit, `too_fast`), so the sender's page and app mark
//! their own copy "Not sent" instead of leaving it looking posted (gui/pages/chat/line_limit.rs,
//! web/chat/app.js `markLineNotSent`).

use std::sync::Arc;

use crate::relay::relay::{RelayMessage, RelayState};

/// v0.280.0 anti-spam: time-gate new identities from posting in public channels for the first
/// NEW_IDENTITY_GRACE_SECS seconds after their first name registration. DMs are not gated
/// (already 1-to-1 + E2EE; no broadcast amplification). Bots (`bot_*`) bypass: they auth via
/// bot_secret and the operator controls onboarding. The grace period is long enough to deter
/// scripted flooding but short enough that a real user clicking through onboarding rarely hits
/// it (a few seconds of "read the welcome message" already exceeds it).
const NEW_IDENTITY_GRACE_SECS: i64 = 60;

/// Whether `key`'s chat line `content`, sent at `timestamp` by someone whose role is `role`, is
/// refused. When it is, the sender has been told why.
pub async fn refused(state: &Arc<RelayState>, key: &str, role: &str, content: &str, timestamp: u64) -> bool {
    if let Some(wait) = new_account_wait(state, key) {
        let words = format!(
            "Welcome! New accounts wait {NEW_IDENTITY_GRACE_SECS}s before posting in public channels (anti-spam). Try again in {wait}s. DMs work right away."
        );
        tell(state, key, Some(words), timestamp, "new_account_wait", None);
        return true;
    }

    // v0.200.0: enforce per-role message length limits. Reject (don't truncate) so the user knows
    // their message wasn't fully delivered. v0.262 (R4): the limit is OWNED by the role itself (no
    // server_settings tier hop); role_def() falls back to the safe default-deny role for unknown
    // ids. Counted in characters, as the apps count (line_limit.rs `over_from`).
    let limit = state.db.role_def(role).max_chars as usize;
    let len = content.chars().count();
    if len > limit {
        let words = format!(
            "Message rejected: {} chars exceeds your role's limit of {}. \
             (Your role: {}; an admin can change it in Server Settings → Roles.)",
            len,
            limit,
            if role.is_empty() { "unverified" } else { role },
        );
        tell(state, key, Some(words), timestamp, "too_long", Some(limit));
        return true;
    }

    // Channel chat's send limiter (handlers/chat_rate.rs); bots and admins skip it.
    if !crate::relay::handlers::chat_rate::allow(state, key, role).await {
        tell(state, key, None, timestamp, "too_fast", None);
        return true;
    }

    // Enforce max message length (admins: 10000, others: 2000). Quotes (lines starting with "> ")
    // are exempt.
    let char_limit: usize = if role == "admin" { 10_000 } else { 2_000 };
    let user_text_len: usize = content.lines().filter(|l| !l.starts_with("> ")).map(|l| l.len() + 1).sum();
    if user_text_len > char_limit + 1 {
        let words = format!("Message too long ({} chars, max {}). Please shorten it.", user_text_len.saturating_sub(1), char_limit);
        tell(state, key, Some(words), timestamp, "too_long", Some(char_limit));
        return true;
    }
    false
}

/// The seconds a new identity still waits before posting in public channels, or None.
fn new_account_wait(state: &RelayState, key: &str) -> Option<i64> {
    if key.starts_with("bot_") {
        return None;
    }
    let reg_at_ms = state.db.first_registered_at_for_key(key).ok().flatten()?;
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
    let age_secs = (now_ms - reg_at_ms) / 1000;
    (age_secs >= 0 && age_secs < NEW_IDENTITY_GRACE_SECS).then(|| (NEW_IDENTITY_GRACE_SECS - age_secs).max(1))
}

/// Tells the sender their line was refused: in words when there are any, and as `chat_refused`.
fn tell(state: &RelayState, key: &str, words: Option<String>, timestamp: u64, reason: &str, limit: Option<usize>) {
    if let Some(message) = words {
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: key.to_string(), message });
    }
    let _ = state.broadcast_tx.send(RelayMessage::ChatRefused { to: key.to_string(), timestamp, reason: reason.to_string(), limit });
}
