//! Reading a channel's history from a server (2026-11-02, docs/design/chat-access.md): a channel
//! that is not open to Anyone is read only through a member's signed request, so the app signs
//! every history read it makes. The relay's side is `src/relay/chat_access.rs`.
//!
//! The request is `POST <server>/api/messages` with `{key, timestamp, sig, channel, limit}`: `sig`
//! is the Dilithium3 signature over `humanity_protocol::chat_access::read_purpose("messages",
//! channel)`, then `\n` and the time in milliseconds. It goes in the body, never the address: a
//! Dilithium key and signature are about 10 KB, past what nginx accepts in an address (HTTP 414,
//! the admin-stats lesson in gui/pages/relay_control.rs). Without a seed (no identity loaded) the
//! app asks as a visitor would, with `GET`, and is answered only for an Anyone channel.

/// The body of a signed history read of `channel`, at most `limit` messages, at `ts` (ms).
pub fn signed_body(seed: &[u8], key: &str, channel: &str, limit: usize, ts: u64) -> String {
    let purpose = humanity_protocol::chat_access::read_purpose("messages", channel);
    let sig = crate::net::identity::pq_sign_chat(seed, &purpose, ts);
    serde_json::json!({ "key": key, "timestamp": ts, "sig": sig, "channel": channel, "limit": limit }).to_string()
}

/// Read `channel`'s latest `limit` messages from the server at `base` (no trailing `/`): signed
/// when a seed is given, as a visitor otherwise. The answer's body, or why there is none.
/// Blocking: call it on a background thread.
pub fn fetch_history(agent: &ureq::Agent, base: &str, seed: Option<&[u8]>, key: &str, channel: &str, limit: usize) -> Result<String, String> {
    let url = format!("{base}/api/messages");
    let answer = match seed {
        Some(seed) if !key.is_empty() => {
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis() as u64;
            agent.post(&url).set("Content-Type", "application/json").send_string(&signed_body(seed, key, channel, limit, ts))
        }
        _ => agent.get(&url).query("limit", &limit.to_string()).query("channel", channel).call(),
    };
    match answer {
        Ok(resp) => resp.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, resp)) => {
            let why = resp.into_string().ok().and_then(|b| serde_json::from_str::<serde_json::Value>(&b).ok()).and_then(|v| v["error"].as_str().map(str::to_string));
            Err(match why {
                Some(words) => format!("HTTP {code}: {words}"),
                None => format!("HTTP {code}"),
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body names the reader and the channel and carries a signature the relay checks: the
    /// same purpose words and time, verified with the reader's own key.
    #[test]
    fn the_signed_body_is_what_the_relay_checks() {
        let seed = [7u8; 32];
        let pq = crate::net::identity::derive_pq_identity(&seed).unwrap();
        let body: serde_json::Value = serde_json::from_str(&signed_body(&seed, &pq.dilithium_hex, "lounge", 50, 1_000)).unwrap();
        assert_eq!(body["channel"], "lounge");
        assert_eq!(body["limit"], 50);
        let sig = hex::decode(body["sig"].as_str().unwrap()).unwrap();
        let pk = hex::decode(&pq.dilithium_hex).unwrap();
        let signed = format!("{}\n1000", humanity_protocol::chat_access::read_purpose("messages", "lounge"));
        assert!(humanity_protocol::pq_crypto::verify_dilithium(&pk, signed.as_bytes(), &sig).is_ok());
    }
}
