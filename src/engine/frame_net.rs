//! The frame's network pump (moved out of the frame loop in src/lib.rs, 2026-10-31, notifications
//! rung 2 part 2): dialling the server and keeping its socket, reading every message it sends,
//! the reconnect countdown, the WebRTC manager, the chat history pump and every saved server's
//! background link.
//!
//! One function, called from two places, so a window hidden in the notification area reads and
//! reconnects exactly as a shown one does: the drawn frame (lib.rs, the `cpu.span.network` stage)
//! and the hidden window's slow tick (`engine/tray_window.rs`), which draws nothing and runs no
//! world (docs/design/notifications.md, "The tray").

use crate::engine::state::EngineState;

/// Everything the frame does with the network, once. `dt` is the frame's (clamped) step, which
/// the reconnect countdown and the background links' timers run on.
pub(crate) fn pump(state: &mut EngineState, dt: f32) {
    // ── Auto-connect to server if configured AND seed unlocked ──
    // Full-PQ guard (was the limited-mode squat bug): we must
    // NOT auto-connect with an encrypted/locked seed. A locked
    // identity can't derive Kyber, so a "connect" in that
    // state registers a keyless name-squatter on the relay
    // (Shaostoul → key-with-no-kyber), and every subsequent
    // generate / restart bounces to a new DesktopUser_NNNN.
    // Refuse to connect until private_key_bytes is in memory
    // (Settings → Security → Unlock, or Recover from seed,
    // or Generate New Identity).
    let seed_unlocked = state.gui_state.private_key_bytes.is_some();
    if !seed_unlocked
        && state.gui_state.ws_client.is_none()
        && !state.gui_state.encrypted_private_key.is_empty()
        && state.gui_state.ws_status != "Identity locked — Settings → Security → Unlock (or Recover from seed) to connect"
    {
        // Surface the actionable reason once.
        state.gui_state.ws_status = "Identity locked — Settings → Security → Unlock (or Recover from seed) to connect".to_string();
    }
    // The decision, tested, lives in gui/connections.rs `may_auto_connect`: an
    // unlocked identity, onboarding done, no Disconnect, no pending backoff, and
    // never a server this identity erased its account on (BUG-135). It dials
    // `dial_address`, never an address still being typed (BUG-160).
    if state.gui_state.may_auto_connect() {
        let address = state.gui_state.dial_address().unwrap_or_default().to_string();
        let ws_url = crate::gui::pages::chat::derive_ws_url(&address);
        let name = state.gui_state.user_name.clone();
        let pubkey = if state.gui_state.profile_public_key.is_empty() {
            crate::gui::pages::chat::generate_random_hex_key()
        } else {
            state.gui_state.profile_public_key.clone()
        };

        // Full-PQ: send our key card (our Kyber768 key signed by
        // our identity, engine/key_cards.rs) so peers can seal
        // DMs to us and check the key is ours. It is signed on
        // recovery/unlock; if the seed is in memory but no card
        // was signed yet, sign it now, else connect without one
        // (degraded: can receive once unlocked).
        if state.gui_state.own_key_card.is_empty() {
            crate::engine::key_cards::sign_own_card(&mut state.gui_state);
        }
        let key_card = state.gui_state.own_key_card.clone();

        state.gui_state.ws_client = Some(
            crate::net::ws_client::WsClient::connect_with_key_card(&ws_url, &name, &pubkey, &key_card),
        );
        state.gui_state.connected_server_url = address;
        // Fresh socket: identify handshake not yet complete (v0.794).
        state.gui_state.ws_identified = false;
        state.gui_state.dm_fetch = Default::default();
        state.gui_state.ws_status = "Connecting...".to_string();
    }

    // ── Poll WebSocket messages from relay server ──
    // The pump itself (every `type` the relay can send, plus
    // the socket-died teardown that arms the countdown below)
    // lives in engine/frame_ws_poll.rs: 1,667 lines with one
    // job and one input. Add new message types THERE, not here
    // (monolith ratchet, v0.1320).
    crate::engine::frame_ws_poll::poll_relay_messages(state);

    // ── WebSocket auto-reconnect with exponential backoff ──
    // The decision, tested, is gui/connections.rs `backoff_reconnect_runs` (BUG-160).
    if state.gui_state.backoff_reconnect_runs() {
        state.gui_state.ws_reconnect_timer -= dt;
        let secs_left = state.gui_state.ws_reconnect_timer.ceil() as u32;
        state.gui_state.ws_status = format!("Reconnecting in {}s...", secs_left.max(1));

        if state.gui_state.ws_reconnect_timer <= 0.0 {
            // Attempt reconnect, to the dropped connection's own server
            // (`dial_address`), never to an address still being typed.
            let address = state.gui_state.dial_address().unwrap_or_default().to_string();
            let ws_url = crate::gui::pages::chat::derive_ws_url(&address);
            let name = state.gui_state.user_name.clone();
            let pubkey = if state.gui_state.profile_public_key.is_empty() {
                crate::gui::pages::chat::generate_random_hex_key()
            } else {
                state.gui_state.profile_public_key.clone()
            };
            log::info!("Attempting WebSocket reconnect (attempt {})", state.gui_state.ws_reconnect_attempts + 1);
            // Full-PQ: backoff reconnect must also carry the
            // key card, else DMs break after any drop.
            state.gui_state.ws_client = Some(
                crate::net::ws_client::WsClient::connect_with_key_card(&ws_url, &name, &pubkey, &state.gui_state.own_key_card),
            );
            state.gui_state.connected_server_url = address;
            // Fresh socket: identify handshake not yet complete (v0.794).
            state.gui_state.ws_identified = false;
            state.gui_state.dm_fetch = Default::default();
            state.gui_state.ws_reconnect_attempts += 1;
            // Clear the rate-limit guard now that we are actually retrying: if this
            // attempt is throttled again, the system handler re-arms it. (v0.544)
            state.gui_state.ws_rate_limited = false;
            // Exponential backoff: 5s -> 10s -> 20s -> 40s -> 60s (max)
            state.gui_state.ws_reconnect_delay = (state.gui_state.ws_reconnect_delay * 2.0).min(60.0);
            state.gui_state.ws_status = "Reconnecting...".to_string();
        }
    }

    // ── Reset backoff on successful connection ──
    // Skipped while rate-limited (v0.544): the socket OPENS before the identify that
    // gets throttled, so resetting on mere connection would clobber the 65s back-off
    // and loop straight back into the limit.
    if !state.gui_state.ws_rate_limited
        && state.gui_state.ws_client.as_ref().map_or(false, |c| c.is_connected())
    {
        if state.gui_state.ws_reconnect_attempts > 0 {
            log::info!("WebSocket reconnected after {} attempts", state.gui_state.ws_reconnect_attempts);
        }
        state.gui_state.ws_reconnect_delay = crate::net::ws_client::RECONNECT_DELAY_INITIAL_SECS;
        state.gui_state.ws_reconnect_attempts = 0;
        state.gui_state.ws_reconnect_timer = 0.0;
        // The server we are CONNECTED to is by definition a
        // server worth keeping: make sure it is in the saved
        // list. The operator's original home server never
        // appeared there (only explicitly-added ones did), so
        // switching away meant hunting for a way back
        // (report 2026-08-14).
        let url = state.gui_state.server_url.trim_end_matches('/').to_string();
        if !url.is_empty()
            && !state.gui_state.chat_servers.iter().any(|s| s.url == url)
        {
            state.gui_state.chat_servers.push(crate::gui::ChatServer {
                id: format!("srv_{}", url),
                name: crate::gui::pages::chat::server_display_name(&url),
                url,
                connected: true,
                channels: Vec::new(),
                voice_channels: Vec::new(),
            });
            state.gui_state.settings_dirty = true;
        }
    }

    // ── Native WebRTC DataChannel P2P (increment 1) ──
    // Lazily start the manager once the WS is connected AND we
    // have our pubkey hex (it's the identity peers know us by
    // and the value the offerer rule compares). Then, each
    // frame, relay its outbound signaling to the WS and surface
    // its events (channel open / inbound frames) as debug lines.
    #[cfg(feature = "native")]
    {
        let ws_connected = state
            .gui_state
            .ws_client
            .as_ref()
            .map_or(false, |c| c.is_connected());
        let have_key = !state.gui_state.profile_public_key.is_empty();

        // Lazy start.
        if ws_connected && have_key && state.gui_state.webrtc.is_none() {
            let my_key = state.gui_state.profile_public_key.clone();
            // The server's own STUN entry comes from the server
            // this session is connected to (ws/wss URL -> http/https
            // base, /ws suffix dropped), not a hardcoded host. Call
            // credentials ride the chat socket (step E).
            let relay_base = {
                let ws_url = state.gui_state.connected_server_url.trim();
                let ws_url = ws_url.trim_end_matches('/');
                let ws_url = ws_url.strip_suffix("/ws").unwrap_or(ws_url);
                if let Some(rest) = ws_url.strip_prefix("wss://") {
                    format!("https://{rest}")
                } else if let Some(rest) = ws_url.strip_prefix("ws://") {
                    format!("http://{rest}")
                } else {
                    ws_url.to_string()
                }
            };
            let handle =
                crate::net::webrtc::WebrtcManager::start(my_key, relay_base);
            state.gui_state.webrtc = Some(handle);
            crate::debug::push_debug("WebRTC P2P manager started");
        }

        // Per-frame pump: relay outbound webrtc_signal JSON to
        // the WS, and drain events into the debug console. We
        // collect first (immutable borrow of webrtc), then act
        // (the WS send + debug push) to avoid overlapping
        // borrows of gui_state.
        if state.gui_state.webrtc.is_some() {
            // Short, log-friendly form of a long pubkey hex.
            fn short(k: &str) -> String {
                if k.len() > 12 { format!("{}…", &k[..12]) } else { k.to_string() }
            }
            let (outbound, events) = {
                let w = state.gui_state.webrtc.as_ref().unwrap();
                (w.poll_outbound(), w.poll_events())
            };
            for json in outbound {
                if let Some(ref ws) = state.gui_state.ws_client {
                    // A dc_offer carries the target's friendship pass (passes v2, engine/dm.rs).
                    ws.send(&crate::engine::dm::with_pass_on_offer(&state.gui_state, json));
                }
            }
            for ev in events {
                match ev {
                    crate::net::webrtc::WebrtcEvent::ChannelOpen { peer } => {
                        crate::debug::push_debug(format!(
                            "WebRTC: channel OPEN with {}", short(&peer)
                        ));
                        // On open, fire the dev test frame if the
                        // chat page armed one for this peer.
                        if state.gui_state.webrtc_test_peer.as_deref() == Some(peer.as_str()) {
                            if let Some(ref w) = state.gui_state.webrtc {
                                w.send_text(peer.clone(), "native p2p test".to_string());
                            }
                            crate::debug::push_debug(format!(
                                "WebRTC: sent test frame to {}", short(&peer)
                            ));
                        }
                    }
                    crate::net::webrtc::WebrtcEvent::Frame { peer, text } => {
                        // inc-2: a frame may be a P2P group-object
                        // push ({type:"p2p_group_obj", submission}).
                        // handle_p2p_group_obj verifies + dedups +
                        // gates + decrypts + renders it, and returns
                        // true if it consumed the frame (even when it
                        // legitimately dropped it). Only fall back to
                        // the inc-1 debug line if it was NOT a group
                        // obj frame (e.g. the "native p2p test" text).
                        let consumed = crate::gui::pages::chat::handle_p2p_group_obj(
                            &mut state.gui_state, &peer, &text,
                        );
                        if !consumed {
                            crate::debug::push_debug(format!(
                                "WebRTC: frame from {}: {}", short(&peer), text
                            ));
                        }
                    }
                    crate::net::webrtc::WebrtcEvent::VoiceConnected { peer } => {
                        // Phase C/D: a voice peer's transport is up. Track it
                        // so the session pumps our mic Opus to it.
                        crate::debug::push_debug(format!(
                            "Voice: CONNECTED with {}", short(&peer)
                        ));
                        state.gui_state.voice_connected_peers.insert(peer);
                    }
                    crate::net::webrtc::WebrtcEvent::VoiceFrame { peer, opus } => {
                        // Phase D: inbound Opus from a voice peer -> the voice
                        // session decodes, mixes, and plays it. Count frames
                        // for the debug log (~50 frames ~= 1s of speech).
                        // Only while voice is wanted (a room or a call): a frame
                        // after leaving (closing into the tray leaves all voice,
                        // notifications rung 2 part 2) feeds no session.
                        if !crate::gui::pages::chat::voice_wanted(&state.gui_state) {
                            continue;
                        }
                        crate::net::voice::push_remote_opus(peer.clone(), opus.clone());
                        state.gui_state.voice_rx_frames = state.gui_state.voice_rx_frames.wrapping_add(1);
                        if state.gui_state.voice_rx_frames % 100 == 1 {
                            crate::debug::push_debug(format!(
                                "Voice: receiving audio from {} ({} frames)",
                                short(&peer), state.gui_state.voice_rx_frames
                            ));
                        }
                    }
                    crate::net::webrtc::WebrtcEvent::Closed { peer } => {
                        state.gui_state.voice_connected_peers.remove(&peer);
                        // If this peer was our 1:1 call, the call is over
                        // (covers the web side closing its RTCPeerConnection
                        // without us seeing a hangup message, e.g. tab close).
                        if state.gui_state.call_active
                            .as_ref()
                            .map(|(k, _)| *k == peer)
                            .unwrap_or(false)
                        {
                            state.gui_state.call_active = None;
                        }
                        crate::debug::push_debug(format!(
                            "WebRTC: channel CLOSED with {}", short(&peer)
                        ));
                    }
                    // Calls through the server (step E): ready, or cannot be had.
                    ev @ (crate::net::webrtc::WebrtcEvent::RelayReady { .. }
                    | crate::net::webrtc::WebrtcEvent::RelayUnavailable { .. }) => {
                        crate::engine::call_relay::on_event(&mut state.gui_state, ev)
                    }
                }
            }
        }
        // Ask for the call or voice room's credentials, end the one left (step E).
        crate::engine::call_relay::pump(&mut state.gui_state);
    }

    // Chat history fetch + drain (background thread, short
    // timeouts; extracted with the 2026-08-13 freeze fix, see
    // net_route::chat_history_pump for the whole story).
    crate::engine::net_route::chat_history_pump(state);

    // Background relay connections: dial every saved server,
    // keep parked links alive, store their traffic + unread
    // (multi-connection stage 3, engine/bg_connections.rs).
    crate::engine::bg_connections::pump_background_connections(state, dt);
    // Reports about groups (10j): lookups, checks and removals run off the frame.
    crate::engine::group_report::pump(&mut state.gui_state);
}
