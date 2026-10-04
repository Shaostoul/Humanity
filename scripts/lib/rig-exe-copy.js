// Copy the game exe into a rig folder, waiting out the lock a just-stopped
// game leaves on the old copy.
//
// Every rig boots a COPY of the exe (so the build in target/release stays
// free for cargo). Before copying, the rig stops anything still running from
// its copy, but on Windows the image file stays locked for a moment after the
// process is gone: a copy straight after the stop fails with EBUSY. The rigs
// used to retry once after two seconds, which was not always enough: on
// 2026-10-04 `verify-copresence --plots` lost its second join order to EBUSY
// because the first order's game was still letting go of the file. So this
// keeps retrying, on a short interval, for up to a minute, and says when it
// had to wait.
//
// Used by probe-sweep, verify-copresence, verify-screens and
// verify-live-screen: one copy routine, so the next fix lands in all four.

"use strict";

const fs = require("fs");

const RETRY_CODES = new Set(["EBUSY", "EPERM", "EACCES"]);

function sleepSync(ms) {
  try {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
  } catch {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      /* SharedArrayBuffer unavailable: short spin */
    }
  }
}

/// Copy `src` to `dest`. `opt.stop()` (optional) is called first and again
/// every few seconds while the copy keeps failing, so a game that was still
/// starting when the first stop ran is caught too. Returns
/// { waited_ms, attempts }. Throws the last error once `opt.timeoutMs`
/// (default 60 s) has passed, or at once for an error that is not a lock.
function copyExeIntoRig(src, dest, opt = {}) {
  const copy = opt.copy || ((a, b) => fs.copyFileSync(a, b));
  const sleep = opt.sleep || sleepSync;
  const now = opt.now || (() => Date.now());
  const log = opt.log || (() => {});
  const stop = opt.stop || (() => {});
  const timeoutMs = opt.timeoutMs == null ? 60000 : opt.timeoutMs;
  const pollMs = opt.pollMs == null ? 500 : opt.pollMs;
  const restopEvery = opt.restopEvery == null ? 10 : opt.restopEvery;

  stop();
  const t0 = now();
  let attempts = 0;
  for (;;) {
    attempts += 1;
    try {
      copy(src, dest);
      const waited = now() - t0;
      if (attempts > 1) log(`[rig-copy] the old copy was still locked; copied after ${(waited / 1000).toFixed(1)} s (${attempts} tries)`);
      return { waited_ms: waited, attempts };
    } catch (e) {
      if (!e || !RETRY_CODES.has(e.code)) throw e;
      if (now() - t0 >= timeoutMs) {
        e.message = `${e.message}\n  the rig's copy of the exe stayed locked for ${Math.round((now() - t0) / 1000)} s after its game was stopped; ` +
          `something else may be running from ${dest}`;
        throw e;
      }
      if (attempts === 1) log(`[rig-copy] ${dest} is still locked (${e.code}); waiting for the stopped game to let go of it`);
      if (attempts % restopEvery === 0) stop();
      sleep(pollMs);
    }
  }
}

module.exports = { copyExeIntoRig, RETRY_CODES };
