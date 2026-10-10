//! The sealed-sender mailbox of the server we are on, page by page and live (10n N7 and 10o O1 of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): the `dm_fetch` that goes out once per
//! connection, its `dm_batch` pages, and a live `dm_new`. Moved out of engine/frame_ws_poll.rs so
//! each rule has a test with a recording socket; parked servers do the same in
//! engine/bg_connections.rs.
//!
//! WHY (10o O1). Every device signed in as a person receives every `dm_batch` page, because they
//! share one mailbox. Before 10o this app counted any last page as "my mailbox was read", so
//! another device's fetch started this one's pass sweep before this device had read its own mail;
//! and it moved its read position (the DM store's high-water mark) on any page and on any live DM,
//! so with a backlog over one page a live DM moved the position past rows never read, and the next
//! page was asked for from there. Now only pages carrying this connection's own ref count
//! (net/mailbox_fetch.rs), the next page starts from the last id of its own previous page, and a
//! live DM moves the position only once its own fetch is done.

use crate::gui::GuiState;
use crate::net::mailbox_fetch::Page;
use serde_json::Value;

/// Send this connection's mailbox fetch from the store's read position (the channel list arm,
/// which only arrives on a signed-in socket), with a fresh ref. True when it went out; the caller
/// asks once per connection (`dm_fetch.sent`).
pub(crate) fn begin_fetch(gs: &mut GuiState) -> bool {
    crate::engine::dm::ensure_dm_store(gs);
    let after_id = gs.dm_store.as_ref().map(|s| s.high_water()).unwrap_or(0);
    let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) else { return false };
    let Some(frame) = gs.dm_fetch.begin(after_id) else { return false };
    client.send(&frame.to_string());
    true
}

/// A `dm_batch` page. One carrying another ref, or none, is another of my devices' page (or a
/// stale one) and is ignored entirely: its rows come in this connection's own fetch. One of ours:
/// each envelope decrypted, verified and taken in (engine/dm.rs `ingest_dm`), the read position
/// moved to its last id, then the next page asked for from there, or, on the last page, the
/// mailbox counted read and the pass sweep that waited for it run (10n N7).
pub(crate) fn on_batch(gs: &mut GuiState, frame: &Value) {
    let Page::Own { last_id, next, read } = gs.dm_fetch.page(frame) else { return };
    let mut ingested = 0usize;
    let mut dropped = 0usize;
    for m in frame.get("messages").and_then(Value::as_array).into_iter().flatten() {
        let raw = m.get("content").and_then(Value::as_str).unwrap_or("");
        if raw.is_empty() {
            continue;
        }
        match crate::engine::dm::open_verify_dm(raw, gs) {
            Ok(inner) => {
                if crate::engine::dm::ingest_dm(gs, &inner) {
                    ingested += 1;
                }
            }
            // Not ours, or tampered: skipped, but the read position still moves past it.
            Err(_) => dropped += 1,
        }
    }
    crate::engine::dm::ensure_dm_store(gs);
    if let Some(store) = gs.dm_store.as_mut() {
        store.set_high_water(last_id);
        store.save();
    }
    crate::engine::dm::rebuild_dm_sidebar(gs);
    // A DM conversation on screen is refreshed from the store, so fetched history appears in place.
    let active = gs.chat_active_channel.clone();
    if let Some(peer) = active.strip_prefix("dm:") {
        crate::engine::dm::reload_dm_channel(gs, peer);
    }
    if ingested > 0 || dropped > 0 {
        log::info!("DM batch: {ingested} new message(s), {dropped} undecryptable envelope(s) skipped");
    }
    if let (Some(next), Some(client)) = (next, gs.ws_client.as_ref().filter(|c| c.is_connected())) {
        client.send(&next.to_string());
    }
    if read {
        crate::engine::dm::on_mailbox_read(gs);
    }
}

/// A live `dm_new`: a sealed envelope with no sender on the wire, opened with our own key, only
/// its Dilithium-verified inner trusted, and taken in as usual. It moves the read position only
/// once this connection's own fetch is done (10o O1): before that the row may lie past rows the
/// fetch has not reached yet, and moving past it would skip them. (Its own fetch brings it again;
/// the store takes it once.) True when it is a new message from someone else in a conversation not
/// on screen, with DM notifications on: the caller plays the ding.
pub(crate) fn on_new(gs: &mut GuiState, frame: &Value) -> bool {
    let mail_id = frame.get("id").and_then(Value::as_i64).unwrap_or(0);
    let raw = frame.get("content").and_then(Value::as_str).unwrap_or("");
    if raw.is_empty() {
        return false;
    }
    let ding = match crate::engine::dm::open_verify_dm(raw, gs) {
        Ok(inner) => {
            let is_from_me = inner.from == gs.profile_public_key;
            let peer = if is_from_me { &inner.to } else { &inner.from };
            let dm_is_open = gs.chat_active_channel == format!("dm:{peer}");
            let was_new = crate::engine::dm::ingest_dm(gs, &inner);
            was_new && !is_from_me && !dm_is_open && gs.notif_dm_enabled
        }
        Err(e) => {
            // Not ours, tampered, or a spoofed sender: never shown. Once the fetch is done the
            // read position still moves past it, so a poison envelope cannot wedge every fetch.
            log::warn!("DM envelope {mail_id} dropped: {e}");
            false
        }
    };
    let moves = gs.dm_fetch.is_read();
    if let Some(store) = gs.dm_store.as_mut() {
        if moves {
            store.set_high_water(mail_id);
        }
        store.save();
    }
    ding
}

// The two-device sequence of 10o O1 (another device's pages first, a live DM during a backlog), on
// the server we are on and on a parked one, is src/engine/parity_tests.rs, a child of
// engine/put_answer_tests.rs (whose recording-socket app it shares). The ref rules alone, without
// a socket: net/mailbox_fetch.rs.
