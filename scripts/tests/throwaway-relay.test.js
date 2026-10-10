// The throwaway relay's loopback-only guarantee (scripts/lib/throwaway-relay.js),
// 2026-10-03.
//
// Why it matters: on Windows, a program that LISTENS on 0.0.0.0 makes Windows
// Defender Firewall stop whoever is at the keyboard with "Windows Defender
// Firewall has blocked some features of this app", once for every exe path.
// The throwaway relay runs from a new temp folder every time, so before this
// the operator got a prompt for every rig run (54 of his firewall rules were
// for these copies). The relay now listens on 127.0.0.1 (relayEnv sets its
// BIND_ADDRESS), and startRelay asks the OS what the relay's PID listens on
// and refuses anything else. These tests pin both halves: the setting, and a
// check that can actually fail.
//
// Pure node. One live check lists THIS process's own listener (a node server
// on 127.0.0.1, which never prompts) through the real netstat / ss / lsof, so
// the parser is proven on real output too, not only on the samples below. The
// last two run startRelay() on a stand-in relay in node, also on 127.0.0.1,
// so they need no release build. No real relay is started. Run:
// node --test scripts/tests/throwaway-relay.test.js (in `just rig-tests`).

const test = require("node:test");
const assert = require("node:assert");
const net = require("node:net");

const TR = require("../lib/throwaway-relay.js");

// Red first, 2026-10-03, against the lib as it was (no BIND_ADDRESS line, so
// the shell's value went straight through):
//   AssertionError: the throwaway relay must be told to listen on loopback only
//   actual: '0.0.0.0', expected: '127.0.0.1'
// (and the three tests below failed there with "TR.parseListening is not a
// function" and the like: the check did not exist).
test("relayEnv tells the relay to listen on loopback only, whatever the shell says", () => {
  const before = process.env.BIND_ADDRESS;
  process.env.BIND_ADDRESS = "0.0.0.0"; // a shell that would open it up
  try {
    const env = TR.relayEnv(43210, "x/relay.db");
    assert.strictEqual(env.BIND_ADDRESS, "127.0.0.1", "the throwaway relay must be told to listen on loopback only");
    assert.strictEqual(TR.LOOPBACK_BIND, "127.0.0.1");
    assert.strictEqual(env.PORT, "43210");
  } finally {
    if (before === undefined) delete process.env.BIND_ADDRESS;
    else process.env.BIND_ADDRESS = before;
  }
});

// The relay's call forwarder (src/relay/call_forwarder.rs, 2026-10-09) opens a
// UDP port of its own. A throwaway relay keeps it on loopback (TURN_BIND) and on
// a port the system picks (TURN_PORT 0), so two rigs, or a rig beside the
// operator's own node, never fight over 3478 and Windows never asks about it.
// Red first, 2026-10-09, against relayEnv without the two lines: "actual:
// '0.0.0.0', expected: '127.0.0.1'" (the shell's TURN_BIND went straight through).
test("relayEnv keeps the call forwarder on loopback and on a free port, whatever the shell says", () => {
  const before = { bind: process.env.TURN_BIND, port: process.env.TURN_PORT };
  process.env.TURN_BIND = "0.0.0.0";
  process.env.TURN_PORT = "3478";
  try {
    const env = TR.relayEnv(43210, "x/relay.db", { TURN_BIND: "0.0.0.0" });
    assert.strictEqual(env.TURN_BIND, "127.0.0.1", "the forwarder must listen on loopback only");
    assert.strictEqual(env.TURN_PORT, "0", "on a port the system picks");
  } finally {
    for (const [k, v] of [["TURN_BIND", before.bind], ["TURN_PORT", before.port]]) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
  }
});

// Captured formats. The PID under test is 4242; 1252 and 9999 are other
// programs whose rows must be ignored.
const WIN = `
Active Connections

  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1252
  TCP    127.0.0.1:52011        0.0.0.0:0              LISTENING       4242
  TCP    127.0.0.1:52011        127.0.0.1:52020        ESTABLISHED     4242
  TCP    127.0.0.1:52020        127.0.0.1:52011        ESTABLISHED     9999
  TCP    [::]:135               [::]:0                 LISTENING       1252
  TCP    [::1]:52011            [::]:0                 ABHÖREN         4242
  UDP    0.0.0.0:5353           *:*                                    4242
`;

const LINUX = `LISTEN 0      1024       127.0.0.1:52011      0.0.0.0:*    users:(("HumanityOS",pid=4242,fd=9))
LISTEN 0      4096         0.0.0.0:22         0.0.0.0:*    users:(("sshd",pid=1252,fd=3))
LISTEN 0      1024           [::1]:52011         [::]:*    users:(("HumanityOS",pid=4242,fd=10))
LISTEN 0      511                *:80               *:*    users:(("nginx",pid=9999,fd=6))
`;

const MAC = `p4242
f9
n127.0.0.1:52011
f10
n[::1]:52011
`;

// Red first, 2026-10-03, with parseListening made to ignore the PID column:
//   only PID 4242's LISTENING sockets (netstat)
//   expected: '127.0.0.1:52011, [::1]:52011'
//   actual: '0.0.0.0:135, 127.0.0.1:52011, [::]:135, [::1]:52011'
// (other programs' rows counted as the relay's).
test("parseListening keeps only the process's listening sockets, in every OS tool's format", () => {
  const show = (rows) => rows.map((r) => (r.host.includes(":") ? `[${r.host}]` : r.host) + `:${r.port}`).join(", ");
  const want = "127.0.0.1:52011, [::1]:52011";
  assert.strictEqual(show(TR.parseListening(WIN, "win32", 4242)), want, "only PID 4242's LISTENING sockets (netstat)");
  assert.strictEqual(show(TR.parseListening(LINUX, "linux", 4242)), want, "only PID 4242's LISTEN sockets (ss)");
  assert.strictEqual(show(TR.parseListening(MAC, "darwin", 4242)), want, "lsof rows");
  assert.strictEqual(show(TR.parseListening(WIN, "win32", 1252)), "0.0.0.0:135, [::]:135");
  assert.strictEqual(show(TR.parseListening(LINUX, "linux", 9999)), "*:80");
});

// Red first, 2026-10-03, with isLoopbackHost made to say yes to everything
// (the check that cannot fail):
//   "0.0.0.0 is every interface (or a network one): expected a problem, got null"
test("loopbackOnlyProblem refuses every wildcard spelling, a wrong port, and finding nothing", () => {
  const row = (host, port = 52011) => ({ host, port, line: `${host}:${port}` });
  assert.strictEqual(TR.loopbackOnlyProblem([row("127.0.0.1"), row("::1")], 52011), null, "loopback only is fine");
  for (const wild of ["0.0.0.0", "::", "*", "192.168.1.42", "::ffff:192.168.1.42"]) {
    const p = TR.loopbackOnlyProblem([row("127.0.0.1"), row(wild)], 52011);
    assert.ok(p, `${wild} is every interface (or a network one): expected a problem, got ${p}`);
    assert.match(p, /not on loopback only/);
  }
  assert.match(TR.loopbackOnlyProblem([], 52011), /no listening socket/, "nothing found verifies nothing");
  assert.match(TR.loopbackOnlyProblem([row("127.0.0.1", 1)], 52011), /not listening on its port 52011/);
  assert.ok(TR.isLoopbackHost("127.0.0.1") && TR.isLoopbackHost("127.4.5.6") && TR.isLoopbackHost("::1"));
  assert.ok(TR.isLoopbackHost("::ffff:127.0.0.1"));
  assert.ok(!TR.isLoopbackHost("0.0.0.0") && !TR.isLoopbackHost("::") && !TR.isLoopbackHost("*"));
});

// The live leg: the real tool, on this machine, sees this process listening on
// loopback and passes it. Red first, 2026-10-03, with parseListening made to
// skip TCP rows: "the dev relay (pid 21488) is not a loopback-only listener:
// the operating system shows no listening socket for it at all, so nothing
// was verified". (With the PID ignored it failed too: this machine has
// programs listening on 0.0.0.0, and they were counted as this process.)
test("assertLoopbackOnly passes a real loopback listener, read from the OS", async () => {
  const srv = net.createServer();
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  const { port } = srv.address();
  try {
    const rows = TR.assertLoopbackOnly(process.pid, port);
    assert.ok(rows.some((r) => r.port === port && r.host === "127.0.0.1"), `this process's 127.0.0.1:${port} was listed`);
  } finally {
    await new Promise((r) => srv.close(r));
  }
});

// ── startRelay() itself runs the check (critic review, 2026-10-03) ──
//
// The tests above prove the check works; these prove startRelay() CALLS it,
// and stops a relay it refuses. Without them, deleting the one line in
// startRelay() that runs the check left every test here green.
//
// They need no release build: startRelay() takes the program to start as an
// option, and these pass one that starts a tiny stand-in relay in node, which
// answers /health and listens where the relay would, on the BIND_ADDRESS that
// relayEnv() hands it (127.0.0.1, so it never raises a firewall prompt).
// startRelay() still makes its temp folder and copies "the exe" into it (this
// test file stands in for it), so the folder's removal can be checked.

const fs = require("node:fs");
const { spawn } = require("node:child_process");
// startRelay needs the judged exe's hash (BUG-133); "the exe" here is this file.
const THIS_SHA = require("node:crypto").createHash("sha256").update(fs.readFileSync(__filename)).digest("hex");

// Its /health also says which admin keys reached it (ADMIN_KEYS, as the real relay reads them at
// startup, src/relay/mod.rs), so a test can see what the relay process itself was given.
const STAND_IN_RELAY = `
const http = require("node:http");
http
  .createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify({ status: "ok", admin_keys: process.env.ADMIN_KEYS ?? null }));
  })
  .listen(Number(process.env.PORT), process.env.BIND_ADDRESS);
`;

/** A spawnProcess for startRelay(): runs the stand-in relay with the options
 *  (folder, environment, log) startRelay() chose, and remembers the child. */
function standIn(seen) {
  return (_exe, _args, options) => {
    seen.options = options;
    seen.child = spawn(process.execPath, ["-e", STAND_IN_RELAY], options);
    return seen.child;
  };
}

const exitedOrGone = (child) =>
  new Promise((resolve) => {
    if (child.exitCode !== null || child.signalCode !== null) return resolve(true);
    const t = setTimeout(() => resolve(false), 5000);
    child.once("exit", () => {
      clearTimeout(t);
      resolve(true);
    });
  });

// Red first, 2026-10-03, with the `h.listening = checkListening(...)` line
// deleted from startRelay():
//   AssertionError: startRelay() must ask the OS what the relay listens on
//   and keep what it saw; listening was []
test("startRelay asks the OS what the relay listens on, and keeps the evidence", async () => {
  const seen = {};
  const h = await TR.startRelay({
    sourceExe: __filename,
    expectSha256: THIS_SHA,
    prefix: "throwaway-relay-selftest-",
    healthTimeoutMs: 20000,
    spawnProcess: standIn(seen),
  });
  try {
    assert.ok(h.health, `the stand-in relay answered /health (log: ${h.logText()})`);
    assert.ok(
      h.listening.some((r) => r.port === h.port && r.host === "127.0.0.1"),
      `startRelay() must ask the OS what the relay listens on and keep what it saw; listening was ${JSON.stringify(h.listening)}`,
    );
    assert.strictEqual(seen.options.env.BIND_ADDRESS, "127.0.0.1", "it was started with the loopback setting");
  } finally {
    await h.stop();
  }
  assert.ok(!fs.existsSync(seen.options.cwd), "stop() removed its folder");
});

// Red first, 2026-10-03, with the same line deleted:
//   AssertionError [ERR_ASSERTION]: startRelay() must refuse a relay its
//   check refuses (it resolved instead)
// And with the `await h.stop()` before the rethrow deleted:
//   AssertionError [ERR_ASSERTION]: a refused relay is stopped before the
//   error goes up
// (Both red runs finish in about a second, with no stand-in left running.)
test("a relay the check refuses is stopped, its folder removed, and the error goes up", async () => {
  const seen = {};
  const asked = [];
  let h = null;
  let err = null;
  try {
    h = await TR.startRelay({
      sourceExe: __filename,
      expectSha256: THIS_SHA,
      prefix: "throwaway-relay-selftest-",
      healthTimeoutMs: 20000,
      spawnProcess: standIn(seen),
      checkListening: (pid, port) => {
        asked.push([pid, port]);
        throw new Error("refused by this test");
      },
    });
  } catch (e) {
    err = e;
  }
  // Note what startRelay() left behind BEFORE tidying up, then tidy up
  // whatever the outcome. A regression must FAIL here, not leave the
  // stand-in running: a live child keeps this test file from ever exiting
  // (the first red runs of this test hung until their timeout, 2026-10-03).
  const exited = (c) => c.exitCode !== null || c.signalCode !== null;
  const stoppedFirst = !!seen.child && exited(seen.child);
  const dirGoneFirst = !!seen.options && !fs.existsSync(seen.options.cwd);
  if (h) await h.stop();
  if (seen.child && !exited(seen.child)) {
    seen.child.kill();
    await exitedOrGone(seen.child);
  }
  if (seen.options) fs.rmSync(seen.options.cwd, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });

  assert.ok(err, "startRelay() must refuse a relay its check refuses (it resolved instead)");
  assert.match(err.message, /refused by this test/);
  assert.strictEqual(asked.length, 1, "the check ran once");
  assert.strictEqual(asked[0][0], seen.child.pid, "on the relay's own PID");
  assert.strictEqual(String(asked[0][1]), seen.options.env.PORT, "and its port");
  assert.ok(stoppedFirst, "a refused relay is stopped before the error goes up");
  assert.ok(dirGoneFirst, "and its folder is removed");
});

// ── The copy is the judged exe (BUG-133) ──
// A rig gates the exe, then startRelay() copies it: a build finishing in
// between would run a relay nobody checked. With expectSha256 (the hash the
// gate recorded) the copy must be those bytes.
// Red first, 2026-10-03, against the startRelay() before expectSha256:
//   AssertionError [ERR_ASSERTION]: a copy that is not the judged exe must be
//   refused (it resolved instead)
test("a relay copy that is not the exe the gate judged is refused before anything starts", async () => {
  const seen = {};
  let h = null;
  let err = null;
  try {
    h = await TR.startRelay({
      sourceExe: __filename,
      prefix: "throwaway-relay-selftest-",
      healthTimeoutMs: 20000,
      expectSha256: "0".repeat(64),
      spawnProcess: standIn(seen),
    });
  } catch (e) {
    err = e;
  }
  if (h) await h.stop();
  if (seen.child) {
    seen.child.kill();
    await exitedOrGone(seen.child);
  }
  assert.ok(err, "a copy that is not the judged exe must be refused (it resolved instead)");
  assert.match(err.message, /not the exe the freshness gate judged/);
  assert.ok(!seen.child, "nothing was started");

  // The judged bytes pass.
  const crypto = require("node:crypto");
  const ok = await TR.startRelay({
    sourceExe: __filename,
    prefix: "throwaway-relay-selftest-",
    healthTimeoutMs: 20000,
    expectSha256: crypto.createHash("sha256").update(fs.readFileSync(__filename)).digest("hex"),
    spawnProcess: standIn({}),
  });
  try {
    assert.ok(ok.health, "the judged copy starts");
  } finally {
    await ok.stop();
  }
});

// ── expectSha256 is required (critic review, 2026-10-03) ──
// Red first, 2026-10-03, against the startRelay() of c15a4be8b, where a null
// expectSha256 skipped the copy check without a word:
//   AssertionError [ERR_ASSERTION]: a relay with no judged hash must be refused
//   (it resolved instead)
test("startRelay refuses to start without the hash of the judged exe", async () => {
  for (const missing of [undefined, null, ""]) {
    const seen = {};
    let h = null;
    let err = null;
    try {
      h = await TR.startRelay({
        sourceExe: __filename,
        expectSha256: missing,
        prefix: "throwaway-relay-selftest-",
        healthTimeoutMs: 20000,
        spawnProcess: standIn(seen),
      });
    } catch (e) {
      err = e;
    }
    if (h) await h.stop();
    if (seen.child) {
      seen.child.kill();
      await exitedOrGone(seen.child);
    }
    assert.ok(err, `a relay with no judged hash must be refused (it resolved instead), expectSha256=${JSON.stringify(missing)}`);
    assert.match(err.message, /needs expectSha256/);
    assert.ok(!seen.child, "nothing was started");
  }
});

// ── The relay reads THIS tree's data/ (critic review, 2026-10-03) ──
// The relay reads data/ relative to its working folder. A folder holding only
// server-config.json made every read miss, so the relay ran the copies built
// into the exe (crew.ron, room_equipment.ron, proposal_types.ron) or nothing
// (chores.ron, market/categories.json), while the gate called it current.
// Red first, 2026-10-03, against the startRelay() of c15a4be8b:
//   Error: ENOENT: no such file or directory, open
//   '...\Temp\throwaway-relay-selftest-TDz8nF\data\npc\crew.ron'
test("the relay's folder holds this tree's data/, and the tree's data/ is never written", async () => {
  const path = require("node:path");
  const os = require("node:os");
  const src = fs.mkdtempSync(path.join(os.tmpdir(), "throwaway-relay-data-src-"));
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), "throwaway-relay-outside-"));
  try {
    // A small data tree: a small file, a big one (hard-linked), names the relay
    // owns (never mirrored), and a junction (never followed).
    fs.mkdirSync(path.join(src, "npc"));
    fs.writeFileSync(path.join(src, "npc", "crew.ron"), "(crew)");
    fs.writeFileSync(path.join(src, "big.bin"), Buffer.alloc((1 << 20) + 1, 7));
    fs.writeFileSync(path.join(src, "server-config.json"), '{"from":"the tree"}');
    fs.writeFileSync(path.join(src, "relay.db"), "the operator's own relay database");
    fs.mkdirSync(path.join(src, "uploads"));
    fs.writeFileSync(path.join(outside, "secret.txt"), "outside the tree");
    fs.symlinkSync(outside, path.join(src, "elsewhere"), "junction");

    const seen = {};
    const h = await TR.startRelay({
      sourceExe: __filename,
      expectSha256: THIS_SHA,
      prefix: "throwaway-relay-selftest-",
      config: { server_name: "mirror test" },
      dataFrom: src,
      healthTimeoutMs: 20000,
      spawnProcess: standIn(seen),
    });
    const data = path.join(h.dir, "data");
    try {
      assert.strictEqual(fs.readFileSync(path.join(data, "npc", "crew.ron"), "utf8"), "(crew)");
      assert.deepStrictEqual(JSON.parse(fs.readFileSync(path.join(data, "server-config.json"), "utf8")), { server_name: "mirror test" }, "the caller's config, not the tree's");
      assert.ok(!fs.existsSync(path.join(data, "relay.db")), "the relay's own names are never mirrored");
      assert.ok(!fs.existsSync(path.join(data, "uploads")));
      assert.ok(!fs.existsSync(path.join(data, "elsewhere")), "a junction is not followed");
      assert.deepStrictEqual([...h.data.skipped].sort(), ["relay.db", "server-config.json", "uploads"]);
      assert.strictEqual(h.data.files, 2);
      assert.strictEqual(h.data.copied, 1);
      assert.strictEqual(h.data.linked, 1, "the file over 1 MiB is hard-linked, not copied");
      assert.strictEqual(fs.statSync(path.join(data, "big.bin")).ino, fs.statSync(path.join(src, "big.bin")).ino);
      // A write by the relay to a small mirrored file stays in its copy.
      fs.writeFileSync(path.join(data, "npc", "crew.ron"), "(changed by the relay)");
      assert.strictEqual(fs.readFileSync(path.join(src, "npc", "crew.ron"), "utf8"), "(crew)");
    } finally {
      await h.stop();
    }
    assert.ok(!fs.existsSync(data), "stop() removed its folder");
    assert.strictEqual(fs.readFileSync(path.join(src, "big.bin")).length, (1 << 20) + 1, "removing the folder left the tree's big file alone");
    assert.strictEqual(fs.readFileSync(path.join(outside, "secret.txt"), "utf8"), "outside the tree");

    // And by default it is this tree's own data/.
    const d = await TR.startRelay({
      sourceExe: __filename,
      expectSha256: THIS_SHA,
      prefix: "throwaway-relay-selftest-",
      healthTimeoutMs: 20000,
      spawnProcess: standIn({}),
    });
    try {
      const held = fs.readdirSync(path.join(d.dir, "data"));
      const mine = path.join(d.dir, "data", "npc", "crew.ron");
      assert.ok(fs.existsSync(mine), `the relay's folder holds this tree's data/npc/crew.ron (it held: ${JSON.stringify(held.slice(0, 12))})`);
      assert.strictEqual(fs.readFileSync(mine, "utf8"), fs.readFileSync(path.join(TR.REPO_DATA, "npc", "crew.ron"), "utf8"));
      assert.ok(!held.includes("relay.db"), "never a relay.db from the tree");
      assert.ok(d.data && d.data.files > 100, `the whole data/ tree was mirrored (${JSON.stringify(d.data)})`);
    } finally {
      await d.stop();
    }
  } finally {
    try {
      fs.rmdirSync(path.join(src, "elsewhere"));
    } catch {}
    fs.rmSync(src, { recursive: true, force: true });
    fs.rmSync(outside, { recursive: true, force: true });
  }
});

// ── An explicit environment for the relay (ship homes increment 5, 2026-10-05) ──
// verify-copresence --build needs a rank holder: the relay makes every key in ADMIN_KEYS an admin
// at startup (src/relay/mod.rs), and the built-in Admin role holds the ship-editing rank. relayEnv
// drops the SHELL's ADMIN_KEYS on purpose (a developer's own key must never make a stranger on a
// throwaway relay an admin, and a rig must not depend on what its shell happens to hold), so a rig
// names its admin itself: startRelay({ env: { ADMIN_KEYS } }). What it may not name: the port,
// the database and the listen address, which are what make it a throwaway loopback relay.
// Red first, 2026-10-05, against the lib before the option:
//   AssertionError [ERR_ASSERTION]: an explicit ADMIN_KEYS goes into the relay's environment
//   + actual - expected
//   + undefined
//   - 'rig-admin-key'
test("an explicit admin key reaches the relay, the shell's does not", async () => {
  const before = process.env.ADMIN_KEYS;
  process.env.ADMIN_KEYS = "shell-admin-key"; // a developer's shell holding their own admin key
  try {
    const plain = TR.relayEnv(43210, "x/relay.db");
    assert.strictEqual(plain.ADMIN_KEYS, undefined, "the shell's ADMIN_KEYS never reaches a throwaway relay");
    const named = TR.relayEnv(43210, "x/relay.db", { ADMIN_KEYS: "rig-admin-key" });
    assert.strictEqual(named.ADMIN_KEYS, "rig-admin-key", "an explicit ADMIN_KEYS goes into the relay's environment");
    assert.strictEqual(named.BIND_ADDRESS, "127.0.0.1", "and the relay still listens on loopback only");

    // Through startRelay, to the relay process itself (the stand-in reports what it was given).
    const seen = {};
    const h = await TR.startRelay({
      sourceExe: __filename,
      expectSha256: THIS_SHA,
      prefix: "throwaway-relay-selftest-",
      healthTimeoutMs: 20000,
      spawnProcess: standIn(seen),
      env: { ADMIN_KEYS: "rig-admin-key" },
    });
    try {
      assert.ok(h.health, `the stand-in relay answered /health (log: ${h.logText()})`);
      assert.strictEqual(h.health.admin_keys, "rig-admin-key", `the rig's admin key reached the relay (it was given ${JSON.stringify(h.health.admin_keys)})`);
    } finally {
      await h.stop();
    }
    const none = await TR.startRelay({
      sourceExe: __filename,
      expectSha256: THIS_SHA,
      prefix: "throwaway-relay-selftest-",
      healthTimeoutMs: 20000,
      spawnProcess: standIn({}),
    });
    try {
      assert.strictEqual(none.health.admin_keys, null, `with none named, the relay is given no admin key at all, never the shell's (it was given ${JSON.stringify(none.health.admin_keys)})`);
    } finally {
      await none.stop();
    }

    // What makes it a throwaway loopback relay cannot be named, in any case of letters, and a
    // refused env starts nothing.
    for (const k of ["BIND_ADDRESS", "PORT", "DATABASE_PATH", "bind_address"]) {
      const s = {};
      let err = null;
      try {
        const x = await TR.startRelay({
          sourceExe: __filename,
          expectSha256: THIS_SHA,
          prefix: "throwaway-relay-selftest-",
          healthTimeoutMs: 20000,
          spawnProcess: standIn(s),
          env: { [k]: "0.0.0.0" },
        });
        await x.stop();
      } catch (e) {
        err = e;
      }
      if (s.child) {
        s.child.kill();
        await exitedOrGone(s.child);
      }
      assert.ok(err, `env may not set ${k} (startRelay resolved instead)`);
      assert.match(err.message, /sets .* itself/);
      assert.ok(!s.child, `nothing was started for an env setting ${k}`);
    }
  } finally {
    if (before === undefined) delete process.env.ADMIN_KEYS;
    else process.env.ADMIN_KEYS = before;
  }
});
