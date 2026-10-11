//! Add to Media's upload, sent in pieces (the relay half is src/relay/resumable.rs; the design is
//! docs/design/media-showcase.md, "Resumable uploads, exact" and its As built note). The
//! operator's question was "What if I want to upload a multi-GB file?": one request carries at
//! most 1 GB, and a connection that drops at 900 MB loses all of it. Here a file goes a piece at
//! a time; the server writes and counts each before the next is sent, so after a drop the app asks
//! how far the server got and carries on from there.
//!
//! THE STEPS, all on a worker thread ([`work`]):
//! 1. The file's BLAKE3 hash, read a megabyte at a time (the server checks the finished file
//!    against it, so a resume that went wrong is caught, not stored).
//! 2. The start: `POST /api/upload/resumable?token=` with `{filename, size, content_type, media:
//!    1, blake3}`, answered `{id, offset: 0, size, chunk_max}`. The page keeps the id (and the
//!    file's path, size and modification time) in the app's config, so an upload cut off by a
//!    restart can be carried on ([`Unfinished`]).
//! 3. The pieces: `PATCH .../{id}?token=&offset=<n>`, each at most the server's `chunk_max`.
//!    A 409 with an `offset` says where the server is; a 409 `busy` means another piece is being
//!    written, so it waits and asks. A dropped connection or a server error waits a little longer
//!    each time, asks where the server got (`GET .../{id}`), and carries on. A 429 is the role's
//!    day or hour allowance: its own words and when it resets are shown, and the upload carries on
//!    by itself then (or at Resume). A 507 at the finish is no room left: the file is kept on the
//!    server for a day, and Retry sends the empty piece at the end, which tries the finish again.
//!    A finish whose answer was lost is found in the status answer's `done`.
//! 4. The finished upload's answer (`{url, filename, size, type}`) goes back to the page, which
//!    publishes it then (signed then: a signature is good for five minutes, and an upload can
//!    take hours).
//!
//! The token: each connection to the server has its own upload token, and a reconnect replaces it
//! (the relay's C). The page hands the worker the current one every frame ([`Control::set_token`]);
//! a session belongs to the person, so any of their tokens carries it on. The token rides in the
//! address here, because these routes read it there; every word shown is said without it
//! (`hide_token`).
//!
//! Pause, Resume, Retry and Cancel reach the worker through [`Control`]; it never touches the
//! page's state. The network is behind [`Transport`], so the tests run the whole flow against a
//! server simulated in memory (resumable_tests.rs).

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::gui::widgets::image_cache::hide_token;

/// An upload that did not finish, kept in the app's config (`AppConfig::media_unfinished_upload`)
/// so it can be carried on after the app restarts. It is carried on only on the same server, by
/// the same person, from the same file unchanged (its size and modification time).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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

/// What is said when an unfinished upload's file changed since it started.
pub const FILE_CHANGED: &str = "The file changed since its upload started, so the rest cannot follow what was sent. That upload was cancelled; add the file again.";
/// What is said when the server no longer has the upload (one left alone for a day is removed).
pub const UPLOAD_GONE: &str = "This upload is no longer on the server: an upload left alone for a day is removed. Add the file again.";
/// What is said while the connection is down.
pub const RECONNECTING: &str = "The connection to the server dropped. The upload carries on from where the server got as soon as it answers again.";
/// What is said while the server has not signed this person in again yet (a reconnect).
pub const WAITING_FOR_SIGN_IN: &str = "Waiting for this server to sign you in again; the upload carries on then.";

/// The piece size until the server states its own (`chunk_max`, 8 MiB on the relay).
const DEFAULT_CHUNK: u64 = 8 * 1024 * 1024;

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
    /// No room at the finish (507): the file is kept on the server; Retry tries the finish again.
    NoRoom { words: String },
    /// The connection dropped, or the server needs a moment: carrying on as soon as it can.
    Reconnecting { words: String },
    /// Stored: the single request's answer, `{url, filename, size, type}`.
    Done { answer: Value },
    /// Stopped for good: the words, and whether the upload is gone from the server (so there is
    /// nothing left to carry on, and the kept record goes too).
    Failed { words: String, gone: bool },
    /// Cancelled at the person's word, and the server told.
    Cancelled,
}

// ── Pause, Resume, Retry, Cancel and the token ──────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct Flags {
    paused: bool,
    cancelled: bool,
    /// Stopped without telling the server (the page moved to another server or person): the
    /// server keeps the upload a day, and the kept record offers it again there.
    abandoned: bool,
    /// Retry pressed (after no room) or Resume pressed while waiting for an allowance.
    retry: bool,
    token: Option<String>,
}

/// How the page steers the worker. Every change wakes it.
pub struct Control {
    flags: Mutex<Flags>,
    wake: Condvar,
    /// The first wait after a dropped connection; each one after doubles, up to 30 times this.
    pace: Duration,
}

impl Control {
    pub fn new(token: Option<String>) -> Self {
        Self::with_pace(token, Duration::from_secs(1))
    }

    /// A control whose waits after a failure start at `pace` (the tests use a millisecond).
    pub fn with_pace(token: Option<String>, pace: Duration) -> Self {
        Control { flags: Mutex::new(Flags { token, ..Default::default() }), wake: Condvar::new(), pace }
    }

    fn change(&self, f: impl FnOnce(&mut Flags)) {
        let mut flags = self.flags.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut flags);
        self.wake.notify_all();
    }

    pub fn pause(&self, on: bool) {
        self.change(|f| {
            f.paused = on;
            if !on {
                f.retry = true; // Resume also ends a wait for an allowance
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

    /// Try the finish again (after no room), or carry on now (while waiting for an allowance).
    pub fn retry(&self) {
        self.change(|f| f.retry = true);
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

    fn flags(&self) -> Flags {
        self.flags.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Wait while `keep_waiting` says so, at most `timeout` (None: until woken and done), and
    /// return the flags as they are then.
    fn wait(&self, timeout: Option<Duration>, keep_waiting: impl Fn(&Flags) -> bool) -> Flags {
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

    fn take_retry(&self) -> bool {
        let mut flags = self.flags.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::replace(&mut flags.retry, false)
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
    fn token_refused(&self) -> bool {
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

    fn url(&self, tail: &str, token: &str) -> String {
        let enc: String = url::form_urlencoded::byte_serialize(token.as_bytes()).collect();
        let sep = if tail.contains('?') { '&' } else { '?' };
        format!("{}/api/upload/resumable{tail}{sep}token={enc}", self.base)
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
        Self::answer(self.agent.post(&self.url("", token)).set("Content-Type", "application/json").send_string(&body.to_string()))
    }
    fn status(&self, token: &str, id: &str) -> Result<Reply, String> {
        Self::answer(self.agent.get(&self.url(&format!("/{id}"), token)).call())
    }
    fn piece(&self, token: &str, id: &str, offset: u64, bytes: &[u8]) -> Result<Reply, String> {
        let url = self.url(&format!("/{id}?offset={offset}"), token);
        Self::answer(self.agent.request("PATCH", &url).set("Content-Type", "application/octet-stream").send_bytes(bytes))
    }
    fn cancel(&self, token: &str, id: &str) -> Result<Reply, String> {
        Self::answer(self.agent.delete(&self.url(&format!("/{id}"), token)).call())
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
}

/// The start's body (the relay's `StartBody`).
pub fn start_body(plan: &Plan, blake3: &str) -> Value {
    json!({ "filename": plan.name, "size": plan.size, "content_type": plan.content_type, "media": 1, "blake3": blake3 })
}

/// The file's BLAKE3 hash in lowercase hex, read a megabyte at a time, reporting progress; None
/// when cancelled on the way.
pub fn hash_file(path: &Path, ctl: &Control, mut progress: impl FnMut(u64)) -> Result<Option<String>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("The file could not be read ({e})."))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        if ctl.is_cancelled() {
            return Ok(None);
        }
        let n = f.read(&mut buf).map_err(|e| format!("The file could not be read ({e})."))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(done);
    }
    Ok(Some(hasher.finalize().to_hex().to_string()))
}

fn read_piece(path: &Path, at: u64, len: u64) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("The file could not be read ({e})."))?;
    f.seek(SeekFrom::Start(at)).map_err(|e| format!("The file could not be read ({e})."))?;
    let mut bytes = vec![0u8; len as usize];
    f.read_exact(&mut bytes).map_err(|e| format!("The file could not be read: it may have changed ({e})."))?;
    Ok(bytes)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Waits after a failure: `pace`, then twice that, up to thirty times it.
struct Backoff {
    pace: Duration,
    next: Duration,
}

impl Backoff {
    fn new(pace: Duration) -> Self {
        Backoff { pace, next: pace }
    }
    fn reset(&mut self) {
        self.next = self.pace;
    }
    /// Wait out the next step (woken early by Pause, Cancel or a new token).
    fn wait(&mut self, ctl: &Control) {
        let token = ctl.flags().token;
        ctl.wait(Some(self.next), |f| !f.cancelled && !f.paused && f.token == token);
        self.next = (self.next * 2).min(self.pace * 30);
    }
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

/// Tell the server to drop the upload, as best it can be told (it expires in a day anyway), unless
/// it was abandoned rather than cancelled.
fn cancel_on_server(t: &dyn Transport, ctl: &Control, id: &str) {
    let flags = ctl.flags();
    if flags.abandoned {
        return;
    }
    if let Some(token) = flags.token {
        if let Err(e) = t.cancel(&token, id) {
            log::warn!("Media upload: the server could not be told of the cancel ({e}); it removes the upload after a day.");
        }
    }
}

/// Wait while an allowance is used up: until it resets, or Resume, Pause or Cancel.
fn wait_for_allowance(ctl: &Control, resets_at: i64) -> Flags {
    let left = Duration::from_millis((resets_at - now_ms()).max(0) as u64);
    ctl.wait(Some(left), |f| !f.cancelled && !f.retry && !f.paused)
}

/// Upload `plan` through `t`, reporting on `tx`. Blocking: run it on a worker thread. Ends with
/// `Done`, `Failed` or `Cancelled`.
pub fn work(t: &dyn Transport, ctl: &Control, plan: &Plan, tx: &Sender<Event>) {
    let send = |e: Event| {
        let _ = tx.send(e);
    };
    let mut refused: Option<String> = None;
    let mut chunk = DEFAULT_CHUNK;
    let mut backoff = Backoff::new(ctl.pace);

    // 1 and 2: the hash and the start, unless carrying on with an upload already started.
    let id = match plan.resume.clone() {
        Some(id) => id,
        None => {
            let hash = match hash_file(&plan.path, ctl, |done| send(Event::Hashing { done })) {
                Ok(Some(h)) => h,
                Ok(None) => return send(Event::Cancelled),
                Err(words) => return send(Event::Failed { words, gone: true }),
            };
            let body = start_body(plan, &hash);
            loop {
                if ctl.is_cancelled() {
                    return send(Event::Cancelled);
                }
                let Some(token) = token_for(ctl, &refused, tx) else { return send(Event::Cancelled) };
                match t.start(&token, &body) {
                    Ok(r) if r.status == 200 || r.status == 201 => {
                        let v = r.json();
                        let Some(id) = v.get("id").and_then(Value::as_str).map(str::to_string) else {
                            return send(Event::Failed { words: "The server's answer to the start was not understood.".into(), gone: true });
                        };
                        chunk = v.get("chunk_max").and_then(Value::as_u64).filter(|c| *c > 0).unwrap_or(DEFAULT_CHUNK);
                        send(Event::Started { id: id.clone() });
                        break id;
                    }
                    Ok(r) if r.status == 429 && r.json().get("resets_at").is_some() => {
                        let resets_at = r.json().get("resets_at").and_then(Value::as_i64).unwrap_or(0);
                        send(Event::Waiting { words: r.words(), resets_at });
                        let flags = wait_for_allowance(ctl, resets_at);
                        ctl.take_retry();
                        if !flags.cancelled && !flags.paused {
                            send(Event::Sending);
                        }
                    }
                    Ok(r) if r.token_refused() => refused = Some(token),
                    Ok(r) if r.status >= 500 && r.status != 507 => {
                        send(Event::Reconnecting { words: RECONNECTING.to_string() });
                        backoff.wait(ctl);
                    }
                    Ok(r) => return send(Event::Failed { words: r.words(), gone: true }),
                    Err(_) => {
                        send(Event::Reconnecting { words: RECONNECTING.to_string() });
                        backoff.wait(ctl);
                    }
                }
            }
        }
    };

    // 3: the pieces. `offset` None means "ask the server where it got": what an upload carried
    // on after a restart starts with; a new one starts at 0, as its start said.
    let mut offset: Option<u64> = if plan.resume.is_some() { None } else { Some(0) };
    loop {
        let flags = ctl.flags();
        if flags.cancelled {
            cancel_on_server(t, ctl, &id);
            return send(Event::Cancelled);
        }
        if flags.paused {
            send(Event::Paused);
            let flags = ctl.wait(None, |f| f.paused && !f.cancelled);
            ctl.take_retry();
            if !flags.cancelled {
                send(Event::Sending);
            }
            continue;
        }
        let Some(token) = token_for(ctl, &refused, tx) else { continue };
        let at = match offset {
            Some(at) => at,
            None => match t.status(&token, &id) {
                Ok(r) if r.status == 200 => {
                    let v = r.json();
                    if let Some(done) = v.get("done").filter(|d| !d.is_null()) {
                        return send(Event::Done { answer: done.clone() });
                    }
                    chunk = v.get("chunk_max").and_then(Value::as_u64).filter(|c| *c > 0).unwrap_or(chunk);
                    let at = v.get("offset").and_then(Value::as_u64).unwrap_or(0);
                    send(Event::Progress { offset: at });
                    offset = Some(at);
                    at
                }
                Ok(r) if r.status == 404 => return send(Event::Failed { words: UPLOAD_GONE.to_string(), gone: true }),
                Ok(r) if r.token_refused() => {
                    refused = Some(token);
                    continue;
                }
                Ok(r) if r.status >= 500 => {
                    send(Event::Reconnecting { words: RECONNECTING.to_string() });
                    backoff.wait(ctl);
                    continue;
                }
                Ok(r) => return send(Event::Failed { words: r.words(), gone: false }),
                Err(_) => {
                    send(Event::Reconnecting { words: RECONNECTING.to_string() });
                    backoff.wait(ctl);
                    continue;
                }
            },
        };
        // The next piece; at the end, the empty piece that (tries the) finish.
        let len = chunk.min(plan.size.saturating_sub(at));
        let bytes = match read_piece(&plan.path, at, len) {
            Ok(b) => b,
            Err(words) => {
                cancel_on_server(t, ctl, &id);
                return send(Event::Failed { words, gone: true });
            }
        };
        match t.piece(&token, &id, at, &bytes) {
            Ok(r) if r.status == 200 => {
                let v = r.json();
                if v.get("url").is_some() {
                    return send(Event::Done { answer: v });
                }
                let got = v.get("offset").and_then(Value::as_u64).unwrap_or(at + len);
                offset = Some(got);
                backoff.reset();
                send(Event::Progress { offset: got });
            }
            Ok(r) if r.status == 409 => {
                let v = r.json();
                if v.get("busy").and_then(Value::as_bool) == Some(true) {
                    backoff.wait(ctl);
                    offset = None;
                } else {
                    offset = v.get("offset").and_then(Value::as_u64);
                }
            }
            Ok(r) if r.status == 429 => {
                let v = r.json();
                let resets_at = v.get("resets_at").and_then(Value::as_i64).unwrap_or_else(|| now_ms() + 60_000);
                send(Event::Waiting { words: r.words(), resets_at });
                let flags = wait_for_allowance(ctl, resets_at);
                ctl.take_retry();
                offset = v.get("offset").and_then(Value::as_u64).or(offset);
                if !flags.cancelled && !flags.paused {
                    send(Event::Sending);
                }
            }
            Ok(r) if r.status == 507 => {
                // No room at the finish: the server keeps the whole file for a day. Wait for Retry.
                send(Event::NoRoom { words: r.words() });
                let flags = ctl.wait(None, |f| !f.cancelled && !f.retry);
                ctl.take_retry();
                offset = None;
                if !flags.cancelled {
                    send(Event::Sending);
                }
            }
            Ok(r) if r.status == 404 => return send(Event::Failed { words: UPLOAD_GONE.to_string(), gone: true }),
            Ok(r) if r.status == 410 || r.status == 422 => return send(Event::Failed { words: r.words(), gone: true }),
            Ok(r) if r.token_refused() => refused = Some(token),
            Ok(r) if r.status >= 500 => {
                send(Event::Reconnecting { words: RECONNECTING.to_string() });
                backoff.wait(ctl);
                offset = None;
            }
            Ok(r) => {
                // Anything else (a piece the server will not take, a role that may no longer
                // upload) cannot be carried on: the server is told, and the person why.
                cancel_on_server(t, ctl, &id);
                return send(Event::Failed { words: r.words(), gone: true });
            }
            Err(_) => {
                // The answer was lost: the server may or may not have the piece. Ask it.
                send(Event::Reconnecting { words: RECONNECTING.to_string() });
                backoff.wait(ctl);
                offset = None;
            }
        }
    }
}

// ── The page's handle on a running upload ───────────────────────────────────────────────────

/// Where a running upload is, as the page shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Hashing,
    Sending,
    Paused,
    Waiting { words: String, resets_at: i64 },
    NoRoom { words: String },
    Reconnecting { words: String },
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
                Event::Hashing { done } => self.hashed = *done,
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
                Event::Sending => self.phase = Phase::Sending,
                Event::Waiting { words, resets_at } => self.phase = Phase::Waiting { words: words.clone(), resets_at: *resets_at },
                Event::NoRoom { words } => self.phase = Phase::NoRoom { words: words.clone() },
                Event::Reconnecting { words } => self.phase = Phase::Reconnecting { words: words.clone() },
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
                Event::Cancelled => {
                    self.phase = Phase::Cancelled;
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
            Phase::Done { .. } => "Uploaded. Putting it on the Media page...".to_string(),
            Phase::Failed { words } => words.clone(),
            Phase::Cancelled => "Cancelled.".to_string(),
        }
    }
}

/// "at 14:00 UTC" style words for when an allowance resets, or "now" once it has.
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

#[cfg(test)]
#[path = "resumable_tests.rs"]
pub(crate) mod tests;
