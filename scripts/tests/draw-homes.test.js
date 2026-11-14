// The home drawings come from the data (scripts/draw-homes.js, 2026-11-13,
// docs/design/home-room-program.md section 7): the RON reader reads every shipped home design the
// way the game does (zones, walls, slabs), the street is at the top of every drawing whichever wall
// the door is in, and every room's label and size in a drawing is the file's. Pure node, no game:
// runs in `just rig-tests`. The layout CHECKS are Rust tests over the same files
// (crates/humanity-ship/src/home_program_tests.rs).
//
// RED FIRST (2026-11-13): with `frame` mapping a west door to the drawing's left edge instead of
// its top, "the street is at the top of every drawing" failed with 'cabin: the door is drawn at v
// 1.25, not 0'; with the room table reading each zone's label but summing only the
// primary box, "a room drawn as several boxes is sized as one" failed with 'Greenhouse: 544 m2
// drawn, 616 in the file'.

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const D = require('../draw-homes.js');

const SHIPPED = path.resolve(__dirname, '..', '..', 'data', 'homes', 'shipped');
const designs = fs
  .readdirSync(SHIPPED)
  .filter((n) => n.endsWith('.ron'))
  .map((n) => D.parseRon(fs.readFileSync(path.join(SHIPPED, n), 'utf8')));

test('every shipped home design reads as the game reads it', () => {
  const kinds = designs.map((d) => d.kind).sort();
  for (const k of ['cabin', 'homestead', 'household_homestead', 'one_person_homestead']) assert.ok(kinds.includes(k), `${k} is read`);
  for (const d of designs) {
    assert.ok(d.body.width > 0 && d.body.depth > 0, `${d.kind} has a body`);
    assert.ok(Array.isArray(d.body.zones) && d.body.zones.length > 0, `${d.kind} has zones`);
    for (const w of d.body.walls) assert.ok(Array.isArray(w.a) && w.a.length === 2, `${d.kind}: a wall's corner is an (x, z) pair`);
  }
  const hh = designs.find((d) => d.kind === 'household_homestead');
  assert.deepStrictEqual(D.storeys(hh.body), [{ floor: 0, top: 3 }, { floor: 3, top: 6 }], 'the household homestead has two storeys');
});

test('the street is at the top of every drawing', () => {
  for (const d of designs) {
    const f = D.frame(d);
    const [, v] = f.uv(d.door[0], d.door[1]);
    assert.ok(Math.abs(v) < 1e-6, `${d.kind}: the door is drawn at v ${v}, not 0`);
  }
});

test('a room drawn as several boxes is sized as one', () => {
  const hh = designs.find((d) => d.kind === 'household_homestead');
  const gh = D.roomTable(hh).find((r) => r.id === 'room-greenhouse');
  assert.strictEqual(gh.label, 'Greenhouse');
  assert.strictEqual(Math.round(gh.area), 616, `Greenhouse: ${gh.area} m2 drawn, 616 in the file`);
});

test("every room's label and size in a drawing is the file's", () => {
  const one = JSON.parse(JSON.stringify(designs.find((d) => d.kind === 'one_person_homestead')));
  one.body.zones.find((z) => z.id === 'room-bedroom').label = 'A label only this test writes';
  const svg = D.drawStorey(one, 0, {});
  assert.ok(svg.includes('A label only this test writes'), 'the drawing shows the label in the data');
  assert.ok(svg.includes('3.5 x 3.5 m, 12.25 m2'), "the bedroom's size is the zone's");
});
