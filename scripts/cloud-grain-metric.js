// Cloud edge-glitter metric: mean absolute 4-neighbour Laplacian over the
// central 60% of the frame (HUD excluded). High = stippled / glittering edges,
// low = smooth. Like the spoke metric it needs a same-run control; compare
// A/B cells captured in ONE sweep with the clock pinned (cloud_clock).
//
//   node scripts/cloud-grain-metric.js "label=path/a.png" "label=path/b.png"
const sharp = require('sharp');
(async () => {
  for (const arg of process.argv.slice(2)) {
    const [tag, f] = arg.split('=');
    const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height;
    let s = 0, n = 0;
    for (let y = Math.floor(H * 0.2); y < H * 0.8; y++) {
      for (let x = Math.floor(W * 0.2); x < W * 0.8; x++) {
        const i = y * W + x;
        const l = 4 * data[i] - data[i - 1] - data[i + 1] - data[i - W] - data[i + W];
        s += Math.abs(l); n++;
      }
    }
    console.log(tag.padEnd(28), 'grain =', (s / n).toFixed(3));
  }
})();
