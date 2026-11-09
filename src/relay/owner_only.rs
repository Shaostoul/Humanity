//! The relay's private files, readable by the relay's own account alone (BUG-285, 2026-11-07).
//!
//! WHY. On united-humanity.us the live database was mode 644 in a 755 folder, so every account
//! on the box (www-data for nginx, the Forgejo service, any other) could read every member's
//! profile, room messages, push addresses and reports, and `vapid_private.key` (the Web Push
//! signing key, made before keys were written private) was 644 too. Nothing set a umask, so SQLite
//! made its files with the default 644. nginx never served `/opt`, so this was defence in depth,
//! not a known leak.
//!
//! WHAT. At every start, right after the database opens (`run_relay`), [`tighten`] sets the
//! database, its write-ahead log and shared memory, every key and secrets file beside it, and the
//! folders of private copies (the backups, continuous backup's work and the copies it keeps for
//! other servers, the members-only and in-progress uploads) to 0600 for files and 0700 for folders.
//! SQLite gives a write-ahead log or shared-memory file it makes later the mode of the database
//! file, so they stay private. New files: the relay's unit carries `UMask=0077` (the drop-in
//! `scripts/systemd/humanity-relay.service.d/owner-only.conf`, installed on every server by
//! `install-server-files.sh`), and the relay's own sealed backups are written 0600 here too
//! ([`write`]), for a server run without that unit.
//!
//! NOT TOUCHED. The database's own folder (on a server it is the checkout's `data/`, which also
//! holds the game data), the public uploads (`data/uploads`, served by the relay itself through
//! nginx's proxy), and anything not named here. Checked on both servers on 2026-11-07, read only:
//! nginx proxies `/uploads/` to the relay and serves only `/var/www`; the backup timer, the disk
//! guard, the watchdog, the deploy and the operator's backup pull all run as root, which reads
//! 0600 files whoever owns them.
//!
//! A file someone else owns (a deploy's copy of the program in `<repo>/backups`, root's) is not the
//! relay's to change and is passed over: only what has the database file's owner is changed.
//!
//! On any system without unix permissions (the desktop app on Windows hosting a node) this does
//! nothing: [`apply`] is a no-op there.

use std::path::{Path, PathBuf};

/// The mode of a private file: read and write by the owner only.
pub const FILE_MODE: u32 = 0o600;
/// The mode of a private folder: the owner only.
pub const DIR_MODE: u32 = 0o700;

/// Files directly beside the database that are private by name, whatever their number.
fn private_by_name(name: &str) -> bool {
    name.ends_with(".key")
        || name.ends_with(".secrets.json")
        || name == "backup-host.json"
        || name == "owner-claim-code.txt"
}

/// The folders beside the database that hold only private copies: the relay's 6-hourly sealed
/// backups, continuous backup's work, the copies it keeps for other servers, and the uploads that
/// are not public (members-only Files items, uploads in progress, items waiting to publish).
pub const PRIVATE_FOLDERS: &[&str] = &["backups", "backup-stream", "backup-host", "media-members", "uploads-resume", "media-waiting"];

/// Everything [`tighten`] would make private for the database at `db_path`, with the mode each
/// gets, among what exists now: the database and its companions, the private files beside it,
/// [`PRIVATE_FOLDERS`] and all they hold, and `repo_backups` (the folder "Back up now" and the
/// backup timer write to, `<repo>/backups`) with what it holds. Symbolic links are never followed.
pub fn plan(db_path: &Path, repo_backups: Option<&Path>) -> Vec<(PathBuf, u32)> {
    let mut out = Vec::new();
    let is_real = |p: &Path| std::fs::symlink_metadata(p).map(|m| !m.file_type().is_symlink()).unwrap_or(false);
    let db_name = db_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = db_path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let p = dir.join(format!("{db_name}{suffix}"));
        if is_real(&p) {
            out.push((p, FILE_MODE));
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut beside: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter(|e| private_by_name(&e.file_name().to_string_lossy()))
            .map(|e| e.path())
            .collect();
        beside.sort();
        out.extend(beside.into_iter().map(|p| (p, FILE_MODE)));
    }
    let mut folders: Vec<PathBuf> = PRIVATE_FOLDERS.iter().map(|f| dir.join(f)).collect();
    if let Some(b) = repo_backups {
        if !folders.iter().any(|f| f == b) {
            folders.push(b.to_path_buf());
        }
    }
    for folder in folders {
        walk(&folder, &mut out);
    }
    out
}

/// `folder` (if it is a real folder) and everything under it, folders first.
fn walk(folder: &Path, out: &mut Vec<(PathBuf, u32)>) {
    let Ok(meta) = std::fs::symlink_metadata(folder) else { return };
    if !meta.is_dir() {
        return;
    }
    out.push((folder.to_path_buf(), DIR_MODE));
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut children: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    children.sort_by_key(|e| e.file_name());
    for e in children {
        let Ok(t) = e.file_type() else { continue };
        if t.is_dir() {
            walk(&e.path(), out);
        } else if t.is_file() {
            out.push((e.path(), FILE_MODE));
        }
    }
}

/// Give each path in `plan` owned by `owner` (a user id; any owner when None) its mode where it
/// differs. Returns (changed, could not change).
#[cfg(unix)]
pub fn apply(plan: &[(PathBuf, u32)], owner: Option<u32>) -> (usize, Vec<PathBuf>) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let mut changed = 0;
    let mut failed = Vec::new();
    for (path, mode) in plan {
        let Ok(meta) = std::fs::symlink_metadata(path) else { continue };
        if meta.permissions().mode() & 0o777 == *mode || owner.is_some_and(|o| meta.uid() != o) {
            continue;
        }
        match std::fs::set_permissions(path, std::fs::Permissions::from_mode(*mode)) {
            Ok(()) => changed += 1,
            Err(_) => failed.push(path.clone()),
        }
    }
    (changed, failed)
}

/// No unix permissions here: nothing to do.
#[cfg(not(unix))]
pub fn apply(_plan: &[(PathBuf, u32)], _owner: Option<u32>) -> (usize, Vec<PathBuf>) {
    (0, Vec::new())
}

/// Make the private files of the database at `db_path` owner-only ([`plan`], [`apply`]), and say
/// in the log what changed and what could not be.
pub fn tighten(db_path: &Path, repo_backups: Option<&Path>) {
    let (changed, failed) = apply(&plan(db_path, repo_backups), owner_of(db_path));
    if changed > 0 {
        tracing::info!("owner-only: {changed} private files and folders made readable by this account alone");
    }
    if !failed.is_empty() {
        tracing::warn!(
            "owner-only: {} could not be made private: {}",
            failed.len(),
            failed.iter().take(5).map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        );
    }
}

/// The user id that owns `path` (the database file: this relay's account), on unix.
fn owner_of(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).ok().map(|m| m.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// `bytes` written to `path` (replacing any file there) through a new file that is private from
/// its first byte on unix, then renamed into place: the relay's sealed backups and a restore's
/// plain copy, which a server run without the unit's umask would otherwise write 0644.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.{:08x}.new", rand::random::<u32>()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(FILE_MODE);
    }
    let res = opts.open(&tmp).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    });
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

#[cfg(test)]
mod tests {
    //! The plan is tested on every system; the modes themselves only where unix permissions exist.
    //! Seen red on 2026-11-07: before this module nothing at all set a mode on the database, its
    //! log or `vapid_private.key` (the live files were 0644), so there was no plan to test; the
    //! node test `scripts/tests/owner-only-unit.test.js` was red against the unit and its drop-in.
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hos-owner-only-{tag}-{}", rand::random::<u32>()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn the_plan_names_every_private_file_and_nothing_public() {
        let root = scratch("plan");
        let data = root.join("data");
        for f in [
            "relay.db", "relay.db-wal", "relay.db-shm", "backup.key", "erased-accounts.key", "vapid_private.key",
            "backup-destination.secrets.json", "backup-host.json", "owner-claim-code.txt",
            "backups/relay_20261107_001625.db.enc", "backup-stream/pending/seg-1", "backup-host/abcd/g/seg-2",
            "media-members/12_plan.pdf", "uploads-resume/3_part", "media-waiting/4_item.zip",
            // Public, or not the relay's to judge: never in the plan.
            "uploads/1791581986303_photo.jpg", "plants.csv", "i18n/en.json", "notes.txt",
        ] {
            touch(&data.join(f));
        }
        touch(&root.join("backups").join("relay-20261107-000000.db.aes"));
        let repo_backups = root.join("backups");
        let plan = plan(&data.join("relay.db"), Some(&repo_backups));
        let has = |p: &Path, mode: u32| plan.iter().any(|(q, m)| q == p && *m == mode);
        for f in ["relay.db", "relay.db-wal", "relay.db-shm", "backup.key", "erased-accounts.key", "vapid_private.key",
            "backup-destination.secrets.json", "backup-host.json", "owner-claim-code.txt",
            "backups/relay_20261107_001625.db.enc", "backup-stream/pending/seg-1", "backup-host/abcd/g/seg-2",
            "media-members/12_plan.pdf", "uploads-resume/3_part", "media-waiting/4_item.zip"] {
            assert!(has(&data.join(f), FILE_MODE), "{f} is made 0600: {plan:?}");
        }
        for d in ["backups", "backup-stream", "backup-stream/pending", "backup-host", "backup-host/abcd/g", "media-members"] {
            assert!(has(&data.join(d), DIR_MODE), "{d}/ is made 0700");
        }
        assert!(has(&repo_backups, DIR_MODE) && has(&repo_backups.join("relay-20261107-000000.db.aes"), FILE_MODE), "<repo>/backups too");
        for f in ["uploads", "uploads/1791581986303_photo.jpg", "plants.csv", "i18n/en.json", "notes.txt"] {
            assert!(!plan.iter().any(|(q, _)| q == &data.join(f)), "{f} is left as it is");
        }
        assert!(!plan.iter().any(|(q, _)| q == &data), "the database's own folder (the game data's too) is left as it is");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn tighten_makes_them_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("modes");
        let data = root.join("data");
        for f in ["relay.db", "relay.db-wal", "vapid_private.key", "backups/a.db.enc", "uploads/b.jpg"] {
            touch(&data.join(f));
            std::fs::set_permissions(data.join(f), std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        tighten(&data.join("relay.db"), None);
        let mode = |f: &str| std::fs::metadata(data.join(f)).unwrap().permissions().mode() & 0o777;
        for f in ["relay.db", "relay.db-wal", "vapid_private.key", "backups/a.db.enc"] {
            assert_eq!(mode(f), 0o600, "{f}");
        }
        assert_eq!(mode("backups"), 0o700);
        assert_eq!(mode("uploads/b.jpg"), 0o644, "a public upload is left as it is");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_replaces_the_file_and_leaves_no_temporary() {
        let root = scratch("write");
        let p = root.join("relay_x.db.enc");
        write(&p, b"first").unwrap();
        write(&p, b"second").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"second");
        let left: Vec<_> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "no temporary left: {left:?}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
