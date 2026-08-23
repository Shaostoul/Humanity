//! Background relay connections pump (multi-connection stage 3).
//!
//! The active server lives in GuiState's legacy `ws_*` / `chat_*` fields and
//! is driven by the full router in lib.rs. Every OTHER saved server lives in
//! `GuiState::connections` (see `ServerConnection`), and THIS pump keeps
//! those links alive: it dials saved servers that have no connection yet,
//! answers their identify challenges, stores incoming chat / federated chat
//! / DMs into the per-connection buffers (so unpark restores a current
//! picture, not a stale one), marks unread, and redials dropped links with
//! the same exponential backoff the active connection uses.
//!
//! Scope: this is deliberately a COMPACT router. Admin state (roles, bans,
//! server settings), tasks, listings, trades, and voice are active-server
//! concerns; their pages refetch on demand after a switch, so those message
//! types are dropped here. Reply context on background chat lines is also
//! not parsed (the line lands, its reply banner does not); the full router
//! rebuilds it from history if the user ever needs it.

use crate::engine::state::EngineState;
use crate::gui::pages::chat::norm_server_url;

/// Per-frame tick: dial, drain, store, redial. `dt` drives the reconnect
/// countdown, matching the active connection's backoff behavior.
pub(crate) fn pump_background_connections(state: &mut EngineState, dt: f32) {
    // Same gates as the active auto-connect: no identity, no sockets. A
    // random throwaway key would squat names on every saved server at once.
    if !state.gui_state.onboarding_complete
        || state.gui_state.private_key_bytes.is_none()
        || state.gui_state.profile_public_key.is_empty()
    {
        return;
    }

    dial_missing_saved_servers(state);

    // Drain every background socket first (only `ws` is borrowed), then
    // handle messages one at a time so handlers are free to borrow any
    // part of EngineState (decrypt helpers take the whole GuiState).
    let mut inbox: Vec<(usize, String)> = Vec::new();
    for (i, conn) in state.gui_state.connections.iter_mut().enumerate() {
        if conn.ws.is_none() && !conn.manually_disconnected && conn.reconnect_timer > 0.0 {
            conn.reconnect_timer -= dt;
        }
        if let Some(ws) = conn.ws.as_mut() {
            for m in ws.poll_messages() {
                inbox.push((i, m));
            }
        }
        if conn.ws.as_ref().map_or(false, |w| w.is_dropped()) {
            conn.ws = None;
            conn.identified = false;
            conn.status = "Disconnected".to_string();
            if !conn.manually_disconnected {
                conn.reconnect_timer = conn.reconnect_delay;
                conn.reconnect_delay = (conn.reconnect_delay * 2.0).min(60.0);
            }
        }
    }
    for (ci, raw) in inbox {
        handle_bg_message(state, ci, &raw);
    }

    pump_carrier_history(state);
    redial_dropped(state);
}

/// One-at-a-time REST history fetch for background carriers' federated
/// channels: drain a finished fetch into the connection's buffer, then
/// start the next queued channel. Same worker-thread + mpsc + short-timeout
/// shape as net_route::chat_history_pump (never blocks the render thread).
fn pump_carrier_history(state: &mut EngineState) {
    let my_key = state.gui_state.profile_public_key.clone();
    for conn in state.gui_state.connections.iter_mut() {
        // Drain a finished fetch.
        let mut done = false;
        if let Some(rx) = conn.history_rx.as_ref() {
            match rx.try_recv() {
                Ok((_channel, Ok(body))) => {
                    merge_history_into(conn, &body, &my_key);
                    done = true;
                }
                Ok((channel, Err(e))) => {
                    log::warn!("bg history fetch {}#{} failed: {}", conn.url, channel, e);
                    done = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => done = true,
            }
        }
        if done {
            conn.history_rx = None;
        }
        // Start the next queued channel.
        if conn.history_rx.is_none() {
            if let Some(channel) = conn.history_queue.pop() {
                let base = conn.display_url.trim_end_matches('/').to_string();
                let api_url = format!("{}/api/messages?limit=50&channel={}", base, channel);
                let (tx, rx) = std::sync::mpsc::channel();
                conn.history_rx = Some(rx);
                std::thread::spawn(move || {
                    let result = ureq::get(&api_url)
                        .timeout(std::time::Duration::from_secs(8))
                        .call()
                        .map_err(|e| e.to_string())
                        .and_then(|resp| resp.into_string().map_err(|e| e.to_string()));
                    let _ = tx.send((channel, result));
                });
            }
        }
    }
}

/// Merge a /api/messages body into a background connection's buffer,
/// rebuilding federated rows EXACTLY like the live arm (name suffix,
/// origin_server, server_id as sender key) and deduplicating against
/// what the connection already holds.
fn merge_history_into(conn: &mut crate::gui::ServerConnection, body: &str, my_key: &str) {
    let Ok(data) = serde_json::from_str::<serde_json::Value>(body) else {
        return;
    };
    let Some(messages) = data.get("messages").and_then(|v| v.as_array()) else {
        return;
    };
    for msg in messages {
        let is_federated = msg.get("type").and_then(|v| v.as_str()) == Some("federated_chat");
        let origin_server = if is_federated {
            msg.get("server_id").and_then(|v| v.as_str()).unwrap_or("").to_string()
        } else {
            String::new()
        };
        let sender_name = if is_federated {
            let from = msg.get("from_name").and_then(|v| v.as_str()).unwrap_or("Anonymous");
            let sname = msg.get("server_name").and_then(|v| v.as_str()).unwrap_or("federated");
            format!("{} ({})", from, sname)
        } else {
            msg.get("sender_name")
                .or_else(|| msg.get("from_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Anonymous")
                .to_string()
        };
        let sender_key = if is_federated {
            origin_server.clone()
        } else {
            msg.get("sender_key")
                .or_else(|| msg.get("from"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let timestamp = msg.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
        let channel = msg
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_string();
        if content.is_empty() {
            continue;
        }
        // Our own line already echoed on this connection.
        if !my_key.is_empty() && sender_key == my_key && conn.sent_timestamps.contains(&timestamp)
        {
            continue;
        }
        // Dedup against live lines already buffered (federated needs
        // content in the key: sender_key is the origin server for those).
        if conn.messages.iter().any(|m| {
            m.sender_key == sender_key
                && m.timestamp_ms == timestamp
                && (!is_federated || m.content == content)
        }) {
            continue;
        }
        conn.messages.push(crate::gui::ChatMessage {
            sender_name,
            sender_key,
            content,
            timestamp: crate::gui::pages::chat::format_timestamp(timestamp),
            timestamp_ms: timestamp,
            channel,
            server: conn.url.clone(),
            origin_server,
            ..Default::default()
        });
    }
    // History lands after live lines: restore chronological order for the
    // per-channel render (the vec order IS the display order).
    conn.messages.sort_by_key(|m| m.timestamp_ms);
    while conn.messages.len() > 200 {
        conn.messages.remove(0);
    }
}

/// Open a background connection to ONE saved server that has none yet.
/// One dial per identify-handshake keeps a long server list from
/// stampeding the network (and the relays' rate limits) at startup.
fn dial_missing_saved_servers(state: &mut EngineState) {
    // Exclude BOTH the connected URL and the intended one (server_url):
    // at boot the active auto-connect may not have fired yet, and dialing
    // its server here first would race it into a duplicate connection.
    let active = norm_server_url(&state.gui_state.connected_server_url);
    let intended = norm_server_url(&state.gui_state.server_url);
    let existing: std::collections::HashSet<String> =
        state.gui_state.connections.iter().map(|c| c.url.clone()).collect();
    // Dial EVERY missing server in one pass (each socket handshakes on its
    // own thread), so the sidebar settles in a single wave instead of
    // servers popping in one by one -- the operator's boot-churn report.
    // Server lists are a handful of entries; no stampede to throttle.
    let targets: Vec<String> = state
        .gui_state
        .chat_servers
        .iter()
        .map(|s| s.url.clone())
        .filter(|u| {
            let n = norm_server_url(u);
            !n.is_empty() && n != active && n != intended && !existing.contains(&n)
        })
        .collect();
    if targets.is_empty() {
        return;
    }
    let name = state.gui_state.user_name.clone();
    let pubkey = state.gui_state.profile_public_key.clone();
    let kyber = state.gui_state.kyber_public_b64.clone();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for url in targets {
        let n = norm_server_url(&url);
        if !seen.insert(n.clone()) {
            continue; // duplicate saved entry for the same server
        }
        let ws_url = crate::gui::pages::chat::derive_ws_url(&url);
        log::info!("Background connect: dialing saved server {url}");
        state.gui_state.connections.push(crate::gui::ServerConnection {
            url: n,
            display_url: url.trim().to_string(),
            ws: Some(crate::net::ws_client::WsClient::connect_with_kyber(
                &ws_url, &name, &pubkey, &kyber,
            )),
            status: "Connecting...".to_string(),
            reconnect_delay: 5.0,
            active_channel: "general".to_string(),
            ..Default::default()
        });
    }
}

/// Redial dropped background links whose backoff countdown expired.
fn redial_dropped(state: &mut EngineState) {
    let name = state.gui_state.user_name.clone();
    let pubkey = state.gui_state.profile_public_key.clone();
    let kyber = state.gui_state.kyber_public_b64.clone();
    for conn in state.gui_state.connections.iter_mut() {
        if conn.ws.is_none()
            && !conn.manually_disconnected
            && conn.reconnect_timer <= 0.0
            && !conn.url.is_empty()
        {
            let ws_url = crate::gui::pages::chat::derive_ws_url(&conn.display_url);
            log::info!(
                "Background reconnect: {} (attempt {})",
                conn.url,
                conn.reconnect_attempts + 1
            );
            conn.ws = Some(crate::net::ws_client::WsClient::connect_with_kyber(
                &ws_url, &name, &pubkey, &kyber,
            ));
            conn.identified = false;
            conn.status = "Reconnecting...".to_string();
            conn.reconnect_attempts += 1;
            conn.rate_limited = false;
            // Re-arm so a failed attempt waits out the (doubled) delay
            // instead of retrying every frame.
            conn.reconnect_timer = conn.reconnect_delay;
        }
    }
}

/// Load the per-(identity, server) DM store for a parked connection.
/// On-demand — no cached handle on the connection struct: DM events are
/// rare, the file is small, and only the ACTIVE server keeps a cached
/// store on GuiState.
fn bg_dm_store(state: &EngineState, ci: usize) -> Option<crate::net::dm_store::DmStore> {
    let seed = state.gui_state.private_key_bytes.as_ref()?;
    if state.gui_state.profile_public_key.is_empty() {
        return None;
    }
    let conn = state.gui_state.connections.get(ci)?;
    Some(crate::net::dm_store::DmStore::load(
        seed,
        &state.gui_state.profile_public_key,
        &conn.url,
    ))
}

/// Fold one verified sealed-sender DM into a parked connection's sidebar
/// entry + message buffer (so unpark restores a current picture).
fn bg_apply_dm(
    state: &mut EngineState,
    ci: usize,
    inner: &crate::net::dm_pq::DmInner,
    is_from_me: bool,
) {
    let partner = if is_from_me { inner.to.clone() } else { inner.from.clone() };
    if partner.is_empty() {
        return;
    }
    // Resolve a display name from the parked server's roster.
    let display = {
        let conn = &state.gui_state.connections[ci];
        conn.users
            .iter()
            .find(|u| u.public_key == partner)
            .map(|u| u.name.clone())
            .filter(|n| !n.is_empty() && n != "Anonymous")
            .unwrap_or_else(|| partner.chars().take(8).collect())
    };
    let sender_display = if is_from_me {
        if state.gui_state.user_name.is_empty() {
            "You".to_string()
        } else {
            state.gui_state.user_name.clone()
        }
    } else {
        display.clone()
    };
    let preview = if is_from_me {
        format!("You: {}", inner.text)
    } else {
        inner.text.clone()
    };
    let ts_str = crate::gui::pages::chat::format_timestamp(inner.ts);
    let conn = &mut state.gui_state.connections[ci];
    if let Some(d) = conn.dms.iter_mut().find(|d| d.user_key == partner) {
        d.last_message = preview;
        d.timestamp = ts_str.clone();
        if !is_from_me {
            d.unread = true;
        }
    } else {
        conn.dms.push(crate::gui::ChatDm {
            user_name: display,
            user_key: partner.clone(),
            last_message: preview,
            timestamp: ts_str.clone(),
            unread: !is_from_me,
        });
    }
    conn.messages.push(crate::gui::ChatMessage {
        sender_name: sender_display,
        sender_key: inner.from.clone(),
        content: inner.text.clone(),
        timestamp: ts_str,
        timestamp_ms: inner.ts,
        channel: format!("dm:{}", partner),
        server: conn.url.clone(),
        ..Default::default()
    });
    while conn.messages.len() > 200 {
        conn.messages.remove(0);
    }
}

/// The compact per-message router for background connections.
fn handle_bg_message(state: &mut EngineState, ci: usize, raw: &str) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) else {
        return;
    };
    if ci >= state.gui_state.connections.len() {
        return;
    }
    state.gui_state.connections[ci].msgs_in += 1;
    match val.get("type").and_then(|v| v.as_str()) {
        Some("identify_challenge") => {
            // Same signed preimage as the active connection's arm in lib.rs:
            // "hum/identify/v1\n{nonce}\n{pubkey}", Dilithium3 over the seed.
            let nonce = val.get("nonce").and_then(|v| v.as_str()).unwrap_or("");
            if nonce.is_empty() {
                return;
            }
            let Some(seed) = state.gui_state.private_key_bytes.as_ref() else {
                return;
            };
            let preimage = format!(
                "hum/identify/v1\n{}\n{}",
                nonce, state.gui_state.profile_public_key
            );
            let sig = crate::net::identity::pq_sign_raw(seed, preimage.as_bytes());
            use base64::{engine::general_purpose::STANDARD as B64, Engine};
            let response = serde_json::json!({
                "type": "identify_response",
                "sig_b64": B64.encode(&sig),
            })
            .to_string();
            if let Some(ws) = state.gui_state.connections[ci].ws.as_ref() {
                ws.send(&response);
            }
        }
        Some("peer_list") => {
            // First post-bind message: the handshake is complete.
            let users = parse_users(val.get("peers"), "display_name", None, state);
            let conn = &mut state.gui_state.connections[ci];
            conn.identified = true;
            conn.status = "Connected".to_string();
            conn.reconnect_attempts = 0;
            conn.reconnect_delay = 5.0;
            conn.rate_limited = false;
            conn.users = users;
            log::info!("Background connect: {} identified", conn.url);
        }
        Some("full_user_list") => {
            let users = parse_users(val.get("users"), "name", Some("online"), state);
            state.gui_state.connections[ci].users = users;
        }
        Some("channel_list") => {
            let Some(channels) = val.get("channels").and_then(|v| v.as_array()) else {
                return;
            };
            let conn = &mut state.gui_state.connections[ci];
            // Preserve unread marks across rebuilds, same as the active arm.
            let unread_ids: std::collections::HashSet<String> = conn
                .channels
                .iter()
                .filter(|c| c.unread)
                .map(|c| c.id.clone())
                .collect();
            conn.channels.clear();
            for ch in channels {
                let id = ch
                    .get("id")
                    .or_else(|| ch.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("general")
                    .to_string();
                conn.channels.push(crate::gui::ChatChannel {
                    name: ch.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string(),
                    description: ch
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    category: ch
                        .get("category_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Text")
                        .to_string(),
                    voice_enabled: ch.get("voice_enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    read_only: ch.get("read_only").and_then(|v| v.as_bool()).unwrap_or(false),
                    federated: ch.get("federated").and_then(|v| v.as_bool()).unwrap_or(false),
                    local_only: ch.get("local_only").and_then(|v| v.as_bool()).unwrap_or(false),
                    unread: unread_ids.contains(&id),
                    id,
                    ..Default::default()
                });
            }
            // Arm the one-time history fetch for this server's FEDERATED
            // channels, so Commons rooms have depth from every carrier
            // without the user ever visiting it. Drained one channel per
            // in-flight fetch by the pump below.
            if !conn.history_fetched {
                conn.history_fetched = true;
                conn.history_queue = conn
                    .channels
                    .iter()
                    .filter(|c| c.federated)
                    .map(|c| c.id.clone())
                    .collect();
                // Sealed-sender DMs: channel_list only arrives on a bound
                // socket, so fetch this parked server's mailbox too. The
                // high-water mark lives in the per-server local store.
                let after_id = bg_dm_store(state, ci)
                    .map(|s| s.high_water())
                    .unwrap_or(0);
                let conn = &state.gui_state.connections[ci];
                if let Some(ws) = conn.ws.as_ref() {
                    ws.send(
                        &serde_json::json!({ "type": "dm_fetch", "after_id": after_id })
                            .to_string(),
                    );
                }
            }
        }
        Some("chat") => {
            let sender_key = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let sender_name = val
                .get("from_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Anonymous")
                .to_string();
            let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
            let channel = val
                .get("channel")
                .and_then(|v| v.as_str())
                .unwrap_or("general")
                .to_string();
            if content.is_empty() {
                return;
            }
            // The open Commons view of this room counts as "on screen":
            // don't light unread for a conversation the user is watching.
            let room_open = crate::gui::pages::chat::commons_room_of(
                &state.gui_state.chat_active_channel,
            ) == Some(channel.as_str());
            let conn = &mut state.gui_state.connections[ci];
            // Our own echo from a session on this server before it was
            // parked: the sent-timestamps list rode along into the park.
            if sender_key == state.gui_state.profile_public_key
                && conn.sent_timestamps.contains(&ts)
            {
                return;
            }
            if !room_open {
                if let Some(c) = conn.channels.iter_mut().find(|c| c.id == channel) {
                    c.unread = true;
                }
            }
            conn.messages.push(crate::gui::ChatMessage {
                sender_name,
                sender_key,
                content,
                timestamp: crate::gui::pages::chat::format_timestamp(ts),
                timestamp_ms: ts,
                channel,
                server: conn.url.clone(),
                ..Default::default()
            });
            while conn.messages.len() > 200 {
                conn.messages.remove(0);
            }
        }
        Some("federated_chat") => {
            let server_id = val.get("server_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let server_name = val
                .get("server_name")
                .and_then(|v| v.as_str())
                .unwrap_or("federated")
                .to_string();
            let from_name = val.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
            let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if content.is_empty() || ts == 0 {
                return;
            }
            let room_open = crate::gui::pages::chat::commons_room_of(
                &state.gui_state.chat_active_channel,
            ) == Some(channel.as_str());
            let conn = &mut state.gui_state.connections[ci];
            // Same cross-carrier dedup key as the active arm.
            if conn.messages.iter().any(|m| {
                m.origin_server == server_id && m.timestamp_ms == ts && m.content == content
            }) {
                return;
            }
            if !room_open {
                if let Some(c) = conn.channels.iter_mut().find(|c| c.id == channel) {
                    c.unread = true;
                }
            }
            conn.messages.push(crate::gui::ChatMessage {
                sender_name: format!("{} ({})", from_name, server_name),
                sender_key: server_id.clone(),
                content,
                timestamp: crate::gui::pages::chat::format_timestamp(ts),
                timestamp_ms: ts,
                channel,
                server: conn.url.clone(),
                origin_server: server_id,
                ..Default::default()
            });
            while conn.messages.len() > 200 {
                conn.messages.remove(0);
            }
        }
        Some("dm_new") => {
            // Sealed-sender envelope on a parked server: no sender on the
            // wire — decrypt with our key, trust only the verified inner.
            let mail_id = val.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let raw_env = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if raw_env.is_empty() {
                return;
            }
            match crate::engine::dm::open_verify_dm(&raw_env, &state.gui_state) {
                Ok(inner) => {
                    let Some(mut store) = bg_dm_store(state, ci) else { return };
                    let is_new = store.insert(&inner);
                    store.set_high_water(mail_id);
                    store.save();
                    if !is_new {
                        return; // duplicate (echo of our own send / refetch)
                    }
                    let is_from_me = inner.from == state.gui_state.profile_public_key;
                    // DM ding even for a parked server: a person is a
                    // person, whichever relay carried them.
                    if !is_from_me && state.gui_state.notif_dm_enabled {
                        state
                            .pending_sfx
                            .push(("sfx.chat_message", "audio/ui/chat_message.ogg"));
                    }
                    bg_apply_dm(state, ci, &inner, is_from_me);
                }
                Err(e) => {
                    // Not ours / spoofed — drop, but keep the high-water
                    // moving so a poison envelope can't wedge fetches.
                    log::warn!("Parked-server DM envelope {mail_id} dropped: {e}");
                    if let Some(mut store) = bg_dm_store(state, ci) {
                        store.set_high_water(mail_id);
                        store.save();
                    }
                }
            }
        }
        Some("dm_batch") => {
            // Mailbox page for a parked server: fill the local store, then
            // rebuild the parked sidebar list from it.
            let Some(mut store) = bg_dm_store(state, ci) else { return };
            let mut last_id: i64 = 0;
            let mut fresh: Vec<crate::net::dm_pq::DmInner> = Vec::new();
            if let Some(msgs) = val.get("messages").and_then(|v| v.as_array()) {
                for m in msgs {
                    let id = m.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                    if id > last_id {
                        last_id = id;
                    }
                    let raw = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    if raw.is_empty() {
                        continue;
                    }
                    if let Ok(inner) = crate::engine::dm::open_verify_dm(raw, &state.gui_state) {
                        if store.insert(&inner) {
                            fresh.push(inner);
                        }
                    }
                }
            }
            store.set_high_water(last_id);
            store.save();
            let done = val.get("done").and_then(|v| v.as_bool()).unwrap_or(true);
            let me = state.gui_state.profile_public_key.clone();
            for inner in &fresh {
                let is_from_me = inner.from == me;
                bg_apply_dm(state, ci, inner, is_from_me);
            }
            if !done {
                let after_id = store.high_water();
                let conn = &state.gui_state.connections[ci];
                if let Some(ws) = conn.ws.as_ref() {
                    ws.send(
                        &serde_json::json!({ "type": "dm_fetch", "after_id": after_id })
                            .to_string(),
                    );
                }
            }
        }
        Some("name_taken") => {
            // Retrying with the same name would loop forever; stop redialing
            // and surface why. The user resolves it from the active side.
            let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("Name taken");
            let conn = &mut state.gui_state.connections[ci];
            conn.ws = None;
            conn.identified = false;
            conn.manually_disconnected = true;
            conn.status = format!("Sign-in refused: {}", msg);
            log::warn!("Background connect to {} refused: {}", conn.url, msg);
        }
        Some("system") => {
            let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("");
            if msg.contains("Too many connection attempts") {
                let conn = &mut state.gui_state.connections[ci];
                conn.rate_limited = true;
                conn.reconnect_delay = 65.0;
            }
        }
        // Admin, tasks, listings, voice, trades, game: active-server
        // concerns; their pages refetch after a switch. Dropped by design.
        _ => {}
    }
}

/// Shared roster parse for peer_list ("display_name", all online) and
/// full_user_list ("name" + an `online` flag). Also harvests Kyber keys
/// into the global peer_kyber_keys map, which is keyed by user public key
/// and therefore safely shared across servers.
fn parse_users(
    arr: Option<&serde_json::Value>,
    name_field: &str,
    online_field: Option<&str>,
    state: &mut EngineState,
) -> Vec<crate::gui::ChatUser> {
    let mut users = Vec::new();
    let Some(list) = arr.and_then(|v| v.as_array()) else {
        return users;
    };
    for user in list {
        let key = user
            .get("public_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(kyber) = user.get("kyber_public").and_then(|v| v.as_str()) {
            if !kyber.is_empty() && !key.is_empty() {
                state
                    .gui_state
                    .peer_kyber_keys
                    .insert(key.clone(), kyber.to_string());
            }
        }
        let status = match online_field {
            Some(f) if !user.get(f).and_then(|v| v.as_bool()).unwrap_or(false) => {
                "offline".to_string()
            }
            _ => user
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("online")
                .to_string(),
        };
        users.push(crate::gui::ChatUser {
            name: user
                .get(name_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Anonymous")
                .to_string(),
            public_key: key,
            role: user.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            status,
        });
    }
    users
}

#[cfg(test)]
mod tests {
    #[test]
    fn merge_history_populates_the_carrier_buffer_from_the_real_api_shape() {
        let mut conn = crate::gui::ServerConnection {
            url: "https://a.example".into(),
            display_url: "https://a.example".into(),
            ..Default::default()
        };
        // The exact serialization /api/messages returns: RelayMessage rows,
        // chat + federated_chat mixed.
        let body = r#"{"messages":[
            {"type":"chat","from":"k1","from_name":"Alice","content":"hello","timestamp":1000,"channel":"general"},
            {"type":"chat","from":"k2","from_name":"Bela","content":"hey","timestamp":1500,"channel":"general"},
            {"type":"federated_chat","server_id":"deadbeef","server_name":"peer","from_name":"Cyra","content":"hi from afar","timestamp":2000,"channel":"general"}
        ],"cursor":3}"#;
        super::merge_history_into(&mut conn, body, "");
        assert_eq!(conn.messages.len(), 3, "all rows land in the carrier buffer");
        assert!(conn.messages.iter().all(|m| m.channel == "general"));
        assert_eq!(conn.messages[2].origin_server, "deadbeef");
        assert_eq!(conn.messages[2].sender_name, "Cyra (peer)");
        // Re-merging the same body is idempotent (dedup holds).
        super::merge_history_into(&mut conn, body, "");
        assert_eq!(conn.messages.len(), 3, "second merge must not duplicate");
    }
}
