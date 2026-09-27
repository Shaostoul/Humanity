// Straight-stripe detector: how much of a crop's detail energy sits in ONE
// narrow spatial frequency (a straight parallel grating) rather than spread
// over many (snaking crests, glitter).
//
//   node scripts/ocean-stripe-metric.mjs <cap.png> --crop x,y [--n 512] [--hp 24] [--pmin 12] [--pmax 90]
//
// Written 2026-09-27 for the parallel stripes across the sun glint at
// deck-55-nadir (the 850 m wave train drawn with straight crests). Readings on
// that fixture, crop 1000,650: share3 0.59 with the defect, 0.05 with the
// coarse crest warp running wherever a train is drawn. An aperiodic glint sits
// near 0.05; a grating sits far above 0.2. Pick a crop inside the glint: away
// from it the ring energy is tiny and the ratio means little.
//
// Steps: grayscale NxN crop at (x,y), high-pass (pixel minus box mean of
// radius hp, removes the glint envelope), Hann window, 2D FFT, power. In the
// ring of periods pmin..pmax px it reports:
//   peak      the strongest bin's power / median ring power
//   share3    fraction of ring energy in the strongest 3x3 bin neighbourhood
//   period, angle of the strongest bin
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : d; };
const file = args[0];
const [cx, cy] = opt("--crop", "0,0").split(",").map(Number);
const N = Number(opt("--n", "512"));
const hp = Number(opt("--hp", "24"));
const pmin = Number(opt("--pmin", "12"));
const pmax = Number(opt("--pmax", "90"));

const { data } = await sharp(file).extract({ left: cx, top: cy, width: N, height: N })
  .grayscale().raw().toBuffer({ resolveWithObject: true });
const W = N;
const ii = new Float64Array((W + 1) * (W + 1));
for (let y = 0; y < W; y++) { let row = 0; for (let x = 0; x < W; x++) { row += data[y * W + x]; ii[(y + 1) * (W + 1) + x + 1] = ii[y * (W + 1) + x + 1] + row; } }
const re = new Float64Array(W * W), im = new Float64Array(W * W);
for (let y = 0; y < W; y++) for (let x = 0; x < W; x++) {
  const x0 = Math.max(0, x - hp), y0 = Math.max(0, y - hp), x1 = Math.min(W - 1, x + hp) + 1, y1 = Math.min(W - 1, y + hp) + 1;
  const s = ii[y1 * (W + 1) + x1] - ii[y0 * (W + 1) + x1] - ii[y1 * (W + 1) + x0] + ii[y0 * (W + 1) + x0];
  const hv = data[y * W + x] - s / ((x1 - x0) * (y1 - y0));
  const wx = 0.5 - 0.5 * Math.cos(2 * Math.PI * x / (W - 1)), wy = 0.5 - 0.5 * Math.cos(2 * Math.PI * y / (W - 1));
  re[y * W + x] = hv * wx * wy;
}
function fft1(r, i, n, off, stride) {
  // iterative radix-2 on a strided line
  const R = new Float64Array(n), I = new Float64Array(n);
  for (let k = 0; k < n; k++) { R[k] = r[off + k * stride]; I[k] = i[off + k * stride]; }
  for (let a = 1, j = 0; a < n; a++) { let bit = n >> 1; for (; j & bit; bit >>= 1) j ^= bit; j ^= bit; if (a < j) { [R[a], R[j]] = [R[j], R[a]]; [I[a], I[j]] = [I[j], I[a]]; } }
  for (let len = 2; len <= n; len <<= 1) {
    const ang = -2 * Math.PI / len, wr = Math.cos(ang), wi = Math.sin(ang);
    for (let s = 0; s < n; s += len) { let cr = 1, ci = 0; for (let k = 0; k < len / 2; k++) {
      const ar = R[s + k], ai = I[s + k], br = R[s + k + len / 2] * cr - I[s + k + len / 2] * ci, bi = R[s + k + len / 2] * ci + I[s + k + len / 2] * cr;
      R[s + k] = ar + br; I[s + k] = ai + bi; R[s + k + len / 2] = ar - br; I[s + k + len / 2] = ai - bi;
      const t = cr * wr - ci * wi; ci = cr * wi + ci * wr; cr = t; } }
  }
  for (let k = 0; k < n; k++) { r[off + k * stride] = R[k]; i[off + k * stride] = I[k]; }
}
for (let y = 0; y < W; y++) fft1(re, im, W, y * W, 1);
for (let x = 0; x < W; x++) fft1(re, im, W, x, W);
const P = (u, v) => { const uu = (u + W) % W, vv = (v + W) % W; const k = vv * W + uu; return re[k] * re[k] + im[k] * im[k]; };
const ring = [];
let best = { p: -1 };
let total = 0;
for (let v = -W / 2; v < W / 2; v++) for (let u = 0; u < W / 2; u++) {
  if (u === 0 && v <= 0) continue;
  const f = Math.hypot(u, v) / W; if (f <= 0) continue;
  const per = 1 / f; if (per < pmin || per > pmax) continue;
  const p = P(u, v); ring.push(p); total += p;
  if (p > best.p) best = { p, u, v, per, ang: Math.atan2(v, u) * 180 / Math.PI };
}
ring.sort((a, b) => a - b);
const med = ring[Math.floor(ring.length / 2)];
let nb = 0; for (let dv = -1; dv <= 1; dv++) for (let du = -1; du <= 1; du++) nb += P(best.u + du, best.v + dv);
console.log(JSON.stringify({ file: file.split(/[\\/]/).pop(), peak: +(best.p / med).toFixed(1), share3: +(nb / total).toFixed(4), period_px: +best.per.toFixed(1), angle_deg: +best.ang.toFixed(1), ring_energy: +(total / ring.length).toExponential(3) }));
