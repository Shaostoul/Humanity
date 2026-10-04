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
//!   - with the DAY of the erase and the window in days that was in force on that day
//!     (day granularity, like `dm_mailbox`'s `received_day`; the table is WITHOUT ROWID, so
//!     not even the order of erases within a day is kept). The window is stored because it
//!     is what the person read before deciding ("remembers for up to 30 days"): an admin who
//!     later RAISES `erased_accounts_ttl_days` must not stretch that promise (review finding
//!     1), while one who LOWERS it shortens every entry at once. Each entry lives for the
//!     smaller of the two;
//!   - for at most that many days: an entry is matched while it is younger than that many
//!     whole UTC days (a 30-day window matches on the day of the erase and the 29 after it),
//!     so the number a person reads is an upper bound, never an underestimate (review finding
//!     4). The sweep (`erased_accounts_sweep`, at relay start, on the six-hour maintenance
//!     pass, and after every change to the server's settings: storage/expiry.rs) deletes
//!     exactly the entries the lookup no longer matches, plus any dated after tomorrow (a
//!     clock that jumped forward at the erase, finding 5);
//!   - and never more than `erased_accounts_cap` rows at once (server setting, default
//!     100,000): when full, the oldest go first, so a flood of erases cannot grow the table
//!     without bound.
//!
//! What outlives an entry, said plainly in the sentence a person reads (`erase_memory_sentence`)
//! and in docs/reference/retention_and_deletion_semantics.md: a copy of the row in each backup
//! of the database taken while it existed, until that backup is deleted. The relay's log lines
//! about erased accounts name no key (handlers/sign_ups.rs), so the logs keep only that such an
//! event happened.
//!
//! WHY KEYED, AND WHY THE SECRET LIVES OUTSIDE THE DATABASE. Public keys are public: anyone
//! who has seen someone in chat has their key. A plain hash would let whoever holds a copy of
//! the database (a backup, a breach, a subpoena) hash a list of known keys and learn who
//! erased their account here. With the secret in a separate file beside the live database,
//! a copy of the database alone cannot be checked that way. If the secret file is lost (a
//! server moved without it), a new one is made and the old fingerprints simply never match
//! again; they are culled on schedule like any other row. The cost of that is only the old
//! behaviour: another device of an erased account may sign up again by itself. A file that is
//! there but damaged is never overwritten; this run then uses a secret of its own and says so
//! (/health `erase_memory`, `just brief`), because every restart would otherwise forget every
//! remembered erase with nothing but a log line to show for it (finding 7).
//!
//! What uses it: the identify path (handlers/sign_ups.rs `erased_at_identify`) tells a
//! remembered key's client that the account was erased here and signs nothing up, unless the
//! person pressed Connect (native) or Enter (web), whose identify carries `sign_up_again`; the
//! name registration and the membership row on that same path check again in the same step
//! as they write (`register_name_unless_erased`, `join_server_unless_erased`), so an erase
//! landing between the identify and the registration still wins (finding 12); the game join
//! refuses a remembered key (handlers/home_plots.rs `refused_join`, and again under the game
//! world's lock in msg_handlers.rs `handle_game_join`); the export lists the entry to the key
//! it is about (storage/account.rs `export_account`).

use super::Storage;
use rusqlite::{params, Connection, OptionalExtension};

/// The secret's file, created beside the live database on first use (`data/` on the VPS, next
/// to `relay.db` and `backup.key`). `*.key` is git-ignored and outside the backups directory.
pub const KEY_FILE: &str = "erased-accounts.key";

/// Domain separation for the fingerprint, so the same secret could never produce the same
/// value for any other purpose.
const FINGERPRINT_CONTEXT: &[u8] = b"hum/erased-account/v1\n";

/// The defaults, the same as server_settings.rs's (a settings read that fails falls back here).
pub(crate) const DEFAULT_TTL_DAYS: i64 = 30;
const DEFAULT_CAP: i64 = 100_000;

/// The one-way fingerprint of `public_key` under this relay's `secret`: 64 hex characters.
/// The key is trimmed and lower-cased first, so the same identity written in upper-case hex
/// is the same entry (identify and the erase both use the string the client sent).
pub fn fingerprint(secret: &[u8; 32], public_key: &str) -> String {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(FINGERPRINT_CONTEXT);
    h.update(public_key.trim().to_ascii_lowercase().as_bytes());
    h.finalize().to_hex().to_string()
}

/// This relay's fingerprint secret for the database at `db_path`, and whether it is KEPT
/// (read from `KEY_FILE` beside the database, or created there now). If the file is damaged,
/// unreadable or cannot be written, a secret for this run only is used (false): entries made
/// now then stop matching at the next start, which is the behaviour before this existed,
/// never a crash. The storage reports that (`erase_memory_kept`).
pub(crate) fn load_secret(db_path: &std::path::Path) -> ([u8; 32], bool) {
    let dir = super::backup_crypto::key_dir_for_db(db_path);
    if let (Some(k), _) = super::backup_crypto::load_named_key(&dir, KEY_FILE) {
        return (k, true);
    }
    tracing::error!(
        "erased accounts: no usable {KEY_FILE} in {}; using a secret for this run only, so \
         erases remembered now are forgotten at the next start (fix or remove the file; /health \
         reports erase_memory: this_run_only until then)",
        dir.display()
    );
    (rand::random(), false)
}

/// The sentence a person reads before they erase (native Settings > Account, web Erase
/// account, the receipt after the erase). `days` is this server's `erased_accounts_ttl_days`.
/// A client that does not have the number (a relay too old to send it) shows nothing rather
/// than a promise about a server that may not keep it (review finding 6). "Up to": the entry
/// is matched for at most that many days. The backups clause: a copy rides in each backup taken
/// while the entry existed, until that backup is deleted (finding 2). One sentence; the web
/// says the same words (web/chat/chat-privacy.js `eraseMemorySentence`).
pub fn erase_memory_sentence(days: i64) -> String {
    let how_long = if days == 1 { "for up to 1 day".to_string() } else { format!("for up to {days} days") };
    format!(
        "After the erase this server remembers {how_long} that this account was erased, as a \
         one-way fingerprint that is not your name or your data, so your other devices do not \
         sign you up again by themselves; then the entry is deleted here, and a copy of it in \
         this server's backups lasts until that backup is deleted."
    )
}

/// What `redeem_link_code_unless_erased` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkRedeemed {
    /// The key is now registered under this name (and has the code creator's role).
    Linked(String),
    /// No such code, or it expired: nothing written.
    NoSuchCode,
    /// This relay remembers the key erasing its account here: nothing written, the code kept.
    ErasedHere,
}

impl Storage {
    /// The fingerprint `key` is remembered under on this relay.
    pub fn erased_account_fingerprint(&self, key: &str) -> String {
        fingerprint(&self.erase_secret, key)
    }

    /// False when this run could not read or keep its fingerprint secret (`load_secret`), so
    /// the erases it remembers are forgotten at the next start. /health says so.
    pub fn erase_memory_kept(&self) -> bool {
        self.erase_secret_kept
    }

    /// The window in force now, in days (the server setting, at least 1).
    fn erase_window_now(&self) -> i64 {
        self.get_server_settings().map(|s| s.erased_accounts_ttl_days).unwrap_or(DEFAULT_TTL_DAYS).max(1)
    }

    /// Remember that `key` erased its account here, today, under the window in force now.
    /// Erasing again moves the day forward. Then trims the table to the cap, oldest first,
    /// never dropping the entry just written (`trim_erased_accounts`).
    pub fn remember_erased_account(&self, key: &str) -> Result<(), rusqlite::Error> {
        let cap = self.get_server_settings().map(|s| s.erased_accounts_cap).unwrap_or(DEFAULT_CAP);
        let ttl = self.erase_window_now();
        let fp = self.erased_account_fingerprint(key);
        let day = super::dms::unix_day_now();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO erased_accounts (fingerprint, erased_day, ttl_days) VALUES (?1, ?2, ?3)",
                params![fp, day, ttl],
            )?;
            trim_erased_accounts(conn, cap, &fp)?;
            Ok(())
        })
    }

    /// True while this relay remembers that `key` erased its account here (`remembered_on`).
    /// Checked against the current setting too, so a shortened window takes effect at once,
    /// before the sweep deletes the rows.
    pub fn erased_account_remembered(&self, key: &str) -> bool {
        let window = self.erase_window_now();
        let fp = self.erased_account_fingerprint(key);
        self.with_read_conn(|conn| remembered_on(conn, &fp, window, super::dms::unix_day_now()))
            .unwrap_or_else(|e| {
                // Fail open: a read error must not lock a person out of signing in. The cost
                // is the old behaviour (an erased account's other device may sign up again).
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

    /// `register_name`, unless this relay remembers `key` erasing its account here: the check
    /// and the write in ONE step on the writer connection, which the erase's own record
    /// (`remember_erased_account`) also needs. So an erase that lands between the identify's
    /// check and this write (review finding 12) either comes first and is seen here, or comes
    /// after and its own delete removes the row written here. False: nothing was written.
    pub fn register_name_unless_erased(&self, name: &str, key: &str) -> Result<bool, rusqlite::Error> {
        let (fp, window, today) = (self.erased_account_fingerprint(key), self.erase_window_now(), super::dms::unix_day_now());
        self.with_conn(|conn| {
            if !key.starts_with("bot_") && remembered_on(conn, &fp, window, today)? {
                return Ok(false);
            }
            super::messages::register_name_on(conn, name, key)?;
            Ok(true)
        })
    }

    /// `join_server`, unless this relay remembers `key` erasing its account here, in one step
    /// the same way. Ok(false) when nothing was written: already a member, or erased here.
    pub fn join_server_unless_erased(&self, key: &str, name: &str) -> Result<bool, rusqlite::Error> {
        let (fp, window, today) = (self.erased_account_fingerprint(key), self.erase_window_now(), super::dms::unix_day_now());
        self.with_conn(|conn| {
            if !key.starts_with("bot_") && remembered_on(conn, &fp, window, today)? {
                return Ok(false);
            }
            super::members::join_server_on(conn, key, name)
        })
    }

    /// `redeem_link_code`, unless this relay remembers `key` erasing its account here, in one
    /// step the same way (review of BUG-135 option 2, second round, finding 8): a link code
    /// registers the key under the code's name and gives it the creator's role, so on the
    /// identify path it is a sign-up like the name and the member row. Refused: nothing is
    /// written and the code is not used up.
    pub fn redeem_link_code_unless_erased(&self, code: &str, key: &str) -> Result<LinkRedeemed, rusqlite::Error> {
        let (fp, window, today) = (self.erased_account_fingerprint(key), self.erase_window_now(), super::dms::unix_day_now());
        self.with_conn(|conn| {
            if !key.starts_with("bot_") && remembered_on(conn, &fp, window, today)? {
                return Ok(LinkRedeemed::ErasedHere);
            }
            Ok(match super::messages::redeem_link_code_on(conn, code, key)? {
                Some(name) => LinkRedeemed::Linked(name),
                None => LinkRedeemed::NoSuchCode,
            })
        })
    }

    /// The cull: delete every entry the lookup no longer matches (older than the smaller of
    /// its own window and `ttl_days`, or dated after tomorrow), then trim to `cap`, oldest
    /// first. Returns (expired, trimmed). Run by `run_expiry_sweeps` (storage/expiry.rs).
    pub fn erased_accounts_sweep(&self, ttl_days: i64, cap: i64) -> Result<(usize, usize), rusqlite::Error> {
        let today = super::dms::unix_day_now();
        self.with_conn(|conn| {
            // The exact complement of `remembered_on`'s match, so nothing it no longer
            // recognises stays on disk past the next sweep.
            let expired = conn.execute(
                "DELETE FROM erased_accounts
                 WHERE erased_day <= ?1 - MIN(ttl_days, ?2) OR erased_day > ?1 + 1",
                params![today, ttl_days.max(1)],
            )?;
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

/// Whether the entry `fp` is remembered on `today` with the server's window `window_now`: it
/// is matched while it is younger than the smaller of the window it was made under and the
/// window now, in whole days (so "up to N days" is never exceeded), and is not dated after
/// tomorrow. The sweep deletes exactly what this stops matching.
fn remembered_on(conn: &Connection, fp: &str, window_now: i64, today: i64) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT 1 FROM erased_accounts
         WHERE fingerprint = ?1 AND erased_day > ?2 - MIN(ttl_days, ?3) AND erased_day <= ?2 + 1",
        params![fp, today, window_now.max(1)],
        |_| Ok(()),
    )
    .optional()
    .map(|found| found.is_some())
}

/// Delete the oldest rows until at most `cap` remain. Oldest is the earliest day; within one
/// day the rows are equally old (only the day is kept), so the tie goes by fingerprint, which
/// is as good as random. `keep` (the entry just written) is never the one dropped, so the
/// newest erase is always remembered even when a flood fills the table in a single day.
fn trim_erased_accounts(conn: &Connection, cap: i64, keep: &str) -> Result<usize, rusqlite::Error> {
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
        let path = fresh_dir(tag).join("relay.db");
        (Storage::open(&path).expect("open test db"), path)
    }

    fn fresh_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("hum_erased_{tag}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test folder");
        dir
    }

    /// Write an entry with a chosen day (the tests' clock) under a window of `ttl` days, as
    /// `remember_erased_account` would have on that day.
    fn put_on_day(db: &Storage, key: &str, day: i64, ttl: i64) {
        let fp = db.erased_account_fingerprint(key);
        db.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO erased_accounts (fingerprint, erased_day, ttl_days) VALUES (?1, ?2, ?3)",
                params![fp, day, ttl],
            )
        })
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

    /// Expiry by day: an entry as old as the window or older is not remembered (at once, even
    /// before the sweep) and the sweep deletes it; one inside the window stays.
    ///
    /// Seen red 2026-10-04 with the sweep's DELETE by day taken out: "assertion `left ==
    /// right` failed: the sweep deletes the entries past the window / left: 0 / right: 2".
    #[test]
    fn entries_older_than_the_window_are_forgotten_and_culled() {
        let (db, _) = fresh_db("expiry");
        set_window(&db, 30, 100_000);
        let today = super::super::dms::unix_day_now();
        put_on_day(&db, "old_key", today - 31, 30);
        put_on_day(&db, "edge_key", today - 30, 30);
        put_on_day(&db, "inside_key", today - 29, 30);
        put_on_day(&db, "new_key", today, 30);
        assert!(!db.erased_account_remembered("old_key"), "31 days ago is past a 30-day window");
        assert!(!db.erased_account_remembered("edge_key"), "30 days ago is past it too: 'up to 30 days'");
        assert!(db.erased_account_remembered("inside_key"), "29 days ago is inside it");
        let (expired, trimmed) = db.erased_accounts_sweep(30, 100_000).unwrap();
        assert_eq!(expired, 2, "the sweep deletes the entries past the window");
        assert_eq!(trimmed, 0);
        assert_eq!(db.erased_accounts_count().unwrap(), 2);
        // A shorter window set by the admin takes effect at once, for every entry.
        set_window(&db, 1, 100_000);
        assert!(!db.erased_account_remembered("inside_key"));
        assert!(db.erased_account_remembered("new_key"));
        assert_eq!(db.erased_accounts_sweep(1, 100_000).unwrap(), (1, 0), "the sweep follows the shorter window");
    }

    /// Review finding 1: a person reads "remembers for up to 30 days" and decides; an admin
    /// then raises the window to 365. Their entry must still be forgotten after the 30 days
    /// they were promised, by the lookup and by the sweep. The erase records the window in
    /// force when it was made.
    ///
    /// Seen red 2026-10-04 on c0c04fc39 (the table kept only the day, measured against the
    /// window of today): "a raised window kept an erase past the 30 days it was promised".
    #[test]
    fn raising_the_window_never_extends_an_earlier_erase() {
        let (db, _) = fresh_db("raise");
        set_window(&db, 30, 100_000);
        let today = super::super::dms::unix_day_now();
        put_on_day(&db, "promised_30", today - 31, 30);
        put_on_day(&db, "still_inside", today - 10, 30);
        db.remember_erased_account("recorded_now").unwrap();
        let recorded: i64 = db
            .with_read_conn(|c| {
                c.query_row(
                    "SELECT ttl_days FROM erased_accounts WHERE fingerprint = ?1",
                    params![db.erased_account_fingerprint("recorded_now")],
                    |r| r.get(0),
                )
            })
            .unwrap();
        assert_eq!(recorded, 30, "the erase did not record the window it was made under");
        set_window(&db, 365, 100_000);
        assert!(!db.erased_account_remembered("promised_30"), "a raised window kept an erase past the 30 days it was promised");
        assert!(db.erased_account_remembered("still_inside"), "an erase inside its own window was forgotten");
        assert_eq!(db.erased_accounts_sweep(365, 100_000).unwrap(), (1, 0), "the sweep kept an erase past the 30 days it was promised");
    }

    /// Review finding 5: a row dated in the future (the clock jumped forward at the erase,
    /// then was put right) would never reach the cutoff. Past tomorrow it is not remembered and
    /// the sweep deletes it; tomorrow itself is a day of tolerance.
    ///
    /// Seen red 2026-10-04 on c0c04fc39: "a row dated far in the future is still remembered".
    #[test]
    fn a_row_dated_in_the_future_is_culled() {
        let (db, _) = fresh_db("future");
        set_window(&db, 30, 100_000);
        let today = super::super::dms::unix_day_now();
        put_on_day(&db, "clock_jumped", today + 400, 30);
        put_on_day(&db, "tomorrow", today + 1, 30);
        assert!(!db.erased_account_remembered("clock_jumped"), "a row dated far in the future is still remembered");
        assert_eq!(db.erased_accounts_sweep(30, 100_000).unwrap(), (1, 0), "a row dated far in the future survived the sweep");
        assert!(db.erased_account_remembered("tomorrow"), "a row dated tomorrow is within the tolerance");
    }

    /// Review findings 4 and 15: "for up to N days" is an upper bound. With a window of 1 day
    /// an erase is remembered on its own (UTC) day only; with 30, through the 29th day after.
    ///
    /// Seen red 2026-10-04 on c0c04fc39 (the match included both ends): "a 1-day window
    /// remembered yesterday's erase".
    #[test]
    fn the_window_is_at_most_the_days_named() {
        let (db, _) = fresh_db("bound");
        let today = super::super::dms::unix_day_now();
        set_window(&db, 1, 100_000);
        put_on_day(&db, "today", today, 1);
        put_on_day(&db, "yesterday", today - 1, 1);
        assert!(db.erased_account_remembered("today"));
        assert!(!db.erased_account_remembered("yesterday"), "a 1-day window remembered yesterday's erase");
        set_window(&db, 30, 100_000);
        put_on_day(&db, "day_29", today - 29, 30);
        put_on_day(&db, "day_30", today - 30, 30);
        assert!(db.erased_account_remembered("day_29"));
        assert!(!db.erased_account_remembered("day_30"), "a 30-day window remembered an erase 30 days old");
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
        put_on_day(&db, "day_minus_9", today - 9, 30);
        put_on_day(&db, "day_minus_5", today - 5, 30);
        put_on_day(&db, "day_minus_2", today - 2, 30);
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
    /// has exactly its three columns (the fingerprint, the day, the window), and the value
    /// depends on this relay's secret (a copy of the database alone cannot be checked against
    /// a list of known keys).
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
        assert_eq!(cols, ["fingerprint", "erased_day", "ttl_days"], "only the fingerprint, the day and the window");
        let fp = db.erased_account_fingerprint(key);
        assert_eq!(fp.len(), 64);
        assert_ne!(fp, fingerprint(&[0u8; 32], key), "another secret gives another fingerprint");
        assert_eq!(fp, fingerprint(&db.erase_secret, &key.to_uppercase()), "case does not change it");
        assert!(db.erase_memory_kept(), "a fresh secret file is kept");
        // The secret is kept: a restart on the same database still recognises the key.
        drop(db);
        let reopened = Storage::open(&path).expect("reopen");
        assert!(reopened.erased_account_remembered(key), "a restart forgot the erase");
    }

    /// Review finding 7, the database's half: a damaged secret file is not overwritten, the
    /// run still opens (with a secret of its own), and the storage SAYS it cannot keep what
    /// it remembers, which /health turns into `erase_memory: this_run_only`.
    ///
    /// Seen red 2026-10-04 with `load_secret` answering true whatever the file held: "a
    /// damaged secret file was not reported".
    #[test]
    fn a_damaged_secret_file_is_reported_and_left_alone() {
        let dir = fresh_dir("damaged_secret");
        std::fs::write(dir.join(KEY_FILE), b"0123456789").unwrap();
        let db = Storage::open(&dir.join("relay.db")).expect("a damaged secret file must not stop the relay");
        assert!(!db.erase_memory_kept(), "a damaged secret file was not reported");
        assert_eq!(std::fs::read(dir.join(KEY_FILE)).unwrap(), b"0123456789", "the damaged file was overwritten");
        db.remember_erased_account("dd44").unwrap();
        assert!(db.erased_account_remembered("dd44"), "this run still remembers with its own secret");
    }

    /// Review finding 12: the name registration and the membership row on the identify path
    /// check the erase in the same step as they write, so an erase recorded after the
    /// identify's own check still wins. Not remembered: written as usual. Remembered: nothing
    /// written. A bot is never refused.
    ///
    /// Seen red 2026-10-04 with both checks taken out (the plain INSERTs): "the name was
    /// registered to a key erased here".
    #[test]
    fn registration_and_membership_refuse_a_key_erased_here() {
        let (db, _) = fresh_db("register");
        assert!(db.register_name_unless_erased("Kept", "a1a1").unwrap(), "an ordinary key was refused");
        assert!(db.join_server_unless_erased("a1a1", "Kept").unwrap(), "an ordinary key did not join");
        assert_eq!(db.name_for_key("a1a1").unwrap().as_deref(), Some("Kept"));
        db.remember_erased_account("e2e2").unwrap();
        assert!(!db.register_name_unless_erased("Gone", "e2e2").unwrap(), "the name was registered to a key erased here");
        assert_eq!(db.name_for_key("e2e2").unwrap(), None, "the name was registered to a key erased here");
        assert!(!db.join_server_unless_erased("e2e2", "Gone").unwrap());
        assert!(!db.is_member("e2e2"), "a member row was made for a key erased here");
        // Once the person chose to come back (their identify forgot the entry), both write.
        db.forget_erased_account("e2e2").unwrap();
        assert!(db.register_name_unless_erased("Gone", "e2e2").unwrap());
        assert!(db.join_server_unless_erased("e2e2", "Gone").unwrap());
        // A bot is never affected, even with an entry put in by hand.
        db.remember_erased_account("bot_x").unwrap();
        assert!(db.register_name_unless_erased("BotX", "bot_x").unwrap(), "a bot was refused");
    }

    /// Review of BUG-135 option 2, second round, finding 8: a link code (which registers the
    /// key under the code's name and gives it the creator's role) checks the erase in the same
    /// step as it writes, like the name and the membership above. Remembered: nothing written
    /// and the code not used up; once the person chose to come back, it links as usual.
    ///
    /// Seen red 2026-10-04 with the step redeeming without the check (the plain
    /// `redeem_link_code` relay.rs called): "assertion `left == right` failed: a key erased here
    /// was linked to a name / left: Linked(\"Owner\") / right: ErasedHere".
    #[test]
    fn a_link_code_writes_nothing_for_a_key_erased_here() {
        let (db, _) = fresh_db("link_code");
        db.register_name("Owner", "0a0a").unwrap();
        db.set_role("0a0a", "verified").unwrap();
        let code = db.create_link_code("Owner", "0a0a").unwrap();
        db.remember_erased_account("e3e3").unwrap();
        assert_eq!(db.redeem_link_code_unless_erased(&code, "e3e3").unwrap(), LinkRedeemed::ErasedHere, "a key erased here was linked to a name");
        assert_eq!(db.name_for_key("e3e3").unwrap(), None, "the name was registered to a key erased here");
        assert_eq!(db.get_role("e3e3").unwrap(), "", "the creator's role was given to a key erased here");
        db.forget_erased_account("e3e3").unwrap();
        assert_eq!(db.redeem_link_code_unless_erased(&code, "e3e3").unwrap(), LinkRedeemed::Linked("Owner".into()), "the refused code was used up");
        assert_eq!(db.get_role("e3e3").unwrap(), "verified");
        assert_eq!(db.redeem_link_code_unless_erased("NOPE-NOPE", "f4f4").unwrap(), LinkRedeemed::NoSuchCode);
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

    /// The sentence a person reads before erasing: one sentence, the real number of days as an
    /// upper bound, what is kept (a one-way fingerprint, not their name or data), that the entry
    /// is then deleted here, and that a copy rides in the backups until each is deleted. The
    /// web says exactly the same (scripts/tests/erase-sign-up-again.test.js).
    ///
    /// Seen red 2026-10-04 with the c0c04fc39 sentence put back (review finding 2: it ended
    /// "after that nothing of it is left", untrue of the backups): "assertion `left == right`
    /// failed / left: \"After the erase this server remembers for 30 days that this account was
    /// erased, ...; after that nothing of it is left.\"".
    #[test]
    fn the_erase_sentence_names_the_days_and_what_is_kept() {
        let s = erase_memory_sentence(30);
        assert_eq!(
            s,
            "After the erase this server remembers for up to 30 days that this account was erased, as a one-way fingerprint that is not your name or your data, so your other devices do not sign you up again by themselves; then the entry is deleted here, and a copy of it in this server's backups lasts until that backup is deleted."
        );
        assert!(erase_memory_sentence(12).contains("for up to 12 days"), "it names the real number of days");
        assert!(erase_memory_sentence(1).contains("for up to 1 day that"));
        for s in [erase_memory_sentence(30), erase_memory_sentence(1)] {
            assert!(s.contains("backups lasts until that backup is deleted"), "the backups are not named: {s}");
            assert!(!s.contains("nothing of it is left"), "the old promise is back: {s}");
            assert_eq!(s.matches(". ").count(), 0, "one sentence: {s}");
            assert!(s.ends_with('.'));
        }
    }
}
