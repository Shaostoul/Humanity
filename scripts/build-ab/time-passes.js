#!/usr/bin/env node
// Where one test build of our crate spends its time (build-speed review item 10).
//
//   node scripts/build-ab/time-passes.js a|b        (after ab-debug.js built that variant)
//
// Our crate compiled COLD (its incremental folders moved aside and deleted after), with
// RUSTC_BOOTSTRAP=1 and -Z time-passes, in target\ab-<variant>. Prints the thirty longest passes
// and the rows that answer "analysis or code generation?": type checking, borrow checking and
// MIR against codegen and LLVM. That share is what says whether splitting the crate would help
// (docs/history/2026-06-30.md recorded one crate as final): parallel codegen already spreads
// across CPUs inside one crate, so a split mainly buys parallel ANALYSIS and smaller
// recompiles. The full output is target\build-ab\time-passes-<variant>.txt.
// The slowest tests: RUSTC_BOOTSTRAP=1 node scripts/lib-tests.js --force -Z unstable-options --report-time
// (libtest prints each test's time; sort by it).

"use strict";

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const T = require("./tools.js");

const variant = process.argv[2] || "b";
const wrapper = T.ensureWrapper();
const log = T.logger(`time-passes-${variant}`);
const targetDir = path.join(T.ROOT, "target", `ab-${variant}`);
const inc = path.join(targetDir, "debug", "incremental");
const aside = path.join(targetDir, "incremental-aside");
fs.mkdirSync(aside, { recursive: true });
for (const d of fs.existsSync(inc) ? fs.readdirSync(inc) : []) {
  if (/^humanity_engine-/.test(d)) fs.renameSync(path.join(inc, d), path.join(aside, d));
}
const lib = path.join(T.ROOT, "src", "lib.rs");
const now = new Date();
fs.utimesSync(lib, now, now); // cargo reruns rustc for our crate; the content is unchanged
const env = { ...process.env, RUSTC_BOOTSTRAP: "1", CARGO_TARGET_DIR: targetDir, CARGO_BUILD_JOBS: "4", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper };
const l = T.load();
const t0 = Date.now();
const r = spawnSync("cargo", [...T.VARIANTS[variant], "rustc", "--features", "native", "--lib", "--profile", "test", "--", "-C", "link-arg=/DEBUG:NONE", "-Z", "time-passes"], {
  cwd: T.ROOT,
  env,
  encoding: "utf8",
  maxBuffer: 1 << 30,
});
const text = (r.stderr || "") + (r.stdout || "");
fs.writeFileSync(path.join(T.OUT, `time-passes-${variant}.txt`), text);
const rows = [];
for (const line of text.split(/\r?\n/)) {
  const m = line.match(/time:\s+([\d.]+);.*?\s([A-Za-z_][\w:-]*)\s*$/);
  if (m) rows.push({ s: Number(m[1]), pass: m[2] });
}
const get = (name) => rows.filter((x) => x.pass === name).reduce((a, x) => Math.max(a, x.s), 0);
log(`exit ${r.status}, wall ${((Date.now() - t0) / 1000).toFixed(0)} s, ${rows.length} timed passes (load at the start: ${l.procs} cargo+rustc, ${l.free} MB free)`);
for (const x of [...rows].sort((a, b) => b.s - a.s).slice(0, 30)) log(`${x.s.toFixed(2).padStart(9)} s  ${x.pass}`);
for (const k of ["total", "analysis", "type_check_crate", "MIR_borrow_checking", "MIR_effect_checking", "monomorphization_collector", "codegen_crate", "codegen_to_LLVM_IR", "LLVM_passes", "link_binary", "incr_comp_persist_result_cache", "incr_comp_persist_dep_graph", "generate_crate_metadata"]) {
  log(`  ${k.padEnd(32)} ${get(k).toFixed(2)} s`);
}
fs.rmSync(aside, { recursive: true, force: true });
