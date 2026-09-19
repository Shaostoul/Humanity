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
// numbers data/blueprints/ship_structure.ron uses), y is EYE height so 1.7 is a
// standing person. yaw 0 looks north (-Z), +PI/2 east (+X), PI south, -PI/2
// west. pitch is radians, positive up.
//
// ONE GPU (CLAUDE.md): this refuses to boot while any HumanityOS.exe is
// running, the operator's own game included, and never sets the take-focus env
// var.
//
// Usage:
//   node scripts/photograph-home.js [--exe PATH] [--only id,id] [--width N] [--height N]
// Exit 0 = every vantage captured. 1 = refused. 2 = one or more captures failed.

const fs = require("fs");
const path = require("path");
const { spawn, execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");

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

const RIG = path.join(REPO, ".probe-rig", "home-photos");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const VANTAGES = path.join(__dirname, "home-vantages.json");

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

let vantages = JSON.parse(fs.readFileSync(VANTAGES, "utf8")).vantages;
if (ONLY) {
  const want = new Set(ONLY.split(",").map((s) => s.trim()));
  vantages = vantages.filter((v) => want.has(v.id));
  if (!vantages.length) refuse([`ERROR: --only ${ONLY} matched no vantage`]);
}

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
  // The DXC dlls beside the exe take boot from ~25 s to ~5 s. Without them the
  // fallback shader compiler is so slow that a rig looks broken while it is
  // merely slow, which has been misread as a regression before.
  for (const dll of ["dxcompiler.dll", "dxil.dll"]) {
    const s = path.join(path.dirname(EXE), dll);
    if (fs.existsSync(s)) fs.copyFileSync(s, path.join(RIG, dll));
  }
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
const panicCount = () => (fs.existsSync(LOG) ? (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length : 0);

// ── the run ─────────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RIG, "runs", stamp);

async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = { kind: "photograph-home", stamp, exe: EXE, size: [WIDTH, HEIGHT], shots: [], panics: 0 };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));

  log(`launching ${path.basename(EXE)} in ${path.relative(REPO, RIG)}`);
  const child = spawn(path.join(RIG, "HumanityOS.exe"), [], {
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
  });
  const pid = child.pid;
  child.unref();
  let killed = false;
  const kill = () => {
    if (killed) return;
    killed = true;
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
    await waitBoot(240000);
    log("entering world (autopilot)...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: "" });
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

    for (const v of vantages) {
      const pose = `${v.pos[0]},${v.pos[1]},${v.pos[2]},${v.yaw},${v.pitch}`;
      req("showcase_request.json", { cam: pose, time: String(v.time ?? 11), time_scale: "0" });
      // Two seconds is enough for the teleport, the machine meshes in the new
      // room and the lighting to settle. The FIRST capture after a boot is a
      // different state from every later one (the rigs have been bitten by
      // this), so the first vantage is captured twice and the first is thrown.
      await sleep(2200);
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
      manifest.shots.push({ id: v.id, room: v.room, note: v.note, pose, file: name, ok: fs.existsSync(path.join(OUT, name)) });
      save();
      log(`ok   ${v.id.padEnd(16)} ${v.note || ""}`);
    }
  } catch (e) {
    log(`ERROR: ${e.message}`);
    failed++;
  }

  manifest.panics = panicCount();
  save();
  kill();
  log(`${manifest.shots.filter((s) => s.ok).length}/${vantages.length} captured, ${manifest.panics} panic(s)`);
  log(`evidence: ${path.relative(REPO, OUT)}`);
  process.exit(failed || manifest.panics ? 2 : 0);
}

main();
