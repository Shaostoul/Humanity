//! SQLite persistence for relay message history and peers.
//!
//! Security note: The security of the Humanity protocol lives in the
//! cryptographic object layer (Ed25519 signatures, XChaCha20-Poly1305
//! encryption), not in the storage layer. SQLite is just the container
//! for signed, tamper-evident objects.

use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

use crate::relay::RelayMessage;

/// A persisted DM record.
#[derive(Debug, Clone)]
pub struct DmRecord {
    pub from_key: String,
    pub from_name: String,
    pub to_key: String,
    pub content: String,
    pub timestamp: u64,
    /// Whether this DM is end-to-end encrypted.
    pub encrypted: bool,
    /// Base64-encoded nonce/IV for encrypted DMs.
    pub nonce: Option<String>,
}

/// A DM conversation summary.
#[derive(Debug, Clone)]
pub struct DmConversation {
    pub partner_key: String,
    pub partner_name: String,
    pub last_message: String,
    pub last_timestamp: u64,
    pub unread_count: i64,
}

/// A persisted pinned message record.
#[derive(Debug, Clone)]
pub struct PinnedMessageRecord {
    pub channel: String,
    pub from_key: String,
    pub from_name: String,
    pub content: String,
    pub original_timestamp: u64,
    pub pinned_by: String,
    pub pinned_at: u64,
}

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
                ON reactions(channel);

            CREATE TABLE IF NOT EXISTS pinned_messages (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                channel           TEXT NOT NULL,
                from_key          TEXT NOT NULL,
                from_name         TEXT NOT NULL,
                content           TEXT NOT NULL,
                original_timestamp INTEGER NOT NULL,
                pinned_by         TEXT NOT NULL,
                pinned_at         INTEGER NOT NULL,
                UNIQUE(channel, from_key, original_timestamp)
            );

            CREATE INDEX IF NOT EXISTS idx_pinned_channel
                ON pinned_messages(channel);"
        )?;

        // Server state key-value store (for persisting lockdown, etc.).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS server_state (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );"
        )?;

        // Federation: federated server registry.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS federated_servers (
                server_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                public_key TEXT,
                trust_tier INTEGER NOT NULL DEFAULT 0,
                accord_compliant INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'unknown',
                last_seen INTEGER,
                added_at INTEGER NOT NULL
            );"
        )?;

        // DM table for direct messages.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS direct_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_key TEXT NOT NULL,
                from_name TEXT NOT NULL,
                to_key TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                read INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_dm_conversation
                ON direct_messages(from_key, to_key);
            CREATE INDEX IF NOT EXISTS idx_dm_to
                ON direct_messages(to_key);"
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

        // User data sync table (settings, notes, todos, etc.).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_data (
                public_key TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                updated_at INTEGER NOT NULL
            );"
        )?;

        // Migration: user_status table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_status (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                status TEXT NOT NULL DEFAULT 'online',
                status_text TEXT NOT NULL DEFAULT ''
            );"
        )?;

        // Migration: profiles table for user bios and social links.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                name    TEXT PRIMARY KEY COLLATE NOCASE,
                bio     TEXT NOT NULL DEFAULT '',
                socials TEXT NOT NULL DEFAULT '{}'
            );"
        )?;

        // Channel categories for Discord-like grouping.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS channel_categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                collapsed INTEGER NOT NULL DEFAULT 0
            );"
        )?;

        // Persistent voice channels.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS voice_channels (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                position    INTEGER DEFAULT 0,
                created_by  TEXT,
                created_at  INTEGER NOT NULL
            );"
        )?;

        // Link preview cache for URL embeds.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS link_previews (
                url TEXT PRIMARY KEY,
                title TEXT,
                description TEXT,
                image TEXT,
                site_name TEXT,
                fetched_at INTEGER NOT NULL
            );"
        )?;

        // Project board: tasks (kanban).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_tasks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                status      TEXT NOT NULL DEFAULT 'backlog',
                priority    TEXT NOT NULL DEFAULT 'medium',
                assignee    TEXT,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                position    INTEGER NOT NULL DEFAULT 0,
                labels      TEXT NOT NULL DEFAULT '[]'
            );

            CREATE INDEX IF NOT EXISTS idx_project_tasks_status
                ON project_tasks(status);

            CREATE TABLE IF NOT EXISTS task_comments (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id     INTEGER NOT NULL,
                author_key  TEXT NOT NULL,
                author_name TEXT NOT NULL,
                content     TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                FOREIGN KEY (task_id) REFERENCES project_tasks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_task_comments_task
                ON task_comments(task_id);"
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

        // Migration: add category_id column to channels if missing.
        let has_category_id: bool = conn
            .prepare("SELECT category_id FROM channels LIMIT 0")
            .is_ok();
        if !has_category_id {
            conn.execute_batch(
                "ALTER TABLE channels ADD COLUMN category_id INTEGER DEFAULT NULL;"
            )?;
            info!("Migration: added category_id column to channels");
        }

        // Migration: add ecdh_public column to registered_names for E2EE DMs.
        let has_ecdh: bool = conn
            .prepare("SELECT ecdh_public FROM registered_names LIMIT 0")
            .is_ok();
        if !has_ecdh {
            conn.execute_batch(
                "ALTER TABLE registered_names ADD COLUMN ecdh_public TEXT DEFAULT NULL;"
            )?;
            info!("Migration: added ecdh_public column to registered_names");
        }

        // Migration: add encrypted and nonce columns to direct_messages for E2EE.
        let has_dm_encrypted: bool = conn
            .prepare("SELECT encrypted FROM direct_messages LIMIT 0")
            .is_ok();
        if !has_dm_encrypted {
            conn.execute_batch(
                "ALTER TABLE direct_messages ADD COLUMN encrypted INTEGER DEFAULT 0;
                 ALTER TABLE direct_messages ADD COLUMN nonce TEXT DEFAULT NULL;"
            )?;
            info!("Migration: added encrypted/nonce columns to direct_messages");
        }

        // Migration: add reply_to columns to messages for threaded replies.
        let has_reply_to: bool = conn
            .prepare("SELECT reply_to_from FROM messages LIMIT 0")
            .is_ok();
        if !has_reply_to {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN reply_to_from TEXT DEFAULT NULL;
                 ALTER TABLE messages ADD COLUMN reply_to_timestamp INTEGER DEFAULT NULL;"
            )?;
            info!("Migration: added reply_to_from/reply_to_timestamp columns to messages");
        }

        // Index for efficient thread queries (find all replies to a message).
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_reply_to
                ON messages(reply_to_from, reply_to_timestamp);"
        )?;

        // Follows table (social system).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS follows (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                follower_key TEXT NOT NULL,
                followed_key TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                UNIQUE(follower_key, followed_key)
            );

            CREATE INDEX IF NOT EXISTS idx_follows_follower
                ON follows(follower_key);
            CREATE INDEX IF NOT EXISTS idx_follows_followed
                ON follows(followed_key);"
        )?;

        // Groups tables (foundation).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS groups (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                creator_key TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                invite_code TEXT UNIQUE
            );

            CREATE TABLE IF NOT EXISTS group_members (
                group_id    TEXT NOT NULL,
                member_key  TEXT NOT NULL,
                role        TEXT NOT NULL DEFAULT 'member',
                joined_at   TEXT NOT NULL,
                PRIMARY KEY (group_id, member_key)
            );

            CREATE INDEX IF NOT EXISTS idx_group_members_key
                ON group_members(member_key);

            CREATE TABLE IF NOT EXISTS group_messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id    TEXT NOT NULL,
                from_key    TEXT NOT NULL,
                from_name   TEXT NOT NULL DEFAULT '',
                content     TEXT NOT NULL,
                timestamp   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_group_messages_group
                ON group_messages(group_id, timestamp);"
        )?;

        // Friend codes table (out-of-band friend discovery).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS friend_codes (
                code TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                uses_remaining INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_friend_codes_key
                ON friend_codes(public_key);"
        )?;

        // Marketplace listings table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS marketplace_listings (
                id TEXT PRIMARY KEY,
                seller_key TEXT NOT NULL,
                seller_name TEXT,
                title TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL,
                condition TEXT,
                price TEXT,
                payment_methods TEXT,
                location TEXT,
                images TEXT,
                status TEXT DEFAULT 'active',
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_marketplace_seller
                ON marketplace_listings(seller_key);
            CREATE INDEX IF NOT EXISTS idx_marketplace_status
                ON marketplace_listings(status);
            CREATE INDEX IF NOT EXISTS idx_marketplace_category
                ON marketplace_listings(category);"
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

        // Cryptographically random 8-char hex code (4 random bytes via CSPRNG).
        let random_bytes: [u8; 4] = rand::rng().random();
        let code = format!("{:02X}{:02X}{:02X}{:02X}", random_bytes[0], random_bytes[1], random_bytes[2], random_bytes[3]);

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

        // Cryptographically random 8-char hex code (CSPRNG).
        let random_val: u32 = rand::rng().random();
        let code = format!("{:08X}", random_val);

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

    // ── Pin methods ──

    /// Pin a message. Returns true if pinned, false if already pinned.
    pub fn pin_message(
        &self,
        channel: &str,
        from_key: &str,
        from_name: &str,
        content: &str,
        original_timestamp: u64,
        pinned_by: &str,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let rows = conn.execute(
            "INSERT OR IGNORE INTO pinned_messages (channel, from_key, from_name, content, original_timestamp, pinned_by, pinned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![channel, from_key, from_name, content, original_timestamp as i64, pinned_by, now as i64],
        )?;
        Ok(rows > 0)
    }

    /// Unpin a message by its 1-based index in the channel's pin list (ordered by pinned_at).
    /// Returns true if removed.
    pub fn unpin_message(&self, channel: &str, index: usize) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Get all pins for the channel ordered by pinned_at ASC.
        let mut stmt = conn.prepare(
            "SELECT id FROM pinned_messages WHERE channel = ?1 ORDER BY pinned_at ASC"
        )?;
        let ids: Vec<i64> = stmt.query_map(params![channel], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        if index == 0 || index > ids.len() {
            return Ok(false);
        }

        let target_id = ids[index - 1];
        let rows = conn.execute("DELETE FROM pinned_messages WHERE id = ?1", params![target_id])?;
        Ok(rows > 0)
    }

    /// Get all pinned messages for a channel, ordered by pinned_at ASC.
    pub fn get_pinned_messages(&self, channel: &str) -> Result<Vec<PinnedMessageRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT channel, from_key, from_name, content, original_timestamp, pinned_by, pinned_at
             FROM pinned_messages
             WHERE channel = ?1
             ORDER BY pinned_at ASC"
        )?;
        let records = stmt.query_map(params![channel], |row| {
            Ok(PinnedMessageRecord {
                channel: row.get(0)?,
                from_key: row.get(1)?,
                from_name: row.get(2)?,
                content: row.get(3)?,
                original_timestamp: row.get::<_, i64>(4)? as u64,
                pinned_by: row.get(5)?,
                pinned_at: row.get::<_, i64>(6)? as u64,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(records)
    }

    /// Search messages by content, optionally filtered by channel.
    pub fn search_messages(&self, query: &str, channel: Option<&str>, limit: usize) -> Result<Vec<(i64, String, RelayMessage)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.min(50);
        if let Some(ch) = channel {
            let mut stmt = conn.prepare(
                "SELECT id, channel_id, raw_json FROM messages
                 WHERE msg_type = 'chat' AND content LIKE '%' || ?1 || '%' AND channel_id = ?2
                 ORDER BY timestamp DESC LIMIT ?3"
            )?;
            let results = stmt.query_map(params![query, ch, limit], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?.filter_map(|r| r.ok())
            .filter_map(|(id, ch, raw)| {
                serde_json::from_str::<RelayMessage>(&raw).ok().map(|msg| (id, ch, msg))
            })
            .collect();
            Ok(results)
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, channel_id, raw_json FROM messages
                 WHERE msg_type = 'chat' AND content LIKE '%' || ?1 || '%'
                 ORDER BY timestamp DESC LIMIT ?2"
            )?;
            let results = stmt.query_map(params![query, limit], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?.filter_map(|r| r.ok())
            .filter_map(|(id, ch, raw)| {
                serde_json::from_str::<RelayMessage>(&raw).ok().map(|msg| (id, ch, msg))
            })
            .collect();
            Ok(results)
        }
    }

    /// Search messages with full filtering: query, channel, from (sender name), limit.
    /// Escapes SQL LIKE special characters in the query.
    /// Also searches DMs if channel is None.
    pub fn search_messages_full(&self, query: &str, channel: Option<&str>, from_name: Option<&str>, limit: usize, requester_key: &str) -> Result<Vec<(i64, String, RelayMessage)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.min(100);

        // Escape SQL LIKE special chars
        let escaped_query = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");

        // Build dynamic query for channel messages
        let mut sql = String::from(
            "SELECT id, channel_id, raw_json FROM messages WHERE msg_type = 'chat' AND content LIKE '%' || ?1 || '%' ESCAPE '\\'"
        );
        let mut param_idx = 2u32;
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(escaped_query.clone())];

        if let Some(ch) = channel {
            sql.push_str(&format!(" AND channel_id = ?{param_idx}"));
            params_vec.push(Box::new(ch.to_string()));
            param_idx += 1;
        }
        if let Some(fname) = from_name {
            let escaped_from = fname
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            sql.push_str(&format!(" AND from_name LIKE '%' || ?{param_idx} || '%' ESCAPE '\\'"));
            params_vec.push(Box::new(escaped_from));
            param_idx += 1;
        }
        sql.push_str(&format!(" ORDER BY timestamp DESC LIMIT ?{param_idx}"));
        params_vec.push(Box::new(limit as i64));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let mut results: Vec<(i64, String, RelayMessage)> = stmt.query_map(params_refs.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?.filter_map(|r| r.ok())
        .filter_map(|(id, ch, raw)| {
            serde_json::from_str::<crate::relay::RelayMessage>(&raw).ok().map(|msg| (id, ch, msg))
        })
        .collect();

        // Also search DMs if no specific channel filter
        if channel.is_none() {
            let mut dm_sql = String::from(
                "SELECT id, from_key, from_name, content, timestamp FROM direct_messages WHERE content LIKE '%' || ?1 || '%' ESCAPE '\\' AND (from_key = ?2 OR to_key = ?2)"
            );
            let mut dm_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(escaped_query.clone()), Box::new(requester_key.to_string())];
            let mut dm_idx = 3u32;

            if let Some(fname) = from_name {
                let escaped_from = fname
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                dm_sql.push_str(&format!(" AND from_name LIKE '%' || ?{dm_idx} || '%' ESCAPE '\\'"));
                dm_params.push(Box::new(escaped_from));
                dm_idx += 1;
            }
            dm_sql.push_str(&format!(" ORDER BY timestamp DESC LIMIT ?{dm_idx}"));
            dm_params.push(Box::new(limit as i64));

            let dm_refs: Vec<&dyn rusqlite::types::ToSql> = dm_params.iter().map(|p| p.as_ref()).collect();

            if let Ok(mut dm_stmt) = conn.prepare(&dm_sql) {
                let dm_results: Vec<(i64, String, crate::relay::RelayMessage)> = dm_stmt.query_map(dm_refs.as_slice(), |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                }).ok()
                .into_iter()
                .flatten()
                .filter_map(|r| r.ok())
                .map(|(id, from_key, fname, content, ts)| {
                    let msg = crate::relay::RelayMessage::Chat {
                        from: from_key,
                        from_name: Some(fname),
                        content,
                        timestamp: ts as u64,
                        signature: None,
                        channel: "DM".to_string(),
                        reply_to: None,
                        thread_count: None,
                    };
                    (id, "DM".to_string(), msg)
                })
                .collect();
                results.extend(dm_results);
            }

            // Sort combined results by timestamp DESC and truncate
            results.sort_by(|a, b| {
                let ts_a = Self::extract_timestamp(&a.2);
                let ts_b = Self::extract_timestamp(&b.2);
                ts_b.cmp(&ts_a)
            });
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Extract timestamp from a RelayMessage (helper for sorting).
    fn extract_timestamp(msg: &crate::relay::RelayMessage) -> u64 {
        match msg {
            crate::relay::RelayMessage::Chat { timestamp, .. } => *timestamp,
            crate::relay::RelayMessage::Dm { timestamp, .. } => *timestamp,
            _ => 0,
        }
    }

    /// Delete a message by its database row ID. Returns the from_key if found.
    /// If admin_key is Some, allows deleting anyone's message; otherwise only from_key must match.
    pub fn delete_message_by_id(&self, msg_id: i64, requester_key: &str, is_admin: bool) -> Result<Option<(String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Look up the message.
        let result = conn.query_row(
            "SELECT from_key, channel_id FROM messages WHERE id = ?1",
            params![msg_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok((from_key, channel_id)) => {
                if from_key == requester_key || is_admin {
                    conn.execute("DELETE FROM messages WHERE id = ?1", params![msg_id])?;
                    Ok(Some((from_key, channel_id)))
                } else {
                    Ok(None) // Not authorized
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Save user status (online/away/busy/dnd) and status text.
    pub fn save_user_status(&self, name: &str, status: &str, status_text: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_status (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                status TEXT NOT NULL DEFAULT 'online',
                status_text TEXT NOT NULL DEFAULT ''
            );"
        )?;
        conn.execute(
            "INSERT INTO user_status (name, status, status_text) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET status = ?2, status_text = ?3",
            params![name, status, status_text],
        )?;
        Ok(())
    }

    /// Load user status. Returns (status, status_text) or None.
    pub fn load_user_status(&self, name: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // Table might not exist yet.
        match conn.query_row(
            "SELECT status, status_text FROM user_status WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok(result) => Ok(Some(result)),
            Err(_) => Ok(None),
        }
    }

    /// Clear status text on disconnect (but keep status preference).
    pub fn clear_user_status_text(&self, name: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE user_status SET status_text = '' WHERE name = ?1 COLLATE NOCASE",
            params![name],
        );
        Ok(())
    }

    /// Get the count of pinned messages in a channel.
    pub fn get_pinned_count(&self, channel: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM pinned_messages WHERE channel = ?1",
            params![channel],
            |row| row.get(0),
        )
    }

    /// Get the last chat message in a channel (for /pin command).
    pub fn get_last_message_in_channel(&self, channel: &str) -> Result<Option<(String, String, String, u64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT from_key, from_name, content, timestamp FROM messages
             WHERE channel_id = ?1 AND msg_type = 'chat'
             ORDER BY id DESC LIMIT 1",
            params![channel],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1).unwrap_or_default(),
                row.get::<_, String>(2).unwrap_or_default(),
                row.get::<_, i64>(3)? as u64,
            )),
        ) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the timestamp of a user's last message in a specific channel.
    pub fn get_last_user_message_timestamp(&self, from_key: &str, channel_id: &str) -> Result<Option<u64>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT timestamp FROM messages WHERE from_key = ?1 AND channel_id = ?2 AND msg_type = 'chat' ORDER BY id DESC LIMIT 1",
            params![from_key, channel_id],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(ts) => Ok(Some(ts as u64)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
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

    // ── Direct Message methods ──

    /// Store a direct message. Returns the new row id.
    pub fn store_dm(
        &self,
        from_key: &str,
        from_name: &str,
        to_key: &str,
        content: &str,
        timestamp: u64,
    ) -> Result<i64, rusqlite::Error> {
        self.store_dm_e2ee(from_key, from_name, to_key, content, timestamp, false, None)
    }

    /// Store a DM with optional E2EE metadata.
    pub fn store_dm_e2ee(
        &self,
        from_key: &str,
        from_name: &str,
        to_key: &str,
        content: &str,
        timestamp: u64,
        encrypted: bool,
        nonce: Option<&str>,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO direct_messages (from_key, from_name, to_key, content, timestamp, encrypted, nonce)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![from_key, from_name, to_key, content, timestamp as i64, encrypted as i32, nonce],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Load DM conversation between two users (both directions), ordered by timestamp ASC.
    /// Accepts either public keys or names — resolves by name if the value matches a registered name.
    pub fn load_dm_conversation(
        &self,
        key1: &str,
        key2: &str,
        limit: usize,
    ) -> Result<Vec<DmRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT from_key, from_name, to_key, content, timestamp, COALESCE(encrypted, 0), nonce FROM (
                SELECT from_key, from_name, to_key, content, timestamp, encrypted, nonce FROM direct_messages
                WHERE (from_key = ?1 AND to_key = ?2) OR (from_key = ?2 AND to_key = ?1)
                ORDER BY timestamp DESC
                LIMIT ?3
            ) sub ORDER BY timestamp ASC"
        )?;
        let records = stmt.query_map(params![key1, key2, limit], |row| {
            Ok(DmRecord {
                from_key: row.get(0)?,
                from_name: row.get(1)?,
                to_key: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get::<_, i64>(4)? as u64,
                encrypted: row.get::<_, i32>(5)? != 0,
                nonce: row.get(6)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(records)
    }

    /// Load DM conversation by name — finds ALL keys for both names and loads messages between any combination.
    pub fn load_dm_conversation_by_name(
        &self,
        name1: &str,
        name2: &str,
        limit: usize,
    ) -> Result<Vec<DmRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT from_key, from_name, to_key, content, timestamp, COALESCE(encrypted, 0), nonce FROM (
                SELECT from_key, from_name, to_key, content, timestamp, encrypted, nonce FROM direct_messages
                WHERE (from_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE)
                       AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE))
                   OR (from_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE)
                       AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE))
                ORDER BY timestamp DESC
                LIMIT ?3
            ) sub ORDER BY timestamp ASC"
        )?;
        let records = stmt.query_map(params![name1, name2, limit], |row| {
            Ok(DmRecord {
                from_key: row.get(0)?,
                from_name: row.get(1)?,
                to_key: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get::<_, i64>(4)? as u64,
                encrypted: row.get::<_, i32>(5)? != 0,
                nonce: row.get(6)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(records)
    }

    /// List all DM conversations for a user, with last message preview and unread count.
    /// Resolves by name: finds ALL keys for the user's name and aggregates conversations by partner name.
    pub fn get_dm_conversations(&self, my_key: &str) -> Result<Vec<DmConversation>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Look up my name from my key.
        let my_name: Option<String> = conn.query_row(
            "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
            params![my_key],
            |row| row.get(0),
        ).ok();

        // Get all my keys (all keys registered to my name).
        let my_keys: Vec<String> = if let Some(ref name) = my_name {
            let mut stmt = conn.prepare(
                "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
            )?;
            stmt.query_map(params![name], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            vec![my_key.to_string()]
        };

        // Build a comma-separated placeholder for IN clause.
        // SQLite doesn't support array params, so we build it dynamically.
        let placeholders: Vec<String> = my_keys.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let in_clause = placeholders.join(",");

        // Find all distinct partner keys from DMs involving any of my keys.
        let query = format!(
            "SELECT partner_key, MAX(timestamp) as last_ts FROM (
                SELECT to_key as partner_key, timestamp FROM direct_messages WHERE from_key IN ({0})
                UNION ALL
                SELECT from_key as partner_key, timestamp FROM direct_messages WHERE to_key IN ({0})
            ) WHERE partner_key NOT IN ({0}) GROUP BY partner_key ORDER BY last_ts DESC",
            in_clause
        );
        let mut stmt = conn.prepare(&query)?;
        let partners: Vec<(String, i64)> = stmt.query_map(
            rusqlite::params_from_iter(my_keys.iter().chain(my_keys.iter()).chain(my_keys.iter())),
            |row| Ok((row.get(0)?, row.get(1)?))
        )?.filter_map(|r| r.ok()).collect();

        // Group partners by name to merge multi-key users into single conversations.
        let mut seen_names: HashMap<String, usize> = HashMap::new();
        let mut conversations = Vec::new();

        for (partner_key, _last_ts) in &partners {
            // Resolve partner name.
            let partner_name: String = conn.query_row(
                "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                params![partner_key],
                |row| row.get(0),
            ).unwrap_or_else(|_| {
                conn.query_row(
                    "SELECT from_name FROM direct_messages WHERE from_key = ?1 ORDER BY timestamp DESC LIMIT 1",
                    params![partner_key],
                    |row| row.get(0),
                ).unwrap_or_else(|_| partner_key[..8.min(partner_key.len())].to_string())
            });

            let name_lower = partner_name.to_lowercase();

            // If we've already seen this name, skip (the first one has the most recent timestamp).
            if seen_names.contains_key(&name_lower) {
                continue;
            }

            // Get ALL keys for this partner name.
            let partner_keys: Vec<String> = conn.prepare(
                "SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE"
            )?.query_map(params![partner_name], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            let partner_keys = if partner_keys.is_empty() { vec![partner_key.clone()] } else { partner_keys };

            // Build dynamic query for last message across all key combinations.
            let my_ph: Vec<String> = (1..=my_keys.len()).map(|i| format!("?{}", i)).collect();
            let p_ph: Vec<String> = (my_keys.len()+1..=my_keys.len()+partner_keys.len()).map(|i| format!("?{}", i)).collect();
            let last_q = format!(
                "SELECT from_name, content, timestamp FROM direct_messages
                 WHERE (from_key IN ({}) AND to_key IN ({})) OR (from_key IN ({}) AND to_key IN ({}))
                 ORDER BY timestamp DESC LIMIT 1",
                my_ph.join(","), p_ph.join(","), p_ph.join(","), my_ph.join(",")
            );
            let all_params: Vec<String> = my_keys.iter().chain(partner_keys.iter()).cloned().collect();
            let last_msg: Option<(String, String, i64)> = conn.prepare(&last_q)?
                .query_row(rusqlite::params_from_iter(&all_params), |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                }).ok();

            // Unread count: messages FROM any partner key TO any of my keys that are unread.
            let unread_q = format!(
                "SELECT COUNT(*) FROM direct_messages
                 WHERE from_key IN ({}) AND to_key IN ({}) AND read = 0",
                p_ph.join(","), my_ph.join(",")
            );
            // For this query, partner keys come first, then my keys.
            let unread_params: Vec<String> = partner_keys.iter().chain(my_keys.iter()).cloned().collect();
            let unread_count: i64 = conn.prepare(&unread_q)?
                .query_row(rusqlite::params_from_iter(&unread_params), |row| row.get(0))
                .unwrap_or(0);

            if let Some((_, content, timestamp)) = last_msg {
                seen_names.insert(name_lower, conversations.len());
                conversations.push(DmConversation {
                    partner_key: partner_key.clone(),
                    partner_name,
                    last_message: content,
                    last_timestamp: timestamp as u64,
                    unread_count,
                });
            }
        }
        Ok(conversations)
    }

    /// Mark all DMs FROM from_key TO to_key as read.
    pub fn mark_dms_read(&self, from_key: &str, to_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE direct_messages SET read = 1 WHERE from_key = ?1 AND to_key = ?2 AND read = 0",
            params![from_key, to_key],
        )?;
        Ok(())
    }

    /// Mark DMs as read by name — marks messages from any of the partner's keys to any of the reader's keys.
    pub fn mark_dms_read_by_name(&self, partner_name: &str, reader_name: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE direct_messages SET read = 1
             WHERE from_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE)
               AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE)
               AND read = 0",
            params![partner_name, reader_name],
        )?;
        Ok(())
    }

    /// Look up the name for a public key.
    pub fn name_for_key(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
            params![public_key],
            |row| row.get(0),
        ) {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ── ECDH Public Key methods (E2EE DMs) ──

    /// Store or update the ECDH P-256 public key for a given Ed25519 public key.
    pub fn store_ecdh_public(&self, public_key: &str, ecdh_public: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE registered_names SET ecdh_public = ?1 WHERE public_key = ?2",
            params![ecdh_public, public_key],
        )?;
        Ok(())
    }

    /// Get the ECDH P-256 public key for a given Ed25519 public key.
    pub fn get_ecdh_public(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT ecdh_public FROM registered_names WHERE public_key = ?1 AND ecdh_public IS NOT NULL LIMIT 1",
            params![public_key],
            |row| row.get(0),
        ) {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ── Server state (key-value) methods ──

    /// Get a server state value by key.
    pub fn get_state(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT value FROM server_state WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ) {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Set a server state value.
    pub fn set_state(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO server_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    // ── User data sync methods ──

    /// Save user data blob (JSON string). Upserts by public key.
    pub fn save_user_data(&self, public_key: &str, data: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT INTO user_data (public_key, data, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(public_key) DO UPDATE SET data = ?2, updated_at = ?3",
            params![public_key, data, now],
        )?;
        Ok(())
    }

    /// Load user data blob. Returns (data_json, updated_at) or None.
    pub fn load_user_data(&self, public_key: &str) -> Result<Option<(String, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT data, updated_at FROM user_data WHERE public_key = ?1",
            params![public_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ) {
            Ok(result) => Ok(Some(result)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ── Federation methods ──

    /// A federated server record.
    #[allow(dead_code)]
    pub fn add_federated_server(
        &self,
        server_id: &str,
        name: &str,
        url: &str,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "INSERT OR IGNORE INTO federated_servers (server_id, name, url, trust_tier, added_at)
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![server_id, name, url, now],
        )?;
        Ok(rows > 0)
    }

    /// Remove a federated server by ID.
    pub fn remove_federated_server(&self, server_id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM federated_servers WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(rows > 0)
    }

    /// List all federated servers, ordered by trust tier DESC then name.
    pub fn list_federated_servers(&self) -> Result<Vec<FederatedServer>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT server_id, name, url, public_key, trust_tier, accord_compliant, status, last_seen, added_at
             FROM federated_servers
             ORDER BY trust_tier DESC, name ASC"
        )?;
        let servers = stmt.query_map([], |row| {
            Ok(FederatedServer {
                server_id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                public_key: row.get(3)?,
                trust_tier: row.get(4)?,
                accord_compliant: row.get::<_, i32>(5)? != 0,
                status: row.get(6)?,
                last_seen: row.get(7)?,
                added_at: row.get(8)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(servers)
    }

    /// Set the trust tier for a federated server.
    pub fn set_server_trust_tier(&self, server_id: &str, tier: i32) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE federated_servers SET trust_tier = ?1 WHERE server_id = ?2",
            params![tier, server_id],
        )?;
        Ok(rows > 0)
    }

    /// Update a federated server's status and last_seen.
    pub fn update_federated_server_status(&self, server_id: &str, status: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "UPDATE federated_servers SET status = ?1, last_seen = ?2 WHERE server_id = ?3",
            params![status, now, server_id],
        )?;
        Ok(rows > 0)
    }

    /// Update a federated server's info from a server-info response.
    pub fn update_federated_server_info(
        &self,
        server_id: &str,
        name: &str,
        public_key: Option<&str>,
        accord_compliant: bool,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "UPDATE federated_servers SET name = ?1, public_key = ?2, accord_compliant = ?3, status = 'online', last_seen = ?4 WHERE server_id = ?5",
            params![name, public_key, accord_compliant as i32, now, server_id],
        )?;
        Ok(rows > 0)
    }

    // ── Voice Channel methods ──

    /// Create a voice channel. Returns the new channel ID.
    pub fn create_voice_channel(&self, name: &str, created_by: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM voice_channels",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        conn.execute(
            "INSERT INTO voice_channels (name, position, created_by, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, max_pos + 1, created_by, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Delete a voice channel by ID.
    pub fn delete_voice_channel(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM voice_channels WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// List all voice channels, ordered by position.
    pub fn list_voice_channels(&self) -> Result<Vec<VoiceChannelRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, position, created_by, created_at FROM voice_channels ORDER BY position ASC, id ASC"
        )?;
        let channels = stmt.query_map([], |row| {
            Ok(VoiceChannelRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                position: row.get(2)?,
                created_by: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(channels)
    }

    /// Rename a voice channel.
    pub fn rename_voice_channel(&self, id: i64, new_name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE voice_channels SET name = ?1 WHERE id = ?2",
            params![new_name, id],
        )?;
        Ok(rows > 0)
    }

    /// Check if a voice channel exists.
    pub fn voice_channel_exists(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT 1 FROM voice_channels WHERE id = ?1",
            params![id],
            |_| Ok(()),
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get the server's own Ed25519 keypair (generated on first call, stored in server_state).
    /// Returns (public_key_hex, secret_key_hex).
    pub fn get_or_create_server_keypair(&self) -> Result<(String, String), rusqlite::Error> {
        // Check if already stored.
        if let Some(pk) = self.get_state("server_public_key")? {
            if let Some(sk) = self.get_state("server_secret_key")? {
                return Ok((pk, sk));
            }
        }
        // Generate new keypair using random bytes.
        use ed25519_dalek::SigningKey;
        let secret_bytes: [u8; 32] = rand::rng().random();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let public_key = signing_key.verifying_key();
        let pk_hex = hex::encode(public_key.as_bytes());
        let sk_hex = hex::encode(signing_key.to_bytes());
        self.set_state("server_public_key", &pk_hex)?;
        self.set_state("server_secret_key", &sk_hex)?;
        info!("Generated server Ed25519 keypair: {}", pk_hex);
        Ok((pk_hex, sk_hex))
    }
    // ── Channel Category methods ──

    /// Create a channel category. Returns the new category ID.
    pub fn create_category(&self, name: &str, position: i32) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channel_categories (name, position) VALUES (?1, ?2)",
            params![name, position],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Delete a channel category by name. Channels in it become uncategorized.
    pub fn delete_category(&self, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cat_id: Option<i64> = conn.query_row(
            "SELECT id FROM channel_categories WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |row| row.get(0),
        ).ok();
        if let Some(id) = cat_id {
            conn.execute("UPDATE channels SET category_id = NULL WHERE category_id = ?1", params![id])?;
            conn.execute("DELETE FROM channel_categories WHERE id = ?1", params![id])?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Rename a category.
    pub fn rename_category(&self, old_name: &str, new_name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE channel_categories SET name = ?1 WHERE name = ?2 COLLATE NOCASE",
            params![new_name, old_name],
        )?;
        Ok(rows > 0)
    }

    /// Set a channel's category.
    pub fn set_channel_category(&self, channel_id: &str, category_name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cat_id: Option<i64> = conn.query_row(
            "SELECT id FROM channel_categories WHERE name = ?1 COLLATE NOCASE",
            params![category_name],
            |row| row.get(0),
        ).ok();
        if let Some(id) = cat_id {
            let rows = conn.execute(
                "UPDATE channels SET category_id = ?1 WHERE id = ?2",
                params![id, channel_id],
            )?;
            Ok(rows > 0)
        } else {
            Ok(false)
        }
    }

    /// List all categories ordered by position.
    pub fn list_categories(&self) -> Result<Vec<(i64, String, i32)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, position FROM channel_categories ORDER BY position ASC, id ASC"
        )?;
        let cats = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(cats)
    }

    // ── Link Preview Cache methods ──

    /// Get cached link preview for a URL.
    pub fn get_link_preview(&self, url: &str) -> Result<Option<LinkPreviewRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT url, title, description, image, site_name, fetched_at FROM link_previews WHERE url = ?1",
            params![url],
            |row| Ok(LinkPreviewRecord {
                url: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                image: row.get(3)?,
                site_name: row.get(4)?,
                fetched_at: row.get(5)?,
            }),
        ) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Cache a link preview.
    pub fn cache_link_preview(&self, url: &str, title: Option<&str>, description: Option<&str>, image: Option<&str>, site_name: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT OR REPLACE INTO link_previews (url, title, description, image, site_name, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![url, title, description, image, site_name, now],
        )?;
        Ok(())
    }

    // ── Project Board: Task methods ──

    /// Create a new task. Returns the new task ID.
    pub fn create_task(
        &self,
        title: &str,
        description: &str,
        status: &str,
        priority: &str,
        assignee: Option<&str>,
        created_by: &str,
        labels: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        // Position: max position in status column + 1.
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM project_tasks WHERE status = ?1",
            params![status],
            |row| row.get(0),
        ).unwrap_or(0);
        conn.execute(
            "INSERT INTO project_tasks (title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9)",
            params![title, description, status, priority, assignee, created_by, now, max_pos + 1, labels],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Update an existing task.
    pub fn update_task(
        &self,
        id: i64,
        title: &str,
        description: &str,
        priority: &str,
        assignee: Option<&str>,
        labels: &str,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let rows = conn.execute(
            "UPDATE project_tasks SET title = ?1, description = ?2, priority = ?3, assignee = ?4, labels = ?5, updated_at = ?6 WHERE id = ?7",
            params![title, description, priority, assignee, labels, now, id],
        )?;
        Ok(rows > 0)
    }

    /// Move a task to a new status column.
    pub fn move_task(&self, id: i64, new_status: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM project_tasks WHERE status = ?1",
            params![new_status],
            |row| row.get(0),
        ).unwrap_or(0);
        let rows = conn.execute(
            "UPDATE project_tasks SET status = ?1, position = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_status, max_pos + 1, now, id],
        )?;
        Ok(rows > 0)
    }

    /// Delete a task and its comments.
    pub fn delete_task(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM task_comments WHERE task_id = ?1", params![id])?;
        let rows = conn.execute("DELETE FROM project_tasks WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// List all tasks, ordered by status then position.
    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels
             FROM project_tasks
             ORDER BY position ASC, id ASC"
        )?;
        let tasks = stmt.query_map([], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
                priority: row.get(4)?,
                assignee: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                position: row.get(9)?,
                labels: row.get(10)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(tasks)
    }

    /// Get a single task by ID.
    pub fn get_task(&self, id: i64) -> Result<Option<TaskRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT id, title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels
             FROM project_tasks WHERE id = ?1",
            params![id],
            |row| Ok(TaskRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
                priority: row.get(4)?,
                assignee: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                position: row.get(9)?,
                labels: row.get(10)?,
            }),
        ) {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Add a comment to a task.
    pub fn add_task_comment(
        &self,
        task_id: i64,
        author_key: &str,
        author_name: &str,
        content: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT INTO task_comments (task_id, author_key, author_name, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![task_id, author_key, author_name, content, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get comments for a task, ordered by created_at ASC.
    pub fn get_task_comments(&self, task_id: i64) -> Result<Vec<TaskCommentRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, task_id, author_key, author_name, content, created_at
             FROM task_comments WHERE task_id = ?1 ORDER BY created_at ASC"
        )?;
        let comments = stmt.query_map(params![task_id], |row| {
            Ok(TaskCommentRecord {
                id: row.get(0)?,
                task_id: row.get(1)?,
                author_key: row.get(2)?,
                author_name: row.get(3)?,
                content: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(comments)
    }

    /// Get comment count per task (for display on cards).
    pub fn get_task_comment_counts(&self) -> Result<HashMap<i64, i64>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT task_id, COUNT(*) FROM task_comments GROUP BY task_id"
        )?;
        let counts: HashMap<i64, i64> = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(counts)
    }

    // ── Follow/Friend System ──

    /// Add a follow relationship. Returns true if newly created.
    pub fn add_follow(&self, follower_key: &str, followed_key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = now_millis().to_string();
        let rows = conn.execute(
            "INSERT OR IGNORE INTO follows (follower_key, followed_key, created_at) VALUES (?1, ?2, ?3)",
            params![follower_key, followed_key, now],
        )?;
        Ok(rows > 0)
    }

    /// Remove a follow relationship. Returns true if actually removed.
    pub fn remove_follow(&self, follower_key: &str, followed_key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM follows WHERE follower_key = ?1 AND followed_key = ?2",
            params![follower_key, followed_key],
        )?;
        Ok(rows > 0)
    }

    /// Get list of keys that `user_key` is following.
    pub fn get_following(&self, user_key: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT followed_key FROM follows WHERE follower_key = ?1")?;
        let keys: Vec<String> = stmt.query_map(params![user_key], |row| row.get(0))?
            .filter_map(|r| r.ok()).collect();
        Ok(keys)
    }

    /// Get list of keys that follow `user_key`.
    pub fn get_followers(&self, user_key: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT follower_key FROM follows WHERE followed_key = ?1")?;
        let keys: Vec<String> = stmt.query_map(params![user_key], |row| row.get(0))?
            .filter_map(|r| r.ok()).collect();
        Ok(keys)
    }

    /// Check if two users are mutual followers (friends).
    pub fn are_friends(&self, key_a: &str, key_b: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM follows WHERE
             (follower_key = ?1 AND followed_key = ?2) OR
             (follower_key = ?2 AND followed_key = ?1)",
            params![key_a, key_b],
            |row| row.get(0),
        )?;
        Ok(count >= 2)
    }

    // ── Group System ──

    /// Create a new group. Returns the group id and invite code.
    pub fn create_group(&self, name: &str, creator_key: &str) -> Result<(String, String), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let id = format!("grp_{:08x}", rand::rng().random::<u32>());
        let invite_code = format!("{:06x}", rand::rng().random::<u32>() & 0xFFFFFF);
        let now = now_millis().to_string();
        conn.execute(
            "INSERT INTO groups (id, name, creator_key, created_at, invite_code) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, creator_key, now, invite_code],
        )?;
        conn.execute(
            "INSERT INTO group_members (group_id, member_key, role, joined_at) VALUES (?1, ?2, 'admin', ?3)",
            params![id, creator_key, now],
        )?;
        Ok((id, invite_code))
    }

    /// Join a group by invite code. Returns (group_id, group_name) on success.
    pub fn join_group_by_invite(&self, invite_code: &str, member_key: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let result: Option<(String, String)> = conn.query_row(
            "SELECT id, name FROM groups WHERE invite_code = ?1",
            params![invite_code],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        if let Some((ref gid, _)) = result {
            let now = now_millis().to_string();
            conn.execute(
                "INSERT OR IGNORE INTO group_members (group_id, member_key, role, joined_at) VALUES (?1, ?2, 'member', ?3)",
                params![gid, member_key, now],
            )?;
        }
        Ok(result)
    }

    /// Leave a group.
    pub fn leave_group(&self, group_id: &str, member_key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM group_members WHERE group_id = ?1 AND member_key = ?2",
            params![group_id, member_key],
        )?;
        Ok(rows > 0)
    }

    /// Get groups that a user is a member of.
    pub fn get_user_groups(&self, member_key: &str) -> Result<Vec<(String, String, String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT g.id, g.name, COALESCE(g.invite_code, ''), gm.role FROM groups g
             JOIN group_members gm ON g.id = gm.group_id
             WHERE gm.member_key = ?1 ORDER BY g.name"
        )?;
        let groups = stmt.query_map(params![member_key], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(groups)
    }

    /// Get members of a group.
    pub fn get_group_members(&self, group_id: &str) -> Result<Vec<(String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT member_key, role FROM group_members WHERE group_id = ?1"
        )?;
        let members = stmt.query_map(params![group_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(members)
    }

    /// Check if a user is a member of a group.
    pub fn is_group_member(&self, group_id: &str, member_key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM group_members WHERE group_id = ?1 AND member_key = ?2",
            params![group_id, member_key],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Store a group message.
    pub fn store_group_message(&self, group_id: &str, from_key: &str, from_name: &str, content: &str, timestamp: u64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO group_messages (group_id, from_key, from_name, content, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![group_id, from_key, from_name, content, timestamp],
        )?;
        Ok(())
    }

    // ── Marketplace methods ──

    /// Create a marketplace listing.
    pub fn create_listing(
        &self,
        id: &str,
        seller_key: &str,
        seller_name: &str,
        title: &str,
        description: &str,
        category: &str,
        condition: &str,
        price: &str,
        payment_methods: &str,
        location: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO marketplace_listings (id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'active', datetime('now'))",
            params![id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location],
        )?;
        Ok(())
    }

    /// Update a marketplace listing. Returns true if updated.
    pub fn update_listing(
        &self,
        id: &str,
        seller_key: &str,
        title: &str,
        description: &str,
        category: &str,
        condition: &str,
        price: &str,
        payment_methods: &str,
        location: &str,
        status: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = if is_admin {
            conn.execute(
                "UPDATE marketplace_listings SET title=?1, description=?2, category=?3, condition=?4, price=?5, payment_methods=?6, location=?7, status=COALESCE(?8, status), updated_at=datetime('now') WHERE id=?9",
                params![title, description, category, condition, price, payment_methods, location, status, id],
            )?
        } else {
            conn.execute(
                "UPDATE marketplace_listings SET title=?1, description=?2, category=?3, condition=?4, price=?5, payment_methods=?6, location=?7, status=COALESCE(?8, status), updated_at=datetime('now') WHERE id=?9 AND seller_key=?10",
                params![title, description, category, condition, price, payment_methods, location, status, id, seller_key],
            )?
        };
        Ok(rows > 0)
    }

    /// Delete a marketplace listing. Returns true if deleted.
    pub fn delete_listing(&self, id: &str, seller_key: &str, is_admin: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = if is_admin {
            conn.execute("DELETE FROM marketplace_listings WHERE id=?1", params![id])?
        } else {
            conn.execute("DELETE FROM marketplace_listings WHERE id=?1 AND seller_key=?2", params![id, seller_key])?
        };
        Ok(rows > 0)
    }

    /// Get all marketplace listings, optionally filtered.
    pub fn get_listings(&self, category: Option<&str>, status: Option<&str>, limit: usize) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let query = format!(
            "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
             FROM marketplace_listings
             WHERE 1=1 {} {}
             ORDER BY created_at DESC
             LIMIT ?1",
            if category.is_some() { "AND category = ?2" } else { "" },
            if status.is_some() { if category.is_some() { "AND status = ?3" } else { "AND status = ?2" } } else { "" },
        );
        let mut stmt = conn.prepare(&query)?;
        let listings = if let Some(cat) = category {
            if let Some(st) = status {
                stmt.query_map(params![limit, cat, st], map_listing_row)?
            } else {
                stmt.query_map(params![limit, cat], map_listing_row)?
            }
        } else if let Some(st) = status {
            stmt.query_map(params![limit, st], map_listing_row)?
        } else {
            stmt.query_map(params![limit], map_listing_row)?
        };
        Ok(listings.filter_map(|r| r.ok()).collect())
    }

    /// Get a single listing by ID.
    pub fn get_listing_by_id(&self, id: &str) -> Result<Option<MarketplaceListing>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
             FROM marketplace_listings WHERE id=?1",
            params![id],
            map_listing_row,
        ) {
            Ok(l) => Ok(Some(l)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get listings for a specific seller.
    pub fn get_user_listings(&self, seller_key: &str) -> Result<Vec<MarketplaceListing>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at
             FROM marketplace_listings WHERE seller_key=?1 ORDER BY created_at DESC"
        )?;
        let listings = stmt.query_map(params![seller_key], map_listing_row)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(listings)
    }

    /// Load recent group messages.
    pub fn load_group_messages(&self, group_id: &str, limit: usize) -> Result<Vec<(String, String, String, u64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT from_key, from_name, content, timestamp FROM group_messages
             WHERE group_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let mut messages: Vec<(String, String, String, u64)> = stmt.query_map(params![group_id, limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.filter_map(|r| r.ok()).collect();
        messages.reverse();
        Ok(messages)
    }

    // ── Friend Code System ──

    /// Characters for friend codes (no 0/O/1/I/l confusion).
    const FRIEND_CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    /// Create a friend code for a user. Returns the code string.
    /// Rate limited to max 5 active codes per user.
    pub fn create_friend_code(&self, public_key: &str, expires_at: u64, max_uses: i32) -> Result<String, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_millis();

        // Clean up expired codes first.
        let _ = conn.execute("DELETE FROM friend_codes WHERE expires_at < ?1", params![now as i64]);

        // Check rate limit: max 5 active codes per user.
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM friend_codes WHERE public_key = ?1",
            params![public_key],
            |row| row.get(0),
        ).unwrap_or(0);
        if count >= 5 {
            return Err("You already have 5 active friend codes. Wait for them to expire.".to_string());
        }

        // Generate 8-char code from safe alphabet.
        let mut code = String::with_capacity(8);
        let chars = Self::FRIEND_CODE_CHARS;
        use rand::Rng;
        let mut rng = rand::rng();
        for _ in 0..8 {
            let idx = rng.random_range(0..chars.len());
            code.push(chars[idx] as char);
        }

        conn.execute(
            "INSERT INTO friend_codes (code, public_key, created_at, expires_at, uses_remaining) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![code, public_key, now as i64, expires_at as i64, max_uses],
        ).map_err(|e| format!("DB error: {e}"))?;

        Ok(code)
    }

    /// Redeem a friend code. Returns Ok(Some((owner_public_key, owner_name))) on success.
    pub fn redeem_friend_code(&self, code: &str) -> Result<Option<(String, Option<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = now_millis() as i64;

        // Look up the code (case-insensitive).
        let result = conn.query_row(
            "SELECT public_key, uses_remaining FROM friend_codes WHERE code = ?1 COLLATE NOCASE AND expires_at > ?2",
            params![code, now],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        );

        match result {
            Ok((owner_key, uses)) => {
                if uses <= 1 {
                    conn.execute("DELETE FROM friend_codes WHERE code = ?1 COLLATE NOCASE", params![code])?;
                } else {
                    conn.execute(
                        "UPDATE friend_codes SET uses_remaining = uses_remaining - 1 WHERE code = ?1 COLLATE NOCASE",
                        params![code],
                    )?;
                }

                // Look up owner's name.
                let owner_name: Option<String> = conn.query_row(
                    "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                    params![owner_key],
                    |row| row.get(0),
                ).ok();

                Ok(Some((owner_key, owner_name)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Clean up expired friend codes.
    pub fn cleanup_expired_friend_codes(&self) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = now_millis() as i64;
        let rows = conn.execute("DELETE FROM friend_codes WHERE expires_at < ?1", params![now])?;
        Ok(rows)
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A project task record from the database.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub position: i64,
    pub labels: String,
}

/// A task comment record from the database.
#[derive(Debug, Clone)]
pub struct TaskCommentRecord {
    pub id: i64,
    pub task_id: i64,
    pub author_key: String,
    pub author_name: String,
    pub content: String,
    pub created_at: i64,
}

/// A cached link preview record.
#[derive(Debug, Clone)]
pub struct LinkPreviewRecord {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub site_name: Option<String>,
    pub fetched_at: i64,
}

/// A voice channel record from the database.
#[derive(Debug, Clone)]
pub struct VoiceChannelRecord {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub created_by: Option<String>,
    pub created_at: i64,
}

/// A marketplace listing record from the database.
#[derive(Debug, Clone)]
pub struct MarketplaceListing {
    pub id: String,
    pub seller_key: String,
    pub seller_name: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub condition: Option<String>,
    pub price: Option<String>,
    pub payment_methods: Option<String>,
    pub location: Option<String>,
    pub images: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

fn map_listing_row(row: &rusqlite::Row) -> rusqlite::Result<MarketplaceListing> {
    Ok(MarketplaceListing {
        id: row.get(0)?,
        seller_key: row.get(1)?,
        seller_name: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        category: row.get(5)?,
        condition: row.get(6)?,
        price: row.get(7)?,
        payment_methods: row.get(8)?,
        location: row.get(9)?,
        images: row.get(10)?,
        status: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

/// A federated server record from the database.
#[derive(Debug, Clone)]
pub struct FederatedServer {
    pub server_id: String,
    pub name: String,
    pub url: String,
    pub public_key: Option<String>,
    pub trust_tier: i32,
    pub accord_compliant: bool,
    pub status: String,
    pub last_seen: Option<i64>,
    pub added_at: i64,
}
