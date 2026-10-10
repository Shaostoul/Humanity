//! "Who can reach me" (step B of docs/design/blocking-and-safe-mode.md, section 10c), the
//! native client's half that touches the socket and the app state: the relay's `reach_settings`
//! (the only source of what Settings > Safety shows) and `reach_refused`, the `reach_set` the
//! page sends, contact requests sent and received, the Requests list's Accept and Ignore, and
//! the "People I choose" ticks (Message, Call and Trade per friend, 10c-ii), which re-issue a
//! friend's pass (engine/dm.rs `reissue_pass`). The rules themselves, without a socket, are
//! src/net/reach.rs.
//!
//! A contact request (10c, amended in review) is an ordinary signed sealed DM whose text is
//! `[[hum:contact-request:v1]]{"name":...,"pass":...}`, sent with `"contact_request": true` so the
//! relay lets it past the recipient's gate. The pass is the requester's for the recipient: Accept
//! stores it and attaches it to the reply, which is how the reply gets past the REQUESTER's own
//! gate, so two people on the safe defaults can become friends.
//!
//! The message pump (frame_ws_poll.rs) reaches this file through one line, `on_frame`; it sits
//! at its file-size budget, so nothing more goes there.

use crate::gui::GuiState;
use crate::net::dm_pq::DmInner;
use crate::net::dm_store::{DmStore, SentPass};
use crate::net::reach::{Audience, ContactRequest, ReachKind, ReachSettings};

/// How long Settings > Safety waits for the server to answer a change before saying it has not.
pub(crate) const REACH_ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// What a sender sees in their conversation when the person's gate refused a message (10c,
/// word for word). The same for everyone refused, so it never singles anyone out (4.7).
pub(crate) const REFUSED_SENTENCE: &str =
    "This person only accepts messages from people they know. You can send a contact request: they will see only your name.";

/// The relay's step B frames, from the message pump: `reach_settings` and `reach_refused`.
pub(crate) fn on_frame(gs: &mut GuiState, frame: &serde_json::Value) {
    match frame.get("type").and_then(|t| t.as_str()) {
        Some("reach_settings") => on_reach_settings(gs, frame),
        Some("reach_refused") => on_reach_refused(gs, frame),
        _ => {}
    }
}

/// `reach_settings`: what the server holds for us, after every sign-in and every `reach_set`.
/// Kept in the DM store (per server, encrypted), which is where the Safety page and the "show
/// as a request" rule read it.
fn on_reach_settings(gs: &mut GuiState, frame: &serde_json::Value) {
    let Some(settings) = ReachSettings::from_frame(frame) else {
        log::warn!("reach_settings not read (a kind missing or a word this app does not know): {frame}");
        return;
    };
    gs.reach.asked = None;
    gs.reach.status.clear();
    if crate::engine::dm::ensure_dm_store(gs) {
        if let Some(store) = gs.dm_store.as_mut() {
            store.set_reach_settings(settings);
            store.save();
        }
    }
}

/// `reach_refused`: the person's gate turned away something we sent. A message: their
/// conversation (and their profile card) shows the sentence and a Send request button. A trade
/// request: the Trade page's status line, and a notice. A call is never answered at all (10c),
/// so nothing arrives for one.
fn on_reach_refused(gs: &mut GuiState, frame: &serde_json::Value) {
    let Some(to) = frame.get("to").and_then(|v| v.as_str()).filter(|t| !t.is_empty()) else { return };
    match frame.get("kind").and_then(|v| v.as_str()).and_then(ReachKind::from_wire) {
        // Not a notice of its own: our own control messages (a follow, a pass) are refused the
        // same way, and a notice for each would read as an error the person never made. The
        // conversation shows it where a message was written.
        Some(ReachKind::Message) => {
            // A report to a group's creator that their server did not let through (10j) is said
            // as such, not turned into an offer of a contact request. A refused contact request
            // (only "Nobody" refuses one) says they are not taking requests, and the notice stops
            // offering another; a refused message offers one.
            if crate::engine::group_report::refused(gs, to) {
            } else if frame.get("request").and_then(|v| v.as_bool()) == Some(true) {
                gs.reach.refused.insert(to.to_string(), crate::net::reach::Refusal::NotTakingRequests);
            } else {
                gs.reach.refused.entry(to.to_string()).or_insert(crate::net::reach::Refusal::Refused);
            }
        }
        Some(ReachKind::Trade) => {
            let line = format!(
                "{} only accepts trade requests from people they know. Nothing was sent.",
                crate::engine::dm::dm_display_name(gs, to)
            );
            gs.pending_notices.push(line.clone());
            gs.trade_status = line;
        }
        _ => {}
    }
}

/// Ask the server to set one kind's audience (`reach_set`). The page keeps showing what the
/// server last said until its `reach_settings` answer arrives.
pub(crate) fn ask(gs: &mut GuiState, kind: ReachKind, audience: Audience) {
    // Step G: while the protected setup is on, changing a row needs the PIN.
    if !crate::engine::protected::allows(gs, crate::net::protected::ProtectedAction::ReachRow(kind, audience)) {
        return;
    }
    let Some(client) = gs.ws_client.as_ref().filter(|c| c.is_connected()) else {
        gs.reach.status = "Not connected, so nothing was changed. Connect to the server to change who can reach you there.".to_string();
        return;
    };
    client.send(&ReachSettings::reach_set_frame(&[(kind, audience)]).to_string());
    let mut want = current(gs).unwrap_or_default();
    want.set(kind, audience);
    gs.reach.asked = Some((want, std::time::Instant::now()));
    gs.reach.status.clear();
}

/// What the server last said, if it has said (nothing while not signed in to one).
pub(crate) fn current(gs: &GuiState) -> Option<ReachSettings> {
    gs.dm_store.as_ref().and_then(|s| s.reach_settings())
}

/// Do we share a P2P group with `key`? The groups the relay lists for us and the open group's
/// roster (the same facts the direct-connection gate reads, frame_ws_poll_offers.rs). Before the
/// group list has loaded the answer is not known here, and then the relay's own check stands
/// (true). Only for tidying the Requests list (`settle_requests`): the checks on what ARRIVES (a
/// DM's text, a call's ring) take the server's word instead (`LET_THROUGH_SHARES_GROUP`).
pub(crate) fn shares_group(gs: &GuiState, key: &str) -> bool {
    gs.p2p_groups_last_fetch.is_none()
        || gs.p2p_groups.iter().any(|g| g.members.iter().any(|m| m == key))
        || gs.p2p_group_fp_to_key.values().any(|k| k == key)
}

// ── Arrival ─────────────────────────────────────────────────────────────────

/// Group membership in this app's own checks on what arrives is the SERVER's call (10m R8): under
/// "Friends and people in my groups", someone the server let a DM or a ring through from counts as
/// sharing a group with us, because the relay checked group membership against its own records
/// before passing it on. This app's own list of groups goes stale (the desktop refreshes it only
/// while Chat is open), and an allowed ring from someone who had just joined a group was dropped.
/// It only matters under that audience; every other one ignores it.
pub(crate) const LET_THROUGH_SHARES_GROUP: bool = true;

/// THE "show as a request" rule on arrival (10c): a verified DM from someone our own message
/// setting would refuse goes to the Requests list, name only, and its text is dropped here,
/// never stored. Returns true when it was screened out (the caller renders nothing).
pub(crate) fn file_if_refused(gs: &mut GuiState, inner: &DmInner) -> bool {
    let Some(store) = gs.dm_store.as_mut() else { return false };
    let before = store.requests().len();
    let screened = file_if_refused_in(store, inner);
    if screened {
        let listed = store.requests().len() > before;
        store.save();
        if listed {
            let name = crate::engine::dm::dm_display_name(gs, &inner.from);
            gs.pending_notices.push(format!("{name} wants to be in contact. Accept or ignore it under Requests, in Chat or Settings > Safety."));
        }
    }
    screened
}

/// The same rule on any server's store (the active one above, or a parked server's,
/// engine/bg_connections.rs); the caller saves. Control messages and contact requests are never
/// screened here: they carry no words of the sender's, and the friendship handshake rides them.
/// Under "Nobody" nothing is listed either (the server turns contact requests away there too);
/// the text is still dropped. Whether the sender shares a group with us is the server's call
/// (`LET_THROUGH_SHARES_GROUP`, 10m R8).
pub(crate) fn file_if_refused_in(store: &mut DmStore, inner: &DmInner) -> bool {
    use crate::net::dm_pq::{CTL_FOLLOW, CTL_FRIEND_CERT, CTL_UNFOLLOW};
    let control = [CTL_FOLLOW, CTL_UNFOLLOW, CTL_FRIEND_CERT].contains(&inner.text.as_str())
        || inner.text.starts_with(crate::net::reach::CONTACT_REQUEST_MARKER);
    if control || store.admits_dm_from(&inner.from, LET_THROUGH_SHARES_GROUP) {
        return false;
    }
    if store.reach_settings().unwrap_or_default().message != Audience::Nobody {
        store.add_request(ContactRequest { key: inner.from.clone(), ts: inner.ts, pass: String::new() });
    }
    true
}

/// What became of a contact request on arrival.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Arrival {
    /// Not a request, a pass that does not check out, or "Nobody": nothing kept.
    Dropped,
    /// Listed under Requests (`true` when it is a new entry).
    Listed(bool),
    /// We already follow them (we asked them too): their request is their answer, and the
    /// friendship completes without a list entry.
    Completes,
    /// Our own request, echoed from another of our devices: its pass and our follow are recorded.
    OwnEcho,
}

/// A contact request (a verified DM whose text carries the marker) on any server's store; the
/// caller saves. The pass must be the SIGNED sender's, for us, on this store's server; one that
/// is not is dropped with the request. `me` is our key. `may_complete` says whether a request
/// from someone we already follow may finish the friendship by itself: false while the protected
/// setup is on and they are not on its approved list (`ProtectedSetup::pass_allowed`), because
/// finishing it then would store their pass and count them a friend while our own pass to them
/// is refused, leaving them stuck as half a friend; such a request is listed instead, where
/// Accept asks for the PIN (10h: "Contact requests show the name with Accept (needs the PIN)").
pub(crate) fn contact_request_in(store: &mut DmStore, me: &str, inner: &DmInner, may_complete: bool) -> Arrival {
    let Some((_claimed_name, pass)) = crate::net::reach::parse_contact_request_text(&inner.text) else { return Arrival::Dropped };
    if inner.from == me {
        // Ours, from another device: the pass it gave is ours to withdraw later, and asking is
        // following.
        let Ok((given, _)) = crate::relay::core::pq_crypto::parse_friend_cert(&pass) else { return Arrival::Dropped };
        store.record_pass_sent(&inner.to, SentPass { serial: given.serial, may: given.may.wire() });
        store.set_following(&inner.to, true);
        // An echo of a pass to them: the device that knows has spoken (10m R3).
        store.clear_changed_elsewhere(&inner.to);
        return Arrival::OwnEcho;
    }
    let server = store.pass_server().unwrap_or("").to_string();
    if let Err(e) = crate::relay::core::pq_crypto::verify_friend_cert(&server, &inner.from, me, &pass) {
        log::warn!("contact request from {}… dropped: its pass failed its check ({e:?})", &inner.from[..12.min(inner.from.len())]);
        return Arrival::Dropped;
    }
    if store.is_following(&inner.from) && may_complete {
        store.store_cert_from(&inner.from, &pass);
        store.set_follower(&inner.from, true);
        store.remove_request(&inner.from);
        return Arrival::Completes;
    }
    if store.reach_settings().unwrap_or_default().message == Audience::Nobody {
        return Arrival::Dropped;
    }
    Arrival::Listed(store.add_request(ContactRequest { key: inner.from.clone(), ts: inner.ts, pass }))
}

/// A contact request on the active server: listed (with a notice when new), or completing a
/// friendship we had asked for ourselves (our pass goes to them if they have none from us yet,
/// carrying theirs past their gate).
pub(crate) fn ingest_contact_request(gs: &mut GuiState, inner: &DmInner) {
    let me = gs.profile_public_key.clone();
    let may_complete = gs.protected.setup.pass_allowed(&inner.from);
    let Some(store) = gs.dm_store.as_mut() else { return };
    let arrival = contact_request_in(store, &me, inner, may_complete);
    store.save();
    match arrival {
        Arrival::Listed(true) => {
            let name = crate::engine::dm::dm_display_name(gs, &inner.from);
            gs.pending_notices.push(format!("{name} sent you a contact request. Accept or ignore it under Requests, in Chat or Settings > Safety."));
        }
        Arrival::Completes => {
            // Their pass reached us with it (10m R7): ours rides it past their gate now.
            gs.reach.pass_refused.remove(&inner.from);
            crate::engine::dm::send_friend_cert(gs, &inner.from);
            crate::engine::dm::refresh_social_mirrors(gs);
        }
        Arrival::OwnEcho => crate::engine::dm::refresh_social_mirrors(gs),
        Arrival::Dropped | Arrival::Listed(false) => {}
    }
}

/// On a new member list: drop requests from people who have since become friends (our settings
/// now let them message us and we follow them).
pub(crate) fn settle_requests(gs: &mut GuiState) {
    let keyed: Vec<(String, bool)> = gs
        .dm_store
        .as_ref()
        .map(|s| s.requests().iter().map(|r| (r.key.clone(), shares_group(gs, &r.key))).collect())
        .unwrap_or_default();
    let Some(store) = gs.dm_store.as_mut() else { return };
    let settled: Vec<String> =
        keyed.into_iter().filter(|(k, shares)| store.is_friend(k) && store.admits_dm_from(k, *shares)).map(|(k, _)| k).collect();
    for key in &settled {
        store.remove_request(key);
    }
    if !settled.is_empty() {
        store.save();
    }
}

/// The name a request shows: the one the server's member list holds for its (signed) key, with
/// `true`; never the name a request claimed. Someone not on the list right now shows the start
/// of their key, with `false`.
pub(crate) fn request_name(gs: &GuiState, req: &ContactRequest) -> (String, bool) {
    member_name(gs, &req.key)
}

/// The name the server's member list holds for `key`, with `true`; otherwise the start of the
/// key, with `false`. Also how Settings > Safety > Blocked people names each person (step C).
pub(crate) fn member_name(gs: &GuiState, key: &str) -> (String, bool) {
    match gs.chat_users.iter().find(|u| u.public_key == key) {
        Some(u) if !u.name.is_empty() && u.name != "Anonymous" => (u.name.clone(), true),
        _ => (key.chars().take(8).collect(), false),
    }
}

// ── Sending, Accept, Ignore ─────────────────────────────────────────────────

/// The two `dm_put` frames of a contact request to `peer` (theirs, flagged `contact_request`, and
/// our self-copy, so our other devices learn the pass we gave), and the pass to record once the
/// server took theirs (10l). The pass is ours for them, with the default `may` (step A): asking
/// someone to connect is consenting to hear back from them.
pub(crate) fn contact_request_puts(gs: &GuiState, peer: &str) -> Result<(serde_json::Value, serde_json::Value, SentPass), String> {
    if crate::engine::block::is_blocked(gs, peer) {
        return Err("You blocked them. Unblock them first, in Settings > Safety > Blocked people.".into());
    }
    if !gs.peer_kyber_keys.contains_key(peer) {
        return Err("Their key is not known yet. They need to have been online on this server once.".into());
    }
    // Step F's recovery-phrase guard: a request carries our name, so a name holding the phrase stops it.
    if crate::engine::warnings::holds_own_phrase(gs, &[&gs.user_name]) {
        return Err(crate::net::warnings::GUARD_LINE.into());
    }
    let (pass, sent) = crate::engine::dm::mint_pass(gs, peer, &crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY)
        .ok_or("The request could not be made yet: this server's identity is not known. Try again in a moment.")?;
    let text = crate::net::reach::contact_request_text(&gs.user_name, &pass);
    let (mut theirs, ours) = crate::engine::dm::control_puts(gs, peer, &text, None).ok_or("The request could not be sealed.")?;
    theirs["contact_request"] = serde_json::Value::Bool(true);
    Ok((theirs, ours, sent))
}

/// What Send request says while a request or pass to that person still waits for the server's
/// answer (10m R6, word for word as the web chat says it): one on its way per friend at a time.
pub(crate) const STILL_WAITING: &str = "A request or pass to them is still waiting for this server to answer. Try again in a moment.";

/// Send `peer` a contact request (the Send request button). The pass in it counts as given, and
/// asking counts as following them, only once the server took the request (10l; 10m R1, as on the
/// web: engine/put_answer.rs), and from then Unfollow withdraws it like any other; one not taken
/// records nothing, follows no one, and the notice offers Send request again. Refused while a
/// request or pass to them still waits for its answer (R6).
pub(crate) fn send_contact_request(gs: &mut GuiState, peer: &str) -> Result<(), String> {
    // Step G: asking is following and gives them a pass, so with the protected setup on it
    // needs the PIN; nothing is sent until it has been entered (the prompt is open now).
    if !crate::engine::protected::allows(gs, crate::net::protected::ProtectedAction::SendRequest(peer.to_string())) {
        return Ok(());
    }
    if !crate::engine::dm::ensure_dm_store(gs) {
        return Err("Unlock your identity and connect first.".into());
    }
    if gs.pending_puts.has_peer(peer) {
        return Err(STILL_WAITING.into());
    }
    let (theirs, ours, sent) = contact_request_puts(gs, peer)?;
    if !crate::engine::dm::send_held(gs, peer, theirs, ours, sent, crate::net::put_answers::Held::Request, false) {
        return Err("Connect to the server first.".into());
    }
    gs.reach.refused.insert(peer.to_string(), crate::net::reach::Refusal::RequestSent);
    gs.reach.status.clear();
    Ok(())
}

/// Accept `key`'s request: follow them back, which makes the two of you friends. The request
/// stands for their follow, and the pass it brought is now held, so our pass and our follow go
/// to them carrying it (`friend_cert`, engine/dm.rs `control_puts`) past their own gate.
pub(crate) fn accept_request(gs: &mut GuiState, key: &str) {
    // Step G: accepting makes a friend, so with the protected setup on it needs the PIN.
    if !crate::engine::protected::allows(gs, crate::net::protected::ProtectedAction::AcceptRequest(key.to_string())) {
        return;
    }
    if !crate::engine::dm::ensure_dm_store(gs) {
        return;
    }
    if gs.dm_store.as_ref().is_some_and(|s| s.requests().iter().any(|r| r.key == key)) {
        crate::engine::dm::person_chose(gs, key); // the person's own choice, made here (10m)
    }
    let Some(store) = gs.dm_store.as_mut() else { return };
    let Some(req) = store.remove_request(key) else { return };
    if !req.pass.is_empty() {
        store.store_cert_from(key, &req.pass);
    }
    store.set_follower(key, true);
    store.set_following(key, true);
    store.save();
    crate::engine::dm::send_friend_cert(gs, key);
    let _ = crate::engine::dm::send_dm_control(gs, key, crate::net::dm_pq::CTL_FOLLOW, None);
    crate::engine::dm::refresh_social_mirrors(gs);
    gs.reach.status.clear();
}

/// Ignore `key`'s request: it leaves the list with the pass it brought, and no one is told.
pub(crate) fn ignore_request(gs: &mut GuiState, key: &str) {
    if let Some(store) = gs.dm_store.as_mut() {
        store.remove_request(key);
        store.save();
    }
}

/// Tick or untick one of a friend's three ticks in "People I choose" (10c-ii): the choice is
/// kept, and their pass is re-issued to allow exactly what is ticked now (the new one minted
/// first, then the old one withdrawn, so the relay honours the change at once).
pub(crate) fn set_tick(gs: &mut GuiState, peer: &str, kind: ReachKind, on: bool) {
    // Step G: while the protected setup is on, changing a tick needs the PIN.
    if !crate::engine::protected::allows(gs, crate::net::protected::ProtectedAction::Tick(peer.to_string(), kind, on)) {
        return;
    }
    // The person's own choice for them, made here (10m R3, R7): the pass goes out from it.
    crate::engine::dm::person_chose(gs, peer);
    if let Some(store) = gs.dm_store.as_mut() {
        let mut ticks = store.ticks(peer);
        ticks.set(kind, on);
        store.set_ticks(peer, ticks);
        store.save();
    }
    crate::engine::dm::reissue_pass(gs, peer);
}

/// The Requests list, contact requests and the refusal notice without a socket.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::pq_crypto::{derive_dilithium_seed, verify_friend_cert, DilithiumKeypair};

    const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

    fn identity(n: u8) -> (Vec<u8>, String) {
        let seed = vec![n; 32];
        (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
    }

    fn inner(from: &str, to: &str, text: &str) -> DmInner {
        DmInner { from: from.into(), to: to.into(), ts: 7, text: text.into(), sig_b64: format!("{from}{to}{text}"), cert: None }
    }

    /// A signed-in app for `seed` / `me` on SERVER, with its own DM store (a unique file name).
    fn app(me: &str, seed: &[u8], tag: &str) -> GuiState {
        let mut gs = GuiState::default();
        gs.profile_public_key = me.to_string();
        gs.private_key_bytes = Some(seed.to_vec());
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let mut store = DmStore::load(seed, me, &format!("wss://{tag}-{nanos}.example"));
        store.set_pass_server(SERVER);
        gs.dm_store = Some(store);
        gs
    }

    fn kyber_of(seed: &[u8]) -> String {
        crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64()
    }

    /// Open a `dm_put` frame's envelope with the recipient's seed and verify its signature.
    fn open(put: &serde_json::Value, seed: &[u8]) -> DmInner {
        let me = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap();
        crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&me, put["content"].as_str().unwrap()).unwrap()).unwrap()
    }

    /// THE ROUND TRIP (10c, amended): Ann's request to Ben is a signed sealed DM flagged
    /// `contact_request`, carrying her pass for him; Ben's app verifies it and lists Ann by the
    /// name the member list holds for her key (not the one she claimed), never as a message;
    /// Accept makes them friends, and Ben's reply carries Ann's pass as `friend_cert`, the pass
    /// Ann's relay checks. A request whose pass is someone else's, or not for Ben, is dropped;
    /// our own request echoed from another device records the pass we gave.
    /// Seen red 2026-10-09 with `accept_request` not storing the request's pass: "the reply
    /// carries Ann's pass", `friend_cert` None.
    #[test]
    fn a_contact_request_round_trip() {
        let (ann_seed, ann) = identity(101);
        let (ben_seed, ben) = identity(102);
        let (cy_seed, cy) = identity(103);
        let mut ann_app = app(&ann, &ann_seed, "reach-ann");
        ann_app.user_name = "Ann".into();
        ann_app.peer_kyber_keys.insert(ben.clone(), kyber_of(&ben_seed));
        let mut ben_app = app(&ben, &ben_seed, "reach-ben");
        ben_app.peer_kyber_keys.insert(ann.clone(), kyber_of(&ann_seed));
        ben_app.chat_users.push(crate::gui::ChatUser { name: "Ann_K".into(), public_key: ann.clone(), role: String::new(), status: "online".into() });

        let (theirs, ours, sent) = contact_request_puts(&ann_app, &ben).unwrap();
        assert_eq!((theirs["to"].as_str(), theirs["contact_request"].as_bool()), (Some(ben.as_str()), Some(true)), "flagged, to Ben");
        assert!(theirs.get("friend_cert").is_none(), "Ann holds no pass of Ben's yet");
        let arrived = open(&theirs, &ben_seed);
        assert_eq!(arrived.from, ann, "the signature says who asked");
        let (claimed, pass) = crate::net::reach::parse_contact_request_text(&arrived.text).unwrap();
        assert_eq!(claimed, "Ann");
        assert_eq!(verify_friend_cert(SERVER, &ann, &ben, &pass).map(|p| p.serial), Ok(sent.serial.clone()), "her pass, for Ben, on this server");
        assert_eq!(open(&ours, &ann_seed).text, arrived.text, "her self-copy holds the same request");

        assert!(!crate::engine::dm::ingest_dm(&mut ben_app, &arrived), "a request is never a message");
        let store = ben_app.dm_store.as_ref().unwrap();
        assert!(store.conversation(&ann).is_empty());
        assert_eq!(store.requests().len(), 1, "it is listed");
        assert_eq!(request_name(&ben_app, &store.requests()[0]), ("Ann_K".to_string(), true), "by the member list's name for her key");
        assert_eq!(store.cert_for(&ann), None, "her pass is not held until Accept");

        accept_request(&mut ben_app, &ann);
        let store = ben_app.dm_store.as_ref().unwrap();
        assert!(store.requests().is_empty() && store.is_friend(&ann), "Accept makes them friends");
        let (reply, _) = crate::engine::dm::control_puts(&ben_app, &ann, crate::net::dm_pq::CTL_FOLLOW, None).unwrap();
        assert_eq!(reply["friend_cert"].as_str(), Some(pass.as_str()), "the reply carries Ann's pass");

        // Ann's own request echoed to her other device: the pass she gave is recorded.
        let mut ann_other = app(&ann, &ann_seed, "reach-ann2");
        assert_eq!(contact_request_in(ann_other.dm_store.as_mut().unwrap(), &ann, &open(&ours, &ann_seed), true), Arrival::OwnEcho);
        let store = ann_other.dm_store.as_ref().unwrap();
        assert_eq!(store.passes_sent_to(&ben).iter().map(|p| p.serial.clone()).collect::<Vec<_>>(), vec![sent.serial.clone()]);
        assert!(store.is_following(&ben), "asking is following");

        // A pass that is not the signed sender's, or not for Ben, is dropped with the request.
        let cy_pass = crate::relay::core::pq_crypto::build_friend_cert(&cy_seed, SERVER, &cy, &ben, &"cc".repeat(16), &crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY).unwrap();
        let forged = inner(&ann, &ben, &crate::net::reach::contact_request_text("Ann", &cy_pass));
        let mut ben_fresh = app(&ben, &ben_seed, "reach-ben2");
        assert_eq!(contact_request_in(ben_fresh.dm_store.as_mut().unwrap(), &ben, &forged, true), Arrival::Dropped, "someone else's pass");
        let for_cy = crate::relay::core::pq_crypto::build_friend_cert(&ann_seed, SERVER, &ann, &cy, &"dd".repeat(16), &crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY).unwrap();
        let misaddressed = inner(&ann, &ben, &crate::net::reach::contact_request_text("Ann", &for_cy));
        assert_eq!(contact_request_in(ben_fresh.dm_store.as_mut().unwrap(), &ben, &misaddressed, true), Arrival::Dropped, "a pass for someone else");
        assert!(ben_fresh.dm_store.as_ref().unwrap().requests().is_empty());

        for a in [&ann_app, &ben_app, &ann_other, &ben_fresh] {
            a.dm_store.as_ref().unwrap().remove_file_for_test();
        }
    }

    /// The "show as a request" rule on arrival, end to end through `ingest_dm`: under the safe
    /// defaults a stranger's DM is filed under Requests and its text is never stored; a friend's
    /// (one holding our pass) is an ordinary DM; a control message is never screened; our own
    /// self-copy is never a request; once the server says "anyone", a stranger's DM is ordinary
    /// too; under "nobody" a stranger's DM is dropped and nothing is listed. A `reach_refused` for
    /// a message marks the conversation without a notice; one for a call says nothing.
    /// Seen red 2026-10-09 with `file_if_refused`'s call taken out of `ingest_dm`: "a stranger's
    /// DM is not ingested" failed (ingest_dm returned true and stored the text).
    #[test]
    fn strangers_dms_become_requests() {
        let (my_seed, me) = identity(91);
        let (_s, stranger) = identity(92);
        let (_f, friend) = identity(93);
        let mut gs = app(&me, &my_seed, "reach-dms");
        gs.dm_store.as_mut().unwrap().record_pass_sent(&friend, SentPass { serial: "ab".repeat(16), may: "invite,message,trade,voice_message".into() });

        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&stranger, &me, "buy my thing")), "a stranger's DM is not ingested");
        let store = gs.dm_store.as_ref().unwrap();
        assert_eq!(store.requests().len(), 1, "it is filed as a request");
        assert_eq!((store.requests()[0].key.as_str(), store.requests()[0].pass.as_str()), (stranger.as_str(), ""));
        assert!(store.conversation(&stranger).is_empty(), "and its text is not stored");
        assert_eq!(gs.pending_notices.len(), 1, "with one notice");

        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&friend, &me, "hello")), "a friend's DM is a DM");
        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&me, &stranger, "my own self-copy")), "our own self-copy is never a request");
        assert!(!file_if_refused(&mut gs, &inner(&stranger, &me, crate::net::dm_pq::CTL_FOLLOW)), "a control message is never screened");
        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&stranger, &me, "again")));
        assert_eq!(gs.dm_store.as_ref().unwrap().requests().len(), 1, "a repeat makes no second row");

        on_frame(&mut gs, &serde_json::json!({ "type": "reach_settings", "settings": { "message": "anyone", "call": "chosen", "trade": "friends" } }));
        assert_eq!(current(&gs).map(|s| s.message), Some(Audience::Anyone), "the server's word is kept");
        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&stranger, &me, "second try")), "under Anyone a stranger's DM is a DM");

        on_frame(&mut gs, &serde_json::json!({ "type": "reach_settings", "settings": { "message": "nobody", "call": "chosen", "trade": "friends" } }));
        let (_o, other) = identity(94);
        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&other, &me, "under nobody")), "under Nobody a stranger's DM is dropped");
        assert!(gs.dm_store.as_ref().unwrap().requests().iter().all(|r| r.key != other), "and nothing is listed");

        on_frame(&mut gs, &serde_json::json!({ "type": "reach_refused", "kind": "call", "to": friend }));
        assert!(gs.reach.refused.is_empty(), "a refused call says nothing");
        let notices = gs.pending_notices.len();
        on_frame(&mut gs, &serde_json::json!({ "type": "reach_refused", "kind": "message", "to": stranger }));
        assert_eq!(gs.reach.refused.get(&stranger), Some(&crate::net::reach::Refusal::Refused), "a refused message marks the conversation");
        // A refused contact request (2026-10-10): the notice stops offering another. Seen red with
        // the `request` flag ignored: "a refused request says they are not taking requests".
        on_frame(&mut gs, &serde_json::json!({ "type": "reach_refused", "kind": "message", "to": stranger, "request": true }));
        assert_eq!(gs.reach.refused.get(&stranger), Some(&crate::net::reach::Refusal::NotTakingRequests), "a refused request says they are not taking requests");
        on_frame(&mut gs, &serde_json::json!({ "type": "reach_refused", "kind": "message", "to": stranger }));
        assert_eq!(gs.reach.refused.get(&stranger), Some(&crate::net::reach::Refusal::NotTakingRequests), "and a later refused message does not offer one again");
        assert_eq!(gs.pending_notices.len(), notices, "with no notice of its own");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// A pass re-issued from another device: its echo sets the ticks to what that device chose
    /// and withdraws our record of passes saying otherwise, so this device does not mint again;
    /// and an untick of Call while the new pass cannot go out withdraws the calling pass at once.
    /// Seen red 2026-10-09 with the echo arm not withdrawing the old pass: "the old pass is
    /// withdrawn here too", the default pass still standing beside the new one.
    #[test]
    fn an_echoed_reissue_moves_the_tick_and_the_record() {
        use crate::net::reach::FriendTicks;
        use crate::relay::core::pq_crypto::build_friend_cert;
        let (my_seed, me) = identity(95);
        let (_b, ben) = identity(96);
        let mut gs = app(&me, &my_seed, "reach-echo");
        let old = "11".repeat(16);
        gs.dm_store.as_mut().unwrap().record_pass_sent(&ben, SentPass { serial: old.clone(), may: "invite,message,trade,voice_message".into() });
        assert!(gs.dm_store.as_ref().unwrap().passes_out_of_step().is_empty(), "a default pass with no choice is in step");

        let new = "22".repeat(16);
        let all = FriendTicks { message: true, call: true, trade: true };
        let cert = build_friend_cert(&my_seed, SERVER, &me, &ben, &new, &crate::net::reach::intended_may(all)).unwrap();
        let mut echo = inner(&me, &ben, crate::net::dm_pq::CTL_FRIEND_CERT);
        echo.cert = Some(cert);
        crate::engine::dm::ingest_dm(&mut gs, &echo);
        let store = gs.dm_store.as_ref().unwrap();
        assert_eq!(store.ticks(&ben), all, "the ticks follow the other device");
        assert_eq!(store.passes_sent_to(&ben).iter().map(|p| p.serial.clone()).collect::<Vec<_>>(), vec![new.clone()], "the old pass is withdrawn here too");
        assert_eq!(store.pending_withdrawals(), [old], "and waits for the relay to confirm");
        assert!(store.passes_out_of_step().is_empty(), "nothing left to re-issue");

        // Untick here with no DM key for Ben: the call pass cannot be replaced yet, so it goes now.
        set_tick(&mut gs, &ben, ReachKind::Call, false);
        let store = gs.dm_store.as_ref().unwrap();
        assert_eq!(store.ticks(&ben), FriendTicks::default(), "Message and Trade still ticked");
        assert!(store.passes_sent_to(&ben).is_empty(), "taking calling away does not wait for a new pass");
        assert!(store.pending_withdrawals().contains(&new));
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// 10c-ii, changing a tick while the new pass cannot go out yet (no DM key for the friend
    /// here, so the sweep delivers it later): unticking Message takes the old pass back at once,
    /// because it allowed messages; ticking Trade for a friend with nothing ticked keeps their
    /// only pass until the new one goes out, because it took nothing away.
    /// Seen red 2026-10-10 with `reissue_pass` still asking only whether CALLING was taken
    /// away: "unticking Message takes the old pass back at once" failed (the pass stood).
    #[test]
    fn changing_a_tick_takes_back_at_once_or_waits() {
        use crate::net::reach::{intended_may_wire, FriendTicks};
        let (my_seed, me) = identity(97);
        let (_b, ben) = identity(98);
        let (_c, cy) = identity(99);
        let mut gs = app(&me, &my_seed, "reach-ticks");
        let none = FriendTicks { message: false, call: false, trade: false };
        {
            let store = gs.dm_store.as_mut().unwrap();
            store.record_pass_sent(&ben, SentPass { serial: "33".repeat(16), may: intended_may_wire(FriendTicks::default()) });
            store.set_ticks(&cy, none);
            store.record_pass_sent(&cy, SentPass { serial: "44".repeat(16), may: intended_may_wire(none) });
        }

        set_tick(&mut gs, &ben, ReachKind::Message, false);
        let store = gs.dm_store.as_ref().unwrap();
        assert_eq!(store.ticks(&ben), FriendTicks { message: false, call: false, trade: true }, "the choice is kept");
        assert!(store.passes_sent_to(&ben).is_empty(), "unticking Message takes the old pass back at once");
        assert_eq!(store.pending_withdrawals(), ["33".repeat(16)], "and the withdrawal waits for the relay");
        assert_eq!(store.intended_may_wire(&ben), "trade", "the pass the sweep gives them next allows trades only");

        set_tick(&mut gs, &cy, ReachKind::Trade, true);
        let store = gs.dm_store.as_ref().unwrap();
        assert_eq!(store.passes_sent_to(&cy).len(), 1, "ticking Trade takes nothing away: their pass stays until the new one goes out");
        assert_eq!(store.passes_out_of_step(), vec![cy.clone()], "and the sweep re-issues it");
        assert_eq!(store.pending_withdrawals().len(), 1, "nothing more withdrawn");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// 10c-ii: Unfollow clears the friend's choice, ours from here and ours echoed from another
    /// of our devices, so a friendship begun again starts from the defaults. (Block clears it
    /// too: engine/block.rs `block_withdraws_the_passes_and_unfollows`.)
    /// Seen red 2026-10-10 with `clear_ticks` taken out of `set_follow`: "Unfollow clears the
    /// choice" failed (Call still ticked).
    #[test]
    fn unfollow_clears_the_choice() {
        use crate::net::reach::FriendTicks;
        let (my_seed, me) = identity(87);
        let (_b, ben) = identity(88);
        let (_c, cy) = identity(89);
        let mut gs = app(&me, &my_seed, "reach-unfollow");
        let chosen = FriendTicks { message: false, call: true, trade: true };
        {
            let store = gs.dm_store.as_mut().unwrap();
            for peer in [&ben, &cy] {
                store.set_following(peer, true);
                store.set_follower(peer, true);
                store.set_ticks(peer, chosen);
            }
        }
        crate::engine::dm::set_follow(&mut gs, &ben, false);
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.is_following(&ben));
        assert_eq!(store.ticks(&ben), FriendTicks::default(), "Unfollow clears the choice");

        // Our unfollow of Cy, made on another device and echoed here.
        crate::engine::dm::ingest_dm(&mut gs, &inner(&me, &cy, crate::net::dm_pq::CTL_UNFOLLOW));
        let store = gs.dm_store.as_ref().unwrap();
        assert!(!store.is_following(&cy));
        assert_eq!(store.ticks(&cy), FriendTicks::default(), "an Unfollow from our other device clears it too");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// Ring an idle app as `who`; true when it rang.
    fn rings(gs: &mut GuiState, who: &str) -> bool {
        gs.call_incoming = None;
        crate::engine::call_relay::on_ring(gs, who.to_string(), "Someone".into());
        gs.call_incoming.is_some()
    }

    /// A ring our own call setting would not let through is ignored here too (the relay checks
    /// first; this is for a server that passes one on anyway): under the safe defaults (calls from
    /// People I choose) a stranger's ring, and a friend's whose pass does not name calls, ring
    /// nothing, leave no line and send nothing back; a friend whose pass names calls rings, and so
    /// does one whose pass went unanswered (10l: the relay honours it if it stored it), whose DMs
    /// are DMs too, not requests. Under Anyone a stranger rings.
    /// Seen red 2026-10-10 two ways: without the check in `on_ring`, "a stranger's ring rings
    /// nothing"; with `passes_held_by` reading the record only, "and so does one whose pass went
    /// unanswered".
    #[test]
    fn rings_our_call_setting_refuses_are_ignored() {
        let (my_seed, me) = identity(94);
        let (_s, stranger) = identity(95);
        let (_f, friend) = identity(96);
        let (_c, caller) = identity(97);
        let (_u, unsure) = identity(98);
        let mut gs = app(&me, &my_seed, "reach-rings");
        let (client, sent) = crate::net::ws_client::WsClient::recording();
        gs.ws_client = Some(client);
        let store = gs.dm_store.as_mut().unwrap();
        store.record_pass_sent(&friend, SentPass { serial: "ab".repeat(16), may: "invite,message,trade,voice_message".into() });
        store.record_pass_sent(&caller, SentPass { serial: "cd".repeat(16), may: "call,invite,message,trade,voice_message".into() });
        store.pass_on_its_way(&unsure, SentPass { serial: "ef".repeat(16), may: "call,invite,message,trade,voice_message".into() });

        assert!(!rings(&mut gs, &stranger), "a stranger's ring rings nothing under the defaults");
        assert!(!rings(&mut gs, &friend), "nor a friend's whose pass does not name calls");
        assert!(gs.pending_notices.is_empty(), "an ignored ring leaves no line");
        assert!(sent.try_recv().is_err(), "and sends nothing back");
        assert!(rings(&mut gs, &caller), "a friend whose pass names calls rings");
        assert!(rings(&mut gs, &unsure), "and so does one whose pass went unanswered");
        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&unsure, &me, "hello")), "whose DM is a DM, not a request");

        gs.dm_store.as_mut().unwrap().set_reach_settings(ReachSettings { call: Audience::Anyone, ..Default::default() });
        assert!(rings(&mut gs, &stranger), "under Anyone a stranger rings");
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
    }

    /// 10m R8: GROUP MEMBERSHIP IN THIS APP'S OWN CHECKS IS THE SERVER'S CALL. Under "Friends and
    /// people in my groups" someone the server let a DM or a ring through from counts as sharing
    /// a group: this app's own group list goes stale (refreshed only while Chat is open), and
    /// here it has loaded empty, as a stale one is when Gus has just joined. His DM is a DM, not a
    /// request, on the active server and on a parked server's store, and his ring rings. Under
    /// Friends nothing changes: the same DM is a request.
    /// Seen red 2026-10-10 two ways: with `file_if_refused_in` given the stale list's answer (no
    /// group) instead of the server's, "a DM the server let through under Groups is a DM" failed;
    /// and with `on_ring` asking `shares_group` (the stale list) again, "and his ring rings".
    #[test]
    fn group_membership_is_the_servers_call() {
        let (my_seed, me) = identity(84);
        let (_g, gus) = identity(85);
        let mut gs = app(&me, &my_seed, "reach-groups");
        gs.p2p_groups_last_fetch = Some(std::time::Instant::now());
        assert!(!shares_group(&gs, &gus), "the setup: this app's own list knows no group with Gus");
        let groups = ReachSettings { message: Audience::Groups, call: Audience::Groups, ..Default::default() };
        gs.dm_store.as_mut().unwrap().set_reach_settings(groups);
        assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&gus, &me, "see you at the garden")), "a DM the server let through under Groups is a DM");
        assert!(rings(&mut gs, &gus), "and his ring rings");
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let mut parked = DmStore::load(&my_seed, &me, &format!("wss://reach-parked-{nanos}.example"));
        parked.set_reach_settings(groups);
        assert!(!file_if_refused_in(&mut parked, &inner(&gus, &me, "on another server")), "and on a parked server's store");

        gs.dm_store.as_mut().unwrap().set_reach_settings(ReachSettings::default());
        assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&gus, &me, "under Friends")), "under Friends it is a request");
        assert!(gs.dm_store.as_ref().unwrap().requests().iter().any(|r| r.key == gus));
        gs.dm_store.as_ref().unwrap().remove_file_for_test();
        parked.remove_file_for_test();
    }
}
