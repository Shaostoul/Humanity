#!/usr/bin/env node
/**
 * clean-test-temp: deletes what test runs left in the system temp folder
 * (BUG-159, 2026-10-05).
 *
 * WHY: until 2026-10-05 about sixty test files each built their own path in
 * the temp folder (`hum_<what>_<pid>_<time>.db`, or a folder) and nothing
 * deleted it afterwards. By then the folder held 186,503 `hum_*` entries,
 * 76,906 of them SQLite databases, and every test run added more. The tests
 * now delete what they make (src/test_temp.rs). This recipe is the one-time
 * sweep of the pile, and the clean-up after a run that was killed before its
 * tests could delete their own files.
 *
 * WHAT IT DELETES: entries directly in the temp folder whose names start with
 * `hum_` (every test's temp path has that prefix) and that nothing has changed
 * for more than a day. Files and folders alike, nothing else in the folder. A
 * test running now made its files minutes ago, so the age keeps it safe too.
 *
 * It does NOTHING while cargo, rustc or a test binary is running: a test run
 * may be using its files right now. Try again when the builds are done.
 *
 * Usage: just clean-test-temp            (node scripts/clean-test-temp.js)
 *   --dir <folder>   sweep this folder instead of the temp folder (to try it
 *                    on a scratch folder)
 *   --days <n>       how long an entry must have been untouched (default 1)
 *   --dry-run        report what would go, delete nothing
 *
 * Tested by scripts/tests/clean-test-temp.test.js, on scratch folders only.
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execSync } = require('child_process');

const PREFIX = 'hum_';
const DAY_MS = 24 * 60 * 60 * 1000;

/** Is a running process with this image name a build or a test run? */
function isBuildOrTest(name) {
  const n = name.toLowerCase();
  if (n === 'cargo.exe' || n === 'rustc.exe' || n === 'cargo' || n === 'rustc') return true;
  // A cargo test binary is named after its crate or test file plus cargo's
  // 16-hex-digit hash (humanity_engine-01ca28c5dc016c23.exe); `just lints`
  // builds each lint as <name>.test.exe.
  return /-[0-9a-f]{16}(\.exe)?$/.test(n) || /\.test(\.exe)?$/.test(n);
}

/** The builds and test runs going on now, by image name. */
function busy() {
  let names = [];
  try {
    if (process.platform === 'win32') {
      const out = execSync('tasklist /FO CSV /NH', { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], windowsHide: true });
      names = out.split(/\r?\n/).map((line) => (line.match(/^"([^"]+)"/) || [])[1]).filter(Boolean);
    } else {
      const out = execSync('ps -eo args=', { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
      names = out.split('\n').map((line) => path.basename(line.trim().split(/\s+/)[0] || '')).filter(Boolean);
    }
  } catch {
    return ['(could not list the running processes)'];
  }
  return [...new Set(names.filter(isBuildOrTest))];
}

/** Bytes in a file, or in everything under a folder (links not followed). */
function sizeOf(p) {
  let st;
  try {
    st = fs.lstatSync(p);
  } catch {
    return 0;
  }
  if (!st.isDirectory()) return st.size;
  let sum = 0;
  let names = [];
  try {
    names = fs.readdirSync(p);
  } catch {
    return 0;
  }
  for (const n of names) sum += sizeOf(path.join(p, n));
  return sum;
}

/**
 * Delete the `hum_*` entries in `dir` untouched for more than `days` days.
 * Returns what was found, removed, and left (an entry still in use).
 */
function sweep({ dir, days = 1, dryRun = false, now = Date.now() }) {
  const cutoff = now - days * DAY_MS;
  const result = { seen: 0, old: 0, removedFiles: 0, removedFolders: 0, bytes: 0, left: [] };
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (!e.name.startsWith(PREFIX)) continue;
    result.seen++;
    const p = path.join(dir, e.name);
    let st;
    try {
      st = fs.lstatSync(p);
    } catch {
      continue; // gone already
    }
    if (st.mtimeMs > cutoff) continue;
    result.old++;
    const bytes = sizeOf(p);
    const folder = st.isDirectory();
    if (!dryRun) {
      try {
        fs.rmSync(p, { recursive: true, force: true });
      } catch (err) {
        result.left.push(`${e.name} (${err.code || err.message})`);
        continue;
      }
    }
    if (folder) result.removedFolders++;
    else result.removedFiles++;
    result.bytes += bytes;
  }
  return result;
}

function mb(bytes) {
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(2)} GB` : `${(bytes / 1e6).toFixed(1)} MB`;
}

function main() {
  const args = process.argv.slice(2);
  const opt = (name, dflt) => {
    const i = args.indexOf(name);
    return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : dflt;
  };
  const dir = path.resolve(opt('--dir', os.tmpdir()));
  const days = Number(opt('--days', '1'));
  const dryRun = args.includes('--dry-run');
  if (!(days >= 0)) {
    console.error(`clean-test-temp: --days must be a number of days, got ${opt('--days')}`);
    process.exit(2);
  }
  const running = busy();
  if (running.length) {
    console.log(`clean-test-temp: not now, a build or a test run is going: ${running.join(', ')}. Nothing was deleted; run it again when they are done.`);
    process.exit(1);
  }
  const t0 = Date.now();
  const r = sweep({ dir, days, dryRun });
  const removed = r.removedFiles + r.removedFolders;
  const verb = dryRun ? 'would remove' : 'removed';
  console.log(`clean-test-temp: ${dir}`);
  console.log(`  ${r.seen} ${PREFIX}* entries, ${r.old} untouched for more than ${days} day(s)`);
  console.log(`  ${verb} ${removed} (${r.removedFiles} files, ${r.removedFolders} folders), ${mb(r.bytes)}, in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  if (r.left.length) {
    console.log(`  ${r.left.length} could not be deleted (still in use?) and are left for next time:`);
    for (const l of r.left.slice(0, 20)) console.log(`    ${l}`);
    if (r.left.length > 20) console.log(`    ... and ${r.left.length - 20} more`);
  }
}

if (require.main === module) main();

module.exports = { sweep, isBuildOrTest, PREFIX };
