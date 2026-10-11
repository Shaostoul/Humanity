//! Resumable uploads (relay/resumable.rs, storage/upload_sessions.rs): the proof the design asks
//! for (docs/design/media-showcase.md, "Resumable uploads, exact"). Each test names the break it
//! was seen failing against ("Seen red"), with the words it failed with.
//!
//! No socket and no relay process: the routes' `*_core` functions are called on a relay state
//! over a temporary database, with folders of the test's own, so nothing lands in the
//! checkout's data folder.

use super::*;
use crate::relay::storage::media::MEDIA_SHELF;
use crate::relay::storage::{ServerSettings, Storage};
use crate::test_temp::TempPath;

const MIB: u64 = 1024 * 1024;

/// A relay over a temporary database, and its four folders (deleted when the rig drops). A Media
/// upload is stored in `waiting` until it is published (R6).
struct Rig {
    state: Arc<RelayState>,
    uploads: TempPath,
    members: TempPath,
    resume: TempPath,
    waiting: TempPath,
}

fn rig(tag: &str) -> Rig {
    Rig {
        state: Arc::new(RelayState::new(Storage::open_temp(tag))),
        uploads: crate::test_temp::dir(&format!("{tag}_uploads")),
        members: crate::test_temp::dir(&format!("{tag}_members")),
        resume: crate::test_temp::dir(&format!("{tag}_resume")),
        waiting: crate::test_temp::dir(&format!("{tag}_waiting")),
    }
}

/// What a start asks for: a Media file of `size` bytes named `name`, of type `ct`.
fn file(name: &str, size: u64, ct: &str) -> StartBody {
    StartBody { filename: name.into(), size, content_type: Some(ct.into()), media: Some(1), blake3: None, keep_metadata: None }
}

impl Rig {
    fn db(&self) -> &Storage {
        &self.state.db
    }
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
    /// `key` is signed in with `token` (an upload token, as a socket's identify hands out).
    async fn sign_in(&self, key: &str, token: &str) {
        self.state.upload_tokens.write().await.insert(token.to_string(), key.to_string());
    }
    /// A verified person (a role that may upload pictures and files) signed in with `token`.
    async fn person(&self, key: &str, token: &str) {
        self.db().set_role(key, "verified").unwrap();
        self.sign_in(key, token).await;
    }
    fn settings(&self, change: impl FnOnce(&mut ServerSettings)) {
        let mut s = self.db().get_server_settings().unwrap();
        change(&mut s);
        assert!(self.db().set_server_settings(&s, "admin_key").unwrap());
    }
    /// Start an upload; its id.
    async fn start(&self, token: &str, b: StartBody) -> Result<String, Refused> {
        let (code, answer) = start_core(&self.state, self.folders(), Some(token), b).await?;
        assert_eq!(code, StatusCode::CREATED);
        Ok(answer.0["id"].as_str().unwrap().to_string())
    }
    /// Send `piece` at `offset`.
    async fn send(&self, id: &str, token: &str, offset: u64, piece: &[u8]) -> Result<Value, Refused> {
        piece_core(&self.state, self.folders(), id, Some(token), Some(offset), Bytes::copy_from_slice(piece)).await.map(|j| j.0)
    }
    /// The pieces' file of session `id`.
    fn part(&self, id: &str) -> PathBuf {
        part_path(&self.resume, id).unwrap()
    }
    fn session(&self, id: &str) -> Option<UploadSession> {
        self.db().upload_session(id).unwrap()
    }
    /// The files in a folder, by name.
    fn files_in(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

/// `n` bytes that are not all the same, so a piece written in the wrong place shows.
fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
}

/// A JPEG whose EXIF block carries a location, as a phone writes it: the start of a picture, an
/// APP1 segment with the coordinates, and the start of the image data (all the metadata strip
/// reads).
fn jpeg_with_location() -> Vec<u8> {
    let exif = b"Exif\0\0GPS-LATITUDE-47.6553,-122.6870";
    let mut out = vec![0xFF, 0xD8];
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(exif);
    out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
    out.extend(std::iter::repeat(0x55u8).take(200));
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// A FILE SENT IN THREE PIECES IS STORED ON THE MEDIA SHELF, with its size, and the answer is the
/// single request's (`upload_rules::upload_answer`, which both routes call): each piece answers
/// how far the server got, the status says the same, and the last piece answers with the stored
/// upload. The pieces' file is gone after; the session row stays, finished, with that answer (R2).
///
/// Seen red 2026-10-11 with the finish storing the file on shelf 0 (chat) instead of the
/// session's: "the file is on the Media shelf with its size / left: (0, 2500, \"clip.mp4\") /
/// right: (2, 2500, \"clip.mp4\")".
#[tokio::test]
async fn a_file_sent_in_three_pieces_is_stored_on_the_media_shelf() {
    let r = rig("resume_three");
    r.person("ann", "tok_ann").await;
    let data = bytes(2500);

    let (code, started) = start_core(&r.state, r.folders(), Some("tok_ann"), file("clip.mp4", 2500, "video/mp4")).await.unwrap();
    assert_eq!(code, StatusCode::CREATED);
    let id = started.0["id"].as_str().unwrap().to_string();
    assert!(valid_id(&id), "the id is 32 hex characters: {id}");
    assert_eq!(started.0, json!({ "id": id, "offset": 0, "size": 2500, "chunk_max": CHUNK_MAX }));
    assert!(r.part(&id).is_file(), "the pieces' file is made at the start");

    assert_eq!(r.send(&id, "tok_ann", 0, &data[..1000]).await.unwrap(), json!({ "offset": 1000, "size": 2500 }));
    assert_eq!(r.send(&id, "tok_ann", 1000, &data[1000..2000]).await.unwrap(), json!({ "offset": 2000, "size": 2500 }));
    let status = status_core(&r.state, &id, Some("tok_ann")).await.unwrap().0;
    assert_eq!(status, json!({ "offset": 2000, "size": 2500, "chunk_max": CHUNK_MAX }));

    let done = r.send(&id, "tok_ann", 2000, &data[2000..]).await.unwrap();
    let name = done["filename"].as_str().unwrap().to_string();
    assert_eq!(done, crate::relay::upload_rules::upload_answer(&name, 2500, "video"), "the single request's answer");
    assert!(name.ends_with("_clipmp4.mp4"), "the single request's naming rule: {name}");
    assert_eq!(std::fs::read(r.waiting.join(&name)).unwrap(), data, "the stored file is the file that was sent");
    assert!(Rig::files_in(&r.uploads).is_empty(), "a Media upload is at no public address until it is published (R6)");
    let (shelf, size, original): (i64, i64, String) = r
        .db()
        .with_conn(|c| c.query_row("SELECT shared, size_bytes, original_name FROM user_uploads WHERE filename = ?1", [&name], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))))
        .unwrap();
    assert_eq!((shelf, size, original.as_str()), (MEDIA_SHELF, 2500, "clip.mp4"), "the file is on the Media shelf with its size");
    assert_eq!(r.db().media_bytes(Some("ann")), 2500, "the Media budget counts it");
    assert!(r.session(&id).is_some_and(|s| s.finished()), "the session stays, finished, until the idle expiry (R2)");
    assert!(Rig::files_in(&r.resume).is_empty(), "its pieces' file is gone");
    assert_eq!(status_core(&r.state, &id, Some("tok_ann")).await.unwrap().0["done"], done, "and the status says it is done");
}

/// A PIECE SENT AT THE WRONG PLACE GETS 409 WITH THE RIGHT OFFSET (a piece sent twice, or one
/// that skips ahead), and nothing of it is written; a piece that says nowhere is refused.
///
/// Seen red 2026-10-11 with the offset check turned off: "the same piece again: Object
/// {\"offset\": Number(2000), \"size\": Number(3000)}" (the repeated piece was counted again).
#[tokio::test]
async fn a_wrong_offset_gets_409_with_the_right_one() {
    let r = rig("resume_offset");
    r.person("ann", "tok_ann").await;
    let data = bytes(3000);
    let id = r.start("tok_ann", file("clip.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann", 0, &data[..1000]).await.unwrap();

    for (at, why) in [(0u64, "the same piece again"), (2000, "a piece that skips ahead")] {
        let refused = r.send(&id, "tok_ann", at, &data[at as usize..at as usize + 1000]).await.expect_err(why);
        assert_eq!(refused.status(), StatusCode::CONFLICT, "{why}");
        match &refused {
            Refused::Json(_, body) => {
                assert_eq!((body["offset"].as_i64(), body["size"].as_i64()), (Some(1000), Some(3000)), "{why}: the server's offset");
                assert!(body["error"].as_str().unwrap().contains("Send from there"), "{body}");
            }
            other => panic!("{why}: a 409 says where the server is, in JSON: {other:?}"),
        }
    }
    assert_eq!(std::fs::metadata(r.part(&id)).unwrap().len(), 1000, "nothing of a refused piece was written");
    let none = piece_core(&r.state, r.folders(), &id, Some("tok_ann"), None, Bytes::from_static(b"x")).await.unwrap_err();
    assert_eq!(none.status(), StatusCode::BAD_REQUEST, "a piece must say where it starts");
    assert_eq!(r.send(&id, "tok_ann", 1000, &data[1000..2000]).await.unwrap()["offset"], 2000, "the right offset carries on");
}

/// A SESSION IS THE PERSON'S, NOT THE CONNECTION'S: after a reconnect (a new token for the same
/// key) the upload carries on and finishes; another person's token gets 404, the same answer as
/// an id that does not exist, for the status, a piece and a cancel; a token that is no session
/// is refused.
///
/// Seen red 2026-10-11 with `own_session` handing out any person's session: "another person's
/// token gets 404 for the status: Json(Object {\"chunk_max\": Number(8388608), \"offset\":
/// Number(1000), \"size\": Number(2000)})".
#[tokio::test]
async fn another_token_of_the_same_key_carries_on_and_another_persons_gets_404() {
    let r = rig("resume_tokens");
    r.person("ann", "tok_ann_1").await;
    r.person("bea", "tok_bea").await;
    let data = bytes(2000);
    let id = r.start("tok_ann_1", file("clip.mp4", 2000, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann_1", 0, &data[..1000]).await.unwrap();

    // Ann reconnects: her old token is gone, a new one names the same key.
    r.state.upload_tokens.write().await.remove("tok_ann_1");
    r.sign_in("ann", "tok_ann_2").await;
    assert_eq!(status_core(&r.state, &id, Some("tok_ann_2")).await.unwrap().0["offset"], 1000);

    let missing = status_core(&r.state, "0123456789abcdef0123456789abcdef", Some("tok_bea")).await.unwrap_err();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let theirs = status_core(&r.state, &id, Some("tok_bea")).await.expect_err("another person's token gets 404 for the status");
    assert_eq!((theirs.status(), theirs.words()), (missing.status(), missing.words()), "the same answer as no such upload");
    let piece = r.send(&id, "tok_bea", 1000, &data[1000..]).await.expect_err("another person's token gets 404 for a piece");
    assert_eq!(piece.status(), StatusCode::NOT_FOUND);
    let cancel = cancel_core(&r.state, r.folders(), &id, Some("tok_bea")).await.expect_err("another person's token gets 404 for a cancel");
    assert_eq!(cancel.status(), StatusCode::NOT_FOUND);
    assert!(r.part(&id).is_file(), "nothing another person sent touched the upload");
    let old = status_core(&r.state, &id, Some("tok_ann_1")).await.unwrap_err();
    assert_eq!(old.status(), StatusCode::FORBIDDEN, "a token that is no longer a session: {}", old.words());

    let done = r.send(&id, "tok_ann_2", 1000, &data[1000..]).await.unwrap();
    assert_eq!(done["size"], 2000, "the new token finished it");
}

/// A FILE OVER THE PERSON'S SHARE IS REFUSED AT THE START, with the share words (what they have
/// used, their share, the file), before anything is written.
///
/// Seen red 2026-10-11 with the room check taken out of the start: "a file over the share is
/// refused at the start: \"b68a991816fb8bafb75593a65be9c176\"" (a session id came back).
#[tokio::test]
async fn a_file_over_the_share_is_refused_at_start_with_the_share_words() {
    let r = rig("resume_share");
    r.person("ann", "tok_ann").await;
    r.settings(|s| s.media_per_person_mb = 1);
    r.db().record_upload("ann", "1_old.mp4", 100, MEDIA_SHELF, "old.mp4", (MIB / 2) as i64).unwrap();

    let refused = r.start("tok_ann", file("clip.mp4", MIB * 6 / 10, "video/mp4")).await.expect_err("a file over the share is refused at the start");
    assert_eq!(refused.status(), StatusCode::INSUFFICIENT_STORAGE);
    assert!(
        refused.words().contains("Your share of the Media space is used up: 0.5 MB of your 1.0 MB used, and this file is 0.6 MB"),
        "{}",
        refused.words()
    );
    assert!(r.db().all_upload_sessions().unwrap().is_empty(), "no session was made");
    assert!(Rig::files_in(&r.resume).is_empty(), "no file was made");
    assert!(r.start("tok_ann", file("clip.mp4", MIB * 4 / 10, "video/mp4")).await.is_ok(), "a file that fits starts");
}

/// TWO UPLOADS THAT FIT ALONE BUT NOT TOGETHER CANNOT BOTH START: the person's uploads in
/// progress count against their share, and everyone's against the Media budget, each refusal
/// saying how much of "used" is still arriving. A cancel gives the room back.
///
/// Seen red 2026-10-11 with the start counting nothing in progress (`Reserved::default()`): "the
/// second start was refused: \"12c5aa4e86deb28e95ea9ff157fabbde\"".
#[tokio::test]
async fn two_uploads_that_fit_alone_but_not_together_cannot_both_start() {
    let r = rig("resume_together");
    r.person("ann", "tok_ann").await;
    r.person("bea", "tok_bea").await;
    r.settings(|s| {
        s.media_per_person_mb = 1;
        s.media_budget_mb = 2;
    });
    let first = r.start("tok_ann", file("a.mp4", MIB * 6 / 10, "video/mp4")).await.unwrap();
    let second = r.start("tok_ann", file("b.mp4", MIB * 6 / 10, "video/mp4")).await.expect_err("the second start was refused");
    assert_eq!(second.status(), StatusCode::INSUFFICIENT_STORAGE);
    assert!(
        second.words().contains("0.6 MB of your 1.0 MB used, and this file is 0.6 MB") && second.words().contains("That counts 0.6 MB of uploads still in progress."),
        "{}",
        second.words()
    );

    // Bea's share has room, but the budget does not, counting Ann's upload in progress.
    r.start("tok_bea", file("c.mp4", MIB * 9 / 10, "video/mp4")).await.unwrap();
    let budget = r.start("tok_bea", file("d.mp4", MIB / 20, "video/mp4")).await.err();
    assert!(budget.is_none(), "a small one still fits both: {budget:?}");
    r.settings(|s| s.media_budget_mb = 1);
    let full = r.start("tok_bea", file("e.mp4", 1000, "video/mp4")).await.expect_err("the budget counts everyone's uploads in progress");
    assert!(full.words().contains("The Media space on this server is full") && full.words().contains("still in progress"), "{}", full.words());

    cancel_core(&r.state, r.folders(), &first, Some("tok_ann")).await.unwrap();
    r.settings(|s| s.media_budget_mb = 2);
    assert!(r.start("tok_ann", file("b.mp4", MIB * 6 / 10, "video/mp4")).await.is_ok(), "a cancel gives the room back");
}

/// AT MOST THREE UPLOADS IN PROGRESS PER PERSON: the fourth is refused (429) until one ends;
/// another person is not held to Ann's three.
///
/// Seen red 2026-10-11 with the limit check turned off: "a fourth upload at once is refused:
/// \"acb5fd14894a862de322176f70a26a65\"".
#[tokio::test]
async fn at_most_three_uploads_at_once_per_person() {
    let r = rig("resume_three_at_once");
    r.person("ann", "tok_ann").await;
    r.person("bea", "tok_bea").await;
    let mut ids = Vec::new();
    for n in 0..OPEN_SESSIONS_MAX {
        ids.push(r.start("tok_ann", file(&format!("{n}.mp4"), 100, "video/mp4")).await.unwrap());
    }
    let fourth = r.start("tok_ann", file("3.mp4", 100, "video/mp4")).await.expect_err("a fourth upload at once is refused");
    assert_eq!(fourth.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(fourth.words().contains("You have 3 uploads in progress"), "{}", fourth.words());
    assert!(r.start("tok_bea", file("b.mp4", 100, "video/mp4")).await.is_ok(), "Bea has her own three");
    cancel_core(&r.state, r.folders(), &ids[0], Some("tok_ann")).await.unwrap();
    assert!(r.start("tok_ann", file("3.mp4", 100, "video/mp4")).await.is_ok(), "one ended, one more starts");
}

/// A PIECE PAST THE FILE'S SIZE IS REFUSED, and so is one over `chunk_max`; nothing of either is
/// written.
///
/// Seen red 2026-10-11 with the end-of-file check turned off: "a piece past the size is refused:
/// Object {\"filename\": String(\"1791708108270_clipmp4.mp4\"), \"size\": Number(1000), ...}" (a
/// 1200-byte file stored under a 1000-byte answer).
#[tokio::test]
async fn a_piece_past_the_size_is_refused() {
    let r = rig("resume_past");
    r.person("ann", "tok_ann").await;
    let id = r.start("tok_ann", file("clip.mp4", 1000, "video/mp4")).await.unwrap();
    let past = r.send(&id, "tok_ann", 0, &bytes(1200)).await.expect_err("a piece past the size is refused");
    assert_eq!(past.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(past.words().contains("goes past the end of the file"), "{}", past.words());
    let big = r.start("tok_ann", file("big.mp4", 2 * CHUNK_MAX as u64, "video/mp4")).await.unwrap();
    let over = r.send(&big, "tok_ann", 0, &vec![1u8; CHUNK_MAX + 1]).await.expect_err("a piece over chunk_max is refused");
    assert!(over.status() == StatusCode::PAYLOAD_TOO_LARGE && over.words().contains("at most 8 MB"), "{}", over.words());
    for s in [&id, &big] {
        assert_eq!(std::fs::metadata(r.part(s)).unwrap().len(), 0, "nothing of a refused piece was written");
        assert_eq!(r.session(s).unwrap().offset, 0);
    }
}

/// THE SINGLE REQUEST'S RULES HOLD AT THE START (upload_rules.rs, one copy shared by both): a
/// blocked extension, an unknown type, a sharing switch that is off, a person with no role or a
/// role with uploads off, a picture over the picture limit. And the start's own: only Media, not
/// empty, not past the sanity ceiling, a hash that is a hash. Nothing is written for any of them.
///
/// Seen red 2026-10-11 with the start's type check told the file was encrypted (which skips the
/// lists): "a .exe is refused at the start".
#[tokio::test]
async fn a_blocked_extension_is_refused_at_start() {
    let r = rig("resume_rules");
    r.person("ann", "tok_ann").await;
    r.sign_in("nobody", "tok_nobody").await;
    let refused = |b: StartBody, token: &'static str| {
        let r = &r;
        async move { r.start(token, b).await.err().map(|e| (e.status(), e.words())) }
    };
    let (code, words) = refused(file("run.exe", 100, "application/octet-stream"), "tok_ann").await.expect("a .exe is refused at the start");
    assert_eq!((code, words.as_str()), (StatusCode::BAD_REQUEST, "File type .exe is not allowed."));
    let (_, words) = refused(file("page.svg", 100, "image/svg+xml"), "tok_ann").await.expect("an .svg is refused");
    assert!(words.contains(".svg is not allowed"), "{words}");
    let (_, words) = refused(file("notes.xyz", 100, "application/x-what"), "tok_ann").await.expect("an unknown type is refused");
    assert!(words.starts_with("Unsupported file type"), "{words}");
    let (code, _) = refused(file("photo.jpg", PICTURE_MAX_BYTES as u64 + 1, "image/jpeg"), "tok_ann").await.expect("a picture over the limit");
    assert_eq!(code, StatusCode::PAYLOAD_TOO_LARGE);
    let (code, words) = refused(file("film.mp4", SIZE_CEILING + 1, "video/mp4"), "tok_ann").await.expect("past the ceiling");
    assert!(code == StatusCode::PAYLOAD_TOO_LARGE && words.contains("at most 64 GB"), "{words}");
    assert_eq!(refused(file("empty.mp4", 0, "video/mp4"), "tok_ann").await.map(|e| e.0), Some(StatusCode::BAD_REQUEST));
    let chat = StartBody { media: None, ..file("clip.mp4", 100, "video/mp4") };
    assert_eq!(refused(chat, "tok_ann").await.map(|e| e.0), Some(StatusCode::BAD_REQUEST), "only Media");
    let bad_hash = StartBody { blake3: Some("not-a-hash".into()), ..file("clip.mp4", 100, "video/mp4") };
    assert_eq!(refused(bad_hash, "tok_ann").await.map(|e| e.0), Some(StatusCode::BAD_REQUEST));
    let (code, words) = refused(file("clip.mp4", 100, "video/mp4"), "tok_nobody").await.expect("no role");
    assert!(code == StatusCode::FORBIDDEN && words.contains("only verified users"), "{words}");

    r.settings(|s| s.file_sharing_enabled = false);
    let (code, words) = refused(file("clip.mp4", 100, "video/mp4"), "tok_ann").await.expect("file sharing off");
    assert_eq!((code, words.as_str()), (StatusCode::FORBIDDEN, "File sharing is disabled on this server."));
    assert!(r.start("tok_ann", file("photo.png", 100, "image/png")).await.is_ok(), "pictures have their own switch");
    r.settings(|s| s.image_sharing_enabled = false);
    let (_, words) = refused(file("photo.png", 100, "image/png"), "tok_ann").await.expect("image sharing off");
    assert_eq!(words, "Image sharing is disabled on this server.");

    assert_eq!(r.db().all_upload_sessions().unwrap().len(), 1, "only the one allowed start made a session");
    assert_eq!(Rig::files_in(&r.resume).len(), 1, "and a file");
}

/// A BLAKE3 MISMATCH IS REFUSED AND CLEANED UP: the file that arrived is not the one the app
/// hashed (a resume that went wrong), so nothing is kept, row or file, and the words say to send
/// it again. A matching hash is stored.
///
/// Seen red 2026-10-11 with the hash comparison turned off: "a file that does not match its hash
/// is refused: Object {\"filename\": String(\"1791708105389_clipmp4.mp4\"), \"size\": Number(1500),
/// \"type\": String(\"video\"), ...}".
#[tokio::test]
async fn a_blake3_mismatch_is_refused_and_cleaned_up() {
    let r = rig("resume_hash");
    r.person("ann", "tok_ann").await;
    let data = bytes(1500);
    let mut other = data.clone();
    other[700] ^= 0xFF;

    let wrong = StartBody { blake3: Some(blake3::hash(&other).to_hex().to_string()), ..file("clip.mp4", 1500, "video/mp4") };
    let id = r.start("tok_ann", wrong).await.unwrap();
    r.send(&id, "tok_ann", 0, &data[..1000]).await.unwrap();
    let refused = r.send(&id, "tok_ann", 1000, &data[1000..]).await.expect_err("a file that does not match its hash is refused");
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(refused.words().contains("does not match"), "{}", refused.words());
    assert!(r.session(&id).is_none(), "the session is gone");
    assert!(Rig::files_in(&r.resume).is_empty(), "the pieces' file is gone");
    assert!(Rig::files_in(&r.uploads).is_empty() && Rig::files_in(&r.waiting).is_empty(), "nothing was stored");

    let right = StartBody { blake3: Some(blake3::hash(&data).to_hex().to_string().to_uppercase()), ..file("clip.mp4", 1500, "video/mp4") };
    let id = r.start("tok_ann", right).await.unwrap();
    let done = r.send(&id, "tok_ann", 0, &data).await.unwrap();
    assert_eq!(std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap(), data, "a matching file is stored");
}

/// A PICTURE'S LOCATION IS STRIPPED AT THE FINISH, as in the single request: a phone JPEG's EXIF
/// (where the GPS lives) is gone from the stored file, which is still a JPEG, and the answer's
/// size is the stored size. A file that says it is a PNG but is not one is refused at the finish
/// and cleaned up.
///
/// Seen red 2026-10-11 with the finish skipping the picture step (the file stored as it
/// arrived): "the location is gone from the stored picture".
#[tokio::test]
async fn a_pictures_location_is_stripped_at_finish() {
    let r = rig("resume_picture");
    r.person("ann", "tok_ann").await;
    let photo = jpeg_with_location();
    assert!(contains(&photo, b"GPS-LATITUDE"), "test setup: the location is in the photo");
    let id = r.start("tok_ann", file("garden.jpg", photo.len() as u64, "image/jpeg")).await.unwrap();
    r.send(&id, "tok_ann", 0, &photo[..20]).await.unwrap();
    let done = r.send(&id, "tok_ann", 20, &photo[20..]).await.unwrap();
    let stored = std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap();
    assert!(!contains(&stored, b"GPS-LATITUDE"), "the location is gone from the stored picture");
    assert_eq!(&stored[..3], b"\xFF\xD8\xFF", "it is still a JPEG");
    assert_eq!((done["type"].as_str(), done["size"].as_u64()), (Some("image"), Some(stored.len() as u64)));
    assert!(Rig::files_in(&r.resume).is_empty(), "the pieces' file is gone");

    let fake = r.start("tok_ann", file("fake.png", 100, "image/png")).await.unwrap();
    let refused = r.send(&fake, "tok_ann", 0, &bytes(100)).await.expect_err("not a PNG");
    assert_eq!(refused.words(), "File content does not match claimed image type.");
    assert!(r.session(&fake).is_none() && Rig::files_in(&r.resume).is_empty(), "a file that is not what it says is not kept");
}

/// CANCELLING (DELETE) REMOVES THE SESSION AND ITS FILE; afterwards the upload is not found.
///
/// Seen red 2026-10-11 with the cancel leaving the pieces' file: "the pieces' file went with
/// it".
#[tokio::test]
async fn cancelling_removes_the_session_and_its_file() {
    let r = rig("resume_cancel");
    r.person("ann", "tok_ann").await;
    let id = r.start("tok_ann", file("clip.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann", 0, &bytes(1000)).await.unwrap();
    assert_eq!(cancel_core(&r.state, r.folders(), &id, Some("tok_ann")).await.unwrap().0, json!({ "ok": true }));
    assert!(r.session(&id).is_none(), "the session is gone");
    assert!(!r.part(&id).exists(), "the pieces' file went with it");
    assert_eq!(status_core(&r.state, &id, Some("tok_ann")).await.unwrap_err().status(), StatusCode::NOT_FOUND);
}

/// ONE PIECE AT A TIME PER SESSION: while one is being written, a second piece and a cancel get
/// 409 (saying it is busy, without moving anything); once it is done they go through.
///
/// Seen red 2026-10-11 with `Writing::begin` never saying busy: "a second piece while one is
/// written gets 409: Object {\"offset\": Number(1000), \"size\": Number(2000)}".
#[tokio::test]
async fn one_piece_at_a_time_per_session() {
    let r = rig("resume_busy");
    r.person("ann", "tok_ann").await;
    let id = r.start("tok_ann", file("clip.mp4", 2000, "video/mp4")).await.unwrap();
    let writing = Writing::begin(&id).expect("nothing is being written yet");
    let second = r.send(&id, "tok_ann", 0, &bytes(1000)).await.expect_err("a second piece while one is written gets 409");
    assert_eq!(second.status(), StatusCode::CONFLICT);
    assert!(matches!(&second, Refused::Json(_, body) if body["busy"] == true), "{second:?}");
    assert_eq!(cancel_core(&r.state, r.folders(), &id, Some("tok_ann")).await.unwrap_err().status(), StatusCode::CONFLICT);
    assert_eq!(std::fs::metadata(r.part(&id)).unwrap().len(), 0, "nothing was written");
    drop(writing);
    assert_eq!(r.send(&id, "tok_ann", 0, &bytes(1000)).await.unwrap()["offset"], 1000, "once it is done, the next goes through");
}

/// AN IDLE SESSION EXPIRES: one with no piece for a day goes in the expiry pass (the one the
/// relay runs at start, every six hours and after a settings change), row and file; a newer one
/// stays.
///
/// Seen red 2026-10-11 with the sweep taken out of `run_expiry_sweeps_in`: "the idle session's
/// file went".
#[tokio::test]
async fn an_idle_session_expires() {
    let r = rig("resume_idle");
    r.person("ann", "tok_ann").await;
    let old = r.start("tok_ann", file("old.mp4", 3000, "video/mp4")).await.unwrap();
    let new = r.start("tok_ann", file("new.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&old, "tok_ann", 0, &bytes(1000)).await.unwrap();
    let day_and_an_hour = 25 * 60 * 60 * 1000i64;
    r.db().with_conn(|c| c.execute("UPDATE upload_sessions SET touched_at = touched_at - ?1 WHERE id = ?2", rusqlite::params![day_and_an_hour, old])).unwrap();

    r.db().run_expiry_sweeps_in(r.folders());
    assert!(!r.part(&old).exists(), "the idle session's file went");
    assert!(r.session(&old).is_none(), "and its row");
    assert!(r.part(&new).exists() && r.session(&new).is_some(), "a newer one stays");
}

/// THE START-UP CHECK makes the folder and the rows agree (the relay stopped mid-piece): a file
/// longer than its stored offset is cut back to it, so the upload carries on from there and
/// finishes whole; a session whose file is gone is removed; files with no session are removed;
/// a file shorter than its offset lowers the offset to what is there.
///
/// Seen red 2026-10-11 with the cut (`set_len`) taken out of the start-up check: "the long file is
/// cut back to its offset / left: 1700 / right: 1000".
#[tokio::test]
async fn the_start_up_check_cuts_a_long_file_back_and_removes_orphans() {
    let r = rig("resume_reconcile");
    r.person("ann", "tok_ann").await;
    let data = bytes(3000);
    let long = r.start("tok_ann", file("long.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&long, "tok_ann", 0, &data[..1000]).await.unwrap();
    // The relay wrote the next piece and stopped before counting it: the file has 1700 bytes of
    // which only 1000 are counted (and the rest is not even what the app will send).
    {
        use std::io::Write;
        std::fs::OpenOptions::new().append(true).open(r.part(&long)).unwrap().write_all(&[0xEE; 700]).unwrap();
    }
    let lost = r.start("tok_ann", file("lost.mp4", 3000, "video/mp4")).await.unwrap();
    std::fs::remove_file(r.part(&lost)).unwrap();
    let short = r.start("tok_ann", file("short.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&short, "tok_ann", 0, &data[..1000]).await.unwrap();
    r.db().with_conn(|c| c.execute("UPDATE upload_sessions SET offset = 2000 WHERE id = ?1", [&short])).unwrap();
    std::fs::write(r.resume.join("0123456789abcdef0123456789abcdef.part"), b"no session").unwrap();
    std::fs::write(r.resume.join("stray.tmp"), b"not ours").unwrap();

    let done = r.db().reconcile_upload_sessions_in(&r.resume).unwrap();
    assert_eq!(done, crate::relay::storage::upload_sessions::Reconciled { files_removed: 2, rows_removed: 1, cut_back: 1, offset_lowered: 1 });
    assert_eq!(std::fs::metadata(r.part(&long)).unwrap().len(), 1000, "the long file is cut back to its offset");
    assert!(r.session(&lost).is_none(), "the session with no file is gone");
    assert_eq!(r.session(&short).unwrap().offset, 1000, "the short file's offset is what is there");
    let mut left = Rig::files_in(&r.resume);
    left.retain(|f| f != &format!("{long}.part") && f != &format!("{short}.part"));
    assert!(left.is_empty(), "files with no session are gone: {left:?}");

    let finished = r.send(&long, "tok_ann", 1000, &data[1000..]).await.unwrap();
    assert_eq!(std::fs::read(r.waiting.join(finished["filename"].as_str().unwrap())).unwrap(), data, "it carried on and finished whole");
}

/// AN ACCOUNT ERASE REMOVES THE PERSON'S SESSIONS, rows and files, and says so in its receipt;
/// the export lists them first (file name, size, offset; not the id). Another person's stays.
///
/// Seen red 2026-10-11 with the erase not deleting the pieces' files: the receipt said
/// "(\"upload_session_files_removed\", 0)" beside "(\"upload_sessions\", 2)".
#[tokio::test]
async fn an_account_erase_removes_sessions() {
    let r = rig("resume_erase");
    r.person("ann", "tok_ann").await;
    r.person("bea", "tok_bea").await;
    let a1 = r.start("tok_ann", file("one.mp4", 3000, "video/mp4")).await.unwrap();
    let a2 = r.start("tok_ann", file("two.mp4", 3000, "video/mp4")).await.unwrap();
    r.send(&a1, "tok_ann", 0, &bytes(1000)).await.unwrap();
    let b1 = r.start("tok_bea", file("hers.mp4", 3000, "video/mp4")).await.unwrap();

    let export = r.db().export_account("ann", "Ann");
    let listed = export["upload_sessions"].as_array().unwrap();
    assert_eq!(listed.len(), 2, "the export lists Ann's uploads in progress");
    let one = listed.iter().find(|l| l["filename"] == "one.mp4").expect("one.mp4 is listed");
    assert_eq!((one["size"].as_i64(), one["offset"].as_i64()), (Some(3000), Some(1000)), "its size and how much arrived");
    assert!(one.get("id").is_none(), "not the id, which carries the upload on");

    let receipt = r.db().delete_account_in("ann", "Ann", r.folders());
    assert!(!receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "{receipt:?}");
    assert!(receipt.contains(&("upload_sessions".to_string(), 2)) && receipt.contains(&("upload_session_files_removed".to_string(), 2)), "{receipt:?}");
    assert!(!r.part(&a1).exists() && !r.part(&a2).exists(), "Ann's pieces' files went");
    assert!(r.db().upload_sessions_of("ann").unwrap().is_empty(), "and her sessions");
    assert!(!r.db().erase_left_rows("ann"), "nothing of hers is left");
    assert!(r.part(&b1).exists() && r.session(&b1).is_some(), "Bea's upload stays");
}

/// A DATABASE FROM BEFORE `upload_sessions` OPENS AND GAINS IT (CLAUDE.md BUG-046: a live
/// database already has every other table, so the schema must add this one without touching
/// them). Built as such a database is: everything but the new table, with a row in an old one.
///
/// Seen red 2026-10-11 with the schema creating the table under another name: "the new table is
/// there: SqliteFailure(Error { code: Unknown, extended_code: 1 }, Some(\"no such table:
/// upload_sessions\"))".
#[test]
fn an_older_database_gains_the_sessions_table() {
    let path = crate::test_temp::db("resume_older_db");
    {
        let db = Storage::open(&path).expect("open");
        db.with_conn(|c| c.execute_batch("DROP TABLE IF EXISTS upload_sessions;")).unwrap();
        db.record_upload("carol", "1_old.png", 4, 0i64, "old.png", 10).unwrap();
    }
    let db = Storage::open(&path).expect("a database from before upload_sessions opens");
    assert_eq!(db.get_upload_count("carol").unwrap(), 1, "the old row is intact");
    let s = UploadSession {
        id: "0123456789abcdef0123456789abcdef".into(),
        public_key: "carol".into(),
        filename: "film.mp4".into(),
        content_type: "video/mp4".into(),
        size: 10,
        offset: 0,
        shelf: MEDIA_SHELF,
        blake3: String::new(),
        created_at: 1,
        touched_at: 1,
        keep_metadata: false,
        hash_checked: false,
        done_answer: String::new(),
    };
    db.insert_upload_session(&s).expect("the new table is there");
    assert_eq!(db.upload_session(&s.id).unwrap(), Some(s));
    drop(db);
}

/// THE RELAY WIRES IT: the four routes beside `/api/upload`, the piece route with its own body
/// limit (one piece plus a little, not axum's 2 MB default nor the 1 GB of the single request),
/// the start-up check where the part files are cleared (and never the same folder), and the
/// Uploads switch covering the new paths.
///
/// Seen red 2026-10-11 with the start-up check taken out of `run_relay`: "the start-up check
/// runs in run_relay".
#[test]
fn the_relay_wires_the_routes_the_start_up_check_and_the_piece_limit() {
    let mod_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/relay/mod.rs")).unwrap();
    assert!(mod_rs.contains(r#".route("/api/upload/resumable", post(resumable::start))"#), "the start route");
    assert!(
        mod_rs.contains(r#".route("/api/upload/resumable/{id}", get(resumable::status).patch(resumable::piece).delete(resumable::cancel).layer(axum::extract::DefaultBodyLimit::max(resumable::PIECE_BODY_LIMIT)))"#),
        "the session routes, with the piece route's body limit"
    );
    let start = mod_rs.find("pub async fn run_relay()").expect("run_relay");
    let run = &mod_rs[start..];
    let clear = run.find("clear_part_files(").expect("the part files are cleared at start");
    let check = run.find("reconcile_upload_sessions_in(").expect("the start-up check runs in run_relay");
    assert!(check > clear && check - clear < 1500, "the start-up check runs where the part files are cleared");
    assert_ne!(crate::relay::upload_limits::UPLOAD_PART_DIR, crate::relay::storage::media::RESUME_DIR, "clearing the part files never touches the sessions' pieces");
    assert!(PIECE_BODY_LIMIT > CHUNK_MAX && PIECE_BODY_LIMIT < 2 * CHUNK_MAX);
    use crate::relay::features::{route_feature, Feature};
    assert_eq!(route_feature("/api/upload/resumable"), Some(Feature::Uploads));
    assert_eq!(route_feature("/api/upload/resumable/0123456789abcdef0123456789abcdef"), Some(Feature::Uploads));
}

/// One MP4 box (size, type, contents), for the video tests below.
fn mp4_box(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(name);
    v.extend_from_slice(payload);
    v
}

/// A phone-style video: `ftyp`, then a `moov` holding the movie header and a `udta` with the
/// `©xyz` place it was filmed (as Android and others write it), then the picture data.
fn phone_video() -> Vec<u8> {
    let mut xyz = vec![0x00, 0x12, 0x15, 0xC7]; // text length, language
    xyz.extend_from_slice(b"+47.6553-122.6870/");
    let udta = mp4_box(b"udta", &mp4_box(b"\xA9xyz", &xyz));
    let moov = mp4_box(b"moov", &[mp4_box(b"mvhd", &[0u8; 100]), udta].concat());
    [mp4_box(b"ftyp", b"isom\0\0\x02\0isomiso2mp41"), moov, mp4_box(b"mdat", &bytes(3000))].concat()
}

/// A PHONE VIDEO LOSES WHERE IT WAS FILMED (BUG-190): sent in pieces, the stored file no longer
/// holds the place, is the same size, and its picture data is byte for byte what was sent. A
/// video whose boxes cannot be read through (a box claiming to run past the movie that holds it)
/// is refused with words that say why, and its session and file go.
///
/// Seen red 2026-10-11 with `finish_video` not called at the finish: "the place it was filmed is
/// gone from the stored video".
#[tokio::test]
async fn a_videos_location_is_removed_at_finish() {
    let r = rig("resume_video");
    r.person("ann", "tok_ann").await;
    let video = phone_video();
    assert!(contains(&video, b"+47.6553-122.6870"), "test setup: the place is in the video");
    let id = r.start("tok_ann", file("walk.mp4", video.len() as u64, "video/mp4")).await.unwrap();
    r.send(&id, "tok_ann", 0, &video[..100]).await.unwrap();
    let done = r.send(&id, "tok_ann", 100, &video[100..]).await.unwrap();
    let stored = std::fs::read(r.waiting.join(done["filename"].as_str().unwrap())).unwrap();
    assert!(!contains(&stored, b"+47.6553-122.6870"), "the place it was filmed is gone from the stored video");
    assert_eq!(stored.len(), video.len(), "nothing moved: the same size");
    assert_eq!(&stored[stored.len() - 3000..], &video[video.len() - 3000..], "the picture data is untouched");
    assert_eq!(done["type"], "video");

    // A box inside `moov` that claims to run past it: the walk stops early there.
    let mut broken = video.clone();
    let mvhd_at = broken.windows(4).position(|w| w == b"mvhd").unwrap() - 4;
    broken[mvhd_at..mvhd_at + 4].copy_from_slice(&5000u32.to_be_bytes());
    let bad = r.start("tok_ann", file("broken.mp4", broken.len() as u64, "video/mp4")).await.unwrap();
    let refused = r.send(&bad, "tok_ann", 0, &broken).await.expect_err("a video that cannot be read through");
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused.words(), crate::relay::upload_rules::VIDEO_UNREADABLE);
    assert!(r.session(&bad).is_none() && !r.part(&bad).exists(), "nothing of it is kept");
}

/// THE SINGLE REQUEST STRIPS VIDEOS TOO: `upload_file` sends every file that is neither a picture
/// nor encrypted through `finish_file_as` (once called `finish_video`) before it is moved into place, and removes the part file
/// when it is refused. (A source check: the single request stores into the live data/uploads
/// folder, which a test must not write to; `finish_file_as` itself is proven by the test above.)
///
/// Seen red 2026-10-11 with the call taken out of `upload_file`: "the single request strips a
/// video's location".
#[test]
fn the_single_request_strips_videos_too() {
    // Line endings normalised: a Windows checkout has CRLF.
    let api = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/relay/api.rs")).unwrap().replace("\r\n", "\n");
    let start = api.find("pub async fn upload_file(").expect("upload_file");
    let body = &api[start..start + api[start..].find("\n}\n").unwrap()];
    let non_picture = &body[body.find("axum::body::Bytes::from(stripped)").expect("the picture branch")..];
    let call = non_picture.find("finish_file_as(&path, keep_metadata)").expect("the single request strips a video's location");
    let guard = non_picture.find("if !is_encrypted").expect("only files that are not encrypted");
    assert!(guard < call, "an encrypted file is never opened");
    assert!(non_picture[call..].contains("remove_file(&part.path)"), "a refused video's part file is removed");
}

/// A finish of an upload with this file name waits for [`FINISH_HOLD`] at its start
/// (resumable.rs `finish`, test builds only), so a test can hold one finish while it is running
/// (R2, in resumable_follow_ups_tests.rs).
pub(super) const HELD_FINISH_NAME: &str = "held-finish.mp4";
pub(super) static FINISH_HOLD: std::sync::Mutex<()> = std::sync::Mutex::new(());

// The increment 2 follow-ups and the review's fixes (2026-10-11): R1, R2, R3, B1, B3, B4, R7.
#[path = "resumable_follow_ups_tests.rs"]
mod follow_ups;
