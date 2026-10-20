//! A single request's day and hour allowances, round 3 (R3-1, 2026-10-11; docs/design/
//! files-showcase.md, "Round 3: fixes from the round 2 review"). The allowances were checked when
//! a request started and counted after its file was stored, back on the async side: requests sent
//! at once all passed on one allowance, a connection closed during the store left its file stored
//! and never counted, and an upload that crossed midnight UTC counted in the day it started. They
//! are now checked and counted together when the file is stored, under the room lock, inside the
//! store's blocking work (media.rs `store_upload`).
//!
//! The real `upload_file` handler's work is driven (`api.rs upload_file_in`) with real files, in
//! folders of the test's own; nothing lands in the checkout's data folder. Each test names the
//! break it was seen failing against ("Seen red").

use super::*;
use crate::relay::api::{upload_file_in, UploadQuery};
use crate::relay::files::{self, Folders};
use crate::relay::storage::upload_counts::{Span, DAY_MS};
use axum::extract::FromRequest;
use std::sync::Arc;

const MIB: u64 = 1024 * 1024;

fn relay(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

/// `key` is a verified member signed in with `token`, their role changed by `change`.
async fn uploader(state: &RelayState, key: &str, token: &str, change: impl FnOnce(&mut RoleDef)) {
    state.db.set_role(key, "verified").unwrap();
    let mut role = state.db.role_def("verified");
    change(&mut role);
    state.db.upsert_role(&role).unwrap();
    state.upload_tokens.write().await.insert(token.into(), key.into());
}

/// The folders of a test's own, and where a file streams as it arrives.
struct Dirs {
    uploads: crate::test_temp::TempPath,
    members: crate::test_temp::TempPath,
    resume: crate::test_temp::TempPath,
    waiting: crate::test_temp::TempPath,
    part: crate::test_temp::TempPath,
}

fn dirs(tag: &str) -> Dirs {
    let d = |what: &str| crate::test_temp::dir(&format!("{tag}_{what}"));
    Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting"), part: d("part") }
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
    /// The files stored in the chat folder (hidden ones left out).
    fn stored(&self) -> usize {
        std::fs::read_dir(&*self.uploads).unwrap().filter(|e| !e.as_ref().unwrap().file_name().to_string_lossy().starts_with('.')).count()
    }
}

/// `POST /api/upload` (to chat) of a `len`-byte file named `name`, by the session `token`.
async fn send(state: &Arc<RelayState>, d: &Dirs, token: &str, name: &str, len: usize) -> Result<axum::Json<serde_json::Value>, Refused> {
    let boundary = "round3Boundary";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: video/mp4\r\n\r\n").into_bytes();
    body.extend(std::iter::repeat(7u8).take(len));
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header(axum::http::header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .header(axum::http::header::CONTENT_LENGTH, body.len().to_string())
        .body(axum::body::Body::from(body))
        .unwrap();
    let headers = req.headers().clone();
    let multipart = axum::extract::Multipart::from_request(req, &()).await.expect("a multipart request");
    let query = UploadQuery { token: Some(token.into()), ..Default::default() };
    upload_file_in(state, d.folders(), &d.part, query, headers, multipart).await
}

fn now() -> i64 {
    crate::relay::storage::now_millis() as i64
}

/// R3-1 (a). SINGLE REQUESTS SENT AT ONCE CANNOT ALL PASS ON ONE ALLOWANCE: five 1 MB files sent
/// together against a day of 3 MB: three are stored and two refused (429, the day's words), and
/// the day's count is the three. The same with the hour's count: five at once against two uploads
/// an hour, two stored. (1 MB files: a request driven here, outside the route, is held to axum's
/// default 2 MB body limit, which the route itself raises.)
///
/// Seen red 2026-10-11 with the allowance check and the count taken back out of `store_upload`
/// and the count put back after the store's await, as merged: "three 1 MB files fit a 3 MB day:
/// [] / left: 5 / right: 3".
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requests_sent_at_once_cannot_all_pass_one_allowance() {
    let state = relay("r31_once");
    let d = dirs("r31_once");
    uploader(&state, "ann", "tok_ann", |r| {
        r.max_upload_mb = 25;
        r.max_upload_mb_per_day = 3;
        r.max_uploads_per_hour = 0;
        r.max_uploads_kept = 50;
    })
    .await;
    let names: Vec<String> = (0..5).map(|i| format!("day{i}.mp4")).collect();
    let sent = futures::future::join_all(names.iter().map(|n| send(&state, &d, "tok_ann", n, MIB as usize))).await;
    let stored = sent.iter().filter(|s| s.is_ok()).count();
    let refusals: Vec<_> = sent.iter().filter_map(|s| s.as_ref().err()).collect();
    assert_eq!(stored, 3, "three 1 MB files fit a 3 MB day: {refusals:?}");
    for refused in refusals {
        assert_eq!(refused.0, StatusCode::TOO_MANY_REQUESTS, "{}", refused.1);
        assert!(refused.1.contains("3 MB a day") && refused.1.contains("this file would pass that"), "{}", refused.1);
    }
    assert_eq!(state.db.upload_count("ann", Span::Day, now()), 3 * MIB as i64, "the day's count is the three stored");
    assert_eq!(d.stored(), 3, "and only they are on disk");

    // The hour's count of uploads, the same way, by someone else (each person has their own).
    uploader(&state, "bob", "tok_bob", |r| r.max_uploads_per_hour = 2).await;
    let names: Vec<String> = (0..5).map(|i| format!("hour{i}.mp4")).collect();
    let sent = futures::future::join_all(names.iter().map(|n| send(&state, &d, "tok_bob", n, 1000))).await;
    assert_eq!(sent.iter().filter(|s| s.is_ok()).count(), 2, "two uploads an hour: {sent:?}");
    assert_eq!(state.db.upload_count("bob", Span::Hour, now()), 2);
}

/// R3-1 (b). A CONNECTION CLOSED DURING THE STORE STILL COUNTS ITS FILE: the request's future is
/// dropped while its file is being stored (held here once the file is in place under its hidden
/// name). The store runs on by itself, as a blocking task does, and the file it stores is counted
/// against the day and the hour.
///
/// Seen red 2026-10-11 with the count back after the store's await, as merged: "the stored file
/// is counted against the day / left: 0 / right: 1048576".
#[tokio::test]
async fn a_connection_closed_during_the_store_still_counts_its_file() {
    let state = relay("r31_drop");
    let d = dirs("r31_drop");
    uploader(&state, "ann", "tok_ann", |r| r.max_upload_mb_per_day = 100).await;
    let gate = files::hold::gate("held-r31-drop.mp4");
    {
        let mut request = Box::pin(send(&state, &d, "tok_ann", "held-r31-drop.mp4", MIB as usize));
        let mut held = false;
        for _ in 0..400 {
            if gate.reached() {
                held = true;
                break;
            }
            if let Ok(done) = tokio::time::timeout(std::time::Duration::from_millis(25), request.as_mut()).await {
                panic!("the request ended before its store was held: {done:?}");
            }
        }
        assert!(held, "test setup: the store is held");
    } // the connection closes: the request's future is dropped, its store is not
    gate.open();
    let mut stored = false;
    for _ in 0..200 {
        if d.stored() == 1 && state.db.upload_count("ann", Span::Hour, now()) == 1 {
            stored = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(d.stored(), 1, "test setup: the store finished by itself");
    assert_eq!(state.db.upload_count("ann", Span::Day, now()), MIB as i64, "the stored file is counted against the day");
    assert!(stored, "and the upload against the hour");
}

/// R3-1 (c). AN UPLOAD THAT CROSSES MIDNIGHT UTC COUNTS IN THE DAY IT WAS STORED: the relay's
/// clock reads 23:59:59.900 when the request starts and 00:00:00.400 the next day by the time its
/// file is stored. The file is counted in the new day, and the day before has nothing.
///
/// Seen red 2026-10-11 with the store counting at the time the request started (the handler's
/// `now`), as merged: "counted in the day it was stored / left: 0 / right: 1048576".
#[tokio::test]
async fn an_upload_crossing_midnight_counts_in_the_day_it_was_stored() {
    let state = relay("r31_midnight");
    let d = dirs("r31_midnight");
    let midnight = 20_000 * DAY_MS;
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = reads.clone();
    let clock = move || if counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 { midnight - 100 } else { midnight + 400 };
    assert!(state.upload_clock.set(Box::new(clock)).is_ok(), "a new relay has no test clock yet");
    uploader(&state, "ann", "tok_ann", |r| r.max_upload_mb_per_day = 10).await;

    send(&state, &d, "tok_ann", "late.mp4", MIB as usize).await.expect("stored");
    assert!(reads.load(std::sync::atomic::Ordering::SeqCst) >= 2, "test setup: the clock was read at the start and at the store");
    assert_eq!(state.db.upload_count("ann", Span::Day, midnight + 400), MIB as i64, "counted in the day it was stored");
    assert_eq!(state.db.upload_count("ann", Span::Hour, midnight + 400), 1, "and its upload in that hour");
}
