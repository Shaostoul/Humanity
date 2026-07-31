#!/usr/bin/env node
/**
 * Lane map for concurrent Claude sessions sharing ONE checkout.
 *
 *   node scripts/lanes.js                  print the lane map
 *   node scripts/lanes.js --check-staged   warn if the staged set spans lanes
 *   node scripts/lanes.js --check <paths>  which lane owns these paths
 *
 * Why this exists: several sessions edit this repo at the same time, in the same
 * working directory. There is no per-session state on disk to key off (same cwd,
 * same git index), so ownership cannot be detected automatically. This file makes
 * ownership legible so a session can stay out of another's way, and flags the
 * signature of an accidental sweep: one commit spanning every lane at once.
 *
 * Advisory by design. The mechanical protection is that `just ship` commits only
 * what you staged (Justfile `_commit`), so nobody's in-flight work can be swept
 * into someone else's commit.
 */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const MAP = path.join(ROOT, 'data', 'coordination', 'lanes.json');

let map;
try {
  map = JSON.parse(fs.readFileSync(MAP, 'utf8'));
} catch (e) {
  console.error('lanes: could not read data/coordination/lanes.json: ' + e.message);
  process.exit(1);
}

const norm = (p) => p.replace(/\\/g, '/').replace(/^\.\//, '');

/** The lane that owns `file`, or null. Longest matching prefix wins. */
function laneFor(file) {
  const f = norm(file);
  let best = null;
  let bestLen = -1;
  for (const lane of map.lanes || []) {
    for (const own of lane.owns || []) {
      const o = norm(own);
      const hit = o.endsWith('/') ? f.startsWith(o) : f === o;
      if (hit && o.length > bestLen) {
        best = lane;
        bestLen = o.length;
      }
    }
  }
  return best;
}

function isShared(file) {
  const f = norm(file);
  return ((map.shared && map.shared.paths) || []).some((p) => norm(p) === f);
}

function printMap() {
  console.log('');
  console.log('Lanes (data/coordination/lanes.json) - several Claude sessions share this checkout.');
  console.log('');
  for (const lane of map.lanes || []) {
    console.log('  ' + lane.id.padEnd(8) + lane.label);
    for (const own of lane.owns || []) console.log('           ' + own);
    console.log('');
  }
  console.log('  shared   touched by everyone, most likely to collide:');
  for (const p of (map.shared && map.shared.paths) || []) console.log('           ' + p);
  console.log('');
  console.log('  Before editing outside your lane, expect someone else is in there.');
  console.log('  Stage only your own files: just mine <paths>   then: just ship "msg"');
  console.log('');
}

function stagedFiles() {
  try {
    return execSync('git diff --cached --name-only', { cwd: ROOT, encoding: 'utf8' })
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);
  } catch (e) {
    return [];
  }
}

function report(files, { warnOnSpread }) {
  if (!files.length) {
    console.log('lanes: nothing staged.');
    return 0;
  }
  const byLane = new Map();
  const shared = [];
  const unowned = [];
  for (const f of files) {
    if (isShared(f)) { shared.push(f); continue; }
    const lane = laneFor(f);
    if (!lane) { unowned.push(f); continue; }
    if (!byLane.has(lane.id)) byLane.set(lane.id, []);
    byLane.get(lane.id).push(f);
  }

  for (const [id, fs_] of byLane) {
    console.log('  lane ' + id + ': ' + fs_.length + ' file' + (fs_.length === 1 ? '' : 's'));
  }
  if (shared.length) console.log('  shared: ' + shared.length + ' (' + shared.join(', ') + ')');
  if (unowned.length) console.log('  unclaimed: ' + unowned.length);

  // The sweep signature: one commit touching every lane at once. That is almost
  // always `git add -A` in a shared tree rather than a genuine cross-cutting change.
  if (warnOnSpread && byLane.size >= 3) {
    console.log('');
    console.log('  ! This commit spans ' + byLane.size + ' lanes at once.');
    console.log('    In a shared checkout that usually means another session\'s work got');
    console.log('    swept in. Check `git diff --cached --name-only` and unstage what is');
    console.log('    not yours: git restore --staged <paths>');
    console.log('');
  }
  return byLane.size;
}

const args = process.argv.slice(2);
if (args[0] === '--check-staged') {
  report(stagedFiles(), { warnOnSpread: true });
} else if (args[0] === '--check') {
  report(args.slice(1), { warnOnSpread: true });
} else {
  printMap();
}
