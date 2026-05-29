// pq-object.js — build + sign Humanity Network signed objects in the browser.
//
// Mirrors src/relay/core/object.rs (ObjectBuilder + signable_bytes + object_id).
// Produces the exact submission shape the relay's POST /api/v2/objects expects
// (see web/shared/pq-identity.js postObject), so a web-built object verifies on
// the relay via put_signed_object → Object::verify_signature.
//
// Correctness rests on web/shared/canonical-cbor.js being byte-identical to the
// Rust encoder, which scripts/group-object-kat.mjs locks against the Rust golden.
//
// This is generic (any object_type); the group_* helpers are the P2P-groups
// payloads (docs/design/p2p-groups.md). The same builder serves votes, vouches,
// recovery shares, etc. — all currently blocked on web-side object construction.
//
// Crypto is INJECTED (sign + blake3) so this module stays pure + unit-testable
// and doesn't hard-bind to a CDN vs vendored bundle. The chat client wires the
// real Dilithium sign (window.pqSignMessage / HumOS.pq.pqSign) + blake3.

import { encodeObjectCanonical, cborText, cborBytes, cborMap, cborUint, cborArray, decodeCanonicalCbor } from './canonical-cbor.js';

const DILITHIUM_SIG_LEN = 3309;
const PAYLOAD_ENCODING_PLAINTEXT = 'cbor_canonical_v1';

function _b64(u8) {
  let s = '';
  for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]);
  return btoa(s);
}
function _b64d(s) {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
function _hex(u8) {
  return [...u8].map((x) => x.toString(16).padStart(2, '0')).join('');
}

/**
 * Build + sign a signed object. Returns `{ objectId, submission }` where
 * `submission` is ready to POST to /api/v2/objects (HumOS.pq.postObject).
 *
 * opts:
 *   objectType            (string, required)
 *   payload               (Uint8Array canonical CBOR, required)
 *   authorPublicKey       (Uint8Array, 1952 bytes, required)
 *   sign                  (async (Uint8Array) => Uint8Array 3309-byte sig, required)
 *   blake3                (Uint8Array => Uint8Array 32-byte hash, required)
 *   spaceId, channelId    (string, optional)
 *   createdAt             (number ms, optional but recommended)
 *   references            (string[] of object_id hex; default [])
 *   payloadSchemaVersion  (number, default 1)
 *   payloadEncoding       (string, default "cbor_canonical_v1")
 */
export async function buildSignedObject(opts) {
  const base = {
    protocol_version: 1,
    object_type: opts.objectType,
    space_id: opts.spaceId ?? null,
    channel_id: opts.channelId ?? null,
    author_public_key: opts.authorPublicKey,
    created_at: opts.createdAt ?? null,
    references: opts.references || [],
    payload_schema_version: opts.payloadSchemaVersion ?? 1,
    payload_encoding: opts.payloadEncoding || PAYLOAD_ENCODING_PLAINTEXT,
    payload: opts.payload,
    // Zero-filled placeholder — matches Rust signable_bytes() so the relay
    // recomputes the SAME bytes when it verifies.
    signature: new Uint8Array(DILITHIUM_SIG_LEN),
  };

  const signableBytes = encodeObjectCanonical(base);
  const signature = await opts.sign(signableBytes);
  if (!(signature instanceof Uint8Array) || signature.length !== DILITHIUM_SIG_LEN) {
    throw new Error(`pq-object: sign() must return a ${DILITHIUM_SIG_LEN}-byte Uint8Array`);
  }

  const signed = { ...base, signature };
  const canon = encodeObjectCanonical(signed);
  const objectId = _hex(opts.blake3(canon));

  const submission = {
    protocol_version: 1,
    object_type: opts.objectType,
    author_public_key_b64: _b64(opts.authorPublicKey),
    references: opts.references || [],
    payload_schema_version: opts.payloadSchemaVersion ?? 1,
    payload_encoding: opts.payloadEncoding || PAYLOAD_ENCODING_PLAINTEXT,
    payload_b64: _b64(opts.payload),
    signature_b64: _b64(signature),
  };
  if (opts.spaceId != null) submission.space_id = opts.spaceId;
  if (opts.channelId != null) submission.channel_id = opts.channelId;
  if (opts.createdAt != null) submission.created_at = opts.createdAt;

  return { objectId, submission };
}

/**
 * Verify a signed-object SUBMISSION (the POST /api/v2/objects shape) LOCALLY —
 * the exact ML-DSA-65 check the relay runs in `put_signed_object`, so a peer
 * can trust an object received over a P2P DataChannel with ZERO relay
 * involvement (P2P-groups Phase 3). Recomputes `signable_bytes` (canonical CBOR
 * with a zero-filled signature, exactly as `buildSignedObject` does) and the
 * `object_id` (BLAKE3 of the canonical bytes with the real signature).
 *
 * Returns `{ ok, objectId, payload, authorPubHex, createdAt, references }` on a
 * valid signature, or `{ ok: false }` on anything malformed/forged. Never throws.
 *
 * @param submission the JSON object as produced by buildSignedObject / returned by the relay
 * @param deps `{ blake3: (Uint8Array)=>Uint8Array, pqVerify: async (pubBytes, msgBytes, sigBytes)=>bool }`
 */
export async function verifyObjectSubmission(submission, { blake3, pqVerify }) {
  try {
    if (!submission || !submission.author_public_key_b64 || !submission.payload_b64 || !submission.signature_b64) {
      return { ok: false };
    }
    const authorPub = _b64d(submission.author_public_key_b64);
    const payload = _b64d(submission.payload_b64);
    const signature = _b64d(submission.signature_b64);
    if (signature.length !== DILITHIUM_SIG_LEN) return { ok: false };

    const base = {
      protocol_version: submission.protocol_version ?? 1,
      object_type: submission.object_type,
      space_id: submission.space_id ?? null,
      channel_id: submission.channel_id ?? null,
      author_public_key: authorPub,
      created_at: submission.created_at ?? null,
      references: submission.references || [],
      payload_schema_version: submission.payload_schema_version ?? 1,
      payload_encoding: submission.payload_encoding || PAYLOAD_ENCODING_PLAINTEXT,
      payload,
      // Zero-filled signature for the signable preimage — must match Rust.
      signature: new Uint8Array(DILITHIUM_SIG_LEN),
    };
    const signableBytes = encodeObjectCanonical(base);
    const ok = await pqVerify(authorPub, signableBytes, signature);
    if (!ok) return { ok: false };

    const objectId = _hex(blake3(encodeObjectCanonical({ ...base, signature })));
    return {
      ok: true,
      objectId,
      payload,
      authorPubHex: _hex(authorPub),
      createdAt: submission.created_at ?? null,
      references: base.references,
    };
  } catch (_e) {
    return { ok: false };
  }
}

/* ── P2P group payloads (docs/design/p2p-groups.md object-format spec) ── */

/** `group_v1` payload: `{ name }`, plus `share_history: 1` ONLY when the group
 *  shares its message history with members who join later. Omitting the field
 *  (the private default) keeps the byte-for-byte encoding of pre-history-toggle
 *  groups, so old group_ids + the canonical KAT are unaffected. `share_history`
 *  is part of the SIGNED object so the policy is tamper-proof + travels with the
 *  group (multi-device / future multi-admin all see the same choice).
 *  The group_id = the resulting object's id. */
export function groupV1Payload(name, shareHistory) {
  const pairs = [[cborText('name'), cborText(name)]];
  if (shareHistory) pairs.push([cborText('share_history'), cborUint(1)]);
  return cborMap(pairs); // cborMap sorts keys canonically — insertion order is irrelevant
}

/** `group_member_v1` payload: `{ action: "admit"|"remove", subject: <pubkey> }`. */
export function groupMemberV1Payload(action, subjectPubkey) {
  return cborMap([
    [cborText('action'), cborText(action)],
    [cborText('subject'), cborBytes(subjectPubkey)],
  ]);
}

/**
 * Convenience: build + sign a `group_v1` object.
 * Returns `{ objectId (= the group id), submission }`.
 */
export async function buildGroupV1({ name, shareHistory, authorPublicKey, sign, blake3, createdAt }) {
  return buildSignedObject({
    objectType: 'group_v1',
    payload: groupV1Payload(name, shareHistory),
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/** Read `share_history` from a fetched group_v1 object (via GET /api/v2/objects/{id},
 *  whose `payload_b64` is the canonical CBOR payload). Returns true only if the
 *  group explicitly opted into sharing history; absent/0 → false (private). */
export function groupSharesHistory(payloadBytes) {
  try {
    const p = decodeCanonicalCbor(payloadBytes);
    return !!(p && p.share_history);
  } catch (_e) { return false; }
}

/**
 * Convenience: build + sign a `group_member_v1` admit/remove entry referencing
 * `groupId`. Phase 1: must be signed by the group creator to take effect.
 */
export async function buildGroupMemberV1({ groupId, action, subjectPubkey, authorPublicKey, sign, blake3, createdAt }) {
  return buildSignedObject({
    objectType: 'group_member_v1',
    payload: groupMemberV1Payload(action, subjectPubkey),
    references: [groupId],
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/**
 * Build + sign a `group_disband_v1`: the creator tears the whole group down.
 * References [groupId]. The relay honors it only if the author is the group
 * creator — it then hides the group from every member's list. Empty payload
 * (the group_id reference + creator signature carry all the meaning).
 */
export async function buildGroupDisbandV1({ groupId, authorPublicKey, sign, blake3, createdAt }) {
  return buildSignedObject({
    objectType: 'group_disband_v1',
    payload: cborMap([]),
    references: [groupId],
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/** `group_invite_v1` payload: `{ expires_at, secret_hash }` (creator-signed). */
export function groupInviteV1Payload(secretHash, expiresAt) {
  return cborMap([
    [cborText('expires_at'), cborUint(expiresAt)],
    [cborText('secret_hash'), cborBytes(secretHash)],
  ]);
}

/** `group_join_v1` payload: `{ secret }` (joiner reveals the invite secret). */
export function groupJoinV1Payload(secret) {
  return cborMap([[cborText('secret'), cborBytes(secret)]]);
}

/** A fresh 32-byte invite secret. */
export function randomInviteSecret() {
  return crypto.getRandomValues(new Uint8Array(32));
}

/**
 * Build + sign a `group_invite_v1` capability. The creator signs a commitment
 * to `BLAKE3(secret)` + an expiry; the ticket (below) carries the secret
 * out-of-band so a holder can self-admit without the creator online.
 * Returns `{ objectId (= the invite id), submission }`.
 */
export async function buildGroupInviteV1({ groupId, secret, expiresAt, authorPublicKey, sign, blake3, createdAt }) {
  const secretHash = blake3(secret);
  return buildSignedObject({
    objectType: 'group_invite_v1',
    payload: groupInviteV1Payload(secretHash, expiresAt),
    references: [groupId],
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/**
 * Build + sign a `group_join_v1`: self-admission by revealing the invite secret.
 * References [groupId, inviteId]. The relay/peers admit the join author iff the
 * secret matches the creator-signed invite and it hasn't expired.
 */
export async function buildGroupJoinV1({ groupId, inviteId, secret, authorPublicKey, sign, blake3, createdAt }) {
  return buildSignedObject({
    objectType: 'group_join_v1',
    payload: groupJoinV1Payload(secret),
    references: [groupId, inviteId],
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/* ── Phase 2: E2EE group messages ──────────────────────────────────────────
 * docs/design/p2p-groups.md Phase 2. The relay stores opaque ciphertext and
 * never holds the group key. One 32-byte epoch key per epoch, sealed once to
 * each member's Kyber pub (same ML-KEM-768 → BLAKE3-KDF → AES-256-GCM as the
 * DM envelope in dm_pq.rs, so a member opens their copy with pqDmOpen). On
 * membership change a new epoch is minted; removed members are cut off from
 * future messages (forward secrecy across churn). Messages are AES-GCM under
 * the current epoch key. */

/** Fresh random 32-byte epoch (AES-256) key for a group. */
export function randomEpochKey() {
  return crypto.getRandomValues(new Uint8Array(32));
}

/**
 * Build + sign a `group_epoch_key_v1` — the same 32-byte epoch key sealed
 * (ML-KEM-768 envelope, matching dm_pq.rs / pq.js pqDmSeal) to each member's
 * Kyber pub. Each member finds their `fp` entry and opens it.
 *
 * opts.members: [{ fp: hex, kyber_public: base64 }, ...]
 * opts.epochKey: Uint8Array(32) (use `randomEpochKey()`).
 * opts.seal: async (kyberPubB64, plaintext) => { ek_ct_b64, nonce_b64, ct_b64 }
 *            — pass `window.pqDmSeal` (same scheme as DMs).
 */
export async function buildGroupEpochKeyV1({ groupId, epoch, epochKey, members, seal, authorPublicKey, sign, blake3, createdAt }) {
  // Seal the epoch key (as base64, since pqDmSeal takes a string) to every
  // member who has a Kyber pub registered. Members without one are skipped
  // — they simply can't decrypt this epoch (they'd need to register).
  const epochKeyB64 = _b64(epochKey);
  const recipients = [];
  for (const m of (members || [])) {
    if (!m || !m.kyber_public) continue;
    const env = await seal(m.kyber_public, epochKeyB64);
    if (!env) continue;
    recipients.push(cborMap([
      [cborText('fp'), cborText(m.fp || '')],
      [cborText('ek_ct'), cborBytes(_b64d(env.ek_ct_b64))],
      [cborText('nonce'), cborBytes(_b64d(env.nonce_b64))],
      [cborText('ct'), cborBytes(_b64d(env.ct_b64))],
    ]));
  }
  const payload = cborMap([
    [cborText('epoch'), cborUint(epoch)],
    [cborText('recipients'), cborArray(recipients)],
  ]);
  return buildSignedObject({
    objectType: 'group_epoch_key_v1',
    payload,
    references: [groupId],
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/**
 * Open MY copy of an epoch-key object's payload — find the recipient entry
 * whose `fp` matches my Kyber/Dilithium fingerprint and decapsulate it.
 *
 * payloadBytes: the raw `payload` bytes from a SignedObjectResponse (b64-decoded).
 * myFp: my author fingerprint (= first 16 bytes of BLAKE3(my Dilithium pubkey), hex).
 * open: async (myKyberSecret, ek_ct_b64, nonce_b64, ct_b64) => plaintextStr
 *       — pass `window.pqDmOpen`.
 * myKyberSecret: my Kyber768 secret key (the same `myKyberSecret` the DM
 *                opener uses, from crypto.js).
 * Returns `{ epoch, epochKey }` or null if our entry is missing or the open fails.
 */
export async function openGroupEpochKey(payloadBytes, myFp, open, myKyberSecret) {
  let payload;
  try { payload = decodeCanonicalCbor(payloadBytes); }
  catch (e) { return null; }
  if (!payload || typeof payload !== 'object') return null;
  const epoch = payload.epoch;
  const recipients = Array.isArray(payload.recipients) ? payload.recipients : [];
  for (const r of recipients) {
    if (!r || r.fp !== myFp) continue;
    const ek_ct_b64 = _b64(r.ek_ct);
    const nonce_b64 = _b64(r.nonce);
    const ct_b64 = _b64(r.ct);
    const plain = await open(myKyberSecret, ek_ct_b64, nonce_b64, ct_b64);
    if (!plain) return null;
    let epochKey;
    try { epochKey = _b64d(plain); } catch (e) { return null; }
    if (epochKey.length !== 32) return null;
    return { epoch, epochKey };
  }
  return null;
}

async function _aesKey(rawKey32) {
  return crypto.subtle.importKey('raw', rawKey32, { name: 'AES-GCM' }, false, ['encrypt', 'decrypt']);
}

/** AES-256-GCM encrypt a UTF-8 string under the epoch key. Returns `{nonce, ct}`. */
export async function aesGcmEncrypt(epochKey, plaintext) {
  const key = await _aesKey(epochKey);
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv: nonce }, key, new TextEncoder().encode(plaintext),
  ));
  return { nonce, ct };
}

/** AES-256-GCM decrypt → UTF-8 string, or null on failure (wrong key / tampered). */
export async function aesGcmDecrypt(epochKey, nonce, ct) {
  try {
    const key = await _aesKey(epochKey);
    const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: nonce }, key, ct);
    return new TextDecoder().decode(plain);
  } catch (e) {
    return null;
  }
}

/**
 * Build + sign a `group_msg_v1` — AES-GCM ciphertext under the epoch key.
 * Payload = canonical CBOR `{epoch, nonce, ct}`. The relay can't read it.
 */
export async function buildGroupMsgV1({ groupId, epoch, epochKey, plaintext, authorPublicKey, sign, blake3, createdAt }) {
  const { nonce, ct } = await aesGcmEncrypt(epochKey, plaintext);
  const payload = cborMap([
    [cborText('epoch'), cborUint(epoch)],
    [cborText('nonce'), cborBytes(nonce)],
    [cborText('ct'), cborBytes(ct)],
  ]);
  return buildSignedObject({
    objectType: 'group_msg_v1',
    payload,
    references: [groupId],
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
}

/**
 * Parse a `group_msg_v1` payload back to `{epoch, nonce, ct}` (Uint8Arrays).
 * Pair with `aesGcmDecrypt(epochKey, nonce, ct)` to get the plaintext.
 */
export function parseGroupMsgPayload(payloadBytes) {
  let p;
  try { p = decodeCanonicalCbor(payloadBytes); }
  catch (e) { return null; }
  if (!p || typeof p !== 'object') return null;
  return { epoch: p.epoch, nonce: p.nonce, ct: p.ct };
}

/**
 * Parse a `group_epoch_key_v1` payload back to `{epoch, recipients}` where
 * each recipient is `{fp, ek_ct, nonce, ct}` (binary fields as Uint8Array).
 * Used by the creator's rekey-on-join logic to inspect who's already covered.
 */
export function parseGroupEpochKeyPayload(payloadBytes) {
  let p;
  try { p = decodeCanonicalCbor(payloadBytes); }
  catch (e) { return null; }
  if (!p || typeof p !== 'object') return null;
  const recipients = Array.isArray(p.recipients) ? p.recipients : [];
  return { epoch: p.epoch, recipients };
}

/* ── Connection ticket (shared out-of-band: copy/paste or QR) ──
 * Phase 1 carries what a joiner needs to self-admit through any relay:
 * group id + name, the invite id, and the secret. (Phase 4 adds bootstrap
 * peers + kyber pubs for relay-free connection.) It is NOT itself signed —
 * its authority is the creator-signed group_invite_v1 it points at, which the
 * relay/peers already hold and verify. */
export function encodeInviteTicket({ groupId, groupName, inviteId, secret }) {
  const obj = {
    v: 1,
    g: groupId,
    n: groupName,
    i: inviteId,
    s: _b64(secret),
  };
  const json = JSON.stringify(obj);
  // base64url so it is URL/QR-safe.
  return btoa(json).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function decodeInviteTicket(str) {
  const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const json = atob(b64);
  const o = JSON.parse(json);
  if (!o || o.v !== 1 || !o.g || !o.i || !o.s) throw new Error('invalid invite ticket');
  const secret = Uint8Array.from(atob(o.s), (c) => c.charCodeAt(0));
  return { groupId: o.g, groupName: o.n || '', inviteId: o.i, secret };
}
