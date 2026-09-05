// Pixel diff of two captures (the bit-exact gate for a perf trim): max and
// mean absolute difference over RGB, and the count of pixels differing by
// more than 1 level, over the central 80% of the frame (HUD excluded).
//   node scripts/cloud-diff.js before.png after.png
const sharp = require("sharp");
(async () => {
  const [a, b] = process.argv.slice(2);
  const A = await sharp(a).raw().toBuffer({ resolveWithObject: true });
  const B = await sharp(b).raw().toBuffer({ resolveWithObject: true });
  if (A.info.width !== B.info.width || A.info.height !== B.info.height) { console.log("size mismatch"); process.exit(2); }
  const W = A.info.width, H = A.info.height, C = A.info.channels;
  let max = 0, sum = 0, n = 0, over1 = 0, over8 = 0;
  for (let y = Math.floor(H * 0.1); y < H * 0.9; y++) for (let x = Math.floor(W * 0.1); x < W * 0.9; x++) {
    const i = (y * W + x) * C; let d = 0;
    for (let c = 0; c < 3; c++) d = Math.max(d, Math.abs(A.data[i + c] - B.data[i + c]));
    if (d > max) max = d; sum += d; n++; if (d > 1) over1++; if (d > 8) over8++;
  }
  console.log(`max ${max}  mean ${(sum / n).toFixed(3)}  pixels>1: ${(100 * over1 / n).toFixed(3)}%  pixels>8: ${(100 * over8 / n).toFixed(3)}%`);
})();
