// The gate for PRIORITIES 1b, "the cloud deck erases the aurora", as numbers.
//
// Two measurements, because the question has two halves and one number cannot
// answer both:
//
//   gx       mean GREEN-EXCESS per pixel in 8-bit sRGB codes: max(0, g - (r+b)/2)
//            averaged over the same crop scripts/aurora-comb.js reads (rows
//            10..95 percent, columns 5..95 percent, which keeps the HUD out).
//            This is the metric the item was written in. It was never committed
//            as code before 2026-09-27; this definition reproduces the logged
//            2026-09-22 figures to within 0.7 percent (14.72 / 14.73 against the
//            logged 14.811 / 14.826 on that sweep's clouds-OFF captures, and
//            2.196 against 2.198 for clouds ON over water).
//
//   deliv    the light the aurora DELIVERS, per channel, in LINEAR light:
//            mean(linear(lit) - linear(dark)) over the same crop, where `dark`
//            is the same fixture rendered with the emission pass switched off
//            (showcase {"aurora":"0"}). Everything that is not the aurora
//            cancels in the subtraction, whatever sits behind it.
//
// Why both. gx is read AFTER the sRGB encode, and that encode is concave: the
// same added light lifts a dark pixel by more codes than a brighter one. So an
// aurora laid correctly over night cloud that is not quite black reads a
// slightly lower gx than the same aurora over black ground, and gx alone cannot
// say whether the deck is still eating light or the metric is just compressing
// it. deliv can: an additive pass delivers the SAME linear light over the deck
// as over bare ground, so deliv(clouds ON) / deliv(clouds OFF) is the
// threshold-free answer to "does the deck still dim the aurora". A pass that
// blended OVER instead would show up here as a deficit exactly where the
// curtain is bright.
//
// usage: node scripts/aurora-gate.js <lit.png>[,<dark.png>] ...
//   Each argument is one frame, optionally paired with its aurora-OFF twin.
const sharp = require("sharp");

const args = process.argv.slice(2);
if (!args.length) {
  console.error("usage: node scripts/aurora-gate.js <lit.png>[,<dark.png>] ...");
  process.exit(2);
}

// sRGB code (0..255) to linear, as a table.
const LIN = new Float64Array(256);
for (let c = 0; c < 256; c++) {
  const v = c / 255;
  LIN[c] = v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

async function load(f) {
  const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
  return { data, W: info.width, H: info.height, C: info.channels };
}

// The aurora-comb crop, so the two scripts always read the same pixels.
function crop(W, H) {
  return {
    y0: Math.floor(H * 0.1),
    y1: Math.floor(H * 0.95),
    x0: Math.floor(W * 0.05) + 1,
    x1: Math.floor(W * 0.95) - 1,
  };
}

const name = (f) => f.split(/[\\/]/).slice(-1)[0];

(async () => {
  for (const arg of args) {
    const [litPath, darkPath] = arg.split(",");
    const lit = await load(litPath);
    const { y0, y1, x0, x1 } = crop(lit.W, lit.H);
    let n = 0, gx = 0, lum = 0;
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        const i = (y * lit.W + x) * lit.C;
        const r = lit.data[i], g = lit.data[i + 1], b = lit.data[i + 2];
        gx += Math.max(0, g - (r + b) / 2);
        lum += r * 0.299 + g * 0.587 + b * 0.114;
        n++;
      }
    }
    let line = `${name(litPath).padEnd(34)} gx ${(gx / n).toFixed(3).padStart(7)}  meanL ${(lum / n).toFixed(1).padStart(5)}`;
    // Same refusal as aurora-comb: on a daylit frame green is vegetation.
    if (lum / n > 40) line += "  (daylit: gx counts vegetation, not aurora)";
    if (darkPath) {
      const dark = await load(darkPath);
      if (dark.W !== lit.W || dark.H !== lit.H) {
        console.log(`${line}  !! dark twin ${name(darkPath)} is ${dark.W}x${dark.H}, lit is ${lit.W}x${lit.H}`);
        continue;
      }
      let dr = 0, dg = 0, db = 0;
      for (let y = y0; y < y1; y++) {
        for (let x = x0; x < x1; x++) {
          const i = (y * lit.W + x) * lit.C;
          dr += LIN[lit.data[i]] - LIN[dark.data[i]];
          dg += LIN[lit.data[i + 1]] - LIN[dark.data[i + 1]];
          db += LIN[lit.data[i + 2]] - LIN[dark.data[i + 2]];
        }
      }
      // x1000 so the numbers read as small integers-and-decimals.
      const k = 1000 / n;
      line += `  deliv x1e3 R ${(dr * k).toFixed(3)} G ${(dg * k).toFixed(3)} B ${(db * k).toFixed(3)}` +
        `  (dark twin ${name(darkPath)})`;
    }
    console.log(line);
  }
})();
