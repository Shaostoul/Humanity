// The rig's bed check (scripts/lib/bed-probe-check.js), fed made-up readouts. Pure node, no game:
// runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/bed-probe-check.test.js
//
// Whole-Earth maps, the rung between 2 and 3 (2026-11-09, docs/design/whole-earth-maps.md 6.1 and
// 6.2): the gate the road beds and building pads are measured by. The cases are the ways such a
// judge goes wrong: passing rung 2's floating ribbon, passing rung 2's buried skirt, passing a
// flat street or a flat lot that proves nothing, and passing a readout with no drawn ground.
//
// Seen red 2026-11-09, each against a deliberate mutation of the judge (and green again after):
// - the bury allowance made infinite (`bury_m: Infinity`): "rung 2's ribbon on a side slope fails"
//   failed, its buried uphill edge no longer reported;
// - the skirt allowance made infinite (`skirt_m: Infinity`): "rung 2's skirt fails" failed;
// - the slope requirement dropped (`min_rise_m: 0`): "a flat street is misconfigured, not a pass"
//   failed, a street on flat ground passing as proof of a bed.

const test = require("node:test");
const assert = require("node:assert");
const { judgeBeds } = require("../lib/bed-probe-check.js");

/** A road cross-section: natural ground `raw(off)`, drawn ground `drawn(off)`, ribbon `rib(off)`. */
function road(raw, drawn, rib, half = 3.25) {
  const samples = [];
  for (let i = -40; i <= 40; i++) {
    const off = i * 0.25;
    samples.push({ off_m: off, raw_m: raw(off), drawn_m: drawn(off), ground_m: drawn(off), leaf_depth: 20, ribbon_m: Math.abs(off) <= half ? rib(off) : null });
  }
  return { kind: "road", osm_id: 1, class: 3, half_w_m: half, core_m: half + 1, from_asked_m: 1.0, beds: true, walk_depth: 20, pack_depth: 20, samples };
}
const ROAD = { road: [47.6, -122.6], rules: ["road_on_bed"] };
const slope = (off) => 5 + 0.15 * off; // a 15% side slope

test("a road on its bed passes", () => {
  const v = judgeBeds(road(slope, () => 5, () => 5.04), ROAD);
  assert.strictEqual(v.ok, true, v.failures.join("; "));
  assert.ok(v.rules.road_on_bed.drawn_spread_m < 0.01);
});

test("rung 2's ribbon on a side slope fails (ground follows the slope, ribbon at the centre plus 0.18)", () => {
  const v = judgeBeds(road(slope, slope, () => 5.18), ROAD);
  assert.strictEqual(v.ok, false);
  assert.strictEqual(v.misconfigured, false);
  assert.ok(v.failures.some((f) => f.includes("no flat bed")), v.failures.join("; "));
  assert.ok(v.failures.some((f) => f.includes("buried edge")), v.failures.join("; "));
  assert.ok(v.failures.some((f) => f.includes("floats")), v.failures.join("; "));
});

test("a flat street is misconfigured, not a pass", () => {
  const v = judgeBeds(road(() => 5, () => 5, () => 5.04), ROAD);
  assert.strictEqual(v.misconfigured, true);
});

test("a cross-section with no drawn ground is misconfigured", () => {
  const r = road(slope, () => 5, () => 5.04);
  r.samples[40].drawn_m = null;
  assert.strictEqual(judgeBeds(r, ROAD).misconfigured, true);
});

/** A building's corners: natural `raws`, drawn `drawns`, the mesher's floor and bottom. */
function building(raws, drawns, floor, bottom) {
  return {
    kind: "building", osm_id: 2, floor_m: floor, apron_m: 1.5, mesh_floor_m: floor, mesh_bottom_m: bottom, beds: true, walk_depth: 20, pack_depth: 20,
    corners: raws.map((r, i) => ({ lat: 0, lon: 0, raw_m: r, drawn_m: drawns[i], ground_m: drawns[i], leaf_depth: 20, apron_drawn_m: drawns[i] })),
  };
}
const BLD = { building: [47.6, -122.6], rules: ["building_on_pad"] };

test("a building on its pad passes", () => {
  const v = judgeBeds(building([4, 5, 6, 5], [5, 5, 5, 5], 5, 4), BLD);
  assert.strictEqual(v.ok, true, v.failures.join("; "));
});

test("rung 2's skirt fails (floor at the lowest corner, walls 2.5 m below it, ground on the slope)", () => {
  const v = judgeBeds(building([4, 5, 6, 5], [4, 5, 6, 5], 4, 1.5), BLD);
  assert.strictEqual(v.ok, false);
  assert.ok(v.failures.some((f) => f.includes("sunk into the slope")), v.failures.join("; "));
  assert.ok(v.failures.some((f) => f.includes("buried skirt")), v.failures.join("; "));
});

test("a wall that starts above the ground fails (daylight under it)", () => {
  const v = judgeBeds(building([4, 5, 6, 5], [5, 5, 5, 4.9], 5, 5), BLD);
  assert.ok(v.failures.some((f) => f.includes("daylight")), v.failures.join("; "));
});

test("a flat lot is misconfigured", () => {
  assert.strictEqual(judgeBeds(building([5, 5, 5, 5], [5, 5, 5, 5], 5, 5), BLD).misconfigured, true);
});

test("asking for a road and reading a building is misconfigured", () => {
  assert.strictEqual(judgeBeds(building([4, 5, 6, 5], [5, 5, 5, 5], 5, 4), ROAD).misconfigured, true);
});
