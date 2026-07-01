#!/usr/bin/env node
/**
 * vote-object-kat.mjs — cross-language known-answer test for the WEB governance
 * voting path (web/pages/governance.html): canonical CBOR + signed-object
 * encoding + Dilithium3 signing for a fixed vote_v1 input.
 *
 * Asserts, against the Rust goldens in
 * src/relay/core/object.rs::vote_v1_cross_language_kat (duplicated here on
 * purpose — editing the encoding must break BOTH or neither):
 *   1. payload bytes ({choice:"yes"} canonical CBOR) are byte-identical,
 *   2. the SIGNABLE bytes (canonical object with a zero-filled 3309-byte
 *      signature) are byte-identical (length + BLAKE3),
 *   3. the frozen deterministic RUST signature fixture
 *      (src/relay/core/pq_kat_vote_sig.hex) VERIFIES under noble over the
 *      JS-computed signable bytes — a Dilithium verify binds the exact message
 *      bytes, so a pass is cryptographic proof of byte equality,
 *   4. splicing that Rust signature into the JS object reproduces the Rust
 *      object_id exactly (full-object byte equality),
 *   5. a fresh noble signature over the same bytes round-trips (the browser
 *      signing path; RustCrypto accepting noble signatures is already locked
 *      by pq_crypto.rs::dilithium_js_signature_verifies_in_rust).
 *
 * If this fails, a web-cast governance vote would be rejected by the relay
 * (401 signature verification failed) — DO NOT SHIP.
 *
 * Run: `node scripts/vote-object-kat.mjs` (or `just vote-kat`).
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const CBOR = join(REPO, 'web', 'shared', 'canonical-cbor.js');
const OBJ = join(REPO, 'web', 'shared', 'pq-object.js');
const BUNDLE = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');
const RUST_SIG_FIXTURE = join(REPO, 'src', 'relay', 'core', 'pq_kat_vote_sig.hex');

// GOLDEN — must equal src/relay/core/object.rs::VOTE_KAT_*.
const KAT_PROPOSAL_ID =
  '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const KAT_CREATED_AT = 1_751_328_000_000;
const GOLDEN_PAYLOAD_HEX = 'a16663686f69636563796573';
const GOLDEN_SIGNABLE_LEN = 5512;
const GOLDEN_SIGNABLE_BLAKE3 =
  '053292d63bce79f4c936256509a95470d59cb416470c2d7416b136052a29cdb1';
const GOLDEN_OBJECT_ID =
  '375d12d51758c7b5a83e82f79dee29e1543e381192eacf138979841bc0b97e76';
// Same identity as pq-kat.mjs / pq_crypto.rs::dilithium_cross_language_kat.
const KAT_MASTER = new Uint8Array(32).fill(7);
const KAT_PK_BLAKE3 =
  '3f4ff5c7e6505ca7b0dd6cb32c53839f8cff19772e291d4f18b082d1f7dc0126';

const hx = (b) => [...b].map((x) => x.toString(16).padStart(2, '0')).join('');
const unhex = (s) => Uint8Array.from(s.match(/.{2}/g), (h) => parseInt(h, 16));
function fail(msg) { console.error(`vote-object-kat: FAIL — ${msg}`); process.exit(1); }

try {
  const { cborText, cborMap } = await import(pathToFileURL(CBOR).href);
  const { buildVoteV1, voteV1Payload } = await import(pathToFileURL(OBJ).href);
  const noble = await import(pathToFileURL(BUNDLE).href);
  if (!noble.ml_dsa65 || !noble.blake3) fail('vendored bundle missing ml_dsa65/blake3 — run `just pq-vendor`');
  const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();

  // Derive the KAT identity exactly as the chat/governance pages do
  // (pq.js pqDeriveIdentity): BLAKE3 derive_key("hum/dilithium3/v1") → keygen.
  const ctx = new TextEncoder().encode('hum/dilithium3/v1');
  const dilSeed = noble.blake3.create({ context: ctx, dkLen: 32 }).update(KAT_MASTER).digest();
  const kp = noble.ml_dsa65.keygen(dilSeed);
  if (hx(blake3(kp.publicKey)) !== KAT_PK_BLAKE3) {
    fail('KAT keypair drifted — run `just pq-kat` first, its failure is the root cause');
  }

  // 1) Payload bytes.
  const payloadHex = hx(voteV1Payload('yes'));
  if (payloadHex !== GOLDEN_PAYLOAD_HEX) {
    fail(`vote payload encoding mismatch\n  got      ${payloadHex}\n  expected ${GOLDEN_PAYLOAD_HEX}`);
  }
  // voteV1Payload must reject anything the relay would misindex as "abstain".
  let threw = false;
  try { voteV1Payload('YES'); } catch (_e) { threw = true; }
  if (!threw) fail('voteV1Payload accepted an invalid choice');

  // 2+4) Build the vote with a capturing signer that returns the frozen Rust
  // signature — the built object must then be byte-identical to Rust's
  // (same signable bytes AND same object_id).
  const rustSig = unhex(readFileSync(RUST_SIG_FIXTURE, 'utf8').trim());
  if (rustSig.length !== 3309) fail(`Rust signature fixture wrong length: ${rustSig.length}`);
  let signable = null;
  const built = await buildVoteV1({
    proposalId: KAT_PROPOSAL_ID,
    choice: 'yes',
    authorPublicKey: kp.publicKey,
    sign: async (msg) => { signable = msg; return rustSig; },
    blake3,
    createdAt: KAT_CREATED_AT,
  });
  if (!signable) fail('buildVoteV1 never called sign()');
  if (signable.length !== GOLDEN_SIGNABLE_LEN) {
    fail(`signable length mismatch: got ${signable.length}, expected ${GOLDEN_SIGNABLE_LEN}`);
  }
  const signableB3 = hx(blake3(signable));
  if (signableB3 !== GOLDEN_SIGNABLE_BLAKE3) {
    fail(`signable bytes mismatch (canonical encoding drifted web↔native)\n  got      ${signableB3}\n  expected ${GOLDEN_SIGNABLE_BLAKE3}`);
  }
  if (built.objectId !== GOLDEN_OBJECT_ID) {
    fail(`object_id mismatch\n  got      ${built.objectId}\n  expected ${GOLDEN_OBJECT_ID}`);
  }

  // 3) The Rust signature must VERIFY over the JS-computed signable bytes
  // (cryptographic byte-equality proof, Rust-sign → JS-verify direction).
  if (!noble.ml_dsa65.verify(rustSig, signable, kp.publicKey)) {
    fail('frozen Rust signature does NOT verify over the JS signable bytes');
  }

  // 5) The real browser path: noble signs the same bytes and it round-trips.
  const jsSig = noble.ml_dsa65.sign(signable, kp.secretKey);
  if (jsSig.length !== 3309) fail(`noble signature wrong length: ${jsSig.length}`);
  if (!noble.ml_dsa65.verify(jsSig, signable, kp.publicKey)) {
    fail('noble sign/verify round-trip over the vote signable bytes failed');
  }

  // Submission wire-shape sanity (the relay's SignedObjectSubmission fields).
  const s = built.submission;
  for (const k of ['protocol_version', 'object_type', 'author_public_key_b64',
    'references', 'payload_schema_version', 'payload_encoding', 'payload_b64',
    'signature_b64', 'created_at']) {
    if (!(k in s)) fail(`submission missing field ${k}`);
  }
  if (s.object_type !== 'vote_v1') fail('submission object_type wrong');
  if (s.references.length !== 1 || s.references[0] !== KAT_PROPOSAL_ID) {
    fail('submission references must be [proposal_object_id]');
  }
  if (hx(Uint8Array.from(Buffer.from(s.payload_b64, 'base64'))) !== GOLDEN_PAYLOAD_HEX) {
    fail('submission payload_b64 does not decode to the golden payload');
  }

  // Encoder unit vector from src/relay/core/encoding.rs::canonical_map_key_ordering:
  // {z:1, a:2, bb:3} sorts a, z, bb (shorter encoded key first, then bytewise).
  const sortHex = hx(cborMap([
    [cborText('z'), new Uint8Array([0x01])],
    [cborText('a'), new Uint8Array([0x02])],
    [cborText('bb'), new Uint8Array([0x03])],
  ]));
  if (sortHex !== 'a3616102617a0162626203') {
    fail(`map key ordering drifted\n  got      ${sortHex}\n  expected a3616102617a0162626203`);
  }

  console.log('vote-object-kat: PASS — web vote_v1 == Rust byte-for-byte (payload, signable bytes, object_id); Rust sig verifies over JS bytes; noble sign round-trips');
  process.exit(0);
} catch (e) {
  fail(e && e.stack ? e.stack : String(e));
}
