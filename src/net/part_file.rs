//! Files written a piece at a time (a download, a private file's Save, Play here's copy) go to a
//! hidden PART file first and get their real name only when they are complete (round 3 of
//! docs/design/files-showcase.md, the 2026-10-11 review's finding 6: a download cut off by the
//! app quitting left a partial file under its final name, and a private file's Save left its
//! `.humanityos-*.part` in the downloads folder).
//!
//! THE RULES:
//! - A part file is named `.humanityos-<process>-<n>-<nanos>.part`, made with `create_new`, in
//!   the folder the finished file goes to (so the move at the end never crosses a disk).
//! - [`Part::finish`] gives it its real name, never over an existing file and never by "check, then
//!   create" (finding 7, the race between two downloads of one name): the name is CLAIMED, by a
//!   hard link that fails if the name exists, or, on a file system without hard links, by making
//!   an empty file there with `create_new` and moving the part over it.
//! - A part that is dropped without being finished is deleted (a failure, a cancel, a panic).
//! - Every part this run made is listed, so the app's exit deletes the ones still being written
//!   ([`sweep_own`], engine/quit.rs); the next start deletes what a run that did not close cleanly
//!   left ([`sweep_stale`], only parts nobody has written to for [`STALE_AFTER`], so another
//!   instance's transfer under way is never touched).
//!
//! Plain std, no widget: it lived in gui/widgets/ until the crate split's rung 0, when it moved to
//! the network side, the lowest part of the app that writes one (net/file_transfer.rs); the Files
//! page and the chat's saves use it from here.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// What every part file's name starts with.
pub(crate) const PREFIX: &str = ".humanityos-";
/// What every part file's name ends with.
pub(crate) const SUFFIX: &str = ".part";
/// A part nobody has written to for this long is a leftover. A transfer that goes quiet for a
/// minute is given up (open.rs `transfer_agent`), so a live one is never this old.
pub(crate) const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

/// The part files this run made and has not finished or deleted yet.
static OPEN: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn open_list() -> std::sync::MutexGuard<'static, Vec<PathBuf>> {
    OPEN.lock().unwrap_or_else(|p| p.into_inner())
}

/// Whether `name` is one of this app's part files.
pub(crate) fn is_part_name(name: &str) -> bool {
    name.starts_with(PREFIX) && name.ends_with(SUFFIX)
}

/// A hidden file being written, deleted unless it is finished.
pub(crate) struct Part {
    path: PathBuf,
    file: Option<File>,
}

impl Part {
    /// A new part file in `dir` (made if missing), open for writing.
    pub(crate) fn create(dir: &Path) -> Result<Part, String> {
        static N: AtomicU64 = AtomicU64::new(0);
        std::fs::create_dir_all(dir).map_err(|e| format!("The folder could not be made ({e})."))?;
        for _ in 0..100 {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            let name = format!("{PREFIX}{}-{}-{nanos:x}{SUFFIX}", std::process::id(), N.fetch_add(1, Ordering::SeqCst));
            let path = dir.join(name);
            match std::fs::OpenOptions::new().write(true).read(true).create_new(true).open(&path) {
                Ok(file) => {
                    open_list().push(path.clone());
                    return Ok(Part { path, file: Some(file) });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("The file could not be written ({e}).")),
            }
        }
        Err("The file could not be written (no free name).".to_string())
    }

    /// Where it is.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The open file, to write into.
    pub(crate) fn file(&mut self) -> &mut File {
        self.file.as_mut().expect("a part's file is open until it is finished")
    }

    /// Done: written to disk, closed, and given the name `name` in `dir` (the same folder it was
    /// made in), or "name (2)" and so on when that is taken. Returns where it is now.
    pub(crate) fn finish(mut self, dir: &Path, name: &str) -> Result<PathBuf, String> {
        if let Some(file) = self.file.take() {
            file.sync_all().map_err(|e| format!("The file could not be written ({e})."))?;
        }
        let placed = claim_and_move(&self.path, dir, name)?;
        forget(&self.path);
        self.path = PathBuf::new(); // nothing left for Drop to delete
        Ok(placed)
    }
}

impl Drop for Part {
    fn drop(&mut self) {
        self.file = None; // closed first: Windows deletes an open file only once it is closed
        if !self.path.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.path);
            forget(&self.path);
        }
    }
}

fn forget(path: &Path) {
    open_list().retain(|p| p != path);
}

/// The name `name` in `dir`, then "stem (2).ext", "stem (3).ext" and so on.
fn candidates<'a>(dir: &'a Path, name: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..=999).map(move |n| dir.join(if n == 1 { name.to_string() } else { format!("{stem} ({n}){ext}") }))
}

/// Give the file at `from` the first free name for `name` in `dir`, claimed so that no other
/// writer can take the same one in between: a hard link fails when the name exists. Where hard
/// links are not possible, an empty file made with `create_new` claims the name and `from` is
/// moved over it.
pub(crate) fn claim_and_move(from: &Path, dir: &Path, name: &str) -> Result<PathBuf, String> {
    for target in candidates(dir, name) {
        match std::fs::hard_link(from, &target) {
            Ok(()) => {
                let _ = std::fs::remove_file(from);
                return Ok(target);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => match std::fs::OpenOptions::new().write(true).create_new(true).open(&target) {
                Ok(claim) => {
                    drop(claim);
                    std::fs::rename(from, &target).map_err(|e| {
                        let _ = std::fs::remove_file(&target);
                        format!("The file could not be named ({e}).")
                    })?;
                    return Ok(target);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("The file could not be named ({e}).")),
            },
        }
    }
    Err("There is no free name for the file in that folder.".to_string())
}

/// The app is closing (engine/quit.rs): delete every part this run is still writing.
pub(crate) fn sweep_own() {
    let paths: Vec<PathBuf> = std::mem::take(&mut *open_list());
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

/// [`sweep_own`] for the parts in `dir` only (the tests, which run side by side, each sweep their
/// own folder).
pub(crate) fn sweep_own_in(dir: &Path) {
    let paths: Vec<PathBuf> = {
        let mut list = open_list();
        let (mine, rest): (Vec<PathBuf>, Vec<PathBuf>) = list.drain(..).partition(|p| p.parent() == Some(dir));
        *list = rest;
        mine
    };
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

/// Whether `path` is a part file this run is writing (so the exit deletes it).
pub(crate) fn is_listed(path: &Path) -> bool {
    open_list().iter().any(|p| p == path)
}

/// Delete this app's part files in `dir` that nobody has written to for `older_than`: what a run
/// that did not close cleanly left. Returns how many went.
pub(crate) fn sweep_stale(dir: &Path, older_than: Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut gone = 0;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !is_part_name(&name) {
            continue;
        }
        let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).unwrap_or_default();
        if age >= older_than && std::fs::remove_file(e.path()).is_ok() {
            gone += 1;
        }
    }
    gone
}

// The downloads folder's once-a-run sweep (`sweep_downloads_once`) is the GUI's, beside the
// downloads folder it names (gui/widgets/image_cache.rs), since the crate split's rung 0.


#[cfg(test)]
#[path = "part_file_tests.rs"]
mod tests;
