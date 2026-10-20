//! Media round 5's storage (storage/files_items.rs and the parts of storage/files.rs it changed;
//! docs/design/media-round-5-plan.md 3.1). Each test names the break it was seen failing against
//! ("Seen red"), with the words it failed with.

use super::super::{CategoryEdit, FilesQuery, Publish, DEFAULT_LICENCE, MEDIA_SHELF};
use super::*;

/// The item numbers of one page of the list for `q`.
fn ids_of(db: &Storage, q: &FilesQuery) -> Vec<i64> {
    db.list_media(q).unwrap().into_iter().map(|i| i.id).collect()
}

const MIB: i64 = 1024 * 1024;
/// A budget and share nothing in these tests comes near.
const ROOMY: i64 = 1 << 40;

fn db() -> Storage {
    Storage::open_temp("media_round5")
}

/// An upload by `key` on `shelf` (0 chat, 1 Shared Files, 2 Media) of `size` bytes.
fn upload(db: &Storage, key: &str, file: &str, shelf: i64, size: i64) {
    db.record_upload(key, file, 100, shelf, file, size).unwrap();
}

fn publish(db: &Storage, key: &str, file: &str) -> Result<FilesItem, FilesError> {
    db.publish_media(key, &Publish { filename: file, visibility: "public", ..Default::default() }, ROOMY, ROOMY, None)
}

/// `key` attaches the waiting upload `file` to item `id` as `role` (no folder to move it in).
fn attach(db: &Storage, key: &str, id: i64, file: &str, role: &str) -> Result<Attached, FilesError> {
    db.attach_media_file(key, id, file, role, 0, |_| Ok(false))
}

fn why<T: std::fmt::Debug>(r: Result<T, FilesError>) -> (Refusal, String) {
    match r {
        Err(FilesError::Refused(w, words)) => (w, words),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn count(db: &Storage, sql: &str) -> i64 {
    db.with_conn(|c| c.query_row(sql, [], |r| r.get(0))).unwrap()
}

/// BUG-046 FOR ROUND 5: A DATABASE FROM v0.1489.0 OPENS AND MIGRATES. Its `media_items` already
/// exists without `licence`, `project_use` and `downloads` (exactly storage/mod.rs's CREATE TABLE
/// as v0.1489.0 shipped it), with one item featured and its upload. `Storage::open` survives; the
/// item reads "All rights reserved", not offered for the game, no downloads, featured on the main
/// shelf (a stored 1 means what it did), no added files and no cover; a list sorted by downloads
/// (which the new index serves) answers; and a second open changes nothing.
///
/// Seen red 2026-10-12 with the downloads index moved into the media tables' batch (before the
/// ALTERs): "startup must survive a v0.1489.0 media database: SqlInputError { error: Error { code:
/// Unknown, extended_code: 1 }, msg: \"no such column: downloads\", ...".
#[test]
fn opens_a_v0_1489_media_database_and_migrates_it() {
    let path = crate::test_temp::db("media_v1489");
    {
        let conn = rusqlite::Connection::open(&path).expect("raw open");
        conn.execute_batch(
            "CREATE TABLE user_uploads (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                filename    TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL,
                shared        INTEGER NOT NULL DEFAULT 0,
                original_name TEXT NOT NULL DEFAULT '',
                size_bytes    INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO user_uploads (public_key, filename, uploaded_at, shared, original_name, size_bytes)
                VALUES ('ann', '1760_x.png', 1760000000000, 2, 'dusk.png', 1234);
            CREATE TABLE IF NOT EXISTS media_items (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                upload_filename TEXT    NOT NULL UNIQUE,
                uploader_key    TEXT    NOT NULL,
                title           TEXT    NOT NULL DEFAULT '',
                description     TEXT    NOT NULL DEFAULT '',
                category_id     INTEGER NOT NULL,
                kind            TEXT    NOT NULL,
                labels          TEXT    NOT NULL DEFAULT '',
                visibility      TEXT    NOT NULL DEFAULT 'public',
                featured        INTEGER NOT NULL DEFAULT 0,
                size_bytes      INTEGER NOT NULL,
                views           INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL,
                from_shelf      INTEGER NOT NULL,
                kept_by         TEXT,
                kept_at         INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_media_items_uploader ON media_items(uploader_key, id);
            CREATE INDEX IF NOT EXISTS idx_media_items_category ON media_items(category_id, id);
            INSERT INTO media_items (upload_filename, uploader_key, title, category_id, kind, visibility, featured, size_bytes, created_at, from_shelf)
                VALUES ('1760_x.png', 'ann', 'Dusk', 1, 'picture', 'public', 1, 1234, 1760000001000, 2);",
        )
        .expect("a v0.1489.0 media database");
    }

    let db = Storage::open(&path).expect("startup must survive a v0.1489.0 media database");
    let item = db.media_item(1).unwrap().expect("the item is still there");
    assert_eq!((item.licence.as_str(), item.project_use, item.downloads), (DEFAULT_LICENCE, false, 0));
    assert_eq!(item.featured, Featured::Main, "a stored 1 is the main shelf, as it was");
    assert!(item.files.is_empty() && item.cover.is_none());
    assert_eq!((item.file_count, item.total_bytes), (1, 1234));
    let by_downloads = db.list_media(&FilesQuery { sort: Sort::Downloads, ..Default::default() }).unwrap();
    assert_eq!(by_downloads.iter().map(|i| i.id).collect::<Vec<_>>(), vec![1], "a list sorted by downloads answers");
    let columns = |db: &Storage| count(db, "SELECT COUNT(*) FROM pragma_table_info('media_items')");
    assert_eq!(columns(&db), 19, "sixteen columns and the three new ones");
    drop(db);

    let again = Storage::open(&path).expect("a second open");
    assert_eq!(columns(&again), 19, "a second open adds nothing");
    assert_eq!(again.media_item(1).unwrap(), Some(item), "and changes nothing");
}

/// AN UPLOAD ALREADY ON AN ITEM IS NEVER PUBLISHED AGAIN: an item's own file is refused as before
/// ("already on the Files page"), and so is a file added to an item ("already part of a Media
/// item"), which `media_item_holding` finds as that item's file; adding either to an item again is
/// refused the same way.
///
/// Seen red 2026-10-12 with the holding check in `publish_media_placing` back to `media_items`
/// alone: "expected a refusal, got Ok(FilesItem { id: 2, upload_filename: \"2_extra.zip\", ...".
#[test]
fn a_published_upload_or_an_attached_one_is_never_published_again() {
    let db = db();
    upload(&db, "ann", "1_main.png", MEDIA_SHELF, 10);
    upload(&db, "ann", "2_extra.zip", MEDIA_SHELF, 20);
    let item = publish(&db, "ann", "1_main.png").unwrap();
    let added = attach(&db, "ann", item.id, "2_extra.zip", ROLE_FILE).unwrap().item;
    let file_no = added.files[0].file;
    assert_eq!(db.media_item_holding("2_extra.zip").unwrap(), Some((item.id, file_no)), "an added file is held by its item");
    assert_eq!(db.media_item_holding("1_main.png").unwrap(), Some((item.id, 0)), "the item's own file is its file 0");
    assert_eq!(db.media_item_holding("3_none.png").unwrap(), None);

    assert_eq!(why(publish(&db, "ann", "2_extra.zip")), (Refusal::Conflict, ALREADY_ON_AN_ITEM.to_string()), "an added file");
    let (w, words) = why(publish(&db, "ann", "1_main.png"));
    assert_eq!(w, Refusal::Conflict);
    assert!(words.contains("already on the Files page"), "{words}");
    assert_eq!(why(attach(&db, "ann", item.id, "2_extra.zip", ROLE_FILE)).0, Refusal::Conflict, "added twice");
    assert_eq!(why(attach(&db, "ann", item.id, "1_main.png", ROLE_FILE)).0, Refusal::Conflict, "an item's own file added to it");
}

/// EVERY FILE COUNTS AGAINST THE SPACE AND THE SHARE: an item put on the Files page from the Shared
/// Files list (1 MB), two added files (2 and 3 MB) and a cover (4 MB) count 10 MB against the
/// space and against Ann's share; the item says so (`total_bytes`, and three files without the
/// cover); and a publish past her share is refused naming the 10 MB.
///
/// Seen red 2026-10-12 with `attach_media_file` also moving the added file's row to the item's own
/// file's shelf (`UPDATE user_uploads SET shared = 1`, off the Media shelf): "every file counts
/// against Ann's share / left: 1048576 / right: 10485760".
#[test]
fn every_file_counts_against_the_space_and_the_share() {
    let db = db();
    upload(&db, "ann", "1_main.png", 1, MIB);
    upload(&db, "ann", "2_a.zip", MEDIA_SHELF, 2 * MIB);
    upload(&db, "ann", "3_b.stl", MEDIA_SHELF, 3 * MIB);
    upload(&db, "ann", "4_cover.jpg", MEDIA_SHELF, 4 * MIB);
    let item = publish(&db, "ann", "1_main.png").unwrap();
    attach(&db, "ann", item.id, "2_a.zip", ROLE_FILE).unwrap();
    attach(&db, "ann", item.id, "3_b.stl", ROLE_FILE).unwrap();
    let item = attach(&db, "ann", item.id, "4_cover.jpg", ROLE_COVER).unwrap().item;

    assert_eq!(db.media_bytes(Some("ann")), 10 * MIB, "every file counts against Ann's share");
    assert_eq!(db.media_bytes(None), 10 * MIB, "and against the space");
    assert_eq!((item.total_bytes, item.file_count), (10 * MIB, 3), "the item's own, two added and the cover; three files");
    assert_eq!(item.cover.as_ref().map(|c| c.filename.as_str()), Some("4_cover.jpg"));
    let usage = db.media_usage().unwrap();
    assert_eq!(usage.people.first().map(|p| p.bytes), Some(10 * MIB));
    assert_eq!(usage.largest.first().map(|i| i.total_bytes), Some(10 * MIB), "the largest list goes by the whole item");

    upload(&db, "ann", "5_more.png", 0, MIB);
    let (w, words) = why(db.publish_media("ann", &Publish { filename: "5_more.png", visibility: "public", ..Default::default() }, ROOMY, 10 * MIB + MIB / 2, None));
    assert_eq!(w, Refusal::Full);
    assert!(words.contains("10.0 MB of your 10.5 MB used"), "the share's words name the whole item: {words}");
}

/// A KEPT ITEM KEEPS THE FILES IT HAD WHEN IT WAS KEPT (decision 2): kept with one added file, the
/// item and that file count against the space and nobody's share; a file added after the keep
/// counts against Ann's share until someone keeps the item again.
///
/// Seen red 2026-10-12 with `KEPT_FILES` taking every added file of a kept item (the `added_at <=
/// kept_at` rule left out): "a file added after the keep counts against her share / left: 0 /
/// right: 4194304".
#[test]
fn a_kept_item_keeps_the_files_it_had_when_kept() {
    let db = db();
    upload(&db, "ann", "1_main.png", MEDIA_SHELF, MIB);
    upload(&db, "ann", "2_a.zip", MEDIA_SHELF, 2 * MIB);
    upload(&db, "ann", "3_later.zip", MEDIA_SHELF, 4 * MIB);
    let item = publish(&db, "ann", "1_main.png").unwrap();
    attach(&db, "ann", item.id, "2_a.zip", ROLE_FILE).unwrap();
    db.with_conn(|c| c.execute("UPDATE media_item_files SET added_at = 1000", [])).unwrap();
    db.set_media_kept(item.id, Some("mod"), 2000).unwrap();
    assert_eq!(db.media_bytes(Some("ann")), 4 * MIB, "only the upload still waiting is hers");
    assert_eq!(db.media_usage().unwrap().kept_bytes, 3 * MIB, "the item and its file are the server's");

    attach(&db, "ann", item.id, "3_later.zip", ROLE_FILE).unwrap();
    assert_eq!(db.media_bytes(Some("ann")), 4 * MIB, "a file added after the keep counts against her share");
    let usage = db.media_usage().unwrap();
    assert_eq!((usage.kept_bytes, usage.used_bytes), (3 * MIB, 7 * MIB));
    assert_eq!(usage.people.iter().find(|p| p.key == "ann").map(|p| p.bytes), Some(4 * MIB));

    db.set_media_kept(item.id, Some("mod"), now_ms() + 1000).unwrap();
    assert_eq!(db.media_bytes(Some("ann")), 0, "kept again, it is the server's too");
    assert_eq!(db.media_usage().unwrap().kept_bytes, 7 * MIB);
}

/// Every item of a sorted list, page by page (`per` to a page), following the cursor.
fn every_page(db: &Storage, sort: Sort, per: i64) -> Vec<i64> {
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..20 {
        let page = db.list_media(&FilesQuery { sort, cursor, limit: Some(per), ..Default::default() }).unwrap();
        seen.extend(page.iter().map(|i| i.id));
        if (page.len() as i64) < per {
            break;
        }
        let last = page.last().unwrap();
        cursor = Some((sort.value_of(last), last.id));
        assert_eq!(parse_cursor(&cursor_text(sort.value_of(last), last.id)), cursor, "the cursor reads back");
    }
    seen
}

/// THE LIST SORTS AND PAGES BY CURSOR WITHOUT SKIPPING TIES: seven items, two downloaded five times
/// and five downloaded three times, listed most downloaded first two to a page: each item once, in
/// order, the ties newest first; the same by picks; and newest first by the item's number. A cursor
/// this server would never give is not read.
///
/// Seen red 2026-10-12 with the cursor comparing the value alone (`{value} < ?5`): "every item once,
/// in order / left: [5, 2, 7, 6] / right: [5, 2, 7, 6, 4, 3, 1]".
#[test]
fn the_list_sorts_and_pages_by_cursor_without_skipping_ties() {
    let db = db();
    for n in 1..=7 {
        upload(&db, "ann", &format!("{n}_x.png"), MEDIA_SHELF, 10);
        publish(&db, "ann", &format!("{n}_x.png")).unwrap();
    }
    for (id, downloads) in [(1, 3), (2, 5), (3, 3), (4, 3), (5, 5), (6, 3), (7, 3)] {
        for _ in 0..downloads {
            assert!(db.add_media_download(id).unwrap());
        }
    }
    assert!(!db.add_media_download(99).unwrap(), "no such item");
    assert_eq!(every_page(&db, Sort::Downloads, 2), vec![5, 2, 7, 6, 4, 3, 1], "every item once, in order");
    for (picker, id) in [("bea", 3), ("cy", 3), ("bea", 6)] {
        db.pick_media(picker, id).unwrap();
    }
    assert_eq!(every_page(&db, Sort::Picks, 3), vec![3, 6, 7, 5, 4, 2, 1], "most picked first, then newest");
    assert_eq!(every_page(&db, Sort::New, 2), vec![7, 6, 5, 4, 3, 2, 1], "newest first");
    assert_eq!(ids_of(&db, &FilesQuery { sort: Sort::Downloads, cursor: Some((3, 4)), ..Default::default() }), vec![3, 1], "after a tie, the rest of it");
    for bad in ["", "3", "3.", ".4", "3.4.5", "-3.4", "3.x", "99999999999999999999.1"] {
        assert_eq!(parse_cursor(bad), None, "{bad:?} is not a cursor");
    }
}

/// FEATURED IN A CATEGORY SHOWS ON ITS SHELF ONLY (3.1 item 9): with no category the shelf is the
/// main one, its items featured "main"; with category c it is c's, its items featured "main" or
/// "category"; an item not featured is on no shelf, and every item stays in the list.
///
/// Seen red 2026-10-12 with the category clause dropped from the featured filter (`m.featured IN
/// (1, 2)` whatever the category): "the main shelf has the main item only / left: [3, 2, 1] /
/// right: [1]".
#[test]
fn featured_in_a_category_shows_on_its_shelf_only() {
    let db = db();
    let cats = db
        .replace_media_categories(&[
            CategoryEdit { id: Some(1), name: "General".into(), description: String::new() },
            CategoryEdit { id: None, name: "Fire flow arts".into(), description: "Staff, poi and fans".into() },
            CategoryEdit { id: None, name: "Water".into(), description: String::new() },
        ])
        .unwrap();
    let (fire, water) = (cats[1].id, cats[2].id);
    let item_in = |n: i64, category: i64, featured: Featured| {
        let file = format!("{n}_x.png");
        upload(&db, "ann", &file, MEDIA_SHELF, 10);
        let item = db.publish_media("ann", &Publish { filename: &file, category_id: Some(category), visibility: "public", ..Default::default() }, ROOMY, ROOMY, None).unwrap();
        db.set_media_featured(item.id, featured).unwrap();
        item.id
    };
    let main = item_in(1, fire, Featured::Main);
    let fire_only = item_in(2, fire, Featured::Category);
    let water_only = item_in(3, water, Featured::Category);
    let none = item_in(4, fire, Featured::None);

    let shelf = |category: Option<i64>| ids_of(&db, &FilesQuery { featured: true, category, ..Default::default() });
    assert_eq!(shelf(None), vec![main], "the main shelf has the main item only");
    assert_eq!(shelf(Some(fire)), vec![fire_only, main], "its category's shelf has both of its own");
    assert_eq!(shelf(Some(water)), vec![water_only]);
    assert_eq!(ids_of(&db, &FilesQuery::default()), vec![none, water_only, fire_only, main], "every item stays in the list");
    assert_eq!(db.media_item(fire_only).unwrap().unwrap().featured, Featured::Category);
    assert_eq!(why(db.set_media_featured(999, Featured::Main)).0, Refusal::NotFound);
}

/// AN EDIT KEEPS THE LICENCE TO THE UPLOADER AND LOGS IT (decisions 1 and 8): the publish's choice
/// is logged with its signature; an admin edits the words, the category and the labels, and is
/// refused a change of the licence or of the game tick; someone else is refused any edit; Ann's own
/// change of licence is logged with the edit's signature, its text and its time.
///
/// Seen red 2026-10-12 with the requester check taken out of `edit_media` (`licence_changes &&
/// uploader != requester`): "expected a refusal, got Ok(FilesItem { id: 1, upload_filename:
/// \"1_a.png\", ..." (the admin's change of licence went through).
#[test]
fn edit_media_keeps_the_licence_to_the_uploader_and_logs_it() {
    let db = db();
    upload(&db, "ann", "1_a.png", MEDIA_SHELF, 10);
    let signed = Signed { key: "ann", purpose: "media_publish:1_a.png:all_rights_reserved:0", body: "", ts: 5, sig: "sig-p" };
    let item = db
        .publish_media("ann", &Publish { filename: "1_a.png", visibility: "public", signed: Some(signed), ..Default::default() }, ROOMY, ROOMY, None)
        .unwrap();
    assert_eq!(item.licence, DEFAULT_LICENCE);
    let log = db.media_licence_log(item.id).unwrap();
    assert_eq!(log.len(), 1, "the publish's choice is logged");
    assert_eq!((log[0].licence.as_str(), log[0].signed_purpose.as_str(), log[0].signature.as_str(), log[0].signed_ts), (DEFAULT_LICENCE, signed.purpose, "sig-p", 5));

    let words = Edit { title: "Dusk", description: "First line\nThe rest", category_id: 1, labels: "flashing", licence: DEFAULT_LICENCE, project_use: false };
    let admin_sig = Signed { key: "op", purpose: "media_edit:1:aa", body: "{}", ts: 6, sig: "sig-op" };
    let edited = db.edit_media("op", true, item.id, &words, &admin_sig).unwrap();
    assert_eq!((edited.title.as_str(), edited.description.as_str(), edited.labels.clone()), ("Dusk", "First line\nThe rest", vec!["flashing".to_string()]));
    assert_eq!(db.media_licence_log(item.id).unwrap().len(), 1, "words alone are not a licence choice");

    let (w, refusal) = why(db.edit_media("op", true, item.id, &Edit { licence: "cc_by_4", ..words }, &admin_sig));
    assert_eq!(w, Refusal::NotAllowed);
    assert!(refusal.contains("change its licence"), "{refusal}");
    assert_eq!(why(db.edit_media("op", true, item.id, &Edit { project_use: true, ..words }, &admin_sig)).0, Refusal::NotAllowed, "nor the game tick");
    assert_eq!(why(db.edit_media("bea", false, item.id, &words, &admin_sig)).0, Refusal::NotAllowed, "someone else edits nothing");
    assert_eq!(why(db.edit_media("ann", false, item.id, &Edit { category_id: 77, ..words }, &admin_sig)).0, Refusal::Bad, "no such category");
    assert_eq!(why(db.edit_media("ann", false, 999, &words, &admin_sig)).0, Refusal::NotFound);

    let ann_sig = Signed { key: "ann", purpose: "media_edit:1:bb", body: "{\"licence\":\"cc_by_4\"}", ts: 7, sig: "sig-ann" };
    let chosen = db.edit_media("ann", false, item.id, &Edit { licence: "cc_by_4", project_use: true, ..words }, &ann_sig).unwrap();
    assert_eq!((chosen.licence.as_str(), chosen.project_use), ("cc_by_4", true));
    let log = db.media_licence_log(item.id).unwrap();
    assert_eq!(log.len(), 2, "her change is logged");
    let last = &log[1];
    assert_eq!(
        (last.licence.as_str(), last.project_use, last.set_by.as_str(), last.signed_purpose.as_str(), last.signed_body.as_str(), last.signed_ts, last.signature.as_str()),
        ("cc_by_4", true, "ann", "media_edit:1:bb", "{\"licence\":\"cc_by_4\"}", 7, "sig-ann")
    );
}

/// ATTACHING AND DETACHING: only the uploader adds, only their own upload waiting on the Media
/// shelf, a cover only a picture and a new cover replacing the old (whose rows go); at most twenty
/// files; file 0 is never taken off; a file is taken off by its number and its own name, by the
/// uploader or an admin, and its rows go.
///
/// Seen red 2026-10-12 with the owner check on the upload taken out of `attach_media_file`
/// (`owner == signer`): "someone else's upload / left: Conflict / right: NotFound" (the upload then
/// passed and was refused later, for another reason).
#[test]
fn files_are_added_and_taken_off_by_the_rules() {
    let db = db();
    upload(&db, "ann", "1_main.png", MEDIA_SHELF, 10);
    upload(&db, "bea", "2_bea.zip", MEDIA_SHELF, 10);
    upload(&db, "ann", "3_chat.zip", 0, 10);
    upload(&db, "ann", "4_c.zip", MEDIA_SHELF, 10);
    upload(&db, "ann", "5_c1.png", MEDIA_SHELF, 10);
    upload(&db, "ann", "6_c2.webp", MEDIA_SHELF, 10);
    let item = publish(&db, "ann", "1_main.png").unwrap();
    db.publish_media("bea", &Publish { filename: "2_bea.zip", visibility: "public", ..Default::default() }, ROOMY, ROOMY, None).ok();

    assert_eq!(why(attach(&db, "bea", item.id, "4_c.zip", ROLE_FILE)).0, Refusal::NotAllowed, "only the uploader adds");
    assert_eq!(why(attach(&db, "ann", item.id, "2_bea.zip", ROLE_FILE)).0, Refusal::NotFound, "someone else's upload");
    assert_eq!(why(attach(&db, "ann", item.id, "3_chat.zip", ROLE_FILE)).0, Refusal::NotFound, "a chat upload is not waiting on the Media shelf");
    assert_eq!(why(attach(&db, "ann", item.id, "4_c.zip", ROLE_COVER)), (Refusal::Bad, "A cover must be a picture: PNG, JPEG, GIF or WebP.".to_string()));
    assert_eq!(why(attach(&db, "ann", 999, "4_c.zip", ROLE_FILE)).0, Refusal::NotFound);
    assert_eq!(why(attach(&db, "ann", item.id, "4_c.zip", "poster")).0, Refusal::Bad);

    let first = attach(&db, "ann", item.id, "5_c1.png", ROLE_COVER).unwrap();
    assert_eq!(first.replaced, None);
    let second = attach(&db, "ann", item.id, "6_c2.webp", ROLE_COVER).unwrap();
    assert_eq!(second.replaced.as_deref(), Some("5_c1.png"), "a new cover replaces the old");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM user_uploads WHERE filename = '5_c1.png'"), 0, "and the old one's rows go");
    assert_eq!(second.item.cover.as_ref().map(|c| c.kind.as_str()), Some("picture"));

    let added = attach(&db, "ann", item.id, "4_c.zip", ROLE_FILE).unwrap().item;
    let file_no = added.files[0].file;
    for n in 0..18 {
        let name = format!("{}_f.bin", 10 + n);
        upload(&db, "ann", &name, MEDIA_SHELF, 1);
        attach(&db, "ann", item.id, &name, ROLE_FILE).unwrap();
    }
    upload(&db, "ann", "40_over.bin", MEDIA_SHELF, 1);
    assert_eq!(why(attach(&db, "ann", item.id, "40_over.bin", ROLE_FILE)), (Refusal::Bad, "An item can hold at most 20 files.".to_string()));
    assert_eq!(db.media_item(item.id).unwrap().unwrap().file_count, 20);

    assert_eq!(why(db.detach_media_file("ann", false, item.id, 0, "1_main.png")).0, Refusal::Bad, "file 0 is the item");
    assert_eq!(why(db.detach_media_file("bea", false, item.id, file_no, "4_c.zip")).0, Refusal::NotAllowed);
    assert_eq!(why(db.detach_media_file("ann", false, item.id, file_no, "10_f.bin")), (Refusal::Conflict, OUT_OF_DATE.to_string()));
    assert_eq!(why(db.detach_media_file("ann", false, item.id, 9999, "4_c.zip")).0, Refusal::NotFound);
    let (after, name) = db.detach_media_file("op", true, item.id, file_no, "4_c.zip").unwrap();
    assert_eq!((name.as_str(), after.file_count), ("4_c.zip", 19), "an admin takes a file off");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM user_uploads WHERE filename = '4_c.zip'"), 0, "its rows go");
    assert!(db.add_media_view(item.id).unwrap() && !db.add_media_view(999).unwrap());
    assert_eq!(db.media_item(item.id).unwrap().unwrap().views, 1);
}
