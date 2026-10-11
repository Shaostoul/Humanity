//! Tests of the desktop's round 4 (docs/design/media-showcase.md, "Round 4: fixes from the round 3
//! review"): D4-1 to D4-4, against the server simulated in memory (resumable_tests.rs, which
//! answers a cancel as the round 4 spec states it) and, where the page's own HTTP request is the
//! thing tested, a server on this machine's loopback address (nothing leaves the machine). Each
//! was seen to fail once against a deliberate break of the code it guards; what it saw red is in
//! its comment.

use super::resumable::tests::{done_answer, file, run_with, spawn_work, until, wait_for, Fake, Trick, TOKEN, WINDOW};
use super::resumable::{Control, Event, Phase, Reply, Unfinished};
use super::tests::{ctx_and_theme, demo_answer, page_state, settled, shows, SERVER};
use super::upload_cancel::{self, Told};
use super::uploading::{self, After};
use super::uploads_tests::{http_server, picked, until as poll_until};
use super::*;
use std::time::{Duration, Instant};

/// A 409 busy, as the relay answers a piece or a cancel while it is writing.
fn busy() -> Reply {
    Reply { status: 409, body: serde_json::json!({ "error": "Another piece of this upload is being written.", "busy": true }).to_string() }
}

/// An allowance an hour away for every piece, so an upload sits started and waiting.
fn waits_an_hour(fake: &Fake) {
    fake.s().allowance_forever = Some((upload_waits::now_ms() + 3_600_000, Some(3600.0)));
}

/// THE EMPTY PIECE NEVER READS THE FILE (D4-1): a 507 at the finish keeps every piece on the
/// server, the file on this computer is then deleted, and Retry still finishes the upload from
/// the server's copy. Nothing is cancelled on the server.
///
/// Seen red 2026-10-11 with the empty piece read from the file again (round 3's `read_piece` for
/// every piece): "Retry finishes from the server's copy: [..., NoRoom { .. }, Sending, Failed {
/// words: \"The file could not be read any more, so the upload stopped (...). Nothing of it stays
/// on the server.\", gone: true }]", and the server's whole copy was cancelled.
#[test]
fn the_empty_piece_never_reads_the_file() {
    let fake = Fake::new(8192);
    fake.s().finish_trick = Some(Trick::NoRoom);
    let (f, plan, bytes) = file(20_000);
    let events = run_with(fake.clone(), plan, TOKEN, |ctl, e| {
        if matches!(e, Event::NoRoom { .. }) {
            std::fs::remove_file(&f).expect("the local file goes");
            ctl.retry();
        }
    });
    assert!(done_answer(&events).is_some(), "Retry finishes from the server's copy: {events:?}");
    let s = fake.s();
    assert!(!s.log.contains(&"cancel".to_string()), "nothing is cancelled on the server: {:?}", s.log);
    assert_eq!(s.data, bytes, "the server's copy is the file");
}

/// CARRY ON FINISHES FROM THE SERVER'S COPY, WHATEVER THE LOCAL FILE (D4-1): an upload a restart
/// cut off after every piece arrived (a 507 kept them, or the finish's answer was lost) is carried
/// on although its file is gone from this computer: it asks where the server got, sends the empty
/// piece that finishes it, and is up, never cancelled.
///
/// Seen red 2026-10-11 with round 3's `resume_with` (a file not unchanged was discarded at once):
/// "it is carried on" (the uploads list was empty and the status read "The file changed since
/// its upload started, ...").
#[test]
fn carry_on_finishes_from_the_servers_copy_whatever_the_local_file() {
    let ctx = egui::Context::default();
    let mut state = page_state(demo_answer(), true);
    state.upload_token = Some((SERVER.to_string(), TOKEN.to_string()));
    let path = crate::test_temp::file("media_r4_gone", "webm");
    let bytes: Vec<u8> = (0..20_000u32).map(|i| (i % 241) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let id = "00112233445566778899aabbccddeeff";
    let record = Unfinished {
        server: state.media.server.clone(),
        key: "me".into(),
        id: id.into(),
        path: path.to_path_buf(),
        name: "clip.webm".into(),
        size: 20_000,
        modified_ms: resumable::modified_ms(&path),
        title: "Dusk".into(),
        ..Default::default()
    };
    std::fs::remove_file(&path).unwrap(); // gone from this computer since
    state.media.unfinished = vec![record];
    let fake = Fake::new(8192);
    {
        let mut s = fake.s();
        s.id = Some(id.into());
        s.size = 20_000;
        s.blake3 = blake3::hash(&bytes).to_hex().to_string();
        s.data = bytes.clone(); // every piece is on the server
    }
    uploading::resume_with(&mut state, id, fake.clone());
    assert!(!state.media.uploads.is_empty(), "it is carried on: {}", state.media.status);
    poll_until(&ctx, &mut state, "it is up", |s| s.media.uploads.iter().all(|e| !matches!(e.after, After::Uploading)));
    let s = fake.s();
    assert!(s.done.is_some(), "finished from the server's copy: {:?}", s.log);
    assert!(!s.log.contains(&"cancel".to_string()), "never cancelled: {:?}", s.log);
    assert_eq!(s.log.iter().filter(|l| l.starts_with("piece")).cloned().collect::<Vec<_>>(), vec!["piece 20000+0".to_string()], "only the empty piece");
}

/// A CANCEL WHILE THE SERVER STORES THE FINISH IS ASKED AGAIN, AND REMOVES THE STORED FILE (D4-2):
/// the finish's answer is lost and the connection keeps dropping, so the status that would carry
/// the finish's `done` is never answered; Cancel then asks the server, which is still storing and
/// answers 409 busy twice. The cancel is asked again until the server is done, and its answer
/// (`removed_stored`) removes the finished file too: the upload ends Cancelled, confirmed.
///
/// Seen red 2026-10-11 with round 3's cancel (one request, any answer taken as done): "asked again
/// while the server was busy" (left: 1 / right: 3), and nothing was removed.
#[test]
fn a_cancel_while_the_server_stores_the_finish_is_asked_again() {
    let fake = Fake::new(8192);
    {
        let mut s = fake.s();
        s.finish_trick = Some(Trick::LoseFinish);
        s.status_lost = usize::MAX;
        s.cancel_answers = [busy(), busy()].into();
    }
    let (_f, plan, _) = file(20_000);
    let (ctl, rx, handle) = spawn_work(&fake, plan, Control::with_pace(Some(TOKEN.into()), Duration::from_millis(1)));
    let deadline = Instant::now() + Duration::from_secs(10);
    while fake.count("status") < 2 {
        assert!(Instant::now() < deadline, "the finish went and its answer was lost: {:?}", fake.s().log);
        std::thread::sleep(Duration::from_millis(1));
    }
    ctl.cancel();
    let end = until(&rx, |e| matches!(e, Event::Cancelled { .. })).pop();
    handle.join().unwrap();
    assert_eq!(end, Some(Event::Cancelled { told: Told::Gone }), "confirmed by the server");
    assert_eq!(fake.count("cancel"), 3, "asked again while the server was busy (left: asks / right: 3)");
    assert!(fake.s().stored_removed, "and the file it stored is removed with the upload");
}

/// A CANCEL THE SERVER DID NOT CONFIRM IS SAID PLAINLY, AND THE UPLOAD STAYS LISTED (D4-2): the
/// server refuses the cancel; the page does not say "Cancelled", says why, and keeps the record,
/// offered with Carry on and Cancel it.
///
/// Seen red 2026-10-11 with every answer taken as done (round 3): "said plainly, not Cancelled"
/// (left: "Cancelled: nothing of it stays on the server.").
#[test]
fn a_cancel_the_server_did_not_confirm_is_said_and_kept() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    state.upload_token = Some((SERVER.to_string(), TOKEN.to_string()));
    let fake = Fake::new(8192);
    waits_an_hour(&fake);
    let (_f, form) = picked("r4_refused", 20_000, "Refused");
    uploading::start_with(&mut state, &form, fake.clone()).expect("it starts");
    let local = state.media.uploads[0].local;
    poll_until(&ctx, &mut state, "it starts and waits", |s| s.media.uploads.iter().all(|e| matches!(e.run.phase, Phase::Waiting { .. }) && !e.record.id.is_empty()));
    fake.s().cancel_forever = Some(Reply { status: 403, body: serde_json::json!({ "error": "Your role may no longer upload on this server." }).to_string() });
    uploading::cancel(&mut state, local);
    poll_until(&ctx, &mut state, "the upload stops", |s| s.media.uploads.is_empty());
    assert_eq!(state.media.status, upload_cancel::not_cancelled_words("Your role may no longer upload on this server."), "said plainly, not Cancelled");
    assert_eq!(uploading::offered(&state).len(), 1, "its record stays");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "An upload did not finish: r4_refused.webm") && shows(&out, "Cancel it"), "listed with its Cancel");
}

/// CANCELLING AN UPLOAD THAT IS UP NEEDS NO SIGNATURE, AND IS SAID DONE ONLY WHEN THE SERVER SAYS
/// SO (D4-2): with the identity locked the upload stays up but not on Media; its Cancel asks the
/// server with the upload token (`DELETE`, which removes the stored file with the session), the
/// row says it is being cancelled meanwhile, and "Cancelled" is said once the server confirmed.
///
/// Seen red 2026-10-11 with round 3's signed take-off (no signature: nothing sent, "Cancelled"
/// said at once): "nothing is said done before the server answers" (left: "Cancelled: nothing of
/// it stays on the server.").
#[test]
fn cancelling_an_upload_that_is_up_needs_no_signature() {
    let ctx = egui::Context::default();
    let mut state = page_state(demo_answer(), true);
    state.upload_token = Some((SERVER.to_string(), TOKEN.to_string()));
    assert!(state.private_key_bytes.is_none(), "no identity unlocked: no signature");
    let fake = Fake::new(8192);
    let (_f, form) = picked("r4_up", 10_000, "Up");
    uploading::start_with(&mut state, &form, fake.clone()).expect("it starts");
    poll_until(&ctx, &mut state, "it is up, not on Media", |s| s.media.uploads.iter().all(|e| matches!(e.after, After::NotPublished { .. })));
    let local = state.media.uploads[0].local;
    uploading::cancel(&mut state, local);
    assert_ne!(state.media.status, uploading::CANCELLED_LINE, "nothing is said done before the server answers");
    assert!(state.media.uploads[0].cancelling, "the row says it is being cancelled");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.media.uploads.is_empty() {
        poll(&ctx, &mut state);
        assert!(Instant::now() < deadline, "the server answers: {:?}", fake.s().log);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(fake.s().stored_removed, "the server removed the stored file");
    assert_eq!(state.media.status, uploading::CANCELLED_LINE);
    assert!(state.media.unfinished.is_empty(), "and the record goes");
}

/// AN UNFINISHED UPLOAD THE SERVER LISTS STAYS LISTED UNTIL THE SERVER CONFIRMS ITS CANCEL (D4-2):
/// Cancel on "Your unfinished uploads on this server" asks the server (the page's own `DELETE`,
/// here to a loopback server that answers 409 busy first); the row stays, saying it is being
/// cancelled, while the server is busy, and goes once it answered `ok`.
///
/// Seen red 2026-10-11 with round 3's row taken off at the press: "the row stays while the server
/// is busy" (no shape held "Holiday.mp4").
#[test]
fn an_unfinished_upload_stays_listed_until_the_server_confirms_its_cancel() {
    let (ctx, theme) = ctx_and_theme();
    let (base, seen) = http_server(vec![(Duration::ZERO, 409, r#"{"error":"Another piece of this upload is being written.","busy":true}"#), (Duration::ZERO, 200, r#"{"ok":true}"#)]);
    let mut state = page_state(demo_answer(), true);
    state.server_url = base.clone();
    sync_server(&mut state);
    state.media.requested = true;
    state.upload_token = Some((base, TOKEN.to_string()));
    let mine = serde_json::json!({ "used_bytes": 0, "share_bytes": 1 << 30, "waiting": [],
        "uploads_in_progress": [{ "id": "s1", "filename": "Holiday.mp4", "size": 4 << 20, "offset": 1 << 20, "touched_at": 1 }] });
    state.media.mine = Some(api::parse_mine(&mine));
    let out = settled(&ctx, &theme, &mut state);
    let row = crate::gui::screen_surface::find_text_in_shapes(&out.shapes, "Holiday.mp4").expect("listed").rect;
    let cancel = out.shapes.iter().find_map(|cs| match &cs.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == "Cancel" && (t.pos.y + t.galley.size().y / 2.0 - row.center().y).abs() < 12.0 => Some(t.pos + t.galley.size() / 2.0),
        _ => None,
    });
    let pos = cancel.expect("a Cancel beside it");
    let m = egui::Modifiers::default();
    let input = |events| egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), events, ..Default::default() };
    for events in [vec![egui::Event::PointerMoved(pos)], vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]] {
        let _ = ctx.run(input(events), |ctx| draw(ctx, &theme, &mut state));
    }
    // The server answered busy; the page waits a moment before asking again.
    let deadline = Instant::now() + Duration::from_secs(10);
    while seen.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the cancel reaches the server");
        std::thread::sleep(Duration::from_millis(5));
    }
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Holiday.mp4"), "the row stays while the server is busy");
    assert!(shows(&out, "Cancelling..."), "saying it is being cancelled");
    while state.media.jobs.iter().any(|(j, _)| matches!(j, Job::Discard { .. })) {
        poll(&ctx, &mut state);
        assert!(Instant::now() < deadline, "the server confirms: {:?}", seen.lock().unwrap());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(seen.lock().unwrap().iter().filter(|r| r.starts_with("DELETE /api/upload/resumable/s1")).count(), 2, "asked again after the busy answer");
    assert_eq!(state.media.status, uploading::CANCELLED_LINE);
    assert!(!shows(&settled(&ctx, &theme, &mut state), "Holiday.mp4"), "and the row goes once the server confirmed");
}

/// The page on a loopback server at `base`, signed in with an identity unlocked (a signature to
/// publish with), and an upload of `tag` through the simulated server gone to be published.
fn publishing_upload(ctx: &egui::Context, base: &str, tag: &str) -> (GuiState, u64, crate::test_temp::TempPath) {
    let mut state = page_state(demo_answer(), true);
    state.server_url = base.to_string();
    sync_server(&mut state);
    state.media.requested = true;
    state.upload_token = Some((base.to_string(), TOKEN.to_string()));
    state.private_key_bytes = Some(vec![7u8; 32]);
    let (f, form) = picked(tag, 10_000, "Taken back");
    uploading::start_with(&mut state, &form, Fake::new(8192)).expect("it starts");
    poll_until(ctx, &mut state, "it goes to be published", super::uploads_tests::publishing);
    let local = state.media.uploads[0].local;
    (state, local, f)
}

/// Read every answer until the uploads list is empty (ten seconds at most).
fn until_no_uploads(ctx: &egui::Context, state: &mut GuiState) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.media.uploads.is_empty() {
        poll(ctx, state);
        uploading::poll(ctx, state);
        assert!(Instant::now() < deadline, "the uploads list empties: {}", state.media.status);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A CANCELLED PUBLISH IS SAID CANCELLED ONLY WHEN TAKING IT OFF WENT (D4-2): Cancel while the
/// publish is on its way, and the worker takes the item off when the publish lands; the server
/// refuses that, and the page says so in the server's words, never "Cancelled".
///
/// Seen red 2026-10-11 with the worker's take-off answer dropped (round 3: "Cancelled" whatever it
/// got): "said in the server's words" (left: "Cancelled: nothing of it stays on the server.").
#[test]
fn a_cancelled_publish_is_said_cancelled_only_when_taken_off() {
    let ctx = egui::Context::default();
    let (base, seen) = http_server(vec![
        (Duration::from_millis(400), 200, r#"{"ok":true,"item":{"id":50,"url":"/uploads/1700_clip.webm"}}"#),
        (Duration::ZERO, 500, "storage: disk full"),
    ]);
    let (mut state, local, _f) = publishing_upload(&ctx, &base, "r4_undo");
    uploading::cancel(&mut state, local);
    until_no_uploads(&ctx, &mut state);
    assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("POST /api/media/remove")), "the worker tried to take it off");
    assert_eq!(state.media.status, publishing::not_taken_off_words("storage: disk full"), "said in the server's words");
}

/// A PUBLISH THAT CANNOT BE TAKEN OFF FOR WANT OF A SIGNATURE SAYS SO (D4-2): the publish's answer
/// is in when Cancel is pressed, so the page takes the item off itself, which needs a signature;
/// with the identity locked since, nothing can be sent, and the page says that, not "Cancelled".
///
/// Seen red 2026-10-11 with round 3's `take_off_now` (nothing sent, nothing said) and its
/// "Cancelled" on the answer: "said plainly" (left: "Cancelled: nothing of it stays on the
/// server.").
#[test]
fn a_publish_with_no_signature_to_take_it_off_says_so() {
    let ctx = egui::Context::default();
    let (base, seen) = http_server(vec![(Duration::ZERO, 200, r#"{"ok":true,"item":{"id":51,"url":"/uploads/1700_clip.webm"}}"#)]);
    let (mut state, local, _f) = publishing_upload(&ctx, &base, "r4_locked");
    let deadline = Instant::now() + Duration::from_secs(10);
    while seen.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the publish is sent");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(300)); // its answer is in, not yet read by the page
    state.private_key_bytes = None; // locked since
    uploading::cancel(&mut state, local);
    assert_eq!(state.media.status, publishing::NO_SIGNATURE_TO_TAKE_OFF, "said plainly");
    until_no_uploads(&ctx, &mut state);
    assert_eq!(state.media.status, publishing::NO_SIGNATURE_TO_TAKE_OFF, "and not overwritten by the publish's answer");
}

/// TYPING A SERVER ADDRESS NEVER ABANDONS UPLOADS (D4-3): the Chat page's connect form and the main
/// menu edit the Server field letter by letter while a connection is up or waiting to sign in
/// again; the Media page follows the CONNECTED server, so its uploads carry on and its token still
/// counts. Only a connection to another server moves the page.
///
/// Seen red 2026-10-11 with `sync_server` comparing against the Server field (round 3): "typing a
/// server address abandons no upload (left: 0 / right: 1)".
#[test]
fn typing_a_server_address_never_abandons_uploads() {
    let ctx = egui::Context::default();
    let mut state = page_state(demo_answer(), true);
    state.connected_server_url = SERVER.to_string();
    state.upload_token = Some((SERVER.to_string(), TOKEN.to_string()));
    let fake = Fake::new(8192);
    waits_an_hour(&fake);
    let (_f, form) = picked("r4_typing", 20_000, "Typing");
    uploading::start_with(&mut state, &form, fake.clone()).expect("it starts");
    for typed in ["h", "ht", "https://another.example"] {
        state.server_url = typed.to_string();
        state.hold_dialling_until_connect(); // what the field's edit does (BUG-160)
        sync_server(&mut state); // what every frame runs (poll_upload)
        uploading::poll(&ctx, &mut state);
    }
    assert_eq!(state.media.uploads.len(), 1, "typing a server address abandons no upload (left: {} / right: 1)", state.media.uploads.len());
    let flags = state.media.uploads[0].run.ctl.flags();
    assert!(!flags.cancelled && !flags.abandoned, "its upload carries on");
    assert_eq!(server_here(&state), crate::gui::pages::chat::norm_server_url(SERVER), "the page follows the connected server");
    assert!(signed_in_here(&state), "whose token still counts");
    // Connect pressed for the typed address: the connection is to another server now.
    state.connected_server_url = "https://another.example".to_string();
    assert!(sync_server(&mut state), "a connection to another server moves the page");
    assert!(state.media.uploads.is_empty());
}

/// A STALE RESUME NEVER SKIPS THE STOP FOR RETRY (D4-4): Pause and Resume pressed while the
/// worker is in no wait (here, before it starts) left the Retry flag set in round 3, so the tenth
/// server error in a row did not stop for Retry: ten more were sent first. Now a press sets the
/// flag only while a wait is held, and a held wait starts and ends with it clear.
///
/// Seen red 2026-10-11 with Resume setting the flag at every press and a held wait not clearing it
/// on entry (round 3): "the tenth of the same error stops it (left: 20 starts / right: 10)".
#[test]
fn a_stale_resume_never_skips_the_stop_for_retry() {
    let fake = Fake::new(8192);
    fake.s().start_forever = Some(Reply { status: 500, body: "storage: disk full".into() });
    let (_f, plan, _) = file(20_000);
    let ctl = Control::with_pace(Some(TOKEN.into()), Duration::from_millis(1));
    ctl.pause(true);
    ctl.pause(false);
    let (ctl, rx, handle) = spawn_work(&fake, plan, ctl);
    wait_for(&rx, |e| matches!(e, Event::Stuck { .. }));
    std::thread::sleep(WINDOW);
    let starts = fake.count("start");
    assert_eq!(starts, upload_waits::STUCK_AFTER as usize, "the tenth of the same error stops it (left: {starts} starts / right: 10)");
    ctl.cancel();
    handle.join().unwrap();
}

/// A STALE RESUME NEVER CUTS AN ALLOWANCE'S WAIT SHORT (D4-4): the same stale flag, in round 3,
/// ended the first wait for an allowance at once and sent the next piece.
///
/// Seen red 2026-10-11 with the same break: "the allowance's wait is waited (left: 2 pieces /
/// right: 1)".
#[test]
fn a_stale_resume_never_cuts_an_allowances_wait_short() {
    let fake = Fake::new(8192);
    waits_an_hour(&fake);
    let (_g, plan, _) = file(20_000);
    let ctl = Control::with_waits(Some(TOKEN.into()), Duration::from_millis(1), Duration::from_millis(1));
    ctl.pause(true);
    ctl.pause(false);
    let (ctl, rx, handle) = spawn_work(&fake, plan, ctl);
    wait_for(&rx, |e| matches!(e, Event::Waiting { .. }));
    std::thread::sleep(WINDOW);
    let pieces = fake.count("piece");
    assert_eq!(pieces, 1, "the allowance's wait is waited (left: {pieces} pieces / right: 1)");
    ctl.cancel();
    handle.join().unwrap();
}
