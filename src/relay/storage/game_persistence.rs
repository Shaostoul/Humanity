//! Durable persistence for the server-authoritative game world.
//!
//! Background: the relay's `GameWorld` (entities, positions, game_time, quest
//! state) lives entirely IN MEMORY (see `relay/handlers/game_state.rs`). Before
//! this module, a relay restart wiped the shared world AND every player's
//! quest/XP progress — players reconnected to a brand-new ship with zeroed
//! progress. This module gives the relay two dedicated SQLite tables so both
//! survive restarts:
//!
//!   * `game_world_snapshots` — one row per logical world (keyed by `world_id`).
//!     Stores the serialized entity set + `game_time` + `next_entity_id` as a
//!     single JSON blob. The whole authoritative world is one snapshot; we don't
//!     normalize entities into rows because the entity shape is a free-form
//!     `serde_json::Value` (components vary per entity type) and we always
//!     load/save the world wholesale, never per-entity.
//!
//!   * `player_progress` — one row per player (keyed by their `public_key`).
//!     Stores the RPG-ish progression that previously lived only inside the
//!     player entity's in-memory `components`: current quest id, the list of
//!     completed quests, XP, and reputation. A returning player is re-seeded
//!     from this row on `game_join` so they keep what they earned even if the
//!     world snapshot was invalidated (e.g. by a spawn-logic version bump).
//!
//! Both tables are created in `storage/mod.rs::open()` (guarded
//! `CREATE TABLE IF NOT EXISTS`), exactly like every other domain table.
//!
//! Scope note: this is PERSISTENCE ONLY. The server stays the single source of
//! truth for the live world; these methods just make that truth durable. No
//! zones / interest-management / anti-cheat / sharding here — those are later
//! increments.

use rusqlite::{OptionalExtension, params};

use super::{Storage, now_millis};

/// A persisted game-world snapshot row.
///
/// `snapshot_json` is the opaque serialized world blob (the caller in
/// `game_state.rs` decides its exact shape — currently entities + game_time +
/// next_entity_id). `game_time` and `next_entity_id` are ALSO stored as their
/// own columns so an operator (or a future admin tool) can inspect/inventory
/// worlds without parsing the JSON, and so a cheap "how far has this world
/// advanced?" query never needs a full deserialize.
#[derive(Debug, Clone)]
pub struct GameWorldSnapshot {
    /// Logical world identifier. Today there is one world; the column lets us
    /// host multiple (per-instance / per-shard) later without a schema change.
    pub world_id: String,
    /// Opaque serialized world JSON (entities + game_time + next_entity_id).
    pub snapshot_json: String,
    /// Simulation clock at save time (seconds since world start).
    pub game_time: f64,
    /// Next entity id to hand out — preserved so reused ids never collide with
    /// entities restored from the snapshot.
    pub next_entity_id: u64,
    /// Unix-millis timestamp of the last save (for debugging / staleness).
    pub updated_at: u64,
}

/// A persisted per-player progression row.
///
/// Mirrors the progression fields the game keeps inside a player entity's
/// in-memory `components`. `completed_quests` is stored as a JSON array string
/// (e.g. `["explore_ship","meet_the_crew"]`) — SQLite has no array type and
/// the rest of the codebase already stores list-shaped columns as JSON text
/// (see `project_tasks.labels`), so we match that convention.
#[derive(Debug, Clone)]
pub struct PlayerProgress {
    /// The player's Dilithium public-key hex — their canonical identity.
    pub public_key: String,
    /// The id of the quest the player is currently on (e.g. "explore_ship").
    /// `None` once they've finished the whole starter chain.
    pub current_quest: Option<String>,
    /// Quest ids the player has completed, in completion order.
    pub completed_quests: Vec<String>,
    /// Accumulated experience points.
    pub xp: u64,
    /// Accumulated reputation with the crew.
    pub reputation: u64,
    /// Unix-millis timestamp of the last update.
    pub updated_at: u64,
}

impl Storage {
    // ── Game world snapshot ──────────────────────────────────────────

    /// Persist (insert-or-replace) the world snapshot for `world_id`.
    ///
    /// The caller serializes the world into `snapshot_json`; we just durably
    /// store it alongside the two scalar columns. Upsert on the `world_id`
    /// primary key, so saving repeatedly (e.g. the 30-second periodic save)
    /// overwrites in place rather than accumulating rows.
    pub fn save_game_world(
        &self,
        world_id: &str,
        snapshot_json: &str,
        game_time: f64,
        next_entity_id: u64,
    ) -> Result<(), rusqlite::Error> {
        let now = now_millis() as i64;
        // SQLite stores u64 as i64; next_entity_id is far below i64::MAX in
        // practice (we'd need ~9.2e18 spawns), so the cast is safe.
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO game_world_snapshots
                     (world_id, snapshot_json, game_time, next_entity_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(world_id) DO UPDATE SET
                     snapshot_json  = excluded.snapshot_json,
                     game_time      = excluded.game_time,
                     next_entity_id = excluded.next_entity_id,
                     updated_at     = excluded.updated_at",
                params![
                    world_id,
                    snapshot_json,
                    game_time,
                    next_entity_id as i64,
                    now,
                ],
            )?;
            Ok(())
        })
    }

    /// Load the world snapshot for `world_id`, or `None` if none has been
    /// saved yet (fresh relay / fresh world). Read-only, so it rides the
    /// concurrent read pool when available and falls back to the writer.
    pub fn load_game_world(
        &self,
        world_id: &str,
    ) -> Result<Option<GameWorldSnapshot>, rusqlite::Error> {
        let read = self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT world_id, snapshot_json, game_time, next_entity_id, updated_at
                 FROM game_world_snapshots WHERE world_id = ?1",
                params![world_id],
                |r| {
                    Ok(GameWorldSnapshot {
                        world_id: r.get::<_, String>(0)?,
                        snapshot_json: r.get::<_, String>(1)?,
                        game_time: r.get::<_, f64>(2)?,
                        // i64 → u64: stored value is always non-negative.
                        next_entity_id: r.get::<_, i64>(3)? as u64,
                        updated_at: r.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()
        });
        // The read pool can be exhausted (returns SQLITE_BUSY); fall back to
        // the writer connection so a load never spuriously fails.
        match read {
            Ok(v) => Ok(v),
            Err(_) => self.with_conn(|conn| {
                conn.query_row(
                    "SELECT world_id, snapshot_json, game_time, next_entity_id, updated_at
                     FROM game_world_snapshots WHERE world_id = ?1",
                    params![world_id],
                    |r| {
                        Ok(GameWorldSnapshot {
                            world_id: r.get::<_, String>(0)?,
                            snapshot_json: r.get::<_, String>(1)?,
                            game_time: r.get::<_, f64>(2)?,
                            next_entity_id: r.get::<_, i64>(3)? as u64,
                            updated_at: r.get::<_, i64>(4)? as u64,
                        })
                    },
                )
                .optional()
            }),
        }
    }

    // ── Player progress ──────────────────────────────────────────────

    /// Persist (insert-or-replace) a player's progression.
    ///
    /// `completed` is the list of completed quest ids; it's serialized to a
    /// JSON array string for storage. Upsert on the `public_key` primary key,
    /// so every quest completion / reward for a player overwrites their single
    /// row in place.
    pub fn save_player_progress(
        &self,
        public_key: &str,
        current_quest: Option<&str>,
        completed: &[String],
        xp: u64,
        reputation: u64,
    ) -> Result<(), rusqlite::Error> {
        // Serialize the completed list to a JSON array string. This can only
        // fail on an allocator OOM (Vec<String> → JSON is infallible in
        // practice); map it into the rusqlite error channel just in case.
        let completed_json = serde_json::to_string(completed).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        })?;
        let now = now_millis() as i64;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO player_progress
                     (public_key, current_quest, completed_quests, xp, reputation, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(public_key) DO UPDATE SET
                     current_quest    = excluded.current_quest,
                     completed_quests = excluded.completed_quests,
                     xp               = excluded.xp,
                     reputation       = excluded.reputation,
                     updated_at       = excluded.updated_at",
                params![
                    public_key,
                    current_quest,
                    completed_json,
                    xp as i64,
                    reputation as i64,
                    now,
                ],
            )?;
            Ok(())
        })
    }

    /// Load a player's saved progression, or `None` if they've never been
    /// persisted (brand-new player). Used on `game_join` to re-seed a
    /// returning player's quest/XP/reputation.
    pub fn load_player_progress(
        &self,
        public_key: &str,
    ) -> Result<Option<PlayerProgress>, rusqlite::Error> {
        // Shared row-mapper so the read-pool path and the writer fallback stay
        // identical. completed_quests is stored as a JSON array string; a
        // corrupt/legacy value degrades to an empty list rather than erroring.
        fn map_row(r: &rusqlite::Row) -> rusqlite::Result<PlayerProgress> {
            let completed_json: String = r.get(2)?;
            let completed_quests: Vec<String> =
                serde_json::from_str(&completed_json).unwrap_or_default();
            Ok(PlayerProgress {
                public_key: r.get::<_, String>(0)?,
                current_quest: r.get::<_, Option<String>>(1)?,
                completed_quests,
                xp: r.get::<_, i64>(3)? as u64,
                reputation: r.get::<_, i64>(4)? as u64,
                updated_at: r.get::<_, i64>(5)? as u64,
            })
        }
        const SQL: &str =
            "SELECT public_key, current_quest, completed_quests, xp, reputation, updated_at
             FROM player_progress WHERE public_key = ?1";
        let read = self.with_read_conn(|conn| {
            conn.query_row(SQL, params![public_key], map_row).optional()
        });
        match read {
            Ok(v) => Ok(v),
            Err(_) => self.with_conn(|conn| {
                conn.query_row(SQL, params![public_key], map_row).optional()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spin up a throwaway on-disk SQLite DB with the full relay schema.
    /// Mirrors the per-module helper used across `storage/` (see ai_status.rs):
    /// a unique temp path so parallel test runs never collide.
    fn make_test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_gamepersist_test_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// A world snapshot must round-trip: what we save is exactly what we load,
    /// including the opaque JSON blob, game_time, and next_entity_id.
    #[test]
    fn world_snapshot_round_trips() {
        let db = make_test_storage();

        // No snapshot saved yet → None.
        assert!(db.load_game_world("world_main").unwrap().is_none());

        // A representative blob: entities + the two scalars, like game_state.rs.
        let blob = r#"{"entities":{"1":{"entity_type":"player","position":[1.0,2.0,3.0]}},"next_entity_id":42,"game_time":123.5}"#;
        db.save_game_world("world_main", blob, 123.5, 42).unwrap();

        let loaded = db.load_game_world("world_main").unwrap().expect("snapshot present");
        assert_eq!(loaded.world_id, "world_main");
        assert_eq!(loaded.snapshot_json, blob, "JSON blob must be preserved byte-for-byte");
        assert_eq!(loaded.game_time, 123.5, "game_time must round-trip");
        assert_eq!(loaded.next_entity_id, 42, "next_entity_id must round-trip");
    }

    /// Saving the same world_id again overwrites in place (upsert), it does
    /// not accumulate rows — the periodic 30s save relies on this.
    #[test]
    fn world_snapshot_upserts_in_place() {
        let db = make_test_storage();
        db.save_game_world("world_main", r#"{"v":1}"#, 1.0, 10).unwrap();
        db.save_game_world("world_main", r#"{"v":2}"#, 99.0, 77).unwrap();

        let loaded = db.load_game_world("world_main").unwrap().unwrap();
        assert_eq!(loaded.snapshot_json, r#"{"v":2}"#, "second save should win");
        assert_eq!(loaded.game_time, 99.0);
        assert_eq!(loaded.next_entity_id, 77);

        // Exactly one row for this world_id.
        let count: i64 = db
            .with_conn(|c| {
                c.query_row(
                    "SELECT COUNT(*) FROM game_world_snapshots WHERE world_id = 'world_main'",
                    [],
                    |r| r.get(0),
                )
            })
            .unwrap();
        assert_eq!(count, 1, "upsert must keep a single row per world_id");
    }

    /// Player progress must round-trip: xp, current_quest, and the
    /// completed_quests list all preserved exactly.
    #[test]
    fn player_progress_round_trips() {
        let db = make_test_storage();
        let pk = "deadbeefcafe";

        // No progress saved yet → None.
        assert!(db.load_player_progress(pk).unwrap().is_none());

        let completed = vec!["explore_ship".to_string(), "meet_the_crew".to_string()];
        db.save_player_progress(pk, Some("survey_storage"), &completed, 300, 15)
            .unwrap();

        let loaded = db.load_player_progress(pk).unwrap().expect("progress present");
        assert_eq!(loaded.public_key, pk);
        assert_eq!(loaded.current_quest.as_deref(), Some("survey_storage"), "current_quest must round-trip");
        assert_eq!(loaded.completed_quests, completed, "completed_quests list must round-trip in order");
        assert_eq!(loaded.xp, 300, "xp must round-trip");
        assert_eq!(loaded.reputation, 15, "reputation must round-trip");
    }

    /// A finished player (no current quest, several completed) round-trips
    /// with `current_quest = None`.
    #[test]
    fn player_progress_handles_no_current_quest() {
        let db = make_test_storage();
        let pk = "feedface";
        let completed = vec![
            "explore_ship".to_string(),
            "meet_the_crew".to_string(),
            "survey_storage".to_string(),
        ];
        db.save_player_progress(pk, None, &completed, 600, 30).unwrap();

        let loaded = db.load_player_progress(pk).unwrap().unwrap();
        assert!(loaded.current_quest.is_none(), "finished player has no current quest");
        assert_eq!(loaded.completed_quests.len(), 3);
        assert_eq!(loaded.xp, 600);
    }

    /// Re-saving a player's progress overwrites their single row (upsert),
    /// modeling repeated quest completions for the same player.
    #[test]
    fn player_progress_upserts_in_place() {
        let db = make_test_storage();
        let pk = "abc123";
        db.save_player_progress(pk, Some("explore_ship"), &[], 0, 0).unwrap();
        db.save_player_progress(
            pk,
            Some("meet_the_crew"),
            &["explore_ship".to_string()],
            100,
            5,
        )
        .unwrap();

        let loaded = db.load_player_progress(pk).unwrap().unwrap();
        assert_eq!(loaded.current_quest.as_deref(), Some("meet_the_crew"));
        assert_eq!(loaded.xp, 100);
        assert_eq!(loaded.completed_quests, vec!["explore_ship".to_string()]);

        let count: i64 = db
            .with_conn(|c| {
                c.query_row(
                    "SELECT COUNT(*) FROM player_progress WHERE public_key = 'abc123'",
                    [],
                    |r| r.get(0),
                )
            })
            .unwrap();
        assert_eq!(count, 1, "upsert must keep a single row per player");
    }
}
