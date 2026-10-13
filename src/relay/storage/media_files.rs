//! The Media page's storage, round 5 (2026-10-12; docs/design/media-round-5-plan.md 3.1, with
//! the orchestrator's decisions in its section 9). A child of storage/media.rs (`#[path]`), so it
//! reads that file's private helpers; the routes are relay/media.rs and relay/media_edit.rs.
//!
//! - [`Featured`]: `media_items.featured` holds 0 (none), 1 (the main shelf) or 2 (its category's
//!   shelf only). Rows written before round 5 are 0 or 1 and mean what they did.
//! - [`Sort`] and its cursor ([`cursor_text`], [`parse_cursor`]): newest, most picked or most
//!   downloaded first, paged by (value, item number) so ties are never skipped or repeated.
//! - [`ItemFile`], [`fill_files`]: an item's added files and its cover, rows of
//!   `media_item_files`. The item's own file stays `media_items.upload_filename` (file 0), so
//!   everything that names an item by its file (W5, tickets, Keep, Remove) works as before.
//! - [`Storage::media_item_holding`]: the item a file is on, as its own file or an added one.
//! - [`Storage::attach_media_file`], [`Storage::detach_media_file`]: adding a file or the cover
//!   (the uploader's own upload, waiting on the Media shelf) and taking one off.
//! - [`Storage::edit_media`]: the words, category, labels, licence and permission. A licence is
//!   the rights holder's grant, so only the uploader changes it or the game tick (decision 1);
//!   each choice is kept with the signature over it (`media_licence_log`, decision 8).
//! - [`Storage::add_media_view`], [`Storage::add_media_download`]: the counters (who counted is
//!   decided in memory, relay/media_counts.rs; only the numbers are written).

use super::{clipped, item_by_id, kind_of, now_ms, refused, MediaError, MediaItem, Refusal, MEDIA_DESCRIPTION_MAX, MEDIA_SHELF, MEDIA_TITLE_MAX};
use crate::relay::storage::Storage;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

/// Where an item is featured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Featured {
    /// On no shelf.
    #[default]
    None,
    /// On the main shelf, and on its category's shelf too.
    Main,
    /// On its category's shelf only.
    Category,
}

impl Featured {
    /// What the column holds: 0, 1 or 2. Anything else reads as none.
    pub fn from_stored(v: i64) -> Self {
        match v {
            1 => Featured::Main,
            2 => Featured::Category,
            _ => Featured::None,
        }
    }

    pub fn stored(self) -> i64 {
        match self {
            Featured::None => 0,
            Featured::Main => 1,
            Featured::Category => 2,
        }
    }

    /// The word the wire carries: `main`, `category` or `none`.
    pub fn word(self) -> &'static str {
        match self {
            Featured::None => "none",
            Featured::Main => "main",
            Featured::Category => "category",
        }
    }

    /// The word read back; None for anything else.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "none" => Some(Featured::None),
            "main" => Some(Featured::Main),
            "category" => Some(Featured::Category),
            _ => None,
        }
    }

    /// On a shelf at all.
    pub fn is_on(self) -> bool {
        self != Featured::None
    }
}

/// The list's order, each then by item number, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Newest first (the default).
    #[default]
    New,
    /// Most picked first.
    Picks,
    /// Most downloaded first.
    Downloads,
}

impl Sort {
    /// `new`, `picks` or `downloads`; None for anything else.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "new" => Some(Sort::New),
            "picks" => Some(Sort::Picks),
            "downloads" => Some(Sort::Downloads),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Sort::New => "new",
            Sort::Picks => "picks",
            Sort::Downloads => "downloads",
        }
    }

    /// The value the list orders and pages by, over `media_items m`.
    pub(super) fn value_sql(self) -> &'static str {
        match self {
            Sort::New => "m.id",
            Sort::Picks => "(SELECT COUNT(*) FROM media_picks p WHERE p.item_id = m.id)",
            Sort::Downloads => "m.downloads",
        }
    }

    /// The same value read from an item the list gave.
    pub fn value_of(self, item: &MediaItem) -> i64 {
        match self {
            Sort::New => item.id,
            Sort::Picks => item.picks,
            Sort::Downloads => item.downloads,
        }
    }
}

/// The cursor the list hands out for the page after an item with this value and number:
/// `"<value>.<id>"`. The apps treat it as opaque.
pub fn cursor_text(value: i64, id: i64) -> String {
    format!("{value}.{id}")
}

/// A cursor read back: two runs of digits joined by one dot, each a number this server could
/// have given. None for anything else.
pub fn parse_cursor(text: &str) -> Option<(i64, i64)> {
    let (value, id) = text.split_once('.')?;
    let number = |s: &str| -> Option<i64> {
        if s.is_empty() || s.len() > 18 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    Some((number(value)?, number(id)?))
}

/// The most files one item holds: its own file and up to 19 added ones (decision 9). The cover
/// is not counted.
pub const ITEM_FILES_MAX: i64 = 20;
/// A `media_item_files.role`: an added file, or the item's cover picture.
pub const ROLE_FILE: &str = "file";
pub const ROLE_COVER: &str = "cover";

/// What publishing, or adding, an upload that is already on an item is told (409).
pub const ALREADY_ON_AN_ITEM: &str = "That upload is already part of a Media item.";
/// An action naming an item's file by a name that is not that file's (409): the W5 words.
pub const OUT_OF_DATE: &str = "That is not the Media item you were looking at: the page is out of date. Load it again.";

/// One of an item's added files, or its cover, as the item carries it. Its size and the name it
/// had on its uploader's device are its `user_uploads` row's.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ItemFile {
    /// Its number on the item (`media_item_files.id`); the item's own file is 0.
    pub file: i64,
    pub filename: String,
    pub original_name: String,
    /// `picture`, `video`, `model` or `file`, from its stored name ([`kind_of`]).
    pub kind: String,
    pub size_bytes: i64,
}

/// Every added file and cover of the items `ids`, in the order of `position`: by item, its files
/// and its cover.
pub(super) fn files_of(conn: &Connection, ids: &[i64]) -> Result<HashMap<i64, (Vec<ItemFile>, Option<ItemFile>)>, rusqlite::Error> {
    let mut out: HashMap<i64, (Vec<ItemFile>, Option<ItemFile>)> = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let marks = vec!["?"; ids.len()].join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT f.item_id, f.id, f.upload_filename, f.role,
                COALESCE((SELECT u.original_name FROM user_uploads u WHERE u.filename = f.upload_filename ORDER BY u.id LIMIT 1), ''),
                COALESCE((SELECT u.size_bytes FROM user_uploads u WHERE u.filename = f.upload_filename ORDER BY u.id LIMIT 1), 0)
         FROM media_item_files f WHERE f.item_id IN ({marks}) ORDER BY f.item_id, f.position, f.id"
    ))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| {
        let filename: String = r.get(2)?;
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(3)?,
            ItemFile { file: r.get(1)?, kind: kind_of(&filename).to_string(), filename, original_name: r.get(4)?, size_bytes: r.get(5)? },
        ))
    })?;
    for row in rows {
        let (item, role, file) = row?;
        let entry = out.entry(item).or_default();
        if role == ROLE_COVER {
            entry.1 = Some(file);
        } else {
            entry.0.push(file);
        }
    }
    Ok(out)
}

/// Fill `items`' added files and covers from `media_item_files` (one query for them all).
pub(super) fn fill_files(conn: &Connection, items: &mut [MediaItem]) -> Result<(), rusqlite::Error> {
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    let mut by_item = files_of(conn, &ids)?;
    for item in items.iter_mut() {
        if let Some((files, cover)) = by_item.remove(&item.id) {
            item.files = files;
            item.cover = cover;
        }
    }
    Ok(())
}

/// The item holding the stored file `name`, and which of its files it is (0 for the item's own).
pub(super) fn holding(conn: &Connection, name: &str) -> Result<Option<(i64, i64)>, rusqlite::Error> {
    let own: Option<i64> = conn.query_row("SELECT id FROM media_items WHERE upload_filename = ?1", params![name], |r| r.get(0)).optional()?;
    if let Some(id) = own {
        return Ok(Some((id, 0)));
    }
    conn.query_row("SELECT item_id, id FROM media_item_files WHERE upload_filename = ?1", params![name], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
}

/// A choice signed by the person who made it, as it is kept in the licence log: their key, the
/// exact purpose signed (`media_publish:...` or `media_edit:...`), the edit's text when it was an
/// edit (so the hash in the purpose can be checked again), the signed timestamp and the signature.
#[derive(Debug, Clone, Copy)]
pub struct Signed<'a> {
    pub key: &'a str,
    pub purpose: &'a str,
    pub body: &'a str,
    pub ts: i64,
    pub sig: &'a str,
}

/// One row of the licence log.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LicenceChoice {
    pub item_id: i64,
    pub licence: String,
    pub project_use: bool,
    pub set_by: String,
    pub signed_purpose: String,
    pub signed_body: String,
    pub signed_ts: i64,
    pub signature: String,
    pub set_at: i64,
}

/// Keep item `id`'s licence choice with the signature over it.
pub(super) fn log_licence(conn: &Connection, id: i64, licence: &str, project_use: bool, s: &Signed) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO media_licence_log (item_id, licence, project_use, set_by, signed_purpose, signed_body, signed_ts, signature, set_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, licence, project_use as i64, s.key, s.purpose, s.body, s.ts, s.sig, now_ms()],
    )?;
    Ok(())
}

/// What an edit sets: every field, as the form shows it (relay/media_edit.rs checks the lengths,
/// the labels against the labels file and the licence against the ones offered first).
#[derive(Debug, Clone, Copy)]
pub struct Edit<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub category_id: i64,
    /// The label ids, checked and joined as they are stored (relay/media.rs `normalize_labels`).
    pub labels: &'a str,
    pub licence: &'a str,
    pub project_use: bool,
}

/// A file added to an item: the item as it now is, and the old cover's stored name when a new
/// cover replaced it (its row went; the route deletes its file).
#[derive(Debug, Clone, PartialEq)]
pub struct Attached {
    pub item: MediaItem,
    pub replaced: Option<String>,
}

const MIB: i64 = 1024 * 1024;

impl Storage {
    /// The item holding the stored file `name`, and which of its files it is: 0 when it is the
    /// item's own file, else the added file's or cover's number. Publishing refuses such an upload,
    /// and a resumable upload's cancel answers `published` for it (relay/resumable.rs).
    pub fn media_item_holding(&self, name: &str) -> Result<Option<(i64, i64)>, rusqlite::Error> {
        self.with_read_conn(|conn| holding(conn, name))
    }

    /// Add the upload `upload` to item `id` as one of its files (`role` [`ROLE_FILE`]) or as its
    /// cover ([`ROLE_COVER`]). Only the uploader, with an upload of their own that waits on the
    /// Media shelf, is on no item and is not encrypted; at most [`ITEM_FILES_MAX`] files to an
    /// item; a cover only a picture; no bigger than the person's role's largest Media file
    /// (`max_file_mb`, 0 none). The upload already counts against the space and the share while it
    /// waits, and stays on the Media shelf, so nothing about the space changes. A new cover replaces
    /// the old one (its rows go; the route deletes its file). `place` runs before anything is
    /// committed, told whether the item is members only: the route moves the file out of the
    /// waiting folder there, and nothing is committed when the move fails.
    pub fn attach_media_file(
        &self,
        signer: &str,
        id: i64,
        upload: &str,
        role: &str,
        max_file_mb: i64,
        place: impl FnOnce(bool) -> std::io::Result<bool>,
    ) -> Result<Attached, MediaError> {
        if role != ROLE_FILE && role != ROLE_COVER {
            return refused(Refusal::Bad, "A file is added as \"file\" or as \"cover\".");
        }
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let found: Option<(String, String)> = tx
                .query_row("SELECT uploader_key, visibility FROM media_items WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            let Some((uploader, visibility)) = found else {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            };
            if uploader != signer {
                return refused(Refusal::NotAllowed, "Only the person who put it on the Media page can add files to it.");
            }
            let row: Option<(String, i64, i64)> = tx
                .query_row(
                    "SELECT public_key, shared, size_bytes FROM user_uploads WHERE filename = ?1 ORDER BY id LIMIT 1",
                    params![upload],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((_, _, size)) = row.filter(|(owner, shelf, _)| owner == signer && *shelf == MEDIA_SHELF) else {
                return refused(Refusal::NotFound, "There is no upload of yours waiting with that name.");
            };
            if upload.to_ascii_lowercase().ends_with(".enc") {
                return refused(Refusal::NotAllowed, "That file is encrypted for a private conversation, so it can never be shown on the Media page.");
            }
            if holding(&tx, upload)?.is_some() {
                return refused(Refusal::Conflict, ALREADY_ON_AN_ITEM);
            }
            if role == ROLE_COVER && kind_of(upload) != "picture" {
                return refused(Refusal::Bad, "A cover must be a picture: PNG, JPEG, GIF or WebP.");
            }
            if role == ROLE_FILE {
                let added: i64 =
                    tx.query_row("SELECT COUNT(*) FROM media_item_files WHERE item_id = ?1 AND role = ?2", params![id, ROLE_FILE], |r| r.get(0))?;
                if 1 + added >= ITEM_FILES_MAX {
                    return refused(Refusal::Bad, format!("An item can hold at most {ITEM_FILES_MAX} files."));
                }
            }
            if max_file_mb > 0 && size > max_file_mb.saturating_mul(MIB) {
                return refused(Refusal::TooLarge, crate::relay::upload_rules::media_file_limit_words(max_file_mb));
            }
            let mut replaced = None;
            if role == ROLE_COVER {
                let old: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT id, upload_filename FROM media_item_files WHERE item_id = ?1 AND role = ?2",
                        params![id, ROLE_COVER],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((old_id, old_name)) = old {
                    tx.execute("DELETE FROM media_item_files WHERE id = ?1", params![old_id])?;
                    tx.execute("DELETE FROM user_uploads WHERE filename = ?1 AND shared = ?2", params![old_name, MEDIA_SHELF])?;
                    replaced = Some(old_name);
                }
            }
            let position: i64 = tx.query_row(
                "SELECT COALESCE(MAX(position), 0) + 1 FROM media_item_files WHERE item_id = ?1 AND role = ?2",
                params![id, role],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO media_item_files (item_id, upload_filename, role, position, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, upload, role, position, now_ms()],
            )?;
            place(visibility == "members").map_err(|e| MediaError::Files(format!("The file could not be moved into place: {e}")))?;
            let item = item_by_id(&tx, id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?;
            tx.commit()?;
            Ok(Attached { item, replaced })
        })
    }

    /// Take file `file_no` off item `id`: its uploader, or an admin. Never file 0, the item itself
    /// (remove the item instead); `file_name` must be that file's stored name (W5). The file was
    /// uploaded for Media, so its rows go; returns the item as it now is and the stored name, whose
    /// file the route deletes.
    pub fn detach_media_file(&self, requester: &str, is_admin: bool, id: i64, file_no: i64, file_name: &str) -> Result<(MediaItem, String), MediaError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let uploader: Option<String> =
                tx.query_row("SELECT uploader_key FROM media_items WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
            let Some(uploader) = uploader else {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            };
            if uploader != requester && !is_admin {
                return refused(Refusal::NotAllowed, "Only the person who put it there, or an admin, can remove a file from a Media item.");
            }
            if file_no == 0 {
                return refused(Refusal::Bad, "The first file is the item itself: remove the item instead.");
            }
            let name: Option<String> = tx
                .query_row("SELECT upload_filename FROM media_item_files WHERE id = ?1 AND item_id = ?2", params![file_no, id], |r| r.get(0))
                .optional()?;
            let Some(name) = name else {
                return refused(Refusal::NotFound, "That Media item has no file with that number.");
            };
            if name != file_name.trim().trim_start_matches("/uploads/") {
                return refused(Refusal::Conflict, OUT_OF_DATE);
            }
            tx.execute("DELETE FROM media_item_files WHERE id = ?1", params![file_no])?;
            tx.execute("DELETE FROM user_uploads WHERE filename = ?1 AND shared = ?2", params![name, MEDIA_SHELF])?;
            let item = item_by_id(&tx, id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?;
            tx.commit()?;
            Ok((item, name))
        })
    }

    /// Edit item `id`: its uploader, or an admin. An admin changes the words, the category and the
    /// labels, never the licence or the game tick (`project_use`): those are the rights holder's grant
    /// (decision 1). A change of either is kept in the licence log with `signed`, the edit's own
    /// signature. Returns the item as it now is.
    pub fn edit_media(&self, requester: &str, is_admin: bool, id: i64, e: &Edit, signed: &Signed) -> Result<MediaItem, MediaError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let found: Option<(String, String, i64)> = tx
                .query_row("SELECT uploader_key, licence, project_use FROM media_items WHERE id = ?1", params![id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            let Some((uploader, licence, project_use)) = found else {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            };
            if uploader != requester && !is_admin {
                return refused(Refusal::NotAllowed, "Only the person who put it there, or an admin, can edit a Media item.");
            }
            let licence_changes = e.licence != licence || e.project_use != (project_use != 0);
            if licence_changes && uploader != requester {
                return refused(
                    Refusal::NotAllowed,
                    "Only the person who put it on the Media page can change its licence or offer it for the game.",
                );
            }
            let category_exists: bool =
                tx.query_row("SELECT EXISTS(SELECT 1 FROM media_categories WHERE id = ?1)", params![e.category_id], |r| r.get(0))?;
            if !category_exists {
                return refused(Refusal::Bad, format!("There is no category number {} on this server.", e.category_id));
            }
            tx.execute(
                "UPDATE media_items SET title = ?1, description = ?2, category_id = ?3, labels = ?4, licence = ?5, project_use = ?6 WHERE id = ?7",
                params![
                    clipped(e.title, MEDIA_TITLE_MAX),
                    clipped(e.description, MEDIA_DESCRIPTION_MAX),
                    e.category_id,
                    e.labels,
                    e.licence,
                    e.project_use as i64,
                    id
                ],
            )?;
            if licence_changes {
                log_licence(&tx, id, e.licence, e.project_use, signed)?;
            }
            let item = item_by_id(&tx, id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?;
            tx.commit()?;
            Ok(item)
        })
    }

    /// Item `id`'s licence choices, oldest first.
    pub fn media_licence_log(&self, id: i64) -> Result<Vec<LicenceChoice>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT item_id, licence, project_use, set_by, signed_purpose, signed_body, signed_ts, signature, set_at
                 FROM media_licence_log WHERE item_id = ?1 ORDER BY id ASC",
            )?;
            let rows = stmt
                .query_map(params![id], |r| {
                    Ok(LicenceChoice {
                        item_id: r.get(0)?,
                        licence: r.get(1)?,
                        project_use: r.get::<_, i64>(2)? != 0,
                        set_by: r.get(3)?,
                        signed_purpose: r.get(4)?,
                        signed_body: r.get(5)?,
                        signed_ts: r.get(6)?,
                        signature: r.get(7)?,
                        set_at: r.get(8)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// One more view of item `id`; whether there is such an item. Who is counted, once a day, is
    /// decided in memory before this (relay/media_counts.rs).
    pub fn add_media_view(&self, id: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| Ok(conn.execute("UPDATE media_items SET views = views + 1 WHERE id = ?1", params![id])? == 1))
    }

    /// One more download of item `id`; whether there is such an item.
    pub fn add_media_download(&self, id: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| Ok(conn.execute("UPDATE media_items SET downloads = downloads + 1 WHERE id = ?1", params![id])? == 1))
    }
}

#[cfg(test)]
#[path = "media_files_tests.rs"]
mod tests;
