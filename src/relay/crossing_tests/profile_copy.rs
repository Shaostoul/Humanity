//! From relay/core/profile_copy_tests.rs: the shared profile-copy fixture, its frame read by the
//! server's message type (`relay::RelayMessage`).

use humanity_protocol::profile_copy::PublicCopy;
use humanity_protocol::profile_copy::copy_text;
use humanity_protocol::profile_copy::is_key_text;
use humanity_protocol::profile_copy::is_withdrawal;
use humanity_protocol::profile_copy::next_copy_time;
use humanity_protocol::profile_copy::sign_copy;
use humanity_protocol::profile_copy::fixtures::FIXTURE;
use humanity_protocol::profile_copy::fixtures::FX_NOW_MS;
use humanity_protocol::profile_copy::fixtures::FX_TIMES;
use humanity_protocol::profile_copy::fixtures::FxCase;
use humanity_protocol::profile_copy::fixtures::copy_of_frame;
use humanity_protocol::profile_copy::fixtures::fx_cases;
use humanity_protocol::profile_copy::fixtures::fx_key;
use humanity_protocol::profile_copy::verify_copy;
use humanity_protocol::profile_copy::within_limits;
use serde_json::{json, Value};
use humanity_protocol::profile_copy::fixtures::FX_SIGNER;

/// The person's own copy exactly as the relay sends it (the echo of a save, asking): the frame a
/// device reads its copy from.
fn own_copy_frame(key: &str, c: &FxCase) -> Value {
    use crate::relay::relay::RelayMessage;
    let opt = |s: &str| if s.is_empty() { None } else { Some(s.to_string()) };
    serde_json::to_value(RelayMessage::ProfileData {
        name: "Ann".to_string(),
        bio: c.bio.to_string(),
        socials: c.socials.to_string(),
        avatar_url: opt(c.avatar_url),
        banner_url: opt(c.banner_url),
        pronouns: opt(c.pronouns),
        location: opt(c.location),
        website: opt(c.website),
        accepts_files: Some("friends".to_string()),
        privacy: Some(c.privacy.to_string()),
        stored: Some(true),
        signed_at: Some(c.signed_at),
        sign_now: Some(true),
        target: Some(key.to_string()),
    })
    .expect("a frame")
}

/// The whole fixture, as this code makes it today.
fn fixture_now() -> Value {
    let key = fx_key();
    let cases: Vec<Value> = fx_cases()
        .iter()
        .map(|c| {
            let frame = own_copy_frame(&key, c);
            let copy = copy_of_frame(&frame);
            let at = next_copy_time(FX_NOW_MS, c.signed_at, 0);
            json!({
                "label": c.label,
                "now_ms": FX_NOW_MS,
                "frame": frame,
                "copy": copy,
                "at": at,
                "text": copy_text(&key, &copy, at),
                "signature": sign_copy(&[FX_SIGNER; 32], &key, &copy, at),
            })
        })
        .collect();
    let times: Vec<Value> = FX_TIMES
        .iter()
        .map(|&(now_ms, signed_at, last_here, at)| json!({ "now_ms": now_ms, "signed_at": signed_at, "last_here": last_here, "at": at }))
        .collect();
    json!({
        "about": "The copy of a person's profile that other servers keep (BUG-191, 2026-10-12, docs/design/bug-191-profile-copies-plan.md 3.1 to 3.4). Made by the relay's own builder (crates/humanity-protocol/src/profile_copy.rs; ML-DSA signing there is deterministic) for the key of the 32-byte seed [signer_master_byte; 32]. Each case: `frame` is the person's own copy exactly as the relay sends it to a device (a profile_data with privacy, signed_at and sign_now: true); `copy` is public_copy of the frame's name and fields (an absent field is blank) under its privacy map; `at` is next_copy_time(now_ms, frame.signed_at, 0); `text` the exact words signed (copy_text); `signature` their Dilithium3 signature, lowercase hex. A web signature over the same words differs (noble's signing is hedged) but verifies the same. `next_copy_time` rows: at = max(now_ms, signed_at + 1, last_here + 1). Written by profile_copy_tests.rs write_the_profile_copy_fixture; checked by the_shared_fixture_is_what_the_builder_makes; read by both apps' tests.",
        "signer_master_byte": FX_SIGNER,
        "public_key": key,
        "cases": cases,
        "next_copy_time": times,
    })
}

/// WRITES scripts/tests/fixtures/profile-copy-v1.json from fixed seeds. Run it again only when the
/// shape changes on purpose: `node scripts/lib-tests.js write_the_profile_copy_fixture --ignored`.
#[test]
#[ignore = "writes scripts/tests/fixtures/profile-copy-v1.json; run: node scripts/lib-tests.js write_the_profile_copy_fixture --ignored"]
fn write_the_profile_copy_fixture() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(FIXTURE);
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fixture_now()).unwrap())).expect("the fixture is written");
}

/// THE SHARED FIXTURE BOTH APPS READ IS WHAT THIS CODE MAKES. From each case's own-copy frame the
/// builder makes the fixture's copy, time, words and signature byte for byte (ML-DSA signing here
/// is deterministic); the signature verifies with the key of the fixture's seed, and one changed
/// character in the words (the time, or the bio) fails it; the `next_copy_time` rows hold; and the
/// whole file is what the writer makes today, so a change to the relay's frame shows here.
///
/// Seen red 2026-10-12 with the first case's bio in the fixture's frame edited (`Λισαβόνα` to
/// `Λισσαβόνα`): "case 0: the copy is public_copy of the frame" failed.
#[test]
fn the_shared_fixture_is_what_the_builder_makes() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(FIXTURE);
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("JSON");
    let seed = [fx["signer_master_byte"].as_u64().expect("the seed byte") as u8; 32];
    let key = fx["public_key"].as_str().expect("the key").to_string();
    assert_eq!(key, fx_key(), "the key the seed gives");
    assert!(is_key_text(&key));

    let cases = fx["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 4, "four cases");
    for (i, case) in cases.iter().enumerate() {
        let frame = &case["frame"];
        assert_eq!((frame["type"].as_str(), frame["sign_now"].as_bool()), (Some("profile_data"), Some(true)), "case {i}: an own copy that asks");
        let copy = copy_of_frame(frame);
        let fixed: PublicCopy = serde_json::from_value(case["copy"].clone()).expect("a copy");
        assert_eq!(copy, fixed, "case {i}: the copy is public_copy of the frame");
        assert!(within_limits(&key, &copy), "case {i}: within the limits");
        let at = next_copy_time(case["now_ms"].as_u64().unwrap(), frame["signed_at"].as_u64().unwrap(), 0);
        assert_eq!(Some(at), case["at"].as_u64(), "case {i}: the time");
        let text = copy_text(&key, &copy, at);
        assert_eq!(Some(text.as_str()), case["text"].as_str(), "case {i}: the words");
        let sig = sign_copy(&seed, &key, &copy, at);
        assert_eq!(Some(sig.as_str()), case["signature"].as_str(), "case {i}: the signature, byte for byte");
        assert!(verify_copy(&key, &copy, at, &sig), "case {i}: it verifies");
        assert!(!verify_copy(&key, &copy, at ^ 1, &sig), "case {i}: one changed digit in the time fails it");
        if !copy.bio.is_empty() {
            let mut bio = copy.bio.chars().collect::<Vec<_>>();
            bio[0] = if bio[0] == 'x' { 'y' } else { 'x' };
            let changed = PublicCopy { bio: bio.into_iter().collect(), ..copy.clone() };
            assert!(!verify_copy(&key, &changed, at, &sig), "case {i}: one changed letter in the bio fails it");
        }
    }
    assert!(is_withdrawal(&serde_json::from_value(cases[1]["copy"].clone()).unwrap()), "the unlisted case is the withdrawal");

    for row in fx["next_copy_time"].as_array().expect("time rows") {
        let n = |k: &str| row[k].as_u64().unwrap();
        assert_eq!(next_copy_time(n("now_ms"), n("signed_at"), n("last_here")), n("at"), "{row}");
    }
    assert_eq!(fx, fixture_now(), "the fixture is what the writer makes today");
}
