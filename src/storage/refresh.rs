//! Keeping an installed data folder current after an update (BUG-163 part 2, built 2026-10-12;
//! design: docs/design/data-folder-updates.md).
//!
//! An installed game writes its data folder on its first run, and an update replaces only the
//! exe. Before this, nothing ever wrote the folder again, so it kept the files of whichever
//! version first ran on the machine: a ship file an updated server refuses as "a different
//! ship", an item list missing every item added since, a starting kit with nothing in it.
//!
//! What happens now:
//! - Whenever the game writes a shipped file into the folder (the first run, a refresh, or "Use
//!   the game's copy" in Settings > Data) it records the BLAKE3 hash of exactly the bytes it
//!   wrote, in the stamp `data/.shipped.json`, with the version they came from (`Stamp`).
//! - On a start whose exe ships different data from what the stamp records (another version, or
//!   another build of the same version), every shipped file is judged (`refresh`):
//!   - it already holds this exe's copy: nothing to do;
//!   - it still holds exactly what the game last wrote there: replaced with this exe's copy;
//!   - it is absent: written, so a modder finds every shipped file in the folder;
//!   - anything else was edited, by the player or a mod: KEPT as it is (the operator's decision,
//!     2026-10-12: a player's edit is never overwritten), and Settings > Data lists it with "Use
//!     the game's copy", which moves the player's file aside and never deletes it.
//!   A file the folder holds that this exe does not ship is left alone: nothing reads it.
//! - A folder written before the stamp existed has no record of what was written, so it is
//!   judged by when its files were written (`first_run_files`): a first run writes them all
//!   within seconds, so a file written then and not changed since is unedited, and one changed
//!   later is kept.
//! - A folder that is a source tree, or that is reached through a link (every rig's data folder
//!   is a junction to the checkout's own), is never touched and never stamped (`why_left_alone`).
//!   A game run from the repo reads data/ in place, and an autosave that wrote there once
//!   rewrote four tracked files (BUG-151).
//!
//! The home designs, the ship file and the machine layouts follow the same rule (the
//! orchestrator's decision of 2026-10-12, ship-homes-increment-6-plan.md section 8, item 6): one
//! nobody edited follows the new default, one the player saved from the editor is theirs and is
//! kept. Every file is replaced whole (written beside it, then renamed over it), so a start that
//! dies part-way never leaves half a file, and a pass that could not write everything keeps the
//! stamp's old version, so the next start finishes it.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The stamp's name in the data folder. A dot first, so a file listing puts it out of the way.
pub const STAMP_FILE: &str = ".shipped.json";

/// The version of this exe, the one a refresh brings the folder to.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long after the first file of a folder written before stamps a file may have been
/// written and still count as part of that first run. A first run writes every file within a
/// second or two (the operator's 90 files carry one second); the slack covers a slow disk and a
/// virus scanner checking each file as it is written. Nobody edits a data file in the first half
/// minute of a game's first run.
const FIRST_RUN_WINDOW: Duration = Duration::from_secs(30);

/// The fewest files a first run's moment must hold before a folder with no stamp is judged by it:
/// a first run writes every file the game ships (90 in July 2026), so a "moment" holding a few
/// files is not one, and nothing is judged unedited by it.
const FIRST_RUN_MIN_FILES: usize = 10;

/// What `data/.shipped.json` holds: the record of what the game wrote into the folder.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// The version whose copies the folder holds. None: a refresh of a folder written before
    /// stamps existed that could not write every file (the next start finishes it).
    #[serde(default)]
    pub version: Option<String>,
    /// A hash over every file that version ships, path and bytes (`shipped_fingerprint`), so two
    /// builds of one version that ship different data are told apart.
    #[serde(default)]
    pub shipped: String,
    /// Per file, by its path under the folder: the hash of exactly the bytes the game last wrote
    /// there. A file whose hash still matches was not edited.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// What the last refresh did, for Settings > Data. None: nothing has been refreshed since the
    /// first run wrote the folder.
    #[serde(default)]
    pub last_update: Option<Report>,
}

/// What one refresh did.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The version the folder held before. None: it was written before stamps existed.
    pub from: Option<String>,
    /// The version it was brought to.
    pub to: String,
    /// When, in Unix seconds.
    #[serde(default)]
    pub at: u64,
    /// Files that held an earlier version's copy, now this version's.
    #[serde(default)]
    pub updated: Vec<String>,
    /// Files that were absent, now written.
    #[serde(default)]
    pub added: Vec<String>,
    /// Files kept as they are because they were edited.
    #[serde(default)]
    pub kept: Vec<String>,
    /// Files that could not be read or written: the next start tries again.
    #[serde(default)]
    pub failed: Vec<NotWritten>,
}

/// A file a refresh could not bring up to date, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotWritten {
    pub file: String,
    pub why: String,
}

impl Report {
    /// Every file is this version's, or kept because it was edited.
    pub fn finished(&self) -> bool {
        self.failed.is_empty()
    }
}

/// What a start did with the folder.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// It already holds this exe's copies: nothing was read but the stamp.
    Current,
    /// It was refreshed (see the report; it may not have finished).
    Refreshed(Report),
    /// It is not the game's to change, and why (`why_left_alone`).
    LeftAlone(String),
}

/// The hash a stamp records for some bytes.
pub fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// One hash over everything a version ships (each path and the hash of its bytes, in path
/// order). Two builds of one version with different data differ here, so a refresh follows the
/// data, not only the version number.
pub fn shipped_fingerprint(shipped: &[(&str, &[u8])]) -> String {
    let mut lines: Vec<String> = shipped.iter().map(|(rel, bytes)| format!("{rel}\0{}\n", hash(bytes))).collect();
    lines.sort();
    hash(lines.concat().as_bytes())
}

fn is_link(path: &Path) -> bool {
    // A junction counts: on Windows `is_symlink` is true for every name-surrogate reparse point,
    // which a junction is (the test below makes one with mklink /J and checks).
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

/// Why `dir` is not the game's own copy to change, or None when it is (design section 4).
///
/// - `dir` itself is a link: every rig runs the game against the checkout's data/ through a
///   junction (BUG-133), and a link can lead anywhere;
/// - its parent holds Cargo.toml: it is a source tree's data/, which `find_data_dir` prefers and
///   a game run from the repo reads in place (`just launch` boots an archive from the repo root,
///   where data/ sits beside the exe);
/// - the folder it really is (following links) is one;
/// - any folder on the way to a shipped file, or the file itself, is a link: a rig that links
///   data/'s entries one by one (probe-sweep's shipped-assets mode) has a real data/ whose
///   subfolders are junctions into the checkout.
///
/// `rels`: the paths under `dir` a refresh would write.
pub fn why_left_alone(dir: &Path, rels: &[&str]) -> Option<String> {
    if is_link(dir) {
        return Some(format!("{} is a link to another folder, so it is not the game's own copy", dir.display()));
    }
    let in_source_tree = |d: &Path| d.parent().is_some_and(|p| p.join("Cargo.toml").exists());
    if in_source_tree(dir) {
        return Some("it is a source tree's own data folder (the folder above it holds Cargo.toml), which only a commit changes".into());
    }
    if let Ok(real) = std::fs::canonicalize(dir) {
        if in_source_tree(&real) {
            return Some(format!("it leads into a source tree's data folder ({}), which only a commit changes", real.display()));
        }
    }
    let mut checked: HashSet<PathBuf> = HashSet::new();
    for rel in rels.iter().copied().chain([STAMP_FILE]) {
        let mut at = dir.to_path_buf();
        let mut shown = String::new();
        for part in Path::new(rel).components() {
            at.push(part);
            if !shown.is_empty() {
                shown.push('/');
            }
            shown.push_str(&part.as_os_str().to_string_lossy());
            if checked.insert(at.clone()) && is_link(&at) {
                return Some(format!("data/{shown} in it is a link to another folder, so it is not the game's own copy"));
            }
        }
    }
    None
}

/// The stamp in `dir`, or None when there is none (or it cannot be read, which is logged: the
/// folder is then judged as one written before stamps).
pub fn read_stamp(dir: &Path) -> Option<Stamp> {
    let path = dir.join(STAMP_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            log::warn!("{} could not be read ({e}); the data folder is judged as one written before stamps", path.display());
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(stamp) => Some(stamp),
        Err(e) => {
            log::warn!("{} does not parse ({e}); the data folder is judged as one written before stamps", path.display());
            None
        }
    }
}

fn write_stamp(dir: &Path, stamp: &Stamp) -> std::io::Result<()> {
    let text = serde_json::to_vec_pretty(stamp).map_err(std::io::Error::other)?;
    write_whole(&dir.join(STAMP_FILE), &text)
}

/// Where `write_whole` writes a file's new bytes before renaming them over it: beside it, with
/// the process id in the name so two games starting at once never share one.
pub(crate) fn tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// Write `bytes` to `path` whole: to a file beside it, then renamed over it, so a start that dies
/// part-way leaves the old file or the new one, never half of either.
pub(crate) fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path(path);
    let written = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Which files of a folder with no stamp were written by its first run and not changed since
/// (design section 3). `times`: every shipped file the folder holds, with when it was last
/// written (None: that could not be read, and the file is never judged unedited).
///
/// A first run writes every file within seconds, so the first run's moment is the earliest
/// stretch of `FIRST_RUN_WINDOW` holding at least half of the files (and at least
/// `FIRST_RUN_MIN_FILES`). The files written inside it were not changed since; one written
/// later (an edit) or earlier (a file copied in with its old time) is kept. A folder with no such
/// moment (most of its files edited, say) has nothing judged unedited, and keeps everything that
/// differs from this exe's copy: a file left old is shown in Settings > Data, a file overwritten
/// is lost, so every doubt keeps the file.
fn first_run_files(times: &[(&str, Option<SystemTime>)]) -> HashSet<String> {
    let mut known: Vec<(SystemTime, &str)> = times.iter().filter_map(|(rel, t)| t.map(|t| (t, *rel))).collect();
    known.sort();
    let need = times.len().div_ceil(2).max(FIRST_RUN_MIN_FILES);
    let mut end = 0;
    for start in 0..known.len() {
        let limit = known[start].0 + FIRST_RUN_WINDOW;
        end = end.max(start);
        while end < known.len() && known[end].0 <= limit {
            end += 1;
        }
        if end - start >= need {
            return known[start..end].iter().map(|(_, rel)| rel.to_string()).collect();
        }
    }
    HashSet::new()
}

/// One shipped file as the folder holds it.
enum OnDisk {
    Absent,
    Unreadable(String),
    Present { hash: String, modified: Option<SystemTime> },
}

/// Bring `dir` up to the copies in `shipped` (path under the folder, bytes), the data `version`
/// ships, keeping every file that was edited. See the module notes for the rule; `Outcome` says
/// what happened. Writes nothing in a folder `why_left_alone` names.
pub fn refresh(dir: &Path, version: &str, shipped: &[(&str, &[u8])]) -> Outcome {
    // The usual start: the stamp says the folder already holds this exe's copies. Nothing is
    // written on this path, so it needs no other check (and a start pays only for the stamp).
    let fingerprint = shipped_fingerprint(shipped);
    let old = read_stamp(dir);
    if let Some(stamp) = &old {
        if stamp.version.as_deref() == Some(version) && stamp.shipped == fingerprint {
            return Outcome::Current;
        }
    }
    let rels: Vec<&str> = shipped.iter().map(|(rel, _)| *rel).collect();
    if let Some(why) = why_left_alone(dir, &rels) {
        return Outcome::LeftAlone(why);
    }

    let on_disk: Vec<OnDisk> = rels
        .iter()
        .map(|rel| {
            let path = dir.join(rel);
            match std::fs::read(&path) {
                Ok(bytes) => OnDisk::Present {
                    hash: hash(&bytes),
                    modified: std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => OnDisk::Absent,
                Err(e) => OnDisk::Unreadable(e.to_string()),
            }
        })
        .collect();
    // No stamp: what the first run wrote is told by when it was written. The files that already
    // hold this exe's copy count too: they were written in the same moment.
    let first_run = old.is_none().then(|| {
        let times: Vec<(&str, Option<SystemTime>)> = rels
            .iter()
            .zip(&on_disk)
            .filter_map(|(rel, state)| match state {
                OnDisk::Present { modified, .. } => Some((*rel, *modified)),
                _ => None,
            })
            .collect();
        first_run_files(&times)
    });

    let mut files = old.as_ref().map(|s| s.files.clone()).unwrap_or_default();
    let mut report = Report {
        from: old.as_ref().and_then(|s| s.version.clone()),
        to: version.to_string(),
        at: now_secs(),
        ..Default::default()
    };
    for ((rel, bytes), state) in shipped.iter().zip(&on_disk) {
        let new_hash = hash(bytes);
        let absent = match state {
            OnDisk::Unreadable(why) => {
                report.failed.push(NotWritten { file: rel.to_string(), why: format!("it could not be read ({why})") });
                continue;
            }
            OnDisk::Present { hash: on, .. } if *on == new_hash => {
                files.insert(rel.to_string(), new_hash);
                continue;
            }
            OnDisk::Present { hash: on, .. } => {
                let unedited = match (&old, &first_run) {
                    (Some(stamp), _) => stamp.files.get(*rel) == Some(on),
                    (None, Some(first)) => first.contains(*rel),
                    (None, None) => false,
                };
                if !unedited {
                    report.kept.push(rel.to_string());
                    continue;
                }
                // What the game wrote here, as judged: recorded before the write, so a write
                // that fails still leaves the next start knowing this file was not edited.
                files.insert(rel.to_string(), on.clone());
                false
            }
            OnDisk::Absent => true,
        };
        match write_whole(&dir.join(rel), bytes) {
            Ok(()) => {
                files.insert(rel.to_string(), new_hash);
                let list = if absent { &mut report.added } else { &mut report.updated };
                list.push(rel.to_string());
            }
            Err(e) => report.failed.push(NotWritten { file: rel.to_string(), why: format!("it could not be written ({e})") }),
        }
    }

    // A pass that could not write everything keeps the old version, so the next start runs it
    // again; the files it did write are recorded either way, so they are known as the game's.
    let finished = report.finished();
    let stamp = Stamp {
        version: if finished { Some(version.to_string()) } else { old.as_ref().and_then(|s| s.version.clone()) },
        shipped: if finished { fingerprint } else { old.as_ref().map(|s| s.shipped.clone()).unwrap_or_default() },
        files,
        last_update: Some(report.clone()),
    };
    if let Err(e) = write_stamp(dir, &stamp) {
        report.failed.push(NotWritten { file: STAMP_FILE.into(), why: format!("it could not be written ({e})") });
    }
    Outcome::Refreshed(report)
}

/// Write every file of `shipped` into `dir`, which does not exist yet (the first run), and stamp
/// it. Returns how many were written and each that was not, and why.
pub fn write_fresh(dir: &Path, version: &str, shipped: &[(&str, &[u8])]) -> (usize, Vec<NotWritten>) {
    let rels: Vec<&str> = shipped.iter().map(|(rel, _)| *rel).collect();
    if let Some(why) = why_left_alone(dir, &rels) {
        log::info!("data folder {} not written: {why}", dir.display());
        return (0, Vec::new());
    }
    let mut files = BTreeMap::new();
    let mut failed = Vec::new();
    for (rel, bytes) in shipped {
        match write_whole(&dir.join(rel), bytes) {
            Ok(()) => {
                files.insert(rel.to_string(), hash(bytes));
            }
            Err(e) => failed.push(NotWritten { file: rel.to_string(), why: format!("it could not be written ({e})") }),
        }
    }
    let finished = failed.is_empty();
    let stamp = Stamp {
        version: finished.then(|| version.to_string()),
        shipped: if finished { shipped_fingerprint(shipped) } else { String::new() },
        files,
        last_update: None,
    };
    let written = stamp.files.len();
    if let Err(e) = write_stamp(dir, &stamp) {
        failed.push(NotWritten { file: STAMP_FILE.into(), why: format!("it could not be written ({e})") });
    }
    (written, failed)
}

/// First free `<name>.yours-<version>` beside `path` (then `-2`, `-3`, ...): where "Use the
/// game's copy" moves the player's file. Its last extension is no longer the data file's, so no
/// loader reads it (the quest loader takes `*.ron` only).
fn aside_path(path: &Path, version: &str) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let first = path.with_file_name(format!("{name}.yours-{version}"));
    if !first.exists() {
        return first;
    }
    (2u32..)
        .map(|n| path.with_file_name(format!("{name}.yours-{version}-{n}")))
        .find(|p| !p.exists())
        .expect("a free name")
}

/// "Use the game's copy" for the kept file `rel` of `dir`: the player's file is moved aside as
/// `<name>.yours-<version>` (never deleted), `bytes` (this version's copy) is written in its
/// place, and the stamp records it and no longer lists it as kept. Returns where the player's
/// file went (None: there was none to move). A folder `why_left_alone` names (judged on every
/// path the game ships, `shipped_rels`, as the refresh judges it) is refused.
pub fn use_games_copy_in(dir: &Path, rel: &str, version: &str, bytes: &[u8], shipped_rels: &[&str]) -> Result<Option<PathBuf>, String> {
    let rels: Vec<&str> = shipped_rels.iter().copied().chain([rel]).collect();
    if let Some(why) = why_left_alone(dir, &rels) {
        return Err(format!("the game does not change this folder: {why}"));
    }
    let path = dir.join(rel);
    let aside = if path.exists() {
        let aside = aside_path(&path, version);
        std::fs::rename(&path, &aside).map_err(|e| format!("your file could not be moved aside ({e}), so nothing was changed"))?;
        Some(aside)
    } else {
        None
    };
    if let Err(e) = write_whole(&path, bytes) {
        if let Some(aside) = &aside {
            let _ = std::fs::rename(aside, &path);
        }
        return Err(format!("the game's copy could not be written ({e}), so your file was put back"));
    }
    // Only a stamp that exists is changed: starting one here would leave a folder written before
    // stamps with a record of one file, and every other file would then count as edited.
    if let Some(mut stamp) = read_stamp(dir) {
        stamp.files.insert(rel.to_string(), hash(bytes));
        if let Some(last) = stamp.last_update.as_mut() {
            last.kept.retain(|k| k != rel);
        }
        if let Err(e) = write_stamp(dir, &stamp) {
            log::warn!("data/{rel} now holds the game's copy, but {} could not be written ({e})", dir.join(STAMP_FILE).display());
        }
    }
    Ok(aside)
}

// ── The running game: on start, and for Settings > Data ─────────────────────

/// What Settings > Data shows about the data folder (`view`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// The folder.
    pub dir: PathBuf,
    /// Why the game does not change it, when it does not.
    pub left_alone: Option<String>,
    /// The version whose copies it holds (the stamp's), None when that is not known.
    pub version: Option<String>,
    /// It has a stamp (false: written before stamps, or left alone).
    pub stamped: bool,
    /// What the last refresh did, from the stamp.
    pub last_update: Option<Report>,
}

/// The view of the folder, read once and kept until something here changes it.
static VIEW: RwLock<Option<View>> = RwLock::new(None);

/// What `dir` looks like to Settings > Data, read from the folder (no writes).
pub fn view_of(dir: &Path) -> View {
    let left_alone = why_left_alone(dir, crate::embedded_data::EMBEDDED_KEYS);
    let stamp = if left_alone.is_none() { read_stamp(dir) } else { None };
    View {
        dir: dir.to_path_buf(),
        left_alone,
        version: stamp.as_ref().and_then(|s| s.version.clone()),
        stamped: stamp.is_some(),
        last_update: stamp.and_then(|s| s.last_update),
    }
}

/// The data folder as Settings > Data shows it. None while there is none (the storage chooser
/// has not run, or nothing has been written).
pub fn view() -> Option<View> {
    if let Some(v) = VIEW.read().ok().and_then(|g| (*g).clone()) {
        return Some(v);
    }
    let dir = super::writable_data_dir()?;
    if !dir.exists() {
        return None;
    }
    let v = view_of(&dir);
    if let Ok(mut g) = VIEW.write() {
        *g = Some(v.clone());
    }
    Some(v)
}

fn forget_view() {
    if let Ok(mut g) = VIEW.write() {
        *g = None;
    }
}

/// On start, for a data folder that already exists: bring it up to this exe's copies
/// (`refresh`), log what happened in one line, and keep it for Settings > Data. Called by
/// `storage::extract_data_if_needed`, before anything reads the folder.
pub fn refresh_on_start(dir: &Path) -> Outcome {
    let shipped: Vec<(&str, &[u8])> = crate::embedded_data::EMBEDDED_KEYS
        .iter()
        .filter_map(|rel| crate::embedded_data::get_embedded(rel).map(|text| (*rel, text.as_bytes())))
        .collect();
    let outcome = refresh(dir, VERSION, &shipped);
    match &outcome {
        Outcome::Current => {}
        Outcome::LeftAlone(why) => log::info!("data folder {} left as it is: {why}", dir.display()),
        Outcome::Refreshed(r) => {
            log::info!("data folder {} {}", dir.display(), summary(r));
            if !r.failed.is_empty() {
                let list: Vec<String> = r.failed.iter().map(|f| format!("{}: {}", f.file, f.why)).collect();
                log::warn!("data folder {}: not brought up to date this start (tried again at the next): {}", dir.display(), list.join("; "));
            }
            let written = r.updated.len() + r.added.len();
            if written > 0 {
                // These files now hold the copies compiled into this exe, which is what the note
                // says (a rig refuses such a run; a rig's data folder is a link, never refreshed).
                crate::embedded_data::note_builtin_copy(
                    "*",
                    format_args!("{written} data files in {} were brought up to date from the copies compiled into this exe", dir.display()),
                );
            }
        }
    }
    forget_view();
    outcome
}

/// "refreshed from 0.1440.0 to 0.1463.0: 61 files updated, 2 kept because they were edited",
/// the design's one line.
pub fn summary(r: &Report) -> String {
    let from = r.from.as_deref().unwrap_or("a version from before stamps");
    let mut parts = vec![format!("{} updated", files_word(r.updated.len()))];
    if !r.added.is_empty() {
        parts.push(format!("{} added", r.added.len()));
    }
    parts.push(format!("{} kept because {} edited", r.kept.len(), if r.kept.len() == 1 { "it was" } else { "they were" }));
    if !r.failed.is_empty() {
        parts.push(format!("{} not written", r.failed.len()));
    }
    format!("refreshed from {from} to {}: {}", r.to, parts.join(", "))
}

fn files_word(n: usize) -> String {
    format!("{n} {}", if n == 1 { "file" } else { "files" })
}

/// Settings > Data's "Use the game's copy" for the kept file `rel` of this game's data folder.
/// Returns the sentence to show.
pub fn use_the_games_copy(rel: &str) -> Result<String, String> {
    let dir = super::writable_data_dir().ok_or("this game has no data folder yet")?;
    let text = crate::embedded_data::get_embedded(rel).ok_or_else(|| format!("the game has no copy of its own of {rel}"))?;
    let aside = use_games_copy_in(&dir, rel, VERSION, text.as_bytes(), crate::embedded_data::EMBEDDED_KEYS)?;
    crate::embedded_data::note_builtin_copy(
        "*",
        format_args!("data/{rel} in {} now holds the copy compiled into this exe, at the player's request", dir.display()),
    );
    forget_view();
    Ok(match aside {
        Some(aside) => format!(
            "{rel} now holds the game's copy. Yours is kept beside it as {}. The game reads the new copy from the next start.",
            aside.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        ),
        None => format!("{rel} now holds the game's copy. The game reads it from the next start."),
    })
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
