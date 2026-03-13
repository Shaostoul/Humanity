use super::Storage;
use super::ReactionRecord;
use rusqlite::params;

impl Storage {
    // ── Reaction methods ──

    /// Toggle a reaction. Returns Ok(true) if added, Ok(false) if removed.
    pub fn toggle_reaction(
        &self,
        target_from: &str,
        target_timestamp: u64,
        emoji: &str,
        reactor_key: &str,
        reactor_name: &str,
        channel: &str,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Check if the reaction already exists.
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM reactions WHERE target_from = ?1 AND target_timestamp = ?2 AND emoji = ?3 AND reactor_key = ?4",
            params![target_from, target_timestamp as i64, emoji, reactor_key],
            |row| { let c: i64 = row.get(0)?; Ok(c > 0) },
        )?;
        if exists {
            conn.execute(
                "DELETE FROM reactions WHERE target_from = ?1 AND target_timestamp = ?2 AND emoji = ?3 AND reactor_key = ?4",
                params![target_from, target_timestamp as i64, emoji, reactor_key],
            )?;
            Ok(false)
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            conn.execute(
                "INSERT INTO reactions (target_from, target_timestamp, emoji, reactor_key, reactor_name, channel, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![target_from, target_timestamp as i64, emoji, reactor_key, reactor_name, channel, now],
            )?;
            Ok(true)
        }
    }

    /// Load reactions for a given channel (most recent N by created_at).
    pub fn load_channel_reactions(&self, channel_id: &str, limit: usize) -> Result<Vec<ReactionRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT target_from, target_timestamp, emoji, reactor_key, reactor_name
             FROM reactions
             WHERE channel = ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        )?;
        let records = stmt.query_map(params![channel_id, limit], |row| {
            Ok(ReactionRecord {
                target_from: row.get(0)?,
                target_timestamp: row.get::<_, i64>(1)? as u64,
                emoji: row.get(2)?,
                reactor_key: row.get(3)?,
                reactor_name: row.get(4)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(records)
    }

    /// Edit a message's content by sender key and timestamp.
    /// Updates the content column and the raw_json blob.
    /// Returns true if a row was updated.
    pub fn edit_message(&self, from_key: &str, timestamp: u64, new_content: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // First, fetch the existing raw_json so we can update it.
        let existing: Option<(i64, String)> = conn.query_row(
            "SELECT id, raw_json FROM messages WHERE from_key = ?1 AND timestamp = ?2 LIMIT 1",
            params![from_key, timestamp as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok();

        if let Some((id, raw)) = existing {
            // Parse and update the JSON blob.
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&raw) {
                val["content"] = serde_json::Value::String(new_content.to_string());
                let new_raw = serde_json::to_string(&val).unwrap_or(raw);
                let rows = conn.execute(
                    "UPDATE messages SET content = ?1, raw_json = ?2 WHERE id = ?3",
                    params![new_content, new_raw, id],
                )?;
                Ok(rows > 0)
            } else {
                // Fallback: just update the content column.
                let rows = conn.execute(
                    "UPDATE messages SET content = ?1 WHERE from_key = ?2 AND timestamp = ?3",
                    params![new_content, from_key, timestamp as i64],
                )?;
                Ok(rows > 0)
            }
        } else {
            Ok(false)
        }
    }
}
