// spawnGame and the built-in-data check (scripts/lib/game-launch.js), BUG-133's
// critic review of 2026-10-03. Pure node, no game, no GPU: runs in `just
// rig-tests`. The "game" here is node itself, so these run in about two seconds.
//
// RED FIRST (2026-10-03): this file against the tree before game-launch.js
// existed failed every test with
//   Error: Cannot find module '../lib/game-launch.js'
// and, with game-launch.js present but the exit handler's reapHandoff call
// deleted, the hand-off test failed with
//   AssertionError: the hand-off was stopped: expected 1 stopped process, got 0
// With the requireBootCopy line deleted from spawnGame, the refusal test failed:
//   AssertionError: a copy that is not the judged exe must be refused (exit 1); status 0

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync, execSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const GL = require("../lib/game-launch.js");

const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-game-launch-test-"));
process.on("exit", () => fs.rmSync(tmpRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 }));
// spawnGame takes the machine-wide rig boot lock (lib/boot-lock.js): this file's lock is its own,
// so it never waits on, or holds up, a real rig running on this machine meanwhile.
process.env.HUMANITY_RIG_LOCK = path.join(tmpRoot, "rig-boot.lock");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const NODE = process.execPath;
const judged = (exe) => ({ result: { exe, exe_sha256: GL.fileSha256(exe), exe_fingerprint: null, verdict: "current" } });
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
    await sleep(100);
  }
  return fn();
}

test("spawnGame refuses a copy that is not the bytes the gate judged, and starts nothing", () => {
  const marker = path.join(tmpRoot, "started.txt");
  // A child node that calls spawnGame with a record of different bytes, and
  // (should spawnGame start the "game" anyway) waits for it, so the marker it
  // writes is there to be seen rather than lost when this node exits.
  const script = `
    const GL = require(${JSON.stringify(path.join(REPO, "scripts", "lib", "game-launch.js"))});
    const w = GL.spawnGame(process.execPath, ["-e", "require('fs').writeFileSync(process.argv[1], 'x')", ${JSON.stringify(marker)}], {
      fresh: { result: { exe: process.execPath, exe_sha256: "0".repeat(64), exe_fingerprint: null } },
      rigName: "the-test-rig",
      log: () => {},
    });
    w.child.on("exit", () => process.exit(0));
  `;
  const r = spawnSync(NODE, ["-e", script], { encoding: "utf8", timeout: 30000 });
  const out = `${r.stdout}\n${r.stderr}`;
  assert.strictEqual(r.status, 1, `a copy that is not the judged exe must be refused (exit 1); status ${r.status}\n${out}`);
  assert.match(out, /BOOT COPY CHANGED/);
  assert.match(out, /the-test-rig: REFUSED - nothing was booted/);
  assert.ok(!fs.existsSync(marker), "nothing was started");
});

test("spawnGame always runs the game with HUMANITY_NO_HANDOFF=1", async () => {
  const out = path.join(tmpRoot, "env.json");
  const env = { ...process.env };
  delete env.HUMANITY_NO_HANDOFF;
  const w = GL.spawnGame(NODE, ["-e", `require('fs').writeFileSync(${JSON.stringify(out)}, JSON.stringify(process.env.HUMANITY_NO_HANDOFF || null))`], {
    fresh: judged(NODE),
    rigName: "test",
    env,
    log: () => {},
  });
  await until(() => w.exited());
  assert.strictEqual(JSON.parse(fs.readFileSync(out, "utf8")), "1");
  assert.strictEqual(w.code, 0);
});

test("a game that hands itself off and exits is reported, and the hand-off is stopped", async () => {
  // The stand-in for a newer archive in C:\Humanity: node under the name a
  // hand-off target has. Not "HumanityOS.exe", so no other rig's one-GPU guard
  // ever sees it.
  const handoffExe = path.join(tmpRoot, "v9.9.9_HumanityOS.exe");
  fs.copyFileSync(NODE, handoffExe);
  const pidFile = path.join(tmpRoot, "handoff.pid");
  // The "old build": starts the hand-off target detached, exactly as
  // launch_and_exit does, and exits 0.
  const oldBuild = `
    const { spawn } = require("child_process");
    const c = spawn(${JSON.stringify(handoffExe)}, ["-e", "setTimeout(() => {}, 60000)"], { detached: true, stdio: "ignore" });
    require("fs").writeFileSync(${JSON.stringify(pidFile)}, String(c.pid));
    c.unref();
    process.exit(0);
  `;
  const logs = [];
  const w = GL.spawnGame(NODE, ["-e", oldBuild], { fresh: judged(NODE), rigName: "test", log: (m) => logs.push(m) });
  await until(() => w.exited());
  const handoffPid = Number(fs.readFileSync(pidFile, "utf8"));
  try {
    assert.strictEqual(w.code, 0);
    assert.strictEqual(w.query_failed, false, "the process list was read");
    assert.strictEqual(w.handoff.length, 1, `the hand-off was stopped: expected 1 stopped process, got ${w.handoff.length}`);
    assert.strictEqual(w.handoff[0].pid, handoffPid);
    assert.match(w.describe(), /handed itself off to .*v9\.9\.9_HumanityOS\.exe/);
    assert.ok(logs.some((l) => /HAND-OFF stopped/.test(l)), logs.join("\n"));
    assert.ok(await until(() => !alive(handoffPid), 5000), "the handed-off process is gone");
  } finally {
    try {
      execSync(`taskkill /PID ${handoffPid} /F`, { stdio: "ignore" });
    } catch {}
  }
});

test("an exit the rig asked for is not treated as a hand-off", async () => {
  const w = GL.spawnGame(NODE, ["-e", "setTimeout(() => {}, 60000)"], { fresh: judged(NODE), rigName: "test", log: () => {} });
  w.expectExit();
  execSync(`taskkill /PID ${w.pid} /F`, { stdio: "ignore" });
  await until(() => w.exited());
  assert.deepStrictEqual(w.handoff, []);
});

test("builtinDataLines finds the marker the game writes, and the marker matches the Rust const", () => {
  const rust = fs.readFileSync(path.join(REPO, "src", "embedded_data.rs"), "utf8");
  const m = rust.match(/pub const BUILTIN_COPY_MARKER: &str = "([^"]+)";/);
  assert.ok(m, "src/embedded_data.rs defines BUILTIN_COPY_MARKER");
  assert.strictEqual(GL.BUILTIN_COPY_MARKER, m[1], "the rigs look for exactly what the game writes");
  assert.ok(/log::warn!\(\s*"\{BUILTIN_COPY_MARKER\}/.test(rust), "note_builtin_copy writes the marker at the start of its line");
  const log = [
    "2026-10-03T22:00:00.000Z INFO  humanity_engine] Loaded items.csv",
    "2026-10-03T22:00:01.000Z WARN  humanity_engine::embedded_data] [built-in data copy] data/ground/materials.ron: it does not parse (x); this run uses the copy compiled into the exe, not the file on disk",
  ].join("\r\n");
  const lines = GL.builtinDataLines(log);
  assert.strictEqual(lines.length, 1);
  assert.match(GL.builtinDataRefusal(lines), /BUILT-IN DATA: .*1 data file/);
  assert.match(GL.builtinDataRefusal(lines), /data\/ground\/materials\.ron: it does not parse/);
  assert.strictEqual(GL.builtinDataRefusal([]), null);
});

// LOW IMPACT (point 7, 2026-10-22, lib/rig-needs.js). The rig, and only the rig, asks for it; the
// game must say it runs light, or it is stopped. Seen red first against spawnGame without point 7:
// "an inherited HUMANITY_LOW_IMPACT reached the game" and "the game was never asked".
const fakeGame = (dir, { confirm }) => [
  "-e",
  `const fs=require('fs');const v=process.env.HUMANITY_LOW_IMPACT||null;` +
    `fs.writeFileSync(${JSON.stringify(path.join(dir, "env.json"))},JSON.stringify(v));` +
    (confirm
      ? `if(v){fs.mkdirSync('debug',{recursive:true});fs.writeFileSync('debug/low_impact.json',JSON.stringify({on:true,value:v,fps:30,window:'keep',settings:{a:1},preset:'minimum',preset_found:true}));}`
      : "") +
    `setTimeout(()=>{},60000);`,
];

test("an inherited HUMANITY_LOW_IMPACT never reaches the game: only the rig asks", async () => {
  const dir = fs.mkdtempSync(path.join(tmpRoot, "li-inherit-"));
  const w = GL.spawnGame(NODE, fakeGame(dir, { confirm: false }), { fresh: judged(NODE), rigName: "test", cwd: dir, env: { ...process.env, HUMANITY_LOW_IMPACT: "1" }, log: () => {} });
  await until(() => fs.existsSync(path.join(dir, "env.json")));
  w.expectExit();
  w.child.kill();
  assert.strictEqual(JSON.parse(fs.readFileSync(path.join(dir, "env.json"), "utf8")), null, "an inherited HUMANITY_LOW_IMPACT reached the game");
  assert.ok(!w.low_impact, "a rig that did not ask is recorded as low impact");
});

test("a rig that asks for low impact gets it, and the game's confirmation is read", async () => {
  const dir = fs.mkdtempSync(path.join(tmpRoot, "li-ask-"));
  const logs = [];
  const w = GL.spawnGame(NODE, fakeGame(dir, { confirm: true }), { fresh: judged(NODE), rigName: "test", cwd: dir, lowImpact: "fps=30,window=keep", log: (m) => logs.push(m) });
  await until(() => fs.existsSync(path.join(dir, "env.json")));
  await until(() => w.low_impact && w.low_impact.confirmed);
  w.expectExit();
  w.child.kill();
  assert.strictEqual(JSON.parse(fs.readFileSync(path.join(dir, "env.json"), "utf8")), "fps=30,window=keep", "the game was never asked");
  assert.ok(w.low_impact && w.low_impact.confirmed, "the game's confirmation was never read");
  assert.strictEqual(w.low_impact.confirmed.fps, 30);
  assert.ok(logs.some((l) => /low impact confirmed by the game: 30 fps, window keep/.test(l)), logs.join("\n"));
});

test("a game asked for low impact that never confirms is stopped, and the rig refuses", () => {
  const dir = fs.mkdtempSync(path.join(tmpRoot, "li-silent-"));
  // A stale confirmation from an earlier run must not count: spawnGame deletes it first.
  fs.mkdirSync(path.join(dir, "debug"), { recursive: true });
  fs.writeFileSync(path.join(dir, "debug", "low_impact.json"), JSON.stringify({ on: true, value: "1", fps: 15, window: [960, 540] }));
  const pidFile = path.join(dir, "pid.txt");
  const script = `
    process.env.HUMANITY_RIG_LOCK = ${JSON.stringify(path.join(dir, "lock"))};
    const GL = require(${JSON.stringify(path.join(REPO, "scripts", "lib", "game-launch.js"))});
    const w = GL.spawnGame(process.execPath, ${JSON.stringify(fakeGame(dir, { confirm: false }))}, {
      fresh: { result: { exe: process.execPath, exe_sha256: GL.fileSha256(process.execPath), exe_fingerprint: null } },
      rigName: "the-test-rig", cwd: ${JSON.stringify(dir)}, lowImpact: "1", lowImpactConfirmMs: 2500, log: () => {},
    });
    require("fs").writeFileSync(${JSON.stringify(pidFile)}, String(w.pid));
    setTimeout(() => process.exit(0), 20000);
  `;
  const r = spawnSync(NODE, ["-e", script], { encoding: "utf8", timeout: 40000 });
  const out = `${r.stdout}\n${r.stderr}`;
  assert.strictEqual(r.status, 1, `a game that never confirmed low impact was left running (status ${r.status})\n${out}`);
  assert.match(out, /the-test-rig: REFUSED - the game was asked to run in low impact/);
  const pid = Number(fs.readFileSync(pidFile, "utf8"));
  assert.ok(!alive(pid), "the silent game was not stopped");
});
