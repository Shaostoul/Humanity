// verify-runtime's two clocks, its refusal and its flags (scripts/verify-runtime.js). Pure node,
// no game, no GPU: runs in `just rig-tests`.
//
// The failure it closes (2026-10-21, two false failures in one afternoon): the gate's 12-minute
// clock started with the recipe, and the machine guard in front of the boot (probe-sweep's wait
// while other agents' cargo and rustc ran) used it up. The gate killed its probe before anything
// had booted and printed "RESULT: FAIL the probe never got far enough to write a manifest ... A
// hang here is a boot or world-entry hang, not slowness". Nothing had hung; nothing had booted.
// And `--game-only`, the flag that stops a panics-only check waiting on builds, was silently
// dropped: verify-runtime never passed it to the sweep, and never said it had ignored it.
//
// How these run without a game: the "exe" carries this tree's source stamp, so the freshness gate
// passes it, and the machine guard's process list is faked (a cargo.exe that the test takes away
// when it chooses). The first test's exe is node itself with the stamp appended, which Windows
// runs as node: a NODE_OPTIONS preload makes it sit still, as a game that hangs in its boot
// would, so the gate's clock can be seen to start with it and to run out on it. The other tests'
// exe is not a program at all, so the sweep stops where it would start the game (nothing boots).
// Every run has its own rig folder and lock file under this test's temp folder.
//
// RED, 2026-10-21, recorded: this file in a scratch copy of the tree whose verify-runtime.js,
// probe-sweep.js, lib/machine-guard.js and lib/game-launch.js were those of 888157493. All 5 failed,
// four of them with the very transcript of the two false failures:
//   ✖ the gate's clock starts with the game: ...
//       AssertionError: the gate fired while the probe was still waiting for the machine:
//       [machine-guard] pre-boot: WAITING - 1 process(es) competing for this machine:
//       [gate] TIMEOUT after 0.05 min - killing the probe.
//       RESULT: FAIL  the probe never got far enough to write a manifest (probe exit 1).
//       The run was killed after 0.05 minutes. A hang here is a boot or
//       world-entry hang, not slowness: ...
//   ✖ a machine that never goes quiet is REFUSED ...
//       AssertionError: exit 2, not 1   (the same TIMEOUT and "RESULT: FAIL", no REFUSED anywhere)
//   ✖ --game-only reaches the sweep ...
//       AssertionError: the sweep waited on a build although --game-only was given:
//       [machine-guard] pre-boot: WAITING - 1 process(es) competing for this machine:
//       [gate] TIMEOUT after 0.05 min - killing the probe.
//   ✖ an argument verify-runtime does not know is refused ...
//       AssertionError: exit 2, not 1 (it ran, ignoring --gmae-only?)
//       RESULT: FAIL  the probe never got far enough to write a manifest (probe exit 1).
//       (a third false failure: the sweep had only refused to start a file that is not a program,
//       and the old gate called that a boot failure)
//   ✖ a time that is not a number is refused ...
//       AssertionError: exit 2
//       [gate] TIMEOUT after NaN min - killing the probe.
// And the first test's positive control, red on its own: with the gate's phase poll taken out of
// this tree's verify-runtime.js (a scratch copy), so its clock can never start,
//   ✖ the gate's clock starts with the game: ...
//       AssertionError: a game that never finished booting was not timed out:
// so the test cannot pass by a gate that simply never fires.

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawn, execSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const SCRIPT = path.join(REPO, "scripts", "verify-runtime.js");
const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-verify-runtime-gate-test-"));
process.env.HUMANITY_RIG_LOCK = path.join(tmpRoot, "default.lock"); // never the real machine lock
process.on("exit", () => {
  removeJunctions(tmpRoot);
  fs.rmSync(tmpRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
});
const L = require("../lib/src-fingerprint.js");

let n = 0;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const kill = (pid) => {
  try {
    execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });
  } catch {}
};
/** The rig holds junctions to the tree's data/ and assets/: remove them as links before anything
 *  removes the folder. */
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

let stampText = null;
function stamp() {
  if (!stampText) {
    const tree = L.fingerprintTree(REPO);
    stampText =
      `HOS-SRC-STAMP v1\nfingerprint ${tree.fingerprint}\nfiles ${tree.files.size}\nprofile release\nfeatures native\n` +
      `${tree.text}HOS-SRC-STAMP END\n`;
  }
  return Buffer.from(stampText, "utf8");
}
/** This tree's stamp on bytes that are not a program: the sweep gets as far as starting it. */
function stampedNonProgram() {
  const f = path.join(tmpRoot, `nonprogram-${++n}.exe`);
  fs.writeFileSync(f, Buffer.concat([crypto.randomBytes(4096), stamp()]));
  return f;
}
/** This tree's stamp on node itself: a program Windows runs, which the freshness gate passes. */
function stampedNode() {
  const f = path.join(tmpRoot, `stamped-node-${++n}.exe`);
  fs.writeFileSync(f, Buffer.concat([fs.readFileSync(process.execPath), stamp()]));
  return f;
}

/** Run verify-runtime. `procs` is the faked process list (a file the test may rewrite);
 *  `onOut(text, child)` sees the output as it comes. */
function runGate(argv, { procs = "", env = {}, onOut = () => {}, limitMs = 150000 } = {}) {
  const k = ++n;
  const rig = path.join(tmpRoot, `rig-${k}`);
  const list = path.join(tmpRoot, `procs-${k}.txt`);
  fs.writeFileSync(list, procs);
  const t0 = Date.now();
  const child = spawn(process.execPath, [SCRIPT, "--rig", rig, ...argv], {
    cwd: REPO,
    windowsHide: true,
    env: {
      ...process.env,
      HUMANITY_MACHINE_GUARD_FAKE: list,
      HUMANITY_MACHINE_GUARD_POLL_MS: "300",
      HUMANITY_RIG_LOCK: path.join(tmpRoot, `lock-${k}`, "rig-boot.lock"),
      ...env,
    },
  });
  let out = "";
  const lines = []; // [ms since start, line] for ordering
  const take = (d) => {
    const s = d.toString();
    out += s;
    for (const l of s.split(/\r?\n/)) if (l.trim()) lines.push([Date.now() - t0, l]);
    onOut(out, { list, rig, t0 });
  };
  child.stdout.on("data", take);
  child.stderr.on("data", take);
  return new Promise((resolve) => {
    const t = setTimeout(() => {
      kill(child.pid);
      resolve({ status: "timeout", out, lines, rig, list });
    }, limitMs);
    child.on("exit", (code) => {
      clearTimeout(t);
      removeJunctions(rig);
      // HOS_TEST_SHOW=1: print each run's whole transcript, to read what a person would see.
      if (process.env.HOS_TEST_SHOW) console.log(`----- verify-runtime ${argv.join(" ")} (exit ${code})\n${out}`);
      resolve({ status: code, out, lines, rig, list });
    });
  });
}
const BUILD = "4242|cargo.exe|C:\\elsewhere\\cargo.exe\n";
const tail = (r) => r.out.slice(-2500);

test("the gate's clock starts with the game: a machine wait longer than the gate does not trip it, a hang after the boot does", async () => {
  const exe = stampedNode();
  const started = path.join(tmpRoot, `fake-game-started-${n}.txt`);
  // Preloaded into every node this run starts; it acts only in the one running as the rig's
  // HumanityOS.exe: that "game" records its pid and sits still, as a boot that never finishes.
  const hang = path.join(tmpRoot, `hang-${n}.js`);
  fs.writeFileSync(
    hang,
    `if (/HumanityOS\\.exe$/i.test(process.execPath)) {
       require("fs").writeFileSync(process.env.HOS_TEST_FAKE_GAME_STARTED, String(process.pid));
       setInterval(() => {}, 1000);
       setTimeout(() => process.exit(0), 90000);
     }\n`
  );
  const GATE_S = 3; // --timeout-min 0.05
  let waitingAt = null;
  let clearedAt = null;
  const r = await runGate(["--exe", exe, "--timeout-min", "0.05", "--wait-min", "1"], {
    procs: BUILD,
    // Forward slashes: NODE_OPTIONS reads a backslash inside quotes as an escape.
    env: { NODE_OPTIONS: `--require "${hang.replace(/\\/g, "/")}"`, HOS_TEST_FAKE_GAME_STARTED: started },
    onOut: (out, { list, t0 }) => {
      if (waitingAt == null && /pre-boot: WAITING/.test(out)) {
        waitingAt = Date.now() - t0;
        // The build ends twice the gate's length later: a gate timing the wait would fire first.
        setTimeout(() => {
          clearedAt = Date.now() - t0;
          fs.writeFileSync(list, "");
        }, (GATE_S + 3) * 1000);
      }
    },
  });
  let gamePid = null;
  try {
    gamePid = Number(fs.readFileSync(started, "utf8")) || null;
  } catch {}
  if (gamePid) kill(gamePid);
  const at = (re) => (r.lines.find(([, l]) => re.test(l)) || [null])[0];
  const timeoutAt = at(/TIMEOUT/);
  const clockAt = at(/gate clock starts now/);
  assert.ok(waitingAt != null, `the sweep never waited for the faked build:\n${tail(r)}`);
  assert.ok(timeoutAt == null || (clearedAt != null && timeoutAt > clearedAt), `the gate fired while the probe was still waiting for the machine:\n${tail(r)}`);
  assert.ok(gamePid, `the fake game never started:\n${tail(r)}`);
  assert.ok(clockAt != null, `the gate never said its clock started:\n${tail(r)}`);
  const waited = Number((r.out.match(/the game started \(pid \d+\) (\d+) s after the probe did/) || [])[1]);
  assert.ok(waited >= GATE_S + 2, `the wait before the boot (${waited} s) was longer than the gate (${GATE_S} s), so this test means something`);
  // And the gate still catches a hang once the game exists: its positive control.
  assert.ok(timeoutAt != null && timeoutAt > clockAt, `a game that never finished booting was not timed out:\n${tail(r)}`);
  assert.match(r.out, /TIMEOUT: 3 s since the game started/);
  assert.match(r.out, /RESULT: FAIL {2}the probe never got far enough/);
  assert.match(r.out, /killed 3 s after the game started \(the \d+ s before that/);
  assert.strictEqual(r.status, 2, `a hang after the boot is a failure (exit 2):\n${tail(r)}`);
  // The verdict says which shader compiler the game had, by the game's own test (the pair beside
  // the rig's exe). On this machine the sweep finds the pair in the main checkout, or does not.
  const dxc = ["dxcompiler.dll", "dxil.dll"].every((d) => fs.existsSync(path.join(r.rig, d)));
  const verdict = r.out.slice(r.out.lastIndexOf("-".repeat(72), r.out.indexOf("RESULT:")));
  if (dxc) assert.match(verdict, /shader compiler {2}DXC/, `the verdict names DXC:\n${tail(r)}`);
  else assert.match(verdict, /SHADER COMPILER: FXC/, `the verdict names FXC, loudly:\n${tail(r)}`);
});

test("a machine that never goes quiet is REFUSED, exit 1, nothing booted: not a FAIL", async () => {
  const exe = stampedNonProgram();
  const r = await runGate(["--exe", exe, "--timeout-min", "0.05", "--wait-min", "0.05"], { procs: BUILD });
  assert.strictEqual(r.status, 1, `exit ${r.status}, not 1\n${tail(r)}`);
  assert.match(r.out, /probe-sweep: REFUSED: the machine never went quiet in 3 s; nothing was booted\./);
  assert.match(r.out, /RESULT: REFUSED {2}the machine never went quiet in 3 s; nothing was booted\./);
  assert.match(r.out, /cargo\.exe pid 4242/, "what was still up is named");
  assert.match(r.out, /not a verdict on the build/);
  assert.doesNotMatch(r.out, /RESULT: FAIL|TIMEOUT/);
  assert.ok(!fs.existsSync(path.join(r.rig, "HumanityOS.exe")), "nothing was copied into the rig, let alone booted");
});

test("--game-only reaches the sweep: a build does not hold a games-only run", async () => {
  const exe = stampedNonProgram();
  const r = await runGate(["--exe", exe, "--game-only", "--timeout-min", "0.05", "--wait-min", "0.05"], { procs: BUILD });
  assert.doesNotMatch(r.out, /pre-boot: WAITING/, `the sweep waited on a build although --game-only was given:\n${tail(r)}`);
  assert.match(r.out, /games only|other games and rigs only/);
  // It went on to the start of the "game", which is not a program: nothing booted, exit 1.
  assert.match(r.out, /probe-sweep: could not start .*HumanityOS.exe .*nothing booted/, tail(r));
  assert.match(r.out, /RESULT: REFUSED {2}the probe stopped before it started the game \(probe exit 1\); nothing was booted\./);
  assert.strictEqual(r.status, 1, tail(r));
});

test("an argument verify-runtime does not know is refused, not ignored", async () => {
  const exe = stampedNonProgram();
  const r = await runGate(["--exe", exe, "--gmae-only", "--timeout-min", "0.05"], { procs: "" });
  assert.strictEqual(r.status, 1, `exit ${r.status}, not 1 (it ran, ignoring --gmae-only?)\n${tail(r)}`);
  assert.match(r.out, /verify-runtime: REFUSED - unknown argument\(s\): --gmae-only\. Nothing was booted\./);
  assert.match(r.out, /--game-only/, "the known flags are listed, so the typo is obvious");
  assert.doesNotMatch(r.out, /\[sweep\]|\[fresh\]/, "refused before anything ran");
});

test("a time that is not a number is refused (a NaN clock would kill its own probe at once)", async () => {
  const exe = stampedNonProgram();
  const r = await runGate(["--exe", exe, "--timeout-min", "twelve"], { procs: "" });
  assert.strictEqual(r.status, 1, `exit ${r.status}\n${tail(r)}`);
  assert.match(r.out, /--timeout-min must be a number of minutes above 0, got "twelve"/);
});
