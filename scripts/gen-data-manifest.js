#!/usr/bin/env node
/**
 * gen-data-manifest.js
 *
 * Writes `data-manifest-<version>.json` listing every file the release's data archive
 * (HumanityOS-data-<tag>.tar.gz) carries, each with:
 *   - relative path
 *   - byte size
 *   - SHA-256 hash (hex)
 *   - per-file URL on the Forgejo source mirror
 *
 * THE LIST IS THE ARCHIVE'S (2026-11-10): it comes from scripts/lib/release-files.js, the one list
 * the workflow's bundle copy and its data archive are made from, so the manifest never names a file
 * the release does not have and never misses one it does. Until then this walked only data/,
 * assets/icons/ and assets/shaders/: it named none of the models, textures and sounds the archive
 * carries, and named the audio placeholders the archive leaves out. scripts/tests/release-files.test.js
 * checks the two lists are the same (`--paths-only`).
 *
 * The manifest enables file-level delta sync: a client can compare local file
 * hashes against the manifest and fetch only the files that changed. Used by
 * the layered/modular update system (Step 4.5 of distribution-sovereignty plan;
 * docs/admin/torrent-infrastructure.md: the in-app updater does not read it yet).
 *
 * Usage:
 *   node scripts/gen-data-manifest.js <version-tag> [--root=<path>] [--out=<path>]
 *     version-tag: e.g. "v0.130.0" — used to construct per-file Forgejo URLs
 *     --root: source root to scan (default: this script's parent directory)
 *     --out:  output path (default: <root>/data-manifest-<version>.json)
 *   node scripts/gen-data-manifest.js --paths-only [--root=<path>]
 *     print the paths the manifest would list, one a line, without hashing (the test's view)
 *
 * Hash function: SHA-256 (universally supported, fast enough for the archive's files).
 * BLAKE3 is the project's canonical hash but requires a non-stdlib dep; SHA-256
 * via node:crypto is built-in and adequate for integrity-check purposes.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const release = require('./lib/release-files');

const FORGEJO_USER = 'shaostoul';
const FORGEJO_REPO = 'humanity';
const FORGEJO_BASE = `https://git.united-humanity.us/${FORGEJO_USER}/${FORGEJO_REPO}/raw/tag`;

const args = process.argv.slice(2);
const rootArg = args.find(a => a.startsWith('--root='));
const ROOT = rootArg ? path.resolve(rootArg.slice('--root='.length)) : path.resolve(__dirname, '..');

if (args.includes('--paths-only')) {
  process.stdout.write(release.releaseFiles(ROOT).join('\n') + '\n');
  process.exit(0);
}

const version = args.find(a => !a.startsWith('--'));
if (!version) {
  console.error('usage: gen-data-manifest.js <version-tag> [--root=<path>] [--out=<path>] | --paths-only [--root=<path>]');
  process.exit(1);
}
const outArg = args.find(a => a.startsWith('--out='));
const outputPath = outArg ? path.resolve(outArg.slice('--out='.length)) : path.join(ROOT, `data-manifest-${version}.json`);

function hashFile(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

const entries = release.releaseFiles(ROOT).map(rel => {
  const full = path.join(ROOT, rel);
  return {
    path: rel,
    size: fs.statSync(full).size,
    sha256: hashFile(full),
    url: `${FORGEJO_BASE}/${version}/${rel}`,
  };
});

const totalBytes = entries.reduce((s, e) => s + e.size, 0);

const manifest = {
  _comment:
    'Per-file integrity manifest for the HumanityOS data archive (HumanityOS-data-<tag>.tar.gz): ' +
    'exactly the files it carries. Each file is content-addressable via its sha256 hash and ' +
    'fetchable via the Forgejo source mirror. Clients comparing local hashes to this manifest can ' +
    'sync only the files that changed since their last update.',
  version,
  base_url: FORGEJO_BASE,
  source_dirs: release.ROOTS,
  source_files: release.FILES,
  excluded: release.EXCLUDED.map(r => `${r.what}: ${r.why}`),
  generated_at: new Date().toISOString(),
  file_count: entries.length,
  total_bytes: totalBytes,
  files: entries,
};

fs.writeFileSync(outputPath, JSON.stringify(manifest, null, 2) + '\n', 'utf8');

const mb = (totalBytes / (1024 * 1024)).toFixed(2);
console.log(`Wrote ${outputPath} — ${entries.length} files, ${mb} MB`);
