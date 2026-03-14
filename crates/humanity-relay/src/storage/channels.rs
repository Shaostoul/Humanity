use super::Storage;
use rusqlite::params;

/// Set the `message_id` field on a Chat message so clients can correlate `message_deleted` events.
fn inject_message_id(msg: crate::relay::RelayMessage, id: i64) -> crate::relay::RelayMessage {
    if let crate::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, channel, reply_to, thread_count, .. } = msg {
        crate::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, channel, reply_to, thread_count, message_id: Some(id) }
    } else {
        msg
    }
}

impl Storage {
    // ── Channel methods ──

    /// Create a channel. Returns true if created, false if already exists.
    pub fn create_channel(&self, id: &str, name: &str, description: Option<&str>, created_by: &str, read_only: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "INSERT OR IGNORE INTO channels (id, name, description, created_by, created_at, read_only) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, description, created_by, now, read_only as i32],
        )?;
        Ok(rows > 0)
    }

    /// Delete a channel.
    pub fn delete_channel(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM channels WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// Rename a channel ID/name and migrate message-scoped data.
    /// Returns true when a channel was renamed.
    pub fn rename_channel(&self, old_id: &str, new_id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.conn.lock().unwrap();

        // Refuse if destination already exists.
        let dest_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM channels WHERE id = ?1",
            params![new_id],
            |row| row.get(0),
        )?;
        if dest_exists > 0 {
            return Ok(false);
        }

        let tx = conn.transaction()?;

        // Rename channel row.
        let changed = tx.execute(
            "UPDATE channels SET id = ?1, name = ?1 WHERE id = ?2",
            params![new_id, old_id],
        )?;

        if changed == 0 {
            tx.rollback()?;
            return Ok(false);
        }

        // Migrate message and channel-scoped metadata.
        tx.execute("UPDATE messages SET channel_id = ?1 WHERE channel_id = ?2", params![new_id, old_id])?;
        tx.execute("UPDATE reactions SET channel = ?1 WHERE channel = ?2", params![new_id, old_id])?;
        tx.execute("UPDATE pinned_messages SET channel = ?1 WHERE channel = ?2", params![new_id, old_id])?;

        tx.commit()?;
        Ok(true)
    }

    /// List all channels (id, name, description, read_only, category_id).
    pub fn list_channels(&self) -> Result<Vec<(String, String, Option<String>, bool)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description, COALESCE(read_only, 0) FROM channels ORDER BY COALESCE(position, 100) ASC, created_at ASC")?;
        let channels = stmt.query_map([], |row| {
            let ro: i32 = row.get(3)?;
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, ro != 0))
        })?.filter_map(|r| r.ok()).collect();
        Ok(channels)
    }

    /// List all channels with category info.
    pub fn list_channels_with_categories(&self) -> Result<Vec<(String, String, Option<String>, bool, Option<i64>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description, COALESCE(read_only, 0), category_id FROM channels ORDER BY COALESCE(position, 100) ASC, created_at ASC")?;
        let channels = stmt.query_map([], |row| {
            let ro: i32 = row.get(3)?;
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, ro != 0, row.get(4)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(channels)
    }

    /// Set the read_only flag on a channel.
    pub fn set_channel_read_only(&self, id: &str, read_only: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE channels SET read_only = ?1 WHERE id = ?2",
            params![read_only as i32, id],
        )?;
        Ok(rows > 0)
    }

    /// Check if a channel is read-only.
    /// Check if a channel exists.
    pub fn channel_exists(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT 1 FROM channels WHERE id = ?1",
            params![id],
            |_row| Ok(()),
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn is_channel_read_only(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT COALESCE(read_only, 0) FROM channels WHERE id = ?1",
            params![id],
            |row| row.get::<_, i32>(0),
        ) {
            Ok(val) => Ok(val != 0),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Set a channel's sort position (lower = higher in list).
    pub fn set_channel_position(&self, id: &str, position: i32) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE channels SET position = ?1 WHERE id = ?2",
            params![position, id],
        )?;
        Ok(rows > 0)
    }

    /// Ensure the default "general" channel exists.
    pub fn ensure_default_channel(&self) -> Result<(), rusqlite::Error> {
        self.create_channel("general", "general", Some("General discussion"), "system", false)?;
        Ok(())
    }

    /// Store a message with channel scope.
    pub fn store_message_in_channel(&self, msg: &crate::relay::RelayMessage, channel_id: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let raw = serde_json::to_string(msg).unwrap_or_default();

        match msg {
            crate::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, .. } => {
                conn.execute(
                    "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id)
                     VALUES ('chat', ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![from, from_name, content, timestamp, signature, raw, channel_id],
                )?;
            }
            _ => return Ok(0),
        }
        Ok(conn.last_insert_rowid())
    }

    /// Store a message with channel scope and reply reference.
    pub fn store_message_in_channel_with_reply(&self, msg: &crate::relay::RelayMessage, channel_id: &str, reply_to_from: &str, reply_to_timestamp: u64) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let raw = serde_json::to_string(msg).unwrap_or_default();

        match msg {
            crate::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, .. } => {
                conn.execute(
                    "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id, reply_to_from, reply_to_timestamp)
                     VALUES ('chat', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![from, from_name, content, timestamp, signature, raw, channel_id, reply_to_from, reply_to_timestamp as i64],
                )?;
            }
            _ => return Ok(0),
        }
        Ok(conn.last_insert_rowid())
    }

    /// Get all replies to a specific message (identified by from_key + timestamp).
    /// Returns Vec<(from_key, from_name, content, timestamp, channel_id)>.
    pub fn get_thread(&self, from_key: &str, timestamp: u64, limit: usize) -> Result<Vec<(String, String, String, u64, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT from_key, COALESCE(from_name, ''), content, timestamp, COALESCE(channel_id, 'general')
             FROM messages
             WHERE reply_to_from = ?1 AND reply_to_timestamp = ?2 AND msg_type = 'chat'
             ORDER BY timestamp ASC
             LIMIT ?3"
        )?;
        let results = stmt.query_map(params![from_key, timestamp as i64, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? as u64,
                row.get::<_, String>(4)?,
            ))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    /// Count replies to a specific message.
    pub fn get_thread_count(&self, from_key: &str, timestamp: u64) -> Result<u32, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE reply_to_from = ?1 AND reply_to_timestamp = ?2 AND msg_type = 'chat'",
            params![from_key, timestamp as i64],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }

    /// Load messages for a specific channel.
    pub fn load_channel_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<crate::relay::RelayMessage>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, raw_json FROM (
                SELECT raw_json, id FROM messages
                WHERE msg_type = 'chat' AND channel_id = ?1
                ORDER BY id DESC
                LIMIT ?2
            ) sub ORDER BY id ASC"
        )?;
        let messages = stmt.query_map(params![channel_id, limit], |row| {
            let id: i64 = row.get(0)?;
            let raw: String = row.get(1)?;
            Ok((id, raw))
        })?.filter_map(|r| r.ok())
        .filter_map(|(id, raw)| {
            serde_json::from_str::<crate::relay::RelayMessage>(&raw).ok()
                .map(|msg| inject_message_id(msg, id))
        })
        .collect();
        Ok(messages)
    }

    /// Load messages for a channel after a given row ID (for API polling).
    pub fn load_channel_messages_after(&self, channel_id: &str, after_id: i64, limit: usize) -> Result<(Vec<crate::relay::RelayMessage>, i64), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, raw_json FROM messages
             WHERE id > ?1 AND msg_type = 'chat' AND channel_id = ?2
             ORDER BY id ASC
             LIMIT ?3"
        )?;
        let mut messages = Vec::new();
        let mut max_id = after_id;
        let rows = stmt.query_map(params![after_id, channel_id, limit], |row| {
            let id: i64 = row.get(0)?;
            let raw: String = row.get(1)?;
            Ok((id, raw))
        })?;
        for row in rows {
            if let Ok((id, raw)) = row {
                if id > max_id { max_id = id; }
                if let Ok(msg) = serde_json::from_str::<crate::relay::RelayMessage>(&raw) {
                    messages.push(inject_message_id(msg, id));
                }
            }
        }
        Ok((messages, max_id))
    }

    /// Remove a specific key from a name (device revocation).
    pub fn revoke_device(&self, name: &str, key_prefix: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Find keys matching the prefix for this name.
        let mut stmt = conn.prepare(
            "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE AND public_key LIKE ?2"
        )?;
        let prefix_pattern = format!("{}%", key_prefix);
        let keys: Vec<String> = stmt.query_map(params![name, prefix_pattern], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        for key in &keys {
            conn.execute(
                "DELETE FROM registered_names WHERE name = ?1 COLLATE NOCASE AND public_key = ?2",
                params![name, key],
            )?;
        }
        Ok(keys)
    }

    /// Release a name entirely (admin action — removes all key associations).
    pub fn release_name(&self, name: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM registered_names WHERE name = ?1 COLLATE NOCASE",
            params![name],
        )?;
        Ok(rows)
    }

    /// Get all public keys registered to a name.
    pub fn keys_for_name(&self, name: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
        )?;
        let keys = stmt.query_map(params![name], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(keys)
    }

    /// Get all keys for a name with their labels and registration dates.
    pub fn keys_for_name_detailed(&self, name: &str) -> Result<Vec<(String, Option<String>, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT public_key, label, registered_at FROM registered_names WHERE name = ?1 COLLATE NOCASE ORDER BY registered_at"
        )?;
        let keys = stmt.query_map(params![name], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(keys)
    }

    /// Set a label for a specific key belonging to a name.
    pub fn label_key(&self, name: &str, public_key: &str, label: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let label_val = if label.is_empty() { None } else { Some(label) };
        let count = conn.execute(
            "UPDATE registered_names SET label = ?1 WHERE name = ?2 COLLATE NOCASE AND public_key = ?3",
            params![label_val, name, public_key],
        )?;
        Ok(count > 0)
    }

    /// List all registered names with their highest role.
    /// Returns Vec<(name, role, key_count)> sorted alphabetically.
    pub fn list_all_users(&self) -> Result<Vec<(String, String, usize)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT rn.name, rn.public_key, COALESCE(ur.role, '') as role
             FROM registered_names rn
             LEFT JOIN user_roles ur ON rn.public_key = ur.public_key
             ORDER BY rn.name COLLATE NOCASE"
        )?;
        let rows: Vec<(String, String, String)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();

        // Group by name, take highest role.
        let mut users: std::collections::BTreeMap<String, (String, usize)> = std::collections::BTreeMap::new();
        let role_priority = |r: &str| -> u8 {
            match r { "admin" => 4, "mod" => 3, "donor" => 2, "verified" => 1, _ => 0 }
        };
        for (name, _key, role) in &rows {
            let lower_name = name.to_lowercase();
            let entry = users.entry(lower_name).or_insert((String::new(), 0));
            entry.1 += 1; // key count
            if role_priority(role) > role_priority(&entry.0) {
                entry.0 = role.clone();
            }
        }
        // Collect with original-case name from first occurrence.
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (name, _key, _role) in &rows {
            let lower = name.to_lowercase();
            if seen.insert(lower.clone()) {
                if let Some((role, count)) = users.get(&lower) {
                    result.push((name.clone(), role.clone(), *count));
                }
            }
        }
        Ok(result)
    }

    /// Get the role for a public key (returns "" if no role set).
    pub fn get_role(&self, public_key: &str) -> Result<String, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT role FROM user_roles WHERE public_key = ?1",
            params![public_key],
            |row| row.get(0),
        ) {
            Ok(role) => Ok(role),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(String::new()),
            Err(e) => Err(e),
        }
    }

    /// Set the role for a public key.
    pub fn set_role(&self, public_key: &str, role: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO user_roles (public_key, role) VALUES (?1, ?2)
             ON CONFLICT(public_key) DO UPDATE SET role = ?2",
            params![public_key, role],
        )?;
        Ok(())
    }

    /// Check if a public key is banned.
    pub fn is_banned(&self, public_key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM banned_keys WHERE public_key = ?1",
            params![public_key],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Ban or unban a public key.
    pub fn set_banned(&self, public_key: &str, banned: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        if banned {
            conn.execute(
                "INSERT OR IGNORE INTO banned_keys (public_key, banned_at) VALUES (?1, ?2)",
                params![public_key, std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64],
            )?;
        } else {
            conn.execute("DELETE FROM banned_keys WHERE public_key = ?1", params![public_key])?;
        }
        Ok(())
    }

    /// Delete ALL messages (admin wipe).
    pub fn wipe_messages(&self) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM messages", [])?;
        Ok(rows)
    }

    /// Delete all messages in a specific channel.
    pub fn wipe_channel_messages(&self, channel_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM messages WHERE channel_id = ?1",
            params![channel_id],
        )?;
        Ok(rows)
    }

    /// Garbage collect inactive names.
    /// Finds names where no messages exist from any of the name's keys in the
    /// last `days` days AND all keys have role "" or "user" (not privileged).
    /// Deletes those names and returns them.
    pub fn garbage_collect_names(&self, days: u64) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cutoff_ms = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            now - (days as i64 * 24 * 60 * 60 * 1000)
        };

        // Find all distinct names.
        let mut name_stmt = conn.prepare(
            "SELECT DISTINCT name FROM registered_names"
        )?;
        let all_names: Vec<String> = name_stmt.query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut to_delete = Vec::new();

        for name in &all_names {
            // Get all keys for this name.
            let mut key_stmt = conn.prepare(
                "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
            )?;
            let keys: Vec<String> = key_stmt.query_map(params![name], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            if keys.is_empty() { continue; }

            // Check if any key has a privileged role.
            let mut has_privileged = false;
            for key in &keys {
                let role: String = conn.query_row(
                    "SELECT COALESCE((SELECT role FROM user_roles WHERE public_key = ?1), '')",
                    params![key],
                    |row| row.get(0),
                ).unwrap_or_default();
                if !role.is_empty() && role != "user" {
                    has_privileged = true;
                    break;
                }
            }
            if has_privileged { continue; }

            // Check if any key has messages in the last `days` days.
            let mut has_recent = false;
            for key in &keys {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM messages WHERE from_key = ?1 AND timestamp > ?2",
                    params![key, cutoff_ms],
                    |row| row.get(0),
                )?;
                if count > 0 {
                    has_recent = true;
                    break;
                }
            }
            if has_recent { continue; }

            to_delete.push(name.clone());
        }

        // Delete the inactive names.
        for name in &to_delete {
            conn.execute(
                "DELETE FROM registered_names WHERE name = ?1 COLLATE NOCASE",
                params![name],
            )?;
        }

        Ok(to_delete)
    }

    /// Delete a message by sender key and timestamp (only your own messages).
    pub fn delete_message(&self, from_key: &str, timestamp: u64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM messages WHERE from_key = ?1 AND timestamp = ?2",
            params![from_key, timestamp as i64],
        )?;
        Ok(rows > 0)
    }

    /// Get total message count.
    pub fn message_count(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE msg_type = 'chat'",
            [],
            |row| row.get(0),
        )
    }
}
