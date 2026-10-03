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

/// Is this file name a database backup, plain or sealed? `.db` (plain,
/// only made when no key exists), `.db.aes` (the VPS 30-minute script's
/// openssl seal) and `.db.enc` (the relay's own AES-GCM seal). Since the
/// 2026-08-24 encryption every routine backup is sealed, and the panel and
/// the status row that matched `.db` alone showed "no backups" once the
/// last plain copies were deleted (2026-10-02).
pub fn is_backup_file(name: &str) -> bool {
    name.ends_with(".db") || name.ends_with(".db.aes") || name.ends_with(".db.enc")
}

/// List the backups in `backups/`, plain and sealed, newest first, capped
/// so a years-old rotation dir cannot flood the message. Missing dir =
/// empty list (fresh host).
pub fn list_backups() -> Vec<BackupEntry> {
    let mut out: Vec<BackupEntry> = std::fs::read_dir(BACKUPS_DIR)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| is_backup_file(&e.file_name().to_string_lossy()))
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

/// How long the relay waits before its FIRST 6-hourly snapshot after a
/// start (2026-10-02, BUG-123): until the newest existing snapshot is
/// `every` old, and never less than `floor` so a start settles first. The
/// wait used to be a flat 6 hours from every start, so a server deployed
/// more often than that took no snapshot at all, and the mail and message
/// expiry that runs in the same pass never ran either.
pub fn first_snapshot_wait(
    newest_age: Option<std::time::Duration>,
    every: std::time::Duration,
    floor: std::time::Duration,
) -> std::time::Duration {
    match newest_age {
        Some(age) if age < every => (every - age).max(floor),
        _ => floor,
    }
}

/// Age of the newest of the relay's own snapshots (`relay_*.db` or
/// `relay_*.db.enc`) in `dir`, by modified time; None when there is none.
pub fn newest_snapshot_age(dir: &std::path::Path) -> Option<std::time::Duration> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with("relay_") && (n.ends_with(".db") || n.ends_with(".db.enc"))
        })
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
        .map(|t| t.elapsed().unwrap_or_default())
}

impl super::Storage {
    /// Take a consistent snapshot of the live database into
    /// `backups/manual-<utc-timestamp>.db` via `VACUUM INTO` and return its
    /// entry. Runs on the writer connection; SQLite guarantees the snapshot
    /// is transactionally consistent even while the relay keeps serving.
    ///
    /// Sealed like every other backup (2026-10-02): the snapshot is
    /// encrypted to `manual-<ts>.db.enc` with the machine-local key beside
    /// the live database and the plain copy removed. Before, the button left
    /// a readable copy of the whole server in backups/ that nothing rotated.
    /// With no key it keeps the plain file, as the 6-hourly pass does.
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
        // The live database's own file, so the key is the one beside it.
        let live: String = self
            .with_conn(|conn| conn.query_row("PRAGMA database_list", [], |r| r.get::<_, String>(2)))
            .map_err(|e| format!("locating the live database: {e}"))?;
        let key_dir = super::backup_crypto::key_dir_for_db(std::path::Path::new(&live));
        let (file, path) = match super::backup_crypto::load_or_create_key(&key_dir) {
            Some(key) => {
                let enc_file = format!("{file}.enc");
                let enc_path = format!("{BACKUPS_DIR}/{enc_file}");
                super::backup_crypto::encrypt_file(&key, std::path::Path::new(&path), std::path::Path::new(&enc_path))
                    .map_err(|e| format!("sealing the backup: {e}"))?;
                let _ = std::fs::remove_file(&path);
                (enc_file, enc_path)
            }
            None => (file, path),
        };
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).map_err(|e| e.to_string())?;
        Ok(BackupEntry { file, size_bytes, modified_epoch: secs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first snapshot after a start is due when the newest one turns
    /// six hours old, not six hours after the start (BUG-123). Red check,
    /// run: returning `every` unconditionally fails the first assertion.
    #[test]
    fn first_snapshot_is_due_by_the_newest_ones_age() {
        let h = |x: u64| std::time::Duration::from_secs(x * 3600);
        let floor = std::time::Duration::from_secs(120);
        assert_eq!(first_snapshot_wait(None, h(6), floor), floor, "no snapshot yet: take one now");
        assert_eq!(first_snapshot_wait(Some(h(5)), h(6), floor), h(1), "5 h old: the next is due in 1 h");
        assert_eq!(first_snapshot_wait(Some(h(9)), h(6), floor), floor, "overdue: now");
        assert_eq!(
            first_snapshot_wait(Some(h(6) - std::time::Duration::from_secs(10)), h(6), floor),
            floor,
            "due in 10 s: still waits the floor"
        );
    }

    /// A sealed backup is a backup (2026-10-02): the panel listed only
    /// `.db`, and once the plain copies were gone it showed none.
    #[test]
    fn sealed_backups_count_as_backups() {
        for n in ["relay-20261003-044437.db.aes", "relay_20261002_233715.db.enc", "manual-1.db.enc", "relay-x.db"] {
            assert!(is_backup_file(n), "{n}");
        }
        for n in ["HumanityOS-20261003-005506", "backup-pull.log", "relay.db-wal"] {
            assert!(!is_backup_file(n), "{n}");
        }
    }

    /// backup_now must produce a SEALED, non-empty snapshot that decrypts
    /// with the key beside the live database into a database carrying the
    /// data. Runs from a scratch CWD so the repo's real backups/ dir is
    /// untouched. Red check, run: with the seal step skipped the file name
    /// assertion fails (a plain .db).
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
        assert!(entry.file.ends_with(".db.enc"), "the snapshot is sealed: {}", entry.file);
        assert!(
            !dir.join(BACKUPS_DIR).join(entry.file.trim_end_matches(".enc")).exists(),
            "no plain copy left behind"
        );

        // Decrypted with the key beside the live database, the snapshot
        // opens as a real database carrying the data.
        let key = crate::relay::storage::backup_crypto::load_or_create_key(&dir).expect("key beside live.db");
        let plain = dir.join("snap.db");
        crate::relay::storage::backup_crypto::decrypt_file(&key, &dir.join(BACKUPS_DIR).join(&entry.file), &plain)
            .expect("decrypts");
        let snap = rusqlite::Connection::open(&plain).unwrap();
        let n: i64 = snap
            .query_row("SELECT COUNT(*) FROM user_roles WHERE role='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "snapshot missing the written row");

        assert_eq!(list_backups().len(), 1, "panel list sees the snapshot");
        std::env::set_current_dir(old_cwd).unwrap();
    }
}
