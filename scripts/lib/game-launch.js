// Starting the game from a rig: the one way every rig does it (BUG-133).
//
// A rig's claim is "THIS binary, run against THIS tree, did X". Three things
// can make that claim false after the freshness gate (scripts/check-fresh-exe.js,
// through src-fingerprint.js runFreshGate) has passed, and this module is where
// each is closed, so no rig can forget one:
//
//   1. The bytes that start are not the bytes the gate judged (a build finished
//      between the gate and the copy). spawnGame() checks the copy against the
//      gate's recorded SHA-256 IMMEDIATELY before it spawns, through
//      requireBootCopy, and refuses (exit 1, nothing started) on a difference.
//      Because the check lives inside the function that spawns, it cannot be
//      skipped or put in the wrong order by a rig.
//
//   2. The process that runs is not the one that started: src/main.rs
//      find_newer_exe hands a launch off to a newer signed v*_HumanityOS.exe in
//      C:\Humanity and exits 0. Builds with the switch never do that when
//      HUMANITY_NO_HANDOFF is set, and spawnGame() always sets it. A build from
//      BEFORE the switch (an archive run on purpose with --allow-other-build)
//      can still do it, so the watch below treats an exit the rig did not ask
//      for as what it is: it finds any HumanityOS process the game started
//      (by parent process id, which Windows keeps after the parent exits),
//      stops it, and reports the hand-off. Without this the handed-off copy ran
//      on the operator's real profile, from C:\Humanity, and nothing killed it
//      (the rigs only kill their own rig copy). Critic review, 2026-10-03.
//
//   3. The run used data the tree does not have: a data file on disk that is
//      missing or fails to parse makes its loader serve the copy built into the
//      exe, the rig stays green, and the next build (which embeds the tree's
//      file) behaves differently. Every such fallback logs one marker line
//      (crates/humanity-core/src/embedded_data.rs note_builtin_copy; scripts/lib/compiled-in.js
//      refuses a loader that does not), and builtinDataLines() finds them in a
//      run.log so the rig can refuse the run.
//
//   4. The game ran on settings the rig was not built on (2026-10-04): fresh
//      installs start in Normal mode with progress kept, and a rig's sandbox is a
//      fresh install; and a sandbox that once reached the live server kept it in
//      its saved servers and dialed it at every boot (2026-10-05: a
//      verify-screens run identified on united-humanity.us with a rig identity).
//      spawnGame pins the Dev mode, the default home every launch, the rig's own
//      relay (or a dead loopback port) as its server and no saved servers into the
//      sandbox's config.json right before it spawns (lib/rig-gameplay.js), and
//      REFUSES to start a game whose config still names a server off this
//      computer.
//
//   5. Another rig booted in the same moment (2026-10-21, BUG-140's fixer: the
//      builds on the machine ended, every rig waiting on them woke in the same
//      poll, and their games fought over the 12 GB card). spawnGame will not
//      start a game without the machine-wide boot lock (lib/boot-lock.js): a rig
//      whose machine guard already took it goes straight on; any other rig waits
//      here, bounded, while another rig holds it, and refuses (exit 1, nothing
//      started) if it never comes free. Then it tells a parent gate the game has
//      started (machine-guard.js signalPhase "booted"), which is when
//      verify-runtime's clock begins.
//
//   6. The game compiled its shaders with FXC (2026-10-21): no DXC pair beside
//      the exe, so its pipeline build took ~170 s of a 180 s autopilot wait,
//      and the rig's verdict did not say so. spawnGame checks the pair beside
//      the exe it starts (the game's own test, src/renderer/mod.rs), records it
//      on the watch (shader_compiler) and in the "booted" signal, and on FXC
//      prints lib/dxc-dlls.js fxcBanner as the rig's last lines.
//
//   7. The game ran at full graphics beside the operator's program when the rig
//      meant it to run light (2026-10-22, lib/rig-needs.js): low impact
//      (src/engine/low_impact.rs) is asked for ONLY through opts.lowImpact, which
//      sets HUMANITY_LOW_IMPACT for the game; an inherited one is dropped, so a
//      picture rig started from a shell that has it set never photographs a
//      low-impact game. A game asked to run light must say it does by writing
//      debug/low_impact.json in its folder (a stale one is deleted before the
//      spawn); one that has not said so within lowImpactConfirmMs (5 min) is
//      stopped and the rig refuses (exit 1): a build from before the switch would
//      otherwise run at full graphics beside his game for the whole check.
//
//   8. The boot was seen and nobody could say whose it was (2026-11-14, the operator: "It appears
//      the game often loads up then cycles through a bunch of static scenes then turns off. What's
//      happening?"). spawnGame always sets HUMANITY_NO_FOCUS, the developer's quiet-boot signal, so
//      the game hides its window from the screen (src/engine/rig_window.rs: cloaked, no taskbar
//      button, every frame still drawn; HUMANITY_RIG_VISIBLE=1 in the rig's env keeps it visible
//      behind), and it writes the boot's one line to the machine-wide ledger (lib/boot-ledger.js,
//      %LOCALAPPDATA%\HumanityOS\dev\boots.jsonl: when, which checkout, which script and rig, what
//      for, how long, how it ended). `node scripts/boot-ledger.js` prints the last day.

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { spawn, execSync } = require("child_process");
const { requireBootCopy } = require("./src-fingerprint.js");
const MG = require("./machine-guard.js");
const RG = require("./rig-gameplay.js");
const DXC = require("./dxc-dlls.js");
const BL = require("./boot-ledger.js");

/** The checkout these scripts belong to (where a missing DXC pair is to go, or its main checkout). */
const REPO = path.resolve(__dirname, "..", "..");
/** The FXC banner is printed once per rig process, however many games it starts. */
let fxcBannerArmed = false;

/** The marker crates/humanity-core/src/embedded_data.rs BUILTIN_COPY_MARKER writes (a test pins the two). */
const BUILTIN_COPY_MARKER = "[built-in data copy]";

/** The run.log lines that say the run served a built-in copy of a data file. */
function builtinDataLines(text) {
  return String(text || "")
    .split(/\r?\n/)
    .filter((l) => l.includes(BUILTIN_COPY_MARKER))
    .map((l) => l.trim());
}

/** A rig's refusal text for those lines (null when there are none). */
function builtinDataRefusal(lines) {
  if (!lines || !lines.length) return null;
  return [
    `BUILT-IN DATA: the run served ${lines.length} data file(s) from the copy compiled into the exe, not from this tree's data/:`,
    ...lines.slice(0, 8).map((l) => `  ${l.replace(/^.*?\[built-in data copy\]\s*/, "")}`),
    ...(lines.length > 8 ? [`  ... and ${lines.length - 8} more (run.log)`] : []),
    "A file the tree has that the loader could not use (missing, or it does not parse) is not what a",
    "rebuild would run, so this run says nothing about this tree. Fix the file (just validate-data), then run again.",
  ].join("\n");
}

/** The processes whose parent is `pid`: [{ pid, name, exe }]. Windows keeps a
 *  process's ParentProcessId after the parent exits, which is what lets a
 *  hand-off be found after the game that made it is gone. */
function childProcesses(pid) {
  const cmd =
    'powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \\"ParentProcessId=' +
    Number(pid) +
    "\\\" | ForEach-Object { $_.ProcessId.ToString() + '|' + $_.Name + '|' + $_.ExecutablePath }\"";
  try {
    return MG.parseProcs(execSync(cmd, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }));
  } catch {
    return null; // the query itself failed: say so rather than "none found"
  }
}

/** Stop every process `pid` started whose name matches `pattern` (a hand-off is a
 *  v*_HumanityOS.exe). Returns { stopped: [...], query_failed }. */
function reapHandoff(pid, pattern = /HumanityOS/i) {
  const kids = childProcesses(pid);
  if (kids === null) return { stopped: [], query_failed: true };
  const stopped = [];
  for (const k of kids) {
    if (!pattern.test(k.name || "") && !pattern.test(k.exe || "")) continue;
    try {
      execSync(`taskkill /PID ${k.pid} /T /F`, { stdio: "ignore" });
    } catch {
      /* already gone */
    }
    stopped.push(k);
  }
  return { stopped, query_failed: false };
}

/**
 * Start the game copy a rig is about to judge.
 *   exe    the path to spawn (the rig's copy)
 *   args   its arguments
 *   opts   { fresh (runFreshGate's return), rigName, cwd, env (default process.env),
 *            detached, stdio (default "ignore"), handoffPattern (default /HumanityOS/i),
 *            log (default console.log), gameplay (default {}: lib/rig-gameplay.js; a rig
 *            may ask for fresh_world_each_launch, and a rig with a throwaway relay
 *            names it as server_url, which must be on this computer),
 *            lockWaitMs (how long to wait for another rig's boot lock; the
 *            machine guard's WAIT_MIN_DEFAULT),
 *            lowImpact (the HUMANITY_LOW_IMPACT value lib/rig-needs.js planBoot chose, or
 *            null for full graphics; point 7), lowImpactConfirmMs (how long the game has to
 *            confirm it, LOW_IMPACT_CONFIRM_MS),
 *            purpose (what the boot is for, for the boot ledger: the vantages, a game_boot; the
 *            environment's HUMANITY_BOOT_PURPOSE otherwise), keepOpen (the rig leaves the game
 *            running when it ends, as probe-sweep --keep-open does: its ledger line waits, pending,
 *            for whoever closes it; point 8) }
 * Refuses (exit 1, nothing started) unless `exe` is byte-identical to the exe the
 * gate judged. Always sets HUMANITY_NO_HANDOFF=1. Pins the Dev play mode, the
 * default home every launch, the rig's server and no saved servers into the config
 * of the sandbox `exe` runs in, and refuses (exit 1, nothing started) when that
 * config still names a server off this computer. Holds the machine-wide boot lock
 * before it starts anything (point 5), refusing (exit 1) if another rig holds it
 * past lockWaitMs.
 * Returns the watch: {
 *   child, pid,
 *   gameplay      what was pinned ({ path, gameplay, changed }), null outside a sandbox
 *   exited()      null while it runs, else what happened ("exited with code 0", ...)
 *   handoff       the hand-off processes found and stopped (after an exit)
 *   describe()    one sentence for a rig's error, naming a hand-off when there was one
 *   expectExit()  call before the rig stops the game itself: that exit is not news
 *   low_impact    null at full graphics, else { asked, confirmed } (confirmed: the game's
 *                 debug/low_impact.json once it has written it)
 * }
 */
function spawnGame(exe, args, opts = {}) {
  const rigName = opts.rigName || "rig";
  requireBootCopy(exe, opts.fresh, rigName);
  // HUMANITY_NO_FOCUS always (point 8): a rig's boot is a developer's quiet boot, hidden from the
  // operator's screen by the game itself, whatever env the rig passed.
  const env = { ...(opts.env || process.env), HUMANITY_NO_HANDOFF: "1", HUMANITY_NO_FOCUS: "1" };
  // LOW IMPACT (point 7): only the rig asks, never the shell it was started from.
  delete env[LOW_IMPACT_ENV];
  if (opts.lowImpact) env[LOW_IMPACT_ENV] = String(opts.lowImpact);
  const log = opts.log || console.log;
  // The game's proof it runs light lands in its own folder's debug/ (its working directory).
  const lowImpactFile = path.join(opts.cwd || process.cwd(), "debug", LOW_IMPACT_FILE);
  if (opts.lowImpact) fs.rmSync(lowImpactFile, { force: true });
  // The sandbox's settings, in the config this very game reads, immediately before it starts
  // (point 4 at the top): a rig is a Dev sandbox on this computer whatever its config.json held.
  const gameplay = RG.pinSandboxGameplay(exe, env, opts.gameplay, log, rigName);
  if (gameplay && gameplay.public_servers.length) {
    console.error("");
    console.error(`RIG CONFIG NAMES A SERVER OFF THIS COMPUTER (${gameplay.path}), which a rig must never dial:`);
    for (const p of gameplay.public_servers.slice(0, 8)) console.error(`  ${p.path}: ${p.value}`);
    console.error("scripts/lib/rig-gameplay.js pinGameplay clears the fields it knows; teach it this one.");
    console.error("");
    console.error(`${rigName}: REFUSED - nothing was booted.`);
    process.exit(1);
  }
  // ONE BOOT AT A TIME (point 5). Last before the spawn, so nothing above can
  // leave the lock held by a rig that then refused for another reason (its
  // process exit releases it either way). Only the lock is looked at: the rig
  // made its own check for other games before it built its folder, and a rig
  // with a throwaway relay (one more HumanityOS.exe, ours) would wait on itself.
  const lk = MG.waitForFree({ procs: false, label: `${rigName}: boot lock`, rig: rigName, log, timeoutMs: opts.lockWaitMs });
  if (!lk.free) MG.refuseBusy(lk, { rig: rigName, hint: ["Another rig has held the boot lock all that time: wait for it, then run again."] });
  // THE BOOT LEDGER (point 8). A boot this folder's last rig left running and nobody closed through
  // the ledger is written now, its end unknown; then this boot is announced to the game as recorded
  // by its launcher, so the game writes no line of its own.
  const ledgerDir = path.join(opts.cwd || path.dirname(exe), "debug");
  BL.flushStalePending(ledgerDir, env);
  env[BL.OWNER_ENV] = "spawnGame";
  let child;
  try {
    child = spawn(exe, args || [], {
      cwd: opts.cwd,
      detached: !!opts.detached,
      stdio: opts.stdio || "ignore",
      env,
    });
  } catch (e) {
    // Windows refuses a file that is not a program before any process exists
    // (spawn throws, "spawn UNKNOWN"): nothing was started, say so plainly.
    throw new Error(`${rigName}: could not start ${exe} (${e.message}); nothing booted`);
  }
  // WHICH SHADER COMPILER (point 6): by the game's own test, the DXC pair beside
  // the exe it runs. FXC is said at the boot, as the rig's LAST lines (its
  // verdict is printed by then) and to a parent gate.
  const compiler = MG.isGameName(path.basename(exe)) ? DXC.compilerFor(exe) : null;
  if (compiler) log(`[${rigName}] shader compiler: ${compiler}${compiler === "FXC" ? ` (no ${DXC.DXC_DLLS.join(" + ")} beside ${exe}; boot is minutes slower)` : ""}`);
  // Not inside `node --test` (NODE_TEST_CONTEXT): tests start node under the game's name in folders
  // with no dlls, and a banner at the end of `just rig-tests` would read as a real rig on FXC. The
  // test of the banner itself runs its rig without that variable.
  if (compiler === "FXC" && child.pid && !process.env.NODE_TEST_CONTEXT) {
    const banner = DXC.fxcBanner({ exe, pid: child.pid, rig: rigName, where: DXC.pairHome(REPO) });
    if (!fxcBannerArmed) {
      fxcBannerArmed = true;
      process.on("exit", () => {
        for (const l of banner) console.error(l);
      });
    }
  }
  // The game exists: a parent gate's clock starts now (verify-runtime). No pid
  // means the start failed (an 'error' event follows): no boot to time.
  if (child.pid) MG.signalPhase("booted", { rig: rigName, game_pid: child.pid, shader_compiler: compiler, low_impact: opts.lowImpact || null });
  const pattern = opts.handoffPattern || /HumanityOS/i;
  let expected = false;
  const w = {
    child,
    pid: child.pid,
    gameplay,
    shader_compiler: compiler, // "DXC" | "FXC" (null: not the game, or not Windows)
    exit: null,
    code: null,
    handoff: [],
    query_failed: false,
    exited: () => w.exit,
    expectExit: () => {
      expected = true;
    },
    describe: () => {
      if (!w.exit) return null;
      if (w.handoff.length) {
        return (
          `the game ${w.exit} and handed itself off to ${w.handoff.map((h) => `${h.exe || h.name} (pid ${h.pid})`).join(", ")}, ` +
          "which was stopped: the binary that ran was not the one checked (src/main.rs find_newer_exe; only a build from " +
          "before HUMANITY_NO_HANDOFF still does this)"
        );
      }
      if (w.code === 0) {
        return (
          `the game ${w.exit} straight away: a clean exit this early is the shape of a hand-off to a newer ` +
          `v*_HumanityOS.exe (src/main.rs find_newer_exe)${w.query_failed ? ", and the process list could not be read to look for one" : ", though no handed-off process was found"}`
        );
      }
      return `the game ${w.exit} (see run.log)`;
    },
  };
  // THE BOOT'S LEDGER LINE (point 8): written once, when the game exits or the rig's process ends,
  // whichever comes first; a boot the rig keeps open waits, pending, for whoever closes it.
  const ledgerStart = BL.startRecord({
    checkout: REPO,
    script: process.argv[1] ? path.basename(process.argv[1]) : null,
    rig: rigName,
    via: env[BL.VIA_ENV],
    purpose: opts.purpose || env[BL.PURPOSE_ENV],
    pid: child.pid,
    exe,
    hidden: BL.meantHidden(env),
    lowImpact: opts.lowImpact,
    writer: "spawnGame",
  });
  // No process: no boot to record. Nor a stand-in (the rig tests start node as their "game"): only
  // an exe named like the game is a boot, unless a test named its own ledger file to read back.
  const recordable = MG.isGameName(path.basename(exe)) || !!(env[BL.FILE_ENV] || process.env[BL.FILE_ENV]);
  let ledgerDone = !child.pid || !recordable;
  const writeLedger = (ended) => {
    if (ledgerDone) return;
    ledgerDone = true;
    BL.append(BL.finishRecord(ledgerStart, ended), env);
  };
  // Registered on the next turn, so it runs AFTER the exit handler a rig adds straight after this
  // call (probe-sweep's, which stops the game): by then expectExit() says whether the rig stopped it.
  setImmediate(() =>
    process.on("exit", () => {
      if (ledgerDone) return;
      if (w.exit) return writeLedger(expected ? `stopped by the rig (${w.exit})` : w.exit);
      if (opts.keepOpen && !expected) {
        ledgerDone = true;
        BL.writePending(ledgerDir, ledgerStart);
        return;
      }
      writeLedger(expected ? "stopped by the rig" : "rig ended (the game was still running at that moment)");
    })
  );
  child.on("exit", (code, signal) => {
    w.exit = signal ? `was ended by ${signal}` : `exited with code ${code}`;
    w.code = code;
    writeLedger(expected ? `stopped by the rig (${w.exit})` : w.exit);
    if (expected) return;
    // Not asked for: look for a hand-off before anything else happens.
    const r = reapHandoff(child.pid, pattern);
    w.handoff = r.stopped;
    w.query_failed = r.query_failed;
    if (r.stopped.length) {
      for (const h of r.stopped) log(`[${rigName}] HAND-OFF stopped: pid ${h.pid} ${h.exe || h.name}, started by the game (pid ${child.pid}) as it exited`);
    }
  });
  child.on("error", (e) => {
    w.exit = `could not be started (${e.message})`;
  });
  // LOW IMPACT, CONFIRMED (point 7): the game writes what it runs at; until it does, the rig
  // does not know it is light. Polled, never blocking the rig's own work meanwhile.
  w.low_impact = opts.lowImpact ? { asked: String(opts.lowImpact), confirmed: null } : null;
  if (opts.lowImpact && child.pid) {
    const limit = opts.lowImpactConfirmMs != null ? opts.lowImpactConfirmMs : LOW_IMPACT_CONFIRM_MS;
    const t0 = Date.now();
    const timer = setInterval(() => {
      if (w.exit) return clearInterval(timer); // the rig deals with an exit
      const c = readLowImpact(lowImpactFile);
      if (c && c.on === true && String(c.value) === String(opts.lowImpact)) {
        clearInterval(timer);
        w.low_impact.confirmed = c;
        const win = Array.isArray(c.window) ? c.window.join("x") : c.window;
        log(`[${rigName}] low impact confirmed by the game: ${c.fps} fps, window ${win}, ${Object.keys(c.settings || {}).length} settings laid over its config (${c.preset_found ? `the "${c.preset}" preset` : "NO PRESET FOUND: frame cap and window only"})`);
        return;
      }
      if (Date.now() - t0 < limit) return;
      clearInterval(timer);
      w.expectExit();
      try {
        execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: "ignore" });
      } catch {
        /* already gone */
      }
      console.error("");
      console.error(`${rigName}: REFUSED - the game was asked to run in low impact (HUMANITY_LOW_IMPACT=${opts.lowImpact}) and had not`);
      console.error(`said it does (${lowImpactFile}) ${Math.round(limit / 1000)} s after it started, so it was stopped: it would have run at`);
      console.error("full graphics beside the operator's program. A build from before src/engine/low_impact.rs does that; rebuild,");
      console.error("or run this check when his program has closed. Nothing was judged.");
      console.error("");
      MG.signalPhase("refused", { rig: rigName, reason: "low_impact", headline: `${rigName}: REFUSED: the game never confirmed low impact` });
      process.exit(1);
    }, 500);
    if (timer.unref) timer.unref();
  }
  return w;
}

/** The variable that asks the game for low impact (src/engine/low_impact.rs ENV). */
const LOW_IMPACT_ENV = "HUMANITY_LOW_IMPACT";
/** The file the game writes in its debug/ when it runs light (low_impact.rs write_confirmation). */
const LOW_IMPACT_FILE = "low_impact.json";
/** How long a game asked to run light has to say it does: boot on FXC takes minutes. */
const LOW_IMPACT_CONFIRM_MS = 5 * 60 * 1000;

/** A low-impact confirmation file, or null (missing, or half written). */
function readLowImpact(p) {
  try {
    return JSON.parse(fs.readFileSync(p, "utf8"));
  } catch {
    return null;
  }
}

/** SHA-256 hex of a file (for tests and callers that build a fresh record). */
const fileSha256 = (p) => crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex");

module.exports = {
  LOW_IMPACT_ENV,
  LOW_IMPACT_FILE,
  LOW_IMPACT_CONFIRM_MS,
  readLowImpact,
  BUILTIN_COPY_MARKER,
  builtinDataLines,
  builtinDataRefusal,
  childProcesses,
  reapHandoff,
  spawnGame,
  fileSha256,
};
