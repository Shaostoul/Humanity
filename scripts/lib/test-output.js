// A test program's output, its two streams kept apart (scripts/lib-tests.js, point 10; 2026-11-21).
//
// libtest writes its report to stdout, and a result line in two writes: "test x ... " (flushed),
// then "ok" and the newline. The GPU helper's watchdog (src/renderer/test_gpu.rs) writes its lines
// straight to stderr, from its own thread, at any moment. lib-tests.js used to keep both streams in
// ONE string and split that into lines, so a watchdog line that arrived between libtest's two
// writes was read as part of libtest's line: the hang report repeated
// "  test ignored_unless_asked ... GPU TEST STILL RUNNING: ..." as one garbled line, and the policy
// test that looks for the watchdog line on a line of its own failed whenever the machine was busy
// enough to shuffle the two (seen in several rig-tests runs on 2026-11-21).
//
// So each stream is split into lines on its own, and every reader reads the stream its text is
// written to: libtest's report from `stdout`, the watchdog's lines from `stderr`. `out`, both
// streams in the order they arrived, is kept only as the log a person reads (it is what the screen
// showed), never as text to split into lines.

"use strict";

// Past this, a stream stops being kept (a runaway test printing without end).
const CAP = 64 * 1024 * 1024;
// A watchdog line, from its marker to the end of the line (src/renderer/test_gpu.rs, `say`).
const WATCHDOG_RE = /GPU TEST (?:STILL RUNNING|HUNG):.*/;
// A result line libtest has begun and not finished: the name written, the result not yet.
const HALF_RE = /^test (\S+)(?: - should panic)? \.\.\. $/;

/**
 * Collects a test program's output as it arrives: add("stdout" | "stderr", chunk). onStdoutLine is
 * handed each whole line of stdout, split on stdout's own newlines, so nothing written to stderr
 * can land inside one; end() hands it the last, unfinished line (if any) and returns it.
 */
function collector(onStdoutLine = () => {}) {
  let tail = ""; // stdout since its last newline
  const c = {
    out: "",
    stdout: "",
    stderr: "",
    add(stream, chunk) {
      const d = String(chunk);
      if (c.out.length < CAP) c.out += d;
      if (c[stream].length < CAP) c[stream] += d;
      if (stream !== "stdout") return;
      const lines = (tail + d).split(/\r?\n/);
      tail = lines.pop();
      for (const l of lines) onStdoutLine(l);
    },
    end() {
      const t = tail;
      tail = "";
      if (t) onStdoutLine(t);
      return t;
    },
  };
  return c;
}

/**
 * The GPU helper's watchdog lines in a run ({ stderr }), each once, each from its marker on: read
 * from stderr alone, so libtest's half-written line cannot be glued to the front of one, and cut at
 * the marker, so neither can text a test left unfinished on stderr itself.
 */
function watchdogLines(res) {
  const said = new Set();
  for (const l of String((res && res.stderr) || "").split(/\r?\n/)) {
    const m = WATCHDOG_RE.exec(l);
    if (m) said.add(m[0].trim());
  }
  return [...said];
}

/**
 * The test whose result line libtest had begun and not finished when the program stopped (stdout
 * ends "test x ... " with nothing after it), or null. What it means depends on the thread count:
 * scripts/lib-tests.js, hangReport.
 */
function halfLine(res) {
  const s = String((res && res.stdout) || "");
  const m = HALF_RE.exec(s.slice(s.lastIndexOf("\n") + 1));
  return m ? m[1] : null;
}

/**
 * The tests a stopped run may be stuck in. `list`: the tests the run selected, in libtest's order
 * (or null); `done`: the names with a result line; `threads`: how many libtest ran at once; `half`:
 * halfLine's answer. A half line means two different things:
 *   - with ONE thread libtest writes a test's name as the test STARTS, so it names the test that was
 *     running when the program was stopped: `running`, the first suspect (the probe of 2026-11-21:
 *     the tiny suite's hang ends stdout "test hangs_silently_when_asked ... ");
 *   - with more, libtest writes the name only once the test's thread has exited, and the result
 *     straight after, so it is a finished test cut off mid-print: counted as finished, never a
 *     suspect (naming it would send the reader after a test that ended).
 * Returns { running, inFlight, finished } (inFlight is null without a list).
 */
function suspects({ list, done, threads, half }) {
  const running = half && threads === 1 ? half : null;
  const ended = half && threads > 1 ? half : null;
  if (!list) return { running, inFlight: null, finished: null };
  const unfinished = list.filter((n) => !done.has(n) && n !== ended);
  const inFlight = (running ? [running, ...unfinished.filter((n) => n !== running)] : unfinished).slice(0, threads);
  return { running, inFlight, finished: list.length - unfinished.length };
}

module.exports = { collector, watchdogLines, halfLine, suspects, WATCHDOG_RE };
