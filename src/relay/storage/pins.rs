use super::Storage;
use super::PinnedMessageRecord;
use rusqlite::params;
use std::collections::HashMap;

impl Storage {
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
        self.with_conn(|conn| {
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
        })
    }

    /// Unpin a message by its 1-based index in the channel's pin list (ordered by pinned_at).
    /// Returns true if removed.
    pub fn unpin_message(&self, channel: &str, index: usize) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Get all pinned messages for a channel, ordered by pinned_at ASC.
    pub fn get_pinned_messages(&self, channel: &str) -> Result<Vec<PinnedMessageRecord>, rusqlite::Error> {
        // Read-only: SELECT + query_map. Read pool.
        self.with_read_conn(|conn| {
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
        })
    }

    /// Search messages by content, optionally filtered by channel.
    pub fn search_messages(&self, query: &str, channel: Option<&str>, limit: usize) -> Result<Vec<(i64, String, crate::relay::relay::RelayMessage)>, rusqlite::Error> {
        // Read-only: SELECT + query_map (both branches). Read pool.
        self.with_read_conn(|conn| {
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
                    serde_json::from_str::<crate::relay::relay::RelayMessage>(&raw).ok().map(|msg| (id, ch, msg))
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
                    serde_json::from_str::<crate::relay::relay::RelayMessage>(&raw).ok().map(|msg| (id, ch, msg))
                })
                .collect();
                Ok(results)
            }
        })
    }

    /// Search messages with full filtering: query, channel, from (sender name), limit.
    /// Supports native FTS5 syntax (boolean AND/OR/NOT, "phrases", prefix*).
    /// Public channels only — DMs live in the sealed dm_mailbox (opaque
    /// ciphertext, no sender) and are searched client-side.
    pub fn search_messages_full(&self, query: &str, channel: Option<&str>, from_name: Option<&str>, limit: usize, _requester_key: &str) -> Result<Vec<(i64, String, crate::relay::relay::RelayMessage)>, rusqlite::Error> {
        // Read-only: FTS5 MATCH (with LIKE fallback). No writes anywhere in
        // the closure. Read pool.
        self.with_read_conn(|conn| {
            let limit = limit.min(100);

            // Build FTS5 query: support native syntax (boolean AND/OR/NOT, "phrases",
            // prefix*, column:filters) while folding channel/from_name into the MATCH
            // expression when possible for maximum index efficiency.
            let fts_query = {
                let mut parts: Vec<String> = Vec::new();
                // User's text query — pass through to FTS5 as-is for full syntax support.
                // Wrap in braces to keep operator precedence correct with ANDed filters.
                parts.push(format!("content : ({})", query));
                if let Some(ch) = channel {
                    // Channel filter via FTS5 column filter — uses the index, not a table scan.
                    let escaped_ch = ch.replace('"', "\"\"");
                    parts.push(format!("channel_id : \"{}\"", escaped_ch));
                }
                if let Some(fname) = from_name {
                    let escaped_fn = fname.replace('"', "\"\"");
                    parts.push(format!("from_name : \"{}\"", escaped_fn));
                }
                parts.join(" AND ")
            };

            // Try FTS5 first (fast full-text index). Falls back to LIKE if unavailable.
            let results: Vec<(i64, String, crate::relay::relay::RelayMessage)> = {
                let fts_sql =
                    "SELECT m.id, m.channel_id, m.raw_json \
                     FROM messages_fts \
                     JOIN messages m ON messages_fts.rowid = m.id \
                     WHERE messages_fts MATCH ?1 AND m.msg_type = 'chat' \
                     ORDER BY messages_fts.rank \
                     LIMIT ?2";

                match conn.prepare(fts_sql).and_then(|mut s| {
                    s.query_map(rusqlite::params![fts_query, limit as i64], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                    }).map(|rows| {
                        rows.filter_map(|r| r.ok())
                            .filter_map(|(id, ch, raw)| {
                                serde_json::from_str::<crate::relay::relay::RelayMessage>(&raw)
                                    .ok().map(|msg| (id, ch, msg))
                            })
                            .collect::<Vec<_>>()
                    })
                }) {
                    Ok(v) => v,
                    // FTS5 unavailable or query syntax error — fall back to LIKE
                    Err(_) => {
                        let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                        let mut sql = String::from(
                            "SELECT id, channel_id, raw_json FROM messages \
                             WHERE msg_type = 'chat' AND content LIKE '%' || ?1 || '%' ESCAPE '\\'"
                        );
                        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(escaped.clone())];
                        let mut pidx = 2u32;
                        if let Some(ch) = channel {
                            sql.push_str(&format!(" AND channel_id = ?{pidx}"));
                            params.push(Box::new(ch.to_string()));
                            pidx += 1;
                        }
                        if let Some(fname) = from_name {
                            let ef = fname.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                            sql.push_str(&format!(" AND from_name LIKE '%' || ?{pidx} || '%' ESCAPE '\\'"));
                            params.push(Box::new(ef));
                            pidx += 1;
                        }
                        sql.push_str(&format!(" ORDER BY timestamp DESC LIMIT ?{pidx}"));
                        params.push(Box::new(limit as i64));
                        let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
                        conn.prepare(&sql)?.query_map(refs.as_slice(), |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                        })?.filter_map(|r| r.ok())
                        .filter_map(|(id, ch, raw)| {
                            serde_json::from_str::<crate::relay::relay::RelayMessage>(&raw)
                                .ok().map(|msg| (id, ch, msg))
                        })
                        .collect()
                    }
                }
            };

            // DMs are NOT searchable server-side (sealed-sender cutover,
            // 2026-08-23): the dm_mailbox holds only opaque ciphertext with
            // no sender, so a content LIKE would match nothing meaningful.
            // DM search is a client-side concern over the local history store.

            Ok(results)
        })
    }

    /// Delete a message by its database row ID. Returns the from_key if found.
    /// If admin_key is Some, allows deleting anyone's message; otherwise only from_key must match.
    pub fn delete_message_by_id(&self, msg_id: i64, requester_key: &str, is_admin: bool) -> Result<Option<(String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Save user status (online/away/busy/dnd) and status text.
    pub fn save_user_status(&self, name: &str, status: &str, status_text: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Load user status. Returns (status, status_text) or None.
    pub fn load_user_status(&self, name: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        // Read-only single-row lookup. The user_status table is created in
        // Storage::open migrations, so the read-only pooled connection sees it;
        // the closure also folds any error to Ok(None) by existing design. Read pool.
        self.with_read_conn(|conn| {
            // Table might not exist yet.
            match conn.query_row(
                "SELECT status, status_text FROM user_status WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ) {
                Ok(result) => Ok(Some(result)),
                Err(_) => Ok(None),
            }
        })
    }

    /// Clear status text on disconnect (but keep status preference).
    pub fn clear_user_status_text(&self, name: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            let _ = conn.execute(
                "UPDATE user_status SET status_text = '' WHERE name = ?1 COLLATE NOCASE",
                params![name],
            );
            Ok(())
        })
    }

    /// Get the count of pinned messages in a channel.
    pub fn get_pinned_count(&self, channel: &str) -> Result<i64, rusqlite::Error> {
        // Read-only COUNT. Read pool.
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM pinned_messages WHERE channel = ?1",
                params![channel],
                |row| row.get(0),
            )
        })
    }

    /// Get the last chat message in a channel (for /pin command).
    pub fn get_last_message_in_channel(&self, channel: &str) -> Result<Option<(String, String, String, u64)>, rusqlite::Error> {
        // Read-only single-row lookup (/pin command). Read pool.
        self.with_read_conn(|conn| {
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
        })
    }

    /// Get the timestamp of a user's last message in a specific channel.
    pub fn get_last_user_message_timestamp(&self, from_key: &str, channel_id: &str) -> Result<Option<u64>, rusqlite::Error> {
        // Read-only single-row lookup (per-user rate/slow-mode check). Read pool.
        self.with_read_conn(|conn| {
            match conn.query_row(
                "SELECT timestamp FROM messages WHERE from_key = ?1 AND channel_id = ?2 AND msg_type = 'chat' ORDER BY id DESC LIMIT 1",
                params![from_key, channel_id],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(ts) => Ok(Some(ts as u64)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// List all registered users with their first public key.
    /// Returns Vec<(name, first_key, role, key_count)> sorted alphabetically.
    pub fn list_all_users_with_keys(&self) -> Result<Vec<(String, String, String, usize)>, rusqlite::Error> {
        // Read-only: SELECT + in-memory grouping (drives broadcast_full_user_list,
        // the sidebar). No writes. Read pool.
        self.with_read_conn(|conn| {
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
        })
    }
}
