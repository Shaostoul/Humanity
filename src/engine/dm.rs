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
use crate::net::put_answers::{Held, PendingPut};

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
    // The block list is the identity's, not the server's (step C), but it is needed wherever DMs
    // are, so it is loaded here too.
    crate::engine::block::ensure_block_list(gui_state);
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
        // A conversation with someone we blocked leaves the list (step C); it is kept in the
        // store, so Unblock brings it back.
        .filter(|s| !crate::engine::block::is_blocked(gui_state, &s.peer))
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

/// Keep a scratch pad note in the encrypted DM store (10m R10, as the web keeps its own): the
/// scratch pad sends nothing, so this is the only copy, and for a file the only copy of its key.
/// Per identity and server, like the store. Nothing is kept while the identity is locked or no
/// server is chosen (there is no store to key it by); the note still shows until the view
/// changes, as before.
pub(crate) fn keep_scratch_note(gui_state: &mut GuiState, ts: u64, text: &str, reply: Option<&crate::gui::ReplyContext>) {
    if !ensure_dm_store(gui_state) {
        return;
    }
    let reply = reply.map(|r| crate::net::dm_store::NoteReply {
        sender_key: r.sender_key.clone(),
        sender_name: r.sender_name.clone(),
        preview: r.preview.clone(),
        timestamp_ms: r.timestamp_ms,
    });
    if let Some(store) = gui_state.dm_store.as_mut() {
        store.add_scratch_note(crate::net::dm_store::ScratchNote { ts, text: text.to_string(), reply });
        store.save();
    }
}

/// The scratch pad was opened: its kept notes into the shared message list (the standard
/// renderer draws them), each with its reply quote, under the person's own name.
pub(crate) fn load_scratchpad(gui_state: &mut GuiState) {
    gui_state.chat_messages.retain(|m| m.channel != "scratchpad");
    if !ensure_dm_store(gui_state) {
        return;
    }
    let Some(store) = gui_state.dm_store.as_ref() else { return };
    let name = if gui_state.user_name.is_empty() { "You".to_string() } else { gui_state.user_name.clone() };
    let notes: Vec<crate::gui::ChatMessage> = store
        .scratch_notes()
        .iter()
        .map(|n| crate::gui::ChatMessage {
            sender_name: name.clone(),
            sender_key: gui_state.profile_public_key.clone(),
            content: n.text.clone(),
            timestamp: crate::gui::pages::chat::format_timestamp(n.ts),
            timestamp_ms: n.ts,
            channel: "scratchpad".to_string(),
            reply_to: n.reply.as_ref().map(|r| crate::gui::ReplyContext {
                sender_key: r.sender_key.clone(),
                sender_name: r.sender_name.clone(),
                preview: r.preview.clone(),
                timestamp_ms: r.timestamp_ms,
                conversation: "scratchpad".to_string(),
                private: true,
            }),
            ..Default::default()
        })
        .collect();
    gui_state.chat_messages.extend(notes);
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
    // Step C (2026-10-09, blocking-and-safe-mode.md 10d), before anything is stored, listed or
    // notified: a note to ourselves about the block list is applied, and anything from a key we
    // blocked (a message, a knock, a follow notice, a pass, a contact request) is dropped.
    if crate::engine::block::screens_dm(gui_state, inner) {
        return false;
    }
    // 10n: a note to ourselves saying what a friend may do is applied (once, the newer choice
    // winning) and never shown; one from anyone else is dropped unread.
    if crate::engine::choice::screens_dm(gui_state, inner) {
        return false;
    }
    if matches!(
        inner.text.as_str(),
        crate::net::dm_pq::CTL_FOLLOW | crate::net::dm_pq::CTL_UNFOLLOW | crate::net::dm_pq::CTL_FRIEND_CERT
    ) {
        ingest_control(gui_state, inner);
        return false; // acted on; nothing to render
    }
    // Step B (2026-10-09, blocking-and-safe-mode.md 10c): a contact request is a control message
    // too (its pass checked, its sender listed under Requests); and a DM from someone our own
    // message setting would refuse is listed the same way, name only, its text never stored.
    if inner.text.starts_with(crate::net::reach::CONTACT_REQUEST_MARKER) {
        crate::engine::reach::ingest_contact_request(gui_state, inner);
        return false;
    }
    // 10j: a report about a group I created, from one of its members, waits for its check against
    // my own copy of the group and is listed under Safety, never stored or shown as a message.
    // Before the rule below, because a group's members need not be my friends.
    if crate::engine::group_report::ingest(gui_state, inner) {
        return false;
    }
    if crate::engine::reach::file_if_refused(gui_state, inner) {
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
    let mut forget_approval: Option<String> = None;
    let mut send_withdrawals = false;
    // This device's own passes still waiting for the server's answer.
    let on_its_way = gui_state.pending_puts.serials_to(&peer);
    if let Some(store) = gui_state.dm_store.as_mut() {
        match inner.text.as_str() {
            crate::net::dm_pq::CTL_FOLLOW => {
                if from_me {
                    // Our own follow echoed from another device (and any Unfollow of ours still
                    // waiting to go out for them is void: they are followed again, 10n N3).
                    store.set_following(&peer, true);
                    store.drop_pending_unfollow(&peer);
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
                    // (that device withdrew the ones it knew of; a repeat is harmless), and so
                    // does our choice of what they may do (10c-ii), as it does on that device:
                    // back to the defaults as of the Unfollow's own time (10n), so a choice made
                    // after it, read later, still wins.
                    store.set_following(&peer, false);
                    store.clear_choice_at(&peer, inner.ts);
                    withdraw_from = Some(peer.clone());
                    // Step G: off the protected setup's approved list here too.
                    forget_approval = Some(peer.clone());
                } else {
                    store.set_follower(&peer, false);
                    store.forget_cert_from(&peer);
                }
            }
            crate::net::dm_pq::CTL_FRIEND_CERT => {
                if let Some(cert) = inner.cert.as_deref() {
                    if from_me {
                        // A pass we gave, echoed from another device (10n N4): remember its
                        // serial, so any device can withdraw it, unless it allows more than the
                        // choice for them, which travels in its own note (engine/choice.rs). An
                        // echo never changes the choice and never withdraws another pass: echoes
                        // arrive late, out of order, or never, and rebuilding the choice from them
                        // is what lost or reversed choices across devices before 10n.
                        match crate::relay::core::pq_crypto::parse_friend_cert(cert) {
                            // Already on the record: this device's own pass, whose self-copy goes
                            // out once the server took it (10l) and so comes back after it was
                            // recorded. Nothing to learn.
                            Ok((pass, _)) if store.passes_sent_to(&peer).iter().any(|p| p.serial == pass.serial) => {}
                            // One this device is withdrawing (10m R5, the web's `adoptEchoedPass`):
                            // an echo arriving late must not bring back a pass the person took back.
                            Ok((pass, _)) if store.pending_withdrawals().contains(&pass.serial) => {}
                            // This device's own pass whose answer has not come (on its way, or
                            // never answered): its answer, `dm_put_ok`, records it if it comes.
                            // (Since 10n N5 every self-copy waits for that answer, so this is a
                            // guard, not a path.)
                            Ok((pass, _)) if on_its_way.contains(&pass.serial) || store.passes_unanswered_to(&peer).iter().any(|p| p.serial == pass.serial) => {}
                            // It allows more than the choice for them (an older pass read after a
                            // newer choice): withdrawn at once, never recorded.
                            Ok((pass, _)) if crate::net::reach::grants_beyond(&pass.may.wire(), &store.intended_may_wire(&peer)) => {
                                store.withdraw_serial(&pass.serial);
                                send_withdrawals = true;
                            }
                            Ok((pass, _)) => {
                                // The device that knows has spoken for them (10m R3): this one may
                                // look after their pass on its own again.
                                store.clear_changed_elsewhere(&peer);
                                store.record_pass_sent(&peer, SentPass { serial: pass.serial, may: pass.may.wire() });
                            }
                            Err(e) => log::warn!("our own friendship pass echoed unreadable ({e:?}); ignored"),
                        }
                    } else {
                        let server = store.pass_server().unwrap_or("").to_string();
                        match crate::relay::core::pq_crypto::verify_friend_cert(&server, &inner.from, &me, cert) {
                            Ok(_) => {
                                store.store_cert_from(&inner.from, cert);
                                // Their pass gets us past their gate now: the refusal notice in
                                // our conversation with them (step B) has done its job.
                                gui_state.reach.refused.remove(&inner.from);
                                // And ours, refused by their settings before (10m R7), can go now:
                                // it rides their pass, so the pacer sends it soon.
                                if gui_state.reach.pass_refused.remove(&inner.from) {
                                    gui_state.pass_pacer.held = true;
                                }
                            }
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
    if let Some(peer) = forget_approval {
        crate::engine::protected::forget(gui_state, &peer);
    }
    if send_withdrawals {
        send_pending_withdrawals(gui_state);
    }
    refresh_social_mirrors(gui_state);
}

/// Seal + send one control message to `peer` (recipient copy + self copy,
/// exactly like a chat DM so other devices stay in sync). Returns false
/// when we can't seal yet (no kyber key for the peer).
pub(crate) fn send_dm_control(gui_state: &mut GuiState, peer: &str, text: &str, cert: Option<String>) -> bool {
    send_dm_control_at(gui_state, peer, text, cert.as_deref(), now_ms())
}

/// [`send_dm_control`] signed at `ts` rather than now: an Unfollow that waited for a connection
/// (10n N3) goes out dated when the person made it. False, sending nothing, while it cannot be
/// sealed or nothing is connected.
pub(crate) fn send_dm_control_at(gui_state: &GuiState, peer: &str, text: &str, cert: Option<&str>, ts: u64) -> bool {
    let Some(ref client) = gui_state.ws_client else { return false };
    if !client.is_connected() {
        return false;
    }
    let Some((put_peer, put_self)) = control_puts_at(gui_state, peer, text, cert, ts) else { return false };
    client.send(&put_peer.to_string());
    client.send(&put_self.to_string());
    true
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// The two `dm_put` frames of one control message to `peer` (theirs, then our self-copy), built
/// without sending, so what goes on the wire can be tested. Theirs carries the pass THEY gave us
/// when we hold one (`friend_cert`), so it rides the friend lane past their "who can reach me"
/// gate: after Accept that is the pass their contact request brought (step B). None when it
/// cannot be sealed yet (locked, or no DM key for them: they must come online once).
pub(crate) fn control_puts(gui_state: &GuiState, peer: &str, text: &str, cert: Option<&str>) -> Option<(serde_json::Value, serde_json::Value)> {
    control_puts_at(gui_state, peer, text, cert, now_ms())
}

/// [`control_puts`] signed at `ts`.
pub(crate) fn control_puts_at(gui_state: &GuiState, peer: &str, text: &str, cert: Option<&str>, ts: u64) -> Option<(serde_json::Value, serde_json::Value)> {
    let seed = gui_state.private_key_bytes.as_ref()?;
    let me = &gui_state.profile_public_key;
    let Some(peer_kyber) = gui_state.peer_kyber_keys.get(peer) else {
        let what: String = text.chars().take(26).collect(); // the marker, not a whole pass
        log::warn!("control '{what}' to {}… not sent: no kyber key yet (they must come online once)", &peer[..12.min(peer.len())]);
        return None;
    };
    let my_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).ok()?;
    let inner_json = crate::net::dm_pq::build_signed_inner_ext(seed, me, peer, ts, text, cert).ok()?;
    let env_peer = crate::net::dm_pq::seal_v2(peer_kyber, &inner_json).ok()?;
    let env_self = crate::net::dm_pq::seal_v2(&my_kp.public_base64(), &inner_json).ok()?;
    let mut put_peer = serde_json::json!({ "type": "dm_put", "to": peer, "content": env_peer });
    if let Some(c) = gui_state.dm_store.as_ref().and_then(|s| s.cert_for(peer)) {
        put_peer["friend_cert"] = serde_json::Value::String(c.to_string());
    }
    Some((put_peer, serde_json::json!({ "type": "dm_put", "to": me, "content": env_self })))
}

/// Issue + deliver MY friendship pass to `peer` (idempotent: nothing when one stands).
///
/// v2 (2026-10-09): the pass names this server's did:hum (from its `identify_challenge`), a
/// fresh random serial and what the friend may do: what is ticked for them in Settings >
/// Safety's "People I choose" (10c-ii), which for someone nobody chose anything for is the
/// defaults two new friends get, leaving out calls (calls come only from people the person
/// chooses). Nothing is minted until the server and the peer's DM key are known; the sweep on the
/// next member list tries again.
pub(crate) fn send_friend_cert(gui_state: &mut GuiState, peer: &str) {
    // One on its way already (10l): its answer settles it, and a second would be a second pass.
    // And none on its own for a friend this device leaves alone (10m R3, R7); the person's own
    // acts (Follow, Accept) clear that first (`person_chose`).
    if gui_state.dm_store.as_ref().is_some_and(|s| !s.cert_sent_to(peer)) && !gui_state.pending_puts.has_peer(peer) && !left_alone(gui_state, peer) {
        mint_and_send_pass(gui_state, peer, Held::Pass { reissue: false });
    }
}

/// A friend this device sends no pass ON ITS OWN (the sweep, a follow-back arriving, a contact
/// request completing a friendship), 10m: one whose pass my other device withdrew (R3, the DM
/// store's "changed on my other device" mark), and one whose pass the server refused this run
/// for THEIR settings (R7), where sending it again would only be refused again.
pub(crate) fn left_alone(gui_state: &GuiState, peer: &str) -> bool {
    gui_state.reach.pass_refused.contains(peer) || gui_state.dm_store.as_ref().is_some_and(|s| s.changed_elsewhere(peer))
}

/// The person acted for `peer` on this device (changed their ticks, followed or unfollowed them,
/// accepted their request): whatever this device was leaving alone for them (`left_alone`) it
/// looks after again, from what the person has just chosen here.
pub(crate) fn person_chose(gui_state: &mut GuiState, peer: &str) {
    gui_state.reach.pass_refused.remove(peer);
    if let Some(store) = gui_state.dm_store.as_mut() {
        if store.clear_changed_elsewhere(peer) {
            store.save();
        }
    }
}

/// Mint a new pass for `peer` carrying the choice for them (10n: what the person ticked in
/// Settings > Safety's "People I choose", 10c-ii, here or on another device; the step A defaults
/// when nothing was chosen) and send it. True once it is on its way: it counts as given only when
/// the server answers `dm_put_ok` (10l, engine/put_answer.rs). False when it cannot go out yet (no
/// server identity, no DM key for them, locked, offline, or this connection's mailbox not read
/// yet, N7), which the pass sweep retries.
fn mint_and_send_pass(gui_state: &mut GuiState, peer: &str, held: Held) -> bool {
    if crate::engine::block::is_blocked(gui_state, peer) {
        return false; // never a pass for someone we blocked (step C), whatever a stray echo says
    }
    // 10n N7: nothing goes out before this connection's mailbox was read, so a pass never carries
    // a choice that a note waiting there has already changed.
    if !mailbox_read(gui_state) {
        return false;
    }
    let Some(may) = gui_state.dm_store.as_ref().map(|s| s.intended_may_wire(peer)) else { return false };
    if !gui_state.peer_kyber_keys.contains_key(peer) {
        return false; // cannot seal to them yet; no point signing a pass that cannot be sent
    }
    let words: Vec<&str> = may.split(',').collect();
    let Some((cert, sent)) = mint_pass(gui_state, peer, &words) else { return false };
    let Some((theirs, ours)) = control_puts(gui_state, peer, crate::net::dm_pq::CTL_FRIEND_CERT, Some(&cert)) else { return false };
    send_held(gui_state, peer, theirs, ours, sent, held)
}

/// Send `theirs`, a `dm_put` that changes friendship state, with a fresh `ref`, and hold it until
/// the server answers (10l): `ours`, the self-copy, waits with it and goes out only once the
/// server took theirs, so our other devices never learn of a pass the friend never got; the pass
/// waits on the store's list of unanswered passes, never yet on the record. Every pass's, with no
/// exception (10n N5 withdrew 10m R2's early self-copy: the choice now travels in its own note,
/// and the early copy was how other devices came to record passes the server had refused). False,
/// sending nothing, while not connected.
pub(crate) fn send_held(gui_state: &mut GuiState, peer: &str, mut theirs: serde_json::Value, ours: serde_json::Value, pass: SentPass, held: Held) -> bool {
    let Some(client) = gui_state.ws_client.as_ref().filter(|c| c.is_connected()) else { return false };
    let Some(reference) = crate::net::put_answers::new_ref() else { return false };
    theirs["ref"] = serde_json::Value::String(reference.clone());
    client.send(&theirs.to_string());
    let self_copy = ours;
    if let Some(store) = gui_state.dm_store.as_mut() {
        store.pass_on_its_way(peer, pass.clone());
        store.save();
    }
    let at = std::time::Instant::now();
    gui_state.pending_puts.hold(PendingPut { reference, peer: peer.to_string(), pass, self_copy, held, at });
    true
}

/// Sign MY pass for `peer` on this server allowing `may`, under a fresh serial: the pass JSON,
/// and the record to keep once it has gone out. Nothing is recorded here. None until the server's
/// identity is known (its `identify_challenge`), or while the identity is locked.
pub(crate) fn mint_pass(gui_state: &GuiState, peer: &str, may: &[&str]) -> Option<(String, SentPass)> {
    use crate::relay::core::pq_crypto::{build_friend_cert, new_friend_cert_serial, FriendMay};
    // Step G: with the protected setup on, a pass goes only to a friend the PIN holder let be one
    // (kept in its review, or befriended with the PIN), so no route makes a friend without it: not
    // even a follow made before the setup and followed back after it.
    if !crate::engine::protected::pass_allowed(gui_state, peer) {
        return None;
    }
    let server = gui_state.dm_store.as_ref()?.pass_server()?;
    let seed = gui_state.private_key_bytes.as_ref()?;
    let serial = new_friend_cert_serial()?;
    // The minting step lives with its check in relay::core::pq_crypto, so both feature sets can
    // reach it (the relay's own tests mint passes too).
    match build_friend_cert(seed, server, &gui_state.profile_public_key, peer, &serial, may) {
        Ok(cert) => Some((cert, SentPass { serial, may: FriendMay::from_words(may.iter().copied()).ok()?.wire() })),
        Err(e) => {
            log::warn!("friendship pass not minted: {e:?}");
            None
        }
    }
}

/// THE PASSES FOLLOW THE CHOICE (10n N4), whenever the choice for `peer` changed (made here,
/// engine/choice.rs `make`) and on every sweep for a friend owed a pass:
/// 1. every pass of mine to them, on record or on its way, that allows more than the choice is
///    withdrawn at once (`withdraw_beyond_choice`): consent taken back takes effect now, and the
///    friend falls back to what the person's settings allow strangers until the new pass is taken.
///    This runs ALWAYS (10m R4), also when a pass carrying the choice already stands or one is on
///    its way, so an untick made before an earlier tick's pass was answered still takes that pass
///    back (once stored it would allow what was just unticked);
/// 2. then, when no pass carrying the choice stands and none is on its way, a new pass carrying
///    it goes out (`issue_choice_pass`). An added tick takes nothing away, so the old pass stands
///    until the server took the new one (10l), and is withdrawn then.
///
/// The note telling my other devices goes before both (`make`), so they hear the new choice before
/// they hear of its withdrawals. True when a new pass went out.
pub(crate) fn follow_choice(gui_state: &mut GuiState, peer: &str) -> bool {
    withdraw_beyond_choice(gui_state, peer);
    issue_choice_pass(gui_state, peer)
}

/// Step 1 of [`follow_choice`]: withdraw every pass to `peer`, given or still unanswered, that
/// allows a kind the relay checks which the choice does not (net/reach.rs `grants_beyond`; going
/// from `invite` alone to Trade takes nothing away, so a friend's only pass is never withdrawn
/// early for that). Also used when a choice arrives in a note (engine/choice.rs).
pub(crate) fn withdraw_beyond_choice(gui_state: &mut GuiState, peer: &str) {
    let Some(store) = gui_state.dm_store.as_mut() else { return };
    let want = store.intended_may_wire(peer);
    if !store.withdraw_passes_to_except(peer, |p| !crate::net::reach::grants_beyond(&p.may, &want)).is_empty() {
        store.save();
        send_pending_withdrawals(gui_state);
    }
}

/// Step 2 of [`follow_choice`]: a pass carrying the choice, when none stands and none is on its
/// way. It replaces the passes still standing (`Held::Pass { reissue }`): those are withdrawn
/// once the server took it.
fn issue_choice_pass(gui_state: &mut GuiState, peer: &str) -> bool {
    let Some(store) = gui_state.dm_store.as_ref() else { return false };
    let want = store.intended_may_wire(peer);
    let standing = store.passes_sent_to(peer);
    if standing.iter().any(|p| p.may == want) || gui_state.pending_puts.has_peer(peer) {
        return false;
    }
    let reissue = !standing.is_empty();
    mint_and_send_pass(gui_state, peer, Held::Pass { reissue })
}

/// Take back every pass I gave `peer` (on Unfollow; Block does the same through
/// engine/block.rs `enforce_on_store`, which also unfollows without telling them): the
/// store moves their serials to the waiting withdrawals, and those go to the relay now if we
/// are connected, or on the next connection. A send to them still waiting for its answer is
/// dropped (10m R6, as on the web): its answer records nothing, and a new pass or request need
/// not wait for it.
pub(crate) fn withdraw_passes(gui_state: &mut GuiState, peer: &str) {
    gui_state.pending_puts.drop_peer(peer);
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

/// The relay's `cert_revoked {serial}`: that withdrawal is done. When it was another of my
/// devices that withdrew it (10m R3), the pass leaves this device's record too, and a friend left
/// without one is marked "changed on my other device" (net/dm_store.rs `withdrawal_confirmed`):
/// this device sends them no pass on its own (`left_alone`) and its People I choose list shows
/// them as updating their pass. Any withdrawal that queued is sent.
pub(crate) fn withdrawal_confirmed(gui_state: &mut GuiState, frame: &serde_json::Value) {
    let Some(serial) = frame.get("serial").and_then(|v| v.as_str()) else { return };
    let mut marked = None;
    if let Some(store) = gui_state.dm_store.as_mut() {
        marked = store.withdrawal_confirmed(serial);
        store.save();
    }
    if let Some(peer) = marked {
        log::info!("a pass to {}… was withdrawn on another of my devices; left to that device", &peer[..12.min(peer.len())]);
        send_pending_withdrawals(gui_state);
        refresh_social_mirrors(gui_state);
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
/// The passes go out at the server's pace (`send_owed_passes`); what does not fit is held and
/// sent as the budget refills (`pace_owed_passes`, every frame).
///
/// 10n N7: it runs only once this connection's mailbox has been read and applied
/// (`mailbox_read`), so a device that was offline learns my notes (a choice, a block) before it
/// sends anything; the member list usually arrives first, and the last mailbox page then runs it
/// (`on_mailbox_read`). Then, in this order: the notes and Unfollows made while not connected
/// (N1, N3: before any withdrawal or pass of theirs), what a block made elsewhere takes back, the
/// withdrawals still unconfirmed, and the passes owed.
pub(crate) fn sweep_friend_passes(gui_state: &mut GuiState) {
    if !ensure_dm_store(gui_state) || !mailbox_read(gui_state) {
        return;
    }
    crate::engine::choice::flush(gui_state);
    // Step C: a block made on another server or device takes back what we gave here first, so
    // nothing below hands a blocked person a pass.
    crate::engine::block::sweep(gui_state);
    send_pending_withdrawals(gui_state);
    gui_state.pass_pacer.held = send_owed_passes(gui_state, std::time::Instant::now());
    crate::engine::reach::settle_requests(gui_state);
}

/// 10n N7: has this connection's mailbox been read to its last page and applied? The fetch goes
/// out once per connection (`dm_fetch_sent`, reset with every new socket) and `dm_fetch_done` is
/// set by its last page, so a new connection reads false until its own fetch is done.
pub(crate) fn mailbox_read(gui_state: &GuiState) -> bool {
    gui_state.dm_fetch_sent && gui_state.dm_fetch_done
}

/// The last page of this connection's mailbox has been read and applied (frame_ws_poll.rs, the
/// `dm_batch` arm): the pass sweep that waited for it runs now (10n N7).
pub(crate) fn on_mailbox_read(gui_state: &mut GuiState) {
    gui_state.dm_fetch_done = true;
    sweep_friend_passes(gui_state);
}

/// Give a pass carrying the choice to every friend owed one (10n N4: a mutual follow, or anyone
/// holding a pass from us, who holds none carrying it; net/dm_store.rs `owed_passes`), through
/// [`follow_choice`], within the background budget (net/put_pacer.rs): the server refuses, and does
/// not deliver, a `dm_put` over its burst. The pacing is a politeness; the guarantee is 10l's
/// answer: a pass counts as given only once the server took it, so one refused or unanswered
/// leaves the friend owed, and this sends them another carrying the same choice. A friend whose
/// pass is still on its way is left alone until its answer, and so is one this device leaves
/// alone (10m R3 and R7). True when some had to wait for the budget to refill.
pub(crate) fn send_owed_passes(gui_state: &mut GuiState, now: std::time::Instant) -> bool {
    use crate::net::put_pacer::PUTS_PER_CONTROL;
    crate::engine::put_answer::expire(gui_state, now);
    let owed = gui_state.dm_store.as_ref().map(|s| s.owed_passes()).unwrap_or_default();
    for peer in owed {
        if gui_state.pending_puts.has_peer(&peer) || left_alone(gui_state, &peer) {
            continue;
        }
        if !gui_state.pass_pacer.has(PUTS_PER_CONTROL, now) {
            return true;
        }
        if follow_choice(gui_state, &peer) {
            gui_state.pass_pacer.spend(PUTS_PER_CONTROL, now);
        }
    }
    false
}

/// Every frame: sends whose answer has not come in 30 seconds count as not taken (10l), and
/// passes the sweep held back for want of budget go out as it refills, at the server's pace,
/// while connected and once this connection's mailbox was read (10n N7; frame_ws_poll.rs, top of
/// the pump).
pub(crate) fn pace_owed_passes(gui_state: &mut GuiState, now: std::time::Instant) {
    crate::engine::put_answer::expire(gui_state, now);
    if !gui_state.pass_pacer.held || !mailbox_read(gui_state) || !gui_state.pass_pacer.has(crate::net::put_pacer::PUTS_PER_CONTROL, now) {
        return;
    }
    if !gui_state.ws_client.as_ref().is_some_and(|c| c.is_connected()) || gui_state.dm_store.is_none() {
        return;
    }
    gui_state.pass_pacer.held = send_owed_passes(gui_state, now);
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
    // Step G: with the protected setup on, Follow and Follow back make a friend and need the PIN
    // (Unfollow never does).
    if on && !crate::engine::protected::allows(gui_state, crate::net::protected::ProtectedAction::Follow(peer.to_string())) {
        return;
    }
    if on && crate::engine::block::is_blocked(gui_state, peer) {
        // A block takes the follow back (step C); following again starts with Unblock.
        gui_state.pending_notices.push("You blocked them. Unblock them first, in Settings > Safety > Blocked people.".to_string());
        return;
    }
    // The person's own choice for them, made here (10m R3, R7).
    person_chose(gui_state, peer);
    let mut at = now_ms();
    if let Some(store) = gui_state.dm_store.as_mut() {
        store.set_following(peer, on);
        if on {
            store.drop_pending_unfollow(peer); // followed again: an Unfollow still waiting is void
        } else {
            // Unfollow clears our choice of what they may do (10c-ii): a friendship begun again
            // later starts from the defaults, like any new one. As of the Unfollow's own signed
            // time (10n, as the web chat does): my other devices clear it as of that same time
            // when its self-copy reaches them (`clear_choice_at`), so a choice made after it on
            // another device still wins on every device. Never dated before the choice it clears.
            at = store.choose(peer, &crate::net::reach::intended_may_wire(crate::net::reach::FriendTicks::default()));
        }
        store.save();
    }
    if !on {
        // Step G: off the protected setup's approved list; a new friendship needs the PIN again.
        crate::engine::protected::forget(gui_state, peer);
    }
    let text = if on { crate::net::dm_pq::CTL_FOLLOW } else { crate::net::dm_pq::CTL_UNFOLLOW };
    let sent = send_dm_control_at(gui_state, peer, text, None, at);
    if !on && !sent {
        // 10n N3: an Unfollow that cannot go out now (nothing connected, or no DM key for them)
        // waits, both its copies, and goes on the next connection (engine/choice.rs `flush`);
        // until 10n it was simply lost, and my other devices kept following them.
        if let Some(store) = gui_state.dm_store.as_mut() {
            store.queue_unfollow(peer, at);
            store.save();
        }
    }
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
#[path = "dm_pace_tests.rs"]
mod pace_tests;

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
