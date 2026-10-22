#!/usr/bin/env node
// photograph-home: stand in every room of the player's acre and take a picture.
//
// A home is judged by looking at it, and until this existed the only way to see
// a room was to boot the game by hand and walk there. This boots the release
// binary in a portable rig, enters the world through the dev autopilot, then
// for each vantage parks the camera at an authored pose and captures the real
// 3D viewport. The output is a folder of PNGs plus a manifest naming each one.
//
// The poses live in scripts/home-vantages.json, one row per room, so re-arranging
// the home is a data edit here too: move a room, move its vantage, re-run.
//
// Two engine IPC verbs do the work, both already used by the other rigs:
//   debug/showcase_request.json  {"cam":"x,y,z,yaw,pitch","time":"11","time_scale":"0"}
//       parks the free camera at a home-local pose and freezes the clock, so a
//       sweep's pictures differ by viewpoint and nothing else.
//   debug/screenshot_request.json  (optionally {"width":W,"height":H})
//       captures the viewport to debug/screenshot_N.png.
//
// Camera frame: x, y, z are metres in the home zone's own coordinates (the same
// numbers data/homes/homestead.ron uses; on plot p1 the home sits at the ship's origin, so
// they are ship metres too), y is EYE height so 1.7 is a
// standing person. yaw 0 looks north (-Z), +PI/2 east (+X), PI south, -PI/2
// west. pitch is radians, positive up.
//
// ONE GPU (CLAUDE.md): this refuses to boot while any HumanityOS.exe is
// running, the operator's own game included, and never sets the take-focus env
// var.
//
// Frame cost per room (2026-09-26): run with HUMANITY_FRAME_COSTS=1 in the
// environment and the engine rewrites debug/frame_costs.json once a second.
// After each capture the rig waits for a fresh one and keeps it beside the
// picture as <id>.costs.json, with frame_ms in the manifest, so a before and
// after of a room is two runs and a diff instead of a guess.
//
// Same-boot A/B (2026-09-27): `--ab KEY` photographs every vantage twice, once
// as it stands and once with the showcase switch KEY set to "0", then sets it
// back to "1" before the next pose. The pictures are <id>.png and
// <id>.KEY-off.png (each with its .costs.json when frame costs are on), so a
// dev switch like room GI's (`--ab room_gi`) or the aurora's is judged on two
// frames of one boot that differ by that switch and nothing else, and its cost
// is the difference of two costs files taken seconds apart.
//
// A/B between two VALUES (2026-10-21): `--ab KEY --ab-values ON,OFF` sets KEY to ON before the
// picture and to OFF for its twin (<id>.KEY-OFF.png), then releases KEY to its sticky default
// (scripts/lib/showcase-pins.js) before the next pose, waiting `--ab-settle MS` (default 1500)
// after each change. For a setting that is not an on/off switch: the Bounced light detail level,
// `--ab bounced_light_detail --ab-values low,ambient --ab-settle 5000`, photographs each room at
// Low and at Ambient in one boot (a level change lays the light out again, so give it seconds).
//
// `--settle MS` (default 1300) is the wait between the pose and the picture; `--early MS` also
// takes a picture that soon after the pose (<id>.early.png), to see what a room shows the moment
// the player arrives. With frame costs on, the room light probes' state at each picture
// (debug/room_gi.json: which rooms hold probes, the budget, the memory) is kept beside it as
// <id>.room_gi.json (and <id>.early.room_gi.json, <id>.KEY-OFF.room_gi.json).
//
// Usage:
//   node scripts/photograph-home.js [--exe PATH] [--only id,id] [--width N] [--height N] [--ab KEY]
//                                   [--ab-values ON,OFF] [--ab-settle MS] [--settle MS] [--early MS]
//                                   [--vantages PATH] [--allow-other-build "<reason>"]
// Exit 0 = every vantage captured. 1 = refused. 2 = one or more captures failed, a panic,
// or the run served a data file from the copy built into the exe (BUILT-IN DATA).
//
// THE BINARY (BUG-133): the exe must be this tree's build (the freshness gate,
// scripts/check-fresh-exe.js, by the source stamp compiled into it), or another
// build on purpose with --allow-other-build "<why>", recorded in the manifest as
// other_build. The copy in the rig must be byte-identical to what the gate
// judged, and the game starts through lib/game-launch.js spawnGame, which checks
// that copy again right before it spawns, sets HUMANITY_NO_HANDOFF=1 so it runs
// that copy and never hands itself to a newer v*_HumanityOS.exe, and reports (and
// stops) a hand-off an older build makes anyway. A run whose run.log says a data
// file was served from the copy built into the exe is refused (BUILT-IN DATA).

const fs = require("fs");
const path = require("path");
const { spawn, execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
// The one shared lookup for the DXC shader compiler dlls (see setupRig).
const DXC = require("./lib/dxc-dlls.js");
// The freshness gate and the boot-copy check (BUG-133).
const { runFreshGate, requireBootCopy, bootRecord, otherBuildNotice } = require("./lib/src-fingerprint.js");
// Starting the game, and what a run.log must not say (BUG-133).
const GL = require("./lib/game-launch.js");
const WR = require("./lib/write-request.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const ONLY = opt("--only", null);
const WIDTH = Number(opt("--width", "1600"));
const HEIGHT = Number(opt("--height", "900"));
const KEEP_OPEN = args.includes("--keep-open");
const AB = opt("--ab", null);
// --ab-values ON,OFF: the two values of an A/B that is not a "1"/"0" switch.
const AB_VALUES = opt("--ab-values", null);
const [AB_ON, AB_OFF] = AB_VALUES ? AB_VALUES.split(",").map((s) => s.trim()) : [null, "0"];
const AB_RELEASE = AB_VALUES ? require("./lib/showcase-pins.js").STICKY_PIN_RESETS[AB] ?? "1" : "1";
const AB_SETTLE = Number(opt("--ab-settle", "1500"));
const SETTLE = Number(opt("--settle", "1300"));
const EARLY = opt("--early", null) === null ? null : Number(opt("--early", "0"));

const RIG = path.join(REPO, ".probe-rig", "home-photos");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
// --vantages PATH reads another pose list of the same shape (trying out new
// poses without touching the shipped list).
const VANTAGES = path.resolve(opt("--vantages", path.join(__dirname, "home-vantages.json")));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[photos] ${m}`);
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

// ── refusals ────────────────────────────────────────────────────────────────
const running = MG.listInstances();
if (running.length) {
  refuse([
    `REFUSED: HumanityOS.exe is already running (pid ${running.map((p) => p.pid).join(", ")}).`,
    "One GPU, one instance (CLAUDE.md). Wait for it to exit, then run again.",
  ]);
}
if (!fs.existsSync(EXE)) {
  refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
}
if (!fs.existsSync(VANTAGES)) refuse([`ERROR: no vantage file at ${VANTAGES}`]);
if (AB_VALUES && (!AB || !AB_ON || !AB_OFF)) refuse(["ERROR: --ab-values ON,OFF needs --ab KEY and two values"]);

let vantages = JSON.parse(fs.readFileSync(VANTAGES, "utf8")).vantages;
if (ONLY) {
  const want = new Set(ONLY.split(",").map((s) => s.trim()));
  vantages = vantages.filter((v) => want.has(v.id));
  if (!vantages.length) refuse([`ERROR: --only ${ONLY} matched no vantage`]);
}

// This tree's build, or another on purpose (--allow-other-build "<why>"). Before
// the rig is touched, so a refusal changes nothing.
const FRESH = runFreshGate(EXE, args, { cwd: REPO });
if (FRESH.status !== 0) refuse(["photograph-home: REFUSED - see the freshness failure above. Nothing was booted."]);
const OTHER_BUILD = FRESH.other_build;

// ── rig ─────────────────────────────────────────────────────────────────────
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
  // portable.txt keeps identity/config/saves inside the rig and lets the dev
  // autopilot run; no_focus.txt is the belt to the launch-focus inversion's
  // braces, so a capture sweep can never take the operator's window focus.
  fs.writeFileSync(path.join(RIG, "portable.txt"), "photograph-home rig\n");
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  killRigProcesses();
  fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  // What boots is the copy, so the copy must be what the gate judged.
  requireBootCopy(path.join(RIG, "HumanityOS.exe"), FRESH, "photograph-home");
  // The DXC dlls take boot from ~25 s to ~5 s. Without them the fallback
  // shader compiler is so slow that a rig looks broken while it is merely
  // slow, which has been misread as a regression before. They come from
  // beside the exe or else the repo root (target/release, the default exe's
  // folder, holds none), through the one shared lookup,
  // scripts/lib/dxc-dlls.js, which logs which folder it used or that it
  // found neither.
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.png$/.test(f) || /_done\.json$/.test(f) || WR.isRequestLeftover(f) || f === "frame_costs.json" || f === "room_gi.json")
      fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}

// Written whole, through the one shared writer (scripts/lib/write-request.js): a side file
// renamed into place. A plain write can be read EMPTY by the game's next frame, which deletes
// it, and the request is lost with no error (verify-falls lost one so on 2026-10-20).
function req(name, body) {
  WR.writeRequest(path.join(DEBUG, name), body);
}
function clearDone(name) {
  const p = path.join(DEBUG, name);
  if (fs.existsSync(p)) fs.unlinkSync(p);
}
async function waitFile(name, timeoutMs, pollMs = 250) {
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
async function waitBoot(timeoutMs, game = null) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(LOG)) {
      const txt = fs.readFileSync(LOG, "utf8");
      if (/PANIC/.test(txt)) throw new Error("PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return true;
    }
    // An exit before boot finishes, said as one (naming a hand-off if the game made one).
    if (game && game.exited()) throw new Error(`before it finished booting, ${game.describe()}`);
    await sleep(1000);
  }
  throw new Error("the exe did not finish booting in time");
}
// The frame-cost drop written after this call (null when the run was not
// started with HUMANITY_FRAME_COSTS=1). The engine rewrites it once a
// second, so waiting for a newer mtime means the numbers are from frames
// drawn at this pose, not from the walk here.
async function freshCosts() {
  const p = path.join(DEBUG, "frame_costs.json");
  const since = Date.now();
  while (Date.now() - since < 4000) {
    try {
      if (fs.statSync(p).mtimeMs > since) return JSON.parse(fs.readFileSync(p, "utf8"));
    } catch {}
    await sleep(200);
  }
  return null;
}
const panicCount = () => (fs.existsSync(LOG) ? (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length : 0);
// The room light probes' dev drop (debug/room_gi.json, once a second with frame costs on), written
// after this call, kept beside a picture as <name>; nothing when frame costs are off.
async function keepRoomGi(outName) {
  const p = path.join(DEBUG, "room_gi.json");
  const since = Date.now();
  while (Date.now() - since < 2500) {
    try {
      if (fs.statSync(p).mtimeMs > since) {
        const body = JSON.parse(fs.readFileSync(p, "utf8"));
        fs.writeFileSync(path.join(OUT, outName), JSON.stringify(body, null, 2));
        return body;
      }
    } catch {}
    await sleep(150);
  }
  return null;
}
// One picture of whatever is on screen now, copied into the run folder as <name>.
async function capture(name) {
  clearDone("screenshot_done.json");
  req("screenshot_request.json", { width: WIDTH, height: HEIGHT });
  const d = await waitFile("screenshot_done.json", 30000);
  if (!d || d.ok !== true || !fs.existsSync(path.join(RIG, d.path))) return null;
  fs.copyFileSync(path.join(RIG, d.path), path.join(OUT, name));
  return name;
}

// ── the run ─────────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RIG, "runs", stamp);

async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "photograph-home",
    stamp,
    exe: EXE,
    // Which binary these pictures are of (BUG-133), and the record of a
    // deliberate other-build run.
    binary: bootRecord(FRESH, path.join(RIG, "HumanityOS.exe")),
    ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
    size: [WIDTH, HEIGHT],
    shots: [],
    panics: 0,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));

  log(`launching ${path.basename(EXE)} in ${path.relative(REPO, RIG)}`);
  // spawnGame: the copy is checked against the judged bytes right before the
  // spawn, HUMANITY_NO_HANDOFF=1 (run this copy, never a newer
  // v*_HumanityOS.exe), and an exit we did not ask for is watched for.
  const game = GL.spawnGame(path.join(RIG, "HumanityOS.exe"), [], {
    fresh: FRESH,
    rigName: "photograph-home",
    log,
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
  });
  const child = game.child;
  const pid = child.pid;
  child.unref();
  let killed = false;
  const kill = () => {
    if (killed) return;
    killed = true;
    game.expectExit(); // our own stop: not a hand-off to look for
    try {
      execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });
    } catch {}
    killRigProcesses();
  };
  process.on("exit", () => {
    if (!KEEP_OPEN) kill();
  });

  let failed = 0;
  try {
    log("waiting for boot...");
    await waitBoot(240000, game);
    log("entering world (autopilot)...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", {}); // no server_url: the pinned one stands. An empty one let the game fill in the live server and dial it (2026-10-05, BUG-160)
    const ap = await waitFile("autopilot_done.json", 240000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    // Park aboard the home once so the station frame, the fly mode and gravity
    // are all in the state the vantages assume; without this the first pose
    // lands while the camera is still wherever world entry left it.
    clearDone("camera_done.json");
    req("camera_request.json", { station: "home" });
    const cam = await waitFile("camera_done.json", 60000);
    if (!cam || cam.ok !== true) throw new Error(`station park failed: ${JSON.stringify(cam)}`);
    // The home's machines and walls are built a few frames after world entry.
    await sleep(6000);

    // THE HOME IS IN ORBIT AND THE CAMERA IS NOT GLUED TO IT. The showcase
    // `cam` verb teleports the player BODY, which then sits at a fixed point
    // while the station flies on: measured 2026-09-19, a camera left alone for
    // six seconds had drifted out through the roof and was looking back down at
    // the acre from sixty metres away. Setting the game hour is worse again,
    // because the station's position is propagated from the clock, so a
    // twelve-hour jump moves the whole home most of the way to the moon and
    // every picture comes back as empty space with a speck of homestead in it.
    //
    // So: no clock changes at all, and before every pose the camera is re-glued
    // to the station with a plain `camera_request`, which re-syncs the world
    // origin to the station's current position. Then the pose, then the capture,
    // promptly. The lighting in a picture is therefore whatever the world clock
    // happens to say; that is the honest cost of not touching it.
    for (const v of vantages) {
      clearDone("camera_done.json");
      req("camera_request.json", { station: "home" });
      const glue = await waitFile("camera_done.json", 20000);
      if (!glue || glue.ok !== true) {
        log(`FAIL ${v.id}: could not re-glue to the station: ${JSON.stringify(glue)}`);
        manifest.shots.push({ id: v.id, room: v.room, ok: false });
        failed++;
        save();
        continue;
      }
      const pose = `${v.pos[0]},${v.pos[1]},${v.pos[2]},${v.yaw},${v.pitch}`;
      req("showcase_request.json", { cam: pose });
      // --early: a picture the moment the player arrives (the room light
      // probes' state with it), before anything has settled.
      let early = null;
      if (EARLY !== null) {
        await sleep(EARLY);
        early = await capture(`${v.id}.early.png`);
        if (early) await keepRoomGi(`${v.id}.early.room_gi.json`);
      }
      // Long enough for the teleport, the machine meshes in the new room and
      // the lighting to settle; short enough that the drift above is under a
      // metre. The FIRST capture after a boot is a different state from every
      // later one (the rigs have been bitten by this), so the first vantage is
      // captured twice and the first is thrown.
      await sleep(SETTLE);
      // --ab-values: the picture is taken at the first value.
      if (AB && AB_ON) {
        req("showcase_request.json", { [AB]: AB_ON });
        await sleep(AB_SETTLE);
      }
      const shots = v === vantages[0] ? 2 : 1;
      let out = null;
      for (let i = 0; i < shots; i++) {
        clearDone("screenshot_done.json");
        req("screenshot_request.json", { width: WIDTH, height: HEIGHT });
        const d = await waitFile("screenshot_done.json", 30000);
        if (!d || d.ok !== true) {
          log(`FAIL ${v.id}: ${d ? d.error : "no screenshot_done.json"}`);
          failed++;
          out = null;
          break;
        }
        out = d.path;
        if (i + 1 < shots) await sleep(1200);
      }
      if (!out) {
        manifest.shots.push({ id: v.id, room: v.room, ok: false });
        save();
        continue;
      }
      const src = path.join(RIG, out);
      const name = `${v.id}.png`;
      if (fs.existsSync(src)) fs.copyFileSync(src, path.join(OUT, name));
      const shot = { id: v.id, room: v.room, note: v.note, pose, file: name, ok: fs.existsSync(path.join(OUT, name)) };
      if (early) shot.early_file = early;
      const costs = await freshCosts();
      if (costs) {
        fs.writeFileSync(path.join(OUT, `${v.id}.costs.json`), JSON.stringify(costs, null, 2));
        shot.frame_ms = costs.frame_ms;
      }
      const gi = await keepRoomGi(`${v.id}.room_gi.json`);
      if (gi) shot.room_gi = { resident_rooms: gi.resident_rooms, resident_probes: gi.resident_probes, budget: gi.budget_probes };
      // The A/B twin: the same pose with the switch off (or at the second
      // value), then the switch released so the next vantage starts from the
      // normal state.
      if (AB) {
        req("showcase_request.json", { [AB]: AB_OFF });
        await sleep(AB_SETTLE);
        clearDone("screenshot_done.json");
        req("screenshot_request.json", { width: WIDTH, height: HEIGHT });
        const d = await waitFile("screenshot_done.json", 30000);
        const offName = AB_VALUES ? `${v.id}.${AB}-${AB_OFF}.png` : `${v.id}.${AB}-off.png`;
        const offStem = offName.replace(/\.png$/, "");
        if (d && d.ok === true && fs.existsSync(path.join(RIG, d.path))) {
          fs.copyFileSync(path.join(RIG, d.path), path.join(OUT, offName));
          shot.off_file = offName;
          const offCosts = await freshCosts();
          if (offCosts) {
            fs.writeFileSync(path.join(OUT, `${offStem}.costs.json`), JSON.stringify(offCosts, null, 2));
            shot.off_frame_ms = offCosts.frame_ms;
          }
          await keepRoomGi(`${offStem}.room_gi.json`);
        } else {
          log(`FAIL ${v.id} (${AB} ${AB_OFF}): ${d ? d.error : "no screenshot_done.json"}`);
          failed++;
        }
        req("showcase_request.json", { [AB]: AB_RELEASE });
        await sleep(300);
      }
      manifest.shots.push(shot);
      save();
      log(`ok   ${v.id.padEnd(16)} ${v.note || ""}`);
    }
  } catch (e) {
    log(`ERROR: ${e.message}`);
    failed++;
  }

  manifest.panics = panicCount();
  // BUILT-IN DATA (BUG-133): a loader served the exe's own copy of a data file
  // instead of the tree's (missing, or it does not parse): not this tree's run.
  manifest.builtin_data = GL.builtinDataLines(fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "");
  save();
  // --keep-open leaves the game running for a look around (2026-09-26: this
  // kill() used to run regardless, so the flag did nothing).
  if (KEEP_OPEN) log(`left running (--keep-open): taskkill /PID ${pid} /T /F to close it`);
  else kill();
  log(`${manifest.shots.filter((s) => s.ok).length}/${vantages.length} captured, ${manifest.panics} panic(s)`);
  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  if (manifest.builtin_data.length) log(GL.builtinDataRefusal(manifest.builtin_data));
  log(`evidence: ${path.relative(REPO, OUT)}`);
  process.exit(failed || manifest.panics || manifest.builtin_data.length ? 2 : 0);
}

main();
