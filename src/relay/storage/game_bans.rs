//! Game-world bans (v0.474), deliberately SEPARATE from chat bans.
//!
//! HumanityOS treats communication and play as different rights tiers: chat is
//! a RIGHT (free speech is guaranteed -- the chat ban path must never be widened
//! to silence anyone for game behavior), while playing on the shared 3D world is
//! a PRIVILEGE that can be revoked. This module is the storage half of that
//! separation. It is a near-mirror of the chat ban infra in `channels.rs`
//! (`BannedUser` / `ban_user` / `unban_user` / `is_banned` / `list_banned`) but
//! reads/writes a WHOLLY DISJOINT table (`game_banned_keys`) so the two systems
//! can never collide:
//!
//!   * A game-banned key is NOT in `banned_keys`, so it still passes
//!     `is_banned` at the identify handshake -> its socket stays open, chat +
//!     DMs flow normally.
//!   * The single enforcement point is `is_game_banned` inside
//!     `handle_game_join` (msg_handlers.rs), which runs BEFORE the world lock,
//!     so a game-banned player never spawns and never broadcasts a join.
//!
//! This mirrors the existing precedent of `muted_members` being a separate
//! table from `banned_keys` (channels.rs) so two structurally-similar features
//! stay isolated. See docs/design/characters-and-servers.md and the design
//! plan in the v0.474 release notes.

use super::{Storage, now_millis};
use rusqlite::{OptionalExtension, params};

/// One game-world ban. Serialized over the WS protocol as part of
/// `game_banned_list` so the Game Admin page can list bans + offer Unban.
/// Carries `reason` + `banned_by` for moderation audit (the chat `BannedUser`
/// does not), and a `character_id` for forward-compatible per-character bans
/// (NULL = account-wide, the v1 default).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameBan {
    pub public_key: String,
    /// Specific character this ban targets, or None for an account-wide game
    /// ban (the v1 default). The schema's composite PK keeps account-wide and
    /// per-character rows distinct (SQLite treats NULL as distinct in a PK).
    pub character_id: Option<String>,
    /// Why the player was game-banned (shown to admins; not the chat path).
    pub reason: String,
    /// public_key of the admin who issued the ban (audit trail).
    pub banned_by: String,
    /// Unix ms when the ban was applied.
    pub banned_at: i64,
}

impl Storage {
    /// Game-ban a public key (optionally scoped to one character). INSERT OR
    /// REPLACE so re-banning refreshes the reason + timestamp. No-op on an
    /// empty key. Records `banned_by` for audit. Reads/writes ONLY the
    /// `game_banned_keys` table -- never touches chat's `banned_keys`.
    pub fn game_ban(
        &self,
        public_key: &str,
        character_id: Option<&str>,
        reason: &str,
        banned_by: &str,
    ) -> Result<(), rusqlite::Error> {
        if public_key.is_empty() {
            return Ok(());
        }
        self.with_conn(|conn| {
            // Idempotent upsert via delete-then-insert. We can't rely on
            // INSERT OR REPLACE here: SQLite treats NULL primary-key values as
            // DISTINCT, so re-banning the same key account-wide (character_id =
            // NULL) would insert a duplicate row instead of refreshing. Clearing
            // the same scope first guarantees exactly one row per (key, scope).
            match character_id {
                Some(cid) => conn.execute(
                    "DELETE FROM game_banned_keys WHERE public_key = ?1 AND character_id = ?2",
                    params![public_key, cid],
                )?,
                None => conn.execute(
                    "DELETE FROM game_banned_keys WHERE public_key = ?1 AND character_id IS NULL",
                    params![public_key],
                )?,
            };
            conn.execute(
                "INSERT INTO game_banned_keys
                   (public_key, character_id, reason, banned_by, banned_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    public_key,
                    character_id,
                    reason,
                    banned_by,
                    now_millis() as i64,
                ],
            )?;
            Ok(())
        })
    }

    /// Lift a game ban. NULL-aware: an account-wide unban (character_id = None)
    /// only removes the account-wide row, a per-character unban only that
    /// character's row. Never affects chat access.
    pub fn game_unban(
        &self,
        public_key: &str,
        character_id: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            match character_id {
                Some(cid) => conn.execute(
                    "DELETE FROM game_banned_keys
                     WHERE public_key = ?1 AND character_id = ?2",
                    params![public_key, cid],
                )?,
                None => conn.execute(
                    "DELETE FROM game_banned_keys
                     WHERE public_key = ?1 AND character_id IS NULL",
                    params![public_key],
                )?,
            };
            Ok(())
        })
    }

    /// THE game-world enforcement read: is this key banned from the game world?
    /// Returns the most-recent matching ban (any character scope), or None.
    /// Called inside `handle_game_join` only -- never on the chat/identify path.
    /// Read-pool with a writer fallback (same durability pattern as
    /// `load_player_progress`).
    pub fn is_game_banned(&self, public_key: &str) -> Result<Option<GameBan>, rusqlite::Error> {
        fn map_row(r: &rusqlite::Row) -> rusqlite::Result<GameBan> {
            Ok(GameBan {
                public_key: r.get::<_, String>(0)?,
                character_id: r.get::<_, Option<String>>(1)?,
                reason: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                banned_by: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                banned_at: r.get::<_, i64>(4)?,
            })
        }
        const SQL: &str =
            "SELECT public_key, character_id, reason, banned_by, banned_at
             FROM game_banned_keys WHERE public_key = ?1
             ORDER BY banned_at DESC LIMIT 1";
        let read =
            self.with_read_conn(|conn| conn.query_row(SQL, params![public_key], map_row).optional());
        match read {
            Ok(v) => Ok(v),
            Err(_) => self
                .with_conn(|conn| conn.query_row(SQL, params![public_key], map_row).optional()),
        }
    }

    /// Every current game ban, newest first. Drives the Game Admin page list.
    pub fn list_game_bans(&self) -> Result<Vec<GameBan>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT public_key, character_id, reason, banned_by, banned_at
                 FROM game_banned_keys ORDER BY banned_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(GameBan {
                    public_key: row.get(0)?,
                    character_id: row.get::<_, Option<String>>(1)?,
                    reason: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    banned_by: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    banned_at: row.get(4)?,
                })
            })?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_gameban_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// Core loop: game_ban records the key + reason + audit, is_game_banned and
    /// list_game_bans see it, game_unban clears it. Verifies the schema + the
    /// NULL-scope (account-wide) SQL all execute at runtime.
    #[test]
    fn game_ban_roundtrip() {
        let db = fresh_db();
        assert!(db.is_game_banned("cheater_key").unwrap().is_none());

        db.game_ban("cheater_key", None, "speed hacking", "admin_key").unwrap();

        let ban = db.is_game_banned("cheater_key").unwrap().expect("banned");
        assert_eq!(ban.public_key, "cheater_key");
        assert_eq!(ban.reason, "speed hacking");
        assert_eq!(ban.banned_by, "admin_key");
        assert!(ban.character_id.is_none());
        assert_eq!(db.list_game_bans().unwrap().len(), 1);

        db.game_unban("cheater_key", None).unwrap();
        assert!(db.is_game_banned("cheater_key").unwrap().is_none());
        assert!(db.list_game_bans().unwrap().is_empty());
    }

    /// Re-banning the same key account-wide must REFRESH (one row), not
    /// duplicate. This is the SQLite-NULL-PK trap the delete-then-insert in
    /// game_ban guards against.
    #[test]
    fn account_wide_reban_does_not_duplicate() {
        let db = fresh_db();
        db.game_ban("k", None, "first", "a").unwrap();
        db.game_ban("k", None, "second", "a").unwrap();
        let list = db.list_game_bans().unwrap();
        assert_eq!(list.len(), 1, "re-ban should refresh, not duplicate");
        assert_eq!(list[0].reason, "second", "newest reason wins");
    }

    /// An empty key is a no-op (can't ban a keyless registration), mirroring
    /// the chat ban_user guard.
    #[test]
    fn empty_key_is_a_noop() {
        let db = fresh_db();
        db.game_ban("", None, "x", "a").unwrap();
        assert!(db.list_game_bans().unwrap().is_empty());
    }
}
