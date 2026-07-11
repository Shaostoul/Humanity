#!/usr/bin/env node
// Build data/planets/earth_albedo.bin from NASA Blue Marble Next Generation.
//
// Source: "The Blue Marble: Next Generation w/ Topography and Bathymetry",
// August 2004 composite, 5400x2700 8-bit RGB PNG (~14.8 MB):
//   https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73776/world.topo.bathy.200408.3x5400x2700.png
// (NASA Visible Earth record 73776. NOTE: the record id in some older notes,
// 74443, 404s -- 73776 is the verified-live location as of 2026-07-11.)
// NASA imagery is a US government work: public domain, free to ship.
// August was chosen because it is the classic "green northern hemisphere"
// month and matches the snow state most people picture from orbital photos.
// The "topo.bathy" variant bakes in hillshaded topography AND ocean-floor
// bathymetry shading, so oceans carry believable depth gradients for free.
//
// Layout of the source (and therefore of the output): equirectangular,
// row 0 = northernmost band, col 0 = westernmost (lon -180), i.e. exactly
// the registration the elevation grid uses (scripts/build-earth-heightmap.js
// / src/terrain/planet_heightmap.rs), so the colors line up with the real
// elevation data with zero remapping.
//
// Processing:
//   1. DECODE the PNG with a self-contained decoder (zlib inflate is built
//      into Node; unfiltering is ~40 lines) -- no npm dependencies, same
//      zero-install policy as every other script in scripts/.
//   2. DOWNSAMPLE 5400x2700 -> 4096x2048 by area-weighted averaging done in
//      LINEAR light (sRGB decode -> average -> sRGB encode). Averaging sRGB
//      bytes directly would darken every coastline/cloud-edge mix; linear
//      averaging is the physically correct box filter. The ratio (1.318x)
//      is fractional, so edge texels contribute partial-coverage weights.
//   3. WRITE the "HOSALB1" container consumed by
//      src/terrain/planet_albedo.rs:
//        bytes 0..7   magic b"HOSALB1"
//        bytes 7..11  u32 LE width
//        bytes 11..15 u32 LE height
//        bytes 15..   width*height*3 sRGB bytes, row-major RGB, row 0 north,
//                     col 0 at lon -180 (cell-centered like the heightmap).
//      Payload stays sRGB-encoded (maximum precision where eyes see it);
//      the Rust loader converts to linear at sample time.
//   4. SELF-VERIFY by bilinear-sampling the OUTPUT at known real-world
//      anchors (Sahara sandy, mid-Atlantic deep blue, Amazon green,
//      Antarctica white) with the same lat/lon math the Rust sampler uses.
//      A flipped, byte-swapped, or channel-swapped file cannot pass all of
//      them.
//
// Usage:
//   node scripts/build-earth-albedo.js <world.topo.bathy.200408.3x5400x2700.png> [out.bin]
// Default output: <repo>/data/planets/earth_albedo.bin (~25.2 MB, committed:
// Earth ships with the game; data/ is distributed with the exe).

'use strict';

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

// Output grid: 4096x2048 (2:1 equirectangular, ~0.088 degree cells --
// comparable to the 0.1-degree elevation grid, so color and relief carry
// matching levels of real detail).
const OUT_W = 4096;
const OUT_H = 2048;
const MAGIC = 'HOSALB1';

function fail(msg) {
  console.error('ERROR: ' + msg);
  process.exit(1);
}

const srcPath = process.argv[2];
if (!srcPath) {
  fail('usage: node scripts/build-earth-albedo.js <blue-marble.png> [out.bin]');
}
const outPath =
  process.argv[3] || path.join(__dirname, '..', 'data', 'planets', 'earth_albedo.bin');

// ---- 1. Decode the PNG (signature, chunks, inflate, unfilter) --------------
function decodePng(buf) {
  const SIG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  if (buf.length < 8 || !buf.subarray(0, 8).equals(SIG)) {
    fail('not a PNG file (bad signature)');
  }
  let off = 8;
  let ihdr = null;
  const idats = [];
  while (off + 8 <= buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString('ascii', off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === 'IHDR') {
      ihdr = {
        width: data.readUInt32BE(0),
        height: data.readUInt32BE(4),
        bitDepth: data[8],
        colorType: data[9],
        interlace: data[12],
      };
    } else if (type === 'IDAT') {
      idats.push(data);
    } else if (type === 'IEND') {
      break;
    }
    off += 8 + len + 4; // length + type + data + CRC (CRC not verified: zlib
    // inflate below fails loudly on any real corruption)
  }
  if (!ihdr) fail('PNG has no IHDR chunk');
  if (ihdr.bitDepth !== 8) fail(`unsupported bit depth ${ihdr.bitDepth} (need 8)`);
  // Color type 2 = RGB, 6 = RGBA (alpha dropped). Blue Marble ships type 2.
  if (ihdr.colorType !== 2 && ihdr.colorType !== 6) {
    fail(`unsupported color type ${ihdr.colorType} (need RGB or RGBA)`);
  }
  if (ihdr.interlace !== 0) fail('interlaced PNG not supported (re-export non-interlaced)');
  const bpp = ihdr.colorType === 2 ? 3 : 4;
  const raw = zlib.inflateSync(Buffer.concat(idats));
  const stride = ihdr.width * bpp;
  if (raw.length !== (stride + 1) * ihdr.height) {
    fail(`inflated size ${raw.length} != expected ${(stride + 1) * ihdr.height}`);
  }

  // Unfilter scanlines (PNG filters 0-4). prev = previous (unfiltered) row.
  const out = Buffer.alloc(stride * ihdr.height);
  const paeth = (a, b, c) => {
    const p = a + b - c;
    const pa = Math.abs(p - a);
    const pb = Math.abs(p - b);
    const pc = Math.abs(p - c);
    if (pa <= pb && pa <= pc) return a;
    if (pb <= pc) return b;
    return c;
  };
  for (let y = 0; y < ihdr.height; y++) {
    const filter = raw[y * (stride + 1)];
    const rowIn = (stride + 1) * y + 1;
    const rowOut = stride * y;
    const prevOut = rowOut - stride;
    for (let x = 0; x < stride; x++) {
      const rawv = raw[rowIn + x];
      const left = x >= bpp ? out[rowOut + x - bpp] : 0;
      const up = y > 0 ? out[prevOut + x] : 0;
      const upLeft = y > 0 && x >= bpp ? out[prevOut + x - bpp] : 0;
      let v;
      switch (filter) {
        case 0: v = rawv; break;                                  // None
        case 1: v = rawv + left; break;                           // Sub
        case 2: v = rawv + up; break;                             // Up
        case 3: v = rawv + ((left + up) >> 1); break;             // Average
        case 4: v = rawv + paeth(left, up, upLeft); break;        // Paeth
        default: fail(`unknown PNG filter ${filter} on row ${y}`);
      }
      out[rowOut + x] = v & 0xff;
    }
  }
  return { width: ihdr.width, height: ihdr.height, bpp, pixels: out };
}

console.log(`Decoding ${srcPath} ...`);
const png = decodePng(fs.readFileSync(srcPath));
console.log(`Source: ${png.width} x ${png.height}, ${png.bpp === 3 ? 'RGB' : 'RGBA'}`);
if (png.width < OUT_W || png.height < OUT_H) {
  fail(`source smaller than the ${OUT_W}x${OUT_H} output; refusing to upsample`);
}
if (Math.abs(png.width / png.height - 2.0) > 0.01) {
  fail('source is not 2:1 equirectangular; wrong file?');
}

// ---- 2. Area-weighted downsample in linear light ---------------------------
// sRGB <-> linear via the exact IEC 61966-2-1 transfer curve, LUT'd for the
// decode direction (the hot path: every source texel).
const SRGB_TO_LINEAR = new Float64Array(256);
for (let i = 0; i < 256; i++) {
  const c = i / 255;
  SRGB_TO_LINEAR[i] = c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}
function linearToSrgbByte(v) {
  const c = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
  return Math.max(0, Math.min(255, Math.round(c * 255)));
}

// Fractional-coverage tap list for one output axis: for each dest index, the
// list of (src index, weight) pairs covering [d * scale, (d+1) * scale).
function coverageTaps(dstSize, srcSize) {
  const scale = srcSize / dstSize;
  const taps = new Array(dstSize);
  for (let d = 0; d < dstSize; d++) {
    const lo = d * scale;
    const hi = (d + 1) * scale;
    const first = Math.floor(lo);
    const last = Math.min(Math.ceil(hi), srcSize) - 1;
    const list = [];
    for (let s = first; s <= last; s++) {
      const w = Math.min(hi, s + 1) - Math.max(lo, s);
      if (w > 1e-12) list.push([s, w]);
    }
    taps[d] = list;
  }
  return taps;
}

console.log(`Downsampling to ${OUT_W} x ${OUT_H} (area-weighted, linear light) ...`);
const xTaps = coverageTaps(OUT_W, png.width);
const yTaps = coverageTaps(OUT_H, png.height);
const out = Buffer.alloc(OUT_W * OUT_H * 3);
const bpp = png.bpp;
for (let Y = 0; Y < OUT_H; Y++) {
  const rows = yTaps[Y];
  for (let X = 0; X < OUT_W; X++) {
    const cols = xTaps[X];
    let r = 0, g = 0, b = 0, wsum = 0;
    for (const [sy, wy] of rows) {
      const rowBase = sy * png.width * bpp;
      for (const [sx, wx] of cols) {
        const o = rowBase + sx * bpp;
        const w = wy * wx;
        r += SRGB_TO_LINEAR[png.pixels[o]] * w;
        g += SRGB_TO_LINEAR[png.pixels[o + 1]] * w;
        b += SRGB_TO_LINEAR[png.pixels[o + 2]] * w;
        wsum += w;
      }
    }
    const oo = (Y * OUT_W + X) * 3;
    out[oo] = linearToSrgbByte(r / wsum);
    out[oo + 1] = linearToSrgbByte(g / wsum);
    out[oo + 2] = linearToSrgbByte(b / wsum);
  }
  if (Y % 256 === 0) process.stdout.write(`  row ${Y}/${OUT_H}\r`);
}
console.log('');

// ---- 3. Write the HOSALB1 container ----------------------------------------
const header = Buffer.alloc(7 + 4 + 4);
header.write(MAGIC, 0, 'ascii');
header.writeUInt32LE(OUT_W, 7);
header.writeUInt32LE(OUT_H, 11);
fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, Buffer.concat([header, out]));
console.log(`Wrote ${outPath} (${((header.length + out.length) / 1e6).toFixed(2)} MB)`);

// ---- 4. Self-verify with the Rust sampler's exact bilinear math -------------
// Mirrors PlanetAlbedo::sample: cell centers at (index + 0.5), longitude
// wraps, latitude clamps. Returns sRGB bytes (what the file stores).
function sample(latDeg, lonDeg) {
  const fx = ((lonDeg + 180) / 360) * OUT_W - 0.5;
  const fy = ((90 - latDeg) / 180) * OUT_H - 0.5;
  const x0 = Math.floor(fx), y0 = Math.floor(fy);
  const tx = fx - x0, ty = fy - y0;
  const at = (x, y, c) => {
    const xi = ((x % OUT_W) + OUT_W) % OUT_W;
    const yi = Math.min(OUT_H - 1, Math.max(0, y));
    return out[(yi * OUT_W + xi) * 3 + c];
  };
  const rgb = [];
  for (let c = 0; c < 3; c++) {
    const top = at(x0, y0, c) + (at(x0 + 1, y0, c) - at(x0, y0, c)) * tx;
    const bot = at(x0, y0 + 1, c) + (at(x0 + 1, y0 + 1, c) - at(x0, y0 + 1, c)) * tx;
    rgb.push(top + (bot - top) * ty);
  }
  return rgb;
}

// Anchors: [name, lat, lon, check([r,g,b]), what failing means].
const anchors = [
  ['Sahara', 23.0, 10.0, ([r, g, b]) => r > g && g > b && r > 90,
    'expected sandy (r > g > b, bright) -- catches channel swap / flip'],
  ['Mid-Atlantic', 10.0, -30.0, ([r, g, b]) => b > r + 10 && b >= 15 && Math.max(r, g, b) < 140,
    'expected dark deep-ocean blue (b well above r, overall dark; the real value is ~rgb(1,5,20))'],
  ['Amazon', -4.0, -63.0, ([r, g, b]) => g > r && g > b,
    'expected rainforest green (g dominant) -- catches N/S or E/W flip'],
  ['Antarctica', -85.0, 0.0, ([r, g, b]) => r > 150 && g > 150 && b > 150,
    'expected bright ice (all channels high) -- catches latitude flip'],
];
let ok = true;
for (const [name, lat, lon, check, why] of anchors) {
  const rgb = sample(lat, lon);
  const pass = check(rgb);
  ok = ok && pass;
  console.log(
    `  ${pass ? 'PASS' : 'FAIL'} ${name} (${lat}, ${lon}): rgb(${rgb.map((v) => v.toFixed(0)).join(', ')}) -- ${why}`
  );
}
if (!ok) {
  fs.unlinkSync(outPath); // never leave a bad grid where the game will load it
  fail('anchor self-verification failed; output deleted');
}
console.log('All anchors pass. Albedo grid is ready to commit.');
