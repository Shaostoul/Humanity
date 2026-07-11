#!/usr/bin/env node
/**
 * render-sky-wallpaper.js
 *
 * Offline HIGH-RESOLUTION renders of the HumanityOS skybox - straight from
 * the data, no GPU, no screen: the level-9 Gaia census glow (dust rifts,
 * Magellanic Clouds) + every star of the best installed catalog splatted
 * individually + photographic halos on the naked-eye set. For sharing and
 * desktop backgrounds; the in-game sky is this exact stack at runtime.
 *
 * Usage:
 *   node scripts/render-sky-wallpaper.js [outdir]   (default: wallpapers/)
 *
 * Outputs:
 *   sky_equirect_8192x4096.png  - the full-sky master (equirectangular)
 *   sky_core_3840x2160.png      - 4K crop centered on the galactic center
 *   sky_core_3440x1440.png      - ultrawide crop, same centering
 *
 * Pipeline: decode data/galaxy_glow.png (our own filter-0 PNG; see
 * build-galaxy-glow.js writePng) -> linearize (it is display-referred, the
 * v0.802.2 lesson) -> bilinear-upsample to the master grid -> splat stars
 * from the best of stars-gaia25m.bin / stars-athyg.bin / stars.bin as
 * gaussian points (sub-pixel sized for the faint millions, growing gently
 * with brightness) -> halo splats for the standard catalog's mag <= 2 set
 * (matching the runtime halo layer's spirit) -> asinh shoulder -> sRGB
 * encode. All map math uses THE equirect contract from the glow pipeline:
 * u = atan2(y,x)/2pi + 0.5, v = acos(z)/pi.
 */

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const ROOT = path.resolve(__dirname, '..');
const OUT_DIR = path.resolve(process.argv[2] || path.join(ROOT, 'wallpapers'));

const MW = 8192; // master width
const MH = 4096;

// Tone constants (display-referred output). GLOW_GAIN lifts the baked glow
// a touch for wallpaper drama vs the in-game delicate-veil default; the
// asinh shoulder keeps the core and Sirius from clipping.
const GLOW_GAIN = 1.35;
const STAR_GAIN = 0.055; // linear flux multiplier for point splats
const ASINH_K = 6.0;

// ── Catalog + glow readers (HOSSTAR1 + our filter-0 PNG) ────────────────────
function readCatalog(file) {
  const b = fs.readFileSync(file);
  if (b.subarray(0, 8).toString('ascii') !== 'HOSSTAR1') throw new Error(`${file}: bad magic`);
  const n = b.readUInt32LE(8);
  return { buf: b, n };
}

function readGlow(file) {
  const b = fs.readFileSync(file);
  const w = b.readUInt32BE(16), h = b.readUInt32BE(20);
  // Collect IDAT, inflate, strip the filter-0 bytes our writer emits.
  let o = 8;
  const idat = [];
  while (o < b.length) {
    const len = b.readUInt32BE(o);
    const type = b.toString('ascii', o + 4, o + 8);
    if (type === 'IDAT') idat.push(b.subarray(o + 8, o + 8 + len));
    o += 12 + len;
  }
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = w * 3 + 1;
  const px = new Uint8Array(w * h * 3);
  for (let y = 0; y < h; y++) {
    if (raw[y * stride] !== 0) throw new Error('glow PNG uses a filter != 0; reader assumes our own writer');
    px.set(raw.subarray(y * stride + 1, (y + 1) * stride), y * w * 3);
  }
  return { w, h, px };
}

/** B-V -> linear RGB, the same curve as the engine (ci_to_rgb port). */
function ciToRgb(ci) {
  const t = Math.min(2.0, Math.max(-0.4, ci));
  let r, g, b;
  if (t < 0.4) { r = 0.62 + t * 0.95; g = 0.75 + t * 0.6; b = 1.0; }
  else { r = 1.0; g = 0.85 - (t - 0.4) * 0.35; b = 0.5 - (t - 0.4) * 0.3; }
  return [Math.min(1, Math.max(0, r)), Math.min(1, Math.max(0, g)), Math.min(1, Math.max(0.05, b))];
}

function main() {
  const t0 = Date.now();
  fs.mkdirSync(OUT_DIR, { recursive: true });

  // ── 1. glow base, linearized + upsampled ──
  const glow = readGlow(path.join(ROOT, 'data', 'galaxy_glow.png'));
  console.log(`glow: ${glow.w}x${glow.h}`);
  const lin = { r: new Float32Array(MW * MH), g: new Float32Array(MW * MH), b: new Float32Array(MW * MH) };
  for (let y = 0; y < MH; y++) {
    const sy = ((y + 0.5) / MH) * glow.h - 0.5;
    const y0 = Math.max(0, Math.floor(sy)), y1 = Math.min(glow.h - 1, y0 + 1);
    const fy = sy - y0;
    for (let x = 0; x < MW; x++) {
      const sx = ((x + 0.5) / MW) * glow.w - 0.5;
      let x0 = Math.floor(sx);
      const fx = sx - x0;
      // lon wraps
      const x0w = ((x0 % glow.w) + glow.w) % glow.w;
      const x1w = (x0w + 1) % glow.w;
      const i = y * MW + x;
      for (let c = 0; c < 3; c++) {
        const p00 = glow.px[(y0 * glow.w + x0w) * 3 + c] / 255;
        const p10 = glow.px[(y0 * glow.w + x1w) * 3 + c] / 255;
        const p01 = glow.px[(y1 * glow.w + x0w) * 3 + c] / 255;
        const p11 = glow.px[(y1 * glow.w + x1w) * 3 + c] / 255;
        const v = (p00 * (1 - fx) + p10 * fx) * (1 - fy) + (p01 * (1 - fx) + p11 * fx) * fy;
        const l = Math.pow(v, 2.2) * GLOW_GAIN; // display -> linear
        (c === 0 ? lin.r : c === 1 ? lin.g : lin.b)[i] = l;
      }
    }
  }
  console.log(`glow upsample: ${Date.now() - t0} ms`);

  // ── 2. star splats from the best installed catalog ──
  const catalogPath = ['stars-gaia25m.bin', 'stars-athyg.bin', 'stars.bin']
    .map((f) => path.join(ROOT, 'data', f))
    .find((p) => fs.existsSync(p));
  const cat = readCatalog(catalogPath);
  console.log(`catalog: ${path.basename(catalogPath)} (${cat.n} stars)`);
  const t1 = Date.now();
  for (let s = 0; s < cat.n; s++) {
    const o = 16 + s * 15;
    const dx = cat.buf.readFloatLE(o), dy = cat.buf.readFloatLE(o + 4), dz = cat.buf.readFloatLE(o + 8);
    const mag = cat.buf.readUInt16LE(o + 12) / 1024 - 2.0;
    const ci = (cat.buf.readUInt8(o + 14) / 255) * 2.4 - 0.4;
    const u = Math.atan2(dy, dx) / (2 * Math.PI) + 0.5;
    const v = Math.acos(Math.min(1, Math.max(-1, dz))) / Math.PI;
    const flux = Math.pow(10, (6.5 - mag) / 2.5) * STAR_GAIN;
    const [cr, cg, cb] = ciToRgb(ci);
    const fx = u * MW, fy = v * MH;
    // Splat radius grows softly with brightness: faint = single pixel,
    // naked-eye = a small gaussian dot (this is PSF, not the halo layer).
    const sigma = Math.min(3.0, Math.max(0.5, 1.6 - mag * 0.25));
    const rad = Math.max(1, Math.ceil(sigma * 2.5));
    const cx = Math.floor(fx), cy = Math.floor(fy);
    for (let py = cy - rad; py <= cy + rad; py++) {
      if (py < 0 || py >= MH) continue;
      for (let px2 = cx - rad; px2 <= cx + rad; px2++) {
        const pxw = ((px2 % MW) + MW) % MW;
        const ddx = px2 + 0.5 - fx, ddy = py + 0.5 - fy;
        const w = Math.exp(-(ddx * ddx + ddy * ddy) / (2 * sigma * sigma)) / (2 * Math.PI * sigma * sigma);
        const i = py * MW + pxw;
        lin.r[i] += flux * w * cr;
        lin.g[i] += flux * w * cg;
        lin.b[i] += flux * w * cb;
      }
    }
  }
  console.log(`star splats: ${Date.now() - t1} ms`);

  // ── 3. halo splats (the runtime halo layer's wallpaper cousin): the
  //      standard catalog's mag <= 2 set gets a wide soft gaussian. ──
  const std = readCatalog(path.join(ROOT, 'data', 'stars.bin'));
  let halos = 0;
  for (let s = 0; s < std.n; s++) {
    const o = 16 + s * 15;
    const mag = std.buf.readUInt16LE(o + 12) / 1024 - 2.0;
    if (mag > 2.0) continue;
    const dx = std.buf.readFloatLE(o), dy = std.buf.readFloatLE(o + 4), dz = std.buf.readFloatLE(o + 8);
    const ci = (std.buf.readUInt8(o + 14) / 255) * 2.4 - 0.4;
    const u = Math.atan2(dy, dx) / (2 * Math.PI) + 0.5;
    const v = Math.acos(Math.min(1, Math.max(-1, dz))) / Math.PI;
    const [cr, cg, cb] = ciToRgb(ci);
    const fx = u * MW, fy = v * MH;
    const amp = 0.28 * Math.pow(Math.pow(10, -mag / 2.5) / Math.pow(10, 1.44 / 2.5), 0.6);
    const sigma = 9 + (2.0 - mag) * 4; // wide, soft
    const rad = Math.ceil(sigma * 2.5);
    const cx = Math.floor(fx), cy = Math.floor(fy);
    for (let py = cy - rad; py <= cy + rad; py++) {
      if (py < 0 || py >= MH) continue;
      for (let px2 = cx - rad; px2 <= cx + rad; px2++) {
        const pxw = ((px2 % MW) + MW) % MW;
        const ddx = px2 + 0.5 - fx, ddy = py + 0.5 - fy;
        const r2 = ddx * ddx + ddy * ddy;
        let w = Math.exp(-r2 / (2 * sigma * sigma));
        // Subtle 4-point cross, matching the in-game halo taste.
        const ax = Math.abs(ddx), ay = Math.abs(ddy);
        w += 0.15 * (Math.exp(-(ay * ay) / 2 - ax / (sigma * 1.8)) + Math.exp(-(ax * ax) / 2 - ay / (sigma * 1.8)));
        const i = py * MW + pxw;
        lin.r[i] += amp * w * cr * 0.05;
        lin.g[i] += amp * w * cg * 0.05;
        lin.b[i] += amp * w * cb * 0.05;
      }
    }
    halos++;
  }
  console.log(`halos: ${halos}`);

  // ── 4. tone map (asinh shoulder) + sRGB encode ──
  const rgb = new Uint8Array(MW * MH * 3);
  const shoulder = Math.asinh(ASINH_K);
  for (let i = 0; i < MW * MH; i++) {
    for (let c = 0; c < 3; c++) {
      const l = (c === 0 ? lin.r : c === 1 ? lin.g : lin.b)[i];
      const t = Math.asinh(ASINH_K * Math.min(4, l)) / shoulder;
      rgb[i * 3 + c] = Math.round(255 * Math.min(1, Math.pow(Math.min(1, t), 1 / 2.2)));
    }
  }

  writePng(path.join(OUT_DIR, `sky_equirect_${MW}x${MH}.png`), rgb, MW, MH);

  // ── 5. crops centered on the galactic center ──
  // GC at RA 266.417, Dec -29.008 -> u,v on the master grid.
  const gcRa = (266.417 * Math.PI) / 180, gcDec = (-29.008 * Math.PI) / 180;
  const gcd = [Math.cos(gcDec) * Math.cos(gcRa), Math.cos(gcDec) * Math.sin(gcRa), Math.sin(gcDec)];
  const gu = Math.atan2(gcd[1], gcd[0]) / (2 * Math.PI) + 0.5;
  const gv = Math.acos(gcd[2]) / Math.PI;
  for (const [cw, ch] of [[3840, 2160], [3440, 1440]]) {
    const cx0 = Math.round(gu * MW - cw / 2);
    const cy0 = Math.min(MH - ch, Math.max(0, Math.round(gv * MH - ch / 2)));
    const crop = new Uint8Array(cw * ch * 3);
    for (let y = 0; y < ch; y++) {
      for (let x = 0; x < cw; x++) {
        const sx = (((cx0 + x) % MW) + MW) % MW;
        const si = ((cy0 + y) * MW + sx) * 3;
        crop.set(rgb.subarray(si, si + 3), (y * cw + x) * 3);
      }
    }
    writePng(path.join(OUT_DIR, `sky_core_${cw}x${ch}.png`), crop, cw, ch);
  }
  console.log(`done in ${((Date.now() - t0) / 1000).toFixed(0)} s -> ${OUT_DIR}`);
}

// Minimal PNG writer (same shape as build-galaxy-glow.js's).
function crc32(buf) {
  let c, table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  c = -1;
  for (let i = 0; i < buf.length; i++) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}
function chunk(type, data) {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, 'ascii');
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}
function writePng(file, rgb, w, h) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 2;
  const raw = Buffer.alloc(h * (1 + w * 3));
  for (let y = 0; y < h; y++) {
    raw[y * (1 + w * 3)] = 0;
    Buffer.from(rgb.buffer, rgb.byteOffset + y * w * 3, w * 3).copy(raw, y * (1 + w * 3) + 1);
  }
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
  fs.writeFileSync(file, png);
  // Read-back tripwire (the v0.807.2 phantom-bake lesson).
  const back = fs.readFileSync(file);
  if (back.readUInt32BE(16) !== w || back.readUInt32BE(20) !== h) throw new Error(`${file}: write verify failed`);
  console.log(`${path.basename(file)}: ${(back.length / 1048576).toFixed(1)} MB`);
}

main();
