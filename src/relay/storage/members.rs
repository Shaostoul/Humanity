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

/// Shared SQL for the public member-directory opt-out (audit 2026-06-12). A member is
/// visible UNLESS their profile's privacy JSON sets `directory:"unlisted"`. The list,
/// count, and single-member queries all use these SAME fragments so pagination totals
/// stay consistent with the listing. LEFT JOIN to `profiles` (keyed by name, COLLATE
/// NOCASE) so a member with no profile row (NULL privacy) stays listed, listed is the
/// default and opt-out is explicit (you appear in a server's directory when you JOIN
/// it). `json_extract` returns NULL for an absent key OR malformed JSON, both treated
/// as listed; `save_profile_extended` already rejects invalid privacy JSON so a stored
/// `unlisted` is always honored.
const MEMBER_DIR_JOIN: &str =
    " FROM server_members m LEFT JOIN profiles p ON m.name = p.name COLLATE NOCASE ";
const MEMBER_DIR_VISIBLE: &str =
    "(json_extract(p.privacy, '$.directory') IS NULL OR json_extract(p.privacy, '$.directory') <> 'unlisted')";

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

    /// Purge known test-bot rows from `server_members` AND
    /// `registered_names`. Runs at relay startup so accumulated
    /// AISampleBot / TestBot / SampleBot rows from
    /// `scripts/ai-sample-client.js` runs don't pollute the user list.
    /// Returns (server_members_deleted, registered_names_deleted).
    ///
    /// We must clean BOTH tables because:
    ///   - `server_members` is the kick/membership source-of-truth
    ///   - `registered_names` drives the visible "full user list" sidebar
    ///     (see `list_all_users_with_keys()` in pins.rs which the
    ///     `broadcast_full_user_list` handler uses). Without purging
    ///     `registered_names`, kicked bots stay in the sidebar forever.
    pub fn purge_test_bot_members(&self) -> Result<(usize, usize), rusqlite::Error> {
        self.with_conn(|conn| {
            let members_deleted = conn.execute(
                "DELETE FROM server_members WHERE \
                    name LIKE 'AISampleBot%' OR \
                    name LIKE 'TestBot%' OR \
                    name LIKE 'SampleBot%'",
                [],
            )?;
            let names_deleted = conn.execute(
                "DELETE FROM registered_names WHERE \
                    name LIKE 'AISampleBot%' COLLATE NOCASE OR \
                    name LIKE 'TestBot%' COLLATE NOCASE OR \
                    name LIKE 'SampleBot%' COLLATE NOCASE",
                [],
            )?;
            Ok((members_deleted, names_deleted))
        })
    }

    /// Delete a user's registered-name row(s) by public_key. Used by the
    /// kick handler to make sure the visible user list (sourced from
    /// `registered_names`) is consistent with the membership table.
    pub fn delete_registered_name(&self, public_key: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let changed = conn.execute(
                "DELETE FROM registered_names WHERE public_key = ?1",
                params![public_key],
            )?;
            Ok(changed)
        })
    }

    /// Delete ALL registered-name rows matching a display name (case-
    /// insensitive). Used by the kick handler as a fallback when the
    /// target has no public_key (e.g. legacy rows with empty keys, or
    /// users whose key wasn't propagated to the client's user-modal).
    /// Operator-reported case 2026-05-12 — `DesktopUser_4000` had an
    /// empty key in `registered_names` so key-based kick no-op'd.
    pub fn delete_registered_names_by_name(&self, name: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| {
            let changed = conn.execute(
                "DELETE FROM registered_names WHERE name = ?1 COLLATE NOCASE",
                params![name],
            )?;
            Ok(changed)
        })
    }

    /// Get a paginated list of server members, optionally filtered by search term.
    pub fn get_members(
        &self,
        limit: usize,
        offset: usize,
        search: Option<&str>,
    ) -> Result<Vec<MemberRecord>, rusqlite::Error> {
        // Read-only: paginated SELECT (both search/no-search branches end in
        // query_map + collect). Member listing is a hot read; use the pool.
        self.with_read_conn(|conn| {
            if let Some(q) = search {
                let pattern = format!("%{}%", q);
                let sql = format!(
                    "SELECT m.public_key, m.name, m.role, m.joined_at, m.last_seen{MEMBER_DIR_JOIN}\
                     WHERE (m.name LIKE ?1 OR m.public_key LIKE ?1) AND {MEMBER_DIR_VISIBLE} \
                     ORDER BY m.joined_at DESC LIMIT ?2 OFFSET ?3"
                );
                let mut stmt = conn.prepare(&sql)?;
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
                let sql = format!(
                    "SELECT m.public_key, m.name, m.role, m.joined_at, m.last_seen{MEMBER_DIR_JOIN}\
                     WHERE {MEMBER_DIR_VISIBLE} \
                     ORDER BY m.joined_at DESC LIMIT ?1 OFFSET ?2"
                );
                let mut stmt = conn.prepare(&sql)?;
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
        // Read-only single-row lookup. Read pool. Honors the directory opt-out: an
        // unlisted member returns None, and get_member_by_key (api.rs) maps None -> 404.
        self.with_read_conn(|conn| {
            let sql = format!(
                "SELECT m.public_key, m.name, m.role, m.joined_at, m.last_seen{MEMBER_DIR_JOIN}\
                 WHERE m.public_key = ?1 AND {MEMBER_DIR_VISIBLE}"
            );
            conn.query_row(
                &sql,
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
        // Read-only COUNT. Read pool. Uses the SAME join + visibility filter as
        // get_members so the total matches the listed rows (no phantom pages).
        self.with_read_conn(|conn| {
            if let Some(q) = search {
                let pattern = format!("%{}%", q);
                let sql = format!(
                    "SELECT COUNT(*){MEMBER_DIR_JOIN}\
                     WHERE (m.name LIKE ?1 OR m.public_key LIKE ?1) AND {MEMBER_DIR_VISIBLE}"
                );
                conn.query_row(&sql, params![pattern], |row| row.get(0))
            } else {
                let sql = format!("SELECT COUNT(*){MEMBER_DIR_JOIN}WHERE {MEMBER_DIR_VISIBLE}");
                conn.query_row(&sql, [], |row| row.get(0))
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
        // Read-only COUNT (seller profile view). Read pool.
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM marketplace_listings WHERE seller_key = ?1 AND status = 'active'",
                params![seller_key],
                |row| row.get(0),
            )
        })
    }

    /// Get the N most recently joined members (for admin dashboard).
    ///
    /// Intentionally NOT filtered by the directory opt-out: this feeds the operator's
    /// admin dashboard, which should see every join (the opt-out only hides a member
    /// from the PUBLIC `/api/members` directory, not from the server's own admins).
    pub fn recent_joins(&self, limit: usize) -> Result<Vec<serde_json::Value>, rusqlite::Error> {
        // Read-only: SELECT + query_map + collect (admin dashboard). Read pool.
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, name, role, joined_at
                 FROM server_members
                 ORDER BY joined_at DESC
                 LIMIT ?1"
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                let public_key: String = row.get(0)?;
                let name: Option<String> = row.get(1)?;
                let role: String = row.get(2)?;
                let joined_at: String = row.get(3)?;
                Ok(serde_json::json!({
                    "public_key": public_key,
                    "name": name,
                    "role": role,
                    "joined_at": joined_at,
                }))
            })?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod directory_optout_tests {
    //! Public member-directory opt-out (audit 2026-06-12): a member with
    //! profile privacy `directory:"unlisted"` is hidden from /api/members,
    //! its count, and the single-member lookup; everyone else stays listed.
    //! Also proves json_extract is available in this build (first storage use).
    use super::*;

    fn test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_members_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    #[test]
    fn unlisted_member_hidden_everywhere_else_listed() {
        let db = test_storage();
        db.join_server("KEY_ALICE", "alice").unwrap();
        db.join_server("KEY_BOB", "bob").unwrap();
        // alice has no profile row at all (NULL privacy) -> stays listed (default).
        // bob opts out via profile privacy.
        db.save_profile_extended("bob", "", "{}", "", "", "", "", "", "{\"directory\":\"unlisted\"}")
            .unwrap();

        let listed = db.get_members(100, 0, None).unwrap();
        let names: Vec<String> = listed.iter().filter_map(|m| m.name.clone()).collect();
        assert!(names.contains(&"alice".to_string()), "alice (no profile) stays listed");
        assert!(!names.contains(&"bob".to_string()), "bob opted out -> hidden");
        assert_eq!(db.get_member_count(None).unwrap(), 1, "count matches the listed rows");
        assert!(db.get_member("KEY_BOB").unwrap().is_none(), "unlisted -> None (api maps to 404)");
        assert!(db.get_member("KEY_ALICE").unwrap().is_some(), "listed -> Some");

        // A member who sets privacy WITHOUT the directory key stays listed.
        db.join_server("KEY_CAROL", "carol").unwrap();
        db.save_profile_extended("carol", "", "{}", "", "", "", "", "", "{\"location\":\"private\"}")
            .unwrap();
        assert_eq!(db.get_member_count(None).unwrap(), 2, "carol (privacy, no directory key) listed");

        // The search path honors the filter too (no surfacing an unlisted member).
        assert!(db.get_members(100, 0, Some("bob")).unwrap().is_empty(), "search hides unlisted");
        assert_eq!(db.get_member_count(Some("bob")).unwrap(), 0, "search count hides unlisted");
    }
}
