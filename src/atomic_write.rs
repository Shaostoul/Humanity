//! Writing a file so a crash at any moment leaves either the whole old file or the whole new
//! one: the bytes go to a temp file beside it (`<name>.tmp`), are flushed to the disk, and the
//! temp file is renamed over the real one, retried briefly while another program holds it.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was `persistence`'s (the saves were its first user), and the
//! settings, the protected setup's file on the network side, the how-to runs and the GUI's own
//! files all write through it, so the network side reached into the simulation for a file write.
//! Std only. `staging_path` and `stage_write` are public for the saves' crash test
//! (`persistence`, `a_crash_between_write_and_rename_leaves_the_previous_save`).

use std::path::{Path, PathBuf};

/// The temp file a save is written to before it is renamed over the real one:
/// `offline_home.json.tmp`, in the same folder (a rename only replaces
/// atomically within one volume). Its extension is not `.json`, so nothing
/// that scans the saves folder for saves ever mistakes it for one.
pub fn staging_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Step one of a crash-safe write: put all the bytes in the temp file and
/// flush them to the disk. The real file is not touched. A crash after this
/// and before `commit_staged` leaves the real file exactly as it was.
pub fn stage_write(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    let tmp = staging_path(path);
    let mut f = std::fs::File::create(&tmp).map_err(|e| format!("could not create {}: {e}", tmp.display()))?;
    f.write_all(bytes).map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
    // On the disk, not just in the OS cache, before the rename makes it the
    // save: otherwise a power cut could leave a renamed but empty file.
    f.sync_all().map_err(|e| format!("could not flush {}: {e}", tmp.display()))?;
    Ok(tmp)
}

/// Step two: rename the temp file over the real one. The rename replaces the
/// file in one step, so a reader (or a crash) sees the old save or the new
/// one, never half of either. On Windows a rename can fail for a moment while
/// another program (an antivirus scan, a backup tool) holds the target open,
/// so it is retried briefly before giving up; giving up leaves the old save
/// intact.
fn commit_staged(tmp: &Path, path: &Path) -> Result<(), String> {
    let mut last_err = None;
    for attempt in 0..5u32 {
        match std::fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(10 << attempt));
            }
        }
    }
    Err(format!(
        "could not replace {} (the previous version is still there): {}",
        path.display(),
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// Write `bytes` to `path` so that a crash at any moment leaves either the
/// whole old file or the whole new one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = stage_write(path, bytes)?;
    commit_staged(&tmp, path)
}
