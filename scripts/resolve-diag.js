// Read out the cloud RESOLVE diagnostic build.
//
// The diagnostic build makes assets/shaders/cloud_resolve.wgsl return
//   vec4(noise_w, alpha, 0, 1)
// instead of the resolved colour, so a capture carries the two numbers the
// TV-static arc had been reasoning about without data:
//
//   R = noise_w  how hard the variance-adaptive SPATIAL filter engages (0..1)
//   G = alpha    the TEMPORAL blend rate; low means deep accumulation
//
// Design intent, from the shader's own comments: at rest alpha should sit near
// 0.12 (the anti-boil cap, ~8-frame average) and noise_w should be near 1 on a
// noisy neighbourhood. The spatial strength applied is
// noise_w * mix(0.35, 0.75, shallow), where shallow rises with alpha, so the
// two filters deliberately defer to each other.
//
// usage: node scripts/resolve-diag.js <capture.png> [--crop x0,x1,y0,y1]
const sharp = require("sharp");

const args = process.argv.slice(2);
let crop = [0.2, 0.8, 0.2, 0.8];
const files = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--crop") crop = args[++i].split(",").map(Number);
  else files.push(args[i]);
}

(async () => {
  for (const f of files) {
    const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height, C = info.channels;
    const x0 = Math.floor(W * crop[0]), x1 = Math.floor(W * crop[1]);
    const y0 = Math.floor(H * crop[2]), y1 = Math.floor(H * crop[3]);
    let n = 0, sn = 0, sa = 0, hiN = 0, deepA = 0, midA = 0;
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        const i = (y * W + x) * C;
        const nw = data[i] / 255, al = data[i + 1] / 255;
        n++; sn += nw; sa += al;
        if (nw > 0.5) hiN++;
        if (al < 0.2) deepA++;
        else if (al < 0.6) midA++;
      }
    }
    console.log(f.split(/[\/]/).slice(-1)[0]);
    console.log("  noise_w  mean " + (sn / n).toFixed(3) + "   strongly engaged (>0.5): " + ((100 * hiN) / n).toFixed(1) + "%");
    console.log("  alpha    mean " + (sa / n).toFixed(3) + "   deep (<0.2): " + ((100 * deepA) / n).toFixed(1) + "%   mid (0.2-0.6): " + ((100 * midA) / n).toFixed(1) + "%");
  }
})();
