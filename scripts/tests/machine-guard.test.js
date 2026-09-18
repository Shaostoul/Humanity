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
  for (const n of MG.WATCHED) assert.ok(q.includes(`Name='${n}'`), `query omits ${n}`);
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
    assert.deepStrictEqual(r, { free: true, waited_s: 0, blockers: [] });
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
    assert.deepStrictEqual(r.blockers.map((p) => p.pid), [5151]);
    assert.ok(lines.some((l) => l.includes("WAITING")), "entering the wait must be logged");
    assert.ok(lines.some((l) => l.includes("rustc.exe")), "the blocker must be named, not counted");
    assert.ok(lines.some((l) => l.includes("GAVE UP")), "giving up must be logged");
  });
});
