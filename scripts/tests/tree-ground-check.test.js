// The probe rig's ground_probe judgement (scripts/lib/tree-ground-check.js),
// fed made-up readouts. Pure node, no game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/tree-ground-check.test.js
//
// BUG-156 (2026-10-05): trees hung metres above the drawn ground beside the
// Silverdale waterfront, and the only witness was a picture. The game now
// reads out each tree's base against the ground drawn under it
// (src/engine/tree_ground.rs) and this judges it. The cases below are the
// ways a judge like this goes wrong: passing a floating stand, passing by
// measuring nothing, or not noticing the player floats too.

const test = require("node:test");
const assert = require("node:assert");
const { judgeTreeGround, DEFAULTS } = require("../lib/tree-ground-check.js");

/** A readout of `gaps` (metres, base above the drawn ground), all on screen. */
function readout(gaps, { eyeAbove = 1.75, offScreen = [] } = {}) {
  const tree = (g, i, on) => ({
    i,
    on_screen: on,
    dist_m: 50 + i * 10,
    leaf_depth: 18,
    gap_drawn_m: g,
  });
  return {
    eye: { above_drawn_m: eyeAbove },
    harvest: { depth: 20 },
    trees: [...gaps.map((g, i) => tree(g, i, true)), ...offScreen.map((g, i) => tree(g, gaps.length + i, false))],
  };
}

test("a stand on the drawn ground passes", () => {
  const v = judgeTreeGround(readout([-0.03, -0.02, 0.01, 0.05]), true);
  assert.strictEqual(v.ok, true, v.failures.join(" | "));
  assert.strictEqual(v.summary.trees_on_screen, 4);
});

test("one tree floating over the drawn ground fails, and is named", () => {
  // BUG-156's picture: most of the stand fine, a few hanging metres up.
  const v = judgeTreeGround(readout([-0.02, 0.0, 6.4, 0.04]), true);
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /1 of 4 on-screen trees float/);
  assert.match(v.failures.join(" "), /#2 \+6\.40 m/);
});

test("a buried tree fails too", () => {
  const v = judgeTreeGround(readout([0.0, -3.2]), true);
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /sunk more than/);
});

test("trees off the screen are not judged", () => {
  // The readout covers the whole near set; only what the capture shows counts.
  const v = judgeTreeGround(readout([0.02], { offScreen: [9.0, -9.0] }), true);
  assert.strictEqual(v.ok, true, v.failures.join(" | "));
});

test("a check that measured nothing fails rather than passes", () => {
  const v = judgeTreeGround(readout([], { offScreen: [0.0] }), true);
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /measuring nothing/);
});

test("an eye not standing on the drawn ground fails", () => {
  // The same defect under the player: the walk clamp stood the eye on the
  // surface the survey DEM replaced.
  const v = judgeTreeGround(readout([0.0], { eyeAbove: 9.3 }), true);
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /eye is 9\.30 m above the drawn ground/);
});

test("the game reporting it could not measure fails", () => {
  const v = judgeTreeGround({ error: "not standing on a chunked body" }, true);
  assert.strictEqual(v.ok, false);
  assert.strictEqual(judgeTreeGround(null, true).ok, false);
});

test("a vantage's own limits override the defaults", () => {
  const r = readout([0.5]);
  assert.strictEqual(judgeTreeGround(r, true).ok, false, `0.5 m is past the default ${DEFAULTS.max_float_m} m`);
  assert.strictEqual(judgeTreeGround(r, { max_float_m: 0.6 }).ok, true);
});
