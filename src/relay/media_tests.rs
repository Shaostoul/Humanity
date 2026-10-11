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

/// `key` uploads `file` as chat media and publishes it with `visibility`.
fn published(db: &Storage, key: &str, file: &str, visibility: &str) -> MediaItem {
    db.record_upload(key, file, 100, 0i64, file, 10).unwrap();
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
    let uploads = crate::test_temp::dir("media_publish_uploads");
    let (ann, out) = (person(6), person(7));
    db.join_server(&ann.key, "Ann").unwrap();
    db.record_upload(&ann.key, "1_a.png", 100, 0i64, "a.png", 10).unwrap();
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
    assert_eq!(publish_core(db, &s, &known, &uploads, &outsider).unwrap_err().0, StatusCode::FORBIDDEN, "a non-member");
    let (code, words) = publish_core(db, &s, &known, &uploads, &body(&["gore"], None)).unwrap_err();
    assert!(code == StatusCode::BAD_REQUEST && words.contains("Flashing lights (flashing)"), "{words}");
    let (_, words) = publish_core(db, &s, &known, &uploads, &body(&["NSFW"], None)).unwrap_err();
    assert!(words.contains("Adult content is not allowed"), "{words}");
    assert_eq!(publish_core(db, &s, &known, &uploads, &body(&[], Some("friends"))).unwrap_err().0, StatusCode::BAD_REQUEST);

    let ok = publish_core(db, &s, &known, &uploads, &body(&["Graphic", "spoiler", "graphic"], Some("Members"))).unwrap().0;
    let labels: Vec<&str> = ok["item"]["labels"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    assert_eq!(labels, vec!["spoiler", "graphic"], "labels are stored in the labels file's order");
    assert_eq!(ok["item"]["visibility"], "members");
    assert_eq!(ok["item"]["url"], "/uploads/1_a.png");
    assert_eq!(ok["item"]["category"]["id"], GENERAL_CATEGORY_ID);
}

/// TAKING AN ITEM DOWN THROUGH THE ROUTE deletes its file: a member who is not the uploader is
/// refused and the file stays; an admin's removal deletes it.
///
/// Seen red 2026-10-11 with the file delete taken out of `remove_core`: "the file is gone".
#[test]
fn removing_an_item_deletes_its_file() {
    let state = relay("media_remove_route");
    let db = &state.db;
    let uploads = crate::test_temp::dir("media_remove_uploads");
    let (ann, cal, op) = (person(8), person(9), person(10));
    for (p, n) in [(&ann, "Ann"), (&cal, "Cal"), (&op, "Op")] {
        db.join_server(&p.key, n).unwrap();
    }
    db.set_role(&op.key, "admin").unwrap();
    std::fs::write(uploads.join("1_a.png"), b"picture").unwrap();
    let item = published(db, &ann.key, "1_a.png", "public");

    assert_eq!(remove_core(db, &uploads, item.id, &cal.key).unwrap_err().0, StatusCode::FORBIDDEN);
    assert!(uploads.join("1_a.png").exists(), "a refused removal left the file");
    let done = remove_core(db, &uploads, item.id, &op.key).unwrap().0;
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
    let body = DecideBody { key: op.key.clone(), timestamp: ts, sig: op.sign("media_suggestions", ts), id, accept: true };
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
    let body = |ts: u64, signer: &Person| FeatureBody { key: op.key.clone(), timestamp: ts, sig: signer.sign("media_feature", ts), id: item.id, featured: true };

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
