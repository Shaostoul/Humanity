#!/usr/bin/env node
// boot-timing: measure how long the app takes to become playable, and say
// WHERE the time went, several runs at a time.
//
// Why this exists. The engine already instruments its own boot: `[BootPhase]`
// lines from the renderer, one `[Pipelines]` line per pipeline build, and a
// consolidated `=== BOOT TIMING ===` summary plus `debug/boot_timing.json`
// written the moment the 3D world is ready. What did not exist was a way to
// collect those over REPEATED boots on a quiet machine, which is the only way
// a boot number means anything: a single boot is contaminated by whatever else
// the machine was doing, and the spread between runs is usually wider than the
// win somebody is claiming.
//
// It boots the SAME portable rig the other gates use (portable.txt so the
// throwaway autopilot identity can never touch the operator's real install,
// no_focus.txt so the window never steals focus), enters the world through the
// autopilot, reads the numbers, and kills the instance. One instance at a time,
// through the shared one-machine guard.
//
// Usage:
//   node scripts/boot-timing.js [--runs N] [--exe PATH] [--rig DIR]
//                               [--label NAME] [--out FILE]
//                               [--cold] [--keep-cache] [--timeout-min N]
//                               [--allow-other-build "<reason>"]
//
//   --runs N        how many boots to average (default 3).
//   --label NAME    a name for this arm, written into the JSON (e.g. "cold").
//   --cold          delete the rig's on-disk caches before EVERY run.
//                   NOTE what this does NOT cover: there is no persistent
//                   pipeline cache, because wgpu 24 offers one only on
//                   Vulkan and Windows runs DX12 (the run log says so:
//                   "[Pipelines] adapter offers a persistent pipeline cache").
//                   What remains outside our reach is the OS file cache and
//                   the GPU driver's own shader cache, which is why the FIRST
//                   boot of a session is reliably the slowest one and why
//                   these runs are reported as a median with the spread.
//   --keep-cache    leave the caches alone (the default).
//   --out FILE      where to write the collected JSON (default: under the rig).
//   --allow-other-build "<reason>"
//                   time a build that is NOT this tree's on purpose (an archive
//                   as the A arm of an A/B). Without it the freshness gate
//                   (scripts/check-fresh-exe.js) refuses anything but this
//                   tree's build; with it the JSON records other_build. Either
//                   way the rig copy must be byte-identical to what the gate
//                   judged (checked again right before every spawn, by
//                   lib/game-launch.js spawnGame), and every boot runs with
//                   HUMANITY_NO_HANDOFF=1, so a timing is always of the binary
//                   named (BUG-133). A boot that exits early is reported as one,
//                   naming (and stopping) a hand-off if an older build made one.
//
// Exit 0 = every run produced a boot_timing.json with zero panics and served no
//          data file from the copy built into the exe (BUILT-IN DATA).
// Exit 1 = refused before any boot (no exe, no shader-compiler dll pair, the
//          freshness gate, or a rig copy that is not the judged exe).
// Exit 2 = a run failed to boot, timed out, or panicked.

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

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};

const RUNS = Math.max(1, Number(opt("--runs", "3")));
const EXE_SRC = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
// Its own rig, nested inside .probe-rig so the existing gitignore covers it and
// so a concurrent probe-sweep never fights this run over the same exe copy.
const RIG = path.resolve(opt("--rig", path.join(REPO, ".probe-rig", "boot")));
const LABEL = opt("--label", "run");
const COLD = args.includes("--cold");
const TIMEOUT_MS = Number(opt("--timeout-min", "6")) * 60 * 1000;
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const OUT = path.resolve(opt("--out", path.join(RIG, `boot-timing-${LABEL}.json`)));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[boot-timing] ${m}`);
// The freshness gate's result (set before the rig is touched) and the record of
// a deliberate other-build run.
let FRESH = null;
let OTHER_BUILD = null;

// ── Rig setup ─────────────────────────────────────────────────────────────
// Same shape as probe-sweep's rig, deliberately: a portable sandbox whose
// data/ and assets/ are junctions back to the repo, so what boots here is the
// repo's content without a copy to go stale.
function ensureJunction(link, target) {
  try {
    const st = fs.lstatSync(link);
    if (st.isSymbolicLink() || st.isDirectory()) return;
  } catch {
    /* not there yet */
  }
  execSync(`cmd /c mklink /J "${link}" "${target}"`, { stdio: "ignore" });
}

function killRigProcesses() {
  const rigExe = path.join(RIG, "HumanityOS.exe").replace(/'/g, "''");
  try {
    execSync(
      `powershell -NoProfile -Command "Get-Process HumanityOS -ErrorAction SilentlyContinue | ` +
        `Where-Object { $_.Path -eq '${rigExe}' } | Stop-Process -Force"`,
      { stdio: "ignore" }
    );
  } catch {
    /* nothing of ours was running */
  }
}

function setupRig() {
  if (!fs.existsSync(EXE_SRC)) {
    console.error(
      `ERROR: exe not found: ${EXE_SRC}\n  build one first: cargo build --features native --release`
    );
    process.exit(1);
  }
  fs.mkdirSync(RIG, { recursive: true });
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  // portable.txt keeps identity/config/saves inside the rig, and is what lets
  // the dev autopilot run at all (it refuses against a real installed identity).
  fs.writeFileSync(path.join(RIG, "portable.txt"), "boot-timing rig\n");
  // no_focus.txt: the engine reads this marker beside its exe and boots
  // background even if the spawner forgot HUMANITY_NO_FOCUS.
  fs.writeFileSync(
    path.join(RIG, "no_focus.txt"),
    "engine marker: boot background, never steal focus.\n"
  );
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  killRigProcesses();
  fs.copyFileSync(EXE_SRC, path.join(RIG, "HumanityOS.exe"));
  // Every run boots this copy, so it must be what the gate judged.
  requireBootCopy(path.join(RIG, "HumanityOS.exe"), FRESH, "boot-timing");
  // DXC beside the exe: without it the DX12 backend falls back to FXC and
  // every shader compile takes four times as long, which looks exactly like a
  // regression. Copying it is not optional for a boot measurement.
  //
  // The pair comes from beside the exe or else the repo root, through the one
  // shared lookup, scripts/lib/dxc-dlls.js, which logs which folder it used.
  // This check used to look only beside the exe, and target/release (the
  // default exe's folder) holds no dlls, so a default run stopped here with a
  // message telling you to copy them "from target/release", which had none.
  const dxc = DXC.copyDxcDlls({ exe: EXE_SRC, repo: REPO, dest: RIG, log });
  if (!dxc.found) {
    // Refused even when the rig folder still holds a pair from an earlier
    // run (the line above says so when it does): a boot number is only worth
    // comparing when this run copied the pair itself and knows where it came
    // from.
    console.error(
      "ERROR: no folder searched holds both dxcompiler.dll and dxil.dll (the line above\n" +
        "  lists what each one holds). Without a pair this run copied itself, the boot\n" +
        "  would use FXC (about four times slower) or a leftover pair of unknown origin,\n" +
        "  so its numbers could not be compared. Put both dlls in the repo root (from the\n" +
        "  Windows SDK bin folder or a DirectXShaderCompiler release) and re-run."
    );
    process.exit(1);
  }
}

/// The engine's own on-disk caches under the rig. Keep this list in step with
/// any new one the boot path grows. It cannot reach the OS file cache or the
/// GPU driver's shader cache, so `--cold` means "our caches are cold", not
/// "nothing anywhere is warm".
function clearCaches() {
  const cachePaths = [path.join(RIG, "cache"), path.join(RIG, "shader_cache")];
  for (const p of cachePaths) fs.rmSync(p, { recursive: true, force: true });
}

// ── One boot ──────────────────────────────────────────────────────────────
async function oneRun(n) {
  if (COLD) clearCaches();
  for (const f of ["boot_timing.json", "autopilot_done.json", "autopilot_request.json"]) {
    fs.rmSync(path.join(DEBUG, f), { force: true });
  }
  fs.rmSync(LOG, { force: true });

  // ONE MACHINE: a boot beside another instance or a release build measures
  // the other process as much as this one.
  const waited = MG.waitForFree({ label: `boot ${n}`, timeoutMs: 40 * 60 * 1000 });
  if (!waited.free) {
    console.error("ERROR: the machine never went quiet; a boot measured beside a build is not data.");
    process.exit(2);
  }

  const t0 = Date.now();
  // spawnGame: the copy is checked against the judged bytes right before the
  // spawn, and HUMANITY_NO_HANDOFF=1 times this copy, never a newer
  // v*_HumanityOS.exe; an exit we did not ask for is watched for.
  const game = GL.spawnGame(path.join(RIG, "HumanityOS.exe"), [], {
    fresh: FRESH,
    rigName: "boot-timing",
    log,
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
  });
  const child = game.child;
  // The autopilot request is consumed on the first frame that finds it, so it
  // can be dropped immediately: the engine polls until it appears.
  fs.writeFileSync(path.join(DEBUG, "autopilot_request.json"), JSON.stringify({})); // no server_url: the pinned one stands. An empty one let the game fill in the live server and dial it (2026-10-05, BUG-160)

  let timing = null;
  let earlyExit = null;
  while (Date.now() - t0 < TIMEOUT_MS) {
    if (fs.existsSync(LOG) && /PANIC/.test(fs.readFileSync(LOG, "utf8"))) {
      break;
    }
    // An exit before it is playable, said as one (a hand-off is named) rather
    // than as a timeout.
    if (game.exited()) {
      earlyExit = game.describe();
      break;
    }
    const p = path.join(DEBUG, "boot_timing.json");
    if (fs.existsSync(p)) {
      try {
        timing = JSON.parse(fs.readFileSync(p, "utf8"));
        break;
      } catch {
        /* half written */
      }
    }
    await sleep(250);
  }
  const wallMs = Date.now() - t0;
  const logText = fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "";
  const panics = (logText.match(/PANIC/g) || []).length;
  // Copy the log aside: the next run deletes it, and a failed run's log is the
  // only thing that explains the failure.
  try {
    fs.copyFileSync(LOG, path.join(RIG, `run-${LABEL}-${n}.log`));
  } catch {
    /* no log at all */
  }
  game.expectExit(); // our own stop: not a hand-off to look for
  try {
    process.kill(-child.pid);
  } catch {
    /* already gone */
  }
  killRigProcesses();
  await sleep(1500); // let the process actually exit before the next boot

  return {
    run: n,
    wall_ms: wallMs,
    panics,
    // BUILT-IN DATA (BUG-133): run.log lines where a loader served the exe's
    // own copy of a data file instead of the tree's; any at all fails the run.
    builtin_data: GL.builtinDataLines(logText),
    early_exit: earlyExit,
    timing,
    // Sub-spans the renderer logs but the BootTimer does not carry.
    phases: parseBootPhases(logText),
    pipelines: parsePipelines(logText),
  };
}

/// `[BootPhase] name: N ms`, including the two-space-indented sub-spans.
function parseBootPhases(text) {
  const out = [];
  const re = /\[BootPhase\]\s+(\S[^:]*?):\s+([\d.]+) ms/g;
  let m;
  while ((m = re.exec(text))) out.push({ name: m[1].trim(), ms: Number(m[2]) });
  return out;
}

/// `[Pipelines] 19 PSOs compiled in 4.9s wall (34.4s serial sum): a 1.29s, ...`
function parsePipelines(text) {
  const out = [];
  const re = /\[Pipelines\] (\d+) (?:megashader )?PSOs compiled in ([\d.]+)s wall \(([\d.]+)s serial sum\): (.*)/g;
  let m;
  while ((m = re.exec(text))) {
    const per = {};
    for (const part of m[4].split(", ")) {
      const mm = part.match(/^(.*) ([\d.]+)s$/);
      if (mm) per[mm[1]] = Number(mm[2]);
    }
    out.push({ count: Number(m[1]), wall_s: Number(m[2]), serial_s: Number(m[3]), per });
  }
  // The cloud fullscreen PSOs log their own line.
  const re2 = /\[Pipelines\] (\d+) cloud fullscreen PSOs recompiled in ([\d.]+)s/g;
  while ((m = re2.exec(text))) out.push({ cloud_fullscreen: Number(m[1]), wall_s: Number(m[2]) });
  return out;
}

// ── Reporting ─────────────────────────────────────────────────────────────
const med = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
};

function report(runs) {
  const ok = runs.filter((r) => r.timing && r.panics === 0);
  console.log("");
  console.log(`=== boot timing: ${LABEL}, ${ok.length}/${runs.length} good runs ===`);
  if (!ok.length) return;
  const totals = ok.map((r) => r.timing.total_ms);
  console.log(
    `  wall to playable: median ${med(totals).toFixed(0)} ms  ` +
      `(runs: ${totals.map((t) => t.toFixed(0)).join(", ")})`
  );
  // Rank the BootTimer's own phases.
  const names = [];
  for (const r of ok) for (const p of r.timing.phases) if (!names.includes(p.name)) names.push(p.name);
  const rows = names
    .map((n) => {
      const vals = ok.map((r) => (r.timing.phases.find((p) => p.name === n) || { ms: 0 }).ms);
      return { name: n, ms: med(vals), vals };
    })
    .sort((a, b) => b.ms - a.ms);
  console.log("  phases, biggest first:");
  for (const r of rows) {
    console.log(
      `    ${r.name.padEnd(24)} ${r.ms.toFixed(0).padStart(7)} ms   ` +
        `[${r.vals.map((v) => v.toFixed(0)).join(" ")}]`
    );
  }
  // Rank the renderer's sub-spans, which is where renderer_init breaks down.
  const subNames = [];
  for (const r of ok) for (const p of r.phases) if (!subNames.includes(p.name)) subNames.push(p.name);
  if (subNames.length) {
    const subs = subNames
      .map((n) => {
        const vals = ok.map((r) => (r.phases.find((p) => p.name === n) || { ms: 0 }).ms);
        return { name: n, ms: med(vals), vals };
      })
      .sort((a, b) => b.ms - a.ms);
    console.log("  renderer sub-spans, biggest first:");
    for (const r of subs) {
      console.log(
        `    ${r.name.padEnd(24)} ${r.ms.toFixed(0).padStart(7)} ms   ` +
          `[${r.vals.map((v) => v.toFixed(0)).join(" ")}]`
      );
    }
  }
  const pl = ok.map((r) => r.pipelines[0]).filter(Boolean);
  if (pl.length) {
    console.log(
      `  [Pipelines] ${pl[0].count} PSOs: wall median ${med(pl.map((p) => p.wall_s)).toFixed(1)}s ` +
        `(runs: ${pl.map((p) => p.wall_s.toFixed(1)).join(", ")}), ` +
        `serial median ${med(pl.map((p) => p.serial_s)).toFixed(1)}s`
    );
  }
}

// ── Main ──────────────────────────────────────────────────────────────────
(async () => {
  // This tree's build, or another on purpose (--allow-other-build "<why>").
  FRESH = runFreshGate(EXE_SRC, args, { cwd: REPO });
  if (FRESH.status !== 0) {
    console.error("boot-timing: REFUSED - see the freshness failure above. Nothing was booted.");
    process.exit(1);
  }
  OTHER_BUILD = FRESH.other_build;
  setupRig();
  log(`rig ${RIG}`);
  log(`exe ${EXE_SRC}`);
  log(`${RUNS} run(s), label "${LABEL}", ${COLD ? "COLD cache each run" : "cache kept between runs"}`);
  const runs = [];
  for (let i = 1; i <= RUNS; i++) {
    log(`run ${i}/${RUNS} ...`);
    const r = await oneRun(i);
    if (!r.timing) log(`  run ${i} FAILED (no boot_timing.json, panics=${r.panics}${r.early_exit ? `; ${r.early_exit}` : ""})`);
    else log(`  run ${i}: ${r.timing.total_ms.toFixed(0)} ms to playable, panics=${r.panics}`);
    if (r.builtin_data.length) log(GL.builtinDataRefusal(r.builtin_data));
    runs.push(r);
  }
  fs.writeFileSync(
    OUT,
    JSON.stringify(
      {
        label: LABEL,
        cold: COLD,
        exe: EXE_SRC,
        // Which binary was timed (BUG-133), and the record of a deliberate
        // other-build arm.
        binary: bootRecord(FRESH, path.join(RIG, "HumanityOS.exe")),
        ...(OTHER_BUILD ? { other_build: OTHER_BUILD } : {}),
        runs,
      },
      null,
      2
    )
  );
  report(runs);
  if (OTHER_BUILD) log(otherBuildNotice(OTHER_BUILD));
  log(`wrote ${OUT}`);
  const bad = runs.filter((r) => !r.timing || r.panics > 0 || r.builtin_data.length > 0).length;
  process.exit(bad ? 2 : 0);
})();
