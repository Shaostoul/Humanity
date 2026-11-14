// The boot ledger (scripts/lib/boot-ledger.js, 2026-11-14): every game boot a developer's script
// starts appends one line to %LOCALAPPDATA%\HumanityOS\dev\boots.jsonl, written by the launcher
// (spawnGame), and `node scripts/boot-ledger.js` reports per worktree and per rig. Pure node, no
// game: the "game" is node itself. Each test points the ledger at its own file
// (HUMANITY_BOOT_LEDGER_FILE), so nothing here touches the machine's real ledger.
//
// RED FIRST (2026-11-14): with boot-ledger.js present but game-launch.js swapped back to the HEAD
// copy (no ledger write, no HUMANITY_NO_FOCUS), 4 of the 10 tests failed:
//   x spawnGame writes one line per boot, and runs the game as a developer's quiet boot
//     AssertionError: the game runs as a developer's quiet boot
//   x HUMANITY_RIG_VISIBLE is recorded as a visible boot
//   x a rig that stops its game as it exits records 'stopped by the rig'
//     AssertionError: one line: []
//   x a boot kept open waits as pending, and whoever closes it finishes the line
// (and without boot-ledger.js at all: Error: Cannot find module '../lib/boot-ledger.js').

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const REPO = path.resolve(__dirname, "..", "..");
const BL = require("../lib/boot-ledger.js");
const GL = require("../lib/game-launch.js");

const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "hos-boot-ledger-test-"));
process.on("exit", () => fs.rmSync(tmpRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 }));
process.env.HUMANITY_RIG_LOCK = path.join(tmpRoot, "rig-boot.lock");
const NODE = process.execPath;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const judged = (exe) => ({ result: { exe, exe_sha256: GL.fileSha256(exe), exe_fingerprint: null, verdict: "current" } });
let n = 0;
const freshLedger = () => path.join(tmpRoot, `boots-${++n}.jsonl`);
const lines = (file) => (fs.existsSync(file) ? fs.readFileSync(file, "utf8").split(/\r?\n/).filter(Boolean).map((l) => JSON.parse(l)) : []);
async function until(fn, ms = 15000) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    const v = fn();
    if (v) return v;
    await sleep(100);
  }
  return fn();
}

test("the worktree name is the folder under .claude/worktrees, else the checkout's name", () => {
  assert.strictEqual(BL.worktreeName("C:\\Humanity\\.claude\\worktrees\\agent-abc"), "agent-abc");
  assert.strictEqual(BL.worktreeName("C:\\Humanity"), "Humanity");
  assert.strictEqual(BL.worktreeName(null), null);
});

test("the game's half writes the same field names (src/engine/boot_ledger.rs)", () => {
  const rust = fs.readFileSync(path.join(REPO, "src", "engine", "boot_ledger.rs"), "utf8");
  for (const f of BL.FIELDS) assert.ok(rust.includes(`"${f}":`), `boot_ledger.rs writes no "${f}" field`);
  assert.ok(rust.includes(`"${BL.OWNER_ENV}"`) && rust.includes(`"${BL.FILE_ENV}"`), "the two share the variable names");
  const rec = BL.startRecord({ checkout: "C:\\x", rig: "r" });
  assert.deepStrictEqual(Object.keys(rec).sort(), [...BL.FIELDS].sort(), "a record carries every field");
});

test("a finished record has its end, duration and ending", () => {
  const start = Date.parse("2026-11-14T09:00:00Z");
  const rec = BL.finishRecord(BL.startRecord({ startMs: start, rig: "probe-sweep" }), "exited with code 0", start + 61500);
  assert.strictEqual(rec.t, "2026-11-14T09:00:00.000Z");
  assert.strictEqual(rec.end, "2026-11-14T09:01:01.500Z");
  assert.strictEqual(rec.duration_s, 61.5);
  assert.strictEqual(rec.ended, "exited with code 0");
});

test("the report groups per worktree and per rig, and names the repeats", () => {
  const file = freshLedger();
  const env = { ...process.env, [BL.FILE_ENV]: file };
  const now = Date.now();
  const mk = (wt, rig, purpose, sec, hidden = true) =>
    BL.append(BL.finishRecord(BL.startRecord({ startMs: now - 60000, checkout: `C:\\H\\.claude\\worktrees\\${wt}`, rig, purpose, hidden }), "stopped by the rig", now - 60000 + sec * 1000), env);
  mk("agent-a", "probe-sweep", "vantages: v1", 30);
  mk("agent-a", "probe-sweep", "vantages: v1", 40);
  mk("agent-a", "verify-screens", null, 100, false);
  mk("agent-b", "probe-sweep", "vantages: v2", 20);
  // An old line outside the window, and a broken line, are skipped.
  fs.appendFileSync(file, JSON.stringify({ ...BL.startRecord({ startMs: now - 3 * 86400000 }), rig: "old" }) + "\nnot json\n");
  const boots = BL.readBoots(file, now - 86400000);
  assert.strictEqual(boots.length, 4);
  const s = BL.summarize(boots);
  assert.strictEqual(s.boots, 4);
  assert.strictEqual(s.seconds, 190);
  assert.deepStrictEqual(s.worktrees.map((g) => [g.key, g.boots]), [["agent-a", 3], ["agent-b", 1]]);
  assert.deepStrictEqual(s.rigs.map((g) => [g.key, g.boots]), [["probe-sweep", 3], ["verify-screens", 1]]);
  assert.strictEqual(s.rigs[1].visible, 1, "a visible boot is counted as such");
  assert.deepStrictEqual(s.repeats.map((g) => [g.key, g.boots]), [["agent-a | probe-sweep | vantages: v1", 2]]);
  // The CLI reads the same file.
  const r = spawnSync(NODE, [path.join(REPO, "scripts", "boot-ledger.js")], { encoding: "utf8", env });
  assert.strictEqual(r.status, 0, r.stderr);
  assert.match(r.stdout, /Last 24 h: 4 boot\(s\)/);
  assert.match(r.stdout, /agent-a\s+3 boots/);
  assert.match(r.stdout, /Repeated/);
});

test("spawnGame writes one line per boot, and runs the game as a developer's quiet boot", async () => {
  const file = freshLedger();
  const out = path.join(tmpRoot, "game-env.json");
  const env = { ...process.env, [BL.FILE_ENV]: file };
  delete env.HUMANITY_NO_FOCUS;
  const game = `require('fs').writeFileSync(${JSON.stringify(out)}, JSON.stringify({ nf: process.env.HUMANITY_NO_FOCUS || null, owner: process.env.HUMANITY_BOOT_LEDGER || null }))`;
  const w = GL.spawnGame(NODE, ["-e", game], { fresh: judged(NODE), rigName: "test-rig", purpose: "vantages: a,b", cwd: tmpRoot, env, log: () => {} });
  await until(() => w.exited());
  await until(() => lines(file).length);
  const seen = JSON.parse(fs.readFileSync(out, "utf8"));
  assert.strictEqual(seen.nf, "1", "the game runs as a developer's quiet boot");
  assert.strictEqual(seen.owner, "spawnGame", "the game is told its launcher writes the line");
  const ls = lines(file);
  assert.strictEqual(ls.length, 1, "one line per boot");
  const l = ls[0];
  assert.strictEqual(l.writer, "spawnGame");
  assert.strictEqual(l.rig, "test-rig");
  assert.strictEqual(l.purpose, "vantages: a,b");
  assert.strictEqual(l.ended, "exited with code 0");
  assert.strictEqual(l.hidden, true);
  assert.strictEqual(l.pid, w.pid);
  assert.strictEqual(l.checkout, REPO);
  assert.ok(typeof l.duration_s === "number" && l.duration_s >= 0);
});

test("HUMANITY_RIG_VISIBLE is recorded as a visible boot", async () => {
  const file = freshLedger();
  const env = { ...process.env, [BL.FILE_ENV]: file, [BL.RIG_VISIBLE_ENV]: "1" };
  const w = GL.spawnGame(NODE, ["-e", "0"], { fresh: judged(NODE), rigName: "visible-rig", cwd: tmpRoot, env, log: () => {} });
  await until(() => w.exited());
  await until(() => lines(file).length);
  assert.strictEqual(lines(file)[0].hidden, false);
});

// A rig that stops its own game as it exits (probe-sweep's shape), and one that keeps it open.
function rigScript(file, dir, { keepOpen }) {
  return `
    const { execSync } = require("child_process");
    const GL = require(${JSON.stringify(path.join(REPO, "scripts", "lib", "game-launch.js"))});
    const exe = process.execPath;
    const fresh = { result: { exe, exe_sha256: GL.fileSha256(exe), exe_fingerprint: null, verdict: "current" } };
    process.env.HUMANITY_BOOT_LEDGER_FILE = ${JSON.stringify(file)};
    // Its own boot lock: this test process holds its lock for as long as it runs (spawnGame above).
    process.env.HUMANITY_RIG_LOCK = ${JSON.stringify(path.join(dir, "rig-boot.lock"))};
    const w = GL.spawnGame(exe, ["-e", "setTimeout(() => {}, 60000)"], { fresh, rigName: "exit-rig", cwd: ${JSON.stringify(dir)}, detached: true, keepOpen: ${keepOpen}, log: () => {} });
    w.child.unref();
    require("fs").writeFileSync(${JSON.stringify(path.join(dir, "game.pid"))}, String(w.pid));
    // Registered straight after the spawn, as probe-sweep does: the rig stops its game on the way out.
    process.on("exit", () => { if (!${keepOpen}) { w.expectExit(); try { execSync("taskkill /PID " + w.pid + " /T /F", { stdio: "ignore" }); } catch {} } });
    setTimeout(() => process.exit(0), 1500);
  `;
}

test("a rig that stops its game as it exits records 'stopped by the rig'", () => {
  const file = freshLedger();
  const dir = fs.mkdtempSync(path.join(tmpRoot, "rig-"));
  const r = spawnSync(NODE, ["-e", rigScript(file, dir, { keepOpen: false })], { encoding: "utf8", timeout: 30000, env: { ...process.env, NODE_TEST_CONTEXT: "" } });
  assert.strictEqual(r.status, 0, r.stderr);
  const ls = lines(file);
  assert.strictEqual(ls.length, 1, `one line: ${JSON.stringify(ls)}`);
  assert.match(ls[0].ended, /^stopped by the rig/);
  assert.strictEqual(ls[0].rig, "exit-rig");
});

test("a boot kept open waits as pending, and whoever closes it finishes the line", () => {
  const file = freshLedger();
  const dir = fs.mkdtempSync(path.join(tmpRoot, "rig-"));
  const r = spawnSync(NODE, ["-e", rigScript(file, dir, { keepOpen: true })], { encoding: "utf8", timeout: 30000 });
  assert.strictEqual(r.status, 0, r.stderr);
  const pid = Number(fs.readFileSync(path.join(dir, "game.pid"), "utf8"));
  try {
    assert.strictEqual(lines(file).length, 0, "nothing written while the game is still up");
    const pending = path.join(dir, "debug", BL.PENDING_FILE);
    assert.ok(fs.existsSync(pending), "the boot is pending in the rig folder's debug/");
    // Another pid's close does not finish it; this one's does.
    assert.strictEqual(BL.finishPending(path.join(dir, "debug"), "closed", { pid: pid + 1, env: { [BL.FILE_ENV]: file } }), null);
    const done = BL.finishPending(path.join(dir, "debug"), "closed by the dev MCP (game_close): closed", { pid, env: { [BL.FILE_ENV]: file } });
    assert.ok(done && done.ended.startsWith("closed by the dev MCP"));
    assert.strictEqual(lines(file).length, 1);
    assert.ok(!fs.existsSync(pending));
  } finally {
    try {
      require("node:child_process").execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });
    } catch {}
  }
});

test("a pending boot nobody finished is written at the next boot in that folder", () => {
  const file = freshLedger();
  const dir = path.join(tmpRoot, "stale", "debug");
  BL.writePending(dir, BL.startRecord({ rig: "probe-sweep", pid: 4242 }));
  const done = BL.flushStalePending(dir, { [BL.FILE_ENV]: file });
  assert.match(done.ended, /end was not seen/);
  assert.strictEqual(lines(file).length, 1);
  assert.strictEqual(BL.flushStalePending(dir, { [BL.FILE_ENV]: file }), null, "only once");
});

// Seen 2026-11-14: dxc-dlls.test.js's stand-in game (node copied as HumanityOS.exe, started from a
// child rig whose env has no NODE_TEST_CONTEXT, as a real rig's) wrote two "the-test-rig" boots
// into the machine's real ledger; that test now names its own ledger file. This test covers the
// other stand-in shape, node under its own name. RED FIRST: with
// spawnGame's `recordable` guard taken out, this test failed with
//   AssertionError: a stand-in's run wrote a ledger line
test("a stand-in game (not named like the game) is not a boot", () => {
  const local = fs.mkdtempSync(path.join(tmpRoot, "local-"));
  const dir = fs.mkdtempSync(path.join(tmpRoot, "rig-"));
  const script = `
    const GL = require(${JSON.stringify(path.join(REPO, "scripts", "lib", "game-launch.js"))});
    const exe = process.execPath;
    const fresh = { result: { exe, exe_sha256: GL.fileSha256(exe), exe_fingerprint: null, verdict: "current" } };
    const w = GL.spawnGame(exe, ["-e", "0"], { fresh, rigName: "stand-in", cwd: ${JSON.stringify(dir)}, log: () => {} });
    w.child.on("exit", () => setTimeout(() => process.exit(0), 200));
  `;
  const env = { ...process.env, LOCALAPPDATA: local, HUMANITY_RIG_LOCK: path.join(dir, "rig-boot.lock") };
  delete env.NODE_TEST_CONTEXT;
  delete env[BL.FILE_ENV];
  const r = spawnSync(NODE, ["-e", script], { encoding: "utf8", timeout: 30000, env });
  assert.strictEqual(r.status, 0, r.stderr);
  assert.ok(!fs.existsSync(BL.ledgerPath({ LOCALAPPDATA: local })), "a stand-in's run wrote a ledger line");
});

test("inside node --test nothing reaches the machine's ledger unless a file is named", () => {
  const saved = process.env[BL.FILE_ENV];
  delete process.env[BL.FILE_ENV];
  try {
    assert.strictEqual(BL.append({ t: "x" }, { NODE_TEST_CONTEXT: "child-v8" }), false);
  } finally {
    if (saved !== undefined) process.env[BL.FILE_ENV] = saved;
  }
});
