use rusqlite::params;
use super::Storage;

impl Storage {
    /// Store or replace the encrypted vault blob for a public key.
    /// The blob is opaque to the server — already AES-256-GCM encrypted by the client.
    pub fn store_vault_blob(&self, public_key: &str, blob: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now  = super::now_millis() as i64;
        conn.execute(
            "INSERT OR REPLACE INTO vault_blobs (public_key, blob, updated_at) VALUES (?1, ?2, ?3)",
            params![public_key, blob, now],
        )?;
        Ok(())
    }

    /// Retrieve the encrypted vault blob and its last-updated timestamp.
    pub fn get_vault_blob(&self, public_key: &str) -> Option<(String, u64)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT blob, updated_at FROM vault_blobs WHERE public_key = ?1",
            params![public_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1).map(|v| v as u64)?)),
        ).ok()
    }

    /// Delete the vault blob for a public key (user-initiated wipe).
    pub fn delete_vault_blob(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM vault_blobs WHERE public_key = ?1",
            params![public_key],
        )?;
        Ok(())
    }
}
