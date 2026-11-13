// The space program's reader and arithmetic (scripts/space-program.js, 2026-11-06), and the two
// gates on the shipped data: data/ship/space_program.toml validates, and the apartment block
// template (data/ship/blocks/apartment_atrium.toml) fits its homes in its own ring.
//
// Red evidence, each seen before the code or data was right:
//   - "the shipped block template checks out": with the corner homes' per_storey set to 8 the
//     test failed with "a residential storey's homes need 1108 m2 but the ring leaves 954 m2 after
//     the gallery and the cores".
//   - "a stacked drum space covers its area divided by its levels": with the `/ (s.levels || 1)`
//     division removed from totalFor it failed with "expected 50, got 100" (shops 2 m2 x 50
//     people on 2 levels).
//   - "a net room carries circulation and structure": with `indoor()` returning its input it
//     failed with "expected 154, got 100".
//   - "a unit row rounds up, keeps its minimum and adds its spare": with spare_units ignored it
//     failed with "expected 3 units, got 2".
//   - the reader's multi-line nested array: with newlines refused inside arrays it threw
//     "TOML line 3: unsupported value """ (and the shipped file failed at its line 81).
// The red runs: a scratch script copied the script, the test and data/ship to a scratch folder and
// broke one thing at a time (2026-11-06).
//
// 2026-11-07 (the operator's answers: a sector of 500, one level of farms with two or more allowed,
// the comfortable garden, 1 and 10 billion), each new check seen red the same way first:
//   - "a farm row stacks into [farms] levels": with `s.farm ? farmLevels :` taken out of totalFor
//     it failed with "farm floor on two levels: expected 125, got 250".
//   - "a sector row counts once per 500-person sector": with the 'sector' case still reading
//     units_per_sector it failed with "one sector: expected 10, got NaN".
//   - "a home's gardens must fit its greenhouse": with the check removed from validateProgram the
//     error list was empty and the match on "gardens need 366.6 m2" failed.
//   - "the farms and the homestead grow the comfortable garden": with farm_growing put back to
//     65.29 it failed with "the ship's farms: expected 122.2 (the garden's comfort tier), got 65.29"
//     (and the shipped numbers with it: "Drum 1 holds at most: expected 7781, got 8948").
//   - "hoop stress": with the rim's own weight (rho g R) left out it failed with "wall at 1 km:
//     expected 0.088221, got 0.087162".
//   - "the night-noise distance": with the wall loss added instead of taken off it failed with
//     "enclosed: expected 0.708, got 22,387.211".
//   - "a sector's layouts": with the neighbourhoods laid along the patch's SHORTER side it failed
//     with "half ring node: expected 258.2, got 435.1".
//   - "one ship cannot hold a billion": with the factor 2 dropped from the longest ship it failed
//     with "ships for a billion: expected 204, got 407".
//   - the shipped numbers the master plan quotes (Drum 1 holds 7,781; a sector 0.206 km2; 204 and
//     2,033 ships): seen red by changing the homestead share to 0.09 ("Drum 1 holds at most:
//     expected 7781, got 7343"; "ships for a billion: expected 204, got 214").
// The red runs: the scratch script sg3-red-space.js (2026-11-07), one mutation at a time.
//
// 2026-11-13 (the sizes research pass: the bow's bays sized from defined transport classes, the
// private craft's parking spot from a real craft): "the bow the master plan quotes" seen red with
// the scratch script rs-red-bow.js: with the parking spot put back to the unsourced 150 m2 the
// no-homestead limit came out 14,600 instead of 11,500 and the sector's bow 19,282 m2 instead of
// 20,532; with the Class M, C and H bay rows removed, 13,500 and 9,772.
//
// 2026-11-13 (the shield correction, master plan 1.5 item 8): "the shield the operator decided"
// seen red with the scratch script sh-red-shield.js: with both shield_t_per_m2 put back to the
// old 4.5 it failed with "[drum] shield: expected 10 t/m2, got 4.5"; with only [scale]'s, "[scale]
// and [drum] carry the same shield"; with both at 4.5 and those two pins dropped from a copy,
// "Drum 1's shield: expected about 35.3 million t, got 15.9", and with that line dropped too,
// "mass a person in 1 km drums: expected about 5,219 t, got 2653".

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const path = require('path');
const { execFileSync } = require('child_process');
const sp = require('../space-program.js');

const SCRIPT = path.join(__dirname, '..', 'space-program.js');

test('the reader reads tables, arrays of tables, nested multi-line arrays and comments', () => {
  const doc = sp.parseToml(
    [
      'schema = 1 # a comment',
      'name = "a # not a comment"',
      'segs = [',
      '  [0.0, 150.0, 5.0],   # fore hub',
      '  [150.0, 900.0, 5.0],',
      ']',
      '[defaults]',
      'on = true',
      'big = 1_000',
      '[[row]]',
      'id = "a"',
      '[[row]]',
      'id = "b"',
      'esc = "say \\"hi\\""',
    ].join('\r\n'),
  );
  assert.strictEqual(doc.schema, 1);
  assert.strictEqual(doc.name, 'a # not a comment');
  assert.deepStrictEqual(doc.segs, [
    [0, 150, 5],
    [150, 900, 5],
  ]);
  assert.strictEqual(doc.defaults.on, true);
  assert.strictEqual(doc.defaults.big, 1000);
  assert.deepStrictEqual(
    doc.row.map((r) => r.id),
    ['a', 'b'],
  );
  assert.strictEqual(doc.row[1].esc, 'say "hi"');
});

test('the reader names the line of what it cannot read', () => {
  assert.throws(() => sp.parseToml('a = 1\nb = [1, 2\nc = 3\n'), /TOML line 3/);
  assert.throws(() => sp.parseToml('a = 1\na = 2\n'), /key a set twice/);
  assert.throws(() => sp.parseToml('x = {inline = 1}\n'), /TOML line 1: unsupported value/);
});

test('the shipped space program validates', () => {
  const prog = sp.loadProgram();
  assert.deepStrictEqual(sp.validateProgram(prog), []);
  assert.ok(prog.space.length > 30, `${prog.space.length} spaces`);
});

test('the shipped block template checks out', () => {
  const prog = sp.loadProgram();
  const block = prog.home.find((h) => h.block).block;
  assert.deepStrictEqual(block.problems, []);
  assert.ok(block.homes > 0 && block.people > 0);
});

// A tiny program whose totals can be worked by hand.
function fixture() {
  return {
    defaults: { population: 50, neighbourhood_people: 20, indoor_circulation: 1.4, structure_allowance: 0.1 },
    sector: { residents: 500 },
    drum: { drum_radius_m: 250, drum_length_m: 2000, drum_usable: 1, spine_radius_m: 30, spine_usable: 0.8, spine_segments: [[0, 2000, 5]], shield_t_per_m2: 10 },
    farms: { levels: 1 },
    bow: { radius_m: 60, length_m: 300, deck_spacing_m: 15, usable: 0.7 },
    home: [
      { id: 'plot', share: 0.5, people_per_home: 2.5, plot_m2: 100, plot_gross: 1.5, feeds_itself: true, source: 's' },
      { id: 'room', share: 0.5, people_per_home: 1, unit_m2: 10, shared_m2: 0, storeys: 2, net: true, feeds_itself: false, source: 's' },
    ],
    space: [
      { id: 'shops', zone: 'drum', per: 'person', m2_per_person: 2, levels: 2, net: false, source: 's' },
      { id: 'office', zone: 'spine', per: 'person', m2_per_person: 2, net: true, source: 's' },
      { id: 'farm', zone: 'drum', per: 'person', who: 'fed_by_ship', m2_per_person: 10, levels: 1, net: false, source: 's' },
      { id: 'reactor', zone: 'spine', per: 'unit', unit_m2: 100, people_per_unit: 30, min_units: 1, spare_units: 1, net: false, source: 's' },
      { id: 'node', zone: 'drum', per: 'neighbourhood', unit_m2: 40, levels: 1, net: false, source: 's' },
      { id: 'park', zone: 'drum', per: 'person', m2_per_person: 1, levels: 1, net: false, min_m2_per_neighbourhood: 30, source: 's' },
      { id: 'lifts', zone: 'end_cap', per: 'drum', units_per_drum: 8, unit_m2: 5, net: false, source: 's' },
      { id: 'bridge', zone: 'bow', per: 'ship', unit_m2: 70, net: false, source: 's' },
    ],
    source: [{ id: 's' }],
  };
}

const row = (t, id) => t.rows.find((r) => r.id === id);
const near = (a, b, what) => assert.ok(Math.abs(a - b) < 1e-6, `${what}: expected ${b}, got ${a}`);

test('a stacked drum space covers its area divided by its levels', () => {
  const t = sp.totalFor(fixture(), 50, null);
  near(row(t, 'shops').built_m2, 100, 'shops built');
  near(row(t, 'shops').floor_m2, 50, 'shops floor');
});

test('a net room carries circulation and structure', () => {
  const t = sp.totalFor(fixture(), 50, null);
  // 2 m2 x 50 people x 1.4 x 1.1
  near(row(t, 'office').built_m2, 154, 'office built');
  // 25 single rooms of 10 m2 x 1.4 x 1.1 on two storeys
  const room = t.homes.find((h) => h.id === 'room');
  near(room.built_m2, 385, 'rooms built');
  near(room.floor_m2, 192.5, 'rooms floor');
});

test('a unit row rounds up, keeps its minimum and adds its spare', () => {
  const t = sp.totalFor(fixture(), 50, null);
  assert.strictEqual(row(t, 'reactor').units, 3, `expected 3 units, got ${row(t, 'reactor').units}`);
  const small = sp.totalFor(fixture(), 5, null);
  assert.strictEqual(row(small, 'reactor').units, 2, 'the minimum of one plus the spare');
});

test('only the people their homes do not feed count toward the farms', () => {
  const t = sp.totalFor(fixture(), 50, null);
  near(t.fedByShip, 25, 'fed by ship');
  near(row(t, 'farm').floor_m2, 250, 'farm floor');
});

test('neighbourhood, drum and ship rows count once per their unit', () => {
  const t = sp.totalFor(fixture(), 50, null);
  assert.strictEqual(t.neighbourhoods, 3);
  near(row(t, 'node').floor_m2, 120, 'three nodes');
  near(row(t, 'park').floor_m2, 90, 'the per-neighbourhood minimum beats 50 m2');
  near(row(t, 'lifts').floor_m2, 40, 'eight lifts');
  near(row(t, 'bridge').floor_m2, 70, 'one bridge');
  near(t.zone.bow, 70, 'the bow holds the bridge');
});

test('plots cover their gross area and the zones add up', () => {
  const t = sp.totalFor(fixture(), 50, null);
  const plot = t.homes.find((h) => h.id === 'plot');
  assert.strictEqual(plot.homes, 10);
  near(plot.floor_m2, 1500, 'ten plots at 1.5 x 100 m2');
  near(t.zone.drum, 1500 + 192.5 + 50 + 250 + 120 + 90, 'the drum total');
  near(t.zone.spine, 154 + 300, 'the spine total');
});

test('Drum 1 offers the drum floor the operator sized and the spine decks the plan lays out', () => {
  const prog = sp.loadProgram();
  const room = sp.drumRoom(prog);
  near(Math.round(room.drum), Math.round(2 * Math.PI * 250 * 2000), 'drum floor');
  // 30 + 150 + 60 + 70 + 30 decks
  assert.strictEqual(room.spineDecks, 340);
  assert.strictEqual(room.bowDecks, 20);
});

test('the checks catch a bad mix, an unknown source and an unknown zone', () => {
  const p = fixture();
  p.home[0].share = 0.6;
  p.space[0].source = 'nowhere';
  p.space[1].zone = 'attic';
  const errs = sp.validateProgram(p).join('\n');
  assert.match(errs, /shares sum to 1\.1000/);
  assert.match(errs, /shops: unknown source nowhere/);
  assert.match(errs, /office: zone attic/);
});

test('the block template check catches homes that do not fit and a crowded home', () => {
  const t = sp.parseToml(require('fs').readFileSync(path.join(__dirname, '..', '..', 'data', 'ship', 'blocks', 'apartment_atrium.toml'), 'utf8'));
  t.unit.find((u) => u.id === 'two_bed').per_storey = 8;
  t.unit.find((u) => u.id === 'studio').people = 3;
  const problems = sp.summarizeBlock(t).problems.join('\n');
  assert.match(problems, /homes need 1108 m2 but the ring leaves 954 m2/);
  assert.match(problems, /studio: 3 people in 1 bedroom/);
});

test('a farm row stacks into [farms] levels, and --farm-levels overrides it', () => {
  const p = fixture();
  p.space.find((s) => s.id === 'farm').farm = true;
  near(row(sp.totalFor(p, 50, null), 'farm').floor_m2, 250, 'farm floor on one level');
  p.farms.levels = 2;
  near(row(sp.totalFor(p, 50, null), 'farm').floor_m2, 125, 'farm floor on two levels');
  near(row(sp.totalFor(p, 50, null, { farmLevels: 5 }), 'farm').floor_m2, 50, 'farm floor on five levels');
  // A row that is not a farm keeps its own levels.
  near(row(sp.totalFor(p, 50, null, { farmLevels: 5 }), 'shops').floor_m2, 50, 'shops keep their two levels');
});

test('a sector row counts once per 500-person sector', () => {
  const p = fixture();
  p.space.push({ id: 'hall', zone: 'drum', per: 'sector', unit_m2: 10, levels: 1, net: false, source: 's' });
  near(row(sp.totalFor(p, 50, null), 'hall').floor_m2, 10, 'one sector');
  near(row(sp.totalFor(p, 1200, null), 'hall').floor_m2, 30, 'three sectors');
  assert.strictEqual(sp.totalFor(p, 1200, null).sectors, 3);
});

test('a home that feeds itself must have room for its household\'s gardens', () => {
  const p = fixture();
  Object.assign(p.home[0], { people_per_home: 3, garden_m2_per_person: 122.2, greenhouse_m2: 300 });
  assert.match(sp.validateProgram(p).join('\n'), /plot: 3 residents' gardens need 366.6 m2 but its greenhouse is 300 m2/);
  p.home[0].greenhouse_m2 = 990;
  assert.deepStrictEqual(sp.validateProgram(p), []);
});

test('the ship\'s farms and the homestead grow the comfortable garden the operator chose', () => {
  // One number in two files: the garden program's comfort tier (lit canopy x the path factor) is
  // what the space program's farm row and the homestead's garden say.
  const sg = require('../survival-garden.js');
  const D = sg.load();
  const a = sg.analyse(D, 'comfort', 1);
  const lit = Object.entries(a.plotArea).filter(([id]) => (a.prods[id].floorWeight || 1) >= 1).reduce((s, [, v]) => s + v, 0);
  const want = Math.round(lit * D.garden.layout.path_factor * 10) / 10;
  const prog = sp.loadProgram();
  const farm = prog.space.find((s) => s.id === 'farm_growing');
  assert.ok(Math.abs(farm.m2_per_person - want) < 0.05, `the ship's farms: expected ${want} (the garden's comfort tier), got ${farm.m2_per_person}`);
  const home = prog.home.find((h) => h.id === 'homestead');
  assert.ok(Math.abs(home.garden_m2_per_person - want) < 0.05, `the homestead's garden: expected ${want}, got ${home.garden_m2_per_person}`);
  assert.strictEqual(prog.farms.levels, 1, 'one level of farms to start (the operator, 2026-11-07)');
});

test('hoop stress: the rim carries its own weight and what presses on it', () => {
  const sc = { safety_factor: 2, air_kpa: 101.325, contents_t_per_m2: 1 };
  const carbon = { id: 'c', strength_mpa: 2550, density_kg_m3: 1560 };
  const h = sp.hoop(carbon, 1000, sc);
  // 1,275 MPa working; q = 101,325 + 9,806.65 Pa; t = q R / (sigma - rho g R)
  const want = (111131.65 * 1000) / (1.275e9 - 1560 * 9.80665 * 1000);
  assert.ok(Math.abs(h.thickness - want) < 1e-6, `wall at 1 km: expected ${want.toFixed(6)}, got ${h.thickness.toFixed(6)}`);
  near(h.maxRadius, 1.275e9 / (1560 * 9.80665), 'the widest empty drum');
  // Past the widest drum no wall is thick enough.
  assert.strictEqual(sp.hoop(carbon, 90000, sc).thickness, Infinity);
});

test('the night-noise distance follows the master plan\'s estimate', () => {
  const noise = { night_facade_db: 40, hard_floor_db: 8 };
  // 3.13's table: a 100 dB(A) machine in the open needs 398 m, in a room with 45 dB walls 2.2 m.
  assert.ok(Math.abs(sp.noiseDistance(100, 0, noise) - 398.1) < 0.1, `open: ${sp.noiseDistance(100, 0, noise)}`);
  const enclosed = sp.noiseDistance(90, 45, noise);
  assert.ok(Math.abs(enclosed - 0.708) < 0.001, `enclosed: expected 0.708, got ${enclosed.toLocaleString('en-US')}`);
});

test('a sector\'s layouts on a drum: ring, half ring, quarter ring, and their walks', () => {
  const sector = { residents: 500, layers: [['neighbourhood', 167]], walk_m_per_s: 1.4, node_walk_max_s: 300, daily_walk_max_s: 900 };
  const [ring, half, quarter] = sp.sectorLayouts(200000, 250, sector);
  const circ = 2 * Math.PI * 250;
  near(half.around, circ / 2, 'half ring around');
  near(half.along, 200000 / (circ / 2), 'half ring along');
  // Three neighbourhoods side by side along the 785 m side: each node 127.3 + 130.9 m from its corner.
  assert.ok(Math.abs(half.nodeWalk - 258.2) < 0.1, `half ring node: expected 258.2, got ${half.nodeWalk.toFixed(1)}`);
  assert.ok(Math.abs(half.hallWalk - 520.0) < 0.1, `half ring hall: expected 520.0, got ${half.hallWalk.toFixed(1)}`);
  assert.ok(ring.hallWalk > half.hallWalk && half.hallWalk > quarter.hallWalk, 'the whole ring walks farthest to its hall');
  assert.ok([ring, half, quarter].every((l) => l.walksOk));
});

test('one ship cannot hold a billion; the fleet that can', () => {
  const prog = sp.loadProgram();
  const s = sp.scaleStudy(prog, null);
  const bulk = s.classes.find((c) => c.id === 'r1000');
  const [b1, b10] = bulk.fleets;
  assert.ok(b1.oneShipSpineShare > 1, `one ship's spine would weigh ${b1.oneShipSpineShare} times its load`);
  assert.strictEqual(b1.ships, Math.ceil(b1.drums / bulk.drumsPerShip));
  assert.strictEqual(b1.ships, 204, `ships for a billion: expected 204, got ${b1.ships}`);
  assert.strictEqual(b10.ships, 2033, `ships for ten billion: expected 2033, got ${b10.ships}`);
  assert.ok(bulk.shipLength <= s.maxShipLength, 'a ship stays within the spine\'s length');
  assert.ok(bulk.length <= 1.3 * bulk.radius + 1e-9, 'every drum is inside the stability limit');
  assert.ok(bulk.buckleLength < bulk.shipLength, 'a spine pushed from the stern would buckle');
});

test('the shipped numbers the master plan quotes', () => {
  const prog = sp.loadProgram();
  const full = sp.maxPopulation(prog, null);
  assert.strictEqual(full, 7781, `Drum 1 holds at most: expected 7781, got ${full}`);
  const s = sp.sectorStudy(prog, null);
  assert.ok(Math.abs(s.loneFloor - 206000) < 1000, `a sector's drum floor: ${s.loneFloor}`);
  assert.strictEqual(s.sectorsInDrum1, 15);
  assert.ok(s.halfRingLength <= s.stableLength, `two half-ring sectors make a drum of ${s.halfRingLength} m, inside ${s.stableLength}`);
  assert.ok(s.best.walksOk, 'some layout meets both walking rules');
  assert.strictEqual(sp.maxPopulation(prog, null, { farmLevels: 2 }), 9031, 'two levels of farms');
});

// The operator chose 5 mSv a year inside, about 10 t/m2 of water (master plan 1.5, item 8,
// 2026-11-13; the sizes research, section 7, corrected the old 4.5 t/m2). The masses below are
// the ones the master plan (section 0 and 3.16) and closed-loop-test.md quote; the ship counts do
// not depend on the shield (the spine's longest length does not depend on what it carries).
test('the shield the operator decided, and the masses the master plan quotes from it', () => {
  const prog = sp.loadProgram();
  assert.strictEqual(prog.drum.shield_t_per_m2, 10, `[drum] shield: expected 10 t/m2, got ${prog.drum.shield_t_per_m2}`);
  assert.strictEqual(prog.scale.shield_t_per_m2, prog.drum.shield_t_per_m2, '[scale] and [drum] carry the same shield');
  const drum1Shield = sp.drumRoom(prog).shield_t / 1e6;
  assert.ok(Math.abs(drum1Shield - 35.3) < 0.1, `Drum 1's shield: expected about 35.3 million t, got ${drum1Shield.toFixed(1)}`);
  const s = sp.scaleStudy(prog, null);
  const bulk = s.classes.find((c) => c.id === 'r1000');
  const perPerson = bulk.fleets[0].perPersonT;
  assert.ok(Math.abs(perPerson - 5219) < 5, `mass a person in 1 km drums: expected about 5,219 t, got ${Math.round(perPerson)}`);
  assert.ok(Math.abs(bulk.fleets[0].fleetKg / 1e15 - 5.22) < 0.01, `a billion's fleet: expected 5.22 x 10^12 t, got ${(bulk.fleets[0].fleetKg / 1e15).toFixed(2)}`);
  assert.strictEqual(bulk.fleets[0].ships, 204, 'the shield does not change the ship count');
});

test('the bow the master plan quotes: a bay for every transport class, a parking spot that fits its craft', () => {
  const prog = sp.loadProgram();
  for (const id of ['hangar_bays', 'ferry_bays', 'freighter_bays', 'heavy_bays']) {
    assert.ok(prog.space.some((s) => s.id === id && s.zone === 'bow'), `the bow has its ${id}`);
  }
  const noHomesteads = sp.maxPopulation(prog, { homestead: 0, cabin: 0.22, apartment: 0.78 });
  assert.strictEqual(noHomesteads, 11500, `Drum 1 with no homesteads (the bow full): expected 11500, got ${noHomesteads}`);
  const bow = Math.round(sp.totalFor(prog, 500, null, {}).zone.bow);
  assert.ok(Math.abs(bow - 20532) < 500, `one sector's bow decks: expected about 20,532 m2, got ${bow}`);
});

test('the checks catch a scale study naming a material that is not there', () => {
  const p = fixture();
  p.material = [{ id: 'steel', strength_mpa: 700, density_kg_m3: 7850, source: 's' }];
  p.scale = { spine_material: 'unobtainium', hull_material: 'steel' };
  assert.match(sp.validateProgram(p).join('\n'), /spine_material unobtainium is not a \[\[material\]\]/);
});

test('the command line answers and its exit code says whether Drum 1 holds the population', () => {
  const sector = execFileSync(process.execPath, [SCRIPT, '--sector'], { encoding: 'utf8' });
  assert.match(sector, /The sector: 500 residents/);
  assert.match(sector, /a half ring .* meets both walking rules/);
  const scale = execFileSync(process.execPath, [SCRIPT, '--scale'], { encoding: 'utf8' });
  assert.match(scale, /a FLEET of 204 ships/);
  const out = execFileSync(process.execPath, [SCRIPT, '100'], { encoding: 'utf8' });
  assert.match(out, /Space program for 100 people/);
  assert.match(out, /Drum 1 HOLDS 100 people/);
  let code = 0;
  try {
    execFileSync(process.execPath, [SCRIPT, '100000000'], { encoding: 'utf8' });
  } catch (e) {
    code = e.status;
  }
  assert.strictEqual(code, 1, 'a hundred million people do not fit one drum');
  const block = execFileSync(process.execPath, [SCRIPT, '--block'], { encoding: 'utf8' });
  assert.match(block, /checks: all pass/);
});
