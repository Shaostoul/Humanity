// Child of push_routes.rs: the subscribe and unsubscribe routes, called the way axum calls them,
// with requests signed the way the website signs them (Dilithium3 over `purpose\ntimestamp`).
//
// RED FIRST (2026-11-06, BUG-255). Each gate was seen failing before it was trusted, in one run of
// `cargo test --features relay --no-default-features --lib push_` with the faults put in at once:
// the endpoint guard taken out (outbound_tests.rs' header says how), `subscribe_purpose`
// leaving the endpoint out of the words signed, and `push_unsubscribe` removing by endpoint with no
// signature checked (the route as it was). What each test said then is at the test.

use super::*;
use crate::relay::outbound::OutboundGuard;
use crate::relay::storage::Storage;
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use std::net::IpAddr;

/// RFC 8291 appendix A's user agent: a real P-256 key and auth secret, as a browser gives them.
const P256DH: &str = "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
const GOOD: &str = "https://push.example.com/wpush/v2/abc";

fn person(b: u8) -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&derive_dilithium_seed(&[b; 32]))
}

fn key_of(p: &DilithiumKeypair) -> String {
    hex::encode(p.public_key())
}

fn sign(p: &DilithiumKeypair, purpose: &str, ts: u64) -> String {
    hex::encode(p.sign(format!("{purpose}\n{ts}").as_bytes()))
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

/// A relay whose push names are fixed: push.example.com is public, the others lead where they say.
fn relay(tag: &str) -> Arc<RelayState> {
    let mut state = RelayState::new(Storage::open_temp(&format!("push_routes_{tag}")));
    state.outbound = OutboundGuard::with_names(&[
        ("push.example.com", ip("142.250.72.106")),
        ("other.example.com", ip("142.250.72.107")),
        ("loopback.example.com", ip("127.0.0.1")),
        ("internal.example.com", ip("10.1.2.3")),
        ("metadata.example.com", ip("169.254.169.254")),
        ("v6-loopback.example.com", ip("::1")),
        ("mixed.example.com", ip("142.250.72.106")),
        ("mixed.example.com", ip("192.168.0.10")),
    ]);
    Arc::new(state)
}

fn subscribe_body(p: &DilithiumKeypair, endpoint: &str, ts: u64) -> PushSubscribeRequest {
    PushSubscribeRequest {
        key: key_of(p),
        endpoint: endpoint.into(),
        p256dh: P256DH.into(),
        auth: AUTH.into(),
        timestamp: ts,
        sig: sign(p, &subscribe_purpose(endpoint, P256DH, AUTH), ts),
    }
}

async fn subscribe(st: &Arc<RelayState>, body: PushSubscribeRequest) -> (u16, Value) {
    match push_subscribe(State(st.clone()), Json(body)).await {
        Ok(Json(v)) => (200, v),
        Err((code, Json(v))) => (code.as_u16(), v),
    }
}

async fn unsubscribe(st: &Arc<RelayState>, body: PushUnsubscribeRequest) -> (u16, Value) {
    match push_unsubscribe(State(st.clone()), Json(body)).await {
        Ok(Json(v)) => (200, v),
        Err((code, Json(v))) => (code.as_u16(), v),
    }
}

fn kept(st: &RelayState, p: &DilithiumKeypair) -> Vec<String> {
    st.db.get_push_subscriptions(&key_of(p)).into_iter().map(|s| s.endpoint).collect()
}

/// A properly signed subscribe to loopback, the private networks, a cloud's metadata service,
/// IPv6 loopback and unique-local, an IPv4-mapped loopback, or plain http, is refused and nothing
/// is kept. Seen red with the guard taken out: "https://127.0.0.1/wpush was kept: (200, {\"status\":\"subscribed\"})".
#[tokio::test]
async fn a_subscribe_to_a_private_or_plain_address_is_refused() {
    let st = relay("private");
    let alice = person(0x51);
    let cases = [
        ("https://127.0.0.1/wpush", "address"),
        ("https://10.0.0.5/wpush", "address"),
        ("https://192.168.1.1/wpush", "address"),
        ("https://169.254.169.254/latest/meta-data/", "address"),
        ("https://[::1]/wpush", "address"),
        ("https://[fc00::1]/wpush", "address"),
        ("https://[::ffff:127.0.0.1]/wpush", "address"),
        ("http://push.example.com/wpush", "not_https"),
        ("https://localhost/wpush", "private"),
        ("https://user:pw@push.example.com/wpush", "userinfo"),
    ];
    for (endpoint, reason) in cases {
        let (code, v) = subscribe(&st, subscribe_body(&alice, endpoint, now_ms())).await;
        assert_eq!((code, v["reason"].as_str()), (400, Some(reason)), "{endpoint} was kept: ({code}, {v})");
        assert!(v["error"].as_str().is_some_and(|w| w.len() > 20), "plain words: {v}");
    }
    let long = format!("https://push.example.com/{}", "a".repeat(2048));
    let (code, v) = subscribe(&st, subscribe_body(&alice, &long, now_ms())).await;
    assert_eq!((code, v["reason"].as_str()), (400, Some("too_long")));
    assert!(kept(&st, &alice).is_empty(), "nothing was kept: {:?}", kept(&st, &alice));

    // Malformed keys are refused too (a push to them could never be built).
    let mut bad = subscribe_body(&alice, GOOD, now_ms());
    bad.p256dh = "AAAA".into();
    bad.sig = sign(&alice, &subscribe_purpose(GOOD, "AAAA", AUTH), bad.timestamp);
    assert_eq!(subscribe(&st, bad).await.1["reason"], "keys");

    // And a public one is kept.
    let (code, v) = subscribe(&st, subscribe_body(&alice, GOOD, now_ms())).await;
    assert_eq!(code, 200, "{v}");
    assert_eq!(kept(&st, &alice), vec![GOOD.to_string()]);
}

/// A public-looking name is looked up, and refused when any address it has is not public (the
/// test's resolver answers loopback, 10.x, 169.254.x, ::1, or a public address beside a private
/// one), or when it has none. Seen red with the guard taken out: "https://loopback.example.com/wpush
/// was kept: (200, {\"status\":\"subscribed\"})".
#[tokio::test]
async fn a_name_that_leads_to_a_private_address_is_refused() {
    let st = relay("names");
    let alice = person(0x52);
    for name in ["loopback", "internal", "metadata", "v6-loopback", "mixed"] {
        let endpoint = format!("https://{name}.example.com/wpush");
        let (code, v) = subscribe(&st, subscribe_body(&alice, &endpoint, now_ms())).await;
        assert_eq!((code, v["reason"].as_str()), (400, Some("private")), "{endpoint} was kept: ({code}, {v})");
    }
    let (code, v) = subscribe(&st, subscribe_body(&alice, "https://nowhere.example.com/wpush", now_ms())).await;
    assert_eq!((code, v["reason"].as_str()), (400, Some("unknown_name")), "{v}");
    assert!(kept(&st, &alice).is_empty());
}

/// The signature covers the endpoint and both keys: a request signed for one endpoint is refused
/// with another (or with other keys), a timestamp too far ahead is refused, and the same request
/// is accepted once only. Seen red with the endpoint left out of the words signed: "a signature
/// over another endpoint was accepted: (200, {\"status\":\"subscribed\"})".
#[tokio::test]
async fn a_signature_over_another_endpoint_is_refused() {
    let st = relay("swap");
    let alice = person(0x53);
    let ts = now_ms();
    let mut swapped = subscribe_body(&alice, GOOD, ts);
    swapped.endpoint = "https://other.example.com/attacker".into();
    let (code, v) = subscribe(&st, swapped).await;
    assert_eq!((code, v["reason"].as_str()), (401, Some("signature")), "a signature over another endpoint was accepted: ({code}, {v})");

    let mut rekeyed = subscribe_body(&alice, GOOD, ts);
    rekeyed.auth = "AAAAAAAAAAAAAAAAAAAAAA".into();
    assert_eq!(subscribe(&st, rekeyed).await.0, 401, "a signature over other keys was accepted");

    let mut unsigned = subscribe_body(&alice, GOOD, ts);
    unsigned.sig = String::new();
    assert_eq!(subscribe(&st, unsigned).await.0, 401, "an unsigned subscribe was accepted");
    assert!(kept(&st, &alice).is_empty());

    let ahead = now_ms() + 5 * 60 * 1000;
    let (code, v) = subscribe(&st, subscribe_body(&alice, GOOD, ahead)).await;
    assert_eq!((code, v["reason"].as_str()), (400, Some("clock")), "a timestamp from the future: {v}");

    let ts = now_ms();
    assert_eq!(subscribe(&st, subscribe_body(&alice, GOOD, ts)).await.0, 200);
    let (code, v) = subscribe(&st, subscribe_body(&alice, GOOD, ts)).await;
    assert_eq!((code, v["reason"].as_str()), (409, Some("replay")), "the same request twice: {v}");
}

/// Unsubscribe needs the owner's signature over the endpoint: unsigned, wrongly signed, signed over
/// another endpoint, or signed by someone else, the subscription stays; the owner's removes it. The
/// old unsigned shape `{endpoint}` no longer even parses. Seen red with the route as it was (remove
/// by endpoint, nothing checked): "an unsigned unsubscribe was accepted: (200, {\"status\":\"unsubscribed\"})".
#[tokio::test]
async fn unsubscribe_needs_the_owners_signature() {
    let st = relay("unsub");
    let (alice, bob) = (person(0x54), person(0x55));
    assert_eq!(subscribe(&st, subscribe_body(&alice, GOOD, now_ms())).await.0, 200);

    let body = |p: &DilithiumKeypair, signed_for: &str, ts: u64| PushUnsubscribeRequest {
        key: key_of(p),
        endpoint: GOOD.into(),
        timestamp: ts,
        sig: sign(p, &unsubscribe_purpose(signed_for), ts),
    };
    let mut unsigned = body(&alice, GOOD, now_ms());
    unsigned.sig = String::new();
    let (code, v) = unsubscribe(&st, unsigned).await;
    assert_eq!((code, v["reason"].as_str()), (401, Some("signature")), "an unsigned unsubscribe was accepted: ({code}, {v})");

    let mut forged = body(&alice, GOOD, now_ms());
    forged.sig = sign(&bob, &unsubscribe_purpose(GOOD), forged.timestamp);
    assert_eq!(unsubscribe(&st, forged).await.0, 401, "bob's signature under alice's key was accepted");

    let (code, _) = unsubscribe(&st, body(&alice, "https://other.example.com/x", now_ms())).await;
    assert_eq!(code, 401, "a signature over another endpoint was accepted");

    let (code, v) = unsubscribe(&st, body(&bob, GOOD, now_ms())).await;
    assert_eq!((code, v["reason"].as_str()), (404, Some("not_found")), "bob removed alice's subscription: {v}");
    assert_eq!(kept(&st, &alice), vec![GOOD.to_string()], "alice's subscription stays");

    assert!(serde_json::from_value::<PushUnsubscribeRequest>(json!({ "endpoint": GOOD })).is_err(), "the old unsigned shape parses");

    let (code, v) = unsubscribe(&st, body(&alice, GOOD, now_ms())).await;
    assert_eq!(code, 200, "the owner's unsubscribe: {v}");
    assert!(kept(&st, &alice).is_empty(), "removed");
}

/// The words signed are the fixture's, which the website's test holds its requests to. Seen red
/// with the endpoint left out: left "push_subscribe\nBCVxsr7N...\nBTBZMqHH6r4Tts7J_aSIgg\n1800000000000".
#[test]
fn the_signed_words_are_the_fixtures() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/push-signing-v1.json");
    let f: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (endpoint, p256dh, auth) = (f["endpoint"].as_str().unwrap(), f["p256dh"].as_str().unwrap(), f["auth"].as_str().unwrap());
    let ts = f["timestamp"].as_u64().unwrap();
    assert_eq!(format!("{}\n{ts}", subscribe_purpose(endpoint, p256dh, auth)), f["subscribe_preimage"].as_str().unwrap());
    assert_eq!(format!("{}\n{ts}", unsubscribe_purpose(endpoint)), f["unsubscribe_preimage"].as_str().unwrap());
    assert!(crate::relay::outbound::url_shape(endpoint, crate::relay::outbound::Schemes::HttpsOnly).is_ok(), "the fixture's endpoint is one a push may go to");
    assert!(keys_ok(p256dh, auth));
}
