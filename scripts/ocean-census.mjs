// Census gate for the underwater-extinction fix (water agent, 2026-08-22):
// count sea-band pixels within +-3 of sRGB (9,38,64) - the shader's
// fully-extinguished in-scatter color, unreachable by any other path.
// Defective captures: 969 (fog) / 2431 (clear). Fixed target: ~0.
import { createRequire } from "node:module";
const sharp = createRequire(import.meta.url)("sharp");
const file = process.argv[2];
const { data, info } = await sharp(file).raw().toBuffer({ resolveWithObject: true });
let hits = 0;
const [tr, tg, tb] = [9, 38, 64];
// Sea band: skip the top third (sky) and the HUD strips.
const y0 = Math.floor(info.height * 0.45);
for (let y = y0; y < info.height - 60; y++) {
  for (let x = 0; x < info.width; x++) {
    const o = (y * info.width + x) * info.channels;
    if (
      Math.abs(data[o] - tr) <= 3 &&
      Math.abs(data[o + 1] - tg) <= 3 &&
      Math.abs(data[o + 2] - tb) <= 3
    ) {
      hits++;
    }
  }
}
console.log(file, "extinction-color census:", hits);
