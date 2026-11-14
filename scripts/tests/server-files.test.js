// The files a server runs from outside its checkout (scripts/server-files.txt) and the script
// that installs them (scripts/install-server-files.sh), BUG-252, 2026-11-06.
//
// Run: node --test scripts/tests/server-files.test.js (in `just rig-tests`).
//
// public.guide restarted every 30 minutes because /usr/local/bin/humanity-backup-db was a copy
// installed by hand in August: the release follower updated the checkout and never that copy,
// and the deploy refreshed united-humanity.us's with a plain `cp -f`. This file runs the real
// installer under bash into a scratch tree (INSTALL_ROOT), from this checkout's own files, with
// stand-ins for install and systemctl that log what they were asked; and it reads the units, the
// deploy and the fallback deploy to check that every copy outside the checkout is in the list and
// goes through the installer.
//
// Each case was seen red once, against a deliberately broken copy of the installer
// (HOS_INSTALLER pointed at it) or, for the deploy and unit checks, against the files as they were
// before the fix; its comment says which and what the red run showed (Windows, 2026-11-06).

const test = require("node:test");
const assert = require("node:assert");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const ROOT = path.join(__dirname, "..", "..");
const INSTALLER = process.env.HOS_INSTALLER || path.join(ROOT, "scripts", "install-server-files.sh");
const MANIFEST = path.join(ROOT, "scripts", "server-files.txt");
const WINDOWS = process.platform === "win32";
// HOS_SERVER_FILES_LIB: a deliberately broken copy of the list's reader, for the red runs.
const lib = require(process.env.HOS_SERVER_FILES_LIB || "../lib/server-files.js");
const { parseManifest } = lib;

// Git for Windows' bash, not the WSL launcher in System32 that a bare "bash" can resolve to.
function bashPath() {
  if (!WINDOWS) return "bash";
  for (const p of ["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"]) {
    if (fs.existsSync(p)) return p;
  }
  return "bash";
}
// A path as bash sees it (C:\x\y is /c/x/y under Git Bash).
const px = (p) => (WINDOWS ? p.replace(/\\/g, "/").replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`) : p);
const read = (p) => fs.readFileSync(p, "utf8");
const md5 = (p) => crypto.createHash("md5").update(fs.readFileSync(p)).digest("hex");
const lf = (s) => s.replace(/\r\n/g, "\n");

const entries = parseManifest(read(MANIFEST)).files;

// A scratch server: an empty root to install into, and stand-ins for install (logged, then run)
// and systemctl (logged only).
const worlds = [];
function makeServer() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum-srvfiles-"));
  worlds.push(root);
  const w = { root, sys: path.join(root, "sys"), bin: path.join(root, "stubs"), log: path.join(root, "stub.log") };
  fs.mkdirSync(w.sys);
  fs.mkdirSync(w.bin);
  const stub = (name, body) => {
    fs.writeFileSync(path.join(w.bin, name), `#!/bin/bash\n${body}\n`);
    fs.chmodSync(path.join(w.bin, name), 0o755);
  };
  stub("systemctl", 'echo "systemctl $*" >> "$STUB_LOG"\nexit 0');
  stub("install", 'echo "install $*" >> "$STUB_LOG"\nreal="$(type -P -a install | grep -vF "$STUB_DIR/" | head -n 1)"\nexec "$real" "$@"');
  return w;
}
test.after(() => {
  for (const r of worlds) fs.rmSync(r, { recursive: true, force: true });
});

function run(w, role, env = {}) {
  // The installer is run the way the deploy and the follower run it: `bash <file> <repo> <role>`.
  // A CR in a copy checked out on Windows would stop bash, so a CRLF copy is run as LF.
  const lfCopy = path.join(w.root, "install-server-files.sh");
  fs.writeFileSync(lfCopy, lf(read(INSTALLER)));
  // HANG_GUARD_MS is a guard against a hung installer, never a speed check: nothing here asserts how
  // long a run takes. It was 60 s until 2026-11-14, when one run took 25 to 35 s with other worktrees
  // compiling (Git Bash starts every `install`, `cmp` and `md5sum` as a whole emulated process) and a
  // busier machine passed 60 s: spawnSync killed bash, status came back null and the test failed with
  // nothing wrong in the installer. 300 s still ends a real hang, and says so in words. Red evidence,
  // 2026-11-14: a copy of the installer that sleeps first (HOS_INSTALLER, with HOS_HANG_GUARD_MS=5000
  // so the red run is quick) fails "the installer did not finish within 5 s (a hang)"; the role
  // filter taken out still fails "the follower is never installed on the main server" (below).
  const HANG_GUARD_MS = Number(process.env.HOS_HANG_GUARD_MS) || 300000;
  const r = spawnSync(bashPath(), ["-c", 'export PATH="$STUB_DIR:$PATH"; exec bash "$INSTALLER" "$REPO_DIR" "$ROLE"'], {
    encoding: "utf8",
    timeout: HANG_GUARD_MS,
    env: { ...process.env, STUB_DIR: px(w.bin), STUB_LOG: px(w.log), INSTALL_ROOT: px(w.sys), INSTALLER: px(lfCopy), REPO_DIR: px(ROOT), ROLE: role, ...env },
  });
  const hung = r.error && r.error.code === "ETIMEDOUT" ? `\nthe installer did not finish within ${HANG_GUARD_MS / 1000} s (a hang)` : "";
  return { status: r.status, out: `${r.stdout || ""}${r.stderr || ""}${hung}` };
}
const logLines = (w) => (fs.existsSync(w.log) ? read(w.log).split(/\r?\n/).filter(Boolean) : []);
const at = (w, dest) => path.join(w.sys, ...dest.split("/").filter(Boolean));

// Seen red with the role filter taken out of the installer (`if [ "$role" != all ] ...; continue`
// deleted): "the follower is never installed on the main server" failed, the main role had
// installed /usr/local/lib/humanity/relay-follow-releases.sh and the follow units.
test("an empty main server gets every line for its role, and none of the follow node's", () => {
  const w = makeServer();
  const r = run(w, "main");
  assert.strictEqual(r.status, 0, r.out);
  for (const e of entries) {
    const there = fs.existsSync(at(w, e.dest));
    if (e.role === "follow") {
      assert.ok(!there, `the follower is never installed on the main server: ${e.dest}`);
    } else {
      assert.ok(there, `${e.dest} is installed`);
      assert.strictEqual(md5(at(w, e.dest)), md5(path.join(ROOT, e.src)), `${e.dest} is the repo's copy, byte for byte`);
      assert.match(r.out, new RegExp(`installed ${e.dest.replace(/[.]/g, "\\.")} \\(there was none\\)`));
    }
  }
  assert.deepStrictEqual(logLines(w).filter((l) => l.startsWith("systemctl")), ["systemctl daemon-reload"], "units changed: one reload");
  assert.match(r.out, /0 failed \(role main\)/);
});

// Seen red with `"$to.new"` replaced by `"$to"` in the install line and the mv taken out (an
// in-place write, as the old deploy's `cp -f` was): "installed beside the live file, then renamed
// over it" failed on the first install line, which named the live path.
test("a follow node gets its own lines too, each installed beside the live file and renamed over it", () => {
  const w = makeServer();
  const r = run(w, "follow");
  assert.strictEqual(r.status, 0, r.out);
  for (const e of entries) {
    assert.ok(fs.existsSync(at(w, e.dest)), `${e.dest} is installed on a follow node`);
    assert.ok(!fs.existsSync(`${at(w, e.dest)}.new`), `no ${e.dest}.new is left behind`);
  }
  const installs = logLines(w).filter((l) => l.startsWith("install "));
  assert.strictEqual(installs.length, entries.length, "one install per line");
  for (const l of installs) {
    assert.match(l, /^install -m [0-7]{3} \S+ \S+\.new$/, `installed beside the live file, then renamed over it: ${l}`);
  }
  const follower = entries.find((e) => e.src === "scripts/relay-follow-releases.sh");
  assert.ok(follower && follower.mode === "755", "the follower is installed executable");
  if (!WINDOWS) {
    for (const e of entries) {
      assert.strictEqual(fs.statSync(at(w, e.dest)).mode & 0o777, parseInt(e.mode, 8), `${e.dest} has mode ${e.mode}`);
    }
  }
});

// Public.guide's case, the reason for all of this. Seen red with the `cmp -s` check inverted
// (`! cmp -s`): "the stale backup script is replaced" failed (the August copy's md5 was still
// there); and with the `was=` line emptied: "says what it replaced" failed (the line read
// "installed /usr/local/bin/humanity-backup-db (there was none)").
test("a stale copy is replaced and the line says what it replaced; a current one is left alone", () => {
  const w = makeServer();
  assert.strictEqual(run(w, "follow").status, 0);
  const backup = at(w, "/usr/local/bin/humanity-backup-db");
  // The August copy: other bytes, an August date.
  fs.writeFileSync(backup, "#!/bin/bash\nsqlite3 /opt/Humanity/data/relay.db \".backup '$OUT'\"\n");
  const aug = new Date("2026-08-14T05:47:00Z");
  fs.utimesSync(backup, aug, aug);
  const staleMd5 = md5(backup).slice(0, 8);
  const unit = at(w, "/etc/systemd/system/humanity-backup-db.timer");
  const unitIno = fs.statSync(unit).ino;
  fs.writeFileSync(w.log, "");

  const r = run(w, "follow");
  assert.strictEqual(r.status, 0, r.out);
  assert.strictEqual(md5(backup), md5(path.join(ROOT, "scripts", "humanity-backup-db.sh")), "the stale backup script is replaced");
  const repoMd5 = md5(path.join(ROOT, "scripts", "humanity-backup-db.sh")).slice(0, 8);
  assert.match(
    r.out,
    new RegExp(`replaced /usr/local/bin/humanity-backup-db \\(md5 ${staleMd5}, dated 2026-08-14\\) with this release's copy \\(md5 ${repoMd5}\\)`),
    "says what it replaced",
  );
  assert.match(r.out, new RegExp(`1 installed or replaced, ${entries.length - 1} already the release's copy, 0 failed`));
  assert.strictEqual(fs.statSync(unit).ino, unitIno, "a unit that is already current is not rewritten");
  assert.deepStrictEqual(logLines(w).filter((l) => l.startsWith("systemctl")), [], "no unit changed: no reload");
  assert.deepStrictEqual(logLines(w).filter((l) => l.startsWith("install ")).length, 1, "one file installed");

  // The next run: nothing to do.
  const again = run(w, "follow");
  assert.match(again.out, new RegExp(`0 installed or replaced, ${entries.length} already the release's copy, 0 failed`));
});

// Seen red with the `[ ! -f "$from" ]` check taken out: install failed on the missing source
// ("exits 0": 1) and the run said "could not install". The list here has no newline after its
// last line: seen red with the loop's `|| [ -n "$kind" ]` taken out (the line was never read, and
// the run said nothing about it).
test("a release without one of the files leaves the installed copy as it is", () => {
  const w = makeServer();
  const list = path.join(w.root, "server-files.txt");
  fs.writeFileSync(list, "file all 755 scripts/no-such-script.sh /usr/local/bin/no-such-script");
  const kept = at(w, "/usr/local/bin/no-such-script");
  fs.mkdirSync(path.dirname(kept), { recursive: true });
  fs.writeFileSync(kept, "the node's own copy\n");
  const r = run(w, "main", { MANIFEST: px(list) });
  assert.strictEqual(r.status, 0, `exits 0: ${r.out}`);
  assert.match(r.out, /this release has no scripts\/no-such-script\.sh; \/usr\/local\/bin\/no-such-script is left as it is/);
  assert.strictEqual(read(kept), "the node's own copy\n");
});

// Seen red with the path and mode checks taken out of the line check: the run exited 0 having
// "installed usr/local/bin/relative (there was none)", a relative path under the current folder.
test("a malformed line is refused, named, and fails the run; the role must be main or follow", () => {
  const w = makeServer();
  const list = path.join(w.root, "server-files.txt");
  fs.writeFileSync(list, "file all 755 scripts/humanity-backup-db.sh usr/local/bin/relative\n");
  const r = run(w, "main", { MANIFEST: px(list) });
  assert.strictEqual(r.status, 1, r.out);
  assert.match(r.out, /does not read as/);
  assert.strictEqual(run(w, "everything").status, 2);
});

// The list against the units: every path a unit in scripts/systemd/ runs from outside the
// checkout, and every unit file itself, is a line, so no copy can be left for nothing to refresh.
// Seen red with the humanity-backup-db line taken out of the list: "a unit runs
// /usr/local/bin/humanity-backup-db, which is not in scripts/server-files.txt".
test("every unit file, and every file a unit runs from outside the checkout, is in the list", () => {
  const dests = new Set(entries.map((e) => e.dest));
  const sources = new Set(entries.map((e) => e.src));
  const unitDir = path.join(ROOT, "scripts", "systemd");
  for (const f of fs.readdirSync(unitDir)) {
    assert.ok(sources.has(`scripts/systemd/${f}`), `scripts/systemd/${f} is in scripts/server-files.txt`);
    const exec = read(path.join(unitDir, f)).match(/^ExecStart=(?:\/bin\/bash\s+)?(\S+)/m);
    if (!exec || exec[1].startsWith("/opt/Humanity/")) continue;
    assert.ok(dests.has(exec[1]), `a unit runs ${exec[1]}, which is not in scripts/server-files.txt`);
  }
  for (const e of entries) assert.ok(fs.existsSync(path.join(ROOT, e.src)), `${e.src} exists`);
  // The follower names its own install path; the unit runs that copy.
  const follower = read(path.join(ROOT, "scripts", "relay-follow-releases.sh"));
  const def = follower.match(/^INSTALLED_COPY="\$\{INSTALLED_COPY:-(\S+)\}"$/m);
  assert.ok(def && dests.has(def[1]), "the follower's INSTALLED_COPY is the list's line for it");
  const list = parseManifest(read(MANIFEST));
  assert.deepStrictEqual(list.servers.map((s) => s.role).sort(), ["follow", "main"], "both servers are listed with their roles");
  assert.ok(read(path.join(ROOT, ".gitattributes")).includes("scripts/server-files.txt text eol=lf"), "the list stays LF for bash");
});

// Seen red against deploy.yml and the Justfile before the fix: "the deploy installs server files
// through the installer" (it copied the backup script with `cp -f` into /usr/local/bin and the
// units into /etc/systemd/system, and never the backup script's own units).
test("the deploy, the fallback deploy and the follower install through the installer, never with cp", () => {
  const deploy = read(path.join(ROOT, ".github", "workflows", "deploy.yml"));
  assert.match(deploy, /bash scripts\/install-server-files\.sh \/opt\/Humanity main/, "the deploy installs server files through the installer");
  assert.doesNotMatch(deploy, /cp -f \S+ \/(usr\/local|etc\/systemd)\//, "the deploy copies nothing into /usr/local or /etc/systemd by hand");
  const just = read(path.join(ROOT, "Justfile"));
  const sync = just.match(/^sync:\n[\s\S]*?\n(?=\S)/m);
  assert.ok(sync, "the Justfile has a sync recipe");
  assert.match(sync[0], /bash scripts\/install-server-files\.sh \/opt\/Humanity main/, "the fallback deploy installs through the installer");
  assert.doesNotMatch(sync[0], /cp -f \S+ \/(usr\/local|etc\/systemd)\//, "the fallback deploy copies nothing by hand");
  const follower = read(path.join(ROOT, "scripts", "relay-follow-releases.sh"));
  assert.match(follower, /bash "\$REPO\/scripts\/install-server-files\.sh" "\$REPO" follow/, "the follower runs the release's installer");
});

// `just brief`'s SCRIPTS row (scripts/check-server-files.js), from what a server prints. The
// remote output below is public.guide's as read on 2026-11-06 (its backup script the August copy,
// md5 8286827d; the repo's 46f8cdf0). Seen red with judgeServer comparing only whether a copy
// exists (`installed !== repo` taken out): the stale server was judged fine (ok true, expected
// false), its row would have read "all 11 installed files match".
test("the brief's row names a stale or missing copy on a server, and says so when all match", () => {
  const { parseRemote, judgeServer, remoteScript, filesFor, SERVER_REPO } = lib;
  const repo = (src) => md5(path.join(ROOT, src));
  const guide = { alias: "humanity-vps2", role: "follow", name: "public.guide" };
  const head = "2edc3fae7046b97137b2ce220a32a57d3a9b5314";
  const printed = (override) =>
    [`HEAD ${head}`, "TAG v0.1549.0"]
      .concat(
        filesFor(entries, guide.role).flatMap((e) => {
          const installed = e.dest in override ? override[e.dest] : repo(e.src);
          return [installed ? `${installed}  ${e.dest}` : `MISSING ${e.dest}`, `${repo(e.src)}  ${SERVER_REPO}/${e.src}`];
        }),
      )
      .join("\n");
  const n = filesFor(entries, "follow").length;

  const stale = judgeServer(guide, entries, parseRemote(printed({ "/usr/local/bin/humanity-backup-db": "8286827deb350f5655593647ce55bbc3" })), repo);
  assert.strictEqual(stale.ok, false);
  const want = repo("scripts/humanity-backup-db.sh").slice(0, 8);
  assert.ok(
    stale.line.startsWith(`STALE: public.guide: 1 of ${n} installed files differ from v0.1549.0's copies: /usr/local/bin/humanity-backup-db (8286827d; the repo's ${want});`),
    `public.guide's stale backup script is named: ${stale.line}`,
  );
  assert.match(stale.line, /the follower installs them within the hour/);

  const missing = judgeServer(guide, entries, parseRemote(printed({ "/etc/systemd/system/humanity-relay-follow.timer": null })), repo);
  assert.match(missing.line, /\/etc\/systemd\/system\/humanity-relay-follow\.timer \(missing; /, missing.line);

  const fine = judgeServer(guide, entries, parseRemote(printed({})), repo);
  assert.deepStrictEqual([fine.ok, fine.line], [true, `public.guide: all ${n} installed files match v0.1549.0's copies`]);

  // The main server checks its own lines only (no follower there), and with the commit not in the
  // local repo the server checkout's copies are the reference.
  const main = { alias: "humanity-vps", role: "main", name: "united-humanity.us" };
  const mainPrinted = printed({}).replace("TAG v0.1549.0", "TAG ");
  const noCommit = judgeServer(main, entries, parseRemote(mainPrinted), () => null);
  assert.deepStrictEqual(
    [noCommit.ok, noCommit.line],
    [true, `united-humanity.us: all ${filesFor(entries, "main").length} installed files match the copies in its checkout (${head.slice(0, 8)})`],
  );

  // What runs on the server only reads.
  const script = remoteScript(filesFor(entries, "follow"));
  assert.doesNotMatch(script, /\b(rm|mv|cp|install|tee|chmod|chown|systemctl|sed -i)\b/, "the ssh call changes nothing");
  assert.doesNotMatch(script.replace(/2>\/dev\/null/g, ""), />/, "and writes no file");
  assert.match(script, /safe\.directory=\/opt\/Humanity/, "git reads the relay user's checkout as root");
  assert.match(read(path.join(ROOT, "scripts", "brief.js")), /check-server-files\.js/, "just brief runs the check");
});
