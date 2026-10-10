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
use crate::net::dm_store::SentPass;

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
    // The store of the server the app is on (`dial_address`), never of an address still being
    // typed into the Server field: the connection it was typed over reconnects with the draft
    // there (BUG-160's follow-up), and a store loaded under the draft would take its DMs.
    let Some(address) = gui_state.dial_address() else { return false };
    if gui_state.profile_public_key.is_empty() {
        return false;
    }
    let server = crate::gui::pages::chat::norm_server_url(address);
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
///
/// Friendship passes v2 (2026-10-09, blocking-and-safe-mode.md 10b): a pass names this server,
/// a serial and what it allows. A received one is checked against the server's did:hum before
/// it is kept; the echo of one we gave from another of our devices is read for its serial, so
/// any device can withdraw it; an unfollow from our side withdraws what we gave, and one from
/// theirs drops the pass they gave us (they withdrew it).
fn ingest_control(gui_state: &mut GuiState, inner: &DmInner) {
    let me = gui_state.profile_public_key.clone();
    let from_me = inner.from == me;
    let peer = if from_me { inner.to.clone() } else { inner.from.clone() };
    let mut want_cert_for: Option<String> = None;
    let mut withdraw_from: Option<String> = None;
    if let Some(store) = gui_state.dm_store.as_mut() {
        match inner.text.as_str() {
            crate::net::dm_pq::CTL_FOLLOW => {
                if from_me {
                    // Our own follow echoed from another device.
                    store.set_following(&peer, true);
                } else {
                    store.set_follower(&peer, true);
                    // Mutual now? Hand them our pass (once).
                    if store.is_following(&peer) && !store.cert_sent_to(&peer) {
                        want_cert_for = Some(peer.clone());
                    }
                }
            }
            crate::net::dm_pq::CTL_UNFOLLOW => {
                if from_me {
                    // Our own unfollow, from another device: the passes we gave go too
                    // (that device withdrew the ones it knew of; a repeat is harmless).
                    store.set_following(&peer, false);
                    withdraw_from = Some(peer.clone());
                } else {
                    store.set_follower(&peer, false);
                    store.forget_cert_from(&peer);
                }
            }
            crate::net::dm_pq::CTL_FRIEND_CERT => {
                if let Some(cert) = inner.cert.as_deref() {
                    if from_me {
                        // A pass we gave, echoed from another device: remember its serial.
                        match crate::relay::core::pq_crypto::parse_friend_cert(cert) {
                            Ok((pass, _)) => store.record_pass_sent(&peer, SentPass { serial: pass.serial, may: pass.may.wire() }),
                            Err(e) => log::warn!("our own friendship pass echoed unreadable ({e:?}); ignored"),
                        }
                    } else {
                        let server = store.pass_server().unwrap_or("").to_string();
                        match crate::relay::core::pq_crypto::verify_friend_cert(&server, &inner.from, &me, cert) {
                            Ok(_) => store.store_cert_from(&inner.from, cert),
                            Err(e) => log::warn!("friendship pass from {} failed its check ({e:?}); dropped", &inner.from[..12.min(inner.from.len())]),
                        }
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
    if let Some(peer) = withdraw_from {
        withdraw_passes(gui_state, &peer);
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

/// Issue + deliver MY friendship pass to `peer` (idempotent: nothing when one stands).
///
/// v2 (2026-10-09): the pass names this server's did:hum (from its `identify_challenge`), a
/// fresh random serial and what the friend may do: the defaults two new friends get, which
/// leave out calls (calls come only from people the person chooses, step B's "may call me"
/// list). Nothing is minted until the server and the peer's DM key are known; the sweep on the
/// next member list tries again.
pub(crate) fn send_friend_cert(gui_state: &mut GuiState, peer: &str) {
    use crate::relay::core::pq_crypto::{build_friend_cert, new_friend_cert_serial, FriendMay, FRIEND_PASS_DEFAULT_MAY};
    let Some(server) = gui_state.dm_store.as_ref().filter(|s| !s.cert_sent_to(peer)).and_then(|s| s.pass_server().map(str::to_string)) else {
        return;
    };
    if !gui_state.peer_kyber_keys.contains_key(peer) {
        return; // cannot seal to them yet; no point signing a pass that cannot be sent
    }
    let Some(seed) = gui_state.private_key_bytes.clone() else { return };
    let Some(serial) = new_friend_cert_serial() else { return };
    let me = gui_state.profile_public_key.clone();
    // The minting step lives with its check in relay::core::pq_crypto, so both feature sets can
    // reach it (the relay's own tests mint passes too).
    let cert = match build_friend_cert(&seed, &server, &me, peer, &serial, &FRIEND_PASS_DEFAULT_MAY) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("friendship pass not minted: {e:?}");
            return;
        }
    };
    let may = FriendMay::from_words(FRIEND_PASS_DEFAULT_MAY).map(|m| m.wire()).unwrap_or_default();
    if send_dm_control(gui_state, peer, crate::net::dm_pq::CTL_FRIEND_CERT, Some(cert)) {
        if let Some(store) = gui_state.dm_store.as_mut() {
            store.record_pass_sent(peer, SentPass { serial, may });
            store.save();
        }
    }
}

/// Take back every pass I gave `peer` (on Unfollow; later on Block and Remove friend): the
/// store moves their serials to the waiting withdrawals, and those go to the relay now if we
/// are connected, or on the next connection.
pub(crate) fn withdraw_passes(gui_state: &mut GuiState, peer: &str) {
    if let Some(store) = gui_state.dm_store.as_mut() {
        store.withdraw_passes_to(peer);
        store.save();
    }
    send_pending_withdrawals(gui_state);
}

/// Send `cert_revoke {serial}` for every withdrawal the relay has not confirmed yet. The relay
/// answers each with `cert_revoked {serial}` (`withdrawal_confirmed` below); until then they are
/// resent on every member list, so a withdrawal made offline is never lost.
pub(crate) fn send_pending_withdrawals(gui_state: &GuiState) {
    let Some(store) = gui_state.dm_store.as_ref() else { return };
    let Some(ref client) = gui_state.ws_client else { return };
    if !client.is_connected() {
        return;
    }
    for serial in store.pending_withdrawals() {
        client.send(&serde_json::json!({ "type": "cert_revoke", "serial": serial }).to_string());
    }
}

/// The relay's `cert_revoked {serial}`: that withdrawal is done.
pub(crate) fn withdrawal_confirmed(gui_state: &mut GuiState, frame: &serde_json::Value) {
    let Some(serial) = frame.get("serial").and_then(|v| v.as_str()) else { return };
    if let Some(store) = gui_state.dm_store.as_mut() {
        store.withdrawal_confirmed(serial);
        store.save();
    }
}

/// The server's did:hum, from its `identify_challenge`: the server every pass given or held here
/// names. Kept in the DM store, which is per server; a changed identity voids the old passes.
pub(crate) fn note_server_did(gui_state: &mut GuiState, did: Option<&str>) {
    let Some(did) = did.filter(|d| !d.is_empty()) else { return };
    if !ensure_dm_store(gui_state) {
        return;
    }
    if let Some(store) = gui_state.dm_store.as_mut() {
        if store.set_pass_server(did) {
            store.save();
        }
    }
}

/// Bring the passes up to date, on every member list (which arrives only on a signed-in socket,
/// with the DM keys the passes are sealed to): resend unconfirmed withdrawals, and give a pass
/// to every mutual follow that has none standing. That covers the first run after v2 (the v1
/// records are not read), a server whose identity changed, and a send that could not go out.
pub(crate) fn sweep_friend_passes(gui_state: &mut GuiState) {
    if !ensure_dm_store(gui_state) {
        return;
    }
    send_pending_withdrawals(gui_state);
    let owed = gui_state.dm_store.as_ref().map(|s| s.friends_without_pass()).unwrap_or_default();
    for peer in owed {
        send_friend_cert(gui_state, &peer);
    }
}

/// The pass `peer` gave me, to attach whenever I reach them (dm_put, a trade request, a call
/// ring, a direct-connection offer).
pub(crate) fn pass_for(gui_state: &GuiState, peer: &str) -> Option<String> {
    gui_state.dm_store.as_ref().and_then(|s| s.cert_for(peer)).map(str::to_string)
}

/// An outgoing frame from the WebRTC manager, with the target's pass attached when it is a
/// direct-connection offer (step A of 10b: the relay reads it; step B decides on it).
pub(crate) fn with_pass_on_offer(gui_state: &GuiState, frame: String) -> String {
    if !frame.contains("\"dc_offer\"") {
        return frame;
    }
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&frame) else { return frame };
    if v["type"] != "webrtc_signal" || v["signal_type"] != "dc_offer" {
        return frame;
    }
    match v["to"].as_str().and_then(|to| pass_for(gui_state, to)) {
        Some(pass) => {
            v["friend_cert"] = serde_json::Value::String(pass);
            v.to_string()
        }
        None => frame,
    }
}

/// Follow / unfollow `peer` (the UI entry point). Updates local state,
/// notifies the peer with a sealed control, and completes the friendship
/// (pass exchange) when the follow becomes mutual. Unfollowing withdraws the
/// passes we gave them (2026-10-09): an unfollow means we no longer consent
/// to the friend lane, and the relay now honours that at once.
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
    } else {
        withdraw_passes(gui_state, peer);
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

/// The native client's half of friendship passes v2 (2026-10-09), without a socket: what
/// `ingest_control` keeps, drops and withdraws, and what an outgoing offer carries. (The web
/// client's half is scripts/tests/friend-pass-web.test.js; the relay's, its own tests.)
#[cfg(test)]
mod pass_tests {
    use super::*;
    use crate::relay::core::pq_crypto::{build_friend_cert, derive_dilithium_seed, DilithiumKeypair, FRIEND_PASS_DEFAULT_MAY};

    fn identity(n: u8) -> (Vec<u8>, String) {
        let seed = vec![n; 32];
        (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
    }

    fn inner(from: &str, to: &str, text: &str, cert: Option<String>) -> DmInner {
        DmInner { from: from.into(), to: to.into(), ts: 1, text: text.into(), sig_b64: format!("{from}{to}{text}"), cert }
    }

    /// A pass from a friend is kept only when it names this server and us; their unfollow drops
    /// it; the echo of a pass we gave from our other device records its serial; our own unfollow
    /// (from another device) withdraws what we gave, waiting for the relay to confirm. A dc_offer
    /// to a friend carries their pass; anything else goes out untouched.
    /// Seen red 2026-10-09 with `store.forget_cert_from(&peer)` taken out of the unfollow arm:
    /// "their unfollow drops their pass", left `Some(..)`.
    #[test]
    fn ingest_keeps_drops_and_withdraws_passes() {
        let (my_seed, me) = identity(81);
        let (ben_seed, ben) = identity(82);
        let (_cy_seed, cy) = identity(83);
        let server = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
        let mut gs = GuiState::default();
        gs.profile_public_key = me.clone();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let mut store = crate::net::dm_store::DmStore::load(&my_seed, &me, &format!("wss://pass-test-{nanos}.example"));
        store.set_pass_server(server);
        gs.dm_store = Some(store);
        let serial = "00112233445566778899aabbccddeeff";
        let pass_from = |seed: &[u8], issuer: &str, srv: &str| build_friend_cert(seed, srv, issuer, &me, serial, &FRIEND_PASS_DEFAULT_MAY).unwrap();
        let held = |gs: &GuiState| gs.dm_store.as_ref().unwrap().cert_for(&ben).map(str::to_string);

        ingest_control(&mut gs, &inner(&ben, &me, dm_pq::CTL_FRIEND_CERT, Some(pass_from(&ben_seed, &ben, "did:hum:elsewhere"))));
        assert_eq!(held(&gs), None, "a pass given on another server is not kept");
        let good = pass_from(&ben_seed, &ben, server);
        ingest_control(&mut gs, &inner(&ben, &me, dm_pq::CTL_FRIEND_CERT, Some(good.clone())));
        assert_eq!(held(&gs), Some(good.clone()), "a good pass is kept");

        let offer = serde_json::json!({ "type": "webrtc_signal", "to": ben, "signal_type": "dc_offer", "data": "{}" }).to_string();
        let sent: serde_json::Value = serde_json::from_str(&with_pass_on_offer(&gs, offer)).unwrap();
        assert_eq!(sent["friend_cert"], good.as_str(), "a dc_offer to a friend carries their pass");
        let answer = serde_json::json!({ "type": "webrtc_signal", "to": ben, "signal_type": "dc_answer", "data": "{}" }).to_string();
        assert_eq!(with_pass_on_offer(&gs, answer.clone()), answer, "an answer goes out untouched");
        let to_cy = serde_json::json!({ "type": "webrtc_signal", "to": cy, "signal_type": "dc_offer", "data": "{}" }).to_string();
        assert_eq!(with_pass_on_offer(&gs, to_cy.clone()), to_cy, "no pass held, nothing added");

        ingest_control(&mut gs, &inner(&ben, &me, dm_pq::CTL_UNFOLLOW, None));
        assert_eq!(held(&gs), None, "their unfollow drops their pass");

        // A pass we gave Cy, echoed from our other device, then our own unfollow of Cy from there.
        let ours = build_friend_cert(&my_seed, server, &me, &cy, serial, &FRIEND_PASS_DEFAULT_MAY).unwrap();
        ingest_control(&mut gs, &inner(&me, &cy, dm_pq::CTL_FRIEND_CERT, Some(ours)));
        assert!(gs.dm_store.as_ref().unwrap().cert_sent_to(&cy), "the echo records the pass we gave");
        ingest_control(&mut gs, &inner(&me, &cy, dm_pq::CTL_UNFOLLOW, None));
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.cert_sent_to(&cy), "our unfollow takes it back");
        assert_eq!(store.pending_withdrawals(), [serial.to_string()], "and the withdrawal waits for the relay");
        withdrawal_confirmed(&mut gs, &serde_json::json!({ "type": "cert_revoked", "serial": serial }));
        assert!(gs.dm_store.as_ref().unwrap().pending_withdrawals().is_empty(), "until it answers");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }
}
