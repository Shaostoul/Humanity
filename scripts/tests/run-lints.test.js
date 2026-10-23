// The lint runner (scripts/run-lints.js) and its compile cache (scripts/lib/rustc-cache.js).
//
// What would make it a check that cannot fail: a cached program from BEFORE a lint was edited
// being run after the edit, so a lint made red still reads green; a failure that does not set
// the exit code; or a second red lint hidden behind the first. Each is tried here on a
// throwaway checkout with three tiny lints compiled by the real rustc.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const RC = require("../lib/rustc-cache.js");

const RUNNER = path.join(__dirname, "..", "run-lints.js");

function checkout() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hum_run_lints_"));
  fs.mkdirSync(path.join(root, "tests"));
  return root;
}
const lint = (root, name, body) => fs.writeFileSync(path.join(root, "tests", `${name}.rs`), body);
const PASS = '#[test]\nfn fine() { assert_eq!(1 + 1, 2); }\n';
const FAIL = (why) => `#[test]\nfn red() { panic!("${why}"); }\n`;
// Reads the checkout through CARGO_MANIFEST_DIR at compile time, as every real lint does.
const READS_ROOT = '#[test]\nfn marker_present() { let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("marker.txt"); assert!(p.exists(), "no marker in {}", p.display()); }\n';

function run(root, ...names) {
  const r = spawnSync(process.execPath, [RUNNER, "--root", root, ...names], { encoding: "utf8" });
  return { status: r.status, out: (r.stdout || "") + (r.stderr || "") };
}

test("every lint runs and every failure is reported, with exit 1", () => {
  const root = checkout();
  try {
    lint(root, "a_lint", PASS);
    lint(root, "b_lint", FAIL("b is red"));
    lint(root, "c_lint", FAIL("c is red too"));
    const r = run(root, "a_lint", "b_lint", "c_lint");
    assert.strictEqual(r.status, 1, r.out);
    assert.match(r.out, />> lint ok: a_lint/);
    assert.match(r.out, /LINT FAILED: b_lint[\s\S]*b is red/);
    // The old loop stopped at b; c must still be run and named.
    assert.match(r.out, /LINT FAILED: c_lint[\s\S]*c is red too/);
    assert.match(r.out, /LINT FAILED: 2 lints: b_lint, c_lint/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a lint edited after it was cached is compiled again (its old green program is not run)", () => {
  const root = checkout();
  try {
    lint(root, "edit_lint", PASS);
    assert.strictEqual(run(root, "edit_lint").status, 0);
    assert.strictEqual(run(root, "edit_lint").status, 0, "second run, from the cache");
    lint(root, "edit_lint", FAIL("made red after caching"));
    const r = run(root, "edit_lint");
    assert.strictEqual(r.status, 1, "the edited lint must fail, not replay the cached pass:\n" + r.out);
    assert.match(r.out, /made red after caching/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("the checkout's path is part of the key: a program built for one checkout is not reused by another", () => {
  const a = checkout();
  const b = checkout();
  try {
    lint(a, "root_lint", READS_ROOT);
    lint(b, "root_lint", READS_ROOT);
    fs.writeFileSync(path.join(a, "marker.txt"), "here");
    assert.strictEqual(run(a, "root_lint").status, 0);
    // Same source, other checkout without the marker: must compile its own and fail.
    const r = run(b, "root_lint");
    assert.strictEqual(r.status, 1, r.out);
    assert.match(r.out, /no marker in/);
  } finally {
    fs.rmSync(a, { recursive: true, force: true });
    fs.rmSync(b, { recursive: true, force: true });
  }
});

test("a lint that does not compile fails with rustc's message", () => {
  const root = checkout();
  try {
    lint(root, "broken_lint", "#[test]\nfn x() { let y: u32 = \"no\"; }\n");
    const r = run(root, "broken_lint");
    assert.strictEqual(r.status, 1);
    assert.match(r.out, /LINT FAILED: broken_lint[\s\S]*did not compile[\s\S]*mismatched types/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("sources that pull in other files at compile time are never cached", () => {
  assert.match(RC.uncacheableReason('const X: &str = include_str!("data.txt");'), /include_str/);
  assert.match(RC.uncacheableReason("#[path = \"x.rs\"]\nmod x;"), /path/);
  assert.match(RC.uncacheableReason("mod helpers;\n"), /mod x;/);
  assert.strictEqual(RC.uncacheableReason("mod tests {\n}\n"), null);
  assert.strictEqual(RC.uncacheableReason(PASS), null);
});

test("the key changes with the toolchain, the arguments, the environment and the bytes", () => {
  const base = { srcBytes: Buffer.from(PASS), rustcV: "rustc 1.99.1", args: ["--test"], env: { CARGO_MANIFEST_DIR: "C:\\a" } };
  const k = RC.cacheKey(base);
  assert.notStrictEqual(RC.cacheKey({ ...base, rustcV: "rustc 1.88.0" }), k);
  assert.notStrictEqual(RC.cacheKey({ ...base, args: ["--test", "-O"] }), k);
  assert.notStrictEqual(RC.cacheKey({ ...base, env: { CARGO_MANIFEST_DIR: "C:\\b" } }), k);
  assert.notStrictEqual(RC.cacheKey({ ...base, srcBytes: Buffer.from(PASS + " ") }), k);
  assert.strictEqual(RC.cacheKey({ ...base }), k);
});
