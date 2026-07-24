//! Backup surface for the in-app Backups panel (v0.938, in-app-ops gap #2):
//! list what exists in `backups/` and take a new backup on demand. The
//! on-demand path uses SQLite's own `VACUUM INTO` - a consistent, compacted
//! snapshot taken inside the engine, so the button works on ANY host (VPS,
//! desktop hosting a LAN world) with no shell scripts involved. The rotating
//! scheduled backups (cron on the VPS) and off-host pulls are separate layers
//! and unaffected; RESTORE deliberately stays an attended host-side procedure
//! (swapping the live DB file under an open pool is not a button).

use serde::{Deserialize, Serialize};

/// One backup file as shown in the panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    /// File name inside backups/ (never a full path - nothing to leak).
    pub file: String,
    pub size_bytes: u64,
    /// Seconds since the Unix epoch (mtime).
    pub modified_epoch: u64,
}

/// Directory the panel lists and `backup_now` writes into, relative to the
/// process CWD like `data/relay.db` itself. The VPS deploy already excludes
/// `backups/` from sync; created on first use elsewhere.
pub const BACKUPS_DIR: &str = "backups";

/// List `backups/*.db`, newest first, capped so a years-old rotation dir
/// cannot flood the message. Missing dir = empty list (fresh host).
pub fn list_backups() -> Vec<BackupEntry> {
    let mut out: Vec<BackupEntry> = std::fs::read_dir(BACKUPS_DIR)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().extension().and_then(|x| x.to_str()) == Some("db")
                })
                .filter_map(|e| {
                    let md = e.metadata().ok()?;
                    Some(BackupEntry {
                        file: e.file_name().to_string_lossy().to_string(),
                        size_bytes: md.len(),
                        modified_epoch: md
                            .modified()
                            .ok()?
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?
                            .as_secs(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.modified_epoch.cmp(&a.modified_epoch));
    out.truncate(50);
    out
}

impl super::Storage {
    /// Take a consistent snapshot of the live database into
    /// `backups/manual-<utc-timestamp>.db` via `VACUUM INTO` and return its
    /// entry. Runs on the writer connection; SQLite guarantees the snapshot
    /// is transactionally consistent even while the relay keeps serving.
    pub fn backup_now(&self) -> Result<BackupEntry, String> {
        std::fs::create_dir_all(BACKUPS_DIR).map_err(|e| format!("create {BACKUPS_DIR}/: {e}"))?;
        // Seconds-resolution name is enough: a second click within the same
        // second fails on the existing file rather than corrupting anything.
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let file = format!("manual-{secs}.db");
        let path = format!("{BACKUPS_DIR}/{file}");
        self.with_conn(|conn| conn.execute("VACUUM INTO ?1", rusqlite::params![path]))
            .map_err(|e| format!("VACUUM INTO failed: {e}"))?;
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).map_err(|e| e.to_string())?;
        Ok(BackupEntry { file, size_bytes, modified_epoch: secs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// backup_now must produce an openable, non-empty SQLite file. Runs from
    /// a scratch CWD so the repo's real backups/ dir is untouched.
    #[test]
    fn backup_now_produces_an_openable_snapshot() {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("hum_backup_{pid}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let db = crate::relay::storage::Storage::open(&dir.join("live.db")).expect("open");
        db.set_role("someone", "admin").unwrap();
        let entry = db.backup_now().expect("backup_now");
        assert!(entry.size_bytes > 0, "snapshot is empty");

        // The snapshot must open as a real database carrying the data.
        let snap = rusqlite::Connection::open(dir.join(BACKUPS_DIR).join(&entry.file)).unwrap();
        let n: i64 = snap
            .query_row("SELECT COUNT(*) FROM user_roles WHERE role='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "snapshot missing the written row");

        assert_eq!(list_backups().len(), 1, "panel list sees the snapshot");
        std::env::set_current_dir(old_cwd).unwrap();
    }
}
