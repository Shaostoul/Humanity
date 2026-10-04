// Tests for scripts/lib/rig-exe-copy.js: the rigs' exe copy waits out the lock
// a just-stopped game leaves on the old copy (the 2026-10-04 --plots EBUSY).
"use strict";

const test = require("node:test");
const assert = require("node:assert");
const { copyExeIntoRig } = require("../lib/rig-exe-copy.js");

// A fake clock and a copy that fails with `code` for the first `fails` tries.
function harness(fails, code = "EBUSY") {
  let t = 0;
  let tries = 0;
  let stops = 0;
  const logs = [];
  return {
    opt: {
      now: () => t,
      sleep: (ms) => {
        t += ms;
      },
      stop: () => {
        stops += 1;
      },
      log: (m) => logs.push(m),
      copy: () => {
        tries += 1;
        if (tries <= fails) {
          const e = new Error(`${code}: resource busy or locked`);
          e.code = code;
          throw e;
        }
      },
    },
    get tries() {
      return tries;
    },
    get stops() {
      return stops;
    },
    logs,
  };
}

test("an unlocked copy goes through at once, stopping the old game first", () => {
  const h = harness(0);
  const r = copyExeIntoRig("a", "b", h.opt);
  assert.strictEqual(r.attempts, 1);
  assert.strictEqual(r.waited_ms, 0);
  assert.strictEqual(h.stops, 1);
  assert.strictEqual(h.logs.length, 0);
});

test("a lock that outlives two seconds (the old single retry) is waited out", () => {
  // 8 failures at 500 ms = 4 s locked: the old one-retry-after-2-s code threw here.
  const h = harness(8);
  const r = copyExeIntoRig("a", "b", h.opt);
  assert.strictEqual(r.attempts, 9);
  assert.strictEqual(r.waited_ms, 4000);
  assert.ok(h.logs.some((m) => /still locked/.test(m)), "says it is waiting");
  assert.ok(h.logs.some((m) => /copied after 4\.0 s/.test(m)), "says how long it waited");
});

test("EPERM and EACCES count as the same lock", () => {
  for (const code of ["EPERM", "EACCES"]) {
    const h = harness(3, code);
    assert.strictEqual(copyExeIntoRig("a", "b", h.opt).attempts, 4);
  }
});

test("a game still starting is stopped again while the copy keeps failing", () => {
  const h = harness(25);
  copyExeIntoRig("a", "b", h.opt);
  // the first stop, then one every 10 failed tries (after tries 10 and 20)
  assert.strictEqual(h.stops, 3);
});

test("a lock that never clears fails after the timeout, naming the folder", () => {
  const h = harness(1e9);
  assert.throws(
    () => copyExeIntoRig("a", "C:/rig/HumanityOS.exe", { ...h.opt, timeoutMs: 60000 }),
    (e) => e.code === "EBUSY" && /stayed locked for 60 s/.test(e.message) && /C:\/rig\/HumanityOS\.exe/.test(e.message),
  );
});

test("an error that is not a lock is thrown at once, not retried", () => {
  const h = harness(5, "ENOENT");
  assert.throws(() => copyExeIntoRig("a", "b", h.opt), (e) => e.code === "ENOENT");
  assert.strictEqual(h.tries, 1);
});
