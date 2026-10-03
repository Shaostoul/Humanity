// The clip maker's side of the movie-mode protocol, against a stand-in for the
// game (2026-10-02 review follow-ups).
//
// Run:  node --test scripts/tests/make-clips.test.js
//
// The game is never booted (one GPU, one instance: CLAUDE.md). Instead each
// test plays the game's side of the request files in a scratch folder, the
// way src/engine/movie.rs does: it consumes record_cancel.json, writes the
// SAME log lines movie.rs writes (read from the Rust source, so a reworded log
// line fails here instead of silently breaking the script), and answers in
// record_done.json. The functions under test are the script's own, run on
// real files.
//
// The still test runs the real ffmpeg on a flat-colour clip when one is
// installed, and is skipped when none is.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const MC = require("../make-clips.js");

const REPO = path.resolve(__dirname, "..", "..");
const MOVIE_RS = fs.readFileSync(path.join(REPO, "src", "engine", "movie.rs"), "utf8");

// movie.rs's two answers to a cancel, from its source.
const stoppedTpl = MOVIE_RS.match(/log::info!\("(\[Movie\] cancelled at frame \{\})", rec\.frame\)/);
const nothingLine = MOVIE_RS.match(/log::info!\("(\[Movie\] cancel requested with no recording running: nothing to stop)"\)/);

function scratch(name) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `make-clips-${name}-`));
  fs.mkdirSync(path.join(dir, "debug"), { recursive: true });
  fs.mkdirSync(path.join(dir, "logs"), { recursive: true });
  fs.writeFileSync(path.join(dir, "logs", "run.log"), "");
  MC.useRigDirs(dir);
  return dir;
}

// The game's side: every 20 ms (a frame), consume a cancel and hand it to
// `onCancel`, which logs and answers the way movie.rs does.
function fakeGame(dir, onCancel) {
  const cancel = path.join(dir, "debug", "record_cancel.json");
  const g = {
    log: (msg) =>
      fs.appendFileSync(path.join(dir, "logs", "run.log"), `[2026-10-02T12:00:00.000Z INFO  humanity_engine::engine::movie] ${msg}\n`),
    done: (v) => fs.writeFileSync(path.join(dir, "debug", "record_done.json"), JSON.stringify(v)),
    stopped: (frame) => g.log(stoppedTpl[1].replace("{}", String(frame))),
    nothingToStop: () => g.log(nothingLine[1]),
  };
  const iv = setInterval(() => {
    if (fs.existsSync(cancel)) {
      fs.unlinkSync(cancel);
      onCancel(g);
    }
  }, 20);
  g.stop = () => clearInterval(iv);
  return g;
}

test("the script reads the log lines movie.rs actually writes", () => {
  assert.ok(stoppedTpl, "movie.rs no longer logs `[Movie] cancelled at frame {}`: update LOG_STOPPED in make-clips.js");
  assert.ok(nothingLine, "movie.rs no longer logs the nothing-to-stop line: update LOG_NOTHING_TO_STOP in make-clips.js");
  assert.match(stoppedTpl[1].replace("{}", "120"), MC.LOG_STOPPED);
  assert.match(nothingLine[1], MC.LOG_NOTHING_TO_STOP);
  assert.doesNotMatch(nothingLine[1], MC.LOG_STOPPED);
});

/// A RECORDING THAT FINISHES AS THE WAIT RUNS OUT IS KEPT. The game wrote
/// ok:true, then read the cancel with nothing left to stop. Seen red
/// 2026-10-02: with awaitRecording's `stopped.ok === true` branch removed (the
/// old caller), the shot came back ok:false, "cancel: undefined".
test("a recording that finishes as the wait runs out is the shot's result", async () => {
  const dir = scratch("finished");
  const answer = { ok: true, path: "debug/clip_x.mp4", frames: 300, fps: 30, size: [1920, 1080], wall_s: 181 };
  const game = fakeGame(dir, (g) => {
    g.done(answer);
    g.nothingToStop();
  });
  try {
    const r = await MC.awaitRecording("x", 200, { graceMs: 300, finalizeMs: 10000, takeBackMs: 3000 });
    assert.deepStrictEqual(r, answer);
    assert.ok(!fs.existsSync(path.join(dir, "debug", "record_cancel.json")), "no cancel left behind for the next shot");
  } finally {
    game.stop();
  }
});

/// A STOPPED RECORDING THAT TAKES MORE THAN 5 S TO CLOSE ITS FILE IS WAITED
/// FOR, at the script's own defaults. The game logs the stop at once, then is
/// busy closing ffmpeg (encoder flush, +faststart rewrite) for 6 s before the
/// done file. Seen red 2026-10-02: with the finalize wait cut back to the old
/// 5 s after the cancel is read (`finalizeMs` swapped for `graceMs` in
/// cancelRecording), the cancel returned null and the shot said "no reply".
test("a recording still closing its file after 5 s is waited for", async () => {
  const dir = scratch("slow-close");
  const answer = { ok: false, error: "cancelled", frames: 120, path: "debug/clip_y.mp4" };
  const game = fakeGame(dir, (g) => {
    g.stopped(180);
    setTimeout(() => g.done(answer), 6000);
  });
  try {
    const r = await MC.awaitRecording("y", 200);
    assert.strictEqual(r.ok, false);
    assert.match(r.error, /cancel: cancelled\)$/, r.error);
  } finally {
    game.stop();
  }
});

/// AN EARLIER SHOT'S "nothing to stop" DOES NOT CUT THIS CANCEL SHORT: only
/// the log written since this cancel counts. Seen red 2026-10-02: with
/// cancelRecording reading the whole log (`logFrom` = 0), the old line ended
/// the wait after the grace and this returned null.
test("only the log written since this cancel counts", async () => {
  const dir = scratch("old-lines");
  const game = fakeGame(dir, (g) => {
    g.stopped(60);
    setTimeout(() => g.done({ ok: false, error: "cancelled", frames: 0 }), 1500);
  });
  try {
    game.nothingToStop(); // an earlier shot's cancel, before this one was sent
    const r = await MC.cancelRecording({ graceMs: 300, finalizeMs: 10000, takeBackMs: 3000 });
    assert.deepStrictEqual(r, { ok: false, error: "cancelled", frames: 0 });
  } finally {
    game.stop();
  }
});

/// WITH NOTHING RUNNING, THE CANCEL GIVES UP AFTER THE GRACE, not the
/// finalize budget. Seen red 2026-10-02: with the nothing-to-stop branch
/// removed, it waited out the 10 s budget and the elapsed check failed.
test("a cancel with nothing running gives up after the grace", async () => {
  const dir = scratch("nothing");
  const game = fakeGame(dir, (g) => g.nothingToStop());
  try {
    game.stopped(40); // an earlier shot's stop: it must not stretch this wait
    const t0 = Date.now();
    const r = await MC.cancelRecording({ graceMs: 300, finalizeMs: 10000, takeBackMs: 3000 });
    assert.strictEqual(r, null);
    assert.ok(Date.now() - t0 < 4000, `took ${Date.now() - t0} ms`);
  } finally {
    game.stop();
  }
});

/// A CANCEL THE GAME NEVER READS IS TAKEN BACK, so it cannot stop the next
/// shot's recording.
test("an unread cancel is taken back", async () => {
  const dir = scratch("unread");
  const r = await MC.cancelRecording({ graceMs: 300, finalizeMs: 10000, takeBackMs: 600 });
  assert.strictEqual(r, null);
  assert.ok(!fs.existsSync(path.join(dir, "debug", "record_cancel.json")));
});

/// THE STILL IS PLAIN yuv420p, FULL RANGE SAID OUTRIGHT (2026-10-02). Seen
/// red: with the old `format=yuvj420p` and no -color_range, the first
/// assertion fails, and the ffmpeg test below prints "deprecated pixel format".
test("the still is cut as yuv420p with the full range set explicitly", () => {
  const still = MC.cutArgs("m.mp4", "x", 8, "out")[2].argv;
  const vf = still[still.indexOf("-vf") + 1];
  assert.ok(!/yuvj/.test(still.join(" ")), vf);
  assert.match(vf, /out_range=pc,format=yuv420p$/);
  assert.strictEqual(still[still.indexOf("-color_range") + 1], "pc");
  assert.strictEqual(still[still.indexOf("-update") + 1], "1");
});

/// The real cuts on the installed ffmpeg: a flat (200,120,60) master in the
/// engine's BT.709 encoding, the wide cut, then the still. No warnings, and
/// the colour survives within JPEG's rounding (the cutArgs comment: it came
/// back (199,118,56) on 2026-10-02).
test("the still cut runs clean on the installed ffmpeg and keeps the colour", (t) => {
  const ffmpeg = MC.findFfmpeg();
  if (!ffmpeg) return t.skip("no ffmpeg installed");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "make-clips-still-"));
  const run = (argv) => spawnSync(ffmpeg, ["-hide_banner", "-loglevel", "warning", "-y", ...argv], { encoding: "buffer" });
  const master = path.join(dir, "x-master.mp4");
  // The engine's own master encoding (movie.rs ffmpeg_args), from RGB.
  const m = run(["-f", "lavfi", "-i", "color=c=0xC8783C:s=640x360:r=30:d=1", "-vf",
    "format=rgba,scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv",
    "-c:v", "libx264", "-preset", "veryfast", "-crf", "14", "-pix_fmt", "yuv420p", ...MC.BT709_TAGS, master]);
  assert.strictEqual(m.status, 0, m.stderr.toString());
  const [wide, , still] = MC.cutArgs(master, "x", 1, dir);
  for (const c of [wide, still]) {
    const r = run(c.argv);
    assert.strictEqual(r.status, 0, `${c.what}: ${r.stderr}`);
    assert.strictEqual(r.stderr.toString().trim(), "", `${c.what} warned: ${r.stderr}`);
  }
  const px = run(["-i", path.join(dir, "x.jpg"), "-vf", "crop=2:2:iw/2:ih/2", "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]);
  assert.strictEqual(px.status, 0, px.stderr.toString());
  const rgb = [...px.stdout.subarray(0, 3)];
  [200, 120, 60].forEach((want, i) => assert.ok(Math.abs(rgb[i] - want) <= 6, `got ${rgb} for (200,120,60)`));
  fs.rmSync(dir, { recursive: true, force: true });
});

/// THE USAGE HEADER NAMES EVERY EXIT CODE the script can end with. Seen red
/// 2026-10-02: without the 130 line (Ctrl+C, Ctrl+Break, closed console).
test("the usage header lists every exit code", () => {
  const src = fs.readFileSync(path.join(REPO, "scripts", "make-clips.js"), "utf8");
  const header = src.slice(0, src.indexOf("const fs = require"));
  const codes = new Set([...src.matchAll(/process\.exit\((\d+)\)/g)].map((x) => x[1]));
  // `process.exit(failed || manifest.panics ? 2 : 0)`
  codes.add("0").add("2");
  for (const c of codes) assert.match(header, new RegExp(`\\b${c} = `), `exit ${c} is not in the usage header`);
});
