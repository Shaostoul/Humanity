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
  // Asked for, the view is always judged: a recording with no camera on any frame of the pass
  // (a recorder that dropped or renamed `cam`) FAILS here instead of leaving the check out
  // (increment 2 review, finding 11: the run passed with one check fewer).
  if (checkView && !pass.some((s) => s.cam)) {
    add("in_view", false, "the recording carries no camera, so the view cannot be judged");
  } else if (checkView) {
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
 *              floor, the points the game walks through (the showcase `walk_to`
 *              verb; until increment 4 it was moved there with `cam` teleports
 *              the relay's old 100 m rule let through)
 *   doors      "from->to" for each door crossed
 *   error      why there is no route (an unknown place, no doors between)
 * Inside a place each leg is a straight line. When the report carries its
 * walls (`walls`, every wall a person walks against) a leg that would cross one
 * goes round by one corner instead, along x then z or along z then x, whichever
 * is clear (`detourRound`): the game's way in from p1 went straight through the
 * Commons' room block (increment 2 review, finding 14; found by `routeWalls`).
 * A leg with no clear corner stays straight, and `routeWalls` then says so.
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
  const straight = [...crossings.flatMap((c) => c.steps.map((s) => s.slice())), to.slice()];
  const waypoints = [];
  let at = from;
  for (const p of straight) {
    waypoints.push(...detourRound(at, p, report.walls, report));
    at = p;
  }
  return { waypoints, points: stepsAlong(from, waypoints, maxStep), doors: crossings.map((c) => `${c.door.from}->${c.door.to}`), error: null };
}

/** The points to walk from `a` to `b` (b last): [b] when the straight leg crosses
 *  none of `walls`, else round one corner, [corner, b], along x first or along z
 *  first, whichever crosses none; else a path through the place found on a grid
 *  that keeps clear of every wall (`clearPath`, the review of increment 4: the
 *  Respawn walk from p2 to the Commons' far corner had no clear corner, went
 *  straight at the room block's wall, and stopped there; the rig only knew once it
 *  checked that every walk arrives); [b] when none is found (`doorRoute`). Pure. */
function detourRound(a, b, walls, report = null) {
  if (!Array.isArray(walls) || !routeWalls([a, b], walls)) return [b];
  for (const corner of [[b[0], a[1], a[2]], [a[0], a[1], b[2]]]) {
    if (!routeWalls([a, corner, b], walls)) return [corner, b];
  }
  return clearPath(a, b, walls, { inside: report ? (q) => aboardFloor(report, q) : null }) || [b];
}

/** True when `p` stands, across the floor, inside one of the report's places or a door's tube:
 *  where a walk aboard may go (a path found on a grid never leaves the ship through a doorway
 *  and round the outside). Pure. */
function aboardFloor(report, p) {
  const inBox = (min, max) => p[0] >= min[0] && p[0] <= max[0] && p[2] >= min[2] && p[2] <= max[2];
  return (report.places || []).some((pl) => inBox(pl.min, pl.max)) || (report.doors || []).some((d) => Array.isArray(d.tube) && inBox(d.tube[0], d.tube[1]));
}

/** How far from every wall a path `clearPath` finds keeps, metres: the player's 0.3 m and a
 *  margin, so the game's collision never pushes a walk off its line (a push stops a walk short). */
const PATH_CLEAR_M = 0.75;

/** A walk from `a` to `b` (b last) that crosses no wall and keeps PATH_CLEAR_M from every wall,
 *  found on a grid of `cellM` over the box around both (and `padM` beyond), the start and end
 *  exempt, then cut to as few straight legs as stay clear. null when there is none. Pure. */
function clearPath(a, b, walls, { cellM = 0.5, padM = 40, clearM = PATH_CLEAR_M, inside = null } = {}) {
  const lo = [Math.min(a[0], b[0]) - padM, Math.min(a[2], b[2]) - padM];
  const hi = [Math.max(a[0], b[0]) + padM, Math.max(a[2], b[2]) + padM];
  const nx = Math.ceil((hi[0] - lo[0]) / cellM) + 1;
  const nz = Math.ceil((hi[1] - lo[1]) / cellM) + 1;
  const at = (i, k) => [lo[0] + i * cellM, a[1], lo[1] + k * cellM];
  const cellOf = (p) => [Math.round((p[0] - lo[0]) / cellM), Math.round((p[2] - lo[1]) / cellM)];
  const near = (walls || []).filter((w) => Math.max(w[0], w[2]) >= lo[0] && Math.min(w[0], w[2]) <= hi[0] && Math.max(w[1], w[3]) >= lo[1] && Math.min(w[1], w[3]) <= hi[1]);
  const clearAt = (p) => (!inside || inside(p)) && near.every((w) => wallDistXZ(p, w) >= clearM);
  const [si, sk] = cellOf(a);
  const [ti, tk] = cellOf(b);
  const key = (i, k) => k * nx + i;
  const free = new Map();
  const isFree = (i, k) => {
    if (i < 0 || k < 0 || i >= nx || k >= nz) return false;
    if ((i === si && k === sk) || (i === ti && k === tk)) return true;
    const kk = key(i, k);
    if (!free.has(kk)) free.set(kk, clearAt(at(i, k)));
    return free.get(kk);
  };
  const prev = new Map([[key(si, sk), -1]]);
  const queue = [[si, sk]];
  const steps = [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [1, -1], [-1, 1], [-1, -1]];
  while (queue.length && !prev.has(key(ti, tk))) {
    const [i, k] = queue.shift();
    for (const [di, dk] of steps) {
      const [j, l] = [i + di, k + dk];
      if (prev.has(key(j, l)) || !isFree(j, l)) continue;
      // A diagonal step only past two free sides, never through a wall's corner.
      if (di && dk && !(isFree(i + di, k) && isFree(i, k + dk))) continue;
      prev.set(key(j, l), key(i, k));
      queue.push([j, l]);
    }
  }
  if (!prev.has(key(ti, tk))) return null;
  const cells = [];
  for (let c = key(ti, tk); c !== -1; c = prev.get(c)) cells.unshift([c % nx, Math.floor(c / nx)]);
  const pts = cells.map(([i, k]) => at(i, k));
  pts[0] = a.slice();
  pts[pts.length - 1] = b.slice();
  // Cut to straight legs: from each point, the farthest later one reached by a clear leg (sampled
  // every quarter metre; the start and end may sit nearer a wall than the rest).
  const legClear = (p, q) => {
    if (routeWalls([p, q], near)) return false;
    const n = Math.max(1, Math.ceil(Math.hypot(q[0] - p[0], q[2] - p[2]) / 0.25));
    for (let s = 1; s < n; s++) {
      const x = [p[0] + ((q[0] - p[0]) * s) / n, p[1], p[2] + ((q[2] - p[2]) * s) / n];
      if (!clearAt(x) && Math.hypot(x[0] - a[0], x[2] - a[2]) > clearM && Math.hypot(x[0] - b[0], x[2] - b[2]) > clearM) return false;
    }
    return true;
  };
  const out = [];
  for (let i = 0; i < pts.length - 1; ) {
    let j = pts.length - 1;
    while (j > i + 1 && !legClear(pts[i], pts[j])) j--;
    out.push(pts[j]);
    i = j;
  }
  return out;
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

/** The first wall of `walls` (the door points' `walls`: [ax, az, bx, bz] each, ship metres
 *  across the floor) that the walk `points` crosses, as { leg, from, to, wall }, or null.
 *  A proper crossing only: touching a wall's end, or running along one, does not count, which
 *  errs toward passing a route that grazes a doorway's edge. Inside a place a route is a
 *  straight line between its steps (doorRoute), so this is what says it does not walk through
 *  an interior wall a Dev edit put across it (increment 2 review, finding 14). Pure. */
function routeWalls(points, walls) {
  const orient = (ax, az, bx, bz, cx, cz) => (bx - ax) * (cz - az) - (bz - az) * (cx - ax);
  for (let i = 1; i < (points || []).length; i++) {
    const [p, q] = [points[i - 1].map(Number), points[i].map(Number)];
    for (const w of walls || []) {
      const d1 = orient(p[0], p[2], q[0], q[2], w[0], w[1]);
      const d2 = orient(p[0], p[2], q[0], q[2], w[2], w[3]);
      const d3 = orient(w[0], w[1], w[2], w[3], p[0], p[2]);
      const d4 = orient(w[0], w[1], w[2], w[3], q[0], q[2]);
      if (d1 * d2 < 0 && d3 * d4 < 0) return { leg: i, from: p, to: q, wall: w };
    }
  }
  return null;
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
  // The PLAN: no step longer than the relay's rule allows (the 90 m the design names). What the
  // relay did with each is the next check (increment 2 review, finding 12: this one alone, named
  // as if the relay had accepted each step, read nothing the relay said).
  add(
    "game_steps_planned_short",
    steps.length > 0 && longest <= MEET_MAX_STEP_M,
    steps.length
      ? `the game's way in was planned from its door ${fmt3(m.gameFrom)} in ${steps.length} steps, the longest ${longest.toFixed(1)} m (at most ${MEET_MAX_STEP_M} m)`
      : "the game was never moved",
  );
  // Each planned step, in order, among the positions the relay passed on for the game (the
  // first walker, at home, logs them): a step the relay turned down would leave the game
  // stranded behind it, whatever the plan said.
  const relayed = Array.isArray(m.gameRelayed) ? m.gameRelayed : null;
  let from = 0;
  const missed = [];
  for (const s of steps) {
    const at = relayed ? relayed.findIndex((r, i) => i >= from && d3(r, s) <= 0.5) : -1;
    if (at < 0) missed.push(s);
    else from = at;
  }
  add(
    "game_steps_relayed",
    !!relayed && steps.length > 0 && missed.length === 0,
    !relayed
      ? "what the relay passed on for the game was not recorded"
      : missed.length
        ? `${missed.length} of ${steps.length} planned steps never passed on by the relay in order (first ${fmt3(missed[0])}); it passed on ${relayed.length} position(s)`
        : `the relay passed on every one of the ${steps.length} planned steps, in order (${relayed.length} position(s) logged)`,
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
  // Nothing crosses a wall (finding 14): the game's way in, the walker's way out, and the camera's
  // view of the line, against every wall a person walks against (the door points' `walls`).
  const walls = Array.isArray(m.walls) ? m.walls : null;
  const line = m.line || null;
  const mid = line ? line.start.map((v, k) => (Number(v) + Number(line.end[k])) / 2) : null;
  const crossings = walls
    ? [
        ["the game's way in", routeWalls([m.gameFrom, ...steps].filter(Array.isArray), walls)],
        ["the walker's way out", routeWalls(Array.isArray(m.walker_route) ? m.walker_route : [], walls)],
        ...(line && Array.isArray(m.gameCamera) ? [line.start, mid, line.end].map((p) => ["the camera's view of the line", routeWalls([m.gameCamera, p], walls)]) : []),
      ].filter(([, hit]) => hit)
    : [];
  add(
    "routes_clear_of_walls",
    !!walls && !!line && Array.isArray(m.walker_route) && crossings.length === 0,
    !walls
      ? "the walls were not recorded (the door points carry them)"
      : !line || !Array.isArray(m.walker_route)
        ? "the line or the walker's route was not recorded"
        : crossings.length
          ? `${crossings[0][0]} crosses the wall (${crossings[0][1].wall.map((v) => Number(v).toFixed(2)).join(", ")}) between ${fmt3(crossings[0][1].from)} and ${fmt3(crossings[0][1].to)}`
          : `the game's way in, the walker's way out and the camera's view of the line cross none of the ${walls.length} walls`,
  );
  // The walker came back into the world AT ITS DOOR (finding 13): its join named the door
  // (--home-spawn), and the relay must have spawned it there, not in the middle of its plot.
  const start = m.walker && m.walker.start;
  const fromDoor = d3(start, m.walkerDoor);
  add(
    "walker_from_its_door",
    fromDoor <= 0.5,
    `the relay spawned the walker at ${fmt3(start)}, ${Number.isFinite(fromDoor) ? fromDoor.toFixed(2) : "?"} m from its door ${fmt3(m.walkerDoor)} (at most 0.5)`,
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

// ── The guest (verify-copresence --plots --order guest, the increment 2 review, finding 2) ──
//
// With the shipped ship's two plots held by two scripted players, the game comes
// in third, a guest: its home is put away off the ship (ShipStructure::
// put_home_away), every plot is drawn as a neighbour's, it stands in the Commons,
// and it cannot build. Until this leg no rig had a guest in it: the put-away and
// the bring-back (rebuild, hull, room GI, collision, machines, animals, plants,
// vehicles and built pieces carried there and back) were proven only by pure
// planners and unit tests, and the commonest guest case (the third person to join)
// was the review's finding 1.

/** The start of the sentence a guest reads when it presses B
 *  (engine/home_plot.rs GUEST_NO_EDITOR; a Rust test keeps the two equal). */
const GUEST_NO_EDITOR_START = "You are a guest on this ship, with no plot of your own";

/** True when `p` stands on a plot's ground: over a plot's box across the floor
 *  (x and z), or inside the door corridor of a plot (a door point whose `from` is
 *  a plot), as engine/home_plot.rs `on_plot_ground` tests it. `plots` are the
 *  probe's ({ id, origin, size }), `doors` the door points'. Pure. */
function onPlotGround(p, plots, doors, tol = 0.01) {
  if (!Array.isArray(p)) return false;
  const inXZ = (lo, hi) => p[0] >= lo[0] - tol && p[0] <= hi[0] + tol && p[2] >= lo[2] - tol && p[2] <= hi[2] + tol;
  const onPlot = (plots || []).some((pl) => inXZ(pl.origin, [0, 1, 2].map((k) => pl.origin[k] + pl.size[k])));
  const inTube = (doors || []).filter((d) => String(d.from).startsWith("plot:")).some((d) => inXZ(d.tube[0], d.tube[1]));
  return onPlot || inTube;
}

/** Every thing of the home in a probe's `home_things`, but the Respawn point:
 *  [label, [x, y, z]] each. */
function homeThingsOf(things) {
  if (!things) return [];
  const one = (label, p) => (Array.isArray(p) ? [[label, p]] : []);
  const many = (label, list) => (Array.isArray(list) ? list.map((p, i) => [`${label} ${i + 1}`, p]) : []);
  return [
    ...one("the hologram", things.hologram),
    ...one("the showroom stage", things.showroom),
    ...many("animal", things.animals),
    ...many("plant", things.plants),
    ...many("built piece", things.structures),
    ...many("vehicle", things.vehicles),
  ];
}

/**
 * Judge a guest run. `g` (the manifest's `guest`):
 *   plots, doors   the ship's plots (probe) and door points' doors
 *   commons        the Commons place ({ min, max })
 *   defaultPlot    the ship's default plot id (a guest's home comes back there)
 *   walkers        [{ name, id, plot }]: the two scripted players holding the plots
 *   arrived        the probe after the guest's welcome: { lastWelcome, homePlot,
 *                  homeAway, camera, homeThings }
 *   editor         after B was pressed: { open, notices }
 *   respawn        the Respawn leg, as judgeRejoin takes it (far: where the relay
 *                  held the guest at the far end of First Street)
 *   back           after stepping out of the shared world: { homeAway, homePlot,
 *                  homeThings }
 *   again          after stepping back in: { lastWelcome, homeAway, camera }
 *   reconnect      the dropped connection: { held (where the relay held the guest),
 *                  during: { joined, homeAway, homePlot } (the home came back),
 *                  before (the camera, walked into the home that came back),
 *                  after: { lastWelcome, rejoin, homeAway, camera }, nudged, seen }
 * Returns { pass, checks }; every id starts guest_.
 */
function judgeGuest(guest) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `guest_${id}`, ok: !!ok, detail });
  const g = guest || {};
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const d3 = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
  const inCommons = (p) => Array.isArray(p) && g.commons && [0, 2].every((k) => p[k] >= g.commons.min[k] - 0.05 && p[k] <= g.commons.max[k] + 0.05);
  const onPlot = (p) => onPlotGround(p, g.plots, g.doors);
  const plotIds = (g.plots || []).map((p) => p.id).sort();
  const held = (g.walkers || []).map((w) => w.plot).filter(Boolean).sort();
  add(
    "plots_taken",
    plotIds.length > 0 && JSON.stringify(held) === JSON.stringify(plotIds),
    `the scripted players hold ${held.join(", ") || "nothing"}; the ship's plots are ${plotIds.join(", ") || "(unknown)"}` +
      (JSON.stringify(held) === JSON.stringify(plotIds) ? ", so the game comes in third, with none left" : ": a plot is left, so the game is no guest"),
  );
  const a = g.arrived || {};
  add(
    "welcome",
    a.lastWelcome === "guest" && a.homePlot === null && a.homeAway === true,
    `its welcome did "${a.lastWelcome}", the home stands on ${a.homePlot ? a.homePlot.id || a.homePlot : "no plot"} and is ${a.homeAway ? "put away" : "NOT put away"}`,
  );
  add("in_commons", inCommons(a.camera), `the guest's camera at ${fmt3(a.camera)} is ${inCommons(a.camera) ? "in" : "NOT in"} the Commons`);
  const respawnPoint = a.homeThings && a.homeThings.respawn;
  add("respawn_point", inCommons(respawnPoint), `its Respawn point ${fmt3(respawnPoint)} is ${inCommons(respawnPoint) ? "in" : "NOT in"} the Commons`);
  const things = homeThingsOf(a.homeThings);
  const onAPlot = things.filter(([, p]) => onPlot(p));
  add(
    "nothing_on_plots",
    !!a.homeThings && things.length > 0 && onAPlot.length === 0,
    !a.homeThings
      ? "where the home's things stand was not recorded"
      : onAPlot.length
        ? `${onAPlot.length} of the home's ${things.length} things stand on a plot, first ${onAPlot[0][0]} at ${fmt3(onAPlot[0][1])}`
        : `none of the home's ${things.length} things (its hologram, showroom stage, animals, plants, built pieces and vehicles) stands on a plot`,
  );
  const e = g.editor || {};
  const told = (e.notices || []).some((n) => String(n).startsWith(GUEST_NO_EDITOR_START));
  add(
    "no_editor",
    e.open === false && told,
    `after B the build editor is ${e.open === false ? "shut" : e.open === true ? "OPEN" : "unknown"}; ${told ? "the guest was told why" : `no notice says why (on screen: ${JSON.stringify(e.notices || [])})`}`,
  );
  for (const c of judgeRejoin(g.respawn || {}, { prefix: "guest_respawn", when: "it pressed Respawn as a guest" }).checks) checks.push(c);
  const rs = g.respawn || {};
  add("respawn_in_commons", inCommons(rs.relaySpawn) && inCommons(rs.camera), `after Respawn the relay spawned the guest at ${fmt3(rs.relaySpawn)} and its camera stands at ${fmt3(rs.camera)}, ${inCommons(rs.relaySpawn) && inCommons(rs.camera) ? "both in" : "NOT both in"} the Commons`);
  const b = g.back || {};
  const def = (g.plots || []).find((p) => p.id === g.defaultPlot);
  const backThings = homeThingsOf(b.homeThings);
  // Nearer the default plot than any other, as judgePlots judges a home's things, and within a
  // metre of its box: the hologram hangs half a metre outside the home's west wall (the first
  // guest run, 20261004-120512, failed on it with "inside"), and a thing left where the home was
  // kept is a kilometre away.
  const onDefault = (p) => !!def && Array.isArray(p) && nearestPlot(p.map(Number), g.plots) === def && footprintGap(p.map(Number), def) <= 1;
  const strays = [...backThings, ["its Respawn point", b.homeThings && b.homeThings.respawn]].filter(([, p]) => !onDefault(p));
  add(
    "home_back",
    b.homeAway === false && (b.homePlot ? b.homePlot.id || b.homePlot : null) === g.defaultPlot && backThings.length === things.length && strays.length === 0,
    `out of the shared world the home is ${b.homeAway === false ? "back" : "STILL AWAY"} on ${b.homePlot ? b.homePlot.id || b.homePlot : "no plot"} (the default is ${g.defaultPlot}), with ${backThings.length} of its ${things.length} things` +
      (strays.length ? `; ${strays[0][0]} stands off it at ${fmt3(strays[0][1])}` : ", every one on the default plot with the Respawn point"),
  );
  const ag = g.again || {};
  add(
    "away_again",
    ag.lastWelcome === "guest" && ag.homeAway === true && inCommons(ag.camera),
    `back in the shared world its welcome did "${ag.lastWelcome}", the home is ${ag.homeAway ? "put away" : "NOT put away"} again, the camera at ${fmt3(ag.camera)} ${inCommons(ag.camera) ? "in" : "NOT in"} the Commons`,
  );
  const r = g.reconnect || {};
  const du = r.during || {};
  const af = r.after || {};
  const setup = du.joined === false && du.homeAway === false && du.homePlot === g.defaultPlot && onPlot(r.before) && af.rejoin === true;
  add(
    "reconnect_setup",
    setup,
    `when the connection dropped the game ${du.joined === false ? "left the shared world" : "did NOT leave the shared world"} and its home came ${du.homeAway === false ? "back" : "NOT back"} on ${du.homePlot || "no plot"}; the camera walked into it, to ${fmt3(r.before)} (${onPlot(r.before) ? "on" : "NOT on"} a plot); the welcome on reconnecting said rejoin ${af.rejoin}` +
      (af.rejoin === true ? " (inside the relay's grace)" : ": NOT a reconnect inside the grace, so this leg proves nothing about it"),
  );
  const gap = d3(af.camera, r.held);
  add(
    "reconnect_off_plot",
    af.lastWelcome === "guest" && af.homeAway === true && !onPlot(af.camera) && gap <= REJOIN_STAND_TOL_M,
    `after the reconnect's welcome ("${af.lastWelcome}", the home ${af.homeAway ? "put away" : "NOT put away"}) the camera stands at ${fmt3(af.camera)}, ${onPlot(af.camera) ? "ON A PLOT (a neighbour's home, with no walls to stop it)" : "on no plot"}, ${Number.isFinite(gap) ? gap.toFixed(2) : "?"} m from where the relay holds the guest ${fmt3(r.held)} (at most ${REJOIN_STAND_TOL_M})`,
  );
  const hit = (r.seen || []).find((p) => d3(p, r.nudged) <= REJOIN_NUDGE_TOL_M);
  add(
    "reconnect_moves_reach_others",
    !!hit,
    hit ? `its next move to ${fmt3(r.nudged)} reached the others through the relay, at ${fmt3(hit)}` : `its next move to ${fmt3(r.nudged)} never reached the others (${(r.seen || []).length} update(s) seen after it)`,
  );
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
/// down from the name. With `after` too (where the name was drawn when asked
/// again right after the capture), the box spans both, 150 px either side: on
/// a busy machine the capture can land a second or more after the first ask,
/// and the figure walks 200 px in that time (2026-10-04, increment 2's
/// meeting: the name asked at x 1505, the figure captured at x 1286). Without
/// a nameplate, the whole picture. Returns { count, box, centroid } (centroid
/// null when nothing counted).
function figurePixels(img, nameplate = null, after = null) {
  const xs = [nameplate, after].filter(Boolean).map((p) => p[0]);
  const box = nameplate
    ? [Math.max(0, Math.round(Math.min(...xs) - 150)), Math.max(0, Math.round(nameplate[1])), Math.min(img.width, Math.round(Math.max(...xs) + 150)), Math.min(img.height, Math.round(nameplate[1] + 450))]
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

// ── The crew (ship homes increment 3) ──────────────────────────────────────
//
// The relay's world is the ship now: its crew work the Commons and its mess hall, in the same
// frame the game draws, and the game draws each crew member where net::sync puts them (the
// recorder's `crew` rows: the DRAWN position, on screen or off). Until increment 3 the relay
// simulated the Pioneer, whose rooms lie inside plot p1, and every crew figure was drawn inside
// the home on p1.

/// Is this pixel a crew member's amber body AS THE GAME DRAWS IT? The crew's material is
/// (0.92, 0.62, 0.18), slightly emissive (lib.rs, "Crew NPCs"), but lit and tone-mapped in the
/// Commons it comes out about (176, 151, 71): measured on the first crew.png (2026-10-04, run
/// 20261004-155649-plots-game-first, the body of Botanist Yara 5 m away), green about 0.86 of
/// red and blue about 0.40. So: bright enough, green 0.78 to 0.95 of red, blue 0.25 to 0.52.
/// Not counted: the HUD's orange activity line under a crew member's name (223, 131, 33: green
/// 0.59 of red, which the first test, written from the material colour, counted instead of the
/// body), oak as the Commons draws it (dark, and blue 0.6 of red), the floor and walls (grey,
/// green above red), white lamps and the sun, and the players' teal. On that picture the test
/// found 17,302 pixels, all but 19 of them on the crew's figures.
function isFigureAmber(r, g, b) {
  return r > 110 && g >= 0.78 * r && g <= 0.95 * r && b >= 0.25 * r && b <= 0.52 * r;
}

/// Fewest crew-amber pixels under a crew member's nameplate that count as a crew figure seen
/// there. A crew member 35 m away in the first crew.png is about 10 x 40 px of body; 150 is
/// well under that and far over the 19 stray pixels the whole rest of that picture held.
const CREW_MIN_PX = 150;

/// Count crew-amber pixels in `img` in the box under a crew member's nameplate at `nameplate`
/// ([x, y], window pixels): 120 px either side and 400 px down (a crew figure stands up to 40 m
/// away, where it is small). Returns { count, box }.
function crewPixels(img, nameplate) {
  const box = [
    Math.max(0, Math.round(nameplate[0] - 120)),
    Math.max(0, Math.round(nameplate[1])),
    Math.min(img.width, Math.round(nameplate[0] + 120)),
    Math.min(img.height, Math.round(nameplate[1] + 400)),
  ];
  let count = 0;
  for (let y = box[1]; y < box[3]; y++) {
    for (let x = box[0]; x < box[2]; x++) {
      const i = (y * img.width + x) * 4;
      if (isFigureAmber(img.rgba[i], img.rgba[i + 1], img.rgba[i + 2])) count++;
    }
  }
  return { count, box };
}

/** Is `p` inside a box { min, max } across the floor (x and z), a centimetre of slack? */
function inBoxXZ(p, box, tol = 0.01) {
  return p[0] >= box.min[0] - tol && p[0] <= box.max[0] + tol && p[2] >= box.min[2] - tol && p[2] <= box.max[2] + tol;
}

/** How close a crew figure's middle may come to a wall's centre line, metres: half a body's
 *  width, the same 0.3 m the relay's own wall test keeps every crew walk clear by (beyond the
 *  wall's half thickness, which the door points' walls do not carry). */
const CREW_WALL_CLEAR_M = 0.3;

/** Distance across the floor from `p` ([x, y, z]) to the wall `w` ([ax, az, bx, bz]). */
function wallDistXZ(p, w) {
  const [ax, az, bx, bz] = w.map(Number);
  const dx = bx - ax;
  const dz = bz - az;
  const l2 = dx * dx + dz * dz;
  const t = l2 < 1e-12 ? 0 : Math.max(0, Math.min(1, ((p[0] - ax) * dx + (p[2] - az) * dz) / l2));
  return Math.hypot(p[0] - ax - dx * t, p[2] - az - dz * t);
}

/** Is `p` on a plot's floor plan (x and z; a figure over a plot at any height is on it)? */
function onPlotXZ(p, plot, tol = 0.01) {
  return p[0] >= plot.origin[0] - tol && p[0] <= plot.origin[0] + plot.size[0] + tol && p[2] >= plot.origin[2] - tol && p[2] <= plot.origin[2] + plot.size[2] + tol;
}

/**
 * Judge the crew, as the game drew them (ship homes increment 3).
 *   recordings  [{ name, frames }]: the run's recordings (the walk at home, the meeting, the
 *               crew look). Each frame's `crew`: [{ id, name, pos }], every crew member drawn.
 *   plots       the ship's plots ({ id, origin, size }).
 *   commons     the Commons' box from the door points ({ min, max }).
 *   walls       every wall a person walks against, from the door points ([ax, az, bx, bz] each).
 *               A recording with `after` set (the step back in, Respawn) must also draw the
 *               whole crew.
 *   expectCrew  how many crew the tree's data/npc/crew.ron has (the relay stands every one).
 *   look        the crew look: the game in the Commons, facing up its east aisle toward the
 *               mess hall. { cam: [x, y, z, yaw, pitch] (where it stood), frames (its own
 *               recording), seen: [{ name, pos, found, pos_px, amber }] (where each crew member
 *               was drawn when the rig looked, else the recording's first frame; the ui `find` of each drawn
 *               crew member's nameplate, the game's own word that the name is on screen, which
 *               the HUD hides past 40 m and behind a wall, and the amber pixels counted under
 *               it in crew.png) }.
 * Checks:
 *   crew_recorded        every recording's frames carry a `crew` list: a recorder from before
 *                        increment 3 records none, and nothing would be judged
 *   crew_drawn           the game drew every crew member (distinct ids) at some point
 *   crew_never_on_a_plot no crew figure in any frame of any recording stands on a plot
 *   crew_clear_of_walls  no crew figure in any frame of any recording stands within
 *                        CREW_WALL_CLEAR_M of a wall: the relay's test judges the walks it
 *                        means, this judges where the game DRAWS them (the review of
 *                        increment 3, finding 15: a chore spot behind the room block's walls
 *                        passed every other crew check, inside the Commons and on no plot)
 *   crew_back_after_rejoin  each recording made after the step back into the world and after
 *                        Respawn (net::sync drops every crew figure and stands it again) draws
 *                        the whole crew (finding 16)
 *   crew_in_the_commons  in the look's recording every crew figure stands in the Commons
 *   crew_seen            AT LEAST ONE CREW FIGURE VISIBLE in the look's picture: a crew member's
 *                        nameplate on screen, its figure drawn in the Commons in front of the
 *                        camera, and a crew figure's amber body in the picture under the name
 *                        (CREW_MIN_PX). Under the name, not necessarily that crew member's own
 *                        body: crew standing in a line up the aisle share a column, and the
 *                        nearest one's body fills every box (the first runs counted the same
 *                        ~16,000 px under five names), so this says one figure is seen, not
 *                        which. The positions and names are the ones of the moment the picture
 *                        was taken (the rig asks straight after it).
 * Returns { pass, checks }.
 */
function judgeCrew({ recordings, plots, commons, walls, expectCrew, look }) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const recs = (recordings || []).filter((r) => r && Array.isArray(r.frames) && r.frames.length);
  const missing = recs.filter((r) => !r.frames.every((f) => Array.isArray(f.crew)));
  add(
    "crew_recorded",
    recs.length > 0 && missing.length === 0,
    !recs.length
      ? "no recording to judge"
      : missing.length
        ? `${missing.map((r) => r.name).join(", ")}: frames with no crew list (a recorder from before increment 3)`
        : `${recs.length} recordings (${recs.map((r) => `${r.name} ${r.frames.length} frames`).join(", ")}), every frame with its crew`,
  );
  const ids = new Map();
  let rows = 0;
  const onPlot = [];
  const inWall = [];
  const wallList = Array.isArray(walls) ? walls : [];
  for (const r of recs) {
    for (const f of r.frames) {
      for (const c of f.crew || []) {
        rows++;
        ids.set(c.id, c.name);
        const pos = c.pos.map(Number);
        const pl = (plots || []).find((p) => onPlotXZ(pos, p));
        if (pl && onPlot.length < 5) onPlot.push({ rec: r.name, t: Number(f.t), name: c.name, pos, plot: pl.id });
        else if (pl) onPlot.push(null);
        const w = wallList.find((w) => wallDistXZ(pos, w) < CREW_WALL_CLEAR_M);
        if (w && inWall.length < 5) inWall.push({ rec: r.name, t: Number(f.t), name: c.name, pos, wall: w, d: wallDistXZ(pos, w) });
        else if (w) inWall.push(null);
      }
    }
  }
  add(
    "crew_drawn",
    Number.isFinite(expectCrew) && expectCrew > 0 && ids.size >= expectCrew,
    `the game drew ${ids.size} crew member(s) (${[...ids.values()].join(", ") || "none"}); the tree's crew.ron has ${expectCrew}`,
  );
  add(
    "crew_never_on_a_plot",
    rows > 0 && onPlot.length === 0 && Array.isArray(plots) && plots.length > 0,
    !rows
      ? "no crew figure was drawn, so none was judged"
      : !Array.isArray(plots) || !plots.length
        ? "the ship's plots are unknown"
        : onPlot.length
          ? `${onPlot.length} drawn crew position(s) ON A PLOT; first: ${onPlot
              .filter(Boolean)
              .slice(0, 3)
              .map((o) => `${o.name} at ${fmtP(o.pos)} on ${o.plot} (${o.rec}, t ${f(o.t)} s)`)
              .join("; ")}`
          : `all ${rows} drawn crew positions (${ids.size} crew, ${recs.length} recordings) on no plot`,
  );
  add(
    "crew_clear_of_walls",
    rows > 0 && wallList.length > 0 && inWall.length === 0,
    !rows
      ? "no crew figure was drawn, so none was judged"
      : !wallList.length
        ? "the ship's walls are unknown"
        : inWall.length
          ? `${inWall.length} drawn crew position(s) IN A WALL (closer than ${CREW_WALL_CLEAR_M} m to its line); first: ${inWall
              .filter(Boolean)
              .slice(0, 3)
              .map((o) => `${o.name} at ${fmtP(o.pos)}, ${o.d.toFixed(2)} m from the wall (${o.wall.map((v) => Number(v).toFixed(1)).join(", ")}) (${o.rec}, t ${f(o.t)} s)`)
              .join("; ")}`
          : `all ${rows} drawn crew positions at least ${CREW_WALL_CLEAR_M} m from all ${wallList.length} walls`,
  );
  const afters = (recordings || []).filter((r) => r && r.after);
  const short = afters.map((r) => {
    const fr = Array.isArray(r.frames) ? r.frames : [];
    const seenIds = new Set(fr.flatMap((x) => (x.crew || []).map((c) => c.id)));
    return { r, frames: fr.length, crew: seenIds.size };
  });
  const missingAfter = short.filter((x) => !x.frames || !(Number.isFinite(expectCrew) && x.crew >= expectCrew));
  add(
    "crew_back_after_rejoin",
    afters.length > 0 && missingAfter.length === 0,
    !afters.length
      ? "nothing was recorded after the step back in or after Respawn"
      : short.map((x) => `after ${x.r.after}: ${x.frames ? `${x.crew} of ${expectCrew} crew drawn over ${x.frames} frames` : "NO RECORDING"}${missingAfter.includes(x) ? " (NOT the whole crew)" : ""}`).join("; "),
  );
  const lk = look || null;
  const lookRows = [];
  for (const fr of (lk && lk.frames) || []) for (const c of fr.crew || []) lookRows.push({ t: Number(fr.t), name: c.name, pos: c.pos.map(Number) });
  const outside = commons ? lookRows.filter((c) => !inBoxXZ(c.pos, commons)) : lookRows;
  add(
    "crew_in_the_commons",
    !!commons && lookRows.length > 0 && outside.length === 0,
    !lk
      ? "the crew look never ran"
      : !commons
        ? "the Commons' box is unknown"
        : !lookRows.length
          ? "no crew figure was drawn during the look"
          : outside.length
            ? `${outside.length} of ${lookRows.length} drawn positions OUTSIDE the Commons; first: ${outside[0].name} at ${fmtP(outside[0].pos)}`
            : `all ${lookRows.length} drawn positions during the look (${new Set(lookRows.map((c) => c.name)).size} crew) inside the Commons (x ${commons.min[0]}..${commons.max[0]}, z ${commons.min[2]}..${commons.max[2]})`,
  );
  const cam = lk && Array.isArray(lk.cam) ? lk.cam.map(Number) : null;
  const firstFrame = lk && lk.frames && lk.frames.length ? lk.frames[0] : null;
  const drawnAt = (name) => {
    const row = firstFrame && (firstFrame.crew || []).find((c) => c.name === name);
    return row ? row.pos.map(Number) : null;
  };
  const seen = ((lk && lk.seen) || []).map((s) => {
    // Where it was drawn when its name was looked for (the rig's one-frame probe then), else the
    // look recording's first frame.
    const pos = Array.isArray(s.pos) ? s.pos.map(Number) : drawnAt(s.name);
    const v = cam && pos ? viewAngle(cam, pos) : null;
    const inFront = !!(v && v.ahead > 0 && v.deg < 50);
    const inC = !!(pos && commons && inBoxXZ(pos, commons));
    const amberOk = Number.isFinite(s.amber) && s.amber >= CREW_MIN_PX;
    return { ...s, pos, deg: v ? v.deg : null, ok: !!s.found && inFront && inC && amberOk, inFront, inC, amberOk };
  });
  const good = seen.filter((s) => s.ok);
  add(
    "crew_seen",
    !!cam && good.length > 0,
    !lk
      ? "the crew look never ran"
      : !cam
        ? "the look recorded no camera"
        : !seen.length
          ? "no crew member was drawn to look for"
          : `${good.length ? "at least one crew figure visible" : "NO crew figure visible"} from ${fmtP(cam)} facing ${f(cam[3])} rad (${good.length} of ${seen.length} named crew members pass, the amber under a name being any crew figure's): ` +
            seen
              .map(
                (s) =>
                  `${s.name} ${s.found ? `named on screen at x ${Number(s.pos_px[0]).toFixed(0)}` : "NOT named on screen"}, ` +
                  `drawn ${s.pos ? fmtP(s.pos) : "nowhere"}${s.inC ? "" : " (NOT in the Commons)"}${s.deg !== null ? ` ${s.deg.toFixed(0)} deg off the view${s.inFront ? "" : " (NOT in front)"}` : ""}, ` +
                  `${Number.isFinite(s.amber) ? `${s.amber} crew-amber px under the name` : "no picture"}${s.amberOk ? "" : ` (fewer than ${CREW_MIN_PX})`}`,
              )
              .join("; "),
  );
  return { pass: checks.every((c) => c.ok), checks };
}

const fmtP = (p) => `(${p.map((v) => Number(v).toFixed(1)).join(", ")})`;

// ── Increment 4: the relay's speed check (src/relay/handlers/move_check.rs) ──────

/** The most a player can bank and move in one update under the relay's rules, metres across
 *  the floor: on foot at `on_foot_mps`, with the jitter margin, kept for `banked_s`, plus the
 *  slack. Read from the text of data/ship/shared_world.ron (the numbers the relay runs on).
 *  Pure on the text; null when a number is missing. */
function bankedAllowanceM(ronText) {
  const num = (k) => {
    const m = String(ronText || "").match(new RegExp(`\\b${k}:\\s*([-0-9.eE+]+)`));
    return m ? Number(m[1]) : NaN;
  };
  const [v, j, b, s] = ["on_foot_mps", "jitter_margin", "banked_s", "slack_m"].map(num);
  const a = v * (1 + j) * b + s;
  return Number.isFinite(a) ? a : null;
}

/** How near where the relay holds the game it must stand after a correction, metres. */
const JUMP_STAND_TOL_M = 0.5;
/** How near where the jump landed a relayed position must be to count as the jump leaking out. */
const JUMP_LEAK_M = 5;
/** The first words of every correction notice the game shows (the relay's `correction_sentence`
 *  in src/relay/handlers/move_check.rs, else the game's own in engine/move_check.rs). */
const CORRECTION_NOTICE_START = "The server put you back where it last saw you";

/**
 * Judge THE OVERSIZED JUMP (verify-copresence --plots, increment 4). The game stood still where
 * the relay held it, then jumped (the showcase `cam` verb) farther than anyone can go in one
 * update; the relay must answer with a correction, the game stand back where the relay holds it,
 * nothing of the jump reach the other player, and the game's next move reach them (corrected,
 * not frozen: the old 100 m rule refused such a move without a word and every one after it).
 * `jump`:
 *   from         [x,y,z] the game's camera before the jump
 *   held         [x,y,z] where the relay held the game before the jump (the last of its moves the
 *                walker saw), or null
 *   target       [x,y,z] where the jump put the camera
 *   allowance_m  the most the relay lets anyone move in one update (`bankedAllowanceM`)
 *   before, after  the probe's `moves` before and after ({count, applied, last}); `last.from` is
 *                where the game itself stood when the correction arrived: where the jump really
 *                put it (the review of increment 4, R7: the judge measured from the rig's intended
 *                target instead)
 *   camera       [x,y,z] the game's camera after the correction
 *   relayedAfterJump  every position the walker saw the relay pass on for the game after the
 *                jump and before the nudge
 *   nudged, seen the next move and the positions the walker saw after it
 *   notices      the notices on the game's screen after the correction (the probe's `notices`)
 * Returns { pass, checks }.
 */
function judgeJump(jump) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `jump_${id}`, ok: !!ok, detail });
  const j = jump || {};
  const d = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
  const dxz = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(Number(a[0]) - Number(b[0]), Number(a[2]) - Number(b[2])) : Infinity);
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const before = (j.before && Number(j.before.count)) || 0;
  const after = j.after || {};
  const drew = Number(after.count) - before;
  // The correction the jump drew: the probe's newest one, and only when the count went up (a
  // `last` from an earlier correction says nothing about this jump).
  const last = drew > 0 && after.last ? after.last : null;
  // Where the jump really put the game: its own record, else (no correction) where it stands.
  const landed = last && Array.isArray(last.from) ? last.from : j.camera;
  const gap = dxz(j.from, landed);
  add(
    "far_enough",
    Number.isFinite(gap) && Number.isFinite(j.allowance_m) && gap > j.allowance_m,
    Number.isFinite(j.allowance_m)
      ? `the jump from ${fmt3(j.from)} put the game at ${fmt3(landed)} (its own record${last ? ", the correction's `from`" : ", no correction"}; the rig asked for ${fmt3(j.target)}), ${Number.isFinite(gap) ? gap.toFixed(1) : "?"} m across the floor; the relay lets anyone move at most ${Number(j.allowance_m).toFixed(1)} m in one update` +
          (gap > j.allowance_m ? "" : " (needs more than that to mean anything)")
      : "the relay's allowance was not recorded (data/ship/shared_world.ron)",
  );
  const toHeld = last ? d(last.at, j.held) : Infinity;
  add(
    "corrected",
    !!last && last.reason === "too_fast" && toHeld <= JUMP_STAND_TOL_M,
    !j.after
      ? "the game's corrections were not recorded (the probe's moves)"
      : !last
        ? `no correction reached the game (${before} before the jump, ${Number(after.count) || 0} after): the relay refused it without a word, or the game ignored it`
        : `correction ${last.seq} (${last.reason}) stood the game at ${fmt3(last.at)}, ${Number.isFinite(toHeld) ? toHeld.toFixed(2) : "?"} m from where the relay held it ${fmt3(j.held)}`,
  );
  add(
    "corrected_once",
    drew === 1,
    !j.after ? "the game's corrections were not recorded" : `the jump drew ${Number.isFinite(drew) ? drew : "?"} correction(s) (exactly one: the relay sends one and drops the updates already on their way)`,
  );
  const said = Array.isArray(j.notices) ? j.notices.filter((n) => String(n).startsWith(CORRECTION_NOTICE_START)) : null;
  add(
    "said_once",
    !!said && said.length === 1,
    !said
      ? "the notices on screen after the correction were not recorded"
      : `${said.length} correction notice(s) on screen after it (exactly one sentence)${said.length ? `: "${said[0]}"` : ""}`,
  );
  const off = d(j.camera, j.held);
  add(
    "stands_where_held",
    off <= JUMP_STAND_TOL_M,
    `after the correction the game's camera at ${fmt3(j.camera)} is ${Number.isFinite(off) ? off.toFixed(2) : "?"} m from where the relay holds it ${fmt3(j.held)}` +
      (off <= JUMP_STAND_TOL_M ? "" : ` (at most ${JUMP_STAND_TOL_M} m)`),
  );
  const leaked = Array.isArray(landed) ? (j.relayedAfterJump || []).filter((p) => dxz(p, landed) <= JUMP_LEAK_M) : [];
  add(
    "never_relayed",
    Array.isArray(j.relayedAfterJump) && Array.isArray(landed) && leaked.length === 0,
    !Array.isArray(j.relayedAfterJump)
      ? "what the relay passed on after the jump was not recorded"
      : !Array.isArray(landed)
        ? "where the jump put the game was not recorded"
        : leaked.length
          ? `the relay passed the jump on to the walker: ${fmt3(leaked[0])}`
          : `nothing near where the jump put the game reached the walker (${j.relayedAfterJump.length} position(s) passed on in between)`,
  );
  const hit = (j.seen || []).find((p) => d(p, j.nudged) <= REJOIN_NUDGE_TOL_M);
  add(
    "moves_reach_others",
    !!hit,
    hit
      ? `its next move to ${fmt3(j.nudged)} reached the walker through the relay, at ${fmt3(hit)}`
      : `its next move to ${fmt3(j.nudged)} never reached the walker (${(j.seen || []).length} update(s) seen after it): frozen`,
  );
  return { pass: checks.every((c) => c.ok), checks };
}

/** The relay's log line for a correction it sent (src/relay/handlers/msg_handlers.rs
 *  `handle_game_position_update`): the player's key (its first 16 hex digits), the correction's
 *  number, whether it is one sent again, and why. */
const RELAY_CORRECTED_RE = /Game: corrected ([0-9a-f]+)\.\. \(correction (\d+)( sent again)?, ([a-z_]+)\)/;

/** The corrections a relay sent, from its log text, by player key (the first 16 hex digits the line
 *  names): each new one, never one sent again. `notKeys`: keys left out (the scripted walkers'),
 *  matched on their first 16 digits. Returns { total, byKey: {key: count} }. Pure. */
function relayCorrections(logText, notKeys = []) {
  const skip = new Set((notKeys || []).map((k) => String(k).slice(0, 16)));
  const byKey = {};
  let total = 0;
  for (const line of String(logText || "").split(/\r?\n/)) {
    const m = line.match(RELAY_CORRECTED_RE);
    if (!m || m[3] || skip.has(m[1].slice(0, 16))) continue;
    byKey[m[1]] = (byKey[m[1]] || 0) + 1;
    total += 1;
  }
  return { total, byKey };
}

/**
 * EVERY HONEST MOVE WAS TAKEN (increment 4): across the whole run, the relay corrected the game
 * only for the one oversized jump, and never a scripted walker. Walking through the doors,
 * shutting the build editor away from the build spot, a teleporter, Respawn, stepping out and back
 * in: the relay must tell each of them from a move nobody could make. `honest`:
 *   gameTotal     the corrections the game applied over the run, both boots (the probe's count)
 *   fromJump      how many of those the jump drew (after - before)
 *   relaySent     the corrections the relay sent the game over the run (new ones, never one sent
 *                 again, from its relay.log, `relayCorrections` without the walkers' keys)
 *   walkerLines   every correction line a walker logged
 * The game's own count alone could not be trusted (the review of increment 4, R7): the game
 * drops a correction that reaches it out of the shared world or older than the last it applied,
 * so a correction sent and dropped was never counted. Returns { pass, checks }.
 */
function judgeHonestMoves(honest) {
  const checks = [];
  const h = honest || {};
  const others = Number(h.gameTotal) - Number(h.fromJump || 0);
  checks.push({
    id: "moves_game_honest_never_corrected",
    ok: Number.isFinite(others) && others === 0,
    detail: !Number.isFinite(Number(h.gameTotal))
      ? "the game's corrections over the run were not recorded"
      : others === 0
        ? `the relay corrected the game ${h.gameTotal} time(s) over the run, every one for the oversized jump`
        : `the relay corrected the game ${others} time(s) for moves that were not the jump (${h.gameTotal} in all): an honest move was taken for one nobody could make`,
  });
  checks.push({
    id: "moves_relay_sent_what_the_game_took",
    ok: Number.isFinite(Number(h.relaySent)) && h.relaySent !== null && Number(h.relaySent) === Number(h.gameTotal),
    detail:
      h.relaySent === null || h.relaySent === undefined || !Number.isFinite(Number(h.relaySent))
        ? "the corrections the relay sent were not recorded (its relay.log in the run folder)"
        : Number(h.relaySent) === Number(h.gameTotal)
          ? `the relay sent the game ${h.relaySent} correction(s) and the game applied ${h.gameTotal}`
          : `the relay sent the game ${h.relaySent} correction(s) but the game applied ${h.gameTotal}: ${Number(h.relaySent) > Number(h.gameTotal) ? "a correction was dropped" : "the game counted one the relay never sent"}`,
  });
  const lines = Array.isArray(h.walkerLines) ? h.walkerLines : null;
  checks.push({
    id: "moves_walkers_never_corrected",
    ok: !!lines && lines.length === 0,
    detail: !lines ? "the walkers' corrections were not recorded" : lines.length ? `a walker was corrected: ${lines[0]}` : "no scripted walker was ever corrected",
  });
  return { pass: checks.every((c) => c.ok), checks };
}

// ── The rig's own walks (the review of increment 4, R4) ──────────────────────────────────────

/** How near the pose's point the camera must already stand for a `cam` that only turns it, m. */
const TURN_IN_PLACE_M = 0.1;

/** Whether a `cam` to `pose` ("x,y,z,yaw,pitch") from a camera at `at` only turns it: the camera
 *  already within TURN_IN_PLACE_M of the pose's point. A walk that stopped short must never be
 *  finished by the turn, which is a teleport (R4: the meeting's and the crew look's turns were
 *  sent whatever the walk before them did). Returns { ok, off }. Pure. */
function turnInPlace(at, pose, tol = TURN_IN_PLACE_M) {
  const p = String(pose).split(",").map(Number);
  const off = Array.isArray(at) && p.length >= 3 ? Math.hypot(Number(at[0]) - p[0], Number(at[1]) - p[1], Number(at[2]) - p[2]) : Infinity;
  return { ok: off <= tol, off };
}

/**
 * Every walk of the run arrived, and every turn in place was one (R4: a walk that never arrived
 * was silent, the rig went on as if it had, and the turn after it finished the walk with a
 * teleport). `walks`: [{ label, to, at, ok }] (the camera where the walk ended); `turns`:
 * [{ pose, at, off, ok }]. Not recorded fails. Returns { pass, checks }.
 */
function judgeWalks(walks, turns) {
  const checks = [];
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(none)");
  const short = Array.isArray(walks) ? walks.filter((w) => !w.ok) : null;
  checks.push({
    id: "walks_all_arrived",
    ok: !!short && walks.length > 0 && short.length === 0,
    detail: !short
      ? "the rig's walks were not recorded"
      : !walks.length
        ? "no walk was recorded"
        : short.length
          ? `${short.length} of ${walks.length} walks never arrived; the first, ${short[0].label}, to ${fmt3(short[0].to)}, stopped at ${fmt3(short[0].at)}`
          : `all ${walks.length} walks arrived`,
  });
  const bad = Array.isArray(turns) ? turns.filter((t) => !t.ok) : null;
  checks.push({
    id: "walks_turns_in_place",
    ok: !!bad && bad.length === 0,
    detail: !bad
      ? "the rig's turns were not recorded"
      : bad.length
        ? `a turn to ${bad[0].pose} was refused: the camera stood ${Number.isFinite(Number(bad[0].off)) ? Number(bad[0].off).toFixed(2) : "?"} m from it, at ${fmt3(bad[0].at)} (more than ${TURN_IN_PLACE_M} m: the walk there stopped short)`
        : `every turn (${turns.length}) was made where the camera already stood`,
  });
  return { pass: checks.every((c) => c.ok), checks };
}

// ── The fast moves that are declared, end to end (the review of increment 4, R1) ─────────────

/** Where to stand beside a pad before stepping onto it: the floor point `stepM` from the pad's middle
 *  along x or z, whichever of the four is farthest from every wall (`walls`: [x1, z1, x2, z2]
 *  segments from the door points), at eye height. Pure. */
function padApproach(padAt, walls, stepM = 1.5, eye = 1.7) {
  const cands = [[stepM, 0], [-stepM, 0], [0, stepM], [0, -stepM]].map(([dx, dz]) => [Number(padAt[0]) + dx, eye, Number(padAt[2]) + dz]);
  const clear = (p) => (Array.isArray(walls) && walls.length ? Math.min(...walls.map((w) => wallDistXZ(p, w))) : Infinity);
  return cands.map((p) => ({ p, clear: clear(p) })).sort((a, b) => b.clear - a.clear)[0];
}

/**
 * Where the rig walks the game before shutting the build editor, so that the close is a jump only
 * its declaration explains: a point in a shared zone whose distance across the floor from the
 * build spot `door` is more than the relay's allowance for one update (`allowanceM`) and well
 * inside the 90 m an editor close may jump (aim `aimM`, 60 m), at least `clearM` from every wall,
 * reached through the doors with no wall in the way (`doorRoute`, `routeWalls`). Returns
 * { at, dist, route } or { error }. Pure.
 */
function editorJumpTarget(report, door, allowanceM, { aimM = 60, maxM = 80, clearM = 1.0, grid = 1 } = {}) {
  const lo = Number(allowanceM) + 6;
  if (!Number.isFinite(lo) || !Array.isArray(door)) return { error: "no allowance or no build spot" };
  const walls = Array.isArray(report.walls) ? report.walls : [];
  const zones = (report.places || []).filter((pl) => pl.kind === "zone");
  const cands = [];
  for (const z of zones) {
    for (let x = Math.ceil(z.min[0]) + 0.5; x < z.max[0]; x += grid) {
      for (let zz = Math.ceil(z.min[2]) + 0.5; zz < z.max[2]; zz += grid) {
        const p = [x, Number(door[1]), zz];
        const dist = Math.hypot(p[0] - door[0], p[2] - door[2]);
        if (dist < lo || dist > maxM) continue;
        const clear = walls.length ? Math.min(...walls.map((w) => wallDistXZ(p, w))) : Infinity;
        if (clear < clearM) continue;
        cands.push({ p, dist });
      }
    }
  }
  cands.sort((a, b) => Math.abs(a.dist - aimM) - Math.abs(b.dist - aimM) || a.p[0] - b.p[0] || a.p[2] - b.p[2]);
  for (const c of cands) {
    const route = doorRoute(report, door, c.p, 40);
    if (route.error) continue;
    if (routeWalls([door, ...route.points], walls)) continue;
    return { at: c.p, dist: c.dist, route: route.points };
  }
  return { error: `no clear point ${lo.toFixed(1)} to ${maxM} m from the build spot ${JSON.stringify(door)} in a shared zone` };
}

/**
 * SHUTTING THE BUILD EDITOR AWAY FROM THE BUILD SPOT (R1). At its door (its build spot) the game
 * walked about 60 m into the ship, more than the relay lets anyone move in one update, opened the
 * build editor and shut it: the close stands it back at the build spot, on its own plot, a jump
 * its next update declares (`moved: {by: "editor"}`) and the relay must take. `ej`:
 *   buildSpot  [x,y,z] the build spot (where the first open and shut at the door stood the game)
 *   walkedTo   [x,y,z] where the walk left the camera before the editor opened
 *   toggled    the editor opened and shut
 *   camera     [x,y,z] the game's camera after it shut
 *   seen       the positions the walker saw the relay pass on for the game after it shut
 *   before, after  the probe's `moves` before the walk and after the shut
 *   allowance_m    the relay's allowance for one update (`bankedAllowanceM`)
 * Returns { pass, checks }.
 */
function judgeEditorJump(ej) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `editorjump_${id}`, ok: !!ok, detail });
  const e = ej || {};
  const d = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(...[0, 1, 2].map((k) => Number(a[k]) - Number(b[k]))) : Infinity);
  const dxz = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(Number(a[0]) - Number(b[0]), Number(a[2]) - Number(b[2])) : Infinity);
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const gap = dxz(e.walkedTo, e.buildSpot);
  add(
    "far_enough",
    Number.isFinite(gap) && Number.isFinite(e.allowance_m) && gap > e.allowance_m,
    Number.isFinite(e.allowance_m)
      ? `the editor opened at ${fmt3(e.walkedTo)}, ${Number.isFinite(gap) ? gap.toFixed(1) : "?"} m across the floor from the build spot ${fmt3(e.buildSpot)}; the relay lets anyone move at most ${Number(e.allowance_m).toFixed(1)} m in one update` +
          (gap > e.allowance_m ? "" : " (needs more than that: shutting it would be a plain walk)")
      : "the relay's allowance was not recorded",
  );
  add("toggled", e.toggled === true, e.toggled === true ? "the build editor opened and shut" : "the build editor did not open and shut");
  const off = d(e.camera, e.buildSpot);
  add(
    "at_build_spot",
    off <= JUMP_STAND_TOL_M,
    `after it shut the game's camera at ${fmt3(e.camera)} is ${Number.isFinite(off) ? off.toFixed(2) : "?"} m from the build spot ${fmt3(e.buildSpot)}` + (off <= JUMP_STAND_TOL_M ? "" : ` (at most ${JUMP_STAND_TOL_M} m)`),
  );
  const hit = (e.seen || []).find((p) => d(p, e.buildSpot) <= REJOIN_NUDGE_TOL_M);
  add(
    "relayed",
    !!hit,
    hit ? `the walker saw the relay pass the game on at the build spot, at ${fmt3(hit)}` : `the walker never saw the game at the build spot (${(e.seen || []).length} position(s) seen after the shut): the relay corrected the jump, or held it back`,
  );
  const drew = e.after && e.before ? Number(e.after.count) - Number(e.before.count) : NaN;
  add(
    "never_corrected",
    drew === 0,
    Number.isFinite(drew) ? `the relay corrected the game ${drew} time(s) between the walk and the shut (none: a declared editor close is taken)` : "the game's corrections were not recorded",
  );
  return { pass: checks.every((c) => c.ok), checks };
}

/**
 * THE HOME'S OWN TELEPORTER (R1). From beside the west pad of the game's home the game stepped
 * onto it (the showcase `walk_to`): the pad jumps it to the east pad, more than the relay lets
 * anyone move in one update, and its next update declares the jump (`moved: {by: "link", zone:
 * "home", ...}`), which the relay must take. `tele`:
 *   link       { zone, from, to, from_at, to_at, reach_m } the link the game reports for that pad
 *   camera     [x,y,z] the game's camera after the jump
 *   seen       the positions the walker saw the relay pass on for the game after it
 *   before, after  the probe's `moves` before stepping on and after
 *   allowance_m    the relay's allowance for one update
 * Returns { pass, checks }.
 */
function judgeTeleporter(tele) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id: `tele_${id}`, ok: !!ok, detail });
  const t = tele || {};
  const l = t.link || null;
  const dxz = (a, b) => (Array.isArray(a) && Array.isArray(b) ? Math.hypot(Number(a[0]) - Number(b[0]), Number(a[2]) - Number(b[2])) : Infinity);
  const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(2)).join(", ")})` : "(never)");
  const gap = l ? dxz(l.from_at, l.to_at) : Infinity;
  add(
    "far_enough",
    !!l && Number.isFinite(t.allowance_m) && gap > t.allowance_m,
    !l
      ? "the game reported no link for its home's west pad"
      : `${l.from} at ${fmt3(l.from_at)} jumps to ${l.to} at ${fmt3(l.to_at)}, ${gap.toFixed(1)} m across the floor; the relay lets anyone move at most ${Number(t.allowance_m).toFixed(1)} m in one update` +
          (gap > t.allowance_m ? "" : " (needs more than that: the jump would be a plain walk)"),
  );
  const reach = l && Number.isFinite(Number(l.reach_m)) ? Number(l.reach_m) + 0.3 : 1.0;
  const off = l ? dxz(t.camera, l.to_at) : Infinity;
  add(
    "arrived",
    off <= reach,
    l ? `after stepping on, the game's camera at ${fmt3(t.camera)} is ${Number.isFinite(off) ? off.toFixed(2) : "?"} m across the floor from the far pad ${fmt3(l.to_at)}` + (off <= reach ? "" : ` (at most ${reach.toFixed(2)} m: it never jumped, or jumped elsewhere)`) : "no link",
  );
  const hit = l ? (t.seen || []).find((p) => dxz(p, l.to_at) <= reach) : null;
  add(
    "relayed",
    !!hit,
    hit ? `the walker saw the relay pass the game on at the far pad, at ${fmt3(hit)}` : `the walker never saw the game at the far pad (${(t.seen || []).length} position(s) seen after the jump): the relay corrected it`,
  );
  const drew = t.after && t.before ? Number(t.after.count) - Number(t.before.count) : NaN;
  add(
    "never_corrected",
    drew === 0,
    Number.isFinite(drew) ? `the relay corrected the game ${drew} time(s) from the step onto the pad until after the jump (none: a declared teleporter jump is taken)` : "the game's corrections were not recorded",
  );
  return { pass: checks.every((c) => c.ok), checks };
}

module.exports = {
  turnInPlace,
  TURN_IN_PLACE_M,
  judgeWalks,
  clearPath,
  PATH_CLEAR_M,
  padApproach,
  editorJumpTarget,
  judgeEditorJump,
  judgeTeleporter,
  bankedAllowanceM,
  judgeJump,
  judgeHonestMoves,
  relayCorrections,
  RELAY_CORRECTED_RE,
  CORRECTION_NOTICE_START,
  JUMP_STAND_TOL_M,
  LIMITS,
  judgeCopresence,
  judgeCrew,
  isFigureAmber,
  crewPixels,
  CREW_MIN_PX,
  CREW_WALL_CLEAR_M,
  wallDistXZ,
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
  routeWalls,
  farPlaces,
  farthestFrom,
  judgeMeet,
  judgeReboot,
  judgeGuest,
  onPlotGround,
  GUEST_NO_EDITOR_START,
  MEET_MAX_STEP_M,
  REJOIN_FAR_M,
};
