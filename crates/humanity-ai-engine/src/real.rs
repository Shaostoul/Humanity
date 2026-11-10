//! The engine with a runtime: `load`, `generate`, `unload` on llama.cpp (rung 2, 2026-11-09). Built
//! only with the `llama` feature; run.rs sends model requests here when the engine is not `--fake`.
//!
//! The protocol is the fake's to the letter (the app cannot tell them apart by the messages): a reply
//! comes as `token` pieces, each whole characters ([`crate::text::Utf8Stream`]), then `done` with the
//! prefill and writing times; a `cancel` read between pieces stops it and says `cancelled`; a `status`
//! read between pieces is answered at once.

use std::sync::mpsc;
use std::time::Instant;

use humanity_ai_protocol::{Device, EngineMessage, ErrorCode, LoadedModel, ModelKind, Turn};

use crate::llama::{self, Brain, LoadError, Sampling};
use crate::run::{pause, Engine, Flow, Incoming};
use crate::text::Utf8Stream;

/// Start the runtime once (after the app's first message, so inside the job object); its outcome is
/// kept for the engine's life. The `hello` lists the CPU backend only when this worked.
pub(crate) fn runtime<W: std::io::Write>(e: &mut Engine<W>) -> Result<&llama::Runtime, String> {
    if e.runtime.is_none() {
        let folder = crate::folder::own_folder().ok_or_else(|| "the engine cannot tell where its own folder is".to_string());
        let verbose = e.opts.llama_log;
        let started = folder.and_then(|f| llama::init(&f, verbose));
        match &started {
            Ok(r) => eprintln!("humanity-ai: llama.cpp started on {} ({})", r.devices.join(", "), r.system_info.trim()),
            Err(why) => eprintln!("humanity-ai: llama.cpp could not start: {why}"),
        }
        e.runtime = Some(started);
    }
    e.runtime.as_ref().expect("set above").as_ref().map_err(|s| s.clone())
}

/// `load`: a brain on the processor. A voice or image model, or the graphics card, is refused in words
/// (rung 2 has only llama.cpp, CPU only).
#[allow(clippy::too_many_arguments)]
pub(crate) fn load<W: std::io::Write>(e: &mut Engine<W>, id: String, path: String, kind: ModelKind, context: u32, threads: u32, device: Device) {
    match kind {
        ModelKind::Brain => {}
        ModelKind::Voice => return e.refuse(None, ErrorCode::NoBackend, "this engine has no voice runtime yet (rung 5)".into()),
        ModelKind::ImageReader => return e.refuse(None, ErrorCode::NoBackend, "this engine has no image runtime yet (rung 8)".into()),
    }
    if device == Device::Vulkan {
        return e.refuse(None, ErrorCode::NoBackend, "this engine was built for the processor only (rung 2)".into());
    }
    if let Err(why) = runtime(e) {
        return e.refuse(None, ErrorCode::NoBackend, why);
    }
    // A model loaded again under the same id replaces the old one, whose memory goes first.
    e.brains.retain(|(m, _)| m.id != id);
    let before = crate::memory::usage().map(|u| u.committed).unwrap_or(0);
    let started = Instant::now();
    let context = context.clamp(256, 32_768);
    match Brain::load(&path, context, threads) {
        Ok(brain) => {
            let after = crate::memory::usage().map(|u| u.committed).unwrap_or(before);
            let memory_mb = after.saturating_sub(before) / (1024 * 1024);
            eprintln!(
                "humanity-ai: loaded {id} ({}, {} parameters, {} MB of weights) in {} ms, {memory_mb} MB committed, {} threads, {} tokens of working memory",
                brain.desc,
                brain.params,
                brain.weights_bytes / (1024 * 1024),
                started.elapsed().as_millis(),
                brain.threads,
                brain.n_ctx
            );
            e.brains.push((LoadedModel { id: id.clone(), kind, memory_mb }, brain));
            e.send(&EngineMessage::Loaded { id, memory_mb });
        }
        Err(LoadError::File(why)) => e.refuse(None, ErrorCode::LoadFailed, why),
        Err(LoadError::Memory(why)) => e.refuse(None, ErrorCode::OutOfMemory, why),
    }
}

/// `generate`: the persona and turns through the model's chat template, the prompt read (reused when
/// the switch allows), then the reply piece by piece.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate<W: std::io::Write>(e: &mut Engine<W>, rx: &mpsc::Receiver<Incoming>, id: &str, request: u64, persona: &str, turns: &[Turn], grammar: Option<&str>, max_tokens: u32) -> Flow {
    let Some(at) = e.brains.iter().position(|(m, _)| m.id == id) else {
        e.refuse(Some(request), ErrorCode::NotLoaded, format!("{id} is not loaded"));
        return Flow::Continue;
    };
    // Out of the list while it works, so the loop can answer a status between pieces.
    let (info, mut brain) = e.brains.remove(at);
    e.working = Some((info.clone(), brain.threads));
    let flow = reply(e, rx, &mut brain, request, persona, turns, grammar, max_tokens);
    e.working = None;
    if !e.reuse {
        brain.forget();
    }
    e.brains.insert(at.min(e.brains.len()), (info, brain));
    flow
}

#[allow(clippy::too_many_arguments)]
fn reply<W: std::io::Write>(e: &mut Engine<W>, rx: &mpsc::Receiver<Incoming>, brain: &mut Brain, request: u64, persona: &str, turns: &[Turn], grammar: Option<&str>, max_tokens: u32) -> Flow {
    let start = Instant::now();
    let tokens = match brain.prompt(persona, turns).and_then(|p| brain.tokenize(&p)) {
        Ok(t) => t,
        Err(why) => {
            e.refuse(Some(request), ErrorCode::Internal, why);
            return Flow::Continue;
        }
    };
    let mut sampler = match brain.sampler(Sampling { seed: seed(), ..Sampling::default() }, grammar) {
        Ok(s) => s,
        Err(why) => {
            e.refuse(Some(request), ErrorCode::Internal, why);
            return Flow::Continue;
        }
    };
    let reuse = e.reuse;
    let mut cancelled = false;
    let read = brain.prefill(&tokens, reuse, || {
        cancelled = pause(e, rx, request, 0);
        cancelled
    });
    match read {
        Err(why) => {
            e.refuse(Some(request), ErrorCode::Internal, why);
            return Flow::Continue;
        }
        Ok(None) => return cancel(e, request),
        Ok(Some(reused)) => {
            if reused > 0 {
                eprintln!("humanity-ai: request {request}: {reused} of {} prompt tokens reused", tokens.len());
            }
        }
    }
    let ms_prefill = start.elapsed().as_millis() as u32;
    let writing = Instant::now();
    let mut text = Utf8Stream::default();
    let mut n: u32 = 0;
    while n < max_tokens.max(1) {
        // What was read stays in the working memory: the next request reuses the shared start of it.
        if pause(e, rx, request, 0) {
            return cancel(e, request);
        }
        match brain.next(&mut sampler) {
            Err(why) => {
                e.refuse(Some(request), ErrorCode::Internal, why);
                return Flow::Continue;
            }
            Ok(None) => break,
            Ok(Some(bytes)) => {
                n += 1;
                e.counters.tokens += 1;
                let piece = text.push(&bytes);
                if !piece.is_empty() {
                    e.send(&EngineMessage::Token { request, text: piece });
                }
            }
        }
    }
    let rest = text.finish();
    if !rest.is_empty() {
        e.send(&EngineMessage::Token { request, text: rest });
    }
    let ms_generate = writing.elapsed().as_millis() as u32;
    e.send(&EngineMessage::Done { request, tokens: n, ms_prefill, ms_generate });
    Flow::Continue
}

fn cancel<W: std::io::Write>(e: &mut Engine<W>, request: u64) -> Flow {
    e.counters.cancelled += 1;
    e.send(&EngineMessage::Cancelled { request });
    Flow::Continue
}

/// A different draw for every reply (the time), so a character does not say the same thing twice.
fn seed() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() ^ (d.as_secs() as u32)).unwrap_or(0x4855_4d41)
}
