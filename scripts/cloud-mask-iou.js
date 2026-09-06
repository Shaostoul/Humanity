// Silhouette IoU between coverage masks (the far rung's G3 descent-ladder gate).
// Feed map_diag 1 renders (coverage as opaque greyscale): each is thresholded
// on LUMINANCE (> T, default 128; the diag's alpha is always 255, never use it),
// cubic-resized to S x S (default 512) so rungs at different altitudes compare
// at one scale, and the intersection-over-union is printed for every
// CONSECUTIVE pair in the argument order, plus the coverage fraction of each.
// Adjacent rungs whose altitude ratio is at most 1.6 should read at or above
// 0.97 on a band-limited field; today's point-sampled bodies read far lower.
//
//   node scripts/cloud-mask-iou.js [--t=128] [--size=512] [--crop=0.8] "label=a.png" "label=b.png" ...
//
// --crop keeps the central fraction of each frame (HUD excluded) before resizing.
const sharp = require("sharp");
const args = process.argv.slice(2);
const opt = { t: 128, size: 512, crop: 0.8 };
const files = [];
for (const a of args) { const m = a.match(/^--(t|size|crop)=([\d.]+)$/); if (m) opt[m[1]] = +m[2]; else files.push(a); }
async function mask(f) {
  const meta = await sharp(f).metadata();
  const W = meta.width, H = meta.height;
  const cw = Math.floor(W * opt.crop), ch = Math.floor(H * opt.crop);
  const { data } = await sharp(f).extract({ left: Math.floor((W - cw) / 2), top: Math.floor((H - ch) / 2), width: cw, height: ch })
    .greyscale().resize(opt.size, opt.size, { kernel: "cubic" }).raw().toBuffer({ resolveWithObject: true });
  const m = new Uint8Array(opt.size * opt.size); let n = 0;
  for (let i = 0; i < m.length; i++) { if (data[i] > opt.t) { m[i] = 1; n++; } }
  return { m, frac: n / m.length };
}
(async () => {
  const items = [];
  for (const arg of files) { const eq = arg.indexOf("="); const tag = eq >= 0 ? arg.slice(0, eq) : require("path").basename(arg); const f = eq >= 0 ? arg.slice(eq + 1) : arg; items.push({ tag, ...(await mask(f)) }); }
  for (const it of items) console.log(it.tag.padEnd(30), "coverage", (100 * it.frac).toFixed(2) + "%");
  for (let k = 1; k < items.length; k++) {
    const a = items[k - 1].m, b = items[k].m; let inter = 0, uni = 0;
    for (let i = 0; i < a.length; i++) { if (a[i] & b[i]) inter++; if (a[i] | b[i]) uni++; }
    console.log("IoU", items[k - 1].tag, "vs", items[k].tag, uni ? (inter / uni).toFixed(4) : "empty");
  }
})();
