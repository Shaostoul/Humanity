//! The Media routes, the increment 2 follow-ups and the review's fixes (2026-10-11;
//! docs/design/media-showcase.md, "Increment 2 follow-ups, exact" and its "As built" note): what
//! every item says (A1), the featured filter (A2), picks by a signed POST (A3), a member's own use
//! (A4), keeping an item on the server (A7), a stale tile never acting on another item (W5), the
//! members' file by a header (W3b), an upload copied into place outside the room lock (R5), a
//! Media upload waiting where nothing serves it (R6), and files from private conversations (B5).
//! Each test names the break it was seen failing against ("Seen red").
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, and nothing is written outside a temporary folder.

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

const MIB: i64 = 1024 * 1024;

struct Person {
    key: String,
    pair: DilithiumKeypair,
}

fn person(seed: u8) -> Person {
    let pair = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    Person { key: hex::encode(pair.public_key()), pair }
}

impl Person {
    fn sign(&self, purpose: &str, ts: u64) -> String {
        hex::encode(self.pair.sign(format!("{purpose}\n{ts}").as_bytes()))
    }
    fn list(&self, ts: u64) -> ListParams {
        ListParams { key: Some(self.key.clone()), timestamp: Some(ts), sig: Some(self.sign("media_list", ts)), ..Default::default() }
    }
}

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
    Dirs {
        uploads: crate::test_temp::dir(&format!("{tag}_uploads")),
        members: crate::test_temp::dir(&format!("{tag}_members")),
        resume: crate::test_temp::dir(&format!("{tag}_resume")),
        waiting: crate::test_temp::dir(&format!("{tag}_waiting")),
    }
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

fn ids(answer: &Value) -> Vec<i64> {
    answer["items"].as_array().expect("an items list").iter().map(|i| i["id"].as_i64().unwrap()).collect()
}

/// A1. EVERY ITEM SAYS WHAT REMOVE DOES TO ITS FILE AND WHAT THE FILE IS CALLED: an item from a
/// chat upload keeps its file (`keeps_file: true`), one uploaded for Media does not; each carries
/// the uploader's own name for the file.
///
/// Seen red 2026-10-11 with `keeps_file` read from the wrong shelf test (`!was_public`): "an item
/// from a chat upload keeps its file / left: Bool(false) / right: true".
#[test]
fn every_item_says_what_remove_does_and_its_own_name() {
    let state = relay("mfu_a1");
    let db = &state.db;
    let d = dirs("mfu_a1");
    let ann = person(1);
    db.join_server(&ann.key, "Ann").unwrap();
    std::fs::write(d.uploads.join("1_chat.png"), b"chat").unwrap();
    db.record_upload(&ann.key, "1_chat.png", 100, 0i64, "Holiday photo.png", 4).unwrap();
    let s = db.get_server_settings().unwrap();
    let chat = PublishBody { key: ann.key.clone(), filename: "1_chat.png".into(), ..Default::default() };
    publish_core(db, &s, &[], d.folders(), &chat).unwrap();
    published(db, &d, &ann.key, "2_made.mp4", "public");

    let list = list_core(db, &s, &ListParams::default(), None, None).unwrap().0;
    let items = list["items"].as_array().unwrap();
    let by_file = |f: &str| items.iter().find(|i| i["url"].as_str().unwrap().ends_with(f)).unwrap().clone();
    assert_eq!(by_file("1_chat.png")["keeps_file"], true, "an item from a chat upload keeps its file");
    assert_eq!(by_file("2_made.mp4")["keeps_file"], false, "one uploaded for Media loses it");
    assert_eq!(by_file("1_chat.png")["original_name"], "Holiday photo.png");
    assert_eq!(by_file("2_made.mp4")["original_name"], "own 2_made.mp4");
    assert_eq!(by_file("2_made.mp4")["kept"], false);
}

/// A2. THE FEATURED FILTER: `featured=1` lists only featured items, through the GET and the signed
/// POST alike, so the strip shows every featured item and not just those on the pages loaded.
///
/// Seen red 2026-10-11 with the filter's SQL clause made always true in `list_media`: "only the
/// featured item / left: [2, 1] / right: [1]".
#[tokio::test]
async fn the_featured_filter_lists_only_featured_items() {
    let state = relay("mfu_a2");
    let d = dirs("mfu_a2");
    let ann = person(2);
    state.db.join_server(&ann.key, "Ann").unwrap();
    let first = published(&state.db, &d, &ann.key, "1_a.png", "public");
    published(&state.db, &d, &ann.key, "2_b.png", "public");
    published(&state.db, &d, &ann.key, "3_c.png", "members");
    state.db.set_media_featured(first.id, true).unwrap();

    let featured = ListParams { featured: Some(1), ..Default::default() };
    let got = list(State(state.clone()), Query(featured)).await.unwrap().0;
    assert_eq!(ids(&got), vec![first.id], "only the featured item");
    let signed = ListParams { featured: Some(1), ..ann.list(now_ms()) };
    assert_eq!(ids(&list_post(State(state.clone()), Json(signed)).await.unwrap().0), vec![first.id], "the signed POST too");
    let one = ListParams { id: Some(first.id), ..Default::default() };
    assert_eq!(ids(&list(State(state.clone()), Query(one)).await.unwrap().0), vec![first.id], "and one item by its number");
}

/// A3. PICKS BY A SIGNED POST: `POST /api/media/picks` with the list's signed body and `of` gives a
/// member that person's picks, members-only ones included, and every item of a member's list says
/// whether that member picked it (`picked_by_me`); the public view carries no such field. Without
/// `of` it is refused.
///
/// Seen red 2026-10-11 with `picked_by_me` taken from the first item for all: "picked_by_me per
/// item / left: [Bool(true), Bool(true)] / right: [Bool(true), Bool(false)]".
#[tokio::test]
async fn picks_by_a_signed_post_and_picked_by_me() {
    let state = relay("mfu_a3");
    let d = dirs("mfu_a3");
    let (ann, bea) = (person(3), person(4));
    state.db.join_server(&ann.key, "Ann").unwrap();
    state.db.join_server(&bea.key, "Bea").unwrap();
    let open = published(&state.db, &d, &ann.key, "1_a.png", "public");
    let inner = published(&state.db, &d, &ann.key, "2_b.png", "members");
    state.db.pick_media(&bea.key, inner.id).unwrap();

    let body = ListParams { of: Some(bea.key.clone()), ..bea.list(now_ms()) };
    let picks = picks_post(State(state.clone()), Json(body)).await.unwrap().0;
    assert_eq!(ids(&picks), vec![inner.id], "Bea's pick, members only, to a member");
    let mine = list_post(State(state.clone()), Json(bea.list(now_ms() + 1))).await.unwrap().0;
    let flags: Vec<Value> = mine["items"].as_array().unwrap().iter().map(|i| i["picked_by_me"].clone()).collect();
    assert_eq!(ids(&mine), vec![inner.id, open.id]);
    assert_eq!(flags, vec![json!(true), json!(false)], "picked_by_me per item");
    let public = list(State(state.clone()), Query(ListParams::default())).await.unwrap().0;
    assert!(public["items"][0].get("picked_by_me").is_none(), "the public view carries no picked_by_me");
    let no_of = picks_post(State(state.clone()), Json(bea.list(now_ms() + 2))).await.unwrap_err();
    assert_eq!(no_of.0, StatusCode::BAD_REQUEST, "{}", no_of.1);
}

/// A4. A MEMBER'S OWN USE IS THE ROOM CHECK'S OWN NUMBER: `used_bytes` counts their items and their
/// uploads waiting to be published (with the names they had on their device) exactly as the room
/// check does, so a file that fits `share_bytes - used_bytes` passes it and one byte more does not;
/// the budget left, their uploads in progress and their role's Media file limit come with it. Not a
/// member: refused.
///
/// Seen red 2026-10-11 with `used_bytes` counting everyone's bytes (`media_bytes(None)`): "the
/// room check's own number / left: Number(6291456) / right: 5242880".
#[test]
fn a_members_own_use_is_the_room_checks_number() {
    let state = relay("mfu_a4");
    let db = &state.db;
    let d = dirs("mfu_a4");
    let (ann, out) = (person(5), person(6));
    db.join_server(&ann.key, "Ann").unwrap();
    let mut s = db.get_server_settings().unwrap();
    s.media_per_person_mb = 8;
    s.media_budget_mb = 20;
    db.record_upload(&ann.key, "1_item.mp4", 100, MEDIA_SHELF, "item.mp4", 3 * MIB).unwrap();
    std::fs::write(d.waiting.join("1_item.mp4"), b"x").unwrap();
    let body = PublishBody { key: ann.key.clone(), filename: "1_item.mp4".into(), ..Default::default() };
    publish_core(db, &s, &[], d.folders(), &body).unwrap();
    db.record_upload(&ann.key, "2_wait.mp4", 100, MEDIA_SHELF, "Clip from the beach.mp4", 2 * MIB).unwrap();
    db.record_upload("someone_else", "3_theirs.mp4", 100, MEDIA_SHELF, "theirs.mp4", MIB).unwrap();

    let mine = mine_core(db, &s, &ann.key).unwrap().0;
    assert_eq!(mine["used_bytes"], 5 * MIB, "the room check's own number");
    assert_eq!(mine["share_bytes"], 8 * MIB);
    assert_eq!(mine["budget_left_bytes"], 14 * MIB);
    assert_eq!((mine["in_progress_bytes"].as_i64(), mine["file_limit_bytes"].as_i64()), (Some(0), Some(0)));
    let waiting = mine["waiting"].as_array().unwrap();
    assert_eq!(waiting.len(), 1);
    assert_eq!((waiting[0]["filename"].as_str(), waiting[0]["original_name"].as_str()), (Some("2_wait.mp4"), Some("Clip from the beach.mp4")));
    let left = (8 * MIB - mine["used_bytes"].as_i64().unwrap()) as u64;
    assert!(media_room(db, &s, &ann.key, left, Reserved::default()).is_ok(), "what is left fits");
    assert!(media_room(db, &s, &ann.key, left + 1, Reserved::default()).is_err(), "one byte more does not");
    assert_eq!(mine_core(db, &s, &out.key).unwrap_err().0, StatusCode::FORBIDDEN);
}

/// A7. KEPT BY THE SERVER: an admin or a moderator keeps an item (a member cannot), and it stops
/// counting against its uploader's share while still counting against the Media budget, is left
/// out of the prune lists, says `kept`, and the usage shows it as its own line. Stopping puts it
/// back in the share even past it (nothing is deleted for that), and its uploader can still
/// remove it.
///
/// Seen red 2026-10-11 with the kept clause taken out of `media_bytes_on`: "a kept item stops
/// counting against its uploader's share / left: 6291456 / right: 0".
#[test]
fn a_kept_item_leaves_its_uploaders_share() {
    let state = relay("mfu_a7");
    let db = &state.db;
    let d = dirs("mfu_a7");
    let (ann, mo, bea) = (person(7), person(8), person(9));
    for (p, n) in [(&ann, "Ann"), (&mo, "Mo"), (&bea, "Bea")] {
        db.join_server(&p.key, n).unwrap();
    }
    db.set_role(&mo.key, "mod").unwrap();
    db.set_role(&bea.key, "verified").unwrap();
    let mut s = db.get_server_settings().unwrap();
    s.media_per_person_mb = 4;
    db.record_upload(&ann.key, "1_art.png", 100, MEDIA_SHELF, "art.png", 6 * MIB).unwrap();
    std::fs::write(d.waiting.join("1_art.png"), b"art").unwrap();
    let item = db
        .publish_media(&ann.key, &store::Publish { filename: "1_art.png", visibility: "public", ..Default::default() }, 1 << 40, 1 << 40, None)
        .unwrap();

    assert_eq!(keep_core(db, &bea.key, item.id, true).unwrap_err().0, StatusCode::FORBIDDEN, "a member cannot keep");
    keep_core(db, &mo.key, item.id, true).expect("a moderator keeps it");
    assert_eq!(db.media_bytes(Some(&ann.key)), 0, "a kept item stops counting against its uploader's share");
    assert_eq!(db.media_bytes(None), 6 * MIB, "it still counts against the Media budget");
    assert!(db.media_item(item.id).unwrap().unwrap().kept);
    let usage = db.media_usage().unwrap();
    assert_eq!(usage.kept_bytes, 6 * MIB, "the usage shows it as its own line");
    assert!(usage.largest.is_empty() && usage.oldest.is_empty() && usage.least_viewed.is_empty(), "no prune list has it");
    assert!(usage.people.iter().all(|p| p.key != ann.key), "nor Ann's share line");

    keep_core(db, &mo.key, item.id, false).unwrap();
    assert_eq!(db.media_bytes(Some(&ann.key)), 6 * MIB, "back in her share, past it");
    assert!(db.media_item(item.id).unwrap().is_some(), "and not deleted for that");
    assert!(media_room(db, &s, &ann.key, 1, Reserved::default()).is_err(), "the share stops her next upload");
    keep_core(db, &mo.key, item.id, true).unwrap();

    // A keep is undone only by the same or a higher role (2026-10-12). Seen red with the rank
    // check taken out of keep_core: the moderator's stop came back Ok, so `unwrap_err` panicked.
    let ad = person(10);
    db.join_server(&ad.key, "Ada").unwrap();
    db.set_role(&ad.key, "admin").unwrap();
    keep_core(db, &ad.key, item.id, true).expect("an admin keeps it");
    let refused = keep_core(db, &mo.key, item.id, false).unwrap_err();
    assert_eq!(refused.0, StatusCode::FORBIDDEN, "a moderator cannot stop an admin's keep");
    assert!(refused.1.contains("only an admin"), "and is told why: {}", refused.1);
    keep_core(db, &mo.key, item.id, true).expect("a moderator keeping it again is fine");
    assert_eq!(db.media_kept_by(item.id).unwrap().as_deref(), Some(ad.key.as_str()), "and leaves it kept by the admin");
    keep_core(db, &ad.key, item.id, false).expect("the admin can stop keeping it");
    keep_core(db, &mo.key, item.id, true).unwrap();
    keep_core(db, &ad.key, item.id, false).expect("an admin can stop a moderator's keep");
    keep_core(db, &mo.key, item.id, true).unwrap();
    remove_core(db, d.folders(), &RemoveTarget::Item(item.id), &ann.key).expect("the uploader can still remove a kept item");
}

/// W5. A STALE TILE NEVER ACTS ON ANOTHER ITEM: item numbers are never reused (the newest item
/// removed, the next one gets a new number), and Remove, Feature, Keep, Visibility and Pick must
/// name the item's file too: another item's file is refused (409) and so is none (400), with
/// nothing changed.
///
/// Seen red 2026-10-11 with `media_items.id` without AUTOINCREMENT: "a removed item's number is
/// never given again / left: 2 / right: 3"; and with the check taken out of `feature`: "another
/// item's file is refused / left: Ok(()) / right: Err(409)".
#[tokio::test]
async fn a_stale_tile_never_acts_on_another_item() {
    let state = relay("mfu_w5");
    let d = dirs("mfu_w5");
    let (ann, op) = (person(10), person(11));
    state.db.join_server(&ann.key, "Ann").unwrap();
    state.db.join_server(&op.key, "Op").unwrap();
    state.db.set_role(&op.key, "admin").unwrap();
    published(&state.db, &d, &ann.key, "1_a.png", "public");
    let newest = published(&state.db, &d, &ann.key, "2_b.png", "public");
    remove_core(&state.db, d.folders(), &RemoveTarget::Item(newest.id), &ann.key).unwrap();
    let next = published(&state.db, &d, &ann.key, "3_c.png", "public");
    assert_eq!(next.id, newest.id + 1, "a removed item's number is never given again");

    let now = now_ms();
    let feature_named = |file: &str, ts: u64| FeatureBody {
        key: op.key.clone(),
        timestamp: ts,
        sig: op.sign(&format!("media_feature:{}", next.id), ts),
        id: next.id,
        featured: true,
        filename: file.into(),
    };
    let stale = feature(State(state.clone()), Json(feature_named("2_b.png", now))).await;
    assert_eq!(stale.map(|_| ()).map_err(|e| e.0), Err(StatusCode::CONFLICT), "another item's file is refused");
    let unnamed = feature(State(state.clone()), Json(feature_named("", now + 1))).await.unwrap_err();
    assert_eq!((unnamed.0, unnamed.1.as_str()), (StatusCode::BAD_REQUEST, NAME_THE_FILE));
    assert!(!state.db.media_item(next.id).unwrap().unwrap().featured, "nothing changed");
    feature(State(state.clone()), Json(feature_named("/uploads/3_c.png", now + 2))).await.expect("its own file, as its address");

    let keep_b = KeepBody { key: op.key.clone(), timestamp: now, sig: op.sign(&keep_purpose(next.id, true), now), id: next.id, kept: true, filename: "1_a.png".into() };
    assert_eq!(keep(State(state.clone()), Json(keep_b)).await.unwrap_err().0, StatusCode::CONFLICT, "keep");
    let vis = VisibilityBody { key: ann.key.clone(), timestamp: now, sig: ann.sign(&format!("media_visibility:{}:members", next.id), now), id: next.id, visibility: "members".into(), filename: "1_a.png".into() };
    assert_eq!(visibility(State(state.clone()), Json(vis)).await.unwrap_err().0, StatusCode::CONFLICT, "visibility");
    let pick_b = ItemBody { key: op.key.clone(), timestamp: now, sig: op.sign(&format!("media_pick:{}", next.id), now), id: next.id, filename: "1_a.png".into() };
    assert_eq!(pick(State(state.clone()), Json(pick_b)).await.unwrap_err().0, StatusCode::CONFLICT, "pick");
    let rm = RemoveBody { key: op.key.clone(), timestamp: now, sig: op.sign(&format!("media_remove:{}", next.id), now), id: Some(next.id), filename: Some("1_a.png".into()) };
    assert_eq!(remove(State(state.clone()), Json(rm)).await.unwrap_err().0, StatusCode::CONFLICT, "remove");
    let rm_unnamed = RemoveBody { id: Some(next.id), filename: None, ..Default::default() };
    assert_eq!(rm_unnamed.target().unwrap_err().0, StatusCode::BAD_REQUEST, "a removal by number names the file too");
    assert!(state.db.media_item(next.id).unwrap().is_some(), "the item is still there");
}

/// A request for the members' file route.
fn file_request(id: i64, header: Option<&str>) -> axum::extract::Request {
    let mut b = axum::http::Request::builder().uri(format!("/api/media/file/{id}"));
    if let Some(t) = header {
        b = b.header(UPLOAD_TOKEN_HEADER, t);
    }
    b.body(axum::body::Body::empty()).unwrap()
}

/// W3b. THE MEMBERS' FILE BY A HEADER: the session's upload token in `X-Upload-Token` opens a
/// members-only file, so an app never puts the token in an address; `?token=` still works; a
/// header that is not a session is refused.
///
/// Seen red 2026-10-11 with `file_from` reading only the query: "the header opens the file /
/// left: 401 / right: 200".
#[tokio::test]
async fn the_members_file_opens_with_the_token_in_a_header() {
    let state = relay("mfu_w3b");
    let d = dirs("mfu_w3b");
    let ann = person(12);
    state.db.join_server(&ann.key, "Ann").unwrap();
    let item = published(&state.db, &d, &ann.key, "1_case.glb", "members");
    state.upload_tokens.write().await.insert("tok_ann".into(), ann.key.clone());

    let by_header = file_from(&state, d.folders(), item.id, TokenQuery::default(), file_request(item.id, Some("tok_ann"))).await;
    assert_eq!(by_header.status().as_u16(), 200, "the header opens the file");
    let bytes = axum::body::to_bytes(by_header.into_body(), 1 << 20).await.unwrap();
    assert_eq!(&bytes[..], b"0123456789");
    let by_query = file_from(&state, d.folders(), item.id, TokenQuery { token: Some("tok_ann".into()), ..Default::default() }, file_request(item.id, None)).await;
    assert_eq!(by_query.status().as_u16(), 200, "the query still works");
    let forged = file_from(&state, d.folders(), item.id, TokenQuery::default(), file_request(item.id, Some("forged"))).await;
    assert_eq!(forged.status().as_u16(), 403);
    let none = file_from(&state, d.folders(), item.id, TokenQuery::default(), file_request(item.id, None)).await;
    assert_eq!(none.status().as_u16(), 401);
}

/// R5. AN UPLOAD IS COPIED INTO PLACE OUTSIDE THE ROOM LOCK, under a hidden name: while `put` runs
/// the lock is free (a copy of many GB never holds every other upload waiting), the file it writes
/// is hidden (a dot name, never counted or served), and a `put` that fails part way leaves
/// nothing behind and no row. The start-up sweep clears hidden leftovers, and /uploads never
/// serves a dot name, written plainly or as `%2e`.
///
/// Seen red 2026-10-11 with the room lock held across `put` (as it was before R5): "the room lock
/// is free while the file is copied".
#[test]
fn an_upload_is_copied_into_place_outside_the_room_lock() {
    let state = relay("mfu_r5");
    let db = &state.db;
    let d = dirs("mfu_r5");
    let s = db.get_server_settings().unwrap();
    let u = NewUpload { key: "ann", shelf: 0, name: "1_a.png", original_name: "a.png", len: 5, keep: 4, private_held_limit: None, allowance: None, session: None };
    let mut free_while_putting = false;
    let mut staged_name = String::new();
    store_upload(
        db,
        &s,
        d.folders(),
        &u,
        |path| {
            // Another thread takes the room lock while the file is being put (other tests may hold
            // it for a moment, so it is given a while; held across `put`, it never gets it here).
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _room = room_lock();
                let _ = tx.send(());
            });
            free_while_putting = rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok();
            staged_name = path.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::write(path, b"hello")
        },
        |path| {
            let _ = std::fs::remove_file(path);
        },
    )
    .unwrap();
    assert!(free_while_putting, "the room lock is free while the file is copied");
    assert!(staged_name.starts_with(store::STAGED_PREFIX), "it arrives under a hidden name: {staged_name}");
    assert_eq!(std::fs::read(d.uploads.join("1_a.png")).unwrap(), b"hello");

    let failing = NewUpload { name: "2_b.png", ..u };
    let failed = store_upload(
        db,
        &s,
        d.folders(),
        &failing,
        |path| {
            std::fs::write(path, b"half a fi")?;
            Err(std::io::Error::other("the disk filled part way"))
        },
        |path| {
            let _ = std::fs::remove_file(path);
        },
    );
    assert_eq!(failed.unwrap_err().0, StatusCode::INTERNAL_SERVER_ERROR);
    let left: Vec<String> = std::fs::read_dir(&d.uploads).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, vec!["1_a.png".to_string()], "a failed copy leaves nothing behind");
    assert_eq!(db.get_upload_count("ann").unwrap(), 1, "and no row");

    std::fs::write(d.uploads.join(format!("{}left", store::STAGED_PREFIX)), vec![0u8; 4096]).unwrap();
    assert_eq!(store::folder_bytes(&d.uploads), 5, "a hidden file is never counted");
    assert_eq!(store::clear_staged_files(&d.uploads), 1, "and the start-up sweep clears it");
    for path in ["/uploads/.incoming-x", "/uploads/%2Eincoming-x", "/uploads/a/.b"] {
        assert!(crate::relay::hidden_upload_path(path), "{path} is never served");
    }
    assert!(!crate::relay::hidden_upload_path("/uploads/1_a.png"));
}

/// R6. A MEDIA UPLOAD WAITS WHERE NOTHING SERVES IT: stored, it is in the waiting folder, not at its
/// `/uploads/` address; published public it moves there, published members only it moves to the
/// members' folder; the room checks count it while it waits, and a removal by name deletes it
/// from there. A chat upload goes straight to the public folder, as before.
///
/// Seen red 2026-10-11 with `store_upload` putting every shelf in the public folder: "a Media
/// upload is at no public address while it waits".
#[test]
fn a_media_upload_waits_where_nothing_serves_it() {
    let state = relay("mfu_r6");
    let db = &state.db;
    let d = dirs("mfu_r6");
    let ann = person(13);
    db.join_server(&ann.key, "Ann").unwrap();
    let s = db.get_server_settings().unwrap();
    let put = |name: &'static str, shelf: i64| {
        let u = NewUpload { key: &ann.key, shelf, name, original_name: name, len: 4, keep: 4, private_held_limit: None, allowance: None, session: None };
        store_upload(db, &s, d.folders(), &u, |p| std::fs::write(p, b"data"), |p| {
            let _ = std::fs::remove_file(p);
        })
        .unwrap();
    };
    put("1_pub.png", MEDIA_SHELF);
    put("2_mem.mp4", MEDIA_SHELF);
    put("3_gone.mp4", MEDIA_SHELF);
    put("4_chat.png", 0);
    assert!(!d.uploads.join("1_pub.png").exists(), "a Media upload is at no public address while it waits");
    assert!(d.waiting.join("1_pub.png").is_file() && d.uploads.join("4_chat.png").is_file(), "it waits; a chat file is public");
    assert_eq!(stored_bytes(d.folders()), 16, "the room counts what waits");
    assert_eq!(db.media_bytes(Some(&ann.key)), 12);

    let publish = |file: &str, visibility: &str| {
        let body = PublishBody { key: ann.key.clone(), filename: file.into(), visibility: Some(visibility.into()), ..Default::default() };
        publish_core(db, &s, &[], d.folders(), &body).unwrap();
    };
    publish("1_pub.png", "public");
    publish("2_mem.mp4", "members");
    assert!(d.uploads.join("1_pub.png").is_file() && !d.waiting.join("1_pub.png").exists(), "published public, it moves to its address");
    assert!(d.members.join("2_mem.mp4").is_file() && !d.uploads.join("2_mem.mp4").exists(), "members only, to the members' folder");
    remove_core(db, d.folders(), &RemoveTarget::Upload("3_gone.mp4".into()), &ann.key).unwrap();
    assert!(!d.waiting.join("3_gone.mp4").exists(), "a removal by name deletes it where it waits");
}

/// B5. FILES FROM PRIVATE CONVERSATIONS: each person may hold their role's `max_private_held_mb`
/// of them at once, refused past it with words that say it is THEIR room and when their oldest
/// leaves; they leave after the DM mailbox lifetime (the expiry pass, row and file); and they are
/// out of the keep-N FIFO, so they never push out the person's room posts (nor the reverse).
///
/// Seen red 2026-10-11 with the private room check taken out of `room_for`: "the person's own room
/// for private files is full"; and with the `.enc` clause taken out of `record_upload`'s FIFO:
/// "private files never push out room posts (file 0): [\"1_room.png\"]".
#[test]
fn private_files_have_their_own_room_and_lifetime() {
    let state = relay("mfu_b5");
    let db = &state.db;
    let d = dirs("mfu_b5");
    let s = db.get_server_settings().unwrap();
    let put = |name: &'static str, len: u64, limit: Option<u64>, keep: i64| {
        let u = NewUpload { key: "ann", shelf: 0, name, original_name: name, len, keep, private_held_limit: limit, allowance: None, session: None };
        store_upload(db, &s, d.folders(), &u, |p| std::fs::write(p, vec![1u8; len as usize]), |p| {
            let _ = std::fs::remove_file(p);
        })
    };
    let limit = Some(3 * MIB as u64);
    put("1_room.png", 10, None, 1).unwrap();
    for (n, name) in ["2_a.enc", "3_b.enc"].into_iter().enumerate() {
        let pruned = put(name, MIB as u64, limit, 1).unwrap();
        assert!(pruned.is_empty(), "private files never push out room posts (file {n}): {pruned:?}");
    }
    let full = put("4_c.enc", 2 * MIB as u64, limit, 1);
    let (code, words) = full.err().unwrap_or_else(|| panic!("the person's own room for private files is full"));
    assert_eq!(code, StatusCode::INSUFFICIENT_STORAGE);
    assert!(words.starts_with("Your private files on this server are full (2.0 MB of 3.0 MB). The oldest leaves on "), "{words}");
    assert!(words.ends_with(", or remove some from Settings > Data."), "{words}");
    assert!(put("5_d.enc", MIB as u64, limit, 1).is_ok(), "one that fits goes");
    assert_eq!(db.get_upload_count("ann").unwrap(), 4, "the room post is still there");

    let month = 31 * 24 * 60 * 60 * 1000i64;
    db.with_conn(|c| c.execute("UPDATE user_uploads SET uploaded_at = uploaded_at - ?1 WHERE filename IN ('2_a.enc', '1_room.png')", [month])).unwrap();
    db.run_expiry_sweeps_in(d.folders());
    assert!(!d.uploads.join("2_a.enc").exists(), "a private file past the DM lifetime goes");
    assert!(d.uploads.join("3_b.enc").exists() && d.uploads.join("1_room.png").exists(), "a newer one stays, and room posts are not touched");
    assert_eq!(db.private_files_held("ann").unwrap().0, 2 * MIB);
}

/// EVERY ITEM NAMES ITS STORED FILE, members-only ones too: a members-only item's `url` carries no
/// name (/api/media/file/<id>), and every write now requires `filename` (W5), so without the field
/// neither app could pick, remove, feature or keep one. Read from the members' list as the apps
/// read it, the name is there and a remove with it goes through.
///
/// Seen red 2026-10-11 with the field left out of `item_json`: "a members-only item names its
/// stored file / left: Null".
#[test]
fn a_members_only_item_names_its_stored_file() {
    let state = relay("mfu_filename");
    let db = &state.db;
    let d = dirs("mfu_filename");
    let ann = person(41);
    db.join_server(&ann.key, "Ann").unwrap();
    published(db, &d, &ann.key, "1_inner.png", "members");
    let s = db.get_server_settings().unwrap();
    let list = list_core(db, &s, &ListParams::default(), Some(&ann.key), None).unwrap().0;
    let item = list["items"].as_array().unwrap().first().expect("the members' list shows it").clone();
    assert!(item["url"].as_str().unwrap().starts_with("/api/media/file/"), "test setup: a members-only address");
    assert_eq!(item["filename"], "1_inner.png", "a members-only item names its stored file");
    let id = item["id"].as_i64().unwrap();
    let name = item["filename"].as_str().unwrap().to_string();
    same_item(db, id, &name).expect("the name it gives is the name a write must send");
    remove_core(db, d.folders(), &RemoveTarget::Item(id), &ann.key).expect("and the remove goes through");
}

/// A PRIVATE FILE NEVER LEAVES BEFORE ITS DM: the mailbox keeps what arrived on day D until day
/// D + the lifetime has ended (dms.rs `mailbox_expire`), so a file uploaded late on day D stays
/// past the lifetime counted in milliseconds and leaves when the day after D + the lifetime begins,
/// which is also what `leaves_at` tells the person. (Persisted-format review, 2026-10-11.)
///
/// Seen red 2026-10-11 with the millisecond cutoff put back: "a file uploaded late on its day is
/// still there on its last day".
#[test]
fn a_private_file_never_leaves_before_its_dm() {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    const HOUR: i64 = 60 * 60 * 1000;
    let state = relay("mfu_day_rule");
    let db = &state.db;
    let d = dirs("mfu_day_rule");
    let day0: i64 = 20_000; // a day number well after 1970
    let uploaded = day0 * DAY + 23 * HOUR;
    std::fs::write(d.uploads.join("1_late.enc"), b"sealed").unwrap();
    db.record_upload("ann", "1_late.enc", 100, 0i64, "late.png", 6).unwrap();
    db.with_conn(|c| c.execute("UPDATE user_uploads SET uploaded_at = ?1 WHERE filename = '1_late.enc'", [uploaded])).unwrap();

    let leaves = crate::relay::storage::private_files::leaves_at(uploaded, 30);
    assert_eq!(leaves, (day0 + 31) * DAY, "it leaves when the day after its day plus 30 begins");
    // Thirty days and thirty minutes after the upload: past the lifetime in milliseconds, but
    // still the DM's last day.
    db.expire_private_files_in(d.folders(), 30, (day0 + 30) * DAY + 23 * HOUR + HOUR / 2).unwrap();
    assert!(d.uploads.join("1_late.enc").exists(), "a file uploaded late on its day is still there on its last day");
    db.expire_private_files_in(d.folders(), 30, leaves + 60_000).unwrap();
    assert!(!d.uploads.join("1_late.enc").exists(), "and it goes once that day has ended");
}
