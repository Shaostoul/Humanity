#!/usr/bin/env node
/**
 * object-verify-kat.mjs — round-trip test for `verifyObjectSubmission`
 * (web/shared/pq-object.js), the P2P-groups Phase 3 receive-side LOCAL verify:
 * a peer trusting a signed object pushed over a WebRTC DataChannel runs this
 * instead of the relay. It MUST accept a genuine object (and recompute the same
 * object_id) and MUST reject any tamper — otherwise a peer could accept a forged
 * group message (or drop valid ones). This is the security-critical primitive,
 * and the only Phase-3 piece testable outside a browser, so it's locked here.
 *
 * Run: `node scripts/object-verify-kat.mjs`
 */
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const OBJ = join(REPO, 'web', 'shared', 'pq-object.js');
const BUNDLE = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

function fail(msg) { console.error(`object-verify-kat: FAIL — ${msg}`); process.exit(1); }

try {
  const { buildGroupV1, verifyObjectSubmission } = await import(pathToFileURL(OBJ).href);
  const noble = await import(pathToFileURL(BUNDLE).href);
  if (!noble.ml_dsa65 || !noble.blake3) fail('vendored bundle missing ml_dsa65/blake3 — run `just pq-vendor`');

  const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
  // Deterministic ML-DSA-65 keypair (test seed; not a real identity).
  const seed = new Uint8Array(32);
  for (let i = 0; i < 32; i++) seed[i] = (i * 7 + 1) % 256;
  const kp = noble.ml_dsa65.keygen(seed);                       // { publicKey:1952B, secretKey }
  // Arg order mirrors web/chat/pq.js: sign(message, secretKey), verify(sig, message, pub).
  const sign = async (msg) => noble.ml_dsa65.sign(msg, kp.secretKey);
  const pqVerify = async (pub, msg, sig) => !!noble.ml_dsa65.verify(sig, msg, pub);

  const built = await buildGroupV1({
    name: 'verify-kat',
    authorPublicKey: kp.publicKey,
    sign, blake3,
    createdAt: 1234567,
  });

  // 1) A genuine object verifies AND verifyObjectSubmission recomputes the same object_id.
  const ok = await verifyObjectSubmission(built.submission, { blake3, pqVerify });
  if (!ok.ok) fail('genuine object was REJECTED (verify returned ok:false)');
  if (ok.objectId !== built.objectId) {
    fail(`object_id mismatch — verify=${ok.objectId} built=${built.objectId}`);
  }

  // 2) A one-byte tamper of the payload must be rejected.
  const tPayload = { ...built.submission };
  const pb = Buffer.from(tPayload.payload_b64, 'base64'); pb[0] ^= 0xff;
  tPayload.payload_b64 = pb.toString('base64');
  if ((await verifyObjectSubmission(tPayload, { blake3, pqVerify })).ok) {
    fail('tampered PAYLOAD was ACCEPTED — verify is broken (forgery would pass)');
  }

  // 3) A one-byte tamper of the signature must be rejected.
  const tSig = { ...built.submission };
  const sb = Buffer.from(tSig.signature_b64, 'base64'); sb[0] ^= 0xff;
  tSig.signature_b64 = sb.toString('base64');
  if ((await verifyObjectSubmission(tSig, { blake3, pqVerify })).ok) {
    fail('tampered SIGNATURE was ACCEPTED — verify is broken');
  }

  console.log('object-verify-kat: PASS — genuine verifies (object_id matches); tampered payload + signature both rejected');
  process.exit(0);
} catch (e) {
  fail(e && e.stack ? e.stack : String(e));
}
