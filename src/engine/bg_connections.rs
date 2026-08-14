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

    dial_one_missing_saved_server(state);

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

    redial_dropped(state);
}

/// Open a background connection to ONE saved server that has none yet.
/// One dial per identify-handshake keeps a long server list from
/// stampeding the network (and the relays' rate limits) at startup.
fn dial_one_missing_saved_server(state: &mut EngineState) {
    if state
        .gui_state
        .connections
        .iter()
        .any(|c| c.ws.is_some() && !c.identified)
    {
        return; // a dial is still handshaking; next one waits its turn
    }
    // Exclude BOTH the connected URL and the intended one (server_url):
    // at boot the active auto-connect may not have fired yet, and dialing
    // its server here first would race it into a duplicate connection.
    let active = norm_server_url(&state.gui_state.connected_server_url);
    let intended = norm_server_url(&state.gui_state.server_url);
    let existing: std::collections::HashSet<String> =
        state.gui_state.connections.iter().map(|c| c.url.clone()).collect();
    let target = state
        .gui_state
        .chat_servers
        .iter()
        .map(|s| s.url.clone())
        .find(|u| {
            let n = norm_server_url(u);
            !n.is_empty() && n != active && n != intended && !existing.contains(&n)
        });
    let Some(url) = target else { return };
    let ws_url = crate::gui::pages::chat::derive_ws_url(&url);
    let name = state.gui_state.user_name.clone();
    let pubkey = state.gui_state.profile_public_key.clone();
    let kyber = state.gui_state.kyber_public_b64.clone();
    log::info!("Background connect: dialing saved server {url}");
    state.gui_state.connections.push(crate::gui::ServerConnection {
        url: norm_server_url(&url),
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
                    unread: unread_ids.contains(&id),
                    id,
                    ..Default::default()
                });
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
            let conn = &mut state.gui_state.connections[ci];
            // Our own echo from a session on this server before it was
            // parked: the sent-timestamps list rode along into the park.
            if sender_key == state.gui_state.profile_public_key
                && conn.sent_timestamps.contains(&ts)
            {
                return;
            }
            if let Some(c) = conn.channels.iter_mut().find(|c| c.id == channel) {
                c.unread = true; // nothing on a parked server is on screen
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
            let conn = &mut state.gui_state.connections[ci];
            // Same cross-carrier dedup key as the active arm.
            if conn.messages.iter().any(|m| {
                m.origin_server == server_id && m.timestamp_ms == ts && m.content == content
            }) {
                return;
            }
            if let Some(c) = conn.channels.iter_mut().find(|c| c.id == channel) {
                c.unread = true;
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
        Some("dm") => {
            let from_key = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let from_name = val
                .get("from_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Anonymous")
                .to_string();
            let raw_content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
            let encrypted = val.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false);
            let nonce = val.get("nonce").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let is_from_me = from_key == state.gui_state.profile_public_key
                || (!state.gui_state.user_name.is_empty()
                    && from_name == state.gui_state.user_name);
            let partner = if is_from_me {
                val.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string()
            } else {
                from_key.clone()
            };
            if partner.is_empty() {
                return;
            }
            // Decrypt BEFORE touching the connection: the helper borrows the
            // whole GuiState immutably.
            let content = crate::engine::dm::decrypt_dm_if_encrypted(
                &raw_content,
                encrypted,
                &nonce,
                &partner,
                &state.gui_state,
            );
            // DM ding even for a parked server: a person is a person,
            // whichever relay carried them.
            if !is_from_me && state.gui_state.notif_dm_enabled {
                state
                    .pending_sfx
                    .push(("sfx.chat_message", "audio/ui/chat_message.ogg"));
            }
            let preview = if is_from_me {
                format!("You: {}", content)
            } else {
                content.clone()
            };
            let ts_str = crate::gui::pages::chat::format_timestamp(ts);
            let conn = &mut state.gui_state.connections[ci];
            if let Some(d) = conn.dms.iter_mut().find(|d| d.user_key == partner) {
                d.last_message = preview;
                d.timestamp = ts_str.clone();
                if !is_from_me {
                    d.unread = true;
                }
            } else {
                let display = if is_from_me {
                    partner.chars().take(8).collect::<String>()
                } else {
                    from_name.clone()
                };
                conn.dms.push(crate::gui::ChatDm {
                    user_name: display,
                    user_key: partner.clone(),
                    last_message: preview,
                    timestamp: ts_str.clone(),
                    unread: !is_from_me,
                });
            }
            conn.messages.push(crate::gui::ChatMessage {
                sender_name: from_name,
                sender_key: from_key,
                content,
                timestamp: ts_str,
                timestamp_ms: ts,
                channel: format!("dm:{}", partner),
                server: conn.url.clone(),
                ..Default::default()
            });
            while conn.messages.len() > 200 {
                conn.messages.remove(0);
            }
        }
        Some("dm_list") => {
            let Some(conversations) = val.get("conversations").and_then(|v| v.as_array()) else {
                return;
            };
            let conn = &mut state.gui_state.connections[ci];
            conn.dms.clear();
            for conv in conversations {
                conn.dms.push(crate::gui::ChatDm {
                    user_name: conv
                        .get("partner_name")
                        .or_else(|| conv.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    user_key: conv
                        .get("partner_key")
                        .or_else(|| conv.get("key"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    last_message: conv
                        .get("last_message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    timestamp: conv
                        .get("timestamp")
                        .and_then(|v| v.as_u64())
                        .map(crate::gui::pages::chat::format_timestamp)
                        .unwrap_or_default(),
                    unread: conv.get("unread").and_then(|v| v.as_bool()).unwrap_or(false),
                });
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
