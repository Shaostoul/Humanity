#!/usr/bin/env node
// Cloud-depth acceptance gates (docs/design/clouds-depth.md): measure a
// probe capture instead of eyeballing it.
//
//   node scripts/measure-cloud-depth.mjs <capture.png> [--top=0.45]
//
// Reports, over the cloud mask in the top fraction of the frame:
//   - recovered pre-ACES radiance p95/p05 ratio (gate: >= 6.0; the flat
//     v0.1152 deck measured 2.2-2.8)
//   - high-pass detail energy at sigma 11 px and 32 px of display-space
//     luminance (gates: >= 0.05 / >= 0.08; v0.1152 from below measured
//     0.0093 / 0.025 while the SAME build from above measured 0.14/0.21,
//     i.e. all structure was top-surface)
//
// The mask keeps low-saturation (grey/white) pixels - clouds - and drops
// blue sky. Run the same script on before/after captures for the honest
// A/B; the numbers are method-sensitive, the RATIO between builds is not.
//
// Zero-dependency PNG reader (8-bit RGB/RGBA, filters 0-4), same recipe as
// scripts/fetch-region-dem.mjs.

import { readFileSync } from "node:fs";
import zlib from "node:zlib";

function decodePng(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error("not a PNG");
  let pos = 8;
  let w = 0, h = 0, bitDepth = 0, colorType = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === "IHDR") {
      w = data.readUInt32BE(0);
      h = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      if (bitDepth !== 8) throw new Error(`bit depth ${bitDepth} unsupported`);
      if (colorType !== 2 && colorType !== 6)
        throw new Error(`color type ${colorType} unsupported`);
      if (data[12] !== 0) throw new Error("interlaced PNG unsupported");
    } else if (type === "IDAT") {
      idat.push(data);
    } else if (type === "IEND") {
      break;
    }
    pos += 12 + len;
  }
  const bpp = colorType === 6 ? 4 : 3;
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = w * bpp;
  const out = new Uint8Array(w * h * 4);
  let prev = new Uint8Array(stride);
  for (let y = 0; y < h; y++) {
    const filter = raw[y * (stride + 1)];
    const row = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const cur = new Uint8Array(stride);
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? cur[x - bpp] : 0;
      const b = prev[x];
      const c = x >= bpp ? prev[x - bpp] : 0;
      let v = row[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      } else if (filter !== 0) throw new Error(`filter ${filter}?`);
      cur[x] = v & 0xff;
    }
    for (let x = 0; x < w; x++) {
      out[(y * w + x) * 4] = cur[x * bpp];
      out[(y * w + x) * 4 + 1] = cur[x * bpp + 1];
      out[(y * w + x) * 4 + 2] = cur[x * bpp + 2];
      out[(y * w + x) * 4 + 3] = bpp === 4 ? cur[x * bpp + 3] : 255;
    }
    prev = cur;
  }
  return { w, h, rgba: out };
}

const srgbInv = (c) =>
  c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);

// The shader's ACES fit: y = x(2.51x + 0.03) / (x(2.43x + 0.59) + 0.14),
// monotone on [0, ~10]. Invert by bisection.
function acesInv(y) {
  if (y <= 0) return 0;
  if (y >= 1) return 10;
  let lo = 0, hi = 10;
  for (let i = 0; i < 48; i++) {
    const m = (lo + hi) / 2;
    const f = (m * (2.51 * m + 0.03)) / (m * (2.43 * m + 0.59) + 0.14);
    if (f < y) lo = m;
    else hi = m;
  }
  return (lo + hi) / 2;
}

// Separable 3-pass box blur ~ gaussian of the requested sigma.
function blur(src, w, h, sigma) {
  const r = Math.max(1, Math.round(sigma * Math.sqrt(Math.PI * 2) / 3 / 2));
  let a = Float32Array.from(src);
  let b = new Float32Array(src.length);
  for (let pass = 0; pass < 3; pass++) {
    // horizontal
    for (let y = 0; y < h; y++) {
      let acc = 0, n = 0;
      for (let x = -r; x < w; x++) {
        if (x + r < w) { acc += a[y * w + x + r]; n++; }
        if (x - r - 1 >= 0) { acc -= a[y * w + x - r - 1]; n--; }
        if (x >= 0) b[y * w + x] = acc / n;
      }
    }
    // vertical
    for (let x = 0; x < w; x++) {
      let acc = 0, n = 0;
      for (let y = -r; y < h; y++) {
        if (y + r < h) { acc += b[(y + r) * w + x]; n++; }
        if (y - r - 1 >= 0) { acc -= b[(y - r - 1) * w + x]; n--; }
        if (y >= 0) a[y * w + x] = acc / n;
      }
    }
  }
  return a;
}

const file = process.argv[2];
if (!file) {
  console.error("usage: node scripts/measure-cloud-depth.mjs <capture.png> [--top=0.45]");
  process.exit(2);
}
let topFrac = 0.45;
for (const arg of process.argv.slice(3)) {
  const m = arg.match(/^--top=([\d.]+)$/);
  if (m) topFrac = parseFloat(m[1]);
}

const { w, h, rgba } = decodePng(readFileSync(file));
const hTop = Math.max(8, Math.round(h * topFrac));

// Display luminance + cloud mask over the analysis strip.
const lum = new Float32Array(w * hTop);
const mask = new Uint8Array(w * hTop);
let maskCount = 0;
for (let y = 0; y < hTop; y++) {
  for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 4;
    const r = rgba[i] / 255, g = rgba[i + 1] / 255, b = rgba[i + 2] / 255;
    const v = Math.max(r, g, b);
    const sat = v > 1e-5 ? (v - Math.min(r, g, b)) / v : 0;
    lum[y * w + x] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    // Cloud = achromatic-ish and not pitch black. Blue sky (sat high, blue
    // dominant) and terrain (warm, saturated) drop out.
    if (sat < 0.28 && v > 0.02) {
      mask[y * w + x] = 1;
      maskCount++;
    }
  }
}
if (maskCount < w * hTop * 0.05) {
  console.log(JSON.stringify({ file, error: "cloud mask under 5% of strip", maskFrac: maskCount / (w * hTop) }));
  process.exit(1);
}

// Gate 1: recovered pre-ACES radiance p95/p05 over the mask.
const rad = [];
for (let p = 0; p < w * hTop; p++) {
  if (!mask[p]) continue;
  rad.push(acesInv(srgbInv(lum[p])));
}
rad.sort((a, b) => a - b);
const q = (f) => rad[Math.min(rad.length - 1, Math.floor(f * rad.length))];
const p05 = Math.max(q(0.05), 1e-4);
const p95 = q(0.95);
const ratio = p95 / p05;

// Gate 2: high-pass detail energy at sigma 11 and 32 px (display domain).
function highpass(sigma) {
  const bl = blur(lum, w, hTop, sigma);
  let e = 0, n = 0;
  for (let p = 0; p < w * hTop; p++) {
    if (!mask[p]) continue;
    e += Math.abs(lum[p] - bl[p]);
    n++;
  }
  return e / Math.max(n, 1);
}
const hp11 = highpass(11);
const hp32 = highpass(32);

console.log(JSON.stringify({
  file,
  size: `${w}x${h}`,
  strip: `top ${Math.round(topFrac * 100)}% (${hTop} rows)`,
  maskFrac: +(maskCount / (w * hTop)).toFixed(3),
  radiance_p05: +p05.toFixed(4),
  radiance_p95: +p95.toFixed(4),
  radiance_ratio: +ratio.toFixed(2),
  gate_ratio_6: ratio >= 6.0 ? "PASS" : "fail",
  highpass_s11: +hp11.toFixed(4),
  gate_s11_005: hp11 >= 0.05 ? "PASS" : "fail",
  highpass_s32: +hp32.toFixed(4),
  gate_s32_008: hp32 >= 0.08 ? "PASS" : "fail",
}, null, 1));
