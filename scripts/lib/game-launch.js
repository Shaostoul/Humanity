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
//      (src/embedded_data.rs note_builtin_copy; scripts/lib/compiled-in.js
//      refuses a loader that does not), and builtinDataLines() finds them in a
//      run.log so the rig can refuse the run.

const fs = require("fs");
const crypto = require("crypto");
const { spawn, execSync } = require("child_process");
const { requireBootCopy } = require("./src-fingerprint.js");
const MG = require("./machine-guard.js");

/** The marker src/embedded_data.rs BUILTIN_COPY_MARKER writes (a test pins the two). */
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
 *            log (default console.log) }
 * Refuses (exit 1, nothing started) unless `exe` is byte-identical to the exe the
 * gate judged. Always sets HUMANITY_NO_HANDOFF=1.
 * Returns the watch: {
 *   child, pid,
 *   exited()      null while it runs, else what happened ("exited with code 0", ...)
 *   handoff       the hand-off processes found and stopped (after an exit)
 *   describe()    one sentence for a rig's error, naming a hand-off when there was one
 *   expectExit()  call before the rig stops the game itself: that exit is not news
 * }
 */
function spawnGame(exe, args, opts = {}) {
  const rigName = opts.rigName || "rig";
  requireBootCopy(exe, opts.fresh, rigName);
  const env = { ...(opts.env || process.env), HUMANITY_NO_HANDOFF: "1" };
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
  const log = opts.log || console.log;
  const pattern = opts.handoffPattern || /HumanityOS/i;
  let expected = false;
  const w = {
    child,
    pid: child.pid,
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
  child.on("exit", (code, signal) => {
    w.exit = signal ? `was ended by ${signal}` : `exited with code ${code}`;
    w.code = code;
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
  return w;
}

/** SHA-256 hex of a file (for tests and callers that build a fresh record). */
const fileSha256 = (p) => crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex");

module.exports = {
  BUILTIN_COPY_MARKER,
  builtinDataLines,
  builtinDataRefusal,
  childProcesses,
  reapHandoff,
  spawnGame,
  fileSha256,
};
