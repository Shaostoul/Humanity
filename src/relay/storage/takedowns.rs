//! Copyright reports about Media items (2026-10-13; the operator's answers of that day: "a simple
//! form so it's easy... a dedicated button somewhere on the media page... We can log the reports
//! publicly"): `takedown_requests`, one row per "Report a copyright problem" form, written by
//! relay/files_takedown.rs, read by the admins who decide it and, once decided, by the public
//! takedown log.
//!
//! WHO WROTE IT IS NOT KNOWN TO THE SERVER. The form needs no sign-in, so there is no key: only
//! what the person chose to type. `name` and `contact` are optional; `statement` (why the work is
//! theirs) is required. These three are PERSONAL: only admins ever see them, they are never in
//! the public log, and they are blanked [`TAKEDOWN_PERSONAL_KEEP_DAYS`] days after the decision
//! by the expiry pass (storage/expiry.rs, [`Storage::takedowns_forget_personal`]). The rest of the
//! row (which item, its title, when, the outcome and the admin's note) stays: it is the public
//! log, and it is kept for good.
//!
//! `item_title` is the item's title when the report came, so the log still names an item that was
//! removed. `item_public` says whether everyone could see that item then (a public item on a
//! public Media page); the public log shows the title only when they could, so a members-only
//! item's title never reaches the log. Neither is personal.
//!
//! `state` is "open", "removed" (an admin removed the item) or "kept" (an admin kept it). An admin
//! decides once; `decided_by` is that admin's key, which the public log never shows.

use rusqlite::{params, OptionalExtension};

use super::Storage;

/// Days after its decision that a copyright report keeps the name, contact and statement the
/// person gave. Then they are blanked; what the public log shows stays.
pub const TAKEDOWN_PERSONAL_KEEP_DAYS: i64 = 90;

/// One copyright report as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct TakedownRow {
    pub id: i64,
    pub item_id: i64,
    /// The item's title when the report came ('' when it had none).
    pub item_title: String,
    /// Whether everyone could see the item when the report came.
    pub item_public: bool,
    /// Unix ms.
    pub created_at: i64,
    /// Personal: blanked [`TAKEDOWN_PERSONAL_KEEP_DAYS`] after the decision.
    pub name: String,
    /// Personal: blanked [`TAKEDOWN_PERSONAL_KEEP_DAYS`] after the decision.
    pub contact: String,
    /// Personal: blanked [`TAKEDOWN_PERSONAL_KEEP_DAYS`] after the decision.
    pub statement: String,
    /// "open", "removed" or "kept".
    pub state: String,
    /// The deciding admin's note, which the public log shows.
    pub decision_note: String,
    /// The deciding admin's key (never in the public log).
    pub decided_by: Option<String>,
    /// Unix ms.
    pub decided_at: Option<i64>,
}

/// A copyright report the route has checked, ready to keep.
pub struct NewTakedown<'a> {
    pub item_id: i64,
    pub item_title: &'a str,
    pub item_public: bool,
    pub name: &'a str,
    pub contact: &'a str,
    pub statement: &'a str,
}

const ROW_COLUMNS: &str =
    "id, item_id, item_title, item_public, created_at, name, contact, statement, state, decision_note, decided_by, decided_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TakedownRow> {
    Ok(TakedownRow {
        id: r.get(0)?,
        item_id: r.get(1)?,
        item_title: r.get(2)?,
        item_public: r.get::<_, i64>(3)? != 0,
        created_at: r.get(4)?,
        name: r.get(5)?,
        contact: r.get(6)?,
        statement: r.get(7)?,
        state: r.get(8)?,
        decision_note: r.get(9)?,
        decided_by: r.get(10)?,
        decided_at: r.get(11)?,
    })
}

impl Storage {
    /// Keep a checked copyright report, open, made at `now_ms`. Returns its number.
    pub fn takedown_add(&self, t: &NewTakedown<'_>, now_ms: i64) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO takedown_requests (item_id, item_title, item_public, created_at, name, contact, statement, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open')",
                params![t.item_id, t.item_title, t.item_public as i64, now_ms, t.name, t.contact, t.statement],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Copyright report `id`, if there is one.
    pub fn takedown_by_id(&self, id: i64) -> Result<Option<TakedownRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(&format!("SELECT {ROW_COLUMNS} FROM takedown_requests WHERE id = ?1"), params![id], row).optional()
        })
    }

    /// The copyright reports nobody has decided yet, oldest first (the one waiting longest on top).
    pub fn takedowns_open(&self) -> Result<Vec<TakedownRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut st = conn.prepare(&format!("SELECT {ROW_COLUMNS} FROM takedown_requests WHERE state = 'open' ORDER BY id ASC"))?;
            let rows = st.query_map([], row)?;
            rows.collect()
        })
    }

    /// The decided copyright reports, the latest decision first, at most `limit`: the public log.
    pub fn takedowns_decided(&self, limit: usize) -> Result<Vec<TakedownRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut st = conn.prepare(&format!(
                "SELECT {ROW_COLUMNS} FROM takedown_requests WHERE state != 'open' ORDER BY decided_at DESC, id DESC LIMIT ?1"
            ))?;
            let rows = st.query_map(params![limit as i64], row)?;
            rows.collect()
        })
    }

    /// Record that admin `by` decided open copyright report `id` as `state` ("removed" or
    /// "kept"), with `note`, at `now_ms`. False when it is not open (decided already, or no such
    /// report), and then nothing changes.
    pub fn takedown_decide(&self, id: i64, state: &str, note: &str, by: &str, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE takedown_requests SET state = ?2, decision_note = ?3, decided_by = ?4, decided_at = ?5
                 WHERE id = ?1 AND state = 'open'",
                params![id, state, note, by, now_ms],
            )
            .map(|n| n > 0)
        })
    }

    /// The expiry pass's part: blank the name, contact and statement of every copyright report
    /// decided more than [`TAKEDOWN_PERSONAL_KEEP_DAYS`] days before `now_ms`. Open reports keep
    /// theirs (an admin still needs them); the row itself stays, for the public log. Returns how
    /// many rows were blanked (a row already blank is not counted again).
    pub fn takedowns_forget_personal(&self, now_ms: i64) -> Result<usize, rusqlite::Error> {
        let cutoff = now_ms - TAKEDOWN_PERSONAL_KEEP_DAYS * 86_400_000;
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE takedown_requests SET name = '', contact = '', statement = ''
                 WHERE state != 'open' AND decided_at IS NOT NULL AND decided_at < ?1
                   AND (name != '' OR contact != '' OR statement != '')",
                params![cutoff],
            )
        })
    }
}

#[cfg(test)]
#[path = "takedowns_tests.rs"]
mod tests;
