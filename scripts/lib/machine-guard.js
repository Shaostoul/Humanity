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

const fs = require("fs");
const { execSync } = require("child_process");

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

// Windows query. ExecutablePath matters as much as the pid for the game: it is
// what separates our own rig copy (ours by construction - only sweeps launch
// from a rig dir) from the operator's game or another agent's rig.
function psQuery() {
  const filter = WATCHED.map((n) => `Name='${n}'`).join(" or ");
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
/// "no second renderer", which a compile does not violate.
function listInstances() {
  return listProcs().filter((p) => !p.name || sameName(p.name, GAME));
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
function foreignProcs(own = {}) {
  const pids = new Set((own.pids || []).map(Number));
  const exe = own.exe || "";
  return listProcs().filter((p) => {
    if (pids.has(p.pid)) return false;
    if (sameName(p.name, GAME) || !p.name) return !sameExe(p.exe, exe);
    return true; // a build: always a contender
  });
}

/// One-line description of a contender, for a log line or a manifest note.
const describe = (p) => `${p.name || "HumanityOS.exe"} pid ${p.pid}${p.exe ? ` (${p.exe})` : ""}`;

/// Wait, bounded, for the machine to be free of contenders.
///
/// Returns { free, waited_s, blockers } and NEVER throws: the caller decides
/// whether a timeout is fatal (verify-screens refuses, probe-sweep proceeds but
/// records the wait). Logs on entry and on exit when it actually waited,
/// because "how long did this sweep sit waiting" is evidence about the machine,
/// not noise - a sweep that waited 150 s ran next to somebody, just not during.
function waitForFree(opt = {}) {
  const own = opt.own || {};
  const timeoutMs = opt.timeoutMs != null ? opt.timeoutMs : 40 * 60 * 1000; // machine rule: up to 40 min
  const pollMs = opt.pollMs != null ? opt.pollMs : 30 * 1000;
  const log = opt.log || ((m) => console.log(m));
  const label = opt.label || "boot";
  const t0 = Date.now();
  let blockers = foreignProcs(own);
  if (!blockers.length) return { free: true, waited_s: 0, blockers: [] };
  log(`[machine-guard] ${label}: WAITING - ${blockers.length} process(es) competing for this machine:`);
  for (const p of blockers) log(`[machine-guard]   ${describe(p)}`);
  log(`[machine-guard] one machine, one measurement. Polling every ${Math.round(pollMs / 1000)} s.`);
  while (Date.now() - t0 < timeoutMs) {
    sleepSync(pollMs);
    blockers = foreignProcs(own);
    const waited = Math.round((Date.now() - t0) / 1000);
    if (!blockers.length) {
      log(`[machine-guard] ${label}: clear after ${waited} s.`);
      return { free: true, waited_s: waited, blockers: [] };
    }
    log(`[machine-guard] ${label}: still waiting (${waited} s) on ${blockers.map(describe).join("; ")}`);
  }
  const waited = Math.round((Date.now() - t0) / 1000);
  log(`[machine-guard] ${label}: GAVE UP after ${waited} s; still up: ${blockers.map(describe).join("; ")}`);
  return { free: false, waited_s: waited, blockers };
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
  const games = seen.filter((p) => !p.name || sameName(p.name, GAME));
  const builds = seen.filter((p) => p.name && !sameName(p.name, GAME));
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

module.exports = {
  WATCHED,
  GAME,
  parseProcs,
  listProcs,
  listInstances,
  foreignProcs,
  waitForFree,
  mergeForeign,
  markCapture,
  describe,
  setLister,
  psQuery,
};
