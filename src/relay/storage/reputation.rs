use super::Storage;
use rusqlite::params;

/// A reputation record from the database.
#[derive(Debug, Clone)]
pub struct ReputationRecord {
    pub public_key: String,
    pub score: i64,
    pub level: i64,
    pub updated_at: i64,
}

/// A reputation event record from the database.
#[derive(Debug, Clone)]
pub struct ReputationEventRecord {
    pub id: i64,
    pub public_key: String,
    pub event_type: String,
    pub points: i64,
    pub reason: String,
    pub created_at: i64,
    pub source_key: String,
}

/// Compute level from score. Every 50 points = 1 level.
fn level_from_score(score: i64) -> i64 {
    if score <= 0 { return 0; }
    (score as f64 / 50.0).floor() as i64
}

/// Map event type to point value.
pub fn points_for_event(event_type: &str) -> i64 {
    match event_type {
        "helpful_message" => 1,
        "task_completed" => 5,
        "trade_completed" => 3,
        "review_given" => 1,
        "reported" => -5,
        "verified_by_mod" => 10,
        _ => 0,
    }
}

impl Storage {
    // ── Reputation methods ──

    /// Add a reputation event and update the user's total score.
    pub fn add_reputation_event(
        &self,
        public_key: &str,
        event_type: &str,
        reason: &str,
        source_key: &str,
    ) -> Result<ReputationRecord, rusqlite::Error> {
        let points = points_for_event(event_type);
        let now = super::now_millis() as i64;

        self.with_conn(|conn| {
            // Insert the event.
            conn.execute(
                "INSERT INTO reputation_events (public_key, event_type, points, reason, created_at, source_key)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![public_key, event_type, points, reason, now, source_key],
            )?;

            // Upsert the reputation total.
            conn.execute(
                "INSERT INTO reputation (public_key, score, level, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(public_key) DO UPDATE SET
                   score = score + ?2,
                   level = CAST((CASE WHEN score + ?2 > 0 THEN (score + ?2) / 50 ELSE 0 END) AS INTEGER),
                   updated_at = ?4",
                params![public_key, points, level_from_score(points), now],
            )?;

            // Return the updated reputation.
            let rec = conn.query_row(
                "SELECT public_key, score, level, updated_at FROM reputation WHERE public_key = ?1",
                params![public_key],
                |row| Ok(ReputationRecord {
                    public_key: row.get(0)?,
                    score: row.get(1)?,
                    level: row.get(2)?,
                    updated_at: row.get(3)?,
                }),
            )?;
            Ok(rec)
        })
    }

    /// Get a user's reputation.
    pub fn get_reputation(&self, public_key: &str) -> Result<ReputationRecord, rusqlite::Error> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT public_key, score, level, updated_at FROM reputation WHERE public_key = ?1",
                params![public_key],
                |row| Ok(ReputationRecord {
                    public_key: row.get(0)?,
                    score: row.get(1)?,
                    level: row.get(2)?,
                    updated_at: row.get(3)?,
                }),
            ) {
                Ok(rec) => Ok(rec),
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Return a default record for users with no reputation yet.
                    Ok(ReputationRecord {
                        public_key: public_key.to_string(),
                        score: 0,
                        level: 0,
                        updated_at: 0,
                    })
                }
                Err(e) => Err(e),
            }
        })
    }

    /// Get reputation event history for a user.
    pub fn get_reputation_history(
        &self,
        public_key: &str,
        limit: usize,
    ) -> Result<Vec<ReputationEventRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, public_key, event_type, points, reason, created_at, source_key
                 FROM reputation_events
                 WHERE public_key = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2"
            )?;
            let rows = stmt.query_map(params![public_key, limit as i64], |row| {
                Ok(ReputationEventRecord {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    event_type: row.get(2)?,
                    points: row.get(3)?,
                    reason: row.get(4)?,
                    created_at: row.get(5)?,
                    source_key: row.get(6)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Get the reputation leaderboard (top users by score).
    pub fn get_reputation_leaderboard(
        &self,
        limit: usize,
    ) -> Result<Vec<ReputationRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT r.public_key, r.score, r.level, r.updated_at
                 FROM reputation r
                 WHERE r.score > 0
                 ORDER BY r.score DESC
                 LIMIT ?1"
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok(ReputationRecord {
                    public_key: row.get(0)?,
                    score: row.get(1)?,
                    level: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?;
            rows.collect()
        })
    }
}
