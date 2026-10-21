// ONE BOOT AT A TIME, machine-wide (2026-10-21).
//
// The machine guard (machine-guard.js waitForFree) makes a rig wait while
// another game or a build is running. That left one hole, found by BUG-140's
// fixer on 2026-10-21: when the builds on the machine end, EVERY rig that was
// waiting on them sees a quiet machine in the same 30 s poll, and they all
// boot together. Two games then compete for the one 12 GB card: it saw a wgpu
// "Not enough memory left" on a 1.47 GB buffer and a frame loop that stopped
// logging 20 s after world entry, and both runs passed when rerun alone. The
// guard could not see it coming, because at the moment each rig looked, no
// other game existed yet: they were all still copying their exes.
//
// So a rig that is about to boot takes this lock first, and holds it until the
// rig's process ends. The guard counts another rig's held lock as "not quiet",
// and lib/game-launch.js spawnGame, the one place every rig starts the game,
// will not start a game without it. Two rigs can no longer boot at once.
//
// THE LOCK FILE: %LOCALAPPDATA%\HumanityOS\rig-boot.lock (beside the disk
// guard's files), machine-wide on purpose. A lock under the repo's .probe-rig/
// would not be: every worktree has its own .probe-rig/, and the rigs that woke
// together were in different worktrees. It holds JSON naming the holder: its
// pid, the start time Windows gives that pid, the rig, its folder and since
// when. HUMANITY_RIG_LOCK=<path> names another file (the tests do this, so a
// test never takes or waits on the real one).
//
// STALE LOCKS: a rig killed hard (taskkill /F, a closed terminal) cannot remove
// its lock. A lock whose pid is gone is stale and the next rig takes it over.
// So is one whose pid now belongs to a different process: Windows reuses pids,
// so the holder's start time is recorded and compared, or a dead rig's lock
// could be "held" for hours by whatever process got its number. The takeover
// itself happens under a second, short-lived file (<lock>.takeover) and only if
// the lock still holds exactly what was judged stale, so a rig that just took a
// fresh lock can never have it deleted by a slower rig that saw the old one.
//
// NO LOCK WITH A FAKE PROCESS LIST: when the machine guard's process list is
// faked (a test, or the manual red proof), the real lock is left alone unless
// HUMANITY_RIG_LOCK names one; likewise under `node --test` (NODE_TEST_CONTEXT).
// A test must never block a real rig, or be blocked by one.

"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");

const ENV_PATH = "HUMANITY_RIG_LOCK";
const DEFAULT_PATH = path.join(
  process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local"),
  "HumanityOS",
  "rig-boot.lock"
);
/** An unreadable lock this young is a holder between its create and its write. */
const YOUNG_MS = 10 * 1000;
/** The takeover file is held for milliseconds; one this old was left by a crash. */
const TAKEOVER_STALE_MS = 30 * 1000;
/** Two readings of one process's start time agree to the 100 ns; a second covers rounding. */
const START_SLACK_MS = 1000;
const FILETIME_EPOCH_MS = 11644473600000;

/** The lock file to use, or null for none (a fake process list or a test run, unless one is named). */
function lockPath({ fake = false } = {}) {
  if (process.env[ENV_PATH]) return path.resolve(process.env[ENV_PATH]);
  if (fake || process.env.NODE_TEST_CONTEXT) return null;
  return DEFAULT_PATH;
}

/** Is a process with this pid running? (EPERM: it exists, we may not signal it.) */
function pidAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
}

/** When Windows says this pid started, in ms since 1970, or null when it cannot say. */
function osStartMs(pid) {
  if (process.platform !== "win32") return null;
  const n = Number(pid);
  if (!Number.isInteger(n) || n <= 0) return null;
  try {
    const out = execFileSync(
      "powershell",
      ["-NoProfile", "-NonInteractive", "-Command", `(Get-Process -Id ${n} -ErrorAction Stop).StartTime.ToFileTimeUtc()`],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], windowsHide: true, timeout: 30000 }
    ).trim();
    if (!/^\d+$/.test(out)) return null;
    return Number(BigInt(out) / 10000n) - FILETIME_EPOCH_MS;
  } catch {
    return null;
  }
}

/** The checks a holder is judged by; a test may hand in its own. */
const defaultProbe = { pidAlive, osStartMs };

const myStart = new WeakMap(); // this process's start time, looked up once per probe
function myStartMs(probe) {
  if (!myStart.has(probe)) myStart.set(probe, probe.osStartMs(process.pid));
  return myStart.get(probe);
}

/** Is the holder this record names still the process that wrote it? */
function holderLive(rec, probe = defaultProbe) {
  if (!rec || !Number.isInteger(rec.pid) || rec.pid <= 0) return false;
  if (!probe.pidAlive(rec.pid)) return false;
  if (rec.start_ms == null) return true; // nothing more to compare: alive is held
  const now = probe.osStartMs(rec.pid);
  if (now == null) return true; // cannot tell: err on the side of held
  return Math.abs(now - rec.start_ms) <= START_SLACK_MS; // another start time: the pid was reused
}

/** Read the lock: { missing } | { raw, record (null when unreadable), mtimeMs }. */
function readLock(file) {
  let raw;
  let st;
  try {
    raw = fs.readFileSync(file, "utf8");
    st = fs.statSync(file);
  } catch (e) {
    if (e.code === "ENOENT") return { missing: true };
    throw e;
  }
  let record = null;
  try {
    const r = JSON.parse(raw);
    if (r && Number.isInteger(r.pid)) record = r;
  } catch {}
  return { missing: false, raw, record, mtimeMs: st.mtimeMs };
}

/** Remove a stale lock, under the takeover file, only if it still holds `rawSeen`.
 *  true = the lock is gone (removed, or already gone); false = try again next look. */
function removeStale(file, rawSeen) {
  const mtx = `${file}.takeover`;
  try {
    fs.closeSync(fs.openSync(mtx, "wx"));
  } catch (e) {
    if (e.code !== "EEXIST") return false;
    try {
      if (Date.now() - fs.statSync(mtx).mtimeMs > TAKEOVER_STALE_MS) fs.unlinkSync(mtx);
    } catch {}
    return false;
  }
  try {
    let now;
    try {
      now = fs.readFileSync(file, "utf8");
    } catch {
      return true; // released meanwhile
    }
    if (now !== rawSeen) return false; // replaced since it was judged: not ours to remove
    fs.unlinkSync(file);
    return true;
  } finally {
    try {
      fs.unlinkSync(mtx);
    } catch {}
  }
}

const held = new Map(); // lock file -> its record, while this process holds it
let armed = false;

/** Remove this process's lock on `file` (every lock it holds when no file is named), if the
 *  file still names this process. Safe to call any number of times. */
function release(file) {
  const files = file ? [path.resolve(file)] : [...held.keys()];
  let any = false;
  for (const f of files) {
    if (!held.delete(f)) continue;
    any = true;
    try {
      const cur = readLock(f);
      if (cur.record && cur.record.pid === process.pid) fs.unlinkSync(f);
    } catch {}
  }
  return any;
}

function hold(file, record) {
  held.set(path.resolve(file), record);
  if (armed) return;
  armed = true;
  // Every way a process ends that can run code ends here; a hard kill leaves a
  // stale lock, which the next rig takes over (pid gone).
  process.on("exit", () => release());
}

/**
 * Take the lock if it is free (or stale, or already ours). Never waits, never throws.
 *   opt.rig    who is asking (the lock names it), default the running script's name
 *   opt.probe  the holder checks (tests)
 * Returns
 *   { ok: true, taken: bool, record, stale }  held now (taken = this call created it;
 *                                             stale = the dead holder it replaced, if any)
 *   { ok: false, holder }                     another live rig holds it
 *   { ok: false, error }                      the lock file cannot be written at all
 */
function tryAcquire(file, opt = {}) {
  const probe = opt.probe || defaultProbe;
  const rig = opt.rig || path.basename(process.argv[1] || "rig", ".js");
  let stale = null;
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
  } catch (e) {
    return { ok: false, error: `${path.dirname(file)}: ${e.message}` };
  }
  for (let attempt = 0; attempt < 4; attempt++) {
    const record = {
      pid: process.pid,
      start_ms: myStartMs(probe),
      rig,
      script: process.argv[1] || null,
      cwd: process.cwd(),
      since: new Date().toISOString(),
    };
    try {
      const fd = fs.openSync(file, "wx");
      try {
        fs.writeSync(fd, JSON.stringify(record, null, 2));
      } finally {
        fs.closeSync(fd);
      }
      hold(file, record);
      return { ok: true, taken: true, record, stale };
    } catch (e) {
      if (e.code !== "EEXIST") return { ok: false, error: `${file}: ${e.message}` };
    }
    let cur;
    try {
      cur = readLock(file);
    } catch (e) {
      return { ok: false, error: `${file}: ${e.message}` };
    }
    if (cur.missing) continue; // released between the create and the read: try again
    if (cur.record && cur.record.pid === process.pid) {
      // Ours: taken earlier by this process (a second boot in one rig). A record with our pid
      // and another start time was a dead process's that had our number: it is ours now too.
      const mine =
        cur.record.start_ms == null || record.start_ms == null || Math.abs(cur.record.start_ms - record.start_ms) <= START_SLACK_MS;
      if (!mine) fs.writeFileSync(file, JSON.stringify(record, null, 2));
      hold(file, mine ? cur.record : record);
      return { ok: true, taken: !mine, record: mine ? cur.record : record, stale: mine ? stale : cur.record };
    }
    if (!cur.record) {
      // Empty or not JSON: a holder between its create and its write, or junk left behind.
      if (Date.now() - cur.mtimeMs < YOUNG_MS) return { ok: false, holder: { pid: null, rig: "(a rig still writing it)" } };
    } else if (holderLive(cur.record, probe)) {
      return { ok: false, holder: cur.record };
    }
    if (!removeStale(file, cur.raw)) return { ok: false, holder: cur.record || { pid: null, rig: "(being taken over)" } };
    stale = cur.record || { pid: null, rig: "(unreadable)" };
  }
  return { ok: false, holder: { pid: null, rig: "(contended)" } };
}

/** The holder a lock file names right now, when it is live; null when the lock is free or stale. */
function peek(file, probe = defaultProbe) {
  try {
    const cur = readLock(file);
    if (cur.missing || !cur.record) return null;
    return holderLive(cur.record, probe) ? cur.record : null;
  } catch {
    return null;
  }
}

/** One line naming a holder, for a log or a refusal. */
function describeHolder(rec) {
  if (!rec) return "(nobody)";
  if (rec.pid == null) return rec.rig || "(unknown)";
  const since = rec.since ? new Date(rec.since) : null;
  const at = since && !isNaN(since) ? ` since ${since.toLocaleTimeString("en-GB", { hour12: false })}` : "";
  return `${rec.rig || "a rig"} pid ${rec.pid}${at}${rec.cwd ? ` (${rec.cwd})` : ""}`;
}

/** What this process holds on `file`, or null. */
const heldRecord = (file) => held.get(path.resolve(file)) || null;

module.exports = {
  ENV_PATH,
  DEFAULT_PATH,
  lockPath,
  pidAlive,
  osStartMs,
  holderLive,
  readLock,
  tryAcquire,
  release,
  peek,
  describeHolder,
  heldRecord,
};
