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
    // `earlier`: the server remembered an erase made before (from another device) and signed
    // nothing up on this connect (relay handlers/sign_ups.rs); handled the same way. This
    // socket is never signed in, so `ws_identified` stays false, and the shared-world follow
    // keeps the sentence because this server is now erased here (home_plot.rs
    // `server_follow`, review finding 13), instead of taking it for a fresh connection.
    let when = if frame.get("earlier").and_then(|v| v.as_bool()) == Some(true) { "earlier" } else { "now" };
    log::warn!("Account erase on {server} ({outcome:?}, {when}): disconnected; it is dialed again only by the Chat page's Connect");
}

/// The receipt came on a PARKED connection (engine/bg_connections.rs): the person switched
/// servers before it arrived. The game left that server's world at the switch
/// (`home_plot::follow_server`), so only the link is closed and remembered.
pub(crate) fn on_parked_server(state: &mut EngineState, ci: usize, frame: &serde_json::Value) {
    state.gui_state.account_erased_on_parked(ci, EraseOutcome::from_receipt(frame));
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
}

/// What the game says under the HUD when a server ADMIN erased this identity's data there (10i,
/// the relay's `account_erased` with `by_admin: true`): 10i's words, so the person is not told
/// they erased it themselves, then the way back, as the self-erase's sentence gives it.
pub(crate) const ERASED_BY_ADMIN_HUD: &str = "A server admin erased your data from this server. Local data on your own devices is untouched. Out of the shared world: to come back, open Chat and press Connect, which signs you up again as a new account.";

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
        EraseOutcome::ErasedByAdmin => ERASED_BY_ADMIN_HUD,
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

    /// 10i: an admin's erase (`by_admin: true` on the relay's `account_erased`) puts 10i's words
    /// under the HUD in place of the relay's in-world "your account on this server was erased",
    /// which it sent first; a self-erase (`by_admin` false or absent) keeps the old sentence.
    ///
    /// Seen red 2026-10-10 with the by-admin outcome mapped to the self-erase's sentence: "an
    /// admin's erase left the self-erase's words under the HUD".
    #[test]
    fn an_admins_erase_says_so_under_the_hud() {
        let frame = |by_admin: Option<bool>| {
            let mut f = serde_json::json!({ "type": "account_erased", "partial": false, "earlier": false });
            if let Some(b) = by_admin {
                f["by_admin"] = serde_json::Value::Bool(b);
            }
            EraseOutcome::from_receipt(&f)
        };
        let said = erase_refusal(frame(Some(true)), Some(home_plot::ERASED));
        assert_eq!(said, Some(ERASED_BY_ADMIN_HUD), "an admin's erase left the self-erase's words under the HUD");
        assert!(ERASED_BY_ADMIN_HUD.starts_with(crate::gui::ERASED_BY_ADMIN_NOTE));
        assert!(ERASED_BY_ADMIN_HUD.contains("Chat") && ERASED_BY_ADMIN_HUD.contains("Connect") && ERASED_BY_ADMIN_HUD.contains("signs you up again"));
        assert_eq!(erase_refusal(frame(Some(true)), Some(ERASED_BY_ADMIN_HUD)), None, "said twice");
        assert_eq!(erase_refusal(frame(Some(false)), None), Some(home_plot::ERASED), "a self-erase lost its words");
        assert_eq!(erase_refusal(frame(None), None), Some(home_plot::ERASED), "a self-erase lost its words");
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
