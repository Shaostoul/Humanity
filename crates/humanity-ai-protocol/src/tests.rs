//! The protocol's own tests: every message survives the line both ways, the version is read first and
//! a mismatch is refused with words, and every words key the protocol names has its English.

use super::*;

fn every_app_message() -> Vec<AppMessage> {
    vec![
        AppMessage::Hello { app_version: "0.1553.0".into() },
        AppMessage::Load {
            id: "granite-4-0-1b".into(),
            path: "C:\\Users\\a\\AppData\\Roaming\\HumanityOS\\ai\\models\\granite\\granite-4.0-1b-Q4_K_M.gguf".into(),
            sha256: "22ec".into(),
            kind: ModelKind::Brain,
            context: 4096,
            threads: 4,
            device: Device::Cpu,
        },
        AppMessage::Generate {
            id: "granite-4-0-1b".into(),
            request: 7,
            persona: "You are Mara, the ship's botanist.".into(),
            turns: vec![Turn { role: Role::User, text: "What grows best here?".into() }, Turn { role: Role::Assistant, text: "Beans.".into() }],
            grammar: Some("{line, mood, gesture}".into()),
            max_tokens: 64,
        },
        AppMessage::Speak { voice: "alba-casual".into(), text: "Hello there.".into(), request: 8 },
        AppMessage::ReadImage { id: "pp-ocrv6".into(), request: 9, png: "iVBORw0KGgo=".into() },
        AppMessage::Cancel { request: 7 },
        AppMessage::Reuse { enabled: false },
        AppMessage::Status,
        AppMessage::Unload { id: "granite-4-0-1b".into() },
        AppMessage::Shutdown,
    ]
}

fn every_engine_message() -> Vec<EngineMessage> {
    vec![
        EngineMessage::Hello { engine_version: "1.0.0".into(), backends: vec![Backend::Cpu, Backend::Vulkan], cpu_features: vec!["avx2".into()], fake: false },
        EngineMessage::Loaded { id: "granite-4-0-1b".into(), memory_mb: 1600 },
        EngineMessage::Error { request: Some(7), code: ErrorCode::OutOfMemory, words_key: ErrorCode::OutOfMemory.words_key().into(), detail: "could not reserve 2900 MB".into() },
        EngineMessage::Error { request: None, code: ErrorCode::BadMessage, words_key: ErrorCode::BadMessage.words_key().into(), detail: "expected value".into() },
        EngineMessage::Token { request: 7, text: "Beans ".into() },
        EngineMessage::Done { request: 7, tokens: 12, ms_prefill: 180, ms_generate: 900 },
        EngineMessage::Pcm { request: 8, samples: "AAAA".into() },
        EngineMessage::Text { request: 9, lines: vec!["INGREDIENTS: WATER".into()], confidence: 0.5 },
        EngineMessage::Cancelled { request: 7 },
        EngineMessage::Ok { to: "reuse".into() },
        EngineMessage::Status {
            memory_mb: Some(12),
            threads: 4,
            loaded: vec![LoadedModel { id: "granite-4-0-1b".into(), kind: ModelKind::Brain, memory_mb: 1600 }],
            counters: Counters { requests: 3, tokens: 40, cancelled: 1, errors: 0 },
            reuse: true,
            fake: true,
        },
    ]
}

/// Every message of 7.3's table, both ways, comes back from its line exactly as it went in, on one line,
/// with the version first. Seen red 2026-11-09 with `encode` building the line through serde_json's Map,
/// which sorts its keys: "the version first: {"app_version":"0.1553.0","type":"hello","v":1}".
#[test]
fn every_message_survives_its_line_both_ways() {
    for m in every_app_message() {
        let line = encode(&m);
        assert!(!line.contains('\n'), "one line: {line}");
        assert!(line.starts_with("{\"v\":1,"), "the version first: {line}");
        assert_eq!(decode_app(&line), Ok(m.clone()), "{line}");
        assert!(line.contains(&format!("\"type\":\"{}\"", m.kind())), "kind() names the wire type: {line}");
    }
    for m in every_engine_message() {
        let line = encode(&m);
        assert!(!line.contains('\n'), "one line: {line}");
        assert!(line.starts_with("{\"v\":1,"), "the version first: {line}");
        assert_eq!(decode_engine(&line), Ok(m), "{line}");
    }
}

/// A line of another version is refused as a version mismatch, even when the rest of it would not
/// parse as any message of ours: the version is read first, so the words name the real reason. Seen red
/// 2026-11-09 with the version checked after the message: the line below came back as Shape("unknown
/// variant `say`").
#[test]
fn another_version_is_refused_before_the_rest_is_read() {
    assert_eq!(decode_engine(r#"{"v":2,"type":"say","words":"hi"}"#), Err(ProtocolError::Version { theirs: 2, ours: PROTOCOL }));
    let old = encode_versioned(&EngineMessage::Ok { to: "reuse".into() }, 0);
    assert_eq!(decode_engine(&old), Err(ProtocolError::Version { theirs: 0, ours: PROTOCOL }));
    let e = decode_app(r#"{"v":99,"type":"hello","app_version":"x"}"#).unwrap_err();
    assert_eq!(e.words_key(), "ai.protocol.version_mismatch");
    assert_eq!(e.fill(), vec![("theirs", "99".to_string()), ("ours", PROTOCOL.to_string())]);
    assert_eq!(e.code(), ErrorCode::Version);
}

/// What is not a message of ours says which way it is not.
#[test]
fn lines_that_are_not_messages_say_why() {
    assert_eq!(decode_engine(r#"{"type":"hello"}"#), Err(ProtocolError::NoVersion));
    assert_eq!(decode_engine(r#"{"v":"1","type":"hello"}"#), Err(ProtocolError::NoVersion));
    assert!(matches!(decode_engine("hello"), Err(ProtocolError::NotJson(_))));
    assert!(matches!(decode_engine("[1,2]"), Err(ProtocolError::NotJson(_))));
    assert!(matches!(decode_engine(r#"{"v":1,"type":"say"}"#), Err(ProtocolError::Shape(_))));
    assert!(matches!(decode_engine(r#"{"v":1,"type":"loaded","id":"x"}"#), Err(ProtocolError::Shape(_))));
    let long = format!("{{\"v\":1,\"type\":\"ok\",\"to\":\"{}\"}}", "x".repeat(MAX_LINE_BYTES));
    assert!(matches!(decode_engine(&long), Err(ProtocolError::NotJson(_))));
}

/// Every words key this crate names (each refusal code's and each protocol error's) has its English in
/// data/i18n/en.json, so no refusal ever reaches a person as a bare key. Seen red 2026-11-09 before the
/// `ai` words were added: "ai.error.version has no words in data/i18n/en.json".
#[test]
fn every_words_key_has_its_english() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json");
    let text = std::fs::read_to_string(&path).expect("data/i18n/en.json");
    let en: serde_json::Value = serde_json::from_str(&text).expect("en.json parses");
    let has = |key: &str| key.split('.').try_fold(&en, |v, part| v.get(part)).is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
    let mut keys: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.words_key()).collect();
    for e in [ProtocolError::Version { theirs: 2, ours: 1 }, ProtocolError::NoVersion, ProtocolError::NotJson(String::new()), ProtocolError::Shape(String::new())] {
        keys.push(e.words_key());
    }
    for k in keys {
        assert!(has(k), "{k} has no words in data/i18n/en.json");
    }
}
