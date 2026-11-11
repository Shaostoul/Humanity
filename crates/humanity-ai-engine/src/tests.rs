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

/// Without --fake and without the `llama` feature (the plain build the app's client tests make), the
/// engine has no runtime: hello with no backends, and every model request refused with its words key,
/// never a crash and never a silence.
#[cfg(not(feature = "llama"))]
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

/// A model's pieces are joined into whole characters: a character split across two pieces is sent
/// once whole, bytes that can never be a character become one replacement mark, and an unfinished
/// character at the end of a reply is not dropped silently. Seen red 2026-11-09 with `push` returning
/// each piece by itself (`String::from_utf8_lossy`, the dangling half trimmed): the second half of
/// "é" came out alone and was lost, "left: \"\" right: \"é\"".
#[test]
fn pieces_are_joined_into_whole_characters() {
    use crate::text::Utf8Stream;
    let mut s = Utf8Stream::default();
    let e_acute = "é".as_bytes();
    assert_eq!(s.push(b"caf"), "caf");
    assert_eq!(s.push(&e_acute[..1]), "", "half a character waits");
    assert_eq!(s.push(&e_acute[1..]), "é");
    assert_eq!(s.push(&[b'a', 0xff, b'b']), "a\u{fffd}b", "a byte that is never UTF-8 is marked, not dropped");
    let hindi = "नमस्ते".as_bytes();
    let mut out = String::new();
    for b in hindi {
        out.push_str(&s.push(&[*b]));
    }
    assert_eq!(out, "नमस्ते", "a byte at a time still comes out whole");
    assert_eq!(s.push(&"😀".as_bytes()[..2]), "");
    assert_eq!(s.finish(), "\u{fffd}", "an unfinished character at the end is marked");
    assert_eq!(s.finish(), "");
}

/// THE REAL ENGINE (the `llama` feature, `just ai-engine test`'s second half): llama.cpp starts with
/// the CPU kernels beside the test program (build.rs copies them there), says hello with the CPU
/// backend, and refuses a file that is not a model in words, going on afterwards. Needs no model file,
/// so it always runs. Seen red 2026-11-09 with build.rs not copying the libraries into deps/ (the test
/// program still started, cargo putting the profile folder on PATH, but found no CPU kernels beside
/// itself): "the CPU backend: Hello { engine_version: \"1.0.0\", backends: [], .. }".
#[cfg(feature = "llama")]
#[test]
fn the_real_engine_starts_and_refuses_a_file_that_is_not_a_model() {
    let not_a_model = std::env::temp_dir().join("humanity-ai-not-a-model.gguf");
    std::fs::write(&not_a_model, b"this is not a GGUF file").expect("a scratch file");
    let load = m(AppMessage::Load { id: "x".into(), path: not_a_model.display().to_string(), sha256: String::new(), kind: ModelKind::Brain, context: 512, threads: 2, device: Device::Cpu });
    let voice = m(AppMessage::Load { id: "v".into(), path: not_a_model.display().to_string(), sha256: String::new(), kind: ModelKind::Voice, context: 512, threads: 2, device: Device::Cpu });
    let (_, out) = talk(&Options::default(), &[m(AppMessage::Hello { app_version: "t".into() }), load, voice, m(AppMessage::Status)]);
    let _ = std::fs::remove_file(&not_a_model);
    assert!(matches!(&out[0], EngineMessage::Hello { fake: false, backends, .. } if backends == &vec![humanity_ai_protocol::Backend::Cpu]), "the CPU backend: {:?}", out[0]);
    assert!(matches!(&out[1], EngineMessage::Error { code: ErrorCode::LoadFailed, words_key, .. } if words_key == "ai.error.load_failed"), "{:?}", out[1]);
    assert!(matches!(&out[2], EngineMessage::Error { code: ErrorCode::NoBackend, .. }), "no voice runtime yet: {:?}", out[2]);
    assert!(matches!(&out[3], EngineMessage::Status { loaded, memory_mb: Some(mb), .. } if loaded.is_empty() && *mb > 0), "{:?}", out[3]);
}

/// The model file the real tests use: HUMANITY_AI_TEST_GGUF, else the operator's copy of Granite 4.0 1B
/// (embeddable-ai.md, rung 2; the Granite finding's addendum). None when neither is on this computer.
#[cfg(feature = "llama")]
fn test_model() -> Option<std::path::PathBuf> {
    let p = std::env::var_os("HUMANITY_AI_TEST_GGUF").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(r"E:\HumanityOS\ai-src\granite-4.0-1b\granite-4.0-1b-Q4_K_M.gguf"));
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("SKIPPED: no model file at {} (set HUMANITY_AI_TEST_GGUF to a GGUF to run this test); it did not run, it did not pass", p.display());
        None
    }
}

#[cfg(feature = "llama")]
fn load_real(path: &std::path::Path) -> String {
    m(AppMessage::Load { id: "granite".into(), path: path.display().to_string(), sha256: String::new(), kind: ModelKind::Brain, context: 2048, threads: 4, device: Device::Cpu })
}

#[cfg(feature = "llama")]
fn ask_real(request: u64, question: &str, grammar: Option<&str>, max_tokens: u32) -> String {
    m(AppMessage::Generate {
        id: "granite".into(),
        request,
        persona: "You are a crew member on a ship in a game. Answer in one short sentence.".into(),
        turns: vec![Turn { role: Role::User, text: question.into() }],
        grammar: grammar.map(str::to_string),
        max_tokens,
    })
}

/// A REAL MODEL LOADS AND ANSWERS: Granite 4.0 1B (or HUMANITY_AI_TEST_GGUF) loads on 4 threads, a
/// question is answered piece by piece with its timing, the status reports its memory and the model,
/// and it unloads. SKIPPED, saying so, when no model file is on this computer. Seen red 2026-11-09
/// with `real::reply` holding its pieces back and sending them only at the end: "the reply came in 1
/// piece(s): \"I am a ship's cook.\"".
#[cfg(feature = "llama")]
#[test]
fn a_real_model_loads_and_answers_piece_by_piece() {
    let Some(model) = test_model() else { return };
    let (_, out) = talk(&Options::default(), &[load_real(&model), ask_real(1, "What is your job aboard?", None, 40), m(AppMessage::Status), m(AppMessage::Unload { id: "granite".into() })]);
    assert!(matches!(&out[0], EngineMessage::Loaded { id, .. } if id == "granite"), "{:?}", out[0]);
    let pieces: Vec<&str> = out.iter().filter_map(|x| if let EngineMessage::Token { request: 1, text } = x { Some(text.as_str()) } else { None }).collect();
    let reply: String = pieces.concat();
    assert!(pieces.len() > 3, "the reply came in {} piece(s): {reply:?}", pieces.len());
    assert!(reply.chars().any(char::is_alphabetic), "words: {reply:?}");
    let done = out.iter().find_map(|x| if let EngineMessage::Done { request: 1, tokens, ms_prefill, ms_generate } = x { Some((*tokens, *ms_prefill, *ms_generate)) } else { None }).expect("a done");
    assert!(done.0 > 0 && done.0 <= 40 && done.2 > 0, "{done:?}");
    eprintln!("REAL MODEL: {reply:?} ({} tokens, prefill {} ms, writing {} ms)", done.0, done.1, done.2);
    assert!(out.iter().any(|x| matches!(x, EngineMessage::Status { loaded, memory_mb: Some(mb), threads: 4, .. } if loaded.len() == 1 && *mb > 50)), "{out:?}");
    assert_eq!(out.last(), Some(&EngineMessage::Ok { to: "unload".into() }));
}

/// The operator's copy of Ministral 3 3B (HUMANITY_AI_TEST_MINISTRAL to point elsewhere; the Ministral
/// finding's 2026-11-11 addendum). None, saying so, when it is not on this computer.
#[cfg(feature = "llama")]
fn ministral() -> Option<std::path::PathBuf> {
    let p = std::env::var_os("HUMANITY_AI_TEST_MINISTRAL").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(r"E:\HumanityOS\ai-src\ministral-3-3b\Ministral-3-3B-Instruct-2512-Q4_K_M.gguf"));
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("SKIPPED: Ministral 3 3B is not at {} (set HUMANITY_AI_TEST_MINISTRAL to its GGUF to run this test); it did not run, it did not pass", p.display());
        None
    }
}

/// MINISTRAL 3 3B LOADS AND ANSWERS ON THE PINNED LLAMA.CPP (2026-11-11, embeddable AI: the Rich brain
/// beside Granite's quick one): Mistral's own GGUF (architecture `mistral3`) loads through the engine on
/// 4 threads, a question is answered piece by piece in words with a done, the status reports it, and it
/// unloads. And the finding's condition on the system prompt, at the engine: the prompt the engine
/// builds carries the project's persona inside Mistral's [SYSTEM_PROMPT] markers and never the
/// repository's own default system message (the GGUF's template holds it, "You power an AI assistant
/// called Le Chat"), with a persona or without one. SKIPPED in words when the file is not on this
/// computer.
///
/// It passed on its first run on the real file (the pinned llama.cpp knows `mistral3`). Seen red
/// 2026-11-11 with HUMANITY_AI_TEST_MINISTRAL pointed at Granite 4.0 1B, whose template is not
/// Mistral's: "Mistral's markers around the project's persona:
/// \"<|start_of_role|>system<|end_of_role|>You are Mara, the hydroponics lead.<|end_of_text|>...\"".
#[cfg(feature = "llama")]
#[test]
fn ministral_loads_and_answers() {
    let Some(model) = ministral() else { return };
    let path = model.display().to_string();
    let load = m(AppMessage::Load { id: "ministral".into(), path: path.clone(), sha256: String::new(), kind: ModelKind::Brain, context: 2048, threads: 4, device: Device::Cpu });
    let ask = m(AppMessage::Generate {
        id: "ministral".into(),
        request: 1,
        persona: "You are a crew member on a ship in a game. Answer in one short sentence.".into(),
        turns: vec![Turn { role: Role::User, text: "What is your job aboard?".into() }],
        grammar: None,
        max_tokens: 40,
    });
    let (_, out) = talk(&Options::default(), &[load, ask, m(AppMessage::Status), m(AppMessage::Unload { id: "ministral".into() })]);
    assert!(matches!(&out[0], EngineMessage::Loaded { id, .. } if id == "ministral"), "Ministral loads: {:?}", out[0]);
    let pieces: Vec<&str> = out.iter().filter_map(|x| if let EngineMessage::Token { request: 1, text } = x { Some(text.as_str()) } else { None }).collect();
    let reply: String = pieces.concat();
    assert!(pieces.len() > 3, "the reply came in {} piece(s): {reply:?}", pieces.len());
    assert!(reply.split_whitespace().filter(|w| w.chars().any(char::is_alphabetic)).count() >= 3, "words: {reply:?}");
    assert!(!reply.contains("[INST]") && !reply.contains("[/INST]") && !reply.contains("[SYSTEM_PROMPT]"), "no template markers in the reply: {reply:?}");
    let done = out.iter().find_map(|x| if let EngineMessage::Done { request: 1, tokens, ms_prefill, ms_generate } = x { Some((*tokens, *ms_prefill, *ms_generate)) } else { None }).expect("a done");
    assert!(done.0 > 0 && done.0 <= 40 && done.2 > 0, "{done:?}");
    eprintln!("MINISTRAL: {reply:?} ({} tokens, prefill {} ms, writing {} ms)", done.0, done.1, done.2);
    assert!(out.iter().any(|x| matches!(x, EngineMessage::Status { loaded, memory_mb: Some(mb), threads: 4, .. } if loaded.len() == 1 && *mb > 1000)), "{out:?}");
    assert_eq!(out.last(), Some(&EngineMessage::Ok { to: "unload".into() }));

    // The system prompt is the project's, never the repository's (the finding, F4 and the addendum).
    let folder = std::env::current_exe().expect("the test program").parent().expect("its folder").to_path_buf();
    crate::llama::init(&folder, false).expect("llama.cpp starts");
    let brain = crate::llama::Brain::load(&path, 512, 2).expect("the model loads");
    let turn = vec![Turn { role: Role::User, text: "Hello?".into() }];
    let with = brain.prompt("You are Mara, the hydroponics lead.", &turn).expect("a prompt");
    assert!(with.contains("[SYSTEM_PROMPT]") && with.contains("You are Mara, the hydroponics lead.") && with.contains("[INST]"), "Mistral's markers around the project's persona: {with:?}");
    let without = brain.prompt("", &turn).expect("a prompt");
    for p in [&with, &without] {
        assert!(!p.contains("Le Chat") && !p.contains("created by Mistral AI"), "never the repository's default system message: {p:?}");
    }
    assert!(!without.contains("[SYSTEM_PROMPT]"), "no persona, no system prompt at all: {without:?}");
}

/// A FIXED SHAPE (8.3's "the free-text reply is generated in a fixed shape"): with a grammar, the reply
/// can only be what the grammar allows. Seen red 2026-11-09 with the grammar never handed to the
/// sampler (`Brain::sampler` passing NULL): "the grammar's shape: \"Yes, I am a crew member on\"".
#[cfg(feature = "llama")]
#[test]
fn a_grammar_fixes_the_replys_shape() {
    let Some(model) = test_model() else { return };
    let grammar = r#"root ::= "yes" | "no""#;
    let (_, out) = talk(&Options::default(), &[load_real(&model), ask_real(2, "Are you a member of the crew?", Some(grammar), 8), ask_real(3, "hi", Some("root ::= ("), 8)]);
    let reply: String = out.iter().filter_map(|x| if let EngineMessage::Token { request: 2, text } = x { Some(text.as_str()) } else { None }).collect();
    assert!(reply == "yes" || reply == "no", "the grammar's shape: {reply:?}");
    assert!(out.iter().any(|x| matches!(x, EngineMessage::Error { request: Some(3), code: ErrorCode::Internal, .. })), "a grammar that does not parse is refused in words: {out:?}");
}

/// An input that hands over its first lines at once and the rest only after a pause, so a message can
/// arrive while a reply is being written (as a person's walking away does).
#[cfg(feature = "llama")]
struct LateInput {
    now: Cursor<Vec<u8>>,
    later: Option<(std::time::Duration, Cursor<Vec<u8>>)>,
}

#[cfg(feature = "llama")]
impl std::io::Read for LateInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.now.read(buf)?;
        if n > 0 {
            return Ok(n);
        }
        if let Some((wait, rest)) = self.later.take() {
            std::thread::sleep(wait);
            self.now = rest;
            return self.now.read(buf);
        }
        Ok(0)
    }
}

/// A CANCEL READ BETWEEN PIECES stops a real reply that has started writing, says so, and the engine
/// answers the next request. The cancel arrives 5 s in (the load takes about 2.5 s on the operator's PC
/// and a piece about 40 ms), so the reply is well under way. Seen red 2026-11-09 with `real::reply` not
/// looking for messages between pieces: the story ran to its end, and the cancel was only read after
/// it, "no done for a cancelled reply".
#[cfg(feature = "llama")]
#[test]
fn a_cancel_stops_a_real_reply_between_pieces() {
    let Some(model) = test_model() else { return };
    let lines = |ls: &[String]| Cursor::new(ls.iter().map(|l| format!("{l}\n")).collect::<String>().into_bytes());
    let long = m(AppMessage::Generate {
        id: "granite".into(),
        request: 4,
        persona: "You are a storyteller. Every answer is a long story of many paragraphs.".into(),
        turns: vec![Turn { role: Role::User, text: "Tell me a long story about a ship and everyone aboard it.".into() }],
        grammar: None,
        max_tokens: 600,
    });
    let input = LateInput {
        now: lines(&[load_real(&model), long]),
        later: Some((std::time::Duration::from_secs(5), lines(&[m(AppMessage::Cancel { request: 4 }), ask_real(5, "Hello?", None, 4)]))),
    };
    let mut raw = Vec::new();
    run(&Options::default(), std::io::BufReader::new(input), &mut raw);
    let out: Vec<EngineMessage> = String::from_utf8(raw).expect("utf-8").lines().map(|l| decode_engine(l).expect("a message")).collect();
    let written = out.iter().filter(|x| matches!(x, EngineMessage::Token { request: 4, .. })).count();
    assert!(out.contains(&EngineMessage::Cancelled { request: 4 }), "no cancelled after {written} pieces");
    assert!(written > 0, "the reply had started writing before the cancel");
    assert!(!out.iter().any(|x| matches!(x, EngineMessage::Done { request: 4, .. })), "no done for a cancelled reply");
    assert!(out.iter().any(|x| matches!(x, EngineMessage::Done { request: 5, .. })), "the next request is answered");
}

/// THE MEASUREMENT of 9.1 to 9.3 on this computer (`just ai-engine measure`; ignored otherwise): the
/// test program drops itself to below-normal priority, as the job object holds the engine, then loads
/// the model on 4 threads (9.2 on a 6-core PC) with a 4,096-token working memory and reports the load
/// time, the committed memory and working set, the prefill rate on a character-sized persona read
/// whole, the writing rate over 128 tokens, and the prefill of a second question with the persona
/// reused. Numbers, not a pass or fail: the design's section 9 records them.
#[cfg(feature = "llama")]
#[test]
#[ignore = "a measurement, run by hand: just ai-engine measure"]
fn measure_the_model_on_this_computer() {
    use std::time::Instant;
    let Some(model) = test_model() else { return };
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn SetPriorityClass(process: isize, class: u32) -> i32;
        }
        // BELOW_NORMAL_PRIORITY_CLASS, as the job object sets it for the engine (src/ai/job.rs).
        // SAFETY: this process's pseudo-handle and a documented class value.
        assert!(unsafe { SetPriorityClass(GetCurrentProcess(), 0x0000_4000) } != 0, "below-normal priority");
    }
    let threads = std::env::var("HUMANITY_AI_MEASURE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(4u32);
    let folder = std::env::current_exe().expect("the test program").parent().expect("its folder").to_path_buf();
    crate::llama::init(&folder, false).expect("llama.cpp starts");
    let mb = |u: Option<crate::memory::Usage>| u.map_or((0, 0), |u| (u.committed / (1 << 20), u.working_set / (1 << 20)));
    let base = mb(crate::memory::usage());
    let t = Instant::now();
    let mut brain = crate::llama::Brain::load(&model.display().to_string(), 4096, threads).expect("the model loads");
    let load_ms = t.elapsed().as_millis();
    let loaded = mb(crate::memory::usage());
    // A character's persona of the size rung 3 plans (its row of crew.ron, rules, example lines).
    let persona = "You are Mara Okafor, the hydroponics lead aboard the colony ship Ne Obliviscaris, a character in HumanityOS run by AI on the player's computer. \
        You tend the grow racks on deck three: lettuce, beans, potatoes and the dwarf fruit trees, and you watch the water and nutrient lines. You are practical, warm and a little dry. \
        You speak in one to three short sentences. You never claim to be a person; if sincerely asked, you say you are a character run by AI. You give no medical, legal or money advice, \
        and for real-world facts about plants or food you point to the Library. You know the crew: Captain Reyes on the bridge, Tomas in engineering, Dr. Lin in the medical bay. \
        Things you have said before: 'The beans like it warm; the lettuce does not.' 'If the pH drifts, the whole rack sulks.' 'Grab a basket if you want to help with the harvest.'";
    let turn = |q: &str| vec![Turn { role: Role::User, text: q.into() }];
    let tokens = brain.tokenize(&brain.prompt(persona, &turn("Describe your work aboard in detail, everything you do in a day.")).unwrap()).unwrap();
    let t = Instant::now();
    brain.prefill(&tokens, true, || false).unwrap();
    let prefill_ms = t.elapsed().as_secs_f64() * 1000.0;
    let mut s = brain.sampler(crate::llama::Sampling { temp: 0.0, ..Default::default() }, None).unwrap();
    let t = Instant::now();
    let mut n = 0;
    while n < 128 && brain.next(&mut s).unwrap().is_some() {
        n += 1;
    }
    let gen_ms = t.elapsed().as_secs_f64() * 1000.0;
    let after = mb(crate::memory::usage());
    let second = brain.tokenize(&brain.prompt(persona, &turn("Where is the galley?")).unwrap()).unwrap();
    let t = Instant::now();
    let reused = brain.prefill(&second, true, || false).unwrap().unwrap_or(0);
    let reuse_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "MEASURE {}: {threads} threads at below-normal priority\n  load: {load_ms} ms; committed {} -> {} MB, working set {} -> {} MB (before -> after the load)\n  prefill: {} tokens in {prefill_ms:.0} ms = {:.1} tokens a second (persona read whole)\n  writing: {n} tokens in {gen_ms:.0} ms = {:.1} tokens a second; committed {} MB, working set {} MB after\n  second question: {} of {} tokens reused, the rest read in {reuse_ms:.0} ms",
        model.display(),
        base.0,
        loaded.0,
        base.1,
        loaded.1,
        tokens.len(),
        tokens.len() as f64 * 1000.0 / prefill_ms,
        n as f64 * 1000.0 / gen_ms,
        after.0,
        after.1,
        reused,
        second.len()
    );
}
