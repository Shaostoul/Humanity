#!/usr/bin/env node
// The JOINT cloud acceptance verdict (environment program increment 8).
//
// Born from the phase-9 failure mode: an integrator change improved the
// speckle metric while the LOOK regressed (contrast p95/p05 fell
// 1.69 -> 1.46 because the old look depended on the integrator inflating
// opacity), and it shipped because speckle was the only number anyone
// gated on. From this increment on, NO integrator or lighting change may
// pass on speckle alone: the verdict is the AND of four metrics plus a
// clear-sky control, and the thresholds live as DATA on the vantage entry
// (tests/visual/vantages.json, "joint_gate" block) so re-tuning them is a
// reviewed data change, not a script edit.
//
//   node scripts/cloud-metrics.mjs <capture.png> --vantage <id>
//        [--control <clear-sky.png>] [--json]
//
// Metrics over the vantage's joint_gate.roi (x,y,w,h):
//   speckle  high-pass rms (pixel minus 3x3 box mean), luminance 0..1
//   mean_l   mean luminance, 0..255
//   contrast p95/p05 luminance ratio
//   p95      95th percentile luminance, 0..255
// Control (optional --control capture, same ROI): speckle must be under
// joint_gate.control_speckle_max - proves the ROI itself is quiet when no
// cloud is in it, i.e. the speckle number measures clouds, not the rig.
//
// Exit 0 = ALL gates pass. Exit 1 = any fail (each printed).

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const sharp = require('sharp');
const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
const files = args.filter(
  (a, i) =>
    !a.startsWith('--') &&
    args[i - 1] !== '--vantage' &&
    args[i - 1] !== '--control',
);
const asJson = args.includes('--json');
const vantageId = (() => {
  const i = args.indexOf('--vantage');
  return i < 0 ? null : args[i + 1];
})();
const controlFile = (() => {
  const i = args.indexOf('--control');
  return i < 0 ? null : args[i + 1];
})();

if (files.length !== 1 || !vantageId) {
  console.error(
    'usage: cloud-metrics.mjs <capture.png> --vantage <id> [--control <clear.png>] [--json]',
  );
  process.exit(2);
}

const repo = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1')), '..');
const vantages = JSON.parse(
  fs.readFileSync(path.join(repo, 'tests', 'visual', 'vantages.json'), 'utf8'),
);
const vantage = vantages.vantages.find((v) => v.id === vantageId);
if (!vantage) {
  console.error(`vantage '${vantageId}' not found`);
  process.exit(2);
}
const gate = vantage.joint_gate;
if (!gate || !gate.roi) {
  console.error(`vantage '${vantageId}' has no joint_gate block`);
  process.exit(2);
}
const [rx, ry, rw, rh] = gate.roi.split(',').map(Number);

async function roiStats(file) {
  const { data, info } = await sharp(file)
    .extract({ left: rx, top: ry, width: rw, height: rh })
    .raw()
    .toBuffer({ resolveWithObject: true });
  const ch = info.channels;
  const W = info.width;
  const H = info.height;
  const lum = new Float64Array(W * H);
  for (let i = 0; i < W * H; i++) {
    const o = i * ch;
    lum[i] = 0.2126 * data[o] + 0.7152 * data[o + 1] + 0.0722 * data[o + 2];
  }
  // Speckle: pixel minus 3x3 box mean, rms, in 0..1 luminance units.
  let acc = 0;
  let n = 0;
  for (let y = 1; y < H - 1; y++) {
    for (let x = 1; x < W - 1; x++) {
      let m = 0;
      for (let oy = -1; oy <= 1; oy++)
        for (let ox = -1; ox <= 1; ox++) m += lum[(y + oy) * W + (x + ox)];
      const d = lum[y * W + x] - m / 9;
      acc += d * d;
      n++;
    }
  }
  const speckle = Math.sqrt(acc / n) / 255;
  const sorted = Float64Array.from(lum).sort();
  const q = (p) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))];
  const mean = lum.reduce((a, b) => a + b, 0) / lum.length;
  const p05 = q(0.05);
  const p95 = q(0.95);
  return {
    speckle: +speckle.toFixed(5),
    mean_l: +mean.toFixed(1),
    p05: +p05.toFixed(1),
    p95: +p95.toFixed(1),
    contrast: +(p95 / Math.max(p05, 1)).toFixed(3),
  };
}

const s = await roiStats(files[0]);
const checks = [
  {
    name: 'speckle',
    ok: s.speckle <= gate.speckle_max,
    got: s.speckle,
    want: `<= ${gate.speckle_max}`,
  },
  {
    name: 'mean_l',
    ok: s.mean_l >= gate.mean_l_min,
    got: s.mean_l,
    want: `>= ${gate.mean_l_min}`,
  },
  {
    name: 'contrast',
    ok: s.contrast >= gate.contrast_min,
    got: s.contrast,
    want: `>= ${gate.contrast_min}`,
  },
  {
    name: 'p95',
    ok: s.p95 >= gate.p95_min,
    got: s.p95,
    want: `>= ${gate.p95_min}`,
  },
];
let control = null;
if (controlFile) {
  control = await roiStats(controlFile);
  checks.push({
    name: 'control_speckle',
    ok: control.speckle <= gate.control_speckle_max,
    got: control.speckle,
    want: `<= ${gate.control_speckle_max}`,
  });
}
const pass = checks.every((c) => c.ok);
const out = { file: files[0], vantage: vantageId, roi: gate.roi, stats: s, control, checks, verdict: pass ? 'PASS' : 'FAIL' };
if (asJson) console.log(JSON.stringify(out, null, 1));
else {
  console.log(`${files[0]}  roi=${gate.roi}`);
  for (const c of checks)
    console.log(`  ${c.ok ? 'ok  ' : 'FAIL'} ${c.name} = ${c.got} (want ${c.want})`);
  console.log(`JOINT VERDICT: ${out.verdict}`);
}
process.exit(pass ? 0 : 1);
