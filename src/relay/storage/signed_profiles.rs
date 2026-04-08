//! Signed profile storage — profiles are cryptographically signed objects
//! that replicate across every server a user touches. Latest timestamp wins.
//! No home server — the signature IS the authority.

use rusqlite::{params, OptionalExtension};

use super::Storage;

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

impl Storage {
    /// Store a signed profile if it's newer than the existing one.
    /// Returns true if the profile was inserted/updated (i.e., it was newer).
    pub fn store_signed_profile(
        &self,
        public_key: &str,
        name: &str,
        bio: &str,
        avatar_url: &str,
        banner_url: &str,
        socials: &str,
        pronouns: &str,
        location: &str,
        website: &str,
        timestamp: u64,
        signature: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            // Check if we already have a newer or equal profile
            let existing_ts: Option<u64> = conn
                .query_row(
                    "SELECT timestamp FROM signed_profiles WHERE public_key = ?1",
                    params![public_key],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(existing) = existing_ts {
                if existing >= timestamp {
                    return Ok(false); // Already have this or newer
                }
            }

            conn.execute(
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
                params![public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp as i64, signature],
            )?;
            Ok(true)
        })
    }

    /// Get a signed profile by public key.
    pub fn get_signed_profile(&self, public_key: &str) -> Result<Option<SignedProfileRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature
                 FROM signed_profiles WHERE public_key = ?1",
                params![public_key],
                |row| {
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
                },
            )
            .optional()
        })
    }

    /// Get all signed profiles updated since a given timestamp.
    /// Used for bulk gossip to newly connected federated servers.
    pub fn get_profiles_since(&self, since_timestamp: u64) -> Result<Vec<SignedProfileRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, name, bio, avatar_url, banner_url, socials, pronouns, location, website, timestamp, signature
                 FROM signed_profiles WHERE timestamp > ?1 ORDER BY timestamp ASC LIMIT 1000",
            )?;
            let rows = stmt.query_map(params![since_timestamp as i64], |row| {
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
            })?;
            rows.collect()
        })
    }

    /// Count total signed profiles cached on this server.
    pub fn count_signed_profiles(&self) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM signed_profiles", [], |row| row.get(0))
        })
    }
}
