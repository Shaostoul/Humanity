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
    parse_group_epoch_key_payload, parse_group_msg_epoch, payload_shares_history,
    random_epoch_key, GroupMemberKey,
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
/// `share_history` is included in the SIGNED payload ONLY when true — omitting it
/// for private (default) groups keeps the byte-for-byte encoding of pre-toggle
/// groups (so old ids + the canonical KAT are unaffected). When true, members
/// who join later get the full history (see `rekey_if_creator_needs`).
pub fn submit_group_v1(server_url: &str, seed: &[u8], name: &str, share_history: bool) -> Result<String, String> {
    let mut pairs = vec![("name", cbor_text(name))];
    if share_history {
        pairs.push(("share_history", cbor_int(1u64)));
    }
    let payload = cbor_map(pairs); // cbor_map sorts keys canonically
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

/// Build + submit a `group_member_v1` removing MYSELF from the group (a
/// self-leave). The relay authorizes self-removal for any member, so the group
/// drops off my list without the creator being involved.
pub fn submit_group_leave(server_url: &str, seed: &[u8], group_id: &str) -> Result<(), String> {
    let identity = derive_pq_identity(seed).map_err(|e| format!("derive identity: {e}"))?;
    let my_pub = hex::decode(&identity.dilithium_hex).map_err(|e| format!("dilithium hex: {e}"))?;
    let payload = cbor_map(vec![
        ("action", cbor_text("remove")),
        ("subject", cbor_bytes(&my_pub)),
    ]);
    let builder = ObjectBuilder::new("group_member_v1")
        .reference(group_id)
        .created_at(now_millis())
        .payload_cbor(&payload)
        .map_err(|e| format!("payload: {e}"))?;
    submit_signed_object(server_url, seed, builder)?;
    Ok(())
}

/// Build + submit a creator-signed `group_disband_v1` tearing the group down
/// for everyone. The relay honors it only if the author is the group creator
/// (empty payload — the group_id reference + signature carry the meaning).
pub fn submit_group_disband(server_url: &str, seed: &[u8], group_id: &str) -> Result<(), String> {
    let payload = cbor_map(vec![]);
    let builder = ObjectBuilder::new("group_disband_v1")
        .reference(group_id)
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
    share_history: bool,
) -> Result<(String /* group_id */, String /* ticket */), String> {
    let group_id = submit_group_v1(server_url, seed, name, share_history)?;

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

/// Serialize a SIGNED `Object` to the exact JSON body the relay's
/// `/api/v2/objects` POST handler expects (the `SignedObjectSubmission` shape).
/// This is the canonical-on-the-wire encoding, shared by the relay POST and the
/// WebRTC mesh push so a peer receives byte-identical bytes to what the relay
/// stores. Kept separate from `submit_signed_object` so callers that want the
/// JSON without POSTing (the mesh broadcast) can reuse it.
fn object_to_submission_json(obj: &crate::relay::core::object::Object) -> String {
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
    submission.to_string()
}

/// Build + sign a `group_msg_v1` and return BOTH its object_id (hex) and the
/// submission JSON string (the body POSTed to `/api/v2/objects`).
///
/// This is the reusable build the send path needs (mirrors web `sendGroupMessage`
/// → `buildGroupMsgV1`): build the object ONCE, then both POST it to the relay
/// (durable cache + offline backfill) AND hand the same JSON to the mesh
/// broadcast. The two copies dedup by object_id, so building once keeps the
/// pushed and polled copies bit-identical.
pub fn build_group_msg_submission(
    seed: &[u8],
    group_id: &str,
    epoch: u64,
    epoch_key: &[u8],
    plaintext: &str,
) -> Result<(String /* object_id hex */, String /* submission JSON */), String> {
    let builder = build_group_msg_v1(group_id, epoch, epoch_key, plaintext)?;
    // Sign with the Dilithium key derived from the seed (same path as
    // submit_signed_object), so the object_id + signature match exactly.
    let dil_seed = derive_dilithium_seed(seed);
    let kp = DilithiumKeypair::from_seed(&dil_seed);
    let obj = builder.sign(&kp).map_err(|e| format!("sign: {e}"))?;
    let object_id = obj.object_id().map_err(|e| format!("object_id: {e}"))?.to_hex();
    let submission_json = object_to_submission_json(&obj);
    Ok((object_id, submission_json))
}

/// POST an already-built submission JSON string to `{server_url}/api/v2/objects`.
/// Used by the send path after `build_group_msg_submission` so the relay gets
/// the identical bytes that were pushed over the mesh. The relay re-verifies the
/// signature before storing.
pub fn post_submission_json(server_url: &str, submission_json: &str) -> Result<(), String> {
    let url = format!("{}/api/v2/objects", server_url.trim_end_matches('/'));
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(submission_json)
        .map_err(|e| format!("POST {url}: {e}"))?;
    if resp.status() != 200 {
        let body = resp.into_string().unwrap_or_default();
        return Err(format!("POST {url}: HTTP non-200 — {body}"));
    }
    Ok(())
}

// ── P2P-pushed submission verification (THE security gate) ───────────────────
// A submission JSON arriving over a WebRTC DataChannel from a peer is UNTRUSTED
// until its ML-DSA signature verifies. This mirrors web `pq-object.js`'s
// `verifyObjectSubmission`: parse the wire JSON back into an `Object`, run the
// SAME `Object::verify_signature()` the relay runs, recompute the object_id from
// the canonical bytes (never trust an attacker-supplied id), and only then hand
// back the verified fields. Any malformed/forged input returns `None` → drop.

/// A peer-pushed submission that PASSED signature verification. Every field here
/// is derived from the verified `Object` — the `object_id` is recomputed locally
/// (BLAKE3 of the canonical bytes), never taken from the wire.
#[derive(Debug, Clone)]
pub struct VerifiedSubmission {
    /// Locally-recomputed object id (hex) — the dedup key.
    pub object_id: String,
    /// The object's declared type (e.g. "group_msg_v1").
    pub object_type: String,
    /// Author's Dilithium public key, hex — for the roster membership gate.
    pub author_pubkey_hex: String,
    /// Author fingerprint (BLAKE3(pubkey)[..16] hex) — matches the roster map keys.
    pub author_fp: String,
    /// The object's references (references[0] is the group_id for group_msg_v1).
    pub references: Vec<String>,
    /// Decoded payload bytes (canonical CBOR) — feed to `open_group_msg`.
    pub payload: Vec<u8>,
    /// Informational created_at (ms), 0 if absent.
    pub created_at: i64,
}

/// Verify a peer-pushed submission JSON string. Returns `Some(VerifiedSubmission)`
/// ONLY if the JSON parses into a well-formed `Object` whose ML-DSA signature
/// verifies against its own `author_public_key`. Returns `None` on ANY failure
/// (bad JSON, bad base64, wrong key/sig length, signature mismatch) — the caller
/// MUST drop on `None`.
///
/// Security note: this is the trust boundary for P2P-pushed group objects.
/// `Object::verify_signature()` recomputes the canonical signable bytes (the
/// object with an all-zero signature field) and checks the Dilithium3 signature
/// over them — identical to what the relay does on ingest — so a forged or
/// tampered object cannot pass. The object_id is recomputed here, not trusted
/// from the sender.
pub fn verify_submission_json(submission_json: &str) -> Option<VerifiedSubmission> {
    use crate::relay::core::object::{Object, PAYLOAD_ENCODING_PLAINTEXT, PROTOCOL_VERSION};

    let v: serde_json::Value = serde_json::from_str(submission_json).ok()?;

    // Decode the wire fields. Anything missing/misformatted → reject.
    let object_type = v.get("object_type")?.as_str()?.to_string();
    let author_public_key = B64
        .decode(v.get("author_public_key_b64")?.as_str()?)
        .ok()?;
    let payload = B64.decode(v.get("payload_b64")?.as_str()?).ok()?;
    let signature = B64.decode(v.get("signature_b64")?.as_str()?).ok()?;

    // Optional fields with sane defaults (must match how the object was built so
    // the canonical bytes — and therefore the signature check — line up).
    let protocol_version = v
        .get("protocol_version")
        .and_then(|x| x.as_u64())
        .unwrap_or(PROTOCOL_VERSION);
    let payload_schema_version = v
        .get("payload_schema_version")
        .and_then(|x| x.as_u64())
        .unwrap_or(1);
    let payload_encoding = v
        .get("payload_encoding")
        .and_then(|x| x.as_str())
        .unwrap_or(PAYLOAD_ENCODING_PLAINTEXT)
        .to_string();
    let space_id = v.get("space_id").and_then(|x| x.as_str()).map(|s| s.to_string());
    let channel_id = v.get("channel_id").and_then(|x| x.as_str()).map(|s| s.to_string());
    let created_at_u64 = v.get("created_at").and_then(|x| x.as_u64());
    let references: Vec<String> = v
        .get("references")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|r| r.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    // Reconstruct the Object faithfully so verify_signature() recomputes the
    // exact canonical bytes the author signed.
    let obj = Object {
        protocol_version,
        object_type: object_type.clone(),
        space_id,
        channel_id,
        author_public_key: author_public_key.clone(),
        created_at: created_at_u64,
        references: references.clone(),
        payload_schema_version,
        payload_encoding,
        payload: payload.clone(),
        signature,
    };

    // THE gate: reject unless the Dilithium3 signature verifies. (Err on a
    // wrong-length key/sig too — verify_dilithium length-checks before verifying.)
    if obj.verify_signature().is_err() {
        return None;
    }

    // Recompute the id locally — NEVER trust a sender-supplied object_id.
    let object_id = obj.object_id().ok()?.to_hex();
    let author_pubkey_hex = hex::encode(&author_public_key);
    let author_fp = author_fingerprint_hex(&author_public_key);

    Some(VerifiedSubmission {
        object_id,
        object_type,
        author_pubkey_hex,
        author_fp,
        references,
        payload,
        created_at: created_at_u64.map(|t| t as i64).unwrap_or(0),
    })
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
/// One still-encrypted group message from the relay (payload decoded from
/// base64 but NOT yet opened). Internal to the load pipeline — lets the network
/// fetch run on a background thread CONCURRENTLY with the roster + epoch-key
/// fetches; decryption happens locally once the epoch key is in hand.
struct RawGroupMsg {
    object_id: String,
    author_fp: String,
    created_at: i64,
    payload: Vec<u8>,
}

/// GET the group's raw (still-encrypted) message log — NO decryption. Kept apart
/// from decryption so it can run concurrently with the roster + epoch-key
/// fetches (see `load_group_blocking`).
fn fetch_group_messages_raw(server_url: &str, group_id: &str) -> Result<Vec<RawGroupMsg>, String> {
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
        let payload_b64 = match m.get("payload_b64").and_then(|x| x.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let payload = match B64.decode(payload_b64) {
            Ok(p) => p,
            Err(_) => continue,
        };
        out.push(RawGroupMsg {
            object_id: m.get("object_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            author_fp: m.get("author_fp").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            created_at: m.get("created_at").and_then(|x| x.as_i64()).unwrap_or(0),
            payload,
        });
    }
    Ok(out)
}

/// Decrypt a raw message log (from `fetch_group_messages_raw`) under a set of
/// epoch keys. Each message opens under the key for ITS OWN epoch — the log
/// spans epochs after a re-key, so a single key can't open all of it. Messages
/// whose epoch we hold no key for (e.g. epochs before we joined) are dropped.
fn decrypt_group_messages_raw(
    raw: Vec<RawGroupMsg>,
    keys: &std::collections::HashMap<u64, Vec<u8>>,
) -> Vec<GroupMessage> {
    raw.into_iter()
        .filter_map(|m| {
            let epoch = parse_group_msg_epoch(&m.payload).ok()?;
            let key = keys.get(&epoch)?;
            open_group_msg(&m.payload, key).ok().map(|text| GroupMessage {
                object_id: m.object_id,
                author_fp: m.author_fp,
                created_at: m.created_at,
                text,
            })
        })
        .collect()
}

/// Fetch + decrypt the group's message log under a SINGLE epoch key (legacy
/// convenience wrapper for callers that only hold one key; decrypts every
/// message with it regardless of epoch). `load_group_blocking` uses the
/// multi-epoch path instead.
pub fn fetch_and_decrypt_group_messages(
    server_url: &str,
    group_id: &str,
    epoch_key: &[u8],
) -> Result<Vec<GroupMessage>, String> {
    let raw = fetch_group_messages_raw(server_url, group_id)?;
    Ok(raw
        .into_iter()
        .filter_map(|m| {
            open_group_msg(&m.payload, epoch_key).ok().map(|text| GroupMessage {
                object_id: m.object_id,
                author_fp: m.author_fp,
                created_at: m.created_at,
                text,
            })
        })
        .collect())
}

/// Everything the chat view needs to render an open P2P group, gathered in one
/// blocking call so it can run on a BACKGROUND THREAD (the UI never freezes).
/// The GUI applies this on the main thread when the worker returns.
#[derive(Debug, Clone, Default)]
pub struct GroupLoad {
    pub group_id: String,
    pub epoch: u64,
    pub epoch_key: Option<Vec<u8>>,
    pub messages: Vec<GroupMessage>,
    /// (author_fp, dilithium_pubkey_hex) for each active member — lets the GUI
    /// map a message's fingerprint to a pubkey (identicon) + name on the main
    /// thread (where it can consult the peer/user list).
    pub members: Vec<(String, String)>,
}

/// Full blocking load for a P2P group: fetch ALL my epoch keys + roster + the
/// message log concurrently (+ creator re-key), then decrypt each message under
/// the key for its own epoch. Best-effort (logs + degrades on partial failure
/// rather than erroring) so a transient hiccup never blanks the view. Pure
/// network/crypto, no shared state — safe to run off the UI thread.
pub fn load_group_blocking(server_url: &str, seed: &[u8], group_id: &str) -> GroupLoad {
    // Fetch the independent pieces CONCURRENTLY on scoped threads. This was up to
    // SIX sequential relay round-trips — rekey's group_v1 + epoch + members, then
    // a redundant fetch_my_epoch_key (epoch again), then a redundant
    // fetch_group_members (members again), then the message log — which is the
    // 1-3s "Loading…" stall the operator saw on open. None of these depends on
    // another at the FETCH layer; only message *decryption* needs the keys, a
    // local step done after. ureq's blocking calls don't hold any lock, so
    // scoped threads give real wall-clock parallelism (these boxes have cores to
    // spare). First-open now ≈ the slowest single fetch chain.
    //
    // fetch_all_epoch_keys pulls EVERY epoch key I was sealed into: the log spans
    // epochs after a re-key, so each message opens under the key for its OWN
    // epoch (a single latest key can't decrypt pre-re-key history).
    let (keys_res, members_res, raw_res, rekey_res) = std::thread::scope(|s| {
        let k = s.spawn(|| fetch_all_epoch_keys(server_url, seed, group_id));
        let m = s.spawn(|| fetch_group_members(server_url, group_id));
        let r = s.spawn(|| fetch_group_messages_raw(server_url, group_id));
        // Creator-only re-key (seals the epoch to members who joined since the
        // last rotation). For a non-creator it's a single GET that bails; for a
        // creator it may rotate + POST. Running it ALONGSIDE the reads means a
        // creator's open is bounded by the rekey chain alone, not rekey-then-
        // everything-else in series. The race with the epoch reads is benign — a
        // fresh rotation is merged into the key set below.
        let rk = s.spawn(|| rekey_if_creator_needs(server_url, seed, group_id));
        (
            k.join().unwrap_or_else(|_| Ok(std::collections::HashMap::new())),
            m.join().unwrap_or_else(|_| Ok(Vec::new())),
            r.join().unwrap_or_else(|_| Ok(Vec::new())),
            rk.join().unwrap_or_else(|_| Ok(None)),
        )
    });

    // (1) Epoch key set (epoch → key) for decrypting the full multi-epoch log.
    let mut keys = match keys_res {
        Ok(m) => m,
        Err(e) => {
            log::warn!("load_group: fetch_all_epoch_keys: {e}");
            std::collections::HashMap::new()
        }
    };
    // Merge a fresh creator rotation (seals a new epoch to newly-joined members).
    match &rekey_res {
        Ok(Some((e, k, added))) => {
            keys.insert(*e, k.clone());
            if *added > 0 {
                log::info!("load_group: rotated epoch key for {added} new member(s) (epoch {e})");
            }
        }
        Ok(None) => {}
        Err(e) => log::warn!("load_group: rekey_if_creator_needs: {e}"),
    }
    // Latest epoch + its key drive sending new messages (GroupLoad.epoch/.epoch_key).
    let epoch = keys.keys().copied().max().unwrap_or(0);
    let epoch_key = keys.get(&epoch).cloned();

    // (2) Roster → (fp, pubkey_hex).
    let members = match members_res {
        Ok(roster) => roster
            .into_iter()
            .filter_map(|(pubkey_hex, _kyber)| {
                hex::decode(&pubkey_hex)
                    .ok()
                    .map(|bytes| (author_fingerprint_hex(&bytes), pubkey_hex))
            })
            .collect(),
        Err(e) => {
            log::warn!("load_group: fetch_group_members: {e}");
            Vec::new()
        }
    };

    // (3) Decrypt the (concurrently-fetched) log — each message under the key for
    //     its own epoch.
    let messages = match raw_res {
        Ok(raw) if !keys.is_empty() => decrypt_group_messages_raw(raw, &keys),
        Ok(_) => Vec::new(),
        Err(e) => {
            log::warn!("load_group: fetch_group_messages_raw: {e}");
            Vec::new()
        }
    };

    GroupLoad { group_id: group_id.to_string(), epoch, epoch_key, messages, members }
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

    // 1b. Does this group share its history with members who join later?
    //     (signed flag in group_v1; absent → false = private default.)
    let share_history = group_obj
        .get("payload_b64")
        .and_then(|x| x.as_str())
        .and_then(|s| B64.decode(s).ok())
        .map(|bytes| payload_shares_history(&bytes))
        .unwrap_or(false);

    // 2. Current epoch + already-covered recipient fingerprints. Keep the raw
    //    payload bytes so a SHARED-history re-seal can unseal the current key.
    let mut current_epoch: u64 = 0;
    let mut covered = std::collections::HashSet::new();
    let mut current_epoch_payload_bytes: Option<Vec<u8>> = None;
    if let Some(payload_bytes) = fetch_group_epoch_payload(server_url, group_id)? {
        if let Ok(parsed) = parse_group_epoch_key_payload(&payload_bytes) {
            current_epoch = parsed.epoch;
            for r in &parsed.recipients {
                covered.insert(r.fp.clone());
            }
        }
        current_epoch_payload_bytes = Some(payload_bytes);
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
    let added = sealable.iter().filter(|m| !covered.contains(&m.fp)).count();

    // 4. Cover the new member(s).
    if share_history {
        // SHARED: do NOT rotate — re-seal the SAME (current) epoch key to the
        // expanded roster so the new member can decrypt the existing history
        // (which is all under this one epoch). The creator unseals their own
        // copy of the current key to re-seal it. Trade-off (a listed con):
        // weaker forward secrecy, since the group never rotates on join.
        if current_epoch >= 1 {
            if let Some(bytes) = &current_epoch_payload_bytes {
                if let Ok(parsed) = parse_group_epoch_key_payload(bytes) {
                    let my_fp = author_fingerprint_hex(&my_dil_bytes);
                    let me = DmPqKeypair::from_bip39_seed(seed)
                        .map_err(|e| format!("dm keypair: {e}"))?;
                    if let Ok((epoch, key)) = open_epoch_key(&parsed, &my_fp, &me) {
                        submit_group_epoch_key_v1(server_url, seed, group_id, epoch, &key, &sealable)?;
                        return Ok(Some((epoch, key, added)));
                    }
                }
            }
        }
        // Fallback (no current epoch / couldn't unseal): seed epoch 1 fresh.
        let key = random_epoch_key();
        submit_group_epoch_key_v1(server_url, seed, group_id, 1, &key, &sealable)?;
        return Ok(Some((1, key, added)));
    }

    // PRIVATE (default): mint a NEW epoch sealed to the full roster — members who
    // joined since the last epoch read from here forward only (forward secrecy).
    let new_epoch = current_epoch + 1;
    let new_key = random_epoch_key();
    submit_group_epoch_key_v1(server_url, seed, group_id, new_epoch, &new_key, &sealable)?;
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

/// Fetch + unseal EVERY epoch key I was sealed into (`GET .../epochs`). Returns
/// `epoch → 32-byte key` for every epoch I can open. The message log spans
/// epochs after a re-key, so the full set is needed to decrypt the whole
/// history (each message opens under the key for its own epoch). An existing
/// member is sealed into every epoch (full history); a later joiner is only
/// sealed from their join epoch on (forward secrecy — they simply can't open
/// pre-join epochs, the intended default). Best-effort per-epoch: an epoch we
/// can't open (no recipient entry) is skipped, not fatal.
pub fn fetch_all_epoch_keys(
    server_url: &str,
    seed: &[u8],
    group_id: &str,
) -> Result<std::collections::HashMap<u64, Vec<u8>>, String> {
    let identity = derive_pq_identity(seed).map_err(|e| format!("derive: {e}"))?;
    let dil_pub_bytes =
        hex::decode(&identity.dilithium_hex).map_err(|e| format!("dilithium hex: {e}"))?;
    let my_fp = author_fingerprint_hex(&dil_pub_bytes);
    let me = DmPqKeypair::from_bip39_seed(seed).map_err(|e| format!("dm keypair: {e}"))?;

    let url = format!(
        "{}/api/v2/groups/{}/epochs",
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
    let epochs = v
        .get("epochs")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = std::collections::HashMap::new();
    for eo in epochs {
        let payload_b64 = match eo.get("payload_b64").and_then(|x| x.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let payload = match B64.decode(payload_b64) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let parsed = match parse_group_epoch_key_payload(&payload) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Ok((epoch, key)) = open_epoch_key(&parsed, &my_fp, &me) {
            out.insert(epoch, key);
        }
    }
    Ok(out)
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
    /// Whether I'm this group's creator — gates the "Disband" action (anyone
    /// can Leave; only the creator can Disband). Computed by the relay.
    pub is_creator: bool,
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
        let is_creator = g.get("is_creator").and_then(|v| v.as_bool()).unwrap_or(false);
        out.push(P2pGroupInfo { group_id, name, members, is_creator });
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

    /// The inc-2 P2P push round-trip: build+sign a group_msg_v1 → serialize to
    /// the submission JSON the relay accepts → verify it as a peer would on the
    /// receive side. The verified object_id MUST equal the build-side id, the
    /// author fields resolve, and the payload decrypts under the same epoch key.
    /// This locks the build helper and the security gate against drift.
    #[test]
    fn build_then_verify_group_msg_roundtrip() {
        let seed = [42u8; 32];
        let epoch_key = crate::net::group_e2ee::random_epoch_key();
        let (build_id, submission_json) =
            build_group_msg_submission(&seed, "grp_abc", 3, &epoch_key, "hello mesh")
                .expect("build submission");

        let verified = verify_submission_json(&submission_json).expect("verifies");
        assert_eq!(verified.object_id, build_id, "recomputed id == build id");
        assert_eq!(verified.object_type, "group_msg_v1");
        assert_eq!(verified.references.first().map(|s| s.as_str()), Some("grp_abc"));

        // Author fields resolve from the seed's Dilithium identity.
        let identity = derive_pq_identity(&seed).unwrap();
        assert_eq!(verified.author_pubkey_hex, identity.dilithium_hex);
        let my_pub = hex::decode(&identity.dilithium_hex).unwrap();
        assert_eq!(verified.author_fp, author_fingerprint_hex(&my_pub));

        // The verified payload decrypts under the epoch key.
        let text = crate::net::group_e2ee::open_group_msg(&verified.payload, &epoch_key)
            .expect("decrypts");
        assert_eq!(text, "hello mesh");
    }

    /// A tampered submission must be REJECTED by the gate. Flipping a payload
    /// byte breaks the canonical bytes the signature covers, so verify fails.
    #[test]
    fn verify_rejects_tampered_submission() {
        let seed = [7u8; 32];
        let epoch_key = crate::net::group_e2ee::random_epoch_key();
        let (_id, submission_json) =
            build_group_msg_submission(&seed, "grp_x", 1, &epoch_key, "secret")
                .expect("build");

        // Tamper: re-encode the payload_b64 with a different ciphertext (a
        // fresh encryption of other text). The signature no longer covers it.
        let mut v: serde_json::Value = serde_json::from_str(&submission_json).unwrap();
        let (_id2, other_json) =
            build_group_msg_submission(&seed, "grp_x", 1, &epoch_key, "DIFFERENT")
                .expect("build2");
        let other: serde_json::Value = serde_json::from_str(&other_json).unwrap();
        v["payload_b64"] = other["payload_b64"].clone(); // swap payload, keep old sig
        let tampered = v.to_string();

        assert!(verify_submission_json(&tampered).is_none(), "tampered → rejected");
        // Garbage in → None, never a panic.
        assert!(verify_submission_json("not json").is_none());
        assert!(verify_submission_json("{}").is_none());
    }
}
