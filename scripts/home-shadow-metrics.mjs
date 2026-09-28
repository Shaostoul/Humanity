// Region-of-interest metrics for the sun-cascade captures: does a home (or a
// planet hut) actually shade the places a shelf or a roof covers?
//
// Usage:
//   node scripts/home-shadow-metrics.mjs <dir> [<dir> ...]
//       every PNG in each dir whose name holds a known vantage id is measured
//       region by region (mean sRGB luma code, mean linear luma, std dev).
//   node scripts/home-shadow-metrics.mjs --gate <before-dir> <after-dir>
//       increment 1's acceptance (docs/design/sun-cascades.md section 4):
//       (a) the lowest shelf top at noon reads at most the night capture of
//           the same build plus 10% (linear), i.e. the sun term is gone;
//       (b) the open floor at noon moved by at most 2 sRGB codes;
//       (c) the planet hut's floor under the roof is darker than the sand in
//           the sun beside it by at least half the sand's light.
//       Exit 1 when a gate fails, 2 when a capture is missing.
//
// WHY A SCRIPT AND NOT A LOOK. The defect it measures is a 3-code difference
// (a shelf top under three shelves read 130 against the open floor's 133 at
// noon, v0.1393): the home cast nothing into the sun map, so every shelf was
// lit as if the shelves above it were not there. Nobody can see 3 codes; a
// region mean can. Boxes are authored at 1600x900 and scaled to the capture.
//
// Captures come from tests/visual/vantages.json (home-racks-noon/dawn/night,
// home-beds-noon, home-overview-noon, planet-built-inside), taken either by
// probe-sweep.js (<id>.png) or by probe-hot-ab.js (<arm>__<id>.png).
//
// The boxes (x0, y0, x1, y1 at 1600x900) were read off the v0.1393 captures
// of those vantages; if a pose moves, re-read them from a fresh capture.

import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
const require = createRequire(import.meta.url);
const sharp = require("sharp");

// Region boxes per vantage FAMILY (the id with its clock suffix dropped).
export const REGIONS = {
  "home-racks": {
    floor: [1180, 700, 1420, 840],
    shelf1_low: [455, 655, 560, 690],
    shelf2: [455, 520, 560, 548],
    shelf3: [455, 390, 560, 415],
    shelf4_top: [455, 262, 560, 285],
    post_base: [575, 690, 600, 760],
    wall: [930, 250, 1150, 380],
  },
  "home-beds": {
    soil_open: [700, 760, 900, 860],
    beds_near: [500, 560, 1100, 700],
  },
  "home-overview": {
    acre_mid: [640, 380, 960, 560],
  },
  "planet-built-inside": {
    floor_under_roof: [700, 700, 900, 820],
    sand_sun: [40, 820, 200, 880],
  },
};

const BASE_W = 1600;
const BASE_H = 900;

function srgbToLinear(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

export async function measure(file, boxes) {
  const img = sharp(file);
  const meta = await img.metadata();
  const sx = meta.width / BASE_W;
  const sy = meta.height / BASE_H;
  const { data, info } = await img.raw().toBuffer({ resolveWithObject: true });
  const out = {};
  for (const [name, [x0, y0, x1, y1]] of Object.entries(boxes)) {
    const X0 = Math.round(x0 * sx), X1 = Math.round(x1 * sx);
    const Y0 = Math.round(y0 * sy), Y1 = Math.round(y1 * sy);
    let n = 0, sum = 0, sum2 = 0, lin = 0;
    for (let y = Y0; y < Y1; y++) {
      for (let x = X0; x < X1; x++) {
        const o = (y * info.width + x) * info.channels;
        const r = data[o], g = data[o + 1], b = data[o + 2];
        const code = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        sum += code;
        sum2 += code * code;
        lin += 0.2126 * srgbToLinear(r) + 0.7152 * srgbToLinear(g) + 0.0722 * srgbToLinear(b);
        n++;
      }
    }
    const mean = sum / n;
    out[name] = { code: mean, lin: lin / n, sd: Math.sqrt(Math.max(sum2 / n - mean * mean, 0)) };
  }
  return { size: [meta.width, meta.height], regions: out };
}

// The vantage family a capture file belongs to, and its clock ("noon",
// "dawn", "night" or ""), from names like `home-racks-noon.png` or
// `near1__home-racks-noon.png`.
export function familyOf(file) {
  const base = path.basename(file, ".png").split("__").pop();
  for (const fam of Object.keys(REGIONS)) {
    if (base === fam) return { fam, clock: "" };
    if (base.startsWith(fam + "-")) return { fam, clock: base.slice(fam.length + 1) };
  }
  return null;
}

async function report(dir) {
  const files = fs.readdirSync(dir).filter((f) => f.endsWith(".png")).sort();
  const rows = {};
  for (const f of files) {
    const fam = familyOf(f);
    if (!fam) continue;
    const m = await measure(path.join(dir, f), REGIONS[fam.fam]);
    rows[f] = m;
    console.log(`${f}  (${m.size.join("x")})`);
    for (const [k, v] of Object.entries(m.regions)) {
      console.log(`  ${k.padEnd(18)} code ${v.code.toFixed(1).padStart(6)}  lin ${v.lin.toFixed(4)}  sd ${v.sd.toFixed(2)}`);
    }
  }
  return rows;
}

// The first capture in `dir` of a family at a clock (any arm prefix), or null.
function find(dir, fam, clock, arm) {
  const want = clock ? `${fam}-${clock}.png` : `${fam}.png`;
  const files = fs.readdirSync(dir).filter((f) => f.endsWith(want) && (!arm || f.startsWith(arm + "__")));
  return files.length ? path.join(dir, files[0]) : null;
}

async function gate(beforeDir, afterDir, arm) {
  const fails = [];
  const need = (dir, fam, clock) => {
    const f = find(dir, fam, clock, arm);
    if (!f) {
      console.error(`missing capture: ${fam}${clock ? "-" + clock : ""} in ${dir}`);
      process.exit(2);
    }
    return f;
  };
  const R = REGIONS["home-racks"];
  const bNoon = (await measure(need(beforeDir, "home-racks", "noon"), R)).regions;
  const aNoon = (await measure(need(afterDir, "home-racks", "noon"), R)).regions;
  const aNight = (await measure(need(afterDir, "home-racks", "night"), R)).regions;
  const shelfCap = aNight.shelf1_low.lin * 1.1;
  console.log(`lowest shelf top: before ${bNoon.shelf1_low.code.toFixed(1)} (lin ${bNoon.shelf1_low.lin.toFixed(4)}), ` +
    `after ${aNoon.shelf1_low.code.toFixed(1)} (lin ${aNoon.shelf1_low.lin.toFixed(4)}), night ${aNight.shelf1_low.code.toFixed(1)} (lin ${aNight.shelf1_low.lin.toFixed(4)}); cap lin ${shelfCap.toFixed(4)}`);
  if (!(aNoon.shelf1_low.lin <= shelfCap)) fails.push("the lowest shelf top at noon is brighter than night + 10%: the sun still reaches it");
  const dFloor = aNoon.floor.code - bNoon.floor.code;
  console.log(`open floor at noon: before ${bNoon.floor.code.toFixed(1)}, after ${aNoon.floor.code.toFixed(1)} (${dFloor >= 0 ? "+" : ""}${dFloor.toFixed(2)} codes, want within 2)`);
  if (!(Math.abs(dFloor) <= 2)) fails.push(`the open floor moved ${dFloor.toFixed(2)} codes`);
  const hutA = find(afterDir, "planet-built-inside", "", arm);
  if (hutA) {
    const hut = (await measure(hutA, REGIONS["planet-built-inside"])).regions;
    const ratio = hut.floor_under_roof.lin / Math.max(hut.sand_sun.lin, 1e-9);
    console.log(`planet hut: floor under roof lin ${hut.floor_under_roof.lin.toFixed(4)}, sand in sun lin ${hut.sand_sun.lin.toFixed(4)}, ratio ${ratio.toFixed(3)} (want < 0.5)`);
    if (!(ratio < 0.5)) fails.push("the hut floor under its roof is not shaded");
  }
  if (fails.length) {
    console.error("\nFAIL");
    for (const f of fails) console.error("  " + f);
    process.exit(1);
  }
  console.log("\nPASS");
}

const args = process.argv.slice(2);
if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, "/")}` || true) {
  if (args[0] === "--gate") {
    const armAt = args.indexOf("--arm");
    await gate(args[1], args[2], armAt >= 0 ? args[armAt + 1] : null);
  } else if (args.length) {
    for (const d of args) await report(d);
  } else {
    console.error("usage: node scripts/home-shadow-metrics.mjs <dir>... | --gate <before-dir> <after-dir> [--arm NAME]");
    process.exit(2);
  }
}
