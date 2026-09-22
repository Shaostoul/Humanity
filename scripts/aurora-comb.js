// Measure the aurora RAY COMB: how much of the aurora's brightness sits in a
// high-frequency stripe pattern rather than in its shape.
//
// Written 2026-09-22 after the operator photographed an aurora band covered in
// regular fine stripes ("weird darkness", and the band reading as fabric). The
// ray modulation runs at AURORA_RAY_LOBES cycles around the oval, and at his
// range those cycles land a few pixels apart, where a pure sine comb reads as
// corduroy rather than as auroral striation. Two different failures live here
// and this metric separates them from "the aurora is dim":
//
//   comb%   mean absolute difference between HORIZONTALLY adjacent aurora
//           pixels, as a percentage of the mean aurora brightness. A smooth
//           curtain is low; a comb is high. This is the corduroy number.
//   mean    mean green over aurora pixels, so a fix that merely dims the
//           aurora cannot be mistaken for a fix that smooths it.
//   px      how many aurora pixels were found, so a fix that shrinks the
//           aurora cannot hide either.
//
// All three have to be read together. Lowering comb% while mean and px collapse
// means the aurora was deleted, not fixed.
//
// usage: node scripts/aurora-comb.js <capture.png> [more.png ...]
const sharp = require("sharp");

const files = process.argv.slice(2);
if (!files.length) {
  console.error("usage: node scripts/aurora-comb.js <capture.png> ...");
  process.exit(2);
}

(async () => {
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height, C = info.channels;
    let n = 0, sum = 0, diff = 0;
    let fn = 0, fsum = 0;
    for (let y = Math.floor(H * 0.1); y < Math.floor(H * 0.95); y++) {
      for (let x = Math.floor(W * 0.05) + 1; x < Math.floor(W * 0.95) - 1; x++) {
        const i = (y * W + x) * C;
        const g = data[i + 1], r = data[i], b = data[i + 2];
        fn++; fsum += r * 0.299 + g * 0.587 + b * 0.114;
        // Aurora pixels: green clearly dominant. The green line is what the
        // ray modulation rides on, so the comb shows there first.
        if (!(g > b + 8 && g > r + 8 && g > 18)) continue;
        n++; sum += g;
        // Horizontal neighbours. The rays run roughly vertically on screen for
        // a band crossing the view, so the comb is a HORIZONTAL oscillation.
        diff += Math.abs(data[i + 1 + C] - data[i + 1 - C]) * 0.5;
      }
    }
    const frameMean = fsum / Math.max(fn, 1);
    // REFUSE TO ANSWER ON A DAYLIT FRAME.
    //
    // Green dominance identifies the aurora only where nothing ELSE green is
    // lit. On a daylit Earth it identifies VEGETATION, and on 2026-09-22 that
    // produced a confident land-versus-water aurora comparison that was really
    // measuring how much Siberian forest was in shot: 73,877 aurora pixels over
    // land against 20,808 over water, from two frames whose mean luminance was
    // 103.5 and 103.0. Both were daylit and neither contained a measurable
    // aurora at all.
    //
    // The aurora only draws over ground that is in darkness, so a frame bright
    // enough to show vegetation cannot be one this metric applies to.
    if (frameMean > 40) {
      console.log(
        f.split(/[\\/]/).slice(-1)[0].padEnd(30),
        " REFUSED: frame mean L " + frameMean.toFixed(1) + " is daylit, so green pixels are vegetation, not aurora."
      );
      continue;
    }
    const mean = sum / Math.max(n, 1);
    console.log(
      f.split(/[\\/]/).slice(-1)[0].padEnd(30),
      "aurora px", String(n).padStart(7),
      " mean green", mean.toFixed(1).padStart(6),
      " comb", ((100 * (diff / Math.max(n, 1))) / Math.max(mean, 1)).toFixed(1).padStart(5) + "%"
    );
  }
})();
