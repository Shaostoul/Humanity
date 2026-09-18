#!/usr/bin/env node
// Perf report over a probe-sweep manifest: a fps/frame-time table across the
// canonical vantages, flagging any stop below its advisory floor, and (when a
// baseline is given) the per-vantage delta so a regression is obvious.
//
// Usage:
//   node scripts/perf-report.js [manifest.json] [--baseline other-manifest.json]
// With no manifest arg it reads .probe-rig/latest-sweep.txt (the newest sweep).
//
// Exit 0 if every captured vantage is at/above its floor; exit 2 otherwise, so
// `just perf-sweep` and CI can gate on it.

const fs = require("fs");
const path = require("path");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const baselineArg = (() => {
  const i = args.indexOf("--baseline");
  return i >= 0 ? args[i + 1] : null;
})();
let manifestPath = args.find((a) => !a.startsWith("--") && a !== baselineArg);

if (!manifestPath) {
  const pointer = path.join(REPO, ".probe-rig", "latest-sweep.txt");
  if (!fs.existsSync(pointer)) {
    console.error("no manifest given and no .probe-rig/latest-sweep.txt - run `just probe-sweep` first");
    process.exit(1);
  }
  manifestPath = path.join(fs.readFileSync(pointer, "utf8").trim(), "manifest.json");
}
const m = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const base = baselineArg && fs.existsSync(baselineArg) ? JSON.parse(fs.readFileSync(baselineArg, "utf8")) : null;
const baseById = base ? Object.fromEntries(base.vantages.map((v) => [v.id, v])) : {};

const pad = (s, n) => String(s).padEnd(n);
const padL = (s, n) => String(s).padStart(n);

console.log(
  `\nPerf sweep  ${m.stamp}   captured ${m.captured}/${m.total}   panics ${m.panics}` +
    (m.contaminated ? `   CONTAMINATED ${m.contaminated}` : "")
);
// A manifest with no `contaminated` key at all predates the mid-sweep machine
// guard (2026-09-18), so nothing in it can say whether a second renderer or a
// build was up. Say so rather than letting the table imply a clean machine.
if (!("contaminated" in m)) {
  console.log("machine     UNRECORDED (manifest predates the mid-sweep guard - cannot say what else was running)");
} else if (m.machine_guard && m.machine_guard.pre_boot_wait_s > 0) {
  console.log(`machine     waited ${m.machine_guard.pre_boot_wait_s} s for a competing process before booting`);
}
// Which settings produced these numbers. A sweep manifest from before
// 2026-08-02 has no record at all, and that is worth saying out loud rather
// than letting the table imply the readings are comparable to a current one.
{
  const g = m.graphics || {};
  const src = m.graphics_source;
  const label = {
    "operator-mirrored": "operator-mirrored ",
    "rig-config-matches-operator": "rig = operator    ",
  }[src] || "RIG DEFAULTS      ";
  if (!src) {
    console.log("graphics    UNRECORDED (manifest predates settings provenance - not comparable)");
  } else {
    console.log(`graphics    ${label}  ssao ${g.ssao_strength}  veg ${g.veg_density}  fog ${g.fog_density}  rd ${g.render_distance}  tree ${g.tree_model_distance}  godray ${g.godray_intensity}`);
  }
}
console.log("-".repeat(base ? 78 : 60));
console.log(
  pad("vantage", 26) + padL("fps", 8) + padL("ms", 8) + padL("floor", 8) + (base ? padL("Δfps", 10) : "") + "  status"
);
console.log("-".repeat(base ? 78 : 60));

let worst = 0;
let belowFloor = 0;
let contaminated = 0;
for (const v of m.vantages) {
  // ONE MACHINE, for the whole sweep (scripts/lib/machine-guard.js). A capture
  // whose window overlapped another renderer or a build is not a reading of
  // this build. Both shapes happened on 2026-09-18: a mid-sweep second instance
  // put 75.79 ms in a table where every clean boot read 66.8 to 67.1, and a
  // concurrent cargo/rustc read 6.6 fps at the limb against a 15 ms GPU sum. So
  // this report does not grade such a capture, does not compare it to a
  // baseline, and exits non-zero - a missing number is recoverable, a wrong one
  // is not.
  if (v.contaminated) {
    contaminated++;
    const who =
      (v.contaminated_by || [])
        .map((p) => `${p.name || "HumanityOS.exe"} ${p.pid}`)
        .join(", ") || "unknown process";
    console.log(
      pad(v.id, 26) + padL(v.fps ?? "-", 8) + padL(v.frame_ms ?? "-", 8) + padL(v.perf_floor_fps ?? "-", 8) +
        (base ? padL("-", 10) : "") + `  CONTAMINATED (${who})`
    );
    worst = 2;
    continue;
  }
  if (!v.ok) {
    console.log(pad(v.id, 26) + padL("-", 8) + padL("-", 8) + padL("-", 8) + (base ? padL("-", 10) : "") + "  CAPTURE FAILED");
    worst = 2;
    continue;
  }
  const floor = v.perf_floor_fps;
  const under = floor != null && v.fps != null && v.fps < floor;
  if (under) belowFloor++;
  let delta = "";
  if (base) {
    const b = baseById[v.id];
    if (b && b.ok && typeof b.fps === "number" && typeof v.fps === "number") {
      const d = Math.round((v.fps - b.fps) * 10) / 10;
      delta = padL((d >= 0 ? "+" : "") + d.toFixed(1), 10);
      // A >25% drop vs baseline is a regression even if still above floor.
      if (b.fps > 0 && v.fps < b.fps * 0.75) worst = Math.max(worst, 2);
    } else {
      delta = padL("-", 10);
    }
  }
  const status = under ? "BELOW FLOOR" : "ok";
  if (under) worst = Math.max(worst, 2);
  console.log(
    pad(v.id, 26) + padL(v.fps, 8) + padL(v.frame_ms, 8) + padL(floor ?? "-", 8) + delta + "  " + status
  );
}
console.log("-".repeat(base ? 78 : 60));
if (contaminated) {
  console.log(`${contaminated} vantage(s) CONTAMINATED: another renderer or a build shared the machine during`);
  console.log("the capture. Those numbers are not readings of this build and were NOT graded. For a clean sweep:");
  console.log('  tasklist //FI "IMAGENAME eq HumanityOS.exe"    # wait until only your own rig is listed');
  console.log('  tasklist //FI "IMAGENAME eq rustc.exe"         # and until no build is compiling');
  console.log("  just probe-sweep --only <vantage>              # the guard now waits for you before booting");
}
if (belowFloor) console.log(`${belowFloor} vantage(s) below the advisory fps floor.`);
if (m.panics) console.log(`WARNING: ${m.panics} PANIC(s) in the probe log this sweep.`);
if (worst === 0) console.log("All captured vantages at/above floor.\n");
process.exit(worst === 0 && m.panics === 0 ? 0 : 2);
