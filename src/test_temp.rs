//! Temporary files and folders for tests that delete themselves (BUG-159).
//! Test builds only: the product cannot reach it.
//!
//! Tests that needed a database or a scratch folder used to build their own
//! path, `std::env::temp_dir().join(format!("hum_..."))`, in some sixty places,
//! and nothing deleted it afterwards. By 2026-10-05 the system temp folder held
//! 186,523 `hum_*` entries, 81,026 of them SQLite databases, and every full test
//! run added 740 more.
//!
//! A test now asks this module for its path and holds the guard it gets back.
//! When the guard is dropped, at the end of the test or while a failed
//! assertion unwinds it, the guard deletes what is at the path: a file and,
//! beside it, the `-wal`, `-shm` and `-journal` files SQLite keeps for a
//! database, or a folder with everything in it.
//!
//! Windows will not delete a file SQLite still has open, and a guard is not
//! always the last thing to let go of its database: a relay test's runtime drops
//! the relay's tasks, and the database they hold, only after the test body and
//! its locals are gone, and r2d2 can close a pool's last read connection a
//! moment after its storage on a thread of its own. So a delete that fails is
//! not given up: the path is kept and tried again each time another guard is
//! dropped, and a last time as the test binary exits. Whatever is still open
//! even then (a database held by a thread nobody stopped) is named on stderr and
//! left for `just clean-test-temp`.
//!
//! Names keep the `hum_` prefix every one of those paths had, so that recipe
//! finds what a killed run leaves behind, and stay unique per test and per
//! process: `hum_<tag>_<pid>_<nanos>_<n>`, plus an extension for a file.
//!
//! For the relay's databases, `Storage::open_temp(tag)` opens a fresh database
//! on such a path and keeps the guard INSIDE the storage, so the files go when
//! the storage does, whoever drops it last. A test that reopens the same file
//! (a restart, a migration) holds a `db(tag)` guard itself instead, declared
//! before the storages it opens on it so it is dropped after them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once};

/// The prefix every path this module hands out starts with. `just
/// clean-test-temp` deletes entries with it that are more than a day old.
pub(crate) const PREFIX: &str = "hum_";

/// The files SQLite keeps beside a database: its write-ahead log, the log's
/// shared-memory index, and the rollback journal (not used in WAL mode, but a
/// database opened without WAL leaves one behind if a write is interrupted).
const SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// Paths whose delete failed because something still had them open. Tried
/// again whenever a guard is dropped, and at exit (see `arm_exit_sweep`).
static LEFT_OPEN: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// A path in the system temp folder that is deleted, with everything under and
/// beside it, when this guard is dropped. Derefs to `Path`, so `&guard` goes
/// wherever a `&Path` does and `guard.join(..)` works as on a path.
pub(crate) struct TempPath {
    path: PathBuf,
}

/// A path for a SQLite database, `hum_<tag>_<pid>_<nanos>_<n>.db`. Nothing is
/// created: opening it creates it.
pub(crate) fn db(tag: &str) -> TempPath {
    file(tag, "db")
}

/// A path for a file with the extension `ext`. Nothing is created.
pub(crate) fn file(tag: &str, ext: &str) -> TempPath {
    TempPath { path: unique(tag, Some(ext)) }
}

/// A path with no extension, for a folder the code under test makes itself or a
/// test needs to be absent. Nothing is created; whatever ends up there, file or
/// folder, is deleted.
pub(crate) fn path(tag: &str) -> TempPath {
    TempPath { path: unique(tag, None) }
}

/// A new, empty folder.
pub(crate) fn dir(tag: &str) -> TempPath {
    let guard = TempPath { path: unique(tag, None) };
    std::fs::create_dir_all(&guard.path)
        .unwrap_or_else(|e| panic!("create the test folder {}: {e}", guard.path.display()));
    guard
}

/// `hum_<tag>_<pid>_<nanos>_<n>[.<ext>]` in the temp folder. The process id
/// keeps parallel test binaries apart, the counter keeps tests in one process
/// apart, and the time keeps a run from meeting what an earlier run under the
/// same (reused) process id left behind.
fn unique(tag: &str, ext: Option<&str>) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut name = format!("{PREFIX}{tag}_{}_{nanos}_{n}", std::process::id());
    if let Some(ext) = ext {
        name.push('.');
        name.push_str(ext);
    }
    std::env::temp_dir().join(name)
}

impl TempPath {
    /// The path itself, for the rare call that wants a `&Path` spelled out.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl std::ops::Deref for TempPath {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Debug for TempPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TempPath").field(&self.path).finish()
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let path = std::mem::take(&mut self.path);
        if !remove(&path) {
            // Still open somewhere (see the module notes): keep it for later.
            left_open().push(path);
            arm_exit_sweep();
        }
        retry_left_open();
    }
}

/// Delete what is at `path`: a folder and everything in it, or a file and the
/// files SQLite keeps beside it. True when nothing of it is left.
fn remove(path: &Path) -> bool {
    if path.is_dir() {
        return gone(std::fs::remove_dir_all(path));
    }
    let mut all = gone(std::fs::remove_file(path));
    for suffix in SIDECARS {
        let mut beside = path.as_os_str().to_owned();
        beside.push(suffix);
        all &= gone(std::fs::remove_file(PathBuf::from(beside)));
    }
    all
}

/// A delete that worked, or found nothing to delete.
fn gone(result: std::io::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

fn left_open() -> std::sync::MutexGuard<'static, Vec<PathBuf>> {
    // Nothing panics while holding the lock, but a poisoned list is still a list.
    LEFT_OPEN.lock().unwrap_or_else(|p| p.into_inner())
}

/// Try again to delete every path whose guard could not delete it, keeping the
/// ones still open. Runs on every guard drop; cheap when the list is empty.
pub(crate) fn retry_left_open() {
    left_open().retain(|p| !remove(p));
}

extern "C" {
    /// The C library's exit hook. `std::process::exit` and a return from
    /// `main` both end in the C runtime's `exit`, which runs these.
    fn atexit(callback: extern "C" fn()) -> std::os::raw::c_int;
}

/// The first time a delete has to wait, register one last try for when the
/// test binary exits: by then every test has returned and dropped its runtime,
/// so whatever held the files has closed them.
fn arm_exit_sweep() {
    static ARMED: Once = Once::new();
    ARMED.call_once(|| {
        // SAFETY: registers a plain function with no captured state; the C
        // runtime calls it once, on the main thread, during `exit`.
        unsafe {
            atexit(sweep_at_exit);
        }
    });
}

/// The last try, at exit. Must not panic: a panic cannot unwind out of a C
/// callback, it aborts. Names what it still could not delete.
extern "C" fn sweep_at_exit() {
    let Ok(mut left) = LEFT_OPEN.lock() else { return };
    left.retain(|p| !remove(p));
    if !left.is_empty() {
        use std::io::Write as _;
        let mut err = std::io::stderr();
        let _ = writeln!(
            err,
            "test_temp: {} temporary path(s) were still open when the tests ended and are left \
             behind (just clean-test-temp deletes them once a day old):",
            left.len()
        );
        for p in left.iter() {
            let _ = writeln!(err, "  {}", p.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file, the files SQLite keeps beside it, and a folder with something in
    /// it are each gone once their guard is dropped, and a guard dropped while
    /// a failed assertion unwinds the test deletes as well.
    ///
    /// Seen red 2026-10-05 with the delete taken out of `Drop`: "the guards
    /// left these behind after the test panicked: [...hum_guard_db_..._2.db-wal,
    /// ...db-shm, ...db-journal, ...db, ...hum_guard_dir_..._5,
    /// ...hum_guard_file_..._6.ron]".
    #[test]
    fn a_dropped_guard_deletes_what_it_made_even_when_the_test_panics() {
        let made: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let db = db("guard_db");
            std::fs::write(&db, b"database").unwrap();
            for suffix in SIDECARS {
                let mut beside = db.as_os_str().to_owned();
                beside.push(suffix);
                std::fs::write(PathBuf::from(&beside), b"beside").unwrap();
                made.lock().unwrap().push(PathBuf::from(beside));
            }
            let folder = dir("guard_dir");
            std::fs::create_dir_all(folder.join("nested")).unwrap();
            std::fs::write(folder.join("nested").join("inside.txt"), b"inside").unwrap();
            let plain = file("guard_file", "ron");
            std::fs::write(&plain, b"( plain )").unwrap();
            made.lock().unwrap().extend([db.to_path_buf(), folder.to_path_buf(), plain.to_path_buf()]);
            for p in made.lock().unwrap().iter() {
                assert!(p.exists(), "the test made {}", p.display());
            }
            panic!("an assertion in the test failed");
        }));
        assert!(unwound.is_err(), "the closure panicked as it was written to");
        let made = made.into_inner().unwrap();
        assert_eq!(made.len(), 6, "a database, its three sidecar files, a folder and a file");
        let left: Vec<&PathBuf> = made.iter().filter(|p| p.exists()).collect();
        assert!(left.is_empty(), "the guards left these behind after the test panicked: {left:?}");
    }

    /// Names start with `hum_` (the sweep recipe's handle) and never repeat.
    #[test]
    fn names_are_prefixed_and_unique() {
        let a = db("names");
        let b = db("names");
        let c = dir("names");
        for p in [a.path(), b.path(), c.path()] {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            assert!(name.starts_with("hum_names_"), "{name}");
            assert_eq!(p.parent(), Some(std::env::temp_dir().as_path()), "{name} is in the temp folder");
        }
        assert_ne!(a.path(), b.path(), "two guards for the same tag got the same path");
        assert_eq!(a.extension().and_then(|e| e.to_str()), Some("db"));
        assert!(c.is_dir(), "dir() creates the folder");
    }

    /// The case Windows makes real: SQLite still has the database open when
    /// its guard is dropped (a relay whose runtime outlives the test body), so
    /// the delete fails. The path is kept, and goes once the database closes
    /// and another guard drops. (On Linux an open file can be deleted, so the
    /// first try already succeeds; the test holds there too.)
    ///
    /// Seen red 2026-10-05 on Windows with the failed path dropped instead of
    /// kept for the retry: "the database its guard could not delete while open
    /// was still there after it closed".
    #[cfg(feature = "relay")]
    #[test]
    fn a_database_still_open_when_its_guard_drops_goes_once_it_closes() {
        let guard = db("guard_still_open");
        let path = guard.to_path_buf();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t (x); INSERT INTO t VALUES (1);")
            .unwrap();
        drop(guard);
        drop(conn);
        drop(file("guard_still_open_next", "txt"));
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        assert!(
            !path.exists() && !PathBuf::from(&wal).exists(),
            "the database its guard could not delete while open was still there after it closed"
        );
    }
}
