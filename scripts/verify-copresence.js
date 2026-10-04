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
// HOMES ON PLOTS (--plots, increments 1b and 2 of docs/design/ship-homes-and-logistics.md):
// the relay hands each player a plot of the ship, and the game moves its home
// to its own plot. --plots runs the rig twice, once per join order (walker
// first, then game first; --order picks one), each with its own relay and
// boot. AT HOME, standing in two homes, it judges where things are: the two
// plot ids differ, by the join order; after joining, the game's camera is
// inside the plot the game should hold; and every position the game DREW for
// the walker (the recorder records figures off screen too) is inside the
// walker's plot, with the smoothness checks on one forward leg of its walk.
// Then the two MEET IN THE COMMONS (increment 2), judged under meet_* ids: the
// game reports its door points (debug/door_points_request.json, from its own
// corridor geometry, so no corridor maths lives in this rig) and is moved from
// its door into the Commons in steps the relay accepts (40 m, under its 100 m
// rule), to a pose facing a line there (MEET_POSE); the walker steps out of the
// world and back in at ITS door (it names it in its join, --home-spawn) and
// walks out through its own corridor into the Commons and along the line
// (second-player.js --route, from the same door points). The game records where
// it drew the walker and photographs it twice: the walk's smoothness, the view,
// the walker's teal body counted in both pictures, its nameplate moving the way
// it walks, the walker drawn coming out through its corridor, the relay holding
// the game in the Commons, and the walker seeing the game there. Then, in each order, the game STEPS OUT of
// the shared world and back (the showcase `solo` verb, the switch the
// launcher's offline home and Dev travel flip), having been moved more than
// 100 m from its door while out: the relay spawns it afresh at its door, and
// the judge checks the game then stands where the relay holds it and that its
// next move reaches the walker (the second review of 1b found it frozen there).
// Last, the game walks, in steps the relay accepts, to the same far place and
// presses Respawn (the showcase `respawn` verb, the death screen's button): the
// relay must stand it at its door too, judged the same way under respawn_* ids
// (the third review found Respawn left it frozen at the far end for everyone).
// Then, from its door, the game opens and shuts the build editor (the showcase
// `build_editor` verb, the B key's own function), which makes the door its build
// spot, walks to the far place again, and opens and shuts the editor there:
// shutting it must leave the game where the relay holds it, judged under editor_*
// ids (round 5 of the review found it put the game back at its build spot, more
// than 100 m away, frozen for everyone). The far place is the corner of a shared
// zone farthest from the door, from the door points, and every walk to it goes
// through the doors. Last, THE NEXT BOOT (increment 2's remembered plot): the
// game steps out, quits and boots again against the same relay, coming in the
// same way; its world load must build the home on the plot it held before it
// joins, and its welcome only confirm it (reboot_* ids).
// Evidence in runs/<stamp>-plots-<order>/.
//
// HOW THE GAME COMES IN (--entry, round 4 of the 1b review): a returning
// player's game identifies on the main menu (its auto-connect) and only then
// is Enter World pressed, so the join gate runs on that frame, BEFORE the
// world has loaded. The first 1b build sent a join naming no ship there and
// the relay refused it as another ship. The autopilot creates the identity as
// it enters, so its socket identifies after the world loads and that race
// never runs. So --entry menu connects first (the autopilot's "enter": false),
// waits for the handshake, answers the first-run privacy window, and presses
// the menu's own Enter World button; --entry autopilot is the all-in-one entry.
// By default game-first comes in from the menu and walker-first by autopilot,
// so a --plots run covers both paths. The judge's entered_from_menu_after_identify
// fails a menu entry on which the race did not really run.
//
// Usage:
//   node scripts/verify-copresence.js [--exe PATH] [--pose x,y,z,yaw,pitch]
//        [--distance M] [--radius M] [--speed M/S] [--timeout-min N] [--keep-open]
//        [--allow-other-build "<reason>"]   run a build that is NOT this tree, on
//        purpose (a red check); passed to scripts/check-fresh-exe.js and recorded
//        in the manifest as other_build
//   node scripts/verify-copresence.js --plots [--order walker-first|game-first|both]
//        [--entry menu|autopilot] [--exe PATH] [--radius M] [--speed M/S] [--timeout-min N]
//        [--meet-pose x,y,z,yaw,pitch] [--allow-other-build "<reason>"]
//   node scripts/verify-copresence.js --dry-verdict <manifest.json>
// Exit 0 = every check passed. Exit 1 = refused to run. Exit 2 = failed.

"use strict";

const fs = require("fs");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
const DXC = require("./lib/dxc-dlls.js");
const TR = require("./lib/throwaway-relay.js");
/** The relay's loopback self-check for the rig's output: what the operating
 *  system showed it listening on (startRelay refuses, and stops the relay, when
 *  any of it is not loopback, so a relay that got here listens on loopback only
 *  and never raises a Windows Firewall prompt). */
const loopbackNote = (relay) =>
  relay.listening && relay.listening.length
    ? `listening only on ${relay.listening.map((x) => `${x.host}:${x.port}`).join(", ")} (loopback, by the operating system's own list)`
    : "its listening addresses were not checked";
const {
  judgeCopresence,
  LIMITS,
  figurePixels,
  FIGURE_MIN_PX,
  approachClear,
  judgePlots,
  forwardLegStart,
  readShipPlots,
  judgeRejoin,
  judgeEditorClose,
  judgeEntry,
  placeAt,
  doorRoute,
  routeClear,
  farPlaces,
  farthestFrom,
  judgeMeet,
  judgeReboot,
} = require("./lib/copresence-judge.js");
const png = require("./lib/png.js");
// The freshness gate, run through its one runner so --allow-other-build reaches
// it and comes back as the manifest's other_build record (BUG-133).
const { runFreshGate, otherBuildNotice } = require("./lib/src-fingerprint.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const flag = (name) => args.includes(name);
const KEEP_OPEN = flag("--keep-open");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
// A --plots run boots the game twice (the second boot proves the remembered plot) and walks
// the meeting and the 1b legs between: 20 minutes per join order by default.
const TIMEOUT_MS = Number(opt("--timeout-min", flag("--plots") ? "20" : "10")) * 60 * 1000;
const DRY = opt("--dry-verdict", null);
const PLOTS = flag("--plots");
const ORDERS = { both: ["walker-first", "game-first"], "walker-first": ["walker-first"], "game-first": ["game-first"] }[opt("--order", "both")];
// How the game comes into the world in a --plots run (see the header): menu or
// autopilot for every order, or by default menu for game-first and autopilot
// for walker-first.
const ENTRY = opt("--entry", null);
const entryFor = (order) => ENTRY || (order === "game-first" ? "menu" : "autopilot");

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
// Where the relay puts the walker. Since increment 1b every player whose join
// names the relay's ship gets a plot of it (src/relay/handlers/game_state.rs
// assign_home); the walker asks /api/server-info for the ship and names it. The
// game joins first, so the walker holds the second plot, p2, and, drawing no
// home and so naming no door, arrives in its middle (origin (0, 0, 99), 55 x 89
// m: (27.5, 1.7, 143.5)). It walks from there to the line in front of the
// camera; the rig checks that straight approach can never pass for the walk
// (the judge's approachClear), here before booting and again on the start the
// walker actually reports.
const SPAWN = [27.5, 1.7, 143.5];
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
  // --dry-verdict re-checks it (`figureVisible`, shared with the meeting in the Commons).
  const vis = figureVisible(Array.isArray(m.screenshots) ? m.screenshots : [], dir);
  add("figure_visible", vis.ok, vis.detail);
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
  if (m.other_build) console.log(otherBuildNotice(m.other_build));
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
if (PLOTS && ENTRY && ENTRY !== "menu" && ENTRY !== "autopilot") refuse(["--entry must be menu or autopilot"]);
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

// This tree's build (or another, on purpose, with --allow-other-build
// "<reason>", recorded in the manifest as other_build; BUG-133).
const fresh = runFreshGate(EXE, args, { cwd: REPO });
if (fresh.status !== 0) {
  console.error("verify-copresence: REFUSED - see the freshness failure above. Nothing was booted.");
  process.exit(1);
}
const OTHER_BUILD = fresh.other_build;

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
    ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
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
    // listening: what the OS showed it listening on (startRelay refuses
    // anything but loopback, so this rig never raises a firewall prompt).
    manifest.relay = { url: relay.httpUrl, pid: relay.pid, dir: relay.dir, health: relay.health, listening: relay.listening.map((r) => r.line) };
    manifest.steps_ok.relay = relay.health
      ? { ok: true, detail: `${relay.httpUrl} answered /health (pid ${relay.pid}, a copy in ${relay.dir}); ${loopbackNote(relay)}` }
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
    manifest.steps_ok.view = await clearFirstRunWindows();
    step("view", manifest.steps_ok.view.ok, manifest.steps_ok.view.detail);

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
    let shots = [];
    if (onPath) {
      // When the walker reached the line, by the computer's clock: the judge's
      // "on time" check counts the walker's real position from it.
      manifest.walker.on_line_epoch_ms = walkerStarted + onPath.hit.at_s * 1000;
      // The pictures (`takeShots`, shared with the meeting in the Commons).
      shots = await takeShots(WALKER_NAME, manifest.walker.on_line_epoch_ms + DRAW_DELAY_S * 1000, (2 * RADIUS) / SPEED, OUT);
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
    // right: +X is to the right for a camera facing north; `nameplateMoves`).
    manifest.steps_ok.on_screen = nameplateMoves(shots, manifest.camera.read_back.yaw, plan.dir, WALKER_NAME);
    step("on_screen", manifest.steps_ok.on_screen.ok, manifest.steps_ok.on_screen.detail);

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

// ── --plots: homes on plots, in both join orders (increments 1b and 2) ───────
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
// holds a plot), and the game records every frame where it drew it. Then the
// two MEET IN THE COMMONS (increment 2), and the 1b legs and the second boot
// follow (see the header).

const PLOTS_WALKER_NAME = "TestBotPlots"; // the TestBot prefix keeps it off the member list
const PLOTS_WALKER_SEED = "verify-copresence-plots-walker";
const SHIP_FILE = path.join(REPO, "data", "blueprints", "ship_structure.ron");
/** One back-and-forth of the walker's line, seconds (second-player.js walks
 *  2r out and 2r back). */
const CYCLE_S = (4 * RADIUS) / SPEED;
/** Long enough to hold one whole forward leg (2r / speed) wherever in the
 *  cycle the recording happens to start, plus the drawing delay and margins. */
const PLOTS_RECORD_S = Math.ceil(CYCLE_S + (2 * RADIUS) / SPEED + 4);

// THE MEETING (increment 2 of docs/design/ship-homes-and-logistics.md). Where
// the game stands in the Commons, x,y,z,yaw,pitch in ship metres at eye height:
// the clear south strip of the Commons (the Commons is x 65..99, z 20..75; its
// room block ends at z 49 and the round stand at z 62.75, both north of here),
// facing south (yaw pi looks along +Z). The line the walker walks is DISTANCE
// in front, along X, 2 x RADIUS long: (72..80, 70). Both walkers' routes reach
// its start without running along it (checked on the real route, routeClear),
// and with nothing between the camera and the line. The rig refuses a pose the
// game's door points do not put inside the Commons.
const MEET_POSE = opt("--meet-pose", "76,1.7,64,3.14159265,-0.05");
/** The longest step the game is moved in on its way from its door into the
 *  Commons (the showcase `cam` verb), metres: well inside the relay's 100 m. */
const MEET_STEP_M = 40;
/** How fast the walker walks its route out of its home into the Commons, m/s:
 *  a brisk walk, so the route does not take a minute of the run. */
const MEET_ROUTE_SPEED = 3;

/** A clear view: answer every first-run window drawn over the middle of the
 *  screen (FIRST_RUN_BUTTONS) the way a person would, by finding its button by
 *  its text and clicking it, then check it is gone. A sandbox that answered it in
 *  an earlier run finds nothing to click. Returns { ok, detail }. (Shared by the
 *  default rig's step 5b and the meeting in the Commons: on the first --plots run
 *  of increment 2 the autopilot's walker-first game had never been asked, and both
 *  pictures of the meeting showed the privacy window and 0 teal pixels.) */
async function clearFirstRunWindows() {
  const cleared = [];
  let ok = true;
  for (const label of FIRST_RUN_BUTTONS) {
    const f = await ui({ action: "find", text: label });
    if (!f || f.ok !== true) {
      ok = false;
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
    ok = ok && gone;
    cleared.push(gone ? `clicked "${label}" at ${fmt(f.pos_px)} px; its window is gone` : `clicked "${label}" but it is STILL drawn`);
  }
  return { ok, detail: cleared.join("; ") };
}

/** The game's door points (debug/door_points_request.json, src/ship/door_points.rs). */
async function doorPointsOf() {
  clearDone("door_points_done.json");
  req("door_points_request.json", {});
  return waitFile("door_points_done.json", 15000);
}

/** Two screenshots a moment apart while the walker crosses its line: a third
 *  and two thirds of the way along, timed from when it reached the line plus
 *  the drawing delay. Right before each, the game is asked where it drew the
 *  walker's NAMEPLATE (the ui `find` verb, exact text): the game's own word that
 *  the figure is on screen, and where. Copies each PNG into `out` as
 *  `<prefix>shot_a.png` and `<prefix>shot_b.png`. */
async function takeShots(name, onLineAt, passS, out, prefix = "") {
  const shots = [];
  for (const [label, share] of [["shot_a", 0.3], ["shot_b", 0.65]]) {
    const due = onLineAt + share * passS * 1000;
    if (Date.now() < due) await sleep(due - Date.now());
    const np = await ui({ action: "find", text: name });
    const requested = (Date.now() - onLineAt) / 1000;
    const s = await screenshot(`${prefix}${label}`);
    // When the capture was done, by the computer's clock: the capture's own long frame (400+ ms
    // on a busy machine) is the rig disturbing the drawing, so the meeting judges its walk on a
    // forward leg after the last of them (meetChecks).
    const taken = Date.now();
    const nameplate = np && np.found && np.text === name ? { pos_px: np.pos_px, rect_px: np.rect_px } : null;
    const npNote = np ? (np.found ? `found "${np.text}"` : "not drawn") : "no answer";
    if (s.ok) {
      fs.copyFileSync(s.src, path.join(out, `${prefix}${label}.png`));
      shots.push({ file: `${prefix}${label}.png`, s_after_reaching_line: Number(requested.toFixed(2)), expected_along_m: Number((SPEED * requested).toFixed(2)), nameplate, nameplate_find: npNote, taken_epoch_ms: taken });
    } else {
      shots.push({ file: null, error: s.error, nameplate, nameplate_find: npNote, taken_epoch_ms: taken });
    }
  }
  return shots;
}

/** The figure is VISIBLE in both pictures, not just drawn: its teal body
 *  counted under where the game put its nameplate (the whole picture when no
 *  nameplate position was recorded). Computed from the PNGs, so --dry-verdict
 *  re-checks it. Returns { ok, detail }. */
function figureVisible(shots, dir) {
  const seenIn = (shots || []).map((sh) => {
    if (!sh.file || !fs.existsSync(path.join(dir, sh.file))) return { file: sh.file, ok: false, note: "no picture" };
    const at = sh.nameplate && sh.nameplate.pos_px ? sh.nameplate.pos_px : null;
    const f = figurePixels(png.decode(fs.readFileSync(path.join(dir, sh.file))), at);
    return { file: sh.file, ok: f.count >= FIGURE_MIN_PX, note: `${f.count} teal px ${at ? "under its nameplate" : "in the whole picture"}${f.centroid ? ` (centre x ${f.centroid[0].toFixed(0)})` : ""}` };
  });
  return {
    ok: seenIn.length === 2 && seenIn.every((x) => x.ok),
    detail: seenIn.length ? `${seenIn.map((x) => `${x.file}: ${x.note}`).join("; ")} (at least ${FIGURE_MIN_PX} each)` : "no screenshots",
  };
}

/** The nameplate was on screen at both moments and moved across the screen the
 *  way the walk goes (the walk's direction along the camera's right). */
function nameplateMoves(shots, yaw, dir, name) {
  const [na, nb] = [shots[0] && shots[0].nameplate, shots[1] && shots[1].nameplate];
  const sideways = dir[0] * Math.cos(yaw) + dir[2] * Math.sin(yaw); // camera right = (cos yaw, 0, sin yaw); +1: walks right
  const ok = !!(na && nb && (nb.pos_px[0] - na.pos_px[0]) * sideways > 0);
  return {
    ok,
    detail:
      na && nb
        ? `${name}'s nameplate drawn at x ${na.pos_px[0].toFixed(0)} px then x ${nb.pos_px[0].toFixed(0)} px, moving ${sideways > 0 ? "right" : "left"} as the walk goes` + (ok ? "" : ": WRONG WAY or not at all")
        : `nameplate at the two screenshots: ${shots.map((s) => s.nameplate_find).join(", ") || "never asked"}`,
  };
}

/** The meeting's checks (increment 2), from the manifest's `meet` record and
 *  its samples: the rig's own steps, judgeMeet (the game's way in, the walker's
 *  way out through its corridor, each seeing the other), the walk itself with
 *  the VIEW (judgeCopresence, checkView on), and both pictures. All ids start
 *  meet_. Re-runs from the manifest. */
function meetChecks(m, dir) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `meet_${id}`, ok: !!ok, detail });
  const s = m.steps_ok || {};
  const meet = m.meet;
  if (!meet) {
    add("ran", false, (s.meet && s.meet.detail) || "the meeting in the Commons never ran");
    return checks;
  }
  add("camera_parked", s.meet_camera && s.meet_camera.ok, s.meet_camera ? s.meet_camera.detail : "never parked");
  // (A manifest from before the view was cleared at the meeting has no record: judged by the pictures alone.)
  if (s.meet_view) add("view_clear", s.meet_view.ok, s.meet_view.detail);
  add("route_clear", meet.route_clear, `the walker's route ${(meet.walker_route || []).map((p) => `(${p.map((v) => Number(v).toFixed(1)).join(", ")})`).join(" -> ")} ${meet.route_clear ? "never runs along the line" : "RUNS ALONG THE LINE and could pass for the walk"}`);
  const samplesPath = meet.samples ? path.join(dir, meet.samples) : null;
  const frames = samplesPath && fs.existsSync(samplesPath) ? JSON.parse(fs.readFileSync(samplesPath, "utf8")).frames || [] : [];
  const walker = { id: meet.walker ? meet.walker.id : null, name: (meet.walker && meet.walker.name) || "TestBotPlots" };
  const walkerDrawn = [];
  for (const fr of frames) for (const p of fr.players || []) if (walker.id !== null && Number(p.id) === Number(walker.id)) walkerDrawn.push(p.pos.map(Number));
  for (const c of judgeMeet({ ...meet, walkerDrawn }).checks) checks.push(c);
  if (frames.length && meet.line) {
    // The walk is judged on the first forward leg AFTER the pictures (the walker walks the line
    // back and forth): each capture holds the game for one long frame (465 ms on a busy machine,
    // 2026-10-04), its figure buffer runs dry, and the frames after it ease back at up to 1.86 m/s
    // for a 1.4 m/s walk. That is the rig disturbing the drawing, not the drawing; the at-home
    // walk is judged on a forward leg the same way. A manifest with no capture times (written
    // before this) is judged on the first pass, as it was.
    const onLine = meet.walker ? meet.walker.on_line_epoch_ms : null;
    const shotTimes = (meet.screenshots || []).map((s) => Number(s.taken_epoch_ms)).filter(Number.isFinite);
    const legMs = ((2 * meet.line.radius) / m.speed) * 1000;
    const leg = Number.isFinite(onLine) && shotTimes.length ? forwardLegStart(onLine, m.speed, meet.line.radius, Math.max(...shotTimes) + 500) : onLine;
    const epochs = frames.map((f) => Number(f.epoch_ms)).filter(Number.isFinite);
    const whole = Number.isFinite(leg) && epochs.length && leg + legMs + 500 <= epochs[epochs.length - 1];
    add(
      "leg_recorded",
      whole,
      Number.isFinite(leg) && epochs.length
        ? `the judged forward leg starts ${((leg - (onLine || leg)) / 1000).toFixed(2)} s after the walker reached the line (${leg === onLine ? "the first pass: this run recorded no capture times" : "the first after the pictures"}) and lasts ${(legMs / 1000).toFixed(2)} s; the recording ran to ${((epochs[epochs.length - 1] - leg) / 1000).toFixed(2)} s past its start` +
            (whole ? "" : ": NOT all of it inside the recording")
        : "no time the walker reached the line, or no frame times",
    );
    if (whole) {
      const j = judgeCopresence({ frames, walker, line: meet.line, speed: m.speed, onLineEpochMs: leg, fromEpochMs: leg === onLine ? null : leg, checkView: true });
      for (const c of j.checks) checks.push({ ...c, id: `meet_${c.id}` });
    }
  } else {
    add("recorded", false, samplesPath ? `no frames in ${rel(samplesPath)}` : "nothing was recorded at the meeting");
  }
  const vis = figureVisible(meet.screenshots, dir);
  add("figure_visible", vis.ok, vis.detail);
  const np = nameplateMoves(meet.screenshots || [], meet.camera_yaw || 0, meet.line_dir || [1, 0, 0], walker.name);
  add("on_screen", np.ok, np.detail);
  return checks;
}

/** The --plots verdict: rig steps, the plot checks, the smoothness checks on
 *  one forward leg of the walk at home, the meeting in the Commons (with the
 *  view and the pictures), the 1b legs and the second boot. Re-runs from the
 *  manifest. */
function plotsVerdict(m, dir) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const s = m.steps_ok || {};
  add("relay_up", s.relay && s.relay.ok, s.relay ? s.relay.detail : "never started");
  // How the game came into the world (a menu entry must have run the race). A
  // manifest from before --entry existed entered by autopilot: every run did.
  for (const c of judgeEntry(m.entry || { kind: "autopilot" }).checks) checks.push(c);
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
      // At home the two stand in their own homes and cannot see each other: no view check here
      // (the meeting below has it).
      const j = judgeCopresence({ frames, walker, line: m.line, speed: m.speed, onLineEpochMs: leg, fromEpochMs: leg, checkView: false });
      for (const c of j.checks) checks.push(c);
      stats = j.stats;
    }
  } else {
    add("forward_leg_recorded", false, "no line, no time the walker reached it, or no frame times: the walk cannot be judged");
  }
  // The meeting in the Commons (increment 2).
  for (const c of meetChecks(m, dir)) checks.push(c);
  // Stepping out of the shared world and back (the second review of 1b).
  if (m.rejoin) {
    for (const c of judgeRejoin(m.rejoin).checks) checks.push(c);
  } else {
    add("rejoin_ran", false, (s.rejoin && s.rejoin.detail) || "the step out of the shared world and back never ran");
  }
  // Respawn far from the door (the third review of 1b).
  if (m.respawn) {
    for (const c of judgeRejoin(m.respawn, { prefix: "respawn", when: "it pressed Respawn" }).checks) checks.push(c);
  } else {
    add("respawn_ran", false, (s.respawn && s.respawn.detail) || "the walk away and Respawn never ran");
  }
  // Shutting the build editor far from the build spot (round 5 of the 1b review).
  if (m.editor) {
    for (const c of judgeEditorClose(m.editor).checks) checks.push(c);
  } else {
    add("editor_ran", false, (s.editor && s.editor.detail) || "the walk away and the build editor's open and shut never ran");
  }
  // The second boot against the same relay (increment 2, the remembered plot).
  if (m.reboot) {
    for (const c of judgeReboot(m.reboot).checks) checks.push(c);
  } else {
    add("reboot_ran", false, (s.reboot && s.reboot.detail) || "the second boot never ran");
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
  if (m.other_build) console.log(otherBuildNotice(m.other_build));
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

/** Forget every plot the rig sandbox's game remembers (increment 2), so each
 *  run's first boot builds on the default plot as a newcomer's does, whatever
 *  port an earlier run's relay happened to share with this one's. */
function forgetSandboxPlots() {
  const cfg = path.join(RIG, "config.json");
  if (!fs.existsSync(cfg)) return;
  try {
    const c = JSON.parse(fs.readFileSync(cfg, "utf8"));
    if (c.home_plots && Object.keys(c.home_plots).length) {
      c.home_plots = {};
      fs.writeFileSync(cfg, JSON.stringify(c, null, 2));
      log("     forgot the plots the sandbox's game remembered from earlier runs");
    }
  } catch (e) {
    log(`     could not read the sandbox's config.json (${e.message}); left as it is`);
  }
}

/** One --plots run in one join order. Returns true when every check passed. */
async function runPlotsOnce(order, runStamp, cleanups) {
  setupRig();
  forgetSandboxPlots();
  const out = path.join(RIG, "runs", `${runStamp}-plots-${order}`);
  fs.mkdirSync(out, { recursive: true });
  const manifest = {
    kind: "verify-copresence-plots",
    order,
    entry: { kind: entryFor(order) },
    stamp: runStamp,
    exe: EXE,
    ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
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
    door_points: null,
    meet: null,
    reboot: null,
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
  // PANIC lines of a first session's run.log, kept when the second boot starts a fresh one.
  let earlierPanics = 0;
  const killGame = () => {
    if (gamePid) {
      try {
        execSync(`taskkill /PID ${gamePid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    killRigProcesses();
  };
  const killAll = () => {
    if (killed) return;
    killed = true;
    if (walker && walker.exitCode === null) {
      try {
        execSync(`taskkill /PID ${walker.pid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    killGame();
    if (relay) {
      relay.kill();
      relay.removeDir();
    }
  };
  cleanups.push(killAll);
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min; killing everything`);
    manifest.steps.push({ id: "timeout", ok: false, detail: `run exceeded ${TIMEOUT_MS / 60000} min` });
    manifest.panics = earlierPanics + panicCount();
    save();
    killAll();
    printPlotsVerdict("RESULT: ", manifest, out);
    process.exit(2);
  }, TIMEOUT_MS);

  // Every line every walker of this run printed, with the computer's clock: the
  // first walker walks at home, the second (the same identity, after the first
  // stepped out) walks to the meeting and stays for the 1b legs.
  const walkerOut = [];
  let walkerExit = null;
  const waitLine = async (re, timeoutMs, from = 0) => {
    let ended = false;
    if (walkerExit) walkerExit.then(() => (ended = true));
    for (const t0 = Date.now(); Date.now() - t0 < timeoutMs; ) {
      const hit = walkerOut.slice(from).find((o) => re.test(o.line));
      if (hit) return { hit, m: hit.line.match(re) };
      if (ended) return null;
      await sleep(50);
    }
    return null;
  };
  /** Start a walker with `extra` arguments after the shared ones; returns the
   *  index in walkerOut its lines start at. */
  const spawnWalker = (extra) => {
    const args = [
      path.join(__dirname, "second-player.js"),
      "--server", relay.url,
      "--name", PLOTS_WALKER_NAME,
      "--seed", PLOTS_WALKER_SEED,
      "--path", "line",
      "--radius", String(RADIUS),
      "--speed", String(SPEED),
      "--seconds", "0", // until this rig stops it
      ...extra,
    ];
    const from = walkerOut.length;
    const started = Date.now();
    // Its input is a pipe: a line "stop" ends it cleanly (stopWalker).
    walker = spawn(process.execPath, args, { cwd: REPO, stdio: ["pipe", "pipe", "pipe"] });
    const take = (b) => {
      for (const line of String(b).split(/\r?\n/).filter(Boolean)) walkerOut.push({ at_s: (Date.now() - started) / 1000, epoch: Date.now(), line });
    };
    walker.stdout.on("data", take);
    walker.stderr.on("data", take);
    walkerExit = new Promise((r) => walker.on("exit", (code) => r(code)));
    return { from, args: args.slice(1) };
  };
  /** Stop the walker the way Ctrl+C does (game_leave, then close): the relay
   *  takes its figure out at once instead of holding it for the 90 s grace. */
  const stopWalker = async () => {
    if (!walker || walker.exitCode !== null) return true;
    try {
      walker.stdin.write("stop\n");
    } catch {}
    const code = await Promise.race([walkerExit, sleep(10000).then(() => "still running")]);
    if (code === "still running") {
      try {
        execSync(`taskkill /PID ${walker.pid} /T /F`, { stdio: "ignore" });
      } catch {}
      return false;
    }
    return true;
  };
  /** Start the walker at home and wait until it is walking: its entity, the
   *  plot the relay gave it, its line, and when it reached the line. */
  const startWalker = async () => {
    const { from, args } = spawnWalker(["--axis", "z"]);
    manifest.walker = { name: PLOTS_WALKER_NAME, seed: PLOTS_WALKER_SEED, args, id: null, start: null };
    const inWorld = await waitLine(/in the world as entity (\d+), starting at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/, 40000, from);
    const plotLine = await waitLine(/home_plot (null|missing|\{.*\})/, 5000, from);
    const centred = await waitLine(/centred on \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/, 5000, from);
    const onPath = await waitLine(/on the path at/, 15000, from);
    if (!inWorld || !centred || !onPath) {
      manifest.steps_ok.walker = { ok: false, detail: `the walker never got walking: ${walkerOut.slice(from).map((o) => o.line).join(" | ")}` };
      step("walker", false, manifest.steps_ok.walker.detail);
      throw new Error("the walker never got walking");
    }
    manifest.walker.id = Number(inWorld.m[1]);
    manifest.walker.start = [Number(inWorld.m[2]), Number(inWorld.m[3]), Number(inWorld.m[4])];
    manifest.walker.home_plot_line = plotLine ? plotLine.hit.line : null;
    const hp = plotLine && plotLine.m[1].startsWith("{") ? JSON.parse(plotLine.m[1]) : null;
    manifest.walker_plot = hp ? hp.id : null;
    manifest.walker.on_line_epoch_ms = onPath.hit.epoch;
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
  const showcase = async (body) => {
    req("showcase_request.json", body);
    // The game deletes the request when it takes it (engine/ipc.rs poll_showcase_request).
    for (const t0 = Date.now(); Date.now() - t0 < 10000 && fs.existsSync(path.join(DEBUG, "showcase_request.json")); ) await sleep(100);
  };
  const until = async (ok, ms) => {
    let p = null;
    for (const t0 = Date.now(); Date.now() - t0 < ms; ) {
      p = await probe();
      if (p && ok(p)) return p;
      await sleep(500);
    }
    return p;
  };
  const sawRe = /saw entity (\d+) at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/;
  /** Every position the walker logged the relay passing on for `entity` (any
   *  entity but `notEntity` when `entity` is null), from walkerOut index `from`. */
  const seenSince = (from, entity, notEntity = null) =>
    walkerOut
      .slice(from)
      .map((o) => o.line.match(sawRe))
      .filter((x) => x && (entity === null ? Number(x[1]) !== notEntity : Number(x[1]) === entity))
      .map((x) => [Number(x[2]), Number(x[3]), Number(x[4])]);

  /** Boot the game and bring it into the shared world the way this order's
   *  entry does (the autopilot, or the returning player's menu: connect first,
   *  answer the privacy window, press Enter World). `entry` records it. Returns
   *  the probe once joined and welcomed (or the last probe). */
  const bootAndEnter = async (entry, label) => {
    const child = spawn(RIG_EXE, [], { cwd: RIG, detached: true, stdio: "ignore", env: gameEnv() });
    gamePid = child.pid;
    fs.writeFileSync(path.join(RIG, "probe_pid.txt"), String(gamePid));
    child.unref();
    step(`${label}launch`, true, `game pid ${gamePid} from ${rel(RIG_EXE)} (background, no focus)`);
    await waitBoot(180000);
    step(`${label}boot`, true, "booted (run.log: cloud noise volumes generated, no PANIC)");
    const fromMenu = entry.kind === "menu";
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: relay.httpUrl, user_name: "CopresencePlots", character_name: "CopresencePlots", enter: !fromMenu });
    const ap = await waitFile("autopilot_done.json", 300000);
    if (!ap || ap.ok !== true) throw new Error(`the autopilot did not run: ${ap ? ap.error || JSON.stringify(ap) : "no answer in 300 s"}`);
    if (fromMenu && ap.entered !== false) throw new Error(`the autopilot entered the world although asked not to (entered=${ap.entered}): this build predates "enter": false`);
    step(`${label}autopilot`, true, fromMenu ? `connecting from the main menu on ${ap.server_url}` : `entering the world on ${ap.server_url}`);
    if (fromMenu) {
      // The returning player's path: connected and identified on the main menu,
      // then Enter World pressed. Wait for the handshake with the world unloaded.
      let mp = null;
      for (const t0 = Date.now(); Date.now() - t0 < 60000; ) {
        mp = await probe();
        if (mp && mp.ok && mp.ws_identified) break;
        await sleep(500);
      }
      // The first-run privacy window opens over the middle of the menu once this
      // identity connects; give it a moment, then answer it as a person would.
      await sleep(1500);
      const cleared = [];
      for (const lbl of FIRST_RUN_BUTTONS) {
        const f = await ui({ action: "find", text: lbl });
        if (f && f.ok === true && f.found && f.text === lbl) {
          await ui({ action: "click", pos: f.pos_px });
          await sleep(800);
          cleared.push(`clicked "${lbl}"`);
        }
      }
      const before = await probe();
      entry.identified = !!(before && before.ws_identified);
      entry.world_loaded = !!(before && before.world_loaded);
      const btn = await ui({ action: "find", text: "Enter World" });
      if (!btn || btn.ok !== true || !btn.found || String(btn.text).trim() !== "Enter World") {
        entry.error = `the main menu's Enter World button was not drawn: ${JSON.stringify(btn)}`;
        step(`${label}enter`, false, entry.error);
        throw new Error(entry.error);
      }
      const click = await ui({ action: "click", pos: btn.pos_px });
      entry.page_after_click = click ? click.active_page : null;
      entry.world_loaded_after_click = click ? !!click.world_loaded : null;
      step(
        `${label}enter`,
        true,
        `identified=${entry.identified} world_loaded=${entry.world_loaded} on the menu${cleared.length ? ` (${cleared.join(", ")})` : ""}; ` +
          `pressed Enter World at ${fmt(btn.pos_px)} px: page ${entry.page_after_click}, world_loaded=${entry.world_loaded_after_click}`,
      );
    }
    // In the shared world, with the welcome applied (a build from before 1b
    // reports no `welcomed`, and is read as soon as it has joined).
    let pr = null;
    for (const t0 = Date.now(); Date.now() - t0 < 120000; ) {
      pr = await probe();
      if (pr && pr.ok && pr.world_loaded && pr.game_joined && pr.copresence_active && pr.welcomed !== false) break;
      if (pr && pr.copresence_refused) break;
      await sleep(1000);
    }
    return pr;
  };

  try {
    relay = await TR.startRelay({ sourceExe: EXE, prefix: "verify-copresence-relay-", config: { server_name: "verify-copresence plots relay" } });
    manifest.relay = { url: relay.httpUrl, pid: relay.pid, dir: relay.dir, health: relay.health, listening: relay.listening.map((x) => x.line) };
    manifest.steps_ok.relay = relay.health
      ? { ok: true, detail: `${relay.httpUrl} answered /health (pid ${relay.pid}); ${loopbackNote(relay)}` }
      : { ok: false, detail: `${relay.httpUrl} never answered /health: ${relay.logText().slice(-400)}` };
    step("relay", manifest.steps_ok.relay.ok, manifest.steps_ok.relay.detail);
    if (!relay.health) throw new Error("the throwaway relay did not come up");

    if (order === "walker-first") await startWalker();

    const pr = await bootAndEnter(manifest.entry, "");
    const joined = !!(pr && pr.ok && pr.game_joined && pr.copresence_active && pr.welcomed !== false);
    manifest.steps_ok.joined = {
      ok: joined,
      detail: pr
        ? `world_loaded=${pr.world_loaded} game_joined=${pr.game_joined} copresence_active=${pr.copresence_active} welcomed=${pr.welcomed} refused=${pr.copresence_refused}` +
          (pr.copresence_refused_note ? ` ("${pr.copresence_refused_note}")` : "")
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
      `the game's home stands on ${manifest.game_plot || "(no plot reported)"} (built on ${pj ? pj.boot_plot : "?"} at boot, the welcome did "${pj ? pj.last_welcome : "?"}"); its camera at ${manifest.camera_after_join ? fmt(manifest.camera_after_join) : "(none)"}; plots from ${manifest.plots_from}`,
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

    // ── MEET IN THE COMMONS (increment 2). The game reports its door points
    // (from its own corridor geometry); the rig routes both players on them.
    // The game is moved from its door into the Commons in steps the relay
    // accepts and stands at the meeting pose; the walker steps out of the world
    // and back in at ITS door, walks out through its corridor into the Commons
    // and along the line in front of the game's camera; the game records where
    // it drew it, and photographs it.
    const dp = await doorPointsOf();
    if (!dp || dp.ok !== true) {
      manifest.steps_ok.meet = { ok: false, detail: `the game reported no door points: ${JSON.stringify(dp)}` };
      step("doors", false, manifest.steps_ok.meet.detail);
      throw new Error("no door points");
    }
    manifest.door_points = dp;
    step("doors", true, `the game reported ${dp.places.length} places (${dp.places.map((p) => p.id).join(", ")}) and ${dp.doors.length} doors`);
    const commons = dp.places.find((p) => p.purpose === "commons");
    const [mx, my, mz, myaw, mpitch] = MEET_POSE.split(",").map(Number);
    const meetCam = [mx, my, mz];
    const plan = planLine(meetCam, myaw, DISTANCE, RADIUS);
    const inCommons = (p) => commons && placeAt({ places: [commons] }, p);
    if (!commons || plan.error || !inCommons(meetCam) || !inCommons(plan.start) || !inCommons(plan.end)) {
      manifest.steps_ok.meet = { ok: false, detail: `the meeting pose ${MEET_POSE} or its line is not inside the Commons the game reports (${commons ? JSON.stringify([commons.min, commons.max]) : "no Commons"})${plan.error ? `: ${plan.error}` : ""}` };
      step("meet", false, manifest.steps_ok.meet.detail);
      throw new Error("no meeting place");
    }
    const ownPlot = dp.places.find((p) => p.kind === "plot" && p.own);
    const gameDoor = ownPlot && ownPlot.door ? ownPlot.door : manifest.home_things && manifest.home_things.respawn;
    const gameWalk = doorRoute(dp, gameDoor, meetCam, MEET_STEP_M);
    if (gameWalk.error) throw new Error(`no route for the game: ${gameWalk.error}`);
    const at0 = await probe();
    const yaw0 = at0 && at0.camera_end ? at0.camera_end.yaw : 0;
    const pitch0 = at0 && at0.camera_end ? at0.camera_end.pitch : 0;
    // From its door, so the walk is the one a person takes out of their home.
    const markGame = walkerOut.length;
    const gameSteps = [gameDoor, ...gameWalk.points];
    for (const p of gameSteps) {
      await showcase({ cam: `${p.join(",")},${yaw0},${pitch0}` });
      await sleep(1500);
    }
    await showcase({ cam: MEET_POSE });
    await sleep(2500);
    const c1 = await probe();
    await sleep(1000);
    const c2 = await probe();
    const camA = c1 && c1.camera_end;
    const camB = c2 && c2.camera_end;
    const drift = camA && camB ? Math.hypot(...camB.pos.map((v, i) => v - camA.pos[i])) : Infinity;
    const turn = camA && camB ? Math.abs(camB.yaw - camA.yaw) : Infinity;
    const off = camB ? Math.hypot(...camB.pos.map((v, i) => v - meetCam[i])) : Infinity;
    const stillIn = !!(c2 && c2.game_joined && c2.copresence_active);
    const parked = drift < 0.01 && turn < 0.001 && stillIn && off < 0.5;
    manifest.steps_ok.meet_camera = {
      ok: parked,
      detail: camB
        ? `the game moved from its door ${fmt(gameDoor)} through ${gameWalk.doors.join(", ")} in ${gameSteps.length} steps to ${fmt(camB.pos)} yaw ${camB.yaw.toFixed(3)} (asked ${MEET_POSE}, ${off.toFixed(3)} m off); ` +
          `${drift < 0.01 && turn < 0.001 ? "holding still" : `NOT holding still (${drift.toFixed(3)} m, ${turn.toFixed(4)} rad)`}; ${stillIn ? "still in the shared world" : "NOT in the shared world any more"}`
        : "the game never reported its camera",
    };
    step("meet_cam", parked, manifest.steps_ok.meet_camera.detail);
    // A clear view of the line before the walker comes (the first-run privacy window).
    manifest.steps_ok.meet_view = await clearFirstRunWindows();
    step("meet_view", manifest.steps_ok.meet_view.ok, manifest.steps_ok.meet_view.detail);
    // Where the relay held the game when it got there: the last of its moves the
    // walker at home saw (any entity but the walker's own).
    const gameSeen = seenSince(markGame, null, manifest.walker.id);
    const gameHeld = gameSeen.length ? gameSeen[gameSeen.length - 1] : null;
    step("meet_held", !!gameHeld, `the relay passed on ${gameSeen.length} of the game's moves; the last at ${gameHeld ? fmt(gameHeld) : "(never)"}`);

    // The walker steps out (cleanly: the relay takes its figure out at once) and
    // comes back at its own door, then walks out through its corridor.
    const left = await stopWalker();
    step("meet_out", left, left ? `${PLOTS_WALKER_NAME} stepped out of the shared world (game_leave)` : "the walker did not stop when asked; it was killed (the relay holds its figure for its grace)");
    await sleep(1500);
    const wPlace = dp.places.find((p) => p.id === `plot:${manifest.walker_plot}`);
    if (!wPlace || !wPlace.door) throw new Error(`the walker's plot ${manifest.walker_plot} is not in the door points`);
    const wDoor = dp.doors.find((d) => d.from === wPlace.id || d.to === wPlace.id);
    const wRoute = doorRoute(dp, wPlace.door, plan.start);
    if (wRoute.error) throw new Error(`no route for the walker: ${wRoute.error}`);
    const routeArg = wRoute.waypoints.slice(0, -1); // the line's start is where the path begins
    const routeClearOk = routeClear([wPlace.door, ...wRoute.waypoints], { start: plan.start, end: plan.end });
    const routeLen = [wPlace.door, ...wRoute.waypoints].slice(1).reduce((a, p, i, all) => a + Math.hypot(...p.map((v, k) => v - (i ? all[i - 1] : wPlace.door)[k])), 0);
    // Sign-in, the route, the first pass (the pictures), back, and the forward leg after it
    // that the walk is judged on (meetChecks), with margins.
    const meetRecordS = Math.min(115, Math.ceil(10 + routeLen / MEET_ROUTE_SPEED + CYCLE_S + (2 * RADIUS) / SPEED + 8));
    clearDone("remote_players_done.json");
    req("remote_players_request.json", { seconds: meetRecordS });
    step("meet_record", true, `recording ${meetRecordS} s: the walker's ${routeLen.toFixed(1)} m route through ${wRoute.doors.join(", ")} at ${MEET_ROUTE_SPEED} m/s, then the line`);
    const w2 = spawnWalker([
      "--axis", plan.axis,
      "--center", plan.center.join(","),
      "--route", routeArg.map((p) => p.join(",")).join(";"),
      "--route-speed", String(MEET_ROUTE_SPEED),
      "--home-spawn", wPlace.door_local.join(","),
    ]);
    const inWorld2 = await waitLine(/in the world as entity (\d+), starting at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/, 40000, w2.from);
    if (!inWorld2) throw new Error(`the walker never came back into the world: ${walkerOut.slice(w2.from).map((o) => o.line).join(" | ")}`);
    const w2id = Number(inWorld2.m[1]);
    const w2start = [Number(inWorld2.m[2]), Number(inWorld2.m[3]), Number(inWorld2.m[4])];
    const atDoor = Math.hypot(...w2start.map((v, i) => v - wPlace.door[i]));
    step("meet_walker", atDoor < 0.5, `${PLOTS_WALKER_NAME} back in the world as entity ${w2id} at ${fmt(w2start)}, ${atDoor.toFixed(2)} m from its door ${fmt(wPlace.door)}; walking out through ${wRoute.doors.join(", ")}`);
    const onPath2 = await waitLine(/on the path at/, (routeLen / MEET_ROUTE_SPEED + 30) * 1000, w2.from);
    let shots = [];
    if (onPath2) {
      shots = await takeShots(PLOTS_WALKER_NAME, onPath2.hit.epoch + DRAW_DELAY_S * 1000, (2 * RADIUS) / SPEED, out, "meet_");
    }
    step("meet_shots", shots.length === 2 && shots.every((s) => s.file), onPath2 ? shots.map((s) => (s.file ? `${s.file} at ${s.s_after_reaching_line} s into the line` : `failed: ${s.error}`)).join("; ") : "the walker never reached the line");
    const presentRe = /player present: entity (\d+) "[^"]*" at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/;
    const recM = await waitFile("remote_players_done.json", (meetRecordS + 60) * 1000);
    if (recM && recM.ok === true) {
      fs.copyFileSync(path.join(DEBUG, "remote_players_done.json"), path.join(out, "meet_samples.json"));
      step("meet_samples", true, `${recM.frame_count} frames over ${recM.recorded_s.toFixed(2)} s of frame clock`);
    } else {
      step("meet_samples", false, `no recording came back: ${JSON.stringify(recM && recM.error)}`);
    }
    // What the walker saw of the game: the relay's word when it joined, and any move after.
    const sawGame = walkerOut
      .slice(w2.from)
      .map((o) => o.line.match(presentRe) || o.line.match(sawRe))
      .filter((x) => x && Number(x[1]) !== w2id)
      .map((x) => [Number(x[2]), Number(x[3]), Number(x[4])]);
    manifest.meet = {
      pose: MEET_POSE,
      commons: { min: commons.min, max: commons.max },
      line: { start: plan.start, end: plan.end, center: plan.center, axis: plan.axis, radius: RADIUS },
      line_dir: plan.dir,
      camera_yaw: camB ? camB.yaw : myaw,
      gameFrom: gameDoor,
      gameSteps: [...gameSteps.slice(1), camB ? camB.pos : meetCam],
      gameCamera: camB ? camB.pos : null,
      gameHeld,
      walkerTube: wDoor ? { min: wDoor.tube[0], max: wDoor.tube[1] } : null,
      walkerSawGame: sawGame,
      walker_route: [wPlace.door, ...wRoute.waypoints],
      route_clear: routeClearOk,
      walker: { id: w2id, name: PLOTS_WALKER_NAME, start: w2start, on_line_epoch_ms: onPath2 ? onPath2.hit.epoch : null },
      samples: recM && recM.ok === true ? "meet_samples.json" : null,
      screenshots: shots,
    };
    manifest.steps_ok.meet = { ok: true, detail: "the meeting ran" };
    save();
    // From here on the walker of the 1b legs is the one at the meeting (still walking its line).
    manifest.walker.meet_id = w2id;

    // ── Step out of the shared world and back (the second review of 1b). The
    // relay takes the game out at its game_leave and, when it joins again,
    // spawns it AFRESH at its door, wherever the game stands. Out of the world,
    // the game is moved to the place on the ship farthest from its door (more
    // than 100 m: one of the shared zones' corners, from the door points), so a
    // game that stayed where it stood would have every update refused by the
    // relay's 100 m rule. The walker (still walking) logs where the relay
    // spawned it and every move the relay passes on.
    const before = await probe();
    const door = before && before.home_things ? before.home_things.respawn : null;
    const yaw = before && before.camera_end ? before.camera_end.yaw : 0;
    const pitch = before && before.camera_end ? before.camera_end.pitch : 0;
    const farTarget = farthestFrom(farPlaces(dp), door || gameDoor);
    const mark = walkerOut.length;
    await showcase({ solo: "1" });
    const outside = await until((p) => p.game_joined === false, 20000);
    const stepped = !!(outside && outside.game_joined === false);
    step("step_out", stepped, stepped ? "the game stepped out of the shared world (solo)" : `the game did not step out: game_joined=${outside && outside.game_joined}`);
    if (!stepped) throw new Error("the game never stepped out of the shared world");
    // EVIDENCE, not judged: out of the world (no move is sent), a picture of the neighbour's
    // plot from the shared zone its corridor runs to, 2 m in from its mouth, looking back down
    // the corridor at its door: the hole in the zone's wall, the corridor, and the neighbour's
    // home drawn as the default design (increment 2, src/ship/neighbours.rs).
    const nbr = dp.doors.find((d) => d.from.startsWith("plot:") && !(dp.places.find((p) => p.id === d.from) || {}).own);
    if (nbr) {
      const dir = [0, 1, 2].map((k) => nbr.mouths[1][k] - nbr.mouths[0][k]);
      const dl = Math.hypot(dir[0], dir[2]) || 1;
      const eye = [nbr.mouths[1][0] + (dir[0] / dl) * 2, nbr.mouths[1][1] + 1.7, nbr.mouths[1][2] + (dir[2] / dl) * 2];
      const lookYaw = Math.atan2(-dir[0] / dl, dir[2] / dl);
      await showcase({ cam: `${eye.join(",")},${lookYaw},0` });
      await sleep(2500);
      const shot = await screenshot("neighbour");
      if (shot.ok) fs.copyFileSync(shot.src, path.join(out, "neighbour.png"));
      manifest.neighbour_view = { door: `${nbr.from}->${nbr.to}`, eye, yaw: lookYaw, file: shot.ok ? "neighbour.png" : null, error: shot.ok ? null : shot.error };
      step("neighbour", true, `evidence: ${shot.ok ? "neighbour.png" : `no picture (${shot.error})`}, from ${fmt(eye)} looking down ${nbr.from}'s corridor`);
    }
    await showcase({ cam: `${farTarget.join(",")},${yaw},${pitch}` });
    await sleep(2500);
    const atFar = await probe();
    const far = atFar && atFar.camera_end ? atFar.camera_end.pos : null;
    step("far", !!far, `out of the world, moved to ${far ? fmt(far) : "(unknown)"} (asked ${fmt(farTarget)}), ${door && far ? Math.hypot(far[0] - door[0], far[2] - door[2]).toFixed(1) : "?"} m from the door at ${door ? fmt(door) : "?"}`);
    await showcase({ solo: "0" });
    const back = await until((p) => p.game_joined && p.welcomed, 30000);
    const rejoined = !!(back && back.game_joined && back.welcomed);
    step("step_back", rejoined, rejoined ? "the game joined the shared world again and its welcome was applied" : `no rejoin: game_joined=${back && back.game_joined} welcomed=${back && back.welcomed}`);
    if (!rejoined) throw new Error("the game never rejoined the shared world");
    // The game is the only other player, and the walker never logs itself; its name is the
    // relay's (the game may join as "Player"), so it is not matched.
    const nameRe = /player joined: entity (\d+) "[^"]*" at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\)/;
    let joinedLine = null;
    for (const t0 = Date.now(); Date.now() - t0 < 10000 && !joinedLine; ) {
      joinedLine = walkerOut.slice(mark).map((o) => o.line.match(nameRe)).find(Boolean) || null;
      if (!joinedLine) await sleep(200);
    }
    const entity = joinedLine ? Number(joinedLine[1]) : null;
    const relaySpawn = joinedLine ? [Number(joinedLine[2]), Number(joinedLine[3]), Number(joinedLine[4])] : null;
    await sleep(1500);
    const settled = await probe();
    const camera = settled && settled.camera_end ? settled.camera_end.pos : null;
    // One small step, by the same verb a person's walk amounts to: the move the
    // relay must pass on.
    const nudged = camera ? [camera[0], camera[1], camera[2] + 1] : null;
    const markNudge = walkerOut.length;
    if (nudged) await showcase({ cam: `${nudged.join(",")},${yaw},${pitch}` });
    await sleep(2500);
    const seen = seenSince(markNudge, entity);
    manifest.rejoin = { far, relaySpawn, camera, nudged, seen, entity, door, far_target: farTarget };
    manifest.steps_ok.rejoin = { ok: true, detail: `rejoined as entity ${entity}, the relay spawned it at ${relaySpawn ? fmt(relaySpawn) : "(never seen)"}` };
    step("rejoin", true, `${manifest.steps_ok.rejoin.detail}; its camera at ${camera ? fmt(camera) : "(none)"}; ${seen.length} relayed move(s) seen after the nudge to ${nudged ? fmt(nudged) : "(none)"}`);

    // ── Respawn far from the door (the third review of 1b). In the world this
    // time, the game walks to the same far place in steps the relay accepts
    // (through the doors, from the door points), so the relay holds it there (the
    // walker sees it arrive), and presses Respawn (the showcase `respawn` verb).
    // Respawn puts the game at its door, more than 100 m from where the relay
    // holds it: unless the relay stands it there too, every update is refused
    // and the others see it frozen at the far end.
    const route = doorRoute(dp, nudged || door || farTarget, farTarget).points;
    const markWalk = walkerOut.length;
    for (const p of route) {
      await showcase({ cam: `${p.join(",")},${yaw},${pitch}` });
      await sleep(1500);
    }
    const walked = seenSince(markWalk, entity);
    const heldFar = walked.length ? walked[walked.length - 1] : null;
    const atFarEnd = !!heldFar && Math.hypot(heldFar[0] - farTarget[0], heldFar[2] - farTarget[2]) < 3;
    step(
      "walk_away",
      atFarEnd,
      `walked ${route.length} steps to ${fmt(farTarget)}; the relay last passed on the game at ${heldFar ? fmt(heldFar) : "(never)"} (${walked.length} relayed move(s))`,
    );
    const markRespawn = walkerOut.length;
    await showcase({ respawn: "1" });
    let respawnLine = null;
    for (const t0 = Date.now(); Date.now() - t0 < 15000 && !respawnLine; ) {
      respawnLine = walkerOut.slice(markRespawn).map((o) => o.line.match(nameRe)).find(Boolean) || null;
      if (!respawnLine) await sleep(200);
    }
    const respawnEntity = respawnLine ? Number(respawnLine[1]) : entity;
    const relayRespawn = respawnLine ? [Number(respawnLine[2]), Number(respawnLine[3]), Number(respawnLine[4])] : null;
    await until((p) => p.game_joined && p.welcomed, 20000);
    await sleep(1500);
    const afterRespawn = await probe();
    const camera2 = afterRespawn && afterRespawn.camera_end ? afterRespawn.camera_end.pos : null;
    const nudged2 = camera2 ? [camera2[0], camera2[1], camera2[2] + 1] : null;
    const markNudge2 = walkerOut.length;
    if (nudged2) await showcase({ cam: `${nudged2.join(",")},${yaw},${pitch}` });
    await sleep(2500);
    const seen2 = seenSince(markNudge2, respawnEntity);
    manifest.respawn = { far: heldFar, relaySpawn: relayRespawn, camera: camera2, nudged: nudged2, seen: seen2, entity: respawnEntity, route };
    manifest.steps_ok.respawn = {
      ok: true,
      detail: relayRespawn ? `respawned as entity ${respawnEntity}, the relay spawned it at ${fmt(relayRespawn)}` : "the walker never saw the game join again after Respawn",
    };
    step("respawn", true, `${manifest.steps_ok.respawn.detail}; its camera at ${camera2 ? fmt(camera2) : "(none)"}; ${seen2.length} relayed move(s) seen after the nudge`);

    // ── Shutting the build editor far from the build spot (round 5 of the 1b
    // review). At its door after Respawn, the game opens the build editor and
    // shuts it (the showcase `build_editor` verb, the B key's own function):
    // that puts the build-mode avatar where it stands and stands it there, its
    // build spot. Then it walks, in steps the relay accepts, to the same far
    // place, more than 100 m from that spot, and opens and shuts the editor
    // again. Shutting it used to stand the game at the build spot, a jump the
    // relay refuses: frozen at the far place for everyone else.
    // A point 1 m from `at` toward `to`, across the floor (x and z).
    const nudgeToward = (at, to) => {
      const dx = to[0] - at[0];
      const dz = to[2] - at[2];
      const len = Math.hypot(dx, dz) || 1;
      return [at[0] + dx / len, at[1], at[2] + dz / len];
    };
    const editorTo = async (open) => {
      await showcase({ build_editor: open ? "1" : "0" });
      return until((p) => p.build_editor === open, 10000);
    };
    const opened1 = await editorTo(true);
    await sleep(1000);
    const shut1 = await editorTo(false);
    await sleep(1500);
    const atSpot = await probe();
    const buildSpot = atSpot && atSpot.camera_end ? atSpot.camera_end.pos : null;
    const toggled = !!(opened1 && opened1.build_editor === true && shut1 && shut1.build_editor === false);
    step(
      "editor_spot",
      toggled && !!buildSpot,
      toggled
        ? `opened and shut the build editor at the door; the build spot is ${buildSpot ? fmt(buildSpot) : "(unknown)"}`
        : `the build editor did not open and shut (build_editor ${opened1 && opened1.build_editor}, then ${shut1 && shut1.build_editor}); a build without the verb?`,
    );
    if (toggled && buildSpot) {
      const route3 = doorRoute(dp, buildSpot, farTarget).points;
      const markWalk3 = walkerOut.length;
      for (const p of route3) {
        await showcase({ cam: `${p.join(",")},${yaw},${pitch}` });
        await sleep(1500);
      }
      const walked3 = seenSince(markWalk3, respawnEntity);
      const held3 = walked3.length ? walked3[walked3.length - 1] : null;
      step(
        "walk_away2",
        !!held3 && Math.hypot(held3[0] - farTarget[0], held3[2] - farTarget[2]) < 3,
        `walked ${route3.length} steps to ${fmt(farTarget)}; the relay last passed on the game at ${held3 ? fmt(held3) : "(never)"} (${walked3.length} relayed move(s))`,
      );
      const opened2 = await editorTo(true);
      await sleep(1000);
      const shut2 = await editorTo(false);
      await sleep(1500);
      const afterShut = await probe();
      const camera3 = afterShut && afterShut.camera_end ? afterShut.camera_end.pos : null;
      // One metre back along the walk, not on along +Z: the far place can stand at the end of
      // street-1, where +Z runs into its end wall and the step stops short (the first green run
      // of this leg: the relay passed the move, at 194.67 for a nudge to 195).
      const toward = route3.length > 1 ? route3[route3.length - 2] : buildSpot;
      const nudged3 = camera3 ? nudgeToward(camera3, toward) : null;
      const markNudge3 = walkerOut.length;
      if (nudged3) await showcase({ cam: `${nudged3.join(",")},${yaw},${pitch}` });
      await sleep(2500);
      const seen3 = seenSince(markNudge3, respawnEntity);
      manifest.editor = { buildSpot, held: held3, camera: camera3, nudged: nudged3, seen: seen3, route: route3 };
      const shutOk = !!(opened2 && opened2.build_editor === true && shut2 && shut2.build_editor === false);
      manifest.steps_ok.editor = { ok: shutOk, detail: shutOk ? "opened and shut the build editor at the far place" : "the build editor did not open and shut at the far place" };
      step("editor", shutOk, `${manifest.steps_ok.editor.detail}; its camera at ${camera3 ? fmt(camera3) : "(none)"}; ${seen3.length} relayed move(s) seen after the nudge`);
    } else {
      manifest.steps_ok.editor = { ok: false, detail: "the build editor never opened and shut at the door" };
    }

    // ── THE NEXT BOOT (increment 2, the remembered plot). The game steps out
    // (the relay takes its figure out at once), quits, and boots again against
    // the same relay, coming in the same way as before. It remembered its plot
    // on this server at the welcome, so its world load builds the home on that
    // plot before it joins, and the welcome only confirms it (a Stay). In the
    // walker-first order the game holds p2, which is not the default plot, so a
    // game that forgot builds on p1 and fails here.
    await showcase({ solo: "1" });
    await until((p) => p.game_joined === false, 20000);
    killGame();
    for (const t0 = Date.now(); Date.now() - t0 < 15000 && MG.listInstances().some((p) => p.pid === gamePid); ) await sleep(500);
    await sleep(1500);
    earlierPanics += panicCount();
    try {
      fs.copyFileSync(LOG, path.join(out, "run-first.log"));
      fs.truncateSync(LOG, 0);
    } catch {}
    const again = { kind: manifest.entry.kind };
    const pr2 = await bootAndEnter(again, "reboot_");
    const joined2 = !!(pr2 && pr2.ok && pr2.game_joined && pr2.copresence_active && pr2.welcomed !== false);
    await sleep(2000);
    const pb = await probe();
    const heldPlot = manifest.game_plot;
    manifest.reboot = {
      entry: again,
      joined: joined2,
      heldPlot,
      // The relay hands out the ship file's plots in order, so the first is the one a newcomer's
      // game builds on (the ship's default plot).
      defaultPlot: Array.isArray(manifest.plots) && manifest.plots.length ? manifest.plots[0].id : null,
      bootPlot: pb ? pb.boot_plot : null,
      lastWelcome: pb ? pb.last_welcome : null,
      camera: pb && pb.camera_end ? pb.camera_end.pos : null,
      plot: Array.isArray(manifest.plots) ? manifest.plots.find((p) => p.id === heldPlot) || null : null,
    };
    manifest.steps_ok.reboot = { ok: joined2, detail: joined2 ? "the game booted again and joined" : "the second boot never joined" };
    step("reboot", joined2, `the second boot built the home on ${manifest.reboot.bootPlot} (it held ${heldPlot}); its welcome did "${manifest.reboot.lastWelcome}"; its camera at ${manifest.reboot.camera ? fmt(manifest.reboot.camera) : "(none)"}`);
  } catch (e) {
    manifest.steps.push({ id: "abort", ok: false, detail: String(e.message || e) });
    log(`ABORT ${e.message || e}`);
  }
  clearTimeout(watchdog);
  manifest.panics = earlierPanics + panicCount();
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
  await stopWalker();
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
