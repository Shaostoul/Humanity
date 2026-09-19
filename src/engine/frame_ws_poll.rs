//! The relay WebSocket MESSAGE PUMP: one frame's worth of "read everything the
//! relay sent us and act on it", plus the teardown that arms the reconnect
//! timer when the socket has died (extracted verbatim from lib.rs, v0.1320).
//!
//! WHY IT IS A MODULE and not still inline in the frame loop: at 1,667 lines
//! it was by a wide margin the largest single thing in `window_event`, and it
//! is ONE job with ONE input. Everything inside is a `match` arm on the
//! relay's `type` field -- chat, DMs, presence, voice rooms, moderation,
//! game-state sync, backups -- so it grows every time the protocol does, and
//! it was growing inside the file that every other feature also has to edit.
//! `src/lib.rs` is the repo's merge funnel (see CLAUDE.md): the fewer reasons
//! there are to open it, the more sessions can work in parallel without
//! three-way merges over the frame loop.
//!
//! WHY IT TAKES `&mut EngineState` and not a list of named borrows (the
//! `ensure_near_tree_models` shape): this block genuinely touches the whole
//! app -- the GUI state, the game world, the audio mixer, the renderer, the
//! save path -- because a relay message can change any of them. Naming the
//! borrows here would be a list of two dozen fields that documents nothing.
//! The near-tree helpers take named borrows for a concrete reason (the caller
//! is holding a disjoint borrow of `planet_chunk_states` at the call site);
//! this one has no such constraint, so the honest signature is the whole
//! state.
//!
//! BEHAVIOUR: none. The body below is the previous inline block byte for byte
//! apart from its indentation, and the call site runs it at exactly the same
//! point in the frame, between the auto-connect attempt and the reconnect
//! backoff countdown. The one structural change is that `ws_dropped` -- which
//! was a frame local shared by the pump and the teardown -- is now a local of
//! this function, because both halves moved together.

use crate::engine::state::EngineState;
// The two sibling helpers this pump calls by bare name. In lib.rs they arrived
// through `mod native_app`'s glob imports of `crate::engine::*`; here they are
// named, which also documents that the pump is the only caller of either.
use crate::engine::ipc_parse::parse_notification_prefs;
use crate::engine::net_route::route_game_message;
use crate::gui::GuiPage;

/// Drain the relay socket for this frame and apply every message to the app.
///
/// Also owns the "socket died" teardown: dropping the client, tearing down the
/// WebRTC manager that rides its signaling, re-arming the on-demand admin
/// panels, and starting the reconnect countdown. The countdown itself still
/// ticks in the frame loop (it needs `dt`).
pub(crate) fn poll_relay_messages(state: &mut EngineState) {
    // ── Poll WebSocket messages from relay server ──
    let mut ws_dropped = false;
    if let Some(ref mut ws) = state.gui_state.ws_client {
        let messages = ws.poll_messages();
        // is_dropped, NOT !is_connected: a fresh spawn is
        // CONNECTING (neither), and tearing it down here used
        // to be impossible only because is_connected lied
        // optimistically (see LinkState in ws_client.rs).
        if ws.is_dropped() {
            if !ws_dropped {
                crate::debug::push_debug("WS connection lost");
            }
            ws_dropped = true;
        }
        for raw in messages {
            // Network overlay (v0.482): count every received frame.
            state.gui_state.ws_msgs_in = state.gui_state.ws_msgs_in.saturating_add(1);
            // Log raw message to debug console (truncate long messages)
            {
                let preview = if raw.len() > 300 { format!("{}...", &raw[..300]) } else { raw.clone() };
                crate::debug::push_debug(format!("WS <<< {}", preview));
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                let msg_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                log::debug!("WS recv: type={}", msg_type);
                match val.get("type").and_then(|t| t.as_str()) {
                    Some("identify_challenge") => {
                        // Inc3b — relay challenged us after `identify`.
                        // Sign the canonical preimage with Dilithium3 from
                        // our BIP39 seed and return `identify_response`.
                        // Closes HIGH-2 (identity spoofing at identify).
                        let nonce = val.get("nonce").and_then(|v| v.as_str()).unwrap_or("");
                        if nonce.is_empty() {
                            log::warn!("identify_challenge missing nonce");
                        } else if let Some(ref seed) = state.gui_state.private_key_bytes {
                            let preimage = format!(
                                "hum/identify/v1\n{}\n{}",
                                nonce, state.gui_state.profile_public_key
                            );
                            let sig = crate::net::identity::pq_sign_raw(seed, preimage.as_bytes());
                            use base64::{engine::general_purpose::STANDARD as B64, Engine};
                            let sig_b64 = B64.encode(&sig);
                            let response = serde_json::json!({
                                "type": "identify_response",
                                "sig_b64": sig_b64,
                            });
                            if let Some(ref ws_client) = state.gui_state.ws_client {
                                ws_client.send(&response.to_string());
                            }
                        } else {
                            log::error!("identify_challenge received but seed not unlocked — cannot sign. Unlock identity to connect.");
                            state.gui_state.ws_status = "Unlock your identity to complete connect (server requested challenge).".to_string();
                        }
                    }
                    Some("chat") => {
                        let sender_key = val.get("from")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let msg_timestamp = val.get("timestamp")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        // Skip only messages WE sent from THIS client
                        // (already added locally). Check by matching
                        // our key + exact timestamp in recent sent list.
                        if sender_key == state.gui_state.profile_public_key
                            && state.gui_state.chat_sent_timestamps.contains(&msg_timestamp)
                        {
                            state.gui_state.chat_sent_timestamps.retain(|&t| t != msg_timestamp);
                            continue;
                        }
                        let sender_name = val.get("from_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Anonymous")
                            .to_string();
                        let content = val.get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let timestamp = val.get("timestamp")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let channel = val.get("channel")
                            .and_then(|v| v.as_str())
                            .unwrap_or("general")
                            .to_string();
                        // Robust dedup (fixes the 2026-05-20 "reply duplicated
                        // after a while" report). The chat_sent_timestamps
                        // fast-path above only catches the FIRST echo of our
                        // OWN sends — it removes the entry on match, so it's
                        // one-shot. A LATER replay of the same message (the WS
                        // reconnect history re-fetch resets history_fetched and
                        // re-pulls the last 50; or a duplicate broadcast) would
                        // otherwise sail past the consumed fast-path and append
                        // a second copy that only clears on app restart (in-
                        // memory only; the relay always had one copy). A message
                        // is uniquely identified by (sender_key, timestamp_ms)
                        // — ms precision, per-sender — so skip if we already
                        // hold it. Cheap: chat_messages is bounded to 200.
                        if state.gui_state.chat_messages.iter()
                            .any(|m| m.sender_key == sender_key && m.timestamp_ms == timestamp)
                        {
                            continue;
                        }
                        // Decode reply_to context if present (threads).
                        let reply_to = val.get("reply_to").and_then(|r| {
                            let from = r.get("from")?.as_str()?.to_string();
                            let from_name = r.get("from_name")?.as_str()?.to_string();
                            let preview = r.get("content")?.as_str()?.to_string();
                            let ts = r.get("timestamp")?.as_u64()?;
                            Some(crate::gui::ReplyContext {
                                sender_key: from,
                                sender_name: from_name,
                                preview,
                                timestamp_ms: ts,
                            })
                        });
                        // Sidebar unread dot: flag the channel when the message
                        // isn't ours and that channel isn't the open one. The
                        // open Commons view of the same room counts as open
                        // (its qualified id is "commons:<room>"). (v0.718)
                        let room_is_open = channel == state.gui_state.chat_active_channel
                            || crate::gui::pages::chat::commons_room_of(
                                &state.gui_state.chat_active_channel,
                            ) == Some(channel.as_str());
                        if sender_key != state.gui_state.profile_public_key
                            && !room_is_open
                        {
                            if let Some(c) = state.gui_state.chat_channels.iter_mut().find(|c| c.id == channel) {
                                c.unread = true;
                            }
                        }
                        // Mention ding (v0.987): someone ELSE names us in a
                        // live channel message. Case-insensitive contains on
                        // the display name; this handler only sees live WS
                        // broadcasts (history arrives via REST), so old
                        // mentions never re-ding.
                        if sender_key != state.gui_state.profile_public_key
                            && state.gui_state.notif_mentions_enabled
                            && !state.gui_state.user_name.is_empty()
                            && content
                                .to_lowercase()
                                .contains(&state.gui_state.user_name.to_lowercase())
                        {
                            state.pending_sfx.push((
                                "sfx.chat_message",
                                "audio/ui/chat_message.ogg",
                            ));
                        }
                        state.gui_state.chat_messages.push(
                            crate::gui::ChatMessage {
                                sender_name,
                                sender_key,
                                content,
                                timestamp: crate::gui::pages::chat::format_timestamp(timestamp),
                                timestamp_ms: timestamp,
                                channel,
                                reply_to,
                                server: crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url),
                                ..Default::default()
                            },
                        );
                        // Bound message buffer
                        while state.gui_state.chat_messages.len() > 200 {
                            state.gui_state.chat_messages.remove(0);
                        }
                    }
                    Some("peer_list") => {
                        let peer_count = val.get("peers").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                        log::info!("peer_list received: {} peers", peer_count);
                        crate::debug::push_debug(format!("Identified OK, {} peers online", peer_count));
                        state.gui_state.chat_users.clear();
                        state.gui_state.ws_status = "Connected".to_string();
                        state.gui_state.server_connected = true;
                        // First post-bind message: the identify handshake is
                        // complete, game messages will now be routed (v0.794).
                        state.gui_state.ws_identified = true;
                        // Fetch this server's real donation info (GET /api/server-info's
                        // `funding` field) so the Donate page shows the connected server's
                        // actual addresses instead of relying on the user (or a self-hosting
                        // operator) having hand-typed them into Settings -- native previously
                        // never fetched this at all (only the web client did). Donation info
                        // is money-routing data, so (adversarial review 2026-07-01): funding
                        // carried over from a DIFFERENT server is cleared immediately, and an
                        // in-flight fetch aimed at a different server is replaced (dropping
                        // its receiver -- the late send fails harmlessly) rather than left to
                        // land its result under the wrong server.
                        {
                            let url = state.gui_state.server_url.trim_end_matches('/').to_string();
                            if state.gui_state.donate_funding_server != url {
                                state.gui_state.donate_funding_server.clear();
                                state.gui_state.donate_addresses_server.clear();
                                state.gui_state.donate_funding_goal = None;
                            }
                            let already_fetching = state
                                .gui_state
                                .donate_info_rx
                                .as_ref()
                                .map_or(false, |(u, _)| *u == url);
                            if !already_fetching {
                                let api = format!("{}/api/server-info", url);
                                let (tx, rx) = std::sync::mpsc::channel();
                                std::thread::spawn(move || {
                                    let result = (|| -> Result<crate::gui::ServerInfo, String> {
                                        let resp = ureq::get(&api)
                                            .timeout(std::time::Duration::from_secs(10))
                                            .call()
                                            .map_err(|e| e.to_string())?;
                                        let body = resp.into_string().map_err(|e| e.to_string())?;
                                        serde_json::from_str::<crate::gui::ServerInfo>(&body).map_err(|e| e.to_string())
                                    })();
                                    let _ = tx.send(result);
                                });
                                state.gui_state.donate_info_rx = Some((url, rx));
                            }
                        }
                        // Request tasks from server on connect
                        if let Some(ref ws_client) = state.gui_state.ws_client {
                            let get_tasks = serde_json::json!({"type": "task_list"});
                            ws_client.send(&get_tasks.to_string());
                        }
                        if let Some(peers) = val.get("peers").and_then(|v| v.as_array()) {
                            for peer in peers {
                                let name = peer.get("display_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Anonymous")
                                    .to_string();
                                let key = peer.get("public_key")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let role = peer.get("role")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let status = peer.get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("online")
                                    .to_string();
                                // Capture peer's Kyber768 public key for full-PQ DM sealing
                                if let Some(kyber) = peer.get("kyber_public").and_then(|v| v.as_str()) {
                                    if !kyber.is_empty() && !key.is_empty() {
                                        state.gui_state.peer_kyber_keys.insert(key.clone(), kyber.to_string());
                                    }
                                }
                                // If this peer is us and our local name is empty, adopt the server's display_name
                                if key == state.gui_state.profile_public_key
                                    && state.gui_state.user_name.is_empty()
                                    && name != "Anonymous"
                                {
                                    log::info!("Adopting display name from server: {}", name);
                                    state.gui_state.user_name = name.clone();
                                    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
                                }
                                state.gui_state.chat_users.push(
                                    crate::gui::ChatUser { name, public_key: key, role, status },
                                );
                            }
                        }
                    }
                    Some("peer_joined") => {
                        let name = val.get("display_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Anonymous")
                            .to_string();
                        let key = val.get("public_key")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let role = val.get("role")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Capture peer's Kyber768 public key for full-PQ DMs
                        if let Some(kyber) = val.get("kyber_public").and_then(|v| v.as_str()) {
                            if !kyber.is_empty() && !key.is_empty() {
                                state.gui_state.peer_kyber_keys.insert(key.clone(), kyber.to_string());
                            }
                        }
                        // Add if not already present
                        if !state.gui_state.chat_users.iter().any(|u| u.public_key == key) {
                            state.gui_state.chat_users.push(
                                crate::gui::ChatUser { name, public_key: key.clone(), role, status: "online".into() },
                            );
                        }
                    }
                    Some("peer_left") => {
                        if let Some(key) = val.get("public_key").and_then(|v| v.as_str()) {
                            state.gui_state.chat_users.retain(|u| u.public_key != key);
                        }
                    }
                    Some("channel_list") => {
                        if let Some(channels) = val.get("channels").and_then(|v| v.as_array()) {
                            // Preserve unread marks across rebuilds — channel_list
                            // re-arrives on any channel admin change and would
                            // otherwise clear every dot. (v0.718)
                            let unread_ids: std::collections::HashSet<String> = state
                                .gui_state
                                .chat_channels
                                .iter()
                                .filter(|c| c.unread)
                                .map(|c| c.id.clone())
                                .collect();
                            state.gui_state.chat_channels.clear();
                            for ch in channels {
                                let id = ch.get("id")
                                    .or_else(|| ch.get("name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("general")
                                    .to_string();
                                let name = ch.get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&id)
                                    .to_string();
                                let description = ch.get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let category = ch.get("category_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Text")
                                    .to_string();
                                // Read the persisted flags from the server so admin
                                // toggles in Server Settings → Channels survive
                                // restarts. Pre-v0.192 servers omit voice_enabled
                                // entirely; treat missing-field as true so old
                                // servers don't accidentally disable voice.
                                let voice_enabled = ch.get("voice_enabled")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true);
                                let read_only = ch.get("read_only")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let federated = ch.get("federated")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let local_only = ch.get("local_only")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let unread = unread_ids.contains(&id);
                                state.gui_state.chat_channels.push(
                                    crate::gui::ChatChannel {
                                        id,
                                        name,
                                        description,
                                        category,
                                        voice_joined: false,
                                        voice_enabled,
                                        read_only,
                                        federated,
                                        local_only,
                                        voice_participants: Vec::new(),
                                        unread,
                                    },
                                );
                            }
                        }
                        // Sealed-sender DMs: channel_list only arrives on a
                        // BOUND socket (post identify-challenge), so this is
                        // the reliable moment to fetch our mailbox. Once per
                        // connection; the high-water mark lives in the local
                        // encrypted store.
                        if !state.gui_state.dm_fetch_sent {
                            crate::engine::dm::ensure_dm_store(&mut state.gui_state);
                            let after_id = state
                                .gui_state
                                .dm_store
                                .as_ref()
                                .map(|s| s.high_water())
                                .unwrap_or(0);
                            if let Some(ref client) = state.gui_state.ws_client {
                                if client.is_connected() {
                                    client.send(&serde_json::json!({
                                        "type": "dm_fetch",
                                        "after_id": after_id,
                                    }).to_string());
                                    state.gui_state.dm_fetch_sent = true;
                                }
                            }
                            // Show whatever local history we already have
                            // while the fetch round-trips, and restore the
                            // client-side social badges (follows removal
                            // 2026-08-24: the store is the only source).
                            crate::engine::dm::rebuild_dm_sidebar(&mut state.gui_state);
                            crate::engine::dm::refresh_social_mirrors(&mut state.gui_state);
                            // Re-assert the chosen presence flag on every
                            // fresh bound socket: server_members flags are
                            // per-server, so a new/wiped server learns the
                            // user's choice immediately. (2026-08-23)
                            if !state.gui_state.settings.privacy_tier.is_empty() {
                                crate::gui::pages::privacy::send_presence_flag(&state.gui_state);
                            }
                        }
                    }
                    Some("server_settings_state") => {
                        // v0.200.0: relay broadcasts current server-wide
                        // settings (per-role char limits, sharing toggles,
                        // etc.). Cache them so the admin UI can show
                        // current values + non-admin clients know what
                        // limits apply to their messages.
                        if let Some(s) = val.get("settings") {
                            match serde_json::from_value::<crate::relay::storage::ServerSettings>(s.clone()) {
                                Ok(settings) => {
                                    log::info!(
                                        "Server settings updated (max_chars: u={} v={} m={} a={})",
                                        settings.max_chars_unverified, settings.max_chars_verified,
                                        settings.max_chars_mod, settings.max_chars_admin
                                    );
                                    state.gui_state.server_settings = Some(settings);
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse server_settings_state: {e}");
                                }
                            }
                        }
                    }
                    Some("role_list") => {
                        // v0.241 (roles Phase R2): relay sends the full
                        // role list on connect + after any role change.
                        // Cache it for the user-modal role dropdown +
                        // badge colors.
                        if let Some(arr) = val.get("roles") {
                            match serde_json::from_value::<Vec<crate::relay::storage::RoleDef>>(arr.clone()) {
                                Ok(roles) => {
                                    log::info!("Received {} role definitions", roles.len());
                                    state.gui_state.chat_roles = roles;
                                }
                                Err(e) => log::warn!("Failed to parse role_list: {e}"),
                            }
                        }
                    }
                    Some("service_state") => {
                        // v0.262.16 (Server→Services): admin-only reply
                        // to service_control. Caches the daemon/soft
                        // snapshot for the Services panel.
                        if let Some(arr) = val.get("services") {
                            match serde_json::from_value::<Vec<crate::relay::services::ServiceInfo>>(arr.clone()) {
                                Ok(svcs) => {
                                    log::info!("Received {} service states", svcs.len());
                                    state.gui_state.service_state = svcs;
                                }
                                Err(e) => log::warn!("Failed to parse service_state: {e}"),
                            }
                        }
                    }
                    Some("backup_list") => {
                        // v0.938: admin-targeted reply to
                        // backup_list_request / backup_run. Drives the
                        // Server Settings -> Backups panel.
                        if let Some(arr) = val.get("backups") {
                            match serde_json::from_value::<Vec<crate::relay::storage::backups::BackupEntry>>(arr.clone()) {
                                Ok(backups) => {
                                    log::info!("Received {} backups", backups.len());
                                    state.gui_state.backup_list = backups;
                                }
                                Err(e) => log::warn!("Failed to parse backup_list: {e}"),
                            }
                        }
                    }
                    Some("banned_list") => {
                        // v0.245: relay sends this only to admins, in
                        // reply to banned_list_request + after any
                        // ban/unban. Drives the Server Settings →
                        // Banned users panel.
                        if let Some(arr) = val.get("users") {
                            match serde_json::from_value::<Vec<crate::relay::storage::BannedUser>>(arr.clone()) {
                                Ok(users) => {
                                    log::info!("Received {} banned users", users.len());
                                    state.gui_state.chat_banned_users = users;
                                }
                                Err(e) => log::warn!("Failed to parse banned_list: {e}"),
                            }
                        }
                    }
                    Some("muted_list") => {
                        // v0.246: relay sends this only to mods/admins,
                        // in reply to muted_list_request + after any
                        // mute/unmute. Drives the Server Settings →
                        // Muted users panel.
                        if let Some(arr) = val.get("users") {
                            match serde_json::from_value::<Vec<crate::relay::storage::MutedUser>>(arr.clone()) {
                                Ok(users) => {
                                    log::info!("Received {} muted users", users.len());
                                    state.gui_state.chat_muted_users = users;
                                }
                                Err(e) => log::warn!("Failed to parse muted_list: {e}"),
                            }
                        }
                    }
                    Some("system") => {
                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                            log::info!("Relay system message: {}", msg);
                            crate::debug::push_debug(format!("System: {}", msg));
                            // Relay throttled this connection (per-IP identify rate
                            // limit). Mirror the web client: back the reconnect off
                            // PAST the 60s window. Without this the native looped every
                            // 5s -- the backoff reset fires when the WS OPENS, before
                            // the identify that gets rate-limited, so it never grew.
                            // The flag stops that reset clobbering this delay until we
                            // next actually retry. (v0.544)
                            if msg.starts_with("Too many connection attempts")
                                || msg.contains("Try again in a minute")
                            {
                                state.gui_state.ws_rate_limited = true;
                                state.gui_state.ws_reconnect_delay = 65.0;
                            }
                            // Filter out internal sync + game messages from chat display.
                            // `__game__:` prefix tags game-engine traffic (ambient
                            // chatter, quest events, NPC dialog, world ticks) — those
                            // belong on the game/perception channel, NOT in #general
                            // where humans are talking. (Bug fix 2026-05-03.)
                            // Multiplayer (v0.472): route game traffic into the sync
                            // system (remote players join / move / leave) instead of
                            // discarding it, then skip the chat display.
                            if let Some(payload) = msg.strip_prefix("__game__:") {
                                let payload = payload.to_string();
                                route_game_message(state, &payload);
                                continue;
                            }
                            if msg.starts_with("__sync_data__")
                                || msg == "sync_ack"
                            {
                                continue;
                            }
                            // Add as a system message in current channel
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            state.gui_state.chat_messages.push(
                                crate::gui::ChatMessage {
                                    sender_name: "System".to_string(),
                                    sender_key: String::new(),
                                    content: msg.to_string(),
                                    timestamp: crate::gui::pages::chat::format_timestamp(now_ms),
                                    timestamp_ms: now_ms,
                                    // Don't leak into an open P2P group / DM (it'd vanish on reload).
                                    channel: crate::gui::pages::chat::notice_channel(&state.gui_state.chat_active_channel),
                                    server: crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                    Some("full_user_list") => {
                        let user_count = val.get("users").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                        log::info!("full_user_list received: {} users", user_count);
                        // Full user list includes online + offline users
                        if let Some(users) = val.get("users").and_then(|v| v.as_array()) {
                            state.gui_state.chat_users.clear();
                            for user in users {
                                let name = user.get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Anonymous")
                                    .to_string();
                                let key = user.get("public_key")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let role = user.get("role")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let online = user.get("online")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let status = if online {
                                    user.get("status")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("online")
                                        .to_string()
                                } else {
                                    "offline".to_string()
                                };
                                // Capture peer's Kyber768 public key for full-PQ DMs
                                if let Some(kyber) = user.get("kyber_public").and_then(|v| v.as_str()) {
                                    if !kyber.is_empty() && !key.is_empty() {
                                        state.gui_state.peer_kyber_keys.insert(key.clone(), kyber.to_string());
                                    }
                                }
                                state.gui_state.chat_users.push(
                                    crate::gui::ChatUser { name, public_key: key, role, status },
                                );
                            }
                            log::info!("Received full user list: {} users", state.gui_state.chat_users.len());
                            // Re-derive the friends mirror now that names/keys
                            // are known (follows removal 2026-08-24: the local
                            // store holds the sets; chat_users supplies rows).
                            crate::engine::dm::refresh_social_mirrors(&mut state.gui_state);
                        }
                    }
                    Some("voice_channel_list") => {
                        // Voice channels received from server
                        if let Some(channels) = val.get("channels").and_then(|v| v.as_array()) {
                            log::info!("Received {} voice channels", channels.len());
                            // Add voice channels that don't already exist as text channels
                            for vc in channels {
                                // Voice is per-channel (v0.493): the entry's id IS a
                                // text channel's string id, carrying that channel's
                                // live voice roster.
                                let vc_id = vc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let vc_name = vc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                if vc_id.is_empty() { continue; }
                                // Live voice roster (public_key, display_name), v0.481.
                                let roster: Vec<(String, String)> = vc.get("participants")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| arr.iter().filter_map(|p| {
                                        let k = p.get("public_key").and_then(|v| v.as_str())?.to_string();
                                        let n = p.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        Some((k, n))
                                    }).collect())
                                    .unwrap_or_default();
                                // Attach the roster onto the matching text channel by id
                                // (channel_list already created it). Fallback: stub it.
                                if let Some(c) = state.gui_state.chat_channels.iter_mut()
                                    .find(|c| c.id == vc_id)
                                {
                                    c.voice_enabled = true;
                                    c.voice_participants = roster;
                                } else if !vc_name.is_empty() {
                                    state.gui_state.chat_channels.push(
                                        crate::gui::ChatChannel {
                                            id: vc_id,
                                            name: vc_name,
                                            description: String::new(),
                                            category: "Text".to_string(),
                                            voice_joined: false,
                                            voice_enabled: true,
                                            read_only: false,
                                            federated: false,
                                            local_only: false,
                                            voice_participants: roster,
                                            unread: false,
                                        },
                                    );
                                }
                            }
                            // Phase C (v0.492): if we are joined to a
                            // voice room, dial the incumbents present in
                            // our first post-join roster (the web's
                            // "newcomer offers, incumbents wait" rule).
                            // Later joiners will offer to us instead.
                            if let Some(room_id) = state.gui_state.voice_active_room.clone() {
                                if !state.gui_state.voice_incumbents_captured {
                                    let my_key = state.gui_state.profile_public_key.clone();
                                    let mut me_present = false;
                                    let mut incumbents: Vec<String> = Vec::new();
                                    for vc in channels {
                                        let id = vc.get("id").and_then(|v| v.as_str());
                                        if id != Some(room_id.as_str()) { continue; }
                                        if let Some(arr) = vc.get("participants").and_then(|v| v.as_array()) {
                                            for p in arr {
                                                if let Some(k) = p.get("public_key").and_then(|v| v.as_str()) {
                                                    if k == my_key { me_present = true; }
                                                    else { incumbents.push(k.to_string()); }
                                                }
                                            }
                                        }
                                    }
                                    // Only act on the roster that lists US (post-join),
                                    // so we capture the correct incumbent set.
                                    if me_present {
                                        state.gui_state.voice_incumbents_captured = true;
                                        crate::debug::push_debug(format!(
                                            "Voice: in room {}, dialing {} incumbent(s)", room_id, incumbents.len()
                                        ));
                                        if let Some(ref webrtc) = state.gui_state.webrtc {
                                            for peer in &incumbents {
                                                webrtc.offer_to_voice(peer.clone(), room_id.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some("profile_data") => {
                        // Our own profile data from the server
                        if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                            state.gui_state.profile_name = name.to_string();
                        }
                        if let Some(bio) = val.get("bio").and_then(|v| v.as_str()) {
                            state.gui_state.profile_bio = bio.to_string();
                        }
                        if let Some(avatar) = val.get("avatar_url").and_then(|v| v.as_str()) {
                            state.gui_state.profile_network_avatar = avatar.to_string();
                        }
                        log::info!("Received profile data from server");
                    }
                    Some("sync_data") | Some("sync_ack") | Some("vault_sync") => {
                        // Vault sync messages - handle silently, don't display in chat
                        crate::debug::push_debug("Vault sync message received (hidden)");
                    }
                    // (follow_list/follow_update arms removed 2026-08-24:
                    // the social graph is client-side; sealed control
                    // messages + the local store feed the badge sets.)
                    Some("account_export_data") => {
                        // Sovereignty (2026-08-23): everything the
                        // server stores about us, written to a local
                        // JSON file the user can keep or inspect.
                        if let Some(data) = val.get("data") {
                            let dir = if let Ok(appdata) = std::env::var("APPDATA") {
                                std::path::PathBuf::from(appdata).join("HumanityOS").join("exports")
                            } else {
                                std::path::PathBuf::from("exports")
                            };
                            let _ = std::fs::create_dir_all(&dir);
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let path = dir.join(format!("account-export-{ts}.json"));
                            match std::fs::write(&path, serde_json::to_string_pretty(data).unwrap_or_default()) {
                                Ok(()) => {
                                    state.gui_state.account_export_status =
                                        format!("Export saved: {}", path.display());
                                    log::info!("Account export written to {}", path.display());
                                }
                                Err(e) => {
                                    state.gui_state.account_export_status =
                                        format!("Export failed to write: {e}");
                                }
                            }
                        }
                    }
                    Some("dm_purged") => {
                        // Confirmation of our own mailbox scrub
                        // ("Delete my server mailbox" in DM Settings).
                        let count = val.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                        state.gui_state.ws_status = format!(
                            "Server mailbox cleared ({count} envelope{} deleted).",
                            if count == 1 { "" } else { "s" }
                        );
                        log::info!("DM mailbox purged: {count} envelopes deleted server-side");
                    }
                    // (legacy group_list arm removed 2026-08-23; groups are P2P E2EE)
                    Some("notification_prefs_data") => {
                        // See GuiState::notif_dm_enabled doc comment: the relay + web
                        // client already fully support this, the native client just
                        // never asked for or read it until now.
                        let prefs = parse_notification_prefs(&val);
                        state.gui_state.notif_dm_enabled = prefs.dm;
                        state.gui_state.notif_mentions_enabled = prefs.mentions;
                        state.gui_state.notif_tasks_enabled = prefs.tasks;
                        state.gui_state.notif_dnd_start = prefs.dnd_start;
                        state.gui_state.notif_dnd_end = prefs.dnd_end;
                        state.gui_state.notif_prefs_loaded = true;
                    }
                    Some("dm_new") => {
                        // Sealed-sender envelope, live-delivered. The wire
                        // carries NO sender — decrypt with our own key and
                        // trust only the Dilithium-verified inner payload.
                        let mail_id = val.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                        let raw_content = val.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        if !raw_content.is_empty() { match crate::engine::dm::open_verify_dm(raw_content, &state.gui_state) {
                            Ok(inner) => {
                                let is_from_me = inner.from == state.gui_state.profile_public_key;
                                let peer = if is_from_me { inner.to.clone() } else { inner.from.clone() };
                                let dm_is_open = state.gui_state.chat_active_channel == format!("dm:{peer}");
                                let was_new = crate::engine::dm::ingest_dm(&mut state.gui_state, &inner);
                                if let Some(store) = state.gui_state.dm_store.as_mut() {
                                    store.set_high_water(mail_id);
                                    store.save();
                                }
                                // DM notify ding (v0.985): someone ELSE's
                                // message, conversation not on screen, DM
                                // notifications enabled.
                                if was_new && !is_from_me && !dm_is_open
                                    && state.gui_state.notif_dm_enabled
                                {
                                    state.pending_sfx.push((
                                        "sfx.chat_message",
                                        "audio/ui/chat_message.ogg",
                                    ));
                                }
                            }
                            Err(e) => {
                                // Not ours / tampered / spoofed sender —
                                // never rendered, but still advance the
                                // high-water so a poison envelope can't
                                // wedge every future fetch at its id.
                                log::warn!("DM envelope {mail_id} dropped: {e}");
                                if let Some(store) = state.gui_state.dm_store.as_mut() {
                                    store.set_high_water(mail_id);
                                    store.save();
                                }
                            }
                        } }
                    }
                    Some("dm_batch") => {
                        // A page of our sealed mailbox (reply to dm_fetch).
                        // Decrypt + verify each envelope into the local
                        // store; page again until the server says done.
                        let mut last_id: i64 = 0;
                        let mut ingested = 0usize;
                        let mut dropped = 0usize;
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
                                match crate::engine::dm::open_verify_dm(raw, &state.gui_state) {
                                    Ok(inner) => {
                                        if crate::engine::dm::ingest_dm(&mut state.gui_state, &inner) {
                                            ingested += 1;
                                        }
                                    }
                                    Err(_) => {
                                        // Not ours / tampered — skip, but the
                                        // high-water still advances past it.
                                        dropped += 1;
                                    }
                                }
                            }
                        }
                        let done = val.get("done").and_then(|v| v.as_bool()).unwrap_or(true);
                        crate::engine::dm::ensure_dm_store(&mut state.gui_state);
                        if let Some(store) = state.gui_state.dm_store.as_mut() {
                            store.set_high_water(last_id);
                            store.save();
                        }
                        crate::engine::dm::rebuild_dm_sidebar(&mut state.gui_state);
                        // If a DM conversation is on screen, refresh it from
                        // the store so fetched history appears in place.
                        let active = state.gui_state.chat_active_channel.clone();
                        if let Some(peer) = active.strip_prefix("dm:") {
                            crate::engine::dm::reload_dm_channel(&mut state.gui_state, peer);
                        }
                        if ingested > 0 || dropped > 0 {
                            log::info!("DM batch: {ingested} new message(s), {dropped} undecryptable envelope(s) skipped");
                        }
                        if !done {
                            let after_id = state.gui_state.dm_store.as_ref().map(|s| s.high_water()).unwrap_or(last_id);
                            if let Some(ref client) = state.gui_state.ws_client {
                                if client.is_connected() {
                                    client.send(&serde_json::json!({
                                        "type": "dm_fetch",
                                        "after_id": after_id,
                                    }).to_string());
                                }
                            }
                        }
                    }
                    // (legacy group_msg/group_history arms removed 2026-08-23)
                    Some("reaction") => {
                        // Single reaction: target_from + target_timestamp + emoji + from
                        let target_from = val.get("target_from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let target_ts = val.get("target_timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                        let emoji = val.get("emoji").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !emoji.is_empty() && target_ts > 0 {
                            for msg in state.gui_state.chat_messages.iter_mut() {
                                if msg.sender_key == target_from && msg.timestamp_ms == target_ts {
                                    let entry = msg.reactions.entry(emoji.clone()).or_insert_with(Vec::new);
                                    // Toggle: if already reacted, remove. Otherwise add.
                                    if let Some(idx) = entry.iter().position(|k| k == &from) {
                                        entry.remove(idx);
                                    } else {
                                        entry.push(from.clone());
                                    }
                                    if entry.is_empty() {
                                        msg.reactions.remove(&emoji);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    Some("reactions_sync") => {
                        // Bulk sync: array of {target_from, target_timestamp, emoji, from}.
                        if let Some(arr) = val.get("reactions").and_then(|v| v.as_array()) {
                            for r in arr {
                                let target_from = r.get("target_from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let target_ts = r.get("target_timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                let emoji = r.get("emoji").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let from = r.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                if emoji.is_empty() || target_ts == 0 { continue; }
                                for msg in state.gui_state.chat_messages.iter_mut() {
                                    if msg.sender_key == target_from && msg.timestamp_ms == target_ts {
                                        let entry = msg.reactions.entry(emoji.clone()).or_insert_with(Vec::new);
                                        if !entry.contains(&from) {
                                            entry.push(from.clone());
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Some("edit") => {
                        // Edited message broadcast — find by sender + timestamp, replace content.
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                        let new_content = val.get("new_content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        for msg in state.gui_state.chat_messages.iter_mut() {
                            if msg.sender_key == from && msg.timestamp_ms == ts {
                                msg.content = new_content;
                                break;
                            }
                        }
                    }
                    Some("delete") => {
                        // v0.281.0: deletion broadcast (own delete OR admin/mod
                        // moderation). Drop the matching message from the local
                        // view by sender_key + timestamp_ms. We don't try to be
                        // clever about preserving thread context — pin/reaction
                        // state for an absent message just lingers harmlessly until
                        // the next channel refetch.
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                        if !from.is_empty() && ts > 0 {
                            state.gui_state.chat_messages.retain(|m| !(m.sender_key == from && m.timestamp_ms == ts));
                        }
                    }
                    Some("message_deleted") => {
                        // v0.282.0: admin/mod deletion via DeleteById (broadcast
                        // by relay when web admin uses the by-id path; native
                        // admins use the simpler `delete` arm above). Payload
                        // carries `from` + `timestamp` for client-side removal —
                        // the message_id is for the web's DOM-keyed approach,
                        // we use (sender_key, timestamp_ms) like everywhere else
                        // on native.
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                        if !from.is_empty() && ts > 0 {
                            state.gui_state.chat_messages.retain(|m| !(m.sender_key == from && m.timestamp_ms == ts));
                        }
                    }
                    Some("typing") => {
                        // v0.282.0: typing indicator broadcast. Insert/refresh the
                        // sender's entry; the renderer prunes anything older than
                        // 3 seconds (matches web's auto-clear). Skip our own typing
                        // event — the relay echoes broadcasts to all sockets, so
                        // we'd otherwise see "Shaostoul is typing…" every time we
                        // touched the input on a different tab.
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if from.is_empty() || from == state.gui_state.profile_public_key {
                            // Continue silently — own echoes are noise.
                        } else {
                            let from_name = val.get("from_name").and_then(|v| v.as_str())
                                .unwrap_or_else(|| {
                                    // Fallback: look up display name in the user list.
                                    // Empty string yields "user" downstream when nothing matches.
                                    ""
                                })
                                .to_string();
                            let display = if from_name.is_empty() {
                                state.gui_state.chat_users.iter()
                                    .find(|u| u.public_key == from)
                                    .map(|u| u.name.clone())
                                    .unwrap_or_else(|| "Someone".to_string())
                            } else {
                                from_name
                            };
                            state.gui_state.chat_typing_users.insert(
                                from,
                                (display, std::time::Instant::now()),
                            );
                        }
                    }
                    // STALE COMMENT CORRECTED 2026-07-01 (overnight loop, cycle 12):
                    // this used to say native had no WebRTC stack and these were
                    // stubs -- that was true at v0.283.0 when this arm was written,
                    // but native voice shipped end-to-end in the v0.485-495 arc
                    // (mic capture via cpal, Opus encode, str0m WebRTC transport --
                    // see docs/STATUS.md's "Native voice" row, src/net/voice.rs,
                    // src/net/webrtc.rs). Both arms below route real signaling into
                    // the live WebRTC manager (`webrtc.submit_voice_signal` /
                    // `submit_signal`), they are not stubs. The channel-list voice
                    // icon's join/leave click handler also shipped (chat.rs, search
                    // "voice_joined") -- it just isn't at the line this comment used
                    // to cite, since line numbers drift as the file grows.
                    #[cfg(feature = "native")]
                    Some("voice_room_signal") => {
                        // Phase C: inbound voice-room signaling (offer /
                        // answer / ice / new_participant) relayed to us.
                        // Route into the WebRTC manager's voice path. Note
                        // `data` here is a JSON OBJECT (the browser
                        // RTCSessionDescription / candidate shape), unlike
                        // the webrtc_signal path where it is a string.
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let room_id = val.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let signal_type = val.get("signal_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let data = val.get("data").cloned().unwrap_or(serde_json::Value::Null);
                        if !from.is_empty() && !signal_type.is_empty() {
                            if let Some(ref webrtc) = state.gui_state.webrtc {
                                webrtc.submit_voice_signal(from, room_id, signal_type, data);
                            }
                        }
                    }
                    #[cfg(feature = "native")]
                    Some("voice_call") => {
                        // 1:1 call control plane (v0.703 — before this, native
                        // silently discarded voice_call and a web caller rang
                        // FOREVER, the worst cross-client bug in the 2026-07-04
                        // parity audit). Protocol matches chat-voice-calls.js:
                        // ring / accept / reject / hangup; the relay stamps
                        // from + from_name with the authenticated sender.
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let from_name = val
                            .get("from_name")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                state.gui_state.chat_users.iter()
                                    .find(|u| u.public_key == from)
                                    .map(|u| u.name.clone())
                            })
                            .unwrap_or_else(|| {
                                let n = from.len().min(8);
                                format!("{}...", &from[..n])
                            });
                        let action = val.get("action").and_then(|v| v.as_str()).unwrap_or("");
                        if !from.is_empty() {
                            match action {
                                "ring" => {
                                    let busy = state.gui_state.call_active.is_some()
                                        || state.gui_state.call_incoming.is_some()
                                        || state.gui_state.call_outgoing.is_some()
                                        || state.gui_state.voice_active_room.is_some();
                                    if busy {
                                        // Auto-reject like the web does when
                                        // already in a call / ringing / a room.
                                        if let Some(ref client) = state.gui_state.ws_client {
                                            let _ = client.send(&serde_json::json!({
                                                "type": "voice_call",
                                                "from": state.gui_state.profile_public_key,
                                                "to": from,
                                                "action": "reject",
                                            }).to_string());
                                        }
                                    } else {
                                        state.gui_state.call_incoming =
                                            Some((from, from_name));
                                    }
                                }
                                "accept" => {
                                    // The peer accepted OUR outgoing call. Per
                                    // the web protocol (chat-voice-calls.js the
                                    // caller creates the offer on accept), we
                                    // move to in-call and send the voice offer
                                    // over CALL_ROOM_ID (which wears the web
                                    // webrtc_signal envelope). v0.705.
                                    if state.gui_state.call_outgoing
                                        .as_ref().map(|(k, _)| *k == from).unwrap_or(false)
                                    {
                                        if let Some((k, n)) = state.gui_state.call_outgoing.take() {
                                            state.gui_state.call_outgoing_deadline = None;
                                            state.gui_state.call_active = Some((k.clone(), n));
                                            if let Some(ref webrtc) = state.gui_state.webrtc {
                                                webrtc.offer_to_voice(
                                                    k,
                                                    crate::net::webrtc::CALL_ROOM_ID.to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                                "hangup" | "reject" => {
                                    // Peer ended it (either side of any state).
                                    let matches_incoming = state.gui_state.call_incoming
                                        .as_ref().map(|(k, _)| *k == from).unwrap_or(false);
                                    let matches_outgoing = state.gui_state.call_outgoing
                                        .as_ref().map(|(k, _)| *k == from).unwrap_or(false);
                                    let matches_active = state.gui_state.call_active
                                        .as_ref().map(|(k, _)| *k == from).unwrap_or(false);
                                    if matches_incoming {
                                        state.gui_state.call_incoming = None;
                                    }
                                    if matches_outgoing {
                                        // They declined (or hung up) our ring.
                                        state.gui_state.call_outgoing = None;
                                        state.gui_state.call_outgoing_deadline = None;
                                    }
                                    if matches_active {
                                        state.gui_state.call_active = None;
                                        state.gui_state.voice_connected_peers.remove(&from);
                                        // Drop the transport now so this peer's
                                        // NEXT call isn't refused by the stale
                                        // connection's is_alive guard.
                                        if let Some(ref webrtc) = state.gui_state.webrtc {
                                            webrtc.close_peer(from.clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some("voice_room") | Some("voice_room_update") => {
                        // Control/legacy voice messages: not consumed by the
                        // native client (join/leave are outbound; the roster
                        // arrives via voice_channel_list).
                    }
                    #[cfg(feature = "native")]
                    Some("webrtc_signal") => {
                        // Two protocols share this envelope (mirroring the web):
                        // dc_* = P2P DataChannel signaling (data is a JSON
                        // STRING) -> submit_signal. Bare offer/answer/ice =
                        // 1:1 VOICE CALL signaling from a browser caller
                        // (data is a JSON OBJECT) -> the voice path under the
                        // reserved CALL_ROOM_ID, but ONLY from the peer whose
                        // call we accepted — never auto-answer an unsolicited
                        // media offer (v0.703).
                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let signal_type = val.get("signal_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let data = val.get("data").cloned().unwrap_or(serde_json::Value::Null);
                        if !from.is_empty() && !signal_type.is_empty() {
                            let is_call_signal = matches!(
                                signal_type.as_str(),
                                "offer" | "answer" | "ice"
                            );
                            if is_call_signal {
                                let call_peer_ok = state.gui_state.call_active
                                    .as_ref()
                                    .map(|(k, _)| *k == from)
                                    .unwrap_or(false);
                                if call_peer_ok {
                                    if let Some(ref webrtc) = state.gui_state.webrtc {
                                        webrtc.submit_voice_signal(
                                            from,
                                            crate::net::webrtc::CALL_ROOM_ID.to_string(),
                                            signal_type,
                                            data,
                                        );
                                    }
                                }
                            } else if let Some(ref webrtc) = state.gui_state.webrtc {
                                webrtc.submit_signal(from, signal_type, data);
                            }
                        }
                    }
                    Some("federated_chat") => {
                        // v0.282.0: chat from a federated peer server. Display
                        // alongside local chat in the same channel, prefixed by
                        // the originating server's name so users can tell where
                        // it came from. The relay only delivers federated_chat
                        // for channels it's federating, so we don't need a
                        // per-channel federation flag client-side.
                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                        let from_name = val.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let server_name = val.get("server_name").and_then(|v| v.as_str()).unwrap_or("federated").to_string();
                        let server_id = val.get("server_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !content.is_empty() && ts > 0 {
                            // Dedup across carriers: once several of our
                            // connections bridge the same room, each delivers
                            // its own copy of the same origin line. The wire
                            // carries no sender key (from_name only), so the
                            // stable identity is (origin_server, timestamp_ms,
                            // content) -- display formatting like server_name
                            // can differ per carrier and must not split it.
                            let duplicate = state.gui_state.chat_messages.iter().any(|m| {
                                m.origin_server == server_id
                                    && m.timestamp_ms == ts
                                    && m.content == content
                            });
                            if !duplicate {
                                let ts_str = {
                                    let secs = ts / 1000;
                                    let h = (secs / 3600) % 24;
                                    let m = (secs / 60) % 60;
                                    format!("{:02}:{:02}", h, m)
                                };
                                state.gui_state.chat_messages.push(crate::gui::ChatMessage {
                                    // Tag the displayed name with the origin server
                                    // so federated messages are visually distinct.
                                    // E.g. "Alice (other-server)".
                                    sender_name: format!("{} ({})", from_name, server_name),
                                    // sender_key uses the server_id so reactions/replies
                                    // can target the federated origin (the relay won't
                                    // honor cross-server reactions today, but the field
                                    // shape is preserved for forward compat).
                                    sender_key: server_id.clone(),
                                    content,
                                    timestamp: ts_str,
                                    timestamp_ms: ts,
                                    channel,
                                    server: crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url),
                                    origin_server: server_id,
                                    ..Default::default()
                                });
                                // Bound message buffer (this arm used to be the
                                // only push without the cap).
                                while state.gui_state.chat_messages.len() > 200 {
                                    state.gui_state.chat_messages.remove(0);
                                }
                            }
                        }
                    }
                    Some("search_results") => {
                        // Server-returned search results. Populate the search modal.
                        if let Some(results) = val.get("results").and_then(|v| v.as_array()) {
                            state.gui_state.chat_search_results.clear();
                            for r in results {
                                let channel = r.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let sender_name = r.get("from_name").and_then(|v| v.as_str())
                                    .or_else(|| r.get("from").and_then(|v| v.as_str()))
                                    .unwrap_or("Anonymous").to_string();
                                let content = r.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let timestamp_ms = r.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                state.gui_state.chat_search_results.push(crate::gui::ChatSearchResult {
                                    channel, sender_name, content, timestamp_ms,
                                });
                            }
                        }
                    }
                    Some("pins_sync") => {
                        // Replace the channel's pin list.
                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if let Some(arr) = val.get("pins").and_then(|v| v.as_array()) {
                            let pins: Vec<crate::gui::ChatPin> = arr.iter().map(|p| crate::gui::ChatPin {
                                from_key: p.get("from_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                from_name: p.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                content: p.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                original_timestamp: p.get("original_timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                                pinned_by: p.get("pinned_by").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                pinned_at: p.get("pinned_at").and_then(|v| v.as_u64()).unwrap_or(0),
                            }).collect();
                            state.gui_state.chat_pins.insert(channel, pins);
                        }
                    }
                    Some("pin_added") => {
                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if let Some(p) = val.get("pin") {
                            let pin = crate::gui::ChatPin {
                                from_key: p.get("from_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                from_name: p.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                content: p.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                original_timestamp: p.get("original_timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                                pinned_by: p.get("pinned_by").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                pinned_at: p.get("pinned_at").and_then(|v| v.as_u64()).unwrap_or(0),
                            };
                            state.gui_state.chat_pins.entry(channel).or_insert_with(Vec::new).push(pin);
                        }
                    }
                    Some("pin_removed") => {
                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let index = val.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        // The relay's PinRemoved index is 1-BASED (it echoes the
                        // /unpin <N> argument; storage does ids[index-1]). This
                        // handler treated it as 0-based, so unpinning pin #1
                        // locally removed the SECOND pin and unpinning the last
                        // pin removed nothing. Off-by-one fixed v0.722.
                        if let Some(pins) = state.gui_state.chat_pins.get_mut(&channel) {
                            let i = index.saturating_sub(1);
                            if index >= 1 && i < pins.len() {
                                pins.remove(i);
                            }
                        }
                    }
                    Some("member_joined") => {
                        log::debug!("Received server message type: member_joined");
                    }
                    Some("member_left") => {
                        // Server-broadcast when a member is kicked / banned /
                        // leaves voluntarily. Prune them from the live user list
                        // + DMs + friends so the sidebar updates immediately.
                        if let Some(pk) = val.get("public_key").and_then(|v| v.as_str()) {
                            let pk = pk.to_string();
                            state.gui_state.chat_users.retain(|u| u.public_key != pk);
                            state.gui_state.chat_friends.retain(|f| f.public_key != pk);
                            state.gui_state.chat_dms.retain(|d| d.user_key != pk);
                            // If the user modal is open for this user, close it.
                            if state.gui_state.chat_user_modal_key == pk {
                                state.gui_state.chat_user_modal_open = false;
                            }
                            log::info!("Member left/kicked/banned: {}", &pk[..pk.len().min(16)]);
                        }
                    }
                    Some("task_list_response") => {
                        // Task list response from the WebSocket task_list request
                        if let Some(tasks) = val.get("tasks").and_then(|v| v.as_array()) {
                            state.gui_state.tasks.clear();
                            for task in tasks {
                                let id = task.get("id")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32;
                                let title = task.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let description = task.get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let status_str = task.get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("todo");
                                let status = match status_str {
                                    "in_progress" => crate::gui::TaskStatus::InProgress,
                                    "done" => crate::gui::TaskStatus::Done,
                                    _ => crate::gui::TaskStatus::Todo,
                                };
                                let priority_str = task.get("priority")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("medium");
                                let priority = match priority_str {
                                    "low" => crate::gui::TaskPriority::Low,
                                    "high" => crate::gui::TaskPriority::High,
                                    "critical" => crate::gui::TaskPriority::Critical,
                                    _ => crate::gui::TaskPriority::Medium,
                                };
                                let assignee = task.get("assignee")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let labels: Vec<String> = task.get("labels")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .split(',')
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.trim().to_string())
                                    .collect();
                                state.gui_state.tasks.push(
                                    crate::gui::GuiTask { id, title, description, priority, status, assignee, labels },
                                );
                                if id >= state.gui_state.task_next_id {
                                    state.gui_state.task_next_id = id + 1;
                                }
                            }
                            log::info!("Received {} tasks from server (task_list_response)", state.gui_state.tasks.len());
                        }
                    }
                    Some("name_taken") => {
                        let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("Name taken");
                        log::warn!("Name taken: {}. Disconnecting and reconnecting with unique name.", msg);
                        crate::debug::push_debug(format!("Name taken, reconnecting: {}", msg));

                        // Generate a fallback name: "DesktopUser_XXXX"
                        let suffix: u16 = (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() % 10000) as u16;
                        let fallback = format!("DesktopUser_{:04}", suffix);
                        state.gui_state.profile_name = fallback.clone();
                        // Persist the fallback as the user_name + save the
                        // config so the NEXT launch reuses this name (relay
                        // accepts re-registration from the same key, so no
                        // collision). Previously, profile_name was set but
                        // user_name stayed at the conflicting value, so each
                        // boot tried "Shaostoul" → name_taken → a brand-new
                        // DesktopUser_NNNN → permanent server-side drip
                        // (operator reported 6+ entries on one key).
                        // The user can rename back to a preferred name from
                        // Settings once the conflicting registration clears.
                        state.gui_state.user_name = fallback.clone();
                        crate::config::AppConfig::from_gui_state(&state.gui_state).save();

                        // Disconnect current connection
                        if let Some(ref mut client) = state.gui_state.ws_client {
                            client.disconnect();
                        }
                        state.gui_state.ws_client = None;

                        // Reconnect with new name (full fresh handshake so
                        // server sends channel_list, dm_list, group_list, etc.)
                        let url = state.gui_state.server_url.clone();
                        let ws_url = url.replace("https://", "wss://").replace("http://", "ws://");
                        let ws_url = format!("{}/ws", ws_url.trim_end_matches('/'));
                        // Full-PQ: keep advertising our Kyber key on the
                        // name-collision reconnect — the 3-arg connect()
                        // sent empty kyber, which silently broke DMs for
                        // any DesktopUser-fallback session.
                        let new_client = crate::net::ws_client::WsClient::connect_with_kyber(&ws_url, &fallback, &state.gui_state.profile_public_key, &state.gui_state.kyber_public_b64);
                        state.gui_state.ws_client = Some(new_client);
                        state.gui_state.connected_server_url = url.clone();
                        // Fresh socket: identify handshake not yet complete (v0.794).
                        state.gui_state.ws_identified = false;
                        state.gui_state.dm_fetch_sent = false;
                        state.gui_state.ws_status = format!("Reconnecting as {}...", fallback);
                        log::info!("Reconnecting as: {}", fallback);
                    }
                    Some("task_list") => {
                        if let Some(tasks) = val.get("tasks").and_then(|v| v.as_array()) {
                            state.gui_state.tasks.clear();
                            for task in tasks {
                                let id = task.get("id")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32;
                                let title = task.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let description = task.get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let status_str = task.get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("todo");
                                let status = match status_str {
                                    "in_progress" => crate::gui::TaskStatus::InProgress,
                                    "done" => crate::gui::TaskStatus::Done,
                                    _ => crate::gui::TaskStatus::Todo,
                                };
                                let priority_str = task.get("priority")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("medium");
                                let priority = match priority_str {
                                    "low" => crate::gui::TaskPriority::Low,
                                    "high" => crate::gui::TaskPriority::High,
                                    "critical" => crate::gui::TaskPriority::Critical,
                                    _ => crate::gui::TaskPriority::Medium,
                                };
                                let assignee = task.get("assignee")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let labels: Vec<String> = task.get("labels")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .split(',')
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.trim().to_string())
                                    .collect();
                                state.gui_state.tasks.push(
                                    crate::gui::GuiTask { id, title, description, priority, status, assignee, labels },
                                );
                                if id >= state.gui_state.task_next_id {
                                    state.gui_state.task_next_id = id + 1;
                                }
                            }
                            log::info!("Received {} tasks from server", state.gui_state.tasks.len());
                        }
                    }
                    // ── Marketplace sync (v0.752, ladder rung 5) ──
                    // The Market page sends listing_browse; the
                    // relay answers listing_list (unicast) and
                    // keeps everyone current with listing_new /
                    // listing_updated / listing_deleted broadcasts.
                    Some("listing_list") => {
                        if let Some(arr) = val.get("listings").and_then(|v| v.as_array()) {
                            state.gui_state.listings = arr
                                .iter()
                                .map(crate::gui::GuiListing::from_relay_json)
                                .collect();
                            state.gui_state.listing_status.clear();
                        }
                    }
                    Some("listing_new") | Some("listing_updated") => {
                        if let Some(l) = val.get("listing") {
                            let gl = crate::gui::GuiListing::from_relay_json(l);
                            if gl.seller_key == state.gui_state.profile_public_key {
                                state.gui_state.listing_status = "Listing published.".to_string();
                            }
                            if let Some(slot) = state
                                .gui_state
                                .listings
                                .iter_mut()
                                .find(|x| x.id == gl.id)
                            {
                                *slot = gl;
                            } else {
                                state.gui_state.listings.insert(0, gl);
                            }
                        }
                    }
                    Some("listing_deleted") => {
                        if let Some(id) = val.get("id").and_then(|v| v.as_str()) {
                            state.gui_state.listings.retain(|l| l.id != id);
                            if state.gui_state.listing_selected.as_deref() == Some(id) {
                                state.gui_state.listing_selected = None;
                            }
                            if state.gui_state.listing_status == "Deleting listing..." {
                                state.gui_state.listing_status = "Listing removed.".to_string();
                            }
                        }
                    }
                    // ── Reviews (v0.755) ── (the listing thread
                    // arms died with the plaintext thread
                    // protocol, 2026-08-23; marketplace contact
                    // rides sealed-sender DMs now.)
                    Some("review_created") => {
                        if let Some(r) = val.get("review") {
                            let gr = crate::gui::GuiReview::from_relay_json(r);
                            let lid = r
                                .get("listing_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if lid == state.gui_state.listing_reviews_for {
                                state.gui_state.listing_reviews.push(gr);
                                // Keep the header honest without a refetch.
                                let n = state.gui_state.listing_reviews.len() as f32;
                                let sum: f32 = state
                                    .gui_state
                                    .listing_reviews
                                    .iter()
                                    .map(|x| x.rating as f32)
                                    .sum();
                                state.gui_state.listing_reviews_count =
                                    state.gui_state.listing_reviews.len() as i64;
                                state.gui_state.listing_reviews_avg =
                                    if n > 0.0 { sum / n } else { 0.0 };
                            }
                        }
                    }
                    Some("review_deleted") => {
                        let lid = val.get("listing_id").and_then(|v| v.as_str()).unwrap_or("");
                        let rid = val.get("review_id").and_then(|v| v.as_i64()).unwrap_or(-1);
                        if lid == state.gui_state.listing_reviews_for {
                            state.gui_state.listing_reviews.retain(|r| r.id != rid);
                            let n = state.gui_state.listing_reviews.len() as f32;
                            let sum: f32 = state
                                .gui_state
                                .listing_reviews
                                .iter()
                                .map(|x| x.rating as f32)
                                .sum();
                            state.gui_state.listing_reviews_count =
                                state.gui_state.listing_reviews.len() as i64;
                            state.gui_state.listing_reviews_avg =
                                if n > 0.0 { sum / n } else { 0.0 };
                        }
                    }
                    Some("private") => {
                        // Private server-to-user message (rate limit, errors, command responses)
                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                            crate::debug::push_debug(format!("Private: {}", msg));
                            // Multiplayer (v0.472): the game_welcome (our player id +
                            // world snapshot) arrives as a private __game__ message.
                            if let Some(payload) = msg.strip_prefix("__game__:") {
                                let payload = payload.to_string();
                                route_game_message(state, &payload);
                                continue;
                            }
                            // P2P trades (v0.756) ride targeted private
                            // wrappers (same delivery web consumes) -
                            // route them to the Trade page, never chat.
                            if let Some(payload) = msg.strip_prefix("__trade_data__:") {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                                    if let Some(t) = v.get("trade") {
                                        let gt = crate::gui::GuiTrade::from_relay_json(t);
                                        if let Some(slot) = state
                                            .gui_state
                                            .trades
                                            .iter_mut()
                                            .find(|x| x.id == gt.id)
                                        {
                                            *slot = gt;
                                        } else {
                                            state.gui_state.trades.insert(0, gt);
                                        }
                                    }
                                }
                                continue;
                            }
                            if let Some(payload) = msg.strip_prefix("__trade_list__:") {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                                    if let Some(arr) = v.get("trades").and_then(|x| x.as_array()) {
                                        state.gui_state.trades = arr
                                            .iter()
                                            .map(crate::gui::GuiTrade::from_relay_json)
                                            .collect();
                                    }
                                }
                                continue;
                            }
                            if let Some(payload) = msg.strip_prefix("__trade_complete__:") {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                                    if let Some(tid) = v.get("trade_id").and_then(|x| x.as_str()) {
                                        if let Some(t) = state
                                            .gui_state
                                            .trades
                                            .iter_mut()
                                            .find(|x| x.id == tid)
                                        {
                                            t.status = "completed".to_string();
                                        }
                                        state.gui_state.trade_status =
                                            "Trade completed - items exchanged.".to_string();
                                    }
                                }
                                continue;
                            }
                            // Filter out profile validation noise (not relevant to chat)
                            let is_profile_noise = msg.contains("Profile URL")
                                || msg.contains("must start with https://")
                                || msg.starts_with("__sync_data__")
                                || msg == "sync_ack";
                            if !is_profile_noise {
                                // Show as system message in chat
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                state.gui_state.chat_messages.push(
                                    crate::gui::ChatMessage {
                                        sender_name: "System".to_string(),
                                        sender_key: String::new(),
                                        content: msg.to_string(),
                                        timestamp: crate::gui::pages::chat::format_timestamp(now_ms),
                                        timestamp_ms: now_ms,
                                        // Don't leak into an open P2P group / DM (it'd vanish on reload).
                                        channel: crate::gui::pages::chat::notice_channel(&state.gui_state.chat_active_channel),
                                        server: crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    }
                    _ => {
                        // Log unhandled message types to debug console
                        let msg_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        crate::debug::push_debug(format!("Unhandled WS type: {}", msg_type));
                    }
                }
            }
        }
    }

    // ── Drop dead WebSocket client and start reconnect timer ──
    if ws_dropped {
        state.gui_state.ws_client = None;
        // Tear down the WebRTC manager too: its signaling rides
        // the WS, so without a live WS it can't negotiate. The
        // thread stops when its handle (and thus the command
        // sender) drops. It re-starts lazily on reconnect.
        #[cfg(feature = "native")]
        {
            state.gui_state.webrtc = None;
        }
        // Force the Banned-users panel to re-request after a
        // reconnect (the relay only sends it on demand). The
        // cached list itself is harmless to keep until then.
        state.gui_state.chat_banned_requested = false;
        state.gui_state.chat_muted_requested = false;
        // Same for the Game Admin game-ban list (v0.474).
        state.gui_state.game_bans_requested = false;
        // And the Backups panel (v0.938).
        state.gui_state.backup_list_requested = false;
        if !state.gui_state.ws_manually_disconnected {
            log::info!("WebSocket disconnected, will reconnect in {}s (attempt {})",
                state.gui_state.ws_reconnect_delay as u32,
                state.gui_state.ws_reconnect_attempts + 1);
            state.gui_state.ws_reconnect_timer = state.gui_state.ws_reconnect_delay;
            state.gui_state.ws_status = format!("Reconnecting in {}s...",
                state.gui_state.ws_reconnect_delay as u32);
        } else {
            state.gui_state.ws_status = "Disconnected".to_string();
        }
    }

    // Self-hosted fast path (field test 4): when the active
    // server IS the node this app hosts and that node is up,
    // never sit out a long backoff -- "I'm obviously connected
    // to myself". Cap the countdown at half a second so the
    // link snaps up as soon as the local node is ready.
    if state.gui_state.ws_client.is_none()
        && !state.gui_state.ws_manually_disconnected
        && state.gui_state.ws_reconnect_timer > 0.5
    {
        if let Some(local) = crate::gui::pages::host_node::running_local_url() {
            let cur = crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url);
            if cur == crate::gui::pages::chat::norm_server_url(&local) {
                state.gui_state.ws_reconnect_timer = 0.5;
            }
        }
    }
}
