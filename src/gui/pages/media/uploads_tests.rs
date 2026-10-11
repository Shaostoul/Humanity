//! Tests of the uploads list and its publishes (round 3 of docs/design/media-showcase.md, the
//! 2026-10-11 review's findings 1 and 11, and the relay's R3-7), with the uploads sent to the
//! server simulated in memory (resumable_tests.rs) and the publishes to a server on this machine's
//! loopback address where one is needed (nothing leaves the machine). Each was seen to fail once
//! before it passed; what it saw red is in its comment.

use super::api::{Mine, Refusal};
use super::resumable::tests::{Fake, TOKEN};
use super::tests::{ctx_and_theme, demo_answer, page_state, settled, shows, SERVER};
use super::*;
use std::io::{Read as _, Write as _};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A file of `len` bytes to upload, and the form that picked it.
pub(crate) fn picked(tag: &str, len: usize, title: &str) -> (crate::test_temp::TempPath, add::Form) {
    let file = crate::test_temp::file(tag, "webm");
    std::fs::write(&file, (0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
    let form = add::Form { title: title.into(), ..add::Form::new(add::Source::Picked { path: file.to_path_buf(), name: format!("{tag}.webm"), len: len as u64 }) };
    (file, form)
}

/// Poll the uploads until `done` says so (ten seconds at most).
pub(crate) fn until(ctx: &egui::Context, state: &mut GuiState, what: &str, done: impl Fn(&GuiState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(state) {
        uploading::poll(ctx, state);
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn publishing(state: &GuiState) -> bool {
    assert!(!state.media.uploads.is_empty(), "an upload ended early: {}", state.media.status);
    state.media.uploads.iter().all(|e| matches!(e.after, uploading::After::Publishing { .. }))
}

/// A server on this machine's loopback address answering each request in turn with `answers` (a
/// wait, then its status and JSON body), closing the connection after each. Every request is
/// kept as "METHOD /path BODY". Returns its address.
pub(crate) fn http_server(answers: Vec<(Duration, u16, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let keep = seen.clone();
    std::thread::spawn(move || {
        for (wait, status, body) in answers {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut got = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                if let Some(i) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            };
            let head = String::from_utf8_lossy(&got[..head_end]).to_string();
            let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            while got.len() < head_end + len {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            let line = head.lines().next().unwrap_or("").split(' ').take(2).collect::<Vec<_>>().join(" ");
            keep.lock().unwrap().push(format!("{line} {}", String::from_utf8_lossy(&got[head_end..])));
            std::thread::sleep(wait);
            let _ = stream.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

/// PUT ON MEDIA FROM THE FILES PAGE DURING AN UPLOAD NEVER TOUCHES IT (finding 1): the upload is a
/// row of the list carrying what its own form said; opening another form replaces only the form,
/// and the upload, once up, is published by its own form, never deleted. Two uploads run at once,
/// each with its own row.
///
/// Seen red 2026-10-11 with the round 2 rule put back in `on_event`'s Done (no picked form being
/// sent open: take it for a Cancel): "the finished uploads are never deleted" (left: 2 removals
/// sent / right: 0).
#[test]
fn put_on_media_during_an_upload_never_touches_it() {
    let (ctx, theme) = ctx_and_theme();
    // A server that holds every publish unanswered, so the rows stay where this test looks.
    let (base, _) = http_server(vec![(Duration::from_secs(60), 200, "{}"); 2]);
    let mut state = page_state(demo_answer(), true);
    state.server_url = base.clone();
    sync_server(&mut state);
    state.media.requested = true;
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.upload_token = Some((base, TOKEN.to_string()));
    let (_a, dusk) = picked("dusk", 20_000, "Dusk");
    let (_b, dawn) = picked("dawn", 30_000, "Dawn");
    uploading::start_with(&mut state, &dusk, Fake::new(8192)).expect("the first starts");
    uploading::start_with(&mut state, &dawn, Fake::new(8192)).expect("a second starts beside it");
    // Put on Media before the page reads what the workers reported: whatever they did meanwhile,
    // the page learns of it (Done included) with this other form open.
    put_on_media(&mut state, "/uploads/1700_homestead.glb", "homestead.glb", 1000);
    let form_now = state.media.form.clone().expect("the Put on Media form is open");
    until(&ctx, &mut state, "both uploads finish", |s| s.media.uploads.iter().all(|e| !matches!(e.after, uploading::After::Uploading)));
    let removals = state.media.jobs.iter().filter(|(j, _)| matches!(j, Job::Remove { .. })).count();
    assert_eq!(removals, 0, "the finished uploads are never deleted (left: removals sent / right: none)");
    assert_eq!(state.media.uploads.len(), 2, "both stay in the list while they are published");
    assert!(publishing(&state), "both go to be published");
    let publishes: Vec<(String, u64)> =
        state.media.jobs.iter().filter_map(|(j, _)| if let Job::Publish { of: add::Of::Upload, filename, who } = j { Some((filename.clone(), *who)) } else { None }).collect();
    assert_eq!(publishes.len(), 2, "each is published: {:?}", state.media.jobs.iter().map(|(j, _)| j.clone()).collect::<Vec<_>>());
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Uploads in progress") && shows(&out, "dusk.webm") && shows(&out, "dawn.webm"), "each upload has its row");
    let locals: Vec<u64> = state.media.uploads.iter().map(|e| e.local).collect();
    assert!(publishes.iter().all(|(_, who)| locals.contains(who)), "each by its own row");
    assert_eq!(state.media.form.as_ref(), Some(&form_now), "the open form is untouched");
    assert_eq!(state.media.uploads.iter().map(|e| e.record.title.as_str()).collect::<Vec<_>>(), vec!["Dusk", "Dawn"], "each with what its own form said");
}

/// A LATE PUBLISH ANSWER NEVER LANDS ON ANOTHER FORM (finding 1): a form's publish answers to that
/// form's number only. A form opened since, its own publish on its way, is not closed by the old
/// one's success nor given its refusal; the old one's words are said on the page.
///
/// Seen red 2026-10-11 with `published` picking the open form by `sending` alone (as in round 2):
/// "a refusal for another form is not put on this one" (the new form's problem read "Not put on the
/// Media page: That file is not yours to publish. ..." and it was no longer sending).
#[test]
fn a_late_publish_answer_never_lands_on_another_form() {
    let mut state = page_state(demo_answer(), true);
    let old = add::Form::new(add::Source::Waiting { filename: "2_old.mp4".into(), name: "old.mp4".into(), len: 1 });
    let mut new = add::Form::new(add::Source::Existing { filename: "3_new.glb".into(), name: "new.glb".into(), len: 1 });
    new.sending = true; // its own publish is on its way
    state.media.form = Some(new.clone());
    on_done(&mut state, Job::Publish { of: add::Of::Waiting, filename: "2_old.mp4".into(), who: old.id }, Err(Refusal::new(403, "That file is not yours to publish.")));
    assert_eq!(state.media.form.as_ref(), Some(&new), "a refusal for another form is not put on this one");
    assert_eq!(state.media.status, "That file is not yours to publish.", "it is said on the page");
    on_done(&mut state, Job::Publish { of: add::Of::Waiting, filename: "2_old.mp4".into(), who: old.id }, Ok(serde_json::json!({ "ok": true })));
    assert_eq!(state.media.form.as_ref(), Some(&new), "nor is it closed by another's success");
    on_done(&mut state, Job::Publish { of: add::Of::Shared, filename: "3_new.glb".into(), who: new.id }, Ok(serde_json::json!({ "ok": true })));
    assert!(state.media.form.is_none(), "its own answer closes it");
}

/// A PUBLISH ALREADY ON MEDIA COUNTS AS DONE (finding 11): an upload whose publish was answered
/// after a quit or a switch is carried on, found finished, and published again; the relay answers
/// 409 "already on the Media page" (its owner check comes first, so it is the person's own). That
/// is done: the row and its record go, and the page says it is on Media.
///
/// Seen red 2026-10-11 without `settle` in `published`: "already on Media is done: [NotPublished {
/// filename: \"1700_clip.webm\", words: \"Not put on the Media page: That upload is already on the
/// Media page. ...\" }]" (the row stayed).
#[test]
fn a_publish_already_on_media_counts_as_done() {
    let ctx = egui::Context::default();
    let mut state = page_state(demo_answer(), true);
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.upload_token = Some((SERVER.to_string(), TOKEN.to_string()));
    let (_f, form) = picked("again", 10_000, "Again");
    uploading::start_with(&mut state, &form, Fake::new(8192)).expect("it starts");
    until(&ctx, &mut state, "it goes to be published", publishing);
    let local = state.media.uploads[0].local;
    assert_eq!(state.media.unfinished.len(), 1, "its record is kept until it is on Media");
    on_done(&mut state, Job::Publish { of: add::Of::Upload, filename: "1700_clip.webm".into(), who: local }, Err(Refusal::new(409, "That upload is already on the Media page.")));
    assert!(state.media.uploads.is_empty(), "already on Media is done: {:?}", state.media.uploads.iter().map(|e| e.after.clone()).collect::<Vec<_>>());
    assert!(state.media.unfinished.is_empty(), "and its record goes");
    assert_eq!(state.media.status, "It is on the Media page now.");
}

/// CANCEL DURING A PUBLISH, THEN A SWITCH, STILL TAKES IT OFF (finding 11): the publish's worker
/// holds the server's address and the signer itself, so when its answer lands after Cancel it
/// takes the item off there, even though the page has moved to another server and forgotten it.
///
/// Seen red 2026-10-11 with the worker's take-off switched off (leaving it to the page, as round 2
/// did, which the switch forgets): "taken off after the switch: [\"POST /api/media {...}\"]" (no
/// remove in ten seconds).
#[test]
fn cancel_during_a_publish_then_a_switch_still_takes_it_off() {
    let ctx = egui::Context::default();
    let (base, seen) = http_server(vec![
        (Duration::from_millis(400), 200, r#"{"ok":true,"item":{"id":50,"url":"/uploads/1700_clip.webm"}}"#),
        (Duration::ZERO, 200, r#"{"ok":true}"#),
    ]);
    let mut state = page_state(demo_answer(), true);
    state.server_url = base.clone();
    sync_server(&mut state);
    state.upload_token = Some((base.clone(), TOKEN.to_string()));
    state.private_key_bytes = Some(vec![7u8; 32]);
    let (_f, form) = picked("switch", 10_000, "Switch");
    uploading::start_with(&mut state, &form, Fake::new(8192)).expect("it starts");
    until(&ctx, &mut state, "it goes to be published", publishing);
    let local = state.media.uploads[0].local;
    uploading::cancel(&mut state, local);
    assert!(state.media.unfinished.is_empty(), "the record goes at the press");
    // Another server, at once: the page forgets this one's state, its jobs included.
    state.server_url = SERVER.to_string();
    assert!(sync_server(&mut state));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !seen.lock().unwrap().iter().any(|r| r.starts_with("POST /api/media/remove")) {
        assert!(Instant::now() < deadline, "taken off after the switch: {:?}", seen.lock().unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    let remove = seen.lock().unwrap().iter().find(|r| r.starts_with("POST /api/media/remove")).cloned().unwrap();
    assert!(remove.contains("\"filename\":\"1700_clip.webm\""), "the item it published: {remove}");
}

/// THE SERVER'S UNFINISHED UPLOADS THIS APP LOST TRACK OF ARE SHOWN WITH CANCEL (R3-7): the
/// `uploads_in_progress` of `/api/media/mine` are read, the ones neither running nor offered here
/// are listed with how much arrived, and Cancel tells the server. One this app has a record of is
/// offered with Carry on instead, not listed twice.
///
/// Seen red 2026-10-11 with `parse_mine` not reading `uploads_in_progress`: "both are read" (left:
/// 0 / right: 2).
#[test]
fn the_servers_unfinished_uploads_are_shown_with_cancel() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = page_state(demo_answer(), true);
    let answer = serde_json::json!({
        "used_bytes": 0, "share_bytes": 1 << 30, "budget_left_bytes": 1i64 << 40, "waiting": [],
        "uploads_in_progress": [
            { "id": "s1", "filename": "Holiday.mp4", "size": 4 << 20, "offset": 1 << 20, "touched_at": 1_760_000_000_000i64 },
            { "id": "s2", "filename": "Known.mp4", "size": 10, "offset": 5, "touched_at": 1 },
        ],
    });
    let mine: Mine = api::parse_mine(&answer);
    assert_eq!(mine.in_progress.len(), 2, "both are read");
    state.media.mine = Some(mine);
    state.media.unfinished = vec![resumable::Unfinished { id: "s2".into(), server: state.media.server.clone(), key: "me".into(), name: "Known.mp4".into(), size: 10, ..Default::default() }];
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Holiday.mp4: 1 MB of 4 MB arrived"), "the server's unfinished upload is listed");
    assert!(shows(&out, "An upload did not finish: Known.mp4"), "one with a record is offered to carry on");
    assert!(!shows(&out, "Known.mp4: "), "and not listed twice");
    // The Cancel beside it, on its row.
    let row = crate::gui::screen_surface::find_text_in_shapes(&out.shapes, "Holiday.mp4").unwrap().rect;
    let mut cancel = None;
    for cs in &out.shapes {
        if let egui::epaint::Shape::Text(t) = &cs.shape {
            let at = t.pos + t.galley.size() / 2.0;
            if t.galley.text() == "Cancel" && (at.y - row.center().y).abs() < 12.0 {
                cancel = Some(at);
            }
        }
    }
    let pos = cancel.expect("a Cancel beside it");
    let m = egui::Modifiers::default();
    let input = |events| egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), events, ..Default::default() };
    for events in [vec![egui::Event::PointerMoved(pos)], vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]] {
        let _ = ctx.run(input(events), |ctx| draw(ctx, &theme, &mut state));
    }
    assert!(state.media.jobs.iter().any(|(j, _)| *j == Job::Discard { id: "s1".into() }), "Cancel tells the server");
    // Round 4 (D4-2) changed this test's last line, which was "and the row goes" at the press:
    // the row now stays, saying it is being cancelled, until the server confirms (round4_tests.rs
    // `an_unfinished_upload_stays_listed_until_the_server_confirms_its_cancel`).
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, "Holiday.mp4") && shows(&out, "Cancelling..."), "and the row stays until the server confirms");
}
