//! A COMPLETE SEALED-SENDER V2 ENVELOPE, BOTH WAYS (2026-11-19, the post-quantum review's plan item
//! 1b, docs/reference/research/2026-11-16-pqc-review/).
//!
//! One envelope sealed by the website's own code (web/chat/crypto.js `pqBuildDmPuts` over pq.js
//! `pqDmSeal`, run with the vendored noble bundle) that this module opens and whose inner Dilithium3
//! signature it verifies; and one sealed by this module (`build_signed_inner` + `seal_v2`) that the
//! website's `pqOpenDmEnvelope` opens and verifies (scripts/tests/pq-cross.test.js). The text is
//! not ASCII on purpose (two-byte, three-byte and four-byte UTF-8), so the signed words' encoding
//! is held to one answer too.
//!
//! The shared fixture is scripts/tests/fixtures/dm-envelope-v2.json. Its `rust` half is written by
//! `write_the_rust_envelope` below, its `web` half by scripts/pq-cross-fixtures.mjs, which seals to
//! the recipient key in the `rust` half (so write the Rust half first):
//!
//!   node scripts/lib-tests.js --crate humanity-engine write_the_rust_envelope --ignored
//!   node scripts/pq-cross-fixtures.mjs
//!
//! These tests pin today's envelope and change nothing in it. Nothing here depends on the Ed25519
//! (wallet) key: both identities come from `[byte; 32]` through the Dilithium3 and Kyber768 labels.

use super::*;

/// The shared fixture, from the repository root.
const FIXTURE: &str = "scripts/tests/fixtures/dm-envelope-v2.json";
const SENDER_MASTER_BYTE: u8 = 11;
const RECIPIENT_MASTER_BYTE: u8 = 22;
const TS: u64 = 1_763_500_000_000;
/// "cross-check: Grüße, café, 日本語, 🌍", written with escapes so the source stays plain ASCII.
const TEXT: &str = "cross-check: Gr\u{fc}\u{df}e, caf\u{e9}, \u{65e5}\u{672c}\u{8a9e}, \u{1f30d}";

/// The fixture, or its copy in the folder HOS_PQ_CROSS_DIR names (how a changed copy is run to
/// see these tests red; scripts/tests/pq-cross.test.js reads the same variable).
fn fixture() -> serde_json::Value {
    let path = match std::env::var_os("HOS_PQ_CROSS_DIR") {
        Some(dir) => std::path::PathBuf::from(dir).join("dm-envelope-v2.json"),
        None => std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(FIXTURE),
    };
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("the fixture is JSON")
}

/// A person's Dilithium3 identity (hex) from the master seed `[byte; 32]`.
fn identity_hex(byte: u8) -> String {
    hex::encode(pq_crypto::DilithiumKeypair::from_seed(&pq_crypto::derive_dilithium_seed(&[byte; 32])).public_key())
}

/// A person's Kyber768 DM keypair from the master seed `[byte; 32]`.
fn dm_keypair(byte: u8) -> DmPqKeypair {
    DmPqKeypair::from_bip39_seed(&[byte; 32]).expect("the DM keypair")
}

/// The wire envelope carries exactly `v` and the three base64 fields: nobody's key, no sender.
fn assert_wire_shape(envelope: &str, who: &str) {
    let v: serde_json::Value = serde_json::from_str(envelope).expect("the envelope is JSON");
    let mut keys: Vec<&str> = v.as_object().expect("an object").keys().map(|k| k.as_str()).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["ct_b64", "ek_ct_b64", "nonce_b64", "v"], "{who}: the envelope's fields");
    assert_eq!(v["v"], 2, "{who}: version 2");
}

/// WRITES the `rust` half of scripts/tests/fixtures/dm-envelope-v2.json, keeping the `web` half as
/// it is. Run it only on purpose (see the module's notes), then scripts/pq-cross-fixtures.mjs.
#[test]
#[ignore = "writes the rust half of scripts/tests/fixtures/dm-envelope-v2.json; run: node scripts/lib-tests.js --crate humanity-engine write_the_rust_envelope --ignored"]
fn write_the_rust_envelope() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(FIXTURE);
    let mut fx: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let recipient_kyber = dm_keypair(RECIPIENT_MASTER_BYTE).public_base64();
    let inner = build_signed_inner(
        &[SENDER_MASTER_BYTE; 32],
        &identity_hex(SENDER_MASTER_BYTE),
        &identity_hex(RECIPIENT_MASTER_BYTE),
        TS,
        TEXT,
    )
    .expect("the signed inner payload");
    fx["about"] = serde_json::json!(
        "A complete sealed-sender v2 DM envelope each way: rust is sealed by src/net/dm_pq.rs (build_signed_inner + seal_v2), \
         web by web/chat/crypto.js pqBuildDmPuts (scripts/pq-cross-fixtures.mjs). Opened and verified by the other side: \
         src/net/dm_pq_cross_tests.rs and scripts/tests/pq-cross.test.js. Identities from [byte; 32] master seeds."
    );
    fx["rust"] = serde_json::json!({
        "written_by": "src/net/dm_pq_cross_tests.rs write_the_rust_envelope (dm_pq build_signed_inner + seal_v2)",
        "sender_master_byte": SENDER_MASTER_BYTE,
        "recipient_master_byte": RECIPIENT_MASTER_BYTE,
        "recipient_kyber_b64": recipient_kyber,
        "ts": TS,
        "text": TEXT,
        "envelope": seal_v2(&recipient_kyber, &inner).expect("sealed"),
    });
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fx).unwrap())).expect("the fixture is written");
}

/// AN ENVELOPE THE WEBSITE SEALED OPENS HERE, AND ITS SENDER IS PROVEN: `open_v2` with the
/// recipient's own key gives the inner payload, and `parse_verify_inner` checks the website's
/// Dilithium3 signature against the sender's key, with the exact time and text the website signed.
/// The website sealed to the recipient key this module derives (the fixture's `rust` half), and
/// nobody else's key opens it.
///
/// Seen red 2026-11-19 with one byte of `web.envelope`'s ciphertext flipped (AES-GCM's tag then
/// fails): "the website's envelope opens with the recipient's key" failed with
/// "aes open (wrong key / tampered): aead::Error".
#[test]
fn an_envelope_the_website_sealed_opens_and_verifies_here() {
    let fx = fixture();
    let web = &fx["web"];
    assert_eq!(web["sealed_to"].as_str(), fx["rust"]["recipient_kyber_b64"].as_str(), "sealed to the key this module derives");
    assert_eq!(web["sealed_to"].as_str(), Some(dm_keypair(RECIPIENT_MASTER_BYTE).public_base64().as_str()));
    let envelope = web["envelope"].as_str().expect("the website's envelope");
    assert_wire_shape(envelope, "the website's envelope");
    let inner_json = open_v2(&dm_keypair(RECIPIENT_MASTER_BYTE), envelope)
        .unwrap_or_else(|e| panic!("the website's envelope opens with the recipient's key: {e}"));
    let inner = parse_verify_inner(&inner_json)
        .unwrap_or_else(|e| panic!("the website's inner signature verifies here: {e}"));
    assert_eq!(inner.from, identity_hex(SENDER_MASTER_BYTE), "from the sender");
    assert_eq!(inner.to, identity_hex(RECIPIENT_MASTER_BYTE), "to the recipient");
    assert_eq!(Some(inner.ts), web["ts"].as_u64(), "the time the website signed");
    assert_eq!(Some(inner.text.as_str()), web["text"].as_str(), "the text the website signed");
    assert_eq!(inner.text, TEXT, "the same text as the Rust half");
    assert!(open_v2(&dm_keypair(SENDER_MASTER_BYTE), envelope).is_err(), "the sender's own key does not open the recipient's copy");
}

/// THE RUST HALF IS STILL WHAT THIS MODULE MAKES AND OPENS: the recipient key is today's
/// derivation, and the envelope opens and verifies here as it does on the website. (ML-KEM
/// encapsulation and the AES-GCM nonce are random, so a fresh seal is never the same bytes; the
/// website's test opens this exact envelope.)
///
/// Seen red 2026-11-19 with one character of `rust.text` changed: "the text this module signed"
/// failed.
#[test]
fn the_rust_envelope_is_what_this_module_makes_and_opens() {
    let fx = fixture();
    let rust = &fx["rust"];
    assert_eq!(rust["recipient_kyber_b64"].as_str(), Some(dm_keypair(RECIPIENT_MASTER_BYTE).public_base64().as_str()));
    let envelope = rust["envelope"].as_str().expect("the Rust envelope");
    assert_wire_shape(envelope, "the Rust envelope");
    let inner = parse_verify_inner(&open_v2(&dm_keypair(RECIPIENT_MASTER_BYTE), envelope).expect("opens")).expect("verifies");
    assert_eq!(inner.from, identity_hex(SENDER_MASTER_BYTE));
    assert_eq!(Some(inner.ts), rust["ts"].as_u64());
    assert_eq!(Some(inner.text.as_str()), rust["text"].as_str(), "the text this module signed");
}
