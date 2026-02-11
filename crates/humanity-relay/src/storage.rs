//! SQLite persistence for relay message history and peers.
//!
//! Security note: The security of the Humanity protocol lives in the
//! cryptographic object layer (Ed25519 signatures, XChaCha20-Poly1305
//! encryption), not in the storage layer. SQLite is just the container
//! for signed, tamper-evident objects.

use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

use crate::relay::RelayMessage;

/// A persisted emoji reaction record.
#[derive(Debug, Clone)]
pub struct ReactionRecord {
    pub target_from: String,
    pub target_timestamp: u64,
    pub emoji: String,
    pub reactor_key: String,
    pub reactor_name: String,
}

/// Persistent storage backed by SQLite.
pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    /// Open or create the database at the given path.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read/write performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                msg_type  TEXT NOT NULL,
                from_key  TEXT,
                from_name TEXT,
                content   TEXT,
                timestamp INTEGER NOT NULL,
                signature TEXT,
                raw_json  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_messages_timestamp
                ON messages(timestamp);

            -- Add channel_id column if it doesn't exist (migration).
            -- SQLite doesn't have IF NOT EXISTS for ALTER TABLE, so we handle it in code.

            CREATE TABLE IF NOT EXISTS peers (
                public_key  TEXT PRIMARY KEY,
                display_name TEXT,
                last_seen   INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS registered_names (
                name        TEXT NOT NULL COLLATE NOCASE,
                public_key  TEXT NOT NULL,
                registered_at INTEGER NOT NULL,
                PRIMARY KEY (name, public_key)
            );

            CREATE INDEX IF NOT EXISTS idx_registered_names_name
                ON registered_names(name COLLATE NOCASE);

            CREATE TABLE IF NOT EXISTS link_codes (
                code        TEXT PRIMARY KEY,
                name        TEXT NOT NULL COLLATE NOCASE,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS channels (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                created_by  TEXT,
                created_at  INTEGER NOT NULL,
                read_only   INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS invite_codes (
                code        TEXT PRIMARY KEY,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                used_by     TEXT
            );

            CREATE TABLE IF NOT EXISTS user_roles (
                public_key  TEXT PRIMARY KEY,
                role        TEXT NOT NULL DEFAULT 'user'
            );

            CREATE TABLE IF NOT EXISTS banned_keys (
                public_key  TEXT PRIMARY KEY,
                banned_at   INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS user_uploads (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                filename    TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_user_uploads_key
                ON user_uploads(public_key, id);

            CREATE TABLE IF NOT EXISTS reports (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                reporter_key TEXT NOT NULL,
                reported_name TEXT NOT NULL,
                reason      TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS reactions (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                target_from     TEXT NOT NULL,
                target_timestamp INTEGER NOT NULL,
                emoji           TEXT NOT NULL,
                reactor_key     TEXT NOT NULL,
                reactor_name    TEXT NOT NULL DEFAULT '',
                channel         TEXT NOT NULL DEFAULT 'general',
                created_at      INTEGER NOT NULL,
                UNIQUE(target_from, target_timestamp, emoji, reactor_key)
            );

            CREATE INDEX IF NOT EXISTS idx_reactions_target
                ON reactions(target_from, target_timestamp);
            CREATE INDEX IF NOT EXISTS idx_reactions_channel
                ON reactions(channel);"
        )?;

        // Migration: add channel_id column to messages if missing.
        let has_channel_id: bool = conn
            .prepare("SELECT channel_id FROM messages LIMIT 0")
            .is_ok();
        if !has_channel_id {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN channel_id TEXT DEFAULT 'general';"
            )?;
            info!("Migration: added channel_id column to messages");
        }

        // Create index on channel_id for efficient per-channel queries.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel_id, id);"
        )?;

        // Migration: profiles table for user bios and social links.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                name    TEXT PRIMARY KEY COLLATE NOCASE,
                bio     TEXT NOT NULL DEFAULT '',
                socials TEXT NOT NULL DEFAULT '{}'
            );"
        )?;

        // Migration: add read_only column to channels if missing.
        let has_read_only: bool = conn
            .prepare("SELECT read_only FROM channels LIMIT 0")
            .is_ok();
        if !has_read_only {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN read_only INTEGER DEFAULT 0;"
            )?;
            info!("Migration: added read_only column to channels");
        }

        // Migration: add position column to channels if missing.
        let has_position: bool = conn
            .prepare("SELECT position FROM channels LIMIT 0")
            .is_ok();
        if !has_position {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN position INTEGER DEFAULT 100;"
            )?;
            info!("Migration: added position column to channels");
        }

        info!("Database opened: {}", path.display());
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Store a message and return its row ID.
    pub fn store_message(&self, msg: &RelayMessage) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let raw = serde_json::to_string(msg).unwrap_or_default();

        match msg {
            RelayMessage::Chat { from, from_name, content, timestamp, signature, .. } => {
                conn.execute(
                    "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json)
                     VALUES ('chat', ?1, ?2, ?3, ?4, ?5, ?6)",
                    params![from, from_name, content, timestamp, signature, raw],
                )?;
            }
            RelayMessage::System { message } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                conn.execute(
                    "INSERT INTO messages (msg_type, content, timestamp, raw_json)
                     VALUES ('system', ?1, ?2, ?3)",
                    params![message, now, raw],
                )?;
            }
            _ => {
                // Don't persist peer_joined, peer_left, peer_list, identify.
                return Ok(0);
            }
        }

        Ok(conn.last_insert_rowid())
    }

    /// Load recent messages (most recent `limit`, ordered oldest first).
    pub fn load_recent_messages(&self, limit: usize) -> Result<Vec<RelayMessage>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT raw_json FROM (
                SELECT raw_json, id FROM messages
                WHERE msg_type = 'chat'
                ORDER BY id DESC
                LIMIT ?1
            ) sub ORDER BY id ASC"
        )?;

        let messages = stmt.query_map(params![limit], |row| {
            let raw: String = row.get(0)?;
            Ok(raw)
        })?
        .filter_map(|r| r.ok())
        .filter_map(|raw| serde_json::from_str::<RelayMessage>(&raw).ok())
        .collect();

        Ok(messages)
    }

    /// Load messages after a given row ID (for API polling).
    pub fn load_messages_after(&self, after_id: i64, limit: usize) -> Result<(Vec<RelayMessage>, i64), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, raw_json FROM messages
             WHERE id > ?1 AND msg_type = 'chat'
             ORDER BY id ASC
             LIMIT ?2"
        )?;

        let mut messages = Vec::new();
        let mut max_id = after_id;

        let rows = stmt.query_map(params![after_id, limit], |row| {
            let id: i64 = row.get(0)?;
            let raw: String = row.get(1)?;
            Ok((id, raw))
        })?;

        for row in rows {
            if let Ok((id, raw)) = row {
                if id > max_id {
                    max_id = id;
                }
                if let Ok(msg) = serde_json::from_str::<RelayMessage>(&raw) {
                    messages.push(msg);
                }
            }
        }

        Ok((messages, max_id))
    }

    /// Get the current max message ID (for cursor).
    pub fn max_message_id(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM messages",
            [],
            |row| row.get(0),
        )
    }

    /// Record a peer's last-seen timestamp.
    pub fn upsert_peer(&self, public_key: &str, display_name: Option<&str>, timestamp: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO peers (public_key, display_name, last_seen)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(public_key)
             DO UPDATE SET display_name = COALESCE(?2, display_name), last_seen = ?3",
            params![public_key, display_name, timestamp],
        )?;
        Ok(())
    }

    /// Check if a name is registered and whether the given key is authorized for it.
    /// Returns: Ok(None) if name is free, Ok(Some(true)) if key is authorized,
    /// Ok(Some(false)) if name is taken by other keys.
    pub fn check_name(&self, name: &str, public_key: &str) -> Result<Option<bool>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM registered_names WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |row| row.get(0),
        )?;
        if count == 0 {
            return Ok(None); // Name is free
        }
        let authorized: i64 = conn.query_row(
            "SELECT COUNT(*) FROM registered_names WHERE name = ?1 COLLATE NOCASE AND public_key = ?2",
            params![name, public_key],
            |row| row.get(0),
        )?;
        Ok(Some(authorized > 0))
    }

    /// Register a name for a public key.
    pub fn register_name(&self, name: &str, public_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT OR IGNORE INTO registered_names (name, public_key, registered_at) VALUES (?1, ?2, ?3)",
            params![name, public_key, now],
        )?;
        Ok(())
    }

    /// Create a link code for adding a new device to an existing name.
    /// Returns the generated code.
    pub fn create_link_code(&self, name: &str, created_by: &str) -> Result<String, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let expires = now + 5 * 60 * 1000; // 5 minutes

        // Simple random 6-char code from timestamp + key (no extra deps).
        let raw = format!("{}{}{}", now, created_by, name);
        let mut hash: u64 = 0;
        for b in raw.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        let code = format!("{:06X}", hash % 0xFFFFFF);

        // Clean up expired codes first.
        conn.execute("DELETE FROM link_codes WHERE expires_at < ?1", params![now])?;

        conn.execute(
            "INSERT INTO link_codes (code, name, created_by, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![code, name, created_by, now, expires],
        )?;

        Ok(code)
    }

    /// Redeem a link code: if valid, register the new key under the name.
    /// Returns Ok(Some(name)) on success, Ok(None) if code is invalid/expired.
    pub fn redeem_link_code(&self, code: &str, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let result = conn.query_row(
            "SELECT name FROM link_codes WHERE code = ?1 COLLATE NOCASE AND expires_at > ?2",
            params![code, now],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(name) => {
                // Delete the used code.
                conn.execute("DELETE FROM link_codes WHERE code = ?1 COLLATE NOCASE", params![code])?;
                // Register the new key.
                conn.execute(
                    "INSERT OR IGNORE INTO registered_names (name, public_key, registered_at) VALUES (?1, ?2, ?3)",
                    params![name, public_key, now],
                )?;
                Ok(Some(name))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

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

    /// List all channels (id, name, description, read_only).
    pub fn list_channels(&self) -> Result<Vec<(String, String, Option<String>, bool)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description, COALESCE(read_only, 0) FROM channels ORDER BY COALESCE(position, 100) ASC, created_at ASC")?;
        let channels = stmt.query_map([], |row| {
            let ro: i32 = row.get(3)?;
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, ro != 0))
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

    /// Load messages for a specific channel.
    pub fn load_channel_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<crate::relay::RelayMessage>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT raw_json FROM (
                SELECT raw_json, id FROM messages
                WHERE msg_type = 'chat' AND channel_id = ?1
                ORDER BY id DESC
                LIMIT ?2
            ) sub ORDER BY id ASC"
        )?;
        let messages = stmt.query_map(params![channel_id, limit], |row| {
            let raw: String = row.get(0)?;
            Ok(raw)
        })?.filter_map(|r| r.ok())
        .filter_map(|raw| serde_json::from_str::<crate::relay::RelayMessage>(&raw).ok())
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
                    messages.push(msg);
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

    // ── Upload tracking (per-user image FIFO) ──

    /// Record an upload for a user. If the user has more than 4 uploads,
    /// deletes the oldest and returns their filenames for disk cleanup.
    pub fn record_upload(&self, public_key: &str, filename: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
    }

    /// Get the number of uploads for a user.
    pub fn get_upload_count(&self, public_key: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM user_uploads WHERE public_key = ?1",
            params![public_key],
            |row| row.get(0),
        )
    }

    // ── Invite code methods ──

    /// Create an invite code (8-char hex, 24-hour expiry). Returns the code.
    pub fn create_invite_code(&self, created_by: &str) -> Result<String, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let expires = now + 24 * 60 * 60 * 1000; // 24 hours

        // Generate 8-char hex from timestamp + creator.
        let raw = format!("{}{}{}", now, created_by, now.wrapping_mul(2654435761));
        let mut hash: u64 = 0;
        for b in raw.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        let code = format!("{:08X}", hash % 0xFFFFFFFF);

        // Clean up expired codes.
        conn.execute("DELETE FROM invite_codes WHERE expires_at < ?1", params![now])?;

        conn.execute(
            "INSERT INTO invite_codes (code, created_by, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![code, created_by, now, expires],
        )?;

        Ok(code)
    }

    /// Redeem an invite code. Returns true if successful.
    pub fn redeem_invite_code(&self, code: &str, used_by: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let result = conn.query_row(
            "SELECT code FROM invite_codes WHERE code = ?1 COLLATE NOCASE AND expires_at > ?2 AND used_by IS NULL",
            params![code, now],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(_) => {
                conn.execute(
                    "UPDATE invite_codes SET used_by = ?1 WHERE code = ?2 COLLATE NOCASE",
                    params![used_by, code],
                )?;
                Ok(true)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    // ── Report methods ──

    /// Add a report.
    pub fn add_report(&self, reporter_key: &str, reported_name: &str, reason: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT INTO reports (reporter_key, reported_name, reason, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![reporter_key, reported_name, reason, now],
        )?;
        Ok(())
    }

    /// Get recent reports (newest first).
    pub fn get_reports(&self, limit: usize) -> Result<Vec<(i64, String, String, String, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, reporter_key, reported_name, reason, created_at FROM reports ORDER BY id DESC LIMIT ?1"
        )?;
        let reports = stmt.query_map(params![limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(reports)
    }

    /// Count reports from a specific key since a given timestamp.
    pub fn count_recent_reports(&self, reporter_key: &str, since_ms: i64) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM reports WHERE reporter_key = ?1 AND created_at > ?2",
            params![reporter_key, since_ms],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Clear all reports. Returns number deleted.
    pub fn clear_reports(&self) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM reports", [])?;
        Ok(rows)
    }

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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO profiles (name, bio, socials) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET bio = ?2, socials = ?3",
            params![name, bio, socials],
        )?;
        Ok(())
    }

    /// Get a user's profile. Returns (bio, socials) or None.
    pub fn get_profile(&self, name: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT bio, socials FROM profiles WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok(profile) => Ok(Some(profile)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Bulk fetch profiles for a list of names.
    pub fn get_profiles_batch(&self, names: &[String]) -> Result<HashMap<String, (String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
    }

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

    /// List all registered users with their first public key.
    /// Returns Vec<(name, first_key, role, key_count)> sorted alphabetically.
    pub fn list_all_users_with_keys(&self) -> Result<Vec<(String, String, String, usize)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT rn.name, rn.public_key, COALESCE(ur.role, '') as role
             FROM registered_names rn
             LEFT JOIN user_roles ur ON rn.public_key = ur.public_key
             ORDER BY rn.name COLLATE NOCASE, rn.registered_at ASC"
        )?;
        let rows: Vec<(String, String, String)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();

        // Group by name, take highest role and first key.
        let mut users: std::collections::BTreeMap<String, (String, String, usize)> = std::collections::BTreeMap::new();
        let role_priority = |r: &str| -> u8 {
            match r { "admin" => 4, "mod" => 3, "donor" => 2, "verified" => 1, _ => 0 }
        };
        for (name, key, role) in &rows {
            let lower_name = name.to_lowercase();
            let entry = users.entry(lower_name).or_insert((key.clone(), String::new(), 0));
            entry.2 += 1; // key count
            if role_priority(role) > role_priority(&entry.1) {
                entry.1 = role.clone();
            }
        }
        // Collect with original-case name from first occurrence.
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (name, _key, _role) in &rows {
            let lower = name.to_lowercase();
            if seen.insert(lower.clone()) {
                if let Some((first_key, role, count)) = users.get(&lower) {
                    result.push((name.clone(), first_key.clone(), role.clone(), *count));
                }
            }
        }
        Ok(result)
    }
}
