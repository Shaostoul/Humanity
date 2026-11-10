// A GARMENT'S TEXTURE, PACKED AND BC7-ENCODED AT CONVERSION TIME (the look's textures, 2026-11-10;
// docs/design/character-animation.md section 16). Called by scripts/gen-makehuman-look.js, each
// garment on a worker thread of its own (this file is also the worker).
//
// THE PACKING (moved here from the app's engine/look_textures.rs, which packed it on every run). A
// material binds ONE albedo texture, so a garment's colour and its normal map share one texture side
// by side: the left half is the diffuse texture (times its ambient occlusion map where it has one, in
// linear light, and with its alpha: the hair's, brows' and lashes' cards), the right half the normal
// map, stored sRGB-encoded so the sRGB texture hands back the map's own values when sampled. Each half
// is mipmapped on its own, down to one block (4 texels) a half, so no block straddles the middle:
// cards keep their alpha's coverage as they shrink (Castano's method, billboard_bake's
// build_mip_chain: lashes and brows do not vanish at a few metres), cloth and normals are averaged in
// linear light (build_opaque_mip_chain's rule).
//
// THE SIZE. Each half is at most TEXTURE_PX (1024) texels at level 0: the sources (512 to 4096) stay
// whole on disk beside it; a person three metres away covers a few hundred pixels. Settings >
// Graphics > Garment textures' Low memory draws from the level below (512). Sources are halved with a
// box filter (alpha-weighted for cards, in linear light for colour, straight for the normal and
// occlusion maps), so every map must be a power of two; a normal or occlusion map smaller than the
// pack is doubled bilinearly (the fedoras' 512 normal maps beside their 2048 colour).
//
// THE ENCODING: scripts/lib/bc7.js, our own (the scan: docs/reference/research/2026-11-10-bc7-
// encoders/). The file is a DDS (DXGI_FORMAT_BC7_UNORM_SRGB, the standard container), every level.

'use strict';
const zlib = require('zlib');
const bc7 = require('./bc7.js');

const TEXTURE_PX = 1024;
const MIP_MIN_PX = 4;
// The alpha a card's texel counts as covered at, for the coverage-keeping mips (the app's opaque hair
// pass cuts at 0.8; the mips keep the share of texels over a half).
const CARD_CUTOFF = 0.5;

// ── PNG (8-bit grey, grey+alpha, RGB, RGBA; plain or Adam7 interlaced) ─────
function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
}

/**
 * Undo a (sub)image's row filters and write its texels as RGBA: texel (x, y) of the sub-image lands
 * at image texel (x0 + x * dx, y0 + y * dy) of the `iw`-wide `rgba` (Adam7's passes; 0, 0, 1, 1 for
 * a plain image). Returns where the sub-image's data ended in `raw`.
 */
function unfilter(raw, at, w, h, ch, rgba, iw, x0, y0, dx, dy) {
  const stride = w * ch;
  let prev = new Uint8Array(stride);
  let line = new Uint8Array(stride);
  let p = at;
  for (let y = 0; y < h; y++) {
    const f = raw[p++];
    for (let x = 0; x < stride; x++) {
      const a = x >= ch ? line[x - ch] : 0;
      const b = prev[x];
      let v = raw[p++];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) v += paeth(a, b, x >= ch ? prev[x - ch] : 0);
      line[x] = v & 0xff;
    }
    let o = ((y0 + y * dy) * iw + x0) * 4;
    const step = dx * 4;
    for (let x = 0, s = 0; x < w; x++, s += ch, o += step) {
      if (ch >= 3) { rgba[o] = line[s]; rgba[o + 1] = line[s + 1]; rgba[o + 2] = line[s + 2]; rgba[o + 3] = ch === 4 ? line[s + 3] : 255; }
      else { rgba[o] = line[s]; rgba[o + 1] = line[s]; rgba[o + 2] = line[s]; rgba[o + 3] = ch === 2 ? line[s + 1] : 255; }
    }
    [prev, line] = [line, prev];
  }
  return p;
}

/** Decode an 8-bit PNG fully to RGBA: { w, h, rgba }. */
function decodePng(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG');
  let p = 8;
  let w = 0, h = 0, depth = 0, ct = 0, interlace = 0;
  const idat = [];
  while (p < buf.length) {
    const n = buf.readUInt32BE(p);
    const type = buf.toString('ascii', p + 4, p + 8);
    const data = buf.subarray(p + 8, p + 8 + n);
    if (type === 'IHDR') { w = data.readUInt32BE(0); h = data.readUInt32BE(4); depth = data[8]; ct = data[9]; interlace = data[12]; }
    if (type === 'IDAT') idat.push(data);
    if (type === 'IEND') break;
    p += 12 + n;
  }
  const ch = { 0: 1, 2: 3, 4: 2, 6: 4 }[ct];
  if (depth !== 8 || !ch || interlace > 1) throw new Error(`a PNG of depth ${depth}, colour type ${ct}, interlace ${interlace} is not read here`);
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const rgba = new Uint8Array(w * h * 4);
  if (interlace === 0) {
    unfilter(raw, 0, w, h, ch, rgba, w, 0, 0, 1, 1);
  } else {
    // Adam7's seven passes: start and step in x and y.
    const passes = [[0, 0, 8, 8], [4, 0, 8, 8], [0, 4, 4, 8], [2, 0, 4, 4], [0, 2, 2, 4], [1, 0, 2, 2], [0, 1, 1, 2]];
    let at = 0;
    for (const [x0, y0, dx, dy] of passes) {
      const pw = Math.ceil((w - x0) / dx), ph = Math.ceil((h - y0) / dy);
      if (pw <= 0 || ph <= 0) continue;
      at = unfilter(raw, at, pw, ph, ch, rgba, w, x0, y0, dx, dy);
    }
  }
  return { w, h, rgba };
}

// ── Colour in linear light ───────────────────────────────────────────────
const TO_LINEAR = new Float64Array(256);
for (let i = 0; i < 256; i++) { const c = i / 255; TO_LINEAR[i] = c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }
// Linear to sRGB through a table of 65,536 steps over 0..1 (a step is under a twentieth of an 8-bit
// step even where the curve is steepest), so the hot loops make no power calls.
const TO_SRGB = new Uint8Array(65536);
for (let i = 0; i < 65536; i++) {
  const x = i / 65535;
  const s = x <= 0.0031308 ? x * 12.92 : 1.055 * x ** (1 / 2.4) - 0.055;
  TO_SRGB[i] = Math.min(255, Math.max(0, Math.floor(s * 255 + 0.5)));
}
function toSrgb(v) {
  return TO_SRGB[v <= 0 ? 0 : v >= 1 ? 65535 : Math.round(v * 65535)];
}

/**
 * Halve an RGBA image (w, h even). `kind`: 'cards' (colour alpha-weighted in linear light, alpha
 * averaged), 'colour' (linear light, alpha averaged), 'raw' (every channel averaged as stored: normal
 * and occlusion maps' own values).
 */
function halve(rgba, w, h, kind) {
  const dw = w >> 1, dh = h >> 1;
  const out = new Uint8Array(dw * dh * 4);
  for (let y = 0; y < dh; y++) {
    for (let x = 0; x < dw; x++) {
      let r = 0, g = 0, b = 0, a = 0, wsum = 0;
      for (let sy = 0; sy < 2; sy++) for (let sx = 0; sx < 2; sx++) {
        const s = ((y * 2 + sy) * w + x * 2 + sx) * 4;
        const al = rgba[s + 3] / 255;
        if (kind === 'raw') { r += rgba[s]; g += rgba[s + 1]; b += rgba[s + 2]; a += rgba[s + 3]; continue; }
        const wt = kind === 'cards' ? al : 1;
        r += TO_LINEAR[rgba[s]] * wt; g += TO_LINEAR[rgba[s + 1]] * wt; b += TO_LINEAR[rgba[s + 2]] * wt;
        wsum += wt;
        a += al;
      }
      const d = (y * dw + x) * 4;
      if (kind === 'raw') {
        out[d] = (r + 2) >> 2; out[d + 1] = (g + 2) >> 2; out[d + 2] = (b + 2) >> 2; out[d + 3] = (a + 2) >> 2;
      } else {
        const ws = Math.max(wsum, 1e-6);
        out[d] = toSrgb(r / ws); out[d + 1] = toSrgb(g / ws); out[d + 2] = toSrgb(b / ws);
        out[d + 3] = Math.min(255, Math.floor((a / 4) * 255 + 0.5));
      }
    }
  }
  return out;
}

/**
 * Double a square RGBA map's size, each new texel bilinear between the four nearest (edges held),
 * every channel as stored: for a normal or occlusion map smaller than the colour it goes with (the
 * fedoras' normal maps are 512 beside a 2048 diffuse).
 */
function doubleRaw(rgba, w) {
  const dw = w * 2;
  const out = new Uint8Array(dw * dw * 4);
  const at = (x, y, c) => rgba[((y < 0 ? 0 : y >= w ? w - 1 : y) * w + (x < 0 ? 0 : x >= w ? w - 1 : x)) * 4 + c];
  for (let y = 0; y < dw; y++) {
    // The new texel's centre in the old map's texels: (y + 0.5) / 2 - 0.5.
    const sy = (y - 0.5) / 2;
    const y0 = Math.floor(sy), fy = sy - y0;
    for (let x = 0; x < dw; x++) {
      const sx = (x - 0.5) / 2;
      const x0 = Math.floor(sx), fx = sx - x0;
      const o = (y * dw + x) * 4;
      for (let c = 0; c < 4; c++) {
        const top = at(x0, y0, c) * (1 - fx) + at(x0 + 1, y0, c) * fx;
        const bot = at(x0, y0 + 1, c) * (1 - fx) + at(x0 + 1, y0 + 1, c) * fx;
        out[o + c] = Math.round(top * (1 - fy) + bot * fy);
      }
    }
  }
  return out;
}

/**
 * A square map brought to `size`: halved while larger (a power of two, or refused), and for 'raw'
 * maps (normal, occlusion) doubled while smaller.
 */
function toSize(img, size, kind, what) {
  if (img.w !== img.h) throw new Error(`${what}: ${img.w} by ${img.h} is not square`);
  if (img.w & (img.w - 1)) throw new Error(`${what}: ${img.w} texels is not a power of two`);
  if (img.w < size && kind !== 'raw') throw new Error(`${what}: ${img.w} texels is smaller than the pack's ${size}`);
  let { rgba, w } = img;
  while (w > size) { rgba = halve(rgba, w, w, kind); w >>= 1; }
  while (w < size) { rgba = doubleRaw(rgba, w); w <<= 1; }
  return rgba;
}

function coverage(rgba, scale) {
  const thr = CARD_CUTOFF * 255;
  let hit = 0;
  for (let i = 3; i < rgba.length; i += 4) if (rgba[i] * scale >= thr) hit++;
  return hit / (rgba.length / 4);
}

/** Scale a level's alpha so its covered share matches `target` (binary search, billboard_bake's). */
function keepCoverage(rgba, target) {
  if (target <= 0) return rgba;
  let lo = 0, hi = 16, best = 1, bestErr = Infinity;
  for (let k = 0; k < 16; k++) {
    const mid = 0.5 * (lo + hi);
    const cov = coverage(rgba, mid);
    const err = Math.abs(cov - target);
    if (err < bestErr) { bestErr = err; best = mid; }
    if (cov > target) hi = mid; else lo = mid;
  }
  const out = Uint8Array.from(rgba);
  for (let i = 3; i < out.length; i += 4) out[i] = Math.min(255, Math.floor(out[i] * best));
  return out;
}

/** A half's mip chain, biggest first, down to MIP_MIN_PX. */
function chain(base, size, kind) {
  const levels = [base];
  if (kind === 'cards') {
    const target = coverage(base, 1);
    let unscaled = base;
    for (let w = size; w > MIP_MIN_PX; w >>= 1) {
      unscaled = halve(unscaled, w, w, 'cards');
      levels.push(keepCoverage(unscaled, target));
    }
  } else {
    let cur = base;
    for (let w = size; w > MIP_MIN_PX; w >>= 1) {
      cur = halve(cur, w, w, 'colour');
      levels.push(cur);
    }
  }
  return levels;
}

/**
 * Pack and encode one garment. `diffuse`, `normal`, `ao`: the PNG files' bytes (normal and ao may
 * be null); `cards`: drawn as alpha cards. Returns { size, dds, meanLuma, hasNormal, psnr }.
 */
function packAndEncode({ id, diffuse, normal, ao, cards }) {
  const d = decodePng(Buffer.from(diffuse));
  const size = Math.min(d.w, TEXTURE_PX);
  let colour = toSize(d, size, cards ? 'cards' : 'colour', `${id} diffuse`);
  colour = Uint8Array.from(colour);
  if (ao) {
    const occ = toSize(decodePng(Buffer.from(ao)), size, 'raw', `${id} ao`);
    for (let i = 0; i < colour.length; i += 4) {
      const o = occ[i] / 255;
      for (let k = 0; k < 3; k++) colour[i + k] = toSrgb(TO_LINEAR[colour[i + k]] * o);
    }
  }
  if (!cards) for (let i = 3; i < colour.length; i += 4) colour[i] = 255;
  let normals;
  if (normal) {
    const raw = toSize(decodePng(Buffer.from(normal)), size, 'raw', `${id} normal`);
    normals = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i += 4) {
      for (let k = 0; k < 3; k++) normals[i + k] = toSrgb(raw[i + k] / 255);
      normals[i + 3] = 255;
    }
  } else {
    const half = toSrgb(0.5);
    normals = new Uint8Array(size * size * 4);
    for (let i = 0; i < normals.length; i += 4) { normals[i] = half; normals[i + 1] = half; normals[i + 2] = 255; normals[i + 3] = 255; }
  }
  // The colour half's mean luminance (linear), over covered texels on a grid of every 7th.
  let lum = 0, n = 0;
  for (let t = 0; t < size * size; t += 7) {
    const i = t * 4;
    if (colour[i + 3] >= 128) { lum += 0.2126 * TO_LINEAR[colour[i]] + 0.7152 * TO_LINEAR[colour[i + 1]] + 0.0722 * TO_LINEAR[colour[i + 2]]; n++; }
  }
  const meanLuma = n ? Math.round((lum / n) * 1000) / 1000 : 0.5;
  const left = chain(colour, size, cards ? 'cards' : 'colour');
  const right = chain(normals, size, 'colour');
  const blocks = [];
  const psnr = { colour: 0, normal: 0, alpha: null };
  for (let l = 0; l < left.length; l++) {
    const s = size >> l;
    const lb = bc7.encodeImage(left[l], s, s, { cards });
    const rb = bc7.encodeImage(right[l], s, s, {});
    // Side by side: each block row of the level is the left half's blocks, then the right's.
    const bw = s / 4;
    const level = Buffer.alloc(bw * 2 * bw * 16);
    for (let by = 0; by < bw; by++) {
      lb.copy(level, by * bw * 2 * 16, by * bw * 16, (by + 1) * bw * 16);
      rb.copy(level, (by * bw * 2 + bw) * 16, by * bw * 16, (by + 1) * bw * 16);
    }
    blocks.push(level);
    if (l === 0) {
      const ld = bc7.decodeImage(lb, s, s);
      const rd = bc7.decodeImage(rb, s, s);
      // A card's colour is measured where it shows (alpha at least a half): the encoder weighs a
      // texel's colour by its alpha, so colour under transparent texels is not kept, by design.
      psnr.colour = round2(bc7.psnr(left[0], ld, [0, 1, 2], cards));
      psnr.normal = round2(bc7.psnr(right[0], rd, [0, 1, 2]));
      if (cards) psnr.alpha = round2(bc7.psnr(left[0], ld, [3]));
    }
  }
  return { size, dds: bc7.writeDds(size * 2, size, blocks), meanLuma, hasNormal: !!normal, psnr };
}

// Two decimals; an exact match (no error at all) as the word.
const round2 = (x) => (Number.isFinite(x) ? Math.round(x * 100) / 100 : 'exact');

/**
 * Pack and encode every job on up to `threads` worker threads; results in the jobs' order (so the
 * output is the same whatever order the workers finish in).
 */
async function packAll(jobs, threads) {
  const { Worker } = require('worker_threads');
  const results = new Array(jobs.length);
  let next = 0;
  const run = () => new Promise((resolve, reject) => {
    const w = new Worker(__filename);
    const feed = () => {
      if (next >= jobs.length) { w.terminate(); resolve(); return; }
      const k = next++;
      w.once('message', (m) => {
        if (m.error) { w.terminate(); reject(new Error(m.error)); return; }
        results[k] = { ...m, dds: Buffer.from(m.dds.buffer, m.dds.byteOffset, m.dds.length) };
        feed();
      });
      w.postMessage(jobs[k]);
    };
    w.once('error', reject);
    feed();
  });
  await Promise.all(Array.from({ length: Math.max(1, Math.min(threads, jobs.length)) }, run));
  return results;
}

const { isMainThread, parentPort } = require('worker_threads');
if (!isMainThread && parentPort) {
  parentPort.on('message', (job) => {
    try {
      const r = packAndEncode(job);
      // A fresh array of its own (a Buffer may sit in a shared pool), handed over without a copy.
      const dds = new Uint8Array(r.dds);
      parentPort.postMessage({ ...r, dds }, [dds.buffer]);
    } catch (e) {
      parentPort.postMessage({ error: `${job.id}: ${e.message}` });
    }
  });
}

module.exports = { packAndEncode, packAll, decodePng, TEXTURE_PX, MIP_MIN_PX };
