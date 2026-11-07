//! DM mailbox — sealed-sender store-and-forward (v2, 2026-08-23).
//!
//! REPLACES the old `direct_messages` table, which stored
//! `from_key, from_name, to_key, timestamp` in the clear next to the
//! ciphertext and therefore accumulated a complete, subpoena-exportable
//! who-talked-to-whom graph (the kind of dataset the
//! 2026 Take-Two subpoena to Discord asked for, docs/reference/findings/
//! 2026-11-07-take-two-discord-subpoena.md). The migration in
//! `storage/mod.rs` DROPS that table outright (secure_delete=ON zeroes
//! the freed pages; a wal_checkpoint(TRUNCATE) folds them out of the WAL).
//!
//! The v2 model:
//!   - A row is (id, to_key, content, received_day). NO sender column
//!     exists — the sender's identity travels INSIDE the sealed envelope
//!     (`net::dm_pq` v2), Dilithium-signed so the recipient can verify it.
//!   - One DM = two independent rows: the recipient's copy (sealed to
//!     their Kyber key, addressed to them) and the sender's self-copy
//!     (sealed to their own key, addressed to themselves) so the
//!     sender's other devices can fetch sent history.
//!   - `received_day` is DAY-granularity (unix days) on purpose: it
//!     exists only so mail can expire, and coarse timestamps leak less.
//!     Display ordering comes from the signed `ts` inside the envelope.
//!   - Mail EXPIRES after `dm_mailbox_ttl_days` (server_settings, default
//!     30). Clients keep long-term history locally (encrypted at rest);
//!     the server holds only the recent delivery window. `mailbox_purge`
//!     lets a user delete their own queue immediately.
//!
//! ROWS ARE KEPT BY MAILBOX, NOT BY ARRIVAL (2026-11-06, docs/BUGS.md BUG-259). Until then the
//! table was `id INTEGER PRIMARY KEY AUTOINCREMENT`: rows numbered, and stored, in arrival order.
//! Both apps sent a DM's recipient copy and the sender's own copy one right after the other,
//! padded to the same size, so a copy of the database (a court order, a breach, a backup with its
//! key) showed neighbouring rows, one addressed to the recipient and one to the sender, pairing
//! who wrote to whom day by day across the whole window: the very graph this table exists not to
//! hold. Now:
//!   - The table is WITHOUT ROWID with PRIMARY KEY (to_key, id): the file keeps rows sorted by
//!     mailbox, and there is no hidden rowid counting arrivals.
//!   - `id` is per mailbox: the day shifted up by [`DAY_SHIFT`], plus the row's place among that
//!     mailbox's own mail that day (`mailbox_put`). Every mailbox's first envelope of a day has the
//!     same number, so the numbers say nothing about the order of mail across mailboxes, and
//!     nothing finer than the day. A device pages its own mailbox exactly as before ("everything
//!     after the last number I saw"), because a mailbox's numbers only ever rise.
//!   - The only time kept is still the day. It cannot be coarser without breaking the expiry the
//!     operator chose (1 to 365 days, kept to the day); a day mixes all of a day's mail together.
//!   - The apps send the sender's own copy apart from the recipient's, a random 20 s to 2 min
//!     later (`src/net/self_mail.rs`, `web/shared/self-mail.js`), so the two do not land next to
//!     each other in the write-ahead log or the continuous backup's segments either.
//!   - Clearing a mailbox (`mailbox_purge`) leaves one empty marker row holding its last number,
//!     so the next envelope still gets a higher number than any device has seen; the marker is
//!     never handed out as mail and ages out with the window.
//!
//! What a subpoena of this table yields: pseudonymous keys, each with N undecryptable blobs of a
//! few padded sizes, the day each arrived and its place in that key's own mail that day. No
//! sender, no names, no content, and no order across mailboxes to pair a message with its sender's
//! own copy. What is still true, said plainly in docs/outreach/how_your_privacy_is_built.md: the
//! server knows the sender at the moment of sending (the put arrives on their signed-in
//! connection); the write-ahead log and the continuous backup keep the order pages were written
//! in for as long as they keep them, so on a server so quiet that nothing else is written in the
//! minutes between a message and its sender's copy, someone holding them could still guess the
//! pair; and backups taken before the fix keep the old order for as long as they are kept. The rows
//! stored in arrival order were deleted by the upgrade after it (2026-11-07, BUG-262: the operator,
//! "Just drop them now."), fetched or not ([`OLD_NUMBER_CEILING`]). Ending the pairing for good is
//! planned in docs/design/unpairable-dms.md.

use super::Storage;
use rusqlite::params;

/// Seconds per day — `received_day` is `unix_seconds / 86400`.
const SECS_PER_DAY: u64 = 86_400;

/// A mailbox row's number is its day shifted up this far, plus its place among that mailbox's own
/// mail (see `mailbox_put`).
pub const DAY_SHIFT: u32 = 20;

/// EVERY NUMBER BELOW THIS IS AN OLD ARRIVAL-ORDER ONE, AND NO OTHER IS (2026-11-07, BUG-262). The
/// mailbox numbered rows 1, 2, 3... in arrival order from 2026-08-23 until the per-mailbox numbers
/// went live on 2026-11-07; the rebuild kept those numbers, and the upgrade now deletes every row
/// below this (storage/mod.rs). Why the line cannot cut a new row: [`next_number`] never gives less
/// than the day it runs shifted up by [`DAY_SHIFT`] (a mailbox's highest plus one, or more), and any
/// day after 1981-03-20 (unix day 4096) shifted up by 20 is at least 2^32, so only a server whose
/// clock read 1981 could make one below it. Why it catches every old one: an old number counted the
/// envelopes ever stored, one each, and passing 2^32 (4.29 billion) would have taken a single server
/// over 600 envelopes a second for the 75 days the old table was live.
pub const OLD_NUMBER_CEILING: i64 = 1 << 32;
const _: () = assert!(4096i64 << DAY_SHIFT == OLD_NUMBER_CEILING, "the ceiling is unix day 4096's first number");

/// Current unix day (UTC).
pub fn unix_day_now() -> i64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / SECS_PER_DAY) as i64
}

/// One fetched mailbox row: its number in its mailbox (for pagination) + the opaque envelope.
#[derive(Debug, Clone)]
pub struct DmMailRow {
    pub id: i64,
    pub content: String,
}

/// The next number in `to_key`'s mailbox on `day`: one past the mailbox's highest (its marker
/// included, after a clear), and never below the day's own base, `day << DAY_SHIFT`. So the first
/// envelope of a day in every mailbox gets the same number, and a mailbox's numbers only rise
/// (a day holds 2^20 numbers; a mailbox past that simply carries on from its highest).
fn next_number(conn: &rusqlite::Connection, to_key: &str, day: i64) -> Result<i64, rusqlite::Error> {
    let highest: Option<i64> =
        conn.query_row("SELECT MAX(id) FROM dm_mailbox WHERE to_key = ?1", params![to_key], |r| r.get(0))?;
    let base = day << DAY_SHIFT;
    Ok(highest.map_or(base, |h| (h + 1).max(base)))
}

impl Storage {
    /// Deposit a sealed envelope into `to_key`'s mailbox. Returns its number in that mailbox.
    pub fn mailbox_put(&self, to_key: &str, content: &str) -> Result<i64, rusqlite::Error> {
        let day = unix_day_now();
        // The writer connection is one, behind its lock, so the number read and the row written
        // cannot interleave with another put.
        self.with_conn(|conn| {
            let id = next_number(conn, to_key, day)?;
            conn.execute(
                "INSERT INTO dm_mailbox (id, to_key, content, received_day) VALUES (?1, ?2, ?3, ?4)",
                params![id, to_key, content, day],
            )?;
            Ok(id)
        })
    }

    /// Fetch up to `limit` envelopes addressed to `to_key` with id > `after_id`,
    /// oldest first. The client pages by passing the last id back. A cleared
    /// mailbox's marker (empty content) is never handed out.
    pub fn mailbox_fetch(
        &self,
        to_key: &str,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<DmMailRow>, rusqlite::Error> {
        // Read-only: pure SELECT. Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, content FROM dm_mailbox
                 WHERE to_key = ?1 AND id > ?2 AND content <> ''
                 ORDER BY id ASC LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(params![to_key, after_id, limit as i64], |row| {
                    Ok(DmMailRow { id: row.get(0)?, content: row.get(1)? })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
    }

    /// Delete EVERYTHING queued for `to_key` (user-initiated scrub). The
    /// caller must have authenticated as `to_key` — a user can only purge
    /// their own mailbox. Returns envelopes deleted.
    ///
    /// One empty marker row is left holding the mailbox's last number (dated today, so it ages
    /// out with the window): without it the next envelope the same day would take the day's first
    /// number again, below where the person's devices already read to, and they would never fetch
    /// it. A device pages by "after the last number I saw" (net/mailbox_fetch.rs).
    pub fn mailbox_purge(&self, to_key: &str) -> Result<usize, rusqlite::Error> {
        let day = unix_day_now();
        self.with_conn(|conn| {
            let highest: Option<i64> =
                conn.query_row("SELECT MAX(id) FROM dm_mailbox WHERE to_key = ?1", params![to_key], |r| r.get(0))?;
            let n = conn.execute("DELETE FROM dm_mailbox WHERE to_key = ?1 AND content <> ''", params![to_key])?;
            if let Some(last) = highest {
                conn.execute("DELETE FROM dm_mailbox WHERE to_key = ?1 AND id < ?2", params![to_key, last])?;
                conn.execute(
                    "INSERT OR IGNORE INTO dm_mailbox (id, to_key, content, received_day) VALUES (?1, ?2, '', ?3)",
                    params![last, to_key, day],
                )?;
            }
            // Fold the secure_delete-zeroed pages out of the WAL so the
            // purged blobs don't linger there (same discipline as the
            // channel bulk-wipe paths).
            self.wal_truncate(conn);
            Ok(n)
        })
    }

    /// Expire mail older than `ttl_days`. Called from the periodic
    /// maintenance loop and at relay boot. Returns rows deleted.
    pub fn mailbox_expire(&self, ttl_days: i64) -> Result<usize, rusqlite::Error> {
        let cutoff = unix_day_now() - ttl_days.max(1);
        self.with_conn(|conn| {
            let n = conn.execute(
                "DELETE FROM dm_mailbox WHERE received_day < ?1",
                params![cutoff],
            )?;
            if n > 0 {
                self.wal_truncate(conn);
            }
            Ok(n)
        })
    }

    // The Kyber768 key a sender seals a DM to is no longer kept here: it travels in the person's
    // own signed key card (storage/key_cards.rs, 2026-10-22), which every app checks itself.

    /// Look up the name for a public key.
    pub fn name_for_key(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        // Read-only single-row lookup. Read pool.
        self.with_read_conn(|conn| {
            match conn.query_row(
                "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(name) => Ok(Some(name)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Storage {
        Storage::open_temp("dms")
    }

    /// Deposit + fetch: rows come back oldest-first, addressed-only.
    #[test]
    fn mailbox_put_fetch_roundtrip() {
        let db = fresh_db();
        let a = db.mailbox_put("bob", "envelope-1").unwrap();
        let b = db.mailbox_put("bob", "envelope-2").unwrap();
        db.mailbox_put("carol", "not-bobs").unwrap();
        assert!(b > a);
        let rows = db.mailbox_fetch("bob", 0, 100).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].content, "envelope-1");
        assert_eq!(rows[1].content, "envelope-2");
        // Carol's mail never appears in Bob's fetch.
        assert!(rows.iter().all(|r| r.content != "not-bobs"));
    }

    /// Pagination: after_id resumes where the last fetch ended.
    #[test]
    fn mailbox_fetch_pagination() {
        let db = fresh_db();
        for i in 0..5 {
            db.mailbox_put("bob", &format!("env-{i}")).unwrap();
        }
        let first = db.mailbox_fetch("bob", 0, 2).unwrap();
        assert_eq!(first.len(), 2);
        let rest = db.mailbox_fetch("bob", first.last().unwrap().id, 100).unwrap();
        assert_eq!(rest.len(), 3);
        assert_eq!(rest[0].content, "env-2");
    }

    /// Purge deletes only the caller's queue.
    #[test]
    fn mailbox_purge_own_queue_only() {
        let db = fresh_db();
        db.mailbox_put("bob", "b1").unwrap();
        db.mailbox_put("bob", "b2").unwrap();
        db.mailbox_put("carol", "c1").unwrap();
        assert_eq!(db.mailbox_purge("bob").unwrap(), 2);
        assert!(db.mailbox_fetch("bob", 0, 100).unwrap().is_empty());
        assert_eq!(db.mailbox_fetch("carol", 0, 100).unwrap().len(), 1);
    }

    /// TTL expiry removes only rows older than the cutoff.
    #[test]
    fn mailbox_expire_ttl() {
        let db = fresh_db();
        db.mailbox_put("bob", "fresh").unwrap();
        // Backdate a row past the TTL window by writing received_day directly.
        let old_day = unix_day_now() - 45;
        db.with_conn(|conn| -> Result<(), rusqlite::Error> {
            conn.execute(
                "INSERT INTO dm_mailbox (id, to_key, content, received_day) VALUES (?1, 'bob', 'stale', ?2)",
                params![old_day << DAY_SHIFT, old_day],
            )?;
            Ok(())
        })
        .unwrap();
        let removed = db.mailbox_expire(30).unwrap();
        assert_eq!(removed, 1);
        let rows = db.mailbox_fetch("bob", 0, 100).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "fresh");
    }

    /// Every row as a copy of the database shows it, in the order the file keeps them (no ORDER
    /// BY): (to_key, id, received_day, content length).
    fn stored_rows(db: &Storage) -> Vec<(String, i64, i64, usize)> {
        db.with_read_conn(|conn| {
            let mut stmt = conn.prepare("SELECT to_key, id, received_day, length(content) FROM dm_mailbox")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? as usize)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .unwrap()
    }

    /// THE PAIRING PROPERTY (2026-11-06): both apps used to send a DM's recipient copy and the
    /// sender's own copy back to back, and the mailbox numbered rows in arrival order, so a copy of
    /// the database showed the two side by side and paired who wrote to whom. Two DMs from two
    /// senders, interleaved exactly that way (Alice to Bob, then Carol to Dave, each recipient copy
    /// followed at once by its self copy, all four envelopes padded to one size): from the stored
    /// rows alone, nothing pairs a recipient copy with its self copy. There is no hidden arrival
    /// counter (no rowid), the numbers do not climb in arrival order, the file keeps rows by
    /// mailbox rather than by arrival, the only time is the day, and the two self copies are
    /// identical row for row apart from their mailbox, as are the two recipient copies.
    ///
    /// Seen red 2026-11-06 against the AUTOINCREMENT table: "the table keeps a hidden arrival
    /// counter (a rowid)" failed (`SELECT rowid` succeeded on it).
    #[test]
    fn the_stored_rows_do_not_pair_a_message_with_its_self_copy() {
        let db = fresh_db();
        // Keys chosen so the mailbox order (a, b, c, d) is not the arrival order (c, a, d, b).
        let (alice, carol, bob, dave) = ("a1", "b2", "c3", "d4");
        let env = "x".repeat(1024); // one padding bucket for all four
        let arrival = [(bob, &env), (alice, &env), (dave, &env), (carol, &env)];
        let ids: Vec<i64> = arrival.iter().map(|(to, e)| db.mailbox_put(to, e).unwrap()).collect();

        let no_rowid = db.with_read_conn(|conn| Ok(conn.prepare("SELECT rowid FROM dm_mailbox").is_err())).unwrap();
        assert!(no_rowid, "the table keeps a hidden arrival counter (a rowid)");
        assert!(!ids.windows(2).all(|w| w[1] > w[0]), "the numbers climb in arrival order: {ids:?}");

        let rows = stored_rows(&db);
        let order: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(order, vec![alice, carol, bob, dave], "the file keeps rows by mailbox, not by arrival");
        for (to, id, day, _) in &rows {
            assert_eq!(*day, unix_day_now(), "{to}: the only time kept is the day");
            assert_eq!(id >> DAY_SHIFT, *day, "{to}: the number carries the day and nothing finer");
        }
        let row = |k: &str| rows.iter().find(|r| r.0 == k).map(|r| (r.1, r.2, r.3)).unwrap();
        assert_eq!(row(alice), row(carol), "the two self copies are identical apart from their mailbox");
        assert_eq!(row(bob), row(dave), "and so are the two recipient copies");
    }

    /// A device asks for the mail after the last number it saw, so a mailbox's numbers must keep
    /// rising whatever happens to its rows. Clearing the mailbox the same day it got mail must not
    /// start them over: the next envelope still gets a higher number than any device has seen,
    /// and the marker that holds that number is never handed out as mail.
    ///
    /// Seen red 2026-11-06 with the marker step taken out of `mailbox_purge`: "the next number is
    /// above every number a device saw" failed (the new envelope took the day's first number
    /// again, below the device's place).
    #[test]
    fn numbers_keep_rising_after_the_mailbox_is_cleared() {
        let db = fresh_db();
        let mut seen = 0;
        for i in 0..3 {
            seen = db.mailbox_put("bob", &format!("env-{i}")).unwrap();
        }
        assert_eq!(db.mailbox_purge("bob").unwrap(), 3, "three envelopes cleared");
        let next = db.mailbox_put("bob", "after").unwrap();
        assert!(next > seen, "the next number is above every number a device saw ({next} <= {seen})");
        let rows = db.mailbox_fetch("bob", seen, 10).unwrap();
        assert_eq!(rows.iter().map(|r| r.content.as_str()).collect::<Vec<_>>(), vec!["after"], "the device's next fetch finds it");
        assert_eq!(db.mailbox_fetch("bob", 0, 10).unwrap().len(), 1, "the marker is never handed out as mail");
        // A second clear with nothing new since keeps the floor where it was.
        assert_eq!(db.mailbox_purge("bob").unwrap(), 1);
        assert_eq!(db.mailbox_purge("bob").unwrap(), 0, "the marker is not counted as an envelope");
        assert!(db.mailbox_put("bob", "later").unwrap() > next);
    }

    /// THE metadata property: the mailbox schema physically has no sender
    /// column. If someone re-adds one, this fails and the sealed-sender
    /// guarantee is gone.
    #[test]
    fn mailbox_schema_has_no_sender_column() {
        let db = fresh_db();
        db.with_read_conn(|conn| {
            let mut stmt = conn.prepare("PRAGMA table_info(dm_mailbox)").unwrap();
            let cols: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            let expected: Vec<String> =
                ["id", "to_key", "content", "received_day"].iter().map(|s| s.to_string()).collect();
            assert_eq!(cols, expected);
            for c in &cols {
                assert!(
                    !c.contains("from") && !c.contains("name"),
                    "sender-identifying column {c} must not exist"
                );
            }
            Ok(())
        })
        .unwrap();
    }

    /// A database whose mailbox still numbers rows in arrival order (`id INTEGER PRIMARY KEY
    /// AUTOINCREMENT`, the shape until 2026-11-06). Opening it rebuilds the table keyed by mailbox
    /// (WITHOUT ROWID). Its rows all carry arrival-order numbers, so none is kept (2026-11-07,
    /// BUG-262; the operator: "Just drop them now."); new mail is numbered from the day, and a
    /// device that read to an old number gets it.
    ///
    /// Seen red 2026-11-06 before the rebuild existed: "the rebuilt table has no rowid" failed (the
    /// old table was left as it was). Since 2026-11-07 it no longer keeps the old rows: red against
    /// the upgrade from before (the rebuild copying them, nothing dropping them), "no old-numbered
    /// row is kept" (left: 3, right: 0).
    #[test]
    fn opens_the_arrival_order_mailbox_and_rebuilds_it_by_mailbox() {
        let path = humanity_core::test_temp::db("dms_arrival_order");
        let day = unix_day_now();
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(&format!(
                "CREATE TABLE dm_mailbox (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    to_key TEXT NOT NULL,
                    content TEXT NOT NULL,
                    received_day INTEGER NOT NULL
                );
                CREATE INDEX idx_dm_mailbox_to ON dm_mailbox(to_key, id);
                CREATE INDEX idx_dm_mailbox_day ON dm_mailbox(received_day);
                INSERT INTO dm_mailbox (to_key, content, received_day) VALUES ('bob', 'to bob', {day});
                INSERT INTO dm_mailbox (to_key, content, received_day) VALUES ('alice', 'alice self', {day});
                INSERT INTO dm_mailbox (to_key, content, received_day) VALUES ('bob', 'to bob again', {day});"
            ))
            .unwrap();
        }
        let db = Storage::open(&path).expect("open rebuilds the mailbox");
        let no_rowid = db.with_read_conn(|conn| Ok(conn.prepare("SELECT rowid FROM dm_mailbox").is_err())).unwrap();
        assert!(no_rowid, "the rebuilt table has no rowid");
        assert_eq!(stored_rows(&db).len(), 0, "no old-numbered row is kept");
        // A device of Bob's that read to number 1 gets the new mail.
        let next = db.mailbox_put("bob", "new").unwrap();
        assert!(next >= OLD_NUMBER_CEILING, "new mail is numbered from the day");
        let after: Vec<String> = db.mailbox_fetch("bob", 1, 10).unwrap().into_iter().map(|r| r.content).collect();
        assert_eq!(after, vec!["new".to_string()]);
        drop(db);
        // And a second open finds nothing to rebuild or drop.
        let again = Storage::open(&path).expect("reopens");
        assert_eq!(again.mailbox_fetch("bob", 0, 10).unwrap().len(), 1);
    }

    /// THE OLD NUMBERS ARE DROPPED (2026-11-07, docs/BUGS.md BUG-262; the operator: "Just drop them
    /// now. Let's not bother with backwards compatibility until v1.0."). A table already kept by
    /// mailbox (the live one since 2026-11-07) still held envelopes with the arrival-order numbers
    /// 1, 2, 3... they came in with. Opening it deletes those, and nothing else: an envelope
    /// numbered from its day stays, a cleared mailbox's marker numbered from its day stays (its
    /// floor still holds), and the marker of a mailbox cleared while it held only old numbers goes
    /// (an old number too, and the floor it held is below every day's number anyway). A device
    /// that read to an old number gets every new envelope; a second open changes nothing.
    ///
    /// Seen red 2026-11-07 before the migration existed (and again against the upgrade from before):
    /// "only the old-numbered rows are gone" failed, all six rows left (the two old envelopes and the
    /// old marker still there).
    #[test]
    fn opening_drops_every_old_numbered_row_and_nothing_else() {
        let path = humanity_core::test_temp::db("dms_old_numbers");
        let day = unix_day_now();
        let new_id = day << DAY_SHIFT;
        // The line between the two schemes (OLD_NUMBER_CEILING's proof): today's first number, and
        // any number `next_number` gives from now on, is above it; every old number is far below.
        assert!(new_id >= OLD_NUMBER_CEILING, "today's numbers are above the line");
        assert_eq!(fresh_db().mailbox_put("x", "e").unwrap(), new_id, "a mailbox's first number is the day's base");
        {
            // The shape the live table has had since the 2026-11-07 rebuild, with what it holds.
            let db = Storage::open(&path).expect("a fresh database");
            db.with_conn(|conn| -> Result<(), rusqlite::Error> {
                for (id, to, content) in [
                    (7, "bob", "old to bob"),            // arrival-order numbers, kept by the rebuild
                    (8, "alice", "old alice self"),
                    (5, "carol", ""),                    // carol cleared while she held only old mail
                    (new_id, "bob", "new to bob"),       // numbered from its day
                    (new_id + 1, "bob", "new to bob 2"),
                    (new_id + 3, "dave", ""),            // dave cleared after new mail: his floor
                ] {
                    conn.execute(
                        "INSERT INTO dm_mailbox (id, to_key, content, received_day) VALUES (?1, ?2, ?3, ?4)",
                        params![id, to, content, day],
                    )?;
                }
                Ok(())
            })
            .unwrap();
        }
        let db = Storage::open(&path).expect("open drops the old numbers");
        let left: Vec<(String, i64, String)> = db
            .with_read_conn(|conn| {
                let mut stmt = conn.prepare("SELECT to_key, id, content FROM dm_mailbox ORDER BY to_key, id")?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .unwrap();
        let want = vec![
            ("bob".to_string(), new_id, "new to bob".to_string()),
            ("bob".to_string(), new_id + 1, "new to bob 2".to_string()),
            ("dave".to_string(), new_id + 3, String::new()),
        ];
        assert_eq!(left, want, "only the old-numbered rows are gone");
        // A device of Bob's that had read to old number 7 gets the new mail, and Dave's floor holds.
        let bobs: Vec<String> = db.mailbox_fetch("bob", 7, 10).unwrap().into_iter().map(|r| r.content).collect();
        assert_eq!(bobs, vec!["new to bob".to_string(), "new to bob 2".to_string()]);
        assert!(db.mailbox_put("dave", "after").unwrap() > new_id + 3, "the floor a cleared mailbox holds is kept");
        assert!(db.mailbox_put("carol", "fresh").unwrap() > 8, "a mailbox whose old marker went still numbers above every old number");
        drop(db);
        // Running twice is harmless: nothing more goes.
        let again = Storage::open(&path).expect("reopens");
        assert_eq!(again.mailbox_fetch("bob", 0, 10).unwrap().len(), 2);
        assert_eq!(again.mailbox_fetch("dave", 0, 10).unwrap().len(), 1);
        assert_eq!(again.mailbox_fetch("carol", 0, 10).unwrap().len(), 1);
    }

    /// Migration: a database that still has the legacy `direct_messages`
    /// table (with its plaintext sender/recipient graph) gets that table
    /// DROPPED on open, and the new mailbox works. Pattern follows
    /// `opens_a_pre_v0675_database_and_migrates_it` (BUG-046 discipline).
    #[test]
    fn opens_a_legacy_db_and_drops_the_dm_graph() {
        let path = humanity_core::test_temp::db("dms_legacy");
        // Build a pre-v2 database shape by hand: the legacy table + rows.
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE direct_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    from_key TEXT NOT NULL,
                    from_name TEXT NOT NULL,
                    to_key TEXT NOT NULL,
                    content TEXT NOT NULL,
                    timestamp INTEGER NOT NULL,
                    read INTEGER NOT NULL DEFAULT 0,
                    encrypted INTEGER DEFAULT 0,
                    nonce TEXT DEFAULT NULL
                );
                INSERT INTO direct_messages (from_key, from_name, to_key, content, timestamp)
                VALUES ('alice', 'Alice', 'bob', 'the graph edge', 1700000000000);",
            )
            .unwrap();
        }
        let db = Storage::open(&path).expect("open migrates");
        db.with_read_conn(|conn| {
            let gone: bool = conn
                .prepare("SELECT 1 FROM direct_messages LIMIT 1")
                .is_err();
            assert!(gone, "legacy direct_messages table must be dropped");
            Ok(())
        })
        .unwrap();
        // And the new mailbox is live in the same database.
        db.mailbox_put("bob", "sealed").unwrap();
        assert_eq!(db.mailbox_fetch("bob", 0, 10).unwrap().len(), 1);
    }
}
