//! The desktop's round 3 against the REAL relay's round 3 (docs/design/media-showcase.md, "Round 3:
//! fixes from the round 2 review"): the relay's own route functions served over HTTP on this
//! machine's loopback address, over a temporary database and folders of the test's own (nothing
//! lands in the checkout's data folder, nothing leaves the machine), and the app's own clients
//! talking to them: the resumable upload (`resumable::Http`), the ticket (`ticket::ask`) and the
//! download (`open::fetch_to`). The other round 3 tests answer with stand-ins written from the
//! design's shapes; these hold the app to what the relay actually answers. Since round 4 they
//! hold the app's cancel to the relay's R4-2 answer too (the last test). Each was seen to fail
//! once before it passed; what it saw red is in its comment.

use super::*;
use crate::relay::media::Folders;
use crate::relay::relay::RelayState;
use crate::relay::resumable as rr;
use crate::relay::storage::Storage;
use axum::extract::{DefaultBodyLimit, Path as AxPath, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The relay's four folders, of this test's own.
struct Dirs {
    uploads: crate::test_temp::TempPath,
    members: crate::test_temp::TempPath,
    resume: crate::test_temp::TempPath,
    waiting: crate::test_temp::TempPath,
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
}

/// A relay served on a loopback port: its state, its folders, its address, and the runtime
/// serving it (dropped with it).
struct Relay {
    state: Arc<RelayState>,
    dirs: Arc<Dirs>,
    base: String,
    /// The relay's clock for the allowances (Unix ms), set by the test.
    clock: Arc<AtomicI64>,
    _rt: tokio::runtime::Runtime,
}

/// The relay's routes the app's uploads, tickets and downloads use, each the relay's own function:
/// `status` and the ticket route as they are mounted in relay/mod.rs, the others their `*_core`
/// over this test's folders (the mounted ones use the live folders), reading the token the way
/// the mounted ones do (`session_token`: the header first).
fn relay(tag: &str) -> Relay {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let state = Arc::new(RelayState::new(Storage::open_temp(tag)));
    let clock = Arc::new(AtomicI64::new(1_000_000_000_000)); // 2001-09-09 01:46 UTC, by the relay's clock
    let read = clock.clone();
    assert!(state.upload_clock.set(Box::new(move || read.load(Ordering::SeqCst))).is_ok());
    let d = |what: &str| crate::test_temp::dir(&format!("{tag}_{what}"));
    let dirs = Arc::new(Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting") });
    for p in [&dirs.uploads, &dirs.members, &dirs.resume, &dirs.waiting] {
        std::fs::create_dir_all(p).unwrap();
    }
    let (d1, d2, d3, d4) = (dirs.clone(), dirs.clone(), dirs.clone(), dirs.clone());
    let app = axum::Router::new()
        .route(
            "/api/upload/resumable",
            post(move |State(s): State<Arc<RelayState>>, Query(q): Query<rr::SessionQuery>, h: HeaderMap, axum::Json(b): axum::Json<rr::StartBody>| {
                let d = d1.clone();
                async move { rr::start_core(&s, d.folders(), rr::session_token(&h, &q).as_deref(), b).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response) }
            }),
        )
        .route(
            "/api/upload/resumable/{id}",
            get(rr::status)
                .patch(move |State(s): State<Arc<RelayState>>, AxPath(id): AxPath<String>, Query(q): Query<rr::SessionQuery>, h: HeaderMap, body: axum::body::Bytes| {
                    let d = d2.clone();
                    async move {
                        rr::piece_core(&s, d.folders(), &id, rr::session_token(&h, &q).as_deref(), q.offset, body).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
                    }
                })
                .delete(move |State(s): State<Arc<RelayState>>, AxPath(id): AxPath<String>, Query(q): Query<rr::SessionQuery>, h: HeaderMap| {
                    let d = d3.clone();
                    async move { rr::cancel_core(&s, d.folders(), &id, rr::session_token(&h, &q).as_deref()).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response) }
                })
                .layer(DefaultBodyLimit::max(rr::PIECE_BODY_LIMIT)),
        )
        .route("/api/media/ticket", post(crate::relay::media_ticket::ticket))
        .route(
            "/api/media/file/{id}",
            get(move |State(s): State<Arc<RelayState>>, AxPath(id): AxPath<i64>, Query(q): Query<crate::relay::media::TokenQuery>, req: axum::extract::Request| {
                let d = d4.clone();
                async move { crate::relay::media::file_from(&s, d.folders(), id, q, req).await }
            }),
        )
        .with_state(state.clone());
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let port = listener.local_addr().unwrap().port();
    rt.spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Relay { state, dirs, base: format!("http://127.0.0.1:{port}"), clock, _rt: rt }
}

impl Relay {
    /// `key`, a verified member, signed in with `token`.
    fn person(&self, key: &str, token: &str) {
        self.state.db.join_server(key, key).unwrap();
        self.state.db.set_role(key, "verified").unwrap();
        self.state.upload_tokens.try_write().expect("test setup: nobody holds the tokens").insert(token.into(), key.into());
    }
}

/// `n` bytes that are not all the same (the relay's own tests' pattern).
fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
}

/// A file of `n` such bytes to upload as "clip.mp4", and its plan.
fn clip(tag: &str, n: usize) -> (crate::test_temp::TempPath, resumable::Plan, Vec<u8>) {
    let path = crate::test_temp::file(tag, "mp4");
    let data = bytes(n);
    std::fs::write(&path, &data).unwrap();
    let plan = resumable::Plan { path: path.to_path_buf(), name: "clip.mp4".into(), size: n as u64, content_type: "video/mp4".into(), resume: None, file_changed: false, clean: false, keep_metadata: false };
    (path, plan, data)
}

/// THE WHOLE UPLOAD AGAINST THE REAL RELAY, WITH ITS DAY'S ALLOWANCE (R3-6, R3-9; findings 3 and
/// 5): every request carries the token in `X-Upload-Token` alone, which the relay's own routes
/// read; a role allowed 1 MB a day sending 1.5 MB has its first piece refused with `left_bytes`
/// and cuts it to that, so the first megabyte goes the same day; the rest waits the relay's own
/// `resets_in_s` (the relay's clock here reads 2001, so its `resets_at` is long past by this PC's,
/// round 2's no-wait case), and goes once the day turns and Resume is pressed. The relay ends up
/// with the file exactly.
///
/// Seen red 2026-10-11 with the piece never cut (as in round 2): "the first megabyte goes the same
/// day: [Hashing ..., Started ..., Waiting { words: \"Your role may upload 1 MB a day on this
/// server, and you have uploaded 0.0 MB today. ...\" }, Sending, Waiting ...]" (the whole file
/// refused each day, nothing taken).
#[test]
fn the_whole_upload_against_the_real_relay_with_its_allowance() {
    let r = relay("desk_r3_allow");
    r.person("ann", "tok_ann");
    let mut role = r.state.db.role_def("verified");
    role.max_upload_mb_per_day = 1;
    r.state.db.upsert_role(&role).unwrap();
    let (_f, plan, data) = clip("desk_r3_allow_clip", 1024 * 1024 * 3 / 2);
    let ctl = Arc::new(resumable::Control::with_pace(Some("tok_ann".into()), Duration::from_millis(5)));
    let (tx, rx) = std::sync::mpsc::channel();
    let (worker_ctl, base) = (ctl.clone(), r.base.clone());
    let handle = std::thread::spawn(move || resumable::work(&resumable::Http::new(&base), &worker_ctl, &plan, &tx));

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut events = Vec::new();
    let mut waited = None;
    loop {
        let e = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_else(|_| panic!("the upload stalled: {events:?}"));
        if let resumable::Event::Waiting { words, resets_at } = &e {
            if waited.is_none() {
                waited = Some((words.clone(), *resets_at));
                r.clock.fetch_add(24 * 3600 * 1000, Ordering::SeqCst); // the day turns
                ctl.pause(false); // Resume
            } else {
                ctl.cancel(); // a second day's wait: stop, and let the assertions say why
            }
        }
        let last = matches!(e, resumable::Event::Done { .. } | resumable::Event::Failed { .. } | resumable::Event::Cancelled { .. });
        events.push(e);
        if last {
            break;
        }
    }
    handle.join().unwrap();
    let cut = events.iter().position(|e| *e == resumable::Event::Progress { offset: 1024 * 1024 });
    let wait = events.iter().position(|e| matches!(e, resumable::Event::Waiting { .. }));
    assert!(cut.is_some() && cut < wait, "the first megabyte goes the same day: {events:?}");
    let (words, resets_at) = waited.expect("the day's allowance was waited");
    assert!(words.starts_with("Your role may upload 1 MB a day on this server"), "the relay's own words: {words}");
    assert!(resets_at > resumable_now() + 20 * 3600 * 1000, "the relay's own resets_in_s, about 22 hours: {} ms ahead", resets_at - resumable_now());
    let Some(resumable::Event::Done { answer }) = events.last() else { panic!("it finished: {events:?}") };
    let stored = resumable::stored_name_of(answer);
    let on_disk = [&r.dirs.waiting, &r.dirs.uploads].iter().map(|d| d.join(&stored)).find(|p| p.exists()).expect("the stored file");
    assert_eq!(std::fs::read(on_disk).unwrap(), data, "the relay holds the file exactly");
}

fn resumable_now() -> i64 {
    upload_waits::now_ms()
}

/// THE SERVER'S UNFINISHED UPLOAD IS LISTED FROM THE REAL RELAY'S ANSWER, AND ITS CANCEL REACHES
/// THE RELAY (R3-7): an upload stopped where it was (the app moved away) is in the relay's
/// `/api/media/mine` as `uploads_in_progress`; with no record of it here the page lists it, and
/// its Cancel (the page's own request, over HTTP with the token in the header) ends the relay's
/// session.
///
/// Seen red 2026-10-11 with `parse_mine` not reading `uploads_in_progress`: "the relay's answer
/// lists it" (left: [] / right: [(\"7f4fe323...\", \"clip.mp4\", 1572864, 1048576)]).
#[test]
fn the_servers_unfinished_upload_is_listed_and_cancelled_on_the_real_relay() {
    let r = relay("desk_r3_mine");
    r.person("ann", "tok_ann");
    // A day's allowance of 1 MB holds a 1.5 MB upload part way, unfinished.
    let mut role = r.state.db.role_def("verified");
    role.max_upload_mb_per_day = 1;
    r.state.db.upsert_role(&role).unwrap();
    let size = 1024 * 1024 * 3 / 2;
    let (_f, plan, _) = clip("desk_r3_mine_clip", size);
    let ctl = Arc::new(resumable::Control::with_pace(Some("tok_ann".into()), Duration::from_millis(5)));
    let (tx, rx) = std::sync::mpsc::channel();
    let (worker_ctl, base) = (ctl.clone(), r.base.clone());
    let handle = std::thread::spawn(move || resumable::work(&resumable::Http::new(&base), &worker_ctl, &plan, &tx));
    let mut id = String::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(20)).expect("it starts and waits for the allowance") {
            resumable::Event::Started { id: started } => id = started,
            resumable::Event::Waiting { .. } => break,
            _ => {}
        }
    }
    ctl.abandon(); // stopped where it is, as a switch to another server does
    handle.join().unwrap();

    let settings = r.state.db.get_server_settings().unwrap();
    let mine = crate::relay::media::mine_core(&r.state.db, &settings, "ann").unwrap().0;
    let parsed = api::parse_mine(&mine);
    let listed: Vec<(String, String, i64, i64)> = parsed.in_progress.iter().map(|p| (p.id.clone(), p.filename.clone(), p.size, p.offset)).collect();
    assert_eq!(listed, vec![(id.clone(), "clip.mp4".to_string(), size as i64, 1024 * 1024)], "the relay's answer lists it");

    let mut state = super::tests::page_state(Default::default(), false);
    state.server_url = r.base.clone();
    sync_server(&mut state);
    state.upload_token = Some((r.base.clone(), "tok_ann".into()));
    state.media.mine = Some(parsed);
    assert_eq!(waiting::unknown_sessions(&state).len(), 1, "with no record here, the page lists it");
    uploading::cancel_on_server(&mut state, &id);
    let deadline = Instant::now() + Duration::from_secs(20);
    while r.state.db.upload_session(&id).unwrap().is_some() {
        assert!(Instant::now() < deadline, "its Cancel ends the relay's session");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Upload `n` bytes as `tag` through the real relay with the app's own client, to the end: the
/// server's id for it and the stored file's name.
fn upload_whole(r: &Relay, tag: &str, n: usize) -> (String, String) {
    let (_f, plan, _) = clip(tag, n);
    let ctl = Arc::new(resumable::Control::with_pace(Some("tok_ann".into()), Duration::from_millis(5)));
    let (tx, rx) = std::sync::mpsc::channel();
    let (worker_ctl, base) = (ctl.clone(), r.base.clone());
    let handle = std::thread::spawn(move || resumable::work(&resumable::Http::new(&base), &worker_ctl, &plan, &tx));
    let mut id = String::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(20)).expect("the upload goes") {
            resumable::Event::Started { id: started } => id = started,
            resumable::Event::Done { answer } => {
                handle.join().unwrap();
                return (id, resumable::stored_name_of(&answer));
            }
            e @ (resumable::Event::Failed { .. } | resumable::Event::Cancelled { .. }) => panic!("the upload ended: {e:?}"),
            _ => {}
        }
    }
}

/// Cancel the server's upload `id` from the page, as its Cancel does, and read the answer.
fn cancel_from_the_page(r: &Relay, id: &str) -> GuiState {
    let mut state = super::tests::page_state(Default::default(), false);
    state.server_url = r.base.clone();
    sync_server(&mut state);
    state.media.requested = true;
    state.upload_token = Some((r.base.clone(), "tok_ann".into()));
    assert!(uploading::cancel_on_server(&mut state, id), "the cancel is asked");
    let ctx = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(20);
    while state.media.jobs.iter().any(|(j, _)| matches!(j, Job::Discard { .. })) {
        poll(&ctx, &mut state);
        assert!(Instant::now() < deadline, "the relay answers the cancel");
        std::thread::sleep(Duration::from_millis(10));
    }
    state
}

/// A CANCEL OF A FINISHED UPLOAD LEAVES NOTHING ON THE REAL RELAY, AND ONE ALREADY ON MEDIA STAYS
/// (round 4: R4-2 and D4-2): an upload whose every piece arrived and was stored, but not
/// published, is cancelled from the page (the relay's own `DELETE`, the token in the header); the
/// relay answers `removed_stored`, its stored file and session are gone, and the page says
/// "Cancelled". One already published is answered `published`: the item stays, and the page says
/// so, never "Cancelled".
///
/// Seen red 2026-10-11 with `upload_cancel::settle` reading `published` under another name
/// ("on_media"): "the page says the item stays" (left: "Cancelled: nothing of it stays on the
/// server.").
#[test]
fn a_finished_upload_cancelled_on_the_real_relay_leaves_nothing() {
    let r = relay("desk_r4_cancel");
    r.person("ann", "tok_ann");
    let (id, stored) = upload_whole(&r, "desk_r4_cancel_a", 300_000);
    assert!(r.dirs.waiting.join(&stored).exists(), "stored, waiting to be published");
    let state = cancel_from_the_page(&r, &id);
    assert_eq!(state.media.status, uploading::CANCELLED_LINE, "confirmed by the relay");
    assert!(!r.dirs.waiting.join(&stored).exists(), "the stored file is gone");
    assert!(r.state.db.upload_session(&id).unwrap().is_none(), "and the session");

    let (id, stored) = upload_whole(&r, "desk_r4_cancel_b", 300_000);
    let settings = r.state.db.get_server_settings().unwrap();
    let body = crate::relay::media::PublishBody { key: "ann".into(), filename: stored.clone(), ..Default::default() };
    crate::relay::media::publish_core(&r.state.db, &settings, &[], r.dirs.folders(), &body).expect("published");
    let state = cancel_from_the_page(&r, &id);
    assert_eq!(state.media.status, upload_cancel::ALREADY_ON_MEDIA, "the page says the item stays");
    assert!(r.state.db.media_item_for_upload(&stored).unwrap().is_some(), "and it does");
}

/// A MEMBERS-ONLY FILE COMES THROUGH A TICKET FROM THE REAL RELAY (R3-8): the app asks the relay's
/// own ticket route with the token in the header and the item's number and file name, accepts the
/// address it answers (that file's, on this server), and downloads through it with no token at
/// all, into a part file named only when complete.
///
/// Seen red 2026-10-11 with `ticket_address` expecting the ticket under another name ("key="): "the
/// relay's ticket is accepted: The server's answer for this file was not understood."
#[test]
fn a_members_only_file_comes_through_a_ticket_from_the_real_relay() {
    let r = relay("desk_r3_ticket");
    r.person("ann", "tok_ann");
    std::fs::write(r.dirs.waiting.join("1_dusk.mp4"), b"0123456789").unwrap();
    r.state.db.record_upload("ann", "1_dusk.mp4", 100, crate::relay::storage::media::MEDIA_SHELF, "dusk.mp4", 10).unwrap();
    let settings = r.state.db.get_server_settings().unwrap();
    let body = crate::relay::media::PublishBody { key: "ann".into(), filename: "1_dusk.mp4".into(), visibility: Some("members".into()), ..Default::default() };
    let answer = crate::relay::media::publish_core(&r.state.db, &settings, &[], r.dirs.folders(), &body).unwrap().0;
    let item = api::Item::from_json(&answer["item"]).expect("the relay's item, as the app reads it");
    assert!(item.members_only());

    let origin = open::origin_of(&r.base, &item, Some("tok_ann")).expect("an origin");
    let address = origin.address().unwrap_or_else(|e| panic!("the relay's ticket is accepted: {e}"));
    // Round 5: the ticket names which of the item's files it opens (`file=0`, the item's own).
    assert!(address.starts_with(&format!("{}/api/media/file/{}?file=0&ticket=", r.base, item.id)) && !address.contains("tok_ann"), "{address}");
    let dir = crate::test_temp::dir("desk_r3_ticket_dl");
    let saved = open::fetch_to(&open::transfer_agent(Duration::from_secs(10)), &address, None, &dir, &open::download_name(&item), 1 << 20).expect("downloaded");
    assert_eq!(std::fs::read(&saved).unwrap(), b"0123456789", "the file, through the ticket alone");
    assert_eq!(saved.file_name().and_then(|n| n.to_str()), Some("dusk.mp4"), "under the uploader's own name");
}
