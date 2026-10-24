//! Runs: one time a person does a how-to (docs/design/how-to.md, "A run"; fields in
//! schemas/how_to.toml [run_fields]).
//!
//! A run belongs to the person, lives on their device, and is never sent anywhere. On the desktop
//! each run is one file, `<app data>/how_to/runs/<run>.json` (`%APPDATA%\HumanityOS\how_to\runs`
//! on Windows, beside `saves/`), written to a `.tmp` and renamed after every tick
//! (persistence::write_atomic), so closing the app or a power cut loses at most the tick being
//! written. The website keeps the same shape in IndexedDB (web/pages/how-to-app.js).
//!
//! A step's state is keyed by the step's ID, never its position: a how-to that is edited,
//! reordered or updated while a run is open keeps every tick, and a step that no longer exists is
//! listed as "This step was removed since you started" (view.rs `removed_steps`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{HowTo, Kind};
use super::time;

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Worked,
    DidNotWork,
    Stopped,
}

/// One step's state in a run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StepState {
    /// When it was ticked (UTC, YYYY-MM-DDTHH:MM:SSZ). A check is done once answered.
    pub done: Option<String>,
    /// A measure step's value.
    pub value: Option<String>,
    /// A check's answer, "yes" or "no".
    pub answer: Option<String>,
    /// Where a No went: a problem id or a step id.
    pub went_to: Option<String>,
}

/// A running timer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaitState {
    /// When it is due (UTC).
    pub ends: String,
}

/// One run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub run: String,
    pub how_to: String,
    /// The how-to's hash when the run started.
    pub hash: String,
    pub reality: String,
    pub started: String,
    #[serde(default)]
    pub finished: Option<String>,
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
    #[serde(default)]
    pub steps: BTreeMap<String, StepState>,
    #[serde(default)]
    pub waits: BTreeMap<String, WaitState>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub photos: Vec<String>,
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub place: Option<String>,
}

impl Run {
    /// A new run of `how_to` (whose hash is `hash`) started at `now_ms`.
    pub fn start(how_to: &HowTo, hash: &str, now_ms: u64) -> Run {
        let stamp = time::compact(now_ms);
        let tail: u16 = rand::random();
        Run {
            run: format!("r-{stamp}-{tail:04x}"),
            how_to: how_to.id.clone(),
            hash: hash.to_string(),
            reality: "real".to_string(),
            started: time::iso(now_ms),
            finished: None,
            outcome: None,
            choices: BTreeMap::new(),
            steps: BTreeMap::new(),
            waits: BTreeMap::new(),
            notes: String::new(),
            photos: Vec::new(),
            task: None,
            place: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.finished.is_none()
    }

    pub fn is_done(&self, step: &str) -> bool {
        self.steps.get(step).map(|s| s.done.is_some()).unwrap_or(false)
    }

    /// Tick a step (or untick it).
    pub fn set_done(&mut self, step: &str, done: bool, now_ms: u64) {
        let s = self.steps.entry(step.to_string()).or_default();
        s.done = if done { Some(time::iso(now_ms)) } else { None };
    }

    /// Answer a check. Yes ticks it. No ticks it too (the question is answered) and records where
    /// No went; when that is a step, the run goes back there: the target and every step from it up
    /// to this check are unticked, so they are done again, the check included.
    pub fn answer(&mut self, how_to: &HowTo, step: &str, yes: bool, now_ms: u64) {
        let no_target = how_to.steps.iter().find(|s| s.id == step).and_then(|s| s.no.clone());
        let back_to = no_target.as_deref().filter(|t| how_to.steps.iter().any(|s| s.id == *t));
        if !yes {
            if let Some(target) = back_to {
                let from = how_to.steps.iter().position(|s| s.id == target);
                let to = how_to.steps.iter().position(|s| s.id == step);
                if let (Some(from), Some(to)) = (from, to) {
                    let (lo, hi) = (from.min(to), from.max(to));
                    for s in &how_to.steps[lo..=hi] {
                        if let Some(st) = self.steps.get_mut(&s.id) {
                            st.done = None;
                            if s.kind == Kind::Check {
                                st.answer = None;
                            }
                        }
                    }
                }
                let st = self.steps.entry(step.to_string()).or_default();
                st.went_to = Some(target.to_string());
                return;
            }
        }
        let st = self.steps.entry(step.to_string()).or_default();
        st.answer = Some(if yes { "yes" } else { "no" }.to_string());
        st.went_to = if yes { None } else { no_target };
        st.done = Some(time::iso(now_ms));
    }

    /// Pick an option of a choose step; it is done once something is picked.
    pub fn choose(&mut self, step: &str, option: &str, now_ms: u64) {
        self.choices.insert(step.to_string(), option.to_string());
        self.set_done(step, true, now_ms);
    }

    /// Write down a measure step's value; it is done once a value is written.
    pub fn record(&mut self, step: &str, value: &str, now_ms: u64) {
        let s = self.steps.entry(step.to_string()).or_default();
        let v = value.trim();
        s.value = if v.is_empty() { None } else { Some(v.to_string()) };
        s.done = if v.is_empty() { None } else { Some(time::iso(now_ms)) };
    }

    /// Start a wait step's timer: it is due `seconds` from `now_ms`. The due time is stored, so the
    /// timer runs while the app is closed.
    pub fn start_wait(&mut self, step: &str, seconds: u64, now_ms: u64) {
        self.waits.insert(step.to_string(), WaitState { ends: time::iso(now_ms + seconds * 1000) });
    }

    /// Milliseconds left on a step's timer (0 when due), or None when it was never started.
    pub fn wait_left_ms(&self, step: &str, now_ms: u64) -> Option<u64> {
        let ends = time::ms_from_iso(&self.waits.get(step)?.ends)?;
        Some(ends.saturating_sub(now_ms))
    }

    /// End the run.
    pub fn finish(&mut self, outcome: Outcome, now_ms: u64) {
        self.outcome = Some(outcome);
        self.finished = Some(time::iso(now_ms));
        self.waits.clear();
    }
}

/// Where a person's runs are kept on this computer.
pub fn default_dir() -> PathBuf {
    match crate::persistence::saves_dir().parent() {
        Some(root) if !root.as_os_str().is_empty() => root.join("how_to").join("runs"),
        _ => PathBuf::from("how_to").join("runs"),
    }
}

/// The runs in a folder.
#[derive(Debug, Clone)]
pub struct RunStore {
    pub dir: PathBuf,
}

impl RunStore {
    pub fn new(dir: impl Into<PathBuf>) -> RunStore {
        RunStore { dir: dir.into() }
    }

    /// Every run that can be read, newest first. A file that cannot be read is skipped and logged,
    /// never deleted: it is the person's own record.
    pub fn list(&self) -> Vec<Run> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return out };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Run>(&b).map_err(|e| e.to_string())) {
                Ok(r) => out.push(r),
                Err(e) => log::warn!("How-to run {} could not be read: {e}", p.display()),
            }
        }
        out.sort_by(|a, b| b.started.cmp(&a.started).then(b.run.cmp(&a.run)));
        out
    }

    /// Write a run, crash-safely.
    pub fn save(&self, run: &Run) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("could not make {}: {e}", self.dir.display()))?;
        let bytes = serde_json::to_vec_pretty(run).map_err(|e| e.to_string())?;
        crate::persistence::write_atomic(&self.path_of(&run.run), &bytes)
    }

    fn path_of(&self, run: &str) -> PathBuf {
        // A run id is ours (r-<digits>-<hex>); anything else is refused a path outside the folder.
        let safe: String = run.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        self.dir.join(format!("{safe}.json"))
    }

    /// The folder, for Settings > Data and the page's "where your runs are kept" line.
    pub fn folder(&self) -> &Path {
        &self.dir
    }
}
