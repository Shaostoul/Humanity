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
//   node scripts/verify-runtime.js [--exe PATH] [--timeout-min N] [--wait-min N]
//                                  [--wait-builds] [--keep-open]
//                                  [--operator-config [--operator-config-path F]]
//                                  [--allow-other-build "<reason>"] [--rig DIR]
//   (--allow-other-build runs a build that is NOT this tree, on purpose, e.g. a
//   red check against an old build: passed to scripts/check-fresh-exe.js and
//   recorded in the sweep manifest as other_build)
//   node scripts/verify-runtime.js --dry-verdict <manifest.json> [--log <run.log>]
// Exit 0 = every vantage reached the world and captured, zero panics.
// Exit 1 = refused to run (stale binary, missing vantage, rig already busy, an
//          unknown flag, a machine that never went quiet): nothing was judged.
// Exit 2 = the app failed the runtime gate. Read the lines above the verdict.
//
// TWO CLOCKS (2026-10-21). Before the game starts, the sweep waits for the
// machine (lib/machine-guard.js: other games, builds, another rig's boot lock),
// up to --wait-min (60). That wait is NOT the gate's business: a machine that
// never goes quiet is "REFUSED: the machine never went quiet in N min; nothing
// was booted", exit 1. The gate's own clock, --timeout-min (12), starts when
// the game process starts (lib/game-launch.js spawnGame tells this script
// through HUMANITY_RIG_BOOT_SIGNAL) and times boot, world entry and the
// vantages. It used to start with the recipe, so on 2026-10-21, with other
// agents building, the guard waited 577 s to 12 min, the gate killed the probe
// before anything booted, and printed "FAIL ... A hang here is a boot or
// world-entry hang, not slowness": two false failures, neither of which booted.
//
// The sweep waits for other games and rigs and for free physical memory, not for
// builds (lib/machine-guard.js, 2026-10-23): the gate reads panics and captures,
// not timings, so a build beside it only slows it down. --wait-builds (forwarded
// to the sweep) waits for builds too. Every flag this script does not know is
// refused, not ignored (--game-only, the old opposite of that flag, was silently
// dropped here until 2026-10-21, and is refused now that it means nothing).
// --rig DIR: the rig folder (default .probe-rig/verify). For the tests, so a
// test never touches a real gate's rig.
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
// ONE MACHINE, in one place: scripts/lib/machine-guard.js. This gate used to
// carry its own copy of the process query, verify-screens.js carried a second
// copy, and probe-sweep.js carried none at all - so a fix to one never reached
// the others. This gate reads panics and captures rather than fps, so it keeps
// gating on the GAME only (listInstances); the sweep it spawns applies the
// wider build-aware guard to its own capture windows.
const MG = require("./lib/machine-guard.js");
// Which shader compiler the probe's game ran, for the verdict (FXC is minutes slower).
const DXC = require("./lib/dxc-dlls.js");
// The freshness gate, run through its one runner so --allow-other-build reaches
// it and comes back as the other_build record for the manifest (BUG-133).
const { runFreshGate, otherBuildNotice, allowOtherFrom } = require("./lib/src-fingerprint.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
// Every flag this gate knows. Anything else is REFUSED, not ignored: an
// ignored flag is a check that silently did not run (a forwarding flag was dropped
// here, unread, until 2026-10-21).
const VALUE_FLAGS = [
  "--exe",
  "--timeout-min",
  "--wait-min",
  "--rig",
  "--operator-config-path",
  "--allow-other-build",
  "--dry-verdict",
  "--dry-exit",
  "--log",
];
const BOOL_FLAGS = ["--keep-open", "--operator-config", "--wait-builds"];
function unknownArgs(argv) {
  const bad = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (BOOL_FLAGS.includes(a)) continue;
    if (VALUE_FLAGS.includes(a)) {
      // --allow-other-build without a reason is the freshness gate's to refuse.
      if (a === "--allow-other-build" && (argv[i + 1] === undefined || argv[i + 1].startsWith("--"))) continue;
      i++;
      continue;
    }
    bad.push(a);
  }
  return bad;
}
{
  const bad = unknownArgs(args);
  if (bad.length) {
    refuse([
      `verify-runtime: REFUSED - unknown argument(s): ${bad.join(" ")}. Nothing was booted.`,
      "An argument this gate does not read would be silently ignored, and the run would not be the one asked for.",
      `Known: ${[...VALUE_FLAGS.map((f) => `${f} <value>`), ...BOOL_FLAGS].join("  ")}`,
    ]);
  }
}
const KEEP_OPEN = args.includes("--keep-open");
const WAIT_BUILDS = args.includes("--wait-builds");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const TIMEOUT_MIN = Number(opt("--timeout-min", "12"));
const WAIT_MIN = Number(opt("--wait-min", String(MG.WAIT_MIN_DEFAULT)));
for (const [flag, v] of [["--timeout-min", TIMEOUT_MIN], ["--wait-min", WAIT_MIN]]) {
  // NaN would make setTimeout fire at once: the gate would kill its own probe.
  if (!(v > 0)) refuse([`verify-runtime: REFUSED - ${flag} must be a number of minutes above 0, got "${opt(flag, "")}". Nothing was booted.`]);
}
/** The gate's clock: from the moment the game process starts. */
const TIMEOUT_MS = TIMEOUT_MIN * 60 * 1000;
/** The sweep's wait for the machine, before the game starts: not on the gate's clock. */
const WAIT_MS = WAIT_MIN * 60 * 1000;
/** Ours only if the sweep's own --wait-min refusal never comes (a hung process query). */
const MACHINE_BACKSTOP_MS = WAIT_MS + 10 * 60 * 1000;
const DRY = opt("--dry-verdict", null);
const DRY_EXIT = Number(opt("--dry-exit", "0"));

// Its OWN rig, not .probe-rig: two or three Claude sessions share this checkout
// and a concurrent `just probe-sweep` would fight this run over the same exe
// copy and the same debug/*.json IPC files. Nested inside .probe-rig so the
// existing gitignore entry covers it. --rig for the tests only.
const RIG = path.resolve(opt("--rig", path.join(REPO, ".probe-rig", "verify")));

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
// Set by the freshness gate in step 3 when --allow-other-build let a different
// build through; null otherwise (and always null in --dry-verdict mode, which
// reads the record back out of the manifest instead).
let OTHER_BUILD = null;
// THE TWO CLOCKS (see the top). Declared here, before --dry-verdict calls
// report(), which reads them. PHASE is "waiting" until the sweep says the game
// has started, then "booted"; REFUSAL is the sweep's own refusal record (a
// machine that never went quiet); TIMED_OUT names which clock ran out.
let PHASE = "waiting";
let BOOTED = null;
let REFUSAL = null;
let TIMED_OUT = null;
let GATE_START = null;
const T0 = Date.now();
const fmt = MG.fmtSpan;

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
const runningInstances = MG.listInstances;
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

// ── 3. The binary must be this tree's build ──────────────────────────────────
// Or another build ON PURPOSE (--allow-other-build "<reason>", a red check
// against an old build): the gate says so loudly and report() writes the
// record into the sweep's manifest as other_build (BUG-133).
const fresh = runFreshGate(EXE, args, { cwd: REPO });
if (fresh.status !== 0) {
  console.error("verify-runtime: REFUSED - see the freshness failure above. Nothing was booted.");
  process.exit(1);
}
OTHER_BUILD = fresh.other_build;

// ── 4. Boot it ───────────────────────────────────────────────────────────────
const sweepArgs = [
  path.join(__dirname, "probe-sweep.js"),
  "--rig", RIG,
  "--exe", EXE,
  "--out", OUT,
  "--only", RUNTIME_VANTAGES.join(","),
];
if (KEEP_OPEN) sweepArgs.push("--keep-open");
// The sweep's wait for the machine has its own limit and its own refusal; the
// gate passes its number down so both say the same thing.
sweepArgs.push("--wait-min", String(WAIT_MIN));
if (WAIT_BUILDS) sweepArgs.push("--wait-builds");
// probe-sweep runs the freshness gate itself (BUG-133: every script that
// boots the game does), so a build this gate let through ON PURPOSE must
// reach it with the same reason, or the sweep would refuse what was allowed.
const allowOther = allowOtherFrom(args);
if (allowOther !== undefined) sweepArgs.push("--allow-other-build", allowOther);
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
console.log(
  `[gate] the wait for the machine (up to ${fmt(WAIT_MS)}${WAIT_BUILDS ? ", builds included (--wait-builds)" : ", for other games, rigs and free memory, not builds"}) is not timed;` +
    ` the ${fmt(TIMEOUT_MS)} gate clock starts when the game does`
);
console.log("");

// Where the sweep says what phase it is in (lib/machine-guard.js signalPhase).
const SIGNAL = path.join(RIG, "boot-signal.json");
fs.mkdirSync(RIG, { recursive: true });
fs.rmSync(SIGNAL, { force: true });
// The sweep writes the game's pid here right after it starts it; one left by an
// earlier run would read as this run's boot.
const PID_FILE = path.join(RIG, "probe_pid.txt");
fs.rmSync(PID_FILE, { force: true });

const child = spawn(process.execPath, sweepArgs, { cwd: REPO, stdio: "inherit", env: { ...process.env, [MG.BOOT_SIGNAL_ENV]: SIGNAL } });

function killProbe() {
  try {
    execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" });
  } catch {}
  // probe-sweep spawns the game detached, so killing node is not enough.
  try {
    const pid = fs.readFileSync(PID_FILE, "utf8").trim();
    if (pid) execSync(`taskkill /PID ${pid} /F`, { stdio: "ignore" });
  } catch {}
}

// Clock 1, the machine: only a backstop. The sweep refuses by itself at
// --wait-min; this fires only if it never does, and still means nothing booted.
const machineWatchdog = setTimeout(() => {
  if (PHASE !== "waiting") return;
  TIMED_OUT = "machine";
  console.error(`\n[gate] the probe was still short of booting ${fmt(MACHINE_BACKSTOP_MS)} after it started (its own wait ends at ${fmt(WAIT_MS)}) - stopping it. Nothing was booted.`);
  killProbe();
}, MACHINE_BACKSTOP_MS);

// Clock 2, the gate: from the moment the game process exists. The sweep's
// spawnGame writes "booted" to SIGNAL; a probe_pid.txt written during this run
// counts too (the sweep writes it right after the spawn), so a signal that
// failed to write can never leave a booted game untimed.
let gateWatchdog = null;
function readPhase(atExit = false) {
  const s = MG.readPhase(SIGNAL);
  if (s && s.phase === "refused") REFUSAL = s;
  let booted = s && s.phase === "booted" ? s : null;
  if (!booted) {
    try {
      booted = { phase: "booted", game_pid: Number(fs.readFileSync(PID_FILE, "utf8").trim()) || null, from: "probe_pid.txt" };
    } catch {}
  }
  if (!booted || PHASE !== "waiting") return;
  PHASE = "booted";
  BOOTED = booted;
  GATE_START = Date.now();
  clearTimeout(machineWatchdog);
  // Seen only as the probe exited (a game that died within a poll): it did boot, so the
  // verdict below is about the build; there is no clock left to start.
  if (atExit) return;
  console.log(
    `[gate] the game started (pid ${booted.game_pid}) ${Math.round((GATE_START - T0) / 1000)} s after the probe did;` +
      ` that wait is not timed. The ${fmt(TIMEOUT_MS)} gate clock starts now.`
  );
  gateWatchdog = setTimeout(() => {
    TIMED_OUT = "gate";
    console.error(`\n[gate] TIMEOUT: ${fmt(TIMEOUT_MS)} since the game started - killing the probe.`);
    killProbe();
  }, TIMEOUT_MS);
}
const phasePoll = setInterval(readPhase, 250);

child.on("exit", (code) => {
  clearInterval(phasePoll);
  readPhase(true); // a phase written just before the exit
  clearTimeout(machineWatchdog);
  clearTimeout(gateWatchdog);
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
  if (!fs.existsSync(LOG)) {
    return PHASE === "booted"
      ? "(no run.log at all - the game process started but never wrote its log)"
      : "(no run.log at all - the exe never started)";
  }
  return fs.readFileSync(LOG, "utf8").split(/\r?\n/).filter(Boolean).slice(-n).join("\n");
}

function report(sweepExit) {
  // A dry verdict must never read like a real gate run in a pasted transcript.
  const VERDICT = DRY ? "DRY VERDICT (nothing was booted): " : "RESULT: ";
  const manifestPath = path.join(OUT, "manifest.json");
  console.log("");
  console.log("-".repeat(72));
  // WHICH SHADER COMPILER the game ran (2026-10-21): FXC's pipeline build takes
  // minutes, so a run on it is said loudly, here in the verdict, and not only in
  // a setup line long scrolled away. From the sweep's manifest, else from the
  // "booted" signal spawnGame wrote (a run that died before its manifest).
  let manifestCompiler = null;
  try {
    manifestCompiler = JSON.parse(fs.readFileSync(manifestPath, "utf8")).shader_compiler || null;
  } catch {}
  const compiler = manifestCompiler || (BOOTED && BOOTED.shader_compiler) || null;
  if (compiler === "FXC") {
    for (const l of DXC.fxcBanner({ exe: path.join(RIG, "HumanityOS.exe"), pid: BOOTED && BOOTED.game_pid, rig: "the probe", where: DXC.pairHome(REPO) })) console.log(l);
  } else if (compiler) {
    console.log(`shader compiler  ${compiler}`);
  }

  if (!fs.existsSync(manifestPath) && !DRY && PHASE === "waiting") {
    // NOTHING BOOTED: the sweep stopped before it started the game. Not a
    // verdict on the build in either direction, and never exit 2: on
    // 2026-10-21 this branch did not exist, and a run killed while it waited
    // for other agents' builds read "FAIL ... a boot or world-entry hang".
    const why = REFUSAL
      ? String(REFUSAL.headline || "").replace(/^.*?REFUSED:\s*/, "")
      : TIMED_OUT === "machine"
        ? `the probe's wait for the machine had not ended after ${fmt(MACHINE_BACKSTOP_MS)} and was stopped; nothing was booted.`
        : `the probe stopped before it started the game (probe exit ${sweepExit}); nothing was booted.`;
    console.log(`${VERDICT}REFUSED  ${why}`);
    console.log("");
    if (REFUSAL && REFUSAL.blockers && REFUSAL.blockers.length) {
      console.log(`Still up after ${REFUSAL.waited_s} s of waiting:`);
      for (const b of REFUSAL.blockers) console.log(`  ${b}`);
      console.log("");
    } else if (!REFUSAL && TIMED_OUT !== "machine") {
      console.log("Its reason is in the probe's lines above (the freshness gate, the rig setup, or the start itself).");
      console.log("");
    }
    console.log("This is not a verdict on the build: it never ran, so it neither passed nor failed.");
    if (REFUSAL) {
      console.log("Run again when the machine is quieter; --wait-min N waits longer. A short memory clears as");
      console.log("compiles finish (just build-slot-status shows them); builds are waited for only with --wait-builds.");
    }
    console.log("-".repeat(72));
    process.exit(1);
  }

  if (!fs.existsSync(manifestPath)) {
    // probe-sweep throws (and writes no manifest) when boot or world entry
    // dies, which is precisely the failure this gate was built for. Say so
    // with the panic in hand, not just an exit code.
    if (OTHER_BUILD) {
      // No manifest to carry the record, so it goes beside where one would be.
      fs.mkdirSync(OUT, { recursive: true });
      fs.writeFileSync(path.join(OUT, "other_build.json"), JSON.stringify({ other_build: OTHER_BUILD }, null, 2));
      console.log(otherBuildNotice(OTHER_BUILD));
    }
    console.log(`${VERDICT}FAIL  the probe never got far enough to write a manifest (probe exit ${sweepExit}).`);
    console.log("");
    if (TIMED_OUT === "gate") {
      // Only the gate's own clock gets here, and it starts with the game, so
      // this IS a hang of the game: the wait in front of it was not counted.
      console.log(`The run was killed ${fmt(TIMEOUT_MS)} after the game started (the ${Math.round((GATE_START - T0) / 1000)} s before that,`);
      console.log("waiting for the machine and setting up the rig, are not counted). A hang here is a boot or");
      console.log("world-entry hang, not slowness or a busy machine: a healthy sweep of 3 vantages takes ~3 minutes.");
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

  // WHICH BUILD this verdict is about. probe-sweep wrote the manifest and knows
  // nothing of the gate, so a deliberate other-build run is recorded here.
  if (OTHER_BUILD) {
    m.other_build = OTHER_BUILD;
    fs.writeFileSync(manifestPath, JSON.stringify(m, null, 2));
  }
  if (m.other_build) console.log(otherBuildNotice(m.other_build));

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
