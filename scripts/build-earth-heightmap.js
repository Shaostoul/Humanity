#!/usr/bin/env node
// Build data/planets/earth_heightmap.bin from NOAA ETOPO1 (public domain).
//
// Source: ETOPO1 Global Relief Model, ice-surface, cell-registered, flat
// binary int16 ("etopo1_ice_c_i2.bin" from
// https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO1/data/ice_surface/cell_registered/binary/etopo1_ice_c_i2.zip
// ~340 MB zip). ETOPO1 is a US government work: public domain, free to ship.
// The raw grid is 21600 x 10800 int16 little-endian meters (1 arc-minute
// cells), row-major from the NW corner: row 0 = northernmost band of cells,
// col 0 = westernmost; cell (r, c) centered at
//   lat = 90 - (r + 0.5)/60,  lon = -180 + (c + 0.5)/60.
// (The grid-registered variant is 21601 x 10801 node samples; this script
// detects it by file size and handles it too, treating node values as cell
// values -- the half-arc-minute center shift is ~0.9 km, invisible at the
// 0.1-degree output resolution.)
//
// Processing:
//   1. AVERAGE 6x6 blocks (not stride-subsample: averaging is a proper
//      box filter, so coastlines and ridges keep their mass instead of
//      aliasing on whichever single sample the stride lands on)
//      -> 3600 x 1800 grid of 0.1-degree cells.
//   2. QUANTIZE to u16 over a FIXED, documented window of -11000..+6500 m
//      (not the per-run data min/max: a fixed window keeps the encoding
//      reproducible and sea level at a known quantum). The window TOP hugs
//      the AVERAGED data max (6394.5 m -- 0.1-degree cells smooth the 8849 m
//      summit spike away) instead of raw-peak headroom, so the color
//      classifier's upper bands (mountain gray / peak white) map onto real
//      Himalaya/Andes cells rather than empty range, and each u16 quantum is
//      finer: 17500/65535 = ~0.267 m. The BOTTOM keeps margin under the
//      averaged min (-10427.8 m). Anything outside clamps (none does today).
//   3. WRITE the "HOSHGT1" container consumed by
//      src/terrain/planet_heightmap.rs (see that file for the byte layout).
//   4. SELF-VERIFY by bilinear-sampling the OUTPUT at known real-world
//      anchors (Everest, Challenger Deep, mid-Pacific, Sahara) with the
//      same math the Rust sampler uses. A byte-order, orientation, or
//      quantization mistake cannot pass all four.
//
// Usage:
//   node scripts/build-earth-heightmap.js <path-to-etopo1_ice_c_i2.bin> [out.bin]
// Default output: <repo>/data/planets/earth_heightmap.bin (~12.4 MB, committed:
// Earth ships with the game; data/ is distributed with the exe, docs/ is not).

'use strict';

const fs = require('fs');
const path = require('path');

// Output grid: 0.1-degree cells. 21600/6 x 10800/6.
const OUT_W = 3600;
const OUT_H = 1800;
// Fixed quantization window (meters relative to sea level; see the header
// comment for why the top hugs the averaged data max). Sea level lands at
// (0 - MIN_M)/(MAX_M - MIN_M) = 11000/17500 = ~0.62857 of the normalized
// domain -- the Rust loader derives that same value from the file header
// (sea_level_normalized), so changing these constants and rebuilding is
// safe: nothing else hardcodes them.
const MIN_M = -11000;
const MAX_M = 6500;
const MAGIC = 'HOSHGT1';

function fail(msg) {
  console.error('ERROR: ' + msg);
  process.exit(1);
}

const srcPath = process.argv[2];
if (!srcPath) {
  fail('usage: node scripts/build-earth-heightmap.js <etopo1_ice_c_i2.bin> [out.bin]');
}
const outPath =
  process.argv[3] || path.join(__dirname, '..', 'data', 'planets', 'earth_heightmap.bin');

// ---- 1. Read the source grid and detect its registration by size ----------
const raw = fs.readFileSync(srcPath);
let srcW, srcH;
if (raw.length === 21600 * 10800 * 2) {
  srcW = 21600; srcH = 10800; // cell-registered (the expected download)
} else if (raw.length === 21601 * 10801 * 2) {
  srcW = 21601; srcH = 10801; // grid-registered fallback
} else {
  fail(`unexpected source size ${raw.length} bytes; expected 21600x10800 or 21601x10801 int16`);
}
console.log(`Source: ${srcW} x ${srcH} int16 LE (${(raw.length / 1e6).toFixed(1)} MB), ${srcPath}`);

// Fast path: view the buffer as an Int16Array (source and every machine we
// build on are little-endian; assert rather than silently mis-read).
const le = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;
if (!le) fail('this script requires a little-endian host (Int16Array view of LE data)');
const src =
  raw.byteOffset % 2 === 0
    ? new Int16Array(raw.buffer, raw.byteOffset, raw.length / 2)
    : new Int16Array(raw.buffer.slice(raw.byteOffset, raw.byteOffset + raw.length));

// ---- 2. Box-average 6x6 blocks down to 3600x1800 ---------------------------
// For grid-registered input (21601 wide) the last column/row duplicate the
// first meridian / south pole edge; averaging the leading 21600x10800 block
// grid is correct to within the half-cell shift noted above.
const BLOCK = 6;
const out = new Uint16Array(OUT_W * OUT_H);
let srcMin = Infinity, srcMax = -Infinity;
console.log(`Averaging ${BLOCK}x${BLOCK} blocks -> ${OUT_W} x ${OUT_H} ...`);
for (let R = 0; R < OUT_H; R++) {
  for (let C = 0; C < OUT_W; C++) {
    let sum = 0;
    for (let r = 0; r < BLOCK; r++) {
      const rowBase = (R * BLOCK + r) * srcW + C * BLOCK;
      for (let c = 0; c < BLOCK; c++) sum += src[rowBase + c];
    }
    const mean = sum / (BLOCK * BLOCK);
    if (mean < srcMin) srcMin = mean;
    if (mean > srcMax) srcMax = mean;
    out[R * OUT_W + C] = quantize(mean);
  }
  if (R % 300 === 0) process.stdout.write(`  row ${R}/${OUT_H}\r`);
}
console.log(`\nAveraged-data range: ${srcMin.toFixed(1)} .. ${srcMax.toFixed(1)} m`);
if (srcMin < MIN_M || srcMax > MAX_M) {
  // Not fatal (values clamp), but it would mean the fixed window is wrong.
  console.warn(`WARNING: data exceeds the fixed ${MIN_M}..${MAX_M} m window; extremes clamp`);
}
if (srcMax < 5000 || srcMin > -8000) {
  fail('averaged data range is implausible for Earth; wrong file or byte order?');
}

// Keep in sync with src/terrain/planet_heightmap.rs::quantize_meters.
function quantize(h) {
  const t = Math.min(1, Math.max(0, (h - MIN_M) / (MAX_M - MIN_M)));
  return Math.round(t * 65535);
}
function dequantize(q) {
  return MIN_M + (q / 65535) * (MAX_M - MIN_M);
}

// ---- 3. Write the HOSHGT1 container ----------------------------------------
const header = Buffer.alloc(7 + 4 + 4 + 4 + 4);
header.write(MAGIC, 0, 'ascii');
header.writeUInt32LE(OUT_W, 7);
header.writeUInt32LE(OUT_H, 11);
header.writeFloatLE(MIN_M, 15);
header.writeFloatLE(MAX_M, 19);
const payload = Buffer.from(out.buffer, out.byteOffset, out.byteLength); // LE host = LE bytes
fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, Buffer.concat([header, payload]));
console.log(`Wrote ${outPath} (${((header.length + payload.length) / 1e6).toFixed(2)} MB)`);

// ---- 4. Self-verify with the Rust sampler's exact bilinear math -------------
// Mirrors PlanetHeightmap::sample_meters_latlon: cell centers at
// (index + 0.5), longitude wraps, latitude clamps.
function sample(latDeg, lonDeg) {
  const fx = ((lonDeg + 180) / 360) * OUT_W - 0.5;
  const fy = ((90 - latDeg) / 180) * OUT_H - 0.5;
  const x0 = Math.floor(fx), y0 = Math.floor(fy);
  const tx = fx - x0, ty = fy - y0;
  const at = (x, y) => {
    const xi = ((x % OUT_W) + OUT_W) % OUT_W;
    const yi = Math.min(OUT_H - 1, Math.max(0, y));
    return dequantize(out[yi * OUT_W + xi]);
  };
  const top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * tx;
  const bot = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * tx;
  return top + (bot - top) * ty;
}

// Anchors: [name, lat, lon, check, description of what failing means].
const anchors = [
  ['Everest region', 28.0, 86.9, (h) => h > 5000,
    'expected > 5000 m (Himalayan massif after 0.1-deg averaging)'],
  ['Challenger Deep', 11.37, 142.59, (h) => h < -7000,
    'expected < -7000 m (Mariana Trench)'],
  ['Mid-Pacific abyss', 5.0, -135.0, (h) => h < -3000,
    'expected < -3000 m (open deep ocean)'],
  ['Sahara', 23.0, 10.0, (h) => h > 100 && h < 3000,
    'expected dry land 100..3000 m (catches inverted encoding)'],
];
let ok = true;
for (const [name, lat, lon, check, why] of anchors) {
  const h = sample(lat, lon);
  const pass = check(h);
  ok = ok && pass;
  console.log(`  ${pass ? 'PASS' : 'FAIL'} ${name} (${lat}, ${lon}): ${h.toFixed(1)} m -- ${why}`);
}
if (!ok) {
  fs.unlinkSync(outPath); // never leave a bad grid where the game will load it
  fail('anchor self-verification failed; output deleted');
}
console.log('All anchors pass. Heightmap is ready to commit.');
