//! Joining and leaving a channel's voice room, the one place both happen: the voice icon on a
//! channel row (chat/left_panel.rs), and the protected setup (step G of
//! docs/design/blocking-and-safe-mode.md, 10h), whose PIN gates a join and whose review step's
//! Remove for a room is a leave.
//!
//! Moved verbatim from the channel list's "apply voice toggle after the loop" block in
//! chat/left_panel.rs, keyed by the room id rather than the row's index, so a join the PIN let
//! through a frame later still reaches the right room. Takes `use super::*` like the page's other
//! children.

use super::*;

/// Join (`joining`) or leave the voice room of channel `room_id` on the server the app is on. The
/// relay tracks voice rooms by the channel's own id (v0.493) and expects a `voice_room` message
/// with action join/leave (the old `voice_join` was ignored, so native join never registered;
/// Phase C, v0.491). A join asks the protected setup's PIN first while it is on; a leave never
/// does.
pub(crate) fn set_voice_room(state: &mut GuiState, room_id: &str, joining: bool) {
    if joining && !crate::engine::protected::allows(state, crate::net::protected::ProtectedAction::JoinVoice(room_id.to_string())) {
        return;
    }
    let (ch_name, ch_id) = match state.chat_channels.iter_mut().find(|c| c.id == room_id) {
        Some(ch) => {
            ch.voice_joined = joining;
            (ch.name.clone(), ch.id.clone())
        }
        None => (String::new(), String::new()),
    };
    if ch_id.is_empty() {
        return;
    }
    let action = if joining { "join" } else { "leave" };
    log::info!("Voice {} requested: {} (room_id {})", action, ch_name, ch_id);
    crate::debug::push_debug(format!("Voice: {} for '{}' (id {})", action, ch_name, ch_id));
    // Phase C: track the active room so the roster handler dials the incumbents
    // (newcomer-offers rule). Reset the incumbent-capture flag on each join.
    if joining {
        state.voice_active_room = Some(ch_id.clone());
        state.voice_incumbents_captured = false;
    } else {
        state.voice_active_room = None;
    }
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({
                "type": "voice_room",
                "action": action,
                "room_id": ch_id,
            });
            client.send(&msg.to_string());
        }
    }
}

/// Should a live voice session run: is the app in a voice room or a 1:1 call? The frame starts and
/// stops the session on it (lib.rs), and the network pump drops a peer's voice frame without it
/// (engine/frame_net.rs), so nothing feeds a session that is not wanted.
pub(crate) fn voice_wanted(state: &GuiState) -> bool {
    state.voice_active_room.is_some() || state.call_active.is_some()
}

/// What [`leave_all_voice`] ended.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct LeftVoice {
    /// A voice room or a 1:1 call (in it, or ringing out) was left.
    pub voice: bool,
    /// Who was ringing in, unanswered (no reply is sent: it rings out on their side, as a busy
    /// ring does, BUG-177).
    pub missed_ring: Option<String>,
}

/// Leave every voice thing at once, the way a person's own Leave, Hang up and Cancel do, and close
/// the microphone: the voice room (`set_voice_room`), the 1:1 call (`hang_up_call`), an outgoing
/// ring (`cancel_outgoing_call`), the Settings microphone test, and the live session with its
/// queued frames both ways. Closing the window into the tray runs it (notifications rung 2,
/// part 2, engine/tray_window.rs): a hidden window must never keep a microphone open, and its
/// sound would come in bursts with no frame to play it. An incoming ring is let ring out, unsent.
pub(crate) fn leave_all_voice(state: &mut GuiState) -> LeftVoice {
    let mut left = LeftVoice::default();
    if let Some(room) = state.voice_active_room.clone() {
        set_voice_room(state, &room, false);
        if state.voice_active_room.take().is_some() {
            // A room no longer in this server's list (`set_voice_room` finds no row): the leave
            // is still said, so the server takes this person out of it.
            if let Some(client) = state.ws_client.as_ref().filter(|c| c.is_connected()) {
                client.send(&serde_json::json!({ "type": "voice_room", "action": "leave", "room_id": room }).to_string());
            }
        }
        left.voice = true;
    }
    if state.call_active.is_some() {
        super::modals::hang_up_call(state);
        left.voice = true;
    }
    if state.call_outgoing.is_some() {
        super::modals::cancel_outgoing_call(state);
        left.voice = true;
    }
    if let Some((_, name)) = state.call_incoming.take() {
        left.missed_ring = Some(name);
    }
    if state.mic_test_active || state.mic_test_prev {
        state.mic_test_active = false;
        state.mic_test_prev = false;
        crate::net::voice::stop_mic_test();
    }
    if state.voice_session_prev {
        crate::net::voice::stop_voice_session();
        state.voice_session_prev = false;
    }
    state.voice_connected_peers.clear();
    crate::net::voice::clear_voice_queues();
    left
}
