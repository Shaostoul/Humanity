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
    "boots a headless relay for a rig that already ran the gate; it verifies the copy it makes against the caller's expectSha256 (checked below)",
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
        if (/HumanityOS(\.exe)?["'`]|EXE_NAME/.test(src) && /\bspawn(Sync|Process)?\s*\(|launchGame\s*\(/.test(src)) {
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

test("every script that boots the game gates it, checks its boot copy, and keeps it from handing off", () => {
  const missing = [];
  for (const f of BOOTS_THE_GAME) {
    const src = fs.readFileSync(path.join(REPO, f), "utf8");
    const lacks = [];
    if (!src.includes("runFreshGate(")) lacks.push("runFreshGate(");
    if (!src.includes("requireBootCopy(")) lacks.push("requireBootCopy(");
    if (!src.includes("HUMANITY_NO_HANDOFF")) lacks.push("HUMANITY_NO_HANDOFF");
    if (!src.includes("other_build")) lacks.push("other_build (the manifest record)");
    if (lacks.length) missing.push(`${f}: no ${lacks.join(", no ")}`);
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
