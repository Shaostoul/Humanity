// Increment-A gate metrics in ONE process (design 1.5, v0.1280).
//   node scripts/cloud-ab-metrics.js <sweep-dir> <mask.png> <CX> <CY> <label=file> ...
// For each file: cloud-masked mean / median / p10 / p90 luminance and B/R over
// the coverage mask (alpha > 128 in the map_diag-1 capture), radial coherence
// bins at (CX,CY), Laplacian grain, cloud fraction, and fps from the manifest.
const sharp = require("sharp"); const fs = require("fs"); const path = require("path");
const dir = process.argv[2], maskFile = process.argv[3];
const CX = +process.argv[4], CY = +process.argv[5];
const items = process.argv.slice(6).map(a => { const i = a.indexOf("="); return [a.slice(0, i), a.slice(i + 1)]; });
const BINS = [[40, 160], [160, 320], [320, 480], [480, 640], [640, 800]];

async function raw(f, ch) { const img = sharp(f); const { data, info } = await (ch === 1 ? img.greyscale() : img.removeAlpha()).raw().toBuffer({ resolveWithObject: true }); return { data, W: info.width, H: info.height, C: info.channels }; }
function coherence(g, cx, cy) {
  const W = Math.floor(g.W / 4), H = Math.floor(g.H / 4), d = new Float32Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) { let s = 0; for (let j = 0; j < 4; j++) for (let i = 0; i < 4; i++) s += g.data[(y * 4 + j) * g.W + x * 4 + i]; d[y * W + x] = s / 16; }
  const num = BINS.map(() => 0), den = BINS.map(() => 0); const cx4 = cx / 4, cy4 = cy / 4;
  for (let y = 1; y < H - 1; y++) for (let x = 1; x < W - 1; x++) {
    const i = y * W + x;
    const gx = (d[i - W + 1] + 2 * d[i + 1] + d[i + W + 1]) - (d[i - W - 1] + 2 * d[i - 1] + d[i + W - 1]);
    const gy = (d[i + W - 1] + 2 * d[i + W] + d[i + W + 1]) - (d[i - W - 1] + 2 * d[i - W] + d[i - W + 1]);
    const m = Math.hypot(gx, gy); if (m < 96) continue;
    const r = Math.hypot(x - cx4, y - cy4) * 4; let b = -1; for (let k = 0; k < BINS.length; k++) if (r >= BINS[k][0] && r < BINS[k][1]) b = k; if (b < 0) continue;
    const psi = Math.atan2(gx, -gy) - Math.atan2(y - cy4, x - cx4); num[b] += m * Math.cos(2 * psi); den[b] += m;
  }
  return BINS.map((_, k) => den[k] > 0 ? num[k] / den[k] : 0);
}
(async () => {
  const mask = await raw(path.join(dir, maskFile), 1);
  let manifest = {}; try { const m = JSON.parse(fs.readFileSync(path.join(dir, "manifest.json"), "utf8")); for (const r of (m.captures || m.vantages || m.results || [])) manifest[r.id || r.vantage || ""] = r.fps || r.frame_fps; } catch (e) {}
  console.log("mask: " + maskFile + "   centre (" + CX + "," + CY + ")   bins " + BINS.map(b => b[0] + "-" + b[1]).join(" "));
  for (const [label, file] of items) {
    const f = path.join(dir, file); if (!fs.existsSync(f)) { console.log(label.padEnd(14) + " (missing)"); continue; }
    const c = await raw(f, 3); const g = await raw(f, 1);
    const W = c.W, H = c.H; const lum = [], rgb = [0, 0, 0]; let n = 0, white = 0, tot = 0, lap = 0;
    for (let y = Math.floor(H * 0.15); y < H * 0.9; y += 2) for (let x = Math.floor(W * 0.1); x < W * 0.9; x += 2) {
      const i = y * W + x; tot++; if (g.data[i] > 235) white++;
      if (y > 0 && y < H - 1 && x > 0 && x < W - 1) lap += Math.abs(4 * g.data[i] - g.data[i - 1] - g.data[i + 1] - g.data[i - W] - g.data[i + W]);
      if (mask.data[i] <= 128) continue;
      const k = i * 3; lum.push(g.data[i]); rgb[0] += c.data[k]; rgb[1] += c.data[k + 1]; rgb[2] += c.data[k + 2]; n++;
    }
    lum.sort((a, b) => a - b); const q = p => lum.length ? lum[Math.floor(p * (lum.length - 1))] : 0;
    const mean = lum.length ? lum.reduce((a, b) => a + b, 0) / lum.length : 0;
    const br = rgb[0] > 0 ? rgb[2] / rgb[0] : 0;
    const A = coherence(g, CX, CY);
    const fps = manifest[file.replace(/\.png$/, "")];
    console.log(label.padEnd(14) + " masked mean " + mean.toFixed(1).padStart(5) + " med " + String(q(0.5)).padStart(3) + " p10 " + String(q(0.1)).padStart(3) + " p90 " + String(q(0.9)).padStart(3) + "  B/R " + br.toFixed(3) + "  grain " + (lap / tot).toFixed(2) + "  white " + (100 * white / tot).toFixed(1) + "%  A " + A.map(v => (v >= 0 ? "+" : "") + v.toFixed(2)).join(" ") + (fps ? "  fps " + fps : ""));
  }
})();
