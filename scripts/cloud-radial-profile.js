// Rotation-invariant radial luminance profile about the nadir pixel: mean in
// concentric annuli. Usage: node scripts/_radial_profile.js CX CY a.png b.png ...
const sharp = require("sharp");
const CX = +process.argv[2], CY = +process.argv[3];
const EDGES = [0, 150, 300, 450, 600, 800, 1000];
(async () => {
  for (const f of process.argv.slice(4)) {
    const { data, info } = await sharp(f).greyscale().raw().toBuffer({ resolveWithObject: true });
    const W = info.width, H = info.height; const s = new Array(EDGES.length - 1).fill(0), n = new Array(EDGES.length - 1).fill(0);
    for (let y = 90; y < H - 90; y += 2) for (let x = 0; x < W; x += 2) {
      const r = Math.hypot(x - CX, y - CY); for (let b = 0; b < EDGES.length - 1; b++) if (r >= EDGES[b] && r < EDGES[b + 1]) { s[b] += data[y * W + x]; n[b]++; break; }
    }
    console.log(f.replace(/.*[\/]/, "").padEnd(24), s.map((v, i) => (n[i] ? (v / n[i]).toFixed(0) : "-").padStart(4)).join(" "), "  (annuli " + EDGES.join("/") + " px)");
  }
})();
