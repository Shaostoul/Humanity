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
// ON A PLANET'S GROUND (the second increment, 2026-10-20). A planet has its own walk
// (`surface_move::radial_step`, which lib.rs hands to the body through
// `CameraController::surface_step` right after `state.on_ground_planet = vs.grounded;`), its own
// gravity (the body's data file) and its own exemptions (dev flight, the flight band, water, a
// camera park's pin), and nothing aboard touches any of them. So after the legs aboard the rig
// parks over open Sahara sand at 23.5 N, 13.08 E (the probe sweep's walk vantage,
// planet-open-noon-walk: flat, dry, nothing built, reachable on foot), lets the ground stream in,
// stands the player on it on foot (`stand`, `walk`), and twice holds Space and lets go: on a
// planet Space is a climb at walking speed, so the body rises about 2.6 m (the climb and its
// coast) and falls back. Forgiving must hurt nobody; Realistic must give what falls.ron says of
// the landing, at the gravity the planet walk itself pulled with (the `falls` block reports it on
// a planet, and the judge checks it against data/planets/earth.ron). Last the player is PLACED
// 3 m over the ground (`stand`) and must not be hurt. The height is the eye's distance from the
// planet's centre (`planet_r`, every recorder frame), not the camera's y. The climb is the walk's
// own speed, limp included: the sprained and broken ankles the legs aboard leave climb at 1.5 m/s,
// not 5 (the first run with the hold timed for 5 m/s rose 0.7 m), so the rig reads the rate the
// game reports (`falls.climb_m_s`) and holds Space as long as that needs (1.68 s).
//
// WHY NOT A DECK. The brief was a deck of known height built on the ground with the dev `build`
// verb, walked off. The planet walk does not stand on built pieces: its floor is the terrain alone
// (lib.rs feeds radial_step the drawn ground's radius; built pieces only stop a step sideways,
// `planet_build::collide_on_site`). Tried in the running game on 2026-10-20: `build` stood up four
// wood walls and a Wood Roof 40 m east of the legs' spot ("5 of 5 pieces stood up"), and the eye
// put at standing height over the roof's top (3.11 m over the ground) came to rest 0.03 m BELOW
// the ground's standing height beside the hut: straight through the roof to the ground inside.
// So a player cannot stand on anything built on a planet, and the climb on Space is the one way
// on foot to leave a planet's ground by metres.
//
// THE RIG QUIRK THIS FOUND AND FIXED. A camera park pins the eye (`probe_hold`) until movement
// input arrives; a person's key press lets go of it, and the rig's `hold` did not. The pin is one
// of surface_step's exemptions, so every fall a rig made after a park was exempt. Seen in the
// running game (2026-10-20, the build before the fix): `probe_hold` true before and after the held
// Space on both planet fall legs, the climb's landing (3.5 m/s) never handed to the body, and the
// rig said FAIL on planet_forgiving_fall.park_released, planet_realistic_fall.park_released and
// planet_placed_drop.park_released (tests/fixtures/falls/red-park-held.json). On a planet the pin
// did NOT stop the walk moving the body (the frame lock moves the body by its anchor, which the
// pin does not hold): it only made the falls exempt. Both key paths now go through
// engine::input::apply_game_action (src/engine/rig_walk.rs tests it, and that lib.rs's keyboard
// arm and `hold_keys` both call it).
//
// A REQUEST LOST MID-WRITE. One run's planet Forgiving leg ran in Realistic: its `illness` request
// was read while the rig was still writing it, empty, and deleted (run.log had no "Showcase:
// illness" line). The rig now writes every request to a side name and renames it into place, and
// the game leaves an empty showcase request for the next frame (engine/ipc.rs take_request_text).
//
// RED FIRST ON A PLANET (2026-10-20). Run against a build with lib.rs's `surface_step` call (the
// planet hand-off) commented out, the quirk fixed: the eye rose 2.60 m and 2.63 m on Space and
// fell back on Earth's 9.81 m/s^2, and the rig said (exit 2):
//   FAIL  planet_forgiving_fall.landing_handed 0 landings reached the body for a 2.60 m drop
//         (expected exactly 1). The planet walk's landing never got to player_landed: the lib.rs
//         hand-off after radial_step (CameraController::surface_step), an exemption left on (dev
//         flight, the flight band, the camera park, water), or radial_step reporting no impact
//   FAIL  planet_realistic_fall.landing_handed (the same, 2.63 m)
//   FAIL  planet_realistic_fall.wound      read at 7.19 m/s falls.ron gives Sprained Ankle
//                                          (sprained_ankle); the body has no wound
//   FAIL  planet_realistic_fall.health     health 78.81 -> 78.81, took -0.00; falls.ron at 7.19
//                                          m/s takes 7.04 (tolerance 0.5)
//   RESULT: FAIL  60/64 passed
// With it back, 69/69 (76/76 once the stairs-down leg was added): both planet landings reached the
// body at about 7.0 m/s, Forgiving took nothing, Realistic sprained the (already sprained) ankle
// again and took 6.2 health. The red run is tests/fixtures/falls/red-surface-step-removed.json.
//
// THE PLANET LANDING READS ABOUT 2% SLOW. The game's planet landing was 7.03 m/s where the eye's
// own 2.62 m drop at 9.81 gives 7.17: the planet walk integrates a fall frame by frame (v then r,
// surface_walk::vertical_step), which loses speed in proportion to the frame (20 frames of 33 ms
// fall: sqrt(20/21) = 0.976, measured 0.979), and `falls::landing_speed` corrects only the last
// frame. Inside the judge's 3% at 30 frames a second; at 54 ms frames it would be about 4% and
// `landing_speed` would fail. Not fixed here (it is the planet walk's integrator).
//
// DOWN THE STAIRS (BUG-208, 2026-10-20). Before BUG-208 nothing in the aboard walk followed the
// floor down, so walking DOWN the homestead's stairs flew off the top tread and landed at the
// bottom from 3.0 m at 7.67 m/s, a sprained ankle in Realistic, and no leg here walked down
// stairs. The realistic_stairs_down leg (right after the Forgiving walk, so the body walks at the
// full 5 m/s) walks up to one of the top treads (watched, not timed), is turned round there by a
// placement onto the top tread, and walks down the whole flight: no landing may reach the body.
// Red first (2026-10-20), on main with BUG-208's step down taken out (camera.rs's "Down a step,
// or off an edge" block disabled), the rig said (exit 2):
//   FAIL  realistic_stairs_down.no_landing a landing reached the body (1 -> 2, the last at 7.67
//         m/s) walking DOWN the stairs: the walk left a tread instead of stepping down it (a 3.00 m
//         flight lands at 7.67 m/s if it is all one fall). The aboard walk's step down (BUG-208,
//         renderer/camera.rs, STEP_DOWN_M)
//   FAIL  realistic_stairs_down.no_wound   new wound(s) sprained_ankle from walking down stairs
//   FAIL  realistic_stairs_down.health     health 100.00 -> 90.32
//   RESULT: FAIL  73/76 passed
// (tests/fixtures/falls/red-stairs-down-before-bug208.json). With the step down back, 76/76: the
// eye walked down all 3.00 m of the stairs, no landing reached the body, no wound, health 100 to
// 100 (tests/fixtures/falls/green-with-planet.json, every leg of the rig).
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
const TIMEOUT_MS = Number(opt("--timeout-min", "10")) * 60 * 1000;

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
/** The stairs-down leg (BUG-208): forward held first this long from the foot (about halfway up
 *  at the walk's 5 m/s), then in short pulses until the body stands within STOP_BELOW_TOP_M of the
 *  top, which a placement can step it up from onto the top tread (the floor sampler's step up is
 *  0.6 m, engine/footing.rs STEP_UP_M). */
const UP_FIRST_S = 0.55;
const UP_PULSE_S = 0.05;
const STOP_BELOW_TOP_M = 0.5;
/** Then forward held this long from the top tread, facing down: past the stairs' foot at 5 m/s. */
const DOWN_HOLD_S = 1.0;
const DOWN_RECORD_S = 5;
/** The ladder leg: Space held this long climbs the 3.2 m to the top at the climb's 3 m/s and
 *  waits there; then no key for HANG_S (the body must hold on); then back held for STEP_BACK_S,
 *  which takes the body out of the ladder's reach (1.2 m) and off it. */
const LADDER_CLIMB_S = 1.8;
const HANG_S = 1.5;
const STEP_BACK_S = 1.2;
const LADDER_RECORD_S = LADDER_CLIMB_S + HANG_S + STEP_BACK_S + 3;
/** Eye height above the feet, metres (src/renderer/camera.rs, `eye_height`). */
const EYE_M = 1.7;
/** The planet legs (scripts/lib/falls-judge.js PLANET: open Sahara sand) climb this high, metres
 *  over the ground, and fall back: at Earth's 9.81 a 7.1 m/s landing, past falls.ron's sprain
 *  (6.26) and short of its broken ankle (7.73) in Realistic, under its first harm at 70% in
 *  Forgiving (4.97). */
const PLANET_RISE_M = 2.6;
/** The planet climb's thrust (src/surface_walk.rs `vertical_step`): it ramps to the climb rate at
 *  max(3 g, 20 m/s^2, rate / 1.5 s). */
const THRUST_ACCEL_G_MULT = 3.0;
const THRUST_ACCEL_MIN = 20.0;
const THRUST_RAMP_S = 1.5;
/**
 * How long to hold Space on a planet to rise `rise` metres. On a planet Space is a climb at the
 * walk's own speed (`surface_move::radial_step`'s walking branch), so the rate is whatever the
 * body walks at now: 5 m/s unhurt, 1.5 m/s with the sprained and broken ankles the legs aboard
 * leave (the game reports it, `falls.climb_m_s`). The thrust ramps to that rate, holds it, and when
 * the key comes up the body coasts up another v^2 / 2 g before it falls. Unhurt at 9.81 that is
 * 0.35 s for 2.6 m; with both ankles hurt, 1.68 s. A frame late letting go adds a few centimetres,
 * and the judge reads the drop the eye really made.
 */
function climbHoldS(rate, g, rise) {
  const a = Math.max(THRUST_ACCEL_G_MULT * g, THRUST_ACCEL_MIN, rate / THRUST_RAMP_S);
  const rampS = rate / a;
  const rampM = (rate * rate) / (2 * a);
  const coastM = (rate * rate) / (2 * g);
  return rampS + Math.max(0, rise - rampM - coastM) / rate;
}
/** How long each planet leg is recorded after Space goes down, seconds, beyond the hold: about
 *  1.5 s of coast and fall, most of a second for the settle glue to set the eye down, and the
 *  body pass. */
const PLANET_RECORD_AFTER_S = 4;
/** The planet placement: the eye put this many metres over its standing height, then let fall. */
const PLANET_PLACED_M = 3.0;
/** After the camera park, seconds for the ground under it to stream in before the stand (the
 *  probe sweep's own settle for this vantage). */
const PLANET_SETTLE_S = 10;

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
      `${id.padEnd(22)} ${String(o.where).padEnd(8)} g ${fmt(o.gravity_m_s2)}  mode ${String(o.mode).padEnd(9)} health ${fmt(o.health_before)} -> ${fmt(o.health_after)}  ` +
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
    if (/\.png$/.test(f) || /_done\.json$/.test(f) || /_request\.json(\.part)?$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}

/** A request file, written whole: to a side name first, then renamed into place. The game polls
 *  for the file every frame and deletes it once read, so a plain write can be caught between
 *  creating the file and filling it: the game reads it empty, deletes it, and the request is lost
 *  with no error anywhere. That lost the planet Forgiving leg's `illness` verb on 2026-10-20 (the
 *  run.log has no "Showcase: illness -> Forgiving" line, and the leg ran in Realistic). */
function req(name, body) {
  const p = path.join(DEBUG, name);
  fs.writeFileSync(`${p}.part`, JSON.stringify(body));
  fs.renameSync(`${p}.part`, p);
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
    // planet_r: on a planet's ground, the eye's distance from the body's centre (the height a
    // planet fall is read in; null aboard).
    frames: (done.frames || []).map((f) => ({ t: f.t, cam: f.cam, health: f.health, ...(f.planet_r !== undefined ? { planet_r: f.planet_r } : {}) })),
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
    // Where the planet legs run: written before the boot, so a run that stopped before them is
    // judged as missing them (the judge requires the planet legs of any run that planned them).
    planet: DATA.planet,
    // Every leg this rig runs: the judge requires them all of a run that carries a plan.
    plan: FJ.ALL_LEGS,
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

    // DOWN THE STAIRS (BUG-208), in Realistic, before anything has hurt the body (so it walks at
    // the full 5 m/s, the speed that used to fly off the top tread). Up from the foot to one of
    // the top treads (watched, not timed: a long frame walks further), turned round there by a
    // placement onto the top tread (the watch starts over once standing), then forward held down
    // the whole flight. Every riser must be a step down: no landing, no wound, no health lost.
    {
      const id = "realistic_stairs_down";
      const r = (st.rot_deg * Math.PI) / 180;
      const alongOf = (c) => (c[0] - o[0] - st.pos[0]) * Math.sin(r) + (c[2] - o[2] - st.pos[2]) * Math.cos(r);
      await showcase({ illness: "realistic" });
      await showcase({ walk: "1", cam: pose(foot) });
      await sleep(1500);
      await showcase({ hold: "forward", hold_s: String(UP_FIRST_S) });
      await sleep(UP_FIRST_S * 1000 + 500);
      for (let k = 0; ; k++) {
        const s = await snapshot(`${id} (going up)`);
        const c = s.camera_end && s.camera_end.pos;
        if (!Array.isArray(c)) throw new Error(`${id}: the recorder gave no camera position`);
        const ground = c[1] - o[1] - EYE_M;
        if (alongOf(c) > st.size[2] / 2) throw new Error(`${id}: walked off the top of the stairs while stopping on them (along ${alongOf(c).toFixed(2)} m)`);
        if (ground >= st.top_m - STOP_BELOW_TOP_M) {
          log(`${id}: standing ${ground.toFixed(2)} m up the stairs after ${k} pulse(s)`);
          break;
        }
        if (k >= 40) throw new Error(`${id}: still only ${ground.toFixed(2)} m up the stairs after ${k} pulses of forward`);
        await showcase({ hold: "forward", hold_s: String(UP_PULSE_S) });
        await sleep(350);
      }
      const top = at(st.top_tread_eye);
      await showcase({ walk: "1", cam: `${top.map((v) => v.toFixed(3)).join(",")},${st.down_yaw.toFixed(4)},0` });
      await sleep(1500);
      const before = await snapshot(`${id} (before)`);
      await startRecording(DOWN_RECORD_S);
      await sleep(300);
      await showcase({ hold: "forward", hold_s: String(DOWN_HOLD_S) });
      const after = await finishRecording(DOWN_RECORD_S, `${id} (the walk down)`);
      manifest.legs.push({ id, mode: "realistic", kind: "stairs_down", start: top, before, after });
      save();
      log(`${id}: health ${fmt(before.vitals && before.vitals.health)} -> ${fmt(after.vitals && after.vitals.health)}, landings ${before.falls && before.falls.landings} -> ${after.falls && after.falls.landings}`);
    }
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

    // ── THE PLANET LEGS ── Parked over open Sahara sand (the camera park pins the eye there,
    // `probe_hold`), the ground left to stream in, then stood on it on foot (`stand`, `walk`).
    // Each fall leg stands the player afresh (a placement: the watch starts over once standing),
    // holds Space long enough to rise PLANET_RISE_M and lets the body fall back. The first held
    // key must let go of the park, as a person's does (engine/rig_walk.rs `hold`).
    {
      const P = DATA.planet;
      const at = `${P.lat},${P.lon}`;
      const stand = (h) => ({ stand: `0,${h.toFixed(2)},0,0,-5`, stand_at: at, walk: "1" });
      log(`planet: parking over ${P.body} at ${at} (the probe sweep's planet-open-noon-walk ground)`);
      clearDone("camera_done.json");
      req("camera_request.json", { body: P.body, lat: P.lat, lon: P.lon, altitude_km: P.park_altitude_km, look_offset_deg: 60 });
      const cam = await waitFile("camera_done.json", 60000);
      if (!cam || cam.ok !== true) throw new Error(`the planet park failed: ${JSON.stringify(cam)}`);
      await sleep(PLANET_SETTLE_S * 1000);
      await showcase(stand(EYE_M));
      await sleep(3000);
      const planetLeg = async (id, mode) => {
        await showcase({ illness: mode });
        await showcase(stand(EYE_M));
        await sleep(2000); // the stand settles; the watch starts over on the first frame standing
        const before = await snapshot(`${id} (before)`);
        const rate = before.falls ? Number(before.falls.climb_m_s) : NaN;
        const g = before.falls ? Number(before.falls.gravity_m_s2) : NaN;
        if (!(rate > 0.1) || !(g > 0)) {
          throw new Error(`${id}: the game reports no climb (climb_m_s ${rate}, gravity ${g}): is the build older than the planet legs, or is the body overloaded?`);
        }
        const holdS = climbHoldS(rate, g, PLANET_RISE_M);
        const recordS = holdS + PLANET_RECORD_AFTER_S;
        log(`${id}: climbing at ${rate.toFixed(2)} m/s, so Space is held ${holdS.toFixed(2)} s to rise about ${PLANET_RISE_M} m`);
        await startRecording(recordS);
        await sleep(300);
        await showcase({ hold: "jump", hold_s: holdS.toFixed(3) });
        const after = await finishRecording(recordS, `${id} (the climb and the fall)`);
        manifest.legs.push({ id, mode, kind: "planet_fall", body: P.body, climb_m_s: rate, hold_s: holdS, before, after });
        save();
        log(`${id}: health ${fmt(before.vitals && before.vitals.health)} -> ${fmt(after.vitals && after.vitals.health)}, landings ${before.falls && before.falls.landings} -> ${after.falls && after.falls.landings}, park ${before.falls && before.falls.probe_hold} -> ${after.falls && after.falls.probe_hold}`);
      };
      await planetLeg("planet_forgiving_fall", "forgiving");
      await planetLeg("planet_realistic_fall", "realistic");
      {
        const id = "planet_placed_drop";
        await showcase({ illness: "realistic" });
        await showcase(stand(EYE_M));
        await sleep(2000);
        const before = await snapshot(`${id} (before)`);
        await startRecording(DROP_RECORD_S);
        await sleep(300);
        await showcase(stand(EYE_M + PLANET_PLACED_M));
        const after = await finishRecording(DROP_RECORD_S, `${id} (the drop)`);
        manifest.legs.push({ id, mode: "realistic", kind: "planet_placement", body: P.body, placed_m: PLANET_PLACED_M, before, after });
        save();
        log(`${id}: health ${fmt(before.vitals && before.vitals.health)} -> ${fmt(after.vitals && after.vitals.health)}, landings ${before.falls && before.falls.landings} -> ${after.falls && after.falls.landings}`);
      }
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
