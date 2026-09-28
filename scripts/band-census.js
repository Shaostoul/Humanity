// BAND CENSUS: how long the flat runs of one display code are in the DARK
// parts of a capture. Written 2026-09-27 for increment 4 of the HDR scene
// target (docs/design/hdr-scene-target.md), the one dither.
//
// WHAT BANDING IS, IN PIXELS. A slow dark gradient quantised to 8 bits lands
// in flat bands one code apart; along any line across the gradient, the codes
// come in long runs of the SAME value (a band) with a one-code step between
// them (the edge the eye reads as a contour). A dither before the write
// replaces each hard step with grain: neighbours then share a code only by
// chance (a pixel keeps its code into the next at most three times in four
// with triangular noise of one code), and the runs collapse to a few pixels
// (a mean of about 2 on a synthetic ramp, against 29 undithered). So the length of a run of identical
// codes is the direct measure of a band's width, and it is what this counts.
//
// WHAT IT COUNTS. Inside the crop (default: the frame less the HUD margins
// the other pixel scripts skip), a pixel is DARK when its brightest channel
// is at most --dark (default 64 of 255, about 5 percent linear light, where
// banding is most visible) and it is not pure black. A run is a maximal line
// of dark pixels with the identical r, g, b triple, taken along rows and
// along columns. Pure black is left out on purpose: the space background is
// exactly 0 and stays exactly 0 under the dither (present.wgsl keeps black
// black), so counting it would report the sky, not the bands.
//
// THE NUMBERS, one line per file:
//   dark px     how many dark pixels the crop holds (a fix that darkens or
//               brightens a region away shows up here first)
//   mean run    dark pixels in runs / number of runs, rows and columns
//               averaged: THE band census
//   px-weighted the length of the run the average dark pixel sits in
//               (sum L^2 / sum L); weighted toward the long runs, which are
//               the visible bands
//   long%       share of dark pixels in a row run of 16 px or more
//
// A genuinely flat dark surface (a matte wall, the unlit side of a hull at
// one exact code) also makes long runs, before and after the dither alike, so
// compare two captures of ONE park, not two different vantages.
//
//   node scripts/band-census.js [--dark N] [--crop x,y,w,h] [--json] a.png [b.png ...]
const sharp = require("sharp");

const args = process.argv.slice(2);
let dark = 64;
let crop = null;
let json = false;
const files = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === "--dark") dark = Number(args[++i]);
  else if (a === "--crop") crop = args[++i].split(",").map(Number);
  else if (a === "--json") json = true;
  else files.push(a);
}
if (!files.length || !Number.isFinite(dark) || (crop && crop.length !== 4)) {
  console.error("usage: node scripts/band-census.js [--dark N] [--crop x,y,w,h] [--json] a.png [b.png ...]");
  process.exit(2);
}

// Runs along one line of `n` pixels whose byte offsets are `at(k)`.
function lineRuns(data, n, at, isDark, sink) {
  let run = 0;
  let prev = -1;
  for (let k = 0; k < n; k++) {
    const o = at(k);
    if (!isDark(o)) {
      if (run) sink(run);
      run = 0;
      prev = -1;
      continue;
    }
    if (prev >= 0 && data[o] === data[prev] && data[o + 1] === data[prev + 1] && data[o + 2] === data[prev + 2]) {
      run++;
    } else {
      if (run) sink(run);
      run = 1;
    }
    prev = o;
  }
  if (run) sink(run);
}

function census(data, W, H, C) {
  const [x0, y0, cw, ch] = crop || [
    Math.floor(W * 0.08),
    Math.floor(H * 0.12),
    Math.floor(W * 0.84),
    Math.floor(H * 0.76),
  ];
  const x1 = Math.min(W, x0 + cw);
  const y1 = Math.min(H, y0 + ch);
  const isDark = (o) => {
    const m = Math.max(data[o], data[o + 1], data[o + 2]);
    return m <= dark && m > 0;
  };
  const acc = () => ({ runs: 0, px: 0, sq: 0, long: 0 });
  const row = acc();
  const col = acc();
  const add = (a) => (L) => {
    a.runs++;
    a.px += L;
    a.sq += L * L;
    if (L >= 16) a.long += L;
  };
  for (let y = y0; y < y1; y++) lineRuns(data, x1 - x0, (k) => (y * W + x0 + k) * C, isDark, add(row));
  for (let x = x0; x < x1; x++) lineRuns(data, y1 - y0, (k) => ((y0 + k) * W + x) * C, isDark, add(col));
  const mean = (a) => (a.runs ? a.px / a.runs : 0);
  const pw = (a) => (a.px ? a.sq / a.px : 0);
  return {
    dark_px: row.px,
    mean_run: (mean(row) + mean(col)) / 2,
    px_weighted: (pw(row) + pw(col)) / 2,
    long_pct: row.px ? (100 * row.long) / row.px : 0,
    row_mean: mean(row),
    col_mean: mean(col),
    crop: [x0, y0, x1 - x0, y1 - y0],
  };
}

(async () => {
  const out = [];
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const r = census(data, info.width, info.height, info.channels);
    r.file = f;
    out.push(r);
    if (!json) {
      const label = f.split(/[\\/]/).slice(-1)[0];
      console.log(
        label.padEnd(44),
        "dark px",
        String(r.dark_px).padStart(8),
        " mean run",
        r.mean_run.toFixed(2).padStart(7),
        " px-weighted",
        r.px_weighted.toFixed(1).padStart(7),
        " long%",
        r.long_pct.toFixed(2).padStart(6)
      );
    }
  }
  if (json) console.log(JSON.stringify(out, null, 1));
})();
