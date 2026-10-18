//! Add to Media's upload, sent in pieces (the relay half is src/relay/resumable.rs; the design is
//! docs/design/media-showcase.md, "Resumable uploads, exact", its As built notes and round 3).
//! The operator's question was "What if I want to upload a multi-GB file?": one request carries
//! at most 1 GB, and a connection that drops at 900 MB loses all of it. Here a file goes a piece
//! at a time; the server writes and counts each before the next is sent, so after a drop the app
//! asks how far the server got and carries on from there.
//!
//! THE STEPS, all on a worker thread ([`work`]):
//! 1. The file's BLAKE3 hash, read a megabyte at a time (the server checks the finished file
//!    against it, so a resume that went wrong is caught, not stored).
//! 2. The start: `POST /api/upload/resumable` with `{filename, size, content_type, media: 1,
//!    blake3}`, answered `{id, offset: 0, size, chunk_max}`. The page keeps the id (and the file's
//!    path, size and modification time) in the app's config, so an upload cut off by a restart can
//!    be carried on ([`Unfinished`]).
//! 3. The pieces: `PATCH .../{id}?offset=<n>`, each at most the server's `chunk_max` (held to
//!    64 MiB, upload_waits.rs `piece_size`). A 409 with an `offset` says where the server is; a 409
//!    `busy` means another piece is being written, so it waits and asks. A dropped connection or a
//!    server error waits a little longer each time, asks where the server got (`GET .../{id}`), and
//!    carries on; ten of the same server error in a row stop it with the server's words and Retry.
//!    A 429 with the allowance's JSON is the role's day or hour allowance: its own words and when
//!    it resets are shown, a piece is cut to `left_bytes` when that still fits, and the upload
//!    carries on by itself after the server's own `resets_in_s` (or at Resume). A 429 without it
//!    is waited out like a server error. A 507 is no room: the file is kept on the server for a
//!    day, and Retry sends the empty piece at the end, which tries the finish again. A finish whose
//!    answer was lost is found in the status answer's `done`. The empty piece never reads the
//!    file on this computer (round 4, D4-1): once every piece is on the server, the server's copy
//!    is finished whatever happened to the local file since.
//! 4. The finished upload's answer (`{url, filename, size, type}`) goes back to the page, which
//!    publishes it then (signed then: a signature is good for five minutes, and an upload can take
//!    hours).
//!
//! The token: each connection to the server has its own upload token, and a reconnect replaces it
//! (the relay's C). The page hands the worker the current one every frame ([`Control::set_token`]);
//! a session belongs to the person, so any of their tokens carries it on. It rides in the
//! `X-Upload-Token` header (R3-9), never in an address, and every word shown is said without it
//! (`hide_token`).
//!
//! Pause, Resume, Retry and Cancel reach the worker through [`Control`]; it never touches the
//! page's state. Every request waits while paused (upload_waits.rs `hold`). A Cancel is asked of
//! the server again while it is busy writing, and what it said comes back with the end
//! (upload_cancel.rs, round 4's D4-2). The network is behind
//! [`Transport`], so the tests run the whole flow against a server simulated in memory
//! (resumable_tests.rs).

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::upload_cancel::{self, Told};
use super::upload_waits::{self as waits, Backoff, Streak};
use crate::gui::widgets::image_cache::hide_token;

pub(crate) use super::upload_run::{stored_name_of, Phase, Run};

/// An upload that did not finish, kept in the app's config (`AppConfig::media_unfinished_uploads`)
/// so it can be carried on after the app restarts. It is carried on only on the same server, by
/// the same person, from the same file unchanged (its size and modification time).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Unfinished {
    /// The server (its normalised address) and the identity the upload belongs to.
    pub server: String,
    pub key: String,
    /// The server's id for the upload.
    pub id: String,
    pub path: PathBuf,
    /// The file's own name, as shown.
    pub name: String,
    pub size: u64,
    /// The file's modification time when the upload started, in ms since 1970.
    pub modified_ms: u64,
    /// What the form said, to publish it with once it is up.
    pub title: String,
    pub description: String,
    pub category_id: i64,
    pub labels: Vec<String>,
    pub members_only: bool,
    /// Media round 5: the licence and the game tick the form chose, published with it; and, for an
    /// upload made from an item's Edit window, the item it is added to when it is up instead of
    /// being published (api_items.rs `AttachTo`).
    pub licence: String,
    pub project_use: bool,
    pub attach: Option<super::api_items::AttachTo>,
    /// More files chosen with this one in Add to Media (2026-10-16): each is uploaded and added
    /// to the item once it is published, as Edit's Add a file adds one.
    pub more: Vec<MoreFile>,
    /// The chat channel to post it in once it is published (Add to Media's "Also post in chat",
    /// 2026-10-17), or empty. Kept with the record, so an upload carried on after a restart posts too.
    #[serde(default)]
    pub post_to: String,
}

/// A file chosen beside an Add to Media upload's own, added to its item once it is up.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MoreFile {
    pub path: PathBuf,
    /// The file's own name, as shown.
    pub name: String,
    pub size: u64,
}

impl MoreFile {
    /// `path` as a further file: its name and size, or why it cannot be read.
    pub fn of(path: &Path) -> Result<MoreFile, String> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
        match std::fs::metadata(path) {
            Ok(m) => Ok(MoreFile { path: path.to_path_buf(), name, size: m.len() }),
            Err(e) => Err(format!("{name} could not be read ({e}).")),
        }
    }
}

/// A file's modification time in ms since 1970, 0 when the system cannot say.
pub fn modified_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Unfinished {
    /// Whether the file on disk is still the one being uploaded: there, the same size, and not
    /// changed since. Only then can the pieces still to come follow the ones already sent.
    pub fn file_unchanged(&self) -> bool {
        std::fs::metadata(&self.path).map(|m| m.len() == self.size).unwrap_or(false) && modified_ms(&self.path) == self.modified_ms
    }
}

/// What is said when an unfinished upload's file changed since it started and the server does not
/// have every piece yet (with every piece there, the server's copy is finished instead: D4-1).
pub const FILE_CHANGED: &str = "The file changed since its upload started, so the rest cannot follow what was sent. Add the file again.";
/// Said after an upload that could not go on, once the server confirmed it dropped what arrived.
pub const NOTHING_STAYS: &str = "Nothing of it stays on the server.";
/// Why a cancel was not confirmed when the server's answer to the start never came: it may hold
/// the start, and then lists it with the person's unfinished uploads.
pub const START_UNANSWERED: &str = "the connection dropped while the server was starting it, so the server may hold the start; it is then listed below with your unfinished uploads on this server.";
/// Why a cancel was not asked: the upload was left on the server to be carried on.
const KEPT_FOR_LATER: &str = "it was left on the server to be carried on.";
/// What is said when the server no longer has the upload (one left alone for a day is removed).
pub const UPLOAD_GONE: &str = "This upload is no longer on the server: an upload left alone for a day is removed. Add the file again.";
/// What is said while the connection is down.
pub const RECONNECTING: &str = "The connection to the server dropped. The upload carries on from where the server got as soon as it answers again.";
/// What is said while the server has not signed this person in again yet (a reconnect).
pub const WAITING_FOR_SIGN_IN: &str = "Waiting for this server to sign you in again; the upload carries on then.";

// ── What the worker reports ─────────────────────────────────────────────────────────────────

/// What happened, sent from the worker to the page.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Hashing the file: this many bytes read so far.
    Hashing { done: u64 },
    /// The server started the upload under this id (the page keeps it in the config).
    Started { id: String },
    /// The server has this many bytes of the file.
    Progress { offset: u64 },
    /// Paused, and sending again.
    Paused,
    Sending,
    /// The role's day or hour allowance is used (429): its words, and when it resets (Unix ms).
    Waiting { words: String, resets_at: i64 },
    /// No room (507): the file is kept on the server; Retry tries again.
    NoRoom { words: String },
    /// The connection dropped, or the server needs a moment: carrying on as soon as it can.
    Reconnecting { words: String },
    /// The same server error ten times running: its words; Retry tries again.
    Stuck { words: String },
    /// Stored: the single request's answer, `{url, filename, size, type}`.
    Done { answer: Value },
    /// Stopped for good: the words, and whether the upload is gone from the server (so there is
    /// nothing left to carry on, and the kept record goes too).
    Failed { words: String, gone: bool },
    /// Cancelled at the person's word, and what the server said to it (round 4, D4-2: the page
    /// says "Cancelled" only for [`Told::Gone`]).
    Cancelled { told: Told },
    /// The file was cleaned into a copy, which is what goes up: where it is, its size and its
    /// modification time (the page keeps them in the upload's record, 2026-10-11).
    Cleaned { path: PathBuf, size: u64, modified_ms: u64 },
}

// ── Pause, Resume, Retry, Cancel and the token ──────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub(crate) struct Flags {
    pub paused: bool,
    pub cancelled: bool,
    /// Stopped without telling the server (the page moved to another server or person): the
    /// server keeps the upload a day, and the kept record offers it again there.
    pub abandoned: bool,
    /// Retry pressed (after no room, or a server error that would not clear) or Resume pressed
    /// while waiting for an allowance. Set only while the worker is in such a wait (`held`), and
    /// cleared when that wait ends (round 4, D4-4).
    pub retry: bool,
    /// The wait the worker is in that Retry or Resume ends, if any (upload_waits.rs).
    pub held: Option<Held>,
    pub token: Option<String>,
}

/// A wait of the worker's that a press ends: an allowance's (Resume or Retry), or one for Retry
/// (no room, or a server error that would not clear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Held {
    Allowance,
    Retry,
}

/// How the page steers the worker. Every change wakes it.
pub struct Control {
    flags: Mutex<Flags>,
    wake: Condvar,
    /// The first wait after a dropped connection; each one after doubles, up to 30 times this.
    pub(crate) pace: Duration,
    /// The least an allowance wait lasts (5 s; the tests that wait one out use less).
    pub(crate) allowance_min: Duration,
}

impl Control {
    pub fn new(token: Option<String>) -> Self {
        Self::with_pace(token, Duration::from_secs(1))
    }

    /// A control whose waits after a failure start at `pace` (the tests use a millisecond).
    pub fn with_pace(token: Option<String>, pace: Duration) -> Self {
        Self::with_waits(token, pace, waits::ALLOWANCE_WAIT_MIN)
    }

    /// A control with its waits set: after a failure from `pace`, and an allowance at least
    /// `allowance_min`.
    pub fn with_waits(token: Option<String>, pace: Duration, allowance_min: Duration) -> Self {
        Control { flags: Mutex::new(Flags { token, ..Default::default() }), wake: Condvar::new(), pace, allowance_min }
    }

    fn change(&self, f: impl FnOnce(&mut Flags)) {
        let mut flags = self.flags.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut flags);
        self.wake.notify_all();
    }

    pub fn pause(&self, on: bool) {
        self.change(|f| {
            f.paused = on;
            // Resume also ends a wait for an allowance, but only while the worker is in one: round
            // 3 set the flag at every Resume, and one pressed at any other moment stayed set until
            // it cut short the next stop for Retry or wait for an allowance (the review's D4-4).
            if !on && f.held == Some(Held::Allowance) {
                f.retry = true;
            }
        });
    }

    pub fn cancel(&self) {
        self.change(|f| f.cancelled = true);
    }

    /// Stop where it is without telling the server, which keeps the upload for a day.
    pub fn abandon(&self) {
        self.change(|f| {
            f.cancelled = true;
            f.abandoned = true;
        });
    }

    /// Try again now (after no room or a server error that would not clear), or carry on now
    /// (while waiting for an allowance). Nothing while the worker is in neither wait (D4-4).
    pub fn retry(&self) {
        self.change(|f| {
            if f.held.is_some() {
                f.retry = true;
            }
        });
    }

    /// The page's current upload token for this server (None while it is signing in again).
    pub fn set_token(&self, token: Option<String>) {
        let same = self.flags.lock().unwrap_or_else(|p| p.into_inner()).token == token;
        if !same {
            self.change(|f| f.token = token);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.flags().paused
    }

    pub fn is_cancelled(&self) -> bool {
        self.flags().cancelled
    }

    pub(crate) fn flags(&self) -> Flags {
        self.flags.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Wait while `keep_waiting` says so, at most `timeout` (None: until woken and done), and
    /// return the flags as they are then.
    pub(crate) fn wait(&self, timeout: Option<Duration>, keep_waiting: impl Fn(&Flags) -> bool) -> Flags {
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        let mut flags = self.flags.lock().unwrap_or_else(|p| p.into_inner());
        while keep_waiting(&flags) {
            match deadline {
                None => flags = self.wake.wait(flags).unwrap_or_else(|p| p.into_inner()),
                Some(d) => {
                    let now = std::time::Instant::now();
                    if now >= d {
                        break;
                    }
                    flags = self.wake.wait_timeout(flags, d - now).unwrap_or_else(|p| p.into_inner()).0;
                }
            }
        }
        flags.clone()
    }

    /// The worker enters a wait a press ends (upload_waits.rs), before the page is told of it, so
    /// a press that follows what the page shows always finds it.
    pub(crate) fn hold_in(&self, held: Held) {
        self.change(|f| {
            f.held = Some(held);
            f.retry = false;
        });
    }

    /// That wait is over: neither the wait nor a press made during it outlives it.
    pub(crate) fn release(&self) {
        let mut flags = self.flags.lock().unwrap_or_else(|p| p.into_inner());
        flags.held = None;
        flags.retry = false;
    }
}

// ── The network ─────────────────────────────────────────────────────────────────────────────

/// One answer from the server: its status and its body as text.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }

    /// The words a person reads: a JSON refusal's `error`, else the text, never an upload token.
    pub fn words(&self) -> String {
        let v = self.json();
        let text = match v.get("error").and_then(Value::as_str) {
            Some(e) => e.to_string(),
            None if self.body.trim().is_empty() || self.body.trim_start().starts_with('<') => format!("The server refused it ({}).", self.status),
            None => self.body.trim().to_string(),
        };
        hide_token(&text)
    }

    /// A refusal of the token itself: it is not a session signed in to this server (a reconnect
    /// replaced it). The upload waits for the new one.
    pub(crate) fn token_refused(&self) -> bool {
        self.status == 403 && (self.body.contains("Invalid upload token") || self.body.contains("not a session"))
    }
}

/// The four requests a resumable upload makes. `Err` is a request that never got an answer (the
/// connection dropped, the server could not be reached), in words with no token in them.
pub trait Transport: Send + Sync {
    fn start(&self, token: &str, body: &Value) -> Result<Reply, String>;
    fn status(&self, token: &str, id: &str) -> Result<Reply, String>;
    fn piece(&self, token: &str, id: &str, offset: u64, bytes: &[u8]) -> Result<Reply, String>;
    fn cancel(&self, token: &str, id: &str) -> Result<Reply, String>;
}

/// The real server at `base` (`https://host`).
pub struct Http {
    base: String,
    agent: ureq::Agent,
}

impl Http {
    pub fn new(base: &str) -> Self {
        // No timeout on a whole request: the last piece's answer waits while the server hashes and
        // stores the whole file (nginx allows it 15 minutes). Only a connection that will not
        // open, or goes quiet that long, gives up, and the upload then carries on from the status.
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(120))
            .timeout_read(Duration::from_secs(20 * 60))
            .build();
        Http { base: base.trim_end_matches('/').to_string(), agent }
    }

    /// A route's address. The token is not in it: it rides in a header ([`Http::with_token`]).
    pub(crate) fn url(&self, tail: &str) -> String {
        format!("{}/api/upload/resumable{tail}", self.base)
    }

    fn with_token(req: ureq::Request, token: &str) -> ureq::Request {
        req.set(super::api::TOKEN_HEADER, token)
    }

    fn answer(result: Result<ureq::Response, ureq::Error>) -> Result<Reply, String> {
        match result {
            Ok(r) => {
                let status = r.status();
                Ok(Reply { status, body: r.into_string().unwrap_or_default() })
            }
            Err(ureq::Error::Status(status, r)) => Ok(Reply { status, body: r.into_string().unwrap_or_default() }),
            Err(other) => Err(hide_token(&format!("The server could not be reached ({other})."))),
        }
    }
}

impl Transport for Http {
    fn start(&self, token: &str, body: &Value) -> Result<Reply, String> {
        let req = Self::with_token(self.agent.post(&self.url("")), token);
        Self::answer(req.set("Content-Type", "application/json").send_string(&body.to_string()))
    }
    fn status(&self, token: &str, id: &str) -> Result<Reply, String> {
        Self::answer(Self::with_token(self.agent.get(&self.url(&format!("/{id}"))), token).call())
    }
    fn piece(&self, token: &str, id: &str, offset: u64, bytes: &[u8]) -> Result<Reply, String> {
        let req = Self::with_token(self.agent.request("PATCH", &self.url(&format!("/{id}?offset={offset}"))), token);
        Self::answer(req.set("Content-Type", "application/octet-stream").send_bytes(bytes))
    }
    fn cancel(&self, token: &str, id: &str) -> Result<Reply, String> {
        Self::answer(Self::with_token(self.agent.delete(&self.url(&format!("/{id}"))), token).call())
    }
}

// ── The work ────────────────────────────────────────────────────────────────────────────────

/// What to upload.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub path: PathBuf,
    /// The file's own name, sent as `filename`.
    pub name: String,
    pub size: u64,
    pub content_type: String,
    /// Carry on with this upload (after a restart) instead of starting one.
    pub resume: Option<String>,
    /// The file on this computer is no longer the one being uploaded (changed, moved or gone since
    /// the upload started): it is carried on only when the server has every piece, and then
    /// finished from the server's copy; otherwise it is cancelled (round 4, D4-1).
    pub file_changed: bool,
    /// Remove location and camera details first (the files section of docs/design/
    /// media-showcase.md, 2026-10-11): before the hash, the worker cleans a COPY of the file
    /// (net/file_clean.rs `clean_copy`), never the person's own, and uploads the copy; the page is
    /// told where it is (`Event::Cleaned`), so an upload carried on after a restart reads the same
    /// bytes. A file that could not be checked is not sent (Media is a public shelf).
    pub clean: bool,
    /// The person kept the file's hidden details by choice: the start says `keep_metadata: 1`, and
    /// the relay then skips its own strip.
    pub keep_metadata: bool,
}

/// The start's body (the relay's `StartBody`).
pub fn start_body(plan: &Plan, blake3: &str) -> Value {
    let mut body = json!({ "filename": plan.name, "size": plan.size, "content_type": plan.content_type, "media": 1, "blake3": blake3 });
    if plan.keep_metadata {
        body["keep_metadata"] = json!(1);
    }
    body
}

/// Why a Media upload was not sent: its file could not be checked for hidden location, and Media is
/// a public shelf (the relay would refuse it too).
pub const NOT_CHECKED_MEDIA: &str = "This file could not be read all the way through, so where it was taken may still be in it. It was not sent: Media is public. To send it exactly as it is, turn off \"Remove location and camera details\".";

/// The cleaning step of `work`: a cleaned copy to upload instead of `plan.path` (its path, size and
/// modification time), None to upload the file itself, or Err with the words to stop with.
pub fn clean_first(plan: &Plan, dir: &Path) -> Result<Option<(PathBuf, u64, u64)>, String> {
    match crate::net::file_clean::clean_copy(&plan.path, &plan.name, dir) {
        Ok((Some(copy), _)) => {
            let size = std::fs::metadata(&copy).map(|m| m.len()).map_err(|e| format!("The cleaned copy could not be read ({e})."))?;
            let modified = modified_ms(&copy);
            Ok(Some((copy, size, modified)))
        }
        Ok((None, c)) if c.unchecked => Err(NOT_CHECKED_MEDIA.to_string()),
        Ok((None, _)) => Ok(None),
        Err(e) => Err(format!("The file could not be read ({e}).")),
    }
}

/// The file's BLAKE3 hash in lowercase hex, read a megabyte at a time, reporting progress and
/// waiting while paused; None when cancelled on the way.
pub fn hash_file(path: &Path, ctl: &Control, tx: &Sender<Event>) -> Result<Option<String>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("The file could not be read ({e})."))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        if !waits::hold(ctl, tx) {
            return Ok(None);
        }
        let n = f.read(&mut buf).map_err(|e| format!("The file could not be read ({e})."))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        let _ = tx.send(Event::Hashing { done });
    }
    Ok(Some(hasher.finalize().to_hex().to_string()))
}

fn read_piece(path: &Path, at: u64, len: u64) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("The file could not be read any more, so the upload stopped ({e})."))?;
    f.seek(SeekFrom::Start(at)).map_err(|e| format!("The file could not be read any more, so the upload stopped ({e})."))?;
    let mut bytes = vec![0u8; len as usize];
    f.read_exact(&mut bytes).map_err(|e| format!("The file could not be read any more: it may have changed or been moved, so the upload stopped ({e})."))?;
    Ok(bytes)
}

/// The current token, waiting while there is none or while it is the one the server refused.
/// None when cancelled meanwhile.
fn token_for(ctl: &Control, refused: &Option<String>, tx: &Sender<Event>) -> Option<String> {
    let flags = ctl.flags();
    let usable = |f: &Flags| f.token.is_some() && &f.token != refused;
    if !usable(&flags) {
        let _ = tx.send(Event::Reconnecting { words: WAITING_FOR_SIGN_IN.to_string() });
        let flags = ctl.wait(None, |f| !f.cancelled && !usable(f));
        if flags.cancelled {
            return None;
        }
        return flags.token;
    }
    flags.token
}

/// Tell the server to drop the upload, unless it was abandoned rather than cancelled: asked again
/// while the server is busy writing, and what it said comes back (round 4, D4-2: round 3 sent one
/// request and took any answer, a 409 busy included, as done).
fn cancel_on_server(t: &dyn Transport, ctl: &Control, id: &str) -> Told {
    let flags = ctl.flags();
    if flags.abandoned {
        return Told::NotConfirmed(KEPT_FOR_LATER.to_string());
    }
    let Some(token) = flags.token else { return Told::NotConfirmed(upload_cancel::NOT_SIGNED_IN.to_string()) };
    let told = upload_cancel::tell(t, &token, id, ctl.pace);
    if let Told::NotConfirmed(why) = &told {
        log::warn!("Media upload: the server did not confirm the cancel ({why}); it drops the upload a day after its last piece.");
    }
    told
}

/// The upload cannot go on (`words`): the server is told to drop what arrived, and the record is
/// forgotten only when it confirmed (otherwise it stays, offered with Cancel it).
fn stop_with(t: &dyn Transport, ctl: &Control, id: &str, words: String) -> Event {
    match cancel_on_server(t, ctl, id) {
        Told::NotConfirmed(why) => Event::Failed { words: format!("{words} {}", upload_cancel::not_cancelled_words(&why)), gone: false },
        Told::Gone | Told::Published => Event::Failed { words: format!("{words} {NOTHING_STAYS}"), gone: true },
    }
}

/// What a request's answer means for the worker, the same for all three requests (start, status,
/// piece); each handles [`Answer::Other`] in its own way.
enum Answer {
    Ok(Value),
    TokenRefused,
    Allowance(waits::Allowance),
    /// A 429 that is not an allowance: waited out (its words).
    Busy(String),
    /// A 507: no room (its words).
    NoRoom(String),
    /// Any other 5xx.
    ServerError(Reply),
    /// No answer at all.
    Lost,
    Other(Reply),
}

fn answer_of(result: Result<Reply, String>, ctl: &Control) -> Answer {
    let r = match result {
        Ok(r) => r,
        Err(_) => return Answer::Lost,
    };
    match r.status {
        200..=299 => Answer::Ok(r.json()),
        _ if r.token_refused() => Answer::TokenRefused,
        429 => match waits::allowance_of(&r, ctl.allowance_min) {
            Some(a) => Answer::Allowance(a),
            None => Answer::Busy(waits::busy_words(&r)),
        },
        507 => Answer::NoRoom(r.words()),
        500..=599 => Answer::ServerError(r),
        _ => Answer::Other(r),
    }
}

/// The worker's waits and counts, shared by the start and the pieces.
struct Waits<'a> {
    ctl: &'a Control,
    tx: &'a Sender<Event>,
    backoff: Backoff,
    streak: Streak,
}

impl Waits<'_> {
    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
    }

    /// A dropped connection, or a 429 that is not an allowance: say so and wait a little longer
    /// each time.
    fn after_trouble(&mut self, words: &str) {
        self.streak.clear();
        self.send(Event::Reconnecting { words: words.to_string() });
        self.backoff.wait(self.ctl);
    }

    /// A server error: wait, or, after ten of the same, show its words and wait for Retry.
    /// False when cancelled meanwhile.
    fn after_server_error(&mut self, r: &Reply) -> bool {
        if self.streak.note(r) >= waits::STUCK_AFTER {
            let flags = waits::wait_for_retry(self.ctl, self.tx, Event::Stuck { words: r.words() });
            self.streak.clear();
            self.backoff.reset();
            if flags.cancelled {
                return false;
            }
            self.send(Event::Sending);
            return true;
        }
        self.send(Event::Reconnecting { words: waits::SERVER_TROUBLE.to_string() });
        self.backoff.wait(self.ctl);
        true
    }

    /// An allowance used up: say so and wait the server's own number, or Resume.
    fn after_allowance(&mut self, a: &waits::Allowance) {
        self.streak.clear();
        let flags = waits::wait_for_allowance(self.ctl, a, self.tx, Event::Waiting { words: a.words.clone(), resets_at: a.resets_at() });
        if !flags.cancelled && !flags.paused {
            self.send(Event::Sending);
        }
    }

    /// No room: show the words and wait for Retry. False when cancelled meanwhile.
    fn after_no_room(&mut self, words: String) -> bool {
        self.streak.clear();
        let flags = waits::wait_for_retry(self.ctl, self.tx, Event::NoRoom { words });
        if flags.cancelled {
            return false;
        }
        self.send(Event::Sending);
        true
    }

    fn fine(&mut self) {
        self.streak.clear();
        self.backoff.reset();
    }
}

/// Upload `plan` through `t`, reporting on `tx`. Blocking: run it on a worker thread. Ends with
/// `Done`, `Failed` or `Cancelled`.
pub fn work(t: &dyn Transport, ctl: &Control, plan: &Plan, tx: &Sender<Event>) {
    let mut w = Waits { ctl, tx, backoff: Backoff::new(ctl.pace), streak: Streak::default() };
    // 0: cleaned on this device first (a new upload only: one carried on already reads its copy).
    let cleaned_plan;
    let plan = if plan.clean && plan.resume.is_none() {
        match clean_first(plan, &crate::net::file_clean::cleaned_dir()) {
            Ok(Some((path, size, modified_ms))) => {
                w.send(Event::Cleaned { path: path.clone(), size, modified_ms });
                cleaned_plan = Plan { path, size, ..plan.clone() };
                &cleaned_plan
            }
            Ok(None) => plan,
            Err(words) => return w.send(Event::Failed { words, gone: true }),
        }
    } else {
        plan
    };
    let mut refused: Option<String> = None;
    let mut chunk = waits::DEFAULT_CHUNK;

    // 1 and 2: the hash and the start, unless carrying on with an upload already started. A
    // cancel before the server named the upload has nothing to tell it, unless a start's answer
    // was lost: then the server may hold it, and the person is told so, not "Cancelled".
    let mut start_unanswered = false;
    let cancelled = |unanswered: bool| Event::Cancelled { told: if unanswered { Told::NotConfirmed(START_UNANSWERED.to_string()) } else { Told::Gone } };
    let id = match plan.resume.clone() {
        Some(id) => id,
        None => {
            let hash = match hash_file(&plan.path, ctl, tx) {
                Ok(Some(h)) => h,
                Ok(None) => return w.send(cancelled(false)),
                Err(words) => return w.send(Event::Failed { words, gone: true }),
            };
            let body = start_body(plan, &hash);
            loop {
                if !waits::hold(ctl, tx) {
                    return w.send(cancelled(start_unanswered));
                }
                let Some(token) = token_for(ctl, &refused, tx) else { return w.send(cancelled(start_unanswered)) };
                if !waits::hold(ctl, tx) {
                    return w.send(cancelled(start_unanswered));
                }
                match answer_of(t.start(&token, &body), ctl) {
                    Answer::Ok(v) => {
                        let Some(id) = v.get("id").and_then(Value::as_str).map(str::to_string) else {
                            return w.send(Event::Failed { words: "The server's answer to the start was not understood.".into(), gone: true });
                        };
                        chunk = waits::piece_size(v.get("chunk_max").and_then(Value::as_u64));
                        w.fine();
                        w.send(Event::Started { id: id.clone() });
                        break id;
                    }
                    Answer::TokenRefused => refused = Some(token),
                    Answer::Allowance(a) => w.after_allowance(&a),
                    Answer::Busy(words) => w.after_trouble(&words),
                    Answer::NoRoom(words) => {
                        if !w.after_no_room(words) {
                            return w.send(cancelled(start_unanswered));
                        }
                    }
                    Answer::ServerError(r) => {
                        if !w.after_server_error(&r) {
                            return w.send(cancelled(start_unanswered));
                        }
                    }
                    Answer::Lost => {
                        start_unanswered = true;
                        w.after_trouble(RECONNECTING);
                    }
                    Answer::Other(r) => return w.send(Event::Failed { words: r.words(), gone: true }),
                }
            }
        }
    };

    // 3: the pieces. `offset` None means "ask the server where it got": what an upload carried on
    // after a restart starts with; a new one starts at 0, as its start said. `cap` is what is left
    // of an allowance's window, when that is less than a piece.
    let mut offset: Option<u64> = if plan.resume.is_some() { None } else { Some(0) };
    let mut cap: Option<u64> = None;
    loop {
        if !waits::hold(ctl, tx) {
            let told = cancel_on_server(t, ctl, &id);
            return w.send(Event::Cancelled { told });
        }
        let Some(token) = token_for(ctl, &refused, tx) else { continue };
        if !waits::hold(ctl, tx) {
            continue; // cancelled meanwhile: the top of the loop tells the server
        }
        let at = match offset {
            Some(at) => at,
            None => match answer_of(t.status(&token, &id), ctl) {
                Answer::Ok(v) => {
                    if let Some(done) = v.get("done").filter(|d| !d.is_null()) {
                        return w.send(Event::Done { answer: done.clone() });
                    }
                    chunk = waits::piece_size(v.get("chunk_max").and_then(Value::as_u64).or(Some(chunk)));
                    let at = v.get("offset").and_then(Value::as_u64).unwrap_or(0);
                    w.send(Event::Progress { offset: at });
                    offset = Some(at);
                    at
                }
                Answer::Other(r) if r.status == 404 => return w.send(Event::Failed { words: UPLOAD_GONE.to_string(), gone: true }),
                Answer::TokenRefused => {
                    refused = Some(token);
                    continue;
                }
                Answer::Allowance(a) => {
                    w.after_allowance(&a);
                    continue;
                }
                Answer::Busy(words) => {
                    w.after_trouble(&words);
                    continue;
                }
                Answer::NoRoom(words) => {
                    w.after_no_room(words);
                    continue;
                }
                Answer::ServerError(r) => {
                    w.after_server_error(&r);
                    continue; // cancelled meanwhile or not, the top of the loop decides
                }
                Answer::Lost => {
                    w.after_trouble(RECONNECTING);
                    continue;
                }
                Answer::Other(r) => return w.send(Event::Failed { words: r.words(), gone: false }),
            },
        };
        if !waits::hold(ctl, tx) {
            continue; // paused between the status and the piece, then cancelled
        }
        // The next piece (at most what is left of an allowance's window); at the end, the empty
        // piece that (tries the) finish. The empty piece never touches the file on this computer
        // (round 4, D4-1): every piece is on the server then, so a file moved, changed or gone
        // since cannot stop the finish, and above all never cancels the server's whole copy (a
        // 507's Retry, or Carry on after a restart, finishes from it).
        let len = chunk.min(cap.unwrap_or(u64::MAX)).min(plan.size.saturating_sub(at));
        let bytes = if len == 0 {
            Vec::new()
        } else if plan.file_changed {
            // Pieces are still to come, and the file they would come from is not the one sent.
            return w.send(stop_with(t, ctl, &id, FILE_CHANGED.to_string()));
        } else {
            match read_piece(&plan.path, at, len) {
                Ok(b) => b,
                Err(words) => return w.send(stop_with(t, ctl, &id, words)),
            }
        };
        match answer_of(t.piece(&token, &id, at, &bytes), ctl) {
            Answer::Ok(v) => {
                if v.get("url").is_some() {
                    return w.send(Event::Done { answer: v });
                }
                let got = v.get("offset").and_then(Value::as_u64).unwrap_or(at + len);
                offset = Some(got);
                cap = cap.map(|c| c.saturating_sub(got.saturating_sub(at))).filter(|c| *c > 0);
                w.fine();
                w.send(Event::Progress { offset: got });
            }
            Answer::Other(r) if r.status == 409 => {
                let v = r.json();
                let there = v.get("offset").and_then(Value::as_u64);
                if v.get("busy").and_then(Value::as_bool) == Some(true) || there.is_none() || there == Some(at) {
                    // Another piece is being written, or the server named no other place: wait
                    // and ask, never at once (a 409 at the same place again would loop).
                    w.backoff.wait(ctl);
                    offset = None;
                } else {
                    offset = there;
                }
            }
            Answer::Allowance(a) => {
                offset = a.offset.or(offset);
                match a.left.filter(|left| *left > 0 && *left < len) {
                    // What still fits goes now: the review's finding 5 (an allowance smaller than
                    // a piece never moved, because every piece was a whole `chunk_max`).
                    Some(left) => cap = Some(left),
                    None => {
                        w.after_allowance(&a);
                        cap = None;
                    }
                }
            }
            Answer::Busy(words) => {
                w.after_trouble(&words);
                offset = None;
            }
            Answer::NoRoom(words) => {
                // The server keeps the whole file for a day. Retry sends the empty piece again.
                offset = None;
                if !w.after_no_room(words) {
                    continue;
                }
            }
            Answer::Other(r) if r.status == 404 => return w.send(Event::Failed { words: UPLOAD_GONE.to_string(), gone: true }),
            Answer::Other(r) if r.status == 410 || r.status == 422 => return w.send(Event::Failed { words: r.words(), gone: true }),
            Answer::TokenRefused => refused = Some(token),
            Answer::ServerError(r) => {
                offset = None;
                if !w.after_server_error(&r) {
                    continue;
                }
            }
            Answer::Lost => {
                // The answer was lost: the server may or may not have the piece. Ask it.
                w.after_trouble(RECONNECTING);
                offset = None;
            }
            Answer::Other(r) => {
                // Anything else (a piece the server will not take, a role that may no longer
                // upload) cannot be carried on: the server is told, and the person why.
                return w.send(stop_with(t, ctl, &id, r.words()));
            }
        }
    }
}

#[cfg(test)]
#[path = "resumable_tests.rs"]
pub(crate) mod tests;
#[cfg(test)]
#[path = "resumable_clean_tests.rs"]
mod clean_tests;
