//! Hide and keep, the storage half (2026-11-20, docs/design/hide-and-keep.md; the law it answers
//! to is docs/reference/findings/2026-11-20-csam-reporting.md). The handler that decides it is
//! handlers/held.rs; a report's `hide_and_keep` decision (handlers/reports.rs) calls it.
//!
//! WHY. In the United States a provider that reports child sexual abuse material to the
//! CyberTipline must keep what it reported for a year, securely, with as few people able to reach
//! it as possible (18 U.S.C. 2258A(h)). "Delete the post" keeps nothing, and the ordinary Delete
//! destroys the files the post linked. Hide and keep takes a reported post out of every view at
//! once and keeps it where only the server's OWNER can ever open it.
//!
//! WHO THE OWNER IS. The one key holding the role `owner` here ([`Storage::owner_card`]). No path
//! in the app grants that role (a signed `grant_role` refuses it, `/mod` grants `mod`): it is set by
//! whoever runs the server. Keeping is REFUSED, and nothing is kept, when there is no owner, when
//! more than one key holds the role, and when the owner has no key card here that checks out
//! ([`OwnerCard`]): a server that cannot seal never keeps a readable copy instead (the operator's
//! limit 2, 2026-11-20).
//!
//! WHAT HIDING ONE POST DOES ([`Storage::hide_post_in`]): the post's row, any pinned copy of it,
//! the `user_uploads` rows of the files it links (so no list shows them and no limit or sweep
//! touches them) and the files themselves leave this server's views. Every folder a route serves
//! is one of [`Folders`]' three, and a file is taken from all three, with a video's poster picture.
//! The handler then tells every client to drop the post and sends the delete to linked servers.
//! An encrypted file (`.enc`, a direct message's or a group's attachment) is never touched: a
//! public post never carries one, and hide and keep never reaches a sealed message (limit 6).
//!
//! WHAT IS KEPT (rung 2): a PACKAGE under the held folder, `<held>/<package>/`, where `package` is
//! 32 random hex digits. Part 0 is the post's words and the list of its files (core `held_seal::
//! Manifest`), then one part per file and per video poster picture, each sealed to the owner's card
//! by core `held_seal` (the "Held copies" row of docs/reference/cryptography.md). Every part is
//! sealed into a staging folder (`.staging-<package>`, never served, removed on any failure) from
//! the original a piece at a time, so no plaintext copy is ever written; only when every part is
//! written and synced does the folder take its name, the post's rows go in one transaction, and the
//! originals are deleted. A row of `held_items` records the package, the report, who and when, the
//! delete time ([`HELD_KEEP_DAYS`]) and nothing readable about the content: the files' names only
//! as keyed BLAKE3 hashes ([`name_hash`]), so the expiry pass can take out a held file a crash left
//! in a served folder ([`Storage::held_sweep_left_behind_in`]). The account erase and export leave
//! `held_items` alone: what was reported is kept for its year whoever asks (2258A(h)).
//!
//! HOW LONG (rung 3): [`HELD_KEEP_DAYS`] after it was held, then the expiry pass deletes the
//! package and the row ([`Storage::held_expire_in`]). A LEGAL HOLD (the owner, with its own end
//! date, at most [`LEGAL_HOLD_MAX_DAYS`] ahead and renewable) keeps it past its year while a
//! preservation request runs (18 U.S.C. 2703(f)); DESTROY NOW (the owner) deletes it early when law
//! enforcement asks (2258B(c)(2)) ([`Storage::held_destroy_in`]). The report that held it is kept as
//! long as any of its items are (storage/reports.rs: the 90-day cull and `/reports-clear` pass it
//! over).

use std::path::{Path, PathBuf};

use humanity_protocol::held_seal::{self, HeldFile, Manifest, MANIFEST_VERSION};
use humanity_protocol::key_card;
use rusqlite::{params, OptionalExtension};

use super::files::Folders;
use super::Storage;

/// The folder held copies are kept in, beside the uploads folder: `data/held` on a live relay, a
/// test's own temporary folder in a test. Nothing serves it.
pub const HELD_DIR: &str = "held";

/// How long a held item is kept: a year after it was held, the period 18 U.S.C. 2258A(h)(1) sets
/// after a CyberTipline report. A legal hold keeps it longer; destroy now ends it early.
pub const HELD_KEEP_DAYS: i64 = 365;
/// A day in milliseconds.
pub const DAY_MS: i64 = 86_400_000;
/// The furthest ahead one legal hold may end; a longer preservation is a hold placed again.
pub const LEGAL_HOLD_MAX_DAYS: i64 = 366;

/// Why a legal hold ending at `until_ms` is refused at `now_ms`, or None (0 lifts a hold).
pub fn legal_hold_refusal(until_ms: i64, now_ms: i64) -> Option<String> {
    if until_ms == 0 {
        return None;
    }
    if until_ms <= now_ms {
        return Some("A legal hold ends after today: choose a later end date.".into());
    }
    if until_ms > now_ms + LEGAL_HOLD_MAX_DAYS * DAY_MS {
        return Some(format!("A legal hold ends at most {LEGAL_HOLD_MAX_DAYS} days ahead; place it again before it ends to keep the item longer."));
    }
    None
}

/// One held item as an admin may see it: when it was held and by whom, how big, when it is
/// deleted and any legal hold. Nothing of what it contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldRow {
    pub id: i64,
    pub package: String,
    pub report_id: i64,
    pub author_key: String,
    pub stamp: i64,
    pub channel: String,
    pub file_count: i64,
    pub parts: i64,
    pub sealed_bytes: i64,
    /// The owner key its parts are sealed to, and that key card's time.
    pub sealed_to: String,
    pub card_at: i64,
    pub held_by: String,
    pub held_at: i64,
    pub delete_after: i64,
    /// 0 when no legal hold runs.
    pub legal_hold_until: i64,
    pub exported_count: i64,
}

impl HeldRow {
    /// When the expiry pass deletes it: its year or its legal hold's end, whichever is later.
    pub fn deleted_at(&self) -> i64 {
        self.delete_after.max(self.legal_hold_until)
    }
}

const HELD_COLUMNS: &str = "id, package, report_id, author_key, stamp, channel, file_count, parts, sealed_bytes, sealed_to, card_at, held_by, held_at, delete_after, legal_hold_until, exported_count";

fn held_row(r: &rusqlite::Row) -> rusqlite::Result<HeldRow> {
    Ok(HeldRow {
        id: r.get(0)?,
        package: r.get(1)?,
        report_id: r.get(2)?,
        author_key: r.get(3)?,
        stamp: r.get(4)?,
        channel: r.get(5)?,
        file_count: r.get(6)?,
        parts: r.get(7)?,
        sealed_bytes: r.get(8)?,
        sealed_to: r.get(9)?,
        card_at: r.get(10)?,
        held_by: r.get(11)?,
        held_at: r.get(12)?,
        delete_after: r.get(13)?,
        legal_hold_until: r.get(14)?,
        exported_count: r.get(15)?,
    })
}
/// The start of a package's folder name while its parts are being sealed.
pub const STAGING_PREFIX: &str = ".staging-";

/// The name a sealed part is kept under in its package's folder.
pub fn part_file(part: usize) -> String {
    format!("{part}.sealed")
}

/// A held file's stored name as `held_items` keeps it: BLAKE3 `derive_key("hum/held-name/v1",
/// name)`, hex. Enough to recognise the file left in a served folder, not to read its name back.
pub fn name_hash(name: &str) -> String {
    hex::encode(blake3::derive_key("hum/held-name/v1", name.as_bytes()))
}

/// Who keeps a copy and for whom: the owner's checked card ([`OwnerCard::Ready`]), who hid it and
/// when.
#[derive(Debug, Clone, Copy)]
pub struct Keeper<'a> {
    pub owner: &'a str,
    pub kyber: &'a [u8],
    pub card_at: u64,
    pub held_by: &'a str,
    pub now_ms: i64,
}

/// What sealing a package wrote: how many parts and how many bytes.
struct Sealed {
    parts: usize,
    bytes: u64,
}

/// Seal the file at `src` into `dst` as part `context`, reading it a piece at a time, and sync it
/// to the disk; the bytes written.
fn seal_file(src: &Path, dst: &Path, kyber: &[u8], context: &str) -> Result<u64, String> {
    use std::io::Write;
    let input = std::fs::File::open(src).map_err(|e| format!("a file could not be read ({e})"))?;
    let out = std::fs::File::create(dst).map_err(|e| format!("a sealed part could not be written ({e})"))?;
    let mut w = std::io::BufWriter::new(out);
    let n = held_seal::seal_stream(kyber, context, input, &mut w)?;
    w.flush().map_err(|e| format!("a sealed part could not be written ({e})"))?;
    let out = w.into_inner().map_err(|e| format!("a sealed part could not be written ({e})"))?;
    out.sync_all().map_err(|e| format!("a sealed part could not be written ({e})"))?;
    Ok(n)
}

/// The held folder beside `folders`' uploads folder.
pub fn held_dir_beside(folders: Folders) -> PathBuf {
    folders.uploads.parent().unwrap_or_else(|| Path::new(".")).join(HELD_DIR)
}

/// The words an admin is told when keeping is refused because there is no owner.
pub const NO_OWNER: &str = "This server has no owner set, so there is nobody a copy could be sealed to. Nothing was kept. \"Delete the post\" takes it out of view without keeping anything.";
/// ... because more than one key holds the owner role.
pub const MANY_OWNERS: &str = "More than one key holds this server's owner role, so a copy could not be sealed to one owner. Nothing was kept. \"Delete the post\" takes it out of view without keeping anything.";
/// ... because the owner has no key card here (the design's sentence, then what works instead).
pub const NO_CARD: &str = "This server's owner has no key card here, so a copy could not be kept sealed. Nothing was kept. \"Delete the post\" takes it out of view without keeping anything. The owner's key card arrives the next time the owner signs in with the app.";
/// ... because the card held for the owner does not check out.
pub const BAD_CARD: &str = "The key card held for this server's owner does not check out, so a copy could not be kept sealed. Nothing was kept. \"Delete the post\" takes it out of view without keeping anything.";

/// Whom a held copy can be sealed to: the server owner's checked key card, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerCard {
    /// The owner's key (lowercase hex), the Kyber768 key their own card carries, and the card's time.
    Ready { owner: String, kyber: Vec<u8>, card_at: u64 },
    /// No key holds the role `owner`.
    NoOwner,
    /// This many keys hold it.
    ManyOwners(usize),
    /// The owner has no key card here.
    NoCard,
    /// The card held for the owner is not a good card for them.
    BadCard,
}

impl OwnerCard {
    /// Why keeping is refused, in words an admin can act on; None when a copy can be sealed.
    pub fn refusal(&self) -> Option<&'static str> {
        match self {
            OwnerCard::Ready { .. } => None,
            OwnerCard::NoOwner => Some(NO_OWNER),
            OwnerCard::ManyOwners(_) => Some(MANY_OWNERS),
            OwnerCard::NoCard => Some(NO_CARD),
            OwnerCard::BadCard => Some(BAD_CARD),
        }
    }
}

/// What hiding one post did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenPost {
    /// Its `held_items` row.
    pub id: i64,
    /// The room it was in ('' read as the main room), for the linked servers' delete.
    pub channel: String,
    /// How many files it linked that were taken out of view.
    pub files: usize,
    /// Files that could not be taken out of a served folder (logged; the admin is told).
    pub left_in_view: Vec<String>,
    /// Whether a pinned copy went too (the room's pins are sent again).
    pub unpinned: bool,
}

/// Why a post could not be hidden; nothing was changed.
#[derive(Debug)]
pub enum HideError {
    Db(rusqlite::Error),
    /// Sealing failed (a file unreadable, the disk full): nothing was kept and nothing hidden.
    Seal(String),
}

impl From<rusqlite::Error> for HideError {
    fn from(e: rusqlite::Error) -> Self {
        HideError::Db(e)
    }
}

impl std::fmt::Display for HideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HideError::Db(e) => write!(f, "the server's database refused it ({e})"),
            HideError::Seal(e) => write!(f, "a sealed copy could not be kept: {e}"),
        }
    }
}

/// The stored names of the files a post's words link that are on this server now, each once,
/// never an encrypted file.
fn files_of(content: &str, folders: Folders) -> Vec<String> {
    super::uploads::upload_names_in(content)
        .into_iter()
        .filter(|n| !n.to_ascii_lowercase().ends_with(".enc"))
        .filter(|n| folders.spot_of(n).is_some())
        .collect()
}

impl Storage {
    /// Seal the post's words and the files `names` into `dir` as package `package` (the module's
    /// header), to `keeper`'s card.
    #[allow(clippy::too_many_arguments)]
    fn seal_package(&self, dir: &Path, package: &str, report_id: i64, author: &str, stamp: u64, channel: &str, words: &str, names: &[String], keeper: &Keeper, folders: Folders) -> Result<Sealed, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("the held folder could not be made ({e})"))?;
        let (mut files, mut part, mut bytes) = (Vec::new(), 1usize, 0u64);
        for name in names {
            let spot = folders.spot_of(name).ok_or_else(|| format!("{name} left its folder while it was being sealed"))?;
            let src = folders.dir(spot).join(name);
            let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
            let (uploader, shelf, original_name) = self.upload_row_of(name).unwrap_or_default();
            bytes += seal_file(&src, &dir.join(part_file(part)), keeper.kyber, &held_seal::part_context(package, part))?;
            let file_part = part;
            part += 1;
            let poster = folders.dir(spot).join(super::files::poster_name(name));
            let poster_part = if poster.is_file() {
                bytes += seal_file(&poster, &dir.join(part_file(part)), keeper.kyber, &held_seal::part_context(package, part))?;
                part += 1;
                Some(part - 1)
            } else {
                None
            };
            let folder = match spot {
                super::files::Spot::Public => "public",
                super::files::Spot::Members => "members",
                super::files::Spot::Waiting => "waiting",
            };
            files.push(HeldFile { part: file_part, name: name.clone(), folder: folder.into(), uploader, shelf, original_name, bytes: size, poster_part });
        }
        let manifest = Manifest {
            v: MANIFEST_VERSION,
            package: package.to_string(),
            report_id,
            author: author.to_string(),
            stamp,
            channel: channel.to_string(),
            words: words.to_string(),
            files,
            held_by: keeper.held_by.to_string(),
            held_at: keeper.now_ms,
        };
        let json = serde_json::to_vec(&manifest).map_err(|e| format!("the list of files could not be written ({e})"))?;
        let sealed = held_seal::seal_bytes(keeper.kyber, &held_seal::part_context(package, 0), &json)?;
        let path = dir.join(part_file(0));
        // Written and synced through one handle: a handle opened only to read cannot be synced on
        // Windows ("Access is denied").
        std::fs::File::create(&path)
            .and_then(|mut f| {
                std::io::Write::write_all(&mut f, &sealed)?;
                f.sync_all()
            })
            .map_err(|e| format!("a sealed part could not be written ({e})"))?;
        Ok(Sealed { parts: part, bytes: bytes + sealed.len() as u64 })
    }

    /// Who uploaded `name`, on which shelf, under what name: from its upload row, if it has one.
    fn upload_row_of(&self, name: &str) -> Option<(String, i64, String)> {
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT public_key, shared, original_name FROM user_uploads WHERE filename = ?1 LIMIT 1",
                params![name],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    /// Take out of every served folder in `folders` any file a held item holds that a crash left
    /// there (the relay stopped between the transaction and the delete of the originals). How many
    /// went. The expiry pass runs it, so it runs at every start too.
    pub fn held_sweep_left_behind_in(&self, folders: Folders) -> Result<usize, rusqlite::Error> {
        let lists: Vec<String> = self.with_read_conn(|conn| {
            let mut st = conn.prepare("SELECT name_hashes FROM held_items")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect()
        })?;
        let held: std::collections::HashSet<String> = lists.iter().flat_map(|l| serde_json::from_str::<Vec<String>>(l).unwrap_or_default()).collect();
        if held.is_empty() {
            return Ok(0);
        }
        let mut gone = 0;
        for dir in [folders.uploads, folders.members, folders.waiting] {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for e in entries.filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().into_owned();
                if held.contains(&name_hash(&name)) && folders.delete(&name) {
                    tracing::warn!("Hide and keep: took a held file left in a served folder out of it");
                    gone += 1;
                }
            }
        }
        Ok(gone)
    }

    /// Every held item, oldest first: what an admin may see of it (no contents).
    pub fn held_items(&self) -> Result<Vec<HeldRow>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut st = conn.prepare(&format!("SELECT {HELD_COLUMNS} FROM held_items ORDER BY id"))?;
            let rows = st.query_map([], held_row)?;
            rows.collect()
        })
    }

    /// The held item `id`, if any.
    pub fn held_item(&self, id: i64) -> Result<Option<HeldRow>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            conn.query_row(&format!("SELECT {HELD_COLUMNS} FROM held_items WHERE id = ?1"), params![id], held_row).optional()
        })
    }

    /// Place a legal hold on item `id` until `until_ms` (a preservation request, 18 U.S.C.
    /// 2703(f): 90 days, and 90 more if asked again), or lift it (`until_ms` 0). The item is then
    /// deleted at whichever is later, its year or the hold's end. Refused (Ok(false)) for no such
    /// item; an end that is not after `now_ms`, or more than [`LEGAL_HOLD_MAX_DAYS`] ahead, is the
    /// caller's to refuse ([`legal_hold_refusal`]).
    pub fn held_set_legal_hold(&self, id: i64, until_ms: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("UPDATE held_items SET legal_hold_until = ?2 WHERE id = ?1", params![id, until_ms]).map(|n| n > 0))
    }

    /// Delete held item `id` now, its sealed parts and its row (destroy now: law enforcement asked,
    /// 18 U.S.C. 2258B(c)(2); or the year and any hold are up). Ok(false) for no such item. The row
    /// stays when the parts could not be removed, so nothing is forgotten while its bytes remain.
    pub fn held_destroy_in(&self, id: i64, folders: Folders) -> Result<bool, rusqlite::Error> {
        let Some(row) = self.held_item(id)? else { return Ok(false) };
        let dir = held_dir_beside(folders).join(&row.package);
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::error!("Held items: could not delete item #{id}'s sealed copy: {e}");
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e)));
            }
        }
        self.with_conn(|conn| conn.execute("DELETE FROM held_items WHERE id = ?1", params![id]).map(|n| n > 0))
    }

    /// The expiry pass's part: delete every held item whose year is up at `now_ms` and that has no
    /// legal hold running then. How many went.
    pub fn held_expire_in(&self, now_ms: i64, folders: Folders) -> Result<usize, rusqlite::Error> {
        let due: Vec<i64> = self.with_read_conn(|conn| {
            let mut st = conn.prepare("SELECT id FROM held_items WHERE delete_after <= ?1 AND legal_hold_until <= ?1")?;
            let ids = st.query_map(params![now_ms], |r| r.get::<_, i64>(0))?;
            ids.collect()
        })?;
        let mut gone = 0;
        for id in due {
            match self.held_destroy_in(id, folders) {
                Ok(true) => gone += 1,
                Ok(false) => {}
                Err(e) => tracing::error!("Held items: item #{id} is due but could not be deleted: {e}"),
            }
        }
        Ok(gone)
    }

    /// Count one export of held item `id`.
    pub fn held_count_export(&self, id: i64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("UPDATE held_items SET exported_count = exported_count + 1 WHERE id = ?1", params![id]).map(|n| n > 0))
    }

    /// What the held folder beside `folders` holds that no row accounts for: a staging folder a
    /// seal left when the relay stopped mid-way, last changed before `stale_before` (removed: it was
    /// never put in place, so nothing was hidden for it), and a package folder with no row (KEPT and
    /// counted, never deleted: a restore from a backup older than the hide, or a stop between the
    /// rename and the transaction, and it may be a copy the law says to keep). (staging folders
    /// removed, packages with no row).
    pub fn held_sweep_strays_in(&self, folders: Folders, stale_before: std::time::SystemTime) -> Result<(usize, usize), rusqlite::Error> {
        let Ok(entries) = std::fs::read_dir(held_dir_beside(folders)) else { return Ok((0, 0)) };
        let packages: std::collections::HashSet<String> = self.with_read_conn(|conn| {
            let mut st = conn.prepare("SELECT package FROM held_items")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect()
        })?;
        let (mut staging, mut orphans) = (0, 0);
        for e in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(STAGING_PREFIX) {
                let stale = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < stale_before);
                if stale && std::fs::remove_dir_all(e.path()).is_ok() {
                    staging += 1;
                }
            } else if !packages.contains(&name) {
                orphans += 1;
            }
        }
        Ok((staging, orphans))
    }

    /// Every key holding the role `owner`, sorted.
    pub fn owner_keys(&self) -> Result<Vec<String>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut st = conn.prepare("SELECT public_key FROM user_roles WHERE role = 'owner' ORDER BY public_key")?;
            let keys = st.query_map([], |r| r.get::<_, String>(0))?;
            keys.collect()
        })
    }

    /// The server owner's key card, checked here (signed by the very key it names, and naming the
    /// owner), or why a copy cannot be sealed. One Dilithium3 verify.
    pub fn owner_card(&self) -> OwnerCard {
        let owners = match self.owner_keys() {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Hide and keep: could not read the owner role: {e}");
                return OwnerCard::NoOwner;
            }
        };
        let owner = match owners.as_slice() {
            [] => return OwnerCard::NoOwner,
            [one] => one.clone(),
            many => return OwnerCard::ManyOwners(many.len()),
        };
        let card = match self.get_key_card(&owner) {
            Ok(Some(c)) => c,
            Ok(None) => return OwnerCard::NoCard,
            Err(e) => {
                tracing::error!("Hide and keep: could not read the owner's key card: {e}");
                return OwnerCard::NoCard;
            }
        };
        if !key_card::card_for(&card, &owner) {
            return OwnerCard::BadCard;
        }
        match key_card::kyber_bytes(&card) {
            Some(kyber) => OwnerCard::Ready { owner, kyber, card_at: card.at },
            None => OwnerCard::BadCard,
        }
    }

    /// Hide and keep the post `author` sent at `stamp` for report `report_id` (the module's
    /// header): seal its words and every file it links to `keeper`'s card into a new package under
    /// the held folder beside `folders`' uploads, then take the post's row, a pinned copy and the
    /// files' upload rows out in one transaction, then the files from every served folder. None
    /// when this server has no such post (its author deleted it, or it expired): nothing done. A
    /// seal that fails (the disk full, a file unreadable) changes nothing: the sealed parts
    /// written so far are removed and the post stays where it was.
    pub fn hide_post_in(&self, report_id: i64, author: &str, stamp: u64, keeper: &Keeper, folders: Folders) -> Result<Option<HiddenPost>, HideError> {
        let Some((content, channel)) = self.post_for_report(author, stamp)? else { return Ok(None) };
        let names = files_of(&content, folders);
        let package = hex::encode(rand::random::<[u8; 16]>());
        let held = held_dir_beside(folders);
        let staging = held.join(format!("{STAGING_PREFIX}{package}"));
        let sealed = match self.seal_package(&staging, &package, report_id, author, stamp, &channel, &content, &names, keeper, folders) {
            Ok(s) => s,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(HideError::Seal(e));
            }
        };
        let home = held.join(&package);
        if let Err(e) = std::fs::rename(&staging, &home) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(HideError::Seal(format!("the sealed copy could not be put in place ({e})")));
        }
        let hashes: Vec<String> = names.iter().map(|n| name_hash(n)).collect();
        let row = self.with_conn_mut(|conn| -> Result<(i64, bool), rusqlite::Error> {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO held_items (package, report_id, author_key, stamp, channel, file_count, parts, sealed_bytes,
                    sealed_to, card_at, name_hashes, held_by, held_at, delete_after)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    package, report_id, author, stamp as i64, channel, names.len() as i64, sealed.parts as i64, sealed.bytes as i64,
                    keeper.owner, keeper.card_at as i64, serde_json::to_string(&hashes).unwrap_or_else(|_| "[]".into()),
                    keeper.held_by, keeper.now_ms, keeper.now_ms + HELD_KEEP_DAYS * DAY_MS
                ],
            )?;
            let id = tx.last_insert_rowid();
            for name in &names {
                tx.execute("DELETE FROM user_uploads WHERE filename = ?1", params![name])?;
            }
            let pins = tx.execute(
                "DELETE FROM pinned_messages WHERE from_key = ?1 AND original_timestamp = ?2",
                params![author, stamp as i64],
            )?;
            tx.execute("DELETE FROM messages WHERE from_key = ?1 AND timestamp = ?2", params![author, stamp as i64])?;
            tx.commit()?;
            Ok((id, pins > 0))
        });
        let (id, unpinned) = match row {
            Ok(r) => r,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&home);
                return Err(HideError::Db(e));
            }
        };
        let mut left_in_view = Vec::new();
        for name in &names {
            folders.delete(name);
            if folders.find(name).is_some() {
                tracing::error!("Hide and keep: {name} could not be taken out of a served folder");
                left_in_view.push(name.clone());
            }
        }
        Ok(Some(HiddenPost { id, channel, files: names.len(), left_in_view, unpinned }))
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
