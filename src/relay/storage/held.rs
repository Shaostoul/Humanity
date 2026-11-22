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
    /// The successor the owner named, whom its parts are also sealed to (storage/owner.rs), or empty.
    pub also_sealed_to: String,
    pub held_by: String,
    pub held_at: i64,
    pub delete_after: i64,
    /// 0 when no legal hold runs.
    pub legal_hold_until: i64,
    pub exported_count: i64,
}

impl HeldRow {
    /// Whether `key`'s device can open its parts: the owner it was sealed to, or the successor.
    pub fn opens_for(&self, key: &str) -> bool {
        !key.is_empty() && (self.sealed_to == key || self.also_sealed_to == key)
    }

    /// How many people its parts are sealed to.
    pub fn recipients(&self) -> usize {
        1 + usize::from(!self.also_sealed_to.is_empty())
    }

    /// When the expiry pass deletes it: its year or its legal hold's end, whichever is later.
    pub fn deleted_at(&self) -> i64 {
        self.delete_after.max(self.legal_hold_until)
    }
}

const HELD_COLUMNS: &str = "id, package, report_id, author_key, stamp, channel, file_count, parts, sealed_bytes, sealed_to, card_at, held_by, held_at, delete_after, legal_hold_until, exported_count, also_sealed_to";

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
        also_sealed_to: r.get(16)?,
    })
}
/// The start of a package's folder name while its parts are being sealed.
pub const STAGING_PREFIX: &str = ".staging-";
/// The start of a WAITING hide's folder (the owner branch's third review, S3, 2026-11-21): a post
/// hidden while an owner change is pending or being carried out is taken out of view at once, but
/// its sealed copy waits until the change is carried out or rolled back, then is sealed to whoever
/// the recipients are then ([`Storage::held_seal_waiting_in`]). Until then its files and a note of
/// the post (`post.json`: its words, room, author, time, report, file list and upload rows) wait
/// here, in the held folder nothing serves, exactly as readable on the server's disk as the post and
/// its files were before the hide (the plaintext had always been there; it is not sealed yet).
/// Why wait: a copy sealed to the old recipients and committed after the carry-out could not be
/// opened by the new owner, and a held row added during phase 2 rolled the change back.
pub const WAITING_PREFIX: &str = ".waiting-";
/// A waiting hide's note of the post.
const WAITING_NOTE: &str = "post.json";

/// What a waiting hide keeps of the post until its copy is sealed ([`WAITING_PREFIX`]).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WaitingPost {
    report_id: i64,
    author: String,
    stamp: u64,
    channel: String,
    words: String,
    names: Vec<String>,
    /// Each file's upload row as it was (stored name, uploader, shelf, original name): the rows go
    /// with the hide, and the sealed manifest names them.
    rows: Vec<(String, String, i64, String)>,
    held_by: String,
    held_at: i64,
}

/// The folders of a waiting hide at `dir`, shaped like the served ones so the seal reads them the
/// same way.
fn waiting_folders(dir: &Path) -> [PathBuf; 4] {
    [dir.join("uploads"), dir.join("members"), dir.join("resume"), dir.join("waiting")]
}

/// The name a sealed part is kept under in its package's folder.
pub fn part_file(part: usize) -> String {
    format!("{part}.sealed")
}

/// Every folder a copy of `package` can be in: its own, an owner change's re-wrapped copy, and the
/// original a change's swap set aside (storage/owner_handover.rs).
pub fn package_dirs(folders: Folders, package: &str) -> [PathBuf; 3] {
    use super::owner::handover::{BEFORE, STAGING};
    let held = held_dir_beside(folders);
    [held.join(package), held.join(format!("{STAGING}{package}")), held.join(format!("{BEFORE}{package}"))]
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
    /// The successor the owner named (their key, and the Kyber768 key their checked card carries),
    /// whom every part is also sealed to.
    pub successor: Option<(&'a str, &'a [u8])>,
    pub held_by: &'a str,
    pub now_ms: i64,
}

/// What sealing a package wrote: how many parts and how many bytes.
struct Sealed {
    parts: usize,
    bytes: u64,
}

/// A reader that counts and hashes (BLAKE3) what passes through it: a file's hash and size for the
/// restore plan, taken in the same single read that seals it.
pub(crate) struct Hashing<R> {
    pub inner: R,
    pub hasher: blake3::Hasher,
    pub bytes: u64,
}

impl<R: std::io::Read> Hashing<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, hasher: blake3::Hasher::new(), bytes: 0 }
    }
}

impl<R: std::io::Read> std::io::Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}

/// What sealing one file wrote: the sealed bytes, and the original's size and BLAKE3 hash (hex).
struct SealedFile {
    sealed: u64,
    plain: u64,
    hash: String,
}

/// Seal the file at `src` into `dst` as part `context`, reading it a piece at a time, and sync it
/// to the disk.
fn seal_file(src: &Path, dst: &Path, kyber: &[&[u8]], context: &str) -> Result<SealedFile, String> {
    use std::io::Write;
    let input = std::fs::File::open(src).map_err(|e| format!("a file could not be read ({e})"))?;
    let mut input = Hashing::new(input);
    let out = std::fs::File::create(dst).map_err(|e| format!("a sealed part could not be written ({e})"))?;
    let mut w = std::io::BufWriter::new(out);
    let n = held_seal::seal_stream(kyber, context, &mut input, &mut w)?;
    w.flush().map_err(|e| format!("a sealed part could not be written ({e})"))?;
    let out = w.into_inner().map_err(|e| format!("a sealed part could not be written ({e})"))?;
    out.sync_all().map_err(|e| format!("a sealed part could not be written ({e})"))?;
    Ok(SealedFile { sealed: n, plain: input.bytes, hash: input.hasher.finalize().to_hex().to_string() })
}

/// Write `bytes` to a new file at `path` and sync it through the one handle (a handle opened only
/// to read cannot be synced on Windows: "Access is denied").
fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::File::create(path)
        .and_then(|mut f| {
            std::io::Write::write_all(&mut f, bytes)?;
            f.sync_all()
        })
        .map_err(|e| format!("a sealed part could not be written ({e})"))
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
    /// The owner's key (lowercase hex), the Kyber768 key their own card carries, and the card's time;
    /// and the successor the owner named, when their card checks out here (their key and Kyber768 key).
    Ready { owner: String, kyber: Vec<u8>, card_at: u64, successor: Option<(String, Vec<u8>)> },
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
    fn seal_package(&self, dir: &Path, package: &str, report_id: i64, author: &str, stamp: u64, channel: &str, words: &str, names: &[String], keeper: &Keeper, folders: Folders, row_of: &dyn Fn(&str) -> Option<(String, i64, String)>) -> Result<Sealed, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("the held folder could not be made ({e})"))?;
        let mut to: Vec<&[u8]> = vec![keeper.kyber];
        to.extend(keeper.successor.map(|(_, k)| k));
        let to = to.as_slice();
        let (mut files, mut part, mut bytes) = (Vec::new(), 1usize, 0u64);
        for name in names {
            let spot = folders.spot_of(name).ok_or_else(|| format!("{name} left its folder while it was being sealed"))?;
            let src = folders.dir(spot).join(name);
            let (uploader, shelf, original_name) = row_of(name).unwrap_or_default();
            let sealed_file = seal_file(&src, &dir.join(part_file(part)), to, &held_seal::part_context(package, part))?;
            bytes += sealed_file.sealed;
            let file_part = part;
            part += 1;
            let poster = folders.dir(spot).join(super::files::poster_name(name));
            let (poster_part, poster_bytes, poster_hash) = if poster.is_file() {
                let p = seal_file(&poster, &dir.join(part_file(part)), to, &held_seal::part_context(package, part))?;
                bytes += p.sealed;
                part += 1;
                (Some(part - 1), p.plain, p.hash)
            } else {
                (None, 0, String::new())
            };
            let folder = match spot {
                super::files::Spot::Public => "public",
                super::files::Spot::Members => "members",
                super::files::Spot::Waiting => "waiting",
            };
            files.push(HeldFile {
                part: file_part,
                name: name.clone(),
                folder: folder.into(),
                uploader,
                shelf,
                original_name,
                bytes: sealed_file.plain,
                hash: sealed_file.hash,
                poster_part,
                poster_bytes,
                poster_hash,
            });
        }
        // The restore plan's key (held_seal's header): made here, written into part 0 (sealed to
        // the owner) and nowhere else, then dropped.
        let restore_key: [u8; 32] = rand::random();
        let mut manifest = Manifest {
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
            restore_key: String::new(),
        };
        let unwritten = |e: serde_json::Error| format!("the list of files could not be written ({e})");
        // The plan is the manifest without its own key, sealed under that key alone.
        let plan = held_seal::seal_restore_plan(&restore_key, package, &serde_json::to_vec(&manifest).map_err(unwritten)?)?;
        write_synced(&dir.join(held_seal::RESTORE_PLAN_FILE), &plan)?;
        manifest.restore_key = hex::encode(restore_key);
        let json = serde_json::to_vec(&manifest).map_err(unwritten)?;
        let sealed = held_seal::seal_bytes(to, &held_seal::part_context(package, 0), &json)?;
        write_synced(&dir.join(part_file(0)), &sealed)?;
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
        // Never inside an owner change's swap (the held folder's one lock, storage/owner_handover.rs).
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        // EVERY copy of the package: its folder, and an owner change's re-wrapped copy and the
        // original a swap set aside (storage/owner_handover.rs), so destroy now and expiry leave
        // nothing an owner change made (the owner review's finding 7, 2026-11-20).
        for dir in package_dirs(folders, &row.package) {
            if dir.exists() {
                if let Err(e) = std::fs::remove_dir_all(&dir) {
                    tracing::error!("Held items: could not delete a copy of item #{id} ({}): {e}", dir.display());
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e)));
                }
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
        use super::owner::handover::{BEFORE, MARKER, STAGING};
        let held = held_dir_beside(folders);
        // The marker is read, and every folder handled, under the held folder's one lock, so no owner
        // change swaps folders while the sweep looks at them (the second review, R4: it read the
        // marker once, then renamed and removed while a change could be part-way).
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let swapping = held.join(MARKER).exists();
        let (mut staging, mut orphans) = (0, 0);
        for e in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
            let name = e.file_name().to_string_lossy().into_owned();
            let stale = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < stale_before);
            if name.starts_with(STAGING_PREFIX) {
                if stale && std::fs::remove_dir_all(e.path()).is_ok() {
                    staging += 1;
                }
            } else if let Some(package) = name.strip_prefix(STAGING) {
                // An owner change's re-wrapped copy (storage/owner_handover.rs): removed once its
                // package is gone, or when no change could still be waiting for it (the owner
                // review's finding 7: the sweep used to skip every name starting with a dot).
                let lifetime = std::time::Duration::from_millis(super::owner::handover::STAGING_LIFETIME_MS as u64);
                let outlived = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().is_ok_and(|age| age > lifetime));
                if (!packages.contains(package) || outlived) && std::fs::remove_dir_all(e.path()).is_ok() {
                    staging += 1;
                }
            } else if let Some(package) = name.strip_prefix(BEFORE) {
                // An original a swap set aside. While a swap is marked, the next start's recovery
                // decides. Otherwise: when its package is back in place, or gone, it is a copy left
                // over (removed); when its package has a row and no folder, it is the ONLY copy, so
                // it is put back.
                if swapping {
                    continue;
                }
                let home = held.join(package);
                if packages.contains(package) && !home.exists() {
                    match std::fs::rename(e.path(), &home) {
                        Ok(()) => tracing::warn!("Held items: put held copy {package} back from where an owner change had set it aside"),
                        Err(err) => tracing::error!("Held items: held copy {package} is only in {} and could not be put back: {err}", e.path().display()),
                    }
                } else if std::fs::remove_dir_all(e.path()).is_ok() {
                    staging += 1;
                }
            } else if !name.starts_with('.') && !packages.contains(&name) {
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
            Some(kyber) => {
                // The successor (storage/owner.rs): sealed to as well when their card checks out; when it
                // no longer does, the copy is sealed to the owner alone and the list says so.
                let successor = self.successor().and_then(|s| {
                    let card = self.get_key_card(&s).ok().flatten().filter(|c| key_card::card_for(c, &s))?;
                    key_card::kyber_bytes(&card).map(|k| (s, k))
                });
                OwnerCard::Ready { owner, kyber, card_at: card.at, successor }
            }
            None => OwnerCard::BadCard,
        }
    }

    /// Hide and keep the post `author` sent at `stamp` for report `report_id` (the module's
    /// header), by `held_by` at `now_ms`: seal its words and every file it links to the owner's
    /// card (and the successor's, when one is named and checks out) into a new package under the
    /// held folder beside `folders`' uploads, then take the post's row, a pinned copy and the files'
    /// upload rows out in one transaction, then the files from every served folder. None when this
    /// server has no such post (its author deleted it, or it expired): nothing done. A seal that
    /// fails (the disk full, a file unreadable, no owner's card to seal to) changes nothing: the
    /// sealed parts written so far are removed and the post stays where it was.
    ///
    /// THE RECIPIENTS ARE READ UNDER THE HELD FOLDER'S ONE LOCK, AT SEALING TIME (the sixth review's
    /// M1, 2026-11-21): the round-6 hide sealed to the card its handler read when the hide began,
    /// before this lock, so an owner change carried out in between (possible once the moderator's
    /// connection dropped and the hide stopped being counted as sealing) left a copy sealed to the
    /// recipients of before. Now the seal, the put-in-place and the record all happen under the lock
    /// a carry-out also holds, with the card read inside it.
    pub fn hide_post_in(&self, report_id: i64, author: &str, stamp: u64, held_by: &str, now_ms: i64, folders: Folders) -> Result<Option<HiddenPost>, HideError> {
        #[cfg(test)]
        before_hide(&held_dir_beside(folders));
        // Read, sealed, put in place and recorded under the held folder's one lock (storage/
        // owner_handover.rs), the recipients read inside it. The POST is read inside it too (the
        // owner branch's seventh review, L3): two moderators deciding one report at once both read
        // the post before the lock and each sealed a copy, two held items for one post; now the
        // second finds it already taken out and does nothing.
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let Some((content, channel)) = self.post_for_report(author, stamp)? else { return Ok(None) };
        let names = files_of(&content, folders);
        let package = hex::encode(rand::random::<[u8; 16]>());
        let held = held_dir_beside(folders);
        let staging = held.join(format!("{STAGING_PREFIX}{package}"));
        let card = self.owner_card();
        let OwnerCard::Ready { owner, kyber, card_at, successor } = &card else {
            return Err(HideError::Seal(card.refusal().unwrap_or(NO_CARD).to_string()));
        };
        let successor = successor.as_ref().map(|(k, kyber)| (k.as_str(), kyber.as_slice()));
        let keeper = &Keeper { owner, kyber, card_at: *card_at, successor, held_by, now_ms };
        let sealed = match self.seal_package(&staging, &package, report_id, author, stamp, &channel, &content, &names, keeper, folders, &|n| self.upload_row_of(n)) {
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
        let row = self.with_conn_mut(|conn| -> Result<(i64, bool), rusqlite::Error> {
            let tx = conn.transaction()?;
            let id = insert_held_row(&tx, &package, report_id, author, stamp, &channel, &names, &sealed, keeper)?;
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

/// A hide and keep a stop cut short ([`Storage::held_unfinished_hides`]): one held post whose report
/// is still open.
#[derive(Debug, Clone, PartialEq)]
pub struct UnfinishedHide {
    pub report_id: i64,
    /// The moderator who decided it.
    pub held_by: String,
    pub author: String,
    pub stamp: u64,
    pub channel: String,
    /// Whether the moderator's signed record of the hide landed before the stop.
    pub recorded: bool,
}

impl Storage {
    /// Every held post whose report is still open: a hide and keep the server stopped part-way
    /// through, after its copy was sealed and its post taken out of view but before its decision
    /// was recorded (the owner branch's seventh review, L2), each with whether its signed record
    /// landed. handlers/held.rs `complete_unfinished_hides` completes them at start.
    pub fn held_unfinished_hides(&self) -> Result<Vec<UnfinishedHide>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut q = conn.prepare(
                "SELECT h.report_id, h.held_by, h.author_key, h.stamp, h.channel,
                        EXISTS(SELECT 1 FROM held_actions a WHERE a.action = 'hide_and_keep' AND a.target = h.report_id)
                 FROM held_items h JOIN reports_v2 r ON r.id = h.report_id
                 WHERE r.state = 'open' AND substr(r.decision_note, 1, length(?1)) <> ?1 ORDER BY h.id",
            )?;
            // A hide that stopped part-way and SAID so (its note begins with the prefix) is no stop
            // to complete: the report stays open for the moderator (the eighth review, M5).
            let rows = q.query_map([crate::relay::storage::reports::PARTIAL_HIDE_PREFIX], |r| {
                Ok(UnfinishedHide { report_id: r.get(0)?, held_by: r.get(1)?, author: r.get(2)?, stamp: r.get::<_, i64>(3)? as u64, channel: r.get(4)?, recorded: r.get(5)? })
            })?;
            rows.collect()
        })
    }
}

/// Hooks a test runs once at the very start of [`Storage::hide_post_in`] for the held folder each
/// names (a test's own folder, so tests running side by side never meet), before anything is read
/// or locked: a test pauses a hide there (the sixth review's M1: the moderator's connection drops
/// mid-hide) or changes the owner meanwhile (a quick change). On whatever thread the hide runs.
#[cfg(test)]
pub(crate) static BEFORE_HIDE: std::sync::Mutex<Vec<(PathBuf, Box<dyn FnMut() + Send>)>> = std::sync::Mutex::new(Vec::new());

/// Run (and drop) the hooks set for the held folder `held`, without the hooks' lock held (a hook
/// may block until its test lets it go).
#[cfg(test)]
fn before_hide(held: &Path) {
    let mut mine: Vec<Box<dyn FnMut() + Send>> = {
        let mut all = BEFORE_HIDE.lock().unwrap_or_else(|p| p.into_inner());
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *all).into_iter().partition(|(at, _)| at == held);
        *all = rest;
        here.into_iter().map(|(_, f)| f).collect()
    };
    for f in mine.iter_mut() {
        f();
    }
}

/// Record a held item: its row (the module's "Data"), in `tx`. Its id.
#[allow(clippy::too_many_arguments)]
fn insert_held_row(tx: &rusqlite::Transaction, package: &str, report_id: i64, author: &str, stamp: u64, channel: &str, names: &[String], sealed: &Sealed, keeper: &Keeper) -> Result<i64, rusqlite::Error> {
    let hashes: Vec<String> = names.iter().map(|n| name_hash(n)).collect();
    tx.execute(
        "INSERT INTO held_items (package, report_id, author_key, stamp, channel, file_count, parts, sealed_bytes,
            sealed_to, card_at, name_hashes, held_by, held_at, delete_after, also_sealed_to)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            package, report_id, author, stamp as i64, channel, names.len() as i64, sealed.parts as i64, sealed.bytes as i64,
            keeper.owner, keeper.card_at as i64, serde_json::to_string(&hashes).unwrap_or_else(|_| "[]".into()),
            keeper.held_by, keeper.now_ms, keeper.now_ms + HELD_KEEP_DAYS * DAY_MS, keeper.successor.map(|(k, _)| k).unwrap_or("")
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

impl Storage {
    /// Hide the post `author` sent at `stamp` for report `report_id` while an owner change is pending
    /// ([`WAITING_PREFIX`], the third review's S3): out of every view NOW, exactly as
    /// [`Storage::hide_post_in`] takes it (the row, a pinned copy and the files' upload rows in one
    /// transaction, the files out of every served folder), but nothing is sealed: the files move into
    /// a waiting folder in the held folder with a note of the post, for
    /// [`Storage::held_seal_waiting_in`] once the change is carried out or rolled back. None when
    /// this server has no such post. A step that fails puts the files back and changes nothing.
    ///
    /// THE NOTE COMES FIRST (the fourth review's finding 5a, 2026-11-21): the folder is made under a
    /// staging name with its note written and synced inside, then renamed into place in one step, and
    /// only then do the files move. So every waiting folder is complete (its note names the post) or
    /// was never made: a stop before the rename leaves a staging folder the stray sweep removes
    /// (nothing had moved), and a stop after it leaves a note the seal pass finishes the hide from
    /// ([`Storage::held_seal_waiting_in`]: the files still in view moved in, the rows taken out). The
    /// whole of it runs under the held folder's one lock, so a seal pass never sees it half-made.
    pub fn hide_post_waiting_in(&self, report_id: i64, author: &str, stamp: u64, held_by: &str, now_ms: i64, folders: Folders) -> Result<Option<HiddenPost>, HideError> {
        // The post read under the lock too, so one post never waits twice (the seventh review, L3).
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let Some((content, channel)) = self.post_for_report(author, stamp)? else { return Ok(None) };
        let names = files_of(&content, folders);
        let rows = names.iter().map(|n| self.upload_row_of(n).map(|(u, s, o)| (n.clone(), u, s, o)).unwrap_or_else(|| (n.clone(), String::new(), 0, String::new()))).collect();
        let id = hex::encode(rand::random::<[u8; 16]>());
        let held = held_dir_beside(folders);
        let staging = held.join(format!("{STAGING_PREFIX}{id}"));
        let dir = held.join(format!("{WAITING_PREFIX}{id}"));
        let [wu, wm, wr, ww] = waiting_folders(&dir);
        let wait = Folders { uploads: &wu, members: &wm, resume: &wr, waiting: &ww };
        let note = WaitingPost { report_id, author: author.to_string(), stamp, channel: channel.clone(), words: content.clone(), names: names.clone(), rows, held_by: held_by.to_string(), held_at: now_ms };
        // 1. The note, complete, under the staging name; then the folder renamed into place.
        let noted = (|| -> Result<(), String> {
            for d in waiting_folders(&staging) {
                std::fs::create_dir_all(&d).map_err(|e| format!("the waiting folder could not be made ({e})"))?;
            }
            write_synced(&staging.join(WAITING_NOTE), &serde_json::to_vec(&note).map_err(|e| e.to_string())?)?;
            std::fs::rename(&staging, &dir).map_err(|e| format!("the waiting folder could not be put in place ({e})"))
        })();
        if let Err(e) = noted {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(HideError::Seal(e));
        }
        // 2. The files, after the note.
        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        let put_back = |moved: &[(PathBuf, PathBuf)]| {
            for (from, to) in moved.iter().rev() {
                let _ = std::fs::rename(to, from);
            }
            let _ = std::fs::remove_dir_all(&dir);
        };
        let made = (|| -> Result<(), String> {
            for name in &names {
                let spot = folders.spot_of(name).ok_or_else(|| format!("{name} left its folder while it was being hidden"))?;
                for file in [name.clone(), super::files::poster_name(name)] {
                    let from = folders.dir(spot).join(&file);
                    if !from.is_file() {
                        continue;
                    }
                    #[cfg(test)]
                    BEFORE_WAITING_MOVE.with(|h| if let Some(f) = h.borrow_mut().as_mut() { f() });
                    let to = wait.dir(spot).join(&file);
                    std::fs::rename(&from, &to).map_err(|e| format!("{file} could not be taken out of view ({e})"))?;
                    moved.push((from, to));
                }
            }
            Ok(())
        })();
        if let Err(e) = made {
            put_back(&moved);
            return Err(HideError::Seal(e));
        }
        // 3. The rows, in one transaction.
        let unpinned = match take_post_rows(self, author, stamp, &names) {
            Ok(u) => u,
            Err(e) => {
                put_back(&moved);
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
        Ok(Some(HiddenPost { id: 0, channel, files: names.len(), left_in_view, unpinned }))
    }

    /// Every waiting hide's folder in the held folder beside `folders`, sorted, each with its note
    /// when it has a readable one.
    fn waiting_hides_in(&self, folders: Folders) -> Vec<(String, PathBuf, Option<WaitingPost>)> {
        let Ok(entries) = std::fs::read_dir(held_dir_beside(folders)) else { return Vec::new() };
        let mut out: Vec<(String, PathBuf, Option<WaitingPost>)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_prefix(WAITING_PREFIX)?.to_string();
                let note = std::fs::read(e.path().join(WAITING_NOTE)).ok().and_then(|b| serde_json::from_slice::<WaitingPost>(&b).ok());
                Some((id, e.path(), note))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// How many hides wait for their sealed copy ([`WAITING_PREFIX`]), and how many waiting folders
    /// have no note (a stop mid-hide on a build that moved the files before writing the note; never
    /// matched to a post by guesswork: the fifth review's L2): both told to the admins in Held
    /// copies (handlers/held.rs), never left silent (the fourth review's finding 5a).
    pub fn held_waiting_counts_in(&self, folders: Folders) -> (usize, usize) {
        let all = self.waiting_hides_in(folders);
        let unnoted = all.iter().filter(|(_, _, n)| n.is_none()).count();
        (all.len() - unnoted, unnoted)
    }

    /// How many waiting folders there are, noted or not.
    pub fn held_waiting_count_in(&self, folders: Folders) -> usize {
        self.waiting_hides_in(folders).len()
    }

    /// The reports whose hide waits for its sealed copy: a decided report is never cleared while it
    /// does (the fourth review's finding 5c), so the held item it becomes still names it.
    pub fn held_waiting_reports_in(&self, folders: Folders) -> Vec<i64> {
        self.waiting_hides_in(folders).into_iter().filter_map(|(_, _, n)| n.map(|n| n.report_id)).collect()
    }

    /// When the longest-waiting hide was hidden (its note's time), if any waits.
    pub fn held_oldest_waiting_in(&self, folders: Folders) -> Option<i64> {
        self.waiting_hides_in(folders).into_iter().filter_map(|(_, _, n)| n.map(|n| n.held_at)).min()
    }

    /// Seal every waiting hide ([`WAITING_PREFIX`]) to the owner's card AS IT IS NOW (and the
    /// successor's, when one is named), under the held folder's one lock: each becomes a held item
    /// like any other, and its waiting folder goes. The caller makes sure no owner change is pending
    /// or being carried out (handlers/owner.rs `seal_waiting`). How many were sealed.
    ///
    /// First every waiting folder is looked at (the fourth review's finding 5a): one with no note
    /// and no file in it at all (a stop before the note, on a build that wrote it last) is removed,
    /// said in the log, since there is nothing in it to pair or lose; one with no note but a file
    /// stays exactly as it is, counted for the admins ([`Storage::held_waiting_counts_in`]) and
    /// named in the log, never silent and never guessed: round 5 paired it with a post whose words
    /// contained a file's name and then sealed and deleted it, so a stray file could go out as the
    /// wrong post's copy (the fifth review's L2). A
    /// noted hide a stop left half-done is finished: the files its note names still in a served
    /// folder are moved in, and its rows taken out (nothing again when it was done). A hide that
    /// cannot be sealed now (no owner card that checks out) keeps waiting, logged.
    ///
    /// IDEMPOTENT BY HIDE ID (the fourth review's finding 5e): the held item's package IS the
    /// waiting folder's id, so a stop between recording the row and removing the folder never makes
    /// a second held item: the next pass finds the row, removes the plaintext and seals nothing again;
    /// a copy put in place but never recorded is replaced.
    pub fn held_seal_waiting_in(&self, folders: Folders) -> usize {
        let held = held_dir_beside(folders);
        let _swap = self.held_swap.lock().unwrap_or_else(|p| p.into_inner());
        let mut ready: Vec<(String, PathBuf, WaitingPost)> = Vec::new();
        for (id, dir, note) in self.waiting_hides_in(folders) {
            let note = match note {
                Some(n) => n,
                None => {
                    let holds_files = walk_files(&dir).iter().any(|f| f.file_name().is_some_and(|n| n != WAITING_NOTE));
                    if !holds_files {
                        let _ = std::fs::remove_dir_all(&dir);
                        tracing::warn!("Hide and keep: removed waiting folder {} (no note and no file: a stop before the hide's note was written; nothing had moved)", dir.display());
                        continue;
                    }
                    // Never matched to a post by its words and never deleted (the fifth review's
                    // L2): counted for every admin in Held copies, in words, and named here.
                    tracing::error!("Hide and keep: waiting folder {} has files but no note saying which post they belong to; they stay unsealed in the held folder, out of every view, until the owner looks at them on the server machine (never guessed, never deleted)", dir.display());
                    continue;
                }
            };
            ready.push((id, dir, note));
        }
        if ready.is_empty() {
            return 0;
        }
        let card = self.owner_card();
        let OwnerCard::Ready { owner, kyber, card_at, successor } = &card else {
            tracing::error!("Hide and keep: {} hidden post(s) wait for a sealed copy, but {}", ready.len(), card.refusal().unwrap_or(NO_CARD));
            return 0;
        };
        let successor = successor.as_ref().map(|(k, kb)| (k.as_str(), kb.as_slice()));
        let mut sealed_count = 0;
        for (package, dir, note) in ready {
            // Already sealed and recorded (a stop before the folder went): only the plaintext goes.
            if let Ok(Some(id)) = self.with_read_conn(|c| c.query_row("SELECT id FROM held_items WHERE package = ?1", params![package], |r| r.get::<_, i64>(0)).optional()) {
                match std::fs::remove_dir_all(&dir) {
                    Ok(()) => tracing::warn!("Hide and keep: held item #{id} was already sealed from waiting hide {package}; its waiting folder is removed now"),
                    Err(e) => tracing::error!("Hide and keep: held item #{id} is sealed, but its waiting folder could not be removed ({e})"),
                }
                continue;
            }
            let [wu, wm, wr, ww] = waiting_folders(&dir);
            let wait = Folders { uploads: &wu, members: &wm, resume: &wr, waiting: &ww };
            // Finish a hide a stop left half-done: what is still in view moves in, the rows go.
            if let Err(e) = finish_waiting_hide(self, &note, folders, wait) {
                tracing::error!("Hide and keep: report #{}'s waiting hide could not be finished ({e}); it keeps waiting", note.report_id);
                continue;
            }
            let keeper = Keeper { owner, kyber, card_at: *card_at, successor, held_by: &note.held_by, now_ms: note.held_at };
            let staging = held.join(format!("{STAGING_PREFIX}{package}"));
            let home = held.join(&package);
            let _ = std::fs::remove_dir_all(&staging);
            if home.exists() {
                // A copy put in place by a pass that stopped before recording it: never recorded,
                // so nothing names it; made again from the plaintext that still waits.
                let _ = std::fs::remove_dir_all(&home);
            }
            let row_of = |n: &str| note.rows.iter().find(|r| r.0 == n).map(|r| (r.1.clone(), r.2, r.3.clone()));
            let sealed = match self.seal_package(&staging, &package, note.report_id, &note.author, note.stamp, &note.channel, &note.words, &note.names, &keeper, wait, &row_of) {
                Ok(s) => s,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&staging);
                    tracing::error!("Hide and keep: report #{}'s waiting copy could not be sealed ({e}); it keeps waiting", note.report_id);
                    continue;
                }
            };
            if let Err(e) = std::fs::rename(&staging, &home) {
                let _ = std::fs::remove_dir_all(&staging);
                tracing::error!("Hide and keep: report #{}'s sealed copy could not be put in place ({e}); it keeps waiting", note.report_id);
                continue;
            }
            let recorded = self.with_conn_mut(|conn| -> Result<i64, rusqlite::Error> {
                let tx = conn.transaction()?;
                let id = insert_held_row(&tx, &package, note.report_id, &note.author, note.stamp, &note.channel, &note.names, &sealed, &keeper)?;
                tx.commit()?;
                Ok(id)
            });
            match recorded {
                Ok(id) => {
                    // Sealed and recorded: the plaintext that waited goes.
                    if let Err(e) = std::fs::remove_dir_all(&dir) {
                        tracing::error!("Hide and keep: held item #{id} is sealed, but its waiting folder could not be removed ({e})");
                    }
                    sealed_count += 1;
                }
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&home);
                    tracing::error!("Hide and keep: report #{}'s sealed copy could not be recorded ({e}); it keeps waiting", note.report_id);
                }
            }
        }
        sealed_count
    }
}

/// Take the post `author` sent at `stamp` out of the database in one transaction: the upload rows
/// of `names`, a pinned copy and the post's row. Whether a pinned copy went. Doing it again is
/// harmless (nothing is left to take).
fn take_post_rows(db: &Storage, author: &str, stamp: u64, names: &[String]) -> Result<bool, rusqlite::Error> {
    db.with_conn_mut(|conn| -> Result<bool, rusqlite::Error> {
        let tx = conn.transaction()?;
        for name in names {
            tx.execute("DELETE FROM user_uploads WHERE filename = ?1", params![name])?;
        }
        let pins = tx.execute("DELETE FROM pinned_messages WHERE from_key = ?1 AND original_timestamp = ?2", params![author, stamp as i64])?;
        tx.execute("DELETE FROM messages WHERE from_key = ?1 AND timestamp = ?2", params![author, stamp as i64])?;
        tx.commit()?;
        Ok(pins > 0)
    })
}

/// Finish a waiting hide from its note (the fourth review's finding 5a): each file the note names
/// that is not in the waiting folder yet but still in a served folder is moved in (with its poster),
/// then the post's rows are taken out. For a hide that completed, nothing moves and nothing is left
/// to take.
fn finish_waiting_hide(db: &Storage, note: &WaitingPost, folders: Folders, wait: Folders) -> Result<(), String> {
    for name in &note.names {
        if wait.spot_of(name).is_some() {
            continue;
        }
        let Some(spot) = folders.spot_of(name) else { continue };
        std::fs::create_dir_all(wait.dir(spot)).map_err(|e| format!("the waiting folder could not be made ({e})"))?;
        for file in [name.clone(), super::files::poster_name(name)] {
            let from = folders.dir(spot).join(&file);
            if from.is_file() {
                std::fs::rename(&from, wait.dir(spot).join(&file)).map_err(|e| format!("{file} could not be taken out of view ({e})"))?;
            }
        }
    }
    take_post_rows(db, &note.author, note.stamp, &note.names).map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
thread_local! {
    /// A test's step just before a waiting hide moves each file (handlers/owner_tests.rs: a panic
    /// here is the relay stopping mid-hide, the fourth review's finding 5a).
    pub(crate) static BEFORE_WAITING_MOVE: std::cell::RefCell<Option<Box<dyn FnMut()>>> = const { std::cell::RefCell::new(None) };
}

/// Every file under `dir`, at any depth.
fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk_files(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// Restore (rung 5): the owner's device sends each file back, checked against the restore plan.
#[path = "held_restore.rs"]
pub mod restore;

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
