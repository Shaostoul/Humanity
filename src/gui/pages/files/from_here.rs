//! What this computer put on Files pages (2026-10-18, for the Files page's "Your folders" tab,
//! files/your_folders.rs): a PRIVATE record, one row each time a file picked on this computer went
//! onto a server's Files page, as its own item or as a file added to one. Each row says which
//! server, which item (its number and its title), and which file: its path, its size and when it
//! was last changed, as they were when it was picked. Your folders marks a file "On
//! united-humanity.us as Dusk" while all three still match; a file whose size or date changed since
//! is a different file now and is not marked.
//!
//! Written when a publish (or an add to an item) from a local file finishes (uploading.rs
//! `published`), from what the upload's record kept of the file it was picked from
//! (`resumable::Unfinished::origin`; the record's own `path` follows a cleaned copy when there is
//! one, and the cleaned copy is not the person's file).
//!
//! KEPT ON THIS COMPUTER ONLY: in `media_from_here.json` beside `config.json`, never sent anywhere,
//! not even to the server it names (a list of someone's files and folders is nobody else's
//! business), and at most [`MAX_ROWS`] rows, the oldest going first. Nothing is written while the
//! storage choice is still undecided on a first boot; the record then lasts for that run only.
//!
//! This is the quick, local way to know. The later, server-checked way is the Media design's phase
//! 2 fingerprints (docs/design/media-round-5-plan.md): a file's own fingerprint asked of the server,
//! which also knows a copy that was renamed or moved, and a file someone else put there.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::gui::widgets::file_browser::FsEntry;

/// At most this many rows are kept; past it the oldest go first.
pub const MAX_ROWS: usize = 5000;

/// The file the record is kept in, beside `config.json`.
pub const FILE_NAME: &str = "media_from_here.json";

/// A file on this computer as it was when it was picked: where it is, its size, and when it was
/// last changed (ms since 1970, 0 when the system cannot say).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    pub path: PathBuf,
    pub size: u64,
    pub modified_ms: u64,
}

impl Local {
    /// The file at `path` as it is now.
    pub fn of(path: &Path) -> Local {
        let meta = std::fs::metadata(path).ok();
        Local { path: path.to_path_buf(), size: meta.as_ref().map(|m| m.len()).unwrap_or(0), modified_ms: ms(meta.and_then(|m| m.modified().ok())) }
    }

    /// A folder listing's file as it is now (the listing read its size and date).
    pub fn of_entry(e: &FsEntry) -> Local {
        Local { path: e.path.clone(), size: e.size, modified_ms: ms(e.modified) }
    }

    /// Whether `other` is this same file, unchanged: the same path (spelled any way the system
    /// treats as the same), the same size, the same date.
    pub fn same_file(&self, other: &Local) -> bool {
        self.size == other.size && self.modified_ms == other.modified_ms && path_key(&self.path) == path_key(&other.path)
    }
}

/// A time as ms since 1970, the way `resumable::modified_ms` counts it; 0 when not known.
fn ms(t: Option<std::time::SystemTime>) -> u64 {
    t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A path spelled the way it is compared: on Windows in lower case, since `E:\Media\Dusk.MP4` and
/// `e:\media\dusk.mp4` are the same file there.
fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// One file put on one server's Files page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sent {
    /// The server, its normalised address (`media::server_here`).
    pub server: String,
    /// The item it is, or was added to: its number (0 when the server's answer did not say) and its
    /// title as it was then (empty when not known).
    pub item: i64,
    pub title: String,
    /// The file it came from, as it was picked.
    pub file: Local,
    /// When it went, ms since 1970.
    pub at_ms: u64,
}

/// The whole record.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// Oldest first.
    pub sent: Vec<Sent>,
    /// Bumped on every change, so a folder shown on the page marks a file the moment its publish
    /// finishes. Not kept in the file.
    #[serde(skip)]
    pub generation: u64,
}

impl Record {
    /// Add a row. The same file put on the same item of the same server again replaces its row;
    /// past [`MAX_ROWS`] the oldest go.
    pub fn add(&mut self, sent: Sent) {
        let key = path_key(&sent.file.path);
        self.sent.retain(|s| !(s.server == sent.server && s.item == sent.item && path_key(&s.file.path) == key));
        self.sent.push(sent);
        if self.sent.len() > MAX_ROWS {
            let extra = self.sent.len() - MAX_ROWS;
            self.sent.drain(..extra);
        }
        self.generation += 1;
    }

    /// The rows that mark each of `entries` (files only), newest first, by the entry's path: only a
    /// file still the same size and date as when it was picked.
    pub fn marks_for(&self, entries: &[FsEntry]) -> HashMap<PathBuf, Vec<Sent>> {
        let mut by_path: HashMap<String, Vec<&Sent>> = HashMap::new();
        for s in &self.sent {
            by_path.entry(path_key(&s.file.path)).or_default().push(s);
        }
        let mut out = HashMap::new();
        for e in entries.iter().filter(|e| !e.is_dir) {
            let Some(rows) = by_path.get(&path_key(&e.path)) else { continue };
            let now = Local::of_entry(e);
            let marks: Vec<Sent> = rows.iter().rev().filter(|s| s.file.same_file(&now)).map(|s| (*s).clone()).collect();
            if !marks.is_empty() {
                out.insert(e.path.clone(), marks);
            }
        }
        out
    }
}

/// Note that the upload `record` (one of Add to Files's, or one added to an item) went onto its
/// server's Files page: `answer` is the server's answer, whose `item` names the item. Nothing is
/// noted for an upload that kept no file it was picked from.
pub(crate) fn note_published(record: &super::resumable::Unfinished, answer: &serde_json::Value) {
    let Some(file) = record.origin.clone().filter(|o| !o.path.as_os_str().is_empty()) else { return };
    let item = answer.get("item").and_then(super::api::Item::from_json);
    let sent = Sent {
        server: record.server.clone(),
        item: item.as_ref().map(|i| i.id).unwrap_or(0),
        title: item.as_ref().map(|i| i.display_title()).unwrap_or_default(),
        file,
        at_ms: ms(Some(std::time::SystemTime::now())),
    };
    with(|r| r.add(sent));
}

/// Read the record from `path`; nothing there, or something unreadable, is an empty record.
pub fn load(path: &Path) -> Record {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Write the record to `path`, whole or not at all (a temporary file, then a rename).
pub fn save(path: &Path, record: &Record) -> std::io::Result<()> {
    let text = serde_json::to_string(record).map_err(std::io::Error::other)?;
    crate::persistence::write_atomic(path, text.as_bytes()).map_err(std::io::Error::other)
}

/// Where the record is kept: beside `config.json`, or nowhere while the storage choice is still
/// undecided (nothing may be written yet).
#[cfg(not(test))]
fn store_path() -> Option<PathBuf> {
    if crate::storage::mode() == crate::storage::StorageMode::Undecided {
        return None;
    }
    Some(crate::config::AppConfig::config_path().with_file_name(FILE_NAME))
}

#[cfg(not(test))]
static RECORD: std::sync::Mutex<Option<Record>> = std::sync::Mutex::new(None);

// Tests keep their own record per test thread, so no test reads another's rows or writes the
// person's real file.
#[cfg(test)]
thread_local! {
    static RECORD: std::cell::RefCell<Record> = std::cell::RefCell::new(Record::default());
}

/// Read or change the record. A change is saved at once.
pub fn with<R>(f: impl FnOnce(&mut Record) -> R) -> R {
    #[cfg(not(test))]
    {
        let mut guard = RECORD.lock().unwrap_or_else(|e| e.into_inner());
        let path = store_path();
        let record = guard.get_or_insert_with(|| path.as_deref().map(load).unwrap_or_default());
        let before = record.generation;
        let answer = f(record);
        if record.generation != before {
            if let Some(path) = path {
                if let Err(e) = save(&path, record) {
                    log::warn!("Files: could not save {}: {e}", path.display());
                }
            }
        }
        answer
    }
    #[cfg(test)]
    {
        RECORD.with(|r| f(&mut r.borrow_mut()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &Path) -> FsEntry {
        let m = std::fs::metadata(path).unwrap();
        FsEntry { name: path.file_name().unwrap().to_string_lossy().to_string(), path: path.to_path_buf(), is_dir: false, size: m.len(), modified: m.modified().ok() }
    }

    /// THE RECORD MARKS A FILE ONLY WHILE IT IS UNCHANGED, KEEPS AT MOST ITS LIMIT, AND READS BACK:
    /// a file picked and sent is marked; the same file sent to the same item again is one row; once
    /// its size changes it is not marked; past the limit the oldest go; saved and read back it is the
    /// same; a damaged file reads as empty.
    ///
    /// Seen red 2026-10-18 with `same_file` comparing the path alone: "a file whose size changed is
    /// not marked" found the old mark.
    #[test]
    fn the_record_marks_a_file_only_while_it_is_unchanged() {
        let dir = crate::test_temp::dir("from_here_rows");
        let file = dir.join("dusk.mp4");
        std::fs::write(&file, b"0123456789").unwrap();
        let mut r = Record::default();
        let sent = Sent { server: "https://h".into(), item: 7, title: "Dusk".into(), file: Local::of(&file), at_ms: 1 };
        r.add(sent.clone());
        r.add(Sent { at_ms: 2, ..sent.clone() });
        assert_eq!(r.sent.len(), 1, "the same file on the same item is one row");
        let marks = r.marks_for(&[entry(&file)]);
        assert_eq!(marks.get(&file).map(|m| m[0].title.as_str()), Some("Dusk"), "an unchanged file is marked");
        std::fs::write(&file, b"0123456789 and more").unwrap();
        assert!(r.marks_for(&[entry(&file)]).is_empty(), "a file whose size changed is not marked");

        for i in 0..MAX_ROWS + 3 {
            r.add(Sent { item: 100 + i as i64, ..sent.clone() });
        }
        assert_eq!(r.sent.len(), MAX_ROWS, "at most the limit is kept");
        assert_eq!(r.sent[0].item, 103, "the oldest went first");

        let path = dir.join(FILE_NAME);
        save(&path, &r).expect("saved");
        assert_eq!(load(&path).sent, r.sent, "read back");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).sent.is_empty(), "a damaged file reads as empty");
    }
}
