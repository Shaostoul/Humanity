#!/usr/bin/env node
// make-clips: film the game for social media (2026-09-30).
//
// Boots the release build in a background rig (it never takes your window,
// never makes a sound), flies to each shot in scripts/clips.json, records it in
// the engine's movie mode (src/engine/movie.rs: every frame advances time by
// exactly 1/30 s and goes straight to ffmpeg, so the video is smooth however
// slowly the frames render), then cuts the postable versions:
//
//   <id>-16x9.mp4   1920x1080, for X, YouTube, LinkedIn, Facebook
//   <id>-9x16.mp4   1080x1920, for Reels, Shorts, TikTok (a centre crop of
//                   the same shot, so it is softer: see LIMITS below)
//   <id>.jpg        a still from the middle of the shot, for a thumbnail
//   <id>-master.mp4 the full-size recording everything else is cut from
//
// plus clips.md, listing every shot with its suggested caption. The folder is
// Videos\HumanityOS clips\<date-time> in your user folder unless --out says
// otherwise.
//
// Graphics: the rig copies YOUR graphics settings (the same mirror
// `just probe-sweep --operator-config` uses), so a clip looks the way the game
// looks on your machine. --rig-defaults skips that.
//
// LIMITS (first version): the picture is the game window's size, so the
// vertical cut is a crop of a landscape frame. A native portrait render is the
// next rung. Clips are silent; captions carry them, the way most social video
// is watched anyway.
//
// ONE GPU (CLAUDE.md): refuses to start while any HumanityOS.exe is running,
// yours included.
//
// Usage:
//   node scripts/make-clips.js [--only id,id] [--exe PATH] [--out DIR]
//                              [--shots PATH] [--rig-defaults] [--keep-master-only]
// Exit 0 = every clip made. 1 = refused. 2 = one or more clips failed.

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
const G = require("./rig-graphics.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const ONLY = opt("--only", null);
const SHOTS = path.resolve(opt("--shots", path.join(__dirname, "clips.json")));
const RIG_DEFAULTS = args.includes("--rig-defaults");
const MASTER_ONLY = args.includes("--keep-master-only");

const RIG = path.join(REPO, ".probe-rig", "clips");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.resolve(opt("--out", path.join(os.homedir(), "Videos", "HumanityOS clips", stamp)));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[clips] ${m}`);
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

// ── ffmpeg, for the cuts (the engine finds its own for the master) ──────────
function findFfmpeg() {
  const tries = ["ffmpeg"];
  const op = G.readConfig(G.operatorConfigPath());
  if (op.ok && op.cfg.ffmpeg_path) {
    const p = op.cfg.ffmpeg_path;
    tries.push(p, path.join(p, "ffmpeg.exe"));
  }
  tries.push("C:\\Apps\\ffmpeg\\bin\\ffmpeg.exe");
  for (const t of tries) {
    const r = spawnSync(t, ["-version"], { stdio: "ignore" });
    if (r.status === 0) return t;
  }
  return null;
}

// ── refusals ────────────────────────────────────────────────────────────────
const running = MG.listInstances();
if (running.length) {
  refuse([
    `REFUSED: HumanityOS.exe is already running (pid ${running.map((p) => p.pid).join(", ")}).`,
    "One GPU, one instance (CLAUDE.md). Close it, then run again.",
  ]);
}
if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
const FFMPEG = findFfmpeg();
if (!FFMPEG) refuse(["ERROR: ffmpeg not found (PATH, Settings > Media, or C:\\Apps\\ffmpeg\\bin)."]);
let clips = JSON.parse(fs.readFileSync(SHOTS, "utf8")).clips;
if (ONLY) {
  const want = new Set(ONLY.split(",").map((s) => s.trim()));
  clips = clips.filter((c) => want.has(c.id));
  if (!clips.length) refuse([`ERROR: --only ${ONLY} matched no shot in ${SHOTS}`]);
}

// ── rig (the same portable sandbox photograph-home.js builds) ───────────────
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
  fs.writeFileSync(path.join(RIG, "portable.txt"), "make-clips rig\n");
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  killRigProcesses();
  fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  // The DXC shader compiler: without it the fallback compiler is so slow that
  // world entry outlasts every timeout (first run, 2026-09-30: four minutes of
  // silence after boot). It sits beside the exe or, for target/release, in the
  // repo root.
  for (const dll of ["dxcompiler.dll", "dxil.dll"]) {
    const s = [path.join(path.dirname(EXE), dll), path.join(REPO, dll)].find((p) => fs.existsSync(p));
    if (s) fs.copyFileSync(s, path.join(RIG, dll));
    else log(`WARNING: ${dll} not found; world entry will be very slow`);
  }
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.(png|mp4)$/.test(f) || /_done\.json$/.test(f) || /_request\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
  if (!RIG_DEFAULTS) mirrorGraphics();
}
// Your visual settings into the rig's config (never identity, vault, server
// or window keys: rig-graphics.js decides what is visual).
function mirrorGraphics() {
  const op = G.readConfig(G.operatorConfigPath());
  if (!op.ok) {
    log(`graphics: your config could not be read (${op.error}); recording at the rig's defaults`);
    return;
  }
  const rigPath = G.rigConfigPath(RIG);
  const rig = G.readConfig(rigPath);
  const m = G.mirrorOperatorGraphics(rig.ok ? rig.cfg : {}, op.cfg);
  fs.mkdirSync(path.dirname(rigPath), { recursive: true });
  fs.writeFileSync(rigPath, JSON.stringify(m.next, null, 2));
  log(`graphics: your settings mirrored (${Object.keys(m.copied).length} changed from the rig's)`);
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
    if (panicCount()) throw new Error("the game panicked (see .probe-rig/clips/logs/run.log)");
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

async function park(camera, what) {
  clearDone("camera_done.json");
  req("camera_request.json", camera);
  const d = await waitFile("camera_done.json", 60000);
  if (!d || d.ok !== true) throw new Error(`${what}: ${JSON.stringify(d)}`);
}

// ── the cuts ────────────────────────────────────────────────────────────────
// Landscape and portrait from one master: crop to the target shape around the
// centre (never stretch), then scale with Lanczos. Even sides for yuv420p.
const cropTo = (a, b) =>
  `crop='trunc(min(iw\\,ih*${a}/${b})/2)*2':'trunc(min(ih\\,iw*${b}/${a})/2)*2'`;
function ff(argv, what) {
  const r = spawnSync(FFMPEG, ["-hide_banner", "-loglevel", "error", "-y", ...argv], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`${what}: ${(r.stderr || "").trim() || `ffmpeg exited ${r.status}`}`);
}
function cut(master, id, seconds) {
  const enc = ["-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-an"];
  const wide = path.join(OUT, `${id}-16x9.mp4`);
  ff(["-i", master, "-vf", `${cropTo(16, 9)},scale=1920:1080:flags=lanczos`, ...enc, wide], `${id} 16x9`);
  ff(["-i", master, "-vf", `${cropTo(9, 16)},scale=1080:1920:flags=lanczos`, ...enc, path.join(OUT, `${id}-9x16.mp4`)], `${id} 9x16`);
  ff(["-ss", String(Math.max(0, seconds / 2)), "-i", wide, "-frames:v", "1", "-q:v", "2", path.join(OUT, `${id}.jpg`)], `${id} still`);
}

// ── the run ─────────────────────────────────────────────────────────────────
async function main() {
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = { kind: "make-clips", stamp, exe: EXE, clips: [], panics: 0 };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));

  log(`launching ${path.basename(EXE)} in the background rig`);
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
  process.on("exit", kill);

  let failed = 0;
  try {
    log("waiting for boot...");
    await waitBoot(240000);
    log("entering the world...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: "" });
    const ap = await waitFile("autopilot_done.json", 240000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    await sleep(4000);

    for (const c of clips) {
      const t0 = Date.now();
      try {
        // Release anything a previous shot, or the rig's own defaults, pinned:
        // the recording's clock must drive the clouds, the water and the wind.
        req("showcase_request.json", { wind: "auto", anim_clock: "auto", time_scale: "1" });
        await sleep(400);
        if (c.showcase) {
          req("showcase_request.json", c.showcase);
          await sleep(800);
        }
        // Park, hold the clock while the ground streams in, park again (the
        // first park of a boot can land at the wrong hour: probe-sweep.js,
        // RE-PARK), and settle.
        await park(c.camera, `${c.id} park`);
        req("showcase_request.json", { time_scale: "0" });
        await sleep((c.settle_s ?? 8) * 1000);
        if (!c.camera.station) {
          await park(c.camera, `${c.id} re-park`);
          await sleep(4000);
        }
        if (c.during) req("showcase_request.json", c.during);
        else req("showcase_request.json", { time_scale: "1" });
        await sleep(300);

        const rec = Object.assign({ fps: 30 }, c.record, { out: `debug/clip_${c.id}.mp4` });
        const frames = Math.round(((rec.warmup_s ?? 2) + (rec.seconds ?? 8)) * rec.fps);
        clearDone("record_done.json");
        req("record_request.json", rec);
        log(`recording ${c.id} (${rec.seconds ?? 8} s)...`);
        // Generous: a heavy frame at full size can take a second to draw.
        const done = await waitFile("record_done.json", Math.max(180000, frames * 2500), 500);
        req("showcase_request.json", { time_scale: "0" });
        if (!done || done.ok !== true) throw new Error(`recording: ${done ? done.error : "no record_done.json"}`);

        const master = path.join(OUT, `${c.id}-master.mp4`);
        fs.copyFileSync(path.join(RIG, done.path), master);
        if (!MASTER_ONLY) cut(master, c.id, rec.seconds ?? 8);
        manifest.clips.push({
          id: c.id, title: c.title, caption: c.caption, ok: true, frames: done.frames,
          size: done.size, record_wall_s: Math.round(done.wall_s), total_s: Math.round((Date.now() - t0) / 1000),
        });
        log(`ok   ${c.id.padEnd(22)} ${done.frames} frames at ${done.size.join("x")}, ${Math.round(done.wall_s)} s to record`);
      } catch (e) {
        failed++;
        manifest.clips.push({ id: c.id, title: c.title, ok: false, error: e.message });
        log(`FAIL ${c.id}: ${e.message}`);
        if (/panicked/.test(e.message)) break;
      }
      save();
    }
  } catch (e) {
    log(`ERROR: ${e.message}`);
    failed++;
  }

  manifest.panics = panicCount();
  save();
  kill();
  // A plain index to post from: what each file is and a first line for it.
  const lines = [`# HumanityOS clips, ${stamp}`, ""];
  for (const c of manifest.clips) {
    lines.push(`## ${c.title || c.id}${c.ok ? "" : " (FAILED)"}`, "");
    if (c.ok) {
      lines.push(`- Landscape: ${c.id}-16x9.mp4`, `- Portrait: ${c.id}-9x16.mp4`, `- Still: ${c.id}.jpg`);
      if (c.caption) lines.push("", `Suggested first line: ${c.caption}`);
    } else lines.push(`Error: ${c.error}`);
    lines.push("");
  }
  fs.writeFileSync(path.join(OUT, "clips.md"), lines.join("\n"));
  log(`${manifest.clips.filter((c) => c.ok).length}/${clips.length} clips made, ${manifest.panics} panic(s)`);
  log(`folder: ${OUT}`);
  process.exit(failed || manifest.panics ? 2 : 0);
}

main();
