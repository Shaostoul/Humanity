//! Block (step C of docs/design/blocking-and-safe-mode.md, section 10d, 2026-10-09): the native
//! client's half that touches the app state and the socket. The list itself, and the notes to
//! oneself that carry it to the person's other devices, are src/net/block_list.rs.
//!
//! WHAT BLOCK DOES, at once and with no confirmation (it is undoable):
//! 1. adds their identity key, never a name, to the block list, with the date;
//! 2. takes back every pass we gave them (`cert_revoke` for each serial, step A), our follow, our
//!    "People I choose" ticks for them (10c-ii) and any request of theirs, on the server we are
//!    on now; on every other server the same happens the next time we are on it (`sweep`).
//!    Under the safe defaults the relay then refuses their messages, calls and trades by itself;
//! 3. hides everything from them on every path in section 4.5's client column: DMs, knocks,
//!    follow notices and contact requests are dropped before they are stored or notified
//!    (`screens_dm`); posts, replies, reactions and typing in channels and groups are hidden by
//!    key (`screens_out`, `hides_message`, `visible_reactions`); call rings are ignored with no
//!    reject sent, direct-connection offers are never answered (`screens_out`, and the offer
//!    gate in frame_ws_poll_offers.rs), and trade requests are left unanswered (`hides_trade`);
//! 4. says one line, `BLOCKED_LINE`. Nothing is ever sent to the blocked person (4.7): not an
//!    unfollow notice, not a call hangup, not a trade refusal.
//!
//! Unblock takes them off the list. It does not follow them again or give a pass: becoming
//! friends again is a fresh follow or contact request. The pass THEY gave us is kept through
//! both: it is theirs to withdraw.
//!
//! Our other devices learn either from a note to ourselves (net/block_list.rs): one signed sealed
//! DM from us to us, one `dm_put` to our own mailbox on each server we are connected to, or, with
//! none connected, kept and sent on the next connection (`flush_notes`). The echo of our own
//! follow or pass for someone we blocked, sent by a device that had not heard of the block yet,
//! does not bring it back: the block wins and the pass is withdrawn again (`own_echo_for_blocked`).
//!
//! In the game (2026-10-10), the relay sends each player's identity key with their name, and a
//! blocked key's figure and name are not drawn (`hides_player`, read by the figure pass in
//! lib.rs and by `net_route::nameplate_labels`). They still stand there; only we stop seeing them.
//!
//! The message pump (frame_ws_poll.rs) reaches this file through `screens_out` on the line that
//! reads each frame; it sits at its file-size budget, so nothing more goes there.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::gui::GuiState;
use crate::net::block_list::{BlockList, BlockNote};
use crate::net::dm_pq::DmInner;
use crate::net::dm_store::DmStore;

/// The one line Block shows (10d, word for word).
pub(crate) const BLOCKED_LINE: &str = "Blocked. You will not see anything from them. They are not told.";

/// The line Unblock shows.
pub(crate) const UNBLOCKED_LINE: &str =
    "Unblocked. They are not told. To be friends again, follow them or send a contact request.";

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Make sure the block list of the identity unlocked now is loaded (and reload it when another
/// identity has been unlocked since). False while there is no identity to key it by.
pub(crate) fn ensure_block_list(gs: &mut GuiState) -> bool {
    if gs.profile_public_key.is_empty() {
        return false;
    }
    if gs.block_list.as_ref().is_some_and(|l| l.identity() == gs.profile_public_key) {
        return true;
    }
    let Some(seed) = gs.private_key_bytes.as_ref() else { return false };
    gs.block_list = Some(BlockList::load(seed, &gs.profile_public_key));
    true
}

/// Is `key` someone we blocked? Never our own key, never an empty one. Read on every message
/// drawn, so it does no loading (`ensure_block_list` runs with the DM store, engine/dm.rs).
pub(crate) fn is_blocked(gs: &GuiState, key: &str) -> bool {
    !key.is_empty() && key != gs.profile_public_key && gs.block_list.as_ref().is_some_and(|l| l.is_blocked(key))
}

/// Is `player` someone we blocked? Their figure is not drawn and their name is not shown in the
/// game (section 4.5, "Game figure and name"). They still stand there: this only stops us seeing
/// them. A player whose key the relay has not sent yet is drawn.
pub(crate) fn hides_player(gs: &GuiState, player: &crate::net::sync::RemotePlayer) -> bool {
    player.key.as_deref().is_some_and(|k| is_blocked(gs, k))
}

// ── The paths in (section 4.5's client column) ──────────────────────────────────────────────

/// THE arrival rule for a verified DM, before anything is stored, listed or notified (engine/dm.rs
/// `ingest_dm`): a note to ourselves about the block list is applied and never shown, and one
/// that is from or to anyone else is ignored; anything a blocked key sent (a message, a knock, a
/// follow notice, a pass, a contact request) is dropped. True when the DM was taken here.
pub(crate) fn screens_dm(gs: &mut GuiState, inner: &DmInner) -> bool {
    if BlockNote::is_note(&inner.text) {
        apply_note(gs, inner);
        return true;
    }
    if inner.from == gs.profile_public_key && is_blocked(gs, &inner.to) {
        let Some(store) = gs.dm_store.as_mut() else { return false };
        if !own_echo_for_blocked(store, inner) {
            return false;
        }
        store.save();
        crate::engine::dm::send_pending_withdrawals(gs);
        crate::engine::dm::refresh_social_mirrors(gs);
        return true;
    }
    inner.from != gs.profile_public_key && is_blocked(gs, &inner.from)
}

/// The same rule on a parked server's store (engine/bg_connections.rs): a note to ourselves is
/// applied to the list and, for a block, to that server's store too; our own echo of a follow or
/// a pass for someone we blocked is taken back there; a blocked key's DM is dropped. The caller
/// saves. True when the DM was taken.
pub(crate) fn screens_dm_parked(gs: &mut GuiState, store: &mut DmStore, inner: &DmInner) -> bool {
    ensure_block_list(gs);
    if BlockNote::is_note(&inner.text) {
        if let Some(BlockNote::Block(key)) = apply_note(gs, inner) {
            enforce_on_store(store, &key);
        }
        return true;
    }
    if inner.from == gs.profile_public_key && is_blocked(gs, &inner.to) {
        return own_echo_for_blocked(store, inner);
    }
    inner.from != gs.profile_public_key && is_blocked(gs, &inner.from)
}

/// Our own follow, pass or contact request for someone we have since blocked, echoed from a
/// device that had not heard of the block yet: the block wins. A pass it gave is recorded so its
/// serial can be withdrawn, then everything is taken back again (`enforce_on_store`). True when
/// the DM was one of those (taken; the caller saves); any other self-copy, such as our own words
/// to them, goes on as usual.
pub(crate) fn own_echo_for_blocked(store: &mut DmStore, inner: &DmInner) -> bool {
    use crate::net::dm_pq::{CTL_FOLLOW, CTL_FRIEND_CERT};
    let text = inner.text.as_str();
    let pass = if text == CTL_FRIEND_CERT {
        inner.cert.clone()
    } else if text.starts_with(crate::net::reach::CONTACT_REQUEST_MARKER) {
        crate::net::reach::parse_contact_request_text(text).map(|(_, pass)| pass)
    } else if text == CTL_FOLLOW {
        None
    } else {
        return false;
    };
    if let Some(Ok((given, _))) = pass.as_deref().map(crate::relay::core::pq_crypto::parse_friend_cert) {
        store.record_pass_sent(&inner.to, crate::net::dm_store::SentPass { serial: given.serial, may: given.may.wire() });
    }
    enforce_on_store(store, &inner.to);
    true
}

/// A note to ourselves: applied when it is from us AND to us (signed by us, so only our own
/// devices can write one); any other is ignored (10d). Notes apply in the order they arrive,
/// each once. A block that changed the list takes back what we gave on the server we are on (no
/// new note is sent: this one already went where it needed to). Returns the note when it
/// changed the list.
fn apply_note(gs: &mut GuiState, inner: &DmInner) -> Option<BlockNote> {
    let me = gs.profile_public_key.clone();
    if inner.from != me || inner.to != me {
        log::warn!("a block note not from us to us was ignored");
        return None;
    }
    let note = BlockNote::parse(&inner.text)?;
    if !ensure_block_list(gs) {
        return None;
    }
    let list = gs.block_list.as_mut()?;
    if !list.first_sight(&inner.dedupe_key()) {
        return None; // the echo of a note we sent, or one delivered again
    }
    let changed = list.apply(&note, inner.ts);
    list.save();
    if !changed {
        return None; // already so, or a note naming ourselves
    }
    match &note {
        BlockNote::Block(key) => {
            take_back(gs, key);
            forget_live(gs, key);
        }
        BlockNote::Unblock(_) => crate::engine::dm::rebuild_dm_sidebar(gs),
    }
    Some(note)
}

/// THE message-pump rule (frame_ws_poll.rs, on the line that reads each frame): frames from a
/// blocked key dropped before the pump acts on them. A channel post (so no unread dot and no
/// mention ding; one already in the list is hidden by `hides_message`), typing, a reaction, a
/// call ring or any call control (ignored silently: the busy auto-reject is never sent), and a
/// direct-connection or call signal (never answered). True = drop it.
pub(crate) fn screens_out(gs: &GuiState, frame: &serde_json::Value) -> bool {
    let kind = frame.get("type").and_then(|t| t.as_str());
    if !matches!(kind, Some("chat" | "typing" | "reaction" | "voice_call" | "webrtc_signal")) {
        return false;
    }
    frame.get("from").and_then(|f| f.as_str()).is_some_and(|from| is_blocked(gs, from))
}

/// Hide a message whose author we blocked: in a channel, a group, a Commons view or a DM
/// conversation (our own words to them stay).
pub(crate) fn hides_message(gs: &GuiState, m: &crate::gui::ChatMessage) -> bool {
    is_blocked(gs, &m.sender_key)
}

/// A message's reactions without those of people we blocked (borrowed as-is when there are none).
pub(crate) fn visible_reactions<'a>(gs: &GuiState, reactions: &'a HashMap<String, Vec<String>>) -> Cow<'a, HashMap<String, Vec<String>>> {
    if !reactions.values().flatten().any(|k| is_blocked(gs, k)) {
        return Cow::Borrowed(reactions);
    }
    let mut kept = reactions.clone();
    for keys in kept.values_mut() {
        keys.retain(|k| !is_blocked(gs, k));
    }
    kept.retain(|_, keys| !keys.is_empty());
    Cow::Owned(kept)
}

/// A trade request from someone we blocked: never listed, never answered (nothing is sent to
/// them). Only a request still waiting on us; a trade already under way or finished stays, so a
/// completed trade still settles.
pub(crate) fn hides_trade(gs: &GuiState, t: &crate::gui::GuiTrade) -> bool {
    t.status == "pending" && t.recipient_key == gs.profile_public_key && is_blocked(gs, &t.initiator_key)
}

// ── Block and Unblock ───────────────────────────────────────────────────────────────────────

/// Block `key` (every Block button). No confirmation: it is undoable.
pub(crate) fn block(gs: &mut GuiState, key: &str) {
    if key.is_empty() || key == gs.profile_public_key {
        return;
    }
    if !ensure_block_list(gs) {
        gs.pending_notices.push("Unlock your identity to block someone.".to_string());
        return;
    }
    let at = now_ms();
    let Some(list) = gs.block_list.as_mut() else { return };
    if !list.block(key, at) {
        return; // already blocked
    }
    list.save();
    take_back(gs, key);
    forget_live(gs, key);
    send_or_queue(gs, &BlockNote::Block(key.to_string()), at);
    gs.pending_notices.push(BLOCKED_LINE.to_string());
}

/// Unblock `key` (Settings > Safety > Blocked people, the DM header, the profile). Nothing is
/// given back: no follow, no pass.
pub(crate) fn unblock(gs: &mut GuiState, key: &str) {
    if !ensure_block_list(gs) {
        return;
    }
    let Some(list) = gs.block_list.as_mut() else { return };
    if !list.unblock(key) {
        return;
    }
    list.save();
    crate::engine::dm::rebuild_dm_sidebar(gs);
    send_or_queue(gs, &BlockNote::Unblock(key.to_string()), now_ms());
    gs.pending_notices.push(UNBLOCKED_LINE.to_string());
}

/// What a block takes back on one server's store: every pass we gave them (their serials join
/// the withdrawals waiting for the relay), our follow, our "People I choose" ticks for them
/// (10c-ii: cleared, so Unblock and a fresh friendship start clean, with any choice note for them
/// still waiting to go out, 10o O4), an Unfollow of them still waiting (10o O3: nothing is sent to
/// someone blocked, and the block's own note tells my other devices), and their entry in Requests.
/// True when anything changed (the caller saves).
pub(crate) fn enforce_on_store(store: &mut DmStore, key: &str) -> bool {
    let mut changed = !store.withdraw_passes_to(key).is_empty();
    if store.is_following(key) {
        store.set_following(key, false);
        changed = true;
    }
    changed |= store.drop_pending_unfollow(key);
    changed |= store.clear_ticks(key);
    changed | store.remove_request(key).is_some()
}

/// On every member list (engine/dm.rs `sweep_friend_passes`): send the notes made while no server
/// was connected, and take back on THIS server what a block made elsewhere (on another server,
/// or on another device) could not, so a block reaches every server the next time we are on it.
pub(crate) fn sweep(gs: &mut GuiState) {
    flush_notes(gs);
    let keys: Vec<String> = gs.block_list.as_ref().map(|l| l.entries().into_iter().map(|(k, _)| k).collect()).unwrap_or_default();
    let Some(store) = gs.dm_store.as_mut() else { return };
    let mut changed = false;
    for key in &keys {
        changed |= enforce_on_store(store, key);
    }
    if changed {
        store.save();
        crate::engine::dm::send_pending_withdrawals(gs);
        crate::engine::dm::refresh_social_mirrors(gs);
    }
}

/// Take back what we gave `key` on the server we are on, and send the withdrawals now if we can.
/// They also leave the protected setup's approved list (step G): befriending them again needs its
/// PIN.
fn take_back(gs: &mut GuiState, key: &str) {
    crate::engine::protected::forget(gs, key);
    // A pass or request to them still waiting for its answer is dropped (10m R6): its answer
    // records nothing (the pass it carried is withdrawn with the rest below).
    gs.pending_puts.drop_peer(key);
    if !crate::engine::dm::ensure_dm_store(gs) {
        return; // not on a server yet: `sweep` does it on the next member list
    }
    if let Some(store) = gs.dm_store.as_mut() {
        if enforce_on_store(store, key) {
            store.save();
        }
    }
    crate::engine::dm::send_pending_withdrawals(gs);
    crate::engine::dm::refresh_social_mirrors(gs);
    crate::engine::dm::rebuild_dm_sidebar(gs);
}

/// Forget what is live from `key` right now, sending them nothing: their typing, a call ringing
/// in or out or under way (its connection is closed, with no hangup sent), and trade requests
/// from them waiting on us.
fn forget_live(gs: &mut GuiState, key: &str) {
    gs.chat_typing_users.remove(key);
    if gs.call_incoming.as_ref().is_some_and(|(k, _)| k == key) {
        gs.call_incoming = None;
    }
    if gs.call_outgoing.as_ref().is_some_and(|(k, _)| k == key) {
        gs.call_outgoing = None;
        gs.call_outgoing_deadline = None;
    }
    if gs.call_active.as_ref().is_some_and(|(k, _)| k == key) {
        gs.call_active = None;
        gs.voice_connected_peers.remove(key);
        if let Some(ref webrtc) = gs.webrtc {
            webrtc.close_peer(key.to_string());
        }
    }
    let me = gs.profile_public_key.clone();
    gs.trades.retain(|t| !(t.status == "pending" && t.recipient_key == me && t.initiator_key == key));
}

// ── The notes to ourselves ──────────────────────────────────────────────────────────────────

/// The `dm_put` of a note to ourselves (one ordinary signed sealed DM: from us, to us, sealed to
/// our own DM key, signed at `at`), and the note's id (`DmInner::dedupe_key`), which this device
/// remembers as it sends so the echo is not applied again. None while the identity is locked.
pub(crate) fn note_put(gs: &GuiState, note: &BlockNote, at: u64) -> Option<(serde_json::Value, String)> {
    note_put_text(gs, &note.text(), at)
}

/// [`note_put`] for any note to ourselves by its text: the choice notes of 10n (engine/choice.rs)
/// travel exactly the way block notes do.
pub(crate) fn note_put_text(gs: &GuiState, text: &str, at: u64) -> Option<(serde_json::Value, String)> {
    let seed = gs.private_key_bytes.as_ref()?;
    let me = &gs.profile_public_key;
    let mine = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).ok()?;
    let inner = crate::net::dm_pq::build_signed_inner(seed, me, me, at, text).ok()?;
    let id = crate::net::dm_pq::parse_verify_inner(&inner).ok()?.dedupe_key();
    let sealed = crate::net::dm_pq::seal_v2(&mine.public_base64(), &inner).ok()?;
    Some((serde_json::json!({ "type": "dm_put", "to": me, "content": sealed }), id))
}

/// Send `note` now if any server is connected; otherwise keep it for the next connection.
fn send_or_queue(gs: &mut GuiState, note: &BlockNote, at: u64) {
    if send_note(gs, note, at) == 0 {
        if let Some(list) = gs.block_list.as_mut() {
            list.queue(note, at);
            list.save();
        }
    }
}

/// Send the notes made while no server was connected (on every member list, so on every new
/// connection). Any that still cannot go out wait for the next one.
pub(crate) fn flush_notes(gs: &mut GuiState) {
    let waiting = gs.block_list.as_mut().map(|l| l.take_pending()).unwrap_or_default();
    if waiting.is_empty() {
        return;
    }
    for pending in waiting {
        send_or_queue(gs, &pending.note(), pending.at);
    }
    if let Some(list) = gs.block_list.as_ref() {
        list.save();
    }
}

/// Deposit `note` in our own mailbox on every server we are connected to (the one we are on and
/// the parked ones; one `dm_put` to our own mailbox on each), so each of our devices learns it
/// whichever server it uses. Returns how many servers it went to; 0 sends nothing and remembers
/// nothing, so the caller can keep it for later.
fn send_note(gs: &mut GuiState, note: &BlockNote, at: u64) -> usize {
    let connected = gs.ws_client.as_ref().is_some_and(|c| c.is_connected())
        || gs.connections.iter().any(|c| c.identified && c.ws.as_ref().is_some_and(|w| w.is_connected()));
    if !connected {
        return 0;
    }
    let Some((put, id)) = note_put(gs, note, at) else { return 0 };
    if let Some(list) = gs.block_list.as_mut() {
        list.first_sight(&id); // our own note: its echo is not applied a second time
        list.save();
    }
    to_my_mailboxes(gs, &put.to_string())
}

/// Send one `dm_put` to our own mailbox on every server we are connected to (the one we are on and
/// the parked ones). Returns how many it went to.
pub(crate) fn to_my_mailboxes(gs: &GuiState, text: &str) -> usize {
    let mut sent = 0;
    if let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) {
        client.send(text);
        sent += 1;
    }
    for conn in gs.connections.iter().filter(|c| c.identified) {
        if let Some(ws) = conn.ws.as_ref().filter(|w| w.is_connected()) {
            ws.send(text);
            sent += 1;
        }
    }
    sent
}

/// Every native item of 10d's proof list that needs no socket. Each test was seen red once on
/// purpose, recorded at the test.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::dm_store::SentPass;
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

    const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

    fn identity(n: u8) -> (Vec<u8>, String) {
        let seed = vec![n; 32];
        (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
    }

    fn inner(from: &str, to: &str, ts: u64, text: &str) -> DmInner {
        DmInner { from: from.into(), to: to.into(), ts, text: text.into(), sig_b64: format!("{from}{to}{ts}{text}"), cert: None }
    }

    /// A signed-in app for `seed` / `me` on SERVER, with its own DM store and its own block list
    /// (unique files, the list in the temp directory).
    fn app(me: &str, seed: &[u8], tag: &str) -> GuiState {
        let mut gs = GuiState::default();
        gs.profile_public_key = me.to_string();
        gs.private_key_bytes = Some(seed.to_vec());
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let mut store = DmStore::load(seed, me, &format!("wss://{tag}-{nanos}.example"));
        store.set_pass_server(SERVER);
        gs.dm_store = Some(store);
        gs.block_list = Some(BlockList::in_temp(seed, me, tag));
        // This connection's mailbox has been read (10n N7), so the pass sweep runs.
        gs.dm_fetch = crate::net::mailbox_fetch::MailboxFetch::already_read();
        gs
    }

    fn tidy(gs: &GuiState) {
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
        gs.block_list.as_ref().unwrap().remove_file_for_test();
    }

    fn default_pass(serial: &str) -> SentPass {
        SentPass { serial: serial.into(), may: "invite,message,trade,voice_message".into() }
    }

    /// Block withdraws every pass we gave and unfollows (10d, item 2), clears our "People I
    /// choose" ticks for them (10c-ii) and drops their request, with the one line and nothing
    /// addressed to them; their typing, their ring and their waiting trade request go too.
    /// Unblock gives nothing back.
    /// Seen red 2026-10-09 with `enforce_on_store`'s withdrawal line taken out: "every pass we
    /// gave is withdrawn" failed (both serials still standing). Seen red 2026-10-10 with
    /// `clear_ticks` taken out of `enforce_on_store`: "Block clears the choice" failed.
    #[test]
    fn block_withdraws_the_passes_and_unfollows() {
        let (seed, me) = identity(141);
        let (_b, ben) = identity(142);
        let mut gs = app(&me, &seed, "block-take-back");
        {
            let store = gs.dm_store.as_mut().unwrap();
            store.record_pass_sent(&ben, default_pass(&"a1".repeat(16)));
            store.record_pass_sent(&ben, default_pass(&"a2".repeat(16)));
            store.set_following(&ben, true);
            store.set_follower(&ben, true);
            store.set_ticks(&ben, crate::net::reach::FriendTicks { message: false, call: true, trade: true });
            store.add_request(crate::net::reach::ContactRequest { key: ben.clone(), ts: 1, pass: String::new() });
        }
        gs.chat_typing_users.insert(ben.clone(), ("Ben".into(), std::time::Instant::now()));
        gs.call_incoming = Some((ben.clone(), "Ben".into()));
        gs.trades.push(crate::gui::GuiTrade { id: "t1".into(), initiator_key: ben.clone(), recipient_key: me.clone(), status: "pending".into(), ..Default::default() });
        gs.trades.push(crate::gui::GuiTrade { id: "t2".into(), initiator_key: ben.clone(), recipient_key: me.clone(), status: "active".into(), ..Default::default() });

        block(&mut gs, &ben);
        assert!(is_blocked(&gs, &ben));
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.cert_sent_to(&ben), "every pass we gave is withdrawn");
        assert_eq!(store.pending_withdrawals(), ["a1".repeat(16), "a2".repeat(16)], "and waits for the relay (cert_revoke for each serial)");
        assert!(!store.is_following(&ben), "we no longer follow them");
        assert!(!store.is_friend(&ben), "so they are no friend");
        assert_eq!(store.ticks(&ben), crate::net::reach::FriendTicks::default(), "Block clears the choice");
        assert!(store.requests().is_empty(), "their request is gone");
        assert!(gs.chat_typing_users.is_empty() && gs.call_incoming.is_none(), "their typing and their ring are forgotten");
        assert_eq!(gs.trades.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["t2"], "their trade request goes; a trade under way stays");
        assert_eq!(gs.pending_notices, [BLOCKED_LINE.to_string()], "the one line");
        assert_eq!(gs.block_list.as_ref().unwrap().entries().len(), 1);

        unblock(&mut gs, &ben);
        assert!(!is_blocked(&gs, &ben));
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.is_following(&ben) && !store.cert_sent_to(&ben), "unblock gives back neither the follow nor a pass");
        tidy(&gs);
    }

    /// A blocked key's DM is not stored or notified, and neither is anything else they send by
    /// DM: a follow notice, a pass, a contact request. Our own self-copy of a message to them is
    /// still ours. Through `ingest_dm` itself, which the dm_new arm's notification ding follows.
    /// Seen red 2026-10-09 with `screens_dm`'s call taken out of `ingest_dm`: "a blocked key's DM
    /// is not ingested" failed (ingest_dm returned true and stored the text).
    #[test]
    fn a_blocked_keys_dm_is_not_stored_or_notified() {
        let (seed, me) = identity(143);
        let (ben_seed, ben) = identity(144);
        let mut gs = app(&me, &seed, "block-dm");
        // Messages from anyone are let through here, so it is the block alone that stops Ben's.
        gs.dm_store.as_mut().unwrap().set_reach_settings(crate::net::reach::ReachSettings {
            message: crate::net::reach::Audience::Anyone,
            ..Default::default()
        });
        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&ben, &me, 1, "before")), "before the block, his DM is a DM");
        assert!(gs.chat_dms.iter().any(|d| d.user_key == ben));
        block(&mut gs, &ben);
        gs.pending_notices.clear();

        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&ben, &me, 2, "after")), "a blocked key's DM is not ingested");
        let store = gs.dm_store.as_ref().unwrap();
        assert!(store.conversation(&ben).iter().all(|m| m.text != "after"), "and not stored");
        assert!(store.requests().is_empty(), "nor listed as a request");
        assert!(gs.pending_notices.is_empty(), "and nothing is said");
        assert!(gs.chat_dms.iter().all(|d| d.user_key != ben), "their conversation leaves the sidebar");

        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&ben, &me, 3, crate::net::dm_pq::CTL_FOLLOW)));
        assert!(!gs.dm_store.as_ref().unwrap().is_follower(&ben), "a follow notice from them is dropped");
        let pass = crate::relay::core::pq_crypto::build_friend_cert(&ben_seed, SERVER, &ben, &me, &"c1".repeat(16), &crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY).unwrap();
        let mut gift = inner(&ben, &me, 4, crate::net::dm_pq::CTL_FRIEND_CERT);
        gift.cert = Some(pass);
        crate::engine::dm::ingest_dm(&mut gs, &gift);
        assert_eq!(gs.dm_store.as_ref().unwrap().cert_for(&ben), None, "a pass they send is not kept");
        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&me, &ben, 5, "my own words")), "our own self-copy to them is still ours");
        tidy(&gs);
    }

    /// A blocked key's contact request is dropped: never listed, nothing said, its pass not
    /// kept. A refused DM from them is not listed either.
    /// Seen red 2026-10-09 with `screens_dm`'s blocked-key line returning false: "a blocked
    /// key's contact request is never listed" failed (one entry in Requests).
    #[test]
    fn a_blocked_keys_contact_request_is_dropped() {
        let (seed, me) = identity(145);
        let (ann_seed, ann) = identity(146);
        let mut gs = app(&me, &seed, "block-request");
        block(&mut gs, &ann);
        gs.pending_notices.clear();
        let pass = crate::relay::core::pq_crypto::build_friend_cert(&ann_seed, SERVER, &ann, &me, &"d1".repeat(16), &crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY).unwrap();
        let request = inner(&ann, &me, 7, &crate::net::reach::contact_request_text("Ann", &pass));
        assert!(!crate::engine::dm::ingest_dm(&mut gs, &request));
        assert!(gs.dm_store.as_ref().unwrap().requests().is_empty(), "a blocked key's contact request is never listed");
        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&ann, &me, 8, "a stranger's words")));
        assert!(gs.dm_store.as_ref().unwrap().requests().is_empty(), "nor is a DM their settings would have refused");
        assert!(gs.pending_notices.is_empty(), "and nothing is said");
        tidy(&gs);
    }

    /// The pump's rule: a blocked key's channel post, typing, reaction, call ring (so no busy
    /// auto-reject is ever sent back) and direct-connection offer are dropped before the pump
    /// acts on them; anyone else's pass. A post already in the list is hidden by key in a
    /// channel, a group and a DM, and their reactions leave the counts. A trade request from them
    /// is hidden.
    /// Seen red 2026-10-09 with `screens_out` matching only "chat": "a ring from a blocked key
    /// is dropped, so nothing is sent back" failed.
    #[test]
    fn a_blocked_keys_post_ring_and_offer_are_screened() {
        let (seed, me) = identity(147);
        let mut gs = app(&me, &seed, "block-pump");
        block(&mut gs, "cy");
        let frame = |kind: &str, from: &str| serde_json::json!({ "type": kind, "from": from, "action": "ring", "signal_type": "dc_offer" });
        assert!(screens_out(&gs, &frame("chat", "cy")), "a blocked key's channel post is dropped");
        assert!(screens_out(&gs, &frame("voice_call", "cy")), "a ring from a blocked key is dropped, so nothing is sent back");
        assert!(screens_out(&gs, &frame("webrtc_signal", "cy")), "an offer from them is never answered");
        assert!(screens_out(&gs, &frame("typing", "cy")) && screens_out(&gs, &frame("reaction", "cy")));
        assert!(!screens_out(&gs, &frame("voice_call", "dee")), "anyone else's ring goes on to the pump");
        assert!(!screens_out(&gs, &frame("peer_joined", "cy")), "the member list still lists them");
        assert!(!screens_out(&gs, &frame("chat", &me)), "our own posts are never screened");

        let post = |from: &str, channel: &str| crate::gui::ChatMessage { sender_key: from.into(), channel: channel.into(), ..Default::default() };
        for channel in ["general", "p2pgroup:g1", "dm:cy"] {
            assert!(hides_message(&gs, &post("cy", channel)), "a blocked key's post is hidden in {channel}");
            assert!(!hides_message(&gs, &post("dee", channel)));
        }
        assert!(!hides_message(&gs, &post(&me, "dm:cy")), "our own words to them stay");

        let mut reactions: HashMap<String, Vec<String>> = HashMap::new();
        reactions.insert("+1".into(), vec!["cy".into(), "dee".into()]);
        reactions.insert("fire".into(), vec!["cy".into()]);
        let shown = visible_reactions(&gs, &reactions);
        assert_eq!(shown.get("+1"), Some(&vec!["dee".to_string()]), "their reaction leaves the count");
        assert!(!shown.contains_key("fire"), "and an emoji only they used is gone");
        let none_blocked: HashMap<String, Vec<String>> = [("+1".to_string(), vec!["dee".to_string()])].into();
        assert!(matches!(visible_reactions(&gs, &none_blocked), Cow::Borrowed(_)), "nothing copied when nobody is blocked");

        let trade = |from: &str, status: &str| crate::gui::GuiTrade { initiator_key: from.into(), recipient_key: me.clone(), status: status.into(), ..Default::default() };
        assert!(hides_trade(&gs, &trade("cy", "pending")), "their trade request is not listed");
        assert!(!hides_trade(&gs, &trade("cy", "completed")), "a finished trade still settles");
        assert!(!hides_trade(&gs, &trade("dee", "pending")));
        tidy(&gs);
    }

    /// THE NOTES TO OURSELVES (10d): Block deposits `[[hum:block:v1]]<key>` in our own mailbox,
    /// sealed to our own key, and our other device that opens it blocks them too, dated by the
    /// note, and takes back its own passes, without sending a note of its own; Unblock's note
    /// lifts it there; a note addressed to anyone else, or written by anyone else, is ignored, and
    /// no note is ever shown as a message. A note delivered again changes nothing.
    /// Seen red 2026-10-09 with `apply_note`'s "from us AND to us" check taken out: "a note
    /// addressed to someone else is ignored" failed (Dee became blocked).
    #[test]
    fn the_notes_sync_our_other_devices_and_others_are_ignored() {
        let (seed, me) = identity(148);
        let (_e, eve) = identity(149);
        let phone = app(&me, &seed, "block-phone");
        let mut desk = app(&me, &seed, "block-desk");
        desk.dm_store.as_mut().unwrap().record_pass_sent("cy", default_pass(&"e1".repeat(16)));

        let (put, id) = note_put(&phone, &BlockNote::Block("cy".into()), 1_000).expect("a note can be sealed");
        assert_eq!((put["type"].as_str(), put["to"].as_str()), (Some("dm_put"), Some(me.as_str())), "to ourselves only");
        let me_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&seed).unwrap();
        let opened = crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&me_kp, put["content"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!((opened.from.as_str(), opened.to.as_str(), opened.text.as_str(), opened.ts), (me.as_str(), me.as_str(), "[[hum:block:v1]]cy", 1_000));
        assert_eq!(id, opened.dedupe_key(), "the id the sender remembers is the note's own");

        assert!(!crate::engine::dm::ingest_dm(&mut desk, &opened), "a note is never a message");
        assert!(is_blocked(&desk, "cy"), "our other device blocks them too");
        assert_eq!(desk.block_list.as_ref().unwrap().blocked_at("cy"), Some(1_000), "dated by the note's signed time");
        let store = desk.dm_store.as_ref().unwrap();
        assert!(!store.cert_sent_to("cy") && store.pending_withdrawals() == ["e1".repeat(16)], "and takes back the pass it gave");
        assert!(store.conversation(&me).is_empty(), "nothing is stored as a message");

        let lift = inner(&me, &me, 2_000, "[[hum:unblock:v1]]cy");
        crate::engine::dm::ingest_dm(&mut desk, &lift);
        assert!(!is_blocked(&desk, "cy"), "Unblock's note lifts it there");
        crate::engine::dm::ingest_dm(&mut desk, &opened);
        assert!(!is_blocked(&desk, "cy"), "the block note, delivered again, changes nothing");

        crate::engine::dm::ingest_dm(&mut desk, &inner(&me, &eve, 3_000, "[[hum:block:v1]]dee"));
        assert!(!is_blocked(&desk, "dee"), "a note addressed to someone else is ignored");
        crate::engine::dm::ingest_dm(&mut desk, &inner(&eve, &me, 3_000, "[[hum:block:v1]]dee"));
        assert!(!is_blocked(&desk, "dee"), "and so is one written by someone else");
        assert!(!crate::engine::dm::ingest_dm(&mut desk, &inner(&eve, &me, 3_001, "[[hum:unblock:v1]]")), "a malformed note is still never a message");
        assert!(desk.dm_store.as_ref().unwrap().conversation(&eve).is_empty());
        tidy(&phone);
        tidy(&desk);
    }

    /// A block made elsewhere reaches this server's store on its next member list: the sweep
    /// takes back the pass and the follow there too.
    /// Seen red 2026-10-09 with `sweep`'s loop body taken out: "the sweep takes back a pass
    /// given on this server" failed.
    #[test]
    fn the_sweep_takes_back_what_a_block_made_elsewhere_could_not() {
        let (seed, me) = identity(150);
        let mut gs = app(&me, &seed, "block-sweep");
        gs.block_list.as_mut().unwrap().block("fay", 10);
        gs.dm_store.as_mut().unwrap().record_pass_sent("fay", default_pass(&"f1".repeat(16)));
        gs.dm_store.as_mut().unwrap().set_following("fay", true);
        crate::engine::dm::sweep_friend_passes(&mut gs);
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.cert_sent_to("fay"), "the sweep takes back a pass given on this server");
        assert!(!store.is_following("fay"));
        tidy(&gs);
    }

    /// The echo of our own pass or follow for someone we have since blocked, sent by a device
    /// that had not heard of the block yet, does not bring them back: the block wins, the pass is
    /// withdrawn again and the follow is not kept.
    /// Seen red 2026-10-09 with the own-echo branch taken out of `screens_dm`: "a pass our other
    /// device gave them is withdrawn again" failed.
    #[test]
    fn an_echo_of_our_own_pass_for_someone_blocked_does_not_bring_it_back() {
        use crate::net::dm_pq::{CTL_FOLLOW, CTL_FRIEND_CERT};
        let (seed, me) = identity(151);
        let (_g, gus) = identity(152);
        let mut gs = app(&me, &seed, "block-echo");
        block(&mut gs, &gus);
        let pass = crate::relay::core::pq_crypto::build_friend_cert(&seed, SERVER, &me, &gus, &"a7".repeat(16), &crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY).unwrap();
        let mut echo = inner(&me, &gus, 20, CTL_FRIEND_CERT);
        echo.cert = Some(pass);
        assert!(!crate::engine::dm::ingest_dm(&mut gs, &echo));
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.cert_sent_to(&gus), "a pass our other device gave them is withdrawn again");
        assert!(store.pending_withdrawals().contains(&"a7".repeat(16)), "its serial goes to the relay");
        crate::engine::dm::ingest_dm(&mut gs, &inner(&me, &gus, 21, CTL_FOLLOW));
        assert!(!gs.dm_store.as_ref().unwrap().is_following(&gus), "nor does our follow come back");
        assert!(is_blocked(&gs, &gus), "the block wins");
        tidy(&gs);
    }

    /// Block and Unblock with no server connected keep their note for the next connection, one
    /// per person, the newer replacing the older, signed with the change's own time; a member-list
    /// sweep with still nothing connected keeps it waiting.
    /// Seen red 2026-10-09 with `send_or_queue` not queueing: "an offline block keeps its note"
    /// failed (nothing waiting).
    #[test]
    fn notes_made_offline_wait_for_the_next_connection() {
        let (seed, me) = identity(153);
        let mut gs = app(&me, &seed, "block-offline");
        let waiting = |gs: &GuiState| gs.block_list.as_ref().unwrap().pending().to_vec();
        block(&mut gs, "hal");
        let kinds = |w: &[crate::net::block_list::PendingNote]| w.iter().map(|p| (p.key.clone(), p.block)).collect::<Vec<_>>();
        assert_eq!(kinds(&waiting(&gs)), [("hal".to_string(), true)], "an offline block keeps its note");
        assert_eq!(Some(waiting(&gs)[0].at), gs.block_list.as_ref().unwrap().blocked_at("hal"), "signed with the block's date");
        unblock(&mut gs, "hal");
        assert_eq!(kinds(&waiting(&gs)), [("hal".to_string(), false)], "the newer change replaces it");
        crate::engine::dm::sweep_friend_passes(&mut gs);
        assert_eq!(waiting(&gs).len(), 1, "still waiting with nothing connected");
        tidy(&gs);
    }

    /// IN THE GAME (section 4.5, "Game figure and name"): the relay's snapshot entry and join
    /// carry each player's key, and someone we blocked has no name over them and no figure drawn
    /// (both places read `hides_player`); anyone else, and a player whose key has not come yet,
    /// is drawn. Seen red 2026-10-10 two ways: `hides_player` answering false, "no name over the
    /// blocked player"; and the figure pass in lib.rs without its check, "the figure pass asks it".
    /// 10m R9, the HUD's co-presence list too: seen red 2026-10-10 with `copresence_names`
    /// ignoring `hidden`, "the blocked player is not in the HUD's list" (Cy listed); and with
    /// lib.rs handing it `&|_| false`, the count assertion (left 2, right 3).
    #[test]
    fn a_blocked_players_figure_and_name_are_not_drawn() {
        use crate::net::protocol::NetMessage;
        let (seed, me) = identity(148);
        let mut gs = app(&me, &seed, "block-game");
        block(&mut gs, "cy");

        let entry = serde_json::json!({ "entity_id": 7, "entity_type": "player", "position": [1.0, 1.7, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "components": { "name": "Cy", "key": "cy" } });
        let keys: Vec<Option<String>> = crate::engine::net_route::snapshot_entry_messages(&entry, None)
            .into_iter()
            .filter_map(|m| match m { NetMessage::PlayerJoined { key, .. } => Some(key), _ => None })
            .collect();
        assert_eq!(keys, vec![Some("cy".to_string())], "a snapshot entry's key reaches the player record");

        let mut world = hecs::World::new();
        for (id, name, key) in [(7u32, "Cy", Some("cy")), (8, "Dee", Some("dee")), (9, "Player 9", None)] {
            let at = glam::Vec3::new(id as f32, 1.7, 0.0);
            world.spawn((
                crate::ecs::components::Transform { position: at, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
                crate::net::sync::RemotePlayer {
                    player_id: id,
                    name: name.into(),
                    look: None,
                    key: key.map(str::to_string),
                    last_position: at,
                    target_position: at,
                    last_rotation: glam::Quat::IDENTITY,
                    target_rotation: glam::Quat::IDENTITY,
                    velocity: glam::Vec3::ZERO,
                    interpolation_t: 1.0,
                    last_update_time: 0.0,
                },
            ));
        }
        let labels = crate::engine::net_route::nameplate_labels(&world, glam::Vec3::ZERO, &|p| hides_player(&gs, p));
        let mut names: Vec<String> = labels.into_iter().map(|l| l.name).collect();
        names.sort();
        assert!(!names.contains(&"Cy".to_string()), "no name over the blocked player: {names:?}");
        assert_eq!(names, vec!["Dee".to_string(), "Player 9".to_string()], "everyone else is named, a player with no key yet too");

        // 10m R9: nor in the HUD's "N here: ..." list, which counts every other player.
        let here = crate::engine::net_route::copresence_names(&world, &|p| hides_player(&gs, p));
        assert_eq!(here, vec!["Dee".to_string(), "Player 9".to_string()], "the blocked player is not in the HUD's list");

        let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();
        assert_eq!(
            lib.matches("crate::engine::block::hides_player(&state.gui_state").count(),
            3,
            "the figure pass asks it, and so do the nameplates and the HUD's co-presence list"
        );
        tidy(&gs);
    }
}
