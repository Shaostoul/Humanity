//! Copyright reports' storage (storage/takedowns.rs, 2026-10-13). Each test names the break it was
//! seen failing against ("Seen red"), with the words it failed with.

use super::*;

fn count(db: &Storage, sql: &str) -> i64 {
    db.with_conn(|c| c.query_row(sql, [], |r| r.get(0))).unwrap()
}

fn report(db: &Storage, item: i64, now: i64) -> i64 {
    db.takedown_add(
        &NewTakedown { item_id: item, item_title: "Dusk", item_public: true, name: "Rosa", contact: "rosa@example.org", statement: "Mine." },
        now,
    )
    .unwrap()
}

/// BUG-046 FOR TAKEDOWNS: A DATABASE FROM BEFORE THE TABLE OPENS AND GAINS IT. The database is
/// today's schema with `takedown_requests` and its indexes dropped, which is exactly what the live
/// one holds before this version (every other table already there, a Media item in it).
/// `Storage::open` survives; the item is untouched; the table is there with its twelve columns and
/// both indexes; a report can be kept and read back; and a second open changes nothing.
///
/// Seen red 2026-10-13 with the takedown table's batch taken out of storage/mod.rs: "the open
/// gave the table its columns / left: 0 / right: 12".
#[test]
fn a_database_from_before_takedowns_opens_and_gains_the_table() {
    let path = crate::test_temp::db("takedowns_pre");
    {
        let db = Storage::open(&path).expect("a fresh database");
        db.with_conn(|c| {
            c.execute_batch(
                "INSERT INTO media_items (upload_filename, uploader_key, title, category_id, kind, size_bytes, created_at, from_shelf)
                     VALUES ('1760_x.png', 'ann', 'Dusk', 1, 'picture', 1234, 1760000001000, 2);
                 DROP INDEX IF EXISTS idx_takedown_requests_state;
                 DROP INDEX IF EXISTS idx_takedown_requests_decided;
                 DROP TABLE IF EXISTS takedown_requests;",
            )
        })
        .expect("shaped like the database before takedowns");
        assert_eq!(count(&db, "SELECT COUNT(*) FROM sqlite_master WHERE name = 'takedown_requests'"), 0, "test setup: no table");
    }

    let db = Storage::open(&path).expect("startup must survive a database from before takedowns");
    assert_eq!(db.media_item(1).unwrap().map(|i| i.title), Some("Dusk".to_string()), "the item is untouched");
    let columns = |db: &Storage| count(db, "SELECT COUNT(*) FROM pragma_table_info('takedown_requests')");
    assert_eq!(columns(&db), 12, "the open gave the table its columns");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND tbl_name = 'takedown_requests' AND name LIKE 'idx_%'"), 2);
    let id = db
        .takedown_add(&NewTakedown { item_id: 1, item_title: "Dusk", item_public: true, name: "", contact: "", statement: "Mine." }, 5)
        .expect("a report is kept in the migrated database");
    let row = db.takedown_by_id(id).unwrap().unwrap();
    assert_eq!((row.item_id, row.state.as_str(), row.decided_at), (1, "open", None));
    drop(db);

    let again = Storage::open(&path).expect("a second open");
    assert_eq!(columns(&again), 12, "a second open adds nothing");
    assert_eq!(again.takedown_by_id(id).unwrap(), Some(row), "and changes nothing");
}

/// THE EXPIRY PASS BLANKS WHAT A PERSON WROTE 90 DAYS AFTER THE DECISION, AND KEEPS THE ROW: an
/// open report keeps its words however old; one decided 89 days ago keeps them; one decided 91 days
/// ago has its name, contact and statement blanked by the maintenance pass while its item, title,
/// outcome and note stay for the public log.
///
/// Seen red 2026-10-13 with the takedown sweep taken out of `run_expiry_sweeps`: "the maintenance
/// pass left a person's words 91 days after the decision / left: \"Rosa\" / right: \"\"".
#[test]
fn the_expiry_pass_blanks_personal_words_ninety_days_after_a_decision() {
    let db = Storage::open_temp("takedowns_expiry");
    let now = crate::relay::storage::now_millis() as i64;
    let day = 86_400_000;
    let open = report(&db, 1, now - 400 * day);
    let recent = report(&db, 2, now - 100 * day);
    let old = report(&db, 3, now - 100 * day);
    assert!(db.takedown_decide(recent, "kept", "Theirs.", "admin", now - 89 * day).unwrap());
    assert!(db.takedown_decide(old, "removed", "Not theirs.", "admin", now - 91 * day).unwrap());
    assert!(!db.takedown_decide(old, "kept", "", "admin", now).unwrap(), "a report is decided once");

    db.run_expiry_sweeps();

    let row = |id| db.takedown_by_id(id).unwrap().unwrap();
    assert_eq!(row(old).name, "", "the maintenance pass left a person's words 91 days after the decision");
    assert_eq!((row(old).contact.as_str(), row(old).statement.as_str()), ("", ""));
    assert_eq!((row(old).item_title.as_str(), row(old).state.as_str(), row(old).decision_note.as_str()), ("Dusk", "removed", "Not theirs."));
    assert_eq!(row(recent).statement, "Mine.", "89 days after the decision the words stay");
    assert_eq!(row(open).name, "Rosa", "an open report keeps its words");
    assert_eq!(db.takedowns_forget_personal(now).unwrap(), 0, "a blanked row is not counted again");
    assert_eq!(db.takedowns_decided(10).unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), vec![recent, old], "the latest decision first");
    assert_eq!(db.takedowns_open().unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), vec![open]);
}
