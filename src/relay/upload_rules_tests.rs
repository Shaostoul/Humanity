//! The upload rules both routes share (upload_rules.rs), the increment 2 follow-ups (2026-10-11):
//! a private file's own limit (B2), a Media file's (B1), the day's and the hour's allowance (B3),
//! `application/octet-stream` judged by its extension (R7), and HEIC and AVIF photos cleaned in
//! place and kept as plain files (the HEIF strip, 2026-10-11; the tests at the end). The single
//! request is driven through the real `upload_file` with a request that says its size up front,
//! so every refusal here comes before a byte is read and nothing is written anywhere; a HEIC that
//! is stored goes through the resumable route, which takes folders of the test's own. Each test
//! names the break it was seen failing against ("Seen red").

use super::*;
use crate::relay::api::{upload_file, UploadQuery};
use crate::relay::storage::upload_counts::Span;
use axum::extract::{FromRequest, Query, State};
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

/// `POST /api/upload` with `query`, saying up front that it carries `declared` bytes.
async fn upload(state: &Arc<RelayState>, query: UploadQuery, declared: u64) -> Result<axum::Json<serde_json::Value>, Refused> {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header(axum::http::header::CONTENT_TYPE, "multipart/form-data; boundary=X")
        .header(axum::http::header::CONTENT_LENGTH, declared.to_string())
        .body(axum::body::Body::from("--X--\r\n"))
        .unwrap();
    let headers = req.headers().clone();
    let multipart = axum::extract::Multipart::from_request(req, &()).await.expect("a multipart request");
    upload_file(State(state.clone()), Query(query), headers, multipart).await
}

/// B2. A FILE IN A PRIVATE CONVERSATION IS HELD TO THE ROLE'S PRIVATE LIMIT, not `max_upload_mb`:
/// with 500 MB for room files and 6 MB for private ones, a 10 MB encrypted upload is refused up
/// front with words naming the private limit, and the encryption's few bytes on top of 6 MB are
/// allowed. The peer list's number is the role's.
///
/// Seen red 2026-10-11 with `for_request` ignoring `encrypted` (the role's limit for every file):
/// "a 10 MB private file is refused up front: Err((400, \"No file provided.\"))".
#[tokio::test]
async fn a_private_file_is_held_to_the_roles_private_limit() {
    let state = relay("rules_b2");
    uploader(&state, "ann", "tok", |r| {
        r.max_upload_mb = 500;
        r.max_private_file_mb = 6;
    })
    .await;
    let private = || UploadQuery { token: Some("tok".into()), encrypted: Some(1), ..Default::default() };
    let refused = upload(&state, private(), 10 * MIB).await;
    match refused {
        Err((code, words)) if code == StatusCode::PAYLOAD_TOO_LARGE => {
            assert_eq!(words, "Files in private conversations are limited to 6 MB for your role on this server.");
        }
        other => panic!("a 10 MB private file is refused up front: {other:?}"),
    }
    let role = state.db.role_def("verified");
    let limit = FileLimit::for_request(&role, 0, true);
    assert_eq!(limit.bytes, 6 * MIB + PRIVATE_ENVELOPE_SLACK, "the encryption's few bytes are allowed on top");
    assert_eq!(private_limit_mb(&role), 6, "what the peer list says");
    let room = FileLimit::for_request(&role, 0, false);
    assert_eq!(room.bytes, 500 * MIB, "a room file keeps the role's own limit");
}

/// B1. A MEDIA FILE IN ONE REQUEST IS HELD TO THE ROLE'S MEDIA LIMIT TOO, when it has one: with
/// `max_media_file_mb` 2, a 3 MB `media=1` request is refused up front with the Media words; a
/// chat file of the same size is not held to it; 0 leaves the role's own limit.
///
/// Seen red 2026-10-11 with the Media branch taken out of `for_request`: "a 3 MB Media file over
/// the role's 2 MB Media limit is refused: Err((400, \"No file provided.\"))".
#[tokio::test]
async fn a_media_file_in_one_request_is_held_to_the_roles_media_limit() {
    let state = relay("rules_b1");
    uploader(&state, "ann", "tok", |r| {
        r.max_upload_mb = 25;
        r.max_media_file_mb = 2;
    })
    .await;
    let media = UploadQuery { token: Some("tok".into()), media: Some(1), ..Default::default() };
    match upload(&state, media, 3 * MIB).await {
        Err((code, words)) if code == StatusCode::PAYLOAD_TOO_LARGE => assert_eq!(words, "Files on the Files page are limited to 2 MB for your role on this server."),
        other => panic!("a 3 MB Media file over the role's 2 MB Media limit is refused: {other:?}"),
    }
    let role = state.db.role_def("verified");
    assert_eq!(FileLimit::for_request(&role, 0, false).bytes, 25 * MIB, "a chat file is not held to it");
    let mut none = role.clone();
    none.max_media_file_mb = 0;
    assert_eq!(FileLimit::for_request(&none, crate::relay::storage::files::MEDIA_SHELF, false).bytes, 25 * MIB, "0: the role's own limit");
}

/// B3. THE DAY'S AND THE HOUR'S ALLOWANCE IN ONE REQUEST: with 10 MB a day and 3 MB uploaded today,
/// an 8 MB request is refused up front (429) with words naming the limit and the reset; with the
/// hour's two uploads used, any request is refused until the hour turns; Admin's 0 is no limit.
/// The counts are fixed windows: midnight UTC and the top of the hour.
///
/// Seen red 2026-10-11 with `day_limit` giving no limit: "an 8 MB request past the day's allowance
/// is refused: Err((400, \"No file provided.\"))".
#[tokio::test]
async fn the_days_and_the_hours_allowance_hold_in_one_request() {
    let state = relay("rules_b3");
    uploader(&state, "ann", "tok", |r| {
        r.max_upload_mb = 25;
        r.max_upload_mb_per_day = 10;
        r.max_uploads_per_hour = 2;
    })
    .await;
    let now = crate::relay::storage::now_millis() as i64;
    count_upload(&state.db, "ann", now, 3 * MIB, true);
    let room = || UploadQuery { token: Some("tok".into()), ..Default::default() };
    match upload(&state, room(), 8 * MIB).await {
        Err((code, words)) if code == StatusCode::TOO_MANY_REQUESTS => {
            assert!(words.starts_with("Your role may upload 10 MB a day on this server, and you have uploaded 3.0 MB today, and this file would pass that."), "{words}");
            assert!(words.contains("The allowance resets at midnight UTC, in "), "{words}");
        }
        other => panic!("an 8 MB request past the day's allowance is refused: {other:?}"),
    }
    count_upload(&state.db, "ann", now, 0, true);
    match upload(&state, room(), MIB).await {
        Err((code, words)) if code == StatusCode::TOO_MANY_REQUESTS => {
            assert!(words.contains("2 uploads an hour") && words.contains("at the top of the hour"), "{words}");
        }
        other => panic!("a third upload in the hour is refused: {other:?}"),
    }
    let admin = state.db.role_def("admin");
    assert_eq!((admin.max_upload_mb_per_day, admin.max_uploads_per_hour, admin.max_private_held_mb), (0, 0, 0), "Admin: no limits");
    assert_eq!(allowance_at_start(&state.db, "ann", &admin, now), Ok(None));
    assert_eq!(Span::Day.starts_at(now) % (24 * 3_600_000), 0);
    assert_eq!(Span::Hour.resets_at(now) - Span::Hour.starts_at(now), 3_600_000);
    assert_eq!(in_words(0, 5 * 3_600_000 + 12 * 60_000), "in 5 hours 12 minutes");
    assert_eq!(in_words(0, 60_000), "in 1 minute");
    assert_eq!(in_words(0, 0), "in less than a minute");
}

/// R7 (a). `application/octet-stream` IS JUDGED BY ITS EXTENSION ALONE: a camera's `.tiff` (which
/// carries GPS in its EXIF, and which nothing here can clean) sent that way is refused (it used to
/// pass with any extension that was not blocked, and skip every strip); a `.jpg`, a `.blend`, and
/// since the HEIF strip (2026-10-11) an iPhone's `.heic`, sent that way still go.
///
/// Seen red 2026-10-11 with the type check back as merged (`octet-stream` an allowed type on its
/// own): "a .heic sent as octet-stream is refused". Seen red again, as rewritten for the HEIF
/// strip, with the same break: "a .tiff sent as octet-stream is refused".
#[test]
fn octet_stream_is_judged_by_its_extension() {
    let tiff = check_type("DSC_0001.tiff", "application/octet-stream", false);
    assert!(tiff.is_err(), "a .tiff sent as octet-stream is refused");
    assert!(tiff.unwrap_err().1.starts_with("Unsupported file type"));
    assert!(check_type("photo.jpg", "application/octet-stream", false).is_ok());
    assert!(check_type("case.blend", "application/octet-stream", false).is_ok());
    assert!(check_type("IMG_0001.heic", "application/octet-stream", false).is_ok(), "a .heic goes, to the HEIF strip");
    assert!(check_type("blob.enc", "application/octet-stream", true).is_ok(), "an encrypted file is never judged");
}

// ── HEIC and AVIF photos (2026-10-11): cleaned in place, kept as plain files ──

use crate::relay::core::strip_heif_metadata::build::{
    bx, exif_bytes, filler, ftyp_avif, ftyp_heic, heif, iphone_heic, Built, Item, Kind, Place, Tables, PLACE,
};

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// `bytes` with exactly `ranges` zeroed.
fn zeroed(bytes: &[u8], ranges: &[std::ops::Range<usize>]) -> Vec<u8> {
    let mut want = bytes.to_vec();
    for r in ranges {
        want[r.clone()].fill(0);
    }
    want
}

/// A HEIC whose Exif item's location in the table points 10,000 bytes past the end of the file,
/// so the strip cannot find the Exif with certainty.
fn unreadable_heic() -> Built {
    let items = [
        Item { id: 1, kind: Kind::Picture(b"hvc1"), data: filler(200, 1), place: Place::Mdat },
        Item { id: 3, kind: Kind::Exif, data: exif_bytes(), place: Place::Mdat },
    ];
    heif(ftyp_heic(), &Tables { skew: 10_000, ..Tables::default() }, &items)
}

/// A HEIC OR AVIF PHOTO IS A PLAIN FILE, NEVER A PICTURE: by its declared type (`image/heic`,
/// `image/heif`, `image/avif`) or, when the declared type says nothing else, its extension, it is
/// not sent to the picture path (whose magic check would refuse it, and whose strip cannot read
/// it), it is stored as `.heic`, `.heif` or `.avif`, and the answer calls it a "document". A
/// `.heic` declared as a JPEG is judged as a JPEG.
///
/// Seen red 2026-10-11 with `picture` not asking `heif_photo` first: "IMG_0001.heic as image/heic
/// is not a picture / left: Some(\"image/unknown\") / right: None".
#[test]
fn a_heic_or_avif_photo_is_a_plain_file_never_a_picture() {
    for (name, ct, ext) in [
        ("IMG_0001.heic", "image/heic", "heic"),
        ("IMG_0002.HEIC", "application/octet-stream", "heic"),
        ("IMG_0003.heif", "image/heif", "heif"),
        ("pic.avif", "image/avif", "avif"),
        ("pic.avif", "image/x-unknown", "avif"),
        ("photo.jpg", "image/heic", "heic"),
    ] {
        let kind = check_type(name, ct, false).unwrap_or_else(|e| panic!("{name} as {ct} is allowed: {e:?}"));
        assert_eq!(kind.picture(ct, false), None, "{name} as {ct} is not a picture");
        assert_eq!(kind.stored_ext(ct, false), ext, "{name} as {ct}");
        assert_eq!(file_type(ct, ext, false), "document", "{name} as {ct}");
    }
    let jpeg_named_heic = check_type("IMG_0004.heic", "image/jpeg", false).unwrap();
    assert_eq!(jpeg_named_heic.picture("image/jpeg", false), Some("image/jpeg"), "a .heic declared as a JPEG is judged as one");
    assert!(picture_is_what_it_says("image/jpeg", &iphone_heic().bytes).is_err(), "and refused, since it is not");
}

/// THE STRIP BOTH ROUTES CALL (`finish_file_as`) CLEANS A HEIC IN PLACE: its Exif item zeroed,
/// every other byte as it was, the size unchanged. A HEIC whose Exif cannot be found with
/// certainty is refused with the words that predate the strip, and taken as sent with
/// `keep_metadata`. A file in the MP4 family with no movie and no HEIF brand is a video cut off
/// before its movie box, and is now told so (`VIDEO_UNREADABLE`) instead of being called a photo.
///
/// Seen red 2026-10-11 against `finish_file_as` as it was before the HEIF strip (a file with no
/// movie refused at once): "a well-formed HEIC is cleaned and kept: Err((422, \"HEIC and AVIF
/// photos cannot be cleaned of where they were taken yet. Save it as a JPEG and send that.\"))".
/// Seen red again with only the no-HEIF-brand branch answering `HEIF_REFUSED` as before: "a video
/// cut off before its movie box is told so / left: Err(\"HEIC and AVIF photos cannot be cleaned
/// ...\")".
#[test]
fn finish_file_as_cleans_a_heic_and_refuses_what_it_cannot_read() {
    let photo = iphone_heic();
    let path = crate::test_temp::file("rules_heic_finish", "heic");
    std::fs::write(&path, &photo.bytes).unwrap();
    let cleaned = finish_file_as(&path, false);
    assert!(cleaned.is_ok(), "a well-formed HEIC is cleaned and kept: {cleaned:?}");
    let after = std::fs::read(&path).unwrap();
    assert!(!contains(&after, PLACE), "where it was taken is gone");
    assert_eq!(after, zeroed(&photo.bytes, photo.ranges_of(3)), "only the Exif item's bytes changed");

    let broken = unreadable_heic().bytes;
    std::fs::write(&path, &broken).unwrap();
    assert_eq!(finish_file_as(&path, false), Err((StatusCode::UNPROCESSABLE_ENTITY, HEIF_REFUSED.to_string())));
    assert_eq!(std::fs::read(&path).unwrap(), broken, "nothing written to a photo it could not read");
    assert_eq!(finish_file_as(&path, true), Ok(()), "kept by choice");

    let cut_video = [bx(b"ftyp", b"isom\0\0\x02\0isomiso2mp41"), bx(b"mdat", &filler(300, 4))].concat();
    std::fs::write(&path, &cut_video).unwrap();
    assert_eq!(
        finish_file_as(&path, false).map_err(|e| e.1),
        Err(VIDEO_UNREADABLE.to_string()),
        "a video cut off before its movie box is told so"
    );
}

/// A HEIC UPLOAD IS STORED WITHOUT WHERE IT WAS TAKEN, instead of refused: sent through the
/// resumable upload route (`start_core` and `piece_core`, with folders of the test's own) as
/// `image/heic`, as `application/octet-stream`, and as an AVIF, each photo is stored as a plain
/// file (`.heic`, `.avif`; the answer's type "document") of the same size, its Exif zeroed and
/// every other byte, the picture's included, as it arrived. A HEIC whose Exif cannot be found
/// with certainty is still refused with the old words (its session goes), and stored as sent with
/// `keep_metadata`.
///
/// The single request (`upload_file`) runs the same steps (`check_type`, `picture`,
/// `finish_file_as`, `stored_ext`, `file_type`, each proven above), but it writes into the relay's
/// live data folder, which a test must not write to; the resumable route takes the test's own
/// folders, so the whole way from the request to the stored file is driven here.
///
/// Seen red 2026-10-11 against `finish_file_as` as it was before the HEIF strip (a file with no
/// movie refused at once): "IMG_0001.heic as image/heic is stored: Words(422, \"HEIC and AVIF
/// photos cannot be cleaned of where they were taken yet. Save it as a JPEG and send that.\")".
#[tokio::test]
async fn a_heic_upload_is_stored_without_where_it_was_taken() {
    use crate::relay::resumable::{piece_core, start_core, StartBody};
    use crate::relay::storage::files::Folders;
    let state = relay("rules_heic_upload");
    uploader(&state, "ann", "tok", |_| {}).await;
    let (uploads, members, resume, waiting) = (
        crate::test_temp::dir("rules_heic_uploads"),
        crate::test_temp::dir("rules_heic_members"),
        crate::test_temp::dir("rules_heic_resume"),
        crate::test_temp::dir("rules_heic_waiting"),
    );
    let folders = || Folders { uploads: &uploads, members: &members, resume: &resume, waiting: &waiting };
    let send = |name: &str, ct: &str, bytes: &[u8], keep: bool| {
        let body = StartBody {
            filename: name.into(),
            size: bytes.len() as u64,
            content_type: Some(ct.into()),
            media: Some(1),
            blake3: None,
            keep_metadata: keep.then_some(1),
        };
        let (state, bytes) = (state.clone(), axum::body::Bytes::copy_from_slice(bytes));
        async move {
            let (_, started) = start_core(&state, folders(), Some("tok"), body).await?;
            let id = started.0["id"].as_str().unwrap().to_string();
            piece_core(&state, folders(), &id, Some("tok"), Some(0), bytes).await.map(|j| j.0)
        }
    };

    let heic = iphone_heic();
    let avif = heif(
        ftyp_avif(),
        &Tables::default(),
        &[
            Item { id: 1, kind: Kind::Picture(b"av01"), data: filler(250, 5), place: Place::Mdat },
            Item { id: 2, kind: Kind::Exif, data: exif_bytes(), place: Place::Mdat },
        ],
    );
    for (name, ct, photo, exif_id, ext) in [
        ("IMG_0001.heic", "image/heic", &heic, 3, ".heic"),
        ("IMG_0002.HEIC", "application/octet-stream", &heic, 3, ".heic"),
        ("PXL_0003.avif", "image/avif", &avif, 2, ".avif"),
    ] {
        let done = send(name, ct, &photo.bytes, false).await.unwrap_or_else(|e| panic!("{name} as {ct} is stored: {e:?}"));
        let stored_name = done["filename"].as_str().unwrap();
        assert!(stored_name.ends_with(ext), "{name}: kept as a plain {ext} file: {stored_name}");
        assert_eq!(done["type"], "document", "{name}");
        assert_eq!(done["size"].as_u64(), Some(photo.bytes.len() as u64), "{name}: the same size");
        let stored = std::fs::read(waiting.join(stored_name)).unwrap();
        assert!(!contains(&stored, PLACE), "{name}: where it was taken is gone");
        assert_eq!(stored, zeroed(&photo.bytes, photo.ranges_of(exif_id)), "{name}: every other byte as it arrived");
        assert_eq!(stored[photo.ranges_of(1)[0].clone()], photo.bytes[photo.ranges_of(1)[0].clone()], "{name}: the picture itself untouched");
    }

    let broken = unreadable_heic().bytes;
    match send("IMG_0004.heic", "image/heic", &broken, false).await {
        Err(refused) => assert_eq!((refused.status(), refused.words()), (StatusCode::UNPROCESSABLE_ENTITY, HEIF_REFUSED.to_string())),
        Ok(done) => panic!("a HEIC whose Exif cannot be found is refused: stored as {done}"),
    }
    assert!(state.db.all_upload_sessions().unwrap().iter().all(|s| s.finished()), "its session goes");
    let done = send("IMG_0004.heic", "image/heic", &broken, true).await.expect("kept by choice, it is stored as sent");
    assert_eq!(std::fs::read(waiting.join(done["filename"].as_str().unwrap())).unwrap(), broken);
}

/// THE STORED NAME KEEPS ITS WORDS AND ITS EXTENSION (2026-10-17). A member's `WH40K Marriah
/// Carey.mp4` was stored as `<ms>_WH40KMarriahCareymp4.mp4`, because the whole name, extension
/// included, went through the filter. Now the stem alone does: spaces and dots become `_`, the
/// folder and the extension (both parts of a `.tar.gz`) go, and nothing that could be a hidden
/// file, a folder escape or a space ever stays.
///
/// Seen red 2026-10-17 against the old filter: `WH40K Marriah Carey.mp4` gave `WH40KMarriahCareymp4`.
#[test]
fn the_stored_name_keeps_its_words_and_its_extension() {
    for (given, stem) in [
        ("WH40K Marriah Carey.mp4", "WH40K_Marriah_Carey"),
        ("clip.mp4", "clip"),
        ("backup.2026.10.tar.gz", "backup_2026_10"),
        ("Backup.TAR.GZ", "Backup"),
        ("a.b.c.png", "a_b_c"),
        ("README", "README"),
        (".hidden", "file"),
        ("..", "file"),
        ("../../etc/passwd", "passwd"),
        (r"C:\Users\x\My Photo.jpg", "My_Photo"),
        ("日本の写真.jpg", "file"),
        ("Café au lait.png", "Caf_au_lait"),
        ("  -_spaced out_-  .txt", "spaced_out"),
        ("", "file"),
    ] {
        let got = safe_stem(given);
        assert_eq!(got, stem, "{given:?}");
        assert!(!got.contains([' ', '.', '/', '\\']), "{given:?}: {got}");
    }
    let long = safe_stem(&format!("{}.mp4", "word ".repeat(20)));
    assert!(long.len() <= 32 && !long.ends_with('_'), "at most 32 long, no trailing gap: {long}");
    let name = stored_name("WH40K Marriah Carey.mp4", "mp4");
    let (ms, rest) = name.split_once('_').unwrap();
    assert!(ms.chars().all(|c| c.is_ascii_digit()) && rest == "WH40K_Marriah_Carey.mp4", "{name}");
}
