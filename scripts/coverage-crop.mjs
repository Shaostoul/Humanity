// SAME-REGION cloud-coverage comparison (the invariance protocol,
// environment program 12c adjudication). Usage:
//   node scripts/coverage-crop.mjs <low.png> <high.png> <lowAltKm> <highAltKm>
// Both captures must be nadir views of the SAME lat/lon from the same
// boot (pinned field). The high frame's central crop covering the low
// frame's ground extent is compared against the low frame's central
// region at the same angular margin. Coverage = fraction of pixels
// whose whiteness (min(r,g,b)) exceeds the cloud threshold - crude but
// identical for both frames, so the DIFFERENCE is the signal.
import { createRequire } from "node:module";
const sharp = createRequire(import.meta.url)("sharp");

const [lowF, highF, lowAlt, highAlt] = process.argv.slice(2);
const la = parseFloat(lowAlt), ha = parseFloat(highAlt);

async function coverage(file, frac) {
  const { data, info } = await sharp(file).raw().toBuffer({ resolveWithObject: true });
  const cx = info.width / 2, cy = info.height / 2;
  const rx = (info.width * frac) / 2, ry = (info.height * frac) / 2;
  let cloud = 0, n = 0;
  for (let y = Math.floor(cy - ry); y < cy + ry; y++) {
    for (let x = Math.floor(cx - rx); x < cx + rx; x++) {
      const o = (y * info.width + x) * info.channels;
      const w = Math.min(data[o], data[o + 1], data[o + 2]);
      if (w > 150) cloud++;
      n++;
    }
  }
  return cloud / n;
}

// The low frame's central band (avoid the fisheye edges): 50% of frame.
// The high frame's crop of the SAME ground region: ground extent scales
// ~ altitude for nadir small-angle views, so the crop fraction is
// (la/ha) * 0.5 of the high frame.
const lowCov = await coverage(lowF, 0.5);
const highCov = await coverage(highF, 0.5 * (la / ha));
console.log("low ", lowF, "coverage", (lowCov * 100).toFixed(1) + "%");
console.log("high", highF, "same-region crop coverage", (highCov * 100).toFixed(1) + "%");
console.log("ratio high/low:", (highCov / Math.max(lowCov, 1e-6)).toFixed(2));
