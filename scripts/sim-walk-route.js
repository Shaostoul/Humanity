#!/usr/bin/env node
// sim-walk-route: the storeys rig's route as data for the headless walk (the headless sim runner,
// increment 4, 2026-11-15; docs/design/headless-sim-runner.md). `HumanityOS --sim walk --route FILE`
// walks it with no window and no GPU; this writes FILE from the same plan verify-storeys walks
// (scripts/lib/storeys-judge.js `buildPlan` over the tree's data), leg for leg and step for step as
// scripts/verify-storeys.js runs them, so the headless walk and the windowed rig cover the same
// legs. The screenshots are left out (they need the picture).
//
// Each leg: `place` (home metres x, z), `floor` (metres), `yaw` (radians, 0 north, +PI/2 east), or
// none to go on from where the last leg ended; then `steps`, each one of
//   {"do":"walk","to":[[x,z],...]}   the rig's walkRoute: turn toward each point, forward held for
//                                     the distance at the walk's speed, again until within 0.25 m,
//                                     then 1.5 s standing
//   {"do":"hold","keys":"forward","s":1.6,"after":1.0}   keys held s seconds, s + after recorded
//   {"do":"face_mid","a":[x,z],"b":[x,z]}                turn toward the middle of a..b
// and `wall` (a, b), the line a held leg must not cross, when the rig's judge has one.
//
// Usage: node scripts/sim-walk-route.js [--out FILE]   (default: stdout)

"use strict";
const fs = require("fs");
const path = require("path");
const SJ = require("./lib/storeys-judge");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const outAt = args.indexOf("--out");
const out = outAt >= 0 ? args[outAt + 1] : null;

const tree = SJ.readTreeData(REPO);
const built = SJ.buildPlan(tree);
if (built.problems.length) {
  console.error("sim-walk-route: the homestead no longer has what the route walks:\n  " + built.problems.join("\n  "));
  process.exit(2);
}
const P = built.plan;
const UP = P.upper_m;
const E = P.elevator;
const car = [E.pos[0], E.pos[2]];
const walk = (to) => ({ do: "walk", to });
const hold = (keys, s, after) => ({ do: "hold", keys, s, after });
const legs = [
  { id: "stairs_up", place: P.front_door, floor: 0, yaw: SJ.yawTo(P.stairs_route[0], P.stairs_route[1]), steps: [walk(P.stairs_route.slice(1))] },
  { id: "bedroom", steps: [walk(P.bedroom_route.slice(1))] },
  { id: "wall_upstairs", place: P.wall.up_from, floor: UP, yaw: Math.PI / 2, steps: [hold("forward", 1.6, 1.0)], wall: { a: P.wall.a, b: P.wall.b } },
  { id: "guard", place: P.guard.from, floor: UP, yaw: 0, steps: [hold("forward", 1.2, 1.0)], wall: { a: P.guard.a, b: P.guard.b } },
  { id: "wall_downstairs", place: P.wall.down_from, floor: 0, yaw: Math.PI / 2, steps: [walk([P.wall.down_to])] },
  { id: "jump_under_slab", place: P.jump_under.at, floor: 0, yaw: Math.PI / 2, steps: [hold("jump", 0.1, 2.4)] },
  { id: "jump_open", place: P.jump_open.at, floor: 0, yaw: Math.PI / 2, steps: [hold("jump", 0.1, 2.4)] },
  { id: "elevator_up", place: P.lift_from, floor: 0, yaw: SJ.yawTo(P.lift_from, car), steps: [walk([car]), hold("jump", 0.15, 3.5)] },
  { id: "elevator_out_top", steps: [walk([P.lift_out])] },
  { id: "elevator_down", steps: [walk([car]), hold("sprint", 0.15, 3.5)] },
  { id: "elevator_out_bottom", steps: [walk([P.lift_out])] },
  { id: "ladder_up", place: P.ladder_from, floor: 0, yaw: Math.PI, steps: [hold("jump", 1.8, 1.0), walk(P.ladder_route.slice(1))] },
  { id: "hatch_gate", steps: [walk([P.hatch_from]), { do: "face_mid", a: P.ladder.gate.a, b: P.ladder.gate.b }, hold("forward", 1.0, 1.0)] },
  { id: "elevator_call", place: P.lift_call_from, floor: UP, yaw: 0, steps: [hold("forward", 4.5, 1.5)] },
];
const route = { name: "storeys", eye_m: P.eye_m, upper_m: UP, legs };
const body = JSON.stringify(route, null, 1);
if (out) {
  fs.writeFileSync(out, body);
  console.log(`sim-walk-route: ${legs.length} legs written to ${out}`);
} else {
  process.stdout.write(body + "\n");
}
