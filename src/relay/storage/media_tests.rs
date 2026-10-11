//! The Media page's storage (storage/media.rs). Each test names the break it was seen failing
//! against ("Seen red"), with the words it failed with.

use super::*;
use crate::relay::storage::Storage;

const MIB: i64 = 1024 * 1024;
/// A budget and share nothing in these tests comes near.
const ROOMY: i64 = 1 << 40;

fn db() -> Storage {
    Storage::open_temp("media")
}

/// An upload by `key` on `shelf` (0 chat, 1 shared, 2 Media) of `size` bytes, as the upload route
/// records it.
fn upload(db: &Storage, key: &str, file: &str, shelf: i64, size: i64) {
    db.record_upload(key, file, 100, shelf, file, size).unwrap();
}

fn publish(db: &Storage, key: &str, file: &str) -> Result<MediaItem, MediaError> {
    db.publish_media(key, &Publish { filename: file, visibility: "public", ..Default::default() }, ROOMY, ROOMY, None)
}

fn publish_in(db: &Storage, key: &str, file: &str, category: i64) -> MediaItem {
    db.publish_media(key, &Publish { filename: file, category_id: Some(category), visibility: "public", ..Default::default() }, ROOMY, ROOMY, None)
        .unwrap()
}

fn why(r: Result<impl std::fmt::Debug, MediaError>) -> (Refusal, String) {
    match r {
        Err(MediaError::Refused(w, words)) => (w, words),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn shelf_of(db: &Storage, file: &str) -> i64 {
    db.with_conn(|c| c.query_row("SELECT shared FROM user_uploads WHERE filename = ?1", [file], |r| r.get(0))).unwrap()
}

fn count(db: &Storage, sql: &str) -> i64 {
    db.with_conn(|c| c.query_row(sql, [], |r| r.get(0))).unwrap()
}

/// ONLY YOUR OWN, NEVER ENCRYPTED: someone else's upload, an encrypted one, a name that is not an
/// upload and a second publish of the same file are refused; your own chat picture is published,
/// moves off the keep-N FIFO (posting more photos never takes it) and stays out of the Shared
/// Files list.
///
/// Seen red 2026-10-11 with the owner check in `publish_media` taken out: "expected a refusal,
/// got Ok(MediaItem { id: 1, upload_filename: \"2_bea.png\", uploader_key: \"ann\", ...".
#[test]
fn publishing_takes_only_your_own_unencrypted_upload() {
    let db = db();
    upload(&db, "ann", "1_ann.png", 0, 10);
    upload(&db, "bea", "2_bea.png", 0, 10);
    upload(&db, "ann", "3_secret.enc", 0, 10);

    let (w, words) = why(publish(&db, "ann", "2_bea.png"));
    assert_eq!(w, Refusal::NotAllowed, "{words}");
    assert!(words.contains("your own uploads"), "{words}");
    let (w, words) = why(publish(&db, "ann", "3_secret.enc"));
    assert_eq!(w, Refusal::NotAllowed, "an encrypted file is never shown: {words}");
    assert!(words.contains("encrypted"), "{words}");
    assert_eq!(why(publish(&db, "ann", "9_ghost.png")).0, Refusal::NotFound);

    let item = publish(&db, "ann", "1_ann.png").expect("ann's own picture publishes");
    assert_eq!((item.kind.as_str(), item.visibility.as_str(), item.category_id), ("picture", "public", GENERAL_CATEGORY_ID));
    assert_eq!(item.category_name, "General");
    assert_eq!(why(publish(&db, "ann", "1_ann.png")).0, Refusal::Conflict, "published twice");

    assert_eq!(shelf_of(&db, "1_ann.png"), MEDIA_SHELF, "a published chat upload leaves the FIFO's shelf");
    let mut pruned = Vec::new();
    for i in 0..5 {
        pruned.extend(db.record_upload("ann", &format!("{}_chat.png", 10 + i), 2, false, "chat.png", 10).unwrap());
    }
    assert!(!pruned.contains(&"1_ann.png".to_string()), "the keep-N FIFO took a published file: {pruned:?}");
    assert!(db.list_shared_uploads(50, None).unwrap().is_empty(), "a Media item is not in the Shared Files list");
}

/// THE BUDGET AND THE SHARE: a file the Media space has no room for, and one the person's share
/// has no room for, are refused with what is used, the limit and the file's size in the words. A
/// Media upload not yet published counts, and publishing it does not count it twice.
///
/// Seen red 2026-10-11 with the budget check reading `used > budget` (the file left out):
/// "expected a refusal, got Ok(MediaItem { id: 2, upload_filename: \"3_c.png\", uploader_key:
/// \"bea\", ...".
#[test]
fn the_budget_and_the_share_refuse_with_both_numbers() {
    let db = db();
    let (budget, share) = (3 * MIB, 2 * MIB);
    let go = |key: &str, file: &str| db.publish_media(key, &Publish { filename: file, visibility: "public", ..Default::default() }, budget, share, None);
    upload(&db, "ann", "1_a.png", 0, 3 * MIB / 2);
    go("ann", "1_a.png").expect("1.5 MB fits");

    upload(&db, "ann", "2_b.png", 0, MIB);
    let (w, words) = why(go("ann", "2_b.png"));
    assert_eq!(w, Refusal::Full);
    assert!(words.contains("1.5 MB of your 2.0 MB used") && words.contains("this file is 1.0 MB"), "{words}");

    upload(&db, "bea", "3_c.png", 0, 2 * MIB);
    let (w, words) = why(go("bea", "3_c.png"));
    assert_eq!(w, Refusal::Full);
    assert!(words.contains("1.5 MB of 3.0 MB used") && words.contains("this file is 2.0 MB"), "{words}");

    // A Media upload counts from the moment it is stored...
    upload(&db, "bea", "4_d.png", MEDIA_SHELF, MIB);
    assert_eq!(db.media_bytes(None), 5 * MIB / 2);
    assert_eq!(db.media_bytes(Some("bea")), MIB);
    // ...and publishing it does not count it a second time (1.5 + 1 of 3 MB).
    go("bea", "4_d.png").expect("a counted Media upload publishes into its own room");
    assert_eq!(db.media_bytes(None), 5 * MIB / 2);
}

/// TAKING AN ITEM DOWN: someone else is refused; the uploader takes their own (the item and every
/// pick of it go; a chat upload goes back to chat, and the file name and where it came from come
/// back for the route); an admin takes anyone's, and an item uploaded for Media loses its row.
///
/// Seen red 2026-10-11 with the uploader-or-admin check taken out of `remove_media`: "expected a
/// refusal, got Ok(\"1_a.png\")".
#[test]
fn remove_is_the_uploaders_or_an_admins_and_takes_its_picks() {
    let db = db();
    upload(&db, "ann", "1_a.png", 0, 10);
    upload(&db, "bea", "2_b.png", MEDIA_SHELF, 10);
    let a = publish(&db, "ann", "1_a.png").unwrap();
    let b = publish(&db, "bea", "2_b.png").unwrap();
    db.pick_media("bea", a.id).unwrap();
    db.pick_media("cal", a.id).unwrap();

    let (w, _) = why(db.remove_media(a.id, "cal", false));
    assert_eq!(w, Refusal::NotAllowed, "cal is neither the uploader nor an admin");
    assert_eq!(db.remove_media(a.id, "ann", false).unwrap(), Removed { filename: "1_a.png".into(), from_shelf: 0 });
    assert!(db.media_item(a.id).unwrap().is_none(), "the item is gone");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM media_picks"), 0, "its picks went with it");
    assert_eq!(shelf_of(&db, "1_a.png"), 0, "its chat upload went back to chat");
    assert_eq!(why(db.remove_media(a.id, "ann", false)).0, Refusal::NotFound);

    let gone = db.remove_media(b.id, "admin", true).unwrap();
    assert_eq!((gone.filename.as_str(), gone.keeps_file()), ("2_b.png", false), "an admin takes anyone's down");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM user_uploads WHERE filename = '2_b.png'"), 0, "an upload made for Media loses its row");
}

/// PICKS: a pick counts once however often it is sent, picks are for other people's items, a
/// person's picks list them, and an unpick takes one away.
///
/// Seen red 2026-10-11 with the own-item refusal taken out of `pick_media`: "expected a refusal,
/// got Ok(3)".
#[test]
fn picks_count_once_and_are_for_other_peoples_items() {
    let db = db();
    upload(&db, "ann", "1_a.png", 0, 10);
    let item = publish(&db, "ann", "1_a.png").unwrap();
    assert_eq!(db.pick_media("bea", item.id).unwrap(), 1);
    assert_eq!(db.pick_media("bea", item.id).unwrap(), 1, "picking twice is one pick");
    assert_eq!(db.pick_media("cal", item.id).unwrap(), 2);
    let (w, words) = why(db.pick_media("ann", item.id));
    assert_eq!(w, Refusal::Bad, "{words}");
    assert_eq!(why(db.pick_media("bea", 999)).0, Refusal::NotFound);

    let beas = db.list_media(&MediaQuery { picked_by: Some("bea".into()), ..Default::default() }).unwrap();
    assert_eq!(beas.iter().map(|i| i.id).collect::<Vec<_>>(), vec![item.id], "bea's picks list the item");
    assert_eq!(beas[0].picks, 2, "the list carries the pick count");
    assert_eq!(db.unpick_media("bea", item.id).unwrap(), 1);
    assert!(db.list_media(&MediaQuery { picked_by: Some("bea".into()), ..Default::default() }).unwrap().is_empty());
}

/// THE LIST: newest first, paged by `before`, filtered by category, kind, person and a search
/// over titles and descriptions (a `%` matches only itself); featuring shows in it.
///
/// Seen red 2026-10-11 with the list ordered `m.id ASC`: "assertion `left == right` failed:
/// newest first / left: [1, 2, 3] / right: [3, 2, 1]".
#[test]
fn the_list_is_newest_first_paged_and_filtered() {
    let db = db();
    upload(&db, "ann", "1_a.png", 0, 10);
    upload(&db, "ann", "2_b.mp4", 0, 10);
    upload(&db, "bea", "3_c.glb", 0, 10);
    let a = db
        .publish_media("ann", &Publish { filename: "1_a.png", title: "Contact staff at dusk", visibility: "public", ..Default::default() }, ROOMY, ROOMY, None)
        .unwrap();
    let b = db
        .publish_media("ann", &Publish { filename: "2_b.mp4", description: "100% fire", visibility: "public", ..Default::default() }, ROOMY, ROOMY, None)
        .unwrap();
    let c = publish(&db, "bea", "3_c.glb").unwrap();
    let ids = |q: MediaQuery| db.list_media(&q).unwrap().iter().map(|i| i.id).collect::<Vec<_>>();

    assert_eq!(ids(MediaQuery::default()), vec![c.id, b.id, a.id], "newest first");
    assert_eq!(ids(MediaQuery { limit: Some(2), ..Default::default() }), vec![c.id, b.id]);
    assert_eq!(ids(MediaQuery { before: Some(b.id), ..Default::default() }), vec![a.id], "the next page");
    assert_eq!(ids(MediaQuery { kind: Some("model".into()), ..Default::default() }), vec![c.id]);
    assert_eq!((a.kind.as_str(), b.kind.as_str(), c.kind.as_str()), ("picture", "video", "model"));
    assert_eq!(ids(MediaQuery { by: Some("ann".into()), ..Default::default() }), vec![b.id, a.id]);
    assert_eq!(ids(MediaQuery { q: Some("STAFF".into()), ..Default::default() }), vec![a.id], "the search ignores case");
    assert_eq!(ids(MediaQuery { q: Some("100%".into()), ..Default::default() }), vec![b.id], "a % is matched as itself");
    assert_eq!(ids(MediaQuery { category: Some(GENERAL_CATEGORY_ID), ..Default::default() }).len(), 3);
    assert_eq!(ids(MediaQuery { category: Some(99), ..Default::default() }), Vec::<i64>::new());

    db.set_media_featured(b.id, true).unwrap();
    assert!(db.media_item(b.id).unwrap().unwrap().featured);
    assert_eq!(why(db.set_media_featured(999, true)).0, Refusal::NotFound);
}

/// CATEGORY SUGGESTIONS: a member's suggestion waits for an admin; one that is already a category
/// or already waiting is refused, and so is a sixth waiting at once; accepting adds the category
/// at the end of the list, declining does not, and a decided one cannot be decided again.
///
/// Seen red 2026-10-11 with the category insert taken out of `decide_media_suggestion`:
/// "assertion `left == right` failed / left: None / right: Some(\"Fire flow arts\")".
#[test]
fn suggestions_wait_for_an_admin_and_accepting_adds_the_category() {
    let db = db();
    let fire = db.suggest_media_category("ann", "Fire flow arts", "contact staff, poi").unwrap();
    assert_eq!((fire.state.as_str(), fire.note.as_str()), ("open", "contact staff, poi"));
    assert_eq!(why(db.suggest_media_category("ann", "general", "")).0, Refusal::Conflict, "General exists");
    assert_eq!(why(db.suggest_media_category("bea", "fire FLOW arts", "")).0, Refusal::Conflict, "already waiting");
    for n in 0..4 {
        db.suggest_media_category("ann", &format!("Topic {n}"), "").unwrap();
    }
    let (w, words) = why(db.suggest_media_category("ann", "Topic 5", ""));
    assert_eq!(w, Refusal::Full, "{words}");
    let builds = db.suggest_media_category("bea", "Builds", "").unwrap();

    let (decided, category) = db.decide_media_suggestion(fire.id, true).unwrap();
    assert_eq!(decided.state, "accepted");
    assert_eq!(category.map(|c| c.name), Some("Fire flow arts".to_string()));
    let names: Vec<String> = db.media_categories().unwrap().into_iter().map(|c| c.name).collect();
    assert_eq!(names, vec!["General".to_string(), "Fire flow arts".to_string()], "accepting adds the category");
    assert_eq!(why(db.decide_media_suggestion(fire.id, false)).0, Refusal::Conflict, "decided twice");

    let (declined, none) = db.decide_media_suggestion(builds.id, false).unwrap();
    assert_eq!((declined.state.as_str(), none), ("declined", None));
    assert_eq!(db.media_categories().unwrap().len(), 2, "declining adds nothing");
    let beas = db.media_suggestions(Some("bea")).unwrap();
    assert_eq!(beas.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Builds"], "a person's own suggestions only");
    assert_eq!(db.media_suggestions(None).unwrap().len(), 6);
}

/// THE CATEGORY LIST, REPLACED WHOLE: added, renamed and reordered by the list; a category left
/// out is removed and its items move to General; General cannot be left out, and an unknown id
/// or two of one name refuse the whole list and change nothing.
///
/// Seen red 2026-10-11 with the move to General taken out of `replace_media_categories`: "a
/// removed category's items move to General / left: 3 / right: 1".
#[test]
fn replacing_the_categories_moves_a_removed_ones_items_to_general() {
    let db = db();
    let edit = |id: Option<i64>, name: &str| CategoryEdit { id, name: name.into(), description: String::new() };
    let list = db.replace_media_categories(&[edit(Some(1), "General"), edit(None, "Fire flow arts"), edit(None, "Builds")]).unwrap();
    assert_eq!(list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["General", "Fire flow arts", "Builds"]);
    let (fire, builds) = (list[1].id, list[2].id);
    upload(&db, "ann", "1_a.png", 0, 10);
    let item = publish_in(&db, "ann", "1_a.png", builds);

    let list = db.replace_media_categories(&[edit(Some(builds), "Homestead builds"), edit(Some(1), "General"), edit(Some(fire), "Fire flow arts")]).unwrap();
    assert_eq!(list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Homestead builds", "General", "Fire flow arts"], "renamed and reordered");

    db.replace_media_categories(&[edit(Some(1), "General"), edit(Some(fire), "Fire flow arts")]).unwrap();
    assert_eq!(db.media_item(item.id).unwrap().unwrap().category_id, GENERAL_CATEGORY_ID, "a removed category's items move to General");

    let before = db.media_categories().unwrap();
    let (w, words) = why(db.replace_media_categories(&[edit(Some(fire), "Fire flow arts")]));
    assert!(w == Refusal::Bad && words.contains("General"), "{words}");
    assert_eq!(why(db.replace_media_categories(&[edit(Some(1), "General"), edit(Some(99), "Ghost")])).0, Refusal::Bad);
    assert_eq!(why(db.replace_media_categories(&[edit(Some(1), "General"), edit(None, "general")])).0, Refusal::Bad);
    assert_eq!(db.media_categories().unwrap(), before, "a refused list changed nothing");
}

/// THE USAGE NUMBERS: bytes used (published items and Media uploads not yet published), each
/// person's use largest first, and the largest, oldest and least viewed items.
///
/// Seen red 2026-10-11 with the largest list ordered `m.size_bytes ASC`: "largest first / left:
/// [\"2_b.png\", \"3_c.png\", \"1_a.png\"] / right: [\"1_a.png\", \"3_c.png\", \"2_b.png\"]".
#[test]
fn the_usage_numbers_add_up() {
    let db = db();
    upload(&db, "ann", "1_a.png", 0, 3 * MIB);
    upload(&db, "ann", "2_b.png", 0, MIB);
    upload(&db, "bea", "3_c.png", 0, 2 * MIB);
    upload(&db, "bea", "4_pending.png", MEDIA_SHELF, MIB / 2);
    upload(&db, "cal", "5_chat.png", 0, 7 * MIB); // chat media: not the Media space's
    for (k, f) in [("ann", "1_a.png"), ("ann", "2_b.png"), ("bea", "3_c.png")] {
        publish(&db, k, f).unwrap();
    }
    db.with_conn(|c| c.execute("UPDATE media_items SET views = 9 WHERE upload_filename != '2_b.png'", [])).unwrap();

    let u = db.media_usage().unwrap();
    assert_eq!(u.used_bytes, 13 * MIB / 2, "3 + 1 + 2 published and 0.5 waiting");
    assert_eq!(u.items, 3);
    let people: Vec<(String, i64, i64)> = u.people.iter().map(|p| (p.key.clone(), p.bytes, p.items)).collect();
    assert_eq!(people, vec![("ann".to_string(), 4 * MIB, 2), ("bea".to_string(), 5 * MIB / 2, 1)]);
    let files = |l: &[MediaItem]| l.iter().map(|i| i.upload_filename.clone()).collect::<Vec<_>>();
    assert_eq!(files(&u.largest), vec!["1_a.png", "3_c.png", "2_b.png"], "largest first");
    assert_eq!(files(&u.oldest)[0], "1_a.png", "oldest first");
    assert_eq!(files(&u.least_viewed)[0], "2_b.png", "least viewed first");
}

/// A SHARED FILE TAKEN OFF THE SHARED FILES LIST (`POST /api/uploads/delete`) leaves the Media
/// page too, picks and all: its file is deleted, so the item would point at nothing.
///
/// Seen red 2026-10-11 with the two media deletes taken out of `delete_shared_upload`: "the item
/// left with its file / left: Some(MediaItem { id: 1, upload_filename: \"1_case.blend\", ...".
#[test]
fn a_shared_file_deleted_from_the_library_leaves_the_media_page() {
    let db = db();
    upload(&db, "ann", "1_case.blend", 1, 10);
    let item = publish(&db, "ann", "1_case.blend").unwrap();
    assert_eq!(item.kind, "model");
    assert_eq!(shelf_of(&db, "1_case.blend"), 1, "a shared file stays in the Shared Files list when published");
    db.pick_media("bea", item.id).unwrap();
    assert_eq!(db.delete_shared_upload("1_case.blend", "ann", false).unwrap(), Some("1_case.blend".to_string()));
    assert_eq!(db.media_item(item.id).unwrap(), None, "the item left with its file");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM media_picks"), 0);
}
