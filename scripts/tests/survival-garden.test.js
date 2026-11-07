// The survival garden's linear program (scripts/survival-garden.js, docs/design/survival-garden.md),
// written 2026-11-07 with the operator's answers of that day: the comfortable garden is the start,
// the mushroom room is in the minimal garden with its growing medium modelled, and the crop data
// fixes are in. Until this file the program had no test of its own.
//
// Red evidence, each seen before the code or data was right (the scratch script sg3-red-garden.js
// copied the script, this test and the data it reads to a scratch tree and broke one thing at a
// time, 2026-11-07):
//   - "the simplex solves a small program": with Bland's entering test flipped (rc > EPS) it failed
//     with "feasible" (the simplex lost the program, and every garden with it).
//   - "a crop's residue": with the water content left out it failed with "soybean residue:
//     expected 13.718999999999996, got 15".
//   - "a mushroom's substrate": with the biological efficiency multiplied instead of divided it
//     failed with "oyster need: expected 151.5151515151515, got 66".
//   - "the growing medium caps the mushrooms": with the substrate row taken out of solve() it failed
//     with "the mushrooms need more than the residue gives (443.9 g against 243.9 g)", and the bare
//     garden's shipped numbers with it ("expected 135, got 123").
//   - "a garden that cannot carry the vitamin D shelf": with the fallback removed the tier came back
//     infeasible ("expected a garden, got infeasible").
//   - "the shipped settings": with start_tier set back to "bare" it failed with "start tier:
//     expected comfort, got bare".
//   - "the design doc matches the data": with the residue's usable share changed to 0.5 in a scratch
//     copy, --check exited 1 ("the design doc's generated block is stale").

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const path = require('path');
const { execFileSync } = require('child_process');
const sg = require('../survival-garden.js');

const SCRIPT = path.join(__dirname, '..', 'survival-garden.js');
const near = (a, b, tol, what) => assert.ok(Math.abs(a - b) <= tol, `${what}: expected ${b}, got ${a}`);

test('the simplex solves a small program worked by hand', () => {
  // minimise x + y with x + 2y >= 4 and 3x + y >= 6: the corner x = 1.6, y = 1.2, total 2.8.
  const r = sg.simplex([1, 1], [
    { a: [1, 2], op: '>=', b: 4 },
    { a: [3, 1], op: '>=', b: 6 },
  ]);
  assert.ok(r.ok, 'feasible');
  near(r.x[0], 1.6, 1e-9, 'x');
  near(r.x[1], 1.2, 1e-9, 'y');
  near(r.value, 2.8, 1e-9, 'total');
  assert.strictEqual(sg.simplex([1], [{ a: [1], op: '<=', b: 1 }, { a: [1], op: '>=', b: 2 }]).ok, false, 'an impossible program says so');
});

test('a crop\'s residue: edible dry matter x (1 - harvest index) / harvest index', () => {
  const D = sg.load();
  // Soybean: USDA water 8.54 g/100 g, BVAD harvest index 40 percent, edible share 1.
  const got = sg.residueDryG(D, 'soybean', { grossG: 10, floorWeight: 1 });
  near(got, 10 * (1 - 0.0854) * (0.6 / 0.4), 1e-9, 'soybean residue');
  near(got, 13.719, 0.001, 'soybean residue');
  // No measured harvest index, no residue counted (an underestimate, said so in the data).
  assert.strictEqual(sg.residueDryG(D, 'broccoli', { grossG: 10, floorWeight: 1 }), 0);
  // A mushroom leaves no residue of its own.
  assert.strictEqual(sg.residueDryG(D, 'oyster_mushroom', { grossG: 10, floorWeight: 0.2 }), 0);
});

test('a mushroom\'s substrate: its harvest over its biological efficiency', () => {
  const D = sg.load();
  near(sg.substrateNeedG(D, 'oyster_mushroom', { grossG: 100 }), 100 / 0.66, 1e-9, 'oyster need');
  near(sg.substrateNeedG(D, 'button_mushroom', { grossG: 100 }), 100 / 0.74, 1e-9, 'button need');
  assert.strictEqual(sg.substrateNeedG(D, 'potato', { grossG: 100 }), 0, 'a green crop needs none');
  // The vitamin D shelf: 15 ug a day at 10 ug/100 g is 150 g of button mushrooms.
  const v = sg.vitaminDShelf(D, 1);
  near(v.freshG, 150, 1e-9, 'vitamin D shelf');
  near(v.substrateG, 150 / 0.74, 1e-9, 'its substrate');
});

const SMALL = ['potato', 'soybean', 'broccoli', 'carrot', 'oyster_mushroom', 'button_mushroom'];

test('the growing medium caps the mushrooms the linear program may choose', () => {
  const D = sg.load();
  D.garden.substrate.residue_usable_share = 0.6;
  const a = sg.analyse(D, 'bare', 1, { onlyIds: SMALL });
  assert.ok(!a.infeasible, 'a garden');
  const need = Object.entries(a.full.area).reduce((s, [id, m2]) => s + a.C[id].substrate_g * m2, 0) + sg.vitaminDShelf(D, 1).substrateG;
  const give = 0.6 * Object.entries(a.full.area).reduce((s, [id, m2]) => s + a.C[id].residue_g * m2, 0);
  assert.ok(need <= give * (1 + 1e-6), `the mushrooms need more than the residue gives (${need.toFixed(1)} g against ${give.toFixed(1)} g)`);
});

test('a garden that cannot carry the vitamin D shelf grows no mushrooms and says so', () => {
  const D = sg.load();
  D.garden.substrate.residue_usable_share = 0;
  const a = sg.analyse(D, 'bare', 1, { onlyIds: SMALL });
  assert.ok(!a.infeasible, 'expected a garden, got infeasible');
  assert.strictEqual(a.vitaminDFallback, true);
  assert.ok(!Object.keys(a.full.area).some((id) => id.endsWith('_mushroom')), 'no mushrooms without substrate');
});

test('the shipped settings: the comfortable garden is the start, the mushroom room is in the minimal garden', () => {
  const D = sg.load();
  assert.strictEqual(D.garden.start_tier, 'comfort', `start tier: expected comfort, got ${D.garden.start_tier}`);
  assert.strictEqual(D.garden.decisions.q1_start_garden, 'The comfortable one.');
  assert.deepStrictEqual(D.garden.tiers.bare.mushroom_room.candidates, ['oyster_mushroom', 'button_mushroom']);
  assert.ok(!D.garden.variants, 'the mushroom room is adopted, not a variant');
  assert.strictEqual(D.garden.layout.bed_machine, 'grow_bed', 'the bed is named for what it does');
  // The crop data fixes (survival-garden.md section 10, approved 2026-11-07).
  assert.strictEqual(Number(D.plants.bean.growth_days), 85, 'dry bean on a dry bean\'s clock');
  assert.strictEqual(Number(D.plants.sweet_potato.area_per_plant_m2), 0.4645, 'sweet potato spacing from NCSU CEFS');
  assert.ok(D.yields.sweet_potato, 'sweet potato cited in yields.ron');
});

test('the shipped gardens: the numbers the design quotes', () => {
  const D = sg.load();
  const lit = (a) => Object.entries(a.plotArea).filter(([id]) => (a.prods[id].floorWeight || 1) >= 1).reduce((s, [, v]) => s + v, 0);
  const comfort = sg.analyse(D, 'comfort', 1);
  near(lit(comfort), 94, 1e-9, 'the comfortable garden, lit canopy m2');
  const bare = sg.analyse(D, 'bare', 1);
  near(lit(bare), 135, 1e-9, 'the bare garden, lit canopy m2');
  assert.deepStrictEqual(Object.keys(bare.plots).sort(), ['broccoli', 'carrot', 'potato', 'soybean'], 'four crops');
  assert.ok(bare.substrate.coverage >= 1, `the residue carries the vitamin D shelf (${bare.substrate.coverage})`);
  assert.ok(comfort.substrate.coverage >= 1, 'and in the comfortable garden');
});

test('the design doc matches the data', () => {
  const out = execFileSync(process.execPath, [SCRIPT, '--check'], { encoding: 'utf8' });
  assert.match(out, /matches the data/);
});
