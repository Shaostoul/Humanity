// Per-row horizontal RMS luminance gradient over the sea band (the
// ocean zebra-stripe falsification instrument, water agent 2026-08-23).
// A correctly filtered sea's profile DECREASES monotonically toward the
// horizon; the pre-fix profile PEAKED just under it (33/27/20 at rows
// 725/750/775 on ocean-storm-horizon vs ~1.6 in the sky) because the
// grazing band was 4-17x under-filtered.
// Usage: node scripts/ocean-rms-profile.mjs <capture.png>
import { createRequire } from "node:module";
const sharp = createRequire(import.meta.url)("sharp");
const file = process.argv[2];
const { data, info } = await sharp(file).raw().toBuffer({ resolveWithObject: true });
const W = info.width, C = info.channels;
for (let y = 650; y < Math.min(1350, info.height - 40); y += 25) {
  let s = 0, n = 0;
  for (let x = 300; x < W - 260; x++) {
    const o = (y * W + x) * C, p = (y * W + x + 1) * C;
    const a = 0.2126 * data[o] + 0.7152 * data[o + 1] + 0.0722 * data[o + 2];
    const b = 0.2126 * data[p] + 0.7152 * data[p + 1] + 0.0722 * data[p + 2];
    s += (b - a) ** 2;
    n++;
  }
  console.log(y, Math.sqrt(s / n).toFixed(2));
}
