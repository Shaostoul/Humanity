// Is the cloud deck LIT correctly? Measured against the ground in the same frame.
//
// Written 2026-09-22, when seven hypotheses about the orbital "TV static" had
// been refuted and nobody had checked the thing that turned out to be wrong: at
// noon over the Sahara, High tier renders cloud DARKER than the desert under it.
//
//   High tier  cloud 120.1  terrain 175.0  ratio 0.69
//   Low tier   cloud 189.2  terrain 175.9  ratio 1.08
//
// Cloud albedo is about 0.7 to 0.9 and Sahara sand about 0.35, so a daylit deck
// must come out BRIGHTER than the desert. A ratio below 1.0 at noon is a
// lighting defect, and no amount of denoising fixes it.
//
// WHY THE RATIO AND NOT THE CLOUD NUMBER ALONE: terrain in the same frame is the
// control. It shares the sun, the exposure, the tonemap and the atmosphere, so
// if terrain matches between two arms and cloud does not, the difference is in
// the cloud path and nowhere else. That is what made the tier comparison
// conclusive.
//
// Classification is crude on purpose and stated so it can be argued with:
//   cloud   = low saturation (max-min < satMax) and above the sky floor
//   terrain = warm (red exceeds blue by warmMin)
// It suits a desert vantage. Over ocean or ice, pass different thresholds or
// pick a different reference surface.
//
// usage: node scripts/cloud-brightness.js <capture.png> [more.png ...]
//        [--sat 18] [--warm 35] [--crop x0,x1,y0,y1]
const sharp = require("sharp");

const args = process.argv.slice(2);
let satMax = 18, warmMin = 35, crop = [0.15, 0.85, 0.15, 0.85];
const files = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--sat") satMax = Number(args[++i]);
  else if (args[i] === "--warm") warmMin = Number(args[++i]);
  else if (args[i] === "--crop") crop = args[++i].split(",").map(Number);
  else files.push(args[i]);
}
if (!files.length) {
  console.error("usage: node scripts/cloud-brightness.js <capture.png> ...");
  process.exit(2);
}

(async () => {
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height, C = info.channels;
    let cN = 0, cL = 0, tN = 0, tL = 0;
    for (let y = Math.floor(H * crop[2]); y < Math.floor(H * crop[3]); y++) {
      for (let x = Math.floor(W * crop[0]); x < Math.floor(W * crop[1]); x++) {
        const i = (y * W + x) * C;
        const r = data[i], g = data[i + 1], b = data[i + 2];
        const L = r * 0.299 + g * 0.587 + b * 0.114;
        if (L < 12) continue; // sky
        const sat = Math.max(r, g, b) - Math.min(r, g, b);
        if (sat < satMax) { cN++; cL += L; }
        else if (r > b + warmMin) { tN++; tL += L; }
      }
    }
    const cloud = cL / Math.max(cN, 1), terrain = tL / Math.max(tN, 1);
    const ratio = cloud / Math.max(terrain, 1e-6);
    console.log(
      f.split(/[\/]/).slice(-1)[0].padEnd(34),
      "cloud", cloud.toFixed(1).padStart(6),
      " terrain", terrain.toFixed(1).padStart(6),
      " ratio", ratio.toFixed(2).padStart(5),
      ratio < 1.0 ? "  <-- DARKER THAN THE GROUND" : ""
    );
  }
})();
