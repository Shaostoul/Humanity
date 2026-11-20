#!/usr/bin/env node
// merged-branches: the ledger of agent branches that have landed on main
// (data/coordination/merged-branches.jsonl, one JSON object per line).
//
// WHY IT EXISTS (2026-11-20): every agent branch is squash-merged, so main's history never
// contains its commits and `just clean-worktrees` could not tell landed work from unmerged work
// (on 2026-11-16 it removed 2 of 211 worktrees). The merge step now writes down what it landed,
// and the cleanup reads it (scripts/lib/landed.js explains the three proofs it uses).
//
//   just record-merge <branch>             the merge step, right after the squash commit
//                                          (the merge-agent skill, section 3)
//   just merged-branches backfill          which existing branches are landed by content
//   just merged-branches backfill --write  ... and append them to the ledger
//   just merged-branches check <branch>    the verdict for one branch, with its reason
//   just merged-branches list              the ledger, newest last
//
// A ledger line: {"branch", "head" (the branch commit that was merged), "squash" (the main
// commit holding it, or null), "date" (that commit's date), "recorded" (when written), "how"
// ("merge-step" | "backfill-at-commit" | "backfill-resolved" | "backfill-at-tip"), "content" ("full": that main commit
// holds every change of the branch; "partial": it does not, landed by the merger's word),
// "subject"}. The cleanup trusts a line only while the branch tip is still "head" (or behind it).
//
// The ledger is written in the MAIN checkout (the first `git worktree list` entry), wherever
// this runs from, because that is where the merges happen and where the cleanup reads it.
// Commit it with your next commit: `just mine data/coordination/merged-branches.jsonl`.
// --ledger <file> writes elsewhere (the tests, or a backfill prepared in a worktree).
//
// Tested by scripts/tests/clean-worktrees.test.js.
'use strict';

const path = require('path');
const L = require('./lib/landed');

const HELP = `Usage:
  node scripts/merged-branches.js record <branch> [--squash <commit>] [--main <ref>] [--ledger <file>]
  node scripts/merged-branches.js backfill [--write] [--main <ref>] [--candidates N] [--ledger <file>]
  node scripts/merged-branches.js check <branch> [--main <ref>] [--ledger <file>]
  node scripts/merged-branches.js list [--ledger <file>]`;

function parse(argv) {
  const o = { cmd: argv[0], args: [], write: false, main: 'main', squash: null, ledger: null, candidates: L.DEFAULT_CANDIDATES };
  for (let i = 1; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--write') o.write = true;
    else if (a === '--main') o.main = argv[++i];
    else if (a === '--squash') o.squash = argv[++i];
    else if (a === '--ledger') o.ledger = argv[++i];
    else if (a === '--candidates') o.candidates = Number(argv[++i]);
    else if (a.startsWith('--')) throw new Error(`unknown option ${a}\n${HELP}`);
    else o.args.push(a);
  }
  return o;
}

function commitInfo(cwd, sha) {
  const out = L.gitOk(cwd, ['log', '-1', '--format=%cI%x00%s', sha]);
  const [date, subject] = out.split('\0');
  return { date, subject: subject.length > 100 ? subject.slice(0, 97) + '...' : subject };
}

/** The merge step: write down that <branch> landed as <squash> (default: main's tip). */
function record(cwd, o, log = console.log) {
  const branch = o.args[0];
  if (!branch) throw new Error(`record needs the branch name\n${HELP}`);
  const main = L.mainWorktree(cwd);
  const ledgerFile = o.ledger || path.join(main, L.LEDGER_REL);
  const head = L.revParse(main, `refs/heads/${branch}`);
  if (!head) throw new Error(`no local branch named ${branch}`);
  const squash = L.revParse(main, o.squash || o.main);
  if (!squash) throw new Error(`cannot read ${o.squash || o.main}`);
  if (L.isAncestor(main, squash, o.main) !== true) {
    throw new Error(`${squash.slice(0, 9)} is not on ${o.main}: record a commit that main contains (did the squash commit fail?)`);
  }
  const { entries } = L.readLedger(ledgerFile);
  const already = entries.find((e) => e.branch === branch && e.head === head);
  if (already) {
    log(`${branch} at ${head.slice(0, 9)} is already recorded (main commit ${String(already.squash).slice(0, 9)}); nothing written.`);
    return already;
  }
  const objects = L.scratchObjects(main);
  let content;
  try {
    const m = L.mergeTree(main, squash, head, objects.env);
    content = m.clean && m.tree === L.treeOf(main, squash) ? 'full' : 'partial';
  } finally {
    objects.dispose();
  }
  const info = commitInfo(main, squash);
  const entry = { branch, head, squash, date: info.date, recorded: new Date().toISOString(), how: 'merge-step', content, subject: info.subject };
  L.appendLedger(ledgerFile, entry);
  log(`Recorded: ${branch} (${head.slice(0, 9)}) landed as ${squash.slice(0, 9)} "${info.subject}".`);
  if (content === 'partial') {
    log(`NOTE: ${squash.slice(0, 9)} does not hold every change ${branch} makes (a part was left out on purpose, ` +
      'or this is the wrong commit: pass --squash <commit>). Recorded anyway, as your word that it landed; the cleanup will remove its worktree.');
  }
  log(`Commit the ledger with your next commit: just mine ${L.LEDGER_REL.replace(/\\/g, '/')}`);
  return entry;
}

/** Every local branch not in main's history and not in the ledger, judged by content. */
function backfill(cwd, o, log = console.log) {
  const main = L.mainWorktree(cwd);
  const ledgerFile = o.ledger || path.join(main, L.LEDGER_REL);
  const { entries } = L.readLedger(ledgerFile);
  const branches = L.git(main, ['branch', '--format=%(refname:short)']).stdout.split(/\r?\n/).filter((b) => b && b !== o.main);
  const objects = L.scratchObjects(main);
  const found = [];
  let inHistory = 0;
  let inLedger = 0;
  const notLanded = [];
  try {
    branches.forEach((b, i) => {
      const tip = L.revParse(main, `refs/heads/${b}`);
      if (!tip) return;
      if (L.isAncestor(main, tip, o.main) === true) { inHistory++; return; }
      if (L.ledgerCovers(main, entries, b, tip)) { inLedger++; return; }
      const v = L.landedByContent(main, tip, o.main, { objects, candidates: o.candidates });
      if (v.landed) {
        const atCommit = v.how === 'content-at-commit' || v.how === 'content-resolved';
        const info = atCommit ? commitInfo(main, v.at) : { date: null, subject: null };
        const entry = {
          branch: b,
          head: tip,
          squash: atCommit ? v.at : null,
          date: info.date,
          recorded: new Date().toISOString(),
          how: { 'content-at-commit': 'backfill-at-commit', 'content-resolved': 'backfill-resolved' }[v.how] || 'backfill-at-tip',
          content: 'full',
          subject: info.subject,
        };
        found.push(entry);
        log(`[${i + 1}/${branches.length}] landed  ${b} ${entry.squash ? 'as ' + entry.squash.slice(0, 9) + ' "' + entry.subject + '"' : '(main already holds every change)'}`);
        if (o.write) L.appendLedger(ledgerFile, entry);
      } else {
        notLanded.push(b);
        log(`[${i + 1}/${branches.length}] kept    ${b}: ${v.reason}`);
      }
    });
  } finally {
    objects.dispose();
  }
  log('');
  log(`${branches.length} branch(es): ${inHistory} in main's history, ${inLedger} already in the ledger, ` +
    `${found.length} landed by content, ${notLanded.length} not proven landed (kept as unmerged work).`);
  log(o.write ? `Appended ${found.length} line(s) to ${ledgerFile}.` : 'Nothing written: pass --write to append the landed ones to the ledger.');
  return { found, notLanded, inHistory, inLedger };
}

function check(cwd, o, log = console.log) {
  const branch = o.args[0];
  if (!branch) throw new Error(`check needs the branch name\n${HELP}`);
  const main = L.mainWorktree(cwd);
  const ledgerFile = o.ledger || path.join(main, L.LEDGER_REL);
  const tip = L.revParse(main, `refs/heads/${branch}`);
  if (!tip) throw new Error(`no local branch named ${branch}`);
  const objects = L.scratchObjects(main);
  try {
    const v = L.landed(main, { branch, tip, mainRef: o.main, ledger: L.readLedger(ledgerFile).entries, objects, candidates: o.candidates });
    log(v.landed ? `${branch}: landed (${v.how}${v.at ? ', main commit ' + v.at.slice(0, 9) : ''})` : `${branch}: NOT landed: ${v.reason}`);
    return v;
  } finally {
    objects.dispose();
  }
}

function list(cwd, o, log = console.log) {
  const main = L.mainWorktree(cwd);
  const ledgerFile = o.ledger || path.join(main, L.LEDGER_REL);
  const { entries, bad } = L.readLedger(ledgerFile);
  for (const e of entries) log(`${(e.date || '').slice(0, 10) || '----------'} ${String(e.squash || '-').slice(0, 9).padEnd(9)} ${e.branch} (${e.how}${e.content === 'partial' ? ', partial' : ''})`);
  log(`${entries.length} line(s) in ${ledgerFile}${bad.length ? `; unreadable line(s): ${bad.join(', ')}` : ''}`);
  return entries;
}

function run(argv, cwd = process.cwd(), log = console.log) {
  const o = parse(argv);
  switch (o.cmd) {
    case 'record': return record(cwd, o, log);
    case 'backfill': return backfill(cwd, o, log);
    case 'check': return check(cwd, o, log);
    case 'list': return list(cwd, o, log);
    default: throw new Error(HELP);
  }
}

if (require.main === module) {
  try {
    run(process.argv.slice(2));
  } catch (e) {
    console.error(`merged-branches: ${e.message}`);
    process.exit(1);
  }
}

module.exports = { run, record, backfill, check, list };
