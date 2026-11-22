//! An admin's commands that set or take a person's role or name (`/verify`, `/unverify`, `/donor`,
//! `/name-release`) and Server Settings' role picker (`set_user_role`), moved out of relay.rs on
//! 2026-11-20 so each refusal is said in words and each path has a test (the owner review's finding
//! 8: the three commands looped over `set_role` without asking whether the person was the server's
//! owner, so an admin was told "X is now verified" while the guard had quietly kept the owner's
//! row, and the owner's linked devices were demoted; `/name-release` freed the owner's name; and
//! the role picker's refusal reached only the log).
//!
//! THE RULE. None of these touches the server's owner, by their name or by any key bound to it:
//! the whole command is refused with words (handlers/owner.rs `refuse_if_owner`), and nothing is
//! changed for any of that name's keys. The storage keeps the same floor underneath
//! (storage/owner.rs `write_role_in`).

use std::sync::Arc;

use crate::relay::handlers::broadcast::{broadcast_full_user_list, broadcast_peer_list};
use crate::relay::handlers::msg_handlers::is_admin_role;
use crate::relay::handlers::owner::refuse_if_owner;
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::owner::{canonical_role, is_owner_protected, OWNER_ROLE};

/// What `/verify`, `/unverify` and `/donor` set, and the words each says when done.
fn role_of(cmd: &str) -> Option<(&'static str, fn(&str) -> String)> {
    match cmd {
        "/verify" => Some(("verified", |n| format!("✦ {n} is now verified."))),
        "/unverify" => Some(("user", |n| format!("{n} is no longer verified."))),
        "/donor" => Some(("donor", |n| format!("💎 {n} is now a donor. Thank you!"))),
        _ => None,
    }
}

/// `cmd` (`/verify`, `/unverify`, `/donor` or `/name-release`) naming `target`, from `me`: the
/// words for `me`, and whether anything changed (the peer and user lists are then sent again).
pub fn name_command_in(state: &RelayState, me: &str, cmd: &str, target: &str) -> (String, bool) {
    if !is_admin_role(&state.db.get_role(me).unwrap_or_default()) {
        let what = match cmd {
            "/name-release" => "release names",
            "/donor" => "set donor status",
            "/unverify" => "unverify users",
            _ => "verify users",
        };
        return (format!("Only admins can {what}."), false);
    }
    if target.is_empty() {
        return (format!("Usage: {cmd} <name>"), false);
    }
    let keys = match state.db.keys_for_name(target) {
        Ok(k) if !k.is_empty() => k,
        _ if cmd == "/name-release" => return (format!("Name '{target}' not found."), false),
        _ => return (format!("User '{target}' not found."), false),
    };
    // The owner, by their name or any key bound to it: refused whole, nothing changed.
    if let Some(why) = refuse_if_owner(state, &keys, cmd) {
        return (why, false);
    }
    if cmd == "/name-release" {
        return match state.db.release_name(target) {
            Ok(n) if n > 0 => (format!("Released name '{target}' ({n} key bindings removed). It can now be claimed by anyone."), true),
            Ok(_) => (format!("Name '{target}' not found."), false),
            Err(e) => {
                tracing::error!("Name release error: {e}");
                ("The name could not be released; the server's log says why.".into(), false)
            }
        };
    }
    let Some((role, done)) = role_of(cmd) else { return (String::new(), false) };
    let mut failed = 0;
    for key in &keys {
        if let Err(e) = state.db.set_role(key, role) {
            failed += 1;
            tracing::error!("{cmd} failed for a key of {target}: {e}");
        }
    }
    if failed > 0 {
        return (format!("{cmd} could not be saved for {failed} of {target}'s {} device(s); the server's log says why.", keys.len()), failed < keys.len());
    }
    (done(target), true)
}

/// [`name_command_in`], said to `me`, with the lists sent again when something changed.
pub async fn name_command(state: &Arc<RelayState>, me: &str, cmd: &str, target: &str) {
    let (words, changed) = name_command_in(state, me, cmd, target);
    let _ = state.broadcast_tx.send(RelayMessage::Private { to: me.to_string(), message: words });
    if changed {
        broadcast_peer_list(state).await;
        broadcast_full_user_list(state).await;
    }
}

/// Server Settings' role picker: `me` gives `target` the role `role_id`. The words for `me` (None
/// for an empty target), and whether it changed.
pub fn set_user_role_in(state: &RelayState, me: &str, target: &str, role_id: &str) -> (Option<String>, bool) {
    if !is_admin_role(&state.db.get_role(me).unwrap_or_default()) {
        return (Some("Only admins can assign roles.".into()), false);
    }
    if target.is_empty() {
        return (None, false);
    }
    if canonical_role(role_id) == OWNER_ROLE {
        return (Some("The owner role is never assigned: it moves only by the server's claim code or the owner's own handover.".into()), false);
    }
    if let Some(why) = refuse_if_owner(state, &[target.to_string()], "grant_role") {
        return (Some(why), false);
    }
    let rd = state.db.role_def(&canonical_role(role_id));
    match state.db.set_role(target, role_id) {
        Ok(()) => {
            let tname = state.db.name_for_key(target).ok().flatten().unwrap_or_else(|| humanity_core::text::prefix(target, 8).to_string());
            (Some(format!("✓ {tname} is now \"{}\".", rd.label)), true)
        }
        Err(e) if is_owner_protected(&e) => (Some("Nobody can change this server's owner's role. Only the owner can hand the role to someone else.".into()), false),
        Err(e) => {
            tracing::error!("set_user_role failed: {e}");
            (Some("The role could not be saved; the server's log says why.".into()), false)
        }
    }
}

/// [`set_user_role_in`], said to `me`, with everyone's user list sent again when it changed.
pub async fn set_user_role(state: &Arc<RelayState>, me: &str, target: &str, role_id: &str) {
    let (words, changed) = set_user_role_in(state, me, target, role_id);
    if let Some(message) = words {
        let _ = state.broadcast_tx.send(RelayMessage::Private { to: me.to_string(), message });
    }
    if changed {
        broadcast_full_user_list(state).await;
    }
}

#[cfg(test)]
#[path = "role_commands_tests.rs"]
mod tests;
