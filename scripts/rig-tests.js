#!/usr/bin/env node
// `just rig-tests`: every scripts/tests/*.test.js under `node --test`, minus the files
// scripts/test-discovery.json excludes with a reason (2026-11-14; scripts/lib/test-discovery.js
// says why this is discovery and not a hand-kept list).
//
// Usage:
//   node scripts/rig-tests.js            run them all (one `node --test` with every file, as before)
//   node scripts/rig-tests.js --list     print the files it would run, one a line, and stop
//   node scripts/rig-tests.js --test-reporter=dot   any other --flag is handed to node --test
//
// Each test file's own header says what it proves; there is no second copy of that here.
"use strict";

const path = require("path");
const { spawnSync } = require("child_process");
const { discover, excludes } = require("./lib/test-discovery.js");

const ROOT = path.resolve(__dirname, "..");
const DIR = path.join(ROOT, "scripts", "tests");

const argv = process.argv.slice(2);
const { names, problems } = discover({ dir: DIR, suffix: ".test.js", exclude: excludes("rig_tests") });
if (problems.length) {
  for (const p of problems) console.error("rig-tests: " + p);
  process.exit(1);
}
const files = names.map((n) => "scripts/tests/" + n);
if (argv.includes("--list")) {
  console.log(files.join("\n"));
  process.exit(0);
}
const r = spawnSync(process.execPath, ["--test", ...argv, ...files], { cwd: ROOT, stdio: "inherit", windowsHide: true });
if (r.error) {
  console.error("rig-tests: could not start node --test: " + r.error.message);
  process.exit(1);
}
process.exit(r.status === null ? 1 : r.status);
