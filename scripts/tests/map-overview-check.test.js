// The rig's Maps overview check (scripts/lib/map-overview-check.js), fed
// made-up reports. Pure node, no game: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/map-overview-check.test.js
//
// Maps increment 2 (2026-10-21): the overview must draw the real star sky
// behind, the belts' dust, the orbits, the bodies and their names, inside the
// symbols' 1 ms budget. The cases below are the ways such a judge goes wrong:
// passing with no star sky, passing a map that lost its belts (the control
// vantage), passing a cost it never read, and passing an over-budget frame.
//
// Seen red 2026-10-21 with the belt count not checked (the line commented
// out): "AssertionError [ERR_ASSERTION]: a map with no belts passed".

const test = require("node:test");
const assert = require("node:assert");
const { judgeOverview, vantageVerdict } = require("../lib/map-overview-check.js");

function report(over = {}) {
  const base = {
    ok: true,
    page_open: true,
    world: { want: "TooFar", renders: 3, sky_only: true },
    symbology: { band: "SolarSystem", bodies: 42, orbits: 21, labels_placed: 9, labels_dropped: 4, belt_points: 16800, ms: 0.6 },
  };
  return {
    ...base,
    ...over,
    world: { ...base.world, ...(over.world || {}) },
    symbology: { ...base.symbology, ...(over.symbology || {}) },
  };
}

test("a full overview with its sky, belts and names passes", () => {
  const c = judgeOverview(report(), {}, { cpu_ms: { "cpu.maps_symbology": 0.42 } });
  assert.ok(c.ok, c.message);
  assert.match(c.message, /16800 belt points/);
});

test("a map with no belts fails, and the control vantage passes by failing", () => {
  const c = judgeOverview(report({ symbology: { belt_points: 0 } }));
  assert.ok(!c.ok, "a map with no belts passed");
  assert.match(c.message, /0 belt points/);
  assert.ok(vantageVerdict(c, "fail").ok);
  assert.ok(!vantageVerdict(judgeOverview(report()), "fail").ok, "a control that passes the check must fail the vantage");
});

test("no star sky behind the symbols fails", () => {
  assert.ok(!judgeOverview(report({ world: { renders: 0 } })).ok);
  assert.ok(!judgeOverview(report({ world: { sky_only: false, want: "Draw" } })).ok, "a whole-world render is not the overview's sky");
});

test("the cost is judged when the costs file is there, and a missing id fails", () => {
  assert.ok(!judgeOverview(report(), {}, { cpu_ms: { "cpu.maps_symbology": 1.7 } }).ok, "an over-budget frame passed");
  const missing = judgeOverview(report(), {}, { cpu_ms: { "cpu.scene": 2.0 } });
  assert.ok(!missing.ok);
  assert.match(missing.message, /was not recorded/);
  assert.ok(!judgeOverview(report({ symbology: { ms: 2.5 } })).ok, "the page's own clock over budget passed");
});

test("the wrong band and no names fail", () => {
  assert.ok(!judgeOverview(report({ symbology: { band: "NearSpace" } })).ok);
  assert.ok(!judgeOverview(report({ symbology: { labels_placed: 1 } })).ok);
  assert.ok(!judgeOverview(null).ok);
});
