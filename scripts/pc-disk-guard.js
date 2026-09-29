#!/usr/bin/env node
/**
 * pc-disk-guard: keeps the dev PC's C: drive from filling with build output
 * (2026-09-29). The PC twin of the VPS's scripts/humanity-disk-guard.sh.
 *
 * WHY: cargo never prunes its build folders. On 2026-09-29 the main target/
 * had grown to 1.13 TB (debug/incremental alone 688 GB) and the worktrees'
 * build folders to 102 GB, with C: at 91%; on 2026-09-27 parallel worktree
 * builds filled C: completely overnight. Nobody should have to notice and ask.
 *
 * WHAT IT DELETES: build output only (target/ folders), never source, saves,
 * screenshots, backups or git data. A deleted build folder costs a rebuild,
 * about 10 minutes, and nothing else.
 *
 *   1. Every run: the target/ of any worktree nobody has built in for
 *      IDLE_HOURS (48 h), judged by the newest of its lock and info files.
 *   2. C: at WARN_PCT (75%) or more: the main target's debug/incremental,
 *      the part that grows fastest.
 *   3. Still at CRIT_PCT (85%) or more: the whole main target/ (cargo clean).
 *
 * It does NOTHING while cargo, rustc or HumanityOS is running (a build or the
 * game may hold those files); it tries again next run.
 *
 * RUNS hourly and silently from Task Scheduler ("HumanityOS Dev Disk Guard",
 * installed by scripts/install-pc-disk-guard.ps1, `just install-disk-guard`).
 * By hand: `just disk-guard` (add `--dry-run` to only report).
 * Log: %LOCALAPPDATA%\HumanityOS\disk-guard.log; the last run's summary:
 * disk-guard-last.json beside it (`just brief` prints it as the DISK row).
 *
 * Testing flags: --root <dir> (a stand-in repo), --drive <path> (the drive to
 * measure), --dry-run. Thresholds from env: PC_DISK_GUARD_WARN_PCT,
 * PC_DISK_GUARD_CRIT_PCT, PC_DISK_GUARD_IDLE_HOURS.
 */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : dflt;
};
const ROOT = path.resolve(opt('--root', path.join(__dirname, '..')));
const DRIVE = opt('--drive', path.parse(ROOT).root);
const DRY = flag('--dry-run');
const WARN_PCT = Number(process.env.PC_DISK_GUARD_WARN_PCT || 75);
const CRIT_PCT = Number(process.env.PC_DISK_GUARD_CRIT_PCT || 85);
const IDLE_HOURS = Number(process.env.PC_DISK_GUARD_IDLE_HOURS || 48);

const STATE_DIR = path.join(process.env.LOCALAPPDATA || path.join(require('os').homedir(), 'AppData', 'Local'), 'HumanityOS');
const LOG = path.join(STATE_DIR, 'disk-guard.log');
const LAST = path.join(STATE_DIR, 'disk-guard-last.json');

function usage() {
  const s = fs.statfsSync(DRIVE);
  const total = s.blocks * s.bsize;
  const free = s.bavail * s.bsize;
  return { pct: Math.round((1 - free / total) * 1000) / 10, freeGB: Math.round(free / 1e9) };
}

function busy() {
  // Anything that may be writing or holding build output right now
  // (PC_DISK_GUARD_BUSY overrides the list, for the test).
  const names = (process.env.PC_DISK_GUARD_BUSY || 'cargo.exe,rustc.exe,HumanityOS.exe').split(',');
  try {
    const out = execSync('tasklist /FO CSV /NH', { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], windowsHide: true });
    return names.filter((n) => out.toLowerCase().includes('"' + n.toLowerCase() + '"'));
  } catch {
    return []; // tasklist unavailable (not Windows): nothing known to be running
  }
}

function lastBuiltMs(target) {
  let newest = 0;
  for (const rel of ['.rustc_info.json', 'debug/.cargo-lock', 'release/.cargo-lock', 'debug', 'release', '.']) {
    try {
      newest = Math.max(newest, fs.statSync(path.join(target, rel)).mtimeMs);
    } catch { /* not there */ }
  }
  return newest;
}

const actions = [];
function remove(dir, why) {
  if (!fs.existsSync(dir)) return;
  actions.push((DRY ? 'would delete ' : 'deleted ') + path.relative(ROOT, dir) + ' (' + why + ')');
  if (!DRY) fs.rmSync(dir, { recursive: true, force: true, maxRetries: 3 });
}

const before = usage();
let skipped = null;
const running = busy();
if (running.length) {
  skipped = 'busy: ' + running.join(', ') + ' running, nothing touched';
} else {
  // 1. Idle worktree build folders.
  const wtRoot = path.join(ROOT, '.claude', 'worktrees');
  const idleMs = IDLE_HOURS * 3600 * 1000;
  let names = [];
  try { names = fs.readdirSync(wtRoot); } catch { /* no worktrees */ }
  for (const name of names) {
    const target = path.join(wtRoot, name, 'target');
    if (!fs.existsSync(target)) continue;
    const age = Date.now() - lastBuiltMs(target);
    if (age > idleMs) remove(target, 'worktree idle ' + Math.round(age / 3600000) + ' h');
  }
  // 2 and 3. The main build folder, only when the drive is filling.
  const main = path.join(ROOT, 'target');
  if (usage().pct >= WARN_PCT || (DRY && before.pct >= WARN_PCT)) {
    remove(path.join(main, 'debug', 'incremental'), 'C: at ' + usage().pct + '%');
  }
  if (usage().pct >= CRIT_PCT || (DRY && before.pct >= CRIT_PCT)) {
    remove(main, 'C: still at ' + usage().pct + '%');
  }
}
const after = usage();

const summary = {
  at: new Date().toISOString(),
  dry_run: DRY,
  before_pct: before.pct,
  after_pct: after.pct,
  free_gb: after.freeGB,
  actions,
  skipped,
};
const line = `${summary.at} C: ${before.pct}% -> ${after.pct}% (${after.freeGB} GB free)` +
  (skipped ? ' | ' + skipped : actions.length ? ' | ' + actions.join('; ') : ' | nothing to do');
console.log(line);
if (!DRY && !flag('--no-log')) {
  try {
    fs.mkdirSync(STATE_DIR, { recursive: true });
    fs.appendFileSync(LOG, line + '\n');
    // Keep the log small: the last 500 runs.
    const lines = fs.readFileSync(LOG, 'utf8').split('\n').filter(Boolean);
    if (lines.length > 500) fs.writeFileSync(LOG, lines.slice(-500).join('\n') + '\n');
    fs.writeFileSync(LAST, JSON.stringify(summary, null, 2) + '\n');
  } catch (e) {
    console.error('could not write the log: ' + e.message);
  }
}
