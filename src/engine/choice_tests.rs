// Child of engine/put_answer_tests.rs (#[path]): section 10n of docs/design/blocking-and-safe-mode.md,
// "The choice for each friend syncs as its own note" (2026-10-10), the desktop's half, with the app
// state and a recording socket, no screen. Two `GuiState`s sharing one seed (each with its own DM
// store and socket) are two devices of one person; what one sends to our own mailbox is delivered to
// the other the way its mailbox fetch or a live `dm_new` would (`deliver`). Uses the parent's
// helpers (`app`, `frames`, `puts_to`, `notes_in`, `answer`, `sweep`, ...). Each test was seen
// failing once on purpose, recorded at the test. The note's bytes against the test vector are in
// net/choice.rs; the store's half of N2 in net/dm_store.rs.

use super::*;
use crate::net::choice::ChoiceNote;
use crate::net::dm_pq::{build_signed_inner, parse_verify_inner, DmInner, CTL_FOLLOW, CTL_UNFOLLOW};
use crate::net::reach::intended_may_wire;

/// The defaults' `may`, what a friend nobody chose anything for gets.
fn defaults() -> String {
    intended_may_wire(FriendTicks::default())
}

/// A pass of ours with the defaults, as if minted and taken before the test starts.
fn default_pass(serial: &str) -> SentPass {
    SentPass { serial: serial.repeat(16), may: defaults() }
}

/// Deliver to device `to`, in order, every `dm_put` among `frames` that went to our own mailbox
/// (choice notes and self-copies), as its mailbox fetch or a live `dm_new` would.
fn deliver(to: &mut GuiState, frames: &[Value], me: &str, seed: &[u8]) {
    for put in puts_to(frames, me) {
        crate::engine::dm::ingest_dm(to, &open(&put, seed));
    }
}

/// A choice note as my other device signs it: from me, to me, at `at`.
fn note(seed: &[u8], me: &str, key: &str, may: &str, at: u64) -> DmInner {
    parse_verify_inner(&build_signed_inner(seed, me, me, at, &ChoiceNote::new(key, may).text()).unwrap()).unwrap()
}

/// The server's `cert_revoked {serial}`, which goes to every device of mine.
fn confirmed(gs: &mut GuiState, serial: &str) {
    crate::engine::dm::withdrawal_confirmed(gs, &json!({ "type": "cert_revoked", "serial": serial }));
}

/// What each frame was: "note" (a choice note to our own mailbox), "ours" (a self-copy), "theirs"
/// (a put to someone else), "withdraw", or its type.
fn order(frames: &[Value], me: &str, seed: &[u8]) -> Vec<String> {
    frames
        .iter()
        .map(|f| match (f["type"].as_str(), f["to"].as_str()) {
            (Some("dm_put"), Some(to)) if to == me && ChoiceNote::is_note(&open(f, seed).text) => "note".to_string(),
            (Some("dm_put"), Some(to)) if to == me => "ours".to_string(),
            (Some("dm_put"), _) => "theirs".to_string(),
            (Some("cert_revoke"), _) => "withdraw".to_string(),
            (t, _) => t.unwrap_or("?").to_string(),
        })
        .collect()
}

/// The last page of this device's mailbox was read: the sweep that waited for it runs (N7), with a
/// fresh budget so the pace never decides the test.
fn mailbox_read(gs: &mut GuiState) {
    gs.pass_pacer = Default::default();
    crate::engine::dm::on_mailbox_read(gs);
}

fn ticks(message: bool, call: bool, trade: bool) -> FriendTicks {
    FriendTicks { message, call, trade }
}

// ── The rules ───────────────────────────────────────────────────────────────────────────────

/// N1. A TICK SENDS ITS NOTE FIRST: unticking Message for Ben sends, in this order, the note to my
/// own mailbox (`[[hum:choice:v1]]<Ben>/trade`, from me to me, signed with the choice's own time),
/// the withdrawal of the pass that allowed messages, and the new pass. The echo of that note,
/// coming back to this device, changes nothing (it was remembered as it went out).
/// Seen red 2026-10-10 with `make` calling `follow_choice` before `send_or_queue`: "the note
/// first, then the withdrawal, then the new pass" failed (left ["withdraw", "theirs", "note"]).
#[test]
fn a_tick_sends_its_note_first_then_the_withdrawal_and_the_pass() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(150);
    let ben = identity(151);
    let (mut a, sent) = app(&seed, &me, "n1-order", &[ben.clone()]);
    a.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, default_pass("a0"));

    crate::engine::reach::set_tick(&mut a, &ben.1, ReachKind::Message, false);
    let out = frames(&sent);
    assert_eq!(order(&out, &me, &seed), ["note", "withdraw", "theirs"], "the note first, then the withdrawal, then the new pass");
    let at = a.dm_store.as_ref().unwrap().choice(&ben.1).map(|c| c.at).unwrap();
    assert_eq!(notes_in(&out, &me, &seed), [(ChoiceNote::new(&ben.1, "trade"), at)], "the note says the new choice, signed with its time");
    let sealed = open(&puts_to(&out, &me)[0], &seed);
    assert_eq!((sealed.from.as_str(), sealed.to.as_str()), (me.as_str(), me.as_str()), "from me, to me only");
    assert_eq!(sealed.text, format!("[[hum:choice:v1]]{}/trade", ben.1));
    assert_eq!(pass_in(&puts_to(&out, &ben.1)[0], &ben.0).may, "trade");

    a.dm_store.as_mut().unwrap().set_ticks(&ben.1, ticks(true, false, true)); // a later choice here
    crate::engine::dm::ingest_dm(&mut a, &sealed);
    assert_eq!(a.dm_store.as_ref().unwrap().ticks(&ben.1), ticks(true, false, true), "its echo changes nothing");
    tidy(&a);
}

/// N1. A CHOICE MADE OFFLINE WAITS, AND GOES FIRST ON THE NEXT CONNECTION: with no server connected,
/// Message then Trade unticked for Ben and Call ticked for Cy send nothing; one note per friend
/// waits (Ben's newer one replacing his older), signed with each choice's time, across a restart
/// too. The untick took Ben's pass back at once. On the next connection, once its mailbox was
/// read, both notes go BEFORE the withdrawal and the passes.
/// Seen red 2026-10-10 with `sweep_friend_passes` flushing the notes after `send_pending_withdrawals`:
/// "the notes first, then the withdrawal" failed (left ["withdraw", "note", "note", ...]).
#[test]
fn a_choice_made_offline_waits_and_goes_first_on_the_next_connection() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(152);
    let (ben, cy) = (identity(153), identity(154));
    let (mut a, sent) = app(&seed, &me, "n1-offline", &[ben.clone(), cy.clone()]);
    let (b0, c0) = (default_pass("b0"), default_pass("c0"));
    a.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, b0.clone());
    a.dm_store.as_mut().unwrap().record_pass_sent(&cy.1, c0);

    let link = a.ws_client.take();
    crate::engine::reach::set_tick(&mut a, &ben.1, ReachKind::Message, false);
    crate::engine::reach::set_tick(&mut a, &ben.1, ReachKind::Trade, false);
    crate::engine::reach::set_tick(&mut a, &cy.1, ReachKind::Call, true);
    assert!(frames(&sent).is_empty(), "nothing goes while not connected");
    let store = a.dm_store.as_ref().unwrap();
    let waiting: Vec<(String, String, u64)> = store.pending_choices().iter().map(|p| (p.peer.clone(), p.may.clone(), p.at)).collect();
    let at = |k: &str| store.choice(k).map(|c| c.at).unwrap();
    let cy_may = intended_may_wire(ticks(true, true, true));
    assert_eq!(waiting, [(ben.1.clone(), "invite".to_string(), at(&ben.1)), (cy.1.clone(), cy_may.clone(), at(&cy.1))], "one note per friend, the newer replacing the older");
    assert_eq!(store.pending_withdrawals(), [b0.serial.clone()], "the untick took Ben's pass back at once");

    store.save();
    let server = crate::gui::pages::chat::norm_server_url(&a.server_url);
    a.dm_store = Some(DmStore::load(&seed, &me, &server));
    assert_eq!(a.dm_store.as_ref().unwrap().pending_choices().len(), 2, "kept across a restart");

    a.ws_client = link;
    a.dm_fetch.done = false; // a new connection, its mailbox not read yet
    sweep(&mut a);
    assert!(frames(&sent).is_empty(), "nothing before the mailbox was read (N7)");
    mailbox_read(&mut a);
    let out = frames(&sent);
    assert_eq!(order(&out, &me, &seed), ["note", "note", "withdraw", "theirs", "theirs"], "the notes first, then the withdrawal, then the passes");
    let store = a.dm_store.as_ref().unwrap();
    assert_eq!(
        notes_in(&out, &me, &seed),
        [(ChoiceNote::new(&ben.1, "invite"), store.choice(&ben.1).unwrap().at), (ChoiceNote::new(&cy.1, &cy_may), store.choice(&cy.1).unwrap().at)]
    );
    assert!(store.pending_choices().is_empty(), "nothing left waiting");
    tidy(&a);
}

/// N2. A NOTE IS APPLIED ONCE, THE NEWER CHOICE WINNING, AND NEVER SHOWN: a note is never a
/// message; an older one changes nothing; at an equal time the larger `may` text wins; a note
/// delivered again changes nothing, not even the "changed on my other device" mark, which a new
/// note about the friend clears (N6). A choice note signed by anyone else, or from me to someone
/// else, is dropped unread (not stored, not a request); one about someone I blocked, one naming
/// me, and a malformed one change nothing and are never shown.
/// Seen red 2026-10-10 two ways: with `apply_in`'s "from us AND to us" check taken out, "a note
/// from anyone else is dropped unread" failed (Ben's choice became "call"); and with `apply_in` not
/// asking `first_sight_note`, "delivered again it changes nothing, not even the mark" failed.
#[test]
fn a_note_is_applied_once_the_newer_winning_and_never_shown() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(155);
    let (ben, eve, cy) = (identity(156), identity(157), identity(158));
    let (mut b, _sent) = app(&seed, &me, "n2", &[ben.clone()]);
    let base = 1_760_000_000_000u64;
    let may = |b: &GuiState, k: &str| b.dm_store.as_ref().unwrap().intended_may_wire(k);

    assert!(!crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &ben.1, "trade", base + 2)), "a note is never a message");
    assert_eq!(may(&b, &ben.1), "trade", "it is applied");
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &ben.1, "call,trade", base + 1));
    assert_eq!(may(&b, &ben.1), "trade", "an older note changes nothing");
    let tie = note(&seed, &me, &ben.1, "message", base + 3);
    crate::engine::dm::ingest_dm(&mut b, &tie);
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &ben.1, "invite", base + 3));
    assert_eq!(may(&b, &ben.1), "message", "at an equal time the larger may wins");

    let p = SentPass { serial: "c1".repeat(16), may: "message".into() };
    b.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, p.clone());
    confirmed(&mut b, &p.serial); // my other device withdrew it: Ben is marked
    assert!(b.dm_store.as_ref().unwrap().changed_elsewhere(&ben.1));
    crate::engine::dm::ingest_dm(&mut b, &tie);
    assert!(b.dm_store.as_ref().unwrap().changed_elsewhere(&ben.1), "delivered again it changes nothing, not even the mark");
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &ben.1, "trade", base + 4));
    assert!(!b.dm_store.as_ref().unwrap().changed_elsewhere(&ben.1), "a new note about him clears the mark (N6)");

    let forged = parse_verify_inner(&build_signed_inner(&eve.0, &eve.1, &me, base + 9, &ChoiceNote::new(&ben.1, "call").text()).unwrap()).unwrap();
    assert!(!crate::engine::dm::ingest_dm(&mut b, &forged));
    let mine_to_eve = parse_verify_inner(&build_signed_inner(&seed, &me, &eve.1, base + 9, &ChoiceNote::new(&ben.1, "call").text()).unwrap()).unwrap();
    assert!(!crate::engine::dm::ingest_dm(&mut b, &mine_to_eve));
    let store = b.dm_store.as_ref().unwrap();
    assert_eq!(store.intended_may_wire(&ben.1), "trade", "a note from anyone else is dropped unread");
    assert!(store.conversation(&eve.1).is_empty() && store.requests().is_empty(), "not stored, and not a request");

    // Dated far ahead, so only the rule itself (not an older time) can keep them out.
    let later = 4_000_000_000_000u64;
    crate::engine::block::block(&mut b, &cy.1);
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &cy.1, "call", later));
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &me, "call", later));
    let bad = note(&seed, &me, &ben.1, "trade", base + 11);
    let bad = DmInner { text: format!("[[hum:choice:v1]]{} /trade", ben.1), ..bad };
    assert!(!crate::engine::dm::ingest_dm(&mut b, &bad), "a malformed note is never shown");
    let store = b.dm_store.as_ref().unwrap();
    // Cy's choice is the Block's clear (the empty may since 10o O6), which the note did not replace.
    assert_eq!((store.choice(&cy.1).map(|c| c.may.as_str()), store.choice(&me)), (Some(crate::net::dm_store::CLEARED), None), "one about someone I blocked, or naming me, changes nothing");
    assert!(store.conversation(&me).is_empty(), "and no note is ever stored as a message");
    tidy(&b);
}

/// N4. A CHOICE FROM MY OTHER DEVICE TAKES BACK AT ONCE, AND THE SWEEP GIVES A PASS CARRYING IT:
/// the note unticking Message for Ben withdraws his default pass here at once (and nothing is
/// minted on its arrival); the next sweep sends him a pass carrying exactly the note's choice,
/// and, while that one is on its way, no second. A pass this device has on its way to Cy when a
/// note takes Call away from her is withdrawn too, so its answer records nothing. Dee, a friend
/// nobody chose anything for, gets the defaults.
/// Seen red 2026-10-10 with `apply_in` not withdrawing the passes beyond the new choice: "Ben's
/// default pass is taken back at once" failed.
#[test]
fn a_choice_from_my_other_device_takes_back_at_once_and_the_sweep_carries_it() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(162);
    let (ben, cy, dee) = (identity(163), identity(164), identity(165));
    let (mut b, sent) = app(&seed, &me, "n4", &[ben.clone(), cy.clone(), dee.clone()]);
    let b0 = default_pass("d0");
    b.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, b0.clone());
    crate::engine::reach::set_tick(&mut b, &cy.1, ReachKind::Call, true);
    let to_cy = puts_to(&frames(&sent), &cy.1);
    let c1 = pass_in(&to_cy[0], &cy.0);
    let now = b.dm_store.as_ref().unwrap().choice(&cy.1).unwrap().at;

    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &ben.1, "trade", now + 1));
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &cy.1, &defaults(), now + 1));
    let out = frames(&sent);
    let store = b.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ben.1).is_empty() && store.pending_withdrawals().contains(&b0.serial), "Ben's default pass is taken back at once");
    assert!(store.pending_withdrawals().contains(&c1.serial), "and Cy's calling pass, still on its way");
    assert!(revoked(&out).contains(&b0.serial) && puts_to(&out, &ben.1).is_empty(), "the withdrawal goes now, and nothing is minted on the note's arrival");
    answer(&mut b, &to_cy[0], true, "");
    assert!(b.dm_store.as_ref().unwrap().passes_sent_to(&cy.1).is_empty(), "Cy's late answer records nothing");

    sweep(&mut b);
    let out = frames(&sent);
    assert_eq!(pass_in(&puts_to(&out, &ben.1)[0], &ben.0).may, "trade", "the sweep sends Ben a pass carrying the note's choice");
    assert_eq!(pass_in(&puts_to(&out, &cy.1)[0], &cy.0).may, defaults(), "and Cy one carrying hers");
    assert_eq!(pass_in(&puts_to(&out, &dee.1)[0], &dee.0).may, defaults(), "Dee, with no choice, gets the defaults");
    sweep(&mut b);
    assert!(puts_to(&frames(&sent), &ben.1).is_empty(), "no second while one is on its way");
    tidy(&b);
}

/// N7. THE SWEEP WAITS FOR THE MAILBOX: on a new connection, before its mailbox fetch was read,
/// nothing goes: not the note made offline, not a pass the sweep owes, not the pass a follow-back
/// arriving would complete, not one the pace would send. Once the last page was read (with my
/// other device's note about Ben in it), the note goes and the passes carry the choices it read.
/// Seen red 2026-10-10 two ways, both at "nothing before the mailbox was read": with the
/// `mailbox_read` check taken out of `sweep_friend_passes` (the note made offline went out), and
/// with it taken out of `mint_and_send_pass` (the follow-back's pass went out).
#[test]
fn the_sweep_waits_until_the_mailbox_was_read() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(166);
    let (ben, cy, dee) = (identity(167), identity(168), identity(169));
    let (mut b, sent) = app(&seed, &me, "n7", &[ben.clone(), dee.clone()]);
    b.peer_kyber_keys.insert(cy.1.clone(), kyber(&cy.0));
    b.dm_store.as_mut().unwrap().set_following(&cy.1, true);
    let link = b.ws_client.take();
    crate::engine::reach::set_tick(&mut b, &dee.1, ReachKind::Call, true); // made offline
    b.ws_client = link;
    b.dm_fetch.done = false;

    sweep(&mut b);
    let back = DmInner { from: cy.1.clone(), to: me.clone(), ts: 5, text: CTL_FOLLOW.into(), sig_b64: "cy-follows".into(), cert: None };
    crate::engine::dm::ingest_dm(&mut b, &back);
    b.pass_pacer.held = true;
    crate::engine::dm::pace_owed_passes(&mut b, Instant::now());
    assert!(frames(&sent).is_empty(), "nothing before the mailbox was read");

    let other = b.dm_store.as_ref().unwrap().choice(&dee.1).unwrap().at;
    crate::engine::dm::ingest_dm(&mut b, &note(&seed, &me, &ben.1, "call,trade", other));
    mailbox_read(&mut b);
    let out = frames(&sent);
    assert_eq!(order(&out, &me, &seed)[0], "note", "then the note made offline goes first");
    assert_eq!(pass_in(&puts_to(&out, &ben.1)[0], &ben.0).may, "call,trade", "and Ben's pass carries the choice the mailbox held");
    assert_eq!(puts_to(&out, &cy.1).len(), 1, "Cy, a friend now, gets hers");
    assert!(pass_in(&puts_to(&out, &dee.1)[0], &dee.0).may.contains("call"), "Dee's carries the choice made offline");
    tidy(&b);
}

/// N4 FOR MY OWN CONTACT REQUEST'S ECHO (the same echo rule as a pass, as the web chat does): my
/// other device asked Ann to connect, with a pass carrying the defaults. A device withdrawing that
/// very pass takes nothing from the echo, not even the follow (10m R5); one whose choice for Ann
/// allows less (trades only) follows her but withdraws the pass at once, never recording it; one
/// with no choice for her records it.
/// Seen red 2026-10-10 two ways: with the "being withdrawn" check taken out of
/// `contact_request_in`, "a device withdrawing it takes nothing back, not even the follow" failed;
/// and with its `grants_beyond` arm taken out, "one allowing more than the choice is withdrawn at
/// once" failed.
#[test]
fn my_own_contact_requests_echo_follows_the_echo_rule() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(182);
    let ann = identity(183);
    let (mut phone, _p) = app(&seed, &me, "req-phone", &[]);
    phone.peer_kyber_keys.insert(ann.1.clone(), kyber(&ann.0));
    let (_theirs, ours, sent) = crate::engine::reach::contact_request_puts(&phone, &ann.1).unwrap();
    let echo = open(&ours, &seed);
    let device = |tag: &str| app(&seed, &me, tag, &[]).0;

    let mut withdrawing = device("req-withdrawing");
    withdrawing.dm_store.as_mut().unwrap().withdraw_serial(&sent.serial);
    crate::engine::dm::ingest_dm(&mut withdrawing, &echo);
    let store = withdrawing.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ann.1).is_empty() && !store.is_following(&ann.1), "a device withdrawing it takes nothing back, not even the follow");

    let mut less = device("req-less");
    less.dm_store.as_mut().unwrap().apply_choice(&ann.1, "trade", 1_760_000_000_000);
    crate::engine::dm::ingest_dm(&mut less, &echo);
    let store = less.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ann.1).is_empty() && store.pending_withdrawals().contains(&sent.serial), "one allowing more than the choice is withdrawn at once");
    assert!(store.is_following(&ann.1), "asking is still following");

    let mut plain = device("req-plain");
    crate::engine::dm::ingest_dm(&mut plain, &echo);
    assert_eq!(plain.dm_store.as_ref().unwrap().passes_sent_to(&ann.1), [sent], "with no choice for her, it is recorded");
    for d in [&phone, &withdrawing, &less, &plain] {
        tidy(d);
    }
}

// ── The two-device sequences from the reviews ──────────────────────────────────────────────

/// SEQUENCE 1, AN UNTICK MADE OFFLINE ON ONE DEVICE, THE OTHER OPENED LATER: both devices hold
/// Ben's default pass P0. The desk, offline, unticks Message; on its next connection it sends the
/// note, withdraws P0 and gives P1 (trades only), which the server takes. The laptop, opened later,
/// reads its mailbox before sending anything: it shows the untick, records P1, takes back P0, and
/// gives Ben nothing back.
/// Seen red 2026-10-10 with `apply_in` returning before applying anything (notes ignored, the
/// pre-10n world where only echoes carried the choice): "the laptop shows the untick" failed (left
/// the defaults).
#[test]
fn sequence_an_untick_made_offline_and_the_other_device_opened_later() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(170);
    let ben = identity(171);
    let (mut desk, desk_sent) = app(&seed, &me, "s1-desk", &[ben.clone()]);
    let (mut laptop, laptop_sent) = app(&seed, &me, "s1-laptop", &[ben.clone()]);
    let p0 = default_pass("e0");
    for d in [&mut desk, &mut laptop] {
        d.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, p0.clone());
    }

    let link = desk.ws_client.take();
    crate::engine::reach::set_tick(&mut desk, &ben.1, ReachKind::Message, false);
    desk.ws_client = link;
    sweep(&mut desk);
    let out = frames(&desk_sent);
    assert_eq!(order(&out, &me, &seed), ["note", "withdraw", "theirs"]);
    let p1 = pass_in(&puts_to(&out, &ben.1)[0], &ben.0);
    answer(&mut desk, &puts_to(&out, &ben.1)[0], true, "");
    let mailbox: Vec<Value> = out.into_iter().chain(frames(&desk_sent)).collect();

    laptop.dm_fetch.done = false;
    deliver(&mut laptop, &mailbox, &me, &seed);
    mailbox_read(&mut laptop);
    let store = laptop.dm_store.as_ref().unwrap();
    assert_eq!(store.ticks(&ben.1), ticks(false, false, true), "the laptop shows the untick");
    assert_eq!(store.passes_sent_to(&ben.1), [p1], "records the new pass");
    assert!(store.pending_withdrawals().contains(&p0.serial), "takes back the old one");
    assert!(puts_to(&frames(&laptop_sent), &ben.1).is_empty(), "and gives Ben nothing back");
    tidy(&desk);
    tidy(&laptop);
}

/// SEQUENCE 2, AN UNTICK WHOSE NEW PASS IS REFUSED, THE OTHER DEVICE ONLINE AND THEN OFFLINE: the
/// desk unticks Message for Ben; the laptop, online, hears the note and takes back P0 at once. The
/// desk's P1 is refused: no self-copy goes, so no device records a pass the server refused. The
/// laptop's own sweep gives Ben a pass carrying the choice (trades only), never the defaults. The
/// laptop goes offline; the desk sends P1 again and it is taken; the laptop, back, records it and
/// owes nothing. Neither device ever gives back messages.
/// Seen red 2026-10-10 with `apply_in` not withdrawing the passes beyond the new choice: "the
/// laptop takes P0 back at once" failed.
#[test]
fn sequence_an_untick_whose_pass_is_refused_with_the_other_device_online_then_offline() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(172);
    let ben = identity(173);
    let (mut desk, desk_sent) = app(&seed, &me, "s2-desk", &[ben.clone()]);
    let (mut laptop, laptop_sent) = app(&seed, &me, "s2-laptop", &[ben.clone()]);
    let p0 = default_pass("f0");
    for d in [&mut desk, &mut laptop] {
        d.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, p0.clone());
    }

    crate::engine::reach::set_tick(&mut desk, &ben.1, ReachKind::Message, false);
    let out = frames(&desk_sent);
    deliver(&mut laptop, &out, &me, &seed); // live: the note
    let store = laptop.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ben.1).is_empty() && store.pending_withdrawals().contains(&p0.serial), "the laptop takes P0 back at once");
    assert_eq!(store.ticks(&ben.1), ticks(false, false, true));

    answer(&mut desk, &puts_to(&out, &ben.1)[0], false, "rate");
    assert!(frames(&desk_sent).is_empty(), "refused: no self-copy");
    sweep(&mut laptop);
    let from_laptop = puts_to(&frames(&laptop_sent), &ben.1);
    assert_eq!(pass_in(&from_laptop[0], &ben.0).may, "trade", "the laptop's pass carries the choice, never the defaults");

    let link = laptop.ws_client.take(); // offline
    sweep(&mut desk);
    let again = puts_to(&frames(&desk_sent), &ben.1);
    let p1 = pass_in(&again[0], &ben.0);
    assert_eq!(p1.may, "trade");
    answer(&mut desk, &again[0], true, "");
    let mailbox = frames(&desk_sent);
    laptop.ws_client = link;
    laptop.dm_fetch.done = false;
    laptop.pending_puts = Default::default(); // its own put's answer was lost while offline
    deliver(&mut laptop, &mailbox, &me, &seed);
    mailbox_read(&mut laptop);
    let store = laptop.dm_store.as_ref().unwrap();
    assert!(store.passes_sent_to(&ben.1).contains(&p1), "back online, the laptop records the desk's pass");
    assert_eq!(store.ticks(&ben.1), ticks(false, false, true), "and still has the untick");
    let after = frames(&laptop_sent);
    assert!(puts_to(&after, &ben.1).iter().all(|p| !pass_in(p, &ben.0).may.contains("message")), "never a pass allowing messages");
    tidy(&desk);
    tidy(&laptop);
}

/// SEQUENCE 3, AN OLDER ECHO READ AFTER A NEWER ONE: the desk ticks Call for Ben (P1, taken), then
/// unticks it (P1 withdrawn, P2 with the defaults, taken). The laptop reads its mailbox out of
/// order: the newer note, the older note, P2's echo, then P1's. It lands on the newer choice (Call
/// off), records P2, and withdraws P1 at once (it allows calls), without withdrawing P2.
/// Seen red 2026-10-10 with the echo arm back in its 10m form (the ticks set from the echo's
/// `may`, every other pass withdrawn): "the laptop keeps the newer choice" failed (Call ticked).
#[test]
fn sequence_an_older_echo_read_after_a_newer_one() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(174);
    let ben = identity(175);
    let (mut desk, desk_sent) = app(&seed, &me, "s3-desk", &[ben.clone()]);
    let (mut laptop, laptop_sent) = app(&seed, &me, "s3-laptop", &[ben.clone()]);
    let p0 = default_pass("90");
    for d in [&mut desk, &mut laptop] {
        d.dm_store.as_mut().unwrap().record_pass_sent(&ben.1, p0.clone());
    }

    let mut steps: Vec<Vec<Value>> = Vec::new();
    for on in [true, false] {
        crate::engine::reach::set_tick(&mut desk, &ben.1, ReachKind::Call, on);
        let out = frames(&desk_sent);
        answer(&mut desk, &puts_to(&out, &ben.1)[0], true, "");
        steps.push(out.into_iter().chain(frames(&desk_sent)).collect());
    }
    let pass_of = |frames: &[Value]| pass_in(&puts_to(frames, &ben.1)[0], &ben.0);
    let (p1, p2) = (pass_of(&steps[0]), pass_of(&steps[1]));
    assert!(p1.may.contains("call") && p2.may == defaults());
    let mine = |frames: &[Value]| puts_to(frames, &me);
    let (n1, e1) = (mine(&steps[0])[0].clone(), mine(&steps[0])[1].clone());
    let (n2, e2) = (mine(&steps[1])[0].clone(), mine(&steps[1])[1].clone());

    laptop.dm_fetch.done = false;
    deliver(&mut laptop, &[n2, n1, e2, e1], &me, &seed);
    mailbox_read(&mut laptop);
    let store = laptop.dm_store.as_ref().unwrap();
    assert_eq!(store.ticks(&ben.1), FriendTicks::default(), "the laptop keeps the newer choice");
    assert!(store.passes_sent_to(&ben.1).contains(&p2) && !store.passes_sent_to(&ben.1).contains(&p1), "records P2, never P1");
    assert!(store.pending_withdrawals().contains(&p1.serial), "and withdraws P1 at once");
    assert!(!store.pending_withdrawals().contains(&p2.serial), "without withdrawing P2");
    assert!(puts_to(&frames(&laptop_sent), &ben.1).is_empty(), "and owes Ben nothing");
    tidy(&desk);
    tidy(&laptop);
}

/// SEQUENCE 4, AN UNFOLLOW MADE OFFLINE (N3): the desk, offline, unfollows Ben: nothing is sent,
/// its pass is taken back and its choice cleared here, and the Unfollow waits, across a restart.
/// On the next connection both its copies go (to Ben, and to my own mailbox), signed with the time
/// of the Unfollow, before the withdrawal. The laptop, reading its mailbox, stops following Ben,
/// takes back its own record of his pass, clears its choice, and gives him nothing. Both devices
/// clear the choice as of the Unfollow's own signed time (as the web chat does), so a choice made
/// after it on a third device that had not heard of it still wins. An Unfollow still waiting when
/// the person blocks them (Cy) is dropped, never sent.
/// Seen red 2026-10-10 three ways: with `set_follow` not queueing an Unfollow it could not send
/// (the old behaviour), "the Unfollow waits" failed; with `flush` not asking `is_blocked`, "an
/// Unfollow waiting when they were blocked is never sent to them" failed; and with `set_follow`
/// clearing the choice dated a moment after the Unfollow's signed time, "cleared as of the
/// Unfollow's own signed time" failed.
#[test]
fn sequence_an_unfollow_made_offline() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(176);
    let (ben, cy) = (identity(177), identity(159));
    let (mut desk, desk_sent) = app(&seed, &me, "s4-desk", &[ben.clone(), cy.clone()]);
    desk.dm_store.as_mut().unwrap().record_pass_sent(&cy.1, default_pass("c4"));
    let (mut laptop, laptop_sent) = app(&seed, &me, "s4-laptop", &[ben.clone()]);
    let p0 = SentPass { serial: "70".repeat(16), may: "call,trade".into() };
    for d in [&mut desk, &mut laptop] {
        let store = d.dm_store.as_mut().unwrap();
        store.record_pass_sent(&ben.1, p0.clone());
        store.apply_choice(&ben.1, "call,trade", 1_760_000_000_000);
    }

    let link = desk.ws_client.take();
    crate::engine::dm::set_follow(&mut desk, &ben.1, false);
    assert!(frames(&desk_sent).is_empty(), "nothing goes while not connected");
    let store = desk.dm_store.as_ref().unwrap();
    let waiting = store.pending_unfollows().to_vec();
    assert_eq!(waiting.iter().map(|u| u.peer.as_str()).collect::<Vec<_>>(), [ben.1.as_str()], "the Unfollow waits");
    assert!(store.pending_withdrawals().contains(&p0.serial) && store.ticks(&ben.1) == FriendTicks::default(), "its pass taken back and its choice cleared here");
    assert_eq!(store.choice(&ben.1).map(|c| c.at), Some(waiting[0].at), "cleared as of the Unfollow's own signed time");
    store.save();
    desk.dm_store = Some(DmStore::load(&seed, &me, &crate::gui::pages::chat::norm_server_url(&desk.server_url)));
    assert_eq!(desk.dm_store.as_ref().unwrap().pending_unfollows(), waiting.as_slice(), "kept across a restart");

    desk.ws_client = link;
    sweep(&mut desk);
    let out = frames(&desk_sent);
    assert_eq!(order(&out, &me, &seed), ["theirs", "ours", "withdraw"], "both copies, then the withdrawal");
    let to_ben = open(&puts_to(&out, &ben.1)[0], &ben.0);
    assert_eq!((to_ben.text.as_str(), to_ben.ts), (CTL_UNFOLLOW, waiting[0].at), "signed with the time of the Unfollow");
    assert!(desk.dm_store.as_ref().unwrap().pending_unfollows().is_empty());

    laptop.dm_fetch.done = false;
    deliver(&mut laptop, &out, &me, &seed);
    mailbox_read(&mut laptop);
    let store = laptop.dm_store.as_ref().unwrap();
    assert!(!store.is_following(&ben.1), "the laptop stops following Ben");
    assert!(store.passes_sent_to(&ben.1).is_empty() && store.pending_withdrawals().contains(&p0.serial), "takes back his pass");
    assert_eq!(store.ticks(&ben.1), FriendTicks::default(), "clears its choice");
    assert_eq!(store.choice(&ben.1).map(|c| c.at), Some(waiting[0].at), "as of the Unfollow's own signed time");
    assert!(puts_to(&frames(&laptop_sent), &ben.1).is_empty(), "and gives him nothing");
    // A choice made after that Unfollow, on a third device that had not heard of it yet, still wins.
    crate::engine::dm::ingest_dm(&mut laptop, &note(&seed, &me, &ben.1, "trade", waiting[0].at + 1));
    assert_eq!(laptop.dm_store.as_ref().unwrap().intended_may_wire(&ben.1), "trade", "a choice made after the Unfollow still wins");

    // An Unfollow still waiting when the person blocks them is never sent: nothing ever goes to a
    // blocked person (10d), and the block's own note tells my other devices.
    let link = desk.ws_client.take();
    crate::engine::dm::set_follow(&mut desk, &cy.1, false);
    crate::engine::block::block(&mut desk, &cy.1);
    desk.ws_client = link;
    sweep(&mut desk);
    assert!(puts_to(&frames(&desk_sent), &cy.1).is_empty(), "an Unfollow waiting when they were blocked is never sent to them");
    assert!(desk.dm_store.as_ref().unwrap().pending_unfollows().is_empty(), "and does not wait any more");
    tidy(&desk);
    tidy(&laptop);
}

/// SEQUENCE 5, A TICK ON A MARKED FRIEND (N6): my other device withdrew the pass this laptop held
/// for Ben, so he is marked "changed on my other device". People I choose draws him with nothing
/// ticked, updating his pass. Ticking Trade gives exactly trades: the choice, its note and the new
/// pass all say "trade", and the mark is gone. Fay, marked the same way but no longer a mutual
/// follow, is not listed. Cy, marked, is freed by a note about him, and the next sweep gives him a
/// pass carrying the note's choice.
/// Seen red 2026-10-10 three ways: with `chosen_rows` drawing the stored ticks for a marked friend,
/// "drawn with nothing ticked" failed; with `set_tick` starting from the stored ticks, "exactly
/// trades" failed (left "invite,message,trade,voice_message"); and with `people_to_choose` listing
/// every marked person, "Fay is not listed" failed.
#[test]
fn sequence_a_tick_on_a_marked_friend() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(178);
    let (ben, cy, fay) = (identity(179), identity(180), identity(181));
    let (mut laptop, sent) = app(&seed, &me, "s5", &[ben.clone(), cy.clone()]);
    laptop.peer_kyber_keys.insert(fay.1.clone(), kyber(&fay.0));
    laptop.dm_store.as_mut().unwrap().set_following(&fay.1, true);
    for (who, s) in [(&ben, "b5"), (&cy, "c5"), (&fay, "f5")] {
        laptop.dm_store.as_mut().unwrap().record_pass_sent(&who.1, default_pass(s));
        confirmed(&mut laptop, &s.repeat(16));
    }
    assert!([&ben, &cy, &fay].iter().all(|w| laptop.dm_store.as_ref().unwrap().changed_elsewhere(&w.1)), "all three marked");

    let rows = crate::gui::pages::safety::chosen_rows(&laptop);
    let row = |k: &str| rows.iter().find(|(key, ..)| key == k).cloned();
    assert_eq!(row(&ben.1).map(|(_, _, t, updating)| (t, updating)), Some((FriendTicks::NONE, true)), "Ben is drawn with nothing ticked, updating his pass");
    assert!(row(&fay.1).is_none(), "Fay is not listed: marked and no longer a mutual follow");

    crate::engine::reach::set_tick(&mut laptop, &ben.1, ReachKind::Trade, true);
    let out = frames(&sent);
    let store = laptop.dm_store.as_ref().unwrap();
    assert_eq!(store.intended_may_wire(&ben.1), "trade", "a tick gives exactly trades");
    assert_eq!(notes_in(&out, &me, &seed).first().map(|(n, _)| n.may.clone()), Some("trade".to_string()), "and says so to my other devices");
    assert_eq!(pass_in(&puts_to(&out, &ben.1)[0], &ben.0).may, "trade", "in a pass allowing just that");
    assert!(!store.changed_elsewhere(&ben.1), "and the mark is gone");

    let at = store.choice(&ben.1).unwrap().at;
    crate::engine::dm::ingest_dm(&mut laptop, &note(&seed, &me, &cy.1, "call,trade", at));
    assert!(!laptop.dm_store.as_ref().unwrap().changed_elsewhere(&cy.1), "a note about Cy frees him");
    sweep(&mut laptop);
    assert_eq!(pass_in(&puts_to(&frames(&sent), &cy.1)[0], &cy.0).may, "call,trade", "and the sweep gives him the note's choice");
    tidy(&laptop);
}
