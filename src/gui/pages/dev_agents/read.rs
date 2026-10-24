//! The readers behind Dev > Claude (2026-10-22): each one turns one file, or one answer from
//! git, into what the panel shows. No egui here, and nothing is ever written: the panel only
//! reads. The panel's background jobs (`super`) call these off the frame, so a slow disk or a
//! slow git never stalls a frame.
//!
//! What each reads, and where the same thing is read by a script:
//! - the worktrees: `git worktree list --porcelain`, one `git log` for every head, one
//!   `git status --porcelain` per worktree (all with optional locks off, so the panel never
//!   takes a worktree's index lock while an agent commits there);
//! - the journal: `data/coordination/orchestrator_state.json`, as `scripts/brief.js` reads it;
//! - TIER 0: the `###` headings under `## TIER 0` in `docs/PRIORITIES.md`;
//! - the share guard: `%LOCALAPPDATA%\HumanityOS\share-guard.json` (`scripts/pc-share-guard.ps1`
//!   writes it once a minute, from PowerShell, so it starts with a byte-order mark);
//! - the disk guard: `%LOCALAPPDATA%\HumanityOS\disk-guard-last.json` (`scripts/pc-disk-guard.js`);
//! - the boot lock: `%LOCALAPPDATA%\HumanityOS\rig-boot.lock` (`scripts/lib/boot-lock.js`).
//!
//! A file that is not there is not an error: it means the thing it reports on has not run here,
//! and the panel says so in those words (`Reading::Absent`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One reading: what was found, or why nothing was.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading<T> {
    /// Not read yet: the first pass is still running.
    Waiting,
    /// Read.
    Found(T),
    /// The file (or program) is not there. Not an error: the panel words it as "not running"
    /// or "never ran here". Holds what is missing, a path or a program's name.
    Absent(String),
    /// It is there but could not be read or understood. Holds what went wrong.
    Unreadable(String),
}

impl<T> Default for Reading<T> {
    fn default() -> Self {
        Reading::Waiting
    }
}

impl<T> Reading<T> {
    /// Turn what was found into something else; a parse failure becomes `Unreadable`.
    pub fn then<U>(self, f: impl FnOnce(T) -> Result<U, String>) -> Reading<U> {
        match self {
            Reading::Found(t) => match f(t) {
                Ok(u) => Reading::Found(u),
                Err(e) => Reading::Unreadable(e),
            },
            Reading::Waiting => Reading::Waiting,
            Reading::Absent(w) => Reading::Absent(w),
            Reading::Unreadable(w) => Reading::Unreadable(w),
        }
    }

    pub fn found(&self) -> Option<&T> {
        match self {
            Reading::Found(t) => Some(t),
            _ => None,
        }
    }
}

/// A text file, with a leading byte-order mark dropped (PowerShell 5.1's `Set-Content -Encoding
/// utf8` writes one, and JSON parsers refuse it). Missing = `Absent`.
pub fn read_text(path: &Path) -> Reading<String> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            Reading::Found(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Reading::Absent(path.display().to_string()),
        Err(e) => Reading::Unreadable(format!("{}: {e}", path.display())),
    }
}

// ── Where the checkout is ────────────────────────────────────────────────────────────────────

/// A HumanityOS source checkout: the folder with Cargo.toml and the coordination files.
pub fn is_checkout(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file() && dir.join("data").join("coordination").is_dir()
}

/// The checkout the panel reports on, found by walking up from each start (the data folder, the
/// exe's folder, the working folder). A worktree resolves to the MAIN checkout it belongs to,
/// because that is where the sessions write the journal and where the taskbar exe sits.
pub fn find_checkout(starts: &[PathBuf]) -> Option<PathBuf> {
    for start in starts {
        for dir in start.ancestors() {
            if is_checkout(dir) {
                return Some(main_checkout_of(dir));
            }
        }
    }
    None
}

/// A worktree's `.git` is a FILE naming `<main>/.git/worktrees/<name>`; a main checkout's `.git`
/// is a folder, so it is its own main. Read without running git.
pub fn main_checkout_of(dir: &Path) -> PathBuf {
    if let Reading::Found(text) = read_text(&dir.join(".git")) {
        if let Some(main) = main_from_gitdir(&text) {
            if is_checkout(&main) {
                return main;
            }
        }
    }
    dir.to_path_buf()
}

/// `gitdir: C:/Humanity/.git/worktrees/agent-x` -> `C:/Humanity`.
pub fn main_from_gitdir(dotgit_file: &str) -> Option<PathBuf> {
    let gitdir = dotgit_file.lines().find_map(|l| l.trim().strip_prefix("gitdir:"))?.trim();
    let worktrees = Path::new(gitdir).parent()?;
    if worktrees.file_name()? != "worktrees" {
        return None;
    }
    let git = worktrees.parent()?;
    if git.file_name()? != ".git" {
        return None;
    }
    Some(git.parent()?.to_path_buf())
}

// ── Worktrees ────────────────────────────────────────────────────────────────────────────────

/// One git worktree: the main checkout, or an agent's folder under `.claude/worktrees/`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Worktree {
    pub path: String,
    /// The commit it is on (full hash); empty for a bare repository.
    pub head: String,
    /// Its branch without `refs/heads/`; None when detached.
    pub branch: Option<String>,
    /// The first entry git lists: the main checkout.
    pub main: bool,
    pub bare: bool,
    /// Some(reason) when git has it locked. The Claude harness locks an agent's worktree while
    /// the agent runs, with a reason ending "(pid N)".
    pub locked: Option<String>,
    /// Git says its folder is gone.
    pub prunable: bool,
    /// The last commit: when (Unix seconds) and its subject line.
    pub committed_at: Option<i64>,
    pub subject: Option<String>,
    /// Files changed and not committed, leaving out `IGNORED_CHANGES`; None when git could not say.
    pub uncommitted: Option<usize>,
    /// Whether the process named in the lock is still running; None when unknown.
    pub holder_running: Option<bool>,
}

impl Worktree {
    /// The folder's own name (`agent-a34fb7ec1b3ca6fc5`, or `Humanity` for the main checkout).
    pub fn name(&self) -> String {
        Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone())
    }

    /// The pid in the lock's reason, if it names one.
    pub fn lock_pid(&self) -> Option<u32> {
        lock_pid(self.locked.as_deref()?)
    }
}

/// `git worktree list --porcelain`: blocks separated by blank lines, one `key value` per line.
pub fn parse_worktree_list(text: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    let mut cur: Option<Worktree> = None;
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            out.extend(cur.take());
            continue;
        }
        let (key, rest) = match line.split_once(' ') {
            Some((k, r)) => (k, r),
            None => (line, ""),
        };
        if key == "worktree" {
            out.extend(cur.take());
            cur = Some(Worktree { path: rest.to_string(), ..Default::default() });
            continue;
        }
        let Some(w) = cur.as_mut() else { continue };
        match key {
            "HEAD" => w.head = rest.to_string(),
            "branch" => w.branch = Some(rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string()),
            "detached" => w.branch = None,
            "bare" => w.bare = true,
            "locked" => w.locked = Some(rest.to_string()),
            "prunable" => w.prunable = true,
            _ => {}
        }
    }
    out.extend(cur.take());
    if let Some(first) = out.first_mut() {
        first.main = true;
    }
    out
}

/// The pid in a lock reason like `claude agent agent-x (pid 16820)`.
pub fn lock_pid(reason: &str) -> Option<u32> {
    let at = reason.rfind("(pid ")?;
    let rest = &reason[at + "(pid ".len()..];
    rest[..rest.find(')')?].trim().parse().ok()
}

/// `git log --no-walk=unsorted --format=%H%x09%ct%x09%s <heads>`: hash -> (time, subject).
pub fn parse_log(text: &str) -> HashMap<String, (i64, String)> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let mut parts = line.trim_end_matches('\r').splitn(3, '\t');
        let (Some(hash), Some(time), Some(subject)) = (parts.next(), parts.next(), parts.next()) else { continue };
        if let Ok(t) = time.trim().parse::<i64>() {
            out.insert(hash.trim().to_string(), (t, subject.to_string()));
        }
    }
    out
}

/// Changes every worktree carries that are not anyone's work: the Claude harness writes its own
/// permissions file into each one (CLAUDE.md: a landed worktree's status shows only this file).
pub const IGNORED_CHANGES: &[&str] = &[".claude/settings.local.json"];

/// How many files `git status --porcelain` lists, leaving out `IGNORED_CHANGES`.
pub fn count_uncommitted(status: &str) -> usize {
    status
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| l.len() > 3)
        .filter(|l| !IGNORED_CHANGES.contains(&l[3..].trim_matches('"')))
        .count()
}

/// The main checkout first, then the rest with the newest commit first.
pub fn sort_worktrees(list: &mut [Worktree]) {
    list.sort_by(|a, b| b.main.cmp(&a.main).then(b.committed_at.unwrap_or(0).cmp(&a.committed_at.unwrap_or(0))));
}

// ── Handoff notes ────────────────────────────────────────────────────────────────────────────

/// One agent's handoff note: `.claude/handoff/<topic>.md`, where an agent writes what it has
/// done, what is verified and what is next, so its work survives a restart (orchestrator,
/// 2026-10-22). Agents isolated in a worktree write theirs inside the worktree.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// The note's own heading (its first line, without the `#`s).
    pub title: String,
    /// The folder it is in: the main checkout's or a worktree's name.
    pub place: String,
    pub path: String,
    /// When it was last written, Unix seconds.
    pub modified: Option<i64>,
}

/// A note's title: its first non-empty line, without heading marks.
pub fn note_title(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    first.trim_start_matches('#').trim().to_string()
}

/// Every `*.md` in `<dir>/.claude/handoff/`. A folder that is not there holds no notes.
pub fn read_notes(dir: &Path, place: &str) -> Vec<Note> {
    let Ok(entries) = std::fs::read_dir(dir.join(".claude").join("handoff")) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Reading::Found(text) = read_text(&path) else { continue };
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        let title = note_title(&text);
        out.push(Note {
            title: if title.is_empty() { entry.file_name().to_string_lossy().into_owned() } else { title },
            place: place.to_string(),
            path: path.display().to_string(),
            modified,
        });
    }
    out
}

/// The notes of the checkout and of every worktree, newest first.
pub fn all_notes(worktrees: &[Worktree]) -> Vec<Note> {
    let mut out: Vec<Note> = worktrees.iter().flat_map(|w| read_notes(Path::new(&w.path), &w.name())).collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

// ── The journal and the priorities ───────────────────────────────────────────────────────────

/// One journal decision, as one line.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub date: String,
    pub text: String,
}

/// The newest `n` entries of `recent_decisions`, newest first, each folded onto one line. The
/// journal keeps the newest at the bottom; entries carry `date` (or `at` in older ones).
pub fn journal_tail(json: &str, n: usize) -> Result<Vec<Decision>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let list = v
        .get("recent_decisions")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "it has no recent_decisions list".to_string())?;
    Ok(list
        .iter()
        .rev()
        .take(n)
        .map(|d| Decision {
            date: d.get("date").or_else(|| d.get("at")).and_then(|x| x.as_str()).unwrap_or("?").to_string(),
            text: one_line(d.get("decision").and_then(|x| x.as_str()).unwrap_or("")),
        })
        .collect())
}

/// Every run of whitespace, line breaks included, as one space.
pub fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `###` headings under `## TIER 0` in PRIORITIES.md, in order, at most `max`.
pub fn tier0_headings(md: &str, max: usize) -> Vec<String> {
    let mut inside = false;
    let mut out = Vec::new();
    for raw in md.lines() {
        let line = raw.trim_end();
        if let Some(h2) = line.strip_prefix("## ") {
            if inside {
                break;
            }
            inside = h2.trim_start().starts_with("TIER 0");
            continue;
        }
        if inside {
            if let Some(h3) = line.strip_prefix("### ") {
                out.push(h3.trim().to_string());
                if out.len() >= max {
                    break;
                }
            }
        }
    }
    out
}

// ── Versions ─────────────────────────────────────────────────────────────────────────────────

/// The package version in Cargo.toml (the first `version = "..."` line, as the scripts read it).
pub fn cargo_version(toml: &str) -> Option<String> {
    toml.lines().find_map(|l| {
        let rest = l.strip_prefix("version")?.trim_start().strip_prefix('=')?.trim_start().strip_prefix('"')?;
        Some(rest[..rest.find('"')?].to_string())
    })
}

/// `v0.1543.1` or `0.1543.1` as three numbers.
pub fn version_parts(v: &str) -> [u64; 3] {
    let mut out = [0u64; 3];
    for (i, part) in v.trim().trim_start_matches('v').split('.').take(3).enumerate() {
        out[i] = part.trim().parse().unwrap_or(0);
    }
    out
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    version_parts(a).cmp(&version_parts(b))
}

/// The newest GitHub release (the one marked Latest).
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub tag: String,
    pub published_at: Option<i64>,
}

/// GitHub's `/releases/latest` answer.
pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let tag = v.get("tag_name").and_then(|t| t.as_str()).ok_or_else(|| {
        v.get("message").and_then(|m| m.as_str()).map(|m| format!("GitHub said: {m}")).unwrap_or_else(|| "no tag_name".into())
    })?;
    Ok(Release {
        tag: tag.to_string(),
        published_at: v.get("published_at").and_then(|p| p.as_str()).and_then(parse_rfc3339),
    })
}

// ── Times ────────────────────────────────────────────────────────────────────────────────────

/// Seconds since 1970 now.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-10-23T06:05:05.551Z` (JavaScript) or `2026-10-22T23:16:13.5876404-07:00` (PowerShell's
/// round-trip format) as Unix seconds. None for anything else.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, sec) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    let mut rest = &s[19..];
    if let Some(frac) = rest.strip_prefix('.') {
        rest = frac.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = if rest.is_empty() || rest == "Z" || rest == "z" {
        0
    } else {
        let sign = match rest.as_bytes()[0] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let (oh, om) = rest[1..].split_once(':')?;
        sign * (oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60)
    };
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec - offset)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ── This computer: the share guard, the disk guard, the boot lock ───────────────────────────

/// `%LOCALAPPDATA%\HumanityOS`, where the guards and the boot lock keep their files.
pub fn local_state_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("HumanityOS"))
}

/// What the share guard last wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct ShareGuard {
    /// Its last pass, Unix seconds.
    pub at: i64,
    /// Development processes it found (and holds at below-normal priority, off the kept core).
    pub dev_processes_now: u64,
    /// Logical processors kept for the operator, of `logical`.
    pub kept: u64,
    pub logical: u64,
}

/// The share guard writes once a minute while it runs, so a status older than this means it
/// has stopped.
pub const SHARE_GUARD_FRESH_SECS: i64 = 180;

pub fn parse_share_guard(text: &str) -> Result<ShareGuard, String> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("not JSON: {e}"))?;
    let at = v.get("at").and_then(|a| a.as_str()).and_then(parse_rfc3339).ok_or("no readable time (at)")?;
    let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    Ok(ShareGuard { at, dev_processes_now: n("dev_processes_now"), kept: n("kept_for_the_operator"), logical: n("logical_processors") })
}

/// What the disk guard last wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskGuard {
    pub at: i64,
    /// Why it did nothing ("busy: cargo.exe running"), when it skipped.
    pub skipped: Option<String>,
    /// How many folders it cleared.
    pub cleared: usize,
    /// C: after its pass.
    pub used_pct: f64,
    pub free_gb: f64,
}

/// The disk guard runs hourly; older than this, its task is probably not installed.
pub const DISK_GUARD_LATE_SECS: i64 = 3 * 3600;

pub fn parse_disk_guard(text: &str) -> Result<DiskGuard, String> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("not JSON: {e}"))?;
    let at = v.get("at").and_then(|a| a.as_str()).and_then(parse_rfc3339).ok_or("no readable time (at)")?;
    Ok(DiskGuard {
        at,
        skipped: v.get("skipped").and_then(|s| s.as_str()).filter(|s| !s.is_empty()).map(str::to_string),
        cleared: v.get("actions").and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0),
        used_pct: v.get("after_pct").and_then(|x| x.as_f64()).unwrap_or(0.0),
        free_gb: v.get("free_gb").and_then(|x| x.as_f64()).unwrap_or(0.0),
    })
}

/// Who the boot lock names.
#[derive(Debug, Clone, PartialEq)]
pub struct BootLock {
    pub pid: Option<u32>,
    pub rig: String,
    pub since: Option<i64>,
}

/// The boot lock: no file = free.
#[derive(Debug, Clone, PartialEq)]
pub enum LockState {
    Free,
    /// Its holder is running (or the panel cannot tell, which the rigs also count as held).
    Held(BootLock),
    /// Its holder has exited: the next rig takes it over.
    Stale(BootLock),
    /// Empty or not JSON: a rig between creating it and writing it, or junk.
    Unreadable,
}

/// One compile in the machine-wide build slot (`scripts/build-slot/hos-build-slot.rs`, docs/ai/
/// builds.md): compiles of our crate take turns for memory, each one writing `held-<pid>.txt`
/// while it compiles and `wait-<pid>.txt` while it waits, as `key=value` lines.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlotEntry {
    pub pid: u32,
    /// What is compiled ("lib-test-native", "buildscript-build-relay", ...).
    pub kind: String,
    pub crate_name: String,
    /// The memory it is expected to need at its peak, MB.
    pub mb: u64,
    /// Since when it has held its room or waited, Unix seconds.
    pub since: Option<i64>,
    /// The checkout or worktree it compiles in.
    pub cwd: String,
    pub waiting: bool,
}

impl SlotEntry {
    /// The folder it compiles in, by name.
    pub fn place(&self) -> String {
        Path::new(&self.cwd).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.cwd.clone())
    }
}

/// One `held-`/`wait-` record, read the way the slot writes it; None without a pid.
pub fn parse_slot_record(text: &str, waiting: bool) -> Option<SlotEntry> {
    let mut e = SlotEntry { waiting, ..Default::default() };
    for line in text.lines() {
        let Some((k, v)) = line.trim_end_matches('\r').split_once('=') else { continue };
        match k {
            "pid" => e.pid = v.trim().parse().ok()?,
            "kind" => e.kind = v.to_string(),
            "crate" => e.crate_name = v.to_string(),
            "mb" => e.mb = v.trim().parse().unwrap_or(0),
            "since_ms" => e.since = v.trim().parse::<i64>().ok().map(|ms| ms / 1000),
            "cwd" => e.cwd = v.to_string(),
            _ => {}
        }
    }
    (e.pid != 0).then_some(e)
}

/// `%LOCALAPPDATA%\HumanityOS\build-slot`.
pub fn build_slot_dir() -> Option<PathBuf> {
    local_state_dir().map(|d| d.join("build-slot"))
}

/// Who compiles and who waits: the compiling first, then the waiting, each oldest first. A record
/// whose process is gone is a dead compile's leftover: the slot removes those itself, and the
/// panel (which never deletes anything) just leaves them out. No folder = the slot is not set up.
pub fn read_build_slot(dir: &Path, running: Option<&HashSet<u32>>) -> Reading<Vec<SlotEntry>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Reading::Absent(dir.display().to_string()),
        Err(e) => return Reading::Unreadable(format!("{}: {e}", dir.display())),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".txt") || !(name.starts_with("held-") || name.starts_with("wait-")) {
            continue;
        }
        let waiting = name.starts_with("wait-");
        let Reading::Found(text) = read_text(&entry.path()) else { continue };
        let Some(e) = parse_slot_record(&text, waiting) else { continue };
        if running.is_some_and(|set| !set.contains(&e.pid)) {
            continue;
        }
        out.push(e);
    }
    out.sort_by(|a, b| a.waiting.cmp(&b.waiting).then(a.since.cmp(&b.since)));
    Reading::Found(out)
}

/// The lock file: `HUMANITY_RIG_LOCK` when set (the tests set it), else the machine-wide one.
pub fn boot_lock_path() -> Option<PathBuf> {
    match std::env::var_os("HUMANITY_RIG_LOCK") {
        Some(p) => Some(PathBuf::from(p)),
        None => local_state_dir().map(|d| d.join("rig-boot.lock")),
    }
}

pub fn parse_boot_lock(text: &str) -> Result<BootLock, String> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("not JSON: {e}"))?;
    let pid = v.get("pid").and_then(|p| p.as_u64()).map(|p| p as u32);
    if pid.is_none() {
        return Err("it names no pid".into());
    }
    Ok(BootLock {
        pid,
        rig: v.get("rig").and_then(|r| r.as_str()).unwrap_or("a rig").to_string(),
        since: v.get("since").and_then(|s| s.as_str()).and_then(parse_rfc3339),
    })
}

/// The lock file's state: missing is `Free`; a holder that is not in `running` is `Stale`.
pub fn lock_state(file: &Reading<String>, running: Option<&HashSet<u32>>) -> Reading<LockState> {
    match file {
        Reading::Waiting => Reading::Waiting,
        Reading::Absent(_) => Reading::Found(LockState::Free),
        Reading::Unreadable(e) => Reading::Unreadable(e.clone()),
        Reading::Found(text) => Reading::Found(match parse_boot_lock(text) {
            Err(_) => LockState::Unreadable,
            Ok(lock) => match (lock.pid, running) {
                (Some(pid), Some(set)) if !set.contains(&pid) => LockState::Stale(lock),
                _ => LockState::Held(lock),
            },
        }),
    }
}

/// Every running process's id, or None when this platform cannot say. One Toolhelp snapshot on
/// Windows (the same call `engine::launch_focus` makes, raw so no dependency is added).
#[cfg(target_os = "windows")]
pub fn running_pids() -> Option<HashSet<u32>> {
    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const INVALID_HANDLE_VALUE: isize = -1;

    #[repr(C)]
    struct ProcessEntry32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> isize;
        fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }

    // SAFETY: the entry is zeroed and sized as the API requires; the snapshot handle is closed
    // on every path after it was opened.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
        let mut pids = HashSet::new();
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            pids.insert(entry.th32_process_id);
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        Some(pids)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn running_pids() -> Option<HashSet<u32>> {
    let dir = std::fs::read_dir("/proc").ok()?;
    Some(dir.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect())
}

// ── git ──────────────────────────────────────────────────────────────────────────────────────

/// Run git in `dir` and return what it printed. Optional locks are off (GIT_OPTIONAL_LOCKS=0):
/// a plain `git status` may refresh and rewrite the index, taking `index.lock`, which would make
/// an agent's `git commit` in that worktree fail at that moment. No console window opens.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "git is not installed or not on PATH".to_string()
        } else {
            format!("git: {e}")
        }
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().next().unwrap_or("git failed").to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every worktree with its last commit, its uncommitted count and whether its lock's holder
/// still runs. One `git worktree list`, one `git log` for all heads, one `git status` each.
pub fn read_worktrees(checkout: &Path, running: Option<&HashSet<u32>>) -> Reading<Vec<Worktree>> {
    let listing = match git(checkout, &["worktree", "list", "--porcelain"]) {
        Ok(t) => t,
        Err(e) if e.starts_with("git is not installed") => return Reading::Absent("git".into()),
        Err(e) => return Reading::Unreadable(e),
    };
    let mut list = parse_worktree_list(&listing);
    let mut heads: Vec<&str> = list.iter().map(|w| w.head.as_str()).filter(|h| !h.is_empty()).collect();
    heads.sort_unstable();
    heads.dedup();
    let commits = if heads.is_empty() {
        HashMap::new()
    } else {
        let mut args = vec!["log", "--no-walk=unsorted", "--format=%H%x09%ct%x09%s"];
        args.extend(heads.iter().copied());
        git(checkout, &args).map(|t| parse_log(&t)).unwrap_or_default()
    };
    for w in &mut list {
        if let Some((t, s)) = commits.get(&w.head) {
            w.committed_at = Some(*t);
            w.subject = Some(s.clone());
        }
        if !w.bare && !w.prunable && Path::new(&w.path).is_dir() {
            w.uncommitted = git(Path::new(&w.path), &["status", "--porcelain"]).ok().map(|s| count_uncommitted(&s));
        }
        w.holder_running = match (w.lock_pid(), running) {
            (Some(pid), Some(set)) => Some(set.contains(&pid)),
            _ => None,
        };
    }
    sort_worktrees(&mut list);
    Reading::Found(list)
}

/// GitHub's newest release. One request; the panel asks every ten minutes while it is open.
pub fn fetch_latest_release() -> Reading<Release> {
    const URL: &str = "https://api.github.com/repos/Shaostoul/Humanity/releases/latest";
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(12)).build();
    let body = agent
        .get(URL)
        .set("User-Agent", "HumanityOS-DevPanel")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub did not answer: {e}"))
        .and_then(|r| r.into_string().map_err(|e| format!("GitHub's answer could not be read: {e}")));
    match body {
        Ok(b) => Reading::Found(b).then(|b| parse_release(&b)),
        Err(e) => Reading::Unreadable(e),
    }
}
