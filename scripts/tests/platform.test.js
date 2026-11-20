// scripts/lib/platform.js: the executable's name, the process list, ending a process tree and
// lowering a priority, each checked on the platform the test runs on (the operator's Windows PC
// in `just rig-tests`, Linux in CI's tooling job), plus the pure parsers on captured text from the
// other platforms.
//
// What would make these checks that cannot fail: a process list that is empty (every "is it
// running" question then reads "no"), a tree kill that ends only the parent (the rig's game
// outlives the rig, holding its exe), a priority call whose result is never read back. So each
// live test starts real processes and reads the OS's own answer back: the list must hold this
// very test's process with its command line, a killed tree's GRANDCHILD must be gone, and the
// priority is read with os.getPriority after the change.
//
// Seen red, 2026-11-20, each broken on purpose once and put back:
//   - killTree without /T on Windows (the parent only): "the grandchild outlived killTree"
//   - lowerPriority with its setPriority line removed: "expected 10, got 0"
//   - listFromProc reading the comm from the FIRST ")" instead of the last: the fake tree's
//     "(evil) name)" process got ppid NaN and was dropped: descendants missed pid 3003
//   - listProcesses on Windows returning []: "this test's own process is not in the list"

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const P = require("../lib/platform.js");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function until(fn, ms = 10000) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    if (fn()) return true;
    await sleep(100);
  }
  return fn();
}

// ── the name ─────────────────────────────────────────────────────────────────────

test("the executable's name: HumanityOS.exe on Windows only, and the built path under target/", () => {
  assert.strictEqual(P.exeName("win32"), "HumanityOS.exe");
  assert.strictEqual(P.exeName("linux"), "HumanityOS");
  assert.strictEqual(P.exeName("darwin"), "HumanityOS");
  assert.strictEqual(P.EXE_NAME, process.platform === "win32" ? "HumanityOS.exe" : "HumanityOS");
  assert.strictEqual(P.builtExe(path.join("r"), "release", "linux"), path.join("r", "target", "release", "HumanityOS"));
  assert.strictEqual(P.builtExe(path.join("r"), "debug", "win32"), path.join("r", "target", "debug", "HumanityOS.exe"));
});

test("a process id that could signal a group or every process is refused", () => {
  for (const bad of [0, 1, -1, "-5", "1e3", "0x20", 2.5, null, undefined, "abc"]) assert.strictEqual(P.okPid(bad), null, String(bad));
  assert.strictEqual(P.okPid(4242), 4242);
  assert.strictEqual(P.okPid("4242"), 4242);
  assert.strictEqual(P.killTree(0), false);
  assert.strictEqual(P.killTree(-1), false);
  assert.strictEqual(P.killTree(process.pid), false, "killTree never ends the caller itself");
});

// ── the pure parsers, on captured text ───────────────────────────────────────────

test("/proc/<pid>/stat: the fields are counted from the LAST parenthesis", () => {
  // Captured on Debian 12 (a cargo, then a process whose name holds a space and parentheses).
  const cargo = "48213 (cargo) S 48190 48213 48190 34816 48213 4194304 51823 120044 0 0 236 41 380 77 20 0 9 0 2318843 1093713920 30113 18446744073709551615 1 1 0 0 0 0 0 0 1024 0 0 0 17 3 0 0 0 0 0";
  assert.deepStrictEqual(P.parseProcStat(cargo), { comm: "cargo", state: "S", ppid: 48190, start: 2318843 });
  const odd = "777 (a (b) c) R 1 777 777 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 99 0 0 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0";
  assert.deepStrictEqual(P.parseProcStat(odd), { comm: "a (b) c", state: "R", ppid: 1, start: 99 });
  assert.strictEqual(P.parseProcStat("garbage"), null);
  assert.strictEqual(P.parseProcStat(""), null);
});

test("a /proc tree is listed and walked: names from exe, argv[0] or comm; zombies left out", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum_platform_proc_"));
  try {
    const proc = (pid, stat, cmdline) => {
      fs.mkdirSync(path.join(root, String(pid)));
      fs.writeFileSync(path.join(root, String(pid), "stat"), stat);
      if (cmdline != null) fs.writeFileSync(path.join(root, String(pid), "cmdline"), cmdline);
    };
    proc(1, "1 (systemd) S 0 1 1 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 5 0", "/sbin/init\0splash\0");
    proc(3000, "3000 (node) S 1 3000 3000 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 100 0", "/usr/bin/node\0rig.js\0--fast\0");
    proc(3001, "3001 (HumanityOS) S 3000 3000 3000 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 101 0", "/home/u/repo/.probe-rig/HumanityOS\0");
    proc(3003, "3003 ((evil) name)) S 3001 3000 3000 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 102 0", "");
    proc(3004, "3004 (defunct) Z 3001 3000 3000 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 103 0", "");
    proc(4000, "4000 (bash) S 1 4000 4000 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 104 0", "bash\0");
    fs.mkdirSync(path.join(root, "self")); // not a pid: skipped
    const list = P.listProcesses({ procRoot: root });
    const by = new Map(list.map((p) => [p.pid, p]));
    assert.strictEqual(by.get(3001).name, "HumanityOS", "argv[0]'s file name");
    assert.strictEqual(by.get(3000).cmd, "/usr/bin/node rig.js --fast");
    assert.strictEqual(by.get(3003).name, "(evil) name)", "no cmdline: the comm");
    assert.ok(!by.has(3004), "a zombie runs nothing");
    assert.deepStrictEqual(P.descendants(3000, list), [3001, 3003]);
    assert.deepStrictEqual(P.childProcesses(3001, list).map((p) => p.pid), [3003]);
    assert.deepStrictEqual(P.descendants(4000, list), []);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a Linux start time: boot time plus the stat line's ticks at 100 a second", () => {
  // Captured on Debian 12: /proc/stat's btime and a process's stat (field 22 = 2318843 ticks).
  const btime = P.bootSecondsFromProcStat("cpu  1 2 3\nintr 9\nctxt 5\nbtime 1763500000\nprocesses 9000\n");
  assert.strictEqual(btime, 1763500000);
  const stat = "48213 (cargo) S 48190 48213 48190 34816 48213 4194304 51823 120044 0 0 236 41 380 77 20 0 9 0 2318843 1093713920 30113";
  assert.strictEqual(P.startMsFromProc(stat, btime), 1763500000 * 1000 + 23188430);
  assert.strictEqual(P.startMsFromProc("junk", btime), null);
  assert.strictEqual(P.bootSecondsFromProcStat("cpu 1 2"), null);
});

test("the live start time of this test's own process is in the past, and stable", () => {
  const a = P.startMs(process.pid);
  assert.ok(Number.isFinite(a), `no start time for this process (${a})`);
  const now = Date.now();
  // Started before now, and not before node itself had been running for its uptime (2 s slack:
  // macOS ps gives whole seconds).
  assert.ok(a <= now + 2000 && a >= now - process.uptime() * 1000 - 2000, `start ${a} against now ${now}, uptime ${process.uptime()} s`);
  assert.strictEqual(P.startMs(process.pid), a, "the same answer twice");
  assert.strictEqual(P.startMs(0), null);
});

test("macOS ps columns: the path is the rest of the line, spaces included", () => {
  // Captured on macOS 14 (`ps -axww -o pid=,ppid=,comm=` and `-o pid=,args=`).
  const cols = P.parsePsColumns("    1     0 /sbin/launchd\n  612     1 /Applications/Visual Studio Code.app/Contents/MacOS/Electron\n 9001   612 /Users/u/repo/target/release/HumanityOS\n");
  assert.deepStrictEqual(cols[1], { pid: 612, ppid: 1, exe: "/Applications/Visual Studio Code.app/Contents/MacOS/Electron" });
  assert.strictEqual(cols[2].pid, 9001);
  const args = P.parsePsArgs(" 9001 /Users/u/repo/target/release/HumanityOS --sim soak --days 1\n  612 Electron\n");
  assert.strictEqual(args.get(9001), "/Users/u/repo/target/release/HumanityOS --sim soak --days 1");
});

test("the Windows query's lines: the command line keeps its own bars; a quote in a filter is refused", () => {
  const list = P.parseWindowsLines("4|0|System||\r\n5120|880|HumanityOS.exe|C:\\Humanity\\HumanityOS.exe|\"C:\\Humanity\\HumanityOS.exe\" --a|b\r\n");
  assert.strictEqual(list.length, 2);
  assert.deepStrictEqual(list[1], { pid: 5120, ppid: 880, name: "HumanityOS.exe", exe: "C:\\Humanity\\HumanityOS.exe", cmd: '"C:\\Humanity\\HumanityOS.exe" --a|b' });
  assert.match(P.windowsQuery("ParentProcessId=5"), /-Filter 'ParentProcessId=5'/);
  assert.throws(() => P.windowsQuery("Name='x'"), /single quote/);
});

test("the words for a person name this platform's own commands", () => {
  if (process.platform === "win32") {
    assert.strictEqual(P.killCommand(42, { tree: true, force: true }), "taskkill //PID 42 //T //F");
    assert.match(P.listCommand(), /^tasklist/);
  } else {
    assert.strictEqual(P.killCommand(42, { force: true }), "kill -KILL 42");
    assert.match(P.listCommand(), /^ps -ax/);
  }
});

// ── live, on this platform ─────────────────────────────────────────────────────

test("the live process list holds this test's own process, with its parent and command line", () => {
  const list = P.listProcesses();
  const me = list.find((p) => p.pid === process.pid);
  assert.ok(me, `this test's own process is not in the list (${list.length} processes listed)`);
  assert.strictEqual(me.ppid, process.ppid);
  assert.match(me.name, /^node(\.exe)?$/i);
  assert.match(me.cmd, /node/i);
});

/** A child that starts a grandchild and prints its pid; both sleep. The grandchild is DETACHED,
 *  the way rigs start the game: on Windows Node puts a non-detached child in a job object that
 *  ends it with its parent, which made the first red run of this test (killTree without /T) pass;
 *  on Linux a detached child leads its own session, so a process-group kill would miss it and
 *  only the walk by parent id finds it. Either way, only a real tree kill ends it. */
function startFamily() {
  const grand = "setInterval(() => {}, 1000)";
  const parent = `const c = require('child_process').spawn(process.execPath, ['-e', ${JSON.stringify(grand)}], { stdio: 'ignore', detached: true, windowsHide: true }); console.log('GRAND ' + c.pid); setInterval(() => {}, 1000);`;
  const child = spawn(process.execPath, ["-e", parent], { stdio: ["ignore", "pipe", "ignore"], windowsHide: true });
  return new Promise((resolve, reject) => {
    let out = "";
    child.stdout.on("data", (d) => {
      out += d;
      const m = /GRAND (\d+)/.exec(out);
      if (m) resolve({ child, grand: Number(m[1]) });
    });
    child.on("error", reject);
    setTimeout(() => reject(new Error("the family never started")), 15000).unref();
  });
}

test("killTree ends the process AND what it started (the grandchild too)", async () => {
  const { child, grand } = await startFamily();
  try {
    assert.ok(P.isAlive(child.pid) && P.isAlive(grand), "the family is running");
    const kids = P.childProcesses(child.pid);
    assert.ok(kids.some((k) => k.pid === grand), `childProcesses(${child.pid}) missed the grandchild ${grand}`);
    assert.strictEqual(P.killTree(child.pid, { force: true }), true);
    assert.ok(await until(() => !P.isAlive(child.pid)), "the parent outlived killTree");
    assert.ok(await until(() => !P.isAlive(grand)), "the grandchild outlived killTree");
  } finally {
    for (const p of [child.pid, grand]) {
      try {
        process.kill(p, "SIGKILL");
      } catch {
        /* gone, as it should be */
      }
    }
  }
});

test("killByExe ends every process running from that one file, and nothing else", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_platform_exe_"));
  const copy = path.join(dir, process.platform === "win32" ? "hum-platform-standin.exe" : "hum-platform-standin");
  fs.copyFileSync(process.execPath, copy);
  if (process.platform !== "win32") fs.chmodSync(copy, 0o755);
  const a = spawn(copy, ["-e", "setInterval(() => {}, 1000)"], { stdio: "ignore", windowsHide: true });
  const other = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { stdio: "ignore", windowsHide: true });
  try {
    assert.ok(await until(() => P.isAlive(a.pid) && P.isAlive(other.pid)));
    P.killByExe(copy);
    assert.ok(await until(() => !P.isAlive(a.pid)), "the copy's process outlived killByExe");
    assert.ok(P.isAlive(other.pid), "a process from another file was ended too");
  } finally {
    for (const p of [a.pid, other.pid]) {
      try {
        process.kill(p, "SIGKILL");
      } catch {
        /* gone */
      }
    }
    await until(() => !P.isAlive(a.pid), 5000);
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("lowerPriority lowers a child to below normal (read back from the OS) and never raises one", async (t) => {
  const child = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { stdio: "ignore", windowsHide: true });
  try {
    assert.ok(await until(() => P.isAlive(child.pid)));
    // The child inherits this runner's priority, and the operator's share guard runs tests below
    // normal: start it at normal first. Windows lets a user do that; Linux does not (raising a nice
    // value needs root), and a runner already niced cannot show "lowered" there.
    if (os.getPriority(child.pid) !== 0) {
      try {
        os.setPriority(child.pid, 0);
      } catch {
        return t.skip(`this runner is already below normal (priority ${os.getPriority(child.pid)}) and the OS will not raise the child back`);
      }
    }
    assert.strictEqual(os.getPriority(child.pid), 0, "the child starts at normal");
    assert.strictEqual(P.lowerPriority(child.pid), "lowered");
    assert.strictEqual(os.getPriority(child.pid), P.BELOW_NORMAL);
    assert.strictEqual(P.lowerPriority(child.pid), "left", "already low: left as it is");
    os.setPriority(child.pid, os.constants.priority.PRIORITY_LOW);
    assert.strictEqual(P.lowerPriority(child.pid), "left", "lower still (idle): never raised");
    assert.strictEqual(os.getPriority(child.pid), os.constants.priority.PRIORITY_LOW);
    assert.strictEqual(P.lowerPriority(0), "failed");
  } finally {
    P.killTree(child.pid, { force: true });
  }
});
