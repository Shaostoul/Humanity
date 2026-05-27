#!/usr/bin/env node
// install-hooks.js — RUN THIS YOURSELF (the operator), not via the agent.
//
//   node scripts/install-hooks.js
//
// Claude can't write its own hook config (a deliberate safety gate — an AI
// shouldn't wire its own auto-running commands). So this installer exists for
// YOU to run: it does the config edits the agent is blocked from doing. It's
// fully transparent — read it, then run it. Idempotent + merge-safe (re-running
// won't duplicate hooks or clobber your other settings); backs up first.
//
// What it does:
//   1. .claude/settings.json      — adds 4 hooks (theme-sync, push-guard,
//      clean-worktrees, precompact-reminder), merging with anything already there.
//   2. .claude/settings.local.json — turns OFF "disableAllHooks" (it's currently
//      true, which silences ALL hooks) so the new ones can actually fire.
// Then open /hooks (or restart Claude Code) to load them.
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const DIR = path.join(ROOT, '.claude');
const PROJ = path.join(DIR, 'settings.json');
const LOCAL = path.join(DIR, 'settings.local.json');

// The 4 hooks, keyed by event. Each entry is one matcher-group.
const HOOKS = {
  PostToolUse: [{
    matcher: 'Write|Edit',
    hooks: [{ type: 'command', command: 'node scripts/hook-theme-sync.js', timeout: 30 }],
  }],
  PreToolUse: [{
    matcher: 'Bash',
    hooks: [{ type: 'command', command: 'node scripts/hook-push-guard.js', if: 'Bash(git push*)', timeout: 15 }],
  }],
  SessionStart: [{
    hooks: [{ type: 'command', command: 'just clean-worktrees', timeout: 120, statusMessage: 'Cleaning stale worktrees' }],
  }],
  PreCompact: [{
    hooks: [{ type: 'command', command: 'node scripts/precompact-reminder.js', timeout: 10, statusMessage: 'Flushing durable state before compaction' }],
  }],
};

function readJson(p) {
  if (!fs.existsSync(p)) return {};
  try { return JSON.parse(fs.readFileSync(p, 'utf8')); } catch (e) { console.error('Could not parse ' + p + ': ' + e.message); process.exit(1); }
}
function backup(p) {
  if (fs.existsSync(p)) fs.copyFileSync(p, p + '.bak-' + Date.now());
}
function commandsIn(eventArr) {
  const out = new Set();
  for (const grp of eventArr || []) for (const h of grp.hooks || []) if (h.command) out.add(h.command);
  return out;
}

fs.mkdirSync(DIR, { recursive: true });

// 1) Merge hooks into settings.json (idempotent by command string).
const proj = readJson(PROJ);
proj.$schema = proj.$schema || 'https://json.schemastore.org/claude-code-settings.json';
proj.hooks = proj.hooks || {};
let added = 0;
for (const [event, groups] of Object.entries(HOOKS)) {
  proj.hooks[event] = proj.hooks[event] || [];
  const existing = commandsIn(proj.hooks[event]);
  for (const grp of groups) {
    const cmd = grp.hooks[0].command;
    if (existing.has(cmd)) { console.log('  = ' + event + ': already present (' + cmd + ')'); continue; }
    proj.hooks[event].push(grp);
    added++;
    console.log('  + ' + event + ': ' + cmd);
  }
}
backup(PROJ);
fs.writeFileSync(PROJ, JSON.stringify(proj, null, 2) + '\n');

// 2) Turn off disableAllHooks in settings.local.json (preserve everything else).
const local = readJson(LOCAL);
let flipped = false;
if (local.disableAllHooks === true) { delete local.disableAllHooks; flipped = true; }
backup(LOCAL);
fs.writeFileSync(LOCAL, JSON.stringify(local, null, 2) + '\n');

console.log('');
console.log('Done. ' + added + ' hook(s) added to .claude/settings.json' +
  (flipped ? '; removed disableAllHooks from settings.local.json so hooks can fire.' : '; disableAllHooks was already off.'));
console.log('Backups written alongside each file (*.bak-<timestamp>).');
console.log('Next: open /hooks in Claude Code (or restart) to load them.');
