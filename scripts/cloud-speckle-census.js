// Cloud speckle census: connected components of "cloud-white" pixels over the
// central 80% of the frame (HUD excluded), binned by area. A speckle is a
// small bright component: from orbit the point-sampled constructed bodies
// print as thousands of 1..16 px islands, while a prefiltered field prints as
// a few hundred large sheets. Like the grain metric it needs a same-run
// control: compare A/B cells captured in ONE sweep with the clock pinned.
//
// Feed it the map_diag 1 render (coverage as opaque greyscale, no stars or
// terrain in it) for a clean floor; on the colour render the no-cloud floor
// is a few hundred components of stars and ground (measured 263 at 873 km).
//
//   node scripts/cloud-speckle-census.js [--t=200] [--amax=64] "label=a.png" "label=b.png"
//
// Prints per image: total components, the cumulative count of components of
// 16 px or less (the G5 speckle count), the luminance-mass share of components
// of AMAX px or less (the G5 speckle mass), the white area, and a size
// histogram so a threshold can be picked from data rather than assumed.
const sharp = require("sharp");
const args = process.argv.slice(2);
const opt = { t: 200, amax: 64 };
const files = [];
for (const a of args) {
  const m = a.match(/^--(t|amax)=(\d+)$/);
  if (m) opt[m[1]] = +m[2]; else files.push(a);
}
const T = opt.t, AMAX = opt.amax;
(async () => {
  for (const arg of files) {
    const eq = arg.indexOf("=");
    const tag = eq >= 0 ? arg.slice(0, eq) : require("path").basename(arg);
    const f = eq >= 0 ? arg.slice(eq + 1) : arg;
    const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height;
    const x0 = Math.floor(W * 0.1), x1 = Math.floor(W * 0.9), y0 = Math.floor(H * 0.1), y1 = Math.floor(H * 0.9);
    const lab = new Int32Array(W * H).fill(-1);
    const sizes = [];
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
      const i = y * W + x;
      if (data[i] <= T || lab[i] >= 0) continue;
      const id = sizes.length; let n = 0; const st = [i]; lab[i] = id;
      while (st.length) {
        const j = st.pop(); n++; const jx = j % W, jy = (j - jx) / W;
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const nx = jx + dx, ny = jy + dy;
          if (nx < x0 || ny < y0 || nx >= x1 || ny >= y1) continue;
          const k = ny * W + nx;
          if (data[k] > T && lab[k] < 0) { lab[k] = id; st.push(k); }
        }
      }
      sizes.push(n);
    }
    const bins = [1, 4, 16, 64, 256, 1024, 4096, 1e9];
    const hist = new Array(bins.length).fill(0), harea = new Array(bins.length).fill(0);
    let tot = 0;
    for (const s of sizes) { tot += s; for (let b = 0; b < bins.length; b++) if (s <= bins[b]) { hist[b]++; harea[b] += s; break; } }
    const small16 = sizes.filter(s => s <= 16).length;
    const speck = sizes.filter(s => s <= AMAX); const sa = speck.reduce((a, b) => a + b, 0);
    const area = (x1 - x0) * (y1 - y0);
    console.log(tag.padEnd(28), "T>" + T, "comps", sizes.length, "speckles(<=16px)", small16,
      "mass(<=" + AMAX + "px)", (100 * sa / area).toFixed(3) + "%", "white area", (100 * tot / area).toFixed(2) + "%",
      "speckle share of white", tot ? (100 * sa / tot).toFixed(1) + "%" : "-");
    console.log("   size hist (<=px: count / area%)", bins.map((b, i) => (b >= 1e9 ? ">4096" : "<=" + b) + ":" + hist[i] + "/" + (100 * harea[i] / area).toFixed(2)).join("  "));
  }
})();
