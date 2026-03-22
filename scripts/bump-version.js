#!/usr/bin/env node
// bump-version.js — Bumps version strings across all 6 locations.
// Usage: node scripts/bump-version.js [patch|minor|major]

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const kind = (process.argv[2] || 'patch').toLowerCase();

if (!['patch', 'minor', 'major'].includes(kind)) {
  console.error(`Invalid bump type: "${kind}". Use patch, minor, or major.`);
  process.exit(1);
}

// 1. Read current version from tauri.conf.json
const tauriConfPath = path.join(ROOT, 'app/tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
const oldVersion = tauriConf.version;
const [maj, min, pat] = oldVersion.split('.').map(Number);

let newVersion;
if (kind === 'major') newVersion = `${maj + 1}.0.0`;
else if (kind === 'minor') newVersion = `${maj}.${min + 1}.0`;
else newVersion = `${maj}.${min}.${pat + 1}`;

console.log(`${oldVersion} -> ${newVersion}  (${kind})`);

// Helper: read, replace, write
function replaceInFile(relPath, search, replacement) {
  const filePath = path.join(ROOT, relPath);
  let content = fs.readFileSync(filePath, 'utf8');
  const updated = content.replace(search, replacement);
  if (updated === content) {
    console.error(`  WARNING: no match in ${relPath} for: ${search}`);
    return;
  }
  fs.writeFileSync(filePath, updated, 'utf8');
  console.log(`  updated ${relPath}`);
}

// 2. tauri.conf.json — "version": "X.Y.Z"
tauriConf.version = newVersion;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n', 'utf8');
console.log('  updated app/tauri.conf.json');

// 3. app/Cargo.toml — version = "X.Y.Z"
replaceInFile(
  'app/Cargo.toml',
  `version = "${oldVersion}"`,
  `version = "${newVersion}"`
);

// 4. web/shared/sw.js — CACHE_NAME = 'humanity-vNN' (increment the number)
const swPath = path.join(ROOT, 'web/shared/sw.js');
const swContent = fs.readFileSync(swPath, 'utf8');
const cacheMatch = swContent.match(/humanity-v(\d+)/);
if (cacheMatch) {
  const oldCacheNum = parseInt(cacheMatch[1], 10);
  const newCacheNum = oldCacheNum + 1;
  const swUpdated = swContent.replace(`humanity-v${oldCacheNum}`, `humanity-v${newCacheNum}`);
  fs.writeFileSync(swPath, swUpdated, 'utf8');
  console.log(`  updated web/shared/sw.js  (humanity-v${oldCacheNum} -> humanity-v${newCacheNum})`);
} else {
  console.error('  WARNING: could not find CACHE_NAME in web/shared/sw.js');
}

// 5. web/pages/settings-app.js — 'HumanityOS — vX.Y.Z · '
replaceInFile(
  'web/pages/settings-app.js',
  `HumanityOS — v${oldVersion}`,
  `HumanityOS — v${newVersion}`
);

// 6. web/pages/ops.html — 'vX.Y.Z'
replaceInFile(
  'web/pages/ops.html',
  `'v${oldVersion}'`,
  `'v${newVersion}'`
);

// 7. web/shared/shell.js — CURRENT_VERSION = 'X.Y.Z'
replaceInFile(
  'web/shared/shell.js',
  `var CURRENT_VERSION = '${oldVersion}'`,
  `var CURRENT_VERSION = '${newVersion}'`
);

// 8. web/activities/download.html — version badge, subtitle, and JS fallback
const dlPath = path.join(ROOT, 'web/activities/download.html');
let dlContent = fs.readFileSync(dlPath, 'utf8');
// Replace both "v0.X.Y" (badge/subtitle) and "'0.X.Y'" (JS fallback) patterns
let dlUpdated = dlContent.split(`v${oldVersion}`).join(`v${newVersion}`);
dlUpdated = dlUpdated.split(`'${oldVersion}'`).join(`'${newVersion}'`);
if (dlUpdated === dlContent) {
  console.error('  WARNING: no match in web/activities/download.html');
} else {
  const count = (dlContent.length - dlUpdated.length === 0) ? 0 :
    (dlContent.split(`v${oldVersion}`).length - 1) + (dlContent.split(`'${oldVersion}'`).length - 1);
  fs.writeFileSync(dlPath, dlUpdated, 'utf8');
  console.log(`  updated web/activities/download.html  (${count} occurrence${count > 1 ? 's' : ''})`);
}

// 9. app/web/manifest.json — version field (if bundle exists)
const manifestPath = path.join(ROOT, 'app/web/manifest.json');
if (fs.existsSync(manifestPath)) {
  try {
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    manifest.version = newVersion;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n', 'utf8');
    console.log('  updated app/web/manifest.json');
  } catch (e) {
    console.error(`  WARNING: could not update manifest.json: ${e.message}`);
  }
} else {
  console.log('  skipped app/web/manifest.json (not found — run bundle-web first)');
}

console.log(`\nVersion bumped: ${oldVersion} -> ${newVersion}`);
