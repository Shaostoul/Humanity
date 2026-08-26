// Score the homestead clock A/B: do three captures that differ ONLY by the
// game clock actually light the hull differently?
//
// Usage: node scripts/home-clock-metrics.mjs <sweep-dir>
//   expects home-clock-dawn.png, home-clock-noon.png, home-clock-night.png
//
// WHY THIS EXISTS (2026-08-26). The operator: "it's turning the planet but, I
// can't get it to affect the homestead locked in orbit with the Earth."
// Scrubbing the hour rotated Earth and left the home in permanent noon.
//
// The trap this script is built to avoid: Earth fills a lot of these frames and
// Earth's texture DOES turn with the clock, so a whole-frame mean changes even
// when the hull lighting is frozen solid. A naive check would have passed on the
// broken build. So the measurement is confined to a crop of the HULL DECK in the
// lower-centre of the frame, where the fixed camera pose guarantees hull and
// nothing else, and the pose is identical across all three captures by
// construction (the {"station":"home"} verb sets it, it is not steered).
//
// The gate is deliberately two-sided:
//   - night must be much darker than noon  (the sun set on the hull)
//   - dawn must differ from noon           (it is not just an on/off switch)
// Before the fix all three crops are bit-identical, so this script FAILS on the
// broken build. That is the requirement: a check that cannot fail is not a
// check (feedback_checks_that_cannot_fail).

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/home-clock-metrics.mjs <sweep-dir>");
  process.exit(2);
}

// Rec.709 luma on linearised sRGB. Linearising matters: the difference between
// a lit and an unlit deck is large in light but compressed by the sRGB curve,
// and a gamma-space mean understates it.
function srgbToLinear(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

async function deckLuma(file) {
  const img = sharp(file);
  const meta = await img.metadata();
  const W = meta.width, H = meta.height;
  // Lower-centre crop: the hull deck under the fixed station pose. Kept well
  // clear of the HUD (top strip, bottom hotbar) and of the frame edges where
  // Earth's limb can intrude.
  const box = {
    left: Math.round(W * 0.34),
    top: Math.round(H * 0.60),
    width: Math.round(W * 0.32),
    height: Math.round(H * 0.22),
  };
  const { data, info } = await img
    .extract(box)
    .raw()
    .toBuffer({ resolveWithObject: true });
  let sum = 0;
  const px = info.width * info.height;
  for (let i = 0; i < px; i++) {
    const o = i * info.channels;
    sum +=
      0.2126 * srgbToLinear(data[o]) +
      0.7152 * srgbToLinear(data[o + 1]) +
      0.0722 * srgbToLinear(data[o + 2]);
  }
  return { luma: sum / px, box, size: [W, H] };
}

const names = ["dawn", "noon", "night"];
const out = {};
for (const n of names) {
  const f = path.join(dir, `home-clock-${n}.png`);
  if (!fs.existsSync(f)) {
    console.error(`missing capture: ${f}`);
    process.exit(2);
  }
  out[n] = await deckLuma(f);
}

const dawn = out.dawn.luma, noon = out.noon.luma, night = out.night.luma;
console.log(`deck crop ${JSON.stringify(out.noon.box)} of ${out.noon.size.join("x")}`);
console.log(`  dawn  mean linear luma ${dawn.toFixed(5)}`);
console.log(`  noon  mean linear luma ${noon.toFixed(5)}`);
console.log(`  night mean linear luma ${night.toFixed(5)}`);

const nightRatio = night / Math.max(noon, 1e-9);
const dawnDelta = Math.abs(dawn - noon) / Math.max(noon, 1e-9);
console.log(`  night/noon ratio ${nightRatio.toFixed(3)}  (want < 0.400)`);
console.log(`  |dawn-noon|/noon  ${(dawnDelta * 100).toFixed(1)}%  (want > 10.0%)`);

const fails = [];
if (!(nightRatio < 0.4)) {
  fails.push(`night deck is not dark: night/noon = ${nightRatio.toFixed(3)}, want < 0.4`);
}
if (!(dawnDelta > 0.1)) {
  fails.push(`dawn deck does not differ from noon: ${(dawnDelta * 100).toFixed(1)}%, want > 10%`);
}

if (fails.length) {
  console.error("\nFAIL");
  for (const f of fails) console.error("  " + f);
  console.error(
    "\nIf all three are near-identical, the homestead lighting is not following\n" +
      "the game clock - which is the exact defect these vantages exist to catch."
  );
  process.exit(1);
}
console.log("\nPASS: the hull deck responds to the game clock.");
