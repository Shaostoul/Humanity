#!/usr/bin/env node
// Seed a fresh checkout's build folder from a warm one (2026-10-23, the build-speed review).
//
//   node scripts/seed-target.js [--from <target folder>] [--to <target folder>] [--profile debug|release]
//                               [--force] [--dry-run]
//   just seed-target
//
// A new worktree's first build compiles every dependency (about 700 crates) before it reaches
// ours. The main checkout's target folder holds those already, built by the same toolchain
// (rust-toolchain.toml) from the same Cargo.lock. This copies the DEPENDENCIES' build output
// (deps\, build\, .fingerprint\) from that folder into this checkout's empty one, so cargo finds
// them fresh and goes straight to our crate.
//
// What it does NOT copy, and why:
//   - our own crate's files (humanity_engine, HumanityOS, the integration tests): cargo hashes a
//     path package's location into its artifacts, so a worktree's build of our crate can never
//     reuse the main checkout's, and they are the bulk of the folder (174 GB on 2026-10-23)
//   - incremental\: only our crate is built incrementally, for the same reason
//   - a folder whose dependencies were built by another toolchain: refused (the copies would all
//     be rebuilt anyway, so the copy would only cost time and disk)
//
// Why it is safe: cargo decides freshness from fingerprints, not from where a file came from. A
// copied dependency whose fingerprint does not match this checkout (another feature set,
// another Cargo.lock entry) is simply rebuilt; the worst a stale copy can cost is the normal
// build. Build scripts' recorded output names their old OUT_DIR, and cargo rewrites that path to
// the new folder when it reads it back (cargo records the OUT_DIR it was generated with for
// exactly this), so nothing points into the source folder afterwards. Timestamps are kept
// (robocopy /COPY:DAT): cargo compares a unit's time with its dependencies'.
//
// When to use it: right after creating a worktree, before its first build. It refuses a
// destination that already holds builds (--force to add to one).

"use strict";

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const { mainCheckoutRoot } = require("./lib/dxc-dlls.js");
const SA = require("./lib/stale-artifacts.js");

const ROOT = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const DRY = args.includes("--dry-run");
const FORCE = args.includes("--force");
const PROFILE = opt("--profile", "debug");
const mainRoot = mainCheckoutRoot(ROOT);
const FROM = path.resolve(opt("--from", mainRoot ? path.join(mainRoot, "target") : ""), PROFILE);
// --to <target folder>: another destination (a measurement); default this checkout's target.
const TO = path.resolve(opt("--to", path.join(ROOT, "target")), PROFILE);

function fail(lines) {
  for (const l of lines) console.error(l);
  process.exit(1);
}

if (!opt("--from", null) && !mainRoot) fail(["seed-target: this is the main checkout; there is nothing to seed it from (give --from <target folder>)."]);
if (!fs.existsSync(path.join(FROM, "deps"))) fail([`seed-target: ${FROM} has no deps folder: nothing built there to copy.`]);
if (path.resolve(FROM).toLowerCase() === path.resolve(TO).toLowerCase()) fail(["seed-target: the source and the destination are the same folder."]);
let existing = [];
try {
  existing = fs.readdirSync(path.join(TO, "deps"));
} catch {}
if (existing.length && !FORCE) {
  fail([
    `seed-target: ${TO} already holds ${existing.length} build file(s). Seeding is for a fresh checkout's first build;`,
    "copying over a live folder could put older copies beside newer ones. --force adds to it anyway.",
  ]);
}

// The same toolchain? Cargo writes the rustc it used into .rustc_info.json beside the profiles.
const rustcHere = (spawnSync("rustc", ["-vV"], { cwd: ROOT, encoding: "utf8", windowsHide: true }).stdout || "").match(/^release: (.+)$/m);
let rustcThere = null;
try {
  const info = JSON.parse(fs.readFileSync(path.join(path.dirname(FROM), ".rustc_info.json"), "utf8"));
  for (const o of Object.values(info.outputs || {})) {
    const m = String(o.stdout || "").match(/^release: (.+)$/m);
    if (m) rustcThere = m[1];
  }
} catch {}
if (rustcHere && rustcThere && rustcHere[1] !== rustcThere) {
  fail([`seed-target: ${FROM} was built by rustc ${rustcThere}; this checkout uses ${rustcHere[1]} (rust-toolchain.toml). Its dependencies would all be rebuilt: not copying.`]);
}

// What to leave out: our crate's files in deps\ (by name), and its unit folders.
const ours = new Set(SA.workspaceCrates(ROOT));
const depsFiles = fs.readdirSync(path.join(FROM, "deps"));
const skipDeps = depsFiles.filter((f) => {
  const p = SA.parseDepName(f);
  return p && ours.has(p.crate);
});
const unitSkip = (dir) => {
  let list = [];
  try {
    list = fs.readdirSync(path.join(FROM, dir));
  } catch {}
  return list.filter((d) => /^humanity-engine-[0-9a-f]{16}$/.test(d));
};
const skipFp = unitSkip(".fingerprint");
const skipBuild = unitSkip("build");

console.log(`seed-target: ${FROM} -> ${TO}`);
console.log(`  leaving out our crate's ${skipDeps.length} file(s) in deps and ${skipFp.length + skipBuild.length} unit folder(s); incremental is not copied`);
if (DRY) {
  console.log("  (dry run: nothing copied)");
  process.exit(0);
}

const t0 = Date.now();
fs.mkdirSync(TO, { recursive: true });
let failed = false;
// Patterns, not names: our crate can have thousands of files (3,606 in deps on 2026-10-23), far
// past a command line's length.
const filePatterns = [...ours].flatMap((c) => [`${c}-*`, `lib${c}-*`]);
for (const [dir, skipFiles, skipDirs] of [
  ["deps", filePatterns, []],
  ["build", [], skipBuild.length ? ["humanity-engine-*"] : []],
  [".fingerprint", [], skipFp.length ? ["humanity-engine-*"] : []],
]) {
  if (!fs.existsSync(path.join(FROM, dir))) continue;
  // robocopy: multi-threaded, keeps timestamps; /XF and /XD leave our crate out. Exit codes
  // below 8 are success (they say what was copied).
  const a = [path.join(FROM, dir), path.join(TO, dir), "/E", "/COPY:DAT", "/DCOPY:DAT", "/MT:16", "/R:1", "/W:1", "/NFL", "/NDL", "/NJH", "/NJS", "/NP"];
  if (skipFiles.length) a.push("/XF", ...skipFiles);
  if (skipDirs.length) a.push("/XD", ...skipDirs);
  const r = spawnSync("robocopy", a, { encoding: "utf8", windowsHide: true, maxBuffer: 1 << 26 });
  if (r.status === null || r.status >= 8) {
    failed = true;
    console.error(`seed-target: robocopy of ${dir} failed (exit ${r.status}): ${(r.stdout || "").slice(-800)}${r.stderr || ""}`);
  }
}
// The toolchain record, so cargo does not run its probes again.
try {
  fs.copyFileSync(path.join(path.dirname(FROM), ".rustc_info.json"), path.join(path.dirname(TO), ".rustc_info.json"));
} catch {}
console.log(`seed-target: ${failed ? "PARTLY copied" : "copied"} in ${((Date.now() - t0) / 1000).toFixed(0)} s. The next build compiles our crate and whatever this checkout's Cargo.lock or features change.`);
process.exit(failed ? 1 : 0);
