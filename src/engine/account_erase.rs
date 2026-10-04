//! An erased account, the engine's side (BUG-135): what the app does when a server confirms
//! it erased our account (the relay's `account_erased`, sent after the erase's receipt).
//!
//! The GUI side closes the connection and remembers the server for this identity
//! (gui/connections.rs); here the config is saved, so a restart does not dial that server
//! either, and the game leaves the shared world with the sentence that says how to come back
//! (`home_plot::ERASED`, the Chat page's Connect), or, when the erase did not finish on the
//! server, how to finish it (`ERASE_UNFINISHED_NOTE`). An erase made while the game stood in
//! the world already got `game_join_denied` (relay/handlers/home_plots.rs
//! `leave_world_for_erase`), which refused the shared world with `ERASED`; one made before
//! Enter World got nothing, so Enter World joined and claimed a plot under the erased key. Both
//! now end the same way: no connection, so no join, until the person presses Connect.

use crate::engine::home_plot;
use crate::engine::state::EngineState;
use crate::gui::EraseOutcome;

/// The receipt came on the ACTIVE connection (engine/frame_ws_poll.rs). `frame` is the relay's
/// `account_erased` message.
pub(crate) fn on_active_server(state: &mut EngineState, frame: &serde_json::Value) {
    let outcome = EraseOutcome::from_receipt(frame);
    let server = home_plot::active_server_key(&state.gui_state);
    state.gui_state.account_erased_on_active(outcome);
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
    if let Some(sentence) = erase_refusal(outcome, state.gui_state.copresence_refused_note.as_deref()) {
        home_plot::refuse_shared_world(state, sentence.to_string(), None);
    }
    log::warn!("Account erase on {server} ({outcome:?}): disconnected; it is dialed again only by the Chat page's Connect");
}

/// The receipt came on a PARKED connection (engine/bg_connections.rs): the person switched
/// servers before it arrived. The game left that server's world at the switch
/// (`home_plot::follow_server`), so only the link is closed and remembered.
pub(crate) fn on_parked_server(state: &mut EngineState, ci: usize, frame: &serde_json::Value) {
    state.gui_state.account_erased_on_parked(ci, EraseOutcome::from_receipt(frame));
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
}

/// The sentence the game leaves the shared world with after an erase, or None when it already
/// shows that very sentence (an erase made in the world got the relay's `game_join_denied`
/// first). Decided by the sentence on show, never by whether this server already refused us:
/// a server can hold a refusal for another reason (another ship, our own ship not loading, a
/// refused welcome), and the review of BUG-135 found that an erase then left that stale
/// sentence under the HUD. Pure.
pub(crate) fn erase_refusal(outcome: EraseOutcome, showing: Option<&str>) -> Option<&'static str> {
    let sentence = match outcome {
        EraseOutcome::Erased => home_plot::ERASED,
        EraseOutcome::Unfinished => crate::gui::ERASE_UNFINISHED_NOTE,
    };
    (showing != Some(sentence)).then_some(sentence)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review of BUG-135, finding 1: the "say it once" check compared the server, so an erase on
    /// a server that already refused us for another reason (here, another ship) kept the stale
    /// sentence under the HUD. And an unfinished erase replaces the relay's in-world "erased"
    /// sentence, which promised a fresh sign-up.
    ///
    /// Seen red 2026-10-04 on 825aa0af4 with the old rule (skip whenever a refusal is held):
    /// "assertion `left == right` failed: a stale refusal stayed under the HUD / left: None".
    #[test]
    fn an_erase_replaces_any_other_refusal_and_says_its_sentence_once() {
        let erased = Some(home_plot::ERASED);
        let unfinished = Some(crate::gui::ERASE_UNFINISHED_NOTE);
        assert_eq!(erase_refusal(EraseOutcome::Erased, Some(home_plot::SHIP_MISMATCH)), erased, "a stale refusal stayed under the HUD");
        assert_eq!(erase_refusal(EraseOutcome::Erased, Some(home_plot::OWN_SHIP)), erased);
        assert_eq!(erase_refusal(EraseOutcome::Erased, None), erased, "an erase before Enter World says it too");
        assert_eq!(erase_refusal(EraseOutcome::Erased, erased), None, "said twice after the relay's own refusal");
        assert_eq!(erase_refusal(EraseOutcome::Unfinished, erased), unfinished, "an unfinished erase kept the sign-up promise");
        assert_eq!(erase_refusal(EraseOutcome::Unfinished, unfinished), None);
    }

    /// The two sockets hand the relay's `account_erased` here. The handlers take the whole
    /// `EngineState` (a renderer, an audio device, a game world), which no unit test can build,
    /// so this reads the two dispatch lines the way tests/engine_wiring_lint.rs reads lib.rs:
    /// deleting either one leaves the receipt unhandled on that socket and fails here.
    ///
    /// Seen red 2026-10-04 on 825aa0af4 with both dispatch lines deleted: "the active socket does not
    /// hand account_erased here" (the first assertion; the parked line's is the second).
    #[test]
    fn both_sockets_hand_the_erase_receipt_here() {
        let read = |rel: &str| {
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
                .unwrap_or_else(|e| panic!("read {rel}: {e}"))
        };
        let active = r#"Some("account_erased") => crate::engine::account_erase::on_active_server(state, &val),"#;
        let parked = r#"Some("account_erased") => crate::engine::account_erase::on_parked_server(state, ci, &val),"#;
        assert!(read("src/engine/frame_ws_poll.rs").contains(active), "the active socket does not hand account_erased here");
        assert!(read("src/engine/bg_connections.rs").contains(parked), "a parked socket does not hand account_erased here");
    }
}
