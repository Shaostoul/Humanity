use super::Storage;
use rand::Rng;
use rusqlite::params;
use crate::relay::relay::RelayMessage;

impl Storage {
    /// Store a message and return its row ID.
    pub fn store_message(&self, msg: &RelayMessage) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Load recent messages (most recent `limit`, ordered oldest first).
    pub fn load_recent_messages(&self, limit: usize) -> Result<Vec<RelayMessage>, rusqlite::Error> {
        // Read-only: SELECT + query_map. Read pool.
        self.with_read_conn(|conn| {
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
        })
    }

    /// Load messages after a given row ID (for API polling).
    pub fn load_messages_after(&self, after_id: i64, limit: usize) -> Result<(Vec<RelayMessage>, i64), rusqlite::Error> {
        // Read-only: SELECT + query_map (API polling cursor). Read pool.
        self.with_read_conn(|conn| {
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
        })
    }

    /// Get the current max message ID (for cursor).
    pub fn max_message_id(&self) -> Result<i64, rusqlite::Error> {
        // Read-only MAX (cursor). Read pool.
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT COALESCE(MAX(id), 0) FROM messages",
                [],
                |row| row.get(0),
            )
        })
    }

    /// Record a peer's last-seen timestamp.
    pub fn upsert_peer(&self, public_key: &str, display_name: Option<&str>, timestamp: i64) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO peers (public_key, display_name, last_seen)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(public_key)
                 DO UPDATE SET display_name = COALESCE(?2, display_name), last_seen = ?3",
                params![public_key, display_name, timestamp],
            )?;
            Ok(())
        })
    }

    /// Check if a name is registered and whether the given key is authorized for it.
    /// Returns: Ok(None) if name is free, Ok(Some(true)) if key is authorized,
    /// Ok(Some(false)) if name is taken by other keys.
    pub fn check_name(&self, name: &str, public_key: &str) -> Result<Option<bool>, rusqlite::Error> {
        // Read-only: two COUNT lookups (name availability check). Read pool.
        self.with_read_conn(|conn| {
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
        })
    }

    /// Register a name for a public key.
    pub fn register_name(&self, name: &str, public_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            conn.execute(
                "INSERT OR IGNORE INTO registered_names (name, public_key, registered_at) VALUES (?1, ?2, ?3)",
                params![name, public_key, now],
            )?;
            Ok(())
        })
    }

    /// Get the EARLIEST `registered_at` timestamp (epoch ms) for a public
    /// key — i.e., when this identity first claimed any name on this relay.
    /// Returns `Ok(None)` when the pubkey has no registered_names row yet
    /// (genuinely new identity).
    ///
    /// Used by the v0.280.0 anti-spam time-gate: newly-registered
    /// identities are blocked from posting in public channels for the
    /// first N seconds. We take MIN across rows because a single pubkey
    /// can legitimately hold multiple names (link-code flow); the
    /// EARLIEST registration is the right "first seen as a participant"
    /// signal.
    pub fn first_registered_at_for_key(&self, public_key: &str) -> Result<Option<i64>, rusqlite::Error> {
        // Read-only MIN aggregate (anti-spam time-gate). Read pool.
        self.with_read_conn(|conn| {
            // MIN(...) returns NULL when there are no rows, which we
            // surface as Ok(None) — query_row would otherwise complain
            // about NoRows for an empty SELECT. Wrapping the value in
            // Option<i64> handles both "no rows" and "rows but NULL".
            let result: Option<i64> = conn.query_row(
                "SELECT MIN(registered_at) FROM registered_names WHERE public_key = ?1",
                params![public_key],
                |row| row.get::<_, Option<i64>>(0),
            )?;
            Ok(result)
        })
    }

    /// True when `public_key` has at least one row in `registered_names`.
    /// Faster than `first_registered_at_for_key` when the caller only
    /// needs the boolean. v0.280.0: anti-spam gate for new-identity-per-IP
    /// uses this to decide whether to count this identify as "novel".
    pub fn pubkey_is_registered(&self, public_key: &str) -> Result<bool, rusqlite::Error> {
        // Read-only COUNT (anti-spam novelty check). Read pool.
        self.with_read_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM registered_names WHERE public_key = ?1",
                params![public_key],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Create a link code for adding a new device to an existing name.
    /// Returns the generated code.
    pub fn create_link_code(&self, name: &str, created_by: &str) -> Result<String, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Redeem a link code: if valid, register the new key under the name.
    /// Returns Ok(Some(name)) on success, Ok(None) if code is invalid/expired.
    pub fn redeem_link_code(&self, code: &str, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Store a federated chat message with origin server tag.
    /// These persist across restarts so federated history isn't lost.
    pub fn store_federated_message(
        &self,
        channel: &str,
        from_name: &str,
        from_key: &str,
        content: &str,
        timestamp: u64,
        raw_json: &str,
        origin_server: &str,
    ) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id, origin_server)
                 VALUES ('federated_chat', ?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
                params![from_key, from_name, content, timestamp as i64, raw_json, channel, origin_server],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Rebuild the FTS5 full-text index from the messages table.
    /// Idempotent — safe to call on every startup or after bulk imports.
    pub fn rebuild_fts_index(&self) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES('rebuild')")?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod anti_spam_helpers_tests {
    //! Coverage for the v0.280.0 anti-spam time-gate helpers
    //! (`first_registered_at_for_key`, `pubkey_is_registered`). The
    //! relay-side gates that USE these helpers are in `relay.rs`; here
    //! we only confirm the storage primitives behave correctly across
    //! the absent / single-name / multi-name cases.
    use super::*;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_anti_spam_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    #[test]
    fn first_registered_at_returns_none_for_unknown_key() {
        let db = fresh_db();
        // No identify has ever occurred. The time-gate code reads this
        // as "no registered_names row yet" and skips the gate (a brand-
        // new identity that hasn't claimed a name yet falls through to
        // existing checks). Most importantly: it does NOT return Err.
        let result = db.first_registered_at_for_key("never_seen_pubkey").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn first_registered_at_returns_registration_time() {
        let db = fresh_db();
        // The fresh schema seeds at least one channel; we use a fresh
        // pubkey + name pair so we own the row's timestamp.
        let before_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        db.register_name("alice_test", "pk_alice").expect("register");
        let result = db.first_registered_at_for_key("pk_alice").unwrap();
        let ts = result.expect("expected a timestamp");
        // Should be ~now (within a generous 5-second window for slow CI).
        assert!(ts >= before_ms - 5_000, "ts {} too old (before_ms {})", ts, before_ms);
        assert!(ts <= before_ms + 5_000, "ts {} too far in future", ts);
    }

    #[test]
    fn first_registered_at_picks_earliest_across_names() {
        // A pubkey can hold multiple names via the link-code flow. The
        // gate cares about WHEN this identity first joined, so MIN is
        // the right reduction — the latest name doesn't reset the
        // grace clock.
        let db = fresh_db();
        db.register_name("alice", "pk_multi").expect("first reg");
        // Tiny sleep so the second registration has a measurably later
        // timestamp on systems where ms granularity is precise.
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.register_name("alice2", "pk_multi").expect("second reg");
        let first = db.first_registered_at_for_key("pk_multi").unwrap().expect("ts");

        // Pull both timestamps directly and verify MIN matches the
        // helper's output.
        let both: Vec<i64> = db.with_conn(|c| {
            let mut stmt = c.prepare("SELECT registered_at FROM registered_names WHERE public_key = ?1")?;
            let rows = stmt.query_map(["pk_multi"], |r| r.get::<_, i64>(0))?;
            let mut out = Vec::new();
            for r in rows { out.push(r?); }
            Ok::<Vec<i64>, rusqlite::Error>(out)
        }).unwrap();
        assert_eq!(both.len(), 2, "expected two rows");
        let expected_min = *both.iter().min().unwrap();
        assert_eq!(first, expected_min);
    }

    #[test]
    fn pubkey_is_registered_distinguishes_seen_and_unseen() {
        let db = fresh_db();
        assert!(!db.pubkey_is_registered("pk_new").unwrap());
        db.register_name("bob", "pk_new").unwrap();
        assert!(db.pubkey_is_registered("pk_new").unwrap());
    }
}
