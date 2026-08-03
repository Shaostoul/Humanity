#!/usr/bin/env node
// verify-runtime: the boot half of verification. `just verify` is entirely
// static (cargo checks, lib tests, file-scanner lints) and never starts the
// app, which is how v0.1029-v0.1038 shipped TEN releases that panicked the
// instant the player entered the world while every check stayed green, and how
// v0.782-v0.784 shipped three releases that could not create a GPU device at
// all. Both failures live past the last line `just verify` executes.
//
// This gate boots the real release binary in the portable probe rig, enters the
// world through autopilot, and visits a small fixed set of vantages chosen to
// touch the code paths that historically died. It fails if any vantage cannot
// be reached or captured, or if the run log contains a single PANIC.
//
// It is deliberately NOT part of `just verify`: verify runs in headless CI with
// no GPU. Run this locally before pushing anything renderer, shader, world or
// asset shaped.
//
// Usage:
//   node scripts/verify-runtime.js [--exe PATH] [--timeout-min N] [--keep-open]
//                                  [--operator-config [--operator-config-path F]]
//   node scripts/verify-runtime.js --dry-verdict <manifest.json> [--log <run.log>]
// Exit 0 = every vantage reached the world and captured, zero panics.
// Exit 1 = refused to run (stale binary, missing vantage, rig already busy).
// Exit 2 = the app failed the runtime gate. Read the lines above the verdict.
//
// --dry-verdict re-reads an EXISTING sweep manifest and prints the same verdict
// without booting anything. It is how the verdict logic itself gets proven (feed
// it a manifest with a failed vantage or panics > 0 and watch it go red) and how
// you re-read an old sweep on a machine with no GPU. Its output is prefixed
// "DRY VERDICT (nothing was booted)" so it can never be pasted somewhere and
// mistaken for a real gate run.

const fs = require("fs");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const KEEP_OPEN = args.includes("--keep-open");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const TIMEOUT_MS = Number(opt("--timeout-min", "12")) * 60 * 1000;
const DRY = opt("--dry-verdict", null);
const DRY_EXIT = Number(opt("--dry-exit", "0"));

// Its OWN rig, not .probe-rig: two or three Claude sessions share this checkout
// and a concurrent `just probe-sweep` would fight this run over the same exe
// copy and the same debug/*.json IPC files. Nested inside .probe-rig so the
// existing gitignore entry covers it.
const RIG = path.join(REPO, ".probe-rig", "verify");

// THE GATE SET. Small on purpose (a gate nobody runs protects nothing) and
// fixed on purpose (no --only flag, so the gate cannot be quietly shrunk to
// whatever passes today). Each entry earns its place from a real incident:
//
//   blue-marble-12000km  full-disc Earth from space. Planet LOD, atmosphere
//                        shell, star/sky pipelines - the "does the renderer
//                        come up and draw a world at all" case.
//   fuji-forest-ground   ground level in a conifer forest. Loads TEXTURED
//                        materials, which is what builds the per-material
//                        albedo bind group lazily - the exact site of the
//                        v0.1029-1038 world-entry panic that menu-only boot
//                        checks could not see.
//   ocean-storm-low      low over a storm sea. Water shell + wave textures +
//                        buoyancy, the most numerically fragile scene we have.
//
// Adding a vantage here is cheap (about 30 s of wall clock). Removing one needs
// a stated reason, same as an allowlist entry.
const RUNTIME_VANTAGES = ["blue-marble-12000km", "fuji-forest-ground", "ocean-storm-low"];

const pad = (s, n) => String(s).padEnd(n);
// Repo-relative when it helps, absolute when the path is outside the repo (a
// "..\..\Users\..." string helps nobody at 2am).
const rel = (p) => {
  const r = path.relative(REPO, p);
  return !r || r.startsWith("..") ? p : r;
};

function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

// ── 1. The vantage ids must still exist ──────────────────────────────────────
// probe-sweep's --only silently filters to whatever matches, so a renamed
// vantage would shrink this gate to two checks, or one, without a word. That is
// the "check that cannot fail" trap; catch it loudly instead.
const specPath = path.join(REPO, "tests", "visual", "vantages.json");
const spec = JSON.parse(fs.readFileSync(specPath, "utf8"));
const known = new Set(spec.vantages.map((v) => v.id));
const missing = RUNTIME_VANTAGES.filter((id) => !known.has(id));
if (missing.length) {
  refuse([
    `VANTAGE GONE: ${missing.join(", ")} no longer exists in ${rel(specPath)}.`,
    "This gate would have silently run a smaller set and still reported PASS.",
    "Fix: pick the replacement vantage id from that file and update RUNTIME_VANTAGES",
    `in ${rel(__filename)} (say in a comment why the new one covers the same ground).`,
  ]);
}

console.log("");
console.log(
  `verify-runtime  ${RUNTIME_VANTAGES.length} vantages, ${DRY ? "dry verdict over an existing manifest" : "real binary, real world entry"}`
);
console.log("");

// Where this run's evidence lands. Reassigned in --dry-verdict mode to point at
// the manifest being re-read.
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
let OUT = path.join(RIG, "sweeps", stamp);
let LOG = path.join(RIG, "logs", "run.log");

// ── 1b. --dry-verdict: verdict logic only, nothing boots ─────────────────────
if (DRY) {
  const manifest = path.resolve(DRY);
  if (!fs.existsSync(manifest)) refuse([`--dry-verdict: no such manifest: ${manifest}`]);
  OUT = path.dirname(manifest);
  const sibling = path.join(OUT, "run.log");
  LOG = path.resolve(opt("--log", fs.existsSync(sibling) ? sibling : LOG));
  console.log(`[dry] verdict only, nothing booted: ${rel(manifest)}`);
  console.log("");
  report(DRY_EXIT);
}

// ── 2. Nobody else may be driving this rig ───────────────────────────────────
// Environment precondition, checked BEFORE the binary one: if another probe
// owns the rig you cannot run at all, whatever binary you were going to test.
function runningInstances() {
  try {
    const out = execSync(
      'powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \\"Name=\'HumanityOS.exe\'\\" | ForEach-Object { $_.ProcessId.ToString() + \'|\' + $_.ExecutablePath }"',
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }
    );
    return out
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l) => {
        const [pid, exe] = l.split("|");
        return { pid: Number(pid), exe: exe || "" };
      });
  } catch {
    return []; // not Windows, or no powershell: degrade to no-op
  }
}
const rigExe = path.join(RIG, "HumanityOS.exe");
const instances = runningInstances();
const mine = instances.filter((p) => p.exe.toLowerCase() === rigExe.toLowerCase());
if (mine.length) {
  refuse([
    `RIG BUSY: a verify-runtime probe is already running (pid ${mine.map((p) => p.pid).join(", ")}).`,
    "Two runs would fight over the same exe copy and the same debug/*.json IPC files,",
    "so the result would mean nothing. Wait for it, or if it is a leftover:",
    `  taskkill //PID ${mine[0].pid} //F`,
  ]);
}
const others = instances.filter((p) => p.exe.toLowerCase() !== rigExe.toLowerCase());
if (others.length) {
  console.log(
    `[note] ${others.length} other HumanityOS.exe instance(s) running (${others.map((p) => p.pid).join(", ")}).`
  );
  console.log("[note] this gate reads panics and captures, not fps, so the verdict still holds.");
}

// ── 3. The binary must be the current build ──────────────────────────────────
const fresh = spawnSync(process.execPath, [path.join(__dirname, "check-fresh-exe.js"), "--exe", EXE], {
  cwd: REPO,
  stdio: "inherit",
});
if (fresh.status !== 0) {
  console.error("verify-runtime: REFUSED - see the freshness failure above. Nothing was booted.");
  process.exit(1);
}

// ── 4. Boot it ───────────────────────────────────────────────────────────────
const sweepArgs = [
  path.join(__dirname, "probe-sweep.js"),
  "--rig", RIG,
  "--exe", EXE,
  "--out", OUT,
  "--only", RUNTIME_VANTAGES.join(","),
];
if (KEEP_OPEN) sweepArgs.push("--keep-open");
// Forwarded so this gate can run at the settings the operator actually plays
// at, which are heavier than the rig defaults on every axis (render_distance
// 2000 vs 500, tree_model_distance 300 vs 120) and therefore likelier to trip a
// world-entry failure. Either way the sweep records what it ran at, and the
// verdict block below prints it.
if (args.includes("--operator-config")) sweepArgs.push("--operator-config");
const opCfgPath = opt("--operator-config-path", null);
if (opCfgPath) sweepArgs.push("--operator-config-path", opCfgPath);

console.log(`[gate] rig ${rel(RIG)}`);
console.log(`[gate] vantages ${RUNTIME_VANTAGES.join(", ")}`);
console.log("");

const child = spawn(process.execPath, sweepArgs, { cwd: REPO, stdio: "inherit" });

let timedOut = false;
const watchdog = setTimeout(() => {
  timedOut = true;
  console.error(`\n[gate] TIMEOUT after ${TIMEOUT_MS / 60000} min - killing the probe.`);
  try {
    execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" });
  } catch {}
  // probe-sweep spawns the game detached, so killing node is not enough.
  try {
    const pid = fs.readFileSync(path.join(RIG, "probe_pid.txt"), "utf8").trim();
    if (pid) execSync(`taskkill /PID ${pid} /F`, { stdio: "ignore" });
  } catch {}
}, TIMEOUT_MS);

child.on("exit", (code) => {
  clearTimeout(watchdog);
  report(code === null ? -1 : code);
});

// ── 5. Verdict, derived from the manifest rather than trusting the exit code ──
function panicExcerpt() {
  if (!fs.existsSync(LOG)) return null;
  const txt = fs.readFileSync(LOG, "utf8");
  const i = txt.indexOf("PANIC");
  if (i < 0) return null;
  return txt
    .slice(Math.max(0, i - 200), i + 1200)
    .split(/\r?\n/)
    .slice(0, 22)
    .join("\n");
}
function logTail(n) {
  if (!fs.existsSync(LOG)) return "(no run.log at all - the exe never started)";
  return fs.readFileSync(LOG, "utf8").split(/\r?\n/).filter(Boolean).slice(-n).join("\n");
}

function report(sweepExit) {
  // A dry verdict must never read like a real gate run in a pasted transcript.
  const VERDICT = DRY ? "DRY VERDICT (nothing was booted): " : "RESULT: ";
  const manifestPath = path.join(OUT, "manifest.json");
  console.log("");
  console.log("-".repeat(72));

  if (!fs.existsSync(manifestPath)) {
    // probe-sweep throws (and writes no manifest) when boot or world entry
    // dies, which is precisely the failure this gate was built for. Say so
    // with the panic in hand, not just an exit code.
    console.log(`${VERDICT}FAIL  the probe never got far enough to write a manifest (probe exit ${sweepExit}).`);
    console.log("");
    if (timedOut) {
      console.log(`The run was killed after ${TIMEOUT_MS / 60000} minutes. A hang here is a boot or`);
      console.log("world-entry hang, not slowness: a healthy sweep of 3 vantages takes ~3 minutes.");
    } else {
      console.log("The app failed to boot or failed to enter the world. That is the exact class of");
      console.log("bug `just verify` cannot see, so treat it as a release blocker, not rig flake.");
    }
    const pan = panicExcerpt();
    console.log("");
    if (pan) {
      console.log("First PANIC in the probe log:");
      console.log(pan);
    } else {
      console.log(`Last lines of ${rel(LOG)}:`);
      console.log(logTail(12));
    }
    console.log("");
    console.log(`Full log: ${rel(LOG)}`);
    console.log("-".repeat(72));
    process.exit(2);
  }

  const m = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const byId = Object.fromEntries(m.vantages.map((v) => [v.id, v]));
  let failed = 0;

  // WHICH SETTINGS this verdict is about. A PASS at rig defaults is not a PASS
  // at the operator's settings: their render_distance is 4x and their
  // tree_model_distance 2.5x the rig's, so a world-entry or streaming failure
  // can live entirely outside what a default run touches.
  const g = m.graphics || {};
  if (!m.graphics_source) {
    console.log("settings  UNRECORDED (this manifest predates settings provenance)");
  } else {
    console.log(
      `settings  ${m.graphics_source}  ssao ${g.ssao_strength}  veg ${g.veg_density}  ` +
      `fog ${g.fog_density}  rd ${g.render_distance}  tree ${g.tree_model_distance}  godray ${g.godray_intensity}`
    );
  }
  console.log("");

  for (const id of RUNTIME_VANTAGES) {
    const v = byId[id];
    if (!v) {
      console.log(`FAIL  ${pad(id, 24)} never ran (missing from the sweep manifest)`);
      failed++;
    } else if (!v.ok) {
      console.log(`FAIL  ${pad(id, 24)} ${v.error || "capture failed"}`);
      failed++;
    } else {
      console.log(`PASS  ${pad(id, 24)} ${pad((v.fps ?? "?") + " fps", 12)}${pad((v.frame_ms ?? "?") + " ms", 11)}${v.screenshot}`);
    }
  }

  console.log("-".repeat(72));
  const panics = m.panics || 0;
  if (panics > 0) {
    console.log(`PANICS: ${panics} in the probe log this run.`);
    const pan = panicExcerpt();
    if (pan) {
      console.log("");
      console.log(pan);
      console.log("");
    }
  }

  if (failed === 0 && panics === 0 && sweepExit === 0) {
    console.log(`${VERDICT}PASS  ${RUNTIME_VANTAGES.length}/${RUNTIME_VANTAGES.length} vantages entered the world and captured, panics=0`);
    console.log(`        screenshots ${rel(OUT)}`);
    console.log("-".repeat(72));
    console.log("");
    process.exit(0);
  }
  // Every vantage looks fine but the rig itself exited non-zero: something went
  // wrong outside the per-vantage records (a kill, a rig-level throw). Never
  // upgrade that to a pass.
  if (failed === 0 && panics === 0) {
    console.log(`${VERDICT}FAIL  every vantage captured, but the probe rig exited ${sweepExit}.`);
    console.log("Something failed outside the per-vantage records. Read the rig output above and");
    console.log(`${rel(LOG)} before trusting this build.`);
    console.log("-".repeat(72));
    process.exit(2);
  }

  console.log(
    `${VERDICT}FAIL  ${RUNTIME_VANTAGES.length - failed}/${RUNTIME_VANTAGES.length} vantages captured, panics=${panics}`
  );
  console.log("");
  console.log("This binary is not shippable. What to do:");
  console.log(`  1. read ${rel(LOG)} (the panic hook always writes the cause there)`);
  console.log(`  2. look at ${rel(OUT)} for the frames that did capture`);
  console.log("  3. reproduce one vantage interactively:");
  console.log(`     node scripts/probe-sweep.js --only ${RUNTIME_VANTAGES[0]} --keep-open`);
  console.log("-".repeat(72));
  process.exit(2);
}
