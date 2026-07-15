//! Ephemeral TURN credentials (v0.857).
//!
//! Replaces the static TURN username/password that used to be committed in the
//! clients (`src/net/webrtc.rs` and `web/chat/chat-voice-rooms.js`). A committed
//! long-term credential lets anyone who reads the repo or the served JS use the
//! operator's TURN relay as free bandwidth, and rotating it meant editing code.
//!
//! Instead, this issues SHORT-LIVED credentials using coturn's REST-API scheme
//! (`use-auth-secret` / `static-auth-secret`), the standard documented in
//! draft-uberti-behave-turn-rest:
//!
//! ```text
//! username   = "<unix-expiry-timestamp>"           (or "<expiry>:<userid>")
//! credential = base64( HMAC-SHA1( secret, username ) )
//! ```
//!
//! coturn computes the same HMAC from its `static-auth-secret` and accepts the
//! credential until the expiry passes. The SECRET lives only on the server (the
//! relay's `TURN_STATIC_SECRET` env var, matching coturn's `static-auth-secret`)
//! and is never sent to a client, so there is nothing in the repo to leak and
//! rotation is a one-line secret change on the VPS.
//!
//! If `TURN_STATIC_SECRET` is unset, this returns STUN servers only (no TURN).
//! Voice still works for everyone except symmetric-NAT peers, exactly as it did
//! whenever a TURN allocation failed, so an unconfigured relay degrades cleanly
//! rather than breaking.

use axum::Json;
use hmac::{Hmac, Mac};
use sha1::Sha1;

/// How long an issued credential is valid. One hour is the common default: long
/// enough to start and hold a call, short enough that a leaked credential is
/// worthless within the hour.
const CREDENTIAL_TTL_SECS: u64 = 3600;

/// Build the ephemeral username/credential pair for `secret`, valid for `ttl`
/// seconds from `now`. Pulled out so it is unit-testable without the HTTP layer.
fn make_credential(secret: &str, now: u64, ttl: u64) -> (String, String) {
    let expiry = now + ttl;
    // coturn accepts "<expiry>" or "<expiry>:<userid>". We tag with "hos" purely
    // for readability in coturn's logs; it is not a secret and not verified.
    let username = format!("{expiry}:hos");
    let mut mac =
        Hmac::<Sha1>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(username.as_bytes());
    let credential =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, mac.finalize().into_bytes());
    (username, credential)
}

/// `GET /api/turn-credentials` — hand a client a fresh, short-lived ICE-server
/// list. Unauthenticated on purpose: the credentials are time-boxed and only
/// grant TURN relaying, the same access the old static credential granted to
/// anyone who opened the page. Rate-limiting is nginx's job if it becomes a
/// concern; issuing a credential is a cheap HMAC.
pub async fn turn_credentials() -> Json<serde_json::Value> {
    // The TURN/STUN host. Defaults to the production relay; override with
    // TURN_SERVER_HOST for a self-hoster on a different domain.
    let host = std::env::var("TURN_SERVER_HOST").unwrap_or_else(|_| "united-humanity.us".to_string());

    // Public Google STUN is always offered as the cheap first resort; TURN is the
    // fallback for symmetric NAT.
    let mut ice = vec![
        serde_json::json!({ "urls": "stun:stun.l.google.com:19302" }),
        serde_json::json!({ "urls": format!("stun:{host}:3478") }),
    ];

    let ttl = CREDENTIAL_TTL_SECS;
    match std::env::var("TURN_STATIC_SECRET") {
        Ok(secret) if !secret.is_empty() => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let (username, credential) = make_credential(&secret, now, ttl);
            ice.push(serde_json::json!({
                "urls": format!("turn:{host}:3478"),
                "username": username,
                "credential": credential,
            }));
            ice.push(serde_json::json!({
                "urls": format!("turns:{host}:5349"),
                "username": username,
                "credential": credential,
            }));
        }
        _ => {
            // No secret configured: STUN-only. Clients handle the absence of TURN
            // entries gracefully (they already tolerate a failed allocation).
        }
    }

    Json(serde_json::json!({ "iceServers": ice, "ttl": ttl }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The credential must be the base64 of HMAC-SHA1(secret, username), which is
    /// exactly what coturn recomputes and checks. If this drifts, every TURN auth
    /// silently fails and voice quietly loses its symmetric-NAT fallback.
    #[test]
    fn credential_is_base64_hmac_sha1_of_the_username() {
        let (username, credential) = make_credential("test-secret", 1_000_000, 3600);
        assert_eq!(username, "1003600:hos", "username is the expiry timestamp");

        // Recompute independently, the way coturn would.
        let mut mac = Hmac::<Sha1>::new_from_slice(b"test-secret").unwrap();
        mac.update(username.as_bytes());
        let expected = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            mac.finalize().into_bytes(),
        );
        assert_eq!(credential, expected);
    }

    /// The username encodes an expiry in the future, so a captured credential
    /// stops working once it passes.
    #[test]
    fn username_expiry_is_now_plus_ttl() {
        let (username, _) = make_credential("s", 500, 900);
        assert!(username.starts_with("1400:"), "expiry = now + ttl, got {username}");
    }

    /// A different secret yields a different credential for the same username, so
    /// rotating the secret invalidates every previously issued credential.
    #[test]
    fn rotating_the_secret_changes_the_credential() {
        let (_, a) = make_credential("secret-a", 1000, 3600);
        let (_, b) = make_credential("secret-b", 1000, 3600);
        assert_ne!(a, b, "credential must depend on the secret");
    }
}
