//! Media round 5, lane 1A (2026-10-12; docs/design/media-round-5-plan.md 3.4 and section 9): an
//! item edited where it is, files added to it and taken off it, and every one of its files moving,
//! counting and going with it (relay/media_edit.rs, and relay/media.rs `visibility_core` and
//! `remove_core`). Each test names the break it was seen failing against ("Seen red"), with the
//! words it failed with.
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, and nothing is written outside a temporary folder.

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use crate::relay::media::{file_from, publish_core, remove_core, visibility_core, PublishBody, RemoveTarget, TokenQuery, UPLOAD_TOKEN_HEADER};
use crate::relay::media_licences::{parse_licences, NOT_FOR_GAME, NOT_OFFERED};
use crate::relay::storage::media::{shown_name_for, MediaItem, ITEM_FILES_MAX, MEDIA_SHELF, OUT_OF_DATE};

/// A person: their key (the Dilithium public key in hex) and the key pair that signs for them.
struct Person {
    key: String,
    pair: DilithiumKeypair,
}

fn person(seed: u8) -> Person {
    let pair = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    Person { key: hex::encode(pair.public_key()), pair }
}

impl Person {
    /// A signature over `purpose\ntimestamp`, as the apps make it.
    fn sign(&self, purpose: &str, ts: u64) -> String {
        hex::encode(self.pair.sign(format!("{purpose}\n{ts}").as_bytes()))
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
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
    let d = |what: &str| crate::test_temp::dir(&format!("{tag}_{what}"));
    Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting") }
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
    /// Which folder `name` is in: `Some(Spot::Public)` is the one served at `/uploads`.
    fn spot(&self, name: &str) -> Option<Spot> {
        self.folders().spot_of(name)
    }
}

/// What `GET /uploads/<name>` answers: the public folder served as relay/mod.rs serves it.
async fn public_status(d: &Dirs, name: &str) -> u16 {
    let req = axum::http::Request::builder().uri(format!("/{name}")).body(axum::body::Body::empty()).unwrap();
    tower_http::services::ServeDir::new(d.uploads.path()).try_call(req).await.unwrap().status().as_u16()
}

/// `key` uploads `file` for Media: it waits in the waiting folder, with its row on the Media shelf.
fn waiting(db: &Storage, d: &Dirs, key: &str, file: &str, bytes: &[u8]) {
    std::fs::write(d.waiting.join(file), bytes).unwrap();
    db.record_upload(key, file, 100, MEDIA_SHELF, &format!("own {file}"), bytes.len() as i64).unwrap();
}

/// `key` uploads `file` for Media and publishes it with `visibility`.
fn published(db: &Storage, d: &Dirs, key: &str, file: &str, visibility: &str) -> MediaItem {
    waiting(db, d, key, file, b"0123456789");
    let s = db.get_server_settings().unwrap();
    let body = PublishBody { key: key.into(), filename: file.into(), visibility: Some(visibility.into()), ..Default::default() };
    let answer = publish_core(db, &s, &[], d.folders(), &body).expect("published").0;
    db.media_item(answer["item"]["id"].as_i64().unwrap()).unwrap().unwrap()
}

/// `key` adds their waiting upload `file` to `item` as `role`, through the route's core.
fn add(db: &Storage, d: &Dirs, key: &str, item: &MediaItem, file: &str, role: &str) -> Answer {
    let b = FileAddBody { key: key.into(), id: item.id, filename: item.upload_filename.clone(), add: file.into(), role: role.into(), ..Default::default() };
    files_add_core(db, d.folders(), &b)
}

/// `key` takes file `file` (stored as `name`) off `item`, through the route's core.
fn take_off(db: &Storage, d: &Dirs, key: &str, item: &MediaItem, file: i64, name: &str) -> Answer {
    let b = FileRemoveBody { key: key.into(), id: item.id, filename: item.upload_filename.clone(), file, file_name: name.into(), ..Default::default() };
    files_remove_core(db, d.folders(), &b)
}

/// The data folder's licences file as it ships, with its Creative Commons warning written (so the
/// CC licences are offered whatever the copy says today).
fn offered() -> LicencesFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/media/licences.json");
    let mut file = parse_licences(&std::fs::read_to_string(path).unwrap()).unwrap();
    if file.cc_warning.trim().is_empty() {
        file.cc_warning = "A Creative Commons licence cannot be taken back for copies already taken.".into();
    }
    file
}

/// An edit of `item` by `key` setting every field: `title`, an empty description, General, no
/// labels, `licence` and the game tick.
fn edit_of(key: &str, item: &MediaItem, title: &str, licence: &str, project_use: bool) -> EditBody {
    EditBody {
        key: key.into(),
        timestamp: 0,
        sig: String::new(),
        id: item.id,
        filename: item.upload_filename.clone(),
        edit: EditFields { title: title.into(), description: String::new(), category_id: 1, labels: vec![], licence: licence.into(), project_use },
    }
}

/// A stand-in for the edit's signature, kept with a licence choice.
fn signed(key: &str) -> Signed<'_> {
    Signed { key, purpose: "media_edit:test", body: "{}", ts: 1, sig: "sig" }
}

fn words(r: Answer) -> (StatusCode, String) {
    match r {
        Err(refused) => refused,
        Ok(answer) => panic!("expected a refusal, got {answer:?}"),
    }
}

fn count(db: &Storage, sql: &str) -> i64 {
    db.with_conn(|c| c.query_row(sql, [], |r| r.get(0))).unwrap()
}

/// THE UPLOADER EDITS AN ITEM, AND THE SIGNATURE COVERS THE EDIT AS SENT: Ann's app writes the
/// edit with its own spacing and key order and signs `media_edit:<id>:<BLAKE3 of that text>`; the
/// relay accepts it and applies every field. The same signature on a body whose edit was reworded
/// is refused (401), the same request sent twice is refused (409, replay), and a licence change is
/// kept with the edit's text exactly as sent, so the hash in its purpose can be checked again.
///
/// Seen red 2026-10-12 against two breaks. (1) The hash taken over the parsed edit, re-serialised
/// (`serde_json::from_str::<Value>(sent).to_string()` in `edit_request`): "the edit as signed:
/// (401, \"Signature verification failed.\")". (2) The purpose without the edit's hash
/// (`edit_purpose` answering `media_edit:<id>`): "a reworded edit with the old signature / left:
/// Ok(String(\"Someone else's words\")) / right: Err(401)" (the reworded body kept the old
/// signature).
#[tokio::test]
async fn the_uploader_edits_an_item_and_the_signature_covers_the_edit_as_sent() {
    let state = relay("me_sig");
    let d = dirs("me_sig");
    let ann = person(1);
    state.db.join_server(&ann.key, "Ann").unwrap();
    let item = published(&state.db, &d, &ann.key, "1_dusk.png", "public");
    let body = |edit: &str, ts: u64, sig: &str| {
        format!(r#"{{"key":"{}","timestamp":{ts},"sig":"{sig}","id":{},"filename":"{}","edit":{edit}}}"#, ann.key, item.id, item.upload_filename)
    };
    // As Ann's app wrote it: spacing and key order of its own, which the hash keeps.
    let edit = r#"{ "title": "Dusk at the lake", "labels": [ "flashing" ], "description": "First line\nThe rest", "category_id": 1, "licence": "all_rights_reserved", "project_use": false }"#;
    let ts = now_ms();
    let sig = ann.sign(&edit_purpose(item.id, edit), ts);

    let reworded = edit.replace("Dusk at the lake", "Someone else's words");
    let stolen = edit_signed(&state, &body(&reworded, ts, &sig)).await.map(|j| j.0["item"]["title"].clone());
    assert_eq!(stolen.map_err(|e| e.0.as_u16()), Err(401), "a reworded edit with the old signature");

    let answer = edit_signed(&state, &body(edit, ts, &sig)).await.map_err(|e| (e.0.as_u16(), e.1)).expect("the edit as signed").0;
    assert_eq!(answer["ok"], true);
    let shown = &answer["item"];
    assert_eq!((shown["title"].as_str(), shown["description"].as_str()), (Some("Dusk at the lake"), Some("First line\nThe rest")));
    assert_eq!(shown["labels"], json!(["flashing"]));
    assert_eq!(state.db.media_item(item.id).unwrap().unwrap().title, "Dusk at the lake", "kept");
    let again = edit_signed(&state, &body(edit, ts, &sig)).await;
    assert_eq!(again.map(|_| ()).map_err(|e| e.0.as_u16()), Err(409), "the same request sent twice");

    // A licence change: kept with the edit's text as sent and the purpose signed over its hash.
    let cc = r#"{"title":"Dusk at the lake","description":"","category_id":1,"labels":[],"licence":"cc_by_4","project_use":true}"#;
    assert!(
        crate::relay::media_licences::licences().offered_one("cc_by_4").is_some(),
        "test setup: this relay offers CC BY 4.0 (data/media/licences.json carries its cc_warning)"
    );
    let ts = now_ms() + 1;
    let purpose = edit_purpose(item.id, cc);
    edit_signed(&state, &body(cc, ts, &ann.sign(&purpose, ts))).await.map_err(|e| (e.0.as_u16(), e.1)).expect("Ann changes her licence");
    let log = state.db.media_licence_log(item.id).unwrap();
    let last = log.last().expect("the choice is kept");
    assert_eq!((last.licence.as_str(), last.project_use, last.set_by.as_str()), ("cc_by_4", true, ann.key.as_str()));
    assert_eq!((last.signed_body.as_str(), last.signed_purpose.as_str()), (cc, purpose.as_str()), "the edit as sent, and what was signed");
    assert_eq!(edit_purpose(item.id, &last.signed_body), last.signed_purpose, "the hash can be checked again");

    let other = published(&state.db, &d, &ann.key, "2_other.png", "public");
    let ts = now_ms() + 2;
    let stale = body(edit, ts, &ann.sign(&edit_purpose(item.id, edit), ts)).replace("1_dusk.png", &other.upload_filename);
    assert_eq!(edit_signed(&state, &stale).await.map(|_| ()).map_err(|e| e.0.as_u16()), Err(409), "another item's file (W5)");
}

/// AN ADMIN EDITS THE WORDS BUT NOT THE LICENCE (decision 1): an admin changes another person's
/// title, description, category and labels; changing the licence or the game tick is refused
/// (403) and changes nothing, words included; a moderator and another member edit nothing. The
/// uploader's own licence change is kept in the log, and nothing of the admin's is.
///
/// Seen red 2026-10-12 with the licence rule removed (the `licence_changes && uploader !=
/// requester` refusal in storage/media_files.rs `edit_media`, the one place the rule lives; the
/// route has no copy of it): "an admin cannot change the licence: Json(Object {\"item\": ...".
#[test]
fn an_admin_edits_the_words_but_not_the_licence() {
    let state = relay("me_admin");
    let d = dirs("me_admin");
    let db = &state.db;
    for (key, name) in [("ann", "Ann"), ("root", "Root"), ("mo", "Mo"), ("cal", "Cal")] {
        db.join_server(key, name).unwrap();
    }
    db.set_role("root", "admin").unwrap();
    db.set_role("mo", "mod").unwrap();
    let item = published(db, &d, "ann", "1_staff.glb", "public");
    let offered = offered();
    let edit = |key: &str, title: &str, licence: &str, game: bool| edit_core(db, &[], &offered, &edit_of(key, &item, title, licence, game), &signed(key));

    edit("ann", "Staff", "cc_by_4", false).expect("the uploader picks CC BY 4.0");
    let answer = edit("root", "Fire staff, tidied by an admin", "cc_by_4", false).expect("an admin edits the words").0;
    assert_eq!(answer["item"]["title"], "Fire staff, tidied by an admin");

    match edit("root", "An admin's title with a new licence", "cc0_1", false) {
        Err((code, w)) => {
            assert_eq!(code, StatusCode::FORBIDDEN);
            assert_eq!(w, "Only the person who put it on the Media page can change its licence or offer it for the game.");
        }
        Ok(answer) => panic!("an admin cannot change the licence: {answer:?}"),
    }
    assert_eq!(words(edit("root", "Fire staff", "cc_by_4", true)).0, StatusCode::FORBIDDEN, "nor offer it for the game");
    let now = db.media_item(item.id).unwrap().unwrap();
    assert_eq!((now.title.as_str(), now.licence.as_str(), now.project_use), ("Fire staff, tidied by an admin", "cc_by_4", false), "nothing changed");

    let mo = words(edit("mo", "A moderator's title", "cc_by_4", false));
    assert_eq!(mo, (StatusCode::FORBIDDEN, "Only the person who put it there, or an admin, can edit a Media item.".to_string()), "moderators edit nothing");
    assert_eq!(words(edit("cal", "Cal's title", "cc_by_4", false)).0, StatusCode::FORBIDDEN, "nor does another member");
    assert_eq!(words(edit("stranger", "x", "cc_by_4", false)), (StatusCode::FORBIDDEN, MEMBERS_ONLY.to_string()));

    let log = db.media_licence_log(item.id).unwrap();
    assert_eq!(log.iter().map(|c| (c.set_by.as_str(), c.licence.as_str())).collect::<Vec<_>>(), vec![("ann", "cc_by_4")], "the uploader's choice alone");
}

/// A CREATIVE COMMONS LICENCE IS REFUSED WHILE ITS WARNING IS MISSING (decision 12, plan 3.7): with
/// a licences file whose `cc_warning` is empty, choosing CC BY 4.0 is refused with the words "That
/// licence is not offered on this server yet." and changes nothing; the game tick is refused
/// under a licence that does not allow it (all rights reserved, CC BY-SA); with the warning
/// written, CC BY 4.0 and the tick go. An item keeps a licence the server stops offering: its
/// words can still be edited. A made-up licence is never offered.
///
/// Seen red 2026-10-12 with the route's `check_choice` skipped (offered() never consulted): "CC BY
/// 4.0 is not offered without its warning: Json(Object {\"item\": ...".
#[test]
fn a_cc_licence_is_refused_while_its_warning_is_missing() {
    let state = relay("me_cc");
    let d = dirs("me_cc");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    let item = published(db, &d, "ann", "1_dusk.png", "public");
    let with_warning = offered();
    let mut quiet = offered();
    quiet.cc_warning.clear();
    let edit = |file: &LicencesFile, title: &str, licence: &str, game: bool| edit_core(db, &[], file, &edit_of("ann", &item, title, licence, game), &signed("ann"));

    match edit(&quiet, "Dusk", "cc_by_4", false) {
        Err(refused) => assert_eq!(refused, (StatusCode::BAD_REQUEST, NOT_OFFERED.to_string())),
        Ok(answer) => panic!("CC BY 4.0 is not offered without its warning: {answer:?}"),
    }
    assert_eq!(db.media_item(item.id).unwrap().unwrap().licence, "all_rights_reserved", "nothing changed");
    assert_eq!(words(edit(&with_warning, "Dusk", "all_rights_reserved", true)), (StatusCode::BAD_REQUEST, NOT_FOR_GAME.to_string()));
    assert_eq!(words(edit(&with_warning, "Dusk", "cc_by_sa_4", true)), (StatusCode::BAD_REQUEST, NOT_FOR_GAME.to_string()));
    assert_eq!(words(edit(&with_warning, "Dusk", "made_up", false)).1, NOT_OFFERED);

    edit(&with_warning, "Dusk", "cc_by_4", true).expect("with its warning, CC BY 4.0 and the game tick go");
    let kept = edit(&quiet, "Dusk, renamed", "cc_by_4", true).expect("an item keeps a licence the server stops offering").0;
    assert_eq!((kept["item"]["title"].as_str(), kept["item"]["licence"].as_str()), (Some("Dusk, renamed"), Some("cc_by_4")));
    assert_eq!(words(edit(&quiet, "Dusk", "cc0_1", true)).1, NOT_OFFERED, "but a change is checked");
}

/// ADD A FILE: IT MOVES INTO THE ITEM'S FOLDER. Through the signed route, Ann adds a waiting upload
/// to her members-only item: it leaves the waiting folder for the members' one, `GET /uploads/<it>`
/// answers 404, its address on the item is the members' route with `?file=<n>`, and that route
/// serves it to a member. Added to a public item, a file goes to the public folder and is served
/// at `/uploads/`.
///
/// Seen red 2026-10-12 with the file left in the waiting folder (the route's `place` answering
/// `Ok(false)` without moving it): "the added file is in the members' folder / left: Some(Waiting)
/// / right: Some(Members)".
#[tokio::test]
async fn add_a_file_moves_it_into_the_items_folder() {
    let state = relay("me_add");
    let d = dirs("me_add");
    let ann = person(1);
    state.db.join_server(&ann.key, "Ann").unwrap();
    state.upload_tokens.write().await.insert("tok_ann".into(), ann.key.clone());
    let item = published(&state.db, &d, &ann.key, "1_clip.mp4", "members");
    waiting(&state.db, &d, &ann.key, "2_textures.zip", b"TEXTURES");

    let ts = now_ms();
    let b = FileAddBody {
        key: ann.key.clone(),
        timestamp: ts,
        sig: ann.sign(&file_add_purpose(item.id, "2_textures.zip", "file"), ts),
        id: item.id,
        filename: item.upload_filename.clone(),
        add: "/uploads/2_textures.zip".into(),
        role: "file".into(),
    };
    let answer = files_add_in(state.clone(), OwnedFolders::of(d.folders()), b).await.map_err(|e| (e.0.as_u16(), e.1)).expect("added").0;
    let n = answer["item"]["files"][1]["file"].as_i64().expect("the added file is file 1 on the wire");
    assert_eq!(answer["item"]["files"][1]["url"], format!("/api/media/file/{}?file={n}", item.id));
    assert_eq!(answer["item"]["file_count"], 2);
    assert_eq!(d.spot("2_textures.zip"), Some(Spot::Members), "the added file is in the members' folder");
    assert_eq!(public_status(&d, "2_textures.zip").await, 404, "and not at /uploads/");
    let mut req = axum::http::Request::builder().uri(format!("/api/media/file/{}?file={n}", item.id));
    req = req.header(UPLOAD_TOKEN_HEADER, "tok_ann");
    let res = file_from(&state, d.folders(), item.id, TokenQuery { file: Some(n), ..Default::default() }, req.body(axum::body::Body::empty()).unwrap()).await;
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().as_ref(), b"TEXTURES");

    let open = published(&state.db, &d, &ann.key, "3_open.png", "public");
    waiting(&state.db, &d, &ann.key, "4_notes.txt", b"NOTES");
    let answer = add(&state.db, &d, &ann.key, &open, "4_notes.txt", "file").expect("added to a public item").0;
    assert_eq!(answer["item"]["files"][1]["url"], "/uploads/4_notes.txt");
    assert_eq!((d.spot("4_notes.txt"), public_status(&d, "4_notes.txt").await), (Some(Spot::Public), 200));
}

/// MEMBERS ONLY MOVES EVERY FILE, AND PUBLIC BRINGS THEM BACK: a public item with an added file and
/// a cover is made members only, and all three leave the public folder (`/uploads/` answers 404
/// for each); made public again, all three are back. When one file cannot move, the change is not
/// made and every file already moved goes back where it was.
///
/// Seen red 2026-10-12 with the loop removed (`place` called for the item's own file alone, as
/// before round 5): "textures.zip stayed public while the item was members only / left: 200 /
/// right: 404".
#[tokio::test]
async fn members_only_moves_every_file_and_public_brings_them_back() {
    let state = relay("me_vis");
    let d = dirs("me_vis");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    let item = published(db, &d, "ann", "1_staff.glb", "public");
    waiting(db, &d, "ann", "2_textures.zip", b"T");
    waiting(db, &d, "ann", "3_cover.png", b"C");
    add(db, &d, "ann", &item, "2_textures.zip", "file").unwrap();
    add(db, &d, "ann", &item, "3_cover.png", "cover").unwrap();
    let all = ["1_staff.glb", "2_textures.zip", "3_cover.png"];

    let shut = visibility_core(db, d.folders(), item.id, "ann", "members").expect("members only").0;
    assert_eq!(public_status(&d, "2_textures.zip").await, 404, "textures.zip stayed public while the item was members only");
    for name in all {
        assert_eq!((name, d.spot(name), public_status(&d, name).await), (name, Some(Spot::Members), 404));
    }
    assert!(shut["item"]["cover"]["url"].as_str().is_some_and(|u| u.starts_with("/api/media/file/")), "{shut}");

    visibility_core(db, d.folders(), item.id, "ann", "public").expect("public again");
    for name in all {
        assert_eq!((name, d.spot(name), public_status(&d, name).await), (name, Some(Spot::Public), 200));
    }

    // A folder where the cover would go: the cover cannot move, so nothing changes.
    std::fs::create_dir_all(d.members.join("3_cover.png")).unwrap();
    let (code, _) = words(visibility_core(db, d.folders(), item.id, "ann", "members"));
    assert_eq!(code, StatusCode::INTERNAL_SERVER_ERROR);
    for name in all {
        assert_eq!((name, d.spot(name)), (name, Some(Spot::Public)), "every file already moved went back");
    }
    assert_eq!(db.media_item(item.id).unwrap().unwrap().visibility, "public", "and the item is still public");
}

/// A COVER MUST BE A PICTURE, AND A NEW ONE REPLACES THE OLD: a zip offered as the cover is refused
/// with the words and stays waiting; a PNG becomes the cover; a JPEG after it replaces it, and the
/// old cover's file and row are gone. The cover is not one of the item's files.
///
/// Seen red 2026-10-12 with the kind check removed (storage/media_files.rs `attach_media_file`):
/// "a zip is no cover: Json(Object {\"item\": ...".
#[test]
fn a_cover_must_be_a_picture_and_a_new_one_replaces_the_old() {
    let state = relay("me_cover");
    let d = dirs("me_cover");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    let item = published(db, &d, "ann", "1_staff.glb", "public");
    for file in ["2_notes.zip", "3_first.png", "4_second.jpg"] {
        waiting(db, &d, "ann", file, b"x");
    }

    match add(db, &d, "ann", &item, "2_notes.zip", "cover") {
        Err(refused) => assert_eq!(refused, (StatusCode::BAD_REQUEST, "A cover must be a picture: PNG, JPEG, GIF or WebP.".to_string())),
        Ok(answer) => panic!("a zip is no cover: {answer:?}"),
    }
    assert_eq!(d.spot("2_notes.zip"), Some(Spot::Waiting), "it stays waiting");

    let first = add(db, &d, "ann", &item, "3_first.png", "cover").expect("a PNG is a cover").0;
    assert_eq!(first["item"]["cover"]["filename"], "3_first.png");
    let second = add(db, &d, "ann", &item, "4_second.jpg", "cover").expect("a new cover").0;
    assert_eq!(second["item"]["cover"]["filename"], "4_second.jpg");
    assert_eq!((second["item"]["file_count"].as_i64(), second["item"]["files"].as_array().map(Vec::len)), (Some(1), Some(1)), "a cover is not a file");
    assert_eq!(d.spot("3_first.png"), None, "the old cover's file is gone");
    assert_eq!(count(db, "SELECT COUNT(*) FROM user_uploads WHERE filename = '3_first.png'"), 0, "and its row");
    assert_eq!(count(db, "SELECT COUNT(*) FROM media_item_files"), 1);
}

/// AN ITEM HOLDS AT MOST TWENTY FILES (decision 9): its own file and 19 added ones; the twentieth
/// added one is refused with the words and stays waiting. A cover is not counted.
///
/// Seen red 2026-10-12 with the limit removed (storage/media_files.rs `attach_media_file`): "the
/// twentieth added file is refused: Json(Object {\"item\": ...".
#[test]
fn an_item_holds_at_most_twenty_files() {
    let state = relay("me_twenty");
    let d = dirs("me_twenty");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    let item = published(db, &d, "ann", "1_staff.glb", "public");
    for i in 1..ITEM_FILES_MAX {
        let file = format!("f{i}.bin");
        waiting(db, &d, "ann", &file, b"x");
        add(db, &d, "ann", &item, &file, "file").unwrap_or_else(|e| panic!("added file {i}: {e:?}"));
    }
    assert_eq!(db.media_item(item.id).unwrap().unwrap().file_count, ITEM_FILES_MAX);

    waiting(db, &d, "ann", "one_too_many.bin", b"x");
    match add(db, &d, "ann", &item, "one_too_many.bin", "file") {
        Err(refused) => assert_eq!(refused, (StatusCode::BAD_REQUEST, "An item can hold at most 20 files.".to_string())),
        Ok(answer) => panic!("the twentieth added file is refused: {answer:?}"),
    }
    assert_eq!(d.spot("one_too_many.bin"), Some(Spot::Waiting), "it stays waiting");
    waiting(db, &d, "ann", "cover.png", b"x");
    add(db, &d, "ann", &item, "cover.png", "cover").expect("a cover is not counted");
}

/// REMOVE A FILE, BUT NEVER THE FIRST: file 0 is the item itself and is refused with the words
/// (remove the item instead); a file named by another file's name is the stale-page refusal
/// (409); a moderator and another member cannot; the uploader can (through the signed route), and
/// so can an admin. Each removed file's row and file are gone.
///
/// Seen red 2026-10-12 with file 0 allowed (the `file_no == 0` refusal removed from
/// storage/media_files.rs `detach_media_file`): "file 0 is the item itself / left: (404, \"That
/// Media item has no file with that number.\") / right: (400, ...)".
#[tokio::test]
async fn remove_a_file_but_never_the_first() {
    let state = relay("me_remove");
    let d = dirs("me_remove");
    let db = &state.db;
    let ann = person(1);
    for (key, name) in [(ann.key.as_str(), "Ann"), ("root", "Root"), ("mo", "Mo"), ("cal", "Cal")] {
        db.join_server(key, name).unwrap();
    }
    db.set_role("root", "admin").unwrap();
    db.set_role("mo", "mod").unwrap();
    let item = published(db, &d, &ann.key, "1_staff.glb", "public");
    for (file, role) in [("2_a.zip", "file"), ("3_b.zip", "file"), ("4_cover.png", "cover")] {
        waiting(db, &d, &ann.key, file, b"x");
        add(db, &d, &ann.key, &item, file, role).unwrap();
    }
    let item = db.media_item(item.id).unwrap().unwrap();
    let (a, b, cover) = (item.files[0].file, item.files[1].file, item.cover.clone().unwrap().file);

    assert_eq!(
        words(take_off(db, &d, &ann.key, &item, 0, "1_staff.glb")),
        (StatusCode::BAD_REQUEST, "The first file is the item itself: remove the item instead.".to_string()),
        "file 0 is the item itself"
    );
    assert_eq!(d.spot("1_staff.glb"), Some(Spot::Public), "and stays");
    assert_eq!(words(take_off(db, &d, &ann.key, &item, a, "3_b.zip")), (StatusCode::CONFLICT, OUT_OF_DATE.to_string()));
    assert_eq!(words(take_off(db, &d, "mo", &item, a, "2_a.zip")).0, StatusCode::FORBIDDEN, "a moderator cannot");
    assert_eq!(words(take_off(db, &d, "cal", &item, a, "2_a.zip")).0, StatusCode::FORBIDDEN, "nor another member");

    let ts = now_ms();
    let body = FileRemoveBody {
        key: ann.key.clone(),
        timestamp: ts,
        sig: ann.sign(&file_remove_purpose(item.id, a), ts),
        id: item.id,
        filename: item.upload_filename.clone(),
        file: a,
        file_name: "2_a.zip".into(),
    };
    let answer = files_remove_in(&state, d.folders(), &body).await.map_err(|e| (e.0.as_u16(), e.1)).expect("the uploader removes it").0;
    assert_eq!((answer["ok"].as_bool(), answer["file_removed"].as_bool()), (Some(true), Some(true)));
    assert_eq!(answer["item"]["files"].as_array().map(|f| f.iter().map(|x| x["file"].as_i64().unwrap()).collect::<Vec<_>>()), Some(vec![0, b]));
    assert_eq!((d.spot("2_a.zip"), count(db, "SELECT COUNT(*) FROM user_uploads WHERE filename = '2_a.zip'")), (None, 0));

    let answer = take_off(db, &d, "root", &item, cover, "4_cover.png").expect("an admin removes the cover").0;
    assert!(answer["item"]["cover"].is_null());
    assert_eq!(d.spot("4_cover.png"), None);
}

/// REMOVING AN ITEM DELETES ITS ADDED FILES AND COVER: an item made for Media with two added files
/// and a cover is removed, and all four files, their upload rows, their `media_item_files` rows
/// and the item's licence choices are gone. An item whose own file came from chat keeps that file
/// (back at its public address), and its added files still go.
///
/// Seen red 2026-10-12 with the added files not returned (`Removed::added` left empty in
/// storage/media.rs `remove_media`): "2_textures.zip is deleted with its item / left:
/// (\"2_textures.zip\", Some(Public)) / right: (\"2_textures.zip\", None)".
#[test]
fn removing_an_item_deletes_its_added_files_and_cover() {
    let state = relay("me_gone");
    let d = dirs("me_gone");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    let item = published(db, &d, "ann", "1_staff.glb", "public");
    for (file, role) in [("2_textures.zip", "file"), ("3_notes.txt", "file"), ("4_cover.png", "cover")] {
        waiting(db, &d, "ann", file, b"x");
        add(db, &d, "ann", &item, file, role).unwrap();
    }
    edit_core(db, &[], &offered(), &edit_of("ann", &item, "Staff", "cc_by_4", false), &signed("ann")).unwrap();
    assert_eq!(db.media_licence_log(item.id).unwrap().len(), 1, "test setup: a licence choice");

    remove_core(db, d.folders(), &RemoveTarget::Item(item.id), "ann").expect("removed");
    for name in ["1_staff.glb", "2_textures.zip", "3_notes.txt", "4_cover.png"] {
        assert_eq!((name, d.spot(name)), (name, None), "{name} is deleted with its item");
    }
    assert_eq!(count(db, "SELECT COUNT(*) FROM user_uploads"), 0, "every upload row");
    assert_eq!(count(db, "SELECT COUNT(*) FROM media_item_files"), 0, "every file row");
    assert_eq!(count(db, "SELECT COUNT(*) FROM media_licence_log"), 0, "and its licence choices (decision 8)");

    // From chat: its own file stays, at its public address; what was added for Media goes.
    std::fs::write(d.uploads.join("5_chat.png"), b"chat").unwrap();
    db.record_upload("ann", "5_chat.png", 100, 0i64, "chat.png", 4).unwrap();
    let s = db.get_server_settings().unwrap();
    let chat = publish_core(db, &s, &[], d.folders(), &PublishBody { key: "ann".into(), filename: "5_chat.png".into(), ..Default::default() }).unwrap().0;
    let chat = db.media_item(chat["item"]["id"].as_i64().unwrap()).unwrap().unwrap();
    waiting(db, &d, "ann", "6_extra.zip", b"x");
    add(db, &d, "ann", &chat, "6_extra.zip", "file").unwrap();
    let answer = remove_core(db, d.folders(), &RemoveTarget::Item(chat.id), "ann").unwrap().0;
    assert_eq!(answer["file_kept"], true);
    assert_eq!((d.spot("5_chat.png"), d.spot("6_extra.zip")), (Some(Spot::Public), None));
}

/// SOMEONE ELSE'S UPLOAD CANNOT BE ATTACHED: Ann cannot add Bea's waiting upload to her item (404,
/// "There is no upload of yours waiting with that name."), nor her own chat upload (not waiting
/// on the Media shelf); Bea cannot add her own upload to Ann's item (403); someone who is not a
/// member cannot add anything. Nothing moves.
///
/// Seen red 2026-10-12 with the owner check removed (the upload's `owner == signer` in
/// storage/media_files.rs `attach_media_file`): "Bea's upload is not Ann's to add:
/// Json(Object {\"item\": ...".
#[test]
fn someone_elses_upload_cannot_be_attached() {
    let state = relay("me_owner");
    let d = dirs("me_owner");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    db.join_server("bea", "Bea").unwrap();
    let item = published(db, &d, "ann", "1_staff.glb", "public");
    waiting(db, &d, "bea", "2_beas.zip", b"x");
    std::fs::write(d.uploads.join("3_chat.png"), b"chat").unwrap();
    db.record_upload("ann", "3_chat.png", 100, 0i64, "chat.png", 4).unwrap();

    match add(db, &d, "ann", &item, "2_beas.zip", "file") {
        Err(refused) => assert_eq!(refused, (StatusCode::NOT_FOUND, "There is no upload of yours waiting with that name.".to_string())),
        Ok(answer) => panic!("Bea's upload is not Ann's to add: {answer:?}"),
    }
    assert_eq!(words(add(db, &d, "ann", &item, "3_chat.png", "file")).0, StatusCode::NOT_FOUND, "a chat upload is not waiting on the Media shelf");
    assert_eq!(
        words(add(db, &d, "bea", &item, "2_beas.zip", "file")),
        (StatusCode::FORBIDDEN, "Only the person who put it on the Media page can add files to it.".to_string())
    );
    assert_eq!(words(add(db, &d, "stranger", &item, "2_beas.zip", "file")), (StatusCode::FORBIDDEN, MEMBERS_ONLY.to_string()));
    assert_eq!((d.spot("2_beas.zip"), d.spot("3_chat.png")), (Some(Spot::Waiting), Some(Spot::Public)), "nothing moved");
    assert_eq!(count(db, "SELECT COUNT(*) FROM media_item_files"), 0);
}

/// THE MEMBERS' ROUTE SERVES FILE N TO A MEMBER: with the session's upload token in the header,
/// `GET /api/media/file/<id>?file=<n>` gives a member that added file of a members-only item;
/// without `file`, the item's own file; a number the item does not have is 404; someone signed in
/// who is not a member gets nothing.
///
/// Seen red 2026-10-12 with the `file` parameter ignored (`file_from` serving file 0 whatever was
/// asked): "file n is the added file / left: (200, [48, 49, 50, ...]) / right: (200, [69, 88, 84,
/// 82, 65])".
#[tokio::test]
async fn the_members_route_serves_file_n_to_a_member() {
    let state = relay("me_route");
    let d = dirs("me_route");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    db.join_server("bea", "Bea").unwrap();
    {
        let mut tokens = state.upload_tokens.write().await;
        tokens.insert("tok_bea".into(), "bea".into());
        tokens.insert("tok_cal".into(), "cal".into());
    }
    let item = published(db, &d, "ann", "1_clip.mp4", "members");
    waiting(db, &d, "ann", "2_extra.mp4", b"EXTRA");
    let n = add(db, &d, "ann", &item, "2_extra.mp4", "file").unwrap().0["item"]["files"][1]["file"].as_i64().unwrap();
    let id = item.id;
    let get = |file: Option<i64>, token: &'static str| {
        let state = state.clone();
        let folders = d.folders();
        async move {
            let req = axum::http::Request::builder().uri("/api/media/file").header(UPLOAD_TOKEN_HEADER, token).body(axum::body::Body::empty()).unwrap();
            let res = file_from(&state, folders, id, TokenQuery { file, ..Default::default() }, req).await;
            let code = res.status().as_u16();
            (code, axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec())
        }
    };

    assert_eq!(get(Some(n), "tok_bea").await, (200, b"EXTRA".to_vec()), "file n is the added file");
    assert_eq!(get(None, "tok_bea").await, (200, b"0123456789".to_vec()), "no file: the item's own");
    assert_eq!(get(Some(0), "tok_bea").await.0, 200);
    assert_eq!(get(Some(n + 100), "tok_bea").await.0, 404, "a number the item does not have");
    assert_eq!(get(Some(n), "tok_cal").await.0, 403, "not a member");
}

/// `key` renames file `file` (stored as `name`) of `item` to `to`, through the route's core.
fn rename(db: &Storage, key: &str, item: &MediaItem, file: i64, name: &str, to: &str) -> Answer {
    let b = FileRenameBody { key: key.into(), id: item.id, filename: item.upload_filename.clone(), file, file_name: name.into(), name: to.into(), ..Default::default() };
    files_rename_core(db, &b)
}

/// `key` makes file `file` (stored as `name`) `item`'s cover, through the route's core.
fn cover(db: &Storage, key: &str, item: &MediaItem, file: i64, name: &str) -> Answer {
    let b = FileCoverBody { key: key.into(), id: item.id, filename: item.upload_filename.clone(), file, file_name: name.into(), ..Default::default() };
    files_cover_core(db, &b)
}

/// A FILE IS RENAMED, AND KEEPS ITS OWN ENDING (operator, 2026-10-17: "Can we allow renaming of
/// files in the edit window?"): the uploader renames an added file and the item's own; a name left
/// without its ending gets it back, and a Blender file named like a picture keeps ".blend" after it;
/// an admin may rename, anyone else may not; an empty name, a slash, a hidden direction mark, a
/// name of only its ending, an out-of-date file name (W5) are refused; the stored file is
/// untouched. Seen red 2026-10-17 with the ending rule left out: "its ending put back" (the name
/// came back as "Wick textures"); and with direction marks let through: "expected a refusal" (the
/// item's own file came back named "photo\u{202e}gnp.blend").
#[test]
fn a_file_is_renamed_and_keeps_its_own_ending() {
    let state = relay("me_rename");
    let d = dirs("me_rename");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    db.join_server("bob", "Bob").unwrap();
    db.set_role("root", "admin").unwrap();
    let item = published(db, &d, "ann", "1_cap.blend", "public");
    waiting(db, &d, "ann", "2_textures.zip", b"x");
    add(db, &d, "ann", &item, "2_textures.zip", "file").expect("added");
    let file_no = db.media_item(item.id).unwrap().unwrap().files[0].file;

    let r = rename(db, "ann", &item, file_no, "2_textures.zip", "Wick textures").expect("renamed").0;
    let shown = r["item"]["files"].as_array().unwrap().iter().find(|f| f["filename"] == "2_textures.zip").map(|f| f["original_name"].clone());
    assert_eq!(shown, Some(serde_json::json!("Wick textures.zip")), "its ending put back");
    let r = rename(db, "ann", &item, 0, "1_cap.blend", "Dragon egg cap.png").expect("the item's own file renamed").0;
    assert_eq!(r["item"]["original_name"], "Dragon egg cap.png.blend", "a Blender file named like a picture stays a Blender file");
    rename(db, "root", &item, 0, "1_cap.blend", "Cap").expect("an admin may rename");
    assert_eq!(words(rename(db, "bob", &item, 0, "1_cap.blend", "Mine")).0, StatusCode::FORBIDDEN, "someone else may not");
    for (bad, why) in [("   ", "A file needs a name."), ("a/b", "A file's name cannot hold a slash or a hidden character."), ("photo\u{202E}gnp", "A file's name cannot hold a slash or a hidden character."), (".blend", "A file needs a name before its ending.")] {
        assert_eq!(words(rename(db, "ann", &item, 0, "1_cap.blend", bad)), (StatusCode::BAD_REQUEST, why.to_string()), "{bad:?}");
    }
    assert_eq!(words(rename(db, "ann", &item, 0, "9_other.blend", "X")).0, StatusCode::CONFLICT, "an out-of-date file name (W5)");
    assert_eq!(words(rename(db, "ann", &item, 0, "1_cap.blend", &"a".repeat(130))).0, StatusCode::BAD_REQUEST, "too long");
    assert!(d.spot("1_cap.blend").is_some(), "the stored file is untouched");
    assert_eq!(shown_name_for("backup", "5_x.tar.gz").unwrap(), "backup.tar.gz");
    assert_eq!(shown_name_for("Notes.ZIP", "6_n.zip").unwrap(), "Notes.ZIP");
}

/// A PICTURE ALREADY ON THE ITEM BECOMES ITS COVER (operator, 2026-10-17: "I uploaded a png with the
/// blend and STL files but, I can't seem to assign it as the preview photo"): the PNG becomes the
/// cover and leaves the files; a second picture made the cover sends the first back to the files,
/// nothing deleted; an STL, the item's own file, the cover itself and someone else's choosing are
/// refused. The cover keeps the name it was given, and a rename of it shows. Seen red 2026-10-17
/// with the old cover deleted instead of kept: "the first cover is a file again"; and with the
/// cover's JSON written without `original_name` (as it was): "the cover keeps its name" (left: Null).
#[test]
fn a_picture_already_on_the_item_becomes_its_cover() {
    let state = relay("me_cover_from");
    let d = dirs("me_cover_from");
    let db = &state.db;
    db.join_server("ann", "Ann").unwrap();
    db.join_server("bob", "Bob").unwrap();
    let item = published(db, &d, "ann", "1_cap.blend", "public");
    for file in ["2_cap.stl", "3_photo.png", "4_side.jpg"] {
        waiting(db, &d, "ann", file, b"x");
        add(db, &d, "ann", &item, file, "file").expect("added");
    }
    let no = |name: &str| db.media_item(item.id).unwrap().unwrap().files.iter().chain(db.media_item(item.id).unwrap().unwrap().cover.iter()).find(|f| f.filename == name).map(|f| f.file).unwrap();
    let r = cover(db, "ann", &item, no("3_photo.png"), "3_photo.png").expect("the PNG is the cover").0;
    assert_eq!(r["item"]["cover"]["filename"], "3_photo.png");
    assert!(!r["item"]["files"].as_array().unwrap().iter().any(|f| f["filename"] == "3_photo.png"), "and no longer one of its files");
    let r = cover(db, "ann", &item, no("4_side.jpg"), "4_side.jpg").expect("another picture").0;
    assert_eq!(r["item"]["cover"]["filename"], "4_side.jpg");
    let files: Vec<&str> = r["item"]["files"].as_array().unwrap().iter().map(|f| f["filename"].as_str().unwrap()).collect();
    assert!(files.contains(&"3_photo.png"), "the first cover is a file again: {files:?}");
    assert!(d.spot("3_photo.png").is_some(), "nothing deleted");
    assert_eq!(r["item"]["cover"]["original_name"], "own 4_side.jpg", "the cover keeps its name");
    let r = rename(db, "ann", &item, no("4_side.jpg"), "4_side.jpg", "Side view").expect("the cover renamed").0;
    assert_eq!(r["item"]["cover"]["original_name"], "Side view.jpg", "and its new name shows");
    assert_eq!(words(cover(db, "ann", &item, no("2_cap.stl"), "2_cap.stl")), (StatusCode::BAD_REQUEST, "A cover must be a picture: PNG, JPEG, GIF or WebP.".to_string()));
    assert_eq!(words(cover(db, "ann", &item, 0, "1_cap.blend")).0, StatusCode::BAD_REQUEST, "the item's own file");
    assert_eq!(words(cover(db, "ann", &item, no("4_side.jpg"), "4_side.jpg")), (StatusCode::BAD_REQUEST, "That picture is the cover already.".to_string()));
    assert_eq!(words(cover(db, "bob", &item, no("3_photo.png"), "3_photo.png")).0, StatusCode::FORBIDDEN, "someone else may not");
}

/// A RENAME'S SIGNATURE HOLDS THE NAME IT WAS SIGNED FOR, AND A COVER'S ITS FILE: Ann's app signs
/// `media_file_rename:<id>:<file>:<BLAKE3 of the name>`; the same signature on another name is
/// refused (401) and nothing changes; the name as signed goes. Her cover choice, signed for one
/// file, does not choose another. Seen red 2026-10-17 with the name left out of the purpose: "the
/// same signature on another name" was accepted.
#[tokio::test]
async fn a_renames_signature_holds_its_name_and_a_covers_its_file() {
    let state = relay("me_rename_sig");
    let d = dirs("me_rename_sig");
    let ann = person(1);
    state.db.join_server(&ann.key, "Ann").unwrap();
    let item = published(&state.db, &d, &ann.key, "1_cap.blend", "public");
    for file in ["2_a.png", "3_b.png"] {
        waiting(&state.db, &d, &ann.key, file, b"x");
        add(&state.db, &d, &ann.key, &item, file, "file").expect("added");
    }
    let files = state.db.media_item(item.id).unwrap().unwrap().files;
    let (a, b) = (files[0].file, files[1].file);
    let ts = now_ms();
    let sig = ann.sign(&file_rename_purpose(item.id, 0, "Dragon egg cap"), ts);
    let body = |name: &str| FileRenameBody { key: ann.key.clone(), timestamp: ts, sig: sig.clone(), id: item.id, filename: item.upload_filename.clone(), file: 0, file_name: "1_cap.blend".into(), name: name.into() };
    assert_eq!(files_rename_in(&state, &body("Someone else's name")).await.map(|_| ()).map_err(|e| e.0.as_u16()), Err(401), "the same signature on another name");
    assert_eq!(state.db.media_item(item.id).unwrap().unwrap().original_name, "own 1_cap.blend", "nothing changed");
    let named = files_rename_in(&state, &body("Dragon egg cap")).await.map_err(|e| (e.0.as_u16(), e.1)).expect("the name as signed").0;
    assert_eq!(named["item"]["original_name"], "Dragon egg cap.blend");

    let ts = now_ms() + 1;
    let sig = ann.sign(&file_cover_purpose(item.id, a), ts);
    let cover_body = |file: i64, name: &str| FileCoverBody { key: ann.key.clone(), timestamp: ts, sig: sig.clone(), id: item.id, filename: item.upload_filename.clone(), file, file_name: name.into() };
    assert_eq!(files_cover_in(&state, &cover_body(b, "3_b.png")).await.map(|_| ()).map_err(|e| e.0.as_u16()), Err(401), "a cover signed for one file does not choose another");
    let chosen = files_cover_in(&state, &cover_body(a, "2_a.png")).await.map_err(|e| (e.0.as_u16(), e.1)).expect("the file as signed").0;
    assert_eq!(chosen["item"]["cover"]["filename"], "2_a.png");
}
