// What scripts/lib-tests.js needs to (1) not repeat a full pass on a tree that already passed,
// and (2) tell a timing flake from a real failure (2026-10-23).
//
// THE TREE. A full lib-test pass is recorded against an identity of the whole checkout, not
// just of the compiled-in sources (scripts/lib/src-fingerprint.js): the lib tests also READ the
// tree at run time (data/ about 280 times, and tests/, scripts/, web/, assets/ and docs/; grep
// HUMANITY_REPO_ROOT under src/ and crates/), so a data-only edit can turn a test red with no source
// change. The identity is:
//   - the commit's tree (`git rev-parse HEAD^{tree}`)
//   - every changed or untracked file git reports, with the SHA-256 of its content (CRLF read
//     as LF) or "deleted"
//   - `rustc -V` (the toolchain is pinned by rust-toolchain.toml; a new one is a new tree)
//   - the feature set the tests were built with
// Files git ignores (target/, saves, logs) are machine state, not the tree, and do not count;
// nor do the two kinds of file no test reads but runs write: the agents' own settings under
// .claude/, and the flaky-test ledger lib-tests.js itself appends to (else a run that recorded
// a flake would make its own tree look changed).
//
// THE OUTPUT. libtest ends a failed run with a "failures:" list naming each failed test; the
// last such list is the one parseFailures reads.

"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { spawnSync } = require("child_process");

const sha256 = (b) => crypto.createHash("sha256").update(b).digest("hex");

function git(root, args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8", windowsHide: true, maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed in ${root}: ${(r.stderr || r.error || "").toString().trim()}`);
  return r.stdout;
}

/** Parse `git status --porcelain=v1 -z` into the paths whose content matters (renames: the new path). */
function parsePorcelainZ(text) {
  const out = [];
  const parts = String(text || "").split("\0");
  for (let i = 0; i < parts.length; i++) {
    const e = parts[i];
    if (e.length < 4) continue;
    const x = e[0];
    const p = e.slice(3);
    out.push(p);
    if (x === "R" || x === "C") i++; // the next field is the old path
  }
  return [...new Set(out)].sort();
}

/** Paths that never count toward the identity (see the header). */
const NOT_INPUTS = (p) => p.startsWith(".claude/") || p === "data/coordination/flaky-tests.jsonl";

/** The identity of the checkout at `root` for a test run: { hash, head_tree, dirty, rustc, features }. */
function treeIdentity(root, { features = "native", rustc } = {}) {
  const headTree = git(root, ["rev-parse", "HEAD^{tree}"]).trim();
  const dirtyPaths = parsePorcelainZ(git(root, ["status", "--porcelain=v1", "-z", "--untracked-files=all"])).filter((p) => !NOT_INPUTS(p));
  const dirty = dirtyPaths.map((p) => {
    const f = path.join(root, p);
    let h = "deleted";
    try {
      const st = fs.statSync(f);
      if (st.isFile()) h = sha256(Buffer.from(fs.readFileSync(f, "latin1").replace(/\r\n/g, "\n"), "latin1"));
      else h = "not-a-file";
    } catch {}
    return [p, h];
  });
  const rustcV = rustc || (spawnSync("rustc", ["-V"], { cwd: root, encoding: "utf8", windowsHide: true }).stdout || "").trim();
  const id = { head_tree: headTree, dirty, rustc: rustcV, features };
  return { ...id, hash: sha256(JSON.stringify(id)) };
}

/** Flags of the test program that change WHICH tests run (with a value or not). */
const SELECTING = new Set(["--skip", "--exact", "--ignored", "--include-ignored", "--list", "--bench"]);
/** Flags that take a separate value. */
const WITH_VALUE = new Set(["--skip", "--test-threads", "--format", "--color", "-Z", "--logfile"]);

/** Does this argument list run the whole suite? (No name filter, nothing that selects.) */
function isFullRun(args) {
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    const flag = a.split("=")[0];
    if (SELECTING.has(flag)) return false;
    if (a.startsWith("-")) {
      if (WITH_VALUE.has(a)) i++;
      continue;
    }
    return false; // a name filter
  }
  return true;
}

/** The arguments with every name filter and selecting flag removed (what a rerun keeps). */
function nonSelecting(args) {
  const out = [];
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    const flag = a.split("=")[0];
    if (SELECTING.has(flag)) {
      if (WITH_VALUE.has(a)) i++;
      continue;
    }
    if (a.startsWith("-")) {
      out.push(a);
      if (WITH_VALUE.has(a) && i + 1 < args.length) out.push(args[++i]);
      continue;
    }
  }
  return out;
}

/** The failed tests libtest lists at the end of its output (its last "failures:" list). */
function parseFailures(output) {
  const lines = String(output || "").split(/\r?\n/);
  let start = -1;
  for (let i = 0; i < lines.length; i++) if (lines[i] === "failures:") start = i;
  if (start < 0) return [];
  const names = [];
  for (let i = start + 1; i < lines.length; i++) {
    const l = lines[i];
    if (!l.trim()) {
      if (names.length) break;
      continue;
    }
    const m = l.match(/^    (\S+)$/);
    if (!m) break;
    names.push(m[1]);
  }
  return names;
}

// THE RERUN'S PROOF (2026-11-21). A failure that passes when run again alone is logged FLAKY and
// does not fail the run, so "it passed alone" has to be proven, not assumed. Until this day it was
// "the rerun exited 0 and printed `running 1 test`": that never asked WHICH test ran, nor whether
// the rerun ran on the files the failure came from (a deterministic failure, a single-brace
// placeholder in data/i18n/en.json, was logged FLAKY on 2026-11-20 after its word was corrected
// while the suite was still running).

/** libtest's line per test (its pretty format). Not anchored at the start: with --nocapture a
 * test's own output can precede it on the line. */
const RESULT_RE = /test (\S+)(?: - should panic)? \.\.\. (ok|FAILED|ignored\b.*)$/;

/** The counts of a run's last "test result:" line: { passed, failed, ignored, filtered } or null. */
function resultCounts(output) {
  const lines = String(output || "").match(/^test result: .*$/gm);
  if (!lines) return null;
  const s = lines[lines.length - 1];
  const n = (k) => {
    const m = new RegExp(`(\\d+) ${k}`).exec(s);
    return m ? Number(m[1]) : 0;
  };
  return { passed: n("passed"), failed: n("failed"), ignored: n("ignored"), filtered: n("filtered out") };
}

/**
 * Did a run of one test, alone, run THAT test and pass it? The output must name `name` ("test
 * <name> ... "), say ok for it where it says a result, and count exactly one pass and no failure.
 * With --test-threads=1 libtest prints "test <name> ... " BEFORE the test runs and "ok" after it,
 * so with --nocapture the test's own output can stand between the two, on lines of its own: there
 * the name and the count are the proof (a rerun is run with --exact, so only that name can pass).
 * { ok: true } or { ok: false, why }, `why` saying in words what the output showed instead.
 */
function passedAlone(output, name) {
  const text = String(output || "");
  const results = [];
  for (const line of text.split(/\r?\n/)) {
    const m = RESULT_RE.exec(line);
    if (m) results.push({ name: m[1], result: m[2] });
  }
  const counts = resultCounts(text);
  const named = text.includes(`test ${name} ... `) || text.includes(`test ${name} - should panic ... `);
  const mine = results.find((r) => r.name === name);
  if (!counts) return { ok: false, why: "the rerun printed no result line (it did not run, or crashed)" };
  if (!named && counts.passed + counts.failed === 0) return { ok: false, why: `the rerun matched no test of that name (${counts.filtered} filtered out): nothing ran` };
  if (!named) return { ok: false, why: `the rerun ran ${results.map((r) => r.name).join(", ") || "no test it named"}, not ${name}` };
  if (mine && mine.result !== "ok") return { ok: false, why: `the rerun's result for it was "${mine.result}"` };
  if (counts.passed !== 1 || counts.failed !== 0) return { ok: false, why: `the rerun counted ${counts.passed} passed and ${counts.failed} failed, not exactly that one test` };
  return { ok: true };
}

/** What changed between two tree identities (treeIdentity), as lines a person reads; [] if none. */
function identityDiff(a, b) {
  if (!a || !b) return ["the checkout's identity could not be read, so nothing proves the files stayed the same"];
  if (a.hash === b.hash) return [];
  const out = [];
  if (a.head_tree !== b.head_tree) out.push(`the commit moved (its tree ${a.head_tree.slice(0, 12)} -> ${b.head_tree.slice(0, 12)}): a commit, checkout, merge or reset`);
  if (a.rustc !== b.rustc) out.push(`the compiler changed (${a.rustc} -> ${b.rustc})`);
  const before = new Map(a.dirty);
  const after = new Map(b.dirty);
  for (const [p, h] of after) if (before.get(p) !== h) out.push(`${p} (${h === "deleted" ? "deleted" : "changed"})`);
  for (const p of before.keys()) if (!after.has(p)) out.push(`${p} (back to the commit's version)`);
  if (!out.length) out.push("the checkout's identity changed");
  return out;
}

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

/** Write a JSON file by temp name and rename, so a reader never sees half of one. */
function writeJsonAtomic(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(value, null, 2) + "\n");
  fs.renameSync(tmp, file);
}

module.exports = { parsePorcelainZ, treeIdentity, isFullRun, nonSelecting, parseFailures, readJson, writeJsonAtomic, SELECTING, RESULT_RE, resultCounts, passedAlone, identityDiff };
