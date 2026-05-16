#!/usr/bin/env node
/**
 * Reproducible build of the vendored post-quantum bundle.
 *
 * HumanityOS makes Dilithium3 the PRIMARY chat identity (PQ migration,
 * v0.251+). A primary-identity dependency must NOT be fetched from a
 * third-party CDN at runtime (supply-chain + offline + CSP). So we
 * vendor a single self-contained ESM module, same-origin, served under
 * CSP `script-src 'self'`.
 *
 * This script pins exact versions, bundles with esbuild (no minify so
 * the output is auditable / diffable), asserts the cross-language
 * known-answer test (the SAME constants as
 * `src/relay/core/pq_crypto.rs::dilithium_cross_language_kat`), then
 * writes `web/shared/vendor/noble-pq.bundle.js` with a provenance
 * header. If noble or RustCrypto ever change their ML-DSA output, the
 * KAT fails here AND in `cargo test` — neither can drift silently.
 *
 * Run: `node scripts/build-noble-bundle.mjs`  (or `just pq-vendor`)
 * Requires network for the npm install of the pinned versions.
 */
import { execSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const PQ_VER = '0.6.1';      // @noble/post-quantum
const HASHES_VER = '2.2.0';  // @noble/hashes
const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const OUT = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

// Canonical known-answer (master = 32 bytes of 0x07). Identical to the
// Rust KAT — DO NOT edit one without the other.
const KAT_MASTER = new Uint8Array(32).fill(7);
const KAT_DIL_SEED = 'f0dfc6e8cc3eebd2e0f0265d2aae0f339090f2d4f92726884e385a48e81e0cc4';
const KAT_PK_BLAKE3 = '3f4ff5c7e6505ca7b0dd6cb32c53839f8cff19772e291d4f18b082d1f7dc0126';

const work = mkdtempSync(join(tmpdir(), 'noblevendor-'));
try {
  console.log(`[pq-vendor] workspace ${work}`);
  writeFileSync(join(work, 'package.json'), JSON.stringify({ private: true, type: 'module' }));
  execSync(
    `npm i --silent --no-audit --no-fund @noble/post-quantum@${PQ_VER} @noble/hashes@${HASHES_VER}`,
    { cwd: work, stdio: 'inherit' }
  );
  writeFileSync(
    join(work, 'entry.js'),
    "export { ml_dsa65 } from '@noble/post-quantum/ml-dsa.js';\n" +
    "export { ml_kem768 } from '@noble/post-quantum/ml-kem.js';\n" +
    "export { blake3 } from '@noble/hashes/blake3.js';\n"
  );
  execSync(
    `npx --yes esbuild entry.js --bundle --format=esm --platform=browser ` +
    `--target=es2022 --legal-comments=none --outfile=bundle.js`,
    { cwd: work, stdio: 'inherit' }
  );

  let code = readFileSync(join(work, 'bundle.js'), 'utf8');
  // Must be fully self-contained (no bare external imports).
  if (/\bfrom\s*["'][^.\/]/.test(code) || /\brequire\(/.test(code)) {
    throw new Error('bundle is not self-contained — external import/require found');
  }

  // Verify the KAT against the freshly built bundle.
  writeFileSync(join(work, 'b.mjs'), code);
  const m = await import('file://' + join(work, 'b.mjs'));
  const hx = (b) => [...b].map((x) => x.toString(16).padStart(2, '0')).join('');
  const ctx = new TextEncoder().encode('hum/dilithium3/v1');
  const seed = m.blake3.create({ context: ctx, dkLen: 32 }).update(KAT_MASTER).digest();
  if (hx(seed) !== KAT_DIL_SEED) {
    throw new Error(`KAT FAIL: dil_seed ${hx(seed)} != ${KAT_DIL_SEED}`);
  }
  const pk = m.ml_dsa65.keygen(seed).publicKey;
  const pkB3 = hx(m.blake3(pk, { dkLen: 32 }));
  if (pkB3 !== KAT_PK_BLAKE3) {
    throw new Error(`KAT FAIL: pk_blake3 ${pkB3} != ${KAT_PK_BLAKE3}`);
  }
  console.log('[pq-vendor] KAT OK — noble matches RustCrypto ml-dsa byte-for-byte');

  const sha = createHash('sha256').update(code).digest('hex');
  const header =
    `/* HumanityOS vendored post-quantum bundle — DO NOT EDIT BY HAND.\n` +
    ` * Regenerate: node scripts/build-noble-bundle.mjs\n` +
    ` * Source: @noble/post-quantum@${PQ_VER} + @noble/hashes@${HASHES_VER}\n` +
    ` * Exports: ml_dsa65 (Dilithium3/ML-DSA-65), ml_kem768 (Kyber768),\n` +
    ` *          blake3. Self-contained ESM, served same-origin so a\n` +
    ` *          primary-identity dependency never relies on a CDN.\n` +
    ` * KAT: BLAKE3-derive(hum/dilithium3/v1, 32x0x07) ->\n` +
    ` *      ${KAT_DIL_SEED}\n` +
    ` *      ml_dsa65.keygen(seed).publicKey blake3 = ${KAT_PK_BLAKE3}\n` +
    ` *      (matches src/relay/core/pq_crypto.rs::dilithium_cross_language_kat)\n` +
    ` * sha256(body) = ${sha}\n` +
    ` */\n`;
  writeFileSync(OUT, header + code);
  console.log(`[pq-vendor] wrote ${OUT}`);
  console.log(`[pq-vendor] sha256(body) = ${sha}`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
