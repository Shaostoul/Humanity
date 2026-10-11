//! The Media page's routes (relay/media.rs): who sees what, the signed writes and the role
//! checks after them, the labels file, and the room an upload is held to. Each test names the
//! break it was seen failing against ("Seen red"), with the words it failed with.
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, and nothing is written outside a temporary folder.

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use crate::relay::storage::media::GENERAL_CATEGORY_ID;

const MIB: i64 = 1024 * 1024;

/// A person: their key (the Dilithium public key in hex, as the relay knows people) and the key
/// pair that signs for them.
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

fn relay(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

/// A public uploads folder and a members-only one of the test's own (deleted when they drop).
/// The third is the folder resumable uploads gather their pieces in (relay/resumable.rs).
fn temp_folders(tag: &str) -> (crate::test_temp::TempPath, crate::test_temp::TempPath, crate::test_temp::TempPath) {
    (
        crate::test_temp::dir(&format!("{tag}_uploads")),
        crate::test_temp::dir(&format!("{tag}_members")),
        crate::test_temp::dir(&format!("{tag}_resume")),
    )
}

/// Which shelf an upload is on (0 chat, 1 Shared Files, 2 Media), or None when its row is gone.
fn shelf_of(db: &Storage, file: &str) -> Option<i64> {
    db.with_conn(|c| {
        use rusqlite::OptionalExtension;
        c.query_row("SELECT shared FROM user_uploads WHERE filename = ?1", [file], |r| r.get(0)).optional()
    })
    .unwrap()
}

/// The Shared Files list's file names.
fn shared_list(db: &Storage) -> Vec<String> {
    db.list_shared_uploads(50, None).unwrap().into_iter().map(|u| u.filename).collect()
}

/// `key` uploads `file` for Media and publishes it with `visibility`.
fn published(db: &Storage, key: &str, file: &str, visibility: &str) -> MediaItem {
    db.record_upload(key, file, 100, MEDIA_SHELF, file, 10).unwrap();
    db.publish_media(key, &Publish { filename: file, visibility, ..Default::default() }, 1 << 40, 1 << 40, None).unwrap()
}

fn ids(answer: &Value) -> Vec<i64> {
    answer["items"].as_array().expect("an items list").iter().map(|i| i["id"].as_i64().unwrap()).collect()
}

fn signed_list(p: &Person, ts: u64) -> ListParams {
    ListParams { key: Some(p.key.clone()), timestamp: Some(ts), sig: Some(p.sign("media_list", ts)), ..Default::default() }
}

/// AN UNSIGNED REQUEST NEVER SEES A MEMBERS-ONLY ITEM: on a public page it gets the public items
/// only, and so does a signed request from someone who is not a member (or is banned); a member's
/// signed request gets both. One person's picks follow the same rule. A bad or half signature is
/// refused rather than served the public view.
///
/// Seen red 2026-10-11 with the list's visibility clause reading `(1 = 1 OR m.visibility =
/// 'public')`: "an unsigned request saw a members-only item / left: [2, 1] / right: [1]".
#[tokio::test]
async fn an_unsigned_request_never_sees_a_members_only_item() {
    let state = relay("media_vis");
    let (ann, out, banned) = (person(1), person(2), person(3));
    state.db.join_server(&ann.key, "Ann").unwrap();
    state.db.join_server(&banned.key, "Bo").unwrap();
    state.db.ban_user(&banned.key, "Bo").unwrap();
    let open = published(&state.db, &ann.key, "1_open.png", "public");
    let inner = published(&state.db, &ann.key, "2_inner.png", "members");
    let now = now_ms();

    let unsigned = list(State(state.clone()), Query(ListParams::default())).await.unwrap().0;
    assert_eq!(ids(&unsigned), vec![open.id], "an unsigned request saw a members-only item");
    assert_eq!(unsigned["members_view"], false);
    let outsider = list(State(state.clone()), Query(signed_list(&out, now))).await.unwrap().0;
    assert_eq!(ids(&outsider), vec![open.id], "a signed request from a non-member saw a members-only item");
    let shut_out = list(State(state.clone()), Query(signed_list(&banned, now))).await.unwrap().0;
    assert_eq!(ids(&shut_out), vec![open.id], "a banned member saw a members-only item");
    let member = list_post(State(state.clone()), Json(signed_list(&ann, now))).await.unwrap().0;
    assert_eq!(ids(&member), vec![inner.id, open.id], "a member's signed request gets both");
    assert_eq!(member["members_view"], true);

    let picker = person(4);
    state.db.join_server(&picker.key, "Pia").unwrap();
    state.db.pick_media(&picker.key, inner.id).unwrap();
    let picks = picks_of(State(state.clone()), Path(picker.key.clone()), Query(ListParams::default())).await.unwrap().0;
    assert!(ids(&picks).is_empty(), "someone's picks showed an unsigned request a members-only item");

    let mut forged = signed_list(&ann, now + 1);
    forged.sig = Some(out.sign("media_list", now + 1));
    assert_eq!(list(State(state.clone()), Query(forged)).await.unwrap_err().0, StatusCode::UNAUTHORIZED);
    let half = ListParams { key: Some(ann.key.clone()), ..Default::default() };
    assert_eq!(list(State(state.clone()), Query(half)).await.unwrap_err().0, StatusCode::BAD_REQUEST);
}

/// A MEMBERS-ONLY SERVER SHOWS AN UNSIGNED REQUEST NOTHING, public items included, and says so;
/// a member still gets everything.
///
/// Seen red 2026-10-11 with the members-only server check taken out of `list_core`: "a
/// members-only server showed an unsigned request its items / left: [1] / right: []".
#[tokio::test]
async fn an_unsigned_request_sees_nothing_on_a_members_only_server() {
    let state = relay("media_members_server");
    let ann = person(5);
    state.db.join_server(&ann.key, "Ann").unwrap();
    let open = published(&state.db, &ann.key, "1_open.png", "public");
    let mut s = state.db.get_server_settings().unwrap();
    s.media_visibility = "members".into();
    state.db.set_server_settings(&s, "admin").unwrap();

    let unsigned = list(State(state.clone()), Query(ListParams::default())).await.unwrap().0;
    assert_eq!(ids(&unsigned), Vec::<i64>::new(), "a members-only server showed an unsigned request its items");
    assert_eq!(unsigned["members_only_for_you"], true);
    assert_eq!(unsigned["media_visibility"], "members");
    let member = list(State(state.clone()), Query(signed_list(&ann, now_ms()))).await.unwrap().0;
    assert_eq!(ids(&member), vec![open.id]);
}

/// PUBLISHING THROUGH THE ROUTE: a member only; the labels are the labels file's (an unknown one
/// is refused with the list, adult content by name), stored in the file's order; the visibility
/// is "public" or "members"; a URL is taken for its file name.
///
/// Seen red 2026-10-11 with `normalize_labels` keeping the order they were given in: "labels are
/// stored in the labels file's order / left: [\"graphic\", \"spoiler\"] / right: [\"spoiler\",
/// \"graphic\"]".
#[test]
fn publishing_needs_a_member_known_labels_and_a_visibility() {
    let state = relay("media_publish_route");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let known = load_labels(std::path::Path::new("data/media/labels.json"));
    let (uploads, members, resume) = temp_folders("media_publish");
    let uploads = Folders { uploads: &uploads, members: &members, resume: &resume };
    let (ann, out) = (person(6), person(7));
    db.join_server(&ann.key, "Ann").unwrap();
    db.record_upload(&ann.key, "1_a.png", 100, MEDIA_SHELF, "a.png", 10).unwrap();
    let body = |labels: &[&str], visibility: Option<&str>| PublishBody {
        key: ann.key.clone(),
        filename: "/uploads/1_a.png".into(),
        title: "Staff spin".into(),
        labels: labels.iter().map(|l| l.to_string()).collect(),
        visibility: visibility.map(str::to_string),
        ..Default::default()
    };

    let mut outsider = body(&[], None);
    outsider.key = out.key.clone();
    assert_eq!(publish_core(db, &s, &known, uploads, &outsider).unwrap_err().0, StatusCode::FORBIDDEN, "a non-member");
    let (code, words) = publish_core(db, &s, &known, uploads, &body(&["gore"], None)).unwrap_err();
    assert!(code == StatusCode::BAD_REQUEST && words.contains("Flashing lights (flashing)"), "{words}");
    let (_, words) = publish_core(db, &s, &known, uploads, &body(&["NSFW"], None)).unwrap_err();
    assert!(words.contains("Adult content is not allowed"), "{words}");
    assert_eq!(publish_core(db, &s, &known, uploads, &body(&[], Some("friends"))).unwrap_err().0, StatusCode::BAD_REQUEST);

    let ok = publish_core(db, &s, &known, uploads, &body(&["Graphic", "spoiler", "graphic"], Some("Members"))).unwrap().0;
    let labels: Vec<&str> = ok["item"]["labels"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    assert_eq!(labels, vec!["spoiler", "graphic"], "labels are stored in the labels file's order");
    assert_eq!(ok["item"]["visibility"], "members");
    let id = ok["item"]["id"].as_i64().unwrap();
    assert_eq!(ok["item"]["url"], format!("/api/media/file/{id}"), "a members-only item is fetched through the members' route");
    assert_eq!(ok["item"]["category"]["id"], GENERAL_CATEGORY_ID);
}

/// TAKING AN ITEM DOWN THROUGH THE ROUTE: a member who is not the uploader is refused and the
/// file stays; an admin's removal of an item uploaded for Media deletes its file.
///
/// Seen red 2026-10-11 with the file delete taken out of `remove_core`: "the file is gone".
#[test]
fn removing_an_item_deletes_its_file() {
    let state = relay("media_remove_route");
    let db = &state.db;
    let (uploads, members, resume) = temp_folders("media_remove");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let (ann, cal, op) = (person(8), person(9), person(10));
    for (p, n) in [(&ann, "Ann"), (&cal, "Cal"), (&op, "Op")] {
        db.join_server(&p.key, n).unwrap();
    }
    db.set_role(&op.key, "admin").unwrap();
    std::fs::write(uploads.join("1_a.png"), b"picture").unwrap();
    db.record_upload(&ann.key, "1_a.png", 100, MEDIA_SHELF, "a.png", 7).unwrap();
    let item = db.publish_media(&ann.key, &Publish { filename: "1_a.png", visibility: "public", ..Default::default() }, 1 << 40, 1 << 40, None).unwrap();

    assert_eq!(remove_core(db, folders, &RemoveTarget::Item(item.id), &cal.key).unwrap_err().0, StatusCode::FORBIDDEN);
    assert!(uploads.join("1_a.png").exists(), "a refused removal left the file");
    let done = remove_core(db, folders, &RemoveTarget::Item(item.id), &op.key).unwrap().0;
    assert_eq!(done["file_removed"], true);
    assert!(!uploads.join("1_a.png").exists(), "the file is gone");
    assert!(db.media_item(item.id).unwrap().is_none());
}

/// THE ADMIN ROUTES refuse everyone else, a member included: featuring, the category list, the
/// decisions on suggestions and the usage numbers. An admin's feature shows in the list.
///
/// Seen red 2026-10-11 with `is_admin` answering true for every key: "a member featured an item".
#[test]
fn the_admin_routes_refuse_everyone_else() {
    let state = relay("media_admin_routes");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let (ann, op) = (person(11), person(12));
    db.join_server(&ann.key, "Ann").unwrap();
    db.join_server(&op.key, "Op").unwrap();
    db.set_role(&op.key, "admin").unwrap();
    let item = published(db, &ann.key, "1_a.png", "public");
    let suggestion = db.suggest_media_category(&ann.key, "Builds", "").unwrap();
    let general = [CategoryEdit { id: Some(1), name: "Everything".into(), description: String::new() }];

    assert!(feature_core(db, &ann.key, item.id, true).is_err(), "a member featured an item");
    assert_eq!(feature_core(db, &ann.key, item.id, true).unwrap_err().0, StatusCode::FORBIDDEN);
    assert_eq!(categories_core(db, &ann.key, &general).unwrap_err().0, StatusCode::FORBIDDEN);
    assert_eq!(decide_core(db, &ann.key, suggestion.id, true).unwrap_err().0, StatusCode::FORBIDDEN);
    assert_eq!(usage_core(db, &s, &ann.key).unwrap_err().0, StatusCode::FORBIDDEN);
    assert!(!db.media_item(item.id).unwrap().unwrap().featured, "the refusal changed nothing");
    assert_eq!(db.media_categories().unwrap()[0].name, "General");

    feature_core(db, &op.key, item.id, true).unwrap();
    assert!(db.media_item(item.id).unwrap().unwrap().featured);
    feature_core(db, &op.key, item.id, false).unwrap();
    assert!(!db.media_item(item.id).unwrap().unwrap().featured, "an admin takes it off the featured row");
    assert_eq!(categories_core(db, &op.key, &general).unwrap().0["categories"][0]["name"], "Everything");
}

/// SUGGESTIONS THROUGH THE ROUTES: a member suggests (someone who is not is refused); an admin
/// sees every suggestion and a member their own; and when an admin decides, the suggester is
/// told, on their own clients.
///
/// Seen red 2026-10-11 with `decide` telling the admin instead of the suggester: "the suggester
/// is told / left: \"<the admin's key>\" / right: \"<ann's key>\"" (keys shortened here).
#[tokio::test]
async fn a_suggester_is_told_when_an_admin_decides() {
    let state = relay("media_suggest_routes");
    let db = &state.db;
    let (ann, bea, out, op) = (person(13), person(14), person(15), person(16));
    for (p, n) in [(&ann, "Ann"), (&bea, "Bea"), (&op, "Op")] {
        db.join_server(&p.key, n).unwrap();
    }
    db.set_role(&op.key, "admin").unwrap();
    assert_eq!(suggest_core(db, &out.key, "Ghosts", "").unwrap_err().0, StatusCode::FORBIDDEN, "not a member");
    let fire = suggest_core(db, &ann.key, "Fire flow arts", "").unwrap().0;
    suggest_core(db, &bea.key, "Builds", "").unwrap();
    let anns = suggestions_core(db, &ann.key).unwrap().0;
    assert_eq!((anns["suggestions"].as_array().unwrap().len(), anns["all"].as_bool()), (1, Some(false)), "a member sees their own");
    assert_eq!(suggestions_core(db, &op.key).unwrap().0["suggestions"].as_array().unwrap().len(), 2, "an admin sees all");

    let mut told = state.broadcast_tx.subscribe();
    let ts = now_ms();
    let id = fire["suggestion"]["id"].as_i64().unwrap();
    let body = DecideBody { key: op.key.clone(), timestamp: ts, sig: op.sign(&decide_purpose(id, true), ts), id, accept: true };
    let answer = decide(State(state.clone()), Json(body)).await.unwrap().0;
    assert_eq!(answer["category"]["name"], "Fire flow arts");
    match told.try_recv() {
        Ok(RelayMessage::Private { to, message }) => {
            assert_eq!(to, ann.key, "the suggester is told");
            assert!(message.contains("\"Fire flow arts\" was accepted"), "{message}");
        }
        other => panic!("no word to the suggester: {other:?}"),
    }
}

/// THE USAGE NUMBERS THROUGH THE ROUTE: the meter in words, the budget and the share in bytes.
///
/// Seen red 2026-10-11 with the meter's budget taken from the per-person share: "the meter / left:
/// \"1.5 MB of 512.0 MB used\" / right: \"1.5 MB of 2048.0 MB used\"".
#[test]
fn the_usage_route_gives_the_meter() {
    let state = relay("media_usage_route");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let op = person(17);
    db.join_server(&op.key, "Op").unwrap();
    db.set_role(&op.key, "owner").unwrap();
    db.record_upload(&op.key, "1_a.png", 100, 0i64, "a.png", 3 * MIB / 2).unwrap();
    db.publish_media(&op.key, &Publish { filename: "1_a.png", visibility: "public", ..Default::default() }, 1 << 40, 1 << 40, None).unwrap();
    let u = usage_core(db, &s, &op.key).unwrap().0;
    assert_eq!(u["meter"], "1.5 MB of 2048.0 MB used", "the meter");
    assert_eq!(u["budget_bytes"], 2048 * MIB);
    assert_eq!(u["per_person_bytes"], 512 * MIB);
    assert_eq!(u["used_bytes"], 3 * MIB / 2);
    assert_eq!(u["largest"][0]["url"], "/uploads/1_a.png");
}

/// THE ROOM AN UPLOAD IS HELD TO: a Media upload to the Media budget and the person's share, with
/// both numbers in the words; a chat file to the general cap, counting the uploads folder without
/// the Media space's bytes (which used to fill the chat cap). And the shelf: `?media=1` is shelf
/// 2, never with an encrypted file.
///
/// Seen red 2026-10-11 with the chat branch counting the whole folder: "a chat file was refused
/// for the Media space's bytes: Some((507, \"Upload storage full (2.0 MB / 1 MB). Please try
/// again later.\"))".
#[test]
fn an_upload_is_held_to_its_own_shelfs_room() {
    let state = relay("media_upload_room");
    let db = &state.db;
    let mut s = db.get_server_settings().unwrap();
    s.max_total_upload_mb = 1;
    s.media_budget_mb = 3;
    s.media_per_person_mb = 2;
    db.record_upload("ann", "1_a.png", 100, MEDIA_SHELF, "a.png", 3 * MIB / 2).unwrap();
    let folder = 2 * MIB as u64; // the 1.5 MB Media file and 0.5 MB of chat files
    let mib = |x: f64| (x * MIB as f64) as u64;

    let chat_ok = upload_room(db, &s, "cal", 0, folder, mib(0.4));
    assert!(chat_ok.is_ok(), "a chat file was refused for the Media space's bytes: {:?}", chat_ok.err().map(|(c, w)| (c.as_u16(), w)));
    let (code, words) = upload_room(db, &s, "cal", 0, folder, mib(0.6)).unwrap_err();
    assert_eq!(code, StatusCode::INSUFFICIENT_STORAGE);
    assert!(words.contains("(0.5 MB / 1 MB)"), "{words}");

    let (code, words) = upload_room(db, &s, "ann", MEDIA_SHELF, folder, mib(0.6)).unwrap_err();
    assert_eq!(code, StatusCode::INSUFFICIENT_STORAGE);
    assert!(words.contains("1.5 MB of your 2.0 MB used") && words.contains("this file is 0.6 MB"), "{words}");
    let (_, words) = upload_room(db, &s, "bea", MEDIA_SHELF, folder, mib(1.6)).unwrap_err();
    assert!(words.contains("1.5 MB of 3.0 MB used") && words.contains("this file is 1.6 MB"), "{words}");
    assert!(upload_room(db, &s, "bea", MEDIA_SHELF, folder, mib(1.4)).is_ok(), "a Media upload is not held to the chat cap");

    assert_eq!(upload_shelf(None, Some(1), None).unwrap(), MEDIA_SHELF);
    assert_eq!(upload_shelf(Some(1), Some(1), None).unwrap(), MEDIA_SHELF, "media wins over share: it lists nowhere until published");
    assert_eq!(upload_shelf(Some(1), None, None).unwrap(), 1);
    assert_eq!(upload_shelf(None, None, Some(1)).unwrap(), 0);
    assert_eq!(upload_shelf(None, Some(1), Some(1)).unwrap_err().0, StatusCode::BAD_REQUEST, "an encrypted file is never Media");
}

/// A MEDIA WRITE NEEDS A FRESH SIGNATURE FROM THE KEY IT NAMES: someone else's signature, a
/// timestamp more than five minutes old or ahead, and the same signed request a second time are
/// refused; the role check runs on the proven key.
///
/// Seen red 2026-10-11 with the nonce check taken out of `check_signed`: "the same signed request
/// was taken twice".
#[tokio::test]
async fn a_media_write_needs_a_fresh_signature() {
    let state = relay("media_signed");
    let (op, ann) = (person(18), person(19));
    state.db.join_server(&op.key, "Op").unwrap();
    state.db.set_role(&op.key, "admin").unwrap();
    let item = published(&state.db, &ann.key, "1_a.png", "public");
    let now = now_ms();
    let purpose = format!("media_feature:{}", item.id);
    let body = |ts: u64, signer: &Person| FeatureBody { key: op.key.clone(), timestamp: ts, sig: signer.sign(&purpose, ts), id: item.id, featured: true };

    let forged = feature(State(state.clone()), Json(body(now, &ann))).await.unwrap_err();
    assert_eq!(forged.0, StatusCode::UNAUTHORIZED, "someone else's signature");
    let old = feature(State(state.clone()), Json(body(now - 6 * 60 * 1000, &op))).await.unwrap_err();
    assert_eq!(old.0, StatusCode::BAD_REQUEST, "{}", old.1);
    let ahead = feature(State(state.clone()), Json(body(now + 6 * 60 * 1000, &op))).await.unwrap_err();
    assert_eq!(ahead.0, StatusCode::BAD_REQUEST, "{}", ahead.1);
    let wrong_purpose = FeatureBody { sig: op.sign("media_remove", now), ..body(now, &op) };
    assert_eq!(feature(State(state.clone()), Json(wrong_purpose)).await.unwrap_err().0, StatusCode::UNAUTHORIZED);

    feature(State(state.clone()), Json(body(now, &op))).await.expect("the admin's signed request");
    let again = feature(State(state.clone()), Json(body(now, &op))).await;
    assert!(again.is_err(), "the same signed request was taken twice");
    assert_eq!(again.unwrap_err().0, StatusCode::CONFLICT);
}

/// THE SHIPPED LABELS FILE PARSES and names the three labels; a missing file falls back to the
/// copy built into the exe; a broken file (a repeated id, an id with a comma, no labels) is
/// refused with the reason.
///
/// Seen red 2026-10-11 with `LabelsFile` reading a `label` field instead of `labels`:
/// "data/media/labels.json parses: \"not a labels file: missing field `label` at line 8 column 1\"".
#[test]
fn the_shipped_labels_file_parses() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/media/labels.json");
    let text = std::fs::read_to_string(&path).expect("data/media/labels.json is shipped");
    let shipped = parse_labels(&text).unwrap_or_else(|e| panic!("data/media/labels.json parses: {e:?}"));
    let ids: Vec<&str> = shipped.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, vec!["spoiler", "flashing", "graphic"]);
    assert!(shipped.iter().all(|l| !l.desc.is_empty()), "every label says what it is for");
    let missing = crate::test_temp::path("media_no_labels");
    assert_eq!(load_labels(&missing), shipped, "a missing file falls back to the built-in copy");

    assert!(parse_labels(r#"{"labels":[{"id":"spoiler","label":"A"},{"id":"spoiler","label":"B"}]}"#).unwrap_err().contains("twice"));
    assert!(parse_labels(r#"{"labels":[{"id":"a,b","label":"A"}]}"#).is_err());
    assert!(parse_labels(r#"{"labels":[]}"#).is_err());
}

// ── The review's fixes (docs/design/media-showcase.md, "Increment 1 fixes", 2026-10-11) ──

/// F1. A MEDIA UPLOAD NOT PUBLISHED WITHIN A DAY EXPIRES, row and file, in the expiry pass (which
/// runs at start, every six hours and after a settings change); until then the admin's usage
/// lists it under waiting with its size, and its owner or an admin can remove it by its file
/// name (someone else cannot).
///
/// Seen red 2026-10-11 with the unpublished sweep taken out of `run_expiry_sweeps_in`: "the
/// expiry pass deleted the day-old upload's file".
#[test]
fn an_unpublished_media_upload_expires_and_can_be_removed_by_name() {
    let state = relay("media_f1");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let (uploads, members, resume) = temp_folders("media_f1");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let (ann, cal, op) = (person(20), person(21), person(22));
    for (p, n) in [(&ann, "Ann"), (&cal, "Cal"), (&op, "Op")] {
        db.join_server(&p.key, n).unwrap();
    }
    db.set_role(&op.key, "admin").unwrap();
    for (file, size) in [("1_old.mp4", 5 * MIB), ("2_new.mp4", 3 * MIB), ("3_also.png", MIB)] {
        std::fs::write(uploads.join(file), b"video").unwrap();
        db.record_upload(&ann.key, file, 100, MEDIA_SHELF, file, size).unwrap();
    }
    let day_and_an_hour = 25 * 60 * 60 * 1000i64;
    db.with_conn(|c| c.execute("UPDATE user_uploads SET uploaded_at = uploaded_at - ?1 WHERE filename = '1_old.mp4'", [day_and_an_hour])).unwrap();

    let u = usage_core(db, &s, &op.key).unwrap().0;
    let waiting: Vec<(String, i64)> =
        u["waiting"].as_array().unwrap().iter().map(|w| (w["filename"].as_str().unwrap().to_string(), w["size_bytes"].as_i64().unwrap())).collect();
    let expect = |l: &[(&str, i64)]| l.iter().map(|(f, n)| (f.to_string(), *n)).collect::<Vec<_>>();
    assert_eq!(waiting, expect(&[("1_old.mp4", 5 * MIB), ("2_new.mp4", 3 * MIB), ("3_also.png", MIB)]), "the usage lists what waits, oldest first");
    assert_eq!(u["used_bytes"], 9 * MIB);

    db.run_expiry_sweeps_in(folders);
    assert!(!uploads.join("1_old.mp4").exists(), "the expiry pass deleted the day-old upload's file");
    assert_eq!(shelf_of(db, "1_old.mp4"), None, "and its row");
    assert!(uploads.join("2_new.mp4").exists(), "a newer upload waits on");
    assert_eq!(db.media_bytes(None), 4 * MIB);

    let by_name = |n: &str| RemoveTarget::Upload(n.to_string());
    assert_eq!(remove_core(db, folders, &by_name("2_new.mp4"), &cal.key).unwrap_err().0, StatusCode::FORBIDDEN, "someone else's");
    assert!(uploads.join("2_new.mp4").exists(), "a refused removal left the file");
    let done = remove_core(db, folders, &by_name("2_new.mp4"), &ann.key).unwrap().0;
    assert_eq!(done["file_removed"], true);
    assert!(!uploads.join("2_new.mp4").exists(), "the owner removed it, file and all");
    remove_core(db, folders, &by_name("/uploads/3_also.png"), &op.key).expect("an admin removes anyone's, by its address too");
    assert_eq!(db.media_bytes(None), 0, "nothing waits now");
    assert_eq!(remove_core(db, folders, &by_name("2_new.mp4"), &ann.key).unwrap_err().0, StatusCode::NOT_FOUND);
    assert!(usage_core(db, &s, &op.key).unwrap().0["waiting"].as_array().unwrap().is_empty());
    let b = RemoveBody { filename: Some("/uploads/2_new.mp4".into()), ..Default::default() };
    assert_eq!(b.target().unwrap(), (by_name("2_new.mp4"), "media_remove:2_new.mp4".to_string()), "a removal by name is signed over the name");
}

/// F2. REMOVING AN ITEM NEVER TAKES A FILE SOMETHING ELSE USES: an item made from a chat file
/// and one made from a Shared Files file keep their files, at their `/uploads/` addresses, and
/// their uploads go back (to chat's keep-N shelf, to the Shared Files list); only an item
/// uploaded for Media loses its file and its row. An admin removing someone's chat photo from
/// Media therefore never breaks the chat message.
///
/// Seen red 2026-10-11 with `Removed::keeps_file` answering false for every shelf (the merged
/// behaviour): "an admin's removal kept the chat photo's file".
#[test]
fn removing_an_item_gives_its_upload_back_and_keeps_a_file_in_use() {
    let state = relay("media_f2");
    let db = &state.db;
    let (uploads, members, resume) = temp_folders("media_f2");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let (ann, op) = (person(23), person(24));
    db.join_server(&ann.key, "Ann").unwrap();
    db.join_server(&op.key, "Op").unwrap();
    db.set_role(&op.key, "admin").unwrap();
    let mut ids = Vec::new();
    for (file, shelf) in [("1_chat.png", 0), ("2_shared.glb", 1), ("3_made.mp4", MEDIA_SHELF)] {
        std::fs::write(uploads.join(file), b"bytes").unwrap();
        db.record_upload(&ann.key, file, 100, shelf, file, 5).unwrap();
        ids.push(db.publish_media(&ann.key, &Publish { filename: file, visibility: "public", ..Default::default() }, 1 << 40, 1 << 40, None).unwrap().id);
    }
    assert_eq!(shelf_of(db, "1_chat.png"), Some(MEDIA_SHELF), "published, the chat file is off the keep-N shelf");

    let done = remove_core(db, folders, &RemoveTarget::Item(ids[0]), &op.key).unwrap().0;
    assert!(uploads.join("1_chat.png").exists(), "an admin's removal kept the chat photo's file");
    assert_eq!((done["file_kept"].as_bool(), done["file_removed"].as_bool()), (Some(true), Some(false)));
    assert_eq!(shelf_of(db, "1_chat.png"), Some(0), "the chat file is a chat file again");

    remove_core(db, folders, &RemoveTarget::Item(ids[1]), &ann.key).unwrap();
    assert!(uploads.join("2_shared.glb").exists(), "the Shared Files file stays");
    assert_eq!(shelf_of(db, "2_shared.glb"), Some(1));
    assert_eq!(shared_list(db), vec!["2_shared.glb".to_string()], "and stays in its list");

    let done = remove_core(db, folders, &RemoveTarget::Item(ids[2]), &ann.key).unwrap().0;
    assert_eq!(done["file_removed"], true);
    assert!(!uploads.join("3_made.mp4").exists(), "an item uploaded for Media loses its file");
    assert_eq!(shelf_of(db, "3_made.mp4"), None, "and its row");
    assert!(ids.iter().all(|id| db.media_item(*id).unwrap().is_none()), "every item is off the page");
}

/// A request for the members' file route, as a browser sends it.
fn file_request(id: i64) -> axum::extract::Request {
    axum::http::Request::builder().uri(format!("/api/media/file/{id}")).body(axum::body::Body::empty()).unwrap()
}

/// F3. MEMBERS ONLY MEANS MEMBERS ONLY: a members-only item's file is not at its `/uploads/`
/// address but in the members-only folder, served by `GET /api/media/file/{id}?token=` to a
/// member with the session's upload token, refused without one, with a token that is not a
/// session, to a banned key and to someone who is not a member. Made public it is back at its
/// address, members only again it leaves it. The line under the switch says what a member is.
///
/// Seen red 2026-10-11 with `publish_core`'s placing closure not moving the file: "a members-only
/// file is still at its /uploads/ address".
#[tokio::test]
async fn a_members_only_file_is_only_for_members() {
    let state = relay("media_f3");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let (uploads, members, resume) = temp_folders("media_f3");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let (ann, bea, bo, out) = (person(25), person(26), person(27), person(28));
    for (p, n) in [(&ann, "Ann"), (&bea, "Bea"), (&bo, "Bo")] {
        db.join_server(&p.key, n).unwrap();
    }
    db.ban_user(&bo.key, "Bo").unwrap();
    std::fs::write(uploads.join("1_case.glb"), b"model bytes").unwrap();
    db.record_upload(&ann.key, "1_case.glb", 100, MEDIA_SHELF, "case.glb", 11).unwrap();

    let body = PublishBody { key: ann.key.clone(), filename: "1_case.glb".into(), visibility: Some("members".into()), ..Default::default() };
    let item = publish_core(db, &s, &[], folders, &body).unwrap().0;
    let id = item["item"]["id"].as_i64().unwrap();
    assert!(!uploads.join("1_case.glb").exists(), "a members-only file is still at its /uploads/ address");
    assert!(members.join("1_case.glb").exists(), "it is in the members-only folder");
    assert_eq!(item["item"]["url"], format!("/api/media/file/{id}"));

    for (token, key) in [("tok_bea", &bea.key), ("tok_bo", &bo.key), ("tok_out", &out.key)] {
        state.upload_tokens.write().await.insert(token.to_string(), key.clone());
    }
    let res = file_core(&state, folders, id, Some("tok_bea"), file_request(id)).await.expect("a member with a token");
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[axum::http::header::CONTENT_DISPOSITION], "attachment");
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    assert_eq!(&bytes[..], b"model bytes");
    for (token, code, who) in [
        (None, StatusCode::UNAUTHORIZED, "no token"),
        (Some("forged"), StatusCode::FORBIDDEN, "a token that is not a session"),
        (Some("tok_bo"), StatusCode::FORBIDDEN, "a banned key"),
        (Some("tok_out"), StatusCode::FORBIDDEN, "not a member"),
    ] {
        let refused = file_core(&state, folders, id, token, file_request(id)).await.map(|r| r.status());
        assert_eq!(refused.err().map(|(c, _)| c), Some(code), "{who}");
    }

    let public = visibility_core(db, folders, id, &ann.key, "public").unwrap().0;
    assert_eq!(public["item"]["url"], "/uploads/1_case.glb");
    assert!(uploads.join("1_case.glb").exists() && !members.join("1_case.glb").exists(), "made public, the file is back at its address");
    visibility_core(db, folders, id, &ann.key, "members").unwrap();
    assert!(!uploads.join("1_case.glb").exists() && members.join("1_case.glb").exists(), "members only again, it leaves it");

    assert!(s.media_members_note.contains("everyone signed in") && s.media_members_note.contains("lockdown"), "{}", s.media_members_note);
    let sent = serde_json::to_value(&s).unwrap();
    assert_eq!(sent["media_members_note"], crate::relay::storage::MEDIA_MEMBERS_NOTE, "the settings answer carries the line");
}

/// F4. BUDGETS HOLD WHEN UPLOADS FINISH TOGETHER: two Media uploads that each fit a 1 MB share
/// alone, but not both, run the room check, the move into place and the row from two threads at
/// once. Each waits in its move (up to half a second) for the other to arrive: without the lock
/// both have passed the check by then; with it the second waits for the first's row and is
/// refused.
///
/// Seen red 2026-10-11 with the room lock taken out of `store_upload`: "two uploads finishing
/// together both passed the room check".
#[test]
fn two_uploads_finishing_together_cannot_both_pass_the_room_check() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let state = relay("media_f4");
    let db = &state.db;
    let mut s = db.get_server_settings().unwrap();
    s.media_per_person_mb = 1;
    let (uploads, members, resume) = temp_folders("media_f4");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let arrived = AtomicUsize::new(0);
    let results: Vec<Result<Vec<String>, Refused>> = std::thread::scope(|scope| {
        let runs: Vec<_> = ["1_a.mp4", "2_b.mp4"]
            .into_iter()
            .map(|name| {
                let (s, arrived) = (&s, &arrived);
                scope.spawn(move || {
                    let u = NewUpload { key: "ann", shelf: MEDIA_SHELF, name, original_name: name, len: 600 * 1024, keep: 4 };
                    store_upload(db, s, folders, &u, |path| {
                        arrived.fetch_add(1, Ordering::SeqCst);
                        let start = std::time::Instant::now();
                        while arrived.load(Ordering::SeqCst) < 2 && start.elapsed() < std::time::Duration::from_millis(500) {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        std::fs::write(path, b"video")
                    })
                })
            })
            .collect();
        runs.into_iter().map(|r| r.join().expect("an upload thread")).collect()
    });
    let passed = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(passed, 1, "two uploads finishing together both passed the room check: {results:?}");
    let refused = results.iter().find_map(|r| r.as_ref().err()).unwrap();
    assert_eq!(refused.0, StatusCode::INSUFFICIENT_STORAGE, "{}", refused.1);
    assert_eq!(db.media_bytes(Some("ann")), 600 * 1024, "the share held");
}

/// F5. A SIGNATURE IS GOOD FOR ONE THING, ONCE: a timestamp 31 seconds ahead is refused (five
/// minutes ahead used to pass, outliving the replay memory); and each write's signature names
/// its target, so one made for one item, file, decision or category list is refused for another.
///
/// Seen red 2026-10-11 with the ahead limit back at five minutes: "a timestamp 31 seconds ahead
/// was taken". Seen red again with `feature`'s purpose without its item id, plain
/// `"media_feature"` on both sides as merged: "a signature for one item was taken for another /
/// left: Ok(()) / right: Err(401)".
#[tokio::test]
async fn a_signature_names_its_target_and_is_never_far_ahead() {
    let state = relay("media_f5");
    let db = &state.db;
    let (op, ann) = (person(29), person(30));
    db.join_server(&op.key, "Op").unwrap();
    db.join_server(&ann.key, "Ann").unwrap();
    db.set_role(&op.key, "admin").unwrap();
    let (a, b) = (published(db, &ann.key, "1_a.png", "public"), published(db, &ann.key, "2_b.png", "public"));
    let now = now_ms();
    let feature_body = |id: i64, signed_for: i64, ts: u64| FeatureBody {
        key: op.key.clone(),
        timestamp: ts,
        sig: op.sign(&format!("media_feature:{signed_for}"), ts),
        id,
        featured: true,
    };

    let ahead = feature(State(state.clone()), Json(feature_body(a.id, a.id, now + 31_000))).await;
    assert!(ahead.is_err(), "a timestamp 31 seconds ahead was taken");
    assert_eq!(ahead.unwrap_err().0, StatusCode::BAD_REQUEST);
    feature(State(state.clone()), Json(feature_body(a.id, a.id, now + 20_000))).await.expect("20 seconds ahead is a clock a little fast");

    let moved = feature(State(state.clone()), Json(feature_body(b.id, a.id, now))).await;
    assert_eq!(moved.map(|_| ()).map_err(|(code, _)| code), Err(StatusCode::UNAUTHORIZED), "a signature for one item was taken for another");
    assert!(!db.media_item(b.id).unwrap().unwrap().featured);

    let pick_b = ItemBody { key: op.key.clone(), timestamp: now, sig: op.sign(&format!("media_pick:{}", a.id), now), id: b.id };
    assert_eq!(pick(State(state.clone()), Json(pick_b)).await.unwrap_err().0, StatusCode::UNAUTHORIZED, "a pick");
    let remove_b = RemoveBody { key: op.key.clone(), timestamp: now, sig: op.sign("media_remove:1_a.png", now), filename: Some("2_b.png".into()), id: None };
    assert_eq!(remove(State(state.clone()), Json(remove_b)).await.unwrap_err().0, StatusCode::UNAUTHORIZED, "a removal by name");
    let suggestion = db.suggest_media_category(&ann.key, "Builds", "").unwrap();
    let decline = DecideBody { key: op.key.clone(), timestamp: now, sig: op.sign(&decide_purpose(suggestion.id, true), now), id: suggestion.id, accept: false };
    assert_eq!(decide(State(state.clone()), Json(decline)).await.unwrap_err().0, StatusCode::UNAUTHORIZED, "an accept sent as a decline");

    let list = r#"[{"id":1,"name":"General"},{"name":"Builds"}]"#;
    let hash = blake3::hash(list.as_bytes()).to_hex();
    let sig = op.sign(&format!("media_categories:{hash}"), now);
    let other = format!(r#"{{"key":"{}","timestamp":{now},"sig":"{sig}","categories":[{{"id":1,"name":"General"}},{{"name":"Ghosts"}}]}}"#, op.key);
    assert_eq!(set_categories(State(state.clone()), other).await.unwrap_err().0, StatusCode::UNAUTHORIZED, "another category list");
    let sent = format!(r#"{{"key":"{}","timestamp":{now},"sig":"{sig}","categories":{list}}}"#, op.key);
    let done = set_categories(State(state.clone()), sent).await.expect("the list signed as sent").0;
    assert_eq!(done["categories"][1]["name"], "Builds");
}

/// F6. SIZES COME FROM THE FILE: publishing an upload whose row says 0 bytes (a row from before
/// sizes were kept) records its size on disk, so the budget and the person's share count it.
///
/// Seen red 2026-10-11 with the size update taken out of `publish_media_placing`: "a zero-size
/// upload was counted as nothing / left: 0 / right: 2048".
#[test]
fn a_zero_size_upload_is_counted_by_its_size_on_disk() {
    let state = relay("media_f6");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let (uploads, members, resume) = temp_folders("media_f6");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let ann = person(31);
    db.join_server(&ann.key, "Ann").unwrap();
    std::fs::write(uploads.join("1_old.png"), vec![7u8; 2048]).unwrap();
    db.record_upload(&ann.key, "1_old.png", 100, 0i64, "old.png", 0).unwrap();
    let body = PublishBody { key: ann.key.clone(), filename: "1_old.png".into(), ..Default::default() };
    let item = publish_core(db, &s, &[], folders, &body).unwrap().0;
    assert_eq!(item["item"]["size_bytes"], 2048);
    assert_eq!(db.media_bytes(None), 2048, "a zero-size upload was counted as nothing");
    assert_eq!(db.media_bytes(Some(&ann.key)), 2048, "in the person's share too");
}

/// F7. FAIL CLOSED: a visibility word the relay does not know, and a settings read that fails,
/// count as members only, so the public gets no items (and, on a members-only page, no
/// categories or labels either) while a member still gets everything.
///
/// Seen red 2026-10-11 with `media_settings` reading `unwrap_or_default()` (a public page): "a
/// failed settings read served the public / left: [1] / right: []".
#[tokio::test]
async fn the_media_page_fails_closed() {
    let state = relay("media_f7");
    let db = &state.db;
    let ann = person(32);
    db.join_server(&ann.key, "Ann").unwrap();
    let open = published(db, &ann.key, "1_open.png", "public");
    db.with_conn(|c| c.execute("UPDATE server_settings SET media_visibility = 'everyone'", [])).unwrap();

    let unsigned = list(State(state.clone()), Query(ListParams::default())).await.unwrap().0;
    assert_eq!(ids(&unsigned), Vec::<i64>::new(), "an unknown visibility word served the public");
    assert_eq!((unsigned["categories"].clone(), unsigned["labels"].clone()), (json!([]), json!([])), "nor its categories and labels");
    let cats = categories(State(state.clone()), Query(ListParams::default())).await.unwrap().0;
    assert_eq!((cats["categories"].clone(), cats["members_only_for_you"].clone()), (json!([]), json!(true)));
    let member = list_post(State(state.clone()), Json(signed_list(&ann, now_ms()))).await.unwrap().0;
    assert_eq!(ids(&member), vec![open.id], "a member still sees the page");
    assert_eq!(member["categories"][0]["name"], "General", "and its categories");
    let cats = categories(State(state.clone()), Query(signed_list(&ann, now_ms() + 1))).await.unwrap().0;
    assert_eq!(cats["labels"].as_array().map(|l| l.len()), Some(labels().len()), "and its labels");

    db.with_conn(|c| c.execute_batch("ALTER TABLE server_settings RENAME TO server_settings_unreadable")).unwrap();
    assert!(db.get_server_settings().is_err(), "the settings cannot be read now");
    let unsigned = list(State(state.clone()), Query(ListParams::default())).await.unwrap().0;
    assert_eq!(ids(&unsigned), Vec::<i64>::new(), "a failed settings read served the public");
    assert_eq!(unsigned["members_only_for_you"], true);
}

/// A FILE ALREADY PUBLIC STAYS PUBLIC: a chat upload is at its `/uploads/` address in the
/// messages that carry it, and a Shared Files one is in that list, so either goes on the Media
/// page as public only. Asking for members only is refused with words that say why and what to do
/// instead, at publishing and at the switch, and the file stays where it was: still at its
/// address, still in the Shared Files list. Removed from Media, each goes back to its shelf.
///
/// Seen red 2026-10-11 with `was_public` answering false: "a chat file was published as members
/// only".
#[test]
fn a_file_already_public_goes_on_media_as_public_only() {
    let state = relay("media_already_public");
    let db = &state.db;
    let s = db.get_server_settings().unwrap();
    let (uploads, members, resume) = temp_folders("media_already_public");
    let folders = Folders { uploads: &uploads, members: &members, resume: &resume };
    let ann = person(31);
    db.join_server(&ann.key, "Ann").unwrap();
    for (file, shelf) in [("1_chat.png", 0i64), ("2_shared.glb", 1)] {
        std::fs::write(uploads.join(file), b"bytes").unwrap();
        db.record_upload(&ann.key, file, 100, shelf, file, 5).unwrap();
    }
    let body = |file: &str, visibility: &str| PublishBody {
        key: ann.key.clone(),
        filename: file.into(),
        visibility: Some(visibility.into()),
        ..Default::default()
    };

    for file in ["1_chat.png", "2_shared.glb"] {
        let refused = publish_core(db, &s, &[], folders, &body(file, "members"));
        let (code, words) = refused.err().unwrap_or_else(|| panic!("a chat file was published as members only: {file}"));
        assert_eq!(code, StatusCode::BAD_REQUEST, "{file}");
        assert!(words.contains("already public") && words.contains("upload it on the Media page"), "{words}");
        assert!(uploads.join(file).exists() && !members.join(file).exists(), "the refusal moved {file}");
    }
    assert_eq!(shared_list(db), vec!["2_shared.glb".to_string()], "the Shared Files file is still listed");
    assert!(db.list_media(&MediaQuery { members: true, ..Default::default() }).unwrap().is_empty(), "nothing was published");

    let chat = publish_core(db, &s, &[], folders, &body("1_chat.png", "public")).unwrap().0;
    let shared = publish_core(db, &s, &[], folders, &body("2_shared.glb", "public")).unwrap().0;
    for item in [&chat, &shared] {
        let id = item["item"]["id"].as_i64().unwrap();
        let file = item["item"]["url"].as_str().unwrap().trim_start_matches("/uploads/").to_string();
        let (code, words) = visibility_core(db, folders, id, &ann.key, "members").unwrap_err();
        assert!(code == StatusCode::BAD_REQUEST && words.contains("already public"), "{words}");
        assert!(uploads.join(&file).exists() && !members.join(&file).exists(), "the switch moved {file}");
        assert_eq!(db.media_item(id).unwrap().unwrap().visibility, "public");
    }
    assert_eq!(shared_list(db), vec!["2_shared.glb".to_string()], "a public Shared Files file stays in that list");

    for item in [&chat, &shared] {
        remove_core(db, folders, &RemoveTarget::Item(item["item"]["id"].as_i64().unwrap()), &ann.key).unwrap();
    }
    assert_eq!((shelf_of(db, "1_chat.png"), shelf_of(db, "2_shared.glb")), (Some(0), Some(1)), "removed, each is back on its shelf");
}
