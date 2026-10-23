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
//   --verbose              the test program's full output (one line per test); default is -q
//   --run-timeout-min N    fail if the test RUN (not the build) takes longer than N minutes
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
//   2. The test program runs with -q (one character per test): a full run's log was 460 KB of
//      "test x ... ok" lines and is a few KB now. Failures print in full either way.
//   3. When one to five tests fail, each is run once more, alone, by its exact name. One that
//      passes alone is a timing flake: a loud FLAKY banner, a line in
//      data/coordination/flaky-tests.jsonl (which test, when, the first failure's message), and
//      the run counts as passed, so a flake does not cost a whole rerun. One that fails again is
//      a real failure, and the run fails. More than five failures are not timing: no reruns.

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
const MAX_RERUNS = 5;

// Our options out, the test program's in.
const argv = process.argv.slice(2);
const opts = { force: false, verbose: false, runTimeoutMin: null };
const passthrough = [];
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (a === "--force") opts.force = true;
  else if (a === "--verbose") opts.verbose = true;
  else if (a === "--run-timeout-min") {
    opts.runTimeoutMin = Number(argv[++i]);
    if (!(opts.runTimeoutMin > 0)) {
      console.error("lib-tests: --run-timeout-min needs a number of minutes");
      process.exit(2);
    }
  } else passthrough.push(a);
}
const full = L.isFullRun(passthrough);
const quietFlag = !opts.verbose && !passthrough.some((a) => a === "-q" || a === "--quiet" || a.startsWith("--format"));
const runArgs = quietFlag ? [...passthrough, "-q"] : passthrough;

function fmtTime(iso) {
  const d = new Date(iso);
  return isNaN(d) ? String(iso) : d.toLocaleString("en-GB", { hour12: false });
}

// 1. The refusal: this exact tree already passed the full suite.
let identity = null;
if (full) {
  try {
    identity = L.treeIdentity(ROOT);
  } catch (e) {
    console.error(`lib-tests: could not read the tree's identity (${e.message}); the pass will not be recorded`);
  }
  const last = L.readJson(PASS_FILE);
  if (identity && last && last.identity && last.identity.hash === identity.hash && !opts.force) {
    const changed = identity.dirty.length ? ` + ${identity.dirty.length} changed file(s)` : "";
    console.log(`lib-tests: SKIPPED. This exact tree already passed the full suite at ${fmtTime(last.at)}:`);
    console.log(`  ${last.result || "(no summary)"}${last.run_s != null ? `, in ${last.run_s} s` : ""}`);
    console.log(`  tree ${identity.head_tree.slice(0, 12)}${changed}, ${identity.rustc}, features ${identity.features}`);
    if (last.flaky && last.flaky.length) console.log(`  (that pass had ${last.flaky.length} flaky test(s): ${last.flaky.join(", ")})`);
    console.log("Running it again on unchanged code proves nothing new (CLAUDE.md). To run it anyway:");
    console.log("  node scripts/lib-tests.js --force        (just test-lib --force)");
    process.exit(0);
  }
}

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

/** Run the test program, its output shown as it comes AND kept. Resolves { code, out, timedOut, secs }. */
function runProgram(exe, args, timeoutMin) {
  return new Promise((resolve) => {
    const t0 = Date.now();
    const c = spawn(exe, args, { cwd: ROOT, env: { ...process.env, CARGO_MANIFEST_DIR: ROOT }, stdio: ["inherit", "pipe", "pipe"] });
    let out = "";
    const keep = (d) => {
      if (out.length < 64 * 1024 * 1024) out += d;
    };
    c.stdout.on("data", (d) => {
      process.stdout.write(d);
      keep(d);
    });
    c.stderr.on("data", (d) => {
      process.stderr.write(d);
      keep(d);
    });
    let timedOut = false;
    const timer = timeoutMin
      ? setTimeout(() => {
          timedOut = true;
          c.kill();
        }, timeoutMin * 60 * 1000)
      : null;
    c.on("error", (e) => resolve({ code: 1, out: out + String(e), timedOut, secs: Math.round((Date.now() - t0) / 1000) }));
    c.on("close", (code) => {
      if (timer) clearTimeout(timer);
      resolve({ code: code === null ? 1 : code, out, timedOut, secs: Math.round((Date.now() - t0) / 1000) });
    });
  });
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

function recordPass(result, secs, flaky) {
  if (!full || !identity) return;
  try {
    L.writeJsonAtomic(PASS_FILE, { at: new Date().toISOString(), identity, result, run_s: secs, flaky });
  } catch (e) {
    console.error(`lib-tests: could not record the pass (${e.message})`);
  }
}

async function main() {
  const exe = buildTestProgram();
  const first = await runProgram(exe, runArgs, opts.runTimeoutMin);
  if (first.timedOut) {
    console.error(`FAILED: the library tests ran past ${opts.runTimeoutMin} minutes (the RUN, not the build: a hung test). Rerun and look for "has been running for over 60 seconds".`);
    process.exit(1);
  }
  if (first.code === 0) {
    recordPass(summaryOf(first.out), first.secs, []);
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
    const again = await runProgram(exe, [name, "--exact", ...rerunArgs(runArgs)], opts.runTimeoutMin);
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
    console.log(`REAL FAILURE: ${real.length} test(s) failed and failed again alone:`);
    for (const n of real) console.log(`  ${n}`);
    process.exit(first.code || 1);
  }
  recordPass(`${summaryOf(first.out) || "(no summary)"}; ${flaky.length} flaky, passed alone`, first.secs, flaky);
  process.exit(0);
}

main().catch((e) => {
  console.error("lib-tests:", e.message);
  process.exit(1);
});
