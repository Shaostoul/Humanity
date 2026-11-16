// The build slot (scripts/build-slot/hos-build-slot.rs, scripts/build-slot.js).
//
// What would make it a check that cannot fail: compiles that "queue" only because they never
// overlapped anyway, a retry that is never exercised by a real out-of-memory death, or a health
// check that calls a missing or stale wrapper fine. So:
//   - three compiles started at once, each of which DIES if another runs beside it (a stand-in
//     rustc, scripts/tests/fixtures/build-slot/fake-rustc.js), all succeed through the slot,
//     one after another; with the slot switched off (HUMANITY_BUILD_SLOT=off) the same three
//     die: the red control, run every time, proving the stand-in can tell
//   - a REAL rustc, run by REAL cargo on a tiny crate, capped by a job object on its first
//     attempt so it really runs out of memory, is retried to success; with one attempt allowed
//     the same build FAILS (the red control)
//   - an out-of-memory death retried, an ordinary error not retried and shown
//   - the health check: not installed, missing, stale, a config it did not write, current
// Every test uses a slot folder of its own (HOS_BUILD_SLOT_DIR) and fake memory figures
// (HOS_BUILD_SLOT_FAKE_MEM_FILE), so none of them waits on, or is waited on by, a real build.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn, spawnSync } = require("child_process");
const RC = require("../lib/rustc-cache.js");
const BS = require("../build-slot.js");

const REPO = path.resolve(__dirname, "..", "..");
const SRC = path.join(REPO, "scripts", "build-slot", "hos-build-slot.rs");
const FAKE = path.join(__dirname, "fixtures", "build-slot", "fake-rustc.js");
const CACHE = path.join(REPO, "target", "test-cache");
const windows = process.platform === "win32";

/** The wrapper, built once per content of its source (cached like the lints). */
function wrapper() {
  const p = RC.plan({
    src: SRC,
    name: "hos-build-slot",
    cacheDir: CACHE,
    args: ["--edition", "2021", "-O", "-A", "warnings", "-C", "debuginfo=0"],
    env: { HOS_BUILD_SLOT_SRC_SHA256: BS.sourceHash(SRC) },
    cwd: REPO,
  });
  const b = RC.buildSync(p);
  assert.ok(b.ok, `the wrapper did not compile:\n${b.stderr}`);
  return b.exe;
}

function tmp(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `hum_build_slot_${prefix}_`));
}

/** Environment for one test: its own slot folder and fake memory. */
function slotEnv(dir, extra = {}) {
  const mem = path.join(dir, "mem.txt");
  if (!fs.existsSync(mem)) fs.writeFileSync(mem, "9000 9000");
  return {
    ...process.env,
    HOS_BUILD_SLOT_DIR: path.join(dir, "slot"),
    HOS_BUILD_SLOT_FAKE_MEM_FILE: mem,
    HOS_BUILD_SLOT_POLL_MS: "100",
    HUMANITY_BUILD_SLOT: "",
    ...extra,
  };
}

/** Start the wrapper running the stand-in rustc as a compile of our crate. */
function startFake(exe, env) {
  return new Promise((resolve) => {
    const c = spawn(exe, [process.execPath, FAKE, "--crate-name", "humanity_engine", "--emit=dep-info,link", "--test", "--cfg", 'feature="native"'], { env, windowsHide: true });
    let out = "";
    let err = "";
    c.stdout.on("data", (d) => (out += d));
    c.stderr.on("data", (d) => (err += d));
    c.on("close", (code) => resolve({ code, out, err }));
  });
}

function runsOf(dir) {
  const text = fs.existsSync(path.join(dir, "runs.log")) ? fs.readFileSync(path.join(dir, "runs.log"), "utf8") : "";
  return text.split("\n").filter(Boolean).map((l) => {
    const [what, at, pid] = l.split(" ");
    return { what, at: +at, pid: +pid };
  });
}

test("the wrapper's own unit tests pass (rustc --test on its source)", () => {
  const p = RC.plan({ src: SRC, name: "hos-build-slot-tests", cacheDir: CACHE, args: ["--test", "--edition", "2021", "-A", "warnings"], env: {}, cwd: REPO });
  const b = RC.buildSync(p);
  assert.ok(b.ok, b.stderr);
  const r = spawnSync(b.exe, [], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /test result: ok/);
});

test("compiles started together take turns through the slot; with it off they die", { skip: !windows && "job objects and process checks are Windows" }, async () => {
  const exe = wrapper();
  // ON: 9 GB free, each needs 4 GB + 2 GB margin, so only one fits at a time.
  const on = tmp("queue_on");
  try {
    const env = slotEnv(on, { FAKE_RUSTC_DIR: on, HOS_BUILD_SLOT_PEAK_MB: "4000", HOS_BUILD_SLOT_ATTEMPTS: "1" });
    const results = await Promise.all([startFake(exe, env), startFake(exe, env), startFake(exe, env)]);
    for (const r of results) assert.strictEqual(r.code, 0, `a compile died through the slot:\n${r.out}\n${r.err}`);
    const runs = runsOf(on);
    assert.strictEqual(runs.filter((x) => x.what === "end-ok").length, 3, JSON.stringify(runs));
    // One after another: every start comes after the previous run's end.
    const starts = runs.filter((x) => x.what === "start").sort((a, b) => a.at - b.at);
    const ends = runs.filter((x) => x.what === "end-ok").sort((a, b) => a.at - b.at);
    for (let i = 1; i < starts.length; i++) assert.ok(starts[i].at >= ends[i - 1].at, `run ${i} started before run ${i - 1} ended: ${JSON.stringify(runs)}`);
    const said = results.map((r) => r.out).join("\n");
    assert.match(said, /WAITING for memory/, "a compile that queued says so");
    assert.match(said, /room for .* after waiting/);
    // The artifact line reached the wrapper's stderr (cargo needs it as it comes).
    assert.ok(results.every((r) => r.err.includes('"$message_type":"artifact"')), "artifact lines pass through");
    // The slot's records are gone afterwards.
    assert.deepStrictEqual(fs.readdirSync(path.join(on, "slot")).filter((f) => /^(held|wait)-/.test(f)), []);
  } finally {
    fs.rmSync(on, { recursive: true, force: true });
  }
  // OFF: the same three at once, and the stand-in kills the crowd. This is what makes the pass
  // above mean something.
  const off = tmp("queue_off");
  try {
    const env = slotEnv(off, { FAKE_RUSTC_DIR: off, HUMANITY_BUILD_SLOT: "off" });
    const results = await Promise.all([startFake(exe, env), startFake(exe, env), startFake(exe, env)]);
    const died = results.filter((r) => r.code !== 0);
    assert.ok(died.length >= 1, "with the slot off, compiles started together must collide (else this test proves nothing)");
    assert.ok(died.every((r) => r.err.includes("memory allocation of")), "they died the out-of-memory way");
  } finally {
    fs.rmSync(off, { recursive: true, force: true });
  }
});

// The 2026-11-14 deadlock through the real wrapper: a head that needs 14.4 GB + 2 GB margin with
// 16.3 GB free and none of ours running. The stand-in kills a copy that runs beside another, so a
// rule that let the compiles behind it start beside it fails here too.
// RED on the rule before the fix (its wrapper built from the parent commit's source, 2026-11-14):
// nothing started; after 30 s the head was still "WAITING for memory: needs 14.1 GB + 2.0 GB
// margin" with "Running: none of ours", and the two behind it waited with it.
test("with none of ours running, a head the full margin does not fit starts, and the line follows", { skip: !windows && "process checks are Windows" }, async () => {
  const exe = wrapper();
  const d = tmp("idle_head");
  const kids = [];
  const startTracked = (env) => {
    const p = new Promise((resolve) => {
      const c = spawn(exe, [process.execPath, FAKE, "--crate-name", "humanity_engine", "--emit=dep-info,link", "--test", "--cfg", 'feature="native"'], { env, windowsHide: true });
      kids.push(c);
      let out = "";
      let err = "";
      c.stdout.on("data", (x) => (out += x));
      c.stderr.on("data", (x) => (err += x));
      c.on("close", (code) => resolve({ code, out, err }));
    });
    return p;
  };
  const killer = setTimeout(() => kids.forEach((c) => c.kill()), 30_000);
  try {
    fs.writeFileSync(path.join(d, "mem.txt"), "16300 16300");
    const base = { FAKE_RUSTC_DIR: d, FAKE_RUSTC_MS: "800", HOS_BUILD_SLOT_ATTEMPTS: "1" };
    const head = startTracked(slotEnv(d, { ...base, HOS_BUILD_SLOT_PEAK_MB: "14400" }));
    // The head is first in line: it has decided (held or waiting) before the others arrive.
    const t0 = Date.now();
    while (!fs.existsSync(path.join(d, "slot")) || !fs.readdirSync(path.join(d, "slot")).some((f) => /^(held|wait)-/.test(f))) {
      assert.ok(Date.now() - t0 < 10_000, "the head never decided");
      await new Promise((r) => setTimeout(r, 50));
    }
    // Two behind it, each 8 GB: one fits at a time, and neither beside the head.
    const behind = [startTracked(slotEnv(d, { ...base, HOS_BUILD_SLOT_PEAK_MB: "8000" })), startTracked(slotEnv(d, { ...base, HOS_BUILD_SLOT_PEAK_MB: "8000" }))];
    const results = await Promise.all([head, ...behind]);
    const said = results.map((r) => r.out).join("\n");
    for (const r of results) assert.strictEqual(r.code, 0, `a compile failed or was stopped after 30 s:\n${said}`);
    assert.match(results[0].out, /none of our compiles is running, so it starts with the idle margin \(1\.0 GB\)/);
    const runs = runsOf(d);
    const starts = runs.filter((x) => x.what === "start").sort((a, b) => a.at - b.at);
    const ends = runs.filter((x) => x.what === "end-ok").sort((a, b) => a.at - b.at);
    assert.strictEqual(ends.length, 3, JSON.stringify(runs));
    for (let i = 1; i < starts.length; i++) assert.ok(starts[i].at >= ends[i - 1].at, `run ${i} started beside another: ${JSON.stringify(runs)}`);
  } finally {
    clearTimeout(killer);
    fs.rmSync(d, { recursive: true, force: true });
  }
});

test("an out-of-memory death is retried; an ordinary error is not, and is shown", { skip: !windows && "Windows" }, async () => {
  const exe = wrapper();
  const d = tmp("retry");
  try {
    const env = slotEnv(d, { FAKE_RUSTC_DIR: d, FAKE_RUSTC_MODE: "oom-once", HOS_BUILD_SLOT_PEAK_MB: "1000" });
    const r = await startFake(exe, env);
    assert.strictEqual(r.code, 0, r.out + r.err);
    assert.match(r.out, /OUT OF MEMORY on attempt 1 of 3: "LLVM ERROR: out of memory"/);
    assert.match(r.out, /attempt 2 SUCCEEDED/);
    assert.ok(!r.err.includes("LLVM ERROR"), "the dead attempt's error is not passed on to cargo");
    assert.match(fs.readFileSync(path.join(d, "slot", "oom-retries.txt"), "utf8"), /attempt=1 .*LLVM ERROR/);
  } finally {
    fs.rmSync(d, { recursive: true, force: true });
  }
  const e = tmp("typeerror");
  try {
    const env = slotEnv(e, { FAKE_RUSTC_DIR: e, FAKE_RUSTC_MODE: "type-error", HOS_BUILD_SLOT_PEAK_MB: "1000" });
    const r = await startFake(exe, env);
    assert.strictEqual(r.code, 1);
    assert.match(r.err, /mismatched types/, "the held-back error reaches cargo");
    assert.ok(!/OUT OF MEMORY/.test(r.out), "not retried");
    assert.strictEqual(runsOf(e).filter((x) => x.what === "start").length, 1);
  } finally {
    fs.rmSync(e, { recursive: true, force: true });
  }
});

// The physical need is learned from the whole tree's working set (2026-11-14): rustc's own leaves
// out the linker it starts at the end. A bin crate links, so its tree is rustc AND the linker.
// RED with the tree watch switched off (TreeWatch::finish returning 0, so the ledger fell back to
// rustc's own working set), 2026-11-14: "the tree's working set (73 MB) counts the linker beside
// rustc's own (73 MB)" failed; with the watch on, 133 MB against 70 MB.
//
// IT FAILED UNDER LOAD, AND THE WATCH WAS WRONG (2026-11-15): twice in loaded runs, passing alone.
// The watch only LISTED the job's processes every 200 ms, and this tiny bin's whole link takes less
// than that, so whenever the link fell between two listings it was never seen: reproduced the same
// day, "working set 77 MB (rustc alone 77 MB)" in 1 of 5 builds beside another compile, and 1 of 10
// on a quiet machine. The watch now opens each process the moment the job's notice says it joined
// (TreeWatch in hos-build-slot.rs), and this test stretches the listing to 10 minutes
// (HOS_BUILD_SLOT_TREE_LIST_MS), so only the notices can count the linker: the event itself, not a
// race between a link and a poll. 10 of 10 builds counted it (134 MB against 74 MB). RED with the
// notices switched off (no port, the listing alone), 3 of 3 builds: "working set 76 MB (rustc alone
// 76 MB)"; and RED again with TreeWatch::finish returning 0 (the evidence above), 2026-11-15: "the
// tree's working set (75 MB) counts the linker beside rustc's own (75 MB)".
test("real cargo, real rustc: a linked crate's ledger line counts the linker's working set too", { skip: !windows && "job objects are Windows" }, () => {
  const exe = wrapper();
  const d = tmp("cargo_ws");
  try {
    const crate = path.join(d, "tinybin");
    fs.mkdirSync(path.join(crate, "src"), { recursive: true });
    fs.writeFileSync(path.join(crate, "Cargo.toml"), '[package]\nname = "tinybin"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n');
    fs.writeFileSync(path.join(crate, "src", "main.rs"), 'fn main() {\n    let v: Vec<String> = (0..1000).map(|i| format!("{i}")).collect();\n    println!("{}", v.len());\n}\n');
    const env = slotEnv(d, { CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: exe, CARGO_TARGET_DIR: path.join(d, "t"), HOS_BUILD_SLOT_TREE_LIST_MS: "600000" });
    const r = spawnSync("cargo", ["build"], { cwd: crate, encoding: "utf8", env });
    assert.strictEqual(r.status, 0, r.stdout + r.stderr);
    const log = fs.readFileSync(path.join(d, "slot", "build-slot.log"), "utf8");
    const m = /done tinybin \(other-build-none\) .* working set (\d+) MB \(rustc alone (\d+) MB\)/.exec(log);
    assert.ok(m, log);
    const [tree, own] = [Number(m[1]), Number(m[2])];
    assert.ok(own > 0 && tree > own, `the tree's working set (${tree} MB) counts the linker beside rustc's own (${own} MB)`);
    const line = fs.readFileSync(path.join(d, "slot", "peaks.txt"), "utf8").trim().split(" ");
    assert.strictEqual(Number(line[4]), tree, "the ledger keeps the tree's working set");
  } finally {
    fs.rmSync(d, { recursive: true, force: true });
  }
});

test("real cargo, real rustc: a compile capped into running out of memory is retried to success", { skip: !windows && "job objects are Windows" }, () => {
  const exe = wrapper();
  const d = tmp("cargo_oom");
  try {
    const crate = path.join(d, "tiny");
    fs.mkdirSync(path.join(crate, "src"), { recursive: true });
    fs.writeFileSync(path.join(crate, "Cargo.toml"), '[package]\nname = "tiny"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n');
    fs.writeFileSync(path.join(crate, "src", "lib.rs"), "pub fn add(a: u32, b: u32) -> u32 { a + b }\n");
    const build = (attempts, target) =>
      spawnSync("cargo", ["build"], {
        cwd: crate,
        encoding: "utf8",
        env: slotEnv(d, {
          CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: exe,
          CARGO_TARGET_DIR: path.join(d, target),
          HOS_BUILD_SLOT_PEAK_MB: "500",
          HOS_BUILD_SLOT_TEST_OOM_MB: "40",
          HOS_BUILD_SLOT_ATTEMPTS: String(attempts),
        }),
      });
    // Red control: one attempt, and the capped rustc's death is the build's.
    const red = build(1, "t1");
    assert.notStrictEqual(red.status, 0, "a rustc capped at 40 MB must fail, or this test proves nothing:\n" + red.stdout + red.stderr);
    assert.match(red.stderr + red.stdout, /E0786|os error 1455|memory allocation of/);
    // Through the slot with retries: the same build succeeds.
    const green = build(3, "t3");
    assert.strictEqual(green.status, 0, green.stdout + green.stderr);
    assert.match(green.stdout, /OUT OF MEMORY on attempt 1 of 3/);
    assert.match(green.stdout, /attempt 2 SUCCEEDED/);
  } finally {
    fs.rmSync(d, { recursive: true, force: true });
  }
});

test("the health check names a missing, stale or foreign wrapper and says the fix", () => {
  const exe = wrapper();
  const root = tmp("health");
  try {
    const P = BS.paths(root);
    fs.mkdirSync(path.dirname(P.src), { recursive: true });
    fs.copyFileSync(SRC, P.src);
    let v = BS.verdict(BS.inspect(root));
    assert.strictEqual(v.state, "not-installed");
    fs.mkdirSync(P.dir, { recursive: true });
    fs.writeFileSync(P.config, "[build]\njobs = 4\n");
    v = BS.verdict(BS.inspect(root));
    assert.strictEqual(v.state, "foreign-config");
    fs.writeFileSync(P.config, BS.configText());
    v = BS.verdict(BS.inspect(root));
    assert.strictEqual(v.state, "missing");
    assert.match(v.lines.join("\n"), /BUILD SLOT MISSING[\s\S]*just build-slot/);
    fs.copyFileSync(exe, P.exe);
    v = BS.verdict(BS.inspect(root));
    assert.strictEqual(v.state, "ok", v.lines.join("\n"));
    fs.appendFileSync(P.src, "\n// a change to the source after the exe was built\n");
    v = BS.verdict(BS.inspect(root));
    assert.strictEqual(v.state, "stale");
    assert.match(v.lines.join("\n"), /BUILD SLOT STALE[\s\S]*just build-slot/);
    // Line endings alone are not a change (a CRLF checkout and an LF one agree).
    const lf = fs.readFileSync(SRC, "utf8").replace(/\r\n/g, "\n");
    fs.writeFileSync(P.src, lf.replace(/\n/g, "\r\n"));
    assert.strictEqual(BS.verdict(BS.inspect(root)).state, "ok");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
