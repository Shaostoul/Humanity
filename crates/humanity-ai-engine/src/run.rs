//! The engine's loop: read a message a line, answer a message a line, until `shutdown` or the end of
//! its standard input (the app is gone).
//!
//! Reading happens on a thread of its own, so a long reply can notice a `cancel` (or answer a
//! `status`) between its pieces; every other message that arrives meanwhile waits its turn.

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use humanity_ai_protocol::{decode_app, encode_versioned, AppMessage, Backend, Counters, EngineMessage, ErrorCode, LoadedModel, ProtocolError, MAX_LINE_BYTES};

use crate::args::Options;
use crate::fake;

/// What the reading thread hands the loop.
#[derive(Debug)]
pub(crate) enum Incoming {
    Line(String),
    /// A line longer than [`MAX_LINE_BYTES`], skipped to its end.
    TooLong,
    /// The app closed the engine's standard input.
    End,
}

/// Read one line of at most `limit` bytes. Ok(None) at the end of the input.
fn read_bounded<R: BufRead>(input: &mut R, limit: usize) -> std::io::Result<Option<Incoming>> {
    let mut buf = Vec::new();
    let n = std::io::Read::take(&mut *input, limit as u64 + 1).read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > limit && buf.last() != Some(&b'\n') {
        // Skip the rest of the over-long line.
        let mut rest = Vec::new();
        loop {
            rest.clear();
            let m = std::io::Read::take(&mut *input, 64 * 1024).read_until(b'\n', &mut rest)?;
            if m == 0 || rest.last() == Some(&b'\n') {
                break;
            }
        }
        return Ok(Some(Incoming::TooLong));
    }
    Ok(Some(Incoming::Line(String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string())))
}

fn spawn_reader<R: BufRead + Send + 'static>(mut input: R) -> mpsc::Receiver<Incoming> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        match read_bounded(&mut input, MAX_LINE_BYTES) {
            Ok(Some(i)) => {
                if tx.send(i).is_err() {
                    return;
                }
            }
            Ok(None) | Err(_) => {
                let _ = tx.send(Incoming::End);
                return;
            }
        }
    });
    rx
}

/// The engine's state between messages.
pub(crate) struct Engine<W: Write> {
    pub(crate) opts: Options,
    out: W,
    /// The reuse switch as the app last sent it (9.6); on until told otherwise.
    pub(crate) reuse: bool,
    /// The persona read last, kept only while reuse is on (the fake stand-in for llama.cpp's prompt
    /// reuse: "Re-use KV cache from a previous request if possible").
    pub(crate) last_persona: Option<String>,
    /// Models loaded (the fake's: its reserved memory with each).
    pub(crate) loaded: Vec<(LoadedModel, Vec<u8>)>,
    /// llama.cpp, started at the first `hello` or `load` (never before the app's first message, so
    /// inside the job object), and the outcome kept (rung 2).
    #[cfg(feature = "llama")]
    pub(crate) runtime: Option<Result<crate::llama::Runtime, String>>,
    /// The real models loaded, each with its working memory.
    #[cfg(feature = "llama")]
    pub(crate) brains: Vec<(LoadedModel, crate::llama::Brain)>,
    /// The model writing a reply now (out of `brains` while it works) and its threads, so a `status`
    /// asked between its pieces still lists it.
    #[cfg(feature = "llama")]
    pub(crate) working: Option<(LoadedModel, u32)>,
    pub(crate) counters: Counters,
    /// Messages that arrived while a reply was being made.
    pub(crate) pending: VecDeque<Incoming>,
    /// The reading thread has handed in the end of the input; a reply in hand finishes first.
    pub(crate) input_ended: bool,
}

/// Whether the loop goes on.
#[derive(Debug, PartialEq)]
pub(crate) enum Flow {
    Continue,
    Exit,
}

impl<W: Write> Engine<W> {
    pub(crate) fn new(opts: Options, out: W) -> Self {
        Engine {
            opts,
            out,
            reuse: true,
            last_persona: None,
            loaded: Vec::new(),
            #[cfg(feature = "llama")]
            runtime: None,
            #[cfg(feature = "llama")]
            brains: Vec::new(),
            #[cfg(feature = "llama")]
            working: None,
            counters: Counters::default(),
            pending: VecDeque::new(),
            input_ended: false,
        }
    }

    /// The backends this engine can run models on: none for the fake (it runs no model), the processor
    /// when llama.cpp is built in and started (rung 2), none otherwise.
    fn backends(&mut self) -> Vec<Backend> {
        if self.opts.fake {
            return Vec::new();
        }
        #[cfg(feature = "llama")]
        if crate::real::runtime(self).is_ok() {
            return vec![Backend::Cpu];
        }
        Vec::new()
    }

    /// Write one message. A closed output means the app is gone; the next read sees the end.
    pub(crate) fn send(&mut self, m: &EngineMessage) {
        let line = encode_versioned(m, self.opts.protocol);
        let _ = writeln!(self.out, "{line}");
        let _ = self.out.flush();
    }

    pub(crate) fn refuse(&mut self, request: Option<u64>, code: ErrorCode, detail: String) {
        self.counters.errors += 1;
        self.send(&EngineMessage::Error { request, code, words_key: code.words_key().to_string(), detail });
    }

    fn refuse_line(&mut self, e: ProtocolError) {
        let code = e.code();
        self.refuse(None, code, e.to_string());
    }

    /// The processor features a runtime would use, for the app's notice (4.7).
    fn cpu_features() -> Vec<String> {
        let mut out = Vec::new();
        #[cfg(target_arch = "x86_64")]
        {
            if std::arch::is_x86_feature_detected!("avx2") {
                out.push("avx2".to_string());
            }
            if std::arch::is_x86_feature_detected!("fma") {
                out.push("fma".to_string());
            }
            if std::arch::is_x86_feature_detected!("f16c") {
                out.push("f16c".to_string());
            }
            if std::arch::is_x86_feature_detected!("avx512f") {
                out.push("avx512f".to_string());
            }
        }
        #[cfg(target_arch = "aarch64")]
        out.push("neon".to_string());
        out
    }

    /// Answer one message.
    pub(crate) fn handle(&mut self, msg: AppMessage, rx: &mpsc::Receiver<Incoming>) -> Flow {
        if self.opts.exit_on.as_deref() == Some(msg.kind()) {
            // The fake's crash, for the client's crash handling (`--fake-exit-on`).
            eprintln!("humanity-ai: exiting on {} as --fake-exit-on asked", msg.kind());
            std::process::exit(3);
        }
        match msg {
            AppMessage::Hello { app_version } => {
                eprintln!("humanity-ai: hello from HumanityOS {app_version}");
                let backends = self.backends();
                let m = EngineMessage::Hello { engine_version: crate::ENGINE_VERSION.to_string(), backends, cpu_features: Self::cpu_features(), fake: self.opts.fake };
                self.send(&m);
            }
            #[allow(unused_variables)]
            AppMessage::Load { id, path, kind, context, threads, device, .. } => {
                if self.opts.fake {
                    fake::load(self, id, kind);
                } else {
                    #[cfg(feature = "llama")]
                    crate::real::load(self, id, path, kind, context, threads, device);
                    #[cfg(not(feature = "llama"))]
                    self.refuse(None, ErrorCode::NoBackend, format!("this engine has no runtime for {id}"));
                }
            }
            #[allow(unused_variables)]
            AppMessage::Generate { id, request, persona, turns, grammar, max_tokens } => {
                self.counters.requests += 1;
                if !self.opts.fake {
                    #[cfg(feature = "llama")]
                    return crate::real::generate(self, rx, &id, request, &persona, &turns, grammar.as_deref(), max_tokens);
                    #[cfg(not(feature = "llama"))]
                    self.refuse(Some(request), ErrorCode::NoBackend, "this engine has no brain runtime".into());
                } else if !self.loaded.iter().any(|(m, _)| m.id == id) {
                    self.refuse(Some(request), ErrorCode::NotLoaded, format!("{id} is not loaded"));
                } else {
                    return fake::generate(self, rx, request, persona, turns.len(), grammar.as_deref(), max_tokens);
                }
            }
            AppMessage::Speak { request, text, .. } => {
                self.counters.requests += 1;
                if !self.opts.fake {
                    self.refuse(Some(request), ErrorCode::NoBackend, "this engine has no voice runtime".into());
                } else {
                    return fake::speak(self, rx, request, &text);
                }
            }
            AppMessage::ReadImage { request, .. } => {
                self.counters.requests += 1;
                if !self.opts.fake {
                    self.refuse(Some(request), ErrorCode::NoBackend, "this engine has no image runtime".into());
                } else {
                    self.send(&EngineMessage::Text { request, lines: fake::LABEL_LINES.iter().map(|s| s.to_string()).collect(), confidence: 1.0 });
                }
            }
            // Nothing is being made when a cancel is read here (one being made reads its own), so it
            // is already done: say so, so the app never waits.
            AppMessage::Cancel { request } => self.send(&EngineMessage::Cancelled { request }),
            AppMessage::Reuse { enabled } => {
                self.reuse = enabled;
                if !enabled {
                    self.last_persona = None;
                    // Nothing read for one request is kept for the next (9.6).
                    #[cfg(feature = "llama")]
                    for (_, brain) in self.brains.iter_mut() {
                        brain.forget();
                    }
                }
                self.send(&EngineMessage::Ok { to: "reuse".into() });
            }
            AppMessage::Status => self.send_status(),
            AppMessage::Unload { id } => {
                let before = self.loaded.len();
                self.loaded.retain(|(m, _)| m.id != id);
                #[cfg(feature = "llama")]
                let before = before + self.brains.len();
                #[cfg(feature = "llama")]
                self.brains.retain(|(m, _)| m.id != id);
                #[cfg(feature = "llama")]
                let gone = before != self.loaded.len() + self.brains.len();
                #[cfg(not(feature = "llama"))]
                let gone = before != self.loaded.len();
                if !gone {
                    self.refuse(None, ErrorCode::NotLoaded, format!("{id} is not loaded"));
                } else {
                    self.send(&EngineMessage::Ok { to: "unload".into() });
                }
            }
            AppMessage::Shutdown => {
                self.send(&EngineMessage::Ok { to: "shutdown".into() });
                return Flow::Exit;
            }
        }
        Flow::Continue
    }

    /// `status`: the engine's committed memory read from the system now (what the job object's cap
    /// counts, src/memory.rs), the threads its models use, what it holds, its counters.
    pub(crate) fn send_status(&mut self) {
        #[allow(unused_mut)]
        let mut loaded: Vec<LoadedModel> = self.loaded.iter().map(|(m, _)| m.clone()).collect();
        #[allow(unused_mut)]
        let mut threads = 1;
        #[cfg(feature = "llama")]
        {
            for (m, brain) in &self.brains {
                loaded.push(m.clone());
                threads = threads.max(brain.threads);
            }
            if let Some((m, t)) = &self.working {
                loaded.push(m.clone());
                threads = threads.max(*t);
            }
        }
        let m = EngineMessage::Status {
            memory_mb: crate::memory::usage().map(|u| u.committed_mb()),
            threads,
            loaded,
            counters: self.counters.clone(),
            reuse: self.reuse,
            fake: self.opts.fake,
        };
        self.send(&m);
    }

    /// The loop over what the reading thread hands in, until `shutdown` or the end of the input.
    pub(crate) fn serve(&mut self, rx: &mpsc::Receiver<Incoming>) -> i32 {
        loop {
            let next = match self.pending.pop_front() {
                Some(i) => i,
                None => rx.recv().unwrap_or(Incoming::End),
            };
            match next {
                Incoming::End => return 0,
                Incoming::TooLong => self.refuse_line(ProtocolError::NotJson(format!("a line over the {MAX_LINE_BYTES}-byte limit"))),
                Incoming::Line(l) if l.trim().is_empty() => {}
                Incoming::Line(l) => match decode_app(&l) {
                    Err(e) => self.refuse_line(e),
                    Ok(msg) => {
                        if self.handle(msg, rx) == Flow::Exit {
                            return 0;
                        }
                    }
                },
            }
        }
    }
}

/// Wait `ms` (0: just look) while a reply is being made, answering a `cancel` of `request` (true: stop
/// the reply) or a `status` meanwhile; everything else waits in `pending`, the end of the input too:
/// the reply in hand finishes (its words go nowhere if the app is gone, and the job object ends the
/// engine with the app anyway), then the loop reads the end. The fake waits its scripted time here;
/// llama.cpp's replies look between pieces with `ms` 0 (rung 2).
pub(crate) fn pause<W: Write>(e: &mut Engine<W>, rx: &mpsc::Receiver<Incoming>, request: u64, ms: u64) -> bool {
    let until = Instant::now() + Duration::from_millis(ms);
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if e.input_ended {
            std::thread::sleep(left);
            return false;
        }
        match rx.recv_timeout(left) {
            Ok(Incoming::Line(l)) => match decode_app(&l) {
                Ok(AppMessage::Cancel { request: r }) if r == request => return true,
                Ok(AppMessage::Status) => e.send_status(),
                _ => e.pending.push_back(Incoming::Line(l)),
            },
            Ok(Incoming::End) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                e.input_ended = true;
                e.pending.push_back(Incoming::End);
            }
            Ok(other) => e.pending.push_back(other),
            Err(mpsc::RecvTimeoutError::Timeout) => return false,
        }
    }
}

/// Run the engine over `input` and `output` (the process's own standard input and output in
/// main.rs; in-memory ones in the tests). Returns the exit code.
pub fn run<R: BufRead + Send + 'static, W: Write>(opts: &Options, input: R, output: W) -> i32 {
    let rx = spawn_reader(input);
    Engine::new(opts.clone(), output).serve(&rx)
}
