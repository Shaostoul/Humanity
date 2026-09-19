//! P2P GROUPS: everything the chat page does to a group that is not drawing it.
//!
//! A P2P group opens like a channel (`p2pgroup:<id>` becomes the active
//! channel and its decrypted messages render in the same centre panel as
//! channels and DMs), so the DRAWING of a group is ordinary chat drawing and
//! stays in the page. What is special about a group is everything around that
//! picture: fetching the list, loading and decrypting a log, minting an invite,
//! posting, leaving, disbanding, and the signed-object traffic that keeps other
//! members' copies in step. That is this file.
//!
//! Extracted VERBATIM from `gui/pages/chat.rs` (file-size ratchet), which stood
//! at 9,053 lines against an 8,000 budget. First of four clusters out in that
//! pass. Nothing here changed shape: the only delta is that five functions the
//! page still calls went from private to `pub(super)`, each marked where it is
//! declared, because their callers stayed behind in `chat.rs`.
//!
//! WHY THIS IS ONE CLUSTER, in the order the code runs:
//!
//!   1. `refresh_p2p_groups` / `spawn_groups_list_refresh` fill the left rail's
//!      Groups section from the relay's `/api/v2/groups` projection.
//!   2. `spawn_group_load` + `apply_group_load` + `replace_p2p_messages` open
//!      one group: rekey if we are the creator, unseal the epoch key, fetch the
//!      roster name map, decrypt the history, and project it into
//!      `state.chat_messages` tagged with the `p2pgroup:<id>` channel so the
//!      standard message renderer handles it (identicons, sender grouping,
//!      theme: all reused, zero parallel UI).
//!   3. `drain_p2p_loaders` collects those worker threads on the UI thread.
//!   4. `send_p2p_group_message` posts, `mint_and_copy_p2p_invite` invites,
//!      `leave_p2p_group` and `disband_p2p_group` end a membership.
//!   5. `ensure_group_mesh`, `broadcast_group_obj` and `handle_p2p_group_obj`
//!      are the peer-to-peer side: the signed group objects that travel
//!      directly between members rather than through the relay.
//!
//! Like the other children of `chat.rs` this takes `use super::*`, so it sees
//! the page's imports and its private helpers (timestamps, `norm_server_url`,
//! the theme handle) without widening a single one of them.

use super::*;

/// Synchronous (ureq) refresh of `state.p2p_groups` from the relay's
/// /api/v2/groups projection. Matches the existing `upload_image_png_blocking`
/// pattern — fine for occasional refreshes (create/join/first-render); promote
/// to a background tokio task if it ever feels janky.
pub(crate) fn refresh_p2p_groups(state: &mut GuiState) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.as_ref() {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return,
    };
    let dilithium_hex = match crate::net::identity::derive_pq_identity(&seed) {
        Ok(id) => id.dilithium_hex,
        Err(e) => {
            log::warn!("refresh_p2p_groups: derive identity failed: {e}");
            return;
        }
    };
    match crate::net::api_v2::fetch_p2p_groups(&server_url, &dilithium_hex) {
        Ok(list) => {
            log::info!("refresh_p2p_groups: {} groups", list.len());
            state.p2p_groups = list;
        }
        Err(e) => {
            log::warn!("refresh_p2p_groups: fetch failed: {e}");
        }
    }
    state.p2p_groups_last_fetch = Some(std::time::Instant::now());
}

// ─────────────────────────── P2P Group (inline chat) ─────────────────────
// A P2P group opens like a channel: clicking it sets the active channel to
// "p2pgroup:<id>" and its decrypted messages render in the SAME center panel
// as channels and DMs (no modal — operator: switching to a group should feel
// like switching from #general to #announcements). These helpers do the
// network + crypto work (blocking ureq, same pattern as image upload):
//   enter_p2p_group — full sync on open: rekey-if-creator, fetch+unseal the
//                      epoch key, load the roster name map, decrypt history.
//   poll_p2p_group  — light 4s refresh: re-decrypt the log with the cached
//                      key (skips the key exchange + roster fetch).
// Both project decrypted GroupMessages into state.chat_messages tagged with
// the "p2pgroup:<id>" channel so the standard message renderer handles them
// (identicons, sender grouping, theme — all reused, zero parallel UI).

/// Format an epoch-ms timestamp as HH:MM:SS (UTC) for the message row.
/// Replace cached messages for `channel` with the freshly-decrypted set, while
/// preserving any of my just-sent local echoes the reload hasn't indexed yet.
/// Repaints from the relay's authoritative log (the source of truth) so edits/
/// removals elsewhere converge.
fn replace_p2p_messages(
    state: &mut GuiState,
    channel: &str,
    msgs: Vec<crate::net::api_v2::GroupMessage>,
) {
    let my_key = state.profile_public_key.clone();
    // My author fingerprint = BLAKE3(my Dilithium pubkey)[..16]. profile_public_key
    // IS the Dilithium pubkey hex (set in mod.rs from the PQ identity), so we
    // compute it with a cheap hex-decode + hash instead of derive_pq_identity —
    // which ran a full Dilithium + Kyber KEYGEN on every poll-apply (~ every 4s)
    // purely to recover a value we already have. Same result, no keygen churn.
    let my_fp = hex::decode(&my_key)
        .ok()
        .map(|b| crate::net::api_v2::author_fingerprint_hex(&b))
        .unwrap_or_default();

    // Preserve very-recent messages the reload hasn't indexed yet — so a poll
    // that races the relay (the author's POST, or a peer's mesh push that the
    // relay hasn't stored yet) doesn't blink a just-shown message out (the
    // "briefly disappeared then reappeared" the operator saw). Two sources:
    //   (a) my own optimistic local echoes (sender_key == my_key), and
    //   (b) inc-2 peer messages rendered from a WebRTC mesh push (handle_p2p_
    //       group_obj), which the 2s relay poll may not have indexed yet.
    // A pending message is kept only until the reload includes it (matched by
    // author fingerprint + content) and at most ~20s (a never-stored message
    // then falls off instead of lingering forever). For a peer message we map
    // its sender_key (pubkey hex) → fingerprint to compare against the reload.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let pending: Vec<ChatMessage> = state
        .chat_messages
        .iter()
        .filter(|m| {
            if m.channel != channel || now_ms.saturating_sub(m.timestamp_ms) >= 20_000 {
                return false;
            }
            // Fingerprint for this message's author: cheap hex-decode + hash of
            // the sender_key (== my_fp for my own echoes; the peer's fp for a
            // pushed peer message). Falls back to empty (never matches) on a
            // non-hex sender_key, which just means we keep it until it ages out.
            let author_fp = if m.sender_key == my_key {
                my_fp.clone()
            } else {
                hex::decode(&m.sender_key)
                    .ok()
                    .map(|b| crate::net::api_v2::author_fingerprint_hex(&b))
                    .unwrap_or_default()
            };
            // Keep it only if the reload doesn't already contain it.
            !msgs.iter().any(|lm| lm.author_fp == author_fp && lm.text == m.content)
        })
        .cloned()
        .collect();

    state.chat_messages.retain(|m| m.channel != channel);
    for m in msgs {
        // inc-2: mark every poll-loaded object_id seen so a mesh push that
        // arrives AFTER the poll already rendered this object (peer's relay POST
        // beat their push to us) is deduped by handle_p2p_group_obj instead of
        // double-rendering. The poll itself rebuilds from the authoritative relay
        // log and never consults the seen-set, so this can't suppress the poll.
        #[cfg(feature = "native")]
        if !m.object_id.is_empty() {
            state.p2p_group_seen_obj_ids.insert(m.object_id.clone());
        }
        let is_me = !my_fp.is_empty() && m.author_fp == my_fp;
        let sender_key = if is_me {
            my_key.clone()
        } else {
            state
                .p2p_group_fp_to_key
                .get(&m.author_fp)
                .cloned()
                .unwrap_or_else(|| m.author_fp.clone())
        };
        let sender_name = if is_me {
            if !state.user_name.is_empty() { state.user_name.clone() } else { "You".to_string() }
        } else {
            state
                .p2p_group_fp_to_name
                .get(&m.author_fp)
                .cloned()
                .unwrap_or_else(|| format!("{}…", &m.author_fp[..12.min(m.author_fp.len())]))
        };
        // Use the app-standard "HH:MM UTC" formatter (same as chrono_now_str,
        // which the local send-echo uses) so a message keeps ONE timestamp
        // format — fixes the echo's HH:MM flipping to HH:MM:SS on reconcile.
        let ts_str = format_timestamp(m.created_at as u64);
        state.chat_messages.push(ChatMessage {
            sender_name,
            sender_key,
            content: m.text,
            timestamp: ts_str,
            timestamp_ms: m.created_at as u64,
            channel: channel.to_string(),
            server: norm_server_url(&state.server_url),
            ..Default::default()
        });
    }
    // Re-add my preserved local echoes (just-sent, not yet in the reload).
    for p in pending {
        state.chat_messages.push(p);
    }
    // Global sort by ms keeps each channel's relative order correct (the
    // renderer filters by channel + iterates in vec order). Cross-channel
    // interleave is irrelevant since only one channel renders at a time.
    state.chat_messages.sort_by_key(|m| m.timestamp_ms);
    while state.chat_messages.len() > 400 { state.chat_messages.remove(0); }

    // inc-2: bound the dedup set so a very long single-group session can't grow
    // it without limit. HashSet has no ordering to evict by, and the poll just
    // re-inserted every currently-loaded (authoritative) object_id above, so if
    // it has ballooned far past the message cap we simply clear it. The only
    // cost is that a push which raced THIS exact rebuild could re-render once;
    // the next 2s poll collapses it. (In practice this branch ~never fires —
    // the set is reset on every group switch.)
    #[cfg(feature = "native")]
    if state.p2p_group_seen_obj_ids.len() > 4000 {
        state.p2p_group_seen_obj_ids.clear();
    }
}

/// Spawn a BACKGROUND load of `group_id` (rekey/epoch/roster/messages) and
/// stash the receiver. The UI never blocks: `drain_p2p_loaders` applies the
/// result when the worker returns. `fresh` = true for a click (resets cached
/// state + shows "Loading…"); false for a periodic refresh (keeps the current
/// view until new data arrives, so polling doesn't flicker).
pub(crate) fn spawn_group_load(state: &mut GuiState, group_id: &str, fresh: bool) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    state.p2p_group_active_id = group_id.to_string();
    state.p2p_group_last_fetch = Some(std::time::Instant::now());
    if fresh {
        // Clear cached state so a stale prior group's key/maps can't leak into
        // the loading view; show the spinner hint until the worker returns.
        state.p2p_group_chat_epoch = 0;
        state.p2p_group_chat_epoch_key = None;
        state.p2p_group_fp_to_key.clear();
        state.p2p_group_fp_to_name.clear();
        state.p2p_group_loading = true;
        // inc-2: fresh dedup set per opened group (mirror web's clear on switch),
        // so object_ids from a prior group can't suppress a new group's messages.
        #[cfg(feature = "native")]
        state.p2p_group_seen_obj_ids.clear();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let gid = group_id.to_string();
    std::thread::spawn(move || {
        let load = crate::net::api_v2::load_group_blocking(&server_url, &seed, &gid);
        let _ = tx.send(load); // receiver may be gone if the user switched away
    });
    state.p2p_group_loader = Some((group_id.to_string(), rx));
}

/// Apply a finished background `GroupLoad` to state (main thread): cache the
/// epoch key + roster maps and repaint the message log. Ignored if the user
/// has since switched to a different group.
// `pub(super)` only because its caller (the left rail's Groups section) stayed
// in `chat.rs`; it was private when the two lived in one file.
pub(super) fn apply_group_load(state: &mut GuiState, load: crate::net::api_v2::GroupLoad) {
    if state.p2p_group_active_id != load.group_id {
        return; // stale — user switched away before this returned
    }
    let channel = format!("p2pgroup:{}", load.group_id);
    state.p2p_group_chat_epoch = load.epoch;
    state.p2p_group_chat_epoch_key = load.epoch_key;
    // Rebuild the fp → pubkey / name maps (name resolved here on the main
    // thread, where chat_users is available).
    state.p2p_group_fp_to_key.clear();
    state.p2p_group_fp_to_name.clear();
    for (fp, pubkey_hex) in &load.members {
        let name = state
            .chat_users
            .iter()
            .find(|u| u.public_key == *pubkey_hex)
            .map(|u| u.name.clone())
            .unwrap_or_else(|| format!("{}…", &pubkey_hex[..8.min(pubkey_hex.len())]));
        state.p2p_group_fp_to_name.insert(fp.clone(), name);
        state.p2p_group_fp_to_key.insert(fp.clone(), pubkey_hex.clone());
    }
    replace_p2p_messages(state, &channel, load.messages);
    state.p2p_group_loading = false;
    // inc-2: now that the roster is known, open/maintain DataChannels to every
    // member so subsequent messages arrive P2P (low-latency). Runs on every load
    // (initial + each 2s refresh) so the mesh tracks roster changes; offer_to is
    // idempotent for already-connected peers.
    #[cfg(feature = "native")]
    ensure_group_mesh(state);
}

/// Spawn a BACKGROUND refresh of the whole P2P-group list (left rail + member
/// counts). Keeps the list fresh when membership changes on another client and
/// lets us detect when the open group was disbanded/left elsewhere.
pub(crate) fn spawn_groups_list_refresh(state: &mut GuiState) {
    if state.p2p_groups_list_loader.is_some() {
        return; // one in flight already
    }
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => return,
    };
    let dilithium_hex = match crate::net::identity::derive_pq_identity(&seed) {
        Ok(id) => id.dilithium_hex,
        Err(_) => return,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok(list) = crate::net::api_v2::fetch_p2p_groups(&server_url, &dilithium_hex) {
            let _ = tx.send(list);
        }
    });
    state.p2p_groups_list_loader = Some(rx);
}

/// Drain any finished background loaders (called once at the top of draw).
/// Applies the active-group load + the group-list refresh, and exits a group
/// that vanished from the list (disbanded or left on another device).
pub(crate) fn drain_p2p_loaders(state: &mut GuiState) {
    // (1) Active-group load.
    if let Some((_gid, rx)) = &state.p2p_group_loader {
        match rx.try_recv() {
            Ok(load) => {
                state.p2p_group_loader = None;
                apply_group_load(state, load);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.p2p_group_loader = None; // worker died without sending
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    // (2) Group-list refresh.
    if let Some(rx) = &state.p2p_groups_list_loader {
        match rx.try_recv() {
            Ok(list) => {
                state.p2p_groups_list_loader = None;
                state.p2p_groups = list;
                // If the open group is gone (disbanded or we left it elsewhere),
                // exit to #general and drop its decrypted history.
                if let Some(agid) = state
                    .chat_active_channel
                    .strip_prefix("p2pgroup:")
                    .map(|s| s.to_string())
                {
                    if !state.p2p_groups.iter().any(|g| g.group_id == agid) {
                        let channel = format!("p2pgroup:{}", agid);
                        state.chat_messages.retain(|m| m.channel != channel);
                        state.chat_active_channel = "general".to_string();
                        state.p2p_group_active_id.clear();
                        state.p2p_group_chat_epoch_key = None;
                        state.p2p_group_loading = false;
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.p2p_groups_list_loader = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
}

/// Mint a fresh 7-day invite ticket for `group_id` and copy it to the
/// clipboard. Sets a transient status line shown in the group header.
// `pub(super)` only because its callers (the Groups section and the centre
// panel's group header) stayed in `chat.rs`.
pub(super) fn mint_and_copy_p2p_invite(
    ctx: &egui::Context,
    state: &mut GuiState,
    group_id: &str,
    group_name: &str,
) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    let mut secret = vec![0u8; 32];
    use rand::RngCore;
    rand::rng().fill_bytes(&mut secret);
    let secret_hash = blake3::hash(&secret).as_bytes().to_vec();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let expires_at = now + 7 * 24 * 3600 * 1000;
    match crate::net::api_v2::submit_group_invite_v1(&server_url, &seed, group_id, expires_at, &secret_hash) {
        Ok(invite_id) => {
            let ticket = crate::net::api_v2::encode_invite_ticket(group_id, group_name, &invite_id, &secret);
            ctx.copy_text(ticket);
            state.p2p_group_invite_status = "Invite ticket copied, share within 7 days.".to_string();
        }
        Err(e) => {
            state.p2p_group_invite_status = format!("Invite failed: {e}");
        }
    }
}

/// Leave a P2P group (self-remove from the roster). Submits a
/// `group_member_v1` remove for my own key, then drops the view back to
/// #general and refreshes the group list so the row disappears.
// `pub(super)` only because its caller (the Groups section's row menu) stayed
// in `chat.rs`.
pub(super) fn leave_p2p_group(state: &mut GuiState, group_id: &str) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    match crate::net::api_v2::submit_group_leave(&server_url, &seed, group_id) {
        Ok(()) => {
            // Drop any cached messages for this group + leave the view.
            let channel = format!("p2pgroup:{}", group_id);
            state.chat_messages.retain(|m| m.channel != channel);
            state.chat_active_channel = "general".to_string();
            state.p2p_group_active_id.clear();
            state.p2p_group_chat_epoch_key = None;
            state.p2p_group_loader = None; // cancel any in-flight load for the left group
            state.p2p_group_loading = false;
            state.p2p_group_invite_status.clear();
            refresh_p2p_groups(state);
        }
        Err(e) => {
            state.p2p_group_invite_status = format!("Leave failed: {e}");
        }
    }
}

/// Disband a P2P group I created (creator-only `group_disband_v1`). Removes it
/// for everyone, drops the view back to #general, and refreshes the list.
// `pub(super)` only because its caller (the Groups section's row menu) stayed
// in `chat.rs`.
pub(super) fn disband_p2p_group(state: &mut GuiState, group_id: &str) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    match crate::net::api_v2::submit_group_disband(&server_url, &seed, group_id) {
        Ok(()) => {
            let channel = format!("p2pgroup:{}", group_id);
            state.chat_messages.retain(|m| m.channel != channel);
            state.chat_active_channel = "general".to_string();
            state.p2p_group_active_id.clear();
            state.p2p_group_chat_epoch_key = None;
            state.p2p_group_loader = None; // cancel any in-flight load for the disbanded group
            state.p2p_group_loading = false;
            state.p2p_group_invite_status.clear();
            refresh_p2p_groups(state);
        }
        Err(e) => {
            state.p2p_group_invite_status = format!("Disband failed: {e}");
        }
    }
}

/// Dispatch a P2P group message: do the synchronous pre-checks (we have a
/// group id, a seed, and an epoch key), then encrypt + POST on a BACKGROUND
/// thread so the UI never blocks on the network round-trip. Returns true if the
/// send was *dispatched* (caller then shows the optimistic echo + clears the
/// input); false only if a pre-check failed (no key/seed — caller aborts the
/// echo and the status line explains why).
///
/// Failure of the background POST is rare (network) and self-reconciles: the
/// optimistic echo is dropped on the next ~4s poll because the relay won't
/// serve back a message it never stored. (Previously this blocked the UI
/// thread on EVERY group message send — tens-to-hundreds of ms per message.)
// `pub(super)` only because its caller (`send_composed_content`, the page's one
// send path) stayed in `chat.rs`.
pub(super) fn send_p2p_group_message(state: &mut GuiState, channel: &str, content: &str) -> bool {
    let gid = match channel.strip_prefix("p2pgroup:") {
        Some(g) => g.to_string(),
        None => return false,
    };
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return false;
        }
    };
    let key = match state.p2p_group_chat_epoch_key.clone() {
        Some(k) => k,
        None => {
            state.p2p_group_invite_status =
                "No epoch key yet, the creator must open the group once first.".to_string();
            return false;
        }
    };
    let epoch = state.p2p_group_chat_epoch.max(1);
    state.p2p_group_invite_status.clear();
    let content = content.to_string();

    // inc-2: build+sign the group_msg_v1 ONCE on the main thread so we have its
    // object_id + submission JSON for the mesh push and the seen-set. (Signing is
    // a few ms of Dilithium — fine inline; the network POST is what we defer to a
    // background thread.) Mirrors web `sendGroupMessage`: build → broadcast →
    // POST. If the build fails we abort the send (no echo) like the web client.
    let (object_id, submission_json) = match crate::net::api_v2::build_group_msg_submission(
        &seed, &gid, epoch, &key, &content,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            log::warn!("group message build failed: {e}");
            state.p2p_group_invite_status = format!("Send failed: {e}");
            return false;
        }
    };

    // Push P2P FIRST (instant for connected roster members), then POST to the
    // relay (durable cache + offline backfill). Mark our own object seen so the
    // 2s poll doesn't re-handle it as if it were a peer push. (This whole file is
    // native-gated, so the mesh handle + seen-set always exist here.)
    state.p2p_group_seen_obj_ids.insert(object_id.clone());
    broadcast_group_obj(state, &gid, &submission_json);

    // Relay POST on a BACKGROUND thread so the UI never blocks on the round-trip.
    std::thread::spawn(move || {
        if let Err(e) = crate::net::api_v2::post_submission_json(&server_url, &submission_json) {
            log::warn!("group message relay POST failed (push may still have delivered; echo drops on next poll if not stored): {e}");
        }
    });
    true
}

/// inc-2: open/maintain WebRTC DataChannels to the active group's roster so
/// pushed messages arrive P2P (low-latency); the 2s relay poll remains the
/// offline backfill + source of truth. Mirrors web `ensureGroupMesh`.
///
/// The manager's offerer rule (only `my_key > peer` actually offers) handles
/// glare, so we just call `offer_to` for EVERY roster member — offline members
/// simply never connect (their offer goes nowhere), and an already-open/-pending
/// channel is a cheap no-op inside the manager. Called from the group-load apply
/// and the periodic refresh, so the mesh tracks roster changes.
#[cfg(feature = "native")]
pub(crate) fn ensure_group_mesh(state: &GuiState) {
    let webrtc = match &state.webrtc {
        Some(w) => w,
        None => return, // manager not started yet (pre-connect)
    };
    // The roster (fp → pubkey hex) is populated by apply_group_load.
    for peer_hex in state.p2p_group_fp_to_key.values() {
        if peer_hex.is_empty() || *peer_hex == state.profile_public_key {
            continue; // skip self
        }
        // offer_to is idempotent + enforces the offerer rule internally.
        webrtc.offer_to(peer_hex.clone());
    }
}

/// inc-2: push a `{"type":"p2p_group_obj","submission":<submission json>}` frame
/// to every connected roster member over their WebRTC DataChannel. `send_text`
/// only reaches peers whose channel is open — that's correct; the relay POST in
/// the send path covers everyone else. Mirrors web `broadcastGroupObj`.
#[cfg(feature = "native")]
pub(crate) fn broadcast_group_obj(state: &GuiState, group_id: &str, submission_json: &str) {
    // Only broadcast for the group we actually have a roster for (the active one).
    if state.p2p_group_active_id != group_id {
        return;
    }
    let webrtc = match &state.webrtc {
        Some(w) => w,
        None => return,
    };
    // The frame's `submission` is the RAW submission JSON parsed back into a
    // value, so the receiver sees the exact object (not a double-encoded string).
    let submission_val: serde_json::Value = match serde_json::from_str(submission_json) {
        Ok(v) => v,
        Err(_) => return,
    };
    let frame = serde_json::json!({
        "type": "p2p_group_obj",
        "submission": submission_val,
    })
    .to_string();
    for peer_hex in state.p2p_group_fp_to_key.values() {
        if peer_hex.is_empty() || *peer_hex == state.profile_public_key {
            continue;
        }
        webrtc.send_text(peer_hex.clone(), frame.clone());
    }
}

/// inc-2: handle a `p2p_group_obj` frame that arrived over the WebRTC mesh
/// (called from the lib.rs per-frame pump when a `Frame{peer,text}` parses as
/// one). Native mirror of web `handleP2pGroupObj`. Returns true if the frame was
/// a (well-formed) p2p_group_obj we consumed — true even when dropped for a
/// legitimate reason (dup, wrong group, not a member, no key) so the caller does
/// NOT fall back to the inc-1 debug-line behavior; false only if the frame isn't
/// a p2p_group_obj at all.
///
/// SECURITY (the audit-critical path): the submission is UNTRUSTED. We
///   1. require `type == "p2p_group_obj"` with a `submission` object,
///   2. `verify_submission_json` — REJECT unless the ML-DSA signature verifies
///      over the canonical bytes (the same check the relay runs); a forged or
///      tampered object returns None here and is dropped,
///   3. dedup by the LOCALLY-recomputed object_id (never the wire id),
///   4. require object_type == "group_msg_v1" AND references[0] == active group,
///   5. gate author ∈ roster (author_fp must be a key in p2p_group_fp_to_key),
///   6. decrypt under the active epoch key; drop if absent/mismatch (the poll
///      backfills the full multi-epoch history).
/// Only after ALL of these do we render + mark the id seen.
#[cfg(feature = "native")]
pub(crate) fn handle_p2p_group_obj(state: &mut GuiState, _peer: &str, frame_text: &str) -> bool {
    // (0) Is this even a p2p_group_obj frame? If not, signal the caller to keep
    //     its existing behavior (the inc-1 "native p2p test" debug line).
    let frame: serde_json::Value = match serde_json::from_str(frame_text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if frame.get("type").and_then(|t| t.as_str()) != Some("p2p_group_obj") {
        return false;
    }
    let submission = match frame.get("submission") {
        Some(s) => s,
        None => return true, // malformed p2p_group_obj — consumed (drop)
    };
    let submission_json = submission.to_string();

    // (1+2) Verify the signature. This is the trust boundary — drop on failure.
    let verified = match crate::net::api_v2::verify_submission_json(&submission_json) {
        Some(v) => v,
        None => {
            log::debug!("p2p_group_obj: signature verify FAILED, dropping");
            return true;
        }
    };

    // (4) Only group messages, only for the currently-open group. (Epoch keys
    //     still ride the relay poll; references[0] is the group_id.)
    if verified.object_type != "group_msg_v1" {
        return true;
    }
    if verified.references.first().map(|s| s.as_str()) != Some(state.p2p_group_active_id.as_str())
        || state.p2p_group_active_id.is_empty()
    {
        return true; // for a different/inactive group — ignore
    }

    // (3) Dedup by the locally-recomputed object_id (push or a prior poll).
    if state.p2p_group_seen_obj_ids.contains(&verified.object_id) {
        return true;
    }

    // (5) Membership gate: the author must be in the active roster. fp_to_key's
    //     keys are author fingerprints; its values are the roster pubkey hexes.
    if !state.p2p_group_fp_to_key.contains_key(&verified.author_fp) {
        log::debug!("p2p_group_obj: author not in roster, dropping");
        return true;
    }

    // (6) Decrypt under the active epoch key. If we don't hold the key yet, drop
    //     — the relay poll backfills once the epoch key is fetched. For multi-
    //     epoch correctness the message carries its own epoch; for inc-2 we open
    //     under the active key and drop on mismatch (the poll's full multi-epoch
    //     load reconciles older epochs).
    let epoch_key = match &state.p2p_group_chat_epoch_key {
        Some(k) => k.clone(),
        None => return true,
    };
    let text = match crate::net::group_e2ee::open_group_msg(&verified.payload, &epoch_key) {
        Ok(t) => t,
        Err(e) => {
            // Wrong epoch key (e.g. a re-key the poll hasn't applied) → drop.
            log::debug!("p2p_group_obj: decrypt failed (epoch mismatch?), dropping: {e}");
            return true;
        }
    };

    // Passed every gate — render it. Resolve sender from the roster exactly like
    // replace_p2p_messages does (fp → name / pubkey hex), and use the standard
    // "HH:MM UTC" formatter so the timestamp matches the poll-rendered copy.
    state.p2p_group_seen_obj_ids.insert(verified.object_id.clone());

    let my_key = state.profile_public_key.clone();
    let is_me = verified.author_pubkey_hex == my_key;
    let sender_key = if is_me {
        my_key.clone()
    } else {
        state
            .p2p_group_fp_to_key
            .get(&verified.author_fp)
            .cloned()
            .unwrap_or_else(|| verified.author_pubkey_hex.clone())
    };
    let sender_name = if is_me {
        if !state.user_name.is_empty() { state.user_name.clone() } else { "You".to_string() }
    } else {
        state
            .p2p_group_fp_to_name
            .get(&verified.author_fp)
            .cloned()
            .unwrap_or_else(|| {
                format!("{}…", &verified.author_fp[..12.min(verified.author_fp.len())])
            })
    };
    let created_ms = verified.created_at.max(0) as u64;
    let channel = format!("p2pgroup:{}", state.p2p_group_active_id);
    state.chat_messages.push(ChatMessage {
        sender_name,
        sender_key,
        content: text,
        timestamp: format_timestamp(created_ms),
        timestamp_ms: created_ms,
        channel,
        server: norm_server_url(&state.server_url),
        ..Default::default()
    });
    state.chat_messages.sort_by_key(|m| m.timestamp_ms);
    while state.chat_messages.len() > 400 {
        state.chat_messages.remove(0);
    }
    true
}

