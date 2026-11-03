//! What the website's front door reads (2026-11-02, docs/design/front-door.md, src/relay/front_door.rs):
//! the release posts in #announcements, newest first, and the releases whose signatures this
//! server checked itself.
//!
//! `release_signatures` holds one row per release version: the manifest's SHA-256, the manifest and
//! its signature file exactly as they were sent (so anyone can be shown what was checked), and when
//! the check passed. A row is written only after both signatures verified against the project's
//! compiled-in release keys (`humanity_protocol::release_signature::check_signed_release`); this
//! layer takes a `SignedRelease`, which only that check makes, so no path can store an unchecked
//! pair. Nothing in it is about a person, so it is in no account export or erase.

use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};

use super::Storage;
use humanity_protocol::release_signature::SignedRelease;

/// One post read for the feed: its row id (the page cursor), who posted it, its words, its time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePost {
    pub id: i64,
    pub from_key: String,
    pub content: String,
    pub timestamp: u64,
}

/// What storing a checked release did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureKept {
    /// The first checked pair for this version.
    New,
    /// The same manifest was already held: nothing changed (the first check's time stays).
    Same,
    /// A different manifest for the same version (the release was signed again, for example
    /// after a binary was rebuilt): the newer pair replaced the older.
    Replaced,
}

impl SignatureKept {
    pub fn word(self) -> &'static str {
        match self {
            SignatureKept::New => "new",
            SignatureKept::Same => "same",
            SignatureKept::Replaced => "replaced",
        }
    }
}

impl Storage {
    /// The newest chat posts in `channel`, before row `before` when given, at most `limit`, newest
    /// first: one read down `idx_messages_channel` (`messages(channel_id, id)`). Every sender's posts
    /// come back; the front door (relay/front_door.rs `cards_from`) keeps only the ones it may show,
    /// so a short answer here means the channel has no older posts.
    pub fn release_posts(&self, channel: &str, before: Option<i64>, limit: usize) -> Result<Vec<ReleasePost>, rusqlite::Error> {
        // Read-only SELECT. Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, COALESCE(from_key, ''), COALESCE(content, ''), timestamp FROM messages
                 WHERE channel_id = ?1 AND id < ?2 AND msg_type = 'chat'
                 ORDER BY id DESC LIMIT ?3",
            )?;
            let before = before.unwrap_or(i64::MAX);
            let rows = stmt
                .query_map(params![channel, before, limit as i64], |row| {
                    Ok(ReleasePost {
                        id: row.get(0)?,
                        from_key: row.get(1)?,
                        content: row.get(2)?,
                        timestamp: row.get::<_, i64>(3)?.max(0) as u64,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
    }

    /// Keep a release whose signatures were checked (`check_signed_release` made `checked`):
    /// `manifest` and `signature` are the two files' text exactly as sent.
    pub fn store_release_signature(&self, checked: &SignedRelease, manifest: &str, signature: &str) -> Result<SignatureKept, rusqlite::Error> {
        let now_ms = super::now_millis() as i64;
        self.with_conn(|conn| {
            let held: Option<String> = conn
                .query_row("SELECT manifest_sha256 FROM release_signatures WHERE version = ?1", params![checked.version], |row| row.get(0))
                .optional()?;
            match held {
                Some(h) if h == checked.manifest_sha256 => Ok(SignatureKept::Same),
                Some(_) => {
                    conn.execute(
                        "UPDATE release_signatures SET manifest_sha256 = ?2, manifest = ?3, signature = ?4, checked_at = ?5 WHERE version = ?1",
                        params![checked.version, checked.manifest_sha256, manifest, signature, now_ms],
                    )?;
                    Ok(SignatureKept::Replaced)
                }
                None => {
                    conn.execute(
                        "INSERT INTO release_signatures (version, manifest_sha256, manifest, signature, checked_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![checked.version, checked.manifest_sha256, manifest, signature, now_ms],
                    )?;
                    Ok(SignatureKept::New)
                }
            }
        })
    }

    /// Which of `versions` this server holds a checked signature for.
    pub fn signed_versions(&self, versions: &[&str]) -> Result<HashSet<String>, rusqlite::Error> {
        // Read-only primary-key lookups, one per version on the page (at most a page). Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare("SELECT 1 FROM release_signatures WHERE version = ?1")?;
            let mut out = HashSet::new();
            for v in versions {
                if stmt.query_row(params![v], |_| Ok(())).optional()?.is_some() {
                    out.insert((*v).to_string());
                }
            }
            Ok(out)
        })
    }

    /// How many releases this server holds a checked signature for (the tests, and `just brief`'s
    /// kind of reader).
    pub fn release_signature_count(&self) -> Result<i64, rusqlite::Error> {
        self.with_read_conn(|conn| conn.query_row("SELECT COUNT(*) FROM release_signatures", [], |row| row.get(0)))
    }
}
