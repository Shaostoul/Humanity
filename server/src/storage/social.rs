use super::Storage;
use rand::Rng;
use rusqlite::{params, OptionalExtension};

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Storage {
    // ── Follow/Friend System ──

    /// Add a follow relationship. Returns true if newly created.
    pub fn add_follow(&self, follower_key: &str, followed_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let now = now_millis().to_string();
            let rows = conn.execute(
                "INSERT OR IGNORE INTO follows (follower_key, followed_key, created_at) VALUES (?1, ?2, ?3)",
                params![follower_key, followed_key, now],
            )?;
            Ok(rows > 0)
        })
    }

    /// Remove a follow relationship. Returns true if actually removed.
    pub fn remove_follow(&self, follower_key: &str, followed_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM follows WHERE follower_key = ?1 AND followed_key = ?2",
                params![follower_key, followed_key],
            )?;
            Ok(rows > 0)
        })
    }

    /// Get list of keys that `user_key` is following.
    pub fn get_following(&self, user_key: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT followed_key FROM follows WHERE follower_key = ?1")?;
            let keys: Vec<String> = stmt.query_map(params![user_key], |row| row.get(0))?
                .filter_map(|r| r.ok()).collect();
            Ok(keys)
        })
    }

    /// Get list of keys that follow `user_key`.
    pub fn get_followers(&self, user_key: &str) -> Result<Vec<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT follower_key FROM follows WHERE followed_key = ?1")?;
            let keys: Vec<String> = stmt.query_map(params![user_key], |row| row.get(0))?
                .filter_map(|r| r.ok()).collect();
            Ok(keys)
        })
    }

    /// Check if two users are mutual followers (friends).
    pub fn are_friends(&self, key_a: &str, key_b: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM follows WHERE
                 (follower_key = ?1 AND followed_key = ?2) OR
                 (follower_key = ?2 AND followed_key = ?1)",
                params![key_a, key_b],
                |row| row.get(0),
            )?;
            Ok(count >= 2)
        })
    }

    // ── Group System ──

    /// Create a new group. Returns the group id and invite code.
    pub fn create_group(&self, name: &str, creator_key: &str) -> Result<(String, String), rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Join a group by invite code. Returns (group_id, group_name) on success.
    pub fn join_group_by_invite(&self, invite_code: &str, member_key: &str) -> Result<Option<(String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
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
        })
    }

    /// Leave a group.
    pub fn leave_group(&self, group_id: &str, member_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM group_members WHERE group_id = ?1 AND member_key = ?2",
                params![group_id, member_key],
            )?;
            Ok(rows > 0)
        })
    }

    /// Get groups that a user is a member of.
    pub fn get_user_groups(&self, member_key: &str) -> Result<Vec<(String, String, String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT g.id, g.name, COALESCE(g.invite_code, ''), gm.role FROM groups g
                 JOIN group_members gm ON g.id = gm.group_id
                 WHERE gm.member_key = ?1 ORDER BY g.name"
            )?;
            let groups = stmt.query_map(params![member_key], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?.filter_map(|r| r.ok()).collect();
            Ok(groups)
        })
    }

    /// Get members of a group.
    pub fn get_group_members(&self, group_id: &str) -> Result<Vec<(String, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT member_key, role FROM group_members WHERE group_id = ?1"
            )?;
            let members = stmt.query_map(params![group_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?.filter_map(|r| r.ok()).collect();
            Ok(members)
        })
    }

    /// Check if a user is a member of a group.
    pub fn is_group_member(&self, group_id: &str, member_key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM group_members WHERE group_id = ?1 AND member_key = ?2",
                params![group_id, member_key],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Store a group message.
    pub fn store_group_message(&self, group_id: &str, from_key: &str, from_name: &str, content: &str, timestamp: u64) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO group_messages (group_id, from_key, from_name, content, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![group_id, from_key, from_name, content, timestamp],
            )?;
            Ok(())
        })
    }

    /// Load recent group messages.
    pub fn load_group_messages(&self, group_id: &str, limit: usize) -> Result<Vec<(String, String, String, u64)>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT from_key, from_name, content, timestamp FROM group_messages
                 WHERE group_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
            )?;
            let mut messages: Vec<(String, String, String, u64)> = stmt.query_map(params![group_id, limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?.filter_map(|r| r.ok()).collect();
            messages.reverse();
            Ok(messages)
        })
    }
}
