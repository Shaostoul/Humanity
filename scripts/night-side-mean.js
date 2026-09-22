// Measure the NIGHT SIDE of a planet capture.
//
// Two dark frames cannot be ranked by eye. The night-side coast glow (a bright
// cyan trace along every coastline and shallow shelf on the dark half) survived
// four hypotheses partly because "looks a bit dimmer" was the only available
// verdict, and a bisect arm that halves the glow looks identical to one that
// does nothing.
//
// Prints, over a crop of the dark half of the disc:
//   mean rgb  the average colour. A blue channel well above red and green is
//             the signature of the glow, because the cyan comes from bright
//             shallow-water albedo lit by a small blue-biased ambient.
//   lit%      the share of pixels whose blue channel clears a threshold, which
//             is what "coastlines traced in light" actually means.
//
// Usage:  node scripts/night-side-mean.js <capture.png> [more.png ...]
//         node scripts/night-side-mean.js --threshold 24 a.png b.png
//
// The crop is tuned for `orbit-terminator-3000km`, where the terminator runs
// down the middle and the dark half is the right of the disc. Pass --crop
// x0,x1,y0,y1 as fractions of width/height for any other framing.

const sharp = require("sharp");

const args = process.argv.slice(2);
let threshold = 18;
let crop = [0.55, 0.74, 0.12, 0.88];
const files = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--threshold") {
    threshold = Number(args[++i]);
  } else if (args[i] === "--crop") {
    crop = args[++i].split(",").map(Number);
    if (crop.length !== 4 || crop.some((v) => !Number.isFinite(v))) {
      console.error("--crop wants x0,x1,y0,y1 as fractions, e.g. 0.55,0.74,0.12,0.88");
      process.exit(2);
    }
  } else {
    files.push(args[i]);
  }
}
if (!files.length) {
  console.error("usage: node scripts/night-side-mean.js [--threshold N] [--crop x0,x1,y0,y1] <png>...");
  process.exit(2);
}

(async () => {
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const W = info.width;
    const H = info.height;
    const C = info.channels;
    const x0 = Math.floor(W * crop[0]);
    const x1 = Math.floor(W * crop[1]);
    const y0 = Math.floor(H * crop[2]);
    const y1 = Math.floor(H * crop[3]);
    let r = 0;
    let g = 0;
    let b = 0;
    let n = 0;
    let lit = 0;
    for (let y = y0; y < y1; y += 2) {
      for (let x = x0; x < x1; x += 2) {
        const i = (y * W + x) * C;
        r += data[i];
        g += data[i + 1];
        b += data[i + 2];
        if (data[i + 2] > threshold) lit++;
        n++;
      }
    }
    const label = f.split(/[\\/]/).slice(-2).join("/");
    console.log(
      label.padEnd(46),
      "mean rgb",
      (r / n).toFixed(2).padStart(6),
      (g / n).toFixed(2).padStart(6),
      (b / n).toFixed(2).padStart(6),
      "  lit%",
      ((100 * lit) / n).toFixed(2).padStart(6)
    );
  }
})();
