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
    // Since 2026-11-21 a line says it was proven: the same files and program, and the rerun named it ok.
    assert.strictEqual(ledger[0].same_inputs, true);
    // The rerun ran ONLY that test, by its exact name.
    const after = runs(root);
    assert.strictEqual(after.filter((x) => x === "flaky_once").length, 2);
    assert.strictEqual(after.filter((x) => x === "always_ok").length, 1);
    // The test program's own -q prints no names; the rerun's proof needs them, so the rerun drops
    // -q (2026-11-21). Seen red while writing the proof: "NOT PROVEN: ... the rerun ran no test it
    // named, not flaky_once", a true flake failing the run.
    fs.rmSync(`${root}.flake.marker`, { force: true });
    const q = run(root, exe, ["-q", "--force"], { TINY_FLAKE_MARKER: `${root}.flake.marker` });
    assert.strictEqual(q.status, 0, q.out);
    assert.match(q.out, /FLAKY: 1 test\(s\) failed in this run and PASSED when run again alone/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
    fs.rmSync(`${root}.flake.marker`, { force: true });
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

// THE RERUN'S PROOF (2026-11-21). On 2026-11-20 i18n::tests::every_placeholder_is_double_braced
// failed on a single-brace placeholder in data/i18n/en.json, the word was corrected while the suite
// was still running, the rerun alone read the corrected file and passed, and lib-tests logged a
// deterministic failure FLAKY (passed_alone: true) and exited 0. Reproduced the same day in a
// worktree against the real core program (the plant corrected the moment its F mark printed):
// "FLAKY: 1 test(s) failed in this run and PASSED when run again alone", exit 0.
// RED before the fix, both of the next two tests: the first exited 0 with the FLAKY banner and a
// ledger line; the second (the program relinked between run and rerun) the same.
const ledgerFile = (root) => path.join(root, "data", "coordination", "flaky-tests.jsonl");

test("a failure whose input was corrected while the run went on is not a flake: the run fails, no ledger line", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const words = path.join(root, "words.json");
    fs.writeFileSync(words, "Hello {name}\n");
    const r = run(root, exe, [], { TINY_INPUT: words, TINY_FIX_INPUT: "1" });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.doesNotMatch(r.out, /FLAKY:/);
    assert.match(r.out, /NOT PROVEN A FLAKE: 1 test\(s\) failed in the run and passed alone, but what they ran on changed between the two:/);
    assert.match(r.out, /\n!!\s+reads_its_input\n/);
    assert.match(r.out, /\n!!\s+words\.json \(changed\)/);
    assert.ok(!fs.existsSync(ledgerFile(root)), "nothing is written to the flaky ledger");
    assert.ok(!fs.existsSync(path.join(root, "target", "lib-tests", "last-full-pass.json")), "and nothing is recorded as a pass");
    // The same failure left alone is what it always was: a REAL FAILURE.
    fs.writeFileSync(words, "Hello {name}\n");
    const r2 = run(root, exe, [], { TINY_INPUT: words });
    assert.notStrictEqual(r2.status, 0, r2.out);
    assert.match(r2.out, /REAL FAILURE: 1 test\(s\) failed and failed again alone:\s+reads_its_input/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("a test program relinked between the run and the rerun: a pass alone proves nothing", () => {
  const root = repo();
  const bin = fs.mkdtempSync(path.join(os.tmpdir(), "hum_lib_tests_bin_"));
  try {
    const exes = twoPrograms(bin);
    const first = exes.split(path.delimiter)[0];
    // flaky_once fails in the first program and passes after; the second program's run touches the
    // first one's file, as another cargo command relinking it would.
    const r = run(root, exes, [], { TINY_FLAKE_MARKER: `${root}.flake.marker`, TINY_TOUCH: first, TINY_TOUCH_IN: "second-member" });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.doesNotMatch(r.out, /FLAKY:/);
    assert.match(r.out, /NOT PROVEN A FLAKE/);
    assert.match(r.out, /first-member\.exe \(the test program was rebuilt/);
    assert.ok(!fs.existsSync(ledgerFile(root)));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(bin, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
    fs.rmSync(`${root}.flake.marker`, { force: true });
  }
});

// Point 9 (2026-11-21): the tests read the checkout as they run, so a full pass across an edit is a
// pass of neither version and is not recorded. RED with the check removed (recordPass without its
// changedSinceStart): the pass was recorded against the tree as it was BEFORE the edit, so undoing
// the edit would have had that version refused as "already passed" though no run ever tested it whole.
test("a full pass during which the checkout changed is not recorded, so the next run runs", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const r = run(root, exe, [], { TINY_APPEND: path.join(root, "notes.txt") });
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /the suite passed, but the checkout changed while it ran, so the pass is NOT recorded/);
    assert.match(r.out, /\n  notes\.txt \(changed\)/);
    assert.ok(!fs.existsSync(path.join(root, "target", "lib-tests", "last-full-pass.json")));
    const again = run(root, exe);
    assert.doesNotMatch(again.out, /SKIPPED/, "the edited tree was never passed as a whole: it runs");
    assert.strictEqual(again.status, 0, again.out);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

// A filter that matches nothing (reported 2026-11-21: `node scripts/lib-tests.js "a|b"` ran no test
// and exited 0). RED before the fix: every run here but --list exited 0.
test("a filter that matches no test fails, in words; one matching only ignored tests says --ignored", () => {
  const exe = suiteExe();
  const root = repo();
  const bin = fs.mkdtempSync(path.join(os.tmpdir(), "hum_lib_tests_bin_"));
  try {
    let r = run(root, exe, ["a|b"]);
    assert.notStrictEqual(r.status, 0, r.out);
    assert.match(r.out, /NOTHING RAN: no test matched "a\|b", so this is not a pass/);
    assert.match(r.out, /plain piece of a test's name, not a pattern/);
    // Across several programs: none matched in any.
    r = run(root, twoPrograms(bin), ["no_such_test_anywhere"]);
    assert.notStrictEqual(r.status, 0, r.out);
    assert.match(r.out, /NOTHING RAN: no test matched "no_such_test_anywhere" in any of the 2 test programs/);
    // Matching only an ignored test runs nothing either, and says how to run it.
    r = run(root, exe, ["ignored_unless_asked"]);
    assert.notStrictEqual(r.status, 0, r.out);
    assert.match(r.out, /NOTHING RAN: "ignored_unless_asked" matched 1 test\(s\), every one #\[ignore\]d/);
    assert.match(r.out, /--ignored/);
    r = run(root, exe, ["ignored_unless_asked", "--ignored"]);
    assert.strictEqual(r.status, 0, r.out);
    assert.ok(runs(root).includes("ignored_unless_asked"), "with --ignored it runs");
    // Several filters, one of which matches: a run.
    r = run(root, exe, ["always_ok", "a|b"]);
    assert.strictEqual(r.status, 0, r.out);
    // --list runs no test by design: not refused.
    r = run(root, exe, ["--list"]);
    assert.strictEqual(r.status, 0, r.out);
    assert.match(r.out, /always_ok: test/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(bin, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

test("a rerun that never reports the test (the program ended with exit 0) is not a pass alone", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    // RED against the check before 2026-11-21 (exit 0 and "running 1 test"): with the proof put back
    // to that, this run exited 0 with "FLAKY: 1 test(s) failed in this run and PASSED when run again
    // alone: exits_the_program_once_it_has_failed" and a ledger line.
    const r = run(root, exe, [], { TINY_EXIT_MARKER: `${root}.exit.marker` });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.doesNotMatch(r.out, /FLAKY:/);
    assert.match(r.out, /NOT PROVEN: 1 test\(s\) failed, and the rerun alone exited 0 without proving it ran them:\s+exits_the_program_once_it_has_failed: the rerun printed no result line/);
    assert.ok(!fs.existsSync(ledgerFile(root)));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
    fs.rmSync(`${root}.exit.marker`, { force: true });
  }
});

test("a rerun's pass is proven by the test's own ok line and a count of exactly one", () => {
  const ok = "\nrunning 1 test\ntest a::b ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s\n";
  assert.deepStrictEqual(L.passedAlone(ok, "a::b"), { ok: true });
  assert.deepStrictEqual(L.passedAlone(ok.replace("test a::b", "printed by the test, no newline test a::b"), "a::b"), { ok: true }, "--nocapture output before the line");
  // --test-threads=1 --nocapture: the name before the test runs, its output, then "ok" on its own line.
  assert.deepStrictEqual(L.passedAlone(ok.replace("test a::b ... ok", "test a::b ... rendering the page\nok"), "a::b"), { ok: true }, "the result after the test's own output");
  // Each of these satisfied the check this replaced (exit 0 and /running 1 test/): RED for it.
  const other = ok.replace("test a::b ... ok", "test a::c ... ok");
  const ignored = "\nrunning 1 test\ntest a::b ... ignored\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 9 filtered out\n";
  for (const [out, why] of [
    [other, /ran a::c, not a::b/],
    [ignored, /"ignored"/],
  ]) {
    assert.match(out, /running 1 test/);
    const p = L.passedAlone(out, "a::b");
    assert.strictEqual(p.ok, false, out);
    assert.match(p.why, why);
  }
  const none = "\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out\n";
  assert.match(L.passedAlone(none, "a::b").why, /matched no test of that name \(10 filtered out\)/);
  assert.match(L.passedAlone("running 1 test\n", "a::b").why, /no result line/);
  // What changed between two identities, in words.
  const a = { hash: "1", head_tree: "t1", rustc: "r", dirty: [["x.txt", "h1"], ["gone.txt", "h2"]] };
  const b = { hash: "2", head_tree: "t1", rustc: "r", dirty: [["x.txt", "h9"], ["new.txt", "deleted"]] };
  assert.deepStrictEqual(L.identityDiff(a, a), []);
  assert.deepStrictEqual(L.identityDiff(a, b), ["x.txt (changed)", "new.txt (deleted)", "gone.txt (back to the commit's version)"]);
  assert.match(L.identityDiff(a, { ...b, head_tree: "t2" })[0], /the commit moved/);
  assert.match(L.identityDiff(a, null)[0], /could not be read/);
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
    // Point 7 (2026-11-14): every thread's stack, dumped before it was stopped, saved beside the
    // run's log, and the stuck test's thread printed by name with its frames. (The cached suite's
    // exe is moved away from its debug file, so its frames are addresses here; that the frames
    // name functions when the PDB is beside the program, as cargo leaves it, is
    // scripts/tests/stack-dump.test.js.) RED before point 7: no "Thread ... when it was stopped"
    // line and no stacks file; the report could only name the tests in flight.
    if (process.platform === "win32" && process.arch === "x64") {
      assert.match(r.out, /Thread "hangs_silently_when_asked" when it was stopped \(innermost first\):\n(    \S+\n){3,}/);
      const stacks = /Every thread's stack when it was stopped: (.*\.stacks\.txt)/.exec(r.out);
      const log = /The run's output up to then: (.*\.log)/.exec(r.out);
      assert.ok(stacks && log, r.out);
      assert.strictEqual(path.dirname(stacks[1]), path.dirname(log[1]), "the dump is saved beside the log");
      assert.match(fs.readFileSync(stacks[1], "utf8"), /^thread \d+ "hangs_silently_when_asked":$/m);
      assert.match(fs.readFileSync(log[1], "utf8"), /running \d+ tests?/);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

// 2026-11-14: the GPU helper (src/renderer/test_gpu.rs) keeps the GPU turn's holder in the file
// HUMANITY_TEST_GPU_TURN names and writes its watchdog lines straight to stderr; a stopped run says
// both. Seen red against the runner before point 6: neither line was printed.
test("a stopped run names the test holding the GPU turn and repeats the watchdog's line", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const r = run(root, exe, ["--stall-min", "0.05"], { TINY_SILENT_HANG: "1", TINY_GPU_TURN_HOLDER: "renderer::x::tests::stuck" });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.match(r.out, /The GPU turn was HELD by renderer::x::tests::stuck/);
    assert.match(r.out, /\n  GPU TEST STILL RUNNING: renderer::x::tests::stuck has held the GPU for 60 s/);
    // Nothing of libtest's is glued to it, in whatever order the two streams arrived (the next test
    // makes the glued order on purpose).
    assert.doesNotMatch(r.out, /\.\.\. *GPU TEST/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

// 2026-11-21: run one test at a time, libtest writes "test hangs_silently_when_asked ... " (flushed)
// as the test STARTS, and the test then writes the watchdog's line to stderr and hangs: the run ends
// on a half-written libtest line, followed in the log by the watchdog's. The report keeps that half
// line as what it is, the test that was running, on its own line, and the watchdog line on its own.
// The half line is written before the test's thread even starts, so this glued order is made by the
// program, not by timing. Seen red against the runner before point 10, three runs of three: "The
// input was expected to not match the regular expression /\.\.\. *GPU TEST/", the report repeating
// "  test hangs_silently_when_asked ... GPU TEST STILL RUNNING: renderer::x::tests::stuck has held
// the GPU for 60 s (src/renderer/test_gpu.rs)" as one line, and no line naming the running test.
test("run one at a time, the half-written line names the running test, and the watchdog line stands alone", () => {
  const exe = suiteExe();
  const root = repo();
  try {
    const r = run(root, exe, ["--stall-min", "0.05", "--test-threads", "1"], { TINY_SILENT_HANG: "1", TINY_GPU_TURN_HOLDER: "renderer::x::tests::stuck" });
    assert.notStrictEqual(r.status, 0, r.out);
    assert.doesNotMatch(r.out, /\.\.\. *GPU TEST/);
    assert.match(r.out, /\n  GPU TEST STILL RUNNING: renderer::x::tests::stuck has held the GPU for 60 s/);
    assert.match(r.out, /it had written "test hangs_silently_when_asked \.\.\. " and never its result, so hangs_silently_when_asked was running when it was stopped/);
    assert.match(r.out, /the hung test is one of these 1:\n  hangs_silently_when_asked\n/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(runLog(root), { force: true });
  }
});

// 2026-11-21: the glued hang report, made on purpose and in order, no timing involved. Under machine
// load the watchdog's line (stderr) arrived between libtest's two writes of one result line (stdout:
// "test ignored_unless_asked ... ", then "ignored\n"), and the report, reading both streams as one
// text, repeated "  test ignored_unless_asked ... GPU TEST STILL RUNNING: ..." as one line, so the
// test above failed under load and passed alone. GLUED is that arrival order, chunk by chunk (a
// probe of the tiny suite the same day saw libtest's line split across two reads exactly so:
// "test flaky_once ... " in one, "ok\n" in the next). scripts/lib/test-output.js, point 10.
// Seen red against the report as it was (both streams one text, split into lines, each whole line
// repeated): "Expected values to be strictly deep-equal: + 'test ignored_unless_asked ... GPU TEST
// STILL RUNNING: renderer::x::tests::stuck has held the GPU for 60 s (src/renderer/test_gpu.rs)'
// - 'GPU TEST STILL RUNNING: renderer::x::tests::stuck has held the GPU for 60 s (...)'".
const GLUED = [
  ["stdout", "\nrunning 9 tests\ntest gpu_device_test ... ok\ntest ignored_unless_asked ... "],
  ["stderr", "GPU TEST STILL RUNNING: renderer::x::tests::stuck has held the GPU for 60 s (src/renderer/test_gpu.rs)\n"],
  ["stdout", "ignored\n"],
];

test("a watchdog line landing inside libtest's half-printed line is repeated whole, and libtest's line still reads", () => {
  const O = require("../lib/test-output.js");
  const lines = [];
  const c = O.collector((l) => lines.push(l));
  for (const [stream, chunk] of GLUED) c.add(stream, chunk);
  assert.strictEqual(c.end(), "", "nothing was left half-printed");
  const watchdog = "GPU TEST STILL RUNNING: renderer::x::tests::stuck has held the GPU for 60 s (src/renderer/test_gpu.rs)";
  assert.deepStrictEqual(O.watchdogLines(c), [watchdog]);
  assert.ok(lines.includes("test ignored_unless_asked ... ignored"), `libtest's line, read from stdout alone, is whole: ${JSON.stringify(lines)}`);
  assert.match(c.out, /test ignored_unless_asked \.\.\. GPU TEST STILL RUNNING/, "the log keeps the order things arrived in");
  // A watchdog line glued to text on stderr itself (a test's own eprint! with no newline, under
  // --nocapture) still starts at its marker; repeated lines are said once.
  assert.deepStrictEqual(O.watchdogLines({ stderr: `half a note ${watchdog}\n${watchdog}\n` }), [watchdog]);
  // The line libtest began and never finished, read from stdout's end.
  assert.strictEqual(O.halfLine({ stdout: "running 2 tests\ntest a ... ok\ntest hangs ... " }), "hangs");
  assert.strictEqual(O.halfLine({ stdout: "running 2 tests\ntest a ... ok\n" }), null);
  assert.strictEqual(O.halfLine({ stdout: "test a ... ok" }), null, "a finished line with no newline yet is not half a line");
  // What a half line means: one thread, the running test, first; several, a test that had ENDED
  // (libtest names a test only after its thread exits), so never a suspect. Seen red with the
  // several-thread case's half line ignored (`ended` always null): inFlight [ 'b', 'c' ] and
  // finished 1 where [ 'c', 'd' ] and 2 were expected, the finished b named as a suspect.
  const list = ["a", "b", "c", "d", "e"];
  const done = new Set(["a"]);
  assert.deepStrictEqual(O.suspects({ list, done, threads: 1, half: "b" }), { running: "b", inFlight: ["b"], finished: 1 });
  assert.deepStrictEqual(O.suspects({ list, done, threads: 2, half: "b" }), { running: null, inFlight: ["c", "d"], finished: 2 });
  assert.deepStrictEqual(O.suspects({ list, done, threads: 2, half: null }), { running: null, inFlight: ["b", "c"], finished: 1 });
  assert.deepStrictEqual(O.suspects({ list: null, done, threads: 1, half: "b" }), { running: "b", inFlight: null, finished: null });
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
    // The tiny suite has eight tests that run and one ignored, and all eight pass in each program:
    // the line adds them up.
    assert.match(r.out, /lib-tests: test result: ok\. 16 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out \(program 1 8, program 2 8\)/);
    const rec = JSON.parse(fs.readFileSync(path.join(root, "target", "lib-tests", "last-full-pass.json"), "utf8"));
    assert.match(rec.result, /16 passed.*\(program 1 8, program 2 8\)/);
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
  const withPaths = {
    ...meta,
    packages: meta.packages.map((p) => ({ ...p, manifest_path: `/r${p.name === "humanity-engine" ? "" : p.name === "tool" ? "/tools" : "/crates/humanity-core"}/Cargo.toml` })),
  };
  const dirs = P.memberDirs(withPaths);
  assert.deepStrictEqual([...dirs.keys()], ["humanity-engine", "humanity-core"], "each member with a library has a folder, and only those");
  assert.strictEqual(dirs.get("humanity-engine").replace(/\\/g, "/"), "/r", "the root package runs from the repo root");
  assert.strictEqual(dirs.get("humanity-core").replace(/\\/g, "/"), "/r/crates/humanity-core", "a crate's program runs from its own folder, as cargo test runs it");
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
