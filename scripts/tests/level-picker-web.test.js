// The web Maps page's level picker and ship plan (Maps increment 3: web/shared/level-picker.js,
// web/shared/ship-plan.js), the web twins of the app's src/gui/widgets/level_picker.rs and
// src/gui/pages/maps/ship_view.rs, fed the plan the app writes (data/maps/ship_plan.json). Pure
// node, no browser: runs in `just rig-tests`.
//
// What it holds: the picker's rows read top to bottom with you marked and Whole last, the same
// rule as the app's; the plan has the ship's two levels; the upstairs Bedroom is on Storey 2 and
// the Dining Room under it is not; each level's walls in the home are exactly that storey's, and
// a wall rising through both storeys stands on both (the app's `wall_in_storey`).
//
// Seen red 2026-10-22 with the picker's rows left in the order given (bottom up) and
// `wallInStorey` taking every wall whose base is at or under the level's floor: "AssertionError
// [ERR_ASSERTION]: Expected values to be strictly deep-equal:" (the rows [-1, 0, 1, null]) and
// "AssertionError [ERR_ASSERTION]: the upper level's walls are not the upper floor's".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");

const ROOT = path.join(__dirname, "..", "..");
const { pickerRows } = require("../../web/shared/level-picker.js");
const Plan = require("../../web/shared/ship-plan.js");
const plan = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "maps", "ship_plan.json"), "utf8"));

const level = (ordinal) => ({ ordinal, short: ordinal >= 0 ? String(ordinal + 1) : `B${-ordinal}`, name: ordinal === 0 ? "Storey 1 (ground)" : ordinal > 0 ? `Storey ${ordinal + 1}` : `Basement ${-ordinal}` });

test("the picker's rows read top to bottom, you marked, Whole last", () => {
  const rows = pickerRows([level(-1), level(0), level(1)], 0, 1);
  assert.deepStrictEqual(rows.map((r) => r.ordinal), [1, 0, -1, null]);
  assert.strictEqual(rows[0].label, "2");
  assert.ok(rows[0].selected && !rows[3].selected);
  assert.strictEqual(rows[1].label, "1  · you");
  assert.strictEqual(rows[1].tooltip, "Storey 1 (ground) (you are on this level)");
  assert.strictEqual(rows[2].label, "B1");
  const whole = pickerRows([level(0), level(1)], null, null);
  assert.ok(whole[whole.length - 1].selected && whole[whole.length - 1].ordinal === null);
});

test("the plan the app wrote has the ship's two levels and the rooms on each", () => {
  assert.deepStrictEqual(plan.levels.map((l) => [l.ordinal, l.name, l.floor]), [[0, "Storey 1 (ground)", 0], [1, "Storey 2", 3]]);
  const up = Plan.roomsOn(plan, 1).map((r) => r.name);
  const ground = Plan.roomsOn(plan, 0).map((r) => r.name);
  assert.ok(up.includes("Bedroom"), `Storey 2: ${up}`);
  assert.ok(!up.includes("Dining Room"), "Storey 2 holds the Dining Room, which is under it");
  assert.ok(ground.includes("Dining Room") && !ground.includes("Bedroom"));
  assert.ok(plan.rooms.find((r) => r.name === "Bedroom").fill, "the Bedroom has its room type's colour");
});

// Seen red 2026-10-23 with every room named: "corridor_0 is named on the plan".
test("a room with no name in the data is not named", () => {
  for (const id of ["corridor_0", "commons:room_1"]) {
    const r = plan.rooms.find((x) => x.id === id);
    assert.ok(r, `no room ${id} in the plan`);
    assert.ok(!Plan.roomIsNamed(r), `${id} is named on the plan`);
  }
  assert.ok(Plan.roomIsNamed(plan.rooms.find((x) => x.name === "Dining Room")));
});

test("a level's walls are those standing in its storey, body by body", () => {
  const home = plan.bodies.find((b) => b.id === "home");
  const [ox, , oz] = home.origin;
  // The home's segments on each level (walls stacked one over the other share their x and z, so
  // they are compared as sorted lists, not sets).
  const inHome = (s) => [s[0], s[2]].every((x) => x >= ox - 0.01 && x <= ox + home.size[0] + 0.01) && [s[1], s[3]].every((z) => z >= oz - 0.01 && z <= oz + home.size[2] + 0.01);
  const homeAt = (o) => Plan.wallsAt(plan, o).filter(inHome).map((s) => s.join(",")).sort();
  const key = (w) => [ox + w[0], oz + w[1], ox + w[2], oz + w[3]].join(",");
  const upstairs = home.walls.filter((w) => w[4] >= 3 - 0.01);
  const downstairs = home.walls.filter((w) => w[4] < 0.01 && w[5] <= 3.01);
  assert.ok(upstairs.length > 0 && downstairs.length > 0, "the shipped home has walls on both storeys");
  assert.deepStrictEqual(homeAt(1), upstairs.map(key).sort(), "the upper level's walls are not the upper floor's");
  assert.deepStrictEqual(homeAt(0), downstairs.map(key).sort(), "the ground level's walls are not the ground floor's");
  // A wall from the ground to the roof (none in the shipped home) stands on both.
  const two = [[0, 3], [3, 6]];
  assert.strictEqual(Plan.wallInStorey(two, [0, 0, 1, 0, 0, 2.5], 1), false, "a ground wall under the slab is not upstairs");
  assert.strictEqual(Plan.wallInStorey(two, [0, 0, 1, 0, 0, 6], 1), true, "a wall to the roof is upstairs too");
  assert.strictEqual(Plan.wallInStorey(two, [0, 0, 1, 0, 0, 6], 0), true);
  assert.strictEqual(Plan.wallInStorey(two, [0, 0, 1, 0, 3, 4], 0), false, "an upstairs guard is not downstairs");
});
