#!/usr/bin/env node
// Score a descent-ladder sweep (environment program increment 1).
//
//   node scripts/ladder-score.mjs <sweep-dir> [--json]
//
// The methodology, from the council's verification design: every rung
// has TWO captures a few seconds apart - the CONTROL pair, measuring
// how much two frames differ from temporal noise (cloud EMA churn,
// waves, dither) with the camera fixed. Adjacent rungs that bracket a
// known altitude boundary form the TEST pair. A boundary PASSES when
//
//   boundary MAD <= 1.5 x max(control MAD at either rung)   AND
//   no connected changed region larger than 0.5% of the frame
//
// i.e. crossing the boundary may not change the image more than merely
// waiting does, and whatever small change exists must not be one
// coherent OBJECT appearing/vanishing (a pop) - diffuse noise spread
// over the frame is fine, a popped cloud deck is not.
//
// CALIBRATION-RED REQUIREMENT: on v0.1168 this scorer MUST fail the
// 331 km, 191 km and 15.8 km boundaries (the audited discontinuities).
// A ladder that passes v0.1168 is a broken ladder - fix the harness
// before trusting a single green from it.

import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
const require = createRequire(import.meta.url);
const sharp = require('sharp');

const args = process.argv.slice(2);
const dir = args.find((a) => !a.startsWith('--'));
const asJson = args.includes('--json');
if (!dir) {
  console.error('usage: ladder-score.mjs <sweep-dir> [--json]');
  process.exit(2);
}
const manifest = JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8'));
const lad = manifest.ladder;
if (!lad) {
  console.error('manifest has no ladder spec - was this a --ladder sweep?');
  process.exit(2);
}

const rungId = (r) => `ladder-${String(r).replace('.', '_')}km`;

// Load a capture downsampled 4x as raw luminance; the HUD band and frame
// edges are cropped out. Downsampling makes the connected-region flood
// fill cheap and suppresses sub-pixel dither.
async function lum(file) {
  const img = sharp(file);
  const meta = await img.metadata();
  const crop = {
    left: Math.round(meta.width * 0.06),
    top: 110,
    width: Math.round(meta.width * 0.88),
    height: meta.height - 110 - 40,
  };
  const W = Math.round(crop.width / 4);
  const H = Math.round(crop.height / 4);
  const { data } = await img
    .extract(crop)
    .resize(W, H, { kernel: 'lanczos3' })
    .greyscale()
    .raw()
    .toBuffer({ resolveWithObject: true });
  return { data, W, H };
}

function mad(a, b) {
  let s = 0;
  const n = Math.min(a.data.length, b.data.length);
  for (let i = 0; i < n; i++) s += Math.abs(a.data[i] - b.data[i]);
  return s / n;
}

// Largest connected component of |a-b| > thresh, as a fraction of the
// frame. 4-connected BFS on the downsampled luminance delta.
function largestPop(a, b, thresh) {
  const { W, H } = a;
  const mask = new Uint8Array(W * H);
  for (let i = 0; i < W * H; i++) mask[i] = Math.abs(a.data[i] - b.data[i]) > thresh ? 1 : 0;
  const seen = new Uint8Array(W * H);
  let best = 0;
  const qx = new Int32Array(W * H);
  const qy = new Int32Array(W * H);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      const i0 = y * W + x;
      if (!mask[i0] || seen[i0]) continue;
      let head = 0,
        tail = 0,
        size = 0;
      qx[tail] = x;
      qy[tail] = y;
      tail++;
      seen[i0] = 1;
      while (head < tail) {
        const cx = qx[head],
          cy = qy[head];
        head++;
        size++;
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const nx = cx + dx,
            ny = cy + dy;
          if (nx < 0 || ny < 0 || nx >= W || ny >= H) continue;
          const ni = ny * W + nx;
          if (mask[ni] && !seen[ni]) {
            seen[ni] = 1;
            qx[tail] = nx;
            qy[tail] = ny;
            tail++;
          }
        }
      }
      if (size > best) best = size;
    }
  return best / (W * H);
}

const cache = new Map();
async function pair(r) {
  const key = String(r);
  if (!cache.has(key)) {
    const a = await lum(path.join(dir, `${rungId(r)}a.png`));
    const b = await lum(path.join(dir, `${rungId(r)}b.png`));
    cache.set(key, { a, b, control: mad(a, b) });
  }
  return cache.get(key);
}

const out = { sweep: dir, boundaries: [], controls: {} };
let failures = 0;
for (const bd of lad.boundaries) {
  try {
    const lo = await pair(bd.lo_km);
    const hi = await pair(bd.hi_km);
    const test = mad(lo.a, hi.a);
    const control = Math.max(lo.control, hi.control);
    const pop = largestPop(lo.a, hi.a, 12);
    const pass = test <= control * 1.5 && pop <= 0.005;
    if (!pass) failures++;
    out.boundaries.push({
      name: bd.name,
      lo_km: bd.lo_km,
      hi_km: bd.hi_km,
      control_mad: +control.toFixed(3),
      test_mad: +test.toFixed(3),
      ratio: +(test / Math.max(control, 1e-6)).toFixed(2),
      largest_pop_frac: +pop.toFixed(5),
      pass,
    });
    out.controls[bd.lo_km] = +lo.control.toFixed(3);
    out.controls[bd.hi_km] = +hi.control.toFixed(3);
  } catch (e) {
    failures++;
    out.boundaries.push({ name: bd.name, error: String(e.message), pass: false });
  }
}
out.pass = failures === 0;

if (asJson) console.log(JSON.stringify(out, null, 1));
else {
  for (const b of out.boundaries) {
    if (b.error) {
      console.log(`FAIL  ${b.name}: ${b.error}`);
      continue;
    }
    console.log(
      `${b.pass ? 'pass' : 'FAIL'}  ${b.name}  test MAD ${b.test_mad} vs control ${b.control_mad}` +
        ` (ratio ${b.ratio}, limit 1.5)  largest pop ${(b.largest_pop_frac * 100).toFixed(2)}% (limit 0.5%)`,
    );
  }
  console.log(out.pass ? 'LADDER PASS' : `LADDER FAIL (${failures} boundary/ies)`);
}
process.exit(out.pass ? 0 : 1);
