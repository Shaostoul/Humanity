#!/usr/bin/env node
// `just verify`'s relay gate (build-speed review item 6): does ONE
//   cargo check --features relay --no-default-features --lib --bins --tests
// give the compile guarantee of the old pair (`cargo check` relay, then
// `cargo test --lib --no-run` relay)?
//
//   node scripts/build-ab/relay-gate.js
//
// Two injected failures, each tried against both gates: (1) a relay test module reaching for a
// native-only item (the class that went unseen until 2026-09-19, twenty errors of it), (2) a
// relay-only error in the binary. Then each gate's cost on a touched tree. Every source edit is
// undone byte for byte whatever happens. If both injections fail BOTH gates, the new check may
// replace the pair: `just verify` then runs `just verify-relay-compiles` in place of its separate
// relay `cargo check`, and that recipe's body becomes the one check. Results:
// target\build-ab\relay-gate.log.

"use strict";

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const T = require("./tools.js");

const wrapper = T.ensureWrapper();
const log = T.logger("relay-gate");
const env = { ...process.env, CARGO_TARGET_DIR: path.join(T.ROOT, "target", "ab-relay"), CARGO_BUILD_JOBS: "4", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper };
const R = ["--features", "relay", "--no-default-features"];
const OLD = [
  ["check", ...R],
  ["test", ...R, "--lib", "--no-run"],
];
const NEW = [["check", ...R, "--lib", "--bins", "--tests"]];

function gate(cmds) {
  const t0 = Date.now();
  for (const c of cmds) {
    const r = spawnSync(wrapper, ["--measure", "cargo", ...c], { cwd: T.ROOT, env, encoding: "utf8", maxBuffer: 1 << 30 });
    const meas = ((r.stderr || "").match(/\[measure\][^\n]*/g) || []).pop() || "";
    if (r.status !== 0) {
      const err = ((r.stderr || "").match(/^error(\[E\d+\])?: [^\n]*/m) || ["(no error line)"])[0];
      return `FAILED at \`cargo ${c.join(" ")}\`: ${err} (${((Date.now() - t0) / 1000).toFixed(0)} s)`;
    }
    if (cmds.length === 1 || c === cmds[cmds.length - 1]) return `passed (${((Date.now() - t0) / 1000).toFixed(0)} s; last ${meas})`;
  }
  return "passed";
}
function withEdit(file, transform, fn) {
  const f = path.join(T.ROOT, file);
  const orig = fs.readFileSync(f);
  fs.writeFileSync(f, transform(orig.toString("utf8")));
  try {
    return fn();
  } finally {
    fs.writeFileSync(f, orig);
  }
}

const l = T.load();
log(`=== load: ${l.procs} cargo+rustc, ${l.free} MB free`);
log(`warm-up, old pair: ${gate(OLD)}`);
log(`warm-up, new check: ${gate(NEW)}`);
const inject1 = (t) => t + "\n#[cfg(test)]\nmod buildspeed_injected_relay_test {\n    #[test]\n    fn reaches_native() {\n        let _ = crate::config::AppConfig::default();\n    }\n}\n";
withEdit("src/relay/mod.rs", inject1, () => {
  log(`inject 1 (a relay test module uses native-only crate::config), old pair: ${gate(OLD)}`);
  log(`inject 1, new check: ${gate(NEW)}`);
});
const inject2 = (t) => t + '\n#[cfg(not(feature = "native"))]\n#[allow(dead_code)]\nfn buildspeed_injected_bin_error() -> u32 {\n    "not a number"\n}\n';
withEdit("src/main.rs", inject2, () => {
  log(`inject 2 (a relay-only error in the binary), old pair: ${gate(OLD)}`);
  log(`inject 2, new check: ${gate(NEW)}`);
});
const touch = () => {
  const f = path.join(T.ROOT, "src", "relay", "mod.rs");
  const t = new Date();
  fs.utimesSync(f, t, t);
};
touch();
log(`cost after a touch, old pair: ${gate(OLD)}`);
touch();
log(`cost after a touch, new check: ${gate(NEW)}`);
