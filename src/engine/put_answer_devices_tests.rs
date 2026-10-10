// Child of engine/put_answer_tests.rs (#[path]): section 10m of docs/design/blocking-and-safe-mode.md,
// "Passes across my own devices, and the review of v0.1478 to v0.1481" (2026-10-10), the desktop's
// half, with the app state and a recording socket, no screen. Uses the parent's helpers (`app`,
// `frames`, `puts_to`, `answer`, `sweep`, ...). Each test was seen failing once on purpose,
// recorded at the test.

use super::*;
use crate::net::dm_pq::{DmInner, CTL_FRIEND_CERT};
use crate::net::reach::{intended_may, intended_may_wire};
use crate::relay::core::pq_crypto::build_friend_cert;

/// What each frame was, in order: their copy of a pass put ("theirs"), our self-copy ("ours"), a
/// choice note to our own mailbox ("note", 10n; told apart from a self-copy by opening it with our
/// `seed`), a withdrawal ("withdraw"), anything else by its type.
fn kinds(frames: &[Value], me: &str, seed: &[u8]) -> Vec<String> {
    frames
        .iter()
        .map(|f| match (f["type"].as_str(), f["to"].as_str()) {
            (Some("dm_put"), Some(to)) if to == me && crate::net::choice::ChoiceNote::is_note(&open(f, seed).text) => "note".to_string(),
            (Some("dm_put"), Some(to)) if to == me => "ours".to_string(),
            (Some("dm_put"), _) => "theirs".to_string(),
            (Some("cert_revoke"), _) => "withdraw".to_string(),
            (t, _) => t.unwrap_or("?").to_string(),
        })
        .collect()
}

/// A pass I gave `peer` (minted on my other device, say): the pass JSON and its record.
fn my_pass(seed: &[u8], me: &str, peer: &str, serial: &str, ticks: FriendTicks) -> (String, SentPass) {
    let cert = build_friend_cert(seed, SERVER, me, peer, serial, &intended_may(ticks)).unwrap();
    (cert, SentPass { serial: serial.to_string(), may: intended_may_wire(ticks) })
}

/// That pass, echoed to this device as my other device's self-copy.
fn echo(me: &str, peer: &str, cert: &str) -> DmInner {
    DmInner { from: me.into(), to: peer.into(), ts: 9, text: CTL_FRIEND_CERT.into(), sig_b64: format!("echo-{peer}-{cert}"), cert: Some(cert.into()) }
}

/// The server's `cert_revoked {serial}`, which goes to every device of mine.
fn confirmed(gs: &mut GuiState, serial: &str) {
    crate::engine::dm::withdrawal_confirmed(gs, &json!({ "type": "cert_revoked", "serial": serial }));
}

/// 10n N5, WHICH WITHDREW 10m R2: EVERY PASS'S SELF-COPY WAITS FOR `dm_put_ok`. Unticking Message
/// for Ben sends, in this order, the note telling my other devices the new choice (N1), the
/// withdrawal of the pass that allowed messages (N4), and the new pass, with no self-copy beside
/// it. Taken: the pass is recorded and exactly one self-copy follows, carrying it. Refused (Cy):
/// nothing is recorded, no self-copy ever goes (so no device of mine can adopt a pass the server
/// refused), Cy is still owed a pass, and the next sweep sends one carrying the same choice. An
/// added tick is the same: no self-copy until taken, and the old pass withdrawn then.
/// Seen red 2026-10-10 with `send_held` sending the self-copy at once beside theirs again (10m R2):
/// "the note, the withdrawal, then the new pass, and no self-copy yet" failed (left ["note",
/// "withdraw", "theirs", "ours"]).
#[test]
fn every_self_copy_waits_for_the_servers_answer() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(180);
    let (ben, cy) = (identity(181), identity(182));
    let (mut gs, sent) = app(&seed, &me, "n5", &[ben.clone(), cy.clone()]);
    let (b0, c0) = (SentPass { serial: "31".repeat(16), may: intended_may_wire(FriendTicks::default()) }, SentPass { serial: "32".repeat(16), may: intended_may_wire(FriendTicks::default()) });
    gs.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, b0.clone());
    gs.dm_store.as_mut().unwrap().record_pass_sent(&cy.1, c0.clone());

    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Message, false);
    let out = frames(&sent);
    assert_eq!(kinds(&out, &me, &seed), ["note", "withdraw", "theirs"], "the note, the withdrawal, then the new pass, and no self-copy yet");
    let new = pass_in(&puts_to(&out, &ben.1)[0], &ben.0);
    assert_eq!((new.may.as_str(), revoked(&out)), ("trade", vec![b0.serial.clone()]), "the new pass allows trades only; the old one is withdrawn");
    assert!(gs.dm_store.as_ref().unwrap().passes_sent_to(&ben.1).is_empty(), "the new pass counts as given only once taken (10l)");
    confirmed(&mut gs, &b0.serial); // the server confirms, so it is not resent below
    answer(&mut gs, &puts_to(&out, &ben.1)[0], true, "");
    let out = frames(&sent);
    assert_eq!(kinds(&out, &me, &seed), ["ours"], "taken: exactly one self-copy");
    assert_eq!(pass_in(&out[0], &seed), new, "carrying the new pass");
    assert_eq!(gs.dm_store.as_ref().unwrap().passes_sent_to(&ben.1), [new], "and it is recorded");

    // Refused: nothing recorded, never a self-copy, still owed, sent again with the same choice.
    crate::engine::reach::set_tick(&mut gs, &cy.1, ReachKind::Trade, false);
    let out = frames(&sent);
    assert_eq!(kinds(&out, &me, &seed), ["note", "withdraw", "theirs"]);
    let refused = pass_in(&puts_to(&out, &cy.1)[0], &cy.0);
    confirmed(&mut gs, &c0.serial);
    answer(&mut gs, &puts_to(&out, &cy.1)[0], false, "rate");
    assert!(frames(&sent).is_empty(), "refused: no self-copy ever goes");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&cy.1).is_empty() && store.owed_passes().contains(&cy.1), "nothing recorded, and Cy is still owed a pass");
    sweep(&mut gs);
    let again = frames(&sent);
    assert_eq!(kinds(&again, &me, &seed), ["theirs"], "the next sweep sends one, with no self-copy");
    assert_eq!(pass_in(&again[0], &cy.0).may, refused.may, "carrying the same choice");
    answer(&mut gs, &again[0], true, "");
    let _ = frames(&sent);

    // An added tick keeps the self-copy until the server took theirs, and withdraws the old pass then.
    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Call, true);
    let out = frames(&sent);
    assert_eq!(kinds(&out, &me, &seed), ["note", "theirs"], "an added tick: no self-copy and no withdrawal yet");
    answer(&mut gs, &puts_to(&out, &ben.1)[0], true, "");
    assert_eq!(kinds(&frames(&sent), &me, &seed), ["ours", "withdraw"], "the self-copy once taken, then the old one withdrawn");
    tidy(&gs);
}

/// R3. A PASS WITHDRAWN BY MY OTHER DEVICE IS NOT REPLACED WITH THE DEFAULTS. The server confirms
/// a serial this device holds as given but did not withdraw: it leaves the record, and Ben, left
/// with no pass, is marked "changed on my other device": neither the sweep nor his follow-back
/// sends him one, across a restart too, and People I choose keeps him, "(updating their pass)".
/// Fay, marked the same way but not following us back, is not listed (10n N6: a marked person who
/// is no longer a mutual follow is not, the web's rule). An echo of a pass to Ben clears the
/// mark; so does the person ticking for Cy here (whose pass then goes out at once); a new server
/// identity clears it with the passes. A pass this device had on its way to Eve when hers was
/// withdrawn elsewhere is withdrawn too, so its late `dm_put_ok` records nothing.
/// Seen red 2026-10-10 with `send_owed_passes` not asking `left_alone`: "the sweep sends Ben no
/// pass on its own" failed. (Seen red for 10n N6 at engine/choice_tests.rs, the marked friend.)
#[test]
fn a_pass_my_other_device_withdrew_is_not_replaced_with_the_defaults() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(183);
    let (ben, cy, eve, fay) = (identity(184), identity(185), identity(186), identity(187));
    let (mut gs, sent) = app(&seed, &me, "r3", &[ben.clone(), cy.clone(), eve.clone()]);
    gs.peer_kyber_keys.insert(fay.1.clone(), kyber(&fay.0));
    let dflt = FriendTicks::default();
    let passes: Vec<SentPass> = [&ben, &cy, &eve, &fay].iter().enumerate().map(|(i, p)| {
        let pass = SentPass { serial: format!("{:02x}", 0x40 + i).repeat(16), may: intended_may_wire(dflt) };
        gs.dm_store.as_mut().unwrap().record_pass_sent(&p.1, pass.clone());
        pass
    }).collect();
    gs.dm_store.as_mut().unwrap().set_following(&fay.1, true);

    confirmed(&mut gs, &passes[0].serial);
    confirmed(&mut gs, &passes[3].serial);
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ben.1).is_empty(), "the pass leaves the record here too");
    assert!(store.pending_withdrawals().is_empty(), "nothing for this device to withdraw");
    assert!(store.changed_elsewhere(&ben.1) && store.changed_elsewhere(&fay.1), "marked: changed on my other device");
    sweep(&mut gs);
    crate::engine::dm::send_friend_cert(&mut gs, &ben.1);
    assert!(puts_to(&frames(&sent), &ben.1).is_empty(), "the sweep sends Ben no pass on its own");
    let rows = crate::gui::pages::safety::chosen_rows(&gs);
    assert!(rows.iter().any(|(k, _, _, updating)| k == &ben.1 && *updating), "People I choose keeps Ben, updating their pass");
    assert!(!rows.iter().any(|(k, ..)| k == &fay.1), "but not Fay, who is no longer a mutual follow");
    gs.dm_store.as_ref().unwrap().save();
    let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
    gs.dm_store = Some(DmStore::load(&seed, &me, &server));
    sweep(&mut gs);
    assert!(puts_to(&frames(&sent), &ben.1).is_empty() && gs.dm_store.as_ref().unwrap().changed_elsewhere(&ben.1), "and after a restart");

    // An echo of a pass to Ben from the device that knows: adopted, and the mark is gone.
    let (cert, b1) = my_pass(&seed, &me, &ben.1, &"51".repeat(16), FriendTicks { message: true, call: false, trade: false });
    crate::engine::dm::ingest_dm(&mut gs, &echo(&me, &ben.1, &cert));
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!((store.passes_sent_to(&ben.1), store.changed_elsewhere(&ben.1)), ([b1].as_slice(), false), "an echo clears the mark");

    // Cy: the person's own tick here clears it, and his pass goes at once with what is ticked.
    confirmed(&mut gs, &passes[1].serial);
    crate::engine::reach::set_tick(&mut gs, &cy.1, ReachKind::Call, true);
    let to_cy = puts_to(&frames(&sent), &cy.1);
    assert_eq!(to_cy.len(), 1, "a tick here sends Cy a pass");
    assert!(pass_in(&to_cy[0], &cy.0).may.contains("call") && !gs.dm_store.as_ref().unwrap().changed_elsewhere(&cy.1));

    // Eve: a pass of this device's on its way when hers was withdrawn elsewhere goes too.
    crate::engine::reach::set_tick(&mut gs, &eve.1, ReachKind::Call, true);
    let to_eve = puts_to(&frames(&sent), &eve.1);
    let e1 = pass_in(&to_eve[0], &eve.0);
    confirmed(&mut gs, &passes[2].serial);
    assert!(gs.dm_store.as_ref().unwrap().pending_withdrawals().contains(&e1.serial), "the pass on its way is withdrawn");
    assert_eq!(revoked(&frames(&sent)), [e1.serial.clone()]);
    answer(&mut gs, &to_eve[0], true, "");
    assert!(gs.dm_store.as_ref().unwrap().passes_sent_to(&eve.1).is_empty(), "its late answer records nothing");

    assert!(gs.dm_store.as_mut().unwrap().set_pass_server("did:hum:another"));
    assert!(!gs.dm_store.as_ref().unwrap().changed_elsewhere(&eve.1), "a new server identity clears the mark");
    tidy(&gs);
}

/// R3, ONE OF THIS DEVICE'S UNANSWERED PASSES WITHDRAWN ELSEWHERE (the web's `withdrawalConfirmed`):
/// unticking Message for Gil sends G1 with its self-copy at once and withdraws g0. My other device
/// adopts G1 and withdraws it in turn; the server confirms G1 here, where it is still unanswered.
/// That leaves Gil with no standing pass, so he is marked, G1's late `dm_put_ok` records nothing,
/// and the sweep leaves him alone. His ticks stay usable: a tick here issues exactly what is
/// ticked and clears the mark, so a lost echo can never leave the row stuck.
/// Seen red 2026-10-10 with `withdrawal_confirmed` looking only at passes given (not the
/// unanswered): "marked when an unanswered pass is withdrawn elsewhere" failed.
#[test]
fn an_unanswered_pass_withdrawn_elsewhere_marks_the_friend_and_a_tick_frees_the_row() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(176);
    let gil = identity(177);
    let (mut gs, sent) = app(&seed, &me, "r3u", &[gil.clone()]);
    let g0 = SentPass { serial: "91".repeat(16), may: intended_may_wire(FriendTicks::default()) };
    gs.dm_store.as_mut().unwrap().record_pass_sent(&gil.1, g0.clone());
    crate::engine::reach::set_tick(&mut gs, &gil.1, ReachKind::Message, false);
    let to_gil = puts_to(&frames(&sent), &gil.1);
    let g1 = pass_in(&to_gil[0], &gil.0);
    confirmed(&mut gs, &g0.serial);
    assert!(!gs.dm_store.as_ref().unwrap().changed_elsewhere(&gil.1), "g0 was this device's own withdrawal: no mark");

    confirmed(&mut gs, &g1.serial);
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.changed_elsewhere(&gil.1), "marked when an unanswered pass is withdrawn elsewhere");
    assert!(store.passes_unanswered_to(&gil.1).is_empty() && store.pending_withdrawals().is_empty());
    answer(&mut gs, &to_gil[0], true, "");
    assert!(gs.dm_store.as_ref().unwrap().passes_sent_to(&gil.1).is_empty(), "G1's late answer records nothing");
    sweep(&mut gs);
    assert!(puts_to(&frames(&sent), &gil.1).is_empty(), "the sweep leaves him alone");

    crate::engine::reach::set_tick(&mut gs, &gil.1, ReachKind::Trade, false);
    let to_gil = puts_to(&frames(&sent), &gil.1);
    let want = gs.dm_store.as_ref().unwrap().intended_may_wire(&gil.1);
    assert_eq!(to_gil.len(), 1, "a tick here issues a pass");
    assert_eq!(pass_in(&to_gil[0], &gil.0).may, want, "exactly what is ticked");
    assert!(!gs.dm_store.as_ref().unwrap().changed_elsewhere(&gil.1), "and frees the row");
    tidy(&gs);
}

/// R4. AN UNTICK ALWAYS TAKES BACK WHAT IT UNTICKED, the reviewer's sequence: Ben holds P0 (the
/// defaults); Call is ticked, so P1 (with calls) goes out; Call is unticked before any answer.
/// P0 matches the ticks again, but P1 would allow calls once stored, so it is withdrawn now and
/// its `dm_put_ok` records nothing. The same for Cy, whose Q1 went unanswered for 30 seconds first.
/// Seen red 2026-10-10 with the early return put back before the take-back: "P1 is taken back"
/// failed (nothing withdrawn).
#[test]
fn an_untick_always_takes_back_what_it_unticked() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(188);
    let (ben, cy) = (identity(189), identity(190));
    let (mut gs, sent) = app(&seed, &me, "r4", &[ben.clone(), cy.clone()]);
    let p0 = SentPass { serial: "61".repeat(16), may: intended_may_wire(FriendTicks::default()) };
    let q0 = SentPass { serial: "62".repeat(16), may: intended_may_wire(FriendTicks::default()) };
    gs.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, p0.clone());
    gs.dm_store.as_mut().unwrap().record_pass_sent(&cy.1, q0.clone());

    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Call, true);
    let to_ben = puts_to(&frames(&sent), &ben.1);
    let p1 = pass_in(&to_ben[0], &ben.0);
    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Call, false);
    let out = frames(&sent);
    assert_eq!(revoked(&out), [p1.serial.clone()], "P1 is taken back");
    assert!(puts_to(&out, &ben.1).is_empty(), "P0 is in step again: no new pass");
    answer(&mut gs, &to_ben[0], true, "");
    assert_eq!(gs.dm_store.as_ref().unwrap().passes_sent_to(&ben.1), [p0], "P1's answer records nothing; P0 stands");
    confirmed(&mut gs, &p1.serial);

    crate::engine::reach::set_tick(&mut gs, &cy.1, ReachKind::Call, true);
    let q1 = pass_in(&puts_to(&frames(&sent), &cy.1)[0], &cy.0);
    crate::engine::dm::pace_owed_passes(&mut gs, Instant::now() + Duration::from_secs(31));
    assert_eq!(gs.dm_store.as_ref().unwrap().passes_unanswered_to(&cy.1), [q1.clone()], "unanswered: perhaps given");
    crate::engine::reach::set_tick(&mut gs, &cy.1, ReachKind::Call, false);
    assert_eq!(revoked(&frames(&sent)), [q1.serial], "an unanswered pass that allows calls is taken back too");
    assert_eq!(gs.dm_store.as_ref().unwrap().passes_sent_to(&cy.1), [q0]);
    tidy(&gs);
}

/// R5. AN ECHO OF A PASS THIS DEVICE IS WITHDRAWING IS IGNORED (the web's `adoptEchoedPass`): Ben's
/// pass allowed calls; Call is unticked here, so it is being withdrawn; then its echo arrives late
/// from my other device. It brings nothing back: not the pass, not the Call tick.
/// Seen red 2026-10-10 with the echo arm's withdrawal check taken out: "the late echo brings back
/// nothing" failed (Call ticked again).
#[test]
fn an_echo_of_a_pass_this_device_is_withdrawing_is_ignored() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(191);
    let ben = identity(192);
    let (mut gs, sent) = app(&seed, &me, "r5", &[ben.clone()]);
    let with_call = FriendTicks { call: true, ..FriendTicks::default() };
    let (cert, p) = my_pass(&seed, &me, &ben.1, &"71".repeat(16), with_call);
    gs.dm_store.as_mut().unwrap().set_ticks(&ben.1, with_call);
    gs.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, p.clone());
    crate::engine::reach::set_tick(&mut gs, &ben.1, ReachKind::Call, false);
    assert!(gs.dm_store.as_ref().unwrap().pending_withdrawals().contains(&p.serial), "being withdrawn");
    let _ = frames(&sent);

    crate::engine::dm::ingest_dm(&mut gs, &echo(&me, &ben.1, &cert));
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!(store.ticks(&ben.1), FriendTicks::default(), "the late echo brings back nothing");
    assert!(!store.passes_sent_to(&ben.1).iter().any(|x| x.serial == p.serial), "not the pass either");
    assert!(store.pending_withdrawals().contains(&p.serial) && revoked(&frames(&sent)).is_empty(), "and the withdrawal stands");
    tidy(&gs);
}

/// R6. ONE REQUEST OR PASS ON ITS WAY PER FRIEND, AND UNFOLLOW OR BLOCK DROPS ONE STILL WAITING: a
/// second Send request while the first waits is refused with the web's sentence and sends nothing,
/// as is a request to a friend whose pass is on its way. Unfollow drops the waiting request: its
/// late `dm_put_ok` records nothing and follows no one, and a new request goes at once. Block
/// drops a waiting pass the same way.
/// Seen red 2026-10-10 two ways: with the waiting check taken out of `send_contact_request`, "a
/// second request waits" failed; and with `withdraw_passes` not dropping the send, "Unfollow
/// drops the send still waiting" failed.
#[test]
fn one_send_on_its_way_per_friend_and_unfollow_or_block_drops_it() {
    use crate::engine::reach::{send_contact_request, STILL_WAITING};
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(193);
    let (ann, bo, cy) = (identity(194), identity(195), identity(196));
    let (mut gs, sent) = app(&seed, &me, "r6", &[bo.clone(), cy.clone()]);
    gs.peer_kyber_keys.insert(ann.1.clone(), kyber(&ann.0));
    let requests = |frames: &[Value], to: &str| puts_to(frames, to).into_iter().filter(|p| p["contact_request"] == true).collect::<Vec<_>>();

    send_contact_request(&mut gs, &ann.1).unwrap();
    let first = requests(&frames(&sent), &ann.1);
    assert_eq!(first.len(), 1);
    assert_eq!(send_contact_request(&mut gs, &ann.1), Err(STILL_WAITING.to_string()), "a second request waits");
    assert!(frames(&sent).is_empty(), "and sends nothing");

    crate::engine::dm::set_follow(&mut gs, &ann.1, false);
    assert!(!gs.pending_puts.has_peer(&ann.1), "Unfollow drops the send still waiting");
    let pass = pass_in(&first[0], &ann.0);
    assert!(gs.dm_store.as_ref().unwrap().pending_withdrawals().contains(&pass.serial), "and withdraws its pass, which may have arrived");
    answer(&mut gs, &first[0], true, "");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ann.1).is_empty() && !store.is_following(&ann.1), "its late answer records nothing and follows no one");
    let _ = frames(&sent);
    send_contact_request(&mut gs, &ann.1).unwrap();
    assert_eq!(requests(&frames(&sent), &ann.1).len(), 1, "a new request goes at once");

    sweep(&mut gs);
    let out = frames(&sent);
    let (to_bo, to_cy) = (puts_to(&out, &bo.1), puts_to(&out, &cy.1));
    assert_eq!(send_contact_request(&mut gs, &cy.1), Err(STILL_WAITING.to_string()), "a request while a pass to them waits");
    crate::engine::block::block(&mut gs, &bo.1);
    assert!(!gs.pending_puts.has_peer(&bo.1), "Block drops the send still waiting");
    answer(&mut gs, &to_bo[0], true, "");
    assert!(gs.dm_store.as_ref().unwrap().passes_sent_to(&bo.1).is_empty(), "its late answer records nothing");
    answer(&mut gs, &to_cy[0], true, "");
    tidy(&gs);
}

/// R7. A PASS REFUSED FOR "reach" IS NOT SENT AGAIN ON ITS OWN: Ben's settings turned our pass
/// away, so the sweep leaves him alone for the rest of the run (the refusal is not repeated each
/// minute), while Cy's, refused for the server's pace, goes again. When Ben's own pass reaches us
/// ours goes again, carrying his past his gate; Dee's goes again at once when the person ticks
/// for her here.
/// Seen red 2026-10-10 with `not_taken` not remembering the reach refusal: "not sent again on its
/// own" failed (a second pass went to Ben).
#[test]
fn a_pass_refused_for_their_settings_is_not_sent_again_on_its_own() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(197);
    let (ben, cy, dee) = (identity(198), identity(199), identity(179));
    let (mut gs, sent) = app(&seed, &me, "r7", &[ben.clone(), cy.clone(), dee.clone()]);
    sweep(&mut gs);
    let out = frames(&sent);
    answer(&mut gs, &puts_to(&out, &ben.1)[0], false, "reach");
    answer(&mut gs, &puts_to(&out, &cy.1)[0], false, "rate");
    answer(&mut gs, &puts_to(&out, &dee.1)[0], false, "reach");
    for _ in 0..2 {
        sweep(&mut gs);
        crate::engine::dm::pace_owed_passes(&mut gs, Instant::now());
        let out = frames(&sent);
        assert!(puts_to(&out, &ben.1).is_empty() && puts_to(&out, &dee.1).is_empty(), "not sent again on its own");
        for p in puts_to(&out, &cy.1) {
            answer(&mut gs, &p, false, "rate");
        }
    }

    // Ben's pass reaches us: ours goes again, carrying his.
    let his = build_friend_cert(&ben.0, SERVER, &ben.1, &me, &"81".repeat(16), &intended_may(FriendTicks::default())).unwrap();
    let from_ben = DmInner { from: ben.1.clone(), to: me.clone(), ts: 3, text: CTL_FRIEND_CERT.into(), sig_b64: "from-ben".into(), cert: Some(his.clone()) };
    crate::engine::dm::ingest_dm(&mut gs, &from_ben);
    sweep(&mut gs);
    let to_ben = puts_to(&frames(&sent), &ben.1);
    assert_eq!(to_ben.len(), 1, "once his pass reaches us, ours goes again");
    assert_eq!(to_ben[0]["friend_cert"].as_str(), Some(his.as_str()), "carrying his past his gate");

    crate::engine::reach::set_tick(&mut gs, &dee.1, ReachKind::Call, true);
    assert_eq!(puts_to(&frames(&sent), &dee.1).len(), 1, "a tick here sends Dee one at once");
    tidy(&gs);
}
