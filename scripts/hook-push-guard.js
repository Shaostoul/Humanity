#!/usr/bin/env node
// PreToolUse hook (matcher "Bash", filtered to git-push via the hook's `if`)
// - wired in .claude/settings.json.
//
// Before a `git push`, WARN (never block) about the two things that have
// repeatedly shipped broken releases from this checkout:
//
//   1. UNTRACKED .rs/.ron/.csv files. CI builds from a fresh checkout and fails
//      when code/data files were committed-but-never-`git add`ed (CLAUDE.md
//      item 12).
//   2. RENDERER-SHAPED commits with no passing `just verify-runtime` newer than
//      them. `just verify` is entirely static and cannot see a device-limit
//      rejection at startup (v0.782-784, three unbootable releases) or a
//      bind-group panic on world entry (v0.1029-1038, TEN releases). The gate
//      that CAN see them only helps if somebody runs it, so this says so at the
//      one moment it matters.
//
// Advisory only - it prints a heads-up and lets the push proceed. Reads
// tool_input.command from stdin. Pure Node stdlib.
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const REPO = path.resolve(__dirname, '..');
// Paths whose changes can only be judged by BOOTING. Keep this tight: a nag
// that fires on doc pushes gets ignored, and an ignored nag protects nothing.
const RUNTIME_SHAPED = [/^src\/renderer\//, /^src\/terrain\//, /^assets\/shaders\//];

function git(cmd) {
  return execSync(`git ${cmd}`, { cwd: REPO, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
}

// Commits that this push would send. Test seam: PUSH_GUARD_RANGE overrides the
// range so the check itself can be proven red on demand (it only changes WHICH
// commits are examined; there is no way to switch the warning off).
function pushRange() {
  if (process.env.PUSH_GUARD_RANGE) return process.env.PUSH_GUARD_RANGE;
  try {
    return `${git('rev-parse --abbrev-ref --symbolic-full-name @{u}')}..HEAD`;
  } catch {
    return 'origin/main..HEAD';
  }
}

// Newest verify-runtime result on disk, or null. Returns {pass, mtime, dir}.
function newestRuntimeVerdict() {
  const sweeps = path.join(REPO, '.probe-rig', 'verify', 'sweeps');
  if (!fs.existsSync(sweeps)) return null;
  let best = null;
  for (const d of fs.readdirSync(sweeps)) {
    const mf = path.join(sweeps, d, 'manifest.json');
    if (!fs.existsSync(mf)) continue;
    const mtime = fs.statSync(mf).mtimeMs;
    if (!best || mtime > best.mtime) {
      try {
        const m = JSON.parse(fs.readFileSync(mf, 'utf8'));
        best = { mtime, dir: d, pass: m.captured === m.total && (m.panics || 0) === 0, m };
      } catch {
        /* half-written manifest; ignore */
      }
    }
  }
  return best;
}

function runtimeWarning() {
  const range = pushRange();
  let files;
  try {
    files = git(`diff --name-only ${range}`).split('\n').filter(Boolean);
  } catch {
    return null; // no upstream yet, detached head, etc - stay quiet
  }
  const shaped = files.filter((f) => RUNTIME_SHAPED.some((re) => re.test(f)));
  if (!shaped.length) return null;

  // When was the newest renderer-shaped commit in this push made?
  let commitTs = 0;
  try {
    commitTs = Number(git(`log -1 --format=%ct ${range} -- ${shaped.map((f) => `"${f}"`).join(' ')}`)) * 1000;
  } catch {
    /* fall through with 0 */
  }
  const v = newestRuntimeVerdict();
  const list = shaped.slice(0, 5).join(', ') + (shaped.length > 5 ? ` (+${shaped.length - 5} more)` : '');

  if (!v) {
    return `PUSH GUARD: this push changes code that can only be judged by BOOTING (${list}), and there is no verify-runtime result on disk at all. Static verify cannot see a startup device-limit rejection (v0.782-784) or a world-entry panic (v0.1029-1038, ten releases). Run: just verify-runtime (~3 min).`;
  }
  if (!v.pass) {
    return `PUSH GUARD: this push changes boot-shaped code (${list}) and the most recent verify-runtime run FAILED (${v.m.captured}/${v.m.total} vantages, panics=${v.m.panics}, .probe-rig/verify/sweeps/${v.dir}). Fix it or re-run before pushing: just verify-runtime`;
  }
  if (commitTs && v.mtime < commitTs) {
    return `PUSH GUARD: this push changes boot-shaped code (${list}) that is NEWER than the last passing verify-runtime (${new Date(v.mtime).toLocaleString()}). That pass was about a different binary. Re-run: just verify-runtime (~3 min).`;
  }
  return null;
}

let input = '';
process.stdin.on('data', (d) => (input += d));
process.stdin.on('end', () => {
  const warnings = [];
  try {
    const j = JSON.parse(input || '{}');
    const cmd = (j.tool_input && j.tool_input.command) || '';
    if (!/\bgit\s+push\b/.test(cmd)) return; // belt-and-suspenders (the hook `if` also filters)

    try {
      const out = git('status --porcelain');
      const untracked = out
        .split('\n')
        .filter((l) => l.startsWith('??') && /\.(rs|ron|csv)$/.test(l.trim()))
        .map((l) => l.slice(3).trim());
      if (untracked.length) {
        warnings.push(
          'PUSH GUARD: untracked code/data files are NOT staged - CI builds from a fresh checkout and will fail without them: ' +
            untracked.join(', ') +
            '. Stage them (git add) before this push, or confirm they are intentionally ignored.'
        );
      }
    } catch (e) {
      /* one check failing must not suppress the other */
    }

    try {
      const rw = runtimeWarning();
      if (rw) warnings.push(rw);
    } catch (e) {
      /* advisory only */
    }

    if (warnings.length) {
      process.stdout.write(JSON.stringify({ systemMessage: warnings.join('\n\n') }));
    }
  } catch (e) {
    // Advisory only - never block a push because the check itself errored.
  }
});
