#!/usr/bin/env node
/**
 * pq-cross-fixtures.mjs: writes the WEBSITE'S half of the post-quantum cross-implementation
 * fixtures (2026-11-19, the post-quantum review's plan item 1b,
 * docs/reference/research/2026-11-16-pqc-review/).
 *
 *   scripts/tests/fixtures/pq-cross-v1.json      its `noble` half: an ML-KEM-768 encapsulation to
 *                                                the key in the `rust` half, a deterministic
 *                                                encapsulation, and ML-DSA-65 signatures (hedged,
 *                                                as the website signs, and deterministic)
 *   scripts/tests/fixtures/dm-envelope-v2.json   its `web` half: a complete sealed-sender v2 DM
 *                                                envelope sealed by web/chat/crypto.js pqBuildDmPuts
 *                                                to the recipient key in the `rust` half
 *
 * Each half is made by one implementation and checked by the other: the Rust tests
 * (crates/humanity-protocol/src/pq_cross_tests.rs, src/net/dm_pq_cross_tests.rs) check these
 * halves, and scripts/tests/pq-cross.test.js checks the Rust halves with the website's code.
 *
 * Order (the website seals to the Rust-derived keys, so the Rust halves come first):
 *   node scripts/lib-tests.js --crate humanity-protocol write_the_rust_half --ignored
 *   node scripts/lib-tests.js --crate humanity-engine write_the_rust_envelope --ignored
 *   node scripts/pq-cross-fixtures.mjs
 *
 * Regenerate only on purpose: after a library upgrade the OLD fixtures are what the new version
 * must still open and match. Nothing is installed or fetched: the vendored noble bundle and the
 * website's own scripts run under node (scripts/lib/chat-pq-page.js).
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

const require = createRequire(import.meta.url);
const { REPO, loadNoble, keysOf, chatPage } = require('./lib/chat-pq-page.js');

const PQ_FIXTURE = join(REPO, 'scripts', 'tests', 'fixtures', 'pq-cross-v1.json');
const DM_FIXTURE = join(REPO, 'scripts', 'tests', 'fixtures', 'dm-envelope-v2.json');
/** The fixed randomness of noble's deterministic encapsulation (the Rust half uses 0x42s). */
const NOBLE_KEM_M = new Uint8Array(32).fill(0x24);

const hex = (b) => Buffer.from(b).toString('hex');
const b64 = (b) => Buffer.from(b).toString('base64');
const unb64 = (s) => new Uint8Array(Buffer.from(s, 'base64'));
const enc = (s) => new TextEncoder().encode(s);

function fail(msg) {
  console.error(`pq-cross-fixtures: FAIL: ${msg}`);
  process.exit(1);
}

/** Keys sorted at every level, as the Rust writer's serde_json writes them, so the two writers agree. */
function sorted(v) {
  if (Array.isArray(v)) return v.map(sorted);
  if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sorted(v[k])]));
  return v;
}

function readFixture(path, half) {
  let fx;
  try { fx = JSON.parse(readFileSync(path, 'utf8')); } catch (e) { fail(`${path} does not read (${e.message}); write its Rust half first`); }
  if (!fx[half]) fail(`${path} has no "${half}" half; write it first (see this script's header)`);
  return fx;
}

const m = await loadNoble();

// ── pq-cross-v1.json, the noble half ──
const pq = readFixture(PQ_FIXTURE, 'rust');
{
  const rk = pq.rust.kem;
  const ek = unb64(rk.ek_b64);
  const mine = await keysOf(rk.recipient_master_byte);
  if (b64(mine.kyb.publicKey) !== rk.ek_b64) fail('noble derives another encapsulation key than the Rust half holds (the key-generation KAT, scripts/pq-kat.mjs, would fail too)');
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(ek); // randomized: pq.js pqDmSeal's own call
  const det = m.ml_kem768.encapsulate(ek, NOBLE_KEM_M);
  const signer = await chatPage(pq.rust.dsa.signer_master_byte);
  const message = pq.rust.dsa.message;
  const dil = (await keysOf(pq.rust.dsa.signer_master_byte)).dil;
  const hedged = await signer.fn('pqSignMessage')(signer.fn('myDilithiumSecret'), enc(message)); // the website's own signing
  if (!hedged) fail('the page\'s pqSignMessage made no signature');
  pq.noble = {
    written_by: 'scripts/pq-cross-fixtures.mjs (the vendored @noble/post-quantum 0.6.1 bundle; the hedged signature through web/chat/pq.js pqSignMessage)',
    kem: {
      encapsulated_to: 'rust.kem.ek_b64',
      ek_blake3: hex(m.blake3(ek, { dkLen: 32 })),
      ct_b64: b64(cipherText),
      ss_hex: hex(sharedSecret),
      deterministic: {
        m_hex: hex(NOBLE_KEM_M),
        ct_blake3: hex(m.blake3(det.cipherText, { dkLen: 32 })),
        ss_hex: hex(det.sharedSecret),
      },
    },
    dsa: {
      signer_master_byte: pq.rust.dsa.signer_master_byte,
      message,
      sig_hedged_b64: b64(hedged),
      sig_deterministic_b64: b64(m.ml_dsa65.sign(enc(message), dil.secretKey, { extraEntropy: false })),
    },
  };
  writeFileSync(PQ_FIXTURE, JSON.stringify(sorted(pq), null, 2) + '\n');
  console.log(`pq-cross-fixtures: wrote the noble half of ${PQ_FIXTURE}`);
}

// ── dm-envelope-v2.json, the web half ──
const dm = readFixture(DM_FIXTURE, 'rust');
{
  const r = dm.rust;
  const sender = await chatPage(r.sender_master_byte, { peerKyber: r.recipient_kyber_b64 });
  const recipientKey = (await keysOf(r.recipient_master_byte)).key;
  const built = await sender.fn('pqBuildDmPuts')(r.text, recipientKey, r.ts);
  if (!built || !built.recipientPut) fail('the page\'s pqBuildDmPuts built nothing');
  const innerBytes = Buffer.byteLength(JSON.stringify(built.inner), 'utf8');
  dm.web = {
    written_by: 'scripts/pq-cross-fixtures.mjs (web/chat/crypto.js pqBuildDmPuts over web/chat/pq.js pqDmSeal, the vendored noble bundle)',
    sender_master_byte: r.sender_master_byte,
    recipient_master_byte: r.recipient_master_byte,
    sealed_to: r.recipient_kyber_b64,
    ts: r.ts,
    text: r.text,
    envelope: built.recipientPut.content,
  };
  writeFileSync(DM_FIXTURE, JSON.stringify(sorted(dm), null, 2) + '\n');
  console.log(`pq-cross-fixtures: wrote the web half of ${DM_FIXTURE} (its sealed inner payload is ${innerBytes} bytes)`);
}
