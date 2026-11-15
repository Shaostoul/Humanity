#!/usr/bin/env node
// The music budgets only a quiet machine can judge (2026-11-15). `just music-quiet-budgets`
//
// WHY: the audio thread's budget tests in crates/humanity-music measure on whatever machine runs
// them, and most of them now hold beside other builds (crates/humanity-music/src/tests_cost.rs:
// the piece and its calm-score baseline taking turns, each block its fastest of several renders).
// One judgement does not: that 32 streamed notes cost at most a quarter more than the same 32
// held whole. Reading rings the worker has just written is more sensitive to builds crowding the
// processor's shared cache than reading recordings straight through, so beside a release build
// the ratio itself moves (one of five rounds read 0.0631 against 0.0500 ms, 2026-11-15), and the
// quarter's margin is too thin to judge there. Those tests are `quiet_*` and #[ignore]d in the
// suite (which keeps a loose bound that holds anywhere); this runs them when nothing is building.
//
// What it does: builds the music crate's test program (running nothing), waits (bounded, default
// 60 min, --wait-min N) until no build and no game is running (scripts/lib/machine-guard.js, the
// same guard the timing sweeps use), runs the `quiet_` tests, and looks again afterwards: a build
// that started during the run makes the result a reading taken beside a build, said so, and the
// run fails rather than report it. Exit 0 passed, 1 failed, 2 the machine never went quiet, 3 a
// build arrived during the run.
//
// Usage: node scripts/music-quiet-budgets.js [--wait-min N]

const path = require("path");
const { spawnSync } = require("child_process");
const MG = require("./lib/machine-guard");

const ROOT = path.join(__dirname, "..");
const args = process.argv.slice(2);
const waitAt = args.indexOf("--wait-min");
const waitMin = waitAt >= 0 ? Number(args[waitAt + 1]) : MG.WAIT_MIN_DEFAULT;
if (!(waitMin >= 0)) {
  console.error("music-quiet-budgets: --wait-min takes a number of minutes");
  process.exit(1);
}

const libTests = (extra) =>
  spawnSync(process.execPath, [path.join(ROOT, "scripts", "lib-tests.js"), "--crate", "humanity-music", "quiet_", ...extra], {
    cwd: ROOT,
    stdio: "inherit",
  }).status;

// 1. Build first (the quiet tests are ignored, so this runs none of them): the wait below is for
//    other people's builds, not this one.
console.log("music-quiet-budgets: building the music crate's tests");
if (libTests([]) !== 0) {
  console.error("music-quiet-budgets: the build failed");
  process.exit(1);
}

// 2. Wait for a machine with no build and no game running.
const own = { pids: [process.pid], waitForBuilds: true };
const w = MG.waitForFree({ own, memoryMb: 0, lock: false, label: "music quiet budgets", timeoutMs: waitMin * 60 * 1000 });
if (!w.free) {
  console.error(`music-quiet-budgets: REFUSED: the machine never went quiet in ${waitMin} min (${w.blockers.length} in the way). Run it again when nothing is building.`);
  for (const p of w.blockers) console.error(`  ${MG.describe(p)}`);
  process.exit(2);
}

// 3. Run the quiet tests, then look again.
const status = libTests(["--ignored", "--nocapture"]);
const after = MG.foreignProcs({ ...own, gameOnly: false });
if (after.length) {
  console.error("music-quiet-budgets: a build or a game started during the run, so its timings are not a quiet reading:");
  for (const p of after) console.error(`  ${MG.describe(p)}`);
  process.exit(3);
}
console.log(status === 0 ? "music-quiet-budgets: passed on a quiet machine" : "music-quiet-budgets: FAILED on a quiet machine");
process.exit(status === 0 ? 0 : 1);
