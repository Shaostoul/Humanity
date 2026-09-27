// Lattice-seam detector for the shared value noise (BUG-103, 2026-09-27).
//
//   node scripts/lattice-seam-metric.mjs <lines.png> [<image.png> ...] [--crop x,y,w,h] [--ch r|l|gb]
//
// <lines.png> is a capture of the DEBUG view the A/B plan patches into the
// wave shell's last write: R = sea_var itself, G = an ID of the value-noise
// lattice cell (every octave and plane that carries weight), B = 0. G changes
// exactly where a lattice line crosses the pixel grid, so its steps are the
// lattice lines. Each further <image.png> (same camera, same boot) is scored
// against those lines.
//
// For every pair of 4-neighbour pixels the script takes |d| of the chosen
// channel (default: R of the debug image itself when no image is given, or
// luminance of each image) and splits the pairs into those that CROSS a
// lattice line (G differs across the pair) and those that do not. It reports
//   cross/off   mean |d| on crossing pairs / mean |d| on the rest. A smooth
//               field reads ~1 (value noise with a smoothstep fade has no
//               step at a line); a field that steps at the lattice reads far
//               above 1. THE seam number.
//   p99 cross   99th percentile |d| on crossing pairs (the size of the seams)
//   big@lines   share of the large steps (|d| > the off-line 99.9th pct)
//               that sit on a crossing pair, against the share of pairs that
//               cross at all (the chance level)
//
// Channels: r (debug field), l (Rec.709 luma, for a real render), gb (G/B
// ratio x 100, the sea's hue axis: `greener` moves G against B).
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args.splice(i, 2)[1] : d; };
const cropArg = opt("--crop", null);
const chArg = opt("--ch", null);
// --self: every file is a DEBUG capture scored on its OWN lattice lines (R
// against its own G). Use it across arms: the first capture of a boot can sit
// a pixel off the others, and borrowed lines then read a clean field as clean
// and a seamed one as clean too.
const selfIdx = args.indexOf("--self");
const selfMode = selfIdx >= 0;
if (selfMode) args.splice(selfIdx, 1);
if (selfMode) {
  for (const f of args) {
    const r = await (await import("node:child_process")).execFileSync(process.execPath, [process.argv[1], f, "--ch", "r", ...(cropArg ? ["--crop", cropArg] : [])], { encoding: "utf8" });
    const j = JSON.parse(r).results[0];
    console.log(`${f.split(/[\\/]/).pop().padEnd(44)} cross/off ${j.cross_off}  mean_cross ${j.mean_cross}  mean_off ${j.mean_off}  p99_cross ${j.p99_cross}  big@lines ${j.big_at_lines} (chance ${j.chance})`);
  }
  process.exit(0);
}
const [linesFile, ...images] = args;

async function load(file) {
  const img = sharp(file);
  const meta = await img.metadata();
  const { data, info } = await img.removeAlpha().raw().toBuffer({ resolveWithObject: true });
  return { data, w: info.width, h: info.height, c: info.channels, meta };
}

const L = await load(linesFile);
let [cx, cy, cw, ch] = cropArg ? cropArg.split(",").map(Number) : [Math.round(L.w * 0.1), Math.round(L.h * 0.1), Math.round(L.w * 0.8), Math.round(L.h * 0.8)];
const G = (x, y) => L.data[(y * L.w + x) * L.c + 1];

// A pair crosses a lattice line when the cell ID differs by more than the
// quantisation noise across it.
const crosses = (x0, y0, x1, y1) => Math.abs(G(x0, y0) - G(x1, y1)) >= 2;

function score(img, chName) {
  const v = (x, y) => {
    const o = (y * img.w + x) * img.c;
    const r = img.data[o], g = img.data[o + 1], b = img.data[o + 2];
    if (chName === "r") return r;
    if (chName === "gb") return (100 * (g + 1)) / (b + 1);
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const on = [], off = [];
  for (let y = cy; y < cy + ch - 1; y++) {
    for (let x = cx; x < cx + cw - 1; x++) {
      const a = v(x, y);
      for (const [dx, dy] of [[1, 0], [0, 1]]) {
        const d = Math.abs(v(x + dx, y + dy) - a);
        (crosses(x, y, x + dx, y + dy) ? on : off).push(d);
      }
    }
  }
  const mean = (a) => a.reduce((s, x) => s + x, 0) / Math.max(1, a.length);
  const pct = (a, p) => { const s = Float64Array.from(a).sort(); return s[Math.min(s.length - 1, Math.floor(p * s.length))] ?? 0; };
  const thr = pct(off, 0.999);
  const bigOn = on.filter((d) => d > thr).length, bigOff = off.filter((d) => d > thr).length;
  const chance = on.length / (on.length + off.length);
  return {
    ch: chName,
    cross_off: +(mean(on) / Math.max(1e-9, mean(off))).toFixed(3),
    mean_cross: +mean(on).toFixed(3),
    mean_off: +mean(off).toFixed(3),
    p99_cross: +pct(on, 0.99).toFixed(2),
    big_at_lines: +(bigOn / Math.max(1, bigOn + bigOff)).toFixed(3),
    chance: +chance.toFixed(4),
    crossing_pairs: on.length,
  };
}

const out = { lines: linesFile, crop: [cx, cy, cw, ch], results: [] };
if (images.length === 0) {
  out.results.push({ image: linesFile, ...score(L, chArg || "r") });
} else {
  for (const f of images) {
    const img = await load(f);
    if (img.w !== L.w || img.h !== L.h) throw new Error(`${f}: size ${img.w}x${img.h} != lines ${L.w}x${L.h}`);
    for (const c of (chArg || "l,gb").split(",")) out.results.push({ image: f, ...score(img, c) });
  }
}
console.log(JSON.stringify(out, null, 1));
