// The rig's foot check (scripts/lib/foot-probe-check.js), fed made-up readouts. Pure node, no game:
// runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/foot-probe-check.test.js
//
// Character animation rung 2a (2026-11-09, docs/design/character-animation.md section 4): the gate
// the feet are measured by. The cases are the ways such a judge goes wrong: passing rung 1's feet
// sunk into stair treads, passing a hovering foot, passing a stair vantage whose figure walks on
// flat floor, and passing a readout in which nobody planted a foot.
//
// Seen red 2026-11-09, each against a deliberate mutation of the judge (and green again after):
// - the sink allowance made infinite (`sink_m: Infinity`): "rung 1's feet through the treads fail"
//   failed, the 0.16 m sink no longer reported;
// - the spread requirement dropped (`minSpread` read as 0): "a stair vantage on flat floor is
//   misconfigured" failed, flat floor passing as proof of the stair.

const test = require("node:test");
const assert = require("node:assert");
const { judgeFeet } = require("../lib/foot-probe-check.js");

function person(who, lowL, highL, lowR, highR, spread = 0.19, frames = 30) {
  const foot = (lo, hi) => ({ planted_frames: frames, lowest_m: lo, highest_m: hi, mean_abs_m: Math.max(Math.abs(lo), Math.abs(hi)) / 2, swing_lowest_m: 0.05 });
  return { who, body: "female", tier: "near", mode: "full", ground_spread_m: spread, on_tread_frames: 4, left: foot(lowL, highL), right: foot(lowR, highR) };
}
const STAIRS = { secs: 1.5, rules: ["feet_on_ground"], min_spread_m: 0.15, who: ["crowd:0"] };

test("feet on the treads pass", () => {
  const v = judgeFeet({ frames: 90, people: [person("crowd:0", -0.004, 0.012, -0.006, 0.009)] }, STAIRS);
  assert.strictEqual(v.ok, true, v.failures.join("; "));
  assert.ok(v.rules.feet_on_ground.worst_sink_m < 0.01);
});

test("rung 1's feet through the treads fail", () => {
  const v = judgeFeet({ frames: 90, people: [person("crowd:0", -0.16, 0.02, -0.12, 0.0)] }, STAIRS);
  assert.strictEqual(v.ok, false);
  assert.strictEqual(v.misconfigured, false);
  assert.match(v.failures.join(" "), /left foot reaches 0\.160 m into the ground/);
});

test("a hovering planted foot fails", () => {
  const v = judgeFeet({ frames: 90, people: [person("crowd:0", 0.0, 0.01, 0.05, 0.09)] }, STAIRS);
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /right foot hovers 0\.090 m/);
});

test("a stair vantage on flat floor is misconfigured, not a pass", () => {
  const v = judgeFeet({ frames: 90, people: [person("crowd:0", -0.002, 0.004, -0.001, 0.003, 0.0)] }, STAIRS);
  assert.strictEqual(v.ok, false);
  assert.strictEqual(v.misconfigured, true);
  assert.match(v.failures[0], /varies only 0\.000 m/);
});

test("nobody planted a foot: misconfigured", () => {
  const v = judgeFeet({ frames: 90, people: [person("crowd:0", 0, 0, 0, 0, 0.2, 2)] }, STAIRS);
  assert.strictEqual(v.misconfigured, true);
  assert.match(v.failures[0], /planted foot-frames/);
});

test("the person the vantage names must be in the readout", () => {
  const v = judgeFeet({ frames: 90, people: [person("me", 0, 0.01, 0, 0.01)] }, STAIRS);
  assert.strictEqual(v.misconfigured, true);
  assert.match(v.failures[0], /has no crowd:0/);
  const none = judgeFeet({ frames: 90, people: [] }, { rules: ["feet_on_ground"] });
  assert.strictEqual(none.misconfigured, true);
});

test("an unknown rule is misconfigured", () => {
  const v = judgeFeet({ frames: 1, people: [person("me", 0, 0, 0, 0)] }, { rules: ["hands_on_rungs"] });
  assert.strictEqual(v.misconfigured, true);
});
