// ONE MACHINE, for the WHOLE sweep - not just the launch, and not just the GPU.
//
// CLAUDE.md's rule is "at most ONE booted HumanityOS instance machine-wide".
// Every rig script honoured it with a single check before spawning, which is
// exactly half the guarantee the rule is worth, and it watched only half the
// machine. Two failures on 2026-09-18, both of which produced a NUMBER rather
// than an error, which is what makes them expensive:
//
//   GPU. docs/design/frame-cost-arc.md, "V1 outcome": the baseline's boot 1
//   read `gpu.celestial` 75.79 ms at `fuji-forest-ground` where every clean
//   boot of the same exe reads 66.8 to 67.1. Another builder's instance had
//   arrived DURING that sweep. The 9 ms of somebody else's GPU work sat in a
//   comparison table as data, and the increment being measured would have
//   "won" 9 ms it never won. Only a second boot exposed it.
//
//   CPU. The orchestrator's re-measure the same afternoon, with no other game
//   instance alive at all: 6.6 fps at the limb against a 15 ms GPU sum. A
//   `cargo.exe` and a 2.3 GB `rustc.exe` from another worktree's release build
//   were saturating the CPU, so every `cpu.*` stage stretched (patch_build 24
//   ms) while the GPU columns stayed normal. A frame time is a measure of the
//   WHOLE machine; the guard has to be too.
//
// So the guard has three jobs, and this module is the one place all of them
// live:
//   1. waitForFree()   - before a boot, WAIT (bounded) for a contender to
//                        leave, and say how long it waited. Silence here is how
//                        a sweep starts beside somebody else's rig or build.
//   2. foreignProcs()  - around a capture, name anything competing that is not
//                        ours. Called before AND after: a process that arrives
//                        and leaves inside the capture window still ruined it.
//   3. parseProcs()    - the pure half, so the guard itself can be tested with
//                        a fake process list (scripts/tests/machine-guard.test.js)
//                        rather than by booting a second renderer, which the
//                        machine rule forbids anyway.
//
// A capture taken while a contender was up is marked `contaminated` in the
// manifest and perf-report.js REFUSES to grade it. Refusing is the point: the
// failure this closes is not "a slow reading", it is "a wrong reading that
// reads as a real one".
//
// And a fourth job, for rigs whose checks are about behaviour rather than
// timings (BUG-161):
//   4. holdBuilds()    - for the length of a capture, hold every running build
//                        at BelowNormal priority and put each back afterwards.
//                        See the section above it for what it may and may not do.
//
// And, since 2026-10-22, the operator's own programs (operator-apps.json): a rig
// that judges the picture or times frames waits for them; a rig whose checks are
// about behaviour runs beside them with its game in LOW IMPACT (own.lowImpact;
// which rig is which: lib/rig-needs.js).
//
// And, since 2026-10-21, the boot lock and the refusal (lib/boot-lock.js):
//   5. waitForFree() takes the machine-wide boot lock when the machine goes
//      quiet, and counts another rig's held lock as NOT quiet, so rigs that
//      were all waiting on the same builds cannot all boot in the same poll.
//   6. refuseBusy()    - the one way a rig says its wait gave up: "REFUSED: the
//                        machine never went quiet in N min; nothing was
//                        booted", exit 1 (refused, the code every rig uses for
//                        "did not run"), never 2 (a check failed).
//   7. signalPhase()   - tells a parent gate (verify-runtime) when the game has
//                        started, so the gate's clock times the game and not
//                        the wait in front of it.

const fs = require("fs");
const os = require("os");
const path = require("path");
const { execSync, execFileSync } = require("child_process");
const BL = require("./boot-lock.js");

/// How long a rig waits for the machine before it refuses, minutes. One number
/// for every rig (2026-10-21; they had 20, 40 and a 12 that was really the
/// verify-runtime gate's clock running out while it waited). Each rig takes
/// --wait-min N to change it for a run.
const WAIT_MIN_DEFAULT = 60;

/// Processes that make a frame-time reading meaningless.
///
/// HumanityOS.exe   a second renderer on the one GPU (the CLAUDE.md rule).
/// cargo.exe        a build in another worktree of this shared checkout.
/// rustc.exe        the same build's actual compiler - a release rustc here
///                  peaks around 2.3 GB and pins cores for minutes.
/// link.exe, cl.exe the MSVC back end, which is the memory-heaviest phase of a
///                  Windows release build and can run without cargo above it.
///
/// Deliberately NOT here: rust-analyzer (an IDE would block every sweep
/// forever) and mspdbsrv (a leftover service that idles). If you add a name,
/// add it because it takes the machine away from a measurement, not because it
/// is untidy - a guard that blocks constantly gets bypassed, and a bypassed
/// guard protects nothing.
const WATCHED = ["HumanityOS.exe", "cargo.exe", "rustc.exe", "link.exe", "cl.exe"];

/// The game binary's name, split out because two gates (verify-runtime,
/// verify-screens) refuse to boot on ANY instance of it while tolerating a
/// build, and their refusal messages say exactly that.
const GAME = "HumanityOS.exe";

/// Is this process name the game? The exe is not always called HumanityOS.exe:
/// `just launch` and `just play` boot the newest ARCHIVE, named
/// v<version>_HumanityOS.exe, and that is how the operator usually starts his
/// own game. An exact-name match missed it (found 2026-10-04), so the one-GPU
/// wait could boot a rig beside his game. Any name ending in HumanityOS.exe,
/// case-insensitive, counts.
const isGameName = (name) => /humanityos\.exe$/i.test(String(name || ""));

/// The operator's own GPU-heavy programs (scripts/lib/operator-apps.json, 2026-10-22). A GeForce
/// cannot be split between him and the rigs. What a rig does while one runs depends on what its
/// verdict is made of (scripts/lib/rig-needs.js): a rig that judges the picture or times frames
/// WAITS for it to close; a rig whose checks are about behaviour runs beside it with its game in
/// LOW IMPACT (src/engine/low_impact.rs), and says so with own.lowImpact (foreignProcs). They are
/// never "ours". Read once; a missing or broken file means none (a rig must still work on a
/// machine without it).
const OPERATOR_APPS = (() => {
  try {
    const j = JSON.parse(fs.readFileSync(path.join(__dirname, "operator-apps.json"), "utf8"));
    return (Array.isArray(j.apps) ? j.apps : []).map((n) => String(n).toLowerCase()).filter((n) => /^[\w .()-]+\.exe$/.test(n));
  } catch {
    return [];
  }
})();
const isOperatorApp = (name) => OPERATOR_APPS.includes(String(name || "").toLowerCase());

/// Is this HumanityOS the operator's OWN game (2026-10-23)? His game is a GPU-heavy program of his
/// like any other in operator-apps.json: a rig that judges the picture or times frames waits for
/// it, and a behaviour rig runs beside it in low impact instead of closing it. What is his and
/// what is a development copy is decided by the path, the way the PC share guard decides it
/// (scripts/pc-share-guard.ps1 Test-DevProcess): a copy under a .probe-rig folder, a worktree, a
/// target folder or a temp folder (the rigs' throwaway relays, the storeys sandbox, the tests'
/// rigs), or the shipped-assets rig, is a development copy and still holds every rig (one booted
/// development instance at a time). Anything else (C:\Humanity\HumanityOS.exe, a
/// v<version>_HumanityOS.exe archive, an installed copy) is his. A path nobody may read is NOT his:
/// erring loud, it holds every rig.
const DEV_COPY_PATH = /\\\.probe-rig|\\\.claude\\worktrees\\|\\target\\(debug|release)\\|\\temp\\|\\tmp\\|HumanityOS-probe-rig/i;
const isOperatorGame = (p) => !!p && isGameName(p.name) && !!p.exe && !DEV_COPY_PATH.test(String(p.exe).replace(/\//g, "\\"));

/// The build tools among WATCHED: everything watched that is not the game. One
/// list of what a build is, so a build added to WATCHED is also held back by
/// holdBuilds (BUG-161), and nothing else ever is.
const BUILDS = WATCHED.filter((n) => !isGameName(n));

// Windows query. ExecutablePath matters as much as the pid for the game: it is
// what separates our own rig copy (ours by construction - only sweeps launch
// from a rig dir) from the operator's game or another agent's rig.
function psQuery() {
  const filter = WATCHED.concat(OPERATOR_APPS)
    .map((n) => (n === GAME ? `Name LIKE '%${n}'` : `Name='${n}'`))
    .join(" or ");
  return (
    'powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter ' +
    `\\"${filter}\\"` +
    " | ForEach-Object { $_.ProcessId.ToString() + '|' + $_.Name + '|' + $_.ExecutablePath }\""
  );
}

/// Parse the `pid|name|exe` lines the query prints into records. PURE: the
/// whole point is that a test can hand it a busy machine without one existing.
/// A process whose ExecutablePath we may not read comes back with exe "" -
/// which the ownership test below treats as NOT ours, i.e. it errs loud.
function parseProcs(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      const a = l.indexOf("|");
      const b = l.indexOf("|", a + 1);
      if (a < 0) return { pid: Number(l), name: "", exe: "" };
      const pid = Number(l.slice(0, a));
      const name = b < 0 ? l.slice(a + 1).trim() : l.slice(a + 1, b).trim();
      const exe = b < 0 ? "" : l.slice(b + 1).trim();
      return { pid, name, exe };
    })
    .filter((p) => Number.isFinite(p.pid) && p.pid > 0);
}

// The live lister, swappable. Two injection routes, both deliberate:
//   setLister(fn)               in-process tests
//   HUMANITY_MACHINE_GUARD_FAKE a child process's tests, and the manual red
//     proof. Value is either a path to a file holding fake query output, or the
//     output itself with \n escapes. An env-injected fake prints a loud line so
//     a real run can never be mistaken for a faked one.
let lister = null;
function setLister(fn) {
  lister = fn;
}

function defaultLister() {
  const fake = process.env.HUMANITY_MACHINE_GUARD_FAKE;
  if (fake) {
    const text = fs.existsSync(fake) ? fs.readFileSync(fake, "utf8") : fake.replace(/\\n/g, "\n");
    console.log("[machine-guard] FAKE process list in use (HUMANITY_MACHINE_GUARD_FAKE) - not a real run");
    return text;
  }
  try {
    return execSync(psQuery(), { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  } catch {
    return ""; // not Windows, or no powershell: degrade to "nothing running"
  }
}

/// Everything the OS can see right now from the watched set.
function listProcs() {
  return parseProcs(lister ? lister() : defaultLister());
}

/// Only the game. verify-runtime and verify-screens gate on this: their rule is
/// "no second renderer", which a compile does not violate. The operator's own
/// programs are not HumanityOS and are not listed here (2026-10-22: for a day they
/// were, and every rig that refuses on another instance refused while he played);
/// what each rig does about them is operatorApps() and scripts/lib/rig-needs.js.
function listInstances() {
  return listProcs().filter((p) => !p.name || isGameName(p.name));
}

/// The operator's own GPU-heavy programs running now: those in operator-apps.json, and his own
/// HumanityOS (isOperatorGame).
function operatorApps() {
  return listProcs().filter((p) => isOperatorApp(p.name) || isOperatorGame(p));
}

/// The HumanityOS instances in a rig's way: every one, except the operator's own game when the
/// rig runs light beside it (own.lowImpact). For the rigs that refuse at once on another instance
/// rather than wait (verify-copresence, verify-screens, verify-live-screen).
function blockingInstances(own = {}) {
  return listInstances().filter((p) => !(own.lowImpact && isOperatorGame(p)));
}

const sameName = (a, b) => String(a).toLowerCase() === String(b).toLowerCase();
const sameExe = (a, b) =>
  !!a && !!b && a.toLowerCase().replace(/\//g, "\\") === b.toLowerCase().replace(/\//g, "\\");

/// Everything competing with this rig for the machine right now.
///
/// `own.pids`  - what we spawned (the sweep knows this: it holds the child).
/// `own.exe`   - the rig's own exe copy. Anything running FROM it is ours by
///               construction: setup kills every process on that path before
///               copying, and only sweeps launch from a rig dir. This is what
///               covers the engine's self-delegation (the exe can hand off to a
///               second copy of itself, with a pid we never saw).
///
/// A BUILD process is never "ours": the rig never compiles. If that ever
/// changes, exclude it by pid here rather than by dropping it from WATCHED.
///
/// `own.gameOnly` - builds do not count, only other game instances (the ONE
///               GPU rule). For a boot check whose only question is "does it
///               enter the world with no panic" (2026-09-27: four worktree
///               agents compiling kept a panics-only check waiting past its
///               own timeout). Timings from such a run are not readings, so
///               the sweep still marks every capture beside a build as
///               contaminated.
///
/// `own.lowImpact` - this rig boots its game in LOW IMPACT (scripts/lib/rig-needs.js
///               planBoot, for a rig whose checks are about behaviour): the
///               operator's own programs (operator-apps.json) are then NOT in
///               its way, and it runs beside them. Without it they are, and the
///               rig waits for them to close (a rig that judges the picture or
///               times frames, or a behaviour rig run with --full-graphics).
function foreignProcs(own = {}) {
  const pids = new Set((own.pids || []).map(Number));
  const exe = own.exe || "";
  return listProcs().filter((p) => {
    if (pids.has(p.pid)) return false;
    if (isGameName(p.name) && sameExe(p.exe, exe)) return false; // our own rig copy, wherever its folder is (never a build)
    if (isOperatorApp(p.name) || isOperatorGame(p)) return !own.lowImpact; // his program (or his own game): in the way unless we run light beside it
    if (isGameName(p.name) || !p.name) return !sameExe(p.exe, exe);
    return !own.gameOnly; // a build: a contender unless only games count
  });
}

// ── MEMORY, NOT "NO BUILDS" (2026-10-23, the build-speed review) ──────────────
//
// A rig used to wait for every cargo, rustc and link to leave before it booted. In a checkout
// shared by several sessions somebody is nearly always compiling, so rigs sat for most of an
// hour, and the reason to wait was never the build itself: it was the memory and the CPU it
// took. The CPU is handled (the share guard keeps builds at below-normal priority, and BUG-161's
// holdBuilds holds them back for a capture), so what a boot needs is MEMORY: a game that boots
// into a machine with no free physical memory pages, crawls and fails checks it would pass.
// So a rig waits for: another game or rig on the one GPU, the operator's own programs, the boot
// lock, and FREE PHYSICAL MEMORY (RIG_MIN_FREE_MB). Builds count only for a run that measures
// TIMINGS (own.waitForBuilds: probe-sweep --wait-builds, which `just perf-sweep` passes, and
// boot-timing), because a frame time is a measure of the whole machine. Every capture beside a
// build is still marked contaminated (markCapture, below), so no timing taken beside one is ever
// graded as a reading.

/// What a booted game needs free, MB: its own working set at world entry plus room for the
/// build slot's compiles not to push it into paging. 6 GB is a starting figure, not a
/// measurement: lower it (HUMANITY_RIG_MIN_FREE_MB) once a rig's run.log has recorded the real one.
const RIG_MIN_FREE_MB_DEFAULT = 6144;
const rigMinFreeMb = () => {
  const v = Number(process.env.HUMANITY_RIG_MIN_FREE_MB);
  return v > 0 ? v : RIG_MIN_FREE_MB_DEFAULT;
};

// The free-memory reader, swappable like the process lister: setFreeMem(fn) in-process, or
// HUMANITY_MACHINE_GUARD_FAKE_FREE_MB for a child process. Under a FAKE process list with no
// fake memory the machine is taken to have room (a test of the process logic must not depend
// on this machine's memory at that moment).
let freeMemFn = null;
function setFreeMem(fn) {
  freeMemFn = fn;
}
/// Free physical memory, MB. os.freemem() is Windows' available physical memory (standby pages
/// included, as Task Manager's "Available").
function freeMemMb() {
  if (freeMemFn) return freeMemFn();
  const fake = process.env.HUMANITY_MACHINE_GUARD_FAKE_FREE_MB;
  if (fake != null && fake !== "") return Number(fake);
  if (!!lister || !!process.env.HUMANITY_MACHINE_GUARD_FAKE) return Infinity;
  return Math.round(os.freemem() / (1024 * 1024));
}
/// The memory blocker for a boot, or null when there is room.
function memoryShortfall(needMb = rigMinFreeMb()) {
  const free = freeMemMb();
  return free >= needMb ? null : { pid: 0, name: "", exe: "", memory: { free_mb: free, need_mb: needMb } };
}
const gbOf = (mb) => `${(mb / 1024).toFixed(1)} GB`;

/// One-line description of a contender, for a log line or a manifest note. A
/// held boot lock (waitForFree) is described as the rig holding it, and a memory
/// shortfall as the numbers.
const describe = (p) =>
  p && p.memory
    ? `free physical memory ${gbOf(p.memory.free_mb)}: a rig needs ${gbOf(p.memory.need_mb)} free to boot without paging (HUMANITY_RIG_MIN_FREE_MB; just build-slot-status shows the compiles holding it)`
    : p && p.lock
      ? `the rig boot lock, held by ${BL.describeHolder(p.lock)}`
      : `${p.name || "HumanityOS.exe"} pid ${p.pid}${p.exe ? ` (${p.exe})` : ""}`;

/// Is the process list faked (a test, or the manual red proof)?
const fakeInUse = () => !!lister || !!process.env.HUMANITY_MACHINE_GUARD_FAKE;

/// The boot lock this process would take: lib/boot-lock.js lockPath, which
/// leaves the real one alone under a fake process list or a test run unless
/// HUMANITY_RIG_LOCK names one.
const bootLockPath = () => BL.lockPath({ fake: fakeInUse() });

/// A span for a person: "60 min", "1.5 min", "30 s", "0.3 s".
function fmtSpan(ms) {
  if (!(ms >= 0)) return "?";
  if (ms >= 60 * 1000) return `${+(ms / 60000).toFixed(1)} min`;
  if (ms >= 10 * 1000) return `${Math.round(ms / 1000)} s`;
  return `${+(ms / 1000).toFixed(1)} s`;
}

/// Wait, bounded, for the machine to be free of contenders, then take the boot
/// lock (lib/boot-lock.js) so no other rig boots beside this one.
///
/// opt.own        what is ours (see foreignProcs). own.waitForBuilds: builds count as
///                contenders too (a run that measures timings); without it they do not
///                (2026-10-23: rigs wait for memory, not for "no builds"; see above)
/// opt.memoryMb   free physical memory to wait for (rigMinFreeMb(); 0 = none)
/// opt.timeoutMs  how long to wait before giving up (WAIT_MIN_DEFAULT)
/// opt.pollMs     how often to look (30 s; HUMANITY_MACHINE_GUARD_POLL_MS for a test)
/// opt.lock       false = do not take the boot lock (default: take it)
/// opt.lockFile   the lock file (default bootLockPath(): none under a fake list or a test)
/// opt.procs      false = look only at the boot lock, not at the process list
///                (spawnGame: the rig has done its own process check)
/// opt.rig        the name the lock records (default the running script's)
///
/// Returns { free, waited, waited_s, blockers } and NEVER throws, plus `lock`
/// (the record) when this call holds the boot lock, `limit_ms` when it gave up,
/// and `lock_error` when the lock file could not be written at all. `waited` says
/// whether it had to wait at all: waited_s is ROUNDED, so a wait under half a
/// second reads 0 and must not be taken for "no wait" (2026-10-04: probe-sweep
/// skipped its re-check after such a wait). The caller decides what a give-up
/// means; every rig that boots refuses (refuseBusy). Logs on entry and on exit
/// when it actually waited, because "how long did this sweep sit waiting" is
/// evidence about the machine, not noise - a sweep that waited 150 s ran next to
/// somebody, just not during.
function waitForFree(opt = {}) {
  const own = opt.own || {};
  const timeoutMs = opt.timeoutMs != null ? opt.timeoutMs : WAIT_MIN_DEFAULT * 60 * 1000;
  // HUMANITY_MACHINE_GUARD_POLL_MS: a test's shorter poll (with
  // HUMANITY_MACHINE_GUARD_FAKE, so a wait can be exercised in a second).
  const envPoll = Number(process.env.HUMANITY_MACHINE_GUARD_POLL_MS);
  const pollMs = opt.pollMs != null ? opt.pollMs : envPoll > 0 ? envPoll : 30 * 1000;
  const log = opt.log || ((m) => console.log(m));
  const label = opt.label || "boot";
  const lockFile = opt.lock === false ? null : opt.lockFile !== undefined ? opt.lockFile : bootLockPath();
  const t0 = Date.now();
  let took = null;
  // One look: the processes and the memory first (a rig never takes the lock while it
  // could not boot anyway), then the lock. Quiet means all three.
  const memoryMb = opt.memoryMb != null ? opt.memoryMb : rigMinFreeMb();
  const look = () => {
    const procs = opt.procs === false ? [] : foreignProcs({ ...own, gameOnly: !own.waitForBuilds });
    const short = opt.procs === false || !(memoryMb > 0) ? null : memoryShortfall(memoryMb);
    if (short) procs.push(short);
    if (procs.length || !lockFile) return { blockers: procs };
    const r = BL.tryAcquire(lockFile, { rig: opt.rig });
    if (r.ok) {
      took = r;
      return { blockers: [] };
    }
    if (r.error) return { blockers: [], error: r.error };
    return { blockers: [{ pid: r.holder.pid, name: "", exe: "", lock: r.holder }] };
  };
  const done = (extra) => {
    const out = { ...extra };
    if (took) {
      out.lock = took.record;
      if (took.stale) log(`[machine-guard] ${label}: took over a stale boot lock from ${BL.describeHolder(took.stale)} (that process is gone)`);
    }
    return out;
  };
  let seen = look();
  if (seen.error) return { free: false, waited: false, waited_s: 0, blockers: [], lock_error: seen.error, lock_file: lockFile };
  if (!seen.blockers.length) return done({ free: true, waited: false, waited_s: 0, blockers: [] });
  log(`[machine-guard] ${label}: WAITING - the machine is busy, ${seen.blockers.length} in the way:`);
  for (const p of seen.blockers) log(`[machine-guard]   ${describe(p)}`);
  log(`[machine-guard] one machine, one boot at a time. Polling every ${fmtSpan(pollMs)}, for up to ${fmtSpan(timeoutMs)}.`);
  while (Date.now() - t0 < timeoutMs) {
    sleepSync(Math.min(pollMs, Math.max(0, timeoutMs - (Date.now() - t0))));
    seen = look();
    const waited = Math.round((Date.now() - t0) / 1000);
    if (seen.error) return { free: false, waited: true, waited_s: waited, blockers: [], lock_error: seen.error, lock_file: lockFile };
    if (!seen.blockers.length) {
      log(`[machine-guard] ${label}: clear after ${waited} s.`);
      return done({ free: true, waited: true, waited_s: waited, blockers: [] });
    }
    log(`[machine-guard] ${label}: still waiting (${waited} s) on ${seen.blockers.map(describe).join("; ")}`);
  }
  const waited = Math.round((Date.now() - t0) / 1000);
  log(`[machine-guard] ${label}: GAVE UP after ${waited} s; still up: ${seen.blockers.map(describe).join("; ")}`);
  return { free: false, waited: true, waited_s: waited, blockers: seen.blockers, limit_ms: timeoutMs };
}

// ── A WAIT THAT GAVE UP: ONE REFUSAL FOR EVERY RIG (2026-10-21) ─────────────
//
// verify-runtime's 12-minute gate used to run its clock over the machine wait
// in front of the boot, so on a busy afternoon (other agents building) the
// gate killed its own probe while it was still waiting and printed "FAIL ...
// A hang here is a boot or world-entry hang, not slowness". Nothing had hung
// and nothing had booted: two false failures on 2026-10-21. A wait that gives
// up is not a verdict on the build, so every rig now says so the same way, and
// exits 1 (refused: nothing ran), never 2 (a check failed).

/// The refusal for a wait that gave up (or a lock that could not be written).
///   opt.rig   the rig's name, first on the line
///   opt.hint  lines saying what the reader can do (a rig's own flags)
function refusalLines(w, opt = {}) {
  const rig = opt.rig || "rig";
  if (w && w.lock_error) {
    return [
      `${rig}: REFUSED: the machine-wide rig boot lock could not be written (${w.lock_error}); nothing was booted.`,
      "Without it this rig could boot beside another one on the one GPU. The lock lives in",
      `${w.lock_file || BL.DEFAULT_PATH}; HUMANITY_RIG_LOCK=<path> names another file.`,
    ];
  }
  const lines = [`${rig}: REFUSED: the machine never went quiet in ${fmtSpan(w && w.limit_ms)}; nothing was booted.`];
  const blockers = (w && w.blockers) || [];
  if (blockers.length) {
    lines.push(`Still up after ${w.waited_s} s of waiting:`);
    for (const b of blockers) lines.push(`  ${describe(b)}`);
  }
  lines.push("This says nothing about the build: it never ran. One machine, one boot at a time (CLAUDE.md).");
  if (blockers.some((b) => b.lock && b.lock.pid)) {
    const b = blockers.find((x) => x.lock && x.lock.pid);
    lines.push(`If that rig is a leftover nobody is waiting on: taskkill //PID ${b.lock.pid} (its lock then goes stale and is taken over).`);
  }
  for (const h of opt.hint || []) lines.push(h);
  return lines;
}

/// Print the refusal, tell a parent gate (signalPhase), exit 1.
function refuseBusy(w, opt = {}) {
  const lines = refusalLines(w, opt);
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  signalPhase("refused", {
    rig: opt.rig || null,
    reason: w && w.lock_error ? "lock" : "machine",
    headline: lines[0],
    waited_s: (w && w.waited_s) || 0,
    limit_ms: (w && w.limit_ms) || null,
    blockers: ((w && w.blockers) || []).map(describe),
  });
  process.exit(1);
}

// ── TELLING A PARENT GATE WHAT PHASE A RIG IS IN (2026-10-21) ────────────────
//
// verify-runtime runs probe-sweep as a child and times it. The child's machine
// wait must not count against the gate, so the child writes its phase to the
// file named by HUMANITY_RIG_BOOT_SIGNAL (set by the parent; absent in a plain
// run, when this does nothing): "booted" from lib/game-launch.js spawnGame the
// moment the game process exists, "refused" from refuseBusy. Written to a temp
// name and renamed, so the parent never reads half a file.

const BOOT_SIGNAL_ENV = "HUMANITY_RIG_BOOT_SIGNAL";

/// Record a phase for the parent gate, when there is one. Never throws.
function signalPhase(phase, extra = {}) {
  const file = process.env[BOOT_SIGNAL_ENV];
  if (!file) return false;
  try {
    const rec = { phase, at: new Date().toISOString(), pid: process.pid, ...extra };
    const tmp = `${file}.${process.pid}.tmp`;
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(tmp, JSON.stringify(rec, null, 2));
    fs.renameSync(tmp, file);
    return true;
  } catch {
    return false;
  }
}

/// Read a phase record a child wrote, or null.
function readPhase(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

// Blocking sleep. The guard runs in the middle of an async sweep but must not
// hand control back to the loop while it waits (the boot would proceed).
function sleepSync(ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    try {
      // 200 ms slices via Atomics: no busy spin, no dependency.
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, Math.min(200, end - Date.now()));
    } catch {
      /* SharedArrayBuffer unavailable: fall through to a short spin */
    }
  }
}

/// Merge two contender observations (before + after a capture) into the set of
/// pids that were up at either end. A process that arrived and exited inside
/// the window still shows up in one of the two, which is why the capture is
/// guarded on BOTH sides rather than sampled once. That is not a corner case:
/// in the V1 incident the other rig was already gone by the time the sweep
/// ended.
function mergeForeign(...lists) {
  const byPid = new Map();
  for (const list of lists) {
    for (const p of list || []) if (!byPid.has(p.pid)) byPid.set(p.pid, p);
  }
  return [...byPid.values()];
}

/// Mark one capture record from a before/after pair of contender observations.
/// Lives HERE rather than inline in probe-sweep so the marking itself is
/// testable without booting anything.
///
/// Returns true when the record was marked contaminated.
function markCapture(rec, before, after, log = console.log) {
  const seen = mergeForeign(before, after);
  if (!seen.length) return false;
  rec.contaminated = true;
  rec.contaminated_by = seen.map((p) => ({ pid: p.pid, name: p.name || GAME, exe: p.exe }));
  // Name the KIND of contention, because the fix differs: a second renderer is
  // a GPU problem, a build is a CPU problem that inflates only the cpu.* stages
  // and can leave the GPU columns looking perfectly normal.
  const games = seen.filter((p) => !p.name || isGameName(p.name));
  const builds = seen.filter((p) => p.name && !isGameName(p.name));
  log(`  !! CONTAMINATED: ${seen.length} process(es) shared this machine during the capture:`);
  for (const p of seen) log(`  !!   ${describe(p)}`);
  if (games.length) log(`  !! A second renderer means the gpu.* figures are not this build's.`);
  if (builds.length) {
    log(`  !! A build (${builds.map((p) => p.name).join(", ")}) means the cpu.* stages and the frame time`);
    log(`  !! are inflated even when the gpu.* columns look normal - the 6.6 fps / 15 ms GPU sum shape.`);
  }
  log(`  !! perf-report will refuse to grade this capture. Wait for the machine, then re-run the vantage.`);
  return true;
}

// ── HOLDING BUILDS BACK DURING A CAPTURE (BUG-161) ──────────────────────────
//
// Waiting for builds to finish (waitForFree) is right for a frame-time reading
// and wrong for a rig whose checks are about BEHAVIOUR: in a shared checkout
// somebody is nearly always compiling, and a rig that waited would never run.
// But a rig that ignores them reads a starved machine as a regression: on
// 2026-10-05 v0.1459.0's first co-presence --plots runs, with ten agents
// compiling, drew the game at 6 to 16 fps and failed the steady-speed checks in
// three of six orders. The rerun with every cargo, rustc and link process held
// at BelowNormal priority (a hand-run scratchpad loop, deprioritize-builds.ps1)
// drew 16 to 24 fps and passed 88/88. holdBuilds() is that loop, in the guard.
//
// What it may do, and all it may do:
//   - LOWER the priority class of a BUILD process (BUILDS) to BelowNormal. The
//     builds keep running; they just yield the CPU to the game while it is
//     recorded. Each pid is re-checked to still be a build, by the same name the
//     listing used, in the same PowerShell call that lowers it, because a pid
//     listed a moment ago may belong to something else by then.
//   - Leave a build that is already at BelowNormal or Idle as it is. Never raise.
//   - On release(), put back exactly what it lowered (matched by pid AND start
//     time, so a reused pid is never touched), plus the children a lowered build
//     started meanwhile, which Windows starts at their parent's BelowNormal (the
//     only way a child of a build becomes BelowNormal without anyone asking).
//     A priority someone else changed in the meantime is left as they set it.
//   - release() also runs when the process exits (an error, Ctrl+C, the rig's
//     own process.exit). A hard kill of the rig (taskkill /F) cannot run any code:
//     the builds then finish at BelowNormal, slower and otherwise unharmed.
// It never kills or suspends anything and changes no system setting. PowerShell
// is called with -NoProfile and -NonInteractive, and its script carries only
// process ids, start times (digits) and priority-class names from a fixed list:
// nothing a process list or a person typed ever reaches it.

/// The priority classes, lowest first.
const PRIORITY_CLASSES = ["Idle", "BelowNormal", "Normal", "AboveNormal", "High", "RealTime"];
/// What the hold lowers a build to.
const HELD_CLASS = "BelowNormal";

const asPid = (v) => {
  const n = Number(v);
  if (!Number.isInteger(n) || n <= 0 || String(v).trim() !== String(n)) throw new Error(`machine-guard: not a process id: ${JSON.stringify(v)}`);
  return n;
};

/// The PowerShell that looks at (and, with lower, lowers) the given pids. Prints
/// one line per pid: `pid|name|parent pid|start time|class|what`, where `what` is
/// lowered, left (not lowered: already at or below BelowNormal, or only looking),
/// notbuild (that pid is something else now), gone, or denied. The name is the
/// Win32_Process name, the one the guard's own listing used (a rustup proxy says
/// "rustup" to Get-Process and "rustc.exe" to the process list); the class and the
/// start time come from Get-Process. Single-quoted throughout, no double quotes,
/// so nothing in it needs escaping on its way through a command line.
function priorityScript(pids, { lower = true } = {}) {
  const ids = (Array.isArray(pids) ? pids : []).map(asPid);
  if (!ids.length) throw new Error("machine-guard: no process id to look at");
  const names = BUILDS.map((n) => `'${n.toLowerCase()}'`).join(",");
  const above = PRIORITY_CLASSES.slice(PRIORITY_CLASSES.indexOf("Normal")).map((c) => `'${c}'`).join(",");
  const out = (tail) => `[string]$id + '|' + $n + '|' + $i[1] + '|' + $s + '|' + $c + '|${tail}'`;
  return [
    `$names = @(${names})`,
    `$ids = @(${ids.join(",")})`,
    `$info = @{}`,
    `Get-CimInstance Win32_Process -Filter '${ids.map((i) => `ProcessId=${i}`).join(" OR ")}' | ForEach-Object { $info[[int]$_.ProcessId] = @([string]$_.Name, [int]$_.ParentProcessId) }`,
    `foreach ($id in $ids) { ` +
      `$i = $info[$id]; ` +
      `if ($null -eq $i) { [string]$id + '|||||gone'; continue }; ` +
      `$n = $i[0].ToLower(); ` +
      `if ($names -notcontains $n) { [string]$id + '|' + $n + '||||notbuild'; continue }; ` +
      `try { ` +
      `$p = Get-Process -Id $id -ErrorAction Stop; ` +
      `$c = [string]$p.PriorityClass; ` +
      `$s = $p.StartTime.ToFileTimeUtc(); ` +
      (lower
        ? `if (@(${above}) -contains $c) { $p.PriorityClass = '${HELD_CLASS}'; ${out("lowered")} } else { ${out("left")} } `
        : `${out("left")} `) +
      `} catch { [string]$id + '|' + $n + '||||denied' } }`,
  ].join("; ");
}

/// The PowerShell that puts held processes back. `entries`: [{ pid, start, to }].
/// Each is set to `to` only if it is still the same process (its start time) and
/// still at BelowNormal (what the hold set); otherwise it is left alone. Prints
/// `pid|restored|class`, `pid|changed|class`, `pid|other` or `pid|gone`.
function restoreScript(entries) {
  const list = (Array.isArray(entries) ? entries : []).map((e) => {
    const pid = asPid(e.pid);
    if (!/^\d+$/.test(String(e.start))) throw new Error(`machine-guard: not a start time: ${JSON.stringify(e.start)}`);
    if (!PRIORITY_CLASSES.includes(e.to)) throw new Error(`machine-guard: not a priority class: ${JSON.stringify(e.to)}`);
    return `'${pid}:${e.start}:${e.to}'`;
  });
  if (!list.length) throw new Error("machine-guard: no process id to put back");
  return [
    `$want = @(${list.join(",")})`,
    `foreach ($e in $want) { ` +
      `$f = $e.Split(':'); $id = [int]$f[0]; ` +
      `try { ` +
      `$p = Get-Process -Id $id -ErrorAction Stop; ` +
      `if ([string]$p.StartTime.ToFileTimeUtc() -ne $f[1]) { [string]$id + '|other'; continue }; ` +
      `$c = [string]$p.PriorityClass; ` +
      `if ($c -ne '${HELD_CLASS}') { [string]$id + '|changed|' + $c; continue }; ` +
      `$p.PriorityClass = $f[2]; ` +
      `[string]$id + '|restored|' + $f[2] ` +
      `} catch { [string]$id + '|gone' } }`,
  ].join("; ");
}

/// The arguments PowerShell is started with: no profile, no prompts, the script.
const powerShellArgs = (script) => ["-NoProfile", "-NonInteractive", "-Command", script];

/// The real runner: one PowerShell call, its output, or "" when it could not run
/// (not Windows, no PowerShell). Hidden window: never a console flash, never focus.
function runPowerShell(req) {
  try {
    return execFileSync("powershell", powerShellArgs(req.script), { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], windowsHide: true, timeout: 60000 });
  } catch {
    return "";
  }
}

/// Parse priorityScript's output into rows.
function parsePriorityRows(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      const [pid, name, ppid, start, cls, what] = l.split("|");
      return { pid: Number(pid), name: name || "", ppid: ppid ? Number(ppid) : null, start: start || "", cls: cls || "", what: what || "" };
    })
    .filter((r) => Number.isInteger(r.pid) && r.pid > 0);
}

/// Parse restoreScript's output into rows.
function parseRestoreRows(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      const [pid, what, cls] = l.split("|");
      return { pid: Number(pid), what: what || "", cls: cls || "" };
    })
    .filter((r) => Number.isInteger(r.pid) && r.pid > 0);
}

const isBuildName = (name) => BUILDS.some((b) => b.toLowerCase() === String(name || "").toLowerCase());

/// One line for a hold's record (the manifest's builds_held), for a verdict.
function describeHold(h) {
  if (!h) return "(no record)";
  const lowered = h.lowered || [];
  const inherited = h.inherited || [];
  const restored = h.restored || [];
  const notBack = h.not_restored || [];
  if (!lowered.length && !inherited.length) {
    const low = (h.left_low || []).length;
    return `${h.label}: no build was running at a priority above ${HELD_CLASS}${low ? ` (${low} already at ${HELD_CLASS} or Idle, left as they were)` : ""}`;
  }
  const names = lowered.map((x) => `${x.name} pid ${x.pid} (was ${x.from})`).join(", ");
  return (
    `${h.label}: ${lowered.length} lowered, ${inherited.length} started at ${HELD_CLASS} by one of them, ${restored.length} put back` +
    (notBack.length ? `; not put back: ${notBack.map((x) => `pid ${x.pid} ${x.why}`).join(", ")}` : "") +
    ` [${names}]`
  );
}

/// Hold every running build at BelowNormal until release() (BUG-161).
///
/// opt.label      what the capture is, for the log and the record
/// opt.log        line logger (console.log)
/// opt.resweepMs  how often to look again for builds started since (10 s; 0 = never).
///                Children of a held build need no look: Windows starts them at
///                BelowNormal already. The look is for brand-new builds.
/// opt.run        the priority runner (runPowerShell). With a FAKE process list
///                (setLister or HUMANITY_MACHINE_GUARD_FAKE) the real runner is
///                only used when handed in on purpose: a fake list's pids are
///                invented, and a real process could own one.
///
/// Returns { release(), sweep() }. release() is safe to call any number of times
/// and returns the hold's record: { label, lowered, inherited, left_low,
/// restored, not_restored }. It never throws.
function holdBuilds(opt = {}) {
  const label = opt.label || "capture";
  const log = opt.log || ((m) => console.log(m));
  const resweepMs = opt.resweepMs != null ? opt.resweepMs : 10 * 1000;
  const fakeList = !!lister || !!process.env.HUMANITY_MACHINE_GUARD_FAKE;
  const run = opt.run || (fakeList ? null : runPowerShell);
  const held = new Map(); // pid -> { pid, name, start, from, how, parent }
  const seen = new Set(); // pids already looked at, so each is inspected once
  const leftLow = [];
  let released = false;
  let record = null;
  let timer = null; // the periodic look, set at the end

  const builds = () => listProcs().filter((p) => isBuildName(p.name));
  // Look at builds not seen before; with `lower`, lower those above BelowNormal.
  // A row at BelowNormal whose parent is held (or is itself such a child, found in
  // this same look: a link.exe under an inherited rustc) inherited the hold. A
  // parent lowered in THIS look does not count: its BelowNormal children were
  // BelowNormal before we touched it, so they are not ours to put back.
  const look = (lower) => {
    const fresh = builds().filter((p) => !seen.has(p.pid));
    if (!fresh.length || !run) return [];
    for (const p of fresh) seen.add(p.pid);
    const rows = parsePriorityRows(run({ kind: lower ? "lower" : "inspect", lower, pids: fresh.map((p) => p.pid), script: priorityScript(fresh.map((p) => p.pid), { lower }) }));
    const added = [];
    // Inherited first, while `held` holds only what earlier looks held plus the
    // children found here: this look's own lowered builds are added after.
    const low = rows.filter((r) => r.what === "left" && r.cls === HELD_CLASS);
    for (let grew = true; grew; ) {
      grew = false;
      for (const r of low) {
        const parent = held.get(r.ppid);
        if (held.has(r.pid) || !parent) continue;
        const e = { pid: r.pid, name: r.name, start: r.start, from: parent.from, how: "inherited", parent: r.ppid };
        held.set(r.pid, e);
        added.push(e);
        grew = true;
      }
    }
    for (const r of rows) {
      if (r.what === "lowered") {
        const e = { pid: r.pid, name: r.name, start: r.start, from: r.cls, how: "lowered" };
        held.set(r.pid, e);
        added.push(e);
      } else if (r.what === "left" && lower && !held.has(r.pid)) {
        leftLow.push({ pid: r.pid, name: r.name, cls: r.cls });
      }
    }
    return added;
  };
  const sweep = () => {
    if (released) return [];
    let added = [];
    try {
      added = look(true);
    } catch (e) {
      log(`[machine-guard] ${label}: could not look at the builds (${e.message}); the capture runs without holding them`);
    }
    const lowered = added.filter((e) => e.how === "lowered");
    if (lowered.length) log(`[machine-guard] ${label}: holding ${lowered.length} build process(es) at ${HELD_CLASS} for the capture (they keep building, behind the game): ${lowered.map((e) => `${e.name} pid ${e.pid} (was ${e.from})`).join(", ")}`);
    return added;
  };
  const putBack = (entries) => {
    const want = entries.filter((e) => /^\d+$/.test(String(e.start)) && PRIORITY_CLASSES.includes(e.from));
    if (!want.length || !run) return [];
    return parseRestoreRows(run({ kind: "restore", entries: want.map((e) => ({ pid: e.pid, start: e.start, to: e.from })), script: restoreScript(want.map((e) => ({ pid: e.pid, start: e.start, to: e.from }))) }));
  };

  const onExit = () => release();
  const signals = [["SIGINT", 130], ["SIGTERM", 143], ["SIGBREAK", 149], ["SIGHUP", 129]];
  const onSignal = {};
  for (const [sig, code] of signals) {
    onSignal[sig] = () => {
      release();
      // Nobody else listening: end the process the way the signal would have,
      // rather than swallow it (a listener stops Node's own default exit).
      if (process.listenerCount(sig) === 0) process.exit(code);
    };
  }

  function release() {
    if (released) return record;
    released = true;
    if (timer) clearInterval(timer);
    process.removeListener("exit", onExit);
    for (const [sig] of signals) process.removeListener(sig, onSignal[sig]);
    const rows = [];
    try {
      // 1. Everything held goes back to what it was.
      rows.push(...putBack([...held.values()]));
      // 2. Children a held build started since the last look were born at
      //    BelowNormal: find them and put them back too. Their parents are back
      //    already, so anything started from now on starts at their own class;
      //    a few rounds catch a child's child.
      for (let round = 0; round < 3; round++) {
        const kids = look(false);
        if (!kids.length) break;
        rows.push(...putBack(kids));
      }
    } catch (e) {
      log(`[machine-guard] ${label}: putting the builds back failed part way (${e.message})`);
    }
    const values = [...held.values()];
    const restored = rows.filter((r) => r.what === "restored").map((r) => r.pid);
    const notRestored = rows
      .filter((r) => r.what !== "restored")
      .map((r) => ({ pid: r.pid, why: r.what === "changed" ? `its priority had been changed to ${r.cls} meanwhile, left as set` : r.what === "other" ? "its pid now belongs to another process, left alone" : "it had finished" }));
    record = {
      label,
      lowered: values.filter((e) => e.how === "lowered").map(({ pid, name, from }) => ({ pid, name, from })),
      inherited: values.filter((e) => e.how === "inherited").map(({ pid, name, parent }) => ({ pid, name, parent })),
      left_low: leftLow,
      restored,
      not_restored: notRestored,
    };
    if (values.length) {
      const gone = notRestored.filter((x) => x.why === "it had finished").length;
      log(`[machine-guard] ${label} over: put back ${restored.length} build process(es) to their own priority` + (gone ? ` (${gone} had finished)` : "") + (notRestored.length - gone ? `; ${notRestored.length - gone} left alone (changed by someone else, or a reused pid)` : ""));
    }
    return record;
  }

  if (!run) {
    log(`[machine-guard] ${label}: FAKE process list in use: build priorities left alone (no runner handed in)`);
  } else {
    process.on("exit", onExit);
    for (const [sig] of signals) process.on(sig, onSignal[sig]);
  }
  const first = sweep();
  if (run && !first.length) {
    log(`[machine-guard] ${label}: no build running above ${HELD_CLASS}; nothing to hold` + (leftLow.length ? ` (${leftLow.length} already at ${HELD_CLASS} or Idle, left as they are)` : ""));
  }
  timer = run && resweepMs > 0 ? setInterval(sweep, resweepMs) : null;
  if (timer && timer.unref) timer.unref(); // the hold never keeps a finished script alive
  return { release, sweep };
}

module.exports = {
  WATCHED,
  GAME,
  isGameName,
  // 2026-10-22: the operator's own GPU-heavy programs: a picture or timing rig waits for
  // them, a behaviour rig runs beside them in low impact (scripts/lib/rig-needs.js).
  OPERATOR_APPS,
  isOperatorApp,
  isOperatorGame,
  operatorApps,
  blockingInstances,
  parseProcs,
  listProcs,
  listInstances,
  foreignProcs,
  waitForFree,
  // 2026-10-21: the boot lock, the one refusal, the phase a parent gate reads.
  WAIT_MIN_DEFAULT,
  fmtSpan,
  // 2026-10-23: rigs wait for free physical memory, not for builds.
  RIG_MIN_FREE_MB_DEFAULT,
  rigMinFreeMb,
  freeMemMb,
  setFreeMem,
  memoryShortfall,
  bootLockPath,
  refusalLines,
  refuseBusy,
  BOOT_SIGNAL_ENV,
  signalPhase,
  readPhase,
  mergeForeign,
  markCapture,
  describe,
  setLister,
  psQuery,
  // BUG-161: builds held at BelowNormal for the length of a capture.
  BUILDS,
  PRIORITY_CLASSES,
  holdBuilds,
  describeHold,
  priorityScript,
  restoreScript,
  powerShellArgs,
  runPowerShell,
};
