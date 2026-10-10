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
