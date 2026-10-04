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
// yours included. The rig game cannot outlive this script: Ctrl+C, Ctrl+Break
// or closing the console stops it and its ffmpeg (see launchGame).
//
// Usage:
//   node scripts/make-clips.js [--only id,id] [--exe PATH] [--out DIR]
//                              [--shots PATH] [--rig-defaults] [--keep-master-only]
//                              [--allow-other-build "<reason>"]
// THE BINARY (BUG-133): the exe must be this tree's build (the freshness gate,
// scripts/check-fresh-exe.js), or another build on purpose with
// --allow-other-build "<why>", recorded in the manifest as other_build; the rig
// copy must be byte-identical to what the gate judged, and the game starts
// through lib/game-launch.js spawnGame, which checks that copy again right before
// it spawns, sets HUMANITY_NO_HANDOFF=1 so it never hands itself to a newer
// v*_HumanityOS.exe, and reports (and stops) a hand-off an older build makes.
// Exit 0 = every clip made. 1 = refused. 2 = one or more clips failed, a panic,
//      or the run served a data file from the copy built into the exe.
//      130 = stopped by Ctrl+C, Ctrl+Break or closing the console (the game
//            and its recording are stopped with it, see launchGame).

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
// The one shared lookup for the DXC shader compiler dlls (see setupRig).
const DXC = require("./lib/dxc-dlls.js");
const G = require("./rig-graphics.js");
// The freshness gate and the boot-copy check (BUG-133).
const { runFreshGate, requireBootCopy, bootRecord, otherBuildNotice } = require("./lib/src-fingerprint.js");
// Starting the game, and what a run.log must not say (BUG-133).
const GL = require("./lib/game-launch.js");

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
// `let` so scripts/tests/make-clips.test.js can point the request files and the
// log at a scratch folder (useRigDirs) and play the game's side of them.
let DEBUG = path.join(RIG, "debug");
let LOG = path.join(RIG, "logs", "run.log");
function useRigDirs(dir) {
  DEBUG = path.join(dir, "debug");
  LOG = path.join(dir, "logs", "run.log");
}
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
// Run from main() rather than at load, so a test harness can require this file
// for launchGame and cutArgs without the refusals firing.
let FFMPEG = null;
// The freshness gate's result, and the record of a deliberate other-build run.
let FRESH = null;
let OTHER_BUILD = null;
function preflight() {
  const running = MG.listInstances();
  if (running.length) {
    refuse([
      `REFUSED: HumanityOS.exe is already running (pid ${running.map((p) => p.pid).join(", ")}).`,
      "One GPU, one instance (CLAUDE.md). Close it, then run again.",
    ]);
  }
  if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
  // This tree's build, or another on purpose (--allow-other-build "<why>").
  // Before the rig is touched, so a refusal changes nothing.
  FRESH = runFreshGate(EXE, args, { cwd: REPO });
  if (FRESH.status !== 0) refuse(["make-clips: REFUSED - see the freshness failure above. Nothing was booted."]);
  OTHER_BUILD = FRESH.other_build;
  FFMPEG = findFfmpeg();
  if (!FFMPEG) refuse(["ERROR: ffmpeg not found (PATH, Settings > Media, or C:\\Apps\\ffmpeg\\bin)."]);
  let clips = JSON.parse(fs.readFileSync(SHOTS, "utf8")).clips;
  if (ONLY) {
    const want = new Set(ONLY.split(",").map((s) => s.trim()));
    clips = clips.filter((c) => want.has(c.id));
    if (!clips.length) refuse([`ERROR: --only ${ONLY} matched no shot in ${SHOTS}`]);
  }
  return clips;
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
  // What boots is the copy, so the copy must be what the gate judged.
  requireBootCopy(path.join(RIG, "HumanityOS.exe"), FRESH, "make-clips");
  // The DXC shader compiler: without it the fallback compiler is so slow that
  // world entry outlasts every timeout (first run, 2026-09-30: four minutes of
  // silence after boot). It sits beside the exe or, for target/release, in the
  // repo root. The one shared lookup, scripts/lib/dxc-dlls.js, takes both dlls
  // from the first folder that holds both (this rig's own copy took each file
  // from wherever it turned up first, so it could pair two different
  // releases), and logs which folder it used or that it found neither.
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.(png|mp4)$/.test(f) || /_(done|request|cancel|rejected)\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
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
  try {
    fs.unlinkSync(path.join(DEBUG, name));
  } catch {} // already gone, or the game took it first
}
// An error that ends the whole run, not just the shot: nothing after it can work.
const fatal = (m) => Object.assign(new Error(m), { fatal: true });
// The rig game (launchGame), so every wait can notice it has gone. Without this
// a game that died mid-recording was waited on for the whole recording timeout,
// 17 to 23 minutes on the long shots (2026-10-02 review).
let game = null;
function checkGame() {
  if (panicCount()) throw fatal("the game panicked (see .probe-rig/clips/logs/run.log)");
  if (game && game.exit) throw fatal(`${game.describe()} (.probe-rig/clips/logs/run.log)`);
}
// Wait for `name` to appear and return its JSON; null on timeout. With
// `refusedName`, that file appearing first is the answer instead (the engine's
// "a recording is already running", which never goes in the done file).
async function waitFile(name, timeoutMs, pollMs = 250, refusedName = null) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    for (const f of refusedName ? [p, path.join(DEBUG, refusedName)] : [p]) {
      if (fs.existsSync(f)) {
        try {
          return JSON.parse(fs.readFileSync(f, "utf8"));
        } catch {} // half-written: read it on the next poll
      }
    }
    checkGame();
    await sleep(pollMs);
  }
  return null;
}
async function waitBoot(timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(LOG)) {
      const txt = fs.readFileSync(LOG, "utf8");
      if (/PANIC/.test(txt)) throw fatal("PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return true;
    }
    checkGame();
    await sleep(1000);
  }
  throw fatal("the exe did not finish booting in time");
}
const panicCount = () => (fs.existsSync(LOG) ? (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length : 0);

// What the game logs when it reads a cancel (src/engine/movie.rs,
// poll_request and take_requests; scripts/tests/make-clips.test.js checks
// these against the Rust source).
const LOG_STOPPED = /\[Movie\] cancelled at frame \d+/;
const LOG_NOTHING_TO_STOP = /\[Movie\] cancel requested with no recording running: nothing to stop/;
// The run log written since byte `from`: the lines this cancel caused, not an
// earlier shot's.
function logSince(from) {
  try {
    const fd = fs.openSync(LOG, "r");
    try {
      const n = Math.max(0, fs.fstatSync(fd).size - from);
      const buf = Buffer.alloc(n);
      fs.readSync(fd, buf, 0, n, from);
      return buf.toString("utf8");
    } finally {
      fs.closeSync(fd);
    }
  } catch {
    return "";
  }
}
const fileSize = (p) => {
  try {
    return fs.statSync(p).size;
  } catch {
    return 0;
  }
};

// Give up on a recording without leaving it running into the next shot: take
// back a request the game has not picked up, ask it to stop one it has (the
// engine closes ffmpeg, so the part written is a valid file, and puts the HUD
// back), and wait for its answer. Returns the done file, or null.
//
// After the game reads the cancel (2026-10-02 review, both were wrong):
//   - A done file that arrives says what happened, ok:true included: the
//     recording finished on its own as the timeout fired, and the caller keeps
//     it (awaitRecording).
//   - Stopping a recording blocks the game's frame while ffmpeg flushes the
//     encoder and its +faststart rewrite copies the whole master to put the
//     index first, seconds on a long one. This gave up 5 s after the cancel
//     was read and reported "no reply" for a recording still closing its
//     file, and the next shot then started against a game that could not
//     answer. Now only the log's "nothing to stop" (no recording was running)
//     ends the wait after `graceMs`; otherwise it waits up to `finalizeMs`.
// A cancel the game never reads in `takeBackMs` is taken back, so it cannot
// stop the NEXT shot's recording.
async function cancelRecording({ takeBackMs = 60000, graceMs = 5000, finalizeMs = 300000 } = {}) {
  const logFrom = fileSize(LOG);
  clearDone("record_request.json");
  req("record_cancel.json", {});
  const cancelFile = path.join(DEBUG, "record_cancel.json");
  const t0 = Date.now();
  let consumedAt = null;
  let noted = false;
  for (;;) {
    const d = await waitFile("record_done.json", 500, 250);
    if (d) {
      // Ended before the game read the cancel: take it back (a no-op once read).
      clearDone("record_cancel.json");
      return d;
    }
    if (consumedAt === null) {
      if (fs.existsSync(cancelFile)) {
        if (Date.now() - t0 > takeBackMs) {
          clearDone("record_cancel.json");
          return null;
        }
        continue;
      }
      consumedAt = Date.now();
    }
    const said = logSince(logFrom);
    if (LOG_NOTHING_TO_STOP.test(said)) {
      if (Date.now() - consumedAt > graceMs) return null;
      continue;
    }
    if (!noted && LOG_STOPPED.test(said)) {
      noted = true;
      log("recording stopped; waiting for the game to close its file");
    }
    if (Date.now() - consumedAt > finalizeMs) return null;
  }
}

// The shot's recording as the game reports it, giving up on one that has not
// answered in `timeoutMs` (cancelRecording). A recording that finished just
// as the wait ran out is the shot's result (2026-10-02 review: its ok:true
// done file arrived during the cancel and the shot failed anyway, reported as
// "cancel: undefined").
async function awaitRecording(id, timeoutMs, cancelOpts) {
  const done = await waitFile("record_done.json", timeoutMs, 500, "record_rejected.json");
  if (done) return done;
  const waited = `${Math.round(timeoutMs / 60000)} min`;
  log(`${id}: no answer in ${waited}, cancelling the recording`);
  const stopped = await cancelRecording(cancelOpts);
  if (stopped && stopped.ok === true) {
    log(`${id}: it finished as the wait ran out; keeping it`);
    return stopped;
  }
  return { ok: false, error: `no answer in ${waited} (cancel: ${stopped ? stopped.error : "no reply"})` };
}

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
// BT.709, in the numbers and in the tags, the same as the master
// (src/engine/movie.rs, BT709_TAGS and ffmpeg_args). Untagged clips were left
// to each player's guess, and players guess differently (2026-10-02 review).
// The scale reads and writes BT.709 by name, so a resize never re-converts the
// matrix whatever ffmpeg would infer, and setparams stamps the frames because
// ffmpeg 7 takes primaries and transfer from them, not from the flags.
const BT709_TAGS = ["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv"];
const BT709_STAMP = "setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv";
const scaleTo = (w, h) =>
  `scale=${w}:${h}:flags=lanczos:in_color_matrix=bt709:out_color_matrix=bt709:in_range=tv:out_range=tv,format=yuv420p,${BT709_STAMP}`;
function ff(argv, what) {
  const r = spawnSync(FFMPEG, ["-hide_banner", "-loglevel", "error", "-y", ...argv], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`${what}: ${(r.stderr || "").trim() || `ffmpeg exited ${r.status}`}`);
}
// The three cuts of one master, as ffmpeg argument lists (split out so they
// can be checked without the game).
function cutArgs(master, id, seconds, outDir) {
  const enc = ["-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p", ...BT709_TAGS, "-movflags", "+faststart", "-an"];
  const wide = path.join(outDir, `${id}-16x9.mp4`);
  return [
    { what: `${id} 16x9`, argv: ["-i", master, "-vf", `${cropTo(16, 9)},${scaleTo(1920, 1080)}`, ...enc, wide] },
    { what: `${id} 9x16`, argv: ["-i", master, "-vf", `${cropTo(9, 16)},${scaleTo(1080, 1920)}`, ...enc, path.join(outDir, `${id}-9x16.mp4`)] },
    // A JPEG is BT.601 full range by definition, and ffmpeg's JPEG path does
    // NOT convert from the wide cut's BT.709 on its own: it reread the numbers
    // as 601 (measured 2026-10-02, ffmpeg 2025-01-22: a flat (200,120,60)
    // came back (193,115,62), pure green (18,255,9)). Converted by name, they
    // come back (199,118,56) and (0,254,0), JPEG's own rounding.
    // Plain yuv420p with the full range said twice, in the scale and as
    // -color_range for the encoder: yuvj420p is deprecated and drew a warning
    // (2026-10-02, ffmpeg 2025-01-22: the same JPEG, byte for byte, without
    // it). -update 1 says one image, not a numbered sequence, which drew the
    // other warning.
    { what: `${id} still`, argv: ["-ss", String(Math.max(0, seconds / 2)), "-i", wide, "-frames:v", "1",
      "-vf", "scale=in_color_matrix=bt709:in_range=tv:out_color_matrix=bt601:out_range=pc,format=yuv420p",
      "-color_range", "pc", "-q:v", "2", "-update", "1", path.join(outDir, `${id}.jpg`)] },
  ];
}
function cut(master, id, seconds) {
  for (const c of cutArgs(master, id, seconds, OUT)) ff(c.argv, c.what);
}

// ── the game ────────────────────────────────────────────────────────────────
// Start the rig game so that it cannot outlive this script (2026-10-02 review:
// Ctrl+C on make-clips left the hidden game, and its ffmpeg, running, and the
// next run then refused to start because a HumanityOS instance existed). Two
// layers, each enough on its own:
//   1. NOT detached. Node puts every child it spawns without `detached` in a
//      Windows job object that is killed when node ends, however it ends:
//      Ctrl+C, a closed console, a hard kill. `detached: true` was what took
//      the game out of that job. ffmpeg, the game's own child, is not in the
//      job (node lets grandchildren break away), but it reads end-of-input
//      when the game dies, finishes a playable file and exits.
//   2. Ctrl+C (SIGINT), Ctrl+Break (SIGBREAK) and a closed console (SIGHUP)
//      kill the game's whole tree and the rig's processes, then exit 130.
//      The game is a windowed program, so a Ctrl+C in this console never
//      reaches it; this handler is what passes it on.
// Seen 2026-10-02 with real console events against a windowed stand-in for
// the game (wscript.exe with a hidden child): Ctrl+C, Ctrl+Break and closing
// the console each stopped the stand-in and its child, and exited 130; the old
// wiring (detached, no handlers) left both running every time. On Ctrl+C the
// handlers alone stopped both, and the job alone stopped the stand-in (its
// child, a ping that reads no input, stayed, where ffmpeg exits). A hard kill
// of node now takes the stand-in with it. ffmpeg given end-of-input by a
// killed parent wrote a valid file of every frame it had been sent.
// Background launch is unchanged: src/engine/launch_focus.rs decides it from
// HUMANITY_NO_FOCUS and the rig's no_focus.txt before it looks at the parent
// process, and the parent is node with or without `detached`. Do NOT add
// windowsHide: it starts a windowed program hidden.
function launchGame(exe, argv, { cwd, env }) {
  // spawnGame (lib/game-launch.js): the copy is checked against the judged
  // bytes right before the spawn, HUMANITY_NO_HANDOFF=1 is always set, and an
  // exit we did not ask for is watched (a hand-off an older build makes on the
  // way out is stopped and named).
  const w = GL.spawnGame(exe, argv, { fresh: FRESH, rigName: "make-clips", log, cwd, stdio: "ignore", env });
  const g = {
    pid: w.pid,
    get exit() {
      return w.exited();
    },
    describe: () => w.describe(),
  };
  let killed = false;
  g.kill = () => {
    if (killed) return;
    killed = true;
    w.expectExit(); // our own stop: not a hand-off to look for
    // /T takes ffmpeg with it. Skipped once the game has exited: its pid may
    // already belong to something else.
    if (!g.exit && g.pid) {
      try {
        execSync(`taskkill /PID ${g.pid} /T /F`, { stdio: "ignore" });
      } catch {}
    }
    killRigProcesses();
  };
  process.on("exit", g.kill);
  for (const sig of ["SIGINT", "SIGBREAK", "SIGHUP"]) {
    process.on(sig, () => {
      log(`${sig}: stopping the game and its recording`);
      g.kill();
      process.exit(130);
    });
  }
  return g;
}

// ── the run ─────────────────────────────────────────────────────────────────
async function main() {
  const clips = preflight();
  setupRig();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "make-clips",
    stamp,
    exe: EXE,
    // Which binary these clips are of (BUG-133), and the record of a
    // deliberate other-build run.
    binary: bootRecord(FRESH, path.join(RIG, "HumanityOS.exe")),
    ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
    clips: [],
    panics: 0,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));

  log(`launching ${path.basename(EXE)} in the background rig`);
  game = launchGame(path.join(RIG, "HumanityOS.exe"), [], {
    cwd: RIG,
    // (spawnGame adds HUMANITY_NO_HANDOFF: run this copy, never a newer v*_HumanityOS.exe.)
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
  });

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
        // the recording's clock must drive the clouds, the water and the wind,
        // and a lens one shot asked for ("fov") must not carry into the next.
        req("showcase_request.json", { wind: "auto", anim_clock: "auto", time_scale: "1", fov: "auto" });
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
        // final_showcase: a request that must land after the re-park, which
        // would undo it, the way probe-sweep.js sends one. The "stand" verb
        // puts the eye at a height over a lat/lon looking along a compass
        // heading, which the park cannot do: it only tilts toward north.
        if (c.final_showcase) {
          req("showcase_request.json", c.final_showcase);
          await sleep((c.final_settle_s ?? 6) * 1000);
        }
        if (c.during) req("showcase_request.json", c.during);
        else req("showcase_request.json", { time_scale: "1" });
        await sleep(300);

        const rec = Object.assign({ fps: 30 }, c.record, { out: `debug/clip_${c.id}.mp4` });
        const frames = Math.round(((rec.warmup_s ?? 2) + (rec.seconds ?? 8)) * rec.fps);
        clearDone("record_done.json");
        clearDone("record_rejected.json");
        req("record_request.json", rec);
        log(`recording ${c.id} (${rec.seconds ?? 8} s)...`);
        // Generous: a heavy frame at full size can take a second to draw.
        const timeoutMs = Math.max(180000, frames * 2500);
        const done = await awaitRecording(c.id, timeoutMs);
        req("showcase_request.json", { time_scale: "0" });
        if (done.ok !== true) throw new Error(`recording: ${done.error}`);
        if (done.ignored) log(`note: ${c.id} asked for ${done.ignored.join(", ")}, which a path overrides`);

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
        if (e.fatal) break;
      }
      save();
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
  game.kill();
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
  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  if (manifest.builtin_data.length) log(GL.builtinDataRefusal(manifest.builtin_data));
  process.exit(failed || manifest.panics || manifest.builtin_data.length ? 2 : 0);
}

if (require.main === module) main();
else module.exports = { launchGame, cutArgs, BT709_TAGS, cancelRecording, awaitRecording, useRigDirs, findFfmpeg, LOG_STOPPED, LOG_NOTHING_TO_STOP };
