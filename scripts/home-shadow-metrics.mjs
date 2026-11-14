// Region-of-interest metrics for the sun-cascade captures: does a home (or a
// planet hut) actually shade the places a shelf or a roof covers?
//
// Usage:
//   node scripts/home-shadow-metrics.mjs <dir> [<dir> ...]
//       every PNG in each dir whose name holds a known vantage id is measured
//       region by region (mean sRGB luma code, mean linear luma, std dev).
//   node scripts/home-shadow-metrics.mjs --gate <before-dir> <after-dir> [--arm A] [--before-arm B]
//       increment 1's acceptance (docs/design/sun-cascades.md section 4),
//       "before" being the same views WITHOUT the near world's sun shadows
//       (main before the shadow atlas, or showcase local_shadows "0") and
//       "after" WITH them:
//       (a) the lowest shelf top at noon keeps at most a quarter of the light
//           it has without shadows (linear): the sun term is gone, and what
//           is left is the sky's and the room's bounced light;
//       (b) the open floor at noon moved by at most 2 sRGB codes;
//       (c) the planet hut's floor under its roof keeps less than half the
//           light it has without shadows (linear).
//       --arm / --before-arm pick the probe-hot-ab arm in each dir (one dir
//       holding both arms of a same-boot A/B: --gate D D --arm on
//       --before-arm off). Exit 1 when a gate fails, 2 when a capture is
//       missing.
//   node scripts/home-shadow-metrics.mjs --yards <dir> [--arm A]
//       the yards gate (2026-11-14): at noon a neighbour's open yard reads no
//       more than YARD_GAP_MAX codes darker than your own under the same sky
//       (tests/visual/row-yards-vantages.json, your home on r1). Exit 1 when
//       it does, 2 when the capture is missing.
//
// WHY THESE CRITERIA (corrected 2026-11-12, shadows rung 1b). The first
// version capped (a) at the NIGHT capture plus 10% and judged (c) against the
// sand beside the hut. Both went stale once the shadow atlas made the shelves
// and the hut actually shaded: the lowest shelf fell from 159 to 55 codes
// (linear 0.376 to 0.047) and still failed the night cap, because what is left
// is the room's bounced light (with showcase room_gi "0" the shelf reads
// 0.0039 against the night's 0.0009), which no shadow map occludes and which
// the night capture, with the sun down, does not have; and the hut vantage now
// looks at the hut's inside wall with no sand in frame. So both now compare
// the SAME region with and without the shadows: a ratio of light kept, the
// thing a shadow changes, instead of a reference that measures something
// else. Seen red (2026-11-12) with the after arm also shadow-less (the off
// arm against itself: shelf and hut both keep about all their light) and
// green with the atlas on; the numbers are in docs/design/local-light-shadows.md
// section 10.
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
  // The same pose with the tents' doors down (2026-11-13, tests/visual/vantages.json film-racks-*):
  // the shelf boxes look at the shelves THROUGH a tent's frosted front, the partial casters' and the
  // blur's A/B (renderer/shadow_transmit.rs, renderer/film_blur.rs).
  "film-racks": {
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
  // r1's yard looking south along the terrace's yards (tests/visual/row-yards-vantages.json, the
  // two-bedroom dev start on r1): your own yard's floor in the foreground, and the neighbour r2's
  // yard floor beyond the mowing strip, right of its first bed and clear of the strip and the house's
  // back wall. Read off the 2026-11-14 captures at 2560x1387 (the r3-sun base and nonear arms).
  "row-yards": {
    own_yard: [625, 616, 937, 681],
    neighbour_yard: [1034, 539, 1078, 555],
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

// The most of its unshadowed light the lowest shelf top may keep at noon (gate a), and the hut's
// floor under its roof (gate c). Measured 2026-11-11/12: the shelf keeps 0.125 (0.047 of 0.376), the
// hut floor 0.125 (0.075 of 0.60); without shadows both keep about 1.0.
export const SHELF_KEEP_MAX = 0.25;
export const HUT_KEEP_MAX = 0.5;

async function gate(beforeDir, afterDir, arm, beforeArm) {
  const fails = [];
  const need = (dir, fam, clock, which) => {
    const f = find(dir, fam, clock, which);
    if (!f) {
      console.error(`missing capture: ${fam}${clock ? "-" + clock : ""}${which ? ` (arm ${which})` : ""} in ${dir}`);
      process.exit(2);
    }
    return f;
  };
  const R = REGIONS["home-racks"];
  const bNoon = (await measure(need(beforeDir, "home-racks", "noon", beforeArm), R)).regions;
  const aNoon = (await measure(need(afterDir, "home-racks", "noon", arm), R)).regions;
  const shelfKeep = aNoon.shelf1_low.lin / Math.max(bNoon.shelf1_low.lin, 1e-9);
  console.log(`lowest shelf top at noon: before ${bNoon.shelf1_low.code.toFixed(1)} (lin ${bNoon.shelf1_low.lin.toFixed(4)}), ` +
    `after ${aNoon.shelf1_low.code.toFixed(1)} (lin ${aNoon.shelf1_low.lin.toFixed(4)}): keeps ${shelfKeep.toFixed(3)} of its light (want at most ${SHELF_KEEP_MAX})`);
  if (!(shelfKeep <= SHELF_KEEP_MAX)) fails.push(`the lowest shelf top at noon keeps ${shelfKeep.toFixed(3)} of its light: the sun still reaches it`);
  const dFloor = aNoon.floor.code - bNoon.floor.code;
  console.log(`open floor at noon: before ${bNoon.floor.code.toFixed(1)}, after ${aNoon.floor.code.toFixed(1)} (${dFloor >= 0 ? "+" : ""}${dFloor.toFixed(2)} codes, want within 2)`);
  if (!(Math.abs(dFloor) <= 2)) fails.push(`the open floor moved ${dFloor.toFixed(2)} codes`);
  const P = REGIONS["planet-built-inside"];
  const hutB = (await measure(need(beforeDir, "planet-built-inside", "", beforeArm), P)).regions;
  const hutA = (await measure(need(afterDir, "planet-built-inside", "", arm), P)).regions;
  const hutKeep = hutA.floor_under_roof.lin / Math.max(hutB.floor_under_roof.lin, 1e-9);
  console.log(`planet hut floor under its roof: before lin ${hutB.floor_under_roof.lin.toFixed(4)}, after lin ${hutA.floor_under_roof.lin.toFixed(4)}: keeps ${hutKeep.toFixed(3)} (want under ${HUT_KEEP_MAX})`);
  if (!(hutKeep < HUT_KEEP_MAX)) fails.push(`the hut floor under its roof keeps ${hutKeep.toFixed(3)} of its light: it is not shaded`);
  if (fails.length) {
    console.error("\nFAIL");
    for (const f of fails) console.error("  " + f);
    process.exit(1);
  }
  console.log("\nPASS");
}

// The most codes a neighbour's open yard may read darker than your own at noon (the yards gate). Your
// yard and a neighbour's stand under the same glass and the same sun, so they should read alike;
// 8 codes leaves room for the room light's small differences between plots, and is far under the
// defect (about 120 codes: 80 against 199 on 2026-11-14).
export const YARD_GAP_MAX = 8;

// THE YARDS GATE (2026-11-14): at noon a neighbour's open yard reads no darker than your own, more
// than YARD_GAP_MAX codes. Seen red on the capture of main as it stood (73b817e7a, the r3-sun base
// arm: "your own yard 199.8, the neighbour's 82.6 (darker by 117.2 codes)", exit 1), on the build
// that made only the neighbour's roof glass (its glass arm read the same 82.6: the hull's plating,
// shut over the neighbours, shaded the yard too; BUG-307), and on the red twin's arm (opaque__,
// showcase neighbour_glass 0); the same view without the near sun view passes (neighbour 202.2).
async function yardsGate(dir, arm) {
  const f = find(dir, "row-yards", "noon", arm);
  if (!f) {
    console.error(`missing capture: row-yards-noon${arm ? ` (arm ${arm})` : ""} in ${dir}`);
    process.exit(2);
  }
  const r = (await measure(f, REGIONS["row-yards"])).regions;
  const gap = r.own_yard.code - r.neighbour_yard.code;
  console.log(`${path.basename(f)}: your own yard ${r.own_yard.code.toFixed(1)}, the neighbour's ${r.neighbour_yard.code.toFixed(1)} ` +
    `(${gap >= 0 ? "darker" : "lighter"} by ${Math.abs(gap).toFixed(1)} codes; it may be at most ${YARD_GAP_MAX} darker)`);
  if (!(gap <= YARD_GAP_MAX)) {
    console.error(`\nFAIL\n  the neighbour's open yard reads ${gap.toFixed(1)} codes darker than your own under the same sky`);
    process.exit(1);
  }
  console.log("\nPASS");
}

const args = process.argv.slice(2);
if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, "/")}` || true) {
  if (args[0] === "--yards") {
    const armAt = args.indexOf("--arm");
    await yardsGate(args[1], armAt >= 0 ? args[armAt + 1] : null);
  } else if (args[0] === "--gate") {
    const armAt = args.indexOf("--arm");
    const beforeArmAt = args.indexOf("--before-arm");
    const arm = armAt >= 0 ? args[armAt + 1] : null;
    // The before dir's arm defaults to the after dir's (two dirs, one arm name each, as before).
    await gate(args[1], args[2], arm, beforeArmAt >= 0 ? args[beforeArmAt + 1] : arm);
  } else if (args.length) {
    for (const d of args) await report(d);
  } else {
    console.error("usage: node scripts/home-shadow-metrics.mjs <dir>... | --gate <before-dir> <after-dir> [--arm NAME] [--before-arm NAME] | --yards <dir> [--arm NAME]");
    process.exit(2);
  }
}
