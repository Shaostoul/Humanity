// The lib-test run policy (scripts/lib-tests.js, scripts/lib/test-pass-ledger.js, 2026-10-23).
//
// What would make it a check that cannot fail: a refusal that also refuses a CHANGED tree (so a
// red edit "passes" by never running), a flake rerun that swallows a real failure, or a quiet
// mode that hides failures. Each is tried on a throwaway repository with a tiny real test
// program (fixtures/lib-tests/tiny_suite.rs) run in place of the library's
// (HOS_LIB_TESTS_ROOT, HOS_LIB_TESTS_EXE).

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const RC = require("../lib/rustc-cache.js");
const L = require("../lib/test-pass-ledger.js");

const REPO = path.resolve(__dirname, "..", "..");
const LIB_TESTS = path.join(REPO, "scripts", "lib-tests.js");
const SUITE = path.join(__dirname, "fixtures", "lib-tests", "tiny_suite.rs");

function suiteExe() {
  const p = RC.plan({ src: SUITE, name: "tiny_suite", cacheDir: path.join(REPO, "target", "test-cache"), args: ["--test", "--edition", "2021", "-A", "warnings"], env: {}, cwd: REPO });
  const b = RC.buildSync(p);
  assert.ok(b.ok, b.stderr);
  return b.exe;
}

function repo() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum_lib_tests_"));
  const g = (...a) => assert.strictEqual(spawnSync("git", a, { cwd: root, encoding: "utf8" }).status, 0, `git ${a.join(" ")}`);
  g("init", "-q");
  g("config", "user.email", "test@example.invalid");
  g("config", "user.name", "test");
  g("config", "core.autocrlf", "false");
  fs.writeFileSync(path.join(root, "a.txt"), "one\n");
  fs.writeFileSync(path.join(root, ".gitignore"), "/target/\n");
  g("add", "a.txt", ".gitignore");
  g("commit", "-q", "-m", "first");
  return root;
}

function run(root, exe, args = [], env = {}) {
  const r = spawnSync(process.execPath, [LIB_TESTS, ...args], {
    encoding: "utf8",
    env: { ...process.env, HOS_LIB_TESTS_ROOT: root, HOS_LIB_TESTS_EXE: exe, TINY_RUN_LOG: runLog(root), ...env },
  });
  return { status: r.status, out: (r.stdout || "") + (r.stderr || "") };
}
// Outside the repository: a file the test writes inside it would itself change the tree.
const runLog = (root) => `${root}.runs.log`;
const runs = (root) => (fs.existsSync(runLog(root)) ? fs.readFileSync(runLog(root), "utf8").split("\n").filter(Boolean) : []);

test("a full pass is recorded, and the same tree is refused until it changes or --force", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    let r = run(root, exe);
    assert.strictEqual(r.status, 0, r.out);
    assert.ok(fs.existsSync(path.join(root, "target", "lib-tests", "last-full-pass.json")));
    const before = runs(root).length;
    r = run(root, exe);
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /SKIPPED\. This exact tree already passed the full suite/);
    assert.strictEqual(runs(root).length, before, "a refused run runs nothing");
    // An untracked file changes the tree: it runs.
    fs.writeFileSync(path.join(root, "new.txt"), "x");
    r = run(root, exe);
    assert.doesNotMatch(r.out, /SKIPPED/);
    assert.ok(runs(root).length > before, "a changed tree runs");
    // An edit to a tracked file, too.
    const n = runs(root).length;
    fs.writeFileSync(path.join(root, "a.txt"), "two\n");
    r = run(root, exe);
    assert.doesNotMatch(r.out, /SKIPPED/);
    assert.ok(runs(root).length > n);
    // Unchanged again: refused; --force runs it.
    assert.match(run(root, exe).out, /SKIPPED/);
    const m = runs(root).length;
    r = run(root, exe, ["--force"]);
    assert.doesNotMatch(r.out, /SKIPPED/);
    assert.ok(runs(root).length > m);
    // A filtered run is never refused (it is not the full suite).
    const k = runs(root).length;
    r = run(root, exe, ["always_ok"]);
    assert.doesNotMatch(r.out, /SKIPPED/);
    assert.ok(runs(root).length > k);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("a test that fails once and passes alone is FLAKY: banner, ledger line, the run passes", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const r = run(root, exe, [], { TINY_FLAKE_MARKER: `${root}.flake.marker` });
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /FLAKY: 1 test\(s\) failed in this run and PASSED when run again alone:\s+!!\s+flaky_once/);
    const ledger = fs.readFileSync(path.join(root, "data", "coordination", "flaky-tests.jsonl"), "utf8").trim().split("\n").map(JSON.parse);
    assert.strictEqual(ledger.length, 1);
    assert.strictEqual(ledger[0].test, "flaky_once");
    assert.match(ledger[0].first_failure, /made-up flake/);
    // The rerun ran ONLY that test, by its exact name.
    const after = runs(root);
    assert.strictEqual(after.filter((x) => x === "flaky_once").length, 2);
    assert.strictEqual(after.filter((x) => x === "always_ok").length, 1);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("a real failure still fails (it fails again alone), and nothing is recorded as a pass", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const r = run(root, exe, [], { TINY_REAL_FAIL: "1" });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.match(r.out, /REAL FAILURE: 1 test\(s\) failed and failed again alone:\s+always_fails/);
    assert.match(r.out, /2 \+ 2 is not 5/, "the failure's message is shown even in quiet mode");
    assert.ok(!fs.existsSync(path.join(root, "target", "lib-tests", "last-full-pass.json")));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("quiet by default, one line per test with --verbose", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const q = run(root, exe, ["always_ok"]);
    assert.strictEqual(q.status, 0);
    assert.doesNotMatch(q.out, /test always_ok \.\.\. ok/);
    const v = run(root, exe, ["always_ok", "--verbose"]);
    assert.match(v.out, /test always_ok \.\.\. ok/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("what counts as the full suite, and the failures list", () => {
  assert.strictEqual(L.isFullRun([]), true);
  assert.strictEqual(L.isFullRun(["--nocapture", "-q", "--test-threads", "4"]), true);
  assert.strictEqual(L.isFullRun(["net::webrtc"]), false);
  assert.strictEqual(L.isFullRun(["--ignored"]), false);
  assert.strictEqual(L.isFullRun(["--skip", "slow"]), false);
  assert.strictEqual(L.isFullRun(["--test-threads", "1"]), true, "a flag's value is not a filter");
  const out = "running 3 tests\n..F\nfailures:\n\n---- a::b stdout ----\nboom\n\nfailures:\n    a::b\n    c::d\n\ntest result: FAILED. 1 passed; 2 failed\n";
  assert.deepStrictEqual(L.parseFailures(out), ["a::b", "c::d"]);
  assert.deepStrictEqual(L.parseFailures("test result: ok. 3 passed"), []);
  assert.deepStrictEqual(L.parsePorcelainZ(" M a.txt\0?? b/c.txt\0R  new.txt\0old.txt\0"), ["a.txt", "b/c.txt", "new.txt"]);
});
