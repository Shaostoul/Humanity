// Rotation-invariant radial luminance profile about the nadir pixel: mean in
// concentric annuli. Usage: node scripts/cloud-radial-profile.js CX CY [--step=N] [--srgb] a.png b.png ...
// Default annuli 0/150/300/450/600/800/1000 px; --step=N (far rung, G0b) uses
// N-px annuli out to the frame corner, and prints each annulus's ratio to the
// median annulus (an annulus above 3x the median inside the predicted band is
// the HARD-vs-auto ring the prove-red looks for).
// --srgb (D1, 2026-09-06): every capture leaves through the sRGB swapchain, so
// a diag-channel render (map_diag 1 / 4 / 10 / 11 / 12) is sRGB-ENCODED and a
// mean or a ratio of its bytes is not a mean or a ratio of the quantity (raw
// bytes read L + r (6 - L) with r about 0.5 on the level map). With --srgb each
// byte is decoded to linear before averaging and the means are printed on the
// same 0..255 scale (linear * 255), so annulus ratios are ratios of coverage.
const sharp = require("sharp");
const argv = process.argv.slice(2);
const stepArg = argv.find(a => a.startsWith("--step="));
const SRGB = argv.includes("--srgb");
const files = argv.filter(a => !a.startsWith("--"));
const CX = +files[0], CY = +files[1];
const STEP = stepArg ? +stepArg.slice(7) : 0;
// Byte -> linear * 255 lookup (identity without --srgb).
const LUT = new Float32Array(256);
for (let i = 0; i < 256; i++) {
  const c = i / 255;
  LUT[i] = SRGB ? 255 * (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)) : i;
}
(async () => {
  for (const f of files.slice(2)) {
    const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height;
    const rmax = Math.ceil(Math.hypot(Math.max(CX, W - CX), Math.max(CY, H - CY)));
    const EDGES = STEP > 0
      ? Array.from({ length: Math.ceil(rmax / STEP) + 1 }, (_, i) => i * STEP)
      : [0, 150, 300, 450, 600, 800, 1000];
    const s = new Array(EDGES.length - 1).fill(0), n = new Array(EDGES.length - 1).fill(0);
    for (let y = 90; y < H - 90; y += 2) for (let x = 0; x < W; x += 2) {
      const r = Math.hypot(x - CX, y - CY);
      const b = STEP > 0 ? Math.floor(r / STEP) : EDGES.findIndex((e, i) => i < EDGES.length - 1 && r >= e && r < EDGES[i + 1]);
      if (b >= 0 && b < s.length) { s[b] += LUT[data[y * W + x]]; n[b]++; } // LUT = linear * 255 under --srgb, identity otherwise
    }
    const means = s.map((v, i) => (n[i] ? v / n[i] : NaN));
    const name = f.replace(/.*[\/]/, "").padEnd(24);
    if (STEP > 0) {
      const valid = means.filter(m => !isNaN(m)).sort((a, b) => a - b);
      const median = valid.length ? valid[Math.floor(valid.length / 2)] : 1;
      console.log(name + "  (annuli of " + STEP + " px, median annulus " + median.toFixed(1) + ")");
      console.log("  " + means.map((m, i) => (isNaN(m) ? "" : `${String(EDGES[i]).padStart(5)}:${m.toFixed(0).padStart(4)} x${(m / Math.max(median, 1e-6)).toFixed(2)}`)).filter(Boolean).join("  "));
    } else {
      console.log(name, means.map(m => (isNaN(m) ? "-" : m.toFixed(0)).padStart(4)).join(" "), "  (annuli " + EDGES.join("/") + " px)");
    }
  }
})();
