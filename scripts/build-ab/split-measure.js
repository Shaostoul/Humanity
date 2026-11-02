#!/usr/bin/env node
// The crate split's build times (docs/design/crate-split.md: every rung says "Measure"; 2026-11-01).
//
//   node scripts/build-ab/split-measure.js <label> <low file> <engine file> [check,test]
//
// For `cargo check --features native` and the lib test build (`node scripts/lib-tests.js --list`,
// which builds every member's test program, as `just test-lib` does): clean every workspace member,
// a cold build, then one comment line appended to <low file> (a file of the lowest crate, the core)
// and a rebuild, then the same to <engine file> and a rebuild, then both files restored byte for
// byte (whatever happens) and an untimed rebuild that leaves the tree built as committed.
//
// A row's compile times are the build slot's own "done <crate> (<kind>) in N s" lines, so time a
// compile spent waiting for memory behind another session's is not counted; only the lines of
// this checkout's compiles (the slot's held-/wait- files name the cwd) inside the row's time
// window are read. The machine's load is logged at each row's start: COMPARE ROWS FROM A QUIET
// MACHINE ONLY. Rung 1's first "before" run, beside other sessions' compiles, read 86 s for an
// edit the quiet machine compiled in 53 s. Measure main and the branch in the same checkout, the
// same evening (check out main's commit, run with label "before", check the branch out again, run
// with label "after"). Results: target\build-ab\split-measure-<label>.log, one JSON line a row.

"use strict";

const { spawn, execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const T = require("./tools.js");

const [label, lowFile, engineFile, which = "check,test"] = process.argv.slice(2);
if (!label || !lowFile || !engineFile) {
  console.error("usage: node scripts/build-ab/split-measure.js <label> <low file> <engine file> [check,test]");
  process.exit(2);
}
const SLOT_DIR = path.dirname(T.SLOT_LOG);
const BUILDS = { check: "cargo check --features native", test: "node scripts/lib-tests.js --list" };
const env = { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || "4" };
fs.mkdirSync(T.OUT, { recursive: true });
const out = path.join(T.OUT, `split-measure-${label}.log`);

/** Cargo, rustc and link processes, HumanityOS instances and free physical memory: the load. */
function load() {
  let procs = "?", games = "?", free = "?";
  try {
    const names = execSync("tasklist /FO CSV /NH", { encoding: "utf8" }).split(/\r?\n/).filter(Boolean)
      .map((l) => l.split('","')[0].replace(/^"/, "").toLowerCase());
    procs = names.filter((n) => n === "cargo.exe" || n === "rustc.exe" || n === "link.exe").length;
    games = names.filter((n) => n.includes("humanityos")).length;
  } catch {}
  try {
    const kb = execSync('powershell -NoProfile -Command "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory"', { encoding: "utf8" });
    free = `${(Number(kb.trim()) / 1024 / 1024).toFixed(1)} GB`;
  } catch {}
  return { cargo_rustc_link: procs, humanityos: games, free };
}

/** The pids of this checkout's compiles the slot holds or makes wait right now. */
function ours(seen) {
  let files = [];
  try { files = fs.readdirSync(SLOT_DIR); } catch { return; }
  for (const f of files) {
    if (!/^(held|wait)-\d+\.txt$/.test(f)) continue;
    let text = "";
    try { text = fs.readFileSync(path.join(SLOT_DIR, f), "utf8"); } catch { continue; }
    const kv = Object.fromEntries(text.split(/\r?\n/).filter((l) => l.includes("=")).map((l) => [l.slice(0, l.indexOf("=")), l.slice(l.indexOf("=") + 1)]));
    if ((kv.cwd || "").toLowerCase() === T.ROOT.toLowerCase()) seen.add(kv.pid);
  }
}

/** The slot's compile and wait lines of those pids between two times (pids get reused later). */
function slotLines(pids, t0, t1) {
  let text = "";
  try { text = fs.readFileSync(T.SLOT_LOG, "utf8"); } catch {}
  const compiles = [];
  let waited = 0;
  for (const line of text.split(/\r?\n/)) {
    const head = /^(\d+) \S+ pid (\d+) (.*)$/.exec(line);
    if (!head || Number(head[1]) < t0 - 1000 || Number(head[1]) > t1 + 500 || !pids.has(head[2])) continue;
    const d = /^done (\S+) \(([^)]+)\) in (\d+) s \(cpu (\d+) s\), peak (\d+) MB/.exec(head[3]);
    if (d) compiles.push(`${d[1]} ${d[2]} ${d[3]} s (cpu ${d[4]} s, peak ${d[5]} MB)`);
    const w = /after waiting (\d+) s/.exec(head[3]);
    if (w) waited += Number(w[1]);
  }
  return { compiles, waited };
}

function row(name, cmd) {
  return new Promise((resolve) => {
    const before = load();
    const seen = new Set();
    const t0 = Date.now();
    const child = spawn("bash", ["-c", cmd], { cwd: T.ROOT, env, stdio: ["ignore", "pipe", "pipe"] });
    let text = "";
    child.stdout.on("data", (d) => { text += d; });
    child.stderr.on("data", (d) => { text += d; });
    const poll = setInterval(() => ours(seen), 500);
    child.on("close", (code) => {
      clearInterval(poll);
      ours(seen);
      const t1 = Date.now();
      const { compiles, waited } = slotLines(seen, t0, t1);
      const r = { name, cmd, exit: code, wall_s: Math.round((t1 - t0) / 100) / 10, waited_s: waited, compiles, load_at_start: before, at: new Date(t1).toISOString() };
      fs.appendFileSync(out, JSON.stringify(r) + "\n");
      console.log(`${name}: exit ${code}, wall ${r.wall_s} s${waited ? `, waited ${waited} s` : ""}\n    ${compiles.join("\n    ") || "(no compile of ours)"}`);
      if (code !== 0) console.log(text.slice(-4000));
      resolve(r);
    });
  });
}

(async () => {
  const meta = JSON.parse(execSync("cargo metadata --no-deps --format-version 1", { cwd: T.ROOT, encoding: "utf8", maxBuffer: 1 << 28 }));
  const clean = "cargo clean " + meta.packages.map((p) => `-p ${p.name}`).join(" ");
  const lowAbs = path.join(T.ROOT, lowFile), engAbs = path.join(T.ROOT, engineFile);
  const lowOrig = fs.readFileSync(lowAbs), engOrig = fs.readFileSync(engAbs);
  const touch = (file, orig) => fs.writeFileSync(file, Buffer.concat([orig, Buffer.from(`\n// split-measure ${Date.now()}\n`)]));
  try {
    for (const b of which.split(",")) {
      const cmd = BUILDS[b];
      if (!cmd) throw new Error(`no build named ${b} (check, test)`);
      await row(`${b}: clean`, clean);
      await row(`${b}: cold`, cmd);
      touch(lowAbs, lowOrig);
      await row(`${b}: one-line edit in ${lowFile}`, cmd);
      touch(engAbs, engOrig);
      await row(`${b}: one-line edit in ${engineFile}`, cmd);
      fs.writeFileSync(lowAbs, lowOrig);
      fs.writeFileSync(engAbs, engOrig);
      await row(`${b}: back to the tree as committed (untimed)`, cmd);
    }
  } finally {
    fs.writeFileSync(lowAbs, lowOrig);
    fs.writeFileSync(engAbs, engOrig);
  }
  console.log(`results: ${out}`);
})();
