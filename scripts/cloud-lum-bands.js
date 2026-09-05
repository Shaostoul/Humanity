// Rotation-invariant luminance bands over the central 80% of the frame (HUD excluded):
// ocean/dark < 60, grey cloud 60..200, white > 200. Usage: node scripts/_lum_bands.js a.png b.png ...
const sharp = require("sharp");
(async () => {
  for (const f of process.argv.slice(2)) {
    const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height; let n = 0, dark = 0, grey = 0, white = 0, s = 0;
    for (let y = Math.floor(H * 0.1); y < H * 0.9; y += 2) for (let x = Math.floor(W * 0.1); x < W * 0.9; x += 2) {
      const l = data[y * W + x]; n++; s += l; if (l < 60) dark++; else if (l <= 200) grey++; else white++;
    }
    console.log(f.replace(/.*[\/]/, "").padEnd(28), "mean", (s / n).toFixed(1), " dark", (100 * dark / n).toFixed(1) + "%", " grey", (100 * grey / n).toFixed(1) + "%", " white", (100 * white / n).toFixed(1) + "%");
  }
})();
