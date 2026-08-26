// Does the STAR FIELD move when the homestead turns?
//
// Usage: node scripts/home-sky-metrics.mjs <sweep-dir>
//   expects home-clock-dawn.png / home-clock-noon.png / home-clock-night.png
//
// WHY (2026-08-27). v0.1225 gave the station a nadir-pointing attitude so the
// sun sweeps the deck once per orbit. The frame conversion was applied at three
// sites - the celestial bodies, the sun direction, and local up - and MISSED a
// fourth: src/renderer/stars.rs builds its sky rotation from the camera's
// forward/up alone, and while riding those are HULL-frame vectors. So the sun
// and Earth swept past correctly while the constellations stayed nailed to the
// deck, which is precisely the failure the frame helper's own doc comment warns
// about. Ironically the release comment claimed "the stars sweep past. That
// sweep is the entire point." It did not sweep.
//
// The measurement: the three home-clock captures share ONE fixed camera pose
// (the {"station":"home"} verb sets it; it is not steered) and differ only by
// the game clock. Under a correct hull frame the station's attitude turns with
// the clock, so the sky behind it must MOVE between them.
//
// The crop is the upper-left quadrant, which in this pose holds Milky Way and
// stars with no Earth, no sun and no hull. That matters: there is no atmosphere
// aboard, so this region's content is independent of the lighting and any change
// in it is rotation, not illumination. A crop containing the deck would "differ"
// merely because the deck lit up, and would pass on the broken build.
//
// PASS = the sky moved. On the pre-fix build the star uniform had no dependence
// on the station rotation whatsoever, so these crops are bit-identical and this
// script FAILS - which is the whole point of it existing.

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/home-sky-metrics.mjs <sweep-dir>");
  process.exit(2);
}

async function skyCrop(file) {
  const img = sharp(file);
  const meta = await img.metadata();
  const W = meta.width, H = meta.height;
  const box = {
    left: Math.round(W * 0.02),
    top: Math.round(H * 0.06),
    width: Math.round(W * 0.34),
    height: Math.round(H * 0.30),
  };
  const { data, info } = await img
    .extract(box)
    .greyscale()
    .raw()
    .toBuffer({ resolveWithObject: true });
  return { data, info, box, size: [W, H] };
}

// Mean absolute difference in 0-255 grey. Stars are sparse and high-contrast, so
// even a modest rotation moves a lot of pixels a long way; an unchanged sky
// scores ~0.
function mad(a, b) {
  const n = Math.min(a.length, b.length);
  let sum = 0;
  for (let i = 0; i < n; i++) sum += Math.abs(a[i] - b[i]);
  return sum / n;
}

const names = ["dawn", "noon", "night"];
const crops = {};
for (const n of names) {
  const f = path.join(dir, `home-clock-${n}.png`);
  if (!fs.existsSync(f)) {
    console.error(`missing capture: ${f}`);
    process.exit(2);
  }
  crops[n] = await skyCrop(f);
}

console.log(`sky crop ${JSON.stringify(crops.noon.box)} of ${crops.noon.size.join("x")}`);
const pairs = [
  ["dawn", "noon"],
  ["noon", "night"],
  ["dawn", "night"],
];
const scores = [];
for (const [a, b] of pairs) {
  const d = mad(crops[a].data, crops[b].data);
  scores.push({ pair: `${a} vs ${b}`, mad: d });
  console.log(`  ${a} vs ${b}: mean abs diff ${d.toFixed(3)} (want > 0.500)`);
}

const worst = Math.min(...scores.map((s) => s.mad));
if (!(worst > 0.5)) {
  console.error("\nFAIL");
  console.error(
    `  the star field does not move with the station: smallest pair difference ${worst.toFixed(3)}`
  );
  console.error(
    "\nA hull that turns under a fixed sky must show the sky sweeping past. If\n" +
      "these crops are identical, the sky is being drawn in HULL coordinates and\n" +
      "is glued to the deck - the missed frame-conversion site."
  );
  process.exit(1);
}
console.log("\nPASS: the sky moves as the homestead turns.");
