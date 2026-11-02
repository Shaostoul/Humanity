//! TESTS OF THE PROTOCOL, TOGETHER WITH THE APP'S NETWORK SIDE (the crate split, 2026-10-31,
//! docs/design/crate-split.md): written beside the protocol's code, they reach into this crate to do
//! it, so each moved up WHOLE to the lowest crate that may use everything it reaches. Nothing in a
//! test changed but its paths. From relay/core/pq_crypto.rs: the relay rebuilds the desktop app's DM
//! words byte for byte (`dm_pq::build_signed_inner`).

use humanity_protocol::pq_crypto::*;
use humanity_protocol::pq_crypto::fixtures::key_hex_of;

/// THE RELAY REBUILDS THE DESKTOP APP'S DM WORDS BYTE FOR BYTE: a payload made by the native
/// builder itself (net::dm_pq `build_signed_inner`, padding and all) checks with the relay's
/// verifier. Native builds only: the relay build has no net::dm_pq.
/// Seen red 2026-10-09 with `DM_INNER_DOMAIN` set to "hum/dm/v1": "the desktop app's DM
/// checks at the relay" failed.
#[cfg(feature = "native")]
#[test]
fn the_relay_rebuilds_the_desktop_apps_dm_words_byte_for_byte() {
    let seed = [0x61u8; 32];
    let from = key_hex_of(0x61);
    let to = key_hex_of(0x62);
    let inner = crate::net::dm_pq::build_signed_inner(&seed, &from, &to, 1_760_000_000_123, "line one\nline two").unwrap();
    let v: serde_json::Value = serde_json::from_str(&inner).unwrap();
    let (ts, text, sig) = (v["ts"].as_u64().unwrap(), v["text"].as_str().unwrap(), v["sig"].as_str().unwrap());
    assert!(verify_dm_inner(&from, &to, ts, text, sig), "the desktop app's DM checks at the relay");
}
