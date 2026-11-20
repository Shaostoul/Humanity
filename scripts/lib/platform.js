// The operating system, in one place, for every rig and dev script (2026-11-20,
// docs/design/cross-platform-dev.md step 1).
//
// WHY. The dev tooling grew on the operator's Windows PC, and every rig script spoke Windows
// directly: `target/release/HumanityOS.exe`, `tasklist`, `taskkill /T /F`, PowerShell's
// `Get-CimInstance Win32_Process`. A contributor on macOS or Linux (2026-11-16) found the rigs
// looking for a file that does not exist there (the binary is `HumanityOS`), a process list that
// always read "nothing running" (so the one-GPU guard saw no other game), and a kill that did
// nothing (so a rig's game outlived the rig). This module is the one place those four things are
// asked of the OS; the rig scripts ask it, never the OS. scripts/tests/platform-sweep.test.js
// fails when a script goes back to asking the OS itself.
//
//   EXE_NAME / exeName(platform)   HumanityOS.exe on Windows, HumanityOS elsewhere
//   builtExe(repo, profile)        <repo>/target/<profile>/<EXE_NAME>
//   listProcesses()                [{ pid, ppid, name, exe, cmd }] for every process we may see:
//                                  Windows through the CIM query (PowerShell), Linux straight
//                                  from /proc (no child process), macOS through `ps`
//   childProcesses(pid)            the processes whose parent is pid
//   killTree(pid, { force })       end a process and everything it started: Windows
//                                  `taskkill /PID n /T [/F]` (exactly what the rigs ran before);
//                                  elsewhere the tree is listed first, then SIGTERM (or, with
//                                  force, SIGKILL) to each, the parent first so it starts no more
//   killByExe(exe)                 force-end every process running FROM that file (a rig's own
//                                  copy: ours by construction)
//   startMs(pid)                   when a process started (ms since 1970), to tell a reused pid
//                                  from the process that wrote a lock
//   lowerPriority(pid)             below normal (Windows BelowNormal; nice 10 elsewhere), never
//                                  raising one already lower. Off Windows a user cannot put a
//                                  lowered priority back (raising it needs root), so it is
//                                  one-way there, and says so
//   killCommand / listCommand      the command a PERSON types, for refusal messages
//
// The Windows behaviour is unchanged by this module: the same taskkill, the same PowerShell, the
// same hidden windows. The operator's machine rules (CLAUDE.md: one game at a time, his share of
// the PC, no focus taken) are enforced by the scripts that call it, the same as before.

"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");

const PLATFORM = process.platform;
const IS_WINDOWS = PLATFORM === "win32";

/** The game binary's file name on a platform. */
const exeName = (platform = PLATFORM) => (platform === "win32" ? "HumanityOS.exe" : "HumanityOS");
const EXE_NAME = exeName();

/** Where cargo puts this checkout's build of the game. */
const builtExe = (repo, profile = "release", platform = PLATFORM) => path.join(repo, "target", profile, exeName(platform));

/** A process id we may signal: a positive integer above 1. process.kill(0) signals our whole
 *  process group and process.kill(-1) every process we may reach, so nothing else gets through. */
function okPid(v) {
  const n = Number(v);
  return Number.isInteger(n) && n > 1 && String(v).trim() === String(n) ? n : null;
}

// ── the process list ─────────────────────────────────────────────────────────────

/** One /proc/<pid>/stat line into { comm, state, ppid, start } (start: clock ticks since boot,
 *  field 22). PURE. The command name sits in parentheses and may hold spaces and parentheses of
 *  its own, so the fields are counted from the LAST ")". */
function parseProcStat(text) {
  const s = String(text || "");
  const open = s.indexOf("(");
  const close = s.lastIndexOf(")");
  if (open < 0 || close < open) return null;
  const rest = s.slice(close + 1).trim().split(/\s+/);
  // rest[0] is field 3 (state), rest[1] field 4 (ppid), rest[19] field 22 (starttime).
  const ppid = Number(rest[1]);
  if (!rest[0] || !Number.isInteger(ppid)) return null;
  return { comm: s.slice(open + 1, close), state: rest[0], ppid, start: rest[19] != null ? Number(rest[19]) : null };
}

/** The process list from a /proc tree (`root`: "/proc", or a test's fake folder). Zombies (exited,
 *  not yet reaped) are left out: they run nothing. A process another user owns lists with exe ""
 *  (its exe link is not ours to read), which every caller treats as NOT ours. */
function listFromProc(root = "/proc") {
  let names;
  try {
    names = fs.readdirSync(root);
  } catch {
    return [];
  }
  const out = [];
  for (const d of names) {
    if (!/^\d+$/.test(d)) continue;
    const dir = path.join(root, d);
    let st;
    try {
      st = parseProcStat(fs.readFileSync(path.join(dir, "stat"), "utf8"));
    } catch {
      continue; // gone between the listing and the read
    }
    if (!st || st.state === "Z") continue;
    let exe = "";
    try {
      exe = fs.readlinkSync(path.join(dir, "exe")).replace(/ \(deleted\)$/, "");
    } catch {
      /* another user's, or a kernel thread */
    }
    let argv = [];
    try {
      argv = fs.readFileSync(path.join(dir, "cmdline"), "utf8").split("\0").filter(Boolean);
    } catch {
      /* gone */
    }
    // The name: the exe's file name, else argv[0]'s, else comm (which the kernel cuts at 15 bytes).
    const name = exe ? path.basename(exe) : argv[0] ? path.basename(argv[0]) : st.comm;
    out.push({ pid: Number(d), ppid: st.ppid, name, exe, cmd: argv.join(" ") });
  }
  return out;
}

/** `ps -axww -o pid=,ppid=,comm=` lines (macOS: comm is the executable's full path) into
 *  [{ pid, ppid, exe }]. PURE. comm is the rest of the line, spaces included. */
function parsePsColumns(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((l) => /^\s*(\d+)\s+(\d+)\s+(.*?)\s*$/.exec(l))
    .filter(Boolean)
    .map((m) => ({ pid: Number(m[1]), ppid: Number(m[2]), exe: m[3] }));
}

/** `ps -axww -o pid=,args=` lines into Map(pid -> command line). PURE. */
function parsePsArgs(text) {
  const m = new Map();
  for (const l of String(text || "").split(/\r?\n/)) {
    const r = /^\s*(\d+)\s(.*)$/.exec(l);
    if (r) m.set(Number(r[1]), r[2].trim());
  }
  return m;
}

function listFromPs() {
  const run = (cols) => execFileSync("ps", ["-axww", "-o", cols], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 20000 });
  let cols;
  let args = new Map();
  try {
    cols = parsePsColumns(run("pid=,ppid=,comm="));
  } catch {
    return [];
  }
  try {
    args = parsePsArgs(run("pid=,args="));
  } catch {
    /* names only */
  }
  return cols.map((p) => {
    const exe = path.isAbsolute(p.exe) ? p.exe : "";
    return { pid: p.pid, ppid: p.ppid, name: path.basename(p.exe), exe, cmd: args.get(p.pid) || "" };
  });
}

/** The Windows query: every process, `pid|ppid|name|exe|cmd` a line. A filter (a CIM WQL
 *  condition, its strings in double quotes) narrows it. Single-quoted throughout, the way
 *  machine-guard.js writes its scripts, so nothing needs escaping on its way through a command
 *  line; a filter holding a single quote is refused. Only digits and our own words reach it. */
function windowsQuery(filter) {
  if (filter && /'/.test(filter)) throw new Error(`platform: a filter may not hold a single quote: ${filter}`);
  const where = filter ? ` -Filter '${filter}'` : "";
  return (
    `Get-CimInstance Win32_Process${where} | ForEach-Object { ` +
    "$_.ProcessId.ToString() + '|' + $_.ParentProcessId.ToString() + '|' + $_.Name + '|' + $_.ExecutablePath + '|' + $_.CommandLine }"
  );
}

/** `pid|ppid|name|exe|cmd` lines into records. PURE. The command line is everything after the
 *  fourth bar: it may hold bars of its own. */
function parseWindowsLines(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      const f = l.split("|");
      return { pid: Number(f[0]), ppid: Number(f[1]), name: (f[2] || "").trim(), exe: (f[3] || "").trim(), cmd: f.slice(4).join("|").trim() };
    })
    .filter((p) => Number.isInteger(p.pid) && p.pid > 0);
}

function listFromWindows(filter) {
  try {
    const text = execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", windowsQuery(filter)], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      windowsHide: true,
      timeout: 60000,
      maxBuffer: 64 * 1024 * 1024,
    });
    return parseWindowsLines(text);
  } catch {
    return null; // the query itself failed (no PowerShell, a timeout): not "nothing running"
  }
}

/** Every process we may see: [{ pid, ppid, name, exe, cmd }]. [] when the list cannot be read
 *  (say so to the caller's reader rather than "nothing running" where it matters).
 *  opt.procRoot reads a /proc tree from another folder (tests); opt.platform overrides the OS. */
function listProcesses(opt = {}) {
  const platform = opt.platform || PLATFORM;
  if (opt.procRoot) return listFromProc(opt.procRoot);
  if (platform === "win32") return listFromWindows(opt.windowsFilter) || [];
  if (platform === "linux") return listFromProc("/proc");
  return listFromPs();
}

/** The processes whose parent is pid, or null when the list could not be read (say so rather
 *  than "none"). On Windows a process keeps its ParentProcessId after its parent exits (what lets
 *  a rig find a hand-off after the game that made it is gone); on Linux and macOS an orphan is
 *  handed to init at once, so only living parents have children. */
function childProcesses(pid, list) {
  const n = Number(pid);
  if (!list && IS_WINDOWS) return listFromWindows(`ParentProcessId=${Math.trunc(n) || 0}`);
  return (list || listProcesses()).filter((p) => p.ppid === n && p.pid !== n);
}

/** Every process below pid in `list` (children, theirs, ...), nearest first. PURE. */
function descendants(pid, list) {
  const kids = new Map();
  for (const p of list) {
    if (p.pid === p.ppid) continue;
    if (!kids.has(p.ppid)) kids.set(p.ppid, []);
    kids.get(p.ppid).push(p.pid);
  }
  const out = [];
  const seen = new Set([Number(pid)]);
  const queue = [Number(pid)];
  while (queue.length) {
    for (const c of kids.get(queue.shift()) || []) {
      if (seen.has(c)) continue;
      seen.add(c);
      out.push(c);
      queue.push(c);
    }
  }
  return out;
}

// ── ending processes ─────────────────────────────────────────────────────────────

/** End pid and every process it started. force: end them now (taskkill /F, SIGKILL); without it
 *  ask them to close (taskkill without /F, SIGTERM), which lets the game save. tree: false ends
 *  pid alone (taskkill without /T). Never throws; true when a signal was delivered (or taskkill
 *  succeeded). */
function killTree(pid, opt = {}) {
  const n = okPid(pid);
  if (n == null || n === process.pid) return false;
  const force = opt.force !== false;
  const whole = opt.tree !== false;
  if (IS_WINDOWS) {
    try {
      execFileSync("taskkill", ["/PID", String(n), ...(whole ? ["/T"] : []), ...(force ? ["/F"] : [])], { stdio: "ignore", windowsHide: true, timeout: 30000 });
      return true;
    } catch {
      return false; // already gone, or not ours to end
    }
  }
  // The tree is listed BEFORE any signal: once the parent dies its children belong to init and
  // can no longer be found by their parent.
  const tree = whole ? [n, ...descendants(n, listProcesses())] : [n];
  let any = false;
  for (const p of tree) {
    try {
      process.kill(p, force ? "SIGKILL" : "SIGTERM");
      any = true;
    } catch {
      /* already gone */
    }
  }
  return any;
}

/** Force-end every process whose executable IS `exe`. Safe by construction for a rig's own copy:
 *  only rigs launch from a rig folder, so the operator's game can never match. Never throws. */
function killByExe(exe) {
  if (!exe) return 0;
  if (IS_WINDOWS) {
    const name = path.parse(exe).name.replace(/'/g, "''");
    const p = String(exe).replace(/'/g, "''");
    try {
      execFileSync(
        "powershell",
        ["-NoProfile", "-NonInteractive", "-Command", `Get-Process -Name '${name}' -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${p}' } | Stop-Process -Force`],
        { stdio: "ignore", windowsHide: true, timeout: 60000 },
      );
    } catch {
      /* none running, or a PowerShell hiccup: the caller's copy retry still covers it */
    }
    return -1; // Windows does not say how many
  }
  const want = path.resolve(exe);
  let n = 0;
  for (const p of listProcesses()) {
    if (!p.exe || path.resolve(p.exe) !== want || p.pid === process.pid) continue;
    try {
      process.kill(p.pid, "SIGKILL");
      n++;
    } catch {
      /* gone */
    }
  }
  return n;
}

/** Is a process still there? (A zombie on Linux counts as gone.) */
function isAlive(pid) {
  const n = okPid(pid);
  if (n == null) return false;
  if (PLATFORM === "linux") {
    try {
      const st = parseProcStat(fs.readFileSync(`/proc/${n}/stat`, "utf8"));
      return !!st && st.state !== "Z";
    } catch {
      return false;
    }
  }
  try {
    process.kill(n, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM"; // there, but not ours
  }
}

// ── start times (a pid is reused: the start time tells the processes apart) ──────────

/** 100 ticks a second: USER_HZ, fixed at 100 by the kernel's ABI on every mainstream Linux. */
const LINUX_HZ = 100;
const FILETIME_EPOCH_MS = 11644473600000; // 1601-01-01 to 1970-01-01

/** Boot time, seconds since 1970, from /proc/stat's btime line. PURE. */
function bootSecondsFromProcStat(text) {
  const m = /^btime\s+(\d+)\s*$/m.exec(String(text || ""));
  return m ? Number(m[1]) : null;
}

/** A Linux process's start, ms since 1970, from its stat line and the boot time. PURE. */
function startMsFromProc(statText, btimeS, hz = LINUX_HZ) {
  const st = parseProcStat(statText);
  if (!st || !Number.isFinite(st.start) || !Number.isFinite(btimeS)) return null;
  return btimeS * 1000 + Math.round((st.start * 1000) / hz);
}

/** When this pid started, ms since 1970, or null when the OS cannot say. Windows: Get-Process's
 *  StartTime (exactly what scripts/lib/boot-lock.js asked before); Linux: /proc; macOS: ps lstart
 *  (to the second). */
function startMs(pid) {
  const n = okPid(pid);
  if (n == null) return null;
  try {
    if (IS_WINDOWS) {
      const out = execFileSync(
        "powershell",
        ["-NoProfile", "-NonInteractive", "-Command", `(Get-Process -Id ${n} -ErrorAction Stop).StartTime.ToFileTimeUtc()`],
        { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], windowsHide: true, timeout: 30000 },
      ).trim();
      return /^\d+$/.test(out) ? Number(BigInt(out) / 10000n) - FILETIME_EPOCH_MS : null;
    }
    if (PLATFORM === "linux") return startMsFromProc(fs.readFileSync(`/proc/${n}/stat`, "utf8"), bootSecondsFromProcStat(fs.readFileSync("/proc/stat", "utf8")));
    const out = execFileSync("ps", ["-o", "lstart=", "-p", String(n)], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 20000 }).trim();
    const t = Date.parse(out);
    return Number.isFinite(t) ? t : null;
  } catch {
    return null;
  }
}

// ── priority ─────────────────────────────────────────────────────────────────────

const BELOW_NORMAL = os.constants.priority.PRIORITY_BELOW_NORMAL; // 10: Windows BelowNormal, nice 10

/** Lower pid to below normal, never raising one already at or below it. Returns "lowered",
 *  "left" (already that low) or "failed" (gone, or not ours). Off Windows this is one-way: a
 *  user may raise a nice value but only root may lower it again. */
function lowerPriority(pid) {
  const n = okPid(pid);
  if (n == null) return "failed";
  try {
    if (os.getPriority(n) >= BELOW_NORMAL) return "left";
    os.setPriority(n, BELOW_NORMAL);
    return "lowered";
  } catch {
    return "failed";
  }
}

/** Can a priority this module lowered be put back by the same user? Only on Windows. */
const CAN_RESTORE_PRIORITY = IS_WINDOWS;

// ── words for a person ─────────────────────────────────────────────────────────

/** The command a person types to end a process (tree), for a refusal message. In Git Bash on
 *  Windows the slashes are doubled (MSYS would read /PID as a path). */
function killCommand(pid, opt = {}) {
  const tree = !!opt.tree;
  const force = !!opt.force;
  if (IS_WINDOWS) return `taskkill //PID ${pid}${tree ? " //T" : ""}${force ? " //F" : ""}`;
  return tree ? `pkill ${force ? "-KILL" : "-TERM"} -P ${pid}; kill ${force ? "-KILL" : "-TERM"} ${pid}` : `kill ${force ? "-KILL" : "-TERM"} ${pid}`;
}

/** The command a person types to see which HumanityOS processes run (or another name's). */
function listCommand(name = "humanityos") {
  return IS_WINDOWS ? `tasklist | grep -i ${name}` : `ps -ax | grep -i ${name}`;
}

module.exports = {
  PLATFORM,
  IS_WINDOWS,
  exeName,
  EXE_NAME,
  builtExe,
  okPid,
  parseProcStat,
  listFromProc,
  parsePsColumns,
  parsePsArgs,
  windowsQuery,
  parseWindowsLines,
  listProcesses,
  childProcesses,
  descendants,
  killTree,
  killByExe,
  isAlive,
  bootSecondsFromProcStat,
  startMsFromProc,
  startMs,
  BELOW_NORMAL,
  lowerPriority,
  CAN_RESTORE_PRIORITY,
  killCommand,
  listCommand,
};
