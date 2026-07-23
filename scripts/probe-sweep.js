#!/usr/bin/env node
// Probe sweep: boot the release exe headless-ish in a portable rig, drive it
// through the canonical vantages in tests/visual/vantages.json, and capture
// one screenshot + the live fps at each. Foundation for BOTH the perf sweep
// (deterministic, `just perf-sweep`) and the visual-regression workflow
// (.claude/workflows/visual-sweep.md, which judges the screenshots).
//
// Everything the loop learned the hard way is baked in: kill ONLY by the pid
// we spawned, portable.txt so autopilot won't refuse (it guards the real
// identity), autopilot BEFORE any camera request (else "3D world not loaded"),
// clear done-files before each request, read the monotonic screenshot path
// back out of screenshot_done.json, and stage the DXC dlls for a ~5 s boot.
//
// Usage:
//   node scripts/probe-sweep.js [--rig DIR] [--exe PATH] [--out DIR]
//                               [--only id1,id2] [--keep-open] [--no-refresh]
// Defaults: rig = .probe-rig, exe = target/release/HumanityOS.exe,
//           out = .probe-rig/sweeps/<timestamp>/
// Writes <out>/manifest.json  [{id, desc, screenshot, fps, frame_ms, expect,
//   regressions, perf_floor_fps, ok, error}]  and copies each PNG into <out>.
//
// Exit 0 if every vantage captured; exit 2 if any vantage failed to capture
// (the visual/perf layers still read the manifest for the ones that worked).

const fs = require("fs");
const path = require("path");
const { spawn, execSync } = require("child_process");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const flag = (name) => args.includes(name);

const RIG = path.resolve(opt("--rig", path.join(REPO, ".probe-rig")));
const EXE_SRC = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const ONLY = opt("--only", "").split(",").map((s) => s.trim()).filter(Boolean);
const KEEP_OPEN = flag("--keep-open");
const NO_REFRESH = flag("--no-refresh");

const stamp = new Date()
  .toISOString()
  .replace(/[-:]/g, "")
  .replace(/\..+/, "")
  .replace("T", "-");
const OUT = path.resolve(opt("--out", path.join(RIG, "sweeps", stamp)));

const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function log(msg) {
  console.log(`[sweep] ${msg}`);
}

// ── Rig setup: a portable, self-contained copy that reads the repo's data +
// assets via NTFS junctions, so a sweep always exercises the CURRENT tree. ──
function ensureJunction(link, target) {
  if (fs.existsSync(link)) return;
  execSync(`cmd /c mklink /J "${link}" "${target}"`, { stdio: "ignore" });
}
function setupRig() {
  fs.mkdirSync(RIG, { recursive: true });
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  // portable.txt: keeps identity/config/saves inside the rig and lets the dev
  // autopilot run (it refuses against a real installed identity).
  fs.writeFileSync(path.join(RIG, "portable.txt"), "probe sweep rig\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  // Offline autopilot seed the game expects to exist for zero-click entry.
  const ap = path.join(DEBUG, "autopilot_request.json");
  if (fs.existsSync(ap)) fs.unlinkSync(ap);
  if (!fs.existsSync(EXE_SRC)) {
    console.error(`ERROR: exe not found: ${EXE_SRC}\n  build one first: cargo build --features native --release`);
    process.exit(1);
  }
  if (!NO_REFRESH) {
    fs.copyFileSync(EXE_SRC, path.join(RIG, "HumanityOS.exe"));
    // DXC dlls beside the exe drop boot from ~25 s (FXC) to ~5 s.
    for (const dll of ["dxcompiler.dll", "dxil.dll"]) {
      const s = path.join(path.dirname(EXE_SRC), dll);
      if (fs.existsSync(s)) fs.copyFileSync(s, path.join(RIG, dll));
    }
  }
}

// ── IPC helpers: write a request json, poll for its done json. ──
function clearDone(...names) {
  for (const n of names) {
    const p = path.join(DEBUG, n);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }
}
async function waitFile(name, timeoutMs, pollMs = 400) {
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
      if (/PANIC/.test(txt)) throw new Error("probe PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return true;
    }
    await sleep(1000);
  }
  throw new Error("probe did not finish booting in time");
}

async function main() {
  const spec = JSON.parse(fs.readFileSync(path.join(REPO, "tests", "visual", "vantages.json"), "utf8"));
  let vantages = spec.vantages;
  if (ONLY.length) vantages = vantages.filter((v) => ONLY.includes(v.id));
  if (!vantages.length) {
    console.error("no vantages selected");
    process.exit(1);
  }

  setupRig();
  // Fresh session: clear old screenshots + done-files + log so paths + boot
  // detection are unambiguous.
  for (const f of fs.readdirSync(DEBUG)) {
    if (/^screenshot_\d+\.png$/.test(f) || /_done\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
  fs.mkdirSync(OUT, { recursive: true });

  log(`launching ${path.basename(EXE_SRC)} in ${RIG}`);
  const child = spawn(path.join(RIG, "HumanityOS.exe"), [], {
    cwd: RIG,
    detached: true,
    stdio: "ignore",
  });
  const pid = child.pid;
  fs.writeFileSync(path.join(RIG, "probe_pid.txt"), String(pid));
  child.unref();
  let killed = false;
  const kill = () => {
    if (killed) return;
    killed = true;
    try {
      execSync(`taskkill /PID ${pid} /F`, { stdio: "ignore" });
    } catch {
      /* already gone */
    }
  };
  process.on("exit", () => { if (!KEEP_OPEN) kill(); });

  const results = [];
  try {
    log("waiting for boot...");
    await waitBoot(180000);
    log("entering world (autopilot)...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: "" });
    const ap = await waitFile("autopilot_done.json", 180000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);

    // Warm-up teleport (v0.930 async sky-sphere builds): the FIRST teleport
    // into a cold scene arrives before the heavy atmosphere shell + terrain
    // finish building on their background threads, so a screenshot then
    // catches a black/undressed frame. Park on Earth's surface and wait for
    // the builds so every real vantage below renders on arrival.
    log("warming up (async sky-sphere + terrain build)...");
    req("showcase_request.json", { time: "12.0" });
    await sleep(2000);
    clearDone("camera_done.json");
    req("camera_request.json", { body: "earth", lat: 23.0, lon: 13.0, altitude_km: 0.05, look_offset_deg: 80 });
    await waitFile("camera_done.json", 60000);
    await sleep(18000);

    for (const v of vantages) {
      log(`vantage ${v.id}`);
      const rec = {
        id: v.id,
        desc: v.desc,
        expect: v.expect,
        regressions: v.regressions || [],
        perf_floor_fps: v.perf_floor_fps ?? null,
        ok: false,
      };
      try {
        if (v.showcase) {
          req("showcase_request.json", v.showcase);
          await sleep(3500);
        }
        clearDone("camera_done.json");
        req("camera_request.json", v.camera);
        const cam = await waitFile("camera_done.json", 60000);
        if (!cam || cam.ok !== true) throw new Error(`camera: ${JSON.stringify(cam)}`);
        await sleep((v.settle_s ?? 8) * 1000);
        clearDone("screenshot_done.json");
        req("screenshot_request.json", {});
        const shot = await waitFile("screenshot_done.json", 60000);
        if (!shot || shot.ok !== true) throw new Error(`screenshot: ${JSON.stringify(shot)}`);
        const srcPng = path.join(RIG, shot.path);
        const destName = `${v.id}.png`;
        fs.copyFileSync(srcPng, path.join(OUT, destName));
        rec.screenshot = destName;
        rec.fps = typeof shot.fps === "number" ? Math.round(shot.fps * 10) / 10 : null;
        rec.frame_ms = typeof shot.frame_ms_avg === "number" ? Math.round(shot.frame_ms_avg * 10) / 10 : null;
        rec.ok = true;
        log(`  captured ${destName}  (${rec.fps} fps / ${rec.frame_ms} ms)`);
      } catch (e) {
        rec.error = String(e.message || e);
        log(`  FAILED: ${rec.error}`);
      }
      results.push(rec);
    }
  } finally {
    if (!KEEP_OPEN) kill();
  }

  const panics = fs.existsSync(LOG) ? (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length : 0;
  const manifest = {
    stamp,
    exe: EXE_SRC,
    panics,
    captured: results.filter((r) => r.ok).length,
    total: results.length,
    vantages: results,
  };
  fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));
  log(`manifest -> ${path.join(OUT, "manifest.json")}`);
  log(`captured ${manifest.captured}/${manifest.total}, panics=${panics}`);
  // Stable pointer to the newest sweep for the workflow + perf report.
  fs.writeFileSync(path.join(RIG, "latest-sweep.txt"), OUT);
  process.exit(manifest.captured === manifest.total && panics === 0 ? 0 : 2);
}

main().catch((e) => {
  console.error(`[sweep] fatal: ${e.stack || e}`);
  process.exit(1);
});
