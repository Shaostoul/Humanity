// The Maps page's "ship centred" check (Maps increment 1,
// docs/design/maps-zoom-to-anything.md): is the ship in the middle of the map?
//
// It judges RENDERED PIXELS, not the page's own idea of where the ship is. The
// rig captures the map twice, once as drawn and once with the ship left out of
// the world layer (the showcase verb {"map_ship":"0"}, engine/map_view.rs), and
// the pixels that differ between the two ARE the ship. The check then asks two
// things of them, inside the map's canvas (its rectangle on the window comes
// from the game's debug/map_done.json):
//   * enough of them that a ship is there at all (`min_frac` of the canvas),
//   * their centroid within `tolerance_frac` of the canvas height from its centre.
// A camera that did not follow the ship sees it leave the frame, so the two
// captures barely differ and the check FAILS: the rig's control vantage
// (maps-follow-home-unfollowed) is there to prove it can.
//
// `threshold` is per channel, 0 to 255: the cloud deck behind the ship is
// re-marched between the two captures and moves a few levels, which is noise,
// not ship.

const PNG = require("./png.js");

/** The ship's pixels: where `a` and `b` differ by more than `threshold` in any
 * channel, inside `rect` ([left, top, right, bottom], pixels), and in a SOLID
 * patch: a changed pixel counts only when at least `solid` of the 9 pixels of
 * its 3x3 neighbourhood changed too. A ship is one connected shape; the star
 * field twinkles between two renders a pixel or two at a time, all over the
 * canvas, and scattered noise has its centroid at the canvas centre (the near
 * control passed the check on star twinkle alone before this, 2026-10-21). */
function shipMask(a, b, rect, threshold = 40, solid = 6) {
  if (a.width !== b.width || a.height !== b.height) {
    throw new Error(`the two captures differ in size: ${a.width}x${a.height} and ${b.width}x${b.height}`);
  }
  const l = Math.max(0, Math.floor(rect[0]));
  const t = Math.max(0, Math.floor(rect[1]));
  const r = Math.min(a.width, Math.ceil(rect[2]));
  const btm = Math.min(a.height, Math.ceil(rect[3]));
  const w = Math.max(0, r - l);
  const h = Math.max(0, btm - t);
  const changed = new Uint8Array(w * h);
  for (let y = t; y < btm; y++) {
    for (let x = l; x < r; x++) {
      const i = (y * a.width + x) * 4;
      const d = Math.max(
        Math.abs(a.rgba[i] - b.rgba[i]),
        Math.abs(a.rgba[i + 1] - b.rgba[i + 1]),
        Math.abs(a.rgba[i + 2] - b.rgba[i + 2]),
      );
      if (d > threshold) changed[(y - t) * w + (x - l)] = 1;
    }
  }
  let count = 0;
  let sx = 0;
  let sy = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!changed[y * w + x]) continue;
      let n = 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const yy = y + dy;
          const xx = x + dx;
          if (yy >= 0 && yy < h && xx >= 0 && xx < w) n += changed[yy * w + xx];
        }
      }
      if (n >= solid) {
        count++;
        sx += x + l;
        sy += y + t;
      }
    }
  }
  const area = Math.max(1, (r - l) * (btm - t));
  return { count, area, centroid: count ? [sx / count, sy / count] : null };
}

/** Judge one map capture pair. `withShip` and `withoutShip` are decoded PNGs
 * ({ width, height, rgba }); `canvas` is [left, top, right, bottom]. */
function judgeShipCentred({ withShip, withoutShip, canvas, tolerance_frac = 0.12, min_frac = 0.01, threshold = 40 }) {
  if (!Array.isArray(canvas) || canvas.length !== 4) {
    return { ok: false, message: "no canvas rectangle from the game (debug/map_done.json canvas_px)" };
  }
  const m = shipMask(withShip, withoutShip, canvas, threshold);
  const frac = m.count / m.area;
  const centre = [(canvas[0] + canvas[2]) / 2, (canvas[1] + canvas[3]) / 2];
  const h = Math.max(1, canvas[3] - canvas[1]);
  if (frac < min_frac || !m.centroid) {
    return {
      ok: false,
      frac,
      message: `no ship in the map: ${(frac * 100).toFixed(2)}% of the canvas changed when the ship was left out (need ${(min_frac * 100).toFixed(2)}%)`,
    };
  }
  const off = Math.hypot(m.centroid[0] - centre[0], m.centroid[1] - centre[1]);
  const offFrac = off / h;
  const where = `the ship's pixels (${(frac * 100).toFixed(1)}% of the canvas) centre ${off.toFixed(0)} px from the canvas centre (${(offFrac * 100).toFixed(1)}% of its height, allowed ${(tolerance_frac * 100).toFixed(0)}%)`;
  return {
    ok: offFrac <= tolerance_frac,
    frac,
    centroid: m.centroid,
    offset_px: off,
    offset_frac: offFrac,
    message: offFrac <= tolerance_frac ? `ship centred: ${where}` : `ship NOT centred: ${where}`,
  };
}

/** The verdict for a vantage: a control (`expect: "fail"`) passes only when
 * the check fails, so a check that cannot fail is caught. */
function vantageVerdict(check, expect = "pass") {
  if (expect === "fail") {
    return check.ok
      ? { ok: false, message: `the control PASSED the ship-centred check, so the check cannot fail: ${check.message}` }
      : { ok: true, message: `the control failed the check, as it must: ${check.message}` };
  }
  return { ok: check.ok, message: check.message };
}

/** Decode two PNG files and judge them. */
function judgeFiles(withShipPath, withoutShipPath, canvas, opts = {}) {
  const fs = require("fs");
  return judgeShipCentred({
    withShip: PNG.decode(fs.readFileSync(withShipPath)),
    withoutShip: PNG.decode(fs.readFileSync(withoutShipPath)),
    canvas,
    ...opts,
  });
}

module.exports = { shipMask, judgeShipCentred, vantageVerdict, judgeFiles };
