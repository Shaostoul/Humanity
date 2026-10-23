// The target seeder (scripts/seed-target.js, 2026-10-23), on a made-up warm target folder.
//
// What would make it harmful rather than useless: copying OUR crate's files (a worktree can
// never reuse them, and they are the bulk), copying into a folder that already has builds,
// copying a folder another toolchain built, or losing the timestamps cargo compares. Each is
// checked here with real files and the real robocopy.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const SEED = path.join(__dirname, "..", "seed-target.js");
const windows = process.platform === "win32";

function rustcRelease() {
  const v = spawnSync("rustc", ["-vV"], { cwd: path.join(__dirname, "..", ".."), encoding: "utf8" }).stdout || "";
  return (v.match(/^release: (.+)$/m) || [])[1];
}

function warmTarget(root, release) {
  const t = path.join(root, "warm");
  const d = path.join(t, "debug");
  for (const sub of ["deps", ".fingerprint/wgpu-0123456789abcdef", ".fingerprint/humanity-engine-fedcba9876543210", "build/libsqlite3-sys-1111111111111111/out", "build/humanity-engine-2222222222222222/out", "incremental/humanity_engine-abc123"]) {
    fs.mkdirSync(path.join(d, sub), { recursive: true });
  }
  const old = new Date(Date.now() - 5 * 24 * 3600 * 1000);
  const put = (rel, body = "x") => {
    const f = path.join(d, rel);
    fs.writeFileSync(f, body);
    fs.utimesSync(f, old, old);
  };
  put("deps/libwgpu-0123456789abcdef.rlib");
  put("deps/libwgpu-0123456789abcdef.rmeta");
  put("deps/libhumanity_engine-fedcba9876543210.rlib");
  put("deps/humanity_engine-fedcba9876543210.exe");
  put("deps/HumanityOS-3333333333333333.exe");
  put("deps/emdash_lint-4444444444444444.exe");
  put(".fingerprint/wgpu-0123456789abcdef/lib-wgpu");
  put(".fingerprint/humanity-engine-fedcba9876543210/lib-humanity_engine");
  put("build/libsqlite3-sys-1111111111111111/out/sqlite3.o");
  put("build/humanity-engine-2222222222222222/out/stamp");
  put("incremental/humanity_engine-abc123/session");
  fs.writeFileSync(path.join(t, ".rustc_info.json"), JSON.stringify({ outputs: { x: { stdout: `rustc 1.99.1\nrelease: ${release}\n` } } }));
  return { t, d, old };
}

test("dependencies are copied with their timestamps; our crate, incremental and its unit folders are not", { skip: !windows && "robocopy is Windows" }, () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum_seed_"));
  try {
    const { t, old } = warmTarget(root, rustcRelease());
    const to = path.join(root, "fresh");
    const r = spawnSync(process.execPath, [SEED, "--from", t, "--to", to], { encoding: "utf8" });
    assert.strictEqual(r.status, 0, r.stdout + r.stderr);
    const d = path.join(to, "debug");
    assert.ok(fs.existsSync(path.join(d, "deps", "libwgpu-0123456789abcdef.rlib")), "a dependency is copied");
    assert.ok(fs.existsSync(path.join(d, ".fingerprint", "wgpu-0123456789abcdef", "lib-wgpu")), "its fingerprint too");
    assert.ok(fs.existsSync(path.join(d, "build", "libsqlite3-sys-1111111111111111", "out", "sqlite3.o")), "and its build script's output");
    for (const ours of ["libhumanity_engine-fedcba9876543210.rlib", "humanity_engine-fedcba9876543210.exe", "HumanityOS-3333333333333333.exe", "emdash_lint-4444444444444444.exe"]) {
      assert.ok(!fs.existsSync(path.join(d, "deps", ours)), `our crate's ${ours} is not copied`);
    }
    assert.ok(!fs.existsSync(path.join(d, ".fingerprint", "humanity-engine-fedcba9876543210")), "nor our crate's fingerprint");
    assert.ok(!fs.existsSync(path.join(d, "build", "humanity-engine-2222222222222222")), "nor our build script's output");
    assert.ok(!fs.existsSync(path.join(d, "incremental")), "incremental is not copied");
    const m = fs.statSync(path.join(d, "deps", "libwgpu-0123456789abcdef.rlib")).mtimeMs;
    assert.ok(Math.abs(m - old.getTime()) < 2000, "the timestamp is kept (cargo compares them)");
    // A second seed into the same folder is refused: it holds builds now.
    const again = spawnSync(process.execPath, [SEED, "--from", t, "--to", to], { encoding: "utf8" });
    assert.strictEqual(again.status, 1);
    assert.match(again.stderr, /already holds .* build file\(s\)/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a folder another toolchain built is refused", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum_seed_"));
  try {
    const { t } = warmTarget(root, "1.0.0");
    const r = spawnSync(process.execPath, [SEED, "--from", t, "--to", path.join(root, "fresh")], { encoding: "utf8" });
    assert.strictEqual(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stderr, /was built by rustc 1\.0\.0; this checkout uses/);
    assert.ok(!fs.existsSync(path.join(root, "fresh", "debug", "deps")), "nothing copied");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
