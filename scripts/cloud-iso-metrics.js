// Metrics for an A/B sweep of tmp-<prefix>-<state>-d<diag>-s<0|1> captures:
// radial energy about the nadir pixel, Laplacian grain, mean luminance and
// cloud fraction, in ONE process (per-file node invocations time out).
//   node scripts/cloud-iso-metrics.js <sweep-dir> <prefix> <state,...> <diag,...> [CY]
const sharp = require("sharp"); const fs = require("fs");
const d = process.argv[2], prefix = process.argv[3];
const states = process.argv[4].split(","), diags = process.argv[5].split(",");
const CY = +(process.argv[6] || 873);
async function load(f) { const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true }); return { data, W: info.width, H: info.height }; }
function spoke(img, CX, CY, R1) { const { data, W } = img; const NA = 720, R0 = 30; const A = new Float64Array(NA); for (let ai = 0; ai < NA; ai++) { const th = ai * 2 * Math.PI / NA, cs = Math.cos(th), sn = Math.sin(th); let s = 0, n = 0; for (let r = R0; r < R1; r += 0.5) { const x = Math.round(CX + cs * r), y = Math.round(CY + sn * r); s += data[y * W + x]; n++; } A[ai] = s / n; } const half = 48; let s = 0; for (let i = 0; i < NA; i++) { let m = 0; for (let k = -half; k <= half; k++) m += A[(i + k + NA) % NA]; m /= (2 * half + 1); s += (A[i] - m) ** 2; } return Math.sqrt(s / NA); }
function grain(img) { const { data, W, H } = img; let s = 0, n = 0; for (let y = Math.floor(H * 0.2); y < H * 0.8; y++) for (let x = Math.floor(W * 0.2); x < W * 0.8; x++) { const i = y * W + x; s += Math.abs(4 * data[i] - data[i - 1] - data[i + 1] - data[i - W] - data[i + W]); n++; } return s / n; }
function frac(img) { const { data, W, H } = img; let s = 0, n = 0, c = 0; for (let y = Math.floor(H * 0.2); y < H * 0.8; y += 2) for (let x = Math.floor(W * 0.2); x < W * 0.8; x += 2) { const l = data[y * W + x]; s += l; n++; if (l > 235) c++; } return (s / n).toFixed(1) + " " + (100 * c / n).toFixed(1) + "%"; }
(async () => {
  for (const t of states) {
    console.log("== " + t + " ==");
    for (const dg of diags) for (const s of ["0", "1"]) {
      const f = d + "/" + prefix + "-" + t + "-d" + dg + "-s" + s + ".png";
      if (!fs.existsSync(f)) { console.log("  (missing " + f + ")"); continue; }
      const img = await load(f);
      console.log("  diag" + dg + " s=" + s + "  spoke " + spoke(img, 1280, CY, 300).toFixed(2).padEnd(7) + " grain " + grain(img).toFixed(2).padEnd(6) + " mean/cloud% " + frac(img));
    }
  }
})();
