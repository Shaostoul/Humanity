// The request writer every rig uses (scripts/lib/write-request.js): a request file is never
// there half-written.
//
// Run: node --test scripts/tests/write-request.test.js   (in `just rig-tests`)
//
// THE RACE. A reader thread does what the game does every frame: read the request if it is
// there, delete it, carry on. Meanwhile this thread writes the request, waits for the reader to
// take it, and writes it again: 300 times at the size verify-falls lost, 100 at 4 KB. Every
// read the reader makes must be the WHOLE request. A read of anything else (empty, or the first
// part of it) is the failure this module exists for: on 2026-10-20 one verify-falls run's
// `illness` showcase request was read empty and deleted, and its planet Forgiving leg ran in
// Realistic with no error anywhere.
//
// The race can fail, and the second test proves it every run: the same reader, against a plain
// fs.writeFileSync 100 times, catches the file empty (about 7 reads in 10 on this machine). If
// that control ever stops catching anything, the first test is no longer evidence and says so.
//
// RED FIRST (2026-10-20). With writeRequest's body swapped for a plain
// `fs.writeFileSync(target, text); return target;`, `node --test` on this file said (exit 1):
//   ✖ a reader racing the writer sees nothing or the whole request, never part of it
//     AssertionError [ERR_ASSERTION]: a 23-byte request was read part-written 236 times in 300
//     writes (read lengths: 0, 0, 0, 0, 0, 0, 0, 0). The game deletes a request once it has read
//     it, so each of those requests was lost.   236 !== 0
//   ✔ the same reader catches a plain fs.writeFileSync half-written (so the race above can fail)
//   ✖ a rename Windows refuses for a moment is retried, then put in place      0 !== 3
//   ✖ a rename that keeps failing removes its side file and says the game never got it
//     AssertionError [ERR_ASSERTION]: Missing expected exception.
//   ℹ tests 7   ℹ pass 4   ℹ fail 3
// and with the real body back, all seven pass.

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { Worker } = require("node:worker_threads");

const WR = require("../lib/write-request.js");

// The reader: the game's per-frame poll, in a thread of its own so it truly races the writer.
// Reads the request (no file is the honest "nothing yet"), records whether it was the whole
// request, deletes it (as every game request reader does once it has read one).
const READER = `
const { workerData, parentPort } = require("node:worker_threads");
const fs = require("node:fs");
const flag = new Int32Array(workerData.flag);
let whole = 0, torn = 0;
const lens = [];
Atomics.store(flag, 1, 1); // running
while (Atomics.load(flag, 0) === 0) {
  let text;
  try { text = fs.readFileSync(workerData.target, "utf8"); } catch { continue; }
  if (text === workerData.whole) whole++;
  else { torn++; if (lens.length < 8) lens.push(text.length); }
  try { fs.unlinkSync(workerData.target); } catch {}
}
parentPort.postMessage({ whole, torn, lens });
`;

function tempDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "write-request-"));
}

/** Write `n` requests with `write(target, body)` against the racing reader; what it saw. */
async function race(write, body, n = 300) {
  const dir = tempDir();
  const target = path.join(dir, "showcase_request.json");
  const whole = typeof body === "string" ? body : JSON.stringify(body);
  const sab = new SharedArrayBuffer(8);
  const flag = new Int32Array(sab);
  const reader = new Worker(READER, { eval: true, workerData: { target, whole, flag: sab } });
  const report = new Promise((resolve, reject) => {
    reader.once("message", resolve);
    reader.once("error", reject);
  });
  try {
    for (const t0 = Date.now(); Atomics.load(flag, 1) === 0; ) {
      if (Date.now() - t0 > 10000) throw new Error("the reader thread never started");
      await new Promise((r) => setTimeout(r, 1));
    }
    let stalls = 0;
    for (let i = 0; i < n; i++) {
      // A plain write can find the name still being let go of by the reader's delete (Windows
      // says EPERM until the last handle closes); try it again, as the old rigs effectively did.
      for (const t0 = Date.now(); ; ) {
        try {
          write(target, body);
          break;
        } catch (e) {
          if (!WR.RETRY_CODES.has(e.code) || Date.now() - t0 > 3000) throw e;
        }
      }
      // Wait for the reader to take it, as a rig waits for the game (verify-falls showcase()).
      for (const t0 = Date.now(); fs.existsSync(target); ) {
        if (Date.now() - t0 > 3000) {
          stalls++;
          try {
            fs.unlinkSync(target);
          } catch {}
          break;
        }
      }
    }
    Atomics.store(flag, 0, 1);
    const seen = await report;
    const left = fs.readdirSync(dir);
    return { ...seen, stalls, left, n, bytes: Buffer.byteLength(whole) };
  } finally {
    Atomics.store(flag, 0, 1);
    await reader.terminate();
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const SMALL = { illness: "forgiving" }; // the size of the request verify-falls lost
const LARGER = { cam: "1,2,3,4,5", note: "x".repeat(4000) };

test("a reader racing the writer sees nothing or the whole request, never part of it", async () => {
  for (const [body, n] of [[SMALL, 300], [LARGER, 100]]) {
    const r = await race(WR.writeRequest, body, n);
    assert.strictEqual(
      r.torn,
      0,
      `a ${r.bytes}-byte request was read part-written ${r.torn} times in ${r.n} writes (read lengths: ${r.lens.join(", ")}). ` +
        `The game deletes a request once it has read it, so each of those requests was lost.`
    );
    assert.strictEqual(r.whole, r.n, `the reader took ${r.whole} of ${r.n} requests whole (${r.stalls} never taken)`);
    assert.deepStrictEqual(r.left, [], `left in the folder after the race: ${r.left.join(", ")} (a side file not cleaned up?)`);
  }
});

test("the same reader catches a plain fs.writeFileSync half-written (so the race above can fail)", async () => {
  const plainWrite = (target, body) => fs.writeFileSync(target, JSON.stringify(body));
  const r = await race(plainWrite, SMALL, 100);
  assert.ok(
    r.torn > 0,
    `the racing reader never caught a plain write part-written in ${r.n} tries. Until it does, the race test above ` +
      `proves nothing on this machine: make the reader faster or the writes more, do not delete this control.`
  );
});

test("the side file sits beside the request, is not a request name, and is gone after", () => {
  const dir = tempDir();
  try {
    const target = path.join(dir, "camera_request.json");
    const side = WR.sideNameFor(target);
    assert.strictEqual(path.dirname(side), dir, "the side file must be in the request's own folder, or the rename is a copy");
    assert.ok(!WR.isRequestFile(side), `${side} would be taken for a request`);
    assert.ok(WR.isRequestLeftover(path.basename(side)), "a left-behind side file must be swept up by a rig's setup");
    assert.notStrictEqual(WR.sideNameFor(target), side, "two writes must not share a side file");

    assert.strictEqual(WR.writeRequest(target, { station: "home" }), target);
    assert.strictEqual(fs.readFileSync(target, "utf8"), '{"station":"home"}');
    assert.deepStrictEqual(fs.readdirSync(dir), ["camera_request.json"]);
    // A second write replaces a request the game has not taken yet, as a plain write did.
    WR.writeRequest(target, { station: "home", screen: "web" });
    assert.strictEqual(fs.readFileSync(target, "utf8"), '{"station":"home","screen":"web"}');
    assert.deepStrictEqual(fs.readdirSync(dir), ["camera_request.json"]);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a rename Windows refuses for a moment is retried, then put in place", () => {
  const dir = tempDir();
  try {
    const target = path.join(dir, "showcase_request.json");
    let calls = 0;
    const rename = (a, b) => {
      calls++;
      if (calls < 3) throw Object.assign(new Error("EPERM: operation not permitted, rename"), { code: "EPERM" });
      fs.renameSync(a, b);
    };
    WR.writeRequest(target, SMALL, { rename, sleep: () => {} });
    assert.strictEqual(calls, 3);
    assert.strictEqual(fs.readFileSync(target, "utf8"), JSON.stringify(SMALL));
    assert.deepStrictEqual(fs.readdirSync(dir), ["showcase_request.json"]);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a rename that keeps failing removes its side file and says the game never got it", () => {
  const dir = tempDir();
  try {
    const target = path.join(dir, "autopilot_request.json");
    let clock = 0;
    let calls = 0;
    const busy = () => {
      calls++;
      throw Object.assign(new Error("EBUSY"), { code: "EBUSY" });
    };
    assert.throws(
      () => WR.writeRequest(target, {}, { rename: busy, sleep: (ms) => (clock += ms), now: () => clock, retryMs: 100, pollMs: 10 }),
      (e) => /autopilot_request\.json could not be put in place \(EBUSY/.test(e.message) && /the game never got it/.test(e.message)
    );
    assert.ok(calls > 5, `retried ${calls} times inside its 100 ms`);
    assert.deepStrictEqual(fs.readdirSync(dir), [], "the side file must not be left behind");

    // Not a lock: refused at once, no retry.
    calls = 0;
    const gone = () => {
      calls++;
      throw Object.assign(new Error("ENOENT"), { code: "ENOENT" });
    };
    assert.throws(() => WR.writeRequest(target, {}, { rename: gone, sleep: () => {} }), /ENOENT/);
    assert.strictEqual(calls, 1);
    assert.deepStrictEqual(fs.readdirSync(dir), []);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("a string body is written as it is; no path or no body is refused", () => {
  const dir = tempDir();
  try {
    const target = path.join(dir, "record_cancel.json");
    WR.writeRequest(target, "stop");
    assert.strictEqual(fs.readFileSync(target, "utf8"), "stop");
    assert.throws(() => WR.writeRequest(target), /no body given for record_cancel\.json/);
    assert.throws(() => WR.writeRequest("", {}), /no request path/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("the request names are the game's", () => {
  for (const n of [
    "showcase_request.json",
    "debug/screenshot_request.json",
    "C:\\rig\\debug\\cloud_profile_dump_request.json",
    "record_cancel.json",
    "debug/record_cancel.json",
  ]) {
    assert.ok(WR.isRequestFile(n), `${n} is a request`);
  }
  for (const n of ["camera_done.json", "ui_request_done.json", "record_done.json", "frame_costs.json", "bookmarks.json", "no_record_cancel.json"]) {
    assert.ok(!WR.isRequestFile(n), `${n} is not a request`);
  }
  assert.ok(WR.isRequestLeftover("showcase_request.json.part"), "verify-falls' older side name");
  assert.ok(WR.isRequestLeftover("showcase_request.json.4242-7.part"));
  assert.ok(!WR.isRequestLeftover("camera_done.json.tmp"), "the game's own side file for a done file is the game's");
});
