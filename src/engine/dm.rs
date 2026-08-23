//! Native DM glue for the sealed-sender v2 protocol (2026-08-23).
//!
//! Envelopes arrive as opaque `{v:2, ek_ct_b64, nonce_b64, ct_b64}` JSON;
//! only our seed-derived Kyber key opens them, and the inner payload's
//! Dilithium signature proves who wrote it (the relay no longer vouches
//! for — or even knows — the sender). Verified messages land in the
//! local encrypted `DmStore`, which is the ONLY archive: the server's
//! mailbox expires.

use crate::gui::GuiState;
use crate::net::dm_pq::{self, DmInner};

/// Decrypt a v2 envelope with our own key and verify the inner Dilithium
/// signature. Err = not ours / tampered / spoofed sender — callers drop
/// the envelope (a spoof must never render with a claimed sender name).
pub(crate) fn open_verify_dm(raw_content: &str, gui_state: &GuiState) -> Result<DmInner, String> {
    let seed = gui_state
        .private_key_bytes
        .as_ref()
        .ok_or("identity locked — unlock to read DMs")?;
    let me = dm_pq::DmPqKeypair::from_bip39_seed(seed)?;
    let inner_json = dm_pq::open_v2(&me, raw_content)?;
    dm_pq::parse_verify_inner(&inner_json)
}

/// Make sure the active server's DM store is loaded. Returns false when
/// identity or server aren't established yet (nothing to key the store by).
pub(crate) fn ensure_dm_store(gui_state: &mut GuiState) -> bool {
    if gui_state.dm_store.is_some() {
        return true;
    }
    let Some(seed) = gui_state.private_key_bytes.clone() else { return false };
    if gui_state.profile_public_key.is_empty() || gui_state.server_url.is_empty() {
        return false;
    }
    let server = crate::gui::pages::chat::norm_server_url(&gui_state.server_url);
    gui_state.dm_store = Some(crate::net::dm_store::DmStore::load(
        &seed,
        &gui_state.profile_public_key.clone(),
        &server,
    ));
    true
}

/// Best display name we know for a peer key: online roster, then friends,
/// then the existing sidebar entry, then a short key prefix.
pub(crate) fn dm_display_name(gui_state: &GuiState, peer: &str) -> String {
    if let Some(u) = gui_state.chat_users.iter().find(|u| u.public_key == peer) {
        if !u.name.is_empty() && u.name != "Anonymous" {
            return u.name.clone();
        }
    }
    if let Some(f) = gui_state.chat_friends.iter().find(|f| f.public_key == peer) {
        if !f.name.is_empty() {
            return f.name.clone();
        }
    }
    if let Some(d) = gui_state.chat_dms.iter().find(|d| d.user_key == peer) {
        if !d.user_name.is_empty() {
            return d.user_name.clone();
        }
    }
    peer.chars().take(8).collect()
}

/// Rebuild the DM sidebar (`chat_dms`) from the local store. The store is
/// authoritative for conversations, previews, order, and unread dots —
/// the relay knows none of that any more.
pub(crate) fn rebuild_dm_sidebar(gui_state: &mut GuiState) {
    let Some(store) = gui_state.dm_store.as_ref() else { return };
    let summaries = store.conversations();
    let rebuilt: Vec<crate::gui::ChatDm> = summaries
        .iter()
        .map(|s| {
            let preview = if s.last_from_me {
                format!("You: {}", s.last_text)
            } else {
                s.last_text.clone()
            };
            crate::gui::ChatDm {
                user_name: dm_display_name(gui_state, &s.peer),
                user_key: s.peer.clone(),
                last_message: preview,
                timestamp: crate::gui::pages::chat::format_timestamp(s.last_ts),
                unread: s.unread,
            }
        })
        .collect();
    gui_state.chat_dms = rebuilt;
}

/// Reload the open `dm:<peer>` channel's messages from the local store
/// into the shared message list (the standard renderer draws them).
pub(crate) fn reload_dm_channel(gui_state: &mut GuiState, peer: &str) {
    let dm_channel = format!("dm:{peer}");
    gui_state.chat_messages.retain(|m| m.channel != dm_channel);
    let Some(store) = gui_state.dm_store.as_ref() else { return };
    let server = crate::gui::pages::chat::norm_server_url(&gui_state.server_url);
    let msgs: Vec<crate::gui::ChatMessage> = store
        .conversation(peer)
        .iter()
        .map(|m| crate::gui::ChatMessage {
            sender_name: if m.from == gui_state.profile_public_key {
                if gui_state.user_name.is_empty() { "You".to_string() } else { gui_state.user_name.clone() }
            } else {
                dm_display_name(gui_state, &m.from)
            },
            sender_key: m.from.clone(),
            content: m.text.clone(),
            timestamp: crate::gui::pages::chat::format_timestamp(m.ts),
            timestamp_ms: m.ts,
            channel: dm_channel.clone(),
            server: server.clone(),
            ..Default::default()
        })
        .collect();
    gui_state.chat_messages.extend(msgs);
}

/// Ingest one verified inner payload: store it, refresh the sidebar, and
/// (when its conversation is on screen) append it to the visible list.
/// Returns true when the message was new (not a duplicate).
pub(crate) fn ingest_dm(gui_state: &mut GuiState, inner: &DmInner) -> bool {
    if !ensure_dm_store(gui_state) {
        return false;
    }
    let peer = {
        let store = gui_state.dm_store.as_mut().unwrap();
        if !store.insert(inner) {
            return false; // duplicate (live echo of our own send, replay, refetch)
        }
        store.peer_of(inner)
    };
    let dm_channel = format!("dm:{peer}");
    let dm_is_open = gui_state.chat_active_channel == dm_channel;
    let is_from_me = inner.from == gui_state.profile_public_key;
    if dm_is_open {
        // Mark read immediately so the dot never flashes on an open chat.
        if let Some(store) = gui_state.dm_store.as_mut() {
            store.mark_read(&peer, inner.ts);
        }
        let server = crate::gui::pages::chat::norm_server_url(&gui_state.server_url);
        gui_state.chat_messages.push(crate::gui::ChatMessage {
            sender_name: if is_from_me {
                if gui_state.user_name.is_empty() { "You".to_string() } else { gui_state.user_name.clone() }
            } else {
                dm_display_name(gui_state, &inner.from)
            },
            sender_key: inner.from.clone(),
            content: inner.text.clone(),
            timestamp: crate::gui::pages::chat::format_timestamp(inner.ts),
            timestamp_ms: inner.ts,
            channel: dm_channel,
            server,
            ..Default::default()
        });
        while gui_state.chat_messages.len() > 200 {
            gui_state.chat_messages.remove(0);
        }
    }
    rebuild_dm_sidebar(gui_state);
    true
}
