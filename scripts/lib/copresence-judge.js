// THE CO-PRESENCE JUDGE: did a real game draw a second player walking past
// smoothly? Pure: it reads samples and returns checks, so it can be tested
// with made-up samples (scripts/tests/copresence-judge.test.js, in `just
// rig-tests`) and re-run on a saved run without booting anything
// (`node scripts/verify-copresence.js --dry-verdict <manifest.json>`).
//
// THE SAMPLES are what the game's remote-player recorder writes
// (debug/remote_players_request.json, src/engine/ipc.rs): one entry per
// rendered frame, { t, players: [{ id, name, pos: [x,y,z], phase }], cam },
// where `t` is the game's own frame clock (the sum of its real frame steps,
// the same step its other-player drawing runs on) and `pos` is where the
// figure was DRAWN that frame. `cam` is [x, y, z, yaw, pitch] of the local
// camera that frame.
//
// THE WALK being judged is the straight line the scripted player
// (scripts/second-player.js --path line) walked: from `line.start` to
// `line.end` at `speed` metres per second. It first walks from wherever the
// relay put it to `line.start` (the approach), and after `line.end` it turns
// back. Neither of those is judged: the PASS is from the first frame the
// figure is on the line at least START_MARGIN along it, to the first frame it
// is within END_MARGIN of the end. Those margins are geometry, not behaviour,
// so nothing the figure does can move a bad frame out of the judged pass.
//
// WHAT A PASS MUST SHOW, every frame of it:
//   - drawn at all, and on every frame (a figure that never appears FAILS);
//   - never further back along the line than the frame before;
//   - moving at the walker's real speed, within SPEED_BAND, measured as the
//     distance drawn between two frames over THEIR recorded frame times
//     (never an assumed 1/60: the rig's own frame times vary);
//   - no single-frame jump;
//   - on the line it walked;
//   - (when the camera was recorded) inside the camera's view;
//   - on time: where it was drawn against where the walker really was, by
//     the computer's own clock (not the game's frame clock, which is the
//     code under test);
//   - and no other figure drawn at all (the rig's relay holds only the game
//     and the walker, so any other is a ghost or the game drawing itself).

"use strict";

/// The limits, each with its reason.
const LIMITS = {
  /// How far a frame's speed may stray from the walker's, as a share: 10%.
  /// What src/net/sync.rs promises: its steady-speed tests hold every frame
  /// within 4% (STEADY_BAND; measured 0.7%) for a receiver at 60 frames a
  /// second. The only thing that changes the drawn speed there is the drawing
  /// easing toward the learned clock difference, at (how late the first
  /// update was - a 10 ms deadband) per second: 28 ms of jitter plus one
  /// 17 ms receiver frame gives 3.5%. What a real run adds: the rig's game
  /// runs in the background, where it is capped at 30 frames a second
  /// (fps_background), so an update can wait a whole 33 ms frame (or a
  /// slower one, under load, 50 ms) to be read off the socket instead of
  /// 17 ms. That grows the same easing term to (1 + 50 - 10) / 1000, about
  /// 4%. Doubled for margin, so one slow frame of a busy machine is not a
  /// failure: 10%. Measured on the real rig 2026-10-03 (8 m walk, 50 frame
  /// pairs): 1.357 to 1.475 m/s for 1.4. The 5.4% fast frames all came right
  /// after the screenshot capture's own 346 ms frame, when the buffer had
  /// briefly run dry and sync.rs blended the gap back over a tenth of a
  /// second (its BLEND_BACK_S); every other frame was within 3%. Round one's
  /// stop-go reads 0 m/s on its holding frames and about 1.5 times the
  /// walking speed on its easing frames, far outside the band.
  SPEED_BAND: 0.10,
  /// The judged pass starts this far along the line, metres: past the corner
  /// where the approach turns onto the line, whose frame mixes the two.
  START_MARGIN_M: 1.0,
  /// And ends this far before the end, metres: before the turn back.
  END_MARGIN_M: 0.5,
  /// How near the line a frame must be to START the pass, metres. Loose on
  /// purpose: only an approach frame within a few degrees of the line's own
  /// direction could meet it, and the rig checks the walker's approach never
  /// runs along the line (`approachClear`).
  WINDOW_TOL_M: 0.3,
  /// How far off the line any judged frame may be, metres. A figure walking a
  /// straight line between updates on a straight line never leaves it; this
  /// only allows for float rounding.
  LINE_TOL_M: 0.1,
  /// A backward step smaller than this is float rounding, not a step, metres.
  BACK_TOL_M: 0.001,
  /// A frame moving more than this many times its own walking distance (plus
  /// 1 cm) is a jump: a snap, not a fast frame.
  JUMP_FACTOR: 2.0,
  /// Fewest frame pairs a pass must hold to say anything: 30 is a second at
  /// 30 frames a second, or two at the 15 a busy background game manages.
  MIN_PAIRS: 30,
  /// Half-angle, degrees, from the camera's heading within which the figure
  /// counts as in view. The camera's vertical field of view is 90 degrees
  /// (renderer/camera.rs), so a wide window's horizontal half-angle is over
  /// 50; 40 keeps the figure well inside the picture.
  VIEW_HALF_ANGLE_DEG: 40,
  /// Latest a figure may be drawn behind where the walker really was,
  /// seconds of its walk (2026-10-03, the critic's "drawn a second late
  /// passes"). What it should be: sync.rs draws INTERP_DELAY_S (150 ms)
  /// behind the newest update, which is itself up to one send interval
  /// (67 ms at 15 Hz) behind the walker, and a background game at 9 to 15
  /// fps reads it up to one frame (about 110 ms) late; about 0.33 s in all.
  /// The walker's "on the path" line, which the expected position counts
  /// from, is read a few ms after it was written. 0.6 s leaves margin, and a
  /// regressed one-second delay (1.4 m at a walk) fails.
  MAX_LAG_S: 0.6,
  /// How far AHEAD of the walker a drawn figure may seem, metres: only
  /// measurement slack (the walker logs "on the path" on the tick that
  /// reached it, up to one 90 ms step past the start). Drawing ahead of
  /// the real walker would mean extrapolating past it.
  EARLY_TOL_M: 0.25,
};

const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const len = (a) => Math.sqrt(dot(a, a));
const f = (x, n = 2) => (Number.isFinite(x) ? x.toFixed(n) : String(x));

/** Is this recorded player the walker? By id when the id is known, else by
 *  name. */
function isWalker(p, walker) {
  if (walker.id !== null && walker.id !== undefined) return Number(p.id) === Number(walker.id);
  return p.name === walker.name;
}

/** Where along the line (u, metres from the start) and how far off it (perp,
 *  metres) a point is. */
function lineCoords(p, A, d) {
  const ap = sub(p, A);
  const u = dot(ap, d);
  const off = [ap[0] - u * d[0], ap[1] - u * d[1], ap[2] - u * d[2]];
  return { u, perp: len(off) };
}

/** Horizontal angle, degrees, between the camera's heading and the direction
 *  to `p`, and whether `p` is in front. The camera looks along
 *  (sin yaw, 0, -cos yaw) (renderer/camera.rs forward_xz). */
function viewAngle(cam, p) {
  const fx = Math.sin(cam[3]);
  const fz = -Math.cos(cam[3]);
  const tx = p[0] - cam[0];
  const tz = p[2] - cam[2];
  const ahead = fx * tx + fz * tz;
  const side = fx * tz - fz * tx;
  return { deg: (Math.abs(Math.atan2(side, ahead)) * 180) / Math.PI, ahead };
}

/**
 * Judge one recording.
 *   frames  the recorder's frames (see the top of this file).
 *   walker  { id, name }: who to look for (id wins when given).
 *   line    { start: [x,y,z], end: [x,y,z] }: the line it walked.
 *   speed   the speed it walked at, m/s.
 *   onLineEpochMs  when the walker reached the start of the line, by the
 *           computer's clock (ms since 1970), as the rig read it off the
 *           walker's "on the path" line. Each frame's `epoch_ms` is the same
 *           clock, so the two give where the walker really was on that frame.
 *           For a later forward leg of a walk that goes back and forth, the
 *           start of THAT leg (`forwardLegStart`).
 *   fromEpochMs  (optional) judge only frames from this moment on, by the
 *           same clock: the start of the forward leg being judged, so the end
 *           of the leg before it (walking back) is never mistaken for it.
 *   checkView  (default true) whether to check the figure was in the
 *           camera's view. The --plots run turns it off for the walk at home
 *           (the two players stand in their own homes and cannot see each
 *           other), and on for the meeting in the Commons (increment 2).
 * Returns { pass, checks: [{ id, ok, detail }], stats }.
 */
function judgeCopresence({ frames, walker, line, speed, onLineEpochMs, fromEpochMs = null, checkView = true }, limits = LIMITS) {
  const L0 = { ...LIMITS, ...limits };
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const stats = { frames_total: Array.isArray(frames) ? frames.length : 0 };
  const who = `${walker.name || "the walker"}${walker.id !== null && walker.id !== undefined ? ` (id ${walker.id})` : ""}`;
  const done = () => ({ pass: checks.length > 0 && checks.every((c) => c.ok), checks, stats });

  // 1. SEEN: the walker is drawn on some frame.
  const track = [];
  const others = new Set();
  (frames || []).forEach((fr, i) => {
    if (Number.isFinite(fromEpochMs) && Number(fr.epoch_ms) < fromEpochMs) return;
    for (const p of fr.players || []) {
      if (isWalker(p, walker))
        track.push({ i, t: Number(fr.t), epoch: fr.epoch_ms === undefined ? null : Number(fr.epoch_ms), pos: p.pos.map(Number), phase: p.phase || null, cam: fr.cam || null });
      else others.add(`${p.name} (id ${p.id})`);
    }
  });
  stats.frames_seen = track.length;
  add(
    "seen",
    track.length > 0,
    track.length
      ? `${who} drawn on ${track.length} of ${stats.frames_total} frames`
      : `${who} never drawn on any of ${stats.frames_total} frames (other players drawn: ${others.size ? [...others].join(", ") : "none"})`,
  );
  // No other figure at all. The rig's own relay holds exactly two players,
  // the game and the walker, and the game never draws itself; another figure
  // is a ghost (a player the relay failed to remove) or the game drawing
  // itself as a remote player (sync.rs's self-echo filter regressing).
  add(
    "only_the_walker",
    others.size === 0,
    others.size
      ? `another figure was drawn: ${[...others].join(", ")}; this relay holds only the game and ${walker.name || "the walker"}`
      : "no other figure was drawn",
  );
  if (!track.length) return done();

  // 2. THE PASS: from START_MARGIN along the line to END_MARGIN before its end.
  const A = line.start.map(Number);
  const B = line.end.map(Number);
  const L = len(sub(B, A));
  const d = sub(B, A).map((x) => x / L);
  for (const s of track) Object.assign(s, lineCoords(s.pos, A, d));
  const k0 = track.findIndex((s) => s.perp <= L0.WINDOW_TOL_M && s.u >= L0.START_MARGIN_M);
  const k1 = k0 < 0 ? -1 : track.findIndex((s, k) => k > k0 && s.u >= L - L0.END_MARGIN_M);
  if (k0 < 0 || k1 < 0) {
    const far = track.reduce((m, s) => (s.perp <= L0.WINDOW_TOL_M && s.u > m ? s.u : m), -Infinity);
    const near = Math.min(...track.map((s) => s.perp));
    add(
      "walked_the_line",
      false,
      k0 < 0
        ? `never drawn ${f(L0.START_MARGIN_M, 1)} m along the ${f(L)} m line (nearest it came to the line: ${f(near, 3)} m)`
        : `started the line but never got within ${f(L0.END_MARGIN_M, 1)} m of its end (farthest: ${f(far)} of ${f(L)} m)`,
    );
    return done();
  }
  const pass = track.slice(k0, k1 + 1);
  stats.pass = { t0: pass[0].t, t1: pass[pass.length - 1].t, u0: pass[0].u, u1: pass[pass.length - 1].u, line_m: L };
  add(
    "walked_the_line",
    true,
    `judged the pass from ${f(pass[0].u)} m (t ${f(pass[0].t)} s) to ${f(pass[pass.length - 1].u)} m (t ${f(pass[pass.length - 1].t)} s) along the ${f(L)} m line`,
  );

  // 3. Drawn on every frame of the pass (frame numbers contiguous).
  const missing = pass[pass.length - 1].i - pass[0].i + 1 - pass.length;
  add("seen_every_frame", missing === 0, missing === 0 ? `drawn on all ${pass.length} frames of the pass` : `missing on ${missing} frame(s) inside the pass`);

  // 4. Frame by frame.
  const speeds = [];
  const back = [];
  const slow = [];
  const jumps = [];
  const offLine = [];
  const outOfView = [];
  let badClock = 0;
  let maxBack = 0;
  let maxMove = 0;
  let maxPerp = 0;
  let maxAngle = 0;
  const lo = speed * (1 - L0.SPEED_BAND);
  const hi = speed * (1 + L0.SPEED_BAND);
  for (let k = 1; k < pass.length; k++) {
    const a = pass[k - 1];
    const b = pass[k];
    const dt = b.t - a.t;
    if (!(dt > 0)) {
      badClock++;
      continue;
    }
    const moved = len(sub(b.pos, a.pos));
    const du = b.u - a.u;
    const v = moved / dt;
    speeds.push(v);
    maxMove = Math.max(maxMove, moved);
    if (du < -L0.BACK_TOL_M) back.push({ t: b.t, du, phase: b.phase });
    maxBack = Math.max(maxBack, -du);
    if (v < lo || v > hi) slow.push({ t: b.t, v, dt, phase: b.phase });
    if (moved > L0.JUMP_FACTOR * speed * dt + 0.01) jumps.push({ t: b.t, moved, dt, phase: b.phase });
  }
  for (const s of pass) {
    maxPerp = Math.max(maxPerp, s.perp);
    if (s.perp > L0.LINE_TOL_M) offLine.push(s);
    if (s.cam) {
      const va = viewAngle(s.cam, s.pos);
      maxAngle = Math.max(maxAngle, va.deg);
      if (va.ahead <= 0 || va.deg > L0.VIEW_HALF_ANGLE_DEG) outOfView.push({ t: s.t, deg: va.deg, ahead: va.ahead });
    }
  }
  const sorted = [...speeds].sort((x, y) => x - y);
  stats.pairs = speeds.length;
  stats.speed = sorted.length
    ? { min: sorted[0], max: sorted[sorted.length - 1], median: sorted[Math.floor(sorted.length / 2)], walker: speed, band: L0.SPEED_BAND }
    : null;
  stats.max_backward_m = maxBack;
  stats.max_frame_move_m = maxMove;
  stats.max_off_line_m = maxPerp;
  const dts = pass.slice(1).map((s, k) => s.t - pass[k].t).filter((x) => x > 0).sort((x, y) => x - y);
  stats.frame_dt = dts.length ? { min: dts[0], max: dts[dts.length - 1], median: dts[Math.floor(dts.length / 2)] } : null;

  add(
    "enough_frames",
    speeds.length >= L0.MIN_PAIRS && badClock === 0,
    `${speeds.length} frame pairs judged (at least ${L0.MIN_PAIRS})` + (badClock ? `; ${badClock} pair(s) whose frame clock did not move forward` : ""),
  );
  add(
    "never_backwards",
    back.length === 0,
    back.length
      ? `${back.length} frame(s) drawn further back along the line than the frame before; worst ${f(Math.min(...back.map((x) => x.du)) * 1000, 1)} mm at t ${f(back[0].t)} s (${back[0].phase})`
      : `no frame moved back along the line (largest backward step ${f(maxBack * 1000, 2)} mm, rounding allowance ${f(L0.BACK_TOL_M * 1000, 0)} mm)`,
  );
  const worstSlow = slow.slice().sort((x, y) => Math.abs(y.v - speed) - Math.abs(x.v - speed));
  add(
    "steady_speed",
    speeds.length > 0 && slow.length === 0,
    stats.speed
      ? `per-frame speed ${f(stats.speed.min, 3)} to ${f(stats.speed.max, 3)} m/s (median ${f(stats.speed.median, 3)}), walker ${f(speed, 3)} m/s, allowed ${f(lo, 3)} to ${f(hi, 3)}` +
          (slow.length
            ? `; ${slow.length} of ${speeds.length} frames outside, worst ${worstSlow
                .slice(0, 3)
                .map((x) => `${f(x.v, 3)} m/s at t ${f(x.t)} s over ${f(x.dt * 1000, 1)} ms (${x.phase})`)
                .join("; ")}`
            : "")
      : "no frame pairs to measure",
  );
  add(
    "no_jump",
    jumps.length === 0,
    jumps.length
      ? `${jumps.length} single-frame jump(s); worst ${f(Math.max(...jumps.map((x) => x.moved)), 3)} m in one frame at t ${f(jumps[0].t)} s (${jumps[0].phase})`
      : `largest move in one frame ${f(maxMove, 3)} m (a jump is over ${f(L0.JUMP_FACTOR, 0)}x that frame's walking distance + 1 cm)`,
  );
  add(
    "on_the_line",
    offLine.length === 0,
    offLine.length
      ? `${offLine.length} frame(s) more than ${f(L0.LINE_TOL_M)} m off the line it walked; worst ${f(maxPerp, 3)} m`
      : `stayed within ${f(maxPerp, 4)} m of the line it walked (allowed ${f(L0.LINE_TOL_M)} m)`,
  );
  // ON TIME, by the computer's clock. Each frame's epoch_ms against when the
  // walker reached the line gives where it really was; the figure must be
  // drawn no more than MAX_LAG_S of walking behind that and never ahead. This
  // is the one check that does not lean on the game's own frame clock, so it
  // also catches that clock running wrong.
  if (Number.isFinite(onLineEpochMs) && pass.every((s) => Number.isFinite(s.epoch))) {
    const lags = [];
    for (const s of pass) {
      const expected = (speed * (s.epoch - onLineEpochMs)) / 1000;
      if (expected >= 0 && expected <= L) lags.push({ t: s.t, lag: expected - s.u });
    }
    const lateM = speed * L0.MAX_LAG_S;
    const bad = lags.filter((x) => x.lag > lateM || x.lag < -L0.EARLY_TOL_M);
    const ls = lags.map((x) => x.lag);
    stats.lag_m = lags.length ? { min: Math.min(...ls), max: Math.max(...ls) } : null;
    add(
      "on_time",
      lags.length >= L0.MIN_PAIRS && bad.length === 0,
      !lags.length
        ? "no frame of the pass fell inside the walk's own timing"
        : `drawn ${f(stats.lag_m.min, 3)} to ${f(stats.lag_m.max, 3)} m behind where the walker really was on ${lags.length} frames ` +
            `(allowed ${f(-L0.EARLY_TOL_M, 2)} to ${f(lateM, 2)} m, ${L0.MAX_LAG_S} s of walking)` +
            (bad.length ? `; ${bad.length} outside, first at t ${f(bad[0].t)} s: ${f(bad[0].lag, 3)} m` : "") +
            (lags.length < L0.MIN_PAIRS ? `; too few frames (at least ${L0.MIN_PAIRS})` : ""),
    );
  } else {
    add(
      "on_time",
      false,
      Number.isFinite(onLineEpochMs)
        ? "the samples carry no epoch_ms (a recorder older than v0.1441), so where the walker really was cannot be told"
        : "no time for when the walker reached the line, so where it really was cannot be told",
    );
  }
  if (checkView && pass.some((s) => s.cam)) {
    stats.max_view_angle_deg = maxAngle;
    add(
      "in_view",
      outOfView.length === 0,
      outOfView.length
        ? `${outOfView.length} frame(s) of the pass outside ${L0.VIEW_HALF_ANGLE_DEG} degrees of the camera's heading (or behind it); first at t ${f(outOfView[0].t)} s, ${f(outOfView[0].deg, 1)} degrees`
        : `within ${f(maxAngle, 1)} degrees of the camera's heading on every frame of the pass (allowed ${L0.VIEW_HALF_ANGLE_DEG})`,
    );
  }
  return done();
}

// ── Homes on plots (increment 1b, `verify-copresence --plots`) ──────────────
//
// The relay hands each player a plot of the ship; the game moves its home to
// its own plot and its camera to that plot's spawn. The rig runs the two join
// orders and these checks judge each run: the two players hold different
// plots, by the order they joined; the game's camera is inside the plot the
// game should hold; and every position the game DREW for the walker is inside
// the walker's plot (the recorder records figures off screen too). Whether
// they can see each other is the meeting in the Commons (increment 2, below:
// door points and routes, `judgeMeet`).

/** True when the walker's straight approach from `from` to the line's start
 *  can never pass for the walk: no point of it is within WINDOW_TOL of the
 *  line at least START_MARGIN along it, which is what opens the judged pass.
 *  An approach from behind the start always is; since increment 1b the
 *  walker joins on its own plot and may come at the line from anywhere, so
 *  the rig checks the real path (second-player.js makeWalk walks it straight).
 *  `line` is { start, end }. Pure. */
function approachClear(from, line, limits = LIMITS) {
  const A = line.start.map(Number);
  const B = line.end.map(Number);
  const L = len(sub(B, A));
  const d = sub(B, A).map((x) => x / L);
  for (let i = 0; i <= 2000; i++) {
    const p = from.map((v, k) => v + ((A[k] - v) * i) / 2000);
    const { u, perp } = lineCoords(p, A, d);
    if (perp <= limits.WINDOW_TOL_M && u >= limits.START_MARGIN_M) return false;
  }
  return true;
}

/** The start, by the computer's clock (ms), of the walker's first FORWARD leg
 *  at or after `atMs`. second-player.js walks a line back and forth for as
 *  long as it runs: start to end in 2r/speed seconds and back in as many, so
 *  a forward leg begins every 4r/speed seconds from when it reached the line. */
function forwardLegStart(onLineEpochMs, speed, radius, atMs) {
  const cycle = ((4 * radius) / speed) * 1000;
  const k = Math.max(0, Math.ceil((atMs - onLineEpochMs) / cycle));
  return onLineEpochMs + k * cycle;
}

/** The plots of the ship file's text (data/blueprints/ship_structure.ron), in
 *  order: [{ id, kind, origin: [x,y,z], size: [w,h,d] }]. For a game too old
 *  to report its plots (the 1a build the red run uses); a game that reports
 *  them is believed instead. */
function readShipPlots(text) {
  const i = text.indexOf("plots: [");
  if (i < 0) return [];
  const j = text.indexOf("default_plot", i);
  const block = text.slice(i, j > 0 ? j : undefined);
  const nums = (s) => s.split(",").map((v) => Number(v.trim()));
  const out = [];
  const re = /id:\s*"([^"]+)",[\s\S]*?kind:\s*"([^"]+)",\s*origin:\s*\(([^)]*)\),\s*size:\s*\(([^)]*)\)/g;
  for (let m; (m = re.exec(block)); ) out.push({ id: m[1], kind: m[2], origin: nums(m[3]), size: nums(m[4]) });
  return out;
}

/** Is point `p` inside a plot's box (a centimetre of slack for rounding)? */
function inPlot(p, plot, tol = 0.01) {
  return [0, 1, 2].every((k) => p[k] >= plot.origin[k] - tol && p[k] <= plot.origin[k] + plot.size[k] + tol);
}
/** How far point `p` is from a plot's footprint, across the floor (x and z),
 *  metres: 0 inside it. */
function footprintGap(p, plot) {
  const dx = Math.max(plot.origin[0] - p[0], 0, p[0] - (plot.origin[0] + plot.size[0]));
  const dz = Math.max(plot.origin[2] - p[2], 0, p[2] - (plot.origin[2] + plot.size[2]));
  return Math.hypot(dx, dz);
}

/** The plot whose footprint is nearest point `p` (the first on a tie). */
function nearestPlot(p, plots) {
  let best = null;
  for (const pl of plots) if (!best || footprintGap(p, pl) < footprintGap(p, best)) best = pl;
  return best;
}

const boxText = (pl) =>
  `${pl.id} (x ${pl.origin[0]}..${pl.origin[0] + pl.size[0]}, y ${pl.origin[1]}..${pl.origin[1] + pl.size[1]}, z ${pl.origin[2]}..${pl.origin[2] + pl.size[2]})`;

/**
 * Judge one --plots run.
 *   order       "walker-first" or "game-first": who claimed a plot first.
 *   plots       the ship's plots in order (the game's report, else the file).
 *   gamePlot    the plot id the game says its home stands on (null for none).
 *   walkerPlot  the plot id the relay gave the walker (null for none).
 *   camera      [x, y, z] of the game's camera after joining.
 *   homeThings  where the game's home's own things stand after joining (the
 *               recorder's `home_things`): { respawn, hologram, showroom,
 *               animals: [[x,y,z]...], plants: [[x,y,z]...] }. Each must be
 *               nearer the plot the game should hold than any other plot (the
 *               hologram hangs half a metre outside the home's west wall, so
 *               "inside" would be too strict). Things the world load placed
 *               for the home and the rebuild does not redo used to stay on
 *               the default plot when the home moved.
 *   frames, walker  the recorder's frames and who the walker is (as above).
 * The plots each SHOULD hold come from the join order alone: the first to
 * join holds the first plot and the second the second (the relay's rule), so
 * a build that hands out no plots fails here with where things really were.
 * Returns { pass, checks }.
 */
function judgePlots({ order, plots, gamePlot, walkerPlot, camera, homeThings, frames, walker }) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  if (!Array.isArray(plots) || plots.length < 2) {
    add("plots_known", false, `the ship's plots are unknown (${JSON.stringify(plots)}); two are needed`);
    return { pass: false, checks };
  }
  const gameFirst = order === "game-first";
  const expGame = gameFirst ? plots[0] : plots[1];
  const expWalker = gameFirst ? plots[1] : plots[0];
  add(
    "plot_ids_differ",
    gamePlot && walkerPlot && gamePlot !== walkerPlot,
    `the game holds ${gamePlot || "no plot"}, the walker ${walkerPlot || "no plot"}`,
  );
  add(
    "plots_by_join_order",
    gamePlot === expGame.id && walkerPlot === expWalker.id,
    `${order}: the game should hold ${expGame.id} and holds ${gamePlot || "none"}; the walker should hold ${expWalker.id} and holds ${walkerPlot || "none"}`,
  );
  const camOk = Array.isArray(camera) && inPlot(camera, expGame);
  add(
    `camera_in_${expGame.id}`,
    camOk,
    Array.isArray(camera)
      ? `after joining, the game's camera at (${camera.map((v) => Number(v).toFixed(2)).join(", ")}) is ${camOk ? "inside" : "OUTSIDE"} ${boxText(expGame)}, the plot the game should hold`
      : "the game never reported its camera after joining",
  );
  const h = homeThings || null;
  const things = h
    ? [
        ["the Respawn point", h.respawn],
        ["the hologram", h.hologram],
        ["the showroom stage", h.showroom],
        ...(h.animals || []).map((a, i) => [`animal ${i + 1}`, a]),
        ...(h.plants || []).map((a, i) => [`plant ${i + 1}`, a]),
        // What the player built aboard and parked (the second review of 1b): none in a
        // fresh rig sandbox, so not required, but each one reported must be on the plot.
        ...(h.structures || []).map((a, i) => [`built piece ${i + 1}`, a]),
        ...(h.vehicles || []).map((a, i) => [`vehicle ${i + 1}`, a]),
      ]
    : [];
  const strays = things.filter(([, p]) => !Array.isArray(p) || nearestPlot(p.map(Number), plots) !== expGame);
  const nA = h ? (h.animals || []).length : 0;
  const nP = h ? (h.plants || []).length : 0;
  add(
    "home_things_on_its_plot",
    h && nA > 0 && nP > 0 && strays.length === 0,
    !h
      ? "the game reported none of its home's things (a build from before the fix)"
      : !nA || !nP
        ? `the game reported ${nA} animals and ${nP} plants; the shipped home has both, so nothing would be checked`
        : strays.length
          ? `${strays.length} of ${things.length} of the home's things are not on ${expGame.id}: ` +
            strays
              .slice(0, 4)
              .map(([n, p]) => `${n} at (${Array.isArray(p) ? p.map((v) => Number(v).toFixed(1)).join(", ") : "nowhere"}) by ${Array.isArray(p) ? nearestPlot(p.map(Number), plots).id : "?"}`)
              .join("; ")
          : `the Respawn point, hologram, showroom stage, ${nA} animals, ${nP} plants, ` +
            `${(h.structures || []).length} built pieces and ${(h.vehicles || []).length} vehicles are all on ${expGame.id}`,
  );
  const drawn = [];
  for (const fr of frames || []) for (const p of fr.players || []) if (isWalker(p, walker)) drawn.push({ t: Number(fr.t), pos: p.pos.map(Number) });
  const outside = drawn.filter((d) => !inPlot(d.pos, expWalker));
  add(
    "walker_drawn_in_its_plot",
    drawn.length > 0 && outside.length === 0,
    !drawn.length
      ? `${walker.name || "the walker"} was never drawn`
      : outside.length
        ? `${outside.length} of ${drawn.length} drawn positions OUTSIDE ${boxText(expWalker)}; first at t ${f(outside[0].t)} s: (${outside[0].pos.map((v) => v.toFixed(2)).join(", ")})`
        : `all ${drawn.length} drawn positions inside ${boxText(expWalker)}`,
  );
  return { pass: checks.every((c) => c.ok), checks };
}

/**
 * Judge how the game came into the world (verify-copresence --plots, round 4
 * of the 1b review). A returning player's game identifies on the main menu
 * (its auto-connect) and only then is Enter World pressed: on that frame the
 * join gate runs BEFORE the world has loaded, and the first build of 1b sent
 * a join naming no ship there, which the relay refused as another ship. The
 * autopilot creates the identity as it enters, so its socket identifies after
 * the world loads and that race never runs. `entry`:
 *   kind               "menu" (connected first, then Enter World pressed) or
 *                      "autopilot" (the all-in-one entry)
 *   identified         the probe before the press saw ws_identified
 *   world_loaded       ... and saw world_loaded (must be false)
 *   page_after_click   the page the press left (must be "None": in the world)
 *   world_loaded_after_click  world_loaded right after the press (must be
 *                      false: the join gate then runs before the world loads)
 * A menu entry passes only when the race really ran; an autopilot entry is
 * recorded as such and passes (it is the other path, judged by the rest).
 * Returns { pass, checks }.
 */
function judgeEntry(entry) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  if (!entry || !entry.kind) {
    add("entry_known", false, "the run never recorded how the game came into the world");
  } else if (entry.kind === "autopilot") {
    add("entry_known", true, "the autopilot entered the world as it created the identity (the identify-first race does not run on this path)");
  } else {
    const raced = entry.identified === true && entry.world_loaded === false && entry.page_after_click === "None" && entry.world_loaded_after_click === false;
    add(
      "entered_from_menu_after_identify",
      raced,
      `before Enter World: ws_identified=${entry.identified} world_loaded=${entry.world_loaded}; after the press: page ${entry.page_after_click} world_loaded=${entry.world_loaded_after_click}` +
        (raced ? " (the join gate ran before the world loaded, the returning player's path)" : ": the identify-first race did NOT run, so this run proves nothing about it"),
    );
  }
  return { pass: checks.every((c) => c.ok), checks };
}

/** How far the game must have stood from where the relay spawns it for the
 *  step-out-and-back check to mean anything: past the relay's 100 m rule, so a
 *  game that stayed where it stood would have every update refused. */
const REJOIN_FAR_M = 100;
/** How near where the relay holds it the game must stand after rejoining. */
const REJOIN_STAND_TOL_M = 0.5;
/** How near the nudged point the relayed update must be. */
const REJOIN_NUDGE_TOL_M = 0.3;

/**
 * Judge the step out of the shared world and back (verify-copresence --plots,
 * the second review of 1b): the game stepped out, its camera was moved far
 * away, it stepped back in, and the relay spawned it afresh at its door.
 *   far          [x,y,z] where the game stood when it stepped back in.
 *   relaySpawn   [x,y,z] where the relay spawned it (the walker's log line
 *                "player joined: ... at (x, y, z)"), or null when never seen.
 *   camera       [x,y,z] the game's camera after the welcome, or null.
 *   nudged       [x,y,z] where the rig then moved the camera, or null.
 *   seen         the positions the walker saw the relay pass on for the game's
 *                new entity after the nudge ([[x,y,z]...]).
 * Checks: the experiment means something (far is past the 100 m rule from the
 * spawn); the game stands where the relay holds it; and its next move reached
 * the others (the relay accepted it, so the player is not frozen).
 *
 * The Respawn leg (the third review of 1b) is judged the same way, with
 * `{ prefix: "respawn", when: "it pressed Respawn" }`: `far` is then where the
 * relay held the game when it pressed Respawn (walked there in steps the relay
 * accepted), `relaySpawn` where the relay spawned it after the button stepped
 * it out and back in.
 * Returns { pass, checks }.
 */
function judgeRejoin({ far, relaySpawn, camera, nudged, seen }, { prefix = "rejoin", when = "it stepped back in" } = {}) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `${prefix}_${id}`, ok: !!ok, detail });
  const d = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const gap = d(far, relaySpawn);
  add(
    "far_from_spawn",
    Number.isFinite(gap) && gap > REJOIN_FAR_M,
    relaySpawn
      ? `the game stood at ${fmt3(far)} when ${when}, ${gap.toFixed(1)} m from where the relay spawned it ${fmt3(relaySpawn)}` +
          (gap > REJOIN_FAR_M ? "" : ` (needs more than ${REJOIN_FAR_M} m to mean anything)`)
      : "the walker never saw the game join again, so where the relay spawned it is unknown",
  );
  const off = d(camera, relaySpawn);
  add(
    "stands_where_held",
    off <= REJOIN_STAND_TOL_M,
    `after rejoining, the game's camera at ${fmt3(camera)} is ${Number.isFinite(off) ? off.toFixed(2) : "?"} m from where the relay holds it ${fmt3(relaySpawn)}` +
      (off <= REJOIN_STAND_TOL_M ? "" : ` (at most ${REJOIN_STAND_TOL_M} m; a game left there is frozen for everyone else)`),
  );
  const hit = (seen || []).find((p) => d(p, nudged) <= REJOIN_NUDGE_TOL_M);
  add(
    "moves_reach_others",
    !!hit,
    hit
      ? `its next move to ${fmt3(nudged)} reached the walker through the relay, at ${fmt3(hit)}`
      : `its next move to ${fmt3(nudged)} never reached the walker (${(seen || []).length} update(s) seen after it): the relay refused it`,
  );
  return { pass: checks.every((c) => c.ok), checks };
}

/**
 * Judge shutting the build editor far from the build spot (verify-copresence
 * --plots, round 5 of the 1b review, finding 1). The game walked, in steps the
 * relay accepted, more than 100 m from its build spot, opened the build editor
 * (the showcase `build_editor` verb, the B key's own function) and shut it.
 * Shutting it stands the player at the build spot when nothing holds them; in
 * the shared world that was a jump the relay refuses, and the others saw the
 * figure frozen where it last stood.
 *   buildSpot  [x,y,z] where shutting the editor stands the game when the spot
 *              is near (the rig opens and shuts it once at the door first, which
 *              puts the build-mode avatar there), or null.
 *   held       [x,y,z] where the relay held the game when it opened the editor:
 *              the last of its moves the walker saw the relay pass on, or null.
 *   camera     [x,y,z] the game's camera after the editor shut, or null.
 *   nudged, seen   as for judgeRejoin: the next move, and what the walker saw.
 * Checks: the experiment means something (the build spot is past the 100 m
 * rule from where the relay holds the game); the game still stands where the
 * relay holds it; and its next move reached the others.
 * Returns { pass, checks }.
 */
function judgeEditorClose({ buildSpot, held, camera, nudged, seen }) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `editor_${id}`, ok: !!ok, detail });
  const d = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const gap = d(buildSpot, held);
  add(
    "far_from_build_spot",
    Number.isFinite(gap) && gap > REJOIN_FAR_M,
    buildSpot && held
      ? `the build spot ${fmt3(buildSpot)} is ${gap.toFixed(1)} m from where the relay held the game when it opened the editor ${fmt3(held)}` +
          (gap > REJOIN_FAR_M ? "" : ` (needs more than ${REJOIN_FAR_M} m to mean anything)`)
      : `the build spot ${fmt3(buildSpot)} or where the relay held the game ${fmt3(held)} is unknown`,
  );
  const off = d(camera, held);
  add(
    "stands_where_held",
    off <= REJOIN_STAND_TOL_M,
    `after the editor shut, the game's camera at ${fmt3(camera)} is ${Number.isFinite(off) ? off.toFixed(2) : "?"} m from where the relay holds it ${fmt3(held)}` +
      (off <= REJOIN_STAND_TOL_M ? "" : ` (at most ${REJOIN_STAND_TOL_M} m; a game put farther than 100 m away is frozen for everyone else)`),
  );
  const hit = (seen || []).find((p) => d(p, nudged) <= REJOIN_NUDGE_TOL_M);
  add(
    "moves_reach_others",
    !!hit,
    hit
      ? `its next move to ${fmt3(nudged)} reached the walker through the relay, at ${fmt3(hit)}`
      : `its next move to ${fmt3(nudged)} never reached the walker (${(seen || []).length} update(s) seen after it): the relay refused it`,
  );
  return { pass: checks.every((c) => c.ok), checks };
}

// ── Door points and routes (increment 2, "Meet in the Commons") ─────────────
//
// The game reports the ship's places and every corridor's door points, from its
// own corridor geometry (debug/door_points_request.json, src/ship/door_points.rs):
//   { ship_hash, places: [{ id, kind, purpose, min, max, door, door_local, own }],
//     doors: [{ from, to, axis, lat, mouths: [a, b], steps: [inFrom, inTo], tube: [min, max] }] }
// A route is planned on that and nothing else, so no corridor maths lives here:
// from the place a walk starts in to the place it ends in, through the doors
// between them (breadth first), each crossed from its step on one side to its
// step on the other. (Until increment 2 the rig's walks used a hard-coded copy
// of the ship's corridors, `respawnRoute`.)

/** The place `p` stands in (its box across the floor, x and z, a few cm of
 *  slack), or null. Pure. */
function placeAt(report, p, tol = 0.05) {
  return (report.places || []).find((pl) => p[0] >= pl.min[0] - tol && p[0] <= pl.max[0] + tol && p[2] >= pl.min[2] - tol && p[2] <= pl.max[2] + tol) || null;
}

/** The place nearest `p` across the floor (for a point in a corridor tube, say). */
function nearestPlace(report, p) {
  let best = null;
  let bestGap = Infinity;
  for (const pl of report.places || []) {
    const dx = Math.max(pl.min[0] - p[0], 0, p[0] - pl.max[0]);
    const dz = Math.max(pl.min[2] - p[2], 0, p[2] - pl.max[2]);
    const gap = Math.hypot(dx, dz);
    if (gap < bestGap) [best, bestGap] = [pl, gap];
  }
  return best;
}

/** Cut a walk through `points` (from `from`) into steps of at most `maxStep`
 *  metres across the floor; the points after `from`. Pure. */
function stepsAlong(from, points, maxStep) {
  const out = [];
  let a = from;
  for (const b of points) {
    const n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[2] - a[2]) / maxStep));
    for (let k = 1; k <= n; k++) out.push([0, 1, 2].map((j) => a[j] + ((b[j] - a[j]) * k) / n));
    a = b;
  }
  return out;
}

/**
 * The walk from `from` to `to` through the ship's doors (see above). Returns
 *   waypoints  the steps of each door crossed, in order, then `to` (the
 *              coarse route a scripted player walks: second-player.js --route)
 *   points     the same walk in steps of at most `maxStep` metres across the
 *              floor (each one the relay accepts under its 100 m rule, for
 *              moving the game with the showcase `cam` verb)
 *   doors      "from->to" for each door crossed
 *   error      why there is no route (an unknown place, no doors between)
 * Pure.
 */
function doorRoute(report, from, to, maxStep = 40) {
  const a = placeAt(report, from) || nearestPlace(report, from);
  const b = placeAt(report, to) || nearestPlace(report, to);
  if (!a || !b) return { waypoints: [], points: [], doors: [], error: "the report has no places" };
  // Breadth first over the places, doors both ways.
  const prev = new Map([[a.id, null]]);
  const queue = [a.id];
  while (queue.length && !prev.has(b.id)) {
    const at = queue.shift();
    for (const d of report.doors || []) {
      const [here, there, steps] = d.from === at ? [d.from, d.to, d.steps] : d.to === at ? [d.to, d.from, [d.steps[1], d.steps[0]]] : [null];
      if (!here || prev.has(there)) continue;
      prev.set(there, { from: here, door: d, steps });
      queue.push(there);
    }
  }
  if (!prev.has(b.id)) return { waypoints: [], points: [], doors: [], error: `no doors lead from ${a.id} to ${b.id}` };
  const crossings = [];
  for (let id = b.id; prev.get(id); id = prev.get(id).from) crossings.unshift(prev.get(id));
  const waypoints = [...crossings.flatMap((c) => c.steps.map((s) => s.slice())), to.slice()];
  return { waypoints, points: stepsAlong(from, waypoints, maxStep), doors: crossings.map((c) => `${c.door.from}->${c.door.to}`), error: null };
}

/** True when no leg of the walk `points` (its start first, ending at the
 *  line's start) can pass for the line in the judge: each leg is checked as
 *  `approachClear` checks a straight approach. Pure. */
function routeClear(points, line, limits = LIMITS) {
  const A = line.start.map(Number);
  const B = line.end.map(Number);
  const L = len(sub(B, A));
  const d = sub(B, A).map((x) => x / L);
  for (let i = 1; i < points.length; i++) {
    const [a, b] = [points[i - 1].map(Number), points[i].map(Number)];
    for (let k = 0; k <= 400; k++) {
      const { u, perp } = lineCoords(a.map((v, j) => v + ((b[j] - v) * k) / 400), A, d);
      if (perp <= limits.WINDOW_TOL_M && u >= limits.START_MARGIN_M) return false;
    }
  }
  return true;
}

/** Places on the floor of every shared zone of the report: each zone's four
 *  corners, a metre in from both walls, at eye height. The rig moves the game
 *  to the one farthest from its door (the step out and back, Respawn, the build
 *  editor). Pure. */
function farPlaces(report) {
  const out = [];
  for (const pl of (report.places || []).filter((p) => p.kind === "zone")) {
    const y = pl.min[1] + 1.7;
    for (const x of [pl.min[0] + 1, pl.max[0] - 1]) for (const z of [pl.min[2] + 1, pl.max[2] - 1]) out.push([x, y, z]);
  }
  return out;
}

/** The farthest of `places` from `from`, across the floor. */
function farthestFrom(places, from) {
  return places.reduce((a, b) => (Math.hypot(b[0] - from[0], b[2] - from[2]) > Math.hypot(a[0] - from[0], a[2] - from[2]) ? b : a));
}

/** The longest step the game is moved in on its way into the Commons, metres:
 *  the relay refuses any update more than 100 m from where it holds a player
 *  (the design's increment 2: "moved in steps of 90 m or less"). */
const MEET_MAX_STEP_M = 90;

/**
 * Judge the meeting in the Commons (verify-copresence --plots, increment 2)
 * beyond the walk itself (judgeCopresence, with the view, and the pictures, are
 * judged beside it). `meet`:
 *   commons      the Commons place from the door points ({ min, max })
 *   gameFrom     where the game started (its door)
 *   gameSteps    every point the game was moved to, in order, the meeting pose last
 *   gameCamera   the game's camera, read back at the meeting pose
 *   gameHeld     where the relay held the game when it got there: the last of
 *                its moves the walker (the first, still in its home) saw
 *   walkerTube   the walker's own door corridor ({ min, max }, from the door points)
 *   walkerDrawn  every position the game drew for the walker, in order
 *   walkerSawGame  where the walker (the one walking to the meeting) saw the
 *                game: the relay's word on joining ("player present") and any
 *                move after it
 * Checks: each step of the game's way in is one the relay accepts and the
 * relay holds it in the Commons; the walker came out through its own corridor
 * and into the Commons, as the game drew it; and the walker saw the game there.
 * Returns { pass, checks }.
 */
function judgeMeet(meet) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `meet_${id}`, ok: !!ok, detail });
  const m = meet || {};
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const d3 = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
  const inBox = (p, box, tol = 0.05) => Array.isArray(p) && box && [0, 2].every((k) => p[k] >= box.min[k] - tol && p[k] <= box.max[k] + tol);
  const steps = Array.isArray(m.gameSteps) ? m.gameSteps : [];
  let longest = 0;
  for (let i = 0; i < steps.length; i++) longest = Math.max(longest, d3(i ? steps[i - 1] : m.gameFrom, steps[i]));
  add(
    "game_steps_the_relay_accepts",
    steps.length > 0 && longest <= MEET_MAX_STEP_M,
    steps.length
      ? `the game was moved from its door ${fmt3(m.gameFrom)} in ${steps.length} steps, the longest ${longest.toFixed(1)} m (at most ${MEET_MAX_STEP_M} m)`
      : "the game was never moved",
  );
  add(
    "game_in_commons",
    inBox(m.gameCamera, m.commons),
    `the game's camera at ${fmt3(m.gameCamera)} is ${inBox(m.gameCamera, m.commons) ? "inside" : "OUTSIDE"} the Commons` +
      (m.commons ? ` (x ${m.commons.min[0]}..${m.commons.max[0]}, z ${m.commons.min[2]}..${m.commons.max[2]})` : ""),
  );
  const held = d3(m.gameHeld, m.gameCamera);
  add(
    "relay_holds_game_there",
    held <= 0.5,
    `the relay last passed the game on at ${fmt3(m.gameHeld)}, ${Number.isFinite(held) ? held.toFixed(2) : "?"} m from its camera (at most 0.5: a move refused leaves it behind)`,
  );
  const drawn = Array.isArray(m.walkerDrawn) ? m.walkerDrawn : [];
  const inTube = drawn.filter((p) => inBox(p, m.walkerTube, 0.05)).length;
  add(
    "walker_through_its_corridor",
    inTube > 0,
    m.walkerTube
      ? `${inTube} of ${drawn.length} drawn positions of the walker inside its own corridor (x ${m.walkerTube.min[0]}..${m.walkerTube.max[0]}, z ${m.walkerTube.min[2]}..${m.walkerTube.max[2]})`
      : "the walker's corridor is unknown",
  );
  const firstIn = drawn.findIndex((p) => inBox(p, m.commons));
  const tubeAt = drawn.findIndex((p) => inBox(p, m.walkerTube, 0.05));
  add(
    "walker_into_commons",
    firstIn >= 0 && tubeAt >= 0 && tubeAt < firstIn,
    firstIn < 0 ? "the walker was never drawn in the Commons" : `the walker was drawn in the Commons from frame ${firstIn}${tubeAt >= 0 ? `, after its corridor (frame ${tubeAt})` : ", never in its corridor first"}`,
  );
  const saw = Array.isArray(m.walkerSawGame) ? m.walkerSawGame : [];
  const last = saw.length ? saw[saw.length - 1] : null;
  const seen = d3(last, m.gameCamera);
  add(
    "walker_sees_game",
    !!last && seen <= 0.5 && inBox(last, m.commons),
    last
      ? `the walker last saw the game at ${fmt3(last)}, ${seen.toFixed(2)} m from where it stands, ${inBox(last, m.commons) ? "in" : "NOT in"} the Commons`
      : "the walker never saw the game",
  );
  return { pass: checks.every((c) => c.ok), checks };
}

/**
 * Judge the second boot against the same relay (verify-copresence --plots, the
 * remembered plot of increment 2). `reboot`:
 *   heldPlot     the plot the game held before it restarted
 *   defaultPlot  the ship's default plot
 *   bootPlot     the plot the world load built the home on (the probe's boot_plot)
 *   lastWelcome  what the welcome then did with the home ("stay", "move", ...)
 *   camera, plot the game's camera after the welcome, and the held plot's box
 * Returns { pass, checks }.
 */
function judgeReboot(reboot) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `reboot_${id}`, ok: !!ok, detail });
  const r = reboot || {};
  const tells = r.heldPlot && r.heldPlot !== r.defaultPlot;
  add(
    "built_on_its_plot",
    !!r.heldPlot && r.bootPlot === r.heldPlot,
    `the second boot built the home on ${r.bootPlot || "(none reported)"} before joining; it held ${r.heldPlot || "(unknown)"}` +
      (tells ? "" : ` (the ship's default plot: this order cannot tell a remembered plot from the default)`),
  );
  add("welcome_confirms", r.lastWelcome === "stay", `its welcome ${r.lastWelcome === "stay" ? "only confirmed the plot (a Stay)" : `did "${r.lastWelcome}"`}`);
  const inPlot = Array.isArray(r.camera) && r.plot && [0, 2].every((k) => r.camera[k] >= r.plot.origin[k] - 0.01 && r.camera[k] <= r.plot.origin[k] + r.plot.size[k] + 0.01);
  add("stands_on_its_plot", inPlot, `after the welcome the camera is at ${Array.isArray(r.camera) ? `(${r.camera.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)"}, ${inPlot ? "on" : "NOT on"} ${r.heldPlot}`);
  return { pass: checks.every((c) => c.ok), checks };
}

// ── Is the figure VISIBLE in a screenshot? ──────────────────────────────────
//
// The samples prove where the game drew the figure; a screenshot can still
// show nothing, because something is drawn over it. That happened on the
// rig's third run (2026-10-03): a first-run "Choose your privacy" window sat
// over the middle of the view, both screenshots showed the window, the HUD
// said "1 here: TestBotCrosser", and every sample check passed. Only a person
// looking at the pictures could tell. This counts the figure's own colour in
// the picture instead.

/// Another player's body is teal (lib.rs "Remote players": base colour
/// [0.15, 0.75, 0.85], slightly emissive). Measured lit in the vehicle bay on
/// 2026-10-03: (80, 193, 204); the floor there reads (181, 186, 192) and the
/// wall (61, 72, 87), neither of which passes. Relative, not absolute, so a
/// dimmer light still reads as teal.
function isFigureTeal(r, g, b) {
  return g > 80 && g - r > 50 && b - r > 50 && Math.abs(g - b) < 60;
}

/// Fewest teal pixels that count as the figure being seen. At the rig's 6 m
/// the body is about 42 x 175 pixels in a 2560 x 1387 picture (6,771 counted
/// on 2026-10-03), so this is about a fifth of it: a figure partly behind
/// something still passes, a covered or absent one reads near zero.
const FIGURE_MIN_PX = 1500;

/// Count figure-teal pixels in `img` ({ width, height, rgba }, scripts/lib/
/// png.js decode). With `nameplate` ([x, y], window pixels, where the game
/// drew the walker's name), only a box under it is counted: 150 px either
/// side (the figure moves a little between asking and capturing) and 450 px
/// down from the name. Without it, the whole picture. Returns { count, box,
/// centroid } (centroid null when nothing counted).
function figurePixels(img, nameplate = null) {
  const box = nameplate
    ? [Math.max(0, Math.round(nameplate[0] - 150)), Math.max(0, Math.round(nameplate[1])), Math.min(img.width, Math.round(nameplate[0] + 150)), Math.min(img.height, Math.round(nameplate[1] + 450))]
    : [0, 0, img.width, img.height];
  let count = 0;
  let sx = 0;
  let sy = 0;
  for (let y = box[1]; y < box[3]; y++) {
    for (let x = box[0]; x < box[2]; x++) {
      const i = (y * img.width + x) * 4;
      if (isFigureTeal(img.rgba[i], img.rgba[i + 1], img.rgba[i + 2])) {
        count++;
        sx += x;
        sy += y;
      }
    }
  }
  return { count, box, centroid: count ? [sx / count, sy / count] : null };
}

module.exports = {
  LIMITS,
  judgeCopresence,
  lineCoords,
  viewAngle,
  isFigureTeal,
  figurePixels,
  FIGURE_MIN_PX,
  approachClear,
  forwardLegStart,
  readShipPlots,
  inPlot,
  nearestPlot,
  judgePlots,
  judgeRejoin,
  judgeEditorClose,
  judgeEntry,
  placeAt,
  doorRoute,
  routeClear,
  farPlaces,
  farthestFrom,
  judgeMeet,
  judgeReboot,
  MEET_MAX_STEP_M,
  REJOIN_FAR_M,
};
