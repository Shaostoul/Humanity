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
    sector: { drum_radius_m: 250, drum_length_m: 2000, drum_usable: 1, spine_radius_m: 30, spine_usable: 0.8, spine_segments: [[0, 2000, 5]], shield_t_per_m2: 4.5 },
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
      { id: 'lifts', zone: 'end_cap', per: 'sector', units_per_sector: 8, unit_m2: 5, net: false, source: 's' },
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

test('neighbourhood, sector and ship rows count once per their unit', () => {
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

test('Sector 1 offers the drum floor the operator sized and the spine decks the plan lays out', () => {
  const prog = sp.loadProgram();
  const room = sp.sectorRoom(prog);
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

test('the command line answers and its exit code says whether Sector 1 holds the population', () => {
  const out = execFileSync(process.execPath, [SCRIPT, '100'], { encoding: 'utf8' });
  assert.match(out, /Space program for 100 people/);
  assert.match(out, /Sector 1 HOLDS 100 people/);
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
