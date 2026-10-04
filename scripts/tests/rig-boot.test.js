// The binary a rig BOOTS is the binary the freshness gate CHECKED (BUG-133, the
// first two gaps). Pure node, no game, no cargo: runs in `just rig-tests`.
//
// Gap 1: some scripts that boot the game never ran the gate (probe-sweep, and
// so verify-runtime's sweep; photograph-home; make-clips; boot-timing).
// Gap 2: every rig copies the exe into its own folder AFTER the gate checked
// the original, so a build finishing in between put an unchecked binary in the
// rig; and a launch could hand itself off to a newer v*_HumanityOS.exe in
// C:\Humanity (src/main.rs find_newer_exe), so the process that ran was not
// the file anybody checked.
//
// RED FIRST (2026-10-03, this file run against the scripts and src as they
// were at 8ae0ba0d4, before the fix):
//   ✖ checkBootCopy: an identical copy passes, a changed one is refused ...
//       TypeError: L.checkBootCopy is not a function
//   ✖ the gate records the hash of the bytes it judged
//       actual: undefined  expected: '<sha256 of the fake exe>'
//   ✖ every script that boots the game gates it, checks its boot copy, and
//     keeps it from handing off
//       scripts/boot-timing.js: no runFreshGate(, no requireBootCopy(, no HUMANITY_NO_HANDOFF
//       scripts/make-clips.js: ... scripts/photograph-home.js: ... scripts/probe-sweep.js: ...
//       scripts/verify-copresence.js: no requireBootCopy(, no HUMANITY_NO_HANDOFF  (and the other two)
//   ✖ verify-runtime forwards --allow-other-build to the sweep it starts
//   ✖ find_newer_exe asks handoff_blocked_now before it looks for a newer build
//   ✖ probe-sweep refuses an unchecked exe before it builds or touches its rig
//       (it went on to create the rig folder and copy the file into it)

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const L = require("../lib/src-fingerprint.js");

const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-rig-boot-test-"));
process.on("exit", () => fs.rmSync(tmpRoot, { recursive: true, force: true }));
let n = 0;
const sha256 = (b) => crypto.createHash("sha256").update(b).digest("hex");
const tmpFile = (content) => {
  const f = path.join(tmpRoot, `f${++n}.exe`);
  fs.writeFileSync(f, content);
  return f;
};

// ── checkBootCopy ────────────────────────────────────────────────────────────
test("checkBootCopy: an identical copy passes, a changed one is refused, no record is refused", () => {
  const exe = tmpFile(crypto.randomBytes(2048));
  const fresh = { result: { exe, exe_sha256: sha256(fs.readFileSync(exe)), exe_fingerprint: null, verdict: "current" } };
  const same = tmpFile(fs.readFileSync(exe));
  assert.strictEqual(L.checkBootCopy(same, fresh).ok, true);

  const changed = tmpFile(Buffer.concat([fs.readFileSync(exe), Buffer.from([0])]));
  const r = L.checkBootCopy(changed, fresh);
  assert.strictEqual(r.ok, false);
  assert.match(r.lines.join("\n"), /BOOT COPY CHANGED/);
  assert.match(r.lines.join("\n"), new RegExp(fresh.result.exe_sha256));

  assert.strictEqual(L.checkBootCopy(same, { result: null }).ok, false, "no gate record: nothing to compare against");
  assert.strictEqual(L.checkBootCopy(path.join(tmpRoot, "nope.exe"), fresh).ok, false, "a missing copy");
});

test("the gate records the hash of the bytes it judged", () => {
  const exe = tmpFile(crypto.randomBytes(1024)); // unstamped: refused, but still hashed
  const g = L.runFreshGate(exe, ["--allow-other-build", "test: hashing an unstamped file"], { stdio: "pipe" });
  assert.strictEqual(g.status, 0, g.stdout + g.stderr);
  assert.strictEqual(g.result.exe_sha256, sha256(fs.readFileSync(exe)));
});

// ── Every script that boots the game ─────────────────────────────────────────
// A script "boots the game" when it names the game exe AND spawns a process.
// Each one found must be listed here, so a new rig cannot be added without
// deciding this.
const BOOTS_THE_GAME = [
  "scripts/probe-sweep.js",
  "scripts/photograph-home.js",
  "scripts/make-clips.js",
  "scripts/boot-timing.js",
  "scripts/verify-copresence.js",
  "scripts/verify-live-screen.js",
  "scripts/verify-screens.js",
];
const NOT_A_RIG = {
  "scripts/archive-build.js":
    "`just launch` / `just play`: boots the newest ARCHIVE for the operator to play, on purpose; it verifies nothing, so there is no claim for the gate to protect",
  "scripts/lib/throwaway-relay.js":
    "boots a headless relay for a rig that already ran the gate; it verifies the copy it makes against the caller's expectSha256, which it requires (throwaway-relay.test.js)",
  "scripts/lib/game-launch.js": "spawnGame itself: the one place a rig starts the game (game-launch.test.js runs it)",
  "scripts/verify-runtime.js": "boots nothing itself: it gates, then runs probe-sweep (which gates again, see below)",
};

function scriptsThatSpawnTheGame() {
  const out = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) {
        if (e.name !== "tests" && e.name !== "node_modules") walk(full);
      } else if (e.name.endsWith(".js")) {
        const src = fs.readFileSync(full, "utf8");
        if (/HumanityOS(\.exe)?["'`]|EXE_NAME/.test(src) && /\bspawn(Sync|Process|Game)?\s*\(|launchGame\s*\(/.test(src)) {
          out.push(path.relative(REPO, full).replace(/\\/g, "/"));
        }
      }
    }
  };
  walk(path.join(REPO, "scripts"));
  return out.sort();
}

test("every script that spawns the game is classified (a new rig must decide)", () => {
  const unlisted = scriptsThatSpawnTheGame().filter((f) => !BOOTS_THE_GAME.includes(f) && !NOT_A_RIG[f]);
  assert.deepStrictEqual(unlisted, [], "add it to BOOTS_THE_GAME (and gate it) or to NOT_A_RIG with the reason");
});

// Weak on its own (critic review, 2026-10-03): the first version only checked
// that the TEXT runFreshGate(, requireBootCopy( and HUMANITY_NO_HANDOFF appeared
// in each rig, which a check placed after the spawn, or never run, would also
// pass. So the ordering now lives in ONE function: lib/game-launch.js
// spawnGame checks the copy against the judged bytes immediately before it
// spawns, always sets HUMANITY_NO_HANDOFF, and watches for an exit (and a
// hand-off) the rig did not ask for; game-launch.test.js runs it for real. What
// is left to check per rig is structural: the game is started ONLY through
// spawnGame (a raw spawn of the game exe is a failure; the --headless relay
// verify-live-screen starts is not the game), and the run.log is checked for a
// built-in data copy. The gate's place BEFORE the rig is touched is then run
// for real, for every rig, in the refusal test below.
// Red first, 2026-10-03, against the rigs of c15a4be8b:
//   scripts/probe-sweep.js: no spawnGame(, no builtinDataLines(; starts the game with a raw spawn( ...
//   (the same for all seven)
test("every script that boots the game starts it only through spawnGame, gated, and checks for built-in data", () => {
  const missing = [];
  for (const f of BOOTS_THE_GAME) {
    const src = fs.readFileSync(path.join(REPO, f), "utf8");
    const lacks = [];
    if (!src.includes("runFreshGate(")) lacks.push("no runFreshGate(");
    if (!src.includes("requireBootCopy(")) lacks.push("no requireBootCopy(");
    if (!/\bspawnGame\s*\(/.test(src)) lacks.push("no spawnGame(");
    if (!src.includes("builtinDataLines(")) lacks.push("no builtinDataLines(");
    if (!src.includes("other_build")) lacks.push("no other_build (the manifest record)");
    // Every raw spawn( in code must be a node helper (process.execPath) or the
    // --headless relay; anything else could be the game started around
    // spawnGame (make-clips once started it as `spawn(exe, ...)`).
    for (const m of src.matchAll(/\bspawn\s*\(/g)) {
      const lineStart = src.lastIndexOf("\n", m.index) + 1;
      if (/^\s*\/\//.test(src.slice(lineStart, m.index))) continue; // a comment
      const call = src.slice(m.index, m.index + 200);
      if (/^spawn\s*\(\s*process\.execPath\b/.test(call)) continue;
      if (/--headless/.test(call)) continue;
      lacks.push(`starts something with a raw spawn( at line ${src.slice(0, m.index).split("\n").length} (the game goes through spawnGame)`);
    }
    if (lacks.length) missing.push(`${f}: ${lacks.join(", ")}`);
  }
  assert.deepStrictEqual(missing, []);
});

test("the throwaway relay verifies its copy when the caller says what it judged", () => {
  // (Its behaviour is tested in throwaway-relay.test.js; this pins the callers.)
  const src = fs.readFileSync(path.join(REPO, "scripts", "lib", "throwaway-relay.js"), "utf8");
  assert.ok(src.includes("expectSha256"), "startRelay() takes no expectSha256");
  for (const caller of ["scripts/verify-copresence.js", "scripts/tests/second-player-relay.test.js"]) {
    assert.ok(fs.readFileSync(path.join(REPO, caller), "utf8").includes("expectSha256"), `${caller} does not pass expectSha256`);
  }
});

test("verify-runtime forwards --allow-other-build to the sweep it starts", () => {
  const src = fs.readFileSync(path.join(REPO, "scripts", "verify-runtime.js"), "utf8");
  const i = src.indexOf("const sweepArgs");
  assert.ok(i >= 0);
  assert.ok(
    /sweepArgs\.push\("--allow-other-build"/.test(src.slice(i, i + 2500)),
    "probe-sweep gates too, so an allowed other build must reach it"
  );
});

test("find_newer_exe asks handoff_blocked_now before it looks for a newer build", () => {
  const src = fs.readFileSync(path.join(REPO, "src", "main.rs"), "utf8");
  const i = src.indexOf("fn find_newer_exe");
  assert.ok(i >= 0);
  const body = src.slice(i, src.indexOf("\n}\n", i));
  const block = body.indexOf("handoff_blocked_now()");
  assert.ok(block >= 0, "find_newer_exe never consults release_update::handoff_blocked_now");
  assert.ok(block < body.indexOf("read_dir"), "the check must come before the scan");
});

test("just launch-bg boots with the hand-off switched off", () => {
  const jf = fs.readFileSync(path.join(REPO, "Justfile"), "utf8");
  const i = jf.indexOf("\nlaunch-bg:");
  assert.ok(i >= 0);
  assert.ok(/HUMANITY_NO_HANDOFF=1/.test(jf.slice(i, i + 400)), "launch-bg does not set HUMANITY_NO_HANDOFF=1");
});

// ── probe-sweep refuses before it touches anything ──────────────────────────
test("probe-sweep refuses an unchecked exe before it builds or touches its rig", () => {
  const exe = tmpFile(crypto.randomBytes(4096)); // no source stamp
  const rig = path.join(tmpRoot, "rig-should-not-exist");
  const r = spawnSync(
    process.execPath,
    [path.join(REPO, "scripts", "probe-sweep.js"), "--exe", exe, "--rig", rig, "--only", "home-overview-noon", "--game-only"],
    { encoding: "utf8", timeout: 120000 }
  );
  const out = `${r.stdout}\n${r.stderr}`;
  assert.strictEqual(r.status, 1, out);
  assert.match(out, /NO SOURCE STAMP/);
  assert.match(out, /probe-sweep: REFUSED/);
  assert.ok(!fs.existsSync(rig), "the rig folder was created: the refusal came too late");
});

// ── Every rig refuses an unchecked exe before it copies anything (BUG-133) ──
// Run for real: each rig is started on a file with no source stamp, with the
// machine guard's process list faked empty (so another agent's game running
// right now cannot make a rig refuse for that reason instead). Each must refuse
// on the freshness gate, exit 1, and leave its rig's HumanityOS.exe untouched:
// the gate runs before the copy, in the rig as it is, not as the source reads.
// Red first, 2026-10-03: with a copy into the rig put in front of photograph-home's
// gate, this test failed with (a deepStrictEqual of the problem list)
//   + 'scripts/photograph-home.js copied into its rig before it refused'
const RIG_EXES = {
  "scripts/photograph-home.js": ".probe-rig/home-photos/HumanityOS.exe",
  "scripts/make-clips.js": ".probe-rig/clips/HumanityOS.exe",
  "scripts/verify-copresence.js": ".probe-rig/copresence/HumanityOS.exe",
  "scripts/verify-live-screen.js": ".probe-rig/live-screen/HumanityOS.exe",
  "scripts/verify-screens.js": ".probe-rig/screens/HumanityOS.exe",
};
test("every rig refuses an unchecked exe on the freshness gate, before it copies anything", () => {
  const exe = tmpFile(crypto.randomBytes(4096)); // no source stamp
  const emptyList = path.join(tmpRoot, "no-processes.txt");
  fs.writeFileSync(emptyList, "");
  const statOf = (p) => {
    try {
      const s = fs.statSync(p);
      return `${s.size}:${s.mtimeMs}`;
    } catch {
      return "absent";
    }
  };
  const runs = [
    ...Object.entries(RIG_EXES).map(([f, rigExe]) => ({ f, rigExe: path.join(REPO, rigExe), args: ["--exe", exe] })),
    // boot-timing takes --rig: a folder that must not even be created.
    { f: "scripts/boot-timing.js", rigExe: path.join(tmpRoot, "boot-rig", "HumanityOS.exe"), args: ["--exe", exe, "--rig", path.join(tmpRoot, "boot-rig"), "--runs", "1"] },
  ];
  assert.deepStrictEqual(
    [...runs.map((r) => r.f), "scripts/probe-sweep.js"].sort(),
    [...BOOTS_THE_GAME].sort(),
    "every rig is run here (probe-sweep has its own test below)"
  );
  const problems = [];
  for (const r of runs) {
    const before = statOf(r.rigExe);
    const out = spawnSync(process.execPath, [path.join(REPO, r.f), ...r.args], {
      encoding: "utf8",
      timeout: 120000,
      env: { ...process.env, HUMANITY_MACHINE_GUARD_FAKE: emptyList },
    });
    const text = `${out.stdout}\n${out.stderr}`;
    if (out.status !== 1) problems.push(`${r.f}: exit ${out.status}, not 1\n${text.slice(-600)}`);
    if (!/NO SOURCE STAMP/.test(text)) problems.push(`${r.f}: did not refuse on the freshness gate\n${text.slice(-600)}`);
    if (statOf(r.rigExe) !== before) problems.push(`${r.f} copied into its rig before it refused`);
  }
  assert.deepStrictEqual(problems, []);
  assert.ok(!fs.existsSync(path.join(tmpRoot, "boot-rig")), "boot-timing created its rig before it refused");
});

// ── probe-sweep gates again after it waited for the machine (BUG-133) ──
// It gates, waits (bounded) for builds and other games to leave, then copies.
// What it waited for may be a cargo build of this very exe, and a rebuild
// changes the bytes, so a sweep that judged the old exe used to wait minutes
// and then always refuse with BOOT COPY CHANGED. Here the "build" is a file
// swapped during the wait for another file that also carries this tree's
// stamp; the process list is faked (a cargo.exe, then nothing) and polled
// every 300 ms. The "exe" is not a program, so Windows refuses to start it
// (spawnGame: could not start ..., nothing booted): what is tested is that the
// run got that far, past a second gate and a passing copy check.
// Red first, 2026-10-03, with the re-check block taken out of probe-sweep.js:
//   AssertionError: the swapped exe was judged again after the wait
//   ... BOOT COPY CHANGED: ...\HumanityOS.exe is not the binary the freshness gate judged.
function stampedFake() {
  const tree = L.fingerprintTree(REPO);
  const stamp =
    `HOS-SRC-STAMP v1\nfingerprint ${tree.fingerprint}\nfiles ${tree.files.size}\nprofile release\nfeatures native\n` +
    `${tree.text}HOS-SRC-STAMP END\n`;
  return Buffer.concat([crypto.randomBytes(4096), Buffer.from(stamp, "utf8")]);
}
test("probe-sweep judges the exe again after a machine wait, so a rebuild during the wait is not refused", async () => {
  const { spawn } = require("node:child_process");
  const exe = tmpFile(stampedFake());
  const rig = path.join(tmpRoot, "regate-rig");
  const list = path.join(tmpRoot, "regate-procs.txt");
  fs.writeFileSync(list, "4242|cargo.exe|C:\elsewhere\cargo.exe\n");
  const child = spawn(
    process.execPath,
    [path.join(REPO, "scripts", "probe-sweep.js"), "--exe", exe, "--rig", rig, "--only", "home-overview-noon"],
    { env: { ...process.env, HUMANITY_MACHINE_GUARD_FAKE: list, HUMANITY_MACHINE_GUARD_POLL_MS: "300" } }
  );
  let out = "";
  let swapped = false;
  const onData = (d) => {
    out += d.toString();
    if (!swapped && /pre-boot: WAITING/.test(out)) {
      swapped = true;
      fs.writeFileSync(exe, stampedFake()); // "the build finished": new bytes, this tree's stamp
      fs.writeFileSync(list, ""); // and the machine is free
    }
  };
  child.stdout.on("data", onData);
  child.stderr.on("data", onData);
  const status = await new Promise((resolve) => {
    const t = setTimeout(() => {
      child.kill();
      resolve("timeout");
    }, 120000);
    child.on("exit", (code) => {
      clearTimeout(t);
      resolve(code);
    });
  });
  // The rig holds junctions to the tree's data/ and assets/: remove them as
  // links before anything removes the folder.
  for (const j of ["data", "assets"]) {
    try {
      fs.rmdirSync(path.join(rig, j));
    } catch {}
  }
  assert.ok(swapped, `the sweep never waited for the faked build:\n${out.slice(-1500)}`);
  assert.match(out, /re-checking the binary after the \d+ s machine wait/, "the swapped exe was judged again after the wait");
  assert.doesNotMatch(out, /BOOT COPY CHANGED/, out.slice(-1500));
  assert.match(out, /boot copy .* is byte-identical to the judged exe/);
  assert.match(out, /probe-sweep: could not start .*HumanityOS.exe .*nothing booted/, out.slice(-1500));
  assert.strictEqual(status, 1, `exit ${status}`);
});
