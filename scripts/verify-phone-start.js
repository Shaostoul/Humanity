#!/usr/bin/env node
// verify-phone-start: proves on this PC what the phone app's start does (Android rung A1, 2026-11-20;
// docs/design/platforms.md; `just verify-phone-start`): a phone starts PAGES-ONLY (the device and
// the window's surface, none of the 3D world's pipelines, the cloud bake or the star sky) and draws a
// frame only when something changed (engine/renderer_slot.rs, engine/redraw.rs). The phone build
// cannot run here, so the phone preview (HUMANITY_PHONE_PREVIEW=1, the same switch the pages' phone
// layout uses) stands in for it.
//
// FOUR BOOTS of the release binary in its own portable rig (.probe-rig/phone-start: background,
// never focused, silent, no server), each closed with taskkill without /F so it saves:
//   desktop      plain: the first frame's time, and wgpu's pipeline count with the world built.
//   phone        the preview: the first frame's time, a visit to several pages through the dev
//                request (debug/showcase_request.json {"page": ...}), then an idle stretch; wgpu's
//                own count of the pipelines it holds, any stray reach for the world, the frames an
//                idle window drew.
//   before       the preview with HUMANITY_REDRAW_ALWAYS=1 (every frame, as before 2026-11-20): the
//                idle window's count for comparison.
//   pages-first  HUMANITY_PAGES_FIRST=1, then the world entered by the dev autopilot: the world's
//                renderer built on demand, and the world drawn without a panic.
// scripts/lib/phone-start-judge.js judges the logs (scripts/tests/phone-start-judge.test.js holds the
// judge, one rule broken at a time). The verdict prints the start times and the idle frame counts.
//
// ONE GPU (CLAUDE.md): waits (up to 60 min, --wait-min N; --no-wait to refuse at once) while any
// HumanityOS runs, through the machine guard every rig shares. A timing rig (scripts/lib/
// rig-needs.js): it measures the start time, so it waits for the operator's GPU-heavy programs too.
//
// Usage:
//   node scripts/verify-phone-start.js [--exe PATH] [--timeout-min N] [--no-wait] [--wait-min N]
//                                      [--idle-s N] [--only desktop,phone,before,pages_first]
//                                      [--allow-other-build "<reason>"]
//   node scripts/verify-phone-start.js --dry-verdict <manifest.json>   re-judge, nothing booted
// Exit 0 = every check held. 1 = refused (nothing booted). 2 = a check failed, or a panic.

"use strict";

const fs = require("fs");
const path = require("path");
const PLAT = require("./lib/platform.js");
const MG = require("./lib/machine-guard.js");
const RN = require("./lib/rig-needs.js");
const DXC = require("./lib/dxc-dlls.js");
const { runFreshGate, requireBootCopy, bootRecord, otherBuildNotice } = require("./lib/src-fingerprint.js");
const GL = require("./lib/game-launch.js");
const PJ = require("./lib/phone-start-judge.js");
const WR = require("./lib/write-request.js");

const RIG_NAME = "verify-phone-start";
const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const EXE = path.resolve(opt("--exe", PLAT.builtExe(REPO)));
const NO_WAIT = args.includes("--no-wait");
const WAIT_MIN = Number(opt("--wait-min", String(MG.WAIT_MIN_DEFAULT)));
const DRY = opt("--dry-verdict", null);
const TIMEOUT_MS = Number(opt("--timeout-min", "15")) * 60 * 1000;
/** The idle stretch: two of the game's ten-second count windows must fall wholly inside it. */
const IDLE_MS = Number(opt("--idle-s", "24")) * 1000;
/** The boots to make (all four unless --only names some); the judge judges only these. */
const ALL_BOOTS = ["desktop", "phone", "before", "pages_first"];
const ONLY = opt("--only", null);
const BOOTS = ONLY ? ONLY.split(",").map((b) => b.trim()).filter(Boolean) : ALL_BOOTS;
const RIG = path.join(REPO, ".probe-rig", "phone-start");
const RUNS = path.join(RIG, "runs");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
/** The pages the phone boot visits (gui/dispatch.rs page ids), each a chance for a stray reach. */
const PAGES = ["chat", "settings", "play", "apps", "files", "library", "tasks", "profile", "music", "maps", "watch", "dev"];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[phone-start] ${m}`);
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}
function printVerdict(run, prefix) {
  const result = PJ.judgePhoneStart(run);
  const lines = PJ.verdictLines(result);
  lines[lines.length - 1] = prefix + lines[lines.length - 1];
  console.log("");
  console.log("-".repeat(100));
  for (const l of lines) console.log(l);
  console.log("-".repeat(100));
  return result;
}

if (DRY) {
  const p = path.resolve(DRY);
  if (!fs.existsSync(p)) refuse([`--dry-verdict: no such manifest: ${p}`]);
  const run = JSON.parse(fs.readFileSync(p, "utf8"));
  const result = printVerdict(run, "DRY VERDICT (nothing was booted): ");
  process.exit(result.ok ? 0 : 2);
}

if (!(WAIT_MIN > 0)) refuse([`${RIG_NAME}: REFUSED - --wait-min must be a number of minutes above 0. Nothing was booted.`]);
if (!(IDLE_MS >= 21000)) refuse([`${RIG_NAME}: REFUSED - --idle-s must be at least 21 (two ten-second count windows). Nothing was booted.`]);
if (BOOTS.some((b) => !ALL_BOOTS.includes(b))) refuse([`${RIG_NAME}: REFUSED - --only takes ${ALL_BOOTS.join(", ")}. Nothing was booted.`]);
if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
const FRESH = runFreshGate(EXE, args, { cwd: REPO });
if (FRESH.status !== 0) refuse([`${RIG_NAME}: REFUSED - see the freshness failure above. Nothing was booted.`]);
const OTHER_BUILD = FRESH.other_build;
const BOOT_PLAN = RN.planOrRefuse("verify-phone-start", { log });
{
  const running = MG.blockingInstances(BOOT_PLAN.own);
  if (running.length && NO_WAIT) {
    refuse([`REFUSED: HumanityOS is already running (pid ${running.map((p) => p.pid).join(", ")}), and --no-wait. One GPU, one instance (CLAUDE.md).`]);
  }
  const w = MG.waitForFree({ own: { gameOnly: true, ...BOOT_PLAN.own }, label: RIG_NAME, rig: RIG_NAME, timeoutMs: WAIT_MIN * 60 * 1000, log });
  if (!w.free) MG.refuseBusy(w, { rig: RIG_NAME, hint: ["What to do: wait for it, then run again; --wait-min N waits longer."] });
}

// ── the rig folder ──────────────────────────────────────────────────────────────────────────────
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
function setupRig() {
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-phone-start rig\n");
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  for (const d of ["saves", "backups"]) fs.rmSync(path.join(RIG, d), { recursive: true, force: true });
  fs.copyFileSync(EXE, path.join(RIG, PLAT.EXE_NAME));
  requireBootCopy(path.join(RIG, PLAT.EXE_NAME), FRESH, RIG_NAME);
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
}
function clearDebug() {
  for (const f of fs.readdirSync(DEBUG)) {
    if (/_done\.json$/.test(f) || WR.isRequestLeftover(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}
const logText = () => (fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "");
async function waitFor(re, game, timeoutMs, what) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    const txt = logText();
    if (/PANIC/.test(txt)) throw new Error(`PANIC while waiting for ${what} (see run.log)`);
    const m = txt.match(re);
    if (m) return m;
    if (game.exited()) throw new Error(`before ${what}, ${game.describe()}`);
    await sleep(250);
  }
  throw new Error(`no ${what} in ${Math.round(timeoutMs / 1000)} s`);
}
async function waitFile(name, timeoutMs) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(p)) {
      try {
        return JSON.parse(fs.readFileSync(p, "utf8"));
      } catch {}
    }
    await sleep(200);
  }
  return null;
}
async function waitExit(game, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (game.exited()) return true;
    await sleep(200);
  }
  return false;
}
function start(env) {
  clearDebug();
  const game = GL.spawnGame(path.join(RIG, PLAT.EXE_NAME), [], {
    fresh: FRESH,
    rigName: "verify-phone-start",
    log,
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    env: { ...process.env, HUMANITY_NO_FOCUS: "1", HUMANITY_PHONE_PREVIEW: "", HUMANITY_PAGES_FIRST: "", HUMANITY_REDRAW_ALWAYS: "", ...env },
    lowImpact: BOOT_PLAN.lowImpact,
  });
  game.child.unref();
  return game;
}
function forceKill(game) {
  game.expectExit();
  try {
    PLAT.killTree(game.child.pid, { force: true });
  } catch {}
}
/** Close the game the way the operator's tools do (taskkill without /F: it saves), forced only if
 *  it is still running 15 s later. */
async function close(game) {
  try {
    PLAT.killTree(game.child.pid, { force: false, tree: false });
  } catch {}
  if (!(await waitExit(game, 15000))) forceKill(game);
}
const countRedraw = () => (logText().match(/\[Redraw\] /g) || []).length;
/** Wait until `n` more ten-second count windows were logged after `from` lines. */
async function waitWindows(game, from, n, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (countRedraw() >= from + n) return;
    if (game.exited()) throw new Error(`while idle, ${game.describe()}`);
    await sleep(500);
  }
  throw new Error(`fewer than ${n} count windows in ${Math.round(timeoutMs / 1000)} s`);
}

async function main() {
  setupRig();
  const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
  const OUT = path.join(RUNS, stamp);
  fs.mkdirSync(OUT, { recursive: true });
  const run = { kind: RIG_NAME, stamp, exe: EXE, binary: bootRecord(FRESH, path.join(RIG, PLAT.EXE_NAME)), ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}), boots: {}, ...(ONLY ? { only: BOOTS } : {}), builtin_data: [], error: null };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(run, null, 2));
  let game = null;
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min`);
    run.error = "timeout";
    save();
    if (game) forceKill(game);
    process.exit(2);
  }, TIMEOUT_MS);
  const keep = (name, extra = {}) => {
    const text = logText();
    run.boots[name] = { log: text, ...extra };
    run.builtin_data.push(...GL.builtinDataLines(text));
    fs.copyFileSync(LOG, path.join(OUT, `run-${name}.log`));
  };
  const firstFrame = /\[Boot\] first frame drawn (\d+) ms/;
  try {
    // ── desktop ──
    if (BOOTS.includes("desktop")) {
    log("desktop: a plain start, to its first frame");
    game = start({});
    await waitFor(firstFrame, game, 240000, "the first frame");
    await sleep(1500);
    await close(game);
    keep("desktop");
    }

    // ── phone ──
    if (BOOTS.includes("phone")) {
    log("phone: the preview, a visit to each page, then idle");
    game = start({ HUMANITY_PHONE_PREVIEW: "1" });
    await waitFor(firstFrame, game, 240000, "the first frame");
    const opened = [];
    for (const page of PAGES) {
      // Each request is answered by one "Showcase: page" line: wait for this one before the next.
      const seen = (logText().match(/Showcase: page /g) || []).length;
      WR.writeRequest(path.join(DEBUG, "showcase_request.json"), { page });
      const t0 = Date.now();
      while (Date.now() - t0 < 20000 && (logText().match(/Showcase: page /g) || []).length <= seen) await sleep(100);
      const line = (logText().match(/Showcase: page [^\r\n]*/g) || []).slice(seen)[0] || "no answer";
      opened.push(`${page}: ${line}`);
      await sleep(600);
    }
    log(`phone: pages visited: ${opened.join(" | ")}`);
    // The idle stretch on the Play page, where a phone lands (the Dev, Maps and Watch pages draw on
    // purpose while open: their live numbers, the orrery, a video).
    WR.writeRequest(path.join(DEBUG, "showcase_request.json"), { page: "play" });
    await sleep(1500);
    await sleep(1000);
    const idleFrom = countRedraw();
    log(`phone: idle ${IDLE_MS / 1000} s (no request, no input)`);
    await waitWindows(game, idleFrom, 2, IDLE_MS + 20000);
    await close(game);
    keep("phone", { idle_from: idleFrom, pages: opened });
    }

    // ── before ──
    if (BOOTS.includes("before")) {
    log("before: the preview drawing every frame (HUMANITY_REDRAW_ALWAYS=1), idle");
    game = start({ HUMANITY_PHONE_PREVIEW: "1", HUMANITY_REDRAW_ALWAYS: "1" });
    await waitFor(firstFrame, game, 240000, "the first frame");
    const beforeFrom = countRedraw();
    await waitWindows(game, beforeFrom, 2, IDLE_MS + 20000);
    await close(game);
    keep("before");
    }

    // ── pages-first ──
    if (BOOTS.includes("pages_first")) {
    log("pages-first: a desktop started pages-only, then the world entered");
    game = start({ HUMANITY_PAGES_FIRST: "1" });
    await waitFor(firstFrame, game, 240000, "the first frame");
    await sleep(1500);
    WR.writeRequest(path.join(DEBUG, "autopilot_request.json"), {});
    const entered = await waitFile("autopilot_done.json", 300000);
    // One more count window after entry, so the count with the world built is in the log.
    const pfFrom = countRedraw();
    await waitWindows(game, pfFrom, 1, 30000).catch(() => {});
    await close(game);
    keep("pages_first", { entered });
    }
  } catch (e) {
    log(`ERROR: ${e.message}`);
    run.error = e.message;
    if (game && !game.exited()) forceKill(game);
    if (fs.existsSync(LOG)) fs.copyFileSync(LOG, path.join(OUT, "run-error.log"));
  }
  clearTimeout(watchdog);
  save();
  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  const builtin = GL.builtinDataRefusal(run.builtin_data);
  if (builtin) for (const l of builtin) log(l);
  const result = printVerdict(run, run.error ? `(the run stopped early: ${run.error}) ` : "");
  log(`evidence: ${path.relative(REPO, OUT)} (manifest.json, run-<boot>.log)`);
  process.exit(result.ok && !run.error && !builtin ? 0 : 2);
}

main();
