// HAS THIS BRANCH LANDED ON MAIN? (2026-11-20, the squash-aware cleanup)
//
// Every agent branch is SQUASH-merged (the merge-agent skill: `git merge --squash`, then one
// commit), so none of its own commits ever appears in main's history. The old cleanup asked only
// "is the branch tip an ancestor of main?", got "no" for every one of them, and on 2026-11-16
// removed 2 of 211 worktrees, keeping the rest as "real work". This module answers the question
// the cleanup actually needs, three ways, cheapest first:
//
//   1. history   the tip is an ancestor of main (a fast-forward or a real merge).
//   2. ledger    data/coordination/merged-branches.jsonl, one line per landed branch, appended
//                by the merge step (`just record-merge <branch>`, scripts/merged-branches.js)
//                and backfilled from history. A line counts only while the branch tip is still
//                the commit that was merged (or behind it): commits made AFTER the merge are
//                new work and fall through to the content proof.
//   3. content   main already holds every change the branch made. Three tests, (a) and (b)
//                exact, (c) a line-by-line reading of the regions a hand resolution touched:
//                  a. a three-way merge of the branch into main (`git merge-tree --write-tree`)
//                     is clean and its result is main's own tree: the branch adds nothing.
//                  b. the same merge into an earlier main commit M (the squash commit, found
//                     among the commits after the fork that touch the branch's files, best
//                     file overlap first) is clean and equals M's tree: the branch's work
//                     landed at M, and whatever main did to those lines later was main's choice.
//                  c. that merge CONFLICTS because the squash itself was resolved by hand (main
//                     had changed the same lines meanwhile), and in every conflict region M has
//                     each line the branch added and none it removed, while everything outside
//                     the regions is M byte for byte (resolvedLanding). Reported separately, as
//                     "landed by content, conflicts resolved", so it can be told apart.
//                A branch whose changes main later REVERTED is not proven by (a) and may be by
//                (b); either way the branch itself is kept, so its commits stay reachable.
//
// WHY NOT THE COMMIT MESSAGES: the squash commits do not name their branch (checked 2026-11-20:
// none of main's last 3,000 commit messages contains "worktree-agent-"), so a backfill from the
// messages alone finds nothing. Content is the evidence that exists.
//
// READ-ONLY ON THE REPOSITORY: `merge-tree --write-tree` must write the trees it builds. They go
// to a throwaway object folder in the OS temp folder (GIT_OBJECT_DIRECTORY, with the real store
// as a read-only alternate), deleted afterwards, so a dry run writes nothing into .git/objects.
// Every git call also runs with GIT_OPTIONAL_LOCKS=0, so `git status` on a sibling worktree never
// takes that worktree's index.lock while an agent is working in it.
//
// Tested by scripts/tests/clean-worktrees.test.js on throwaway repositories.
'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

/** Where the ledger lives, relative to the MAIN checkout (never a worktree's copy). */
const LEDGER_REL = path.join('data', 'coordination', 'merged-branches.jsonl');

/**
 * The only uncommitted change a landed worktree may carry and still be removed: Claude Code's
 * per-checkout permission file, tracked until a047d3d05 (2026-11-16) and ignored since, so every
 * worktree made before then shows it as modified. Its content is that agent's approved commands,
 * nothing anyone wrote. Anything else uncommitted is real work and keeps the worktree.
 */
const ALLOWED_DIRT = ['.claude/settings.local.json'];

/** How many earlier main commits the content proof tries as the squash commit, per branch. */
const DEFAULT_CANDIDATES = 8;

/**
 * Variables that point git at a repository other than the folder it runs in. Inside a git hook
 * (a pre-commit running tests) GIT_DIR and GIT_INDEX_FILE are set, and `git status` in a
 * sibling worktree would then read THIS checkout's index and could call a dirty worktree clean.
 * Every call here names its folder, so they are dropped.
 */
const REPO_POINTERS = ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_COMMON_DIR', 'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_PREFIX'];

function cleanEnv(extra) {
  const env = { ...process.env };
  for (const k of Object.keys(env)) if (REPO_POINTERS.includes(k.toUpperCase())) delete env[k];
  return { ...env, GIT_OPTIONAL_LOCKS: '0', ...(extra || {}) };
}

function git(cwd, args, env) {
  const r = spawnSync('git', args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 512 * 1024 * 1024,
    windowsHide: true,
    env: cleanEnv(env),
  });
  return { status: r.status, stdout: r.stdout || '', stderr: (r.stderr || '').trim(), error: r.error };
}

function gitOk(cwd, args, env) {
  const r = git(cwd, args, env);
  if (r.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed in ${cwd}: ${r.stderr || (r.error && r.error.message) || 'exit ' + r.status}`);
  }
  return r.stdout.trim();
}

/** Two spellings of one folder compare equal (Windows: case and slashes). */
function samePath(a, b) {
  const norm = (p) => {
    let s = path.resolve(p).replace(/\\/g, '/').replace(/\/+$/, '');
    if (process.platform === 'win32') s = s.toLowerCase();
    return s;
  };
  return norm(a) === norm(b);
}

/** `git worktree list --porcelain`, parsed. The first entry is always the main checkout. */
function listWorktrees(cwd) {
  const out = gitOk(cwd, ['worktree', 'list', '--porcelain']);
  const wts = [];
  let cur = null;
  for (const line of out.split(/\r?\n/)) {
    if (line.startsWith('worktree ')) {
      cur = { path: line.slice(9), head: null, branch: null, detached: false, bare: false, locked: false, lockReason: '', prunable: false };
      wts.push(cur);
    } else if (!cur) {
      continue;
    } else if (line.startsWith('HEAD ')) {
      cur.head = line.slice(5);
    } else if (line.startsWith('branch ')) {
      cur.branch = line.slice(7).replace(/^refs\/heads\//, '');
    } else if (line === 'detached') {
      cur.detached = true;
    } else if (line === 'bare') {
      cur.bare = true;
    } else if (line === 'locked' || line.startsWith('locked ')) {
      cur.locked = true;
      cur.lockReason = line.slice(6).trim();
    } else if (line === 'prunable' || line.startsWith('prunable ')) {
      cur.prunable = true;
    }
  }
  return wts;
}

/** The main checkout's folder, from any worktree of the repository. */
function mainWorktree(cwd) {
  const wts = listWorktrees(cwd);
  if (!wts.length) throw new Error('git worktree list returned nothing');
  return wts[0].path;
}

/**
 * Uncommitted state of one worktree, split into the allowed settings file and everything else.
 * { ok, benign: [{xy, file}], real: [{xy, file}], error }
 */
function uncommitted(wtPath) {
  const r = git(wtPath, ['status', '--porcelain=v1', '-z']);
  if (r.status !== 0) return { ok: false, benign: [], real: [], error: r.stderr || 'git status failed' };
  const entries = [];
  const parts = r.stdout.split('\0');
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i];
    if (!p) continue;
    const xy = p.slice(0, 2);
    entries.push({ xy, file: p.slice(3) });
    if (xy[0] === 'R' || xy[0] === 'C') i++; // the rename's source path follows
  }
  const allowed = (e) => ALLOWED_DIRT.includes(e.file);
  return { ok: true, benign: entries.filter(allowed), real: entries.filter((e) => !allowed(e)), error: '' };
}

function revParse(cwd, ref) {
  const r = git(cwd, ['rev-parse', '--verify', '--quiet', ref + '^{commit}']);
  return r.status === 0 ? r.stdout.trim() : null;
}

/** true / false, or null when git could not say (a missing object). */
function isAncestor(cwd, a, b) {
  const r = git(cwd, ['merge-base', '--is-ancestor', a, b]);
  if (r.status === 0) return true;
  if (r.status === 1) return false;
  return null;
}

/**
 * A throwaway object store for `merge-tree --write-tree`, so the proofs write nothing into the
 * repository. Call dispose() when done.
 */
function scratchObjects(cwd) {
  const common = gitOk(cwd, ['rev-parse', '--path-format=absolute', '--git-common-dir']);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hum_cleanwt_objects_'));
  return {
    env: { GIT_OBJECT_DIRECTORY: dir, GIT_ALTERNATE_OBJECT_DIRECTORIES: path.join(common, 'objects') },
    dispose: () => fs.rmSync(dir, { recursive: true, force: true }),
  };
}

/**
 * Three-way merge of `theirs` into `ours`, in memory. { clean, tree } or { error }. Conflicts are
 * written diff3-style (ours, ||||||| base, theirs) so resolvedLanding() can read all three.
 */
function mergeTree(cwd, ours, theirs, env) {
  const r = git(cwd, ['-c', 'merge.conflictStyle=diff3', 'merge-tree', '--write-tree', '--no-messages', ours, theirs], env);
  if (r.status === 0) return { clean: true, tree: r.stdout.split(/\r?\n/)[0].trim() };
  if (r.status === 1) return { clean: false, tree: r.stdout.split(/\r?\n/)[0].trim() };
  return { clean: false, tree: null, error: r.stderr || 'git merge-tree failed' };
}

function blobLines(cwd, spec, env) {
  const r = git(cwd, ['cat-file', 'blob', spec], env);
  return r.status === 0 ? r.stdout.split('\n') : null;
}

/**
 * One conflict region: did `ours` (the main commit) take the branch's change? Every line the
 * branch added (theirs minus base, counted) must be in ours at least that many times, and no line
 * the branch removed (base minus theirs) may be in ours more often than in theirs. Blank lines are
 * not counted: a resolution that spaces paragraphs differently has not dropped any work.
 */
function regionLands(ours, base, theirs) {
  const count = (lines) => {
    const m = new Map();
    for (const raw of lines) {
      const l = raw.replace(/\r$/, '');
      if (l.trim() === '') continue;
      m.set(l, (m.get(l) || 0) + 1);
    }
    return m;
  };
  const O = count(ours);
  const B = count(base);
  const T = count(theirs);
  for (const [l, t] of T) {
    const added = t - (B.get(l) || 0);
    if (added > 0 && (O.get(l) || 0) < added) return false;
  }
  for (const [l, b] of B) {
    const removed = b - (T.get(l) || 0);
    if (removed > 0 && (O.get(l) || 0) > (T.get(l) || 0)) return false;
  }
  return true;
}

/**
 * A merge of the branch into main commit `commit` CONFLICTED: was it because the squash commit
 * itself was a hand-resolved merge (main had changed the same lines meanwhile: the usual case for
 * PRIORITIES.md, BUGS.md and the journal) that kept all of the branch's lines? Proven when every
 * file where the merge result differs from `commit` is a text file whose conflict regions all
 * land (regionLands) AND which, with each region replaced by its "ours" side, is byte for byte
 * `commit`'s file: the clean parts of the merge add nothing either. Anything else (a file only
 * the branch has, a modify/delete or binary conflict, a clean change main lacks, unreadable
 * markers) is not proven. Measured 2026-11-20 on the real worktrees: of the 50 that (a) and (b)
 * could not prove, this proved 17; the rest had their lines adapted while being merged.
 */
function resolvedLanding(cwd, commit, resultTree, env) {
  const paths = nameList(cwd, ['diff', '--no-renames', '--name-only', '-z', commit, resultTree], env);
  if (!paths) return { ok: false, file: '(git diff failed)' };
  for (const p of paths) {
    const merged = blobLines(cwd, `${resultTree}:${p}`, env);
    const mine = blobLines(cwd, `${commit}:${p}`, env);
    if (!merged || !mine) return { ok: false, file: p };
    const rebuilt = [];
    let state = 'out';
    let regions = 0;
    let o = [];
    let b = [];
    let t = [];
    for (const raw of merged) {
      const l = raw.replace(/\r$/, '');
      if (state === 'out') {
        if (l.startsWith('<<<<<<< ')) { state = 'ours'; o = []; b = []; t = []; regions++; } else rebuilt.push(raw);
      } else if (state === 'ours') {
        if (l.startsWith('||||||| ')) state = 'base';
        else if (l.startsWith('<<<<<<<') || l === '=======') return { ok: false, file: p };
        else o.push(raw);
      } else if (state === 'base') {
        if (l === '=======') state = 'theirs'; else b.push(raw);
      } else if (state === 'theirs') {
        if (l.startsWith('>>>>>>> ')) {
          if (!regionLands(o, b, t)) return { ok: false, file: p };
          rebuilt.push(...o);
          state = 'out';
        } else if (l.startsWith('<<<<<<<')) {
          return { ok: false, file: p };
        } else {
          t.push(raw);
        }
      }
    }
    if (state !== 'out' || regions === 0) return { ok: false, file: p };
    if (rebuilt.join('\n') !== mine.join('\n')) return { ok: false, file: p };
  }
  return { ok: true };
}

function treeOf(cwd, commit) {
  return gitOk(cwd, ['rev-parse', commit + '^{tree}']);
}

/**
 * NUL-separated names from git, or null when git failed. Never an empty list on failure: an
 * error read as "no differences" would PROVE a landing. (Found 2026-11-20 by the dropped-merge
 * test: the diff of a scratch tree ran without the scratch store, failed, and the empty answer
 * called a branch whose line the merger had thrown away landed.)
 */
function nameList(cwd, args, env) {
  const r = git(cwd, args, env);
  if (r.status !== 0) return null;
  return r.stdout.split('\0').filter(Boolean);
}

/**
 * Does main already hold every change `ref` made? Tries the likeliest earlier main commits (the
 * squash commit among them), then main's tip, exactly; then the conflicted ones by their regions.
 * Returns
 *   { landed: true, how: 'content-at-tip' | 'content-at-commit' | 'content-resolved', at: <main commit> }
 *   { landed: false, reason: '...' }
 */
function landedByContent(cwd, ref, mainRef, opts = {}) {
  const tip = revParse(cwd, ref);
  const mainTip = revParse(cwd, mainRef);
  if (!tip) return { landed: false, reason: `cannot read ${ref}` };
  if (!mainTip) return { landed: false, reason: `cannot read ${mainRef}` };
  const own = opts.objects ? null : scratchObjects(cwd);
  const env = (opts.objects || own).env;
  try {
    const base = git(cwd, ['merge-base', mainTip, tip]).stdout.trim();
    if (!base) return { landed: false, reason: 'shares no history with main' };
    const changed = nameList(cwd, ['diff', '--no-renames', '--name-only', '-z', base, tip]);
    if (!changed) return { landed: false, reason: 'git could not list the files it changes' };
    const files = new Set(changed);
    if (files.size === 0) return { landed: true, how: 'content-at-tip', at: mainTip };

    // The earlier main commits first: a merge close to the fork is cheap (about 0.1 s measured
    // 2026-11-20), while a merge into main's tip, hundreds of commits on, took 2 s, and the
    // squash commit is nearly always one of the first few tried. Main's own commits since the
    // fork, with the files each touched (merges skipped: a squash commit is an ordinary commit).
    const logRun = git(cwd, ['log', '--no-merges', '--no-renames', '--name-only', '-z', '--format=%x01%H', `${base}..${mainTip}`]);
    if (logRun.status !== 0) return { landed: false, reason: `git log failed: ${logRun.stderr.split('\n')[0]}` };
    const log = logRun.stdout;
    const commits = [];
    for (const chunk of log.split('\x01')) {
      if (!chunk.trim()) continue;
      const fields = chunk.split(/[\0\n]/).map((s) => s.trim()).filter(Boolean);
      const sha = fields.shift();
      let overlap = 0;
      for (const f of fields) if (files.has(f)) overlap++;
      if (overlap > 0) commits.push({ sha, overlap, order: commits.length });
    }
    // git log lists newest first; the squash commit covers the branch's files best, and among
    // equals the OLDEST is the squash (later commits that also pass merely kept the change).
    // (A filter that skipped commits touching under half as many of the branch's files as the
    // best one was tried on 2026-11-20 and lost a real squash commit: a big unrelated commit
    // can touch more of the branch's files than the squash itself. Overlap only orders.)
    commits.sort((a, b) => b.overlap - a.overlap || b.order - a.order);
    const limit = opts.candidates == null ? DEFAULT_CANDIDATES : opts.candidates;
    const tryList = commits.filter((c) => c.sha !== mainTip).slice(0, limit);
    // Exact proofs first, over every candidate; the resolved-conflict reading only after.
    const conflicted = [];
    for (const c of tryList) {
      const m = mergeTree(cwd, c.sha, tip, env);
      if (m.error) return { landed: false, reason: m.error };
      if (m.clean && m.tree === treeOf(cwd, c.sha)) {
        return { landed: true, how: 'content-at-commit', at: c.sha };
      }
      if (!m.clean && m.tree) conflicted.push({ sha: c.sha, tree: m.tree });
    }

    // Last, main as it stands: catches changes that reached main in several pieces.
    const atTip = mergeTree(cwd, mainTip, tip, env);
    if (atTip.error) return { landed: false, reason: atTip.error };
    if (atTip.clean && atTip.tree === treeOf(cwd, mainTip)) {
      return { landed: true, how: 'content-at-tip', at: mainTip };
    }
    if (!atTip.clean && atTip.tree) conflicted.push({ sha: mainTip, tree: atTip.tree });
    for (const c of conflicted) {
      if (resolvedLanding(cwd, c.sha, c.tree, env).ok) {
        return { landed: true, how: 'content-resolved', at: c.sha };
      }
    }
    const n = git(cwd, ['rev-list', '--count', `${mainTip}..${tip}`]).stdout.trim();
    // Measured 2026-11-20: most branches still unproven after (a) to (c) had a squash commit that
    // touched every one of their files, merged with the agent's lines adapted (a regenerated
    // code map, code reworked while resolving). Content cannot prove those; the merger can. Name
    // the likeliest commit, so a person can look and record it.
    const likely = commits[0] && commits[0].overlap * 2 >= files.size ? commits[0] : null;
    const hint = likely
      ? `; the likeliest squash commit is ${likely.sha.slice(0, 9)}, touching ${likely.overlap} of its ${files.size} file(s): if it is, \`just record-merge <branch> --squash ${likely.sha.slice(0, 9)}\``
      : '';
    return {
      landed: false,
      likely: likely ? likely.sha : null,
      reason: `${n} commit(s) not in main, and main does not hold every change they make to ${files.size} file(s) ` +
        `(tried main's tip and ${tryList.length} earlier main commit(s) touching them)${hint}`,
    };
  } finally {
    if (own) own.dispose();
  }
}

/** The ledger, newest line last. Malformed lines are reported, never fatal. */
function readLedger(file) {
  const entries = [];
  const bad = [];
  if (!fs.existsSync(file)) return { entries, bad };
  const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
  lines.forEach((line, i) => {
    if (!line.trim()) return;
    try {
      const e = JSON.parse(line);
      if (e && typeof e.branch === 'string' && typeof e.head === 'string') entries.push(e);
      else bad.push(i + 1);
    } catch (_) {
      bad.push(i + 1);
    }
  });
  return { entries, bad };
}

/** Append one entry; keeps the file's line ending (CRLF in a CRLF checkout). */
function appendLedger(file, entry) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  let eol = '\n';
  let prefix = '';
  if (fs.existsSync(file)) {
    const text = fs.readFileSync(file, 'utf8');
    if (text.includes('\r\n')) eol = '\r\n';
    if (text.length && !text.endsWith('\n')) prefix = eol;
  }
  fs.appendFileSync(file, prefix + JSON.stringify(entry) + eol);
}

/**
 * The ledger entry that covers this branch at this tip: one whose recorded head IS the tip, or
 * a descendant of it (the branch was reset back). A tip with commits after the recorded head is
 * NOT covered: that is work made after the merge.
 */
function ledgerCovers(cwd, entries, branch, tip) {
  const mine = entries.filter((e) => e.branch === branch).reverse();
  for (const e of mine) {
    if (e.head === tip) return e;
  }
  for (const e of mine) {
    if (isAncestor(cwd, tip, e.head) === true) return e;
  }
  return null;
}

/**
 * The whole verdict for one commit (a branch tip or a detached HEAD):
 *   { landed: true, how: 'history' | 'ledger' | 'content-at-tip' | 'content-at-commit', at, entry }
 *   { landed: false, reason }
 */
function landed(cwd, { branch, tip, mainRef = 'main', ledger = [], objects, candidates }) {
  const anc = isAncestor(cwd, tip, mainRef);
  if (anc === true) return { landed: true, how: 'history', at: revParse(cwd, mainRef) };
  if (anc === null) return { landed: false, reason: `git cannot compare ${tip.slice(0, 9)} with ${mainRef}` };
  if (branch) {
    const e = ledgerCovers(cwd, ledger, branch, tip);
    if (e) return { landed: true, how: 'ledger', at: e.squash || null, entry: e };
  }
  return landedByContent(cwd, tip, mainRef, { objects, candidates });
}

module.exports = {
  ALLOWED_DIRT,
  DEFAULT_CANDIDATES,
  LEDGER_REL,
  appendLedger,
  cleanEnv,
  git,
  gitOk,
  isAncestor,
  landed,
  landedByContent,
  ledgerCovers,
  listWorktrees,
  mainWorktree,
  mergeTree,
  readLedger,
  resolvedLanding,
  revParse,
  samePath,
  scratchObjects,
  treeOf,
  uncommitted,
};
