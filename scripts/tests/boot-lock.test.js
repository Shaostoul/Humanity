// The machine-wide rig boot lock (scripts/lib/boot-lock.js), the machine guard's use of it
// (machine-guard.js waitForFree) and spawnGame's (game-launch.js). Pure node, no game, no GPU:
// runs in `just rig-tests`.
//
// The failure it closes (2026-10-21, BUG-140's fixer): when the builds on the machine ended,
// every rig that was waiting on them saw a quiet machine in the same 30 s poll and booted, and
// two games fought over the one 12 GB card (a wgpu "Not enough memory left" on a 1.47 GB buffer,
// a frame loop that stopped logging 20 s after world entry; both passed when rerun alone). The
// guard could not see it: at the moment each rig looked, the other had no game process yet.
//
// Every lock here is a file under this test's own temp folder (HUMANITY_RIG_LOCK is pointed there
// first thing), so nothing in this file can take, or wait on, the real machine lock a rig of
// another session may be holding right now. The "other rig" is a node child process this test
// starts and stops itself.
//
// RED, 2026-10-21, recorded. First this file in a scratch copy of the tree whose
// lib/machine-guard.js, lib/game-launch.js, probe-sweep.js and verify-runtime.js were those of
// 888157493 (before the lock; boot-lock.js present, nothing using it). 7 of 11 failed:
//   ✖ RED: a lock held by another live rig is not quiet ...
//       AssertionError: the wait must not end while another rig holds the lock
//       true !== false          (waitForFree came back free at once: it never looked at the lock)
//   ✖ a lock whose holder was killed is stale ...
//       TypeError: Cannot read properties of undefined (reading 'pid')   (no lock was taken)
//   ✖ a fake process list or a test run never takes ...
//       TypeError: MG.bootLockPath is not a function
//   ✖ RED: two rigs woken by the same build ending never boot together
//       AssertionError: the two boots overlapped by 1500 ms: [rig-a 0..1500] [rig-b 0..1500]
//   ✖ RED: spawnGame will not start a game while another rig holds the lock ...
//       AssertionError: refused (exit 1) while another rig holds the lock; status 0
//   ✖ spawnGame takes the lock for a rig that has not ...
//       status 1: the child found no lock file to read once its game had started (ENOENT)
//   ✖ probe-sweep waits for another rig's boot lock before it boots ...
//       AssertionError: the sweep never waited for the lock
// The four that passed there test boot-lock.js on its own, so each was then run against this
// tree with one property of boot-lock.js broken in a scratch copy, and each went red alone:
//   the start-time comparison taken out of holderLive (the pid alone decides):
//     ✖ a lock whose pid now belongs to another process is stale ...
//       AssertionError: a live pid with another start time is a reused pid: taken over
//       ({"ok":false,"holder":{"pid":22652,...,"rig":"a-dead-rig",...}})
//   an unreadable lock treated as stale at once, however young:
//     ✖ a lock still being written is respected while young ...
//       AssertionError: a lock seconds old with nothing in it yet is a rig mid-write: held
//   the takeover file ignored (a stale lock removed while another rig takes it over):
//     ✖ a takeover never removes a lock a faster rig has just taken
//       AssertionError: someone else is taking it over: try again next look

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawn, spawnSync, execSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-boot-lock-test-"));
// Nothing below may reach the real lock: a lock any code here takes without naming a file lands
// in this folder.
process.env.HUMANITY_RIG_LOCK = path.join(tmpRoot, "default.lock");
process.on("exit", () => {
  removeJunctions(tmpRoot);
  fs.rmSync(tmpRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
});

const BL = require("../lib/boot-lock.js");
const MG = require("../lib/machine-guard.js");
const GL = require("../lib/game-launch.js");

let n = 0;
const lockFile = () => path.join(tmpRoot, `lock${++n}`, "rig-boot.lock");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
async function until(fn, ms = 15000) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    const v = fn();
    if (v) return v;
    await sleep(50);
  }
  return fn();
}
const kill = (pid) => {
  try {
    execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });
  } catch {}
};
/** A probe rig leaves junctions to the tree's data/ and assets/: remove those as links before
 *  anything removes the folder that holds them. */
function removeJunctions(dir) {
  const walk = (d, depth) => {
    let entries = [];
    try {
      entries = fs.readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const p = path.join(d, e.name);
      let st;
      try {
        st = fs.lstatSync(p);
      } catch {
        continue;
      }
      if (st.isSymbolicLink()) {
        try {
          fs.rmdirSync(p);
        } catch {
          try {
            fs.unlinkSync(p);
          } catch {}
        }
      } else if (st.isDirectory() && depth < 3) walk(p, depth + 1);
    }
  };
  walk(dir, 0);
}

/** Another rig: a node process that takes the lock and holds it until it is killed. */
function otherRig(file, rig = "the-other-rig") {
  const script =
    `const BL = require(${JSON.stringify(require.resolve("../lib/boot-lock.js"))});` +
    `const r = BL.tryAcquire(${JSON.stringify(file)}, { rig: ${JSON.stringify(rig)} });` +
    `console.log(r.ok ? "HELD" : "NOT HELD " + JSON.stringify(r));` +
    `setTimeout(() => {}, 120000);`;
  const child = spawn(process.execPath, ["-e", script], { stdio: ["ignore", "pipe", "inherit"], windowsHide: true });
  return new Promise((resolve, reject) => {
    let out = "";
    child.stdout.on("data", (d) => {
      out += d;
      if (/^HELD/m.test(out)) resolve(child);
      else if (/NOT HELD/.test(out)) reject(new Error(out));
    });
    child.on("exit", () => reject(new Error(`the other rig exited: ${out}`)));
  });
}

// ── the lock itself ──────────────────────────────────────────────────────────
test("the lock is taken when free, names this process and its start time, is re-entrant, and goes when released", () => {
  const f = lockFile();
  const r = BL.tryAcquire(f, { rig: "this-test" });
  assert.strictEqual(r.ok, true, JSON.stringify(r));
  assert.strictEqual(r.taken, true);
  const rec = JSON.parse(fs.readFileSync(f, "utf8"));
  assert.strictEqual(rec.pid, process.pid);
  assert.strictEqual(rec.rig, "this-test");
  if (process.platform === "win32") assert.ok(Number.isFinite(rec.start_ms), "the start time is recorded, so a reused pid can be told apart");
  // A second boot in the same rig (verify-copresence, make-clips, boot-timing boot several times).
  const again = BL.tryAcquire(f, { rig: "this-test" });
  assert.strictEqual(again.ok, true);
  assert.strictEqual(again.taken, false, "already ours: nothing new taken");
  assert.strictEqual(BL.release(f), true);
  assert.ok(!fs.existsSync(f), "released: the file is gone");
  assert.strictEqual(BL.release(f), false, "a second release is a no-op");
});

test("RED: a lock held by another live rig is not quiet: the guard waits, names the holder, and gives up", async () => {
  const f = lockFile();
  const other = await otherRig(f);
  try {
    MG.setLister(() => ""); // an empty machine: only the lock is in the way
    const lines = [];
    const t0 = Date.now();
    const w = MG.waitForFree({ lockFile: f, timeoutMs: 1000, pollMs: 100, label: "pre-boot", rig: "this-test", log: (m) => lines.push(m) });
    assert.strictEqual(w.free, false, "the wait must not end while another rig holds the lock");
    assert.ok(Date.now() - t0 >= 900, "it waited, rather than falling through");
    assert.strictEqual(w.blockers.length, 1);
    assert.strictEqual(w.blockers[0].lock.pid, other.pid, "the holder is named by its pid");
    assert.match(MG.describe(w.blockers[0]), new RegExp(`the rig boot lock, held by the-other-rig pid ${other.pid}`));
    assert.ok(lines.some((l) => /WAITING/.test(l)) && lines.some((l) => /GAVE UP/.test(l)), lines.join("\n"));
    assert.strictEqual(w.limit_ms, 1000);
    // The one refusal every rig gives, with the reader's way out.
    const refusal = MG.refusalLines(w, { rig: "this-test" });
    assert.strictEqual(refusal[0], "this-test: REFUSED: the machine never went quiet in 1 s; nothing was booted.");
    assert.ok(refusal.some((l) => l.includes(`taskkill //PID ${other.pid}`)), refusal.join("\n"));
    assert.ok(refusal.some((l) => /says nothing about the build/.test(l)));
    assert.strictEqual(JSON.parse(fs.readFileSync(f, "utf8")).pid, other.pid, "the holder's lock is untouched");
  } finally {
    MG.setLister(null);
    kill(other.pid);
  }
});

test("a lock whose holder was killed is stale, and the next rig takes it over (and says so)", async () => {
  const f = lockFile();
  const other = await otherRig(f);
  kill(other.pid); // taskkill /F: no exit handler runs, the file stays behind
  assert.ok(await until(() => !alive(other.pid), 10000), "the other rig is gone");
  assert.ok(fs.existsSync(f), "precondition: a hard kill leaves the lock file");
  MG.setLister(() => "");
  try {
    const lines = [];
    const w = MG.waitForFree({ lockFile: f, timeoutMs: 1000, pollMs: 100, label: "pre-boot", rig: "this-test", log: (m) => lines.push(m) });
    assert.strictEqual(w.free, true, lines.join("\n"));
    assert.strictEqual(w.waited, false, "a dead holder is no reason to wait");
    assert.strictEqual(w.lock.pid, process.pid);
    assert.strictEqual(JSON.parse(fs.readFileSync(f, "utf8")).pid, process.pid);
    assert.ok(lines.some((l) => l.includes(`took over a stale boot lock from the-other-rig pid ${other.pid}`)), lines.join("\n"));
  } finally {
    MG.setLister(null);
    BL.release(f);
  }
});

test("a lock whose pid now belongs to another process is stale (its start time differs); the true one is held", async (t) => {
  // A plain process that holds nothing: its pid stands for "a dead rig's number, reused".
  const bystander = spawn(process.execPath, ["-e", "setTimeout(() => {}, 120000)"], { stdio: "ignore", windowsHide: true });
  try {
    await sleep(300);
    const start = BL.osStartMs(bystander.pid);
    if (start == null) return t.skip("this machine cannot report a process start time");
    // Positive control first: the pid AND its real start time is a live holder, kept.
    const f1 = lockFile();
    fs.mkdirSync(path.dirname(f1), { recursive: true });
    fs.writeFileSync(f1, JSON.stringify({ pid: bystander.pid, start_ms: start, rig: "a-live-rig", since: new Date().toISOString() }));
    const kept = BL.tryAcquire(f1, { rig: "this-test" });
    assert.strictEqual(kept.ok, false, "the pid with its own start time is the holder itself: held");
    assert.strictEqual(kept.holder.pid, bystander.pid);
    // The same pid with an hour-earlier start time: the process that wrote it is gone.
    const f2 = lockFile();
    fs.mkdirSync(path.dirname(f2), { recursive: true });
    fs.writeFileSync(f2, JSON.stringify({ pid: bystander.pid, start_ms: start - 3600 * 1000, rig: "a-dead-rig", since: new Date().toISOString() }));
    const r = BL.tryAcquire(f2, { rig: "this-test" });
    assert.strictEqual(r.ok, true, `a live pid with another start time is a reused pid: taken over (${JSON.stringify(r)})`);
    assert.strictEqual(r.stale.rig, "a-dead-rig");
    BL.release(f2);
  } finally {
    bystander.kill();
  }
});

test("a lock still being written is respected while young, and taken when it is old junk", () => {
  const f = lockFile();
  fs.mkdirSync(path.dirname(f), { recursive: true });
  fs.writeFileSync(f, ""); // a rig between its create and its write
  const young = BL.tryAcquire(f, { rig: "this-test" });
  assert.strictEqual(young.ok, false, "a lock seconds old with nothing in it yet is a rig mid-write: held");
  const old = (Date.now() - 60 * 1000) / 1000;
  fs.utimesSync(f, old, old);
  const r = BL.tryAcquire(f, { rig: "this-test" });
  assert.strictEqual(r.ok, true, "a minute-old empty lock is junk a crash left: taken over");
  BL.release(f);
});

test("a takeover never removes a lock a faster rig has just taken", () => {
  // The takeover file is busy (another rig is mid-takeover): this rig does not touch the lock.
  const f = lockFile();
  fs.mkdirSync(path.dirname(f), { recursive: true });
  fs.writeFileSync(f, JSON.stringify({ pid: 999999991, start_ms: 1, rig: "dead" })); // odd: no Windows pid
  fs.writeFileSync(`${f}.takeover`, "");
  const r = BL.tryAcquire(f, { rig: "this-test" });
  assert.strictEqual(r.ok, false, "someone else is taking it over: try again next look");
  assert.ok(fs.existsSync(f), "and the lock is left for them");
  fs.unlinkSync(`${f}.takeover`);
  assert.strictEqual(BL.tryAcquire(f, { rig: "this-test" }).ok, true, "free of contention, the stale lock is taken");
  BL.release(f);
});

test("a fake process list or a test run never takes the operator's real machine lock", () => {
  const saved = { lock: process.env.HUMANITY_RIG_LOCK, ctx: process.env.NODE_TEST_CONTEXT, fake: process.env.HUMANITY_MACHINE_GUARD_FAKE };
  try {
    delete process.env.HUMANITY_RIG_LOCK;
    delete process.env.HUMANITY_MACHINE_GUARD_FAKE;
    process.env.NODE_TEST_CONTEXT = "child-v8";
    assert.strictEqual(BL.lockPath(), null, "under node --test: no lock unless one is named");
    delete process.env.NODE_TEST_CONTEXT;
    assert.strictEqual(BL.lockPath({ fake: true }), null, "a fake process list: no lock unless one is named");
    MG.setLister(() => "");
    assert.strictEqual(MG.bootLockPath(), null, "the guard with a fake list in hand");
    MG.setLister(null);
    assert.strictEqual(BL.lockPath(), BL.DEFAULT_PATH, "a real run: the machine-wide file");
    assert.match(BL.DEFAULT_PATH, /HumanityOS[\\/]rig-boot\.lock$/);
    process.env.HUMANITY_RIG_LOCK = path.join(tmpRoot, "named.lock");
    assert.strictEqual(BL.lockPath({ fake: true }), path.join(tmpRoot, "named.lock"), "a named lock is used even with a fake list");
  } finally {
    MG.setLister(null);
    for (const [k, v] of [["HUMANITY_RIG_LOCK", saved.lock], ["NODE_TEST_CONTEXT", saved.ctx], ["HUMANITY_MACHINE_GUARD_FAKE", saved.fake]]) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
  }
});

// ── two rigs woken together ─────────────────────────────────────────────────
// The incident itself: two rigs wait on the same build; the build ends; both see a quiet machine
// in the same moment. Each "boots" (holds for 1.5 s) the moment its guard lets it. The two boots
// must not overlap. Each child is a real process with its own fake process list (a cargo.exe
// until the go-file exists), polling every 50 ms so both wake within one poll of each other.
test("RED: two rigs woken by the same build ending never boot together", async () => {
  const f = lockFile();
  const k = n;
  const go = path.join(tmpRoot, `go-${k}`);
  const rec = (name) => path.join(tmpRoot, `boot-${k}-${name}.json`);
  const rigScript = (name) => `
    const fs = require("fs");
    const MG = require(${JSON.stringify(require.resolve("../lib/machine-guard.js"))});
    MG.setLister(() => (fs.existsSync(${JSON.stringify(go)}) ? "" : "4242|cargo.exe|C:\\\\elsewhere\\\\cargo.exe"));
    const w = MG.waitForFree({ lockFile: ${JSON.stringify(f)}, timeoutMs: 30000, pollMs: 50, label: "pre-boot", rig: ${JSON.stringify(name)}, log: (m) => console.log(m) });
    if (!w.free) { console.log("GAVE UP"); process.exit(3); }
    const start = Date.now();
    const end = start + 1500; while (Date.now() < end) {}  // "the boot": the lock is held throughout
    fs.writeFileSync(${JSON.stringify(rec(name))}, JSON.stringify({ start, end: Date.now() }));
  `;
  const kids = ["rig-a", "rig-b"].map((name) => {
    const c = spawn(process.execPath, ["-e", rigScript(name)], { stdio: ["ignore", "pipe", "inherit"], windowsHide: true });
    c.out = "";
    c.stdout.on("data", (d) => (c.out += d));
    c.done = new Promise((r) => c.on("exit", r));
    return c;
  });
  try {
    assert.ok(await until(() => kids.every((c) => /WAITING/.test(c.out)), 20000), "both rigs are waiting on the build");
    fs.writeFileSync(go, ""); // the build ends
    const codes = await Promise.race([Promise.all(kids.map((c) => c.done)), sleep(40000).then(() => "timeout")]);
    assert.deepStrictEqual(codes, [0, 0], kids.map((c) => c.out).join("\n---\n"));
    const [a, b] = ["rig-a", "rig-b"].map((name) => JSON.parse(fs.readFileSync(rec(name), "utf8")));
    const overlap = Math.min(a.end, b.end) - Math.max(a.start, b.start);
    const t0 = Math.min(a.start, b.start);
    assert.ok(
      overlap <= 0,
      `the two boots overlapped by ${overlap} ms: [rig-a ${a.start - t0}..${a.end - t0}] [rig-b ${b.start - t0}..${b.end - t0}]`
    );
    assert.ok(!fs.existsSync(f), "each rig's lock went when its process ended");
  } finally {
    for (const c of kids) kill(c.pid);
  }
});

// ── spawnGame: every rig boots through it ───────────────────────────────────
const judged = (exe) => ({ result: { exe, exe_sha256: GL.fileSha256(exe), exe_fingerprint: null, verdict: "current" } });
function spawnGameChild(f, marker, extra = "") {
  const script = `
    const GL = require(${JSON.stringify(path.join(REPO, "scripts", "lib", "game-launch.js"))});
    const fs = require("fs");
    const w = GL.spawnGame(process.execPath, ["-e", "require('fs').writeFileSync(process.argv[1], 'x')", ${JSON.stringify(marker)}], {
      fresh: ${JSON.stringify(judged(process.execPath))}, rigName: "the-test-rig", lockWaitMs: 1500, log: (m) => console.log(m),
    });
    console.log("LOCK " + fs.readFileSync(${JSON.stringify(f)}, "utf8").replace(/\\s+/g, " "));
    ${extra}
    w.child.on("exit", () => process.exit(0));
  `;
  return spawnSync(process.execPath, ["-e", script], {
    encoding: "utf8",
    timeout: 60000,
    env: { ...process.env, HUMANITY_RIG_LOCK: f, HUMANITY_MACHINE_GUARD_POLL_MS: "100" },
  });
}

test("RED: spawnGame will not start a game while another rig holds the lock: it waits, then refuses, starting nothing", async () => {
  const f = lockFile();
  const marker = path.join(tmpRoot, `started-${n}.txt`);
  const other = await otherRig(f);
  try {
    const r = spawnGameChild(f, marker);
    const out = `${r.stdout}\n${r.stderr}`;
    assert.strictEqual(r.status, 1, `refused (exit 1) while another rig holds the lock; status ${r.status}\n${out}`);
    assert.match(out, /the-test-rig: boot lock: WAITING/);
    assert.match(out, /the-test-rig: REFUSED: the machine never went quiet in 1\.5 s; nothing was booted\./);
    assert.match(out, new RegExp(`the rig boot lock, held by the-other-rig pid ${other.pid}`));
    assert.ok(!fs.existsSync(marker), "nothing was started");
  } finally {
    kill(other.pid);
  }
});

test("spawnGame takes the lock for a rig that has not, holds it while the rig runs, and the rig's exit lets it go", () => {
  const f = lockFile();
  const marker = path.join(tmpRoot, `started-${n}.txt`);
  const r = spawnGameChild(f, marker);
  const out = `${r.stdout}\n${r.stderr}`;
  assert.strictEqual(r.status, 0, out);
  const m = out.match(/LOCK (\{.*\})/);
  assert.ok(m, `the lock was held when the game started:\n${out}`);
  assert.strictEqual(JSON.parse(m[1]).rig, "the-test-rig");
  assert.ok(fs.existsSync(marker), "the game started");
  assert.ok(!fs.existsSync(f), "the rig ended: the lock is gone");
});

// ── a real rig waits for the lock (probe-sweep, end to end) ─────────────────
// The sweep is started on a file that carries this tree's source stamp but is not a program, so
// it gets all the way to the start of the "game", where Windows refuses it (nothing boots). The
// process list is faked empty: only the other rig's lock is in the way.
function stampedFake() {
  const L = require("../lib/src-fingerprint.js");
  const tree = L.fingerprintTree(REPO);
  const stamp =
    `HOS-SRC-STAMP v1\nfingerprint ${tree.fingerprint}\nfiles ${tree.files.size}\nprofile release\nfeatures native\n` +
    `${tree.text}HOS-SRC-STAMP END\n`;
  return Buffer.concat([crypto.randomBytes(4096), Buffer.from(stamp, "utf8")]);
}
test("probe-sweep waits for another rig's boot lock before it boots, and takes it once that rig is gone", async () => {
  const f = lockFile();
  const exe = path.join(tmpRoot, `stamped-${n}.exe`);
  fs.writeFileSync(exe, stampedFake());
  const rig = path.join(tmpRoot, `sweep-rig-${n}`);
  const empty = path.join(tmpRoot, `no-processes-${n}.txt`);
  fs.writeFileSync(empty, "");
  const other = await otherRig(f);
  const child = spawn(process.execPath, [path.join(REPO, "scripts", "probe-sweep.js"), "--exe", exe, "--rig", rig, "--only", "home-overview-noon"], {
    env: { ...process.env, HUMANITY_MACHINE_GUARD_FAKE: empty, HUMANITY_MACHINE_GUARD_POLL_MS: "300", HUMANITY_RIG_LOCK: f },
    windowsHide: true,
  });
  let out = "";
  let released = false;
  const onData = (d) => {
    out += d.toString();
    if (!released && /pre-boot: WAITING/.test(out) && /the rig boot lock, held by the-other-rig/.test(out)) {
      released = true;
      kill(other.pid); // the other rig ends (hard: its lock goes stale)
    }
  };
  child.stdout.on("data", onData);
  child.stderr.on("data", onData);
  const status = await new Promise((resolve) => {
    const t = setTimeout(() => {
      kill(child.pid);
      resolve("timeout");
    }, 120000);
    child.on("exit", (code) => {
      clearTimeout(t);
      resolve(code);
    });
  });
  kill(other.pid);
  removeJunctions(rig);
  assert.ok(released, `the sweep never waited for the lock:\n${out.slice(-2000)}`);
  assert.match(out, /took over a stale boot lock from the-other-rig/, out.slice(-2000));
  assert.match(out, /probe-sweep: could not start .*HumanityOS.exe .*nothing booted/, out.slice(-2000));
  assert.strictEqual(status, 1, `exit ${status}`);
});
