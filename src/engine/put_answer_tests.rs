// Child of engine/put_answer.rs (#[path]): 10l's client proof list, with the app state and a
// recording socket, no screen. A pass counts as given only once the server answered `dm_put_ok`
// for it; after `dm_put_refused` or 30 seconds of silence nothing is recorded or withdrawn and the
// next sweep sends it again with the same intended `may`. Each test was seen failing once on
// purpose, recorded at the test.

use super::*;
use crate::net::dm_store::{DmStore, SentPass};
use crate::net::reach::{FriendTicks, ReachKind};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, parse_friend_cert, DilithiumKeypair};
use serde_json::{json, Value};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

fn kyber(seed: &[u8]) -> String {
    crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64()
}

/// A signed-in app on a server of its own, with a socket that records what is sent, and every
/// one of `friends` a mutual follow whose DM key is known.
fn app(seed: &[u8], me: &str, tag: &str, friends: &[(Vec<u8>, String)]) -> (GuiState, Receiver<String>) {
    let mut gs = GuiState::default();
    gs.profile_public_key = me.to_string();
    gs.private_key_bytes = Some(seed.to_vec());
    gs.user_name = "Me".into();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("https://{tag}-{nanos}.example");
    let mut store = DmStore::load(seed, me, &crate::gui::pages::chat::norm_server_url(&gs.server_url));
    store.set_pass_server(SERVER);
    for (s, k) in friends {
        gs.peer_kyber_keys.insert(k.clone(), kyber(s));
        store.set_following(k, true);
        store.set_follower(k, true);
    }
    gs.dm_store = Some(store);
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(seed, me, tag));
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    // This connection's mailbox has been read (10n N7), so the pass sweep and passes run.
    (gs.dm_fetch_sent, gs.dm_fetch_done) = (true, true);
    (gs, sent)
}

fn tidy(gs: &GuiState) {
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
    gs.block_list.as_ref().unwrap().remove_file_for_test();
}

/// Everything sent since the last look.
fn frames(sent: &Receiver<String>) -> Vec<Value> {
    sent.try_iter().map(|f| serde_json::from_str(&f).expect("a JSON frame")).collect()
}

/// The `dm_put`s among `frames` addressed to `to`.
fn puts_to(frames: &[Value], to: &str) -> Vec<Value> {
    frames.iter().filter(|v| v["type"] == "dm_put" && v["to"] == to).cloned().collect()
}

/// Our self-copies among `frames`: the `dm_put`s to our own mailbox (`me`, opened with our `seed`)
/// that are not choice notes (10n), which go to our own mailbox too.
fn self_copies(frames: &[Value], me: &str, seed: &[u8]) -> Vec<Value> {
    puts_to(frames, me).into_iter().filter(|p| !crate::net::choice::ChoiceNote::is_note(&open(p, seed).text)).collect()
}

/// The choice notes (10n) among `frames`, read: each one's friend, `may` and signed time.
fn notes_in(frames: &[Value], me: &str, seed: &[u8]) -> Vec<(crate::net::choice::ChoiceNote, u64)> {
    puts_to(frames, me)
        .iter()
        .map(|p| open(p, seed))
        .filter_map(|i| crate::net::choice::ChoiceNote::parse(&i.text).map(|n| (n, i.ts)))
        .collect()
}

/// The serials `frames` asked the relay to withdraw.
fn revoked(frames: &[Value]) -> Vec<String> {
    frames.iter().filter(|v| v["type"] == "cert_revoke").map(|v| v["serial"].as_str().unwrap().to_string()).collect()
}

/// Open a `dm_put` with its recipient's seed: the verified inner payload.
fn open(put: &Value, seed: &[u8]) -> crate::net::dm_pq::DmInner {
    let kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap();
    crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&kp, put["content"].as_str().unwrap()).unwrap()).unwrap()
}

/// The pass a `dm_put` carries (a friendship pass or a contact request's): its serial and `may`.
fn pass_in(put: &Value, seed: &[u8]) -> SentPass {
    let inner = open(put, seed);
    let cert = match inner.cert {
        Some(c) => c,
        None => crate::net::reach::parse_contact_request_text(&inner.text).expect("a pass or a request").1,
    };
    let (pass, _) = parse_friend_cert(&cert).unwrap();
    SentPass { serial: pass.serial, may: pass.may.wire() }
}

fn answer(gs: &mut GuiState, put: &Value, ok: bool, reason: &str) {
    let r = put["ref"].as_str().expect("the put carries a ref");
    let frame = if ok { json!({ "type": "dm_put_ok", "ref": r }) } else { json!({ "type": "dm_put_refused", "ref": r, "reason": reason }) };
    assert!(on_frame(gs, &frame), "the answer is handled here");
}

/// The next sweep (on a member list), with a fresh budget so the pace never decides the test.
fn sweep(gs: &mut GuiState) {
    gs.pass_pacer = Default::default();
    crate::engine::dm::sweep_friend_passes(gs);
}

/// A PASS IS RECORDED, AND THE OLD ONE WITHDRAWN, ONLY AFTER `dm_put_ok` (10l). A re-issue (Ben's
/// Call ticked) goes out carrying a ref, with no self-copy and no withdrawal: Ben's old pass still
/// stands and the new one is not on the record. On `dm_put_ok` the new one is recorded, the old one
/// withdrawn, and only then our self-copy goes out; when that self-copy comes back to this device
/// it changes nothing. A new pass (Cy, owed one) likewise counts only once taken, and while it is
/// on its way the sweep does not send her a second.
/// Seen red 2026-10-10 with `send_held` recording the pass as it was written to the socket (the
/// old behaviour): "nothing recorded until the server took it" failed.
#[test]
fn a_pass_counts_as_given_only_once_the_server_took_it() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(230);
    let ben = identity(231);
    let cy = identity(232);
    let (mut gs, sent) = app(&seed, &me, "answer-ok", &[ben.clone(), cy.clone()]);
    let old = SentPass { serial: "11".repeat(16), may: crate::net::reach::intended_may_wire(FriendTicks::default()) };
    gs.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, old.clone());

    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Call, true);
    let out = frames(&sent);
    let to_ben = puts_to(&out, &ben.1);
    assert_eq!(to_ben.len(), 1, "the new pass goes to Ben");
    let new = pass_in(&to_ben[0], &ben.0);
    let want = gs.dm_store.as_ref().unwrap().intended_may_wire(&ben.1);
    assert_eq!(new.may, want, "allowing what is ticked now");
    assert!(self_copies(&out, &me, &seed).is_empty() && revoked(&out).is_empty(), "no self-copy and no withdrawal yet");
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!(store.passes_sent_to(&ben.1), [old.clone()], "nothing recorded until the server took it: the old pass stands");
    assert!(store.pending_withdrawals().is_empty());

    answer(&mut gs, &to_ben[0], true, "");
    let out = frames(&sent);
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!(store.passes_sent_to(&ben.1), [new.clone()], "taken: the new pass is recorded");
    assert_eq!(store.pending_withdrawals(), [old.serial.clone()], "and the old one withdrawn");
    assert_eq!(revoked(&out), [old.serial.clone()], "the withdrawal goes to the relay");
    let mine = puts_to(&out, &me);
    assert_eq!(mine.len(), 1, "our self-copy goes out now");
    assert_eq!(pass_in(&mine[0], &seed), new, "carrying the same pass");
    // The self-copy comes back to this device: already on the record, it changes nothing.
    let ticks = gs.dm_store.as_ref().unwrap().ticks(&ben.1);
    crate::engine::dm::ingest_dm(&mut gs, &open(&mine[0], &seed));
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!((store.ticks(&ben.1), store.passes_sent_to(&ben.1)), (ticks, [new].as_slice()), "its echo changes nothing");

    // Cy is owed a pass: the sweep sends it, and it counts only once taken.
    sweep(&mut gs);
    let to_cy = puts_to(&frames(&sent), &cy.1);
    assert_eq!(to_cy.len(), 1, "the sweep sends Cy a pass");
    assert!(!gs.dm_store.as_ref().unwrap().cert_sent_to(&cy.1), "not recorded while on its way");
    sweep(&mut gs);
    assert!(puts_to(&frames(&sent), &cy.1).is_empty(), "and the next sweep leaves her alone while it is");
    answer(&mut gs, &to_cy[0], true, "");
    assert_eq!(gs.dm_store.as_ref().unwrap().passes_sent_to(&cy.1), [pass_in(&to_cy[0], &cy.0)], "taken: recorded");
    assert!(gs.pending_puts.is_empty());
    tidy(&gs);
}

/// AFTER `dm_put_refused`, OR 30 SECONDS OF SILENCE, NOTHING IS RECORDED OR WITHDRAWN (10l): Ben's
/// old pass stands, nothing waits to be withdrawn, his ticks are as the person left them, and the
/// next sweep sends him a new pass with the same intended `may`. A friend owed a first pass (Dee)
/// is likewise still owed. A refused pass is forgotten; an unanswered one is kept aside, never as
/// given, and taken back with the rest when the passes to Ben are (an Unfollow here), since it may
/// have reached him; its answer, coming late, records nothing.
/// Seen red 2026-10-10 two ways: with `not_taken` recording the pass as taken, "a refusal records
/// nothing" failed; and with `expire` settling a silent send as taken, "silence records nothing"
/// failed.
#[test]
fn a_refused_or_unanswered_pass_records_nothing_and_goes_again_with_the_same_may() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(233);
    let ben = identity(234);
    let dee = identity(235);
    let (mut gs, sent) = app(&seed, &me, "answer-no", &[ben.clone(), dee.clone()]);
    let old = SentPass { serial: "22".repeat(16), may: crate::net::reach::intended_may_wire(FriendTicks::default()) };
    gs.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, old.clone());
    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Call, true);
    let ticks = gs.dm_store.as_ref().unwrap().ticks(&ben.1);
    let want = gs.dm_store.as_ref().unwrap().intended_may_wire(&ben.1);
    let nothing_moved = |gs: &GuiState, why: &str| {
        let store = gs.dm_store.as_ref().unwrap();
        assert_eq!(store.passes_sent_to(&ben.1), [old.clone()], "{why}: the old pass stands, nothing new recorded");
        assert!(store.pending_withdrawals().is_empty(), "{why}: nothing withdrawn");
        assert_eq!(store.ticks(&ben.1), ticks, "{why}: the ticks are unchanged");
        assert!(!store.cert_sent_to(&dee.1), "{why}: Dee is still owed");
    };

    // The re-issue, and a sweep for Dee, both refused.
    sweep(&mut gs);
    let out = frames(&sent);
    let (first, dee_first) = (puts_to(&out, &ben.1), puts_to(&out, &dee.1));
    assert_eq!((first.len(), dee_first.len()), (1, 1));
    answer(&mut gs, &first[0], false, "rate");
    answer(&mut gs, &dee_first[0], false, "rate");
    nothing_moved(&gs, "a refusal records nothing");
    assert!(gs.dm_store.as_ref().unwrap().passes_unanswered_to(&ben.1).is_empty(), "a refused pass was never stored: forgotten");
    let out = frames(&sent);
    assert!(puts_to(&out, &me).is_empty() && revoked(&out).is_empty(), "no self-copy, no withdrawal");

    // The next sweep sends both again, Ben's with the same intended may, under a new serial.
    sweep(&mut gs);
    let out = frames(&sent);
    let (second, dee_second) = (puts_to(&out, &ben.1), puts_to(&out, &dee.1));
    assert_eq!((second.len(), dee_second.len()), (1, 1), "the next sweep sends again");
    let (p1, p2) = (pass_in(&first[0], &ben.0), pass_in(&second[0], &ben.0));
    assert_eq!((p2.may.as_str(), p1.may.as_str()), (want.as_str(), want.as_str()), "with the same intended may");
    assert_ne!(p1.serial, p2.serial, "under a new serial");
    assert_eq!(pass_in(&dee_second[0], &dee.0).may, pass_in(&dee_first[0], &dee.0).may);

    // Silence: 30 seconds and no answer.
    crate::engine::dm::pace_owed_passes(&mut gs, Instant::now() + Duration::from_secs(31));
    assert!(gs.pending_puts.is_empty(), "30 seconds: no longer waiting");
    nothing_moved(&gs, "silence records nothing");
    assert!(frames(&sent).iter().all(|f| f["type"] != "cert_revoke"), "and withdraws nothing");
    answer(&mut gs, &second[0], true, "");
    nothing_moved(&gs, "an answer after the 30 seconds records nothing");
    sweep(&mut gs);
    let third = puts_to(&frames(&sent), &ben.1);
    assert_eq!(third.len(), 1, "the next sweep sends again");
    assert_eq!(pass_in(&third[0], &ben.0).may, want, "still with the same intended may");

    // The unanswered pass may have reached Ben: Unfollow takes it back with the rest.
    crate::engine::dm::set_follow(&mut gs, &ben.1, false);
    let withdrawn = gs.dm_store.as_ref().unwrap().pending_withdrawals().to_vec();
    for s in [&old.serial, &p2.serial, &pass_in(&third[0], &ben.0).serial] {
        assert!(withdrawn.contains(s), "Unfollow withdraws {s}, given, unanswered or on its way: {withdrawn:?}");
    }
    assert!(!withdrawn.contains(&p1.serial), "but not the refused one, which was never stored");
    tidy(&gs);
}

/// A CONTACT REQUEST'S PASS COUNTS ONLY ONCE THE SERVER TOOK THE REQUEST (10l): the request goes
/// out with a ref and nothing recorded; on `dm_put_ok` its pass is recorded and our self-copy goes
/// out. A refused request records nothing and forgets its pass; one with no answer in 30 seconds
/// records nothing either; after either, the conversation's notice offers Send request again and
/// says the server did not confirm it. AND ASKING COUNTS AS FOLLOWING ONLY THEN (10m R1, as on the
/// web): not while the request waits, and never for one refused or unanswered.
/// Seen red 2026-10-10 with `send_held` (which the request goes out through) recording the pass as
/// it was written to the socket, the old behaviour: "nothing recorded until the server took it"
/// failed. 10m R1 seen red 2026-10-10 with `send_contact_request` following at send time again
/// (the old behaviour): "not following until the server took it" failed.
#[test]
fn a_contact_requests_pass_counts_only_once_the_server_took_it() {
    use crate::net::reach::Refusal;
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(236);
    let (mut gs, sent) = app(&seed, &me, "answer-req", &[]);
    let people: Vec<(Vec<u8>, String)> = (237..240u8).map(identity).collect();
    for (s, k) in &people {
        gs.peer_kyber_keys.insert(k.clone(), kyber(s));
        gs.reach.refused.insert(k.clone(), Refusal::Refused);
    }
    let [ben, cy, dee] = [&people[0], &people[1], &people[2]];

    crate::engine::reach::send_contact_request(&mut gs, &ben.1).unwrap();
    let out = frames(&sent);
    let request = puts_to(&out, &ben.1);
    assert_eq!(request.len(), 1);
    assert_eq!(request[0]["contact_request"], true, "a contact request");
    assert!(puts_to(&out, &me).is_empty(), "no self-copy yet");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ben.1).is_empty(), "nothing recorded until the server took it");
    assert!(!store.is_following(&ben.1), "not following until the server took it");
    answer(&mut gs, &request[0], true, "");
    let out = frames(&sent);
    assert_eq!(gs.dm_store.as_ref().unwrap().passes_sent_to(&ben.1), [pass_in(&request[0], &ben.0)], "taken: its pass is recorded");
    assert!(gs.dm_store.as_ref().unwrap().is_following(&ben.1), "taken: asking is following now");
    assert!(gs.chat_following_keys.contains(&ben.1), "and the follow shows");
    assert_eq!(puts_to(&out, &me).len(), 1, "and our self-copy goes out");
    assert_eq!(gs.reach.refused.get(&ben.1), Some(&Refusal::RequestSent), "the notice says it was sent");

    crate::engine::reach::send_contact_request(&mut gs, &cy.1).unwrap();
    let to_cy = puts_to(&frames(&sent), &cy.1);
    answer(&mut gs, &to_cy[0], false, "rate");
    crate::engine::reach::send_contact_request(&mut gs, &dee.1).unwrap();
    crate::engine::dm::pace_owed_passes(&mut gs, Instant::now() + Duration::from_secs(31));
    let store = gs.dm_store.as_ref().unwrap();
    for (who, key) in [("refused", &cy.1), ("unanswered", &dee.1)] {
        assert!(store.passes_sent_to(key).is_empty(), "{who}: nothing recorded");
        assert!(!store.is_following(key), "{who}: not following");
        assert_eq!(gs.reach.refused.get(key), Some(&Refusal::Refused), "{who}: the notice offers Send request again");
    }
    assert!(store.passes_unanswered_to(&cy.1).is_empty(), "the refused request's pass is forgotten");
    assert_eq!(store.passes_unanswered_to(&dee.1).len(), 1, "the unanswered one is kept aside, to be withdrawn with the rest");
    assert!(store.pending_withdrawals().is_empty(), "nothing withdrawn");
    assert_eq!(gs.reach.status, REQUEST_NOT_CONFIRMED);
    assert!(puts_to(&frames(&sent), &me).is_empty(), "and no self-copy for either");
    tidy(&gs);
}

/// OUR SELF-COPY GOES ONLY AFTER THE SERVER TOOK THEIRS (10l, and the web's rule): our other
/// devices read the self-copy and adopt the pass in it as given, withdrawing what the friend
/// really holds, so it must never go out for a pass the server has not taken. Three passes go out
/// with no self-copy; Ben's is taken and exactly one self-copy follows, carrying his pass; Cy's is
/// refused and Dee's is never answered (even when its answer comes late), and neither ever gets one.
/// Seen red 2026-10-10 with `send_held` sending the self-copy at once beside theirs: "no self-copy
/// before the server took theirs" failed (three went out).
#[test]
fn our_self_copy_goes_only_after_the_server_took_theirs() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(241);
    let (ben, cy, dee) = (identity(242), identity(243), identity(244));
    let (mut gs, sent) = app(&seed, &me, "answer-self", &[ben.clone(), cy.clone(), dee.clone()]);
    sweep(&mut gs);
    let out = frames(&sent);
    assert!(puts_to(&out, &me).is_empty(), "no self-copy before the server took theirs");
    let (to_ben, to_cy, to_dee) = (puts_to(&out, &ben.1), puts_to(&out, &cy.1), puts_to(&out, &dee.1));
    assert_eq!((to_ben.len(), to_cy.len(), to_dee.len()), (1, 1, 1), "a pass to each");

    answer(&mut gs, &to_ben[0], true, "");
    let mine = puts_to(&frames(&sent), &me);
    assert_eq!(mine.len(), 1, "taken: one self-copy");
    let echo = open(&mine[0], &seed);
    assert_eq!((echo.to.as_str(), pass_in(&mine[0], &seed)), (ben.1.as_str(), pass_in(&to_ben[0], &ben.0)), "carrying Ben's pass");

    answer(&mut gs, &to_cy[0], false, "rate");
    crate::engine::dm::pace_owed_passes(&mut gs, Instant::now() + Duration::from_secs(31));
    answer(&mut gs, &to_dee[0], true, "");
    assert!(puts_to(&frames(&sent), &me).is_empty(), "refused, unanswered, or answered too late: never a self-copy");
    tidy(&gs);
}

/// A PASS NEVER ANSWERED IS "PERHAPS GIVEN" (10l, and the web's rule): the server may have stored
/// it after its answer was lost, so it never counts as given but is taken back by the next pass to
/// that friend the server does take, and by Block (Unfollow: the test above). One pass put per
/// friend is on its way at a time.
/// Seen red 2026-10-10 with `taken` not withdrawing the unanswered passes: "the next pass taken
/// takes back the one never answered" failed.
#[test]
fn a_pass_never_answered_is_taken_back_by_the_next_one_taken_and_by_block() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(245);
    let (ben, cy) = (identity(246), identity(247));
    let (mut gs, sent) = app(&seed, &me, "answer-perhaps", &[ben.clone(), cy.clone()]);
    let silence = |gs: &mut GuiState| crate::engine::dm::pace_owed_passes(gs, Instant::now() + Duration::from_secs(31));

    sweep(&mut gs);
    let out = frames(&sent);
    let (p1, q1) = (pass_in(&puts_to(&out, &ben.1)[0], &ben.0), pass_in(&puts_to(&out, &cy.1)[0], &cy.0));
    silence(&mut gs);
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!((store.passes_unanswered_to(&ben.1), store.passes_unanswered_to(&cy.1)), ([p1.clone()].as_slice(), [q1.clone()].as_slice()), "kept aside, perhaps given");
    assert!(!store.cert_sent_to(&ben.1) && store.pending_withdrawals().is_empty(), "never as given, and nothing withdrawn yet");

    sweep(&mut gs);
    let out = frames(&sent);
    let (to_ben, to_cy) = (puts_to(&out, &ben.1), puts_to(&out, &cy.1));
    assert_eq!((to_ben.len(), to_cy.len()), (1, 1), "the next sweep sends each another");
    sweep(&mut gs);
    assert!(puts_to(&frames(&sent), &ben.1).is_empty(), "one on its way per friend at a time");

    answer(&mut gs, &to_ben[0], true, "");
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!(store.passes_sent_to(&ben.1), [pass_in(&to_ben[0], &ben.0)], "the new one is given");
    assert_eq!(store.pending_withdrawals(), [p1.serial.clone()], "the next pass taken takes back the one never answered");
    assert!(store.passes_unanswered_to(&ben.1).is_empty());
    assert!(revoked(&frames(&sent)).contains(&p1.serial), "and the withdrawal goes to the relay");

    silence(&mut gs);
    let q2 = pass_in(&to_cy[0], &cy.0);
    crate::engine::block::block(&mut gs, &cy.1);
    let withdrawn = gs.dm_store.as_ref().unwrap().pending_withdrawals().to_vec();
    assert!(withdrawn.contains(&q1.serial) && withdrawn.contains(&q2.serial), "Block takes back both passes Cy never had answered: {withdrawn:?}");
    tidy(&gs);
}

/// Section 10m (passes across my own devices, and the review of v0.1478 to v0.1481).
#[path = "put_answer_devices_tests.rs"]
mod devices;

/// Section 10n (the choice for each friend syncs as its own note): each rule, and the five
/// two-device sequences from the reviews. A child here so it shares this file's helpers.
#[path = "choice_tests.rs"]
mod choice;
