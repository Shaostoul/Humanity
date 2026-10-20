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

const fs = require("fs");
const { execSync, execFileSync } = require("child_process");

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

/// The build tools among WATCHED: everything watched that is not the game. One
/// list of what a build is, so a build added to WATCHED is also held back by
/// holdBuilds (BUG-161), and nothing else ever is.
const BUILDS = WATCHED.filter((n) => !isGameName(n));

// Windows query. ExecutablePath matters as much as the pid for the game: it is
// what separates our own rig copy (ours by construction - only sweeps launch
// from a rig dir) from the operator's game or another agent's rig.
function psQuery() {
  const filter = WATCHED.map((n) => (n === GAME ? `Name LIKE '%${n}'` : `Name='${n}'`)).join(" or ");
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
  return listProcs().filter((p) => !p.name || isGameName(p.name));
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
function foreignProcs(own = {}) {
  const pids = new Set((own.pids || []).map(Number));
  const exe = own.exe || "";
  return listProcs().filter((p) => {
    if (pids.has(p.pid)) return false;
    if (isGameName(p.name) || !p.name) return !sameExe(p.exe, exe);
    return !own.gameOnly; // a build: a contender unless only games count
  });
}

/// One-line description of a contender, for a log line or a manifest note.
const describe = (p) => `${p.name || "HumanityOS.exe"} pid ${p.pid}${p.exe ? ` (${p.exe})` : ""}`;

/// Wait, bounded, for the machine to be free of contenders.
///
/// Returns { free, waited, waited_s, blockers } and NEVER throws. `waited` says whether it
/// had to wait at all: waited_s is ROUNDED, so a wait under half a second reads 0 and must not
/// be taken for "no wait" (2026-10-04: probe-sweep skipped its re-check after such a wait).
/// the caller decides
/// whether a timeout is fatal (verify-screens refuses, probe-sweep proceeds but
/// records the wait). Logs on entry and on exit when it actually waited,
/// because "how long did this sweep sit waiting" is evidence about the machine,
/// not noise - a sweep that waited 150 s ran next to somebody, just not during.
function waitForFree(opt = {}) {
  const own = opt.own || {};
  const timeoutMs = opt.timeoutMs != null ? opt.timeoutMs : 40 * 60 * 1000; // machine rule: up to 40 min
  // HUMANITY_MACHINE_GUARD_POLL_MS: a test's shorter poll (with
  // HUMANITY_MACHINE_GUARD_FAKE, so a wait can be exercised in a second).
  const envPoll = Number(process.env.HUMANITY_MACHINE_GUARD_POLL_MS);
  const pollMs = opt.pollMs != null ? opt.pollMs : envPoll > 0 ? envPoll : 30 * 1000;
  const log = opt.log || ((m) => console.log(m));
  const label = opt.label || "boot";
  const t0 = Date.now();
  let blockers = foreignProcs(own);
  if (!blockers.length) return { free: true, waited: false, waited_s: 0, blockers: [] };
  log(`[machine-guard] ${label}: WAITING - ${blockers.length} process(es) competing for this machine:`);
  for (const p of blockers) log(`[machine-guard]   ${describe(p)}`);
  log(`[machine-guard] one machine, one measurement. Polling every ${Math.round(pollMs / 1000)} s.`);
  while (Date.now() - t0 < timeoutMs) {
    sleepSync(pollMs);
    blockers = foreignProcs(own);
    const waited = Math.round((Date.now() - t0) / 1000);
    if (!blockers.length) {
      log(`[machine-guard] ${label}: clear after ${waited} s.`);
      return { free: true, waited: true, waited_s: waited, blockers: [] };
    }
    log(`[machine-guard] ${label}: still waiting (${waited} s) on ${blockers.map(describe).join("; ")}`);
  }
  const waited = Math.round((Date.now() - t0) / 1000);
  log(`[machine-guard] ${label}: GAVE UP after ${waited} s; still up: ${blockers.map(describe).join("; ")}`);
  return { free: false, waited: true, waited_s: waited, blockers };
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
