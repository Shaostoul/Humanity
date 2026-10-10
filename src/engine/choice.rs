//! The choice for each friend syncs as its own note (section 10n of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): the desktop app's half that touches the app
//! state and the socket. The note's words are net/choice.rs; the stored choice, with its time, is
//! the DM store's (net/dm_store.rs `choices`); the passes that follow it are engine/dm.rs
//! (`follow_choice`).
//!
//! WHY. Three reviews in one day (10l, 10m and a review of 10m) kept finding ways a choice made on
//! one of a person's devices was lost or reversed on another: an untick made offline, a refused
//! pass whose self-copy went out early, an older echo read after a newer one, a device that missed
//! a withdrawal. One root: every device rebuilt the ticks from echoes of passes. Now the choice is
//! a note to ourselves, applied once, the newer winning, and the passes follow the choice.
//!
//! - N1 `make`: a tick changed here sets the choice {may, at = now} and sends the note to our own
//!   mailbox on every connected server. With the server we are on not connected it waits in the
//!   DM store (one per friend, a newer one replacing an older, kept across restarts) and goes on
//!   the next connection BEFORE that friend's withdrawals and passes (`flush`, first thing in the
//!   pass sweep).
//! - N2 `screens_dm`, `apply_in`: a note from us to us is applied once (by its signature); it
//!   replaces the stored choice only when newer, an equal time going to the larger `may` text; one
//!   about someone we blocked is ignored; one from anyone else is dropped unread. None is ever
//!   shown as a message.
//! - N3: an Unfollow whose notice could not go out waits the same way (`flush`).
//! - N4: whenever the choice changes, here or by a note, every pass beyond it is withdrawn at once
//!   (engine/dm.rs `withdraw_beyond_choice`); the pass sweep gives a pass carrying it.
//! - N6: a note about a friend marked "changed on my other device" (10m R3) clears the mark.
//!
//! N7 (the sweep waits for the mailbox) is engine/dm.rs `mailbox_read`.

use crate::gui::GuiState;
use crate::net::choice::ChoiceNote;
use crate::net::dm_pq::DmInner;
use crate::net::dm_store::DmStore;

/// N1: the person changed what `peer` may do, on this device, to `may` (canonical). The choice is
/// kept with its time, the note goes to my other devices (or waits), and then the passes follow
/// it: those beyond it withdrawn at once, and one carrying it sent when none stands.
pub(crate) fn make(gs: &mut GuiState, peer: &str, may: &str) {
    let Some(store) = gs.dm_store.as_mut() else { return };
    let at = store.choose(peer, may);
    store.save();
    send_or_queue(gs, &ChoiceNote::new(peer, may), at);
    crate::engine::dm::follow_choice(gs, peer);
}

/// Send `note`, signed at `at`, now if the server we are on is connected; otherwise keep it in its
/// DM store for the next connection. The choice belongs to this server's store, so its own
/// mailbox must get the note: a note sent only to a parked server would never reach my devices
/// that use this one.
fn send_or_queue(gs: &mut GuiState, note: &ChoiceNote, at: u64) {
    if send_note(gs, note, at) == 0 {
        if let Some(store) = gs.dm_store.as_mut() {
            store.queue_choice(&note.key, &note.may, at);
            store.save();
        }
    }
}

/// Deposit `note` in our own mailbox on the server we are on and on every parked server we are
/// connected to (one `dm_put` each), remembering it as seen so its echo is not applied again.
/// Returns how many servers it went to; 0 (the server we are on is not connected, or the identity
/// is locked) sends and remembers nothing.
fn send_note(gs: &mut GuiState, note: &ChoiceNote, at: u64) -> usize {
    if !gs.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
        return 0;
    }
    let Some((put, id)) = crate::engine::block::note_put_text(gs, &note.text(), at) else { return 0 };
    if let Some(store) = gs.dm_store.as_mut() {
        store.first_sight_note(&id);
        store.save();
    }
    crate::engine::block::to_my_mailboxes(gs, &put.to_string())
}

/// N1 and N3, first thing in every pass sweep (so on every connection, once its mailbox was read):
/// the choice notes made while not connected, oldest first, then the Unfollows whose notice could
/// not go out, both copies each, signed with the time the person made them. Any that still cannot
/// go out wait for the next one. An Unfollow of someone followed again, or blocked, since is void.
pub(crate) fn flush(gs: &mut GuiState) {
    let waiting = gs.dm_store.as_mut().map(|s| s.take_pending_choices()).unwrap_or_default();
    for p in waiting {
        send_or_queue(gs, &ChoiceNote::new(&p.peer, &p.may), p.at);
    }
    let unfollows = gs.dm_store.as_mut().map(|s| s.take_pending_unfollows()).unwrap_or_default();
    for u in unfollows {
        // Followed again since: void. Blocked since: nothing is ever sent to them (10d, 4.7), and
        // the block's own note already tells my other devices to stop following them.
        if gs.dm_store.as_ref().is_some_and(|s| s.is_following(&u.peer)) || crate::engine::block::is_blocked(gs, &u.peer) {
            continue;
        }
        if !crate::engine::dm::send_dm_control_at(gs, &u.peer, crate::net::dm_pq::CTL_UNFOLLOW, None, u.at) {
            if let Some(store) = gs.dm_store.as_mut() {
                store.queue_unfollow(&u.peer, u.at);
            }
        }
    }
    if let Some(store) = gs.dm_store.as_ref() {
        store.save();
    }
}

/// N2, THE arrival rule on the server we are on (engine/dm.rs `ingest_dm`, right after Block's):
/// a DM whose text starts with the marker is never a message, readable or not. A usable note from
/// us to us is applied (`apply_in`), and when that withdrew passes the withdrawals go now. True
/// when the DM was a note (taken here).
pub(crate) fn screens_dm(gs: &mut GuiState, inner: &DmInner) -> bool {
    if !ChoiceNote::is_note(&inner.text) {
        return false;
    }
    let me = gs.profile_public_key.clone();
    let blocked = ChoiceNote::parse(&inner.text).is_some_and(|n| crate::engine::block::is_blocked(gs, &n.key));
    let Some(store) = gs.dm_store.as_mut() else { return true };
    if apply_in(store, &me, inner, blocked) {
        store.save();
        crate::engine::dm::send_pending_withdrawals(gs);
        crate::engine::dm::refresh_social_mirrors(gs);
    }
    true
}

/// The same rule on a parked server's store (engine/bg_connections.rs `bg_screen`): the note is
/// applied to that server's choice and passes; its withdrawals wait there for that server's next
/// sweep. The caller saves. True when the DM was a note.
pub(crate) fn screens_dm_parked(gs: &mut GuiState, store: &mut DmStore, inner: &DmInner) -> bool {
    if !ChoiceNote::is_note(&inner.text) {
        return false;
    }
    crate::engine::block::ensure_block_list(gs);
    let blocked = ChoiceNote::parse(&inner.text).is_some_and(|n| crate::engine::block::is_blocked(gs, &n.key));
    apply_in(store, &gs.profile_public_key, inner, blocked);
    true
}

/// Apply a choice note to one server's store (N2, N4, N6). Only a note from us AND to us counts
/// (signed by us, so only our own devices can write one); any other is dropped unread. A note
/// that does not parse, names ourselves, or names someone we blocked (`blocked`) changes nothing.
/// Each note is applied once, by its signature. A note about a friend clears their "changed on my
/// other device" mark (N6), and a choice newer than the stored one replaces it and withdraws at
/// once every pass to them, on record or on its way, that allows more (N4); the pass carrying it
/// comes from the pass sweep. True when the store changed (the caller saves).
pub(crate) fn apply_in(store: &mut DmStore, me: &str, inner: &DmInner, blocked: bool) -> bool {
    if inner.from != me || inner.to != me {
        log::warn!("a choice note not from us to us was dropped unread");
        return false;
    }
    let Some(note) = ChoiceNote::parse(&inner.text) else { return false };
    if note.key == me || blocked || !store.first_sight_note(&inner.dedupe_key()) {
        return false;
    }
    store.clear_changed_elsewhere(&note.key);
    if store.apply_choice(&note.key, &note.may, inner.ts) {
        store.withdraw_passes_to_except(&note.key, |p| !crate::net::reach::grants_beyond(&p.may, &note.may));
    }
    true
}

// 10n's proof list on the desktop, each rule and the five two-device sequences from the reviews,
// is src/engine/choice_tests.rs, a child of engine/put_answer_tests.rs (whose app-with-a-recording-
// socket helpers it shares). The note's bytes against the test vector: net/choice.rs.
