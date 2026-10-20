//! The Files page's requests to its server (the relay half is src/relay/files.rs; the design is
//! docs/design/files-showcase.md, "Increment 2, exact" and "Increment 2 follow-ups, exact"), and
//! the server's answers read into plain structs.
//!
//! EVERY WRITE IS SIGNED the way the relay checks it (`check_signed`): Dilithium over
//! `purpose\ntimestamp`, the same signature every other identity-keyed request of the desktop
//! makes (`net::identity::pq_sign_chat`), with a purpose that names its target, so a signature is
//! good for one thing (the relay's F5): `media_publish:<file name>:<licence id>:<0|1>` (Media round
//! 5: the licence and the game tick are signed with the publish), `media_remove:<item id or file
//! name>`, `media_pick:<item id>` (pick and unpick), `media_feature:<item id>:<main|category|none>`
//! (round 5), `media_keep:<item id>:<on|off>`, `media_suggestions:<suggestion id>:<accept|decline>`
//! and `media_categories:<BLAKE3 hex of the category list exactly as sent>`; round 5's edit and
//! file purposes are in files/api_items.rs. A new suggestion keeps
//! plain `media_suggest`, and the reads keep `media_list`, `media_usage`, `media_mine` and
//! `media_suggestions`. The relay also refuses a (key, purpose, timestamp) it has seen, so every
//! signature here takes a fresh timestamp from [`next_timestamp`], which never hands out the same
//! millisecond twice.
//!
//! EVERY ACTION ON AN ITEM NAMES ITS FILE TOO (the relay's W5, 2026-10-11): remove by number,
//! pick, unpick, feature and keep carry `filename`, the item's stored file name, so a tile left on
//! a page that is out of date never acts on another item. The relay answers 409 when the number
//! and the name do not belong together; the page then loads the list again and says so.
//!
//! A signed read goes in a POST body, never a URL: a Dilithium key and signature make an address
//! of about 10 KB, which a proxy refuses (relay/files.rs).
//!
//! Nothing here runs on the frame: [`Request`] is built on the frame (cheap, and what the tests
//! read), and [`send`] runs it on a worker thread.

use serde_json::{json, Value};

// ── What the server sends back ──────────────────────────────────────────────────────────────

/// One content label as the server lists it (data/files/labels.json on the relay).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Label {
    pub id: String,
    pub label: String,
    pub desc: String,
}

/// One category, in the order the page shows them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub description: String,
}

/// The general category's number (the relay's `GENERAL_CATEGORY_ID`): an item published with no
/// category goes there, and it can be renamed but never removed.
pub const GENERAL_CATEGORY: i64 = 1;

/// Where an item is featured (Media round 5): the relay sends "main" (the main shelf, and its
/// category's), "category" (its category's shelf only) or "none". It was a bool; this app changed
/// with the relay, with no reading of the old shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Featured {
    #[default]
    None,
    Main,
    Category,
}

impl Featured {
    /// The relay's word; anything else reads as none.
    pub fn parse(word: &str) -> Self {
        match word {
            "main" => Featured::Main,
            "category" => Featured::Category,
            _ => Featured::None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Featured::None => "none",
            Featured::Main => "main",
            Featured::Category => "category",
        }
    }

    /// On a shelf at all.
    pub fn is_on(self) -> bool {
        self != Featured::None
    }
}

pub use super::api_items::{ItemFile, Licence, Licences, Sort};

/// One Media item as every route gives it (relay/files.rs `item_json`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub id: i64,
    /// `/uploads/<name>` while public; `/api/files/file/<id>` for a members-only item, fetched
    /// with this session's upload token in a header ([`TOKEN_HEADER`]).
    pub url: String,
    pub title: String,
    pub description: String,
    pub category_id: i64,
    pub category_name: String,
    /// `picture`, `video`, `model` or `file` (data/files/kinds.json names them).
    pub kind: String,
    /// The content label ids.
    pub labels: Vec<String>,
    /// `public` or `members`.
    pub visibility: String,
    /// On the main shelf, its category's only, or neither (round 5).
    pub featured: Featured,
    /// The item's own file's size (file 0).
    pub size_bytes: i64,
    pub views: i64,
    pub uploaded_at: i64,
    pub published_at: i64,
    pub uploader_key: String,
    pub uploader_name: String,
    pub picks: i64,
    /// The uploader's own name for the file ('' when the server does not know it): what Download
    /// saves it as.
    pub original_name: String,
    /// What Remove does to its file (the relay's A1): true gives it back where it came from (a
    /// chat or Shared Files), false deletes it (it was uploaded for Media).
    pub keeps_file: bool,
    /// The server keeps it (A7): an admin or moderator chose to carry it, so it counts against the
    /// Media space and not its uploader's share.
    pub kept: bool,
    /// In the members' view, whether this person picked it (A3). The public view has no such
    /// field, and nothing there can be picked.
    pub picked_by_me: bool,
    /// The item's stored file name, which every action on it names (W5): the `filename` the server
    /// sends, else the last part of its `/uploads/` address. Empty when the server says neither
    /// (a members-only item from a server that does not send it).
    pub stored: String,
    /// Round 5 (2026-10-12): the licence's id (the server's `licences` names it), whether it is
    /// offered for the game, how many people downloaded it, the item's own file with its added
    /// files and cover in bytes, its file count (its own and the added ones), its files (file 0,
    /// its own, first), its cover, and its own page's path (`/files/<id>`).
    pub licence: String,
    pub project_use: bool,
    pub downloads: i64,
    pub total_bytes: i64,
    pub file_count: i64,
    pub files: Vec<ItemFile>,
    pub cover: Option<ItemFile>,
    pub link: String,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

impl Item {
    /// The item in `v`, or None when it has no id.
    pub fn from_json(v: &Value) -> Option<Item> {
        let id = v.get("id").and_then(Value::as_i64)?;
        let category = v.get("category").cloned().unwrap_or(Value::Null);
        let url = s(v, "url");
        let named = s(v, "filename");
        let stored = if named.trim().is_empty() { upload_name_of(&url).unwrap_or("").to_string() } else { bare_name(&named).to_string() };
        Some(Item {
            id,
            url,
            title: s(v, "title"),
            description: s(v, "description"),
            category_id: i(&category, "id"),
            category_name: s(&category, "name"),
            kind: s(v, "kind"),
            labels: v
                .get("labels")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            visibility: s(v, "visibility"),
            featured: Featured::parse(&s(v, "featured")),
            size_bytes: i(v, "size_bytes"),
            views: i(v, "views"),
            uploaded_at: i(v, "uploaded_at"),
            published_at: i(v, "published_at"),
            uploader_key: s(v, "uploader_key"),
            uploader_name: s(v, "uploader_name"),
            picks: i(v, "picks"),
            original_name: s(v, "original_name"),
            keeps_file: b(v, "keeps_file"),
            kept: b(v, "kept"),
            picked_by_me: b(v, "picked_by_me"),
            stored,
            licence: s(v, "licence"),
            project_use: b(v, "project_use"),
            downloads: i(v, "downloads"),
            total_bytes: i(v, "total_bytes"),
            file_count: i(v, "file_count"),
            files: v.get("files").and_then(Value::as_array).map(|a| a.iter().filter_map(ItemFile::from_json).collect()).unwrap_or_default(),
            cover: v.get("cover").and_then(ItemFile::from_json),
            link: s(v, "link"),
        })
    }

    /// Members only: anything the server does not call public (it fails closed the same way).
    pub fn members_only(&self) -> bool {
        self.visibility != "public"
    }

    /// The stored file name of a public item (the last part of its `/uploads/` address).
    pub fn upload_name(&self) -> Option<&str> {
        upload_name_of(&self.url)
    }

    /// What every action on this item names: its number and its stored file name (W5). Refused
    /// with [`NO_FILE_NAME`] when the server did not say the name, rather than sent without it.
    pub fn reference(&self) -> Result<ItemRef, String> {
        if self.stored.trim().is_empty() {
            return Err(NO_FILE_NAME.to_string());
        }
        Ok(ItemRef { id: self.id, filename: self.stored.clone() })
    }

    /// What the tile calls it: its title, else the uploader's own name for the file, else its
    /// stored name, else the name of its kind.
    pub fn display_title(&self) -> String {
        if !self.title.trim().is_empty() {
            self.title.trim().to_string()
        } else if !self.original_name.trim().is_empty() {
            self.original_name.trim().to_string()
        } else if let Some(name) = self.upload_name() {
            name.to_string()
        } else {
            super::kinds::one_name(&self.kind)
        }
    }

    /// Who put it there, by name ("someone" when the server knows no name).
    pub fn by_name(&self) -> &str {
        if self.uploader_name.trim().is_empty() {
            "someone"
        } else {
            self.uploader_name.trim()
        }
    }
}

/// The last part of a `/uploads/` address.
fn upload_name_of(url: &str) -> Option<&str> {
    url.strip_prefix("/uploads/").filter(|n| !n.is_empty() && !n.contains('/'))
}

/// What is said when an item's file name is not known, so nothing can be asked of it.
pub const NO_FILE_NAME: &str =
    "The server did not say this item's file name, so this app cannot act on it safely. Load the page again; if it stays like this, the server needs updating.";

/// What an action on an item names: its number and its stored file name (the relay's W5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRef {
    pub id: i64,
    pub filename: String,
}

/// The list's answer (`GET /api/files`, `POST /api/files/list`, `POST /api/files/picks`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListAnswer {
    pub items: Vec<Item>,
    /// The cursor that asks for the next page (round 5, opaque); None at the end.
    pub next_cursor: Option<String>,
    /// Whether the answer is the members' view (a signed request from a member).
    pub members_view: bool,
    /// The server's `media_visibility`: `public` or `members`.
    pub media_visibility: String,
    /// The whole page is members only and this viewer is not one.
    pub members_only_for_you: bool,
    pub categories: Vec<Category>,
    pub labels: Vec<Label>,
    /// The licences the server offers and the words beside them (round 5); "All rights reserved"
    /// alone when the answer carries none.
    pub licences: Licences,
}

/// Read a category list (the list's `categories`, or a categories answer's).
pub fn parse_categories(v: Option<&Value>) -> Vec<Category> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| Some(Category { id: c.get("id")?.as_i64()?, name: s(c, "name"), description: s(c, "description") }))
                .collect()
        })
        .unwrap_or_default()
}

/// Read a label list.
pub fn parse_labels(v: Option<&Value>) -> Vec<Label> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|l| !s(l, "id").is_empty())
                .map(|l| Label { id: s(l, "id"), label: s(l, "label"), desc: s(l, "desc") })
                .collect()
        })
        .unwrap_or_default()
}

/// Read a list of items under `key` of `v`.
fn items_at(v: &Value, key: &str) -> Vec<Item> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Item::from_json).collect()).unwrap_or_default()
}

/// Read the list's answer.
pub fn parse_list(v: &Value) -> ListAnswer {
    ListAnswer {
        items: items_at(v, "items"),
        next_cursor: v.get("next_cursor").and_then(Value::as_str).filter(|c| !c.is_empty()).map(str::to_string),
        members_view: b(v, "members_view"),
        media_visibility: s(v, "media_visibility"),
        members_only_for_you: b(v, "members_only_for_you"),
        categories: parse_categories(v.get("categories")),
        labels: parse_labels(v.get("labels")),
        licences: Licences::from_json(v.get("licences")),
    }
}

/// A category suggestion (`GET or POST /api/files/suggestions`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Suggestion {
    pub id: i64,
    pub suggester_key: String,
    pub suggester_name: String,
    pub name: String,
    pub note: String,
    pub created_at: i64,
    /// `open`, `accepted` or `declined`.
    pub state: String,
}

/// Read the suggestions answer.
pub fn parse_suggestions(v: &Value) -> Vec<Suggestion> {
    v.get("suggestions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(Suggestion {
                        id: x.get("id")?.as_i64()?,
                        suggester_key: s(x, "suggester_key"),
                        suggester_name: s(x, "suggester_name"),
                        name: s(x, "name"),
                        note: s(x, "note"),
                        created_at: i(x, "created_at"),
                        state: s(x, "state"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A Media upload not yet published (it expires after a day unless it is).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Waiting {
    pub filename: String,
    /// The uploader's own name for the file ('' when unknown).
    pub original_name: String,
    pub uploader_name: String,
    pub size_bytes: i64,
    pub uploaded_at: i64,
    pub expires_at: i64,
}

impl Waiting {
    /// The name shown for it: the uploader's own, else the stored one.
    pub fn shown_name(&self) -> &str {
        if self.original_name.trim().is_empty() {
            &self.filename
        } else {
            self.original_name.trim()
        }
    }
}

fn parse_waiting(v: Option<&Value>) -> Vec<Waiting> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|w| Waiting {
                    filename: s(w, "filename"),
                    original_name: s(w, "original_name"),
                    uploader_name: s(w, "uploader_name"),
                    size_bytes: i(w, "size_bytes"),
                    uploaded_at: i(w, "uploaded_at"),
                    expires_at: i(w, "expires_at"),
                })
                .filter(|w| !w.filename.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The admins' numbers (`POST /api/files/usage`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub used_bytes: i64,
    pub budget_bytes: i64,
    pub per_person_bytes: i64,
    /// What the items the server keeps take (A7): inside `used_bytes`, in nobody's share.
    pub kept_bytes: i64,
    pub items: i64,
    pub largest: Vec<Item>,
    pub oldest: Vec<Item>,
    pub least_viewed: Vec<Item>,
    pub waiting: Vec<Waiting>,
}

/// Read the usage answer.
pub fn parse_usage(v: &Value) -> Usage {
    Usage {
        used_bytes: i(v, "used_bytes"),
        budget_bytes: i(v, "budget_bytes"),
        per_person_bytes: i(v, "per_person_bytes"),
        kept_bytes: i(v, "kept_bytes"),
        items: i(v, "items"),
        largest: items_at(v, "largest"),
        oldest: items_at(v, "oldest"),
        least_viewed: items_at(v, "least_viewed"),
        waiting: parse_waiting(v.get("waiting")),
    }
}

/// A member's own use of the Media space (`POST /api/files/mine`, the relay's A4), counted the
/// way the server's own room check counts it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mine {
    /// Their items and their uploads waiting to be published (items the server keeps left out).
    pub used_bytes: i64,
    pub share_bytes: i64,
    /// What the whole Media space has left.
    pub budget_left_bytes: i64,
    /// Their resumable uploads still arriving, which a new upload's start counts as used.
    pub in_progress_bytes: i64,
    /// The largest Media file their role may send; 0 for none (up to the share).
    pub file_limit_bytes: i64,
    /// Their uploads waiting to be published.
    pub waiting: Vec<Waiting>,
    /// Their resumable uploads not finished yet (`uploads_in_progress`, R3-7).
    pub in_progress: Vec<super::waiting::InProgress>,
}

impl Mine {
    /// What is left of their share for a new upload: the share less what is used and what is
    /// still arriving.
    pub fn share_left(&self) -> i64 {
        (self.share_bytes - self.used_bytes - self.in_progress_bytes).max(0)
    }
}

/// Read the `mine` answer.
pub fn parse_mine(v: &Value) -> Mine {
    Mine {
        used_bytes: i(v, "used_bytes"),
        share_bytes: i(v, "share_bytes"),
        budget_left_bytes: i(v, "budget_left_bytes"),
        in_progress_bytes: i(v, "in_progress_bytes"),
        file_limit_bytes: i(v, "file_limit_bytes"),
        waiting: parse_waiting(v.get("waiting")),
        in_progress: super::waiting::parse_in_progress(v.get("uploads_in_progress")),
    }
}

// ── Words ──────────────────────────────────────────────────────────────────────────────────

/// A size in the words the page uses: megabytes below a gigabyte, else gigabytes, one decimal
/// with a whole number shown whole ("412 MB", "12.4 GB", "50 GB").
pub fn size_words(bytes: i64) -> String {
    // The website's `sizeWords` (web/pages/files-app.js), word for word: a small file in KB or bytes,
    // never "0 MB" (operator's screenshot, 2026-10-17: "ClipOutsideB.stl, file, 0 MB").
    // Held to the shared cases both apps read (scripts/tests/fixtures/size-words-cases.json).
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes.max(0) as f64;
    // One decimal, an exact half rounding UP as the browser's toFixed does (1.25 MB reads
    // "1.3 MB"; Rust's own {:.1} rounds a half to even, "1.2"). Counted in whole tenths of
    // `unit`: floor((20 * bytes + unit) / (2 * unit)) is the nearest tenth, a tie taken up.
    let one_decimal = |unit: i64| {
        let tenths = (20 * bytes.max(0) as i128 + unit as i128) / (2 * unit as i128);
        if tenths % 10 == 0 { format!("{}", tenths / 10) } else { format!("{}.{}", tenths / 10, tenths % 10) }
    };
    if b >= 1024.0 * MB {
        format!("{} GB", one_decimal(1 << 30))
    } else if b >= MB {
        let n = b / MB;
        format!("{} MB", if n >= 100.0 { format!("{}", n.round()) } else { one_decimal(1 << 20) })
    } else if b >= 1024.0 {
        format!("{} KB", (b / 1024.0).round())
    } else {
        format!("{} {}", b as i64, if b as i64 == 1 { "byte" } else { "bytes" })
    }
}

/// The meter: "12.4 GB of 50 GB used".
pub fn meter_words(used: i64, budget: i64) -> String {
    format!("{} of {} used", size_words(used), size_words(budget))
}

/// The meter's line for the items the server keeps.
pub fn kept_words(kept: i64) -> String {
    format!("{} of it kept by the server: counted in the file space, in nobody's share.", size_words(kept))
}

/// The line above the Add to Files form: "You have 412 MB of your 512 MB left".
pub fn share_line(mine: &Mine) -> String {
    format!("You have {} of your {} left", size_words(mine.share_left()), size_words(mine.share_bytes))
}

/// Why a Media file of `len` bytes cannot go, from the server's own numbers (`mine`, when known):
/// over the largest Media file the person's role may send, over what is left of their share, or
/// over what is left of the whole Media space. None when it fits, or when the numbers are not
/// known yet (the server checks them itself at the start). Checked before a byte is sent, so an
/// upload over the share never starts.
/// Several files going up together (2026-10-16): each must fit on its own (`room_problem`, said
/// with its name), and all of them together in the person's share and the server's space, so a
/// batch never starts uploads that the last of them cannot finish.
pub fn room_problem_together(files: &[(&str, u64)], mine: Option<&Mine>) -> Option<String> {
    for (name, len) in files {
        if let Some(problem) = room_problem(*len, mine) {
            return Some(if files.len() > 1 { format!("{name}: {problem}") } else { problem });
        }
    }
    let mine = mine?;
    if files.len() < 2 {
        return None;
    }
    let total: i64 = files.iter().map(|(_, len)| *len as i64).sum();
    if total > mine.share_left() {
        return Some(format!(
            "These {} files come to {}, more than the {} left of your share of this server's file space, so none was uploaded. Choose fewer, remove one of your items, or ask an admin to raise each person's share.",
            files.len(),
            size_words(total),
            size_words(mine.share_left())
        ));
    }
    if total > mine.budget_left_bytes.max(0) {
        return Some(format!(
            "These {} files come to {}, more than the {} left in this server's whole file space, so none was uploaded. Choose fewer, or its admins can make more room.",
            files.len(),
            size_words(total),
            size_words(mine.budget_left_bytes)
        ));
    }
    None
}

pub fn room_problem(len: u64, mine: Option<&Mine>) -> Option<String> {
    let mine = mine?;
    let len = len as i64;
    if mine.file_limit_bytes > 0 && len > mine.file_limit_bytes {
        return Some(format!(
            "This file is {}, over the largest file your role may put on this server's Files page ({}). An admin can raise it in Server Settings > Roles.",
            size_words(len),
            size_words(mine.file_limit_bytes)
        ));
    }
    if len > mine.share_left() {
        return Some(format!(
            "This file is {}, more than the {} left of your share of this server's file space, so it was not uploaded. Remove one of your items, or ask an admin to raise each person's share.",
            size_words(len),
            size_words(mine.share_left())
        ));
    }
    if len > mine.budget_left_bytes.max(0) {
        return Some(format!(
            "This file is {}, more than the {} left in this server's whole file space, so it was not uploaded. Its admins can make more room.",
            size_words(len),
            size_words(mine.budget_left_bytes)
        ));
    }
    None
}

/// The confirmation when a Remove takes the file away with the item.
pub const REMOVE_DELETES: &str = "Remove from Files and delete the file.";
/// The confirmation when the file stays where it came from (a chat or Shared Files).
pub const REMOVE_KEEPS: &str = "Remove from Files. The file stays where it came from.";

/// What Remove does to `item`'s file, in the words the confirmation shows: the item's own
/// `keeps_file`, which the server sends with every item (A1).
pub fn remove_words(item: &Item) -> &'static str {
    if item.keeps_file {
        REMOVE_KEEPS
    } else {
        REMOVE_DELETES
    }
}

// ── Signing ────────────────────────────────────────────────────────────────────────────────

/// A timestamp in Unix milliseconds that is never the same as one handed out before in this
/// run: the relay refuses a (key, purpose, timestamp) it has seen, and two requests in one
/// millisecond (a Refresh and a More) would otherwise share one.
pub fn next_timestamp() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut prev = LAST.load(Ordering::SeqCst);
    loop {
        let next = now.max(prev + 1);
        match LAST.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(seen) => prev = seen,
        }
    }
}

/// Who signs: the person's Dilithium public key (hex) and the seed it comes from.
#[derive(Clone)]
pub struct Signer {
    pub key: String,
    seed: Vec<u8>,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer").field("key", &self.key).finish_non_exhaustive() // never the seed
    }
}

impl Signer {
    pub fn new(key: &str, seed: &[u8]) -> Self {
        Signer { key: key.to_string(), seed: seed.to_vec() }
    }

    /// `body` with `key`, `timestamp` and `sig` over `purpose` added.
    pub fn signed(&self, purpose: &str, mut body: Value) -> Value {
        let ts = next_timestamp();
        let sig = crate::net::identity::pq_sign_chat(&self.seed, purpose, ts);
        if !body.is_object() {
            body = json!({});
        }
        body["key"] = json!(self.key);
        body["timestamp"] = json!(ts);
        body["sig"] = json!(sig);
        body
    }
}

/// The signer for this app's identity, when it is unlocked.
pub fn signer_of(state: &crate::gui::GuiState) -> Option<Signer> {
    let seed = state.private_key_bytes.as_ref()?;
    if state.profile_public_key.is_empty() {
        return None;
    }
    Some(Signer::new(&state.profile_public_key, seed))
}

/// The purpose of a signed list read (and of a signed picks read).
pub const LIST: &str = "media_list";
/// The purpose of a new category suggestion.
pub const SUGGEST: &str = "media_suggest";
/// The purpose of the suggestions read.
pub const SUGGESTIONS: &str = "media_suggestions";
/// The purpose of the admins' usage read.
pub const USAGE: &str = "media_usage";
/// The purpose of a member's own use read.
pub const MINE: &str = "media_mine";

/// The stored file name a request names: as given, or the last part of its `/uploads/` address.
pub fn bare_name(given: &str) -> &str {
    given.trim().trim_start_matches("/uploads/")
}

/// The licence an item gets when none is chosen (the relay's `DEFAULT_LICENCE`).
pub const DEFAULT_LICENCE: &str = "all_rights_reserved";

/// The purpose a publish is signed over (round 5): the file, the licence and the game tick.
pub fn publish_purpose(filename: &str, licence: &str, project_use: bool) -> String {
    let licence = if licence.trim().is_empty() { DEFAULT_LICENCE } else { licence.trim() };
    format!("media_publish:{}:{licence}:{}", bare_name(filename), project_use as u8)
}

/// What a removal names: a published item (its number and file), or a Media upload waiting to be
/// published (its file name alone).
#[derive(Debug, Clone, PartialEq)]
pub enum RemoveTarget {
    Item(ItemRef),
    Upload(String),
}

pub fn remove_purpose(target: &RemoveTarget) -> String {
    match target {
        RemoveTarget::Item(r) => format!("media_remove:{}", r.id),
        RemoveTarget::Upload(name) => format!("media_remove:{}", bare_name(name)),
    }
}

pub fn pick_purpose(id: i64) -> String {
    format!("media_pick:{id}")
}

/// The purpose featuring item `id` at `to` is signed over (round 5).
pub fn feature_purpose(id: i64, to: Featured) -> String {
    format!("media_feature:{id}:{}", to.word())
}

pub fn keep_purpose(id: i64, kept: bool) -> String {
    format!("media_keep:{id}:{}", if kept { "on" } else { "off" })
}

pub fn decide_purpose(id: i64, accept: bool) -> String {
    format!("media_suggestions:{id}:{}", if accept { "accept" } else { "decline" })
}

/// The purpose of a category list: BLAKE3 over the list's JSON text exactly as it is sent.
pub fn categories_purpose(categories_text: &str) -> String {
    format!("media_categories:{}", blake3::hash(categories_text.as_bytes()).to_hex())
}

// ── Requests ───────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One request, built on the frame and sent on a worker thread.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: Method,
    /// The path and query, from `/api/...`.
    pub path: String,
    /// The JSON body of a POST, as text (the categories request's text is what is hashed).
    pub body: Option<String>,
    /// The purpose the body is signed over, for the tests and the log; None for an unsigned read.
    pub purpose: Option<String>,
}

pub(super) fn post(path: &str, purpose: String, body: Value) -> Request {
    Request { method: Method::Post, path: path.to_string(), body: Some(body.to_string()), purpose: Some(purpose) }
}

fn enc(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// The list's filters, as the page holds them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    pub category: Option<i64>,
    pub kind: Option<String>,
    /// A person, by their key (the page shows their name).
    pub by: Option<String>,
    pub q: String,
    /// The person's own picks instead of everything.
    pub my_picks: bool,
    /// Only the featured items (the strip at the top, A2).
    pub featured: bool,
    /// One item by its number (the Reports page opens a reported item this way, A5).
    pub id: Option<i64>,
    /// The order (round 5): newest (the default, not sent), most picked or most downloaded.
    pub sort: Sort,
}

/// How many items a page of the list asks for.
pub const PAGE: i64 = 48;

/// The list request for `filters`, the page after `cursor` (the last answer's `next_cursor`,
/// round 5; the first page with none). With a signer (someone signed in to this server) it is a
/// signed POST, which gets members-only items to a member: the list, or for "My picks" the
/// person's picks (`POST /api/files/picks` with `of`, A3). Without one, the public GET of either.
pub fn list_request(filters: &Filters, cursor: Option<&str>, limit: i64, me: &str, signer: Option<&Signer>) -> Request {
    let q = filters.q.trim();
    let Some(signer) = signer else {
        let mut query = vec![format!("limit={limit}")];
        if let Some(c) = filters.category {
            query.push(format!("category={c}"));
        }
        if let Some(k) = filters.kind.as_deref() {
            query.push(format!("kind={}", enc(k)));
        }
        if let Some(by) = filters.by.as_deref() {
            query.push(format!("by={}", enc(by)));
        }
        if !q.is_empty() {
            query.push(format!("q={}", enc(q)));
        }
        if filters.featured {
            query.push("featured=1".to_string());
        }
        if let Some(id) = filters.id {
            query.push(format!("id={id}"));
        }
        if filters.sort != Sort::New {
            query.push(format!("sort={}", filters.sort.word()));
        }
        if let Some(c) = cursor {
            query.push(format!("cursor={}", enc(c)));
        }
        let path = if filters.my_picks { format!("/api/files/picks/{}", enc(me)) } else { "/api/files".to_string() };
        return Request { method: Method::Get, path: format!("{path}?{}", query.join("&")), body: None, purpose: None };
    };
    let mut body = json!({ "limit": limit });
    if let Some(c) = filters.category {
        body["category"] = json!(c);
    }
    if let Some(k) = filters.kind.as_deref() {
        body["kind"] = json!(k);
    }
    if let Some(by) = filters.by.as_deref() {
        body["by"] = json!(by);
    }
    if !q.is_empty() {
        body["q"] = json!(q);
    }
    if filters.featured {
        body["featured"] = json!(1);
    }
    if let Some(id) = filters.id {
        body["id"] = json!(id);
    }
    if filters.sort != Sort::New {
        body["sort"] = json!(filters.sort.word());
    }
    if let Some(c) = cursor {
        body["cursor"] = json!(c);
    }
    if filters.my_picks {
        body["of"] = json!(me);
        return post("/api/files/picks", LIST.to_string(), signer.signed(LIST, body));
    }
    post("/api/files/list", LIST.to_string(), signer.signed(LIST, body))
}

/// What an item is published as (the Add to Files and Put on the Files page form).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PublishFields {
    /// The stored file name (or its `/uploads/` address).
    pub filename: String,
    pub title: String,
    pub description: String,
    /// Always sent: the category the form shows ([`GENERAL_CATEGORY`] when none was chosen).
    pub category_id: i64,
    pub labels: Vec<String>,
    /// `public` or `members`.
    pub visibility: String,
    /// The licence's id (round 5); empty is [`DEFAULT_LICENCE`].
    pub licence: String,
    /// "Offer this for the game" (round 5): only with a licence whose `game` the server sets.
    pub project_use: bool,
}

pub fn publish_request(signer: &Signer, f: &PublishFields) -> Request {
    let name = bare_name(&f.filename).to_string();
    let licence = if f.licence.trim().is_empty() { DEFAULT_LICENCE } else { f.licence.trim() };
    let purpose = publish_purpose(&name, licence, f.project_use);
    let body = json!({
        "filename": name,
        "title": f.title.trim(),
        "description": f.description.trim(),
        "category_id": f.category_id,
        "labels": f.labels,
        "visibility": if f.visibility == "members" { "members" } else { "public" },
        "licence": licence,
        "project_use": f.project_use,
    });
    post("/api/files", purpose.clone(), signer.signed(&purpose, body))
}

pub fn remove_request(signer: &Signer, target: &RemoveTarget) -> Request {
    let purpose = remove_purpose(target);
    let body = match target {
        RemoveTarget::Item(r) => json!({ "id": r.id, "filename": bare_name(&r.filename) }),
        RemoveTarget::Upload(name) => json!({ "filename": bare_name(name) }),
    };
    post("/api/files/remove", purpose.clone(), signer.signed(&purpose, body))
}

/// Pick (`on`) or unpick an item: one purpose for both, as the relay checks it.
pub fn pick_request(signer: &Signer, item: &ItemRef, on: bool) -> Request {
    let purpose = pick_purpose(item.id);
    let path = if on { "/api/files/pick" } else { "/api/files/unpick" };
    post(path, purpose.clone(), signer.signed(&purpose, json!({ "id": item.id, "filename": bare_name(&item.filename) })))
}

/// Feature an item on the main shelf, its category's only, or neither (round 5): admins.
pub fn feature_request(signer: &Signer, item: &ItemRef, to: Featured) -> Request {
    let purpose = feature_purpose(item.id, to);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "featured": to.word() });
    post("/api/files/feature", purpose.clone(), signer.signed(&purpose, body))
}

/// Keep an item on the server (`kept`) or stop keeping it (A7): admins and moderators.
pub fn keep_request(signer: &Signer, item: &ItemRef, kept: bool) -> Request {
    let purpose = keep_purpose(item.id, kept);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "kept": kept });
    post("/api/files/keep", purpose.clone(), signer.signed(&purpose, body))
}

pub fn suggest_request(signer: &Signer, name: &str, note: &str) -> Request {
    post("/api/files/suggest", SUGGEST.to_string(), signer.signed(SUGGEST, json!({ "name": name.trim(), "note": note.trim() })))
}

pub fn suggestions_request(signer: &Signer) -> Request {
    post("/api/files/suggestions", SUGGESTIONS.to_string(), signer.signed(SUGGESTIONS, json!({})))
}

pub fn decide_request(signer: &Signer, id: i64, accept: bool) -> Request {
    let purpose = decide_purpose(id, accept);
    post("/api/files/suggestions/decide", purpose.clone(), signer.signed(&purpose, json!({ "id": id, "accept": accept })))
}

pub fn usage_request(signer: &Signer) -> Request {
    post("/api/files/usage", USAGE.to_string(), signer.signed(USAGE, json!({})))
}

/// A member's own use of the Media space, waiting uploads included (A4).
pub fn mine_request(signer: &Signer) -> Request {
    post("/api/files/mine", MINE.to_string(), signer.signed(MINE, json!({})))
}

/// One category in an admin's whole list: an existing one by id, or a new one with none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CategoryEdit {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
}

/// The whole category list. The `categories` member's text is written once and both hashed into
/// the purpose and put in the body as it is, so the relay hashes exactly the same characters.
pub fn categories_request(signer: &Signer, list: &[CategoryEdit]) -> Request {
    let cats: Vec<Value> = list
        .iter()
        .map(|c| {
            let mut v = json!({ "name": c.name.trim(), "description": c.description.trim() });
            if let Some(id) = c.id {
                v["id"] = json!(id);
            }
            v
        })
        .collect();
    let cats_text = Value::Array(cats).to_string();
    let purpose = categories_purpose(&cats_text);
    let ts = next_timestamp();
    let sig = crate::net::identity::pq_sign_chat(&signer.seed, &purpose, ts);
    let body = format!(
        "{{\"key\":{},\"timestamp\":{ts},\"sig\":{},\"categories\":{cats_text}}}",
        Value::from(signer.key.as_str()),
        Value::from(sig)
    );
    Request { method: Method::Post, path: "/api/files/categories".to_string(), body: Some(body), purpose: Some(purpose) }
}

/// The header the members' file route takes this session's upload token in (the relay's W3b).
/// The token is never put in an address: an address ends up in error words, logs and lists.
pub const TOKEN_HEADER: &str = "X-Upload-Token";

/// Where a Media item's file is fetched from: its public address, or, for a members-only item,
/// the members' file route, asked with [`token_header`].
pub fn file_url(base: &str, item: &Item) -> String {
    let base = base.trim_end_matches('/');
    if item.members_only() {
        format!("{base}/api/files/file/{}", item.id)
    } else if item.url.starts_with("http://") || item.url.starts_with("https://") {
        item.url.clone()
    } else {
        format!("{base}{}", item.url)
    }
}

/// The header a fetch of `item`'s file carries: this session's upload token for a members-only
/// item, nothing for a public one.
pub fn token_header(item: &Item, token: Option<&str>) -> Option<(&'static str, String)> {
    item.members_only().then(|| (TOKEN_HEADER, token.unwrap_or("").to_string()))
}

// ── Sending ────────────────────────────────────────────────────────────────────────────────

/// A request the server refused or could not be sent: the HTTP status (0 when the server could
/// not be reached) and the words to show, with any upload token taken out of them.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub status: u16,
    pub words: String,
}

impl Refusal {
    pub fn new(status: u16, words: impl Into<String>) -> Self {
        Refusal { status, words: crate::gui::widgets::image_cache::hide_token(&words.into()) }
    }

    /// The relay's 409 to an action on an item: the tile was out of date (W5).
    pub fn stale(&self) -> bool {
        self.status == 409
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.words)
    }
}

/// Send `req` to the server at `base` (an `https://host` address) and read its JSON answer. A
/// refusal comes back with its status and the server's own words. Blocking: call it on a worker
/// thread.
pub fn send(base: &str, req: &Request) -> Result<Value, Refusal> {
    let url = format!("{}{}", base.trim_end_matches('/'), req.path);
    let result = match (req.method, req.body.as_deref()) {
        (Method::Get, _) => ureq::get(&url).timeout(std::time::Duration::from_secs(30)).call(),
        (Method::Post, body) => ureq::post(&url)
            .timeout(std::time::Duration::from_secs(60))
            .set("Content-Type", "application/json")
            .send_string(body.unwrap_or("{}")),
    };
    let resp = result.map_err(|e| match e {
        ureq::Error::Status(code, r) => {
            let text = r.into_string().unwrap_or_default();
            let text = text.trim();
            if text.is_empty() || text.starts_with('<') {
                Refusal::new(code, format!("The server refused it ({code})."))
            } else {
                Refusal::new(code, text)
            }
        }
        other => Refusal::new(0, format!("The server could not be reached ({other}).")),
    })?;
    let text = resp.into_string().map_err(|e| Refusal::new(0, format!("The answer could not be read ({e}).")))?;
    serde_json::from_str(&text).map_err(|e| Refusal::new(0, format!("The answer was not understood ({e}).")))
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
