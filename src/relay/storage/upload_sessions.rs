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
    /// The person chose to keep the file's hidden information (`keep_metadata: 1` at the start,
    /// B4, 2026-10-11): the finish skips the picture and video strip, never the picture's own
    /// check that it is what it says.
    pub keep_metadata: bool,
    /// The BLAKE3 hash given at the start matched the whole file, checked once before the strip
    /// changed it (R1, 2026-10-11): a retried finish never hashes again.
    pub hash_checked: bool,
    /// The finished upload's answer (the single request's JSON), '' until the file is stored. A
    /// finished session stays until the idle expiry so an app that lost its connection during the
    /// finish still gets the answer (R2, 2026-10-11); its pieces' file is gone by then.
    pub done_answer: String,
}

impl UploadSession {
    /// Whether the file is stored and this row only keeps its answer.
    pub fn finished(&self) -> bool {
        !self.done_answer.is_empty()
    }
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

/// The hidden name session `id`'s pieces file has while it moves into place (R3-5, 2026-10-11):
/// storage/media.rs `STAGED_PREFIX`, `resume-` and the id. The finish moves the pieces file from
/// the resume folder into the folder it is stored in under this name (relay/media.rs
/// `store_upload`), then waits for the room lock before the last rename. It is the ONLY copy of
/// what was sent, so the name says whose it is: a relay that stops in between, or a move back
/// that fails, leaves a file that is put back with its session ([`restore_staged_part`]) instead
/// of being cleared at start as a half-copied file.
pub fn staged_part_name(id: &str) -> String {
    format!("{}resume-{id}", super::media::STAGED_PREFIX)
}

/// The session id a [`staged_part_name`] carries, or None for any other name.
pub fn staged_part_id(name: &str) -> Option<&str> {
    name.strip_prefix(super::media::STAGED_PREFIX)?.strip_prefix("resume-").filter(|id| valid_id(id))
}

/// Put session `id`'s pieces file back into the resume folder from where a finish staged it (the
/// Media waiting folder, or the public one), when it is not in the resume folder already (R3-5).
/// Whether the pieces file is in the resume folder now. Never deletes the staged file unless it
/// is safely back.
pub fn restore_staged_part(folders: super::media::Folders, id: &str) -> bool {
    let Some(part) = part_path(folders.resume, id) else { return false };
    if part.is_file() {
        return true;
    }
    let name = staged_part_name(id);
    for dir in [folders.waiting, folders.uploads] {
        let staged = dir.join(&name);
        if !staged.is_file() {
            continue;
        }
        let back = std::fs::create_dir_all(folders.resume).and_then(|_| crate::relay::upload_limits::move_file(&staged, &part));
        return match back {
            Ok(()) => {
                tracing::info!("Resumable upload {id}: its pieces' file was put back with its session");
                true
            }
            Err(e) => {
                tracing::error!("Resumable upload {id}: its pieces' file could not be put back yet ({e}); it is kept where it is");
                false
            }
        };
    }
    false
}

/// Remove session `id`'s pieces file wherever it is (R4-5, 2026-10-11): in the resume folder, or
/// staged under its session's name ([`staged_part_name`]) in the Media waiting folder or the public
/// one, where a finish that could not move it back keeps it (R3-5). The cancel, the expiry pass and
/// the account erase all remove a session this way, so a staged file never outlives its session
/// until the next restart. Whether any file was removed.
pub fn remove_session_files(folders: super::media::Folders, id: &str) -> bool {
    let Some(part) = part_path(folders.resume, id) else { return false };
    let staged = staged_part_name(id);
    let mut removed = std::fs::remove_file(part).is_ok();
    for dir in [folders.waiting, folders.uploads] {
        removed |= std::fs::remove_file(dir.join(&staged)).is_ok();
    }
    removed
}

/// What the start-up settling of the upload folders did ([`Storage::settle_uploads_at_start`]).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    /// Staged pieces files put back with their sessions (R3-5).
    pub restored: usize,
    /// Copies cut off part way into the public or the Media waiting folder, removed (R5).
    pub staged_cleared: usize,
    pub reconciled: Reconciled,
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
    /// Pieces counted against the day's allowance whose offset was never stored: their bytes
    /// given back to the day they were counted in (R4-3).
    pub given_back: usize,
}

const COLUMNS: &str = "id, public_key, filename, content_type, size, offset, shelf, blake3, created_at, touched_at, keep_metadata, hash_checked, done_answer";

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
        keep_metadata: r.get::<_, i64>(10)? != 0,
        hash_checked: r.get::<_, i64>(11)? != 0,
        done_answer: r.get(12)?,
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
                &format!("INSERT INTO upload_sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"),
                params![
                    s.id, s.public_key, s.filename, s.content_type, s.size, s.offset, s.shelf, s.blake3, s.created_at, s.touched_at,
                    s.keep_metadata as i64, s.hash_checked as i64, s.done_answer
                ],
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

    /// Every open session, everyone's, oldest first (finished ones included: see
    /// [`UploadSession::finished`]).
    pub fn all_upload_sessions(&self) -> Result<Vec<UploadSession>, rusqlite::Error> {
        self.with_conn(|conn| sessions_where(conn, "", &[]))
    }

    /// The hash given at the start of session `id` matched the whole file (R1): never hashed
    /// again. Whether the session is still there.
    pub fn mark_upload_hash_checked(&self, id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("UPDATE upload_sessions SET hash_checked = 1 WHERE id = ?1", params![id]).map(|n| n > 0))
    }

    /// Session `id`'s file is stored and `answer` is what the finish answered (R2): the row stays,
    /// with that answer, until the idle expiry. Whether the session was still there.
    pub fn finish_upload_session(&self, id: &str, answer: &str, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE upload_sessions SET done_answer = ?2, offset = size, touched_at = ?3 WHERE id = ?1",
                params![id, answer, now_ms],
            )
            .map(|n| n > 0)
        })
    }

    /// A piece of session `id` arrived (or is arriving) at `now_ms`: the expiry pass leaves it
    /// another day. Whether the session is still there.
    pub fn touch_upload_session(&self, id: &str, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("UPDATE upload_sessions SET touched_at = ?2 WHERE id = ?1", params![id, now_ms]).map(|n| n > 0))
    }

    /// `offset` bytes of session `id` are safely on disk. Whether the session is still there (an
    /// expiry or a cancel may have taken it while the piece was written). The piece counted on the
    /// session ([`Storage::count_upload_piece`]) is stored with it, so the session holds no count
    /// after (R4-3).
    pub fn set_upload_session_offset(&self, id: &str, offset: i64, now_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE upload_sessions SET offset = ?2, touched_at = ?3, counted_bytes = 0, counted_at = 0 WHERE id = ?1",
                params![id, offset, now_ms],
            )
            .map(|n| n > 0)
        })
    }

    /// A piece of session `id`, `bytes` long, counted against `key`'s day's allowance at `now_ms`
    /// (R3-1), and the count written on the session until the piece's offset is stored (R4-3), in
    /// one transaction: a relay that stops between counting a piece and storing its offset gives
    /// the bytes back at its next start ([`Storage::give_back_unstored_pieces`]), to the day they
    /// were counted in. Whether the session is still there (nothing is counted when it is not).
    pub fn count_upload_piece(&self, id: &str, key: &str, now_ms: i64, bytes: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let there = tx.execute("UPDATE upload_sessions SET counted_bytes = ?2, counted_at = ?3 WHERE id = ?1", params![id, bytes, now_ms])? > 0;
            if there {
                super::upload_counts::add_count_on(&tx, key, super::upload_counts::Span::Day, now_ms, bytes)?;
            }
            tx.commit()?;
            Ok(there)
        })
    }

    /// The piece counted on session `id` at `counted_at` ([`Storage::count_upload_piece`]) was not
    /// stored after all (its write failed, or the session went meanwhile): its `bytes` go back off
    /// `key`'s day they were counted in, and the session holds no count (R4-3).
    pub fn take_back_upload_piece(&self, id: &str, key: &str, counted_at: i64, bytes: i64) -> Result<(), rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            super::upload_counts::take_back_on(&tx, key, super::upload_counts::Span::Day, counted_at, bytes)?;
            tx.execute("UPDATE upload_sessions SET counted_bytes = 0, counted_at = 0 WHERE id = ?1", params![id])?;
            tx.commit()
        })
    }

    /// At relay start, when no piece can be arriving (R4-3, 2026-10-11): every piece counted
    /// against the day's allowance whose offset was never stored (the relay stopped between
    /// counting it and storing its offset) gives its bytes back to the day it was counted in, and
    /// its session holds no count after. The start-up check cuts such a piece off the file with
    /// anything else past the offset, so the app sends it again and it is counted then: without
    /// this it was counted twice. How many sessions gave bytes back.
    pub fn give_back_unstored_pieces(&self) -> Result<usize, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let counted: Vec<(String, String, i64, i64)> = {
                let mut stmt = tx.prepare("SELECT id, public_key, counted_bytes, counted_at FROM upload_sessions WHERE counted_bytes > 0")?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<Vec<_>, _>>()?;
                rows
            };
            for (id, key, bytes, at) in &counted {
                super::upload_counts::take_back_on(&tx, key, super::upload_counts::Span::Day, *at, *bytes)?;
                tx.execute("UPDATE upload_sessions SET counted_bytes = 0, counted_at = 0 WHERE id = ?1", params![id])?;
            }
            tx.commit()?;
            Ok(counted.len())
        })
    }

    /// Remove session `id`'s row (its file is the caller's to delete). Whether there was one.
    pub fn delete_upload_session(&self, id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("DELETE FROM upload_sessions WHERE id = ?1", params![id]).map(|n| n > 0))
    }

    /// The expiry pass's share (storage/expiry.rs): every session with no piece for
    /// [`SESSION_IDLE_MS`] before `now_ms` goes, row and pieces file, wherever that file is: the
    /// resume folder, or staged by a finish that could not move it back (R4-5,
    /// [`remove_session_files`]). How many went.
    pub fn expire_upload_sessions_in(&self, folders: super::media::Folders, now_ms: i64) -> Result<usize, rusqlite::Error> {
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
            remove_session_files(folders, id);
        }
        Ok(ids.len())
    }

    /// Delete the pieces' files of every session of `key` (the account erase, before it deletes
    /// the rows: storage/account.rs), wherever each is: the resume folder, or staged by a finish
    /// (R4-5, [`remove_session_files`]). How many sessions had a file deleted.
    pub fn delete_upload_session_files_in(&self, key: &str, folders: super::media::Folders) -> usize {
        let ids: Vec<String> = self.upload_sessions_of(key).map(|v| v.into_iter().map(|s| s.id).collect()).unwrap_or_default();
        ids.iter().filter(|id| remove_session_files(folders, id)).count()
    }

    /// At relay start, when no upload can be arriving (relay/mod.rs `run_relay`), in the order
    /// that loses nothing (R3-5, 2026-10-11): a resumable upload's pieces file that a finish had
    /// staged goes back with its session ([`Storage::restore_staged_upload_parts`]); then copies
    /// cut off part way into the public or the Media waiting folder are removed (storage/media.rs
    /// `clear_staged_files`, R5), never a staged pieces file; then the resume folder and the
    /// sessions are made to agree ([`Storage::reconcile_upload_sessions_in`]).
    pub fn settle_uploads_at_start(&self, folders: super::media::Folders) -> Result<Settled, rusqlite::Error> {
        let restored = self.restore_staged_upload_parts(folders);
        let staged_cleared = super::media::clear_staged_files(folders.uploads) + super::media::clear_staged_files(folders.waiting);
        let restored = restored?;
        // A staged pieces file still there could not be moved back just now: its session is kept
        // (with no pieces file in the resume folder) so the next finish tries again.
        let mut still_staged = std::collections::HashSet::new();
        for dir in [folders.waiting, folders.uploads] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                let names = entries.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned());
                still_staged.extend(names.filter_map(|n| staged_part_id(&n).map(str::to_string)));
            }
        }
        let reconciled = self.reconcile_upload_sessions_keeping(folders.resume, &still_staged)?;
        Ok(Settled { restored, staged_cleared, reconciled })
    }

    /// At relay start, BEFORE the folder is reconciled ([`Storage::settle_uploads_at_start`]): every pieces
    /// file a finish had staged ([`staged_part_name`]) and the relay stopped with is settled (R3-5,
    /// 2026-10-11). One whose session is still in progress and has no pieces file goes back to the
    /// resume folder, so the reconciliation keeps the session. One whose session's pieces file is
    /// there was a copy, and one whose session is gone or finished is left over: both are deleted.
    /// (storage/media.rs `clear_staged_files` never touches these names, so a database that cannot
    /// be read here deletes nothing.) How many were put back.
    pub fn restore_staged_upload_parts(&self, folders: super::media::Folders) -> Result<usize, rusqlite::Error> {
        let sessions = self.all_upload_sessions()?;
        let live: std::collections::HashSet<&str> = sessions.iter().filter(|s| !s.finished()).map(|s| s.id.as_str()).collect();
        let mut restored = 0;
        for dir in [folders.waiting, folders.uploads] {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(id) = staged_part_id(&name) else { continue };
                if live.contains(id) && part_path(folders.resume, id).is_some_and(|p| !p.is_file()) {
                    restored += usize::from(restore_staged_part(folders, id));
                } else {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(restored)
    }

    /// At relay start, when no piece can be arriving: make the folder and the rows agree. A file
    /// with no session is deleted (so is anything else in the folder: it is the relay's own); a
    /// session with no file is deleted; a file longer than its stored offset (the relay stopped
    /// between writing a piece and storing the offset) is cut back to the offset, so the app sends
    /// that piece again; a file shorter than its offset lowers the offset to what is there. Before
    /// any of that, a piece counted and never stored gives its bytes back to the day it was counted
    /// in (R4-3, [`Storage::give_back_unstored_pieces`]), so the piece sent again counts once.
    pub fn reconcile_upload_sessions_in(&self, dir: &Path) -> Result<Reconciled, rusqlite::Error> {
        self.reconcile_upload_sessions_keeping(dir, &std::collections::HashSet::new())
    }

    /// [`Storage::reconcile_upload_sessions_in`], keeping the sessions in `staged` even with no
    /// file in `dir`: their pieces file is staged elsewhere and could not be moved back yet (R3-5).
    pub fn reconcile_upload_sessions_keeping(&self, dir: &Path, staged: &std::collections::HashSet<String>) -> Result<Reconciled, rusqlite::Error> {
        let mut done = Reconciled::default();
        // First, before any file is cut back or any row goes: a piece counted and never stored
        // gives its bytes back (R4-3).
        done.given_back = self.give_back_unstored_pieces()?;
        let sessions = self.all_upload_sessions()?;
        // The files first: anything that is not a live session's file goes.
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let ours = name.strip_suffix(".part").is_some_and(|id| sessions.iter().any(|s| s.id == id && !s.finished()));
                if !ours && std::fs::remove_file(&path).is_ok() {
                    done.files_removed += 1;
                }
            }
        }
        for s in &sessions {
            if s.finished() {
                // A finished session has no pieces' file by design: its row keeps the answer for
                // the app until the idle expiry (R2).
                continue;
            }
            let Some(path) = part_path(dir, &s.id) else {
                // A row whose id could never name a file (written by hand): it goes.
                self.delete_upload_session(&s.id)?;
                done.rows_removed += 1;
                continue;
            };
            let len = match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => m.len() as i64,
                _ if staged.contains(&s.id) => continue,
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
