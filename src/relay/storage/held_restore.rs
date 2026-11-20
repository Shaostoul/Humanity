//! Restore, the storage half (2026-11-20, docs/design/hide-and-keep.md rung 5). A child of
//! storage/held.rs (#[path]); the handler is handlers/held_restore.rs.
//!
//! The owner's device opens a held package and uploads each file again; the relay checks every
//! byte against the RESTORE PLAN it sealed when it held the post (crates/humanity-protocol/src/
//! held_seal.rs, "The restore plan"), which only the key inside the owner's part 0 opens. The plan,
//! never the device, says what the post's words were, what each file is called, which folder it
//! was in and who uploaded it, so a restore can put back exactly what was held and nothing else.
//!
//! A part arrives in pieces (nginx's 1 MB limit on `/api/`), appended to `.restore-<part>.incoming`
//! in the package's own folder, which nothing serves; once it is as long as the plan says it is
//! checked against the plan's BLAKE3 hash and renamed `restore-<part>`, or deleted if it does not
//! match. When every file is there, [`Storage::held_restore_in`] moves each into its folder under
//! its stored name, puts back the files' upload rows and the post's row in one transaction, and
//! deletes the held copy and its row.

use std::path::{Path, PathBuf};

use humanity_protocol::held_seal::{self, Manifest};
use rusqlite::params;

use super::{held_dir_beside, HeldRow};
use crate::relay::storage::files::{bare_name, poster_name, Folders, Spot};
use crate::relay::storage::Storage;

/// The most a restore's piece may carry: under nginx's 1 MB default for `/api/`.
pub const RESTORE_PIECE_MAX: usize = 768 * 1024;

/// The name a part waits under while its pieces arrive.
pub fn incoming_name(part: usize) -> String {
    format!(".restore-{part}.incoming")
}

/// The name a part is kept under once it matched the plan.
pub fn restored_name(part: usize) -> String {
    format!("restore-{part}")
}

/// The folder of held item `row`.
pub fn package_dir(row: &HeldRow, folders: Folders) -> PathBuf {
    held_dir_beside(folders).join(&row.package)
}

/// Open item `row`'s restore plan with the key the owner's device sent (hex): the plan, or why
/// not in words.
pub fn open_plan(row: &HeldRow, folders: Folders, key_hex: &str) -> Result<Manifest, String> {
    let key: [u8; 32] = hex::decode(key_hex.trim()).ok().and_then(|k| k.try_into().ok()).ok_or_else(|| "The restore key is missing or not 32 bytes of hex.".to_string())?;
    let sealed = std::fs::read(package_dir(row, folders).join(held_seal::RESTORE_PLAN_FILE)).map_err(|_| "This held item has no restore plan.".to_string())?;
    let plain = held_seal::open_restore_plan(&key, &row.package, &sealed).map_err(|_| "The restore key does not open this held item's plan.".to_string())?;
    let plan: Manifest = serde_json::from_slice(&plain).map_err(|_| "This held item's plan cannot be read.".to_string())?;
    if plan.package != row.package {
        return Err("This held item's plan is another package's.".into());
    }
    Ok(plan)
}

/// What part `part` must be, by the plan: its size and BLAKE3 hash (hex); None when the plan has
/// no file or poster picture in that part.
pub fn expected(plan: &Manifest, part: usize) -> Option<(u64, String)> {
    plan.files.iter().find_map(|f| {
        if f.part == part {
            Some((f.bytes, f.hash.clone()))
        } else if f.poster_part == Some(part) {
            Some((f.poster_bytes, f.poster_hash.clone()))
        } else {
            None
        }
    })
}

/// What one piece did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Taken; the part now holds this many bytes.
    More(u64),
    /// The part is whole and matched the plan.
    Done,
}

/// Why a piece was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PieceRefused {
    /// The piece does not start where the part ends (the bytes the server has).
    WrongOffset(u64),
    TooLarge,
    /// The whole part did not match the plan: it was deleted, to be sent again.
    Mismatch,
    Disk(String),
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex().to_string())
}

/// Take one piece of part `part` of `row` (already checked against the plan: it is `size` bytes
/// with BLAKE3 `hash`), starting at byte `offset`.
pub fn take_piece(row: &HeldRow, folders: Folders, part: usize, size: u64, hash: &str, offset: u64, piece: &[u8]) -> Result<Piece, PieceRefused> {
    use std::io::Write;
    let dir = package_dir(row, folders);
    if dir.join(restored_name(part)).is_file() {
        return Ok(Piece::Done);
    }
    if piece.len() > RESTORE_PIECE_MAX {
        return Err(PieceRefused::TooLarge);
    }
    let incoming = dir.join(incoming_name(part));
    let have = std::fs::metadata(&incoming).map(|m| m.len()).unwrap_or(0);
    if offset != have {
        return Err(PieceRefused::WrongOffset(have));
    }
    let end = have + piece.len() as u64;
    if end > size {
        return Err(PieceRefused::TooLarge);
    }
    let disk = |e: std::io::Error| PieceRefused::Disk(e.to_string());
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&incoming).map_err(disk)?;
    f.write_all(piece).map_err(disk)?;
    f.sync_all().map_err(disk)?;
    drop(f);
    if end < size {
        return Ok(Piece::More(end));
    }
    if hash_file(&incoming).map_err(disk)? != hash {
        let _ = std::fs::remove_file(&incoming);
        return Err(PieceRefused::Mismatch);
    }
    std::fs::rename(&incoming, dir.join(restored_name(part))).map_err(disk)?;
    Ok(Piece::Done)
}

/// What a restore put back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub author: String,
    pub from_name: Option<String>,
    pub words: String,
    pub stamp: u64,
    /// The room as the post's row keeps it ('' read as the main room).
    pub channel: String,
    pub files: usize,
    /// The post's new row, for the clients' copies.
    pub message_id: i64,
}

fn spot_of(folder: &str) -> Option<Spot> {
    match folder {
        "public" => Some(Spot::Public),
        "members" => Some(Spot::Members),
        "waiting" => Some(Spot::Waiting),
        _ => None,
    }
}

impl Storage {
    /// Put held item `row` back as `plan` says, at `now_ms`: every file (and poster picture) that
    /// matched into its folder under its stored name (a file of that name already there is left as
    /// it is), the files' upload rows and the post's row in one transaction, then the held copy and
    /// its row deleted. Refused, with nothing changed, while any file has not arrived whole.
    pub fn held_restore_in(&self, row: &HeldRow, plan: &Manifest, folders: Folders, now_ms: i64) -> Result<Restored, String> {
        let dir = package_dir(row, folders);
        // Every place a file goes, checked before anything moves.
        let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
        for f in &plan.files {
            if !bare_name(&f.name) || f.name.starts_with('.') {
                return Err(format!("The plan names a file this server would not store ({}).", f.name));
            }
            let spot = spot_of(&f.folder).ok_or_else(|| format!("The plan names a folder this server does not have ({}).", f.folder))?;
            let mut each = vec![(f.part, folders.dir(spot).join(&f.name))];
            if let Some(p) = f.poster_part {
                each.push((p, folders.dir(spot).join(poster_name(&f.name))));
            }
            for (part, dest) in each {
                let staged = dir.join(restored_name(part));
                if !staged.is_file() && !dest.is_file() {
                    return Err(format!("Part {part} has not arrived whole yet, so nothing was put back."));
                }
                moves.push((staged, dest));
            }
        }
        for (staged, dest) in &moves {
            if staged.is_file() && !dest.exists() {
                std::fs::rename(staged, dest).map_err(|e| format!("A file could not be put back in its folder ({e}); the files moved so far stay, and Restore can be tried again."))?;
            }
        }
        let from_name = self.name_for_key(&row.author_key).ok().flatten().filter(|n| !n.is_empty());
        let raw = serde_json::json!({
            "type": "chat", "from": row.author_key, "from_name": from_name, "content": plan.words,
            "timestamp": row.stamp, "channel": if row.channel.is_empty() { humanity_protocol::chat_access::MAIN_CHANNEL } else { row.channel.as_str() },
        })
        .to_string();
        let message_id = self
            .with_conn_mut(|conn| -> Result<i64, rusqlite::Error> {
                let tx = conn.transaction()?;
                for f in plan.files.iter().filter(|f| !f.uploader.is_empty()) {
                    let there: i64 = tx.query_row("SELECT COUNT(*) FROM user_uploads WHERE filename = ?1", params![f.name], |r| r.get(0))?;
                    if there == 0 {
                        tx.execute(
                            "INSERT INTO user_uploads (public_key, filename, uploaded_at, shared, original_name, size_bytes, poster_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            params![f.uploader, f.name, now_ms, f.shelf, f.original_name, f.bytes as i64, f.poster_bytes as i64],
                        )?;
                    }
                }
                let existing: Option<i64> = tx
                    .query_row("SELECT id FROM messages WHERE from_key = ?1 AND timestamp = ?2 LIMIT 1", params![row.author_key, row.stamp], |r| r.get(0))
                    .ok();
                let id = match existing {
                    Some(id) => id,
                    None => {
                        tx.execute(
                            "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id) VALUES ('chat', ?1, ?2, ?3, ?4, NULL, ?5, ?6)",
                            params![row.author_key, from_name, plan.words, row.stamp, raw, row.channel],
                        )?;
                        tx.last_insert_rowid()
                    }
                };
                tx.commit()?;
                Ok(id)
            })
            .map_err(|e| format!("The files are back, but the post could not be put back ({e}); Restore can be tried again."))?;
        if let Err(e) = self.held_destroy_in(row.id, folders) {
            tracing::error!("Held items: item #{} was restored but its held copy could not be deleted: {e}", row.id);
        }
        Ok(Restored {
            author: row.author_key.clone(),
            from_name,
            words: plan.words.clone(),
            stamp: row.stamp as u64,
            channel: row.channel.clone(),
            files: plan.files.len(),
            message_id,
        })
    }
}
