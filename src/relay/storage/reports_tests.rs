// Child of storage/reports.rs (#[path], so `use super::*` reaches everything it has): the
// `reports_v2` table on its own (2026-10-09, docs/design/blocking-and-safe-mode.md 10e, its Proof
// list): a database from before it, the 90-day cull, the export and the erase. The handler's
// checks are in handlers/reports_tests.rs.

use super::*;

/// A report from `reporter` about `target` with `evidence`, made at `created_at`.
fn file(db: &Storage, reporter: &str, target: &str, evidence: &str, created_at: i64) -> i64 {
    db.report_add(
        &NewReport {
            reporter_key: reporter,
            target_key: target,
            context: "dm",
            reason: "harassment",
            note: "it keeps happening",
            evidence,
            evidence_signed: evidence,
            evidence_hash: "00",
            report_ts: created_at as u64,
            report_sig: "c2ln",
        },
        created_at,
    )
    .unwrap()
}

fn decided_at(db: &Storage, id: i64, at: i64) {
    db.with_conn(|c| c.execute("UPDATE reports_v2 SET state = 'decided', decision = 'dismiss', decided_by = 'mod', decided_at = ?2 WHERE id = ?1", params![id, at]))
        .unwrap();
}

const DAY: i64 = 86_400_000;

/// The table is a plain `CREATE TABLE IF NOT EXISTS` in a batch of its own, so a database from
/// before it gains it on the next start (the BUG-046 shape: the live relay's database is never a
/// fresh one), with the columns the handler reads; and the old name-and-reason `reports` table,
/// which nothing reads any more, is dropped, while everything else stays.
///
/// Seen red 2026-10-09 with the reports_v2 batch in `Storage::open` creating a table of another
/// name: "a database from before step D has no reports_v2 table after a restart: no such table:
/// reports_v2".
#[test]
fn a_database_from_before_reports_v2_gains_it_and_loses_the_old_table() {
    let dir = crate::test_temp::dir("reports_premigration");
    let path = dir.join("relay.db");
    let db = Storage::open(&path).expect("open");
    db.with_conn(|c| {
        c.execute_batch(
            "DROP TABLE IF EXISTS reports_v2;
             CREATE TABLE reports (id INTEGER PRIMARY KEY AUTOINCREMENT, reporter_key TEXT NOT NULL,
                reported_name TEXT NOT NULL, reason TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL);
             INSERT INTO reports (reporter_key, reported_name, reason, created_at) VALUES ('k', 'Someone', 'spam', 1);",
        )
    })
    .unwrap();
    db.join_server("old_key", "Old").unwrap();
    drop(db);

    let reopened = Storage::open(&path).expect("a database from before step D opens");
    let cols: Result<Vec<String>, rusqlite::Error> = reopened.with_read_conn(|c| {
        let mut st = c.prepare("PRAGMA table_info(reports_v2)")?;
        let cols: Vec<String> = st.query_map([], |r| r.get::<_, String>(1))?.filter_map(|r| r.ok()).collect();
        if cols.is_empty() {
            c.prepare("SELECT id FROM reports_v2")?;
        }
        Ok(cols)
    });
    let cols = cols.unwrap_or_else(|e| panic!("a database from before step D has no reports_v2 table after a restart: {e}"));
    for want in ["reporter_key", "target_key", "context", "reason", "note", "evidence", "evidence_signed", "evidence_hash", "report_ts", "report_sig", "created_at", "state", "decision", "decision_note", "decided_by", "decided_at"] {
        assert!(cols.iter().any(|c| c == want), "reports_v2 has no {want}: {cols:?}");
    }
    let old_gone = reopened.with_read_conn(|c| c.prepare("SELECT 1 FROM reports LIMIT 0").map(|_| ())).is_err();
    assert!(old_gone, "the old name-and-reason reports table is still there");
    assert!(reopened.is_member("old_key"), "and what else it held is still there");
    let id = file(&reopened, "a", "b", "[]", 5);
    assert!(reopened.report_by_id(id).unwrap().is_some());
}

/// Kept, then culled: a report decided more than 90 days ago is deleted by the expiry pass the
/// relay runs at start, every six hours and after a settings change; one decided 89 days ago
/// stays, and an open report stays however old it is (nobody has decided it).
///
/// Seen red 2026-10-09 with the reports sweep taken out of `run_expiry_sweeps`: "the pass left a
/// report decided 91 days ago". And with `reports_expire` cutting on `created_at` alone, whatever
/// the state and the decision's date: "a report decided 89 days ago was culled".
#[test]
fn the_expiry_pass_culls_reports_decided_more_than_90_days_ago() {
    let db = Storage::open_temp_dir("reports_cull");
    let now = super::super::now_millis() as i64;
    let old = file(&db, "r1", "t", "[]", now - 200 * DAY);
    let recent = file(&db, "r2", "t", "[]", now - 200 * DAY);
    let open = file(&db, "r3", "t", "[]", now - 400 * DAY);
    decided_at(&db, old, now - 91 * DAY);
    decided_at(&db, recent, now - 89 * DAY);
    db.run_expiry_sweeps();
    assert!(db.report_by_id(old).unwrap().is_none(), "the pass left a report decided 91 days ago");
    assert!(db.report_by_id(recent).unwrap().is_some(), "a report decided 89 days ago was culled");
    assert!(db.report_by_id(open).unwrap().is_some(), "an open report was culled");
}

/// The reports a person filed are in their export, with the evidence they chose and what became
/// of each, but not who decided it or the reviewer's note. Their erase keeps every report for the
/// admins and takes the reporter out of it: the key, the signature over the report and the text
/// they signed, and in the evidence each direct message's `to` (them) and its signature; what the
/// relay found (`checked`) and the text stay. The reported person's export does not list it, and
/// their erase leaves it. Another reporter's report is untouched. A report the key files after
/// its erase (a device still signed in) counts as left over.
///
/// Seen red 2026-10-09 with the export's reports_v2 query finding nothing: "the export lists the
/// report they filed: []"; and with the `forget_reporter` call taken out of `delete_account`:
/// "the erase says so" (no reports_filed_unnamed in the receipt).
#[test]
fn the_export_lists_reports_filed_and_the_erase_takes_the_reporter_out() {
    let db = Storage::open_temp("reports_account");
    db.register_name("Rae", "rae_key").unwrap();
    db.register_name("Tom", "tom_key").unwrap();
    let evidence = serde_json::json!([
        { "kind": "dm", "from": "tom_key", "to": "rae_key", "ts": 7, "text": "go away", "sig": "c2ln", "checked": true },
        { "kind": "group_text", "from": "tom_key", "ts": 8, "text": "and you", "checked": false }
    ])
    .to_string();
    let id = file(&db, "rae_key", "tom_key", &evidence, 1_000);
    let other = file(&db, "ola_key", "tom_key", &evidence, 1_000);
    assert!(db.report_record_decision(id, "mod_key", "warn", "talked to him", 2_000).unwrap());

    let export = db.export_account("rae_key", "Rae");
    let filed = export["reports_filed"].as_array().cloned().unwrap_or_default();
    assert_eq!(filed.len(), 1, "the export lists the report they filed: {filed:?}");
    assert_eq!(filed[0]["target_key"], "tom_key");
    assert_eq!(filed[0]["decision"], "warn");
    assert!(filed[0]["evidence"].as_str().unwrap_or("").contains("go away"), "with the evidence they chose");
    assert!(filed[0].get("decided_by").is_none() && filed[0].get("decision_note").is_none(), "not the reviewer or their note");
    let tom_export = db.export_account("tom_key", "Tom");
    assert!(!tom_export.to_string().contains("go away"), "the reported person's export does not list it");

    let receipt = db.delete_account("rae_key", "Rae");
    assert!(receipt.iter().any(|(l, n)| l == "reports_filed_unnamed" && *n == 1), "the erase says so: {receipt:?}");
    let kept = db.report_by_id(id).unwrap().expect("the report stays for the admins");
    assert_eq!(kept.reporter_key, "", "the erase takes the reporter out of their report");
    assert_eq!(kept.decision.as_deref(), Some("warn"), "and the decision stays");
    let (sig, signed): (String, String) = db
        .with_conn(|c| c.query_row("SELECT report_sig, evidence_signed FROM reports_v2 WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))))
        .unwrap();
    assert!(sig.is_empty() && signed.is_empty(), "their signature and the text they signed are gone");
    let items: Vec<serde_json::Value> = serde_json::from_str(&kept.evidence).unwrap();
    assert_eq!(items[0]["to"], "", "the direct message no longer names them");
    assert!(items[0].get("sig").is_none(), "nor carries a signature that names them");
    assert_eq!(items[0]["checked"], true, "what the relay found stays");
    assert_eq!(items[0]["text"], "go away", "and so does the text");
    assert!(!db.erase_left_rows("rae_key"), "nothing of the account is left");
    assert_eq!(db.report_by_id(other).unwrap().unwrap().reporter_key, "ola_key", "another reporter's report is untouched");

    db.delete_account("tom_key", "Tom");
    assert!(db.report_by_id(id).unwrap().is_some(), "the reported person's erase leaves reports about them");

    file(&db, "rae_key", "tom_key", "[]", 3_000);
    assert!(db.erase_left_rows("rae_key"), "a report filed after the erase was not seen as left over");
}

/// The limits the handler reads, the decision recorded once only, and the post lookup.
///
/// Seen red 2026-10-09 with `report_record_decision`'s `AND state = 'open'` dropped: "a decided
/// report is decided again".
#[test]
fn counts_decisions_and_post_lookup() {
    let db = Storage::open_temp("reports_rows");
    file(&db, "r", "t1", "[]", 1_000);
    file(&db, "r", "t2", "[]", 2_000);
    assert_eq!(db.reports_filed_since("r", 500).unwrap(), 2);
    assert_eq!(db.reports_filed_since("r", 1_500).unwrap(), 1, "only those after the time");
    assert!(db.reported_since("r", "t1", 500).unwrap());
    assert!(!db.reported_since("r", "t1", 1_500).unwrap(), "t1 was reported before then");
    assert!(!db.reported_since("someone", "t1", 0).unwrap());

    let id = file(&db, "r", "t3", "[]", 3_000);
    assert!(db.report_record_decision(id, "admin", "mute", "n", 4_000).unwrap());
    assert!(!db.report_record_decision(id, "admin", "ban", "n", 5_000).unwrap(), "a decided report is decided again");
    let r = db.report_by_id(id).unwrap().unwrap();
    assert_eq!((r.state.as_str(), r.decision.as_deref(), r.decided_by.as_deref(), r.decided_at), ("decided", Some("mute"), Some("admin"), Some(4_000)));
    assert_eq!(db.reports_in_state("open", 10).unwrap().len(), 2);
    assert_eq!(db.reports_in_state("decided", 10).unwrap().len(), 1);
    assert_eq!(db.reports_clear_decided().unwrap(), 1);
    assert_eq!(db.reports_in_state("open", 10).unwrap().len(), 2, "clearing keeps the open ones");

    db.with_conn(|c| {
        c.execute(
            "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id) VALUES ('chat', 'poster', 'P', 'a post', 42, '{}', 'general')",
            [],
        )
    })
    .unwrap();
    assert_eq!(db.post_for_report("poster", 42).unwrap(), Some(("a post".to_string(), "general".to_string())));
    assert_eq!(db.post_for_report("poster", 43).unwrap(), None);
    assert_eq!(db.post_for_report("someone", 42).unwrap(), None);
}
