// The lib-test run policy (scripts/lib-tests.js, scripts/lib/test-pass-ledger.js, 2026-10-23).
//
// What would make it a check that cannot fail: a refusal that also refuses a CHANGED tree (so a
// red edit "passes" by never running), a flake rerun that swallows a real failure, or a quiet
// mode that hides failures. Each is tried on a throwaway repository with a tiny real test
// program (fixtures/lib-tests/tiny_suite.rs) run in place of the library's
// (HOS_LIB_TESTS_ROOT, HOS_LIB_TESTS_EXE). The operator's GPU game is HOS_LIB_TESTS_GPU_APPS, "" (none)
// unless a test says otherwise, so no result depends on what he happens to be playing.

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
    env: { ...process.env, HOS_LIB_TESTS_ROOT: root, HOS_LIB_TESTS_EXE: exe, TINY_RUN_LOG: runLog(root), HOS_LIB_TESTS_GPU_APPS: "", ...env },
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

test("beside the operator's GPU game the GPU tests stand down, are named, and run alone once it closes", () => {
  const exe = suiteExe();
  const root = repo();
  const passFile = path.join(root, "target", "lib-tests", "last-full-pass.json");
  const gpuRuns = () => runs(root).filter((x) => x === "gpu_device_test").length;
  try {
    // 1. His game runs: the GPU test does nothing, the run passes, and says so loudly.
    let r = run(root, exe, [], { HOS_LIB_TESTS_GPU_APPS: "helldivers2.exe" });
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /helldivers2\.exe is running: every test that opens a GPU device stands down/);
    assert.match(r.out, /GPU TESTS NOT RUN: 1 test\(s\) open a GPU device and stood down, because helldivers2\.exe is running/);
    assert.match(r.out, /!!\s+gpu_device_test/);
    assert.strictEqual(gpuRuns(), 0, "it stood down");
    let rec = JSON.parse(fs.readFileSync(passFile, "utf8"));
    assert.deepStrictEqual(rec.gpu_skipped, ["gpu_device_test"], "recorded as a pass WITHOUT it");
    assert.strictEqual(rec.gpu_busy, "helldivers2.exe");
    // 2. Same tree, game still running: nothing runs, and the reason is given.
    const before = runs(root).length;
    r = run(root, exe, [], { HOS_LIB_TESTS_GPU_APPS: "helldivers2.exe" });
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /SKIPPED\. This exact tree already passed the full suite, except 1 test\(s\) that open a GPU device/);
    assert.strictEqual(runs(root).length, before, "nothing ran");
    // 3. Same tree, game closed: ONLY the GPU test runs, and now the full pass is recorded.
    r = run(root, exe);
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /running just those now/);
    assert.strictEqual(gpuRuns(), 1, "the GPU test ran");
    assert.strictEqual(runs(root).length, before + 1, "and nothing else did");
    rec = JSON.parse(fs.readFileSync(passFile, "utf8"));
    assert.strictEqual(rec.gpu_skipped, undefined, "a full pass now");
    // 4. And then the plain refusal.
    r = run(root, exe);
    assert.match(r.out, /SKIPPED\. This exact tree already passed the full suite at/);
    assert.doesNotMatch(r.out, /except/);
    // --gpu-anyway runs it beside the game.
    r = run(root, exe, ["gpu_device_test", "--gpu-anyway"], { HOS_LIB_TESTS_GPU_APPS: "helldivers2.exe" });
    assert.strictEqual(r.status, 0, r.out);
    assert.strictEqual(gpuRuns(), 2);
    assert.doesNotMatch(r.out, /GPU TESTS NOT RUN/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("a run that goes silent (a thread that reports, then cannot exit) is stopped and names the test", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const t0 = Date.now();
    const r = run(root, exe, ["--stall-min", "0.05"], { TINY_SILENT_HANG: "1" });
    const secs = (Date.now() - t0) / 1000;
    assert.notStrictEqual(r.status, 0, r.out);
    assert.ok(secs < 45, `stopped within seconds, not left to hang (${secs} s)`);
    assert.match(r.out, /FAILED: the test program printed nothing for 0\.05 minutes, so it was stopped: a hung test/);
    assert.match(r.out, /the hung test is one of these \d+:[\s\S]*\n  hangs_silently_when_asked/);
    assert.ok(!fs.existsSync(path.join(root, "target", "lib-tests", "last-full-pass.json")), "a stopped run is not a pass");
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

// THE WORKSPACE (2026-11-01, the crate split's rung 1): one test program per member. What would make
// it a check that cannot fail: a run that takes "the" program and runs one member's tests, a summary
// that reports one program's count as the whole, or a failure rerun in the wrong program (where its
// name finds nothing, which reads as "it passed alone"). Two copies of the tiny suite stand in for
// two members' programs.
const P = require("../lib/lib-test-programs.js");

function twoPrograms(dir) {
  const exe = suiteExe();
  const a = path.join(dir, "first-member.exe");
  const b = path.join(dir, "second-member.exe");
  fs.copyFileSync(exe, a);
  fs.copyFileSync(exe, b);
  return [a, b].join(path.delimiter);
}

test("every member's program runs; the counts are added up; the pass is recorded once", () => {
  const root = repo();
  const bin = fs.mkdtempSync(path.join(os.tmpdir(), "hum_lib_tests_bin_"));
  try {
    const r = run(root, twoPrograms(bin));
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /lib-tests: program 1 \(first-member\.exe\)/);
    assert.match(r.out, /lib-tests: program 2 \(second-member\.exe\)/);
    assert.strictEqual(runs(root).filter((x) => x === "always_ok").length, 2, "both programs ran");
    // The tiny suite has five tests, and all five pass in each program: the line adds them up.
    assert.match(r.out, /lib-tests: test result: ok\. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out \(program 1 5, program 2 5\)/);
    const rec = JSON.parse(fs.readFileSync(path.join(root, "target", "lib-tests", "last-full-pass.json"), "utf8"));
    assert.match(rec.result, /10 passed.*\(program 1 5, program 2 5\)/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(bin, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("a failure in one member's program is rerun in that program, and fails the run", () => {
  const root = repo();
  const bin = fs.mkdtempSync(path.join(os.tmpdir(), "hum_lib_tests_bin_"));
  try {
    const r = run(root, twoPrograms(bin), [], { TINY_REAL_FAIL_IN: "second-member" });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.match(r.out, /lib-tests: again, alone: always_fails \(program 2\)/);
    assert.match(r.out, /REAL FAILURE: 1 test\(s\) failed and failed again alone:\s+always_fails/);
    // Once in each program, then once more alone, in the second.
    assert.strictEqual(runs(root).filter((x) => x === "always_fails").length, 3);
    assert.ok(!fs.existsSync(path.join(root, "target", "lib-tests", "last-full-pass.json")));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(bin, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("the workspace's members, each package's test program, and one summary line for several", () => {
  const meta = {
    workspace_members: ["path+file:///r#humanity-engine@0.1.0", "path+file:///r/crates/humanity-core#0.1.0", "path+file:///r/tools#tool@0.1.0"],
    packages: [
      { id: "path+file:///r#humanity-engine@0.1.0", name: "humanity-engine", targets: [{ kind: ["lib"] }, { kind: ["bin"] }] },
      { id: "path+file:///r/crates/humanity-core#0.1.0", name: "humanity-core", targets: [{ kind: ["lib"] }] },
      { id: "path+file:///r/tools#tool@0.1.0", name: "tool", targets: [{ kind: ["bin"] }] },
    ],
  };
  assert.deepStrictEqual(P.membersOf(meta), ["humanity-engine", "humanity-core"], "a member with no library has no lib tests");
  assert.strictEqual(P.packageName("path+file:///C:/x/crates/humanity-core#0.1547.0"), "humanity-core");
  assert.strictEqual(P.packageName("path+file:///C:/x#humanity-engine@0.1547.0"), "humanity-engine");
  assert.strictEqual(P.packageName("humanity-protocol 0.1547.0 (path+file:///C:/x/crates/humanity-protocol)"), "humanity-protocol");
  const art = (id, exe, kind = ["lib"], test = true) => JSON.stringify({ reason: "compiler-artifact", package_id: id, target: { kind }, profile: { test }, executable: exe });
  const { exes, other } = P.testPrograms([
    art("path+file:///r/crates/humanity-core#0.1.0", "core.exe"),
    art("path+file:///r#humanity-engine@0.1.0", "engine.exe"),
    art("path+file:///r#humanity-engine@0.1.0", "HumanityOS.exe", ["bin"], false),
    art("path+file:///r#humanity-engine@0.1.0", "it.exe", ["test"]),
    "[build-slot 22:00:00] humanity_core (core-test-native): room for 3.0 GB; compiling.",
  ]);
  assert.deepStrictEqual([...exes], [["humanity-core", "core.exe"], ["humanity-engine", "engine.exe"]], "only libraries' test programs");
  assert.deepStrictEqual(other, ["[build-slot 22:00:00] humanity_core (core-test-native): room for 3.0 GB; compiling."]);
  assert.strictEqual(
    P.combinedSummary([
      { name: "humanity-core", summary: "test result: ok. 10 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.00s" },
      { name: "humanity-engine", summary: "test result: FAILED. 20 passed; 2 failed; 3 ignored; 0 measured; 0 filtered out; finished in 9.00s" },
    ]),
    "test result: FAILED. 30 passed; 2 failed; 4 ignored; 0 measured; 0 filtered out (humanity-core 10, humanity-engine 20)",
  );
  assert.strictEqual(P.combinedSummary([{ name: "a", summary: "test result: ok. 1 passed" }]), "test result: ok. 1 passed");
  assert.match(P.combinedSummary([{ name: "a", summary: "test result: ok. 1 passed" }, { name: "b", summary: null }]), /^test result: FAILED\..*b: no result line/);
});
