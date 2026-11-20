// `just clean-worktrees` (scripts/clean-worktrees.js), the landed proofs (scripts/lib/landed.js)
// and the ledger (scripts/merged-branches.js), on throwaway repositories in the OS temp folder
// (prefix hum_cleanwt_, swept by `just clean-test-temp` if a run is killed). Never this checkout.
//
// The failure it closes (2026-11-16): every agent branch is squash-merged, so the old cleanup's
// "is the tip an ancestor of main?" said "unmerged" for every landed branch, and it removed 2 of
// 211 worktrees. The opposite failure is the one it must never reintroduce (2026-07-01, three
// times in one day): removing a worktree that holds real work.
//
// RED FIRST (2026-11-20). The OLD scripts/clean-worktrees.sh, run on this file's scenario,
// removed only `fresh` and kept `squashed`, `edited-after` and `settings-only` as "commits not
// merged into main": the 2026-11-16 failure, reproduced. Then each guard was broken in a COPY of
// the scripts and the suite run on the copy (the tests named fail; nothing else changed):
// - landed() reduced to the ancestor test: the plan, real-run, dry-run and ledger tests.
// - candidates removed (main's tip only): the plan, squash-commit, real-run, dry-run and backfill
//   tests (`edited-after`, whose line main later rewrote, was kept).
// - every uncommitted change allowed: the plan, real-run, dry-run and hook tests.
// - judge() without the lock check: the plan, dry-run and force tests. The real run still kept
//   the locked worktree: the re-check just before removing saw the lock (a second wall).
// - the ledger trusted whatever the branch tip: the ledger test.
// - the idle guard removed: the idle test.
// - the scratch object store dropped (trees written into .git/objects): the plan test.
// - the detached-HEAD guard removed: the plan, real-run and dry-run tests.
// - REPO_POINTERS kept (GIT_DIR, GIT_INDEX_FILE inherited): the FIRST version of the hook test
//   still passed, because its dirty file differed from the main checkout's index as well; it was
//   rewritten so the edit matches main's newer commit, and now fails.
// - the resolved-conflict reading off, or regionLands() ignoring added lines, or ignoring removed
//   lines: the plan, real-run, dry-run and backfill tests (`resolved` kept, or `dropped` /
//   `undeleted` removed).
// - nameList() answering [] on a git error: "a git error never proves a landing". This hole was
//   REAL before it was a mutant: the first resolved reading ran its diff without the scratch
//   store, the diff failed, the empty answer proved `dropped` landed, and the dropped test caught
//   it; on the real worktrees it had "proved" three branches the fixed code does not.
// - the main-checkout guard removed: the refusal test (and the idle test after it, because the
//   unguarded run really removed worktrees from the shared scenario).

const { test, after } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const L = require(path.join(ROOT, 'lib', 'landed.js'));
const CW = require(path.join(ROOT, 'clean-worktrees.js'));
const MB = require(path.join(ROOT, 'merged-branches.js'));

function makeRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hum_cleanwt_test_'));
  const hooks = path.join(dir, '.nohooks');
  const g = (cwd, ...args) => {
    const r = spawnSync('git', args, { cwd, encoding: 'utf8', env: L.cleanEnv() });
    if (r.status !== 0) throw new Error(`git ${args.join(' ')} (in ${cwd}): ${r.stderr}`);
    return r.stdout.trim();
  };
  const repo = path.join(dir, 'repo');
  fs.mkdirSync(repo);
  fs.mkdirSync(hooks);
  g(repo, 'init', '-q', '-b', 'main');
  g(repo, 'config', 'user.name', 'test');
  g(repo, 'config', 'user.email', 'test@example.invalid');
  g(repo, 'config', 'commit.gpgsign', 'false');
  g(repo, 'config', 'core.hooksPath', hooks);
  g(repo, 'config', 'core.autocrlf', 'false');
  const write = (cwd, file, text) => {
    fs.mkdirSync(path.dirname(path.join(cwd, file)), { recursive: true });
    fs.writeFileSync(path.join(cwd, file), text);
  };
  const commit = (cwd, msg, files) => {
    for (const [f, t] of Object.entries(files)) write(cwd, f, t);
    g(cwd, 'add', '-f', '--', ...Object.keys(files)); // -f: a global ignore may list the settings file
    g(cwd, 'commit', '-q', '--no-verify', '-m', msg);
    return g(cwd, 'rev-parse', 'HEAD');
  };
  // Like the real repository before a047d3d05: the settings file is TRACKED.
  commit(repo, 'base', {
    'a.txt': 'a1\na2\na3\n',
    'b.txt': 'b1\nb2\nb3\n',
    'notes.md': 'one\n\ntwo\n\nthree\n',
    'resolved.md': 'start\nTODO\nend\n',
    'dropped.md': 'start\nTODO\nend\n',
    'undeleted.md': 'start\nOLD\nnext\nend\n',
    '.claude/settings.local.json': '{"permissions":{"allow":[]}}\n',
  });
  const wtDir = path.join(repo, '.claude', 'worktrees');
  const add = (name) => {
    const p = path.join(wtDir, name);
    g(repo, 'worktree', 'add', '-q', '-b', `worktree-agent-${name}`, p, 'main');
    return p;
  };
  // The merge-agent skill's merge: squash, then one commit on main.
  const squash = (name, msg) => {
    g(repo, 'merge', '--squash', `worktree-agent-${name}`);
    g(repo, 'commit', '-q', '--no-verify', '-m', msg);
    return g(repo, 'rev-parse', 'HEAD');
  };
  // A squash that CONFLICTS (main changed the same lines meanwhile), resolved by hand.
  const conflictSquash = (name, resolved, msg) => {
    const r = spawnSync('git', ['merge', '--squash', `worktree-agent-${name}`], { cwd: repo, encoding: 'utf8', env: L.cleanEnv() });
    if (r.status === 0) throw new Error(`expected ${name}'s squash to conflict`);
    for (const [f, t] of Object.entries(resolved)) write(repo, f, t);
    g(repo, 'add', '--', ...Object.keys(resolved));
    g(repo, 'commit', '-q', '--no-verify', '-m', msg);
    return g(repo, 'rev-parse', 'HEAD');
  };
  const cleanup = () => fs.rmSync(dir, { recursive: true, force: true });
  return { dir, repo, g, write, commit, add, squash, conflictSquash, cleanup };
}

const quiet = () => {};
const OPTS = { dryRun: true, yes: false, all: false, force: false, minIdleHours: 0, main: 'main', candidates: 8 };
const rowFor = (p, name) => p.rows.find((r) => path.basename(r.wt.path) === name);

/** The scenario most tests share: one worktree per case. */
function scenario() {
  const t = makeRepo();
  const { repo, commit, add, squash, write } = t;
  const w = {};

  // squashed: two commits, squash-merged, then main moves on in another file.
  w.squashed = add('squashed');
  commit(w.squashed, 's1', { 'a.txt': 'a1\nA2 squashed\na3\n' });
  commit(w.squashed, 's2', { 'new.txt': 'from squashed\n' });
  squash('squashed', 'squashed: the feature');
  commit(repo, 'later on main', { 'b.txt': 'b1\nb2\nb3 later\n' });

  // edited-after: squash-merged, then main REWRITES the same line, so a merge into main's tip
  // conflicts; the proof must find the squash commit instead.
  w['edited-after'] = add('edited-after');
  commit(w['edited-after'], 'e1', { 'b.txt': 'b1\nB2 edited-after\nb3 later\n' });
  t.editedSquash = squash('edited-after', 'edited-after: its feature');
  commit(repo, 'main rewrites that line', { 'b.txt': 'b1\nB2 rewritten by main\nb3 later\n' });

  // settings-only: squash-merged; its only uncommitted change is the old tracked settings file.
  w['settings-only'] = add('settings-only');
  commit(w['settings-only'], 'so1', { 'notes.md': 'one\n\ntwo settings-only\n\nthree\n' });
  squash('settings-only', 'settings-only: notes');
  write(w['settings-only'], '.claude/settings.local.json', '{"permissions":{"allow":["Bash(ls)"]}}\n');

  // unmerged: a commit main never received.
  w.unmerged = add('unmerged');
  commit(w.unmerged, 'u1', { 'unmerged.txt': 'real unlanded work\n' });

  // dirty: landed, but holds an uncommitted edit to a tracked file.
  w.dirty = add('dirty');
  commit(w.dirty, 'd1', { 'dirty.txt': 'landed part\n' });
  squash('dirty', 'dirty: landed part');
  write(w.dirty, 'a.txt', 'uncommitted edit\n');

  // untracked: landed, but holds a new file nobody added.
  w.untracked = add('untracked');
  commit(w.untracked, 'un1', { 'untracked-landed.txt': 'landed\n' });
  squash('untracked', 'untracked: landed part');
  write(w.untracked, 'src/forgot_to_add.rs', 'fn main() {}\n');

  // locked: landed, but an agent holds it.
  w.locked = add('locked');
  commit(w.locked, 'l1', { 'locked.txt': 'landed\n' });
  squash('locked', 'locked: landed');
  t.g(repo, 'worktree', 'lock', '--reason', 'claude agent agent-locked (pid 999999)', w.locked);

  // resolved: main replaced the same line while the agent worked, so the squash conflicted and
  // was resolved by hand keeping both. A re-merge conflicts again, yet main holds all its lines.
  w.resolved = add('resolved');
  commit(w.resolved, 'r1', { 'resolved.md': 'start\nagent: done it\nend\n' });
  commit(repo, 'main replaces the same line', { 'resolved.md': 'start\nmain: a concurrent note\nend\n' });
  t.resolvedSquash = t.conflictSquash('resolved', { 'resolved.md': 'start\nmain: a concurrent note\nagent: done it\nend\n' }, 'resolved: merged by hand');

  // dropped: the same, but the hand resolution threw the agent's line away (its other file
  // landed). Main does not hold its work, so it is kept.
  w.dropped = add('dropped');
  commit(w.dropped, 'dr1', { 'dropped.md': 'start\nagent: the dropped line\nend\n', 'dropped-landed.txt': 'landed\n' });
  commit(repo, 'main replaces that line too', { 'dropped.md': 'start\nmain: kept instead\nend\n' });
  t.conflictSquash('dropped', { 'dropped.md': 'start\nmain: kept instead\nend\n' }, 'dropped: resolved keeping only main');

  // undeleted: the agent deleted a line, main edited the next one, and the hand resolution kept
  // the deleted line. A removal that did not land is unlanded work too: kept.
  w.undeleted = add('undeleted');
  commit(w.undeleted, 'ud1', { 'undeleted.md': 'start\nnext\nend\n', 'undeleted-landed.txt': 'landed\n' });
  commit(repo, 'main edits the next line', { 'undeleted.md': 'start\nOLD\nnext edited\nend\n' });
  t.conflictSquash('undeleted', { 'undeleted.md': 'start\nOLD\nnext edited\nend\n' }, 'undeleted: resolved keeping the old line');

  // fresh: made from main, no commits yet (history says landed).
  w.fresh = add('fresh');

  // detached: a commit on no branch, landed by content.
  w.detached = path.join(repo, '.claude', 'worktrees', 'detached');
  t.g(repo, 'worktree', 'add', '-q', '--detach', w.detached, 'main');
  commit(w.detached, 'det1', { 'detached.txt': 'detached work\n' });
  t.g(repo, 'cherry-pick', '--no-commit', t.g(w.detached, 'rev-parse', 'HEAD'));
  t.g(repo, 'commit', '-q', '--no-verify', '-m', 'detached work, landed by hand');

  return { ...t, w };
}

// Building a scenario takes about fifty git commands, slow on Windows, so the tests that only
// read it share one; its cleanup() does nothing and the folder goes after the last test. The
// real run, which changes its repository, builds its own.
let sharedScenario = null;
function shared() {
  if (!sharedScenario) sharedScenario = scenario();
  return { ...sharedScenario, cleanup: () => {} };
}
after(() => { if (sharedScenario) sharedScenario.cleanup(); });

test('a squash-merged branch is removed, an unmerged one and one with uncommitted work are kept', () => {
  const t = shared();
  try {
    const objects = () => t.g(t.repo, 'count-objects', '-v');
    const before = objects();
    const p = CW.plan(OPTS, t.repo);
    // The proofs wrote their scratch trees to a temp folder, not the repository.
    assert.strictEqual(objects(), before, 'the plan must write nothing into .git/objects');
    const act = (n) => rowFor(p, n).action;
    assert.strictEqual(act('squashed'), 'remove', rowFor(p, 'squashed').reason);
    assert.strictEqual(act('edited-after'), 'remove', rowFor(p, 'edited-after').reason);
    assert.strictEqual(act('settings-only'), 'remove', rowFor(p, 'settings-only').reason);
    assert.match(rowFor(p, 'settings-only').reason, /settings\.local\.json/);
    assert.strictEqual(act('fresh'), 'remove', rowFor(p, 'fresh').reason);
    assert.strictEqual(act('unmerged'), 'keep');
    assert.match(rowFor(p, 'unmerged').reason, /not landed/);
    assert.strictEqual(act('dirty'), 'keep');
    assert.match(rowFor(p, 'dirty').reason, /uncommitted change.*a\.txt/);
    assert.strictEqual(act('untracked'), 'keep');
    assert.match(rowFor(p, 'untracked').reason, /src\//);
    assert.strictEqual(act('locked'), 'keep');
    assert.match(rowFor(p, 'locked').reason, /locked/);
    assert.strictEqual(act('detached'), 'keep');
    assert.match(rowFor(p, 'detached').reason, /on no branch/);
    assert.strictEqual(act('resolved'), 'remove', rowFor(p, 'resolved').reason);
    assert.strictEqual(rowFor(p, 'resolved').how, 'content-resolved');
    assert.strictEqual(act('dropped'), 'keep', rowFor(p, 'dropped').reason);
    // ...and names a commit a person would check and record with `just record-merge`. (Here the
    // squash and main's own edit each touch one of its two files: the hint says so, in numbers.)
    assert.match(rowFor(p, 'dropped').reason, /likeliest squash commit is [0-9a-f]{9}, touching 1 of its 2 file\(s\): if it is, `just record-merge <branch> --squash/);
    assert.strictEqual(act('undeleted'), 'keep', rowFor(p, 'undeleted').reason);
  } finally {
    t.cleanup();
  }
});

test('the content proof finds the squash commit when main later rewrote the same lines', () => {
  const t = shared();
  try {
    const tip = t.g(t.repo, 'rev-parse', 'worktree-agent-edited-after');
    const v = L.landedByContent(t.repo, tip, 'main');
    assert.strictEqual(v.landed, true, v.reason);
    assert.strictEqual(v.how, 'content-at-commit');
    assert.strictEqual(v.at, t.editedSquash);
  } finally {
    t.cleanup();
  }
});

test('a real run removes only the landed worktrees, keeps every branch, and leaves the rest untouched', () => {
  const t = scenario();
  try {
    const before = t.g(t.repo, 'branch', '--format=%(refname:short)').split('\n').sort();
    const r = spawnSync(process.execPath, [path.join(ROOT, 'clean-worktrees.js'), '--yes', '--min-idle-hours', '0'], { cwd: t.repo, encoding: 'utf8', env: L.cleanEnv() });
    assert.strictEqual(r.status, 0, r.stderr + r.stdout);
    const gone = (n) => !fs.existsSync(t.w[n]);
    for (const n of ['squashed', 'edited-after', 'settings-only', 'fresh', 'resolved']) assert.ok(gone(n), `${n} should be removed\n${r.stdout}`);
    for (const n of ['unmerged', 'dirty', 'untracked', 'locked', 'detached', 'dropped', 'undeleted']) assert.ok(!gone(n), `${n} should be kept\n${r.stdout}`);
    // The kept work is byte-for-byte where it was.
    assert.strictEqual(fs.readFileSync(path.join(t.w.dirty, 'a.txt'), 'utf8'), 'uncommitted edit\n');
    assert.ok(fs.existsSync(path.join(t.w.untracked, 'src', 'forgot_to_add.rs')));
    // Every branch survives except a merged orphan; the fresh one's branch is in main's history,
    // so it is the only one deleted.
    const after = t.g(t.repo, 'branch', '--format=%(refname:short)').split('\n').sort();
    assert.deepStrictEqual(after, before.filter((b) => b !== 'worktree-agent-fresh'));
  } finally {
    t.cleanup();
  }
});

test('a dry run changes nothing', () => {
  const t = shared();
  try {
    const list = () => t.g(t.repo, 'worktree', 'list', '--porcelain');
    const before = list();
    const r = spawnSync(process.execPath, [path.join(ROOT, 'clean-worktrees.js'), '--dry-run', '--min-idle-hours', '0'], { cwd: t.repo, encoding: 'utf8', env: L.cleanEnv() });
    assert.strictEqual(r.status, 0, r.stderr);
    assert.match(r.stdout, /WOULD REMOVE 5 worktree/);
    assert.match(r.stdout, /Dry run: nothing was changed/);
    assert.strictEqual(list(), before);
    for (const n of Object.keys(t.w)) assert.ok(fs.existsSync(t.w[n]), n);
  } finally {
    t.cleanup();
  }
});

test('removing from inside a worktree (where a dispatched agent works) is refused; looking is not', () => {
  const t = shared();
  try {
    const list = () => t.g(t.repo, 'worktree', 'list', '--porcelain');
    const before = list();
    const script = path.join(ROOT, 'clean-worktrees.js');
    const r = spawnSync(process.execPath, [script, '--yes', '--min-idle-hours', '0'], { cwd: t.w.unmerged, encoding: 'utf8', env: L.cleanEnv() });
    assert.strictEqual(r.status, 2, r.stdout + r.stderr);
    assert.match(r.stderr, /refused.*main checkout/s);
    assert.strictEqual(list(), before);
    for (const n of Object.keys(t.w)) assert.ok(fs.existsSync(t.w[n]), n);
    const d = spawnSync(process.execPath, [script, '--dry-run', '--min-idle-hours', '0'], { cwd: t.w.unmerged, encoding: 'utf8', env: L.cleanEnv() });
    assert.strictEqual(d.status, 0, d.stderr);
    assert.match(d.stdout, /Dry run: nothing was changed/);
  } finally {
    t.cleanup();
  }
});

test('a recently active worktree is kept until it has been idle', () => {
  const t = shared();
  try {
    const p = CW.plan({ ...OPTS, minIdleHours: 2 }, t.repo);
    for (const n of ['squashed', 'fresh']) {
      assert.strictEqual(rowFor(p, n).action, 'keep');
      assert.match(rowFor(p, n).reason, /may be in use/);
    }
  } finally {
    t.cleanup();
  }
});

test('--force-unmerged destroys unlanded work but never a locked worktree', () => {
  const t = shared();
  try {
    const p = CW.plan({ ...OPTS, force: true }, t.repo);
    assert.strictEqual(rowFor(p, 'unmerged').action, 'remove');
    assert.strictEqual(rowFor(p, 'unmerged').destroys, true);
    assert.strictEqual(rowFor(p, 'dirty').action, 'remove');
    assert.strictEqual(rowFor(p, 'locked').action, 'keep');
  } finally {
    t.cleanup();
  }
});

test('the ledger: a partial merge counts by the merger\'s word, and commits after it do not', () => {
  const t = makeRepo();
  try {
    const wt = t.add('partial');
    t.commit(wt, 'p1', { 'keep.txt': 'landed\n', 'dropped.txt': 'left out on purpose\n' });
    // The merger lands only keep.txt.
    t.g(t.repo, 'merge', '--squash', 'worktree-agent-partial');
    t.g(t.repo, 'rm', '-q', '--cached', 'dropped.txt');
    fs.rmSync(path.join(t.repo, 'dropped.txt'));
    t.g(t.repo, 'commit', '-q', '--no-verify', '-m', 'partial: keep.txt only');
    const ledger = path.join(t.dir, 'ledger.jsonl');

    let p = CW.plan(OPTS, t.repo);
    assert.strictEqual(rowFor(p, 'partial').action, 'keep', 'without the ledger the content proof cannot pass');

    const e = MB.run(['record', 'worktree-agent-partial', '--ledger', ledger], t.repo, quiet);
    assert.strictEqual(e.content, 'partial');
    assert.strictEqual(e.squash, t.g(t.repo, 'rev-parse', 'main'));
    assert.strictEqual(e.head, t.g(t.repo, 'rev-parse', 'worktree-agent-partial'));
    // Recording twice writes once.
    MB.run(['record', 'worktree-agent-partial', '--ledger', ledger], t.repo, quiet);
    assert.strictEqual(L.readLedger(ledger).entries.length, 1);

    // The cleanup reads the main checkout's ledger: put it there.
    const real = path.join(t.repo, L.LEDGER_REL);
    fs.mkdirSync(path.dirname(real), { recursive: true });
    fs.copyFileSync(ledger, real);
    p = CW.plan(OPTS, t.repo);
    assert.strictEqual(rowFor(p, 'partial').action, 'remove', rowFor(p, 'partial').reason);
    assert.match(rowFor(p, 'partial').reason, /ledger/);

    // The agent keeps working after the merge: that commit is new work.
    t.commit(wt, 'p2', { 'after.txt': 'made after the merge\n' });
    p = CW.plan(OPTS, t.repo);
    assert.strictEqual(rowFor(p, 'partial').action, 'keep', rowFor(p, 'partial').reason);
  } finally {
    t.cleanup();
  }
});

test('record refuses a commit that is not on main', () => {
  const t = makeRepo();
  try {
    const wt = t.add('x');
    const side = t.commit(wt, 'x1', { 'x.txt': 'x\n' });
    assert.throws(() => MB.run(['record', 'worktree-agent-x', '--squash', side, '--ledger', path.join(t.dir, 'l.jsonl')], t.repo, quiet), /not on main/);
  } finally {
    t.cleanup();
  }
});

test('backfill finds the squash commit of each landed branch and skips the unmerged one', () => {
  const t = shared();
  try {
    const ledger = path.join(t.dir, 'ledger.jsonl');
    const objects = () => t.g(t.repo, 'count-objects', '-v');
    const before = objects();
    const r = MB.run(['backfill', '--write', '--ledger', ledger], t.repo, quiet);
    assert.strictEqual(objects(), before, 'the backfill must write nothing into .git/objects');
    const byBranch = Object.fromEntries(r.found.map((e) => [e.branch, e]));
    assert.ok(byBranch['worktree-agent-squashed'], 'squashed found');
    assert.strictEqual(byBranch['worktree-agent-edited-after'].squash, t.editedSquash);
    assert.strictEqual(byBranch['worktree-agent-edited-after'].how, 'backfill-at-commit');
    assert.ok(r.notLanded.includes('worktree-agent-unmerged'));
    assert.ok(r.notLanded.includes('worktree-agent-dropped'));
    assert.ok(r.notLanded.includes('worktree-agent-undeleted'));
    assert.strictEqual(byBranch['worktree-agent-resolved'].how, 'backfill-resolved');
    assert.strictEqual(byBranch['worktree-agent-resolved'].squash, t.resolvedSquash);
    assert.ok(!byBranch['worktree-agent-unmerged']);
    assert.strictEqual(L.readLedger(ledger).entries.length, r.found.length);
    // A second backfill finds nothing new: the ledger covers them.
    const again = MB.run(['backfill', '--ledger', ledger], t.repo, quiet);
    assert.strictEqual(again.found.length, 0);
    assert.strictEqual(again.inLedger, r.found.length);
  } finally {
    t.cleanup();
  }
});

test('a git error never proves a landing', () => {
  const t = shared();
  const objects = L.scratchObjects(t.repo);
  try {
    // A tree git cannot read: the diff fails, and a failure must not read as "no differences".
    const main = t.g(t.repo, 'rev-parse', 'main');
    const v = L.resolvedLanding(t.repo, main, '0123456789abcdef0123456789abcdef01234567', objects.env);
    assert.strictEqual(v.ok, false, JSON.stringify(v));
  } finally {
    objects.dispose();
    t.cleanup();
  }
});

test('inside a git hook (GIT_DIR, GIT_INDEX_FILE set) a dirty worktree is still seen as dirty', () => {
  // The worktree's uncommitted edit makes its files EQUAL to main's newer commit, so read
  // against the main checkout's repository and index (what a hook's variables point at) it
  // looks clean. (A first version of this test used an edit main did not have, which shows
  // up against either index, so it passed with the variables kept: it could not fail.)
  const t = makeRepo();
  const saved = { GIT_INDEX_FILE: process.env.GIT_INDEX_FILE, GIT_DIR: process.env.GIT_DIR };
  try {
    const wt = t.add('hooked');
    t.commit(t.repo, 'main moves a.txt on', { 'a.txt': 'a1\na2 newer\na3\n' });
    t.write(wt, 'a.txt', 'a1\na2 newer\na3\n');
    assert.ok(L.uncommitted(wt).real.some((e) => e.file === 'a.txt'), 'dirty without the variables');
    process.env.GIT_INDEX_FILE = path.join(t.repo, '.git', 'index');
    process.env.GIT_DIR = path.join(t.repo, '.git');
    const st = L.uncommitted(wt);
    assert.ok(st.ok, st.error);
    assert.ok(st.real.some((e) => e.file === 'a.txt'), JSON.stringify(st));
  } finally {
    for (const [k, v] of Object.entries(saved)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
    t.cleanup();
  }
});
