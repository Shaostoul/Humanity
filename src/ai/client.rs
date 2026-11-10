//! The app's side of the AI engine (docs/design/embeddable-ai.md, 7.2): start it when needed, talk to
//! it over pipes, stop it when idle, and survive it.
//!
//! - **Started only when needed**, by its full path, with its standard input and output connected (no
//!   socket, no listener, so no firewall question and no other program can reach it), with no console
//!   window, at below-normal priority, and placed in a job object (job.rs) that caps its memory and ends
//!   it with the app, even when the app crashes. The engine reads nothing before its first message, so
//!   it is in the job before it does any work.
//! - **Every message carries the protocol version**; an engine that answers in another version is
//!   refused with words and never used until Try again ([`State::Refused`]).
//! - **Stopped after a few idle minutes** ([`ClientConfig::idle_stop`]), and always when the app
//!   closes (the job's kill on close; its standard input closing ends it too).
//! - **Crash and out of memory**: if the engine dies, every request it held falls back at once
//!   ([`Outcome::Fallback`]: the feature uses its written lines, subtitles or no read-aloud), the next
//!   request starts it again, and two failures in a row in one session stop automatic restarts and say
//!   so ([`State::Halted`]) until the person presses Try again. The game never waits on the engine:
//!   nothing here blocks; [`AiClient::poll`] reads what has arrived.
//! - **Reuse obeys the switch** (9.6): the engine is told `reuse {enabled}` after its hello and
//!   whenever the Dev page's "turn off all reuse" switch changes.
//!
//! The client is plain state the caller drives: the app's one instance is [`super::shared`]'s, and
//! each test makes its own.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use humanity_ai_protocol::{decode_engine, encode, AppMessage, Backend, Device, EngineMessage, ErrorCode, ModelKind, ProtocolError, Role, Turn, MAX_LINE_BYTES};

/// The memory cap of an engine holding no model, MB (the job object's limit until a model's load
/// raises it, rung 2). The fake engine and the empty engine use a few MB.
pub const ENGINE_ALONE_CAP_MB: u64 = 256;

/// How the client runs the engine.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Stop the engine after this long with nothing asked of it ("a few minutes idle", 7.2).
    pub idle_stop: Duration,
    /// How long a started engine has to say hello before it counts as a failure.
    pub hello_timeout: Duration,
    /// How long `shutdown` has to end it before it is ended.
    pub stop_grace: Duration,
    /// The job object's memory cap, MB.
    pub memory_cap_mb: u64,
    /// Extra arguments for the engine (`--fake` and its knobs for the Dev page and the tests).
    pub args: Vec<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig { idle_stop: Duration::from_secs(5 * 60), hello_timeout: Duration::from_secs(10), stop_grace: Duration::from_secs(2), memory_cap_mb: ENGINE_ALONE_CAP_MB, args: Vec::new() }
    }
}

/// What the engine said about itself in its hello.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineHello {
    pub version: String,
    pub backends: Vec<Backend>,
    pub cpu_features: Vec<String>,
    pub fake: bool,
}

/// Why an engine stopped that was not asked to.
#[derive(Debug, Clone, PartialEq)]
pub enum ExitWhy {
    /// It ended with this exit code (Windows' code for a process ended from outside is 1).
    Code(i32),
    /// It did not say hello in time.
    NoHello(u64),
    /// It could not be started at all.
    StartFailed(String),
    /// It ended and the system gave no code.
    Unknown,
}

/// The engine's state, as Settings > AI and the Dev page show it.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// No engine to start (none installed, and none named for development).
    NotFound,
    /// Not running: never started, stopped on purpose, or stopped after being idle.
    Stopped,
    Starting { since: Instant },
    Running { since: Instant, hello: EngineHello },
    Stopping { since: Instant },
    /// It stopped unexpectedly once: the next request starts it again.
    Crashed { why: ExitWhy },
    /// It stopped unexpectedly twice in a row: no automatic restarts until Try again.
    Halted { why: ExitWhy },
    /// It spoke another protocol version (or not the protocol at all): never used until Try again.
    Refused { error: ProtocolError },
}

impl State {
    /// The key of the words naming this state, in data/i18n/en.json.
    pub fn words_key(&self) -> &'static str {
        match self {
            State::NotFound => "ai.state.not_found",
            State::Stopped => "ai.state.stopped",
            State::Starting { .. } => "ai.state.starting",
            State::Running { .. } => "ai.state.running",
            State::Stopping { .. } => "ai.state.stopping",
            State::Crashed { .. } => "ai.state.crashed",
            State::Halted { .. } => "ai.state.halted",
            State::Refused { .. } => "ai.state.refused",
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self, State::Starting { .. } | State::Running { .. } | State::Stopping { .. })
    }
}

/// Why a request was answered without the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Fallback {
    NotFound,
    /// Two failures in a row: automatic restarts are off.
    Halted,
    /// The engine stopped while it held the request.
    Crashed,
    /// The engine speaks another protocol version.
    Refused,
    /// The engine refused it, in words.
    Engine(ErrorCode),
    /// It was cancelled.
    Cancelled,
}

impl Fallback {
    /// The key of the words saying why, in data/i18n/en.json.
    pub fn words_key(&self) -> &'static str {
        match self {
            Fallback::NotFound => "ai.state.not_found",
            Fallback::Halted => "ai.state.halted",
            Fallback::Crashed => "ai.fallback.crashed",
            Fallback::Refused => "ai.fallback.refused",
            Fallback::Engine(code) => code.words_key(),
            Fallback::Cancelled => "ai.fallback.cancelled",
        }
    }
}

/// How a request ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Answered { request: u64, text: String, tokens: u32, ms_prefill: u32, ms_generate: u32 },
    Fallback { request: u64, why: Fallback },
}

impl Outcome {
    pub fn request(&self) -> u64 {
        match self {
            Outcome::Answered { request, .. } | Outcome::Fallback { request, .. } => *request,
        }
    }
}

/// The client's running totals for this session (the Dev page's counters).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Counters {
    pub starts: u64,
    pub requests: u64,
    pub tokens: u64,
    pub answered: u64,
    pub fallbacks: u64,
    pub crashes: u64,
}

/// What the reading threads hand the client.
enum Event {
    Line(String),
    /// A line longer than the protocol allows.
    TooLong,
    /// The engine's standard output closed: it ended.
    Closed,
}

/// A running engine.
struct Process {
    child: Child,
    stdin: ChildStdin,
    events: mpsc::Receiver<Event>,
    #[cfg(windows)]
    job: Option<super::job::Job>,
}

/// A request the engine holds.
struct InFlight {
    text: String,
    started: Instant,
}

/// The client.
pub struct AiClient {
    pub config: ClientConfig,
    engine: Option<PathBuf>,
    state: State,
    process: Option<Process>,
    /// Failures in a row this session; an answered request resets it.
    failures: u32,
    /// Messages waiting for the engine's hello.
    queued: Vec<AppMessage>,
    in_flight: BTreeMap<u64, InFlight>,
    outcomes: VecDeque<Outcome>,
    next_request: u64,
    last_activity: Instant,
    /// The reuse switch as last sent to this engine.
    reuse_sent: Option<bool>,
    /// Models the engine said it loaded.
    loaded: BTreeSet<String>,
    /// The engine's last `status` answer.
    pub last_status: Option<EngineMessage>,
    /// The engine's last refusal, with its detail (for the Dev page).
    pub last_error: Option<(ErrorCode, String)>,
    /// The last answered request's timing: tokens, prefill ms, writing ms.
    pub last_timing: Option<(u32, u32, u32)>,
    pub counters: Counters,
}

impl AiClient {
    /// A client for the engine at `engine` (None: there is none, [`State::NotFound`]).
    pub fn new(engine: Option<PathBuf>, config: ClientConfig) -> Self {
        let state = if engine.is_some() { State::Stopped } else { State::NotFound };
        AiClient {
            config,
            engine,
            state,
            process: None,
            failures: 0,
            queued: Vec::new(),
            in_flight: BTreeMap::new(),
            outcomes: VecDeque::new(),
            next_request: 1,
            last_activity: Instant::now(),
            reuse_sent: None,
            loaded: BTreeSet::new(),
            last_status: None,
            last_error: None,
            last_timing: None,
            counters: Counters::default(),
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn engine_path(&self) -> Option<&Path> {
        self.engine.as_deref()
    }

    /// Point the client at another engine (the Dev page's choice); a running one is stopped first.
    pub fn set_engine(&mut self, engine: Option<PathBuf>) {
        if self.engine == engine {
            return;
        }
        self.end_process();
        self.engine = engine;
        self.failures = 0;
        self.state = if self.engine.is_some() { State::Stopped } else { State::NotFound };
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }

    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().map(|p| p.child.id())
    }

    pub fn is_loaded(&self, id: &str) -> bool {
        self.loaded.contains(id)
    }

    /// The text a request has received so far (shown word by word as it arrives).
    pub fn partial(&self, request: u64) -> Option<&str> {
        self.in_flight.get(&request).map(|f| f.text.as_str())
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Start the engine (also the path of Try again, which first clears the failures). Does nothing while
    /// one is live; refuses a relative path and a missing engine.
    pub fn start(&mut self) {
        if self.state.is_live() {
            return;
        }
        let Some(path) = self.engine.clone() else {
            self.state = State::NotFound;
            return;
        };
        if let Err(e) = super::paths::full_path(&path) {
            self.fail(ExitWhy::StartFailed(e));
            return;
        }
        match spawn(&path, &self.config) {
            Ok(process) => {
                log::info!("[AI] engine started: {} (process {})", path.display(), process.child.id());
                self.process = Some(process);
                self.state = State::Starting { since: Instant::now() };
                self.reuse_sent = None;
                self.loaded.clear();
                self.counters.starts += 1;
                self.last_activity = Instant::now();
                self.write(&AppMessage::Hello { app_version: crate::gui::VERSION.to_string() });
            }
            Err(e) => self.fail(ExitWhy::StartFailed(e)),
        }
    }

    /// Try again: clear the failures and the refusal, and start.
    pub fn try_again(&mut self) {
        self.end_process();
        self.failures = 0;
        self.state = State::Stopped;
        self.start();
    }

    /// Ask the engine to finish and exit; ended after [`ClientConfig::stop_grace`] if it does not.
    pub fn stop(&mut self) {
        if matches!(self.state, State::Running { .. } | State::Starting { .. }) {
            self.write(&AppMessage::Shutdown);
            self.state = State::Stopping { since: Instant::now() };
        }
    }

    /// End the engine at once without telling it, the way a crash would (the Dev page's button, to
    /// see the fallback and the restart rules).
    pub fn kill_as_crash(&mut self) {
        if let Some(p) = self.process.as_mut() {
            let _ = p.child.kill();
        }
    }

    /// Ask for the engine's status (the answer lands in [`AiClient::last_status`]).
    pub fn ask_status(&mut self) {
        if matches!(self.state, State::Running { .. }) {
            self.write(&AppMessage::Status);
        }
    }

    /// Load a model the app has checked against the catalog (src/ai/models.rs), with a working memory of
    /// `context` tokens and `threads` threads (9.2), on the processor.
    pub fn load(&mut self, id: &str, path: &str, sha256: &str, kind: ModelKind, context: u32, threads: u32) {
        self.send_when_ready(AppMessage::Load { id: id.into(), path: path.into(), sha256: sha256.into(), kind, context, threads, device: Device::Cpu });
    }

    /// Change the job object's memory cap, MB (9.1): raised to a model's need plus a margin BEFORE its
    /// load, so the load fits; kept for the next start too. Takes effect at once on a running engine.
    pub fn set_memory_cap(&mut self, mb: u64) {
        self.config.memory_cap_mb = mb;
        #[cfg(windows)]
        if let Some(job) = self.process.as_ref().and_then(|p| p.job.as_ref()) {
            if let Err(e) = job.set_limits(mb) {
                log::warn!("[AI] could not change the engine's memory cap to {mb} MB: {e}");
            }
        }
    }

    /// End the engine now and wait for it to be gone (Remove needs its files closed: Windows keeps a
    /// running program's files and a loaded model's file open). Not a failure; every request it held
    /// falls back.
    pub fn shutdown_now(&mut self) {
        let held: Vec<u64> = std::mem::take(&mut self.in_flight).into_keys().collect();
        for r in held {
            self.fall_back(r, Fallback::Cancelled);
        }
        self.queued.clear();
        self.end_process();
        self.state = if self.engine.is_some() { State::Stopped } else { State::NotFound };
    }

    /// Ask a brain for a reply. Returns the request's number; its [`Outcome`] arrives through
    /// [`AiClient::take_outcomes`] (at once, as a fallback, when the engine cannot be used).
    pub fn generate(&mut self, id: &str, persona: &str, question: &str, max_tokens: u32) -> u64 {
        let request = self.next_request;
        self.next_request += 1;
        self.counters.requests += 1;
        let blocked = match &self.state {
            State::NotFound => Some(Fallback::NotFound),
            State::Halted { .. } => Some(Fallback::Halted),
            State::Refused { .. } => Some(Fallback::Refused),
            _ => None,
        };
        if let Some(why) = blocked {
            self.fall_back(request, why);
            return request;
        }
        self.in_flight.insert(request, InFlight { text: String::new(), started: Instant::now() });
        let turns = vec![Turn { role: Role::User, text: question.to_string() }];
        self.send_when_ready(AppMessage::Generate { id: id.into(), request, persona: persona.into(), turns, grammar: None, max_tokens });
        request
    }

    /// Cancel a request (the person walked away). A running engine is told; one not yet running never
    /// sees the request at all, and an engine is never started just to cancel.
    pub fn cancel(&mut self, request: u64) {
        if !self.in_flight.contains_key(&request) {
            return;
        }
        if matches!(self.state, State::Running { .. }) {
            self.write(&AppMessage::Cancel { request });
            return;
        }
        self.queued.retain(|m| !matches!(m, AppMessage::Generate { request: r, .. } if *r == request));
        self.in_flight.remove(&request);
        self.fall_back(request, Fallback::Cancelled);
    }

    /// Every request that ended since the last call.
    pub fn take_outcomes(&mut self) -> Vec<Outcome> {
        self.outcomes.drain(..).collect()
    }

    /// The engine's committed memory now and at its most, MB (Windows; None elsewhere).
    pub fn memory_mb(&self) -> (Option<f64>, Option<f64>) {
        #[cfg(windows)]
        if let Some(p) = &self.process {
            return (super::job::memory_mb(&p.child), p.job.as_ref().and_then(|j| j.peak_mb()));
        }
        (None, None)
    }

    /// Whether the engine sits in its job object, at below-normal priority (the Dev page's lines and
    /// the tests): (in the job, priority class). None when no engine runs or the system has no jobs.
    pub fn containment(&self) -> Option<(bool, u32)> {
        #[cfg(windows)]
        if let Some(p) = &self.process {
            return Some((p.job.as_ref().is_some_and(|j| j.holds(&p.child)), super::job::priority_class(&p.child)));
        }
        None
    }

    /// Close the job object's handle while the engine runs: what the operating system does when the app
    /// exits or crashes. The engine must end (kill on close). For the tests.
    #[cfg(all(test, windows))]
    pub(crate) fn drop_job_for_test(&mut self) {
        if let Some(p) = self.process.as_mut() {
            p.job = None;
        }
    }

    /// For the tests: wait for the engine process to end, up to `within`; whether it did.
    #[cfg(test)]
    pub(crate) fn process_ended_within(&mut self, within: Duration) -> bool {
        let until = Instant::now() + within;
        while Instant::now() < until {
            match self.process.as_mut().map(|p| p.child.try_wait()) {
                None | Some(Ok(Some(_))) => return true,
                _ => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        false
    }

    /// Read everything the engine has said, notice an engine that ended, and keep time: the hello
    /// timeout, the stop grace, the idle stop, and the reuse switch. Never blocks.
    pub fn poll(&mut self) {
        let mut closed = false;
        let mut lines = Vec::new();
        if let Some(p) = &self.process {
            while let Ok(ev) = p.events.try_recv() {
                match ev {
                    Event::Line(l) => lines.push(Ok(l)),
                    Event::TooLong => lines.push(Err(ProtocolError::NotJson(format!("a line over {MAX_LINE_BYTES} bytes")))),
                    Event::Closed => closed = true,
                }
            }
        }
        for l in lines {
            match l.and_then(|l| decode_engine(&l)) {
                Ok(m) => self.on_message(m),
                Err(e) => self.on_unreadable(e),
            }
        }
        if closed {
            self.on_closed();
        }
        let now = Instant::now();
        let (late_hello, late_stop, running) = match &self.state {
            State::Starting { since } => (now.duration_since(*since) > self.config.hello_timeout, false, false),
            State::Stopping { since } => (false, now.duration_since(*since) > self.config.stop_grace, false),
            State::Running { .. } => (false, false, true),
            _ => (false, false, false),
        };
        if late_hello {
            let secs = self.config.hello_timeout.as_secs();
            self.end_process();
            self.fail(ExitWhy::NoHello(secs));
        } else if late_stop {
            self.end_process();
            self.state = State::Stopped;
            self.restart_if_asked();
        } else if running {
            let reuse = humanity_core::reuse::enabled();
            if self.reuse_sent != Some(reuse) {
                self.write(&AppMessage::Reuse { enabled: reuse });
                self.reuse_sent = Some(reuse);
            }
            if self.in_flight.is_empty() && now.duration_since(self.last_activity) > self.config.idle_stop {
                log::info!("[AI] engine idle for {} s: stopping it", self.config.idle_stop.as_secs());
                self.stop();
            }
        }
    }

    fn on_message(&mut self, m: EngineMessage) {
        self.last_activity = Instant::now();
        match m {
            EngineMessage::Hello { engine_version, backends, cpu_features, fake } => {
                if engine_version != super::paths::ENGINE_VERSION {
                    log::info!("[AI] the engine is version {engine_version}; this app expects {}", super::paths::ENGINE_VERSION);
                }
                // Only a starting engine becomes a running one: one asked to stop before its hello
                // stays stopping (its shutdown is already on its way).
                if !matches!(self.state, State::Starting { .. }) {
                    return;
                }
                self.state = State::Running { since: Instant::now(), hello: EngineHello { version: engine_version, backends, cpu_features, fake } };
                for q in std::mem::take(&mut self.queued) {
                    self.write(&q);
                }
            }
            EngineMessage::Loaded { id, .. } => {
                self.loaded.insert(id);
            }
            EngineMessage::Token { request, text } => {
                if let Some(f) = self.in_flight.get_mut(&request) {
                    f.text.push_str(&text);
                    self.counters.tokens += 1;
                }
            }
            EngineMessage::Done { request, tokens, ms_prefill, ms_generate } => {
                if let Some(f) = self.in_flight.remove(&request) {
                    self.failures = 0;
                    self.counters.answered += 1;
                    self.last_timing = Some((tokens, ms_prefill, ms_generate));
                    log::info!("[AI] request {request} answered in {} ms", f.started.elapsed().as_millis());
                    self.outcomes.push_back(Outcome::Answered { request, text: f.text, tokens, ms_prefill, ms_generate });
                }
            }
            EngineMessage::Cancelled { request } => {
                if self.in_flight.remove(&request).is_some() {
                    self.fall_back(request, Fallback::Cancelled);
                }
            }
            EngineMessage::Error { request, code, detail, .. } => {
                log::warn!("[AI] the engine refused: {code:?}: {detail}");
                self.last_error = Some((code, detail));
                if let Some(r) = request {
                    if self.in_flight.remove(&r).is_some() {
                        self.fall_back(r, Fallback::Engine(code));
                    }
                }
            }
            m @ EngineMessage::Status { .. } => self.last_status = Some(m),
            EngineMessage::Ok { .. } | EngineMessage::Pcm { .. } | EngineMessage::Text { .. } => {}
        }
    }

    /// A line the client could not read: another protocol version (or not the protocol at all) means
    /// this engine is not one this app can use, so it is refused and ended, and never guessed at.
    fn on_unreadable(&mut self, e: ProtocolError) {
        log::warn!("[AI] the engine sent a line this app cannot read: {e}");
        if matches!(self.state, State::Refused { .. }) {
            return;
        }
        self.end_process();
        for r in std::mem::take(&mut self.in_flight).into_keys() {
            self.fall_back(r, Fallback::Refused);
        }
        self.queued.clear();
        self.state = State::Refused { error: e };
    }

    /// The engine's output closed: it ended. On purpose (Stopping) it is simply stopped; otherwise it
    /// is a failure.
    fn on_closed(&mut self) {
        let code = self.process.as_mut().and_then(|p| {
            let until = Instant::now() + Duration::from_secs(2);
            loop {
                match p.child.try_wait() {
                    Ok(Some(status)) => return Some(status.code()),
                    Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
                    _ => return None,
                }
            }
        });
        self.end_process();
        match self.state {
            State::Stopping { .. } | State::Refused { .. } => {
                if matches!(self.state, State::Stopping { .. }) {
                    self.state = State::Stopped;
                    self.restart_if_asked();
                }
            }
            _ => {
                let why = match code {
                    Some(Some(c)) => ExitWhy::Code(c),
                    _ => ExitWhy::Unknown,
                };
                self.fail(why);
            }
        }
    }

    /// Something was asked while the engine was stopping: start a fresh one for it now the old one is gone.
    fn restart_if_asked(&mut self) {
        if !self.queued.is_empty() {
            self.start();
        }
    }

    /// Count a failure, fall back on everything held, and choose Crashed or Halted.
    fn fail(&mut self, why: ExitWhy) {
        log::warn!("[AI] the engine stopped unexpectedly: {why:?}");
        self.failures += 1;
        self.counters.crashes += 1;
        for r in std::mem::take(&mut self.in_flight).into_keys() {
            self.fall_back(r, Fallback::Crashed);
        }
        self.queued.clear();
        self.loaded.clear();
        self.state = if self.failures >= 2 { State::Halted { why } } else { State::Crashed { why } };
    }

    fn fall_back(&mut self, request: u64, why: Fallback) {
        self.counters.fallbacks += 1;
        self.outcomes.push_back(Outcome::Fallback { request, why });
    }

    /// Send now if the engine is running, after its hello if it is starting, and start it first if
    /// it is stopped (or crashed once).
    fn send_when_ready(&mut self, m: AppMessage) {
        self.last_activity = Instant::now();
        match self.state {
            State::Running { .. } => self.write(&m),
            State::Starting { .. } => self.queued.push(m),
            State::Stopped | State::Crashed { .. } => {
                self.queued.push(m);
                self.start();
            }
            // Stopping: kept, and the engine starts again once the old one is gone (`restart_if_asked`).
            State::Stopping { .. } => self.queued.push(m),
            // Halted, Refused, NotFound: never started on its own.
            _ => {
                if let AppMessage::Generate { request, .. } = m {
                    self.in_flight.remove(&request);
                    let why = match self.state {
                        State::Halted { .. } => Fallback::Halted,
                        State::Refused { .. } => Fallback::Refused,
                        State::NotFound => Fallback::NotFound,
                        _ => Fallback::Crashed,
                    };
                    self.fall_back(request, why);
                }
            }
        }
    }

    /// Write one message; a broken pipe means the engine is gone, which the reading thread reports.
    fn write(&mut self, m: &AppMessage) {
        if let Some(p) = self.process.as_mut() {
            let line = encode(m);
            if writeln!(p.stdin, "{line}").and_then(|_| p.stdin.flush()).is_err() {
                log::warn!("[AI] could not write to the engine (it has ended)");
            }
        }
    }

    /// End the process (if any) and forget it.
    fn end_process(&mut self) {
        if let Some(mut p) = self.process.take() {
            let _ = p.child.kill();
            let _ = p.child.wait();
        }
        self.loaded.clear();
    }
}

impl Drop for AiClient {
    fn drop(&mut self) {
        self.end_process();
    }
}

/// Start the engine program: full path, pipes, its own folder as the working folder, no console
/// window, below-normal priority, and into a new job object before anything is sent to it.
fn spawn(path: &Path, config: &ClientConfig) -> Result<Process, String> {
    let mut cmd = Command::new(path);
    cmd.args(&config.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(folder) = path.parent() {
        cmd.current_dir(folder);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(super::job::CREATION_FLAGS);
    }
    // The job first, so a failure to make one never leaves an engine running outside it.
    #[cfg(windows)]
    let job = super::job::Job::new(config.memory_cap_mb)?;
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(windows)]
    if let Err(e) = job.assign(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    let stdin = child.stdin.take().ok_or("no standard input")?;
    let stdout = child.stdout.take().ok_or("no standard output")?;
    let stderr = child.stderr.take().ok_or("no standard error")?;
    let (tx, events) = mpsc::channel();
    std::thread::Builder::new()
        .name("ai-engine-out".into())
        .spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut buf = Vec::new();
                match std::io::Read::take(&mut reader, MAX_LINE_BYTES as u64 + 1).read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if buf.len() > MAX_LINE_BYTES && buf.last() != Some(&b'\n') => {
                        // Skip the rest of the over-long line.
                        let mut rest = Vec::new();
                        while let Ok(n) = std::io::Read::take(&mut reader, 64 * 1024).read_until(b'\n', &mut rest) {
                            if n == 0 || rest.last() == Some(&b'\n') {
                                break;
                            }
                            rest.clear();
                        }
                        if tx.send(Event::TooLong).is_err() {
                            return;
                        }
                    }
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                        if !line.is_empty() && tx.send(Event::Line(line)).is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = tx.send(Event::Closed);
        })
        .map_err(|e| e.to_string())?;
    // The engine's notes for a person go into the app's log.
    std::thread::Builder::new()
        .name("ai-engine-err".into())
        .spawn(move || {
            for line in BufReader::new(stderr.take(64 * 1024 * 1024)).lines().map_while(Result::ok) {
                log::info!("[AI engine] {line}");
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(Process {
        child,
        stdin,
        events,
        #[cfg(windows)]
        job: Some(job),
    })
}
