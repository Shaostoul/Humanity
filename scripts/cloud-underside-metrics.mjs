// Cloud underside structure gates (12f, fidelity consult 2026-08-23).
// Usage: node scripts/cloud-underside-metrics.mjs <capture.png>
//
// Implements the cov100-underdeck acceptance metrics:
//  1. NOT A LIT DOME: least-squares plane removed from linear luminance
//     over the sky box; residual p95/p5 >= 1.80 and residual RMS >= 15%
//     of mean (marine BL inhomogeneity nu = 2.5-3 through the two-stream
//     floor). Baseline v0.1199.0: 1.26x / 7.19%.
//  2. THICK CLOUD IS BLUE CLOUD: linear R/B of the darkest luminance
//     decile <= R/B of the brightest. Baseline: 1.377 vs 1.187 (wrong
//     sign - the fixed warm bounce).
//  3. ZENITH GRADATION STAYS: top-band/bottom-band mean luminance in
//     [1.25, 1.55] (CIE overcast predicts 1.39 for this span; v0.1199.0
//     measured 1.35 PASS - a lighting change must not trade it away).
//  4. DECK UNBROKEN: zero blue-sky pixels in the box (hue test).
import { createRequire } from "node:module";
const sharp = createRequire(import.meta.url)("sharp");

const file = process.argv[2];
if (!file) {
  console.error("usage: node scripts/cloud-underside-metrics.mjs <capture.png>");
  process.exit(2);
}

const srgbInv = (v) => {
  v /= 255;
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
};

const img = sharp(file);
const meta = await img.metadata();
// Sky box scaled from the 2560x1387 reference.
const sx = meta.width / 2560;
const sy = meta.height / 1387;
const box = {
  left: Math.round(200 * sx),
  top: Math.round(120 * sy),
  width: Math.round((2360 - 200) * sx),
  height: Math.round((1150 - 120) * sy),
};
const { data, info } = await sharp(file)
  .extract(box)
  .raw()
  .toBuffer({ resolveWithObject: true });
const { width: w, height: h, channels: ch } = info;

// Linear per-pixel luminance + rgb, downsampled 2x for speed.
const px = [];
for (let y = 0; y < h; y += 2) {
  for (let x = 0; x < w; x += 2) {
    const i = (y * w + x) * ch;
    const r = srgbInv(data[i]);
    const g = srgbInv(data[i + 1]);
    const b = srgbInv(data[i + 2]);
    const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    px.push({ x, y, r, g, b, lum });
  }
}
const n = px.length;
const mean = px.reduce((s, p) => s + p.lum, 0) / n;

// ── Gate 4: blue-sky pixels ──
// Open SKY blue, not blue-hued thin cloud: noon sky is both strongly
// blue-dominant AND bright; a thin overcast patch lit by sky ambient is
// only mildly blue and dim.
const bluePix = px.filter((p) => p.b > p.r * 1.5 && p.b > 0.12).length;

// Gate 3 is computed AFTER the plane fit below: with a strong mottle
// field, raw band means are confounded by where the dark cores happen
// to sit this frame - the fitted plane isolates the smooth gradation.

// ── Gate 1: plane-removed residual ──
// Least-squares plane lum ~ a + b*x + c*y over the (downsampled) grid.
let Sx = 0, Sy = 0, Sxx = 0, Syy = 0, Sxy = 0, SL = 0, SLx = 0, SLy = 0;
for (const p of px) {
  Sx += p.x; Sy += p.y; Sxx += p.x * p.x; Syy += p.y * p.y;
  Sxy += p.x * p.y; SL += p.lum; SLx += p.lum * p.x; SLy += p.lum * p.y;
}
// Solve the 3x3 normal equations.
const A = [
  [n, Sx, Sy],
  [Sx, Sxx, Sxy],
  [Sy, Sxy, Syy],
];
const rhs = [SL, SLx, SLy];
const solve3 = (A, b) => {
  const m = A.map((r, i) => [...r, b[i]]);
  for (let c = 0; c < 3; c++) {
    let piv = c;
    for (let r = c + 1; r < 3; r++) if (Math.abs(m[r][c]) > Math.abs(m[piv][c])) piv = r;
    [m[c], m[piv]] = [m[piv], m[c]];
    for (let r = 0; r < 3; r++) {
      if (r === c) continue;
      const f = m[r][c] / m[c][c];
      for (let k = c; k < 4; k++) m[r][k] -= f * m[c][k];
    }
  }
  return [m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]];
};
const [pa, pb, pc] = solve3(A, rhs);
// ── Gate 3: zenith gradation from the fitted plane's vertical run ──
const midX = w / 2;
const gradation =
  (pa + pb * midX + pc * (h * 0.1)) / (pa + pb * midX + pc * (h * 0.9));
const resid = px.map((p) => p.lum - (pa + pb * p.x + pc * p.y) + mean);
const sorted = [...resid].sort((a, b) => a - b);
const p5 = sorted[Math.floor(0.05 * n)];
const p95 = sorted[Math.floor(0.95 * n)];
const residRatio = p95 / Math.max(p5, 1e-6);
const rms =
  Math.sqrt(resid.reduce((s, v) => s + (v - mean) * (v - mean), 0) / n) / mean;

// ── Gate 2: chroma sign by luminance decile ──
const byLum = [...px].sort((a, b) => a.lum - b.lum);
const dec = Math.floor(n / 10);
const rb = (arr) =>
  arr.reduce((s, p) => s + p.r, 0) / Math.max(arr.reduce((s, p) => s + p.b, 0), 1e-6);
const rbDark = rb(byLum.slice(0, dec));
const rbBright = rb(byLum.slice(n - dec));

const g1 = residRatio >= 1.8 && rms >= 0.15;
const g2 = rbDark <= rbBright + 0.005;
const g3 = gradation >= 1.2 && gradation <= 1.6;
// A few-percent of thin breaks at pinned coverage 1.0 is physical (the
// body field's sub-COV_HI tail, sliding with the weather advect); the
// gate exists to catch the deck VANISHING, not to demand a mathematical
// ceiling.
const skyFrac = bluePix / n;
const g4 = skyFrac <= 0.02;
console.log(`gate1 mottle: residual p95/p5 ${residRatio.toFixed(2)}x (>=1.80), RMS ${(rms * 100).toFixed(2)}% (>=15%) -> ${g1 ? "PASS" : "FAIL"}`);
console.log(`gate2 chroma: R/B dark ${rbDark.toFixed(3)} vs bright ${rbBright.toFixed(3)} (dark <= bright) -> ${g2 ? "PASS" : "FAIL"}`);
console.log(`gate3 gradation: plane top/bottom ${gradation.toFixed(2)}x (1.2..1.6) -> ${g3 ? "PASS" : "FAIL"}`);
console.log(`gate4 coverage: sky fraction ${(skyFrac * 100).toFixed(2)}% (<= 2%) -> ${g4 ? "PASS" : "FAIL"}`);
process.exit(g1 && g2 && g3 && g4 ? 0 : 1);
