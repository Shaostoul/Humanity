// Profile cloud GRAIN across the terminator, band by band.
//
// Written 2026-09-22, when a band profile showed the defect is 73x worse at the
// dusk line than at noon (0.55 percent in full daylight, 40.43 at the dusk line,
// about 4 on the night side where nothing is directly lit). The operator had
// already said exactly that with no instrument: the clouds are "glistening ...
// most noticeable at the dusk line".
//
// WHY THIS TOOL EXISTS RATHER THAN A SINGLE NUMBER. Every grain measurement
// before it was taken at approach-2000km-high, which is NOON, and noon is the
// weakest regime by a factor of seventy. A candidate fix that halves the noise
// there may do nothing where the defect lives, so grain candidates are ranked
// with this, not with a whole-frame average.
//
// It follows from the bisect: the carrier is the direct-sun term, which is
// exp(-tau_sun), and at a grazing sun the optical path through the deck is
// longest and most variable, so the exponential is at its most nonlinear there.
//
// Reports, per vertical band from the lit edge to the night side:
//   mean L    band brightness, so a fix that merely darkens cannot pass
//   speckle   mean absolute deviation from the four-neighbour mean, over the
//             band mean. This is the grain.
//
// usage: node scripts/terminator-grain.js <capture.png> [more.png ...]
//        [--bands 10] [--x0 0.30] [--x1 0.70]
const sharp = require("sharp");

const args = process.argv.slice(2);
let bands = 10, X0 = 0.30, X1 = 0.70;
const files = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--bands") bands = Number(args[++i]);
  else if (args[i] === "--x0") X0 = Number(args[++i]);
  else if (args[i] === "--x1") X1 = Number(args[++i]);
  else files.push(args[i]);
}
if (!files.length) {
  console.error("usage: node scripts/terminator-grain.js <capture.png> ...");
  process.exit(2);
}

(async () => {
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height, C = info.channels;
    const lum = (i) => data[i] * 0.299 + data[i + 1] * 0.587 + data[i + 2] * 0.114;
    // A tight vertical crop: a loose one counts the chat overlay and the HUD,
    // which on a dark frame read as bright cloud and poison the band.
    const y0 = Math.floor(H * 0.25), y1 = Math.floor(H * 0.75);
    console.log(f.split(/[\\/]/).slice(-1)[0]);
    let peak = 0, peakBand = -1;
    for (let b = 0; b < bands; b++) {
      const w = (X1 - X0) / bands;
      const x0 = Math.floor(W * (X0 + w * b)) + 1;
      const x1 = Math.floor(W * (X0 + w * (b + 1))) - 1;
      let n = 0, sum = 0, dev = 0;
      for (let y = y0; y < y1; y++) {
        for (let x = x0; x < x1; x++) {
          const i = (y * W + x) * C;
          const L = lum(i);
          if (L < 8) continue; // unlit: nothing to be grainy
          const m = (lum(i - C) + lum(i + C) + lum(i - W * C) + lum(i + W * C)) / 4;
          n++; sum += L; dev += Math.abs(L - m);
        }
      }
      if (n < 500) { console.log("  band " + b + "  (too few lit pixels)"); continue; }
      const sp = (100 * dev) / sum;
      if (sp > peak) { peak = sp; peakBand = b; }
      console.log(
        "  band " + String(b).padStart(2),
        " mean L", (sum / n).toFixed(1).padStart(6),
        " speckle", sp.toFixed(2).padStart(6) + "%"
      );
    }
    console.log("  PEAK " + peak.toFixed(2) + "% at band " + peakBand);
  }
})();
