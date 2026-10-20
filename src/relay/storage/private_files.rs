//! Files from private conversations (2026-10-11; docs/design/files-showcase.md, follow-up B5, the
//! operator's answers 9 and 10). A file sent in a DM or a P2P group is encrypted on the sender's
//! device and uploaded with `?encrypted=1`; the relay stores the sealed blob as `<name>.enc`
//! (upload_rules.rs `FileKind::stored_ext`, the only way a stored name ends in `.enc`) on the chat
//! shelf. It can never be shown publicly, and the relay cannot read it.
//!
//! - They LEAVE after the DM mailbox lifetime (`dm_mailbox_ttl_days`, Server Settings, default 30),
//!   counted from when each was uploaded: the expiry pass deletes the row and the file
//!   ([`Storage::expire_private_files_in`]). Long-term history lives on the people's own devices,
//!   as for the messages themselves.
//! - Each person may HOLD up to their role's `max_private_held_mb` of them at once (0: no limit),
//!   checked with the room check under the same lock (relay/files.rs `store_upload`).
//! - They are OUT of the keep-N FIFO (storage/uploads.rs `record_upload`): a person's private
//!   files would otherwise push out their own room posts, and their room posts would delete the
//!   files their DMs point at. The two limits above hold them instead.

use super::Storage;
use rusqlite::params;

/// How every private file's stored name ends: an encrypted upload is stored as `.enc`.
pub const PRIVATE_SUFFIX: &str = ".enc";
/// One day, in Unix milliseconds.
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Whether a stored name is a private conversation's file.
pub fn is_private_file(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(PRIVATE_SUFFIX)
}

impl Storage {
    /// How many bytes of private files `key` holds now, and when the oldest was uploaded (Unix
    /// ms; None when they hold none).
    pub fn private_files_held(&self, key: &str) -> Result<(i64, Option<i64>), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COALESCE(SUM(size_bytes), 0), MIN(uploaded_at) FROM user_uploads
                 WHERE public_key = ?1 AND LOWER(filename) LIKE '%.enc'",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
    }

    /// The expiry pass's share (storage/expiry.rs): every private file uploaded more than
    /// `ttl_days` days before `now_ms` is deleted, row and file, from `folders`. How many went.
    pub fn expire_private_files_in(&self, folders: super::files::Folders, ttl_days: i64, now_ms: i64) -> Result<usize, rusqlite::Error> {
        // The DM mailbox's own rule (dms.rs `mailbox_expire`): what arrived on day D stays until
        // day D + ttl has ended. Counted to the millisecond, a file could leave up to a day before
        // the DM that points to it (persisted-format review, 2026-10-11).
        let cutoff = (now_ms.div_euclid(DAY_MS) - ttl_days.max(1)) * DAY_MS;
        let names: Vec<String> = self.with_conn(|conn| {
            let names: Vec<String> = {
                let mut stmt = conn.prepare("SELECT filename FROM user_uploads WHERE LOWER(filename) LIKE '%.enc' AND uploaded_at < ?1")?;
                let names = stmt.query_map(params![cutoff], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
                names
            };
            for name in &names {
                conn.execute("DELETE FROM user_uploads WHERE filename = ?1", params![name])?;
            }
            Ok::<_, rusqlite::Error>(names)
        })?;
        for name in &names {
            folders.delete(name);
        }
        Ok(names.len())
    }
}

/// When a private file uploaded at `uploaded_at` leaves, on a server keeping them `ttl_days`: the
/// start of the day after its upload day plus the lifetime, the moment its DM leaves too.
pub fn leaves_at(uploaded_at: i64, ttl_days: i64) -> i64 {
    (uploaded_at.div_euclid(DAY_MS) + ttl_days.max(1) + 1) * DAY_MS
}
