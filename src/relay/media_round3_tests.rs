//! The Media routes, round 3 (2026-10-11; docs/design/media-showcase.md, "Round 3: fixes from the
//! round 2 review"): the role's largest Media file held at a publish too (R3-3), a member's own
//! uploads in progress (R3-7), and tickets that open one members-only file without the session's
//! token in any address (R3-8, relay/media_ticket.rs). Each test names the break it was seen
//! failing against ("Seen red").
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, and nothing is written outside a temporary folder.

use super::*;
use crate::relay::media_ticket::{self, TicketBody, TICKET_REFUSED};
use crate::relay::storage::upload_sessions::UploadSession;
use std::time::{Duration, Instant};

const MIB: i64 = 1024 * 1024;

fn relay(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

/// The four folders of a test's own.
struct Dirs {
    uploads: crate::test_temp::TempPath,
    members: crate::test_temp::TempPath,
    resume: crate::test_temp::TempPath,
    waiting: crate::test_temp::TempPath,
}

fn dirs(tag: &str) -> Dirs {
    let d = |what: &str| crate::test_temp::dir(&format!("{tag}_{what}"));
    Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting") }
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
}

/// `key` uploads `file` for Media (in the waiting folder, as `store_upload` puts one) and publishes
/// it with `visibility`.
fn published(db: &Storage, d: &Dirs, key: &str, file: &str, visibility: &str) -> MediaItem {
    std::fs::write(d.waiting.join(file), b"0123456789").unwrap();
    db.record_upload(key, file, 100, MEDIA_SHELF, &format!("own {file}"), 10).unwrap();
    let s = db.get_server_settings().unwrap();
    let body = PublishBody { key: key.into(), filename: file.into(), visibility: Some(visibility.into()), ..Default::default() };
    let answer = publish_core(db, &s, &[], d.folders(), &body).unwrap().0;
    db.media_item(answer["item"]["id"].as_i64().unwrap()).unwrap().unwrap()
}

/// R3-3. THE ROLE'S LARGEST MEDIA FILE HOLDS AT A PUBLISH TOO: a 3 MB file uploaded to chat (where
/// the role's per-file limit is 25 MB) cannot be put on the Media page when the role's Media files
/// are limited to 2 MB (413, the upload's own words), and stays where it was; a 1 MB one can; 0
/// (every role's default) leaves it to the share.
///
/// Seen red 2026-10-11 with the check taken out of `publish_media_placing`: "a 3 MB chat file is
/// not published past the role's 2 MB Media limit: Json(Object {\"item\": ...".
#[test]
fn the_roles_media_file_limit_holds_at_a_publish_too() {
    let state = relay("mr3_r33");
    let db = &state.db;
    let d = dirs("mr3_r33");
    let ann = "ann";
    db.join_server(ann, "Ann").unwrap();
    db.set_role(ann, "verified").unwrap();
    let mut role = db.role_def("verified");
    role.max_upload_mb = 25;
    role.max_media_file_mb = 2;
    db.upsert_role(&role).unwrap();
    for (file, size) in [("1_big.mp4", 3 * MIB), ("2_small.mp4", MIB)] {
        std::fs::write(d.uploads.join(file), b"from chat").unwrap();
        db.record_upload(ann, file, 100, 0i64, file, size).unwrap();
    }
    let s = db.get_server_settings().unwrap();
    let publish = |file: &str| publish_core(db, &s, &[], d.folders(), &PublishBody { key: ann.into(), filename: file.into(), ..Default::default() });

    match publish("1_big.mp4") {
        Err((code, words)) => {
            assert_eq!(code, StatusCode::PAYLOAD_TOO_LARGE);
            assert_eq!(words, "Media files are limited to 2 MB for your role on this server.");
        }
        Ok(answer) => panic!("a 3 MB chat file is not published past the role's 2 MB Media limit: {answer:?}"),
    }
    assert!(d.uploads.join("1_big.mp4").is_file(), "it stays where it was");
    assert!(db.media_item_for_upload("1_big.mp4").unwrap().is_none(), "and is no Media item");
    assert!(publish("2_small.mp4").is_ok(), "a 1 MB one goes");

    role.max_media_file_mb = 0;
    db.upsert_role(&role).unwrap();
    assert!(publish("1_big.mp4").is_ok(), "0: up to the share");
}

/// R3-7. A MEMBER'S OWN USE LISTS THEIR UPLOADS IN PROGRESS: `/api/media/mine` answers
/// `uploads_in_progress: [{id, filename, size, offset, touched_at}]`, their own resumable uploads
/// still arriving, oldest first, so an app can show and cancel one it lost track of. A finished
/// one (its file is in `waiting`) and anyone else's are not in it.
///
/// Seen red 2026-10-11 with the field taken out of `mine_core`: "uploads_in_progress is a list".
#[test]
fn a_members_own_use_lists_their_uploads_in_progress() {
    let state = relay("mr3_r37");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    let session = |id: char, key: &str, filename: &str, offset: i64, done: &str| UploadSession {
        id: std::iter::repeat(id).take(32).collect(),
        public_key: key.into(),
        filename: filename.into(),
        content_type: "video/mp4".into(),
        size: 5000,
        offset,
        shelf: MEDIA_SHELF,
        blake3: String::new(),
        created_at: 1000 + offset,
        touched_at: 2000 + offset,
        keep_metadata: false,
        hash_checked: false,
        done_answer: done.into(),
    };
    db.insert_upload_session(&session('a', "ann", "walk.mp4", 1200, "")).unwrap();
    db.insert_upload_session(&session('b', "ann", "done.mp4", 5000, "{\"url\":\"/uploads/x\"}")).unwrap();
    db.insert_upload_session(&session('c', "bob", "bobs.mp4", 300, "")).unwrap();
    db.insert_upload_session(&session('d', "ann", "dusk.mp4", 4000, "")).unwrap();

    let mine = mine_core(db, &db.get_server_settings().unwrap(), "ann").unwrap().0;
    let list = mine["uploads_in_progress"].as_array().expect("uploads_in_progress is a list");
    let a: String = std::iter::repeat('a').take(32).collect();
    assert_eq!(list.len(), 2, "ann's two still arriving: {list:?}");
    assert_eq!(list[0], json!({ "id": a, "filename": "walk.mp4", "size": 5000, "offset": 1200, "touched_at": 3200 }));
    assert_eq!(list[1]["filename"], "dusk.mp4");
    assert_eq!(mine["in_progress_bytes"], 10000, "the same uploads the total counts");
}

/// A members-only item of `key`'s, and `key` signed in with `token`.
fn member_with_item(state: &RelayState, d: &Dirs, key: &str, token: &str, file: &str) -> MediaItem {
    state.db.join_server(key, key).unwrap();
    state.upload_tokens.try_write().expect("test setup: nobody holds the tokens").insert(token.into(), key.into());
    published(&state.db, d, key, file, "members")
}

/// A GET of item `id`'s file with `ticket`, and a `Range` header when given.
async fn open(state: &RelayState, d: &Dirs, id: i64, ticket: &str, range: Option<&str>) -> (u16, Vec<u8>) {
    let mut b = axum::http::Request::builder().uri(format!("/api/media/file/{id}?ticket={ticket}"));
    if let Some(r) = range {
        b = b.header(header::RANGE, r);
    }
    let q = TokenQuery { ticket: Some(ticket.into()), ..Default::default() };
    let res = file_from(state, d.folders(), id, q, b.body(axum::body::Body::empty()).unwrap()).await;
    let code = res.status().as_u16();
    (code, axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec())
}

/// The ticket in a ticket answer's `url`.
fn ticket_of(answer: &Value) -> String {
    answer["url"].as_str().unwrap().split("?ticket=").nth(1).expect("a ticket in the url").to_string()
}

/// R3-8 (a). A TICKET OPENS ONE MEMBERS-ONLY FILE, RANGES INCLUDED, AND NOTHING ELSE: a member asks
/// with the session's upload token in `X-Upload-Token` and the item's number and file name, and is
/// given `{url: "/api/media/file/<id>?ticket=...", expires_in_s: 600}`. The ticket is 64 random hex
/// characters with nothing of the token in it; it opens that file whole and by range (a video
/// seeks), and is refused for another members-only item, as a made-up one is. Without the header
/// there is no ticket, and a file name that is not the item's is the stale-page refusal (W5).
///
/// Seen red 2026-10-11 with the ticket bound to neither its item nor its file name (`Tickets::lookup`
/// not comparing the item, `admitted` not comparing the file name; either one alone refuses another
/// item): "a ticket opens no other item / left: 200 / right: 403".
#[tokio::test]
async fn a_ticket_opens_one_members_file_ranges_included() {
    let state = relay("mr3_r38a");
    let d = dirs("mr3_r38a");
    let item = member_with_item(&state, &d, "ann", "tok_ann", "1_clip.mp4");
    let other = published(&state.db, &d, "ann", "2_other.mp4", "members");
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(UPLOAD_TOKEN_HEADER, "tok_ann".parse().unwrap());

    let answer = media_ticket::ticket(State(state.clone()), headers.clone(), Json(TicketBody { id: item.id, filename: item.upload_filename.clone() }))
        .await
        .unwrap()
        .0;
    assert_eq!(answer["expires_in_s"], 600);
    let ticket = ticket_of(&answer);
    assert_eq!(answer["url"], format!("/api/media/file/{}?ticket={ticket}", item.id));
    assert!(ticket.len() == 64 && ticket.bytes().all(|c| c.is_ascii_hexdigit()), "256 random bits in hex: {ticket}");
    assert!(!answer.to_string().contains("tok_ann"), "nothing of the session's token");

    assert_eq!(open(&state, &d, item.id, &ticket, None).await, (200, b"0123456789".to_vec()), "the whole file");
    assert_eq!(open(&state, &d, item.id, &ticket, Some("bytes=2-5")).await, (206, b"2345".to_vec()), "a range of it");
    assert_eq!(open(&state, &d, other.id, &ticket, None).await.0, 403, "a ticket opens no other item");
    let made_up = "ab".repeat(32);
    let (code, words) = open(&state, &d, item.id, &made_up, None).await;
    assert_eq!((code, String::from_utf8(words).unwrap()), (403, TICKET_REFUSED.to_string()), "a made-up ticket");

    let no_header = media_ticket::ticket(State(state.clone()), axum::http::HeaderMap::new(), Json(TicketBody { id: item.id, filename: item.upload_filename.clone() })).await;
    assert_eq!(no_header.unwrap_err().0, StatusCode::UNAUTHORIZED, "the token goes in the header");
    let stale = media_ticket::ticket(State(state.clone()), headers, Json(TicketBody { id: item.id, filename: other.upload_filename.clone() })).await;
    assert_eq!(stale.unwrap_err().0, StatusCode::CONFLICT, "bound to the item's own file");
}

/// R3-8 (b). A TICKET RUNS OUT, AND IS ONLY FOR MEMBERS: a ticket given eleven minutes ago is
/// refused (a copied link stops working); someone signed in who is not a member is given none; a
/// member's live ticket stops working once they are banned, and a banned member is given none.
/// Tickets are held in memory only.
///
/// Seen red 2026-10-11 with `Tickets::lookup` not comparing the time (in the same run as the
/// break above, which this first check does not depend on): "a ticket from eleven minutes ago is
/// refused / left: 200 / right: 403".
#[tokio::test]
async fn a_ticket_runs_out_and_is_only_for_members() {
    let state = relay("mr3_r38b");
    let d = dirs("mr3_r38b");
    let item = member_with_item(&state, &d, "ann", "tok_ann", "1_clip.mp4");
    let name = item.upload_filename.clone();

    let long_ago = Instant::now().checked_sub(Duration::from_secs(11 * 60)).expect("this machine has been up eleven minutes");
    let old = ticket_of(&media_ticket::ticket_core(&state, "ann", item.id, &name, long_ago).unwrap().0);
    assert_eq!(open(&state, &d, item.id, &old, None).await.0, 403, "a ticket from eleven minutes ago is refused");
    let fresh = ticket_of(&media_ticket::ticket_core(&state, "ann", item.id, &name, Instant::now()).unwrap().0);
    assert_eq!(open(&state, &d, item.id, &fresh, None).await.0, 200, "a fresh one opens it");

    state.upload_tokens.write().await.insert("tok_carl".into(), "carl".into());
    let stranger = media_ticket::ticket_core(&state, "carl", item.id, &name, Instant::now());
    assert_eq!(stranger.unwrap_err().0, StatusCode::FORBIDDEN, "someone who is not a member is given none");

    state.db.ban_user("ann", "ann").unwrap();
    assert_eq!(open(&state, &d, item.id, &fresh, None).await.0, 403, "banned, their live ticket stops working");
    assert_eq!(media_ticket::ticket_core(&state, "ann", item.id, &name, Instant::now()).unwrap_err().0, StatusCode::FORBIDDEN);
    assert_eq!(state.media_tickets.len(), 1, "in memory, and the expired one is dropped when the next is given");
}
