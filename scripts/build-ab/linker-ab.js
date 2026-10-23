#!/usr/bin/env node
// A/B: Rust's own linker (rust-lld) against Microsoft's link.exe (build-speed review item 3).
//
//   node scripts/build-ab/linker-ab.js a|b [rounds]      (after ab-debug.js built that variant)
//
// On the library's test program in target\ab-<variant>. Each step reruns rustc for our crate
// with only the linker setting changed; incremental compilation reuses everything, so the
// difference is the link (time-passes' link_binary row). Then BUG-174's question: does the test
// program link WITH its debug file (no /DEBUG:NONE) under each linker? Adopting rust-lld also
// needs the RELEASE exe built with it to boot: build it with
//   CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld.exe cargo build --features native --release
// and run `just verify-runtime` on it. Results: target\build-ab\linker-ab-<variant>.log.

"use strict";

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const T = require("./tools.js");

const variant = process.argv[2] || "b";
const rounds = Number(process.argv[3] || 2);
const wrapper = T.ensureWrapper();
const log = T.logger(`linker-ab-${variant}`);
const targetDir = path.join(T.ROOT, "target", `ab-${variant}`);
const env = { ...process.env, RUSTC_BOOTSTRAP: "1", CARGO_TARGET_DIR: targetDir, CARGO_BUILD_JOBS: "4", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper };

function run(label, extra) {
  const t0 = Date.now();
  const l = T.load();
  const r = spawnSync("cargo", [...T.VARIANTS[variant], "rustc", "--features", "native", "--lib", "--profile", "test", "--", ...extra, "-Z", "time-passes"], {
    cwd: T.ROOT,
    env,
    encoding: "utf8",
    maxBuffer: 1 << 30,
  });
  const text = (r.stderr || "") + (r.stdout || "");
  const pass = (name) => Math.max(0, ...[...text.matchAll(new RegExp(`time:\\s+([\\d.]+);[^\\n]*\\s${name}\\s*$`, "gm"))].map((m) => Number(m[1])));
  const err = (text.match(/LNK\d+[^\n]*|error: linking with[^\n]*|lld-link: error[^\n]*/g) || []).slice(0, 3).join(" | ");
  const deps = path.join(targetDir, "debug", "deps");
  const newest = (ext) =>
    fs
      .readdirSync(deps)
      .filter((f) => /^humanity_engine-[0-9a-f]{16}\./.test(f) && f.endsWith(ext))
      .map((f) => ({ f, st: fs.statSync(path.join(deps, f)) }))
      .sort((a, b) => b.st.mtimeMs - a.st.mtimeMs)[0];
  const fresh = (x) => x && x.st.mtimeMs >= t0 - 1000;
  const exe = newest(".exe");
  const pdb = newest(".pdb");
  log(
    `[${variant}] ${label}: exit ${r.status}, wall ${((Date.now() - t0) / 1000).toFixed(0)} s, link_binary ${pass("link_binary").toFixed(2)} s, ` +
      `rustc total ${pass("total").toFixed(1)} s, exe ${fresh(exe) ? (exe.st.size / 1e6).toFixed(0) + " MB" : "(not written)"}, ` +
      `pdb ${fresh(pdb) ? (pdb.st.size / 1e6).toFixed(0) + " MB" : "(none)"}${err ? `, ERROR ${err}` : ""} (load: ${l.procs} cargo+rustc, ${l.free} MB free)`
  );
}

log(`=== ${(spawnSync("rustc", ["-V"], { cwd: T.ROOT, encoding: "utf8" }).stdout || "").trim()}`);
for (let i = 1; i <= rounds; i++) {
  run(`round ${i}: link.exe, /DEBUG:NONE (today's lib-tests)`, ["-C", "link-arg=/DEBUG:NONE"]);
  run(`round ${i}: rust-lld, /DEBUG:NONE`, ["-C", "linker=rust-lld.exe", "-C", "link-arg=/DEBUG:NONE"]);
}
run("BUG-174: link.exe WITH its debug file", []);
run("BUG-174: rust-lld WITH its debug file", ["-C", "linker=rust-lld.exe"]);
