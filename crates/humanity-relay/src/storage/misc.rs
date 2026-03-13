use super::Storage;
use rand::Rng;
use rusqlite::params;

impl Storage {
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
    /// Resolves by name: finds the most recently updated data across all keys for the same user.
    pub fn load_user_data(&self, public_key: &str) -> Result<Option<(String, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        // First try: load data from any key belonging to the same name (most recent wins).
        let result = conn.query_row(
            "SELECT ud.data, ud.updated_at FROM user_data ud
             INNER JOIN registered_names rn ON ud.public_key = rn.public_key
             WHERE rn.name = (SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1)
             ORDER BY ud.updated_at DESC LIMIT 1",
            params![public_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        );
        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Fallback: try direct key lookup (for unregistered users).
                match conn.query_row(
                    "SELECT data, updated_at FROM user_data WHERE public_key = ?1",
                    params![public_key],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                ) {
                    Ok(data) => Ok(Some(data)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            }
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
    pub fn list_federated_servers(&self) -> Result<Vec<super::FederatedServer>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT server_id, name, url, public_key, trust_tier, accord_compliant, status, last_seen, added_at
             FROM federated_servers
             ORDER BY trust_tier DESC, name ASC"
        )?;
        let servers = stmt.query_map([], |row| {
            Ok(super::FederatedServer {
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

    // ── Channel Federation methods ──

    /// Mark a channel as federated (or un-federate it).
    pub fn set_channel_federated(&self, channel_id: &str, federated: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE channels SET federated = ?1 WHERE id = ?2",
            params![federated as i32, channel_id],
        )?;
        Ok(rows > 0)
    }

    /// Check if a channel is federated.
    pub fn is_channel_federated(&self, channel_id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let val: i32 = conn.query_row(
            "SELECT COALESCE(federated, 0) FROM channels WHERE id = ?1",
            params![channel_id],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(val != 0)
    }

    /// Get all federated channel IDs.
    pub fn get_federated_channels(&self) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM channels WHERE federated = 1")?;
        let ids = stmt.query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
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
    pub fn list_voice_channels(&self) -> Result<Vec<super::VoiceChannelRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, position, created_by, created_at FROM voice_channels ORDER BY position ASC, id ASC"
        )?;
        let channels = stmt.query_map([], |row| {
            Ok(super::VoiceChannelRecord {
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
        tracing::info!("Generated server Ed25519 keypair: {}", pk_hex);
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
    pub fn get_link_preview(&self, url: &str) -> Result<Option<super::LinkPreviewRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT url, title, description, image, site_name, fetched_at FROM link_previews WHERE url = ?1",
            params![url],
            |row| Ok(super::LinkPreviewRecord {
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
}
