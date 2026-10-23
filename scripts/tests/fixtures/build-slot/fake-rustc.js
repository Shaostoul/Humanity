// A stand-in for rustc that behaves like one compile of our crate on a full machine
// (scripts/tests/build-slot.test.js). The build slot runs it as `node fake-rustc.js <rustc args>`.
//
//   FAKE_RUSTC_DIR   a folder shared by every copy started in one test
//   FAKE_RUSTC_MS    how long the "compile" takes (default 1200)
//   FAKE_RUSTC_MODE  "crowd" (default): a copy that finds another one running dies the way rustc
//                    dies out of memory ("memory allocation of N bytes failed", exit 0xC0000409),
//                    so two at once is fatal, as two 10 GB compiles were on the 40 GB machine.
//                    "oom-once": the first run dies of memory, later runs succeed.
//                    "type-error": prints one rustc error diagnostic and exits 1 (not memory).
//
// Each run appends "start <ms> <pid>" and "end <ms> <pid> <ok|died>" to <dir>/runs.log.

"use strict";
const fs = require("fs");
const path = require("path");

const dir = process.env.FAKE_RUSTC_DIR;
const ms = Number(process.env.FAKE_RUSTC_MS || 1200);
const mode = process.env.FAKE_RUSTC_MODE || "crowd";
const runs = path.join(dir, "runs.log");
const note = (m) => fs.appendFileSync(runs, `${m} ${Date.now()} ${process.pid}\n`);
const sleep = (t) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, t);
const OOM_EXIT = -1073740791; // 0xC0000409

note("start");
if (mode === "type-error") {
  process.stderr.write('{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308"},"level":"error","spans":[],"children":[],"rendered":"error[E0308]: mismatched types\\n"}\n');
  note("end-failed");
  process.exit(1);
}
if (mode === "oom-once") {
  const marker = path.join(dir, "oom-once.done");
  if (!fs.existsSync(marker)) {
    fs.writeFileSync(marker, "");
    process.stderr.write("LLVM ERROR: out of memory\n");
    note("end-died");
    process.exit(OOM_EXIT);
  }
  sleep(ms / 4);
  note("end-ok");
  process.exit(0);
}
// crowd
const running = path.join(dir, "running");
try {
  fs.mkdirSync(running);
} catch {
  process.stderr.write("memory allocation of 1073741824 bytes failed\n");
  note("end-died");
  process.exit(OOM_EXIT);
}
// an artifact line, the way rustc tells cargo the metadata is ready (must pass straight through)
process.stderr.write('{"$message_type":"artifact","artifact":"libfake.rmeta","emit":"metadata"}\n');
sleep(ms);
fs.rmdirSync(running);
note("end-ok");
process.exit(0);
