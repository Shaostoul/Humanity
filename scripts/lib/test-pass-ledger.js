// What scripts/lib-tests.js needs to (1) not repeat a full pass on a tree that already passed,
// and (2) tell a timing flake from a real failure (2026-10-23).
//
// THE TREE. A full lib-test pass is recorded against an identity of the whole checkout, not
// just of the compiled-in sources (scripts/lib/src-fingerprint.js): the lib tests also READ the
// tree at run time (data/ about 280 times, and tests/, scripts/, web/, assets/ and docs/; grep
// CARGO_MANIFEST_DIR under src/), so a data-only edit can turn a test red with no source
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

module.exports = { parsePorcelainZ, treeIdentity, isFullRun, nonSelecting, parseFailures, readJson, writeJsonAtomic, SELECTING };
