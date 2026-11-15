// Dump every thread's stack of a running process to a file (2026-11-14), with the dev tool
// scripts/stackdump/hos-stackdump.rs (dbghelp's StackWalk64 over each thread; see its header).
//
// WHY: a frozen program is one thread dump away from an answer. scripts/lib-tests.js calls this
// on a test program that has gone quiet (or run past its time limit), BEFORE stopping it, and
// saves the dump beside the run's log, so the hang report names the stuck test's thread AND
// where it is stuck instead of only the tests that were in flight.
//
// The tool is std-only Rust built with plain rustc into the content-keyed folder the lints use
// (scripts/lib/rustc-cache.js: the key is the source, `rustc -V` and the arguments, so it cannot
// go stale). It lowers itself to below-normal priority, takes symbols only from the program's
// own folder (never a symbol server: no download), and reads the target without changing it.
// Windows x86-64 only; elsewhere dumpStacks says so and returns ok: false.

"use strict";

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const RC = require("./rustc-cache.js");

const REPO = path.resolve(__dirname, "..", "..");
const SRC = path.join(REPO, "scripts", "stackdump", "hos-stackdump.rs");

/** Build (or find already built) the dumper. { ok, exe, error }. */
function dumperExe() {
  if (process.platform !== "win32" || process.arch !== "x64") return { ok: false, exe: null, error: "the stack dumper is for x86-64 Windows only" };
  try {
    const p = RC.plan({ src: SRC, name: "hos-stackdump", cacheDir: path.join(REPO, "target", "test-cache"), args: ["--edition", "2021", "-O", "-A", "warnings", "-C", "debuginfo=0"], env: {}, cwd: REPO });
    const b = RC.buildSync(p);
    return b.ok ? { ok: true, exe: b.exe, error: null } : { ok: false, exe: null, error: `the stack dumper did not compile:\n${b.stderr}` };
  } catch (e) {
    return { ok: false, exe: null, error: `the stack dumper could not be built: ${e.message}` };
  }
}

/**
 * Write every thread's stack of process `pid` to `outFile`. Returns { ok, file, ms, error }:
 * `file` is written even when the dump failed, holding why, so the report can always name it.
 */
function dumpStacks(pid, outFile, { timeoutMs = 180_000 } = {}) {
  const t0 = Date.now();
  fs.mkdirSync(path.dirname(outFile), { recursive: true });
  const d = dumperExe();
  if (!d.ok) {
    fs.writeFileSync(outFile, `no stack dump: ${d.error}\n`);
    return { ok: false, file: outFile, ms: Date.now() - t0, error: d.error };
  }
  const r = spawnSync(d.exe, [String(pid)], { encoding: "utf8", timeout: timeoutMs, maxBuffer: 1 << 26, windowsHide: true });
  const ms = Date.now() - t0;
  const failed = r.status !== 0 ? (r.error ? r.error.message : `exit ${r.status}: ${(r.stderr || "").trim()}`) : null;
  fs.writeFileSync(outFile, (r.stdout || "") + (failed ? `\nthe stack dump did not finish: ${failed}\n` : ""));
  return { ok: !failed, file: outFile, ms, error: failed };
}

module.exports = { dumperExe, dumpStacks, SRC };
