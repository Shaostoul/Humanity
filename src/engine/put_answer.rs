//! The server's answers to the `dm_put`s that change friendship state (section 10l of
//! docs/design/blocking-and-safe-mode.md, "A pass counts as given only once the server took it",
//! 2026-10-10): `dm_put_ok` and `dm_put_refused`, and the 30-second silence that counts as a
//! refusal. The sends themselves go out through engine/dm.rs `send_held`; what is held is
//! net/put_answers.rs.
//!
//! - `dm_put_ok`: the pass is recorded as given, the passes a re-issue replaces are withdrawn, and
//!   so is any older pass to them whose answer never came; our self-copy goes out (our other
//!   devices learn of it only now, so they never adopt a pass the server had not taken).
//! - `dm_put_refused`: never stored, so nothing is recorded or withdrawn and it is forgotten. The
//!   friend stays owed a pass, and the next sweep sends one with the same intended `may` (the
//!   ticks live apart from the record, so they are untouched).
//! - No answer in 30 seconds: the same, except that the pass stays on the store's list of passes
//!   never answered ("perhaps given"), because the server may have stored it after its answer was
//!   lost; it never counts as given, and is withdrawn by Unfollow, by Block, by a tick taken away,
//!   and by the next pass to that friend the server does take.
//!
//! One pass put per friend is on its way at a time: the sweep, a re-issue and a new pass all
//! leave a friend alone while one waits for its answer.
//!
//! A contact request's pass is settled the same way; one not taken also puts the conversation's
//! notice back to offering Send request, saying the server did not confirm it.
//!
//! The message pump (frame_ws_poll.rs) reaches this file through one call in its catch-all chain,
//! `on_frame`; it sits at its file-size budget.

use crate::gui::GuiState;
use crate::net::put_answers::{Answer, Held, PendingPut};

/// What the conversation's notice says when the server did not confirm a contact request (refused
/// for a reason other than their settings, or no answer): the request may not have arrived.
pub(crate) const REQUEST_NOT_CONFIRMED: &str = "The server did not confirm the request. You can send it again.";

/// `dm_put_ok` / `dm_put_refused` from the relay. True when the frame was one of them (handled
/// here, even when its send was already settled), so the pump logs nothing for it.
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) -> bool {
    let Some(answer) = Answer::read(frame) else { return false };
    // An answer after the 30 seconds finds nothing: that send already counts as not taken, and
    // the pass it carried stays among the unanswered, to be withdrawn with the others.
    let Some(put) = gs.pending_puts.take(answer.reference()) else { return true };
    match answer {
        Answer::Taken(_) => taken(gs, put),
        Answer::Refused(_, reason) => not_taken(gs, put, Some(&reason)),
    }
    true
}

/// Sends that have waited 30 seconds: not taken (engine/dm.rs `pace_owed_passes`, every frame).
pub(crate) fn expire(gs: &mut GuiState, now: std::time::Instant) {
    for put in gs.pending_puts.expired(now) {
        not_taken(gs, put, None);
    }
}

/// The server took it: record the pass, withdraw what a re-issue replaces and any older pass to
/// them whose answer never came, send our self-copy.
fn taken(gs: &mut GuiState, put: PendingPut) {
    let on_its_way = gs.pending_puts.serials_to(&put.peer);
    let Some(store) = gs.dm_store.as_mut() else { return };
    let serial = put.pass.serial.clone();
    // False when it was withdrawn while on its way (a tick taken away, an unfollow, a block, our
    // other device's re-issue): it is not brought back as given, and nobody else hears of it.
    let recorded = store.pass_taken(&put.peer, &serial);
    if recorded {
        if put.held == (Held::Pass { reissue: true }) {
            store.withdraw_passes_to_except(&put.peer, |p| p.serial == serial);
        }
        // An older pass whose answer was lost may be standing with them too: the server may
        // have stored it. They hold this one now, so that one goes.
        store.withdraw_unanswered_to_except(&put.peer, |p| on_its_way.contains(&p.serial));
    }
    store.save();
    // A tick changed again while this one was on its way, or it was withdrawn on the way: the
    // sweep runs again at the server's pace (engine/dm.rs `pace_owed_passes`).
    let behind = store.friends_without_pass().contains(&put.peer) || store.passes_out_of_step().contains(&put.peer);
    if behind {
        gs.pass_pacer.held = true;
    }
    if !recorded {
        return;
    }
    if let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) {
        client.send(&put.self_copy.to_string());
    }
    crate::engine::dm::send_pending_withdrawals(gs);
    crate::engine::dm::refresh_social_mirrors(gs);
}

/// Refused (`reason`), or no answer at all (None): nothing recorded, nothing withdrawn, and the
/// friend still owed a pass. A refused pass was never stored and is forgotten; an unanswered one
/// stays among the unanswered.
fn not_taken(gs: &mut GuiState, put: PendingPut, reason: Option<&str>) {
    if reason.is_some() {
        if let Some(store) = gs.dm_store.as_mut() {
            store.pass_refused(&put.peer, &put.pass.serial);
            store.save();
        }
    }
    let sent_line = gs.reach.refused.get(&put.peer) == Some(&crate::net::reach::Refusal::RequestSent);
    // Turned away by their settings: `reach_refused` says so in the conversation already.
    if put.held == Held::Request && reason != Some("reach") && sent_line {
        gs.reach.refused.insert(put.peer.clone(), crate::net::reach::Refusal::Refused);
        gs.reach.status = REQUEST_NOT_CONFIRMED.to_string();
    }
}

#[cfg(test)]
#[path = "put_answer_tests.rs"]
mod tests;
