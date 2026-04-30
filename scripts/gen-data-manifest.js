#!/usr/bin/env node
/**
 * gen-data-manifest.js
 *
 * Walks `data/` (and optionally `assets/icons/`, `assets/shaders/`) and writes
 * `data-manifest-<version>.json` listing every file with:
 *   - relative path
 *   - byte size
 *   - SHA-256 hash (hex)
 *   - per-file URL on the Forgejo source mirror
 *
 * The manifest enables file-level delta sync: a client can compare local file
 * hashes against the manifest and fetch only the files that changed. Used by
 * the layered/modular update system (Step 4.5 of distribution-sovereignty plan).
 *
 * Usage:
 *   node scripts/gen-data-manifest.js <version-tag> [--root=<path>] [--out=<path>]
 *     version-tag: e.g. "v0.130.0" — used to construct per-file Forgejo URLs
 *     --root: source root to scan (default: this script's parent directory)
 *     --out:  output path (default: <root>/data-manifest-<version>.json)
 *
 * Hash function: SHA-256 (universally supported, fast enough for ~108 files).
 * BLAKE3 is the project's canonical hash but requires a non-stdlib dep; SHA-256
 * via node:crypto is built-in and adequate for integrity-check purposes.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const FORGEJO_USER = 'shaostoul';
const FORGEJO_REPO = 'humanity';
const FORGEJO_BASE = `https://git.united-humanity.us/${FORGEJO_USER}/${FORGEJO_REPO}/raw/tag`;

const SOURCE_DIRS = [
  'data',
  'assets/icons',
  'assets/shaders',
];

const args = process.argv.slice(2);
const version = args.find(a => !a.startsWith('--'));
if (!version) {
  console.error('usage: gen-data-manifest.js <version-tag> [--root=<path>] [--out=<path>]');
  process.exit(1);
}
const rootArg = args.find(a => a.startsWith('--root='));
const outArg = args.find(a => a.startsWith('--out='));
const ROOT = rootArg ? path.resolve(rootArg.slice('--root='.length)) : path.resolve(__dirname, '..');
const outputPath = outArg ? path.resolve(outArg.slice('--out='.length)) : path.join(ROOT, `data-manifest-${version}.json`);

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.isFile()) out.push(full);
  }
  return out;
}

function hashFile(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

const allFiles = [];
for (const src of SOURCE_DIRS) {
  walk(path.join(ROOT, src), allFiles);
}

const entries = allFiles.map(full => {
  const rel = path.relative(ROOT, full).replace(/\\/g, '/');
  return {
    path: rel,
    size: fs.statSync(full).size,
    sha256: hashFile(full),
    url: `${FORGEJO_BASE}/${version}/${rel}`,
  };
}).sort((a, b) => a.path.localeCompare(b.path));

const totalBytes = entries.reduce((s, e) => s + e.size, 0);

const manifest = {
  _comment:
    'Per-file integrity manifest for HumanityOS data + assets layer. Each file ' +
    'is content-addressable via its sha256 hash and fetchable via the Forgejo ' +
    'source mirror. Clients comparing local hashes to this manifest can sync ' +
    'only the files that changed since their last update.',
  version,
  base_url: FORGEJO_BASE,
  source_dirs: SOURCE_DIRS,
  generated_at: new Date().toISOString(),
  file_count: entries.length,
  total_bytes: totalBytes,
  files: entries,
};

fs.writeFileSync(outputPath, JSON.stringify(manifest, null, 2) + '\n', 'utf8');

const mb = (totalBytes / (1024 * 1024)).toFixed(2);
console.log(`Wrote ${outputPath} — ${entries.length} files, ${mb} MB`);
