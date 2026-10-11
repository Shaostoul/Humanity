use super::Storage;
use rusqlite::params;
use std::collections::HashMap;

/// The three words `accepts_files` may hold (2026-10-11, docs/design/media-showcase.md "Files on
/// the sender's device and in messages", part 3): who the person accepts files in messages from.
/// Protocol words, read by both apps: the receiving app enforces them, and a sender's app reads
/// them from the profile to say "Ann accepts files only from friends." before sending.
pub const ACCEPTS_FILES_WORDS: [&str; 3] = ["nobody", "friends", "anyone"];

/// What `accepts_files` reads as for a person who never chose: the column's default, and the
/// answer for someone with no profile row at all.
pub const ACCEPTS_FILES_DEFAULT: &str = "friends";

/// `word` if it is one of [`ACCEPTS_FILES_WORDS`], exactly as written (no trimming or case
/// folding: these are protocol words an app sends, not something a person types).
pub fn accepts_files_word(word: &str) -> Option<&'static str> {
    ACCEPTS_FILES_WORDS.iter().copied().find(|w| *w == word)
}

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
        // Read-only single-row lookup. Read pool.
        self.with_read_conn(|conn| {
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
        // Read-only: prepared SELECT looped per name (no writes). Read pool.
        self.with_read_conn(|conn| {
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
    ///
    /// `accepts_files`: `None` when the sender did not send it (an app from before the field,
    /// v0.1485.1 and older), and the stored word is KEPT (a new row takes
    /// [`ACCEPTS_FILES_DEFAULT`]), the same not-sent rule as the role limits
    /// (storage/roles.rs `LIMIT_NOT_SENT`). `Some` of anything but [`ACCEPTS_FILES_WORDS`] is an
    /// error and nothing is saved. Returns the word stored after the save, for the answer.
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
        accepts_files: Option<&str>,
    ) -> Result<String, rusqlite::Error> {
        if accepts_files.is_some_and(|w| accepts_files_word(w).is_none()) {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
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
            // ?10 NULL (not sent): a new row takes the default, an existing row keeps its word
            // (in the DO UPDATE, a bare column name is the row as it was).
            conn.execute(
                "INSERT INTO profiles (name, bio, socials, avatar_url, banner_url, pronouns, location, website, privacy, accepts_files)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, COALESCE(?10, ?11))
                 ON CONFLICT(name) DO UPDATE SET
                   bio = ?2, socials = ?3, avatar_url = ?4, banner_url = ?5,
                   pronouns = ?6, location = ?7, website = ?8, privacy = ?9,
                   accepts_files = COALESCE(?10, accepts_files)",
                params![name, bio, socials, avatar_url, banner_url, pronouns, location, website, privacy, accepts_files, ACCEPTS_FILES_DEFAULT],
            )?;
            conn.query_row(
                "SELECT accepts_files FROM profiles WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| row.get::<_, String>(0),
            )
        })
    }

    /// Who `name` accepts files in messages from: their stored word, or
    /// [`ACCEPTS_FILES_DEFAULT`] when they have no profile row (or a word this relay does not
    /// know, which only a hand-edited database could hold).
    pub fn accepts_files_of(&self, name: &str) -> String {
        let stored: Option<String> = self
            .with_read_conn(|conn| {
                use rusqlite::OptionalExtension;
                conn.query_row(
                    "SELECT accepts_files FROM profiles WHERE name = ?1 COLLATE NOCASE",
                    params![name],
                    |row| row.get::<_, String>(0),
                )
                .optional()
            })
            .unwrap_or_else(|e| {
                tracing::error!("profiles: could not read who a person accepts files from, using the default: {e}");
                None
            });
        stored
            .as_deref()
            .and_then(accepts_files_word)
            .unwrap_or(ACCEPTS_FILES_DEFAULT)
            .to_string()
    }

    /// Fetch the full extended profile row for internal use (all fields, no privacy filtering).
    pub fn get_profile_extended(
        &self,
        name: &str,
    ) -> Result<Option<(String, String, String, String, String, String, String, String)>, rusqlite::Error> {
        // Returns (bio, socials, avatar_url, banner_url, pronouns, location, website, privacy).
        // Read-only single-row lookup. Read pool.
        self.with_read_conn(|conn| {
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
        // Published to everyone, never private: its point is that a sender's app (a stranger's
        // included) can say "Ann accepts files only from friends." before sending.
        map.insert("accepts_files".into(), self.accepts_files_of(name));
        Ok(Some(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The word stored in `name`'s profile row, read straight from the column (no default filled
    /// in, which `accepts_files_of` would do and so hide a missing column or row).
    fn column(db: &Storage, name: &str) -> Result<Option<String>, rusqlite::Error> {
        db.with_read_conn(|c| {
            use rusqlite::OptionalExtension;
            c.query_row("SELECT accepts_files FROM profiles WHERE name = ?1 COLLATE NOCASE", params![name], |r| r.get(0))
                .optional()
        })
    }

    /// BUG-046's rule for a new column on a live table: a database from before `accepts_files`
    /// (its profiles table as v0.1485.1 left it, with a row) opens, gains the column, and the
    /// row it held reads "friends"; a save then sets and keeps the word as on a fresh database.
    ///
    /// Seen red 2026-10-11 with the `accepts_files` ALTER taken out of `Storage::open`: "a
    /// database from before accepts_files has no accepts_files column after a restart: no such
    /// column: accepts_files".
    #[test]
    fn an_older_database_gains_accepts_files_with_the_default() {
        let path = crate::test_temp::db("accepts_files_mig");
        {
            let conn = rusqlite::Connection::open(&*path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE profiles (
                    name    TEXT PRIMARY KEY COLLATE NOCASE,
                    bio     TEXT NOT NULL DEFAULT '',
                    socials TEXT NOT NULL DEFAULT '{}',
                    avatar_url TEXT NOT NULL DEFAULT '',
                    banner_url  TEXT NOT NULL DEFAULT '',
                    pronouns    TEXT NOT NULL DEFAULT '',
                    location    TEXT NOT NULL DEFAULT '',
                    website     TEXT NOT NULL DEFAULT '',
                    privacy     TEXT NOT NULL DEFAULT '{}'
                );
                INSERT INTO profiles (name, bio) VALUES ('Ann', 'from before');",
            )
            .expect("seed the old profiles table");
        }

        let db = Storage::open(&path).expect("a database from before accepts_files opens");
        let ann = column(&db, "Ann")
            .unwrap_or_else(|e| panic!("a database from before accepts_files has no accepts_files column after a restart: {e}"));
        assert_eq!(ann.as_deref(), Some("friends"), "the row it held reads the default");
        assert_eq!(db.get_profile("Ann").unwrap().map(|p| p.0).as_deref(), Some("from before"), "and keeps what it had");

        let stored = db.save_profile_extended("Ann", "now", "{}", "", "", "", "", "", "{}", Some("anyone")).unwrap();
        assert_eq!(stored, "anyone");
        db.save_profile_extended("Ann", "again", "{}", "", "", "", "", "", "{}", None).unwrap();
        assert_eq!(column(&db, "Ann").unwrap().as_deref(), Some("anyone"));
    }

    /// The storage itself refuses a word that is not one of the three, and changes nothing (the
    /// relay's handler refuses it first, with a notice; this is the floor under it). A save that
    /// does not send the field keeps the stored word, and a person new to the table gets the
    /// default; someone with no row at all reads as the default.
    ///
    /// Seen red 2026-10-11 with the word check taken out of `save_profile_extended`: "a word
    /// that is not one of the three is refused: \"everyone\"" (the save stored it).
    #[test]
    fn only_the_three_words_are_stored_and_not_sending_one_keeps_it() {
        let db = Storage::open_temp("accepts_files_words");
        assert_eq!(db.accepts_files_of("Bo"), "friends", "no row reads as the default");
        assert_eq!(db.save_profile_extended("Bo", "b", "{}", "", "", "", "", "", "{}", None).unwrap(), "friends", "a new row takes the default");
        for word in ["nobody", "anyone", "friends"] {
            assert_eq!(db.save_profile_extended("Bo", "b", "{}", "", "", "", "", "", "{}", Some(word)).unwrap(), word);
            assert_eq!(db.accepts_files_of("Bo"), word);
        }
        db.save_profile_extended("Bo", "b", "{}", "", "", "", "", "", "{}", Some("nobody")).unwrap();
        for bad in ["everyone", "Friends", " anyone", ""] {
            assert!(
                db.save_profile_extended("Bo", "changed", "{}", "", "", "", "", "", "{}", Some(bad)).is_err(),
                "a word that is not one of the three is refused: {bad:?}"
            );
        }
        assert_eq!(db.accepts_files_of("Bo"), "nobody", "and nothing changed");
        assert_eq!(db.get_profile("Bo").unwrap().map(|p| p.0).as_deref(), Some("b"));
        assert_eq!(db.save_profile_extended("Bo", "c", "{}", "", "", "", "", "", "{}", None).unwrap(), "nobody", "not sent keeps it");
    }

    /// The person's export lists who they accept files from (in their profile row), and their
    /// erase takes it with the row: afterwards their name reads as the default and nothing of the
    /// row is left. Another person's word stays.
    ///
    /// Seen red 2026-10-11 with the export's profile grab narrowed to `SELECT name, bio, socials`:
    /// "the export lists who they accept files from: null"; and with the profiles DELETE in
    /// `delete_account` made to match no row: "the erase clears it: Some(\"nobody\")".
    #[test]
    fn the_export_lists_accepts_files_and_the_erase_clears_it() {
        let db = Storage::open_temp("accepts_files_account");
        db.register_name("Cy", "c1c1").unwrap();
        db.register_name("Di", "d1d1").unwrap();
        db.save_profile_extended("Cy", "hi", "{}", "", "", "", "", "", "{}", Some("nobody")).unwrap();
        db.save_profile_extended("Di", "hey", "{}", "", "", "", "", "", "{}", Some("anyone")).unwrap();

        let export = db.export_account("c1c1", "Cy");
        let profile = export["profile"].as_array().cloned().unwrap_or_default();
        assert_eq!(profile.len(), 1, "the export has their profile row: {export}");
        assert_eq!(profile[0]["accepts_files"], "nobody", "the export lists who they accept files from: {}", profile[0]["accepts_files"]);

        let receipt = db.delete_account("c1c1", "Cy");
        assert!(!receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "{receipt:?}");
        assert_eq!(column(&db, "Cy").unwrap(), None, "the erase clears it: {:?}", column(&db, "Cy").unwrap());
        assert_eq!(db.accepts_files_of("Cy"), "friends", "their name reads as the default afterwards");
        assert_eq!(db.accepts_files_of("Di"), "anyone", "another person's word stays");
    }
}
