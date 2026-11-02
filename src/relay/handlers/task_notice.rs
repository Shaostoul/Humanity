//! A task given to you, or a task of yours changed (notifications rung 3, 2026-11-01;
//! docs/design/notifications.md, section 11).
//!
//! Every path that changes a board task ends here with the task as it was and as it is: the
//! chat's `task_create`, `task_update` and `task_move` (raw and enum), and the bot API's `POST` and
//! `PATCH /api/tasks`. [`change`] (pure) says what happened to the task's assignee: it was newly
//! given to someone, or it kept its assignee and its title, description, status or priority
//! changed. [`after_change`] sends that person a `task_notice` on each of their own sockets, and a
//! Web Push when they have none open and their `tasks` switch is on.
//!
//! - **The assignee is a name**, the board's own field (a person types it on the Tasks page), so
//!   the recipients are the keys registered under that name here ([`recipients`]: the registry
//!   the mention push uses), never the person who made the change.
//! - **Not signed:** the board is this server's own record, and every client already takes the
//!   task list from the same socket unsigned; a signature would prove only that the server sent
//!   it, which the socket already does.
//! - **No table:** the notice is worked out from the `project_tasks` row before and after the
//!   change (CLAUDE.md, "The removed tables stay removed").
//! - **The push says neither the task nor who changed it:** the push services (Google, Mozilla,
//!   Apple) carry it.

use std::sync::Arc;

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::{PushKind, Storage, TaskRecord};

/// What happened to a task, for the person it is given to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// It was given to them: its assignee is newly their name.
    Assigned,
    /// It is theirs and its title, description, status or priority changed.
    Changed,
}

impl Change {
    /// The id the wire and the clients use.
    pub fn id(self) -> &'static str {
        match self {
            Change::Assigned => "assigned",
            Change::Changed => "changed",
        }
    }
}

/// An assignee as the board compares it: trimmed, any case; empty is nobody.
fn who(assignee: Option<&str>) -> Option<String> {
    assignee.map(str::trim).filter(|s| !s.is_empty()).map(str::to_lowercase)
}

/// What the change from `before` (`None`: the task is new) to `after` means for its assignee:
/// the assignee they now have, and what happened. `None` when nobody is to be told: the task has
/// no assignee, or nothing a notice is about changed (a move between projects, the labels).
pub fn change(before: Option<&TaskRecord>, after: &TaskRecord) -> Option<(String, Change)> {
    let now = who(after.assignee.as_deref())?;
    let assignee = after.assignee.as_deref().unwrap_or_default().trim().to_string();
    let Some(b) = before else { return Some((assignee, Change::Assigned)) };
    if who(b.assignee.as_deref()).as_deref() != Some(now.as_str()) {
        return Some((assignee, Change::Assigned));
    }
    let changed = b.title != after.title || b.description != after.description || b.status != after.status || b.priority != after.priority;
    changed.then_some((assignee, Change::Changed))
}

/// Who is told: the keys registered under `assignee` on this server, never `actor` (the person
/// who made the change, from this device or another of theirs).
pub fn recipients(db: &Storage, assignee: &str, actor: &str) -> Vec<String> {
    let mut keys = db.keys_for_name(assignee).unwrap_or_default();
    keys.retain(|k| k != actor);
    keys.sort();
    keys.dedup();
    keys
}

/// The push's words: neither the task's title nor who changed it.
pub fn push_words(c: Change) -> (&'static str, &'static str) {
    match c {
        Change::Assigned => ("A task was given to you", "Open HumanityOS to see it."),
        Change::Changed => ("A task of yours changed", "Open HumanityOS to see it."),
    }
}

/// Tell the task's assignee what `actor_key` (named `actor_name`) just did to it: a `task_notice`
/// to each of their sockets, and a Web Push (with their `tasks` switch on) when none is open.
pub async fn after_change(state: &Arc<RelayState>, actor_key: &str, actor_name: &str, before: Option<&TaskRecord>, after: &TaskRecord) {
    let Some((assignee, what)) = change(before, after) else { return };
    let to = recipients(&state.db, &assignee, actor_key);
    if to.is_empty() {
        return;
    }
    let online: Vec<bool> = {
        let peers = state.peers.read().await;
        to.iter().map(|k| peers.contains_key(k)).collect()
    };
    for (key, online) in to.into_iter().zip(online) {
        let _ = state.broadcast_tx.send(RelayMessage::TaskNotice {
            target: Some(key.clone()),
            task_id: after.id,
            title: after.title.clone(),
            change: what.id().to_string(),
            status: after.status.clone(),
            by_name: actor_name.to_string(),
            by_key: actor_key.to_string(),
        });
        if !online && state.db.wants_push(&key, PushKind::Task) {
            let (title, body) = push_words(what);
            state.send_push_notification(&key, title, body, &format!("task-{}", after.id), "/tasks");
        }
    }
}

/// The display name `key` is signed in with here, else the start of the key (the board's habit).
pub async fn name_of(state: &Arc<RelayState>, key: &str) -> String {
    let peers = state.peers.read().await;
    peers.get(key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| key.chars().take(8).collect())
}

#[cfg(test)]
#[path = "task_notice_tests.rs"]
mod tests;
