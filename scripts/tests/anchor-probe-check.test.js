// The rig's interaction check (scripts/lib/anchor-probe-check.js), fed made-up readouts. Pure node,
// no game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/anchor-probe-check.test.js
//
// Character animation rung 2b (2026-11-10, docs/design/character-animation.md section 17): the gate
// the drawn body is measured by against what it touches. The cases are the ways such a judge goes
// wrong: passing hips standing over a seat (the clip's own seat height on a lower bench), passing a
// climbing hand off its rung, passing a first-person head drawn round the eye, passing a vantage with
// no body under the eye, and passing a readout in which nobody held the anchor.
//
// Seen red 2026-11-10, each against a deliberate mutation of the judge (and green again after):
// - the seat allowance made infinite (`seat_float_m: Infinity`): "hips over a lower bench fail"
//   failed, the 0.071 m float no longer reported;
// - the body-under-the-eye requirement dropped: "no body under the eye fails" failed, an empty view
//   passing as a hidden head.
// - the soles' clearance dropped (`feet_clear_m: -Infinity`, 2026-11-10, the hanging legs): "legs hanging
//   from a high seat pass; planted feet or thighs through the seat fail" failed, the planted twin's
//   soles 0.003 m over the floor no longer reported ("did not match /soles on the floor/").

const test = require("node:test");
const assert = require("node:assert");
const { judgeAnchors } = require("../lib/anchor-probe-check.js");

const m = (lo, hi, frames = 30) => ({ frames, lowest_m: lo, highest_m: hi, mean_abs_m: Math.max(Math.abs(lo), Math.abs(hi)) / 2 });
const person = (who, anchor, measures) => ({ who, body: "male", anchor, kind: "Sit", phase_frames: { enter: 0, hold: 45, exit: 0 }, measures });
const SEAT = { secs: 1.5, rules: ["hips_on_seat"], who: ["crowd:0"] };

test("hips on the seat pass", () => {
  const v = judgeAnchors({ frames: 45, people: [person("crowd:0", "couch_seat_middle", { hips_m: m(-0.006, 0.011) })] }, SEAT);
  assert.strictEqual(v.ok, true, v.failures.join("; "));
});

test("hips over a lower bench fail", () => {
  const v = judgeAnchors({ frames: 45, people: [person("crowd:0", "couch_seat_middle", { hips_m: m(0.064, 0.071) })] }, SEAT);
  assert.strictEqual(v.ok, false);
  assert.strictEqual(v.misconfigured, false);
  assert.match(v.failures.join(" "), /stands 0\.071 m over the seat/);
});

test("hips sunk into the seat fail", () => {
  const v = judgeAnchors({ frames: 45, people: [person("crowd:0", "chair_seat", { hips_m: m(-0.05, 0.0) })] }, SEAT);
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /reaches 0\.050 m into the seat/);
});

test("a hand off its rung fails, one on it passes", () => {
  const spec = { rules: ["hands_on_rungs"], who: ["crowd:0"] };
  const on = judgeAnchors({ frames: 45, people: [person("crowd:0", "ladder_climb", { left_hand_m: m(-0.004, 0.012), right_hand_m: m(-0.002, 0.015) })] }, spec);
  assert.strictEqual(on.ok, true, on.failures.join("; "));
  const off = judgeAnchors({ frames: 45, people: [person("crowd:0", "ladder_climb", { left_hand_m: m(0.18, 0.31) })] }, spec);
  assert.strictEqual(off.ok, false);
  assert.match(off.failures.join(" "), /left_hand_m is 0\.310 m off its rung/);
});

test("a first-person head round the eye fails, a hidden one passes", () => {
  const spec = { rules: ["head_hidden"], who: ["me"] };
  const hidden = judgeAnchors({ frames: 45, people: [person("me", "first_person", { eye_nearest_m: m(0.16, 0.22), body_below_m: m(1.5, 1.52) })] }, spec);
  assert.strictEqual(hidden.ok, true, hidden.failures.join("; "));
  const round = judgeAnchors({ frames: 45, people: [person("me", "first_person", { eye_nearest_m: m(0.004, 0.02), body_below_m: m(1.5, 1.52) })] }, spec);
  assert.strictEqual(round.ok, false);
  assert.match(round.failures.join(" "), /inside the camera's near plane/);
});

test("no body under the eye fails", () => {
  const v = judgeAnchors({ frames: 45, people: [person("me", "first_person", { eye_nearest_m: m(1.2, 1.4), body_below_m: m(0.1, 0.2) })] }, { rules: ["head_hidden"], who: ["me"] });
  assert.strictEqual(v.ok, false);
  assert.match(v.failures.join(" "), /no body drawn under the eye/);
});

test("legs hanging from a high seat pass; planted feet or thighs through the seat fail", () => {
  const spec = { rules: ["legs_hang"], who: ["crowd:0"] };
  const legs = (thigh, feet) => ({ people: [{ who: "crowd:0", body: "female", anchor: "chair_seat", measures: { thigh_m: m(thigh, thigh), feet_m: m(feet, feet) } }] });
  assert.equal(judgeAnchors(legs(-0.047, 0.055), spec).ok, true);
  const planted = judgeAnchors(legs(-0.055, 0.003), spec);
  assert.equal(planted.ok, false);
  assert.match(planted.failures.join(" "), /soles on the floor/);
  assert.match(planted.failures.join(" "), /thighs driven through/);
  const missing = judgeAnchors({ people: [{ who: "crowd:0", body: "female", anchor: "chair_seat", measures: { thigh_m: m(-0.01, -0.01) } }] }, spec);
  assert.equal(missing.misconfigured, true);
});

test("a readout with nobody holding the anchor is misconfigured", () => {
  const few = judgeAnchors({ frames: 45, people: [person("crowd:0", "chair_seat", { hips_m: m(0.0, 0.0, 2) })] }, SEAT);
  assert.strictEqual(few.misconfigured, true);
  const missing = judgeAnchors({ frames: 45, people: [] }, SEAT);
  assert.strictEqual(missing.misconfigured, true);
  assert.match(missing.failures.join(" "), /has no crowd:0/);
});
