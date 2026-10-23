#!/usr/bin/env node
// Run the std-only lint programs (`just lints`), each compiled once per content (2026-10-22).
//
// Usage: node scripts/run-lints.js <lint name>...      e.g. emdash_lint theme_token_lint
//        (the Justfile's `lints` recipe passes the full list; that list is the authority)
//
// Each name is tests/<name>.rs, compiled with `rustc --test --edition 2021 -A warnings` the way
// the recipe always did (never through cargo, so the native bin is never linked: the Windows
// LNK1318 limit, BUG-174), with CARGO_MANIFEST_DIR set to this checkout because the lints read
// it at compile time. The compiled program is kept in target/lint-cache/ under a name made of
// the source's hash, `rustc -V`, the arguments and that path (scripts/lib/rustc-cache.js), so an
// unchanged lint is not compiled again.
//
// MEASURED 2026-10-23 (Rust 1.99.1, 19 lints): the old bash loop took 32.7 s, but compiling was
// the smaller part (18.9 s for all 19 one after another; 4.8 s four at a time). Running them was
// 30.5 s, and 24.0 s of that was ONE lint, terminology_lint, which built 32 strings for every
// line it read; fixed the same day (1.5 s). So the runner caches the compiles AND runs four
// lints at a time: a warm run is a few seconds.
//
// EVERY lint runs, and every failure is reported. The old loop stopped at the first red one,
// which switched off every lint behind it (tests/file_size_ratchet.rs records three sessions
// where `just verify` could not run at all for that reason). Output: ">> lint ok: <name>" for
// each that passed, in the order given, then "LINT FAILED: <name>" and that lint's own output
// for each that failed, then exit 1. A lint that does not COMPILE fails the same way, with
// rustc's message.
//
// --no-cache compiles every lint fresh (into the cache folder) when you suspect the cache;
// --timing prints how long the compiles and the runs took; --root <dir> runs another checkout's.

"use strict";

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");
const RC = require("./lib/rustc-cache.js");

const argv = process.argv.slice(2);
// --root <dir>: another checkout (the runner's own test uses a throwaway one).
const rootAt = argv.indexOf("--root");
const ROOT = rootAt >= 0 ? path.resolve(argv.splice(rootAt, 2)[1]) : path.resolve(__dirname, "..");
const noCache = argv.includes("--no-cache");
const timing = argv.includes("--timing");
const names = argv.filter((a) => !a.startsWith("--"));
if (!names.length) {
  console.error("run-lints: name the lints to run (tests/<name>.rs), e.g. node scripts/run-lints.js emdash_lint");
  process.exit(2);
}

const CACHE = path.join(ROOT, "target", "lint-cache");
const ARGS = ["--test", "--edition", "2021", "-A", "warnings"];
const ENV = { CARGO_MANIFEST_DIR: ROOT };
const AT_ONCE = 4;

/** Run fn over items, AT_ONCE at a time, results in the items' order. */
async function pool(items, fn) {
  const out = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const i = next++;
      out[i] = await fn(items[i], i);
    }
  };
  await Promise.all(Array.from({ length: Math.min(AT_ONCE, items.length) }, worker));
  return out;
}

/** Run one lint program: { status, out } with its stdout and stderr together. */
function runLint(exe) {
  return new Promise((resolve) => {
    const c = spawn(exe, [], { cwd: ROOT, env: { ...process.env, ...ENV }, windowsHide: true });
    let out = "";
    c.stdout.on("data", (d) => (out += d));
    c.stderr.on("data", (d) => (out += d));
    c.on("error", (e) => resolve({ status: 1, out: out + `\n  could not run ${exe}: ${e.message}\n` }));
    c.on("close", (code) => resolve({ status: code === null ? 1 : code, out }));
  });
}

async function main() {
  const t0 = Date.now();
  const missing = names.filter((n) => !fs.existsSync(path.join(ROOT, "tests", `${n}.rs`)));
  if (missing.length) {
    for (const n of missing) {
      console.log(`LINT FAILED: ${n}`);
      console.log(`  tests/${n}.rs does not exist (a name in the Justfile's lints list with no file)`);
    }
    process.exit(1);
  }
  const plans = names.map((n) => {
    const p = RC.plan({ src: path.join(ROOT, "tests", `${n}.rs`), name: n, cacheDir: CACHE, args: ARGS, env: ENV, cwd: ROOT });
    if (noCache) p.cached = false;
    return p;
  });
  const builds = await pool(plans, (p) => RC.buildAsync(p));
  const tCompiled = Date.now();
  const runs = await pool(builds, (b) => (b.ok ? runLint(b.exe) : Promise.resolve(null)));
  const failed = [];
  for (let i = 0; i < names.length; i++) {
    const b = builds[i];
    const r = runs[i];
    if (b.scratch) fs.rmSync(b.scratch, { recursive: true, force: true });
    if (!b.ok) failed.push({ n: names[i], text: `  tests/${names[i]}.rs did not compile:\n${b.stderr}` });
    else if (r.status !== 0) failed.push({ n: names[i], text: r.out });
    else console.log(`>> lint ok: ${names[i]}`);
  }
  for (const f of failed) {
    console.log(`LINT FAILED: ${f.n}`);
    process.stdout.write(f.text.endsWith("\n") ? f.text : f.text + "\n");
  }
  if (timing || failed.length) {
    const compiled = builds.filter((b) => !b.cached).length;
    console.log(
      `run-lints: ${names.length} lints, ${failed.length} failed; ${compiled} compiled in ${((tCompiled - t0) / 1000).toFixed(1)} s, ` +
        `run in ${((Date.now() - tCompiled) / 1000).toFixed(1)} s (${AT_ONCE} at a time)`
    );
  }
  if (failed.length) {
    if (failed.length > 1) console.log(`LINT FAILED: ${failed.length} lints: ${failed.map((f) => f.n).join(", ")}`);
    process.exit(1);
  }
}

main().catch((e) => {
  console.log(`LINT FAILED: run-lints itself (${e.message})`);
  process.exit(1);
});
