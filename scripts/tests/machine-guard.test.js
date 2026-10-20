// Prove the one-machine guard can actually fail.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/machine-guard.test.js
//
// The defects this guards against (both 2026-09-18) are NOT "the rig crashed".
// They are "the rig returned a number, the number was wrong, and nothing in the
// output said so":
//
//   * another builder's HumanityOS.exe arrived mid-sweep and one gpu.celestial
//     reading came out 9 ms high (docs/design/frame-cost-arc.md, V1 outcome);
//   * a concurrent cargo/rustc release build in another worktree saturated the
//     CPU and a limb vantage read 6.6 fps against a 15 ms GPU sum, every cpu.*
//     stage stretched while the GPU columns stayed normal.
//
// A guard for that is worth something only if it goes red on each, so that is
// what these tests assert, by feeding it a fake process list rather than
// booting a second renderer or starting a build (the machine rule forbids both
// during working hours, and a test that needs a 2 GB rustc is a test nobody
// runs).

const test = require("node:test");
const assert = require("node:assert");
const MG = require("../lib/machine-guard.js");

// The `pid|name|exe` shape the Win32_Process query prints.
const RIG_EXE = "C:\\Humanity\\.probe-rig\\HumanityOS.exe";
const FOREIGN_EXE = "C:\\Humanity\\target\\release\\HumanityOS.exe";
const RUSTC = "C:\\Users\\Shaos\\.rustup\\toolchains\\stable-x86_64-pc-windows-msvc\\bin\\rustc.exe";
const OWN = { pids: [4242], exe: RIG_EXE };
const CLEAN = `4242|HumanityOS.exe|${RIG_EXE}`;

const withFake = (text, fn) => {
  MG.setLister(() => text);
  try {
    return fn();
  } finally {
    MG.setLister(null); // back to the live query
  }
};
const foreignIn = (text, own = OWN) => withFake(text, () => MG.foreignProcs(own));

test("parseProcs reads pid, name and exe out of the query output", () => {
  const rows = MG.parseProcs(`4242|HumanityOS.exe|${RIG_EXE}\r\n9001|rustc.exe|${RUSTC}\r\n`);
  assert.deepStrictEqual(rows, [
    { pid: 4242, name: "HumanityOS.exe", exe: RIG_EXE },
    { pid: 9001, name: "rustc.exe", exe: RUSTC },
  ]);
});

test("parseProcs survives a process whose path we cannot read", () => {
  // Win32_Process returns an empty ExecutablePath for a process the query
  // cannot open. It still counts: erring loud is correct here.
  assert.deepStrictEqual(MG.parseProcs("9001|cargo.exe|"), [{ pid: 9001, name: "cargo.exe", exe: "" }]);
});

test("the watch list covers the game and the build tools, and nothing idle", () => {
  // Pinned deliberately: a guard that blocks on an IDE's rust-analyzer gets
  // bypassed, and a bypassed guard protects nothing.
  assert.ok(MG.WATCHED.includes("HumanityOS.exe"));
  assert.ok(MG.WATCHED.includes("cargo.exe"));
  assert.ok(MG.WATCHED.includes("rustc.exe"));
  assert.ok(!MG.WATCHED.some((n) => /rust-analyzer|mspdbsrv/i.test(n)));
  // The query the guard runs must actually ask for every watched name.
  const q = MG.psQuery();
  // The game by suffix (LIKE), so the operator's v<version>_HumanityOS.exe
  // archive is listed too (2026-10-04); the build tools by exact name.
  for (const n of MG.WATCHED) {
    const want = n === MG.GAME ? `Name LIKE '%${n}'` : `Name='${n}'`;
    assert.ok(q.includes(want), `query omits ${want}`);
  }
});

test("our own rig instance is NOT a contender (by pid and by exe path)", () => {
  assert.deepStrictEqual(foreignIn(CLEAN), []);
  // By exe path alone: covers the engine delegating to a second copy of itself,
  // whose pid the sweep never saw (the reason kill uses taskkill /T).
  assert.deepStrictEqual(foreignIn(CLEAN, { pids: [], exe: RIG_EXE }), []);
  // Path separators and case must not decide ownership.
  assert.deepStrictEqual(foreignIn(CLEAN, { pids: [], exe: "c:/humanity/.probe-rig/humanityos.exe" }), []);
});

test("RED: a second game instance arriving mid-sweep marks the capture contaminated", () => {
  const rec = { id: "fuji-forest-ground", fps: 12, frame_ms: 83.5, ok: true };
  const lines = [];
  const before = foreignIn(CLEAN);
  assert.deepStrictEqual(before, [], "precondition: the machine was clean at the start of the window");
  // The other builder's rig boots while the capture is settling.
  const after = foreignIn(`${CLEAN}\n7777|HumanityOS.exe|${FOREIGN_EXE}`);
  assert.strictEqual(MG.markCapture(rec, before, after, (m) => lines.push(m)), true);
  assert.strictEqual(rec.contaminated, true);
  assert.deepStrictEqual(rec.contaminated_by, [
    { pid: 7777, name: "HumanityOS.exe", exe: FOREIGN_EXE },
  ]);
  assert.ok(lines.some((l) => l.includes("CONTAMINATED")), "the failure must be printed, not only recorded");
  assert.ok(
    lines.some((l) => l.includes("second renderer")),
    "a second renderer must be named as a GPU problem"
  );
});

test("RED: a concurrent cargo/rustc build marks the capture contaminated", () => {
  // The CPU shape: no second renderer anywhere, and the reading is still junk.
  const rec = { id: "limb-400km", fps: 6.6, frame_ms: 151, ok: true };
  const lines = [];
  const busy = `${CLEAN}\n5150|cargo.exe|C:\\Users\\Shaos\\.cargo\\bin\\cargo.exe\n5151|rustc.exe|${RUSTC}`;
  const seen = foreignIn(busy);
  assert.deepStrictEqual(
    seen.map((p) => p.name).sort(),
    ["cargo.exe", "rustc.exe"],
    "a build is a contender even though no second game instance exists"
  );
  assert.strictEqual(MG.markCapture(rec, seen, seen, (m) => lines.push(m)), true);
  assert.strictEqual(rec.contaminated, true);
  assert.deepStrictEqual(rec.contaminated_by.map((p) => p.name).sort(), ["cargo.exe", "rustc.exe"]);
  // The message has to say WHICH kind, because the reader's next move differs:
  // a build inflates cpu.* and the frame time while gpu.* looks fine.
  const text = lines.join("\n");
  assert.ok(
    text.includes("cpu.*") && text.includes("gpu.*") && text.includes("rustc.exe"),
    "a build must be named as a CPU problem that leaves the GPU columns looking normal"
  );
});

test("a build is never mistaken for one of ours, whatever the rig path is", () => {
  // The rig never compiles, so no cargo/rustc can be excused by exe matching.
  const busy = `5151|rustc.exe|${RUSTC}`;
  assert.deepStrictEqual(foreignIn(busy, { pids: [], exe: RUSTC }).map((p) => p.pid), [5151]);
});

test("a process that arrives AND leaves inside the window still marks it", () => {
  // This is the V1 shape: by the time the sweep ended, the other rig was gone.
  // Sampling once at either end would have missed it; sampling both does not.
  const rec = {};
  const before = foreignIn(`${CLEAN}\n7777|HumanityOS.exe|${FOREIGN_EXE}`);
  const after = foreignIn(CLEAN);
  assert.deepStrictEqual(after, [], "it had already exited by the end of the window");
  assert.strictEqual(MG.markCapture(rec, before, after, () => {}), true);
  assert.deepStrictEqual(rec.contaminated_by.map((p) => p.pid), [7777]);
});

test("GREEN: a clean window leaves the record untouched", () => {
  const rec = { id: "moon-surface-200m", fps: 30 };
  const clean = foreignIn(CLEAN);
  assert.strictEqual(MG.markCapture(rec, clean, clean, () => {}), false);
  assert.ok(!("contaminated" in rec), "a clean capture must not carry the key at all");
});

test("listInstances sees only the game, so a build cannot block verify-screens", () => {
  // verify-runtime and verify-screens judge panics, captures and clicks, not
  // timings. A build slows them; it does not change their verdict, and refusing
  // to boot because somebody is compiling would make those gates unrunnable in
  // a shared checkout.
  withFake(`${CLEAN}\n5151|rustc.exe|${RUSTC}`, () => {
    assert.deepStrictEqual(MG.listInstances().map((p) => p.pid), [4242]);
    assert.strictEqual(MG.listProcs().length, 2);
  });
});

test("mergeForeign de-duplicates by pid across the two samples", () => {
  const a = [{ pid: 7777, name: "HumanityOS.exe", exe: FOREIGN_EXE }];
  const b = [
    { pid: 7777, name: "HumanityOS.exe", exe: FOREIGN_EXE },
    { pid: 5151, name: "rustc.exe", exe: RUSTC },
  ];
  assert.deepStrictEqual(MG.mergeForeign(a, b).map((p) => p.pid), [7777, 5151]);
});

test("waitForFree returns immediately on a clean machine and reports zero wait", () => {
  withFake(CLEAN, () => {
    const r = MG.waitForFree({ own: OWN, timeoutMs: 1, pollMs: 1, log: () => {} });
    assert.deepStrictEqual(r, { free: true, waited: false, waited_s: 0, blockers: [] });
  });
});

test("RED: waitForFree blocks, logs and reports the blocker - for a build as well", () => {
  withFake(`${CLEAN}\n5151|rustc.exe|${RUSTC}`, () => {
    const lines = [];
    const t0 = Date.now();
    // Bounded so the test finishes: the point is that it WAITED and gave up
    // loudly rather than booting silently beside the build.
    const r = MG.waitForFree({ own: OWN, timeoutMs: 120, pollMs: 40, log: (m) => lines.push(m), label: "pre-boot" });
    assert.strictEqual(r.free, false);
    assert.ok(Date.now() - t0 >= 100, "it must actually have waited, not fallen through");
    // A 120 ms wait rounds to 0 s, and still WAS a wait: a caller deciding
    // "did we wait?" from waited_s > 0 got it wrong (2026-10-04, probe-sweep
    // skipped its re-check of the exe). Red check, run 2026-10-04 with the
    // `waited` field removed from waitForFree: "a sub-second wait is still a
    // wait" failed (undefined !== true).
    assert.strictEqual(r.waited_s, 0, "precondition: this wait rounds to 0 s");
    assert.strictEqual(r.waited, true, "a sub-second wait is still a wait");
    assert.deepStrictEqual(r.blockers.map((p) => p.pid), [5151]);
    assert.ok(lines.some((l) => l.includes("WAITING")), "entering the wait must be logged");
    assert.ok(lines.some((l) => l.includes("rustc.exe")), "the blocker must be named, not counted");
    assert.ok(lines.some((l) => l.includes("GAVE UP")), "giving up must be logged");
  });
});

// The operator starts his own game with `just launch` / `just play`, which boot
// the newest ARCHIVE: the process is named v<version>_HumanityOS.exe, not
// HumanityOS.exe (found 2026-10-04). It must count as the game everywhere the
// exact name does: in the one-GPU wait, in contamination, and as a renderer
// rather than a build.
// Red check, run 2026-10-04 with isGameName replaced by an exact-name match:
// "the operator's just-launch archive is a game" failed (false), and the
// contamination test named it a build ("a second renderer" missing).
test("RED: the operator's just-launch archive (v<version>_HumanityOS.exe) is the game", () => {
  assert.strictEqual(MG.isGameName("v0.1448.0_HumanityOS.exe"), true, "the operator's just-launch archive is a game");
  assert.strictEqual(MG.isGameName("HumanityOS.exe"), true);
  assert.strictEqual(MG.isGameName("humanityos.exe"), true, "Windows names are case-insensitive");
  assert.strictEqual(MG.isGameName("rustc.exe"), false);
  assert.strictEqual(MG.isGameName("HumanityOSxexe"), false, "the dot is a dot");
  const ARCHIVE = "C:\Humanity\v0.1448.0_HumanityOS.exe";
  const rec = { id: "home-overview-noon", fps: 30, frame_ms: 33, ok: true };
  const lines = [];
  const before = foreignIn(CLEAN);
  const after = foreignIn(`${CLEAN}\n8888|v0.1448.0_HumanityOS.exe|${ARCHIVE}`);
  assert.deepStrictEqual(after.map((p) => p.pid), [8888], "the archive is a contender, not ignored");
  assert.strictEqual(MG.markCapture(rec, before, after, (m) => lines.push(m)), true);
  assert.ok(lines.some((l) => l.includes("second renderer")), "the archive is named as a second renderer, not a build");
});

test("the live query matches the game by suffix, so an archive is listed at all", () => {
  // The WMI filter must not be an exact Name= match for the game.
  const src = require("node:fs").readFileSync(require.resolve("../lib/machine-guard.js"), "utf8");
  assert.ok(/Name LIKE '%\$\{n\}'/.test(src), "the game is matched with LIKE '%HumanityOS.exe'");
});

// ── HOLDING BUILDS BACK DURING A CAPTURE (BUG-161) ──────────────────────────
//
// v0.1459.0's first co-presence --plots runs, with ten agents compiling, drew the game at 6 to
// 16 fps and failed the steady-speed checks in three of six orders; the rerun with every cargo,
// rustc and link process held at BelowNormal by a hand-run scratchpad loop passed 88/88.
// holdBuilds() is that loop inside the guard, and these tests hold it to its promises: it only
// ever LOWERS, only build processes, puts back exactly what it lowered (and the children that
// inherited the lower priority from it), and does so on an error or Ctrl+C too.
//
// The process table below is a fake Windows: the guard's runner is handed a function that
// answers its priority requests the way its PowerShell does, so no real process is touched.
// Every fake pid is ODD: Windows process ids are multiples of 4, so even a guard whose safety
// broke could not reach a real process from these tests. One test at the end does use the real
// PowerShell, on a rustc this test starts itself and on nothing else.
//
// Red, run 2026-10-20. Before holdBuilds existed every test below failed ("MG.holdBuilds is not
// a function", "MG.priorityScript is not a function", BUILDS undefined, and the exit-path test
// "restored on an uncaught error: " with no restore at all). Then against the finished guard,
// one property broken at a time in a scratch copy (the worktree untouched), each went red:
//   no exit hook             -> "restored on an uncaught error: lower" (lowered, never put back)
//   release does not look for children started since the last look -> "a child a held build
//                               starts after the last look ..." (BelowNormal !== Normal)
//   a fake list reaching the real runner -> "a fake process list never reaches ..."
//   no name re-check in the script -> "each pid is checked to still be a build before it is
//                               touched", and the real-PowerShell test: the node child listed
//                               as cargo.exe was lowered
//   inheritance through a parent lowered in the same look -> "low before the hold: left low,
//                               even though its parent was held" (Normal !== BelowNormal)
//   no start-time check in the restore -> "a reused pid is never touched: the start time is
//                               compared", and the real-PowerShell test: a wrong start time
//                               restored the rustc anyway
// The first run of those found three of them going UNNOTICED (the script-text test matched
// the names list rather than the check, and nothing exercised a pre-hold low child or a real
// wrong start time); the assertions above were tightened until each went red.

const RANK = ["Idle", "BelowNormal", "Normal", "AboveNormal", "High", "RealTime"];
/** A fake Windows process table and a runner that answers the guard's requests as its
 *  PowerShell does. `procs`: [{ pid, name, cls, ppid, start }]. */
function fakeWindows(procs) {
  const table = new Map(procs.map((p) => [p.pid, { ...p }]));
  const calls = [];
  const stem = (n) => String(n).toLowerCase();
  const run = (req) => {
    calls.push(req);
    if (req.kind === "restore") {
      return req.entries
        .map((e) => {
          const p = table.get(e.pid);
          if (!p) return `${e.pid}|gone`;
          if (String(p.start) !== String(e.start)) return `${e.pid}|other`;
          if (p.cls !== "BelowNormal") return `${e.pid}|changed|${p.cls}`;
          p.cls = e.to;
          return `${e.pid}|restored|${e.to}`;
        })
        .join("\n");
    }
    return req.pids
      .map((id) => {
        const p = table.get(id);
        if (!p) return `${id}|||||gone`;
        const n = stem(p.name);
        if (!["cargo.exe", "rustc.exe", "link.exe", "cl.exe"].includes(n)) return `${id}|${n}||||notbuild`;
        const was = p.cls;
        if (req.lower && RANK.indexOf(was) > RANK.indexOf("BelowNormal")) {
          p.cls = "BelowNormal";
          return `${id}|${n}|${p.ppid}|${p.start}|${was}|lowered`;
        }
        return `${id}|${n}|${p.ppid}|${p.start}|${was}|left`;
      })
      .join("\n");
  };
  const listing = () => [...table.values()].map((p) => `${p.pid}|${p.name}|C:\\fake\\${p.name}`).join("\n");
  /** A process starting while the hold is on: Windows gives a child of a BelowNormal (or Idle)
   *  parent its parent's class, and Normal otherwise. */
  const spawn = (p) => {
    const parent = table.get(p.ppid);
    const cls = p.cls || (parent && (parent.cls === "BelowNormal" || parent.cls === "Idle") ? parent.cls : "Normal");
    table.set(p.pid, { start: 900 + p.pid, ...p, cls });
  };
  return { table, calls, run, listing, spawn, cls: (pid) => (table.get(pid) || {}).cls };
}
const GAME_ROW = { pid: 4243, name: "HumanityOS.exe", cls: "Normal", ppid: 1, start: 1 };
const ARCHIVE_ROW = { pid: 8889, name: "v0.1459.0_HumanityOS.exe", cls: "Normal", ppid: 1, start: 2 };
const holdIn = (win, extra = {}) => {
  MG.setLister(() => win.listing());
  const lines = [];
  const h = MG.holdBuilds({ label: "test capture", run: win.run, resweepMs: 0, log: (m) => lines.push(m), ...extra });
  return { h, lines };
};

test("BUILDS: the build tools the guard watches, and never the game", () => {
  assert.ok(Array.isArray(MG.BUILDS) && MG.BUILDS.length >= 3);
  for (const n of ["cargo.exe", "rustc.exe", "link.exe"]) assert.ok(MG.BUILDS.includes(n), `${n} is a build`);
  for (const n of MG.BUILDS) {
    assert.ok(MG.WATCHED.includes(n), `${n} is watched: one list of what a build is`);
    assert.ok(!MG.isGameName(n), `${n} is not the game`);
  }
});

test("RED: the priority script names only the pids it is given, re-checks each is a build, and stops nothing", () => {
  const s = MG.priorityScript([5151, 5153], { lower: true });
  assert.ok(s.includes("$ids = @(5151,5153)"), s);
  assert.ok(s.includes("ProcessId=5151 OR ProcessId=5153"), "the process list is asked for these pids only");
  assert.ok(s.includes("Get-Process -Id $id"), "each process is opened by its id, never by a name");
  assert.ok(/\$names = @\('cargo\.exe','rustc\.exe','link\.exe'/.test(s), "the build names it checks against");
  // The re-check itself, and BEFORE anything is changed: a pid that is no longer a build is
  // skipped (continue) ahead of the line that lowers.
  const check = s.indexOf("if ($names -notcontains $n) {");
  const setAt = s.indexOf("$p.PriorityClass = 'BelowNormal'");
  assert.ok(check > 0, "each pid is checked to still be a build before it is touched");
  assert.ok(setAt > check, "lowering sets BelowNormal, after the build check");
  assert.ok(/notbuild'; continue }/.test(s.slice(check, setAt)), "a pid that is no longer a build is skipped");
  for (const bad of [/Stop-Process/i, /\.Kill\(/i, /taskkill/i, /Set-ItemProperty|Set-Service|powercfg|bcdedit|reg add/i, /"/]) {
    assert.ok(!bad.test(s), `the script must not contain ${bad}`);
  }
  assert.ok(!MG.priorityScript([5151], { lower: false }).includes("= 'BelowNormal'"), "an inspection changes nothing");
  // Only numbers reach the script: anything else is refused before PowerShell sees it.
  assert.throws(() => MG.priorityScript(["5151; Stop-Process -Name *"]), /process id/);
  assert.throws(() => MG.priorityScript([]), /process id/);
  const r = MG.restoreScript([{ pid: 5151, start: "133000000000000000", to: "Normal" }]);
  assert.ok(r.includes("'5151:133000000000000000:Normal'"), r);
  // Both guards come before the one line that changes anything: the same process (its start
  // time; a reused pid is someone else), and still at what the hold set.
  const startAt = r.indexOf("if ([string]$p.StartTime.ToFileTimeUtc() -ne $f[1]) { [string]$id + '|other'; continue }");
  const changedAt = r.indexOf("if ($c -ne 'BelowNormal') { [string]$id + '|changed|' + $c; continue }");
  const restoreAt = r.indexOf("$p.PriorityClass = $f[2]");
  assert.ok(startAt > 0, "a reused pid is never touched: the start time is compared");
  assert.ok(changedAt > 0, "a priority someone else changed meanwhile is left as they set it");
  assert.ok(restoreAt > startAt && restoreAt > changedAt, "both checks come before the restore");
  assert.throws(() => MG.restoreScript([{ pid: 5151, start: "1", to: "Normal; Stop-Process" }]), /priority/);
  assert.throws(() => MG.restoreScript([{ pid: 5151, start: "x", to: "Normal" }]), /start/);
  // PowerShell is called with -NoProfile, and the script is its only other input.
  const args = MG.powerShellArgs(s);
  assert.deepStrictEqual(args.slice(0, 2), ["-NoProfile", "-NonInteractive"]);
  assert.strictEqual(args[args.length - 1], s);
});

test("RED: a hold lowers the running builds and nothing else: not the game, not a build already low", () => {
  const win = fakeWindows([
    GAME_ROW,
    ARCHIVE_ROW,
    { pid: 5151, name: "cargo.exe", cls: "Normal", ppid: 1, start: 10 },
    { pid: 5153, name: "rustc.exe", cls: "AboveNormal", ppid: 5151, start: 11 },
    { pid: 5155, name: "link.exe", cls: "Normal", ppid: 5153, start: 12 },
    { pid: 5157, name: "cargo.exe", cls: "Idle", ppid: 1, start: 13 },
    { pid: 5159, name: "rustc.exe", cls: "BelowNormal", ppid: 5157, start: 14 },
    // Somebody lowered this rustc by hand BEFORE the hold, under a cargo that was still Normal.
    // The hold lowers its cargo, but this one was low already, and is not the hold's to raise.
    { pid: 5163, name: "rustc.exe", cls: "BelowNormal", ppid: 5151, start: 15 },
  ]);
  try {
    const { h, lines } = holdIn(win);
    assert.strictEqual(win.cls(5151), "BelowNormal");
    assert.strictEqual(win.cls(5153), "BelowNormal");
    assert.strictEqual(win.cls(5155), "BelowNormal");
    assert.strictEqual(win.cls(5157), "Idle", "a build already at Idle is never raised");
    assert.strictEqual(win.cls(5159), "BelowNormal");
    assert.strictEqual(win.cls(4243), "Normal", "the game is never touched");
    assert.strictEqual(win.cls(8889), "Normal", "nor the operator's archive of it");
    // The game never even reaches the priority script.
    for (const c of win.calls) for (const id of c.pids || []) assert.ok(![4243, 8889].includes(id), `pid ${id} was handed to the priority script`);
    assert.ok(lines.some((l) => /holding 3 build process\(es\) at BelowNormal/.test(l)), lines.join("\n"));
    assert.ok(lines.some((l) => l.includes("rustc.exe pid 5153 (was AboveNormal)")), "each one is named with what it was");
    const s = h.release();
    assert.deepStrictEqual([5151, 5153, 5155].map(win.cls), ["Normal", "AboveNormal", "Normal"], "each put back to what it was");
    assert.strictEqual(win.cls(5157), "Idle", "never raised");
    assert.strictEqual(win.cls(5159), "BelowNormal", "never raised");
    assert.strictEqual(win.cls(5163), "BelowNormal", "low before the hold: left low, even though its parent was held");
    assert.deepStrictEqual(s.inherited, [], "nothing here inherited the hold");
  } finally {
    MG.setLister(null);
  }
});

test("RED: release puts back what it lowered and the children that inherited it, and nothing else", () => {
  const win = fakeWindows([
    GAME_ROW,
    { pid: 5151, name: "cargo.exe", cls: "Normal", ppid: 1, start: 10 },
    { pid: 5153, name: "rustc.exe", cls: "High", ppid: 1, start: 11 },
    { pid: 5161, name: "cargo.exe", cls: "Normal", ppid: 1, start: 20 },
    { pid: 5171, name: "cargo.exe", cls: "Normal", ppid: 1, start: 30 },
    { pid: 5181, name: "cargo.exe", cls: "Idle", ppid: 1, start: 40 },
  ]);
  try {
    const { h, lines } = holdIn(win);
    // While the capture runs: cargo 5151 starts a rustc, which Windows starts at BelowNormal
    // (its parent's); that rustc starts a link. A low build of someone else's starts a rustc at
    // Idle. Somebody raises 5161 to High by hand. 5171 exits, and its pid is taken by another
    // cargo (a different process: a different start time).
    win.spawn({ pid: 5191, name: "rustc.exe", ppid: 5151 });
    win.spawn({ pid: 5193, name: "link.exe", ppid: 5191 });
    win.spawn({ pid: 5195, name: "rustc.exe", ppid: 5181 });
    assert.strictEqual(win.cls(5191), "BelowNormal", "precondition: the child inherited the hold");
    assert.strictEqual(win.cls(5193), "BelowNormal");
    win.table.get(5161).cls = "High";
    win.table.set(5171, { pid: 5171, name: "cargo.exe", cls: "BelowNormal", ppid: 1, start: 31 });
    h.sweep(); // the hold's periodic look: finds the new ones
    const s = h.release();
    assert.strictEqual(win.cls(5151), "Normal", "put back");
    assert.strictEqual(win.cls(5153), "High", "put back to what it was, not to Normal");
    assert.strictEqual(win.cls(5191), "Normal", "a child that inherited the hold is put back to its parent's own priority");
    assert.strictEqual(win.cls(5193), "Normal", "and its child");
    assert.strictEqual(win.cls(5161), "High", "a priority someone else set meanwhile is left as they set it");
    assert.strictEqual(win.cls(5171), "BelowNormal", "a reused pid is a different process: never touched");
    assert.strictEqual(win.cls(5181), "Idle", "never raised");
    assert.strictEqual(win.cls(5195), "Idle", "a low build's own child is not ours to raise");
    assert.strictEqual(win.cls(4243), "Normal");
    assert.deepStrictEqual(s.lowered.map((x) => x.pid).sort(), [5151, 5153, 5161, 5171]);
    assert.deepStrictEqual(s.inherited.map((x) => x.pid).sort(), [5191, 5193]);
    assert.deepStrictEqual(s.restored.sort(), [5151, 5153, 5191, 5193]);
    assert.ok(lines.some((l) => /put back 4 build process\(es\)/.test(l)), lines.join("\n"));
    // Once only: a second release hands back the same record and runs nothing.
    const callsBefore = win.calls.length;
    assert.strictEqual(h.release(), s);
    assert.strictEqual(win.calls.length, callsBefore);
  } finally {
    MG.setLister(null);
  }
});

test("RED: a child a held build starts after the last look is still found and put back", () => {
  // The periodic look runs every few seconds; a rustc started between the last look and the
  // release is BelowNormal by inheritance too, and release must find it rather than leave it low.
  const win = fakeWindows([{ pid: 5151, name: "cargo.exe", cls: "Normal", ppid: 1, start: 10 }]);
  try {
    const { h } = holdIn(win);
    win.spawn({ pid: 5197, name: "rustc.exe", ppid: 5151 });
    const s = h.release();
    assert.strictEqual(win.cls(5197), "Normal");
    assert.deepStrictEqual(s.inherited.map((x) => x.pid), [5197]);
  } finally {
    MG.setLister(null);
  }
});

test("a fake process list never reaches a real process's priority", () => {
  // Other rig tests feed the guard a made-up process list (HUMANITY_MACHINE_GUARD_FAKE or
  // setLister). Their pids are invented, so the real PowerShell must never be pointed at them:
  // without a runner handed in on purpose, a hold over a fake list touches nothing.
  MG.setLister(() => "5151|cargo.exe|C:\\fake\\cargo.exe");
  try {
    const lines = [];
    const h = MG.holdBuilds({ label: "fake", resweepMs: 0, log: (m) => lines.push(m) });
    const s = h.release();
    assert.deepStrictEqual(s.lowered, []);
    assert.ok(lines.some((l) => /fake process list/i.test(l)), lines.join("\n"));
  } finally {
    MG.setLister(null);
  }
});

test("describeHold says what a hold did in one line", () => {
  const d = MG.describeHold({ label: "the walk at home", lowered: [{ pid: 5151, name: "cargo.exe", from: "Normal" }], inherited: [{ pid: 5191, name: "rustc.exe", parent: 5151 }], left_low: [], restored: [5151, 5191], not_restored: [] });
  assert.match(d, /the walk at home: 1 lowered, 1 started at BelowNormal by one of them, 2 put back/);
  assert.match(MG.describeHold({ label: "x", lowered: [], inherited: [], left_low: [], restored: [], not_restored: [] }), /no build was running/);
});

// The exit paths run in a child process, because the point is what happens when a process
// ENDS: the rig's own Ctrl+C handler exits through process.exit, an uncaught error exits with
// code 1, and a plain end of the script exits normally. Each must put the build back first.
function childExit(how) {
  const os = require("node:os");
  const fs = require("node:fs");
  const path = require("node:path");
  const { spawnSync } = require("node:child_process");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mg-hold-"));
  const logFile = path.join(dir, "calls.txt");
  const script = path.join(dir, "child.js");
  fs.writeFileSync(
    script,
    `const MG = require(${JSON.stringify(require.resolve("../lib/machine-guard.js"))});
const fs = require("fs");
let cls = "Normal";
MG.setLister(() => "5151|cargo.exe|C:\\\\fake\\\\cargo.exe");
const run = (req) => {
  fs.appendFileSync(${JSON.stringify(logFile)}, req.kind + "\\n");
  if (req.kind === "restore") { cls = "Normal"; return "5151|restored|Normal"; }
  if (req.lower && cls === "Normal") { cls = "BelowNormal"; return "5151|cargo.exe|1|10|Normal|lowered"; }
  return "5151|cargo.exe|1|10|" + cls + "|left";
};
MG.holdBuilds({ label: "child", run, resweepMs: 0, log: () => {} });
${how}
`,
  );
  const r = spawnSync(process.execPath, [script], { encoding: "utf8", windowsHide: true });
  const calls = fs.existsSync(logFile) ? fs.readFileSync(logFile, "utf8").trim().split(/\r?\n/) : [];
  fs.rmSync(dir, { recursive: true, force: true });
  return { code: r.status, calls };
}

test("RED: the hold puts the builds back on an uncaught error, on Ctrl+C and on a plain exit", () => {
  const thrown = childExit(`throw new Error("the rig hit a bug mid-capture");`);
  assert.strictEqual(thrown.code, 1, "an uncaught error still ends the process");
  assert.ok(thrown.calls.includes("restore"), `restored on an uncaught error: ${thrown.calls}`);
  // Ctrl+C with nobody else listening: the guard puts the builds back and ends the process the
  // way Ctrl+C would (130), rather than swallowing it.
  const ctrlC = childExit(`setInterval(() => {}, 1000); process.emit("SIGINT");`);
  assert.strictEqual(ctrlC.code, 130, "Ctrl+C still ends the process");
  assert.ok(ctrlC.calls.includes("restore"), `restored on Ctrl+C: ${ctrlC.calls}`);
  // Ctrl+C under the rig's own handler (process.exit(130)): the exit hook puts them back.
  const rigCtrlC = childExit(`process.on("SIGINT", () => process.exit(130)); process.emit("SIGINT");`);
  assert.strictEqual(rigCtrlC.code, 130);
  assert.ok(rigCtrlC.calls.includes("restore"), `restored under the rig's Ctrl+C handler: ${rigCtrlC.calls}`);
  const plain = childExit("");
  assert.strictEqual(plain.code, 0, "the hold keeps nothing alive: the script ends");
  assert.ok(plain.calls.includes("restore"), `restored on a plain exit: ${plain.calls}`);
});

test("on Windows, the real PowerShell lowers a real rustc this test started, and puts it back", (t) => {
  if (process.platform !== "win32") return t.skip("Windows only");
  const { spawn, execFileSync } = require("node:child_process");
  // The toolchain's own rustc.exe, not the rustup proxy on PATH: the process list names the
  // proxy rustup.exe, so the guard (rightly) refuses it as not a build.
  let rustcExe = null;
  try {
    rustcExe = require("node:path").join(execFileSync("rustc", ["--print", "sysroot"], { encoding: "utf8", windowsHide: true }).trim(), "bin", "rustc.exe");
  } catch {}
  if (!rustcExe || !require("node:fs").existsSync(rustcExe)) return t.skip("no Rust toolchain on PATH");
  // `rustc -` waits for its source on stdin: a real rustc.exe that does nothing until told.
  const child = spawn(rustcExe, ["-"], { stdio: ["pipe", "ignore", "ignore"], windowsHide: true });
  let failed = null;
  child.on("error", (e) => (failed = e));
  const priorityOf = (pid) =>
    execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", `[string](Get-Process -Id ${pid}).PriorityClass`], { encoding: "utf8", windowsHide: true }).trim();
  // A second child that is NOT a build (node, waiting a few seconds), which the made-up list
  // below calls cargo.exe: the script's own look at the process list must refuse it.
  const notBuild = spawn(process.execPath, ["-e", "setTimeout(() => {}, 15000)"], { stdio: "ignore", windowsHide: true });
  return new Promise((resolve) => setTimeout(resolve, 500)).then(() => {
    if (failed || !child.pid) {
      notBuild.kill();
      return t.skip(`no rustc to start (${failed && failed.message})`);
    }
    // Only this test's own two children are listed, so the real runner cannot reach any other
    // process; the node one under a build's name, as a pid reused by a non-build would be.
    MG.setLister(() => `${child.pid}|rustc.exe|\n${notBuild.pid}|cargo.exe|`);
    try {
      assert.strictEqual(priorityOf(child.pid), "Normal", "precondition");
      assert.strictEqual(priorityOf(notBuild.pid), "Normal", "precondition");
      const lines = [];
      const h = MG.holdBuilds({ label: "real", run: MG.runPowerShell, resweepMs: 0, log: (m) => lines.push(m) });
      assert.strictEqual(priorityOf(child.pid), "BelowNormal", `held: ${lines.join(" | ")}`);
      assert.strictEqual(priorityOf(notBuild.pid), "Normal", "a process that is not a build is never touched, whatever the list called it");
      // The start-time check, for real: told to put back this pid with another process's start
      // time (a reused pid), the restore leaves it alone.
      const other = MG.runPowerShell({ script: MG.restoreScript([{ pid: child.pid, start: "1", to: "Normal" }]) }).trim();
      assert.strictEqual(other, `${child.pid}|other`, "a different start time is a different process");
      assert.strictEqual(priorityOf(child.pid), "BelowNormal", "and it is left as it was");
      const s = h.release();
      assert.strictEqual(priorityOf(child.pid), "Normal", `put back: ${lines.join(" | ")}`);
      assert.deepStrictEqual(s.restored, [child.pid]);
      assert.deepStrictEqual(s.lowered.map((x) => x.pid), [child.pid], "only the real build was lowered");
    } finally {
      MG.setLister(null);
      child.stdin.end(); // empty source: rustc reports it has no main and exits on its own
      notBuild.kill(); // this test's own child
    }
  });
});
