// Old builds of our crate in a target folder (scripts/lib/stale-artifacts.js) and the disk
// guard step that deletes them (scripts/pc-disk-guard.js step 0), 2026-10-23.
//
// What would make it a check that cannot fail, or worse, a cleaner that eats a live build: a
// rule that never finds anything (so "nothing to do" always passes), or one that deletes the
// build the next compile needs. So: a made-up target folder with old and current builds of our
// crate, a dependency and an incremental folder; the plan names exactly the old ones; the guard,
// run on that folder for real, deletes them and leaves every current file, the dependency and
// the newest builds of a crate nobody touched for a month.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const SA = require("../lib/stale-artifacts.js");

const DAY = SA.DAY;
const now = Date.now();

test("names: artifacts, fingerprint folders, incremental folders", () => {
  assert.deepStrictEqual(SA.parseDepName("libhumanity_engine-0123456789abcdef.rlib"), { crate: "humanity_engine", hash: "0123456789abcdef" });
  assert.deepStrictEqual(SA.parseDepName("humanity_engine-0123456789abcdef.exe"), { crate: "humanity_engine", hash: "0123456789abcdef" });
  assert.deepStrictEqual(SA.parseDepName("HumanityOS-fedcba9876543210.pdb"), { crate: "HumanityOS", hash: "fedcba9876543210" });
  assert.strictEqual(SA.parseDepName("readme.txt"), null);
  assert.strictEqual(SA.parseDepName("humanity_engine-123.rlib"), null, "a hash is 16 hex digits");
  assert.deepStrictEqual(SA.parseIncrementalDir("humanity_engine-1abc2def3gh4i"), { crate: "humanity_engine", id: "1abc2def3gh4i" });
});

test("the plan: old builds of our crate go; current ones, the newest few and dependencies stay", () => {
  const e = (kind, crate, key, ageDays, bytes = 100) => ({ kind, path: `${kind}/${crate}-${key}`, crate, key, mtimeMs: now - ageDays * DAY, bytes });
  const entries = [
    // our crate: 8 builds, the newest today, the oldest 40 days ago
    ...[0, 1, 2, 5, 9, 12, 20, 40].map((d, i) => e("deps", "humanity_engine", `h${i}`, d)),
    // a dependency with eight old copies (more than keep): never touched
    ...[30, 31, 32, 33, 34, 35, 36, 37].map((d, i) => e("deps", "wgpu", `w${i}`, d)),
    // incremental: two old sessions of our crate, one current
    e("incremental", "humanity_engine", "i0", 0),
    ...[10, 11, 12, 13, 14, 15, 16].map((d, i) => e("incremental", "humanity_engine", `i${i + 1}`, d)),
  ];
  const plan = SA.planPrune(entries, { crates: ["humanity_engine", "HumanityOS"], now, staleDays: 3, keep: 6 });
  const removed = plan.remove.map((x) => x.path).sort();
  // deps: the 6 newest kept (0,1,2,5,9,12 days), so only 20 and 40 days go
  assert.deepStrictEqual(removed.filter((p) => p.startsWith("deps/")), ["deps/humanity_engine-h6", "deps/humanity_engine-h7"]);
  // incremental: 8 folders, the 6 newest kept, the two oldest (15 and 16 days) go
  assert.deepStrictEqual(removed.filter((p) => p.startsWith("incremental/")), ["incremental/humanity_engine-i6", "incremental/humanity_engine-i7"]);
  assert.ok(!removed.some((p) => p.includes("wgpu")), "a dependency is never touched");
  // Recent but beyond the newest six: kept, because it is not yet stale.
  const recent = SA.planPrune(
    [0, 0.5, 1, 1.5, 2, 2.5, 2.9].map((d, i) => e("deps", "humanity_engine", `r${i}`, d)),
    { crates: ["humanity_engine"], now, staleDays: 3, keep: 6 }
  );
  assert.deepStrictEqual(recent.remove, [], "nothing written in the last 3 days is deleted");
  // A crate nobody built for a month keeps its newest six.
  const month = SA.planPrune(
    [30, 31, 32, 33, 34, 35, 36].map((d, i) => e("deps", "humanity_engine", `m${i}`, d)),
    { crates: ["humanity_engine"], now, staleDays: 3, keep: 6 }
  );
  assert.deepStrictEqual(month.remove.map((x) => x.key), ["m6"]);
});

/** A made-up checkout with a target folder; returns its paths. */
function fakeCheckout() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum_disk_guard_"));
  const deps = path.join(root, "target", "debug", "deps");
  const inc = path.join(root, "target", "debug", "incremental");
  fs.mkdirSync(deps, { recursive: true });
  fs.mkdirSync(inc, { recursive: true });
  fs.mkdirSync(path.join(root, "tests"));
  fs.writeFileSync(path.join(root, "tests", "emdash_lint.rs"), "");
  const put = (file, ageDays) => {
    fs.writeFileSync(file, "x".repeat(1000));
    const t = new Date(now - ageDays * DAY);
    fs.utimesSync(file, t, t);
  };
  // our crate: 8 builds of the test program (exe + pdb), the oldest 40 days
  const ages = [0, 1, 2, 5, 9, 12, 20, 40];
  ages.forEach((d, i) => {
    const h = String(i).repeat(16);
    put(path.join(deps, `humanity_engine-${h}.exe`), d);
    put(path.join(deps, `humanity_engine-${h}.pdb`), d);
  });
  // an integration test, two builds, both old: the newest six (both) stay
  put(path.join(deps, `emdash_lint-${"a".repeat(16)}.exe`), 50);
  put(path.join(deps, `emdash_lint-${"b".repeat(16)}.exe`), 60);
  // a dependency with eight very old copies (more than the six a crate keeps): all stay
  for (let i = 0; i < 8; i++) put(path.join(deps, `libwgpu-${String.fromCharCode(99 + i).repeat(16)}.rlib`), 90 + i);
  return { root, deps, inc, ages };
}

test("the disk guard deletes the old builds for real, beside a 'running build', and nothing else", () => {
  const { root, deps } = fakeCheckout();
  try {
    const guard = path.join(__dirname, "..", "pc-disk-guard.js");
    // A busy machine (the guard's own test hook names a process that is always running), so
    // steps 1 to 3 are skipped: step 0 must run anyway.
    const env = { ...process.env, PC_DISK_GUARD_BUSY: "System,cargo.exe", PC_DISK_GUARD_WARN_PCT: "101", PC_DISK_GUARD_CRIT_PCT: "101" };
    const dry = spawnSync(process.execPath, [guard, "--root", root, "--dry-run", "--no-log"], { encoding: "utf8", env });
    assert.match(dry.stdout, /would delete 4 old build file\(s\) of our crate/, dry.stdout + dry.stderr);
    assert.strictEqual(fs.readdirSync(deps).length, 26, "a dry run deletes nothing");
    const real = spawnSync(process.execPath, [guard, "--root", root, "--no-log"], { encoding: "utf8", env });
    assert.match(real.stdout, /deleted 4 old build file\(s\) of our crate/, real.stdout + real.stderr);
    assert.match(real.stdout, /busy: /, "steps 1 to 3 were skipped as busy, step 0 ran anyway");
    const left = fs.readdirSync(deps).sort();
    assert.ok(!left.includes(`humanity_engine-${"6".repeat(16)}.exe`) && !left.includes(`humanity_engine-${"7".repeat(16)}.pdb`), "the two oldest builds (20 and 40 days) are gone");
    assert.ok(left.includes(`humanity_engine-${"0".repeat(16)}.exe`), "today's build stays");
    assert.ok(left.includes(`humanity_engine-${"5".repeat(16)}.pdb`), "the sixth newest (12 days) stays");
    assert.ok(left.includes(`emdash_lint-${"b".repeat(16)}.exe`), "a crate's newest six stay however old");
    assert.strictEqual(left.filter((f) => f.startsWith("libwgpu-")).length, 8, "a dependency is never touched");
    assert.strictEqual(left.length, 22);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
