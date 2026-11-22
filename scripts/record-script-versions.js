#!/usr/bin/env node
// Record the bytes each cache-busting version of a web script means (2026-11-21, D8 of the
// signed-requests round 2).
//
// A page loads a script as `<script src="/chat/pq.js?v=N">`, and a browser that has `?v=N` cached
// keeps running those bytes. So a version number must mean ONE set of bytes for ever: change the
// script, change N. Round 1 of the signed requests bumped pq.js to ?v=2 and pq-relay-auth.js to
// ?v=3, numbers main had already used for older files, so a browser holding the old files kept
// them on several pages and signed requests the relay no longer took.
//
// The table: scripts/tests/fixtures/script-versions.json, { "<script path>": { "<version>":
// "<sha256 of its bytes, line endings as LF>" } }. scripts/tests/script-versions-web.test.js holds
// every page to it: each versioned script a page loads is in the table at that version with
// exactly its bytes, and every page loads a script at the same version.
//
// Usage, after changing a script and giving it a NEW version on every page that loads it:
//   node scripts/record-script-versions.js          record the new versions (refuses a reuse)
//   node scripts/record-script-versions.js --check  only say what is wrong (the test's check)
//   node scripts/record-script-versions.js --backfill [ref]  once: add every version main's history
//                                                    gave each script (default ref origin/main)
//
// A recorded version is never rewritten: when a version already means other bytes, it refuses and
// names the script, so the fix is always a fresh number (a date such as 20261121 never collides).
//
// VERSIONS ONLY RISE (2026-11-21, signed requests round 3, C9). The table first held only the
// versions in use that day, so a number used before it (pq.js was loaded at ?v=1, then ?v=2) could be
// taken again with new bytes, and nothing stopped a page going DOWN to an older number a browser
// may still hold. So: the table was backfilled once from git (every version any page under web/
// ever loaded each script at, marked "git:<commit>" where the bytes were not recorded), a version
// is a whole number, and a new one must be greater than every version recorded for that script.

'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.join(__dirname, '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const TABLE = process.env.HOS_SCRIPT_VERSIONS || path.join(ROOT, 'scripts', 'tests', 'fixtures', 'script-versions.json');

/** Every HTML page under web/ (not the desktop's bundled copy, not vendored code). */
function pages() {
  const out = [];
  (function walk(dir) {
    for (const e of fs.readdirSync(path.join(WEB, dir), { withFileTypes: true })) {
      const rel = dir ? dir + '/' + e.name : e.name;
      if (e.isDirectory()) {
        if (!['app', 'vendor', 'node_modules'].includes(e.name)) walk(rel);
      } else if (e.name.endsWith('.html')) out.push(rel);
    }
  })('');
  return out.sort();
}

/** The versioned script references of every page: [{page, script, version}]. */
function references() {
  const refs = [];
  for (const page of pages()) {
    const html = fs.readFileSync(path.join(WEB, page), 'utf8');
    for (const m of html.matchAll(/<script[^>]*\ssrc="(\/[^"?#]+\.js)\?v=([^"&#]+)"/g)) {
      refs.push({ page, script: m[1].replace(/^\//, ''), version: m[2] });
    }
  }
  return refs;
}

/** SHA-256 of a script's bytes with CRLF read as LF (a Windows checkout and the server agree). */
function hashOf(script) {
  const p = path.join(WEB, script);
  if (!fs.existsSync(p)) return null;
  const text = fs.readFileSync(p, 'utf8').replace(/\r\n/g, '\n');
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

function readTable() {
  return fs.existsSync(TABLE) ? JSON.parse(fs.readFileSync(TABLE, 'utf8')) : {};
}

/** Whether `version` is a whole number (every version is). */
function isWhole(version) {
  return /^[0-9]{1,15}$/.test(String(version));
}

/** The greatest version recorded for a script's row, 0 when none. */
function highest(row) {
  let top = 0;
  for (const v of Object.keys(row || {})) if (isWhole(v) && Number(v) > top) top = Number(v);
  return top;
}

/** Why `version` may not be recorded new for a row, or '' when it may: whole, and above every one. */
function newVersionProblem(script, version, row) {
  if (!isWhole(version)) return script + '?v=' + version + ' is not a whole number: a version is one.';
  const top = highest(row);
  if (Number(version) <= top) {
    return script + '?v=' + version + ' is not greater than ' + top + ', the highest version recorded for it: versions only rise (a lower or reused number may be in a browser\'s cache with other bytes). Use ' + (top + 1) + ' or more.';
  }
  return '';
}

/**
 * Every version main's history gave each script: { script: { version: commit } }, from the added
 * and removed `<script src="/x.js?v=N">` lines of every page under web/ (not the desktop's bundled
 * copy under web/app, not vendored code), the commit the oldest of them came in.
 */
function historyVersions(ref) {
  const { execFileSync } = require('child_process');
  const log = execFileSync('git', ['log', ref, '--format=@@commit %H', '-p', '--no-color', '--unified=0', '--', 'web'], {
    cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 30,
  });
  const found = {};
  let commit = null;
  let file = '';
  for (const line of log.split('\n')) {
    if (line.startsWith('@@commit ')) { commit = line.slice(9).trim(); continue; }
    if (line.startsWith('diff --git ')) { file = line.split(' b/')[1] || ''; continue; }
    if (!file.endsWith('.html') || /(^|\/)(app|vendor|node_modules)\//.test(file.replace(/^web\//, ''))) continue;
    if (!(line.startsWith('+') || line.startsWith('-')) || line.startsWith('+++') || line.startsWith('---')) continue;
    for (const m of line.matchAll(/<script[^>]*\ssrc="(\/[^"?#]+\.js)\?v=([^"&#]+)"/g)) {
      const script = m[1].replace(/^\//, '');
      // The log runs newest first, so the last commit seen for a version is the oldest.
      (found[script] || (found[script] = {}))[m[2]] = commit;
    }
  }
  return found;
}

/** Add history's versions to the table (never touching one already recorded). */
function backfill(ref) {
  const table = readTable();
  let added = 0;
  for (const [script, versions] of Object.entries(historyVersions(ref))) {
    const row = table[script] || (table[script] = {});
    for (const [version, commit] of Object.entries(versions)) {
      if (!row[version]) { row[version] = 'git:' + String(commit).slice(0, 12); added++; }
    }
  }
  writeTable(table);
  console.log('backfilled ' + added + ' version(s) from ' + ref + ' into ' + path.relative(ROOT, TABLE));
}

/** Write the table sorted by script, then by version as a number. */
function writeTable(table) {
  const sorted = {};
  for (const k of Object.keys(table).sort()) {
    sorted[k] = {};
    for (const v of Object.keys(table[k]).sort((a, b) => (Number(a) - Number(b)) || a.localeCompare(b))) sorted[k][v] = table[k][v];
  }
  fs.writeFileSync(TABLE, JSON.stringify(sorted, null, 2) + '\n');
}

/** What is wrong with the pages against the table, in words; [] when nothing. */
function problems(table) {
  const out = [];
  const byScript = new Map();
  for (const r of references()) {
    if (!byScript.has(r.script)) byScript.set(r.script, new Map());
    const v = byScript.get(r.script);
    if (!v.has(r.version)) v.set(r.version, []);
    v.get(r.version).push(r.page);
  }
  for (const [script, versions] of byScript) {
    if (versions.size > 1) {
      const list = [...versions].map(([v, ps]) => v + ' (' + ps.join(', ') + ')').join('; ');
      out.push(script + ' is loaded at more than one version: ' + list + '. Every page loads its newest.');
    }
    const now = hashOf(script);
    for (const [version, ps] of versions) {
      const recorded = table[script] && table[script][version];
      const lower = recorded ? '' : newVersionProblem(script, version, table[script]);
      if (now === null) out.push(script + ' is loaded by ' + ps.join(', ') + ' but does not exist.');
      else if (lower) out.push(lower);
      else if (!recorded) out.push(script + '?v=' + version + ' is not recorded: run node scripts/record-script-versions.js.');
      else if (recorded !== now) {
        out.push(script + ' changed but ?v=' + version + ' already meant other bytes (' + ps.join(', ') + '): give it a new version on every page that loads it, then run node scripts/record-script-versions.js.');
      }
    }
  }
  return out;
}

function record() {
  const table = readTable();
  const refused = [];
  let added = 0;
  for (const r of references()) {
    const now = hashOf(r.script);
    if (now === null) continue;
    const row = table[r.script] || (table[r.script] = {});
    if (!row[r.version]) {
      const why = newVersionProblem(r.script, r.version, row);
      if (why) refused.push(why);
      else { row[r.version] = now; added++; }
    } else if (row[r.version] !== now) refused.push(r.script + '?v=' + r.version + ' already meant other bytes: choose a new version.');
  }
  writeTable(table);
  for (const line of [...new Set(refused)]) console.error('REFUSED: ' + line);
  console.log('recorded ' + added + ' new version(s) in ' + path.relative(ROOT, TABLE));
  return refused.length === 0;
}

// record and highest are scripts/bump-version.js's (step 8: every release moves the scripts it
// stamps to a fresh version and records it).
module.exports = { references, hashOf, readTable, problems, historyVersions, newVersionProblem, record, highest, TABLE, WEB };

if (require.main === module) {
  const at = process.argv.indexOf('--backfill');
  if (at >= 0) {
    backfill(process.argv[at + 1] && !process.argv[at + 1].startsWith('--') ? process.argv[at + 1] : 'origin/main');
    process.exit(0);
  }
  if (process.argv.includes('--check')) {
    const p = problems(readTable());
    for (const line of p) console.error(line);
    process.exit(p.length ? 1 : 0);
  }
  process.exit(record() ? 0 : 1);
}
