use rusqlite::params;
use super::Storage;

impl Storage {
    /// Record a key rotation from old_key -> new_key.
    /// Both signatures are stored for auditability.
    /// sig_by_old = sign(new_key + "\n" + timestamp, old_private_key)
    /// sig_by_new = sign(old_key + "\n" + timestamp, new_private_key)
    pub fn record_key_rotation(
        &self,
        old_key:    &str,
        new_key:    &str,
        sig_by_old: &str,
        sig_by_new: &str,
    ) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            let now  = super::now_millis() as i64;
            conn.execute(
                "INSERT OR REPLACE INTO key_rotations \
                 (old_key, new_key, sig_by_old, sig_by_new, rotated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![old_key, new_key, sig_by_old, sig_by_new, now],
            )?;
            Ok(())
        })
    }

    /// Return the new key and rotation timestamp if old_key has been rotated.
    pub fn get_key_rotation(&self, old_key: &str) -> Option<(String, u64)> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT new_key, rotated_at FROM key_rotations WHERE old_key = ?1",
                params![old_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1).map(|v| v as u64)?)),
            ).ok()
        })
    }

    /// Follow the rotation chain to the most current key for a given starting key.
    /// Caps at 10 hops to guard against cycles.
    pub fn resolve_current_key(&self, key: &str) -> String {
        let mut current = key.to_string();
        for _ in 0..10 {
            match self.get_key_rotation(&current) {
                Some((new, _)) => current = new,
                None           => break,
            }
        }
        current
    }
}
