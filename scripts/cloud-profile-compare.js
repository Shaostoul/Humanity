// Cloud PROFILE atlas comparison (perf increment 4, the far rung; the G1 /
// G4 pass bars of docs/design/cloud-far-rung.md).
//
//   node scripts/cloud-profile-compare.js <dumpA-dir> <dumpB-dir> [--levels=0-5] [--global]
//     Per level, |delta f| between two atlas dumps (debug/cloud_profile_L<L>_s<p>.png,
//     the pair slices p = 0..5 hold (f_k, G_k, f_k+1, G_k+1) in RGBA, f in R and B):
//     the mean, the p90 and a 10-bin histogram (0.1 wide) of |delta f| over every
//     texel and bin, plus the same for |delta G| (G and A). Bars: mean under 0.03,
//     p90 under 0.10, no bin above the fifth holding more than 2 percent of texels.
//     --global adds the three global slices (cloud_profile_global_0/1.png pairs).
//     Texels whose alpha-encoded row is void in BOTH dumps (all four channels 0)
//     are skipped so unbaked rows do not read as agreement.
//
//   node scripts/cloud-profile-compare.js --grad=<level-map.png> --cx=<x> --cy=<y> [--steps=<lodb steps px, comma list>]
//     The G2 non-radial instrument on a map_diag 11 render (level / 6 as greyscale):
//     along the four cardinal lines through the nadir (cx, cy), the gradient in LEVEL
//     units between adjacent pixels; reports the max step, the count of steps above
//     0.5 level (excluding the pixels named in --steps, the expected lodb steps), and
//     a 2D histogram of (gx, gy) in level units over the whole frame (7 x 7 bins from
//     -1.5 to 1.5, per-pixel finite differences).
const sharp = require("sharp");
const fs = require("fs");
const path = require("path");

const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const a = args.find(x => x.startsWith("--" + name + "="));
  return a ? a.slice(name.length + 3) : dflt;
};
const flag = name => args.includes("--" + name);
const positional = args.filter(a => !a.startsWith("--"));

async function raw(f) {
  const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
  return { data, W: info.width, H: info.height, C: info.channels };
}

// |delta| statistics over a list of samples: mean, p90, 10-bin histogram.
function stats(samples) {
  if (samples.length === 0) return null;
  const sorted = Float32Array.from(samples).sort();
  const mean = sorted.reduce((a, b) => a + b, 0) / sorted.length;
  const p90 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.9))];
  const hist = new Array(10).fill(0);
  for (const v of sorted) hist[Math.min(9, Math.floor(v * 10))]++;
  return { n: sorted.length, mean, p90, hist: hist.map(h => h / sorted.length) };
}

function fmt(label, s) {
  if (!s) return label.padEnd(10) + " (no texels)";
  const bars = s.hist.map(h => (100 * h).toFixed(1).padStart(5)).join(" ");
  const tailBad = s.hist.slice(5).some(h => h > 0.02);
  return (
    label.padEnd(10) +
    ` n=${String(s.n).padStart(8)} mean=${s.mean.toFixed(4)} p90=${s.p90.toFixed(4)}  hist%(0.1 bins): ${bars}` +
    `  ${s.mean < 0.03 && s.p90 < 0.1 && !tailBad ? "PASS" : "FAIL"}`
  );
}

// Compare one pair of PNGs: channels (ra, rb) hold f, (ga, gb) hold G.
async function comparePair(fa, fb, fCh, gCh) {
  const A = await raw(fa), B = await raw(fb);
  if (A.W !== B.W || A.H !== B.H) throw new Error(`size mismatch ${fa} vs ${fb}`);
  const df = [], dg = [];
  for (let i = 0; i < A.W * A.H; i++) {
    const o = i * A.C;
    let voidA = true, voidB = true;
    for (let c = 0; c < 4; c++) { if (A.data[o + c]) voidA = false; if (B.data[o + c]) voidB = false; }
    if (voidA && voidB) continue;
    for (const c of fCh) df.push(Math.abs(A.data[o + c] - B.data[o + c]) / 255);
    for (const c of gCh) dg.push(Math.abs(A.data[o + c] - B.data[o + c]) / 255);
  }
  return { df, dg };
}

async function compareDumps(dirA, dirB) {
  const range = opt("levels", "0-5").split("-").map(Number);
  console.log(`A = ${dirA}\nB = ${dirB}`);
  let anyFail = false;
  for (let L = range[0]; L <= (range[1] ?? range[0]); L++) {
    const df = [], dg = [];
    for (let p = 0; p < 6; p++) {
      const name = `cloud_profile_L${L}_s${p}.png`;
      const fa = path.join(dirA, name), fb = path.join(dirB, name);
      if (!fs.existsSync(fa) || !fs.existsSync(fb)) { console.log(`L${L} s${p}: missing`); continue; }
      const r = await comparePair(fa, fb, [0, 2], [1, 3]);
      for (const v of r.df) df.push(v); for (const v of r.dg) dg.push(v); // no spread: a million samples overflows the call stack
    }
    const sf = stats(df), sg = stats(dg);
    console.log(fmt(`L${L} |df|`, sf));
    console.log(fmt(`L${L} |dG|`, sg));
    if (sf && !(sf.mean < 0.03 && sf.p90 < 0.1 && !sf.hist.slice(5).some(h => h > 0.02))) anyFail = true;
  }
  if (flag("global")) {
    const df = [], dg = [];
    for (const tag of ["0", "1"]) {
      const name = `cloud_profile_global_${tag}.png`;
      const fa = path.join(dirA, name), fb = path.join(dirB, name);
      if (!fs.existsSync(fa) || !fs.existsSync(fb)) { console.log(`global ${tag}: missing`); continue; }
      const r = await comparePair(fa, fb, [0, 2], [1, 3]);
      for (const v of r.df) df.push(v); for (const v of r.dg) dg.push(v); // no spread: a million samples overflows the call stack
    }
    console.log(fmt("global |df|", stats(df)));
    console.log(fmt("global |dG|", stats(dg)));
  }
  console.log(anyFail ? "RESULT: FAIL" : "RESULT: PASS (window levels)");
}

// The G2 gradient instrument on a channel-11 render.
async function gradient(file) {
  const cx = +opt("cx", "1280"), cy = +opt("cy", "693");
  const steps = new Set(opt("steps", "").split(",").filter(Boolean).map(Number));
  const g = await sharp(file).greyscale().raw().toBuffer({ resolveWithObject: true });
  const W = g.info.width, H = g.info.height, d = g.data;
  const lvl = (x, y) => (d[y * W + x] / 255) * 6; // channel 11 = level / 6
  const lines = [
    { name: "east", pts: Array.from({ length: W - cx - 1 }, (_, i) => [cx + i, cy]) },
    { name: "west", pts: Array.from({ length: cx }, (_, i) => [cx - i, cy]) },
    { name: "south", pts: Array.from({ length: H - cy - 1 }, (_, i) => [cx, cy + i]) },
    { name: "north", pts: Array.from({ length: cy }, (_, i) => [cx, cy - i]) },
  ];
  console.log(`${file}  nadir (${cx},${cy})  ${W}x${H}`);
  let worst = 0;
  for (const ln of lines) {
    let max = 0, over = 0, where = -1;
    for (let i = 1; i < ln.pts.length; i++) {
      const [x0, y0] = ln.pts[i - 1], [x1, y1] = ln.pts[i];
      if (steps.has(i)) continue; // an expected lodb step at this distance
      const s = Math.abs(lvl(x1, y1) - lvl(x0, y0));
      if (s > max) { max = s; where = i; }
      if (s > 0.5) over++;
    }
    worst = Math.max(worst, max);
    console.log(`  ${ln.name.padEnd(6)} max step ${max.toFixed(3)} level at ${where} px, steps > 0.5: ${over}`);
  }
  // 2D histogram of (gx, gy) in level units, 7 x 7 bins over [-1.5, 1.5].
  const bins = 7, lo = -1.5, hi = 1.5, hist = Array.from({ length: bins }, () => new Array(bins).fill(0));
  let n = 0;
  for (let y = 0; y < H - 1; y++) for (let x = 0; x < W - 1; x++) {
    const gx = lvl(x + 1, y) - lvl(x, y), gy = lvl(x, y + 1) - lvl(x, y);
    const bx = Math.min(bins - 1, Math.max(0, Math.floor((gx - lo) / (hi - lo) * bins)));
    const by = Math.min(bins - 1, Math.max(0, Math.floor((gy - lo) / (hi - lo) * bins)));
    hist[by][bx]++; n++;
  }
  console.log("  2D histogram of (gx, gy), percent of pixels, rows = gy from -1.5 to 1.5, cols = gx:");
  for (const row of hist) console.log("   " + row.map(v => (100 * v / n).toFixed(2).padStart(7)).join(""));
  console.log(`  RESULT: ${worst <= 0.5 ? "PASS" : "FAIL"} (max cardinal step ${worst.toFixed(3)} level, bar 0.5)`);
}

(async () => {
  const grad = opt("grad", null);
  if (grad) return gradient(grad);
  if (positional.length < 2) {
    console.log("usage: node scripts/cloud-profile-compare.js <dumpA-dir> <dumpB-dir> [--levels=0-5] [--global]\n" +
      "       node scripts/cloud-profile-compare.js --grad=<level-map.png> --cx=1280 --cy=693 [--steps=...]");
    process.exit(2);
  }
  await compareDumps(positional[0], positional[1]);
})().catch(e => { console.error(e.message || e); process.exit(1); });
