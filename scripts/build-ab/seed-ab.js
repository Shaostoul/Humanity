#!/usr/bin/env node
// A fresh checkout's first build, seeded from a warm target, against the cold one (build-speed
// review item 9).
//
//   node scripts/build-ab/seed-ab.js        (after `ab-debug.js b` built target\ab-b)
//
// The "fresh checkout" is a copy of this tree's tracked files at another path, as a new worktree
// would be (cargo hashes a path package's location, so our crate rebuilds either way); the warm
// target is target\ab-b. The seeded first build is measured; the cold baseline is ab-b's own
// cold build in target\build-ab\ab-b.log. A clear win (the seeded build compiles our crate and
// the path dependencies, not 700 crates) and no harm (a stale copy only costs a normal build,
// which cargo's fingerprints guarantee) is what would justify a `just seed-target` recipe.

"use strict";

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const T = require("./tools.js");

const wrapper = T.ensureWrapper();
const log = T.logger("seed-ab");
const TREE = path.join(T.ROOT, "target", "seedtree");
fs.rmSync(TREE, { recursive: true, force: true });
const files = spawnSync("git", ["ls-files", "-z"], { cwd: T.ROOT, encoding: "utf8", maxBuffer: 1 << 28 }).stdout.split("\0").filter(Boolean);
let t0 = Date.now();
for (const f of files) {
  try {
    fs.mkdirSync(path.dirname(path.join(TREE, f)), { recursive: true });
    fs.copyFileSync(path.join(T.ROOT, f), path.join(TREE, f));
  } catch {}
}
log(`copied ${files.length} tracked files to ${TREE} in ${((Date.now() - t0) / 1000).toFixed(0)} s`);
t0 = Date.now();
const seed = spawnSync(process.execPath, [path.join(T.ROOT, "scripts", "seed-target.js"), "--from", path.join(T.ROOT, "target", "ab-b"), "--to", path.join(TREE, "target")], { cwd: TREE, encoding: "utf8" });
log(`seed: exit ${seed.status} in ${((Date.now() - t0) / 1000).toFixed(0)} s: ${(seed.stdout + seed.stderr).trim().split("\n").join(" | ")}`);
const env = { ...process.env, CARGO_TARGET_DIR: path.join(TREE, "target"), CARGO_BUILD_JOBS: "4", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper };
const l = T.load();
t0 = Date.now();
const r = spawnSync(wrapper, ["--measure", "cargo", ...T.VARIANTS.b, "rustc", "--features", "native", "--lib", "--profile", "test", "--", "-C", "link-arg=/DEBUG:NONE"], { cwd: TREE, env, encoding: "utf8", maxBuffer: 1 << 30 });
const compiled = ((r.stderr || "").match(/^\s+Compiling (\S+)/gm) || []).map((x) => x.trim().split(/\s+/)[1]);
const meas = ((r.stderr || "").match(/\[measure\][^\n]*/g) || []).pop() || "(no measure line)";
log(`seeded first build: exit ${r.status}, wall ${((Date.now() - t0) / 1000).toFixed(0)} s, ${compiled.length} crate(s) compiled: ${compiled.join(", ")}`);
log(`seeded first build: ${meas} (load at the start: ${l.procs} cargo+rustc, ${l.free} MB free)`);
