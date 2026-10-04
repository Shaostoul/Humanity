//! Accounts erased here, remembered for a limited time (BUG-135, 2026-10-04).
//!
//! WHY THIS EXISTS. Erasing your account (Settings > Account, the relay's `account_delete`)
//! removes everything this server keeps about you, and your devices that are online at that
//! moment are told (`account_erased`) and stop reconnecting. A device that was OFFLINE then
//! (a second computer with the app closed, a web tab between reconnects) used to come back
//! later with the same key and be signed up again by itself: the relay had no record of the
//! erase, so it registered the name again, and a game join claimed a plot.
//!
//! THE OPERATOR'S DECISION (2026-10-04), verbatim: "Let's go with option 2 that way we have a
//! way to cull the list over a period of time. That way we don't end up with a massive log of
//! all the accounts that erased themselves after many years or a malicious attack."
//!
//! So the relay remembers an erase, but only:
//!   - as a ONE-WAY KEYED FINGERPRINT of the public key (BLAKE3, keyed with a 32-byte secret
//!     this relay generates once and keeps beside the live database, the way it keeps
//!     `backup.key`; see `load_secret`), never the key itself, never a name;
//!   - with the DAY of the erase and nothing else (day granularity, like `dm_mailbox`'s
//!     `received_day`; the table is WITHOUT ROWID, so not even the order of erases within a
//!     day is kept);
//!   - for `erased_accounts_ttl_days` days (server setting, default 30), after which the row is
//!     deleted (`erased_accounts_sweep`, run at relay start and on the six-hour maintenance
//!     sweep, the same one that expires the DM mailbox);
//!   - and never more than `erased_accounts_cap` rows at once (server setting, default
//!     100,000): when full, the oldest go first, so a flood of erases cannot grow the table
//!     without bound.
//!
//! WHY KEYED, AND WHY THE SECRET LIVES OUTSIDE THE DATABASE. Public keys are public: anyone
//! who has seen someone in chat has their key. A plain hash would let whoever holds a copy of
//! the database (a backup, a breach, a subpoena) hash a list of known keys and learn who
//! erased their account here. With the secret in a separate file beside the live database,
//! a copy of the database alone cannot be checked that way. If the secret file is lost (a
//! server moved without it), a new one is made and the old fingerprints simply never match
//! again; they are culled on schedule like any other row. The cost of that is only the old
//! behaviour: another device of an erased account may sign up again by itself.
//!
//! What uses it: the identify path (handlers/sign_ups.rs `erased_at_identify`) tells a
//! remembered key's client that the account was erased here and signs nothing up, unless the
//! person pressed Connect (native) or Enter (web), whose identify carries `sign_up_again`; the
//! game join refuses a remembered key (handlers/home_plots.rs `refused_join`); the export lists
//! the entry to the key it is about (storage/account.rs `export_account`).

use super::Storage;
use rusqlite::params;

/// The secret's file, created beside the live database on first use (`data/` on the VPS, next
/// to `relay.db` and `backup.key`). `*.key` is git-ignored and outside the backups directory.
pub const KEY_FILE: &str = "erased-accounts.key";

/// Domain separation for the fingerprint, so the same secret could never produce the same
/// value for any other purpose.
const FINGERPRINT_CONTEXT: &[u8] = b"hum/erased-account/v1\n";

/// The one-way fingerprint of `public_key` under this relay's `secret`: 64 hex characters.
/// The key is trimmed and lower-cased first, so the same identity written in upper-case hex
/// is the same entry (identify and the erase both use the string the client sent).
pub fn fingerprint(secret: &[u8; 32], public_key: &str) -> String {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(FINGERPRINT_CONTEXT);
    h.update(public_key.trim().to_ascii_lowercase().as_bytes());
    h.finalize().to_hex().to_string()
}

/// This relay's fingerprint secret for the database at `db_path`: read from `KEY_FILE` beside
/// it, or created there on first use (backup_crypto.rs, the same code that keeps
/// `backup.key`). If the file cannot be read or written, a secret for this run only is used
/// and the failure is logged: entries made now then stop matching at the next start, which
/// is the behaviour before this existed, never a crash.
pub(crate) fn load_secret(db_path: &std::path::Path) -> [u8; 32] {
    let dir = super::backup_crypto::key_dir_for_db(db_path);
    if let Some(k) = super::backup_crypto::load_or_create_named_key(&dir, KEY_FILE) {
        return k;
    }
    tracing::error!(
        "erased accounts: no usable {KEY_FILE} in {}; using a secret for this run only, so \
         erases remembered now are forgotten at the next start",
        dir.display()
    );
    rand::random()
}

/// The sentence a person reads before they erase (native Settings > Account, web Erase
/// account, the receipt after the erase). `days` is this server's `erased_accounts_ttl_days`;
/// None when the app has not received it yet, which names the default instead of a number it
/// does not know. One sentence; the web says the same words (web/chat/chat-privacy.js
/// `eraseMemorySentence`).
pub fn erase_memory_sentence(days: Option<i64>) -> String {
    let how_long = match days {
        Some(1) => "for 1 day".to_string(),
        Some(n) => format!("for {n} days"),
        None => "for a set number of days (30 unless its admin changed it)".to_string(),
    };
    format!(
        "After the erase this server remembers {how_long} that this account was erased, as a \
         one-way fingerprint that is not your name or your data, so your other devices do not \
         sign you up again by themselves; after that nothing of it is left."
    )
}

impl Storage {
    /// The fingerprint `key` is remembered under on this relay.
    pub fn erased_account_fingerprint(&self, key: &str) -> String {
        fingerprint(&self.erase_secret, key)
    }

    /// Remember that `key` erased its account here, today. Erasing again moves the day
    /// forward. Then trims the table to the cap, oldest first, never dropping the entry just
    /// written (`trim_erased_accounts`).
    pub fn remember_erased_account(&self, key: &str) -> Result<(), rusqlite::Error> {
        let cap = self.get_server_settings().map(|s| s.erased_accounts_cap).unwrap_or(DEFAULT_CAP);
        let fp = self.erased_account_fingerprint(key);
        let day = super::dms::unix_day_now();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO erased_accounts (fingerprint, erased_day) VALUES (?1, ?2)",
                params![fp, day],
            )?;
            trim_erased_accounts(conn, cap, &fp)?;
            Ok(())
        })
    }

    /// True while this relay remembers that `key` erased its account here: an entry exists
    /// and is younger than `erased_accounts_ttl_days`. Checked against the setting itself, so
    /// a shortened window takes effect at once, before the sweep deletes the rows.
    pub fn erased_account_remembered(&self, key: &str) -> bool {
        let ttl = self.get_server_settings().map(|s| s.erased_accounts_ttl_days).unwrap_or(DEFAULT_TTL_DAYS);
        let cutoff = super::dms::unix_day_now() - ttl.max(1);
        let fp = self.erased_account_fingerprint(key);
        self.with_read_conn(|conn| {
            use rusqlite::OptionalExtension;
            conn.query_row(
                "SELECT 1 FROM erased_accounts WHERE fingerprint = ?1 AND erased_day >= ?2",
                params![fp, cutoff],
                |_| Ok(()),
            )
            .optional()
            .map(|found| found.is_some())
        })
        .unwrap_or_else(|e| {
            // Fail open: a read error must not lock a person out of signing in. The cost is
            // the old behaviour (an erased account's other device may sign up again).
            tracing::error!("erased accounts: lookup failed: {e}");
            false
        })
    }

    /// The person chose to come back (their identify carried `sign_up_again`): forget the
    /// entry, so this key is an ordinary new sign-up from here on. True when one was there.
    pub fn forget_erased_account(&self, key: &str) -> Result<bool, rusqlite::Error> {
        let fp = self.erased_account_fingerprint(key);
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM erased_accounts WHERE fingerprint = ?1", params![fp])?;
            Ok(n > 0)
        })
    }

    /// The cull: delete entries older than `ttl_days`, then trim to `cap`, oldest first.
    /// Returns (expired, trimmed). Run at relay start and on the six-hour maintenance sweep
    /// (relay/mod.rs), beside the DM mailbox expiry.
    pub fn erased_accounts_sweep(&self, ttl_days: i64, cap: i64) -> Result<(usize, usize), rusqlite::Error> {
        let cutoff = super::dms::unix_day_now() - ttl_days.max(1);
        self.with_conn(|conn| {
            let expired = conn.execute("DELETE FROM erased_accounts WHERE erased_day < ?1", params![cutoff])?;
            let trimmed = trim_erased_accounts(conn, cap, "")?;
            if expired + trimmed > 0 {
                // Fold the secure_delete-zeroed pages out of the WAL, as the mailbox expiry does.
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            }
            Ok((expired, trimmed))
        })
    }

    /// How many erases this relay remembers right now.
    pub fn erased_accounts_count(&self) -> Result<i64, rusqlite::Error> {
        self.with_read_conn(|conn| conn.query_row("SELECT COUNT(*) FROM erased_accounts", [], |r| r.get(0)))
    }
}

/// The defaults, the same as server_settings.rs's (a settings read that fails falls back here).
const DEFAULT_TTL_DAYS: i64 = 30;
const DEFAULT_CAP: i64 = 100_000;

/// Delete the oldest rows until at most `cap` remain. Oldest is the earliest day; within one
/// day the rows are equally old (only the day is kept), so the tie goes by fingerprint, which
/// is as good as random. `keep` (the entry just written) is never the one dropped, so the
/// newest erase is always remembered even when a flood fills the table in a single day.
fn trim_erased_accounts(conn: &rusqlite::Connection, cap: i64, keep: &str) -> Result<usize, rusqlite::Error> {
    let cap = cap.max(1);
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM erased_accounts", [], |r| r.get(0))?;
    if count <= cap {
        return Ok(0);
    }
    conn.execute(
        "DELETE FROM erased_accounts WHERE fingerprint IN (
             SELECT fingerprint FROM erased_accounts WHERE fingerprint != ?1
             ORDER BY erased_day ASC, fingerprint ASC LIMIT ?2)",
        params![keep, count - cap],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database in a folder of its own, so its fingerprint secret (made beside it) is its
    /// own too: tests that share one folder would race to create one shared secret.
    fn fresh_db(tag: &str) -> (Storage, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("hum_erased_{tag}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test folder");
        let path = dir.join("relay.db");
        (Storage::open(&path).expect("open test db"), path)
    }

    /// Write an entry with a chosen day (the tests' clock), as `remember_erased_account` would.
    fn put_on_day(db: &Storage, key: &str, day: i64) {
        let fp = db.erased_account_fingerprint(key);
        db.with_conn(|c| c.execute("INSERT OR REPLACE INTO erased_accounts (fingerprint, erased_day) VALUES (?1, ?2)", params![fp, day]))
            .expect("insert");
    }

    fn set_window(db: &Storage, ttl_days: i64, cap: i64) {
        let mut s = db.get_server_settings().expect("settings");
        s.erased_accounts_ttl_days = ttl_days;
        s.erased_accounts_cap = cap;
        assert!(db.set_server_settings(&s, "admin_key").expect("save"));
    }

    /// Record, look up, forget: an erase is remembered for its key and no other, and coming
    /// back forgets it.
    ///
    /// Seen red 2026-10-04 with `remember_erased_account` writing nothing (its INSERT taken
    /// out): "assertion failed: an erase is remembered for the key that erased".
    #[test]
    fn an_erase_is_remembered_for_that_key_only_until_it_comes_back() {
        let (db, _) = fresh_db("record");
        assert!(!db.erased_account_remembered("aa11"), "nothing remembered before any erase");
        db.remember_erased_account("aa11").expect("record");
        assert!(db.erased_account_remembered("aa11"), "an erase is remembered for the key that erased");
        assert!(db.erased_account_remembered("  AA11 "), "the same key in upper case, with spaces");
        assert!(!db.erased_account_remembered("bb22"), "another key is not remembered");
        assert_eq!(db.erased_accounts_count().unwrap(), 1);
        assert!(db.forget_erased_account("aa11").unwrap(), "coming back forgets the entry");
        assert!(!db.erased_account_remembered("aa11"));
        assert!(!db.forget_erased_account("aa11").unwrap(), "nothing left to forget");
    }

    /// Expiry by day: an entry older than the window is not remembered (at once, even before
    /// the sweep) and the sweep deletes it; one inside the window stays.
    ///
    /// Seen red 2026-10-04 with the sweep's DELETE by day taken out: "assertion `left ==
    /// right` failed: the sweep deletes the entry past the window / left: 0 / right: 1".
    #[test]
    fn entries_older_than_the_window_are_forgotten_and_culled() {
        let (db, _) = fresh_db("expiry");
        set_window(&db, 30, 100_000);
        let today = super::super::dms::unix_day_now();
        put_on_day(&db, "old_key", today - 31);
        put_on_day(&db, "edge_key", today - 30);
        put_on_day(&db, "new_key", today);
        assert!(!db.erased_account_remembered("old_key"), "31 days ago is past a 30-day window");
        assert!(db.erased_account_remembered("edge_key"), "exactly 30 days ago is still inside it");
        let (expired, trimmed) = db.erased_accounts_sweep(30, 100_000).unwrap();
        assert_eq!(expired, 1, "the sweep deletes the entry past the window");
        assert_eq!(trimmed, 0);
        assert_eq!(db.erased_accounts_count().unwrap(), 2);
        // A shorter window set by the admin takes effect at once.
        set_window(&db, 1, 100_000);
        assert!(!db.erased_account_remembered("edge_key"));
        assert!(db.erased_account_remembered("new_key"));
    }

    /// The cap: when full, the oldest entries go first, and the one just written always stays,
    /// even when every entry has the same day (a flood of erases in one day).
    ///
    /// Seen red 2026-10-04 with the trim taken out of `remember_erased_account`: "assertion
    /// `left == right` failed: the table holds no more than the cap / left: 4 / right: 3".
    #[test]
    fn a_full_table_drops_its_oldest_entries_first() {
        let (db, _) = fresh_db("cap");
        set_window(&db, 30, 3);
        let today = super::super::dms::unix_day_now();
        put_on_day(&db, "day_minus_9", today - 9);
        put_on_day(&db, "day_minus_5", today - 5);
        put_on_day(&db, "day_minus_2", today - 2);
        db.remember_erased_account("today_key").unwrap();
        assert_eq!(db.erased_accounts_count().unwrap(), 3, "the table holds no more than the cap");
        assert!(!db.erased_account_remembered("day_minus_9"), "the oldest went first");
        for kept in ["day_minus_5", "day_minus_2", "today_key"] {
            assert!(db.erased_account_remembered(kept), "{kept} was dropped instead of the oldest");
        }
        // Same day for everyone: the newest erase is never the one dropped.
        let (db, _) = fresh_db("cap_same_day");
        set_window(&db, 30, 2);
        for k in ["s1", "s2", "s3", "s4", "s5"] {
            db.remember_erased_account(k).unwrap();
            assert!(db.erased_account_remembered(k), "{k}, just erased, was dropped by the cap");
        }
        assert_eq!(db.erased_accounts_count().unwrap(), 2);
        // An admin lowering the cap: the sweep trims to it.
        set_window(&db, 30, 1);
        assert_eq!(db.erased_accounts_sweep(30, 1).unwrap(), (0, 1));
        assert_eq!(db.erased_accounts_count().unwrap(), 1);
    }

    /// The fingerprint is not the key: no column holds the key or any part of it, the table
    /// has exactly the two columns, and the value depends on this relay's secret (a copy of
    /// the database alone cannot be checked against a list of known keys).
    ///
    /// Seen red 2026-10-04 with `fingerprint` returning the trimmed key itself: "a stored
    /// value holds the key: [\"4f2a...\"]" (the first assertion).
    #[test]
    fn the_fingerprint_is_not_the_key_and_depends_on_the_secret() {
        let (db, path) = fresh_db("fingerprint");
        let key = "4f2a9c11d0e7b35a6c88aa01f4e2d9b7";
        db.remember_erased_account(key).unwrap();
        let (cols, stored): (Vec<String>, Vec<String>) = db
            .with_read_conn(|c| {
                let mut st = c.prepare("PRAGMA table_info(erased_accounts)")?;
                let cols: Vec<String> = st.query_map([], |r| r.get::<_, String>(1))?.filter_map(|r| r.ok()).collect();
                let mut st = c.prepare("SELECT fingerprint FROM erased_accounts")?;
                let fps: Vec<String> = st.query_map([], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
                Ok((cols, fps))
            })
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert!(
            stored.iter().all(|v| !v.contains(&key[..8]) && !key.contains(&v[..8])),
            "a stored value holds the key: {stored:?}"
        );
        assert_eq!(cols, vec!["fingerprint".to_string(), "erased_day".to_string()], "only the fingerprint and the day");
        let fp = db.erased_account_fingerprint(key);
        assert_eq!(fp.len(), 64);
        assert_ne!(fp, fingerprint(&[0u8; 32], key), "another secret gives another fingerprint");
        assert_eq!(fp, fingerprint(&db.erase_secret, &key.to_uppercase()), "case does not change it");
        // The secret is kept: a restart on the same database still recognises the key.
        drop(db);
        let reopened = Storage::open(&path).expect("reopen");
        assert!(reopened.erased_account_remembered(key), "a restart forgot the erase");
    }

    /// BUG-046 discipline: a relay whose database was made by the previous schema (no
    /// `erased_accounts` table, no `erased_accounts_*` settings columns) opens, gains the table
    /// and the two settings at their defaults, keeps the operator's other settings, and the
    /// new paths work on it.
    ///
    /// Seen red 2026-10-04 with the two guarded ALTERs taken out of storage/mod.rs: "settings
    /// after the upgrade: SqlInputError { ... msg: \"no such column: erased_accounts_ttl_days\"
    /// ... }" (get_server_settings read a column the old file lacks; on the live relay every
    /// settings read would have fallen back to defaults and every Save failed).
    #[test]
    fn opens_a_database_made_by_the_previous_schema() {
        let (db, path) = fresh_db("previous_schema");
        let mut s = db.get_server_settings().unwrap();
        s.dm_mailbox_ttl_days = 7; // an operator's tuned value, to survive the upgrade
        assert!(db.set_server_settings(&s, "op_key").unwrap());
        drop(db);
        {
            // Rewind the file to the previous schema.
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "DROP TABLE erased_accounts;
                 ALTER TABLE server_settings DROP COLUMN erased_accounts_ttl_days;
                 ALTER TABLE server_settings DROP COLUMN erased_accounts_cap;",
            )
            .expect("rewind to the previous schema (SQLite >= 3.35)");
            assert!(conn.prepare("SELECT 1 FROM erased_accounts").is_err(), "test premise: the table is gone");
        }
        let db = Storage::open(&path).unwrap_or_else(|e| panic!("reopen MUST NOT fail on the previous schema: {e:?}"));
        let got = db.get_server_settings().unwrap_or_else(|e| panic!("settings after the upgrade: {e:?}"));
        assert_eq!(got.erased_accounts_ttl_days, 30, "the window defaults to 30 days");
        assert_eq!(got.erased_accounts_cap, 100_000, "the cap defaults to 100,000");
        assert_eq!(got.dm_mailbox_ttl_days, 7, "the operator's other settings survive");
        assert_eq!(db.erased_accounts_count().unwrap(), 0, "the table exists, empty");
        db.remember_erased_account("cc33").unwrap();
        assert!(db.erased_account_remembered("cc33"));
    }

    /// The sentence a person reads before erasing: one sentence, the real number of days, and
    /// the three things it promises (a one-way fingerprint, not their name or data, nothing
    /// left after). The web says exactly the same (scripts/tests/erase-sign-up-again.test.js).
    ///
    /// Seen red 2026-10-04 with the number of days replaced by "for a while": "assertion `left
    /// == right` failed / left: \"After the erase this server remembers for a while that this
    /// account was erased, ...\"".
    #[test]
    fn the_erase_sentence_names_the_days_and_what_is_kept() {
        let s = erase_memory_sentence(Some(30));
        assert_eq!(
            s,
            "After the erase this server remembers for 30 days that this account was erased, as a one-way fingerprint that is not your name or your data, so your other devices do not sign you up again by themselves; after that nothing of it is left."
        );
        assert!(erase_memory_sentence(Some(12)).contains("for 12 days"), "it names the real number of days");
        assert!(erase_memory_sentence(Some(1)).contains("for 1 day that"));
        assert!(erase_memory_sentence(None).contains("30 unless its admin changed it"));
        for s in [erase_memory_sentence(Some(30)), erase_memory_sentence(None)] {
            assert_eq!(s.matches(". ").count(), 0, "one sentence: {s}");
            assert!(s.ends_with('.'));
        }
    }
}
