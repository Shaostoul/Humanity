//! Resumable uploads' sessions (2026-10-11; docs/design/media-showcase.md, "Resumable uploads,
//! exact"). The routes are relay/resumable.rs.
//!
//! A big Media file is sent in pieces. Each session is one row of `upload_sessions` (created in
//! storage/mod.rs) and one file, `<resume folder>/<id>.part`, which grows by one piece at a time.
//! The row's `offset` is how many bytes of that file are safely on disk: a piece is written and
//! synced first, and only then is the offset stored. So after a crash the file can be LONGER than
//! the offset (the piece was written, the offset was not), never usefully shorter, and the
//! start-up reconciliation ([`Storage::reconcile_upload_sessions_in`]) cuts it back.
//!
//! A session belongs to a person (their key), not to a connection: upload tokens change on every
//! reconnect, and any current token of the same key carries on. It ends when the file is whole and
//! stored (the row goes), when it is cancelled, or after a day with no piece received (the
//! expiry pass, storage/expiry.rs). An account erase takes the person's sessions and their files
//! (storage/account.rs).

use super::Storage;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// How long a session may go without a piece before the expiry pass removes it, row and file.
pub const SESSION_IDLE_MS: i64 = 24 * 60 * 60 * 1000;

/// One upload in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct UploadSession {
    /// 32 random lowercase hex characters ([`valid_id`]); the pieces' file is `<id>.part`.
    pub id: String,
    /// Whose upload it is.
    pub public_key: String,
    /// The file's own name, as the app gave it (the stored name is made when it is whole).
    pub filename: String,
    pub content_type: String,
    /// What the file will be, in bytes.
    pub size: i64,
    /// How many bytes are safely on disk.
    pub offset: i64,
    /// Where the file goes when whole: 2, the Media shelf (storage/media.rs `MEDIA_SHELF`).
    pub shelf: i64,
    /// The BLAKE3 hash (lowercase hex) the app said the whole file has, or '' when it gave none.
    pub blake3: String,
    pub created_at: i64,
    /// When the last piece arrived (or the session started): the expiry pass reads it.
    pub touched_at: i64,
}

/// Whether `id` is a session id: exactly 32 lowercase hex characters. Anything else is never
/// joined to a folder, so no id can reach outside it.
pub fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The pieces' file of session `id` in `dir`, or None for something that is not a session id.
pub fn part_path(dir: &Path, id: &str) -> Option<PathBuf> {
    valid_id(id).then(|| dir.join(format!("{id}.part")))
}

/// What the start-up reconciliation did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Reconciled {
    /// Files in the folder with no session: deleted.
    pub files_removed: usize,
    /// Sessions with no file: deleted.
    pub rows_removed: usize,
    /// Files longer than their stored offset (a crash between the write and the offset): cut back.
    pub cut_back: usize,
    /// Files SHORTER than their stored offset (should never happen; a disk that lost a write):
    /// the offset is lowered to what is there, so the app sends the rest again.
    pub offset_lowered: usize,
}

const COLUMNS: &str = "id, public_key, filename, content_type, size, offset, shelf, blake3, created_at, touched_at";

fn from_row(r: &rusqlite::Row) -> Result<UploadSession, rusqlite::Error> {
    Ok(UploadSession {
        id: r.get(0)?,
        public_key: r.get(1)?,
        filename: r.get(2)?,
        content_type: r.get(3)?,
        size: r.get(4)?,
        offset: r.get(5)?,
        shelf: r.get(6)?,
        blake3: r.get(7)?,
        created_at: r.get(8)?,
        touched_at: r.get(9)?,
    })
}

fn sessions_where(conn: &Connection, filter: &str, binds: &[&dyn rusqlite::types::ToSql]) -> Result<Vec<UploadSession>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM upload_sessions {filter} ORDER BY created_at ASC, id ASC"))?;
    let rows = stmt.query_map(binds, from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

impl Storage {
    /// Record a new session (the route has checked everything and made its file).
    pub fn insert_upload_session(&self, s: &UploadSession) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                &format!("INSERT INTO upload_sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"),
                params![s.id, s.public_key, s.filename, s.content_type, s.size, s.offset, s.shelf, s.blake3, s.created_at, s.touched_at],
            )
            .map(|_| ())
        })
    }

    /// The session `id`, if there is one.
    pub fn upload_session(&self, id: &str) -> Result<Option<UploadSession>, rusqlite::Error> {
        self.with_conn(|conn| conn.query_row(&format!("SELECT {COLUMNS} FROM upload_sessions WHERE id = ?1"), params![id], from_row).optional())
    }

    /// Every open session of `key`, oldest first.
    pub fn upload_sessions_of(&self, key: &str) -> Result<Vec<UploadSession>, rusqlite::Error> {
        self.with_conn(|conn| sessions_where(conn, "WHERE public_key = ?1", &[&key]))
    }

    /// Every open session, everyone's, oldest first.
    pub fn all_upload_sessions(&self) -> Result<Vec<UploadSession>, rusqlite::Error> {
        self.with_conn(|conn| sessions_where(conn, "", &[]))
    }

    /// A piece of session `id` arrived (or is arriving) at `now_ms`: the expiry pass leaves it
    /// another day. Whether the session is still there.
    pub fn touch_upload_session(&self, id: &str, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("UPDATE upload_sessions SET touched_at = ?2 WHERE id = ?1", params![id, now_ms]).map(|n| n > 0))
    }

    /// `offset` bytes of session `id` are safely on disk. Whether the session is still there (an
    /// expiry or a cancel may have taken it while the piece was written).
    pub fn set_upload_session_offset(&self, id: &str, offset: i64, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute("UPDATE upload_sessions SET offset = ?2, touched_at = ?3 WHERE id = ?1", params![id, offset, now_ms]).map(|n| n > 0)
        })
    }

    /// Remove session `id`'s row (its file is the caller's to delete). Whether there was one.
    pub fn delete_upload_session(&self, id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("DELETE FROM upload_sessions WHERE id = ?1", params![id]).map(|n| n > 0))
    }

    /// The expiry pass's share (storage/expiry.rs): every session with no piece for
    /// [`SESSION_IDLE_MS`] before `now_ms` goes, row and file (in `dir`). How many went.
    pub fn expire_upload_sessions_in(&self, dir: &Path, now_ms: i64) -> Result<usize, rusqlite::Error> {
        let ids: Vec<String> = self.with_conn(|conn| {
            let ids: Vec<String> = {
                let mut stmt = conn.prepare("SELECT id FROM upload_sessions WHERE touched_at < ?1")?;
                let ids = stmt.query_map(params![now_ms - SESSION_IDLE_MS], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
                ids
            };
            for id in &ids {
                conn.execute("DELETE FROM upload_sessions WHERE id = ?1", params![id])?;
            }
            Ok::<_, rusqlite::Error>(ids)
        })?;
        for id in &ids {
            if let Some(path) = part_path(dir, id) {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(ids.len())
    }

    /// Delete the pieces' files of every session of `key` from `dir` (the account erase, before
    /// it deletes the rows: storage/account.rs). How many files were deleted.
    pub fn delete_upload_session_files_in(&self, key: &str, dir: &Path) -> usize {
        let ids: Vec<String> = self.upload_sessions_of(key).map(|v| v.into_iter().map(|s| s.id).collect()).unwrap_or_default();
        ids.iter().filter_map(|id| part_path(dir, id)).filter(|p| std::fs::remove_file(p).is_ok()).count()
    }

    /// At relay start, when no piece can be arriving: make the folder and the rows agree. A file
    /// with no session is deleted (so is anything else in the folder: it is the relay's own); a
    /// session with no file is deleted; a file longer than its stored offset (the relay stopped
    /// between writing a piece and storing the offset) is cut back to the offset, so the app sends
    /// that piece again; a file shorter than its offset lowers the offset to what is there.
    pub fn reconcile_upload_sessions_in(&self, dir: &Path) -> Result<Reconciled, rusqlite::Error> {
        let mut done = Reconciled::default();
        let sessions = self.all_upload_sessions()?;
        // The files first: anything that is not a live session's file goes.
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let ours = name.strip_suffix(".part").is_some_and(|id| sessions.iter().any(|s| s.id == id));
                if !ours && std::fs::remove_file(&path).is_ok() {
                    done.files_removed += 1;
                }
            }
        }
        for s in &sessions {
            let Some(path) = part_path(dir, &s.id) else {
                // A row whose id could never name a file (written by hand): it goes.
                self.delete_upload_session(&s.id)?;
                done.rows_removed += 1;
                continue;
            };
            let len = match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => m.len() as i64,
                _ => {
                    self.delete_upload_session(&s.id)?;
                    done.rows_removed += 1;
                    continue;
                }
            };
            if len > s.offset {
                let cut = std::fs::OpenOptions::new().write(true).open(&path).and_then(|f| {
                    f.set_len(s.offset as u64)?;
                    f.sync_all()
                });
                match cut {
                    Ok(()) => done.cut_back += 1,
                    Err(e) => {
                        // A file that cannot be cut back cannot be trusted: the session goes.
                        tracing::error!("Resumable upload {}: could not cut its file back ({e}); removing it", s.id);
                        let _ = std::fs::remove_file(&path);
                        self.delete_upload_session(&s.id)?;
                        done.rows_removed += 1;
                    }
                }
            } else if len < s.offset {
                self.with_conn(|conn| conn.execute("UPDATE upload_sessions SET offset = ?2 WHERE id = ?1", params![s.id, len]))?;
                done.offset_lowered += 1;
            }
        }
        Ok(done)
    }
}
