#!/usr/bin/env node
// verify-falls: proves in the RUNNING game that falls hurt the player the way the code and
// data/medical/falls.ron say (2026-10-20, `just verify-falls`).
//
// Falls shipped in v0.1527.0 (src/systems/falls.rs) with unit tests over the walk, the watch and
// the body pass, and nothing that looked at the game: `player_landed` logged nothing and no probe
// said what health or wounds the player had, so a landing that never reached the body would have
// left everything green. This boots the release binary in its own portable rig (.probe-rig/falls,
// background, never focused, silent, no server), enters the world through the dev autopilot (the
// rig's own home, aboard the homestead), and walks the player up the homestead's tallest
// staircase and off its top, the way a player does, by holding the forward key (the showcase
// `hold` verb, the controller's own action path): once in Forgiving, once in Realistic. Then it
// climbs the homestead's 3 m ladder by holding Space, lets go at the top (the body must hold on),
// and steps back off it. Last it PLACES the player at the stairs' height beside them and lets them
// fall: a teleport's drop, which must hurt nobody. And the first look after world entry must show
// no landing reached the body getting there. scripts/lib/falls-judge.js judges it all against
// falls.ron, read by its own code.
//
// WHY THE STAIRS AND THE LADDER. The homestead has no loft or deck at the top of its ladder or its
// staircase (nothing in data/homes/homestead.ron stands at 3 m but the top step itself), so the
// raised places a player can reach aboard are the top step of stairs-1 (3 m) and the top of
// ladder-1 (the eye hangs 0.2 m over its 3 m top rung). Walking on past the top step is a 3 m
// drop, 7.67 m/s at the walk's 9.81 m/s^2; stepping off the ladder's top a 3.2 m one, 7.92 m/s. A
// placement onto the top step does not work (the floor sampler's step-up cap keeps a body 3 m
// above its last floor from standing there), so the walk starts on the floor 1 m before the first
// step and goes up the stairs too.
//
// THE FRAME RATE MOVES THE DROP. A tread is 0.26 m and the walk 5 m/s, so at frames over about
// 50 ms (a machine busy with a build) the walk can skip the top tread and leave from the one below
// it: the first real run's Realistic walk fell 2.79 m, not 3.00 (7.39 m/s, a sprain and 8.17
// health, not 9.68). That is the game, not the rig, so the judge works out what falls.ron says
// from the drop the camera actually made, after checking the game's landing speed agrees with it.
//
// WHAT THE GAME REPORTS (src/engine/body_probe.rs): the recorder (debug/remote_players_request.json)
// writes health per frame, and in its done JSON the home session's vitals (health, wounds by id
// and name) and a `falls` block (landings handed to the body, the last one's speed, the mode,
// gravity, flight, aboard). The showcase `illness` verb sets Settings > Gameplay > Illness and
// injury as its radio does. The run.log's "[Falls]" lines (src/systems/falls.rs player_landed)
// are kept in the manifest.
//
// RED FIRST (2026-10-20). This rig was run against a build with lib.rs's hand-off commented out
// (the `take_landing` -> `falls::player_landed` block after `death_pack::frame`), the state in
// which every unit test of falls stays green. In the running game the camera fell 3.00 m off the
// stairs and 3.20 m off the ladder and the body never moved; the rig said (exit 2):
//   FAIL  forgiving_walk.landing_handed    0 landings reached the body for a 3.00 m drop (expected
//         exactly 1). The walk's landing never got to player_landed: the lib.rs hand-off
//         (take_landing -> falls::player_landed), FallWatch treating the walk as a placement, or
//         the walk reporting no impact
//   FAIL  realistic_walk.landing_handed    (the same)
//   FAIL  realistic_walk.wound             read at 7.67 m/s falls.ron gives Sprained Ankle
//                                          (sprained_ankle); the body has no wound
//   FAIL  realistic_walk.health            health 100.00 -> 100.00, took 0.00; falls.ron at 7.67
//                                          m/s takes 9.68 (tolerance 0.5)
//   FAIL  realistic_ladder.landing_handed  0 landings reached the body for a 3.20 m drop ...
//   FAIL  realistic_ladder.wound           read at 7.92 m/s falls.ron gives Broken Ankle
//                                          (broken_ankle); the body has no wound
//   FAIL  realistic_ladder.health          health 100.00 -> 100.00, took 0.00; falls.ron at 7.92
//                                          m/s takes 11.51 (tolerance 0.5)
//   RESULT: FAIL  25/32 passed
// With the hand-off back the same rig passed 40/40. Both runs are kept, trimmed, as
// tests/fixtures/falls/red-hand-off-removed.json and green.json, and scripts/tests/
// falls-judge.test.js holds the judge to them.
//
// ONE GPU (CLAUDE.md): refuses while any HumanityOS.exe runs, the operator's own game included,
// and never sets the take-focus env var.
//
// Usage:
//   node scripts/verify-falls.js [--exe PATH] [--keep-open] [--timeout-min N]
//                                [--allow-other-build "<reason>"]
//   node scripts/verify-falls.js --dry-verdict <manifest.json>   re-judge, nothing booted
// Exit 0 = every check passed. 1 = refused (nothing booted). 2 = a check failed, a panic, or a
// data file served from the copy built into the exe (BUILT-IN DATA).
//
// THE BINARY (BUG-133): the exe must be this tree's build (the freshness gate,
// scripts/check-fresh-exe.js), or another build on purpose with --allow-other-build "<why>",
// recorded in the manifest as other_build. The copy in the rig must be byte-identical to what the
// gate judged, and the game starts through lib/game-launch.js spawnGame (the copy checked again
// right before it spawns, HUMANITY_NO_HANDOFF=1, the sandbox's settings pinned).

"use strict";

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
const DXC = require("./lib/dxc-dlls.js");
const { runFreshGate, requireBootCopy, bootRecord, otherBuildNotice } = require("./lib/src-fingerprint.js");
const GL = require("./lib/game-launch.js");
const FJ = require("./lib/falls-judge.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const KEEP_OPEN = args.includes("--keep-open");
const DRY = opt("--dry-verdict", null);
const TIMEOUT_MS = Number(opt("--timeout-min", "8")) * 60 * 1000;

const RIG = path.join(REPO, ".probe-rig", "falls");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");

/** How long forward is held, seconds: the 4.6 m to the top step's edge at the 5 m/s walk is about
 *  a second, and the 3 m fall another 0.8 s, so the body is still walking when it lands. */
const HOLD_S = 2.0;
/** How long each walk is recorded, seconds: the hold, the fall and a few seconds after the
 *  landing for the body pass to take it. */
const WALK_RECORD_S = 6;
/** How long the placement's drop is recorded, seconds. */
const DROP_RECORD_S = 4;
/** The ladder leg: Space held this long climbs the 3.2 m to the top at the climb's 3 m/s and
 *  waits there; then no key for HANG_S (the body must hold on); then back held for STEP_BACK_S,
 *  which takes the body out of the ladder's reach (1.2 m) and off it. */
const LADDER_CLIMB_S = 1.8;
const HANG_S = 1.5;
const STEP_BACK_S = 1.2;
const LADDER_RECORD_S = LADDER_CLIMB_S + HANG_S + STEP_BACK_S + 3;
/** Eye height above the feet, metres (src/renderer/camera.rs, `eye_height`). */
const EYE_M = 1.7;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[falls] ${m}`);
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

function printVerdict(run, prefix) {
  const result = FJ.judgeFalls(run);
  console.log("");
  console.log("-".repeat(96));
  for (const [id, o] of Object.entries(result.observed)) {
    const wounds = (o.wounds_after || []).map((w) => `${w.name} (${w.id})`).join(", ") || "none";
    console.log(
      `${id.padEnd(15)} mode ${String(o.mode).padEnd(9)} health ${fmt(o.health_before)} -> ${fmt(o.health_after)}  ` +
        `drop ${fmt(o.drop_m)} m  landing ${o.landing_m_s === null ? "none handed to the body" : `${fmt(o.landing_m_s)} m/s`}  wounds ${wounds}`
    );
  }
  console.log("");
  const lines = FJ.verdictLines(result);
  lines[lines.length - 1] = prefix + lines[lines.length - 1];
  for (const l of lines) console.log(l);
  console.log("-".repeat(96));
  return result;
}
const fmt = (x) => (Number.isFinite(Number(x)) && x !== null ? Number(x).toFixed(2) : "(none)");

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
const running = MG.listInstances();
if (running.length) {
  refuse([
    `REFUSED: HumanityOS is already running (pid ${running.map((p) => p.pid).join(", ")}).`,
    "One GPU, one instance (CLAUDE.md). Close it (taskkill //PID <pid>, then //F if it is still",
    "there 15 s later), or wait for the rig that started it, then run again.",
  ]);
}
if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
const FRESH = runFreshGate(EXE, args, { cwd: REPO });
if (FRESH.status !== 0) refuse(["verify-falls: REFUSED - see the freshness failure above. Nothing was booted."]);
const OTHER_BUILD = FRESH.other_build;
let DATA;
try {
  DATA = FJ.readTreeData(REPO);
} catch (e) {
  refuse([`verify-falls: REFUSED - the tree's data could not be read: ${e.message}`]);
}
if (!DATA.stairs) {
  refuse([
    "verify-falls: REFUSED - data/homes/homestead.ron has no Steps-shaped staircase to walk off.",
    "The walk legs need a raised floor of a known height aboard; pick another and update this rig.",
  ]);
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
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-falls rig\n");
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  killRigProcesses();
  fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  requireBootCopy(path.join(RIG, "HumanityOS.exe"), FRESH, "verify-falls");
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.png$/.test(f) || /_done\.json$/.test(f) || /_request\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}

function req(name, body) {
  fs.writeFileSync(path.join(DEBUG, name), JSON.stringify(body));
}
function clearDone(name) {
  const p = path.join(DEBUG, name);
  if (fs.existsSync(p)) fs.unlinkSync(p);
}
async function waitFile(name, timeoutMs, pollMs = 200) {
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
  while (fs.existsSync(p) && Date.now() - t0 < 10000) await sleep(50);
  req("showcase_request.json", body);
  while (fs.existsSync(p)) {
    if (Date.now() - t0 > 15000) throw new Error(`the game never took the showcase request ${JSON.stringify(body)}`);
    await sleep(50);
  }
}
/** What the judge reads of a recorder answer (the rest is ship-homes evidence it does not need). */
function trim(done) {
  const hs = done.home_session || {};
  return {
    falls: done.falls || null,
    vitals: hs.vitals || null,
    home_box: hs.home && hs.home.box ? hs.home.box : null,
    frames: (done.frames || []).map((f) => ({ t: f.t, cam: f.cam, health: f.health })),
    camera_end: done.camera_end || null,
    notices: done.notices || [],
  };
}
async function startRecording(seconds) {
  clearDone("remote_players_done.json");
  req("remote_players_request.json", { seconds });
}
async function finishRecording(seconds, what) {
  const done = await waitFile("remote_players_done.json", seconds * 1000 + 30000);
  if (!done || done.ok !== true) throw new Error(`${what}: the recorder never answered (${JSON.stringify(done)})`);
  return trim(done);
}
async function snapshot(what) {
  await startRecording(0);
  return finishRecording(0, what);
}
const panicCount = () => (fs.existsSync(LOG) ? (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length : 0);
const fallLines = () =>
  fs.existsSync(LOG)
    ? fs
        .readFileSync(LOG, "utf8")
        .split(/\r?\n/)
        .filter((l) => l.includes("[Falls]"))
        .map((l) => l.trim())
    : [];

// ── the run ──────────────────────────────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RIG, "runs", stamp);

async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "verify-falls",
    stamp,
    exe: EXE,
    binary: bootRecord(FRESH, path.join(RIG, "HumanityOS.exe")),
    ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
    data: DATA,
    origin: null,
    legs: [],
    falls_log: [],
    panics: 0,
    builtin_data: [],
    error: null,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));

  log(`launching ${path.basename(EXE)} in ${path.relative(REPO, RIG)}`);
  const game = GL.spawnGame(path.join(RIG, "HumanityOS.exe"), [], {
    fresh: FRESH,
    rigName: "verify-falls",
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
    log("entering world (autopilot)...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", {}); // no server_url: the pinned one (a dead loopback port) stands (BUG-160)
    const ap = await waitFile("autopilot_done.json", 240000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    // The home's walls, rooms and floors are built a few frames after world entry.
    await sleep(6000);

    // Where the home stands in the camera's frame: its box's corner (the plot's origin; on the
    // default plot p1, the ship's origin). The stairs are in home metres.
    const s0 = await snapshot("the first look");
    // Also the world-entry check: no landing may have reached the body getting here.
    manifest.entry = s0;
    const o = s0.home_box && Array.isArray(s0.home_box.min) ? s0.home_box.min.map(Number) : [0, 0, 0];
    manifest.origin = o;
    const at = (p) => [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
    const st = DATA.stairs;
    const foot = at(st.foot);
    const pose = (p) => `${p.map((v) => v.toFixed(3)).join(",")},${st.yaw.toFixed(4)},0`;
    log(`home at (${o.join(", ")}); ${st.id} ${st.size[1]} m tall, from (${foot.map((v) => v.toFixed(2)).join(", ")}) facing up it`);
    save();

    // Each walk: the mode, stand at the foot, a look before, then forward held while recording.
    const walkLeg = async (id, mode) => {
      await showcase({ illness: mode });
      await showcase({ walk: "1", cam: pose(foot) });
      await sleep(1500); // the placement settles; the watch resets on the first frame standing
      const before = await snapshot(`${id} (before)`);
      await startRecording(WALK_RECORD_S);
      await sleep(300);
      await showcase({ hold: "forward", hold_s: String(HOLD_S) });
      const after = await finishRecording(WALK_RECORD_S, `${id} (the walk)`);
      manifest.legs.push({ id, mode, kind: "walk", start: foot, before, after });
      save();
      log(`${id}: health ${fmt(before.vitals && before.vitals.health)} -> ${fmt(after.vitals && after.vitals.health)}, landings ${before.falls && before.falls.landings} -> ${after.falls && after.falls.landings}`);
    };
    await walkLeg("forgiving_walk", "forgiving");
    await walkLeg("realistic_walk", "realistic");

    // The ladder: at its foot facing it, Space held climbs to the top; let go and hang there;
    // then back held steps off it.
    if (DATA.ladder) {
      const id = "realistic_ladder";
      const L = DATA.ladder;
      const stand = at(L.stand);
      await showcase({ illness: "realistic" });
      await showcase({ walk: "1", cam: `${stand.map((v) => v.toFixed(3)).join(",")},${L.yaw.toFixed(4)},0` });
      await sleep(1500);
      const before = await snapshot(`${id} (before)`);
      await startRecording(LADDER_RECORD_S);
      await sleep(300);
      await showcase({ hold: "jump", hold_s: String(LADDER_CLIMB_S) });
      await sleep((LADDER_CLIMB_S + HANG_S) * 1000);
      await showcase({ hold: "back", hold_s: String(STEP_BACK_S) });
      const after = await finishRecording(LADDER_RECORD_S, `${id} (the climb and the step off)`);
      manifest.legs.push({ id, mode: "realistic", kind: "ladder", start: stand, hang_s: HANG_S, before, after });
      save();
      log(`${id}: health ${fmt(before.vitals && before.vitals.health)} -> ${fmt(after.vitals && after.vitals.health)}, landings ${before.falls && before.falls.landings} -> ${after.falls && after.falls.landings}`);
    } else {
      log("no ladder in data/homes/homestead.ron: the ladder leg cannot run (the judge says so)");
    }

    // The placement: at the foot, a look before, then PLACED as high as the top step's eye and
    // left to fall the same 3 m.
    {
      const id = "placed_drop";
      await showcase({ illness: "realistic" });
      await showcase({ walk: "1", cam: pose(foot) });
      await sleep(1500);
      const before = await snapshot(`${id} (before)`);
      await startRecording(DROP_RECORD_S);
      await sleep(300);
      const high = at([st.placed_eye[0], st.placed_eye[1], st.placed_eye[2]]);
      await showcase({ walk: "1", cam: pose(high) });
      const after = await finishRecording(DROP_RECORD_S, `${id} (the drop)`);
      manifest.legs.push({ id, mode: "realistic", kind: "placement", start: high, before, after });
      save();
      log(`${id}: health ${fmt(before.vitals && before.vitals.health)} -> ${fmt(after.vitals && after.vitals.health)}, landings ${before.falls && before.falls.landings} -> ${after.falls && after.falls.landings}`);
    }
  } catch (e) {
    log(`ERROR: ${e.message}`);
    manifest.error = e.message;
  }
  clearTimeout(watchdog);

  manifest.panics = panicCount();
  manifest.builtin_data = GL.builtinDataLines(fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "");
  manifest.falls_log = fallLines();
  save();
  try {
    fs.copyFileSync(LOG, path.join(OUT, "run.log"));
  } catch {}
  if (KEEP_OPEN) log(`left running (--keep-open): taskkill //PID ${pid} to close it`);
  else kill();

  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  if (manifest.builtin_data.length) log(GL.builtinDataRefusal(manifest.builtin_data));
  if (manifest.falls_log.length) {
    log("run.log's fall lines:");
    for (const l of manifest.falls_log) log(`  ${l.replace(/^\[[^\]]*\]\s*/, "")}`);
  }
  const result = printVerdict(manifest, manifest.error ? `(the run stopped early: ${manifest.error}) ` : "RESULT: ");
  log(`evidence: ${path.relative(REPO, OUT)}`);
  process.exit(result.ok && !manifest.error ? 0 : 2);
}

main();
