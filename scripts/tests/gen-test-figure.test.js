// The procedural test figure and its clip library (character animation rung 1, 2026-11-08): the
// committed glTF files must be exactly what scripts/gen-test-figure.js makes, so the body the tests
// and rigs prove skinning and retargeting with can always be rebuilt from its source.
//
// Seen red 2026-11-08 with the generator's walk thigh swing changed from 25 to 26 degrees and the
// files not regenerated: "assets/models/characters/hos_test_figure/hos_test_figure.bin differs".

"use strict";
const test = require("node:test");
const assert = require("node:assert");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.join(__dirname, "..", "..");

test("the committed test figure and clips match the generator", () => {
  const r = spawnSync(process.execPath, [path.join(ROOT, "scripts", "gen-test-figure.js"), "--check"], { encoding: "utf8" });
  assert.strictEqual(r.status, 0, `${r.stdout}\n${r.stderr}`);
});

test("the canonical walk swings each leg forward of the hips and back again", () => {
  const { CLIPS, turnsFrom, qrot } = require(path.join(ROOT, "scripts", "gen-test-figure.js"));
  const walk = CLIPS.find((c) => c.name === "walk");
  // The left thigh points down (-Y) at rest; forward is +Z.
  const zAt = (p) => qrot(turnsFrom(walk.at(p).L).leftUpperLeg, [0, -1, 0])[2];
  assert.ok(zAt(0.25) > 0.3, `forward at a quarter: ${zAt(0.25)}`);
  assert.ok(zAt(0.75) < -0.3, `back at three quarters: ${zAt(0.75)}`);
});
