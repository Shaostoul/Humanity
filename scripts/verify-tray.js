#!/usr/bin/env node
// verify-tray: proves in the RUNNING game the hidden-window chain of notifications rung 2, part 2
// (`just verify-tray`; docs/design/notifications.md, "The tray"): closing the window into the
// tray hides it, leaves the world saved and every call and voice room left, draws no frame while
// hidden, comes back from the icon, and quits saved both from the tray's Quit and from `taskkill`
// without /F while hidden. The critic's review of part 2 (2026-10-31) found the chain had no
// committed check: the first agent's rig runs were by hand.
//
// TWO BOOTS of the release binary in its own portable rig (.probe-rig/tray: background, never
// focused, silent, no server), each with HUMANITY_TRAY=1 (a script's instance has no tray icon
// otherwise; HUMANITY_TRAY_CLICKS is NOT set, so it never serves clicks on the operator's notices,
// docs/ai/game-boots.md). Everything is driven through debug/tray_request.json (src/engine/tray.rs
// `poll_dev_request`), the same code each click runs:
//   boot 1   world entry by the dev autopilot; `setting` closing-into-the-tray on, desktop notices
//            off (no notice on the operator's screen); `voice_close`, a stand-in voice room and
//            call set and the close button pressed in one step (no frame between, so no voice
//            session and no microphone ever starts); a wait; `icon`; `close` again; the tray's
//            `menu` Quit, while hidden: the game must exit by itself, saved.
//   boot 2   (no world) closing-into-the-tray on, `close`, then `taskkill /PID` without /F: the game
//            must exit by itself within 15 s, saved, never forced.
// scripts/lib/tray-judge.js judges it (scripts/tests/tray-judge.test.js holds the judge, one broken
// link at a time).
//
// ONE GPU (CLAUDE.md): waits (up to 60 min, --wait-min N; --no-wait to refuse at once) while any
// HumanityOS runs, through the machine guard every rig shares; a behaviour rig (scripts/lib/
// rig-needs.js), so beside one of the operator's GPU-heavy programs it boots in low impact.
//
// Usage:
//   node scripts/verify-tray.js [--exe PATH] [--timeout-min N] [--no-wait] [--wait-min N]
//                               [--allow-other-build "<reason>"]
//   node scripts/verify-tray.js --dry-verdict <manifest.json>   re-judge, nothing booted
// Exit 0 = every link held. 1 = refused (nothing booted). 2 = a link broke, or a panic.

"use strict";

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const MG = require("./lib/machine-guard.js");
const RN = require("./lib/rig-needs.js");
const DXC = require("./lib/dxc-dlls.js");
const { runFreshGate, requireBootCopy, bootRecord, otherBuildNotice } = require("./lib/src-fingerprint.js");
const GL = require("./lib/game-launch.js");
const TJ = require("./lib/tray-judge.js");
const WR = require("./lib/write-request.js");

const RIG_NAME = "verify-tray";
const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const NO_WAIT = args.includes("--no-wait");
const WAIT_MIN = Number(opt("--wait-min", String(MG.WAIT_MIN_DEFAULT)));
const DRY = opt("--dry-verdict", null);
const TIMEOUT_MS = Number(opt("--timeout-min", "12")) * 60 * 1000;
const RIG = path.join(REPO, ".probe-rig", "tray");
const RUNS = path.join(RIG, "runs");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
/// How long the rig waits with the window hidden before it asks whether frames stopped.
const HIDDEN_WAIT_MS = 3000;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[tray] ${m}`);
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}
function printVerdict(run, prefix) {
  const result = TJ.judgeTray(run);
  const lines = TJ.verdictLines(result);
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
if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
const FRESH = runFreshGate(EXE, args, { cwd: REPO });
if (FRESH.status !== 0) refuse([`${RIG_NAME}: REFUSED - see the freshness failure above. Nothing was booted.`]);
const OTHER_BUILD = FRESH.other_build;
const BOOT_PLAN = RN.planOrRefuse("verify-tray", { log });
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
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-tray rig\n");
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  for (const d of ["saves", "backups"]) fs.rmSync(path.join(RIG, d), { recursive: true, force: true });
  fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  requireBootCopy(path.join(RIG, "HumanityOS.exe"), FRESH, RIG_NAME);
  DXC.copyDxcDlls({ exe: EXE, repo: REPO, dest: RIG, log });
}
function clearDebug() {
  for (const f of fs.readdirSync(DEBUG)) {
    if (/_done\.json$/.test(f) || WR.isRequestLeftover(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}
const logText = () => (fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "");
const panicCount = () => (logText().match(/PANIC/g) || []).length;
async function waitFile(name, timeoutMs) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(p)) {
      try {
        return JSON.parse(fs.readFileSync(p, "utf8"));
      } catch {}
    }
    await sleep(100);
  }
  return null;
}
/** One tray request, answered in tray_done.json (the hidden window's tick answers it too). */
async function tray(body) {
  const done = path.join(DEBUG, "tray_done.json");
  if (fs.existsSync(done)) fs.unlinkSync(done);
  WR.writeRequest(path.join(DEBUG, "tray_request.json"), body);
  const d = await waitFile("tray_done.json", 20000);
  if (!d) throw new Error(`no answer to ${JSON.stringify(body)} in 20 s`);
  if (d.error) throw new Error(`${JSON.stringify(body)}: ${d.error}`);
  return d;
}
async function waitBoot(game, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    const txt = logText();
    if (/PANIC/.test(txt)) throw new Error("PANIC during boot (see run.log)");
    if (/Cloud noise volumes generated/.test(txt)) return;
    if (game.exited()) throw new Error(`before it finished booting, ${game.describe()}`);
    await sleep(1000);
  }
  throw new Error("the exe did not finish booting in time");
}
async function waitExit(game, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (game.exited()) return true;
    await sleep(200);
  }
  return false;
}
function start() {
  clearDebug();
  const game = GL.spawnGame(path.join(RIG, "HumanityOS.exe"), [], {
    fresh: FRESH,
    rigName: "verify-tray",
    log,
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    // The tray, asked for; clicks on notices NOT (HUMANITY_TRAY_CLICKS unset).
    env: { ...process.env, HUMANITY_NO_FOCUS: "1", HUMANITY_TRAY: "1", HUMANITY_TRAY_CLICKS: "" },
    lowImpact: BOOT_PLAN.lowImpact,
  });
  game.child.unref();
  return game;
}
function forceKill(game) {
  game.expectExit();
  try {
    execSync(`taskkill /PID ${game.child.pid} /T /F`, { stdio: "ignore" });
  } catch {}
}

async function main() {
  setupRig();
  const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
  const OUT = path.join(RUNS, stamp);
  fs.mkdirSync(OUT, { recursive: true });
  const run = { kind: RIG_NAME, stamp, exe: EXE, binary: bootRecord(FRESH, path.join(RIG, "HumanityOS.exe")), ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}), panics: 0, builtin_data: [], error: null };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(run, null, 2));
  let game = null;
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min`);
    run.error = "timeout";
    save();
    if (game) forceKill(game);
    process.exit(2);
  }, TIMEOUT_MS);
  try {
    // ── boot 1 ──
    log("boot 1: launching, then world entry");
    game = start();
    await waitBoot(game, 240000);
    const apDone = path.join(DEBUG, "autopilot_done.json");
    if (fs.existsSync(apDone)) fs.unlinkSync(apDone);
    WR.writeRequest(path.join(DEBUG, "autopilot_request.json"), {});
    const ap = await waitFile("autopilot_done.json", 240000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    await sleep(4000);
    run.first = await tray({ action: "state" });
    await tray({ action: "setting", close_to_tray: true, desktop: false });
    const before = logText().length;
    const hid = await tray({ action: "voice_close" });
    run.hide = { done: hid, log: logText().slice(before) };
    log(`closed: hidden ${hid.hidden}, page ${hid.page}, voice ${JSON.stringify(hid.voice)}`);
    await sleep(HIDDEN_WAIT_MS);
    run.hidden_later = await tray({ action: "state" });
    log(`hidden ${HIDDEN_WAIT_MS} ms: no frame for ${run.hidden_later.ms_since_last_frame} ms`);
    await tray({ action: "icon" });
    await sleep(1500);
    run.shown = await tray({ action: "state" });
    log(`shown: last frame ${run.shown.ms_since_last_frame} ms ago`);
    await tray({ action: "close" });
    const quitFrom = logText().length;
    // Quit from the tray's menu while hidden: the answer may or may not land before it exits.
    WR.writeRequest(path.join(DEBUG, "tray_request.json"), { action: "menu", id: "quit" });
    const exited = await waitExit(game, 20000);
    run.quit = { exited, log: logText().slice(quitFrom) };
    log(`Quit while hidden: ${exited ? "it exited by itself" : "STILL RUNNING after 20 s"}`);
    run.panics += panicCount();
    run.builtin_data.push(...GL.builtinDataLines(logText()));
    fs.copyFileSync(LOG, path.join(OUT, "run-boot1.log"));
    if (!exited) forceKill(game);
    save();

    // ── boot 2 ──
    log("boot 2: launching (no world), closing into the tray, then taskkill without /F");
    game = start();
    await waitBoot(game, 240000);
    await sleep(2000);
    await tray({ action: "setting", close_to_tray: true, desktop: false });
    const closed = await tray({ action: "close" });
    const tkFrom = logText().length;
    try {
      execSync(`taskkill /PID ${game.child.pid}`, { stdio: "ignore" });
    } catch {}
    const graceful = await waitExit(game, 15000);
    run.taskkill = { hidden_first: closed.hidden === true && closed.visible === false, exited_gracefully: graceful, log: logText().slice(tkFrom) };
    log(`taskkill while hidden: ${graceful ? "it closed by itself" : "it had to be FORCED"}`);
    run.panics += panicCount();
    run.builtin_data.push(...GL.builtinDataLines(logText()));
    fs.copyFileSync(LOG, path.join(OUT, "run-boot2.log"));
    if (!graceful) forceKill(game);
  } catch (e) {
    log(`ERROR: ${e.message}`);
    run.error = e.message;
    if (game && !game.exited()) forceKill(game);
  }
  clearTimeout(watchdog);
  save();
  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  // A data file served from the copy built into the exe is not this tree's data: the run fails.
  const builtin = GL.builtinDataRefusal(run.builtin_data);
  if (builtin) for (const l of builtin) log(l);
  const result = printVerdict(run, run.error ? `(the run stopped early: ${run.error}) ` : "");
  log(`evidence: ${path.relative(REPO, OUT)} (manifest.json, run-boot1.log, run-boot2.log)`);
  process.exit(result.ok && !run.error && !builtin ? 0 : 2);
}

main();
