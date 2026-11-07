// Where the relay watchdog's notices go (scripts/humanity-relay-watchdog.sh), BUG-252, 2026-11-06.
//
// Run: node --test scripts/tests/watchdog-target.test.js (in `just rig-tests`).
//
// public.guide's relay restarted every 30 minutes for two days (a stale backup script emptied its
// log), and each time its watchdog posted "[Watchdog] Relay recovered and is responding again."
// into #announcements, the public channel every visitor reads first: the operator's screenshot was
// a channel full of them. A server's health is its admins' business, so the recovery notice goes
// where the down alert already went, the alert channels the admins set up (Server Settings > ADMIN
// > Alert channels, sent through scripts/humanity-alert.js), and the journal; never a chat channel.
//
// This file runs the real watchdog under bash with stand-ins on PATH: curl answers the health
// check from a file the test sets (200 or 503) and logs every call with its arguments (a chat
// post is a curl POST to the relay's /api/send); node logs the alert script's call (the admins'
// channels); logger and systemctl log. REPO and STATE_FILE point into a scratch folder through the
// script's own settings.
//
// Seen red 2026-11-06 against the watchdog as it was (HOS_WATCHDOG pointed at HEAD's copy, its
// REPO and STATE_FILE lines pointed at the scratch folder by hand, since that copy had no settings):
// "a flapping relay never posts to a chat channel" failed, three POSTs to /api/send with
// "channel":"announcements", one per recovery.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const WATCHDOG = process.env.HOS_WATCHDOG || path.join(__dirname, "..", "humanity-relay-watchdog.sh");
const WINDOWS = process.platform === "win32";

function bashPath() {
  if (!WINDOWS) return "bash";
  for (const p of ["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"]) {
    if (fs.existsSync(p)) return p;
  }
  return "bash";
}
const px = (p) => (WINDOWS ? p.replace(/\\/g, "/").replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`) : p);
const read = (p) => fs.readFileSync(p, "utf8");

const worlds = [];
test.after(() => {
  for (const r of worlds) fs.rmSync(r, { recursive: true, force: true });
});

// A scratch server: a checkout with the bot secret in .env (the old notice's credential), the
// alert script and a relay program, and the stand-ins.
function makeServer() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum-watchdog-"));
  worlds.push(root);
  const w = { root, repo: path.join(root, "opt", "Humanity"), bin: path.join(root, "stubs"), log: path.join(root, "stub.log") };
  w.state = path.join(root, "run", "humanity-relay-watchdog.state");
  w.health = path.join(root, "health-code");
  fs.mkdirSync(path.join(w.repo, "scripts"), { recursive: true });
  fs.mkdirSync(path.join(w.repo, "target", "release"), { recursive: true });
  fs.mkdirSync(path.dirname(w.state), { recursive: true });
  fs.mkdirSync(w.bin);
  fs.writeFileSync(path.join(w.repo, ".env"), "API_SECRET=s3cret\n");
  fs.writeFileSync(path.join(w.repo, "scripts", "humanity-alert.js"), "// stand-in\n");
  const relay = path.join(w.repo, "target", "release", "HumanityOS");
  fs.writeFileSync(relay, "#!/bin/sh\n");
  fs.chmodSync(relay, 0o755);
  const stub = (name, body) => {
    fs.writeFileSync(path.join(w.bin, name), `#!/bin/bash\n${body}\n`);
    fs.chmodSync(path.join(w.bin, name), 0o755);
  };
  // The health check is the call with -w (it prints the HTTP code); anything else is a post.
  stub("curl", '{ printf "curl"; for a in "$@"; do printf " %s" "$a"; done; echo; } >> "$STUB_LOG"\nfor a in "$@"; do if [ "$a" = -w ]; then cat "$FAKE_HEALTH"; exit 0; fi; done\nexit 0');
  stub("node", '{ printf "node"; for a in "$@"; do printf " [%s]" "$a"; done; echo; } >> "$STUB_LOG"\nexit 0');
  stub("logger", 'echo "logger $*" >> "$STUB_LOG"\nexit 0');
  stub("systemctl", 'echo "systemctl $*" >> "$STUB_LOG"\nexit 0');
  return w;
}

// One watchdog run (one timer tick) with the relay answering `code`.
function tick(w, code) {
  fs.writeFileSync(w.health, code);
  const copy = path.join(w.root, "watchdog.sh");
  fs.writeFileSync(copy, read(WATCHDOG).replace(/\r\n/g, "\n"));
  const r = spawnSync(bashPath(), ["-c", 'export PATH="$STUB_DIR:$PATH"; exec bash "$WATCHDOG_COPY"'], {
    encoding: "utf8",
    timeout: 30000,
    env: { ...process.env, STUB_DIR: px(w.bin), STUB_LOG: px(w.log), FAKE_HEALTH: px(w.health), WATCHDOG_COPY: px(copy), REPO: px(w.repo), STATE_FILE: px(w.state) },
  });
  return { status: r.status, out: `${r.stdout || ""}${r.stderr || ""}` };
}
const logLines = (w) => (fs.existsSync(w.log) ? read(w.log).split(/\r?\n/).filter(Boolean) : []);

test("a flapping relay never posts to a chat channel; each recovery goes to the admins' alert channels and the journal, in plain words", () => {
  const w = makeServer();
  tick(w, "200");
  // Three outages, as public.guide's half-hourly ones: a failed check, a second (restart), healthy.
  for (let i = 0; i < 3; i++) {
    tick(w, "503");
    tick(w, "503");
    tick(w, "200");
  }
  const lines = logLines(w);
  const posts = lines.filter((l) => l.startsWith("curl") && !/ -w /.test(l));
  assert.deepStrictEqual(posts, [], `a flapping relay never posts to a chat channel (posts: ${posts.join(" | ")})`);
  assert.ok(!lines.some((l) => /announcements|\/api\/send/.test(l)), "nothing is sent to #announcements or the chat API");
  assert.strictEqual(lines.filter((l) => l === "systemctl restart humanity-relay").length, 3, "the relay is restarted once per outage");

  const alerts = lines.filter((l) => l.startsWith("node "));
  const recovered = alerts.filter((l) => / \[info\]$/.test(l));
  assert.strictEqual(recovered.length, 3, `each recovery is sent to the admins' alert channels once (${alerts.join(" | ")})`);
  for (const l of recovered) {
    const m = l.match(/^node \[(\S+humanity-alert\.js)\] \[([^\]]+)\] \[info\]$/);
    assert.ok(m, `through the alert script: ${l}`);
    // Plain words: a sentence a person reads, no codes, no unit names, no shell.
    assert.match(m[2], /^[A-Za-z][A-Za-z ,.'-]+\.$/, `plain words: ${m[2]}`);
    assert.doesNotMatch(m[2], /HTTP|\b\d{3}\b|systemctl|journalctl|unit|\[/, `plain words: ${m[2]}`);
  }
  assert.strictEqual(lines.filter((l) => /^logger -t humanity-relay-watchdog -- RECOVERED/.test(l)).length, 3, "and each is in the journal");
  // The down alert went out once per outage too, not on every tick.
  assert.strictEqual(alerts.filter((l) => / \[warn\]$/.test(l)).length, 3, "one down alert per outage");
});

// BUG-169's case kept: one failed check and then healthy (a deploy's restart, one slow answer) is
// not an outage, and nothing is sent anywhere.
test("one failed check and then healthy sends nothing to anyone", () => {
  const w = makeServer();
  tick(w, "200");
  tick(w, "503");
  tick(w, "200");
  const lines = logLines(w);
  assert.deepStrictEqual(lines.filter((l) => l.startsWith("node ") || (l.startsWith("curl") && !/ -w /.test(l))), []);
  assert.ok(lines.some((l) => /not an outage, nothing posted|not an outage, nothing sent/.test(l)));
});
