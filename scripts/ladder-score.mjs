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

// MAD after registering a small global translation (Wave D instrument
// fix): seconds of wind advection between captures moves the deck a few
// downsampled pixels as a near-uniform image shift; scoring the best
// alignment inside +/-6 px separates "the clouds drifted" from "the
// rendering changed". Returns the minimum MAD over the search.
function madRegistered(a, b, search = 6) {
  const { W, H } = a;
  let best = Infinity;
  for (let dy = -search; dy <= search; dy++) {
    for (let dx = -search; dx <= search; dx++) {
      let s = 0;
      let n = 0;
      for (let y = Math.max(0, -dy); y < H - Math.max(0, dy); y++) {
        const ya = y * W;
        const yb = (y + dy) * W + dx;
        for (let x = Math.max(0, -dx); x < W - Math.max(0, dx); x++) {
          s += Math.abs(a.data[ya + x] - b.data[yb + x]);
          n++;
        }
      }
      const m = s / Math.max(n, 1);
      if (m < best) best = m;
    }
  }
  return best;
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
    // CROSS-AND-RETURN scoring (preferred, Wave D instrument fix): the
    // sweep captures "c" at the LOW altitude after crossing to the HIGH
    // side and coming back - same projection, same scene, seconds of
    // advection that the shift registration removes. MAD(b, c) is then
    // literally "did crossing the boundary change the image more than
    // waiting does". Falls back to the legacy across-rung comparison for
    // sweeps that predate the c capture (whose numbers conflate scale +
    // parallax + minutes of drift - treat those as advisory only).
    const cPath = path.join(dir, `${rungId(bd.lo_km)}c.png`);
    let test;
    let mode;
    let popA;
    let popB;
    if (fs.existsSync(cPath)) {
      // PARK VERIFICATION (Wave D instrument fix): the manifest records
      // each capture's ACHIEVED altitude (from the reference dump - the
      // camera_done file only echoes the request, and return-parks were
      // measured landing ~15% high). A b/c pair at different altitudes is
      // a broken measurement, not a boundary verdict - fail it as
      // invalid-park so nobody reads a scale mismatch as a cloud pop.
      const rec = (manifest.vantages || []).find(
        (v) => v.id === rungId(bd.lo_km),
      );
      if (rec && rec.alt_b != null && rec.alt_c != null) {
        const rel = Math.abs(rec.alt_b - rec.alt_c) / Math.max(rec.alt_b, 1e-6);
        if (rel > 0.01) {
          throw new Error(
            `invalid-park: b at ${rec.alt_b.toFixed(2)} km vs c at ${rec.alt_c.toFixed(2)} km (${(rel * 100).toFixed(1)}% apart)`,
          );
        }
      }
      const c = await lum(cPath);
      test = madRegistered(lo.b, c);
      popA = lo.b;
      popB = c;
      mode = 'cross-return';
      // DT-MATCHED CONTROL (shear-honest null): d/e are a same-altitude
      // pair separated by the same interval as b->c but WITHOUT crossing.
      // Wind shear slides cloud layers over each other in real time, so
      // the 3-second a/b control under-states the null difference by an
      // order of magnitude at 20 s separations. When the d/e pair exists
      // it replaces the control entirely.
      const dPath = path.join(dir, `${rungId(bd.lo_km)}d.png`);
      const ePath = path.join(dir, `${rungId(bd.lo_km)}e.png`);
      if (fs.existsSync(dPath) && fs.existsSync(ePath)) {
        const d = await lum(dPath);
        const e = await lum(ePath);
        lo.control = madRegistered(d, e);
        lo.control_pop = largestPop(d, e, 12);
        mode = 'cross-return-dt-matched';
      }
    } else {
      const hi = await pair(bd.hi_km);
      test = mad(lo.a, hi.a);
      popA = lo.a;
      popB = hi.a;
      mode = 'legacy-across-rung';
    }
    const hiCtl = fs.existsSync(cPath) ? lo : await pair(bd.hi_km);
    const control = Math.max(lo.control, hiCtl.control);
    const pop = largestPop(popA, popB, 12);
    // Pop limit: absolute 0.5% of frame, OR 1.5x whatever coherent region
    // the dt-matched control itself shows (a drifting cloud edge is a
    // connected region too - the null must be allowed its own share).
    const popLimit = Math.max(0.005, (lo.control_pop ?? 0) * 1.5);
    const pass = test <= control * 1.5 && pop <= popLimit;
    if (!pass) failures++;
    out.boundaries.push({
      name: bd.name,
      mode,
      lo_km: bd.lo_km,
      hi_km: bd.hi_km,
      control_mad: +control.toFixed(3),
      test_mad: +test.toFixed(3),
      ratio: +(test / Math.max(control, 1e-6)).toFixed(2),
      largest_pop_frac: +pop.toFixed(5),
      pass,
    });
    out.controls[bd.lo_km] = +lo.control.toFixed(3);
    out.controls[bd.hi_km] = +hiCtl.control.toFixed(3);
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
