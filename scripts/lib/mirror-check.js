// THE MIRROR CHECK (2026-11-21, docs/design/mirrors.md; the vantage field `mirror_probe` in
// scripts/probe-sweep.js). Two questions about the bedroom's mirror, judged from what the game
// actually drew, never from what it was told:
//
// 1. PASSES. Does a mirror in view add the reflection pass, and a mirror out of view none? The
//    game's mirror probe (src/engine/mirrors.rs `Probe`, the showcase `mirror_probe`) counts the
//    reflection passes the renderer drew over a second (`MirrorView::passes`, counted inside
//    `render_scene_onto` itself) and the frames in that second. `expect: "reflects"`: a mirror is
//    reflecting and there was a pass on (nearly) every frame. `expect: "none"`: not one pass.
// 2. THE FIGURE. Is the player's own body in the reflection, in first person? The copresence
//    judge's shown/hidden picture method (copresence-judge.js judgeCrew): the same view photographed
//    twice, once as it is and once with the player left out of the reflection (the showcase
//    `{"mirror_me":"0"}`), and the pixels that differ counted INSIDE the glass's box on the window
//    (from the probe) and OUTSIDE it. The body must change a real share of the glass and almost
//    nothing elsewhere (the eye's own first-person body is drawn the same in both).
//
// Pure functions over decoded images (scripts/lib/png.js); unit-tested in
// scripts/tests/mirror-check.test.js.

"use strict";

const PNG = require("./png.js");

/** The change mask's rules: a pixel counts as changed when a channel differs by more than
 * `threshold`, and only in a SOLID patch (at least `solid` of its 3x3 neighbourhood changed), so the
 * anti-aliasing's last flicker of a few scattered pixels between two captures is not a figure. */
const THRESHOLD = 24;
const SOLID = 6;

/** Changed pixels of `a` against `b` inside `box` (pixels, [x0, y0, x1, y1]) and outside it. */
function figureChange(a, b, box, threshold = THRESHOLD, solid = SOLID) {
  if (a.width !== b.width || a.height !== b.height) {
    throw new Error(`the two captures differ in size: ${a.width}x${a.height} and ${b.width}x${b.height}`);
  }
  const { width: w, height: h } = a;
  const raw = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) {
    const o = i * 4;
    const d = Math.max(Math.abs(a.rgba[o] - b.rgba[o]), Math.abs(a.rgba[o + 1] - b.rgba[o + 1]), Math.abs(a.rgba[o + 2] - b.rgba[o + 2]));
    if (d > threshold) raw[i] = 1;
  }
  const [x0, y0, x1, y1] = [Math.max(0, Math.floor(box[0])), Math.max(0, Math.floor(box[1])), Math.min(w, Math.ceil(box[2])), Math.min(h, Math.ceil(box[3]))];
  let inside = 0;
  let outside = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!raw[y * w + x]) continue;
      let n = 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const yy = y + dy;
          const xx = x + dx;
          if (yy >= 0 && yy < h && xx >= 0 && xx < w && raw[yy * w + xx]) n++;
        }
      }
      if (n < solid) continue;
      if (x >= x0 && x < x1 && y >= y0 && y < y1) inside++;
      else outside++;
    }
  }
  const boxPx = Math.max(0, x1 - x0) * Math.max(0, y1 - y0);
  const outPx = w * h - boxPx;
  return { inside, outside, box_px: boxPx, inside_frac: boxPx ? inside / boxPx : 0, outside_frac: outPx ? outside / outPx : 0 };
}

/** Question 1: the passes against the expectation. */
function judgePasses(probe, expect) {
  const failures = [];
  const notes = [];
  const frames = Number(probe && probe.frames) || 0;
  const passes = Number(probe && probe.passes) || 0;
  notes.push(`${passes} reflection pass(es) in ${frames} frame(s), level ${probe && probe.level}, ${probe && probe.mirrors_placed} mirror(s) placed`);
  if (expect === "reflects") {
    if (!probe || !probe.active) failures.push("no mirror reflects here, and one should (the mirror is in view)");
    if (frames === 0) failures.push("the probe counted no frames");
    else if (passes < Math.floor(frames * 0.8)) failures.push(`only ${passes} reflection pass(es) in ${frames} frame(s): a mirror in view must reflect on every frame`);
  } else if (expect === "none") {
    if (probe && probe.active) failures.push(`${probe.active.id} reflects here, and nothing should (no mirror in view)`);
    if (passes !== 0) failures.push(`${passes} reflection pass(es) drawn with no mirror in view: nothing should be drawn`);
  } else {
    failures.push(`mirror_probe.expect is ${JSON.stringify(expect)}; it must be "reflects" or "none" (MISCONFIGURED)`);
  }
  return { failures, notes };
}

/** Question 2: the figure in the glass, from the shown and hidden pictures and the probe's box. */
function judgeFigure(shown, hidden, probe, opts = {}) {
  const failures = [];
  const notes = [];
  const box = probe && probe.active && probe.active.box;
  if (!Array.isArray(box) || box.length !== 4) {
    failures.push("the probe gave no box for the glass on the window (no mirror reflecting, or a corner behind the eye)");
    return { failures, notes, change: null };
  }
  const px = [box[0] * shown.width, box[1] * shown.height, box[2] * shown.width, box[3] * shown.height];
  const change = figureChange(shown, hidden, px);
  const minIn = opts.min_inside_frac ?? 0.02;
  const maxOut = opts.max_outside_frac ?? 0.003;
  notes.push(
    `the glass's box ${px.map((v) => Math.round(v)).join(",")} (${change.box_px} px): ${change.inside} px changed inside (${(change.inside_frac * 100).toFixed(2)}%), ${change.outside} outside (${(change.outside_frac * 100).toFixed(3)}%)`,
  );
  if (change.inside_frac < minIn) failures.push(`the player's body changes only ${(change.inside_frac * 100).toFixed(2)}% of the glass (at least ${(minIn * 100).toFixed(1)}% wanted): the body is not in the reflection`);
  if (change.outside_frac > maxOut) failures.push(`${(change.outside_frac * 100).toFixed(3)}% of the window outside the glass changed too (at most ${(maxOut * 100).toFixed(2)}%): the two pictures differ by more than the reflected body`);
  return { failures, notes, change };
}

/** The whole verdict for a vantage: `shownPath` / `hiddenPath` may be null when the vantage only
 * counts passes. */
function judgeMirror({ probe, opts, shownPath, hiddenPath }) {
  const fs = require("fs");
  const p = judgePasses(probe, opts.expect);
  const failures = [...p.failures];
  const notes = [...p.notes];
  let change = null;
  if (shownPath && hiddenPath) {
    const f = judgeFigure(PNG.decode(fs.readFileSync(shownPath)), PNG.decode(fs.readFileSync(hiddenPath)), probe, opts);
    notes.push(...f.notes);
    change = f.change;
    if (opts.figure_expect === "fail") {
      // A CONTROL (the red twin on real pixels): the player is left out of the reflection in BOTH
      // pictures, so the figure check must fail; passing would mean it sees a body that is not there.
      if (f.failures.length === 0) failures.push("the control passed the figure check with no body in the reflection: the check cannot fail");
      else notes.push(`control: the figure check failed as it must (${f.failures.join("; ")})`);
    } else {
      failures.push(...f.failures);
    }
  }
  return { ok: failures.length === 0, misconfigured: failures.some((f) => f.includes("MISCONFIGURED")), failures, notes, change };
}

function summarize(v) {
  return v.ok ? `ok (${v.notes.join("; ")})` : `FAILED: ${v.failures.join(" | ")}`;
}

module.exports = { figureChange, judgePasses, judgeFigure, judgeMirror, summarize, THRESHOLD, SOLID };
