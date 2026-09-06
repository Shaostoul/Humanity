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
//   node scripts/cloud-profile-compare.js --grad=<ch11.png> --ch10=<ch10.png> --cx=<x> --cy=<y> [--steps=<px, comma list>] [--floor=0.02]
//     The G2 non-radial instrument, per PIXEL: the level map is 6 lin(ch11) / lin(ch10)
//     (channel 11 is level / 6 accumulated with the colour's own weights, so it must be
//     divided by channel 10, the same accumulation of w_pf; both captures leave through
//     the sRGB swapchain and are DECODED to linear first: on raw bytes every level reads
//     L + r (6 - L) with r about 0.5, a 50 percent global share that does not exist).
//     Along the four cardinal lines through the nadir (cx, cy), the step in LEVEL units
//     between adjacent pixels whose lin(ch10) is at or above --floor (8-bit quantisation
//     makes the ratio meaningless below about 0.02 accumulated alpha); reports the max
//     step, the count of steps above 0.5 level (excluding the distances named in
//     --steps, the expected lodb steps), and a 2D histogram of (gx, gy) in level units
//     over the whole frame (7 x 7 bins from -1.5 to 1.5, per-pixel finite differences).
//
//   node scripts/cloud-profile-compare.js --cardinal=<ch11.png> --ch10=<ch10.png> [--hard10=<ch10 HARD.png>]
//                                         [--cx=1280] [--cy=693] [--bin=10] [--floor=0.02] [--steps=...] [--dump]
//     The D1 gate's G2 instrument, per BIN: the same decoded level map 6 lin(ch11) /
//     lin(ch10), averaged over 10 px bins (--bin) along the four cardinal lines through
//     the nadir (a bin is void when fewer than half its pixels clear the alpha floor);
//     VALID ONLY WHERE w_pf == 1 (873 km and above at cloud_res 1): the shader
//     accumulates channel 11 as trans * a_i * level / 6 WITHOUT the w_pf factor while
//     channel 10 carries trans * a_i * w_pf, so where w_pf < 1 the quotient is inflated
//     by 1 / w_pf and the printed "level" is not a level. At the D1 gate camera the
//     premise was checked: lin(prof-ch10-873-r1) against lin(orbit-speckle-873-r1-mask)
//     (same camera, same pins) differs by 0.014 mean, 0.018 rms, no pixel over 0.1, so
//     channel 10 IS the accumulated alpha there. Do not run it at 250 km or below;
//     prints the max step between ADJACENT bins per line, where it sits (px from the
//     nadir), the count of steps above 0.5 level, and PASS / FAIL at the 0.5 bar.
//     With --hard10 (the map_diag 10 capture of the same camera at knob 8, cell
//     prof-ch10-873-r1-hard) it also prints the same-pixel BAND EXCESS per bin,
//     lin(ch10_auto) / lin(ch10_hard): at 873 km w_pf is identically 1 on both arms so
//     channel 10 IS each arm's accumulated alpha, and the ratio is a pure coverage
//     ratio with no level arithmetic in it (the critic's R1: the L_hard / L_auto form
//     was the constant 1). Reported, not barred: the per-level estimator floor is
//     measured, not asserted. --dump lists every bin. Both modes skip the first --r0 px
//     (default 10) from the nadir: the HUD crosshair sits on the nadir pixel and reads as
//     a false 3.5-level tread in the innermost bin (proven on the G0 captures).
//
//   node scripts/cloud-profile-compare.js --holes=<bit-off mask.png> --vs=<bit-on mask.png> [--t=128] [--crop=0.8]
//     The D3 slant check (prof-vert-250-r1-prod-look40 against -fix-look40): pixels white
//     in the bit-off mask and black in the bit-on mask are LOST (a cloud the from-above
//     stride stepped over sideways), the reverse are GAINED (the thin layer found).
//     Prints both as points of frame and as a share of the bit-off mask, and the largest
//     lost 8-connected blob in px; PASS / FAIL at lost under 1 point AND largest blob
//     under 400 px (reported for the orchestrator, the script decides nothing).
const sharp = require("sharp");

// sRGB transfer, decoded per byte. EVERY diag-channel capture (map_diag 1, 4,
// 10, 11, 12) is written through the sRGB swapchain (src/renderer/mod.rs picks
// the first sRGB surface format and the scene texture is created with it), so
// a ratio of two captures, or of two pixels, must be formed on LINEAR values.
// The atlas DUMPS (compareDumps below) are raw RGBA8 texture reads and are NOT
// decoded: f and G are stored linearly there.
const LIN = new Float32Array(256);
for (let i = 0; i < 256; i++) {
  const c = i / 255;
  LIN[i] = c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}
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

// The decoded level map: 6 lin(ch11) / lin(ch10) per pixel, NaN where the
// accumulated alpha lin(ch10) is under the floor (no profile share to read a
// level from, or too little for 8 bits to carry a ratio). Both captures are
// map_diag renders of the SAME camera in the same sweep (ch11 = level / 6,
// ch10 = w_pf, each accumulated with the colour's own weights trans * a_i).
async function levelMap(ch11File, ch10File, floor) {
  const a = await sharp(ch11File).greyscale().raw().toBuffer({ resolveWithObject: true });
  const b = await sharp(ch10File).greyscale().raw().toBuffer({ resolveWithObject: true });
  if (a.info.width !== b.info.width || a.info.height !== b.info.height) throw new Error("ch11 / ch10 size mismatch");
  const W = a.info.width, H = a.info.height, n = W * H;
  const lvl = new Float32Array(n), alpha = new Float32Array(n);
  let valid = 0;
  for (let i = 0; i < n; i++) {
    const w = LIN[b.data[i]];
    alpha[i] = w;
    if (w >= floor) { lvl[i] = (6 * LIN[a.data[i]]) / w; valid++; } else lvl[i] = NaN;
  }
  return { W, H, lvl, alpha, valid };
}

// The four cardinal lines through (cx, cy), each as a list of pixels ordered
// by distance from the nadir (index = px from the nadir).
function cardinalLines(W, H, cx, cy) {
  return [
    { name: "east", pts: Array.from({ length: W - cx }, (_, i) => [cx + i, cy]) },
    { name: "west", pts: Array.from({ length: cx + 1 }, (_, i) => [cx - i, cy]) },
    { name: "south", pts: Array.from({ length: H - cy }, (_, i) => [cx, cy + i]) },
    { name: "north", pts: Array.from({ length: cy + 1 }, (_, i) => [cx, cy - i]) },
  ];
}

// The G2 gradient instrument, per pixel, on the DECODED level map.
async function gradient(file) {
  const ch10 = opt("ch10", null);
  if (!ch10) throw new Error("--grad needs --ch10=<map_diag 10 capture of the same camera>: channel 11 is level / 6 times the profile share and must be divided by channel 10 (and both sRGB-decoded) before it is a level");
  const cx = +opt("cx", "1280"), cy = +opt("cy", "693"), floor = +opt("floor", "0.02"), r0 = +opt("r0", "10");
  const steps = new Set(opt("steps", "").split(",").filter(Boolean).map(Number));
  const { W, H, lvl, valid } = await levelMap(file, ch10, floor);
  const L = (x, y) => lvl[y * W + x];
  console.log(`${file} / ${ch10}  nadir (${cx},${cy})  ${W}x${H}  sRGB-decoded, alpha floor ${floor}: ${(100 * valid / (W * H)).toFixed(1)}% of pixels carry a level`);
  let worst = 0;
  for (const ln of cardinalLines(W, H, cx, cy)) {
    let max = 0, over = 0, where = -1, pairs = 0;
    for (let i = Math.max(1, r0); i < ln.pts.length; i++) { // r0: the HUD crosshair on the nadir pixel
      const [x0, y0] = ln.pts[i - 1], [x1, y1] = ln.pts[i];
      if (steps.has(i)) continue; // an expected lodb step at this distance
      const a = L(x0, y0), b = L(x1, y1);
      if (Number.isNaN(a) || Number.isNaN(b)) continue; // no share on one side: not a level step
      pairs++;
      const s = Math.abs(b - a);
      if (s > max) { max = s; where = i; }
      if (s > 0.5) over++;
    }
    worst = Math.max(worst, max);
    console.log(`  ${ln.name.padEnd(6)} max step ${max.toFixed(3)} level at ${where} px, steps > 0.5: ${over} (of ${pairs} valid pairs)`);
  }
  // 2D histogram of (gx, gy) in level units, 7 x 7 bins over [-1.5, 1.5],
  // over pixels whose three-pixel stencil all carries a level.
  const bins = 7, lo = -1.5, hi = 1.5, hist = Array.from({ length: bins }, () => new Array(bins).fill(0));
  let n = 0;
  for (let y = 0; y < H - 1; y++) for (let x = 0; x < W - 1; x++) {
    const c = L(x, y), r = L(x + 1, y), d = L(x, y + 1);
    if (Number.isNaN(c) || Number.isNaN(r) || Number.isNaN(d)) continue;
    const gx = r - c, gy = d - c;
    const bx = Math.min(bins - 1, Math.max(0, Math.floor((gx - lo) / (hi - lo) * bins)));
    const by = Math.min(bins - 1, Math.max(0, Math.floor((gy - lo) / (hi - lo) * bins)));
    hist[by][bx]++; n++;
  }
  console.log("  2D histogram of (gx, gy), percent of pixels, rows = gy from -1.5 to 1.5, cols = gx:");
  for (const row of hist) console.log("   " + row.map(v => (100 * v / Math.max(n, 1)).toFixed(2).padStart(7)).join(""));
  console.log(`  RESULT: ${worst <= 0.5 ? "PASS" : "FAIL"} (max cardinal step ${worst.toFixed(3)} level, bar 0.5)`);
}

// The D1 gate's G2 instrument, per 10 px bin along the cardinal lines.
async function cardinal(file) {
  const ch10 = opt("ch10", null);
  if (!ch10) throw new Error("--cardinal needs --ch10=<map_diag 10 capture of the same camera>");
  const cx = +opt("cx", "1280"), cy = +opt("cy", "693"), floor = +opt("floor", "0.02"), bin = Math.max(1, +opt("bin", "10")), r0 = +opt("r0", "10");
  const steps = opt("steps", "").split(",").filter(Boolean).map(Number);
  const dump = flag("dump");
  const { W, H, lvl, alpha, valid } = await levelMap(file, ch10, floor);
  const hard10 = opt("hard10", null);
  let hardAlpha = null;
  if (hard10) {
    const h = await sharp(hard10).greyscale().raw().toBuffer({ resolveWithObject: true });
    if (h.info.width !== W || h.info.height !== H) throw new Error("hard10 size mismatch");
    hardAlpha = new Float32Array(W * H);
    for (let i = 0; i < W * H; i++) hardAlpha[i] = LIN[h.data[i]];
  }
  console.log(`${file} / ${ch10}${hard10 ? " / hard " + hard10 : ""}  nadir (${cx},${cy})  ${W}x${H}  bins of ${bin} px from ${r0} px, sRGB-decoded, alpha floor ${floor}: ${(100 * valid / (W * H)).toFixed(1)}% of pixels carry a level`);
  let worst = 0, worstExcess = 0;
  for (const ln of cardinalLines(W, H, cx, cy)) {
    // Bin means: a bin is void when fewer than half its pixels clear the floor.
    const nb = Math.floor(ln.pts.length / bin);
    const means = new Array(nb).fill(NaN), excess = new Array(nb).fill(NaN);
    for (let k = 0; k < nb; k++) {
      let s = 0, c = 0, sa = 0, sh = 0, ch = 0;
      for (let j = 0; j < bin; j++) {
        const [x, y] = ln.pts[k * bin + j];
        const i = y * W + x;
        if (!Number.isNaN(lvl[i])) { s += lvl[i]; c++; }
        if (hardAlpha && alpha[i] >= floor && hardAlpha[i] >= floor) { sa += alpha[i]; sh += hardAlpha[i]; ch++; }
      }
      if (c * 2 >= bin) means[k] = s / c;
      if (hardAlpha && ch * 2 >= bin) excess[k] = sa / sh;
    }
    let max = 0, over = 0, where = -1, pairs = 0, emax = 0, ewhere = -1;
    for (let k = 1; k < nb; k++) {
      const d0 = (k - 1) * bin, d1 = k * bin;
      if (d0 < r0) continue; // the HUD crosshair sits on the nadir pixel: a false tread in the innermost bin
      if (steps.some(p => p >= d0 && p < d1 + bin)) continue; // an expected lodb step inside this pair
      if (!Number.isNaN(means[k - 1]) && !Number.isNaN(means[k])) {
        pairs++;
        const s = Math.abs(means[k] - means[k - 1]);
        if (s > max) { max = s; where = d1; }
        if (s > 0.5) over++;
      }
      if (!Number.isNaN(excess[k]) && excess[k] > emax) { emax = excess[k]; ewhere = d1; }
    }
    worst = Math.max(worst, max);
    worstExcess = Math.max(worstExcess, emax);
    console.log(`  ${ln.name.padEnd(6)} max bin step ${max.toFixed(3)} level at ${where} px, steps > 0.5: ${over} (of ${pairs} valid bin pairs)` +
      (hardAlpha ? `; max band excess auto/hard ${emax.toFixed(3)} at ${ewhere} px` : ""));
    if (dump) {
      console.log("    " + means.map((m, k) => Number.isNaN(m) ? "" : `${String(k * bin).padStart(5)}:${m.toFixed(2)}${hardAlpha && !Number.isNaN(excess[k]) ? "/" + excess[k].toFixed(3) : ""}`).filter(Boolean).join(" "));
    }
  }
  console.log(`  RESULT: ${worst <= 0.5 ? "PASS" : "FAIL"} (max cardinal bin step ${worst.toFixed(3)} level, bar 0.5)` +
    (hardAlpha ? `; band excess max ${worstExcess.toFixed(3)} (reported; the per-level estimator floor measured about 1.03 to 1.05)` : ""));
}

// The D3 slant check (prof-vert-250-r1-prod-look40 vs -fix-look40): the
// from-above SDF bound is the vertical gap to THIS cloud, and off-nadir a
// stride of kilometres carries the ray sideways out of the 3x3 cell
// neighbourhood the bound came from, so a taller cloud two cells away can be
// stepped over. That shows as a HOLE: a pixel white in the bit-off mask and
// black in the bit-on mask. This mode counts LOST (white in A, black in B) and
// GAINED (black in A, white in B) pixels inside the central --crop of the
// frame, each as points of frame and as a share of A's mask, and the largest
// LOST 8-connected blob in px (a stepped-over cloud is one big blob; a few
// scattered pixels are threshold flicker). Both masks are map_diag 1 renders
// thresholded on luminance at --t (128 default, the same silhouette bar as
// cloud-mask-iou.js).
async function holes(fileA) {
  const fileB = opt("vs", null);
  if (!fileB) throw new Error("--holes=<bit-off mask> needs --vs=<bit-on mask of the same camera>");
  const T = +opt("t", 128), crop = +opt("crop", 0.8);
  const a = await sharp(fileA).greyscale().raw().toBuffer({ resolveWithObject: true });
  const b = await sharp(fileB).greyscale().raw().toBuffer({ resolveWithObject: true });
  if (a.info.width !== b.info.width || a.info.height !== b.info.height) throw new Error("mask size mismatch");
  const W = a.info.width, H = a.info.height;
  const cw = Math.floor(W * crop), ch = Math.floor(H * crop);
  const x0 = Math.floor((W - cw) / 2), y0 = Math.floor((H - ch) / 2);
  const n = cw * ch;
  // lost[i] = 1 where A is white and B is black; the reverse is gained.
  const lost = new Uint8Array(n);
  let nA = 0, nB = 0, nLost = 0, nGain = 0;
  for (let y = 0; y < ch; y++) for (let x = 0; x < cw; x++) {
    const src = (y0 + y) * W + (x0 + x), i = y * cw + x;
    const wa = a.data[src] > T, wb = b.data[src] > T;
    if (wa) nA++;
    if (wb) nB++;
    if (wa && !wb) { lost[i] = 1; nLost++; }
    if (!wa && wb) nGain++;
  }
  // Largest lost blob, 8-connected, iterative flood fill on a visited copy.
  const seen = new Uint8Array(n);
  let largest = 0, blobs = 0;
  const stack = new Int32Array(n);
  for (let s = 0; s < n; s++) {
    if (!lost[s] || seen[s]) continue;
    let sp = 0, size = 0;
    stack[sp++] = s; seen[s] = 1;
    while (sp > 0) {
      const i = stack[--sp]; size++;
      const x = i % cw, y = (i - x) / cw;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        if (!dx && !dy) continue;
        const xx = x + dx, yy = y + dy;
        if (xx < 0 || yy < 0 || xx >= cw || yy >= ch) continue;
        const j = yy * cw + xx;
        if (lost[j] && !seen[j]) { seen[j] = 1; stack[sp++] = j; }
      }
    }
    blobs++;
    if (size > largest) largest = size;
  }
  const pts = v => (100 * v / n).toFixed(3);
  const share = v => nA > 0 ? (100 * v / nA).toFixed(2) : "n/a";
  console.log(`holes: A (bit off) = ${path.basename(fileA)}, B (bit on) = ${path.basename(fileB)}, crop ${crop} (${cw} x ${ch} px), t ${T}`);
  console.log(`  coverage A ${pts(nA)} points, B ${pts(nB)} points`);
  console.log(`  LOST (white in A, black in B): ${nLost} px = ${pts(nLost)} points of frame = ${share(nLost)} percent of A's mask, in ${blobs} blobs, largest ${largest} px`);
  console.log(`  GAINED (black in A, white in B): ${nGain} px = ${pts(nGain)} points of frame = ${share(nGain)} percent of A's mask`);
  console.log(`  RESULT: ${nLost <= 0.01 * n && largest < 400 ? "PASS" : "FAIL"} (bar: lost under 1 point of frame AND largest lost blob under 400 px; reported for the orchestrator's decision, the bit is not defaulted on by this script)`);
}

(async () => {
  const grad = opt("grad", null);
  if (grad) return gradient(grad);
  const card = opt("cardinal", null);
  if (card) return cardinal(card);
  const hol = opt("holes", null);
  if (hol) return holes(hol);
  if (positional.length < 2) {
    console.log("usage: node scripts/cloud-profile-compare.js <dumpA-dir> <dumpB-dir> [--levels=0-5] [--global]\n" +
      "       node scripts/cloud-profile-compare.js --grad=<ch11.png> --ch10=<ch10.png> --cx=1280 --cy=693 [--steps=...] [--floor=0.02]\n" +
      "       node scripts/cloud-profile-compare.js --cardinal=<ch11.png> --ch10=<ch10.png> [--hard10=<ch10 hard.png>] [--bin=10] [--floor=0.02] [--dump]\n" +
      "       node scripts/cloud-profile-compare.js --holes=<bit-off mask.png> --vs=<bit-on mask.png> [--t=128] [--crop=0.8]");
    process.exit(2);
  }
  await compareDumps(positional[0], positional[1]);
})().catch(e => { console.error(e.message || e); process.exit(1); });
