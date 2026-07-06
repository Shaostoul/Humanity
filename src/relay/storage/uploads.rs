use super::Storage;
use rusqlite::params;

/// One row of the public shared-file library (v0.675).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SharedUpload {
    /// Stored filename under data/uploads/ (timestamp-mangled; the URL part).
    pub filename: String,
    /// The uploader's original filename (what humans should see).
    pub original_name: String,
    pub size_bytes: i64,
    pub uploaded_at: i64,
    pub uploader_key: String,
    /// Display name resolved from server_members ('' if unknown).
    pub uploader_name: String,
}

impl Storage {
    // ── Upload tracking (per-user media FIFO + the shared-file library) ──

    /// Record a new upload for `public_key` and FIFO-prune so at most
    /// `max_per_user` NON-shared uploads are retained (oldest deleted).
    /// Returns the filenames of pruned uploads so the caller can delete them
    /// from disk. `max_per_user` comes from server_settings.max_uploads_per_user
    /// (was a hardcoded 4 before v0.237). Clamped to >= 1 defensively.
    ///
    /// `shared` (v0.675): shared files enter the public library (GET
    /// /api/uploads) and are EXEMPT from this FIFO -- a shared .blend must not
    /// vanish because its uploader posted four chat photos. The server-wide
    /// disk cap (checked at upload time) still bounds total storage.
    pub fn record_upload(
        &self,
        public_key: &str,
        filename: &str,
        max_per_user: i64,
        shared: bool,
        original_name: &str,
        size_bytes: i64,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let max_per_user = max_per_user.max(1);
        self.with_conn(|conn| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            // Insert the new upload record.
            conn.execute(
                "INSERT INTO user_uploads (public_key, filename, uploaded_at, shared, original_name, size_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![public_key, filename, now, shared as i64, original_name, size_bytes],
            )?;

            // Count this key's NON-shared uploads (the media-FIFO population).
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM user_uploads WHERE public_key = ?1 AND shared = 0",
                params![public_key],
                |row| row.get(0),
            )?;

            let mut to_delete = Vec::new();
            if count > max_per_user {
                let excess = count - max_per_user;
                // Find the oldest NON-shared uploads to delete.
                let mut stmt = conn.prepare(
                    "SELECT id, filename FROM user_uploads
                     WHERE public_key = ?1 AND shared = 0 ORDER BY id ASC LIMIT ?2",
                )?;
                let rows: Vec<(i64, String)> = stmt.query_map(params![public_key, excess], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?.filter_map(|r| r.ok()).collect();

                for (id, fname) in &rows {
                    conn.execute("DELETE FROM user_uploads WHERE id = ?1", params![id])?;
                    to_delete.push(fname.clone());
                }
            }

            Ok(to_delete)
        })
    }

    /// Get the number of uploads for a user (all kinds).
    pub fn get_upload_count(&self, public_key: &str) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM user_uploads WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            )
        })
    }

    /// The public shared-file library (v0.675): newest first, optional
    /// case-insensitive name filter, uploader display name resolved from
    /// server_members where known. Only `shared = 1` rows are visible --
    /// ordinary chat media stays unlisted (reachable only by whoever was given
    /// its URL, exactly as before this feature).
    /// Remove a shared upload by its stored `filename`. Allowed when the
    /// requester OWNS the upload, or `is_admin` is true (the server owner
    /// curating the public library -- the operator's stated need to add AND
    /// remove files people can download). Returns:
    ///   Ok(Some(filename)) -> row deleted; caller unlinks the disk file.
    ///   Ok(None)           -> no such shared upload, or not the owner and
    ///                         not admin (caller returns 404/403 accordingly
    ///                         via `shared_upload_owner`).
    /// v0.709.
    pub fn delete_shared_upload(
        &self,
        filename: &str,
        requester_key: &str,
        is_admin: bool,
    ) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let owner: Option<String> = conn
                .query_row(
                    "SELECT public_key FROM user_uploads WHERE filename = ?1 AND shared = 1",
                    params![filename],
                    |row| row.get(0),
                )
                .ok();
            let owner = match owner {
                Some(o) => o,
                None => return Ok(None), // not a known shared upload
            };
            if !is_admin && owner != requester_key {
                return Ok(None); // not authorized to remove someone else's file
            }
            conn.execute(
                "DELETE FROM user_uploads WHERE filename = ?1",
                params![filename],
            )?;
            Ok(Some(filename.to_string()))
        })
    }

    pub fn list_shared_uploads(
        &self,
        limit: i64,
        search: Option<&str>,
    ) -> Result<Vec<SharedUpload>, rusqlite::Error> {
        let limit = limit.clamp(1, 500);
        self.with_conn(|conn| {
            let like = search
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!("%{}%", s.trim().to_lowercase()))
                .unwrap_or_else(|| "%".to_string());
            let mut stmt = conn.prepare(
                "SELECT u.filename, u.original_name, u.size_bytes, u.uploaded_at,
                        u.public_key, COALESCE(m.name, '')
                 FROM user_uploads u
                 LEFT JOIN server_members m ON m.public_key = u.public_key
                 WHERE u.shared = 1
                   AND (LOWER(u.original_name) LIKE ?1 OR LOWER(u.filename) LIKE ?1)
                 ORDER BY u.id DESC LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![like, limit], |row| {
                    Ok(SharedUpload {
                        filename: row.get(0)?,
                        original_name: row.get(1)?,
                        size_bytes: row.get(2)?,
                        uploaded_at: row.get(3)?,
                        uploader_key: row.get(4)?,
                        uploader_name: row.get(5)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod shared_library_tests {
    use super::super::Storage;

    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_uploads_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// Shared files are exempt from the per-user media FIFO: posting many chat
    /// photos must never prune a shared .blend (the review-grade failure this
    /// design exists to prevent).
    #[test]
    fn shared_files_survive_the_media_fifo() {
        let db = make_test_storage();
        db.record_upload("alice", "1_case.blend", 2, true, "phone_case.blend", 1000)
            .expect("shared upload records");
        // Five non-shared images against a keep-2 FIFO: three prunes expected,
        // never the shared file.
        let mut pruned_all = Vec::new();
        for i in 0..5 {
            let f = format!("{}_photo.png", i + 2);
            let pruned = db.record_upload("alice", &f, 2, false, "photo.png", 10).expect("records");
            pruned_all.extend(pruned);
        }
        assert_eq!(pruned_all.len(), 3, "keep-2 FIFO pruned the excess images");
        assert!(
            !pruned_all.iter().any(|f| f.contains("blend")),
            "the shared file must NEVER be FIFO-pruned: {pruned_all:?}"
        );
        let listed = db.list_shared_uploads(50, None).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].original_name, "phone_case.blend");
    }

    /// Only shared=1 rows are listed (chat media stays unlisted), and the
    /// search filter matches the ORIGINAL name case-insensitively.
    #[test]
    fn library_lists_only_shared_and_search_filters() {
        let db = make_test_storage();
        db.record_upload("bob", "1_bushing.stl", 4, true, "Car_Bushing_v2.stl", 500).unwrap();
        db.record_upload("bob", "2_secret.png", 4, false, "secret.png", 10).unwrap();
        db.record_upload("bob", "3_case.blend", 4, true, "phone_case.blend", 900).unwrap();

        let all = db.list_shared_uploads(50, None).unwrap();
        assert_eq!(all.len(), 2, "unshared chat media must not be listed");
        assert!(all.iter().all(|u| u.original_name != "secret.png"));

        let hits = db.list_shared_uploads(50, Some("bushing")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].original_name, "Car_Bushing_v2.stl");
        assert_eq!(hits[0].uploader_key, "bob");
    }

    /// Startup must survive a LIVE (pre-v0.675) database where user_uploads
    /// already exists without the shared/original_name/size_bytes columns.
    /// This is exactly what killed the v0.675.0 deploy: the (shared, id) index
    /// sat in the main schema batch, which runs BEFORE the ALTER block adds the
    /// column on an existing DB, so the whole batch aborted and the relay died
    /// with exit status 3. Fresh-DB tests (all the ones above) cannot see this.
    #[test]
    fn opens_a_pre_v0675_database_and_migrates_it() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_uploads_mig_{pid}_{nanos}.db"));

        // Build the OLD table shape + a pre-existing row, like the live DB.
        {
            let conn = rusqlite::Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE user_uploads (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    public_key  TEXT NOT NULL,
                    filename    TEXT NOT NULL,
                    uploaded_at INTEGER NOT NULL
                );
                INSERT INTO user_uploads (public_key, filename, uploaded_at)
                    VALUES ('carol', 'old_photo.png', 1);",
            )
            .expect("seed old-shape table");
        }

        // The line that crashed in production:
        let db = Storage::open(&path).expect("startup must survive a pre-v0.675 DB");

        // Old row is intact, defaulted unshared, and the new paths work on it.
        assert_eq!(db.get_upload_count("carol").unwrap(), 1);
        assert!(db.list_shared_uploads(50, None).unwrap().is_empty());
        db.record_upload("carol", "1_case.blend", 4, true, "case.blend", 42).unwrap();
        assert_eq!(db.list_shared_uploads(50, None).unwrap().len(), 1);
    }

    /// Removing a shared file: the owner can remove their own; a non-owner
    /// cannot; an admin can remove anyone's; a missing file yields None.
    #[test]
    fn delete_shared_upload_owner_and_admin_rules() {
        let db = make_test_storage();
        db.record_upload("alice", "1_a.blend", 2, true, "a.blend", 10).unwrap();
        db.record_upload("bob", "2_b.stl", 2, true, "b.stl", 10).unwrap();
        assert_eq!(db.list_shared_uploads(50, None).unwrap().len(), 2);

        // A non-owner, non-admin cannot remove alice's file.
        assert_eq!(db.delete_shared_upload("1_a.blend", "mallory", false).unwrap(), None);
        assert_eq!(db.list_shared_uploads(50, None).unwrap().len(), 2);

        // The owner removes their own.
        assert_eq!(
            db.delete_shared_upload("1_a.blend", "alice", false).unwrap(),
            Some("1_a.blend".to_string())
        );
        assert_eq!(db.list_shared_uploads(50, None).unwrap().len(), 1);

        // An admin removes bob's file even though not the owner.
        assert_eq!(
            db.delete_shared_upload("2_b.stl", "operator", true).unwrap(),
            Some("2_b.stl".to_string())
        );
        assert!(db.list_shared_uploads(50, None).unwrap().is_empty());

        // A file that does not exist yields None (caller -> 403/404).
        assert_eq!(db.delete_shared_upload("3_ghost.obj", "operator", true).unwrap(), None);
    }
}
