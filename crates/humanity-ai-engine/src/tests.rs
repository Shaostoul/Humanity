//! The engine's own tests, run by `just ai-engine test` (it is not a member of the app's workspace, so
//! `just verify` does not build it; the app's client tests in src/ai/ drive the real program too).
//! Each drives `run` over an in-memory input and reads the lines it wrote.

use std::io::Cursor;

use humanity_ai_protocol::{decode_engine, encode, encode_versioned, AppMessage, Device, EngineMessage, ErrorCode, ModelKind, Turn, Role, PROTOCOL};

use crate::args::{parse, Command, Options};
use crate::folder::library_path;
use crate::run;

fn fake() -> Options {
    Options { fake: true, ms_prefill: 0, ms_per_token: 0, ..Options::default() }
}

/// Run the engine over these messages (and raw lines) and read back what it wrote.
fn talk(opts: &Options, lines: &[String]) -> (i32, Vec<EngineMessage>) {
    let input = lines.iter().map(|l| format!("{l}\n")).collect::<String>();
    let mut out = Vec::new();
    let code = run(opts, Cursor::new(input.into_bytes()), &mut out);
    let text = String::from_utf8(out).expect("utf-8");
    (code, text.lines().map(|l| decode_engine(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect())
}

fn m(msg: AppMessage) -> String {
    encode(&msg)
}

fn load(id: &str) -> String {
    m(AppMessage::Load { id: id.into(), path: "C:\\x.gguf".into(), sha256: "00".into(), kind: ModelKind::Brain, context: 4096, threads: 2, device: Device::Cpu })
}

fn generate(request: u64, persona: &str) -> String {
    m(AppMessage::Generate { id: "fake".into(), request, persona: persona.into(), turns: vec![Turn { role: Role::User, text: "Hello?".into() }], grammar: None, max_tokens: 64 })
}

/// The fake says hello as the fake, loads, and answers a reply word by word, then its timing; status
/// counts it; shutdown ends the loop with code 0, and nothing after it is answered.
#[test]
fn the_fake_says_hello_loads_and_answers_word_by_word() {
    let (code, out) = talk(&fake(), &[m(AppMessage::Hello { app_version: "test".into() }), load("fake"), generate(1, "p"), m(AppMessage::Status), m(AppMessage::Shutdown), m(AppMessage::Status)]);
    assert_eq!(code, 0);
    assert!(matches!(&out[0], EngineMessage::Hello { fake: true, backends, engine_version, .. } if backends.is_empty() && engine_version == crate::ENGINE_VERSION), "{:?}", out[0]);
    assert_eq!(out[1], EngineMessage::Loaded { id: "fake".into(), memory_mb: 0 });
    let words: String = out.iter().filter_map(|x| if let EngineMessage::Token { request: 1, text } = x { Some(text.as_str()) } else { None }).collect();
    assert_eq!(words, crate::fake::LINES[1], "turn 1's line, whole");
    let done = out.iter().find_map(|x| if let EngineMessage::Done { request: 1, tokens, .. } = x { Some(*tokens) } else { None }).expect("a done");
    assert_eq!(done as usize, crate::fake::LINES[1].split(' ').count());
    // A status asked while a reply is being made is answered at once, between its words (the Dev page asks
    // while a reply streams), so it may come before the done.
    assert!(out.iter().any(|x| matches!(x, EngineMessage::Status { counters, fake: true, .. } if counters.requests == 1)), "{out:?}");
    assert_eq!(out.last(), Some(&EngineMessage::Ok { to: "shutdown".into() }), "nothing after shutdown");
}

/// With reuse on, the same persona a second time is read in a tenth of the time; with reuse off every
/// request reads it whole (9.6, the switch's effect the Dev page shows). Seen red 2026-11-09 with the
/// reuse check left out of `fake::generate`: "reuse on: the second persona is reused: [120, 120]".
#[test]
fn the_reuse_switch_changes_the_prefill() {
    let opts = Options { ms_prefill: 120, ..fake() };
    let prefills = |reuse: bool| -> Vec<u32> {
        let (_, out) = talk(&opts, &[m(AppMessage::Reuse { enabled: reuse }), load("fake"), generate(1, "same persona"), generate(2, "same persona")]);
        out.iter().filter_map(|x| if let EngineMessage::Done { ms_prefill, .. } = x { Some(*ms_prefill) } else { None }).collect()
    };
    let on = prefills(true);
    assert!(on[0] >= 120 && on[1] < 60, "reuse on: the second persona is reused: {on:?}");
    let off = prefills(false);
    assert!(off[0] >= 120 && off[1] >= 120, "reuse off: every persona read whole: {off:?}");
}

/// A cancel that arrives while a reply is being made stops it between words and says so; nothing
/// after the cancel belongs to that request.
#[test]
fn a_cancel_stops_a_reply_between_words() {
    let opts = Options { ms_per_token: 30, ..fake() };
    let (_, out) = talk(&opts, &[load("fake"), generate(5, "p"), m(AppMessage::Cancel { request: 5 }), m(AppMessage::Status)]);
    let tokens = out.iter().filter(|x| matches!(x, EngineMessage::Token { request: 5, .. })).count();
    assert!(tokens < crate::fake::LINES[1].split(' ').count(), "stopped early: {tokens} pieces");
    assert!(out.contains(&EngineMessage::Cancelled { request: 5 }));
    assert!(!out.iter().any(|x| matches!(x, EngineMessage::Done { request: 5, .. })), "no done for a cancelled reply");
    assert!(matches!(out.last(), Some(EngineMessage::Status { counters, .. }) if counters.cancelled == 1));
}

/// Without --fake, rung 1's engine has no runtime: hello with no backends, and every model request
/// refused with its words key, never a crash and never a silence.
#[test]
fn without_fake_every_model_request_is_refused_in_words() {
    let (_, out) = talk(&Options::default(), &[m(AppMessage::Hello { app_version: "t".into() }), load("x"), generate(2, "p"), m(AppMessage::Speak { voice: "v".into(), text: "hi".into(), request: 3 })]);
    assert!(matches!(&out[0], EngineMessage::Hello { fake: false, backends, .. } if backends.is_empty()));
    for x in &out[1..] {
        assert!(matches!(x, EngineMessage::Error { code: ErrorCode::NoBackend, words_key, .. } if words_key == "ai.error.no_backend"), "{x:?}");
    }
    assert_eq!(out.len(), 4);
}

/// A line of another protocol version is refused as a version mismatch, a broken line as a bad
/// message, and the engine goes on answering after both.
#[test]
fn a_line_it_cannot_read_is_refused_and_the_engine_goes_on() {
    let other = encode_versioned(&AppMessage::Status, PROTOCOL + 1);
    let (_, out) = talk(&fake(), &[other, "not json".into(), r#"{"v":1,"type":"dance"}"#.into(), m(AppMessage::Status)]);
    assert!(matches!(&out[0], EngineMessage::Error { code: ErrorCode::Version, .. }), "{:?}", out[0]);
    assert!(matches!(&out[1], EngineMessage::Error { code: ErrorCode::BadMessage, .. }));
    assert!(matches!(&out[2], EngineMessage::Error { code: ErrorCode::BadMessage, .. }));
    assert!(matches!(&out[3], EngineMessage::Status { counters, .. } if counters.errors == 3));
}

/// A reply to a model not loaded is refused; unloading one not loaded too.
#[test]
fn a_model_not_loaded_is_refused() {
    let (_, out) = talk(&fake(), &[generate(1, "p"), m(AppMessage::Unload { id: "fake".into() }), load("fake"), m(AppMessage::Unload { id: "fake".into() })]);
    assert!(matches!(&out[0], EngineMessage::Error { code: ErrorCode::NotLoaded, request: Some(1), .. }));
    assert!(matches!(&out[1], EngineMessage::Error { code: ErrorCode::NotLoaded, .. }));
    assert_eq!(out[3], EngineMessage::Ok { to: "unload".into() });
}

/// The fake's memory: a load reserves and keeps what --fake-load-mb says, and reports it.
#[test]
fn a_fake_load_reserves_its_memory() {
    let (_, out) = talk(&Options { load_mb: 8, ..fake() }, &[load("fake"), m(AppMessage::Status)]);
    assert_eq!(out[0], EngineMessage::Loaded { id: "fake".into(), memory_mb: 8 });
    assert!(matches!(&out[1], EngineMessage::Status { loaded, .. } if loaded.len() == 1 && loaded[0].memory_mb == 8));
}

/// Speech comes in 100 ms chunks of silence, then done.
#[test]
fn the_fake_speaks_silence_in_chunks() {
    let (_, out) = talk(&fake(), &[m(AppMessage::Speak { voice: "v".into(), text: "Thirty characters of words....".into(), request: 4 })]);
    let chunks: Vec<&String> = out.iter().filter_map(|x| if let EngineMessage::Pcm { samples, .. } = x { Some(samples) } else { None }).collect();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].len(), (2400usize * 2).div_ceil(3) * 4, "100 ms of 24 kHz 16-bit, as base64");
    assert!(matches!(out.last(), Some(EngineMessage::Done { request: 4, .. })));
}

/// The end of the input (the app is gone) ends the engine.
#[test]
fn the_end_of_the_input_ends_the_engine() {
    let (code, out) = talk(&fake(), &[]);
    assert_eq!(code, 0);
    assert!(out.is_empty());
}

/// The command line: --fake and its knobs, --version, and a refusal for anything else, and for a
/// --fake-* knob without --fake.
#[test]
fn the_command_line_is_read_strictly() {
    let p = |a: &[&str]| parse(a.iter().map(|s| s.to_string()));
    assert_eq!(p(&[]), Ok(Command::Run(Options::default())));
    assert_eq!(p(&["--version"]), Ok(Command::Version));
    let Ok(Command::Run(o)) = p(&["--fake", "--fake-ms-prefill", "5", "--fake-load-mb", "300", "--fake-exit-on", "generate"]) else { panic!() };
    assert!(o.fake && o.ms_prefill == 5 && o.load_mb == 300 && o.exit_on.as_deref() == Some("generate"));
    assert!(p(&["--fake-load-mb", "3"]).is_err(), "a fake knob needs --fake");
    assert!(p(&["--fast"]).is_err());
    assert!(p(&["--fake", "--fake-ms-prefill"]).is_err());
}

/// A library is named only by a plain file name, beside the engine.
#[test]
fn a_library_is_only_ever_a_name_beside_the_engine() {
    let folder = if cfg!(windows) { std::path::PathBuf::from("C:\\engine") } else { std::path::PathBuf::from("/engine") };
    assert_eq!(library_path(&folder, "audiocpp.dll"), Ok(folder.join("audiocpp.dll")));
    for bad in ["..\\x.dll", "../x.dll", "C:\\x.dll", "sub/x.dll", "", ".", "..", ".hidden.dll"] {
        assert!(library_path(&folder, bad).is_err(), "{bad:?} refused");
    }
    assert!(library_path(std::path::Path::new("engine"), "x.dll").is_err(), "a relative folder refused");
}

/// The fake's base64 is the standard one.
#[test]
fn base64_is_the_standard_one() {
    use crate::fake::encode_base64 as b;
    assert_eq!(b(b""), "");
    assert_eq!(b(b"f"), "Zg==");
    assert_eq!(b(b"fo"), "Zm8=");
    assert_eq!(b(b"foo"), "Zm9v");
    assert_eq!(b(b"foobar"), "Zm9vYmFy");
}
