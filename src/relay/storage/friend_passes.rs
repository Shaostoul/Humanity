//! Withdrawn friendship passes (2026-10-09, docs/design/blocking-and-safe-mode.md 10b).
//!
//! A friendship pass (relay/core/pq_crypto.rs, v2) is held by the friend and checked by the
//! relay without a friends list. What the relay DOES keep is the list of passes their issuers
//! have taken back: `friend_cert_revocations (issuer_fingerprint, serial, revoked_day)`. A pass
//! carries a random serial, so one withdrawal names exactly one pass, and the issuer is the key
//! of the signed-in socket that sent `cert_revoke` (handlers/friend_passes.rs), so nobody can
//! withdraw a pass they did not give.
//!
//! NOT THE KEY: A KEYED FINGERPRINT OF IT (the coordinator's review, 2026-10-09). Public keys
//! are public, so a row holding one would tell whoever holds a copy of the database (a backup,
//! a breach, a subpoena) that this person took back friendships, and when. The row holds
//! BLAKE3 keyed with this relay's machine-local secret (the one `erased_accounts` uses,
//! `data/erased-accounts.key` beside the live database, never in the backups) over
//! `hum/withdrawn-pass/v1\n` + the lower-cased key. The different first line keeps the two uses
//! apart: the same key gives unrelated values in the two tables, and nobody can match one
//! against the other. One secret means one file to carry when a server moves, not two.
//! With the database alone a row names nobody; the serial means nothing without the pass,
//! which only the two friends hold. A relay changed to log every pass presented could match a
//! serial to its holder, but the same changed relay could log who writes to whom today, so
//! this adds no new kind of exposure (section 5.3 of the design).
//!
//! KEPT PAST AN ACCOUNT ERASE. A pass has no end date (the operator, 2026-10-09), and the rows
//! no longer name anybody, so an erase leaves them: otherwise someone who erased their account
//! and signed up again with the same key (the same recovery phrase) would bring back every pass
//! they had withdrawn. The person's own export still lists them, by computing their fingerprint.
//!
//! WHEN THE SECRET CANNOT BE KEPT (a damaged or unwritable `erased-accounts.key`, which
//! `erased_accounts::load_secret` reports as `erase_secret_kept == false`, and /health as
//! `erase_memory: this_run_only`): this run's own secret would make every row written before it
//! unreadable, and every row written now unreadable after the next start. So the relay fails
//! safe: it records no withdrawal (and does not confirm one, so the issuer's client keeps
//! resending until a run can keep it), and counts every pass as withdrawn, which puts friends in
//! the stranger's lane (their messages still go, within the daily budget). A MISSING file is
//! created afresh, as for erased accounts; then the old rows never match again, which brings
//! withdrawn passes back. That is why CLAUDE.md says to carry the key file when moving a server.

use rusqlite::params;

use super::Storage;

/// The most withdrawals one key may have on record (a few thousand friendships changed or ended
/// over a lifetime is far beyond normal use). A ceiling, so a script with a key cannot fill the
/// server's disk with made-up serials.
pub const FRIEND_CERT_WITHDRAWALS_MAX: usize = 4_096;

/// Domain separation for the fingerprint: never the same value as `erased_accounts`' under the
/// same secret.
const WITHDRAWAL_FINGERPRINT_CONTEXT: &[u8] = b"hum/withdrawn-pass/v1\n";

/// The one-way fingerprint a withdrawal by `issuer_key` is kept under, with this relay's
/// `secret`: 64 hex characters. Trimmed and lower-cased first, so one identity written in
/// upper-case hex is the same issuer.
pub fn withdrawal_fingerprint(secret: &[u8; 32], issuer_key: &str) -> String {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(WITHDRAWAL_FINGERPRINT_CONTEXT);
    h.update(issuer_key.trim().to_ascii_lowercase().as_bytes());
    h.finalize().to_hex().to_string()
}

/// What `withdraw_friend_cert` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendCertWithdrawal {
    /// Recorded now: from this moment the pass counts as no pass.
    Recorded,
    /// It was already withdrawn (a resend, or another of the issuer's devices got there first).
    AlreadyWithdrawn,
    /// Not recorded: this key already has [`FRIEND_CERT_WITHDRAWALS_MAX`] on record.
    TooMany,
    /// Not recorded: this run cannot keep its fingerprint secret (see the module note), so a row
    /// written now would stop matching at the next start. The issuer's client resends it later.
    CannotKeep,
}

impl Storage {
    /// The fingerprint `issuer_key`'s withdrawals are kept under on this relay.
    pub fn friend_pass_withdrawal_fingerprint(&self, issuer_key: &str) -> String {
        withdrawal_fingerprint(&self.erase_secret, issuer_key)
    }

    /// Record that `issuer_key` has taken back the pass named `serial`, on `day` (Unix days).
    /// Idempotent: withdrawing the same pass twice is [`FriendCertWithdrawal::AlreadyWithdrawn`].
    pub fn withdraw_friend_cert(&self, issuer_key: &str, serial: &str, day: i64) -> Result<FriendCertWithdrawal, rusqlite::Error> {
        if !self.erase_secret_kept {
            return Ok(FriendCertWithdrawal::CannotKeep);
        }
        let fp = self.friend_pass_withdrawal_fingerprint(issuer_key);
        self.with_conn(|conn| {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM friend_cert_revocations WHERE issuer_fingerprint = ?1 AND serial = ?2)",
                params![fp, serial],
                |r| r.get(0),
            )?;
            if exists {
                return Ok(FriendCertWithdrawal::AlreadyWithdrawn);
            }
            let have: i64 = conn.query_row(
                "SELECT COUNT(*) FROM friend_cert_revocations WHERE issuer_fingerprint = ?1",
                params![fp],
                |r| r.get(0),
            )?;
            if have as usize >= FRIEND_CERT_WITHDRAWALS_MAX {
                return Ok(FriendCertWithdrawal::TooMany);
            }
            conn.execute(
                "INSERT INTO friend_cert_revocations (issuer_fingerprint, serial, revoked_day) VALUES (?1, ?2, ?3)",
                params![fp, serial, day],
            )?;
            Ok(FriendCertWithdrawal::Recorded)
        })
    }

    /// Has `issuer_key` taken back the pass named `serial`? Answers YES when it cannot know: a
    /// run that cannot keep its fingerprint secret (see the module note), or a database that
    /// cannot be read. The sender then falls back to the stranger's lane (their message still
    /// goes, within the daily budget), which is the safe side of not knowing whether consent
    /// stands.
    pub fn friend_cert_withdrawn(&self, issuer_key: &str, serial: &str) -> bool {
        if !self.erase_secret_kept {
            return true;
        }
        let fp = self.friend_pass_withdrawal_fingerprint(issuer_key);
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM friend_cert_revocations WHERE issuer_fingerprint = ?1 AND serial = ?2)",
                params![fp, serial],
                |r| r.get::<_, bool>(0),
            )
        })
        .unwrap_or_else(|e| {
            tracing::error!("friend passes: could not read the withdrawals: {e}");
            true
        })
    }

    /// How many withdrawals `issuer_key` has on record.
    pub fn friend_cert_withdrawals_of(&self, issuer_key: &str) -> usize {
        let fp = self.friend_pass_withdrawal_fingerprint(issuer_key);
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM friend_cert_revocations WHERE issuer_fingerprint = ?1",
                params![fp],
                |r| r.get::<_, i64>(0),
            )
        })
        .unwrap_or(0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own, for a test that opens its database more than once or puts a secret
    /// file beside it first (the guard outlives the storages declared after it).
    fn fresh_dir(tag: &str) -> crate::test_temp::TempPath {
        crate::test_temp::dir(&format!("friend_passes_{tag}"))
    }

    /// A withdrawal is recorded once, read back for its issuer only, and a resend is harmless.
    /// The table holds a keyed fingerprint, never the key, and not the value the erased-accounts
    /// table would hold for the same key under the same secret; a restart on the same database
    /// still reads it. One key cannot fill the table: past the ceiling nothing more is recorded.
    /// An account erase LEAVES the rows (they name nobody, and keeping them stops a sign-up with
    /// the same key from bringing back passes it withdrew), is not reported as unfinished
    /// because of them, and the person's own export still lists them.
    /// Seen red 2026-10-09 four ways: with the erase deleting the rows again under its own
    /// receipt label (the receipt assertion, listing `friend_pass_withdrawals`) and under
    /// another label ("an erase keeps the withdrawal"); with the plain lower-cased key stored
    /// instead of its fingerprint ("a stored value holds the key"); and, in the first version of
    /// this test, with the issuer left out of `friend_cert_withdrawn`'s WHERE clause ("another
    /// key's pass with the same serial is untouched").
    #[test]
    fn withdrawals_are_fingerprinted_capped_and_outlive_an_erase() {
        let dir = fresh_dir("main");
        let path = dir.join("relay.db");
        let db = Storage::open(&path).expect("open");
        let ada = "4F2A9C11D0E7B35A6C88AA01F4E2D9B7";
        let serial = "00112233445566778899aabbccddeeff";
        assert!(!db.friend_cert_withdrawn(ada, serial));
        assert_eq!(db.withdraw_friend_cert(ada, serial, 20_000).unwrap(), FriendCertWithdrawal::Recorded);
        assert!(db.friend_cert_withdrawn(ada, serial));
        assert!(db.friend_cert_withdrawn(&ada.to_lowercase(), serial), "the same key in lower case is the same issuer");
        assert!(!db.friend_cert_withdrawn("bo_key", serial), "another key's pass with the same serial is untouched");
        assert_eq!(db.withdraw_friend_cert(ada, serial, 20_001).unwrap(), FriendCertWithdrawal::AlreadyWithdrawn);
        assert_eq!(db.friend_cert_withdrawals_of(ada), 1);

        // What is stored: the fingerprint, the serial and the day. Nothing that holds the key.
        let (cols, stored): (Vec<String>, Vec<String>) = db
            .with_read_conn(|c| {
                let mut st = c.prepare("PRAGMA table_info(friend_cert_revocations)")?;
                let cols: Vec<String> = st.query_map([], |r| r.get::<_, String>(1))?.filter_map(|r| r.ok()).collect();
                let mut st = c.prepare("SELECT issuer_fingerprint FROM friend_cert_revocations")?;
                let fps: Vec<String> = st.query_map([], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
                Ok((cols, fps))
            })
            .unwrap();
        assert_eq!(cols, ["issuer_fingerprint", "serial", "revoked_day"]);
        let lower = ada.to_lowercase();
        assert!(stored.iter().all(|v| !v.to_lowercase().contains(&lower[..8]) && v.len() == 64), "a stored value holds the key: {stored:?}");
        assert_eq!(stored, vec![db.friend_pass_withdrawal_fingerprint(ada)]);
        assert_ne!(stored[0], db.erased_account_fingerprint(ada), "not the erased-accounts value for the same key");
        assert_ne!(stored[0], withdrawal_fingerprint(&[0u8; 32], ada), "another secret gives another fingerprint");

        // The ceiling, for one key only. All but one filled in a single transaction (thousands
        // of separate writes would only make the test slow), the last through the real door.
        let spam_fp = db.friend_pass_withdrawal_fingerprint("spam_key");
        db.with_conn_mut(|conn| {
            let tx = conn.transaction().unwrap();
            for i in 1..FRIEND_CERT_WITHDRAWALS_MAX {
                tx.execute(
                    "INSERT INTO friend_cert_revocations (issuer_fingerprint, serial, revoked_day) VALUES (?1, ?2, 20000)",
                    params![spam_fp, format!("{i:032x}")],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        });
        assert_eq!(db.withdraw_friend_cert("spam_key", &format!("{:032x}", 0xffff_ffffu32), 20_000).unwrap(), FriendCertWithdrawal::Recorded);
        assert_eq!(db.withdraw_friend_cert("spam_key", &format!("{:032x}", 0xffff_fffeu32), 20_000).unwrap(), FriendCertWithdrawal::TooMany);
        assert_eq!(db.friend_cert_withdrawals_of("spam_key"), FRIEND_CERT_WITHDRAWALS_MAX);
        assert_eq!(db.withdraw_friend_cert(ada, "ffeeddccbbaa99887766554433221100", 20_000).unwrap(), FriendCertWithdrawal::Recorded, "the ceiling is per key");

        // An erase leaves Ada's withdrawals: they still count, the erase is not reported as
        // unfinished because of them, and her export lists them.
        db.register_name("Ada", ada).unwrap();
        let export = db.export_account(ada, "Ada");
        assert_eq!(export["friend_pass_withdrawals"].as_array().map(|a| a.len()), Some(2), "her export lists her withdrawals");
        let receipt = db.delete_account(ada, "Ada");
        assert!(!receipt.iter().any(|(label, _)| label.contains("friend_pass")), "{receipt:?}");
        assert!(db.friend_cert_withdrawn(ada, serial), "an erase keeps the withdrawal");
        assert_eq!(db.friend_cert_withdrawals_of(ada), 2);
        assert!(!db.erase_left_rows(ada), "the kept withdrawals do not make the erase unfinished");
        assert_eq!(db.export_account(ada, "Ada")["friend_pass_withdrawals"].as_array().map(|a| a.len()), Some(2));

        // A restart on the same database (and secret file) still reads them.
        drop(db);
        let reopened = Storage::open(&path).expect("reopen");
        assert!(reopened.friend_cert_withdrawn(ada, serial), "a restart forgot the withdrawal");
    }

    /// A damaged secret file (the erased-accounts module's "this run only" case): nothing can be
    /// recorded so that it lasts, and nothing written before can be read, so no withdrawal is
    /// recorded or confirmed, and every pass counts as withdrawn this run.
    /// Seen red 2026-10-09 with the `erase_secret_kept` check taken out of
    /// `friend_cert_withdrawn`: "every pass counts as withdrawn when the secret cannot be kept".
    #[test]
    fn a_damaged_secret_counts_every_pass_as_withdrawn_and_records_nothing() {
        let dir = fresh_dir("damaged");
        std::fs::write(dir.join(super::super::erased_accounts::KEY_FILE), b"0123456789").unwrap();
        let db = Storage::open(&dir.join("relay.db")).expect("a damaged secret file must not stop the relay");
        assert!(!db.erase_memory_kept(), "precondition: this run cannot keep its secret");
        let serial = "00112233445566778899aabbccddeeff";
        assert!(db.friend_cert_withdrawn("ada_key", serial), "every pass counts as withdrawn when the secret cannot be kept");
        assert_eq!(db.withdraw_friend_cert("ada_key", serial, 20_000).unwrap(), FriendCertWithdrawal::CannotKeep);
        assert_eq!(db.friend_cert_withdrawals_of("ada_key"), 0, "nothing recorded");
    }
}
