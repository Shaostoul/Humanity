#!/usr/bin/env node
// clean-worktrees: remove agent worktrees whose work has landed on main, and nothing else.
//
//   just clean-worktrees --dry-run     print what would be removed and why; change nothing
//   just clean-worktrees               remove them (the recipe passes --yes)
//   node scripts/clean-worktrees.js    ask before removing
//
// OPERATOR / ORCHESTRATOR ONLY. Never run it from a dispatched subagent's own task: it cannot
// know which sibling worktree another session is about to use (CLAUDE.md, START HERE).
//
// HISTORY, each line a failure this script must never repeat:
// - 2026-07-01, three times in one day: the old script removed every worktree once --yes was
//   passed, with no content check. Subagents told to "read CLAUDE.md first" ran it as their own
//   first step and force-deleted sibling worktrees mid-review: commits, branches and all. Since
//   then a worktree with real work survives unless --force-unmerged is passed.
// - 2026-10-22: a hand cleanup found the script would have removed a running agent's freshly
//   made, still-clean worktree. Claude Code now LOCKS a running agent's worktree ("locked claude
//   agent <name> (pid N)"); a locked worktree is never removed, not even by --force-unmerged, and
//   one whose git state changed in the last --min-idle-hours (default 2) is kept as well.
// - 2026-11-16: it removed 2 of 211 worktrees. Every agent branch is SQUASH-merged, so its
//   commits never appear in main's history, and the old ancestor test called each one unmerged.
//   Landing is now decided by scripts/lib/landed.js: main's history, then the ledger of landed
//   branches (data/coordination/merged-branches.jsonl, written by `just record-merge`), then
//   the content itself (main holds every change the branch makes). Old worktrees also showed the
//   once-tracked .claude/settings.local.json as modified; that one file no longer counts as work.
//
// WHAT IS REMOVED: a worktree that is not the main checkout (nor the one you run from, unless
// --all), not locked, idle for --min-idle-hours, with no uncommitted change except
// .claude/settings.local.json, and whose commits have landed. ITS BRANCH IS KEPT, so every
// commit stays reachable; only orphan branches that main's own history already contains are
// deleted (claude/* and worktree-agent-*, as before).
// WHAT IS KEPT, always: the main checkout; a locked worktree; one that git cannot read; one
// with any other uncommitted or untracked file; a recently active one; one with commits main
// does not hold by content; a detached HEAD whose commits are on no branch. Each is printed
// with its reason. --force-unmerged also removes the uncommitted and unlanded ones (it DESTROYS
// that work); it still never touches a locked or recently active worktree.
//
// Every git call is read-only until the removal step (GIT_OPTIONAL_LOCKS=0; the content proof
// writes its scratch trees to a temp folder), and each worktree is re-checked just before it is
// removed, in case an agent started using it after the plan was made.
//
// Tested by scripts/tests/clean-worktrees.test.js on throwaway repositories.
'use strict';

const fs = require('fs');
const path = require('path');
const readline = require('readline');
const L = require('./lib/landed');

const HELP = `Usage: node scripts/clean-worktrees.js [--dry-run] [--yes] [--all] [--force-unmerged]
                                        [--min-idle-hours N] [--main <ref>] [--candidates N]
  --dry-run          print the plan with a reason per worktree; change nothing
  --yes              do not ask before removing (the just recipe passes it)
  --all              also consider the worktree you run this from
  --force-unmerged   ALSO remove worktrees with uncommitted work or unlanded commits (destroys it)
  --min-idle-hours N keep a worktree whose git state changed in the last N hours (default 2)
  --main <ref>       the branch work lands on (default main)
  --candidates N     earlier main commits the content proof tries per branch (default ${L.DEFAULT_CANDIDATES})
  --from-worktree    allow removing from inside a worktree (normally only the main checkout may)`;

function parseArgs(argv) {
  const o = { dryRun: false, yes: false, all: false, force: false, minIdleHours: 2, main: 'main', candidates: L.DEFAULT_CANDIDATES, help: false, fromWorktree: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--dry-run' || a === '-n') o.dryRun = true;
    else if (a === '--from-worktree') o.fromWorktree = true;
    else if (a === '--yes' || a === '-y') o.yes = true;
    else if (a === '--all') o.all = true;
    else if (a === '--force-unmerged') o.force = true;
    else if (a === '--min-idle-hours') o.minIdleHours = Number(argv[++i]);
    else if (a === '--main') o.main = argv[++i];
    else if (a === '--candidates') o.candidates = Number(argv[++i]);
    else if (a === '--help' || a === '-h') o.help = true;
    else throw new Error(`unknown argument: ${a}\n${HELP}`);
  }
  if (!Number.isFinite(o.minIdleHours) || o.minIdleHours < 0) throw new Error('--min-idle-hours needs a number of hours, 0 or more');
  if (!Number.isInteger(o.candidates) || o.candidates < 0) throw new Error('--candidates needs a whole number, 0 or more');
  return o;
}

/** Newest change to a worktree's git state: its admin HEAD, index and reflog, and its folder. */
function lastActivityMs(wtPath) {
  const times = [];
  const stat = (p) => {
    try { times.push(fs.statSync(p).mtimeMs); } catch (_) { /* missing is fine */ }
  };
  stat(wtPath);
  const r = L.git(wtPath, ['rev-parse', '--absolute-git-dir']);
  if (r.status === 0) {
    const admin = r.stdout.trim();
    stat(path.join(admin, 'HEAD'));
    stat(path.join(admin, 'index'));
    stat(path.join(admin, 'logs', 'HEAD'));
  }
  return times.length ? Math.max(...times) : 0;
}

/** Is the process a Claude Code lock names still running? (Advice only: a pid can be reused.) */
function lockPidAlive(reason) {
  const m = /pid (\d+)/.exec(reason || '');
  if (!m) return null;
  try { process.kill(Number(m[1]), 0); return true; } catch (e) { return e.code === 'EPERM'; }
}

function short(sha) { return sha ? sha.slice(0, 9) : '?'; }

const HOW_LABEL = {
  history: 'in main\'s history',
  ledger: 'by the ledger',
  'content-at-commit': 'by content at the squash commit',
  'content-at-tip': 'by content at main\'s tip',
  'content-resolved': 'by content, conflicts resolved',
};

function describeCommit(cwd, sha) {
  const r = L.git(cwd, ['log', '-1', '--format=%ad %s', '--date=short', sha]);
  const s = r.stdout.trim();
  return s.length > 90 ? s.slice(0, 87) + '...' : s;
}

function landedReason(cwd, v) {
  switch (v.how) {
    case 'history': return 'landed: every commit is already in main\'s history';
    case 'ledger': return `landed: the ledger records it as main commit ${short(v.at)} (${v.entry.date || 'no date'}, ${v.entry.how || 'recorded'})`;
    case 'content-at-commit': return `landed: main commit ${short(v.at)} holds every change it makes (${describeCommit(cwd, v.at)})`;
    case 'content-at-tip': return 'landed: main already holds every change it makes';
    case 'content-resolved': return `landed: main commit ${short(v.at)} holds every line it added and none it removed; a re-merge conflicts only where that commit was resolved by hand (${describeCommit(cwd, v.at)})`;
    default: return 'landed';
  }
}

function listFew(entries) {
  const names = entries.slice(0, 3).map((e) => `${e.xy.trim() || '?'} ${e.file}`);
  return names.join(', ') + (entries.length > 3 ? `, and ${entries.length - 3} more` : '');
}

/**
 * Decide one worktree. Returns { action: 'remove' | 'keep' | 'prune', reason, destroys }.
 * Pure reading: nothing here changes the repository.
 */
function judge(ctx, wt) {
  const { main, opts, ledger, objects, now } = ctx;
  if (wt.bare) return { action: 'keep', reason: 'a bare repository entry' };
  if (wt.prunable || !fs.existsSync(wt.path)) {
    return { action: 'prune', reason: 'its folder is gone; `git worktree prune` forgets the entry (nothing on disk to lose)' };
  }
  if (wt.locked) {
    const alive = lockPidAlive(wt.lockReason);
    const note = alive === false ? '; that process is no longer running, so the lock may be stale: `git worktree unlock <path>` once you know the agent is done' : '';
    return { action: 'keep', reason: `locked (${wt.lockReason || 'no reason given'}): an agent is using it${note}` };
  }
  const st = L.uncommitted(wt.path);
  if (!st.ok) return { action: 'keep', reason: `git cannot read its state: ${st.error.split('\n')[0]}` };

  const idleH = (now - lastActivityMs(wt.path)) / 3600000;
  if (idleH < opts.minIdleHours) {
    return { action: 'keep', reason: `its git state changed ${Math.max(0, idleH * 60).toFixed(0)} min ago, so it may be in use (--min-idle-hours ${opts.minIdleHours}; pass 0 to ignore)` };
  }
  if (st.real.length) {
    const r = `${st.real.length} uncommitted change(s): ${listFew(st.real)}`;
    if (opts.force) return { action: 'remove', reason: r, destroys: true };
    return { action: 'keep', reason: r };
  }
  if (!wt.head) return { action: 'keep', reason: 'no HEAD commit to judge' };

  const v = L.landed(main, { branch: wt.branch, tip: wt.head, mainRef: opts.main, ledger, objects, candidates: opts.candidates });
  if (wt.detached && v.landed && v.how !== 'history') {
    const holders = L.git(main, ['for-each-ref', '--contains', wt.head, '--format=%(refname:short)', 'refs/heads']).stdout.trim();
    if (!holders) {
      const r = `detached HEAD ${short(wt.head)} whose commits are on no branch (its work is in main by content, but removing it would orphan the commits): give it a branch first, \`git -C "${wt.path}" switch -c rescue/${path.basename(wt.path)}\``;
      if (opts.force) return { action: 'remove', reason: r, destroys: true };
      return { action: 'keep', reason: r };
    }
  }
  const settingsNote = st.benign.length ? '; its only uncommitted change is .claude/settings.local.json (Claude Code\'s permission file)' : '';
  if (v.landed) return { action: 'remove', reason: landedReason(main, v) + settingsNote, how: v.how };
  const r = `not landed: ${v.reason}`;
  if (opts.force) return { action: 'remove', reason: r, destroys: true };
  return { action: 'keep', reason: r };
}

/** Folders in .claude/worktrees that git does not list as worktrees. */
function orphanFolders(main, wts) {
  const dir = path.join(main, '.claude', 'worktrees');
  if (!fs.existsSync(dir)) return [];
  const out = [];
  for (const name of fs.readdirSync(dir)) {
    const p = path.join(dir, name);
    let st;
    try { st = fs.statSync(p); } catch (_) { continue; }
    if (!st.isDirectory()) continue;
    if (wts.some((w) => L.samePath(w.path, p))) continue;
    out.push(p);
  }
  return out;
}

function judgeOrphanFolder(ctx, p) {
  const { opts, now } = ctx;
  let mtime = 0;
  try { mtime = fs.statSync(p).mtimeMs; } catch (_) { /* gone */ }
  if ((now - mtime) / 3600000 < opts.minIdleHours) {
    return { action: 'keep', reason: 'changed recently; it may be a worktree being made right now' };
  }
  if (!fs.existsSync(path.join(p, '.git'))) {
    return { action: 'remove', reason: 'not a worktree and has no .git: the remains of a removal that could not finish (no commits to lose)' };
  }
  const st = L.uncommitted(p);
  if (!st.ok) return { action: 'keep', reason: `has a .git but git cannot read it (${st.error.split('\n')[0]}): look inside before deleting it by hand` };
  if (st.real.length) {
    const r = `${st.real.length} uncommitted change(s): ${listFew(st.real)}`;
    return opts.force ? { action: 'remove', reason: r, destroys: true } : { action: 'keep', reason: r };
  }
  const head = L.revParse(p, 'HEAD');
  const anc = head ? L.isAncestor(ctx.main, head, opts.main) : null;
  if (anc === true) return { action: 'remove', reason: 'clean, and its HEAD is in main\'s history' };
  const r = `its HEAD ${short(head)} is not in main's history`;
  return opts.force ? { action: 'remove', reason: r, destroys: true } : { action: 'keep', reason: r };
}

/** claude/* and worktree-agent-* branches with no worktree that main's history contains. */
function mergedOrphanBranches(main, mainRef, wts, current) {
  const out = [];
  const checkedOut = new Set(wts.map((w) => w.branch).filter(Boolean));
  const list = (args) => {
    const r = L.git(main, ['for-each-ref', '--format=%(refname:short)', ...args, 'refs/heads']);
    if (r.status !== 0) throw new Error(`git for-each-ref failed: ${r.stderr}`);
    return r.stdout.split(/\r?\n/).filter(Boolean);
  };
  // One call for "every branch whose tip main's history contains", not one per branch.
  const merged = new Set(list([`--merged=${mainRef}`]));
  let kept = 0;
  for (const b of list([])) {
    if (!/^(claude\/|worktree-agent-)/.test(b) || b === current || checkedOut.has(b)) continue;
    if (merged.has(b)) out.push(b);
    else kept++;
  }
  return { remove: out, kept };
}

function ask(question) {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((resolve) => rl.question(question, (a) => { rl.close(); resolve(a); }));
}

/** Build the whole plan without changing anything. */
function plan(opts, cwd = process.cwd()) {
  const main = L.mainWorktree(cwd);
  const current = L.git(cwd, ['rev-parse', '--show-toplevel']).stdout.trim();
  const currentBranch = L.git(cwd, ['rev-parse', '--abbrev-ref', 'HEAD']).stdout.trim();
  if (!L.revParse(main, opts.main)) throw new Error(`cannot read the branch ${opts.main} in ${main}`);
  const ledgerFile = path.join(main, L.LEDGER_REL);
  const { entries: ledger, bad } = L.readLedger(ledgerFile);
  const wts = L.listWorktrees(main);
  const objects = L.scratchObjects(main);
  const ctx = { main, opts, ledger, objects, now: Date.now() };
  const rows = [];
  // A first run proves most branches by content, seconds each: show it is working (terminal only).
  const progress = process.stderr.isTTY && !opts.quiet
    ? (i, n, name) => process.stderr.write(`\r  judging ${i}/${n} ${name}`.padEnd(70).slice(0, 79) + (i === n ? '\n' : ''))
    : () => {};
  try {
    const others = wts.slice(1);
    for (const [i, wt] of others.entries()) {
      progress(i + 1, others.length, path.basename(wt.path));
      if (!opts.all && current && L.samePath(wt.path, current)) {
        rows.push({ wt, action: 'keep', reason: 'the worktree this was run from (--all to include it)' });
        continue;
      }
      rows.push({ wt, ...judge(ctx, wt) });
    }
    const folders = orphanFolders(main, wts).map((p) => ({ path: p, ...judgeOrphanFolder(ctx, p) }));
    // A branch is an orphan once its worktree goes, so judge against the worktrees that stay.
    const going = new Set(rows.filter((r) => r.action !== 'keep').map((r) => r.wt.path));
    const staying = wts.filter((w) => !going.has(w.path));
    const branches = mergedOrphanBranches(main, opts.main, staying, currentBranch);
    return { main, ledgerFile, ledgerBad: bad, rows, folders, branches };
  } finally {
    objects.dispose();
  }
}

function printPlan(p, opts, log = console.log) {
  const remove = p.rows.filter((r) => r.action === 'remove');
  const keep = p.rows.filter((r) => r.action === 'keep');
  const prune = p.rows.filter((r) => r.action === 'prune');
  const name = (r) => `${path.basename(r.wt.path)}${r.wt.branch ? ' [' + r.wt.branch + ']' : r.wt.detached ? ' [detached]' : ''}`;
  if (p.ledgerBad.length) log(`WARNING: ${p.ledgerFile} has unreadable line(s) ${p.ledgerBad.join(', ')}; they were ignored.`);
  log(`KEEP ${keep.length} worktree(s):`);
  for (const r of keep) log(`  - ${name(r)}: ${r.reason}`);
  log('');
  log(`${opts.dryRun ? 'WOULD REMOVE' : 'REMOVE'} ${remove.length} worktree(s) (their branches are kept):`);
  for (const r of remove) log(`  - ${name(r)}: ${r.destroys ? 'DESTROYS WORK (--force-unmerged): ' : ''}${r.reason}`);
  if (prune.length) {
    log('');
    log(`${opts.dryRun ? 'WOULD PRUNE' : 'PRUNE'} ${prune.length} entry(ies) whose folder is gone:`);
    for (const r of prune) log(`  - ${name(r)}`);
  }
  const fRemove = p.folders.filter((f) => f.action === 'remove');
  const fKeep = p.folders.filter((f) => f.action === 'keep');
  if (p.folders.length) {
    log('');
    log(`Folders in .claude/worktrees that git does not list: ${fRemove.length} to remove, ${fKeep.length} kept`);
    for (const f of p.folders) log(`  - ${path.basename(f.path)}: ${f.action === 'remove' ? (opts.dryRun ? 'would remove' : 'remove') : 'keep'}: ${f.destroys ? 'DESTROYS WORK: ' : ''}${f.reason}`);
  }
  log('');
  log(`Branches with no worktree: ${p.branches.remove.length} already in main's history ${opts.dryRun ? 'would be' : 'will be'} deleted; ` +
    `${p.branches.kept} others are kept (a squash-landed branch keeps its commits reachable).`);
  for (const b of p.branches.remove) log(`  - ${b}`);
  const byHow = {};
  for (const r of remove) {
    const k = r.destroys ? 'forced, destroying work' : HOW_LABEL[r.how] || 'landed';
    byHow[k] = (byHow[k] || 0) + 1;
  }
  log('');
  log(`Summary: ${remove.length} to remove (${Object.entries(byHow).map(([k, n]) => `${n} ${k}`).join(', ') || 'none'}), ${keep.length} kept, ${prune.length} to prune.`);
}

async function main() {
  let opts;
  try { opts = parseArgs(process.argv.slice(2)); } catch (e) { console.error(e.message); process.exit(2); }
  if (opts.help) { console.log(HELP); return; }
  // The 2026-07-01 failure, made mechanical: a dispatched agent works inside a worktree, and
  // "never run this from a subagent" was a doc rule that failed twice before the script was
  // hardened. Removing runs only from the main checkout; looking (--dry-run) runs anywhere.
  if (!opts.dryRun && !opts.fromWorktree) {
    const main = L.mainWorktree(process.cwd());
    const here = L.git(process.cwd(), ['rev-parse', '--show-toplevel']).stdout.trim();
    if (!here || !L.samePath(here, main)) {
      console.error(`clean-worktrees: refused. This removes SIBLING worktrees, so it runs only from the main checkout (${main}), ` +
        'by the operator or the orchestrator once every agent of the session is merged or abandoned. ' +
        `You are in ${here || 'no checkout'}: if you are a dispatched agent, this is not your step. ` +
        'To only look: just clean-worktrees --dry-run. (An orchestrator working from a worktree: --from-worktree.)');
      process.exit(2);
    }
  }
  const p = plan(opts);
  printPlan(p, opts);
  if (opts.dryRun) { console.log('\nDry run: nothing was changed.'); return; }

  const remove = p.rows.filter((r) => r.action === 'remove');
  const prune = p.rows.filter((r) => r.action === 'prune');
  const folders = p.folders.filter((f) => f.action === 'remove');
  if (!remove.length && !prune.length && !folders.length && !p.branches.remove.length) {
    console.log('\nNothing to remove.');
    return;
  }
  if (!opts.yes) {
    if (!process.stdin.isTTY) {
      console.error('\nNot a terminal, so nothing was removed: pass --yes to remove, or --dry-run to only look.');
      process.exit(2);
    }
    const a = await ask('\nRemove these? (y/N) ');
    if (!/^y(es)?$/i.test(a.trim())) { console.log('Aborted: nothing was changed.'); return; }
  }

  console.log('\nRemoving...');
  let removed = 0;
  const failed = [];
  for (const r of remove) {
    // Re-check right before removing: an agent may have locked or edited it since the plan.
    const fresh = L.listWorktrees(p.main).find((w) => L.samePath(w.path, r.wt.path));
    if (!fresh) continue;
    if (fresh.locked) { console.log(`  kept ${path.basename(r.wt.path)}: it was locked after the plan was made`); continue; }
    if (!r.destroys) {
      const st = L.uncommitted(r.wt.path);
      if (!st.ok || st.real.length || fresh.head !== r.wt.head) {
        console.log(`  kept ${path.basename(r.wt.path)}: it changed after the plan was made`);
        continue;
      }
    }
    // One --force: removes the settings-file change and ignored build output, and git itself
    // still refuses a locked worktree.
    const g = L.git(p.main, ['worktree', 'remove', '--force', r.wt.path]);
    if (g.status === 0) removed++;
    else failed.push(`${path.basename(r.wt.path)}: ${g.stderr.split('\n')[0]}`);
  }
  L.git(p.main, ['worktree', 'prune']);
  for (const f of folders) {
    try { fs.rmSync(f.path, { recursive: true, force: true }); } catch (e) { failed.push(`${path.basename(f.path)}: ${e.message}`); }
  }
  for (const b of p.branches.remove) {
    const g = L.git(p.main, ['branch', '-D', b]);
    if (g.status !== 0) failed.push(`branch ${b}: ${g.stderr.split('\n')[0]}`);
  }
  console.log(`\nRemoved ${removed} worktree(s), pruned ${prune.length}, deleted ${p.branches.remove.length} merged branch(es) and ${folders.length} leftover folder(s).`);
  if (failed.length) {
    console.log(`\n${failed.length} could not be removed (often a file held open by a running program; close it and run again):`);
    for (const f of failed) console.log(`  - ${f}`);
  }
}

if (require.main === module) {
  main().catch((e) => { console.error(`clean-worktrees: ${e.message}`); process.exit(1); });
}

module.exports = { parseArgs, plan, printPlan, judge };
