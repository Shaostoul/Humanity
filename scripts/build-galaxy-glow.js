#!/usr/bin/env node
/**
 * build-galaxy-glow.js
 *
 * Bakes the Milky Way GLOW LAYER: integrates the light of every star in the
 * extended catalog (data/stars-athyg.bin, ~2.5M stars; falls back to
 * data/stars.bin, ~120k) into a 2048x1024 equirectangular texture,
 * data/galaxy_glow.png, which the renderer draws as an additive sky pass
 * BEHIND the star points (src/renderer/stars.rs). This replaces the old
 * procedural fake band (galactic_band_stars) with REAL integrated starlight:
 * the band is bright where the catalog is dense (the galactic plane) and
 * warmest at the bulge (Sagittarius), because that is what the data says.
 *
 * COMMIT the output PNG. Regenerate whenever the catalog binaries change:
 *   node scripts/build-galaxy-glow.js        (or after re-running
 *   build-stars-bin.js / build-athyg-bin.js). Takes ~10-30 s (the blur).
 *   For the full-quality bake, data/stars-athyg.bin must be present (it is
 *   gitignored; download it via Settings > Graphics or scripts/build-athyg-bin.js).
 *
 * ── THE EQUIRECT MAPPING CONTRACT (must match the shader EXACTLY) ─────────
 *
 *   Directions are unit vectors in the equatorial J2000 frame, the same frame
 *   the HOSSTAR1 catalog records store (x = vernal equinox, z = north
 *   celestial pole). The texture mapping is:
 *
 *     u = atan2(dir.y, dir.x) / (2*pi) + 0.5      (RA;  0..1, wraps at u=0/1)
 *     v = acos(clamp(dir.z, -1, 1)) / pi          (Dec; 0 = north pole,
 *                                                  0.5 = celestial equator,
 *                                                  1 = south pole)
 *
 *   This EXACT formula appears in three places and they MUST stay identical -
 *   it is the only alignment contract between the baked texture and the live
 *   star points:
 *     1. here (accumulation),
 *     2. assets/shaders/galaxy_glow.wgsl (fragment sampling) + the embedded
 *        fallback copy in src/renderer/stars.rs,
 *     3. src/renderer/stars.rs::dir_to_equirect_uv / equirect_uv_to_dir
 *        (the pure Rust port, unit-tested against known directions).
 *
 * ── Pipeline ──────────────────────────────────────────────────────────────
 *
 *   1. Read HOSSTAR1 records (16 B header, 15 B/star: f32x3 unit dir,
 *      u16 mag q=(mag+2)*1024, u8 ci over [-0.4, 2.0]).
 *   2. Per star: flux = 10^(-mag/2.5), CAPPED at the flux of mag CAP_MAG.
 *      Why the cap: bright stars (Sirius at -1.44 is ~150x a mag 4 star) are
 *      already rendered as resolved POINTS by the star pipeline; uncapped,
 *      the handful of brightest stars would each out-glow the entire
 *      galactic-core region in the baked texture and drag the peak away from
 *      the real galactic center. Capped, every star still contributes a
 *      little halo, but the glow is dominated by the MANY faint stars -
 *      which is physically what the Milky Way band is.
 *   3. Accumulate flux-weighted RGB (same ci_to_rgb curve as stars.rs) into
 *      the equirect grid.
 *   4. ANGULARLY-UNIFORM separable gaussian blur of the flux field, two
 *      scales (a tight core blur + a wide soft halo, mixed): the horizontal
 *      pass WRAPS at the u seam and widens its sigma by 1/sin(polar angle)
 *      per row - a texel row near a pole spans far less sky per texel, so a
 *      fixed texel-space kernel would under-blur there and leave lone polar
 *      stars as hot spikes (the first bake did exactly that: the "peak" was
 *      a single star at dec +89, 119 deg from the galactic center). The
 *      vertical pass CLAMPS at the poles (v is already linear in angle).
 *      Only AFTER the blur is each texel divided by its solid angle
 *      (proportional to sin(polar angle)) to turn flux-per-texel into
 *      radiance - the order matters: normalizing before the blur re-creates
 *      the pole spikes the adaptive kernel exists to prevent.
 *   5. ADD the analytic UNRESOLVED-BULGE term: a warm (K-giant colored)
 *      gaussian in galactic coordinates centered on the galactic center.
 *      WHY a synthetic term in a real-data bake: ATHYG is magnitude-limited
 *      (~mag 11-12), so it contains almost none of the BILLIONS of distant
 *      bulge stars whose collective unresolved light is what makes the real
 *      Milky Way's core bright and warm in every photo - probes of the pure
 *      ATHYG bake showed the integrated-light maximum at the Carina arm
 *      tangent (the densest patch a mag-limited optical catalog can see),
 *      with the galactic-center region barely above the band average. The
 *      bulge term supplies that missing unresolved light, scaled RELATIVE to
 *      the baked data (x BULGE_GAIN the 99th-percentile band radiance).
 *      RUNG 3 NOTE: when the glow is re-baked from Gaia billion-star data,
 *      the bulge light will be IN the data - set BULGE_GAIN to 0 (or delete
 *      the term); everything else in this pipeline stays.
 *   6. Tone map: SUBTRACT the isotropic baseline, then gamma + asinh. The
 *      baseline (the FLOOR_PCT percentile of nonzero radiance) is the
 *      roughly-uniform glow of NEARBY stars - a mag-limited catalog is
 *      dominated by local stars spread almost evenly across the whole
 *      sphere, and probes showed the median sky texel at ~half the
 *      galactic-center radiance: mapped directly, the whole sky would be
 *      mid-gray. The band/bulge is the EXCESS above that baseline; empty
 *      and average sky maps to (near) black - the "must not wash out the
 *      black of space" requirement. The excess is tone mapped on luminance
 *      (rgb rescaled proportionally to preserve the warm-core hue): a mild
 *      TONE_GAMMA expansion suppresses residual off-band mottle, an asinh
 *      shoulder compresses the top so the core does not clip, and the
 *      output is normalized so the peak lands at HEADROOM (0.85), leaving
 *      room for the in-game intensity slider (0..2) to push brighter
 *      without instantly clipping.
 *   7. Write data/galaxy_glow.png (8-bit RGB, zlib via node's built-in) -
 *      no npm dependencies.
 *   8. SELF-VERIFY: after an EXTRA wide smoothing pass (~4 deg), the
 *      brightest texel must sit within a few degrees of the real galactic
 *      center (RA 266.405, Dec -28.936; Sagittarius A*). If the peak lands
 *      anywhere else the bake is wrong (bad mapping, bad cap, or corrupt
 *      input) and the script fails. Why the extra smoothing: at native texel
 *      resolution a compact open cluster genuinely has higher surface
 *      brightness than the core (the real Pleiades run ~19.4 mag/arcsec^2 vs
 *      the Sagittarius cloud's ~20.5 - an early bake's raw peak landed dead
 *      on the Pleiades, which is astrophysically CORRECT); the core's
 *      dominance is a broad-scale property, so the alignment check evaluates
 *      it at a broad scale.
 *
 *   Set GLOW_PROBE=1 to also print the smoothed-peak location at several
 *   scales (the tuning diagnostics used to calibrate the constants above).
 */

'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const ROOT = path.resolve(__dirname, '..');
const ATHYG_PATH = path.join(ROOT, 'data', 'stars-athyg.bin');
const HYG_PATH = path.join(ROOT, 'data', 'stars.bin');
const OUT_PATH = path.join(ROOT, 'data', 'galaxy_glow.png');

// Mutable on purpose (v0.807.x): the ATHYG star-integration path stays at
// 2048x1024; the Gaia census path raises to 4096x2048 when it detects a
// level-9 (nside 512) input, because that is the data's own resolving power
// (0.11 deg cells ~= one texel at 4096) - the shared blur/PNG helpers read
// these module dims.
let W = 2048;
let H = 1024;
const MAGIC = 'HOSSTAR1';
const HEADER = 16;
const RECORD = 15;

// Per-star flux cap (see header step 2). Mag 4.0 -> flux ~0.025. Chosen so
// bright RESOLVED stars (drawn as points by the star pipeline) leave only a
// subtle baked halo; compact clusters (Pleiades, Orion) still glow but do
// not out-shine the galactic core after tone mapping.
const CAP_MAG = 4.0;
// Blur scales in TEXELS (2048 px = 360 deg, so 1 texel ~ 0.176 deg).
// The core pass must be wide enough to MELT individual stars into haze: at
// sigma 2.5 a single capped mag-5 star peaked at ~2x the isotropic baseline
// and the whole sky read as confetti speckle instead of glow (first
// committed-quality bake); at sigma 6 (~1 deg) the same star lands BELOW
// the baseline and only collective structure survives, which is exactly
// what a glow layer is. The wide pass adds the soft halo around the band.
const SIGMA_CORE = 6.0;
const SIGMA_HALO = 18.0;
// Halo share cut 0.40 -> 0.22 (v0.802.2): the wide pass is what read as
// "smoke clouds" filling the sky; tighter structure keeps space black.
const HALO_MIX = 0.22; // out = (1-mix)*core + mix*halo
// Unresolved-bulge term (pipeline step 5). Gaussian in galactic coordinates:
// sigma 14 deg along the plane, 7 deg across it (the bright oval in real
// panoramas is roughly 30 x 15 deg). Peak radiance = BULGE_GAIN x the 99th
// percentile of the baked nonzero radiance (data-relative so re-bakes with a
// different catalog/cap keep the same core-to-band contrast). Color: B-V
// 1.1, the K-giant gold that dominates the real bulge's visible light.
const BULGE_SIGMA_L_DEG = 14;
const BULGE_SIGMA_B_DEG = 7;
// 3.0 -> 2.0 (v0.802.2): the analytic bulge dominated the frame at 3x.
const BULGE_GAIN = 2.0;
const BULGE_CI = 1.1;
// Tone map (pipeline step 6): baseline = this percentile of nonzero
// radiance is subtracted first (the isotropic nearby-star glow); the
// normalized excess is then raised to TONE_GAMMA (mild expansion: pushes
// weak off-band mottle - smoothed Poisson noise of the foreground - toward
// black while barely touching the band and core); the knee peak/KNEE_DIV is
// where asinh compression starts biting. Tuned on the diagnostics line:
// target roughly band spine ~40-55% gray, mid-band ~15-30%, off-band
// mottle under ~10%, core at HEADROOM. (The first tuning pass used floor
// p40 / knee peak/50 / no gamma and the whole sky came out as 30%-gray
// clouds - as bright as the band itself.)
// Retuned v0.802.2 after the operator's live report ("way too bright - I
// don't feel like I'm off world"; even 0.1 intensity read bright, which was
// mostly the shader's missing linearization, fixed in the same release, but
// the bake itself also carried too much mid-tone energy). New targets on the
// diagnostics line: band spine ~20-30% gray, mid-band ~8-15%, off-band under
// ~4% (truly black sky between the arms), core at HEADROOM. The Milky Way
// from a real dark site is a DELICATE veil - the reference photo is a long
// exposure; the game should feel like the sky, not the photo's exposure.
const FLOOR_PCT = 0.70;
const TONE_GAMMA = 1.6;
const KNEE_DIV = 12;
// Peak output level at intensity 1.0 - headroom baked into the PNG so the
// core does not clip and the 0..2 intensity slider has room to push.
const HEADROOM = 0.6;
// Self-verify tolerance (degrees) between the smoothed peak and Sgr A*.
const PEAK_TOL_DEG = 10;

/** B-V color index -> RGB. EXACT port of src/renderer/stars.rs::ci_to_rgb
 *  so the glow's hue matches the point-star colors it sits behind. */
function ciToRgb(ci) {
  ci = Math.min(2.0, Math.max(-0.4, ci));
  let r, g, b;
  if (ci < 0.0) {
    r = 0.6 + ci * 0.5; g = 0.7 + ci * 0.25; b = 1.0;          // O/B blue-white
  } else if (ci < 0.4) {
    r = 0.6 + ci * 1.0; g = 0.7 + ci * 0.75; b = 1.0 - ci * 0.5; // A/F white
  } else if (ci < 0.8) {
    const t = (ci - 0.4) / 0.4;
    r = 1.0; g = 1.0 - t * 0.15; b = 0.8 - t * 0.3;             // G yellow
  } else if (ci < 1.4) {
    const t = (ci - 0.8) / 0.6;
    r = 1.0; g = 0.85 - t * 0.35; b = 0.5 - t * 0.3;            // K orange
  } else {
    const t = (ci - 1.4) / 0.6;
    r = 1.0 - t * 0.2; g = 0.5 - t * 0.2; b = 0.2 - t * 0.1;    // M red
  }
  const c01 = (x) => Math.min(1, Math.max(0, x));
  return [c01(r), c01(g), c01(b)];
}

/** ra/dec (degrees) -> unit equatorial J2000 Cartesian (HYG convention). */
function dirFromRaDec(raDeg, decDeg) {
  const ra = (raDeg * Math.PI) / 180;
  const dec = (decDeg * Math.PI) / 180;
  return [Math.cos(dec) * Math.cos(ra), Math.cos(dec) * Math.sin(ra), Math.sin(dec)];
}

/** THE mapping contract (see header). Returns fractional u,v in [0,1]. */
function dirToUv(x, y, z) {
  const u = Math.atan2(y, x) / (2 * Math.PI) + 0.5;
  const v = Math.acos(Math.min(1, Math.max(-1, z))) / Math.PI;
  return [u, v];
}

/** Inverse of dirToUv, for texel-center -> direction in the self-verify. */
function uvToDir(u, v) {
  const ra = (u - 0.5) * 2 * Math.PI;
  const theta = v * Math.PI; // polar angle from +z (north celestial pole)
  const s = Math.sin(theta);
  return [s * Math.cos(ra), s * Math.sin(ra), Math.cos(theta)];
}

/** Build a normalized 1D gaussian kernel for a given sigma. */
function makeKernel(sigma) {
  const radius = Math.max(1, Math.ceil(sigma * 3));
  const kernel = new Float64Array(radius * 2 + 1);
  let ksum = 0;
  for (let i = -radius; i <= radius; i++) {
    const wgt = Math.exp(-(i * i) / (2 * sigma * sigma));
    kernel[i + radius] = wgt;
    ksum += wgt;
  }
  for (let i = 0; i < kernel.length; i++) kernel[i] /= ksum;
  return { kernel, radius };
}

// Horizontal sigma threshold in texels: when 1/sin pushes the row's sigma
// past this, the gaussian is wider than the row and the correct limit is a
// UNIFORM average of the whole row (a pole-adjacent row is only a tiny
// circle of sky - a lone star there, e.g. Polaris at dec +89, must smear
// around the full circle or the later 1/sin(theta) radiance division turns
// it into the brightest "glow" in the sky, 119 deg from the galactic
// center, which is exactly how the first two bakes failed).
const UNIFORM_H_SIGMA = W / 6;

/** ANGULARLY-UNIFORM separable gaussian blur of a W*H flux field (see
 *  pipeline step 4 in the header). Horizontal pass WRAPS at the u seam and
 *  widens sigma by 1/sin(polar angle) per row so the kernel covers the same
 *  patch of SKY at every latitude; vertical pass CLAMPS at the poles.
 *  Per-row kernels are cached by their rounded sigma (rows at similar
 *  latitude share kernels). */
function gaussianBlur(src, sigma) {
  const tmp = new Float64Array(W * H);
  const kernelCache = new Map();
  for (let yy = 0; yy < H; yy++) {
    const theta = ((yy + 0.5) / H) * Math.PI;
    const rowSigma = sigma / Math.max(1e-6, Math.sin(theta));
    const row = yy * W;
    if (rowSigma >= UNIFORM_H_SIGMA) {
      // Pole-adjacent row: the kernel would span the row anyway - take the
      // exact limit, a uniform average (conserves the row's flux).
      let mean = 0;
      for (let xx = 0; xx < W; xx++) mean += src[row + xx];
      mean /= W;
      for (let xx = 0; xx < W; xx++) tmp[row + xx] = mean;
      continue;
    }
    const key = Math.max(1, Math.round(rowSigma));
    let k = kernelCache.get(key);
    if (!k) {
      k = makeKernel(key);
      kernelCache.set(key, k);
    }
    const { kernel, radius } = k;
    for (let xx = 0; xx < W; xx++) {
      let acc = 0;
      for (let i = -radius; i <= radius; i++) {
        // Wrap at the seam; the offset can leave [0,W) by more than one W
        // for wide kernels, so use a floored modulo instead of "+ W".
        let sx = (xx + i) % W;
        if (sx < 0) sx += W;
        acc += src[row + sx] * kernel[i + radius];
      }
      tmp[row + xx] = acc;
    }
  }
  // Vertical, clamp at poles (v is linear in polar angle: no adaptation).
  const { kernel, radius } = makeKernel(sigma);
  const dst = new Float64Array(W * H);
  for (let yy = 0; yy < H; yy++) {
    for (let xx = 0; xx < W; xx++) {
      let acc = 0;
      for (let i = -radius; i <= radius; i++) {
        const sy = Math.min(H - 1, Math.max(0, yy + i));
        acc += tmp[sy * W + xx] * kernel[i + radius];
      }
      dst[yy * W + xx] = acc;
    }
  }
  return dst;
}

// ── Minimal dependency-free PNG writer (8-bit RGB, one IDAT) ──────────────
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function pngChunk(type, data) {
  const out = Buffer.alloc(8 + data.length + 4);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, 'ascii');
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}
function writePng(rgb /* Uint8Array W*H*3 */) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(W, 0);
  ihdr.writeUInt32BE(H, 4);
  ihdr[8] = 8;  // bit depth
  ihdr[9] = 2;  // color type: truecolor RGB
  ihdr[10] = 0; // compression
  ihdr[11] = 0; // filter
  ihdr[12] = 0; // interlace
  // Scanlines: filter byte 0 (None) + raw RGB. Level-9 deflate does well on
  // the mostly-dark, smooth glow data.
  const raw = Buffer.alloc(H * (1 + W * 3));
  for (let yy = 0; yy < H; yy++) {
    const o = yy * (1 + W * 3);
    raw[o] = 0;
    Buffer.from(rgb.buffer, yy * W * 3, W * 3).copy(raw, o + 1);
  }
  const idat = zlib.deflateSync(raw, { level: 9 });
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', idat),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

// ── GAIA DENSITY MODE (star ladder rung 3a, v0.804) ─────────────────────────
// `node scripts/build-galaxy-glow.js --gaia <color_hpx8.csv>` bakes the glow
// from the REAL Gaia DR3 census instead of integrating catalog stars: one CSV
// row per HEALPix level-8 cell (786,432 equal-area cells, ~0.229 deg across)
// carrying the star COUNT and the summed G/BP/RP fluxes of ALL 1.81 billion
// sources (aggregated server-side by ESA's TAP service; the query is in the
// journal + docs). Why counts, not flux: interstellar DUST blocks the faint
// distant stars that dominate the count, so the count map carries the dark
// rifts (the Great Rift!) that a magnitude-limited catalog bake cannot see -
// this is the "make it look like the real photo" step. Color: per-cell mean
// BP-RP from the flux sums (reddened cells at dust edges come out brown for
// free). HEALPix cells are equal-area BY CONSTRUCTION, so counts are already
// true density - no solid-angle normalization step exists in this path.
// Data credit: ESA/Gaia/DPAC (facts/measurements; the derived texture is
// project CC0). The synthetic ATHYG-mode bulge is OFF here (BULGE_GAIN
// applies only to the star-integration path): the census sees the bulge.
// Detected from the input's max cell index at load: level 8 (nside 256,
// 786,432 cells, source_id >> 43) or level 9 (nside 512, 3,145,728 cells,
// source_id >> 41 - fetched as two sub-cap halves from ESA).
let GAIA_NSIDE = 256;

/** Equatorial direction (theta = colatitude from +z, phi in [0,2pi)) to a
 *  NESTED HEALPix index. The standard HEALPix ang2pix_nest algorithm
 *  (Gorski et al. 2005), verified below against unmistakable sky anchors
 *  (galactic center, LMC, SMC - enormous density spikes; a convention slip
 *  would miss them by whole faces). */
function ang2pixNest(nside, theta, phi) {
  const z = Math.cos(theta);
  const za = Math.abs(z);
  const tt = ((phi % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI) / (Math.PI / 2); // [0,4)
  let face, ix, iy;
  if (za <= 2 / 3) {
    // Equatorial region.
    const temp1 = nside * (0.5 + tt);
    const temp2 = nside * (z * 0.75);
    const jp = Math.floor(temp1 - temp2); // ascending edge line index
    const jm = Math.floor(temp1 + temp2); // descending edge line index
    const ifp = Math.floor(jp / nside);   // in {0..4}
    const ifm = Math.floor(jm / nside);
    if (ifp === ifm) face = (ifp & 3) + 4;
    else if (ifp < ifm) face = ifp & 3;
    else face = (ifm & 3) + 8;
    ix = jm & (nside - 1);
    iy = nside - (jp & (nside - 1)) - 1;
  } else {
    // Polar caps.
    const ntt = Math.min(3, Math.floor(tt));
    const tp = tt - ntt;
    const tmp = nside * Math.sqrt(3 * (1 - za));
    let jp = Math.floor(tp * tmp);
    let jm = Math.floor((1 - tp) * tmp);
    jp = Math.min(jp, nside - 1);
    jm = Math.min(jm, nside - 1);
    if (z >= 0) {
      face = ntt;
      ix = nside - jm - 1;
      iy = nside - jp - 1;
    } else {
      face = ntt + 8;
      ix = jp;
      iy = jm;
    }
  }
  // Bit-interleave ix (even bits) and iy (odd bits); bit count = log2(nside)
  // (8 at nside 256, 9 at nside 512 - hardcoding 8 truncated level-9 indices).
  const bits = Math.log2(nside);
  let pix = 0;
  for (let b = 0; b < bits; b++) {
    pix |= ((ix >> b) & 1) << (2 * b);
    pix |= ((iy >> b) & 1) << (2 * b + 1);
  }
  return face * nside * nside + pix;
}

/** Mean per-cell BP-RP color index -> the star pipeline's B-V-like domain.
 *  BP-RP for normal stars spans ~-0.5..4 (reddening pushes redder); an
 *  empirical B-V ~ 0.85*(BP-RP) - 0.1 linearization is plenty for a GLOW
 *  hue - ciToRgb clamps to [-0.4, 2.0] anyway. */
function bpRpToCi(bpf, rpf) {
  if (!(bpf > 0) || !(rpf > 0)) return 0.65; // no color data: neutral warm
  const bpRp = -2.5 * Math.log10(bpf / rpf);
  return 0.85 * bpRp - 0.1;
}

function bakeGaia(csvPaths) {
  const t0 = Date.now();
  // Accept several CSVs (a level-9 aggregation exceeds ESA's 3M-row cap and
  // arrives as two source_id halves). First pass: find the max cell index to
  // DETECT the healpix level, then size everything from it.
  const allLines = [];
  let maxIdx = 0;
  for (const p of csvPaths) {
    const lines = fs.readFileSync(p, 'utf8').trim().split('\n');
    for (let i = 1; i < lines.length; i++) {
      allLines.push(lines[i]);
      const h = Number(lines[i].slice(0, lines[i].indexOf(',')));
      if (h > maxIdx) maxIdx = h;
    }
  }
  // Ladder: level 8 (nside 256) -> 2048, level 9 -> 4096, level 10 (nside
  // 1024, 12.6M cells, 0.057 deg) -> 8192x4096 - one texel ~0.044 deg,
  // parity with a 1440p screen pixel (~0.035 deg at game FOV), which is the
  // point of level 10: the glow stops being softer than the display.
  GAIA_NSIDE = maxIdx >= 12 * 512 * 512 ? 1024 : maxIdx >= 12 * 256 * 256 ? 512 : 256;
  if (GAIA_NSIDE === 512) {
    W = 4096;
    H = 2048;
  } else if (GAIA_NSIDE === 1024) {
    W = 8192;
    H = 4096;
  }
  console.log(`gaia: nside ${GAIA_NSIDE} detected -> ${W}x${H} texture`);
  const ncell = 12 * GAIA_NSIDE * GAIA_NSIDE;
  const cellN = new Float64Array(ncell);
  const cellBp = new Float64Array(ncell);
  const cellRp = new Float64Array(ncell);
  for (const line of allLines) {
    const f = line.split(',');
    const h = Number(f[0]);
    if (!(h >= 0 && h < ncell)) continue;
    cellN[h] = Number(f[1]);
    cellBp[h] = Number(f[3]) || 0;
    cellRp[h] = Number(f[4]) || 0;
  }
  console.log(`gaia: ${allLines.length} cells loaded`);

  // Anchors: these sightlines are enormous density spikes in the real
  // census. If ang2pixNest had a convention slip they would land on
  // ordinary cells and this bake would be beautifully wrong forever.
  const median = (() => {
    const s = Array.from(cellN.filter((v) => v > 0)).sort((a, b) => a - b);
    return s[Math.floor(s.length / 2)];
  })();
  for (const [name, ra, dec, minRatio] of [
    ['galactic center', 266.417, -29.008, 5],
    ['LMC', 80.894, -69.756, 10],
    ['SMC', 13.187, -72.829, 5],
  ]) {
    const [dx, dy, dz] = dirFromRaDec(ra, dec);
    const h = ang2pixNest(GAIA_NSIDE, Math.acos(dz), Math.atan2(dy, dx));
    const ratio = cellN[h] / median;
    if (!(ratio >= minRatio)) {
      throw new Error(`gaia anchor FAILED: ${name} cell density ${cellN[h]} is only ${ratio.toFixed(1)}x the median (need >= ${minRatio}x) - healpix mapping is wrong`);
    }
    console.log(`anchor ${name}: ${ratio.toFixed(0)}x median density`);
  }

  // Per-texel lookup: log-compressed density as luminance, per-cell BP-RP
  // color as hue. Percentile normalization keeps the delicate-veil targets
  // from the v0.803 calibration: floor at p35 (high-latitude sky = black),
  // full scale at p99.9 (the core, LMC).
  const accR = new Float64Array(W * H);
  const accG = new Float64Array(W * H);
  const accB = new Float64Array(W * H);
  const logN = new Float64Array(W * H);
  for (let py = 0; py < H; py++) {
    const v = (py + 0.5) / H;
    for (let px = 0; px < W; px++) {
      const u = (px + 0.5) / W;
      const [dx, dy, dz] = uvToDir(u, v);
      const h = ang2pixNest(GAIA_NSIDE, Math.acos(Math.min(1, Math.max(-1, dz))), Math.atan2(dy, dx));
      const idx = py * W + px;
      logN[idx] = Math.log10(1 + cellN[h]);
      const [r, g, b] = ciToRgb(bpRpToCi(cellBp[h], cellRp[h]));
      accR[idx] = r; accG[idx] = g; accB[idx] = b;
    }
  }
  // Luminance normalization percentiles.
  const sorted = Float64Array.from(logN).sort();
  const pct = (p) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))];
  const lo = pct(0.35);
  const hi = pct(0.999);
  console.log(`gaia luminance: p35 ${lo.toFixed(2)} -> p99.9 ${hi.toFixed(2)} (log10 counts)`);
  const lum = new Float64Array(W * H);
  for (let i = 0; i < W * H; i++) {
    const t = Math.min(1, Math.max(0, (logN[i] - lo) / (hi - lo)));
    // Gamma 1.5: same delicate-veil intent as the star bake's tone map -
    // push faint off-band residue toward black, keep the band + rifts.
    lum[i] = Math.pow(t, 1.5) * HEADROOM;
  }
  // Light blur to melt the healpix cell edges (cells ~1.3 texels at 2048).
  const lumBlurred = gaussianBlur(lum, 1.5);
  const rgb = new Uint8Array(W * H * 3);
  for (let i = 0; i < W * H; i++) {
    rgb[i * 3] = Math.round(255 * Math.min(1, lumBlurred[i] * accR[i]));
    rgb[i * 3 + 1] = Math.round(255 * Math.min(1, lumBlurred[i] * accG[i]));
    rgb[i * 3 + 2] = Math.round(255 * Math.min(1, lumBlurred[i] * accB[i]));
  }
  // THE BUG THAT SHIPPED A PHANTOM (2026-07-11): writePng RETURNS the buffer;
  // this call used to discard it and then stat the STALE file - the level-8
  // census bake never reached disk, its "gaia bake: ... KB" log line was
  // reading the old ATHYG texture, and every anchor check passed because they
  // test the in-memory grid. Write it, then READ BACK the IHDR dims as a
  // tripwire: a bake that does not change the file must never lie again.
  fs.writeFileSync(OUT_PATH, writePng(rgb));
  const back = fs.readFileSync(OUT_PATH);
  const bw = back.readUInt32BE(16), bh = back.readUInt32BE(20);
  if (bw !== W || bh !== H) {
    throw new Error(`gaia bake: written PNG is ${bw}x${bh}, expected ${W}x${H} - write failed`);
  }
  const kb = (back.length / 1024).toFixed(0);
  console.log(`gaia bake: ${OUT_PATH} ${bw}x${bh} (${kb} KB) from the 1.81B-source census in ${Date.now() - t0} ms`);
}

function main() {
  const gaiaIdx = process.argv.indexOf('--gaia');
  if (gaiaIdx >= 0) {
    // Everything after --gaia is an input CSV (level-9 pulls arrive split).
    const csvs = process.argv.slice(gaiaIdx + 1).filter((p) => fs.existsSync(p));
    if (csvs.length === 0) {
      console.error('usage: node scripts/build-galaxy-glow.js --gaia <hpx.csv> [...more parts]');
      process.exit(1);
    }
    bakeGaia(csvs);
    return;
  }
  const t0 = Date.now();
  const srcPath = fs.existsSync(ATHYG_PATH) ? ATHYG_PATH : HYG_PATH;
  const bin = fs.readFileSync(srcPath);
  if (bin.subarray(0, 8).toString('ascii') !== MAGIC) {
    console.error(`${srcPath}: bad HOSSTAR1 magic`);
    process.exit(1);
  }
  const starCount = bin.readUInt32LE(8);
  if (bin.length < HEADER + starCount * RECORD) {
    console.error(`${srcPath}: truncated (${starCount} stars claimed)`);
    process.exit(1);
  }
  console.log(`input: ${path.basename(srcPath)} (${starCount} stars)`);
  if (srcPath === HYG_PATH) {
    console.warn(
      'WARNING: stars-athyg.bin not found; baking from the ~120k HYG catalog. ' +
      'The band will be thin - fetch the extended catalog for the real bake.'
    );
  }

  // ── 1+2+3: accumulate flux-weighted RGB radiance ──
  const accR = new Float64Array(W * H);
  const accG = new Float64Array(W * H);
  const accB = new Float64Array(W * H);
  const fluxCap = Math.pow(10, -CAP_MAG / 2.5);
  for (let i = 0; i < starCount; i++) {
    const o = HEADER + i * RECORD;
    const dx = bin.readFloatLE(o);
    const dy = bin.readFloatLE(o + 4);
    const dz = bin.readFloatLE(o + 8);
    const mag = bin.readUInt16LE(o + 12) / 1024 - 2.0;
    const ci = (bin.readUInt8(o + 14) / 255) * 2.4 - 0.4;
    const flux = Math.min(fluxCap, Math.pow(10, -mag / 2.5));
    const [u, v] = dirToUv(dx, dy, dz);
    // u=1.0 exactly (atan2 returning +pi) wraps to texel 0.
    const px = Math.min(W - 1, Math.floor(u * W)) % W;
    const py = Math.min(H - 1, Math.floor(v * H));
    const [r, g, b] = ciToRgb(ci);
    const idx = py * W + px;
    accR[idx] += flux * r;
    accG[idx] += flux * g;
    accB[idx] += flux * b;
  }
  console.log(`accumulate: ${Date.now() - t0} ms`);

  // ── 4: two-scale angularly-uniform blur, THEN solid-angle normalization ──
  // The blur runs on FLUX (per texel); dividing by texel solid angle
  // (proportional to sin(theta)) afterwards yields radiance. Doing it in the
  // other order re-creates the lone-polar-star spikes (see header).
  const tBlur = Date.now();
  // Normalization floor: within one blur-width of a pole the blur has
  // smeared flux across rows whose true solid angles differ enormously
  // (row 0 covers ~650x less sky than row 40), so dividing each row by its
  // OWN sin(theta) re-amplifies leaked flux into a false polar hotspot
  // (Polaris kept winning the peak check this way). Radiance structure finer
  // than the blur kernel is unresolvable anyway, so floor the divisor at
  // sin(3 halo sigmas from the pole). Slightly dims the poles; the band
  // never goes within ~60 deg of a celestial pole, so it is unaffected.
  const sinFloor = Math.sin(((3 * SIGMA_HALO) / H) * Math.PI);
  const blur = (ch) => {
    const core = gaussianBlur(ch, SIGMA_CORE);
    const halo = gaussianBlur(core, SIGMA_HALO);
    const out = new Float64Array(W * H);
    for (let yy = 0; yy < H; yy++) {
      const s = 1 / Math.max(sinFloor, Math.sin(((yy + 0.5) / H) * Math.PI));
      for (let xx = 0; xx < W; xx++) {
        const i = yy * W + xx;
        out[i] = ((1 - HALO_MIX) * core[i] + HALO_MIX * halo[i]) * s;
      }
    }
    return out;
  };
  const bR = blur(accR);
  const bG = blur(accG);
  const bB = blur(accB);
  console.log(`blur: ${Date.now() - tBlur} ms`);

  // ── Percentiles of the pure-data radiance (bulge amplitude + baseline) ──
  const gc = dirFromRaDec(266.405, -28.936); // Sagittarius A*
  const [gu, gv] = dirToUv(gc[0], gc[1], gc[2]);
  const nz = [];
  for (let i = 0; i < W * H; i++) {
    const l = Math.max(bR[i], bG[i], bB[i]);
    if (l > 0) nz.push(l);
  }
  if (nz.length === 0) {
    console.error('bake produced an all-black texture - input corrupt?');
    process.exit(1);
  }
  nz.sort((a, b) => a - b);
  const pct = (p) => nz[Math.min(nz.length - 1, Math.floor(p * nz.length))];
  {
    // Diagnostics so a failed or barely-passing verify is debuggable from
    // the log alone: GC-texel radiance + the tone-map tuning inputs.
    const gi =
      Math.min(H - 1, Math.floor(gv * H)) * W + (Math.min(W - 1, Math.floor(gu * W)) % W);
    const gl = Math.max(bR[gi], bG[gi], bB[gi]);
    console.log(
      `radiance: GC texel ${gl.toExponential(3)}; nonzero L ` +
      `p10 ${pct(0.1).toExponential(2)} p50 ${pct(0.5).toExponential(2)} ` +
      `p90 ${pct(0.9).toExponential(2)} p99 ${pct(0.99).toExponential(2)}`
    );
  }

  // ── 5: the analytic unresolved-bulge term (see header - rung 3 zeroes it) ──
  {
    const ngp = dirFromRaDec(192.8595, 27.1283); // North Galactic Pole (J2000)
    // In-plane basis at the GC: e1 points at the GC, e2 = ngp x e1, so
    // (d.e1, d.e2, d.ngp) give galactic longitude offset + latitude directly.
    const e2 = [
      ngp[1] * gc[2] - ngp[2] * gc[1],
      ngp[2] * gc[0] - ngp[0] * gc[2],
      ngp[0] * gc[1] - ngp[1] * gc[0],
    ];
    const amp = BULGE_GAIN * pct(0.99);
    const [br2, bg2, bb2] = ciToRgb(BULGE_CI);
    const sigL = (BULGE_SIGMA_L_DEG * Math.PI) / 180;
    const sigB = (BULGE_SIGMA_B_DEG * Math.PI) / 180;
    for (let yy = 0; yy < H; yy++) {
      for (let xx = 0; xx < W; xx++) {
        const d = uvToDir((xx + 0.5) / W, (yy + 0.5) / H);
        const x1 = d[0] * gc[0] + d[1] * gc[1] + d[2] * gc[2];
        const x2 = d[0] * e2[0] + d[1] * e2[1] + d[2] * e2[2];
        const zb = d[0] * ngp[0] + d[1] * ngp[1] + d[2] * ngp[2];
        // Galactic latitude + longitude offset from the GC, in radians.
        const bLat = Math.asin(Math.min(1, Math.max(-1, zb)));
        const lOff = Math.atan2(x2, x1);
        const prof = Math.exp(
          -0.5 * ((lOff / sigL) * (lOff / sigL) + (bLat / sigB) * (bLat / sigB))
        );
        if (prof < 1e-4) continue;
        const i = yy * W + xx;
        bR[i] += amp * prof * br2;
        bG[i] += amp * prof * bg2;
        bB[i] += amp * prof * bb2;
      }
    }
    console.log(`bulge: amp ${amp.toExponential(3)} (${BULGE_GAIN} x p99), B-V ${BULGE_CI}`);
  }

  // ── 6: baseline-subtracted asinh tone map on luminance, hue-preserving ──
  const floor = pct(FLOOR_PCT);
  let peak = 0;
  for (let i = 0; i < W * H; i++) {
    const e = Math.max(bR[i], bG[i], bB[i]) - floor;
    if (e > peak) peak = e;
  }
  const norm = Math.asinh(KNEE_DIV);
  const rgb = new Uint8Array(W * H * 3);
  for (let i = 0; i < W * H; i++) {
    const l = Math.max(bR[i], bG[i], bB[i]);
    const e = l - floor;
    if (e <= 0) continue; // at or below the isotropic baseline: stays black
    // Normalized excess -> mild expansion gamma -> asinh shoulder.
    const t = Math.pow(e / peak, TONE_GAMMA);
    const scale = ((Math.asinh(t * KNEE_DIV) / norm) * HEADROOM) / l;
    rgb[i * 3] = Math.min(255, Math.round(bR[i] * scale * 255));
    rgb[i * 3 + 1] = Math.min(255, Math.round(bG[i] * scale * 255));
    rgb[i * 3 + 2] = Math.min(255, Math.round(bB[i] * scale * 255));
  }

  // ── 8: self-verify - the BROAD-scale peak sits at the galactic center ──
  // Wide smoothing (~4 deg) collapses compact clusters (their flux is fixed,
  // their area grows) while the broad band barely changes - at THIS scale the
  // galactic core must be the brightest thing in the sky.
  const tVerify = Date.now();
  const lum = new Float64Array(W * H);
  for (let i = 0; i < W * H; i++) lum[i] = Math.max(bR[i], bG[i], bB[i]);
  // TUNING PROBE (temporary): report the smoothed peak at several scales.
  if (process.env.GLOW_PROBE) {
    for (const sg of [24, 48, 96]) {
      const sm = gaussianBlur(lum, sg);
      let pk = 0, pi2 = 0;
      for (let i = 0; i < W * H; i++) if (sm[i] > pk) { pk = sm[i]; pi2 = i; }
      const yy = Math.floor(pi2 / W), xx = pi2 % W;
      const d2 = uvToDir((xx + 0.5) / W, (yy + 0.5) / H);
      const dt = Math.min(1, Math.max(-1, d2[0] * gc[0] + d2[1] * gc[1] + d2[2] * gc[2]));
      const gi2 = Math.min(H - 1, Math.floor(gv * H)) * W + (Math.min(W - 1, Math.floor(gu * W)) % W);
      console.log(
        `probe sigma ${sg}: peak ${pk.toExponential(3)} at (${xx},${yy}), ` +
        `${((Math.acos(dt) * 180) / Math.PI).toFixed(1)} deg from GC; GC value ${sm[gi2].toExponential(3)}`
      );
    }
  }
  const smooth = gaussianBlur(lum, 24);
  let sPeak = 0;
  let sIdx = 0;
  for (let i = 0; i < W * H; i++) {
    if (smooth[i] > sPeak) { sPeak = smooth[i]; sIdx = i; }
  }
  const spy = Math.floor(sIdx / W);
  const spx = sIdx % W;
  const pd = uvToDir((spx + 0.5) / W, (spy + 0.5) / H);
  const dot = Math.min(1, Math.max(-1, pd[0] * gc[0] + pd[1] * gc[1] + pd[2] * gc[2]));
  const sepDeg = (Math.acos(dot) * 180) / Math.PI;
  console.log(
    `smoothed peak (${spx},${spy}) vs galactic center (${(gu * W).toFixed(1)},${(gv * H).toFixed(1)}): ` +
    `${sepDeg.toFixed(2)} deg apart (tolerance ${PEAK_TOL_DEG}, smoothing ${Date.now() - tVerify} ms)`
  );
  if (sepDeg > PEAK_TOL_DEG) {
    console.error('VERIFY FAILED: broad-scale glow does not peak at the galactic center - bad bake.');
    process.exit(1);
  }

  // ── 6: write the PNG ──
  const png = writePng(rgb);
  fs.writeFileSync(OUT_PATH, png);
  console.log(
    `galaxy_glow.png: ${W}x${H} RGB, ${(png.length / 1048576).toFixed(2)} MB ` +
    `(peak ${(HEADROOM * 100).toFixed(0)}% gray, knee peak/${KNEE_DIV}) in ${Date.now() - t0} ms`
  );
}

main();
