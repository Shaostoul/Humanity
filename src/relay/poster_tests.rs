//! A video's poster picture, the relay half (2026-11-06; docs/design/media-player.md, "Rung 2: the
//! poster picture"). A child of relay/poster.rs.
//!
//! The real upload handler's work is driven (`api.rs upload_file_in`) with real files in folders of
//! the test's own, the message deletes through the relay's own (`delete_own_message_in`,
//! `delete_message_by_id_in`), the erase, the expiry pass, the Files page's publish, visibility,
//! removal and copyright decision through their `*_core` functions, and the public address through
//! the same `/uploads` service the relay mounts, on a loopback port. Nothing lands in the
//! checkout's data folder. Each test names the break it was seen failing against ("Seen red").

use super::*;
use crate::relay::api::{upload_file_in, UploadQuery};
use crate::relay::files::{labels, publish_core, remove_core, visibility_core, Folders, PublishBody, RemoveTarget};
use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::files::poster_name;
use crate::relay::storage::upload_counts::Span;
use crate::relay::storage::Storage;
use axum::extract::FromRequest;
use std::sync::Arc;

const ANN: &str = "ann_key";
const BOB: &str = "bob_key";
const MOD: &str = "mod_key";

// ── Pictures and videos ──

/// A real JPEG `w` x `h`, a gradient, made here.
fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80).encode_image(&img).unwrap();
    out.into_inner()
}

/// The words a phone writes where it took a picture, as they ride in an Exif segment.
const WHERE_TAKEN: &[u8] = b"GPSLatitude 47.5650 N GPSLongitude 122.6270 W";

/// `jpeg` with an Exif segment (APP1) holding [`WHERE_TAKEN`] just after its start.
fn jpeg_with_exif(w: u32, h: u32) -> Vec<u8> {
    let plain = jpeg(w, h);
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(WHERE_TAKEN);
    let len = (payload.len() + 2) as u16;
    let mut out = plain[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&plain[2..]);
    out
}

/// A real PNG, for a poster picture of the wrong type.
fn png() -> Vec<u8> {
    let img = image::RgbImage::from_fn(32, 18, |_, _| image::Rgb([10, 20, 30]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

// ── The picture itself ──

/// A POSTER PICTURE IS A SMALL JPEG, CLEANED. A 640 by 360 JPEG is taken, and comes back without
/// the Exif segment that held where it was taken; a PNG, bytes that start like a JPEG and are not
/// one, a JPEG 641 pixels wide (or tall), and one over 128 KB are each refused with words saying
/// why.
///
/// Seen red 2026-11-06 with `check_poster`'s side check taken out: the 641 x 100 picture was taken
/// ("called `Result::unwrap_err()` on an `Ok` value"); with the strip taken out (the bytes handed
/// back as they came): "where it was taken is gone".
#[test]
fn a_poster_picture_is_a_small_jpeg_and_is_cleaned() {
    let kept = check_poster(jpeg_with_exif(640, 360)).expect("a 640 x 360 JPEG is a poster picture");
    assert!(kept.starts_with(&[0xFF, 0xD8, 0xFF]), "still a JPEG");
    assert!(!contains(&kept, WHERE_TAKEN), "where it was taken is gone");
    assert!(image::load_from_memory_with_format(&kept, image::ImageFormat::Jpeg).is_ok(), "and it still decodes");

    let refused = check_poster(png()).unwrap_err();
    assert_eq!((refused.0, refused.1.as_str()), (StatusCode::UNSUPPORTED_MEDIA_TYPE, NOT_A_JPEG), "a PNG is refused");
    let mut fake = vec![0xFF, 0xD8, 0xFF, 0xE0];
    fake.extend(std::iter::repeat(0x41).take(2000));
    assert_eq!(check_poster(fake).unwrap_err().0, StatusCode::UNSUPPORTED_MEDIA_TYPE, "bytes that only start like a JPEG are refused");
    for (w, h) in [(641, 100), (100, 641)] {
        let refused = check_poster(jpeg(w, h)).unwrap_err();
        assert_eq!(refused.0, StatusCode::BAD_REQUEST, "a {w} x {h} picture is refused: {}", refused.1);
        assert!(refused.1.contains("at most 640 pixels"), "{}", refused.1);
    }
    let mut big = jpeg(64, 36);
    big.resize(POSTER_MAX_BYTES + 1, 0);
    let refused = check_poster(big).unwrap_err();
    assert_eq!(refused.0, StatusCode::PAYLOAD_TOO_LARGE, "{}", refused.1);
    assert!(refused.1.contains("at most 128 KB"), "{}", refused.1);
}

// ── The upload ──

fn relay(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

/// `key` is a verified member signed in with `token`, keeping `keep` chat files.
async fn uploader(state: &RelayState, key: &str, token: &str, keep: i64) {
    state.db.join_server(key, key).unwrap();
    state.db.set_role(key, "verified").unwrap();
    let mut role = state.db.role_def("verified");
    role.max_upload_mb = 25;
    role.max_upload_mb_per_day = 500;
    role.max_uploads_per_hour = 0;
    role.max_uploads_kept = keep;
    state.db.upsert_role(&role).unwrap();
    state.upload_tokens.write().await.insert(token.into(), key.into());
}

/// The folders of a test's own, and where a file streams as it arrives.
struct Dirs {
    uploads: humanity_core::test_temp::TempPath,
    members: humanity_core::test_temp::TempPath,
    resume: humanity_core::test_temp::TempPath,
    waiting: humanity_core::test_temp::TempPath,
    part: humanity_core::test_temp::TempPath,
}

fn dirs(tag: &str) -> Dirs {
    let d = |what: &str| humanity_core::test_temp::dir(&format!("{tag}_{what}"));
    Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting"), part: d("part") }
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
    /// The names in `dir` (hidden ones left out), sorted.
    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
        v.sort();
        v
    }
}

/// One form part: its name, its file name, its type and its bytes.
struct Part<'a>(&'a str, &'a str, &'a str, Vec<u8>);

/// The bytes of a video file as the tests send it (not a film: the relay only looks at an MP4's
/// boxes, and these are none).
fn video_bytes() -> Vec<u8> {
    std::iter::repeat(7u8).take(4096).collect()
}

/// `POST /api/upload` of `parts`, in order, by the session `token`, with `query`'s flags.
async fn send(state: &Arc<RelayState>, d: &Dirs, token: &str, parts: Vec<Part<'_>>, mut query: UploadQuery) -> Result<serde_json::Value, (StatusCode, String)> {
    let boundary = "posterBoundary";
    let mut body = Vec::new();
    for Part(name, filename, ty, bytes) in parts {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {ty}\r\n\r\n").as_bytes());
        body.extend_from_slice(&bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header(axum::http::header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .header(axum::http::header::CONTENT_LENGTH, body.len().to_string())
        .body(axum::body::Body::from(body))
        .unwrap();
    let headers = req.headers().clone();
    let multipart = axum::extract::Multipart::from_request(req, &()).await.expect("a multipart request");
    query.token = Some(token.into());
    upload_file_in(state, d.folders(), &d.part, query, headers, multipart).await.map(|j| j.0)
}

/// A video with its poster picture, as both apps send one: the poster part first.
fn with_poster(name: &str, poster: Vec<u8>) -> Vec<Part<'_>> {
    vec![Part(POSTER_FIELD, "poster.jpg", "image/jpeg", poster), Part("file", name, "video/mp4", video_bytes())]
}

/// The stored name from an upload's answer.
fn stored(answer: &serde_json::Value) -> String {
    answer["filename"].as_str().expect("a stored name").to_string()
}

/// THE RELAY KEEPS A VIDEO'S POSTER PICTURE BESIDE IT, AND REFUSES A WRONG ONE. A video sent with
/// its poster picture first: the answer names both, the picture is kept as `<video>.poster.jpg`
/// in the same folder, cleaned of where it was taken, its size in the video's row, and the day's
/// count holds the video's bytes and the picture's. Refused, with nothing stored at all: a PNG as
/// the poster picture, one over 128 KB, one over 640 pixels, two of them, one with a document, and
/// one with a file from a private conversation (`encrypted=1`). A video sent without one is kept
/// as before, with no picture.
///
/// Seen red 2026-11-06 with `allowed_with` answering Ok for anything: "a poster picture with a
/// document is refused" (`notes.pdf` was stored, a poster picture beside it); with only its
/// private-conversation check taken out: "a poster picture with a file from a private conversation:
/// said why" (left: "Only a video (MP4 or WebM) can carry a poster picture.", the second check
/// still refusing it, but for the wrong reason, which is why the words are held too); with the
/// strip taken out of `check_poster`: "a JPEG, cleaned of where it was taken".
#[tokio::test]
async fn the_relay_keeps_a_videos_poster_picture_beside_it_and_refuses_a_wrong_one() {
    let state = relay("poster_store");
    let d = dirs("poster_store");
    uploader(&state, ANN, "tok_ann", 50).await;

    let picture = jpeg_with_exif(640, 360);
    let answer = send(&state, &d, "tok_ann", with_poster("meetup.mp4", picture), UploadQuery::default()).await.expect("a video with its poster picture is stored");
    let name = stored(&answer);
    assert_eq!(answer["poster"].as_str(), Some(format!("/uploads/{}", poster_name(&name)).as_str()), "the answer names the poster picture: {answer}");
    assert_eq!(Dirs::names(&d.uploads), vec![name.clone(), poster_name(&name)], "the video and its picture, side by side");
    let kept = std::fs::read(d.uploads.join(poster_name(&name))).unwrap();
    assert!(kept.starts_with(&[0xFF, 0xD8, 0xFF]) && !contains(&kept, WHERE_TAKEN), "a JPEG, cleaned of where it was taken");
    assert_eq!(state.db.poster_bytes_of(&name), kept.len() as i64, "its size is in the video's row");
    let day = state.db.upload_count(ANN, Span::Day, crate::relay::storage::now_millis() as i64);
    assert_eq!(day, (video_bytes().len() + kept.len()) as i64, "the day's count holds the video and its picture");

    let mut big = jpeg(64, 36);
    big.resize(POSTER_MAX_BYTES + 10, 0);
    let refusals: Vec<(&str, Vec<Part>, UploadQuery, StatusCode, Option<&str>)> = vec![
        ("a PNG as the poster picture", with_poster("a.mp4", png()), UploadQuery::default(), StatusCode::UNSUPPORTED_MEDIA_TYPE, Some(NOT_A_JPEG)),
        ("a poster picture over 128 KB", with_poster("b.mp4", big), UploadQuery::default(), StatusCode::PAYLOAD_TOO_LARGE, None),
        ("a poster picture over 640 pixels", with_poster("c.mp4", jpeg(800, 450)), UploadQuery::default(), StatusCode::BAD_REQUEST, None),
        (
            "two poster pictures",
            vec![Part(POSTER_FIELD, "p.jpg", "image/jpeg", jpeg(64, 36)), Part(POSTER_FIELD, "q.jpg", "image/jpeg", jpeg(64, 36)), Part("file", "d.mp4", "video/mp4", video_bytes())],
            UploadQuery::default(),
            StatusCode::BAD_REQUEST,
            Some(TWO_POSTERS),
        ),
        (
            "a poster picture with a document",
            vec![Part(POSTER_FIELD, "p.jpg", "image/jpeg", jpeg(64, 36)), Part("file", "notes.pdf", "application/pdf", b"%PDF-1.7 notes".to_vec())],
            UploadQuery::default(),
            StatusCode::BAD_REQUEST,
            Some(NOT_A_VIDEO),
        ),
        (
            "a poster picture with a file from a private conversation",
            vec![Part(POSTER_FIELD, "p.jpg", "image/jpeg", jpeg(64, 36)), Part("file", "attachment.enc", "application/octet-stream", vec![9u8; 512])],
            UploadQuery { encrypted: Some(1), ..Default::default() },
            StatusCode::BAD_REQUEST,
            Some(PRIVATE_REFUSED),
        ),
    ];
    for (what, parts, query, code, words) in refusals {
        let refused = send(&state, &d, "tok_ann", parts, query).await.map(|a| a.to_string());
        let refused = refused.expect_err(&format!("{what} is refused"));
        assert_eq!(refused.0, code, "{what}: {}", refused.1);
        if let Some(words) = words {
            assert_eq!(refused.1, words, "{what}: said why");
        }
    }
    assert_eq!(Dirs::names(&d.uploads), vec![name.clone(), poster_name(&name)], "nothing of a refused upload was stored");
    assert_eq!(Dirs::names(&d.part), Vec::<String>::new(), "and no part file was left");

    let plain = send(&state, &d, "tok_ann", vec![Part("file", "old_way.mp4", "video/mp4", video_bytes())], UploadQuery::default()).await.expect("a video without one");
    assert!(plain.get("poster").is_none(), "no poster picture named: {plain}");
    assert_eq!(state.db.poster_bytes_of(&stored(&plain)), 0);
    assert!(!d.uploads.join(poster_name(&stored(&plain))).exists());
}

// ── Every road a video leaves by ──

/// Store a chat line by `from` at `timestamp` in #global, as the chat arm stores one.
fn post(state: &RelayState, from: &str, content: &str, timestamp: u64) -> i64 {
    let chat = RelayMessage::Chat {
        from: from.into(),
        from_name: Some(from.into()),
        content: content.into(),
        timestamp,
        signature: Some(format!("sig-{timestamp}")),
        channel: "global".into(),
        reply_to: None,
        thread_count: None,
        message_id: None,
    };
    state.db.store_message_in_channel(&chat, "global").unwrap()
}

/// Whether `name` and its poster picture are each in `dir`.
fn both_in(dir: &std::path::Path, name: &str) -> (bool, bool) {
    (dir.join(name).is_file(), dir.join(poster_name(name)).is_file())
}

/// DELETING A MESSAGE DELETES THE VIDEO IT POSTED, AND THE VIDEO'S POSTER PICTURE. Ann's message
/// with her video is deleted by her (the chat's own Delete): the video, its picture and its row
/// go. Not deleted with it: her video another message of hers still links, her Shared File, and
/// Bob's video her message linked. A moderator's delete by row number takes the files the same way
/// (they are the author's). The keep-N limit's prune takes a video's picture with it too.
///
/// Seen red 2026-11-06 with `delete_files_of_deleted_message_in` taken out of
/// `delete_own_message_in`: "Ann's deleted message took its video and picture / left: (true, true)
/// / right: (false, false)"; with the `NOT EXISTS` clause taken out of
/// `forget_chat_files_of_deleted_message`: "its row went too (twice and shared stay) / left: 1 /
/// right: 2" (the video another message still linked went with it).
#[tokio::test]
async fn deleting_a_message_deletes_the_video_it_posted_and_its_poster_picture() {
    let state = relay("poster_message_delete");
    let d = dirs("poster_message_delete");
    uploader(&state, ANN, "tok_ann", 50).await;
    uploader(&state, BOB, "tok_bob", 50).await;
    let up = |token: &'static str, name: &'static str, q: UploadQuery| {
        let (state, d) = (&state, &d);
        async move { stored(&send(state, d, token, with_poster(name, jpeg(320, 180)), q).await.expect("stored")) }
    };
    let gone = up("tok_ann", "gone.mp4", UploadQuery::default()).await;
    let twice = up("tok_ann", "twice.mp4", UploadQuery::default()).await;
    let shared = up("tok_ann", "shared.mp4", UploadQuery { share: Some(1), ..Default::default() }).await;
    let bobs = up("tok_bob", "bobs.mp4", UploadQuery::default()).await;
    let stamp = 1_791_481_200_000u64;
    post(&state, ANN, &format!("look /uploads/{gone} and /uploads/{twice} and /uploads/{shared} and Bob's /uploads/{bobs}."), stamp);
    post(&state, ANN, &format!("again: https://example.org/uploads/{twice}"), stamp + 1);

    assert!(crate::relay::handlers::federation_log::delete_own_message_in(&state, ANN, stamp, d.folders()).await.unwrap(), "the message is deleted");
    assert_eq!(both_in(&d.uploads, &gone), (false, false), "Ann's deleted message took its video and picture");
    assert_eq!(state.db.poster_bytes_of(&gone), 0);
    assert_eq!(state.db.get_upload_count(ANN).unwrap(), 2, "its row went too (twice and shared stay)");
    assert_eq!(both_in(&d.uploads, &twice), (true, true), "a video another message still links stays");
    assert_eq!(both_in(&d.uploads, &shared), (true, true), "a Shared File stays");
    assert_eq!(both_in(&d.uploads, &bobs), (true, true), "someone else's video stays");

    // A moderator deletes the other message by its row number: the video it alone links now goes.
    state.db.set_role(MOD, "mod").unwrap();
    let id: i64 = state.db.with_conn(|c| c.query_row("SELECT id FROM messages WHERE timestamp = ?1", rusqlite::params![(stamp + 1) as i64], |r| r.get(0))).unwrap();
    let by_id = crate::relay::handlers::federation_log::delete_message_by_id_in(&state, id, MOD, true, d.folders()).await.unwrap();
    assert!(by_id.is_some(), "the moderator's delete went");
    assert_eq!(both_in(&d.uploads, &twice), (false, false), "a moderator's delete takes the author's video and picture");

    // The keep-N limit: Bob keeps one chat file, so his next video pushes his first out, picture too.
    let mut role = state.db.role_def("verified");
    role.max_uploads_kept = 1;
    state.db.upsert_role(&role).unwrap();
    let next = stored(&send(&state, &d, "tok_bob", with_poster("next.mp4", jpeg(64, 36)), UploadQuery::default()).await.unwrap());
    assert_eq!(both_in(&d.uploads, &bobs), (false, false), "the keep-N prune took the video and its picture");
    assert_eq!(both_in(&d.uploads, &next), (true, true));
}

/// THE RETENTION SWEEP AND AN ADMIN'S CHANNEL CLEAR TAKE THE FILES THEIR MESSAGES POSTED (BUG-256,
/// 2026-11-06). Ann's 40-day-old message posted two videos; the retention is lowered to 30 days and
/// the expiry pass runs: the message goes, and with it the video only it linked, its poster picture
/// and its row. Her other video, which a recent message still links, stays, and so does Bob's.
/// Then an admin clears #global: the files the cleared messages linked go too. And clearing every
/// channel takes the rest.
///
/// Seen red 2026-11-06 with the file step taken out of `expire_messages_in`: "the sweep took the
/// video only the expired message linked, and its picture / left: (true, true) / right: (false,
/// false)"; with it taken out of `wipe_channel_messages_in`: "the clear took Bob's video and its
/// picture / left: (true, true)".
#[tokio::test]
async fn the_retention_sweep_and_a_channel_clear_take_the_files_their_messages_posted() {
    let state = relay("poster_sweep_clear");
    let d = dirs("poster_sweep_clear");
    uploader(&state, ANN, "tok_ann", 50).await;
    uploader(&state, BOB, "tok_bob", 50).await;
    let up = |token: &'static str, name: &'static str| {
        let (state, d) = (&state, &d);
        async move { stored(&send(state, d, token, with_poster(name, jpeg(320, 180)), UploadQuery::default()).await.expect("stored")) }
    };
    let old = up("tok_ann", "old.mp4").await;
    let kept = up("tok_ann", "kept.mp4").await;
    let bobs = up("tok_bob", "bobs.mp4").await;
    let now = crate::relay::storage::now_millis();
    post(&state, ANN, &format!("from last month: /uploads/{old} and /uploads/{kept}"), now - 40 * 86_400_000);
    post(&state, ANN, &format!("still here: /uploads/{kept}"), now - 60_000);
    post(&state, BOB, &format!("mine: /uploads/{bobs}"), now - 30_000);

    let mut s = state.db.get_server_settings().unwrap();
    s.message_retention_days = 30;
    state.db.set_server_settings(&s, "admin").unwrap();
    state.db.run_expiry_sweeps_in(d.folders());
    assert_eq!(state.db.message_count().unwrap(), 2, "the 40-day-old message expired");
    assert_eq!(both_in(&d.uploads, &old), (false, false), "the sweep took the video only the expired message linked, and its picture");
    assert_eq!(state.db.get_upload_count(ANN).unwrap(), 1, "and its row");
    assert_eq!(both_in(&d.uploads, &kept), (true, true), "a video a remaining message links stays");
    assert_eq!(both_in(&d.uploads, &bobs), (true, true), "and so does someone else's");

    assert_eq!(state.db.wipe_channel_messages_in("global", d.folders()).unwrap(), 2, "the admin clears #global");
    assert_eq!(both_in(&d.uploads, &bobs), (false, false), "the clear took Bob's video and its picture");
    assert_eq!(both_in(&d.uploads, &kept), (false, false), "and Ann's, now no message links it");
    assert_eq!((state.db.get_upload_count(ANN).unwrap(), state.db.get_upload_count(BOB).unwrap()), (0, 0), "their rows went too");

    // Clearing every channel takes what the cleared messages linked in every room.
    let last = up("tok_bob", "last.mp4").await;
    post(&state, BOB, &format!("again /uploads/{last}"), now);
    assert_eq!(state.db.wipe_messages_in(d.folders()).unwrap(), 1);
    assert_eq!(both_in(&d.uploads, &last), (false, false), "clearing every channel took it too");
}

/// AN ERASED ACCOUNT, AN UNPUBLISHED MEDIA UPLOAD'S EXPIRY AND A LONE PICTURE: each video goes with
/// its poster picture. Erasing Ann's account deletes her video and its picture. Bob's Media upload
/// (`media=1`, waiting to be published) that waits past a day is deleted by the expiry pass, its
/// picture with it; and a poster picture left without its video (a relay stopped between the two)
/// is swept by the same pass, while a video's own picture beside it stays.
///
/// Seen red 2026-11-06 with `Folders::delete`'s poster loop taken out: "the erase took her video's
/// picture too / left: (false, true) / right: (false, false)"; with `sweep_lone_posters` left out
/// of the expiry pass: "a picture without its video is swept".
#[tokio::test]
async fn an_erase_an_expiry_and_a_lone_picture_each_take_the_poster_picture() {
    let state = relay("poster_erase_expiry");
    let d = dirs("poster_erase_expiry");
    uploader(&state, ANN, "tok_ann", 50).await;
    uploader(&state, BOB, "tok_bob", 50).await;
    let anns = stored(&send(&state, &d, "tok_ann", with_poster("mine.mp4", jpeg(320, 180)), UploadQuery::default()).await.unwrap());
    let waiting = stored(&send(&state, &d, "tok_bob", with_poster("later.mp4", jpeg(320, 180)), UploadQuery { media: Some(1), ..Default::default() }).await.unwrap());
    assert_eq!(both_in(&d.waiting, &waiting), (true, true), "a Media upload waits with its picture, out of the public folder");
    assert_eq!(both_in(&d.uploads, &waiting), (false, false));

    state.db.delete_account_in(ANN, ANN, d.folders());
    assert_eq!(both_in(&d.uploads, &anns), (false, false), "the erase took her video's picture too");

    let day_after = crate::relay::storage::now_millis() as i64 + 2 * 24 * 60 * 60 * 1000;
    assert_eq!(state.db.expire_unpublished_media_in(d.folders(), day_after).unwrap(), 1);
    assert_eq!(both_in(&d.waiting, &waiting), (false, false), "the expiry took the waiting upload and its picture");

    let keeper = stored(&send(&state, &d, "tok_bob", with_poster("keeper.mp4", jpeg(64, 36)), UploadQuery::default()).await.unwrap());
    std::fs::write(d.uploads.join(poster_name("1760000000000_left_behind.mp4")), jpeg(64, 36)).unwrap();
    state.db.run_expiry_sweeps_in(d.folders());
    assert!(!d.uploads.join(poster_name("1760000000000_left_behind.mp4")).exists(), "a picture without its video is swept");
    assert_eq!(both_in(&d.uploads, &keeper), (true, true), "a video's own picture stays");
}

/// `key` publishes the Media upload `file` (already stored) with `visibility`; its item number.
fn publish(state: &RelayState, d: &Dirs, key: &str, file: &str, visibility: &str) -> i64 {
    let s = state.db.get_server_settings().unwrap();
    let body = PublishBody { key: key.into(), filename: file.into(), title: "The meetup".into(), visibility: Some(visibility.into()), ..Default::default() };
    publish_core(&state.db, &s, labels(), d.folders(), &body).expect("published").0["item"]["id"].as_i64().unwrap()
}

/// GET `path` from the `/uploads` service the relay mounts (relay/mod.rs `nest_service`), over
/// `d.uploads`, on a loopback port: the status and the content type.
async fn fetch_public(d: &Dirs, path: &str) -> (u16, String) {
    let app = axum::Router::new().nest_service("/uploads", tower_http::services::ServeDir::new(d.uploads.to_path_buf()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let res = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
    let ty = res.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let status = res.status().as_u16();
    server.abort();
    (status, ty)
}

/// A MEMBERS-ONLY VIDEO'S POSTER PICTURE IS NEVER MORE PUBLIC THAN THE VIDEO. Published public,
/// the picture is served at its address beside the video (a JPEG). Made members only, the video
/// moves to the members' folder and its picture with it: the public address answers 404 to
/// everyone, a non-member included. Back to public, both come back. Removing the item (uploaded
/// for Media, so its file goes) deletes the picture too, and a copyright report decided Removed
/// does the same to another item.
///
/// Seen red 2026-11-06 with `Folders::move_to`'s poster move taken out: "published public, its
/// picture is served beside it / left: (404, \"\") / right: (200, \"image/jpeg\")" (publishing moves
/// the file out of the waiting folder, and the picture stayed behind, so the same move is what
/// takes it off the public address when the item is made members only); with `Folders::delete`'s
/// poster loop taken out: "removing the item took its picture / left: ((false, true), (false,
/// false))".
#[tokio::test]
async fn a_members_only_videos_poster_picture_is_never_more_public_than_the_video() {
    let state = relay("poster_members_only");
    let d = dirs("poster_members_only");
    uploader(&state, ANN, "tok_ann", 50).await;
    let name = stored(&send(&state, &d, "tok_ann", with_poster("meetup.mp4", jpeg(320, 180)), UploadQuery { media: Some(1), ..Default::default() }).await.unwrap());
    let id = publish(&state, &d, ANN, &name, "public");
    let picture = format!("/uploads/{}", poster_name(&name));
    assert_eq!(fetch_public(&d, &picture).await, (200, "image/jpeg".to_string()), "published public, its picture is served beside it");

    let _ = visibility_core(&state.db, d.folders(), id, ANN, "members").expect("made members only");
    assert_eq!(both_in(&d.members, &name), (true, true), "the video and its picture moved to the members' folder");
    assert_eq!(fetch_public(&d, &picture).await.0, 404, "made members only, its picture is refused at the public address");
    assert_eq!(fetch_public(&d, &format!("/uploads/{name}")).await.0, 404, "as the video is");

    let _ = visibility_core(&state.db, d.folders(), id, ANN, "public").expect("public again");
    assert_eq!(fetch_public(&d, &picture).await.0, 200, "public again, its picture is served again");

    let _ = remove_core(&state.db, d.folders(), &RemoveTarget::Item(id), ANN).expect("removed");
    assert_eq!((both_in(&d.uploads, &name), both_in(&d.members, &name)), ((false, false), (false, false)), "removing the item took its picture");
    assert_eq!(state.db.media_bytes(Some(ANN)), 0, "and nothing of it counts in her share");

    // A copyright report decided Removed takes another item's file and picture the same way.
    let other = stored(&send(&state, &d, "tok_ann", with_poster("other.mp4", jpeg(320, 180)), UploadQuery { media: Some(1), ..Default::default() }).await.unwrap());
    let other_id = publish(&state, &d, ANN, &other, "public");
    assert!(state.db.media_bytes(Some(ANN)) > video_bytes().len() as i64, "her share counts the video and its picture");
    state.db.set_role(MOD, "admin").unwrap();
    let form = serde_json::json!({ "item": other_id, "name": "Rosa Painter", "contact": "rosa@example.org", "statement": "I filmed this.", "good_faith": true }).to_string();
    let mut h = axum::http::HeaderMap::new();
    h.insert("x-real-ip", axum::http::HeaderValue::from_static("203.0.113.7"));
    let _ = crate::relay::files_takedown::takedown_core(&state, &h, &form, crate::relay::storage::now_millis() as i64).await.expect("reported");
    let report: i64 = state.db.with_conn(|c| c.query_row("SELECT MAX(id) FROM takedown_requests", [], |r| r.get(0))).unwrap();
    let _ = crate::relay::files_takedown::decide_core(&state.db, d.folders(), MOD, report, crate::relay::files_takedown::Outcome::Removed, "", crate::relay::storage::now_millis() as i64)
        .expect("decided");
    assert_eq!(both_in(&d.uploads, &other), (false, false), "a decided copyright report took the video and its picture");
}
