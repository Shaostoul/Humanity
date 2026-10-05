// THE SHARED-BUILD JUDGE (ship homes increment 5, "building only on your own plot", 2026-10-05;
// docs/design/ship-homes-increment-5-plan.md section 5): did a real game, a real relay and two
// scripted players keep the building rules, and draw what the relay keeps where it keeps it?
//
// Pure: it reads one run's record (the manifest scripts/verify-copresence.js --build writes) and
// the evidence that came with it (the game's recordings, the two pictures, the relay's log text),
// and returns checks, so it is tested with made-up runs (scripts/tests/shared-build-judge.test.js,
// in `just rig-tests`) and re-run on a saved run without booting anything
// (`node scripts/verify-copresence.js --dry-verdict <manifest.json>`).
//
// THE RUN, in the plan's steps (verify-copresence.js runBuildOnce says how each record is made):
//   1-2  a throwaway relay whose admin is R; walker A joins first (a plot, p1), the game second
//        (p2), walker R third (p3, the rank holder). `ranks_from_the_server`, `three_plots_held`.
//   3    THEIRS REACHES US: A builds a foundation and a wall on its plot; the game draws both
//        within 1.5 s of A hearing the relay keep them, where they were built, as scaffolds that
//        finish on the relay's clock, and never in its save (`theirs_*`, `shared_pieces_not_saved`).
//   4    OURS REACHES THEM: the game places a foundation on its own plot (the showcase `place`
//        verb, E's own path); it is the game's own, spent once, and A sees it (`ours_*`).
//   5    THE NAMED CASE, both ways: A's build in the game's plot is refused as someone else's plot,
//        by the relay, and the game never draws it; the game's place in A's plot is refused on its
//        own screen with the plot sentence before anything is spent or sent (`refused_*`).
//   6    WHO TAKES DOWN: A cannot take the game's piece down; the game can, A sees it go, and the
//        materials come back once (`their_take_down_refused`, `our_take_down_*`).
//   7    THE SHIP'S SHARED SPACES need the rank: the game's place in the Commons and A's build
//        there are refused with the rank sentence; R's wall there is drawn where it stands, in
//        view, and seen in the picture (`zone_*`).
//   8    DEV KEEPS EVERYTHING ONLY OFFLINE: no ship editing while joined; out of the shared world,
//        ship editing and no shared piece; back in, the same pieces by the same ids, finished
//        (`dev_*`, `solo_back_*`).
//   9    OVER THE RUN: no correction of the game or a walker, no panic, no built-in data.
//
// WHAT THE GAME REPORTS (increment 5 Wave 2B, src/engine/ipc.rs; the contract this judge reads):
//   the recorder's frames each carry `shared`: every piece the server keeps that this game has in
//   its world that frame, { piece_id, frame, blueprint_id, pos [x,y,z] ship metres (the bottom
//   centre the piece is drawn from), rot [x,y,z,w], scale [x,y,z], mine, built (false while it is
//   a scaffold), progress (s), rect [x0,y0,x1,y1] window pixels or null (behind the camera) }.
//   The probe's `shared_build` holds ranks, ship_editing, pieces (the same rows, no rect),
//   save_constructions, pack { item: count }, pending { builds, unbuilds, intents }, hint (the
//   placing line under the crosshair, or null) and editor_zone.
//
// WHAT THE RELAY LOGS (Wave 2A, never a key): "Game: built piece {id} {blueprint} on {frame}",
// "Game: build refused ({reason}/{why}) on {frame}", "Game: took down piece {id} on {frame}",
// "Game: take-down refused ({reason}/{why})".

"use strict";

const { judgeWalks, relayCorrections } = require("./copresence-judge.js");
// The turn the game's placement makes (placement::quarter_turn), one copy, pinned to the Rust by
// scripts/tests/second-player.test.js.
const { quarterTurn } = require("../second-player.js");

/// The limits, each with its reason.
const LIMITS = {
  /// How soon after the builder hears the relay keep a piece the game must have it in its world,
  /// seconds (plan section 5, steps 3 and 7). The relay sends `game_built` to everyone with the
  /// frame in view under the same lock, so both hear it within a frame or two of each other; a
  /// background game at 9 to 15 frames a second reads its socket once a frame.
  SEEN_WITHIN_S: 1.5,
  /// How far from where it was built a piece may be drawn, metres (plan: "within 1 cm"). The relay
  /// keeps the pose to the millimetre and the frame corners are whole metres, so it should be 0.
  POSE_TOL_M: 0.01,
  /// How far each part of the drawn turn may be from the exact quarter turn (either sign of the
  /// quaternion). The relay keeps exactly `placement::quarter_turn`; 1e-4 is a hundredth of a
  /// degree, and the next quarter turn is 0.29 away.
  TURN_TOL: 1e-4,
  /// How far the drawn size may be from the blueprint's, metres (shared.rs SIZE_TOLERANCE_M).
  SIZE_TOL_M: 0.001,
  /// How far from the relay's clock a scaffold may finish, seconds (plan: "placed_at + build_time
  /// +- 0.5 s"): one slow frame of a background game, and the recorder seeing it a frame late.
  FINISH_TOL_S: 0.5,
  /// The game ticks its systems with each frame's step held at this, seconds (src/lib.rs, `let dt
  /// = raw_dt.min(0.1)`), so every frame slower than it grows a scaffold that much less: added to
  /// when it should finish (`lostToSlowFrames`), never to the tolerance.
  SYSTEM_DT_CAP_S: 0.1,
  /// PROVISIONAL (plan section 5 step 7: "the tint measured off the first run, as the crew amber
  /// was"). The least share of the wall's screen rect that must be wood-tinted in the picture with
  /// the wall, and the least it must gain over the picture before the wall went up. The wall seen
  /// face on at 6 m fills nearly all of its rect; the gain is what separates it from oak or a crew
  /// figure behind it. Pin both, and isWoodTint, on the first real run, from the colours its
  /// zone_wall_visible line prints.
  WOOD_MIN_SHARE: 0.35,
  WOOD_GAIN_SHARE: 0.25,
};

/// The words the game shows for a build on someone else's plot and for one in the ship's shared
/// spaces without the rank: the start of shared.rs `Why::NotYourPlot` and `Why::ShipRank` words,
/// the one copy of each sentence (the judge test reads shared.rs and holds these to it).
const PLOT_SENTENCE = "this is someone else's plot; you build only on your own plot";
const RANK_SENTENCE = "the ship's shared spaces are built by people this server has given that rank";

/// The relay's log lines (Wave 2A, plan section 3.3), never a key. A refusal with no `why` may be
/// written "(reason)", "(reason/)" or "(reason/-)".
const RELAY_BUILT_RE = /Game: built piece (\d+) (\S+) on (\S+)/;
const RELAY_BUILD_REFUSED_RE = /Game: build refused \(([a-z_]+)(?:\/([a-z_-]*))?\) on (\S+)/;
const RELAY_TOOK_DOWN_RE = /Game: took down piece (\d+) on (\S+)/;
const RELAY_TAKE_DOWN_REFUSED_RE = /Game: take-down refused \(([a-z_]+)(?:\/([a-z_-]*))?\)/;

/// Is this pixel the wood a built wall is drawn in? The material is (0.48, 0.33, 0.20)
/// (engine/planet_build.rs push_render_objects, "wood"); lit and tone-mapped, the crew's amber
/// (0.92, 0.62, 0.18) came out about (176, 151, 71), green 0.86 of red and blue 0.40, so wood is
/// expected near green 0.83 and blue 0.6 of red. PROVISIONAL until the first run measures it
/// (LIMITS.WOOD_*). Blue at least 0.53 of red keeps out the crew's amber and the scaffold's (both
/// 0.25 to 0.52), green at most 0.93 keeps out the grey floor and walls.
function isWoodTint(r, g, b) {
  return r >= 70 && g >= 0.7 * r && g <= 0.93 * r && b >= 0.53 * r && b <= 0.8 * r && r - b >= 20;
}

const num = (v) => (v === null || v === undefined || v === "" ? NaN : Number(v));
const fmt3 = (p) => (Array.isArray(p) ? `(${p.map((v) => Number(v).toFixed(3)).join(", ")})` : "(none)");
const fs2 = (x) => (Number.isFinite(x) ? x.toFixed(2) : "?");
const dist = (a, b) => (Array.isArray(a) && Array.isArray(b) && a.length >= 3 && b.length >= 3 ? Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]) : Infinity);

/** The frame `id` of the run (from the game's door points: { id, min, max }), or null. */
function frameOf(run, id) {
  return ((run && run.frames) || []).find((f) => f.id === id) || null;
}

/** `local` (metres from `frame`'s corner) in ship metres, or null. */
function shipPoint(frame, local) {
  if (!frame || !Array.isArray(frame.min) || !Array.isArray(local) || local.length < 3) return null;
  return [0, 1, 2].map((k) => Number(frame.min[k]) + Number(local[k]));
}

/** How far a drawn turn [x,y,z,w] is from the exact quarter turn `turns`, the larger part's gap,
 *  either sign of the quaternion. Infinity when there is no turn. */
function turnGap(rot, turns) {
  if (!Array.isArray(rot) || rot.length !== 4 || !rot.every((v) => Number.isFinite(Number(v)))) return Infinity;
  const q = quarterTurn(turns);
  const same = Math.max(...q.map((v, i) => Math.abs(v - Number(rot[i]))));
  const flipped = Math.max(...q.map((v, i) => Math.abs(v + Number(rot[i]))));
  return Math.min(same, flipped);
}

/** The largest gap between a drawn size and the blueprint's, metres. */
function sizeGap(scale, size) {
  if (!Array.isArray(scale) || !Array.isArray(size) || scale.length !== 3 || size.length !== 3) return Infinity;
  return Math.max(...scale.map((v, i) => Math.abs(Number(v) - Number(size[i]))));
}

/** Every recorded frame holding `pieceId`, in order: [{ i, epoch, dt, row }]. */
function rowsOf(frames, pieceId) {
  const out = [];
  (frames || []).forEach((fr, i) => {
    const row = (Array.isArray(fr.shared) ? fr.shared : []).find((r) => Number(r.piece_id) === Number(pieceId));
    if (row) out.push({ i, epoch: Number(fr.epoch_ms), dt: Number(fr.dt), row });
  });
  return out;
}

/** The time lost to frames slower than the systems' step cap between frame `from` (exclusive)
 *  and `to` (inclusive), seconds: what such frames did not grow a scaffold by. */
function lostToSlowFrames(frames, from, to) {
  let lost = 0;
  for (let i = from + 1; i <= to && i < frames.length; i++) {
    const dt = Number(frames[i].dt);
    if (Number.isFinite(dt) && dt > LIMITS.SYSTEM_DT_CAP_S) lost += dt - LIMITS.SYSTEM_DT_CAP_S;
  }
  return lost;
}

/** The relay lines of a window that match `re`, as match arrays. */
function relayHits(lines, re) {
  return (Array.isArray(lines) ? lines : []).map((l) => String(l).match(re)).filter(Boolean);
}

/** Is `why` (a log line's group) the given code, reading "", "-" and none as no why? */
const whyIs = (got, want) => (got === "" || got === "-" || got === undefined ? null : got) === want;

/** Did a refusal record say `action` refused for `reason` and `why`? */
function refusedAs(refusal, action, reason, why) {
  return !!refusal && refusal.action === action && refusal.reason === reason && (refusal.why || null) === why;
}

/** The text of a refusal record, for a detail. */
const refusalText = (r) => (r ? `${r.action} refused: ${r.reason}${r.why ? ` (${r.why})` : ""}` : "no refusal");

/** Is `sentence` on the game's screen: in a notice (a toast), or the placing line under the
 *  crosshair (`hint`)? Null when neither was recorded. */
function shown(rec, sentence) {
  const notices = rec && Array.isArray(rec.notices) ? rec.notices : null;
  const hint = rec && typeof rec.hint === "string" ? rec.hint : null;
  if (!notices && hint === null) return null;
  return (notices || []).some((n) => String(n).includes(sentence)) || (hint !== null && hint.includes(sentence));
}

/** The material counts of `blueprint` that changed between two pack records, as
 *  [{ item, n, before, after, later }]; null when a record is missing. */
function packMoves(run, blueprint, before, after, later) {
  const bp = run && run.blueprints && run.blueprints[blueprint];
  if (!bp || !Array.isArray(bp.materials) || !bp.materials.length || !before || !after) return null;
  return bp.materials.map(([item, n]) => ({ item, n: Number(n), before: num(before[item]), after: num(after[item]), later: later ? num(later[item]) : NaN }));
}

/** The pixel box of `rect` inside an image of `w` by `h`, or null when nothing of it is. */
function clampRect(rect, w, h) {
  if (!Array.isArray(rect) || rect.length !== 4 || !rect.every((v) => Number.isFinite(Number(v)))) return null;
  const [x0, y0, x1, y1] = rect.map(Number);
  const box = [Math.max(0, Math.floor(x0)), Math.max(0, Math.floor(y0)), Math.min(w, Math.ceil(x1)), Math.min(h, Math.ceil(y1))];
  return box[2] > box[0] && box[3] > box[1] ? box : null;
}

/** The wood-tinted pixels of `img` ({ width, height, rgba }, scripts/lib/png.js) in `box`, the
 *  box's area, and the median colour there (what the first real run calibrates isWoodTint from). */
function woodIn(img, box) {
  let count = 0;
  const reds = [];
  const greens = [];
  const blues = [];
  for (let y = box[1]; y < box[3]; y++) {
    for (let x = box[0]; x < box[2]; x++) {
      const i = (y * img.width + x) * 4;
      const [r, g, b] = [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]];
      if (isWoodTint(r, g, b)) count++;
      reds.push(r);
      greens.push(g);
      blues.push(b);
    }
  }
  const median = (v) => v.sort((a, b) => a - b)[Math.floor(v.length / 2)];
  return { count, area: (box[2] - box[0]) * (box[3] - box[1]), median: reds.length ? [median(reds), median(greens), median(blues)] : null };
}

/** The wall's screen rect at the moment of the picture with it: the recorded frame holding the
 *  piece nearest in time to `atMs`, built ones first. { rect, epoch } or null. */
function rectAt(frames, pieceId, atMs) {
  const rows = rowsOf(frames, pieceId);
  if (!rows.length) return null;
  const pool = rows.some((r) => r.row.built === true) ? rows.filter((r) => r.row.built === true) : rows;
  const best = pool.reduce((a, b) => (Math.abs(b.epoch - atMs) < Math.abs(a.epoch - atMs) ? b : a));
  return { rect: Array.isArray(best.row.rect) ? best.row.rect : null, epoch: best.epoch };
}

/**
 * Judge one --build run.
 *   run       the manifest (see the top of this file and verify-copresence.js runBuildOnce)
 *   evidence  { recordings: { <file>: frames[] }, pictures: { <file>: { width, height, rgba } },
 *             relayLog: the relay's log text }: what the manifest names, read from its folder.
 * Returns { pass, checks: [{ id, ok, detail }] }. A record the run did not make FAILS its check;
 * nothing is ever passed unchecked.
 */
function judgeSharedBuild(run, evidence = {}) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const m = run || {};
  const s = m.steps_ok || {};
  const recs = evidence.recordings || {};
  const pics = evidence.pictures || {};
  const framesOf = (file) => (file && Array.isArray(recs[file]) ? recs[file] : null);

  // ── 1-2. The relay, the game and the two walkers, and who holds what.
  add("relay_up", s.relay && s.relay.ok, s.relay ? s.relay.detail : "never started");
  add("game_in_world", s.joined && s.joined.ok, s.joined ? s.joined.detail : "never joined");
  add("walkers_in_world", s.walkers && s.walkers.ok, s.walkers ? s.walkers.detail : "the builders never joined");
  const w = m.walkers || {};
  const A = w.builder || {};
  const R = w.rank || {};
  const G = m.game || {};
  const held = [A.plot, G.plot, R.plot];
  add(
    "three_plots_held",
    held.every((p) => typeof p === "string" && p) && new Set(held).size === 3,
    `${A.name || "the builder"} holds ${A.plot || "no plot"}, the game ${G.plot || "no plot"}, ${R.name || "the rank holder"} ${R.plot || "no plot"}` +
      (new Set(held).size === 3 ? "" : ": NOT three plots of their own"),
  );
  const rk = (r) => (r && typeof r === "object" ? `can_edit_ship ${r.can_edit_ship === true}, take_down_any ${r.take_down_any === true}` : "not told");
  const ranksOk =
    !!R.ranks && R.ranks.can_edit_ship === true && R.ranks.take_down_any === true &&
    !!A.ranks && A.ranks.can_edit_ship === false && A.ranks.take_down_any === false &&
    !!G.ranks && G.ranks.can_edit_ship === false && G.ranks.take_down_any === false;
  add(
    "ranks_from_the_server",
    ranksOk,
    `the welcomes said: ${R.name || "the rank holder"} (the relay's ADMIN_KEYS) ${rk(R.ranks)}; ${A.name || "the builder"} ${rk(A.ranks)}; the game ${rk(G.ranks)}` +
      (ranksOk ? "" : " (the rank holder must have both, the others neither)"),
  );

  // ── 3. THEIRS REACHES US.
  const theirs = m.theirs || {};
  const builds = Array.isArray(theirs.builds) && theirs.builds.length ? theirs.builds : null;
  const tFrames = framesOf(theirs.samples);
  add(
    "theirs_built",
    !!builds && builds.every((b) => Number.isInteger(b.piece_id) && !b.refusal),
    !builds
      ? "not recorded: the builder's builds on its own plot"
      : builds.map((b) => `${b.cmd ? `${b.cmd.blueprint} on ${b.cmd.frame}` : "?"} -> ${Number.isInteger(b.piece_id) ? `piece ${b.piece_id}` : b.refusal ? refusalText(b.refusal) : "no answer"}`).join("; "),
  );
  {
    const per = (builds || []).map((b) => {
      const seen = Number.isInteger(b.piece_id) && tFrames ? rowsOf(tFrames, b.piece_id)[0] : null;
      const gap = seen ? (seen.epoch - num(b.line_epoch_ms)) / 1000 : NaN;
      return { b, seen, gap, ok: !!seen && Number.isFinite(gap) && Math.abs(gap) <= LIMITS.SEEN_WITHIN_S };
    });
    add(
      "theirs_drawn_in_time",
      !!builds && !!tFrames && per.every((x) => x.ok),
      !tFrames
        ? "not recorded: the game's recording while the builder built"
        : !builds
          ? "not recorded: the builder's builds"
          : per.map((x) => (x.seen ? `piece ${x.b.piece_id} in the game's world ${fs2(x.gap)} s after the builder heard it kept` : `piece ${x.b.piece_id} NEVER in the game's world`) + (x.ok || !x.seen ? "" : ` (within ${LIMITS.SEEN_WITHIN_S} s)`)).join("; "),
    );
  }
  {
    const per = (builds || []).map((b) => {
      const rows = Number.isInteger(b.piece_id) && tFrames ? rowsOf(tFrames, b.piece_id) : [];
      const want = b.cmd ? shipPoint(frameOf(m, b.cmd.frame), b.cmd.local) : null;
      const size = b.cmd && m.blueprints && m.blueprints[b.cmd.blueprint] ? m.blueprints[b.cmd.blueprint].size : null;
      const pos = rows.length ? Math.max(...rows.map((r) => dist(r.row.pos, want))) : Infinity;
      const turn = rows.length && b.cmd ? Math.max(...rows.map((r) => turnGap(r.row.rot, b.cmd.turns))) : Infinity;
      const sz = rows.length ? Math.max(...rows.map((r) => sizeGap(r.row.scale, size))) : Infinity;
      const inFrame = rows.length > 0 && rows.every((r) => b.cmd && r.row.frame === b.cmd.frame);
      const ok = rows.length > 0 && want !== null && pos <= LIMITS.POSE_TOL_M && turn <= LIMITS.TURN_TOL && sz <= LIMITS.SIZE_TOL_M && inFrame;
      return { b, rows, want, pos, turn, sz, inFrame, ok };
    });
    add(
      "theirs_where_built",
      !!builds && !!tFrames && per.every((x) => x.ok),
      !builds || !tFrames
        ? "not recorded: the builds, or the game's recording of them"
        : per
            .map((x) =>
              !x.rows.length
                ? `piece ${x.b.piece_id} never drawn`
                : `piece ${x.b.piece_id} drawn on ${x.rows[0].row.frame} at most ${(x.pos * 100).toFixed(1)} cm from ${fmt3(x.want)} (its frame's corner + where it was built), ${x.turn.toExponential(1)} off quarter turn ${x.b.cmd ? x.b.cmd.turns : "?"}, size ${(x.sz * 1000).toFixed(1)} mm off` +
                  (x.ok ? "" : ` (within ${LIMITS.POSE_TOL_M * 100} cm, ${LIMITS.TURN_TOL}, ${LIMITS.SIZE_TOL_M * 1000} mm, in ${x.b.cmd ? x.b.cmd.frame : "its frame"})`),
            )
            .join("; "),
    );
  }
  {
    const per = (builds || []).map((b) => {
      const rows = Number.isInteger(b.piece_id) && tFrames ? rowsOf(tFrames, b.piece_id) : [];
      const bt = b.cmd && m.blueprints && m.blueprints[b.cmd.blueprint] ? num(m.blueprints[b.cmd.blueprint].build_time) : NaN;
      const placed = num(b.placed_at);
      if (!rows.length) return { b, ok: false, note: `piece ${b.piece_id} never drawn` };
      // A diagnosis more than a guard: a piece seen live and drawn finished at once also misses
      // the finish time below by its whole build time; this says why in words.
      if (rows[0].row.built === true) return { b, ok: false, note: `piece ${b.piece_id} appeared already finished: never a scaffold, so it was not grown from when the relay kept it` };
      const done = rows.find((r) => r.row.built === true);
      if (!done) return { b, ok: false, note: `piece ${b.piece_id} was still a scaffold when the recording ended` };
      if (!Number.isFinite(bt) || !Number.isFinite(placed)) return { b, ok: false, note: `piece ${b.piece_id}: its build time or when the relay kept it was not recorded` };
      const lost = lostToSlowFrames(tFrames, rows[0].i, done.i);
      const expect = (placed + bt + lost) * 1000;
      const gap = (done.epoch - expect) / 1000;
      const ok = Math.abs(gap) <= LIMITS.FINISH_TOL_S;
      return { b, ok, note: `piece ${b.piece_id} finished ${gap >= 0 ? "+" : ""}${gap.toFixed(2)} s from when the relay's clock says (kept at ${placed.toFixed(3)} + ${bt} s to build${lost > 0 ? ` + ${lost.toFixed(2)} s of frames slower than ${LIMITS.SYSTEM_DT_CAP_S} s` : ""})` + (ok ? "" : ` (within ${LIMITS.FINISH_TOL_S} s)`) };
    });
    add(
      "theirs_scaffold_finishes",
      !!builds && !!tFrames && per.every((x) => x.ok),
      !builds || !tFrames ? "not recorded: the builds, or the game's recording of them" : per.map((x) => x.note).join("; "),
    );
  }
  const ours = m.ours || {};
  {
    const vals = [theirs.save_before, theirs.save_after, ours.save].map(num);
    add(
      "shared_pieces_not_saved",
      vals.every(Number.isFinite) && vals.every((v) => v === vals[0]),
      `the game's save holds ${vals.map((v) => (Number.isFinite(v) ? v : "?")).join(", then ")} constructions: before the builder's pieces, with them in its world, and with its own shared piece standing` +
        (vals.every(Number.isFinite) && vals.every((v) => v === vals[0]) ? "" : " (a shared piece must never enter the save)"),
    );
  }

  // ── 4. OURS REACHES THEM.
  const place = ours.place || null;
  {
    const p = ours.piece || null;
    const want = place ? shipPoint(frameOf(m, place.frame), place.local) : null;
    const pos = p ? dist(p.pos, want) : Infinity;
    const turn = p && place ? turnGap(p.rot, place.turns) : Infinity;
    const ok = !!p && !!place && p.mine === true && p.frame === place.frame && pos <= LIMITS.POSE_TOL_M && turn <= LIMITS.TURN_TOL;
    add(
      "ours_mine",
      ok,
      !place
        ? "not recorded: the game's place on its own plot"
        : !p
          ? `the game's ${place.blueprint} on ${place.frame} never stood in its world as a shared piece`
          : `piece ${p.piece_id} on ${p.frame}, mine ${p.mine === true}, ${(pos * 100).toFixed(1)} cm from ${fmt3(want)}, ${turn.toExponential(1)} off quarter turn ${place.turns}` +
            (ok ? "" : ` (the game's own, on ${place.frame}, within ${LIMITS.POSE_TOL_M * 100} cm and the exact turn)`),
    );
  }
  {
    const moves = place ? packMoves(m, place.blueprint, ours.pack_before, ours.pack_after, ours.pack_later) : null;
    const ok = !!moves && moves.every((x) => x.before - x.after === x.n && x.later === x.after);
    add(
      "ours_spent_once",
      ok,
      !moves
        ? "not recorded: the pack before and after the game's place"
        : moves.map((x) => `${x.item}: ${x.before} before, ${x.after} once kept, ${Number.isFinite(x.later) ? x.later : "?"} a few seconds later (the piece takes ${x.n})`).join("; ") + (ok ? "" : ": NOT spent exactly once"),
    );
  }
  {
    const seen = ours.seen_by_walker || null;
    const gap = seen && place ? (num(seen.epoch_ms) - num(place.epoch_ms)) / 1000 : NaN;
    const idOk = !(seen && ours.piece && Number.isInteger(ours.piece.piece_id)) || Number(seen.piece_id) === Number(ours.piece.piece_id);
    const ok = !!seen && !!place && seen.frame === place.frame && dist(seen.local, place.local) <= LIMITS.POSE_TOL_M && Number(seen.turn) === Number(place.turns) && gap >= 0 && gap <= LIMITS.SEEN_WITHIN_S && idOk;
    add(
      "ours_reaches_them",
      ok,
      !place
        ? "not recorded: the game's place"
        : !seen
          ? `the builder never saw the game's ${place.blueprint} built on ${place.frame}`
          : `the builder saw piece ${seen.piece_id} built on ${seen.frame} at ${fmt3(seen.local)} turn ${seen.turn}, ${fs2(gap)} s after the game placed it at ${fmt3(place.local)} turn ${place.turns}` +
            (ok ? "" : ` (the same pose, within ${LIMITS.SEEN_WITHIN_S} s${idOk ? "" : ", and the piece the game holds"})`),
    );
  }

  // ── 5. THE NAMED CASE, both ways.
  const named = m.named || {};
  {
    const r = named.theirs_in_ours || null;
    const frame = r && r.cmd ? r.cmd.frame : null;
    const logged = r ? relayHits(r.relay_lines, RELAY_BUILD_REFUSED_RE).filter((h) => h[1] === "not_allowed" && whyIs(h[2], "not_your_plot") && h[3] === frame) : [];
    const kept = r ? relayHits(r.relay_lines, RELAY_BUILT_RE) : [];
    const drawn = r && Array.isArray(r.drawn_others) ? r.drawn_others : null;
    const ok = !!r && refusedAs(r.refusal, "build", "not_allowed", "not_your_plot") && logged.length === 1 && kept.length === 0 && !!drawn && drawn.length === 0;
    add(
      "refused_their_build_in_our_plot",
      ok,
      !r
        ? "not recorded: the builder's build inside the game's plot"
        : `the builder's build in ${frame}: ${refusalText(r.refusal)}; the relay logged ${logged.length} refusal(s) as someone else's plot${kept.length ? ` and KEPT it (${kept[0][0]})` : ""}; the game drew ${drawn ? (drawn.length ? `piece(s) ${drawn.join(", ")} there` : "nothing new there") : "(not recorded)"}` +
          (ok ? "" : " (refused not_allowed / not_your_plot, by the relay, never drawn)"),
    );
  }
  const refusedHere = (r, sentence, what) => {
    if (!r) return { ok: false, detail: `not recorded: ${what}` };
    const said = shown(r, sentence);
    const moves = r.place ? packMoves(m, r.place.blueprint, r.pack_before, r.pack_after, null) : null;
    const unspent = !!moves && moves.every((x) => Number.isFinite(x.before) && x.before === x.after);
    const sent = relayHits(r.relay_lines, RELAY_BUILT_RE).length + relayHits(r.relay_lines, RELAY_BUILD_REFUSED_RE).length;
    const sentOk = Array.isArray(r.relay_lines) && sent === 0;
    const seen = Array.isArray(r.walker_lines) ? r.walker_lines.filter((l) => /saw built:/.test(l)) : null;
    const pend = r.pending && typeof r.pending === "object" ? r.pending : null;
    const pendOk = !!pend && num(pend.builds) === 0 && num(pend.intents) === 0;
    const ok = said === true && unspent && sentOk && !!seen && seen.length === 0 && pendOk;
    return {
      ok,
      detail:
        `${said === null ? "nothing on screen was recorded" : said ? "the sentence is on screen" : `the sentence "${sentence}" is NOT on screen (notices ${JSON.stringify(r.notices || [])}, hint ${JSON.stringify(r.hint ?? null)})`}; ` +
        `${moves ? (unspent ? "nothing spent" : `SPENT: ${moves.map((x) => `${x.item} ${x.before} -> ${x.after}`).join(", ")}`) : "the pack was not recorded"}; ` +
        `${Array.isArray(r.relay_lines) ? (sent ? `the relay logged ${sent} build line(s) in that window` : "no build line from the relay in that window") : "the relay's log was not recorded"}; ` +
        `${seen ? (seen.length ? `a walker saw a build: ${seen[0]}` : "the walkers saw nothing new") : "the walkers' lines were not recorded"}; ` +
        `${pend ? `waiting: ${num(pend.builds)} build(s), ${num(pend.intents)} to send` : "what the game has waiting was not recorded"}`,
    };
  };
  {
    const r = refusedHere(named.ours_in_theirs || null, PLOT_SENTENCE, "the game's place inside the builder's plot");
    add("refused_our_build_in_their_plot", r.ok, r.detail);
  }

  // ── 6. WHO TAKES DOWN.
  const td = m.take_down || {};
  {
    const r = td.theirs || null;
    const logged = r ? relayHits(r.relay_lines, RELAY_TAKE_DOWN_REFUSED_RE).filter((h) => h[1] === "not_allowed" && whyIs(h[2], "not_your_plot")) : [];
    const gone = r ? relayHits(r.relay_lines, RELAY_TOOK_DOWN_RE) : [];
    const ok = !!r && refusedAs(r.refusal, "unbuild", "not_allowed", "not_your_plot") && logged.length === 1 && gone.length === 0 && r.still_drawn === true;
    add(
      "their_take_down_refused",
      ok,
      !r
        ? "not recorded: the builder's take-down of the game's piece"
        : `the builder took down piece ${r.piece_id}: ${refusalText(r.refusal)}; the relay logged ${logged.length} refusal(s)${gone.length ? " and TOOK IT DOWN" : ""}; the game ${r.still_drawn === true ? "still draws it" : r.still_drawn === false ? "NO LONGER draws it" : "(not recorded)"}` +
          (ok ? "" : " (refused not_allowed / not_your_plot, still standing)"),
    );
  }
  {
    const r = td.ours || null;
    const seen = r && r.seen_by_walker ? r.seen_by_walker : null;
    const gap = seen && r ? (num(seen.epoch_ms) - num(r.epoch_ms)) / 1000 : NaN;
    const ok = !!r && !!seen && Number(seen.piece_id) === Number(r.piece_id) && gap >= 0 && gap <= LIMITS.SEEN_WITHIN_S && r.gone === true;
    add(
      "our_take_down_reaches_them",
      ok,
      !r
        ? "not recorded: the game's take-down of its own piece"
        : `${seen ? `the builder saw piece ${seen.piece_id} come down ${fs2(gap)} s after the game took it down` : `the builder NEVER saw piece ${r.piece_id} come down`}; the game ${r.gone === true ? "no longer has it" : r.gone === false ? "STILL HAS IT" : "(not recorded)"}` +
          (ok ? "" : ` (within ${LIMITS.SEEN_WITHIN_S} s, and gone from the game's world)`),
    );
  }
  {
    const r = td.ours || null;
    const moves = r ? packMoves(m, r.blueprint, r.pack_before, r.pack_after, r.pack_later) : null;
    const ok = !!moves && moves.every((x) => x.after - x.before === x.n && x.later === x.after);
    add(
      "our_take_down_refunds_once",
      ok,
      !moves
        ? "not recorded: the pack before and after the game's take-down"
        : moves.map((x) => `${x.item}: ${x.before} before, ${x.after} once it came down, ${Number.isFinite(x.later) ? x.later : "?"} a few seconds later (the piece gives back ${x.n})`).join("; ") + (ok ? "" : ": NOT given back exactly once"),
    );
  }

  // ── 7. THE SHIP'S SHARED SPACES.
  const zone = m.zone || {};
  add("zone_camera_parked", zone.parked && zone.parked.ok, zone.parked ? zone.parked.detail : "not recorded: the game's walk into the Commons");
  {
    const r = refusedHere(zone.game_place || null, RANK_SENTENCE, "the game's place in the Commons");
    add("zone_refused_without_rank", r.ok, r.detail);
  }
  {
    const r = zone.walker_refused || null;
    const frame = r && r.cmd ? r.cmd.frame : null;
    const logged = r ? relayHits(r.relay_lines, RELAY_BUILD_REFUSED_RE).filter((h) => h[1] === "not_allowed" && whyIs(h[2], "ship_rank") && h[3] === frame) : [];
    const ok = !!r && refusedAs(r.refusal, "build", "not_allowed", "ship_rank") && logged.length === 1;
    add(
      "zone_walker_refused_without_rank",
      ok,
      !r ? "not recorded: the builder's build in the Commons" : `the builder's build in ${frame}: ${refusalText(r.refusal)}; the relay logged ${logged.length} refusal(s) for the rank` + (ok ? "" : " (refused not_allowed / ship_rank)"),
    );
  }
  const wall = zone.wall || {};
  const zFrames = framesOf(wall.samples);
  {
    const rows = Number.isInteger(wall.piece_id) && zFrames ? rowsOf(zFrames, wall.piece_id) : [];
    const gap = rows.length ? (rows[0].epoch - num(wall.line_epoch_ms)) / 1000 : NaN;
    const want = wall.cmd ? shipPoint(frameOf(m, wall.cmd.frame), wall.cmd.local) : null;
    const pos = rows.length ? Math.max(...rows.map((r) => dist(r.row.pos, want))) : Infinity;
    const turn = rows.length && wall.cmd ? Math.max(...rows.map((r) => turnGap(r.row.rot, wall.cmd.turns))) : Infinity;
    const ok = rows.length > 0 && !!wall.cmd && Math.abs(gap) <= LIMITS.SEEN_WITHIN_S && pos <= LIMITS.POSE_TOL_M && turn <= LIMITS.TURN_TOL && rows.every((r) => r.row.frame === wall.cmd.frame);
    add(
      "zone_rank_wall_drawn",
      ok,
      !wall.cmd || !zFrames
        ? "not recorded: the rank holder's wall in the Commons, or the game's recording of it"
        : !Number.isInteger(wall.piece_id)
          ? `the rank holder's wall was never kept: ${refusalText(wall.refusal)}`
          : !rows.length
            ? `piece ${wall.piece_id} NEVER in the game's world`
            : `piece ${wall.piece_id} in the game's world ${fs2(gap)} s after the rank holder heard it kept, at most ${(pos * 100).toFixed(1)} cm from ${fmt3(want)}, ${turn.toExponential(1)} off quarter turn ${wall.cmd.turns}` +
              (ok ? "" : ` (within ${LIMITS.SEEN_WITHIN_S} s, ${LIMITS.POSE_TOL_M * 100} cm and the exact turn)`),
    );
  }
  const after = wall.after ? pics[wall.after] : null;
  const before = wall.before ? pics[wall.before] : null;
  const at = Number.isInteger(wall.piece_id) && zFrames ? rectAt(zFrames, wall.piece_id, num(wall.after_epoch_ms)) : null;
  {
    const size = after ? [after.width, after.height] : Array.isArray(wall.screen_px) ? wall.screen_px.map(Number) : null;
    const rect = at && at.rect ? at.rect.map(Number) : null;
    const ok = !!size && !!rect && rect.every(Number.isFinite) && rect[0] < rect[2] && rect[1] < rect[3] && rect[0] >= 0 && rect[1] >= 0 && rect[2] <= size[0] && rect[3] <= size[1];
    add(
      "zone_wall_in_view",
      ok,
      !size
        ? "not recorded: the picture's size"
        : !at
          ? "the wall was never in the game's recording"
          : !rect
            ? "the wall's screen rect is null: behind the camera"
            : `the wall drawn over x ${rect[0].toFixed(0)}..${rect[2].toFixed(0)}, y ${rect[1].toFixed(0)}..${rect[3].toFixed(0)} of a ${size[0]} x ${size[1]} view` + (ok ? "" : ": NOT all inside it"),
    );
  }
  {
    const box = at && after ? clampRect(at.rect, after.width, after.height) : null;
    const a = box && after ? woodIn(after, box) : null;
    const b = box && before && before.width === after.width && before.height === after.height ? woodIn(before, box) : null;
    const share = a && a.area ? a.count / a.area : NaN;
    const gain = a && b && a.area ? (a.count - b.count) / a.area : NaN;
    const ok = share >= LIMITS.WOOD_MIN_SHARE && gain >= LIMITS.WOOD_GAIN_SHARE;
    add(
      "zone_wall_visible",
      ok,
      !after || !before
        ? "not recorded: the pictures before and with the wall"
        : !box
          ? "no part of the wall's rect is in the picture"
          : !b
            ? "the picture before the wall is another size"
            : `${a.count} of ${a.area} px in the wall's rect are wood with it (${(share * 100).toFixed(0)}%, median ${JSON.stringify(a.median)}), ${b.count} before it (median ${JSON.stringify(b.median)}): a gain of ${(gain * 100).toFixed(0)}%` +
              (ok ? "" : ` (at least ${LIMITS.WOOD_MIN_SHARE * 100}% with it and a gain of ${LIMITS.WOOD_GAIN_SHARE * 100}%; PROVISIONAL limits, see LIMITS.WOOD_*)`),
    );
  }

  // ── 8. DEV KEEPS EVERYTHING ONLY OFFLINE.
  const dev = m.dev || {};
  {
    const j = dev.joined || null;
    const ok = !!j && j.ship_editing === false && j.editor_open === true && j.editor_zone === "home";
    add(
      "dev_off_while_joined",
      ok,
      !j
        ? "not recorded: Dev in the shared world"
        : `in the shared world, in Dev: ship editing ${j.ship_editing}; the build editor ${j.editor_open === true ? "opened" : "did NOT open"}, on zone ${JSON.stringify(j.editor_zone ?? null)}` + (ok ? "" : " (no ship editing; the editor opens on the home)"),
    );
  }
  {
    const o = dev.solo || null;
    const ok = !!o && o.joined === false && o.ship_editing === true && Array.isArray(o.pieces) && o.pieces.length === 0;
    add(
      "dev_on_offline",
      ok,
      !o
        ? "not recorded: Dev out of the shared world"
        : `out of the shared world (joined ${o.joined}): ship editing ${o.ship_editing}, ${Array.isArray(o.pieces) ? o.pieces.length : "?"} shared piece(s) in the world` + (ok ? "" : " (ship editing, and no shared piece)"),
    );
  }
  {
    const back = dev.back || null;
    const got = back && Array.isArray(back.pieces) ? back.pieces.map((p) => Number(p.piece_id)).sort((a, b) => a - b) : null;
    const want = Array.isArray(dev.expect_ids) ? dev.expect_ids.map(Number).sort((a, b) => a - b) : null;
    const ok = !!got && !!want && want.length > 0 && back.joined === true && JSON.stringify(got) === JSON.stringify(want);
    add(
      "solo_back_same_ids",
      ok,
      !back || !want ? "not recorded: the shared pieces after stepping back in" : `back in the shared world: pieces ${JSON.stringify(got)}, before stepping out ${JSON.stringify(want)}` + (ok ? "" : ": NOT the same pieces"),
    );
  }
  {
    const back = dev.back || null;
    const pieces = back && Array.isArray(back.pieces) ? back.pieces : null;
    const young = pieces ? pieces.filter((p) => p.built !== true) : null;
    const ok = !!pieces && pieces.length > 0 && young.length === 0;
    add(
      "solo_back_finished",
      ok,
      !pieces
        ? "not recorded: the shared pieces after stepping back in"
        : young.length
          ? `piece(s) ${young.map((p) => p.piece_id).join(", ")} came back as scaffolds, though each was finished long before: grown again from nothing`
          : `all ${pieces.length} came back finished, as pieces older than their build time must`,
    );
  }

  // ── 9. OVER THE RUN. Every move honest (increment 4's speed check): this run makes no jump, so
  // the game may draw no correction at all, the relay must have sent the game exactly what it took
  // (its log, without the builders' keys: relayCorrections), and the builders stand still.
  const relaySent = typeof evidence.relayLog === "string" && Array.isArray(m.walker_keys) ? relayCorrections(evidence.relayLog, m.walker_keys).total : null;
  const gameTotal = num(m.corrections_game);
  add(
    "moves_game_honest_never_corrected",
    gameTotal === 0,
    Number.isFinite(gameTotal) ? `the relay corrected the game ${gameTotal} time(s) over the run (it makes no jump: none may come)` : "not recorded: the game's corrections over the run",
  );
  add(
    "moves_relay_sent_what_the_game_took",
    Number.isFinite(gameTotal) && relaySent !== null && relaySent === gameTotal,
    relaySent === null ? "not recorded: the corrections the relay sent (its relay.log in the run folder)" : `the relay sent the game ${relaySent} correction(s) and the game applied ${Number.isFinite(gameTotal) ? gameTotal : "?"}`,
  );
  const wl = Array.isArray(m.walker_corrections) ? m.walker_corrections : null;
  add("moves_walkers_never_corrected", !!wl && wl.length === 0, !wl ? "not recorded: the builders' corrections" : wl.length ? `a builder was corrected: ${wl[0]}` : "no builder was ever corrected");
  for (const c of judgeWalks(m.walks, m.turns).checks) checks.push(c);
  const panics = num(m.panics);
  add("no_panics", panics === 0, Number.isFinite(panics) ? `${panics} PANIC line(s) in run.log` : "not recorded: run.log's PANIC lines");
  add(
    "no_builtin_data",
    Array.isArray(m.builtin_data) && m.builtin_data.length === 0,
    !Array.isArray(m.builtin_data)
      ? "not recorded: which data files came from the copy built into the exe"
      : m.builtin_data.length
        ? `${m.builtin_data.length} line(s) in run.log/relay.log serving a built-in copy: ${String(m.builtin_data[0]).replace(/^.*?\[built-in data copy\]\s*/, "")}`
        : "every data file came from the tree's data/ (no built-in copy served), in the game and the relay",
  );
  return { pass: checks.every((c) => c.ok), checks };
}

module.exports = {
  LIMITS,
  PLOT_SENTENCE,
  RANK_SENTENCE,
  RELAY_BUILT_RE,
  RELAY_BUILD_REFUSED_RE,
  RELAY_TOOK_DOWN_RE,
  RELAY_TAKE_DOWN_REFUSED_RE,
  isWoodTint,
  woodIn,
  clampRect,
  shipPoint,
  turnGap,
  rowsOf,
  lostToSlowFrames,
  judgeSharedBuild,
};
