// The falls judge (scripts/lib/falls-judge.js): what `just verify-falls` passes and fails.
//
// Run: node --test scripts/tests/falls-judge.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted. A made-up run that went right passes every check; then one broken
// thing at a time, each failing its own checks. The made-up run is built from the tree's own data
// (falls.ron's curve, game.csv's gravity, the homestead's stairs, ladder, floor slabs and rooms, and
// the deck the rig places from them), so a change to the data moves the run with it, and the
// readers are tested on the real files too. Last, the same runs written to a
// folder are judged by the rig itself, `node scripts/verify-falls.js --dry-verdict`, as a saved run
// would be.
//
// THE SHAPE THAT MATTERS MOST is `handOffRemoved`: what the running game reports when the walk's
// landing never reaches the body (lib.rs's `take_landing` -> `falls::player_landed` hand-off taken
// out, or a FallWatch that swallows every landing). The camera still drops 3 m, the health and the
// wounds do not move, and the landing count stays where it was. Every unit test of falls stays
// green in that build; this judge must not.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const FJ = require("../lib/falls-judge.js");

const REPO = path.join(__dirname, "..", "..");
const DATA = FJ.readTreeData(REPO);
const G = DATA.gravity_m_s2;
const ST = DATA.stairs;
const EYE = 1.7;
/** The rig's deck aboard (ship homes increment 7): placed in the room open to the roof. */
const DK = DATA.deck;
const V3 = Math.sqrt(2 * G * DK.top_m); // the walk's landing off the deck's edge
const SPRAIN = DATA.falls.steps[0].effect;
const HARM3 = FJ.expectFall(DATA.falls, V3, "realistic").harm;
const LD = DATA.ladder;
/** The step from the top rung's hold down onto the upper floor, and its landing (which cannot hurt). */
const STEP_OFF_M = LD.hang_eye_m - LD.hatch.floor_eye_m;
const V_STEP = Math.sqrt(2 * G * STEP_OFF_M);
/** A fall from the ladder's top down the hatch's hole to the floor below. */
const V_HOLE = Math.sqrt(2 * G * (LD.hang_eye_m - EYE));
/** What the console answers the rig's line (gui/pages/construction.rs `add_structure`). */
const DECK_SAID = `added structure #8 (deck) at (${DK.pos.join(",")}) yaw 0`;

// ── The made-up run ─────────────────────────────────────────────────────────────────────────────

/** A camera track at 60 frames a second on the rig's deck: standing on it at its stand point,
 *  walking at 5 m/s off its south edge, falling under G and standing where it landed; or,
 *  `placed`, standing on the floor beside it, then put at the deck's height and falling; or,
 *  `noDeck`, the deck never placed, so the placement onto it fell before the recording began and
 *  the walk goes along the floor. `health(t, landedT)` gives each frame's health. */
function track({ placed = false, noDeck = false, health = () => 100 } = {}) {
  const dt = 1 / 60;
  const frames = [];
  let t = 0;
  let [x, y, z] = placed ? [DK.placed_eye[0], EYE, DK.placed_eye[2]] : DK.stand.slice();
  if (noDeck) y = EYE;
  const push = () => {
    frames.push({ t, cam: [x, y, z, DK.yaw, 0], health: null });
    t += dt;
  };
  for (let i = 0; i < 18; i++) push();
  const top = DK.top_m + EYE;
  if (placed) {
    y = top;
  } else {
    // Along the deck to its edge (the walk is south, +z).
    for (let i = 0; i < Math.ceil(DK.edge_m / (5 * dt)); i++) {
      z += 5 * dt;
      push();
    }
  }
  // The fall: v = g t from rest at the top (a walk off the edge has no vertical speed).
  let v = 0;
  while (y > EYE) {
    push();
    v += G * dt;
    y = Math.max(EYE, y - v * dt);
    if (!placed) z += 5 * dt;
  }
  const landedT = t;
  for (let i = 0; i < 180; i++) push();
  for (const f of frames) f.health = health(f.t, landedT);
  return frames;
}

/** The ladder through the hatch at 60 frames a second: standing in the hole at its foot facing it,
 *  climbing at 3 m/s to the top rung's hold, then walking at 1.5 m/s off the top the way the floor
 *  is, holding on (at the hold's height) while in the ladder's reach and stepping down onto the
 *  floor past it. `gateShut`: the gate stops the body short of the ladder's line, so it hangs at
 *  the top over the hole. `downHole`: it walks off the top the other way, over the hole, and falls
 *  to the floor below. Returns { frames, impact } (the landing's speed, null if none). */
function ladderTrack({ gateShut = false, downHole = false, health = () => 100 } = {}) {
  const H = LD.hatch;
  const dt = 1 / 60;
  const frames = [];
  let t = 0;
  let [x, y, z] = H.stand.slice();
  const push = () => {
    frames.push({ t, cam: [x, y, z, H.yaw, 0], health: null });
    t += dt;
  };
  for (let i = 0; i < 18; i++) push();
  while (y < LD.hang_eye_m) {
    y = Math.min(LD.hang_eye_m, y + 3 * dt);
    push();
  }
  for (let i = 0; i < 18; i++) push();
  const sign = downHole ? -1 : 1;
  const along = () => (x - LD.pos[0]) * H.dir[0] + (z - LD.pos[2]) * H.dir[1];
  const reach = () => Math.hypot(x - LD.pos[0], z - LD.pos[2]) <= LD.reach_m;
  const floorEye = downHole ? EYE : H.floor_eye_m;
  let v = 0;
  let landedT = Infinity;
  let impact = null;
  for (let i = 0; i < 240; i++) {
    const nx = x + sign * H.dir[0] * 1.5 * dt;
    const nz = z + sign * H.dir[1] * 1.5 * dt;
    const blocked = gateShut && (nx - LD.pos[0]) * H.dir[0] + (nz - LD.pos[2]) * H.dir[1] > -(0.08 + 0.3);
    if (!blocked && Math.abs(sign * along()) < H.step_m) [x, z] = [nx, nz];
    if (!reach() && y > floorEye) {
      v += G * dt;
      y = Math.max(floorEye, y - v * dt);
      if (y === floorEye && impact === null) {
        impact = Math.sqrt(2 * G * (LD.hang_eye_m - floorEye));
        landedT = t;
      }
    }
    push();
  }
  for (const f of frames) f.health = health(f.t, landedT);
  return { frames, impact };
}

const fallsBlock = (over = {}) => ({ landings: 0, last_m_s: null, mode: "forgiving", gravity_m_s2: G, flying: false, aboard: true, ...over });
const sprain = () => ({ id: SPRAIN, name: DATA.effects[SPRAIN].name, remaining_s: 1209600 });
const vitals = (health, wounds = []) => ({ satiation: 80, hydration: 80, health, health_max: 100, wounds, registry: true });
const snap = (falls, v) => ({ falls, vitals: v, home_box: { min: [0, 0, 0], max: [55, 6, 89] }, frames: [{ t: 0, cam: [...DK.stand, DK.yaw, 0], health: v.health }] });

/** A deck walk leg (kind "deck") as the rig records it. */
const deckWalk = (id, mode, before, after) => ({ id, mode, kind: "deck", start: DK.stand.slice(), deck_m: DK.top_m, walk_m_s: 5, hold_s: (DK.edge_m + DK.walk_on_m) / 5, before, after });

/** A run that went right: nothing landed getting into the world; the console placed the deck; the
 *  walks off the deck land at V3, Forgiving hurts nobody, Realistic sprains and takes HARM3 two
 *  frames after the landing; the climb through the hatch ends on the upper floor, its only landing
 *  the step off the top rung (V_STEP), nothing hurt; and the placement's drop reaches no body. */
function goodRun() {
  const after = 100 - HARM3;
  const landedHurt = (t, landedT) => (t >= landedT + 2 / 60 ? after : 100);
  const climb = ladderTrack({ health: () => after });
  return {
    data: DATA,
    origin: [0, 0, 0],
    deck_console: { line: DK.console, said: DECK_SAID },
    entry: snap(fallsBlock(), vitals(100)),
    legs: [
      deckWalk("forgiving_walk", "forgiving", snap(fallsBlock(), vitals(100)), { falls: fallsBlock({ landings: 1, last_m_s: V3 }), vitals: vitals(100), frames: track() }),
      deckWalk(
        "realistic_walk",
        "realistic",
        snap(fallsBlock({ landings: 1, last_m_s: V3, mode: "realistic" }), vitals(100)),
        { falls: fallsBlock({ landings: 2, last_m_s: V3, mode: "realistic" }), vitals: vitals(after, [sprain()]), frames: track({ health: landedHurt }) }
      ),
      {
        id: "realistic_ladder",
        mode: "realistic",
        kind: "ladder_hatch",
        start: LD.hatch.stand.slice(),
        climb_s: 1.8,
        before: snap(fallsBlock({ landings: 2, last_m_s: V3, mode: "realistic" }), vitals(after, [sprain()])),
        after: { falls: fallsBlock({ landings: 3, last_m_s: climb.impact, mode: "realistic" }), vitals: vitals(after, [sprain()]), frames: climb.frames },
      },
      {
        id: "placed_drop",
        mode: "realistic",
        kind: "placement",
        start: DK.placed_eye.slice(),
        before: snap(fallsBlock({ landings: 3, last_m_s: climb.impact, mode: "realistic" }), vitals(after, [sprain()])),
        after: {
          falls: fallsBlock({ landings: 3, last_m_s: climb.impact, mode: "realistic" }),
          vitals: vitals(after, [sprain()]),
          frames: track({ placed: true, health: () => after }),
        },
      },
    ],
    panics: 0,
    builtin_data: [],
  };
}
const clone = (x) => JSON.parse(JSON.stringify(x));
const leg = (run, id) => run.legs.find((l) => l.id === id);
const failed = (run) =>
  FJ.judgeFalls(run)
    .checks.filter((c) => !c.ok)
    .map((c) => c.id)
    .sort();

/** What the game reports with the lib.rs hand-off taken out: every landing stays in the watch. */
function handOffRemoved() {
  const run = goodRun();
  for (const l of run.legs) {
    for (const r of [l.before, l.after]) {
      r.falls.landings = 0;
      r.falls.last_m_s = null;
      r.vitals = vitals(100);
    }
    for (const f of l.after.frames) f.health = 100;
  }
  return run;
}

// ── The readers, on the tree's own files ────────────────────────────────────────────────────────

test("the readers read the tree: falls.ron's curve, game.csv's gravity, the effects' names, the homestead's stairs, ladder and floors", () => {
  assert.strictEqual(DATA.falls.speed_share, 0.7);
  assert.deepStrictEqual(DATA.falls.harm[0], { m_s: 6.26, health: 2 });
  assert.deepStrictEqual(
    DATA.falls.steps.map((s) => s.effect),
    ["sprained_ankle", "broken_ankle", "broken_leg"]
  );
  assert.strictEqual(DATA.falls.landed, "You land hard.");
  assert.strictEqual(G, 9.81);
  assert.strictEqual(DATA.effects.sprained_ankle.name, "Sprained Ankle");
  assert.strictEqual(DATA.effects.sprained_ankle.kind, "wound");
  // The tallest staircase aboard, read from the tree (its place, turn and risers are the data's,
  // so the stairs can move again without this test moving with them): stairs-1, its risers the
  // type's, a storey tall, up to the upper floor.
  const types = FJ.readStructureTypes(fs.readFileSync(path.join(REPO, "data", "blueprints", "structure_types.ron"), "utf8"));
  assert.strictEqual(ST.id, "stairs-1");
  assert.strictEqual(ST.steps, types[ST.type_id].steps);
  assert.ok(ST.steps > 1 && Math.abs(ST.tread_m - ST.size[2] / ST.steps) < 1e-12);
  assert.ok(DATA.floors.some((s) => Math.abs(s.top - ST.top_m) < 1e-9), "the stairs climb to a floor of the home");
  // Facing up them: the yaw looks along the way the steps rise (the piece's local +z turned by its
  // rot_deg, (sin r, cos r)), and the walk starts off the stairs, a metre before the first step.
  const r = (ST.rot_deg * Math.PI) / 180;
  assert.ok(Math.abs(Math.sin(ST.yaw) - Math.sin(r)) < 1e-9 && Math.abs(-Math.cos(ST.yaw) - Math.cos(r)) < 1e-9, `yaw ${ST.yaw} for rot_deg ${ST.rot_deg}`);
  const along = (p) => (p[0] - ST.pos[0]) * Math.sin(r) + (p[2] - ST.pos[2]) * Math.cos(r);
  assert.ok(Math.abs(along(ST.foot) - (-ST.size[2] / 2 - 1)) < 1e-9, "the walk starts a metre before the first step");
  assert.ok(Math.abs(along(ST.top_tread_eye) - (ST.size[2] / 2 - ST.tread_m / 2)) < 1e-9, "the walk down starts on the top tread");
  assert.ok(Math.abs(Math.sin(ST.down_yaw) + Math.sin(r)) < 1e-9 && Math.abs(-Math.cos(ST.down_yaw) + Math.cos(r)) < 1e-9, "facing down the stairs");
  assert.strictEqual(ST.placed_eye[1], ST.top_m + EYE);
  // The ladder climbs through a hatch onto that floor: its climber stands in the hole (under no
  // floor at the ladder's top), facing the way off the top, which ends on the floor out of its reach.
  assert.ok(LD.hatch, "the homestead's ladder has a floor at its top");
  const H = LD.hatch;
  assert.ok(!FJ.floorAt(DATA.floors, LD.top_m, H.stand[0], H.stand[2]), "the climber stands under the hatch's hole");
  assert.ok(FJ.floorAt(DATA.floors, LD.top_m, H.step_to[0], H.step_to[2]), "the walk off the top ends on the upper floor");
  assert.ok(Math.hypot(H.step_to[0] - LD.pos[0], H.step_to[2] - LD.pos[2]) > LD.reach_m, "and out of the ladder's reach");
  assert.ok(Math.abs(Math.sin(H.yaw) - H.dir[0]) < 1e-9 && Math.abs(-Math.cos(H.yaw) - H.dir[1]) < 1e-9, "facing the way off the top");
  assert.ok(Math.abs(STEP_OFF_M - 0.2) < 1e-9, "the top rung's hold is 0.2 m over the floor");
});

test("the deck the rig places aboard: the upper floor's height, in the room open to the roof, clear of walls", () => {
  assert.ok(!DK.why, DK.why);
  assert.strictEqual(DK.type_id, "deck");
  assert.strictEqual(DK.top_m, Math.max(...DATA.floors.map((s) => s.top)), "its top is the upper floor's height");
  assert.ok(Math.abs(DK.pos[1] + DK.size[1] - DK.top_m) < 1e-9, "placed so its top is there");
  assert.strictEqual(DK.console, `add_structure deck ${DK.pos[0]} ${DK.pos[1]} ${DK.pos[2]} 0`);
  // No slab over the deck, its walk off the edge, or the placement beside it.
  for (const p of [DK.pos, DK.stand, [DK.stand[0], 0, DK.stand[2] + DK.edge_m + DK.walk_on_m], DK.placed_eye]) {
    assert.ok(!DATA.floors.some((s) => FJ.floorAt([s], s.top, p[0], p[2])), `a floor slab over ${p}`);
  }
  assert.strictEqual(DK.room, "room-greenhouse", "the homestead's one room open to the roof");
  // The stand point is on the deck, the edge_m from its south edge; the placement off it.
  assert.ok(Math.abs(DK.pos[2] + DK.size[2] / 2 - DK.stand[2] - DK.edge_m) < 1e-9);
  assert.ok(DK.placed_eye[2] < DK.pos[2] - DK.size[2] / 2, "the placement is off the deck");
  // Where it cannot go, it says why.
  const types = { deck: { kind: "Deck", shape: "Slab", size: [4, 0.2, 4] } };
  const room = [{ id: "room-a", origin: [0, 0, 0], size: [20, 3, 20] }];
  const slab = [{ a: [30, 0], b: [40, 10], top: 3 }];
  assert.match(FJ.pickAboardDeck({}, room, slab, [], 6).why, /no Deck/);
  assert.match(FJ.pickAboardDeck(types, room, [], [], 6).why, /no upper floor/);
  assert.match(FJ.pickAboardDeck(types, room, slab, [], 4).why, /no room for an eye/);
  assert.match(FJ.pickAboardDeck(types, room, [{ a: [0, 0], b: [40, 40], top: 3 }], [], 6).why, /open to the roof/);
  assert.match(FJ.pickAboardDeck(types, room, slab, [{ a: [0, 11], b: [20, 11], base: 0, height: 3 }], 6).why, /wall stands across/);
  // A wall at the deck's own level stops the walk on it too; a beam over the eye on the deck does not.
  assert.match(FJ.pickAboardDeck(types, room, slab, [{ a: [0, 11], b: [20, 11], base: 3, height: 3 }], 6).why, /wall stands across/);
  assert.ok(!FJ.pickAboardDeck(types, room, slab, [{ a: [0, 11], b: [20, 11], base: 5, height: 1 }], 6).why, "a beam over the head of a body on the deck");
});

test("the judge's own reading of falls.ron gives what falls.ron's comments say", () => {
  // A 3 m drop on the walk's gravity: 7.67 m/s, a sprain and 9.7 health in Realistic, nothing in
  // Forgiving (read at 70%: 5.37, under the first harm at 6.26).
  assert.ok(Math.abs(V3 - 7.672) < 0.001, `${V3}`);
  const real = FJ.expectFall(DATA.falls, V3, "realistic");
  const soft = FJ.expectFall(DATA.falls, V3, "forgiving");
  assert.strictEqual(real.wound, "sprained_ankle");
  assert.ok(Math.abs(real.harm - 9.685) < 0.01, `${real.harm}`);
  assert.deepStrictEqual([soft.wound, soft.harm], [null, 0]);
  assert.ok(Math.abs(soft.read - 5.370) < 0.001);
  // The file's points exactly, and its "six storeys (18.94 m/s) takes about 117".
  for (const p of DATA.falls.harm) assert.ok(Math.abs(FJ.harmAt(DATA.falls, p.m_s) - p.health) < 1e-9, `${p.m_s}`);
  assert.strictEqual(Math.round(FJ.harmAt(DATA.falls, 18.94)), 117);
  assert.strictEqual(FJ.harmAt(DATA.falls, 6.25), 0);
  assert.strictEqual(FJ.woundAt(DATA.falls, 7.73), "broken_ankle");
  assert.strictEqual(FJ.woundAt(DATA.falls, 10.93), "broken_leg");
  // The step off the ladder's top cannot hurt; a fall down the hatch from there can.
  assert.deepStrictEqual(FJ.expectFall(DATA.falls, V_STEP, "realistic"), { read: V_STEP, harm: 0, wound: null });
  assert.strictEqual(FJ.expectFall(DATA.falls, V_HOLE, "realistic").wound, "broken_ankle");
});

// ── The judgement ────────────────────────────────────────────────────────────────────────────────

test("a run that went right passes every check, the landing-time check included", () => {
  const r = FJ.judgeFalls(goodRun());
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  const ids = r.checks.map((c) => c.id);
  for (const want of [
    "deck_placed",
    "forgiving_walk.stood_on_deck",
    "forgiving_walk.dropped",
    "forgiving_walk.landing_handed",
    "forgiving_walk.landing_speed",
    "forgiving_walk.wound",
    "forgiving_walk.health",
    "realistic_walk.stood_on_deck",
    "realistic_walk.wound",
    "realistic_walk.wound_named",
    "realistic_walk.health",
    "realistic_walk.harm_at_landing",
    "realistic_ladder.climbed",
    "realistic_ladder.on_upper_floor",
    "realistic_ladder.no_fall",
    "realistic_ladder.no_wound",
    "realistic_ladder.health",
    "world_entry",
    "placed_drop.dropped",
    "placed_drop.no_landing",
    "placed_drop.no_wound",
    "placed_drop.health",
    "panics",
    "no_builtin_data",
  ]) {
    assert.ok(ids.includes(want), `${want} was never judged`);
  }
  assert.ok(FJ.verdictLines(r).at(-1).startsWith("PASS"));
  assert.match(r.checks.find((c) => c.id === "realistic_ladder.no_fall").detail, /one landing reached the body, at 1\.98 m\/s: the 0\.20 m step/);
});

test("RED: the landing never reaches the body (the lib.rs hand-off taken out)", () => {
  assert.deepStrictEqual(failed(handOffRemoved()), ["forgiving_walk.landing_handed", "realistic_walk.health", "realistic_walk.landing_handed", "realistic_walk.wound"]);
  const r = FJ.judgeFalls(handOffRemoved());
  const handed = r.checks.find((c) => c.id === "realistic_walk.landing_handed");
  assert.match(handed.detail, /0 landings reached the body for a 3\.00 m drop/);
  assert.match(r.checks.find((c) => c.id === "realistic_walk.wound").detail, /gives Sprained Ankle \(sprained_ankle\); the body has no wound/);
});

test("RED: the deck never placed (the console's line not taken): the walks stand on the floor and drop nothing", () => {
  const run = goodRun();
  run.deck_console = { line: DK.console, said: "unknown structure type 'deck'. types: stairs, ramp" };
  for (const id of ["forgiving_walk", "realistic_walk"]) {
    const l = leg(run, id);
    l.after.falls = { ...l.before.falls };
    l.after.vitals = clone(l.before.vitals);
    l.after.frames = track({ noDeck: true, health: () => l.before.vitals.health });
  }
  assert.deepStrictEqual(failed(run), ["deck_placed", "forgiving_walk.dropped", "forgiving_walk.stood_on_deck", "realistic_walk.dropped", "realistic_walk.stood_on_deck"]);
  const r = FJ.judgeFalls(run);
  assert.match(r.checks.find((c) => c.id === "forgiving_walk.stood_on_deck").detail, /stood 0\.00 m over the floor .*not standing on the deck the rig placed/);
  assert.ok(!r.checks.some((c) => c.id === "forgiving_walk.wound"), "nothing about wounds is judged on a leg that never fell");
  // No console answer recorded at all: said so.
  delete run.deck_console;
  assert.match(FJ.judgeFalls(run).checks.find((c) => c.id === "deck_placed").detail, /no console answer/);
});

test("RED: Forgiving read at full speed (the mode's share not applied)", () => {
  const run = goodRun();
  const f = leg(run, "forgiving_walk");
  f.after.vitals = vitals(100 - HARM3, [sprain()]);
  for (const fr of f.after.frames) fr.health = 100 - HARM3;
  assert.deepStrictEqual(failed(run).filter((id) => id.startsWith("forgiving_walk")), ["forgiving_walk.health", "forgiving_walk.wound"]);
});

test("RED: a placement's drop reaches the body (FallWatch's exemption broken)", () => {
  const run = goodRun();
  const p = leg(run, "placed_drop");
  const h = p.before.vitals.health;
  p.after.falls = fallsBlock({ landings: 4, last_m_s: V3, mode: "realistic" });
  p.after.vitals = vitals(h - HARM3, [sprain(), { id: "broken_leg", name: "Broken Leg", remaining_s: 7257600 }]);
  assert.deepStrictEqual(failed(run), ["placed_drop.health", "placed_drop.no_landing", "placed_drop.no_wound"]);
});

test("RED: the hatch's gate does not let the climber through (it hangs at the ladder's top over the hole)", () => {
  const run = goodRun();
  const l = leg(run, "realistic_ladder");
  const h = l.before.vitals.health;
  l.after.falls = { ...l.before.falls };
  l.after.frames = ladderTrack({ gateShut: true, health: () => h }).frames;
  assert.deepStrictEqual(failed(run), ["realistic_ladder.on_upper_floor"]);
  assert.match(FJ.judgeFalls(run).checks.find((c) => c.id === "realistic_ladder.on_upper_floor").detail, /hanging at the ladder's top over the hatch.*ladder_gates/);
});

test("RED: off the ladder's top down the hatch's hole: a fall, a wound, not on the floor", () => {
  const run = goodRun();
  const l = leg(run, "realistic_ladder");
  const h = l.before.vitals.health;
  const e = FJ.expectFall(DATA.falls, V_HOLE, "realistic");
  const down = ladderTrack({ downHole: true, health: (t, landedT) => (t >= landedT + 2 / 60 ? h - e.harm : h) });
  l.after.falls = fallsBlock({ landings: l.before.falls.landings + 1, last_m_s: down.impact, mode: "realistic" });
  l.after.vitals = vitals(h - e.harm, [sprain(), { id: e.wound, name: DATA.effects[e.wound].name, remaining_s: 3628800 }]);
  l.after.frames = down.frames;
  assert.deepStrictEqual(failed(run), ["realistic_ladder.health", "realistic_ladder.no_fall", "realistic_ladder.no_wound", "realistic_ladder.on_upper_floor"]);
  const r = FJ.judgeFalls(run);
  assert.match(r.checks.find((c) => c.id === "realistic_ladder.on_upper_floor").detail, /back on the floor below/);
  assert.match(r.checks.find((c) => c.id === "realistic_ladder.no_fall").detail, /harder than the 0\.20 m step off the top/);
});

test("RED: the climb never reaches the ladder's top", () => {
  const run = goodRun();
  const l = leg(run, "realistic_ladder");
  l.after.falls = { ...l.before.falls };
  l.after.frames = l.after.frames.map((f) => ({ ...f, cam: [...LD.hatch.stand, LD.hatch.yaw, 0] }));
  assert.deepStrictEqual(failed(run), ["realistic_ladder.climbed", "realistic_ladder.on_upper_floor"]);
  assert.match(FJ.judgeFalls(run).checks.find((c) => c.id === "realistic_ladder.on_upper_floor").detail, /back on the floor below/);
});

test("RED: a landing reached the body on the way into the world", () => {
  const run = goodRun();
  run.entry = snap(fallsBlock({ landings: 1, last_m_s: 9.0 }), vitals(88, [sprain()]));
  assert.deepStrictEqual(failed(run), ["world_entry"]);
});

// ── The planet legs (2026-10-20, second increment) ───────────────────────────────────────────────
// On a planet's ground the walk is `radial_step`, handed to the body by lib.rs's
// `CameraController::surface_step`; its height is the eye's distance from the planet's centre
// (`planet_r`), its gravity the planet's own, and a camera park's pin is one of its exemptions.

const PG = DATA.planet.gravity_m_s2;
const R0 = 6371000 + 312.4 + EYE + 0.05; // a standing eye over Sahara sand, metres from Earth's centre

/** The planet walk at 60 frames a second, the way radial_step runs it: standing at R0; Space held
 *  `climbS` (thrust ramping at 3 g to the walk's 5 m/s), then let go: the coast and the fall under
 *  `g`, the settle glue catching it within 0.5 m of the floor (the landing, at the speed
 *  falls::landing_speed gives) and easing the eye down at 4/s; then standing. `placedM`: no climb,
 *  the eye put that high and left to fall. `pinned`: the camera park holds the eye, nothing moves.
 *  Returns { frames, impact } (impact null if nothing landed). */
function planetTrack({ climbS = 0.35, placedM = 0, pinned = false, g = PG, health = () => 100 } = {}) {
  const dt = 1 / 60;
  const frames = [];
  let t = 0;
  let r = R0;
  let v = 0;
  let impact = null;
  let landedT = Infinity;
  const push = () => {
    frames.push({ t, cam: [0.4, 1.7, -0.2, 0, -0.087], health: null, planet_r: r });
    t += dt;
  };
  for (let i = 0; i < 18; i++) push();
  if (!pinned) {
    if (placedM) r = R0 + placedM;
    for (let k = 0; k < climbS / dt; k++) {
      v = Math.min(5, v + Math.max(3 * g, 20) * dt);
      r += v * dt;
      push();
    }
    // Four seconds of the walk after the key comes up: the coast, the fall and the glue's ease.
    for (let k = 0; k < 240; k++) {
      if (v <= 0 && r - R0 < 0.5) {
        if (impact === null && v < 0) {
          impact = Math.sqrt(v * v + 2 * g * (r - R0));
          landedT = t;
        }
        v = 0;
        r = R0 + (r - R0) * Math.exp(-4 * dt);
      } else {
        v -= g * dt;
        r = Math.max(R0, r + v * dt);
      }
      push();
    }
  }
  for (let i = 0; i < 120; i++) push();
  for (const f of frames) f.health = health(f.t, landedT);
  return { frames, impact };
}

const planetFalls = (over = {}) => ({
  landings: 0,
  last_m_s: null,
  mode: "realistic",
  gravity_m_s2: PG,
  flying: false,
  aboard: false,
  planet: "earth",
  probe_hold: false,
  ...over,
});

/** goodRun() with the planet legs a healthy build makes: the Forgiving climb lands hurting nobody,
 *  the Realistic one sprains, and the placement's drop reaches no body. */
/** `wounds` a minute older (a wound's time left runs down on its own), and with `renewed` (ids)
 *  started over, as a second wound of the same kind does (StatusEffects::apply keeps the longer). */
const aged = (wounds, renewed = []) =>
  wounds.map((w) => ({ ...w, remaining_s: renewed.includes(w.id) ? w.remaining_s - 4 : w.remaining_s - 60 }));

function goodPlanetRun() {
  const run = goodRun();
  run.planet = DATA.planet;
  const last = run.legs.at(-1).after;
  const h0 = last.vitals.health;
  // The aboard legs left a sprained ankle; a minute on, the planet legs begin.
  const w0 = aged(last.vitals.wounds);
  const n0 = last.falls.landings;
  const soft = planetTrack();
  const v = soft.impact;
  const e = FJ.expectFall(DATA.falls, v, "realistic");
  const hurt = planetTrack({ health: (t, landedT) => (t >= landedT + 2 / 60 ? h0 - e.harm : h0) });
  // The Realistic fall sprains the ankle again: the sprain starts its healing over.
  const hurtWounds = aged(last.vitals.wounds, [SPRAIN]);
  run.legs.push(
    {
      id: "planet_forgiving_fall",
      mode: "forgiving",
      kind: "planet_fall",
      body: "earth",
      before: { falls: planetFalls({ landings: n0, last_m_s: last.falls.last_m_s, mode: "forgiving", probe_hold: true }), vitals: vitals(h0, w0), frames: [] },
      after: { falls: planetFalls({ landings: n0 + 1, last_m_s: v, mode: "forgiving" }), vitals: vitals(h0, w0), frames: soft.frames.map((f) => ({ ...f, health: h0 })) },
    },
    {
      id: "planet_realistic_fall",
      mode: "realistic",
      kind: "planet_fall",
      body: "earth",
      before: { falls: planetFalls({ landings: n0 + 1, last_m_s: v }), vitals: vitals(h0, w0), frames: [] },
      after: { falls: planetFalls({ landings: n0 + 2, last_m_s: hurt.impact }), vitals: vitals(h0 - e.harm, hurtWounds), frames: hurt.frames },
    },
    {
      id: "planet_placed_drop",
      mode: "realistic",
      kind: "planet_placement",
      body: "earth",
      before: { falls: planetFalls({ landings: n0 + 2, last_m_s: hurt.impact }), vitals: vitals(h0 - e.harm, hurtWounds), frames: [] },
      after: {
        falls: planetFalls({ landings: n0 + 2, last_m_s: hurt.impact }),
        vitals: vitals(h0 - e.harm, hurtWounds),
        frames: planetTrack({ climbS: 0, placedM: 3, health: () => h0 - e.harm }).frames,
      },
    }
  );
  // Down the stairs (BUG-208), right after the Forgiving walk: a healthy build steps down every
  // riser and nothing lands. With a plan, the run must have every leg the rig runs.
  run.plan = FJ.ALL_LEGS;
  run.legs.splice(1, 0, stairsDownLeg(run.legs[0].after));
  // The deck (2026-10-20): stood on the 3.2 m deck built on the sand, walked off its open edge.
  run.legs.push(deckLeg(run.legs.at(-1).after));
  return run;
}

/** The deck's height (data/blueprints/basic.ron: the Wood Wall's 3.0 m and the Wood Roof's 0.2). */
const DECK_M = DATA.planet.deck.top_m;
/** The sand's clearance under a standing eye (src/surface_walk.rs DRAWN_CLEARANCE_M). */
const SAND_CLEARANCE_M = 0.05;

/** The planet walk at 60 frames a second, standing on the deck and walking off its edge: the eye
 *  stands DECK_M - SAND_CLEARANCE_M over its standing height on the sand (R0), then falls under G
 *  the way radial_step runs it, the settle glue catching it within 0.5 m of the floor (the landing,
 *  at the speed falls::landing_speed gives) and easing it down at 4/s; then standing on the sand.
 *  `through`: the walk before 2026-10-20, whose floor was the terrain alone, so the eye stands on
 *  the sand under the deck the whole time and walks out from under it. Returns { frames, impact }
 *  (impact null if nothing landed). */
function planetDeckTrack({ through = false, health = () => 100 } = {}) {
  const dt = 1 / 60;
  const frames = [];
  let t = 0;
  let r = through ? R0 : R0 + DECK_M - SAND_CLEARANCE_M;
  let v = 0;
  let impact = null;
  let landedT = Infinity;
  let z = 1.0;
  const push = () => {
    frames.push({ t, cam: [40, 1.7, z, Math.PI, -0.087], health: null, planet_r: r });
    t += dt;
  };
  for (let i = 0; i < 18; i++) push();
  // To the edge 1 m away at the limping 1.5 m/s, then the fall.
  for (let k = 0; k < 40; k++) {
    z += 1.5 * dt;
    push();
  }
  for (let k = 0; k < 240; k++) {
    z += k < 60 ? 1.5 * dt : 0;
    if (v <= 0 && r - R0 < 0.5) {
      if (impact === null && v < 0) {
        impact = Math.sqrt(v * v + 2 * G * (r - R0));
        landedT = t;
      }
      v = 0;
      r = R0 + (r - R0) * Math.exp(-4 * dt);
    } else {
      v -= G * dt;
      r = Math.max(R0, r + v * dt);
    }
    push();
  }
  for (const f of frames) f.health = health(f.t, landedT);
  return { frames, impact };
}

/** The deck leg of a healthy build, after the record `prev` (the planet placement's): one landing
 *  at the drop's speed, what falls.ron says of it in Realistic (on a body the legs before already
 *  sprained and broke an ankle, so a wound it already has starts its healing over). */
function deckLeg(prev) {
  const h0 = prev.vitals.health;
  const n0 = prev.falls.landings;
  const { impact } = planetDeckTrack();
  const e = FJ.expectFall(DATA.falls, impact, "realistic");
  const frames = planetDeckTrack({ health: (t, landedT) => (t >= landedT + 2 / 60 ? h0 - e.harm : h0) }).frames;
  const wounds = prev.vitals.wounds.map((w) => ({ ...w, remaining_s: w.id === e.wound ? w.remaining_s + 50 : w.remaining_s - 10 }));
  if (e.wound && !wounds.some((w) => w.id === e.wound)) wounds.push({ id: e.wound, name: DATA.effects[e.wound].name, remaining_s: 1e7 });
  return {
    id: "planet_deck_fall",
    mode: "realistic",
    kind: "planet_deck",
    body: "earth",
    deck_m: DECK_M,
    walk_m_s: 1.5,
    hold_s: 2.5 / 1.5,
    before: { falls: planetFalls({ landings: n0, last_m_s: prev.falls.last_m_s }), vitals: clone(prev.vitals), frames: [] },
    after: { falls: planetFalls({ landings: n0 + 1, last_m_s: impact }), vitals: vitals(h0 - e.harm, wounds), frames },
  };
}

/** The eye walking DOWN stairs-1 from its top tread at 5 m/s, 60 frames a second, through the
 *  room past the stairs' foot. `flies`: the walk before BUG-208, which left the top tread and fell
 *  the whole flight under G (landing at `landedT`); otherwise the eye follows every tread down.
 *  The flight's way up (its local +z turned by rot_deg), its risers and its treads are the tree's. */
function stairsDownTrack({ flies = false, health = () => 100 } = {}) {
  const dt = 1 / 60;
  const riser = ST.size[1] / ST.steps;
  const half = ST.size[2] / 2;
  const r = (ST.rot_deg * Math.PI) / 180;
  const [ux, uz] = [Math.sin(r), Math.cos(r)];
  const floorAt = (x, z) => {
    const along = (x - ST.pos[0]) * ux + (z - ST.pos[2]) * uz;
    if (along < -half || along > half) return 0;
    return Math.min(ST.top_m, (Math.floor((along + half) / ST.tread_m) + 1) * riser);
  };
  const frames = [];
  let t = 0;
  let [x, y, z] = ST.top_tread_eye;
  let v = 0;
  let landedT = Infinity;
  const push = () => {
    frames.push({ t, cam: [x, y, z, ST.down_yaw, 0], health: null });
    t += dt;
  };
  for (let i = 0; i < 18; i++) push();
  for (let i = 0; i < 60; i++) {
    x -= ux * 5 * dt;
    z -= uz * 5 * dt;
    if (flies && y > EYE) {
      v += G * dt;
      y = Math.max(EYE, y - v * dt);
      if (y === EYE && landedT === Infinity) landedT = t;
    } else if (!flies) {
      y = floorAt(x, z) + EYE;
    }
    push();
  }
  for (let i = 0; i < 120; i++) {
    if (flies && y > EYE) {
      v += G * dt;
      y = Math.max(EYE, y - v * dt);
      if (y === EYE && landedT === Infinity) landedT = t;
    }
    push();
  }
  for (const f of frames) f.health = health(f.t, landedT);
  return frames;
}

/** The stairs-down leg of a healthy build, between the records `prev` (the Forgiving walk's). */
function stairsDownLeg(prev) {
  return {
    id: "realistic_stairs_down",
    mode: "realistic",
    kind: "stairs_down",
    start: ST.top_tread_eye.slice(),
    before: { falls: fallsBlock({ ...prev.falls, mode: "realistic" }), vitals: clone(prev.vitals), frames: [] },
    after: { falls: fallsBlock({ ...prev.falls, mode: "realistic" }), vitals: clone(prev.vitals), frames: stairsDownTrack() },
  };
}

test("a healthy walk down the stairs passes: every riser a step down, nothing landed", () => {
  const r = FJ.judgeFalls(goodPlanetRun());
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  for (const want of ["realistic_stairs_down.dropped", "realistic_stairs_down.no_landing", "realistic_stairs_down.no_wound", "realistic_stairs_down.health"]) {
    assert.ok(r.checks.some((c) => c.id === want), `${want} was never judged`);
  }
  assert.ok(Math.abs(r.observed.realistic_stairs_down.drop_m - 3.0) < 0.01);
});

test("RED: walking down the stairs flies off the top tread and lands (the walk before BUG-208)", () => {
  const run = goodPlanetRun();
  const l = leg(run, "realistic_stairs_down");
  const h = l.before.vitals.health;
  l.after.falls = fallsBlock({ ...l.before.falls, landings: l.before.falls.landings + 1, last_m_s: V3 });
  l.after.vitals = vitals(h - HARM3, [sprain()]);
  l.after.frames = stairsDownTrack({ flies: true, health: (t, landedT) => (t >= landedT + 2 / 60 ? h - HARM3 : h) });
  assert.deepStrictEqual(failed(run), ["realistic_stairs_down.health", "realistic_stairs_down.no_landing", "realistic_stairs_down.no_wound"]);
  const d = FJ.judgeFalls(run).checks.find((c) => c.id === "realistic_stairs_down.no_landing").detail;
  assert.match(d, /walking DOWN the stairs.*BUG-208/);
});

test("RED: a walk down that never left the top, and a planned run without the stairs leg", () => {
  const still = goodPlanetRun();
  const l = leg(still, "realistic_stairs_down");
  l.after.frames = l.after.frames.map((f) => ({ ...f, cam: [...ST.top_tread_eye, 0, 0] }));
  assert.deepStrictEqual(failed(still), ["realistic_stairs_down.dropped"]);
  const short = goodPlanetRun();
  short.legs = short.legs.filter((x) => x.id !== "realistic_stairs_down");
  assert.deepStrictEqual(failed(short), ["realistic_stairs_down.ran"]);
  // An older saved run (no plan) is judged by the legs its rig had.
  delete short.plan;
  assert.deepStrictEqual(failed(short), []);
});

test("the planet's data: Earth's gravity from data/planets/earth.ron, and the climb's drop is a sprain in Realistic and nothing in Forgiving", () => {
  assert.deepStrictEqual(
    { body: DATA.planet.body, g: DATA.planet.gravity_m_s2, curved: DATA.planet.curved },
    { body: "earth", g: 9.81, curved: false }
  );
  assert.deepStrictEqual(FJ.readPlanetGravity("(\n  name: \"X\",\n  gravity: 3.71,\n  gravity_curve: Some([(0.0, 3.71)]),\n)"), { gravity_m_s2: 3.71, curved: true });
  assert.throws(() => FJ.readPlanetGravity("( name: \"X\" )"), /no top-level gravity/);
  // The made-up climb: the rig's 0.35 s of Space peaks about 2.6 m up and lands near 7.1 m/s.
  const { frames, impact } = planetTrack();
  const d = FJ.dropOf(frames, FJ.PLANET_DROP);
  assert.ok(d.drop_m > 2.4 && d.drop_m < 2.9, `${d.drop_m}`);
  assert.ok(Math.abs(impact - Math.sqrt(2 * PG * d.drop_m)) < 0.15, `${impact}`);
  assert.strictEqual(FJ.expectFall(DATA.falls, impact, "realistic").wound, "sprained_ankle");
  assert.deepStrictEqual(FJ.expectFall(DATA.falls, impact, "forgiving").wound, null);
  // The least drop a planet fall must make: falls.ron's first harm reached, with 5% to spare.
  assert.ok(Math.abs(FJ.planetMinDrop(DATA.falls, PG) - (1.05 * 6.26 * 6.26) / (2 * PG)) < 1e-9);
});

test("a run with the planet legs that went right passes every check, the planet's own included", () => {
  const r = FJ.judgeFalls(goodPlanetRun());
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  const ids = r.checks.map((c) => c.id);
  for (const want of [
    "planet_forgiving_fall.on_foot_on_planet",
    "planet_forgiving_fall.park_released",
    "planet_forgiving_fall.gravity",
    "planet_forgiving_fall.dropped",
    "planet_forgiving_fall.landing_handed",
    "planet_forgiving_fall.landing_speed",
    "planet_forgiving_fall.wound",
    "planet_forgiving_fall.health",
    "planet_realistic_fall.landing_handed",
    "planet_realistic_fall.wound",
    "planet_realistic_fall.wound_named",
    "planet_realistic_fall.health",
    "planet_realistic_fall.harm_at_landing",
    "planet_placed_drop.park_released",
    "planet_placed_drop.dropped",
    "planet_placed_drop.no_landing",
    "planet_placed_drop.no_wound",
    "planet_placed_drop.health",
  ]) {
    assert.ok(ids.includes(want), `${want} was never judged`);
  }
  assert.match(FJ.verdictLines(r).at(-1), /^PASS .*aboard and on a planet's ground/);
});

/** What the game reports with lib.rs's `surface_step` call taken out: the planet walk still
 *  rises and falls, and no landing reaches the body (the aboard legs are untouched). */
function surfaceStepRemoved() {
  const run = goodPlanetRun();
  const keep = run.legs.find((l) => l.id === "planet_forgiving_fall").before;
  for (const l of run.legs.filter((x) => x.id.startsWith("planet_"))) {
    for (const r of [l.before, l.after]) {
      r.falls.landings = keep.falls.landings;
      r.falls.last_m_s = keep.falls.last_m_s;
      r.vitals = clone(keep.vitals);
    }
    for (const f of l.after.frames) f.health = keep.vitals.health;
  }
  return run;
}

test("RED: a planet landing never reaches the body (lib.rs's surface_step hand-off taken out)", () => {
  // The deck's landing (2026-10-20) goes the same way: it is a planet walk's landing too.
  assert.deepStrictEqual(failed(surfaceStepRemoved()), [
    "planet_deck_fall.health",
    "planet_deck_fall.landing_handed",
    "planet_deck_fall.wound",
    "planet_forgiving_fall.landing_handed",
    "planet_realistic_fall.health",
    "planet_realistic_fall.landing_handed",
    "planet_realistic_fall.wound",
  ]);
  const handed = FJ.judgeFalls(surfaceStepRemoved()).checks.find((c) => c.id === "planet_realistic_fall.landing_handed");
  assert.match(handed.detail, /0 landings reached the body for a 2\.\d\d m drop .*CameraController::surface_step/);
});

test("RED: the rig's held key leaves the camera park in place (the walk never moves, every fall exempt)", () => {
  const run = goodPlanetRun();
  for (const id of ["planet_forgiving_fall", "planet_realistic_fall"]) {
    const l = leg(run, id);
    l.after.falls = { ...l.before.falls, probe_hold: true };
    l.after.vitals = clone(l.before.vitals);
    l.after.frames = planetTrack({ pinned: true, health: () => l.before.vitals.health }).frames;
  }
  const f = failed(run);
  for (const want of ["planet_forgiving_fall.park_released", "planet_forgiving_fall.dropped", "planet_realistic_fall.park_released", "planet_realistic_fall.dropped"]) {
    assert.ok(f.includes(want), `${want} not failed: ${f.join(", ")}`);
  }
  // The placement after them started with the park still on: its "no landing" would be the
  // park's exemption, not the placement test's, so it fails too.
  const p = leg(run, "planet_placed_drop");
  p.before.falls.probe_hold = true;
  assert.ok(failed(run).includes("planet_placed_drop.park_released"));
});

test("RED: a planet placement's drop reaches the body (surface_step's placement test broken)", () => {
  const run = goodPlanetRun();
  const p = leg(run, "planet_placed_drop");
  const h = p.before.vitals.health;
  p.after.falls = planetFalls({ landings: p.before.falls.landings + 1, last_m_s: 7.67 });
  p.after.vitals = vitals(h - HARM3, [...p.before.vitals.wounds, { id: "broken_leg", name: DATA.effects.broken_leg.name, remaining_s: 7257600 }]);
  assert.deepStrictEqual(failed(run), ["planet_placed_drop.health", "planet_placed_drop.no_landing", "planet_placed_drop.no_wound"]);
});

test("RED: Forgiving on a planet read at full speed, the planet's gravity not the file's, dev flight on, aboard", () => {
  const a = goodPlanetRun();
  const f = leg(a, "planet_forgiving_fall");
  const e = FJ.expectFall(DATA.falls, f.after.falls.last_m_s, "realistic");
  // Read at full speed the Forgiving fall sprains the (already sprained) ankle again.
  f.after.vitals = vitals(f.before.vitals.health - e.harm, f.before.vitals.wounds.map((w) => (w.id === SPRAIN ? { ...w, remaining_s: w.remaining_s + 56 } : w)));
  assert.deepStrictEqual(failed(a), ["planet_forgiving_fall.health", "planet_forgiving_fall.wound"]);
  const b = goodPlanetRun();
  leg(b, "planet_realistic_fall").after.falls.gravity_m_s2 = 9.80665;
  assert.ok(failed(b).includes("planet_realistic_fall.gravity"));
  const c = goodPlanetRun();
  leg(c, "planet_realistic_fall").before.falls.flying = true;
  assert.ok(failed(c).includes("planet_realistic_fall.on_foot_on_planet"));
  const d = goodPlanetRun();
  leg(d, "planet_realistic_fall").after.falls.aboard = true;
  assert.ok(failed(d).includes("planet_realistic_fall.on_foot_on_planet"));
});

test("the planet legs are judged with the game's own gravity there (a curved world's reading stands)", () => {
  const run = goodPlanetRun();
  run.data = { ...run.data, planet: { ...run.data.planet, curved: true } };
  // A heavier world: the same climb coasts less high, and its landing is sqrt(2 g h) at ITS g.
  const g = 11.15;
  const l = leg(run, "planet_realistic_fall");
  const t = planetTrack({ g });
  const e = FJ.expectFall(DATA.falls, t.impact, "realistic");
  const h0 = l.before.vitals.health;
  l.after.falls = planetFalls({ landings: l.before.falls.landings + 1, last_m_s: t.impact, gravity_m_s2: g });
  l.after.frames = planetTrack({ g, health: (tt, landedT) => (tt >= landedT + 2 / 60 ? h0 - e.harm : h0) }).frames;
  // Whatever falls.ron says of that landing, on a body that already has a sprained ankle (started
  // over if it is that).
  const others = l.before.vitals.wounds.filter((w) => w.id !== e.wound);
  l.after.vitals = vitals(h0 - e.harm, [...others, ...(e.wound ? [{ id: e.wound, name: DATA.effects[e.wound].name, remaining_s: 1e9 }] : [])]);
  const bad = failed(run).filter((id) => id.startsWith("planet_realistic_fall"));
  assert.deepStrictEqual(bad, [], `judged at the game's ${g}: ${bad.join(", ")}`);
  // A judge that took Earth's (or the ship's) 9.81 would have called that landing too fast for
  // its own drop: the camera's speed differs by more than the tolerance.
  const drop = FJ.dropOf(l.after.frames, FJ.PLANET_DROP).drop_m;
  assert.ok(Math.abs(t.impact - Math.sqrt(2 * 9.81 * drop)) > Math.max(FJ.LIMITS.SPEED_TOL_M_S, t.impact * FJ.LIMITS.SPEED_TOL_SHARE));
});

test("RED: a run that planned the planet legs and stopped before them", () => {
  const run = goodPlanetRun();
  run.legs = run.legs.filter((l) => !l.id.startsWith("planet_"));
  assert.deepStrictEqual(failed(run), ["planet_deck_fall.ran", "planet_forgiving_fall.ran", "planet_placed_drop.ran", "planet_realistic_fall.ran"]);
});

// ── The deck (2026-10-20) ─────────────────────────────────────────────────────────────────────────
// A Wood Roof on three Wood Walls built on the sand, the player stood on the roof and walked off
// its edge. Before 2026-10-20 the planet walk's floor was the terrain alone and went through it.

test("the deck is the tree's: a Wood Roof on three Wood Walls, 3.2 m, open to the south, 40 m east of the spot", () => {
  const d = DATA.planet.deck;
  assert.ok(Math.abs(d.top_m - 3.2) < 1e-9, `the deck is ${d.top_m} m`);
  assert.strictEqual(d.spec, "wood_wall@40,-2,0;wood_wall@38,0,1;wood_wall@42,0,1;roof@40,0,0");
  assert.deepStrictEqual([d.stand, d.edge_m, d.heading_deg], [[40, 1], 1, 180]);
  assert.strictEqual(FJ.pickDeck({ wood_wall: { size: [4, 3, 0.2] } }), null, "no roof, no deck");
});

test("a healthy deck leg passes: stood on the deck, walked off it, one landing at the drop's speed and its wound", () => {
  const r = FJ.judgeFalls(goodPlanetRun());
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  for (const want of [
    "planet_deck_fall.mode",
    "planet_deck_fall.on_foot_on_planet",
    "planet_deck_fall.park_released",
    "planet_deck_fall.gravity",
    "planet_deck_fall.stood_on_deck",
    "planet_deck_fall.dropped",
    "planet_deck_fall.landing_handed",
    "planet_deck_fall.landing_speed",
    "planet_deck_fall.wound",
    "planet_deck_fall.wound_named",
    "planet_deck_fall.health",
    "planet_deck_fall.harm_at_landing",
  ]) {
    assert.ok(r.checks.some((c) => c.id === want), `${want} was never judged`);
  }
  const o = r.observed.planet_deck_fall;
  assert.ok(Math.abs(o.drop_m - (DECK_M - SAND_CLEARANCE_M)) < 1e-6, JSON.stringify(o));
  assert.strictEqual(o.deck_m, DECK_M);
});

test("RED: the walk goes through the deck to the ground (the planet walk before 2026-10-20)", () => {
  const run = goodPlanetRun();
  const l = leg(run, "planet_deck_fall");
  l.after.falls = { ...l.before.falls };
  l.after.vitals = clone(l.before.vitals);
  l.after.frames = planetDeckTrack({ through: true, health: () => l.before.vitals.health }).frames;
  assert.deepStrictEqual(failed(run), ["planet_deck_fall.dropped", "planet_deck_fall.stood_on_deck"]);
  const d = FJ.judgeFalls(run).checks.find((c) => c.id === "planet_deck_fall.stood_on_deck").detail;
  assert.match(d, /went through it to the ground .*built_floor_radius/);
});

test("RED: a deck landing that never reaches the body, and a planned run without the deck leg", () => {
  const run = goodPlanetRun();
  const l = leg(run, "planet_deck_fall");
  l.after.falls = { ...l.after.falls, landings: l.before.falls.landings, last_m_s: l.before.falls.last_m_s };
  l.after.vitals = clone(l.before.vitals);
  for (const f of l.after.frames) f.health = l.before.vitals.health;
  assert.deepStrictEqual(failed(run), ["planet_deck_fall.health", "planet_deck_fall.landing_handed", "planet_deck_fall.wound"]);
  const short = goodPlanetRun();
  short.legs = short.legs.filter((x) => x.id !== "planet_deck_fall");
  assert.deepStrictEqual(failed(short), ["planet_deck_fall.ran"]);
  // A run saved before the deck leg is judged by the plan its rig wrote.
  short.plan = FJ.ALL_LEGS.filter((id) => id !== "planet_deck_fall");
  assert.deepStrictEqual(failed(short), []);
});

// ── Real captures from the running game (tests/fixtures/falls/) ──────────────────────────────────
// Two runs of `just verify-falls` on 2026-10-20, trimmed to what the judge reads. GREEN is this
// tree's build. RED is a build with lib.rs's `take_landing` -> `falls::player_landed` hand-off
// commented out: in the running game the camera dropped 3.00 m off the stairs and 3.20 m off the
// ladder, and the body's health, wounds and landing count never moved. Each carries the data it ran
// on, so it is judged against that, not against today's falls.ron.

const FIXTURES = path.join(REPO, "tests", "fixtures", "falls");
const fixture = (name) => JSON.parse(fs.readFileSync(path.join(FIXTURES, name), "utf8"));

test("the real green run passes every check (its walk skipped the top step at 54 ms frames, and the judge followed the real drop)", () => {
  const run = fixture("green.json");
  const r = FJ.judgeFalls(run);
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  // The realistic walk left from the 13th step: 2.79 m, 7.39 m/s, a sprain and 8.17 health, which
  // a judge expecting the nominal 3 m (9.68 health) would have failed.
  const w = r.observed.realistic_walk;
  assert.ok(Math.abs(w.drop_m - 2.79) < 0.01 && Math.abs(w.landing_m_s - 7.39) < 0.01, JSON.stringify(w));
  assert.deepStrictEqual(r.observed.realistic_ladder.new_wounds, ["broken_ankle"]);
  assert.strictEqual(run.falls_log.length, 3, "one [Falls] line a landing that reached the body");
});

test("the real red run (the lib.rs hand-off taken out) fails on every leg a landing should have reached", () => {
  assert.deepStrictEqual(failed(fixture("red-hand-off-removed.json")), [
    "forgiving_walk.landing_handed",
    "realistic_ladder.health",
    "realistic_ladder.landing_handed",
    "realistic_ladder.wound",
    "realistic_walk.health",
    "realistic_walk.landing_handed",
    "realistic_walk.wound",
  ]);
});

// The planet legs' first real run (2026-10-20), against the build whose engine/rig_walk.rs `hold`
// pressed its keys straight into the controller: the camera park's pin stayed on through the held
// Space (probe_hold true before and after), the eye rose 0.6 m and fell back, and nothing reached
// the body. Aboard every check passed. This is the rig quirk the planet legs exist to keep fixed.
test("the real run with the rig quirk (the camera park kept through a held key) fails the planet legs on exactly that", () => {
  const run = fixture("red-park-held.json");
  assert.deepStrictEqual(failed(run), [
    "planet_forgiving_fall.dropped",
    "planet_forgiving_fall.park_released",
    "planet_placed_drop.park_released",
    "planet_realistic_fall.dropped",
    "planet_realistic_fall.park_released",
  ]);
  const r = FJ.judgeFalls(run);
  assert.ok(r.observed.planet_realistic_fall.drop_m < 1, JSON.stringify(r.observed.planet_realistic_fall));
  assert.match(r.checks.find((c) => c.id === "planet_realistic_fall.park_released").detail, /rig_walk\.rs `hold` must let go of the park/);
});

// GREEN, EVERY LEG (2026-10-20): this tree's build (main with BUG-207 and BUG-208), 76/76, its
// plan naming all eight legs. Down the stairs from the top tread: 3.00 m of stairs and no landing.
// On Earth's ground the climb rose 2.61 m and 2.63 m, both landings reached the body (6.99 and
// 7.03 m/s; the eye's drop says 7.16 and 7.18: the planet walk's integrator reads about 2% slow at
// 30 frames a second, inside the judge's 3%), Forgiving took nothing and Realistic sprained the
// already sprained ankle again (its healing started over) and took 6.19.
test("the real green run with every leg passes every check", () => {
  const run = fixture("green-with-planet.json");
  // Saved before the deck leg (2026-10-20): its rig planned every leg but that one, and a run is
  // held to the plan its rig wrote.
  assert.deepStrictEqual(run.plan, FJ.ALL_LEGS.filter((id) => !FJ.DECK_LEGS.includes(id)), "the run planned every leg its rig had");
  const r = FJ.judgeFalls(run);
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  assert.ok(r.checks.some((c) => c.id === "realistic_stairs_down.no_landing"));
  assert.ok(Math.abs(r.observed.realistic_stairs_down.drop_m - 3.0) < 0.02 && r.observed.realistic_stairs_down.landing_m_s === null);
  const p = r.observed.planet_realistic_fall;
  assert.deepStrictEqual(p.new_wounds, ["sprained_ankle"]);
  assert.ok(Math.abs(p.landing_m_s - 7.03) < 0.01 && Math.abs(p.health_before - p.health_after - 6.19) < 0.05, JSON.stringify(p));
  assert.strictEqual(r.observed.planet_forgiving_fall.health_before - r.observed.planet_forgiving_fall.health_after < 0.5, true);
});

test("the rig plans every leg the judge knows", () => {
  const src = fs.readFileSync(path.join(REPO, "scripts", "verify-falls.js"), "utf8");
  assert.match(src, /plan: FJ\.ALL_LEGS,/, "verify-falls.js no longer writes the plan the judge holds a run to");
  for (const id of FJ.ALL_LEGS) assert.ok(src.includes(`"${id}"`), `verify-falls.js never runs ${id}`);
});

// RED FIRST DOWN THE STAIRS (2026-10-20): main with BUG-208's step down taken out (camera.rs's
// "Down a step, or off an edge" block disabled), the walk before that fix. In the running game the
// body walked off the top tread facing down, flew the flight and landed from 3.00 m at 7.67 m/s:
// a Sprained Ankle and 9.68 health, from walking down stairs. Every other check passed.
test("the real run before BUG-208 fails the walk down the stairs on exactly that", () => {
  const run = fixture("red-stairs-down-before-bug208.json");
  assert.deepStrictEqual(failed(run), ["realistic_stairs_down.health", "realistic_stairs_down.no_landing", "realistic_stairs_down.no_wound"]);
  const o = FJ.judgeFalls(run).observed.realistic_stairs_down;
  assert.ok(Math.abs(o.landing_m_s - 7.67) < 0.02 && Math.abs(o.drop_m - 3.0) < 0.02, JSON.stringify(o));
  assert.deepStrictEqual(o.new_wounds, ["sprained_ankle"]);
});

// RED FIRST ON A PLANET (2026-10-20): a build with lib.rs's `surface_step` call commented out, the
// hand-off that reports a planet landing, the quirk already fixed. In the running game the eye rose
// 2.60 m and 2.63 m on Space at the walk's 1.50 m/s (both ankles hurt by the legs aboard) and fell
// back; nothing reached the body in either mode. Aboard every check passed.
test("the real run with the planet hand-off taken out fails the planet falls on exactly that", () => {
  const run = fixture("red-surface-step-removed.json");
  assert.deepStrictEqual(failed(run), [
    "planet_forgiving_fall.landing_handed",
    "planet_realistic_fall.health",
    "planet_realistic_fall.landing_handed",
    "planet_realistic_fall.wound",
  ]);
  const o = FJ.judgeFalls(run).observed.planet_realistic_fall;
  assert.ok(o.drop_m > 2.5 && o.drop_m < 2.8 && o.landing_m_s === null, JSON.stringify(o));
  assert.ok(Math.abs(o.gravity_m_s2 - 9.81) < 1e-3);
});

// RED FIRST ON THE DECK (2026-10-20): a build of the planet-floor tree with lib.rs passing no built
// floor, so the planet walk's floor was the terrain alone, as before that day. In the running game
// the dev `build` verb stood the deck up ("4 of 4 pieces stood up"), the eye stood on the sand under
// it (-0.01 m over the ground it ended on) and walking off the edge dropped nothing. Every other leg
// passed, 80/82.
test("the real run with the built floor disabled fails the deck leg on exactly that", () => {
  const run = fixture("red-deck-floor-disabled.json");
  assert.deepStrictEqual(run.plan, FJ.ALL_LEGS, "the run planned every leg the rig has");
  assert.deepStrictEqual(failed(run), ["planet_deck_fall.dropped", "planet_deck_fall.stood_on_deck"]);
  const o = FJ.judgeFalls(run).observed.planet_deck_fall;
  assert.ok(Math.abs(o.drop_m) < 0.05 && o.landing_m_s === null && Math.abs(o.deck_m - 3.2) < 1e-9, JSON.stringify(o));
});

// GREEN, EVERY LEG WITH THE DECK (2026-10-20): this tree's build, 88/88. On the deck the eye stood
// 3.12 m over the sand it came to rest on (the deck is 3.20 m; the sand's 0.05 m clearance and the
// ground between), walking off its edge fell 3.12 m, and one landing reached the body at 7.67 m/s
// (the drop says 7.82: the planet walk's integrator about 2% slow): a Sprained Ankle again, just
// under falls.ron's 7.73 m/s broken ankle, and 9.70 health.
test("the real green run with the deck passes every check", () => {
  const run = fixture("green-with-deck.json");
  assert.deepStrictEqual(run.plan, FJ.ALL_LEGS, "the run planned every leg the rig has");
  const r = FJ.judgeFalls(run);
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  const o = r.observed.planet_deck_fall;
  assert.ok(Math.abs(o.drop_m - 3.12) < 0.01 && Math.abs(o.landing_m_s - 7.67) < 0.01, JSON.stringify(o));
  assert.deepStrictEqual(o.new_wounds, ["sprained_ankle"]);
  assert.ok(Math.abs(o.health_before - o.health_after - 9.7) < 0.05, JSON.stringify(o));
});

// RED FIRST ON THE UPPER FLOOR (2026-10-21, ship homes increment 7): a build of the upper-floor
// rig's tree with the ladder hatch's gate shut to a climber too (engine/footing.rs `shut_gates`
// extending `ladder_gates` whatever `on_ladder` said) and the rig's deck never placed (its console
// line not sent). In the running game the two walks stood 0.00 m over the greenhouse floor (the
// placement at the deck's height fell to the floor before the recording) and the climber hung at
// the ladder's top over the hatch at z 27.62, short of the gate at 28.0. Every other leg passed,
// 71/77, the stairs-down leg's 16 risers included.
test("the real run with no deck and the hatch's gate shut to a climber fails exactly those legs", () => {
  const run = fixture("red-gate-shut-no-deck.json");
  assert.deepStrictEqual(run.plan, FJ.ALL_LEGS, "the run planned every leg the rig has");
  assert.deepStrictEqual(failed(run), [
    "deck_placed",
    "forgiving_walk.dropped",
    "forgiving_walk.stood_on_deck",
    "realistic_ladder.on_upper_floor",
    "realistic_walk.dropped",
    "realistic_walk.stood_on_deck",
  ]);
  const r = FJ.judgeFalls(run);
  assert.match(r.checks.find((c) => c.id === "realistic_ladder.on_upper_floor").detail, /hanging at the ladder's top over the hatch at \(33\.00, 4\.90, 27\.62\)/);
  assert.ok(Math.abs(r.observed.realistic_stairs_down.drop_m - 3.0) < 0.02 && r.observed.realistic_stairs_down.landing_m_s === null);
});

// GREEN ON THE UPPER FLOOR (2026-10-21): this tree's build, 88/88. Both walks off the rig's deck fell
// 3.00 m and landed at 7.67 m/s (Forgiving: nothing; Realistic: a Sprained Ankle and 9.68 health);
// the climb through the hatch ended standing on the lift lobby's floor with one landing at 1.98 m/s,
// the 0.20 m step down from the top rung's hold, and nothing hurt.
test("the real green run on the upper floor passes every check", () => {
  const run = fixture("green-upper-floor.json");
  assert.deepStrictEqual(run.plan, FJ.ALL_LEGS, "the run planned every leg the rig has");
  const r = FJ.judgeFalls(run);
  assert.deepStrictEqual(r.checks.filter((c) => !c.ok), []);
  assert.strictEqual(r.checks.length, 88);
  for (const id of ["forgiving_walk", "realistic_walk"]) {
    const o = r.observed[id];
    assert.ok(Math.abs(o.drop_m - 3.0) < 0.02 && Math.abs(o.landing_m_s - 7.67) < 0.02, JSON.stringify(o));
  }
  assert.deepStrictEqual(r.observed.realistic_walk.new_wounds, ["sprained_ankle"]);
  const l = r.observed.realistic_ladder;
  // (The body heals a few ten-thousandths of a point between the two records.)
  assert.ok(Math.abs(l.landing_m_s - 1.98) < 0.01 && l.new_wounds.length === 0 && Math.abs(l.health_before - l.health_after) < 0.01, JSON.stringify(l));
  assert.match(run.deck_console.said, /^added structure #\d+ \(deck\)/);
  assert.match(run.deck_removed, /^removed structure #\d+/);
});

test("RED: the walk lands at a speed its own drop does not give", () => {
  const run = goodRun();
  leg(run, "realistic_walk").after.falls.last_m_s = 3.0; // the frame's velocity, say, not sqrt(2 g h)
  const f = failed(run);
  assert.ok(f.includes("realistic_walk.landing_speed"), f.join(", "));
});

test("RED: the mode verb not taken, dev flight on, the wrong gravity", () => {
  const a = goodRun();
  leg(a, "realistic_walk").after.falls.mode = "forgiving";
  assert.ok(failed(a).includes("realistic_walk.mode"));
  const b = goodRun();
  leg(b, "forgiving_walk").before.falls.flying = true;
  assert.ok(failed(b).includes("forgiving_walk.on_foot_aboard"));
  const c = goodRun();
  leg(c, "forgiving_walk").after.falls.gravity_m_s2 = 1.62;
  assert.ok(failed(c).includes("forgiving_walk.gravity"));
});

test("RED: the health fell before the eye landed, or long after", () => {
  const early = goodRun();
  for (const fr of leg(early, "realistic_walk").after.frames) fr.health = 100 - HARM3;
  assert.deepStrictEqual(failed(early), ["realistic_walk.harm_at_landing"]);
  const late = goodRun();
  const frames = leg(late, "realistic_walk").after.frames;
  const landedT = FJ.dropOf(frames).landed_t;
  for (const fr of frames) fr.health = fr.t >= landedT + 2.5 ? 100 - HARM3 : 100;
  assert.deepStrictEqual(failed(late), ["realistic_walk.harm_at_landing"]);
});

test("RED: a build older than the body probe, a missing leg, a panic, built-in data", () => {
  const old = goodRun();
  delete leg(old, "realistic_walk").after.vitals.health;
  assert.ok(failed(old).includes("realistic_walk.body_reported"));
  const short = goodRun();
  short.legs = short.legs.filter((l) => l.id !== "placed_drop");
  assert.ok(failed(short).includes("placed_drop.ran"));
  const loud = goodRun();
  loud.panics = 1;
  loud.builtin_data = ["[built-in data copy] medical/falls.ron"];
  assert.deepStrictEqual(failed(loud), ["no_builtin_data", "panics"]);
});

// ── The rig's --dry-verdict over saved runs ──────────────────────────────────────────────────────

test("the rig judges a saved run: green passes, the hand-off shape fails, nothing is booted", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hos-falls-judge-"));
  try {
    for (const [name, run, code, word] of [
      ["green", goodRun(), 0, /DRY VERDICT \(nothing was booted\): PASS/],
      ["red", handOffRemoved(), 2, /DRY VERDICT \(nothing was booted\): FAIL .*realistic_walk\.landing_handed/],
    ]) {
      const p = path.join(dir, `${name}.json`);
      fs.writeFileSync(p, JSON.stringify(run));
      const out = spawnSync(process.execPath, [path.join(REPO, "scripts", "verify-falls.js"), "--dry-verdict", p], { encoding: "utf8", timeout: 60000 });
      const text = `${out.stdout}\n${out.stderr}`;
      assert.strictEqual(out.status, code, text);
      assert.match(text, word);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
