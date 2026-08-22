// Pure-black census over the horizon band (the water agent's hairline
// finding): counts pixels with r+g+b <= 6 in the middle band of the
// frame, excluding HUD margins. No shading path emits exact black -
// every one adds aerial in-scatter or an ambient floor - so any hit is
// the coverage gap.
import { createRequire } from "node:module";
const sharp = createRequire(import.meta.url)("sharp");
const file = process.argv[2];
const { data, info } = await sharp(file).raw().toBuffer({ resolveWithObject: true });
let hits = 0;
let rows = new Map();
const y0 = Math.floor(info.height * 0.3);
const y1 = Math.floor(info.height * 0.75);
for (let y = y0; y < y1; y++) {
  for (let x = 10; x < info.width - 10; x++) {
    const o = (y * info.width + x) * info.channels;
    if (data[o] + data[o + 1] + data[o + 2] <= 6) {
      hits++;
      rows.set(y, (rows.get(y) || 0) + 1);
    }
  }
}
const top = [...rows.entries()].sort((a, b) => b[1] - a[1]).slice(0, 4);
console.log(file, "pure-black census:", hits, "| top rows:", JSON.stringify(top));
