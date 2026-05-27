#!/usr/bin/env node
/**
 * group-object-kat.mjs — cross-language known-answer test for the canonical
 * CBOR + signed-object encoding the web client uses to build group objects
 * (and, in time, all web-built signed objects).
 *
 * Asserts that web/shared/canonical-cbor.js reproduces, byte-for-byte, the
 * Rust encoder in src/relay/core/{encoding,object}.rs for a fixed group_v1
 * input. The two GOLDEN constants below are duplicated from the Rust test
 * src/relay/storage/groups_p2p.rs::group_v1_canonical_kat on purpose — editing
 * the encoding must break BOTH or neither. If this fails, a web-built group
 * object would be unverifiable by the relay — DO NOT SHIP.
 *
 * Run: `node scripts/group-object-kat.mjs` (or `just group-kat`).
 */
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const CBOR = join(REPO, 'web', 'shared', 'canonical-cbor.js');
const OBJ = join(REPO, 'web', 'shared', 'pq-object.js');
const BUNDLE = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

// GOLDEN — must equal src/relay/storage/groups_p2p.rs::{GOLDEN_PAYLOAD_HEX, GOLDEN_OBJECT_ID}.
const GOLDEN_PAYLOAD_HEX = 'a1646e616d65696b61742d67726f7570';
const GOLDEN_OBJECT_ID =
  'c909a8dfa825419c4034608b6f6482b883c15d3cbf88d1f1c76b01fe70f7db9b';

const hx = (b) => [...b].map((x) => x.toString(16).padStart(2, '0')).join('');
function fail(msg) { console.error(`group-object-kat: FAIL — ${msg}`); process.exit(1); }

try {
  const { cborText, cborMap, encodeObjectCanonical } = await import(pathToFileURL(CBOR).href);
  const noble = await import(pathToFileURL(BUNDLE).href);
  if (!noble.blake3) fail('vendored bundle missing blake3 export — run `just pq-vendor`');
  const blake3 = (data) => noble.blake3.create({ dkLen: 32 }).update(data).digest();

  // Fixed, language-neutral input (mirrors the Rust KAT exactly).
  const payload = cborMap([[cborText('name'), cborText('kat-group')]]);
  const payloadHex = hx(payload);
  if (payloadHex !== GOLDEN_PAYLOAD_HEX) {
    fail(`payload encoding mismatch\n  got      ${payloadHex}\n  expected ${GOLDEN_PAYLOAD_HEX}`);
  }

  const author_public_key = new Uint8Array(1952);
  for (let i = 0; i < author_public_key.length; i++) author_public_key[i] = i % 256;
  const signature = new Uint8Array(3309);
  for (let i = 0; i < signature.length; i++) signature[i] = i % 256;

  const canon = encodeObjectCanonical({
    protocol_version: 1,
    object_type: 'group_v1',
    space_id: null,
    channel_id: null,
    author_public_key,
    created_at: 1000,
    references: [],
    payload_schema_version: 1,
    payload_encoding: 'cbor_canonical_v1',
    payload,
    signature,
  });
  const objectId = hx(blake3(canon));
  if (objectId !== GOLDEN_OBJECT_ID) {
    fail(`object_id mismatch (canonical encoding drifted web↔native)\n  got      ${objectId}\n  expected ${GOLDEN_OBJECT_ID}\n  canon_len ${canon.length}`);
  }

  // Exercise the higher-level builder end-to-end (groupV1Payload + assembly +
  // object_id) with a stub signer returning the same fixed signature, so the
  // whole pq-object.js path is proven to yield the golden id too.
  const { buildGroupV1 } = await import(pathToFileURL(OBJ).href);
  const built = await buildGroupV1({
    name: 'kat-group',
    authorPublicKey: author_public_key,
    sign: async () => signature,
    blake3,
    createdAt: 1000,
  });
  if (built.objectId !== GOLDEN_OBJECT_ID) {
    fail(`buildGroupV1 object_id mismatch\n  got      ${built.objectId}\n  expected ${GOLDEN_OBJECT_ID}`);
  }

  console.log('group-object-kat: PASS — web canonical CBOR == Rust (payload, object_id, buildGroupV1)');
  process.exit(0);
} catch (e) {
  fail(e && e.stack ? e.stack : String(e));
}
