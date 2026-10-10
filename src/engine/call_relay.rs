//! Calls through the server (step E of docs/design/blocking-and-safe-mode.md, section 10f,
//! 2026-10-09): the desktop app's glue between its state, the chat socket and the WebRTC
//! manager. The protocol and its small state machine are src/net/call_relay.rs; the forwarder
//! client and the relay-only connections are src/net/webrtc.rs.
//!
//! Every frame, `pump` works out which call or voice room this app is in (`wanted_scope`), asks
//! the server for that one's credentials, and gives up after `CREDENTIALS_WAIT` when nothing
//! comes. The reply reaches `on_frame` from the message pump's catch-all arm (frame_ws_poll.rs
//! sits at its size budget, so nothing more goes there) and goes to the manager; a refusal is
//! said at once. lib.rs passes the manager's two relay events to `on_event`. Whenever the call
//! or room cannot go through the server, the person is told once (a notice) and the call bar
//! keeps the line (src/gui/pages/chat/call_relay_bar.rs); nothing falls back to a direct
//! connection.

use std::time::Instant;

use crate::gui::GuiState;
use crate::net::call_relay::{self, CallScope, Step};
use crate::net::webrtc::WebrtcEvent;

/// The call or voice room this app is in, whose connection must go through the server.
///
/// A 1:1 call once accepted (`call_active`). A voice room once the server's roster lists us in
/// it: the server gives a room's credentials only to someone in its live roster, so asking
/// earlier would be refused. Once asked, the room stays wanted while we are in it, even if a
/// roster update briefly leaves us out (it must not end the room's connections). The desktop
/// app is never in both (Call is off inside a voice room, and a ring there is turned away); if
/// it ever were, the call would win.
pub(crate) fn wanted_scope(gs: &GuiState) -> Option<CallScope> {
    if let Some((peer, _)) = &gs.call_active {
        return Some(CallScope::Call(peer.clone()));
    }
    let room = gs.voice_active_room.as_ref()?;
    let scope = CallScope::Room(room.clone());
    let me = &gs.profile_public_key;
    let listed = gs
        .chat_channels
        .iter()
        .any(|c| &c.id == room && c.voice_participants.iter().any(|(k, _)| k == me));
    (listed || gs.call_relay.scope() == Some(&scope)).then_some(scope)
}

/// Once a frame: ask for the credentials of the call or room the app is in, end the one it
/// left, and give up on an unanswered request. Nothing happens without a WebRTC manager (it
/// rides the chat socket; a new one starts with nothing asked).
pub(crate) fn pump(gs: &mut GuiState) {
    if gs.webrtc.is_none() {
        gs.call_relay.reset();
        return;
    }
    let wanted = wanted_scope(gs);
    let steps = gs.call_relay.update(wanted, Instant::now());
    for step in steps {
        apply(gs, step);
    }
}

/// A frame the message pump did not handle. True when it was a `call_credentials` reply (taken
/// or dropped here).
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    if frame.get("type").and_then(|t| t.as_str()) != Some("call_credentials") {
        return false;
    }
    match call_relay::parse_reply(frame) {
        Some(reply) => {
            if let Some(step) = gs.call_relay.on_reply(reply) {
                apply(gs, step);
            }
        }
        None => log::debug!("call_credentials reply names no single room or call; ignored"),
    }
    true
}

/// The WebRTC manager's word on the connection through the server.
pub(crate) fn on_event(gs: &mut GuiState, ev: WebrtcEvent) {
    match ev {
        WebrtcEvent::RelayReady { scope } => gs.call_relay.on_ready(&scope),
        WebrtcEvent::RelayUnavailable { scope, lost } => {
            if gs.call_relay.on_unavailable(&scope, lost) {
                if let Some(line) = gs.call_relay.line_for(&scope) {
                    gs.pending_notices.push(line.to_string());
                }
            }
        }
        _ => {}
    }
}

fn apply(gs: &mut GuiState, step: Step) {
    match step {
        Step::Ask(frame) => match gs.ws_client.as_ref().filter(|c| c.is_connected()) {
            Some(client) => client.send(&frame),
            // No socket: the request times out into the line, as if unanswered.
            None => log::debug!("call_credentials not sent: not connected"),
        },
        Step::Use(creds) => {
            if let Some(webrtc) = &gs.webrtc {
                webrtc.use_relay(creds);
            }
        }
        Step::End(scope) => {
            if let Some(webrtc) = &gs.webrtc {
                webrtc.end_relay(scope);
            }
        }
        Step::GaveUp(scope) => {
            gs.pending_notices.push(call_relay::NOT_SET_UP_LINE.to_string());
            if let Some(webrtc) = &gs.webrtc {
                webrtc.relay_unavailable(scope);
            }
        }
    }
}

/// The line a busy person sees when someone rang while they were in another call or a room.
pub(crate) fn missed_call_line(name: &str) -> String {
    format!("Missed call from {name}: you were in another call or a voice room.")
}

/// A ring arrived (the message pump hands it here). Idle: it rings. Busy (in a call, ringing
/// either way, or in a voice room): nothing is sent back and it rings out on the caller's side,
/// the way a blocked caller's ring does, and this person sees a missed-call line instead. An
/// automatic "reject" used to go back, which told a caller who cannot see us online (hidden
/// status, BUG-172) that we were there after all (BUG-177, 2026-10-10).
pub(crate) fn on_ring(gs: &mut GuiState, from: String, from_name: String) {
    // Our own "who can reach me" setting for calls (10c), checked here as well as on the relay:
    // a ring that an older or misconfigured server passes on anyway is ignored the way a blocked
    // caller's is (engine/block.rs), with nothing sent back and no line, so it rings out on their
    // side. With no DM store the relay's check stands, as for a DM (engine/reach.rs
    // `file_if_refused`).
    let shares = crate::engine::reach::shares_group(gs, &from);
    if gs.dm_store.as_ref().is_some_and(|s| !s.admits_from(crate::net::reach::ReachKind::Call, &from, shares)) {
        log::info!("call: a ring our call setting does not let through was ignored");
        return;
    }
    let busy = gs.call_active.is_some() || gs.call_incoming.is_some() || gs.call_outgoing.is_some() || gs.voice_active_room.is_some();
    if busy {
        gs.pending_notices.push(missed_call_line(&from_name));
    } else {
        gs.call_incoming = Some((from, from_name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::ChatChannel;

    /// BUG-177: a ring while busy sends nothing back (no "reject" that would say we are online)
    /// and leaves a missed-call line; an idle ring rings. Seen red 2026-10-10 with the old
    /// automatic reject put back: "a busy ring sends nothing back".
    #[test]
    fn a_ring_while_busy_rings_out_and_leaves_a_missed_call_line() {
        let mut gs = app();
        let (client, sent) = crate::net::ws_client::WsClient::recording();
        gs.ws_client = Some(client);
        on_ring(&mut gs, "ab".into(), "Ann".into());
        assert_eq!(gs.call_incoming, Some(("ab".into(), "Ann".into())), "an idle ring rings");
        assert!(gs.pending_notices.is_empty());

        on_ring(&mut gs, "cd".into(), "Cy".into());
        assert_eq!(gs.call_incoming, Some(("ab".into(), "Ann".into())), "the first ring is untouched");
        assert!(sent.try_recv().is_err(), "a busy ring sends nothing back");
        assert_eq!(gs.pending_notices, vec![missed_call_line("Cy")]);

        gs.call_incoming = None;
        gs.voice_active_room = Some("12".into());
        on_ring(&mut gs, "ef".into(), "Ed".into());
        assert_eq!(gs.call_incoming, None, "in a voice room counts as busy");
        assert!(sent.try_recv().is_err());
    }
    use crate::net::call_relay::{RelayStatus, NOT_SET_UP_LINE};

    fn app() -> GuiState {
        let mut gs = GuiState::default();
        gs.profile_public_key = "me".into();
        gs
    }

    /// A call is wanted once accepted; a voice room once the roster lists us, and then while we
    /// stay in it. Seen red 2026-10-09 with the roster check dropped (a room wanted at the
    /// click): "not before the roster lists us".
    #[test]
    fn the_call_or_room_the_app_is_in() {
        let mut gs = app();
        assert_eq!(wanted_scope(&gs), None);
        gs.voice_active_room = Some("12".into());
        gs.chat_channels.push(ChatChannel { id: "12".into(), ..Default::default() });
        assert_eq!(wanted_scope(&gs), None, "not before the roster lists us");
        gs.chat_channels[0].voice_participants = vec![("me".into(), "Me".into())];
        let room = CallScope::Room("12".into());
        assert_eq!(wanted_scope(&gs), Some(room.clone()));

        // Asked once; a roster update that leaves us out does not end it.
        gs.call_relay.update(Some(room.clone()), Instant::now());
        gs.chat_channels[0].voice_participants.clear();
        assert_eq!(wanted_scope(&gs), Some(room.clone()), "still in the room");
        gs.voice_active_room = None;
        assert_eq!(wanted_scope(&gs), None, "left");

        gs.call_active = Some(("ab".into(), "Ab".into()));
        assert_eq!(wanted_scope(&gs), Some(CallScope::Call("ab".into())));
    }

    /// A reply reaches the state machine through `on_frame`; a refusal is said once, at once.
    /// Seen red 2026-10-09 with `on_frame` returning before the refusal was handed on: "the
    /// line is said".
    #[test]
    fn a_refusal_is_said_once() {
        let mut gs = app();
        let call = CallScope::Call("ab".into());
        gs.call_relay.update(Some(call.clone()), Instant::now());
        let refusal = serde_json::json!({ "type": "call_credentials", "call": "ab", "refused": true });
        assert!(on_frame(&mut gs, &refusal));
        assert_eq!(gs.pending_notices, vec![NOT_SET_UP_LINE.to_string()], "the line is said");
        assert_eq!(gs.call_relay.status_of(&call), Some(RelayStatus::Unavailable));
        assert!(on_frame(&mut gs, &refusal));
        assert_eq!(gs.pending_notices.len(), 1, "once");
        assert!(!on_frame(&mut gs, &serde_json::json!({ "type": "reports" })), "not ours");

        // The forwarder failing later is not said again.
        on_event(&mut gs, WebrtcEvent::RelayUnavailable { scope: call, lost: false });
        assert_eq!(gs.pending_notices.len(), 1);
    }

    /// The manager's failure is said once; no manager, nothing in hand.
    #[test]
    fn the_forwarder_failing_is_said_and_a_lost_socket_forgets() {
        let mut gs = app();
        let room = CallScope::Room("12".into());
        gs.call_relay.update(Some(room.clone()), Instant::now());
        let granted = serde_json::json!({
            "type": "call_credentials", "room": "12",
            "urls": ["turn:127.0.0.1:3478?transport=udp"], "username": "1:x", "credential": "c", "ttl": 3600
        });
        assert!(on_frame(&mut gs, &granted));
        assert_eq!(gs.call_relay.status_of(&room), Some(RelayStatus::Connecting));
        on_event(&mut gs, WebrtcEvent::RelayUnavailable { scope: room.clone(), lost: false });
        assert_eq!(gs.pending_notices, vec![NOT_SET_UP_LINE.to_string()]);
        pump(&mut gs); // no WebRTC manager (the socket went away)
        assert_eq!(gs.call_relay.scope(), None, "forgotten with the manager");
    }
}
