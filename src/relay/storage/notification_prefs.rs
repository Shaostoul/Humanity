use super::Storage;
use rusqlite::params;

/// Notification preference record from the database.
#[derive(Debug, Clone)]
pub struct NotifPrefs {
    pub public_key: String,
    pub dm_enabled: bool,
    pub mentions_enabled: bool,
    pub tasks_enabled: bool,
    pub dnd_start: Option<String>,
    pub dnd_end: Option<String>,
}

impl Storage {
    /// Save notification preferences for a user (upsert).
    pub fn save_notification_prefs(
        &self,
        public_key: &str,
        dm: bool,
        mentions: bool,
        tasks: bool,
        dnd_start: Option<&str>,
        dnd_end: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO notification_prefs (public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(public_key) DO UPDATE SET
                     dm_enabled = excluded.dm_enabled,
                     mentions_enabled = excluded.mentions_enabled,
                     tasks_enabled = excluded.tasks_enabled,
                     dnd_start = excluded.dnd_start,
                     dnd_end = excluded.dnd_end",
                params![public_key, dm as i32, mentions as i32, tasks as i32, dnd_start, dnd_end],
            )?;
            Ok(())
        })
    }

    /// Get notification preferences for a user. Returns None if no prefs are stored.
    pub fn get_notification_prefs(&self, public_key: &str) -> Option<NotifPrefs> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT public_key, dm_enabled, mentions_enabled, tasks_enabled, dnd_start, dnd_end
                 FROM notification_prefs WHERE public_key = ?1",
                params![public_key],
                |row| {
                    Ok(NotifPrefs {
                        public_key: row.get(0)?,
                        dm_enabled: row.get::<_, i32>(1)? != 0,
                        mentions_enabled: row.get::<_, i32>(2)? != 0,
                        tasks_enabled: row.get::<_, i32>(3)? != 0,
                        dnd_start: row.get(4)?,
                        dnd_end: row.get(5)?,
                    })
                },
            )
            .ok()
        })
    }
}
