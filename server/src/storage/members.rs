use super::Storage;
use rusqlite::{params, OptionalExtension};

/// A server member record from the database.
#[derive(Debug, Clone)]
pub struct MemberRecord {
    pub public_key: String,
    pub name: Option<String>,
    pub role: String,
    pub joined_at: String,
    pub last_seen: Option<String>,
}

impl Storage {
    // ── Server Membership methods ──

    /// Join the server as a member. If already a member, this is a no-op.
    pub fn join_server(&self, public_key: &str, name: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let changed = conn.execute(
                "INSERT OR IGNORE INTO server_members (public_key, name, role, joined_at, last_seen)
                 VALUES (?1, ?2, 'member', datetime('now'), datetime('now'))",
                params![public_key, name],
            )?;
            Ok(changed > 0)
        })
    }

    /// Leave the server (delete member record).
    pub fn leave_server(&self, public_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let changed = conn.execute(
                "DELETE FROM server_members WHERE public_key = ?1",
                params![public_key],
            )?;
            Ok(changed > 0)
        })
    }

    /// Get a paginated list of server members, optionally filtered by search term.
    pub fn get_members(
        &self,
        limit: usize,
        offset: usize,
        search: Option<&str>,
    ) -> Result<Vec<MemberRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            if let Some(q) = search {
                let pattern = format!("%{}%", q);
                let mut stmt = conn.prepare(
                    "SELECT public_key, name, role, joined_at, last_seen
                     FROM server_members
                     WHERE name LIKE ?1 OR public_key LIKE ?1
                     ORDER BY joined_at DESC
                     LIMIT ?2 OFFSET ?3"
                )?;
                let rows = stmt.query_map(params![pattern, limit as i64, offset as i64], |row| {
                    Ok(MemberRecord {
                        public_key: row.get(0)?,
                        name: row.get(1)?,
                        role: row.get(2)?,
                        joined_at: row.get(3)?,
                        last_seen: row.get(4)?,
                    })
                })?;
                rows.collect()
            } else {
                let mut stmt = conn.prepare(
                    "SELECT public_key, name, role, joined_at, last_seen
                     FROM server_members
                     ORDER BY joined_at DESC
                     LIMIT ?1 OFFSET ?2"
                )?;
                let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
                    Ok(MemberRecord {
                        public_key: row.get(0)?,
                        name: row.get(1)?,
                        role: row.get(2)?,
                        joined_at: row.get(3)?,
                        last_seen: row.get(4)?,
                    })
                })?;
                rows.collect()
            }
        })
    }

    /// Get a single member by public key.
    pub fn get_member(&self, public_key: &str) -> Result<Option<MemberRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT public_key, name, role, joined_at, last_seen
                 FROM server_members WHERE public_key = ?1",
                params![public_key],
                |row| Ok(MemberRecord {
                    public_key: row.get(0)?,
                    name: row.get(1)?,
                    role: row.get(2)?,
                    joined_at: row.get(3)?,
                    last_seen: row.get(4)?,
                }),
            ).optional()
        })
    }

    /// Update last_seen timestamp for a member.
    pub fn update_last_seen(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE server_members SET last_seen = datetime('now') WHERE public_key = ?1",
                params![public_key],
            )?;
            Ok(())
        })
    }

    /// Get total member count, optionally filtered by search.
    pub fn get_member_count(&self, search: Option<&str>) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            if let Some(q) = search {
                let pattern = format!("%{}%", q);
                conn.query_row(
                    "SELECT COUNT(*) FROM server_members WHERE name LIKE ?1 OR public_key LIKE ?1",
                    params![pattern],
                    |row| row.get(0),
                )
            } else {
                conn.query_row(
                    "SELECT COUNT(*) FROM server_members",
                    [],
                    |row| row.get(0),
                )
            }
        })
    }

    /// Check if a public key is a server member.
    pub fn is_member(&self, public_key: &str) -> bool {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT 1 FROM server_members WHERE public_key = ?1",
                params![public_key],
                |_| Ok(true),
            ).unwrap_or(false)
        })
    }

    /// Update a member's name (when they change display name).
    pub fn update_member_name(&self, public_key: &str, name: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE server_members SET name = ?1 WHERE public_key = ?2",
                params![name, public_key],
            )?;
            Ok(())
        })
    }

    /// Get listing count for a specific seller (for seller profiles).
    pub fn get_seller_listing_count(&self, seller_key: &str) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM marketplace_listings WHERE seller_key = ?1 AND status = 'active'",
                params![seller_key],
                |row| row.get(0),
            )
        })
    }
}
