//! Resumable uploads: the increment 2 follow-ups and the review's fixes (2026-10-11;
//! docs/design/files-showcase.md, "Increment 2 follow-ups, exact" and its "As built" note). A
//! child of resumable_tests.rs, so its rig and its file makers are at hand. Each test names the
//! break it was seen failing against ("Seen red"), with the words it failed with.

use super::*;
use crate::relay::storage::upload_counts::{Span, DAY_MS};

/// The verified role with `change` made to it, as an admin's Roles editor saves it.
fn verified_role(r: &Rig, change: impl FnOnce(&mut crate::relay::storage::RoleDef)) {
    let mut role = r.db().role_def("verified");
    change(&mut role);
    r.db().upsert_role(&role).unwrap();
}

/// A row that fills `key`'s Media share but `room` bytes, as a waiting upload would.
fn fill_share(r: &Rig, key: &str, share_mb: i64, room: i64) {
    r.settings(|s| s.media_per_person_mb = share_mb);
    r.db().record_upload(key, "9_filler.mp4", 100, MEDIA_SHELF, "filler.mp4", share_mb * MIB as i64 - room).unwrap();
}

fn unfill(r: &Rig) {
    r.db().with_conn(|c| c.execute("DELETE FROM user_uploads WHERE filename = '9_filler.mp4'", [])).unwrap();
}

/// How many of `key`'s uploads are stored.
fn stored_count(r: &Rig, key: &str) -> i64 {
    r.db().with_conn(|c| c.query_row("SELECT COUNT(*) FROM user_uploads WHERE public_key = ?1 AND filename != '9_filler.mp4'", [key], |row| row.get(0))).unwrap()
}

/// R1. A RETRIED FINISH NEVER HASHES THE STRIPPED FILE: a phone video sent with its BLAKE3 hash
/// reaches the finish with no room (507, kept); the strip has already rewritten its location boxes
/// by then. Room is made and the empty piece at its end tries again: the hash was checked once and
/// recorded, so the video is stored, without the place it was filmed.
///
/// Seen red 2026-10-11 with `finish` hashing on every try (the `!s.hash_checked` test taken out):
/// "the retried finish stores the video: Words(422, \"The file that arrived does not match the
/// file you sent ...\")".
#[tokio::test]
async fn a_retried_finish_never_hashes_the_stripped_file() {
    let r = rig("resume_r1");
    r.person("ann", "tok_ann").await;
    let video = phone_video();
    let hash = blake3::hash(&video).to_hex().to_string();
    let body = StartBody { blake3: Some(hash), ..file("walk.mp4", video.len() as u64, "video/mp4") };
    let id = r.start("tok_ann", body).await.unwrap();
    fill_share(&r, "ann", 1, 100);

    let full = r.send(&id, "tok_ann", 0, &video).await.expect_err("no room at the finish");
    assert_eq!(full.status(), StatusCode::INSUFFICIENT_STORAGE, "{}", full.words());
    assert!(r.session(&id).is_some_and(|s| s.hash_checked), "the hash was checked once and recorded");
    assert!(!contains(&std::fs::read(r.part(&id)).unwrap(), b"+47.6553-122.6870"), "test setup: the strip already changed the file");

    unfill(&r);
    let size = video.len() as u64;
    let done = r.send(&id, "tok_ann", size, &[]).await;
    let done = done.unwrap_or_else(|e| panic!("the retried finish stores the video: {e:?}"));
    let stored = std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap();
    assert!(!contains(&stored, b"+47.6553-122.6870"), "stored without the place it was filmed");
}

/// R2 (a). A CONNECTION DROPPED DURING THE FINISH LOSES NOTHING: the last piece's answer never
/// reaches the app (here: it is thrown away), yet the status says the upload is done and carries
/// the stored file's answer, and a PATCH on it answers the same without storing it twice.
///
/// Seen red 2026-10-11 with the finish deleting the session row as before: "the status after a
/// lost answer: Words(404, \"There is no upload in progress with that id.\")".
#[tokio::test]
async fn a_lost_finish_answer_is_found_again() {
    let r = rig("resume_r2a");
    r.person("ann", "tok_ann").await;
    let data = bytes(1500);
    let id = r.start("tok_ann", file("clip.mp4", 1500, "video/mp4")).await.unwrap();
    let _lost = r.send(&id, "tok_ann", 0, &data).await; // the app never sees this answer

    let status = status_core(&r.state, &id, Some("tok_ann")).await;
    let status = status.unwrap_or_else(|e| panic!("the status after a lost answer: {e:?}")).0;
    assert_eq!((status["offset"].as_i64(), status["size"].as_i64()), (Some(1500), Some(1500)));
    let done = status["done"].clone();
    let name = done["filename"].as_str().expect("the status carries the stored file's answer").to_string();
    assert_eq!(std::fs::read(r.waiting.join(&name)).unwrap(), data);

    let again = r.send(&id, "tok_ann", 1500, &[]).await.unwrap();
    assert_eq!(again, done, "a PATCH on a finished upload answers the same");
    let any_offset = r.send(&id, "tok_ann", 0, &data[..10]).await.unwrap();
    assert_eq!(any_offset, done, "and does nothing, wherever it says it starts");
    assert_eq!(stored_count(&r, "ann"), 1, "stored once");
    assert!(r.db().all_upload_sessions().unwrap().iter().all(|s| s.finished()), "the row stays, finished, until the idle expiry");
    assert!(r.start("tok_ann", file("next.mp4", 100, "video/mp4")).await.is_ok(), "a finished upload is not counted as in progress");
}

/// R2 (b). THE ONE-PIECE MARKER LIVES AS LONG AS THE FINISH: the app's request for the last piece
/// is dropped while the finish runs (held here at its start); a second request at once (the normal
/// resume: the status shows the file whole, then an empty piece) is told the upload is busy rather
/// than starting a second finish. Once the first is done, the PATCH answers with its result, and
/// the file is stored once.
///
/// Seen red 2026-10-11 with the marker held by the request alone (`writing.clone()` not moved
/// into the finish): "a second request while the finish runs is told it is busy:
/// Err(Elapsed(()))" (it started a second finish, which waited behind the first).
#[tokio::test]
async fn a_dropped_request_keeps_its_finish_to_itself() {
    let r = rig("resume_r2b");
    r.person("ann", "tok_ann").await;
    let data = bytes(1200);
    let id = r.start("tok_ann", file(HELD_FINISH_NAME, 1200, "video/mp4")).await.unwrap();
    let hold = FINISH_HOLD.lock().unwrap_or_else(|p| p.into_inner());
    {
        let mut first = Box::pin(r.send(&id, "tok_ann", 0, &data));
        let waited = tokio::time::timeout(std::time::Duration::from_millis(300), &mut first).await;
        assert!(waited.is_err(), "test setup: the finish is held");
    } // the app's connection drops: its request is gone, its finish is not

    let second = tokio::time::timeout(std::time::Duration::from_secs(2), r.send(&id, "tok_ann", 1200, &[])).await;
    match second {
        Ok(Err(busy)) => {
            assert_eq!(busy.status(), StatusCode::CONFLICT, "{busy:?}");
            assert!(matches!(&busy, Refused::Json(_, b) if b["busy"] == true), "{busy:?}");
        }
        other => panic!("a second request while the finish runs is told it is busy: {other:?}"),
    }
    drop(hold);
    let mut done = None;
    for _ in 0..100 {
        let s = status_core(&r.state, &id, Some("tok_ann")).await.unwrap().0;
        if s.get("done").is_some() {
            done = Some(s["done"].clone());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let done = done.expect("the first finish completed on its own");
    assert_eq!(r.send(&id, "tok_ann", 1200, &[]).await.unwrap(), done, "the PATCH answers with its result");
    assert_eq!(stored_count(&r, "ann"), 1, "stored once");
}

/// R3. A FILE THAT CANNOT BE WRITTEN AT THE FINISH KEEPS ITS UPLOAD: the pieces' file is made
/// read-only, so the location strip cannot open it (500); the session and its file stay. Made
/// writable again, the empty piece at its end stores it. (Only a video that cannot be read through
/// removes the session: a_videos_location_is_removed_at_finish.)
///
/// Seen red 2026-10-11 with every error of the strip removing the session (as merged): "a write
/// failure keeps the session: 500 Internal Server Error Failed to check the file's hidden
/// information: Access is denied. (os error 5)".
#[tokio::test]
async fn a_write_failure_at_the_finish_keeps_the_upload() {
    let r = rig("resume_r3");
    r.person("ann", "tok_ann").await;
    let video = phone_video();
    let size = video.len() as u64;
    let id = r.start("tok_ann", file("walk.mp4", size, "video/mp4")).await.unwrap();
    fill_share(&r, "ann", 1, 100);
    assert_eq!(r.send(&id, "tok_ann", 0, &video).await.unwrap_err().status(), StatusCode::INSUFFICIENT_STORAGE, "test setup: kept for no room");
    unfill(&r);

    let set_read_only = |on: bool| {
        let mut p = std::fs::metadata(r.part(&id)).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(on);
        std::fs::set_permissions(r.part(&id), p).unwrap();
    };
    set_read_only(true);
    let failed = r.send(&id, "tok_ann", size, &[]).await.expect_err("the strip cannot open a read-only file");
    let kept = r.session(&id).is_some() && r.part(&id).is_file();
    if r.part(&id).is_file() {
        set_read_only(false);
    }
    assert!(kept, "a write failure keeps the session: {} {}", failed.status(), failed.words());
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR, "{}", failed.words());

    let done = r.send(&id, "tok_ann", size, &[]).await.expect("made writable, it is stored");
    assert!(r.waiting.join(done["filename"].as_str().unwrap()).is_file());
}

/// B1. A ROLE'S LARGEST MEDIA FILE HOLDS AT THE START: with `max_media_file_mb` 1, a 2 MB file is
/// refused with words naming the limit before anything is written; a smaller one starts; 0 (every
/// role's default) leaves it to the share.
///
/// Seen red 2026-10-11 with the check turned off in `start_core`: "a 2 MB file over the role's
/// 1 MB Media limit is refused".
#[tokio::test]
async fn a_roles_media_file_limit_holds_at_the_start() {
    let r = rig("resume_b1");
    r.person("ann", "tok_ann").await;
    assert!(r.start("tok_ann", file("big.mp4", 2 * MIB, "video/mp4")).await.is_ok(), "0: up to the share");
    verified_role(&r, |v| v.max_media_file_mb = 1);
    let refused = r.start("tok_ann", file("big.mp4", 2 * MIB, "video/mp4")).await;
    let refused = refused.err().unwrap_or_else(|| panic!("a 2 MB file over the role's 1 MB Media limit is refused"));
    assert_eq!(refused.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(refused.words(), "Files on the Files page are limited to 1 MB for your role on this server.");
    assert!(r.start("tok_ann", file("small.mp4", MIB / 2, "video/mp4")).await.is_ok());
}

/// B3. THE DAY'S AND THE HOUR'S ALLOWANCE, IN PIECES: a role allowed 1 MB a day sends 0.6 MB, and
/// the next 0.6 MB piece is refused (429, in JSON: which allowance, when it resets, where the
/// upload is), nothing of it written and the session kept; after the reset (the day's count moved
/// back a day here) the same piece goes and the file is stored. A role allowed two uploads an hour
/// cannot start a third, told when the hour turns.
///
/// Seen red 2026-10-11 with the piece check in `piece_core` told the piece was empty: "the piece
/// past the day's allowance is refused".
#[tokio::test]
async fn the_days_allowance_stops_a_piece_and_the_rest_comes_after_the_reset() {
    let r = rig("resume_b3");
    r.person("ann", "tok_ann").await;
    verified_role(&r, |v| {
        v.max_upload_mb_per_day = 1;
        v.max_uploads_per_hour = 2;
    });
    let size = MIB * 12 / 10;
    let data = bytes(size as usize);
    let half = (size / 2) as usize;
    let id = r.start("tok_ann", file("clip.mp4", size, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann", 0, &data[..half]).await.unwrap();
    let refused = r.send(&id, "tok_ann", half as u64, &data[half..]).await;
    let refused = refused.err().unwrap_or_else(|| panic!("the piece past the day's allowance is refused"));
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    let Refused::Json(_, body) = &refused else { panic!("a day's refusal is JSON: {refused:?}") };
    let now = crate::relay::storage::now_millis() as i64;
    assert_eq!(body["allowance"], "day");
    assert_eq!(body["resets_at"].as_i64(), Some(Span::Day.resets_at(now)), "midnight UTC");
    assert_eq!(body["offset"].as_u64(), Some(half as u64), "where the upload is");
    let words = body["error"].as_str().unwrap();
    assert!(words.contains("1 MB a day") && words.contains("midnight UTC") && words.contains("send the rest after the reset"), "{words}");
    assert_eq!(std::fs::metadata(r.part(&id)).unwrap().len(), half as u64, "nothing of it was written");
    assert!(r.session(&id).is_some(), "the session is kept");

    let third = r.start("tok_ann", file("third.mp4", 100, "video/mp4")).await;
    assert!(third.is_ok(), "two starts this hour: {third:?}");
    let fourth = r.start("tok_ann", file("fourth.mp4", 100, "video/mp4")).await.expect_err("a third start in the hour");
    let Refused::Json(code, body) = &fourth else { panic!("an hour's refusal is JSON: {fourth:?}") };
    assert_eq!((*code, body["allowance"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("hour")));
    assert!(body["error"].as_str().unwrap().contains("2 uploads an hour"), "{body}");

    r.db().with_conn(|c| c.execute("UPDATE upload_counts SET starts_at = starts_at - ?1 WHERE span = 'day'", [DAY_MS])).unwrap();
    let done = r.send(&id, "tok_ann", half as u64, &data[half..]).await.expect("after the reset the rest goes");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data);
}

/// A HEIC still photo: `ftyp` then a top-level `meta` (which holds the picture and its location)
/// and the picture data, and no movie.
fn heic_photo() -> Vec<u8> {
    let meta = mp4_box(b"meta", &[[0u8; 4].to_vec(), mp4_box(b"iinf", b"GPS+47.6553-122.6870")].concat());
    [mp4_box(b"ftyp", b"heic\0\0\0\0mif1heic"), meta, mp4_box(b"mdat", &bytes(400))].concat()
}

/// R7 and B4. A HEIC PHOTO WHOSE ITEM TABLES CANNOT BE READ IS REFUSED, unless the person keeps
/// its hidden information. (Since the HEIF strip, 2026-10-11, a well-formed HEIC is cleaned and
/// stored instead: upload_rules_tests.rs. This test's synthetic photo has no item tables the strip
/// can read, so it is still refused with words that say what to do, and its session goes.) With `keep_metadata: 1` it is stored as sent,
/// and so are a phone video and a phone JPEG, with their locations, by the person's own choice;
/// a picture that is not what it says is refused either way.
///
/// Seen red 2026-10-11 with the no-movie check turned off in `finish_file_as`: "a HEIC photo is
/// refused". Seen red again with `keep_metadata` not stored on the session at the start: "kept by
/// choice, the HEIC is stored: Words(422, \"HEIC and AVIF photos cannot be cleaned of where they
/// were taken yet. ...\")".
#[tokio::test]
async fn a_heic_photo_is_refused_unless_the_person_keeps_everything() {
    let r = rig("resume_b4");
    r.person("ann", "tok_ann").await;
    let heic = heic_photo();
    let id = r.start("tok_ann", file("IMG_0001.mp4", heic.len() as u64, "video/mp4")).await.unwrap();
    let refused = r.send(&id, "tok_ann", 0, &heic).await;
    let refused = refused.err().unwrap_or_else(|| panic!("a HEIC photo is refused"));
    assert_eq!((refused.status(), refused.words()), (StatusCode::UNPROCESSABLE_ENTITY, crate::relay::upload_rules::HEIF_REFUSED.to_string()));
    assert!(r.session(&id).is_none(), "its session goes");

    let keep = |name: &str, len: usize, ct: &str| StartBody { keep_metadata: Some(1), ..file(name, len as u64, ct) };
    let id = r.start("tok_ann", keep("IMG_0001.mp4", heic.len(), "video/mp4")).await.unwrap();
    let done = r.send(&id, "tok_ann", 0, &heic).await.expect("kept by choice, the HEIC is stored");
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), heic);

    let video = phone_video();
    let id = r.start("tok_ann", keep("walk.mp4", video.len(), "video/mp4")).await.unwrap();
    let done = r.send(&id, "tok_ann", 0, &video).await.unwrap();
    let stored = std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap();
    assert!(contains(&stored, b"+47.6553-122.6870"), "kept by choice, the video keeps where it was filmed");

    let photo = jpeg_with_location();
    let id = r.start("tok_ann", keep("garden.jpg", photo.len(), "image/jpeg")).await.unwrap();
    let done = r.send(&id, "tok_ann", 0, &photo).await.unwrap();
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), photo, "and the photo its EXIF");

    let fake = r.start("tok_ann", keep("fake.png", 100, "image/png")).await.unwrap();
    let refused = r.send(&fake, "tok_ann", 0, &bytes(100)).await.expect_err("not a PNG, kept or not");
    assert_eq!(refused.words(), "File content does not match claimed image type.");
}
