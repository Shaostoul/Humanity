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
//                               [--width PX] [--shipped-assets]
//                               [--operator-config [--operator-config-path F]]
// Defaults: rig = .probe-rig, exe = target/release/HumanityOS.exe,
//           out = .probe-rig/sweeps/<timestamp>/, width = 2560
// Writes <out>/manifest.json  [{id, desc, screenshot, fps, frame_ms, expect,
//   regressions, perf_floor_fps, capture_w, capture_h, ok, error}]  and copies
//   each PNG into <out>.
//
// --operator-config: mirror the operator's LIVE graphics settings into the rig
//   before boot (%APPDATA%\HumanityOS\config.json - the same file
//   AppConfig::config_path() resolves for their install). Only visual settings
//   are copied: never identity, vault, keys, server, window/focus, or frame
//   caps, so the rig stays a portable autopilot sandbox. Fails loudly if that
//   config is missing or predates the graphics settings - it never silently
//   falls back to rig defaults while claiming to be the operator.
//
//   Every sweep records what it ran at, mirrored or not: manifest.graphics is
//   the settings the run actually used, manifest.graphics_source says where
//   they came from, and a default (non-mirrored) run also records
//   manifest.graphics_vs_operator plus prints the gap. This exists because on
//   2026-08-02 an SSAO A/B on the rig returned "nothing here, +-3% noise" for a
//   bug that separates by 6.6-9.5% at the operator's settings: the rig differed
//   on ssao_strength, veg_density, fog_density, render_distance,
//   tree_model_distance and godray_intensity, and nothing in the output said
//   so. See scripts/rig-graphics.js.
//
// --width PX (default 2560): the capture width this sweep is ALLOWED to
//   produce. Any vantage whose PNG comes back at another width is marked
//   ok:false with a loud line, because a resolution-dependent gate read at the
//   wrong width is not a baseline, it is a number that looks like one. Pass
//   --width 0 to disable the check (e.g. deliberately sweeping a small window).
//
// --shipped-assets: run against what a RELEASE BUNDLE actually contains.
//   .github/workflows/build-desktop.yml copies data/ + assets/icons/ +
//   assets/shaders/ and nothing else, so assets/models/ (62 photoscans) and
//   assets/textures/ never reach a player. The normal rig junctions the whole
//   repo assets/, so no ordinary sweep can see a shipped build's vegetation at
//   all. See setupRig() for why this rig must live OUTSIDE the repo.
//
// Exit 0 if every vantage captured at the expected width; exit 2 if any
// vantage failed to capture, came back at the wrong width, or (in
// --shipped-assets mode) the log proves the run was not actually testing a
// shipped build. The visual/perf layers still read the manifest either way.

const fs = require("fs");
const path = require("path");
const { spawn, execSync } = require("child_process");
const G = require("./rig-graphics.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const flag = (name) => args.includes(name);

// --shipped-assets MUST run from a rig OUTSIDE the repo. Measured 2026-08-02:
// two attempts at .probe-rig-ship* inside C:\Humanity, each with an assets/
// holding only icons+shaders junctions, STILL loaded all 62 photoscans.
// find_data_dir (src/lib.rs:194) walks up six parents from the exe, so it finds
// C:\Humanity\data, and that candidate wins the "repo data dir" first
// preference (its parent holds Cargo.toml). AssetManager::resolve_model_path
// (src/assets/mod.rs:149) then falls back to the data dir's PARENT -- the repo
// root -- and assets/models/ is silently re-attached. Outside the repo the
// walk-up finds nothing, so the rig's own data/ wins and the models genuinely
// go missing. %LOCALAPPDATA% is used because it is guaranteed writable, on the
// same volume as the repo (so the per-entry hard links below work), and has no
// data/ directory anywhere up its parent chain.
const SHIPPED_ASSETS = flag("--shipped-assets");
const DEFAULT_RIG = SHIPPED_ASSETS
  ? path.join(process.env.LOCALAPPDATA || require("os").tmpdir(), "HumanityOS-probe-rig-shipped")
  : path.join(REPO, ".probe-rig");
const RIG = path.resolve(opt("--rig", DEFAULT_RIG));
const EXE_SRC = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const ONLY = opt("--only", "").split(",").map((s) => s.trim()).filter(Boolean);
const KEEP_OPEN = flag("--keep-open");
// Expected capture width. See the header block: a resolution-dependent gate
// judged at the wrong width is worse than no reading at all, because it looks
// like a baseline. 2560 is the operator's own client width (2560x1440 monitor,
// 2560x1410 work area, 23 px caption -> 2560x1387 client).
const EXPECT_W = parseInt(opt("--width", "2560"), 10) || 0;
// --reload-shaders: after world entry, bump the mtime of the rig's OWN copy of
// the pbr shader parts so the engine's hot-reload picks them up. The initial
// compile uses the include_str! embedded sources, so a shader edit made BEFORE
// launch is silently ignored - which is why rig-only shader diagnostics
// (LOD tints, coverage masks) appeared to do nothing, and why the only way
// they had ever worked was editing the REPO copy, which hot-reloads straight
// into the operator's running game. With this flag the rig can test a tinted
// shader entirely inside .probe-rig/assets.
const RELOAD_SHADERS = flag("--reload-shaders");
const NO_REFRESH = flag("--no-refresh");
// --operator-config: run at the operator's live graphics settings instead of
// whatever the rig's config.json happens to hold. See prepareGraphics().
const OPERATOR_CONFIG = flag("--operator-config");
const OPERATOR_CONFIG_PATH = opt("--operator-config-path", null);

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

// A REAL directory at `link` -- never a junction. If a previous ordinary sweep
// left a whole-tree junction here, replace it: a junctioned assets/ is exactly
// the thing --shipped-assets exists to avoid.
function ensureRealDir(dir) {
  if (fs.existsSync(dir)) {
    const st = fs.lstatSync(dir);
    if (st.isSymbolicLink()) {
      fs.unlinkSync(dir); // removes the link only, never the target's contents
    } else {
      return;
    }
  }
  fs.mkdirSync(dir, { recursive: true });
}

// Mirror `src` INTO the real directory `dst`, one entry at a time: directories
// become junctions (live, free) and files become hard links (live, free, same
// volume). The point of going per-entry instead of junctioning the parent is
// that `dst` stays a real path whose PARENT is the rig, so resolve_model_path's
// data-dir-parent fallback lands in the rig and not in the repo.
function mirrorEntries(src, dst, { only = null } = {}) {
  ensureRealDir(dst);
  for (const e of fs.readdirSync(src, { withFileTypes: true })) {
    if (only && !only.includes(e.name)) continue;
    const from = path.join(src, e.name);
    const to = path.join(dst, e.name);
    if (fs.existsSync(to)) {
      // Refresh links every run: a repo file replaced by write-new+rename
      // leaves a stale hard link pointing at the old content.
      try { fs.unlinkSync(to); } catch { continue; }
    }
    try {
      if (e.isDirectory()) fs.symlinkSync(from, to, "junction");
      else fs.linkSync(from, to);
    } catch (err) {
      log(`  WARN could not link ${e.name}: ${err.message}`);
    }
  }
}

function setupRig() {
  fs.mkdirSync(RIG, { recursive: true });
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  // portable.txt: keeps identity/config/saves inside the rig and lets the dev
  // autopilot run (it refuses against a real installed identity).
  fs.writeFileSync(path.join(RIG, "portable.txt"), "probe sweep rig\n");
  // no_focus.txt: the ENGINE reads this marker beside its exe and boots
  // background (visible-inactive, no cursor grab) even when the spawner
  // forgot HUMANITY_NO_FOCUS. Agents hand-roll boots and copy this rig
  // pattern, so the marker travels with the copies (v0.1079, after the env
  // var alone failed to stop focus steals for the third time).
  fs.writeFileSync(
    path.join(RIG, "no_focus.txt"),
    "engine marker: boot background, never steal focus. delete only if you want this exe to take focus.\n"
  );
  if (SHIPPED_ASSETS) {
    if (!path.relative(REPO, RIG).startsWith("..")) {
      console.error(
        `ERROR: --shipped-assets rig must live OUTSIDE the repo, got ${RIG}.\n` +
        `  find_data_dir walks up six parents and would resolve ${path.join(REPO, "data")},\n` +
        `  whose parent re-attaches assets/models/ -- the run would silently test a dev build.`
      );
      process.exit(1);
    }
    // data/: every entry, so the rig's data/ is a real directory.
    mirrorEntries(path.join(REPO, "data"), path.join(RIG, "data"));
    // assets/: ONLY what .github/workflows/build-desktop.yml ships.
    mirrorEntries(path.join(REPO, "assets"), path.join(RIG, "assets"), {
      only: ["icons", "shaders"],
    });
    // Anything a previous non-shipped run left behind (models/, textures/) has
    // to go, or the run is testing a dev build wearing a shipped-build label.
    for (const stray of fs.readdirSync(path.join(RIG, "assets"))) {
      if (stray === "icons" || stray === "shaders") continue;
      const p = path.join(RIG, "assets", stray);
      if (fs.lstatSync(p).isSymbolicLink() || fs.lstatSync(p).isFile()) fs.unlinkSync(p);
      else fs.rmSync(p, { recursive: true, force: true });
      log(`  removed non-shipped asset ${stray} from the rig`);
    }
    log(`shipped-assets rig at ${RIG} (data/ per-entry, assets/ = icons + shaders only)`);
  } else {
    ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
    ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  }
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

// ── Graphics provenance ───────────────────────────────────────────────────
// Runs once, before boot. Two jobs, and the first one is not optional:
//   1. RECORD what this run will actually render at, into the manifest.
//   2. With --operator-config, make that the operator's live visual settings.
// A sweep with no record of its settings produced a real wrong answer on
// 2026-08-02 (see the header). Everything here is loud on purpose.
function prepareGraphics() {
  const rigCfgPath = G.rigConfigPath(RIG);
  const rec = {
    graphics: null,
    graphics_source: null,
    graphics_config_path: rigCfgPath,
    graphics_missing_keys: [],
    graphics_operator_config: null,
    graphics_mirrored: null,
    graphics_not_mirrored: null,
    graphics_vs_operator: null,
  };

  const dataDirOverride = (process.env.HUMANITY_DATA_DIR || "").trim();
  if (dataDirOverride) {
    log(`NOTE: HUMANITY_DATA_DIR=${dataDirOverride} is set, so the rig's config.json is`);
    log(`      ${rigCfgPath}, NOT ${path.join(RIG, "config.json")}.`);
  }

  let rigRead = G.readConfig(rigCfgPath);

  if (OPERATOR_CONFIG) {
    const opPath = OPERATOR_CONFIG_PATH ? path.resolve(OPERATOR_CONFIG_PATH) : G.operatorConfigPath();
    const opRead = G.readConfig(opPath);
    if (!opRead.ok) {
      console.error(
        `ERROR: --operator-config asked for the operator's live graphics settings, but\n` +
        `  ${opPath}\n` +
        `  could not be read (${opRead.error}).\n` +
        `  That file is written by the app itself. Launch HumanityOS once (just play) and exit,\n` +
        `  or pass --operator-config-path <file> if their config lives elsewhere.\n` +
        `  Refusing to sweep: a run at rig defaults labelled "operator settings" is exactly the\n` +
        `  mistake this flag exists to prevent.`
      );
      process.exit(1);
    }
    const opSnap = G.snapshotGraphics(opRead.cfg);
    if (opSnap.missing.length) {
      console.error(
        `ERROR: ${opPath} is not a live HumanityOS config: it has no ${opSnap.missing.join(", ")}.\n` +
        `  (${Object.keys(opRead.cfg).length} keys, last written ${opRead.mtime})\n` +
        `  That is almost always a stale leftover from an older build. Find the config the operator\n` +
        `  actually plays with (AppConfig::config_path in src/config.rs) and pass it with\n` +
        `  --operator-config-path, or launch the app once to write a current one.`
      );
      process.exit(1);
    }
    const m = G.mirrorOperatorGraphics(rigRead.cfg, opRead.cfg);
    fs.mkdirSync(path.dirname(rigCfgPath), { recursive: true }); // HUMANITY_DATA_DIR may not exist yet
    fs.writeFileSync(rigCfgPath, JSON.stringify(m.next, null, 2));
    log("=".repeat(72));
    log(`MIRRORING OPERATOR GRAPHICS from ${opPath} (written ${opRead.mtime})`);
    if (!rigRead.ok) {
      log(`  the rig had no config.json (${rigRead.error}); wrote one with the visual settings only.`);
      log(`  Everything else in this rig comes from the engine's built-in defaults.`);
    }
    const copiedKeys = Object.keys(m.copied);
    if (!copiedKeys.length) log("  (rig already matched the operator on every visual setting)");
    for (const k of copiedKeys) {
      log(`  ${k.padEnd(26)} ${G.fmt(m.copied[k].rig_was)} -> ${G.fmt(m.copied[k].operator)}`);
    }
    const skippedKeys = Object.keys(m.skipped);
    if (skippedKeys.length) {
      log(`  NOT mirrored (still differs from the operator, on purpose):`);
      for (const k of skippedKeys) log(`    ${k.padEnd(26)} ${m.skipped[k]}`);
    }
    if (m.unknown.length) {
      log(`  !! NEW CONFIG KEY(S) mirrored by the default-visual rule: ${m.unknown.join(", ")}`);
      log(`  !! If any of those are not graphics settings, classify them in scripts/rig-graphics.js`);
      log(`  !! (NOT_VISUAL / RECORD_ONLY / PRIVATE_KEYS) so no rig ever copies them again.`);
    }
    log("=".repeat(72));
    rigRead = G.readConfig(rigCfgPath);
    rec.graphics_source = "operator-mirrored";
    rec.graphics_operator_config = { path: opRead.path, mtime: opRead.mtime };
    rec.graphics_mirrored = m.copied;
    rec.graphics_not_mirrored = m.skipped;
  } else {
    // Default run: change nothing, but make the difference impossible to miss.
    const opRead = G.readConfig(OPERATOR_CONFIG_PATH ? path.resolve(OPERATOR_CONFIG_PATH) : G.operatorConfigPath());
    const diff = G.diffAgainstOperator(rigRead.cfg, opRead);
    // "Rig defaults" is a claim, so only make it when it is true: a rig can
    // already hold the operator's values from an earlier --operator-config run,
    // and mislabelling that would poison the warning people need to trust.
    const mirrorableDiffs = diff.ok ? Object.keys(diff.diffs).filter((k) => diff.diffs[k].mirrorable) : [];
    const matchesOperator = rigRead.ok && diff.ok && mirrorableDiffs.length === 0;
    rec.graphics_source = !rigRead.ok
      ? "unknown (rig has no config.json yet)"
      : !diff.ok
        ? "rig-defaults (operator config unreadable)"
        : matchesOperator
          ? "rig-config-matches-operator"
          : "rig-defaults";
    rec.graphics_vs_operator = diff.ok
      ? { operator_config: diff.path, operator_config_mtime: diff.mtime, differs: diff.diffs }
      : { operator_config: diff.path, unreadable: diff.reason };
    log("=".repeat(72));
    if (matchesOperator) {
      log("NOTE: not mirrored this run, but the rig config already matches the operator");
      log("      on every mirrorable visual setting. Values this sweep will render at:");
    } else {
      log("NOTE: rig graphics defaults in effect (NOT operator settings).");
    }
    for (const k of G.REQUIRED_KEYS) {
      const rigVal = rigRead.ok ? rigRead.cfg[k] : undefined;
      const opVal = diff.ok ? opRead.cfg[k] : undefined;
      const opCol = diff.ok ? `   operator ${G.fmt(opVal)}` : "";
      log(`  ${k.padEnd(22)} rig ${G.fmt(rigVal).padEnd(10)}${opCol}`);
    }
    if (diff.ok) {
      const extra = Object.keys(diff.diffs).filter((k) => !G.REQUIRED_KEYS.includes(k));
      const extraVisual = extra.filter((k) => diff.diffs[k].mirrorable);
      const extraFixed = extra.filter((k) => !diff.diffs[k].mirrorable);
      if (extraVisual.length) {
        log(`  +${extraVisual.length} more visual setting(s) differ: ${extraVisual.slice(0, 8).join(", ")}${extraVisual.length > 8 ? ", ..." : ""}`);
      }
      if (extraFixed.length) {
        log(`  ${extraFixed.length} non-visual setting(s) also differ and are never mirrored: ${extraFixed.join(", ")}`);
      }
      if (extra.length) log(`  full list in manifest.graphics_vs_operator`);
    } else {
      log(`  (could not read the operator's config to compare: ${diff.reason} at ${diff.path})`);
    }
    log("Any visual verdict from this sweep is a verdict about THOSE values.");
    if (!matchesOperator) {
      log("On 2026-08-02 an SSAO A/B read +-3% \"nothing here\" on the rig while the same");
      log("bug separates 6.6-9.5% at the operator's settings.");
    }
    log("Pass --operator-config to mirror the operator's live graphics before booting");
    log("(a match today is incidental: nothing keeps this rig in step with them).");
    log("=".repeat(72));
  }

  const snap = G.snapshotGraphics(rigRead.cfg);
  rec.graphics = rigRead.ok ? snap.values : null;
  rec.graphics_missing_keys = rigRead.ok ? snap.missing : G.REQUIRED_KEYS.slice();
  if (rec.graphics_missing_keys.length) {
    log(`  !! NO SETTINGS RECORD for ${rec.graphics_missing_keys.join(", ")}.`);
    log(`  !! ${rigRead.ok ? `${rigCfgPath} has no such key(s)` : `${rigCfgPath}: ${rigRead.error}`}.`);
    log(`  !! This sweep's manifest cannot say what it rendered at. If those settings were`);
    log(`  !! renamed in src/config.rs, update REQUIRED_KEYS in scripts/rig-graphics.js.`);
  }
  return rec;
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

  // Settings provenance, BEFORE the boot that will use them: mirrors the
  // operator's graphics when asked, and either way records what this run runs
  // at. Must come after setupRig (the rig dir + portable.txt must exist) and
  // before spawn (the engine reads config.json once at startup).
  const gfx = prepareGraphics();

  log(`launching ${path.basename(EXE_SRC)} in ${RIG}`);
  const child = spawn(path.join(RIG, "HumanityOS.exe"), [], {
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    // Never steal the operator's foreground focus (v0.1016): raw-input
    // mouse-look only reaches the FOCUSED window, so a rig launch used to
    // kill the operator's aim mid-game (their "mouse stopped aiming,
    // fixed itself when I tabbed back in" report - WASD kept working off
    // the held-key set, masking it). The engine's HUMANITY_NO_FOCUS
    // window path (v0.828) exists exactly for this; the rig just never
    // set it.
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
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
    if (RELOAD_SHADERS) {
      const sdir = path.join(RIG, "assets", "shaders", "pbr");
      const now = new Date();
      for (const f of fs.readdirSync(sdir).filter((f) => f.endsWith(".wgsl"))) {
        fs.utimesSync(path.join(sdir, f), now, now);
      }
      log("touched rig shader parts (hot-reload)");
      await sleep(4000);
    }
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
        // HOLD ALTITUDE (v0.1049). A camera_request parks the player and
        // leaves fly mode on, but gravity/buoyancy still act, so during a
        // 12 s settle a 150 m park FALLS to the sea and every "high eye"
        // ocean capture came back reading Alt 3-6 m - which is exactly why
        // the rig could not reproduce the operator's altitude-triggered
        // water artifacts. Re-park at the identical pose once the patches
        // are built, then shoot within a second: warm caches AND the
        // altitude that was asked for.
        if (v.hold_altitude) {
          clearDone("camera_done.json");
          req("camera_request.json", v.camera);
          const rehold = await waitFile("camera_done.json", 60000);
          if (!rehold || rehold.ok !== true) throw new Error(`re-park: ${JSON.stringify(rehold)}`);
          await sleep(900);
        }
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
        // Capture WIDTH x HEIGHT from the PNG IHDR (bytes 16-23). Several
        // vantage gates are resolution dependent (the backlit-canopy gate
        // reads 0.448/22.8% at 2560x1387 and 0.211/35.3% at 1280x720 on the
        // SAME build), so a manifest without the width makes those readings
        // unjudgeable after the fact.
        //
        // ROOT CAUSE OF THE 1280x720 CAPTURES, corrected 2026-08-02: it was
        // never "a second rig running concurrently". Every launch asks for
        // with_maximized(true), but winit can only apply MAXIMIZED through
        // ShowWindow, and since v0.1081 a background launch is created hidden
        // and shown by launch_focus::show_window_behind (SW_SHOWNOACTIVATE),
        // which never carried the maximize -- so the window opened at the
        // with_inner_size fallback of 1280x720 while `just play` opened
        // maximized at 2560x1387. It was deterministic, not a race: EVERY
        // background sweep captured 1280x720. Fixed in v0.1095 by sizing the
        // window to the monitor work area with SWP_NOACTIVATE.
        try {
          const hdr = Buffer.alloc(24);
          const fd = fs.openSync(srcPng, "r");
          fs.readSync(fd, hdr, 0, 24, 0);
          fs.closeSync(fd);
          rec.capture_w = hdr.readUInt32BE(16);
          rec.capture_h = hdr.readUInt32BE(20);
        } catch { /* dimensions stay absent rather than wrong */ }
        rec.expect_w = EXPECT_W || null;
        // A blind sweep must not be mistakable for a baseline.
        if (EXPECT_W && rec.capture_w !== EXPECT_W) {
          rec.ok = false;
          rec.error = `WRONG CAPTURE WIDTH: ${rec.capture_w}x${rec.capture_h}, expected ${EXPECT_W} wide`;
          log(`  !! ${rec.id}: WRONG CAPTURE WIDTH ${rec.capture_w}x${rec.capture_h} (expected ${EXPECT_W} wide).`);
          log(`  !! Resolution-dependent gates (backlit canopy, crown gap, canopy overdraw) CANNOT be`);
          log(`  !! baselined from this capture. Check that the window maximized: run.log should carry`);
          log(`  !! "focus: ... maximized-without-activating to the work area". Pass --width ${rec.capture_w} to accept it.`);
        } else {
          rec.ok = true;
          log(`  captured ${destName}  (${rec.fps} fps / ${rec.frame_ms} ms, ${rec.capture_w}x${rec.capture_h})`);
        }
      } catch (e) {
        rec.error = String(e.message || e);
        log(`  FAILED: ${rec.error}`);
      }
      results.push(rec);
    }
  } finally {
    if (!KEEP_OPEN) kill();
  }

  const logText = fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "";
  const panics = (logText.match(/PANIC/g) || []).length;

  // --shipped-assets is only meaningful if the engine actually resolved the
  // rig's data dir AND actually fell back to procedural geometry for the
  // photoscan species. Two prior attempts LOOKED like shipped runs and were
  // silently loading all 62 photoscans out of the repo, so this is asserted,
  // not assumed.
  let shipped_ok = null;
  if (SHIPPED_ASSETS) {
    const wantDataDir = path.join(RIG, "data").toLowerCase().replace(/\//g, "\\");
    const dataLine = (logText.match(/AssetManager: data directory = (.+)/) || [])[1];
    const dataDirOk =
      !!dataLine && dataLine.trim().toLowerCase().replace(/\//g, "\\") === wantDataDir;
    const fallbacks = (logText.match(/procedural fallback built \(shipped-build path\)/g) || []).length;
    shipped_ok = dataDirOk && fallbacks > 0;
    if (!dataDirOk) {
      log(`  !! NOT A SHIPPED BUILD: AssetManager resolved "${dataLine || "(no line in log)"}"`);
      log(`  !! expected "${path.join(RIG, "data")}" -- the repo's assets/models/ is still attached.`);
    }
    if (fallbacks === 0) {
      log(`  !! NOT A SHIPPED BUILD: zero "[NearTree] ... procedural fallback built (shipped-build path)" lines.`);
      log(`  !! The photoscan species loaded their glTF, so this run measured a dev build.`);
    }
    if (shipped_ok) log(`shipped-assets confirmed: rig data dir + ${fallbacks} procedural fallback lines`);
  }

  // Did the RUN change any setting it started from? The engine rewrites
  // config.json whenever a setting is applied, and any key GuiState does not
  // round-trip would quietly revert to its default mid-sweep - which would make
  // the pre-boot record a lie. Cheap to check, so check it.
  const postRun = G.readConfig(G.rigConfigPath(RIG));
  const postSnap = G.snapshotGraphics(postRun.cfg);
  let graphicsChanged = null;
  if (gfx.graphics && postRun.ok) {
    const changed = {};
    for (const [k, before] of Object.entries(gfx.graphics)) {
      if (k in postSnap.values && postSnap.values[k] !== before) {
        changed[k] = { before, after: postSnap.values[k] };
      }
    }
    if (Object.keys(changed).length) {
      graphicsChanged = changed;
      log(`  !! THE RUN CHANGED ITS OWN GRAPHICS SETTINGS mid-sweep:`);
      for (const [k, d] of Object.entries(changed)) {
        log(`  !!   ${k.padEnd(24)} ${G.fmt(d.before)} -> ${G.fmt(d.after)}`);
      }
      log(`  !! manifest.graphics records the values this sweep STARTED at; captures taken`);
      log(`  !! after the change did not use them. Check ${gfx.graphics_config_path}.`);
    }
  }

  const manifest = {
    stamp,
    exe: EXE_SRC,
    rig: RIG,
    shipped_assets: SHIPPED_ASSETS,
    shipped_ok,
    expect_w: EXPECT_W || null,
    panics,
    captured: results.filter((r) => r.ok).length,
    total: results.length,
    ...gfx,
    graphics_changed_during_run: graphicsChanged,
    vantages: results,
  };
  fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));
  log(`manifest -> ${path.join(OUT, "manifest.json")}`);
  log(`captured ${manifest.captured}/${manifest.total}, panics=${panics}`);
  // Repeat the provenance at the END too: the tail of the log is what an agent
  // or a tired operator actually reads before writing down a verdict.
  const gfxCount = Object.keys(manifest.graphics || {}).length;
  if (manifest.graphics_source === "operator-mirrored") {
    log(`graphics: OPERATOR-MIRRORED (${gfxCount} settings recorded in manifest.graphics)`);
  } else if (manifest.graphics_source === "rig-config-matches-operator") {
    log(`graphics: rig config, already equal to the operator's visual settings`);
    log(`          (${gfxCount} settings recorded in manifest.graphics)`);
  } else {
    log(`graphics: RIG DEFAULTS, not the operator's settings (${gfxCount} settings recorded in`);
    log(`          manifest.graphics). Re-run with --operator-config before calling any visual`);
    log(`          effect absent, weak, or fixed.`);
  }
  // Stable pointer to the newest sweep for the workflow + perf report.
  fs.writeFileSync(path.join(RIG, "latest-sweep.txt"), OUT);
  const clean =
    manifest.captured === manifest.total && panics === 0 && (shipped_ok === null || shipped_ok);
  process.exit(clean ? 0 : 2);
}

main().catch((e) => {
  console.error(`[sweep] fatal: ${e.stack || e}`);
  process.exit(1);
});
