// The hair's fringe from the RENDERED pixels (2026-11-13, temporal anti-aliasing rung 1b;
// docs/design/renderer-roadmap.md section 3.3, src/renderer/hashed_alpha.rs).
//
// Usage:
//   node scripts/hair-fringe-metrics.mjs <dir> [--gate] [--a <arm>] [--b <arm>] [--id <vantage>]
//                                        [--rect left,top,width,height]
//     <dir> holds captures of the taa-hair-planet-third vantage: probe-hot-ab's <arm><n>__<id>.png
//     with the arms "hashed" and "twopass" (--a and --b rename them), or a probe-sweep's <id>.png
//     and <id>-two-pass.png (the vantage and its red twin). For each capture it measures the HAIR'S
//     COVERED AREA in the head's crop (--rect; the default is where the vantage frames the head at
//     2560 x 1387): every pixel counts by how far its luma lies from the sand's toward the hair's
//     (SAND and HAIR below), so a strand that covers half a pixel counts half. The fringe the two
//     passes leave out on a planet (the player's body stands in render space, which has no
//     transparent list) is that partial coverage round the core.
//     --gate: exit 1 unless every capture of arm a covers more than the capture of arm b with the
//     same repeat number (probe-hot-ab takes them seconds apart in one park, so a pair shares the
//     head's place in its sway; the sway moves part of the head in and out of the crop between
//     repeats by more than the fringe adds).
//
// WHY AN AREA AND NOT A PIXEL DIFFERENCE. The body sways at idle between captures (the rig's
// time_scale 0 does not stop the skeleton), so a per-pixel difference against the twin finds the
// head's movement: two two-pass captures of one park differed by up to 6x more darker than lighter
// pixels at the core's edge. The covered area does not move with the head.
//
// Seen red first (2026-11-13, the three-repeat same-park A/B of rung 1b, RTX 4070 at 2560 x 1387):
// hashed against two-pass covered 13593, 13344, 13616 px against 13385, 12982, 13405 (pairs +208,
// +362, +211: the fringe, about 2% of the head's area) and passed; the red twin as arm a (two-pass
// against hashed) failed with pairs -208, -362, -211; and the CONTROL, Off against two-pass (the
// same hair passes, only the anti-aliasing differs), failed with pairs -216, +585, -234, so the gate
// does not pass on sway or on anti-aliasing alone.

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

/** sRGB luma of the sand behind the head (the open Sahara at 16:30 reads 40 to 90). */
export const SAND = 50;
/** sRGB luma of the bob's dark hair, fully covered (0 to 30). */
export const HAIR = 15;

const args = process.argv.slice(2);
const dir = args[0];
const opt = (k, d) => {
  const i = args.indexOf(k);
  return i >= 0 ? args[i + 1] : d;
};
const gate = args.includes("--gate");
const armA = opt("--a", "hashed");
const armB = opt("--b", "twopass");
const id = opt("--id", "taa-hair-planet-third");
const [left, top, width, height] = opt("--rect", "880,1085,200,150").split(",").map(Number);

async function covered(file) {
  const data = await sharp(file).extract({ left, top, width, height }).removeAlpha().raw().toBuffer();
  let area = 0;
  for (let k = 0; k < data.length; k += 3) {
    const y = 0.2126 * data[k] + 0.7152 * data[k + 1] + 0.0722 * data[k + 2];
    area += Math.min(1, Math.max(0, (SAND - y) / (SAND - HAIR)));
  }
  return area;
}

function pick(arm) {
  const re = new RegExp(`^${arm}(\\d+)__${id}\\.png$`);
  return fs.readdirSync(dir).filter((f) => re.test(f)).sort().map((f) => path.join(dir, f));
}

let a = pick(armA);
let b = pick(armB);
if (a.length === 0 && b.length === 0) {
  a = [path.join(dir, `${id}.png`)];
  b = [path.join(dir, `${id}-two-pass.png`)];
}
for (const f of [...a, ...b]) {
  if (!fs.existsSync(f)) {
    console.error(`missing ${f}`);
    process.exit(1);
  }
}
const va = await Promise.all(a.map(covered));
const vb = await Promise.all(b.map(covered));
const fmt = (v) => v.map((x) => x.toFixed(0)).join(", ");
const pairs = va.slice(0, vb.length).map((x, i) => x - vb[i]);
const pass = pairs.length > 0 && pairs.every((d) => d > 0);
console.log(`${id}: hair covers ${fmt(va)} px (${armA}) against ${fmt(vb)} px (${armB}) in the ${width} x ${height} head crop, pairs ${pairs.map((d) => (d > 0 ? "+" : "") + d.toFixed(0)).join(", ")} -> ${pass ? `${armA} draws more hair (the fringe)` : `${armA} does NOT draw more hair`}`);
if (gate && !pass) process.exit(1);
