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

// 1. Read current version from native/Cargo.toml
const cargoPath = path.join(ROOT, 'native/Cargo.toml');
const cargoContent = fs.readFileSync(cargoPath, 'utf8');
const vMatch = cargoContent.match(/^version\s*=\s*"(\d+\.\d+\.\d+)"/m);
if (!vMatch) {
  console.error('Could not find version in native/Cargo.toml');
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

// 2. native/Cargo.toml
replaceInFile('native/Cargo.toml', `version = "${oldVersion}"`, `version = "${newVersion}"`);

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

// 4. web/pages/settings-app.js
replaceInFile('web/pages/settings-app.js', `HumanityOS — v${oldVersion}`, `HumanityOS — v${newVersion}`);

// 5. web/pages/ops.html
replaceInFile('web/pages/ops.html', `'v${oldVersion}'`, `'v${newVersion}'`);

// 6. web/shared/shell.js
replaceInFile('web/shared/shell.js', `var CURRENT_VERSION = '${oldVersion}'`, `var CURRENT_VERSION = '${newVersion}'`);

// 7. web/activities/download.html
const dlPath = path.join(ROOT, 'web/activities/download.html');
if (fs.existsSync(dlPath)) {
  let dl = fs.readFileSync(dlPath, 'utf8');
  let updated = dl.split(`v${oldVersion}`).join(`v${newVersion}`);
  updated = updated.split(`'${oldVersion}'`).join(`'${newVersion}'`);
  if (updated !== dl) {
    fs.writeFileSync(dlPath, updated, 'utf8');
    console.log(`  updated web/activities/download.html`);
  }
}

console.log(`\nVersion bumped: ${oldVersion} -> ${newVersion}`);
