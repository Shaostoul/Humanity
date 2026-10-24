// The Maps ship check's own tests (Maps increment 3, scripts/lib/map-ship-check.js).
// Run by `just rig-tests`. Each case says what the check must refuse.
//
// Seen red 2026-10-22 against a check that ignored `not_named` and the tiles under an
// edit that were kept: "The input did not match the regular expression /Dining Room is
// named/. Input: 'the level shown is \"Storey 1 (ground)\", want \"Storey 2\"; Bedroom is
// not named; ...'" and "... /0\/1 is under the edit and was kept/. Input: '1 dropped of 3
// kept, 2 under the edit'".

const test = require("node:test");
const assert = require("node:assert");
const { judgeShip, judgeEdit, vantageVerdict } = require("../lib/map-ship-check.js");

const upstairs = {
  ok: true,
  page_open: true,
  ship: {
    level: "Storey 2",
    rooms_named: ["Bedroom", "Study", "Hall", "Wet Room"],
    rooms_filled: 9,
    you_drawn: true,
    plan: false,
    tiles: { have: [], baked: 0, last_bake_ms: 0, dropped: [] },
  },
};
const spec = { level: "Storey 2", named: ["Bedroom"], not_named: ["Dining Room"], you_drawn: true };

test("the upstairs report passes the upstairs spec", () => {
  const c = judgeShip(upstairs, spec);
  assert.ok(c.ok, c.message);
});

test("the ground level fails it: the Bedroom is not there and the Dining Room is", () => {
  const ground = { ...upstairs, ship: { ...upstairs.ship, level: "Storey 1 (ground)", rooms_named: ["Dining Room", "Kitchen"], you_drawn: false } };
  const c = judgeShip(ground, spec);
  assert.ok(!c.ok);
  assert.match(c.message, /Bedroom is not named/);
  assert.match(c.message, /Dining Room is named/);
  assert.ok(vantageVerdict(c, "fail").ok, "a control that fails passes");
  assert.ok(!vantageVerdict(judgeShip(upstairs, spec), "fail").ok, "a control that passes fails");
});

test("no report, or no page, fails", () => {
  assert.ok(!judgeShip(null, spec).ok);
  assert.ok(!judgeShip({ ...upstairs, page_open: false }, spec).ok);
});

test("the plan wants its tiles kept and its bake within budget", () => {
  const rect = (x, y) => [x * 64, y * 64, x * 64 + 64, y * 64 + 64];
  const tile = (x, y) => ({ level: 1, z: 3, x, y, rect: rect(x, y) });
  const plan = { ...upstairs, ship: { ...upstairs.ship, plan: true, tiles: { have: [tile(0, 0), tile(0, 1)], baked: 2, last_bake_ms: 1.4, dropped: [] } } };
  assert.ok(judgeShip(plan, { plan: true, min_tiles: 2, max_bake_ms: 2 }).ok);
  assert.ok(!judgeShip(plan, { plan: true, min_tiles: 3 }).ok, "too few tiles passed");
  assert.ok(!judgeShip(plan, { plan: true, min_tiles: 1, max_bake_ms: 1 }).ok, "a slow bake passed");
});

test("an edit must drop exactly the tiles under it", () => {
  const rect = (x, y) => [x * 64, y * 64, x * 64 + 64, y * 64 + 64];
  const tile = (x, y) => ({ level: 1, z: 3, x, y, rect: rect(x, y) });
  const before = [tile(0, 0), tile(0, 1), tile(1, 0)];
  const footprint = [37, 61, 43, 67]; // crosses z = 64: tiles (0, 0) and (0, 1)
  assert.ok(judgeEdit(before, { tiles: { dropped: [tile(0, 0), tile(0, 1)] } }, footprint).ok);
  assert.match(judgeEdit(before, { tiles: { dropped: [tile(0, 0)] } }, footprint).message, /0\/1 is under the edit and was kept/);
  assert.match(judgeEdit(before, { tiles: { dropped: [tile(0, 0), tile(0, 1), tile(1, 0)] } }, footprint).message, /1\/0 was dropped and is not under the edit/);
  assert.ok(!judgeEdit(before, { tiles: { dropped: [] } }, footprint).ok, "an edit that dropped nothing passed");
});
