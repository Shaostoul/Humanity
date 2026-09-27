// Profile cloud GRAIN across the terminator, band by band, with a measure a
// blur cannot win.
//
// Written 2026-09-22, when a band profile showed the defect is 73x worse at the
// dusk line than at noon (0.55 percent in full daylight, 40.43 at the dusk line,
// about 4 on the night side where nothing is directly lit). The operator had
// already said exactly that with no instrument: the clouds are "glistening ...
// most noticeable at the dusk line".
//
// WHY THIS TOOL EXISTS RATHER THAN A SINGLE NUMBER. Every grain measurement
// before it was taken at approach-2000km-high, which is NOON, and noon is the
// weakest regime by a factor of seventy. A candidate fix that halves the noise
// there may do nothing where the defect lives, so grain candidates are ranked
// with this, not with a whole-frame average.
//
// ---------------------------------------------------------------------------
// THE FLAW THE FIRST VERSION HAD (found 2026-09-22, fixed 2026-09-27)
// ---------------------------------------------------------------------------
// The first version reported one number per band: the mean absolute deviation
// of each pixel from the mean of its four neighbours, over the band mean. That
// column is still printed as "speckle (old)" so every number quoted before today
// can be compared, but it is NOT the grain any more, because a blur wins it.
// Replacing each pixel with its neighbourhood mean (which is exactly what the
// cloud resolve's spatial filter does when forced to full, `cur_s = mu`) drives
// a deviation-from-the-neighbourhood-mean to zero by construction. On the real
// 2026-09-22 captures a plain 3x3 box blur of the baseline, done in this
// script after the fact, scores 40.4 -> about 6 percent: a better "grain" than
// the animated-jitter fix that actually works. A grain number a blur can win
// is not a grain number (PRIORITIES 2a-ii).
//
// ---------------------------------------------------------------------------
// WHAT IT MEASURES NOW, AND THE REASONING FOR EACH PART
// ---------------------------------------------------------------------------
// 1. NOISE AND REAL DETAIL ARE MEASURED IN DIFFERENT PLACES.
//    Every capture of one vantage is the same camera, so where the genuine
//    cloud edges are is geometry, not something an arm gets to change. That
//    geometry is taken ONCE, from a heavily smoothed (sigma 4 px) copy of the
//    REFERENCE capture (the first file), and applied to every file:
//      edge pixels      lit, and in the steepest tenth of the band. This is
//                       where the real cloud detail is.
//      interior pixels  lit, in the flattest half of the band (smoothed
//                       gradient at or below the band median), AND at least
//                       GAP = 2 * sqrt2 * b px (12 px by default) from every
//                       edge pixel, so an outline's own band response (still
//                       strong several px away) is not counted as noise.
//                       Whatever varies here is noise, plus a little real
//                       texture. Without the gap, a synthetic frame with
//                       perfectly flat cloud interiors and NO noise read 0.8%
//                       of "noise"; with it, the daylight band of the real
//                       baseline reads 0.012%.
//    Using one mask means two arms are always judged on the same pixels.
//    (--own-mask gives each file its own, for single captures of other scenes.)
//
// 2. ONE SCALE, ~15 PX, IS THE HEADLINE, AND THE REASON IS WHERE THE DETAIL IS.
//    Each capture is split into spatial scales with a SQUARED difference of
//    Gaussians. The headline band is sigma 2.83 to 4 px, which peaks at a
//    period of about 15 px. Three things point at that scale:
//      - It is where the real cloud detail lives at this range. Measured on
//        the terminator captures, the genuine cloud edges carry NO detail
//        finer than about a 9 px period: their edge profile rises about 0.1
//        of the full step per pixel all the way out to 10 px, a soft ramp.
//        Below that scale every bit of energy in the frame is noise, so there
//        is nothing to compare the noise against.
//      - Frozen-jitter noise is white, so it has energy at 15 px too, and a
//        3x3 blur barely touches it there: a blur removes the pixel-scale
//        sparkle and leaves the mottle, which is what the forced-filter
//        capture looks like when zoomed.
//      - On the operator's 2560 px monitor 15 px is about 3 cycles per
//        degree at a normal viewing distance, the peak of human contrast
//        sensitivity. The pixel-scale sparkle the old metric weighed is the
//        scale the eye weighs least.
//
// 3. GRAIN = NOISE / REAL DETAIL AT THAT SCALE.
//      noise   RMS of the band response over the interior pixels, divided by
//              their mean level.
//      detail  RMS of the band response over the edge pixels, divided by their
//              mean level, with the noise removed in quadrature
//              (sqrt(edge^2 - noise^2)), because noise sits on edges too.
//      GRAIN   100 * noise / detail, in percent: how loud the noise is next
//              to the real structure at the same scale.
//    A band is only judged when its detail is at least twice its noise
//    (GRAIN <= 50%); on the night side there can be too little lit edge to
//    measure against, and the ratio would report the scarcity of edges.
//
// 4. THE SCALE LOCK: WHY A SMALL BLUR CANNOT WIN IT. A blur is a linear
//    filter. At any one spatial frequency it multiplies the noise and the real
//    detail by the SAME factor, so their ratio at that frequency cannot change.
//    Only something that uses more information than one frame holds can lower
//    it: averaging several frames of independent noise (the temporal
//    accumulator with an animated jitter), or sampling the cloud better.
//    That argument is exact only for an infinitely narrow band. A finite band
//    can tilt: the noise sits at its fine end and the cloud detail at its
//    coarse end, so a blur that bites the fine end lowers the ratio. That is
//    why the band is a SQUARED difference of Gaussians (its low-frequency tail
//    falls as the fourth power of frequency, not the square, so steep cloud
//    structure below the band cannot leak in), and why it sits at 15 px and
//    not finer. Measured on the 2026-09-22 baseline with the same controls
//    --controls runs, looking at the ratio alone (--no-sharpness-lock): at
//    ~9 px a 5x5 box already takes 18% off it and a sigma-3 Gaussian 33%; at
//    ~11 px the Gaussian still takes 20%; at ~15 px none of the 54 blurs of
//    the nine real captures lowered GRAIN by more than 3.2%.
//
// 5. THE SHARPNESS LOCK: WHY A WIDE BLUR CANNOT WIN IT EITHER. A blur as wide
//    as the band itself (sigma 3 px: its response falls from 0.75 to 0.17
//    across the band) tilts it, and whether that lowers GRAIN depends on where
//    in the band a scene keeps its detail. On the real terminator captures it
//    did not; on a synthetic scene with its detail at the coarse end it did,
//    by 14%. So a lower GRAIN only counts as BETTER when the real detail is
//    kept too: at least 90% of the reference's detail at ~9 px AND at the
//    headline scale. A frame with less GRAIN and less detail is reported as
//    "blurred". A wide blur always pays at 9 px first (sigma 3 keeps 27% of it
//    on the real baseline), which is exactly the scale a small blur cannot
//    reach and the scale lock cannot see. The two locks cover each other.
//
// 6. WHAT A BLUR DOES GET, REPORTED BUT NOT SCORED. A blur does remove the
//    pixel-scale sparkle, and at this vantage there is no real detail at that
//    scale to lose. That is real and it is printed: "speckle (old)" is the
//    pixel-scale noise, and "noise removed" compares it with the reference.
//    It is not the headline because it is the one number any filter wins.
//
// 7. WHAT IT DOES NOT CATCH. The interior noise at 15 px includes the cloud's
//    own real texture, so no arm reaches zero. Detail is measured on
//    tonemapped 8-bit pixels, so an arm that changes the noise level also
//    shifts edge contrast a few percent through the display curve (averaging
//    before the tonemap is not the same as after); the animated arm keeps
//    93-97% (both tables in PRIORITIES 2a-ii), inside the 90% lock. Where the
//    reference's detail is not well
//    above its noise (GRAIN around 40%, as bands 3-4 of the 2026-09-27
//    terminator capture read), the reference's noise-corrected detail is
//    itself noise-limited, so a DENOISING arm can read "detail kept" above
//    100% (155% at band 3 for the animated jitter). That inflates no verdict
//    (the lock is a floor), but read those bands' detail figures as rough.
//    And the whole measure is a STILL: it says
//    nothing about how the grain moves, which is what fizz is for, and nothing
//    about motion, where the temporal filter cannot converge (PRIORITIES 2b-0).
//
// Reports, per vertical band from the lit edge to the night side:
//   mean L         band brightness, so a fix that merely darkens cannot pass
//   speckle (old)  the 2026-09-22 number, pixel-scale noise, a blur wins it
//   GRAIN          noise / real detail at ~15 px, the number to rank arms by
// and, at the focus band, one row per file against the reference:
//   noise removed  at the pixel scale and at 15 px
//   detail kept    at ~9 px and ~15 px (the scales where real detail exists;
//                  a blur loses detail at 9 px first)
//   verdict        BETTER: GRAIN at least 10% lower with 90% of the detail kept
//                  at both scales. blurred: GRAIN lower, detail lost. worse:
//                  GRAIN at least 10% higher. same: anything between. 10% is
//                  about the smallest step in a noise level anyone sees.
//   fizz           when <name>-b.png sits beside a capture (the same camera a
//                  few seconds later in the same boot): mean |a - b| over the
//                  mean level. "fizz (roi)" is the 2026-09-22 definition,
//                  over the whole 0.30-0.70 x 0.25-0.75 region; "fizz (band)"
//                  is the focus band alone.
//
// PROOF THAT IT CAN FAIL: --controls blurs every given capture in memory
// (3x3 box, the forced filter's own kernel; 5x5 box; Gaussians of sigma 0.5,
// 1, 2 and 3), scores each against its source, and exits 1 if ANY blur is
// judged BETTER, or if a blur is so weak that the OLD speckle did not fall for
// it by at least 30% (a control the flawed metric does not reward proves
// nothing). Each lock is shown load-bearing by switching the other one off:
//   --no-sharpness-lock --scale 0.71,1.41   the mild blurs win at the pixel
//                                           scale: red, exit 1 (the scale lock
//                                           is what stops them)
//   --no-sharpness-lock                      on a scene whose detail sits at the
//                                           coarse end of the band, the sigma-3
//                                           Gaussian wins: red (the sharpness
//                                           lock is what stops it)
// --no-sharpness-lock is a diagnostic for exactly this and prints a warning;
// never rank arms with it. scripts/tests/terminator-grain.test.js runs all of
// it on synthetic captures inside `just rig-tests`.
//
// usage: node scripts/terminator-grain.js <reference.png> [arm.png ...]
//        [--band 5] [--bands 10] [--x0 0.30] [--x1 0.70] [--controls]
//        [--profile] [--own-mask] [--scale a,b] [--no-sharpness-lock] [--json out.json]
// The reference is the first file. Without --band the focus is the band where
// the reference's 15 px noise is loudest among the bands with enough real
// detail to judge it against (detail at least twice the noise); on
// orbit-terminator-3000km that is band 5, the dusk line.
// Just recipe: `just terminator-grain <reference.png> [arm.png ...]`.
const fs = require("fs");
const path = require("path");
const sharp = require("sharp");

const args = process.argv.slice(2);
let bands = 10, X0 = 0.30, X1 = 0.70, focusBand = null, controls = false,
  profileAll = false, ownMask = false, jsonOut = null, sharpnessLock = true;
// The headline scale: squared DoG between these two sigmas (px). Peak period
// is 2*pi / sqrt(2 ln(b^2/a^2) / (b^2 - a^2)) = 15.1 px for 2.83, 4.
let SCALE = [2.83, 4.0];
// The finer detail scale, reported (not scored): peak period ~9.2 px.
const SCALE9 = [1.41, 2.83];
const files = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === "--bands") bands = Number(args[++i]);
  else if (a === "--x0") X0 = Number(args[++i]);
  else if (a === "--x1") X1 = Number(args[++i]);
  else if (a === "--band") focusBand = Number(args[++i]);
  else if (a === "--controls") controls = true;
  else if (a === "--profile") profileAll = true;
  else if (a === "--own-mask") ownMask = true;
  // DIAGNOSTIC ONLY: switches the sharpness lock off so the controls can be
  // shown to fail. Never rank arms with it (see header, PROOF THAT IT CAN FAIL).
  else if (a === "--no-sharpness-lock") sharpnessLock = false;
  else if (a === "--json") jsonOut = args[++i];
  else if (a === "--scale") SCALE = args[++i].split(",").map(Number);
  else files.push(a);
}
if (!files.length || SCALE.length !== 2 || !(SCALE[0] < SCALE[1])) {
  console.error(
    "usage: node scripts/terminator-grain.js <reference.png> [arm.png ...]\n" +
      "       [--band 5] [--bands 10] [--controls] [--profile] [--own-mask] [--scale a,b]\n" +
      "       [--no-sharpness-lock (diagnostic: proves the controls can fail)] [--json out.json]"
  );
  process.exit(2);
}

// Tight vertical crop: a loose one counts the chat overlay and the HUD, which
// on a dark frame read as bright cloud and poison the band. Unchanged since v1.
const Y0F = 0.25, Y1F = 0.75;
const LIT = 8; // below this (8-bit luma) a pixel is unlit: nothing to be grainy
const MARGIN = 48; // px of context around the ROI so the widest filter is not edge-clamped
const MIN_INTERIOR = 400, MIN_EDGE = 150;
const BETTER = 0.9; // GRAIN at or below 0.9x the reference = better; 1.1x = worse
// A band is only judged when its real detail is at least twice its noise
// (GRAIN at or below 50%). Below that there is too little cloud edge to measure
// the noise against, and the ratio reports the scarcity of edges, not grain.
const JUDGEABLE_MAX_GRAIN = 50;
const judgeable = (s) => !!s && isFinite(s.grain) && s.grain <= JUDGEABLE_MAX_GRAIN;
// THE SHARPNESS LOCK (header, point 5). Less GRAIN only counts as BETTER when
// the real detail is kept too: at least 90% of the reference's detail at BOTH
// ~9 px and the headline scale. Same 10% step as the GRAIN threshold, used the
// other way round: a 10% loss of detail is as visible as a 10% change in noise.
// A frame with less GRAIN and less detail is "blurred", which is not credit.
const DETAIL_KEPT_MIN = 0.9;
function verdictOf(rel, kept9, kept15) {
  if (rel == null) return "-";
  if (rel <= BETTER) {
    if (!sharpnessLock) return "BETTER";
    return kept9 != null && kept15 != null && kept9 >= DETAIL_KEPT_MIN && kept15 >= DETAIL_KEPT_MIN ? "BETTER" : "blurred";
  }
  return rel >= 1 / BETTER ? "worse" : "same";
}
// A control blur must be one the OLD speckle clearly fell for (scored at least
// 30% better), or passing it proves nothing about the flaw being guarded.
const CONTROL_MIN_FOOL = 0.3;

async function loadLuma(f) {
  const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
  const W = info.width, H = info.height, C = info.channels;
  // Float64, not Float32: a grey 8 has luma 8*(0.299+0.587+0.114) = 7.9999999999 in
  // double precision and 8.0 in float32, so float32 moves pixels across the LIT cut
  // and the old speckle column stops reproducing the 2026-09-22 numbers.
  const L = new Float64Array(W * H);
  for (let i = 0, j = 0; i < W * H; i++, j += C) L[i] = data[j] * 0.299 + data[j + 1] * 0.587 + data[j + 2] * 0.114;
  return { L, W, H };
}

// Separable Gaussian with clamped edges, truncated at 3.5 sigma.
function gauss(src, W, H, s) {
  const r = Math.max(1, Math.ceil(3.5 * s)), n = 2 * r + 1, k = new Float32Array(n);
  let sum = 0;
  for (let i = -r; i <= r; i++) { k[i + r] = Math.exp(-(i * i) / (2 * s * s)); sum += k[i + r]; }
  for (let i = 0; i < n; i++) k[i] /= sum;
  return separable(src, W, H, k, r);
}
// Separable box blur of (2r+1) x (2r+1), clamped edges. The controls use it.
function boxBlur(src, W, H, r) {
  const n = 2 * r + 1, k = new Float32Array(n).fill(1 / n);
  return separable(src, W, H, k, r);
}
function separable(src, W, H, k, r) {
  const n = k.length, tmp = new Float32Array(W * H), out = new Float32Array(W * H);
  const row = new Float32Array(W + 2 * r), col = new Float32Array(H + 2 * r);
  for (let y = 0; y < H; y++) {
    const o = y * W;
    for (let x = -r; x < W + r; x++) row[x + r] = src[o + (x < 0 ? 0 : x >= W ? W - 1 : x)];
    for (let x = 0; x < W; x++) { let a = 0; for (let j = 0; j < n; j++) a += row[x + j] * k[j]; tmp[o + x] = a; }
  }
  for (let x = 0; x < W; x++) {
    for (let y = -r; y < H + r; y++) col[y + r] = tmp[(y < 0 ? 0 : y >= H ? H - 1 : y) * W + x];
    for (let y = 0; y < H; y++) { let a = 0; for (let j = 0; j < n; j++) a += col[y + j] * k[j]; out[y * W + x] = a; }
  }
  return out;
}
// Squared difference of Gaussians. In frequency (Ga - Gb)^2 = G(a*sqrt2)
// - 2 G(sqrt(a^2+b^2)) + G(b*sqrt2), so three blurs make it. Its low-frequency
// tail falls as w^4 instead of the plain DoG's w^2, which is what keeps steep
// coarse cloud structure out of the band (see header, point 4).
function bandSq(img, W, H, a, b, memo) {
  const g = (s) => {
    const key = s.toFixed(4);
    if (!memo.has(key)) memo.set(key, gauss(img, W, H, s));
    return memo.get(key);
  };
  const A = g(a * Math.SQRT2), M = g(Math.sqrt(a * a + b * b)), B = g(b * Math.SQRT2);
  const o = new Float32Array(W * H);
  for (let i = 0; i < o.length; i++) o[i] = A[i] - 2 * M[i] + B[i];
  return o;
}

// Geometry of the frame: the ROI crop with margin, and the per-band pixel boxes.
function frameGeometry(W, H) {
  const cx0 = Math.max(0, Math.floor(W * X0) - MARGIN), cx1 = Math.min(W, Math.floor(W * X1) + MARGIN);
  const cy0 = Math.max(0, Math.floor(H * Y0F) - MARGIN), cy1 = Math.min(H, Math.floor(H * Y1F) + MARGIN);
  const y0 = Math.floor(H * Y0F), y1 = Math.floor(H * Y1F);
  const boxes = [];
  const w = (X1 - X0) / bands;
  for (let b = 0; b < bands; b++) {
    boxes.push({ x0: Math.floor(W * (X0 + w * b)) + 1, x1: Math.floor(W * (X0 + w * (b + 1))) - 1, y0, y1 });
  }
  return { W, H, cx0, cy0, cw: cx1 - cx0, ch: cy1 - cy0, boxes, roi: { x0: Math.floor(W * X0), x1: Math.floor(W * X1), y0, y1 } };
}
function cropOf(L, geo) {
  const out = new Float64Array(geo.cw * geo.ch);
  for (let y = 0; y < geo.ch; y++) for (let x = 0; x < geo.cw; x++) out[y * geo.cw + x] = L[(geo.cy0 + y) * geo.W + geo.cx0 + x];
  return out;
}

// The mask (point 1 of the header): per band, which pixels are interior and
// which are edge, from a sigma-4 smoothing of ONE capture.
//
// Interior pixels must also be out of reach of every edge. The band filter's
// widest Gaussian is sigma b*sqrt2 (5.66 px at the default scale), so an edge's
// own band response is still strong several pixels away from it, and a
// flat-looking pixel next to a cloud outline would count that response as
// noise. So the interior excludes anything within GAP = 2 * b * sqrt2 px of an
// edge pixel (in ANY band or in the margin), where the edge response has fallen
// below exp(-2), about 14 percent. Without the gap, a synthetic scene with
// perfectly flat cloud interiors and no texture at all read 0.79 percent of
// "noise" at 15 px that averaging nine frames could not remove: it was the
// edges, seen from next door.
function buildMask(crop, geo) {
  const { cw, ch } = geo;
  const g4 = gauss(crop, cw, ch, 4);
  const grad = new Float32Array(cw * ch);
  for (let y = 1; y < ch - 1; y++) for (let x = 1; x < cw - 1; x++) {
    const i = y * cw + x;
    grad[i] = Math.hypot(g4[i + 1] - g4[i - 1], g4[i + cw] - g4[i - cw]) / 2;
  }
  // Per-band thresholds: the median and the 90th percentile of the smoothed
  // gradient over the band's lit pixels.
  const thr = geo.boxes.map((bx) => {
    const lit = [];
    for (let y = bx.y0; y < bx.y1; y++) for (let x = bx.x0; x < bx.x1; x++) {
      const i = (y - geo.cy0) * cw + (x - geo.cx0);
      if (g4[i] >= LIT) lit.push(grad[i]);
    }
    if (!lit.length) return null;
    lit.sort((p, q) => p - q);
    return { med: lit[Math.floor(0.5 * (lit.length - 1))], p90: lit[Math.floor(0.9 * (lit.length - 1))] };
  });
  // Edge pixels over the WHOLE crop (margins and neighbouring bands included,
  // each column judged by its own band's threshold), so an outline just
  // outside a band still keeps the interior away from itself.
  const w = (X1 - X0) / bands;
  const colBand = new Int32Array(cw);
  for (let x = 0; x < cw; x++) {
    colBand[x] = Math.min(bands - 1, Math.max(0, Math.floor(((geo.cx0 + x) / geo.W - X0) / w)));
  }
  const edge = new Uint8Array(cw * ch);
  for (let y = 0; y < ch; y++) for (let x = 0; x < cw; x++) {
    const i = y * cw + x, t = thr[colBand[x]];
    if (t && g4[i] >= LIT && grad[i] >= t.p90) edge[i] = 1;
  }
  const GAP = Math.ceil(2 * SCALE[1] * Math.SQRT2);
  const near = dilate(edge, cw, ch, GAP);
  const kind = new Uint8Array(cw * ch); // 0 unused, 1 interior, 2 edge
  const counts = [];
  for (let b = 0; b < geo.boxes.length; b++) {
    const bx = geo.boxes[b], t = thr[b];
    let ni = 0, ne = 0;
    if (t) {
      for (let y = bx.y0; y < bx.y1; y++) for (let x = bx.x0; x < bx.x1; x++) {
        const i = (y - geo.cy0) * cw + (x - geo.cx0);
        if (g4[i] < LIT) continue;
        if (edge[i]) { kind[i] = 2; ne++; }
        else if (grad[i] <= t.med && !near[i]) { kind[i] = 1; ni++; }
      }
    }
    counts.push({ interior: ni, edge: ne, gap: GAP });
  }
  return { kind, counts };
}
// Square dilation of a 0/1 mask by r px (separable running max).
function dilate(m, W, H, r) {
  const tmp = new Uint8Array(W * H), out = new Uint8Array(W * H);
  for (let y = 0; y < H; y++) {
    let last = -1e9;
    for (let x = 0; x < W; x++) { if (m[y * W + x]) last = x; if (x - last <= r) tmp[y * W + x] = 1; }
    last = 1e9;
    for (let x = W - 1; x >= 0; x--) { if (m[y * W + x]) last = x; if (last - x <= r) tmp[y * W + x] = 1; }
  }
  for (let x = 0; x < W; x++) {
    let last = -1e9;
    for (let y = 0; y < H; y++) { if (tmp[y * W + x]) last = y; if (y - last <= r) out[y * W + x] = 1; }
    last = 1e9;
    for (let y = H - 1; y >= 0; y--) { if (tmp[y * W + x]) last = y; if (last - y <= r) out[y * W + x] = 1; }
  }
  return out;
}

// Noise, edge energy and noise-corrected detail of one band response, per band.
function ratioStats(resp, crop, geo, mask) {
  const out = [];
  for (let b = 0; b < geo.boxes.length; b++) {
    const bx = geo.boxes[b];
    let ni = 0, si = 0, li = 0, ne = 0, se = 0, le = 0;
    for (let y = bx.y0; y < bx.y1; y++) for (let x = bx.x0; x < bx.x1; x++) {
      const i = (y - geo.cy0) * geo.cw + (x - geo.cx0);
      const k = mask.kind[i];
      if (!k) continue;
      const v = resp[i];
      if (k === 1) { ni++; si += v * v; li += crop[i]; } else { ne++; se += v * v; le += crop[i]; }
    }
    if (ni < MIN_INTERIOR || ne < MIN_EDGE || li <= 0 || le <= 0) { out.push(null); continue; }
    const noise = (100 * Math.sqrt(si / ni)) / (li / ni);
    const edge = (100 * Math.sqrt(se / ne)) / (le / ne);
    const detail = Math.sqrt(Math.max(0, edge * edge - noise * noise));
    out.push({ noise, edge, detail, grain: detail > 0 ? (100 * noise) / detail : Infinity });
  }
  return out;
}

// The 2026-09-22 metric, byte for byte in its arithmetic, on the crop. Kept so
// every number quoted before the fix can be compared: it IS the pixel-scale noise.
function oldSpeckle(crop, geo) {
  const { cw } = geo, out = [];
  for (const bx of geo.boxes) {
    let n = 0, sum = 0, dev = 0;
    for (let y = bx.y0; y < bx.y1; y++) for (let x = bx.x0; x < bx.x1; x++) {
      const i = (y - geo.cy0) * cw + (x - geo.cx0);
      const L = crop[i];
      if (L < LIT) continue;
      const m = (crop[i - 1] + crop[i + 1] + crop[i - cw] + crop[i + cw]) / 4;
      n++; sum += L; dev += Math.abs(L - m);
    }
    out.push(n < 500 ? null : { meanL: sum / n, speckle: (100 * dev) / sum });
  }
  return out;
}

function fizzOf(a, b, geo, box) {
  let s = 0, d = 0;
  for (let y = box.y0; y < box.y1; y++) for (let x = box.x0; x < box.x1; x++) {
    const i = y * geo.W + x;
    if (a[i] < LIT) continue;
    s += a[i]; d += Math.abs(a[i] - b[i]);
  }
  return s > 0 ? (100 * d) / s : null;
}

function score(crop, geo, mask) {
  const memo = new Map();
  return {
    old: oldSpeckle(crop, geo),
    s15: ratioStats(bandSq(crop, geo.cw, geo.ch, SCALE[0], SCALE[1], memo), crop, geo, mask),
    s9: ratioStats(bandSq(crop, geo.cw, geo.ch, SCALE9[0], SCALE9[1], memo), crop, geo, mask),
  };
}

// Peak period (px) of the headline band, for labels: a squared DoG peaks where
// the plain DoG does, at w^2 = 2 ln(b^2/a^2) / (b^2 - a^2).
const PX = Math.round((2 * Math.PI) / Math.sqrt((2 * Math.log((SCALE[1] * SCALE[1]) / (SCALE[0] * SCALE[0]))) / (SCALE[1] * SCALE[1] - SCALE[0] * SCALE[0])));
const pct = (v, d = 2) => (v == null || !isFinite(v) ? "-" : v.toFixed(d) + "%");
const pad = (s, n) => String(s).padStart(n);
const base = (f) => path.basename(f);

(async () => {
  const loaded = [];
  for (const f of files) loaded.push({ f, img: await loadLuma(f) });
  const ref = loaded[0];
  const geo = frameGeometry(ref.img.W, ref.img.H);
  for (const l of loaded) {
    if (!ownMask && (l.img.W !== ref.img.W || l.img.H !== ref.img.H)) {
      console.error(
        `ERROR: ${base(l.f)} is ${l.img.W}x${l.img.H} but the reference ${base(ref.f)} is ${ref.img.W}x${ref.img.H}.\n` +
          "  The shared edge mask only makes sense for captures of ONE camera. Compare captures of the\n" +
          "  same vantage and width, or pass --own-mask to judge each file on its own geometry."
      );
      process.exit(2);
    }
  }
  const refCrop = cropOf(ref.img.L, geo);
  const refMask = buildMask(refCrop, geo);
  const results = [];
  for (const l of loaded) {
    const g = ownMask ? frameGeometry(l.img.W, l.img.H) : geo;
    const crop = l === ref ? refCrop : cropOf(l.img.L, g);
    const mask = ownMask && l !== ref ? buildMask(crop, g) : refMask;
    const r = { file: l.f, geo: g, crop, mask, ...score(crop, g, mask) };
    const sib = l.f.replace(/\.png$/i, "-b.png");
    if (sib !== l.f && fs.existsSync(sib)) {
      const b = await loadLuma(sib);
      if (b.W === l.img.W && b.H === l.img.H) r.fizzPair = { sib, a: l.img.L, b: b.L };
    }
    results.push(r);
  }

  // Focus band: explicit, or the band where the reference's 15 px noise is
  // loudest AMONG JUDGEABLE bands. Not where GRAIN peaks: on the night side the
  // real detail can drop below the noise, the ratio explodes, and the "peak"
  // lands on a band with nothing to judge (band 9 of the 2026-09-22 baseline
  // read 197% in an early draft of this measure, before the edge gap). The
  // loudest noise that still has real detail beside it is where the defect lives.
  let fb = focusBand;
  if (fb == null) {
    let loud = -1;
    results[0].s15.forEach((s, b) => { if (judgeable(s) && s.noise > loud) { loud = s.noise; fb = b; } });
  }
  if (fb == null || fb < 0 || fb >= bands) { console.error("no band with enough lit cloud to judge"); process.exit(2); }

  // Profiles.
  const printProfile = (r, label) => {
    console.log(`${base(r.file)}${label}`);
    console.log(`  band   mean L   speckle (old)    GRAIN   noise@${PX}px  detail@${PX}px`);
    let pk = -1, pkb = -1, opk = -1, opkb = -1;
    for (let b = 0; b < bands; b++) {
      const o = r.old[b], s = r.s15[b];
      if (!o) { console.log(`  ${pad(b, 4)}   (too few lit pixels)`); continue; }
      if (o.speckle > opk) { opk = o.speckle; opkb = b; }
      if (judgeable(s) && s.grain > pk) { pk = s.grain; pkb = b; }
      const g = !s ? "(no edges)" : judgeable(s) ? pct(s.grain, 1) : "(weak detail)";
      console.log(
        `  ${pad(b, 4)} ${pad(o.meanL.toFixed(1), 8)} ${pad(pct(o.speckle), 15)} ${pad(g, 13)}` +
          ` ${pad(s ? pct(s.noise, 3) : "-", 11)} ${pad(s ? pct(s.detail, 3) : "-", 12)}`
      );
    }
    console.log(`  PEAK GRAIN ${pct(pk, 1)} at band ${pkb}   (old speckle peak ${pct(opk)} at band ${opkb})`);
  };
  if (!sharpnessLock) {
    console.log(
      "!! --no-sharpness-lock: the sharpness lock is OFF. This run exists to show the controls can fail;\n" +
        "!! its verdicts are NOT a ranking of anything.\n"
    );
  }
  printProfile(results[0], "   [reference]");
  if (profileAll) for (const r of results.slice(1)) printProfile(r, "");

  // Comparison at the focus band.
  const R0 = results[0];
  const s0 = R0.s15[fb], o0 = R0.old[fb], d90 = R0.s9[fb];
  console.log(
    `\nAT BAND ${fb}: GRAIN = noise / real detail at ~${PX} px (sigma ${SCALE[0]}-${SCALE[1]}), against the reference ${base(R0.file)}`
  );
  console.log(
    `  capture                               mean L  speckle(old)   GRAIN   vs ref  verdict | noise removed px / ${PX}px | detail kept 9px / ${PX}px | fizz roi / band`
  );
  const rows = [];
  for (const r of results) {
    const s = r.s15[fb], o = r.old[fb], d9 = r.s9[fb];
    const fz = r.fizzPair
      ? { roi: fizzOf(r.fizzPair.a, r.fizzPair.b, r.geo, r.geo.roi), band: fizzOf(r.fizzPair.a, r.fizzPair.b, r.geo, r.geo.boxes[fb]) }
      : null;
    const rel = s && s0 ? s.grain / s0.grain : null;
    const verdict = verdictOf(rel, d9 && d90 ? d9.detail / d90.detail : null, s && s0 ? s.detail / s0.detail : null);
    const row = {
      file: r.file, band: fb, meanL: o ? o.meanL : null, speckle_old: o ? o.speckle : null,
      grain: s ? s.grain : null, noise15: s ? s.noise : null, detail15: s ? s.detail : null, detail9: d9 ? d9.detail : null,
      grain_vs_ref: rel, verdict,
      noise_removed_px: o && o0 ? 1 - o.speckle / o0.speckle : null,
      noise_removed_15: s && s0 ? 1 - s.noise / s0.noise : null,
      detail_kept_9: d9 && d90 ? d9.detail / d90.detail : null,
      detail_kept_15: s && s0 ? s.detail / s0.detail : null,
      fizz_roi: fz ? fz.roi : null, fizz_band: fz ? fz.band : null,
      mask: r.mask.counts[fb],
    };
    rows.push(row);
    const x = (v) => (v == null ? "-" : (100 * v).toFixed(0) + "%");
    console.log(
      `  ${base(r.file).slice(0, 36).padEnd(36)} ${pad(row.meanL == null ? "-" : row.meanL.toFixed(1), 7)} ${pad(pct(row.speckle_old), 13)}` +
        ` ${pad(pct(row.grain, 1), 7)} ${pad(rel == null ? "-" : (1 / rel).toFixed(2) + "x", 7)}  ${verdict.padEnd(6)}` +
        ` | ${pad(x(row.noise_removed_px), 8)} / ${pad(x(row.noise_removed_15), 5)}      | ${pad(x(row.detail_kept_9), 7)} / ${pad(x(row.detail_kept_15), 5)}     |` +
        ` ${fz ? pad(pct(fz.roi), 6) + " / " + pad(pct(fz.band), 6) : "-"}`
    );
  }
  console.log(`  (mask at band ${fb}: ${R0.mask.counts[fb].interior} interior px, ${R0.mask.counts[fb].edge} edge px; "vs ref" > 1 is less grain)`);

  // Controls: blur each capture and prove the blur cannot win.
  let failed = 0;
  const controlRows = [];
  if (controls) {
    console.log(`\nCONTROLS at band ${fb}: a blur must NOT be judged BETTER (and must fool the old speckle, or it proves nothing)`);
    console.log("  source                               blur        speckle(old)          GRAIN         detail kept 9/15   verdict   result");
    // Two kinds of blur on purpose. The MILD one (sigma 0.5) keeps the 9 px
    // detail, so only the scale lock can stop it: it is the control that goes
    // red when the scale is moved to where a blur wins (--scale 0.71,1.41).
    // The WIDE ones (sigma 2, 3) reach into the headline band itself, where a
    // finite band can tilt, so the sharpness lock is what stops them.
    const blurs = [
      ["box 3x3", (c, g) => boxBlur(c, g.cw, g.ch, 1)],
      ["box 5x5", (c, g) => boxBlur(c, g.cw, g.ch, 2)],
      ["gauss s0.5", (c, g) => gauss(c, g.cw, g.ch, 0.5)],
      ["gauss s1", (c, g) => gauss(c, g.cw, g.ch, 1)],
      ["gauss s2", (c, g) => gauss(c, g.cw, g.ch, 2)],
      ["gauss s3", (c, g) => gauss(c, g.cw, g.ch, 3)],
    ];
    for (const r of results) {
      const src15 = r.s15[fb], srcOld = r.old[fb], src9 = r.s9[fb];
      if (!src15 || !srcOld || !src9) { console.log(`  ${base(r.file)}: band ${fb} has too little lit cloud to control`); continue; }
      for (const [name, fn] of blurs) {
        const blurred = fn(r.crop, r.geo);
        const sc = score(blurred, r.geo, r.mask);
        const b15 = sc.s15[fb], bOld = sc.old[fb], b9 = sc.s9[fb];
        const gRel = b15 ? b15.grain / src15.grain : null;
        const oRel = bOld ? bOld.speckle / srcOld.speckle : null;
        const k9 = b9 ? b9.detail / src9.detail : null, k15 = b15 ? b15.detail / src15.detail : null;
        const v = verdictOf(gRel, k9, k15);
        let result = "PASS";
        if (gRel == null || oRel == null) result = "FAIL (could not score)";
        else if (oRel > 1 - CONTROL_MIN_FOOL) result = "FAIL (control too weak: the old speckle fell only " + (100 * (1 - oRel)).toFixed(0) + "%)";
        else if (v === "BETTER") {
          result = "FAIL (a blur was judged BETTER: GRAIN " + (100 * (1 - gRel)).toFixed(0) + "% lower" +
            (sharpnessLock ? " with the detail kept)" : ", sharpness lock OFF)");
        }
        if (result !== "PASS") failed++;
        controlRows.push({ file: r.file, blur: name, speckle_old: bOld && bOld.speckle, grain: b15 && b15.grain, grain_rel: gRel, speckle_rel: oRel, detail_kept_9: k9, detail_kept_15: k15, verdict: v, result });
        const x = (q) => (q == null ? "-" : (100 * q).toFixed(0) + "%");
        console.log(
          `  ${base(r.file).slice(0, 36).padEnd(36)} ${name.padEnd(10)} ${pad(pct(srcOld.speckle), 7)} -> ${pad(pct(bOld && bOld.speckle), 7)}` +
            `  ${pad(pct(src15.grain, 1), 6)} -> ${pad(pct(b15 && b15.grain, 1), 6)}   ${pad(x(k9), 5)} / ${pad(x(k15), 5)}   ${v.padEnd(8)}  ${result}`
        );
      }
    }
  }

  if (jsonOut) {
    fs.writeFileSync(jsonOut, JSON.stringify({ reference: files[0], band: fb, scale: SCALE, scale9: SCALE9, rows, controls: controlRows }, null, 2));
    console.log(`\nwrote ${jsonOut}`);
  }
  if (controls) {
    if (failed) {
      console.log(
        `\nCONTROLS FAILED (${failed}). A plain blur scored as a grain improvement at this scale, or a control was too weak to test anything.\n` +
          "  Why it matters: if a blur can win, a spatial-filter arm's GRAIN number rewards blurring, not a better picture,\n" +
          "  and ranking candidates by it repeats the 2026-09-22 flaw (PRIORITIES 2a-ii).\n" +
          "  What to do: do not rank spatial arms on this band. Check the scale (--scale, default 2.83,4) against this\n" +
          "  vantage's detail, or measure at a vantage whose cloud edges carry detail at the scale being judged."
      );
      process.exit(1);
    }
    console.log(`\nCONTROLS PASSED: no blur of any given capture scored as a GRAIN improvement at band ${fb}.`);
  }
})();
