//! The relay's database at start (BUG-230, 2026-11-03): what the check before the upgrade
//! found, the upgrade's one transaction, and the words a start that fails stops with.
//!
//! The flow is `Storage::open_resilient` in `mod.rs`: check the file read-only, then upgrade it
//! (`Storage::open`), and restore a backup only when the check found the FILE damaged. Until this
//! split, any error from the open, an upgrade step the code got wrong included, sent the file to
//! recovery: the newest data was moved aside as `relay.db.corrupt-<time>` and an older backup
//! started in its place. What the operator sees in each case: docs/INCIDENT-PLAYBOOK.md, "The
//! relay will not start after an update".

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A step of the upgrade (`Storage::open`) that failed: where it is in the code, and what SQLite
/// said. A failed step is the code's fault, not the file's, so the start stops with it named and
/// the file as it was (the whole upgrade is one transaction, undone), instead of sending the file
/// to recovery.
#[derive(Debug)]
pub struct UpgradeFailed {
    /// The `?` that returned the error: `src/relay/storage/mod.rs` and the line of the step.
    pub step: &'static std::panic::Location<'static>,
    /// What SQLite said.
    pub error: rusqlite::Error,
}

impl From<rusqlite::Error> for UpgradeFailed {
    /// `#[track_caller]` makes `Location::caller()` the `?` that converted the error (std's
    /// `?` passes its place through), so every step names itself without a label beside it.
    #[track_caller]
    fn from(error: rusqlite::Error) -> Self {
        Self { step: std::panic::Location::caller(), error }
    }
}

impl UpgradeFailed {
    /// The step's place, `src/relay/storage/mod.rs:<line>`, with forward slashes on every system
    /// (Windows builds record the path with backslashes).
    pub fn place(&self) -> String {
        format!("{}:{}", self.step.file().replace('\\', "/"), self.step.line())
    }
}

impl std::fmt::Display for UpgradeFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.place(), self.error)
    }
}

impl std::error::Error for UpgradeFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// What the check before any upgrade step found (`Storage::check_readonly`).
pub(super) enum Checked {
    /// It passed `PRAGMA quick_check`.
    Healthy,
    /// The bytes are wrong: SQLite says the file is not a database or is malformed, or
    /// quick_check listed damage. The words say what was found.
    Damaged(String),
    /// The check could not run, for a reason that says nothing about the bytes: the file could
    /// not be opened or read (its permissions, a lock another program holds, the disk).
    NotChecked(rusqlite::Error),
}

impl Checked {
    /// Only SQLite's two words for wrong bytes are damage: SQLITE_NOTADB ("file is not a
    /// database") and SQLITE_CORRUPT ("database disk image is malformed", every extended code of
    /// it). Anything else (CANTOPEN, PERM, BUSY, LOCKED, IOERR, NOMEM, READONLY) says the file
    /// could not be read, not that it is wrong, and moving it aside for an older backup would
    /// lose its newest data for nothing.
    pub(super) fn of_error(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseCorrupt) | Some(rusqlite::ErrorCode::NotADatabase) => Self::Damaged(error.to_string()),
            _ => Self::NotChecked(error),
        }
    }
}

/// Why `Storage::open_resilient` did not open the relay's database. Each case says in plain
/// words what happened to the file and what to do: [`OpenFailure::lines`] for the log, one line
/// each, and `Display` (the same lines joined) for the panic the start stops with, which the
/// desktop's Host a node page shows too.
#[derive(Debug)]
pub enum OpenFailure {
    /// The file is healthy (it passed the check), but a step of the upgrade failed. The upgrade
    /// is one transaction, undone, so nothing in the file changed and nothing was moved.
    /// `restored_from`: when it was a backup's copy whose upgrade failed (put in place of a
    /// damaged file just before), that backup.
    UpgradeFailed { path: PathBuf, failed: UpgradeFailed, restored_from: Option<PathBuf> },
    /// The check could not run (not damage). Nothing moved.
    NotChecked { path: PathBuf, error: rusqlite::Error },
    /// The file is damaged and no backup could be put in its place (none passed the check, or
    /// one could not be copied in). The damaged file is where it was.
    Damaged { path: PathBuf, backups: Vec<PathBuf>, damage: String },
}

impl OpenFailure {
    /// The words for the log, a sentence or two per line, so `journalctl -u humanity-relay`
    /// shows each whole.
    pub fn lines(&self) -> Vec<String> {
        match self {
            Self::UpgradeFailed { path, failed, restored_from } => {
                let mut lines = vec![
                    format!(
                        "DATABASE UPGRADE FAILED: the relay did not start. {} is healthy (it passed its check), but this release (v{}) could not upgrade it.",
                        path.display(),
                        env!("CARGO_PKG_VERSION")
                    ),
                    format!("The step that failed: {failed}."),
                ];
                lines.push(match restored_from {
                    None => "Nothing in the database was changed: the upgrade is one transaction, and it was undone. No file was moved and no backup was restored.".to_string(),
                    Some(backup) => format!(
                        "The database was damaged, so the backup {} was put in its place first (the damaged file is kept beside it, the lines above say where); the upgrade then failed on that copy, and left it as it was.",
                        backup.display()
                    ),
                });
                if failed.error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseCorrupt) {
                    lines.push("SQLite found damage during the upgrade although the check before it passed: run PRAGMA integrity_check on a copy of the file before deciding anything.".to_string());
                }
                lines.push("What to do: run the previous release again (it still opens this file as it is), and report the step above so the upgrade can be fixed. Do not restore a backup over this file: it holds the newest data.".to_string());
                lines
            }
            Self::NotChecked { path, error } => vec![
                format!("The relay did not start: {} could not be checked ({error}).", path.display()),
                "This is not damage: SQLite could not read the file (its permissions, a lock another program holds on it, or the disk). Nothing was moved or replaced.".to_string(),
                "What to do: fix what stops the relay reading the file, then start it again.".to_string(),
            ],
            Self::Damaged { path, backups, damage } => vec![
                format!(
                    "DB recovery FAILED: the relay did not start. {} is damaged ({damage}), and no backup in {} could be put in its place (none passed the check, or copying one in failed: the lines above say which).",
                    path.display(),
                    backups.iter().map(|b| b.display().to_string()).collect::<Vec<_>>().join(" or ")
                ),
                "The damaged file was left where it is; nothing was restored over it.".to_string(),
                "What to do: restore a known-good backup by hand (docs/INCIDENT-PLAYBOOK.md, \"SQLite corruption\"), or run scripts/pq-wipe.sh for an intentional fresh slate.".to_string(),
            ],
        }
    }
}

impl std::fmt::Display for OpenFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.lines().join(" "))
    }
}

impl std::error::Error for OpenFailure {}

/// The upgrade's one transaction: every step of `Storage::open` runs inside it, so the steps are
/// kept together or not at all. `begin` takes the write lock (BEGIN IMMEDIATE); `commit` keeps
/// every step; dropped uncommitted (a `?` returned early, at the step that failed) it undoes them.
///
/// While it is open, a commit hook refuses every commit but its own. SQLite answers a few errors
/// (a full disk, an I/O error, no memory) by undoing the whole transaction at once. In a step
/// whose error `open` ignores (the ALTER-if-missing loops, the search index's rebuild), the
/// steps after it would then each commit on their own, a half upgrade again; with the hook each
/// such commit is turned back into a rollback, and `commit` says the transaction ended early.
pub(super) struct UpgradeTransaction<'c> {
    conn: &'c Connection,
    /// Set just before the one COMMIT the hook lets through.
    committing: Arc<AtomicBool>,
    /// Set by the hook when it refused a commit: the transaction had ended before its end.
    refused: Arc<AtomicBool>,
    done: bool,
}

impl<'c> UpgradeTransaction<'c> {
    pub(super) fn begin(conn: &'c Connection) -> Result<Self, rusqlite::Error> {
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let committing = Arc::new(AtomicBool::new(false));
        let refused = Arc::new(AtomicBool::new(false));
        let (ours, refusals) = (committing.clone(), refused.clone());
        conn.commit_hook(Some(move || {
            if ours.load(Ordering::SeqCst) {
                return false;
            }
            refusals.store(true, Ordering::SeqCst);
            true // turn this commit into a rollback
        }));
        Ok(Self { conn, committing, refused, done: false })
    }

    /// Keep every step. Refused when the transaction ended before now (see the type's note).
    pub(super) fn commit(mut self) -> Result<(), rusqlite::Error> {
        self.done = true;
        let result = if self.refused.load(Ordering::SeqCst) || self.conn.is_autocommit() {
            let _ = self.conn.execute_batch("ROLLBACK;");
            Err(Self::ended_early())
        } else {
            self.committing.store(true, Ordering::SeqCst);
            let committed = self.conn.execute_batch("COMMIT;");
            if committed.is_err() {
                let _ = self.conn.execute_batch("ROLLBACK;");
            }
            committed
        };
        self.conn.commit_hook(None::<fn() -> bool>);
        result
    }

    fn ended_early() -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ABORT),
            Some(
                "SQLite ended the upgrade's one transaction before its last step (a step whose error is ignored met one that SQLite answers by undoing everything, such as a full disk); every commit after it was refused, so nothing of the upgrade was kept"
                    .to_string(),
            ),
        )
    }
}

impl Drop for UpgradeTransaction<'_> {
    /// A step failed: undo them all.
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let _ = self.conn.execute_batch("ROLLBACK;");
        self.conn.commit_hook(None::<fn() -> bool>);
        if self.refused.load(Ordering::SeqCst) {
            // The step named in the error is where a refused commit surfaced; this says why.
            tracing::error!("{}", Self::ended_early());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IF SQLITE ENDS THE UPGRADE'S TRANSACTION EARLY, NOTHING AFTER IT COMMITS. On a full disk
    /// or an I/O error SQLite undoes the whole transaction itself and the connection is back in
    /// autocommit; a step whose error `open` ignores goes on, and each step after it would commit
    /// on its own. Here the early end is a ROLLBACK by hand: the next write's commit is refused and
    /// undone, `commit` says the transaction ended early, nothing is kept, and once it has said so
    /// the hook is gone, so the writer commits as usual.
    ///
    /// Seen red 2026-11-03 with the hook not registered: "a step after the early end does not
    /// commit" (the INSERT committed on its own).
    #[test]
    fn a_transaction_ended_early_lets_nothing_after_it_commit() {
        let path = humanity_core::test_temp::db("upgrade_ended_early");
        let conn = Connection::open(&*path).unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
        let count = || conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0)).unwrap();

        let upgrade = UpgradeTransaction::begin(&conn).unwrap();
        conn.execute_batch("INSERT INTO t VALUES (1);").unwrap();
        conn.execute_batch("ROLLBACK;").unwrap();
        assert!(conn.execute_batch("INSERT INTO t VALUES (2);").is_err(), "a step after the early end does not commit");
        assert!(upgrade.commit().is_err(), "the commit says the transaction ended early");
        assert_eq!(count(), 0, "nothing of the upgrade was kept");

        conn.execute_batch("INSERT INTO t VALUES (3);").unwrap();
        assert_eq!(count(), 1, "the hook is gone once the upgrade is over");

        let upgrade = UpgradeTransaction::begin(&conn).unwrap();
        conn.execute_batch("INSERT INTO t VALUES (4); INSERT INTO t VALUES (5);").unwrap();
        upgrade.commit().expect("an upgrade that runs to its end commits");
        assert_eq!(count(), 3, "with every step");
    }
}
