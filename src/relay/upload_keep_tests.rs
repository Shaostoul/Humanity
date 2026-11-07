// Child of relay/upload_rules.rs (#[path]): a picture the server cannot clean is refused, never
// stored with what it carries; it is kept as sent only on the person's own signed choice for that
// very file (2026-11-06, docs/BUGS.md BUG-260; the operator: "the server never sends anything on its
// own without that flag"). Each test was seen red first, recorded at the test.

use super::*;
use crate::relay::api::{upload_file_in, UploadQuery};
use crate::relay::files::Folders;
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;
use axum::extract::FromRequest;
use std::sync::Arc;

fn relay(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

/// `key` is a verified member signed in with `token`.
async fn uploader(state: &RelayState, key: &str, token: &str) {
    state.db.join_server(key, key).unwrap();
    state.db.set_role(key, "verified").unwrap();
    let mut role = state.db.role_def("verified");
    role.max_upload_mb = 25;
    role.max_upload_mb_per_day = 500;
    role.max_uploads_per_hour = 0;
    state.db.upsert_role(&role).unwrap();
    state.upload_tokens.write().await.insert(token.into(), key.into());
}

/// The folders of a test's own.
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
    fn stored(&self) -> Vec<String> {
        std::fs::read_dir(&self.uploads).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect()
    }
}

/// `POST /api/upload` of `file` (named `name`, typed `ct`), with the `keep` part first when given,
/// by the session `token`, with `query`'s flags.
async fn send(state: &Arc<RelayState>, d: &Dirs, token: &str, name: &str, ct: &str, file: &[u8], keep: Option<String>, mut query: UploadQuery) -> Result<serde_json::Value, (StatusCode, String)> {
    let boundary = "keepBoundary";
    let mut body = Vec::new();
    if let Some(k) = keep {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{KEEP_FIELD}\"\r\n\r\n{k}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: {ct}\r\n\r\n").as_bytes());
    body.extend_from_slice(file);
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
    query.token = Some(token.into());
    upload_file_in(state, d.folders(), &d.part, query, headers, multipart).await.map(|j| j.0)
}

/// A JPEG that says where it was taken and whose structure breaks before its picture (its EXIF
/// segment claims to run past the end of the file, as a damaged or cut-off photo's does), so the
/// strip cannot walk it.
fn broken_jpeg_with_location() -> Vec<u8> {
    let exif = b"Exif\0\0GPS-LATITUDE-47.6553,-122.6870";
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&0x7000u16.to_be_bytes());
    out.extend_from_slice(exif);
    out.extend(std::iter::repeat(0x55u8).take(300));
    out
}

const PLACE: &[u8] = b"GPS-LATITUDE-47.6553,-122.6870";

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn keep_query() -> UploadQuery {
    UploadQuery { keep_metadata: Some(1), ..Default::default() }
}

/// A PICTURE THE SERVER CANNOT CLEAN IS REFUSED, NOT STORED. A JPEG carrying a location whose walk
/// breaks is answered 422 (the "could not be cleaned" code both apps ask about) with words that say
/// so, and nothing is kept; a PNG that stops early likewise. A clean JPEG is stored as before.
///
/// Seen red 2026-11-06 against `finish_picture` as it was (the strip's fail-open answer stored):
/// "a JPEG that cannot be cleaned is refused: stored as ..." (the file was kept, location and all).
#[tokio::test]
async fn a_picture_the_server_cannot_clean_is_refused_not_stored() {
    let state = relay("keep_refused");
    let d = dirs("keep_refused");
    let (_, ann) = test_identity(93);
    uploader(&state, &ann, "tok").await;

    let photo = broken_jpeg_with_location();
    match send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, None, UploadQuery::default()).await {
        Err((code, words)) => assert_eq!((code, words.as_str()), (CANNOT_CLEAN, PICTURE_UNREADABLE)),
        Ok(done) => panic!("a JPEG that cannot be cleaned is refused: stored as {done}"),
    }
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDRshort".to_vec();
    let refused = send(&state, &d, "tok", "chart.png", "image/png", &png, None, UploadQuery::default()).await.expect_err("a PNG that stops early");
    assert_eq!(refused.0, CANNOT_CLEAN);
    assert!(d.stored().is_empty(), "nothing was kept: {:?}", d.stored());
}

/// KEEPING A FILE'S HIDDEN DETAILS TAKES THE PERSON'S OWN SIGNED CHOICE FOR THAT FILE. With
/// `keep_metadata=1`: no `keep` part, one signed over another file, one whose signature was altered,
/// one signed by someone else, and one older than 15 minutes are each refused (403, the words say
/// why) with nothing stored; the person's own, made now over this file's BLAKE3, stores it exactly
/// as sent, its location included (their choice). The resumable upload's start holds the same rule.
///
/// Seen red 2026-11-06 against the upload before the keep choice (`keep_metadata=1` honoured on its
/// own): "keep_metadata without the person's signature is refused: stored as ...".
#[tokio::test]
async fn keeping_its_details_takes_the_persons_signed_choice_for_this_file() {
    let state = relay("keep_signed");
    let d = dirs("keep_signed");
    let (seed, ann) = test_identity(94);
    let (bob_seed, bob) = test_identity(95);
    uploader(&state, &ann, "tok").await;
    let photo = broken_jpeg_with_location();
    let json = |c: &KeepChoice| serde_json::to_string(c).unwrap();
    let refused_with = |r: Result<serde_json::Value, (StatusCode, String)>, what: &str| match r {
        Err((code, words)) => assert_eq!((code, words.as_str()), (StatusCode::FORBIDDEN, KEEP_UNCHECKED), "{what}"),
        Ok(done) => panic!("{what} is refused: stored as {done}"),
    };

    refused_with(send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, None, keep_query()).await, "keep_metadata without the person's signature");
    let other = test_keep(&seed, &ann, b"another file");
    refused_with(send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, Some(json(&other)), keep_query()).await, "a choice signed for another file");
    let mut altered = test_keep(&seed, &ann, &photo);
    let flip = if altered.sig.starts_with('A') { "B" } else { "A" };
    altered.sig.replace_range(0..1, flip);
    refused_with(send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, Some(json(&altered)), keep_query()).await, "an altered signature");
    let bobs = test_keep(&bob_seed, &bob, &photo);
    refused_with(send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, Some(json(&bobs)), keep_query()).await, "a choice signed by someone else");
    let stale = test_keep_at(&seed, &ann, &photo, crate::relay::storage::now_millis() - KEEP_FRESH_MS - 60_000);
    refused_with(send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, Some(json(&stale)), keep_query()).await, "a choice made too long ago");
    refused_with(send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, Some("not a choice".into()), keep_query()).await, "a keep part that is not a choice");
    assert!(d.stored().is_empty(), "nothing was kept: {:?}", d.stored());

    let mine = test_keep(&seed, &ann, &photo);
    let done = send(&state, &d, "tok", "garden.jpg", "image/jpeg", &photo, Some(json(&mine)), keep_query()).await.expect("the person's own choice for this file");
    let kept = std::fs::read(d.uploads.join(done["filename"].as_str().unwrap())).unwrap();
    assert_eq!(kept, photo, "stored exactly as sent");
    assert!(contains(&kept, PLACE), "its location kept, by the person's choice");

    // The resumable upload (the Media page's) holds the same rule at its start.
    use crate::relay::resumable::{start_core, StartBody};
    let start = |keep: Option<KeepChoice>, blake3: Option<String>| StartBody {
        filename: "walk.mp4".into(),
        size: photo.len() as u64,
        content_type: Some("video/mp4".into()),
        media: Some(1),
        blake3,
        keep_metadata: Some(1),
        keep,
    };
    let hash = blake3_hex(&photo);
    for (what, body) in [
        ("a resumable start without the choice", start(None, Some(hash.clone()))),
        ("a resumable start without the file's hash", start(Some(test_keep(&seed, &ann, &photo)), None)),
        ("a resumable start signed by someone else", start(Some(test_keep(&bob_seed, &bob, &photo)), Some(hash.clone()))),
    ] {
        match start_core(&state, d.folders(), Some("tok"), body).await {
            Err(r) => assert_eq!((r.status(), r.words()), (StatusCode::FORBIDDEN, KEEP_UNCHECKED.to_string()), "{what}"),
            Ok(_) => panic!("{what} is refused"),
        }
    }
    assert!(start_core(&state, d.folders(), Some("tok"), start(Some(test_keep(&seed, &ann, &photo)), Some(hash))).await.is_ok(), "the person's own choice starts it");
}

/// AN UPLOAD'S 422 MEANS ONLY "COULD NOT BE CLEANED". Both apps answer a 422 from an upload by asking
/// the person whether to send the file anyway, so no other refusal of an upload route may be one:
/// every 422 in the routes' files is built through `CANNOT_CLEAN`.
///
/// Seen red 2026-11-06 with the resumable upload's hash mismatch still a 422 ("resumable.rs builds a
/// 422 that is not CANNOT_CLEAN").
#[test]
fn an_uploads_422_means_only_could_not_be_cleaned() {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("src/relay");
    for file in ["api.rs", "resumable.rs", "upload_rules.rs", "upload_limits.rs", "poster.rs", "files.rs"] {
        let src = std::fs::read_to_string(root.join(file)).unwrap();
        for line in src.lines().filter(|l| l.contains("UNPROCESSABLE_ENTITY") && !l.trim_start().starts_with("//")) {
            assert!(
                line.contains("pub(crate) const CANNOT_CLEAN: StatusCode = StatusCode::UNPROCESSABLE_ENTITY;"),
                "{file} builds a 422 that is not CANNOT_CLEAN: {line}"
            );
        }
    }
}
