// The mirror rig's check (scripts/lib/mirror-check.js), fed made-up probes and captures. Pure
// node, no game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/mirror-check.test.js
//
// Mirrors (2026-11-21, docs/design/mirrors.md): a mirror in view adds the reflection pass and one
// out of view none, counted from the passes the renderer drew; and the player's own body shows in
// the glass, judged by the shown/hidden picture method. The cases are the ways such a judge goes
// wrong: passing a mirror that drew nothing, passing passes with no mirror in view, passing a
// "figure" that is really the whole view changing, or a figure outside the glass.
//
// Seen red 2026-11-21 with figureChange counting every changed pixel as inside the box (the box
// test left out): "AssertionError: a change outside the glass counted as the body".

const test = require("node:test");
const assert = require("node:assert");
const { figureChange, judgePasses, judgeFigure } = require("../lib/mirror-check.js");

/** A w x h image of one grey, with `blobs` ([x, y, w, h]) painted light. */
function image(w, h, blobs = [], grey = 30) {
  const rgba = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) rgba.set([grey, grey, grey, 255], i * 4);
  for (const [bx, by, bw, bh] of blobs) {
    for (let y = by; y < by + bh; y++) {
      for (let x = bx; x < bx + bw; x++) rgba.set([200, 180, 160, 255], (y * w + x) * 4);
    }
  }
  return { width: w, height: h, rgba };
}

// A 400 x 300 window whose mirror glass covers x 150..250, y 50..250 (shares of the window below).
const PROBE = { frames: 60, passes: 60, level: "high", mirrors_placed: 1, active: { id: "standing_mirror_1", size: [320, 640], box: [150 / 400, 50 / 300, 250 / 400, 250 / 300] } };

test("a mirror reflecting on every frame passes; one that drew nothing, or no mirror, fails", () => {
  assert.deepStrictEqual(judgePasses(PROBE, "reflects").failures, []);
  assert.ok(judgePasses({ ...PROBE, passes: 3 }, "reflects").failures.some((f) => f.includes("only 3")), "3 passes in 60 frames is not reflecting");
  assert.ok(judgePasses({ ...PROBE, active: null }, "reflects").failures.some((f) => f.includes("no mirror reflects")));
});

test("no mirror in view passes only with not one pass drawn", () => {
  const none = { frames: 60, passes: 0, level: "high", mirrors_placed: 1, active: null };
  assert.deepStrictEqual(judgePasses(none, "none").failures, []);
  assert.ok(judgePasses({ ...none, passes: 1 }, "none").failures.some((f) => f.includes("1 reflection pass")), "a single pass with no mirror in view fails");
  assert.ok(judgePasses(PROBE, "none").failures.some((f) => f.includes("reflects here")));
  assert.ok(judgePasses(PROBE, "sometimes").failures.some((f) => f.includes("MISCONFIGURED")));
});

test("the body in the glass passes; no body, or a change outside the glass, fails", () => {
  const hidden = image(400, 300);
  const shown = image(400, 300, [[180, 90, 40, 140]]); // a figure inside the glass
  assert.deepStrictEqual(judgeFigure(shown, hidden, PROBE).failures, []);
  assert.ok(judgeFigure(hidden, hidden, PROBE).failures.some((f) => f.includes("not in the reflection")), "the same picture twice has no body");
  const outside = image(400, 300, [[10, 10, 60, 60]]);
  const c = figureChange(outside, hidden, [150, 50, 250, 250]);
  assert.strictEqual(c.inside, 0, "a change outside the glass counted as the body");
  assert.ok(c.outside > 3000);
  const both = image(400, 300, [[180, 90, 40, 140], [0, 0, 120, 120]]);
  assert.ok(judgeFigure(both, hidden, PROBE).failures.some((f) => f.includes("outside the glass changed")), "the whole view changing is not a reflected body");
});

test("the no-body control passes only by failing the figure check", () => {
  const fs = require("fs");
  const os = require("os");
  const path = require("path");
  const PNG = require("../lib/png.js");
  const { judgeMirror } = require("../lib/mirror-check.js");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mirror-check-"));
  const write = (name, img) => {
    const p = path.join(dir, name);
    fs.writeFileSync(p, PNG.encode(img));
    return p;
  };
  const empty = write("empty.png", image(400, 300));
  const body = write("body.png", image(400, 300, [[180, 90, 40, 140]]));
  const control = { expect: "reflects", figure: true, figure_expect: "fail" };
  assert.strictEqual(judgeMirror({ probe: PROBE, opts: control, shownPath: empty, hiddenPath: empty }).ok, true, "no body in either picture: the control passes");
  assert.strictEqual(judgeMirror({ probe: PROBE, opts: control, shownPath: body, hiddenPath: empty }).ok, false, "a body in the shown picture: the control fails");
  fs.rmSync(dir, { recursive: true, force: true });
});

test("scattered single-pixel flicker is not a figure", () => {
  const hidden = image(400, 300);
  const noisy = image(400, 300);
  for (let k = 0; k < 400; k++) {
    const x = 150 + ((k * 37) % 100);
    const y = 50 + ((k * 53) % 200);
    noisy.rgba.set([255, 255, 255, 255], (y * 400 + x) * 4);
  }
  assert.ok(judgeFigure(noisy, hidden, PROBE).failures.some((f) => f.includes("not in the reflection")));
});
