#!/usr/bin/env node
// verify-copresence: proves, in the REAL game with nobody at the keyboard,
// that a second player is drawn and walks smoothly.
//
// WHY: the week plan's Day 3 (v0.1440.0) shipped scripts/second-player.js (a
// scripted player that signs in as a real identity, joins the shared world and
// walks a path) and smooth drawing of other players in src/net/sync.rs
// (snapshot interpolation on the sender's clock). Unit tests and a relay test
// cover both, but they feed the drawing made-up deliveries; nothing proved
// that the game a person runs draws the figure at all, let alone smoothly.
// That is the "verify on the state the player actually reaches" rule. This
// rig closes it.
//
// WHAT IT DOES, one plain line per step:
//   1. guard   refuses (exit 1) while ANY HumanityOS.exe runs (one GPU, one
//              instance; scripts/lib/machine-guard.js), and checks the build
//              is the current source (scripts/check-fresh-exe.js).
//   2. relay   a throwaway relay: a COPY of the exe, --headless, in a temp
//              folder, on a free port, with its own database, killed by PID
//              on exit and on Ctrl+C (scripts/lib/throwaway-relay.js).
//   3. boot    ONE game instance in its own probe-rig sandbox
//              (.probe-rig/copresence: portable, background, never focused,
//              silent), pointed at that relay by the autopilot request.
//   4. join    waits until the game has joined the shared world (the
//              recorder's one-frame probe reports game_joined).
//   5. camera  puts the player at a known home pose facing a quarter turn
//              (the showcase request's `cam` verb, which keeps the player
//              walking in the shared world; the camera request would not, see
//              step 5 below), then reads the camera's real position and yaw
//              back from the game (the recorder's one-frame probe), so the
//              rig knows where it looks.
//   6. walk    scripts/second-player.js walks a straight line across the view,
//              6 m in front of the camera, at a walking pace.
//   7. record  the game records, every frame, where it DREW each remote
//              player, with its own frame times (debug/remote_players_request
//              .json, src/engine/ipc.rs).
//   8. shots   two viewport screenshots a moment apart with the walker in view.
//   9. judge   scripts/lib/copresence-judge.js: seen; never backwards; speed
//              near the walker's real speed on every frame (by the RECORDED
//              frame times); no single-frame jump; stays on its line; in view;
//              and in both screenshots its teal body is actually visible
//              under the nameplate the game drew (a window drawn over it
//              fails, which is what hid it on this rig's third run).
// Everything is saved under .probe-rig/copresence/runs/<stamp>/: manifest.json
// beside samples.json (the recorder's frames), the two PNGs, run.log,
// relay.log and walker.log.
//
// NEVER PRODUCTION: the relay is one this rig started, on loopback, with a
// database nobody else has. second-player.js refuses a non-loopback server
// unless told otherwise, and this rig never tells it otherwise.
//
// FOCUS: the game is launched as a script child, which the engine already
// treats as background (src/engine/launch_focus.rs), with HUMANITY_NO_FOCUS=1
// and the no_focus.txt marker as belt and braces. Every other HUMANITY_*
// variable of this shell is left out of its environment, so no focus opt-in
// can leak in from the shell either.
//
// HOMES ON PLOTS (--plots, increment 1b of docs/design/ship-homes-and-logistics.md):
// the relay hands each player a plot of the ship, and the game moves its home
// to its own plot. --plots runs the rig twice, once per join order (walker
// first, then game first; --order picks one), each with its own relay and
// boot, and judges where things are rather than whether they can be seen
// (they stand in two homes and cannot see each other until increment 2): the
// two plot ids differ, by the join order; after joining, the game's camera is
// inside the plot the game should hold; and every position the game DREW for
// the walker (the recorder records figures off screen too) is inside the
// walker's plot, with the smoothness checks on one forward leg of its walk.
// No camera pose, no screenshots. Evidence in runs/<stamp>-plots-<order>/.
//
// Usage:
//   node scripts/verify-copresence.js [--exe PATH] [--pose x,y,z,yaw,pitch]
//        [--distance M] [--radius M] [--speed M/S] [--timeout-min N] [--keep-open]
//   node scripts/verify-copresence.js --plots [--order walker-first|game-first|both]
//        [--exe PATH] [--radius M] [--speed M/S] [--timeout-min N]
//   node scripts/verify-copresence.js --dry-verdict <manifest.json>
// Exit 0 = every check passed. Exit 1 = refused to run. Exit 2 = failed.

"use strict";

const fs = require("fs");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
const DXC = require("./lib/dxc-dlls.js");
const TR = require("./lib/throwaway-relay.js");
const { judgeCopresence, LIMITS, figurePixels, FIGURE_MIN_PX, approachClear, judgePlots, forwardLegStart, readShipPlots } = require("./lib/copresence-judge.js");
const png = require("./lib/png.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const flag = (name) => args.includes(name);
const KEEP_OPEN = flag("--keep-open");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const TIMEOUT_MS = Number(opt("--timeout-min", "10")) * 60 * 1000;
const DRY = opt("--dry-verdict", null);
const PLOTS = flag("--plots");
const ORDERS = { both: ["walker-first", "game-first"], "walker-first": ["walker-first"], "game-first": ["game-first"] }[opt("--order", "both")];

// THE STAGE. The vehicle bay of the player's home (scripts/home-vantages.json
// "21-vehicle-bay"), standing at eye height (1.7 m; the floor is y 0), facing
// north (yaw 0 looks along -Z). In home-frame metres, which aboard the home
// are also the shared world's coordinates: the game sends its camera position
// as its own position, and draws other players at the positions they send.
// The yaw must be a quarter turn because second-player.js walks lines along X
// or Z only.
const POSE = opt("--pose", "30,1.7,20,0,-0.05");
// The line: DISTANCE in front of the camera, 2 x RADIUS long, across the view.
// At 6 m out and 4 m either side, the figure stays within 34 degrees of the
// camera's heading (the camera's vertical field of view is 90 degrees, so the
// picture is wider still). 8 m, not 6: the background game draws 13 to 17
// frames a second on this machine, and a 6 m line gave only 43 judged frame
// pairs on 2026-10-03, too near the judge's floor of 30 for a slower machine.
const DISTANCE = Number(opt("--distance", "6"));
const RADIUS = Number(opt("--radius", "4"));
// second-player.js's own walking pace.
const SPEED = Number(opt("--speed", "1.4"));
// Where the relay puts the walker. Since increment 1b every player gets a plot
// of the ship (src/relay/handlers/game_state.rs assign_home) and spawns at its
// spawn; the game joins first, so the walker holds the second plot, p2, and
// arrives at p2's spawn (data/blueprints/ship_structure.ron + the homestead
// design's spawn: (53.5, 1.7, 139.5), design section 2.4). It walks from there
// to the line in front of the camera; the rig checks that straight approach
// can never pass for the walk (the judge's approachClear), here before booting
// and again on the start the walker actually reports.
const SPAWN = [53.5, 1.7, 139.5];
// second-player.js reaches the start of its path within 4 s however far it is
// (APPROACH_SECONDS), so a walk of this long covers the approach, one full
// pass and a margin.
const APPROACH_MAX_S = 4;
const WALK_S = Math.ceil(APPROACH_MAX_S + (2 * RADIUS) / SPEED + 2);
// The recording starts just before the walker and runs past its end; signing
// in (loading the post-quantum library, deriving the keys) takes a few seconds.
const RECORD_S = WALK_S + 10;
const WALKER_NAME = "TestBotCrosser"; // the TestBot prefix keeps it off the member list
const WALKER_SEED = "verify-copresence-walker";
// The figure is drawn this far behind the walker (src/net/sync.rs INTERP_DELAY_S).
const DRAW_DELAY_S = 0.15;
// First-run windows drawn over the middle of the view, by the text of the
// button that answers each with its default (see step 5b).
const FIRST_RUN_BUTTONS = ["Use this privacy level"];

const RIG = path.join(REPO, ".probe-rig", "copresence");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const RIG_EXE = path.join(RIG, "HumanityOS.exe");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pad = (s, n) => String(s).padEnd(n);
const rel = (p) => {
  const r = path.relative(REPO, p);
  return !r || r.startsWith("..") ? p : r;
};
const fmt = (p) => `(${p.map((v) => Number(v).toFixed(2)).join(", ")})`;
function log(msg) {
  console.log(`[copresence] ${msg}`);
}
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

// ── The plan: where the line goes, from the camera's pose ────────────────────
/** The straight line to walk across the camera's view, or { error }. Pure. */
function planLine(cam, yaw, distance, radius) {
  const fwd = [Math.sin(yaw), 0, -Math.cos(yaw)];
  const center = [cam[0] + fwd[0] * distance, cam[1], cam[2] + fwd[2] * distance];
  let axis;
  if (Math.abs(fwd[0]) < 0.01) axis = "x";
  else if (Math.abs(fwd[2]) < 0.01) axis = "z";
  else return { error: `the camera's yaw ${yaw} is not a quarter turn: second-player.js walks lines along X or Z only, so face north, east, south or west` };
  const dir = axis === "x" ? [1, 0, 0] : [0, 0, 1];
  const start = center.map((v, i) => v - dir[i] * radius);
  const end = center.map((v, i) => v + dir[i] * radius);
  return { center, axis, dir, start, end };
}
/** True when the walker's straight approach from `from` can never pass for
 *  the line in the judge (the judge's approachClear). */
const clearApproach = (from, plan) => approachClear(from, { start: plan.start, end: plan.end });

// ── The verdict: rig steps plus the judge, over a manifest ──────────────────
function verdict(m, dir) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const s = m.steps_ok || {};
  add("relay_up", s.relay && s.relay.ok, s.relay ? s.relay.detail : "never started");
  add("game_in_world", s.joined && s.joined.ok, s.joined ? s.joined.detail : "never joined");
  add("camera_parked", s.camera && s.camera.ok, s.camera ? s.camera.detail : "never parked");
  add("walker_ran", s.walker && s.walker.ok, s.walker ? s.walker.detail : "never ran");
  add("approach_clear", s.approach && s.approach.ok, s.approach ? s.approach.detail : "the walker's start was never reported");
  add("view_clear", s.view && s.view.ok, s.view ? s.view.detail : "never checked");
  add("screenshots", s.screenshots && s.screenshots.ok, s.screenshots ? s.screenshots.detail : "none taken");
  add("on_screen", s.on_screen && s.on_screen.ok, s.on_screen ? s.on_screen.detail : "the nameplate was never looked for");
  // The figure is VISIBLE in both pictures, not just drawn: its teal body
  // counted under where the game put its nameplate (the whole picture when no
  // nameplate position was recorded). Computed from the PNGs here, so
  // --dry-verdict re-checks it.
  const shots = Array.isArray(m.screenshots) ? m.screenshots : [];
  const seenIn = shots.map((sh) => {
    if (!sh.file || !fs.existsSync(path.join(dir, sh.file))) return { file: sh.file, ok: false, note: "no picture" };
    const at = sh.nameplate && sh.nameplate.pos_px ? sh.nameplate.pos_px : null;
    const f = figurePixels(png.decode(fs.readFileSync(path.join(dir, sh.file))), at);
    return { file: sh.file, ok: f.count >= FIGURE_MIN_PX, note: `${f.count} teal px ${at ? "under its nameplate" : "in the whole picture"}${f.centroid ? ` (centre x ${f.centroid[0].toFixed(0)})` : ""}` };
  });
  add(
    "figure_visible",
    seenIn.length === 2 && seenIn.every((x) => x.ok),
    seenIn.length ? `${seenIn.map((x) => `${x.file}: ${x.note}`).join("; ")} (at least ${FIGURE_MIN_PX} each)` : "no screenshots",
  );
  let judged = null;
  const samplesPath = m.samples ? path.join(dir, m.samples) : null;
  if (samplesPath && fs.existsSync(samplesPath) && m.walker && m.line) {
    const rec = JSON.parse(fs.readFileSync(samplesPath, "utf8"));
    judged = judgeCopresence({
      frames: rec.frames || [],
      walker: { id: m.walker.id, name: m.walker.name },
      line: { start: m.line.start, end: m.line.end },
      speed: m.speed,
      onLineEpochMs: m.walker.on_line_epoch_ms,
    });
    for (const c of judged.checks) checks.push(c);
  } else {
    add("recorded", false, samplesPath ? `no samples at ${rel(samplesPath)}` : "nothing was recorded");
  }
  const panics = Number(m.panics || 0);
  add("no_panics", panics === 0, `${panics} PANIC line(s) in run.log`);
  return { checks, pass: checks.every((c) => c.ok), stats: judged ? judged.stats : null };
}

function printVerdict(prefix, m, dir) {
  const { checks, pass, stats } = verdict(m, dir);
  console.log("");
  console.log("-".repeat(72));
  for (const c of checks) console.log(`${c.ok ? "PASS" : "FAIL"}  ${pad(c.id, 21)} ${c.detail}`);
  console.log("-".repeat(72));
  if (stats && stats.speed) {
    console.log(
      `measured: ${stats.pairs} frame pairs judged; drawn speed ${stats.speed.min.toFixed(3)} to ${stats.speed.max.toFixed(3)} m/s ` +
        `(median ${stats.speed.median.toFixed(3)}) for a ${stats.speed.walker} m/s walk; frame times ` +
        `${(stats.frame_dt.min * 1000).toFixed(1)} to ${(stats.frame_dt.max * 1000).toFixed(1)} ms`,
    );
  }
  if (pass) console.log(`${prefix}PASS  ${checks.length}/${checks.length} co-presence checks passed`);
  else {
    const failed = checks.filter((c) => !c.ok).map((c) => c.id);
    console.log(`${prefix}FAIL  ${checks.length - failed.length}/${checks.length} passed; failed: ${failed.join(", ")}`);
  }
  console.log(`        evidence ${rel(dir)}`);
  console.log("-".repeat(72));
  console.log("");
  return pass;
}

if (DRY) {
  const manifest = path.resolve(DRY);
  if (!fs.existsSync(manifest)) refuse([`--dry-verdict: no such manifest: ${manifest}`]);
  const m = JSON.parse(fs.readFileSync(manifest, "utf8"));
  console.log(`[dry] verdict only, nothing booted: ${rel(manifest)}`);
  const printer = m.kind === "verify-copresence-plots" ? printPlotsVerdict : printVerdict;
  const pass = printer("DRY VERDICT (nothing was booted): ", m, path.dirname(manifest));
  process.exit(pass ? 0 : 2);
}

// ── Preconditions ────────────────────────────────────────────────────────────
console.log("");
console.log("verify-copresence  a real game, a real relay, a scripted second player walking past");
console.log("");

const poseNums = POSE.split(",").map(Number);
if (poseNums.length !== 5 || !poseNums.every(Number.isFinite)) refuse([`--pose must be five numbers x,y,z,yaw,pitch, got "${POSE}"`]);
if (!(DISTANCE > 0 && RADIUS > 0 && SPEED > 0)) refuse(["--distance, --radius and --speed must be above 0"]);
if (PLOTS && !ORDERS) refuse(["--order must be walker-first, game-first or both"]);
// Refuse a stage the judge could not read before booting anything.
if (!PLOTS) {
  const p = planLine(poseNums.slice(0, 3), poseNums[3], DISTANCE, RADIUS);
  if (p.error) refuse([`REFUSED: ${p.error}.`]);
  if (!clearApproach(SPAWN, p)) {
    refuse([
      `REFUSED: the walker's approach from ${fmt(SPAWN)} (where the relay spawns it) to the line ${fmt(p.start)} -> ${fmt(p.end)}`,
      "would run along the line itself, so it could be judged as the walk. Pick a pose whose line lies across the approach.",
    ]);
  }
  if (2 * RADIUS <= LIMITS.START_MARGIN_M + LIMITS.END_MARGIN_M + 1) refuse([`--radius ${RADIUS} leaves too short a pass to judge`]);
}

const instances = MG.listInstances();
if (instances.length) {
  refuse([
    `REFUSED: HumanityOS.exe is already running (${instances.map((p) => `pid ${p.pid} ${p.exe}`).join("; ")}).`,
    "One GPU, one instance (CLAUDE.md). This rig also starts a headless relay from the same",
    "binary, so a leftover of either would be counted twice. Wait for it to exit, then run again.",
    "If it is a leftover rig of ours: taskkill //PID <pid> //F",
  ]);
}

const fresh = spawnSync(process.execPath, [path.join(__dirname, "check-fresh-exe.js"), "--exe", EXE], { cwd: REPO, stdio: "inherit" });
if (fresh.status !== 0) {
  console.error("verify-copresence: REFUSED - see the freshness failure above. Nothing was booted.");
  process.exit(1);
}

// ── Rig setup (the verify-live-screen pattern) ──────────────────────────────
function ensureJunction(link, target) {
  try {
    const st = fs.lstatSync(link);
    if (st.isSymbolicLink() || st.isDirectory()) {
      try {
        if (fs.realpathSync(link).toLowerCase() === fs.realpathSync(target).toLowerCase()) return;
      } catch {}
      fs.rmSync(link, { recursive: true, force: true });
    }
  } catch {}
  fs.symlinkSync(target, link, "junction");
}
function killRigProcesses() {
  const rigExe = RIG_EXE.replace(/'/g, "''");
  try {
    execSync(
      `powershell -NoProfile -Command "Get-Process HumanityOS -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${rigExe}' } | Stop-Process -Force"`,
      { stdio: "ignore" },
    );
  } catch {}
}
function setupRig() {
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  // portable.txt: identity/config/saves stay inside the rig, and the dev
  // autopilot runs (it refuses against a real installed identity).
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-copresence rig\n");
  // no_focus.txt: the engine boots background even if the env var is lost.
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
  killRigProcesses();
  try {
    fs.copyFileSync(EXE, RIG_EXE);
  } catch (e) {
    if (e.code !== "EBUSY") throw e;
    execSync("ping -n 3 127.0.0.1 >nul", { shell: "cmd.exe" });
    fs.copyFileSync(EXE, RIG_EXE);
  }
  // The DXC shader compiler dlls, from beside the exe or else the repo root
  // (target/release has none; the repo root does). Without them the game
  // falls back to FXC, and on 2026-10-03 this rig's first run sat in FXC's
  // pipeline compile for over three minutes, past the autopilot's wait.
  // One shared lookup, scripts/lib/dxc-dlls.js, which logs which folder it
  // used or that it found neither.
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.png$/.test(f) || /_done\.json(\.tmp)?$/.test(f) || /_request\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}

// ── IPC helpers (the same file-drop protocol the other rigs use) ────────────
function clearDone(...names) {
  for (const n of names) {
    const p = path.join(DEBUG, n);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }
}
async function waitFile(name, timeoutMs, pollMs = 250) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(p)) {
      try {
        return JSON.parse(fs.readFileSync(p, "utf8"));
      } catch {
        // half-written; retry
      }
    }
    await sleep(pollMs);
  }
  return null;
}
function req(name, body) {
  fs.writeFileSync(path.join(DEBUG, name), JSON.stringify(body));
}
async function waitBoot(timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(LOG)) {
      const txt = fs.readFileSync(LOG, "utf8");
      if (/PANIC/.test(txt)) throw new Error("PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return true;
    }
    await sleep(1000);
  }
  throw new Error("the exe did not finish booting in time");
}
function panicCount() {
  if (!fs.existsSync(LOG)) return 0;
  return (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length;
}
/** The recorder's one-frame probe: is the game in the shared world, and where
 *  is its camera? */
async function probe() {
  clearDone("remote_players_done.json");
  req("remote_players_request.json", { seconds: 0 });
  return waitFile("remote_players_done.json", 15000);
}
/** One main-UI request (debug/ui_request.json, src/engine/ipc.rs
 *  poll_ui_request): `find` a drawn text, or `click` a window pixel. */
async function ui(body, timeoutMs = 15000) {
  clearDone("ui_request_done.json");
  req("ui_request.json", body);
  return waitFile("ui_request_done.json", timeoutMs);
}
async function screenshot(name) {
  clearDone("screenshot_done.json");
  req("screenshot_request.json", { note: `verify-copresence ${name}` });
  const shot = await waitFile("screenshot_done.json", 30000);
  if (!shot || !shot.ok || !shot.path) return { ok: false, error: shot ? shot.error || "no path" : "no screenshot_done.json in 30 s" };
  const src = path.isAbsolute(shot.path) ? shot.path : path.join(RIG, shot.path);
  if (!fs.existsSync(src)) return { ok: false, error: `the game said it wrote ${shot.path}, which is not there` };
  return { ok: true, src };
}

/** The game's environment: this shell's, minus every HUMANITY_* variable (no
 *  focus opt-in, data folder or owner key can leak in), plus the background
 *  marker. */
function gameEnv() {
  const env = {};
  for (const [k, v] of Object.entries(process.env)) if (!k.toUpperCase().startsWith("HUMANITY_")) env[k] = v;
  env.HUMANITY_NO_FOCUS = "1";
  return env;
}

// ── The live run ─────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RIG, "runs", stamp);

async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "verify-copresence",
    stamp,
    exe: EXE,
    rig: RIG,
    pose: POSE,
    distance_m: DISTANCE,
    radius_m: RADIUS,
    speed: SPEED,
    limits: LIMITS,
    relay: null,
    camera: null,
    line: null,
    walker: null,
    samples: null,
    screenshots: [],
    steps: [],
    steps_ok: {},
    foreign_before: [],
    foreign_after: [],
    panics: 0,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));
  const step = (id, ok, detail, extra = {}) => {
    manifest.steps.push({ id, ok, detail, ...extra });
    save();
    log(`${ok ? "ok  " : "FAIL"} ${pad(id, 10)} ${detail}`);
    return ok;
  };

  // Everything we start, so nothing is left running whatever happens. The
  // relay has its own net (scripts/lib/throwaway-relay.js); this one covers
  // the game (started detached, so it is NOT in Node's kill-on-exit job) and
  // the walker.
  let relay = null;
  let gamePid = null;
  let walker = null;
  let killed = false;
  const killAll = () => {
    if (killed) return;
    killed = true;
    if (walker && walker.exitCode === null) {
      try {
        execSync(`taskkill /PID ${walker.pid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    if (gamePid) {
      try {
        execSync(`taskkill /PID ${gamePid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    killRigProcesses();
    if (relay) {
      relay.kill();
      relay.removeDir();
    }
  };
  process.on("exit", () => {
    if (!KEEP_OPEN) killAll();
  });
  // Ctrl+C and friends: exit the way an interrupted program does; the "exit"
  // handler above (and the relay helper's) then cleans up.
  for (const [sig, code] of [["SIGINT", 130], ["SIGTERM", 143], ["SIGBREAK", 149], ["SIGHUP", 129]]) {
    process.on(sig, () => process.exit(code));
  }
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min; killing everything`);
    manifest.steps.push({ id: "timeout", ok: false, detail: `run exceeded ${TIMEOUT_MS / 60000} min` });
    manifest.panics = panicCount();
    save();
    killAll();
    printVerdict("RESULT: ", manifest, OUT);
    process.exit(2);
  }, TIMEOUT_MS);

  const walkerOut = [];
  try {
    // ── 2. The throwaway relay.
    relay = await TR.startRelay({
      sourceExe: EXE,
      prefix: "verify-copresence-relay-",
      config: { server_name: "verify-copresence relay" },
    });
    manifest.relay = { url: relay.httpUrl, pid: relay.pid, dir: relay.dir, health: relay.health };
    manifest.steps_ok.relay = relay.health
      ? { ok: true, detail: `${relay.httpUrl} answered /health (pid ${relay.pid}, a copy in ${relay.dir})` }
      : { ok: false, detail: `${relay.httpUrl} never answered /health: ${relay.logText().slice(-400)}` };
    step("relay", manifest.steps_ok.relay.ok, manifest.steps_ok.relay.detail);
    if (!relay.health) throw new Error("the throwaway relay did not come up");

    // ── 3. The game, pointed at OUR relay.
    const child = spawn(RIG_EXE, [], { cwd: RIG, detached: true, stdio: "ignore", env: gameEnv() });
    gamePid = child.pid;
    fs.writeFileSync(path.join(RIG, "probe_pid.txt"), String(gamePid));
    child.unref();
    manifest.foreign_before = MG.foreignProcs({ pids: [gamePid, relay.pid], exe: RIG_EXE, gameOnly: true }).map(MG.describe);
    step("launch", true, `game pid ${gamePid} from ${rel(RIG_EXE)} (background, no focus)`);
    await waitBoot(180000);
    step("boot", true, "booted (run.log: cloud noise volumes generated, no PANIC)");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: relay.httpUrl, user_name: "CopresenceRig", character_name: "CopresenceRig" });
    const ap = await waitFile("autopilot_done.json", 300000);
    if (!ap || ap.ok !== true) {
      const last = fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8").trim().split(/\r?\n/).pop() : "(no run.log)";
      throw new Error(
        ap
          ? `the autopilot refused: ${ap.error || JSON.stringify(ap)}`
          : `the game never answered the autopilot request in 300 s (still booting? the last line of run.log: ${last})`,
      );
    }
    step("autopilot", true, `entering the world as ${ap.character_name} on ${ap.server_url}`);

    // ── 4. In the shared world, with the welcome applied: on arriving, the
    // welcome stands the player where the relay holds them (increment 1b,
    // engine/home_plot.rs), so a pose set before it would be undone by it. A
    // build from before 1b reports no `welcomed` and is read once joined.
    let pr = null;
    for (const t0 = Date.now(); Date.now() - t0 < 120000; ) {
      pr = await probe();
      if (pr && pr.ok && pr.world_loaded && pr.game_joined && pr.copresence_active && pr.welcomed !== false) break;
      await sleep(1000);
    }
    const joined = !!(pr && pr.ok && pr.world_loaded && pr.game_joined && pr.copresence_active && pr.welcomed !== false);
    manifest.steps_ok.joined = {
      ok: joined,
      detail: pr
        ? `world_loaded=${pr.world_loaded} ws_identified=${pr.ws_identified} game_joined=${pr.game_joined} copresence_active=${pr.copresence_active} welcomed=${pr.welcomed}`
        : "the recorder never answered (is this build older than the recorder?)",
    };
    step("join", joined, manifest.steps_ok.joined.detail);
    if (!joined) throw new Error("the game never joined the shared world");

    // ── 5. The camera, at a known pose, WITHOUT leaving the shared world.
    // Not the camera request: every form of it turns on dev fly mode, and fly
    // mode steps the player OUT of the shared world on purpose (lib.rs, "Dev
    // travel sync": "Engaging fly while JOINED to the shared world STEPS
    // OUT"), which despawns every other player. Seen on this rig's second run
    // (2026-10-03): "Co-presence: stepped out of the shared world (solo)" a
    // frame after the station request. The showcase request's `cam` verb moves
    // the player's body and camera and nothing else (photograph-home.js uses
    // it), so the player stays a walking member of the shared world.
    const entry = pr && pr.camera_end;
    if (entry) log(`     world entry left the camera at ${fmt(entry.pos)} yaw ${entry.yaw.toFixed(3)}`);
    clearDone("showcase_done.json");
    req("showcase_request.json", { cam: POSE });
    // Let the pose settle (the body lands on the floor in walking mode), then
    // read it back from the game itself, twice a second apart: the line is
    // planned from where the camera REALLY is, and only if it holds still.
    await sleep(2500);
    const p1 = await probe();
    await sleep(1000);
    const p2 = await probe();
    const c1 = p1 && p1.camera_end;
    const c2 = p2 && p2.camera_end;
    const drift = c1 && c2 ? Math.hypot(...c2.pos.map((v, i) => v - c1.pos[i])) : Infinity;
    const turn = c1 && c2 ? Math.abs(c2.yaw - c1.yaw) : Infinity;
    const plan = c2 ? planLine(c2.pos, c2.yaw, DISTANCE, RADIUS) : { error: "the game never reported its camera" };
    const asked = poseNums.slice(0, 3);
    const off = c2 ? Math.hypot(...c2.pos.map((v, i) => v - asked[i])) : Infinity;
    manifest.camera = { requested: POSE, entry: entry || null, read_back: c2 || null, off_requested_m: off };
    manifest.line = plan.error ? null : { start: plan.start, end: plan.end, center: plan.center, axis: plan.axis, radius: RADIUS };
    const held = drift < 0.01 && turn < 0.001;
    const stillIn = !!(p2 && p2.game_joined && p2.copresence_active);
    manifest.steps_ok.camera = {
      ok: !plan.error && held && stillIn && off < 0.5,
      detail: plan.error
        ? plan.error
        : `at ${fmt(c2.pos)} yaw ${c2.yaw.toFixed(3)} pitch ${c2.pitch.toFixed(3)} (asked ${POSE}, ${off.toFixed(3)} m off); ` +
          `${held ? "holding still" : `NOT holding still: moved ${drift.toFixed(3)} m and turned ${turn.toFixed(4)} rad in a second`}; ` +
          `${stillIn ? "still in the shared world" : "NOT in the shared world any more"}; ` +
          `the walk: ${fmt(plan.start)} -> ${fmt(plan.end)} along ${plan.axis}, ${DISTANCE} m out`,
    };
    step("camera", manifest.steps_ok.camera.ok, manifest.steps_ok.camera.detail);
    if (!manifest.steps_ok.camera.ok) throw new Error("the camera did not hold a known pose in the shared world");

    // ── 5b. A clear view. A brand-new identity is asked to pick its privacy
    // level in a window drawn over the middle of the screen, exactly where
    // the walker crosses: on this rig's third run (2026-10-03) both
    // screenshots showed that window and no figure, while the HUD said
    // "1 here: TestBotCrosser". Answer it the way a person would: find the
    // button by its text, click it (the default, Private, is already
    // selected), then make sure the window is gone. A rig sandbox that
    // already answered it in an earlier run simply finds nothing to click.
    const cleared = [];
    let viewOk = true;
    for (const label of FIRST_RUN_BUTTONS) {
      const f = await ui({ action: "find", text: label });
      if (!f || f.ok !== true) {
        viewOk = false;
        cleared.push(`could not ask about "${label}": ${JSON.stringify(f)}`);
        continue;
      }
      if (!f.found || f.text !== label) {
        cleared.push(`"${label}" not on screen`);
        continue;
      }
      await ui({ action: "click", pos: f.pos_px });
      await sleep(800);
      const again = await ui({ action: "find", text: label });
      const gone = !!(again && again.ok === true && !(again.found && again.text === label));
      viewOk = viewOk && gone;
      cleared.push(gone ? `clicked "${label}" at ${fmt(f.pos_px)} px; its window is gone` : `clicked "${label}" but it is STILL drawn`);
    }
    manifest.steps_ok.view = { ok: viewOk, detail: cleared.join("; ") };
    step("view", viewOk, manifest.steps_ok.view.detail);

    // ── 6 + 7. Start recording, then start the walker.
    clearDone("remote_players_done.json");
    req("remote_players_request.json", { seconds: RECORD_S });
    step("record", true, `recording every frame for ${RECORD_S} s of the game's frame clock`);
    const walkerArgs = [
      path.join(__dirname, "second-player.js"),
      "--server", relay.url,
      "--name", WALKER_NAME,
      "--seed", WALKER_SEED,
      "--path", "line",
      "--axis", plan.axis,
      "--center", plan.center.join(","),
      "--radius", String(RADIUS),
      "--speed", String(SPEED),
      "--seconds", String(WALK_S),
    ];
    const walkerStarted = Date.now();
    walker = spawn(process.execPath, walkerArgs, { cwd: REPO, stdio: ["ignore", "pipe", "pipe"] });
    const take = (b) => {
      for (const line of String(b).split(/\r?\n/).filter(Boolean)) walkerOut.push({ at_s: (Date.now() - walkerStarted) / 1000, line });
    };
    walker.stdout.on("data", take);
    walker.stderr.on("data", take);
    const walkerExit = new Promise((r) => walker.on("exit", (code) => r(code)));
    const waitLine = async (re, timeoutMs) => {
      let ended = false;
      walkerExit.then(() => (ended = true));
      for (const t0 = Date.now(); Date.now() - t0 < timeoutMs; ) {
        const hit = walkerOut.find((o) => re.test(o.line));
        if (hit) return { hit, m: hit.line.match(re) };
        if (ended) return null;
        await sleep(50);
      }
      return null;
    };
    manifest.walker = { name: WALKER_NAME, seed: WALKER_SEED, args: walkerArgs.slice(1), id: null, start: null, exit_code: null };
    const inWorld = await waitLine(/in the world as entity (\d+), starting at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/, 40000);
    if (!inWorld) throw new Error(`the walker never got into the world: ${walkerOut.map((o) => o.line).join(" | ")}`);
    manifest.walker.id = Number(inWorld.m[1]);
    manifest.walker.start = [Number(inWorld.m[2]), Number(inWorld.m[3]), Number(inWorld.m[4])];
    // The judge's window is sound only if the approach never runs along the
    // line; checked on the start the relay really gave it.
    const clear = clearApproach(manifest.walker.start, plan);
    manifest.steps_ok.approach = {
      ok: clear,
      detail: clear
        ? `the walker started at ${fmt(manifest.walker.start)}; its straight approach to the line's start ${fmt(plan.start)} never runs along the line`
        : `the walker started at ${fmt(manifest.walker.start)}: its approach to ${fmt(plan.start)} runs along the line and could be judged as the walk; pick another --pose`,
    };
    step("walker", true, `${WALKER_NAME} in the world as entity ${manifest.walker.id}, starting at ${fmt(manifest.walker.start)}`);

    // ── 8. Two screenshots a moment apart while it crosses: a third and
    // two thirds of the way along, timed from when it reached the line
    // (plus the drawing delay). Right before each, the game is asked where it
    // drew the walker's NAMEPLATE (the ui `find` verb, exact text match: the
    // HUD's "1 here: <name>" roster line only contains the name, so it can
    // only win when no nameplate is drawn). That is the game's own word that
    // the figure is on screen, and where, independent of anyone's eyes.
    const onPath = await waitLine(/on the path at/, 15000);
    const shots = [];
    if (onPath) {
      const passS = (2 * RADIUS) / SPEED;
      // When the walker reached the line, by the computer's clock: the judge's
      // "on time" check counts the walker's real position from it.
      manifest.walker.on_line_epoch_ms = walkerStarted + onPath.hit.at_s * 1000;
      const onLineAt = walkerStarted + onPath.hit.at_s * 1000 + DRAW_DELAY_S * 1000;
      for (const [name, share] of [["shot_a", 0.3], ["shot_b", 0.65]]) {
        const due = onLineAt + share * passS * 1000;
        if (Date.now() < due) await sleep(due - Date.now());
        const np = await ui({ action: "find", text: WALKER_NAME });
        const requested = (Date.now() - onLineAt) / 1000;
        const s = await screenshot(name);
        const nameplate = np && np.found && np.text === WALKER_NAME ? { pos_px: np.pos_px, rect_px: np.rect_px } : null;
        const npNote = np ? (np.found ? `found "${np.text}"` : "not drawn") : "no answer";
        if (s.ok) {
          fs.copyFileSync(s.src, path.join(OUT, `${name}.png`));
          shots.push({ file: `${name}.png`, s_after_reaching_line: Number(requested.toFixed(2)), expected_along_m: Number((SPEED * requested).toFixed(2)), nameplate, nameplate_find: npNote });
        } else {
          shots.push({ file: null, error: s.error, nameplate, nameplate_find: npNote });
        }
      }
    }
    manifest.screenshots = shots;
    const shotsOk = shots.length === 2 && shots.every((s) => s.file);
    manifest.steps_ok.screenshots = {
      ok: shotsOk,
      detail: onPath
        ? shots.map((s) => (s.file ? `${s.file} at ${s.s_after_reaching_line} s into the line (about ${s.expected_along_m} m along)` : `failed: ${s.error}`)).join("; ")
        : "the walker never reported reaching its line",
    };
    step("shots", shotsOk, manifest.steps_ok.screenshots.detail);
    // The nameplate was on screen at both moments, and moved across the
    // screen the way the walk goes (the walk's direction along the camera's
    // right: +X is to the right for a camera facing north).
    const [na, nb] = [shots[0] && shots[0].nameplate, shots[1] && shots[1].nameplate];
    const rightX = Math.cos(manifest.camera.read_back.yaw); // camera right = (cos yaw, 0, sin yaw)
    const rightZ = Math.sin(manifest.camera.read_back.yaw);
    const sideways = plan.dir[0] * rightX + plan.dir[2] * rightZ; // +1: walks to the right
    const npOk = !!(na && nb && (nb.pos_px[0] - na.pos_px[0]) * sideways > 0);
    manifest.steps_ok.on_screen = {
      ok: npOk,
      detail:
        na && nb
          ? `${WALKER_NAME}'s nameplate drawn at x ${na.pos_px[0].toFixed(0)} px then x ${nb.pos_px[0].toFixed(0)} px ` +
            `(y ${na.pos_px[1].toFixed(0)}, ${nb.pos_px[1].toFixed(0)}), moving ${sideways > 0 ? "right" : "left"} as the walk goes` +
            (npOk ? "" : ": WRONG WAY or not at all")
          : `nameplate at the two screenshots: ${shots.map((s) => s.nameplate_find).join(", ") || "never asked"}`,
    };
    step("on_screen", npOk, manifest.steps_ok.on_screen.detail);

    // ── The walker finishes on its own; the recording ends after it.
    const code = await Promise.race([walkerExit, sleep((WALK_S + 30) * 1000).then(() => "still running")]);
    manifest.walker.exit_code = code;
    manifest.steps_ok.walker = {
      ok: code === 0,
      detail: `${WALKER_NAME} (entity ${manifest.walker.id}) walked ${WALK_S} s and exited ${code}`,
    };
    step("walked", code === 0, manifest.steps_ok.walker.detail);
    const rec = await waitFile("remote_players_done.json", (RECORD_S + 60) * 1000);
    if (!rec || rec.ok !== true) {
      step("samples", false, `no recording came back: ${JSON.stringify(rec && rec.error)}`);
    } else {
      fs.copyFileSync(path.join(DEBUG, "remote_players_done.json"), path.join(OUT, "samples.json"));
      manifest.samples = "samples.json";
      manifest.recording = {
        frame_count: rec.frame_count,
        recorded_s: rec.recorded_s,
        wall_s: rec.wall_s,
        truncated: rec.truncated,
        game_joined: rec.game_joined,
      };
      step("samples", true, `${rec.frame_count} frames over ${rec.recorded_s.toFixed(2)} s of frame clock (${rec.wall_s.toFixed(2)} s wall) -> samples.json`);
    }
  } catch (e) {
    manifest.steps.push({ id: "abort", ok: false, detail: String(e.message || e) });
    log(`ABORT ${e.message || e}`);
  }
  clearTimeout(watchdog);
  manifest.foreign_after = MG.foreignProcs({ pids: [gamePid, relay && relay.pid].filter(Boolean), exe: RIG_EXE, gameOnly: true }).map(MG.describe);
  manifest.panics = panicCount();
  fs.writeFileSync(path.join(OUT, "walker.log"), walkerOut.map((o) => `${o.at_s.toFixed(2)}s ${o.line}`).join("\n") + "\n");
  try {
    fs.copyFileSync(LOG, path.join(OUT, "run.log"));
  } catch {}
  if (relay) {
    try {
      fs.copyFileSync(relay.logPath, path.join(OUT, "relay.log"));
    } catch {}
  }
  save();
  if (!KEEP_OPEN) killAll();
  else
    log(
      `--keep-open: the game (pid ${gamePid}) is still running; the relay stopped with this script (its helper cleans it up on exit), ` +
        `so the game shows no other player now. taskkill //PID ${gamePid} //T //F`,
    );
  const pass = printVerdict("RESULT: ", manifest, OUT);
  if (!pass) {
    console.log("What to do:");
    console.log(`  1. read the first FAIL line above and the step log above it: each says what it saw`);
    if (fs.existsSync(path.join(OUT, "shot_a.png"))) console.log(`  2. look at ${rel(path.join(OUT, "shot_a.png"))} and shot_b.png: is the teal figure there?`);
    if (fs.existsSync(path.join(OUT, "samples.json"))) console.log(`  3. samples.json holds every frame's drawn position and phase (Interpolating, Extrapolating, Holding)`);
    console.log(`  4. read walker.log, relay.log ("Game:" lines) and run.log (any PANIC) in ${rel(OUT)}`);
    console.log(`  5. re-judge without booting: node scripts/verify-copresence.js --dry-verdict ${rel(path.join(OUT, "manifest.json"))}`);
    console.log("");
  }
  process.exit(pass ? 0 : 2);
}

// ── --plots: homes on plots, in both join orders (increment 1b) ──────────────
//
// One run per join order, each with its own relay (so the plots start free)
// and its own boot of the game (one GPU: the runs never overlap):
//   walker-first  the walker joins before the game boots, so it claims the
//                 first plot (p1) and the game the second (p2): the game must
//                 move its home off the default plot it built at boot.
//   game-first    the game joins first and keeps the default plot (p1); the
//                 walker arrives second, on p2.
// In both, the walker walks a line from its own spawn along +Z, back and forth
// for as long as it runs (--center auto starts the path at its spawn when it
// holds a plot), and the game records every frame where it drew it.

const PLOTS_WALKER_NAME = "TestBotPlots"; // the TestBot prefix keeps it off the member list
const PLOTS_WALKER_SEED = "verify-copresence-plots-walker";
const SHIP_FILE = path.join(REPO, "data", "blueprints", "ship_structure.ron");
/** One back-and-forth of the walker's line, seconds (second-player.js walks
 *  2r out and 2r back). */
const CYCLE_S = (4 * RADIUS) / SPEED;
/** Long enough to hold one whole forward leg (2r / speed) wherever in the
 *  cycle the recording happens to start, plus the drawing delay and margins. */
const PLOTS_RECORD_S = Math.ceil(CYCLE_S + (2 * RADIUS) / SPEED + 4);

/** The --plots verdict: rig steps, the plot checks, and the smoothness checks
 *  on one forward leg of the walk (no view, no screenshots: the two players
 *  cannot see each other until increment 2). Re-runs from the manifest. */
function plotsVerdict(m, dir) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const s = m.steps_ok || {};
  add("relay_up", s.relay && s.relay.ok, s.relay ? s.relay.detail : "never started");
  add("game_in_world", s.joined && s.joined.ok, s.joined ? s.joined.detail : "never joined");
  add("walker_in_world", s.walker && s.walker.ok, s.walker ? s.walker.detail : "never ran");
  const samplesPath = m.samples ? path.join(dir, m.samples) : null;
  const frames = samplesPath && fs.existsSync(samplesPath) ? JSON.parse(fs.readFileSync(samplesPath, "utf8")).frames || [] : [];
  if (!frames.length) add("recorded", false, samplesPath ? `no frames in ${rel(samplesPath)}` : "nothing was recorded");
  // (Not PLOTS_WALKER_NAME: --dry-verdict runs this before that constant is set.)
  const walker = { id: m.walker ? m.walker.id : null, name: (m.walker && m.walker.name) || "TestBotPlots" };
  const plots = judgePlots({
    order: m.order,
    plots: m.plots,
    gamePlot: m.game_plot,
    walkerPlot: m.walker_plot,
    camera: m.camera_after_join,
    homeThings: m.home_things,
    frames,
    walker,
  });
  for (const c of plots.checks) checks.push(c);
  let stats = null;
  const onLine = m.walker ? m.walker.on_line_epoch_ms : null;
  const epochs = frames.map((f) => Number(f.epoch_ms)).filter(Number.isFinite);
  if (m.line && Number.isFinite(onLine) && epochs.length) {
    const first = epochs[0];
    const last = epochs[epochs.length - 1];
    const legMs = ((2 * m.radius) / m.speed) * 1000;
    const leg = forwardLegStart(onLine, m.speed, m.radius, first + 500);
    const whole = leg + legMs + 500 <= last;
    add(
      "forward_leg_recorded",
      whole,
      `the recording ran ${((last - first) / 1000).toFixed(1)} s; the judged forward leg starts ${((leg - first) / 1000).toFixed(2)} s in and lasts ${(legMs / 1000).toFixed(2)} s` +
        (whole ? "" : ": NOT all of it inside the recording"),
    );
    if (whole) {
      const j = judgeCopresence({ frames, walker, line: m.line, speed: m.speed, onLineEpochMs: leg, fromEpochMs: leg, checkView: false });
      for (const c of j.checks) checks.push(c);
      stats = j.stats;
    }
  } else {
    add("forward_leg_recorded", false, "no line, no time the walker reached it, or no frame times: the walk cannot be judged");
  }
  const panics = Number(m.panics || 0);
  add("no_panics", panics === 0, `${panics} PANIC line(s) in run.log`);
  return { checks, pass: checks.every((c) => c.ok), stats };
}

function printPlotsVerdict(prefix, m, dir) {
  const { checks, pass, stats } = plotsVerdict(m, dir);
  console.log("");
  console.log("-".repeat(72));
  console.log(`--plots, ${m.order}`);
  for (const c of checks) console.log(`${c.ok ? "PASS" : "FAIL"}  ${pad(c.id, 25)} ${c.detail}`);
  console.log("-".repeat(72));
  if (stats && stats.speed) {
    console.log(
      `measured: ${stats.pairs} frame pairs judged; drawn speed ${stats.speed.min.toFixed(3)} to ${stats.speed.max.toFixed(3)} m/s ` +
        `(median ${stats.speed.median.toFixed(3)}) for a ${stats.speed.walker} m/s walk`,
    );
  }
  if (pass) console.log(`${prefix}PASS  ${checks.length}/${checks.length} plot checks passed (${m.order})`);
  else {
    const failed = checks.filter((c) => !c.ok).map((c) => c.id);
    console.log(`${prefix}FAIL  ${checks.length - failed.length}/${checks.length} passed (${m.order}); failed: ${failed.join(", ")}`);
  }
  console.log(`        evidence ${rel(dir)}`);
  console.log("-".repeat(72));
  console.log("");
  return pass;
}

/** One --plots run in one join order. Returns true when every check passed. */
async function runPlotsOnce(order, runStamp, cleanups) {
  setupRig();
  const out = path.join(RIG, "runs", `${runStamp}-plots-${order}`);
  fs.mkdirSync(out, { recursive: true });
  const manifest = {
    kind: "verify-copresence-plots",
    order,
    stamp: runStamp,
    exe: EXE,
    rig: RIG,
    speed: SPEED,
    radius: RADIUS,
    relay: null,
    plots: null,
    plots_from: null,
    game_plot: null,
    walker_plot: null,
    camera_after_join: null,
    home_things: null,
    walker: null,
    line: null,
    samples: null,
    steps: [],
    steps_ok: {},
    panics: 0,
  };
  const save = () => fs.writeFileSync(path.join(out, "manifest.json"), JSON.stringify(manifest, null, 2));
  const step = (id, ok, detail) => {
    manifest.steps.push({ id, ok, detail });
    save();
    log(`${ok ? "ok  " : "FAIL"} ${pad(id, 10)} ${detail}`);
    return ok;
  };
  let relay = null;
  let gamePid = null;
  let walker = null;
  let killed = false;
  const killAll = () => {
    if (killed) return;
    killed = true;
    if (walker && walker.exitCode === null) {
      try {
        execSync(`taskkill /PID ${walker.pid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    if (gamePid) {
      try {
        execSync(`taskkill /PID ${gamePid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    killRigProcesses();
    if (relay) {
      relay.kill();
      relay.removeDir();
    }
  };
  cleanups.push(killAll);
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min; killing everything`);
    manifest.steps.push({ id: "timeout", ok: false, detail: `run exceeded ${TIMEOUT_MS / 60000} min` });
    manifest.panics = panicCount();
    save();
    killAll();
    printPlotsVerdict("RESULT: ", manifest, out);
    process.exit(2);
  }, TIMEOUT_MS);

  const walkerOut = [];
  let walkerStarted = 0;
  let walkerExit = null;
  const waitLine = async (re, timeoutMs) => {
    let ended = false;
    if (walkerExit) walkerExit.then(() => (ended = true));
    for (const t0 = Date.now(); Date.now() - t0 < timeoutMs; ) {
      const hit = walkerOut.find((o) => re.test(o.line));
      if (hit) return { hit, m: hit.line.match(re) };
      if (ended) return null;
      await sleep(50);
    }
    return null;
  };
  /** Start the walker and wait until it is walking: its entity, the plot the
   *  relay gave it, its line, and when it reached the line. */
  const startWalker = async () => {
    const args = [
      path.join(__dirname, "second-player.js"),
      "--server", relay.url,
      "--name", PLOTS_WALKER_NAME,
      "--seed", PLOTS_WALKER_SEED,
      "--path", "line",
      "--axis", "z",
      "--radius", String(RADIUS),
      "--speed", String(SPEED),
      "--seconds", "0", // until this rig stops it
    ];
    walkerStarted = Date.now();
    walker = spawn(process.execPath, args, { cwd: REPO, stdio: ["ignore", "pipe", "pipe"] });
    const take = (b) => {
      for (const line of String(b).split(/\r?\n/).filter(Boolean)) walkerOut.push({ at_s: (Date.now() - walkerStarted) / 1000, line });
    };
    walker.stdout.on("data", take);
    walker.stderr.on("data", take);
    walkerExit = new Promise((r) => walker.on("exit", (code) => r(code)));
    manifest.walker = { name: PLOTS_WALKER_NAME, seed: PLOTS_WALKER_SEED, args: args.slice(1), id: null, start: null };
    const inWorld = await waitLine(/in the world as entity (\d+), starting at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/, 40000);
    const plotLine = await waitLine(/home_plot (null|missing|\{.*\})/, 5000);
    const centred = await waitLine(/centred on \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/, 5000);
    const onPath = await waitLine(/on the path at/, 15000);
    if (!inWorld || !centred || !onPath) {
      manifest.steps_ok.walker = { ok: false, detail: `the walker never got walking: ${walkerOut.map((o) => o.line).join(" | ")}` };
      step("walker", false, manifest.steps_ok.walker.detail);
      throw new Error("the walker never got walking");
    }
    manifest.walker.id = Number(inWorld.m[1]);
    manifest.walker.start = [Number(inWorld.m[2]), Number(inWorld.m[3]), Number(inWorld.m[4])];
    manifest.walker.home_plot_line = plotLine ? plotLine.hit.line : null;
    const hp = plotLine && plotLine.m[1].startsWith("{") ? JSON.parse(plotLine.m[1]) : null;
    manifest.walker_plot = hp ? hp.id : null;
    manifest.walker.on_line_epoch_ms = walkerStarted + onPath.hit.at_s * 1000;
    const c = [Number(centred.m[1]), Number(centred.m[2]), Number(centred.m[3])];
    manifest.line = { start: [c[0], c[1], c[2] - RADIUS], end: [c[0], c[1], c[2] + RADIUS], axis: "z", radius: RADIUS };
    // Walking; the end of the run says whether it still was when the recording ended.
    manifest.steps_ok.walker = { ok: true, detail: `${PLOTS_WALKER_NAME} (entity ${manifest.walker.id}) joined and is walking` };
    step(
      "walker",
      true,
      `${PLOTS_WALKER_NAME} in the world as entity ${manifest.walker.id} at ${fmt(manifest.walker.start)}, ${plotLine ? plotLine.m[0] : "home_plot never logged"}; walking ${fmt(manifest.line.start)} -> ${fmt(manifest.line.end)} and back`,
    );
  };

  try {
    relay = await TR.startRelay({ sourceExe: EXE, prefix: "verify-copresence-relay-", config: { server_name: "verify-copresence plots relay" } });
    manifest.relay = { url: relay.httpUrl, pid: relay.pid, dir: relay.dir, health: relay.health };
    manifest.steps_ok.relay = relay.health
      ? { ok: true, detail: `${relay.httpUrl} answered /health (pid ${relay.pid})` }
      : { ok: false, detail: `${relay.httpUrl} never answered /health: ${relay.logText().slice(-400)}` };
    step("relay", manifest.steps_ok.relay.ok, manifest.steps_ok.relay.detail);
    if (!relay.health) throw new Error("the throwaway relay did not come up");

    if (order === "walker-first") await startWalker();

    const child = spawn(RIG_EXE, [], { cwd: RIG, detached: true, stdio: "ignore", env: gameEnv() });
    gamePid = child.pid;
    fs.writeFileSync(path.join(RIG, "probe_pid.txt"), String(gamePid));
    child.unref();
    step("launch", true, `game pid ${gamePid} from ${rel(RIG_EXE)} (background, no focus)`);
    await waitBoot(180000);
    step("boot", true, "booted (run.log: cloud noise volumes generated, no PANIC)");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: relay.httpUrl, user_name: "CopresencePlots", character_name: "CopresencePlots" });
    const ap = await waitFile("autopilot_done.json", 300000);
    if (!ap || ap.ok !== true) throw new Error(`the autopilot did not run: ${ap ? ap.error || JSON.stringify(ap) : "no answer in 300 s"}`);
    step("autopilot", true, `entering the world on ${ap.server_url}`);

    // In the shared world, with the welcome applied (a build from before 1b
    // reports no `welcomed`, and is read as soon as it has joined).
    let pr = null;
    for (const t0 = Date.now(); Date.now() - t0 < 120000; ) {
      pr = await probe();
      if (pr && pr.ok && pr.world_loaded && pr.game_joined && pr.copresence_active && pr.welcomed !== false) break;
      if (pr && pr.copresence_refused) break;
      await sleep(1000);
    }
    const joined = !!(pr && pr.ok && pr.game_joined && pr.copresence_active && pr.welcomed !== false);
    manifest.steps_ok.joined = {
      ok: joined,
      detail: pr
        ? `world_loaded=${pr.world_loaded} game_joined=${pr.game_joined} copresence_active=${pr.copresence_active} welcomed=${pr.welcomed} refused=${pr.copresence_refused}`
        : "the recorder never answered",
    };
    step("join", joined, manifest.steps_ok.joined.detail);
    if (!joined) throw new Error("the game never joined the shared world");
    // Let the move settle (the rebuild runs on the welcome's frame), then read
    // where the game put its home and its camera.
    await sleep(2000);
    const pj = await probe();
    manifest.game_plot = pj && pj.home_plot ? pj.home_plot.id : null;
    manifest.camera_after_join = pj && pj.camera_end ? pj.camera_end.pos : null;
    // Where the home's own things stand (Respawn point, hologram, showroom
    // stage, animals, plants): judged against the plot the game should hold.
    manifest.home_things = pj && pj.home_things ? pj.home_things : null;
    if (pj && Array.isArray(pj.ship_plots) && pj.ship_plots.length) {
      manifest.plots = pj.ship_plots;
      manifest.plots_from = "the game's report";
    } else {
      manifest.plots = readShipPlots(fs.readFileSync(SHIP_FILE, "utf8"));
      manifest.plots_from = "the ship file (the game reported none)";
    }
    step(
      "home",
      true,
      `the game's home stands on ${manifest.game_plot || "(no plot reported)"}; its camera at ${manifest.camera_after_join ? fmt(manifest.camera_after_join) : "(none)"}; plots from ${manifest.plots_from}`,
    );

    if (order === "game-first") await startWalker();

    clearDone("remote_players_done.json");
    req("remote_players_request.json", { seconds: PLOTS_RECORD_S });
    step("record", true, `recording every frame for ${PLOTS_RECORD_S} s of the game's frame clock (one walk cycle is ${CYCLE_S.toFixed(2)} s)`);
    const rec = await waitFile("remote_players_done.json", (PLOTS_RECORD_S + 60) * 1000);
    if (!rec || rec.ok !== true) {
      step("samples", false, `no recording came back: ${JSON.stringify(rec && rec.error)}`);
    } else {
      fs.copyFileSync(path.join(DEBUG, "remote_players_done.json"), path.join(out, "samples.json"));
      manifest.samples = "samples.json";
      step("samples", true, `${rec.frame_count} frames over ${rec.recorded_s.toFixed(2)} s of frame clock`);
    }
    const stillWalking = walker && walker.exitCode === null;
    manifest.steps_ok.walker = {
      ok: stillWalking,
      detail: stillWalking
        ? `${PLOTS_WALKER_NAME} (entity ${manifest.walker.id}) was still walking when the recording ended`
        : `${PLOTS_WALKER_NAME} had stopped (exit ${walker ? walker.exitCode : "none"}) before the recording ended`,
    };
    step("walked", stillWalking, manifest.steps_ok.walker.detail);
  } catch (e) {
    manifest.steps.push({ id: "abort", ok: false, detail: String(e.message || e) });
    log(`ABORT ${e.message || e}`);
  }
  clearTimeout(watchdog);
  manifest.panics = panicCount();
  fs.writeFileSync(path.join(out, "walker.log"), walkerOut.map((o) => `${o.at_s.toFixed(2)}s ${o.line}`).join("\n") + "\n");
  try {
    fs.copyFileSync(LOG, path.join(out, "run.log"));
  } catch {}
  if (relay) {
    try {
      fs.copyFileSync(relay.logPath, path.join(out, "relay.log"));
    } catch {}
  }
  save();
  killAll();
  // The relay helper removes the folder; give the game a moment to be gone
  // before the next run copies the exe over its file.
  await sleep(2000);
  return printPlotsVerdict("RESULT: ", manifest, out);
}

async function mainPlots() {
  const cleanups = [];
  process.on("exit", () => {
    for (const k of cleanups) k();
  });
  for (const [sig, code] of [["SIGINT", 130], ["SIGTERM", 143], ["SIGBREAK", 149], ["SIGHUP", 129]]) {
    process.on(sig, () => process.exit(code));
  }
  const results = [];
  for (const order of ORDERS) {
    log(`── --plots, ${order} ──`);
    results.push([order, await runPlotsOnce(order, stamp, cleanups)]);
  }
  console.log("");
  for (const [order, ok] of results) console.log(`${ok ? "PASS" : "FAIL"}  --plots ${order}`);
  process.exit(results.every(([, ok]) => ok) ? 0 : 2);
}

(PLOTS ? mainPlots() : main()).catch((e) => {
  console.error(e);
  process.exit(2);
});
