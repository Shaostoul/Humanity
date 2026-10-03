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

/// How many "Back up now" copies stay in the backups folder (2026-10-03).
///
/// Each press writes a full sealed copy of the whole database, and until
/// this constant existed nothing ever removed one, so a server whose admin
/// pressed the button before every deploy grew its backups folder without
/// limit (found 2026-10-02). Ten, because a manual copy is usually taken
/// just before an attended risky step (a deploy, a migration, a bulk
/// delete): ten covers a long evening of those with room to reach back
/// past a mistake noticed late, while capping the folder at ten database
/// copies. The scheduled snapshots are a separate layer with their own
/// rotation (the relay's 6-hourly pass keeps 5, the VPS 30-minute script
/// keeps 15) and pruning here never touches them.
pub const MANUAL_BACKUPS_KEPT: usize = 10;

/// The timestamp inside a "Back up now" file name, `manual-<secs>.db`
/// (made only when no backup key exists) or `manual-<secs>.db.enc` (the
/// sealed form). None for every other name, so the scheduled snapshots
/// (`relay-*`, `relay_*`), the backup key, logs and anything a person
/// dropped into the folder are never candidates for pruning.
fn manual_backup_stamp(name: &str) -> Option<u64> {
    let rest = name.strip_prefix("manual-")?;
    let digits = rest.strip_suffix(".db.enc").or_else(|| rest.strip_suffix(".db"))?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// What a press of Back up now did about the OLDER manual copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PruneOutcome {
    /// The copies deleted to keep the newest ones, oldest first. Empty
    /// while there are no more than the kept number.
    Removed(Vec<String>),
    /// Nothing was deleted on this press because this copy, already in the
    /// folder, is stamped LATER than the one just made: the clock went back
    /// (a corrected host clock, a restored VM snapshot). Ranking by stamp
    /// would then delete the copy made on the previous press, every press,
    /// while the future-dated copies stayed, so pruning waits until the
    /// clock passes them. The name is the newest such copy.
    SkippedNewerExists(String),
}

/// What one press of Back up now did: the copy it made, and what happened
/// to the older copies. `reply` is the chat message the admin gets.
#[derive(Debug, Clone)]
pub struct BackupDone {
    pub entry: BackupEntry,
    pub prune: PruneOutcome,
}

impl BackupDone {
    /// The confirmation sent to the admin who pressed the button. It names
    /// every copy the press deleted, so an admin who took a copy before a
    /// migration and kept pressing afterwards is told when that copy goes,
    /// instead of finding out from the server log (critic's review,
    /// 2026-10-03).
    pub fn reply(&self) -> String {
        let made = format!(
            "Backup complete: {} ({:.1} MB)",
            self.entry.file,
            self.entry.size_bytes as f64 / 1_048_576.0
        );
        match &self.prune {
            PruneOutcome::Removed(names) if names.is_empty() => format!("{made}."),
            PruneOutcome::Removed(names) if names.len() == 1 => format!(
                "{made}; removed the copy with the earliest stamp, {}, to keep the newest {MANUAL_BACKUPS_KEPT}.",
                names[0]
            ),
            PruneOutcome::Removed(names) => format!(
                "{made}; removed the {} copies with the earliest stamps, {}, to keep the newest {MANUAL_BACKUPS_KEPT}.",
                names.len(),
                names.join(", ")
            ),
            PruneOutcome::SkippedNewerExists(newer) => format!(
                "{made}. No older copy was removed this time: {newer} is dated later than this \
                 one, so the server clock seems to have gone back."
            ),
        }
    }
}

/// Delete all but the newest `keep` "Back up now" copies in `dir` and say
/// which were deleted.
///
/// Newest is read from the timestamp in the file name, not the file's
/// modified time: the name is what the button wrote, and two copies made
/// in the same second can share a modified time on some file systems.
///
/// If any copy is stamped later than `just_made` (the copy this press
/// wrote), the clock went back and nothing is deleted on this press; see
/// `PruneOutcome::SkippedNewerExists`. Otherwise `just_made` holds the
/// first slot and is never deleted, which still matters when another copy
/// shares its second (a plain `.db` and a sealed `.db.enc` from the same
/// second) and when `keep` is 0. A file that cannot be deleted is logged
/// and skipped: the new backup already exists, so a leftover old copy is
/// not a reason to report the press as failed.
pub fn prune_manual_backups(dir: &std::path::Path, keep: usize, just_made: &str) -> PruneOutcome {
    let mut manual: Vec<(u64, String)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    manual_backup_stamp(&name).map(|stamp| (stamp, name))
                })
                .collect()
        })
        .unwrap_or_default();
    // The clock went back: a copy already here is dated after this one.
    // Strictly later only; a copy from the same second is not "newer".
    if let Some(made) = manual_backup_stamp(just_made) {
        if let Some((_, newer)) = manual.iter().filter(|(s, _)| *s > made).max() {
            return PruneOutcome::SkippedNewerExists(newer.clone());
        }
    }
    // The copy just made sorts first, then newest stamp first; equal stamps
    // fall back to the name so the order never depends on read_dir's order.
    manual.sort_by(|a, b| {
        (b.1 == just_made)
            .cmp(&(a.1 == just_made))
            .then(b.0.cmp(&a.0))
            .then(a.1.cmp(&b.1))
    });
    let mut deleted = Vec::new();
    for (_, name) in manual.into_iter().skip(keep) {
        if name == just_made {
            continue; // only reachable with keep == 0; the new copy always stays
        }
        match std::fs::remove_file(dir.join(&name)) {
            Ok(()) => deleted.push(name),
            Err(e) => tracing::warn!("Back up now: could not remove the old copy {name}: {e}"),
        }
    }
    // Deleted newest first above; the admin reads them oldest first.
    deleted.reverse();
    PruneOutcome::Removed(deleted)
}

/// List the backups in `backups/`, plain and sealed, newest first, capped
/// so a years-old rotation dir cannot flood the message. Missing dir =
/// empty list (fresh host).
pub fn list_backups() -> Vec<BackupEntry> {
    list_backups_in(std::path::Path::new(BACKUPS_DIR))
}

/// `list_backups` for any folder. Split out so tests can list a scratch
/// folder by its full path instead of changing the process working
/// directory, which every test thread shares.
pub fn list_backups_in(dir: &std::path::Path) -> Vec<BackupEntry> {
    let mut out: Vec<BackupEntry> = std::fs::read_dir(dir)
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
    ///
    /// After each new copy the older manual copies past the newest
    /// `MANUAL_BACKUPS_KEPT` are deleted (2026-10-03); before, nothing
    /// rotated them at all. The result says which were deleted, so the
    /// admin's confirmation can name them (`BackupDone::reply`).
    pub fn backup_now(&self) -> Result<BackupDone, String> {
        // The name has one-second resolution, so a second press within the
        // same second reuses it and never adds a file. What it does to the
        // first press's copy depends on the key:
        // - No key (the plain `.db` is still there): VACUUM INTO refuses an
        //   existing file, so the second press fails and the first copy is
        //   left alone.
        // - With a key: the plain file was removed after sealing, so VACUUM
        //   INTO succeeds and `encrypt_file` rewrites that second's sealed
        //   copy with `std::fs::write`, which empties the file before
        //   writing. If that write fails part way (a full disk, say), the
        //   earlier copy from the same second is left cut short. This was
        //   so before 2026-10-03 too; an earlier version of this comment
        //   claimed it could never corrupt a copy, which was wrong.
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        self.backup_into(std::path::Path::new(BACKUPS_DIR), secs)
    }

    /// The body of `backup_now`, with its two inputs passed in: the folder
    /// and the timestamp that goes into the file name. Tests call it with a
    /// scratch folder given by its full path (so no test changes the process
    /// working directory, which every test thread shares) and with made-up
    /// timestamps (so a dozen presses do not take a dozen real seconds).
    fn backup_into(&self, dir: &std::path::Path, secs: u64) -> Result<BackupDone, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}/: {e}", dir.display()))?;
        let file = format!("manual-{secs}.db");
        let path = dir.join(&file).to_string_lossy().to_string();
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
                let enc_path = dir.join(&enc_file).to_string_lossy().to_string();
                super::backup_crypto::encrypt_file(&key, std::path::Path::new(&path), std::path::Path::new(&enc_path))
                    .map_err(|e| format!("sealing the backup: {e}"))?;
                let _ = std::fs::remove_file(&path);
                (enc_file, enc_path)
            }
            None => (file, path),
        };
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).map_err(|e| e.to_string())?;
        // Only now, with the new copy safely written, retire the old ones.
        let prune = prune_manual_backups(dir, MANUAL_BACKUPS_KEPT, &file);
        match &prune {
            PruneOutcome::Removed(names) if names.is_empty() => {}
            PruneOutcome::Removed(names) => tracing::info!(
                "Back up now: kept the newest {MANUAL_BACKUPS_KEPT} manual copies, removed {}",
                names.join(", ")
            ),
            PruneOutcome::SkippedNewerExists(newer) => tracing::warn!(
                "Back up now: removed no older copy this press, because {newer} is dated later \
                 than the copy just made ({file}); the clock seems to have gone back. Pruning \
                 resumes once the clock passes that date, or remove the future-dated copy by hand."
            ),
        }
        Ok(BackupDone { entry: BackupEntry { file, size_bytes, modified_epoch: secs }, prune })
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

    /// A fresh scratch folder per test, named by process, test and clock so
    /// tests running side by side (cargo runs them on parallel threads)
    /// never share one. Every test below passes full paths into the code
    /// under test, so none of them changes the process working directory:
    /// that is shared by every test thread, and changing it used to make
    /// any other test reading a relative path flaky while this one ran.
    fn scratch(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("hum_backup_{tag}_{}_{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The names in `dir` that a press of Back up now could have written.
    fn manual_names(dir: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| manual_backup_stamp(n).is_some())
            .collect();
        v.sort();
        v
    }

    /// backup_now must produce a SEALED, non-empty snapshot that decrypts
    /// with the key beside the live database into a database carrying the
    /// data. Writes into a scratch folder by its full path, so the repo's
    /// real backups/ dir is untouched and the working directory is never
    /// changed. Red check, run: with the seal step skipped the file name
    /// assertion fails (a plain .db).
    #[test]
    fn backup_now_produces_an_openable_snapshot() {
        let dir = scratch("open");
        let bdir = dir.join(BACKUPS_DIR);
        let db = crate::relay::storage::Storage::open(&dir.join("live.db")).expect("open");
        db.set_role("someone", "admin").unwrap();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let entry = db.backup_into(&bdir, secs).expect("backup_now").entry;
        assert!(entry.size_bytes > 0, "snapshot is empty");
        assert!(entry.file.ends_with(".db.enc"), "the snapshot is sealed: {}", entry.file);
        assert!(
            !bdir.join(entry.file.trim_end_matches(".enc")).exists(),
            "no plain copy left behind"
        );

        // Decrypted with the key beside the live database, the snapshot
        // opens as a real database carrying the data.
        let key = crate::relay::storage::backup_crypto::load_or_create_key(&dir).expect("key beside live.db");
        let plain = dir.join("snap.db");
        crate::relay::storage::backup_crypto::decrypt_file(&key, &bdir.join(&entry.file), &plain)
            .expect("decrypts");
        let snap = rusqlite::Connection::open(&plain).unwrap();
        let n: i64 = snap
            .query_row("SELECT COUNT(*) FROM user_roles WHERE role='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "snapshot missing the written row");

        assert_eq!(list_backups_in(&bdir).len(), 1, "panel list sees the snapshot");
        drop(snap);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Pressing Back up now MANUAL_BACKUPS_KEPT + 3 times leaves exactly
    /// MANUAL_BACKUPS_KEPT copies, and they are the newest ones (found
    /// 2026-10-02: nothing ever removed a manual copy). Each press goes
    /// through the real path (VACUUM INTO, seal, prune) with made-up
    /// timestamps one second apart.
    ///
    /// Red check, run 2026-10-03 and again after the result became a
    /// `PruneOutcome`: with the `prune_manual_backups` call in `backup_into`
    /// replaced by `PruneOutcome::Removed(Vec::new())`, this failed with
    /// "assertion `left == right` failed: only the newest 10 manual copies
    /// remain" (left: all 13 names, manual-1790000000 to -012; right: the
    /// 10 from manual-1790000003).
    #[test]
    fn manual_presses_keep_only_the_newest_copies() {
        let dir = scratch("keep");
        let bdir = dir.join(BACKUPS_DIR);
        let db = crate::relay::storage::Storage::open(&dir.join("live.db")).expect("open");
        let base: u64 = 1_790_000_000;
        let presses = MANUAL_BACKUPS_KEPT + 3;
        for i in 0..presses as u64 {
            let entry = db.backup_into(&bdir, base + i).expect("press").entry;
            assert!(bdir.join(&entry.file).exists(), "the copy this press made is there: {}", entry.file);
        }
        let expected: Vec<String> = {
            let mut v: Vec<String> = (3..presses as u64)
                .map(|i| format!("manual-{}.db.enc", base + i))
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            manual_names(&bdir),
            expected,
            "only the newest {MANUAL_BACKUPS_KEPT} manual copies remain"
        );
        assert_eq!(list_backups_in(&bdir).len(), MANUAL_BACKUPS_KEPT, "the panel lists the same count");
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Pruning manual copies never touches the scheduled snapshots in the
    /// same folder (the relay's `relay_*` and the VPS script's `relay-*`),
    /// nor anything else that is not a Back up now file. The scheduled and
    /// stray files are written FIRST, so they are the oldest files in the
    /// folder: a prune that ranked every backup file by age would take them.
    ///
    /// Red check, run 2026-10-03: with the candidate filter in
    /// `prune_manual_backups` widened from `manual_backup_stamp(&name)` to
    /// `is_backup_file(&name).then(|| manual_backup_stamp(&name).unwrap_or(0))`
    /// (every backup file a candidate, the scheduled ones ranked oldest),
    /// this failed with "a scheduled snapshot was pruned:
    /// relay-20261003-044437.db.aes". With the prune call in `backup_into`
    /// replaced by `PruneOutcome::Removed(Vec::new())` instead, it failed on
    /// the cap: "manual copies still capped" (left: 13, right: 10). Both
    /// re-run after the clock-back skip was added (2026-10-03).
    #[test]
    fn pruning_never_touches_scheduled_snapshots() {
        let dir = scratch("sched");
        let bdir = dir.join(BACKUPS_DIR);
        std::fs::create_dir_all(&bdir).unwrap();
        let kept_untouched = [
            "relay-20261003-044437.db.aes",
            "relay-20261003-051437.db",
            "relay_20261002_233715.db.enc",
            "relay_20261002_173715.db",
            // Near misses on the manual pattern: not ours, so not pruned.
            "manual-notes.txt",
            "manual-.db.enc",
            "manual-12a.db.enc",
            "manual-5.db.aes",
            "backup-pull.log",
        ];
        for name in kept_untouched {
            std::fs::write(bdir.join(name), name.as_bytes()).unwrap();
        }
        let db = crate::relay::storage::Storage::open(&dir.join("live.db")).expect("open");
        let base: u64 = 1_790_000_000;
        for i in 0..(MANUAL_BACKUPS_KEPT + 3) as u64 {
            db.backup_into(&bdir, base + i).expect("press");
        }
        for name in kept_untouched {
            let body = std::fs::read(bdir.join(name));
            assert!(body.is_ok(), "a scheduled snapshot was pruned: {name}");
            assert_eq!(body.unwrap(), name.as_bytes(), "{name} was rewritten");
        }
        assert_eq!(manual_names(&bdir).len(), MANUAL_BACKUPS_KEPT, "manual copies still capped");
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// When the clock has gone back, so a copy already in the folder is
    /// dated after the one a press just made, that press deletes nothing,
    /// logs why and tells the admin (critic's review, 2026-10-03). Ranking
    /// by stamp alone deleted the PREVIOUS press's copy on every press
    /// while the future-dated copies stayed: with copies 1000 to 1009 in
    /// the folder, presses at 500, 560 and 620 deleted 1000, then 500, then
    /// 560, so a copy taken just before a risky step died on the next press.
    /// Once the clock passes the future-dated copies, pruning resumes.
    ///
    /// Red check, run 2026-10-03: with the `SkippedNewerExists` early
    /// return removed from `prune_manual_backups`, this failed with
    /// "assertion `left == right` failed: the press at 500 removed nothing"
    /// (left: Removed(["manual-1000.db.enc"]), right:
    /// SkippedNewerExists("manual-1009.db.enc")). With `deleted.reverse()`
    /// removed instead, the last step failed with "pruning resumes once the
    /// clock passes the future-dated copies" (left: the same four names,
    /// newest first).
    #[test]
    fn a_press_after_the_clock_went_back_removes_nothing() {
        let dir = scratch("clockback");
        let bdir = dir.join(BACKUPS_DIR);
        std::fs::create_dir_all(&bdir).unwrap();
        let kept = MANUAL_BACKUPS_KEPT as u64;
        let future: Vec<String> = (1000..1000 + kept).map(|s| format!("manual-{s}.db.enc")).collect();
        for name in &future {
            std::fs::write(bdir.join(name), b"x").unwrap();
        }
        let newest_future = format!("manual-{}.db.enc", 1000 + kept - 1);
        let db = crate::relay::storage::Storage::open(&dir.join("live.db")).expect("open");
        let mut made = Vec::new();
        for secs in [500u64, 560, 620] {
            let done = db.backup_into(&bdir, secs).expect("press");
            assert_eq!(
                done.prune,
                PruneOutcome::SkippedNewerExists(newest_future.clone()),
                "the press at {secs} removed nothing"
            );
            let reply = done.reply();
            assert!(
                reply.ends_with(&format!(
                    "No older copy was removed this time: {newest_future} is dated later than this \
                     one, so the server clock seems to have gone back."
                )),
                "the admin is told why: {reply}"
            );
            made.push(done.entry.file);
        }
        let mut expected = future.clone();
        expected.extend(made.iter().cloned());
        expected.sort();
        assert_eq!(manual_names(&bdir), expected, "every copy is still there, the three new ones too");

        // The clock has caught up: this press prunes back to the limit,
        // oldest stamps first, and names all four.
        let done = db.backup_into(&bdir, 2000).expect("press");
        assert_eq!(
            done.prune,
            PruneOutcome::Removed(vec![
                "manual-500.db.enc".to_string(),
                "manual-560.db.enc".to_string(),
                "manual-620.db.enc".to_string(),
                "manual-1000.db.enc".to_string(),
            ]),
            "pruning resumes once the clock passes the future-dated copies"
        );
        assert_eq!(manual_names(&bdir).len(), MANUAL_BACKUPS_KEPT);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The copy a press just made keeps the first slot even when another
    /// copy shares its second, and survives even when nothing is kept.
    /// Equal stamps are not "newer" (a plain `.db` and a sealed `.db.enc`
    /// from the same second: the key existed on the first press, so it was
    /// sealed, and could not be read on the second, so that one stayed plain;
    /// the other order cannot happen, because the plain file already there
    /// stops the second VACUUM INTO), so pruning runs and the tie must still
    /// go to the new copy.
    ///
    /// Two guards hold this, and each was broken on its own, 2026-10-03:
    /// - With the `(b.1 == just_made)` key removed from the sort, the loop's
    ///   skip still saved the new copy but it no longer held the slot, so
    ///   one copy too many stayed: "assertion `left == right` failed: the
    ///   new copy holds the one slot" (left: ["manual-100.db",
    ///   "manual-100.db.enc"], right: ["manual-100.db.enc"]).
    /// - With the loop's `if name == just_made` skip removed, the keep-0
    ///   half failed with "the copy just made was deleted with nothing
    ///   kept: manual-2.db.enc".
    #[test]
    fn the_copy_just_made_always_keeps_its_slot() {
        let dir = scratch("slot");
        for n in ["manual-90.db.enc", "manual-95.db.enc", "manual-100.db", "manual-100.db.enc"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let out = prune_manual_backups(&dir, 1, "manual-100.db.enc");
        assert_eq!(manual_names(&dir), vec!["manual-100.db.enc"], "the new copy holds the one slot");
        assert_eq!(
            out,
            PruneOutcome::Removed(vec![
                "manual-90.db.enc".to_string(),
                "manual-95.db.enc".to_string(),
                "manual-100.db".to_string(),
            ]),
            "the rest are removed, named oldest first"
        );
        let _ = std::fs::remove_dir_all(&dir);

        let dir = scratch("slot0");
        for n in ["manual-1.db.enc", "manual-2.db.enc"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let out = prune_manual_backups(&dir, 0, "manual-2.db.enc");
        assert!(
            dir.join("manual-2.db.enc").exists(),
            "the copy just made was deleted with nothing kept: manual-2.db.enc"
        );
        assert_eq!(out, PruneOutcome::Removed(vec!["manual-1.db.enc".to_string()]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The admin's confirmation names every copy the press deleted (critic's
    /// review, 2026-10-03: the names used to go only to the server log, so
    /// a copy taken before a migration could vanish ten presses later with
    /// no notice). Through the real path (VACUUM INTO, seal, prune):
    /// - presses up to the limit delete nothing and the reply says only
    ///   what was made;
    /// - the next press names the one copy it removed;
    /// - a first press into a folder where copies piled up (a server that
    ///   ran before the limit existed) names them all, oldest first.
    ///
    /// Red checks, run 2026-10-03, each on its own:
    /// - `reply` returning only "Backup complete: ... MB." whatever was
    ///   pruned failed with "the reply names the removed copy: Backup
    ///   complete: manual-1790000010.db.enc (0.9 MB)." The same text came
    ///   back with the prune call in `backup_into` replaced by
    ///   `PruneOutcome::Removed(Vec::new())`.
    /// - With `deleted.reverse()` removed from `prune_manual_backups`, it
    ///   failed with "the reply names them oldest first: Backup complete:
    ///   manual-1790000000.db.enc (0.9 MB); removed the 3 oldest copies (the
    ///   reply has said "copies with the earliest stamps" since the wording
    ///   review the same night),
    ///   manual-3.db.enc, manual-2.db.enc, manual-1.db.enc, to keep the
    ///   newest 10."
    #[test]
    fn the_reply_names_every_copy_removed() {
        let dir = scratch("reply");
        let db = crate::relay::storage::Storage::open(&dir.join("live.db")).expect("open");
        let base: u64 = 1_790_000_000;
        let kept = MANUAL_BACKUPS_KEPT as u64;

        let bdir = dir.join(BACKUPS_DIR);
        for i in 0..kept {
            let reply = db.backup_into(&bdir, base + i).expect("press").reply();
            assert!(
                reply.starts_with(&format!("Backup complete: manual-{}.db.enc (", base + i))
                    && reply.ends_with(" MB)."),
                "under the limit the reply only says what was made: {reply}"
            );
        }
        let reply = db.backup_into(&bdir, base + kept).expect("press").reply();
        assert!(
            reply.ends_with(&format!(
                " MB); removed the copy with the earliest stamp, manual-{base}.db.enc, to keep the newest {MANUAL_BACKUPS_KEPT}."
            )),
            "the reply names the removed copy: {reply}"
        );

        let piled = dir.join("piled");
        std::fs::create_dir_all(&piled).unwrap();
        for s in 1..=kept + 2 {
            std::fs::write(piled.join(format!("manual-{s}.db.enc")), b"x").unwrap();
        }
        let reply = db.backup_into(&piled, base).expect("press").reply();
        assert!(
            reply.ends_with(&format!(
                " MB); removed the 3 copies with the earliest stamps, manual-1.db.enc, manual-2.db.enc, \
                 manual-3.db.enc, to keep the newest {MANUAL_BACKUPS_KEPT}."
            )),
            "the reply names them oldest first: {reply}"
        );
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
