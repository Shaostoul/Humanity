//! Media round 5's contract on the relay (2026-10-12; docs/design/media-round-5-plan.md, Wave 0):
//! a ticket opens one of an item's files and no other, and the shared fixture both apps read
//! (scripts/tests/fixtures/media-round5-v1.json) is what the relay makes. A child of
//! media_round3_tests.rs, so its relay, folders and file helpers are at hand. Each test names the
//! break it was seen failing against ("Seen red").

use super::*;
use crate::relay::media_edit::{edit_purpose, edit_request, file_add_purpose, file_remove_purpose};
use crate::relay::storage::media::{CategoryEdit, ROLE_COVER, ROLE_FILE};

/// A GET of file `file` of item `id` with `ticket` (`?file=` left out when `file` is None).
async fn open_file(state: &RelayState, d: &Dirs, id: i64, file: Option<i64>, ticket: &str) -> (u16, Vec<u8>) {
    let uri = match file {
        Some(n) => format!("/api/media/file/{id}?file={n}&ticket={ticket}"),
        None => format!("/api/media/file/{id}?ticket={ticket}"),
    };
    let req = axum::http::Request::builder().uri(uri).body(axum::body::Body::empty()).unwrap();
    let q = TokenQuery { ticket: Some(ticket.into()), file, ..Default::default() };
    let res = file_from(state, d.folders(), id, q, req).await;
    let code = res.status().as_u16();
    (code, axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec())
}

/// A TICKET OPENS ONLY THE FILE IT WAS GIVEN FOR (round 5: a members-only item can hold several):
/// a ticket for an added file opens that file, at `/api/media/file/<id>?file=<n>&ticket=`, and not
/// the item's own; a ticket for the item's own file opens it, with or without `?file=0`, and not
/// the added one; asked with `download`, the address is the counted download route; a file number
/// the item does not have is given no ticket.
///
/// Seen red 2026-10-12 with `admitted` comparing the item's own file name whatever file was asked:
/// "a ticket for the added file opens it / left: (403, [84, 104, 97, ...]) / right: (200, [69,
/// 88, 84, 82, 65])".
#[tokio::test]
async fn a_ticket_opens_only_the_file_it_was_given_for() {
    let state = relay("mr5_ticket");
    let d = dirs("mr5_ticket");
    let item = member_with_item(&state, &d, "ann", "tok_ann", "1_clip.mp4");
    std::fs::write(d.waiting.join("2_extra.mp4"), b"EXTRA").unwrap();
    state.db.record_upload("ann", "2_extra.mp4", 100, MEDIA_SHELF, "extra.mp4", 5).unwrap();
    let f = d.folders();
    let added = state.db.attach_media_file("ann", item.id, "2_extra.mp4", ROLE_FILE, 0, |members| f.place("2_extra.mp4", members)).unwrap().item;
    let n = added.files[0].file;
    assert!(d.members.join("2_extra.mp4").is_file(), "test setup: the added file of a members-only item is in the members' folder");
    let body = |file: i64, download: bool| media_ticket::TicketBody { id: item.id, filename: item.upload_filename.clone(), file, download };

    let answer = media_ticket::ticket_core(&state, "ann", &body(n, false), Instant::now()).unwrap().0;
    let extra = ticket_of(&answer);
    assert_eq!(answer["url"], format!("/api/media/file/{}?file={n}&ticket={extra}", item.id));
    assert_eq!(open_file(&state, &d, item.id, Some(n), &extra).await, (200, b"EXTRA".to_vec()), "a ticket for the added file opens it");
    assert_eq!(open_file(&state, &d, item.id, None, &extra).await.0, 403, "and not the item's own file");
    assert_eq!(open_file(&state, &d, item.id, Some(0), &extra).await.0, 403);

    let own = ticket_of(&media_ticket::ticket_core(&state, "ann", &body(0, false), Instant::now()).unwrap().0);
    assert_eq!(open_file(&state, &d, item.id, None, &own).await, (200, b"0123456789".to_vec()), "the item's own, file 0 by default");
    assert_eq!(open_file(&state, &d, item.id, Some(0), &own).await.0, 200);
    assert_eq!(open_file(&state, &d, item.id, Some(n), &own).await.0, 403, "and not the added one");

    let download = media_ticket::ticket_core(&state, "ann", &body(n, true), Instant::now()).unwrap().0;
    assert!(download["url"].as_str().unwrap().starts_with(&format!("/api/media/download/{}?file={n}&ticket=", item.id)), "{download}");
    assert_eq!(media_ticket::ticket_core(&state, "ann", &body(999, false), Instant::now()).unwrap_err().0, StatusCode::NOT_FOUND, "no such file");
}

// ── The shared fixture (plan 3.12) ──

/// The fixture both apps read.
const FIXTURE: &str = "scripts/tests/fixtures/media-round5-v1.json";
/// The uploader's and the viewer's keys: stand-ins (a real key is 3,904 hex characters), the same
/// in every run.
const ANN: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const BEA: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
/// The edit's signed timestamp in the fixture's body.
const EDIT_TS: u64 = 1_760_000_003_000;

/// The fixture as the relay makes it today, from fixed rows: Ann's "Dusk" (a picture under CC BY 4.0,
/// offered for the game, labelled, featured in its category, two more files counted as one added
/// file and a cover, three downloads, five views, one pick) and her "Staff" (a model, all rights
/// reserved), seen by Bea, a member.
fn fixture_now() -> Value {
    let state = relay("mr5_fixture");
    let db = &state.db;
    let d = dirs("mr5_fixture");
    db.join_server(ANN, "Ann").unwrap();
    db.join_server(BEA, "Bea").unwrap();
    let cats = db
        .replace_media_categories(&[
            CategoryEdit { id: Some(1), name: "General".into(), description: String::new() },
            CategoryEdit { id: None, name: "Fire flow arts".into(), description: "Staff, poi, fans and the rest".into() },
        ])
        .unwrap();
    let fire = cats[1].id;
    for (file, original, size) in [("1760_x.png", "dusk.png", 1234), ("1761_y.zip", "textures.zip", 4000), ("1762_c.jpg", "cover.jpg", 4642), ("1763_z.glb", "staff.glb", 9000)] {
        std::fs::write(d.waiting.join(file), b"x").unwrap();
        db.record_upload(ANN, file, 100, MEDIA_SHELF, original, size).unwrap();
    }
    let s = db.get_server_settings().unwrap();
    let dusk = PublishBody {
        key: ANN.into(),
        filename: "1760_x.png".into(),
        title: "Dusk".into(),
        description: "First line\nThe rest".into(),
        category_id: Some(fire),
        labels: vec!["flashing".into()],
        licence: Some("cc_by_4".into()),
        project_use: true,
        ..Default::default()
    };
    publish_core(db, &s, labels(), d.folders(), &dusk).expect("Dusk is published");
    let staff = PublishBody { key: ANN.into(), filename: "1763_z.glb".into(), title: "Staff".into(), ..Default::default() };
    publish_core(db, &s, labels(), d.folders(), &staff).expect("Staff is published");
    let f = d.folders();
    db.attach_media_file(ANN, 1, "1761_y.zip", ROLE_FILE, 0, |members| f.place("1761_y.zip", members)).unwrap();
    db.attach_media_file(ANN, 1, "1762_c.jpg", ROLE_COVER, 0, |members| f.place("1762_c.jpg", members)).unwrap();
    db.set_media_featured(1, Featured::Category).unwrap();
    db.pick_media(BEA, 1).unwrap();
    db.with_conn(|c| {
        c.execute_batch(
            "UPDATE media_items SET downloads = 3, views = 5 WHERE id = 1;
             UPDATE user_uploads SET uploaded_at = 1760000000000;
             UPDATE media_items SET created_at = 1760000001000 + id;
             UPDATE media_item_files SET added_at = 1760000002000;",
        )
    })
    .unwrap();

    let list_params = ListParams { sort: Some("downloads".into()), limit: Some(1), ..Default::default() };
    let list = list_core(db, &s, &list_params, Some(BEA), None).expect("the list").0;
    db.with_conn(|c| c.execute("UPDATE media_items SET visibility = 'members' WHERE id = 1", [])).unwrap();
    let members_only = item_json(&db.media_item(1).unwrap().unwrap());

    // The edit: every field, its keys in the order serde_json writes them (by name), as sent.
    let edit = json!({ "title": "Dusk", "description": "First line\nThe rest", "category_id": fire, "labels": ["flashing"], "licence": "cc_by_4", "project_use": true });
    let edit_text = edit.to_string();
    let body = format!("{{\"key\":\"{ANN}\",\"timestamp\":{EDIT_TS},\"sig\":\"SIG\",\"id\":1,\"filename\":\"1760_x.png\",\"edit\":{edit_text}}}");
    let (_, sent, purpose) = edit_request(&body).expect("the edit parses");
    assert_eq!(sent, edit_text, "the edit as sent is the text hashed");
    let file_no = db.media_item(1).unwrap().unwrap().files[0].file;

    json!({
        "about": "Media round 5's contract (docs/design/media-round-5-plan.md 3.2 to 3.4 and 3.12, with section 10's answers). Made by the relay's own code from fixed rows (src/relay/media_round5_tests.rs, write_the_media_round5_fixture; checked by the_shared_fixture_is_what_the_relay_makes); read by both apps' tests. `item_members_view` is one item as a member sees it in the list (picked_by_me included); `item_members_only` the same item once it is members only (every url through /api/media/file/<id>?file=<n>); `list_by_downloads` the list's answer to `list_request` (sort downloads, one to a page) with its next_cursor and the server's licences; `licences` that answer's licences on their own (the offered licences, the game tick, the two warnings and the hosting paragraph shown beside Add to Media); `edit` an edit request's body exactly as sent, its `edit` member's text (keys in name order) and the purpose signed over its BLAKE3; `purposes` the other writes' purposes. Keys are stand-ins and `SIG` stands for a Dilithium3 signature over `purpose\\ntimestamp`.",
        "item_members_view": list["items"][0],
        "item_members_only": members_only,
        "list_request": { "sort": "downloads", "limit": 1 },
        "list_by_downloads": list,
        "licences": crate::relay::media_licences::licences().answer(),
        "edit": { "id": 1, "filename": "1760_x.png", "timestamp": EDIT_TS, "text": edit_text, "body": body, "purpose": purpose },
        "purposes": {
            "publish": publish_purpose("1760_x.png", "cc_by_4", true),
            "publish_default": publish_purpose("1763_z.glb", crate::relay::storage::media::DEFAULT_LICENCE, false),
            "feature_main": feature_purpose(1, Featured::Main),
            "feature_category": feature_purpose(1, Featured::Category),
            "feature_none": feature_purpose(1, Featured::None),
            "file_add": file_add_purpose(1, "1761_y.zip", "file"),
            "cover_add": file_add_purpose(1, "1762_c.jpg", "cover"),
            "file_remove": file_remove_purpose(1, file_no),
        },
    })
}

/// WRITES scripts/tests/fixtures/media-round5-v1.json from fixed rows. Run it again only when the
/// contract changes on purpose: `node scripts/lib-tests.js write_the_media_round5_fixture --ignored`.
#[test]
#[ignore = "writes scripts/tests/fixtures/media-round5-v1.json; run: node scripts/lib-tests.js write_the_media_round5_fixture --ignored"]
fn write_the_media_round5_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fixture_now()).unwrap())).expect("the fixture is written");
}

/// THE SHARED FIXTURE IS WHAT THE RELAY MAKES: the file both apps read is, byte for byte, what the
/// relay's own code makes today from the same rows (an item's shape, the list's answer and its
/// cursor, the licences, the edit's purpose over its text as sent, and the other purposes), so a
/// change to the wire shows here first. The edit's purpose is the one the relay checks for that
/// body, and a body whose edit was changed after it was signed gets another.
///
/// Seen red 2026-10-12 with `featured` sent as a bool again (`i.featured.is_on()` in `item_json`):
/// "the fixture is what the relay makes today (run write_the_media_round5_fixture after an
/// intended change)" (the relay made `"featured": true` where the fixture says `"category"`).
#[test]
fn the_shared_fixture_is_what_the_relay_makes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let text = std::fs::read_to_string(&path).expect("the fixture is there").replace("\r\n", "\n");
    let made = format!("{}\n", serde_json::to_string_pretty(&fixture_now()).unwrap());
    let fx: Value = serde_json::from_str(&text).expect("JSON");
    let item = &fx["item_members_view"];
    assert_eq!((item["featured"].as_str(), item["licence"].as_str(), item["project_use"].as_bool()), (Some("category"), Some("cc_by_4"), Some(true)));
    assert_eq!(item["files"].as_array().map(Vec::len), Some(2), "the item's own file and the added one");
    assert_eq!((item["files"][0]["file"].as_i64(), item["cover"]["filename"].as_str(), item["link"].as_str()), (Some(0), Some("1762_c.jpg"), Some("/media/1")));
    assert_eq!(fx["list_by_downloads"]["next_cursor"].as_str(), Some("3.1"), "the cursor after Dusk's three downloads");
    assert!(fx["item_members_only"]["files"][1]["url"].as_str().is_some_and(|u| u.starts_with("/api/media/file/1?file=")));
    let (_, _, purpose) = edit_request(fx["edit"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(Some(purpose.as_str()), fx["edit"]["purpose"].as_str(), "the purpose the relay checks for that body");
    assert_eq!(purpose, edit_purpose(1, fx["edit"]["text"].as_str().unwrap()));
    let changed = fx["edit"]["body"].as_str().unwrap().replace("\"title\":\"Dusk\"", "\"title\":\"Dawn\"");
    assert_ne!(edit_request(&changed).unwrap().2, purpose, "an edit changed after it was signed has another purpose");
    assert!(text == made, "the fixture is what the relay makes today (run write_the_media_round5_fixture after an intended change)");
}
