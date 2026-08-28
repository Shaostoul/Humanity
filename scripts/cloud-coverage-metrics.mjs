// Does the cloud field reach the coverage the weather asks for?
//
// Usage: node scripts/cloud-coverage-metrics.mjs <sweep-dir> [vantage-id ...]
//
// WHY (2026-08-28). Operator: "a place that looks like it is supposed to have
// big dense clouds are instead rendering as smaller than their container. Like
// the voxel the cloud is in is only filled like 5%... the cloud chunks can never
// get large enough to fill the presently empty space between what are supposed
// to be large cloud lobes."
//
// Measured, and they are right: asked for 0.95, the field delivers 0.14.
//
// TWO THINGS THIS SCRIPT EXISTS TO GET RIGHT, both of which were got wrong first:
//
// 1. MEASURE FROM NADIR. Screen coverage only equals AREAL coverage looking
//    straight down. At a grazing angle even a sparse field fills most of the
//    frame, and an earlier attempt to measure this from a horizon-ish camera
//    read 46% both with and without a change and concluded nothing had happened.
//    The vantage must have look_offset_deg 0.
//
// 2. CLASSIFY ON SATURATION, NOT BRIGHTNESS. Calibrated against real pixels in
//    a capture: cloud sits at saturation 0.05-0.07, sand at 0.36, sky at 0.39,
//    while LUMINANCE overlaps all three (cloud 163-224, sand 191, sky 170). A
//    brightness threshold therefore cannot separate them, and an earlier version
//    that used one counted sunlit ground and pale horizon haze as cloud.
//
// The expected value comes from the vantage's own showcase.cloud_cover, so the
// gate compares what was ASKED for against what was DELIVERED rather than
// against a hand-tuned constant that would need re-tuning whenever the weather
// pin changes.

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/cloud-coverage-metrics.mjs <sweep-dir> [vantage-id ...]");
  process.exit(2);
}
const wanted = process.argv.slice(3);

const spec = JSON.parse(
  fs.readFileSync(path.join("tests", "visual", "vantages.json"), "utf8")
);
let entries = spec.vantages.filter(
  v => v.regressions && v.regressions.includes("the cloud field must reach the coverage the weather asks for")
);
if (wanted.length) entries = entries.filter(v => wanted.includes(v.id));
if (!entries.length) {
  console.error("no coverage vantages matched");
  process.exit(2);
}

// Saturation below this is cloud. See the calibration note in the header.
const CLOUD_SAT = 0.15;

async function coverage(file) {
  const img = sharp(file);
  const meta = await img.metadata();
  // Central crop: away from the HUD strips at top and bottom, and away from the
  // frame edges where a nadir view starts to slant.
  const box = {
    left: Math.round(meta.width * 0.12),
    top: Math.round(meta.height * 0.14),
    width: Math.round(meta.width * 0.76),
    height: Math.round(meta.height * 0.58),
  };
  const { data, info } = await img.extract(box).raw().toBuffer({ resolveWithObject: true });
  const ch = info.channels;
  const n = info.width * info.height;
  let cloud = 0;
  for (let i = 0; i < n; i++) {
    const o = i * ch;
    const r = data[o], g = data[o + 1], b = data[o + 2];
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
    if (mx > 0 && (mx - mn) / mx < CLOUD_SAT) cloud++;
  }
  return cloud / n;
}

let fails = 0;
for (const v of entries) {
  const f = path.join(dir, `${v.id}.png`);
  if (!fs.existsSync(f)) { console.error(`missing capture: ${f}`); process.exit(2); }
  if (v.camera.look_offset_deg !== 0) {
    console.error(
      `${v.id}: look_offset_deg is ${v.camera.look_offset_deg}, not 0.\n` +
      "  Areal coverage can only be measured looking straight down. Refusing to score."
    );
    fails++;
    continue;
  }
  const asked = Number(v.showcase.cloud_cover);
  const got = await coverage(f);
  const ratio = got / Math.max(asked, 1e-6);
  console.log(`\n${v.id}`);
  console.log(`  weather asked for : ${(asked * 100).toFixed(1)}%`);
  console.log(`  field delivered   : ${(got * 100).toFixed(1)}%`);
  console.log(`  delivered / asked : ${ratio.toFixed(3)}   (want > 0.70)`);
  if (ratio < 0.70) {
    console.error(
      `  FAIL - the field reaches only ${(ratio * 100).toFixed(0)}% of the coverage it was asked for.\n` +
      "  The gaps between clouds cannot close: whatever the weather says, the\n" +
      "  field saturates at what its clouds can fill."
    );
    fails++;
  } else {
    console.log("  PASS");
  }
}

if (fails) { console.error(`\n${fails} vantage(s) FAILED.`); process.exit(1); }
console.log("\nPASS: the cloud field reaches the coverage the weather asks for.");
