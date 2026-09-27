// Cloud edge-glitter metric: mean absolute 4-neighbour Laplacian over the
// central 60% of the frame (HUD excluded). High = stippled / glittering edges,
// low = smooth. Like the spoke metric it needs a same-run control; compare
// A/B cells captured in ONE sweep with the clock pinned (cloud_clock).
//
// A BLUR WINS THIS NUMBER (2026-09-27, PRIORITIES 2a-ii). A 4-neighbour
// Laplacian is the deviation of a pixel from its neighbourhood mean, and
// replacing pixels with that mean (a 3x3 box blur, which is what the cloud
// resolve's spatial filter does when forced to full) drives it toward zero by
// construction. On the 2026-09-22 terminator captures this script reads the
// frozen-jitter baseline 14.99, a plain 3x3 box blur of that same capture
// 2.81, and the animated-jitter fix that actually removes the noise 5.35: it
// ranks the blur first. It is still a fair
// comparison between two renders that are equally sharp (the far-rung G5 gate
// compares Ultra with High at one camera), but for any arm that filters
// spatially or changes softness, use scripts/terminator-grain.js: noise over
// real cloud detail at ~15 px, credited only when the detail is kept, with a
// --controls mode that proves no blur can win it.
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
  console.log('(a blur lowers this number by construction; for spatially filtered arms use scripts/terminator-grain.js --controls)');
})();
