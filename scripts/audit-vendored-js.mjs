#!/usr/bin/env node
/**
 * audit-vendored-js.mjs: `npm audit` over the exact versions the website's vendored post-quantum
 * bundle was built from (2026-11-19, the post-quantum review's plan item 1b,
 * docs/reference/research/2026-11-16-pqc-review/; run by .github/workflows/advisories.yml).
 *
 * WHY: web/ has no package.json: every library the website ships is vendored as one file. The one
 * that carries the identity, web/shared/vendor/noble-pq.bundle.js (ML-DSA-65, ML-KEM-768, BLAKE3),
 * is built by scripts/build-noble-bundle.mjs from the versions pinned there (PQ_VER, HASHES_VER),
 * and the bundle's own header names them. With no lockfile, `npm audit` has nothing to read, so this
 * script writes one: a throwaway package depending on exactly those versions, resolved with
 * `npm install --package-lock-only --ignore-scripts` (package metadata only: no package code is
 * downloaded or run), then audited. It fails, as `npm audit` does, on any known vulnerability.
 *
 * LIMIT: the two pinned packages are exact; anything they depend on is resolved today, which may be
 * newer than what the bundle holds (the bundle has no lockfile to say). The bundle exports only
 * ml_dsa65, ml_kem768 and blake3, which come from the two pinned packages.
 *
 * Run: node scripts/audit-vendored-js.mjs          (exit code is npm audit's)
 *      node scripts/audit-vendored-js.mjs --pins   (prints the pins and exits; no network)
 */
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const BUILDER = join(REPO, 'scripts', 'build-noble-bundle.mjs');
const BUNDLE = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

/**
 * The versions the vendored bundle was built from: the builder's pins, which must be the ones the
 * bundle's header names (a bundle rebuilt from other versions, or a pin changed without rebuilding,
 * is refused, so what is audited is what ships). Returns {"@noble/post-quantum": "x", ...}.
 */
export function vendoredPins(builderText = readFileSync(BUILDER, 'utf8'), bundleText = readFileSync(BUNDLE, 'utf8')) {
  const pin = (name) => {
    const m = builderText.match(new RegExp(`const ${name}\\s*=\\s*'([0-9]+\\.[0-9]+\\.[0-9]+)'`));
    if (!m) throw new Error(`scripts/build-noble-bundle.mjs has no exact ${name} pin`);
    return m[1];
  };
  const pins = { '@noble/post-quantum': pin('PQ_VER'), '@noble/hashes': pin('HASHES_VER') };
  const source = bundleText.slice(0, 600).match(/Source: ([^\r\n]+)/);
  const named = source ? source[1].trim() : '';
  const expected = `@noble/post-quantum@${pins['@noble/post-quantum']} + @noble/hashes@${pins['@noble/hashes']}`;
  if (named !== expected) {
    throw new Error(`the vendored bundle's header names "${named}", but scripts/build-noble-bundle.mjs pins "${expected}": rebuild the bundle (just pq-vendor) or fix the pin`);
  }
  return pins;
}

function main() {
  const pins = vendoredPins();
  if (process.argv.includes('--pins')) {
    console.log(JSON.stringify(pins));
    return 0;
  }
  const work = mkdtempSync(join(tmpdir(), 'vendored-audit-'));
  try {
    writeFileSync(join(work, 'package.json'), JSON.stringify({ name: 'vendored-audit', private: true, dependencies: pins }, null, 2));
    const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
    const opts = { cwd: work, stdio: 'inherit', shell: process.platform === 'win32' };
    execFileSync(npm, ['install', '--package-lock-only', '--ignore-scripts', '--no-audit', '--no-fund'], opts);
    console.log(`audit-vendored-js: auditing ${Object.entries(pins).map(([n, v]) => `${n}@${v}`).join(', ')} (the vendored post-quantum bundle)`);
    try {
      execFileSync(npm, ['audit', '--audit-level=low'], opts);
    } catch (e) {
      console.error('audit-vendored-js: FAIL: a known advisory names a version the vendored bundle was built from');
      return typeof e.status === 'number' && e.status !== 0 ? e.status : 1;
    }
    return 0;
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(fileURLToPath(import.meta.url)) === resolve(process.argv[1])) process.exit(main());
