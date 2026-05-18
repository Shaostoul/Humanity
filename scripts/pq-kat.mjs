#!/usr/bin/env node
/**
 * pq-kat.mjs — cross-language post-quantum known-answer test.
 *
 * Asserts that the VENDORED bundle the chat client actually ships
 * (web/shared/vendor/noble-pq.bundle.js) reproduces the exact same
 * Dilithium3 derivation as the Rust relay
 * (src/relay/core/pq_crypto.rs::dilithium_cross_language_kat). If this
 * fails, a chat-derived PQ identity would be unverifiable by the relay
 * — DO NOT SHIP. Run: `node scripts/pq-kat.mjs` (or `just pq-kat`).
 *
 * The constants below are duplicated in the Rust test on purpose:
 * editing the derivation must break BOTH or neither.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const BUNDLE = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

const KAT_MASTER = new Uint8Array(32).fill(7);
const KAT_DIL_SEED =
  'f0dfc6e8cc3eebd2e0f0265d2aae0f339090f2d4f92726884e385a48e81e0cc4';
const KAT_PK_BLAKE3 =
  '3f4ff5c7e6505ca7b0dd6cb32c53839f8cff19772e291d4f18b082d1f7dc0126';
// Kyber768 (ML-KEM-768) DM-key derivation — same constants as
// pq_crypto.rs::kyber_cross_language_kat. Frozen 2026-05-18.
const KAT_KYBER_SEED =
  '817975ca77f0b8a878088723602d68e0b2ff863ab0071c0b4c091d9fa114c639117a1f6ced5be40be2fdc1c3781fbdaf84c83d9d25153703620a6a5c1498eb2b';
const KAT_KYBER_PK_BLAKE3 =
  'e5325adfbe9bbcedda20dbb333b9b94524ca853d4c641f03a199a96568c92664';

const hx = (b) => [...b].map((x) => x.toString(16).padStart(2, '0')).join('');

function fail(msg) {
  console.error(`pq-kat: FAIL — ${msg}`);
  process.exit(1);
}

try {
  // Confirm the vendored file is present + carries its provenance header.
  const head = readFileSync(BUNDLE, 'utf8').slice(0, 80);
  if (!head.includes('HumanityOS vendored post-quantum bundle')) {
    fail('vendored bundle missing or lacks provenance header — run `just pq-vendor`');
  }
  const m = await import(pathToFileURL(BUNDLE).href);
  if (!m.ml_dsa65 || !m.blake3) fail('bundle missing ml_dsa65 / blake3 exports');
  if (!m.ml_kem768) fail('bundle missing ml_kem768 export — run `just pq-vendor`');

  const ctx = new TextEncoder().encode('hum/dilithium3/v1');
  const seed = m.blake3.create({ context: ctx, dkLen: 32 }).update(KAT_MASTER).digest();
  if (hx(seed) !== KAT_DIL_SEED) {
    fail(`dil_seed mismatch\n  got      ${hx(seed)}\n  expected ${KAT_DIL_SEED}`);
  }
  const pk = m.ml_dsa65.keygen(seed).publicKey;
  if (pk.length !== 1952) fail(`pk length ${pk.length} != 1952`);
  const pkB3 = hx(m.blake3(pk, { dkLen: 32 }));
  if (pkB3 !== KAT_PK_BLAKE3) {
    fail(`pk blake3 mismatch — noble vs RustCrypto ML-DSA drift\n  got      ${pkB3}\n  expected ${KAT_PK_BLAKE3}`);
  }

  // Round-trip a signature so the signing path (Increment 2+) is
  // covered too. noble 0.6.x API: sign(msg, secretKey),
  // verify(sig, msg, publicKey).
  const msg = new TextEncoder().encode('humanity pq kat');
  const kp = m.ml_dsa65.keygen(seed);
  const sig = m.ml_dsa65.sign(msg, kp.secretKey);
  if (sig.length !== 3309) fail(`signature length ${sig.length} != 3309`);
  if (!m.ml_dsa65.verify(sig, msg, kp.publicKey)) fail('sign/verify round-trip failed');

  // ── Kyber768 (DM key) cross-language KAT ──
  // The recipient's DM keypair is derived from the SAME seed on web
  // and native. If noble ml_kem768 keygen and RustCrypto ml-kem
  // diverge from the same 64-byte seed, every cross-client DM silently
  // fails — exactly the bug the full-PQ cutover kills. This gate is
  // why we can wipe + cut over safely.
  const kctx = new TextEncoder().encode('hum/kyber768/v1');
  const kseed = m.blake3.create({ context: kctx, dkLen: 64 }).update(KAT_MASTER).digest();
  if (hx(kseed) !== KAT_KYBER_SEED) {
    fail(`kyber_seed mismatch\n  got      ${hx(kseed)}\n  expected ${KAT_KYBER_SEED}`);
  }
  const kpk = m.ml_kem768.keygen(kseed).publicKey;
  if (kpk.length !== 1184) fail(`kyber pk length ${kpk.length} != 1184`);
  const kpkB3 = hx(m.blake3(kpk, { dkLen: 32 }));
  if (kpkB3 !== KAT_KYBER_PK_BLAKE3) {
    fail(`kyber pk blake3 mismatch — noble vs RustCrypto ML-KEM drift\n  got      ${kpkB3}\n  expected ${KAT_KYBER_PK_BLAKE3}`);
  }
  // Encapsulate/decapsulate self-roundtrip (noble API order check —
  // we've been bitten by noble arg order before).
  const kkp = m.ml_kem768.keygen(kseed);
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(kkp.publicKey);
  const ss2 = m.ml_kem768.decapsulate(cipherText, kkp.secretKey);
  if (hx(sharedSecret) !== hx(ss2)) fail('ml_kem768 encapsulate/decapsulate shared-secret mismatch');

  console.log('pq-kat: PASS — vendored noble matches the Rust relay byte-for-byte (Dilithium3 + Kyber768)');
  process.exit(0);
} catch (e) {
  fail(e && e.message ? e.message : String(e));
}
