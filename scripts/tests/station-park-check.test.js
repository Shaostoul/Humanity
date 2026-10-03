// The station park check (scripts/lib/station-park-check.js), fed made-up
// done files. Pure node, no game: runs in `just rig-tests`.
//
// A check that cannot fail proves nothing, so most of this file is done files
// that MUST fail, starting with the two BUG-132 actually wrote: the first park
// after the Earth warm-up (the pose plus the Earth-to-home offset) and the
// re-park after the camera had already let go of the station (a few hundred
// metres to a kilometre out).

const { test } = require("node:test");
const assert = require("node:assert");
const { judgeStationPark, judgeStationCapture, parsePose5, STATION_POSE_TOL_M } = require("../lib/station-park-check.js");

// tests/visual/vantages.json home-overview-noon and home-racks-noon.
const OVERVIEW = { station: "home", pose: "27.5,26,96,0,-0.42", time: 20.15 };
const RACKS = { station: "home", pose: "6.9,1.5,58.6,-0.55,-0.14", time: 20.15 };
const CLOCK = { station: "home", time: 14.15 }; // home-clock-dawn: no pose

test("a pose is exactly five finite numbers", () => {
  assert.deepStrictEqual(parsePose5("6.9,1.5,58.6,-0.55,-0.14"), { pos: [6.9, 1.5, 58.6], yaw: -0.55, pitch: -0.14 });
  assert.strictEqual(parsePose5("1,2,3,4"), null);
  assert.strictEqual(parsePose5("1,2,x,4,5"), null);
  assert.strictEqual(parsePose5(undefined), null);
});

test("a park that landed on its pose passes", () => {
  const done = { ok: true, station: "home", position: [27.5, 26.0, 96.0], yaw_pitch: [0, -0.42], station_ride: true };
  const v = judgeStationPark(OVERVIEW, done);
  assert.strictEqual(v.judged, true);
  assert.strictEqual(v.ok, true, v.message);
});

test("f32 rounding of the pose is inside the tolerance", () => {
  const done = { ok: true, position: [6.9000001, 1.4999999, 58.600002], yaw_pitch: [-0.55, -0.14], station_ride: true };
  assert.strictEqual(judgeStationPark(RACKS, done).ok, true);
});

test("BUG-132 first pass: pose plus the Earth-to-home offset FAILS", () => {
  // The exact position run.log printed for home-overview-noon on v0.1442.0.
  const done = { ok: true, station: "home", position: [32629394.0, -2489632.5, -26913838.0], yaw_pitch: [0, -0.42] };
  const v = judgeStationPark(OVERVIEW, done);
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /missed its pose/);
  assert.match(v.message, /BUG-132/);
});

test("BUG-132 second pass: a kilometre out FAILS", () => {
  const done = { ok: true, station: "home", position: [270, 26, -1230], yaw_pitch: [0, -0.42] };
  const v = judgeStationPark(OVERVIEW, done);
  assert.strictEqual(v.ok, false);
  assert.ok(v.error_m > 1000, `error ${v.error_m}`);
});

test("a miss just over the tolerance FAILS", () => {
  const done = { ok: true, position: [27.5 + STATION_POSE_TOL_M * 1.5, 26, 96], yaw_pitch: [0, -0.42], station_ride: true };
  assert.strictEqual(judgeStationPark(OVERVIEW, done).ok, false);
});

test("on the pose but not riding the station FAILS", () => {
  const done = { ok: true, position: [27.5, 26, 96], yaw_pitch: [0, -0.42], station_ride: false };
  const v = judgeStationPark(OVERVIEW, done);
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /NOT riding/);
});

test("on the pose but looking the wrong way FAILS", () => {
  const done = { ok: true, position: [27.5, 26, 96], yaw_pitch: [1.2, -0.42], station_ride: true };
  const v = judgeStationPark(OVERVIEW, done);
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /wrong way/);
});

test("a yaw that differs by a whole turn is the same look", () => {
  const done = { ok: true, position: [27.5, 26, 96], yaw_pitch: [2 * Math.PI, -0.42], station_ride: true };
  assert.strictEqual(judgeStationPark(OVERVIEW, done).ok, true);
});

test("a pose park whose done has no position FAILS rather than passing", () => {
  const v = judgeStationPark(OVERVIEW, { ok: true, station: "home" });
  assert.strictEqual(v.judged, true);
  assert.strictEqual(v.ok, false);
});

test("a malformed vantage pose FAILS", () => {
  const v = judgeStationPark({ station: "home", pose: "1,2,3" }, { ok: true, position: [1, 2, 3] });
  assert.strictEqual(v.ok, false);
});

test("no pose: judged against the engine's own requested position", () => {
  const good = { ok: true, requested: [0, 34, 34], position: [0, 34, 34.01], station_ride: true };
  assert.strictEqual(judgeStationPark(CLOCK, good).ok, true);
  const bad = { ok: true, requested: [0, 34, 34], position: [0, 34, 900], station_ride: true };
  assert.strictEqual(judgeStationPark(CLOCK, bad).ok, false);
});

test("no pose and an older exe with nothing to compare: not judged, not passed", () => {
  const v = judgeStationPark(CLOCK, { ok: true, station: "home" });
  assert.strictEqual(v.judged, false);
});

test("a planet park is not a station park", () => {
  const v = judgeStationPark({ body: "earth", lat: 23, lon: 13 }, { ok: true });
  assert.strictEqual(v.judged, false);
});

test("capture time: the camera still on its pose passes, drifted off FAILS", () => {
  const park = { ok: true, position: [6.9, 1.5, 58.6], yaw_pitch: [-0.55, -0.14], station_ride: true };
  const held = { ok: true, camera_home: [6.9, 1.5, 58.6], camera_yaw_pitch: [-0.55, -0.14], station_ride: true };
  assert.strictEqual(judgeStationCapture(RACKS, park, held).ok, true);
  const drifted = { ok: true, camera_home: [6.9, 1.5, 61.0], camera_yaw_pitch: [-0.55, -0.14], station_ride: true };
  const v = judgeStationCapture(RACKS, park, drifted);
  assert.strictEqual(v.ok, false);
  assert.match(v.message, /screenshot_done/);
});

test("capture time: an exe without camera_home is not judged", () => {
  const v = judgeStationCapture(RACKS, {}, { ok: true, path: "debug/screenshot_1.png" });
  assert.strictEqual(v.judged, false);
});

// A full sweep must re-test BUG-132 (review, 2026-10-03): the bug only shows
// when a POSE or SCREEN station park comes straight after a planet park (the
// camera away from the home). The no-pose park never had it, and a park made
// while already riding the station adds a zero offset, so a vantage order
// that puts every pose and screen park after another station park keeps a
// full sweep green with the bug back. This pins the order.
test("a full sweep parks a pose and a screen view straight after a planet view", () => {
  const spec = JSON.parse(require("fs").readFileSync(require("path").join(__dirname, "..", "..", "tests", "visual", "vantages.json"), "utf8"));
  const kind = (v) => (v.camera && v.camera.station !== undefined ? (v.camera.pose ? "pose" : v.camera.screen ? "screen" : "default") : "planet");
  const vs = spec.vantages;
  const firstAfterPlanet = (k) => vs.some((v, i) => i > 0 && kind(v) === k && kind(vs[i - 1]) === "planet");
  assert.ok(firstAfterPlanet("pose"), "some pose station vantage must come straight after a planet vantage");
  assert.ok(firstAfterPlanet("screen"), "some screen station vantage must come straight after a planet vantage");
});

test("a default or screen park facing the wrong way FAILS", () => {
  const camera = { station: "home" };
  const parkDone = { requested: [0, 34, 34], requested_yaw_pitch: [0, -0.33] };
  const r = judgeStationPark(camera, { ...parkDone, ok: true, position: [0, 34, 34], yaw_pitch: [3.1, -0.33], station_ride: true });
  assert.equal(r.ok, false, JSON.stringify(r));
  assert.match(r.message, /look|yaw|facing|rad/i, "it fails on the look, not on something else");
  // And the same park facing the way the engine chose passes.
  const good = judgeStationPark(camera, { ...parkDone, ok: true, position: [0, 34, 34], yaw_pitch: [0, -0.33], station_ride: true });
  assert.equal(good.ok, true, JSON.stringify(good));
  // Red check, run 2026-10-03: with expectedStationPose returning yaw and
  // pitch null for a park without a pose (the old code), the first assert
  // failed: the park facing the wrong way passed.
});
