//! The Media page's storage (2026-10-11; docs/design/media-showcase.md, "Increment 1, exact").
//! The routes that use it are src/relay/media.rs.
//!
//! Four tables, each created in storage/mod.rs:
//! - `media_items`: what people published from their OWN uploads, one row per upload. The file
//!   itself stays a `user_uploads` row. A Media upload (`POST /api/upload?media=1`) sits on shelf
//!   2 (`user_uploads.shared = 2`): out of the per-person keep-N FIFO (which prunes shelf 0 only)
//!   and out of the Shared Files list (which lists shelf 1 only). Publishing moves a chat file
//!   (shelf 0) to shelf 2 too, and a Shared Files file (shelf 1) to shelf 2 while it is members
//!   only; the item remembers where its upload was (`from_shelf`), and removing the item gives
//!   the upload back there and keeps its file (the review's F2, 2026-10-11).
//! - `media_categories`: the server's own, in position order. Row 1 is General: it cannot be
//!   removed, because a removed category's items move to it.
//! - `media_category_suggestions`: categories members asked for, open until an admin decides.
//! - `media_picks`: each person's picks of other people's items. A pick is a reference, so it
//!   goes when the item goes.
//!
//! THE FILES live in two folders ([`Folders`]): the public one, served at `/uploads`, and
//! `data/media-members/`, which nothing serves but `GET /api/media/file/{id}` to a member. A
//! members-only item's file is in the second (F3).
//!
//! Every refusal says what to do in words a person can act on ([`MediaError::Refused`]); the
//! routes turn its [`Refusal`] into the HTTP status.

use super::Storage;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// The public uploads folder, served at `/uploads` (relay/mod.rs).
pub const UPLOADS_DIR: &str = "data/uploads";
/// The members-only Media folder: nothing serves it but `GET /api/media/file/{id}` (relay/media.rs).
pub const MEMBERS_DIR: &str = "data/media-members";
/// How long a Media upload waits to be published before the expiry pass deletes it, file and
/// row (storage/expiry.rs; the review's F1).
pub const UNPUBLISHED_KEEP_MS: i64 = 24 * 60 * 60 * 1000;

/// Where the Media page's files are: the public uploads folder and the members-only one. The
/// relay's own are [`Folders::live`]; a test names folders of its own, so nothing it does lands in
/// the checkout's data folder.
#[derive(Debug, Clone, Copy)]
pub struct Folders<'a> {
    pub uploads: &'a Path,
    pub members: &'a Path,
}

impl Folders<'static> {
    /// The relay's own folders, `data/uploads` and `data/media-members`.
    pub fn live() -> Self {
        Folders { uploads: Path::new(UPLOADS_DIR), members: Path::new(MEMBERS_DIR) }
    }
}

/// A stored file name that is safe to join to a folder: not empty, no separator, no "..".
pub fn bare_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains('\\') && !name.contains("..")
}

impl Folders<'_> {
    /// Where `name` is now: the members-only folder first, then the public one.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        if !bare_name(name) {
            return None;
        }
        [self.members, self.uploads].into_iter().map(|d| d.join(name)).find(|p| p.is_file())
    }

    /// Delete `name` from both folders; whether a file was deleted.
    pub fn delete(&self, name: &str) -> bool {
        if !bare_name(name) {
            return false;
        }
        let members = std::fs::remove_file(self.members.join(name)).is_ok();
        let public = std::fs::remove_file(self.uploads.join(name)).is_ok();
        members || public
    }

    /// Move `name` into the members-only folder (`members`) or back to the public one; whether
    /// it moved. A file already there, or in neither, is left alone. A copy (another disk) that
    /// cannot remove its source removes the copy and fails, so a file is never in both.
    pub fn place(&self, name: &str, members: bool) -> std::io::Result<bool> {
        if !bare_name(name) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a bare file name"));
        }
        let (from, to) = if members { (self.uploads, self.members) } else { (self.members, self.uploads) };
        let src = from.join(name);
        if !src.is_file() {
            return Ok(false);
        }
        std::fs::create_dir_all(to)?;
        let dst = to.join(name);
        if std::fs::rename(&src, &dst).is_err() {
            std::fs::copy(&src, &dst)?;
            if let Err(e) = std::fs::remove_file(&src) {
                let _ = std::fs::remove_file(&dst);
                return Err(e);
            }
        }
        Ok(true)
    }
}

/// The category every server has and nobody can remove: row 1, "General" until an admin renames
/// it. A removed category's items move here.
pub const GENERAL_CATEGORY_ID: i64 = 1;
/// The `user_uploads.shared` value of a Media upload or a published one (0 is chat media under
/// the keep-N FIFO, 1 the Shared Files library).
pub const MEDIA_SHELF: i64 = 2;
/// The most items one page of the list holds, and the number when none is asked for.
pub const MEDIA_LIST_MAX: i64 = 100;
pub const MEDIA_LIST_DEFAULT: i64 = 50;
/// How long the words people type may be, in characters.
pub const MEDIA_TITLE_MAX: usize = 120;
pub const MEDIA_DESCRIPTION_MAX: usize = 2000;
pub const CATEGORY_NAME_MAX: usize = 60;
pub const CATEGORY_DESCRIPTION_MAX: usize = 300;
pub const SUGGESTION_NOTE_MAX: usize = 500;
/// The most categories one server can have.
pub const CATEGORIES_MAX: usize = 100;
/// The most suggestions one person can have waiting for an admin at once.
pub const OPEN_SUGGESTIONS_PER_PERSON: i64 = 5;
/// The most items one person can pick.
pub const PICKS_PER_PERSON: i64 = 1000;
/// How many items each prune list in the usage numbers holds.
pub const USAGE_LIST_LEN: i64 = 20;

/// Why a Media request was refused, for the HTTP status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Something in the request is wrong (a name too long, a category that does not exist).
    Bad,
    /// The person may not do this (someone else's upload, not an admin).
    NotAllowed,
    /// No such item, upload or suggestion.
    NotFound,
    /// It is already so (published twice, a suggestion already decided).
    Conflict,
    /// The Media space, or the person's share of it, has no room for this.
    Full,
}

/// A refused Media request (with its words), the database failing underneath, or a file that
/// could not be moved (nothing was committed then).
#[derive(Debug)]
pub enum MediaError {
    Refused(Refusal, String),
    Db(rusqlite::Error),
    Files(String),
}

impl From<rusqlite::Error> for MediaError {
    fn from(e: rusqlite::Error) -> Self {
        MediaError::Db(e)
    }
}

fn refused<T>(why: Refusal, words: impl Into<String>) -> Result<T, MediaError> {
    Err(MediaError::Refused(why, words.into()))
}

/// Bytes as people read them on this page: megabytes with one decimal.
pub fn mb(bytes: i64) -> String {
    format!("{:.1} MB", bytes.max(0) as f64 / (1024.0 * 1024.0))
}

/// The refusal when the Media space has no room: what is used, the budget, and the file.
pub fn budget_full_words(used: i64, budget: i64, size: i64) -> String {
    format!(
        "The Media space on this server is full: {} of {} used, and this file is {}. An admin can remove items or raise the Media budget in Server Settings.",
        mb(used),
        mb(budget),
        mb(size)
    )
}

/// The refusal when the person's share of the Media space has no room for the file.
pub fn share_full_words(mine: i64, share: i64, size: i64) -> String {
    format!(
        "Your share of the Media space is used up: {} of your {} used, and this file is {}. Remove one of your items, or ask an admin to raise each person's share in Server Settings.",
        mb(mine),
        mb(share),
        mb(size)
    )
}

/// One Media item as the list gives it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MediaItem {
    pub id: i64,
    pub upload_filename: String,
    pub uploader_key: String,
    /// The uploader's name on this server ('' when unknown).
    pub uploader_name: String,
    pub title: String,
    pub description: String,
    pub category_id: i64,
    pub category_name: String,
    /// `picture`, `video`, `model` or `file`.
    pub kind: String,
    /// The label ids, in the order data/media/labels.json lists them.
    pub labels: Vec<String>,
    /// `public` or `members`.
    pub visibility: String,
    pub featured: bool,
    pub size_bytes: i64,
    /// Nothing counts views yet: that waits for increment 2's viewer (the review's F7), so until
    /// then every item reads 0 and the least-viewed prune list is oldest first.
    pub views: i64,
    /// When the file was uploaded, and when it was put on the Media page (Unix ms).
    pub uploaded_at: i64,
    pub published_at: i64,
    /// How many people picked it.
    pub picks: i64,
}

/// One category, in the order the page shows them.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MediaCategory {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub position: i64,
}

/// One category in an admin's whole-list replace: an existing one by its `id` (renamed or moved
/// by its place in the list), or a new one with no id.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct CategoryEdit {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// A category a member asked for.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MediaSuggestion {
    pub id: i64,
    pub suggester_key: String,
    pub suggester_name: String,
    pub name: String,
    pub note: String,
    pub created_at: i64,
    /// `open`, `accepted` or `declined`.
    pub state: String,
}

/// What an item is to be published as. `labels` is already checked against the labels file and
/// joined (relay/media.rs `normalize_labels`); `visibility` is `public` or `members`.
#[derive(Debug, Clone, Default)]
pub struct Publish<'a> {
    pub filename: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub category_id: Option<i64>,
    pub labels: &'a str,
    pub visibility: &'a str,
}

/// The list's filters. `members` is whether the viewer gets the members' view (members-only
/// items included); the route decides it from a checked signature.
#[derive(Debug, Clone, Default)]
pub struct MediaQuery {
    pub category: Option<i64>,
    pub kind: Option<String>,
    pub by: Option<String>,
    pub q: Option<String>,
    pub before: Option<i64>,
    pub limit: Option<i64>,
    pub members: bool,
    pub picked_by: Option<String>,
}

/// One person's use of the Media space.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PersonUse {
    pub key: String,
    pub name: String,
    pub bytes: i64,
    pub items: i64,
}

/// A Media upload not yet published: it counts against the Media space, and the expiry pass
/// deletes it at `expires_at` unless it is published first (the review's F1).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WaitingUpload {
    pub filename: String,
    pub uploader_key: String,
    pub uploader_name: String,
    pub size_bytes: i64,
    /// Unix ms.
    pub uploaded_at: i64,
    pub expires_at: i64,
}

/// The numbers an admin prunes by.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MediaUsage {
    /// Bytes of every published item and every Media upload not yet published.
    pub used_bytes: i64,
    pub items: i64,
    pub people: Vec<PersonUse>,
    pub largest: Vec<MediaItem>,
    pub oldest: Vec<MediaItem>,
    pub least_viewed: Vec<MediaItem>,
    /// Media uploads waiting to be published, oldest first.
    pub waiting: Vec<WaitingUpload>,
}

/// What removing an item left for the route to do with its file: give it back to the public
/// folder (an upload from chat or Shared Files, `from_shelf` 0 or 1, kept) or delete it (one
/// uploaded for Media, 2).
#[derive(Debug, Clone, PartialEq)]
pub struct Removed {
    pub filename: String,
    pub from_shelf: i64,
}

impl Removed {
    /// Whether the file stays (the upload went back to chat or Shared Files).
    pub fn keeps_file(&self) -> bool {
        was_public(self.from_shelf)
    }
}

/// Whether an upload from `shelf` was public before it went on the Media page: a chat upload
/// (shelf 0) is at its `/uploads/` address in the messages that carry it, and a Shared Files one
/// (1) is in that list. Such a file goes on Media as public only: making it members only would
/// take it from under the messages and the list that already show it, and could not take back
/// the copies anyone already saved (decided 2026-10-11, the review's question after F3).
pub fn was_public(shelf: i64) -> bool {
    shelf == 0 || shelf == 1
}

/// What is said when a file that is already public is asked to be members only.
pub const ALREADY_PUBLIC: &str = "That file is already public where it was shared (in a chat or the Shared Files list), so it can only go on the Media page as public. To share something with members only, upload it on the Media page itself.";

/// The shelf an upload from `from_shelf` sits on while it is published with `visibility`: a
/// Shared Files file stays listed there (it is always public: [`was_public`]); everything else is
/// on the Media shelf, out of the keep-N FIFO.
fn published_shelf(from_shelf: i64, visibility: &str) -> i64 {
    if from_shelf == 1 && visibility == "public" {
        1
    } else {
        MEDIA_SHELF
    }
}

/// An item's visibility as it is stored: "public" only when it says so; anything else is
/// members only (fail closed, the review's F7).
fn stored_visibility(v: &str) -> &'static str {
    if v == "public" {
        "public"
    } else {
        "members"
    }
}

/// An item's kind from its stored file name, which the upload route gave the extension of what
/// the file is: a picture, a video, a 3D model, or any other file.
pub fn kind_of(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "webp" => "picture",
        "mp4" | "webm" => "video",
        "blend" | "stl" | "obj" | "gltf" | "glb" => "model",
        _ => "file",
    }
}

/// The kinds, for checking a filter.
pub const KINDS: [&str; 4] = ["picture", "video", "model", "file"];

/// The bytes counted against the Media space: every upload on the Media shelf and every upload
/// published as an item (an upload from the Shared Files library counts once published), for
/// `key` alone or for everyone, leaving out `except` (an upload being published, already counted
/// if it was a Media upload).
fn media_bytes_on(conn: &Connection, key: Option<&str>, except: Option<&str>) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COALESCE(SUM(size_bytes), 0) FROM user_uploads
         WHERE (shared = ?1 OR filename IN (SELECT upload_filename FROM media_items))
           AND (?2 IS NULL OR public_key = ?2)
           AND (?3 IS NULL OR filename != ?3)",
        params![MEDIA_SHELF, key, except],
        |r| r.get(0),
    )
}

/// The columns of [`MediaItem`] in [`item_from_row`]'s order, over `media_items m`.
const ITEM_COLUMNS: &str = "m.id, m.upload_filename, m.uploader_key, COALESCE(sm.name, ''), m.title, m.description,
    m.category_id, COALESCE(c.name, ''), m.kind, m.labels, m.visibility, m.featured, m.size_bytes, m.views,
    COALESCE((SELECT MIN(u.uploaded_at) FROM user_uploads u WHERE u.filename = m.upload_filename), m.created_at),
    m.created_at, (SELECT COUNT(*) FROM media_picks p WHERE p.item_id = m.id)";
const ITEM_JOINS: &str = "FROM media_items m
    LEFT JOIN server_members sm ON sm.public_key = m.uploader_key
    LEFT JOIN media_categories c ON c.id = m.category_id";

fn item_from_row(r: &rusqlite::Row) -> Result<MediaItem, rusqlite::Error> {
    let labels: String = r.get(9)?;
    Ok(MediaItem {
        id: r.get(0)?,
        upload_filename: r.get(1)?,
        uploader_key: r.get(2)?,
        uploader_name: r.get(3)?,
        title: r.get(4)?,
        description: r.get(5)?,
        category_id: r.get(6)?,
        category_name: r.get(7)?,
        kind: r.get(8)?,
        labels: labels.split(',').filter(|l| !l.is_empty()).map(str::to_string).collect(),
        visibility: r.get(10)?,
        featured: r.get::<_, i64>(11)? != 0,
        size_bytes: r.get(12)?,
        views: r.get(13)?,
        uploaded_at: r.get(14)?,
        published_at: r.get(15)?,
        picks: r.get(16)?,
    })
}

fn item_by_id(conn: &Connection, id: i64) -> Result<Option<MediaItem>, rusqlite::Error> {
    conn.query_row(&format!("SELECT {ITEM_COLUMNS} {ITEM_JOINS} WHERE m.id = ?1"), params![id], item_from_row)
        .optional()
}

/// Items by an ORDER BY over `media_items m`, `limit` of them, for the prune lists.
fn items_ordered(conn: &Connection, order: &str, limit: i64) -> Result<Vec<MediaItem>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("SELECT {ITEM_COLUMNS} {ITEM_JOINS} ORDER BY {order} LIMIT ?1"))?;
    let rows = stmt.query_map(params![limit], item_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// `text` held to `max` characters, trimmed.
fn clipped(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// A search term made safe for LIKE: its `%`, `_` and `\` matched as themselves.
fn like_pattern(q: &str) -> String {
    let mut out = String::from("%");
    for c in q.trim().to_lowercase().chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

fn now_ms() -> i64 {
    super::now_millis() as i64
}

impl Storage {
    /// The bytes counted against the Media space, for `key` alone or everyone. A Media upload
    /// (`POST /api/upload?media=1`) counts from the moment it is stored (`record_upload` with
    /// [`MEDIA_SHELF`]), published or not.
    pub fn media_bytes(&self, key: Option<&str>) -> i64 {
        self.with_conn(|conn| media_bytes_on(conn, key, None)).unwrap_or(0)
    }

    /// Put one of `signer`'s own uploads on the Media page. Refused: an upload that is not theirs,
    /// an encrypted one (a file from a private conversation can never be shown), one already
    /// published, a category that does not exist, and a file the Media space (`budget` bytes) or
    /// the person's share of it (`share` bytes) has no room for, with both numbers in the words.
    /// `disk_size` is the file's size on disk, used when the upload row has none (a row from
    /// before sizes were kept); the row then takes that size, so the budget counts it (F6). A
    /// chat upload moves to the Media shelf, so the keep-N FIFO never takes a published file; a
    /// Shared Files upload published as members only leaves that list while it is published.
    pub fn publish_media(&self, signer: &str, p: &Publish, budget: i64, share: i64, disk_size: Option<i64>) -> Result<MediaItem, MediaError> {
        self.publish_media_placing(signer, p, budget, share, disk_size, |_| Ok(false))
    }

    /// [`Self::publish_media`], with `place` run once everything is checked and before the item
    /// is committed, told whether the item is members only. The route moves a members-only
    /// item's file out of the public folder there, so an item is never committed with its file
    /// still public, and nothing is committed when the move fails (F3).
    pub fn publish_media_placing(
        &self,
        signer: &str,
        p: &Publish,
        budget: i64,
        share: i64,
        disk_size: Option<i64>,
        place: impl FnOnce(bool) -> std::io::Result<bool>,
    ) -> Result<MediaItem, MediaError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let upload: Option<(String, i64, i64)> = tx
                .query_row(
                    "SELECT public_key, shared, size_bytes FROM user_uploads WHERE filename = ?1 ORDER BY id LIMIT 1",
                    params![p.filename],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((owner, shelf, row_size)) = upload else {
                return refused(Refusal::NotFound, "There is no upload with that name on this server.");
            };
            if owner != signer {
                return refused(Refusal::NotAllowed, "You can only put your own uploads on the Media page.");
            }
            if p.filename.to_ascii_lowercase().ends_with(".enc") {
                return refused(
                    Refusal::NotAllowed,
                    "That file is encrypted for a private conversation, so it can never be shown on the Media page.",
                );
            }
            let already: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM media_items WHERE upload_filename = ?1)",
                params![p.filename],
                |r| r.get(0),
            )?;
            if already {
                return refused(Refusal::Conflict, "That upload is already on the Media page.");
            }
            let visibility = stored_visibility(p.visibility);
            if visibility == "members" && was_public(shelf) {
                return refused(Refusal::Bad, ALREADY_PUBLIC);
            }
            let category = p.category_id.unwrap_or(GENERAL_CATEGORY_ID);
            let category_exists: bool =
                tx.query_row("SELECT EXISTS(SELECT 1 FROM media_categories WHERE id = ?1)", params![category], |r| r.get(0))?;
            if !category_exists {
                return refused(Refusal::Bad, format!("There is no category number {category} on this server."));
            }
            let size = if row_size > 0 { row_size } else { disk_size.unwrap_or(0).max(0) };
            let used = media_bytes_on(&tx, None, Some(p.filename))?;
            if used + size > budget {
                return refused(Refusal::Full, budget_full_words(used, budget, size));
            }
            let mine = media_bytes_on(&tx, Some(signer), Some(p.filename))?;
            if mine + size > share {
                return refused(Refusal::Full, share_full_words(mine, share, size));
            }
            tx.execute(
                "INSERT INTO media_items (upload_filename, uploader_key, title, description, category_id, kind, labels, visibility, featured, size_bytes, views, created_at, from_shelf)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, 0, ?10, ?11)",
                params![
                    p.filename,
                    signer,
                    clipped(p.title, MEDIA_TITLE_MAX),
                    clipped(p.description, MEDIA_DESCRIPTION_MAX),
                    category,
                    kind_of(p.filename),
                    p.labels,
                    visibility,
                    size,
                    now_ms(),
                    shelf
                ],
            )?;
            let id = tx.last_insert_rowid();
            // Off the keep-N FIFO; a Shared Files upload stays listed there only while it is
            // public (F3). A row with no size takes the size found on disk (F6).
            tx.execute(
                "UPDATE user_uploads SET shared = ?1, size_bytes = CASE WHEN size_bytes > 0 THEN size_bytes ELSE ?2 END
                 WHERE filename = ?3",
                params![published_shelf(shelf, visibility), size, p.filename],
            )?;
            let item = item_by_id(&tx, id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?;
            place(visibility == "members").map_err(|e| MediaError::Files(format!("The file could not be moved into place: {e}")))?;
            tx.commit()?;
            Ok(item)
        })
    }

    /// Make item `id` public or members only (`visibility`): its uploader, or an admin. An item
    /// whose file was already public (chat or Shared Files, [`was_public`]) stays public; `place`
    /// runs before the change is committed, told whether the item is now members only, so the
    /// route moves the file with it and nothing changes when the move fails (F3). Returns the
    /// item as it now is.
    pub fn set_media_visibility(
        &self,
        id: i64,
        requester: &str,
        is_admin: bool,
        visibility: &str,
        place: impl FnOnce(&str, bool) -> std::io::Result<bool>,
    ) -> Result<MediaItem, MediaError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let found: Option<(String, String, i64, String)> = tx
                .query_row(
                    "SELECT uploader_key, upload_filename, from_shelf, visibility FROM media_items WHERE id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let Some((uploader, filename, from_shelf, now)) = found else {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            };
            if uploader != requester && !is_admin {
                return refused(Refusal::NotAllowed, "Only the person who put it there, or an admin, can change who sees a Media item.");
            }
            let to = stored_visibility(visibility);
            if to == "members" && was_public(from_shelf) {
                return refused(Refusal::Bad, ALREADY_PUBLIC);
            }
            if now != to {
                tx.execute("UPDATE media_items SET visibility = ?1 WHERE id = ?2", params![to, id])?;
                tx.execute(
                    "UPDATE user_uploads SET shared = ?1 WHERE filename = ?2",
                    params![published_shelf(from_shelf, to), filename],
                )?;
            }
            place(&filename, to == "members").map_err(|e| MediaError::Files(format!("The file could not be moved: {e}")))?;
            let item = item_by_id(&tx, id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?;
            tx.commit()?;
            Ok(item)
        })
    }

    /// The list, newest first: one page of up to [`MEDIA_LIST_MAX`] items older than `before`.
    /// Members-only items only in the members' view.
    pub fn list_media(&self, q: &MediaQuery) -> Result<Vec<MediaItem>, rusqlite::Error> {
        let limit = q.limit.unwrap_or(MEDIA_LIST_DEFAULT).clamp(1, MEDIA_LIST_MAX);
        let search = q.q.as_deref().filter(|s| !s.trim().is_empty()).map(like_pattern);
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {ITEM_COLUMNS} {ITEM_JOINS}
                 WHERE (?1 IS NULL OR m.category_id = ?1)
                   AND (?2 IS NULL OR m.kind = ?2)
                   AND (?3 IS NULL OR m.uploader_key = ?3)
                   AND (?4 IS NULL OR LOWER(m.title) LIKE ?4 ESCAPE '\\' OR LOWER(m.description) LIKE ?4 ESCAPE '\\')
                   AND (?5 IS NULL OR m.id < ?5)
                   AND (?6 = 1 OR m.visibility = 'public')
                   AND (?7 IS NULL OR m.id IN (SELECT item_id FROM media_picks WHERE picker_key = ?7))
                 ORDER BY m.id DESC LIMIT ?8"
            ))?;
            let rows = stmt
                .query_map(
                    params![q.category, q.kind, q.by, search, q.before, q.members as i64, q.picked_by, limit],
                    item_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// One item, whoever may see it (the route decides that).
    pub fn media_item(&self, id: i64) -> Result<Option<MediaItem>, rusqlite::Error> {
        self.with_conn(|conn| item_by_id(conn, id))
    }

    /// Take an item off the Media page: its uploader, or an admin (`is_admin`). The item and
    /// every pick of it go. Its upload goes back where it was before it was published (a chat
    /// file to chat, a Shared Files file to that list) and keeps its file, so an admin taking
    /// someone's chat photo off the Media page never breaks the chat message; only an upload
    /// made for Media loses its row, and the route its file (F2). Returns what the route does
    /// with the file.
    pub fn remove_media(&self, id: i64, requester: &str, is_admin: bool) -> Result<Removed, MediaError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let found: Option<(String, String, i64)> = tx
                .query_row("SELECT uploader_key, upload_filename, from_shelf FROM media_items WHERE id = ?1", params![id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            let Some((uploader, filename, from_shelf)) = found else {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            };
            if uploader != requester && !is_admin {
                return refused(Refusal::NotAllowed, "Only the person who put it there, or an admin, can remove a Media item.");
            }
            tx.execute("DELETE FROM media_picks WHERE item_id = ?1", params![id])?;
            tx.execute("DELETE FROM media_items WHERE id = ?1", params![id])?;
            let removed = Removed { filename, from_shelf };
            if removed.keeps_file() {
                tx.execute("UPDATE user_uploads SET shared = ?1 WHERE filename = ?2", params![from_shelf, removed.filename])?;
            } else {
                tx.execute("DELETE FROM user_uploads WHERE filename = ?1", params![removed.filename])?;
            }
            tx.commit()?;
            Ok(removed)
        })
    }

    /// The item made from upload `filename`, if it is published.
    pub fn media_item_for_upload(&self, filename: &str) -> Result<Option<i64>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row("SELECT id FROM media_items WHERE upload_filename = ?1", params![filename], |r| r.get(0)).optional()
        })
    }

    /// Remove a Media upload not yet published (`POST /api/media/remove` with its file name):
    /// its owner, or an admin. The row goes; the route deletes the file (F1). Refused: no such
    /// upload waiting, or someone else's.
    pub fn remove_unpublished_media(&self, filename: &str, requester: &str, is_admin: bool) -> Result<(), MediaError> {
        self.with_conn(|conn| {
            let owner: Option<String> = conn
                .query_row(
                    "SELECT public_key FROM user_uploads
                     WHERE filename = ?1 AND shared = ?2 AND filename NOT IN (SELECT upload_filename FROM media_items)
                     ORDER BY id LIMIT 1",
                    params![filename, MEDIA_SHELF],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(owner) = owner else {
                return refused(Refusal::NotFound, "There is no Media upload waiting to be published with that name.");
            };
            if owner != requester && !is_admin {
                return refused(Refusal::NotAllowed, "Only the person who uploaded it, or an admin, can remove a Media upload.");
            }
            conn.execute("DELETE FROM user_uploads WHERE filename = ?1 AND shared = ?2", params![filename, MEDIA_SHELF])?;
            Ok(())
        })
    }

    /// The expiry pass's share (storage/expiry.rs): every Media upload not published within
    /// [`UNPUBLISHED_KEEP_MS`] of `now_ms` is deleted, row and file, from `folders`. A Media
    /// upload is a Media file, so this is the only thing that takes one before it is published:
    /// nothing prunes it to make room for a chat file (F1). Returns how many went.
    pub fn expire_unpublished_media_in(&self, folders: Folders, now_ms: i64) -> Result<usize, rusqlite::Error> {
        let names: Vec<String> = self.with_conn(|conn| {
            let names: Vec<String> = {
                let mut stmt = conn.prepare(
                    "SELECT filename FROM user_uploads
                     WHERE shared = ?1 AND uploaded_at < ?2 AND filename NOT IN (SELECT upload_filename FROM media_items)",
                )?;
                let names = stmt.query_map(params![MEDIA_SHELF, now_ms - UNPUBLISHED_KEEP_MS], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
                names
            };
            for name in &names {
                conn.execute("DELETE FROM user_uploads WHERE filename = ?1 AND shared = ?2", params![name, MEDIA_SHELF])?;
            }
            Ok::<_, rusqlite::Error>(names)
        })?;
        for name in &names {
            folders.delete(name);
        }
        Ok(names.len())
    }

    /// `picker` picks item `id`; returns how many picks it has now. Picking twice is one pick.
    /// Refused: an item that does not exist, the picker's own (picks are for other people's), and
    /// past [`PICKS_PER_PERSON`].
    pub fn pick_media(&self, picker: &str, id: i64) -> Result<i64, MediaError> {
        self.with_conn(|conn| {
            let uploader: Option<String> = conn
                .query_row("SELECT uploader_key FROM media_items WHERE id = ?1", params![id], |r| r.get(0))
                .optional()?;
            let Some(uploader) = uploader else {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            };
            if uploader == picker {
                return refused(Refusal::Bad, "Picks are for other people's items; yours are already listed under your name.");
            }
            let mine: i64 = conn.query_row("SELECT COUNT(*) FROM media_picks WHERE picker_key = ?1", params![picker], |r| r.get(0))?;
            if mine >= PICKS_PER_PERSON {
                return refused(Refusal::Full, format!("You have {PICKS_PER_PERSON} picks, the most one person can have. Unpick one first."));
            }
            conn.execute(
                "INSERT OR IGNORE INTO media_picks (picker_key, item_id, picked_at) VALUES (?1, ?2, ?3)",
                params![picker, id, now_ms()],
            )?;
            Ok(conn.query_row("SELECT COUNT(*) FROM media_picks WHERE item_id = ?1", params![id], |r| r.get(0))?)
        })
    }

    /// `picker` takes back their pick of item `id` (nothing happens when there was none);
    /// returns how many picks it has now.
    pub fn unpick_media(&self, picker: &str, id: i64) -> Result<i64, MediaError> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM media_picks WHERE picker_key = ?1 AND item_id = ?2", params![picker, id])?;
            Ok(conn.query_row("SELECT COUNT(*) FROM media_picks WHERE item_id = ?1", params![id], |r| r.get(0))?)
        })
    }

    /// Feature item `id` on the page, or take it off the featured row (the route checks the
    /// admin).
    pub fn set_media_featured(&self, id: i64, featured: bool) -> Result<(), MediaError> {
        self.with_conn(|conn| {
            let n = conn.execute("UPDATE media_items SET featured = ?1 WHERE id = ?2", params![featured as i64, id])?;
            if n == 0 {
                return refused(Refusal::NotFound, "There is no Media item with that number.");
            }
            Ok(())
        })
    }

    /// The server's categories, in the order the page shows them.
    pub fn media_categories(&self) -> Result<Vec<MediaCategory>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, name, description, position FROM media_categories ORDER BY position ASC, id ASC")?;
            let rows = stmt
                .query_map([], |r| Ok(MediaCategory { id: r.get(0)?, name: r.get(1)?, description: r.get(2)?, position: r.get(3)? }))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Replace the whole list at once (Server Settings' category editor): each entry an existing
    /// category by id (renamed, described, moved by its place) or a new one; a category left out
    /// is removed and its items move to General. Refused whole, nothing changed: General left
    /// out, an id that is not a category here, an empty or repeated name, more than
    /// [`CATEGORIES_MAX`].
    pub fn replace_media_categories(&self, list: &[CategoryEdit]) -> Result<Vec<MediaCategory>, MediaError> {
        if list.len() > CATEGORIES_MAX {
            return refused(Refusal::Bad, format!("A server can have at most {CATEGORIES_MAX} categories."));
        }
        if !list.iter().any(|c| c.id == Some(GENERAL_CATEGORY_ID)) {
            return refused(
                Refusal::Bad,
                "General (category 1) cannot be removed: it holds the items of removed categories. It can be renamed.",
            );
        }
        let mut seen = std::collections::HashSet::new();
        for c in list {
            let name = c.name.trim();
            if name.is_empty() {
                return refused(Refusal::Bad, "Every category needs a name.");
            }
            if name.chars().count() > CATEGORY_NAME_MAX {
                return refused(Refusal::Bad, format!("A category name can be at most {CATEGORY_NAME_MAX} characters: \"{name}\"."));
            }
            if !seen.insert(name.to_lowercase()) {
                return refused(Refusal::Bad, format!("Two categories are both called \"{name}\"."));
            }
        }
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let existing: Vec<i64> = {
                let mut stmt = tx.prepare("SELECT id FROM media_categories")?;
                let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
                ids
            };
            for c in list {
                if let Some(id) = c.id {
                    if !existing.contains(&id) {
                        return refused(Refusal::Bad, format!("There is no category number {id} on this server."));
                    }
                }
            }
            let kept: Vec<i64> = list.iter().filter_map(|c| c.id).collect();
            for id in existing.iter().filter(|id| !kept.contains(id)) {
                tx.execute("UPDATE media_items SET category_id = ?1 WHERE category_id = ?2", params![GENERAL_CATEGORY_ID, id])?;
                tx.execute("DELETE FROM media_categories WHERE id = ?1", params![id])?;
            }
            for (position, c) in list.iter().enumerate() {
                let name = clipped(&c.name, CATEGORY_NAME_MAX);
                let description = clipped(&c.description, CATEGORY_DESCRIPTION_MAX);
                match c.id {
                    Some(id) => tx.execute(
                        "UPDATE media_categories SET name = ?1, description = ?2, position = ?3 WHERE id = ?4",
                        params![name, description, position as i64, id],
                    )?,
                    None => tx.execute(
                        "INSERT INTO media_categories (name, description, position) VALUES (?1, ?2, ?3)",
                        params![name, description, position as i64],
                    )?,
                };
            }
            tx.commit()?;
            Ok(())
        })?;
        Ok(self.media_categories()?)
    }

    /// A member asks for a category. Refused: an empty or long name, one that is already a
    /// category or already waiting, and past [`OPEN_SUGGESTIONS_PER_PERSON`] waiting.
    pub fn suggest_media_category(&self, key: &str, name: &str, note: &str) -> Result<MediaSuggestion, MediaError> {
        let name = name.trim();
        if name.is_empty() {
            return refused(Refusal::Bad, "A suggestion needs the category's name.");
        }
        if name.chars().count() > CATEGORY_NAME_MAX {
            return refused(Refusal::Bad, format!("A category name can be at most {CATEGORY_NAME_MAX} characters."));
        }
        let id = self.with_conn(|conn| {
            let taken: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM media_categories WHERE LOWER(name) = LOWER(?1))",
                params![name],
                |r| r.get(0),
            )?;
            if taken {
                return refused(Refusal::Conflict, format!("\"{name}\" is already a category."));
            }
            let waiting: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM media_category_suggestions WHERE state = 'open' AND LOWER(name) = LOWER(?1))",
                params![name],
                |r| r.get(0),
            )?;
            if waiting {
                return refused(Refusal::Conflict, format!("\"{name}\" has already been suggested and is waiting for an admin."));
            }
            let open: i64 = conn.query_row(
                "SELECT COUNT(*) FROM media_category_suggestions WHERE suggester_key = ?1 AND state = 'open'",
                params![key],
                |r| r.get(0),
            )?;
            if open >= OPEN_SUGGESTIONS_PER_PERSON {
                return refused(
                    Refusal::Full,
                    format!("You have {open} suggestions waiting for an admin, the most at once. Wait for a decision first."),
                );
            }
            conn.execute(
                "INSERT INTO media_category_suggestions (suggester_key, name, note, created_at, state) VALUES (?1, ?2, ?3, ?4, 'open')",
                params![key, name, clipped(note, SUGGESTION_NOTE_MAX), now_ms()],
            )?;
            Ok(conn.last_insert_rowid())
        })?;
        Ok(self.media_suggestion(id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?)
    }

    fn media_suggestion(&self, id: i64) -> Result<Option<MediaSuggestion>, rusqlite::Error> {
        Ok(self.suggestions_where(None, Some(id))?.pop())
    }

    /// The suggestions, open ones first, then the newest: everyone's, or `only`'s own.
    pub fn media_suggestions(&self, only: Option<&str>) -> Result<Vec<MediaSuggestion>, rusqlite::Error> {
        self.suggestions_where(only, None)
    }

    fn suggestions_where(&self, only: Option<&str>, id: Option<i64>) -> Result<Vec<MediaSuggestion>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id, s.suggester_key, COALESCE(sm.name, ''), s.name, s.note, s.created_at, s.state
                 FROM media_category_suggestions s
                 LEFT JOIN server_members sm ON sm.public_key = s.suggester_key
                 WHERE (?1 IS NULL OR s.suggester_key = ?1) AND (?2 IS NULL OR s.id = ?2)
                 ORDER BY (s.state = 'open') DESC, s.id DESC LIMIT 500",
            )?;
            let rows = stmt
                .query_map(params![only, id], |r| {
                    Ok(MediaSuggestion {
                        id: r.get(0)?,
                        suggester_key: r.get(1)?,
                        suggester_name: r.get(2)?,
                        name: r.get(3)?,
                        note: r.get(4)?,
                        created_at: r.get(5)?,
                        state: r.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// An admin decides a suggestion. Accepting adds the category at the end of the list (unless
    /// one by that name exists by now); returns the suggestion as decided and the category it
    /// became. Refused: no such suggestion, or one already decided.
    pub fn decide_media_suggestion(&self, id: i64, accept: bool) -> Result<(MediaSuggestion, Option<MediaCategory>), MediaError> {
        let category_id = self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let found: Option<(String, String)> = tx
                .query_row("SELECT name, state FROM media_category_suggestions WHERE id = ?1", params![id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            let Some((name, state)) = found else {
                return refused(Refusal::NotFound, "There is no suggestion with that number.");
            };
            if state != "open" {
                return refused(Refusal::Conflict, format!("That suggestion was already {state}."));
            }
            tx.execute(
                "UPDATE media_category_suggestions SET state = ?1 WHERE id = ?2",
                params![if accept { "accepted" } else { "declined" }, id],
            )?;
            let mut category = None;
            if accept {
                let existing: Option<i64> = tx
                    .query_row("SELECT id FROM media_categories WHERE LOWER(name) = LOWER(?1)", params![name], |r| r.get(0))
                    .optional()?;
                category = Some(match existing {
                    Some(c) => c,
                    None => {
                        let count: i64 = tx.query_row("SELECT COUNT(*) FROM media_categories", [], |r| r.get(0))?;
                        if count as usize >= CATEGORIES_MAX {
                            return refused(Refusal::Full, format!("This server already has {CATEGORIES_MAX} categories, the most it can have."));
                        }
                        let position: i64 =
                            tx.query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM media_categories", [], |r| r.get(0))?;
                        tx.execute(
                            "INSERT INTO media_categories (name, description, position) VALUES (?1, '', ?2)",
                            params![name, position],
                        )?;
                        tx.last_insert_rowid()
                    }
                });
            }
            tx.commit()?;
            Ok(category)
        })?;
        let suggestion = self.media_suggestion(id)?.ok_or(MediaError::Db(rusqlite::Error::QueryReturnedNoRows))?;
        let category = match category_id {
            Some(c) => self.media_categories()?.into_iter().find(|x| x.id == c),
            None => None,
        };
        Ok((suggestion, category))
    }

    /// The numbers an admin prunes by: bytes used, each person's use (largest first), and the
    /// largest, oldest and least viewed items.
    pub fn media_usage(&self) -> Result<MediaUsage, rusqlite::Error> {
        self.with_conn(|conn| {
            let used_bytes = media_bytes_on(conn, None, None)?;
            let items: i64 = conn.query_row("SELECT COUNT(*) FROM media_items", [], |r| r.get(0))?;
            let mut stmt = conn.prepare(
                "SELECT u.public_key, COALESCE(sm.name, ''), SUM(u.size_bytes),
                        (SELECT COUNT(*) FROM media_items m WHERE m.uploader_key = u.public_key)
                 FROM user_uploads u
                 LEFT JOIN server_members sm ON sm.public_key = u.public_key
                 WHERE u.shared = ?1 OR u.filename IN (SELECT upload_filename FROM media_items)
                 GROUP BY u.public_key ORDER BY SUM(u.size_bytes) DESC, u.public_key ASC",
            )?;
            let people = stmt
                .query_map(params![MEDIA_SHELF], |r| Ok(PersonUse { key: r.get(0)?, name: r.get(1)?, bytes: r.get(2)?, items: r.get(3)? }))?
                .collect::<Result<Vec<_>, _>>()?;
            // Waiting to be published (F1): on the Media shelf, no item, oldest first.
            let mut stmt = conn.prepare(
                "SELECT u.filename, u.public_key, COALESCE(sm.name, ''), u.size_bytes, u.uploaded_at
                 FROM user_uploads u
                 LEFT JOIN server_members sm ON sm.public_key = u.public_key
                 WHERE u.shared = ?1 AND u.filename NOT IN (SELECT upload_filename FROM media_items)
                 ORDER BY u.uploaded_at ASC, u.id ASC LIMIT 500",
            )?;
            let waiting = stmt
                .query_map(params![MEDIA_SHELF], |r| {
                    let uploaded_at: i64 = r.get(4)?;
                    Ok(WaitingUpload {
                        filename: r.get(0)?,
                        uploader_key: r.get(1)?,
                        uploader_name: r.get(2)?,
                        size_bytes: r.get(3)?,
                        uploaded_at,
                        expires_at: uploaded_at + UNPUBLISHED_KEEP_MS,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(MediaUsage {
                used_bytes,
                items,
                people,
                largest: items_ordered(conn, "m.size_bytes DESC, m.id ASC", USAGE_LIST_LEN)?,
                oldest: items_ordered(conn, "m.created_at ASC, m.id ASC", USAGE_LIST_LEN)?,
                least_viewed: items_ordered(conn, "m.views ASC, m.created_at ASC, m.id ASC", USAGE_LIST_LEN)?,
                waiting,
            })
        })
    }
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
