//! `GET /api/turn-credentials`: the address-lookup servers a client may ask, with no sign-in.
//!
//! Since step E (2026-10-09, docs/design/blocking-and-safe-mode.md 10f) this is our own STUN
//! responder and nothing else. Google's STUN servers are gone from it: asking them showed every
//! caller's address to Google. And it never hands out TURN: the call forwarder's credentials come
//! only over the signed-in socket, for one voice room or call (`call_credentials` in
//! call_credentials.rs), never from an unauthenticated URL.
//!
//! History: until v0.857 the clients carried a static TURN password, and until 2026-10-09 this
//! endpoint issued coturn REST credentials from `TURN_STATIC_SECRET`. coturn was taken off the
//! server after the 2026-08-07 incident (docs/INCIDENT-PLAYBOOK.md), so both are gone.
//!
//! When the forwarder is not listening (voice switched off, or its port could not be opened),
//! the list is empty rather than naming a port where nothing answers: a client waiting on a STUN
//! server that never replies only slows its own call setup.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::relay::call_credentials::CREDENTIAL_TTL_SECS;
use crate::relay::relay::RelayState;

/// The `iceServers` list for a forwarder STUN URL (or none).
fn ice_servers(stun_url: Option<String>) -> Vec<serde_json::Value> {
    stun_url.into_iter().map(|url| serde_json::json!({ "urls": url })).collect()
}

/// `GET /api/turn-credentials`. The shape (`iceServers`, `ttl`) is unchanged, so a client that
/// read it before reads it now.
pub async fn turn_credentials(State(state): State<Arc<RelayState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "iceServers": ice_servers(state.calls.stun_url()), "ttl": CREDENTIAL_TTL_SECS }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::storage::Storage;

    /// The list is our own STUN entry and nothing else: no Google, no TURN, no credential. With
    /// the forwarder not listening it is empty.
    ///
    /// Seen red 2026-10-09 with Google's entry put back in front of ours in `ice_servers`:
    /// "nothing listening: nothing offered" (the list still held Google's entry).
    #[test]
    fn only_our_own_stun_entry_is_offered() {
        let state = RelayState::new(Storage::open_temp("turn_only_ours"));
        assert!(ice_servers(state.calls.stun_url()).is_empty(), "nothing listening: nothing offered");

        state.calls.set_listening("relay.example", 3478);
        let list = ice_servers(state.calls.stun_url());
        let urls: Vec<&str> = list.iter().map(|e| e["urls"].as_str().unwrap_or_default()).collect();
        assert_eq!(urls, vec!["stun:relay.example:3478"]);
        for e in &list {
            assert!(e.get("username").is_none() && e.get("credential").is_none(), "no credential: {e}");
        }
        let text = serde_json::to_string(&list).unwrap();
        assert!(!text.contains("google") && !text.contains("turn:") && !text.contains("turns:"), "{text}");
    }

    /// An IPv6 host goes in brackets, as a URL needs.
    #[test]
    fn an_ipv6_host_is_bracketed() {
        let state = RelayState::new(Storage::open_temp("turn_v6"));
        state.calls.set_listening("2001:db8::7", 3478);
        assert_eq!(state.calls.stun_url().as_deref(), Some("stun:[2001:db8::7]:3478"));
    }
}
