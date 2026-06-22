#!/usr/bin/env node
/**
 * `just brief` -- one-shot orientation for a fresh session (AI or human).
 *
 * Surfaces in ONE screen the things the START-HERE checklist reads one file at a
 * time: version drift vs the latest release, the last CI deploy result, the
 * release-signing state (the desktop auto-update gate), and the newest journal
 * decision + current focus. Every external (`gh`) call degrades gracefully so this
 * works offline.
 *
 * Why this exists: the version-sync ritual, the "is CI red?" check, and the
 * "what was the last session doing?" read are the three things most likely to be
 * stale-or-surprising at session start, and nothing surfaced them together.
 */
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const root = path.join(__dirname, '..');

function sh(cmd) {
  try {
    return execSync(cmd, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch (e) {
    // Some helpers (check-release-signing) exit non-zero by design but still print
    // useful stdout; surface it rather than dropping the whole line.
    const out = (e && e.stdout ? String(e.stdout).trim() : '');
    return out || null;
  }
}

function semver(v) {
  return String(v || '').replace(/^v/, '').split('.').map((n) => parseInt(n, 10) || 0);
}
function cmpVer(a, b) {
  const x = semver(a), y = semver(b);
  for (let i = 0; i < 3; i++) {
    if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) - (y[i] || 0);
  }
  return 0;
}

// ── 1. Version drift (local Cargo.toml vs the latest GitHub release) ──
const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const localV = (cargo.match(/^version\s*=\s*"(.+?)"/m) || [])[1] || '?';

const relLine = sh('gh release list --repo Shaostoul/Humanity --limit 1');
let latestRel = null;
if (relLine) {
  // Tab/space separated columns vary by gh version; pull the first vX.Y.Z token.
  const m = relLine.match(/v?\d+\.\d+\.\d+/);
  if (m) latestRel = m[0].replace(/^v?/, 'v');
}

let versionLine;
if (!latestRel) {
  versionLine = `local ${localV}  (could not read GitHub releases -- offline?)`;
} else {
  const c = cmpVer(localV, latestRel);
  if (c > 0) versionLine = `LOCAL AHEAD: local ${localV} > release ${latestRel}  -- push + tag + release (+ sign) per the SOP`;
  else if (c < 0) versionLine = `LOCAL BEHIND: local ${localV} < release ${latestRel}  -- investigate (local should never trail)`;
  else versionLine = `in sync at ${localV}`;
}

// ── 2. Last CI deploy ──
const ci = sh('gh run list --repo Shaostoul/Humanity --workflow "Deploy to VPS" --limit 1');

// ── 3. Release signing (the desktop auto-update gate) ──
const signing = sh(`node "${path.join(__dirname, 'check-release-signing.js')}" 3`);

// ── 4. Journal: current focus + newest decision ──
let focus = null, lastDecision = null;
try {
  const j = JSON.parse(fs.readFileSync(path.join(root, 'data', 'coordination', 'orchestrator_state.json'), 'utf8'));
  if (j.current_focus) focus = j.current_focus;
  const rd = j.recent_decisions;
  if (Array.isArray(rd) && rd.length) {
    const last = rd[rd.length - 1];
    lastDecision = `${last.at || '?'}: ${String(last.decision || '').slice(0, 360)}${(last.decision || '').length > 360 ? '...' : ''}`;
  }
} catch { /* journal unreadable */ }

// ── Render ──
const L = [];
L.push('===================  HumanityOS brief  ===================');
L.push('');
L.push('VERSION:  ' + versionLine);
L.push('');
L.push('CI DEPLOY (latest):');
L.push(ci ? '  ' + ci.split('\n')[0] : '  (unavailable -- offline? check: just ci)');
L.push('');
L.push('SIGNING:  ' + (signing ? signing.split('\n').slice(0, 3).join('\n          ') : '(check unavailable -- run: just check-signing)'));
L.push('');
if (focus) {
  L.push('FOCUS (orchestrator_state.json):');
  L.push('  ' + focus.replace(/\s+/g, ' ').slice(0, 600));
  L.push('');
}
if (lastDecision) {
  L.push('LAST DECISION:');
  L.push('  ' + lastDecision.replace(/\s+/g, ' '));
  L.push('');
}
L.push('NEXT: docs/PRIORITIES.md (top of "Active focus") is the next action.');
L.push('      `node scripts/agent-status.js` for per-scope status (now flags stale rows).');
L.push('==========================================================');
console.log(L.join('\n'));
