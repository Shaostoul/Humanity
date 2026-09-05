// Radial-coherence metric for the nadir fan (design 1c, v0.1275).
//
// The spoke metric high-passes the angular profile of radial MEANS, so it is
// blind to radial ELONGATION and it is content-weighted. This one measures the
// thing the residual hunt named: edges whose tangent runs ALONG the radius from
// a centre. Greyscale, 4x downsample, Sobel; for pixels with |g| > 24, psi =
// angle(edge tangent) - angle(radial); A = sum |g| cos(2 psi) / sum |g| over
// radius bins. +1 = spokes, 0 = isotropic, -1 = rings. Control centres are
// always printed (CX +- 640, CY +- 400) so a nadir reading has something to be
// compared against, and --search grids the frame for the coherence peak (how
// the lean test is read: the peak should move off the nadir).
//
// Sliver census: alpha > 128 components, PCA; a sliver = aspect > 4, minor axis
// < 40 px (full-res), major axis within 10 deg of radial. This is the vantage
// regression line "no sharp radial slivers" as a number.
//
//   node scripts/cloud-radial-coherence.js <png> [CX] [CY] [--search]
// Nadir pixel for the rig 2560x1387 captures (fovy 90.05): f = 693 px,
// CY = 693.5 + 693 * tan(look_offset): 730 at 3 deg, 841 at 12 deg, 1275 at 40.
const sharp = require("sharp");
const f = process.argv[2];
const CX0 = +(process.argv[3] || 1280), CY0 = +(process.argv[4] || 841);
const SEARCH = process.argv.includes("--search");
const BINS = [[40, 160], [160, 320], [320, 480], [480, 640], [640, 800], [800, 1000], [1000, 1300]];

async function load() {
  const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true });
  return { data, W: info.width, H: info.height };
}
function down4(img) {
  const W = Math.floor(img.W / 4), H = Math.floor(img.H / 4), out = new Float32Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    let s = 0; for (let j = 0; j < 4; j++) for (let i = 0; i < 4; i++) s += img.data[(y * 4 + j) * img.W + x * 4 + i];
    out[y * W + x] = s / 16;
  }
  return { d: out, W, H };
}
function sobel(g) {
  const { d, W, H } = g; const gx = new Float32Array(W * H), gy = new Float32Array(W * H);
  for (let y = 1; y < H - 1; y++) for (let x = 1; x < W - 1; x++) {
    const i = y * W + x;
    gx[i] = (d[i - W + 1] + 2 * d[i + 1] + d[i + W + 1]) - (d[i - W - 1] + 2 * d[i - 1] + d[i + W - 1]);
    gy[i] = (d[i + W - 1] + 2 * d[i + W] + d[i + W + 1]) - (d[i - W - 1] + 2 * d[i - W] + d[i - W + 1]);
  }
  return { gx, gy, W, H };
}
function coherence(sb, cx, cy) {
  const { gx, gy, W, H } = sb; const cx4 = cx / 4, cy4 = cy / 4;
  const num = BINS.map(() => 0), den = BINS.map(() => 0), cnt = BINS.map(() => 0);
  for (let y = 1; y < H - 1; y++) for (let x = 1; x < W - 1; x++) {
    const i = y * W + x; const m = Math.hypot(gx[i], gy[i]); if (m < 24 * 4) continue;
    const r4 = Math.hypot(x - cx4, y - cy4); const r = r4 * 4;
    let b = -1; for (let k = 0; k < BINS.length; k++) if (r >= BINS[k][0] && r < BINS[k][1]) b = k;
    if (b < 0) continue;
    // The centre may sit at or beyond the frame edge (a steep look puts the
    // nadir below the bottom row); the pixel loop is already in bounds, so
    // partial annuli are simply measured over the pixels that exist.
    // edge tangent is perpendicular to the gradient; radial from centre
    const tang = Math.atan2(gx[i], -gy[i]); const rad = Math.atan2(y - cy4, x - cx4);
    const psi = tang - rad;
    num[b] += m * Math.cos(2 * psi); den[b] += m; cnt[b]++;
  }
  // A bin with no strong edges is EMPTY, not coherent: a uniform fog frame
  // must not read as a cured rosette (2026-09-05, the vacuous 0.00 lesson).
  return BINS.map((_, k) => ({ A: den[k] > 0 ? num[k] / den[k] : NaN, n: cnt[k] }));
}
function fmt(A) {
  return A.map(r => {
    const v = typeof r === "number" ? r : r.A, n = typeof r === "number" ? null : r.n;
    if (Number.isNaN(v)) return " empty".padEnd(12);
    const core = (v >= 0 ? "+" : "") + v.toFixed(2);
    return (n === null ? core : core + "(" + n + ")").padEnd(12);
  }).join(" ");
}
function slivers(img, cx, cy) {
  // components of alpha > 128 on the 4x grid; PCA per component
  const g = down4(img); const { d, W, H } = g; const lab = new Int32Array(W * H).fill(-1); let n = 0;
  const comps = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x; if (d[i] <= 128 || lab[i] >= 0) continue;
    const st = [i]; lab[i] = n; const pts = [];
    while (st.length) { const j = st.pop(); pts.push(j); const jx = j % W, jy = (j - jx) / W;
      for (const [dx, dy] of [[1,0],[-1,0],[0,1],[0,-1]]) { const nx = jx + dx, ny = jy + dy; if (nx < 0 || ny < 0 || nx >= W || ny >= H) continue; const k = ny * W + nx; if (d[k] > 128 && lab[k] < 0) { lab[k] = n; st.push(k); } } }
    comps.push(pts); n++;
  }
  let count = 0, area = 0; const total = W * H;
  for (const pts of comps) {
    if (pts.length < 6) continue;
    let mx = 0, my = 0; for (const j of pts) { mx += j % W; my += Math.floor(j / W); } mx /= pts.length; my /= pts.length;
    let sxx = 0, syy = 0, sxy = 0; for (const j of pts) { const dx = j % W - mx, dy = Math.floor(j / W) - my; sxx += dx * dx; syy += dy * dy; sxy += dx * dy; }
    sxx /= pts.length; syy /= pts.length; sxy /= pts.length;
    const tr = sxx + syy, det = sxx * syy - sxy * sxy; const disc = Math.sqrt(Math.max(tr * tr / 4 - det, 0));
    const l1 = tr / 2 + disc, l2 = Math.max(tr / 2 - disc, 1e-6);
    const major = 4 * Math.sqrt(l1) * 4, minor = 4 * Math.sqrt(l2) * 4; // full-res px, ~4 sigma
    const ang = 0.5 * Math.atan2(2 * sxy, sxx - syy);
    const rad = Math.atan2(my - cy / 4, mx - cx / 4);
    let dpsi = Math.abs(((ang - rad) + Math.PI) % Math.PI); dpsi = Math.min(dpsi, Math.PI - dpsi);
    if (major / minor > 4 && minor < 40 && dpsi < 10 * Math.PI / 180) { count++; area += pts.length; }
  }
  return { count, areaFrac: area / total };
}
(async () => {
  const img = await load(); const sb = sobel(down4(img));
  console.log("bins(px): " + BINS.map(b => b[0] + "-" + b[1]).join(" "));
  console.log("  nadir (" + CX0 + "," + CY0 + "):  " + fmt(coherence(sb, CX0, CY0)));
  for (const [cx, cy] of [[CX0 - 640, CY0], [CX0 + 640, CY0], [CX0, CY0 - 400], [CX0, CY0 + 400]]) {
    if (cx < 200 || cx > img.W - 200 || cy < 200 || cy > img.H - 200) continue;
    console.log("  ctrl  (" + cx + "," + cy + "):  " + fmt(coherence(sb, cx, cy)));
  }
  const sv = slivers(img, CX0, CY0);
  console.log("  slivers: " + sv.count + "  area " + (100 * sv.areaFrac).toFixed(2) + "%");
  if (SEARCH) {
    let best = null;
    for (let cy = 200; cy <= img.H - 200; cy += 64) for (let cx = 200; cx <= img.W - 200; cx += 64) {
      const A = coherence(sb, cx, cy); const m = (A[1] + A[2] + A[3] + A[4]) / 4;
      if (!best || m > best.m) best = { cx, cy, m };
    }
    console.log("  search peak: (" + best.cx + "," + best.cy + ") mean A(bins 2-5) = " + best.m.toFixed(3));
  }
})();
