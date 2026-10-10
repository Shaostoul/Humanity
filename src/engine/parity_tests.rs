// Child of engine/put_answer_tests.rs (#[path]): section 10o of docs/design/blocking-and-safe-mode.md,
// "The parity review of 10n" (2026-10-10), the desktop's half, with the app state and a recording
// socket, no screen. Uses the parent's helpers (`app`, `frames`, `puts_to`, `notes_in`, `pass_in`,
// `revoked`, `sweep`, ...). Each test was seen failing once on purpose, recorded at the test. O1's
// ref rules alone are in net/mailbox_fetch.rs, O6 on the store in net/dm_store.rs, and the second
// half of O7 (the echo of my own contact request) is engine/choice_tests.rs
// `my_own_contact_requests_echo_follows_the_echo_rule`.

use super::*;
use crate::engine::bg_connections::{bg_begin_fetch, bg_dm_batch, bg_dm_new};
use crate::engine::mailbox::{begin_fetch, on_batch, on_new};
use crate::net::choice::ChoiceNote;
use crate::net::dm_pq::{build_signed_inner, parse_verify_inner, DmInner, CTL_FOLLOW, CTL_UNFOLLOW};
use crate::net::dm_store::CLEARED;
use crate::net::mailbox_fetch::MailboxFetch;

const BASE: u64 = 1_760_000_000_000;

/// A choice note as my other device signs it: from me, to me, at `at`.
fn note(seed: &[u8], me: &str, key: &str, may: &str, at: u64) -> DmInner {
    parse_verify_inner(&build_signed_inner(seed, me, me, at, &ChoiceNote::new(key, may).text()).unwrap()).unwrap()
}

/// A mailbox row as the relay pages it: its id, and a choice note sealed to my own key.
fn note_row(id: i64, seed: &[u8], me: &str, key: &str, may: &str, at: u64) -> Value {
    let inner = build_signed_inner(seed, me, me, at, &ChoiceNote::new(key, may).text()).unwrap();
    json!({ "id": id, "content": crate::net::dm_pq::seal_v2(&kyber(seed), &inner).unwrap() })
}

/// A `dm_batch` page carrying `reference`.
fn page(reference: &str, rows: &[Value], done: bool) -> Value {
    json!({ "type": "dm_batch", "ref": reference, "messages": rows, "done": done })
}

/// The same row delivered live, as `dm_new`.
fn live(row: &Value) -> Value {
    json!({ "type": "dm_new", "id": row["id"], "content": row["content"] })
}

/// The `dm_fetch`es among `frames`: (after_id, ref).
fn fetches(frames: &[Value]) -> Vec<(i64, String)> {
    frames.iter().filter(|f| f["type"] == "dm_fetch").map(|f| (f["after_id"].as_i64().unwrap(), f["ref"].as_str().expect("a fetch carries a ref").to_string())).collect()
}

fn may_in(store: &DmStore, key: &str) -> Option<String> {
    store.choice(key).map(|c| c.may.clone())
}

/// What each frame was: "note" (a choice note to my own mailbox), "ours" (another put to my own
/// mailbox), "theirs", "withdraw", or its type.
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

// ── O1. A device's own mailbox pages ────────────────────────────────────────────────────────

/// O1, THE REVIEW'S SEQUENCE, on the server we are on: two devices of one person on one server,
/// both fetching their mailbox (a backlog over one page). Device B's two pages reach device A
/// first, carrying B's refs: A takes in no row, does not move its read position, does not count
/// its mailbox read, and sends nothing (no page asked for, no sweep, no pass). A live note arrives
/// during A's fetch: it is handled, but A's read position does not move. A's own first page moves
/// the position to its last id, and A asks for the next page from there with a fresh ref; so the
/// row past the first page (my choice for Ben) is not skipped, and only on A's own last page does
/// the sweep run, giving Ben a pass carrying that choice.
/// Seen red 2026-10-10 two ways: with `MailboxFetch::page` taking any page as ours (its ref
/// comparison taken out), "B's pages change nothing at A" failed (Dee's and Ben's choices taken in
/// from B's pages); and with `on_new` moving the read position during the fetch (`moves` forced
/// true, the old rule, under which the next page was then asked for from 12 and row 8 skipped),
/// "a live note does not move A's read position" failed (left 12).
#[test]
fn a_device_counts_only_its_own_mailbox_pages() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(210);
    let (ben, cy, dee) = (identity(211), identity(212), identity(213));
    let (mut a, a_sent) = app(&seed, &me, "o1-a", &[ben.clone()]);
    let (mut b, b_sent) = app(&seed, &me, "o1-b", &[ben.clone()]);
    a.dm_fetch = MailboxFetch::default(); // new connections: nothing fetched yet
    b.dm_fetch = MailboxFetch::default();
    let rows = [
        note_row(3, &seed, &me, &dee.1, "call,trade", BASE + 1),
        note_row(5, &seed, &me, &cy.1, "trade", BASE + 2),
        note_row(8, &seed, &me, &ben.1, "trade", BASE + 3), // past the first page
        note_row(12, &seed, &me, &cy.1, "message", BASE + 4), // also delivered live during the fetch
    ];
    let water = |d: &GuiState| d.dm_store.as_ref().unwrap().high_water();
    let may = |d: &GuiState, k: &str| may_in(d.dm_store.as_ref().unwrap(), k);

    assert!(begin_fetch(&mut a) && begin_fetch(&mut b));
    let a_fetch = fetches(&frames(&a_sent));
    let b_fetch = fetches(&frames(&b_sent));
    assert_eq!(a_fetch.len(), 1);
    assert_eq!(a_fetch[0].0, 0, "A fetches from its read position");
    assert_ne!(a_fetch[0].1, b_fetch[0].1, "each device's fetch has its own ref");

    // B's pages, which every device of mine receives, reach A before A's own.
    let b1 = page(&b_fetch[0].1, &rows[..2], false);
    on_batch(&mut b, &b1);
    let b2 = page(&fetches(&frames(&b_sent))[0].1, &rows[2..], true);
    on_batch(&mut a, &b1);
    on_batch(&mut a, &b2);
    assert_eq!((may(&a, &dee.1), may(&a, &ben.1)), (None, None), "B's pages change nothing at A: not one row taken in");
    assert_eq!(water(&a), 0, "nor its read position");
    assert!(!crate::engine::dm::mailbox_read(&a), "A does not count its mailbox read on B's last page");
    assert!(frames(&a_sent).is_empty(), "and sends nothing: no page asked for, no sweep, no pass to Ben");

    on_new(&mut a, &live(&rows[3]));
    assert_eq!(may(&a, &cy.1).as_deref(), Some("message"), "a live note during A's fetch is handled");
    assert_eq!(water(&a), 0, "a live note does not move A's read position while its fetch is unfinished");

    on_batch(&mut a, &page(&a_fetch[0].1, &rows[..2], false));
    assert_eq!(may(&a, &dee.1).as_deref(), Some("call,trade"), "A's own page is taken in");
    assert_eq!(water(&a), 5, "and moves the read position to its last id");
    let next = fetches(&frames(&a_sent));
    assert_eq!(next.len(), 1, "the next page is asked for");
    assert_eq!(next[0].0, 5, "from the last id of A's own page, never past a row A has not read");
    assert_ne!(next[0].1, a_fetch[0].1, "with a fresh ref");
    assert!(!crate::engine::dm::mailbox_read(&a), "not read before its last page");

    a.pass_pacer = Default::default();
    on_batch(&mut a, &page(&next[0].1, &rows[2..], true));
    assert_eq!(may(&a, &ben.1).as_deref(), Some("trade"), "no row was skipped: Ben's choice is read");
    assert_eq!(may(&a, &cy.1).as_deref(), Some("message"), "and the live note, fetched again, is applied once");
    assert_eq!(water(&a), 12);
    assert!(crate::engine::dm::mailbox_read(&a), "A's own last page counts its mailbox read");
    assert_eq!(pass_in(&puts_to(&frames(&a_sent), &ben.1)[0], &ben.0).may, "trade", "and only now the sweep gives Ben a pass, carrying the choice it read");
    tidy(&a);
    tidy(&b);
}

/// O1 ON A PARKED SERVER (`ServerConnection.mailbox`, engine/bg_connections.rs): the same rules.
/// Another device's pages change nothing in that server's store and do not count its mailbox
/// read; a live note during its fetch is applied without moving the read position; its own pages
/// page from their own last id, and its own last page counts it read.
/// Seen red 2026-10-10 two ways: with `MailboxFetch::page`'s ref comparison taken out, "another
/// device's pages change nothing there" failed (left Dee's choice and 12); and with `bg_dm_new`
/// moving the read position during the fetch (`moves` forced true), "a live note does not move
/// its read position" failed (left 12).
#[test]
fn a_parked_server_counts_only_its_own_mailbox_pages() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(214);
    let (ben, dee) = (identity(215), identity(216));
    let (mut gs, _active) = app(&seed, &me, "o1-parked-active", &[]);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let url = crate::gui::pages::chat::norm_server_url(&format!("https://o1-parked-{nanos}.example"));
    let (ws, sent) = crate::net::ws_client::WsClient::recording();
    gs.connections.push(crate::gui::ServerConnection { url: url.clone(), display_url: url.clone(), ws: Some(ws), ..Default::default() });
    let store = || DmStore::load(&seed, &me, &url);
    let rows = [
        note_row(3, &seed, &me, &dee.1, "call,trade", BASE + 1),
        note_row(5, &seed, &me, &ben.1, "call", BASE + 2),
        note_row(8, &seed, &me, &ben.1, "trade", BASE + 3),
        note_row(12, &seed, &me, &dee.1, "message", BASE + 4),
    ];

    bg_begin_fetch(&mut gs, 0);
    let first = fetches(&frames(&sent));
    assert_eq!(first.iter().map(|f| f.0).collect::<Vec<_>>(), [0]);
    bg_dm_batch(&mut gs, 0, &page("another-device-1", &rows[..2], false));
    bg_dm_batch(&mut gs, 0, &page("another-device-2", &rows[2..], true));
    assert_eq!((may_in(&store(), &dee.1), store().high_water()), (None, 0), "another device's pages change nothing there");
    assert!(!gs.connections[0].mailbox.is_read() && frames(&sent).is_empty(), "nor count its mailbox read, nor ask for a page");

    bg_dm_new(&mut gs, 0, &live(&rows[3]));
    assert_eq!(may_in(&store(), &dee.1).as_deref(), Some("message"), "a live note during its fetch is applied");
    assert_eq!(store().high_water(), 0, "a live note does not move its read position");

    bg_dm_batch(&mut gs, 0, &page(&first[0].1, &rows[..2], false));
    assert_eq!(store().high_water(), 5);
    let next = fetches(&frames(&sent));
    assert_eq!(next[0].0, 5, "its next page from the last id of its own page");
    bg_dm_batch(&mut gs, 0, &page(&next[0].1, &rows[2..], true));
    let parked = store();
    assert_eq!((may_in(&parked, &ben.1).as_deref(), may_in(&parked, &dee.1).as_deref()), (Some("trade"), Some("message")), "no row skipped, none applied twice");
    assert_eq!(parked.high_water(), 12);
    assert!(gs.connections[0].mailbox.is_read(), "its own last page counts it read");
    parked.remove_file_for_test();
    tidy(&gs);
}

// ── O3. A queued Unfollow goes only while it still holds ────────────────────────────────────

/// O3 (the desktop's rule, kept): an Unfollow made with no server connected waits, and is dropped
/// rather than sent when, by the time it would go, I follow them again or have blocked them. It is
/// cleared at once by following again here (Ben), by the echo of my own follow from my other device
/// (Cy) and by Block (Dee); and the flush checks again as it goes (Eve, followed again by a path
/// that left her Unfollow waiting). On the next connection no Unfollow goes to anyone.
/// Seen red 2026-10-10 two ways: with `enforce_on_store` not dropping a waiting Unfollow, "cleared
/// by Block" failed; and with `flush` not asking `is_following`, "no Unfollow goes to anyone"
/// failed (Eve's went).
#[test]
fn a_queued_unfollow_goes_only_while_it_still_holds() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(217);
    let (ben, cy, dee, eve) = (identity(218), identity(219), identity(220), identity(221));
    let (mut a, sent) = app(&seed, &me, "o3", &[ben.clone(), cy.clone(), dee.clone(), eve.clone()]);
    let link = a.ws_client.take();
    for k in [&ben.1, &cy.1, &dee.1, &eve.1] {
        crate::engine::dm::set_follow(&mut a, k, false);
    }
    let waiting = |a: &GuiState| a.dm_store.as_ref().unwrap().pending_unfollows().iter().map(|u| u.peer.clone()).collect::<Vec<_>>();
    assert_eq!(waiting(&a).len(), 4, "four Unfollows wait");

    crate::engine::dm::set_follow(&mut a, &ben.1, true);
    assert!(!waiting(&a).contains(&ben.1), "cleared by following again here");
    let echo = DmInner { from: me.clone(), to: cy.1.clone(), ts: BASE, text: CTL_FOLLOW.into(), sig_b64: "my-follow-of-cy".into(), cert: None };
    crate::engine::dm::ingest_dm(&mut a, &echo);
    assert!(!waiting(&a).contains(&cy.1), "cleared by the echo of my own follow");
    crate::engine::block::block(&mut a, &dee.1);
    assert!(!waiting(&a).contains(&dee.1), "cleared by Block");
    a.dm_store.as_mut().unwrap().set_following(&eve.1, true);
    assert_eq!(waiting(&a), [eve.1.clone()]);

    a.ws_client = link;
    sweep(&mut a);
    let out = frames(&sent);
    for who in [&ben, &cy, &dee, &eve] {
        assert!(puts_to(&out, &who.1).iter().all(|p| open(p, &who.0).text != CTL_UNFOLLOW), "no Unfollow goes to anyone");
    }
    assert!(waiting(&a).is_empty(), "and none waits any more");
    tidy(&a);
}

// ── O4. Block drops a queued choice note ────────────────────────────────────────────────────

/// O4: a choice note waiting to go out is dropped when Block clears the choice (the web's rule).
/// The desk, offline, unticks Message for Ben and ticks Call for Cy: two notes wait. On the next
/// connection its mailbox holds my other device's block of Ben: reading it clears Ben's choice and
/// his waiting note, so no note about Ben ever goes (sent after the Block, it gave my other
/// devices a choice this one had cleared). Cy's note still goes.
/// Seen red 2026-10-10 with `clear_ticks` leaving the waiting note in place: "no note about Ben
/// ever goes" failed.
#[test]
fn block_drops_a_queued_choice_note() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(222);
    let (ben, cy) = (identity(223), identity(224));
    let (mut desk, sent) = app(&seed, &me, "o4", &[ben.clone(), cy.clone()]);
    let link = desk.ws_client.take();
    crate::engine::reach::set_tick(&mut desk, &ben.1, ReachKind::Message, false);
    crate::engine::reach::set_tick(&mut desk, &cy.1, ReachKind::Call, true);
    assert_eq!(desk.dm_store.as_ref().unwrap().pending_choices().len(), 2, "two notes wait");

    desk.ws_client = link;
    desk.dm_fetch.done = false; // a new connection, its mailbox not read yet
    let block_note = crate::net::block_list::BlockNote::Block(ben.1.clone()).text();
    let blocked = parse_verify_inner(&build_signed_inner(&seed, &me, &me, BASE, &block_note).unwrap()).unwrap();
    crate::engine::dm::ingest_dm(&mut desk, &blocked);
    let store = desk.dm_store.as_ref().unwrap();
    assert_eq!(store.pending_choices().iter().map(|p| p.peer.clone()).collect::<Vec<_>>(), Vec::<String>::new(), "Ben's note is dropped (Cy's has gone already, O5)");
    assert_eq!(may_in(store, &ben.1).as_deref(), Some(CLEARED), "his choice is cleared");
    desk.pass_pacer = Default::default();
    crate::engine::dm::on_mailbox_read(&mut desk);
    let notes: Vec<String> = notes_in(&frames(&sent), &me, &seed).into_iter().map(|(n, _)| n.key).collect();
    assert!(!notes.contains(&ben.1), "no note about Ben ever goes");
    assert_eq!(notes, [cy.1.clone()], "Cy's note still goes");
    tidy(&desk);
}

// ── O5. Waiting choice notes go before any withdrawal ───────────────────────────────────────

/// O5: every path that sends withdrawals sends the waiting choice notes first (the web's rule),
/// so my other devices hear the new choice before they hear an old pass was withdrawn. The desk,
/// offline, unticks Message for Ben (his default pass is taken back at once, and the note waits).
/// On a new connection, before its mailbox was read: blocking Cy sends Ben's note BEFORE the
/// withdrawals. Offline again, another untick (Cy is gone; Dee loses Trade), then a note from my
/// other device about Fay arrives on the new connection, withdrawing her pass: again the waiting
/// note goes first.
/// Seen red 2026-10-10 with `send_pending_withdrawals` not flushing the notes: "Ben's note goes
/// before the withdrawals" failed (left ["withdraw", "withdraw", "ours"]).
#[test]
fn waiting_choice_notes_go_before_any_withdrawal() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(225);
    let (ben, cy, dee, fay) = (identity(226), identity(227), identity(228), identity(229));
    let (mut desk, sent) = app(&seed, &me, "o5", &[ben.clone(), cy.clone(), dee.clone(), fay.clone()]);
    let default_may = crate::net::reach::intended_may_wire(FriendTicks::default());
    for (who, s) in [(&ben, "b5"), (&cy, "c5"), (&dee, "d5"), (&fay, "f5")] {
        desk.dm_store.as_mut().unwrap().record_pass_sent(&who.1, SentPass { serial: s.repeat(16), may: default_may.clone() });
    }

    let link = desk.ws_client.take();
    crate::engine::reach::set_tick(&mut desk, &ben.1, ReachKind::Message, false);
    desk.ws_client = link;
    desk.dm_fetch.done = false; // a new connection, its mailbox not read yet
    crate::engine::block::block(&mut desk, &cy.1);
    let out = order(&frames(&sent), &me, &seed);
    assert_eq!(out, ["note", "withdraw", "withdraw", "ours"], "Ben's note goes before the withdrawals (then the block's own note)");

    let link = desk.ws_client.take();
    crate::engine::reach::set_tick(&mut desk, &dee.1, ReachKind::Trade, false);
    desk.ws_client = link;
    crate::engine::dm::ingest_dm(&mut desk, &note(&seed, &me, &fay.1, "trade", BASE));
    let out = frames(&sent);
    let kinds = order(&out, &me, &seed);
    // The withdrawals not yet confirmed by the relay (Ben's and Cy's, with Dee's and Fay's) all go.
    assert!(kinds[0] == "note" && kinds.len() == 5 && kinds[1..].iter().all(|k| k == "withdraw"), "on a note's arrival too, the waiting note goes first: {kinds:?}");
    assert_eq!(notes_in(&out, &me, &seed)[0].0.key, dee.1, "Dee's");
    assert!(revoked(&out).contains(&"f5".repeat(16)), "and Fay's pass is withdrawn after it");
    tidy(&desk);
}

// ── O7. A note about someone blocked is not remembered as seen ──────────────────────────────

/// O7 (the desktop's order, kept): a choice note about someone I blocked is ignored and NOT
/// remembered as seen, so after Unblock the same note, delivered again, applies.
/// Seen red 2026-10-10 with `apply_in` asking `first_sight_note` before the blocked check: "after
/// Unblock the same note, delivered again, applies" failed (the choice stayed cleared).
#[test]
fn a_note_about_someone_blocked_is_not_remembered_as_seen() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(209);
    let cy = identity(208);
    let (mut a, _sent) = app(&seed, &me, "o7", &[]);
    crate::engine::block::block(&mut a, &cy.1);
    let later = note(&seed, &me, &cy.1, "call,trade", 4_000_000_000_000);
    crate::engine::dm::ingest_dm(&mut a, &later);
    assert_eq!(may_in(a.dm_store.as_ref().unwrap(), &cy.1).as_deref(), Some(CLEARED), "ignored while blocked");
    crate::engine::block::unblock(&mut a, &cy.1);
    crate::engine::dm::ingest_dm(&mut a, &later);
    assert_eq!(may_in(a.dm_store.as_ref().unwrap(), &cy.1).as_deref(), Some("call,trade"), "after Unblock the same note, delivered again, applies");
    tidy(&a);
}
