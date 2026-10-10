//! Reports the admins can check (2026-10-09, docs/design/blocking-and-safe-mode.md 10e):
//! `reports_v2`, one row per report, written by handlers/reports.rs once it has checked the
//! report, and read by the admins' and moderators' review.
//!
//! A row holds who reported whom (both identity keys, never names), the context and reason, the
//! reporter's note, the evidence as stored (each item with `checked`: what the relay could prove
//! of it), the evidence exactly as the reporter signed it with its hash, the reporter's own time
//! and signature, and once decided: the decision, the reviewer's note, the reviewer and the
//! time.
//!
//! How long it is kept: an OPEN report until someone decides it; a decided one for
//! [`REPORT_KEEP_DAYS_AFTER_DECISION`] days after its decision (the operator's answer to
//! question 7, 2026-10-09), culled by the expiry pass (storage/expiry.rs). The evidence is
//! readable direct-message text a reporter handed over by choice, so it is not kept longer than
//! the decision needs.
//!
//! The reporter's account: the export lists the reports they filed (storage/account.rs); the
//! erase keeps each report for the admins but takes the reporter out of it ([`forget_reporter`]).
//! The reported person's account: their export does not list reports about them (the evidence of
//! a direct-message report names the reporter, who is never to be told to them), and their erase
//! leaves those reports, which exist to hold them to account, as a ban does.

use rusqlite::{params, Connection, OptionalExtension};

use super::Storage;

/// Days a decided report is kept after its decision (10a, question 7).
pub const REPORT_KEEP_DAYS_AFTER_DECISION: i64 = 90;

/// A report as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportRow {
    pub id: i64,
    /// Empty once the reporter erased their account.
    pub reporter_key: String,
    pub target_key: String,
    pub context: String,
    pub reason: String,
    pub note: String,
    /// The stored evidence, a JSON array, each item with `checked`.
    pub evidence: String,
    /// Unix ms.
    pub created_at: i64,
    /// "open" or "decided".
    pub state: String,
    pub decision: Option<String>,
    pub decision_note: String,
    pub decided_by: Option<String>,
    /// Unix ms.
    pub decided_at: Option<i64>,
}

/// A report the handler has checked, ready to keep.
pub struct NewReport<'a> {
    pub reporter_key: &'a str,
    pub target_key: &'a str,
    pub context: &'a str,
    pub reason: &'a str,
    pub note: &'a str,
    /// The stored evidence (each item with `checked`), a JSON array.
    pub evidence: &'a str,
    /// The `evidence` text exactly as the reporter sent and signed it.
    pub evidence_signed: &'a str,
    pub evidence_hash: &'a str,
    pub report_ts: u64,
    pub report_sig: &'a str,
}

const ROW_COLUMNS: &str =
    "id, reporter_key, target_key, context, reason, note, evidence, created_at, state, decision, decision_note, decided_by, decided_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ReportRow> {
    Ok(ReportRow {
        id: r.get(0)?,
        reporter_key: r.get(1)?,
        target_key: r.get(2)?,
        context: r.get(3)?,
        reason: r.get(4)?,
        note: r.get(5)?,
        evidence: r.get(6)?,
        created_at: r.get(7)?,
        state: r.get(8)?,
        decision: r.get(9)?,
        decision_note: r.get(10)?,
        decided_by: r.get(11)?,
        decided_at: r.get(12)?,
    })
}

impl Storage {
    /// Keep a checked report, open, made at `now_ms`. Returns its id.
    pub fn report_add(&self, r: &NewReport<'_>, now_ms: i64) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO reports_v2 (reporter_key, target_key, context, reason, note, evidence, evidence_signed,
                    evidence_hash, report_ts, report_sig, created_at, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'open')",
                params![
                    r.reporter_key,
                    r.target_key,
                    r.context,
                    r.reason,
                    r.note,
                    r.evidence,
                    r.evidence_signed,
                    r.evidence_hash,
                    r.report_ts as i64,
                    r.report_sig,
                    now_ms
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// How many reports `reporter` filed after `since_ms`.
    pub fn reports_filed_since(&self, reporter: &str, since_ms: i64) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM reports_v2 WHERE reporter_key = ?1 AND created_at > ?2",
                params![reporter, since_ms],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n as usize)
        })
    }

    /// Did `reporter` report `target` after `since_ms`?
    pub fn reported_since(&self, reporter: &str, target: &str, since_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM reports_v2 WHERE reporter_key = ?1 AND target_key = ?2 AND created_at > ?3)",
                params![reporter, target, since_ms],
                |r| r.get::<_, bool>(0),
            )
        })
    }

    /// The reports in `state` ("open" or "decided") that are not about `viewer`, newest first, at
    /// most `limit`. Left out in the query, not after it, so reports about a staff member cannot
    /// fill their list and hide the others (handlers/reports.rs: nobody sees a report about
    /// themselves).
    pub fn reports_in_state(&self, state: &str, viewer: &str, limit: usize) -> Result<Vec<ReportRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut st = conn.prepare(&format!(
                "SELECT {ROW_COLUMNS} FROM reports_v2 WHERE state = ?1 AND target_key != ?2 ORDER BY id DESC LIMIT ?3"
            ))?;
            let rows = st.query_map(params![state, viewer, limit as i64], row)?;
            rows.collect()
        })
    }

    /// The latest reports in any state that are not about `viewer`, newest first, at most `limit`
    /// (the `/reports` command).
    pub fn reports_latest(&self, viewer: &str, limit: usize) -> Result<Vec<ReportRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut st = conn.prepare(&format!("SELECT {ROW_COLUMNS} FROM reports_v2 WHERE target_key != ?1 ORDER BY id DESC LIMIT ?2"))?;
            let rows = st.query_map(params![viewer, limit as i64], row)?;
            rows.collect()
        })
    }

    /// Report `id`, if it is kept.
    pub fn report_by_id(&self, id: i64) -> Result<Option<ReportRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(&format!("SELECT {ROW_COLUMNS} FROM reports_v2 WHERE id = ?1"), params![id], row).optional()
        })
    }

    /// Record that `reviewer` decided open report `id` as `decision`, with `note`, at `now_ms`.
    /// False when it is not open (already decided, or gone), and then nothing changes.
    pub fn report_record_decision(&self, id: i64, reviewer: &str, decision: &str, note: &str, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE reports_v2 SET state = 'decided', decision = ?2, decision_note = ?3, decided_by = ?4, decided_at = ?5
                 WHERE id = ?1 AND state = 'open'",
                params![id, decision, note, reviewer, now_ms],
            )
            .map(|n| n > 0)
        })
    }

    /// The expiry pass's part: delete the reports decided more than
    /// [`REPORT_KEEP_DAYS_AFTER_DECISION`] days before `now_ms`. Open reports stay. Returns how
    /// many went.
    pub fn reports_expire(&self, now_ms: i64) -> Result<usize, rusqlite::Error> {
        let cutoff = now_ms - REPORT_KEEP_DAYS_AFTER_DECISION * 86_400_000;
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM reports_v2 WHERE state = 'decided' AND decided_at IS NOT NULL AND decided_at < ?1",
                params![cutoff],
            )
        })
    }

    /// `/reports-clear`: delete every decided report now. Open reports stay until someone decides
    /// them. Returns how many went.
    pub fn reports_clear_decided(&self) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("DELETE FROM reports_v2 WHERE state = 'decided'", []))
    }

    /// The public post `from` sent at `timestamp`, as this server has it: (text, channel).
    pub fn post_for_report(&self, from: &str, timestamp: u64) -> Result<Option<(String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COALESCE(content, ''), COALESCE(channel_id, '') FROM messages WHERE from_key = ?1 AND timestamp = ?2 LIMIT 1",
                params![from, timestamp as i64],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
        })
    }
}

/// The reporter's account erase (storage/account.rs `delete_account`): every report `key` filed
/// stays for the admins, with the reporter taken out of it. Blanked: the reporter's key, their
/// signature over the report and the evidence text they signed (a signature can be tried against
/// every member's key until one fits), and in each stored evidence item addressed to them (a
/// direct message's `to`) that `to` and the message's own signature (which names them the same
/// way). What the relay found when it checked the evidence (`checked`) stays. Returns how many
/// reports were changed.
pub(super) fn forget_reporter(conn: &Connection, key: &str) -> Result<usize, rusqlite::Error> {
    let filed: Vec<(i64, String)> = {
        let mut st = conn.prepare("SELECT id, evidence FROM reports_v2 WHERE reporter_key = ?1")?;
        let rows = st.query_map(params![key], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (id, evidence) in &filed {
        let mut items: Vec<serde_json::Value> = serde_json::from_str(evidence).unwrap_or_default();
        for item in items.iter_mut().filter(|i| i.get("to").and_then(|t| t.as_str()) == Some(key)) {
            item["to"] = serde_json::Value::String(String::new());
            if let Some(obj) = item.as_object_mut() {
                obj.remove("sig");
            }
        }
        conn.execute(
            "UPDATE reports_v2 SET reporter_key = '', report_sig = '', evidence_signed = '', evidence = ?2 WHERE id = ?1",
            params![id, serde_json::Value::Array(items).to_string()],
        )?;
    }
    Ok(filed.len())
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
