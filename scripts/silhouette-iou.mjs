#!/usr/bin/env node
// Silhouette-stability scorer for the Wave B cloud ladder (environment
// program increment 9).
//
// The defect: a cloud continuously RESHAPES as the camera approaches,
// because the hard carve threshold answers all-or-nothing per mip level
// and the mip blend moves with distance. Three nadir captures of the SAME
// deck patch from 5/11/35 km (silhouette-5km/11km/35km vantages) let us
// measure it: binarize each to a cloud mask, rescale the far rungs'
// central crops to the near rung's ground frame (nadir + same lat/lon =
// concentric), and report pairwise IoU.
//
//   node scripts/silhouette-iou.mjs <near.png> <mid.png> <far.png>
//        [--alts 5,11,35] [--deck 3] [--json]
//
// GATE (program doc, increment 9): IoU(near vs far) >= 0.85, with the
// pre-fix baseline measured red first.

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const sharp = require('sharp');

const args = process.argv.slice(2);
const files = args.filter(
  (a, i) => !a.startsWith('--') && args[i - 1] !== '--alts' && args[i - 1] !== '--deck',
);
const asJson = args.includes('--json');
const alts = (() => {
  const i = args.indexOf('--alts');
  return (i < 0 ? '5,11,35' : args[i + 1]).split(',').map(Number);
})();
const deck = (() => {
  const i = args.indexOf('--deck');
  return i < 0 ? 3.0 : Number(args[i + 1]);
})();

if (files.length !== 3 || alts.length !== 3) {
  console.error('usage: silhouette-iou.mjs <near.png> <mid.png> <far.png> [--alts 5,11,35] [--deck 3]');
  process.exit(2);
}

const SIZE = 512; // common mask resolution

async function mask(file, altKm) {
  const img = sharp(file);
  const meta = await img.metadata();
  // Central crop covering the NEAR rung's ground patch: the near rung's
  // central 55% of frame maps to a crop of 0.55 * (near_h / this_h) of
  // this frame (heights above the deck reference).
  const hNear = alts[0] - deck;
  const hThis = altKm - deck;
  const frac = 0.55 * (hNear / hThis);
  const w = Math.round(meta.width * frac);
  const h = Math.round(meta.height * frac);
  const left = Math.round((meta.width - w) / 2);
  const top = Math.round((meta.height - h) / 2);
  const { data, info } = await img
    .extract({ left, top, width: w, height: h })
    .resize(SIZE, SIZE, { kernel: 'cubic' })
    .raw()
    .toBuffer({ resolveWithObject: true });
  const ch = info.channels;
  const m = new Uint8Array(SIZE * SIZE);
  for (let i = 0; i < SIZE * SIZE; i++) {
    const r = data[i * ch];
    const g = data[i * ch + 1];
    const b = data[i * ch + 2];
    const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    // Cloud = bright AND neutral-to-cool (terrain clearings are tan,
    // R > B; forests/water are dark). Same rule for every rung, so the
    // comparison is self-consistent even if the absolute rule is crude.
    m[i] = lum > 140 && b >= r * 0.8 ? 1 : 0;
  }
  return m;
}

function iou(a, b) {
  let inter = 0;
  let uni = 0;
  for (let i = 0; i < a.length; i++) {
    if (a[i] & b[i]) inter++;
    if (a[i] | b[i]) uni++;
  }
  return uni === 0 ? 1 : inter / uni;
}

const masks = [];
for (let i = 0; i < 3; i++) masks.push(await mask(files[i], alts[i]));
let cover = masks.map(
  (m) => +(m.reduce((s, v) => s + v, 0) / m.length).toFixed(3),
);
// DEGENERACY GUARD (increment 11): IoU only discriminates when the shared
// ground patch holds MIXED structure. At extreme covers the dominant
// phase matches itself trivially (a 95% mask scores ~0.9 against any
// other 95% mask - measured: inverting phases let the known-red baseline
// PASS at 0.939, the checks-that-cannot-fail class). Refuse instead:
// re-aim the vantage column at a deck edge so every rung sees cloud AND
// clear in the compared patch.
const phase = 'cloud';
if (cover[0] < 0.15 || cover[0] > 0.85) {
  console.error(
    `DEGENERATE: near-rung cloud cover ${cover[0]} - the compared patch is (nearly) all one phase; ` +
      `re-aim the silhouette vantage column at mixed deck/lane structure. No verdict.`,
  );
  process.exit(3);
}
const out = {
  files,
  alts,
  deck,
  phase,
  cover,
  iou_near_mid: +iou(masks[0], masks[1]).toFixed(3),
  iou_mid_far: +iou(masks[1], masks[2]).toFixed(3),
  iou_near_far: +iou(masks[0], masks[2]).toFixed(3),
};
out.gate_085 = out.iou_near_far >= 0.85 ? 'PASS' : 'FAIL';
if (asJson) console.log(JSON.stringify(out, null, 1));
else {
  console.log(`phase ${phase}; cover per rung: ${cover.join(' / ')}`);
  console.log(
    `IoU near-mid ${out.iou_near_mid}  mid-far ${out.iou_mid_far}  near-far ${out.iou_near_far}  [gate >= 0.85: ${out.gate_085}]`,
  );
}
process.exit(out.iou_near_far >= 0.85 ? 0 : 1);
