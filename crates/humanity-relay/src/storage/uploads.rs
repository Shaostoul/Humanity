use super::Storage;
use rusqlite::params;

impl Storage {
    // ── Upload tracking (per-user image FIFO) ──

    /// Record an upload for a user. If the user has more than 4 uploads,
    /// deletes the oldest and returns their filenames for disk cleanup.
    pub fn record_upload(&self, public_key: &str, filename: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            // Insert the new upload record.
            conn.execute(
                "INSERT INTO user_uploads (public_key, filename, uploaded_at) VALUES (?1, ?2, ?3)",
                params![public_key, filename, now],
            )?;

            // Count uploads for this key.
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM user_uploads WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            )?;

            let mut to_delete = Vec::new();
            if count > 4 {
                let excess = count - 4;
                // Find the oldest uploads to delete.
                let mut stmt = conn.prepare(
                    "SELECT id, filename FROM user_uploads WHERE public_key = ?1 ORDER BY id ASC LIMIT ?2"
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

    /// Get the number of uploads for a user.
    pub fn get_upload_count(&self, public_key: &str) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM user_uploads WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            )
        })
    }
}
