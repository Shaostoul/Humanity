use super::Storage;
use rusqlite::{params, OptionalExtension};

/// A guild record from the database.
#[derive(Debug, Clone)]
pub struct GuildRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_key: String,
    pub icon: String,
    pub color: String,
    pub created_at: String,
    pub member_count: i64,
}

/// A guild member record from the database.
#[derive(Debug, Clone)]
pub struct GuildMemberRecord {
    pub guild_id: String,
    pub public_key: String,
    pub role: String,
    pub joined_at: String,
    pub name: Option<String>,
}

/// A guild invite record from the database.
#[derive(Debug, Clone)]
pub struct GuildInviteRecord {
    pub id: String,
    pub guild_id: String,
    pub created_by: String,
    pub code: String,
    pub uses_remaining: i64,
    pub expires_at: i64,
}

impl Storage {
    // ── Guild CRUD ──

    /// Create a new guild.
    pub fn create_guild(
        &self,
        id: &str,
        name: &str,
        description: &str,
        owner_key: &str,
        icon: &str,
        color: &str,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO guilds (id, name, description, owner_key, icon, color, created_at, member_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), 1)",
                params![id, name, description, owner_key, icon, color],
            )?;
            // Owner is automatically a member with 'owner' role.
            conn.execute(
                "INSERT INTO guild_members (guild_id, public_key, role, joined_at)
                 VALUES (?1, ?2, 'owner', datetime('now'))",
                params![id, owner_key],
            )?;
            Ok(())
        })
    }

    /// Get a guild by ID.
    pub fn get_guild(&self, id: &str) -> Result<Option<GuildRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, name, description, owner_key, icon, color, created_at, member_count
                 FROM guilds WHERE id = ?1",
                params![id],
                |row| Ok(GuildRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    owner_key: row.get(3)?,
                    icon: row.get(4)?,
                    color: row.get(5)?,
                    created_at: row.get(6)?,
                    member_count: row.get(7)?,
                }),
            ).optional()
        })
    }

    /// Update a guild. Only the owner can update.
    pub fn update_guild(
        &self,
        id: &str,
        owner_key: &str,
        name: &str,
        description: &str,
        icon: &str,
        color: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE guilds SET name=?1, description=?2, icon=?3, color=?4
                 WHERE id=?5 AND owner_key=?6",
                params![name, description, icon, color, id, owner_key],
            )?;
            Ok(rows > 0)
        })
    }

    /// Delete a guild. Only the owner can delete.
    pub fn delete_guild(&self, id: &str, owner_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            // Delete all members and invites first.
            conn.execute("DELETE FROM guild_members WHERE guild_id = ?1", params![id])?;
            conn.execute("DELETE FROM guild_invites WHERE guild_id = ?1", params![id])?;
            let rows = conn.execute(
                "DELETE FROM guilds WHERE id = ?1 AND owner_key = ?2",
                params![id, owner_key],
            )?;
            Ok(rows > 0)
        })
    }

    /// Join a guild. Returns true if the user was added.
    pub fn join_guild(&self, guild_id: &str, public_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let changed = conn.execute(
                "INSERT OR IGNORE INTO guild_members (guild_id, public_key, role, joined_at)
                 VALUES (?1, ?2, 'member', datetime('now'))",
                params![guild_id, public_key],
            )?;
            if changed > 0 {
                conn.execute(
                    "UPDATE guilds SET member_count = member_count + 1 WHERE id = ?1",
                    params![guild_id],
                )?;
            }
            Ok(changed > 0)
        })
    }

    /// Leave a guild. Returns true if the user was removed.
    /// Owners cannot leave — they must transfer or delete the guild.
    pub fn leave_guild(&self, guild_id: &str, public_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            // Check if user is the owner.
            let role: Option<String> = conn.query_row(
                "SELECT role FROM guild_members WHERE guild_id = ?1 AND public_key = ?2",
                params![guild_id, public_key],
                |row| row.get(0),
            ).optional()?;
            if role.as_deref() == Some("owner") {
                return Ok(false); // Owner can't leave
            }
            let changed = conn.execute(
                "DELETE FROM guild_members WHERE guild_id = ?1 AND public_key = ?2",
                params![guild_id, public_key],
            )?;
            if changed > 0 {
                conn.execute(
                    "UPDATE guilds SET member_count = MAX(0, member_count - 1) WHERE id = ?1",
                    params![guild_id],
                )?;
            }
            Ok(changed > 0)
        })
    }

    /// Get members of a guild.
    pub fn get_guild_members(
        &self,
        guild_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<GuildMemberRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT gm.guild_id, gm.public_key, gm.role, gm.joined_at, sm.name
                 FROM guild_members gm
                 LEFT JOIN server_members sm ON sm.public_key = gm.public_key
                 WHERE gm.guild_id = ?1
                 ORDER BY CASE gm.role
                   WHEN 'owner' THEN 0
                   WHEN 'officer' THEN 1
                   ELSE 2
                 END, gm.joined_at
                 LIMIT ?2 OFFSET ?3"
            )?;
            let rows = stmt.query_map(params![guild_id, limit as i64, offset as i64], |row| {
                Ok(GuildMemberRecord {
                    guild_id: row.get(0)?,
                    public_key: row.get(1)?,
                    role: row.get(2)?,
                    joined_at: row.get(3)?,
                    name: row.get(4)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Create an invite code for a guild.
    pub fn create_guild_invite(
        &self,
        id: &str,
        guild_id: &str,
        created_by: &str,
        code: &str,
        uses_remaining: i64,
        expires_at: i64,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO guild_invites (id, guild_id, created_by, code, uses_remaining, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, guild_id, created_by, code, uses_remaining, expires_at],
            )?;
            Ok(())
        })
    }

    /// Use an invite code to join a guild. Returns the guild_id if successful.
    pub fn use_guild_invite(&self, code: &str, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = super::now_millis() as i64;
            let invite: Option<(String, String, i64)> = conn.query_row(
                "SELECT id, guild_id, uses_remaining FROM guild_invites
                 WHERE code = ?1 AND expires_at > ?2 AND uses_remaining > 0",
                params![code, now],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).optional()?;

            if let Some((invite_id, guild_id, uses)) = invite {
                // Add member
                let added = conn.execute(
                    "INSERT OR IGNORE INTO guild_members (guild_id, public_key, role, joined_at)
                     VALUES (?1, ?2, 'member', datetime('now'))",
                    params![guild_id, public_key],
                )?;
                if added > 0 {
                    conn.execute(
                        "UPDATE guilds SET member_count = member_count + 1 WHERE id = ?1",
                        params![guild_id],
                    )?;
                }
                // Decrement uses
                if uses <= 1 {
                    conn.execute("DELETE FROM guild_invites WHERE id = ?1", params![invite_id])?;
                } else {
                    conn.execute(
                        "UPDATE guild_invites SET uses_remaining = uses_remaining - 1 WHERE id = ?1",
                        params![invite_id],
                    )?;
                }
                Ok(Some(guild_id))
            } else {
                Ok(None)
            }
        })
    }

    /// Get all guilds a user is a member of.
    pub fn get_guilds_for_user(&self, public_key: &str) -> Result<Vec<GuildRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT g.id, g.name, g.description, g.owner_key, g.icon, g.color, g.created_at, g.member_count
                 FROM guilds g
                 INNER JOIN guild_members gm ON g.id = gm.guild_id
                 WHERE gm.public_key = ?1
                 ORDER BY g.name"
            )?;
            let rows = stmt.query_map(params![public_key], |row| {
                Ok(GuildRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    owner_key: row.get(3)?,
                    icon: row.get(4)?,
                    color: row.get(5)?,
                    created_at: row.get(6)?,
                    member_count: row.get(7)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Search guilds by name.
    pub fn search_guilds(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<GuildRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let pattern = format!("%{}%", query);
            let mut stmt = conn.prepare(
                "SELECT id, name, description, owner_key, icon, color, created_at, member_count
                 FROM guilds
                 WHERE name LIKE ?1 OR description LIKE ?1
                 ORDER BY member_count DESC, name
                 LIMIT ?2 OFFSET ?3"
            )?;
            let rows = stmt.query_map(params![pattern, limit as i64, offset as i64], |row| {
                Ok(GuildRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    owner_key: row.get(3)?,
                    icon: row.get(4)?,
                    color: row.get(5)?,
                    created_at: row.get(6)?,
                    member_count: row.get(7)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Get all guilds (paginated).
    pub fn get_all_guilds(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<GuildRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, owner_key, icon, color, created_at, member_count
                 FROM guilds
                 ORDER BY member_count DESC, name
                 LIMIT ?1 OFFSET ?2"
            )?;
            let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
                Ok(GuildRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    owner_key: row.get(3)?,
                    icon: row.get(4)?,
                    color: row.get(5)?,
                    created_at: row.get(6)?,
                    member_count: row.get(7)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Check if a user is a member of a guild.
    pub fn is_guild_member(&self, guild_id: &str, public_key: &str) -> bool {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT 1 FROM guild_members WHERE guild_id = ?1 AND public_key = ?2",
                params![guild_id, public_key],
                |_| Ok(true),
            ).unwrap_or(false)
        })
    }

    /// Get a user's role in a guild.
    pub fn get_guild_member_role(&self, guild_id: &str, public_key: &str) -> Option<String> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT role FROM guild_members WHERE guild_id = ?1 AND public_key = ?2",
                params![guild_id, public_key],
                |row| row.get(0),
            ).optional().unwrap_or(None)
        })
    }
}
