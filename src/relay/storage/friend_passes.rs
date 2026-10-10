//! Withdrawn friendship passes (2026-10-09, docs/design/blocking-and-safe-mode.md 10b).
//!
//! A friendship pass (relay/core/pq_crypto.rs, v2) is held by the friend and checked by the
//! relay without a friends list. What the relay DOES keep is the list of passes their issuers
//! have taken back: `friend_cert_revocations (issuer_key, serial, revoked_day)`. A pass carries
//! a random serial, so one withdrawal names exactly one pass, and the issuer is the key of the
//! signed-in socket that sent `cert_revoke` (handlers/friend_passes.rs), so nobody can withdraw
//! a pass they did not give.
//!
//! What a copy of this table shows: that a key took back some passes on some days, and the
//! random serials. Not who they were given to: the serial means nothing without the pass, which
//! only the two friends hold. A relay changed to log every pass presented could match a serial
//! to its holder, but the same changed relay could log who writes to whom today, so this adds
//! no new kind of exposure (section 5.3 of the design).
//!
//! No end date: a pass has none (the operator, 2026-10-09), so a row is kept until the issuer
//! erases their account (storage/account.rs `delete_account`).

use rusqlite::params;

use super::Storage;

/// The most withdrawals one key may have on record (a few thousand friendships changed or ended
/// over a lifetime is far beyond normal use). A ceiling, so a script with a key cannot fill the
/// server's disk with made-up serials.
pub const FRIEND_CERT_WITHDRAWALS_MAX: usize = 4_096;

/// What `withdraw_friend_cert` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendCertWithdrawal {
    /// Recorded now: from this moment the pass counts as no pass.
    Recorded,
    /// It was already withdrawn (a resend, or another of the issuer's devices got there first).
    AlreadyWithdrawn,
    /// Not recorded: this key already has [`FRIEND_CERT_WITHDRAWALS_MAX`] on record.
    TooMany,
}

impl Storage {
    /// Record that `issuer_key` has taken back the pass named `serial`, on `day` (Unix days).
    /// Idempotent: withdrawing the same pass twice is [`FriendCertWithdrawal::AlreadyWithdrawn`].
    pub fn withdraw_friend_cert(&self, issuer_key: &str, serial: &str, day: i64) -> Result<FriendCertWithdrawal, rusqlite::Error> {
        self.with_conn(|conn| {
            let have: i64 = conn.query_row(
                "SELECT COUNT(*) FROM friend_cert_revocations WHERE issuer_key = ?1",
                params![issuer_key],
                |r| r.get(0),
            )?;
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM friend_cert_revocations WHERE issuer_key = ?1 AND serial = ?2)",
                params![issuer_key, serial],
                |r| r.get(0),
            )?;
            if exists {
                return Ok(FriendCertWithdrawal::AlreadyWithdrawn);
            }
            if have as usize >= FRIEND_CERT_WITHDRAWALS_MAX {
                return Ok(FriendCertWithdrawal::TooMany);
            }
            conn.execute(
                "INSERT INTO friend_cert_revocations (issuer_key, serial, revoked_day) VALUES (?1, ?2, ?3)",
                params![issuer_key, serial, day],
            )?;
            Ok(FriendCertWithdrawal::Recorded)
        })
    }

    /// Has `issuer_key` taken back the pass named `serial`? A database that cannot be read
    /// answers YES: the sender then falls back to the stranger's lane (their message still goes,
    /// within the daily budget), which is the safe side of not knowing whether consent stands.
    pub fn friend_cert_withdrawn(&self, issuer_key: &str, serial: &str) -> bool {
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM friend_cert_revocations WHERE issuer_key = ?1 AND serial = ?2)",
                params![issuer_key, serial],
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
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM friend_cert_revocations WHERE issuer_key = ?1",
                params![issuer_key],
                |r| r.get::<_, i64>(0),
            )
        })
        .unwrap_or(0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A withdrawal is recorded once, read back for its issuer only, and a resend is harmless.
    /// One key cannot fill the table: past the ceiling nothing more is recorded. An account
    /// erase takes the issuer's rows with it (the row's only promised end), and nobody else's.
    /// Seen red 2026-10-09 with the issuer left out of `friend_cert_withdrawn`'s WHERE clause:
    /// "another key's pass with the same serial is untouched", left true.
    #[test]
    fn withdrawals_are_per_issuer_capped_and_go_with_the_account() {
        let db = Storage::open_temp("friend_passes");
        let serial = "00112233445566778899aabbccddeeff";
        assert!(!db.friend_cert_withdrawn("ada_key", serial));
        assert_eq!(db.withdraw_friend_cert("ada_key", serial, 20_000).unwrap(), FriendCertWithdrawal::Recorded);
        assert!(db.friend_cert_withdrawn("ada_key", serial));
        assert!(!db.friend_cert_withdrawn("bo_key", serial), "another key's pass with the same serial is untouched");
        assert_eq!(db.withdraw_friend_cert("ada_key", serial, 20_001).unwrap(), FriendCertWithdrawal::AlreadyWithdrawn);
        assert_eq!(db.friend_cert_withdrawals_of("ada_key"), 1);

        // The ceiling, for one key only. All but one filled in a single transaction (thousands
        // of separate writes would only make the test slow), the last through the real door.
        db.with_conn_mut(|conn| {
            let tx = conn.transaction().unwrap();
            for i in 1..FRIEND_CERT_WITHDRAWALS_MAX {
                tx.execute(
                    "INSERT INTO friend_cert_revocations (issuer_key, serial, revoked_day) VALUES ('spam_key', ?1, 20000)",
                    params![format!("{i:032x}")],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        });
        assert_eq!(db.friend_cert_withdrawals_of("spam_key"), FRIEND_CERT_WITHDRAWALS_MAX - 1);
        assert_eq!(db.withdraw_friend_cert("spam_key", &format!("{:032x}", 0xffff_ffffu32), 20_000).unwrap(), FriendCertWithdrawal::Recorded);
        assert_eq!(db.withdraw_friend_cert("spam_key", &format!("{:032x}", 0xffff_fffeu32), 20_000).unwrap(), FriendCertWithdrawal::TooMany);
        assert_eq!(db.friend_cert_withdrawals_of("spam_key"), FRIEND_CERT_WITHDRAWALS_MAX);
        assert_eq!(db.withdraw_friend_cert("ada_key", "ffeeddccbbaa99887766554433221100", 20_000).unwrap(), FriendCertWithdrawal::Recorded, "the ceiling is per key");

        // An erase takes Ada's withdrawals and leaves the rest.
        db.register_name("Ada", "ada_key").unwrap();
        let receipt = db.delete_account("ada_key", "Ada");
        assert!(receipt.iter().any(|(label, n)| label == "friend_pass_withdrawals" && *n == 2), "{receipt:?}");
        assert_eq!(db.friend_cert_withdrawals_of("ada_key"), 0);
        assert!(!db.friend_cert_withdrawn("ada_key", serial));
        assert_eq!(db.friend_cert_withdrawals_of("spam_key"), FRIEND_CERT_WITHDRAWALS_MAX);
    }
}
