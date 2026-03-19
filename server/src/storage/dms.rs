use super::Storage;
use super::{DmRecord, DmConversation};
use rusqlite::params;
use std::collections::HashMap;

impl Storage {
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
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO direct_messages (from_key, from_name, to_key, content, timestamp, encrypted, nonce)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![from_key, from_name, to_key, content, timestamp as i64, encrypted as i32, nonce],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Load DM conversation between two users (both directions), ordered by timestamp ASC.
    /// Accepts either public keys or names — resolves by name if the value matches a registered name.
    pub fn load_dm_conversation(
        &self,
        key1: &str,
        key2: &str,
        limit: usize,
    ) -> Result<Vec<DmRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Load DM conversation by name — finds ALL keys for both names and loads messages between any combination.
    pub fn load_dm_conversation_by_name(
        &self,
        name1: &str,
        name2: &str,
        limit: usize,
    ) -> Result<Vec<DmRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// List all DM conversations for a user, with last message preview and unread count.
    /// Resolves by name: finds ALL keys for the user's name and aggregates conversations by partner name.
    pub fn get_dm_conversations(&self, my_key: &str) -> Result<Vec<DmConversation>, rusqlite::Error> {
        self.with_conn(|conn| {
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
            let in_clause = my_keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");

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
        })
    }

    /// Mark all DMs FROM from_key TO to_key as read.
    pub fn mark_dms_read(&self, from_key: &str, to_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE direct_messages SET read = 1 WHERE from_key = ?1 AND to_key = ?2 AND read = 0",
                params![from_key, to_key],
            )?;
            Ok(())
        })
    }

    /// Mark DMs as read by name — marks messages from any of the partner's keys to any of the reader's keys.
    pub fn mark_dms_read_by_name(&self, partner_name: &str, reader_name: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE direct_messages SET read = 1
                 WHERE from_key IN (SELECT public_key FROM registered_names WHERE name = ?1 COLLATE NOCASE)
                   AND to_key IN (SELECT public_key FROM registered_names WHERE name = ?2 COLLATE NOCASE)
                   AND read = 0",
                params![partner_name, reader_name],
            )?;
            Ok(())
        })
    }

    /// Look up the name for a public key.
    pub fn name_for_key(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(name) => Ok(Some(name)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    // ── ECDH Public Key methods (E2EE DMs) ──

    /// Store or update the ECDH P-256 public key for a given Ed25519 public key.
    pub fn store_ecdh_public(&self, public_key: &str, ecdh_public: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE registered_names SET ecdh_public = ?1 WHERE public_key = ?2",
                params![ecdh_public, public_key],
            )?;
            Ok(())
        })
    }

    /// Get the ECDH P-256 public key for a given Ed25519 public key.
    pub fn get_ecdh_public(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT ecdh_public FROM registered_names WHERE public_key = ?1 AND ecdh_public IS NOT NULL LIMIT 1",
                params![public_key],
                |row| row.get(0),
            ) {
                Ok(key) => Ok(Some(key)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}
