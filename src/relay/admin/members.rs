//! The member moderation actions (kick, mute, unmute, ban, unban, mod, unmod) through the action
//! registry (2026-10-22). They run the very function the chat commands run
//! (handlers/broadcast.rs `handle_mod_command`), with its own checks (no acting on yourself, no
//! moderator acting on an admin, the admin-only ones), so the three ways to moderate (a typed
//! command, the profile buttons, and this) cannot drift apart.
//!
//! The admin log says what was done, never to whom: a name or a key in the log would outlive the
//! person's account erase. The banned and muted lists keep who, and go with the account.

use std::sync::Arc;

use super::{Caller, Inputs, Ran, Refusal};
use crate::relay::relay::RelayState;

/// Whether `handle_mod_command`'s answer is a refusal rather than the thing done.
pub fn is_refusal(answer: &str) -> bool {
    answer.starts_with("User '") || answer.starts_with("You ") || answer.starts_with("Only ")
}

/// The log line for `cmd` done: what, never whom.
fn log_words(cmd: &str) -> &'static str {
    match cmd {
        "/kick" => "Kicked a member.",
        "/mute" => "Muted a member.",
        "/unmute" => "Unmuted a member.",
        "/ban" => "Banned a member.",
        "/unban" => "Unbanned a member.",
        "/mod" => "Made a member a moderator.",
        "/unmod" => "Took a moderator's role away.",
        _ => "Moderated a member.",
    }
}

/// Run one moderation command for `caller` on the member named in `inputs`.
pub async fn moderate(state: Arc<RelayState>, caller: Caller, inputs: Inputs, cmd: &'static str) -> Result<Ran, Refusal> {
    let name = inputs.text("name").ok_or_else(|| Refusal::bad("Whose? Give their name."))?;
    // The chat path knows the roles admin and mod by those words; an owner acts as an admin.
    let role = match caller.role.as_str() {
        "owner" => "admin",
        "moderator" => "mod",
        other => other,
    };
    let answer = crate::relay::handlers::broadcast::handle_mod_command(&state, cmd, role, &name, &caller.key).await;
    if is_refusal(&answer) {
        let status = if answer.starts_with("User '") { axum::http::StatusCode::NOT_FOUND } else { axum::http::StatusCode::FORBIDDEN };
        // The answer names the person; the log keeps only that it did not apply.
        return Err(Refusal::with(status, answer).logged_as("A moderation that did not apply."));
    }
    Ok(Ran::said(answer).logged_as(log_words(cmd)))
}
