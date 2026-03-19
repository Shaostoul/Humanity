use super::Storage;
use rusqlite::params;
use std::collections::HashMap;

impl Storage {
    // ── Profile methods ──

    /// Upsert a user profile (bio + socials JSON).
    /// Validates: bio max 280 chars, socials must be valid JSON and max 1024 chars.
    pub fn save_profile(&self, name: &str, bio: &str, socials: &str) -> Result<(), rusqlite::Error> {
        // Validate bio length.
        if bio.len() > 280 {
            return Err(rusqlite::Error::QueryReturnedNoRows); // abuse as validation error
        }
        // Validate socials is valid JSON and within size limit.
        if socials.len() > 1024 || serde_json::from_str::<serde_json::Value>(socials).is_err() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO profiles (name, bio, socials) VALUES (?1, ?2, ?3)
                 ON CONFLICT(name) DO UPDATE SET bio = ?2, socials = ?3",
                params![name, bio, socials],
            )?;
            Ok(())
        })
    }

    /// Get a user's profile. Returns (bio, socials) or None.
    pub fn get_profile(&self, name: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT bio, socials FROM profiles WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ) {
                Ok(profile) => Ok(Some(profile)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Bulk fetch profiles for a list of names.
    pub fn get_profiles_batch(&self, names: &[String]) -> Result<HashMap<String, (String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut result = HashMap::new();
            // SQLite doesn't support array params, so we query one at a time.
            // For typical user counts (<1000) this is fine.
            let mut stmt = conn.prepare(
                "SELECT name, bio, socials FROM profiles WHERE name = ?1 COLLATE NOCASE"
            )?;
            for name in names {
                if let Ok(row) = stmt.query_row(params![name], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                }) {
                    result.insert(row.0.to_lowercase(), (row.1, row.2));
                }
            }
            Ok(result)
        })
    }

    /// Upsert the full extended profile (all fields including new columns).
    /// Validates per-field length limits and that both `socials` and `privacy` are valid JSON.
    pub fn save_profile_extended(
        &self,
        name: &str,
        bio: &str,
        socials: &str,
        avatar_url: &str,
        banner_url: &str,
        pronouns: &str,
        location: &str,
        website: &str,
        privacy: &str,
    ) -> Result<(), rusqlite::Error> {
        if bio.len() > 280 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        if socials.len() > 1024 || serde_json::from_str::<serde_json::Value>(socials).is_err() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        if avatar_url.len() > 512 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        if banner_url.len() > 512 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        if pronouns.len() > 64 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        if location.len() > 128 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        if website.len() > 256 { return Err(rusqlite::Error::QueryReturnedNoRows); }
        if privacy.len() > 512 || serde_json::from_str::<serde_json::Value>(privacy).is_err() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO profiles (name, bio, socials, avatar_url, banner_url, pronouns, location, website, privacy)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(name) DO UPDATE SET
                   bio = ?2, socials = ?3, avatar_url = ?4, banner_url = ?5,
                   pronouns = ?6, location = ?7, website = ?8, privacy = ?9",
                params![name, bio, socials, avatar_url, banner_url, pronouns, location, website, privacy],
            )?;
            Ok(())
        })
    }

    /// Fetch the full extended profile row for internal use (all fields, no privacy filtering).
    pub fn get_profile_extended(
        &self,
        name: &str,
    ) -> Result<Option<(String, String, String, String, String, String, String, String)>, rusqlite::Error> {
        // Returns (bio, socials, avatar_url, banner_url, pronouns, location, website, privacy).
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT bio, socials, avatar_url, banner_url, pronouns, location, website, privacy
                 FROM profiles WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                )),
            ) {
                Ok(p) => Ok(Some(p)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Fetch a profile, filtering out private fields unless the requester is a friend.
    /// Returns a map of field name -> value for only the fields the requester may see.
    /// Fields omitted from the map should not be rendered on the client side.
    pub fn get_public_profile(
        &self,
        name: &str,
        requester_is_friend: bool,
    ) -> Result<Option<std::collections::HashMap<String, String>>, rusqlite::Error> {
        let row = match self.get_profile_extended(name)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let (bio, socials, avatar_url, banner_url, pronouns, location, website, privacy_json) = row;

        // Parse the privacy map; unknown fields default to public.
        let privacy: std::collections::HashMap<String, String> =
            serde_json::from_str(&privacy_json).unwrap_or_default();

        let is_private = |field: &str| -> bool {
            // A field is hidden if it is marked "private" AND the requester is not a friend.
            !requester_is_friend
                && privacy.get(field).map(|v| v == "private").unwrap_or(false)
        };

        let mut map = std::collections::HashMap::new();
        // bio and socials are always visible — they serve as the public-facing display.
        map.insert("bio".into(), bio);
        map.insert("socials".into(), socials);
        if !avatar_url.is_empty() { map.insert("avatar_url".into(), avatar_url); }
        if !banner_url.is_empty()  { map.insert("banner_url".into(), banner_url); }
        if !is_private("pronouns") && !pronouns.is_empty() {
            map.insert("pronouns".into(), pronouns);
        }
        if !is_private("location") && !location.is_empty() {
            map.insert("location".into(), location);
        }
        if !is_private("website") && !website.is_empty() {
            map.insert("website".into(), website);
        }
        Ok(Some(map))
    }
}
