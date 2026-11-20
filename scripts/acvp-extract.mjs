#!/usr/bin/env node
/**
 * acvp-extract.mjs: extracts NIST's official ML-KEM and ML-DSA test vectors for OUR parameter
 * sets only (ML-KEM-768, ML-DSA-65) into committed fixtures (2026-11-19, the post-quantum
 * review's plan item 1b, docs/reference/research/2026-11-16-pqc-review/).
 *
 * Input: a folder holding NIST ACVP-Server's `internalProjection.json` files (each in a folder
 * named for its vector set, as in the repository's gen-val/json-files/), a `COMMIT` file naming
 * the ACVP-Server commit they came from, and a `SHA256SUMS` file. The files are DATA: they are
 * parsed as JSON and filtered, nothing in them is run. Every file's SHA-256 must match SHA256SUMS
 * or nothing is written.
 *
 *   node scripts/acvp-extract.mjs <folder>
 *
 * Output: scripts/tests/fixtures/acvp/<parameter set>-<mode>.json, each recording its source
 * (repository, commit, path, the source file's SHA-256), the date and nature of the change
 * (only our parameter set's test groups kept, as NIST's notice asks of modified copies; the
 * notice itself is scripts/tests/fixtures/acvp/NOTICE.md), and any of our parameter set's groups
 * left out, each with the reason. Tests: crates/humanity-protocol/src/pq_acvp_tests.rs (RustCrypto
 * ml-kem and ml-dsa, the pinned versions) and scripts/tests/pq-acvp.test.js (the vendored noble
 * bundle). Re-run only to move to another ACVP-Server commit, on purpose.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const OUT = join(REPO, 'scripts', 'tests', 'fixtures', 'acvp');
const EXTRACTED_ON = '2026-11-19';

/** What we use: the parameter set each vector set is cut down to. */
const SETS = [
  { set: 'ML-KEM-keyGen-FIPS203', parameterSet: 'ML-KEM-768', out: 'ML-KEM-768-keyGen.json' },
  { set: 'ML-KEM-encapDecap-FIPS203', parameterSet: 'ML-KEM-768', out: 'ML-KEM-768-encapDecap.json' },
  { set: 'ML-DSA-keyGen-FIPS204', parameterSet: 'ML-DSA-65', out: 'ML-DSA-65-keyGen.json' },
  { set: 'ML-DSA-sigGen-FIPS204', parameterSet: 'ML-DSA-65', out: 'ML-DSA-65-sigGen.json' },
  { set: 'ML-DSA-sigVer-FIPS204', parameterSet: 'ML-DSA-65', out: 'ML-DSA-65-sigVer.json' },
];

/**
 * Groups of our parameter set that neither implementation can run, left out of the fixtures (and
 * listed in each fixture's `left_out` with this reason). Everything else of our parameter set is
 * kept, and the tests run every group kept.
 */
function leaveOut(group) {
  if (group.preHash === 'preHash') {
    return 'HashML-DSA (FIPS 204 section 5.4, pre-hash): RustCrypto ml-dsa 0.1.1 has no HashML-DSA interface, the vendored noble bundle exports none of the hash functions it needs, and HumanityOS signs only with pure ML-DSA';
  }
  return null;
}

function fail(msg) {
  console.error(`acvp-extract: FAIL: ${msg}`);
  process.exit(1);
}

const dir = process.argv[2];
if (!dir) fail('usage: node scripts/acvp-extract.mjs <folder with COMMIT, SHA256SUMS and <set>/internalProjection.json>');
const commit = readFileSync(join(dir, 'COMMIT'), 'utf8').trim();
if (!/^[0-9a-f]{40}$/.test(commit)) fail(`COMMIT is not a full commit hash: ${commit}`);
const sums = new Map(
  readFileSync(join(dir, 'SHA256SUMS'), 'utf8').split(/\r?\n/).filter(Boolean).map((line) => {
    const m = line.match(/^([0-9a-f]{64}) [ *](.+)$/);
    if (!m) fail(`a SHA256SUMS line does not read: ${line}`);
    return [m[2].replace(/\\/g, '/'), m[1]];
  }),
);

const written = [];
const outputs = [];
for (const { set, parameterSet, out } of SETS) {
  const rel = `${set}/internalProjection.json`;
  const bytes = readFileSync(join(dir, set, 'internalProjection.json'));
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  if (sums.get(rel) !== sha256) fail(`${rel}: SHA-256 ${sha256} is not the one SHA256SUMS lists (${sums.get(rel) || 'none'})`);
  const src = JSON.parse(bytes.toString('utf8'));
  const ours = src.testGroups.filter((g) => g.parameterSet === parameterSet);
  if (!ours.length) fail(`${rel} has no ${parameterSet} test group`);
  const kept = ours.filter((g) => !leaveOut(g));
  const left_out = ours.filter((g) => leaveOut(g)).map((g) => ({
    tgId: g.tgId,
    ...Object.fromEntries(Object.entries(g).filter(([k, v]) => k !== 'tests' && k !== 'tgId' && typeof v !== 'object')),
    tests: g.tests.length,
    reason: leaveOut(g),
  }));
  const fixture = {
    source: {
      repository: 'https://github.com/usnistgov/ACVP-Server',
      commit,
      path: `gen-val/json-files/${rel}`,
      sha256,
      notice: 'NIST-developed; the licence notice is scripts/tests/fixtures/acvp/NOTICE.md. Acknowledgement: the National Institute of Standards and Technology is the source of these test vectors.',
    },
    changed: `${EXTRACTED_ON}, by scripts/acvp-extract.mjs: only the ${parameterSet} test groups kept (HumanityOS uses no other parameter set)${left_out.length ? ', less the groups listed in left_out' : ''}; every kept group and test is unchanged.`,
    parameterSet,
    vsId: src.vsId,
    algorithm: src.algorithm,
    mode: src.mode,
    revision: src.revision,
    isSample: src.isSample,
    left_out,
    testGroups: kept,
  };
  outputs.push([out, fixture]);
  written.push(`${out}: ${kept.length} groups, ${kept.reduce((n, g) => n + g.tests.length, 0)} tests${left_out.length ? `, left out ${left_out.map((g) => `tg ${g.tgId}`).join(', ')}` : ''}`);
}

mkdirSync(OUT, { recursive: true });
for (const [out, fixture] of outputs) writeFileSync(join(OUT, out), JSON.stringify(fixture, null, 1) + '\n');
console.log(`acvp-extract: ACVP-Server ${commit}`);
for (const w of written) console.log(`  ${w}`);
