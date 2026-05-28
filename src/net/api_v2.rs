//! Native client → relay v2 signed-object submission + connection-ticket helpers.
//!
//! Mirrors what `web/shared/pq-object.js` does for the web client, so a P2P
//! group created on native is byte-identical to one created on web (the
//! canonical CBOR encoder is on both sides and locked by `just group-kat`).
//!
//! Used by `src/gui/pages/chat.rs` to:
//!   - create a group (`group_v1`),
//!   - mint a creator-signed invite + emit a shareable connection ticket
//!     (`group_invite_v1` + base64-of-JSON, matching `encodeInviteTicket`),
//!   - join via a ticket (parse it + `group_join_v1`).
//!
//! HTTP is the existing blocking-ureq pattern (see `upload_image_png_blocking`).
//! Promote to a background tokio task if it ever feels janky in the UI.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD as B64URL};

use crate::relay::core::object::ObjectBuilder;
use crate::relay::core::pq_crypto::{DilithiumKeypair, derive_dilithium_seed};

/// Sign a built object and POST it to `{server_url}/api/v2/objects`. Returns
/// the object_id on success. The relay verifies the signature before storing.
pub fn submit_signed_object(
    server_url: &str,
    seed32: &[u8],
    builder: ObjectBuilder,
) -> Result<String, String> {
    let dil_seed = derive_dilithium_seed(seed32);
    let kp = DilithiumKeypair::from_seed(&dil_seed);
    let obj = builder.sign(&kp).map_err(|e| format!("sign: {e}"))?;
    let object_id = obj.object_id().map_err(|e| format!("object_id: {e}"))?.to_hex();

    // Match the SignedObjectSubmission JSON shape (api_v2_objects.rs).
    let mut submission = serde_json::json!({
        "protocol_version": obj.protocol_version,
        "object_type": obj.object_type,
        "author_public_key_b64": B64.encode(&obj.author_public_key),
        "references": obj.references,
        "payload_schema_version": obj.payload_schema_version,
        "payload_encoding": obj.payload_encoding,
        "payload_b64": B64.encode(&obj.payload),
        "signature_b64": B64.encode(&obj.signature),
    });
    if let Some(s) = &obj.space_id { submission["space_id"] = serde_json::Value::String(s.clone()); }
    if let Some(c) = &obj.channel_id { submission["channel_id"] = serde_json::Value::String(c.clone()); }
    if let Some(t) = obj.created_at { submission["created_at"] = serde_json::Value::from(t); }

    let url = format!("{}/api/v2/objects", server_url.trim_end_matches('/'));
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(&submission.to_string())
        .map_err(|e| format!("POST {url}: {e}"))?;
    if resp.status() != 200 {
        let body = resp.into_string().unwrap_or_default();
        return Err(format!("POST {url}: HTTP non-200 — {body}"));
    }
    Ok(object_id)
}

/// Encode a shareable P2P-group connection ticket. Mirrors web
/// `encodeInviteTicket` byte-for-byte (base64url of compact JSON, no padding),
/// so a ticket made on native parses cleanly in `decodeInviteTicket` on web.
pub fn encode_invite_ticket(
    group_id: &str,
    group_name: &str,
    invite_id: &str,
    secret: &[u8],
) -> String {
    let obj = serde_json::json!({
        "v": 1,
        "g": group_id,
        "n": group_name,
        "i": invite_id,
        "s": B64.encode(secret),
    });
    let json = obj.to_string();
    B64URL.encode(json.as_bytes())
}

/// Decode a ticket string into `(group_id, group_name, invite_id, secret)`.
/// Accepts the format `encode_invite_ticket` produces (and matches the web).
pub fn decode_invite_ticket(s: &str) -> Result<(String, String, String, Vec<u8>), String> {
    let bytes = B64URL
        .decode(s.trim().as_bytes())
        .map_err(|e| format!("base64url: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("json: {e}"))?;
    let group_id = v.get("g").and_then(|x| x.as_str()).ok_or("missing group id (g)")?.to_string();
    let group_name = v.get("n").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let invite_id = v.get("i").and_then(|x| x.as_str()).ok_or("missing invite id (i)")?.to_string();
    let secret_b64 = v.get("s").and_then(|x| x.as_str()).ok_or("missing secret (s)")?;
    let secret = B64.decode(secret_b64).map_err(|e| format!("secret b64: {e}"))?;
    Ok((group_id, group_name, invite_id, secret))
}

// ── High-level group operations ────────────────────────────────────────────
// Each composes a CBOR payload, builds + signs the object, and POSTs it. They
// mirror the web's `buildGroupV1` / `buildGroupInviteV1` / `buildGroupJoinV1`
// (`pq-object.js`); together they let native participate in the P2P-group flow
// (docs/design/p2p-groups.md).

use crate::relay::core::encoding::{cbor_bytes, cbor_int, cbor_map, cbor_text};

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Build + submit a `group_v1`. Returns the new `group_id` (= the object's id).
pub fn submit_group_v1(server_url: &str, seed: &[u8], name: &str) -> Result<String, String> {
    let payload = cbor_map(vec![("name", cbor_text(name))]);
    let builder = ObjectBuilder::new("group_v1")
        .created_at(now_millis())
        .payload_cbor(&payload)
        .map_err(|e| format!("payload: {e}"))?;
    submit_signed_object(server_url, seed, builder)
}

/// Build + submit a creator-signed `group_invite_v1` committing to `secret_hash`
/// and expiring `expires_at_ms`. Returns the new `invite_id`.
pub fn submit_group_invite_v1(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
    expires_at_ms: u64,
    secret_hash: &[u8],
) -> Result<String, String> {
    let payload = cbor_map(vec![
        ("expires_at", cbor_int(expires_at_ms)),
        ("secret_hash", cbor_bytes(secret_hash)),
    ]);
    let builder = ObjectBuilder::new("group_invite_v1")
        .reference(group_id)
        .created_at(now_millis())
        .payload_cbor(&payload)
        .map_err(|e| format!("payload: {e}"))?;
    submit_signed_object(server_url, seed, builder)
}

/// Build + submit a `group_join_v1` revealing the invite secret.
pub fn submit_group_join_v1(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
    invite_id: &str,
    secret: &[u8],
) -> Result<(), String> {
    let payload = cbor_map(vec![("secret", cbor_bytes(secret))]);
    let builder = ObjectBuilder::new("group_join_v1")
        .reference(group_id)
        .reference(invite_id)
        .created_at(now_millis())
        .payload_cbor(&payload)
        .map_err(|e| format!("payload: {e}"))?;
    submit_signed_object(server_url, seed, builder)?;
    Ok(())
}

/// One-shot: create a group AND mint a first 7-day invite for it, returning the
/// shareable connection ticket. This is the create-modal happy path so the user
/// gets something to copy/share immediately.
pub fn create_group_and_first_invite(
    server_url: &str,
    seed: &[u8],
    name: &str,
) -> Result<(String /* group_id */, String /* ticket */), String> {
    let group_id = submit_group_v1(server_url, seed, name)?;
    let mut secret = vec![0u8; 32];
    use rand::RngCore;
    rand::rng().fill_bytes(&mut secret);
    let secret_hash = blake3::hash(&secret).as_bytes().to_vec();
    let expires_at = now_millis() + 7 * 24 * 3600 * 1000;
    let invite_id = submit_group_invite_v1(server_url, seed, &group_id, expires_at, &secret_hash)?;
    let ticket = encode_invite_ticket(&group_id, name, &invite_id, &secret);
    Ok((group_id, ticket))
}

/// Join a P2P group by parsing a ticket and submitting a `group_join_v1`.
/// Returns `(group_id, group_name)` on successful submission (the relay
/// validates the secret + expiry inside `index_group_join`).
pub fn join_group_by_ticket(server_url: &str, seed: &[u8], ticket: &str) -> Result<(String, String), String> {
    let (group_id, group_name, invite_id, secret) = decode_invite_ticket(ticket)?;
    submit_group_join_v1(server_url, seed, &group_id, &invite_id, &secret)?;
    Ok((group_id, group_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_roundtrip() {
        let secret = b"a-test-invite-secret-bytes-32-bb".to_vec();
        let t = encode_invite_ticket("grp_abc", "Research", "inv_xyz", &secret);
        let (g, n, i, s) = decode_invite_ticket(&t).expect("ticket parses");
        assert_eq!(g, "grp_abc");
        assert_eq!(n, "Research");
        assert_eq!(i, "inv_xyz");
        assert_eq!(s, secret);
    }

    #[test]
    fn ticket_rejects_garbage() {
        assert!(decode_invite_ticket("not-base64-or-json").is_err());
        // Valid base64 but not the right JSON shape:
        let bad = B64URL.encode(b"{\"hello\":\"world\"}");
        assert!(decode_invite_ticket(&bad).is_err());
    }
}
