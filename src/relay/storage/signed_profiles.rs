//! The copies of people's profiles that other servers keep (`signed_profiles`): each one the
//! profile as anyone may see it (`core::profile_copy::PublicCopy`), signed by its owner's own app.
//! One row per key, network-wide: the latest signed copy wins on every server. No home server:
//! the signature is the authority. A withdrawal (empty name, every field blank) replaces a copy
//! when its owner goes unlisted.
//!
//! This layer does not check signatures (the relay does, off the async pool, before it stores):
//! it holds the keep rules, so no path can store what no server should keep.

use rusqlite::{params, OptionalExtension};

use super::Storage;
use crate::relay::core::profile_copy::{is_withdrawal, PublicCopy, MAX_AHEAD_MS};

/// The most copies a server sends a peer when their link comes up (the catch-up): the newest.
pub const CATCH_UP_COPIES_MAX: usize = 1_000;

/// A signed profile cached on this server.
#[derive(Debug, Clone)]
pub struct SignedProfileRecord {
    pub public_key: String,
    pub name: String,
    pub bio: String,
    pub avatar_url: String,
    pub banner_url: String,
    pub socials: String,
    pub pronouns: String,
    pub location: String,
    pub website: String,
    pub timestamp: u64,
    pub signature: String,
}

impl SignedProfileRecord {
    /// The copy this row holds, as it was signed.
    pub fn copy(&self) -> PublicCopy {
        PublicCopy {
            name: self.name.clone(),
            bio: self.bio.clone(),
            avatar_url: self.avatar_url.clone(),
            banner_url: self.banner_url.clone(),
            socials: self.socials.clone(),
            pronouns: self.pronouns.clone(),
            location: self.location.clone(),
            website: self.website.clone(),
        }
    }
}

/// The columns every read of a row selects, in `record_of`'s order.
const COLUMNS: &str = "public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature";

fn record_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<SignedProfileRecord> {
    Ok(SignedProfileRecord {
        public_key: row.get(0)?,
        name: row.get(1)?,
        bio: row.get(2)?,
        avatar_url: row.get(3)?,
        banner_url: row.get(4)?,
        socials: row.get(5)?,
        pronouns: row.get(6)?,
        location: row.get(7)?,
        website: row.get(8)?,
        timestamp: row.get::<_, i64>(9)? as u64,
        signature: row.get(10)?,
    })
}

impl Storage {
    /// Keep a signed copy of `public_key`'s profile dated `timestamp`, replacing the whole row
    /// held for that key. Returns whether it was kept. Refused (Ok(false), nothing changed):
    /// - an empty signature: a copy that cannot prove itself is never kept;
    /// - a copy dated more than a day ahead of this server's clock ([`MAX_AHEAD_MS`]): "latest
    ///   wins" has no upper bound otherwise, so a copy dated far ahead would win every later save
    ///   until the clock caught up (the audit of 2026-06-12);
    /// - a copy not newer than the one held;
    /// - a withdrawal when nothing is held: a server that never had a person's copy keeps no
    ///   trace of their going unlisted.
    ///
    /// A withdrawal over a held copy replaces the whole row, so the old fields are gone here.
    /// The caller checks the signature and the limits first (`core::profile_copy`).
    pub fn store_signed_profile(
        &self,
        public_key: &str,
        copy: &PublicCopy,
        timestamp: u64,
        signature: &str,
    ) -> Result<bool, rusqlite::Error> {
        if signature.is_empty() {
            return Ok(false);
        }
        let now_ms = super::now_millis();
        if now_ms > 0 && timestamp > now_ms.saturating_add(MAX_AHEAD_MS) {
            return Ok(false);
        }
        self.with_conn(|conn| {
            let held: Option<i64> = conn
                .query_row(
                    "SELECT timestamp FROM signed_profiles WHERE public_key = ?1",
                    params![public_key],
                    |row| row.get(0),
                )
                .optional()?;
            match held {
                Some(held) if held as u64 >= timestamp => return Ok(false),
                None if is_withdrawal(copy) => return Ok(false),
                _ => {}
            }
            let changed = conn.execute(
                "INSERT INTO signed_profiles (public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(public_key) DO UPDATE SET
                   name = excluded.name,
                   bio = excluded.bio,
                   avatar_url = excluded.avatar_url,
                   banner_url = excluded.banner_url,
                   socials = excluded.socials,
                   pronouns = excluded.pronouns,
                   location = excluded.location,
                   website = excluded.website,
                   timestamp = excluded.timestamp,
                   signature = excluded.signature
                 WHERE excluded.timestamp > signed_profiles.timestamp",
                params![
                    public_key, copy.name, copy.bio, copy.avatar_url, copy.banner_url, copy.socials,
                    copy.pronouns, copy.location, copy.website, timestamp as i64, signature
                ],
            )?;
            Ok(changed > 0)
        })
    }

    /// Get a signed profile by public key.
    pub fn get_signed_profile(&self, public_key: &str) -> Result<Option<SignedProfileRecord>, rusqlite::Error> {
        // Read-only single-row lookup. Read pool.
        self.with_read_conn(|conn| {
            conn.query_row(
                &format!("SELECT {COLUMNS} FROM signed_profiles WHERE public_key = ?1"),
                params![public_key],
                record_of,
            )
            .optional()
        })
    }

    /// The copies this server sends a peer when their link comes up (the catch-up, BUG-191): the
    /// copies of keys with an account HERE (`registered_names`), withdrawals included, newest
    /// first, at most [`CATCH_UP_COPIES_MAX`]. A copy this server only received from another
    /// server is that server's to send. So a server that was down when someone made a field
    /// friends-only or went unlisted learns it when the link returns.
    pub fn copies_of_accounts_here(&self) -> Result<Vec<SignedProfileRecord>, rusqlite::Error> {
        // Read-only: SELECT + query_map. Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLUMNS} FROM signed_profiles sp
                 WHERE sp.signature != ''
                   AND EXISTS (SELECT 1 FROM registered_names rn WHERE rn.public_key = sp.public_key)
                 ORDER BY sp.timestamp DESC LIMIT ?1"
            ))?;
            let rows = stmt.query_map(params![CATCH_UP_COPIES_MAX as i64], record_of)?;
            rows.collect()
        })
    }

    /// Count total signed profiles cached on this server.
    pub fn count_signed_profiles(&self) -> Result<i64, rusqlite::Error> {
        // Read-only COUNT. Read pool.
        self.with_read_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM signed_profiles", [], |row| row.get(0))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::profile_copy::withdrawal;

    fn copy_of(name: &str, location: &str) -> PublicCopy {
        PublicCopy {
            name: name.to_string(),
            bio: "hello".to_string(),
            socials: "{}".to_string(),
            location: location.to_string(),
            ..PublicCopy::default()
        }
    }

    /// A COPY WITHOUT A SIGNATURE IS NEVER KEPT, neither as a first copy nor over a held one:
    /// before BUG-191 the save stored one with an empty signature, which no server could check.
    ///
    /// Seen red 2026-10-12 with the empty-signature rule removed from `store_signed_profile`: "a
    /// first copy without a signature was kept" failed.
    #[test]
    fn a_copy_without_a_signature_is_never_kept() {
        let db = Storage::open_temp("sp_unsigned");
        let now = super::super::now_millis();
        assert!(!db.store_signed_profile("ann_key", &copy_of("Ann", "Lisbon"), now, "").unwrap(), "a first copy without a signature was kept");
        assert!(db.get_signed_profile("ann_key").unwrap().is_none(), "and nothing is held");
        assert!(db.store_signed_profile("ann_key", &copy_of("Ann", "Lisbon"), now, "sig").unwrap());
        assert!(!db.store_signed_profile("ann_key", &copy_of("Ann", ""), now + 1, "").unwrap(), "nor over a held one");
        assert_eq!(db.get_signed_profile("ann_key").unwrap().unwrap().location, "Lisbon", "the signed one stays");
    }

    /// GOING UNLISTED: a withdrawal over a held copy replaces the WHOLE row, so the old fields are
    /// gone here; where nothing is held, a withdrawal is not kept, so a server that never had the
    /// person's copy keeps no trace that they went unlisted. A later listed copy replaces the
    /// withdrawal.
    ///
    /// Seen red 2026-10-12 with the "nothing held" rule removed from `store_signed_profile`: "a
    /// withdrawal for a key never held left a trace" failed.
    #[test]
    fn a_withdrawal_replaces_a_held_copy_and_leaves_no_trace_otherwise() {
        let db = Storage::open_temp("sp_withdrawal");
        let now = super::super::now_millis();
        assert!(!db.store_signed_profile("bo_key", &withdrawal(), now, "sig").unwrap(), "a withdrawal for a key never held left a trace");
        assert!(db.get_signed_profile("bo_key").unwrap().is_none(), "nothing is held for Bo");

        assert!(db.store_signed_profile("ann_key", &copy_of("Ann", "Lisbon"), now, "sig1").unwrap());
        assert!(db.store_signed_profile("ann_key", &withdrawal(), now + 1, "sig2").unwrap(), "a withdrawal over a held copy is kept");
        let held = db.get_signed_profile("ann_key").unwrap().expect("the withdrawal is held");
        assert_eq!(held.copy(), withdrawal(), "the whole row is replaced: {held:?}");
        assert_eq!((held.timestamp, held.signature.as_str()), (now + 1, "sig2"));

        assert!(db.store_signed_profile("ann_key", &copy_of("Ann", ""), now + 2, "sig3").unwrap(), "listed again later");
        assert_eq!(db.get_signed_profile("ann_key").unwrap().unwrap().name, "Ann");
    }

    /// A copy dated more than a day ahead of this server's clock is not kept (it would win every
    /// later save until the clock caught up); one within the day is. A copy not newer than the one
    /// held (older or the same time) is not kept either, and changes nothing.
    ///
    /// Seen red 2026-10-12 with the day-ahead rule removed from `store_signed_profile`: "a copy a
    /// day and a minute ahead was kept" failed. (Written `held > timestamp`, the same-time case
    /// stays green: the upsert's own `WHERE excluded.timestamp > signed_profiles.timestamp` holds it
    /// too, checked that day.)
    #[test]
    fn a_copy_more_than_a_day_ahead_or_not_newer_is_not_kept() {
        let db = Storage::open_temp("sp_times");
        let now = super::super::now_millis();
        assert!(!db.store_signed_profile("ann_key", &copy_of("Ann", "far"), now + MAX_AHEAD_MS + 60_000, "sig").unwrap(), "a copy a day and a minute ahead was kept");
        assert!(db.get_signed_profile("ann_key").unwrap().is_none());
        assert!(db.store_signed_profile("ann_key", &copy_of("Ann", "Lisbon"), now + MAX_AHEAD_MS - 60_000, "sig").unwrap(), "within the day is kept");
        let at = now + MAX_AHEAD_MS - 60_000;
        assert!(!db.store_signed_profile("ann_key", &copy_of("Ann", "Porto"), at, "sig2").unwrap(), "a copy with the same time as the one held was kept");
        assert!(!db.store_signed_profile("ann_key", &copy_of("Ann", "Porto"), at - 1, "sig2").unwrap(), "an older copy was kept");
        let held = db.get_signed_profile("ann_key").unwrap().unwrap();
        assert_eq!((held.location.as_str(), held.signature.as_str()), ("Lisbon", "sig"), "nothing changed");
    }

    /// THE CATCH-UP sends the copies of this server's own accounts (any key under a registered
    /// name, a linked device's included), withdrawals among them, newest first; never a copy that
    /// only arrived from another server for someone with no account here.
    ///
    /// Seen red 2026-10-12 with the account filter removed from `copies_of_accounts_here`: "the
    /// catch-up holds this server's accounts only" failed (left ["far_key", "ann_phone", "bo_key",
    /// "ann_key"], right ["ann_phone", "bo_key", "ann_key"]).
    #[test]
    fn the_catch_up_holds_only_this_servers_accounts_newest_first() {
        let db = Storage::open_temp("sp_catch_up");
        db.register_name("Ann", "ann_key").unwrap();
        db.register_name("Ann", "ann_phone").unwrap();
        db.register_name("Bo", "bo_key").unwrap();
        let now = super::super::now_millis();
        db.store_signed_profile("ann_key", &copy_of("Ann", "Lisbon"), now - 4_000, "sig").unwrap();
        db.store_signed_profile("bo_key", &copy_of("Bo", ""), now - 3_000, "sig").unwrap();
        db.store_signed_profile("bo_key", &withdrawal(), now - 2_500, "sig").unwrap();
        db.store_signed_profile("ann_phone", &copy_of("Ann", ""), now - 2_000, "sig").unwrap();
        db.store_signed_profile("far_key", &copy_of("Far", "Porto"), now - 1_000, "sig").unwrap();
        let keys: Vec<String> = db.copies_of_accounts_here().unwrap().into_iter().map(|r| r.public_key).collect();
        assert_eq!(keys, ["ann_phone", "bo_key", "ann_key"], "the catch-up holds this server's accounts only");
        let bo = db.copies_of_accounts_here().unwrap().into_iter().find(|r| r.public_key == "bo_key").unwrap();
        assert!(is_withdrawal(&bo.copy()), "a withdrawal is sent too");
    }

    /// THE SAVE'S OLD UNSIGNED COPIES ARE DROPPED AT OPEN (BUG-046 shape: a database made before
    /// this change, opened by this build). They were never valid copies, and the ones from before
    /// 2026-10-11 hold fields kept for friends. A signed row stays.
    ///
    /// Seen red 2026-10-12 without the open-time DELETE in storage/mod.rs: "the old unsigned copy
    /// is gone" failed.
    #[test]
    fn unsigned_copies_from_before_are_dropped_at_open() {
        let path = crate::test_temp::db("sp_old_unsigned");
        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE signed_profiles (
                    public_key  TEXT PRIMARY KEY,
                    name        TEXT NOT NULL DEFAULT '',
                    bio         TEXT NOT NULL DEFAULT '',
                    avatar_url  TEXT NOT NULL DEFAULT '',
                    banner_url  TEXT NOT NULL DEFAULT '',
                    socials     TEXT NOT NULL DEFAULT '{}',
                    pronouns    TEXT NOT NULL DEFAULT '',
                    location    TEXT NOT NULL DEFAULT '',
                    website     TEXT NOT NULL DEFAULT '',
                    timestamp   INTEGER NOT NULL DEFAULT 0,
                    signature   TEXT NOT NULL DEFAULT ''
                );
                INSERT INTO signed_profiles (public_key, name, bio, location, timestamp, signature)
                    VALUES ('ann_key', 'Ann', 'old', 'Lisbon', 1, '');
                INSERT INTO signed_profiles (public_key, name, bio, timestamp, signature)
                    VALUES ('far_key', 'Far', 'from a peer', 2, 'sig');",
            )
            .expect("seed the old table");
        }
        let db = Storage::open(&path).expect("startup survives a database from before");
        assert!(db.get_signed_profile("ann_key").unwrap().is_none(), "the old unsigned copy is gone");
        assert_eq!(db.get_signed_profile("far_key").unwrap().map(|r| r.bio).as_deref(), Some("from a peer"), "a signed copy stays");
    }
}
