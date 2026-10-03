// THE STATION PARK CHECK: did a {"station":"home",...} camera request put the
// camera where it asked, in the home's own coordinates? Pure: it reads the
// vantage's camera request and the engine's done files and returns a verdict,
// so it can be tested with made-up done files (scripts/tests/
// station-park-check.test.js, in `just rig-tests`) without booting anything.
//
// WHY IT EXISTS (BUG-132, 2026-10-03): the station verb added the PREVIOUS
// frame's station offset to the requested pose. After probe-sweep's warm-up
// that offset was the distance from Earth's surface to the home, so the camera
// was parked tens of thousands of km out in empty space, the next frame read
// that as "left the station" and let go of it, and the capture showed stars.
// probe-sweep still called the vantage "ok", because nothing compared where
// the camera went with where it was sent. This is that comparison.
//
// WHAT IT COMPARES:
//   * the EXPECTED pose: the vantage's own "pose" ("x,y,z,yaw,pitch", home
//     frame, metres and radians) when it has one; otherwise the engine's
//     `requested` field (the over-the-deck or facing-a-screen position the
//     engine worked out itself, which the script cannot know in advance).
//   * the REPORTED pose: camera_done's `position` (since the BUG-132 fix a
//     MEASUREMENT, taken on the frame after the requested clock moved the
//     station, never an echo of the request), or, at capture time,
//     screenshot_done's `camera_home` (where the camera was when the picture
//     was taken).
//   * `station_ride`: a camera that is not riding the station is not aboard,
//     whatever its coordinates say.
//
// A request that is not a station request is not judged (judged: false).
// So is a station request with nothing to compare against (an older exe's
// over-the-deck park, which reports no position), and the caller logs that
// rather than calling it a pass.

"use strict";

// How far the camera may land from its requested pose, in metres. The pose is
// written as f32 metres in the home frame, so an honest park is exact to well
// under a millimetre; 5 cm is "a few cm" of slack for f32 rounding along the
// way, and still three orders of magnitude below the BUG-132 miss (hundreds of
// metres at the smallest, tens of thousands of km at the largest).
const STATION_POSE_TOL_M = 0.05;
// How far the look direction may turn, in radians (about half a degree).
const STATION_ANGLE_TOL_RAD = 0.01;

/** "x,y,z,yaw,pitch" -> { pos: [x,y,z], yaw, pitch }, or null when malformed. */
function parsePose5(text) {
  if (typeof text !== "string") return null;
  const v = text.split(",").map((s) => Number(s.trim()));
  if (v.length !== 5 || v.some((x) => !Number.isFinite(x))) return null;
  return { pos: [v[0], v[1], v[2]], yaw: v[3], pitch: v[4] };
}

/** True for an array of exactly three finite numbers. */
function isVec3(a) {
  return Array.isArray(a) && a.length === 3 && a.every((x) => typeof x === "number" && Number.isFinite(x));
}

/** Smallest signed difference between two angles, in radians. */
function angleDiff(a, b) {
  let d = (a - b) % (2 * Math.PI);
  if (d > Math.PI) d -= 2 * Math.PI;
  if (d < -Math.PI) d += 2 * Math.PI;
  return d;
}

const fmt = (p) => `(${p.map((x) => x.toFixed(2)).join(", ")})`;

/**
 * The home-frame pose a station request must land on, or null when the
 * request is not a station request or there is nothing to compare against.
 * `parkDone` is the camera_done of the park (its `requested` is the fallback).
 */
function expectedStationPose(camera, parkDone) {
  if (!camera || camera.station === undefined) return null;
  if (camera.pose !== undefined) return parsePose5(camera.pose);
  if (parkDone && isVec3(parkDone.requested)) {
    // No pose of its own: the engine chose the position (over the deck, or in
    // front of a named screen), and reports the look it chose with it
    // (`requested_yaw_pitch`), so the look is checked too (review,
    // 2026-10-03: it was not, so a park facing away passed).
    const yp = Array.isArray(parkDone.requested_yaw_pitch) ? parkDone.requested_yaw_pitch : null;
    return { pos: parkDone.requested, yaw: yp ? yp[0] : null, pitch: yp ? yp[1] : null };
  }
  return null;
}

/**
 * Judge one reported camera pose against the expected one.
 *   reported: { pos: [x,y,z] | undefined, riding: bool | undefined, yawPitch: [yaw,pitch] | undefined }
 *   what:     short name of the done file, for the message
 * Returns { judged, ok, error_m, message }.
 */
function judgePose(expected, reported, what, tol = {}) {
  const tolM = tol.tolM ?? STATION_POSE_TOL_M;
  const tolRad = tol.tolRad ?? STATION_ANGLE_TOL_RAD;
  if (!isVec3(reported.pos)) {
    return {
      judged: true,
      ok: false,
      error_m: null,
      message: `${what} carries no camera position, so the station park cannot be checked (expected home-frame ${fmt(expected.pos)})`,
    };
  }
  const p = reported.pos;
  const e = expected.pos;
  const err = Math.hypot(p[0] - e[0], p[1] - e[1], p[2] - e[2]);
  const problems = [];
  if (err > tolM) {
    problems.push(
      `missed its pose by ${err.toFixed(2)} m: asked for home-frame ${fmt(e)}, ${what} reports ${fmt(p)}` +
        ` (allowed ${tolM} m). The capture will not show the home there (BUG-132).`,
    );
  }
  if (reported.riding === false) {
    problems.push(`${what} says the camera is NOT riding the station, so it is not aboard the home`);
  }
  if (expected.yaw !== null && expected.yaw !== undefined && Array.isArray(reported.yawPitch)) {
    const dy = angleDiff(reported.yawPitch[0], expected.yaw);
    const dp = angleDiff(reported.yawPitch[1], expected.pitch);
    if (Math.abs(dy) > tolRad || Math.abs(dp) > tolRad) {
      problems.push(
        `looks the wrong way: asked for yaw ${expected.yaw.toFixed(3)} pitch ${expected.pitch.toFixed(3)},` +
          ` ${what} reports yaw ${reported.yawPitch[0].toFixed(3)} pitch ${reported.yawPitch[1].toFixed(3)}`,
      );
    }
  }
  return {
    judged: true,
    ok: problems.length === 0,
    error_m: err,
    message: problems.length ? `station park ${problems.join("; ")}` : `station park on its pose (off by ${(err * 100).toFixed(1)} cm)`,
  };
}

/**
 * Judge a camera_done against the vantage's station request.
 * Returns { judged: false, message } when there is nothing to judge.
 */
function judgeStationPark(camera, parkDone, tol) {
  if (!camera || camera.station === undefined) return { judged: false, ok: true, message: "not a station park" };
  const expected = expectedStationPose(camera, parkDone);
  if (!expected) {
    if (camera.pose !== undefined) {
      return { judged: true, ok: false, error_m: null, message: `the vantage's pose ${JSON.stringify(camera.pose)} is not five numbers` };
    }
    return { judged: false, ok: true, message: "station park without a pose, and this exe reports no requested position to compare" };
  }
  return judgePose(
    expected,
    { pos: parkDone && parkDone.position, riding: parkDone && parkDone.station_ride, yawPitch: parkDone && parkDone.yaw_pitch },
    "camera_done",
    tol,
  );
}

/**
 * Judge where the camera was when the picture was taken (screenshot_done's
 * `camera_home`, `camera_yaw_pitch`, `station_ride`) against the same
 * expected pose. An exe that predates those fields is not judged here; the
 * park check above still is.
 */
function judgeStationCapture(camera, parkDone, shotDone, tol) {
  if (!camera || camera.station === undefined) return { judged: false, ok: true, message: "not a station park" };
  if (!shotDone || shotDone.camera_home === undefined) {
    return { judged: false, ok: true, message: "this exe does not report the capture-time camera" };
  }
  const expected = expectedStationPose(camera, parkDone);
  if (!expected) return { judged: false, ok: true, message: "nothing to compare the capture-time camera against" };
  return judgePose(
    expected,
    { pos: shotDone.camera_home, riding: shotDone.station_ride, yawPitch: shotDone.camera_yaw_pitch },
    "screenshot_done",
    tol,
  );
}

module.exports = {
  STATION_POSE_TOL_M,
  STATION_ANGLE_TOL_RAD,
  parsePose5,
  expectedStationPose,
  judgeStationPark,
  judgeStationCapture,
};
