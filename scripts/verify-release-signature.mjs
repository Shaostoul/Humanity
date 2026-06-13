#!/usr/bin/env node
/**
 * verify-release-signature.mjs — INDEPENDENT verification of a signed release.
 *
 * Confirms a published release-manifest.json + .sig.json validates against the
 * committed operator public keys, using DIFFERENT crypto implementations than
 * the Rust signer (noble ml-dsa for Dilithium3, Node's built-in crypto for
 * Ed25519) — a true cross-check, not the same code agreeing with itself. Also
 * confirms a downloaded artifact's SHA-256 matches the signed manifest.
 *
 * Usage: node scripts/verify-release-signature.mjs <dir>
 *   where <dir> holds release-manifest.json, release-manifest.json.sig.json,
 *   and (optionally) the platform binaries to hash-check.
 */
import { readFileSync, existsSync } from 'node:fs';
import { createHash, createPublicKey, verify as nodeVerify } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const dir = process.argv[2];
if (!dir) { console.error('usage: verify-release-signature.mjs <dir>'); process.exit(2); }

const pubs = JSON.parse(readFileSync(join(REPO, 'data', 'release', 'signing_pubkeys.json'), 'utf8'));
const manifestBytes = readFileSync(join(dir, 'release-manifest.json'));
const sig = JSON.parse(readFileSync(join(dir, 'release-manifest.json.sig.json'), 'utf8'));

const hex = (s) => Uint8Array.from(Buffer.from(s, 'hex'));
const b64 = (s) => Uint8Array.from(Buffer.from(s, 'base64'));
let ok = true;
const pass = (m) => console.log(`  PASS  ${m}`);
const fail = (m) => { console.error(`  FAIL  ${m}`); ok = false; };

if (sig.alg !== 'hybrid-ed25519-mldsa65-v1') fail(`unexpected alg: ${sig.alg}`);
else pass(`alg = ${sig.alg}`);

// ── Ed25519 (Node built-in). Wrap the raw 32-byte key in DER SPKI. ──
try {
  const raw = hex(pubs.ed25519);
  if (raw.length !== 32) throw new Error(`ed25519 pubkey ${raw.length} bytes`);
  const der = Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), Buffer.from(raw)]);
  const key = createPublicKey({ key: der, format: 'der', type: 'spki' });
  const sigBytes = b64(sig.ed25519);
  if (sigBytes.length !== 64) throw new Error(`ed25519 sig ${sigBytes.length} bytes`);
  if (nodeVerify(null, manifestBytes, key, sigBytes)) pass('Ed25519 signature valid (Node crypto)');
  else fail('Ed25519 signature INVALID');
} catch (e) { fail(`Ed25519: ${e.message}`); }

// ── Dilithium3 (noble ml-dsa, independent of the RustCrypto signer). ──
try {
  const m = await import(pathToFileURL(join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
  if (!m.ml_dsa65) throw new Error('noble bundle missing ml_dsa65');
  const pk = hex(pubs.dilithium);
  if (pk.length !== 1952) throw new Error(`dilithium pubkey ${pk.length} bytes`);
  const sigBytes = b64(sig.dilithium);
  if (sigBytes.length !== 3309) throw new Error(`dilithium sig ${sigBytes.length} bytes`);
  // noble 0.6.x: verify(sig, msg, publicKey)
  if (m.ml_dsa65.verify(sigBytes, new Uint8Array(manifestBytes), pk)) pass('Dilithium3 signature valid (noble ml-dsa)');
  else fail('Dilithium3 signature INVALID');
} catch (e) { fail(`Dilithium3: ${e.message}`); }

// ── Artifact hash check: every binary present in <dir> must match the manifest. ──
const manifest = JSON.parse(manifestBytes.toString('utf8'));
console.log(`  manifest version: ${manifest.version}, ${manifest.artifacts.length} artifact(s)`);
let checked = 0;
for (const a of manifest.artifacts) {
  const p = join(dir, a.name);
  if (!existsSync(p)) { console.log(`  (skip hash: ${a.name} not downloaded)`); continue; }
  const got = createHash('sha256').update(readFileSync(p)).digest('hex');
  if (got.toLowerCase() === a.sha256.toLowerCase()) { pass(`artifact hash matches: ${a.name}`); checked++; }
  else fail(`artifact hash MISMATCH: ${a.name}\n    manifest ${a.sha256}\n    actual   ${got}`);
}
if (checked === 0) console.log('  (no artifacts downloaded to hash-check — signature proof above stands)');

console.log(ok ? '\nRESULT: VERIFIED ✓  (both signatures valid via independent implementations)'
               : '\nRESULT: FAILED ✗');
process.exit(ok ? 0 : 1);
