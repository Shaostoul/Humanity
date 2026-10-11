//! The page's handle on one upload running on its worker thread (resumable.rs `work`): where it
//! is, as the uploads list shows it (its bar and its words), read from what the worker reports.

use std::sync::mpsc::Receiver;
use std::sync::Arc;

use serde_json::Value;

use super::resumable::{work, Control, Event, Plan, Transport};
use super::upload_waits::now_ms;

/// Where a running upload is, as the page shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Hashing,
    Sending,
    Paused,
    Waiting { words: String, resets_at: i64 },
    NoRoom { words: String },
    Reconnecting { words: String },
    /// The same server error ten times running: its words, and Retry.
    Stuck { words: String },
    /// Stored; the page publishes it.
    Done { filename: String },
    Failed { words: String },
    Cancelled,
}

/// An upload running on its worker thread.
pub struct Run {
    pub ctl: Arc<Control>,
    rx: Receiver<Event>,
    pub name: String,
    pub size: u64,
    /// The server's id, once it started.
    pub id: Option<String>,
    /// How much of the file the server has, and how much was hashed.
    pub sent: u64,
    pub hashed: u64,
    pub phase: Phase,
}

impl Run {
    /// Start `plan` on a worker thread through `t`.
    pub fn start(t: Arc<dyn Transport>, plan: Plan, ctl: Arc<Control>) -> Run {
        let (tx, rx) = std::sync::mpsc::channel();
        let (name, size, id) = (plan.name.clone(), plan.size, plan.resume.clone());
        let phase = if id.is_some() { Phase::Sending } else { Phase::Hashing };
        let worker_ctl = ctl.clone();
        std::thread::Builder::new()
            .name("media-upload".into())
            .spawn(move || work(t.as_ref(), &worker_ctl, &plan, &tx))
            .ok();
        Run { ctl, rx, name, size, id, sent: 0, hashed: 0, phase }
    }

    /// Read what the worker reported since the last frame, keeping `phase` and the counts, and
    /// return the events the page acts on (`Started`, `Done`, `Failed`, `Cancelled`).
    pub fn poll(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = self.rx.try_recv() {
            match &e {
                Event::Hashing { done } => {
                    self.hashed = *done;
                    if !matches!(self.phase, Phase::Paused) {
                        self.phase = Phase::Hashing;
                    }
                }
                Event::Started { id } => {
                    self.id = Some(id.clone());
                    self.phase = Phase::Sending;
                    out.push(e.clone());
                }
                Event::Progress { offset } => {
                    self.sent = *offset;
                    if !matches!(self.phase, Phase::Paused) {
                        self.phase = Phase::Sending;
                    }
                }
                Event::Paused => self.phase = Phase::Paused,
                Event::Sending => self.phase = if self.id.is_none() && self.hashed < self.size { Phase::Hashing } else { Phase::Sending },
                Event::Waiting { words, resets_at } => self.phase = Phase::Waiting { words: words.clone(), resets_at: *resets_at },
                Event::NoRoom { words } => self.phase = Phase::NoRoom { words: words.clone() },
                Event::Reconnecting { words } => self.phase = Phase::Reconnecting { words: words.clone() },
                Event::Stuck { words } => self.phase = Phase::Stuck { words: words.clone() },
                Event::Done { answer } => {
                    let filename = stored_name_of(answer);
                    self.sent = self.size;
                    self.phase = Phase::Done { filename };
                    out.push(e.clone());
                }
                Event::Failed { words, .. } => {
                    self.phase = Phase::Failed { words: words.clone() };
                    out.push(e.clone());
                }
                Event::Cancelled { .. } => {
                    self.phase = Phase::Cancelled;
                    out.push(e.clone());
                }
                // Cleaned into a copy (2026-10-11): the copy's size is what goes up.
                Event::Cleaned { size, .. } => {
                    self.size = *size;
                    out.push(e.clone());
                }
            }
        }
        out
    }

    /// How far along: the hash first, then the pieces.
    pub fn fraction(&self) -> f32 {
        let size = self.size.max(1) as f32;
        match self.phase {
            Phase::Hashing => self.hashed as f32 / size,
            _ => self.sent as f32 / size,
        }
    }

    /// The line beside the progress bar.
    pub fn words(&self) -> String {
        use super::api::size_words;
        let of = |n: u64| format!("{} of {}", size_words(n as i64), size_words(self.size as i64));
        match &self.phase {
            Phase::Hashing => format!("Checking the file before it goes: {}", of(self.hashed)),
            Phase::Sending => format!("Uploading: {}", of(self.sent)),
            Phase::Paused => format!("Paused at {}", of(self.sent)),
            Phase::Waiting { words, resets_at } => format!("{words} It carries on by itself {}, or press Resume.", when_words(*resets_at)),
            Phase::NoRoom { words } => words.clone(),
            Phase::Reconnecting { words } => words.clone(),
            Phase::Stuck { words } => format!("The server keeps refusing it: {words} Press Retry to try again."),
            Phase::Done { .. } => "Uploaded. Putting it on the Media page...".to_string(),
            Phase::Failed { words } => words.clone(),
            Phase::Cancelled => "Cancelled.".to_string(),
        }
    }
}

/// "in 2 hours 5 minutes" style words for when an allowance resets, or "now" once it has.
fn when_words(resets_at: i64) -> String {
    let left = resets_at - now_ms();
    if left <= 0 {
        return "now".to_string();
    }
    let minutes = (left + 59_999) / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("in {m} minute{}", if m == 1 { "" } else { "s" }),
        (h, 0) => format!("in {h} hour{}", if h == 1 { "" } else { "s" }),
        (h, m) => format!("in {h} hour{} {m} minute{}", if h == 1 { "" } else { "s" }, if m == 1 { "" } else { "s" }),
    }
}

/// The stored file name in a finished upload's answer (`filename`, else its address's last part).
pub fn stored_name_of(answer: &Value) -> String {
    let named = answer.get("filename").and_then(Value::as_str).unwrap_or("");
    if !named.is_empty() {
        return named.to_string();
    }
    answer.get("url").and_then(Value::as_str).map(|u| super::api::bare_name(u).to_string()).unwrap_or_default()
}
