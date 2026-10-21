#!/usr/bin/env node
// verify-storeys: proves in the RUNNING game that the homestead's upper floor works (ship homes
// increment 7, 2026-10-21, `just verify-storeys`): docs/design/ship-homes-increment-7-plan.md
// section 3 "Rig (V)"; the layout as built: docs/design/homestead-upper-floor.md.
//
// Increment 7 landed in five lanes (geometry, movement, rules, content, editor), each with unit
// tests over a small synthetic two-storey home, and nothing that walked the real homestead in the
// running game. This boots the release binary in its own portable rig (.probe-rig/storeys:
// background, never focused, silent, no server), enters the world through the dev autopilot (the
// rig's own home, the homestead, as verify-falls does), and walks it on the held movement keys
// (the showcase `hold` verb, the controller's own action path), turning between straight legs with
// the `face` verb (a mouse turn: nothing moves the body but the walk). The legs:
//
//   stairs_up        placed at the front door (the home's spawn), walk west down the hall to the
//                    stair's foot and up stairs-1 to the landing past its top: the floor stood on
//                    is the upper floor (3.0 m), the HUD names the upper hall, no landing reaches
//                    the body, the body walked every metre (no frame faster than a walk).
//   bedroom          on from there: south onto the gallery, east to the bedroom door, north 2 m
//                    into the bedroom: the HUD names it, the body never leaves the upper floor.
//   wall_upstairs    placed in the dressing room upstairs, forward held into U7 (the wall between
//                    it and the bathroom): the eye stays on its side.
//   guard            placed on the gallery, forward held into G1, the balustrade over the open
//                    stairwell: the body stays on the gallery and never falls.
//   wall_downstairs  placed in the second bedroom on the ground floor, walked east across U7's
//                    line: downstairs it walks through freely (the wall is upstairs).
//   jump_under_slab  placed in the ground hall under the gallery's floor, Space: the eye never
//                    rises past the slab's underside less the head allowance (the head clamp).
//   jump_open        the same jump in the greenhouse, open to the 6 m roof: it rises past where
//                    the clamp held the first, so the slab, not a weak jump, held that one.
//   elevator_up      placed in the service way south of elevator-1's shaft, walked onto the car,
//                    Space: the car rides to the upper stop with the body on it, and Space does
//                    not jump while riding.
//   elevator_out_top walked south off the car into the lift lobby: the HUD names it.
//   elevator_down    walked back onto the car, Shift: it rides down with the body.
//   elevator_out_bottom  walked south off it on the ground floor.
//   ladder_up        placed in the ladder hatch on the ground floor, Space held: the climb to the
//                    top, then walked south off it over the gate onto the lobby floor and west
//                    out of the ladder's reach, into the lobby's middle: the HUD names it.
//   hatch_gate       walked back in line with the ladder and north into the hatch's gap: the
//                    gate across the ladder's top holds a body that is not on the ladder.
//   elevator_call    (the car still at the bottom) placed in the lift lobby, forward held toward
//                    the shaft: the landing door holds the body while the car is away, the car
//                    comes up when called from this landing, and the body walks onto it.
// And three screenshots kept with the run: the upper hall from the stair-top landing (east over
// the stairwell and its guards), the bedroom from inside its door, and the ground hall looking up
// the stair. A person looks at them: the judge only checks they were taken.
//
// scripts/lib/storeys-judge.js judges it all against the tree's data (homestead.ron, the structure
// types, the wall materials, rooms.ron, and the three walk constants it reads out of the code), and
// refuses to boot when the layout no longer has what the route walks (`buildPlan`).
//
// WHAT THE GAME REPORTS (src/engine/storey_probe.rs): the recorder (debug/remote_players_request.
// json) writes, every frame, a `storey` block (the floor stood on, the ceiling, ladder and car
// state, the room the HUD names, every elevator car's height, the landings so far) and in its done
// JSON a `storeys` block (each elevator, its stops and its car). The showcase `face` verb turns the
// camera without moving the body.
//
// SANDBOX DATA (--data DIR): the rig's game reads DIR instead of the tree's data/, while the judge
// still judges against the TREE's layout. This is how a red run is made without another build:
// copy data/ somewhere, take the homestead's floor slabs out (`floors: []` in
// data/homes/homestead.ron), run with --data on it, and the judge must fail. The game prefers a
// source tree's data/ to the folder beside the exe (lib.rs `find_data_dir`), so a sandbox run
// boots from a rig folder in the OS temp folder, outside any tree, with DIR junctioned beside the
// exe; and every run records the data folder the game logged (`data_dir`), which the judge checks
// against the one the run meant. The evidence still goes to .probe-rig/storeys/runs/, the folder
// named <stamp>-sandbox. The manifest records DIR as `sandbox_data` and the verdict says so.
//
// ONE GPU (CLAUDE.md): waits (up to 60 min, --wait-min N; --no-wait to refuse at once) while any
// HumanityOS runs, another rig's or the operator's own game, or another rig holds the machine-wide
// boot lock, and never sets the take-focus env var.
//
// Usage:
//   node scripts/verify-storeys.js [--exe PATH] [--keep-open] [--timeout-min N] [--no-wait] [--wait-min N]
//                                  [--data DIR] [--allow-other-build "<reason>"]
//   node scripts/verify-storeys.js --dry-verdict <manifest.json>   re-judge, nothing booted
// Exit 0 = every check passed. 1 = refused (nothing booted). 2 = a check failed, a panic, or a
// data file served from the copy built into the exe (BUILT-IN DATA).
//
// THE BINARY (BUG-133): the exe must be this tree's build (the freshness gate,
// scripts/check-fresh-exe.js), or another build on purpose with --allow-other-build "<why>",
// recorded in the manifest as other_build. The copy in the rig must be byte-identical to what the
// gate judged, and the game starts through lib/game-launch.js spawnGame.

"use strict";

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
const DXC = require("./lib/dxc-dlls.js");
const { runFreshGate, requireBootCopy, bootRecord, otherBuildNotice } = require("./lib/src-fingerprint.js");
const GL = require("./lib/game-launch.js");
const SJ = require("./lib/storeys-judge.js");
const WR = require("./lib/write-request.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const KEEP_OPEN = args.includes("--keep-open");
const NO_WAIT = args.includes("--no-wait");
// --wait-min N: how long to wait for another HumanityOS before refusing (lib/machine-guard.js
// WAIT_MIN_DEFAULT, the one number every rig uses). Checked before anything else is read.
const WAIT_MIN = Number(opt("--wait-min", String(MG.WAIT_MIN_DEFAULT)));
if (!(WAIT_MIN > 0)) {
  console.error(`verify-storeys: REFUSED - --wait-min must be a number of minutes above 0, got "${opt("--wait-min", "")}". Nothing was booted.`);
  process.exit(1);
}
const DRY = opt("--dry-verdict", null);
const TIMEOUT_MS = Number(opt("--timeout-min", "12")) * 60 * 1000;
const SANDBOX = opt("--data", null) ? path.resolve(opt("--data", null)) : null;

// WHERE THE RIG BOOTS. The game takes the first data folder whose parent holds a Cargo.toml (a
// source tree's live data) over the one beside the exe (lib.rs `find_data_dir`), walking up from
// the exe. A rig folder inside this repo therefore reads the repo's data/ whatever its own `data`
// junction points at, which is right for a normal run and silently wrong for a sandbox: the
// first red run (2026-10-21) junctioned a floorless homestead, the game read the repo's, and the
// run PASSED 75/75. So a sandbox run boots from a folder outside any source tree, and every run
// records the data folder the game says it used (`data_dir`), which the judge checks.
const RIG = SANDBOX ? path.join(require("os").tmpdir(), "hos-verify-storeys-sandbox") : path.join(REPO, ".probe-rig", "storeys");
const RUNS = path.join(REPO, ".probe-rig", "storeys", "runs");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");

/** The walk's speed, m/s (renderer/camera.rs WALK_SPEED_MPS): how long forward is held for a
 *  distance. The route corrects itself from where each hold actually ended. */
const WALK_M_S = 5.0;
/** A straight leg of a route is walked until the body stands this near its end, metres. */
const ARRIVE_M = 0.25;
/** At most this many holds for one straight leg before the rig says it is blocked. */
const MAX_PULSES = 10;
/** Less progress than this in one hold, metres, is a body that cannot go that way. */
const STUCK_M = 0.03;
/** Seconds a placement is left to settle before the leg's first look. */
const SETTLE_MS = 1500;
/** Seconds each held key's recording runs on after the key comes up: the body has stopped. */
const PULSE_TAIL_S = 0.8;
/** Seconds a walked leg is recorded after its last hold, standing: a body that walked off an edge
 *  is still in the air when the walk arrives over its end point (a 3 m fall takes 0.8 s), and its
 *  landing must be in the leg (the third red run's stair leg ended mid-fall, so its 3 m landing
 *  went unseen). */
const AFTER_S = 1.5;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[storeys] ${m}`);
const fmt = (x) => (Number.isFinite(Number(x)) && x !== null ? Number(x).toFixed(2) : "(none)");
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

function printVerdict(run, prefix) {
  const result = SJ.judgeStoreys(run);
  console.log("");
  console.log("-".repeat(100));
  for (const [id, o] of Object.entries(result.observed)) {
    const end = o.end ? `(${o.end.map(fmt).join(", ")})` : "(none)";
    console.log(`${id.padEnd(20)} ${String(o.frames).padStart(5)} frames  ends at ${end}  floor ${fmt(o.floor_end)}  room ${o.room_end || "none"}  landings ${o.landings.length ? o.landings.map(fmt).join(", ") : "none"}`);
  }
  console.log("");
  if (run.sandbox_data) console.log(`SANDBOX DATA: this run's game read ${run.sandbox_data}, not the tree's data/. The verdict is about that data.`);
  const lines = SJ.verdictLines(result);
  lines[lines.length - 1] = prefix + lines[lines.length - 1];
  for (const l of lines) console.log(l);
  console.log("-".repeat(100));
  return result;
}

// ── --dry-verdict: the judge over a saved run, nothing booted ────────────────────────────────────
if (DRY) {
  const p = path.resolve(DRY);
  if (!fs.existsSync(p)) refuse([`--dry-verdict: no such manifest: ${p}`]);
  const run = JSON.parse(fs.readFileSync(p, "utf8"));
  if (run.other_build) console.log(otherBuildNotice(run.other_build));
  const result = printVerdict(run, "DRY VERDICT (nothing was booted): ");
  process.exit(result.ok ? 0 : 2);
}

// ── refusals, before anything is touched ─────────────────────────────────────────────────────────
if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
if (SANDBOX && !fs.existsSync(path.join(SANDBOX, "homes", "homestead.ron"))) refuse([`--data ${SANDBOX}: no homes/homestead.ron there; point it at a copy of the whole data/ folder`]);
let TREE;
let PLAN;
try {
  TREE = SJ.readTreeData(REPO);
  const built = SJ.buildPlan(TREE);
  if (built.problems.length) {
    refuse([
      "verify-storeys: REFUSED - the homestead no longer has what the route walks (scripts/lib/storeys-judge.js ROUTE):",
      ...built.problems.map((p) => `  - ${p}`),
      "Move the route's points to the new layout (docs/design/homestead-upper-floor.md), then run again. Nothing was booted.",
    ]);
  }
  PLAN = built.plan;
} catch (e) {
  refuse([`verify-storeys: REFUSED - the tree's data could not be read: ${e.message}`]);
}
const FRESH = runFreshGate(EXE, args, { cwd: REPO });
if (FRESH.status !== 0) refuse(["verify-storeys: REFUSED - see the freshness failure above. Nothing was booted."]);
const OTHER_BUILD = FRESH.other_build;

// One GPU: wait for another rig (or the operator's game) to finish, never boot beside it.
{
  const running = MG.listInstances();
  if (running.length && NO_WAIT) {
    refuse([
      `REFUSED: HumanityOS is already running (pid ${running.map((p) => p.pid).join(", ")}), and --no-wait.`,
      "One GPU, one instance (CLAUDE.md). Close it (taskkill //PID <pid>, then //F if it is still there 15 s later), or wait for the rig that started it.",
    ]);
  }
  // Ends by taking the machine-wide boot lock (lib/boot-lock.js); a wait that gives up is the one
  // refusal every rig gives (exit 1, "the machine never went quiet in N min; nothing was booted").
  const w = MG.waitForFree({ own: { gameOnly: true }, label: "verify-storeys", rig: "verify-storeys", timeoutMs: WAIT_MIN * 60 * 1000, log });
  if (!w.free) {
    MG.refuseBusy(w, {
      rig: "verify-storeys",
      hint: ["What to do: wait for it (or close it: taskkill //PID <pid>, then //F if it is still there 15 s later), then run again; --wait-min N waits longer."],
    });
  }
}

// ── the rig ──────────────────────────────────────────────────────────────────────────────────────
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
  const rigExe = path.join(RIG, "HumanityOS.exe").replace(/'/g, "''");
  try {
    execSync(
      `powershell -NoProfile -Command "Get-Process HumanityOS -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${rigExe}' } | Stop-Process -Force"`,
      { stdio: "ignore" }
    );
  } catch {}
}
function setupRig() {
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  // portable.txt keeps identity, config and saves inside the rig and lets the dev autopilot run;
  // no_focus.txt is the belt to the launch-focus inversion's braces.
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-storeys rig\n");
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), SANDBOX || path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  killRigProcesses();
  // Every run starts from the home the data describes: a save an earlier run left (a character's
  // own home is kept in its save, engine/own_home.rs) would otherwise be the home walked, so a
  // red run on sandbox data could leave its floorless homestead for the next green run.
  for (const d of ["saves", "backups"]) fs.rmSync(path.join(RIG, d), { recursive: true, force: true });
  fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  requireBootCopy(path.join(RIG, "HumanityOS.exe"), FRESH, "verify-storeys");
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.png$/.test(f) || /_done\.json$/.test(f) || WR.isRequestLeftover(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}

/** A request file, written whole through the one writer every rig shares (a side name renamed
 *  into place: the game deletes what it reads, and a plain write can be read empty and lost). */
function req(name, body) {
  WR.writeRequest(path.join(DEBUG, name), body);
}
function clearDone(name) {
  const p = path.join(DEBUG, name);
  if (fs.existsSync(p)) fs.unlinkSync(p);
}
async function waitFile(name, timeoutMs, pollMs = 150) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(p)) {
      try {
        return JSON.parse(fs.readFileSync(p, "utf8"));
      } catch {}
    }
    await sleep(pollMs);
  }
  return null;
}
async function waitBoot(timeoutMs, game) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(LOG)) {
      const txt = fs.readFileSync(LOG, "utf8");
      if (/PANIC/.test(txt)) throw new Error("PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return;
    }
    if (game.exited()) throw new Error(`before it finished booting, ${game.describe()}`);
    await sleep(1000);
  }
  throw new Error("the exe did not finish booting in time");
}
/** A showcase request, waited on until the game has taken it (one file: a second request written
 *  before the first is read would replace it). */
async function showcase(body) {
  const p = path.join(DEBUG, "showcase_request.json");
  const t0 = Date.now();
  while (fs.existsSync(p) && Date.now() - t0 < 10000) await sleep(40);
  req("showcase_request.json", body);
  while (fs.existsSync(p)) {
    if (Date.now() - t0 > 15000) throw new Error(`the game never took the showcase request ${JSON.stringify(body)}`);
    await sleep(40);
  }
}
/** What the judge reads of a recorder answer. */
function trim(done) {
  const hs = done.home_session || {};
  return {
    falls: done.falls || null,
    vitals: hs.vitals || null,
    home_box: hs.home && hs.home.box ? hs.home.box : null,
    storeys: done.storeys || null,
    frames: (done.frames || []).map((f) => ({ t: f.t, cam: f.cam, health: f.health, storey: f.storey })),
    camera_end: done.camera_end || null,
    notices: done.notices || [],
  };
}
/** Record `seconds` of frames while `during()` runs. The action starts only once the game has
 *  taken the recording request (the file is gone), so the recording always holds the action's
 *  start: with a fixed 0.3 s wait instead, a slow frame (a build running beside the rig) started
 *  a hold before the recording and ended the recording before the hold, and the body walked on
 *  unrecorded between two recordings (the third red run, 2026-10-21). */
async function record(seconds, during, what) {
  clearDone("remote_players_done.json");
  const p = path.join(DEBUG, "remote_players_request.json");
  req("remote_players_request.json", { seconds });
  const t0 = Date.now();
  while (fs.existsSync(p)) {
    if (Date.now() - t0 > 15000) throw new Error(`${what}: the game never took the recording request`);
    await sleep(20);
  }
  await sleep(100);
  if (during) await during();
  const done = await waitFile("remote_players_done.json", seconds * 1000 + 30000);
  if (!done || done.ok !== true) throw new Error(`${what}: the recorder never answered (${JSON.stringify(done)})`);
  return trim(done);
}
const snapshot = (what) => record(0, null, what);
const panicCount = () => (fs.existsSync(LOG) ? (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length : 0);

// ── the run ──────────────────────────────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RUNS, SANDBOX ? `${stamp}-sandbox` : stamp);

/** The data folder the game says it reads (lib.rs `find_data_dir`'s log line), and that folder
 *  resolved through any junction, next to the one this run meant: the tree's data/, or --data's.
 *  Waits up to `waitMs` for the line: with the DXC compiler the cloud bake that ends `waitBoot`
 *  can finish BEFORE the data folder is found (the second red run, 2026-10-21, read run.log too
 *  early and recorded no folder at all). */
async function dataDirUsed(waitMs = 60000) {
  const t0 = Date.now();
  let m = null;
  for (;;) {
    const text = fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "";
    m = text.match(/Data directory \(([^)]*)\): (.+?)\s*$/m);
    if (m || Date.now() - t0 > waitMs) break;
    await sleep(500);
  }
  const real = (p) => {
    try {
      return fs.realpathSync(p);
    } catch {
      return p;
    }
  };
  return {
    logged: m ? m[2] : null,
    how: m ? m[1] : null,
    real: m ? real(m[2]) : null,
    expected: real(SANDBOX || path.join(REPO, "data")),
  };
}

async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "verify-storeys",
    stamp,
    exe: EXE,
    binary: bootRecord(FRESH, path.join(RIG, "HumanityOS.exe")),
    ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
    ...(SANDBOX ? { sandbox_data: SANDBOX } : {}),
    plan: PLAN,
    problems: [],
    data_dir: null,
    origin: null,
    entry: null,
    legs: [],
    shots: [],
    panics: 0,
    builtin_data: [],
    error: null,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));

  log(`launching ${path.basename(EXE)} in ${path.relative(REPO, RIG)}${SANDBOX ? ` on SANDBOX DATA ${SANDBOX}` : ""}`);
  const game = GL.spawnGame(path.join(RIG, "HumanityOS.exe"), [], {
    fresh: FRESH,
    rigName: "verify-storeys",
    log,
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
  });
  const pid = game.child.pid;
  game.child.unref();
  let killed = false;
  const kill = () => {
    if (killed) return;
    killed = true;
    game.expectExit();
    try {
      execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });
    } catch {}
    killRigProcesses();
  };
  process.on("exit", () => {
    if (!KEEP_OPEN) kill();
  });
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min: stopping the game`);
    manifest.error = "timeout";
    save();
    kill();
    process.exit(2);
  }, TIMEOUT_MS);

  try {
    log("waiting for boot...");
    await waitBoot(240000, game);
    manifest.data_dir = await dataDirUsed();
    save();
    if (!SJ.sameDir(manifest.data_dir.real, manifest.data_dir.expected)) {
      log(`WARNING: the game reads ${manifest.data_dir.logged} (${manifest.data_dir.real}), not ${manifest.data_dir.expected}: this run is not about the data it meant (the judge fails data_dir)`);
    } else {
      log(`the game reads ${manifest.data_dir.real} (${manifest.data_dir.how})`);
    }
    log("entering world (autopilot)...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", {}); // no server_url: the pinned one (a dead loopback port) stands (BUG-160)
    const ap = await waitFile("autopilot_done.json", 240000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    // The home's walls, rooms and floors are built a few frames after world entry.
    await sleep(6000);

    const s0 = await snapshot("the first look");
    manifest.entry = { home_box: s0.home_box, storeys: s0.storeys, frames: s0.frames };
    const o = s0.home_box && Array.isArray(s0.home_box.min) ? s0.home_box.min.map(Number) : null;
    if (!o) throw new Error("the first look has no home box: the world did not load the homestead aboard");
    manifest.origin = o;
    save();
    const UP = PLAN.upper_m;
    const EYE = PLAN.eye_m;
    const world = (p, floor) => [p[0] + o[0], floor + EYE + o[1], p[1] + o[2]];
    log(`home at (${o.join(", ")}); the upper floor at ${UP} m`);
    // Everything here is judged by what it does, not by what it costs a body, but a fall into a
    // shaft or a hatch should show as harm too: Realistic.
    await showcase({ illness: "realistic" });

    // Where the body is now, in home metres, from a recording's last frame.
    let here = null;
    const track = (rec) => {
      const f = rec.frames[rec.frames.length - 1];
      if (f && Array.isArray(f.cam)) here = [f.cam[0] - o[0], f.cam[1] - o[1], f.cam[2] - o[2]];
      return rec;
    };
    /** Place the body standing on `floor` at the home point p, facing `yaw`, on foot. A teleport:
     *  used to start a leg, never inside a walked route. */
    const place = async (p, floor, yaw, pitch = 0) => {
      const w = world(p, floor);
      await showcase({ walk: "1", cam: `${w.map((v) => v.toFixed(3)).join(",")},${yaw.toFixed(4)},${pitch.toFixed(4)}` });
      await sleep(SETTLE_MS);
    };
    const face = async (yaw, pitch = 0) => {
      await showcase({ face: `${yaw.toFixed(4)},${pitch.toFixed(4)}` });
      await sleep(120);
    };
    const holdKeys = (keys, s) => showcase({ hold: keys, hold_s: s.toFixed(3) });
    /** Walk a route of straight legs from where the body is, each recorded: turned toward the
     *  leg's end, forward held for the distance at the walk's speed, then again from where that
     *  ended, until within ARRIVE_M; then AFTER_S more, standing, so a fall the walk ended in
     *  lands inside the record. Every recording goes into `segs`. */
    const walkRoute = async (segs, pts, what) => {
      await walkPoints(segs, pts, what);
      segs.push({ what: "after", rec: track(await record(AFTER_S, null, `${what} (standing after)`)) });
    };
    const walkPoints = async (segs, pts, what) => {
      for (const target of pts) {
        let pulses = 0;
        for (;;) {
          const d = Math.hypot(target[0] - here[0], target[1] - here[2]);
          if (d <= ARRIVE_M) break;
          if (pulses++ >= MAX_PULSES) throw new Error(`${what}: still ${d.toFixed(2)} m from (${target.join(", ")}) after ${MAX_PULSES} holds, at (${here.map(fmt).join(", ")})`);
          await face(SJ.yawTo([here[0], here[2]], target));
          const s = Math.max(0.04, (d - 0.05) / WALK_M_S);
          const from = here.slice();
          const rec = track(await record(s + PULSE_TAIL_S, () => holdKeys("forward", s), `${what} toward (${target.join(", ")})`));
          segs.push({ what: `walk to (${target.join(", ")})`, hold_s: s, rec });
          if (Math.hypot(here[0] - from[0], here[2] - from[2]) < STUCK_M) {
            throw new Error(`${what}: blocked at (${here.map(fmt).join(", ")}) walking toward (${target.join(", ")}): forward held ${s.toFixed(2)} s moved the body ${Math.hypot(here[0] - from[0], here[2] - from[2]).toFixed(3)} m`);
          }
        }
      }
    };
    /** One leg: run `body(segs)`, keep what it recorded even when it throws, and go on. */
    const leg = async (id, kind, body) => {
      const segs = [];
      const entry = { id, kind, segments: segs };
      manifest.legs.push(entry);
      try {
        await body(segs, entry);
      } catch (e) {
        entry.error = e.message;
        log(`${id}: STOPPED: ${e.message}`);
      }
      save();
      const f = segs.length && segs[segs.length - 1].rec.frames.slice(-1)[0];
      const st = f && f.storey;
      log(`${id}: ${segs.length} recording(s); ends at (${here ? here.map(fmt).join(", ") : "?"}), floor ${st ? fmt(st.floor - o[1]) : "?"}, room ${st && st.room ? st.room.id : "none"}`);
    };
    /** A leg that starts with a placement and a first look. */
    const placedLeg = (id, kind, p, floor, yaw, body) =>
      leg(id, kind, async (segs, entry) => {
        await place(p, floor, yaw);
        segs.push({ what: "before", rec: track(await snapshot(`${id} (before)`)) });
        await body(segs, entry);
      });
    /** Hold keys for `s` seconds while recording `s + after` seconds. */
    const heldLeg = async (segs, keys, s, after, what) => {
      segs.push({ what, hold_s: s, rec: track(await record(s + after, () => holdKeys(keys, s), what)) });
    };
    let firstShot = true;
    /** A screenshot at the shot's pose: turned to it where the body stands when it is there
     *  already (what the walk arrived at), else placed there first. */
    const shoot = async (spec) => {
      const floor = spec.storey === "upper" ? UP : 0;
      const near = here && Math.hypot(here[0] - spec.at[0], here[2] - spec.at[1]) <= 0.6 && Math.abs(here[1] - floor - EYE) < 0.1;
      if (near) await face(spec.yaw, spec.pitch);
      else await place(spec.at, floor, spec.yaw, spec.pitch);
      await sleep(1200);
      const shot = { id: spec.id, note: spec.note, placed: !near, ok: false };
      // The first capture after a boot is a different state from every later one (the rigs have
      // been bitten by it): taken twice, the first thrown away.
      for (let i = 0; i < (firstShot ? 2 : 1); i++) {
        clearDone("screenshot_done.json");
        req("screenshot_request.json", { width: 1600, height: 900 });
        const d = await waitFile("screenshot_done.json", 30000);
        if (!d || d.ok !== true) {
          shot.error = d ? d.error : "no screenshot_done.json";
          break;
        }
        const src = path.join(RIG, d.path);
        const file = `${spec.id}.png`;
        if (fs.existsSync(src)) {
          fs.copyFileSync(src, path.join(OUT, file));
          shot.file = file;
          shot.bytes = fs.statSync(path.join(OUT, file)).size;
          shot.ok = true;
        }
        if (firstShot && i === 0) await sleep(1200);
      }
      firstShot = false;
      shot.where = here;
      manifest.shots.push(shot);
      save();
      log(`shot ${spec.id}: ${shot.ok ? `${shot.file} (${shot.bytes} bytes)` : `FAILED: ${shot.error}`}`);
    };
    const shotSpec = (id) => PLAN.shots.find((s) => s.id === id);

    // 1. In at the front door, down the hall and up the stair.
    await placedLeg("stairs_up", "walk", PLAN.front_door, 0, SJ.yawTo(PLAN.stairs_route[0], PLAN.stairs_route[1]), async (segs) => {
      await walkRoute(segs, PLAN.stairs_route.slice(1), "the front door up the stair");
    });
    await shoot(shotSpec("upper_hall"));
    // 2. On into the bedroom.
    await leg("bedroom", "walk", async (segs) => {
      segs.push({ what: "before", rec: track(await snapshot("bedroom (before)")) });
      await walkRoute(segs, PLAN.bedroom_route.slice(1), "the stair top to the bedroom");
    });
    await shoot(shotSpec("bedroom"));
    // 3. The wall upstairs, then the guard over the stairwell.
    await placedLeg("wall_upstairs", "held", PLAN.wall.up_from, UP, Math.PI / 2, (segs) => heldLeg(segs, "forward", 1.6, 1.0, "forward into the wall upstairs"));
    await placedLeg("guard", "held", PLAN.guard.from, UP, 0, (segs) => heldLeg(segs, "forward", 1.2, 1.0, "forward into the guard"));
    // 4. Under the same wall's line downstairs.
    await placedLeg("wall_downstairs", "walk", PLAN.wall.down_from, 0, Math.PI / 2, (segs) => walkRoute(segs, [PLAN.wall.down_to], "across the wall's line downstairs"));
    // 5. The jumps: under the slab, then in the open.
    await placedLeg("jump_under_slab", "jump", PLAN.jump_under.at, 0, Math.PI / 2, (segs) => heldLeg(segs, "jump", 0.1, 2.4, "a jump under the slab"));
    await placedLeg("jump_open", "jump", PLAN.jump_open.at, 0, Math.PI / 2, (segs) => heldLeg(segs, "jump", 0.1, 2.4, "a jump in the open"));
    await shoot(shotSpec("ground_hall_up_stairs"));
    // 6. The elevator: up, out, back on, down, out.
    const E = PLAN.elevator;
    const car = [E.pos[0], E.pos[2]];
    await placedLeg("elevator_up", "elevator", PLAN.lift_from, 0, SJ.yawTo(PLAN.lift_from, car), async (segs) => {
      await walkRoute(segs, [car], "the service way onto the car");
      await heldLeg(segs, "jump", 0.15, 3.5, "Space on the car");
    });
    await leg("elevator_out_top", "walk", async (segs) => {
      segs.push({ what: "before", rec: track(await snapshot("elevator_out_top (before)")) });
      await walkRoute(segs, [PLAN.lift_out], "off the car into the lift lobby");
    });
    await leg("elevator_down", "elevator", async (segs) => {
      segs.push({ what: "before", rec: track(await snapshot("elevator_down (before)")) });
      await walkRoute(segs, [car], "back onto the car");
      await heldLeg(segs, "sprint", 0.15, 3.5, "Shift on the car");
    });
    await leg("elevator_out_bottom", "walk", async (segs) => {
      segs.push({ what: "before", rec: track(await snapshot("elevator_out_bottom (before)")) });
      await walkRoute(segs, [PLAN.lift_out], "off the car on the ground floor");
    });
    // 7. The ladder up through its hatch, and its gate.
    await placedLeg("ladder_up", "ladder", PLAN.ladder_from, 0, Math.PI, async (segs) => {
      await heldLeg(segs, "jump", 1.8, 1.0, "Space at the ladder: the climb");
      await walkRoute(segs, PLAN.ladder_route.slice(1), "off the ladder's top onto the lobby floor");
    });
    // From in line with the ladder, north into the hatch's gap: the gate across its top.
    await leg("hatch_gate", "held", async (segs) => {
      segs.push({ what: "before", rec: track(await snapshot("hatch_gate (before)")) });
      await walkRoute(segs, [PLAN.hatch_from], "the lobby to in line with the ladder");
      const gate = PLAN.ladder.gate;
      await face(SJ.yawTo([here[0], here[2]], [(gate.a[0] + gate.b[0]) / 2, (gate.a[1] + gate.b[1]) / 2]));
      await heldLeg(segs, "forward", 1.0, 1.0, "forward into the hatch's gap");
    });
    // 8. Called from the upper landing with the car at the bottom (one recording: the judge times
    // the wait at the door on its frame clock).
    await placedLeg("elevator_call", "elevator", PLAN.lift_call_from, UP, 0, (segs) => heldLeg(segs, "forward", 4.5, 1.5, "forward to the landing door, the car away"));
  } catch (e) {
    log(`ERROR: ${e.message}`);
    manifest.error = e.message;
  }
  clearTimeout(watchdog);

  manifest.panics = panicCount();
  manifest.builtin_data = GL.builtinDataLines(fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "");
  save();
  try {
    fs.copyFileSync(LOG, path.join(OUT, "run.log"));
  } catch {}
  if (KEEP_OPEN) log(`left running (--keep-open): taskkill //PID ${pid} to close it`);
  else kill();

  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  if (manifest.builtin_data.length) log(GL.builtinDataRefusal(manifest.builtin_data));
  const result = printVerdict(manifest, manifest.error ? `(the run stopped early: ${manifest.error}) ` : "RESULT: ");
  log(`evidence: ${path.relative(REPO, OUT)} (manifest.json, the screenshots, run.log)`);
  process.exit(result.ok && !manifest.error ? 0 : 2);
}

main();
