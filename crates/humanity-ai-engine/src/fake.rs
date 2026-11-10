//! `--fake`: scripted answers and timing in place of a model (docs/design/embeddable-ai.md, 12.1: "so
//! every screen, fallback and notice is tested in CI and rigs without any model").
//!
//! It plays each part a real runtime will, closely enough that the app cannot tell the difference
//! from the messages alone:
//! - **a reply** comes word by word, after a prefill pause, and a `cancel` stops it between words;
//! - **prompt reuse** (9.6): the same persona a second time, with reuse on, takes a tenth of the
//!   prefill; with reuse off every request reads it whole, so the Dev page shows the switch's effect;
//! - **memory**: `--fake-load-mb` makes each load reserve and touch that much, kept while loaded, so a
//!   load past the job object's cap fails the way a real model's would (`out_of_memory`);
//! - **speech** is silence, in 100 ms chunks of 24 kHz mono 16-bit, faster than real time;
//! - **a label photo** reads as two fixed lines.
//!
//! The lines are test output, like a model's, and never shown to a person as the app's own words.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use humanity_ai_protocol::{EngineMessage, ErrorCode, LoadedModel, ModelKind};

use crate::run::{Engine, Flow, Incoming};

/// The scripted replies, one per turn count, round and round.
pub const LINES: &[&str] = &[
    "This is the fake AI engine. It answers with lines written for testing, so every screen can be tried without a model.",
    "A real engine would answer in the character's own voice. This one only proves the plumbing works.",
    "Nothing typed here leaves this computer, and no model is installed.",
];

/// What the fake label reader reads.
pub const LABEL_LINES: &[&str] = &["FAKE LABEL", "INGREDIENTS: WATER"];

/// The fake's speech: samples a second (24 kHz mono, 16-bit), sent in 100 ms chunks.
const SAMPLE_RATE: usize = 24_000;
const CHUNK_SAMPLES: usize = SAMPLE_RATE / 10;

/// A load: reserve and touch `--fake-load-mb`, or refuse with out_of_memory when it cannot be had.
pub(crate) fn load<W: std::io::Write>(e: &mut Engine<W>, id: String, kind: ModelKind) {
    let mb = e.opts.load_mb;
    let mut block: Vec<u8> = Vec::new();
    if mb > 0 {
        let bytes = (mb as usize).saturating_mul(1024 * 1024);
        if let Err(err) = block.try_reserve_exact(bytes) {
            e.refuse(None, ErrorCode::OutOfMemory, format!("could not reserve {mb} MB for {id}: {err}"));
            return;
        }
        // Touch every page, so the memory is really committed, not just promised.
        block.resize(bytes, 0);
        for i in (0..bytes).step_by(4096) {
            block[i] = 1;
        }
    }
    e.loaded.retain(|(m, _)| m.id != id);
    e.loaded.push((LoadedModel { id: id.clone(), kind, memory_mb: mb }, block));
    e.send(&EngineMessage::Loaded { id, memory_mb: mb });
}

/// Sleep `ms`, answering a `cancel` of `request` (true: stop the reply) or a `status` meanwhile; everything else
/// waits in `pending`, the end of the input too: the reply in hand finishes (its words go nowhere if the
/// app is gone, and the job object ends the engine with the app anyway), then the loop reads the end.
fn pause<W: std::io::Write>(e: &mut Engine<W>, rx: &mpsc::Receiver<Incoming>, request: u64, ms: u64) -> bool {
    let until = Instant::now() + Duration::from_millis(ms);
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if e.input_ended {
            std::thread::sleep(left);
            return false;
        }
        match rx.recv_timeout(left) {
            Ok(Incoming::Line(l)) => match humanity_ai_protocol::decode_app(&l) {
                Ok(humanity_ai_protocol::AppMessage::Cancel { request: r }) if r == request => return true,
                Ok(humanity_ai_protocol::AppMessage::Status) => e.send_status(),
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

fn cancelled<W: std::io::Write>(e: &mut Engine<W>, request: u64) -> Flow {
    e.counters.cancelled += 1;
    e.send(&EngineMessage::Cancelled { request });
    Flow::Continue
}

/// A reply: the prefill pause (a tenth of it for a persona reused), then the scripted line word by word.
pub(crate) fn generate<W: std::io::Write>(e: &mut Engine<W>, rx: &mpsc::Receiver<Incoming>, request: u64, persona: String, turns: usize, max_tokens: u32) -> Flow {
    let start = Instant::now();
    let reused = e.reuse && e.last_persona.as_deref() == Some(persona.as_str());
    let prefill = if reused { e.opts.ms_prefill / 10 } else { e.opts.ms_prefill };
    if pause(e, rx, request, prefill) {
        return cancelled(e, request);
    }
    e.last_persona = e.reuse.then_some(persona);
    let ms_prefill = start.elapsed().as_millis() as u32;
    let writing = Instant::now();
    let line = LINES[turns % LINES.len()];
    let words: Vec<&str> = line.split(' ').collect();
    let n = words.len().min(max_tokens.max(1) as usize);
    for (i, w) in words.iter().take(n).enumerate() {
        if i > 0 {
            if pause(e, rx, request, e.opts.ms_per_token) {
                return cancelled(e, request);
            }
        }
        let text = if i + 1 < n { format!("{w} ") } else { w.to_string() };
        e.counters.tokens += 1;
        e.send(&EngineMessage::Token { request, text });
    }
    let ms_generate = writing.elapsed().as_millis() as u32;
    e.send(&EngineMessage::Done { request, tokens: n as u32, ms_prefill, ms_generate });
    Flow::Continue
}

/// Speech: 100 ms of silence for every 15 characters, each chunk sent after a tenth of its own length.
pub(crate) fn speak<W: std::io::Write>(e: &mut Engine<W>, rx: &mpsc::Receiver<Incoming>, request: u64, text: &str) -> Flow {
    let start = Instant::now();
    let chunks = text.chars().count().div_ceil(15).max(1);
    let silence = encode_base64(&vec![0u8; CHUNK_SAMPLES * 2]);
    for _ in 0..chunks {
        if pause(e, rx, request, 10) {
            return cancelled(e, request);
        }
        e.send(&EngineMessage::Pcm { request, samples: silence.clone() });
    }
    e.send(&EngineMessage::Done { request, tokens: 0, ms_prefill: 0, ms_generate: start.elapsed().as_millis() as u32 });
    Flow::Continue
}

/// Standard base64 (RFC 4648, with padding): the engine's one use of it, so no crate for it.
pub fn encode_base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        out.push(ABC[(n >> 18) as usize & 63] as char);
        out.push(ABC[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { ABC[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { ABC[n as usize & 63] as char } else { '=' });
    }
    out
}
