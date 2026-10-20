// Write a dev-IPC request file WHOLE, so the running game can never read half of it.
//
// Every rig drives the game by dropping a request file into the rig's debug/ folder
// (showcase_request.json, camera_request.json, screenshot_request.json, record_cancel.json,
// ...). The game polls for each one every frame, reads it, and DELETES it (src/engine/ipc.rs,
// src/engine/movie.rs). A plain fs.writeFileSync is two steps on disk: the file is created
// EMPTY, then filled. A frame that lands between the two reads an empty request, deletes it,
// and the request is gone with no error anywhere: the rig carries on as if it had been taken.
//
// That happened on 2026-10-20. One `just verify-falls` run's planet Forgiving leg ran in
// Realistic: its `illness` showcase request was read empty and deleted (run.log had no
// "Showcase: illness" line). Every one of the game's request readers now leaves an EMPTY
// request where it is for 250 ms in case its text is still coming (engine/ipc.rs
// take_request_text, REQUEST_EMPTY_GRACE), but a reader that waits for "not empty" still
// cannot tell a half-filled file from a whole one.
//
// So the rig side writes every request to a side name in the SAME folder and renames it onto
// the request name. A rename within one folder is one step: the request name either does not
// exist yet or names the whole file. Measured on this machine (2026-10-20, a reader thread
// polling and deleting like the game): a plain write was read EMPTY on 210 of 300 writes, even
// for a 10-byte request; written by rename, 0 of 300.
//
// The side name is `<request name>.<pid>-<n>.part`. The game never reads it (it opens its
// request names exactly), and the pid and counter keep two writers from sharing one side
// file. A rename that Windows refuses for a moment (EPERM, EACCES, EBUSY: the name is still
// being let go of by the game's delete, or a virus scanner has the side file open) is retried
// for up to two seconds; anything else, or a refusal that outlasts that, removes the side file
// and throws, so a failed request is never silent.
//
// Used by every rig that drives the game: verify-falls, probe-sweep, verify-copresence,
// verify-screens, verify-live-screen, make-clips, photograph-home, probe-hot-ab and
// boot-timing. ONE writer, so the next fix lands in all of them.
// scripts/tests/write-request.test.js proves it against a racing reader;
// scripts/tests/request-writes.test.js fails if any script under scripts/ writes a request
// file any other way.

"use strict";

const fs = require("fs");
const path = require("path");

// What the game's request files are called. Every request the game polls ends in
// `_request.json` (src/engine/ipc.rs, src/engine/movie.rs), plus the recorder's stop file,
// `record_cancel.json`. scripts/tests/request-writes.test.js checks this against the game's
// own request constants, so a request named some other way fails a test instead of slipping
// past it.
const REQUEST_NAME_RE = /(?:_request\.json|(?:^|[\\/])record_cancel\.json)$/;
// A side file this module left behind (a rig killed between the write and the rename). The
// game never reads one; the rigs sweep them up with the request files at setup. The bare
// `.part` form is the one verify-falls wrote before this module existed.
const REQUEST_PART_RE = /(?:_request\.json|(?:^|[\\/])record_cancel\.json)(?:\.\d+-\d+)?\.part$/;

const RETRY_CODES = new Set(["EPERM", "EACCES", "EBUSY"]);

let sideCounter = 0;

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

/// Is `name` (a file name or path) one of the game's request files?
function isRequestFile(name) {
  return REQUEST_NAME_RE.test(String(name));
}

/// Is `name` a request file, or a side file left behind while writing one? What a rig's setup
/// clears from its debug/ folder.
function isRequestLeftover(name) {
  return isRequestFile(name) || REQUEST_PART_RE.test(String(name));
}

/// The side name a request is written to before it is renamed into place: beside the request
/// (the same folder, so the rename is one step) and unique to this process and write.
function sideNameFor(target) {
  sideCounter += 1;
  return `${target}.${process.pid}-${sideCounter}.part`;
}

/// Write the request `target` (a full path) whole: `body` as JSON (a string or Buffer is
/// written as it is), to a side file first, then renamed onto `target`. Returns `target`.
/// `opt` is for tests: `rename`, `sleep`, `now`, `retryMs` (default 2000), `pollMs` (default 10).
function writeRequest(target, body, opt = {}) {
  if (typeof target !== "string" || target === "") throw new Error("writeRequest: no request path given");
  if (body === undefined) throw new Error(`writeRequest: no body given for ${path.basename(target)} (pass {} for "any content")`);
  const text = typeof body === "string" || Buffer.isBuffer(body) ? body : JSON.stringify(body);
  const rename = opt.rename || ((a, b) => fs.renameSync(a, b));
  const sleep = opt.sleep || sleepSync;
  const now = opt.now || (() => Date.now());
  const retryMs = opt.retryMs == null ? 2000 : opt.retryMs;
  const pollMs = opt.pollMs == null ? 10 : opt.pollMs;

  const side = sideNameFor(target);
  fs.writeFileSync(side, text);
  const t0 = now();
  for (;;) {
    try {
      rename(side, target);
      return target;
    } catch (e) {
      if (e && RETRY_CODES.has(e.code) && now() - t0 < retryMs) {
        sleep(pollMs);
        continue;
      }
      try {
        fs.unlinkSync(side);
      } catch {}
      const why = e && e.code ? e.code : String(e);
      const err = new Error(
        `the request ${path.basename(target)} could not be put in place (${why}, after ${now() - t0} ms).\n` +
          `  It was written whole to ${path.basename(side)} but the rename onto ${target} failed, so the game never got it.\n` +
          `  Is that folder writable, and is nothing else holding ${path.basename(target)} open?`
      );
      err.code = e && e.code;
      err.cause = e;
      throw err;
    }
  }
}

module.exports = { writeRequest, isRequestFile, isRequestLeftover, sideNameFor, REQUEST_NAME_RE, REQUEST_PART_RE, RETRY_CODES };
