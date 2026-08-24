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

/// Collapse an encrypted-attachment marker to a friendly sidebar preview
/// (2026-08-24), so a file DM never shows raw base64 in the conversation list.
pub(crate) fn dm_preview_text(text: &str) -> String {
    if let Some(att) = crate::net::dm_pq::parse_file_marker(text) {
        if att.mime.starts_with("image/") {
            "Photo".to_string()
        } else {
            att.name
        }
    } else {
        text.to_string()
    }
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
            let text = dm_preview_text(&s.last_text);
            let preview = if s.last_from_me {
                format!("You: {}", text)
            } else {
                text
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
///
/// Control messages (follows removal, 2026-08-24) are ACTED ON, never
/// rendered: follow/unfollow notices update the local social sets, and
/// friend-cert deliveries store the credential. Self-copies of our own
/// controls sync our social state across devices for free.
pub(crate) fn ingest_dm(gui_state: &mut GuiState, inner: &DmInner) -> bool {
    if !ensure_dm_store(gui_state) {
        return false;
    }
    if matches!(
        inner.text.as_str(),
        crate::net::dm_pq::CTL_FOLLOW | crate::net::dm_pq::CTL_UNFOLLOW | crate::net::dm_pq::CTL_FRIEND_CERT
    ) {
        ingest_control(gui_state, inner);
        return false; // acted on; nothing to render
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

// ── Client-side social graph (follows removal, 2026-08-24) ─────────────────
// The server stores no follow edges. Follow/unfollow are sealed control
// messages; friendship is a client-held certificate; multi-device sync
// rides the self-copies every control send already deposits.

/// Act on a verified control message (never rendered).
fn ingest_control(gui_state: &mut GuiState, inner: &DmInner) {
    let me = gui_state.profile_public_key.clone();
    let from_me = inner.from == me;
    let peer = if from_me { inner.to.clone() } else { inner.from.clone() };
    let mut want_cert_for: Option<String> = None;
    if let Some(store) = gui_state.dm_store.as_mut() {
        match inner.text.as_str() {
            crate::net::dm_pq::CTL_FOLLOW => {
                if from_me {
                    // Our own follow echoed from another device.
                    store.set_following(&peer, true);
                } else {
                    store.set_follower(&peer, true);
                    // Mutual now? Hand them our certificate (once).
                    if store.is_following(&peer) && !store.cert_sent_to(&peer) {
                        want_cert_for = Some(peer.clone());
                    }
                }
            }
            crate::net::dm_pq::CTL_UNFOLLOW => {
                if from_me {
                    store.set_following(&peer, false);
                } else {
                    store.set_follower(&peer, false);
                }
            }
            crate::net::dm_pq::CTL_FRIEND_CERT => {
                if let Some(cert) = inner.cert.as_deref() {
                    if from_me {
                        // Our own issued cert echoed from another device.
                        store.mark_cert_sent(&peer);
                    } else if crate::relay::core::pq_crypto::verify_friend_cert(&inner.from, &me, cert) {
                        store.store_cert_from(&inner.from, cert);
                    } else {
                        log::warn!("friend-cert from {} failed verification; dropped", &inner.from[..12.min(inner.from.len())]);
                    }
                }
            }
            _ => {}
        }
        store.save();
    }
    if let Some(peer) = want_cert_for {
        send_friend_cert(gui_state, &peer);
    }
    refresh_social_mirrors(gui_state);
}

/// Seal + send one control message to `peer` (recipient copy + self copy,
/// exactly like a chat DM so other devices stay in sync). Returns false
/// when we can't seal yet (no kyber key for the peer).
pub(crate) fn send_dm_control(gui_state: &mut GuiState, peer: &str, text: &str, cert: Option<String>) -> bool {
    let Some(seed) = gui_state.private_key_bytes.clone() else { return false };
    let me = gui_state.profile_public_key.clone();
    let Some(peer_kyber) = gui_state.peer_kyber_keys.get(peer).cloned() else {
        log::warn!("control '{text}' to {}… not sent: no kyber key yet (they must come online once)", &peer[..12.min(peer.len())]);
        return false;
    };
    let Ok(my_kp) = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&seed) else { return false };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let Ok(inner_json) = crate::net::dm_pq::build_signed_inner_ext(&seed, &me, peer, ts, text, cert.as_deref()) else {
        return false;
    };
    let (Ok(env_peer), Ok(env_self)) = (
        crate::net::dm_pq::seal_v2(&peer_kyber, &inner_json),
        crate::net::dm_pq::seal_v2(&my_kp.public_base64(), &inner_json),
    ) else {
        return false;
    };
    // Attach THEIR cert (if we hold one) so the control rides the friend
    // lane instead of spending knock budget.
    let their_cert = gui_state
        .dm_store
        .as_ref()
        .and_then(|s| s.cert_for(peer).map(|c| c.to_string()));
    let Some(ref client) = gui_state.ws_client else { return false };
    if !client.is_connected() {
        return false;
    }
    let mut put_peer = serde_json::json!({ "type": "dm_put", "to": peer, "content": env_peer });
    if let Some(c) = their_cert {
        put_peer["friend_cert"] = serde_json::Value::String(c);
    }
    client.send(&put_peer.to_string());
    client.send(&serde_json::json!({ "type": "dm_put", "to": me, "content": env_self }).to_string());
    true
}

/// Issue + deliver MY friendship certificate to `peer` (idempotent).
pub(crate) fn send_friend_cert(gui_state: &mut GuiState, peer: &str) {
    let already = gui_state.dm_store.as_ref().map(|s| s.cert_sent_to(peer)).unwrap_or(false);
    if already {
        return;
    }
    let Some(seed) = gui_state.private_key_bytes.clone() else { return };
    let me = gui_state.profile_public_key.clone();
    let cert = crate::net::dm_pq::build_friend_cert(&seed, &me, peer);
    if send_dm_control(gui_state, peer, crate::net::dm_pq::CTL_FRIEND_CERT, Some(cert)) {
        if let Some(store) = gui_state.dm_store.as_mut() {
            store.mark_cert_sent(peer);
            store.save();
        }
    }
}

/// Follow / unfollow `peer` (the UI entry point). Updates local state,
/// notifies the peer with a sealed control, and completes the friendship
/// (certificate exchange) when the follow becomes mutual.
pub(crate) fn set_follow(gui_state: &mut GuiState, peer: &str, on: bool) {
    if !ensure_dm_store(gui_state) {
        return;
    }
    if let Some(store) = gui_state.dm_store.as_mut() {
        store.set_following(peer, on);
        store.save();
    }
    let text = if on { crate::net::dm_pq::CTL_FOLLOW } else { crate::net::dm_pq::CTL_UNFOLLOW };
    let _ = send_dm_control(gui_state, peer, text, None);
    if on {
        let mutual = gui_state.dm_store.as_ref().map(|s| s.is_follower(peer)).unwrap_or(false);
        if mutual {
            send_friend_cert(gui_state, peer);
        }
    }
    refresh_social_mirrors(gui_state);
}

/// Rebuild the legacy GuiState social mirrors (the UI reads these) from
/// the local store. Keeps every existing indicator/badge working without
/// touching its draw code.
pub(crate) fn refresh_social_mirrors(gui_state: &mut GuiState) {
    let Some(store) = gui_state.dm_store.as_ref() else { return };
    gui_state.chat_following_keys = store.following().iter().cloned().collect();
    gui_state.chat_followers = store.followers().iter().cloned().collect();
    let friends: Vec<String> = store
        .following()
        .iter()
        .filter(|k| store.is_follower(k))
        .cloned()
        .collect();
    gui_state.chat_friends = gui_state
        .chat_users
        .iter()
        .filter(|u| friends.contains(&u.public_key))
        .cloned()
        .collect();
}
