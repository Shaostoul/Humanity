// BC7: OUR OWN ENCODER AND DECODER (the look's garment textures, 2026-11-10; docs/design/
// character-animation.md section 16, the encoder scan in docs/reference/research/2026-11-10-bc7-
// encoders/). Plain JavaScript with no dependency, so the converter (scripts/gen-makehuman-look.js)
// encodes at CONVERSION time, deterministically, and nothing has to be downloaded or built.
//
// THE FORMAT (Microsoft's "BC7 Format" page, read 2026-11-10, and the bit layouts every BC7 decoder
// shares): a 4 by 4 block of texels is 16 bytes, its mode given by the count of zero bits before the
// first one bit, least significant bit first. A block holds one, two or three "subsets", each a line
// between two endpoints; each texel stores an index along its subset's line. The interpolation is
// ((64 - w) * e0 + w * e1 + 32) >> 6 with w from the 2, 3 or 4 bit weight table below, and the page
// says it plainly: "BC7 decompression hardware must be bit accurate". So a block our encoder writes
// decodes to the same texels on every GPU, and `decodeBlock` here is that decoder (the Rust twin,
// src/renderer/bc7.rs, is checked against the GPU's own decoder in its tests).
//
// WHICH MODES. The encoder writes modes 1, 3 and 6 for opaque blocks and 5, 6 and 7 for blocks with
// alpha: one subset (6: RGBA, 7 bits and a p-bit, 16 levels; 5: colour and alpha apart, 4 levels
// each) and two subsets (1: RGB, 6 bits and a shared p-bit, 8 levels; 3: RGB, 7 bits and a p-bit each,
// 4 levels; 7: RGBA, 5 bits and a p-bit each, 4 levels). Modes 0 and 2 (three subsets) and 4 are
// never written, so the decoders refuse modes 0 and 2 rather than carry a table no block of ours
// uses. The two-subset partitions and their fix-up texels are the format's own tables.
//
// THE FIT, per subset: the principal axis of its texels (power iteration on their covariance), the
// endpoints at the ends of their projections, each endpoint quantized to the mode's bits (the p-bit
// that fits it best), every texel's index chosen by its error, then the endpoints refitted by least
// squares to those indices and quantized again, kept while the error falls. A card's colour counts by
// its alpha (a texel nobody sees does not pull the endpoints), its alpha in full. Two-subset modes try
// the partitions closest to the block's own two-way split. A block whose mode 6 fit is already close
// skips the rest. Error is the sum of squared differences in the texture's own 8-bit values.

'use strict';

const W2 = [0, 21, 43, 64];
const W3 = [0, 9, 18, 27, 37, 46, 55, 64];
const W4 = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
const WEIGHTS = { 2: W2, 3: W3, 4: W4 };

// The 64 two-subset partitions: bit i set when texel i (row-major, y * 4 + x) is in subset 1.
const PARTITIONS2 = [
  0xcccc, 0x8888, 0xeeee, 0xecc8, 0xc880, 0xfeec, 0xfec8, 0xec80, 0xc800, 0xffec, 0xfe80, 0xe800, 0xffe8, 0xff00, 0xfff0, 0xf000,
  0xf710, 0x008e, 0x7100, 0x08ce, 0x008c, 0x7310, 0x3100, 0x8cce, 0x088c, 0x3110, 0x6666, 0x366c, 0x17e8, 0x0ff0, 0x718e, 0x399c,
  0xaaaa, 0xf0f0, 0x5a5a, 0x33cc, 0x3c3c, 0x55aa, 0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc, 0x6996, 0xc33c, 0x9966, 0x0660,
  0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6, 0x639c, 0x9336, 0x9cc6, 0x817e, 0xe718, 0xccf0, 0x0fcc, 0x7744, 0xee22,
];
// The fix-up texel of subset 1 for each two-subset partition (subset 0's is always texel 0): its
// index's top bit is implied 0, so it is stored with one bit fewer.
const ANCHORS2 = [
  15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
  15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2,
  15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6,
  6, 2, 6, 8, 15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15,
];

// Each mode the decoder reads: subsets, partition bits, rotation bits, index-selector bit, colour
// and alpha endpoint bits (0: no alpha, decoded 255), p-bits ('u' one per endpoint, 's' one per
// subset), primary and secondary index bits.
const MODE = [
  null, // 0: three subsets, never written, refused
  { ns: 2, pb: 6, rb: 0, isb: 0, cb: 6, ab: 0, p: 's', ib: 3, ib2: 0 },
  null, // 2: three subsets, never written, refused
  { ns: 2, pb: 6, rb: 0, isb: 0, cb: 7, ab: 0, p: 'u', ib: 2, ib2: 0 },
  { ns: 1, pb: 0, rb: 2, isb: 1, cb: 5, ab: 6, p: '', ib: 2, ib2: 3 },
  { ns: 1, pb: 0, rb: 2, isb: 0, cb: 7, ab: 8, p: '', ib: 2, ib2: 2 },
  { ns: 1, pb: 0, rb: 0, isb: 0, cb: 7, ab: 7, p: 'u', ib: 4, ib2: 0 },
  { ns: 2, pb: 6, rb: 0, isb: 0, cb: 5, ab: 5, p: 'u', ib: 2, ib2: 0 },
];

/** An endpoint value of `prec` bits widened to 8 by repeating its top bits (the format's rule). */
function expand(v, prec) {
  return prec >= 8 ? v : ((v << (8 - prec)) | (v >> (2 * prec - 8))) & 0xff;
}

function interp(e0, e1, w) {
  return ((64 - w) * e0 + w * e1 + 32) >> 6;
}

// ── Bits, least significant first ─────────────────────────────────────────
class BitReader {
  constructor(buf, off) {
    this.buf = buf;
    this.off = off;
    this.pos = 0;
  }
  bits(n) {
    let v = 0;
    for (let k = 0; k < n; k++) {
      const p = this.pos++;
      v |= ((this.buf[this.off + (p >> 3)] >> (p & 7)) & 1) << k;
    }
    return v;
  }
}

class BitWriter {
  constructor(out, off) {
    this.out = out;
    this.off = off;
    this.pos = 0;
    for (let i = 0; i < 16; i++) out[off + i] = 0;
  }
  put(v, n) {
    for (let k = 0; k < n; k++) {
      if ((v >> k) & 1) this.out[this.off + (this.pos >> 3)] |= 1 << (this.pos & 7);
      this.pos++;
    }
  }
}

// ── The decoder ───────────────────────────────────────────────────────────
/**
 * Decode one block at `buf[off..off+16]` into `out` (64 bytes RGBA, row-major) at `o`. Throws on
 * modes 0 and 2 (three subsets; our encoder never writes them). A block with no mode bit (the
 * format's reserved encoding) decodes to transparent black, as the format says.
 */
function decodeBlock(buf, off, out, o = 0) {
  const r = new BitReader(buf, off);
  let mode = 0;
  while (mode < 8 && r.bits(1) === 0) mode++;
  if (mode === 8) {
    out.fill(0, o, o + 64);
    return;
  }
  const m = MODE[mode];
  if (!m) throw new Error(`BC7 mode ${mode} (three subsets) is not written by our encoder and not read here`);
  const partition = m.pb ? r.bits(m.pb) : 0;
  const rotation = m.rb ? r.bits(m.rb) : 0;
  const isel = m.isb ? r.bits(1) : 0;
  const ne = m.ns * 2;
  const ep = [];
  for (let e = 0; e < ne; e++) ep.push([0, 0, 0, 255]);
  for (let c = 0; c < 3; c++) for (let e = 0; e < ne; e++) ep[e][c] = r.bits(m.cb);
  if (m.ab) for (let e = 0; e < ne; e++) ep[e][3] = r.bits(m.ab);
  let cp = m.cb;
  let ap = m.ab;
  if (m.p === 'u') {
    for (let e = 0; e < ne; e++) {
      const p = r.bits(1);
      for (let c = 0; c < 3; c++) ep[e][c] = (ep[e][c] << 1) | p;
      if (m.ab) ep[e][3] = (ep[e][3] << 1) | p;
    }
    cp++;
    if (m.ab) ap++;
  } else if (m.p === 's') {
    for (let s = 0; s < m.ns; s++) {
      const p = r.bits(1);
      for (const e of [2 * s, 2 * s + 1]) for (let c = 0; c < 3; c++) ep[e][c] = (ep[e][c] << 1) | p;
    }
    cp++;
  }
  for (let e = 0; e < ne; e++) {
    for (let c = 0; c < 3; c++) ep[e][c] = expand(ep[e][c], cp);
    ep[e][3] = m.ab ? expand(ep[e][3], ap) : 255;
  }
  const mask = m.ns === 2 ? PARTITIONS2[partition] : 0;
  const anchor = m.ns === 2 ? ANCHORS2[partition] : -1;
  const i1 = new Array(16);
  for (let i = 0; i < 16; i++) i1[i] = r.bits(m.ib - (i === 0 || i === anchor ? 1 : 0));
  let i2 = null;
  if (m.ib2) {
    i2 = new Array(16);
    for (let i = 0; i < 16; i++) i2[i] = r.bits(m.ib2 - (i === 0 ? 1 : 0));
  }
  for (let i = 0; i < 16; i++) {
    const s = (mask >> i) & 1;
    const e0 = ep[2 * s];
    const e1 = ep[2 * s + 1];
    let ci = i1[i], cbits = m.ib, ai = i1[i], abits = m.ib;
    if (i2) {
      if (isel) {
        ci = i2[i]; cbits = m.ib2; ai = i1[i]; abits = m.ib;
      } else {
        ci = i1[i]; cbits = m.ib; ai = i2[i]; abits = m.ib2;
      }
    }
    const wc = WEIGHTS[cbits][ci];
    const wa = WEIGHTS[abits][ai];
    const px = [interp(e0[0], e1[0], wc), interp(e0[1], e1[1], wc), interp(e0[2], e1[2], wc), interp(e0[3], e1[3], wa)];
    if (rotation) {
      const t = px[3];
      px[3] = px[rotation - 1];
      px[rotation - 1] = t;
    }
    out[o + i * 4] = px[0];
    out[o + i * 4 + 1] = px[1];
    out[o + i * 4 + 2] = px[2];
    out[o + i * 4 + 3] = px[3];
  }
}

/** The mode a block was written in (8: the reserved encoding). */
function blockMode(buf, off) {
  for (let m = 0; m < 8; m++) if ((buf[off + (m >> 3)] >> (m & 7)) & 1) return m;
  return 8;
}

// ── The encoder ───────────────────────────────────────────────────────────
// Quantization tables: for an endpoint of `bits` bits with p-bit `p` (or none), the stored value
// whose widened 8-bit value is nearest each 8-bit target.
const QUANT = [];
function quantTable(bits, p) {
  const key = bits * 3 + p + 1;
  if (QUANT[key]) return QUANT[key];
  const q = new Uint8Array(256);
  const e = new Uint8Array(256);
  const n = 1 << bits;
  for (let x = 0; x < 256; x++) {
    let best = 0, bd = 1e9, bv = 0;
    for (let v = 0; v < n; v++) {
      const w = p < 0 ? expand(v, bits) : expand((v << 1) | p, bits + 1);
      const d = Math.abs(w - x);
      if (d < bd) { bd = d; best = v; bv = w; }
    }
    q[x] = best;
    e[x] = bv;
  }
  const t = { q, e };
  QUANT[key] = t;
  return t;
}

const clamp255 = (x) => (x < 0 ? 0 : x > 255 ? 255 : Math.round(x));

// Scratch for a subset's palette (16 levels of RGBA).
const PAL = new Int32Array(64);
// For each index width, the level whose weight is nearest each of 0..64.
const NEAREST = {};
for (const ib of [2, 3, 4]) {
  const W = WEIGHTS[ib];
  NEAREST[ib] = new Uint8Array(65);
  for (let t = 0; t <= 64; t++) {
    let bk = 0;
    for (let k = 1; k < W.length; k++) if (Math.abs(W[k] - t) < Math.abs(W[bk] - t)) bk = k;
    NEAREST[ib][t] = bk;
  }
}

/**
 * Encode one subset of a block. `px`: the block's 64 RGBA bytes; `members`: texel indices; `nc`: 3
 * (RGB, alpha decoded 255) or 4 (RGBA); `cw` per texel per channel weights (Float64Array 64);
 * `bits`, `pmode` ('u' a p-bit each, 'U' both 1, 's' one shared, '' none), `ib` index bits. Returns the quantized endpoints (stored values
 * and p-bits), the widened endpoints, each member's index and the weighted error.
 */
function fitSubset(px, members, nc, cw, bits, pmode, ib) {
  const W = WEIGHTS[ib];
  const levels = W.length;
  const n = members.length;
  // Weighted mean and covariance.
  const mean = [0, 0, 0, 0];
  const wsum = [0, 0, 0, 0];
  for (const i of members) for (let c = 0; c < nc; c++) { mean[c] += cw[i * 4 + c] * px[i * 4 + c]; wsum[c] += cw[i * 4 + c]; }
  for (let c = 0; c < nc; c++) mean[c] = wsum[c] > 1e-9 ? mean[c] / wsum[c] : 0;
  const cov = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  const dv = [0, 0, 0, 0];
  for (const i of members) {
    for (let a = 0; a < nc; a++) dv[a] = (px[i * 4 + a] - mean[a]) * Math.sqrt(cw[i * 4 + a]);
    for (let a = 0; a < nc; a++) for (let b = a; b < nc; b++) cov[a * 4 + b] += dv[a] * dv[b];
  }
  for (let a = 0; a < nc; a++) for (let b = 0; b < a; b++) cov[a * 4 + b] = cov[b * 4 + a];
  // Power iteration from the channel of most spread.
  let axis = [0, 0, 0, 0];
  let top = 0;
  for (let c = 1; c < nc; c++) if (cov[c * 5] > cov[top * 5]) top = c;
  for (let c = 0; c < nc; c++) axis[c] = cov[top * 4 + c];
  let norm = Math.hypot(axis[0], axis[1], axis[2], axis[3]);
  if (norm < 1e-9) { axis = [1, 1, 1, nc === 4 ? 1 : 0]; norm = Math.hypot(...axis); }
  for (let c = 0; c < 4; c++) axis[c] /= norm;
  for (let it = 0; it < 6; it++) {
    const nx = [0, 0, 0, 0];
    for (let a = 0; a < nc; a++) for (let b = 0; b < nc; b++) nx[a] += cov[a * 4 + b] * axis[b];
    const l = Math.hypot(nx[0], nx[1], nx[2], nx[3]);
    if (l < 1e-9) break;
    for (let c = 0; c < 4; c++) axis[c] = nx[c] / l;
  }
  let tmin = Infinity, tmax = -Infinity;
  for (const i of members) {
    let t = 0;
    for (let c = 0; c < nc; c++) t += (px[i * 4 + c] - mean[c]) * axis[c];
    if (t < tmin) tmin = t;
    if (t > tmax) tmax = t;
  }
  if (!Number.isFinite(tmin)) { tmin = 0; tmax = 0; }
  let f0 = [0, 0, 0, 0], f1 = [0, 0, 0, 0];
  for (let c = 0; c < nc; c++) { f0[c] = mean[c] + axis[c] * tmin; f1[c] = mean[c] + axis[c] * tmax; }
  if (nc === 3) { f0[3] = 255; f1[3] = 255; }

  const best = { err: Infinity };
  const idx = new Uint8Array(16);
  const pal = PAL;
  const tryEndpoints = (g0, g1) => {
    const q = quantize(g0, g1, nc, bits, pmode);
    // Each member's index by its error: its projection on the endpoints' line gives the nearest
    // level, and that level and its two neighbours are measured (the weights are not quite even, and
    // a card's colour counts by alpha, so the projection alone can miss by one).
    let err = 0;
    for (let k = 0; k < levels; k++) for (let c = 0; c < 4; c++) pal[k * 4 + c] = interp(q.w0[c], q.w1[c], W[k]);
    let dd = 0;
    const dir = [q.w1[0] - q.w0[0], q.w1[1] - q.w0[1], q.w1[2] - q.w0[2], nc === 4 ? q.w1[3] - q.w0[3] : 0];
    for (let c = 0; c < 4; c++) dd += dir[c] * dir[c];
    const alphaFixed = nc === 3;
    for (let mi = 0; mi < members.length; mi++) {
      const i = members[mi];
      const o = i * 4;
      let k0 = 0;
      if (dd > 0) {
        let t = 0;
        for (let c = 0; c < 4; c++) t += (px[o + c] - q.w0[c]) * dir[c];
        k0 = NEAREST[ib][Math.max(0, Math.min(64, Math.round((t / dd) * 64)))];
      }
      let bk = k0, be = Infinity;
      const lo = k0 > 0 ? k0 - 1 : 0, hi = k0 < levels - 1 ? k0 + 1 : levels - 1;
      for (let k = lo; k <= hi; k++) {
        const p = k * 4;
        const d0 = pal[p] - px[o], d1 = pal[p + 1] - px[o + 1], d2 = pal[p + 2] - px[o + 2];
        let e = d0 * d0 * cw[o] + d1 * d1 * cw[o + 1] + d2 * d2 * cw[o + 2];
        const d3 = (alphaFixed ? 255 : pal[p + 3]) - px[o + 3];
        e += d3 * d3 * cw[o + 3];
        if (e < be) { be = e; bk = k; }
      }
      idx[i] = bk;
      err += be;
    }
    if (err < best.err) {
      best.err = err;
      best.q = q;
      best.idx = Uint8Array.from(idx);
    }
    return err;
  };
  tryEndpoints(f0, f1);
  // Least squares refits: the endpoints that best explain the chosen indices, channel by channel.
  for (let it = 0; it < 2 && best.err > 0; it++) {
    const g0 = [0, 0, 0, 255], g1 = [0, 0, 0, 255];
    let ok = true;
    for (let c = 0; c < nc; c++) {
      let A = 0, B = 0, C = 0, D0 = 0, D1 = 0;
      for (const i of members) {
        const w = cw[i * 4 + c];
        const t = W[best.idx[i]] / 64;
        const x = px[i * 4 + c];
        A += w * (1 - t) * (1 - t); B += w * (1 - t) * t; C += w * t * t; D0 += w * (1 - t) * x; D1 += w * t * x;
      }
      const det = A * C - B * B;
      if (Math.abs(det) < 1e-9) { ok = false; break; }
      g0[c] = (C * D0 - B * D1) / det;
      g1[c] = (A * D1 - B * D0) / det;
    }
    if (!ok) break;
    const before = best.err;
    tryEndpoints(g0, g1);
    if (best.err >= before) break;
  }
  void n;
  return best;
}

/** Quantize two float endpoints to stored values with the p-bits that fit them best. */
function quantize(g0, g1, nc, bits, pmode) {
  const pick = (g, p) => {
    const t = quantTable(bits, p);
    const v = [0, 0, 0, 0], w = [0, 0, 0, 255];
    let err = 0;
    for (let c = 0; c < 4; c++) {
      if (c === 3 && nc === 3) continue;
      const x = clamp255(g[c]);
      v[c] = t.q[x];
      w[c] = t.e[x];
      err += (w[c] - g[c]) * (w[c] - g[c]);
    }
    return { v, w, err };
  };
  if (pmode === 'U') {
    // Both p-bits 1 (an opaque block in mode 6: alpha 127 and a p-bit of 1 is exactly 255).
    const a = pick(g0, 1), b = pick(g1, 1);
    return { v0: a.v, v1: b.v, w0: a.w, w1: b.w, p0: 1, p1: 1 };
  }
  if (pmode === 'u') {
    const a = [pick(g0, 0), pick(g0, 1)];
    const b = [pick(g1, 0), pick(g1, 1)];
    const p0 = a[1].err < a[0].err ? 1 : 0;
    const p1 = b[1].err < b[0].err ? 1 : 0;
    return { v0: a[p0].v, v1: b[p1].v, w0: a[p0].w, w1: b[p1].w, p0, p1 };
  }
  if (pmode === 's') {
    const z = [pick(g0, 0), pick(g1, 0)];
    const o = [pick(g0, 1), pick(g1, 1)];
    const p = o[0].err + o[1].err < z[0].err + z[1].err ? 1 : 0;
    const s = p ? o : z;
    return { v0: s[0].v, v1: s[1].v, w0: s[0].w, w1: s[1].w, p0: p, p1: p };
  }
  const a = pick(g0, -1), b = pick(g1, -1);
  return { v0: a.v, v1: b.v, w0: a.w, w1: b.w, p0: 0, p1: 0 };
}

/** Make a subset's fix-up texel's index top bit 0 by swapping its endpoints (and indices). */
function fixAnchor(fit, members, anchor, levels) {
  if (fit.idx[anchor] < levels / 2) return;
  const q = fit.q;
  [q.v0, q.v1] = [q.v1, q.v0];
  [q.w0, q.w1] = [q.w1, q.w0];
  [q.p0, q.p1] = [q.p1, q.p0];
  for (const i of members) fit.idx[i] = levels - 1 - fit.idx[i];
}

const ALL = Array.from({ length: 16 }, (_, i) => i);

function encodeMode6(px, cw, opaque) {
  // An opaque block keeps both p-bits at 1 so its alpha decodes 255 exactly (a p-bit of 0 tops out
  // at 254).
  const fit = fitSubset(px, ALL, 4, cw, 7, opaque ? 'U' : 'u', 4);
  fixAnchor(fit, ALL, 0, 16);
  return {
    err: fit.err,
    write(out, o) {
      const w = new BitWriter(out, o);
      w.put(1 << 6, 7);
      const q = fit.q;
      for (let c = 0; c < 4; c++) { w.put(q.v0[c], 7); w.put(q.v1[c], 7); }
      w.put(q.p0, 1); w.put(q.p1, 1);
      for (let i = 0; i < 16; i++) w.put(fit.idx[i], i === 0 ? 3 : 4);
    },
  };
}

/** Mode 5: colour (7 bits) and alpha (8 bits) apart, 2-bit indices each, no rotation. */
function encodeMode5(px, cw) {
  const fc = fitSubset(px, ALL, 3, cw, 7, '', 2);
  fixAnchor(fc, ALL, 0, 4);
  // Alpha alone: a scalar fit (no colour counted).
  const aw = new Float64Array(64);
  const ap = new Uint8Array(64);
  for (let i = 0; i < 16; i++) { ap[i * 4] = px[i * 4 + 3]; aw[i * 4] = cw[i * 4 + 3]; }
  const fa = fitScalar(ap, aw);
  fixAnchor(fa, ALL, 0, 4);
  // fitSubset counted colour with alpha decoded 255; take that alpha term back out.
  let alphaAt255 = 0;
  for (let i = 0; i < 16; i++) { const d = 255 - px[i * 4 + 3]; alphaAt255 += d * d * cw[i * 4 + 3]; }
  return {
    err: fc.err - alphaAt255 + fa.err,
    write(out, o) {
      const w = new BitWriter(out, o);
      w.put(1 << 5, 6);
      w.put(0, 2);
      const q = fc.q;
      for (let c = 0; c < 3; c++) { w.put(q.v0[c], 7); w.put(q.v1[c], 7); }
      w.put(fa.q.v0[0], 8); w.put(fa.q.v1[0], 8);
      for (let i = 0; i < 16; i++) w.put(fc.idx[i], i === 0 ? 1 : 2);
      for (let i = 0; i < 16; i++) w.put(fa.idx[i], i === 0 ? 1 : 2);
    },
  };
}

/** One 8-bit channel (in channel 0 of `px`) with 2-bit indices: endpoints from its range, refitted. */
function fitScalar(px, cw) {
  // Reuse the general fit with one channel: nc 3 with the other two zero-weighted would distort the
  // axis, so a direct scalar fit.
  let lo = 255, hi = 0;
  for (let i = 0; i < 16; i++) { const v = px[i * 4]; if (cw[i * 4] > 0) { if (v < lo) lo = v; if (v > hi) hi = v; } }
  if (lo > hi) { lo = 0; hi = 0; }
  const best = { err: Infinity };
  const run = (e0, e1) => {
    e0 = clamp255(e0); e1 = clamp255(e1);
    const idx = new Uint8Array(16);
    let err = 0;
    for (let i = 0; i < 16; i++) {
      let bk = 0, be = Infinity;
      for (let k = 0; k < 4; k++) { const d = interp(e0, e1, W2[k]) - px[i * 4]; const e = d * d * cw[i * 4]; if (e < be) { be = e; bk = k; } }
      idx[i] = bk;
      err += be;
    }
    if (err < best.err) { best.err = err; best.idx = idx; best.q = { v0: [e0], v1: [e1], w0: [e0], w1: [e1], p0: 0, p1: 0 }; }
  };
  run(lo, hi);
  for (let it = 0; it < 2 && best.err > 0; it++) {
    let A = 0, B = 0, C = 0, D0 = 0, D1 = 0;
    for (let i = 0; i < 16; i++) {
      const w = cw[i * 4], t = W2[best.idx[i]] / 64, x = px[i * 4];
      A += w * (1 - t) * (1 - t); B += w * (1 - t) * t; C += w * t * t; D0 += w * (1 - t) * x; D1 += w * t * x;
    }
    const det = A * C - B * B;
    if (Math.abs(det) < 1e-9) break;
    const before = best.err;
    run((C * D0 - B * D1) / det, (A * D1 - B * D0) / det);
    if (best.err >= before) break;
  }
  return best;
}

/** The partitions nearest the block's own two-way split (2-means along its principal axis). */
function candidatePartitions(px, cw, nc, count) {
  // Principal axis of the whole block.
  const mean = [0, 0, 0, 0];
  let wt = 0;
  for (let i = 0; i < 16; i++) {
    const w = nc === 4 ? 1 : cw[i * 4];
    for (let c = 0; c < nc; c++) mean[c] += px[i * 4 + c] * w;
    wt += w;
  }
  for (let c = 0; c < nc; c++) mean[c] /= wt || 1;
  const cov = new Float64Array(16);
  for (let i = 0; i < 16; i++) for (let a = 0; a < nc; a++) for (let b = 0; b < nc; b++) cov[a * 4 + b] += (px[i * 4 + a] - mean[a]) * (px[i * 4 + b] - mean[b]);
  let axis = [1, 1, 1, nc === 4 ? 1 : 0];
  for (let it = 0; it < 6; it++) {
    const nx = [0, 0, 0, 0];
    for (let a = 0; a < nc; a++) for (let b = 0; b < nc; b++) nx[a] += cov[a * 4 + b] * axis[b];
    const l = Math.hypot(...nx);
    if (l < 1e-9) break;
    axis = nx.map((x) => x / l);
  }
  const t = PROJ;
  let c0 = Infinity, c1 = -Infinity;
  for (let i = 0; i < 16; i++) {
    let s = 0;
    for (let c = 0; c < nc; c++) s += (px[i * 4 + c] - mean[c]) * axis[c];
    t[i] = s;
    if (s < c0) c0 = s;
    if (s > c1) c1 = s;
  }
  // 2-means on the projections.
  let mask = 0;
  for (let it = 0; it < 4; it++) {
    mask = 0;
    let s0 = 0, n0 = 0, s1 = 0, n1 = 0;
    for (let i = 0; i < 16; i++) {
      if (Math.abs(t[i] - c1) < Math.abs(t[i] - c0)) { mask |= 1 << i; s1 += t[i]; n1++; } else { s0 += t[i]; n0++; }
    }
    if (n0) c0 = s0 / n0;
    if (n1) c1 = s1 / n1;
  }
  // The `count` partitions nearest that split (fewest texels on the other side; ties to the lower
  // number), by selection: no sort, no allocation but the answer.
  const out = [];
  const taken = TAKEN.fill(0);
  for (let n = 0; n < count; n++) {
    let bk = -1, bd = 99;
    for (let k = 0; k < 64; k++) {
      if (taken[k]) continue;
      let d = popcount16(PARTITIONS2[k] ^ mask);
      if (16 - d < d) d = 16 - d;
      if (d < bd) { bd = d; bk = k; }
    }
    taken[bk] = 1;
    out.push(bk);
  }
  return out;
}
const PROJ = new Float64Array(16);
const TAKEN = new Uint8Array(64);

function popcount16(x) {
  x = x - ((x >> 1) & 0x5555);
  x = (x & 0x3333) + ((x >> 2) & 0x3333);
  x = (x + (x >> 4)) & 0x0f0f;
  return (x + (x >> 8)) & 0x1f;
}

/** A two-subset mode (1, 3 or 7) in one partition. */
function encodeTwo(px, cw, mode, partition) {
  const m = MODE[mode];
  const nc = m.ab ? 4 : 3;
  const mask = PARTITIONS2[partition];
  const s0 = [], s1 = [];
  for (let i = 0; i < 16; i++) ((mask >> i) & 1 ? s1 : s0).push(i);
  const levels = WEIGHTS[m.ib].length;
  const f0 = fitSubset(px, s0, nc, cw, m.cb, m.p, m.ib);
  const f1 = fitSubset(px, s1, nc, cw, m.cb, m.p, m.ib);
  fixAnchor(f0, s0, 0, levels);
  fixAnchor(f1, s1, ANCHORS2[partition], levels);
  const anchor = ANCHORS2[partition];
  return {
    err: f0.err + f1.err,
    write(out, o) {
      const w = new BitWriter(out, o);
      w.put(1 << mode, mode + 1);
      w.put(partition, 6);
      const q = [f0.q, f1.q];
      for (let c = 0; c < nc; c++) for (const s of q) { w.put(s.v0[c], m.cb); w.put(s.v1[c], m.cb); }
      if (m.p === 'u') for (const s of q) { w.put(s.p0, 1); w.put(s.p1, 1); }
      else for (const s of q) w.put(s.p0, 1);
      for (let i = 0; i < 16; i++) {
        const v = (mask >> i) & 1 ? f1.idx[i] : f0.idx[i];
        w.put(v, m.ib - (i === 0 || i === anchor ? 1 : 0));
      }
    },
  };
}

// A mode 6 fit whose weighted error per texel (summed over its channels) is at or under this skips
// the other modes: about one step of 8 bits in each channel, 48 dB, where the eye sees nothing more.
const GOOD = 3.0;
// How many of the partitions nearest the block's own split a two-subset mode tries.
const PARTITION_TRIES = 2;

/**
 * Encode a 4 by 4 block. `px`: 64 bytes RGBA. `opts.cards`: alpha matters (hair, brows, lashes:
 * colour counts by alpha); otherwise the block is opaque (alpha written 255). `opts.good`: a mode 6
 * error per texel at or under which the other modes are not tried. Writes 16 bytes into `out` at
 * `o` and returns the block's weighted error.
 */
function encodeBlock(px, out, o, opts = {}) {
  const cards = !!opts.cards;
  const good = (opts.good ?? GOOD) * 16;
  const parts = opts.partitions ?? PARTITION_TRIES;
  const cw = new Float64Array(64);
  let opaque = true;
  for (let i = 0; i < 16; i++) {
    const a = px[i * 4 + 3];
    if (a !== 255) opaque = false;
    const colourW = cards ? Math.max(a / 255, 1 / 64) : 1;
    cw[i * 4] = colourW; cw[i * 4 + 1] = colourW; cw[i * 4 + 2] = colourW;
    cw[i * 4 + 3] = cards ? 1 : 1;
  }
  const tries = [];
  const m6 = encodeMode6(px, cw, opaque);
  tries.push(m6);
  if (m6.err > good) {
    if (cards && !opaque) {
      tries.push(encodeMode5(px, cw));
      for (const p of candidatePartitions(px, cw, 4, parts + 1)) tries.push(encodeTwo(px, cw, 7, p));
    } else {
      for (const p of candidatePartitions(px, cw, 3, parts)) { tries.push(encodeTwo(px, cw, 1, p)); tries.push(encodeTwo(px, cw, 3, p)); }
    }
  }
  let best = tries[0];
  for (const t of tries) if (t.err < best.err) best = t;
  best.write(out, o);
  return best.err;
}

/** Encode an RGBA image (width and height multiples of 4) into BC7 blocks, row by row. */
function encodeImage(rgba, w, h, opts = {}) {
  if (w % 4 || h % 4) throw new Error(`a BC7 image is whole blocks: ${w} by ${h}`);
  const bw = w / 4, bh = h / 4;
  const out = Buffer.alloc(bw * bh * 16);
  const px = new Uint8Array(64);
  const last = new Uint8Array(64);
  let haveLast = false;
  for (let by = 0; by < bh; by++) {
    for (let bx = 0; bx < bw; bx++) {
      for (let y = 0; y < 4; y++) for (let x = 0; x < 4; x++) {
        const s = ((by * 4 + y) * w + bx * 4 + x) * 4;
        const d = (y * 4 + x) * 4;
        px[d] = rgba[s]; px[d + 1] = rgba[s + 1]; px[d + 2] = rgba[s + 2]; px[d + 3] = rgba[s + 3];
      }
      const o = (by * bw + bx) * 16;
      // A block the same as the one before it (a flat normal map, a solid region) encodes the same:
      // its bytes are copied rather than fitted again (the encoder is a pure function of the block).
      let same = haveLast;
      for (let i = 0; same && i < 64; i++) if (px[i] !== last[i]) same = false;
      if (same) {
        out.copy(out, o, o - 16, o);
      } else {
        encodeBlock(px, out, o, opts);
        last.set(px);
        haveLast = true;
      }
    }
  }
  return out;
}

/** Decode BC7 blocks back to RGBA (w and h multiples of 4). */
function decodeImage(blocks, w, h) {
  const bw = w / 4, bh = h / 4;
  const out = new Uint8Array(w * h * 4);
  const px = new Uint8Array(64);
  for (let by = 0; by < bh; by++) {
    for (let bx = 0; bx < bw; bx++) {
      decodeBlock(blocks, (by * bw + bx) * 16, px, 0);
      for (let y = 0; y < 4; y++) out.set(px.subarray(y * 16, y * 16 + 16), ((by * 4 + y) * w + bx * 4) * 4);
    }
  }
  return out;
}

// ── DDS: the standard container (a DX10 header naming DXGI_FORMAT_BC7_UNORM_SRGB) ──
const DXGI_BC7_UNORM_SRGB = 99;

/** A DDS file: BC7 sRGB, `w` by `h` at level 0, `levels` whole block arrays biggest first. */
function writeDds(w, h, levels) {
  const head = Buffer.alloc(4 + 124 + 20);
  head.write('DDS ', 0, 'ascii');
  head.writeUInt32LE(124, 4); // dwSize
  head.writeUInt32LE(0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000, 8); // CAPS HEIGHT WIDTH PIXELFORMAT MIPMAPCOUNT LINEARSIZE
  head.writeUInt32LE(h, 12);
  head.writeUInt32LE(w, 16);
  head.writeUInt32LE(levels[0].length, 20); // linear size of level 0
  head.writeUInt32LE(0, 24); // depth
  head.writeUInt32LE(levels.length, 28); // mip count
  // dwReserved1[11] stay 0 (bytes 32..76). The pixel format at 76:
  head.writeUInt32LE(32, 76);
  head.writeUInt32LE(0x4, 80); // DDPF_FOURCC
  head.write('DX10', 84, 'ascii');
  head.writeUInt32LE(0x1000 | 0x400000 | 0x8, 108); // TEXTURE MIPMAP COMPLEX
  // The DX10 header at 128.
  head.writeUInt32LE(DXGI_BC7_UNORM_SRGB, 128);
  head.writeUInt32LE(3, 132); // TEXTURE2D
  head.writeUInt32LE(0, 136);
  head.writeUInt32LE(1, 140); // array size
  head.writeUInt32LE(1, 144); // DDS_ALPHA_MODE_STRAIGHT
  return Buffer.concat([head, ...levels]);
}

/** Read a DDS file this module wrote: { w, h, levels: [Buffer] }. Throws on anything else. */
function readDds(buf) {
  if (buf.length < 148 || buf.toString('ascii', 0, 4) !== 'DDS ' || buf.toString('ascii', 84, 88) !== 'DX10') throw new Error('not a DX10 DDS file');
  if (buf.readUInt32LE(128) !== DXGI_BC7_UNORM_SRGB) throw new Error(`DXGI format ${buf.readUInt32LE(128)}, not BC7 sRGB`);
  const h = buf.readUInt32LE(12), w = buf.readUInt32LE(16), n = buf.readUInt32LE(28);
  const levels = [];
  let p = 148;
  for (let l = 0; l < n; l++) {
    const bytes = Math.max(1, (w >> l) >> 2) * Math.max(1, (h >> l) >> 2) * 16;
    if (p + bytes > buf.length) throw new Error('a DDS file cut short');
    levels.push(buf.subarray(p, p + bytes));
    p += bytes;
  }
  if (p !== buf.length) throw new Error(`a DDS file with ${buf.length - p} bytes past its levels`);
  return { w, h, levels };
}

/**
 * Peak signal-to-noise ratio (dB) of `b` against `a` over the channels in `chans`; with `shown`,
 * only over the texels whose alpha in `a` is at least a half (a card's colour where it shows).
 */
function psnr(a, b, chans = [0, 1, 2, 3], shown = false) {
  let se = 0, n = 0;
  for (let i = 0; i < a.length; i += 4) {
    if (shown && a[i + 3] < 128) continue;
    for (const c of chans) { const d = a[i + c] - b[i + c]; se += d * d; n++; }
  }
  if (se === 0) return Infinity;
  return 10 * Math.log10((255 * 255) / (se / n));
}

module.exports = { encodeBlock, decodeBlock, encodeImage, decodeImage, blockMode, writeDds, readDds, psnr, expand, PARTITIONS2, ANCHORS2, WEIGHTS };
