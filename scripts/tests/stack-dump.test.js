// The stack dumper (scripts/stackdump/hos-stackdump.rs, scripts/lib/stack-dump.js, 2026-11-14).
//
// What would make it a check that cannot fail: a dump that names threads but whose frames are
// never resolved to functions (a report that cannot say WHERE), a dump that leaves the program
// suspended or dead, or one that never looks at a real stuck thread. So a real test program,
// compiled with its debug file beside it (the way cargo leaves the lib-test programs), hangs on
// purpose (scripts/tests/fixtures/lib-tests/tiny_suite.rs, TINY_SILENT_HANG: a thread-local whose
// destructor never returns), and its dump must name the stuck test's thread AND show it asleep in
// that destructor's thread-exit cleanup, by function name and source line. The program must still
// be running afterwards (every thread resumed).
// RED, 2026-11-14: before scripts/lib/stack-dump.js existed, require() of it failed; with the
// dumper's StackWalk64 loop removed, the thread had no frames and the frame match failed.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn, spawnSync } = require("child_process");
const SD = require("../lib/stack-dump.js");

const SUITE = path.join(__dirname, "fixtures", "lib-tests", "tiny_suite.rs");
const supported = process.platform === "win32" && process.arch === "x64";

test("a hung test program's dump names the stuck test's thread and where it is stuck", { skip: !supported && "x86-64 Windows only" }, async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_stackdump_"));
  let child = null;
  try {
    const exe = path.join(dir, "tiny_suite.exe");
    // Built in place, so its debug file (tiny_suite.pdb) sits beside it.
    const b = spawnSync("rustc", ["--test", "--edition", "2021", "-A", "warnings", SUITE, "-o", exe], { encoding: "utf8", windowsHide: true });
    assert.strictEqual(b.status, 0, b.stderr);
    assert.ok(fs.existsSync(path.join(dir, "tiny_suite.pdb")), "the debug file is beside the program");
    child = spawn(exe, ["hangs_silently_when_asked", "--exact"], { env: { ...process.env, TINY_SILENT_HANG: "1" }, stdio: "ignore", windowsHide: true });
    // It reports its result, then its thread sleeps forever in the thread-local's destructor.
    await new Promise((r) => setTimeout(r, 1500));
    assert.strictEqual(child.exitCode, null, "the test program is hung, not finished");
    const out = path.join(dir, "dump.stacks.txt");
    const d = SD.dumpStacks(child.pid, out);
    assert.ok(d.ok, d.error);
    const text = fs.readFileSync(out, "utf8");
    assert.match(text, new RegExp(`^hos-stackdump: process ${child.pid} \\(.*tiny_suite\\.exe\\), \\d+ thread\\(s\\)$`, "m"));
    // The stuck thread by its test's name, asleep (by function and source line) inside the
    // thread-exit cleanup that runs thread-locals' destructors.
    const block = /^thread \d+ "hangs_silently_when_asked":\n((?:    .*\n?)+)/m.exec(text);
    assert.ok(block, text);
    assert.match(block[1], /tiny_suite\.exe!std::sys::thread::windows::sleep\+0x[0-9a-f]+  \(.*:\d+\)/, block[1]);
    assert.match(block[1], /thread_local/, block[1]);
    // The main thread is named, waiting to join it.
    assert.match(text, /^thread \d+ "main":\n(    .*\n)*    .*Thread::join/m);
    // Every thread was resumed: the program is still alive, as it was.
    await new Promise((r) => setTimeout(r, 300));
    assert.strictEqual(child.exitCode, null, "the dump left the program running");
  } finally {
    if (child) child.kill();
    await new Promise((r) => setTimeout(r, 300));
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a process that cannot be opened gives a file that says so, not a crash", { skip: !supported && "x86-64 Windows only" }, () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_stackdump_none_"));
  try {
    // Process ids are multiples of 4 on Windows: 3 is never one.
    const d = SD.dumpStacks(3, path.join(dir, "x.stacks.txt"));
    assert.strictEqual(d.ok, false);
    assert.match(fs.readFileSync(d.file, "utf8"), /the stack dump did not finish: exit 1: hos-stackdump: cannot open process 3/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
