// The build leftovers in a server's target/ (scripts/clear-build-cache.sh, the disk guard, the
// deploys), 2026-11-06, found beside BUG-252.
//
// Run: node --test scripts/tests/build-leftovers.test.js (in `just rig-tests`).
//
// Since 2026-10-30 the relay is built on GitHub and the deploy only copies the finished program to
// /opt/Humanity/target/release/HumanityOS, the file the relay's unit starts. Everything else in
// target/ is what the server's own builds left behind: on 2026-11-06 united-humanity.us held 29 GB
// of it (deps/ alone 29 GB, build/ 220 MB, .fingerprint/, incremental/, a 175 MB rlib) and
// public.guide its own, because the deploy cleared it only at 80% disk and the disk guard at 88%.
// Nothing builds there any more, so they are cleared on every deploy and every disk-guard run,
// keeping the program and the deploy's markers beside it (HumanityOS.source and .started say what
// the program was built from and when it was last started; .incoming and its .sha256 are the next
// program, copied up before the deploy's step that swaps it in).
//
// This file runs the real scripts under bash over a scratch target/ laid out as the server's was,
// with stand-ins for df (the disk below every threshold), pgrep (no build running, or one), and
// logger, apt-get and journalctl (logged).
//
// Seen red 2026-11-06 against the scripts as they were (HOS_CLEAR_CACHE / HOS_DISK_GUARD pointed at
// HEAD's copies; the guard's REPO line pointed at the scratch folder by hand, since that copy had
// no setting for it): "the deploy's markers are kept" failed (HEAD's script deleted all of target/
// but the program: HumanityOS.source, .started and .incoming were gone, so the next deploy would
// rebuild and restart for nothing, and a program copied up for the swap was lost); and at 50% disk
// "the disk guard clears the leftovers below every threshold" failed (deps/ was still there). The
// deploy check failed on deploy.yml as it was: "the deploy clears the leftovers on every run".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const ROOT = path.join(__dirname, "..", "..");
const CLEAR = process.env.HOS_CLEAR_CACHE || path.join(ROOT, "scripts", "clear-build-cache.sh");
const GUARD = process.env.HOS_DISK_GUARD || path.join(ROOT, "scripts", "humanity-disk-guard.sh");
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
const lf = (s) => s.replace(/\r\n/g, "\n");

const worlds = [];
test.after(() => {
  for (const r of worlds) fs.rmSync(r, { recursive: true, force: true });
});

// What stays: the program and the deploy's markers.
const KEPT = ["HumanityOS", "HumanityOS.source", "HumanityOS.started", "HumanityOS.incoming", "HumanityOS.incoming.sha256"];

// A checkout whose target/ is laid out as united-humanity.us's was on 2026-11-06.
function makeServer() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum-leftovers-"));
  worlds.push(root);
  const w = { root, repo: path.join(root, "opt", "Humanity"), bin: path.join(root, "stubs"), log: path.join(root, "stub.log") };
  w.target = path.join(w.repo, "target");
  w.release = path.join(w.target, "release");
  const put = (rel, body = "x") => {
    const p = path.join(w.target, rel);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, body);
  };
  put("CACHEDIR.TAG");
  put(".rustc_info.json");
  put("debug/HumanityOS");
  put("release/HumanityOS", "the running relay");
  put("release/HumanityOS.source", "021d35b76a4a1ead2420e5d0b71d2239cf468be1\n");
  put("release/HumanityOS.started", "021d35b76a4a1ead2420e5d0b71d2239cf468be1\n");
  put("release/HumanityOS.incoming", "the next relay");
  put("release/HumanityOS.incoming.sha256", "abc\n");
  put("release/HumanityOS.d");
  put("release/libhumanity_engine.d");
  put("release/libhumanity_engine.rlib", "175 MB of rlib");
  put("release/.cargo-lock");
  put("release/.cargo-build-lock");
  put("release/.cargo-artifact-lock");
  put("release/deps/libserde-1234.rlib", "29 GB of these");
  put("release/build/ring-5678/out/lib.a");
  put("release/incremental/humanity_engine-9/s-1/dep-graph.bin");
  put("release/.fingerprint/serde-1234/lib-serde");
  fs.mkdirSync(path.join(w.release, "examples"));
  fs.mkdirSync(path.join(w.repo, "scripts"), { recursive: true });
  fs.writeFileSync(path.join(w.repo, "scripts", "clear-build-cache.sh"), lf(read(CLEAR)));
  fs.mkdirSync(w.bin);
  const stub = (name, body) => {
    fs.writeFileSync(path.join(w.bin, name), `#!/bin/bash\n${body}\n`);
    fs.chmodSync(path.join(w.bin, name), 0o755);
  };
  stub("df", 'echo "Use%"; echo " ${FAKE_DISK_PCT:-50}%"');
  stub("pgrep", '[ -n "${FAKE_BUILDING:-}" ] && [ "$2" = cargo ] && exit 0; exit 1');
  stub("logger", 'echo "logger $*" >> "$STUB_LOG"');
  stub("apt-get", 'echo "apt-get $*" >> "$STUB_LOG"');
  stub("journalctl", 'echo "journalctl $*" >> "$STUB_LOG"');
  return w;
}

function bash(w, script, env = {}) {
  const r = spawnSync(bashPath(), ["-c", `export PATH="$STUB_DIR:$PATH"; ${script}`], {
    encoding: "utf8",
    timeout: 60000,
    env: { ...process.env, STUB_DIR: px(w.bin), STUB_LOG: px(w.log), REPO: px(w.repo), ...env },
  });
  return { status: r.status, out: `${r.stdout || ""}${r.stderr || ""}` };
}
const clear = (w) => bash(w, 'exec bash "$REPO/scripts/clear-build-cache.sh" "$REPO"');
function guard(w, env = {}) {
  const copy = path.join(w.root, "disk-guard.sh");
  fs.writeFileSync(copy, lf(read(GUARD)));
  return bash(w, `exec bash "${px(copy)}"`, env);
}
const left = (w) => fs.readdirSync(w.release).sort();
const logLines = (w) => (fs.existsSync(w.log) ? read(w.log).split(/\r?\n/).filter(Boolean) : []);

test("the leftovers are cleared and the program and the deploy's markers are kept, untouched", () => {
  const w = makeServer();
  const bin = path.join(w.release, "HumanityOS");
  const ino = fs.statSync(bin).ino;
  const r = clear(w);
  assert.strictEqual(r.status, 0, r.out);
  assert.deepStrictEqual(left(w), KEPT.slice().sort(), `the deploy's markers are kept, and nothing else (left: ${left(w).join(", ")}; ${r.out})`);
  assert.deepStrictEqual(fs.readdirSync(w.target), ["release"], "nothing else in target/ either (debug/, CACHEDIR.TAG, .rustc_info.json)");
  assert.strictEqual(read(bin), "the running relay");
  assert.strictEqual(fs.statSync(bin).ino, ino, "the program the relay runs from is never moved or copied");
  assert.strictEqual(read(path.join(w.release, "HumanityOS.source")), "021d35b76a4a1ead2420e5d0b71d2239cf468be1\n");
  assert.strictEqual(read(path.join(w.release, "HumanityOS.incoming")), "the next relay");
  assert.match(r.out, /deps/, `it says what it removed: ${r.out}`);
  assert.match(r.out, /kept the relay program/);

  const again = clear(w);
  assert.strictEqual(again.status, 0, again.out);
  assert.match(again.out, /nothing to clear/, again.out);
  assert.deepStrictEqual(left(w), KEPT.slice().sort());

  // No target/ at all (a fresh node): nothing to do, and no failure.
  fs.rmSync(w.target, { recursive: true, force: true });
  assert.strictEqual(clear(w).status, 0);
});

test("the disk guard clears the leftovers on every run, below every threshold, but never under a running build", () => {
  const w = makeServer();
  const r = guard(w, { FAKE_DISK_PCT: "50" });
  assert.strictEqual(r.status, 0, r.out);
  assert.ok(!fs.existsSync(path.join(w.release, "deps")), `the disk guard clears the leftovers below every threshold (left: ${left(w).join(", ")}; log: ${logLines(w).join(" | ")})`);
  assert.deepStrictEqual(left(w), KEPT.slice().sort(), "the program and its markers are kept");
  assert.strictEqual(read(path.join(w.release, "HumanityOS")), "the running relay");
  assert.ok(logLines(w).some((l) => /BUILD LEFTOVERS/.test(l)), "the journal says what was cleared");

  // The next run, nothing left: nothing more in the journal about it.
  fs.writeFileSync(w.log, "");
  guard(w, { FAKE_DISK_PCT: "50" });
  assert.ok(!logLines(w).some((l) => /BUILD LEFTOVERS/.test(l)), "a run with nothing to clear says nothing about it");

  // `just sync` building on the server: its files are left alone until the build is done.
  const busy = makeServer();
  const b = guard(busy, { FAKE_BUILDING: "1" });
  assert.strictEqual(b.status, 0, b.out);
  assert.ok(fs.existsSync(path.join(busy.release, "deps")), "a running build's files are left alone");
  assert.ok(logLines(busy).some((l) => /a build is running/.test(l)), `and the journal says why (${logLines(busy).join(" | ")})`);
});

test("the deploy and the fallback deploy clear the leftovers on every run", () => {
  const deploy = lf(read(path.join(ROOT, ".github", "workflows", "deploy.yml")));
  assert.match(deploy, /^\s+bash scripts\/clear-build-cache\.sh \/opt\/Humanity/m, "the deploy runs the clear");
  assert.doesNotMatch(deploy, /-ge 80 \]; then[\s\S]{0,200}clear-build-cache/, "the deploy clears the leftovers on every run, not only at 80% disk");
  const just = read(path.join(ROOT, "Justfile"));
  const sync = just.match(/^sync:\n[\s\S]*?\n(?=\S)/m)[0];
  assert.doesNotMatch(sync, /-ge 80 \]; then[\s\S]{0,200}clear-build-cache/, "the fallback deploy does not wait for 80% disk");
  const build = sync.indexOf("cargo build");
  const after = sync.indexOf("clear-build-cache.sh", build);
  assert.ok(build > 0 && after > build, "the fallback deploy, which builds on the server, clears what its build left");
});
