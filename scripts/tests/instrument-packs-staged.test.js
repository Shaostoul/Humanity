'use strict';
/**
 * THE CATALOG MATCHES THE PACK FILES (2026-11-13, the full packs): every pack data/music/packs.ron
 * lists has a file of exactly its size and SHA-256 in the folder the packs were built into, so what
 * is uploaded as a release asset is what Settings will check a download against. A pack whose bytes
 * or checksum differ would be refused on every download (src/instrument_packs/install.rs).
 *
 * The folder: HUMANITY_INSTRUMENT_PACKS_OUT (the orchestrator's staging folder, where `just
 * instrument-packs --packs-out` wrote them), else E:\HumanityOS\instrument-packs, the pipeline's own
 * default. When neither exists (CI, any other machine) there is nothing to compare and the test says
 * so and passes; named explicitly, a missing folder or file is a failure.
 *
 * Also: the pack files' names are the catalog's, version included, and none is over GitHub's 2 GiB
 * limit for one release asset.
 *
 * RED first (2026-11-13): run with HUMANITY_INSTRUMENT_PACKS_OUT set to the empty staging folder
 * before the full packs were built (the catalog then named the first keys pack): "humanity-instruments-keys-v1.tar
 * is in the staging folder".
 */
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.resolve(__dirname, '..', '..');
const GITHUB_ASSET_LIMIT = 2 * 1024 * 1024 * 1024;

/** Each pack's file, size and checksum, read from the catalog's text. */
function catalogPacks(text) {
  const out = [];
  const re = /id: "([a-z0-9_]+)",\s*version: (\d+),[\s\S]*?file: "([^"]+)",\s*bytes: (\d+),\s*sha256: "([0-9a-f]*)"/g;
  for (const m of text.matchAll(re)) out.push({ id: m[1], version: Number(m[2]), file: m[3], bytes: Number(m[4]), sha256: m[5] });
  return out;
}

function sha256File(p) {
  const h = crypto.createHash('sha256');
  const fd = fs.openSync(p, 'r');
  const buf = Buffer.alloc(8 * 1024 * 1024);
  try {
    let n;
    while ((n = fs.readSync(fd, buf, 0, buf.length, null)) > 0) h.update(buf.subarray(0, n));
  } finally {
    fs.closeSync(fd);
  }
  return h.digest('hex');
}

test('every pack in packs.ron is a staged file of its size and checksum', (t) => {
  const named = process.env.HUMANITY_INSTRUMENT_PACKS_OUT;
  const dir = named;
  const packs = catalogPacks(fs.readFileSync(path.join(ROOT, 'data', 'music', 'packs.ron'), 'utf8'));
  assert.ok(packs.length > 0, "the catalog lists its packs");
  for (const p of packs) {
    assert.strictEqual(p.file, `humanity-instruments-${p.id}-v${p.version}.tar`, `${p.id}: the file name carries its version`);
    assert.ok(p.bytes > 0 && p.bytes < GITHUB_ASSET_LIMIT, `${p.id}: ${p.bytes} bytes fits one release asset`);
  }
  if (!named) {
    t.skip('no staging folder named; set HUMANITY_INSTRUMENT_PACKS_OUT to check one');
    return;
  }
  for (const p of packs) {
    const f = path.join(dir, p.file);
    assert.ok(fs.existsSync(f), `${p.file} is in the staging folder ${dir}`);
    assert.strictEqual(fs.statSync(f).size, p.bytes, `${p.file}: its size is the catalog's`);
    assert.strictEqual(sha256File(f), p.sha256, `${p.file}: its SHA-256 is the catalog's`);
  }
});

module.exports = { catalogPacks };
