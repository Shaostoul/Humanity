// Is cloud DETAIL continuous across the cloud-base-shell horizon?
//
// Usage: node scripts/cloud-seam-metrics.mjs <sweep-dir> [vantage-id ...]
//   with no ids, scores every base-horizon-seam-* vantage it finds.
//
// WHY (2026-08-27). The operator photographed a hard horizontal line at 5.7 and
// 6.2 km altitude, smoother cloud above it and sharper, grainier cloud below.
//
// It is the cloud BASE shell tangent. Inside the slab every ray starts at the
// camera and only its END varies, and it is clipped where the ray dives below
// the cloud base - so the marched segment steps 245 to 619 km across that
// tangent, a factor of 2.530. The per-ray footprint is frozen at the segment
// MIDPOINT, so it inherits that jump, and it sets the density rind plus four
// shape mips. 1.34 mip levels of surface detail, changing along one screen row.
//
// So the statistic is DETAIL, not luminance. A mip change barely moves mean
// brightness - the first attempt at this measured luminance, found nothing, and
// nearly concluded the seam was absent.
//
// THE GUARD THAT MATTERS. The first cut of the seam vantages had no cloud at the
// seam row at all, so it scored empty sky against empty sky and reported
// "before and after identical" - a check that could not fail. This script
// REFUSES to score a row whose cloud fraction is below the vantage's
// cloud_min_frac, and says so loudly. A gate that cannot go red is not a gate.
//
// Thresholds and the predicted row live as DATA on the vantage entry
// (seam_gate), following scripts/cloud-metrics.mjs, so re-tuning them is a
// reviewed data change rather than a script edit.

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/cloud-seam-metrics.mjs <sweep-dir> [vantage-id ...]");
  process.exit(2);
}
const wanted = process.argv.slice(3);

const spec = JSON.parse(
  fs.readFileSync(path.join("tests", "visual", "vantages.json"), "utf8")
);
let entries = spec.vantages.filter(v => v.seam_gate);
if (wanted.length) entries = entries.filter(v => wanted.includes(v.id));
if (!entries.length) {
  console.error("no vantages with a seam_gate block matched");
  process.exit(2);
}

// Cloud is bright and low-saturation against blue sky or tan ground. Classify on
// luminance plus how grey the pixel is, which separates cloud from both.
function isCloud(r, g, b) {
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  const sat = mx === 0 ? 0 : (mx - mn) / mx;
  return lum > 110 && sat < 0.22;
}

async function score(file, gate) {
  const img = sharp(file);
  const meta = await img.metadata();
  const W = meta.width, H = meta.height;
  const x0 = Math.round(W * gate.x_range[0]);
  const x1 = Math.round(W * gate.x_range[1]);
  const top = Math.max(1, gate.row - gate.band - 2);
  const height = Math.min(H - top - 1, gate.band * 2 + 5);
  const { data, info } = await img
    .extract({ left: x0, top, width: x1 - x0, height })
    .raw()
    .toBuffer({ resolveWithObject: true });
  const w = info.width, h = info.height, ch = info.channels;

  const rows = [];
  for (let y = 1; y < h - 1; y++) {
    let cloud = 0, det = 0, lum = 0, n = 0;
    for (let x = 1; x < w - 1; x++) {
      const o = (y * w + x) * ch;
      const r = data[o], g = data[o + 1], b = data[o + 2];
      if (!isCloud(r, g, b)) continue;
      cloud++;
      const L = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      // Horizontal gradient only: a horizontal seam must not be measured with a
      // vertical operator, which would fire on the seam itself and call it detail.
      const oL = (y * w + x - 1) * ch, oR = (y * w + x + 1) * ch;
      const Ll = 0.2126 * data[oL] + 0.7152 * data[oL + 1] + 0.0722 * data[oL + 2];
      const Lr = 0.2126 * data[oR] + 0.7152 * data[oR + 1] + 0.0722 * data[oR + 2];
      det += Math.abs(Lr - Ll) * 0.5;
      lum += L;
      n++;
    }
    rows.push({
      y: top + y,
      frac: cloud / (w - 2),
      det: n ? det / n : 0,
      lum: n ? lum / n : 0,
    });
  }

  const mid = rows.findIndex(r => r.y === gate.row);
  if (mid < 0) return { error: `predicted row ${gate.row} outside the crop` };
  const above = rows.filter(r => r.y >= gate.row - gate.band && r.y <= gate.row - 3);
  const below = rows.filter(r => r.y >= gate.row + 3 && r.y <= gate.row + gate.band);
  const mean = (a, k) => a.reduce((s, v) => s + v[k], 0) / Math.max(a.length, 1);

  const fracAbove = mean(above, "frac"), fracBelow = mean(below, "frac");
  const detAbove = mean(above, "det"), detBelow = mean(below, "det");
  const lumAbove = mean(above, "lum"), lumBelow = mean(below, "lum");
  const ratio = Math.max(detAbove, detBelow) / Math.max(Math.min(detAbove, detBelow), 1e-6);

  return { fracAbove, fracBelow, detAbove, detBelow, lumAbove, lumBelow, ratio };
}

let fails = 0, skipped = 0;
for (const v of entries) {
  const f = path.join(dir, `${v.id}.png`);
  if (!fs.existsSync(f)) { console.error(`missing capture: ${f}`); process.exit(2); }
  const g = v.seam_gate;
  const r = await score(f, g);
  console.log(`\n${v.id}  (tangent ${g.tangent_deg_below_horizontal} deg below horizontal -> row ${g.row})`);
  if (r.error) { console.error("  " + r.error); fails++; continue; }
  console.log(`  cloud fraction   above ${r.fracAbove.toFixed(3)}   below ${r.fracBelow.toFixed(3)}   (need > ${g.cloud_min_frac})`);
  // SECOND GUARD, added after the first one was fooled (2026-08-27).
  //
  // Cloud fraction alone is not enough: near the horizon the sky washes out to a
  // pale, low-saturation haze that the classifier happily calls cloud. That
  // reported cloud_frac 1.000 on BOTH sides of the tangent while the region above
  // the row was in fact empty sky - and the resulting 6.6x "detail step" was
  // simply haze-versus-cloud, not a seam at all. A second check-that-cannot-fail,
  // wearing the costume of the guard written to prevent the first one.
  //
  // Haze has essentially no high-frequency structure, so requiring real detail on
  // BOTH sides separates it from cloud without any new classification.
  const detFloor = 0.15;
  if (r.detAbove < detFloor || r.detBelow < detFloor) {
    console.error(
      "  SKIPPED - one side has almost no structure (detail " +
        r.detAbove.toFixed(3) + " / " + r.detBelow.toFixed(3) +
        ", need > " + detFloor + ").\n" +
      "  That side is horizon haze, not cloud, whatever the cloud fraction says.\n" +
      "  Reproducing this seam needs the camera INSIDE a deck with cloud ABOVE it,\n" +
      "  not a scattered field seen from above."
    );
    skipped++;
    continue;
  }
  if (r.fracAbove < g.cloud_min_frac || r.fracBelow < g.cloud_min_frac) {
    console.error(
      "  SKIPPED - not enough cloud at the seam row to measure anything.\n" +
      "  This is the exact failure the first version of these vantages had: it\n" +
      "  compared empty sky to empty sky and reported no difference. Raise\n" +
      "  cloud_cover or re-aim until both sides are above the threshold."
    );
    skipped++;
    continue;
  }
  console.log(`  detail           above ${r.detAbove.toFixed(3)}   below ${r.detBelow.toFixed(3)}   ratio ${r.ratio.toFixed(3)}  (want < ${g.detail_ratio_max})`);
  console.log(`  mean luminance   above ${r.lumAbove.toFixed(1)}   below ${r.lumBelow.toFixed(1)}`);
  if (r.ratio >= g.detail_ratio_max) {
    console.error(`  FAIL - cloud detail steps by ${((r.ratio - 1) * 100).toFixed(0)}% across the base-shell horizon.`);
    fails++;
  } else {
    console.log("  PASS");
  }
}

if (skipped) {
  console.error(`\n${skipped} vantage(s) UNSCORED. That is not a pass.`);
  process.exit(2);
}
if (fails) { console.error(`\n${fails} vantage(s) FAILED.`); process.exit(1); }
console.log("\nPASS: cloud detail is continuous across the base-shell horizon.");
