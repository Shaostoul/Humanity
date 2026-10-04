//! An erased account, the engine's side (BUG-135): what the app does when a server confirms
//! it erased our account (the relay's `account_erased`, sent after the erase's receipt).
//!
//! The GUI side closes the connection and remembers the server for this identity
//! (gui/connections.rs); here the config is saved, so a restart does not dial that server
//! either, and the game leaves the shared world with the sentence that says how to come back
//! (`home_plot::ERASED`, the Chat page's Connect). An erase made while the game stood in the
//! world already got `game_join_denied` (relay/handlers/home_plots.rs `leave_world_for_erase`),
//! which refused the shared world with that sentence; one made before Enter World got nothing,
//! so Enter World joined and claimed a plot under the erased key. Both now end the same way:
//! no connection, so no join, until the person presses Connect.

use crate::engine::home_plot;
use crate::engine::state::EngineState;

/// The receipt came on the ACTIVE connection (engine/frame_ws_poll.rs).
pub(crate) fn on_active_server(state: &mut EngineState) {
    let server = home_plot::active_server_key(&state.gui_state);
    state.gui_state.account_erased_on_active();
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
    // Said once: an erase in the world was already refused with this sentence.
    if state.copresence_refused.as_deref() != Some(server.as_str()) {
        home_plot::refuse_shared_world(state, home_plot::ERASED.to_string(), None);
    }
    log::warn!("Account erased on {server}: disconnected; it is dialed again only by the Chat page's Connect");
}

/// The receipt came on a PARKED connection (engine/bg_connections.rs): the person switched
/// servers before it arrived. The game left that server's world at the switch
/// (`home_plot::follow_server`), so only the link is closed and remembered.
pub(crate) fn on_parked_server(state: &mut EngineState, ci: usize) {
    state.gui_state.account_erased_on_parked(ci);
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
}
