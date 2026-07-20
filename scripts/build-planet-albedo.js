#!/usr/bin/env node
// Build a data/planets/<body>_albedo.bin from any 8-bit equirectangular PNG.
// Generalized sibling of scripts/build-earth-albedo.js (Earth keeps its own
// dedicated script; this one bakes every OTHER body). Same decode, same
// area-weighted linear-light downsample, same HOSALB1 container consumed by
// src/terrain/planet_albedo.rs -- plus two source-fixup passes real
// non-Earth maps need:
//
//   --roll180     Roll the source columns by half the width. NASA's New
//                 Horizons Pluto mosaic is published with Sputnik Planitia
//                 (lon ~178 E) at the IMAGE CENTER, i.e. col 0 = lon 0.
//                 Our container convention is col 0 = lon -180 (image
//                 center = lon 0), so Pluto needs this roll. The Solar
//                 System Scope Moon/Mars textures are already near-side /
//                 prime-meridian centered (verified against Mare Crisium at
//                 +59 E and Olympus Mons at -134 E) and do NOT need it.
//   --fill-nodata New Horizons flew past Pluto with the south pole in
//                 polar night: everything below roughly -30 latitude is
//                 BLACK no-data in the published mosaic (plus a thin black
//                 seam inside the valid area). Without a fill the planet
//                 renders with a black southern hemisphere. This pass, per
//                 column: (a) black runs bounded above+below by valid
//                 pixels are vertically lerped shut (fixes the seam);
//                 (b) black runs touching the top/bottom edge are filled
//                 with that column's last valid color, blended toward the
//                 longitudinal mean boundary color approaching the pole so
//                 the polar singularity converges to one color instead of
//                 2048 disagreeing column streaks. "Black" = max(r,g,b) <
//                 12 (JPEG-heritage sources never hold a true 0 edge).
//                 Two artifact killers (added after eyeballing the first
//                 Pluto bake): the invalid mask is DILATED 2 px vertically
//                 so the JPEG ringing halo hugging every black edge is
//                 treated as no-data too (it read as a dark seam line
//                 otherwise), and the per-column edge base colors get a
//                 wide circular box blur before blending (raw per-column
//                 bases disagree texel-to-texel and painted the whole fill
//                 with vertical stripes).
//
// Sources baked with this script (2026-07-19). Exact URLs + licenses --
// keep this list current, it IS the credits record:
//   Moon:  https://www.solarsystemscope.com/textures/download/8k_moon.jpg
//          Solar System Scope textures, CC-BY 4.0 (based on NASA elevation
//          and imagery data). 8192x4096. No roll, no fill.
//   Mars:  https://www.solarsystemscope.com/textures/download/8k_mars.jpg
//          Solar System Scope textures, CC-BY 4.0. 8192x4096. No roll/fill.
//   Pluto: https://assets.science.nasa.gov/content/dam/science/psd/solar/2023/09/p/l/pluto_color_mapmosaic.jpg
//          (from https://science.nasa.gov/resource/pluto-global-color-map/)
//          NASA/JHUAPL/SwRI New Horizons global color mosaic, public
//          domain (US government work). 5926x2963. Needs --roll180
//          --fill-nodata (see above).
// JPEG -> PNG conversion is done outside this script (any tool; PowerShell
// System.Drawing works: 8-bit non-interlaced RGB/RGBA out of the box).
//
// Usage:
//   node scripts/build-planet-albedo.js <in.png> <out.bin> <width> <height> \
//        [--body moon|mars|pluto] [--roll180] [--fill-nodata]
//
// --body picks a set of real-feature anchor points that the OUTPUT is
// bilinear-sampled at (same math as the Rust sampler). Their RGB values are
// printed for human sanity-checking and comparative checks (maria darker
// than highlands, Syrtis darker than Hellas, Sputnik brighter than Cthulhu,
// red dominance on Mars, gray on the Moon) hard-fail the bake -- a flipped,
// rolled-wrong, or channel-swapped file cannot pass them. Without --body
// only whole-image stats are printed.

'use strict';

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const MAGIC = 'HOSALB1';
const NO_DATA_MAX = 12; // max(r,g,b) below this = no-data black

function fail(msg) {
  console.error('ERROR: ' + msg);
  process.exit(1);
}

// ---- argv ------------------------------------------------------------------
const positional = [];
let bodyName = null;
let roll180 = false;
let fillNodata = false;
for (const arg of process.argv.slice(2)) {
  if (arg === '--roll180') roll180 = true;
  else if (arg === '--fill-nodata') fillNodata = true;
  else if (arg.startsWith('--body')) {
    bodyName = arg.includes('=') ? arg.split('=')[1] : null;
    if (!bodyName) fail('--body needs a value: --body=moon|mars|pluto');
  } else positional.push(arg);
}
if (positional.length !== 4) {
  fail(
    'usage: node scripts/build-planet-albedo.js <in.png> <out.bin> <width> <height> [--body=moon|mars|pluto] [--roll180] [--fill-nodata]'
  );
}
const [srcPath, outPath] = positional;
const OUT_W = parseInt(positional[2], 10);
const OUT_H = parseInt(positional[3], 10);
if (!Number.isFinite(OUT_W) || !Number.isFinite(OUT_H) || OUT_W < 2 || OUT_H < 2) {
  fail(`bad output size ${positional[2]}x${positional[3]}`);
}
if (OUT_W !== OUT_H * 2) {
  fail(`output must be 2:1 equirectangular (got ${OUT_W}x${OUT_H})`);
}

// ---- 1. Decode the PNG (identical to build-earth-albedo.js) ----------------
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
    off += 8 + len + 4; // CRC unverified: inflate fails loudly on corruption
  }
  if (!ihdr) fail('PNG has no IHDR chunk');
  if (ihdr.bitDepth !== 8) fail(`unsupported bit depth ${ihdr.bitDepth} (need 8)`);
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
        case 0: v = rawv; break;
        case 1: v = rawv + left; break;
        case 2: v = rawv + up; break;
        case 3: v = rawv + ((left + up) >> 1); break;
        case 4: v = rawv + paeth(left, up, upLeft); break;
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

// ---- sRGB <-> linear (shared by every later pass) --------------------------
const SRGB_TO_LINEAR = new Float64Array(256);
for (let i = 0; i < 256; i++) {
  const c = i / 255;
  SRGB_TO_LINEAR[i] = c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}
function linearToSrgbByte(v) {
  const c = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
  return Math.max(0, Math.min(255, Math.round(c * 255)));
}

// ---- 1b. Optional 180-degree column roll -----------------------------------
if (roll180) {
  console.log('Rolling columns 180 degrees (source center = lon 180 -> lon 0) ...');
  const { width, height, bpp, pixels } = png;
  const half = Math.floor(width / 2) * bpp;
  const stride = width * bpp;
  const tmp = Buffer.alloc(stride);
  for (let y = 0; y < height; y++) {
    const row = pixels.subarray(y * stride, (y + 1) * stride);
    row.copy(tmp, 0, half, stride); // right half first
    row.copy(tmp, stride - half, 0, half); // then left half
    tmp.copy(row);
  }
}

// ---- 1c. Optional no-data fill (black seams + unlit pole) ------------------
if (fillNodata) {
  console.log('Filling no-data (black) regions ...');
  const { width, height, bpp, pixels } = png;
  const stride = width * bpp;
  const rawValid = (x, y) => {
    const o = y * stride + x * bpp;
    return Math.max(pixels[o], pixels[o + 1], pixels[o + 2]) >= NO_DATA_MAX;
  };
  // Dilate the invalid mask 2 px vertically: JPEG sources ring a dark halo
  // onto the 1-2 valid pixels touching every black region; treating them as
  // no-data lets the fill overwrite the halo instead of smearing it.
  const invalid = new Uint8Array(width * height);
  for (let x = 0; x < width; x++) {
    for (let y = 0; y < height; y++) {
      if (!rawValid(x, y)) {
        invalid[y * width + x] = 1;
        for (const dy of [-2, -1, 1, 2]) {
          const yy = y + dy;
          if (yy >= 0 && yy < height) invalid[yy * width + x] = 1;
        }
      }
    }
  }
  const isValid = (x, y) => invalid[y * width + x] === 0;
  const getLinear = (x, y) => {
    const o = y * stride + x * bpp;
    return [
      SRGB_TO_LINEAR[pixels[o]],
      SRGB_TO_LINEAR[pixels[o + 1]],
      SRGB_TO_LINEAR[pixels[o + 2]],
    ];
  };
  const setLinear = (x, y, rgb) => {
    const o = y * stride + x * bpp;
    pixels[o] = linearToSrgbByte(rgb[0]);
    pixels[o + 1] = linearToSrgbByte(rgb[1]);
    pixels[o + 2] = linearToSrgbByte(rgb[2]);
  };
  // Base color for a column's edge fill: average the valid pixels a few rows
  // clear of the boundary (JPEG ringing darkens the 1-2 pixels touching the
  // black region, so stepping back avoids smearing that halo poleward).
  const columnBase = (x, yLastValid, dirUp) => {
    let r = 0, g = 0, b = 0, n = 0;
    for (let k = 2; k <= 8; k++) {
      const y = dirUp ? yLastValid - k : yLastValid + k;
      if (y < 0 || y >= height || !isValid(x, y)) continue;
      const c = getLinear(x, y);
      r += c[0]; g += c[1]; b += c[2]; n++;
    }
    if (n === 0) return getLinear(x, yLastValid);
    return [r / n, g / n, b / n];
  };

  // Pass A: interior runs -- lerp shut vertically, in linear light.
  let interiorFilled = 0;
  for (let x = 0; x < width; x++) {
    let y = 0;
    while (y < height) {
      if (isValid(x, y)) { y++; continue; }
      let end = y;
      while (end < height && !isValid(x, end)) end++;
      if (y > 0 && end < height) {
        const above = getLinear(x, y - 1);
        const below = getLinear(x, end);
        const span = end - y + 1;
        for (let yy = y; yy < end; yy++) {
          const t = (yy - y + 1) / span;
          setLinear(x, yy, [
            above[0] + (below[0] - above[0]) * t,
            above[1] + (below[1] - above[1]) * t,
            above[2] + (below[2] - above[2]) * t,
          ]);
          interiorFilled++;
        }
      }
      y = end;
    }
  }

  // Pass B: edge runs (unlit pole). Collect each column's boundary + base
  // color first so the longitudinal mean is available before writing.
  for (const bottom of [true, false]) {
    const bounds = new Int32Array(width).fill(-1); // last valid row per column
    const bases = new Array(width).fill(null);
    let mean = [0, 0, 0];
    let meanN = 0;
    for (let x = 0; x < width; x++) {
      if (bottom) {
        let y = height - 1;
        while (y >= 0 && !isValid(x, y)) y--;
        if (y < 0 || y === height - 1) continue; // all-invalid or nothing to fill
        bounds[x] = y;
        bases[x] = columnBase(x, y, true);
      } else {
        let y = 0;
        while (y < height && !isValid(x, y)) y++;
        if (y >= height || y === 0) continue;
        bounds[x] = y;
        bases[x] = columnBase(x, y, false);
      }
      mean[0] += bases[x][0]; mean[1] += bases[x][1]; mean[2] += bases[x][2];
      meanN++;
    }
    if (meanN === 0) continue; // this edge is fully valid
    mean = [mean[0] / meanN, mean[1] / meanN, mean[2] / meanN];
    // Smooth the base colors longitudinally (wide circular box blur, two
    // passes ~ triangular kernel). Raw per-column bases carry texel-level
    // noise that otherwise paints the whole fill with vertical stripes;
    // ~2.8 degrees of blur keeps the broad color variation and kills them.
    const radius = Math.max(4, Math.round(width / 128));
    for (let pass = 0; pass < 2; pass++) {
      const smoothed = new Array(width).fill(null);
      for (let x = 0; x < width; x++) {
        if (bases[x] === null) continue;
        let r = 0, g = 0, b = 0, n = 0;
        for (let k = -radius; k <= radius; k++) {
          const nb = bases[(((x + k) % width) + width) % width];
          if (nb === null) continue;
          r += nb[0]; g += nb[1]; b += nb[2]; n++;
        }
        smoothed[x] = [r / n, g / n, b / n];
      }
      for (let x = 0; x < width; x++) bases[x] = smoothed[x];
    }
    let edgeFilled = 0;
    for (let x = 0; x < width; x++) {
      if (bounds[x] < 0) continue;
      const yv = bounds[x];
      const base = bases[x];
      const span = bottom ? height - 1 - yv : yv;
      for (let k = 1; k <= span; k++) {
        const y = bottom ? yv + k : yv - k;
        // u: 0 at the data boundary -> 1 at the pole row. t eases toward the
        // longitudinal mean so the pole converges to ONE color (no streaks).
        const u = k / span;
        const t = Math.pow(u, 0.7);
        setLinear(x, y, [
          base[0] + (mean[0] - base[0]) * t,
          base[1] + (mean[1] - base[1]) * t,
          base[2] + (mean[2] - base[2]) * t,
        ]);
        edgeFilled++;
      }
    }
    console.log(
      `  ${bottom ? 'south' : 'north'} edge: filled ${edgeFilled} px across ${meanN} columns`
    );
  }
  console.log(`  interior seams: filled ${interiorFilled} px`);
}

// ---- 2. Area-weighted downsample in linear light ---------------------------
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

// ---- 4. Self-verify: stats + per-body real-feature anchors -----------------
// Bilinear sampler mirroring PlanetAlbedo::sample (cell centers at
// index + 0.5, longitude wraps, latitude clamps). Returns sRGB bytes.
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
const lum = ([r, g, b]) => 0.2126 * r + 0.7152 * g + 0.0722 * b;

// Whole-image stats (always printed): a valid bake is never near-black or
// near-white overall, and stats give the human a one-line sanity read.
{
  let mn = 255, mx = 0, sum = [0, 0, 0];
  for (let i = 0; i < out.length; i += 3) {
    const l = Math.max(out[i], out[i + 1], out[i + 2]);
    if (l < mn) mn = l;
    if (l > mx) mx = l;
    sum[0] += out[i]; sum[1] += out[i + 1]; sum[2] += out[i + 2];
  }
  const n = out.length / 3;
  console.log(
    `Stats: mean rgb(${sum.map((s) => (s / n).toFixed(0)).join(', ')}), ` +
    `max-channel range ${mn}..${mx}`
  );
  if (mx < 40) fail('output is near-black everywhere; wrong source or bad decode');
}

// Anchor sets. Each: [name, lat, lon]. Checks below per body compare them.
const ANCHORS = {
  moon: {
    points: [
      ['Mare Imbrium (dark maria)', 32.8, -15.6],
      ['Mare Tranquillitatis (dark maria)', 8.5, 31.4],
      ['Southern highlands nr Tycho (bright)', -45.0, -11.0],
      ['Far-side highlands (bright)', 20.0, -170.0],
    ],
    check(s) {
      const [imbrium, tranq, tycho, farside] = s.map(lum);
      const errs = [];
      if (!(imbrium < tycho && imbrium < farside)) {
        errs.push('Mare Imbrium not darker than highlands (roll/flip?)');
      }
      if (!(tranq < tycho && tranq < farside)) {
        errs.push('Mare Tranquillitatis not darker than highlands (roll/flip?)');
      }
      for (const [i, rgb] of s.entries()) {
        if (Math.abs(rgb[0] - rgb[1]) > 30 || Math.abs(rgb[1] - rgb[2]) > 30) {
          errs.push(`anchor ${i} is strongly colored; the Moon is gray (channel swap?)`);
        }
      }
      return errs;
    },
  },
  mars: {
    points: [
      ['Olympus Mons / Tharsis (bright)', 18.6, -133.8],
      ['Syrtis Major (classic dark)', 8.0, 67.0],
      ['Hellas Planitia (bright dust bowl)', -42.0, 70.0],
      ['Acidalia Planitia (dark north)', 50.0, -30.0],
    ],
    check(s) {
      const [olympus, syrtis, hellas, acidalia] = s.map(lum);
      const errs = [];
      if (!(syrtis < olympus)) errs.push('Syrtis Major not darker than Tharsis (E/W roll?)');
      if (!(syrtis < hellas)) errs.push('Syrtis Major not darker than Hellas (N/S flip?)');
      if (!(acidalia < hellas)) errs.push('Acidalia not darker than Hellas (N/S flip?)');
      for (const [i, rgb] of s.entries()) {
        if (!(rgb[0] > rgb[2])) {
          errs.push(`anchor ${i} has blue >= red; Mars is red (channel swap?)`);
        }
      }
      return errs;
    },
  },
  pluto: {
    points: [
      ['Sputnik Planitia (bright ice heart)', 20.0, 178.0],
      ['Cthulhu Macula (dark red whale)', -5.0, 105.0],
      ['Lowell Regio (yellowish north pole)', 80.0, 0.0],
      ['Filled south pole (must not be black)', -80.0, 0.0],
    ],
    check(s) {
      const [sputnik, cthulhu, lowell, south] = s;
      const errs = [];
      if (!(lum(sputnik) > 1.5 * lum(cthulhu))) {
        errs.push('Sputnik Planitia not clearly brighter than Cthulhu (roll180 missing/extra?)');
      }
      if (!(Math.min(...sputnik) > 120)) {
        errs.push('Sputnik Planitia not bright; wrong longitude registration?');
      }
      if (!(cthulhu[0] > cthulhu[2])) {
        errs.push('Cthulhu Macula not reddish (channel swap?)');
      }
      if (!(Math.max(...south) >= NO_DATA_MAX)) {
        errs.push('south pole is still black; --fill-nodata missing?');
      }
      if (!(Math.max(...lowell) >= NO_DATA_MAX)) {
        errs.push('north pole black?');
      }
      return errs;
    },
  },
};

if (bodyName) {
  const set = ANCHORS[bodyName];
  if (!set) fail(`unknown --body '${bodyName}' (know: ${Object.keys(ANCHORS).join(', ')})`);
  const samples = set.points.map(([, lat, lon]) => sample(lat, lon));
  for (const [i, [name, lat, lon]] of set.points.entries()) {
    console.log(
      `  ${name} (${lat}, ${lon}): rgb(${samples[i].map((v) => v.toFixed(0)).join(', ')})`
    );
  }
  const errs = set.check(samples);
  for (const e of errs) console.error('  FAIL ' + e);
  if (errs.length > 0) {
    fs.unlinkSync(outPath); // never leave a bad grid where the game will load it
    fail('anchor self-verification failed; output deleted');
  }
  console.log(`All ${bodyName} anchors pass. Albedo grid is ready to commit.`);
} else {
  console.log('No --body given: stats only, no anchor verification.');
}
