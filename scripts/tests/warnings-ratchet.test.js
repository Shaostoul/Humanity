// The Rust warnings ratchet (scripts/warnings-ratchet.js, 2026-11-01).
//
// What would make it a check that cannot fail: a hidden-warnings flag it does not see (so a 0 read
// through RUSTFLAGS=-Awarnings passes, the exact state this PC was in for months), a warning it
// does not count (another package's, rustc's own "N warnings emitted" line, a duplicate from the
// lib's test build counted twice or not at all), or an over-budget count that does not fail. Each
// is tried here on cargo-shaped JSON and made-up environments; the real cargo run was seen red on
// purpose when the ratchet landed (an unused import, docs/ai/builds.md).
//
// Run: node --test scripts/tests/warnings-ratchet.test.js

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const path = require("path");
const R = require("../warnings-ratchet.js");

const ROOT = path.resolve(__dirname, "..", "..");
const MANIFEST = path.join(ROOT, "Cargo.toml");
const OTHER = path.join(ROOT, "vendor", "rav1d", "Cargo.toml");

/** A cargo `compiler-message` line. */
function msg({ level = "warning", text = "unused import: `x`", code = "unused_imports", file = "src/a.rs", line = 3, col = 5, manifest = MANIFEST, spans } = {}) {
  return JSON.stringify({
    reason: "compiler-message",
    package_id: "path+file:///x#humanity-engine@0.1.0",
    manifest_path: manifest,
    target: { kind: ["lib"], name: "humanity_engine" },
    message: {
      level,
      message: text,
      code: code ? { code, explanation: null } : null,
      spans: spans || [{ file_name: file, line_start: line, column_start: col, is_primary: true }],
      rendered: `${level}: ${text}\n --> ${file}:${line}:${col}\n`,
    },
  });
}

test("a hidden-warnings flag in any place cargo reads flags from is found", () => {
  const none = () => null;
  const cases = [
    [{ RUSTFLAGS: "-Awarnings" }, "RUSTFLAGS: -Awarnings"],
    [{ RUSTFLAGS: "-C target-cpu=native -A warnings" }, "RUSTFLAGS: -A warnings"],
    [{ RUSTFLAGS: "--cap-lints allow" }, "RUSTFLAGS: --cap-lints allow"],
    [{ RUSTFLAGS: "--cap-lints=allow" }, "RUSTFLAGS: --cap-lints=allow"],
    [{ RUSTFLAGS: "-Aunused_imports" }, "RUSTFLAGS: -Aunused_imports"],
    [{ CARGO_ENCODED_RUSTFLAGS: "-C\x1fopt-level=1\x1f-Awarnings" }, "CARGO_ENCODED_RUSTFLAGS: -Awarnings"],
    [{ CARGO_BUILD_RUSTFLAGS: "--allow=warnings" }, "CARGO_BUILD_RUSTFLAGS: --allow=warnings"],
    [{ CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: "-Awarnings" }, "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: -Awarnings"],
  ];
  for (const [env, want] of cases) {
    assert.deepStrictEqual(R.hiddenBy(env, { readFile: none }), [want], JSON.stringify(env));
  }
});

test("flags that do not hide warnings are left alone", () => {
  const none = () => null;
  for (const env of [{}, { RUSTFLAGS: "" }, { RUSTFLAGS: "-C target-cpu=native" }, { RUSTFLAGS: "-D warnings" }, { RUSTFLAGS: "-W unused" }]) {
    assert.deepStrictEqual(R.hiddenBy(env, { readFile: none }), [], JSON.stringify(env));
  }
});

test("a config file's rustflags, a crate-wide allow and a manifest allow are found", () => {
  const cfg = path.join(ROOT, ".cargo", "config.toml");
  const files = {
    [cfg]: '[build]\njobs = 4\n# rustflags = ["-Awarnings"]  (a comment, not a flag)\nrustflags = ["-C", "target-cpu=native", "-Awarnings"]\n[target.x86_64-pc-windows-msvc]\nrustflags = "--cap-lints allow"\n',
    [path.join(ROOT, "src", "lib.rs")]: "//! doc\n#![allow(dead_code, warnings)]\n",
    [path.join(ROOT, "Cargo.toml")]: '[package]\nname = "x"\n\n[lints.rust]\nwarnings = "allow"\n',
  };
  const found = R.hiddenBy({}, { readFile: (f) => (f in files ? files[f] : null) });
  assert.deepStrictEqual(found, [
    `${cfg}: -Awarnings`,
    `${cfg}: --cap-lints allow`,
    "src/lib.rs: #![allow(warnings)]",
    'Cargo.toml: [lints.rust] warnings = "allow"',
  ]);
  // The commented-out line alone is not a flag.
  assert.deepStrictEqual(R.configRustflags('# rustflags = ["-Awarnings"]\n'), []);
  // The build slot's real config (no rustflags) is clean.
  assert.deepStrictEqual(R.configRustflags('[build]\nrustc-workspace-wrapper = ".cargo/hos-build-slot.exe"\njobs = 4\n'), []);
});

test("this checkout's own sources hide nothing", () => {
  // The repo-side half only (src/lib.rs, src/main.rs, Cargo.toml); the environment is the machine's.
  const repoOnly = R.hiddenBy({}, { cwd: path.parse(ROOT).root }).filter((f) => !/[\\/]\.cargo[\\/]/.test(f));
  assert.deepStrictEqual(repoOnly, []);
});

test("our warnings are counted once each; other packages, summaries and errors are not", () => {
  const lines = [
    msg({}),
    msg({}), // the same warning again, from the lib's test build
    msg({ text: "unused variable: `y`", code: "unused_variables", line: 9 }),
    msg({ manifest: OTHER, text: "function pointer comparisons do not produce meaningful results", code: "unpredictable_function_pointer_comparisons" }),
    msg({ text: "2 warnings emitted", code: null, spans: [] }),
    msg({ level: "error", text: "cannot find value `z`", code: "E0425" }),
    '{"reason":"compiler-artifact","package_id":"x"}',
    "not json at all",
    "",
  ];
  const { ours, others, rendered } = R.countLines(lines, { manifest: MANIFEST, pkgName: "humanity-engine" });
  assert.strictEqual(ours.size, 2);
  assert.strictEqual(others.size, 1);
  // Every distinct diagnostic is printed, the error included; the summary line is not.
  assert.strictEqual(rendered.length, 4);
  assert.ok(!rendered.some((t) => /warnings emitted/.test(t)));
});

test("the same lint at two places is two warnings", () => {
  const { ours } = R.countLines([msg({ line: 3 }), msg({ line: 4 })], { manifest: MANIFEST, pkgName: "humanity-engine" });
  assert.strictEqual(ours.size, 2);
});

test("a message without a manifest path is placed by its package id", () => {
  const line = JSON.parse(msg({}));
  delete line.manifest_path;
  const theirs = { ...line, package_id: "registry+https://github.com/rust-lang/crates.io-index#egui@0.31.1" };
  const { ours, others } = R.countLines([JSON.stringify(line), JSON.stringify({ ...theirs, message: { ...line.message, line_start: 7, message: "other" } })], { manifest: MANIFEST, pkgName: "humanity-engine" });
  assert.strictEqual(ours.size, 1);
  assert.strictEqual(others.size, 1);
});

test("over the budget fails, at or under passes, under asks for the budget to come down", () => {
  const two = R.countLines([msg({ line: 1 }), msg({ line: 2 })], { manifest: MANIFEST, pkgName: "humanity-engine" });
  const over = R.judge("native", two.ours, two.others, { native: 1, relay: 0 });
  assert.strictEqual(over.ok, false);
  assert.match(over.lines.join("\n"), /FAILED: 2 Rust warning\(s\) in the native build, over its budget of 1/);
  assert.match(over.lines.join("\n"), /unused_imports/);
  const at = R.judge("native", two.ours, two.others, { native: 2 });
  assert.strictEqual(at.ok, true);
  assert.match(at.lines.join("\n"), />> warnings ok \(native\): 2, budget 2/);
  const under = R.judge("native", two.ours, two.others, { native: 5 });
  assert.strictEqual(under.ok, true);
  assert.match(under.lines.join("\n"), /lower "native" .* to 2/);
  const missing = R.judge("relay", two.ours, two.others, { native: 5 });
  assert.strictEqual(missing.ok, false);
});

test("the budget file holds a number for each check, and the checks are the full target set", () => {
  const budget = require("../warnings-budget.json");
  for (const k of Object.keys(R.CHECKS)) {
    assert.strictEqual(typeof budget[k], "number", `budget for ${k}`);
    for (const t of ["--lib", "--bins", "--tests"]) assert.ok(R.CHECKS[k].includes(t), `${k} checks ${t}`);
  }
});
