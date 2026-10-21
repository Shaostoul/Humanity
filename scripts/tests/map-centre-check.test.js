// The rig's Maps "ship centred" check (scripts/lib/map-centre-check.js), fed
// made-up captures. Pure node, no game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/map-centre-check.test.js
//
// Maps increment 1 (2026-10-21): the map follows your ship, and the rig proves
// it on rendered pixels (the ship is what changes when it is left out of the
// world layer). The cases below are the ways such a judge goes wrong: passing
// a ship that is not in the middle, passing when there is no ship at all (the
// unfollowed control), being fooled by cloud noise, or reading the whole
// window instead of the map's canvas.
//
// Seen red 2026-10-21 with `shipMask` reading from the window's top edge
// instead of the canvas's (`const t = 0`): "AssertionError: a change outside
// the canvas counted as ship".

const test = require("node:test");
const assert = require("node:assert");
const { judgeShipCentred, shipMask, vantageVerdict } = require("../lib/map-centre-check.js");

/** A w x h image of one grey, with a `blob` (x, y, w, h) painted white. */
function image(w, h, blob = null, grey = 20) {
  const rgba = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) {
    rgba.set([grey, grey, grey, 255], i * 4);
  }
  if (blob) {
    const [bx, by, bw, bh] = blob;
    for (let y = by; y < by + bh; y++) {
      for (let x = bx; x < bx + bw; x++) rgba.set([230, 230, 230, 255], (y * w + x) * 4);
    }
  }
  return { width: w, height: h, rgba };
}

// A 400x300 window whose map canvas is the right-hand 300x260 below a 40 px bar.
const CANVAS = [100, 40, 400, 300];
const CENTRE = [250, 170];

test("a ship in the middle of the canvas is centred", () => {
  const withShip = image(400, 300, [CENTRE[0] - 40, CENTRE[1] - 25, 80, 50]);
  const v = judgeShipCentred({ withShip, withoutShip: image(400, 300), canvas: CANVAS });
  assert.strictEqual(v.ok, true, v.message);
  assert.ok(v.offset_px < 2, v.message);
});

test("a ship near the canvas edge is not centred, and the message says how far", () => {
  const withShip = image(400, 300, [330, 50, 60, 40]);
  const v = judgeShipCentred({ withShip, withoutShip: image(400, 300), canvas: CANVAS });
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /NOT centred/);
});

test("no ship at all (the unfollowed control) fails as no ship", () => {
  const v = judgeShipCentred({ withShip: image(400, 300), withoutShip: image(400, 300), canvas: CANVAS });
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /no ship in the map/);
});

test("cloud noise under the threshold is not ship", () => {
  const a = image(400, 300, null, 20);
  const b = image(400, 300, null, 45); // 25 levels everywhere: re-marched clouds
  const m = shipMask(a, b, CANVAS, 40);
  assert.strictEqual(m.count, 0);
});

test("a twinkling star field is not ship, however it is spread", () => {
  // Single bright pixels changing all over the canvas, the way the star field
  // differs between two renders: their centroid is the canvas centre, which
  // passed the near control on twinkle alone before the solid-patch rule.
  // Seen red 2026-10-21 with the rule off (solid = 1): the twinkle passed as
  // a centred ship (`v.ok` true).
  const a = image(400, 300);
  const b = image(400, 300);
  for (let y = CANVAS[1] + 1; y < CANVAS[3] - 1; y += 4) {
    for (let x = CANVAS[0] + 1; x < CANVAS[2] - 1; x += 4) b.rgba.set([230, 230, 230, 255], (y * 400 + x) * 4);
  }
  const v = judgeShipCentred({ withShip: a, withoutShip: b, canvas: CANVAS });
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /no ship in the map/, "twinkle counted as ship");
});

test("only the canvas is read, never the bars around it", () => {
  // A change in the 40 px bar above the canvas (the clock ticking) and in the
  // left panel: outside the canvas, so no ship.
  const withShip = image(400, 300, [0, 0, 400, 39]);
  const v = judgeShipCentred({ withShip, withoutShip: image(400, 300), canvas: CANVAS });
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /no ship in the map/, "a change outside the canvas counted as ship");
});

test("a control passes only by failing the check", () => {
  assert.strictEqual(vantageVerdict({ ok: false, message: "no ship" }, "fail").ok, true);
  const v = vantageVerdict({ ok: true, message: "ship centred" }, "fail");
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /cannot fail/);
  assert.strictEqual(vantageVerdict({ ok: true, message: "ship centred" }).ok, true);
});
