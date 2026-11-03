//! Tasks of my own: the desktop's half that touches the app and the sockets (docs/design/tasks.md
//! rung 1, 2026-11-02, fixing BUG-217). The records, the merge, the notes and the encrypted file
//! are gui/mine_tasks.rs; the page is gui/pages/tasks.rs.
//!
//! - [`ensure`]: the tasks of the identity unlocked now are loaded (and loaded again when another
//!   identity is unlocked). Nothing is loaded while no identity is unlocked: the file is
//!   encrypted with a key from the seed.
//! - [`add`], [`change`], [`delete`]: done here at once and saved, then sent to my other devices
//!   as a note to myself: one sealed DM from me to me in my own mailbox on every server I am
//!   connected to (`block::note_put_text`, `block::to_my_mailboxes`, the block list's own way).
//!   With none connected the ids wait in the file and go out on the next connection ([`flush`],
//!   on every member list beside the block list's notes).
//! - [`screens_dm`]: THE arrival rule, on the server I am on (engine/dm.rs `ingest_dm`) and on a
//!   parked one (engine/bg_connections.rs `bg_screen`): a DM whose text starts with the marker is
//!   never stored or shown as a message. One from me to me is merged in; one from anyone else is
//!   dropped unread (anyone can seal mail to my key, only my seed signs as me).
//!
//! The server's board is a separate list (`GuiState::board_tasks`); its `task_list` never touches
//! these (pages/tasks.rs `take_board_list`).

use crate::gui::{GuiState, TaskPriority, TaskStatus};
use crate::net::dm_pq::DmInner;
use crate::gui::mine_tasks::{self, MineTasks, Record};

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Make sure the tasks of the identity unlocked now are loaded. False while no identity is
/// unlocked (there is no key to open them with).
pub(crate) fn ensure(gs: &mut GuiState) -> bool {
    if gs.profile_public_key.is_empty() {
        return false;
    }
    if gs.mine_tasks.as_ref().is_some_and(|m| m.identity() == gs.profile_public_key) {
        return true;
    }
    let Some(seed) = gs.private_key_bytes.as_ref() else { return false };
    // Test builds never read or write the person's real file (a test sets its own list).
    #[cfg(not(test))]
    let list = MineTasks::load(seed, &gs.profile_public_key);
    #[cfg(test)]
    let list = {
        let _ = seed;
        MineTasks::in_memory(&gs.profile_public_key)
    };
    gs.mine_tasks = Some(list);
    true
}

/// A new task of my own, made on this device: kept at once and sent to my other devices. Returns
/// its id, or None while no identity is unlocked.
pub(crate) fn add(gs: &mut GuiState, title: &str, description: &str, priority: TaskPriority, labels: Vec<String>, chain: Option<String>) -> Option<String> {
    if !ensure(gs) {
        return None;
    }
    let id = gs.mine_tasks.as_mut()?.add(title, description, priority, labels, chain, now_ms());
    send_or_keep(gs, vec![id.clone()]);
    Some(id)
}

/// Move one of my tasks to another column and/or give it another priority.
pub(crate) fn change(gs: &mut GuiState, id: &str, status: Option<TaskStatus>, priority: Option<TaskPriority>) {
    if !ensure(gs) {
        return;
    }
    let changed = gs.mine_tasks.as_mut().is_some_and(|m| m.change(id, status, priority, now_ms()));
    if changed {
        send_or_keep(gs, vec![id.to_string()]);
    }
}

/// Delete one of my tasks, here and on my other devices.
pub(crate) fn delete(gs: &mut GuiState, id: &str) {
    if !ensure(gs) {
        return;
    }
    let deleted = gs.mine_tasks.as_mut().is_some_and(|m| m.delete(id, now_ms()));
    if deleted {
        send_or_keep(gs, vec![id.to_string()]);
    }
}

/// Keep the change (saved, its ids marked unsent, so a crash before the send loses nothing),
/// then send the latest versions of `ids` to my other devices and clear the mark once they went;
/// with no server connected they wait for the next connection.
fn send_or_keep(gs: &mut GuiState, ids: Vec<String>) {
    let Some(m) = gs.mine_tasks.as_mut() else { return };
    m.mark_unsent(&ids);
    m.save();
    let records: Vec<Record> = ids.iter().filter_map(|id| m.record(id)).collect();
    if send(gs, &records) {
        if let Some(m) = gs.mine_tasks.as_mut() {
            m.sent(&ids);
            m.save();
        }
    }
}

/// The notes carrying `records`, to my own mailbox on every server I am connected to. False when
/// none is connected or the identity cannot seal (nothing, or not all of it, went), so the caller
/// keeps them; sending one again later is harmless (the merge).
fn send(gs: &GuiState, records: &[Record]) -> bool {
    if records.is_empty() {
        return true;
    }
    let at = now_ms();
    for text in mine_tasks::note_texts(records) {
        let Some((put, _note_id)) = crate::engine::block::note_put_text(gs, &text, at) else { return false };
        if crate::engine::block::to_my_mailboxes(gs, &put.to_string()) == 0 {
            return false;
        }
    }
    true
}

/// Send what was made while no server was connected (on every member list, beside the block
/// list's notes). What still cannot go out waits for the next connection.
pub(crate) fn flush(gs: &mut GuiState) {
    if !gs.mine_tasks.as_ref().is_some_and(|m| !m.unsent().is_empty()) {
        return;
    }
    let ids = gs.mine_tasks.as_mut().map(|m| m.take_unsent()).unwrap_or_default();
    send_or_keep(gs, ids);
}

/// An opened, signature-checked DM: true when it is a task note, which is never stored, listed
/// or shown. One from me to me is merged into my tasks; one from anyone else is dropped unread.
pub(crate) fn screens_dm(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !mine_tasks::is_note(&inner.text) {
        return false;
    }
    let me = gs.profile_public_key.clone();
    if me.is_empty() || inner.from != me || inner.to != me {
        log::warn!("a task note not from us to us was dropped unread");
        return true;
    }
    if !ensure(gs) {
        return true;
    }
    let records = mine_tasks::parse_note(&inner.text);
    if let Some(m) = gs.mine_tasks.as_mut() {
        if m.apply(records) {
            m.save();
        }
    }
    true
}

#[cfg(test)]
#[path = "mine_tasks_tests.rs"]
mod tests;
