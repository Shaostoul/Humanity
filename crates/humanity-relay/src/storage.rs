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
                raw_json  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_messages_timestamp
                ON messages(timestamp);

            CREATE TABLE IF NOT EXISTS peers (
                public_key  TEXT PRIMARY KEY,
                display_name TEXT,
                last_seen   INTEGER NOT NULL
            );"
        )?;

        info!("Database opened: {}", path.display());
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Store a message and return its row ID.
    pub fn store_message(&self, msg: &RelayMessage) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let raw = serde_json::to_string(msg).unwrap_or_default();

        match msg {
            RelayMessage::Chat { from, from_name, content, timestamp } => {
                conn.execute(
                    "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json)
                     VALUES ('chat', ?1, ?2, ?3, ?4, ?5)",
                    params![from, from_name, content, timestamp, raw],
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
