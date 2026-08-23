//! DM mailbox — sealed-sender store-and-forward (v2, 2026-08-23).
//!
//! REPLACES the old `direct_messages` table, which stored
//! `from_key, from_name, to_key, timestamp` in the clear next to the
//! ciphertext and therefore accumulated a complete, subpoena-exportable
//! who-talked-to-whom graph (the exact dataset the Take-Two/Discord
//! court order harvested from 100k bystanders). The migration in
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
//!     Display ordering comes from the signed `ts` inside the envelope;
//!     fetch pagination comes from the rowid.
//!   - Mail EXPIRES after `dm_mailbox_ttl_days` (server_settings, default
//!     30). Clients keep long-term history locally (encrypted at rest);
//!     the server holds only the recent delivery window. `mailbox_purge`
//!     lets a user delete their own queue immediately.
//!
//! What a subpoena of this table yields: pseudonymous keys, each with N
//! undecryptable blobs and day-granularity arrival dates. No sender, no
//! graph, no names, no content.

use super::Storage;
use rusqlite::params;

/// Seconds per day — `received_day` is `unix_seconds / 86400`.
const SECS_PER_DAY: u64 = 86_400;

/// Current unix day (UTC).
pub fn unix_day_now() -> i64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / SECS_PER_DAY) as i64
}

/// One fetched mailbox row: rowid (for pagination) + the opaque envelope.
#[derive(Debug, Clone)]
pub struct DmMailRow {
    pub id: i64,
    pub content: String,
}

impl Storage {
    /// Deposit a sealed envelope into `to_key`'s mailbox. Returns the row id.
    pub fn mailbox_put(&self, to_key: &str, content: &str) -> Result<i64, rusqlite::Error> {
        let day = unix_day_now();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO dm_mailbox (to_key, content, received_day) VALUES (?1, ?2, ?3)",
                params![to_key, content, day],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Fetch up to `limit` envelopes addressed to `to_key` with id > `after_id`,
    /// oldest first. The client pages by passing the last id back.
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
                 WHERE to_key = ?1 AND id > ?2
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
    /// their own mailbox. Returns rows deleted.
    pub fn mailbox_purge(&self, to_key: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM dm_mailbox WHERE to_key = ?1", params![to_key])?;
            // Fold the secure_delete-zeroed pages out of the WAL so the
            // purged blobs don't linger there (same discipline as the
            // channel bulk-wipe paths).
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
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
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            }
            Ok(n)
        })
    }

    // ── Kyber768 DM key (full-PQ cutover, v0.262.33) ──
    //
    // `public_key` IS the Dilithium3 identity hex. `kyber_public` is
    // the recipient's ML-KEM-768 encapsulation key (base64) a sender
    // encapsulates a per-message secret to (net::dm_pq / web pq.js).
    // The relay only stores + serves it; it never sees DM plaintext.

    /// Store/update the recipient's Kyber768 public key (base64).
    pub fn store_kyber_public(&self, public_key: &str, kyber_public: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE registered_names SET kyber_public = ?1 WHERE public_key = ?2",
                params![kyber_public, public_key],
            )?;
            Ok(())
        })
    }

    /// Get the Kyber768 public key (base64) for a Dilithium identity.
    pub fn get_kyber_public(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        // Read-only single-row lookup (DM key fetch before sealing). Read pool.
        self.with_read_conn(|conn| {
            match conn.query_row(
                "SELECT kyber_public FROM registered_names WHERE public_key = ?1 AND kyber_public IS NOT NULL LIMIT 1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(key) => Ok(Some(key)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

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
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_dms_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
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
                "INSERT INTO dm_mailbox (to_key, content, received_day) VALUES ('bob', 'stale', ?1)",
                params![old_day],
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

    /// Migration: a database that still has the legacy `direct_messages`
    /// table (with its plaintext sender/recipient graph) gets that table
    /// DROPPED on open, and the new mailbox works. Pattern follows
    /// `opens_a_pre_v0675_database_and_migrates_it` (BUG-046 discipline).
    #[test]
    fn opens_a_legacy_db_and_drops_the_dm_graph() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_dms_legacy_{pid}_{nanos}.db"));
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
