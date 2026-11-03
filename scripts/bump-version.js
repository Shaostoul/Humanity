#!/usr/bin/env node
// bump-version.js — Bumps version strings across all locations.
// Usage: node scripts/bump-version.js [patch|minor|major]

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const kind = (process.argv[2] || 'patch').toLowerCase();

if (!['patch', 'minor', 'major'].includes(kind)) {
  console.error(`Invalid bump type: "${kind}". Use patch, minor, or major.`);
  process.exit(1);
}

// 1. Read current version from Cargo.toml (root)
const cargoPath = path.join(ROOT, 'Cargo.toml');
const cargoContent = fs.readFileSync(cargoPath, 'utf8');
const vMatch = cargoContent.match(/^version\s*=\s*"(\d+\.\d+\.\d+)"/m);
if (!vMatch) {
  console.error('Could not find version in Cargo.toml');
  process.exit(1);
}
const oldVersion = vMatch[1];
const [maj, min, pat] = oldVersion.split('.').map(Number);

let newVersion;
if (kind === 'major') newVersion = `${maj + 1}.0.0`;
else if (kind === 'minor') newVersion = `${maj}.${min + 1}.0`;
else newVersion = `${maj}.${min}.${pat + 1}`;

console.log(`${oldVersion} -> ${newVersion}  (${kind})`);

// Helper: read, replace, write (silent if file missing)
function replaceInFile(relPath, search, replacement) {
  const filePath = path.join(ROOT, relPath);
  if (!fs.existsSync(filePath)) return;
  let content = fs.readFileSync(filePath, 'utf8');
  const updated = content.replace(search, replacement);
  if (updated !== content) {
    fs.writeFileSync(filePath, updated, 'utf8');
    console.log(`  updated ${relPath}`);
  }
}

// 2. Cargo.toml (root)
replaceInFile('Cargo.toml', `version = "${oldVersion}"`, `version = "${newVersion}"`);

// 2b. Cargo.lock's own entry (2026-10-12). Cargo did not always rewrite it on the release build,
// so the lock lagged a version behind and a later build changed it, which changed the source
// fingerprint and made the delivered exe look stale. Set to the new version whatever it held.
// Every workspace member's entry (2026-11-01, the crate split's rung 1): they all carry the one
// version in [workspace.package], so a lock entry left behind would change the stamp the same way.
replaceInFile('Cargo.lock', /(name = "humanity-[a-z-]+"\r?\nversion = ")[^"]*(")/g, `$1${newVersion}$2`);

// 3. web/shared/sw.js — CACHE_NAME bump
const swPath = path.join(ROOT, 'web/shared/sw.js');
if (fs.existsSync(swPath)) {
  const swContent = fs.readFileSync(swPath, 'utf8');
  const cacheMatch = swContent.match(/humanity-v(\d+)/);
  if (cacheMatch) {
    const oldNum = parseInt(cacheMatch[1], 10);
    const newNum = oldNum + 1;
    fs.writeFileSync(swPath, swContent.replace(`humanity-v${oldNum}`, `humanity-v${newNum}`), 'utf8');
    console.log(`  updated web/shared/sw.js  (humanity-v${oldNum} -> humanity-v${newNum})`);
  }
}

// 4. web/pages/settings-app.js (comma separator; the em-dash here was removed in the v0.364 app-wide sweep)
replaceInFile('web/pages/settings-app.js', `HumanityOS, v${oldVersion}`, `HumanityOS, v${newVersion}`);

// 5. web/pages/ops.html
replaceInFile('web/pages/ops.html', `'v${oldVersion}'`, `'v${newVersion}'`);

// 6. web/shared/shell.js
replaceInFile('web/shared/shell.js', `var CURRENT_VERSION = '${oldVersion}'`, `var CURRENT_VERSION = '${newVersion}'`);

// 7. web/pages/download.html
const dlPath = path.join(ROOT, 'web/pages/download.html');
if (fs.existsSync(dlPath)) {
  let dl = fs.readFileSync(dlPath, 'utf8');
  let updated = dl.split(`v${oldVersion}`).join(`v${newVersion}`);
  updated = updated.split(`'${oldVersion}'`).join(`'${newVersion}'`);
  if (updated !== dl) {
    fs.writeFileSync(dlPath, updated, 'utf8');
    console.log(`  updated web/pages/download.html`);
  }
}

// (Removed 2026-11-02, step 7b: the front page's "updates shipped" proof tile. The front door that
// replaced the page (docs/design/front-door.md) shows the releases themselves, from the server's
// /api/front-door, so there is no number on it to stamp.)

// (Removed v0.698.3: the web/activities/download.html fork is deleted. It was
// a stale duplicate that nginx served at /download while this script kept
// stamping BOTH copies, hiding the drift -- the fork froze at v0.36 framing
// anyway. /download now points at the maintained web/pages/download.html.)

console.log(`\nVersion bumped: ${oldVersion} -> ${newVersion}`);
