//! Getting an installed model ready for the engine (rung 2, 2026-11-09): which one, checked byte for
//! byte before it is loaded (docs/design/embeddable-ai.md, 6.2: "checked after download and before each
//! load"), and what the load asks for (its working memory, its threads, the job object's memory cap).
//!
//! The check reads the whole file (Granite: 1 GB, about two seconds), so it runs on a thread of its own
//! and the caller asks again; nothing waits in the frame. A file that passed is remembered for this run
//! only, by its size and its last change time (so an edit to it is noticed), and only while the Dev
//! page's "turn off all reuse" switch allows reuse; with it off, every load checks again (CLAUDE.md's
//! no-caches rule: in memory, tagged with what its source changes, behind the one switch).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use super::catalog::{AiCatalog, ModelEntry};
use super::install;

/// What the job object's cap allows above a model's need (9.1: "the cap is the model's need plus a
/// margin"), MB: room for a longer conversation than planned and the engine's own buffers. Measured on
/// Granite: 1,028 MB committed after its load, its need 1,640 MB (data/ai/choices.ron).
pub const CAP_MARGIN_MB: u64 = 512;

/// A model ready to load.
#[derive(Debug, Clone, PartialEq)]
pub struct Ready {
    pub entry: ModelEntry,
    pub path: PathBuf,
}

/// Where getting a model ready stands.
#[derive(Debug, Clone, PartialEq)]
pub enum Prepared {
    /// No model of this kind is installed.
    NotInstalled,
    /// Its file is being checked; ask again.
    Checking,
    Ready(Ready),
    /// Its file does not match the catalog (changed on disk since it was installed): not loaded.
    Mismatch(String),
}

#[derive(Debug, Clone)]
enum Check {
    Running(PathBuf),
    Passed { path: PathBuf, len: u64, modified: Option<SystemTime> },
    Failed { path: PathBuf, why: String },
}

static CHECK: Mutex<Option<Check>> = Mutex::new(None);

fn stamp(path: &Path) -> (u64, Option<SystemTime>) {
    std::fs::metadata(path).map(|m| (m.len(), m.modified().ok())).unwrap_or((0, None))
}

/// Settings > AI's brain choice when it is left to the app (10: "Settings picks by hardware and the
/// person can override").
pub const BRAIN_AUTO: &str = "auto";

/// Below this much memory in the computer, Automatic prefers the Quick brain even when the Rich one is
/// installed (9.1: an 8 GB PC runs Granite; 16 GB and up, Ministral).
pub const RICH_NEEDS_TOTAL_MB: u64 = 12 * 1024;

/// The installed brains, in the catalog's order.
pub fn installed_brains(cat: &AiCatalog, root: &Path) -> Vec<(ModelEntry, PathBuf)> {
    let have = install::installed(cat, root);
    cat.models.iter().filter(|m| m.kind == "brain" && have.models.contains_key(&m.id)).map(|m| (m.clone(), install::model_file(root, m))).collect()
}

/// Which installed brain to use: the one whose choice (`characters-rich`, `characters-quick`) the
/// person picked, when it is installed; otherwise, Automatic: the Rich one when installed and the
/// computer has [`RICH_NEEDS_TOTAL_MB`] (`total_mb`, None when unknown counts as enough), else the
/// first installed. None when no brain is installed.
pub fn pick_brain(brains: &[(ModelEntry, PathBuf)], prefer: &str, total_mb: Option<u64>) -> Option<(ModelEntry, PathBuf)> {
    if let Some(b) = brains.iter().find(|(m, _)| m.choice == prefer) {
        return Some(b.clone());
    }
    let rich_fits = total_mb.is_none_or(|t| t >= RICH_NEEDS_TOTAL_MB);
    brains.iter().find(|(m, _)| rich_fits && m.choice == "characters-rich").or_else(|| brains.iter().find(|(m, _)| m.choice != "characters-rich")).or_else(|| brains.first()).cloned()
}

/// The installed brain to use now (see [`pick_brain`]).
pub fn installed_brain(cat: &AiCatalog, root: &Path, prefer: &str) -> Option<(ModelEntry, PathBuf)> {
    pick_brain(&installed_brains(cat, root), prefer, super::hardware::read().memory_total_mb)
}

/// Get the installed brain ready: checked this run (and unchanged since), or a check started now.
/// `prefer` is Settings > AI's brain choice ([`BRAIN_AUTO`] or a choice id).
pub fn prepare_brain(cat: &AiCatalog, root: &Path, prefer: &str) -> Prepared {
    let Some((entry, path)) = installed_brain(cat, root, prefer) else { return Prepared::NotInstalled };
    let mut slot = CHECK.lock().unwrap_or_else(|p| p.into_inner());
    let (len, modified) = stamp(&path);
    match slot.as_ref() {
        Some(Check::Running(p)) if *p == path => return Prepared::Checking,
        Some(Check::Passed { path: p, len: l, modified: m }) if *p == path && *l == len && *m == modified => {
            if humanity_core::reuse::enabled() {
                return Prepared::Ready(Ready { entry, path });
            }
            // Reuse off: hand this one result over once, then check again next time.
            *slot = None;
            return Prepared::Ready(Ready { entry, path });
        }
        Some(Check::Failed { path: p, why }) if *p == path => {
            let why = why.clone();
            *slot = None;
            return Prepared::Mismatch(why);
        }
        _ => {}
    }
    *slot = Some(Check::Running(path.clone()));
    drop(slot);
    let (bytes, sha) = (entry.bytes, entry.sha256.clone());
    let spawned = std::thread::Builder::new().name("ai-model-check".into()).spawn(move || {
        let started = std::time::Instant::now();
        let result = install::verify(&path, bytes, &sha);
        let (len, modified) = stamp(&path);
        log::info!("[AI] checked {} in {} ms: {}", path.display(), started.elapsed().as_millis(), if result.is_ok() { "it matches the catalog" } else { "it does NOT match" });
        let mut slot = CHECK.lock().unwrap_or_else(|p| p.into_inner());
        *slot = Some(match result {
            Ok(()) => Check::Passed { path, len, modified },
            Err(why) => Check::Failed { path, why },
        });
    });
    if spawned.is_err() {
        *CHECK.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
    Prepared::Checking
}

/// The job object's memory cap for a model, MB: its choice's need (data/ai/choices.ron) plus the
/// margin; the need of a choice the file does not list counts as the model file's size twice.
pub fn memory_cap_mb(entry: &ModelEntry) -> u64 {
    let need = super::choices::current().ok().and_then(|c| c.choice(&entry.choice).map(|ch| ch.needs.memory_mb)).unwrap_or(entry.bytes / (1024 * 1024) * 2);
    need + CAP_MARGIN_MB
}

/// Forget the check (after Remove, so a reinstalled file is checked afresh).
pub fn forget_checks() {
    *CHECK.lock().unwrap_or_else(|p| p.into_inner()) = None;
}
