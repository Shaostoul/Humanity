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

import { encodeObjectCanonical, cborText, cborBytes, cborMap } from './canonical-cbor.js';

const DILITHIUM_SIG_LEN = 3309;
const PAYLOAD_ENCODING_PLAINTEXT = 'cbor_canonical_v1';

function _b64(u8) {
  let s = '';
  for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]);
  return btoa(s);
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

/* ── P2P group payloads (docs/design/p2p-groups.md object-format spec) ── */

/** `group_v1` payload: `{ name }`. The group_id = the resulting object's id. */
export function groupV1Payload(name) {
  return cborMap([[cborText('name'), cborText(name)]]);
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
export async function buildGroupV1({ name, authorPublicKey, sign, blake3, createdAt }) {
  return buildSignedObject({
    objectType: 'group_v1',
    payload: groupV1Payload(name),
    authorPublicKey, sign, blake3,
    createdAt: createdAt ?? Date.now(),
  });
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
