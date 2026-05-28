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

use crate::net::dm_pq::DmPqKeypair;
use crate::net::group_e2ee::{
    self, build_group_epoch_key_v1, build_group_msg_v1, open_epoch_key, open_group_msg,
    parse_group_epoch_key_payload, random_epoch_key, GroupMemberKey,
};
use crate::net::identity::derive_pq_identity;
use crate::relay::core::object::ObjectBuilder;
use crate::relay::core::pq_crypto::{DilithiumKeypair, derive_dilithium_seed};

/// Author fingerprint: first 16 bytes of BLAKE3(dilithium pubkey), hex.
/// Matches `crate::relay::storage::signed_objects::author_fingerprint`.
pub fn author_fingerprint_hex(pubkey: &[u8]) -> String {
    let h = blake3::hash(pubkey);
    h.as_bytes()[..16].iter().map(|b| format!("{b:02x}")).collect()
}

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
/// gets something to copy/share immediately. Also issues an initial epoch_key
/// sealed to the creator so chat works immediately for them (matches the
/// web `createP2pGroup` flow).
pub fn create_group_and_first_invite(
    server_url: &str,
    seed: &[u8],
    name: &str,
) -> Result<(String /* group_id */, String /* ticket */), String> {
    let group_id = submit_group_v1(server_url, seed, name)?;

    // Initial epoch key sealed to the creator — without this, the group is
    // identity+membership only and the creator cannot send anything.
    let identity = derive_pq_identity(seed).map_err(|e| format!("derive identity: {e}"))?;
    let dil_pub_bytes =
        hex::decode(&identity.dilithium_hex).map_err(|e| format!("dilithium hex: {e}"))?;
    let my_fp = author_fingerprint_hex(&dil_pub_bytes);
    let initial_epoch_key = random_epoch_key();
    let initial_members = vec![GroupMemberKey {
        fp: my_fp,
        kyber_pub_b64: identity.kyber_public_b64.clone(),
    }];
    submit_group_epoch_key_v1(server_url, seed, &group_id, 1, &initial_epoch_key, &initial_members)?;

    let mut secret = vec![0u8; 32];
    use rand::RngCore;
    rand::rng().fill_bytes(&mut secret);
    let secret_hash = blake3::hash(&secret).as_bytes().to_vec();
    let expires_at = now_millis() + 7 * 24 * 3600 * 1000;
    let invite_id = submit_group_invite_v1(server_url, seed, &group_id, expires_at, &secret_hash)?;
    let ticket = encode_invite_ticket(&group_id, name, &invite_id, &secret);
    Ok((group_id, ticket))
}

/// Submit a `group_epoch_key_v1` (creator-only, gated by the relay's
/// `index_group_epoch_key`). Returns the epoch-key object's id on success.
pub fn submit_group_epoch_key_v1(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
    epoch: u64,
    epoch_key: &[u8],
    members: &[GroupMemberKey],
) -> Result<String, String> {
    let builder = build_group_epoch_key_v1(group_id, epoch, epoch_key, members)?;
    submit_signed_object(server_url, seed, builder)
}

/// Submit a `group_msg_v1` (AES-GCM ciphertext under the epoch key).
pub fn submit_group_msg_v1(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
    epoch: u64,
    epoch_key: &[u8],
    plaintext: &str,
) -> Result<String, String> {
    let builder = build_group_msg_v1(group_id, epoch, epoch_key, plaintext)?;
    submit_signed_object(server_url, seed, builder)
}

/// GET /api/v2/objects/{id}. Returns None on 404, the raw JSON otherwise.
pub fn fetch_signed_object(
    server_url: &str,
    object_id: &str,
) -> Result<Option<serde_json::Value>, String> {
    let url = format!(
        "{}/api/v2/objects/{}",
        server_url.trim_end_matches('/'),
        urlencoded(object_id),
    );
    match ureq::get(&url).call() {
        Ok(resp) => {
            if resp.status() != 200 {
                return Err(format!("GET {url}: HTTP {}", resp.status()));
            }
            let body = resp.into_string().map_err(|e| format!("read: {e}"))?;
            let v: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;
            Ok(Some(v))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => Err(format!("GET {url}: {e}")),
    }
}

/// GET /api/v2/groups/{id}/epoch. Returns the latest epoch-key object's
/// payload bytes (b64-decoded), or None if no epoch has been issued yet.
pub fn fetch_group_epoch_payload(
    server_url: &str,
    group_id: &str,
) -> Result<Option<Vec<u8>>, String> {
    let url = format!(
        "{}/api/v2/groups/{}/epoch",
        server_url.trim_end_matches('/'),
        urlencoded(group_id),
    );
    let resp = match ureq::get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(e) => return Err(format!("GET {url}: {e}")),
    };
    if resp.status() != 200 {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let body = resp.into_string().map_err(|e| format!("read: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;
    let payload_b64 = v
        .get("payload_b64")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "missing payload_b64".to_string())?;
    let payload = B64
        .decode(payload_b64)
        .map_err(|e| format!("payload b64: {e}"))?;
    Ok(Some(payload))
}

/// A decrypted group message (for the chat view).
#[derive(Debug, Clone)]
pub struct GroupMessage {
    pub object_id: String,
    pub author_fp: String,
    pub created_at: i64,
    pub text: String,
}

/// GET /api/v2/groups/{id}/messages → fetch ciphertexts, decrypt with `epoch_key`.
/// Messages from a different epoch (whose ciphertext doesn't authenticate
/// under this key) are silently skipped — for Phase 2 the chat shows only the
/// current epoch's history.
pub fn fetch_and_decrypt_group_messages(
    server_url: &str,
    group_id: &str,
    epoch_key: &[u8],
) -> Result<Vec<GroupMessage>, String> {
    let url = format!(
        "{}/api/v2/groups/{}/messages",
        server_url.trim_end_matches('/'),
        urlencoded(group_id),
    );
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if resp.status() != 200 {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let body = resp.into_string().map_err(|e| format!("read: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;
    let msgs = v
        .get("messages")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(msgs.len());
    for m in msgs {
        let object_id = m.get("object_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let author_fp = m.get("author_fp").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let created_at = m.get("created_at").and_then(|x| x.as_i64()).unwrap_or(0);
        let payload_b64 = match m.get("payload_b64").and_then(|x| x.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let payload = match B64.decode(payload_b64) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Ok(text) = open_group_msg(&payload, epoch_key) {
            out.push(GroupMessage { object_id, author_fp, created_at, text });
        }
    }
    Ok(out)
}

/// GET /api/v2/groups/{id}/members → roster with each member's Kyber pub.
/// Returns `Vec<(dilithium_hex, Option<kyber_pub_b64>)>`.
pub fn fetch_group_members(
    server_url: &str,
    group_id: &str,
) -> Result<Vec<(String, Option<String>)>, String> {
    let url = format!(
        "{}/api/v2/groups/{}/members",
        server_url.trim_end_matches('/'),
        urlencoded(group_id),
    );
    let resp = ureq::get(&url).call().map_err(|e| format!("GET {url}: {e}"))?;
    if resp.status() != 200 {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let body = resp.into_string().map_err(|e| format!("read: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;
    let members = v
        .get("members")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(members.len());
    for m in members {
        let pubkey = m.get("pubkey").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let kyber = m.get("kyber_public").and_then(|x| x.as_str()).map(|s| s.to_string());
        if !pubkey.is_empty() {
            out.push((pubkey, kyber));
        }
    }
    Ok(out)
}

/// If I am the creator AND new members have joined since the last epoch key,
/// mint a fresh epoch sealed to the full roster + submit. Returns
/// `Some((new_epoch, new_epoch_key, added_count))` on rekey, `None` otherwise.
/// Mirrors the web's `rekeyIfCreatorNeeds` in `chat-groups-p2p.js`.
pub fn rekey_if_creator_needs(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
) -> Result<Option<(u64, Vec<u8>, usize)>, String> {
    let identity = derive_pq_identity(seed).map_err(|e| format!("derive: {e}"))?;
    let my_dil_bytes =
        hex::decode(&identity.dilithium_hex).map_err(|e| format!("dilithium hex: {e}"))?;
    let my_pub_b64 = B64.encode(&my_dil_bytes);

    // 1. Am I the creator?
    let group_obj = match fetch_signed_object(server_url, group_id)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let creator_b64 = group_obj
        .get("author_public_key_b64")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if creator_b64 != my_pub_b64 {
        return Ok(None);
    }

    // 2. Current epoch + already-covered recipient fingerprints.
    let mut current_epoch: u64 = 0;
    let mut covered = std::collections::HashSet::new();
    if let Some(payload_bytes) = fetch_group_epoch_payload(server_url, group_id)? {
        if let Ok(parsed) = parse_group_epoch_key_payload(&payload_bytes) {
            current_epoch = parsed.epoch;
            for r in &parsed.recipients {
                covered.insert(r.fp.clone());
            }
        }
    }

    // 3. Current roster with each member's Kyber pub.
    let roster = fetch_group_members(server_url, group_id)?;
    let mut sealable: Vec<GroupMemberKey> = Vec::new();
    let mut has_gap = false;
    for (pubkey_hex, kyber_opt) in roster {
        let kyber = match kyber_opt {
            Some(k) => k,
            None => continue,
        };
        let pub_bytes = match hex::decode(&pubkey_hex) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let fp = author_fingerprint_hex(&pub_bytes);
        if !covered.contains(&fp) {
            has_gap = true;
        }
        sealable.push(GroupMemberKey { fp, kyber_pub_b64: kyber });
    }
    if !has_gap {
        return Ok(None);
    }

    // 4. Mint new epoch sealed to the full sealable roster.
    let new_epoch = current_epoch + 1;
    let new_key = random_epoch_key();
    submit_group_epoch_key_v1(server_url, seed, group_id, new_epoch, &new_key, &sealable)?;
    let added = sealable.iter().filter(|m| !covered.contains(&m.fp)).count();
    Ok(Some((new_epoch, new_key, added)))
}

/// Fetch + unseal MY copy of the group's latest epoch key. Returns
/// `Some((epoch, epoch_key))` if I have a recipient entry, `None` otherwise.
pub fn fetch_my_epoch_key(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
) -> Result<Option<(u64, Vec<u8>)>, String> {
    let identity = derive_pq_identity(seed).map_err(|e| format!("derive: {e}"))?;
    let dil_pub_bytes =
        hex::decode(&identity.dilithium_hex).map_err(|e| format!("dilithium hex: {e}"))?;
    let my_fp = author_fingerprint_hex(&dil_pub_bytes);
    let me = DmPqKeypair::from_bip39_seed(seed).map_err(|e| format!("dm keypair: {e}"))?;

    let payload_bytes = match fetch_group_epoch_payload(server_url, group_id)? {
        Some(p) => p,
        None => return Ok(None),
    };
    let parsed = parse_group_epoch_key_payload(&payload_bytes)?;
    match open_epoch_key(&parsed, &my_fp, &me) {
        Ok(pair) => Ok(Some(pair)),
        Err(_) => Ok(None), // no entry for us / wrong key — handled by rekey
    }
}

/// Join a P2P group by parsing a ticket and submitting a `group_join_v1`.
/// Returns `(group_id, group_name)` on successful submission (the relay
/// validates the secret + expiry inside `index_group_join`).
pub fn join_group_by_ticket(server_url: &str, seed: &[u8], ticket: &str) -> Result<(String, String), String> {
    let (group_id, group_name, invite_id, secret) = decode_invite_ticket(ticket)?;
    submit_group_join_v1(server_url, seed, &group_id, &invite_id, &secret)?;
    Ok((group_id, group_name))
}

/// A P2P group as seen by the relay's read endpoint (roster projection).
#[derive(Debug, Clone)]
pub struct P2pGroupInfo {
    pub group_id: String,
    pub name: String,
    /// Active member Dilithium public keys, hex-encoded.
    pub members: Vec<String>,
}

/// Fetch the caller's P2P groups + each roster from the relay (read-only
/// convenience view of the projection). Used to render P2P groups in the
/// native left panel.
pub fn fetch_p2p_groups(server_url: &str, dilithium_hex: &str) -> Result<Vec<P2pGroupInfo>, String> {
    let url = format!(
        "{}/api/v2/groups?pubkey={}",
        server_url.trim_end_matches('/'),
        urlencoded(dilithium_hex),
    );
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    if resp.status() != 200 {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let body = resp.into_string().map_err(|e| format!("read response: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("parse JSON: {e}"))?;
    let arr = json
        .get("groups")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(arr.len());
    for g in arr {
        let group_id = g.get("group_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if group_id.is_empty() { continue; }
        let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let members: Vec<String> = g
            .get("members")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|m| m.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        out.push(P2pGroupInfo { group_id, name, members });
    }
    Ok(out)
}

/// Minimal URL component encoder — only the chars that actually need it in a
/// query-string position (hex pubkeys are all `[0-9a-f]` so this is a no-op
/// for the common case, but it keeps the function safe for unusual inputs).
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
