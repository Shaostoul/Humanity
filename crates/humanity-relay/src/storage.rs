//! SQLite persistence for relay message history and peers.
//!
//! Security note: The security of the Humanity protocol lives in the
//! cryptographic object layer (Ed25519 signatures, XChaCha20-Poly1305
//! encryption), not in the storage layer. SQLite is just the container
//! for signed, tamper-evident objects.

use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

use crate::relay::RelayMessage;

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
                created_at  INTEGER NOT NULL
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
                ON user_uploads(public_key, id);"
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
    pub fn create_channel(&self, id: &str, name: &str, description: Option<&str>, created_by: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "INSERT OR IGNORE INTO channels (id, name, description, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, description, created_by, now],
        )?;
        Ok(rows > 0)
    }

    /// Delete a channel.
    pub fn delete_channel(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM channels WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// List all channels.
    pub fn list_channels(&self) -> Result<Vec<(String, String, Option<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description FROM channels ORDER BY created_at ASC")?;
        let channels = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(channels)
    }

    /// Ensure the default "general" channel exists.
    pub fn ensure_default_channel(&self) -> Result<(), rusqlite::Error> {
        self.create_channel("general", "general", Some("General discussion"), "system")?;
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
}
