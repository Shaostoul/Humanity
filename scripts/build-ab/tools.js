// What the build-speed A/B scripts share (scripts/build-ab/, 2026-10-23).
//
// The A/Bs of the build-speed review: debug info in dev and test builds (ab-debug.js), Rust's
// own linker against link.exe (linker-ab.js), where a test build of our crate spends its time
// (time-passes.js), the relay gate of `just verify` (relay-gate.js) and a fresh checkout's first
// build seeded from a warm target (seed-ab.js). Each writes its numbers to target\build-ab\.
//
// RUN THEM WHEN THE MACHINE HAS ROOM. On 2026-10-23 they could not be run: fourteen compiles of
// our crate from twelve worktrees held about 102 GB of commit with 0.6 GB of physical memory
// free, and a measurement then measures the paging. Each compile here goes through this
// checkout's build of the build slot (so it waits its turn), and every timing should be reported
// with the machine's load beside it (the scripts log cargo and rustc processes and free memory).

"use strict";

const fs = require("fs");
const path = require("path");
const { spawnSync, execSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");
const OUT = path.join(ROOT, "target", "build-ab");
const WRAPPER = path.join(OUT, "hos-build-slot.exe");
/** The slot's own log, where our crate's compile lines are (its folder: the wrapper's --status). */
const SLOT_LOG = path.join(process.env.HOS_BUILD_SLOT_DIR || path.join(process.env.LOCALAPPDATA || "", "HumanityOS", "build-slot"), "build-slot.log");

/** Build this checkout's slot wrapper into target\build-ab\ (once per run of a script). */
function ensureWrapper() {
  fs.mkdirSync(OUT, { recursive: true });
  const r = spawnSync(process.execPath, [path.join(ROOT, "scripts", "build-slot.js"), "build", "--out", WRAPPER], { cwd: ROOT, encoding: "utf8" });
  if (r.status !== 0) throw new Error(`could not build the slot wrapper: ${r.stdout}${r.stderr}`);
  return WRAPPER;
}

/** Cargo and rustc processes on the machine, and free physical memory (MB): the load to report. */
function load() {
  let procs = -1;
  try {
    procs = execSync("tasklist", { encoding: "utf8" }).split("\n").filter((l) => /^(cargo|rustc)\.exe/i.test(l)).length;
  } catch {}
  let free = "?";
  try {
    free = (execSync(`"${WRAPPER}" --mem`, { encoding: "utf8" }).match(/avail_phys_mb=(\d+)/) || [])[1];
  } catch {}
  return { procs, free };
}

/** A logger that writes target\build-ab\<name>.log as well as the console. */
function logger(name) {
  fs.mkdirSync(OUT, { recursive: true });
  const file = path.join(OUT, `${name}.log`);
  return (m) => {
    const line = `${new Date().toISOString()} ${m}`;
    console.log(line);
    fs.appendFileSync(file, line + "\n");
  };
}

/** The two debug-info variants: a = Cargo.toml as it is, b = the brief's proposal. */
const VARIANTS = {
  a: [],
  b: ["--config", 'profile.dev.debug="line-tables-only"', "--config", 'profile.dev.package."*".debug=false'],
};

module.exports = { ROOT, OUT, WRAPPER, SLOT_LOG, ensureWrapper, load, logger, VARIANTS };
