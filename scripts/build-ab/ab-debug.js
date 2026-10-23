#!/usr/bin/env node
// A/B: debug info in dev and test builds (build-speed review item 2, 2026-10-23).
//
//   node scripts/build-ab/ab-debug.js a|b [--incremental-only]
//
// Variant a is Cargo.toml as it is (debug = 2 everywhere); variant b is line-tables-only for our
// crate and none for the dependencies, given with --config so Cargo.toml is not touched. Each
// variant: a cold build of the library's test program (the scripts/lib-tests.js command) in a
// target folder of its own (target\ab-<variant>), then an incremental rebuild after a one-line
// edit of src/gui/pages/dev.rs (undone after). The whole cargo run is measured by the slot's
// --measure (wall, CPU seconds of the tree, peak memory of all its processes at once), and our
// crate's own compile by the slot's ledger line. Results: target\build-ab\ab-<variant>.log.

"use strict";

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const T = require("./tools.js");

const variant = process.argv[2];
const incOnly = process.argv.includes("--incremental-only");
if (!T.VARIANTS[variant]) {
  console.error("ab-debug: give the variant, a or b");
  process.exit(2);
}
const wrapper = T.ensureWrapper();
const log = T.logger(`ab-${variant}`);
const slotLog = T.SLOT_LOG;
const env = {
  ...process.env,
  CARGO_TARGET_DIR: path.join(T.ROOT, "target", `ab-${variant}`),
  CARGO_BUILD_JOBS: "4",
  CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper,
};
const logSize = () => (fs.existsSync(slotLog) ? fs.statSync(slotLog).size : 0);

function step(name) {
  const from = logSize();
  const t0 = Date.now();
  const l0 = T.load();
  log(`[${variant}] ${name}: start (cargo+rustc processes ${l0.procs}, free physical ${l0.free} MB)`);
  const r = spawnSync(
    wrapper,
    ["--measure", "cargo", ...T.VARIANTS[variant], "rustc", "--features", "native", "--lib", "--profile", "test", "--message-format=json-render-diagnostics", "--", "-C", "link-arg=/DEBUG:NONE"],
    { cwd: T.ROOT, env, encoding: "utf8", maxBuffer: 1 << 30 }
  );
  const wall = (Date.now() - t0) / 1000;
  const slot = fs.existsSync(slotLog) ? fs.readFileSync(slotLog, "utf8").slice(from) : "";
  const mine = slot.split("\n").filter((x) => /humanity_engine \(lib-test-native\)/.test(x));
  const done = mine.find((x) => / done /.test(x)) || "(no done line)";
  const waited =
    mine.map((x) => x.match(/after waiting (\d+) s/)).filter(Boolean).map((m) => +m[1]).reduce((a, b) => a + b, 0) +
    mine.map((x) => x.match(/WAITED (\d+) min/)).filter(Boolean).map((m) => +m[1] * 60).reduce((a, b) => a + b, 0);
  const meas = ((r.stderr || "").match(/\[measure\][^\n]*/g) || []).pop() || "(no measure line)";
  const l1 = T.load();
  log(`[${variant}] ${name}: exit ${r.status}, wall ${wall.toFixed(0)} s (slot wait ${waited} s); our crate: ${done.replace(/^\d+ \S+ pid \d+ /, "")}`);
  log(`[${variant}] ${name}: whole run ${meas}; load at the end: ${l1.procs} cargo+rustc, ${l1.free} MB free`);
  if (r.status !== 0) log((r.stderr || "").slice(-3000));
  return r.status;
}

if (!incOnly) step("cold build");
const TOUCH = path.join(T.ROOT, "src", "gui", "pages", "dev.rs");
const before = fs.readFileSync(TOUCH);
fs.writeFileSync(TOUCH, Buffer.concat([before, Buffer.from(`\n// ab-debug touch ${Date.now()}\n`)]));
try {
  step("incremental after a one-line edit of src/gui/pages/dev.rs");
} finally {
  fs.writeFileSync(TOUCH, before);
}
// The deps folder's size: what the dependencies' debug info costs on disk and to link.
const deps = path.join(T.ROOT, "target", `ab-${variant}`, "debug", "deps");
let bytes = 0;
for (const f of fs.readdirSync(deps)) {
  try {
    bytes += fs.statSync(path.join(deps, f)).size;
  } catch {}
}
log(`[${variant}] target\\ab-${variant}\\debug\\deps: ${(bytes / 1e9).toFixed(1)} GB`);
