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
//  2. No single step is ever longer than 90 m, even after a long pause, so the
//     relay (which drops any update that moves more than 100 m) never drops
//     one.
//  3. The timestamp follows THE TIMESTAMP RULE (top of second-player.js): the
//     sender's own clock in SECONDS, moving on by exactly the time step the
//     walk used, so velocity = distance moved / stamp difference.
//  4. Its default name starts with TestBot (kept off the server's member
//     list), and a `--seed random` run without `--name` gets a name of its
//     own, so a second random run is not refused as "already registered".

const { test } = require("node:test");
const assert = require("node:assert");

const sp = require("../second-player.js");

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
test("no step is longer than 90 m, even after a long pause", () => {
  assert.ok(sp.MAX_STEP_M < 100, "the cap sits under the relay's 100 m limit");
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

  // The one log line the rig reads.
  assert.equal(sp.homePlotLine(welcome(p2)), `home_plot ${JSON.stringify(p2)}`);
  assert.match(sp.homePlotLine(welcome(null)), /^home_plot null /);
  assert.match(sp.homePlotLine({ type: "game_welcome" }), /^home_plot missing /);
});
