#!/usr/bin/env node
// Sky/ocean capture statistics for verification gates (v0.1169).
//
// Born in the 2026-08-18/19 cloud arc, where every eyeball judgement
// failed and every measured one held: the "dots" were diagnosed by
// autocorrelation, the phase-9 regression showed as contrast p95/p05
// falling 1.69 -> 1.46 while the speckle metric improved, and the ocean
// blowout hid because no capture ever measured a storm sea. This script
// makes those numbers one command so probe gates can assert them.
//
//   node scripts/measure-sky.mjs <capture.png> [--crop x,y,w,h] [--json]
//   node scripts/measure-sky.mjs a.png b.png [--crop ...] [--json]   (A/B delta)
//
// Reported per image (over the crop, default: the sky band above the
// horizon and clear of the HUD): mean luminance, p05/p50/p95 and the
// contrast ratio p95/p05, SPECKLE rms (pixel minus 3x3 box mean - the
// texel-scale grain the operator calls static), and the R/B ratio
// (warmth; collapses when something multiplies sky colour twice).
// With two images: each one's stats plus the mean absolute pixel delta,
// the A/B "no-pop" number for altitude-boundary pairs.

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const sharp = require('sharp');

const args = process.argv.slice(2);
// Option VALUES (the token after --crop) must not be mistaken for files.
const files = args.filter(
  (a, i) => !a.startsWith('--') && args[i - 1] !== '--crop',
);
const asJson = args.includes('--json');
const cropArg = (() => {
  const i = args.indexOf('--crop');
  if (i < 0) return null;
  const [x, y, w, h] = (args[i + 1] || '').split(',').map(Number);
  if ([x, y, w, h].some((v) => !Number.isFinite(v))) {
    console.error('bad --crop, want x,y,w,h');
    process.exit(2);
  }
  return { left: x, top: y, width: w, height: h };
})();

if (files.length < 1 || files.length > 2) {
  console.error('usage: measure-sky.mjs <a.png> [b.png] [--crop x,y,w,h] [--json]');
  process.exit(2);
}

async function grab(file) {
  const img = sharp(file);
  const meta = await img.metadata();
  // Default crop: the sky band - clear of the top HUD (~110 px) and the
  // horizon/terrain (lower ~45%), inset from the frame edges.
  const crop = cropArg || {
    left: Math.round(meta.width * 0.08),
    top: 110,
    width: Math.round(meta.width * 0.84),
    height: Math.round(meta.height * 0.55) - 110,
  };
  const { data, info } = await img
    .extract(crop)
    .raw()
    .toBuffer({ resolveWithObject: true });
  return { data, info, crop };
}

function stats({ data, info }) {
  const { width: W, height: H, channels: ch } = info;
  const N = W * H;
  const lum = new Float64Array(N);
  let rs = 0,
    bs = 0;
  for (let i = 0; i < N; i++) {
    const r = data[i * ch],
      g = data[i * ch + 1],
      b = data[i * ch + 2];
    lum[i] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    rs += r;
    bs += b;
  }
  let hp = 0,
    n = 0;
  for (let y = 1; y < H - 1; y++)
    for (let x = 1; x < W - 1; x++) {
      let s = 0;
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++) s += lum[(y + dy) * W + (x + dx)];
      const d = lum[y * W + x] - s / 9;
      hp += d * d;
      n++;
    }
  const sorted = Float64Array.from(lum).sort();
  const q = (f) => sorted[Math.floor(f * (N - 1))];
  return {
    mean_l: +(lum.reduce((a, b) => a + b, 0) / N).toFixed(2),
    p05: +q(0.05).toFixed(1),
    p50: +q(0.5).toFixed(1),
    p95: +q(0.95).toFixed(1),
    contrast: +(q(0.95) / Math.max(q(0.05), 1)).toFixed(3),
    speckle_rms: +Math.sqrt(hp / n).toFixed(3),
    r_over_b: +(rs / Math.max(bs, 1)).toFixed(4),
  };
}

function absDelta(a, b) {
  if (a.data.length !== b.data.length) return null;
  let s = 0;
  for (let i = 0; i < a.data.length; i++) s += Math.abs(a.data[i] - b.data[i]);
  return +(s / a.data.length).toFixed(3);
}

const imgs = await Promise.all(files.map(grab));
const out = {};
files.forEach((f, i) => {
  out[f] = stats(imgs[i]);
});
if (imgs.length === 2) {
  out.ab_mean_abs_delta = absDelta(imgs[0], imgs[1]);
}
out.crop = imgs[0].crop;

if (asJson) {
  console.log(JSON.stringify(out, null, 1));
} else {
  for (const [k, v] of Object.entries(out)) {
    if (k === 'crop' || k === 'ab_mean_abs_delta') continue;
    console.log(
      `${k}\n  mean L ${v.mean_l}  p05/p50/p95 ${v.p05}/${v.p50}/${v.p95}` +
        `  contrast ${v.contrast}  SPECKLE ${v.speckle_rms}  R/B ${v.r_over_b}`,
    );
  }
  if (out.ab_mean_abs_delta != null)
    console.log(`A/B mean abs pixel delta: ${out.ab_mean_abs_delta}`);
}
