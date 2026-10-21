// The storeys judge (scripts/lib/storeys-judge.js): what `just verify-storeys` passes and fails.
//
// Run: node --test scripts/tests/storeys-judge.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted. A made-up run that went right passes every check; then one broken
// thing at a time, each failing its own checks. The made-up run is built from the tree's own data
// (the homestead's stair, slabs, zones, elevator and ladder, and the plan the rig walks), so a
// change to the layout moves the run with it, and the readers are tested on the real files too.
// Then the same runs written to a folder are judged by the rig itself, `node
// scripts/verify-storeys.js --dry-verdict`, as a saved run would be; and the two real runs kept in
// tests/fixtures/storeys/ (the green one, and the red one on a homestead with its floor slabs
// taken out) are judged as they were.
//
// THE SHAPE THAT MATTERS MOST is `noUpperFloor`: what the running game does when the slabs never
// reach the walk. The stair still climbs its sixteen treads to 3.0 m, and walking on past the top
// drops the body 3 m back to the ground floor, where the HUD names the hall downstairs. Every unit
// test over the synthetic two-storey home stays green in that state; this judge must not.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const SJ = require("../lib/storeys-judge.js");

const REPO = path.join(__dirname, "..", "..");
const TREE = SJ.readTreeData(REPO);
const BUILT = SJ.buildPlan(TREE);
const P = BUILT.plan;
const UP = P.upper_m;
const EYE = P.eye_m;
const R = P.radius_m;
const G = P.gravity_m_s2;
const DT = 1 / 60;
const WALK = 5.0;
/// The home's corner in the camera's frame: not the origin, so every reading must take it off.
const O = [120.0, 7.5, -64.0];

// ── The made-up run ─────────────────────────────────────────────────────────────────────────────

/** The room the HUD names at a home point: the zone whose box holds the eye. */
function roomAt(x, y, z) {
  const zn = TREE.home.zones.find(
    (q) => x >= q.origin[0] && x <= q.origin[0] + q.size[0] && z >= q.origin[2] && z <= q.origin[2] + q.size[2] && y >= q.origin[1] && y <= q.origin[1] + q.size[1]
  );
  return zn ? { id: zn.id, name: TREE.room_names[zn.room_type] || zn.id, floor: zn.origin[1] } : null;
}

/** One recorder frame at the home point (x, eye y, z) with its storey block, in world metres. */
function frame(t, x, y, z, s = {}) {
  const floor = s.floor === undefined ? y - EYE : s.floor;
  const room = s.room === undefined ? roomAt(x, y, z) : s.room;
  return {
    t,
    cam: [x + O[0], y + O[1], z + O[2], 0, 0],
    health: 100,
    storey: {
      floor: floor + O[1],
      ceiling: s.ceiling === undefined || s.ceiling === null ? null : s.ceiling + O[1],
      on_ladder: !!s.on_ladder,
      at_ladder: !!s.at_ladder,
      on_lift: !!s.on_lift,
      aboard: true,
      room: room ? { id: room.id, name: room.name, floor: room.floor + O[1] } : null,
      hud_shown: s.hud === undefined ? true : s.hud,
      cars: s.cars || [[0, 0]],
      landings: s.landings || 0,
      last_m_s: s.last === undefined ? null : s.last,
    },
  };
}
const rec = (frames) => ({ frames });
const seg = (what, frames) => ({ what, rec: rec(frames) });

/** The stair's floor under a home point: its tread's top on the flight, the upper floor past it,
 *  the ground before it. */
function stairFloor(x, z) {
  const st = P.stairs;
  const [lx, lz] = SJ.toLocal(st.pos, st.yaw, x, z);
  if (Math.abs(lx) > st.w / 2 || lz < -st.d / 2) return 0;
  if (lz > st.d / 2) return UP;
  const tread = st.d / st.steps;
  const k = Math.min(st.steps, Math.floor((lz + st.d / 2) / tread) + 1);
  return (k * st.h) / st.steps;
}

/** Frames walking from p through each point at the walk's speed, the floor from `floorAt(x, z)`
 *  (and the rest of the storey block from `extra(x, z)`), starting at clock `t0`. */
function walkFrames(pts, floorAt, extra = () => ({}), t0 = 0) {
  const out = [];
  let t = t0;
  let [x, z] = pts[0];
  const push = () => {
    const f = floorAt(x, z);
    out.push(frame(t, x, f + EYE, z, { floor: f, ...extra(x, z) }));
    t += DT;
  };
  push();
  for (const [tx, tz] of pts.slice(1)) {
    for (;;) {
      const d = Math.hypot(tx - x, tz - z);
      if (d < 1e-9) break;
      const step = Math.min(d, WALK * DT);
      x += ((tx - x) / d) * step;
      z += ((tz - z) / d) * step;
      push();
    }
  }
  return out;
}

/** Frames walking from p toward q until held `held` metres off the line a..b (the sweep's push-out
 *  against a wall, a guard or a gate), then pressing there `pressS` seconds. */
function heldFrames(p, q, a, b, held, floor, pressS = 0.8) {
  const out = [];
  let t = 0;
  let [x, z] = p;
  const d0 = Math.hypot(q[0] - p[0], q[1] - p[1]);
  const ux = (q[0] - p[0]) / d0;
  const uz = (q[1] - p[1]) / d0;
  const sign = Math.sign(SJ.sideOf(a, b, x, z));
  const push = () => {
    out.push(frame(t, x, floor + EYE, z, { floor }));
    t += DT;
  };
  push();
  for (;;) {
    const nx = x + ux * WALK * DT;
    const nz = z + uz * WALK * DT;
    if (SJ.sideOf(a, b, nx, nz) * sign < held) break;
    x = nx;
    z = nz;
    push();
  }
  // On to exactly `held` off the line, where the sweep holds it.
  const s = SJ.sideOf(a, b, x, z) * sign;
  x += ux * Math.max(0, s - held);
  z += uz * Math.max(0, s - held);
  for (let k = 0; k < pressS / DT; k++) push();
  return out;
}

/** A jump at the home point: up at 5 m/s under the ceiling (the clamp at its underside less the
 *  head allowance), down under G onto the floor; one landing at the speed of the drop made. */
function jumpFrames(x, z, ceiling) {
  const out = [];
  let t = 0;
  let y = EYE;
  let v = 5.0;
  let top = y;
  const clamp = ceiling === null ? Infinity : ceiling - P.head_m;
  out.push(frame(t, x, y, z, { floor: 0, ceiling }));
  for (;;) {
    t += DT;
    v -= G * DT;
    y += v * DT;
    if (y > clamp) {
      y = clamp;
      v = Math.min(v, 0);
    }
    top = Math.max(top, y);
    if (y <= EYE) {
      y = EYE;
      out.push(frame(t, x, y, z, { floor: 0, ceiling, landings: 1, last: Math.sqrt(2 * G * (top - EYE)) }));
      break;
    }
    out.push(frame(t, x, y, z, { floor: 0, ceiling }));
  }
  for (let k = 0; k < 30; k++) {
    t += DT;
    out.push(frame(t, x, EYE, z, { floor: 0, ceiling, landings: 1, last: Math.sqrt(2 * G * (top - EYE)) }));
  }
  return out;
}

const E = P.elevator;
const CAR = [E.pos[0], E.pos[2]];
const L = P.ladder;
/** Where the body stands held at the shut landing door, metres south of the shaft's edge: its
 *  radius and the gate's half thickness (engine/footing.rs `footprint_sides`, 0.05). */
const DOOR_HELD = R + 0.05;

/** The elevator ride: the car (and the body on it) from `from` to `to` at 1.5 m/s. */
function rideFrames(from, to) {
  const out = [];
  let t = 0;
  let c = from;
  out.push(frame(t, CAR[0], c + EYE, CAR[1], { floor: c, on_lift: true, cars: [[c, to]], room: roomAt(CAR[0], c + EYE, CAR[1]) }));
  while (Math.abs(c - to) > 1e-9) {
    t += DT;
    c += Math.sign(to - c) * Math.min(Math.abs(to - c), 1.5 * DT);
    out.push(frame(t, CAR[0], c + EYE, CAR[1], { floor: c, on_lift: true, cars: [[c, to]] }));
  }
  for (let k = 0; k < 20; k++) {
    t += DT;
    out.push(frame(t, CAR[0], c + EYE, CAR[1], { floor: c, on_lift: true, cars: [[c, to]] }));
  }
  return out;
}

/** The call from the upper landing: walked north from the call point to the landing door, held
 *  there while the car comes up from `carFrom` (called once the body is within reach), then onto
 *  the car. `door`: how far off the shaft's edge the door holds the body (null: it does not). */
function callFrames({ carFrom = 0, door = DOOR_HELD } = {}) {
  const out = [];
  let t = 0;
  const x = P.lift_call_from[0];
  let z = P.lift_call_from[1];
  let car = carFrom;
  let target = carFrom;
  const edge = CAR[1] + E.d / 2; // the shaft's lobby side
  const northWall = CAR[1] - E.d / 2 + R + 0.03; // the shaft's own wall upstairs
  const push = () => out.push(frame(t, x, UP + EYE, z, { floor: z < edge ? car : UP, cars: [[car, target]] }));
  push();
  for (let k = 0; k < 6 / DT; k++) {
    t += DT;
    if (z - edge <= 1.0) target = UP; // within the call reach of the upper landing
    car += Math.sign(target - car) * Math.min(Math.abs(target - car), 1.5 * DT);
    const away = Math.abs(car - UP) > 0.02;
    z = Math.max(away && door !== null ? edge + door : northWall, z - WALK * DT);
    push();
  }
  return out;
}

function greenLegs() {
  const top = P.stairs.top;
  const ladderFrames = () => {
    const out = [];
    let t = 0;
    let y = EYE;
    const [x, z0] = P.ladder_from;
    out.push(frame(t, x, y, z0, { floor: 0, at_ladder: true }));
    while (y < L.hang_eye_m) {
      t += DT;
      y = Math.min(L.hang_eye_m, y + 3 * DT);
      out.push(frame(t, x, y, z0, { floor: 0, at_ladder: true, on_ladder: true }));
    }
    for (let k = 0; k < 40; k++) {
      t += DT;
      out.push(frame(t, x, y, z0, { floor: 0, at_ladder: true, on_ladder: true }));
    }
    return out;
  };
  // South off the top over the gate: holding on at the top (the eye 0.2 m over the floor) while
  // within the ladder's reach, then down the 0.2 m onto the lobby floor (its slab from z 28.1):
  // one soft landing.
  const offLadder = () => {
    const reach = Math.max(L.w, L.d) / 2 + 0.9;
    let landed = false;
    const out = walkFrames(P.ladder_route, (x, z) => (z >= 28.1 ? UP : 0)).map((f) => {
      const x = f.cam[0] - O[0];
      const z = f.cam[2] - O[2];
      const inReach = Math.hypot(x - L.pos[0], z - L.pos[2]) <= reach;
      landed = landed || !inReach;
      const floor = z >= 28.1 ? UP : 0;
      return frame(f.t, x, inReach ? L.hang_eye_m : floor + EYE, z, { floor, at_ladder: inReach, on_ladder: inReach, landings: landed ? 1 : 0, last: landed ? Math.sqrt(2 * G * 0.2) : null });
    });
    const end = out[out.length - 1];
    for (let k = 1; k <= 20; k++) out.push(frame(end.t + k * DT, P.ladder_off[0], UP + EYE, P.ladder_off[1], { floor: UP, landings: 1, last: Math.sqrt(2 * G * 0.2) }));
    return out;
  };
  return [
    {
      id: "stairs_up",
      kind: "walk",
      segments: [seg("before", [frame(0, P.front_door[0], EYE, P.front_door[1], { floor: 0 })]), seg("walk", walkFrames(P.stairs_route, stairFloor))],
    },
    { id: "bedroom", kind: "walk", segments: [seg("before", [frame(0, top[0], UP + EYE, top[1])]), seg("walk", walkFrames(P.bedroom_route, () => UP))] },
    { id: "wall_upstairs", kind: "held", segments: [seg("held", heldFrames(P.wall.up_from, [P.wall.a[0] + 3, P.wall.up_from[1]], P.wall.a, P.wall.b, R + 0.075, UP))] },
    { id: "guard", kind: "held", segments: [seg("held", heldFrames(P.guard.from, [P.guard.from[0], P.guard.a[1] - 3], P.guard.a, P.guard.b, R + 0.03, UP))] },
    { id: "wall_downstairs", kind: "walk", segments: [seg("walk", walkFrames([P.wall.down_from, P.wall.down_to], () => 0))] },
    { id: "jump_under_slab", kind: "jump", segments: [seg("jump", jumpFrames(P.jump_under.at[0], P.jump_under.at[1], P.jump_under.underside_m))] },
    { id: "jump_open", kind: "jump", segments: [seg("jump", jumpFrames(P.jump_open.at[0], P.jump_open.at[1], null))] },
    {
      id: "elevator_up",
      kind: "elevator",
      segments: [seg("onto the car", walkFrames([P.lift_from, CAR], () => 0)), seg("Space", rideFrames(0, UP))],
    },
    { id: "elevator_out_top", kind: "walk", segments: [seg("off", walkFrames([CAR, P.lift_out], () => UP, () => ({ cars: [[UP, UP]] })))] },
    {
      id: "elevator_down",
      kind: "elevator",
      segments: [seg("back on", walkFrames([P.lift_out, CAR], () => UP, () => ({ cars: [[UP, UP]] }))), seg("Shift", rideFrames(UP, 0))],
    },
    { id: "elevator_out_bottom", kind: "walk", segments: [seg("off", walkFrames([CAR, P.lift_out], () => 0))] },
    { id: "ladder_up", kind: "ladder", segments: [seg("climb", ladderFrames()), seg("off", offLadder())] },
    { id: "hatch_gate", kind: "held", segments: [seg("held", heldFrames(P.hatch_from, [P.hatch_from[0], P.hatch_from[1] - 3], L.gate.a, L.gate.b, R + L.d / 2, UP))] },
    { id: "elevator_call", kind: "elevator", segments: [seg("call", callFrames())] },
  ];
}

function greenRun() {
  return {
    kind: "verify-storeys",
    plan: P,
    problems: [],
    origin: O,
    legs: greenLegs(),
    shots: SJ.ALL_SHOTS.map((id) => ({ id, file: `${id}.png`, ok: true, bytes: 900000 })),
    data_dir: { logged: "C:\\Humanity\\data", how: "repo, full", real: "C:\\Humanity\\data", expected: "c:/humanity/data/" },
    panics: 0,
    builtin_data: [],
  };
}

const failed = (run) => SJ.judgeStoreys(run).checks.filter((c) => !c.ok).map((c) => c.id);
const legOf = (run, id) => run.legs.find((l) => l.id === id);

/** THE RED SHAPE: no floor slabs reach the walk. Up the treads to 3.0 m, then past the top the
 *  floor is the ground's, 3 m down: the body falls and lands at sqrt(2 g 3), and the HUD names
 *  the hall downstairs. */
function noUpperFloor(run) {
  // Past the top tread the floor is the ground's: the walk drops 3 m there, and every frame after
  // carries the one hard landing.
  const frames = walkFrames(P.stairs_route, (x, z) => {
    const f = stairFloor(x, z);
    return f === UP ? 0 : f;
  });
  const v = Math.sqrt(2 * G * UP);
  let fell = false;
  for (let i = 1; i < frames.length; i++) {
    if (!fell && frames[i].storey.floor - O[1] === 0 && frames[i - 1].storey.floor - O[1] > UP - 0.5) fell = true;
    if (fell) {
      frames[i].storey.landings = 1;
      frames[i].storey.last_m_s = v;
    }
  }
  legOf(run, "stairs_up").segments[1] = seg("walk", frames);
  return run;
}

// ── The tests ───────────────────────────────────────────────────────────────────────────────────

test("the tree's own data reads, and the route the rig walks fits the homestead as built", () => {
  assert.deepStrictEqual(BUILT.problems, [], BUILT.problems.join("\n"));
  assert.strictEqual(UP, 3.0, "the stair rises to the upper floor at 3.0 m");
  assert.deepStrictEqual(P.stairs.foot, [42, 38.65], "1 m before stairs-1's first step, in the hall");
  assert.deepStrictEqual(P.stairs.top, [35.5, 38.65], "1 m past its top, on the stair-top landing");
  assert.strictEqual(P.stairs.up_yaw, -Math.PI / 2, "the stair rises west");
  assert.deepStrictEqual(P.front_door, [53.5, 40.5]);
  assert.deepStrictEqual(P.upper_hall, { id: "room-upper-hall", name: "Hall" });
  assert.deepStrictEqual(P.bedroom, { id: "room-bedroom", name: "Bedroom" });
  assert.deepStrictEqual(E.stops, [0, 3], "elevator-1 stops at the ground and the upper floor");
  assert.strictEqual(P.jump_under.underside_m, 2.75, "the gallery's slab: top 3.0, 0.25 thick");
  assert.deepStrictEqual(L.gate, { a: [32.6, 28], b: [33.4, 28] }, "the gate across ladder-1's top, the guards' whole 0.8 m gap");
  assert.strictEqual(P.head_m, 0.12);
  assert.strictEqual(R, 0.3);
  assert.strictEqual(EYE, 1.7);
});

test("the readers read every piece of the home design", () => {
  const h = TREE.home;
  assert.strictEqual(h.height, 6.0);
  assert.ok(h.walls.length > 40 && h.floors.length > 15 && h.zones.length > 25, `${h.walls.length} walls, ${h.floors.length} slabs, ${h.zones.length} zones`);
  const u4 = h.walls.find((w) => w.base === 3 && w.a[0] === 34 && w.a[1] === 38 && w.b[0] === 50);
  assert.ok(u4, "U4, the upper hall's north wall");
  assert.deepStrictEqual(
    u4.openings.map((o) => [o.kind, o.at, o.width]),
    [
      ["Door", 7, 0.9],
      ["Door", 9.5, 0.9],
      ["Door", 13, 0.9],
    ]
  );
  assert.ok(h.floors.some((f) => f.material === "Glass") && h.floors.some((f) => f.material === "Solid"), "glass lids and solid floors");
  assert.deepStrictEqual(SJ.readGuardMaterials('(id: 3, name: "Oak", default_thickness_m: 0.10),\n(id: 9, name: "Steel balustrade", form: Some("guard")),'), [9]);
  assert.strictEqual(SJ.readConst("pub const HEAD_ABOVE_EYE_M: f32 = 0.12;", "HEAD_ABOVE_EYE_M"), 0.12);
  assert.strictEqual(SJ.readConst("nothing", "HEAD_ABOVE_EYE_M"), null);
});

test("a straight leg through a wall of its storey is caught, and one through a door is not", () => {
  const h = TREE.home;
  // Ground floor, north from the second bedroom into the hall at x 43: the hall's north wall, no door there.
  assert.strictEqual(SJ.wallsCrossed(h, 0, EYE, R, [43, 36.8], [43, 40]).length, 1);
  // The same at x 47.45, through the second bedroom's door (x 47.0 to 47.9).
  assert.strictEqual(SJ.wallsCrossed(h, 0, EYE, R, [47.45, 36.8], [47.45, 40]).length, 0);
  // Upstairs over the same line at x 43: U4 has no door at x 43 either.
  assert.strictEqual(SJ.wallsCrossed(h, UP, EYE, R, [43, 36.8], [43, 39.9]).length, 1);
  // U7 stops a walk upstairs and not one downstairs.
  assert.strictEqual(SJ.wallsCrossed(h, UP, EYE, R, [43.5, 34], [47, 34]).length, 1);
  assert.strictEqual(SJ.wallsCrossed(h, 0, EYE, R, [43.5, 36.8], [47, 36.8]).length, 0);
});

test("a layout without what the route walks refuses before anything boots", () => {
  const variant = (edit) => {
    const t = JSON.parse(JSON.stringify(TREE));
    edit(t.home);
    return SJ.buildPlan(t).problems;
  };
  const noFloors = variant((h) => (h.floors = []));
  assert.ok(noFloors.some((p) => /no floor slab at 3 m under the stair's top landing/.test(p)), noFloors.join("\n"));
  const noStair = variant((h) => (h.structures = h.structures.filter((s) => s.id !== "stairs-1")));
  assert.ok(noStair.some((p) => /no structure "stairs-1"/.test(p)), noStair.join("\n"));
  const wallUnder = variant((h) => h.walls.push({ a: [45, 30], b: [45, 38], base: 0, height: 3, material: 2, openings: [] }));
  assert.ok(wallUnder.some((p) => /has a ground wall on its line/.test(p)), wallUnder.join("\n"));
  const solidGuard = variant((h) => (h.walls.find((w) => w.base === 3 && w.a[0] === 36.5 && w.a[1] === 39.3).material = 3));
  assert.ok(solidGuard.some((p) => /is not a balustrade/.test(p)), solidGuard.join("\n"));
  const blocked = variant((h) => h.walls.push({ a: [45, 39.5], b: [45, 41.5], base: 0, height: 3, material: 3, openings: [] }));
  assert.ok(blocked.some((p) => /the front door to the stair.*walks into the wall/.test(p)), blocked.join("\n"));
  const noLobby = variant((h) => (h.zones = h.zones.filter((z) => z.id !== "room-lift-lobby")));
  assert.ok(noLobby.some((p) => /no zone "room-lift-lobby"/.test(p)), noLobby.join("\n"));
  // A point the HUD is checked at, 0.3 m from a wall (a partition put up in the bedroom): the room's
  // box may stop short of it, and the HUD would name no room there whatever the storeys do.
  const nearWall = variant((h) => h.walls.push({ a: [38.5, 34.7], b: [40.5, 34.7], base: 3, height: 3, material: 3, openings: [] }));
  assert.ok(nearWall.some((p) => /the bedroom route's end .* within 0\.6 m of a wall/.test(p)), nearWall.join("\n"));
});

test("a run that went right passes every check", () => {
  const result = SJ.judgeStoreys(greenRun());
  const bad = result.checks.filter((c) => !c.ok);
  assert.deepStrictEqual(bad, [], bad.map((c) => `${c.id}: ${c.detail}`).join("\n"));
  assert.ok(result.checks.length > 60, `${result.checks.length} checks`);
  for (const id of SJ.ALL_LEGS) assert.ok(result.observed[id], `observed ${id}`);
  assert.match(SJ.verdictLines(result).slice(-1)[0], /^PASS /);
});

test("THE RED SHAPE: slabs that never reach the walk fail the stair leg on its floor, its room and its fall", () => {
  const f = failed(noUpperFloor(greenRun()));
  for (const want of ["stairs_up.upper_floor", "stairs_up.hud_room", "stairs_up.no_fall"]) assert.ok(f.includes(want), `${want} in ${f.join(", ")}`);
  assert.ok(f.includes("stairs_up.upper_floor") && !f.includes("stairs_up.on_the_treads"), "it did climb the treads: only the floor past the top is missing");
});

test("a wall upstairs that does not stop a body upstairs fails the wall leg", () => {
  const run = greenRun();
  legOf(run, "wall_upstairs").segments = [seg("held", walkFrames([P.wall.up_from, [P.wall.a[0] + 2, P.wall.up_from[1]]], () => UP))];
  assert.deepStrictEqual(failed(run), ["wall_upstairs.held"]);
});

test("a wall upstairs that stops a body downstairs fails the downstairs leg", () => {
  const run = greenRun();
  legOf(run, "wall_downstairs").segments = [seg("held", heldFrames(P.wall.down_from, P.wall.down_to, P.wall.a, P.wall.b, R + 0.075, 0))];
  assert.deepStrictEqual(failed(run), ["wall_downstairs.walked_under"]);
});

test("a jump through the slab fails the head clamp; a jump too weak to reach it fails either way", () => {
  const through = greenRun();
  legOf(through, "jump_under_slab").segments = [seg("jump", jumpFrames(P.jump_under.at[0], P.jump_under.at[1], null).map((f) => ((f.storey.ceiling = P.jump_under.underside_m + O[1]), f)))];
  assert.deepStrictEqual(failed(through), ["jump_under_slab.head_clamped"]);
  // The ceiling not known to the walk at all.
  const unknown = greenRun();
  legOf(unknown, "jump_under_slab").segments[0].rec.frames.forEach((f) => (f.storey.ceiling = null));
  assert.ok(failed(unknown).includes("jump_under_slab.ceiling_known"));
  // A weak jump: never reaches the clamp under the slab, and in the open never rises past it.
  const weak = greenRun();
  for (const id of ["jump_under_slab", "jump_open"]) {
    const fs = legOf(weak, id).segments[0].rec.frames;
    for (const f of fs) f.cam[1] = O[1] + EYE + (f.cam[1] - O[1] - EYE) * 0.5;
  }
  const f = failed(weak);
  assert.ok(f.includes("jump_under_slab.jumped") && f.includes("jump_open.rose_past_the_clamp"), f.join(", "));
});

test("a guard that lets the body over the stairwell's edge fails the guard leg", () => {
  const run = greenRun();
  const over = walkFrames([P.guard.from, [P.guard.from[0], 38.6]], (x, z) => (z < P.guard.a[1] ? stairFloor(x, z) : UP));
  const lastF = over[over.length - 1];
  lastF.storey.landings = 1;
  lastF.storey.last_m_s = Math.sqrt(2 * G * 1.3);
  legOf(run, "guard").segments = [seg("over", over)];
  const f = failed(run);
  for (const want of ["guard.held", "guard.stayed_on_floor", "guard.no_fall"]) assert.ok(f.includes(want), `${want} in ${f.join(", ")}`);
});

test("THE GATE, NOT THE GUARDS' ENDS: a body held only by the guards beside the ladder fails the hatch leg", () => {
  // The ends of G4 and G5 either side of the ladder's gap, when it was 0.6 m (until 2026-10-21; it
  // is 0.8 m now and they no longer touch a body in line with the ladder), held a body about 0.24
  // m off the gate's line on their own; a looser slack would have credited the gate with it.
  const run = greenRun();
  legOf(run, "hatch_gate").segments = [seg("held", heldFrames(P.hatch_from, [P.hatch_from[0], P.hatch_from[1] - 3], L.gate.a, L.gate.b, 0.24, UP))];
  assert.deepStrictEqual(failed(run), ["hatch_gate.held"]);
});

test("an elevator that does not ride, or jumps its rider, fails the ride", () => {
  const stuck = greenRun();
  legOf(stuck, "elevator_up").segments[1] = seg("Space", rideFrames(0, 0).map((f) => ((f.storey.on_lift = false), f)));
  const f = failed(stuck);
  assert.ok(f.includes("elevator_up.car_up") && f.includes("elevator_up.rode"), f.join(", "));
  const hop = greenRun();
  const ride = legOf(hop, "elevator_up").segments[1].rec.frames;
  for (let i = 5; i < 40; i++) ride[i].cam[1] += 1.0;
  assert.deepStrictEqual(failed(hop), ["elevator_up.no_jump"]);
  const down = greenRun();
  legOf(down, "elevator_down").segments[1] = seg("Shift", rideFrames(UP, UP));
  assert.ok(failed(down).includes("elevator_down.car_down"));
});

test("a landing door that opens onto an empty shaft, or a car that was never away, fails the call", () => {
  const open = greenRun();
  legOf(open, "elevator_call").segments = [seg("call", callFrames({ door: null }))];
  assert.ok(failed(open).includes("elevator_call.kept_out"), failed(open).join(", "));
  const waiting = greenRun();
  legOf(waiting, "elevator_call").segments = [seg("call", callFrames({ carFrom: UP }))];
  const f = failed(waiting);
  assert.ok(f.includes("elevator_call.car_away"), f.join(", "));
});

test("the HUD naming the room downstairs, or not drawn, fails the room checks", () => {
  const wrong = greenRun();
  const fs = legOf(wrong, "bedroom").segments[1].rec.frames;
  fs[fs.length - 1].storey.room = { id: "room-dining", name: "Dining Room", floor: O[1] };
  assert.deepStrictEqual(failed(wrong), ["bedroom.hud_room"]);
  const hidden = greenRun();
  const hs = legOf(hidden, "stairs_up").segments[1].rec.frames;
  hs[hs.length - 1].storey.hud_shown = false;
  assert.deepStrictEqual(failed(hidden), ["stairs_up.hud_room"]);
});

test("a placement in the middle of a walk is not a walk", () => {
  const jumped = greenRun();
  const fs = legOf(jumped, "stairs_up").segments[1].rec.frames;
  for (let i = 100; i < fs.length; i++) fs[i].cam[0] -= 2.0;
  assert.ok(failed(jumped).includes("stairs_up.walked"), failed(jumped).join(", "));
  const apart = greenRun();
  for (const f of legOf(apart, "bedroom").segments[1].rec.frames) f.cam[2] += 0.5;
  assert.ok(failed(apart).includes("bedroom.walked"), failed(apart).join(", "));
});

test("THE DATA THE GAME READ: a run whose game read other data than the run meant fails, whatever its legs did", () => {
  // The first red run (2026-10-21): the rig junctioned a floorless sandbox homestead beside the
  // exe, the game read the repo's own data/ (a source tree's data wins, lib.rs find_data_dir),
  // and every leg passed. Its legs were green; only this check can tell.
  const run = greenRun();
  run.sandbox_data = "C:\\Temp\\red-data";
  run.data_dir = { logged: "C:\\Humanity\\data", how: "repo, full", real: "C:\\Humanity\\data", expected: "C:\\Temp\\red-data" };
  assert.deepStrictEqual(failed(run), ["data_dir"]);
  const unknown = greenRun();
  delete unknown.data_dir;
  assert.deepStrictEqual(failed(unknown), ["data_dir"]);
  // The junction beside the exe, resolved to the folder meant: the same folder.
  const junction = greenRun();
  junction.data_dir = { logged: "C:\\Temp\\rig\\data", how: "full, with world/", real: "C:\\Temp\\red-data", expected: "c:\\temp\\red-data\\" };
  assert.deepStrictEqual(failed(junction), []);
});

test("a leg that never ran, a shot never taken, a panic or built-in data each fail", () => {
  const run = greenRun();
  run.legs = run.legs.filter((l) => l.id !== "ladder_up");
  run.shots = run.shots.filter((s) => s.id !== "bedroom");
  run.panics = 1;
  run.builtin_data = ["[DataStore] homes/homestead.ron: served from the copy built into the exe"];
  assert.deepStrictEqual(failed(run).sort(), ["ladder_up.ran", "no_builtin_data", "panics", "shot.bedroom"].sort());
  const empty = greenRun();
  legOf(empty, "guard").segments = [seg("held", [])];
  assert.deepStrictEqual(failed(empty), ["guard.recorded"]);
  assert.deepStrictEqual(failed({}), ["plan"]);
});

test("the rig re-judges a saved run without booting anything (--dry-verdict)", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "storeys-judge-"));
  try {
    const green = path.join(dir, "green.json");
    const red = path.join(dir, "red.json");
    fs.writeFileSync(green, JSON.stringify(greenRun()));
    fs.writeFileSync(red, JSON.stringify(noUpperFloor(greenRun())));
    const run = (p) => spawnSync(process.execPath, [path.join(REPO, "scripts", "verify-storeys.js"), "--dry-verdict", p], { encoding: "utf8" });
    const g = run(green);
    assert.strictEqual(g.status, 0, g.stdout + g.stderr);
    assert.match(g.stdout, /DRY VERDICT \(nothing was booted\): PASS/);
    const r = run(red);
    assert.strictEqual(r.status, 2, r.stdout + r.stderr);
    assert.match(r.stdout, /FAIL\s+stairs_up\.upper_floor/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// The two real runs of 2026-10-21, kept trimmed (scripts/verify-storeys.js's manifests: each
// frame's t, cam and storey block as recorded, numbers to 4 decimals). Each carries the plan it
// ran on, so a later change to the homestead does not move them.
const FIXTURES = path.join(REPO, "tests", "fixtures", "storeys");
const fixture = (name) => JSON.parse(fs.readFileSync(path.join(FIXTURES, name), "utf8"));

test("the real green run (this tree's release build, 76/76) passes as it did", () => {
  const result = SJ.judgeStoreys(fixture("green.json"));
  const bad = result.checks.filter((c) => !c.ok);
  assert.deepStrictEqual(bad, [], bad.map((c) => `${c.id}: ${c.detail}`).join("\n"));
  assert.strictEqual(result.checks.length, 76);
});

test("the real red run (the same build on a homestead with its floor slabs taken out) fails every leg that needs the upper floor", () => {
  const run = fixture("red-no-floor-slabs.json");
  const f = failed(run);
  // Up the treads, then 3 m down past the top: the floor, the room and the fall.
  for (const want of ["stairs_up.upper_floor", "stairs_up.hud_room", "stairs_up.no_fall"]) assert.ok(f.includes(want), `${want} in ${f.join(", ")}`);
  assert.ok(!f.includes("stairs_up.on_the_treads"), "it did climb the treads: only the floor past the top is missing");
  // It read the floorless data it meant to (the first red attempt did not, and passed).
  assert.ok(!f.includes("data_dir"), "the red run read its sandbox data");
  // Every leg that needs the upper floor fails something; the legs on the ground floor alone pass.
  for (const id of ["stairs_up", "bedroom", "wall_upstairs", "guard", "jump_under_slab", "elevator_up", "elevator_out_top", "elevator_down", "ladder_up", "hatch_gate", "elevator_call"]) {
    assert.ok(f.some((c) => c.startsWith(`${id}.`)), `${id} passed on a homestead with no upper floor`);
  }
  for (const id of ["wall_downstairs", "jump_open", "elevator_out_bottom"]) assert.ok(!f.some((c) => c.startsWith(`${id}.`)), `${id} failed: ${f.filter((c) => c.startsWith(`${id}.`))}`);
});
