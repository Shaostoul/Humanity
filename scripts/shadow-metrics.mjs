// Shadow presence from the RENDERED pixels (2026-11-10, the shadow atlas; docs/design/local-light-shadows.md).
//
// Usage:
//   node scripts/shadow-metrics.mjs <dir> [--gate] [--before <dir>]
//     <dir> holds pairs of captures: <id>.png and <id>-off.png (probe-sweep of a shadow-* vantage and
//     its red twin), or <arm>__<id>.png pairs from probe-hot-ab with the arms "on" and "off".
//     For each pair it finds the SHADOW REGION, the pixels the atlas made darker than the same frame
//     with the atlas off by at least DARKER codes of sRGB luma, and reports how much of the frame that
//     is and how much darker it reads there (mean codes, and the ratio of linear light: 0.4 means the
//     shaded floor keeps 40% of its light).
//     --before <dir>: a capture of the same vantage from an older build (<id>.png or fig__<id>.png),
//     measured inside this run's shadow region against the off twin: "no shadow before" is a ratio
//     near 1.0 there, a number rather than an impression.
//     --gate: exit 1 unless every pair's shadow region covers at least MIN_COVER of the frame with a
//     mean drop of at least MIN_DROP codes.
//
// WHY A DIFFERENCE AGAINST THE TWIN AND NOT A BOX. Where a shadow lands depends on the lamp, the pose and
// the body's idle sway; a hand-drawn box measures the box. The red twin is the same frame, the same
// park and the same build with only the atlas switched off, so whatever differs is what the atlas did
// (consecutive captures of one park differ only where the scene itself moves: probe-hot-ab.js). Seen
// red first: on two OFF captures of one park (the twin against itself) the region is under 0.05% of
// the frame, which is the floor this gate stands above.

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

export const DARKER = 6;
export const MIN_COVER = 0.004;
export const MIN_DROP = 12;

function luma(r, g, b) {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function lin(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

async function load(file) {
  const { data, info } = await sharp(file).removeAlpha().raw().toBuffer({ resolveWithObject: true });
  const n = info.width * info.height;
  const y = new Float32Array(n);
  for (let i = 0; i < n; i++) y[i] = luma(data[i * 3], data[i * 3 + 1], data[i * 3 + 2]);
  return { y, w: info.width, h: info.height };
}

// The pixels that MOVE on their own between two captures of one arm (a living body's idle sway, a
// swinging lamp), grown by `grow` pixels: they are left out, because a body's silhouette shifting
// between the on and off captures would read as "darker" where it now stands.
async function motionMask(files, w, h, grow = 10) {
  const moved = new Uint8Array(w * h);
  for (const [a, b] of files) {
    if (!a || !b || !fs.existsSync(a) || !fs.existsSync(b)) continue;
    const A = await load(a);
    const B = await load(b);
    if (A.w !== w || B.w !== w) continue;
    for (let i = 0; i < w * h; i++) if (Math.abs(A.y[i] - B.y[i]) >= DARKER) moved[i] = 1;
  }
  // Grow by a square of side 2 * grow + 1 (separable: rows, then columns).
  const rows = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) {
    let last = -1e9;
    for (let x = 0; x < w; x++) if (moved[y * w + x]) last = x;
      else if (x - last <= grow) rows[y * w + x] = 1;
    last = 1e9;
    for (let x = w - 1; x >= 0; x--) {
      if (moved[y * w + x]) { last = x; rows[y * w + x] = 1; } else if (last - x <= grow) rows[y * w + x] = 1;
    }
  }
  const out = new Uint8Array(w * h);
  for (let x = 0; x < w; x++) {
    let last = -1e9;
    for (let y = 0; y < h; y++) if (rows[y * w + x]) last = y;
      else if (y - last <= grow) out[y * w + x] = 1;
    last = 1e9;
    for (let y = h - 1; y >= 0; y--) {
      if (rows[y * w + x]) { last = y; out[y * w + x] = 1; } else if (last - y <= grow) out[y * w + x] = 1;
    }
  }
  return out;
}

export async function measurePair(onFile, offFile, beforeFile, repeats = []) {
  const on = await load(onFile);
  const off = await load(offFile);
  if (on.w !== off.w || on.h !== off.h) throw new Error(`size differs: ${onFile} vs ${offFile}`);
  const n = on.w * on.h;
  const moving = await motionMask(repeats, on.w, on.h);
  let count = 0, drop = 0, linOn = 0, linOff = 0, movingCount = 0;
  const mask = new Uint8Array(n);
  for (let i = 0; i < n; i++) {
    if (moving[i]) {
      movingCount++;
      continue;
    }
    const d = off.y[i] - on.y[i];
    if (d >= DARKER) {
      mask[i] = 1;
      count++;
      drop += d;
      linOn += lin(on.y[i]);
      linOff += lin(off.y[i]);
    }
  }
  const out = {
    cover: count / n,
    mean_drop_codes: count ? drop / count : 0,
    light_kept: linOff > 0 ? linOn / linOff : 1,
    moving_left_out: movingCount / n,
  };
  if (beforeFile && fs.existsSync(beforeFile)) {
    const b = await load(beforeFile);
    if (b.w === on.w && b.h === on.h && count) {
      let lb = 0, lo = 0, db = 0;
      for (let i = 0; i < n; i++) {
        if (!mask[i]) continue;
        lb += lin(b.y[i]);
        lo += lin(off.y[i]);
        db += off.y[i] - b.y[i];
      }
      out.before_light_kept = lo > 0 ? lb / lo : 1;
      out.before_mean_drop_codes = db / count;
    }
  }
  return out;
}

function pairsIn(dir) {
  const files = fs.readdirSync(dir).filter((f) => f.endsWith(".png"));
  const pairs = [];
  for (const f of files) {
    if (f.endsWith("-off.png")) continue;
    let m;
    if ((m = f.match(/^on__(.+)\.png$/))) {
      const off = `off__${m[1]}.png`;
      if (files.includes(off)) pairs.push({ id: m[1], on: f, off });
    } else if (!f.includes("__")) {
      const id = f.replace(/\.png$/, "");
      const off = `${id}-off.png`;
      if (files.includes(off)) pairs.push({ id, on: f, off });
    }
  }
  return pairs;
}

async function main() {
  const args = process.argv.slice(2);
  const dir = args.find((a) => !a.startsWith("--"));
  const gate = args.includes("--gate");
  const bi = args.indexOf("--before");
  const beforeDir = bi >= 0 ? args[bi + 1] : null;
  if (!dir) {
    console.error("usage: node scripts/shadow-metrics.mjs <dir> [--gate] [--before <dir>]");
    process.exit(2);
  }
  const pairs = pairsIn(dir);
  if (!pairs.length) {
    console.error(`no on/off pairs in ${dir}`);
    process.exit(2);
  }
  let failed = 0;
  const all = {};
  for (const p of pairs) {
    let before = null;
    if (beforeDir) {
      for (const cand of [`${p.id}.png`, `fig__${p.id}.png`, `a__${p.id}.png`]) {
        if (fs.existsSync(path.join(beforeDir, cand))) {
          before = path.join(beforeDir, cand);
          break;
        }
      }
    }
    // Repeat captures of one arm (on2 / off2 from probe-hot-ab) say what moves on its own.
    const rep = (arm) => {
      const f = path.join(dir, `${arm}__${p.id}.png`);
      return fs.existsSync(f) ? f : null;
    };
    const repeats = [
      [path.join(dir, p.off), rep("off2")],
      [path.join(dir, p.on), rep("on2")],
    ];
    const r = await measurePair(path.join(dir, p.on), path.join(dir, p.off), before, repeats);
    // The floor this gate stands above: the off twin against its own repeat, measured the same way.
    if (rep("off2")) {
      const floor = await measurePair(rep("off2"), path.join(dir, p.off), null, repeats);
      r.floor_cover = floor.cover;
      console.log(`     ${p.id}: off against its repeat (the floor): ${(floor.cover * 100).toFixed(3)}% of the frame`);
    }
    all[p.id] = r;
    const ok = r.cover >= MIN_COVER && r.mean_drop_codes >= MIN_DROP;
    if (!ok) failed++;
    const b = r.before_light_kept !== undefined ? `; before: light kept ${r.before_light_kept.toFixed(3)} (${r.before_mean_drop_codes.toFixed(1)} codes darker than the off twin)` : "";
    console.log(
      `${ok ? "OK  " : "FAIL"} ${p.id}: shadow region ${(r.cover * 100).toFixed(2)}% of the frame, ${r.mean_drop_codes.toFixed(1)} codes darker, light kept ${r.light_kept.toFixed(3)}${b} (moving pixels left out: ${(r.moving_left_out * 100).toFixed(2)}%)`
    );
  }
  fs.writeFileSync(path.join(dir, "shadow-metrics.json"), JSON.stringify(all, null, 1));
  if (gate && failed) {
    console.error(`shadow-metrics: ${failed} pair(s) below the gate (region >= ${MIN_COVER * 100}% and >= ${MIN_DROP} codes darker)`);
    process.exit(1);
  }
}

if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, "/")}` || process.argv[1].endsWith("shadow-metrics.mjs")) {
  main();
}
