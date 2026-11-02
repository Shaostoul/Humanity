#!/usr/bin/env node
// Run the library's unit tests: `cargo test --features native --lib`, except on
// Windows, where the test program is linked WITHOUT a debug file (BUG-174).
//
// Usage:  node scripts/lib-tests.js [lib-tests options] [test-filter and test-program options]
//         just test-lib [same]
// e.g.    node scripts/lib-tests.js net::webrtc --nocapture
// Everything that is not one of lib-tests' own options goes to the test program itself, the
// way the words after `--` do for `cargo test`: a name filter, --nocapture, --exact.
//
// lib-tests' own options (2026-10-23):
//   --force                run the full suite even though this exact tree already passed it
//   --verbose              the test program's full output (one line per test); default is one
//                          mark per test, the way -q prints
//   --run-timeout-min N    fail if the test RUN (not the build) takes longer than N minutes
//   --stall-min N          fail if the test program prints nothing for N minutes (default 15,
//                          0 = never), naming the tests that were in flight (2026-11-01)
//   --gpu-anyway           let the tests that open a GPU device run beside the operator's GPU
//                          game (by default they stand down: point 4 below; beside it they may hang)
//
// WHY (2026-10-09): Microsoft's linker refuses to write the library test
// program's debug file (PDB) with "LNK1318: Unexpected PDB error; LIMIT (12)",
// which stopped `just verify` on Windows (Linux CI has no PDB and was never
// affected; builds of 2026-10-05 still linked). Three things were measured that
// day and did NOT help: `strip = "debuginfo"` (on MSVC rustc passes /DEBUG
// whatever strip says), line-tables-only debug info everywhere (the PDB fell
// from 1.08 GB to 487 MB and the link still failed, so it is not the file's
// size), and the object count (about 2,300 objects, far under any module cap).
// Linking with /DEBUG:NONE works, and `cargo rustc` can pass that to this one
// link without changing the build of any dependency, so nothing else rebuilds.
// The cost: a panicking test's stack trace shows addresses instead of function
// names. A failing assertion still names its file and line (that comes from the
// panic location, not the debug file). Release builds keep their PDB.
//
// Use this, not a bare `cargo test --features native --lib`, on Windows: the
// bare form links with /DEBUG again and fails the same way.
//
// THE TEST-RUN POLICY (2026-10-23, the build-speed review; CLAUDE.md: "NEVER re-run a test
// battery that already passed on unchanged code"). A rule only a reader follows, now a
// mechanism:
//   1. A full run (no name filter, nothing that selects tests) that passes is recorded in
//      target/lib-tests/last-full-pass.json against the identity of the whole checkout
//      (scripts/lib/test-pass-ledger.js: the commit's tree, every changed or untracked file's
//      content, rustc -V, the features). A full run asked for on that same identity is
//      REFUSED, exit 0 (it already passed), with the record. --force runs it anyway.
//   2. One character per test on the screen: a full run's log was 460 KB of "test x ... ok"
//      lines and is a few KB now. Failures print in full either way. (Since 2026-11-01 the test
//      program itself prints a line per test and this script turns each into its mark, so that
//      it knows which tests finished: point 5.)
//   3. When one to five tests fail, each is run once more, alone, by its exact name. One that
//      passes alone is a timing flake: a loud FLAKY banner, a line in
//      data/coordination/flaky-tests.jsonl (which test, when, the first failure's message), and
//      the run counts as passed, so a flake does not cost a whole rerun. One that fails again is
//      a real failure, and the run fails. More than five failures are not timing: no reruns.
//
// THE OPERATOR'S GPU GAME (2026-11-01). On that day the full run froze at 3,852 of 5,584 for two
// hours while he played helldivers2.exe, burning CPU, without one "has been running for over 60
// seconds" line, and passed in 232 s once the game closed. The tests that open a GPU device were
// running at that point (renderer::scene_target's round trips run in every full run).
//   4. Before the run this script asks scripts/lib/machine-guard.js which of his GPU-heavy
//      programs run (scripts/lib/operator-apps.json, and his own HumanityOS). When one does, the
//      test program gets HUMANITY_TEST_GPU_BUSY, and every test that would open a device asks
//      src/test_gpu.rs first and stands down, writing its name to target/lib-tests/gpu-skipped.txt
//      (tests/gpu_test_gate_lint.rs holds every such test to asking). libtest counts a test that
//      returned early as passed, so this script says loudly which did nothing and why, and
//      records the pass WITHOUT them: asked again on the same tree once the game has closed, it
//      runs just those tests, and only then is the full pass recorded. --gpu-anyway skips this.
//   5. The silence was libtest's: its main loop joins each finished test's thread BEFORE it
//      prints that test's result, and its 60-second warnings come from the same loop, so a test
//      thread that reports and then cannot exit stops every line after it. So a run that prints
//      nothing for --stall-min minutes (15 by default) is stopped and fails, naming the tests in
//      flight: the first unfinished ones in name order, since libtest starts them in that order.

"use strict";
const fs = require("fs");
const path = require("path");
const { spawn, spawnSync } = require("child_process");
const L = require("./lib/test-pass-ledger.js");

// HOS_LIB_TESTS_ROOT and HOS_LIB_TESTS_EXE are for scripts/tests/lib-tests-policy.test.js: another
// checkout (a throwaway repository), and a test program to run instead of building ours.
const ROOT = process.env.HOS_LIB_TESTS_ROOT ? path.resolve(process.env.HOS_LIB_TESTS_ROOT) : path.join(__dirname, "..");
const GIVEN_EXE = process.env.HOS_LIB_TESTS_EXE || null;
const PASS_FILE = path.join(ROOT, "target", "lib-tests", "last-full-pass.json");
const FLAKY_LEDGER = path.join(ROOT, "data", "coordination", "flaky-tests.jsonl");
const GPU_SKIPPED_FILE = path.join(ROOT, "target", "lib-tests", "gpu-skipped.txt");
const MAX_RERUNS = 5;
const DEFAULT_STALL_MIN = 15;

// Our options out, the test program's in.
const argv = process.argv.slice(2);
const opts = { force: false, verbose: false, runTimeoutMin: null, stallMin: DEFAULT_STALL_MIN, gpuAnyway: false };
const passthrough = [];
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (a === "--force") opts.force = true;
  else if (a === "--verbose") opts.verbose = true;
  else if (a === "--gpu-anyway") opts.gpuAnyway = true;
  else if (a === "--run-timeout-min") {
    opts.runTimeoutMin = Number(argv[++i]);
    if (!(opts.runTimeoutMin > 0)) {
      console.error("lib-tests: --run-timeout-min needs a number of minutes");
      process.exit(2);
    }
  } else if (a === "--stall-min") {
    opts.stallMin = Number(argv[++i]);
    if (!(opts.stallMin >= 0)) {
      console.error("lib-tests: --stall-min needs a number of minutes (0 = never)");
      process.exit(2);
    }
  } else passthrough.push(a);
}
const full = L.isFullRun(passthrough);
// The test program's own -q or --format: its output is shown as it is and not read for names.
const ownFormat = passthrough.some((a) => a === "-q" || a === "--quiet" || a.startsWith("--format"));
const compact = !opts.verbose && !ownFormat;

/** The operator's GPU-heavy programs running now (point 4). HOS_LIB_TESTS_GPU_APPS is for
 *  scripts/tests/lib-tests-policy.test.js: a comma-separated list standing in for the machine's
 *  ("" = none), so a test never depends on what the operator happens to be playing. */
function gpuBusy() {
  if (process.env.HOS_LIB_TESTS_GPU_APPS != null)
    return process.env.HOS_LIB_TESTS_GPU_APPS.split(",").map((s) => s.trim()).filter(Boolean);
  try {
    const MG = require("./lib/machine-guard.js");
    return [...new Set(MG.operatorApps().map((p) => p.name || MG.GAME))];
  } catch (e) {
    console.error(`lib-tests: could not list the operator's programs (${e.message}); the GPU tests run`);
    return [];
  }
}
const busy = opts.gpuAnyway ? [] : gpuBusy();

function fmtTime(iso) {
  const d = new Date(iso);
  return isNaN(d) ? String(iso) : d.toLocaleString("en-GB", { hour12: false });
}

// 1. The refusal: this exact tree already passed the full suite.
let identity = null;
// A pass on this tree that left out the GPU tests (point 4): only they are left to run.
let partialPass = null;
if (full) {
  try {
    identity = L.treeIdentity(ROOT);
  } catch (e) {
    console.error(`lib-tests: could not read the tree's identity (${e.message}); the pass will not be recorded`);
  }
  const last = L.readJson(PASS_FILE);
  if (identity && last && last.identity && last.identity.hash === identity.hash && !opts.force) {
    const changed = identity.dirty.length ? ` + ${identity.dirty.length} changed file(s)` : "";
    const left = Array.isArray(last.gpu_skipped) ? last.gpu_skipped : [];
    const what = left.length ? `the full suite, except ${left.length} test(s) that open a GPU device,` : "the full suite";
    if (!left.length || busy.length) {
      console.log(`lib-tests: SKIPPED. This exact tree already passed ${what} at ${fmtTime(last.at)}:`);
      console.log(`  ${last.result || "(no summary)"}${last.run_s != null ? `, in ${last.run_s} s` : ""}`);
      console.log(`  tree ${identity.head_tree.slice(0, 12)}${changed}, ${identity.rustc}, features ${identity.features}`);
      if (last.flaky && last.flaky.length) console.log(`  (that pass had ${last.flaky.length} flaky test(s): ${last.flaky.join(", ")})`);
      if (left.length) {
        console.log(`Those ${left.length} stood down for ${last.gpu_busy || "a GPU game"}, and ${busy.join(", ")} is running now, so they would stand down again:`);
        for (const n of left) console.log(`  ${n}`);
        console.log("When it has closed, run this again: it runs just those. (--gpu-anyway runs them beside it; they may hang the run.)");
      } else {
        console.log("Running it again on unchanged code proves nothing new (CLAUDE.md). To run it anyway:");
        console.log("  node scripts/lib-tests.js --force        (just test-lib --force)");
      }
      process.exit(0);
    }
    partialPass = last;
    console.log(`lib-tests: this exact tree passed the full suite at ${fmtTime(last.at)} except ${left.length} test(s) that open a GPU device, which stood down for ${last.gpu_busy || "a GPU game"}; running just those now:`);
    for (const n of left) console.log(`  ${n}`);
  }
}
if (busy.length) {
  console.log(`lib-tests: ${busy.join(", ")} is running: every test that opens a GPU device stands down (src/test_gpu.rs), and is named after the run. --gpu-anyway runs them beside it (they may hang the run).`);
}
// What the test program is asked to run: the GPU tests a partial pass left, or what was asked for.
const runArgs = partialPass ? [...partialPass.gpu_skipped, "--exact", ...L.nonSelecting(passthrough)] : passthrough;

// 2. The build slot's wrapper, when this machine has one, must be current before cargo runs it.
if (!GIVEN_EXE) spawnSync(process.execPath, [path.join(__dirname, "build-slot.js"), "ensure"], { cwd: ROOT, stdio: "inherit" });

// 3. Build the test program.
function buildTestProgram() {
  if (GIVEN_EXE) return GIVEN_EXE;
  const args =
    process.platform === "win32"
      ? // The library compiled with --test, linked without a PDB (BUG-174).
        ["rustc", "--features", "native", "--lib", "--profile", "test", "--message-format=json-render-diagnostics", "--", "-C", "link-arg=/DEBUG:NONE"]
      : ["test", "--features", "native", "--lib", "--no-run", "--message-format=json-render-diagnostics"];
  const build = spawnSync("cargo", args, { cwd: ROOT, stdio: ["ignore", "pipe", "inherit"], encoding: "utf8", maxBuffer: 1 << 30 });
  if (build.error) {
    console.error("lib-tests:", build.error.message);
    process.exit(1);
  }
  // Cargo's own messages go to stdout as JSON; the program is the artifact with an executable.
  // Anything else on stdout (the build slot's lines) is passed on to the reader.
  let exe = null;
  for (const line of (build.stdout || "").split("\n")) {
    if (!line.startsWith("{")) {
      if (line.trim()) console.log(line);
      continue;
    }
    try {
      const m = JSON.parse(line);
      if (m.reason === "compiler-artifact" && m.executable && m.profile && m.profile.test) exe = m.executable;
    } catch {
      /* not a cargo message */
    }
  }
  if (build.status !== 0) process.exit(build.status === null ? 1 : build.status);
  if (!exe) {
    console.error("lib-tests: cargo built nothing runnable (no test program in its messages)");
    process.exit(1);
  }
  return exe;
}

// The lines libtest prints per test (its pretty format): a result, and the 60-second warning.
// Not anchored at the start: with --nocapture a test's own output can precede it on the line.
const RESULT_RE = /test (\S+)(?: - should panic)? \.\.\. (ok|FAILED|ignored\b.*)$/;
const WARNED_RE = /test (\S+) has been running for over \d+ seconds/;
const RUNNING_RE = /^running (\d+) tests?$/;

/** The environment the test program gets: the repo root, and the GPU note when his game runs. */
function programEnv() {
  const env = { ...process.env, CARGO_MANIFEST_DIR: ROOT };
  delete env.HUMANITY_TEST_GPU_BUSY;
  delete env.HUMANITY_TEST_GPU_SKIPPED;
  if (busy.length) {
    env.HUMANITY_TEST_GPU_BUSY = busy.join(", ");
    env.HUMANITY_TEST_GPU_SKIPPED = GPU_SKIPPED_FILE;
  }
  return env;
}

/**
 * Run the test program, its output shown as it comes AND kept. When `compact`, each "test x ...
 * ok" line is shown as one mark (. i F), the way -q prints; unless the program was given its own
 * format, every result line is read, so the tests that finished are known (`done`). A program that
 * prints nothing for opts.stallMin minutes is stopped (`stalled`), and so is one running past
 * `timeoutMin` (`timedOut`). Resolves { code, out, timedOut, stalled, secs, done, warned, parsed }.
 */
function runProgram(exe, args, timeoutMin) {
  return new Promise((resolve) => {
    const t0 = Date.now();
    const c = spawn(exe, args, { cwd: ROOT, env: programEnv(), stdio: ["inherit", "pipe", "pipe"] });
    let out = "";
    const keep = (d) => {
      if (out.length < 64 * 1024 * 1024) out += d;
    };
    const parsed = !ownFormat;
    const done = new Set();
    const warned = new Set();
    let total = null;
    let marks = 0;
    let midLine = false; // the screen's last line is a row of marks with no newline yet
    let partial = ""; // stdout since its last newline
    const say = (text) => {
      if (midLine) process.stdout.write("\n");
      midLine = false;
      process.stdout.write(text + "\n");
    };
    const mark = (ch) => {
      process.stdout.write(ch);
      midLine = true;
      marks++;
      if (marks % 88 === 0) {
        process.stdout.write(total ? ` ${marks}/${total}\n` : "\n");
        midLine = false;
      }
    };
    const onLine = (line) => {
      const r = RESULT_RE.exec(line);
      const w = r ? null : WARNED_RE.exec(line);
      const n = r || w ? null : RUNNING_RE.exec(line.trim());
      if (r) done.add(r[1]);
      if (w) warned.add(w[1]);
      if (n) total = Number(n[1]);
      if (!compact) return; // already shown as it came
      if (r) {
        const before = line.slice(0, r.index);
        if (before.trim()) say(before);
        mark(r[2] === "ok" ? "." : r[2] === "FAILED" ? "F" : "i");
      } else say(line);
    };
    // Stopping a program that has gone quiet (point 5), or run too long.
    let stalled = false;
    let timedOut = false;
    let stallTimer = null;
    const stop = () => {
      c.kill();
      // A process that cannot be killed must not hang this script too.
      setTimeout(() => finish(1), 30 * 1000).unref();
    };
    const arm = () => {
      if (!opts.stallMin) return;
      clearTimeout(stallTimer);
      stallTimer = setTimeout(() => {
        stalled = true;
        stop();
      }, opts.stallMin * 60 * 1000);
    };
    arm();
    const timer = timeoutMin
      ? setTimeout(() => {
          timedOut = true;
          stop();
        }, timeoutMin * 60 * 1000)
      : null;
    c.stdout.on("data", (d) => {
      arm();
      keep(d);
      if (!compact) process.stdout.write(d);
      if (!parsed) return;
      const lines = (partial + d).split(/\r?\n/);
      partial = lines.pop();
      for (const l of lines) onLine(l);
    });
    c.stderr.on("data", (d) => {
      arm();
      process.stderr.write(d);
      keep(d);
    });
    let settled = false;
    const finish = (code, extra = "") => {
      if (settled) return;
      settled = true;
      clearTimeout(stallTimer);
      if (timer) clearTimeout(timer);
      if (partial && parsed) onLine(partial);
      if (compact && midLine) process.stdout.write("\n");
      resolve({ code, out: out + extra, timedOut, stalled, secs: Math.round((Date.now() - t0) / 1000), done, warned, parsed });
    };
    c.on("error", (e) => finish(1, String(e)));
    c.on("close", (code) => finish(code === null ? 1 : code));
  });
}

/** The tests a run of `args` selects, in libtest's run order (by name), or null. */
function listTests(exe, args) {
  const keepArgs = [];
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    const flag = a.split("=")[0];
    if (L.SELECTING.has(flag) && flag !== "--list") {
      keepArgs.push(a);
      if (a === "--skip" && i + 1 < args.length) keepArgs.push(args[++i]);
    } else if (!a.startsWith("-")) keepArgs.push(a); // a name filter
    else if ((a === "--test-threads" || a === "--format" || a === "--color" || a === "-Z" || a === "--logfile") && i + 1 < args.length) i++;
  }
  const r = spawnSync(exe, [...keepArgs, "--list", "--format", "terse"], { cwd: ROOT, env: programEnv(), encoding: "utf8", maxBuffer: 1 << 28 });
  if (r.status !== 0) return null;
  return String(r.stdout)
    .split(/\r?\n/)
    .map((l) => /^(.+): test$/.exec(l))
    .filter(Boolean)
    .map((m) => m[1])
    .sort();
}

/** How many tests libtest runs at once for these arguments. */
function threadCount(args) {
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--test-threads" && i + 1 < args.length) return Number(args[i + 1]) || 1;
    if (args[i].startsWith("--test-threads=")) return Number(args[i].split("=")[1]) || 1;
  }
  if (Number(process.env.RUST_TEST_THREADS) > 0) return Number(process.env.RUST_TEST_THREADS);
  return typeof require("os").availableParallelism === "function" ? require("os").availableParallelism() : require("os").cpus().length;
}

/** Why a stopped run was stopped, and which tests were in flight (point 5). */
function hangReport(exe, args, res, why) {
  console.error("");
  console.error(`FAILED: ${why}, so it was stopped: a hung test (on 2026-11-01 one hung silently for two hours).`);
  const list = res.parsed ? listTests(exe, args) : null;
  if (list) {
    const threads = threadCount(args);
    const inFlight = list.filter((n) => !res.done.has(n)).slice(0, threads);
    console.error(`${res.done.size} of ${list.length} tests had finished. libtest starts tests in name order, at most ${threads} at a time, and prints each only after its thread has EXITED, so the hung test is one of these ${inFlight.length}:`);
    for (const n of inFlight) console.error(`  ${n}${res.warned.has(n) ? "   (it said it had been running over 60 seconds)" : ""}`);
  } else if (res.warned.size) {
    console.error("Tests that said they had been running over 60 seconds:");
    for (const n of res.warned) console.error(`  ${n}`);
  }
  console.error("A test thread that reports its result and then cannot exit stops ALL of libtest's output, its 60-second warnings included (scripts/lib-tests.js, point 5).");
  if (busy.length) console.error(`${busy.join(", ")} was running; the GPU tests stood down (src/test_gpu.rs), so look past them.`);
  else console.error("If one of the operator's GPU games was running, this run did not know it (scripts/lib/operator-apps.json lists them).");
  console.error("Run the suspects alone, one at a time: node scripts/lib-tests.js <name> --exact --verbose");
}

/** The tests that stood down for the GPU game in this run (point 4), each once, by name. */
function gpuSkipped() {
  try {
    return [...new Set(fs.readFileSync(GPU_SKIPPED_FILE, "utf8").split(/\r?\n/).map((s) => s.trim()).filter(Boolean))].sort();
  } catch {
    return [];
  }
}

/** The "test result:" summary line of a run, or null. */
const summaryOf = (out) => {
  const m = String(out).match(/^test result: .*$/gm);
  return m ? m[m.length - 1].trim() : null;
};

/** The "---- name stdout ----" block of one failure (the first lines), for the ledger. */
function failureExcerpt(out, name) {
  const lines = String(out).split(/\r?\n/);
  const i = lines.findIndex((l) => l === `---- ${name} stdout ----`);
  if (i < 0) return null;
  const block = [];
  for (let k = i + 1; k < lines.length && block.length < 12; k++) {
    if (lines[k].startsWith("---- ") || lines[k] === "failures:") break;
    block.push(lines[k]);
  }
  return block.join("\n").trim();
}

/** The arguments a rerun keeps: everything but name filters, --skip and --exact (it adds its own). */
function rerunArgs(args) {
  const out = [];
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--skip") {
      i++;
      continue;
    }
    if (a.startsWith("--skip=") || a === "--exact") continue;
    if (!a.startsWith("-")) continue; // a name filter
    out.push(a);
    if ((a === "--test-threads" || a === "--format" || a === "--color" || a === "-Z" || a === "--logfile") && i + 1 < args.length) out.push(args[++i]);
  }
  return out;
}

function banner(lines) {
  const bar = "!".repeat(78);
  console.log("");
  console.log(bar);
  for (const l of lines) console.log(`!! ${l}`);
  console.log(bar);
  console.log("");
}

function recordPass(result, secs, flaky, skipped = []) {
  if (!full || !identity) return;
  const rec = { at: new Date().toISOString(), identity, result, run_s: secs, flaky };
  // A pass without the GPU tests (point 4) says which, so the next run on this tree runs just them.
  if (skipped.length) Object.assign(rec, { gpu_skipped: skipped, gpu_busy: busy.join(", ") });
  try {
    L.writeJsonAtomic(PASS_FILE, rec);
  } catch (e) {
    console.error(`lib-tests: could not record the pass (${e.message})`);
  }
}

/** Say loudly which tests stood down for the GPU game, and what happens to them (point 4). */
function gpuBanner(skipped) {
  if (!skipped.length) return;
  banner([
    `GPU TESTS NOT RUN: ${skipped.length} test(s) open a GPU device and stood down, because ${busy.join(", ")} is running`,
    "(beside it a GPU test can hang the whole run, silently: src/test_gpu.rs):",
    ...skipped.map((n) => `  ${n}`),
    "libtest counts them as passed; they did nothing.",
    full
      ? `The pass is recorded WITHOUT them: once it has closed, run this again on the same tree and it runs just these ${skipped.length}.`
      : `Run them once it has closed: node scripts/lib-tests.js ${skipped.join(" ")} --exact`,
  ]);
}

/** The summary to record: a partial pass's own, completed by the GPU tests it left. */
function resultLine(res, extra = "") {
  const own = summaryOf(res.out) || "(no summary)";
  const line = partialPass ? `${partialPass.result || "(no summary)"}; then its ${partialPass.gpu_skipped.length} GPU test(s): ${own}` : own;
  return line + extra;
}

async function main() {
  const exe = buildTestProgram();
  try {
    fs.mkdirSync(path.dirname(GPU_SKIPPED_FILE), { recursive: true });
    fs.writeFileSync(GPU_SKIPPED_FILE, "");
  } catch (e) {
    console.error(`lib-tests: could not clear ${GPU_SKIPPED_FILE} (${e.message})`);
  }
  const first = await runProgram(exe, runArgs, opts.runTimeoutMin);
  if (first.stalled || first.timedOut) {
    hangReport(exe, runArgs, first, first.stalled ? `the test program printed nothing for ${opts.stallMin} minutes` : `the test RUN (not the build) went past ${opts.runTimeoutMin} minutes`);
    process.exit(1);
  }
  if (first.code === 0) {
    const skipped = gpuSkipped();
    gpuBanner(skipped);
    recordPass(resultLine(first), first.secs, partialPass ? partialPass.flaky || [] : [], skipped);
    process.exit(0);
  }
  const failed = L.parseFailures(first.out);
  if (!failed.length) process.exit(first.code); // a crash or a build-time failure: nothing to rerun
  if (failed.length > MAX_RERUNS) {
    console.log(`lib-tests: ${failed.length} tests failed: more than ${MAX_RERUNS} is not a timing flake, so none is run again.`);
    process.exit(first.code);
  }
  console.log(`lib-tests: ${failed.length} test(s) failed; running each once more, alone, to tell a timing flake from a real failure:`);
  const flaky = [];
  const real = [];
  for (const name of failed) {
    console.log(`lib-tests: again, alone: ${name}`);
    const args = [name, "--exact", ...rerunArgs(runArgs)];
    const again = await runProgram(exe, args, opts.runTimeoutMin);
    if (again.stalled || again.timedOut) {
      hangReport(exe, args, again, `${name}, run again alone, ${again.stalled ? `printed nothing for ${opts.stallMin} minutes` : `went past ${opts.runTimeoutMin} minutes`}`);
      process.exit(1);
    }
    const ran = /running 1 test/.test(again.out);
    if (again.code === 0 && ran) flaky.push(name);
    else real.push(name);
  }
  for (const name of flaky) {
    const entry = {
      at: new Date().toISOString(),
      test: name,
      first_failure: failureExcerpt(first.out, name),
      passed_alone: true,
      tree: identity ? identity.head_tree.slice(0, 12) : null,
      rustc: identity ? identity.rustc : null,
      full_run: full,
    };
    try {
      fs.mkdirSync(path.dirname(FLAKY_LEDGER), { recursive: true });
      fs.appendFileSync(FLAKY_LEDGER, JSON.stringify(entry) + "\n");
    } catch (e) {
      console.error(`lib-tests: could not write ${FLAKY_LEDGER}: ${e.message}`);
    }
  }
  if (flaky.length) {
    banner([
      `FLAKY: ${flaky.length} test(s) failed in this run and PASSED when run again alone:`,
      ...flaky.map((n) => `  ${n}`),
      "That is a test whose result depends on timing or on its neighbours, not on the code under test.",
      `Recorded in ${path.relative(ROOT, FLAKY_LEDGER)} (one line each, with the first failure's message).`,
      real.length ? "" : "The run counts as PASSED. Fix the flake when you can: the ledger says how often it bites.",
    ].filter((l) => l !== ""));
  }
  if (real.length) {
    gpuBanner(gpuSkipped());
    console.log(`REAL FAILURE: ${real.length} test(s) failed and failed again alone:`);
    for (const n of real) console.log(`  ${n}`);
    process.exit(first.code || 1);
  }
  const skipped = gpuSkipped();
  gpuBanner(skipped);
  recordPass(resultLine(first, `; ${flaky.length} flaky, passed alone`), first.secs, flaky, skipped);
  process.exit(0);
}

main().catch((e) => {
  console.error("lib-tests:", e.message);
  process.exit(1);
});
