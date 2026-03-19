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
const tauriConfPath = path.join(ROOT, 'desktop/src-tauri/tauri.conf.json');
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
console.log('  updated desktop/src-tauri/tauri.conf.json');

// 3. desktop/src-tauri/Cargo.toml — version = "X.Y.Z"
replaceInFile(
  'desktop/src-tauri/Cargo.toml',
  `version = "${oldVersion}"`,
  `version = "${newVersion}"`
);

// 4. shared/sw.js — CACHE_NAME = 'humanity-vNN' (increment the number)
const swPath = path.join(ROOT, 'shared/sw.js');
const swContent = fs.readFileSync(swPath, 'utf8');
const cacheMatch = swContent.match(/humanity-v(\d+)/);
if (cacheMatch) {
  const oldCacheNum = parseInt(cacheMatch[1], 10);
  const newCacheNum = oldCacheNum + 1;
  const swUpdated = swContent.replace(`humanity-v${oldCacheNum}`, `humanity-v${newCacheNum}`);
  fs.writeFileSync(swPath, swUpdated, 'utf8');
  console.log(`  updated shared/sw.js  (humanity-v${oldCacheNum} -> humanity-v${newCacheNum})`);
} else {
  console.error('  WARNING: could not find CACHE_NAME in shared/sw.js');
}

// 5. pages/settings-app.js — 'HumanityOS — vX.Y.Z · '
replaceInFile(
  'pages/settings-app.js',
  `HumanityOS — v${oldVersion}`,
  `HumanityOS — v${newVersion}`
);

// 6. pages/ops.html — 'vX.Y.Z'
replaceInFile(
  'pages/ops.html',
  `'v${oldVersion}'`,
  `'v${newVersion}'`
);

// 7. shared/shell.js — CURRENT_VERSION = 'X.Y.Z'
replaceInFile(
  'shared/shell.js',
  `var CURRENT_VERSION = '${oldVersion}'`,
  `var CURRENT_VERSION = '${newVersion}'`
);

// 8. game/download.html — version badge and subtitle (two locations)
const dlPath = path.join(ROOT, 'game/download.html');
let dlContent = fs.readFileSync(dlPath, 'utf8');
const dlUpdated = dlContent.split(`v${oldVersion}`).join(`v${newVersion}`);
if (dlUpdated === dlContent) {
  console.error('  WARNING: no match in game/download.html');
} else {
  fs.writeFileSync(dlPath, dlUpdated, 'utf8');
  const count = dlContent.split(`v${oldVersion}`).length - 1;
  console.log(`  updated game/download.html  (${count} occurrence${count > 1 ? 's' : ''})`);
}

console.log(`\nVersion bumped: ${oldVersion} -> ${newVersion}`);
