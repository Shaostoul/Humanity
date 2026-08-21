#!/usr/bin/env node
// Lattice-periodicity scorer for ocean glint and grazing-water captures
// (environment program increment 7).
//
// The defect class: a specular lobe evaluated on piecewise-interpolated
// mesh normals prints the VERTEX LATTICE as a regular dot/band pattern
// (the "hexagonal dotted orbit glint"). A lattice is invisible to mean/
// contrast stats but unmistakable in the autocorrelation: periodic
// structure puts strong OFF-ORIGIN peaks at the lattice period, while an
// aperiodic glitter field decays monotonically from the origin.
//
//   node scripts/glint-autocorr.mjs <cap.png> [--crop x,y,w,h] [--auto]
//        [--lag N] [--hp K] [--json]
//
// --auto centres a 192x192 window on the brightest 8x8 block (the glint).
// --hp K = box high-pass kernel radius (default 4, i.e. 9x9): removes the
//   glint's smooth envelope so only texel/lattice-scale structure remains.
// Reports r(0,0) (dc), and every LOCAL-MAX autocorr peak at lag radius
// >= 4, as a fraction of dc. GATE (program doc, increment 7): every
// off-origin peak < 0.20 of dc. Pre-fix orbit glint measured 0.89.

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const sharp = require('sharp');

const args = process.argv.slice(2);
const files = args.filter(
  (a, i) =>
    !a.startsWith('--') &&
    args[i - 1] !== '--crop' &&
    args[i - 1] !== '--lag' &&
    args[i - 1] !== '--hp',
);
const asJson = args.includes('--json');
const auto = args.includes('--auto');
const lagMax = (() => {
  const i = args.indexOf('--lag');
  return i < 0 ? 28 : Math.max(8, Number(args[i + 1]) | 0);
})();
const hpR = (() => {
  const i = args.indexOf('--hp');
  return i < 0 ? 4 : Math.max(1, Number(args[i + 1]) | 0);
})();
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

if (files.length !== 1) {
  console.error(
    'usage: glint-autocorr.mjs <cap.png> [--crop x,y,w,h | --auto] [--lag N] [--hp K] [--json]',
  );
  process.exit(2);
}

const img = sharp(files[0]);
const meta = await img.metadata();

let crop = cropArg;
if (!crop && auto) {
  // Find the brightest 8x8 block on a small proxy, then centre 192x192 on it.
  const proxyW = 256;
  const proxyH = Math.round((meta.height / meta.width) * proxyW);
  const { data } = await img
    .clone()
    .resize(proxyW, proxyH, { kernel: 'cubic' })
    .grayscale()
    .raw()
    .toBuffer({ resolveWithObject: true });
  let best = -1;
  let bx = 0;
  let by = 0;
  for (let y = 0; y < proxyH - 2; y += 2) {
    for (let x = 0; x < proxyW - 2; x += 2) {
      const s =
        data[y * proxyW + x] +
        data[y * proxyW + x + 1] +
        data[(y + 1) * proxyW + x] +
        data[(y + 1) * proxyW + x + 1];
      if (s > best) {
        best = s;
        bx = x;
        by = y;
      }
    }
  }
  const cx = Math.round(((bx + 1) / proxyW) * meta.width);
  const cy = Math.round(((by + 1) / proxyH) * meta.height);
  const w = 192;
  crop = {
    left: Math.max(0, Math.min(meta.width - w, cx - w / 2)),
    top: Math.max(0, Math.min(meta.height - w, cy - w / 2)),
    width: w,
    height: w,
  };
}
if (!crop) {
  console.error('need --crop or --auto');
  process.exit(2);
}

const { data, info } = await img
  .extract(crop)
  .grayscale()
  .raw()
  .toBuffer({ resolveWithObject: true });
const W = info.width;
const H = info.height;

// High-pass: pixel minus box mean (radius hpR), computed via integral image.
const ii = new Float64Array((W + 1) * (H + 1));
for (let y = 0; y < H; y++) {
  let row = 0;
  for (let x = 0; x < W; x++) {
    row += data[y * W + x];
    ii[(y + 1) * (W + 1) + (x + 1)] = ii[y * (W + 1) + (x + 1)] + row;
  }
}
const f = new Float64Array(W * H);
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const x0 = Math.max(0, x - hpR);
    const y0 = Math.max(0, y - hpR);
    const x1 = Math.min(W - 1, x + hpR) + 1;
    const y1 = Math.min(H - 1, y + hpR) + 1;
    const n = (x1 - x0) * (y1 - y0);
    const s =
      ii[y1 * (W + 1) + x1] -
      ii[y0 * (W + 1) + x1] -
      ii[y1 * (W + 1) + x0] +
      ii[y0 * (W + 1) + x0];
    f[y * W + x] = data[y * W + x] - s / n;
  }
}

// Autocorrelation over lags [-lagMax, lagMax]^2 (dy >= 0 half-plane; the
// function is symmetric under lag negation).
const L = lagMax;
const side = 2 * L + 1;
const r = new Float64Array(side * side).fill(NaN);
for (let dy = 0; dy <= L; dy++) {
  for (let dx = -L; dx <= L; dx++) {
    if (dy === 0 && dx < 0) continue;
    let acc = 0;
    let n = 0;
    const x0 = Math.max(0, -dx);
    const x1 = Math.min(W, W - dx);
    for (let y = 0; y < H - dy; y++) {
      const a = y * W;
      const b = (y + dy) * W + dx;
      for (let x = x0; x < x1; x++) {
        acc += f[a + x] * f[b + x];
        n++;
      }
    }
    const v = acc / Math.max(n, 1);
    r[(dy + L) * side + (dx + L)] = v;
    r[(L - dy) * side + (L - dx)] = v;
  }
}
const dc = r[L * side + L];

// Local maxima at lag radius >= 4.
const peaks = [];
for (let dy = -L + 1; dy < L; dy++) {
  for (let dx = -L + 1; dx < L; dx++) {
    if (dx * dx + dy * dy < 16) continue;
    const v = r[(dy + L) * side + (dx + L)];
    let isMax = true;
    for (let oy = -1; oy <= 1 && isMax; oy++) {
      for (let ox = -1; ox <= 1; ox++) {
        if (ox === 0 && oy === 0) continue;
        if (r[(dy + oy + L) * side + (dx + ox + L)] > v) {
          isMax = false;
          break;
        }
      }
    }
    if (isMax && v > 0) peaks.push({ dx, dy, frac: v / dc });
  }
}
peaks.sort((a, b) => b.frac - a.frac);
const top = peaks.slice(0, 6);
const worst = top.length ? top[0].frac : 0;

const out = {
  file: files[0],
  crop,
  dc: +dc.toFixed(2),
  worst_offorigin_peak_frac: +worst.toFixed(4),
  gate_020: worst < 0.2 ? 'PASS' : 'FAIL',
  top_peaks: top.map((p) => ({ dx: p.dx, dy: p.dy, frac: +p.frac.toFixed(4) })),
};
if (asJson) console.log(JSON.stringify(out, null, 1));
else {
  console.log(
    `${files[0]}  crop=${crop.left},${crop.top},${crop.width},${crop.height}  dc=${out.dc}`,
  );
  console.log(
    `worst off-origin peak = ${(worst * 100).toFixed(1)}% of dc  [gate <20%: ${out.gate_020}]`,
  );
  for (const p of out.top_peaks)
    console.log(`  peak (${p.dx},${p.dy}) = ${(p.frac * 100).toFixed(1)}%`);
}
process.exit(worst < 0.2 ? 0 : 1);
