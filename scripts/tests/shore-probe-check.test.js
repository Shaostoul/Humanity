// The rig's shore check (scripts/lib/shore-probe-check.js), fed made-up readouts. Pure node, no
// game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/shore-probe-check.test.js
//
// Whole-Earth maps rung 1 (2026-11-09, docs/design/whole-earth-maps.md section 5.5): the gate the
// coasts are measured by. The cases are the ways such a judge goes wrong: passing a wall because
// the land behind it is high anyway, passing a dry harbour, passing a transect whose ends are on
// the wrong side of the coast, trusting a 5.5 km ocean mask to place a coast, and passing by
// measuring nothing.
//
// Seen red 2026-11-09, each against a deliberate mutation of the judge (and green again after):
// - the wet rule testing `g[i] < -wet_depth_m` for "dry": three tests failed, the gentle shore with
//   "wet: 115 of 115 samples more than 15 m offshore stand above -0.3 m; the highest is -1.06 m";
// - no_false_wall with no allowance (`excess = drawnRise - rawRise`): "a gentle shore passes"
//   failed, "the drawn ground rises 0.80 m over 10 m ... where the land layer rises -0.88 m";
// - every coastline source trusted as fine (`fine = (src) => typeof src === "string"`, so the
//   5.5 km ocean mask places coasts): "a coast on the coarse mask alone is misconfigured" and the
//   along-water test failed, the ends judged on the mask instead of refused;
// - the dry rule counting only land below sea level (`g[i] < 0`): "a drowned island fails dry"
//   failed (`true == false`), land 0.1 m above the sea passing as dry.

const test = require("node:test");
const assert = require("node:assert");
const { judgeShore, summarize, distM } = require("../lib/shore-probe-check.js");

/**
 * A readout along a meridian from `from` to `to`, `n + 1` samples. `at(s_m, t)` gives each
 * sample's { ground, raw, cls, src } (src defaults to a map pack's polygons).
 */
function readout(from, to, n, at) {
  const len = distM(from, to);
  const samples = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const s = len * t;
    const v = at(s, t);
    samples.push({
      s_m: s,
      lat: from[0] + (to[0] - from[0]) * t,
      lon: from[1] + (to[1] - from[1]) * t,
      ground_m: v.ground,
      leaf_depth: 13,
      drawn_m: v.ground,
      raw_m: v.raw === undefined ? v.ground : v.raw,
      raw_source: "land_h:Test",
      class: v.cls,
      class_source: v.src || "pack:Test",
    });
  }
  return { body: "earth", from, to, step_m: len / n, length_m: len, samples };
}

// A 500 m north-south transect, land end north; the coast 200 m along (lat ~47.6432).
const LAND = [47.645, -122.695];
const WATER = [47.6405, -122.695];
const N = 200; // 2.5 m apart
const COAST_S = 200;
const shore = (rules, extra) => Object.assign({ land: LAND, water: WATER, rules }, extra || {});

/** A gentle real shore: 3 m land falling to the water at 1 in 20, a bed that keeps falling. */
const gentle = (s) => {
  const x = s - COAST_S; // positive offshore
  const ground = x < 0 ? Math.min(3, -x / 20) : -Math.min(6, 0.3 + x / 20);
  return { ground, raw: x < 0 ? ground : 1.0, cls: x < 0 ? "land" : "sea" };
};

test("a gentle shore passes wet and no_false_wall", () => {
  const v = judgeShore(readout(LAND, WATER, N, gentle), shore(["wet", "no_false_wall"]));
  assert.ok(v.ok, v.failures.join(" | "));
  assert.equal(v.misconfigured, false);
  assert.ok(Math.abs(v.coast.s_m - COAST_S) < 3, `the coast is found ${v.coast.s_m} m along`);
  assert.ok(v.rules.no_false_wall.measured > 10);
  assert.match(summarize(v), /^ok/);
});

test("a sheer wall at the coast fails no_false_wall, though the land behind is real", () => {
  // The v0.1149 picture: land at 4 m right to the edge, then the carve's -5 m one sample later;
  // the land layer itself rises only 0.4 m there.
  const v = judgeShore(
    readout(LAND, WATER, N, (s) => {
      const x = s - COAST_S;
      return { ground: x < 0 ? 4 : -5, raw: x < 0 ? 4 : 3.6, cls: x < 0 ? "land" : "sea" };
    }),
    shore(["no_false_wall"]),
  );
  assert.equal(v.ok, false);
  assert.match(v.failures.join(" "), /no_false_wall: the drawn ground rises 9\.00 m/);
  assert.ok(v.rules.no_false_wall.worst_excess_m > 6);
});

test("a dry harbour fails wet", () => {
  // The v0.1148 picture: the inlet's coarse cell stands the ground above sea level.
  const v = judgeShore(
    readout(LAND, WATER, N, (s) => {
      const x = s - COAST_S;
      return { ground: x < 0 ? 3 : x < 150 ? 1.2 : -2, cls: x < 0 ? "land" : "sea" };
    }),
    shore(["wet"]),
  );
  assert.equal(v.ok, false);
  assert.equal(v.rules.wet.worst_m, 1.2);
  assert.match(v.failures[0], /wet: \d+ of \d+ samples more than 15 m offshore stand above -0\.3 m/);
});

test("a drowned island fails dry, though it passes wet and shallow", () => {
  // The 2026-11-09 Norderney reading, in small: the coarse grid smears the island into a
  // sea-level flat sloping gently under the sea, so the shallow coast's own rules pass it.
  const far = [47.6315, -122.695];
  const n = 600;
  const coast = 600;
  // Land a hand's breadth above the sea (0.25 m falling to 0.1 m), then under it offshore: a
  // threshold read as "below sea level" would pass this, and it must not.
  const smear = (s) => ({ ground: s < coast ? 0.25 - s / 4000 : -0.05 - (s - coast) / 30, cls: s < coast ? "land" : "sea", src: "ocean_mask:0.05deg" });
  const at = [LAND[0] + (far[0] - LAND[0]) * (coast / distM(LAND, far)), LAND[1]];
  const spec = { land: LAND, water: far, rules: ["wet", "shallow", "dry"], coast: { at, source: "test" } };
  const v = judgeShore(readout(LAND, far, n, smear), spec);
  assert.equal(v.rules.wet.ok, true, "the smear is wet offshore");
  assert.equal(v.rules.shallow.ok, true, "and shallow");
  assert.equal(v.rules.dry.ok, false);
  assert.match(v.failures.join(" "), /dry: 235 of 235 samples more than 15 m inland stand below \+0\.3 m \(0 of them below sea level\)/);
  const island = (s) => ({ ground: s < coast ? 2 + (coast - s) / 100 : -0.5 - (s - coast) / 400, cls: s < coast ? "land" : "sea", src: "ocean_mask:0.05deg" });
  const real = judgeShore(readout(LAND, far, n, island), spec);
  assert.ok(real.rules.dry.ok, real.failures.join(" | "));
});

test("ends on the wrong side of the coast are misconfigured, never judged", () => {
  const swapped = judgeShore(readout(LAND, WATER, N, (s) => ({ ground: 0, cls: "sea" })), shore(["wet"]));
  assert.equal(swapped.misconfigured, true);
  assert.match(swapped.failures[0], /MISCONFIGURED: the land end reads sea/);
  const dry = judgeShore(readout(LAND, WATER, N, (s) => ({ ground: 3, cls: "land" })), shore(["wet"]));
  assert.match(dry.failures[0], /the water end reads land/);
  const other = judgeShore(readout(LAND, [47.62, -122.695], N, gentle), shore(["wet"]));
  assert.match(other.failures[0], /is not the spec's/);
});

test("a coast on the coarse mask alone is misconfigured; declared, it is judged and noted", () => {
  const coarse = (s) => Object.assign(gentle(s), { src: "ocean_mask:0.05deg", cls: "land" });
  const undeclared = judgeShore(readout(LAND, WATER, N, coarse), shore(["wet"]));
  assert.equal(undeclared.misconfigured, true);
  assert.match(undeclared.failures[0], /too coarse to place this coast; declare/);
  const atCoast = [LAND[0] + (WATER[0] - LAND[0]) * (COAST_S / distM(LAND, WATER)), LAND[1]];
  const declared = judgeShore(readout(LAND, WATER, N, coarse), shore(["wet"], { coast: { at: atCoast, source: "test" } }));
  assert.equal(declared.misconfigured, false, declared.failures.join(" | "));
  assert.ok(declared.ok, declared.failures.join(" | "));
  assert.equal(declared.coast.from, "declared");
  assert.match(declared.notes.join(" "), /water end is ocean_mask:0\.05deg, which reads land \(the real water end is water\)/);
  const offLine = judgeShore(readout(LAND, WATER, N, coarse), shore(["wet"], { coast: { at: [atCoast[0], -122.69], source: "test" } }));
  assert.match(offLine.failures[0], /m from the transect: it is not on it/);
  const atEnd = judgeShore(readout(LAND, WATER, N, coarse), shore(["wet"], { coast: { at: WATER, source: "test" } }));
  assert.match(atEnd.failures[0], /at an end of the transect/);
  const noSource = judgeShore(readout(LAND, WATER, N, coarse), shore(["wet"], { coast: { at: atCoast } }));
  assert.match(noSource.failures[0], /needs `at` \[lat, lon\] and a `source`/);
});

test("a declared coast that disagrees with the pack's coastline is misconfigured", () => {
  const far = [LAND[0] + (WATER[0] - LAND[0]) * (300 / distM(LAND, WATER)), LAND[1]];
  const v = judgeShore(readout(LAND, WATER, N, gentle), shore(["wet"], { coast: { at: far, source: "test" } }));
  assert.equal(v.misconfigured, true);
  assert.match(v.failures[0], /disagree by 10\d\.\d m/);
});

test("kept_cliff: a ramp fails, a face passes", () => {
  const ramp = (s) => {
    const x = s - COAST_S;
    return { ground: x < 0 ? -x / 10 : -2, cls: x < 0 ? "land" : "sea" };
  };
  const face = (s) => {
    const x = s - COAST_S;
    return { ground: x < -20 ? 100 : x < 0 ? -x * 5 : -2, cls: x < 0 ? "land" : "sea" };
  };
  const r = judgeShore(readout(LAND, WATER, N, ramp), shore(["kept_cliff"]));
  assert.equal(r.ok, false);
  assert.match(r.failures[0], /reaches only 15\.\d m, not 60 m/);
  const f = judgeShore(readout(LAND, WATER, N, face), shore(["kept_cliff"]));
  assert.ok(f.ok, f.failures.join(" | "));
});

test("shallow: a flat beach passes, a step or a steep bank fails", () => {
  // 1500 m transect so the 300 m window fits either side.
  const far = [47.6315, -122.695];
  const n = 600;
  const coast = 600;
  const flat = (s) => ({ ground: (coast - s) / 200, cls: s < coast ? "land" : "sea" });
  const ok = judgeShore(readout(LAND, far, n, flat), { land: LAND, water: far, rules: ["shallow"] });
  assert.ok(ok.ok, ok.failures.join(" | "));
  const step = judgeShore(readout(LAND, far, n, (s) => ({ ground: (coast - s) / 200 + (s > coast + 50 ? -1 : 0), cls: s < coast ? "land" : "sea" })), { land: LAND, water: far, rules: ["shallow"] });
  assert.equal(step.ok, false);
  assert.match(step.failures.join(" "), /a 1\.0\d m step between samples/);
  const bank = judgeShore(readout(LAND, far, n, (s) => ({ ground: (coast - s) / 10, cls: s < coast ? "land" : "sea" })), { land: LAND, water: far, rules: ["shallow"] });
  assert.match(bank.failures.join(" "), /the slope reaches 5\.7 degrees/);
});

test("no_seam along the water: a step at the old box edge fails; the ends are checked", () => {
  const A = [47.593, -122.658];
  const B = [47.588, -122.658];
  const stepAt = (s, t) => ({ ground: t < 0.6 ? -3 : 4, cls: "sea", src: t < 0.6 ? "pack:Test" : "ocean_mask:0.05deg" });
  const spec = { along_water: [A, B], rules: ["no_seam"], crosses_lat: 47.59, water_source: "test" };
  const v = judgeShore(readout(A, B, 200, stepAt), spec);
  assert.equal(v.ok, false);
  assert.match(v.failures[0], /no_seam: a 7\.00 m step between samples at 47\.59.*runs from -3\.00 m to 4\.00 m/);
  const smooth = judgeShore(readout(A, B, 200, (s, t) => ({ ground: -3 + t * 0.2, cls: "sea" })), spec);
  assert.ok(smooth.ok, smooth.failures.join(" | "));
  const noSource = judgeShore(readout(A, B, 200, stepAt), Object.assign({}, spec, { water_source: undefined }));
  assert.match(noSource.failures[0], /too coarse to say it is water; give `water_source`/);
  const onLand = judgeShore(readout(A, B, 200, (s, t) => ({ ground: 2, cls: t < 0.5 ? "land" : "sea" })), spec);
  assert.match(onLand.failures[0], /the first end of an along-water transect reads land/);
  const notCrossing = judgeShore(readout(A, [47.5915, -122.658], 200, (s, t) => ({ ground: -3, cls: "sea" })), Object.assign({}, spec, { along_water: [A, [47.5915, -122.658]] }));
  assert.match(notCrossing.failures[0], /does not cross latitude 47\.59/);
});

test("a check of nothing never passes", () => {
  assert.match(judgeShore(readout(LAND, WATER, N, gentle), shore([])).failures[0], /names no rules/);
  assert.match(judgeShore(readout(LAND, WATER, N, gentle), shore(["wetness"])).failures[0], /unknown rule/);
  assert.match(judgeShore({ error: "not locked to a body" }, shore(["wet"])).failures[0], /could not measure: not locked to a body/);
  assert.match(judgeShore({ samples: [] }, shore(["wet"])).failures[0], /measuring nothing/);
  // The coast 5 m from the water end: nothing lies 15 m offshore.
  const late = (s) => ({ ground: -1, cls: s < 495 ? "land" : "sea" });
  const v = judgeShore(readout(LAND, WATER, N, late), shore(["wet"]));
  assert.equal(v.ok, false);
  assert.match(v.failures[0], /wet: no sample lies more than 15 m offshore/);
  // No raw land layer anywhere: the wall rule measured nothing.
  const noRaw = judgeShore(readout(LAND, WATER, N, (s) => Object.assign(gentle(s), { raw: null })), shore(["no_false_wall"]));
  assert.match(noRaw.failures[0], /nothing measured/);
  assert.match(judgeShore(readout(LAND, WATER, N, gentle), { along_water: [LAND, WATER], rules: ["wet"] }).failures[0], /shore rules need a land end and a water end/);
});
