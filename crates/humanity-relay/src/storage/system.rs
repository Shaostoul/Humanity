use rusqlite::params;
use super::Storage;

impl Storage {
    /// Store or replace the system profile JSON for a public key.
    /// Not encrypted — system specs (OS, CPU, GPU, RAM) are not sensitive.
    pub fn store_system_profile(&self, public_key: &str, profile: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            let now = super::now_millis() as i64;
            conn.execute(
                "INSERT OR REPLACE INTO system_profiles (public_key, profile, updated_at) VALUES (?1, ?2, ?3)",
                params![public_key, profile, now],
            )?;
            Ok(())
        })
    }

    /// Retrieve the system profile JSON and its last-updated timestamp.
    pub fn get_system_profile(&self, public_key: &str) -> Option<(String, u64)> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT profile, updated_at FROM system_profiles WHERE public_key = ?1",
                params![public_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1).map(|v| v as u64)?)),
            ).ok()
        })
    }

    /// Delete the system profile for a public key.
    pub fn delete_system_profile(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM system_profiles WHERE public_key = ?1",
                params![public_key],
            )?;
            Ok(())
        })
    }
}
