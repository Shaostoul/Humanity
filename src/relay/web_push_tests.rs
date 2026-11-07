// Child of web_push.rs (#[path], so `use super::*` reaches everything it has): our Web Push
// against RFC 8291's own worked example, a browser's half of the decryption, the VAPID token
// checked with the public key as a push service checks it, and one push carried end to end from
// a relay state's subscription list to a push service on 127.0.0.1.
//
// RED FIRST (2026-11-06), each gate seen failing on a plausible wrong implementation before it
// was trusted (one run, all three mutations in, `cargo test --features relay
// --no-default-features --lib web_push`: 4 passed, these 3 failed):
// - `rfc8291_example_byte_for_byte`, with the two public keys in `record_keys`' key_info swapped
//   (as || ua): "IKM differs from RFC 8291 appendix A, or the CEK does", the CEK starting
//   [194, 2, 74, ...] where the RFC's starts [160, 136, 85, ...]. (The round trip in
//   `a_browser_opens_what_we_send_and_nothing_else` still passed, both halves sharing the
//   mistake: only the RFC's own numbers catch it.)
// - `the_vapid_token_verifies_with_the_public_key`, with the signature written DER-encoded (the
//   classic ES256 mistake; a push service answers 401/403 and the push is silently lost):
//   "an ES256 signature is 64 octets r || s, this one is 71" (DER lengths vary, 70-72).
// - `a_push_reaches_the_push_service_end_to_end`, with the Content-Encoding header left out:
//   "Content-Encoding: None" (left: None, right: Some("aes128gcm")).
//
// THE TEST SEAM (2026-11-06, BUG-255): pushes now go only to https on a public name, so the two
// end-to-end tests reach their stand-in service (plain http on 127.0.0.1) through
// `OutboundGuard::allowing_test_service(port)`, which exists only in test builds. Seen red without it,
// the guard in place: "the push service heard nothing" and "the 410 subscription was not removed".

use super::*;
use crate::relay::outbound::OutboundGuard;
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::VerifyingKey;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn b64(s: &str) -> Vec<u8> {
    Base64UrlUnpadded::decode_vec(s).expect("base64url")
}

fn b64e(bytes: &[u8]) -> String {
    Base64UrlUnpadded::encode_string(bytes)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

/// RFC 8291 section 5 and appendix A, copied from https://www.rfc-editor.org/rfc/rfc8291.txt
/// (read 2026-11-06), the RFC's line wraps joined.
mod rfc8291 {
    pub const PLAINTEXT: &[u8] = b"When I grow up, I want to be a watermelon";
    pub const AS_PRIVATE: &str = "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw";
    pub const AS_PUBLIC: &str = "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8";
    pub const UA_PRIVATE: &str = "q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94";
    pub const UA_PUBLIC: &str = "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
    pub const SALT: &str = "DGv6ra1nlYgDCS1FRnbzlw";
    pub const AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
    pub const ECDH_SECRET: &str = "kyrL1jIIOHEzg3sM2ZWRHDRB62YACZhhSlknJ672kSs";
    pub const IKM: &str = "S4lYMb_L0FxCeq0WhDx813KgSYqU26kOyzWUdsXYyrg";
    pub const CEK: &str = "oIhVW04MRdy2XN9CiKLxTg";
    pub const NONCE: &str = "4h_95klXJ5E_qnoN";
    pub const BODY: &str = "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN";
}

/// The browser's half (RFC 8291 section 3.4 and RFC 8188, the receiving side): the body opened
/// with the subscriber's private key and auth secret.
fn browser_decrypt(body: &[u8], ua_secret: &SecretKey, auth: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    if body.len() < 21 {
        return Err("shorter than the header".into());
    }
    let salt: [u8; 16] = body[..16].try_into().unwrap();
    let rs = u32::from_be_bytes(body[16..20].try_into().unwrap()) as usize;
    let idlen = body[20] as usize;
    let keyid = body.get(21..21 + idlen).ok_or("key id runs past the body")?;
    let sealed = &body[21 + idlen..];
    if sealed.len() > rs {
        return Err(format!("one record of {} octets, the record size says {rs}", sealed.len()));
    }
    let as_public = PublicKey::from_sec1_bytes(keyid).map_err(|_| "key id is not a P-256 key")?;
    let ua_public = ua_secret.public_key();
    let shared = ua_secret.diffie_hellman(&as_public);
    let (cek, nonce) = record_keys(shared.raw_secret_bytes().as_slice(), auth, &ua_public, &as_public, &salt)?;
    let plain = aes_gcm::Aes128Gcm::new_from_slice(&cek)
        .unwrap()
        .decrypt(aes_gcm::Nonce::from_slice(&nonce), sealed)
        .map_err(|_| "the record does not open (wrong key, or altered)")?;
    // Padding: trailing zero octets, then the last record's delimiter 0x02.
    let end = plain.iter().rposition(|&b| b != 0).ok_or("no padding delimiter")?;
    if plain[end] != 0x02 {
        return Err(format!("the delimiter is {:#04x}, the last record's is 0x02", plain[end]));
    }
    Ok(plain[..end].to_vec())
}

/// A push service's check of the `Authorization` header (RFC 8292 section 3): the token's
/// signature under the `k=` key, and its header. Returns the claims and the key.
fn check_vapid(value: &str) -> Result<(serde_json::Value, Vec<u8>), String> {
    let rest = value.strip_prefix("vapid ").ok_or("not the vapid scheme")?;
    let mut token = None;
    let mut key = None;
    for part in rest.split(',').map(str::trim) {
        if let Some(t) = part.strip_prefix("t=") {
            token = Some(t);
        } else if let Some(k) = part.strip_prefix("k=") {
            key = Some(b64(k));
        }
    }
    let (token, key) = (token.ok_or("no t=")?, key.ok_or("no k=")?);
    let pieces: Vec<&str> = token.split('.').collect();
    if pieces.len() != 3 {
        return Err(format!("a JWT has 3 parts, this one {}", pieces.len()));
    }
    let header: serde_json::Value = serde_json::from_slice(&b64(pieces[0])).map_err(|e| e.to_string())?;
    if header["alg"] != "ES256" || header["typ"] != "JWT" {
        return Err(format!("header {header}"));
    }
    let sig_bytes = b64(pieces[2]);
    if sig_bytes.len() != 64 {
        return Err(format!("an ES256 signature is 64 octets r || s, this one is {}", sig_bytes.len()));
    }
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| "not an ECDSA signature")?;
    let vk = VerifyingKey::from_sec1_bytes(&key).map_err(|_| "k= is not a P-256 key")?;
    vk.verify(format!("{}.{}", pieces[0], pieces[1]).as_bytes(), &sig).map_err(|_| "the signature does not verify")?;
    let claims: serde_json::Value = serde_json::from_slice(&b64(pieces[1])).map_err(|e| e.to_string())?;
    Ok((claims, key))
}

#[test]
fn rfc8291_example_byte_for_byte() {
    use rfc8291::*;
    let as_secret = SecretKey::from_slice(&b64(AS_PRIVATE)).unwrap();
    assert_eq!(as_secret.public_key().to_sec1_point(false).as_bytes(), b64(AS_PUBLIC).as_slice());
    let ua_public = PublicKey::from_sec1_bytes(&b64(UA_PUBLIC)).unwrap();
    let shared = as_secret.diffie_hellman(&ua_public);
    assert_eq!(shared.raw_secret_bytes().as_slice(), b64(ECDH_SECRET).as_slice(), "ecdh_secret");

    // IKM is internal to record_keys; check it the way the RFC states it.
    let mut key_info = WEBPUSH_INFO.to_vec();
    key_info.extend_from_slice(&b64(UA_PUBLIC));
    key_info.extend_from_slice(&b64(AS_PUBLIC));
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&b64(AUTH)), &b64(ECDH_SECRET)).expand(&key_info, &mut ikm).unwrap();
    assert_eq!(ikm.as_slice(), b64(IKM).as_slice(), "IKM");

    let salt: [u8; 16] = b64(SALT).try_into().unwrap();
    let (cek, nonce) = record_keys(&b64(ECDH_SECRET), &b64(AUTH), &ua_public, &as_secret.public_key(), &salt).unwrap();
    assert_eq!(cek.as_slice(), b64(CEK).as_slice(), "IKM differs from RFC 8291 appendix A, or the CEK does");
    assert_eq!(nonce.as_slice(), b64(NONCE).as_slice(), "NONCE");

    let body = encrypt_with(PLAINTEXT, &b64(UA_PUBLIC), &b64(AUTH), &salt, &as_secret).unwrap();
    assert_eq!(b64e(&body), BODY, "the whole message body, header and record, is the RFC's");

    // And the browser's half opens the RFC's own body with the RFC's user-agent key.
    let ua_secret = SecretKey::from_slice(&b64(UA_PRIVATE)).unwrap();
    assert_eq!(browser_decrypt(&b64(BODY), &ua_secret, &b64(AUTH)).unwrap(), PLAINTEXT);
}

#[test]
fn a_browser_opens_what_we_send_and_nothing_else() {
    let ua_secret = random_secret().unwrap();
    let ua_public = ua_secret.public_key().to_sec1_point(false).as_bytes().to_vec();
    let mut auth = [0u8; 16];
    getrandom::getrandom(&mut auth).unwrap();
    let message = br#"{"title":"New message","body":"You have a new message","tag":"dm","url":"/chat"}"#;

    let a = encrypt(message, &ua_public, &auth).unwrap();
    let b = encrypt(message, &ua_public, &auth).unwrap();
    assert_ne!(a, b, "a fresh salt and one-off key every time");
    assert_eq!(browser_decrypt(&a, &ua_secret, &auth).unwrap(), message);
    assert_eq!(a.len(), HEADER_LEN + message.len() + 1 + 16);

    // Someone else's key or auth secret opens nothing, and neither does an altered octet.
    let other = random_secret().unwrap();
    assert!(browser_decrypt(&a, &other, &auth).is_err());
    let mut wrong_auth = auth;
    wrong_auth[0] ^= 1;
    assert!(browser_decrypt(&a, &ua_secret, &wrong_auth).is_err());
    let mut altered = a.clone();
    *altered.last_mut().unwrap() ^= 1;
    assert!(browser_decrypt(&altered, &ua_secret, &auth).is_err());

    // The longest message makes a 4096-octet body, the size every push service takes; one more
    // octet is refused here rather than by the push service.
    let longest = vec![b'x'; MAX_PLAINTEXT];
    assert_eq!(encrypt(&longest, &ua_public, &auth).unwrap().len(), 4096);
    assert!(encrypt(&vec![b'x'; MAX_PLAINTEXT + 1], &ua_public, &auth).is_err());
    // A malformed subscription is refused, never sent.
    assert!(encrypt(message, &ua_public, &auth[..15]).is_err());
    assert!(encrypt(message, &ua_public[..64], &auth).is_err());
}

#[test]
fn the_vapid_token_verifies_with_the_public_key() {
    let key = VapidKey::generate().unwrap();
    let now = 1_800_000_000;
    let value = key
        .authorization("https://fcm.googleapis.com/fcm/send/abc123:XYZ", "mailto:admin@united-humanity.us", now)
        .unwrap();
    let (claims, k) = check_vapid(&value).unwrap();
    assert_eq!(k, key.public_key_uncompressed(), "k= is the key /api/vapid-public-key hands out");
    assert_eq!(k.len(), 65);
    assert_eq!(k[0], 0x04, "uncompressed");
    assert_eq!(claims["aud"], "https://fcm.googleapis.com", "the audience is the endpoint's origin, no path");
    assert_eq!(claims["sub"], "mailto:admin@united-humanity.us");
    let exp = claims["exp"].as_u64().unwrap();
    assert_eq!(exp, now + VALID_SECS);
    assert!(exp - now <= 24 * 60 * 60, "RFC 8292: no more than 24 hours ahead");

    // A non-default port is part of the origin (RFC 6454), as a push service compares it.
    let (claims, _) = check_vapid(&key.authorization("https://push.example:8443/x", "mailto:a@b", now).unwrap()).unwrap();
    assert_eq!(claims["aud"], "https://push.example:8443");

    // The check can fail: another key's k=, or claims altered after signing, are refused.
    let other = VapidKey::generate().unwrap();
    let forged = value.replace(
        &format!("k={}", b64e(&key.public_key_uncompressed())),
        &format!("k={}", b64e(&other.public_key_uncompressed())),
    );
    assert_ne!(forged, value);
    assert_eq!(check_vapid(&forged).unwrap_err(), "the signature does not verify");
    let t = value.strip_prefix("vapid t=").unwrap().split(',').next().unwrap();
    let pieces: Vec<&str> = t.split('.').collect();
    let longer = b64e(serde_json::json!({ "aud": "https://fcm.googleapis.com", "exp": now + 999_999, "sub": "mailto:admin@united-humanity.us" }).to_string().as_bytes());
    let altered = value.replace(pieces[1], &longer);
    assert_eq!(check_vapid(&altered).unwrap_err(), "the signature does not verify");
    assert!(key.authorization("not a url", "mailto:a@b", now).is_err());
}

#[test]
fn the_key_file_holds_the_raw_scalar() {
    // The format jwt-simple wrote: the 32-byte secret scalar. RFC 8291's application-server key
    // is a P-256 scalar too, so its known public key proves the reading.
    let key = VapidKey::from_bytes(&b64(rfc8291::AS_PRIVATE)).unwrap();
    assert_eq!(key.public_key_uncompressed(), b64(rfc8291::AS_PUBLIC));
    assert_eq!(key.to_bytes().as_slice(), b64(rfc8291::AS_PRIVATE).as_slice());
    let fresh = VapidKey::generate().unwrap();
    assert_eq!(VapidKey::from_bytes(&fresh.to_bytes()).unwrap().public_key_uncompressed(), fresh.public_key_uncompressed());
    assert!(VapidKey::from_bytes(&[7u8; 31]).is_err(), "short");
    assert!(VapidKey::from_bytes(&[0u8; 32]).is_err(), "zero is no key");
    assert!(VapidKey::from_bytes(&[0xffu8; 32]).is_err(), "past the curve's order");
}

#[test]
fn the_key_is_made_once_and_kept() {
    let dir = std::env::temp_dir().join(format!("hos-vapid-{}-{}", std::process::id(), now_unix()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("vapid_private.key");

    let made = load_or_create_key(&path).expect("made");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.len(), 43, "base64url of 32 bytes, no padding: {text}");
    let again = load_or_create_key(&path).expect("loaded");
    assert_eq!(again.public_key_uncompressed(), made.public_key_uncompressed(), "the same key on the next start");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600, "private to the server's user");
    }

    // A file holding no key is replaced (and the replacement is a real key).
    std::fs::write(&path, "not a key").unwrap();
    let replaced = load_or_create_key(&path).expect("replaced");
    assert_ne!(replaced.public_key_uncompressed(), made.public_key_uncompressed());
    assert_eq!(load_or_create_key(&path).unwrap().public_key_uncompressed(), replaced.public_key_uncompressed());
    assert!(!dir.join("vapid_private.key.new").exists(), "no temporary file left behind");
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the stand-in push service saw.
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

/// A push service on 127.0.0.1 that records every request; a path ending in /gone answers 410,
/// the way a push service says a subscription has expired.
async fn push_service() -> (u16, Arc<Mutex<Vec<Seen>>>) {
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let app = axum::Router::new().fallback(
        move |method: axum::http::Method, uri: axum::http::Uri, headers: axum::http::HeaderMap, body: axum::body::Bytes| {
            let log = log.clone();
            async move {
                let gone = uri.path().ends_with("/gone");
                log.lock().unwrap().push(Seen { method: method.to_string(), path: uri.path().to_string(), headers, body: body.to_vec() });
                if gone { axum::http::StatusCode::GONE } else { axum::http::StatusCode::CREATED }
            }
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (port, seen)
}

/// Waits (a bounded number of short sleeps) until `done` holds; false if it never does.
async fn until(mut done: impl FnMut() -> bool) -> bool {
    for _ in 0..400 {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    done()
}

#[test]
fn a_push_reaches_the_push_service_end_to_end() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let (port, seen) = push_service().await;
        let mut state = RelayState::new(Storage::open_temp("web_push_e2e"));
        state.vapid_key = Some(VapidKey::generate().unwrap());
        // The stand-in service is plain http on loopback, which the relay refuses (BUG-255); the
        // test-only seam lets exactly its address through (outbound.rs, THE TEST SEAM).
        state.outbound = OutboundGuard::default().allowing_test_service(port);
        let state = Arc::new(state);

        // A browser's subscription: its own key pair and auth secret, an endpoint at the service.
        let ua_secret = random_secret().unwrap();
        let mut auth = [0u8; 16];
        getrandom::getrandom(&mut auth).unwrap();
        let endpoint = format!("http://127.0.0.1:{port}/wpush/v2/abc");
        state
            .db
            .save_push_subscription("alice-key", &endpoint, &b64e(ua_secret.public_key().to_sec1_point(false).as_bytes()), &b64e(&auth))
            .unwrap();

        state.send_push_notification("alice-key", "New message", "You have a new message", "dm", "/chat");
        assert!(until(|| !seen.lock().unwrap().is_empty()).await, "the push service heard nothing");

        let got = seen.lock().unwrap()[0].clone();
        assert_eq!(got.method, "POST");
        assert_eq!(got.path, "/wpush/v2/abc");
        let header = |name: &str| got.headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        assert_eq!(header("content-encoding").as_deref(), Some("aes128gcm"), "Content-Encoding: {:?}", header("content-encoding"));
        assert_eq!(header("ttl").as_deref(), Some("43200"));
        assert_eq!(header("content-type").as_deref(), Some("application/octet-stream"));

        // The token verifies under the key the relay hands browsers, for this service's origin.
        let (claims, k) = check_vapid(&header("authorization").expect("an Authorization header")).unwrap();
        assert_eq!(k, state.vapid_key.as_ref().unwrap().public_key_uncompressed());
        assert_eq!(claims["aud"], format!("http://127.0.0.1:{port}"));
        assert_eq!(claims["sub"], "mailto:admin@united-humanity.us");

        // And the browser opens the notice.
        let notice: serde_json::Value = serde_json::from_slice(&browser_decrypt(&got.body, &ua_secret, &auth).unwrap()).unwrap();
        assert_eq!(notice, serde_json::json!({ "title": "New message", "body": "You have a new message", "tag": "dm", "url": "/chat" }));
    });
}

#[test]
fn an_expired_subscription_is_forgotten() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let (port, seen) = push_service().await;
        let mut state = RelayState::new(Storage::open_temp("web_push_gone"));
        state.vapid_key = Some(VapidKey::generate().unwrap());
        state.outbound = OutboundGuard::default().allowing_test_service(port);
        let state = Arc::new(state);
        let ua = b64e(random_secret().unwrap().public_key().to_sec1_point(false).as_bytes());
        let auth = b64e(&[9u8; 16]);
        let kept = format!("http://127.0.0.1:{port}/push/kept");
        let gone = format!("http://127.0.0.1:{port}/push/gone");
        state.db.save_push_subscription("bob-key", &kept, &ua, &auth).unwrap();
        state.db.save_push_subscription("bob-key", &gone, &ua, &auth).unwrap();
        assert_eq!(state.db.get_push_subscriptions("bob-key").len(), 2);

        state.send_push_notification("bob-key", "t", "b", "dm", "/chat");
        let only_kept = || {
            let subs = state.db.get_push_subscriptions("bob-key");
            subs.len() == 1 && subs[0].endpoint == kept
        };
        assert!(until(only_kept).await, "the 410 subscription was not removed");
        assert_eq!(seen.lock().unwrap().len(), 2, "both were sent");
    });
}
