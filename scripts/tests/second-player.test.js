// The scripted second player (scripts/second-player.js): the parts that need
// no relay (week plan Day 3, 2026-10-02).
//
// Run: node --test scripts/tests/second-player.test.js   (in `just rig-tests`)
//
// Pure node: nothing is booted, nothing is connected to, well under a second,
// which is why it can sit inside `just verify`. The tests that run the script
// against a REAL relay boot one, so they live in their own file and recipe:
// scripts/tests/second-player-relay.test.js, `just verify-second-player`.
//
// What this proves, in plain terms:
//  1. Its paths and its facing use the desktop app's own conventions.
//  2. No single step is ever longer than MAX_STEP_M, even after a long pause,
//     and no walk faster than MAX_SPEED_MPS, so the relay's speed check (ship
//     homes increment 4: at most 46.9 m banked, 25 m/s on foot) never corrects
//     one; and a correction, should one come, stands the walker where the relay
//     holds it and every later update says so.
//  3. The timestamp follows THE TIMESTAMP RULE (top of second-player.js): the
//     sender's own clock in SECONDS, moving on by exactly the time step the
//     walk used, so velocity = distance moved / stamp difference.
//  4. Its default name starts with TestBot (kept off the server's member
//     list), and a `--seed random` run without `--name` gets a name of its
//     own, so a second random run is not refused as "already registered".

const { test } = require("node:test");
const assert = require("node:assert");

const sp = require("../second-player.js");
const { bankedAllowanceM } = require("../lib/copresence-judge.js");
const fs = require("fs");
const path = require("path");
// The relay's own rules for moving aboard (data/ship/shared_world.ron, src/ship/moves.rs).
const RULES = fs.readFileSync(path.join(__dirname, "..", "..", "data", "ship", "shared_world.ron"), "utf8");
const ON_FOOT_MPS = Number(RULES.match(/on_foot_mps:\s*([0-9.]+)/)[1]);

const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

// Red checks run 2026-10-02, each restored afterwards:
//  - facingQuat's yaw changed to atan2(vx, vz) (the crew NPCs' convention,
//    not the desktop camera's): FAILED here at "walking (0, 1) faces (0, -1)",
//    and the relay test at "it faces the way it walks (cos -0.999)".
//  - pathPoint's circle drawn at radius 1 instead of r: FAILED here at "every
//    circle point is radius metres out (s=0)", and the relay test at "only the
//    last 0 of 129 updates were on the circle".
test("paths and facing use the desktop app's conventions", () => {
  // The desktop camera at yaw looks along (sin yaw, 0, -cos yaw)
  // (src/renderer/camera.rs forward) and sends [0, sin(yaw/2), 0, cos(yaw/2)]
  // (src/engine/net_route.rs send_game_position). Turn a rotation back into
  // the way it looks, and it must be the way we walked.
  const looksAlong = (q) => {
    const yaw = 2 * Math.atan2(q[1], q[3]);
    return [Math.sin(yaw), -Math.cos(yaw)];
  };
  for (const [vx, vz] of [[1, 0], [0, 1], [-1, 0], [0, -1], [0.6, -0.8], [-3, 4]]) {
    const [fx, fz] = looksAlong(sp.facingQuat(vx, vz));
    const n = Math.hypot(vx, vz);
    assert.ok(Math.abs(fx - vx / n) < 1e-9 && Math.abs(fz - vz / n) < 1e-9, `walking (${vx}, ${vz}) faces (${fx}, ${fz})`);
  }
  // A camera turned a quarter (yaw = +90 degrees) looks along +X.
  const q = sp.facingQuat(1, 0);
  assert.ok(Math.abs(q[1] - Math.sin(Math.PI / 4)) < 1e-9, "a walker facing +X sends the desktop's yaw of +90 degrees");

  const circle = { path: "circle", center: [5, 1.7, -2], radius: 3 };
  for (let s = 0; s < 40; s += 0.7) {
    const p = sp.pathPoint(circle, s);
    assert.ok(Math.abs(Math.hypot(p[0] - 5, p[2] + 2) - 3) < 1e-9, `every circle point is radius metres out (s=${s})`);
    assert.equal(p[1], 1.7);
  }
  const line = { path: "line", axis: "z", center: [0, 0, 0], radius: 2 };
  assert.deepEqual(sp.pathPoint(line, 0), [0, 0, -2], "a line starts at one end");
  assert.deepEqual(sp.pathPoint(line, 4), [0, 0, 2], "reaches the other end after twice the radius");
  assert.deepEqual(sp.pathPoint(line, 6), [0, 0, 0], "and comes back");
});

// Red check run 2026-10-02: the cap taken out of makeWalk's path step
// (`travelled += plan.speed * dt`, the code before this date). FAILED at
// "after a 120 s pause the step on the path was 148.9 m". A 1.4 m/s walk
// round a 100 m circle covers 168 m of path in 120 s, and the straight line
// between those two points is 148.9 m: the relay would drop that update and,
// measuring every later one from the old place, all the ones after it.
test("no step is longer than MAX_STEP_M, even after a long pause", () => {
  assert.ok(sp.MAX_STEP_M < bankedAllowanceM(RULES), `the cap (${sp.MAX_STEP_M} m) sits under what the relay lets anyone bank (${bankedAllowanceM(RULES)} m)`);
  const plan = { path: "circle", center: [0, 1.7, 0], radius: 100, speed: 1.4 };

  // On the path from the start.
  const walk = sp.makeWalk(plan, sp.pathPoint(plan, 0), 1);
  let prev = walk.here();
  let now = 1;
  for (const pause of [1 / 15, 120, 1 / 15, 600, 1 / 15]) {
    now += pause;
    const { msg } = walk.next(now);
    const step = dist(msg.position, prev.position);
    assert.ok(step <= sp.MAX_STEP_M + 1e-9, `after a ${pause.toFixed(2)} s pause the step on the path was ${step.toFixed(1)} m`);
    assert.ok(step > 0, "and it still moved on");
    prev = msg;
  }

  // On the way from a spawn point 1 km away to the start of the path,
  // including a pause on the way.
  const far = sp.makeWalk(plan, [1100, 1.7, 0], 1);
  prev = far.here();
  now = 1;
  for (const pause of [1 / 15, 10, 1 / 15, 30]) {
    now += pause;
    const { msg } = far.next(now);
    const step = dist(msg.position, prev.position);
    assert.ok(step <= sp.MAX_STEP_M + 1e-9, `after a ${pause.toFixed(2)} s pause the step towards the path was ${step.toFixed(1)} m`);
    assert.ok(dist(msg.position, far.target) < dist(prev.position, far.target), "and it got closer to the path");
    prev = msg;
  }
});

// Red checks run 2026-10-02, each restored afterwards:
//  - makeWalk's stamp sent in milliseconds (`timestamp: clockS * 1000`, the
//    unit the script sent before this date): FAILED at "the first update
//    carries the sender clock as given (12.5 s)", and, with that line removed
//    too, at "the stamp moved on by the time step (66.667 s against a
//    0.0667 s step)".
//  - senderClockS counting milliseconds (`performance.now()` without the
//    / 1000): FAILED at "the sender clock counts seconds: 30 ms of waiting
//    moved it by 29.9334".
test("the timestamp is the sender's clock in seconds, moving on by the walk's own time step", () => {
  const plan = { path: "circle", center: [0, 1.7, 0], radius: 4, speed: 1.4 };
  // The sender may count from any start it likes; 12.5 s is as good as any.
  const t0 = 12.5;
  const walk = sp.makeWalk(plan, sp.pathPoint(plan, 0), t0);
  const first = walk.here();
  assert.equal(first.timestamp, t0, `the first update carries the sender clock as given (${t0} s)`);
  assert.deepEqual(first.velocity, [0, 0, 0], "standing still before the first step");

  let prev = first;
  let now = t0;
  // Uneven steps, as real timers give: the stamps must follow them exactly.
  for (const dt of [1 / 15, 1 / 15, 0.05, 0.2, 1 / 15, 0.0005]) {
    now += dt;
    const { msg } = walk.next(now);
    const stampDt = msg.timestamp - prev.timestamp;
    // A step under a millisecond is treated as one millisecond, by the stamp
    // and the velocity alike (makeWalk's next()).
    const used = Math.max(dt, 0.001);
    assert.ok(Math.abs(stampDt - used) < 1e-9, `the stamp moved on by the time step (${stampDt.toFixed(3)} s against a ${used.toFixed(4)} s step)`);
    const moved = [0, 1, 2].map((k) => (msg.position[k] - prev.position[k]) / stampDt);
    const err = Math.hypot(...[0, 1, 2].map((k) => msg.velocity[k] - moved[k]));
    assert.ok(err < 1e-6, `velocity = distance moved / stamp difference (sent ${msg.velocity.map((v) => v.toFixed(3))}, moved ${moved.map((v) => v.toFixed(3))})`);
    prev = msg;
  }

  const last = walk.stopped(now);
  assert.deepEqual(last.velocity, [0, 0, 0], "the last update stands still");
  assert.deepEqual(last.position, prev.position, "where it was");
  assert.ok(last.timestamp > prev.timestamp, "stamped after the update before it");

  // The real clock: seconds, steady, and never 0 (0 means "no clock").
  const a = sp.senderClockS();
  assert.ok(a > 0, "a sender clock reading is never 0");
  assert.ok(a < 1e6, `the sender clock is seconds since this process started, not milliseconds since 1970 (${a})`);
  const waitFrom = Date.now();
  while (Date.now() - waitFrom < 30) {} // 30 ms, by a different clock
  const moved = sp.senderClockS() - a;
  assert.ok(moved > 0.02 && moved < 1, `the sender clock counts seconds: 30 ms of waiting moved it by ${moved}`);
});

// Red checks run 2026-10-02, each restored afterwards:
//  - the default name put back to "Walker": FAILED at "the default name
//    starts with TestBot (got Walker)".
//  - nameFor returning opts.name unchanged (no key suffix, the code before
//    this date): FAILED at "two random identities get two different names
//    (TestBotWalker and TestBotWalker)".
test("names: TestBot by default, and a random identity gets a name of its own", () => {
  const def = sp.parseOptions([]);
  assert.ok(def.name.startsWith(sp.TEST_BOT_PREFIX), `the default name starts with ${sp.TEST_BOT_PREFIX} (got ${def.name})`);
  assert.equal(def.nameGiven, false);

  const keyA = "3fa9c1" + "0".repeat(58);
  const keyB = "77e012" + "0".repeat(58);

  // A fixed identity keeps the plain name, so it is recognisable run to run.
  assert.equal(sp.nameFor(def, keyA), def.name, "no --seed: the plain name");
  assert.equal(sp.nameFor(sp.parseOptions(["--seed", "my walker"]), keyA), def.name, "a fixed --seed: the plain name");

  // --seed random without --name: the name carries part of the new key.
  const random = sp.parseOptions(["--seed", "random"]);
  const a = sp.nameFor(random, keyA);
  const b = sp.nameFor(random, keyB);
  assert.notEqual(a, b, `two random identities get two different names (${a} and ${b})`);
  assert.equal(a, `${def.name}-3fa9c1`);
  assert.ok(a.startsWith(sp.TEST_BOT_PREFIX), "and keep the TestBot prefix");
  assert.ok(sp.NAME_RULE.test(a), `a name the relay accepts (${a})`);

  // A name asked for is used as asked.
  assert.equal(sp.nameFor(sp.parseOptions(["--seed", "random", "--name", "Bob"]), keyA), "Bob");

  // A long default still fits the relay's 24 characters with its suffix.
  const long = sp.nameFor({ name: `${sp.TEST_BOT_PREFIX}${"x".repeat(17)}`, nameGiven: false, seed: "random" }, keyA);
  assert.equal(long.length, 24, `trimmed to 24 characters (${long})`);
  assert.ok(sp.NAME_RULE.test(long) && long.endsWith("-3fa9c1"));
});

// Increment 1b (docs/design/ship-homes-and-logistics.md): the relay hands
// every player a plot, so another player's spot is inside THEIR home. With a
// plot of its own the walker starts its path at its own spawn on it (no
// approach walk, and the walk stays at home); as a guest the old rule holds.
// Seen red 2026-10-03 with the home_plot branch of chooseCenter removed (the
// Day 3 rule): "a line along x starts at our own spawn" failed, the path
// starting 99.08 m away, around the other player.
test("with a plot of its own the walk starts at its own spawn; a guest walks round the others", () => {
  const welcome = (homePlot) => ({
    type: "game_welcome",
    player_id: 9,
    home_plot: homePlot,
    world_snapshot: [
      { entity_id: 4, entity_type: "player", position: [53.5, 1.7, 40.5], components: { name: "Other" } },
      { entity_id: 9, entity_type: "player", position: [53.5, 1.7, 139.5], components: { name: "Me" } },
    ],
  });
  const p2 = { id: "p2", kind: "homestead", origin: [0, 0, 99], size: [55, 3, 89] };
  for (const [path, axis] of [["line", "x"], ["line", "z"], ["circle", "x"]]) {
    const opts = { center: "auto", path, axis, radius: 4 };
    const { center, start } = sp.chooseCenter(opts, welcome(p2));
    const first = sp.pathPoint({ path, axis, center, radius: 4 }, 0);
    assert.ok(dist(first, start) < 1e-9, `a ${path} along ${axis} starts at our own spawn (${dist(first, start).toFixed(2)} m away)`);
  }
  // A guest (the ship is full): round the other player, as before.
  const guest = sp.chooseCenter({ center: "auto", path: "line", axis: "z", radius: 4 }, welcome(null));
  assert.deepEqual(guest.center, [53.5, 1.7, 40.5]);
  // --center given: as asked.
  assert.deepEqual(sp.chooseCenter({ center: [1, 2, 3], path: "line", axis: "x", radius: 4 }, welcome(p2)).center, [1, 2, 3]);

  // The log lines the rig reads: the plot, and who was already there (increment 2: a player
  // standing still sends no updates, so this is how the walker sees them).
  assert.deepEqual(sp.presentLines(welcome(p2)), ['player present: entity 4 "Other" at (53.50, 1.70, 40.50)']);
  assert.equal(sp.homePlotLine(welcome(p2)), `home_plot ${JSON.stringify(p2)}`);
  assert.match(sp.homePlotLine(welcome(null)), /^home_plot null /);
  assert.match(sp.homePlotLine({ type: "game_welcome" }), /^home_plot missing /);
});

// Ship homes 1b, the second review: only a join naming the relay's ship holds a
// plot (one naming none is a guest in the Commons), so the walker asks the
// relay's public /api/server-info which ship it has and names it in its join,
// like the desktop app. Seen red 2026-10-03 with joinMessage ignoring the ship
// (the 65b3e2c0c walker): "the join names the relay's ship:
// undefined".
test("the join names the relay's ship, read from /api/server-info", async () => {
  const ship = { id: "mothership-1", hash: "0123456789abcdef" };
  const join = sp.joinMessage("TestBotWalker", { body: "x" }, ship);
  assert.equal(join.ship_hash, ship.hash, `the join names the relay's ship: ${join.ship_hash}`);
  assert.equal(join.type, "game_join");
  assert.equal(join.home_spawn, undefined, "a scripted player draws no home, so it names no door");
  assert.equal(sp.joinMessage("TestBotWalker", {}, null).ship_hash, undefined, "no ship known: a guest");

  assert.equal(sp.httpBase("ws://127.0.0.1:3210/ws"), "http://127.0.0.1:3210");
  assert.equal(sp.httpBase("wss://example.org/ws"), "https://example.org");

  // A relay's server-info, served from this process (no relay booted).
  const http = require("node:http");
  let answer = { name: "x", ship };
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(req.url === "/api/server-info" ? JSON.stringify(answer) : "{}");
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  try {
    const url = `ws://127.0.0.1:${server.address().port}/ws`;
    assert.deepEqual(await sp.fetchShip(url), ship, "the relay's ship from /api/server-info");
    answer = { name: "x", ship: { id: "", hash: "" } };
    assert.equal(await sp.fetchShip(url), null, "a relay whose ship did not load names none");
    answer = { name: "x" };
    assert.equal(await sp.fetchShip(url), null, "a relay from before 1b names none");
  } finally {
    server.close();
  }
});

// The rig reads where the relay spawned the OTHER players and which of their
// updates it passed on (verify-copresence.js --plots, stepping out and back).
// Seen red 2026-10-03 with logOthers logging every update (no quarter-metre
// rule): the 0.1 m step "saw entity 4 at
// (53.60, 1.70, 40.50)" was logged and the deep-equal failed.
test("other players' joins and moves are logged for the rig, never our own, never 15 a second", () => {
  const listeners = [];
  const client = { onGame: (fn) => (listeners.push(fn), () => listeners.splice(listeners.indexOf(fn), 1)) };
  const lines = [];
  const stop = sp.logOthers(client, 9, (s) => lines.push(s));
  const emit = (g) => listeners.forEach((fn) => fn(g));
  emit({ type: "game_player_joined", player_id: 4, name: "Rig", position: [53.5, 1.7, 40.5] });
  emit({ type: "game_position_update", player_id: 4, position: [53.5, 1.7, 40.5] });
  emit({ type: "game_position_update", player_id: 4, position: [53.6, 1.7, 40.5] }); // 0.1 m: not logged
  emit({ type: "game_position_update", player_id: 4, position: [53.5, 1.7, 41.5] }); // 1 m: logged
  emit({ type: "game_position_update", player_id: 9, position: [1, 2, 3] }); // our own: never
  emit({ type: "game_player_joined", player_id: 9, name: "Me", position: [1, 2, 3] });
  emit({ type: "game_player_left", player_id: 4 });
  stop();
  emit({ type: "game_player_left", player_id: 5 });
  assert.deepEqual(lines, [
    'player joined: entity 4 "Rig" at (53.50, 1.70, 40.50)',
    "saw entity 4 at (53.50, 1.70, 40.50)",
    "saw entity 4 at (53.50, 1.70, 41.50)",
    "player left: entity 4",
  ]);
});

// Ship homes increment 2, "Meet in the Commons": the walker walks from its own
// door, through its corridor, into the Commons, along a route the rig reads off
// the game's door points (`--route`), at a pace of its own (`--route-speed`),
// and then its path. Seen red 2026-10-04 with makeWalk ignoring plan.route (the
// 1b walker, which went straight to the path in about 4 s): "a step of the
// route is the route's pace: 0.580 m in 1/15 s".
test("a route is walked point by point, at its own pace, then the path", () => {
  // p1's door, its corridor's two steps, the Commons; then a line along x.
  const route = [[54, 1.7, 40], [66, 1.7, 40], [67, 1.7, 64]];
  const plan = { path: "line", axis: "x", center: [76, 1.7, 70], radius: 4, speed: 1.4, route, routeSpeed: 3 };
  const door = [53.5, 1.7, 40.5];
  const walk = sp.makeWalk(plan, door, 0);
  assert.equal(walk.onPath(), false, "a route is walked before the path, even from its first point");
  const want = [...route, sp.pathPoint(plan, 0)];
  const lenOf = (pts) => pts.slice(1).reduce((a, p, i) => a + dist(p, pts[i]), 0);
  const track = [door];
  let now = 0;
  let reached = null;
  for (let i = 0; i < 2000 && reached === null; i++) {
    now += 1 / 15;
    const { msg, reachedPath } = walk.next(now);
    const step = dist(msg.position, track[track.length - 1]);
    assert.ok(step <= 3 / 15 + 1e-9, `a step of the route is the route's pace: ${step.toFixed(3)} m in 1/15 s`);
    track.push(msg.position);
    if (reachedPath) reached = now;
  }
  assert.ok(reached !== null && walk.onPath(), "it reaches the path");
  // Through every point of the route, in order: a step that reaches a point walks on past it,
  // so the walk is drawn within one step (0.2 m at 3 m/s and 15 updates a second) of each.
  let from = 0;
  want.forEach((p, k) => {
    let best = Infinity;
    let at = -1;
    for (let i = from; i < track.length; i++) {
      const d = dist(track[i], p);
      if (d < best) [best, at] = [d, i];
    }
    assert.ok(best <= 3 / 15 + 1e-9, `the walk passes through the route's point ${k + 1} of ${want.length} (${best.toFixed(2)} m away at the closest)`);
    from = at;
  });
  assert.ok(Math.abs(walk.approachM - lenOf([door, ...want])) < 1e-9, `the approach is the route's length (${walk.approachM})`);
  // At the route's pace: its length over 3 m/s, to a step.
  assert.ok(Math.abs(reached - walk.approachM / 3) < 1 / 15 + 1e-9, `the route took ${reached.toFixed(2)} s for ${walk.approachM.toFixed(1)} m at 3 m/s`);
  // Then the path, at the walking pace.
  const a = walk.next(now + 1 / 15).msg.position;
  const b = walk.next(now + 2 / 15).msg.position;
  assert.ok(Math.abs(dist(a, b) - 1.4 / 15) < 1e-9, "the path is walked at the walking speed");
  // And a route after a long pause still never steps more than MAX_STEP_M.
  const paused = sp.makeWalk({ ...plan, routeSpeed: 50 }, door, 0);
  const s1 = paused.next(60).msg.position;
  assert.ok(dist(s1, door) <= sp.MAX_STEP_M + 1e-9, `after a pause the step along the route is ${dist(s1, door).toFixed(1)} m`);
});

// Increment 4: the relay corrects a walk faster than anyone can go (25 m/s on foot), so the
// walker's approach, which used to cover any distance in 4 s, keeps under MAX_SPEED_MPS: from 1 km
// away every step is at most MAX_SPEED_MPS / 15. Red check run 2026-10-04: the cap taken out of
// `approachSpeed` (the 1b code, `Math.max(plan.speed, gap / APPROACH_SECONDS)`): FAILED at "a step
// of the approach went 251.0 m/s, over the walker's 20 m/s".
test("no walk outruns the relay's speed check", () => {
  assert.ok(sp.MAX_SPEED_MPS < ON_FOOT_MPS, `the walker's ${sp.MAX_SPEED_MPS} m/s is under the relay's ${ON_FOOT_MPS} m/s on foot`);
  const plan = { path: "line", axis: "x", center: [0, 1.7, 0], radius: 4, speed: 1.4 };
  const far = sp.makeWalk(plan, [1000, 1.7, 0], 0);
  let prev = far.position();
  for (let i = 1; i <= 300 && !far.onPath(); i++) {
    const { msg } = far.next(i / 15);
    const mps = dist(msg.position, prev) * 15;
    assert.ok(mps <= sp.MAX_SPEED_MPS + 1e-6, `a step of the approach went ${mps.toFixed(1)} m/s, over the walker's ${sp.MAX_SPEED_MPS} m/s`);
    prev = msg.position;
  }
});

// Increment 4: a correction from the relay (game_position_correction) stands the walker where the
// relay holds it; every update after it carries the correction's number (`correction`), so the
// relay takes them; and the walk goes on, back to the path, from there. An older correction than
// the last one taken is ignored. Red check run 2026-10-04: `update` without its `correction`
// field: FAILED at its first check, "no correction yet" (the field was undefined, not 0).
test("a correction stands the walker where the relay holds it", () => {
  const plan = { path: "line", axis: "x", center: [76, 1.7, 70], radius: 4, speed: 1.4 };
  const walk = sp.makeWalk(plan, sp.pathPoint(plan, 0), 0);
  for (let i = 1; i <= 30; i++) walk.next(i / 15);
  assert.equal(walk.next(31 / 15).msg.correction, 0, "no correction yet");
  const held = [70, 1.7, 60];
  assert.ok(walk.corrected(1, held), "a correction is taken");
  assert.deepEqual(walk.position(), held, "the walker stands where the relay holds it");
  const after = walk.next(32 / 15).msg;
  assert.equal(after.correction, 1, `the update after the correction says it stood there (${after.correction})`);
  assert.ok(dist(after.position, held) <= sp.MAX_SPEED_MPS / 15 + 1e-9, "and walks on from there");
  assert.equal(walk.corrected(1, [0, 0, 0]), false, "the same correction twice is taken once");
  // Back to the path, never faster than the walker goes.
  let prev = after.position;
  for (let i = 33; i < 400 && !walk.onPath(); i++) {
    const p = walk.next(i / 15).msg.position;
    assert.ok(dist(p, prev) * 15 <= sp.MAX_SPEED_MPS + 1e-6);
    prev = p;
  }
  assert.ok(walk.onPath(), "it reaches the path again");
});

// THE WALKER'S CORRECTION LINE IS THE ONE THE RIG COUNTS (the review of increment 4, R5). A
// correction goes through the walker's own handler (`onCorrection`, what its main() listens with):
// it logs one line, which the rig's pattern finds with every part (scripts/verify-copresence.js
// counts `walker_corrections` with this same CORRECTED_RE), and the walk is told to stand where
// the relay holds it. Before, the rig matched its own /corrected to/ and nothing tied it to the
// line or the handler. Red checks run 2026-10-04, each restored afterwards: the line reworded
// FAILED "the rig's pattern finds the walker's line: the relay put me back at (76.00, 1.70,
// 64.25) (too_fast, number 2)"; the handler without `walk.corrected` FAILED "the walk was told to
// stand where the relay holds it".
test("a correction is logged in the line the rig counts, and the walk stands where held", () => {
  const lines = [];
  const told = [];
  const walk = { corrected: (seq, at) => told.push([seq, at]) };
  const g = { type: "game_position_correction", position: [76, 1.7, 64.25], seq: 2, reason: "too_fast" };
  assert.ok(sp.onCorrection(g, walk, (s) => lines.push(s)), "a correction is handled");
  assert.equal(lines.length, 1, "one line");
  const m = lines[0].match(sp.CORRECTED_RE);
  assert.ok(m, `the rig's pattern finds the walker's line: ${lines[0]}`);
  assert.deepEqual([Number(m[1]), Number(m[2]), Number(m[3]), m[4], Number(m[5])], [76, 1.7, 64.25, "too_fast", 2]);
  assert.deepEqual(told, [[2, [76, 1.7, 64.25]]], "the walk was told to stand where the relay holds it");
  assert.equal(sp.onCorrection({ type: "game_position_update", position: [1, 2, 3] }, walk, (s) => lines.push(s)), false, "anything else is not a correction");
  assert.equal(lines.length, 1);
  // The rig counts the walkers' corrections with exactly this pattern.
  const rig = fs.readFileSync(path.join(__dirname, "..", "verify-copresence.js"), "utf8");
  assert.match(rig, /walker_corrections = walkerOut\.filter\(\(o\) => CORRECTED_RE\.test\(o\.line\)\)/, "verify-copresence.js counts the walkers' corrections with CORRECTED_RE");
  assert.match(rig, /const \{ CORRECTED_RE \} = require\("\.\/second-player\.js"\);/, "and takes it from the walker's own module");
});

// The walker names its door like a desktop player with that home, so the relay
// spawns it there (increment 2), and the new options read cleanly or refuse.
test("--route, --route-speed and --home-spawn, and the door named in the join", () => {
  const ship = { id: "mothership-1", hash: "0123456789abcdef" };
  const join = sp.joinMessage("TestBotWalker", {}, ship, [53.5, 40.5]);
  assert.deepEqual(join.home_spawn, [53.5, 40.5], "the join names our door");
  assert.equal(sp.joinMessage("TestBotWalker", {}, null, [53.5, 40.5]).home_spawn, undefined, "a guest's join names no door");
  const o = sp.parseOptions(["--route", "54,1.7,40; 66,1.7,40", "--route-speed", "3", "--home-spawn", "53.5,40.5"]);
  assert.deepEqual(o.route, [[54, 1.7, 40], [66, 1.7, 40]]);
  assert.equal(o.routeSpeed, 3);
  assert.deepEqual(o.homeSpawn, [53.5, 40.5]);
  const plain = sp.parseOptions([]);
  assert.deepEqual([plain.route, plain.routeSpeed, plain.homeSpawn], [[], null, null], "none given: no route, no door");
  assert.throws(() => sp.parseOptions(["--route", "1,2"]), /--route must be points/);
  assert.throws(() => sp.parseOptions(["--home-spawn", "1"]), /--home-spawn must be two numbers/);
  assert.throws(() => sp.parseOptions(["--route-speed", "0"]), /--route-speed must be/);
});

// The guest order fills a twelve-plot ship with households (2026-10-04), each from an address of
// its own: the relay signs up at most five new accounts an hour from one, read from the header
// nginx writes. The option takes an address and nothing else, since it goes into a header as it
// stands. Seen red 2026-10-04 before the option existed: "unknown option \"--forwarded-for\"
// (try --help)".
test("--forwarded-for takes an address and only an address", () => {
  assert.equal(sp.parseOptions(["--forwarded-for", "10.77.0.3"]).forwardedFor, "10.77.0.3");
  assert.equal(sp.parseOptions(["--forwarded-for", "::1"]).forwardedFor, "::1");
  assert.equal(sp.parseOptions([]).forwardedFor, null, "none given: no header");
  for (const bad of ["10.0.0.1\r\nX-Evil: 1", "not an address", "10.0.0.1, 10.0.0.2", ""]) {
    assert.throws(() => sp.parseOptions(["--forwarded-for", bad]), /--forwarded-for must be an address/, JSON.stringify(bad));
  }
});

// ── Building in the shared world (ship homes increment 5, 2026-10-05) ──────────────────────────
// verify-copresence --build has scripted players build and take down pieces through the relay, so
// the walker sends `game_build` / `game_unbuild` exactly as the contract has them
// (src/systems/construction/shared.rs `ToRelay`), with a pose the relay's own checks pass: on the
// metre grid, a whole quarter turn as the game's placement makes it, and the blueprint's own size.

const REPO = path.join(__dirname, "..", "..");
const SHARED_RS = fs.readFileSync(path.join(REPO, "src", "systems", "construction", "shared.rs"), "utf8");
/** The field names of one variant of the wire enums in shared.rs, by its serde type string. */
function contractFields(typeName) {
  const at = SHARED_RS.indexOf(`#[serde(rename = "${typeName}")]`);
  assert.ok(at >= 0, `shared.rs names a ${typeName} message`);
  const open = SHARED_RS.indexOf("{", at);
  const close = SHARED_RS.indexOf("}", open);
  // Field names at the start of a line inside the variant's braces (doc comments and serde
  // attributes are skipped by the pattern).
  return [...SHARED_RS.slice(open + 1, close).matchAll(/^\s*([a-z_]+):/gm)].map((m) => m[1]);
}

// The walker's input commands (a rig writes them, one a line) and the messages they send.
// Red first, 2026-10-05, before the commands existed: "TypeError: sp.parseCommand is not a function".
test("build and unbuild commands parse", () => {
  assert.deepEqual(sp.parseCommand("build wood_foundation@plot:p1:48,0,36,0"), {
    kind: "build",
    blueprint: "wood_foundation",
    frame: "plot:p1",
    local: [48, 0, 36],
    turns: 0,
    permit: null,
  });
  const wall = sp.parseCommand("  build wood_wall@zone:commons:11,0.2,50,1  ");
  assert.deepEqual([wall.frame, wall.local, wall.turns], ["zone:commons", [11, 0.2, 50], 1]);
  assert.deepEqual(sp.parseCommand("unbuild 12"), { kind: "unbuild", pieceId: 12, permit: null });
  assert.deepEqual(sp.parseCommand("stop"), { kind: "stop" });
  assert.equal(sp.parseCommand("   "), null, "an empty line is no command");
  assert.deepEqual(sp.parseCommand("permit p1 did:hum:4dQe1bVHyiHm1Vh8rWbx2F 30"), { kind: "permit", plot: "p1", grantee: "did:hum:4dQe1bVHyiHm1Vh8rWbx2F", days: 30 });
  // A permit rides along as the JSON the walker that minted it logged.
  const permit = { issuer: "ab12", server: "did:hum:srv", plot: "p1", grantee: "did:hum:x", expiry: 1900000000, sig: "c2ln" };
  assert.deepEqual(sp.parseCommand(`build wood_wall@plot:p1:46,0.2,36,1 permit ${JSON.stringify(permit)}`).permit, permit);
  assert.deepEqual(sp.parseCommand(`unbuild 7 permit ${JSON.stringify(permit)}`), { kind: "unbuild", pieceId: 7, permit });
  for (const [bad, why] of [
    ["build wood_wall@plot:p1:46,0.2,36", /build wants/],
    ["build wood_wall@plot:p1:46,0.2,36,4", /turns must be 0, 1, 2 or 3/],
    ["build wood_wall@plot:p1:46,x,36,1", /build wants/],
    ["build Wood Wall@plot:p1:46,0,36,1", /build wants/],
    ["build wood_wall@site:moon:1,0,1,0", /frame must be plot:<id> or zone:<id>/],
    ["build wood_wall@p1:1,0,1,0", /frame must be plot:<id> or zone:<id>/],
    ["unbuild x", /unbuild wants a piece number/],
    ["unbuild 0", /unbuild wants a piece number/],
    ["build wood_wall@plot:p1:1,0,1,0 permit {not json", /permit must be/],
    ['build wood_wall@plot:p1:1,0,1,0 permit {"issuer":"ab"}', /permit must be/],
    // A permit that names no server (the words before Wave 0's 6e0174cd7) is not one.
    [`unbuild 7 permit ${JSON.stringify({ ...permit, server: undefined })}`, /permit must be/],
    ["permit plot:p1 did:hum:x 30", /permit wants/],
    ["permit p1 did:hum:x 91", /at most 90 days/],
    ["dance", /unknown command/],
  ]) {
    assert.throws(() => sp.parseCommand(bad), why, bad);
  }

  // The messages: exactly the contract's fields (shared.rs ToRelay), the pose as given, the turn
  // as the game's placement makes it, the size the blueprint's own.
  const blueprints = sp.readBlueprints(fs.readFileSync(path.join(REPO, "data", "blueprints", "basic.ron"), "utf8"));
  const msg = sp.buildMessage(sp.parseCommand("build wood_wall@plot:p1:46,0.2,36,1"), 7, blueprints);
  assert.deepEqual(Object.keys(msg).filter((k) => k !== "type").sort(), contractFields("game_build").filter((k) => k !== "permit").sort());
  assert.equal(msg.type, "game_build");
  assert.equal(msg.req_id, 7);
  assert.equal(msg.blueprint_id, "wood_wall");
  assert.deepEqual(msg.position, [46, 0.2, 36]);
  assert.deepEqual(msg.rotation, sp.quarterTurn(1));
  assert.deepEqual(msg.scale, [4, 3, 0.2], "the wall's own size, read from basic.ron");
  assert.deepEqual(sp.buildMessage(sp.parseCommand(`build wood_wall@plot:p1:46,0.2,36,1 permit ${JSON.stringify(permit)}`), 8, blueprints).permit, permit);
  assert.throws(() => sp.buildMessage(sp.parseCommand("build moon_base@plot:p1:1,0,1,0"), 9, blueprints), /no blueprint "moon_base"/);
  const un = sp.unbuildMessage(sp.parseCommand("unbuild 12"), 10);
  assert.deepEqual(Object.keys(un).filter((k) => k !== "type").sort(), contractFields("game_unbuild").filter((k) => k !== "permit").sort());
  assert.deepEqual(un, { type: "game_unbuild", req_id: 10, piece_id: 12 });
});

// THE TURN IS THE GAME'S OWN (src/systems/construction/placement.rs `quarter_turn`): the relay
// refuses a turn more than half a degree off a whole quarter, and keeps the one the placement
// makes, so a scripted build turned the other way round would come back as a different turn.
// Red first, 2026-10-05, before the function existed: "TypeError: sp.quarterTurn is not a function".
test("quarter turns match placement::quarter_turn", () => {
  const src = fs.readFileSync(path.join(REPO, "src", "systems", "construction", "placement.rs"), "utf8");
  assert.match(
    src,
    /pub fn quarter_turn\(quarter_turns: u8\) -> Quat \{\s*Quat::from_rotation_y\(f32::from\(quarter_turns % 4\) \* std::f32::consts::FRAC_PI_2\)\s*\}/,
    "placement::quarter_turn is still a turn of (quarter_turns % 4) x 90 degrees about +Y; if it changed, change sp.quarterTurn with it",
  );
  // glam's Quat::from_rotation_y(a) is (0, sin(a/2), 0, cos(a/2)), all in f32.
  const f = Math.fround;
  for (let t = 0; t < 8; t++) {
    const a = f(f(t % 4) * f(Math.PI / 2));
    const want = [0, f(Math.sin(f(a * 0.5))), 0, f(Math.cos(f(a * 0.5)))];
    const got = sp.quarterTurn(t);
    assert.equal(got.length, 4);
    got.forEach((v, i) => assert.ok(Math.abs(v - want[i]) <= 1e-7, `quarterTurn(${t})[${i}] = ${v}, glam gives ${want[i]}`));
    assert.equal(sp.turnOf(got), t % 4, `turnOf reads quarterTurn(${t}) back as ${t % 4}`);
    assert.equal(sp.turnOf(got.map((v) => -v)), t % 4, "and the same turn written the other sign round");
  }
  // The way glam turns things: a quarter turn takes +X to -Z (a camera at yaw +90 degrees looks
  // along +X, renderer/camera.rs), so a wall turned once runs along z.
  const rotate = (q, v) => {
    const [x, y, z, w] = q;
    const t = [2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])];
    return [v[0] + w * t[0] + (y * t[2] - z * t[1]), v[1] + w * t[1] + (z * t[0] - x * t[2]), v[2] + w * t[2] + (x * t[1] - y * t[0])];
  };
  const xTurned = rotate(sp.quarterTurn(1), [1, 0, 0]);
  assert.ok(Math.abs(xTurned[0]) < 1e-6 && Math.abs(xTurned[2] + 1) < 1e-6, `one quarter turn takes +X to -Z, got ${xTurned}`);
});

// THE SIZE IS THE BLUEPRINT'S OWN (the relay refuses a footprint more than a millimetre off it, and
// a height levelling could not make): read from the data the relay reads, data/blueprints/basic.ron,
// never a copy in this script. Red first, 2026-10-05, before the reader existed:
// "TypeError: sp.readBlueprints is not a function".
test("sizes read from basic.ron", () => {
  const text = fs.readFileSync(path.join(REPO, "data", "blueprints", "basic.ron"), "utf8");
  const bps = sp.readBlueprints(text);
  const entries = text.split(/\r?\n/).filter((l) => !/^\s*\/\//.test(l) && l.includes('(id: "')).length;
  assert.equal(bps.size, entries, `every blueprint in basic.ron is read (${bps.size} of ${entries})`);
  const foundation = bps.get("wood_foundation");
  assert.deepEqual(foundation.size, [4, 0.2, 4]);
  assert.equal(foundation.build_time, 5);
  assert.deepEqual(foundation.materials, [["wood_plank_0", 8]]);
  assert.equal(foundation.name, "Wood Foundation");
  assert.equal(foundation.shared, true);
  assert.deepEqual(bps.get("wood_wall").size, [4, 3, 0.2]);
  assert.deepEqual(bps.get("wood_wall_window").materials, [["wood_plank_0", 7], ["glass_pane_0", 1]]);
  assert.equal(bps.get("campfire").shared, false, "a piece with no shared field is not shared");
  // Every blueprint the file marks shared, and only those.
  const sharedInText = [...text.matchAll(/\(id: "([a-z0-9_]+)"[^\n]*\bshared: true/g)].map((m) => m[1]).sort();
  assert.deepEqual([...bps.values()].filter((b) => b.shared).map((b) => b.id).sort(), sharedInText);
  assert.equal(sharedInText.length, 7, "the seven shell pieces of increment 5");
  // A comment never counts as a blueprint, whatever brackets it holds.
  const tricky = sp.readBlueprints('[\n  // (id: "ghost", size: (9.0, 9.0, 9.0)) and a ) or two (\n  (id: "a", name: "A (x)", size: (1.0, 2.0, 3.0), build_time: 2.5, materials: [("p", 2)], shared: true), // (id: "b")\n]');
  assert.deepEqual([...tricky.keys()], ["a"]);
  assert.deepEqual(tricky.get("a").size, [1, 2, 3]);
  assert.equal(tricky.get("a").name, "A (x)");
});

// THE PERMIT'S SIGNED WORDS ARE THE RELAY'S (src/relay/core/pq_crypto.rs `plot_permit_preimage`),
// read from the Rust test that pins them, so a change there fails here until this script follows.
// A permit the walker mints verifies with the issuer's key over those words, names the server it
// is good on (the relay's own did:hum, /api/server-info `server_did`), and its fields are the
// contract's (shared.rs `Permit`). Red first, 2026-10-05, before the function existed:
// "TypeError: sp.permitPreimage is not a function". Red again the same day, when Wave 0's
// 6e0174cd7 put the server into the relay's words, against the three-field preimage:
//   AssertionError [ERR_ASSERTION]: permitPreimage("did:hum:srv", "p3", "did:hum:abc", 0)
//     actual: 'hum/permit/v1\ndid:hum:srv\np3\ndid:hum:abc',
//     expected: 'hum/permit/v1\ndid:hum:srv\np3\ndid:hum:abc\n0'
test("permitPreimage equals the Rust KAT string", async () => {
  const rust = fs.readFileSync(path.join(REPO, "src", "relay", "core", "pq_crypto.rs"), "utf8");
  const pins = [...rust.matchAll(/assert_eq!\(\s*plot_permit_preimage\(([^)]*)\)\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)/g)];
  assert.ok(pins.length >= 1, "pq_crypto.rs pins plot_permit_preimage with a literal");
  const unescape = (s) => s.replace(/\\(.)/g, (_, c) => ({ n: "\n", t: "\t", r: "\r", "\\": "\\", '"': '"' })[c] ?? c);
  for (const [, argText, expected] of pins) {
    const args = argText.split(",").map((a) => a.trim()).map((a) => (a.startsWith('"') ? unescape(a.slice(1, -1)) : Number(a)));
    assert.ok(args.every((a) => typeof a === "string" || Number.isFinite(a)), `the pinned call's arguments are literals: ${argText}`);
    assert.equal(sp.permitPreimage(...args), unescape(expected), `permitPreimage(${argText})`);
  }
  // The fields a permit carries on the wire.
  const at = SHARED_RS.indexOf("pub struct Permit {");
  const fields = [...SHARED_RS.slice(at, SHARED_RS.indexOf("\n}", at)).matchAll(/^\s*pub ([a-z_]+):/gm)].map((m) => m[1]);
  const noble = await sp.loadNoble();
  const issuer = sp.deriveIdentity(noble, sp.masterSeedFrom(noble, "permit issuer", "TestBotIssuer"));
  const expiry = 1900000000;
  const server = "did:hum:7Xq1server2DidHere3Abc";
  const permit = sp.mintPermit(issuer, { server, plot: "p1", grantee: "did:hum:4dQe1bVHyiHm1Vh8rWbx2F", expiry });
  assert.deepEqual(Object.keys(permit).sort(), fields.sort(), "the permit's fields are the contract's");
  assert.equal(permit.issuer, issuer.publicKeyHex);
  assert.equal(permit.server, server);
  const words = (srv, plot) => new TextEncoder().encode(sp.permitPreimage(srv, plot, permit.grantee, expiry));
  const pk = new Uint8Array(Buffer.from(issuer.publicKeyHex, "hex"));
  const sig = new Uint8Array(Buffer.from(permit.sig, "base64"));
  assert.ok(noble.ml_dsa65.verify(sig, words(server, "p1"), pk), "the issuer's signature over the permit's words");
  assert.ok(!noble.ml_dsa65.verify(sig, words(server, "p2"), pk), "and not over another plot's");
  assert.ok(!noble.ml_dsa65.verify(sig, words("did:hum:another", "p1"), pk), "nor over the same plot on another server");
  assert.throws(() => sp.mintPermit(issuer, { plot: "p1", grantee: permit.grantee, expiry }), /needs the server's did:hum/, "no permit without the server it is good on");
  // The server's did:hum, read where the relay shows it (/api/server-info `server_did`), from a
  // stand-in served by this process (no relay booted).
  const http = require("node:http");
  let answer = { name: "x", server_did: server };
  const stand = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(req.url === "/api/server-info" ? JSON.stringify(answer) : "{}");
  });
  await new Promise((r) => stand.listen(0, "127.0.0.1", r));
  try {
    const url = `ws://127.0.0.1:${stand.address().port}/ws`;
    assert.equal(await sp.fetchServerDid(url), server, "the relay's own did:hum");
    answer = { name: "x", server_did: "" };
    assert.equal(await sp.fetchServerDid(url), null, "a relay that names none");
    answer = { name: "x" };
    assert.equal(await sp.fetchServerDid(url), null, "a relay from before servers had a DID");
  } finally {
    stand.close();
  }
  // The walker's lines a rig reads for the two ids (its own, the server's).
  assert.equal(`second-player: identity: ${server}`.match(sp.DID_RE)[1], server);
  assert.equal(`second-player: server: ${server}`.match(sp.SERVER_DID_RE)[1], server);
  assert.equal(`second-player: server: ${server}`.match(sp.DID_RE), null, "the server's line is never read as the walker's own");
  // The grantee is named by its did:hum, as the relay holds plots (relay/core/did.rs: base58 of the
  // first 16 bytes of BLAKE3 of the Dilithium key, Bitcoin's alphabet).
  assert.equal(sp.base58(Buffer.from("hello world")), "StV1DL6CwTryKyV", "Bitcoin's base58");
  assert.equal(sp.base58([0, 0, 1]), "112", "each leading zero byte is a 1");
  const did = sp.didFor(noble, issuer.publicKeyHex);
  assert.match(did, /^did:hum:[1-9A-HJ-NP-Za-km-z]{11,22}$/);
  const fp = noble.blake3(new Uint8Array(Buffer.from(issuer.publicKeyHex, "hex"))).slice(0, 16);
  assert.equal(did, `did:hum:${sp.base58(fp)}`);
});

// THE LINES A RIG READS (verify-copresence --build): what the relay told this walker about pieces,
// one line each, found with the patterns this module exports, so the rig and the walker share one
// copy of each (as CORRECTED_RE does). Red first, 2026-10-05, before the logger existed:
// "TypeError: sp.logBuilds is not a function".
test("the build lines are logged in the patterns the rig reads", () => {
  const listeners = [];
  const client = { onGame: (fn) => (listeners.push(fn), () => listeners.splice(listeners.indexOf(fn), 1)) };
  const lines = [];
  const stop = sp.logBuilds(client, (s) => lines.push(`second-player: ${s}`));
  const emit = (g) => listeners.forEach((fn) => fn(g));
  const piece = (id, extra = {}) => ({ piece_id: id, blueprint_id: "wood_wall", position: [46, 0.2, 36], rotation: sp.quarterTurn(1), scale: [4, 3, 0.2], placed_at: 1759680000.25, ...extra });
  emit({ type: "game_built", frame: "plot:p1", seq: 3, server_time: 1759680000.3, piece: piece(7, { mine: true }), req_id: 2 });
  emit({ type: "game_built", frame: "plot:p2", seq: 1, server_time: 1759680001, piece: piece(9) });
  emit({ type: "game_unbuilt", frame: "plot:p2", seq: 2, piece_id: 9 });
  emit({ type: "game_unbuilt", frame: "plot:p1", seq: 4, piece_id: 7, req_id: 3 });
  emit({ type: "game_build_refused", req_id: 4, action: "build", reason: "not_allowed", why: "not_your_plot", message: "Wood Wall not built: this is someone else's plot." });
  emit({ type: "game_build_refused", req_id: 5, action: "unbuild", reason: "no_such_piece", message: "Piece not taken down: it is already gone." });
  emit({ type: "game_pieces", frame: "zone:commons", seq: 6, server_time: 1759680002, part: 1, parts: 1, pieces: [piece(11, { mine: true })] });
  emit({ type: "game_frame_out_of_view", frame: "plot:p4" });
  stop();
  emit({ type: "game_unbuilt", frame: "plot:p1", seq: 5, piece_id: 8 });
  const find = (re) => lines.map((l) => l.match(re)).filter(Boolean);
  const built = find(sp.BUILT_RE);
  assert.equal(built.length, 1, `one own build: ${lines.join(" | ")}`);
  assert.deepEqual(built[0].slice(1), ["7", "wood_wall", "plot:p1", "46.000", "0.200", "36.000", "1", "2", "3", "1759680000.250"]);
  const saw = find(sp.SAW_BUILT_RE);
  assert.deepEqual(saw.map((m) => m.slice(1)), [["9", "wood_wall", "plot:p2", "46.000", "0.200", "36.000", "1", "1"]], "someone else's build is a saw line, never a built one");
  assert.deepEqual(find(sp.SAW_UNBUILT_RE).map((m) => m.slice(1)), [["9", "plot:p2", "2"]]);
  assert.deepEqual(find(sp.TOOK_DOWN_RE).map((m) => m.slice(1)), [["7", "plot:p1", "3", "4"]]);
  const refused = find(sp.REFUSED_RE).map((m) => m.slice(1, 5));
  assert.deepEqual(refused, [["build", "not_allowed", "not_your_plot", "4"], ["unbuild", "no_such_piece", undefined, "5"]]);
  assert.deepEqual(find(sp.SNAPSHOT_PIECE_RE).map((m) => [m[1], m[3], m[8]]), [["11", "zone:commons", " (mine)"]]);
  assert.ok(lines.some((l) => l.includes("frame out of view: plot:p4")));
  assert.ok(!lines.some((l) => l.includes("piece 8")), "nothing after the logger stopped");
  // The welcome's ranks, as the walker logs them on joining.
  assert.equal(sp.ranksLine({ ranks: { can_edit_ship: true, take_down_any: false } }), 'ranks {"can_edit_ship":true,"take_down_any":false}');
  assert.match(sp.ranksLine({}), /^ranks missing/);
  assert.deepEqual(JSON.parse(sp.ranksLine({ ranks: { can_edit_ship: true } }).match(sp.RANKS_RE)[1]), { can_edit_ship: true });
});

// THE WALKER GOES AT THE GAME'S PACE (verify-copresence --build). The relay takes one build and one
// take-down from a player each PERCEPTION_MIN_INTERVAL_MS (src/relay/handlers/msg_handlers.rs
// `perception_rate_allows`) and answers one sooner `rate_limited`, nothing kept. The first --build
// run lost the builder's wall that way (2026-10-05): the rig asks for the wall the moment the
// foundation is kept, the walker sent it a few milliseconds later, and the refusal was the answer
// the rig read, so four checks fell behind it. The walker now does what the game does
// (src/engine/shared_build.rs `send_next`, the contract's SEND_INTERVAL_MS and
// RATE_LIMITED_RETRY_MS): at most one request every 250 ms, and one turned away for pace sent
// again, the same req_id, 500 ms later, that refusal never logged as its answer.
//
// A stand-in relay keeps the relay's own pace rule on a made-up clock, answering each request when
// it gets to it (a little later than it was sent, sometimes later still), and the rig's own reading
// of the walker's lines is done on what the walker logged (verify-copresence.js `command`: the
// first "sent" line after the command; `answer`: the first built, took-down or refused line naming
// that req_id).
//
// Seen red 2026-10-05 before the walker was paced (`makeRequests` sending each request the moment
// it was asked, as main() did):
//   AssertionError [ERR_ASSERTION]: the wall asked for the moment the foundation was kept: the rig
//   reads req 2's answer as "second-player: build refused: rate_limited (req 2): Piece not built:
//   too much was asked at once; it goes again in a moment."
test("builds and take-downs go at the game's pace, and one turned away for pace goes again", () => {
  const handlers = fs.readFileSync(path.join(REPO, "src", "relay", "handlers", "msg_handlers.rs"), "utf8");
  const RELAY_MS = Number(handlers.match(/const PERCEPTION_MIN_INTERVAL_MS: u64 = (\d+);/)[1]);

  // A made-up clock, and timers on it (run in time order; two due together in the order set).
  const clock = { t: 0, timers: [], n: 0 };
  clock.now = () => clock.t;
  clock.setTimer = (fn, ms) => {
    const h = { at: clock.t + Math.max(0, ms), fn, n: clock.n++ };
    clock.timers.push(h);
    return h;
  };
  clock.clearTimer = (h) => {
    const i = clock.timers.indexOf(h);
    if (i >= 0) clock.timers.splice(i, 1);
  };
  clock.run = (until) => {
    for (;;) {
      clock.timers.sort((a, b) => a.at - b.at || a.n - b.n);
      const h = clock.timers[0];
      if (!h || h.at > until) break;
      clock.timers.shift();
      clock.t = Math.max(clock.t, h.at);
      h.fn();
    }
    clock.t = Math.max(clock.t, until);
  };

  // The stand-in relay: it gets to each request `lag(message)` ms after it was sent, takes it when
  // the last one of its kind it took was at least RELAY_MS before (its own clock), else answers
  // `rate_limited` and remembers nothing; `refuse(message)` may turn one away with another reason.
  // Every message sent is kept, with when.
  const stand = { lag: () => 2, refuse: () => null, sent: [], listeners: [], last: new Map(), pieces: 0 };
  const reply = (g) => clock.setTimer(() => stand.listeners.slice().forEach((fn) => fn(g)), 1);
  const handle = (m) => {
    const action = m.type === "game_build" ? "build" : "unbuild";
    const prev = stand.last.get(action);
    const said = stand.refuse(m) || (prev !== undefined && clock.now() - prev < RELAY_MS ? "rate_limited" : null);
    if (said) {
      return reply({ type: "game_build_refused", req_id: m.req_id, action, reason: said, message: "Piece not built: too much was asked at once; it goes again in a moment." });
    }
    stand.last.set(action, clock.now());
    const at = 1759680000 + clock.now() / 1000;
    if (action === "build") {
      stand.pieces += 1;
      reply({ type: "game_built", frame: m.frame, seq: stand.pieces, server_time: at, req_id: m.req_id, piece: { piece_id: stand.pieces, blueprint_id: m.blueprint_id, position: m.position, rotation: m.rotation, scale: m.scale, placed_at: at, mine: true } });
    } else {
      reply({ type: "game_unbuilt", frame: "plot:p1", seq: 99, piece_id: m.piece_id, req_id: m.req_id });
    }
  };
  const client = {
    onGame: (fn) => (stand.listeners.push(fn), () => stand.listeners.splice(stand.listeners.indexOf(fn), 1)),
    send: (m) => {
      const copy = JSON.parse(JSON.stringify(m));
      stand.sent.push({ at: clock.now(), m: copy });
      clock.setTimer(() => handle(copy), stand.lag(copy));
    },
  };

  // The walker as main() wires it.
  const lines = [];
  const log = (s) => lines.push(`second-player: ${s}`);
  const requests = sp.makeRequests({ send: client.send, log, now: clock.now, setTimer: clock.setTimer, clearTimer: clock.clearTimer });
  sp.logBuilds(client, log, requests);
  const blueprints = sp.readBlueprints(fs.readFileSync(path.join(REPO, "data", "blueprints", "basic.ron"), "utf8"));

  // The rig: write a command, find its "sent" line, then the answer naming its req_id, within its
  // BUILD_ANSWER_MS (8 s), as verify-copresence.js `command` and `answer` read them.
  const answerOf = (ls, req) => {
    for (const l of ls) {
      let m = l.match(sp.BUILT_RE);
      if (m && Number(m[8]) === req) return { kind: "built", line: l };
      m = l.match(sp.TOOK_DOWN_RE);
      if (m && Number(m[3]) === req) return { kind: "took", line: l };
      m = l.match(sp.REFUSED_RE);
      if (m && m[4] !== "-" && Number(m[4]) === req) return { kind: "refused", reason: m[2], line: l };
    }
    return null;
  };
  const ask = (text) => {
    const mark = lines.length;
    const cmd = sp.parseCommand(text);
    if (cmd.kind === "build") requests.build(cmd, blueprints);
    else requests.unbuild(cmd);
    let req = null;
    let answer = null;
    for (const t0 = clock.now(); clock.now() - t0 <= 8000 && !answer; ) {
      const sent = lines.slice(mark).map((l) => l.match(sp.SENT_RE)).find(Boolean);
      if (sent) req = Number(sent[2]);
      if (req !== null) answer = answerOf(lines.slice(mark), req);
      if (!answer) clock.run(clock.now() + 5);
    }
    return { req, answer, said: answer ? `"${answer.line}"` : "nothing" };
  };
  const sendsOf = (req) => stand.sent.filter((s) => s.m.req_id === req);

  // 1. The rig's own sequence: the foundation, then the wall the moment the foundation is kept.
  const foundation = ask("build wood_foundation@plot:p1:48,0,36,0");
  assert.equal(foundation.answer && foundation.answer.kind, "built", `the foundation: the rig reads req ${foundation.req}'s answer as ${foundation.said}`);
  const wall = ask("build wood_wall@plot:p1:46,0.2,36,1");
  assert.equal(wall.answer && wall.answer.kind, "built", `the wall asked for the moment the foundation was kept: the rig reads req ${wall.req}'s answer as ${wall.said}`);
  assert.deepEqual([foundation.req, wall.req], [1, 2], "one req_id each, in order");

  // 2. The relay gets to a request late (a busy moment), so the next, sent 250 ms after it, reaches
  // it too soon: turned away for pace, it goes again 500 ms later, the same message, and is kept.
  // The rig reads the piece kept, never the refusal.
  stand.lag = (m) => (m.req_id === 3 ? 150 : 2);
  const late = ask("build wood_wall@plot:p1:50,0.2,36,1");
  assert.equal(late.answer && late.answer.kind, "built", `a request the relay got to late: ${late.said}`);
  const again = ask("build wood_wall@plot:p1:54,0.2,36,1");
  assert.equal(again.answer && again.answer.kind, "built", `the request after it, turned away for pace once: the rig reads req ${again.req}'s answer as ${again.said}`);
  const twice = sendsOf(again.req);
  assert.equal(twice.length, 2, `req ${again.req} went twice (once turned away for pace): ${twice.length}`);
  assert.deepEqual(twice[1].m, twice[0].m, "the same request again, its req_id and all");
  assert.ok(twice[1].at - twice[0].at >= sp.RATE_LIMITED_RETRY_MS, `sent again ${twice[1].at - twice[0].at} ms later, at least RATE_LIMITED_RETRY_MS (the relay's answer comes first)`);
  const too = lines.filter((l) => l.match(sp.TOO_SOON_RE));
  assert.deepEqual(too.map((l) => Number(l.match(sp.TOO_SOON_RE)[2])), [again.req], `the turn-away said in a line of its own: ${too.join(" | ")}`);
  assert.ok(!too.some((l) => sp.REFUSED_RE.test(l) || sp.SENT_RE.test(l)), "never in a line the rig reads as an answer or a first send");
  assert.equal(lines.filter((l) => (l.match(sp.RESENT_RE) || [])[2] === String(again.req)).length, 1, "and the second send said");

  // 3. A take-down goes at the same pace.
  stand.lag = () => 2;
  const down = ask("unbuild 2");
  assert.equal(down.answer && down.answer.kind, "took", `the take-down: ${down.said}`);

  // 4. A request the relay turns away for pace every time: after RATE_LIMITED_TRIES more sends, that
  // refusal is its answer, and nothing more goes.
  stand.refuse = (m) => (m.req_id === 6 ? "rate_limited" : null);
  const never = ask("build wood_wall@plot:p1:58,0.2,36,1");
  assert.equal(never.req, 6);
  assert.deepEqual([never.answer && never.answer.kind, never.answer && never.answer.reason], ["refused", "rate_limited"], `turned away every time: ${never.said}`);
  assert.equal(sendsOf(6).length, 1 + sp.RATE_LIMITED_TRIES, `sent once and again ${sp.RATE_LIMITED_TRIES} times`);
  stand.refuse = (m) => (m.req_id === 7 ? "not_allowed" : null);
  const no = ask("build wood_wall@plot:p1:62,0.2,36,1");
  assert.deepEqual([no.answer && no.answer.kind, no.answer && no.answer.reason], ["refused", "not_allowed"], `any other refusal is the answer at once: ${no.said}`);
  assert.equal(sendsOf(7).length, 1, "and is never sent again");
  clock.run(clock.now() + 5000);
  assert.equal(sendsOf(6).length + sendsOf(7).length, 1 + sp.RATE_LIMITED_TRIES + 1, "nothing more goes for either");

  // 5. Over the whole run: never two requests within SEND_INTERVAL_MS, and each request's first send
  // said once.
  const gaps = stand.sent.slice(1).map((s, i) => s.at - stand.sent[i].at);
  assert.ok(gaps.every((g) => g >= sp.SEND_INTERVAL_MS), `the gaps between sends: ${gaps.join(", ")} ms (at least ${sp.SEND_INTERVAL_MS})`);
  for (let req = 1; req <= 7; req++) {
    assert.equal(lines.filter((l) => (l.match(sp.SENT_RE) || [])[2] === String(req)).length, 1, `req ${req}'s "sent" line, once`);
  }

  // 6. Stopped (the walker leaving), nothing waiting goes: of two asked together, the first went at
  // once and the second never.
  const before = stand.sent.length;
  requests.build(sp.parseCommand("build wood_wall@plot:p1:66,0.2,36,1"), blueprints);
  requests.build(sp.parseCommand("build wood_wall@plot:p1:70,0.2,36,1"), blueprints);
  requests.stop();
  clock.run(clock.now() + 5000);
  assert.equal(stand.sent.length - before, 1, "after stop nothing more went");

  // The pace is the game's own, from the contract, and slower than the relay's.
  const pinned = (name) => Number((SHARED_RS.match(new RegExp(`pub const ${name}: u64 = (\\d+);`)) || [])[1]);
  assert.equal(sp.SEND_INTERVAL_MS, pinned("SEND_INTERVAL_MS"), "the walker's pace is the game's (shared.rs SEND_INTERVAL_MS)");
  assert.equal(sp.RATE_LIMITED_RETRY_MS, pinned("RATE_LIMITED_RETRY_MS"), "and so is its wait before sending again (shared.rs RATE_LIMITED_RETRY_MS)");
  assert.ok(sp.SEND_INTERVAL_MS > RELAY_MS, `${sp.SEND_INTERVAL_MS} ms is slower than the relay's ${RELAY_MS} ms`);
});

// A builder stands still where the relay put it (--path still): it never walks off its plot, and
// every update it sends is the same place, standing still. Red first, 2026-10-05: "--path must be
// circle or line".
test("--path still stands where the relay put it", () => {
  const o = sp.parseOptions(["--path", "still"]);
  assert.equal(o.path, "still");
  const welcome = { player_id: 9, home_plot: { id: "p1" }, world_snapshot: [{ entity_id: 9, entity_type: "player", position: [27.5, 1.7, 44.5] }, { entity_id: 4, entity_type: "player", position: [80, 1.7, 60] }] };
  const { center, start } = sp.chooseCenter({ ...o, center: "auto" }, welcome);
  assert.deepEqual(center, start);
  const guest = sp.chooseCenter({ ...o, center: "auto" }, { ...welcome, home_plot: null });
  assert.deepEqual(guest.center, [27.5, 1.7, 44.5], "a guest standing still stands where it was put, never beside someone else");
  const walk = sp.makeWalk({ path: "still", axis: "x", center, radius: 4, speed: 1.4 }, start, 0);
  for (let i = 1; i <= 30; i++) {
    const { msg } = walk.next(i / 15);
    assert.deepEqual(msg.position, start);
    assert.deepEqual(msg.velocity, [0, 0, 0]);
  }
});
