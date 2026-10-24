//! Tests for the per-address limits (ip_limit.rs) and the edge that applies them.

use super::*;

fn at(base: Instant, secs: f64) -> Instant {
    base + Duration::from_secs_f64(secs)
}

/// A FLOOD FROM ONE ADDRESS IS REFUSED, AND KEEPS WAITING LONGER WHILE IT KEEPS ASKING: the bot
/// class allows a burst of 20, then answers Wait. Asking again during each hold moves along the
/// Fibonacci delays (1, 2, 3, 5, 8, 13, 21, then 21 again); an address that waits as told is let
/// through without being pushed further; a quiet minute starts the sequence again. One that asks
/// again inside its hold, after tokens have refilled, is still refused.
///
/// Seen red 2026-10-22 with the hold ignored in `take` (`held = false`): "asked again inside its
/// hold, it went through / left: Go / right: Wait(21)".
#[test]
fn a_flood_is_refused_and_waits_longer_while_it_keeps_asking() {
    let limits = IpLimits::default();
    let t0 = Instant::now();
    for i in 0..20 {
        assert_eq!(limits.take(Class::Bot, "203.0.113.7", false, t0), Verdict::Go, "request {i} of the burst");
    }
    assert_eq!(limits.take(Class::Bot, "203.0.113.7", false, t0), Verdict::Wait(1), "the 21st waits a second");
    let mut waits = Vec::new();
    for _ in 0..8 {
        if let Verdict::Wait(w) = limits.take(Class::Bot, "203.0.113.7", false, t0) {
            waits.push(w);
        }
    }
    assert_eq!(waits, vec![1, 2, 3, 5, 8, 13, 21, 21], "asking during the hold waits longer");
    assert_eq!(limits.take(Class::Bot, "198.51.100.4", false, t0), Verdict::Go, "another address has its own allowance");

    // Not waiting as told: five seconds later five tokens have refilled, but the address is
    // still inside its 21-second hold, so it is refused and held longer.
    assert_eq!(limits.take(Class::Bot, "203.0.113.7", false, at(t0, 5.0)), Verdict::Wait(21), "asked again inside its hold, it went through");

    // Waiting as told: the hold is over and a token has refilled (one a second for this class).
    let after = at(t0, 26.5);
    assert_eq!(limits.take(Class::Bot, "203.0.113.7", false, after), Verdict::Go, "an address that waited is let through");
    // A quiet minute later the sequence starts again from one second.
    let calm = at(t0, 200.0);
    for _ in 0..20 {
        assert_eq!(limits.take(Class::Bot, "203.0.113.7", false, calm), Verdict::Go);
    }
    assert_eq!(limits.take(Class::Bot, "203.0.113.7", false, calm), Verdict::Wait(1), "the sequence started again");
}

/// THE STEADY RATE: past the burst, the bot class lets one request a second through, so a
/// well-behaved bot posting at its allowance is never refused.
#[test]
fn past_the_burst_the_steady_rate_goes_through() {
    let limits = IpLimits::default();
    let t0 = Instant::now();
    for _ in 0..20 {
        limits.take(Class::Bot, "a", false, t0);
    }
    for s in 1..=30 {
        assert_eq!(limits.take(Class::Bot, "a", false, at(t0, s as f64 * 1.01)), Verdict::Go, "one a second, at second {s}");
    }
}

/// THE TABLE IS BOUNDED: with room for 100 addresses, 1,000 different addresses leave at most
/// 100 entries plus one shared overflow bucket per class, and the newcomers that did not fit
/// share that bucket's allowance (so a flood of addresses is limited together, never let off).
/// Two minutes later the quiet ones are dropped and a new address gets an entry of its own.
///
/// Seen red 2026-10-22 with the cap check removed from `take`: "the table grew past its cap:
/// 1000".
#[test]
fn the_table_is_bounded_and_newcomers_share_one_bucket() {
    let limits = IpLimits::with_cap(100);
    let t0 = Instant::now();
    let mut refused = 0;
    for i in 0..1_000 {
        if let Verdict::Wait(_) = limits.take(Class::Bot, &format!("10.0.{}.{}", i / 256, i % 256), false, t0) {
            refused += 1;
        }
    }
    assert!(limits.tracked() <= 100 + Class::ALL.len(), "the table grew past its cap: {}", limits.tracked());
    // 100 addresses with their own burst of 20, then 900 sharing one burst of 20.
    assert_eq!(refused, 900 - 20, "the newcomers share one allowance");
    let later = at(t0, 130.0);
    assert_eq!(limits.take(Class::Bot, "192.0.2.77", false, later), Verdict::Go, "after the quiet ones go, a new address has room");
    assert!(limits.tracked() <= 3, "the sweep dropped the quiet addresses: {}", limits.tracked());
    let summary = limits.summary(later);
    assert_eq!(summary["refused"]["bot"], 880);
    assert!(summary["overflowed"].as_u64().unwrap() >= 900);
}

/// THE PROXY'S OWN ADDRESS (nginx without the header that names its client, or a local bot)
/// gets ten times the class's allowance, since everyone it carries shares it.
#[test]
fn the_proxys_own_address_has_ten_times_the_allowance() {
    let limits = IpLimits::default();
    let t0 = Instant::now();
    for i in 0..200 {
        assert_eq!(limits.take(Class::Bot, "127.0.0.1", true, t0), Verdict::Go, "request {i}");
    }
    assert!(matches!(limits.take(Class::Bot, "127.0.0.1", true, t0), Verdict::Wait(_)));
}

/// EVERY ROUTE HAS A CLASS, AND NO API ROUTE FALLS INTO THE LOOSEST ONE: every `.route(` in
/// mod.rs `build_router` is read with each method it names and sorted; the bot API's send and
/// the webhook are Bot; nothing under /api/ is Static. Run with --nocapture to print the audit
/// (the table in the 2026-10-22 report came from it).
#[test]
fn every_route_has_a_class_and_no_api_route_is_static() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/relay/mod.rs")).unwrap();
    let body = &src[src.find("pub fn build_router").expect("build_router")..];
    let mut seen = 0;
    for chunk in body.split(".route(").skip(1) {
        // A route written over several lines has its path on the next line.
        let Some(chunk) = chunk.trim_start().strip_prefix('"') else { continue };
        let Some((path, rest)) = chunk.split_once('"') else { continue };
        let call = rest.split("\n        .").next().unwrap_or(rest);
        for (word, method) in [("get(", Method::GET), ("post(", Method::POST), ("put(", Method::PUT), ("patch(", Method::PATCH), ("delete(", Method::DELETE)] {
            let named = call.contains(&format!(" {word}")) || call.contains(&format!(".{word}")) || call.contains(&format!(",{word}")) || call.contains(&format!("({word}"));
            if !named {
                continue;
            }
            let class = Class::of(&method, path);
            println!("{:<7} {:<55} {}", method.as_str(), path, class.key());
            seen += 1;
            if path.starts_with("/api/") {
                assert_ne!(class, Class::Static, "{method} {path} fell into the static class");
            }
        }
    }
    assert!(seen > 120, "read only {seen} routes: the scan lost its way");
    assert_eq!(Class::of(&Method::POST, "/api/send"), Class::Bot);
    assert_eq!(Class::of(&Method::POST, "/api/github-webhook"), Class::Bot);
    assert_eq!(Class::of(&Method::PATCH, "/api/tasks/7"), Class::Bot);
    assert_eq!(Class::of(&Method::GET, "/api/tasks"), Class::Read);
    assert_eq!(Class::of(&Method::GET, "/api/v2/solana/balance/x"), Class::Costly);
    assert_eq!(Class::of(&Method::POST, "/api/v2/zk/verify"), Class::Costly);
    assert_eq!(Class::of(&Method::PATCH, "/api/upload/resumable/ab"), Class::Upload);
    assert_eq!(Class::of(&Method::PUT, "/api/backup-host/o/x/y"), Class::Upload, "another server's backup pieces");
    assert_eq!(Class::of(&Method::GET, "/ws/live/sub/x"), Class::Socket);
    assert_eq!(Class::of(&Method::GET, "/uploads/a.png"), Class::Static);
    assert_eq!(Class::of(&Method::GET, "/files/12"), Class::Read);
}

// ── Through a real relay on a real socket ──

/// One relay on loopback, served the way production serves it (`serve_relay`).
async fn relay() -> (Arc<RelayState>, String) {
    let dir = std::env::temp_dir().join(format!("hos_ip_limit_{}_{}", std::process::id(), rand::random::<u32>()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = crate::relay::storage::Storage::open(&dir.join("relay.db")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (crate::relay::serve_relay(db, listener).await, format!("http://127.0.0.1:{port}"))
}

/// POST /api/send as `real_ip` (or with no header); the status code and Retry-After.
async fn send(client: &reqwest::Client, base: &str, real_ip: Option<&str>) -> (u16, Option<String>) {
    let mut req = client.post(format!("{base}/api/send")).json(&serde_json::json!({ "from_name": "Bot", "content": "hi" }));
    if let Some(ip) = real_ip {
        req = req.header("x-real-ip", ip);
    }
    let res = req.send().await.expect("the relay answers");
    let retry = res.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(str::to_string);
    (res.status().as_u16(), retry)
}

/// /API/SEND IS LIMITED PER ADDRESS, AND A SPOOFED X-REAL-IP FROM A CONNECTION THAT IS NOT A
/// PROXY IS IGNORED. Through nginx on loopback (the default trust), 203.0.113.7 gets its burst
/// of 20 answers (refused by the handler here: no API_SECRET is set, and the limit comes first), then 429 with a
/// Retry-After; 198.51.100.4 has an allowance of its own. With no proxy trusted, the same
/// connection writing a new X-Real-IP on every request is still one address, refused after 20.
///
/// Seen red 2026-10-22 with headers believed from any connection (the `trusts(peer)` check taken
/// out of client_addr.rs `client_ip`, which is how every route behaved before): "a new X-Real-IP
/// on every request was believed / left: 0 / right: 5".
#[tokio::test]
async fn api_send_is_limited_per_address_and_a_spoofed_header_is_ignored() {
    let (state, base) = relay().await;
    let client = reqwest::Client::new();
    for i in 0..20 {
        assert_ne!(send(&client, &base, Some("203.0.113.7")).await.0, 429, "request {i} reaches the handler");
    }
    let (code, retry) = send(&client, &base, Some("203.0.113.7")).await;
    assert_eq!(code, 429, "the 21st request from one address");
    assert_eq!(retry.as_deref(), Some("1"), "with a Retry-After");
    assert_ne!(send(&client, &base, Some("198.51.100.4")).await.0, 429, "another address has its own allowance");

    let (state2, base2) = relay().await;
    state2.ip_limits.set_trusted(TrustedProxies::none());
    let mut codes = Vec::new();
    for i in 0..25 {
        codes.push(send(&client, &base2, Some(&format!("203.0.113.{i}"))).await.0);
    }
    assert_eq!(codes.iter().filter(|c| **c == 429).count(), 5, "a new X-Real-IP on every request was believed: {codes:?}");
    drop(state);
}

/// THE EDGE CHARGES THE ADDRESS THE LAST PROXY WROTE: through nginx on loopback with
/// `X-Forwarded-For: 6.6.6.6, 203.0.113.7` (the client wrote the first, nginx appended the second),
/// the request counts against 203.0.113.7, never against the client's own first entry.
///
/// Seen red 2026-10-22 with client_addr.rs reading X-Forwarded-For from the left (the old
/// ws_handler reading): "left: [(Read, \"6.6.6.6\")] / right: [(Read, \"203.0.113.7\")]".
#[tokio::test]
async fn the_edge_charges_the_address_the_last_proxy_wrote() {
    let (state, base) = relay().await;
    let client = reqwest::Client::new();
    // Any route will do to reach the edge; /health answers 200.
    let res = client.get(format!("{base}/health")).header("x-forwarded-for", "6.6.6.6, 203.0.113.7").send().await.unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let table = state.ip_limits.table.lock().unwrap();
    let keys: Vec<_> = table.entries.keys().cloned().collect();
    assert_eq!(keys, vec![(Class::Read, "203.0.113.7".to_string())]);
}

/// Sign up the key of `[seed; 32]` on the relay at `base` through /ws, writing
/// `X-Forwarded-For: forwarded`; true when the relay refused it as one new account too many from
/// that address (handlers/sign_ups.rs `NEW_IDENTITY_CAP_SENTENCE`).
async fn sign_up_refused(base: &str, seed: u8, forwarded: &str) -> bool {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WsMsg};
    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&crate::relay::core::pq_crypto::derive_dilithium_seed(&[seed; 32]));
    let pubkey = hex::encode(dil.public_key());
    let mut request = format!("{}/ws", base.replacen("http", "ws", 1)).into_client_request().unwrap();
    request.headers_mut().insert("x-forwarded-for", forwarded.parse().unwrap());
    let (mut sock, _) = tokio_tungstenite::connect_async(request).await.expect("connects to /ws");
    let identify = serde_json::json!({ "type": "identify", "public_key": pubkey, "display_name": format!("Newcomer{seed}") });
    sock.send(WsMsg::Text(identify.to_string().into())).await.unwrap();
    let challenge: serde_json::Value = serde_json::from_str(&sock.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    let nonce = challenge["nonce"].as_str().expect("a challenge").to_string();
    let sig = B64.encode(dil.sign(format!("hum/identify/v1\n{nonce}\n{pubkey}").as_bytes()));
    sock.send(WsMsg::Text(serde_json::json!({ "type": "identify_response", "sig_b64": sig }).to_string().into())).await.unwrap();
    let refused = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(msg)) = sock.next().await {
            let Ok(text) = msg.into_text() else { continue };
            if text.contains(crate::relay::handlers::sign_ups::NEW_IDENTITY_CAP_SENTENCE) {
                return true;
            }
            if text.contains("\"peer_list\"") || text.contains("\"channel_list\"") {
                return false;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    let _ = sock.close(None).await;
    refused
}

/// THE WEBSOCKET'S PER-ADDRESS LIMITS READ THE ADDRESS THE EDGE DECIDED. The relay lets one
/// address sign up five new accounts an hour. Before 2026-10-22 ws_handler took the FIRST
/// `X-Forwarded-For` entry, the one a client writes, so a client writing a new one on every
/// connection was a new address each time. Now: from a connection that is not a trusted proxy,
/// six sign-ups each writing a different `X-Forwarded-For` are one address, and the sixth is
/// refused; through nginx on loopback, the six different addresses nginx names are six people.
///
/// Seen red 2026-10-22 with headers believed from any connection (the `trusts(peer)` check taken
/// out of client_addr.rs `client_ip`): "a written X-Forwarded-For escaped the new-account cap /
/// left: [false, false, false, false, false, false]".
#[tokio::test]
async fn the_websockets_limits_read_the_address_the_edge_decided() {
    let (direct, base) = relay().await;
    direct.ip_limits.set_trusted(TrustedProxies::none());
    let mut refused = Vec::new();
    for i in 0..6u8 {
        refused.push(sign_up_refused(&base, 100 + i, &format!("203.0.113.{i}")).await);
    }
    assert_eq!(refused, vec![false, false, false, false, false, true], "a written X-Forwarded-For escaped the new-account cap");

    let (_proxied, base) = relay().await;
    let mut refused = Vec::new();
    for i in 0..6u8 {
        refused.push(sign_up_refused(&base, 110 + i, &format!("203.0.113.{i}")).await);
    }
    assert_eq!(refused, vec![false; 6], "through nginx, six addresses are six people");
}
