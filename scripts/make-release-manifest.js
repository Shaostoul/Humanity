#!/usr/bin/env node
// Build a release-manifest.json for the signed-update scheme.
//
// Usage:  node scripts/make-release-manifest.js <version> <dir>
//
// Reads the RAW per-platform binaries the in-app updater downloads (NOT the
// .tar.gz bundles) from <dir>, computes each one's SHA-256 + size, and writes
// release-manifest.json in the repo root. The operator then signs that file
// with `HumanityOS --sign-release release-manifest.json` (hybrid Ed25519 +
// Dilithium3) and uploads both to the GitHub release. See docs/release-signing.md.
//
// The manifest covers exactly the asset NAMES the updater's find_platform_asset
// looks for, so the hash check in src/release_update.rs lines up.

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const [, , versionArg, dir] = process.argv;
if (!versionArg || !dir) {
  console.error('usage: node scripts/make-release-manifest.js <version> <dir>');
  process.exit(2);
}
const version = versionArg.replace(/^v/, '');

// The raw platform binaries (must match build-desktop.yml asset_name values +
// the updater's find_platform_asset patterns).
const WANTED = [
  'HumanityOS-windows-x64.exe',
  'HumanityOS-linux-x64',
  'HumanityOS-macos-arm64',
  'HumanityOS-macos-x64',
];

const artifacts = [];
for (const name of WANTED) {
  const p = path.join(dir, name);
  if (!fs.existsSync(p)) {
    console.warn(`  skip (missing): ${name}`);
    continue;
  }
  const bytes = fs.readFileSync(p);
  const sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
  artifacts.push({ name, sha256, size: bytes.length });
  console.log(`  ${name}  sha256=${sha256}  ${bytes.length}B`);
}

if (artifacts.length === 0) {
  console.error(`No platform binaries found in ${dir}. Did the release finish building?`);
  process.exit(1);
}

const manifest = { version, artifacts };
fs.writeFileSync('release-manifest.json', JSON.stringify(manifest, null, 2) + '\n');
console.log(`Wrote release-manifest.json — version ${version}, ${artifacts.length} artifact(s).`);
console.log('Next: HumanityOS --sign-release release-manifest.json  (sets HUMANITY_SIGNING_PASSPHRASE)');
