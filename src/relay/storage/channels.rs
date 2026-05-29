use super::Storage;
use rusqlite::params;

/// One banned user. Serialized over the WS protocol as part of
/// `banned_list` so the server-settings admin panel can list bans
/// and offer a per-row Unban.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BannedUser {
    pub public_key: String,
    /// Display name captured at ban time (the user's registered_names
    /// rows are deleted by the kick path, so this is the only record).
    pub name: String,
    /// Unix ms when the ban was applied.
    pub banned_at: i64,
}

/// One muted user. Serialized over the WS protocol as part of
/// `muted_list` so the moderator "Muted users" panel can list mutes
/// and offer a per-row Unmute. Structurally identical to BannedUser but
/// kept distinct so the `muted_at` field name reads correctly and the
/// two features can diverge later (e.g. timed mutes).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MutedUser {
    pub public_key: String,
    /// Display name captured at mute time.
    pub name: String,
    /// Unix ms when the mute was applied.
    pub muted_at: i64,
}

/// Set the `message_id` field on a Chat message so clients can correlate `message_deleted` events.
fn inject_message_id(msg: crate::relay::relay::RelayMessage, id: i64) -> crate::relay::relay::RelayMessage {
    if let crate::relay::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, channel, reply_to, thread_count, .. } = msg {
        crate::relay::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, channel, reply_to, thread_count, message_id: Some(id) }
    } else {
        msg
    }
}

impl Storage {
    // ── Channel methods ──

    /// Create a channel. Returns true if created, false if already exists.
    pub fn create_channel(&self, id: &str, name: &str, description: Option<&str>, created_by: &str, read_only: bool) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let rows = conn.execute(
                "INSERT OR IGNORE INTO channels (id, name, description, created_by, created_at, read_only) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, description, created_by, now, read_only as i32],
            )?;
            Ok(rows > 0)
        })
    }

    /// Delete a channel.
    pub fn delete_channel(&self, id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM channels WHERE id = ?1", params![id])?;
            Ok(rows > 0)
        })
    }

    /// Rename a channel ID/name and migrate message-scoped data.
    /// Returns true when a channel was renamed.
    pub fn rename_channel(&self, old_id: &str, new_id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn_mut(|conn| {
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
        })
    }

    /// List all channels (id, name, description, read_only, category_id).
    pub fn list_channels(&self) -> Result<Vec<(String, String, Option<String>, bool)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, name, description, COALESCE(read_only, 0) FROM channels ORDER BY COALESCE(position, 100) ASC, created_at ASC")?;
            let channels = stmt.query_map([], |row| {
                let ro: i32 = row.get(3)?;
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, ro != 0))
            })?.filter_map(|r| r.ok()).collect();
            Ok(channels)
        })
    }

    /// List all channels with category info.
    pub fn list_channels_with_categories(&self) -> Result<Vec<(String, String, Option<String>, bool, Option<i64>)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, name, description, COALESCE(read_only, 0), category_id FROM channels ORDER BY COALESCE(position, 100) ASC, created_at ASC")?;
            let channels = stmt.query_map([], |row| {
                let ro: i32 = row.get(3)?;
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, ro != 0, row.get(4)?))
            })?.filter_map(|r| r.ok()).collect();
            Ok(channels)
        })
    }

    /// List all channels with category info AND the voice_enabled flag.
    /// Used by `build_channel_list` so the broadcasted channel_list includes
    /// the persisted voice toggle. Tuple shape is
    /// `(id, name, description, read_only, category_id, voice_enabled)`.
    pub fn list_channels_with_categories_and_voice(&self) -> Result<Vec<(String, String, Option<String>, bool, Option<i64>, bool)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, COALESCE(read_only, 0), category_id, COALESCE(voice_enabled, 1)
                 FROM channels
                 ORDER BY COALESCE(position, 100) ASC, created_at ASC"
            )?;
            let channels = stmt.query_map([], |row| {
                let ro: i32 = row.get(3)?;
                let ve: i32 = row.get(5)?;
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, ro != 0, row.get(4)?, ve != 0))
            })?.filter_map(|r| r.ok()).collect();
            Ok(channels)
        })
    }

    /// Set the read_only flag on a channel.
    pub fn set_channel_read_only(&self, id: &str, read_only: bool) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE channels SET read_only = ?1 WHERE id = ?2",
                params![read_only as i32, id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Update a channel's display name (does NOT change the id).
    /// Server-side handler for `channel_update` uses this alongside
    /// `set_channel_description`. Renaming the id is a separate operation
    /// (`rename_channel`) because it requires migrating message-scoped data.
    pub fn set_channel_name(&self, id: &str, name: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE channels SET name = ?1 WHERE id = ?2",
                params![name, id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Update a channel's description.
    pub fn set_channel_description(&self, id: &str, description: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE channels SET description = ?1 WHERE id = ?2",
                params![description, id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Check if a channel exists.
    pub fn channel_exists(&self, id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT 1 FROM channels WHERE id = ?1",
                params![id],
                |_row| Ok(()),
            ) {
                Ok(_) => Ok(true),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
                Err(e) => Err(e),
            }
        })
    }

    pub fn is_channel_read_only(&self, id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT COALESCE(read_only, 0) FROM channels WHERE id = ?1",
                params![id],
                |row| row.get::<_, i32>(0),
            ) {
                Ok(val) => Ok(val != 0),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
                Err(e) => Err(e),
            }
        })
    }

    /// Set a channel's sort position (lower = higher in list).
    pub fn set_channel_position(&self, id: &str, position: i32) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE channels SET position = ?1 WHERE id = ?2",
                params![position, id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Ensure the default "general" channel exists.
    pub fn ensure_default_channel(&self) -> Result<(), rusqlite::Error> {
        self.create_channel("general", "general", Some("General discussion"), "system", false)?;
        Ok(())
    }

    /// Store a message with channel scope.
    pub fn store_message_in_channel(&self, msg: &crate::relay::relay::RelayMessage, channel_id: &str) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            let raw = serde_json::to_string(msg).unwrap_or_default();

            match msg {
                crate::relay::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, .. } => {
                    conn.execute(
                        "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id)
                         VALUES ('chat', ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![from, from_name, content, timestamp, signature, raw, channel_id],
                    )?;
                }
                _ => return Ok(0),
            }
            Ok(conn.last_insert_rowid())
        })
    }

    /// Store a message with channel scope and reply reference.
    pub fn store_message_in_channel_with_reply(&self, msg: &crate::relay::relay::RelayMessage, channel_id: &str, reply_to_from: &str, reply_to_timestamp: u64) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            let raw = serde_json::to_string(msg).unwrap_or_default();

            match msg {
                crate::relay::relay::RelayMessage::Chat { from, from_name, content, timestamp, signature, .. } => {
                    conn.execute(
                        "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id, reply_to_from, reply_to_timestamp)
                         VALUES ('chat', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![from, from_name, content, timestamp, signature, raw, channel_id, reply_to_from, reply_to_timestamp as i64],
                    )?;
                }
                _ => return Ok(0),
            }
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get all replies to a specific message (identified by from_key + timestamp).
    /// Returns Vec<(from_key, from_name, content, timestamp, channel_id)>.
    pub fn get_thread(&self, from_key: &str, timestamp: u64, limit: usize) -> Result<Vec<(String, String, String, u64, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Count replies to a specific message.
    pub fn get_thread_count(&self, from_key: &str, timestamp: u64) -> Result<u32, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE reply_to_from = ?1 AND reply_to_timestamp = ?2 AND msg_type = 'chat'",
                params![from_key, timestamp as i64],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }

    /// Load messages for a specific channel.
    pub fn load_channel_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<crate::relay::relay::RelayMessage>, rusqlite::Error> {
        self.with_conn(|conn| {
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
                serde_json::from_str::<crate::relay::relay::RelayMessage>(&raw).ok()
                    .map(|msg| inject_message_id(msg, id))
            })
            .collect();
            Ok(messages)
        })
    }

    /// Load messages for a channel after a given row ID (for API polling).
    /// When after_id == 0 (initial load), returns the most recent `limit` messages
    /// ordered oldest-first so the client can display them chronologically.
    pub fn load_channel_messages_after(&self, channel_id: &str, after_id: i64, limit: usize) -> Result<(Vec<crate::relay::relay::RelayMessage>, i64), rusqlite::Error> {
        self.with_conn(|conn| {
            // Initial load: get the most recent N, then reverse to oldest-first for display.
            // Polling (after_id > 0): get everything after the cursor, oldest-first.
            let sql = if after_id == 0 {
                "SELECT id, raw_json FROM (
                    SELECT id, raw_json FROM messages
                    WHERE msg_type = 'chat' AND channel_id = ?2
                    ORDER BY id DESC LIMIT ?3
                 ) sub ORDER BY id ASC"
            } else {
                "SELECT id, raw_json FROM messages
                 WHERE id > ?1 AND msg_type = 'chat' AND channel_id = ?2
                 ORDER BY id ASC LIMIT ?3"
            };
            let mut stmt = conn.prepare(sql)?;
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
                    if let Ok(msg) = serde_json::from_str::<crate::relay::relay::RelayMessage>(&raw) {
                        messages.push(inject_message_id(msg, id));
                    }
                }
            }
            Ok((messages, max_id))
        })
    }

    /// Remove a specific key from a name (device revocation).
    pub fn revoke_device(&self, name: &str, key_prefix: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Release a name entirely (admin action — removes all key associations).
    pub fn release_name(&self, name: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM registered_names WHERE name = ?1 COLLATE NOCASE",
                params![name],
            )?;
            Ok(rows)
        })
    }

    /// Get all public keys registered to a name.
    pub fn keys_for_name(&self, name: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
            )?;
            let keys = stmt.query_map(params![name], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(keys)
        })
    }

    /// Get all keys for a name with their labels and registration dates.
    pub fn keys_for_name_detailed(&self, name: &str) -> Result<Vec<(String, Option<String>, i64)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, label, registered_at FROM registered_names WHERE name = ?1 COLLATE NOCASE ORDER BY registered_at"
            )?;
            let keys = stmt.query_map(params![name], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?.filter_map(|r| r.ok()).collect();
            Ok(keys)
        })
    }

    /// Set a label for a specific key belonging to a name.
    pub fn label_key(&self, name: &str, public_key: &str, label: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let label_val = if label.is_empty() { None } else { Some(label) };
            let count = conn.execute(
                "UPDATE registered_names SET label = ?1 WHERE name = ?2 COLLATE NOCASE AND public_key = ?3",
                params![label_val, name, public_key],
            )?;
            Ok(count > 0)
        })
    }

    /// List all registered names with their highest role.
    /// Returns Vec<(name, role, key_count)> sorted alphabetically.
    pub fn list_all_users(&self) -> Result<Vec<(String, String, usize)>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Get the role for a public key (returns "" if no role set).
    pub fn get_role(&self, public_key: &str) -> Result<String, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT role FROM user_roles WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(role) => Ok(role),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(String::new()),
                Err(e) => Err(e),
            }
        })
    }

    /// Set the role for a public key.
    pub fn set_role(&self, public_key: &str, role: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO user_roles (public_key, role) VALUES (?1, ?2)
                 ON CONFLICT(public_key) DO UPDATE SET role = ?2",
                params![public_key, role],
            )?;
            Ok(())
        })
    }

    /// Check if a public key is banned.
    pub fn is_banned(&self, public_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM banned_keys WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Ban a public key, recording the display name so the admin
    /// "Banned users" panel can show who it is and offer an Unban
    /// (the user's registered_names rows are deleted by the kick path,
    /// so the name has to be captured here or it's lost forever).
    /// INSERT OR REPLACE so re-banning refreshes the name + timestamp.
    pub fn ban_user(&self, public_key: &str, name: &str) -> Result<(), rusqlite::Error> {
        if public_key.is_empty() {
            return Ok(()); // can't key-ban a keyless registration
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO banned_keys (public_key, banned_at, name)
                 VALUES (?1, ?2, ?3)",
                params![
                    public_key,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                    name
                ],
            )?;
            Ok(())
        })
    }

    /// Lift a ban for a public key.
    pub fn unban_user(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM banned_keys WHERE public_key = ?1",
                params![public_key],
            )?;
            Ok(())
        })
    }

    /// Every currently-banned user, newest ban first. Drives the
    /// server-settings "Banned users" admin panel.
    pub fn list_banned(&self) -> Result<Vec<BannedUser>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, name, banned_at FROM banned_keys
                 ORDER BY banned_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(BannedUser {
                    public_key: row.get(0)?,
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    banned_at: row.get(2)?,
                })
            })?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
    }

    /// Ban or unban a public key (name-less). Retained for the slash
    /// `/ban` and `/unban` command paths + back-compat; prefer
    /// `ban_user` (records the name) for the modal Ban button.
    pub fn set_banned(&self, public_key: &str, banned: bool) -> Result<(), rusqlite::Error> {
        if banned {
            self.ban_user(public_key, "")
        } else {
            self.unban_user(public_key)
        }
    }

    // ── Mute (v0.246, orthogonal to roles — see muted_members table) ──

    /// True if this key is muted (present in muted_members). Does NOT
    /// cover the legacy role='muted' case — the relay's Chat handler
    /// checks both so old-style mutes keep working until unmuted.
    pub fn is_muted(&self, public_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM muted_members WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Mute a key, recording the display name for the mod "Muted users"
    /// panel. Crucially does NOT touch user_roles — the user keeps their
    /// real role (donor/verified/mod) so unmute restores them exactly.
    /// INSERT OR REPLACE so re-muting refreshes name + timestamp.
    pub fn mute_user(&self, public_key: &str, name: &str) -> Result<(), rusqlite::Error> {
        if public_key.is_empty() {
            return Ok(());
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO muted_members (public_key, muted_at, name)
                 VALUES (?1, ?2, ?3)",
                params![
                    public_key,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                    name
                ],
            )?;
            Ok(())
        })
    }

    /// Lift a mute. Also clears a LEGACY role='muted' row (from the old
    /// destructive /mute that clobbered the role) so users muted before
    /// v0.246 can actually be freed — best-effort reset to the
    /// safe default-deny 'unverified' built-in since their original
    /// role was already lost by the old bandaid.
    pub fn unmute_user(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM muted_members WHERE public_key = ?1",
                params![public_key],
            )?;
            // Legacy cleanup: pre-v0.246 mutes set the role itself.
            conn.execute(
                "UPDATE user_roles SET role = 'unverified'
                 WHERE public_key = ?1 AND role = 'muted'",
                params![public_key],
            )?;
            Ok(())
        })
    }

    /// Every currently-muted user (table-based), newest first. Drives
    /// the server-settings "Muted users" panel.
    pub fn list_muted(&self) -> Result<Vec<MutedUser>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, name, muted_at FROM muted_members
                 ORDER BY muted_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(MutedUser {
                    public_key: row.get(0)?,
                    name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    muted_at: row.get(2)?,
                })
            })?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
    }

    /// Delete ALL messages (admin wipe).
    pub fn wipe_messages(&self) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM messages", [])?;
            Ok(rows)
        })
    }

    /// Delete all messages in a specific channel.
    pub fn wipe_channel_messages(&self, channel_id: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM messages WHERE channel_id = ?1",
                params![channel_id],
            )?;
            Ok(rows)
        })
    }

    /// Garbage collect inactive names.
    /// Finds names where no messages exist from any of the name's keys in the
    /// last `days` days AND all keys have role "" or "user" (not privileged).
    /// Deletes those names and returns them.
    pub fn garbage_collect_names(&self, days: u64) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Delete a message by sender key and timestamp (only your own messages).
    pub fn delete_message(&self, from_key: &str, timestamp: u64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM messages WHERE from_key = ?1 AND timestamp = ?2",
                params![from_key, timestamp as i64],
            )?;
            Ok(rows > 0)
        })
    }

    /// Get total message count.
    pub fn message_count(&self) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE msg_type = 'chat'",
                [],
                |row| row.get(0),
            )
        })
    }

    /// Get message count within the last N hours.
    pub fn message_count_since_hours(&self, hours: u64) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            let cutoff_ms = super::now_millis().saturating_sub(hours * 3600 * 1000) as i64;
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE msg_type = 'chat' AND timestamp > ?1",
                rusqlite::params![cutoff_ms],
                |row| row.get(0),
            )
        })
    }

    /// Get top N channels by message count.
    pub fn top_channels_by_messages(&self, limit: usize) -> Result<Vec<serde_json::Value>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT channel, COUNT(*) as cnt FROM messages
                 WHERE msg_type = 'chat' AND channel IS NOT NULL
                 GROUP BY channel ORDER BY cnt DESC LIMIT ?1"
            )?;
            let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
                let channel: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok(serde_json::json!({ "channel": channel, "count": count }))
            })?;
            rows.collect()
        })
    }

    /// Get message counts per hour for the last 24 hours.
    pub fn messages_per_hour_24h(&self) -> Result<Vec<serde_json::Value>, rusqlite::Error> {
        self.with_conn(|conn| {
            let now_ms = super::now_millis() as i64;
            let cutoff_ms = now_ms - 24 * 3600 * 1000;
            let mut stmt = conn.prepare(
                "SELECT (timestamp - ?1) / 3600000 as hour_bucket, COUNT(*) as cnt
                 FROM messages
                 WHERE msg_type = 'chat' AND timestamp > ?1
                 GROUP BY hour_bucket ORDER BY hour_bucket ASC"
            )?;
            let rows = stmt.query_map(rusqlite::params![cutoff_ms], |row| {
                let hour: i64 = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok(serde_json::json!({ "hour": hour, "count": count }))
            })?;
            rows.collect()
        })
    }
}

// ── Ban / mute storage tests (v0.249 — regression guards for the
//    v0.245 ban-management + v0.246 mute-rework + v0.247 changes) ──
#[cfg(test)]
mod ban_mute_tests {
    use super::*;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_banmute_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// Core v0.245 loop: ban records the key+name, is_banned/list_banned
    /// see it, unban clears it. Also the irreversible-ban-trap guard —
    /// the name MUST be captured at ban time (we never touch
    /// registered_names here, exactly like the real kick path which
    /// deletes it) so an admin can still see + lift the ban.
    #[test]
    fn ban_roundtrip_captures_name() {
        let db = fresh_db();
        assert!(!db.is_banned("attacker_key").unwrap());

        db.ban_user("attacker_key", "Mallory").expect("ban ok");
        assert!(db.is_banned("attacker_key").unwrap());

        let list = db.list_banned().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].public_key, "attacker_key");
        assert_eq!(
            list[0].name, "Mallory",
            "name must survive even though registered_names was never written \
             — this is the whole point of v0.245 (no irreversible bans)"
        );
        assert!(list[0].banned_at > 0);

        db.unban_user("attacker_key").expect("unban ok");
        assert!(!db.is_banned("attacker_key").unwrap());
        assert!(db.list_banned().unwrap().is_empty());
    }

    /// Back-compat: set_banned (the name-less delegator kept for the
    /// slash /ban + /unban paths) still bans/unbans, name blank.
    #[test]
    fn set_banned_delegates() {
        let db = fresh_db();
        db.set_banned("k", true).unwrap();
        assert!(db.is_banned("k").unwrap());
        assert_eq!(db.list_banned().unwrap()[0].name, "");
        db.set_banned("k", false).unwrap();
        assert!(!db.is_banned("k").unwrap());
    }

    /// A keyless registration (empty public_key) can't be key-banned —
    /// ban_user must be a silent no-op, not an error or a junk row.
    #[test]
    fn ban_empty_key_is_noop() {
        let db = fresh_db();
        db.ban_user("", "Ghost").expect("no-op ok");
        assert!(db.list_banned().unwrap().is_empty());
        assert!(!db.is_banned("").unwrap());
    }

    /// Re-banning the same key refreshes the captured name + timestamp
    /// (INSERT OR REPLACE), never duplicates the row.
    #[test]
    fn reban_replaces_not_duplicates() {
        let db = fresh_db();
        db.ban_user("k", "OldName").unwrap();
        db.ban_user("k", "NewName").unwrap();
        let list = db.list_banned().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "NewName");
    }

    /// v0.246 CORE INVARIANT: mute is orthogonal to role. Muting must
    /// NOT touch user_roles, and unmuting must leave a non-'muted' role
    /// exactly as it was (a Donor stays a Donor — the bug v0.246 fixed).
    #[test]
    fn mute_does_not_clobber_role() {
        let db = fresh_db();
        db.set_role("donor_key", "donor").unwrap();

        db.mute_user("donor_key", "Generous").unwrap();
        assert!(db.is_muted("donor_key").unwrap());
        assert_eq!(
            db.get_role("donor_key").unwrap(),
            "donor",
            "mute must not overwrite the user's real role"
        );

        db.unmute_user("donor_key").unwrap();
        assert!(!db.is_muted("donor_key").unwrap());
        assert_eq!(
            db.get_role("donor_key").unwrap(),
            "donor",
            "unmute must restore the user EXACTLY — role untouched"
        );
    }

    /// v0.246 legacy cleanup: a pre-v0.246 destructive /mute set
    /// user_roles.role='muted'. unmute_user must clear that legacy state
    /// (best-effort reset to the safe default-deny 'unverified') so
    /// users muted the old way can actually be freed.
    #[test]
    fn unmute_clears_legacy_muted_role() {
        let db = fresh_db();
        db.set_role("legacy_key", "muted").unwrap(); // simulate old /mute
        assert_eq!(db.get_role("legacy_key").unwrap(), "muted");

        db.unmute_user("legacy_key").unwrap();
        assert_eq!(
            db.get_role("legacy_key").unwrap(),
            "unverified",
            "legacy role='muted' must be reset on unmute or the user is \
             stuck muted forever"
        );
    }

    /// unmute_user must NOT touch a non-'muted' role even when there's
    /// no muted_members row (idempotent, role-safe).
    #[test]
    fn unmute_is_role_safe_for_nonmuted() {
        let db = fresh_db();
        db.set_role("mod_key", "mod").unwrap();
        db.unmute_user("mod_key").unwrap(); // never muted
        assert_eq!(db.get_role("mod_key").unwrap(), "mod");
    }

    /// Mute round-trip + list + empty-key guard.
    #[test]
    fn mute_roundtrip_and_empty_guard() {
        let db = fresh_db();
        db.mute_user("", "Ghost").expect("no-op ok");
        assert!(db.list_muted().unwrap().is_empty());

        db.mute_user("noisy_key", "Spammer").unwrap();
        let list = db.list_muted().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].public_key, "noisy_key");
        assert_eq!(list[0].name, "Spammer");
        assert!(list[0].muted_at > 0);

        db.unmute_user("noisy_key").unwrap();
        assert!(db.list_muted().unwrap().is_empty());
    }

    /// Ban and mute are independent stores — banning doesn't mute and
    /// vice-versa (they gate different things in the relay).
    #[test]
    fn ban_and_mute_are_independent() {
        let db = fresh_db();
        db.ban_user("k", "X").unwrap();
        assert!(db.is_banned("k").unwrap());
        assert!(!db.is_muted("k").unwrap());

        db.unban_user("k").unwrap();
        db.mute_user("k", "X").unwrap();
        assert!(!db.is_banned("k").unwrap());
        assert!(db.is_muted("k").unwrap());
    }

    /// DOCUMENTED CONTRACT (channels.rs `is_muted`): the table-based mute
    /// predicate does NOT cover the legacy `user_roles.role = 'muted'` case.
    /// The relay's Chat handler deliberately checks BOTH (table mute AND the
    /// legacy role) so pre-v0.246 mutes keep working — this test pins down
    /// that `is_muted` alone is table-only, so nobody "simplifies" the relay
    /// to a single `is_muted` check and silently un-mutes every legacy mute.
    #[test]
    fn is_muted_is_table_only_not_legacy_role() {
        let db = fresh_db();
        db.set_role("legacy", "muted").unwrap();
        assert!(
            !db.is_muted("legacy").unwrap(),
            "is_muted must report ONLY the muted_members table; the legacy \
             role='muted' is intentionally NOT covered here (the relay's \
             Chat handler checks the role separately)"
        );
        // A real table mute IS seen.
        db.mute_user("tabled", "Noisy").unwrap();
        assert!(db.is_muted("tabled").unwrap());
    }
}

// ── Channel message store / fetch tests (the chat persistence path that
//    backs handle_dm_open's sibling `handle_chat` history + the per-channel
//    `load_channel_messages` the client paints on channel switch). No prior
//    coverage existed for storing a RelayMessage::Chat and reading it back. ──
#[cfg(test)]
mod channel_message_tests {
    use super::*;
    use crate::relay::relay::RelayMessage;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_chanmsg_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// Build a minimal valid Chat message for a channel.
    fn chat(from: &str, content: &str, timestamp: u64, channel: &str) -> RelayMessage {
        RelayMessage::Chat {
            from: from.to_string(),
            from_name: Some(from.to_string()),
            content: content.to_string(),
            timestamp,
            signature: None,
            channel: channel.to_string(),
            reply_to: None,
            thread_count: None,
            message_id: None,
        }
    }

    /// Helper: pull (content, message_id) out of a Chat message for assertions.
    fn content_and_id(m: &RelayMessage) -> (String, Option<i64>) {
        match m {
            RelayMessage::Chat { content, message_id, .. } => (content.clone(), *message_id),
            _ => panic!("expected a Chat message, got {m:?}"),
        }
    }

    /// Store a chat message in a channel and read it back, ordered oldest→newest.
    /// `load_channel_messages` must also inject the DB row id as `message_id`
    /// (clients rely on it to correlate `message_deleted` events).
    #[test]
    fn store_and_load_channel_messages_roundtrip() {
        let db = fresh_db();
        let id1 = db.store_message_in_channel(&chat("alice", "first", 100, "general"), "general").unwrap();
        let id2 = db.store_message_in_channel(&chat("bob", "second", 200, "general"), "general").unwrap();
        assert!(id1 > 0 && id2 > id1, "row ids increase");

        let msgs = db.load_channel_messages("general", 50).unwrap();
        assert_eq!(msgs.len(), 2);
        let (c0, mid0) = content_and_id(&msgs[0]);
        let (c1, mid1) = content_and_id(&msgs[1]);
        // Oldest first.
        assert_eq!(c0, "first");
        assert_eq!(c1, "second");
        // message_id injected from the DB row id.
        assert_eq!(mid0, Some(id1), "row id injected as message_id");
        assert_eq!(mid1, Some(id2));
    }

    /// Messages are scoped to their channel — loading one channel never returns
    /// another channel's messages (per-channel isolation).
    #[test]
    fn channel_messages_are_isolated_per_channel() {
        let db = fresh_db();
        db.store_message_in_channel(&chat("alice", "in general", 100, "general"), "general").unwrap();
        db.store_message_in_channel(&chat("alice", "in random",  200, "random"),  "random").unwrap();

        let general = db.load_channel_messages("general", 50).unwrap();
        assert_eq!(general.len(), 1);
        assert_eq!(content_and_id(&general[0]).0, "in general");

        let random = db.load_channel_messages("random", 50).unwrap();
        assert_eq!(random.len(), 1);
        assert_eq!(content_and_id(&random[0]).0, "in random");

        // A channel with no messages returns empty (not an error, not cross-talk).
        assert!(db.load_channel_messages("empty", 50).unwrap().is_empty());
    }

    /// `load_channel_messages_after` with after_id == 0 returns the most-recent
    /// N oldest-first (initial load); with a cursor it returns only newer rows.
    #[test]
    fn load_after_initial_vs_cursor() {
        let db = fresh_db();
        let mut ids = Vec::new();
        for i in 0..5 {
            ids.push(db.store_message_in_channel(&chat("u", &format!("m{i}"), 100 + i as u64, "general"), "general").unwrap());
        }
        // Initial load (after_id 0): most-recent 3, oldest-first.
        let (initial, max_initial) = db.load_channel_messages_after("general", 0, 3).unwrap();
        assert_eq!(initial.len(), 3);
        assert_eq!(content_and_id(&initial[0]).0, "m2");
        assert_eq!(content_and_id(&initial[2]).0, "m4");
        assert_eq!(max_initial, *ids.last().unwrap(), "max id advances to newest");

        // Cursor poll: only rows strictly after ids[2] → m3, m4.
        let (after, max_after) = db.load_channel_messages_after("general", ids[2], 50).unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(content_and_id(&after[0]).0, "m3");
        assert_eq!(content_and_id(&after[1]).0, "m4");
        assert_eq!(max_after, ids[4]);
    }

    /// Threaded replies: a reply stores its parent reference, and the parent's
    /// reply count + the thread fetch reflect it. Drives the thread UI.
    #[test]
    fn thread_replies_are_counted_and_fetchable() {
        let db = fresh_db();
        // Parent message.
        db.store_message_in_channel(&chat("alice", "parent", 100, "general"), "general").unwrap();
        // Two replies referencing (alice, 100).
        db.store_message_in_channel_with_reply(&chat("bob",   "reply 1", 200, "general"), "general", "alice", 100).unwrap();
        db.store_message_in_channel_with_reply(&chat("carol", "reply 2", 300, "general"), "general", "alice", 100).unwrap();
        // An unrelated message that is NOT a reply.
        db.store_message_in_channel(&chat("dave", "noise", 400, "general"), "general").unwrap();

        assert_eq!(db.get_thread_count("alice", 100).unwrap(), 2, "two replies counted");
        assert_eq!(db.get_thread_count("alice", 999).unwrap(), 0, "no replies to a non-parent");

        let thread = db.get_thread("alice", 100, 50).unwrap();
        assert_eq!(thread.len(), 2);
        // (from_key, from_name, content, timestamp, channel_id), oldest first.
        assert_eq!(thread[0].2, "reply 1");
        assert_eq!(thread[1].2, "reply 2");
        assert_eq!(thread[0].4, "general", "thread rows carry their channel");
    }

    /// Non-Chat variants are not persisted by the channel store (it returns 0
    /// and inserts nothing) — only Chat rows belong in the message log.
    #[test]
    fn non_chat_message_is_not_persisted() {
        let db = fresh_db();
        let sys = RelayMessage::System { message: "server notice".to_string() };
        let id = db.store_message_in_channel(&sys, "general").unwrap();
        assert_eq!(id, 0, "System message is not stored via the channel path");
        assert!(db.load_channel_messages("general", 50).unwrap().is_empty());
    }
}
