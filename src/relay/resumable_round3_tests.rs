//! Resumable uploads, round 3 (2026-10-11; docs/design/files-showcase.md, "Round 3: fixes from
//! the round 2 review"): pieces checked and counted under the room lock (R3-1), the pieces file
//! never lost on its way into place (R3-5), the allowance answer an app can act on (R3-6), and the
//! token in a header (R3-9). A child of resumable_tests.rs, so its rig and its file makers are at
//! hand. Each test names the break it was seen failing against ("Seen red").

use super::*;
use crate::relay::storage::upload_counts::{Span, DAY_MS, HOUR_MS};
use crate::relay::storage::upload_sessions::staged_part_name;

/// The verified role with `change` made to it, as an admin's Roles editor saves it.
fn verified_role(r: &Rig, change: impl FnOnce(&mut crate::relay::storage::RoleDef)) {
    let mut role = r.db().role_def("verified");
    change(&mut role);
    r.db().upsert_role(&role).unwrap();
}

/// The relay's clock, for the allowances, standing still at `at` (Unix ms).
fn clock_at(r: &Rig, at: i64) {
    assert!(r.state.upload_clock.set(Box::new(move || at)).is_ok(), "a new relay has no test clock yet");
}

/// A row that leaves `key`'s Media share `room` bytes, as a waiting upload would.
fn fill_share(r: &Rig, key: &str, share_mb: i64, room: i64) {
    r.settings(|s| s.media_per_person_mb = share_mb);
    r.db().record_upload(key, "9_filler.mp4", 100, MEDIA_SHELF, "filler.mp4", share_mb * MIB as i64 - room).unwrap();
}

fn unfill(r: &Rig) {
    r.db().with_conn(|c| c.execute("DELETE FROM user_uploads WHERE filename = '9_filler.mp4'", [])).unwrap();
}

/// Run `send` while `control` waits for the store to reach `gate`, acts, and opens it: the store
/// runs on a worker thread, so the two go on together.
async fn with_gate<T>(gate: &files::hold::Gate, send: impl std::future::Future<Output = T>, control: impl FnOnce()) -> T {
    let watch = async {
        for _ in 0..400 {
            if gate.reached() {
                control();
                gate.open();
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        gate.open();
        panic!("the store never reached the gate");
    };
    tokio::join!(send, watch).0
}

/// R3-1 (pieces). PIECES OF TWO UPLOADS SENT AT ONCE CANNOT BOTH PASS ON ONE ALLOWANCE: a role
/// allowed 1 MB a day starts two uploads and sends a 0.6 MB piece of each at the same moment. One
/// is taken and the other refused (429), and the day's count is what was written, never past the
/// allowance. (Each piece's check and count are made together under the room lock, at the time it
/// arrives, inside the blocking work, as a single request's store is.) Each piece is held once its
/// allowance is decided and before it is written, until both have got there or one was refused,
/// so the two are truly at once however fast this machine writes.
///
/// Seen red 2026-10-11 with the check before the write and the count after it, outside the room
/// lock, as merged: "one piece is taken and the other refused / left: 2 / right: 1". (Without the
/// hold this break passed: on a fast disk the first piece was written and counted before the
/// second was checked.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pieces_sent_at_once_cannot_both_pass_one_allowance() {
    let r = rig("resume_r31");
    r.person("ann", "tok_ann").await;
    verified_role(&r, |v| v.max_upload_mb_per_day = 1);
    let size = MIB * 6 / 10;
    let data = bytes(size as usize);
    let gate = files::hold::gate("piece:held-r31-pieces.mp4");
    let a = r.start("tok_ann", file("held-r31-pieces.mp4", size, "video/mp4")).await.unwrap();
    let b = r.start("tok_ann", file("held-r31-pieces.mp4", size, "video/mp4")).await.unwrap();

    let answered = std::sync::atomic::AtomicUsize::new(0);
    let send = |id: String| {
        let (r, data, answered) = (&r, &data, &answered);
        async move {
            let sent = r.send(&id, "tok_ann", 0, data).await;
            answered.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            sent
        }
    };
    let watch = async {
        for _ in 0..400 {
            if gate.arrivals() >= 2 || answered.load(std::sync::atomic::Ordering::SeqCst) >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        gate.open();
    };
    let (sent_a, sent_b, ()) = tokio::join!(send(a), send(b), watch);
    let taken = [sent_a.is_ok(), sent_b.is_ok()].iter().filter(|t| **t).count();
    assert_eq!(taken, 1, "one piece is taken and the other refused");
    let refused = [sent_a, sent_b].into_iter().find_map(|s| s.err()).unwrap();
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS, "{}", refused.words());
    let now = crate::relay::storage::now_millis() as i64;
    assert_eq!(r.db().upload_count("ann", Span::Day, now), size as i64, "the day's count is what was written");
}

/// R3-6. A 429 ALLOWANCE ANSWER SAYS HOW LONG TO WAIT AND HOW MUCH STILL FITS, by the relay's own
/// clock: with 1 MB a day, after 0.6 MB, the next 0.6 MB piece is refused with `resets_in_s` (the
/// seconds to midnight UTC) and `left_bytes` (what the day still allows). A piece cut to exactly
/// that is taken, smaller than `chunk_max` and not the last. With two uploads an hour used, a third
/// start is refused with the seconds to the top of the hour and `left_bytes: 0`.
///
/// Seen red 2026-10-11 with `OverAllowance::json` as merged (no `resets_in_s`, no `left_bytes`):
/// "resets_in_s: seconds to midnight UTC by the relay's clock / left: None / right: Some(4200)".
#[tokio::test]
async fn the_allowance_answer_says_how_long_to_wait_and_how_much_still_fits() {
    let r = rig("resume_r36");
    // 22:50 UTC: 70 minutes to midnight, 10 to the top of the hour.
    clock_at(&r, 20_000 * DAY_MS + 22 * HOUR_MS + 50 * 60_000);
    r.person("ann", "tok_ann").await;
    verified_role(&r, |v| {
        v.max_upload_mb_per_day = 1;
        v.max_uploads_per_hour = 2;
    });
    let size = MIB * 12 / 10;
    let data = bytes(size as usize);
    let half = (size / 2) as usize;
    let id = r.start("tok_ann", file("clip.mp4", size, "video/mp4")).await.unwrap();
    r.start("tok_ann", file("second.mp4", 100, "video/mp4")).await.unwrap(); // the hour's second upload
    r.send(&id, "tok_ann", 0, &data[..half]).await.unwrap();

    let refused = r.send(&id, "tok_ann", half as u64, &data[half..]).await.expect_err("past the day's allowance");
    let Refused::Json(code, body) = &refused else { panic!("a day's refusal is JSON: {refused:?}") };
    assert_eq!(*code, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["resets_in_s"].as_i64(), Some(4200), "resets_in_s: seconds to midnight UTC by the relay's clock");
    let left = MIB - half as u64;
    assert_eq!(body["left_bytes"].as_u64(), Some(left), "left_bytes: what the day still allows");
    assert_eq!((body["allowance"].as_str(), body["offset"].as_u64()), (Some("day"), Some(half as u64)));

    let fits = &data[half..half + left as usize];
    assert!(fits.len() < CHUNK_MAX, "test setup: a piece smaller than chunk_max");
    let taken = r.send(&id, "tok_ann", half as u64, fits).await.expect("a piece cut to left_bytes is taken");
    assert_eq!(taken["offset"].as_u64(), Some(half as u64 + left));

    let third = r.start("tok_ann", file("third.mp4", 100, "video/mp4")).await.expect_err("a third start in the hour");
    let Refused::Json(_, body) = &third else { panic!("an hour's refusal is JSON: {third:?}") };
    assert_eq!(body["allowance"], "hour");
    assert_eq!(body["resets_in_s"].as_i64(), Some(600), "ten minutes to the top of the hour");
    assert_eq!(body["left_bytes"].as_u64(), Some(0), "nothing can start until it resets");
}

/// R3-5 (a). A RELAY THAT STOPS WHILE A FINISH WAITS FOR THE ROOM LOCK LOSES NOTHING: the finish
/// moves the pieces file out of the resume folder under its session's own hidden name (seen here
/// while the finish is held there; let go, it stores the file as before). A relay that stops at
/// that moment leaves exactly that on disk: the start-up settling puts the file back with its
/// session (offset and all), instead of clearing it as a half copy and dropping the session as
/// one with no file, and the empty piece at its end stores it.
///
/// Seen red 2026-10-11 with the restore step taken out of `settle_uploads_at_start`: "the session
/// is kept with its file".
#[tokio::test]
async fn a_finish_cut_off_waiting_for_the_room_lock_is_put_back_at_start() {
    let r = rig("resume_r35a");
    r.person("ann", "tok_ann").await;
    let data = bytes(2000);
    let gate = files::hold::gate("held-r35a.mp4");
    let id = r.start("tok_ann", file("held-r35a.mp4", 2000, "video/mp4")).await.unwrap();
    let staged = r.waiting.join(staged_part_name(&id));
    let mut seen = (false, false);
    let done = with_gate(&gate, r.send(&id, "tok_ann", 0, &data), || seen = (staged.is_file(), r.part(&id).is_file())).await;
    assert_eq!(seen, (true, false), "while it waits, the pieces file is staged under its session's name");
    let done = done.expect("let go, the finish stores the file");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data);

    // The same moment again, and the relay stops there: what it leaves on disk.
    let id = r.start("tok_ann", file("clip.mp4", 2000, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann", 0, &data[..1999]).await.unwrap();
    std::fs::write(r.part(&id), &data).unwrap(); // the last piece written, the finish under way
    r.db().set_upload_session_offset(&id, 2000, crate::relay::storage::now_millis() as i64).unwrap();
    std::fs::rename(r.part(&id), r.waiting.join(staged_part_name(&id))).unwrap();
    std::fs::write(r.waiting.join(format!("{}left", crate::relay::storage::files::STAGED_PREFIX)), b"half a copy").unwrap();

    let settled = r.db().settle_uploads_at_start(r.folders()).unwrap();
    assert!(r.session(&id).is_some() && r.part(&id).is_file(), "the session is kept with its file");
    assert_eq!((settled.restored, settled.staged_cleared), (1, 1), "put back; the plain half copy is cleared");
    assert_eq!(r.session(&id).unwrap().offset, 2000);
    let done = r.send(&id, "tok_ann", 2000, &[]).await.expect("the empty piece at its end stores it");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data);
}

/// R3-5 (b). A PIECES FILE THAT CANNOT GO BACK AFTER A REFUSAL IS KEPT, NEVER DELETED: the finish
/// stages the file, then (held here) the person's 1 MB share fills and the resume folder
/// disappears, so the room check refuses (507) and neither a rename nor a copy can put the file
/// back. It stays where it
/// was staged, under its session's name, with the session; once there is a folder and room again,
/// the empty piece at its end brings it back and stores it.
///
/// Seen red 2026-10-11 with `unstage` as merged (the staged file removed when the rename back
/// failed): "the pieces file is kept where it was staged".
#[tokio::test]
async fn a_pieces_file_that_cannot_go_back_after_a_refusal_is_kept() {
    let r = rig("resume_r35b");
    r.person("ann", "tok_ann").await;
    let data = bytes(3000);
    let gate = files::hold::gate("held-r35b.mp4");
    let id = r.start("tok_ann", file("held-r35b.mp4", 3000, "video/mp4")).await.unwrap();
    let staged = r.waiting.join(staged_part_name(&id));
    // The finish reads the settings when it begins, before the gate: the share is set first, and
    // only what fills it arrives while the finish is held.
    r.settings(|s| s.media_per_person_mb = 1);
    let refused = with_gate(&gate, r.send(&id, "tok_ann", 0, &data), || {
        fill_share(&r, "ann", 1, 100);
        std::fs::remove_dir_all(&*r.resume).unwrap();
    })
    .await
    .expect_err("no room at the finish");
    assert_eq!(refused.status(), StatusCode::INSUFFICIENT_STORAGE, "{}", refused.words());
    let kept = staged.is_file() && std::fs::read(&staged).unwrap() == data;
    std::fs::create_dir_all(&*r.resume).unwrap();
    assert!(kept, "the pieces file is kept where it was staged");
    assert!(r.session(&id).is_some(), "with its session");

    unfill(&r);
    let done = r.send(&id, "tok_ann", 3000, &[]).await.expect("brought back and stored");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data);
    assert!(!staged.exists(), "nothing left staged");
}

/// R3-9. THE RESUMABLE ROUTES TAKE THE TOKEN FROM `X-Upload-Token`: an upload's status is read with
/// the token in the header alone; `?token=` still works; the header wins when both are sent; and
/// all four routes read it through `session_token`.
///
/// Seen red 2026-10-11 with `session_token` reading only the query: "the header alone reads the
/// status / left: 400 / right: 200".
#[tokio::test]
async fn the_resumable_routes_take_the_token_in_a_header() {
    let r = rig("resume_r39");
    r.person("ann", "tok_ann").await;
    let id = r.start("tok_ann", file("clip.mp4", 100, "video/mp4")).await.unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(files::UPLOAD_TOKEN_HEADER, "tok_ann".parse().unwrap());
    let ask = |q: SessionQuery, h: HeaderMap| status(State(r.state.clone()), Path(id.clone()), Query(q), h);

    assert_eq!(ask(SessionQuery::default(), headers.clone()).await.status().as_u16(), 200, "the header alone reads the status");
    let by_query = SessionQuery { token: Some("tok_ann".into()), offset: None };
    assert_eq!(ask(by_query, HeaderMap::new()).await.status().as_u16(), 200, "the query still works");
    assert_eq!(ask(SessionQuery::default(), HeaderMap::new()).await.status().as_u16(), 400, "no token at all");
    let both = SessionQuery { token: Some("someone_else".into()), offset: None };
    assert_eq!(session_token(&headers, &both).as_deref(), Some("tok_ann"), "the header wins");

    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/relay/resumable.rs")).unwrap().replace("\r\n", "\n");
    for route in ["pub async fn start(", "pub async fn status(", "pub async fn cancel(", "pub async fn piece("] {
        let at = src.find(route).unwrap_or_else(|| panic!("{route}"));
        let body = &src[at..at + src[at..].find("\n}\n").unwrap()];
        assert!(body.contains("session_token(&headers, &q)"), "{route} reads the token through session_token");
    }
}

/// R3-9 (nginx). A REFUSED RATE IS ANSWERED 429, NOT 503: both upload locations of the repo's nginx
/// config and of the self-hosting guide say `limit_req_status 429;`, so an app's "wait and try
/// again" sees the status that means it (a 503 reads as the server being down).
///
/// Seen red 2026-10-11 with the line taken out of the guide's resumable location:
/// "docs/admin/SELF-HOSTING.md: location /api/upload/resumable { answers a refused rate with 429".
#[test]
fn nginx_answers_a_refused_rate_with_429_in_the_upload_locations() {
    for file in ["scripts/nginx/humanity.conf", "docs/admin/SELF-HOSTING.md"] {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap().replace("\r\n", "\n");
        for location in ["location /api/upload {", "location /api/upload/resumable {"] {
            let at = text.find(location).unwrap_or_else(|| panic!("{file}: {location}"));
            let block = &text[at..at + text[at..].find("\n    }").unwrap()];
            assert!(block.contains("limit_req_status 429;"), "{file}: {location} answers a refused rate with 429");
        }
    }
}
