//! Tasks of my own ("Mine", docs/design/tasks.md rung 1, 2026-11-02; fixes BUG-217): the records,
//! the merge, the notes to myself that carry them between my devices, and the encrypted file that
//! keeps them on this one. No socket and no drawing, so every rule here has a unit test; the half that
//! acts on the app and the sockets is src/engine/mine_tasks.rs.
//!
//! WHERE THEY LIVE. One encrypted file per identity, `<data folder>/tasks/<hash>.mine` (beside
//! config.json, so a portable install or `HUMANITY_DATA_DIR` keeps its own), written crash-safely
//! (`humanity_core::atomic_write::write_atomic`). AES-256-GCM under a key derived from the seed
//! (BLAKE3 `derive_key("hum/tasks-mine/v1", seed)`, its own domain), and the file's name is a hash
//! of the identity, so a copied file names nobody and says nothing without the seed. No server
//! holds any of it.
//!
//! A FILE THAT DOES NOT OPEN IS NEVER WRITTEN OVER (2026-11-02, the review of rung 1). Only a file
//! that is not there is an ordinary empty start. One that is there but does not open (damaged, or
//! not ours) is renamed aside first, to `<name>.unreadable-<unix seconds>` beside it (a rename,
//! never a delete), and the list starts empty from there; one that cannot be read at all (a lock,
//! a permission), or cannot be set aside, keeps this session from saving over it. Either way the
//! page says so ([`MineTasks::problem`]), and changes still go to my other devices as notes.
//!
//! HOW THEY TRAVEL. A change goes to my other devices as a sealed note to myself, the block
//! list's own way (net/block_list.rs): `[[hum:task:v1]]{"tasks":[...]}`, one or more records,
//! signed by me, sealed to my own key, in my own mailbox on every server this device is connected
//! to. A server sees padded sealed mail to my own mailbox, never that it is a task.
//!
//! THE MERGE (the same rule as web/shared/mine-tasks.js): per id the record with the larger
//! `updated_at` wins; at the same time a deletion wins, then the later status, then the higher
//! priority; otherwise what is held stays. Order of arrival does not matter and a repeat changes
//! nothing, so no "seen" list is kept: my own note coming back, or a relay delivering one again,
//! is harmless. A deletion is a tombstone with no words, kept so an older note cannot bring the
//! task back.

use std::path::{Path, PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};

use crate::gui::{GuiTask, TaskPriority, TaskStatus};
use crate::net::dm_pq::CTL_TASK;

/// The longest title, in characters (the website's form has the same `maxlength`).
pub const TITLE_MAX: usize = 200;
/// The longest description, in characters.
pub const DESCRIPTION_MAX: usize = 3_000;
/// At most this many labels, each at most `LABEL_MAX` characters.
pub const LABELS_MAX: usize = 12;
pub const LABEL_MAX: usize = 32;
/// A chain id (data/onboarding/quests.json): 1 to 64 of a-z, 0-9, '-' and '_'.
const CHAIN_MAX: usize = 64;
/// The most text one note carries. With the inner payload's two keys and signature (about
/// 12.3 KB) a note then fits the largest padding bucket (32,768, `dm_pq::DM_PAD_BUCKETS`), and a
/// single record always fits: the limits above keep the largest one far below this.
pub const NOTE_TEXT_MAX: usize = 19_000;

/// BLAKE3 domain for the file's encryption key. Distinct from every other seed-derived key.
const FILE_KEY_DOMAIN: &str = "hum/tasks-mine/v1";

impl TaskPriority {
    /// The word a task record and the server use (docs/design/tasks.md section 2).
    pub fn as_str(self) -> &'static str {
        match self {
            TaskPriority::Low => "low",
            TaskPriority::Medium => "medium",
            TaskPriority::High => "high",
            TaskPriority::Critical => "critical",
        }
    }

    /// Read that word; anything else is Medium.
    pub fn from_word(word: &str) -> Self {
        match word {
            "low" => TaskPriority::Low,
            "high" => TaskPriority::High,
            "critical" => TaskPriority::Critical,
            _ => TaskPriority::Medium,
        }
    }

    fn rank(self) -> u8 {
        self as u8
    }
}

impl TaskStatus {
    /// The word a task record uses: "todo", "in_progress" or "done".
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Done => "done",
        }
    }

    /// Read a status word. The board's "backlog" and "testing" have no column here, so they and
    /// anything else unknown are Todo.
    pub fn from_word(word: &str) -> Self {
        match word {
            "in_progress" => TaskStatus::InProgress,
            "done" => TaskStatus::Done,
            _ => TaskStatus::Todo,
        }
    }

    fn rank(self) -> u8 {
        self as u8
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// One task as a note and the file carry it (docs/design/tasks.md section 2). The fields are
/// written in this order and empty ones are left out, exactly as the website writes them
/// (scripts/tests/fixtures/task-note-v1.json holds the two apps to the same bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deleted: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub priority: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

/// Words kept: no control characters but line breaks and tabs, at most `max` characters.
fn clean(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').take(max).collect()
}

/// A task id: exactly 32 lowercase hex.
pub fn is_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_chain(id: &str) -> bool {
    !id.is_empty() && id.len() <= CHAIN_MAX && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// A new task id: 128 random bits from the operating system.
pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        // No OS randomness (never seen on a desktop): the time and a hash still give an id no
        // other device picks by chance.
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        bytes.copy_from_slice(&blake3::hash(&nanos.to_le_bytes()).as_bytes()[..16]);
    }
    hex::encode(bytes)
}

impl Record {
    /// The record as it may be kept: None when its id is not one. A deletion keeps no words; the
    /// words are clamped to the limits and the status and priority to the known words.
    pub fn clamped(self) -> Option<Record> {
        if !is_id(&self.id) {
            return None;
        }
        if self.deleted {
            return Some(Record::gone(&self.id, self.updated_at));
        }
        let labels: Vec<String> = self
            .labels
            .iter()
            .map(|l| clean(l, LABEL_MAX).trim().to_string())
            .filter(|l| !l.is_empty())
            .take(LABELS_MAX)
            .collect();
        Some(Record {
            id: self.id,
            deleted: false,
            title: clean(&self.title, TITLE_MAX),
            description: clean(&self.description, DESCRIPTION_MAX),
            status: TaskStatus::from_word(&self.status).as_str().to_string(),
            priority: TaskPriority::from_word(&self.priority).as_str().to_string(),
            labels,
            chain: self.chain.filter(|c| is_chain(c)),
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }

    /// A deletion: the id and when, nothing else.
    pub fn gone(id: &str, at: u64) -> Record {
        Record {
            id: id.to_string(),
            deleted: true,
            title: String::new(),
            description: String::new(),
            status: String::new(),
            priority: String::new(),
            labels: Vec::new(),
            chain: None,
            created_at: 0,
            updated_at: at,
        }
    }

    /// The record of a task held here.
    pub fn of(task: &GuiTask) -> Record {
        Record {
            id: task.id.clone(),
            deleted: false,
            title: task.title.clone(),
            description: task.description.clone(),
            status: task.status.as_str().to_string(),
            priority: task.priority.as_str().to_string(),
            labels: task.labels.clone(),
            chain: task.chain.clone(),
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }

    /// The page's task for a live record.
    fn task(&self) -> GuiTask {
        GuiTask {
            id: self.id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            priority: TaskPriority::from_word(&self.priority),
            status: TaskStatus::from_word(&self.status),
            assignee: String::new(),
            labels: self.labels.clone(),
            chain: self.chain.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// What decides which of two versions wins, compared in order.
    fn rank(&self) -> (u64, bool, u8, u8) {
        if self.deleted {
            return (self.updated_at, true, 0, 0);
        }
        (self.updated_at, false, TaskStatus::from_word(&self.status).rank(), TaskPriority::from_word(&self.priority).rank())
    }
}

/// Does `a` win over `b` (the same task)? Section 2 of the plan: the later change; at the same
/// time a deletion, then the later status, then the higher priority; a tie keeps `b`.
pub fn wins(a: &Record, b: &Record) -> bool {
    a.rank() > b.rank()
}

/// Does this DM text start with the marker? Such a text is never shown as a message, whether or
/// not the rest reads.
pub fn is_note(text: &str) -> bool {
    text.starts_with(CTL_TASK)
}

/// The notes that carry `records`, in order, as many records in each as fit `NOTE_TEXT_MAX`.
pub fn note_texts(records: &[Record]) -> Vec<String> {
    const HEAD: &str = "{\"tasks\":[";
    const TAIL: &str = "]}";
    let mut notes = Vec::new();
    let mut body = String::new();
    for record in records {
        let Ok(json) = serde_json::to_string(record) else { continue };
        let fits = CTL_TASK.len() + HEAD.len() + body.len() + 1 + json.len() + TAIL.len() <= NOTE_TEXT_MAX;
        if !body.is_empty() && !fits {
            notes.push(format!("{CTL_TASK}{HEAD}{body}{TAIL}"));
            body.clear();
        }
        if !body.is_empty() {
            body.push(',');
        }
        body.push_str(&json);
    }
    if !body.is_empty() {
        notes.push(format!("{CTL_TASK}{HEAD}{body}{TAIL}"));
    }
    notes
}

/// The records a note carries, each clamped; those that do not read are left out. Empty for a
/// text that is not a note, or whose body does not read.
pub fn parse_note(text: &str) -> Vec<Record> {
    let Some(json) = text.strip_prefix(CTL_TASK) else { return Vec::new() };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let Some(list) = value.get("tasks").and_then(|t| t.as_array()) else { return Vec::new() };
    list.iter()
        .filter_map(|r| serde_json::from_value::<Record>(r.clone()).ok())
        .filter_map(Record::clamped)
        .collect()
}

/// What is encrypted into the file.
#[derive(Default, Serialize, Deserialize)]
struct Body {
    /// Every task held, live ones and deletions, as records.
    #[serde(default)]
    records: Vec<Record>,
    /// Ids whose latest version has not gone out as a note yet (made while no server was
    /// connected), oldest first.
    #[serde(default)]
    unsent: Vec<String>,
}

/// Why the tasks kept on this device did not open as they were (the Tasks page says so).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenProblem {
    /// The file was there but did not open (damaged, or not ours): it was renamed aside to this
    /// path, kept as it was, and the list started empty.
    KeptAside(PathBuf),
    /// The file could not be read (for a reason other than not being there), or could not be set
    /// aside: nothing is saved over it until the app starts again. Why, in the system's words.
    Unreadable(String),
}

/// The tasks of my own on this device, for one identity.
pub struct MineTasks {
    /// None for a list kept in memory only (a page snapshot): it is never written.
    path: Option<PathBuf>,
    key: [u8; 32],
    me: String,
    /// The live tasks, in the order they were made here or arrived.
    tasks: Vec<GuiTask>,
    /// Deletions: id and when.
    gone: Vec<(String, u64)>,
    unsent: Vec<String>,
    /// Set when the file did not open as it was (see the module's words); `Unreadable` stops
    /// every save.
    problem: Option<OpenProblem>,
}

/// The body of a file's bytes, or None when they do not open (too short, not this key's, or not
/// the body's JSON).
fn open_body(key: &[u8; 32], raw: &[u8]) -> Option<Body> {
    if raw.len() <= 12 {
        return None;
    }
    let (nonce, ct) = raw.split_at(12);
    let plain = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)).decrypt(Nonce::from_slice(nonce), ct).ok()?;
    serde_json::from_slice::<Body>(&plain).ok()
}

/// Rename a file that does not open to `<name>.unreadable-<unix seconds>` beside it (another
/// number after it when that name is taken), so nothing is ever written over it.
fn set_aside(path: &Path) -> OpenProblem {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut aside = path.with_file_name(format!("{name}.unreadable-{secs}"));
    let mut n = 1;
    while aside.exists() {
        aside = path.with_file_name(format!("{name}.unreadable-{secs}-{n}"));
        n += 1;
    }
    match std::fs::rename(path, &aside) {
        Ok(()) => {
            log::warn!("[tasks] the file of my own tasks did not open; kept aside as {}", aside.display());
            OpenProblem::KeptAside(aside)
        }
        Err(e) => {
            log::warn!("[tasks] the file of my own tasks did not open and could not be set aside ({e}); not saving over it");
            OpenProblem::Unreadable(format!("could not be set aside: {e}"))
        }
    }
}

impl MineTasks {
    /// Load (or start empty) the tasks of `identity_hex`, keyed by its seed.
    pub fn load(seed: &[u8], identity_hex: &str) -> Self {
        let tag = blake3::hash(format!("tasks-mine\n{identity_hex}").as_bytes());
        let dir = crate::config::AppConfig::config_path().with_file_name("tasks");
        Self::load_at(seed, identity_hex, Some(dir.join(format!("{}.mine", &tag.to_hex()[..24]))))
    }

    fn load_at(seed: &[u8], identity_hex: &str, path: Option<PathBuf>) -> Self {
        let mut list = Self {
            path,
            key: blake3::derive_key(FILE_KEY_DOMAIN, seed),
            me: identity_hex.to_string(),
            tasks: Vec::new(),
            gone: Vec::new(),
            unsent: Vec::new(),
            problem: None,
        };
        let Some(path) = list.path.clone() else { return list };
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            // Only a file that is not there is an ordinary empty start.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return list,
            // Anything else (a lock, a permission) may pass: never save over the file this session.
            Err(e) => {
                log::warn!("[tasks] the file of my own tasks could not be read ({e}); not saving over it");
                list.problem = Some(OpenProblem::Unreadable(e.to_string()));
                return list;
            }
        };
        match open_body(&list.key, &raw) {
            Some(body) => {
                list.apply(body.records);
                list.unsent = body.unsent;
            }
            // There but not opening: kept aside before anything is written, and an empty start.
            None => list.problem = Some(set_aside(&path)),
        }
        list
    }

    /// A list over the file at `path`, as a restart reads it (tests).
    #[cfg(test)]
    pub(crate) fn at_path_for_test(seed: &[u8], identity_hex: &str, path: PathBuf) -> Self {
        Self::load_at(seed, identity_hex, Some(path))
    }

    /// Why the tasks kept on this device did not open as they were, if they did not.
    pub fn problem(&self) -> Option<&OpenProblem> {
        self.problem.as_ref()
    }

    /// A list in its own file under the temp directory, for tests (never the real data folder).
    #[cfg(test)]
    pub(crate) fn in_temp(seed: &[u8], identity_hex: &str, tag: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        Self::load_at(seed, identity_hex, Some(std::env::temp_dir().join(format!("hum-tasks-test-{tag}-{nanos}.mine"))))
    }

    /// The same file again, as a restart reads it (tests).
    #[cfg(test)]
    pub(crate) fn reopened(&self, seed: &[u8]) -> Self {
        Self::load_at(seed, &self.me, self.path.clone())
    }

    #[cfg(test)]
    pub(crate) fn remove_file_for_test(&self) {
        if let Some(p) = self.path.as_ref() {
            let _ = std::fs::remove_file(p);
        }
    }

    #[cfg(test)]
    pub(crate) fn file_bytes_for_test(&self) -> Vec<u8> {
        self.path.as_ref().and_then(|p| std::fs::read(p).ok()).unwrap_or_default()
    }

    /// A list never written anywhere (the page snapshots draw sample tasks with it).
    pub fn in_memory(identity_hex: &str) -> Self {
        Self::load_at(&[0u8; 32], identity_hex, None)
    }

    /// Persist: encrypt, then write crash-safely. Called after every change.
    pub fn save(&self) {
        let Some(path) = self.path.as_ref() else { return };
        // A file that could not be read is never saved over (it may be whole, behind a lock).
        if matches!(self.problem, Some(OpenProblem::Unreadable(_))) {
            return;
        }
        let body = Body { records: self.records(), unsent: self.unsent.clone() };
        let Ok(json) = serde_json::to_vec(&body) else { return };
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut AesOsRng);
        let Ok(ct) = cipher.encrypt(&nonce, json.as_slice()) else { return };
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = humanity_core::atomic_write::write_atomic(path, &out) {
            log::warn!("[tasks] my own tasks were not saved: {e}");
        }
    }

    /// Whose tasks these are.
    pub fn identity(&self) -> &str {
        &self.me
    }

    /// The live tasks, as the page draws them.
    pub fn tasks(&self) -> &[GuiTask] {
        &self.tasks
    }

    pub fn get(&self, id: &str) -> Option<&GuiTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Is a live task made from this chain?
    pub fn has_chain(&self, chain: &str) -> bool {
        self.tasks.iter().any(|t| t.chain.as_deref() == Some(chain))
    }

    /// The current record of `id` (live or deleted), if held.
    pub fn record(&self, id: &str) -> Option<Record> {
        if let Some(t) = self.get(id) {
            return Some(Record::of(t));
        }
        self.gone.iter().find(|(g, _)| g == id).map(|(g, at)| Record::gone(g, *at))
    }

    /// Every record held, live ones first.
    pub fn records(&self) -> Vec<Record> {
        self.tasks.iter().map(Record::of).chain(self.gone.iter().map(|(id, at)| Record::gone(id, *at))).collect()
    }

    /// The time a change made here now carries: now, or just after the version it changes.
    fn stamp(&self, id: &str, now: u64) -> u64 {
        let before = self.record(id).map(|r| r.updated_at).unwrap_or(0);
        now.max(before + 1)
    }

    /// A new task made on this device at `now`. Its words are clamped; an empty title is "".
    /// Returns its id.
    pub fn add(&mut self, title: &str, description: &str, priority: TaskPriority, labels: Vec<String>, chain: Option<String>, now: u64) -> String {
        let id = new_id();
        let record = Record {
            id: id.clone(),
            deleted: false,
            title: title.to_string(),
            description: description.to_string(),
            status: TaskStatus::Todo.as_str().to_string(),
            priority: priority.as_str().to_string(),
            labels,
            chain,
            created_at: now,
            updated_at: now,
        };
        if let Some(r) = record.clamped() {
            self.put(r);
        }
        id
    }

    /// Move a task to another column and/or give it another priority, at `now`. False when it is
    /// not held or nothing changed.
    pub fn change(&mut self, id: &str, status: Option<TaskStatus>, priority: Option<TaskPriority>, now: u64) -> bool {
        let Some(task) = self.get(id) else { return false };
        let new_status = status.unwrap_or(task.status);
        let new_priority = priority.unwrap_or(task.priority);
        if new_status == task.status && new_priority == task.priority {
            return false;
        }
        let at = self.stamp(id, now);
        let mut record = Record::of(task);
        record.status = new_status.as_str().to_string();
        record.priority = new_priority.as_str().to_string();
        record.updated_at = at;
        self.put(record);
        true
    }

    /// Delete a task at `now`: it leaves the list and a deletion takes its place. False when no
    /// live task has that id.
    pub fn delete(&mut self, id: &str, now: u64) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        let at = self.stamp(id, now);
        self.put(Record::gone(id, at));
        true
    }

    /// Put a version in place of whatever is held for its id (the caller decided it wins). The
    /// live tasks stay in the order they were made (then by id), so every device lists them the
    /// same way whatever order the notes arrived in.
    fn put(&mut self, record: Record) {
        self.gone.retain(|(g, _)| g != &record.id);
        if record.deleted {
            self.tasks.retain(|t| t.id != record.id);
            self.gone.push((record.id, record.updated_at));
            return;
        }
        let task = record.task();
        if let Some(slot) = self.tasks.iter_mut().find(|t| t.id == task.id) {
            if slot.created_at == task.created_at {
                *slot = task;
                return;
            }
        }
        self.tasks.retain(|t| t.id != task.id);
        let at = self.tasks.partition_point(|t| (t.created_at, t.id.as_str()) <= (task.created_at, task.id.as_str()));
        self.tasks.insert(at, task);
    }

    /// Apply records (from a note, or the file): each replaces what is held when it wins. True
    /// when anything changed (the caller saves).
    pub fn apply(&mut self, records: Vec<Record>) -> bool {
        let mut changed = false;
        for record in records.into_iter().filter_map(Record::clamped) {
            let wins_here = match self.record(&record.id) {
                Some(held) => wins(&record, &held),
                None => true,
            };
            if wins_here {
                self.put(record);
                changed = true;
            }
        }
        changed
    }

    /// Remember that the latest versions of `ids` have not gone out yet.
    pub fn mark_unsent(&mut self, ids: &[String]) {
        for id in ids {
            if !self.unsent.contains(id) {
                self.unsent.push(id.clone());
            }
        }
    }

    /// Take the ids waiting to go out (the caller marks again any that could not).
    pub fn take_unsent(&mut self) -> Vec<String> {
        std::mem::take(&mut self.unsent)
    }

    /// The latest versions of `ids` went out: they no longer wait.
    pub fn sent(&mut self, ids: &[String]) {
        self.unsent.retain(|id| !ids.contains(id));
    }

    pub fn unsent(&self) -> &[String] {
        &self.unsent
    }
}

#[cfg(test)]
#[path = "mine_tasks_tests.rs"]
mod tests;
