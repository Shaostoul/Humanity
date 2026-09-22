// Measure SPECKLE: per-pixel high-frequency variation that its own neighbours do
// not explain. A coherent cloud mass has smooth interiors, so a high score means
// salt-and-pepper, which is what the operator is describing as white specks and
// as old TV static.
//
// Reported over cloud-ish pixels only (bright enough to be cloud), because the
// black sky around the planet would otherwise dominate and look perfectly clean.
const sharp = require('sharp');
const files = process.argv.slice(2);

(async () => {
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height, C = info.channels;
    const lum = (i) => (data[i] * 0.299 + data[i + 1] * 0.587 + data[i + 2] * 0.114);
    let n = 0, sumDev = 0, sumL = 0, spikes = 0;
    // Skip the HUD margins.
    for (let y = Math.floor(H * 0.12); y < Math.floor(H * 0.92); y++) {
      for (let x = Math.floor(W * 0.08); x < Math.floor(W * 0.92); x++) {
        const i = (y * W + x) * C;
        const L = lum(i);
        if (L < 12) continue; // sky, not cloud
        // 4-neighbour mean, then how far this pixel sits from it.
        const m = (lum(i - C) + lum(i + C) + lum(i - W * C) + lum(i + W * C)) / 4;
        const d = Math.abs(L - m);
        n++; sumDev += d; sumL += L;
        if (d > 0.35 * Math.max(m, 1)) spikes++;
      }
    }
    const label = f.split(/[\\/]/).slice(-1)[0];
    console.log(
      label.padEnd(36),
      'lit px', String(n).padStart(8),
      ' mean L', (sumL / n).toFixed(1).padStart(6),
      ' speckle', (100 * sumDev / sumL).toFixed(2).padStart(6) + '%',
      ' spikes', ((100 * spikes) / n).toFixed(2).padStart(6) + '%'
    );
  }
})();
