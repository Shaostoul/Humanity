#!/usr/bin/env node
// bump-version.js — Bumps version strings across all locations.
// Usage: node scripts/bump-version.js [patch|minor|major] [--changed-list <file>]
//   --changed-list <file>  write every file the bump changed, one repo-relative path per line, so the
//                          release commit stages exactly those (the pages whose ?v= moved and the
//                          script-version table included: step 8)

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const kind = (process.argv[2] || 'patch').toLowerCase();
const listAt = process.argv.indexOf('--changed-list');
const changedListPath = listAt > 0 ? process.argv[listAt + 1] : null;
if (listAt > 0 && !changedListPath) {
  console.error('--changed-list needs a file to write the changed files into.');
  process.exit(1);
}
/** Every file this bump wrote, repo-relative with forward slashes, in the order written. */
const changed = new Set();
function noteChanged(relPath) {
  changed.add(relPath.split(path.sep).join('/'));
}

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
    noteChanged(relPath);
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
// EXCEPT a member with a version of its own (2026-11-09): humanity-ai-protocol is 1.0.0 because the
// AI engine compiles it too and the app's version would change the engine's bytes every release
// (crates/humanity-ai-protocol/Cargo.toml). Rewriting its lock entry to the app's version made the
// lock disagree with its Cargo.toml, so only members that take `version.workspace = true` are set.
const ownVersioned = new Set();
for (const dir of fs.readdirSync(path.join(ROOT, 'crates'))) {
  const toml = path.join(ROOT, 'crates', dir, 'Cargo.toml');
  if (!fs.existsSync(toml)) continue;
  const text = fs.readFileSync(toml, 'utf8');
  const name = (text.match(/^name\s*=\s*"([^"]+)"/m) || [])[1];
  if (name && /^version\s*=\s*"/m.test(text)) ownVersioned.add(name);
}
replaceInFile('Cargo.lock', /(name = "(humanity-[a-z-]+)"\r?\nversion = ")[^"]*(")/g,
  (whole, head, name, tail) => (ownVersioned.has(name) ? whole : `${head}${newVersion}${tail}`));

// 3. web/shared/sw.js — CACHE_NAME bump
const swPath = path.join(ROOT, 'web/shared/sw.js');
if (fs.existsSync(swPath)) {
  const swContent = fs.readFileSync(swPath, 'utf8');
  const cacheMatch = swContent.match(/humanity-v(\d+)/);
  if (cacheMatch) {
    const oldNum = parseInt(cacheMatch[1], 10);
    const newNum = oldNum + 1;
    fs.writeFileSync(swPath, swContent.replace(`humanity-v${oldNum}`, `humanity-v${newNum}`), 'utf8');
    noteChanged('web/shared/sw.js');
    console.log(`  updated web/shared/sw.js  (humanity-v${oldNum} -> humanity-v${newNum})`);
  }
}

// 4. web/pages/settings-app.js (comma separator; the em-dash here was removed in the v0.364 app-wide sweep)
replaceInFile('web/pages/settings-app.js', `HumanityOS, v${oldVersion}`, `HumanityOS, v${newVersion}`);

// 5. web/pages/ops-app.js (the Ops page's script, out of ops.html since 2026-11-20: the site's
// policy runs no inline script; scripts/tests/version-stamps-web.test.js checks every stamp)
replaceInFile('web/pages/ops-app.js', `'v${oldVersion}'`, `'v${newVersion}'`);

// 6. web/shared/shell.js
replaceInFile('web/shared/shell.js', `var CURRENT_VERSION = '${oldVersion}'`, `var CURRENT_VERSION = '${newVersion}'`);

// 7. web/pages/download.html (its markup) and web/pages/download-app.js (its script, out of the
// page since 2026-11-20 for the same reason as step 5): every copy of the version in each.
for (const rel of ['web/pages/download.html', 'web/pages/download-app.js']) {
  const dlPath = path.join(ROOT, rel);
  if (!fs.existsSync(dlPath)) continue;
  const dl = fs.readFileSync(dlPath, 'utf8');
  let updated = dl.split(`v${oldVersion}`).join(`v${newVersion}`);
  updated = updated.split(`'${oldVersion}'`).join(`'${newVersion}'`);
  if (updated !== dl) {
    fs.writeFileSync(dlPath, updated, 'utf8');
    noteChanged(rel);
    console.log(`  updated ${rel}`);
  }
}

// (Removed 2026-11-02, step 7b: the front page's "updates shipped" proof tile. The front door that
// replaced the page (docs/design/front-door.md) shows the releases themselves, from the server's
// /api/front-door, so there is no number on it to stamp.)

// (Removed v0.698.3: the web/activities/download.html fork is deleted. It was
// a stale duplicate that nginx served at /download while this script kept
// stamping BOTH copies, hiding the drift -- the fork froze at v0.36 framing
// anyway. /download now points at the maintained web/pages/download.html.)

// 8. A fresh ?v= for every script this bump stamped, recorded (2026-11-21, signed requests round 5).
// A page loads a script as `<script src="/shared/shell.js?v=N">`, and a browser holding ?v=N keeps
// those bytes, so a version means one set of bytes for ever (scripts/record-script-versions.js and
// its table, scripts/tests/fixtures/script-versions.json). The steps above change the bytes of
// shell.js and settings-app.js (and any script a later step stamps); left at their old ?v=, the
// table failed after every release and a browser kept the old copy. So the bump itself moves each
// stamped script to a new version on every page that loads it at one, and records it: the release
// cannot forget it. The new number is today's date (YYYYMMDD), or one above the highest the script
// ever had when that is higher (a second release the same day), because versions only rise.
// A script that every page loads bare (no ?v=) has no number to move: the server answers those
// with `no-cache, must-revalidate`, so a browser revalidates them on every load.
const versions = require('./record-script-versions.js');
const tableBefore = fs.existsSync(versions.TABLE) ? fs.readFileSync(versions.TABLE, 'utf8') : null;
const table = versions.readTable();
const now = new Date();
const today = now.getFullYear() * 10000 + (now.getMonth() + 1) * 100 + now.getDate();
const loadsNow = versions.references();
for (const rel of [...changed]) {
  if (!rel.startsWith('web/') || !rel.endsWith('.js')) continue;
  const script = rel.slice('web/'.length);
  const loads = loadsNow.filter((r) => r.script === script);
  if (loads.length === 0) continue;
  let top = versions.highest(table[script]);
  for (const r of loads) if (/^[0-9]+$/.test(r.version)) top = Math.max(top, Number(r.version));
  const fresh = String(Math.max(today, top + 1));
  const esc = script.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const ref = new RegExp('(<script[^>]*\\ssrc="/' + esc + '\\?v=)[^"&#]+(")', 'g');
  for (const page of [...new Set(loads.map((r) => r.page))]) {
    const pagePath = path.join(versions.WEB, page);
    const html = fs.readFileSync(pagePath, 'utf8');
    const updated = html.replace(ref, (whole, head, tail) => head + fresh + tail);
    if (updated !== html) {
      fs.writeFileSync(pagePath, updated, 'utf8');
      noteChanged(path.relative(ROOT, pagePath));
      console.log(`  updated web/${page}  (${script}?v=${fresh})`);
    }
  }
}
const recorded = versions.record();
if (tableBefore === null || fs.readFileSync(versions.TABLE, 'utf8') !== tableBefore) noteChanged(path.relative(ROOT, versions.TABLE));

if (changedListPath) fs.writeFileSync(changedListPath, [...changed].join('\n') + '\n', 'utf8');
if (!recorded) {
  console.error('\nThe script-version table refused a version (above): a page loads a script at a version that'
    + ' already means other bytes. Give that script a new ?v= on every page that loads it, run'
    + ' node scripts/record-script-versions.js, and check with node --test scripts/tests/script-versions-web.test.js.');
  process.exit(1);
}

console.log(`\nVersion bumped: ${oldVersion} -> ${newVersion}`);
