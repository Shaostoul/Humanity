use super::Storage;
use rusqlite::params;

impl Storage {
    // ── Skill DNA ──

    pub fn upsert_skill(&self, user_key: &str, skill_id: &str, reality_xp: f64, fantasy_xp: f64, level: i32) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        conn.execute(
            "INSERT INTO user_skills (user_key, skill_id, reality_xp, fantasy_xp, level, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(user_key, skill_id) DO UPDATE SET reality_xp=?3, fantasy_xp=?4, level=?5, updated_at=?6",
            params![user_key, skill_id, reality_xp, fantasy_xp, level, now],
        )?;
        Ok(())
    }

    pub fn search_skills(&self, skill_id: &str, min_level: i32, limit: usize) -> Result<Vec<(String, f64, f64, i32)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT us.user_key, us.reality_xp, us.fantasy_xp, us.level
             FROM user_skills us
             WHERE us.skill_id = ?1 AND us.level >= ?2
             ORDER BY us.level DESC, us.reality_xp DESC
             LIMIT ?3"
        )?;
        let results = stmt.query_map(params![skill_id, min_level, limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    pub fn get_user_skills(&self, user_key: &str) -> Result<Vec<(String, f64, f64, i32)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT skill_id, reality_xp, fantasy_xp, level FROM user_skills WHERE user_key = ?1 ORDER BY level DESC"
        )?;
        let results = stmt.query_map(params![user_key], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    pub fn get_top_skills(&self, user_key: &str, limit: usize) -> Result<Vec<(String, i32)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT skill_id, level FROM user_skills WHERE user_key = ?1 AND level > 0 ORDER BY level DESC, reality_xp DESC LIMIT ?2"
        )?;
        let results = stmt.query_map(params![user_key, limit], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }

    pub fn get_display_name(&self, public_key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name FROM registered_names WHERE public_key = ?1 LIMIT 1")?;
        let name: Option<String> = stmt.query_row(params![public_key], |row| row.get(0)).ok();
        Ok(name)
    }

    pub fn store_skill_verification(&self, skill_id: &str, from_key: &str, to_key: &str, note: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        conn.execute(
            "INSERT INTO skill_verifications (skill_id, from_key, to_key, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![skill_id, from_key, to_key, note, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Return endorsement counts per skill for a given user key.
    /// Returns Vec of (skill_id, count, most_recent_endorser_name).
    pub fn get_skill_endorsement_counts(&self, user_key: &str) -> Result<Vec<(String, i64, Option<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT sv.skill_id, COUNT(*) as cnt,
                    (SELECT rn.name FROM registered_names rn WHERE rn.public_key = sv.from_key LIMIT 1) as endorser_name
             FROM skill_verifications sv
             WHERE sv.to_key = ?1
             GROUP BY sv.skill_id
             ORDER BY cnt DESC"
        )?;
        let results = stmt.query_map(params![user_key], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(results)
    }
}
