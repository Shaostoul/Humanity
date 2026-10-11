// The self-hosted relay's release follower (scripts/relay-follow-releases.sh, BUG-192), run for
// real under bash against a scratch node.
//
// Run: node --test scripts/tests/relay-follow.test.js
//
// The follower runs hourly as root on a relay the deploy workflow does not update (public.guide)
// and its update path had never run when a review found eight defects in it (2026-10-11): a roll
// back gave the database to root, deleted the follower itself and left the watchdog script
// unrunnable; an update threw away the node's edits to tracked files; a hung release held the
// health check until systemd killed the run, and a killed run left nothing to roll back from;
// it would follow GitHub's "Latest" backwards; git as root refused a checkout owned by the relay's
// user; and the backups were readable by everyone. This file runs the real script, nothing pulled
// out of it, in a scratch world:
//   - a real git repo with three tagged "releases" (v0.1.0, v0.2.0, v0.3.0), cloned the way a
//     node's /opt/Humanity is and checked out at v0.1.0;
//   - stand-ins on PATH (scripts/tests/fixtures/relay-follow/): curl is a fake GitHub and a fake
//     /health that fails while the installed program is one the test lists as broken; systemctl
//     logs, and on a restart the "new release" writes to the database (and can kill the follower,
//     as a time limit or a reboot would); sqlite3 copies the database for .backup; install, chown
//     and chmod log what they were asked (install and chmod then do it, without -o and -g, and
//     install can kill the follower just before the new program goes in); stat
//     answers for the database with the relay user's owner and mode; and git behaves like git run
//     as root on a checkout another user owns, refusing it unless the call names safe.directory,
//     so the cases below also prove that defect fixed (taking safe.directory off the script's
//     git calls turned ten of the eleven red, all but the static unit check);
//   - REPO, STATE_DIR, INSTALLED_COPY, LATEST_API and DOWNLOAD_BASE pointed into the scratch
//     world through the script's own settings, and the follower run from the INSTALLED copy, as
//     the unit runs it.
//
// Each case was seen failing once against a deliberately broken copy of the script
// (HOS_FOLLOW_SCRIPT pointed at the broken copy; HOS_FOLLOW_UNIT for the unit), then passing on
// the real one; its comment says which break and what the red run showed. Those runs were on
// Windows (2026-10-11): the checks inside `if (!WINDOWS)` (file modes read back from disk) have
// not been run yet; on Windows the same facts are read from the stand-ins' log instead (the
// umask a backup was made under, chmod 700 on the state folder, chmod +x on the watchdog).
// Needs git and bash (Git for Windows' on Windows). About a minute on Windows.

const test = require("node:test");
const assert = require("node:assert");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync, spawnSync } = require("node:child_process");

const SCRIPTS = path.join(__dirname, "..");
const FOLLOWER = process.env.HOS_FOLLOW_SCRIPT || path.join(SCRIPTS, "relay-follow-releases.sh");
const UNIT = process.env.HOS_FOLLOW_UNIT || path.join(SCRIPTS, "systemd", "humanity-relay-follow.service");
const STUBS = path.join(__dirname, "fixtures", "relay-follow");
const WINDOWS = process.platform === "win32";

const LATEST_API = "https://api.github.test/repos/Shaostoul/Humanity/releases/latest";
const DOWNLOAD_BASE = "https://github.test/Shaostoul/Humanity/releases/download";
const DB_AS_LEFT = "db: as the running release left it\n";
// The relay's own user and the live file's mode, as stat (the stand-in) reports them.
const RELAY_OWNER = "1001:1002";
const RELAY_MODE = "640";

// Git for Windows' bash, not the WSL launcher in System32 that a bare "bash" can resolve to.
function bashPath() {
  if (!WINDOWS) return "bash";
  for (const p of ["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"]) {
    if (fs.existsSync(p)) return p;
  }
  return "bash";
}

// A path as bash sees it (C:\x\y is /c/x/y under Git Bash).
function px(p) {
  if (!WINDOWS) return p;
  return p.replace(/\\/g, "/").replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`);
}

function git(cwd, ...args) {
  return gitRaw(cwd, ...args).trim();
}
function gitRaw(cwd, ...args) {
  return execFileSync(
    "git",
    ["-c", "core.autocrlf=false", "-c", "user.name=Follow Test", "-c", "user.email=follow@test.invalid", ...args],
    { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
}

const read = (p) => fs.readFileSync(p, "utf8");
const exists = (p) => fs.existsSync(p);

// data/server-config.json at v0.1.0: long enough that an edit near the top and one near the
// bottom are three-way-mergeable (git's context is three lines).
const CONFIG_V1 = [
  "{", '  "name": "Relay",', '  "a": 1,', '  "b": 2,', '  "c": 3,', '  "d": 4,', '  "e": 5,',
  '  "f": 6,', '  "g": 7,', '  "h": 8,', '  "i": 9,', '  "port": 3210,', '  "z": 0', "}", "",
].join("\n");

// The scratch world: a release history, a node checked out at v0.1.0 and running v0.1.0's
// relay, a fake GitHub whose "Latest" is v0.2.0, and the follower installed outside the repo.
function makeWorld() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum-follow-"));
  worlds.push({ root });
  const w = {
    root,
    build: path.join(root, "build"),
    upstream: path.join(root, "upstream.git"),
    repo: path.join(root, "opt", "Humanity"),
    state: path.join(root, "var-lib-humanity-follow"),
    installed: path.join(root, "usr-local-lib", "humanity", "relay-follow-releases.sh"),
    gh: path.join(root, "github"),
    bin: path.join(root, "stubs"),
    log: path.join(root, "stub.log"),
    tmp: path.join(root, "tmp"),
  };
  w.db = path.join(w.repo, "data", "relay.db");
  w.relay = path.join(w.repo, "target", "release", "HumanityOS");
  w.config = path.join(w.repo, "data", "server-config.json");
  w.watchdog = path.join(w.repo, "scripts", "humanity-relay-watchdog.sh");

  fs.mkdirSync(w.bin);
  for (const f of fs.readdirSync(STUBS)) {
    // The stand-ins have no .sh name for .gitattributes to keep at LF, and bash refuses a CR.
    fs.writeFileSync(path.join(w.bin, f), read(path.join(STUBS, f)).replace(/\r\n/g, "\n"));
    fs.chmodSync(path.join(w.bin, f), 0o755);
  }
  fs.mkdirSync(w.tmp);

  // The release history.
  const b = w.build;
  fs.mkdirSync(path.join(b, "data"), { recursive: true });
  fs.mkdirSync(path.join(b, "scripts"), { recursive: true });
  git(root, "init", "-q", b);
  fs.writeFileSync(path.join(b, ".gitignore"), "data/relay.db*\ntarget/\n");
  fs.writeFileSync(path.join(b, "data", "server-config.json"), CONFIG_V1);
  fs.writeFileSync(path.join(b, "scripts", "humanity-relay-watchdog.sh"), "#!/bin/bash\necho watchdog\n");
  git(b, "add", "-A");
  git(b, "update-index", "--chmod=-x", "scripts/humanity-relay-watchdog.sh"); // not executable in git at v0.1.0
  git(b, "commit", "-qm", "v0.1.0");
  git(b, "tag", "v0.1.0");
  // v0.2.0: changes the port (near the bottom), marks the watchdog executable, and is the first
  // release to carry the follower (as v0.1485.1 did not).
  fs.writeFileSync(path.join(b, "data", "server-config.json"), CONFIG_V1.replace('"port": 3210', '"port": 3211'));
  fs.writeFileSync(path.join(b, "scripts", "relay-follow-releases.sh"), read(FOLLOWER) + "\n# the v0.2.0 copy\n");
  git(b, "add", "-A");
  git(b, "update-index", "--chmod=+x", "scripts/humanity-relay-watchdog.sh", "scripts/relay-follow-releases.sh");
  git(b, "commit", "-qm", "v0.2.0");
  git(b, "tag", "v0.2.0");
  // v0.3.0: renames the relay (near the top), the line a node's own edit below also changes.
  fs.writeFileSync(
    path.join(b, "data", "server-config.json"),
    CONFIG_V1.replace('"port": 3210', '"port": 3211').replace('"name": "Relay"', '"name": "Relay three"'),
  );
  git(b, "commit", "-qam", "v0.3.0");
  git(b, "tag", "v0.3.0");
  git(root, "clone", "-q", "--bare", b, w.upstream);

  // The node: cloned from "GitHub", at v0.1.0, running v0.1.0's relay on its database.
  fs.mkdirSync(path.dirname(w.repo), { recursive: true });
  git(root, "clone", "-q", "-c", "core.autocrlf=false", w.upstream, w.repo);
  git(w.repo, "checkout", "-q", "v0.1.0");
  fs.writeFileSync(w.db, DB_AS_LEFT);
  fs.mkdirSync(path.dirname(w.relay), { recursive: true });
  fs.writeFileSync(w.relay, "relay-v0.1.0");

  // The follower, installed where the unit runs it from.
  fs.mkdirSync(path.dirname(w.installed), { recursive: true });
  fs.copyFileSync(FOLLOWER, w.installed);

  // GitHub: each release's relay and its .sha256.
  for (const tag of ["v0.1.0", "v0.2.0", "v0.3.0"]) {
    const dir = path.join(w.gh, "download", tag);
    fs.mkdirSync(dir, { recursive: true });
    const relay = `relay-${tag}`;
    fs.writeFileSync(path.join(dir, "HumanityOS-relay-linux-x64"), relay);
    const sum = crypto.createHash("sha256").update(relay).digest("hex");
    fs.writeFileSync(path.join(dir, "HumanityOS-relay-linux-x64.sha256"), `${sum}  HumanityOS-relay-linux-x64\n`);
  }
  setLatest(w, "v0.2.0");
  return w;
}

function setLatest(w, tag) {
  fs.writeFileSync(path.join(w.gh, "latest.json"), JSON.stringify({ tag_name: tag, prerelease: false }, null, 2));
}

// One run of the follower, from its installed copy, as the unit runs it.
function follow(w, env = {}) {
  const r = spawnSync(bashPath(), ["-c", 'export PATH="$STUB_DIR:$PATH"; exec bash "$FOLLOWER_PATH"'], {
    encoding: "utf8",
    timeout: 180000,
    env: {
      ...process.env,
      REPO: px(w.repo),
      STATE_DIR: px(w.state),
      INSTALLED_COPY: px(w.installed),
      WEB_ROOT: px(path.join(w.root, "no-web-root")),
      LATEST_API,
      DOWNLOAD_BASE,
      HEALTH_TRIES: "2",
      HEALTH_PAUSE: "0",
      FOLLOWER_PATH: px(w.installed),
      STUB_DIR: px(w.bin),
      STUB_LOG: px(w.log),
      FAKE_GH: px(w.gh),
      FAKE_DB_OWNER: RELAY_OWNER,
      FAKE_DB_MODE: RELAY_MODE,
      TMPDIR: px(w.tmp),
      ...env,
    },
  });
  return { status: r.status, out: `${r.stdout || ""}${r.stderr || ""}` };
}

const logLines = (w) => (exists(w.log) ? read(w.log).split(/\r?\n/).filter(Boolean) : []);
const head = (w) => git(w.repo, "rev-parse", "HEAD");
const tagCommit = (w, tag) => git(w.repo, "rev-parse", `${tag}^{commit}`);
const state = (w, name) => path.join(w.state, name);
const watchdogChmods = (w) => logLines(w).filter((l) => /^chmod \+x \S*\/scripts\/humanity-relay-watchdog\.sh$/.test(l));

function cleanup(w) {
  fs.rmSync(w.root, { recursive: true, force: true });
}
// A case that fails stops before its cleanup(); its world goes at the end instead (set
// HOS_FOLLOW_KEEP=1 to keep them for a look).
const worlds = [];
test.after(() => {
  if (!process.env.HOS_FOLLOW_KEEP) for (const w of worlds) cleanup(w);
});

// Seen red three ways: with the self-install block taken out ("the installed follower is the
// release's copy": it was still the old one); with `umask 077` taken out ("the database backup is
// made under umask 077"); and with `umask 022` taken out of g() ("git runs under umask 022: git
// (umask 0077) ... describe --tags --exact-match"), which would have written the release's files
// unreadable by the relay's user.
test("a newer release is installed, recorded, and brings its own copy of the follower", () => {
  const w = makeWorld();
  // A backup left world-readable by the old script (which ran without a umask).
  fs.mkdirSync(w.state, { recursive: true });
  fs.writeFileSync(state(w, "relay.db.before-v0.0.9-20261001T000000Z"), "old backup");
  fs.chmodSync(state(w, "relay.db.before-v0.0.9-20261001T000000Z"), 0o644);

  const r = follow(w);
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /updated to v0\.2\.0; the relay is healthy/);
  assert.strictEqual(head(w), tagCommit(w, "v0.2.0"), "the checkout is at the release");
  assert.strictEqual(read(w.relay), "relay-v0.2.0", "the release's program is installed");
  assert.strictEqual(read(state(w, "installed_tag")).trim(), "v0.2.0");
  assert.ok(!exists(state(w, "pending")), "no pending record is left behind");
  assert.ok(!exists(state(w, "skipped_tag")));
  assert.match(read(w.db), /written by the new release/, "a healthy update keeps the new release's writes");
  assert.strictEqual(read(state(w, "HumanityOS.previous")), "relay-v0.1.0", "the previous program is kept");
  const backups = fs.readdirSync(w.state).filter((f) => f.startsWith("relay.db.before-v0.2.0-"));
  assert.strictEqual(backups.length, 1, "one database backup for this update");
  assert.strictEqual(read(state(w, backups[0])), DB_AS_LEFT, "taken before the new release ran");
  assert.ok(logLines(w).some((l) => /^systemctl restart humanity-relay$/.test(l)));
  // The follower: the release's copy now, installed at the install path.
  assert.match(read(w.installed), /# the v0\.2\.0 copy/, "the installed follower is the release's copy");
  assert.strictEqual(read(w.installed), read(path.join(w.repo, "scripts", "relay-follow-releases.sh")));
  assert.ok(watchdogChmods(w).length >= 1, "the watchdog is made executable after the checkout");
  // Backups root-only: made under umask 077 in a folder set to 700 (what Windows can show; the
  // modes themselves are checked below where the file system keeps them).
  const lines = logLines(w);
  const backup = lines.filter((l) => l.startsWith("sqlite3 "));
  assert.strictEqual(backup.length, 1);
  assert.match(backup[0], /^sqlite3 \(umask 0077\) /, "the database backup is made under umask 077");
  assert.ok(lines.some((l) => l === `chmod 700 ${px(w.state)}`), "the state folder is set to 700");
  assert.ok(lines.some((l) => l === `chmod -R go-rwx ${px(w.state)}`), "and what was in it before is tightened");
  // git writes the release's files for the relay's user to read: never under the script's 077.
  const gits = lines.filter((l) => l.startsWith("git "));
  assert.ok(gits.some((l) => / checkout -q -f v0\.2\.0$/.test(l)), "the checkout went through git");
  for (const l of gits) assert.match(l, /^git \(umask 0022\) /, `git runs under umask 022: ${l}`);
  if (!WINDOWS) {
    assert.ok(fs.statSync(w.watchdog).mode & 0o100, "the watchdog script is executable");
    assert.strictEqual(fs.statSync(w.state).mode & 0o777, 0o700, "the state folder is root-only");
    for (const f of fs.readdirSync(w.state)) {
      assert.strictEqual(fs.statSync(state(w, f)).mode & 0o077, 0, `${f} is readable by its owner alone`);
    }
  }
  // The next hour: nothing to do.
  const again = follow(w);
  assert.strictEqual(again.status, 0, again.out);
  assert.match(again.out, /already on v0\.2\.0/);
  cleanup(w);
});

// Seen red three ways: with the database put back by `cp -a "$db_backup" "$DB"` again, the
// reviewed defect ("one database restore": 0 !== 1, no install -o 1001 -g 1002 -m 640 line in the
// log); with watchdog_runnable taken out of the roll back ("the watchdog is made executable after
// the update AND after the roll back"); and with the roll back taking the follower from the
// checkout, as the old layout did (the installed copy was gone: ENOENT, v0.1.0 carries none).
// The watchdog pause: seen red 2026-10-11 with the timer stop taken out of roll_back (the Linux
// review): "the watchdog's timer is paused around the swap".
test("a release that fails its health check is rolled back: program, checkout, database (with the relay user's owner), follower and watchdog", () => {
  const w = makeWorld();
  fs.writeFileSync(path.join(w.gh, "unhealthy"), "relay-v0.2.0\n");
  const followerBefore = read(w.installed);

  const r = follow(w);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /did not come back healthy on v0\.2\.0: rolling back/);
  assert.match(r.out, /the relay is healthy on v0\.1\.0/);
  assert.strictEqual(read(w.relay), "relay-v0.1.0", "the previous program is back");
  assert.strictEqual(head(w), tagCommit(w, "v0.1.0"), "the checkout is back at the previous release");
  assert.strictEqual(read(w.db), DB_AS_LEFT, "the database is back as it was before the update");
  assert.ok(!exists(`${w.db}-wal`), "the new release's WAL is not left to be replayed onto the old database");
  const restore = logLines(w).filter((l) => /\/data\/relay\.db$/.test(l) && l.startsWith("install "));
  assert.strictEqual(restore.length, 1, `one database restore (log: ${logLines(w).join(" | ")})`);
  assert.match(
    restore[0],
    /^install -o 1001 -g 1002 -m 640 \S*\/relay\.db\.before-v0\.2\.0-\S+ \S*\/data\/relay\.db$/,
    "the database goes back with the live file's owner and mode",
  );
  assert.strictEqual(read(state(w, "skipped_tag")).trim(), "v0.2.0");
  assert.ok(!exists(state(w, "pending")) && !exists(state(w, "pending.failed")), "the record is cleared");
  assert.strictEqual(read(w.installed), followerBefore, "the installed follower is untouched by a roll back");
  assert.ok(watchdogChmods(w).length >= 2, "the watchdog is made executable after the update AND after the roll back");
  if (!WINDOWS) assert.ok(fs.statSync(w.watchdog).mode & 0o100, "the watchdog script is executable at v0.1.0 too");
  // The watchdog's timer is paused across the swap (it could restart the relay between the -wal
  // removal and the database going back), and started again after.
  const lines = logLines(w);
  const at = (re) => lines.findIndex((l) => re.test(l));
  const paused = at(/^systemctl stop humanity-relay-watchdog\.timer$/);
  const swapped = at(/^install -o .*\/data\/relay\.db$/);
  const resumed = at(/^systemctl start humanity-relay-watchdog\.timer$/);
  assert.ok(paused >= 0 && paused < swapped && swapped < resumed, `the watchdog's timer is paused around the swap (log: ${lines.join(" | ")})`);

  // The next hour: that release is skipped, nothing is touched.
  fs.writeFileSync(w.log, "");
  const again = follow(w);
  assert.strictEqual(again.status, 0, again.out);
  assert.match(again.out, /v0\.2\.0 was rolled back here before; waiting for a newer release/);
  assert.ok(!logLines(w).some((l) => l.startsWith("systemctl")));
  cleanup(w);
});

// Seen red 2026-10-11 with `ok=0` taken off the unhealthy-after-roll-back branch (the Linux
// review): "a roll back that leaves the relay unhealthy is not finished" (the record was cleared
// and the next run exited 0, "waiting for a newer release", with the relay down).
test("a roll back that leaves the relay unhealthy is kept as stuck, and every later run says so", () => {
  const w = makeWorld();
  fs.writeFileSync(path.join(w.gh, "unhealthy"), "relay-v0.2.0\nrelay-v0.1.0\n");
  const r = follow(w);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /NOT healthy after the roll back either/);
  assert.ok(exists(state(w, "pending.failed")), "a roll back that leaves the relay unhealthy is not finished");
  const again = follow(w);
  assert.strictEqual(again.status, 1, again.out);
  assert.match(again.out, /a roll back here did not finish/);
  cleanup(w);
});

// Seen red with the "pending record exists" block taken out of the script: the second run said
// "already on v0.2.0" and exited 0, leaving the half-finished update in place ("the next run rolls
// the interrupted update back": 0 !== 1). Also red with the database put back by `cp -a` ("with
// the relay user's owner").
test("a run killed partway through an update is rolled back by the next run", () => {
  const w = makeWorld();
  const killed = follow(w, { FAKE_KILL_ON_RESTART: "1" });
  assert.notStrictEqual(killed.status, 0, killed.out);
  assert.doesNotMatch(killed.out, /updated to/);
  assert.ok(exists(state(w, "pending")), "the killed run left its record");
  assert.strictEqual(read(w.relay), "relay-v0.2.0", "it was killed with the new program in place");

  const r = follow(w);
  assert.strictEqual(r.status, 1, `the next run rolls the interrupted update back: ${r.out}`);
  assert.match(r.out, /stopped partway through updating to v0\.2\.0/);
  assert.strictEqual(read(w.relay), "relay-v0.1.0");
  assert.strictEqual(head(w), tagCommit(w, "v0.1.0"));
  assert.strictEqual(read(w.db), DB_AS_LEFT, "the database the new release ran on is put back");
  assert.strictEqual(read(state(w, "HumanityOS.previous")), "relay-v0.1.0", "the saved previous program was never overwritten");
  assert.strictEqual(read(state(w, "skipped_tag")).trim(), "v0.2.0");
  assert.ok(!exists(state(w, "pending")));
  assert.ok(logLines(w).some((l) => /^install -o 1001 -g 1002 -m 640 /.test(l)), "with the relay user's owner");

  const third = follow(w);
  assert.strictEqual(third.status, 0, third.out);
  assert.match(third.out, /v0\.2\.0 was rolled back here before/);
  cleanup(w);
});

// The record is written before the checkout and the program are touched, so a run killed there
// is undone too; but the new release never ran, so the database the old relay kept writing to is
// left as it is (putting the backup back would lose those writes). Seen red with the database
// put back on every roll back (the `restarted = 1` condition taken out): "the old relay's newest
// writes are kept" failed. And with the pending-record block taken out ("already on v0.2.0").
test("a run killed before the new release started is undone without touching the database", () => {
  const w = makeWorld();
  const killed = follow(w, { FAKE_KILL_ON_INSTALL: "1" });
  assert.notStrictEqual(killed.status, 0, killed.out);
  assert.ok(exists(state(w, "pending")), "the killed run left its record");
  assert.strictEqual(head(w), tagCommit(w, "v0.2.0"), "it was killed after the checkout");

  const r = follow(w);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /stopped partway through updating to v0\.2\.0/);
  assert.strictEqual(head(w), tagCommit(w, "v0.1.0"));
  assert.strictEqual(read(w.relay), "relay-v0.1.0");
  assert.match(read(w.db), /written by the old relay after the backup/, "the old relay's newest writes are kept");
  assert.ok(!logLines(w).some((l) => /\/data\/relay\.db$/.test(l) && l.startsWith("install ")), "the database was not restored");
  assert.strictEqual(read(state(w, "skipped_tag")).trim(), "v0.2.0");
  assert.ok(!exists(state(w, "pending")));
  cleanup(w);
});

// Seen red with the `g apply --3way "$edits"` line taken out of the update path: the node's
// edit was gone after the update ("the node's own edit survives").
test("a node's own edit to a tracked file is carried onto the release", () => {
  const w = makeWorld();
  fs.writeFileSync(w.config, read(w.config).replace('"name": "Relay"', '"name": "My node"'));
  // On public.guide the watchdog's executable bit is a local mode change: not an edit.
  if (!WINDOWS) fs.chmodSync(w.watchdog, 0o755);

  const r = follow(w);
  assert.strictEqual(r.status, 0, r.out);
  assert.strictEqual(head(w), tagCommit(w, "v0.2.0"));
  const cfg = read(w.config);
  assert.match(cfg, /"name": "My node"/, "the node's own edit survives");
  assert.match(cfg, /"port": 3211/, "and the release's change is there too");
  assert.match(r.out, /carried the local edits to data\/server-config\.json/);
  assert.strictEqual(
    gitRaw(w.repo, "status", "--porcelain", "--untracked-files=no"),
    " M data/server-config.json\n",
    "left as a local edit, not staged, and nothing else changed",
  );
  cleanup(w);
});

// public.guide's own case: the follower was copied into the checkout by hand, untracked, and
// v0.2.0 is the first release to carry it. The checkout would overwrite that copy and a roll back
// would delete it, so it is copied aside first and put back by a roll back. Seen red with the
// roll back's copy-back line taken out: the file was gone (ENOENT on scripts/relay-follow-releases.sh).
// The file-by-file put back: the folder form (`cp -a "$files/." "$REPO/"`) gave $REPO the backup
// folder's mode, 700, on Linux (the Linux review of 2026-10-11; Windows cannot show a mode, so
// that half is read off the script). And a copy that already matches the release's is kept too:
// a roll back's checkout of the older release removes it. Seen red 2026-10-11 with the "same as
// the release's copy" skip put back: "a copy matching the release's is back after the roll back".
test("a local file at a path the release starts carrying is kept aside, and put back by a roll back", () => {
  const script = read(path.join(__dirname, "..", "relay-follow-releases.sh"));
  assert.ok(!script.includes('cp -a "$files/." "$REPO/"'), "the put back goes file by file, never the folder onto $REPO");
  const same = makeWorld();
  const sameOwn = path.join(same.repo, "scripts", "relay-follow-releases.sh");
  fs.writeFileSync(sameOwn, gitRaw(same.repo, "show", "v0.2.0:scripts/relay-follow-releases.sh"));
  fs.writeFileSync(path.join(same.gh, "unhealthy"), "relay-v0.2.0\n");
  const rs = follow(same);
  assert.strictEqual(rs.status, 1, rs.out);
  assert.ok(exists(sameOwn), "a copy matching the release's is back after the roll back");
  cleanup(same);

  const w = makeWorld();
  const own = path.join(w.repo, "scripts", "relay-follow-releases.sh");
  fs.writeFileSync(own, "#!/bin/bash\n# the copy put here by hand\n");
  fs.writeFileSync(path.join(w.gh, "unhealthy"), "relay-v0.2.0\n");

  const r = follow(w);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /scripts\/relay-follow-releases\.sh exists here and v0\.2\.0 adds its own; keeping this node's copy in /);
  assert.strictEqual(head(w), tagCommit(w, "v0.1.0"));
  assert.strictEqual(read(own), "#!/bin/bash\n# the copy put here by hand\n", "the node's own copy is back after the roll back");
  cleanup(w);
});

// The mode-only difference is made the way each system shows one. On Linux, as on public.guide:
// the watchdog made executable by hand while git has it 644. On Windows, where git cannot see an
// executable bit, by turning core.filemode on in the node's clone: git there then reports every
// file it marks executable as 644, a mode-only difference on two files. Seen red (on Windows)
// with core.fileMode=false taken off g(): the mode difference went into the edits patch, which
// did not fit onto v0.3.0, and the run refused the release (exit 1, expected 0).
test("a file-mode-only difference is not taken for a local edit", () => {
  const w = makeWorld();
  if (WINDOWS) {
    git(w.repo, "checkout", "-q", "v0.2.0");
    fs.writeFileSync(w.relay, "relay-v0.2.0");
    git(w.repo, "config", "core.filemode", "true");
    setLatest(w, "v0.3.0");
    assert.match(gitRaw(w.repo, "diff", "HEAD"), /old mode 100755/, "the setup has a mode-only difference");
  } else {
    fs.chmodSync(w.watchdog, 0o755);
    assert.match(gitRaw(w.repo, "diff", "HEAD"), /new mode 100755/, "the setup has a mode-only difference");
  }
  const target = WINDOWS ? "v0.3.0" : "v0.2.0";
  const r = follow(w);
  assert.strictEqual(r.status, 0, r.out);
  assert.strictEqual(head(w), tagCommit(w, target));
  assert.doesNotMatch(r.out, /local edits/, "no local edits were found, carried or refused");
  assert.deepStrictEqual(fs.readdirSync(w.state).filter((f) => f.startsWith("local-edits.")), [], "no edits were saved");
  cleanup(w);
});

// Seen red with the fit check (the scratch-index apply) taken out: the run went ahead, the
// three-way apply after the checkout failed ("could not put the local edits onto v0.3.0 after
// all") and the roll back put things back, but the relay had been stopped and started and the
// message did not say the edits do not fit (the regex check failed).
test("a local edit that conflicts with the release stops the update with everything as it was", () => {
  const w = makeWorld();
  setLatest(w, "v0.3.0");
  const edited = read(w.config).replace('"name": "Relay"', '"name": "My node"');
  fs.writeFileSync(w.config, edited);

  const r = follow(w);
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /local edits to data\/server-config\.json do not fit onto v0\.3\.0/);
  assert.strictEqual(read(w.config), edited, "the edit is still there, untouched");
  assert.strictEqual(head(w), tagCommit(w, "v0.1.0"), "the checkout did not move");
  assert.strictEqual(read(w.relay), "relay-v0.1.0");
  assert.strictEqual(read(w.db), DB_AS_LEFT);
  assert.strictEqual(read(state(w, "skipped_tag")).trim(), "v0.3.0");
  assert.ok(!exists(state(w, "pending")));
  const touched = logLines(w).filter((l) => /^(systemctl|install|sqlite3) /.test(l));
  assert.deepStrictEqual(touched, [], "nothing was touched: no systemctl call, no install, no backup");
  assert.strictEqual(
    gitRaw(w.repo, "status", "--porcelain", "--untracked-files=no"),
    " M data/server-config.json\n",
    "the checkout's own index is as it was too (the fit check used a scratch index)",
  );
  cleanup(w);
});

// Seen red with the version comparison replaced by the old string comparison: the run said
// "updating v0.2.0 to v0.1.0" and "updated to v0.1.0; the relay is healthy".
test("an older release marked Latest is not installed", () => {
  const w = makeWorld();
  git(w.repo, "checkout", "-q", "v0.2.0");
  fs.writeFileSync(w.relay, "relay-v0.2.0");
  setLatest(w, "v0.1.0");

  const r = follow(w);
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /GitHub's latest release, v0\.1\.0, is older than v0\.2\.0 here; not going backwards/);
  assert.strictEqual(read(w.relay), "relay-v0.2.0", "an older Latest is not installed");
  assert.strictEqual(head(w), tagCommit(w, "v0.2.0"));
  assert.ok(!logLines(w).some((l) => l.includes("/releases/download/")), "nothing was downloaded");

  // A higher number that sorts lower as text (v0.10.0 after v0.9.0) still counts as newer.
  assert.strictEqual(
    spawnSync(bashPath(), ["-c", 'printf "%s\\n%s\\n" 0.9.0 0.10.0 | sort -V | tail -n 1'], { encoding: "utf8" }).stdout.trim(),
    "0.10.0",
    "sort -V orders versions, which newer() relies on",
  );
  cleanup(w);
});

// Seen red with --max-time taken off the health check's curl: "every curl call has a time limit:
// curl -fs http://127.0.0.1:3210/health".
test("every curl call carries a time limit", () => {
  const w = makeWorld();
  fs.writeFileSync(path.join(w.gh, "unhealthy"), "relay-v0.2.0\n"); // health is checked, failed, rolled back and checked again
  const r = follow(w);
  assert.strictEqual(r.status, 1, r.out);
  const curls = logLines(w).filter((l) => l.startsWith("curl "));
  assert.ok(curls.some((l) => l.includes(LATEST_API)), "the GitHub query was made");
  assert.ok(curls.filter((l) => l.includes("/releases/download/")).length === 2, "both downloads were made");
  assert.ok(curls.filter((l) => l.endsWith("/health")).length >= 3, "health was checked on the update and after the roll back");
  for (const l of curls) assert.match(l, /--max-time [1-9][0-9]*/, `every curl call has a time limit: ${l}`);
  cleanup(w);
});

// Seen red against the unit as reviewed (HOS_FOLLOW_UNIT pointed at HEAD's copy): "the unit runs
// a copy outside the checkout (/opt/Humanity/scripts/relay-follow-releases.sh)". And against
// today's unit with its old TimeoutStartSec=15min put back: "TimeoutStartSec covers download,
// backup, restart and both health waits".
test("the unit runs the installed copy, outside the checkout, with time for the whole run", () => {
  const unit = read(UNIT);
  const exec = unit.match(/^ExecStart=\/bin\/bash (\S+)$/m);
  assert.ok(exec, "ExecStart runs the script through /bin/bash");
  assert.ok(!exec[1].startsWith("/opt/Humanity/"), `the unit runs a copy outside the checkout (${exec[1]})`);
  const def = read(FOLLOWER).match(/^INSTALLED_COPY="\$\{INSTALLED_COPY:-(\S+)\}"$/m);
  assert.ok(def, "the script names its install path");
  assert.strictEqual(exec[1], def[1], "the unit runs the copy the script installs");
  const limit = unit.match(/^TimeoutStartSec=(\d+)min$/m);
  assert.ok(limit && Number(limit[1]) >= 35, "TimeoutStartSec covers download, backup, restart and both health waits (about 35 minutes)");
});
