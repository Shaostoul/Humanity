use core_offline_loop::WorldSnapshot;
use rusqlite::{params, Connection};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                slot TEXT NOT NULL,
                tick INTEGER NOT NULL,
                data_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_slot_id ON snapshots(slot, id DESC);

            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                slot TEXT NOT NULL,
                tick INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_events_slot_id ON events(slot, id DESC);
            ",
        )?;
        Ok(())
    }

    pub fn save_snapshot(&self, slot: &str, world: &WorldSnapshot) -> Result<i64, StoreError> {
        let data_json = serde_json::to_string(world)?;
        self.conn.execute(
            "INSERT INTO snapshots (slot, tick, data_json) VALUES (?1, ?2, ?3)",
            params![slot, world.tick as i64, data_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn load_latest_snapshot(&self, slot: &str) -> Result<Option<WorldSnapshot>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM snapshots WHERE slot = ?1 ORDER BY id DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![slot])?;
        if let Some(row) = rows.next()? {
            let data_json: String = row.get(0)?;
            let world: WorldSnapshot = serde_json::from_str(&data_json)?;
            Ok(Some(world))
        } else {
            Ok(None)
        }
    }

    pub fn append_event(
        &self,
        slot: &str,
        tick: u64,
        event_type: &str,
        payload_json: &str,
    ) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO events (slot, tick, event_type, payload_json) VALUES (?1, ?2, ?3, ?4)",
            params![slot, tick as i64, event_type, payload_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_recent_events(
        &self,
        slot: &str,
        limit: u32,
    ) -> Result<Vec<(i64, u64, String, String)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tick, event_type, payload_json FROM events WHERE slot = ?1 ORDER BY id DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![slot, limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_latest_snapshot() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let store = SqliteStore::open(path).unwrap();

        let mut world = WorldSnapshot::new_default();
        world.tick = 42;
        let _ = store.save_snapshot("default", &world).unwrap();

        let loaded = store.load_latest_snapshot("default").unwrap().unwrap();
        assert_eq!(loaded.tick, 42);
    }

    #[test]
    fn appends_and_reads_events() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let store = SqliteStore::open(path).unwrap();

        let _ = store.append_event("default", 1, "move", "{\"dir\":\"n\"}").unwrap();
        let _ = store.append_event("default", 2, "drink", "{}").unwrap();

        let events = store.list_recent_events("default", 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].2, "drink");
    }
}
