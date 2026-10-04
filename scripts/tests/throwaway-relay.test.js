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
// Pure node apart from one live check, which lists THIS process's own
// listener (a node server on 127.0.0.1, which never prompts) through the real
// netstat / ss / lsof, so the parser is proven on real output too, not only
// on the samples below. Starts no relay. Run: node --test
// scripts/tests/throwaway-relay.test.js (in `just rig-tests`).

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
